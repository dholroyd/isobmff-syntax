//! Color Information Box (colr) parsing and serialization.
//!
//! The Color Information Box specifies the color space of the image.
//!
//! ```text
//! class ColourInformationBox extends Box('colr'){
//!    unsigned int(32) colour_type;
//!    if (colour_type == 'nclx') // on-screen colours
//!    {
//!       unsigned int(16) colour_primaries;
//!       unsigned int(16) transfer_characteristics;
//!       unsigned int(16) matrix_coefficients;
//!       unsigned int(1) full_range_flag;
//!       unsigned int(7) reserved = 0;
//!    }
//!    else if (colour_type == 'rICC')
//!    {
//!       ICC_profile; // restricted ICC profile
//!    }
//!    else if (colour_type == 'prof')
//!    {
//!       ICC_profile; // unrestricted ICC profile
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for ColorInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::COLR;

/// Color type codes.
pub mod color_type {
    use mp4ra_rust::FourCC;

    /// ICC profile color type.
    pub const RICC: FourCC = FourCC(*b"rICC");
    /// Restricted ICC profile color type.
    pub const PROF: FourCC = FourCC(*b"prof");
    /// On-screen video signal color type (nclx).
    pub const NCLX: FourCC = FourCC(*b"nclx");
}

/// Borrowed color information data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorInfo<'a> {
    /// On-screen video signal color type (nclx).
    Nclx {
        /// Colour primaries index.
        color_primaries: u16,
        /// Transfer characteristics index.
        transfer_characteristics: u16,
        /// Matrix coefficients index.
        matrix_coefficients: u16,
        /// Full range flag.
        full_range_flag: bool,
    },
    /// ICC profile (rICC or prof).
    IccProfile {
        /// Whether this is a restricted ICC profile (rICC) or unrestricted (prof).
        restricted: bool,
        /// The ICC profile data.
        profile: &'a [u8],
    },
    /// Unknown color type with raw data.
    Other {
        /// The color type FourCC.
        color_type: FourCC,
        /// The raw data bytes.
        data: &'a [u8],
    },
}

/// Owned color information data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorData {
    /// On-screen video signal color type (nclx).
    Nclx {
        /// Colour primaries index.
        color_primaries: u16,
        /// Transfer characteristics index.
        transfer_characteristics: u16,
        /// Matrix coefficients index.
        matrix_coefficients: u16,
        /// Full range flag.
        full_range_flag: bool,
    },
    /// ICC profile (rICC or prof).
    IccProfile {
        /// Whether this is a restricted ICC profile (rICC) or unrestricted (prof).
        restricted: bool,
        /// The ICC profile data.
        profile: Vec<u8>,
    },
    /// Unknown color type with raw data.
    Other {
        /// The color type FourCC.
        color_type: FourCC,
        /// The raw data bytes.
        data: Vec<u8>,
    },
}

impl ColorData {
    /// Returns the color type FourCC.
    pub fn color_type(&self) -> FourCC {
        match self {
            Self::Nclx { .. } => color_type::NCLX,
            Self::IccProfile { restricted: true, .. } => color_type::RICC,
            Self::IccProfile { restricted: false, .. } => color_type::PROF,
            Self::Other { color_type, .. } => *color_type,
        }
    }
}

impl<'a> From<ColorInfo<'a>> for ColorData {
    fn from(info: ColorInfo<'a>) -> Self {
        match info {
            ColorInfo::Nclx { color_primaries, transfer_characteristics, matrix_coefficients, full_range_flag } => {
                Self::Nclx { color_primaries, transfer_characteristics, matrix_coefficients, full_range_flag }
            }
            ColorInfo::IccProfile { restricted, profile } => {
                Self::IccProfile { restricted, profile: profile.to_vec() }
            }
            ColorInfo::Other { color_type, data } => {
                Self::Other { color_type, data: data.to_vec() }
            }
        }
    }
}

/// Common interface for accessing ColorInformationBox data.
pub trait ColorInformationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the color type.
    fn color_type(&self) -> FourCC;

    /// Returns the structured color information.
    fn color_info(&self) -> ColorInfo<'_>;
}

