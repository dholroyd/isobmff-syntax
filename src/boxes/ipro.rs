//! Item Protection Box (ipro) parsing and serialization.
//!
//! The Item Protection Box contains protection scheme information for items.
//!
//! ```text
//! aligned(8) class ItemProtectionBox
//!    extends FullBox('ipro', version = 0, 0) {
//!    unsigned int(16) protection_count;
//!    for (i=1; i<=protection_count; i++) {
//!       ProtectionSchemeInfoBox protection_information;
//!    }
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemProtectionBox.
pub const BOX_TYPE: BoxCode = BoxCode::IPRO;

/// A typed child of an ItemProtectionBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemProtectionChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for ItemProtectionChild {
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

impl ItemProtectionChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for ItemProtectionChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&ItemProtectionChild> for ItemProtectionChild {
    fn from(source: &ItemProtectionChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing ItemProtectionBox data.
pub trait ItemProtectionBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<ItemProtectionChild>
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

    /// Returns the protection count.
    fn protection_count(&self) -> u16;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw ItemProtectionBox bytes.
#[derive(Clone, Copy)]
pub struct ItemProtectionBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    protection_count: u16,
    children_offset: usize,
}

impl<'a> ItemProtectionBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 2)?;
        let protection_count = BigEndian::read_u16(&data[fullbox_offset + 4..fullbox_offset + 6]);
        let children_offset = fullbox_offset + 6;
        Ok(Self {
            data,
            fullbox_offset,
            protection_count,
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

impl<'a> ItemProtectionBox for ItemProtectionBoxView<'a> {
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

    fn protection_count(&self) -> u16 {
        self.protection_count
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for ItemProtectionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemProtectionBoxView")
            .field("protection_count", &self.protection_count())
            .finish()
    }
}

/// An owned representation of ItemProtectionBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ItemProtectionBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Typed child boxes (ProtectionSchemeInfoBox entries).
    pub children: Vec<ItemProtectionChild>,
}

impl ItemProtectionBoxOwned {
    /// Creates a new ItemProtectionBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let children_size: u64 = self.children.iter().map(|c| c.box_size()).sum();
        let payload = 2 + children_size; // protection_count + children
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
}

impl ItemProtectionBox for ItemProtectionBoxOwned {
    type Child<'a> = &'a ItemProtectionChild;

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

    fn protection_count(&self) -> u16 {
        self.children.len() as u16
    }

    fn children(&self) -> impl Iterator<Item = &ItemProtectionChild> {
        self.children.iter()
    }
}

impl<T: ItemProtectionBox> From<&T> for ItemProtectionBoxOwned {
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

    fn make_ipro() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 2 = 14 bytes
        data.extend_from_slice(&14u32.to_be_bytes());
        data.extend_from_slice(b"ipro");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u16.to_be_bytes()); // protection_count
        data
    }

    #[test]
    fn parse_ipro() {
        let data = make_ipro();
        let view = ItemProtectionBoxView::new(&data).unwrap();

        assert_eq!(view.protection_count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_ipro();
        let view = ItemProtectionBoxView::new(&data).unwrap();
        let owned = ItemProtectionBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
