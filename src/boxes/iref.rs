//! Item Reference Box (iref) parsing and serialization.
//!
//! The Item Reference Box contains item references.
//!
//! ```text
//! aligned(8) class SingleItemTypeReferenceBox(referenceType)
//!    extends Box(referenceType) {
//!    unsigned int(16) from_item_ID;
//!    unsigned int(16) reference_count;
//!    for (j=0; j<reference_count; j++) {
//!       unsigned int(16) to_item_ID;
//!    }
//! }
//!
//! aligned(8) class SingleItemTypeReferenceBoxLarge(referenceType)
//!    extends Box(referenceType) {
//!    unsigned int(32) from_item_ID;
//!    unsigned int(16) reference_count;
//!    for (j=0; j<reference_count; j++) {
//!       unsigned int(32) to_item_ID;
//!    }
//! }
//!
//! aligned(8) class ItemReferenceBox
//!    extends FullBox('iref', version, 0) {
//!    if (version==0) {
//!       SingleItemTypeReferenceBox references[];
//!    } else if (version==1) {
//!       SingleItemTypeReferenceBoxLarge references[];
//!    }
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemReferenceBox.
pub const BOX_TYPE: BoxCode = BoxCode::IREF;

/// A typed child of an ItemReferenceBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemReferenceChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for ItemReferenceChild {
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

impl ItemReferenceChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for ItemReferenceChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&ItemReferenceChild> for ItemReferenceChild {
    fn from(source: &ItemReferenceChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing ItemReferenceBox data.
pub trait ItemReferenceBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<ItemReferenceChild>
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

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw ItemReferenceBox bytes.
#[derive(Clone, Copy)]
pub struct ItemReferenceBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
}

impl<'a> ItemReferenceBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        let version = header.version;
        Ok(Self { data, fullbox_offset, version })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.fullbox_offset + 4..])
    }
}

impl<'a> ItemReferenceBox for ItemReferenceBoxView<'a> {
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

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for ItemReferenceBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("ItemReferenceBoxView")
            .field("version", &self.version())
            .field("reference_types", &child_types)
            .finish()
    }
}

/// An owned representation of ItemReferenceBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ItemReferenceBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Version (determines item ID size: 0=16-bit, 1=32-bit).
    pub version: u8,
    /// Typed child boxes.
    pub children: Vec<ItemReferenceChild>,
}

impl ItemReferenceBoxOwned {
    /// Creates a new empty ItemReferenceBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = self.children.iter().map(|c| c.box_size()).sum::<u64>();
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u8(self.version)?;
        writer.write_u24::<BigEndian>(self.flags)?;

        for child in &self.children {
            child.write_to(writer)?;
        }

        Ok(())
    }
}

impl ItemReferenceBox for ItemReferenceBoxOwned {
    type Child<'a> = &'a ItemReferenceChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn children(&self) -> impl Iterator<Item = &ItemReferenceChild> {
        self.children.iter()
    }
}

impl<T: ItemReferenceBox> From<&T> for ItemReferenceBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            version: source.version(),
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_iref_empty() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"iref");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_iref_empty() {
        let data = make_iref_empty();
        let view = ItemReferenceBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_iref_empty();
        let view = ItemReferenceBoxView::new(&data).unwrap();
        let owned = ItemReferenceBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
