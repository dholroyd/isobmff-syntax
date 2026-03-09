//! Sample to Group Box (sbgp) parsing and serialization.
//!
//! The Sample to Group Box provides the assignment of samples to sample groups.
//!
//! ```text
//! aligned(8) class SampleToGroupBox
//!    extends FullBox('sbgp', version, 0) {
//!    unsigned int(32) grouping_type;
//!    if (version == 1) {
//!       unsigned int(32) grouping_type_parameter;
//!    }
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       unsigned int(32) sample_count;
//!       unsigned int(32) group_description_index;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for SampleToGroupBox.
pub const BOX_TYPE: BoxCode = BoxCode::SBGP;

/// A single entry mapping a run of samples to a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleToGroupEntry {
    /// Number of consecutive samples with the same group description.
    pub sample_count: u32,
    /// Index of the sample group entry (0 means not mapped to any group).
    pub group_description_index: u32,
}

impl SampleToGroupEntry {
    #[inline]
    fn from_bytes(b: &[u8; 8]) -> Self {
        Self {
            sample_count: u32::from_be_bytes(b[0..4].try_into().unwrap()),
            group_description_index: u32::from_be_bytes(b[4..8].try_into().unwrap()),
        }
    }
}

/// Common interface for accessing SampleToGroupBox data.
pub trait SampleToGroupBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the grouping type.
    fn grouping_type(&self) -> FourCC;

    /// Returns the grouping type parameter (version >= 1 only).
    fn grouping_type_parameter(&self) -> Option<u32>;

    /// Returns the entry count.
    fn entry_count(&self) -> u32;

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = SampleToGroupEntry> + '_;
}

/// A borrowing view over raw SampleToGroupBox bytes.
#[derive(Clone, Copy)]
pub struct SampleToGroupBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    entries: FixedSizeEntries<'a, 8>,
}

impl<'a> SampleToGroupBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let header_payload = if version >= 1 { 12 } else { 8 };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, header_payload)?;

        let entry_count_offset = fullbox_offset + 4 + header_payload - 4;
        let entry_count = BigEndian::read_u32(&data[entry_count_offset..entry_count_offset + 4]);
        let entries_offset = fullbox_offset + 4 + header_payload;
        let entries = FixedSizeEntries::new(&data[entries_offset..], entry_count)?;

        Ok(Self {
            data,
            fullbox_offset,
            version,
            entries,
        })
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

    /// Returns the entry at the given index.
    pub fn entry(&self, index: usize) -> Option<SampleToGroupEntry> {
        self.entries.get(index).map(SampleToGroupEntry::from_bytes)
    }

}

impl SampleToGroupBox for SampleToGroupBoxView<'_> {
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

    fn grouping_type(&self) -> FourCC {
        let o = self.payload_offset();
        FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn grouping_type_parameter(&self) -> Option<u32> {
        if self.version >= 1 {
            let o = self.payload_offset() + 4;
            Some(BigEndian::read_u32(&self.data[o..o + 4]))
        } else {
            None
        }
    }

    fn entry_count(&self) -> u32 {
        self.entries.count()
    }

    fn entries(&self) -> impl Iterator<Item = SampleToGroupEntry> + '_ {
        self.entries.iter().map(SampleToGroupEntry::from_bytes)
    }
}

impl std::fmt::Debug for SampleToGroupBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleToGroupBoxView")
            .field("version", &self.version())
            .field("grouping_type", &self.grouping_type())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of SampleToGroupBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleToGroupBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Grouping type.
    pub grouping_type: FourCC,
    /// Grouping type parameter (used when version >= 1).
    pub grouping_type_parameter: Option<u32>,
    /// Entries.
    pub entries: Vec<SampleToGroupEntry>,
}

impl SampleToGroupBoxOwned {
    /// Creates a new SampleToGroupBoxOwned.
    pub fn new(grouping_type: FourCC) -> Self {
        Self {
            flags: 0,
            grouping_type,
            grouping_type_parameter: None,
            entries: Vec::new(),
        }
    }

    fn version(&self) -> u8 {
        if self.grouping_type_parameter.is_some() { 1 } else { 0 }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let version_fields = if self.version() >= 1 { 12u64 } else { 8u64 };
        let payload = version_fields + (self.entries.len() * 8) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        let version = self.version();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;
        writer.write_all(&self.grouping_type.0)?;

        if version >= 1 {
            writer.write_u32::<BigEndian>(self.grouping_type_parameter.unwrap_or(0))?;
        }

        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.sample_count)?;
            writer.write_u32::<BigEndian>(entry.group_description_index)?;
        }

        Ok(())
    }
}

impl Default for SampleToGroupBoxOwned {
    fn default() -> Self {
        Self::new(FourCC(*b"roll"))
    }
}

impl SampleToGroupBox for SampleToGroupBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn grouping_type(&self) -> FourCC {
        self.grouping_type
    }

    fn grouping_type_parameter(&self) -> Option<u32> {
        self.grouping_type_parameter
    }

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> impl Iterator<Item = SampleToGroupEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: SampleToGroupBox> From<&T> for SampleToGroupBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            grouping_type: source.grouping_type(),
            grouping_type_parameter: source.grouping_type_parameter(),
            entries: source.entries().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sbgp_v0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&28u32.to_be_bytes()); // size = 8 + 4 + 4 + 4 + 8
        data.extend_from_slice(b"sbgp");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"roll"); // grouping_type
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count

        // Entry: sample_count=10, group_description_index=1
        data.extend_from_slice(&10u32.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());

        data
    }

    #[test]
    fn parse_sbgp_v0() {
        let data = make_sbgp_v0();
        let view = SampleToGroupBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.grouping_type(), FourCC(*b"roll"));
        assert!(view.grouping_type_parameter().is_none());
        assert_eq!(view.entry_count(), 1);

        let entry = view.entry(0).unwrap();
        assert_eq!(entry.sample_count, 10);
        assert_eq!(entry.group_description_index, 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_sbgp_v0();
        let view = SampleToGroupBoxView::new(&data).unwrap();
        let owned = SampleToGroupBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
