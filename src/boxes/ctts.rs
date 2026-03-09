//! Composition Time to Sample Box (ctts) parsing and serialization.
//!
//! The Composition Time to Sample Box provides the offset between decoding time
//! and composition time for each sample.
//!
//! ```text
//! aligned(8) class CompositionOffsetBox
//!    extends FullBox('ctts', version, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) sample_count;
//!       if (version == 0) {
//!          unsigned int(32) sample_offset;
//!       } else {
//!          signed int(32) sample_offset;
//!       }
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CompositionTimeToSampleBox.
pub const BOX_TYPE: BoxCode = BoxCode::CTTS;

/// A composition time offset entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositionOffsetEntry {
    /// The number of consecutive samples with the same offset.
    pub sample_count: u32,
    /// The composition time offset (signed for version 1, unsigned for version 0).
    pub sample_offset: i64,
}

impl CompositionOffsetEntry {
    #[inline]
    fn from_bytes_v0(b: &[u8; 8]) -> Self {
        Self {
            sample_count: u32::from_be_bytes(b[0..4].try_into().unwrap()),
            sample_offset: u32::from_be_bytes(b[4..8].try_into().unwrap()) as i64,
        }
    }

    #[inline]
    fn from_bytes_v1(b: &[u8; 8]) -> Self {
        Self {
            sample_count: u32::from_be_bytes(b[0..4].try_into().unwrap()),
            sample_offset: i32::from_be_bytes(b[4..8].try_into().unwrap()) as i64,
        }
    }
}

/// Common interface for accessing CompositionTimeToSampleBox data.
pub trait CompositionTimeToSampleBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box (0 or 1).
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the number of entries.
    fn entry_count(&self) -> u32;

    /// Returns the entry at the given index.
    fn entry(&self, index: usize) -> Option<CompositionOffsetEntry>;

    /// Returns an iterator over the entries.
    fn entries(&self) -> impl Iterator<Item = CompositionOffsetEntry> + '_;
}

/// A borrowing view over raw CompositionTimeToSampleBox bytes.
#[derive(Clone, Copy)]
pub struct CompositionTimeToSampleBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    entries: FixedSizeEntries<'a, 8>,
}

impl<'a> CompositionTimeToSampleBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let fullbox_offset = header.validate(data, BOX_TYPE, Some(1), 4)?;

        let entries_offset = fullbox_offset + 8;
        let entry_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);
        let entries = FixedSizeEntries::new(&data[entries_offset..], entry_count)?;

        Ok(Self { data, fullbox_offset, version, entries })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

}

impl CompositionTimeToSampleBox for CompositionTimeToSampleBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn entry_count(&self) -> u32 {
        self.entries.count()
    }

    fn entry(&self, index: usize) -> Option<CompositionOffsetEntry> {
        let decode = if self.version == 0 {
            CompositionOffsetEntry::from_bytes_v0
        } else {
            CompositionOffsetEntry::from_bytes_v1
        };
        self.entries.get(index).map(decode)
    }

    fn entries(&self) -> impl Iterator<Item = CompositionOffsetEntry> + '_ {
        let version = self.version;
        self.entries.iter().map(move |b| {
            if version == 0 {
                CompositionOffsetEntry::from_bytes_v0(b)
            } else {
                CompositionOffsetEntry::from_bytes_v1(b)
            }
        })
    }
}

impl std::fmt::Debug for CompositionTimeToSampleBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositionTimeToSampleBoxView")
            .field("box_size", &self.box_size())
            .field("version", &self.version())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of CompositionTimeToSampleBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct CompositionTimeToSampleBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The entries.
    pub entries: Vec<CompositionOffsetEntry>,
}

impl CompositionTimeToSampleBoxOwned {
    /// Creates a new empty CompositionTimeToSampleBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the version required for this box.
    fn required_version(&self) -> u8 {
        // Use version 1 if any offset is negative
        if self.entries.iter().any(|e| e.sample_offset < 0) {
            1
        } else {
            0
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 4 + (self.entries.len() as u64) * 8;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.required_version();
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.sample_count)?;
            if version == 0 {
                writer.write_u32::<BigEndian>(entry.sample_offset as u32)?;
            } else {
                writer.write_i32::<BigEndian>(entry.sample_offset as i32)?;
            }
        }

        Ok(())
    }
}


impl CompositionTimeToSampleBox for CompositionTimeToSampleBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.required_version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entry(&self, index: usize) -> Option<CompositionOffsetEntry> {
        self.entries.get(index).copied()
    }

    fn entries(&self) -> impl Iterator<Item = CompositionOffsetEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: CompositionTimeToSampleBox> From<&T> for CompositionTimeToSampleBoxOwned {
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

    fn make_ctts_v0(entries: &[CompositionOffsetEntry]) -> Vec<u8> {
        let size = 8 + 4 + 4 + entries.len() * 8;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"ctts");
        data.push(0); // version 0
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for entry in entries {
            data.extend_from_slice(&entry.sample_count.to_be_bytes());
            data.extend_from_slice(&(entry.sample_offset as u32).to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_ctts_v0() {
        let entries = vec![
            CompositionOffsetEntry { sample_count: 10, sample_offset: 1024 },
            CompositionOffsetEntry { sample_count: 20, sample_offset: 2048 },
        ];
        let data = make_ctts_v0(&entries);
        let view = CompositionTimeToSampleBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.entry_count(), 2);
        assert_eq!(view.entry(0), Some(entries[0]));
        assert_eq!(view.entry(1), Some(entries[1]));
    }

    #[test]
    fn roundtrip_v0() {
        let entries = vec![
            CompositionOffsetEntry { sample_count: 10, sample_offset: 1024 },
        ];
        let data = make_ctts_v0(&entries);
        let view = CompositionTimeToSampleBoxView::new(&data).unwrap();
        let owned = CompositionTimeToSampleBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
