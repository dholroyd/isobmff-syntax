//! Timestamp Offset Box (trpo) parsing and serialization.
//!
//! The Timestamp Offset Box provides the offset for RTP timestamps.
//!
//! ```text
//! aligned(8) class TimeOffset extends Box('trpo') {
//!    int(32) offset; // signed offset to add to RTP timestamps
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TimestampOffsetBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"trpo");

/// Common interface for accessing TimestampOffsetBox data.
pub trait TimestampOffsetBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the timestamp offset.
    fn timestamp_offset(&self) -> i32;
}

/// A borrowing view over raw TimestampOffsetBox bytes.
#[derive(Clone, Copy)]
pub struct TimestampOffsetBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> TimestampOffsetBoxView<'a> {
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

impl<'a> TimestampOffsetBox for TimestampOffsetBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn timestamp_offset(&self) -> i32 {
        BigEndian::read_i32(&self.data[self.header_size..self.header_size + 4])
    }
}

impl std::fmt::Debug for TimestampOffsetBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimestampOffsetBoxView")
            .field("timestamp_offset", &self.timestamp_offset())
            .finish()
    }
}

/// An owned representation of TimestampOffsetBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimestampOffsetBoxOwned {
    /// Timestamp offset.
    pub timestamp_offset: i32,
}

impl TimestampOffsetBoxOwned {
    /// Creates a new TimestampOffsetBoxOwned.
    pub fn new(timestamp_offset: i32) -> Self {
        Self { timestamp_offset }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_i32::<BigEndian>(self.timestamp_offset)?;

        Ok(())
    }
}

impl Default for TimestampOffsetBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TimestampOffsetBox for TimestampOffsetBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn timestamp_offset(&self) -> i32 {
        self.timestamp_offset
    }
}

impl<T: TimestampOffsetBox> From<&T> for TimestampOffsetBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            timestamp_offset: source.timestamp_offset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trpo() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"trpo");
        data.extend_from_slice(&500i32.to_be_bytes());
        data
    }

    #[test]
    fn parse_trpo() {
        let data = make_trpo();
        let view = TimestampOffsetBoxView::new(&data).unwrap();
        assert_eq!(view.timestamp_offset(), 500);
    }

    #[test]
    fn roundtrip() {
        let data = make_trpo();
        let view = TimestampOffsetBoxView::new(&data).unwrap();
        let owned = TimestampOffsetBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
