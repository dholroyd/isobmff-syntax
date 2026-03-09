//! AVC Codec Configuration Box (avcC) parsing and serialization.
//!
//! The AVC Codec Configuration Box specifies the AVC/H.264 codec parameters.
//!
//! ```text
//! class AVCConfigurationBox extends Box('avcC') {
//!    AVCDecoderConfigurationRecord() AVCConfig;
//! }
//!
//! aligned(8) class AVCDecoderConfigurationRecord {
//!    unsigned int(8) configurationVersion = 1;
//!    unsigned int(8) AVCProfileIndication;
//!    unsigned int(8) profile_compatibility;
//!    unsigned int(8) AVCLevelIndication;
//!    bit(6) reserved = '111111'b;
//!    unsigned int(2) lengthSizeMinusOne;
//!    bit(3) reserved = '111'b;
//!    unsigned int(5) numOfSequenceParameterSets;
//!    for (i=0; i< numOfSequenceParameterSets; i++) {
//!       unsigned int(16) sequenceParameterSetLength;
//!       bit(8*sequenceParameterSetLength) sequenceParameterSetNALUnit;
//!    }
//!    unsigned int(8) numOfPictureParameterSets;
//!    for (i=0; i< numOfPictureParameterSets; i++) {
//!       unsigned int(16) pictureParameterSetLength;
//!       bit(8*pictureParameterSetLength) pictureParameterSetNALUnit;
//!    }
//!    if (profile_idc == 100 || profile_idc == 110 ||
//!        profile_idc == 122 || profile_idc == 144) {
//!       bit(6) reserved = '111111'b;
//!       unsigned int(2) chroma_format;
//!       bit(5) reserved = '11111'b;
//!       unsigned int(3) bit_depth_luma_minus8;
//!       bit(5) reserved = '11111'b;
//!       unsigned int(3) bit_depth_chroma_minus8;
//!       unsigned int(8) numOfSequenceParameterSetExt;
//!       for (i=0; i< numOfSequenceParameterSetExt; i++) {
//!          unsigned int(16) sequenceParameterSetExtLength;
//!          bit(8*sequenceParameterSetExtLength) sequenceParameterSetExtNALUnit;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for AVCConfigurationBox.
pub const BOX_TYPE: BoxCode = BoxCode::AVCC;

/// Profile IDCs that trigger the high-profile extension fields.
const HIGH_PROFILE_IDCS: [u8; 4] = [100, 110, 122, 144];

/// Common interface for accessing AVCConfigurationBox data.
pub trait AVCConfigurationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the configuration version.
    fn configuration_version(&self) -> u8;

    /// Returns the AVC profile indication.
    fn avc_profile_indication(&self) -> u8;

    /// Returns the profile compatibility.
    fn profile_compatibility(&self) -> u8;

    /// Returns the AVC level indication.
    fn avc_level_indication(&self) -> u8;

    /// Returns the length size minus one.
    fn length_size_minus_one(&self) -> u8;

    /// Returns the number of SPS NAL units.
    fn num_sps(&self) -> u8;

    /// Returns the number of PPS NAL units.
    fn num_pps(&self) -> u8;

    /// Returns an iterator over SPS NAL units.
    fn sps_iter(&self) -> impl Iterator<Item = &[u8]>;

    /// Returns an iterator over PPS NAL units.
    fn pps_iter(&self) -> impl Iterator<Item = &[u8]>;

    /// Returns true if the profile is one of the high profiles (100, 110, 122, 144)
    /// that have additional extension fields.
    fn is_high_profile(&self) -> bool {
        HIGH_PROFILE_IDCS.contains(&self.avc_profile_indication())
    }

    /// Returns the chroma format (0..3), or `None` if not a high profile.
    fn chroma_format(&self) -> Option<u8>;

    /// Returns the bit depth luma minus 8 (0..7), or `None` if not a high profile.
    fn bit_depth_luma_minus8(&self) -> Option<u8>;

    /// Returns the bit depth chroma minus 8 (0..7), or `None` if not a high profile.
    fn bit_depth_chroma_minus8(&self) -> Option<u8>;

    /// Returns an iterator over SPS extension NAL units.
    fn sps_ext_iter(&self) -> impl Iterator<Item = &[u8]>;
}