/// A borrowing view over raw ColorInformationBox bytes.
#[derive(Clone, Copy)]
pub struct ColorInformationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> ColorInformationBoxView<'a> {
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

    /// Returns true if this is an nclx color type.
    pub fn is_nclx(&self) -> bool {
        self.color_type() == color_type::NCLX
    }

    /// Returns true if this is an ICC profile color type.
    pub fn is_icc(&self) -> bool {
        let ct = self.color_type();
        ct == color_type::RICC || ct == color_type::PROF
    }

    /// Returns the ICC profile data (if this is an ICC type).
    pub fn icc_profile(&self) -> Option<&'a [u8]> {
        if self.is_icc() {
            Some(&self.data[self.header_size + 4..])
        } else {
            None
        }
    }

    /// Returns nclx color primaries (if this is nclx type).
    pub fn color_primaries(&self) -> Option<u16> {
        if self.is_nclx() && self.data.len() >= self.header_size + 6 {
            Some(BigEndian::read_u16(&self.data[self.header_size + 4..]))
        } else {
            None
        }
    }

    /// Returns nclx transfer characteristics (if this is nclx type).
    pub fn transfer_characteristics(&self) -> Option<u16> {
        if self.is_nclx() && self.data.len() >= self.header_size + 8 {
            Some(BigEndian::read_u16(&self.data[self.header_size + 6..]))
        } else {
            None
        }
    }

    /// Returns nclx matrix coefficients (if this is nclx type).
    pub fn matrix_coefficients(&self) -> Option<u16> {
        if self.is_nclx() && self.data.len() >= self.header_size + 10 {
            Some(BigEndian::read_u16(&self.data[self.header_size + 8..]))
        } else {
            None
        }
    }

    /// Returns nclx full range flag (if this is nclx type).
    pub fn full_range_flag(&self) -> Option<bool> {
        if self.is_nclx() && self.data.len() >= self.header_size + 11 {
            Some((self.data[self.header_size + 10] >> 7) != 0)
        } else {
            None
        }
    }
}

impl<'a> ColorInformationBox for ColorInformationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn color_type(&self) -> FourCC {
        let o = self.header_size;
        FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn color_info(&self) -> ColorInfo<'_> {
        let ct = self.color_type();
        let payload = &self.data[self.header_size + 4..];
        if ct == color_type::NCLX && payload.len() >= 7 {
            ColorInfo::Nclx {
                color_primaries: BigEndian::read_u16(&payload[0..2]),
                transfer_characteristics: BigEndian::read_u16(&payload[2..4]),
                matrix_coefficients: BigEndian::read_u16(&payload[4..6]),
                full_range_flag: (payload[6] >> 7) != 0,
            }
        } else if ct == color_type::RICC {
            ColorInfo::IccProfile { restricted: true, profile: payload }
        } else if ct == color_type::PROF {
            ColorInfo::IccProfile { restricted: false, profile: payload }
        } else {
            ColorInfo::Other { color_type: ct, data: payload }
        }
    }
}

impl std::fmt::Debug for ColorInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorInformationBoxView")
            .field("color_type", &self.color_type())
            .finish()
    }
}

/// An owned representation of ColorInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorInformationBoxOwned {
    /// Structured color data.
    pub color_data: ColorData,
}

impl ColorInformationBoxOwned {
    /// Creates a new ColorInformationBoxOwned with nclx color type.
    pub fn new_nclx(
        color_primaries: u16,
        transfer_characteristics: u16,
        matrix_coefficients: u16,
        full_range: bool,
    ) -> Self {
        Self {
            color_data: ColorData::Nclx {
                color_primaries,
                transfer_characteristics,
                matrix_coefficients,
                full_range_flag: full_range,
            },
        }
    }

    /// Creates a new ColorInformationBoxOwned with ICC profile.
    pub fn new_icc(profile: Vec<u8>) -> Self {
        Self {
            color_data: ColorData::IccProfile {
                restricted: false,
                profile,
            },
        }
    }

