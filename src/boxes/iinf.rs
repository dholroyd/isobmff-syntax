//! Item Information Box (iinf) parsing and serialization.
//!
//! The Item Information Box provides extra information about items.
//!
//! ```text
//! aligned(8) class ItemInfoBox
//!    extends FullBox('iinf', version, 0) {
//!    if (version == 0) {
//!       unsigned int(16) entry_count;
//!    } else {
//!       unsigned int(32) entry_count;
//!    }
//!    ItemInfoEntry[ entry_count ] item_infos;
//! }
//! ```

use crate::boxes::infe::{
    self, ItemInfoEntryBox as _, ItemInfoEntryBoxOwned, ItemInfoEntryBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::IINF;

/// A typed child of an ItemInfoBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemInfoChild {
    /// An ItemInfoEntryBox child.
    Infe(ItemInfoEntryBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for ItemInfoChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Infe(_) => infe::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Infe(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl ItemInfoChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Infe(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for ItemInfoChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            infe::BOX_TYPE => match ItemInfoEntryBoxView::new(raw.data()) {
                Ok(v) => Self::Infe(ItemInfoEntryBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&ItemInfoChild> for ItemInfoChild {
    fn from(source: &ItemInfoChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing ItemInfoBox data.
pub trait ItemInfoBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<ItemInfoChild>
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
    fn entry_count(&self) -> u32;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw ItemInfoBox bytes.
#[derive(Clone, Copy)]
pub struct ItemInfoBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    entry_count: u32,
    children_offset: usize,
}

impl<'a> ItemInfoBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let entry_count_size = if version == 0 { 2 } else { 4 };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, entry_count_size)?;

        let entry_count_offset = fullbox_offset + 4;
        let entry_count = if version == 0 {
            BigEndian::read_u16(&data[entry_count_offset..entry_count_offset + 2]) as u32
        } else {
            BigEndian::read_u32(&data[entry_count_offset..entry_count_offset + 4])
        };

        let children_offset = fullbox_offset + 4 + entry_count_size;

        Ok(Self {
            data,
            fullbox_offset,
            version,
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

    /// Returns all infe child boxes.
    pub fn infe_boxes(&self) -> Vec<RawBox<'a>> {
        self.children().filter_by_type(BoxCode::INFE)
    }

    /// Returns an iterator over ItemInfoEntryBox children.
    pub fn infe_iter(&self) -> impl Iterator<Item = ItemInfoEntryBoxView<'a>> {
        self.children()
            .filter_as::<ItemInfoEntryBoxView>()
            .filter_map(Result::ok)
    }
}

impl<'a> ItemInfoBox for ItemInfoBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

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

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for ItemInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemInfoBoxView")
            .field("version", &self.version())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of ItemInfoBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ItemInfoBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Typed child boxes (infe boxes).
    pub children: Vec<ItemInfoChild>,
}

impl ItemInfoBoxOwned {
    /// Creates a new empty ItemInfoBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    fn requires_v1(&self) -> bool {
        self.children.len() > u16::MAX as usize
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let entry_count_size: u64 = if self.requires_v1() { 4 } else { 2 };
        let children_size: u64 = self.children.iter().map(|c| c.box_size()).sum();
        let payload = entry_count_size + children_size;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let v1 = self.requires_v1();
        let size = self.serialized_size();
        let version = if v1 { 1 } else { 0 };
        let entry_count = self.children.len() as u32;
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;

        if v1 {
            writer.write_u32::<BigEndian>(entry_count)?;
        } else {
            writer.write_u16::<BigEndian>(entry_count as u16)?;
        }

        for child in &self.children {
            child.write_to(writer)?;
        }

        Ok(())
    }

    /// Returns an iterator over ItemInfoEntryBox children.
    pub fn infe_iter(&self) -> impl Iterator<Item = &ItemInfoEntryBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemInfoChild::Infe(b) => Some(b),
            _ => None,
        })
    }

    /// Adds an ItemInfoEntryBox child.
    pub fn add_infe(&mut self, b: ItemInfoEntryBoxOwned) {
        self.children.push(ItemInfoChild::Infe(b));
    }
}

impl ItemInfoBox for ItemInfoBoxOwned {
    type Child<'a> = &'a ItemInfoChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        if self.requires_v1() { 1 } else { 0 }
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn entry_count(&self) -> u32 {
        self.children.len() as u32
    }

    fn children(&self) -> impl Iterator<Item = &ItemInfoChild> {
        self.children.iter()
    }
}

impl<T: ItemInfoBox> From<&T> for ItemInfoBoxOwned {
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

    fn make_iinf_v0_empty() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&14u32.to_be_bytes()); // 8 + 4 + 2
        data.extend_from_slice(b"iinf");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u16.to_be_bytes()); // entry_count
        data
    }

    #[test]
    fn parse_iinf_v0_empty() {
        let data = make_iinf_v0_empty();
        let view = ItemInfoBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.entry_count(), 0);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_iinf_v0_empty();
        let view = ItemInfoBoxView::new(&data).unwrap();
        let owned = ItemInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
