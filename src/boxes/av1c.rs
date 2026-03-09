//! AV1 Codec Configuration Box (av1C) parsing and serialization.
//!
//! The AV1 Codec Configuration Box specifies the AV1 codec parameters.

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::WriteBytesExt;
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for AV1CodecConfigurationBox.
pub const BOX_TYPE: BoxCode = BoxCode::AV1C;

/// Common interface for accessing AV1CodecConfigurationBox data.
pub trait AV1CodecConfigurationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the marker and version byte.
    fn marker_version(&self) -> u8;

    /// Returns the seq_profile.
    fn seq_profile(&self) -> u8;

    /// Returns the seq_level_idx_0.
    fn seq_level_idx_0(&self) -> u8;

    /// Returns the high_bitdepth flag.
    fn high_bitdepth(&self) -> bool;

    /// Returns the twelve_bit flag.
    fn twelve_bit(&self) -> bool;

    /// Returns the monochrome flag.
    fn monochrome(&self) -> bool;

    /// Returns the chroma_subsampling_x flag.
    fn chroma_subsampling_x(&self) -> bool;

    /// Returns the chroma_subsampling_y flag.
    fn chroma_subsampling_y(&self) -> bool;

    /// Returns the chroma_sample_position.
    fn chroma_sample_position(&self) -> u8;

    /// Returns the reserved byte.
    fn reserved(&self) -> u8;

    /// Returns the configuration OBUs.
    fn config_obus(&self) -> &[u8];
}

/// A borrowing view over raw AV1CodecConfigurationBox bytes.
#[derive(Clone, Copy)]
pub struct AV1CodecConfigurationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> AV1CodecConfigurationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 4)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> AV1CodecConfigurationBox for AV1CodecConfigurationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn marker_version(&self) -> u8 {
        self.data[self.header_size]
    }

    fn seq_profile(&self) -> u8 {
        (self.data[self.header_size + 1] >> 5) & 0x07
    }

    fn seq_level_idx_0(&self) -> u8 {
        self.data[self.header_size + 1] & 0x1F
    }

    fn high_bitdepth(&self) -> bool {
        (self.data[self.header_size + 2] >> 6) & 0x01 != 0
    }

    fn twelve_bit(&self) -> bool {
        (self.data[self.header_size + 2] >> 5) & 0x01 != 0
    }

    fn monochrome(&self) -> bool {
        (self.data[self.header_size + 2] >> 4) & 0x01 != 0
    }

    fn chroma_subsampling_x(&self) -> bool {
        (self.data[self.header_size + 2] >> 3) & 0x01 != 0
    }

    fn chroma_subsampling_y(&self) -> bool {
        (self.data[self.header_size + 2] >> 2) & 0x01 != 0
    }

    fn chroma_sample_position(&self) -> u8 {
        self.data[self.header_size + 2] & 0x03
    }

    fn reserved(&self) -> u8 {
        self.data[self.header_size + 3]
    }

    fn config_obus(&self) -> &[u8] {
        let start = self.header_size + 4;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl std::fmt::Debug for AV1CodecConfigurationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AV1CodecConfigurationBoxView")
            .field("seq_profile", &self.seq_profile())
            .field("seq_level_idx_0", &self.seq_level_idx_0())
            .finish()
    }
}

/// An owned representation of AV1CodecConfigurationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AV1CodecConfigurationBoxOwned {
    /// Marker and version (should be 0x81).
    pub marker_version: u8,
    /// Sequence profile (0-2).
    pub seq_profile: u8,
    /// Sequence level index.
    pub seq_level_idx_0: u8,
    /// High bit depth flag.
    pub high_bitdepth: bool,
    /// Twelve bit flag.
    pub twelve_bit: bool,
    /// Monochrome flag.
    pub monochrome: bool,
    /// Chroma subsampling X flag.
    pub chroma_subsampling_x: bool,
    /// Chroma subsampling Y flag.
    pub chroma_subsampling_y: bool,
    /// Chroma sample position.
    pub chroma_sample_position: u8,
    /// Reserved byte.
    pub reserved: u8,
    /// Configuration OBUs.
    pub config_obus: Vec<u8>,
}

