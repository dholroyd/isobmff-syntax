//! Chunk Offset Box (stco) parsing and serialization.
//!
//! The Chunk Offset Box gives the offset of each chunk into the containing file.
//!
//! ```text
//! aligned(8) class ChunkOffsetBox
//!    extends FullBox('stco', version = 0, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) chunk_offset;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ChunkOffsetBox.
pub const BOX_TYPE: BoxCode = BoxCode::STCO;

/// Common interface for accessing ChunkOffsetBox data.
pub trait ChunkOffsetBox {
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
    fn entry(&self, index: usize) -> Option<u32>;

    /// Returns an iterator over the chunk offsets.
    fn entries(&self) -> impl Iterator<Item = u32> + '_;
}

/// A borrowing view over raw ChunkOffsetBox bytes.
#[derive(Clone, Copy)]
pub struct ChunkOffsetBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entries: FixedSizeEntries<'a, 4>,
}

impl<'a> ChunkOffsetBoxView<'a> {
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

impl ChunkOffsetBox for ChunkOffsetBoxView<'_> {
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

    fn entry(&self, index: usize) -> Option<u32> {
        self.entries.get(index).map(|b| u32::from_be_bytes(*b))
    }

    fn entries(&self) -> impl Iterator<Item = u32> + '_ {
        self.entries.iter().map(|b| u32::from_be_bytes(*b))
    }
}

impl std::fmt::Debug for ChunkOffsetBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkOffsetBoxView")
            .field("box_size", &self.box_size())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of ChunkOffsetBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ChunkOffsetBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The chunk offsets.
    pub entries: Vec<u32>,
}

impl ChunkOffsetBoxOwned {
    /// Creates a new empty ChunkOffsetBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 4 + (self.entries.len() as u64) * 4;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for &offset in &self.entries {
            writer.write_u32::<BigEndian>(offset)?;
        }

        Ok(())
    }
}


impl ChunkOffsetBox for ChunkOffsetBoxOwned {
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

    fn entry(&self, index: usize) -> Option<u32> {
        self.entries.get(index).copied()
    }

    fn entries(&self) -> impl Iterator<Item = u32> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: ChunkOffsetBox> From<&T> for ChunkOffsetBoxOwned {
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

    fn make_stco(offsets: &[u32]) -> Vec<u8> {
        let size = 8 + 4 + 4 + offsets.len() * 4;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"stco");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&(offsets.len() as u32).to_be_bytes());
        for &o in offsets {
            data.extend_from_slice(&o.to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_stco() {
        let offsets = vec![1000, 2000, 3000];
        let data = make_stco(&offsets);
        let view = ChunkOffsetBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 3);
        assert_eq!(view.entry(0), Some(1000));
        assert_eq!(view.entry(1), Some(2000));
        assert_eq!(view.entry(2), Some(3000));
    }

    #[test]
    fn roundtrip() {
        let offsets = vec![1000, 2000, 3000];
        let data = make_stco(&offsets);
        let view = ChunkOffsetBoxView::new(&data).unwrap();
        let owned = ChunkOffsetBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
