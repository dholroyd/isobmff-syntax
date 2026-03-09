//! Data Reference Box (dref) parsing and serialization.
//!
//! The Data Reference Box declares the location of media data within the file.
//!
//! ```text
//! aligned(8) class DataReferenceBox
//!    extends FullBox('dref', version = 0, 0) {
//!    unsigned int(32) entry_count;
//!    for (i=1; i <= entry_count; i++) {
//!       DataEntryBox(entry_version, entry_flags) data_entry;
//!    }
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for DataReferenceBox.
pub const BOX_TYPE: BoxCode = BoxCode::DREF;

/// Data entry flags.
pub mod flags {
    /// Data is in the same file as the movie box.
    pub const SELF_CONTAINED: u32 = 0x000001;
}

/// A typed child of a DataReferenceBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataReferenceChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for DataReferenceChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Other(o) => o.box_size(),
        }
    }
}

impl DataReferenceChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for DataReferenceChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&DataReferenceChild> for DataReferenceChild {
    fn from(source: &DataReferenceChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing DataReferenceBox data.
pub trait DataReferenceBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<DataReferenceChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the number of data entries.
    fn entry_count(&self) -> u32;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw DataReferenceBox bytes.
#[derive(Clone, Copy)]
pub struct DataReferenceBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entry_count: u32,
    children_offset: usize,
}

impl<'a> DataReferenceBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;

        let entry_count_offset = fullbox_offset + 4;
        let entry_count = BigEndian::read_u32(&data[entry_count_offset..entry_count_offset + 4]);
        let children_offset = entry_count_offset + 4;

        Ok(Self {
            data,
            fullbox_offset,
            entry_count,
            children_offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.children_offset..])
    }
}

impl<'a> DataReferenceBox for DataReferenceBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

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
        self.entry_count
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for DataReferenceBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataReferenceBoxView")
            .field("box_size", &self.box_size())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of DataReferenceBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataReferenceBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Typed child boxes.
    pub children: Vec<DataReferenceChild>,
}

impl DataReferenceBoxOwned {
    /// Creates a new DataReferenceBoxOwned with a self-contained URL entry.
    pub fn new_self_contained() -> Self {
        // Create a minimal 'url ' entry with self-contained flag
        let mut entry = Vec::new();
        entry.extend_from_slice(&12u32.to_be_bytes()); // size
        entry.extend_from_slice(b"url "); // type
        entry.push(0); // version
        entry.extend_from_slice(&[0, 0, 1]); // flags = self-contained

        Self {
            flags: 0,
            children: vec![DataReferenceChild::Other(
                OpaqueBoxOwned::new(entry).expect("valid url box"),
            )],
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let children_size: u64 = self.children.iter().map(|c| c.box_size()).sum();
        let payload = 4 + children_size; // entry_count + children
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.children.len() as u32)?;
        for child in &self.children {
            child.write_to(writer)?;
        }
        Ok(())
    }
}

impl Default for DataReferenceBoxOwned {
    fn default() -> Self {
        Self::new_self_contained()
    }
}

impl DataReferenceBox for DataReferenceBoxOwned {
    type Child<'a> = &'a DataReferenceChild;

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
        self.children.len() as u32
    }

    fn children(&self) -> impl Iterator<Item = &DataReferenceChild> {
        self.children.iter()
    }
}

impl<T: DataReferenceBox> From<&T> for DataReferenceBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dref() -> Vec<u8> {
        let mut data = Vec::new();
        // dref header
        data.extend_from_slice(&28u32.to_be_bytes()); // size (8 + 4 + 4 + 12)
        data.extend_from_slice(b"dref");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count

        // url entry
        data.extend_from_slice(&12u32.to_be_bytes()); // size
        data.extend_from_slice(b"url ");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 1]); // flags = self-contained

        data
    }

    #[test]
    fn parse_dref() {
        let data = make_dref();
        let view = DataReferenceBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 1);
        let children: Vec<_> = view.children().collect();
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn roundtrip() {
        let data = make_dref();
        let view = DataReferenceBoxView::new(&data).unwrap();
        let owned = DataReferenceBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn default_self_contained() {
        let dref = DataReferenceBoxOwned::new_self_contained();
        assert_eq!(dref.entry_count(), 1);
    }
}