impl AV1CodecConfigurationBoxOwned {
    /// Creates a new AV1CodecConfigurationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + self.config_obus.len()) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u8(self.marker_version)?;

        let byte1 = (self.seq_profile << 5) | (self.seq_level_idx_0 & 0x1F);
        writer.write_u8(byte1)?;

        let byte2 = ((self.high_bitdepth as u8) << 6)
            | ((self.twelve_bit as u8) << 5)
            | ((self.monochrome as u8) << 4)
            | ((self.chroma_subsampling_x as u8) << 3)
            | ((self.chroma_subsampling_y as u8) << 2)
            | (self.chroma_sample_position & 0x03);
        writer.write_u8(byte2)?;

        writer.write_u8(self.reserved)?;
        writer.write_all(&self.config_obus)?;

        Ok(())
    }
}

impl Default for AV1CodecConfigurationBoxOwned {
    fn default() -> Self {
        Self {
            marker_version: 0x81, // marker=1, version=1
            seq_profile: 0,
            seq_level_idx_0: 0,
            high_bitdepth: false,
            twelve_bit: false,
            monochrome: false,
            chroma_subsampling_x: true,
            chroma_subsampling_y: true,
            chroma_sample_position: 0,
            reserved: 0,
            config_obus: Vec::new(),
        }
    }
}

impl AV1CodecConfigurationBox for AV1CodecConfigurationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn marker_version(&self) -> u8 {
        self.marker_version
    }

    fn seq_profile(&self) -> u8 {
        self.seq_profile
    }

    fn seq_level_idx_0(&self) -> u8 {
        self.seq_level_idx_0
    }

    fn high_bitdepth(&self) -> bool {
        self.high_bitdepth
    }

    fn twelve_bit(&self) -> bool {
        self.twelve_bit
    }

    fn monochrome(&self) -> bool {
        self.monochrome
    }

    fn chroma_subsampling_x(&self) -> bool {
        self.chroma_subsampling_x
    }

    fn chroma_subsampling_y(&self) -> bool {
        self.chroma_subsampling_y
    }

    fn chroma_sample_position(&self) -> u8 {
        self.chroma_sample_position
    }

    fn reserved(&self) -> u8 {
        self.reserved
    }

    fn config_obus(&self) -> &[u8] {
        &self.config_obus
    }
}

impl<T: AV1CodecConfigurationBox> From<&T> for AV1CodecConfigurationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            marker_version: source.marker_version(),
            seq_profile: source.seq_profile(),
            seq_level_idx_0: source.seq_level_idx_0(),
            high_bitdepth: source.high_bitdepth(),
            twelve_bit: source.twelve_bit(),
            monochrome: source.monochrome(),
            chroma_subsampling_x: source.chroma_subsampling_x(),
            chroma_subsampling_y: source.chroma_subsampling_y(),
            chroma_sample_position: source.chroma_sample_position(),
            reserved: source.reserved(),
            config_obus: source.config_obus().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_av1c() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"av1C");
        data.push(0x81); // marker=1, version=1
        data.push(0x00); // seq_profile=0, seq_level_idx_0=0
        data.push(0x0C); // subsampling_x=1, subsampling_y=1
        data.push(0x00); // reserved
        data
    }

    #[test]
    fn parse_av1c() {
        let data = make_av1c();
        let view = AV1CodecConfigurationBoxView::new(&data).unwrap();

        assert_eq!(view.marker_version(), 0x81);
        assert_eq!(view.seq_profile(), 0);
        assert!(view.chroma_subsampling_x());
        assert!(view.chroma_subsampling_y());
    }

    #[test]
    fn roundtrip() {
        let data = make_av1c();
        let view = AV1CodecConfigurationBoxView::new(&data).unwrap();
        let owned = AV1CodecConfigurationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
