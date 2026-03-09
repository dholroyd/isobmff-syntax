//! VP Codec Configuration Box (vpcC) parsing and serialization.
//!
//! The VP Codec Configuration Box specifies the VP8/VP9 codec parameters.
//!
//! ```text
//! aligned(8) class VPCodecConfigurationBox
//!    extends FullBox('vpcC', version = 1, 0) {
//!    unsigned int(8) Profile;
//!    unsigned int(8) Level;
//!    unsigned int(4) bitDepth;
//!    unsigned int(3) chromaSubsampling;
//!    unsigned int(1) videoFullRangeFlag;
//!    unsigned int(8) colourPrimaries;
//!    unsigned int(8) transferCharacteristics;
//!    unsigned int(8) matrixCoefficients;
//!    unsigned int(16) codecIntializationDataSize;
//!    unsigned int(8)[] codecIntializationData;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for VPCodecConfigurationBox.
pub const BOX_TYPE: BoxCode = BoxCode::VPCC;

/// Common interface for accessing VPCodecConfigurationBox data.
pub trait VPCodecConfigurationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the VP profile.
    fn profile(&self) -> u8;

    /// Returns the VP level.
    fn level(&self) -> u8;

    /// Returns the bit depth.
    fn bit_depth(&self) -> u8;

    /// Returns the chroma subsampling.
    fn chroma_subsampling(&self) -> u8;

    /// Returns the color primaries.
    fn colour_primaries(&self) -> u8;

    /// Returns the transfer characteristics.
    fn transfer_characteristics(&self) -> u8;

    /// Returns the matrix coefficients.
    fn matrix_coefficients(&self) -> u8;

    /// Returns the video full range flag.
    fn video_full_range_flag(&self) -> bool;

    /// Returns the codec initialization data size.
    fn codec_initialization_data_size(&self) -> u16;

    /// Returns the codec initialization data.
    fn codec_initialization_data(&self) -> &[u8];
}

/// A borrowing view over raw VPCodecConfigurationBox bytes.
#[derive(Clone, Copy)]
pub struct VPCodecConfigurationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> VPCodecConfigurationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> VPCodecConfigurationBox for VPCodecConfigurationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.data[self.fullbox_offset]
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn profile(&self) -> u8 {
        self.data[self.fullbox_offset + 4]
    }

    fn level(&self) -> u8 {
        self.data[self.fullbox_offset + 5]
    }

    fn bit_depth(&self) -> u8 {
        (self.data[self.fullbox_offset + 6] >> 4) & 0x0F
    }

    fn chroma_subsampling(&self) -> u8 {
        (self.data[self.fullbox_offset + 6] >> 1) & 0x07
    }

    fn video_full_range_flag(&self) -> bool {
        (self.data[self.fullbox_offset + 6] & 0x01) != 0
    }

    fn colour_primaries(&self) -> u8 {
        self.data[self.fullbox_offset + 7]
    }

    fn transfer_characteristics(&self) -> u8 {
        self.data[self.fullbox_offset + 8]
    }

    fn matrix_coefficients(&self) -> u8 {
        self.data[self.fullbox_offset + 9]
    }

    fn codec_initialization_data_size(&self) -> u16 {
        BigEndian::read_u16(&self.data[self.fullbox_offset + 10..self.fullbox_offset + 12])
    }

    fn codec_initialization_data(&self) -> &[u8] {
        let start = self.fullbox_offset + 12;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl std::fmt::Debug for VPCodecConfigurationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VPCodecConfigurationBoxView")
            .field("profile", &self.profile())
            .field("level", &self.level())
            .field("bit_depth", &self.bit_depth())
            .finish()
    }
}

