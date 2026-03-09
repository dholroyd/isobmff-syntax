//! Movie Fragment Header Box (mfhd) parsing and serialization.
//!
//! The Movie Fragment Header Box contains a sequence number.
//!
//! ```text
//! aligned(8) class MovieFragmentHeaderBox
//!    extends FullBox('mfhd', 0, 0) {
//!    unsigned int(32) sequence_number;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieFragmentHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::MFHD;

/// Common interface for accessing MovieFragmentHeaderBox data.
pub trait MovieFragmentHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the sequence number.
    fn sequence_number(&self) -> u32;
}

/// A borrowing view over raw MovieFragmentHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct MovieFragmentHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> MovieFragmentHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }
}

impl MovieFragmentHeaderBox for MovieFragmentHeaderBoxView<'_> {
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

    fn sequence_number(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for MovieFragmentHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieFragmentHeaderBoxView")
            .field("sequence_number", &self.sequence_number())
            .finish()
    }
}

/// An owned representation of MovieFragmentHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieFragmentHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Sequence number.
    pub sequence_number: u32,
}

impl MovieFragmentHeaderBoxOwned {
    /// Creates a new MovieFragmentHeaderBoxOwned.
    pub fn new(sequence_number: u32) -> Self {
        Self {
            flags: 0,
            sequence_number,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(4) + 4 // 8 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.sequence_number)?;
        Ok(())
    }
}

impl Default for MovieFragmentHeaderBoxOwned {
    fn default() -> Self {
        Self::new(1)
    }
}

impl MovieFragmentHeaderBox for MovieFragmentHeaderBoxOwned {
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

    fn sequence_number(&self) -> u32 {
        self.sequence_number
    }
}

impl<T: MovieFragmentHeaderBox> From<&T> for MovieFragmentHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            sequence_number: source.sequence_number(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mfhd(seq: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"mfhd");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&seq.to_be_bytes());
        data
    }

    #[test]
    fn parse_mfhd() {
        let data = make_mfhd(42);
        let view = MovieFragmentHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.sequence_number(), 42);
    }

    #[test]
    fn roundtrip() {
        let data = make_mfhd(100);
        let view = MovieFragmentHeaderBoxView::new(&data).unwrap();
        let owned = MovieFragmentHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