    /// Returns the serialized payload size (after the box header).
    fn payload_size(&self) -> usize {
        4 + match &self.color_data {
            ColorData::Nclx { .. } => 7,
            ColorData::IccProfile { profile, .. } => profile.len(),
            ColorData::Other { data, .. } => data.len(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = self.payload_size() as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_all(&self.color_data.color_type().0)?;
        match &self.color_data {
            ColorData::Nclx { color_primaries, transfer_characteristics, matrix_coefficients, full_range_flag } => {
                writer.write_u16::<BigEndian>(*color_primaries)?;
                writer.write_u16::<BigEndian>(*transfer_characteristics)?;
                writer.write_u16::<BigEndian>(*matrix_coefficients)?;
                writer.write_u8(if *full_range_flag { 0x80 } else { 0x00 })?;
            }
            ColorData::IccProfile { profile, .. } => {
                writer.write_all(profile)?;
            }
            ColorData::Other { data, .. } => {
                writer.write_all(data)?;
            }
        }
        Ok(())
    }
}

impl Default for ColorInformationBoxOwned {
    fn default() -> Self {
        // Default to sRGB-like nclx
        Self::new_nclx(1, 13, 6, true)
    }
}

impl ColorInformationBox for ColorInformationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn color_type(&self) -> FourCC {
        self.color_data.color_type()
    }

    fn color_info(&self) -> ColorInfo<'_> {
        match &self.color_data {
            ColorData::Nclx { color_primaries, transfer_characteristics, matrix_coefficients, full_range_flag } => {
                ColorInfo::Nclx {
                    color_primaries: *color_primaries,
                    transfer_characteristics: *transfer_characteristics,
                    matrix_coefficients: *matrix_coefficients,
                    full_range_flag: *full_range_flag,
                }
            }
            ColorData::IccProfile { restricted, profile } => {
                ColorInfo::IccProfile { restricted: *restricted, profile }
            }
            ColorData::Other { color_type, data } => {
                ColorInfo::Other { color_type: *color_type, data }
            }
        }
    }
}

impl<T: ColorInformationBox> From<&T> for ColorInformationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            color_data: source.color_info().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_colr_nclx() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 7 = 19 bytes
        data.extend_from_slice(&19u32.to_be_bytes());
        data.extend_from_slice(b"colr");
        data.extend_from_slice(b"nclx");
        data.extend_from_slice(&1u16.to_be_bytes()); // color_primaries
        data.extend_from_slice(&13u16.to_be_bytes()); // transfer_characteristics
        data.extend_from_slice(&6u16.to_be_bytes()); // matrix_coefficients
        data.push(0x80); // full_range = true
        data
    }

    #[test]
    fn parse_colr_nclx() {
        let data = make_colr_nclx();
        let view = ColorInformationBoxView::new(&data).unwrap();

        assert!(view.is_nclx());
        assert_eq!(view.color_primaries(), Some(1));
        assert_eq!(view.transfer_characteristics(), Some(13));
        assert_eq!(view.matrix_coefficients(), Some(6));
        assert_eq!(view.full_range_flag(), Some(true));

        // Also test via trait
        let info = view.color_info();
        assert!(matches!(info, ColorInfo::Nclx {
            color_primaries: 1,
            transfer_characteristics: 13,
            matrix_coefficients: 6,
            full_range_flag: true,
        }));
    }

    #[test]
    fn parse_colr_icc() {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 4 + 4
        data.extend_from_slice(b"colr");
        data.extend_from_slice(b"rICC");
        data.extend_from_slice(&[1, 2, 3, 4]); // fake ICC profile

        let view = ColorInformationBoxView::new(&data).unwrap();
        assert!(view.is_icc());
        assert_eq!(view.icc_profile(), Some([1, 2, 3, 4].as_slice()));

        let info = view.color_info();
        assert!(matches!(info, ColorInfo::IccProfile { restricted: true, profile } if profile == [1, 2, 3, 4]));
    }

    #[test]
    fn roundtrip_nclx() {
        let data = make_colr_nclx();
        let view = ColorInformationBoxView::new(&data).unwrap();
        let owned = ColorInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_icc() {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"colr");
        data.extend_from_slice(b"rICC");
        data.extend_from_slice(&[1, 2, 3, 4]);

        let view = ColorInformationBoxView::new(&data).unwrap();
        let owned = ColorInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
