//! Chunk Large Offset Box (co64) parsing and serialization.
//!
//! The Chunk Large Offset Box gives 64-bit offsets for each chunk.
//!
//! ```text
//! aligned(8) class ChunkLargeOffsetBox
//!    extends FullBox('co64', version = 0, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(64) chunk_offset;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ChunkLargeOffsetBox.
pub const BOX_TYPE: BoxCode = BoxCode::CO64;

/// Common interface for accessing ChunkLargeOffsetBox data.
pub trait ChunkLargeOffsetBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the number of entries.
    fn entry_count(&self) -> u32;

    /// Returns the chunk offset at the given index (0-based).
    fn entry(&self, index: usize) -> Option<u64>;

    /// Returns an iterator over the chunk offsets.
    fn entries(&self) -> impl Iterator<Item = u64> + '_;
}

/// A borrowing view over raw ChunkLargeOffsetBox bytes.
#[derive(Clone, Copy)]
pub struct ChunkLargeOffsetBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entries: FixedSizeEntries<'a, 8>,
}

impl<'a> ChunkLargeOffsetBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;

        let entries_offset = fullbox_offset + 8;
        let entry_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);
        let entries = FixedSizeEntries::new(&data[entries_offset..], entry_count)?;

        Ok(Self { data, fullbox_offset, entries })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

}

impl ChunkLargeOffsetBox for ChunkLargeOffsetBoxView<'_> {
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

    fn entry_count(&self) -> u32 {
        self.entries.count()
    }

    fn entry(&self, index: usize) -> Option<u64> {
        self.entries.get(index).map(|b| u64::from_be_bytes(*b))
    }

    fn entries(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.iter().map(|b| u64::from_be_bytes(*b))
    }
}

impl std::fmt::Debug for ChunkLargeOffsetBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkLargeOffsetBoxView")
            .field("box_size", &self.box_size())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of ChunkLargeOffsetBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ChunkLargeOffsetBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The chunk offsets.
    pub entries: Vec<u64>,
}

impl ChunkLargeOffsetBoxOwned {
    /// Creates a new empty ChunkLargeOffsetBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 4 + (self.entries.len() as u64) * 8;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for &offset in &self.entries {
            writer.write_u64::<BigEndian>(offset)?;
        }

        Ok(())
    }
}


impl ChunkLargeOffsetBox for ChunkLargeOffsetBoxOwned {
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

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entry(&self, index: usize) -> Option<u64> {
        self.entries.get(index).copied()
    }

    fn entries(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: ChunkLargeOffsetBox> From<&T> for ChunkLargeOffsetBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            entries: source.entries().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_co64(offsets: &[u64]) -> Vec<u8> {
        let size = 8 + 4 + 4 + offsets.len() * 8;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"co64");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&(offsets.len() as u32).to_be_bytes());
        for &o in offsets {
            data.extend_from_slice(&o.to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_co64() {
        let offsets = vec![0x1_0000_0000u64, 0x2_0000_0000u64];
        let data = make_co64(&offsets);
        let view = ChunkLargeOffsetBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 2);
        assert_eq!(view.entry(0), Some(0x1_0000_0000));
        assert_eq!(view.entry(1), Some(0x2_0000_0000));
    }

    #[test]
    fn roundtrip() {
        let offsets = vec![0x1_0000_0000u64, 0x2_0000_0000u64];
        let data = make_co64(&offsets);
        let view = ChunkLargeOffsetBoxView::new(&data).unwrap();
        let owned = ChunkLargeOffsetBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
