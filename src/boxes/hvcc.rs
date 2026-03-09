//! HEVC Codec Configuration Box (hvcC) parsing and serialization.
//!
//! The HEVC Codec Configuration Box specifies the HEVC/H.265 codec parameters.
//!
//! ```text
//! class HEVCConfigurationBox extends Box('hvcC') {
//!    HEVCDecoderConfigurationRecord() HEVCConfig;
//! }
//!
//! aligned(8) class HEVCDecoderConfigurationRecord {
//!    unsigned int(8) configurationVersion = 1;
//!    unsigned int(2) general_profile_space;
//!    unsigned int(1) general_tier_flag;
//!    unsigned int(5) general_profile_idc;
//!    unsigned int(32) general_profile_compatibility_flags;
//!    unsigned int(48) general_constraint_indicator_flags;
//!    unsigned int(8) general_level_idc;
//!    bit(4) reserved = '1111'b;
//!    unsigned int(12) min_spatial_segmentation_idc;
//!    bit(6) reserved = '111111'b;
//!    unsigned int(2) parallelismType;
//!    bit(6) reserved = '111111'b;
//!    unsigned int(2) chroma_format_idc;
//!    bit(5) reserved = '11111'b;
//!    unsigned int(3) bit_depth_luma_minus8;
//!    bit(5) reserved = '11111'b;
//!    unsigned int(3) bit_depth_chroma_minus8;
//!    unsigned int(16) avgFrameRate;
//!    unsigned int(2) constantFrameRate;
//!    unsigned int(3) numTemporalLayers;
//!    unsigned int(1) temporalIdNested;
//!    unsigned int(2) lengthSizeMinusOne;
//!    unsigned int(8) numOfArrays;
//!    for (j=0; j < numOfArrays; j++) {
//!       unsigned int(1) array_completeness;
//!       bit(1) reserved = 0;
//!       unsigned int(6) NAL_unit_type;
//!       unsigned int(16) numNalus;
//!       for (i=0; i< numNalus; i++) {
//!          unsigned int(16) nalUnitLength;
//!          bit(8*nalUnitLength) nalUnit;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for HEVCConfigurationBox.
pub const BOX_TYPE: BoxCode = BoxCode::HVCC;

/// Common interface for accessing HEVCConfigurationBox data.
pub trait HEVCConfigurationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the configuration version.
    fn configuration_version(&self) -> u8;

    /// Returns the general profile space.
    fn general_profile_space(&self) -> u8;

    /// Returns the general tier flag.
    fn general_tier_flag(&self) -> bool;

    /// Returns the general profile IDC.
    fn general_profile_idc(&self) -> u8;

    /// Returns the general profile compatibility flags.
    fn general_profile_compatibility_flags(&self) -> u32;

    /// Returns the general level IDC.
    fn general_level_idc(&self) -> u8;

    /// Returns the chroma format IDC.
    fn chroma_format_idc(&self) -> u8;

    /// Returns the bit depth luma minus 8.
    fn bit_depth_luma_minus8(&self) -> u8;

    /// Returns the bit depth chroma minus 8.
    fn bit_depth_chroma_minus8(&self) -> u8;

    /// Returns the general constraint indicator flags (6 bytes).
    fn general_constraint_indicator_flags(&self) -> [u8; 6];

    /// Returns the min spatial segmentation IDC.
    fn min_spatial_segmentation_idc(&self) -> u16;

    /// Returns the parallelism type.
    fn parallelism_type(&self) -> u8;

    /// Returns the average frame rate.
    fn avg_frame_rate(&self) -> u16;

    /// Returns the constant frame rate.
    fn constant_frame_rate(&self) -> u8;

    /// Returns the number of temporal layers.
    fn num_temporal_layers(&self) -> u8;

    /// Returns the temporal ID nested flag.
    fn temporal_id_nested(&self) -> bool;

    /// Returns the length size minus one.
    fn length_size_minus_one(&self) -> u8;

    /// Returns the NAL unit arrays (includes num_of_arrays byte and array data).
    fn nal_units(&self) -> &[u8];
}

