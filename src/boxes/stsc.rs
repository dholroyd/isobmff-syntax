//! Sample to Chunk Box (stsc) parsing and serialization.
//!
//! The Sample to Chunk Box contains a table providing the mapping from
//! sample number to chunk number.
//!
//! ```text
//! aligned(8) class SampleToChunkBox
//!    extends FullBox('stsc', version = 0, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) first_chunk;
//!       unsigned int(32) samples_per_chunk;
//!       unsigned int(32) sample_description_index;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SampleToChunkBox.
pub const BOX_TYPE: BoxCode = BoxCode::STSC;

/// A sample-to-chunk entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleToChunkEntry {
    /// The first chunk number with this pattern.
    pub first_chunk: u32,
    /// The number of samples in each chunk.
    pub samples_per_chunk: u32,
    /// The sample description index.
    pub sample_description_index: u32,
}

impl SampleToChunkEntry {
    #[inline]
    fn from_bytes(b: &[u8; 12]) -> Self {
        Self {
            first_chunk: u32::from_be_bytes(b[0..4].try_into().unwrap()),
            samples_per_chunk: u32::from_be_bytes(b[4..8].try_into().unwrap()),
            sample_description_index: u32::from_be_bytes(b[8..12].try_into().unwrap()),
        }
    }
}

/// Common interface for accessing SampleToChunkBox data.
pub trait SampleToChunkBox {
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

    /// Returns the entry at the given index.
    fn entry(&self, index: usize) -> Option<SampleToChunkEntry>;

    /// Returns an iterator over the entries.
    fn entries(&self) -> impl Iterator<Item = SampleToChunkEntry> + '_;
}

/// A borrowing view over raw SampleToChunkBox bytes.
#[derive(Clone, Copy)]
pub struct SampleToChunkBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entries: FixedSizeEntries<'a, 12>,
}

impl<'a> SampleToChunkBoxView<'a> {
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

impl SampleToChunkBox for SampleToChunkBoxView<'_> {
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

    fn entry(&self, index: usize) -> Option<SampleToChunkEntry> {
        self.entries.get(index).map(SampleToChunkEntry::from_bytes)
    }

    fn entries(&self) -> impl Iterator<Item = SampleToChunkEntry> + '_ {
        self.entries.iter().map(SampleToChunkEntry::from_bytes)
    }
}

impl std::fmt::Debug for SampleToChunkBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleToChunkBoxView")
            .field("box_size", &self.box_size())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of SampleToChunkBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SampleToChunkBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The entries.
    pub entries: Vec<SampleToChunkEntry>,
}

impl SampleToChunkBoxOwned {
    /// Creates a new empty SampleToChunkBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 4 + (self.entries.len() as u64) * 12;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.first_chunk)?;
            writer.write_u32::<BigEndian>(entry.samples_per_chunk)?;
            writer.write_u32::<BigEndian>(entry.sample_description_index)?;
        }

        Ok(())
    }
}


impl SampleToChunkBox for SampleToChunkBoxOwned {
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

    fn entry(&self, index: usize) -> Option<SampleToChunkEntry> {
        self.entries.get(index).copied()
    }

    fn entries(&self) -> impl Iterator<Item = SampleToChunkEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: SampleToChunkBox> From<&T> for SampleToChunkBoxOwned {
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

    fn make_stsc(entries: &[SampleToChunkEntry]) -> Vec<u8> {
        let size = 8 + 4 + 4 + entries.len() * 12;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"stsc");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for entry in entries {
            data.extend_from_slice(&entry.first_chunk.to_be_bytes());
            data.extend_from_slice(&entry.samples_per_chunk.to_be_bytes());
            data.extend_from_slice(&entry.sample_description_index.to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_stsc() {
        let entries = vec![
            SampleToChunkEntry { first_chunk: 1, samples_per_chunk: 10, sample_description_index: 1 },
        ];
        let data = make_stsc(&entries);
        let view = SampleToChunkBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 1);
        assert_eq!(view.entry(0), Some(entries[0]));
    }

    #[test]
    fn roundtrip() {
        let entries = vec![
            SampleToChunkEntry { first_chunk: 1, samples_per_chunk: 10, sample_description_index: 1 },
        ];
        let data = make_stsc(&entries);
        let view = SampleToChunkBoxView::new(&data).unwrap();
        let owned = SampleToChunkBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
