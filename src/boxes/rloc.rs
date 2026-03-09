//! Relative Location Box (rloc) parsing and serialization.
//!
//! The Relative Location Box specifies the relative location of an image overlay.
//!
//! ```text
//! aligned(8) class RelativeLocationProperty
//!    extends ItemFullProperty('rloc', version = 0, 0) {
//!    unsigned int(32) horizontal_offset;
//!    unsigned int(32) vertical_offset;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for RelativeLocationBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"rloc");

/// Common interface for accessing RelativeLocationBox data.
pub trait RelativeLocationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the horizontal offset.
    fn horizontal_offset(&self) -> u32;

    /// Returns the vertical offset.
    fn vertical_offset(&self) -> u32;
}

/// A borrowing view over raw RelativeLocationBox bytes.
#[derive(Clone, Copy)]
pub struct RelativeLocationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> RelativeLocationBoxView<'a> {
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

impl<'a> RelativeLocationBox for RelativeLocationBoxView<'a> {
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

    fn horizontal_offset(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn vertical_offset(&self) -> u32 {
        let o = self.fullbox_offset + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for RelativeLocationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelativeLocationBoxView")
            .field("horizontal_offset", &self.horizontal_offset())
            .field("vertical_offset", &self.vertical_offset())
            .finish()
    }
}

/// An owned representation of RelativeLocationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelativeLocationBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Horizontal offset.
    pub horizontal_offset: u32,
    /// Vertical offset.
    pub vertical_offset: u32,
}

impl RelativeLocationBoxOwned {
    /// Creates a new RelativeLocationBoxOwned.
    pub fn new(horizontal_offset: u32, vertical_offset: u32) -> Self {
        Self {
            flags: 0,
            horizontal_offset,
            vertical_offset,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(8) + 8 // 8 + 4 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.horizontal_offset)?;
        writer.write_u32::<BigEndian>(self.vertical_offset)?;

        Ok(())
    }
}

impl Default for RelativeLocationBoxOwned {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl RelativeLocationBox for RelativeLocationBoxOwned {
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

    fn horizontal_offset(&self) -> u32 {
        self.horizontal_offset
    }

    fn vertical_offset(&self) -> u32 {
        self.vertical_offset
    }
}

impl<T: RelativeLocationBox> From<&T> for RelativeLocationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            horizontal_offset: source.horizontal_offset(),
            vertical_offset: source.vertical_offset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rloc() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"rloc");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&100u32.to_be_bytes()); // horizontal_offset
        data.extend_from_slice(&50u32.to_be_bytes()); // vertical_offset
        data
    }

    #[test]
    fn parse_rloc() {
        let data = make_rloc();
        let view = RelativeLocationBoxView::new(&data).unwrap();

        assert_eq!(view.horizontal_offset(), 100);
        assert_eq!(view.vertical_offset(), 50);
    }

    #[test]
    fn roundtrip() {
        let data = make_rloc();
        let view = RelativeLocationBoxView::new(&data).unwrap();
        let owned = RelativeLocationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
