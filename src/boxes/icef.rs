//! Image Extent Frame Box (icef) parsing and serialization.
//!
//! The Image Extent Frame Box specifies frame extents for coded images.
//!
//! ```text
//! aligned(8) class ImageExtentFrameProperty
//!    extends ItemFullProperty('icef', version = 0, 0) {
//!    unsigned int(2) extent_width_size;
//!    unsigned int(2) extent_height_size;
//!    unsigned int(2) origin_x_size;
//!    unsigned int(2) origin_y_size;
//!    unsigned int(32) num_sets;
//!    for (i=0; i<num_sets; i++) {
//!       unsigned int((extent_width_size+1)*8) extent_width;
//!       unsigned int((extent_height_size+1)*8) extent_height;
//!       unsigned int((origin_x_size+1)*8) origin_x;
//!       unsigned int((origin_y_size+1)*8) origin_y;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ImageExtentFrameBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"icef");

/// Common interface for accessing ImageExtentFrameBox data.
pub trait ImageExtentFrameBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the extent data.
    fn extent_data(&self) -> &[u8];
}

/// A borrowing view over raw ImageExtentFrameBox bytes.
#[derive(Clone, Copy)]
pub struct ImageExtentFrameBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> ImageExtentFrameBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the extent data.
    pub fn extent_data(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl<'a> ImageExtentFrameBox for ImageExtentFrameBoxView<'a> {
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

    fn extent_data(&self) -> &[u8] {
        self.extent_data()
    }
}

impl std::fmt::Debug for ImageExtentFrameBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageExtentFrameBoxView")
            .field("data_len", &self.extent_data().len())
            .finish()
    }
}

/// An owned representation of ImageExtentFrameBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ImageExtentFrameBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Extent data.
    pub extent_data: Vec<u8>,
}

impl ImageExtentFrameBoxOwned {
    /// Creates a new ImageExtentFrameBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.extent_data.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.extent_data)?;

        Ok(())
    }
}


impl ImageExtentFrameBox for ImageExtentFrameBoxOwned {
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

    fn extent_data(&self) -> &[u8] {
        &self.extent_data
    }
}

impl<T: ImageExtentFrameBox> From<&T> for ImageExtentFrameBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            extent_data: source.extent_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_icef() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"icef");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_icef() {
        let data = make_icef();
        let view = ImageExtentFrameBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_icef();
        let view = ImageExtentFrameBoxView::new(&data).unwrap();
        let owned = ImageExtentFrameBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