/// A borrowing view over raw HEVCConfigurationBox bytes.
#[derive(Clone, Copy)]
pub struct HEVCConfigurationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> HEVCConfigurationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 22)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> HEVCConfigurationBox for HEVCConfigurationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn configuration_version(&self) -> u8 {
        self.data[self.header_size]
    }

    fn general_profile_space(&self) -> u8 {
        (self.data[self.header_size + 1] >> 6) & 0x03
    }

    fn general_tier_flag(&self) -> bool {
        (self.data[self.header_size + 1] >> 5) & 0x01 != 0
    }

    fn general_profile_idc(&self) -> u8 {
        self.data[self.header_size + 1] & 0x1F
    }

    fn general_profile_compatibility_flags(&self) -> u32 {
        let o = self.header_size + 2;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn general_level_idc(&self) -> u8 {
        self.data[self.header_size + 12]
    }

    fn chroma_format_idc(&self) -> u8 {
        self.data[self.header_size + 16] & 0x03
    }

    fn bit_depth_luma_minus8(&self) -> u8 {
        self.data[self.header_size + 17] & 0x07
    }

    fn bit_depth_chroma_minus8(&self) -> u8 {
        self.data[self.header_size + 18] & 0x07
    }

    fn general_constraint_indicator_flags(&self) -> [u8; 6] {
        let o = self.header_size + 6;
        let mut flags = [0u8; 6];
        flags.copy_from_slice(&self.data[o..o + 6]);
        flags
    }

    fn min_spatial_segmentation_idc(&self) -> u16 {
        let o = self.header_size + 13;
        BigEndian::read_u16(&self.data[o..o + 2]) & 0x0FFF
    }

    fn parallelism_type(&self) -> u8 {
        self.data[self.header_size + 15] & 0x03
    }

    fn avg_frame_rate(&self) -> u16 {
        let o = self.header_size + 19;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn constant_frame_rate(&self) -> u8 {
        (self.data[self.header_size + 21] >> 6) & 0x03
    }

    fn num_temporal_layers(&self) -> u8 {
        (self.data[self.header_size + 21] >> 3) & 0x07
    }

    fn temporal_id_nested(&self) -> bool {
        (self.data[self.header_size + 21] >> 2) & 0x01 != 0
    }

    fn length_size_minus_one(&self) -> u8 {
        self.data[self.header_size + 21] & 0x03
    }

    fn nal_units(&self) -> &[u8] {
        let start = self.header_size + 22;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl std::fmt::Debug for HEVCConfigurationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HEVCConfigurationBoxView")
            .field("configuration_version", &self.configuration_version())
            .field("general_profile_idc", &self.general_profile_idc())
            .field("general_level_idc", &self.general_level_idc())
            .finish()
    }
}

/// An owned representation of HEVCConfigurationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HEVCConfigurationBoxOwned {
    /// Configuration version (should be 1).
    pub configuration_version: u8,
    /// General profile space.
    pub general_profile_space: u8,
    /// General tier flag.
    pub general_tier_flag: bool,
    /// General profile IDC.
    pub general_profile_idc: u8,
    /// General profile compatibility flags.
    pub general_profile_compatibility_flags: u32,
    /// General constraint indicator flags (6 bytes).
    pub general_constraint_indicator_flags: [u8; 6],
    /// General level IDC.
    pub general_level_idc: u8,
    /// Min spatial segmentation IDC.
    pub min_spatial_segmentation_idc: u16,
    /// Parallelism type.
    pub parallelism_type: u8,
    /// Chroma format IDC.
    pub chroma_format_idc: u8,
    /// Bit depth luma minus 8.
    pub bit_depth_luma_minus8: u8,
    /// Bit depth chroma minus 8.
    pub bit_depth_chroma_minus8: u8,
    /// Average frame rate.
    pub avg_frame_rate: u16,
    /// Constant frame rate.
    pub constant_frame_rate: u8,
    /// Number of temporal layers.
    pub num_temporal_layers: u8,
    /// Temporal ID nested.
    pub temporal_id_nested: bool,
    /// Length size minus one.
    pub length_size_minus_one: u8,
    /// NAL unit arrays.
    pub nal_units: Vec<u8>,
}

impl HEVCConfigurationBoxOwned {
    /// Creates a new HEVCConfigurationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (22 + self.nal_units.len()) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;

        writer.write_u8(self.configuration_version)?;

        let byte1 = ((self.general_profile_space & 0x03) << 6)
            | ((self.general_tier_flag as u8) << 5)
            | (self.general_profile_idc & 0x1F);
        writer.write_u8(byte1)?;

        writer.write_u32::<BigEndian>(self.general_profile_compatibility_flags)?;
        writer.write_all(&self.general_constraint_indicator_flags)?;
        writer.write_u8(self.general_level_idc)?;

        writer.write_u16::<BigEndian>(0xF000 | (self.min_spatial_segmentation_idc & 0x0FFF))?;
        writer.write_u8(0xFC | (self.parallelism_type & 0x03))?;
        writer.write_u8(0xFC | (self.chroma_format_idc & 0x03))?;
        writer.write_u8(0xF8 | (self.bit_depth_luma_minus8 & 0x07))?;
        writer.write_u8(0xF8 | (self.bit_depth_chroma_minus8 & 0x07))?;
        writer.write_u16::<BigEndian>(self.avg_frame_rate)?;

        let byte22 = ((self.constant_frame_rate & 0x03) << 6)
            | ((self.num_temporal_layers & 0x07) << 3)
            | ((self.temporal_id_nested as u8) << 2)
            | (self.length_size_minus_one & 0x03);
        writer.write_u8(byte22)?;

        writer.write_all(&self.nal_units)?;

        Ok(())
    }
}

