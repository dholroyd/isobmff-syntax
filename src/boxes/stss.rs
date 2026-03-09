//! Sync Sample Box (stss) parsing and serialization.
//!
//! The Sync Sample Box provides a compact marking of the sync samples
//! (key frames) within the media.
//!
//! ```text
//! aligned(8) class SyncSampleBox
//!    extends FullBox('stss', version = 0, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) sample_number;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SyncSampleBox.
pub const BOX_TYPE: BoxCode = BoxCode::STSS;

/// Common interface for accessing SyncSampleBox data.
pub trait SyncSampleBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the number of sync samples.
    fn entry_count(&self) -> u32;

    /// Returns the sample number at the given index (1-based sample numbers).
    fn entry(&self, index: usize) -> Option<u32>;

    /// Returns an iterator over the sync sample numbers.
    fn entries(&self) -> impl Iterator<Item = u32> + '_;

    /// Checks if the given sample number (1-based) is a sync sample.
    fn is_sync_sample(&self, sample_number: u32) -> bool {
        self.entries().any(|s| s == sample_number)
    }
}

/// A borrowing view over raw SyncSampleBox bytes.
#[derive(Clone, Copy)]
pub struct SyncSampleBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entries: FixedSizeEntries<'a, 4>,
}

impl<'a> SyncSampleBoxView<'a> {
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

impl SyncSampleBox for SyncSampleBoxView<'_> {
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

impl std::fmt::Debug for SyncSampleBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncSampleBoxView")
            .field("box_size", &self.box_size())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of SyncSampleBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SyncSampleBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The sync sample numbers (1-based).
    pub entries: Vec<u32>,
}

impl SyncSampleBoxOwned {
    /// Creates a new empty SyncSampleBoxOwned.
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

        for &sample in &self.entries {
            writer.write_u32::<BigEndian>(sample)?;
        }

        Ok(())
    }
}


impl SyncSampleBox for SyncSampleBoxOwned {
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

impl<T: SyncSampleBox> From<&T> for SyncSampleBoxOwned {
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

    fn make_stss(samples: &[u32]) -> Vec<u8> {
        let size = 8 + 4 + 4 + samples.len() * 4;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"stss");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&(samples.len() as u32).to_be_bytes());
        for &s in samples {
            data.extend_from_slice(&s.to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_stss() {
        let samples = vec![1, 10, 20, 30];
        let data = make_stss(&samples);
        let view = SyncSampleBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 4);
        assert_eq!(view.entry(0), Some(1));
        assert_eq!(view.entry(3), Some(30));
        assert!(view.is_sync_sample(10));
        assert!(!view.is_sync_sample(15));
    }

    #[test]
    fn roundtrip() {
        let samples = vec![1, 10, 20];
        let data = make_stss(&samples);
        let view = SyncSampleBoxView::new(&data).unwrap();
        let owned = SyncSampleBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