/// A borrowing view over raw AVCConfigurationBox bytes.
#[derive(Clone, Copy)]
pub struct AVCConfigurationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> AVCConfigurationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 7)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the configuration data after the fixed header.
    pub fn config_data(&self) -> &'a [u8] {
        let start = self.header_size + 6;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }

    /// Returns the byte offset just past all SPS entries.
    fn offset_after_sps(&self) -> usize {
        let num_sps = self.data[self.header_size + 5] & 0x1F;
        let mut offset = self.header_size + 6;
        for _ in 0..num_sps {
            if offset.checked_add(2).is_none_or(|end| end > self.data.len()) {
                return offset;
            }
            let len = BigEndian::read_u16(&self.data[offset..offset + 2]) as usize;
            offset = match offset.checked_add(2).and_then(|v| v.checked_add(len)) {
                Some(v) => v,
                None => return offset,
            };
        }
        offset
    }

    /// Returns the byte offset just past all PPS entries.
    fn offset_after_pps(&self) -> usize {
        let sps_end = self.offset_after_sps();
        if sps_end >= self.data.len() {
            return sps_end;
        }
        let num_pps = self.data[sps_end];
        let mut offset = match sps_end.checked_add(1) {
            Some(v) => v,
            None => return sps_end,
        };
        for _ in 0..num_pps {
            if offset.checked_add(2).is_none_or(|end| end > self.data.len()) {
                return offset;
            }
            let len = BigEndian::read_u16(&self.data[offset..offset + 2]) as usize;
            offset = match offset.checked_add(2).and_then(|v| v.checked_add(len)) {
                Some(v) => v,
                None => return offset,
            };
        }
        offset
    }
}

/// Iterator over NAL units (SPS or PPS) in AVC configuration data.
struct NalUnitIter<'a> {
    data: &'a [u8],
    offset: usize,
    remaining: usize,
}

impl<'a> Iterator for NalUnitIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        if self.offset.checked_add(2).is_none_or(|end| end > self.data.len()) {
            self.remaining = 0;
            return None;
        }
        let len = BigEndian::read_u16(&self.data[self.offset..self.offset + 2]) as usize;
        self.offset += 2;
        let end = match self.offset.checked_add(len) {
            Some(v) if v <= self.data.len() => v,
            _ => {
                self.remaining = 0;
                return None;
            }
        };
        let slice = &self.data[self.offset..end];
        self.offset = end;
        Some(slice)
    }
}

impl<'a> AVCConfigurationBox for AVCConfigurationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn configuration_version(&self) -> u8 {
        self.data[self.header_size]
    }

    fn avc_profile_indication(&self) -> u8 {
        self.data[self.header_size + 1]
    }

    fn profile_compatibility(&self) -> u8 {
        self.data[self.header_size + 2]
    }

    fn avc_level_indication(&self) -> u8 {
        self.data[self.header_size + 3]
    }

    fn length_size_minus_one(&self) -> u8 {
        self.data[self.header_size + 4] & 0x03
    }

    fn num_sps(&self) -> u8 {
        self.data[self.header_size + 5] & 0x1F
    }

    fn num_pps(&self) -> u8 {
        let offset = self.offset_after_sps();
        if offset < self.data.len() {
            self.data[offset]
        } else {
            0
        }
    }

    fn sps_iter(&self) -> impl Iterator<Item = &[u8]> {
        let num_sps = self.data[self.header_size + 5] & 0x1F;
        NalUnitIter {
            data: self.data,
            offset: self.header_size + 6,
            remaining: num_sps as usize,
        }
    }

    fn pps_iter(&self) -> impl Iterator<Item = &[u8]> {
        let offset = self.offset_after_sps();
        if offset < self.data.len() {
            let num_pps = self.data[offset];
            NalUnitIter {
                data: self.data,
                offset: offset + 1,
                remaining: num_pps as usize,
            }
        } else {
            NalUnitIter {
                data: self.data,
                offset: self.data.len(),
                remaining: 0,
            }
        }
    }

    fn chroma_format(&self) -> Option<u8> {
        if !self.is_high_profile() {
            return None;
        }
        let offset = self.offset_after_pps();
        if offset < self.data.len() {
            Some(self.data[offset] & 0x03)
        } else {
            None
        }
    }

    fn bit_depth_luma_minus8(&self) -> Option<u8> {
        if !self.is_high_profile() {
            return None;
        }
        let offset = self.offset_after_pps();
        let field_offset = offset.checked_add(1)?;
        if field_offset < self.data.len() {
            Some(self.data[field_offset] & 0x07)
        } else {
            None
        }
    }

    fn bit_depth_chroma_minus8(&self) -> Option<u8> {
        if !self.is_high_profile() {
            return None;
        }
        let offset = self.offset_after_pps();
        let field_offset = offset.checked_add(2)?;
        if field_offset < self.data.len() {
            Some(self.data[field_offset] & 0x07)
        } else {
            None
        }
    }

    fn sps_ext_iter(&self) -> impl Iterator<Item = &[u8]> {
        if !self.is_high_profile() {
            return NalUnitIter {
                data: self.data,
                offset: self.data.len(),
                remaining: 0,
            };
        }
        let offset = self.offset_after_pps();
        // Need at least 4 bytes: chroma_format, bit_depth_luma, bit_depth_chroma, num_sps_ext
        let count_offset = match offset.checked_add(3) {
            Some(v) if v < self.data.len() => v,
            _ => {
                return NalUnitIter {
                    data: self.data,
                    offset: self.data.len(),
                    remaining: 0,
                };
            }
        };
        let num_sps_ext = self.data[count_offset] as usize;
        NalUnitIter {
            data: self.data,
            offset: count_offset + 1,
            remaining: num_sps_ext,
        }
    }
}

