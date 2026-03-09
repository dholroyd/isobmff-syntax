//! Image Spatial Extents Box (ispe) parsing and serialization.
//!
//! The Image Spatial Extents Box specifies the width and height of an image item.
//!
//! ```text
//! aligned(8) class ImageSpatialExtentsProperty
//!    extends ItemFullProperty('ispe', version = 0, 0) {
//!    unsigned int(32) image_width;
//!    unsigned int(32) image_height;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ImageSpatialExtentsBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"ispe");

/// Common interface for accessing ImageSpatialExtentsBox data.
pub trait ImageSpatialExtentsBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the image width.
    fn image_width(&self) -> u32;

    /// Returns the image height.
    fn image_height(&self) -> u32;
}

/// A borrowing view over raw ImageSpatialExtentsBox bytes.
#[derive(Clone, Copy)]
pub struct ImageSpatialExtentsBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> ImageSpatialExtentsBoxView<'a> {
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

impl<'a> ImageSpatialExtentsBox for ImageSpatialExtentsBoxView<'a> {
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

    fn image_width(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn image_height(&self) -> u32 {
        let o = self.fullbox_offset + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for ImageSpatialExtentsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageSpatialExtentsBoxView")
            .field("image_width", &self.image_width())
            .field("image_height", &self.image_height())
            .finish()
    }
}

/// An owned representation of ImageSpatialExtentsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageSpatialExtentsBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Image width.
    pub image_width: u32,
    /// Image height.
    pub image_height: u32,
}

impl ImageSpatialExtentsBoxOwned {
    /// Creates a new ImageSpatialExtentsBoxOwned.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            flags: 0,
            image_width: width,
            image_height: height,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(8) + 8 // 8 + 4 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.image_width)?;
        writer.write_u32::<BigEndian>(self.image_height)?;

        Ok(())
    }
}

impl Default for ImageSpatialExtentsBoxOwned {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl ImageSpatialExtentsBox for ImageSpatialExtentsBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        0
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn image_width(&self) -> u32 {
        self.image_width
    }

    fn image_height(&self) -> u32 {
        self.image_height
    }
}

impl<T: ImageSpatialExtentsBox> From<&T> for ImageSpatialExtentsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            image_width: source.image_width(),
            image_height: source.image_height(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ispe() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"ispe");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1920u32.to_be_bytes()); // width
        data.extend_from_slice(&1080u32.to_be_bytes()); // height
        data
    }

    #[test]
    fn parse_ispe() {
        let data = make_ispe();
        let view = ImageSpatialExtentsBoxView::new(&data).unwrap();

        assert_eq!(view.image_width(), 1920);
        assert_eq!(view.image_height(), 1080);
    }

    #[test]
    fn roundtrip() {
        let data = make_ispe();
        let view = ImageSpatialExtentsBoxView::new(&data).unwrap();
        let owned = ImageSpatialExtentsBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