/// An owned representation of VPCodecConfigurationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VPCodecConfigurationBoxOwned {
    /// Flags.
    pub flags: u32,
    /// VP profile.
    pub profile: u8,
    /// VP level.
    pub level: u8,
    /// Bit depth.
    pub bit_depth: u8,
    /// Chroma subsampling.
    pub chroma_subsampling: u8,
    /// Video full range flag.
    pub video_full_range_flag: bool,
    /// Color primaries.
    pub colour_primaries: u8,
    /// Transfer characteristics.
    pub transfer_characteristics: u8,
    /// Matrix coefficients.
    pub matrix_coefficients: u8,
    /// Codec initialization data.
    pub codec_initialization_data: Vec<u8>,
}

impl VPCodecConfigurationBoxOwned {
    /// Creates a new VPCodecConfigurationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (8 + self.codec_initialization_data.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 1, self.flags)?;

        writer.write_u8(self.profile)?;
        writer.write_u8(self.level)?;

        let byte6 = ((self.bit_depth & 0x0F) << 4)
            | ((self.chroma_subsampling & 0x07) << 1)
            | (self.video_full_range_flag as u8);
        writer.write_u8(byte6)?;

        writer.write_u8(self.colour_primaries)?;
        writer.write_u8(self.transfer_characteristics)?;
        writer.write_u8(self.matrix_coefficients)?;
        writer.write_u16::<BigEndian>(self.codec_initialization_data.len() as u16)?;
        writer.write_all(&self.codec_initialization_data)?;

        Ok(())
    }
}

impl Default for VPCodecConfigurationBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            profile: 0,
            level: 0,
            bit_depth: 8,
            chroma_subsampling: 0,
            video_full_range_flag: false,
            colour_primaries: 1,
            transfer_characteristics: 1,
            matrix_coefficients: 1,
            codec_initialization_data: Vec::new(),
        }
    }
}

impl VPCodecConfigurationBox for VPCodecConfigurationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        1
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn profile(&self) -> u8 {
        self.profile
    }

    fn level(&self) -> u8 {
        self.level
    }

    fn bit_depth(&self) -> u8 {
        self.bit_depth
    }

    fn chroma_subsampling(&self) -> u8 {
        self.chroma_subsampling
    }

    fn video_full_range_flag(&self) -> bool {
        self.video_full_range_flag
    }

    fn colour_primaries(&self) -> u8 {
        self.colour_primaries
    }

    fn transfer_characteristics(&self) -> u8 {
        self.transfer_characteristics
    }

    fn matrix_coefficients(&self) -> u8 {
        self.matrix_coefficients
    }

    fn codec_initialization_data_size(&self) -> u16 {
        self.codec_initialization_data.len() as u16
    }

    fn codec_initialization_data(&self) -> &[u8] {
        &self.codec_initialization_data
    }
}

impl<T: VPCodecConfigurationBox> From<&T> for VPCodecConfigurationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            profile: source.profile(),
            level: source.level(),
            bit_depth: source.bit_depth(),
            chroma_subsampling: source.chroma_subsampling(),
            video_full_range_flag: source.video_full_range_flag(),
            colour_primaries: source.colour_primaries(),
            transfer_characteristics: source.transfer_characteristics(),
            matrix_coefficients: source.matrix_coefficients(),
            codec_initialization_data: source.codec_initialization_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vpcc() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // 8 + 4 + 8
        data.extend_from_slice(b"vpcC");
        data.push(1); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(0); // profile
        data.push(0); // level
        data.push(0x80); // bit_depth=8, chroma_subsampling=0, video_full_range=0
        data.push(1); // colour_primaries
        data.push(1); // transfer_characteristics
        data.push(1); // matrix_coefficients
        data.extend_from_slice(&0u16.to_be_bytes()); // codec_initialization_data_size
        data
    }

    #[test]
    fn parse_vpcc() {
        let data = make_vpcc();
        let view = VPCodecConfigurationBoxView::new(&data).unwrap();

        assert_eq!(view.profile(), 0);
        assert_eq!(view.level(), 0);
        assert_eq!(view.bit_depth(), 8);
    }

    #[test]
    fn roundtrip() {
        let data = make_vpcc();
        let view = VPCodecConfigurationBoxView::new(&data).unwrap();
        let owned = VPCodecConfigurationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
