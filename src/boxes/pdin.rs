//! Progressive Download Information Box (pdin) parsing and serialization.
//!
//! The Progressive Download Information Box provides rate/initial delay pairs
//! for progressive download scenarios.
//!
//! ```text
//! aligned(8) class ProgressiveDownloadInfoBox
//!    extends FullBox('pdin', version = 0, 0) {
//!    for (i=0; ; i++) { // to end of box
//!       unsigned int(32) rate;
//!       unsigned int(32) initial_delay;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ProgressiveDownloadInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::PDIN;

/// A rate/initial-delay pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressiveDownloadEntry {
    /// Download rate in bytes per second.
    pub rate: u32,
    /// Suggested initial delay in milliseconds.
    pub initial_delay: u32,
}

/// Common interface for accessing ProgressiveDownloadInfoBox data.
pub trait ProgressiveDownloadInfoBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the entry count.
    fn entry_count(&self) -> usize;

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = ProgressiveDownloadEntry> + '_;
}

/// A borrowing view over raw ProgressiveDownloadInfoBox bytes.
#[derive(Clone, Copy)]
pub struct ProgressiveDownloadInfoBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> ProgressiveDownloadInfoBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
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

    /// Returns an entry at the given index.
    pub fn entry(&self, index: usize) -> Option<ProgressiveDownloadEntry> {
        if index >= self.entry_count() {
            return None;
        }

        let offset = self.payload_offset().checked_add(index.checked_mul(8)?)?;
        if offset + 8 > self.data.len() {
            return None;
        }

        Some(ProgressiveDownloadEntry {
            rate: BigEndian::read_u32(&self.data[offset..offset + 4]),
            initial_delay: BigEndian::read_u32(&self.data[offset + 4..offset + 8]),
        })
    }

}

impl<'a> ProgressiveDownloadInfoBox for ProgressiveDownloadInfoBoxView<'a> {
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

    fn entry_count(&self) -> usize {
        let payload_len = self.data.len() - self.payload_offset();
        payload_len / 8
    }

    fn entries(&self) -> impl Iterator<Item = ProgressiveDownloadEntry> + '_ {
        (0..self.entry_count()).filter_map(|i| self.entry(i))
    }
}

impl std::fmt::Debug for ProgressiveDownloadInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressiveDownloadInfoBoxView")
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of ProgressiveDownloadInfoBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ProgressiveDownloadInfoBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Download entries.
    pub entries: Vec<ProgressiveDownloadEntry>,
}

impl ProgressiveDownloadInfoBoxOwned {
    /// Creates a new ProgressiveDownloadInfoBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.entries.len() * 8) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.rate)?;
            writer.write_u32::<BigEndian>(entry.initial_delay)?;
        }

        Ok(())
    }
}


impl ProgressiveDownloadInfoBox for ProgressiveDownloadInfoBoxOwned {
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

    fn entry_count(&self) -> usize {
        self.entries.len()
    }

    fn entries(&self) -> impl Iterator<Item = ProgressiveDownloadEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: ProgressiveDownloadInfoBox> From<&T> for ProgressiveDownloadInfoBoxOwned {
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

    fn make_pdin() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 8 = 20 bytes
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"pdin");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1000u32.to_be_bytes()); // rate
        data.extend_from_slice(&500u32.to_be_bytes()); // initial_delay
        data
    }

    #[test]
    fn parse_pdin() {
        let data = make_pdin();
        let view = ProgressiveDownloadInfoBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 1);
        let entry = view.entry(0).unwrap();
        assert_eq!(entry.rate, 1000);
        assert_eq!(entry.initial_delay, 500);
    }

    #[test]
    fn roundtrip() {
        let data = make_pdin();
        let view = ProgressiveDownloadInfoBoxView::new(&data).unwrap();
        let owned = ProgressiveDownloadInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
