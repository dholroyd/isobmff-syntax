//! Sequence Offset Box (snro) parsing and serialization.
//!
//! The Sequence Offset Box provides the offset for RTP sequence numbers.
//!
//! ```text
//! class sequenceoffset() extends Box('snro') {
//!    int(32) offset;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SequenceOffsetBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"snro");

/// Common interface for accessing SequenceOffsetBox data.
pub trait SequenceOffsetBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the sequence offset.
    fn sequence_offset(&self) -> i32;
}

/// A borrowing view over raw SequenceOffsetBox bytes.
#[derive(Clone, Copy)]
pub struct SequenceOffsetBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> SequenceOffsetBoxView<'a> {
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

impl<'a> SequenceOffsetBox for SequenceOffsetBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn sequence_offset(&self) -> i32 {
        BigEndian::read_i32(&self.data[self.header_size..self.header_size + 4])
    }
}

impl std::fmt::Debug for SequenceOffsetBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequenceOffsetBoxView")
            .field("sequence_offset", &self.sequence_offset())
            .finish()
    }
}

/// An owned representation of SequenceOffsetBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequenceOffsetBoxOwned {
    /// Sequence offset.
    pub sequence_offset: i32,
}

impl SequenceOffsetBoxOwned {
    /// Creates a new SequenceOffsetBoxOwned.
    pub fn new(sequence_offset: i32) -> Self {
        Self { sequence_offset }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_i32::<BigEndian>(self.sequence_offset)?;

        Ok(())
    }
}

impl Default for SequenceOffsetBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl SequenceOffsetBox for SequenceOffsetBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn sequence_offset(&self) -> i32 {
        self.sequence_offset
    }
}

impl<T: SequenceOffsetBox> From<&T> for SequenceOffsetBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            sequence_offset: source.sequence_offset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snro() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"snro");
        data.extend_from_slice(&1000i32.to_be_bytes());
        data
    }

    #[test]
    fn parse_snro() {
        let data = make_snro();
        let view = SequenceOffsetBoxView::new(&data).unwrap();
        assert_eq!(view.sequence_offset(), 1000);
    }

    #[test]
    fn roundtrip() {
        let data = make_snro();
        let view = SequenceOffsetBoxView::new(&data).unwrap();
        let owned = SequenceOffsetBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
