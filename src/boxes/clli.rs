//! Content Light Level Information Box (clli) parsing and serialization.
//!
//! The Content Light Level Information Box specifies the light level metadata.
//!
//! ```text
//! class ContentLightLevelBox extends Box('clli'){
//!    unsigned int(16) max_content_light_level;
//!    unsigned int(16) max_pic_average_light_level;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ContentLightLevelBox.
pub const BOX_TYPE: BoxCode = BoxCode::CLLI;

/// Common interface for accessing ContentLightLevelBox data.
pub trait ContentLightLevelBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the maximum content light level.
    fn max_content_light_level(&self) -> u16;

    /// Returns the maximum picture average light level.
    fn max_pic_average_light_level(&self) -> u16;
}

/// A borrowing view over raw ContentLightLevelBox bytes.
#[derive(Clone, Copy)]
pub struct ContentLightLevelBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> ContentLightLevelBoxView<'a> {
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

impl<'a> ContentLightLevelBox for ContentLightLevelBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn max_content_light_level(&self) -> u16 {
        let o = self.header_size;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn max_pic_average_light_level(&self) -> u16 {
        let o = self.header_size + 2;
        BigEndian::read_u16(&self.data[o..o + 2])
    }
}

impl std::fmt::Debug for ContentLightLevelBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentLightLevelBoxView")
            .field("max_content_light_level", &self.max_content_light_level())
            .field("max_pic_average_light_level", &self.max_pic_average_light_level())
            .finish()
    }
}

/// An owned representation of ContentLightLevelBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentLightLevelBoxOwned {
    /// Maximum content light level.
    pub max_content_light_level: u16,
    /// Maximum picture average light level.
    pub max_pic_average_light_level: u16,
}

impl ContentLightLevelBoxOwned {
    /// Creates a new ContentLightLevelBoxOwned.
    pub fn new(max_cll: u16, max_pall: u16) -> Self {
        Self {
            max_content_light_level: max_cll,
            max_pic_average_light_level: max_pall,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 2 + 2
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u16::<BigEndian>(self.max_content_light_level)?;
        writer.write_u16::<BigEndian>(self.max_pic_average_light_level)?;

        Ok(())
    }
}

impl Default for ContentLightLevelBoxOwned {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl ContentLightLevelBox for ContentLightLevelBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn max_content_light_level(&self) -> u16 {
        self.max_content_light_level
    }

    fn max_pic_average_light_level(&self) -> u16 {
        self.max_pic_average_light_level
    }
}

impl<T: ContentLightLevelBox> From<&T> for ContentLightLevelBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            max_content_light_level: source.max_content_light_level(),
            max_pic_average_light_level: source.max_pic_average_light_level(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clli() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"clli");
        data.extend_from_slice(&1000u16.to_be_bytes()); // max_cll
        data.extend_from_slice(&400u16.to_be_bytes()); // max_pall
        data
    }

    #[test]
    fn parse_clli() {
        let data = make_clli();
        let view = ContentLightLevelBoxView::new(&data).unwrap();

        assert_eq!(view.max_content_light_level(), 1000);
        assert_eq!(view.max_pic_average_light_level(), 400);
    }

    #[test]
    fn roundtrip() {
        let data = make_clli();
        let view = ContentLightLevelBoxView::new(&data).unwrap();
        let owned = ContentLightLevelBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
