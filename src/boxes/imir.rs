//! Image Mirror Box (imir) parsing and serialization.
//!
//! The Image Mirror Box specifies the mirroring axis.
//!
//! ```text
//! aligned(8) class ImageMirror
//!    extends ItemProperty('imir') {
//!    unsigned int(7) reserved = 0;
//!    unsigned int(1) axis;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::WriteBytesExt;
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ImageMirrorBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"imir");

/// Common interface for accessing ImageMirrorBox data.
pub trait ImageMirrorBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the mirror axis code.
    /// 0 = vertical axis (left-right flip), 1 = horizontal axis (top-bottom flip)
    fn axis(&self) -> u8;

    /// Returns true if mirroring is about the vertical axis (left-right flip).
    fn is_vertical_mirror(&self) -> bool {
        (self.axis() & 0x01) == 0
    }

    /// Returns true if mirroring is about the horizontal axis (top-bottom flip).
    fn is_horizontal_mirror(&self) -> bool {
        (self.axis() & 0x01) == 1
    }
}

/// A borrowing view over raw ImageMirrorBox bytes.
#[derive(Clone, Copy)]
pub struct ImageMirrorBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> ImageMirrorBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 1)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> ImageMirrorBox for ImageMirrorBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn axis(&self) -> u8 {
        self.data[self.header_size] & 0x01
    }
}

impl std::fmt::Debug for ImageMirrorBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageMirrorBoxView")
            .field("axis", &self.axis())
            .field("is_vertical_mirror", &self.is_vertical_mirror())
            .finish()
    }
}

/// An owned representation of ImageMirrorBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageMirrorBoxOwned {
    /// Mirror axis code (0 = vertical, 1 = horizontal).
    pub axis: u8,
}

impl ImageMirrorBoxOwned {
    /// Creates a new ImageMirrorBoxOwned.
    pub fn new(axis: u8) -> Self {
        Self { axis: axis & 0x01 }
    }

    /// Creates a new ImageMirrorBoxOwned for vertical axis mirroring.
    pub fn vertical() -> Self {
        Self::new(0)
    }

    /// Creates a new ImageMirrorBoxOwned for horizontal axis mirroring.
    pub fn horizontal() -> Self {
        Self::new(1)
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(1) + 1 // 8 + 1
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u8(self.axis & 0x01)?;

        Ok(())
    }
}

impl Default for ImageMirrorBoxOwned {
    fn default() -> Self {
        Self::vertical()
    }
}

impl ImageMirrorBox for ImageMirrorBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn axis(&self) -> u8 {
        self.axis & 0x01
    }
}

impl<T: ImageMirrorBox> From<&T> for ImageMirrorBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            axis: source.axis(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_imir() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&9u32.to_be_bytes());
        data.extend_from_slice(b"imir");
        data.push(0); // vertical axis
        data
    }

    #[test]
    fn parse_imir() {
        let data = make_imir();
        let view = ImageMirrorBoxView::new(&data).unwrap();

        assert_eq!(view.axis(), 0);
        assert!(view.is_vertical_mirror());
        assert!(!view.is_horizontal_mirror());
    }

    #[test]
    fn roundtrip() {
        let data = make_imir();
        let view = ImageMirrorBoxView::new(&data).unwrap();
        let owned = ImageMirrorBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
