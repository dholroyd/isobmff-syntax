//! Pixel Aspect Ratio Box (pasp) parsing and serialization.
//!
//! The Pixel Aspect Ratio Box specifies the aspect ratio of pixels.
//!
//! ```text
//! class PixelAspectRatioBox extends Box('pasp'){
//!    unsigned int(32) hSpacing;
//!    unsigned int(32) vSpacing;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for PixelAspectRatioBox.
pub const BOX_TYPE: BoxCode = BoxCode::PASP;

/// Common interface for accessing PixelAspectRatioBox data.
pub trait PixelAspectRatioBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the horizontal spacing.
    fn h_spacing(&self) -> u32;

    /// Returns the vertical spacing.
    fn v_spacing(&self) -> u32;
}

/// A borrowing view over raw PixelAspectRatioBox bytes.
#[derive(Clone, Copy)]
pub struct PixelAspectRatioBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> PixelAspectRatioBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 8)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> PixelAspectRatioBox for PixelAspectRatioBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn h_spacing(&self) -> u32 {
        let o = self.header_size;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn v_spacing(&self) -> u32 {
        let o = self.header_size + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for PixelAspectRatioBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixelAspectRatioBoxView")
            .field("h_spacing", &self.h_spacing())
            .field("v_spacing", &self.v_spacing())
            .finish()
    }
}

/// An owned representation of PixelAspectRatioBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelAspectRatioBoxOwned {
    /// Horizontal spacing.
    pub h_spacing: u32,
    /// Vertical spacing.
    pub v_spacing: u32,
}

impl PixelAspectRatioBoxOwned {
    /// Creates a new PixelAspectRatioBoxOwned.
    pub fn new(h_spacing: u32, v_spacing: u32) -> Self {
        Self { h_spacing, v_spacing }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(8) + 8 // 8 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u32::<BigEndian>(self.h_spacing)?;
        writer.write_u32::<BigEndian>(self.v_spacing)?;

        Ok(())
    }
}

impl Default for PixelAspectRatioBoxOwned {
    fn default() -> Self {
        Self::new(1, 1) // Square pixels
    }
}

impl PixelAspectRatioBox for PixelAspectRatioBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn h_spacing(&self) -> u32 {
        self.h_spacing
    }

    fn v_spacing(&self) -> u32 {
        self.v_spacing
    }
}

impl<T: PixelAspectRatioBox> From<&T> for PixelAspectRatioBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            h_spacing: source.h_spacing(),
            v_spacing: source.v_spacing(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pasp() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"pasp");
        data.extend_from_slice(&1u32.to_be_bytes()); // h_spacing
        data.extend_from_slice(&1u32.to_be_bytes()); // v_spacing
        data
    }

    #[test]
    fn parse_pasp() {
        let data = make_pasp();
        let view = PixelAspectRatioBoxView::new(&data).unwrap();

        assert_eq!(view.h_spacing(), 1);
        assert_eq!(view.v_spacing(), 1);
    }

    #[test]
    fn roundtrip() {
        let data = make_pasp();
        let view = PixelAspectRatioBoxView::new(&data).unwrap();
        let owned = PixelAspectRatioBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
