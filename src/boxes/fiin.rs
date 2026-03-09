//! FD Item Information Box (fiin) parsing and serialization.
//!
//! The FD Item Information Box is a container for file delivery item information.
//!
//! ```text
//! aligned(8) class FDItemInformationBox
//!    extends FullBox('fiin', version = 0, 0) {
//!    unsigned int(16) entry_count;
//!    PartitionEntry partition_entries[ entry_count ];
//!    FDSessionGroupBox session_info; //optional
//!    GroupIdToNameBox group_id_to_name; //optional
//! }
//! ```

use crate::boxes::gitn::{
    self, GroupIdToNameBox as _, GroupIdToNameBoxOwned, GroupIdToNameBoxView,
};
use crate::boxes::paen::{
    self, PartitionEntryBox as _, PartitionEntryBoxOwned, PartitionEntryBoxView,
};
use crate::boxes::segr::{
    self, FDSessionGroupBox as _, FDSessionGroupBoxOwned, FDSessionGroupBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for FDItemInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::FIIN;

/// A typed child of an FDItemInformationBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FDItemInformationChild {
    /// A PartitionEntryBox child.
    Paen(PartitionEntryBoxOwned),
    /// An FDSessionGroupBox child.
    Segr(FDSessionGroupBoxOwned),
    /// A GroupIdToNameBox child.
    Gitn(GroupIdToNameBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for FDItemInformationChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Paen(_) => paen::BOX_TYPE,
            Self::Segr(_) => segr::BOX_TYPE,
            Self::Gitn(_) => gitn::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Paen(b) => b.box_size(),
            Self::Segr(b) => b.box_size(),
            Self::Gitn(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl FDItemInformationChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Paen(b) => b.write_to(writer),
            Self::Segr(b) => b.write_to(writer),
            Self::Gitn(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for FDItemInformationChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            paen::BOX_TYPE => match PartitionEntryBoxView::new(raw.data()) {
                Ok(v) => Self::Paen(PartitionEntryBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            segr::BOX_TYPE => match FDSessionGroupBoxView::new(raw.data()) {
                Ok(v) => Self::Segr(FDSessionGroupBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            gitn::BOX_TYPE => match GroupIdToNameBoxView::new(raw.data()) {
                Ok(v) => match GroupIdToNameBoxOwned::try_from(&v) {
                    Ok(owned) => Self::Gitn(owned),
                    Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
                },
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&FDItemInformationChild> for FDItemInformationChild {
    fn from(source: &FDItemInformationChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing FDItemInformationBox data.
pub trait FDItemInformationBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<FDItemInformationChild>
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

    /// Returns the entry count.
    fn entry_count(&self) -> u16;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw FDItemInformationBox bytes.
#[derive(Clone, Copy)]
pub struct FDItemInformationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entry_count: u16,
}

impl<'a> FDItemInformationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 2)?;

        let entry_count = BigEndian::read_u16(&data[fullbox_offset + 4..fullbox_offset + 6]);

        Ok(Self {
            data,
            fullbox_offset,
            entry_count,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.fullbox_offset + 6..])
    }

    /// Returns an iterator over PartitionEntryBox children.
    pub fn paen_iter(&self) -> impl Iterator<Item = PartitionEntryBoxView<'a>> {
        self.children()
            .filter_as::<PartitionEntryBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the first FDSessionGroupBox child, if present.
    pub fn segr(&self) -> Option<FDSessionGroupBoxView<'a>> {
        self.children()
            .find_as::<FDSessionGroupBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first GroupIdToNameBox child, if present.
    pub fn gitn(&self) -> Option<GroupIdToNameBoxView<'a>> {
        self.children()
            .find_as::<GroupIdToNameBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> FDItemInformationBox for FDItemInformationBoxView<'a> {
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

    fn entry_count(&self) -> u16 {
        self.entry_count
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for FDItemInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FDItemInformationBoxView")
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of FDItemInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct FDItemInformationBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Typed child boxes.
    pub children: Vec<FDItemInformationChild>,
}

impl FDItemInformationBoxOwned {
    /// Creates a new FDItemInformationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 2u64 + self.children.iter().map(|c| c.box_size()).sum::<u64>();
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u16::<BigEndian>(self.children.len() as u16)?;

        for child in &self.children {
            child.write_to(writer)?;
        }

        Ok(())
    }

    /// Returns an iterator over PartitionEntryBox children.
    pub fn paen_iter(&self) -> impl Iterator<Item = &PartitionEntryBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            FDItemInformationChild::Paen(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first FDSessionGroupBox child, if present.
    pub fn segr(&self) -> Option<&FDSessionGroupBoxOwned> {
        self.children.iter().find_map(|c| match c {
            FDItemInformationChild::Segr(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first GroupIdToNameBox child, if present.
    pub fn gitn(&self) -> Option<&GroupIdToNameBoxOwned> {
        self.children.iter().find_map(|c| match c {
            FDItemInformationChild::Gitn(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a PartitionEntryBox child.
    pub fn add_paen(&mut self, b: PartitionEntryBoxOwned) {
        self.children.push(FDItemInformationChild::Paen(b));
    }

    /// Adds an FDSessionGroupBox child.
    pub fn add_segr(&mut self, b: FDSessionGroupBoxOwned) {
        self.children.push(FDItemInformationChild::Segr(b));
    }

    /// Adds a GroupIdToNameBox child.
    pub fn add_gitn(&mut self, b: GroupIdToNameBoxOwned) {
        self.children.push(FDItemInformationChild::Gitn(b));
    }
}


impl FDItemInformationBox for FDItemInformationBoxOwned {
    type Child<'a> = &'a FDItemInformationChild;

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

    fn entry_count(&self) -> u16 {
        self.children.len() as u16
    }

    fn children(&self) -> impl Iterator<Item = &FDItemInformationChild> {
        self.children.iter()
    }
}

impl<T: FDItemInformationBox> From<&T> for FDItemInformationBoxOwned {
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

    fn make_fiin() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 2 = 14 bytes
        data.extend_from_slice(&14u32.to_be_bytes());
        data.extend_from_slice(b"fiin");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u16.to_be_bytes()); // entry_count
        data
    }

    #[test]
    fn parse_fiin() {
        let data = make_fiin();
        let view = FDItemInformationBoxView::new(&data).unwrap();

        assert_eq!(view.entry_count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_fiin();
        let view = FDItemInformationBoxView::new(&data).unwrap();
        let owned = FDItemInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
