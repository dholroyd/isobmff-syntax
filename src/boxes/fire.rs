//! File Reservoir Box (fire) parsing and serialization.
//!
//! The File Reservoir Box specifies file identifiers for file delivery.
//!
//! ```text
//! aligned(8) class FileReservoirBox
//!    extends FullBox('fire', version, 0) {
//!    if (version == 0) {
//!       unsigned int(16) entry_count;
//!    } else {
//!       unsigned int(32) entry_count;
//!    }
//!    for (i=1; i <= entry_count; i++) {
//!       if (version == 0) {
//!          unsigned int(16) item_ID;
//!       } else {
//!          unsigned int(32) item_ID;
//!       }
//!       unsigned int(32) symbol_count;
//!    }
//! }
//! ```

use crate::error::{ParseError, validate_entry_count};
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for FileReservoirBox.
pub const BOX_TYPE: BoxCode = BoxCode::FIRE;

/// A file reservoir entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileReservoirEntry {
    /// Item ID.
    pub item_id: u32,
    /// Symbol count.
    pub symbol_count: u32,
}

impl FileReservoirEntry {
    #[inline]
    fn from_bytes_v0(b: &[u8]) -> Self {
        Self {
            item_id: BigEndian::read_u16(&b[0..2]) as u32,
            symbol_count: BigEndian::read_u32(&b[2..6]),
        }
    }

    #[inline]
    fn from_bytes_v1(b: &[u8]) -> Self {
        Self {
            item_id: BigEndian::read_u32(&b[0..4]),
            symbol_count: BigEndian::read_u32(&b[4..8]),
        }
    }
}

/// Common interface for accessing FileReservoirBox data.
pub trait FileReservoirBox {
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

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = FileReservoirEntry> + '_;
}

/// A borrowing view over raw FileReservoirBox bytes.
#[derive(Clone, Copy)]
pub struct FileReservoirBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    entry_count: u32,
    /// Byte offset where entries start.
    entries_offset: usize,
}

impl<'a> FileReservoirBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let entry_count_size = if version == 0 { 2 } else { 4 };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, entry_count_size)?;

        let entry_count = if version == 0 {
            BigEndian::read_u16(&data[fullbox_offset + 4..fullbox_offset + 6]) as u32
        } else {
            BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8])
        };
        let entries_offset = fullbox_offset + 4 + entry_count_size;
        let entry_size = if version == 0 { 6 } else { 8 }; // u16+u32 or u32+u32
        validate_entry_count(data, entries_offset, entry_count, entry_size)?;

        Ok(Self {
            data,
            fullbox_offset,
            version,
            entry_count,
            entries_offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the entry stride (item_id size + symbol_count size).
    fn entry_stride(&self) -> usize {
        if self.version == 0 { 6 } else { 8 }
    }

    /// Returns an iterator over all entries.
    pub fn entries(&self) -> impl Iterator<Item = FileReservoirEntry> + '_ {
        let decode: fn(&[u8]) -> FileReservoirEntry = if self.version == 0 {
            FileReservoirEntry::from_bytes_v0
        } else {
            FileReservoirEntry::from_bytes_v1
        };
        let stride = self.entry_stride();
        let end = self.entries_offset + self.entry_count as usize * stride;
        self.data[self.entries_offset..end].chunks_exact(stride).map(decode)
    }
}

impl<'a> FileReservoirBox for FileReservoirBoxView<'a> {
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
        self.entry_count
    }

    fn entries(&self) -> impl Iterator<Item = FileReservoirEntry> + '_ {
        FileReservoirBoxView::entries(self)
    }
}

impl std::fmt::Debug for FileReservoirBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileReservoirBoxView")
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of FileReservoirBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct FileReservoirBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Entries.
    pub entries: Vec<FileReservoirEntry>,
}

impl FileReservoirBoxOwned {
    /// Creates a new FileReservoirBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if large (version >= 1) format is needed.
    fn needs_large_format(&self) -> bool {
        self.entries.len() > u16::MAX as usize
            || self.entries.iter().any(|e| e.item_id > u16::MAX as u32)
    }

    fn effective_version(&self) -> u8 {
        if self.needs_large_format() { 1 } else { 0 }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let version = self.effective_version();
        let entry_count_size = if version == 0 { 2 } else { 4 };
        let entry_stride = if version == 0 { 6 } else { 8 }; // u16+u32 or u32+u32
        let payload = (entry_count_size + self.entries.len() * entry_stride) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.effective_version();
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;

        if version == 0 {
            writer.write_u16::<BigEndian>(self.entries.len() as u16)?;
        } else {
            writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
        }

        for entry in &self.entries {
            if version == 0 {
                writer.write_u16::<BigEndian>(entry.item_id as u16)?;
            } else {
                writer.write_u32::<BigEndian>(entry.item_id)?;
            }
            writer.write_u32::<BigEndian>(entry.symbol_count)?;
        }

        Ok(())
    }
}


impl FileReservoirBox for FileReservoirBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.effective_version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> impl Iterator<Item = FileReservoirEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: FileReservoirBox> From<&T> for FileReservoirBoxOwned {
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

    fn make_fire() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&14u32.to_be_bytes()); // 8 + 4 + 2
        data.extend_from_slice(b"fire");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u16.to_be_bytes()); // entry_count
        data
    }

    #[test]
    fn parse_fire() {
        let data = make_fire();
        let view = FileReservoirBoxView::new(&data).unwrap();
        assert_eq!(view.entry_count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_fire();
        let view = FileReservoirBoxView::new(&data).unwrap();
        let owned = FileReservoirBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