impl std::fmt::Debug for AVCConfigurationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AVCConfigurationBoxView")
            .field("configuration_version", &self.configuration_version())
            .field("avc_profile_indication", &self.avc_profile_indication())
            .field("avc_level_indication", &self.avc_level_indication())
            .finish()
    }
}

/// An owned representation of AVCConfigurationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AVCConfigurationBoxOwned {
    /// Configuration version (should be 1).
    pub configuration_version: u8,
    /// AVC profile indication.
    pub avc_profile_indication: u8,
    /// Profile compatibility.
    pub profile_compatibility: u8,
    /// AVC level indication.
    pub avc_level_indication: u8,
    /// Length size minus one.
    pub length_size_minus_one: u8,
    /// SPS NAL units.
    pub sps_list: Vec<Vec<u8>>,
    /// PPS NAL units.
    pub pps_list: Vec<Vec<u8>>,
    /// Chroma format (0..3). Only meaningful when profile is high (100, 110, 122, 144).
    pub chroma_format: u8,
    /// Bit depth luma minus 8 (0..7). Only meaningful when profile is high.
    pub bit_depth_luma_minus8: u8,
    /// Bit depth chroma minus 8 (0..7). Only meaningful when profile is high.
    pub bit_depth_chroma_minus8: u8,
    /// SPS extension NAL units. Only meaningful when profile is high.
    pub sps_ext_list: Vec<Vec<u8>>,
}

impl AVCConfigurationBoxOwned {
    /// Creates a new AVCConfigurationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 6u64; // fixed config
        for sps in &self.sps_list {
            payload += 2 + sps.len() as u64; // length + data
        }
        payload += 1; // num_pps
        for pps in &self.pps_list {
            payload += 2 + pps.len() as u64; // length + data
        }
        if self.is_high_profile() {
            // chroma_format (1) + bit_depth_luma (1) + bit_depth_chroma (1) + num_sps_ext (1)
            payload += 4;
            for sps_ext in &self.sps_ext_list {
                payload += 2 + sps_ext.len() as u64; // length + data
            }
        }
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;

        writer.write_u8(self.configuration_version)?;
        writer.write_u8(self.avc_profile_indication)?;
        writer.write_u8(self.profile_compatibility)?;
        writer.write_u8(self.avc_level_indication)?;
        writer.write_u8(0xFC | (self.length_size_minus_one & 0x03))?;
        writer.write_u8(0xE0 | (self.sps_list.len() as u8 & 0x1F))?;

        for sps in &self.sps_list {
            writer.write_u16::<BigEndian>(sps.len() as u16)?;
            writer.write_all(sps)?;
        }

        writer.write_u8(self.pps_list.len() as u8)?;
        for pps in &self.pps_list {
            writer.write_u16::<BigEndian>(pps.len() as u16)?;
            writer.write_all(pps)?;
        }

        if self.is_high_profile() {
            writer.write_u8(0xFC | (self.chroma_format & 0x03))?;
            writer.write_u8(0xF8 | (self.bit_depth_luma_minus8 & 0x07))?;
            writer.write_u8(0xF8 | (self.bit_depth_chroma_minus8 & 0x07))?;
            writer.write_u8(self.sps_ext_list.len() as u8)?;
            for sps_ext in &self.sps_ext_list {
                writer.write_u16::<BigEndian>(sps_ext.len() as u16)?;
                writer.write_all(sps_ext)?;
            }
        }

