//! Edit List Box (elst) parsing and serialization.
//!
//! The Edit List Box contains an explicit timeline map for the track.
//!
//! ```text
//! aligned(8) class EditListBox extends FullBox('elst', version, flags) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       if (version==1) {
//!          unsigned int(64) segment_duration;
//!          int(64) media_time;
//!       } else { // version==0
//!          unsigned int(32) segment_duration;
//!          int(32) media_time;
//!       }
//!       int(16) media_rate_integer;
//!       int(16) media_rate_fraction = 0;
//!    }
//! }
//! ```

use crate::error::{ParseError, validate_entry_count};
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use crate::types::FixedPoint16_16;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for EditListBox.
pub const BOX_TYPE: BoxCode = BoxCode::ELST;

/// An edit list entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditListEntry {
    /// Duration of this edit in movie timescale units.
    pub segment_duration: u64,
    /// Starting time within the media (-1 for empty edit).
    pub media_time: i64,
    /// Media rate for the edit (typically 1.0).
    pub media_rate: FixedPoint16_16,
}

impl EditListEntry {
    /// Returns true if this is an empty edit (media_time == -1).
    pub fn is_empty_edit(&self) -> bool {
        self.media_time == -1
    }

    /// Returns true if this is a dwell (segment_duration == 0).
    pub fn is_dwell(&self) -> bool {
        self.segment_duration == 0
    }

    #[inline]
    fn from_bytes_v0(b: &[u8]) -> Self {
        Self {
            segment_duration: BigEndian::read_u32(&b[0..4]) as u64,
            media_time: BigEndian::read_i32(&b[4..8]) as i64,
            media_rate: FixedPoint16_16::from_raw(BigEndian::read_i32(&b[8..12])),
        }
    }

    #[inline]
    fn from_bytes_v1(b: &[u8]) -> Self {
        Self {
            segment_duration: BigEndian::read_u64(&b[0..8]),
            media_time: BigEndian::read_i64(&b[8..16]),
            media_rate: FixedPoint16_16::from_raw(BigEndian::read_i32(&b[16..20])),
        }
    }
}

/// Common interface for accessing EditListBox data.
pub trait EditListBox {
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
    fn entry(&self, index: usize) -> Option<EditListEntry>;

    /// Returns an iterator over the entries.
    fn entries(&self) -> impl Iterator<Item = EditListEntry> + '_;
}

/// A borrowing view over raw EditListBox bytes.
#[derive(Clone, Copy)]
pub struct EditListBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> EditListBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, Some(1), 4)?;

        let view = Self { data, fullbox_offset };

        let entry_count = view.entry_count();
        let entry_size = if view.version() == 0 { 12 } else { 20 };
        validate_entry_count(data, fullbox_offset + 8, entry_count, entry_size)?;

        Ok(view)
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

impl EditListBox for EditListBoxView<'_> {
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
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn entry(&self, index: usize) -> Option<EditListEntry> {
        let decode = if self.version() == 0 {
            EditListEntry::from_bytes_v0
        } else {
            EditListEntry::from_bytes_v1
        };
        let entry_size = if self.version() == 0 { 12 } else { 20 };
        let o = self.payload_offset() + 4 + index * entry_size;
        self.data.get(o..o + entry_size).map(decode)
    }

    fn entries(&self) -> impl Iterator<Item = EditListEntry> + '_ {
        let decode: fn(&[u8]) -> EditListEntry = if self.version() == 0 {
            EditListEntry::from_bytes_v0
        } else {
            EditListEntry::from_bytes_v1
        };
        let entry_size = if self.version() == 0 { 12 } else { 20 };
        let start = self.payload_offset() + 4;
        let end = start + self.entry_count() as usize * entry_size;
        self.data[start..end].chunks_exact(entry_size).map(decode)
    }
}

impl std::fmt::Debug for EditListBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditListBoxView")
            .field("box_size", &self.box_size())
            .field("version", &self.version())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of EditListBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct EditListBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The entries.
    pub entries: Vec<EditListEntry>,
}

impl EditListBoxOwned {
    /// Creates a new empty EditListBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the version required for this box.
    fn required_version(&self) -> u8 {
        // Use version 1 if any value exceeds 32-bit range
        let needs_v1 = self.entries.iter().any(|e| {
            e.segment_duration > u32::MAX as u64
                || e.media_time > i32::MAX as i64
                || e.media_time < i32::MIN as i64
        });
        if needs_v1 { 1 } else { 0 }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
let version = self.required_version();
        let entry_size = if version == 0 { 12u64 } else { 20u64 };
        fullbox_header_size_for_payload(4 + (self.entries.len() as u64) * entry_size) + 4 + (self.entries.len() as u64) * entry_size
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.required_version();
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            if version == 0 {
                writer.write_u32::<BigEndian>(entry.segment_duration as u32)?;
                writer.write_i32::<BigEndian>(entry.media_time as i32)?;
            } else {
                writer.write_u64::<BigEndian>(entry.segment_duration)?;
                writer.write_i64::<BigEndian>(entry.media_time)?;
            }
            writer.write_i32::<BigEndian>(entry.media_rate.raw())?;
        }

        Ok(())
    }
}


impl EditListBox for EditListBoxOwned {
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

    fn entry(&self, index: usize) -> Option<EditListEntry> {
        self.entries.get(index).copied()
    }

    fn entries(&self) -> impl Iterator<Item = EditListEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: EditListBox> From<&T> for EditListBoxOwned {
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

    fn make_elst_v0(entries: &[EditListEntry]) -> Vec<u8> {
        let size = 8 + 4 + 4 + entries.len() * 12;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"elst");
        data.push(0); // version 0
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for entry in entries {
            data.extend_from_slice(&(entry.segment_duration as u32).to_be_bytes());
            data.extend_from_slice(&(entry.media_time as i32).to_be_bytes());
            data.extend_from_slice(&entry.media_rate.raw().to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_elst_v0() {
        let entries = vec![
            EditListEntry {
                segment_duration: 1000,
                media_time: 0,
                media_rate: FixedPoint16_16::ONE,
            },
        ];
        let data = make_elst_v0(&entries);
        let view = EditListBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.entry_count(), 1);
        let entry = view.entry(0).unwrap();
        assert_eq!(entry.segment_duration, 1000);
        assert_eq!(entry.media_time, 0);
        assert_eq!(entry.media_rate, FixedPoint16_16::ONE);
    }

    #[test]
    fn empty_edit() {
        let entry = EditListEntry {
            segment_duration: 1000,
            media_time: -1,
            media_rate: FixedPoint16_16::ONE,
        };
        assert!(entry.is_empty_edit());
    }

    #[test]
    fn roundtrip_v0() {
        let entries = vec![
            EditListEntry {
                segment_duration: 1000,
                media_time: 500,
                media_rate: FixedPoint16_16::ONE,
            },
        ];
        let data = make_elst_v0(&entries);
        let view = EditListBoxView::new(&data).unwrap();
        let owned = EditListBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
