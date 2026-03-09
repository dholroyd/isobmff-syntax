//! Item Properties Box (iprp) parsing and serialization.
//!
//! The Item Properties Box contains item property containers and associations.
//!
//! ```text
//! aligned(8) class ItemPropertiesBox extends Box('iprp') {
//!    ItemPropertyContainerBox property_container;
//!    ItemPropertyAssociationBox association[];
//! }
//! ```

use crate::boxes::ipco::{self, ItemPropertyContainerBox as _, ItemPropertyContainerBoxOwned, ItemPropertyContainerBoxView};
use crate::boxes::ipma::{self, ItemPropertyAssociationBox as _, ItemPropertyAssociationBoxOwned, ItemPropertyAssociationBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemPropertiesBox.
pub const BOX_TYPE: BoxCode = BoxCode::IPRP;

/// A typed child of an ItemPropertiesBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemPropertiesChild {
    /// An ItemPropertyContainerBox child.
    Ipco(ItemPropertyContainerBoxOwned),
    /// An ItemPropertyAssociationBox child.
    Ipma(ItemPropertyAssociationBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for ItemPropertiesChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Ipco(_) => ipco::BOX_TYPE,
            Self::Ipma(_) => ipma::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Ipco(b) => b.box_size(),
            Self::Ipma(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl ItemPropertiesChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Ipco(b) => b.write_to(writer),
            Self::Ipma(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for ItemPropertiesChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            ipco::BOX_TYPE => match ItemPropertyContainerBoxView::new(raw.data()) {
                Ok(v) => Self::Ipco(ItemPropertyContainerBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            ipma::BOX_TYPE => match ItemPropertyAssociationBoxView::new(raw.data()) {
                Ok(v) => match ItemPropertyAssociationBoxOwned::try_from(&v) {
                    Ok(owned) => Self::Ipma(owned),
                    Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
                },
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&ItemPropertiesChild> for ItemPropertiesChild {
    fn from(source: &ItemPropertiesChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing ItemPropertiesBox data.
pub trait ItemPropertiesBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<ItemPropertiesChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw ItemPropertiesBox bytes.
#[derive(Clone, Copy)]
pub struct ItemPropertiesBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> ItemPropertiesBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 0)?;
        Ok(Self { data, header })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    #[inline]
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.header.header_size as usize..])
    }

    /// Returns the ItemPropertyContainerBox child, if present.
    pub fn ipco(&self) -> Option<ItemPropertyContainerBoxView<'a>> {
        self.children()
            .find_as::<ItemPropertyContainerBoxView>()
            .and_then(Result::ok)
    }

    /// Returns an iterator over ItemPropertyAssociationBox children.
    pub fn ipma_boxes(&self) -> impl Iterator<Item = ItemPropertyAssociationBoxView<'a>> {
        self.children()
            .filter_as::<ItemPropertyAssociationBoxView>()
            .filter_map(Result::ok)
    }
}

impl<'a> ItemPropertiesBox for ItemPropertiesBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for ItemPropertiesBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self.children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("ItemPropertiesBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of ItemPropertiesBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ItemPropertiesBoxOwned {
    /// Typed child boxes.
    pub children: Vec<ItemPropertiesChild>,
}

impl ItemPropertiesBoxOwned {
    /// Creates a new empty ItemPropertiesBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = self.children.iter().map(|c| c.box_size()).sum::<u64>();
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        for child in &self.children {
            child.write_to(writer)?;
        }
        Ok(())
    }

    /// Returns the ItemPropertyContainerBox child, if present.
    pub fn ipco(&self) -> Option<&ItemPropertyContainerBoxOwned> {
        self.children.iter().find_map(|c| match c {
            ItemPropertiesChild::Ipco(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over ItemPropertyAssociationBox children.
    pub fn ipma_boxes(&self) -> impl Iterator<Item = &ItemPropertyAssociationBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertiesChild::Ipma(b) => Some(b),
            _ => None,
        })
    }

    /// Adds an ItemPropertyContainerBox child.
    pub fn add_ipco(&mut self, b: ItemPropertyContainerBoxOwned) {
        self.children.push(ItemPropertiesChild::Ipco(b));
    }

    /// Adds an ItemPropertyAssociationBox child.
    pub fn add_ipma(&mut self, b: ItemPropertyAssociationBoxOwned) {
        self.children.push(ItemPropertiesChild::Ipma(b));
    }
}

impl ItemPropertiesBox for ItemPropertiesBoxOwned {
    type Child<'a> = &'a ItemPropertiesChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &ItemPropertiesChild> {
        self.children.iter()
    }
}

impl<T: ItemPropertiesBox> From<&T> for ItemPropertiesBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_iprp() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"iprp");

        let view = ItemPropertiesBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"iprp");

        let view = ItemPropertiesBoxView::new(&data).unwrap();
        let owned = ItemPropertiesBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