        Ok(())
    }
}

impl Default for AVCConfigurationBoxOwned {
    fn default() -> Self {
        Self {
            configuration_version: 1,
            avc_profile_indication: 0,
            profile_compatibility: 0,
            avc_level_indication: 0,
            length_size_minus_one: 3,
            sps_list: Vec::new(),
            pps_list: Vec::new(),
            chroma_format: 0,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            sps_ext_list: Vec::new(),
        }
    }
}

impl AVCConfigurationBox for AVCConfigurationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn configuration_version(&self) -> u8 {
        self.configuration_version
    }

    fn avc_profile_indication(&self) -> u8 {
        self.avc_profile_indication
    }

    fn profile_compatibility(&self) -> u8 {
        self.profile_compatibility
    }

    fn avc_level_indication(&self) -> u8 {
        self.avc_level_indication
    }

    fn length_size_minus_one(&self) -> u8 {
        self.length_size_minus_one
    }

    fn num_sps(&self) -> u8 {
        self.sps_list.len() as u8
    }

    fn num_pps(&self) -> u8 {
        self.pps_list.len() as u8
    }

    fn sps_iter(&self) -> impl Iterator<Item = &[u8]> {
        self.sps_list.iter().map(|v| v.as_slice())
    }

    fn pps_iter(&self) -> impl Iterator<Item = &[u8]> {
        self.pps_list.iter().map(|v| v.as_slice())
    }

    fn chroma_format(&self) -> Option<u8> {
        if self.is_high_profile() {
            Some(self.chroma_format)
        } else {
            None
        }
    }

    fn bit_depth_luma_minus8(&self) -> Option<u8> {
        if self.is_high_profile() {
            Some(self.bit_depth_luma_minus8)
        } else {
            None
        }
    }

    fn bit_depth_chroma_minus8(&self) -> Option<u8> {
        if self.is_high_profile() {
            Some(self.bit_depth_chroma_minus8)
        } else {
            None
        }
    }

    fn sps_ext_iter(&self) -> impl Iterator<Item = &[u8]> {
        let slice = if self.is_high_profile() {
            self.sps_ext_list.as_slice()
        } else {
            &[]
        };
        slice.iter().map(|v| v.as_slice())
    }
}

