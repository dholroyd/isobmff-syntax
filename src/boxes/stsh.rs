//! Shadow Sync Sample Box (stsh) parsing and serialization.
//!
//! The Shadow Sync Sample Box provides a list of shadow sync samples.
//! Shadow sync samples allow non-sync samples to be used as sync points.
//!
//! ```text
//! aligned(8) class ShadowSyncSampleBox
//!    extends FullBox('stsh', version = 0, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) shadowed_sample_number;
//!       unsigned int(32) sync_sample_number;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{write_fullbox_header, FullBoxHeader, fullbox_header_size_for_payload};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ShadowSyncSampleBox.
pub const BOX_TYPE: BoxCode = BoxCode::STSH;

/// A shadow sync entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowSyncEntry {
    /// The sample that is shadowed (replaced).
    pub shadowed_sample_number: u32,
    /// The sync sample that shadows it.
    pub sync_sample_number: u32,
}

impl ShadowSyncEntry {
    #[inline]
    fn from_bytes(b: &[u8; 8]) -> Self {
        Self {
            shadowed_sample_number: u32::from_be_bytes(b[0..4].try_into().unwrap()),
            sync_sample_number: u32::from_be_bytes(b[4..8].try_into().unwrap()),
        }
    }
}

/// Common interface for accessing ShadowSyncSampleBox data.
pub trait ShadowSyncSampleBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the entry count.
    fn entry_count(&self) -> u32;

    /// Returns an iterator over shadow sync entries.
    fn entries(&self) -> impl Iterator<Item = ShadowSyncEntry> + '_;
}

/// A borrowing view over raw ShadowSyncSampleBox bytes.
#[derive(Clone, Copy)]
pub struct ShadowSyncSampleBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entries: FixedSizeEntries<'a, 8>,
}

impl<'a> ShadowSyncSampleBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;

        let entry_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);
        let entries = FixedSizeEntries::new(&data[fullbox_offset + 8..], entry_count)?;

        Ok(Self {
            data,
            fullbox_offset,
            entries,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

}

impl ShadowSyncSampleBox for ShadowSyncSampleBoxView<'_> {
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

    fn entries(&self) -> impl Iterator<Item = ShadowSyncEntry> + '_ {
        self.entries.iter().map(ShadowSyncEntry::from_bytes)
    }
}

impl std::fmt::Debug for ShadowSyncSampleBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadowSyncSampleBoxView")
            .field("box_size", &self.box_size())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of ShadowSyncSampleBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ShadowSyncSampleBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Shadow sync entries.
    pub entries: Vec<ShadowSyncEntry>,
}

impl ShadowSyncSampleBoxOwned {
    /// Creates a new empty ShadowSyncSampleBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + self.entries.len() * 8) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.shadowed_sample_number)?;
            writer.write_u32::<BigEndian>(entry.sync_sample_number)?;
        }
        Ok(())
    }
}


impl ShadowSyncSampleBox for ShadowSyncSampleBoxOwned {
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

    fn entries(&self) -> impl Iterator<Item = ShadowSyncEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: ShadowSyncSampleBox> From<&T> for ShadowSyncSampleBoxOwned {
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

    fn make_stsh(entries: &[(u32, u32)]) -> Vec<u8> {
        let size = (8 + 4 + 4 + entries.len() * 8) as u32;
        let mut data = Vec::new();
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(b"stsh");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for (shadowed, sync) in entries {
            data.extend_from_slice(&shadowed.to_be_bytes());
            data.extend_from_slice(&sync.to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_empty_stsh() {
        let data = make_stsh(&[]);
        let view = ShadowSyncSampleBoxView::new(&data).unwrap();
        assert_eq!(view.entry_count(), 0);
        assert_eq!(view.entries().count(), 0);
    }

    #[test]
    fn parse_stsh_with_entries() {
        let data = make_stsh(&[(1, 2), (3, 4), (5, 6)]);
        let view = ShadowSyncSampleBoxView::new(&data).unwrap();
        assert_eq!(view.entry_count(), 3);

        let entries: Vec<_> = view.entries().collect();
        assert_eq!(entries[0].shadowed_sample_number, 1);
        assert_eq!(entries[0].sync_sample_number, 2);
        assert_eq!(entries[1].shadowed_sample_number, 3);
        assert_eq!(entries[1].sync_sample_number, 4);
        assert_eq!(entries[2].shadowed_sample_number, 5);
        assert_eq!(entries[2].sync_sample_number, 6);
    }

    #[test]
    fn roundtrip() {
        let data = make_stsh(&[(10, 20), (30, 40)]);
        let view = ShadowSyncSampleBoxView::new(&data).unwrap();
        let owned = ShadowSyncSampleBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
