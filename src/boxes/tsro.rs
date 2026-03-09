//! Time Offset Box (tsro) parsing and serialization.
//!
//! The Time Offset Box specifies an offset to add to track timestamps.
//!
//! ```text
//! class timeoffset() extends Box('tsro') {
//!    int(32) offset;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TimeOffsetBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"tsro");

/// Common interface for accessing TimeOffsetBox data.
pub trait TimeOffsetBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the offset.
    fn offset(&self) -> i32;
}

/// A borrowing view over raw TimeOffsetBox bytes.
#[derive(Clone, Copy)]
pub struct TimeOffsetBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> TimeOffsetBoxView<'a> {
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

impl<'a> TimeOffsetBox for TimeOffsetBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn offset(&self) -> i32 {
        let o = self.header_size;
        BigEndian::read_i32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for TimeOffsetBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeOffsetBoxView")
            .field("offset", &self.offset())
            .finish()
    }
}

/// An owned representation of TimeOffsetBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeOffsetBoxOwned {
    /// Offset.
    pub offset: i32,
}

impl TimeOffsetBoxOwned {
    /// Creates a new TimeOffsetBoxOwned.
    pub fn new(offset: i32) -> Self {
        Self { offset }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_i32::<BigEndian>(self.offset)?;

        Ok(())
    }
}

impl Default for TimeOffsetBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TimeOffsetBox for TimeOffsetBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn offset(&self) -> i32 {
        self.offset
    }
}

impl<T: TimeOffsetBox> From<&T> for TimeOffsetBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            offset: source.offset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tsro() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"tsro");
        data.extend_from_slice(&(-100i32).to_be_bytes()); // offset
        data
    }

    #[test]
    fn parse_tsro() {
        let data = make_tsro();
        let view = TimeOffsetBoxView::new(&data).unwrap();
        assert_eq!(view.offset(), -100);
    }

    #[test]
    fn roundtrip() {
        let data = make_tsro();
        let view = TimeOffsetBoxView::new(&data).unwrap();
        let owned = TimeOffsetBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