impl TryFrom<&AVCConfigurationBoxView<'_>> for AVCConfigurationBoxOwned {
    type Error = ParseError;

    fn try_from(source: &AVCConfigurationBoxView<'_>) -> Result<Self, Self::Error> {
        let (chroma_format, bit_depth_luma_minus8, bit_depth_chroma_minus8) =
            if source.is_high_profile() {
                (
                    source.chroma_format().ok_or(ParseError::BufferTooShort {
                        expected: source.offset_after_pps() + 1,
                        found: source.data.len(),
                    })?,
                    source.bit_depth_luma_minus8().ok_or(ParseError::BufferTooShort {
                        expected: source.offset_after_pps() + 2,
                        found: source.data.len(),
                    })?,
                    source.bit_depth_chroma_minus8().ok_or(ParseError::BufferTooShort {
                        expected: source.offset_after_pps() + 3,
                        found: source.data.len(),
                    })?,
                )
            } else {
                (0, 0, 0)
            };

        Ok(Self {
            configuration_version: source.configuration_version(),
            avc_profile_indication: source.avc_profile_indication(),
            profile_compatibility: source.profile_compatibility(),
            avc_level_indication: source.avc_level_indication(),
            length_size_minus_one: source.length_size_minus_one(),
            sps_list: source.sps_iter().map(|s| s.to_vec()).collect(),
            pps_list: source.pps_iter().map(|s| s.to_vec()).collect(),
            chroma_format,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
            sps_ext_list: source.sps_ext_iter().map(|s| s.to_vec()).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_avcc() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&15u32.to_be_bytes()); // 8 + 6 + 1 (empty)
        data.extend_from_slice(b"avcC");
        data.push(1); // configuration_version
        data.push(66); // avc_profile_indication (Baseline profile)
        data.push(0); // profile_compatibility
        data.push(31); // avc_level_indication (3.1)
        data.push(0xFF); // length_size_minus_one = 3
        data.push(0xE0); // num_sps = 0
        data.push(0); // num_pps = 0
        data
    }

    fn make_avcc_high_profile() -> Vec<u8> {
        let sps: &[u8] = &[0x67, 0x64, 0x00, 0x1F]; // fake SPS
        let pps: &[u8] = &[0x68, 0xEE, 0x3C, 0x80]; // fake PPS
        let sps_ext: &[u8] = &[0xAA, 0xBB]; // fake SPS ext

        // Total payload:
        //   6 (fixed config)
        // + 2 (sps length) + 4 (sps data)
        // + 1 (num_pps)
        // + 2 (pps length) + 4 (pps data)
        // + 1 (chroma_format) + 1 (bit_depth_luma) + 1 (bit_depth_chroma) + 1 (num_sps_ext)
        // + 2 (sps_ext length) + 2 (sps_ext data)
        // = 6 + 6 + 1 + 6 + 4 + 4 = 27
        let payload_size: u32 = 27;
        let box_size: u32 = 8 + payload_size; // 35

        let mut data = Vec::new();
        data.extend_from_slice(&box_size.to_be_bytes());
        data.extend_from_slice(b"avcC");
        data.push(1); // configuration_version
        data.push(100); // avc_profile_indication (High profile)
        data.push(0); // profile_compatibility
        data.push(31); // avc_level_indication (3.1)
        data.push(0xFF); // reserved(6) + length_size_minus_one = 3
        data.push(0xE1); // reserved(3) + num_sps = 1

        // SPS
        data.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        data.extend_from_slice(sps);

        // PPS count
        data.push(1); // num_pps = 1

        // PPS
        data.extend_from_slice(&(pps.len() as u16).to_be_bytes());
        data.extend_from_slice(pps);

        // High-profile extension fields
        data.push(0xFC | 1); // reserved(6) + chroma_format = 1 (4:2:0)
        data.push(0xF8 | 2); // reserved(5) + bit_depth_luma_minus8 = 2 (10-bit)
        data.push(0xF8 | 2); // reserved(5) + bit_depth_chroma_minus8 = 2 (10-bit)
        data.push(1); // numOfSequenceParameterSetExt = 1

        // SPS ext
        data.extend_from_slice(&(sps_ext.len() as u16).to_be_bytes());
        data.extend_from_slice(sps_ext);

        data
    }

    #[test]
    fn parse_avcc() {
        let data = make_avcc();
        let view = AVCConfigurationBoxView::new(&data).unwrap();

        assert_eq!(view.configuration_version(), 1);
        assert_eq!(view.avc_profile_indication(), 66);
        assert_eq!(view.avc_level_indication(), 31);
        assert!(!view.is_high_profile());
        assert_eq!(view.chroma_format(), None);
        assert_eq!(view.bit_depth_luma_minus8(), None);
        assert_eq!(view.bit_depth_chroma_minus8(), None);
        assert_eq!(view.sps_ext_iter().count(), 0);
    }

    #[test]
    fn parse_avcc_high_profile() {
        let data = make_avcc_high_profile();
        let view = AVCConfigurationBoxView::new(&data).unwrap();

        assert_eq!(view.configuration_version(), 1);
        assert_eq!(view.avc_profile_indication(), 100);
        assert_eq!(view.avc_level_indication(), 31);
        assert!(view.is_high_profile());
        assert_eq!(view.num_sps(), 1);
        assert_eq!(view.num_pps(), 1);
        assert_eq!(view.chroma_format(), Some(1));
        assert_eq!(view.bit_depth_luma_minus8(), Some(2));
        assert_eq!(view.bit_depth_chroma_minus8(), Some(2));

        let sps_ext: Vec<&[u8]> = view.sps_ext_iter().collect();
        assert_eq!(sps_ext.len(), 1);
        assert_eq!(sps_ext[0], &[0xAA, 0xBB]);
    }

    #[test]
    fn roundtrip() {
        let data = make_avcc();
        let view = AVCConfigurationBoxView::new(&data).unwrap();
        let owned = AVCConfigurationBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_high_profile() {
        let data = make_avcc_high_profile();
        let view = AVCConfigurationBoxView::new(&data).unwrap();
        let owned = AVCConfigurationBoxOwned::try_from(&view).unwrap();

        // Verify the owned fields
        assert_eq!(owned.chroma_format, 1);
        assert_eq!(owned.bit_depth_luma_minus8, 2);
        assert_eq!(owned.bit_depth_chroma_minus8, 2);
        assert_eq!(owned.sps_ext_list.len(), 1);
        assert_eq!(owned.sps_ext_list[0], &[0xAA, 0xBB]);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
