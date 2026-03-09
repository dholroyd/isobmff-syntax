//! Mastering Display Color Volume Box (mdcv) parsing and serialization.
//!
//! The Mastering Display Color Volume Box specifies the mastering display metadata.
//!
//! ```text
//! class MasteringDisplayColourVolumeBox extends Box('mdcv'){
//!    for (c = 0; c<3; c++) {
//!       unsigned int(16) display_primaries_x;
//!       unsigned int(16) display_primaries_y;
//!    }
//!    unsigned int(16) white_point_x;
//!    unsigned int(16) white_point_y;
//!    unsigned int(32) max_display_mastering_luminance;
//!    unsigned int(32) min_display_mastering_luminance;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MasteringDisplayColourVolumeBox.
pub const BOX_TYPE: BoxCode = BoxCode::MDCV;

/// Common interface for accessing MasteringDisplayColourVolumeBox data.
pub trait MasteringDisplayColourVolumeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the display primaries (array of 6 values: Gx, Gy, Bx, By, Rx, Ry).
    fn display_primaries(&self) -> [u16; 6];

    /// Returns the white point X coordinate.
    fn white_point_x(&self) -> u16;

    /// Returns the white point Y coordinate.
    fn white_point_y(&self) -> u16;

    /// Returns the maximum display mastering luminance.
    fn max_display_mastering_luminance(&self) -> u32;

    /// Returns the minimum display mastering luminance.
    fn min_display_mastering_luminance(&self) -> u32;
}

/// A borrowing view over raw MasteringDisplayColourVolumeBox bytes.
#[derive(Clone, Copy)]
pub struct MasteringDisplayColourVolumeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> MasteringDisplayColourVolumeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 24)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> MasteringDisplayColourVolumeBox for MasteringDisplayColourVolumeBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn display_primaries(&self) -> [u16; 6] {
        let o = self.header_size;
        [
            BigEndian::read_u16(&self.data[o..o + 2]),
            BigEndian::read_u16(&self.data[o + 2..o + 4]),
            BigEndian::read_u16(&self.data[o + 4..o + 6]),
            BigEndian::read_u16(&self.data[o + 6..o + 8]),
            BigEndian::read_u16(&self.data[o + 8..o + 10]),
            BigEndian::read_u16(&self.data[o + 10..o + 12]),
        ]
    }

    fn white_point_x(&self) -> u16 {
        let o = self.header_size + 12;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn white_point_y(&self) -> u16 {
        let o = self.header_size + 14;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn max_display_mastering_luminance(&self) -> u32 {
        let o = self.header_size + 16;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn min_display_mastering_luminance(&self) -> u32 {
        let o = self.header_size + 20;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for MasteringDisplayColourVolumeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MasteringDisplayColourVolumeBoxView")
            .field("display_primaries", &self.display_primaries())
            .field("white_point", &(self.white_point_x(), self.white_point_y()))
            .finish()
    }
}

/// An owned representation of MasteringDisplayColourVolumeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasteringDisplayColourVolumeBoxOwned {
    /// Display primaries (Gx, Gy, Bx, By, Rx, Ry).
    pub display_primaries: [u16; 6],
    /// White point X.
    pub white_point_x: u16,
    /// White point Y.
    pub white_point_y: u16,
    /// Maximum display mastering luminance.
    pub max_display_mastering_luminance: u32,
    /// Minimum display mastering luminance.
    pub min_display_mastering_luminance: u32,
}

impl MasteringDisplayColourVolumeBoxOwned {
    /// Creates a new MasteringDisplayColourVolumeBoxOwned.
    pub fn new(
        display_primaries: [u16; 6],
        white_point_x: u16,
        white_point_y: u16,
        max_luminance: u32,
        min_luminance: u32,
    ) -> Self {
        Self {
            display_primaries,
            white_point_x,
            white_point_y,
            max_display_mastering_luminance: max_luminance,
            min_display_mastering_luminance: min_luminance,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(24) + 24 // 8 + 24
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        for &p in &self.display_primaries {
            writer.write_u16::<BigEndian>(p)?;
        }
        writer.write_u16::<BigEndian>(self.white_point_x)?;
        writer.write_u16::<BigEndian>(self.white_point_y)?;
        writer.write_u32::<BigEndian>(self.max_display_mastering_luminance)?;
        writer.write_u32::<BigEndian>(self.min_display_mastering_luminance)?;

        Ok(())
    }
}

impl Default for MasteringDisplayColourVolumeBoxOwned {
    fn default() -> Self {
        Self::new([0; 6], 0, 0, 0, 0)
    }
}

impl MasteringDisplayColourVolumeBox for MasteringDisplayColourVolumeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn display_primaries(&self) -> [u16; 6] {
        self.display_primaries
    }

    fn white_point_x(&self) -> u16 {
        self.white_point_x
    }

    fn white_point_y(&self) -> u16 {
        self.white_point_y
    }

    fn max_display_mastering_luminance(&self) -> u32 {
        self.max_display_mastering_luminance
    }

    fn min_display_mastering_luminance(&self) -> u32 {
        self.min_display_mastering_luminance
    }
}

impl<T: MasteringDisplayColourVolumeBox> From<&T> for MasteringDisplayColourVolumeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            display_primaries: source.display_primaries(),
            white_point_x: source.white_point_x(),
            white_point_y: source.white_point_y(),
            max_display_mastering_luminance: source.max_display_mastering_luminance(),
            min_display_mastering_luminance: source.min_display_mastering_luminance(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mdcv() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_be_bytes());
        data.extend_from_slice(b"mdcv");
        // Display primaries
        for i in 0..6 {
            data.extend_from_slice(&((i + 1) as u16 * 100).to_be_bytes());
        }
        data.extend_from_slice(&15635u16.to_be_bytes()); // white_point_x
        data.extend_from_slice(&16450u16.to_be_bytes()); // white_point_y
        data.extend_from_slice(&10000000u32.to_be_bytes()); // max_lum
        data.extend_from_slice(&50u32.to_be_bytes()); // min_lum
        data
    }

    #[test]
    fn parse_mdcv() {
        let data = make_mdcv();
        let view = MasteringDisplayColourVolumeBoxView::new(&data).unwrap();

        assert_eq!(view.white_point_x(), 15635);
        assert_eq!(view.white_point_y(), 16450);
        assert_eq!(view.max_display_mastering_luminance(), 10000000);
        assert_eq!(view.min_display_mastering_luminance(), 50);
    }

    #[test]
    fn roundtrip() {
        let data = make_mdcv();
        let view = MasteringDisplayColourVolumeBoxView::new(&data).unwrap();
        let owned = MasteringDisplayColourVolumeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
