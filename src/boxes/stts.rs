//! Time to Sample Box (stts) parsing and serialization.
//!
//! The Time to Sample Box contains a compact version of a table that allows
//! indexing from decoding time to sample number.
//!
//! ```text
//! aligned(8) class TimeToSampleBox
//!    extends FullBox('stts', version = 0, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) sample_count;
//!       unsigned int(32) sample_delta;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TimeToSampleBox.
pub const BOX_TYPE: BoxCode = BoxCode::STTS;

/// A time-to-sample entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeToSampleEntry {
    /// The number of consecutive samples with the same delta.
    pub sample_count: u32,
    /// The duration of each sample in timescale units.
    pub sample_delta: u32,
}

impl TimeToSampleEntry {
    #[inline]
    fn from_bytes(b: &[u8; 8]) -> Self {
        Self {
            sample_count: u32::from_be_bytes(b[0..4].try_into().unwrap()),
            sample_delta: u32::from_be_bytes(b[4..8].try_into().unwrap()),
        }
    }
}

/// Common interface for accessing TimeToSampleBox data.
pub trait TimeToSampleBox {
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
    fn entry(&self, index: usize) -> Option<TimeToSampleEntry>;

    /// Returns an iterator over the entries.
    fn entries(&self) -> impl Iterator<Item = TimeToSampleEntry> + '_;
}

/// A borrowing view over raw TimeToSampleBox bytes.
#[derive(Clone, Copy)]
pub struct TimeToSampleBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entries: FixedSizeEntries<'a, 8>,
}

impl<'a> TimeToSampleBoxView<'a> {
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

impl TimeToSampleBox for TimeToSampleBoxView<'_> {
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

    fn entry(&self, index: usize) -> Option<TimeToSampleEntry> {
        self.entries.get(index).map(TimeToSampleEntry::from_bytes)
    }

    fn entries(&self) -> impl Iterator<Item = TimeToSampleEntry> + '_ {
        self.entries.iter().map(TimeToSampleEntry::from_bytes)
    }
}

impl std::fmt::Debug for TimeToSampleBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeToSampleBoxView")
            .field("box_size", &self.box_size())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of TimeToSampleBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TimeToSampleBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The entries.
    pub entries: Vec<TimeToSampleEntry>,
}

impl TimeToSampleBoxOwned {
    /// Creates a new empty TimeToSampleBoxOwned.
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

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.sample_count)?;
            writer.write_u32::<BigEndian>(entry.sample_delta)?;
        }

        Ok(())
    }
}


impl TimeToSampleBox for TimeToSampleBoxOwned {
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

    fn entry(&self, index: usize) -> Option<TimeToSampleEntry> {
        self.entries.get(index).copied()
    }

    fn entries(&self) -> impl Iterator<Item = TimeToSampleEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: TimeToSampleBox> From<&T> for TimeToSampleBoxOwned {
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

    fn make_stts(entries: &[TimeToSampleEntry]) -> Vec<u8> {
        let size = 8 + 4 + 4 + entries.len() * 8;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"stts");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for entry in entries {
            data.extend_from_slice(&entry.sample_count.to_be_bytes());
            data.extend_from_slice(&entry.sample_delta.to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_stts() {
        let entries = vec![
            TimeToSampleEntry { sample_count: 100, sample_delta: 1024 },
            TimeToSampleEntry { sample_count: 50, sample_delta: 512 },
        ];
        let data = make_stts(&entries);
        let view = TimeToSampleBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 2);
        assert_eq!(view.entry(0), Some(entries[0]));
        assert_eq!(view.entry(1), Some(entries[1]));
        assert_eq!(view.entry(2), None);
    }

    #[test]
    fn roundtrip() {
        let entries = vec![
            TimeToSampleEntry { sample_count: 100, sample_delta: 1024 },
        ];
        let data = make_stts(&entries);
        let view = TimeToSampleBoxView::new(&data).unwrap();
        let owned = TimeToSampleBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