impl Default for HEVCConfigurationBoxOwned {
    fn default() -> Self {
        Self {
            configuration_version: 1,
            general_profile_space: 0,
            general_tier_flag: false,
            general_profile_idc: 0,
            general_profile_compatibility_flags: 0,
            general_constraint_indicator_flags: [0; 6],
            general_level_idc: 0,
            min_spatial_segmentation_idc: 0,
            parallelism_type: 0,
            chroma_format_idc: 1,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            avg_frame_rate: 0,
            constant_frame_rate: 0,
            num_temporal_layers: 1,
            temporal_id_nested: false,
            length_size_minus_one: 3,
            nal_units: Vec::new(),
        }
    }
}

impl HEVCConfigurationBox for HEVCConfigurationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn configuration_version(&self) -> u8 {
        self.configuration_version
    }

    fn general_profile_space(&self) -> u8 {
        self.general_profile_space
    }

    fn general_tier_flag(&self) -> bool {
        self.general_tier_flag
    }

    fn general_profile_idc(&self) -> u8 {
        self.general_profile_idc
    }

    fn general_profile_compatibility_flags(&self) -> u32 {
        self.general_profile_compatibility_flags
    }

    fn general_level_idc(&self) -> u8 {
        self.general_level_idc
    }

    fn chroma_format_idc(&self) -> u8 {
        self.chroma_format_idc
    }

    fn bit_depth_luma_minus8(&self) -> u8 {
        self.bit_depth_luma_minus8
    }

    fn bit_depth_chroma_minus8(&self) -> u8 {
        self.bit_depth_chroma_minus8
    }

    fn general_constraint_indicator_flags(&self) -> [u8; 6] {
        self.general_constraint_indicator_flags
    }

    fn min_spatial_segmentation_idc(&self) -> u16 {
        self.min_spatial_segmentation_idc
    }

    fn parallelism_type(&self) -> u8 {
        self.parallelism_type
    }

    fn avg_frame_rate(&self) -> u16 {
        self.avg_frame_rate
    }

    fn constant_frame_rate(&self) -> u8 {
        self.constant_frame_rate
    }

    fn num_temporal_layers(&self) -> u8 {
        self.num_temporal_layers
    }

    fn temporal_id_nested(&self) -> bool {
        self.temporal_id_nested
    }

    fn length_size_minus_one(&self) -> u8 {
        self.length_size_minus_one
    }

    fn nal_units(&self) -> &[u8] {
        &self.nal_units
    }
}

impl<T: HEVCConfigurationBox> From<&T> for HEVCConfigurationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            configuration_version: source.configuration_version(),
            general_profile_space: source.general_profile_space(),
            general_tier_flag: source.general_tier_flag(),
            general_profile_idc: source.general_profile_idc(),
            general_profile_compatibility_flags: source.general_profile_compatibility_flags(),
            general_constraint_indicator_flags: source.general_constraint_indicator_flags(),
            general_level_idc: source.general_level_idc(),
            min_spatial_segmentation_idc: source.min_spatial_segmentation_idc(),
            parallelism_type: source.parallelism_type(),
            chroma_format_idc: source.chroma_format_idc(),
            bit_depth_luma_minus8: source.bit_depth_luma_minus8(),
            bit_depth_chroma_minus8: source.bit_depth_chroma_minus8(),
            avg_frame_rate: source.avg_frame_rate(),
            constant_frame_rate: source.constant_frame_rate(),
            num_temporal_layers: source.num_temporal_layers(),
            temporal_id_nested: source.temporal_id_nested(),
            length_size_minus_one: source.length_size_minus_one(),
            nal_units: source.nal_units().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hvcc() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&31u32.to_be_bytes()); // 8 + 23
        data.extend_from_slice(b"hvcC");
        data.push(1); // configuration_version
        data.push(0x00); // profile_space, tier, profile_idc
        data.extend_from_slice(&[0, 0, 0, 0]); // profile_compatibility_flags
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // constraint_indicator_flags
        data.push(0); // level_idc
        data.extend_from_slice(&[0xF0, 0x00]); // min_spatial_segmentation_idc
        data.push(0xFC); // parallelism_type
        data.push(0xFD); // chroma_format_idc = 1
        data.push(0xF8); // bit_depth_luma_minus8
        data.push(0xF8); // bit_depth_chroma_minus8
        data.extend_from_slice(&[0, 0]); // avg_frame_rate
        data.push(0x0F); // constant_frame_rate, num_temporal_layers, temporal_id_nested, length_size_minus_one
        data.push(0); // num_of_arrays
        data
    }

    #[test]
    fn parse_hvcc() {
        let data = make_hvcc();
        let view = HEVCConfigurationBoxView::new(&data).unwrap();

        assert_eq!(view.configuration_version(), 1);
        assert_eq!(view.chroma_format_idc(), 1);
    }

    #[test]
    fn roundtrip() {
        let data = make_hvcc();
        let view = HEVCConfigurationBoxView::new(&data).unwrap();
        let owned = HEVCConfigurationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
