//! Item Property Association Box (ipma) parsing and serialization.
//!
//! The Item Property Association Box associates items with properties.
//!
//! ```text
//! aligned(8) class ItemPropertyAssociationBox
//!    extends FullBox('ipma', version, flags) {
//!    unsigned int(32) entry_count;
//!    for (i = 0; i < entry_count; i++) {
//!       if (version < 1)
//!          unsigned int(16) item_ID;
//!       else
//!          unsigned int(32) item_ID;
//!       unsigned int(8) association_count;
//!       for (i=0; i<association_count; i++) {
//!          bit(1) essential;
//!          if (flags & 1)
//!             unsigned int(15) property_index;
//!          else
//!             unsigned int(7) property_index;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemPropertyAssociationBox.
pub const BOX_TYPE: BoxCode = BoxCode::IPMA;

/// A property association for a single property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyAssociation {
    /// Whether the property is essential.
    pub essential: bool,
    /// 1-based property index (0 means no property).
    pub property_index: u16,
}

/// Shared interface over an item's property-association entry.
///
/// Implemented by both the borrowing [`ItemPropertyAssociationView`] and by
/// `&ItemPropertyAssociationOwned`, so generic code can iterate either without
/// knowing whether the source is a view or an owned box.
pub trait ItemPropertyAssociation {
    /// Returns the item ID.
    fn item_id(&self) -> u32;

    /// Returns the number of property associations.
    fn association_count(&self) -> usize;

    /// Returns an iterator over the property associations.
    fn associations(&self) -> impl Iterator<Item = PropertyAssociation> + '_;

    /// Materialises an owned copy of this entry.
    fn to_owned(&self) -> ItemPropertyAssociationOwned {
        ItemPropertyAssociationOwned {
            item_id: self.item_id(),
            associations: self.associations().collect(),
        }
    }
}

/// A borrowing view over a single `ipma` entry's raw bytes.
#[derive(Clone, Copy)]
pub struct ItemPropertyAssociationView<'a> {
    item_id: u32,
    associations_data: &'a [u8],
    association_count: usize,
    prop_size: usize,
}

impl<'a> ItemPropertyAssociation for ItemPropertyAssociationView<'a> {
    fn item_id(&self) -> u32 {
        self.item_id
    }

    fn association_count(&self) -> usize {
        self.association_count
    }

    fn associations(&self) -> impl Iterator<Item = PropertyAssociation> + '_ {
        let prop_size = self.prop_size;
        self.associations_data
            .chunks_exact(prop_size)
            .map(move |chunk| {
                if prop_size == 2 {
                    let val = BigEndian::read_u16(chunk);
                    PropertyAssociation {
                        essential: (val >> 15) != 0,
                        property_index: val & 0x7FFF,
                    }
                } else {
                    let val = chunk[0];
                    PropertyAssociation {
                        essential: (val >> 7) != 0,
                        property_index: (val & 0x7F) as u16,
                    }
                }
            })
    }
}

/// An owned `ipma` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemPropertyAssociationOwned {
    /// Item ID.
    pub item_id: u32,
    /// Property associations for this item.
    pub associations: Vec<PropertyAssociation>,
}

impl ItemPropertyAssociation for &ItemPropertyAssociationOwned {
    fn item_id(&self) -> u32 {
        self.item_id
    }

    fn association_count(&self) -> usize {
        self.associations.len()
    }

    fn associations(&self) -> impl Iterator<Item = PropertyAssociation> + '_ {
        self.associations.iter().copied()
    }

    fn to_owned(&self) -> ItemPropertyAssociationOwned {
        (*self).clone()
    }
}

/// Common interface for accessing ItemPropertyAssociationBox data.
pub trait ItemPropertyAssociationBox {
    /// The concrete entry type yielded by [`Self::entries`].
    type Entry<'a>: ItemPropertyAssociation
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

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_;
}

/// A borrowing view over raw ItemPropertyAssociationBox bytes.
#[derive(Clone, Copy)]
pub struct ItemPropertyAssociationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    fl: u32,
    entry_count: u32,
}

impl<'a> ItemPropertyAssociationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let fl = header.flags;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;

        let entry_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);

        Ok(Self {
            data,
            fullbox_offset,
            version,
            fl,
            entry_count,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    fn item_id_size(&self) -> usize {
        if self.version < 1 { 2 } else { 4 }
    }

    fn property_index_size(&self) -> usize {
        if self.fl & 1 != 0 { 2 } else { 1 }
    }

}

impl<'a> ItemPropertyAssociationBox for ItemPropertyAssociationBoxView<'a> {
    type Entry<'b> = ItemPropertyAssociationView<'b> where Self: 'b;

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
        self.fl
    }

    fn entry_count(&self) -> u32 {
        self.entry_count
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        let item_id_size = self.item_id_size();
        let prop_size = self.property_index_size();
        let mut offset = self.fullbox_offset + 8;
        let mut remaining = self.entry_count;
        let mut errored = false;

        std::iter::from_fn(move || {
            if errored || remaining == 0 {
                return None;
            }
            remaining -= 1;

            if offset + item_id_size + 1 > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + item_id_size + 1,
                    found: self.data.len(),
                }));
            }

            let item_id = if item_id_size == 4 {
                BigEndian::read_u32(&self.data[offset..offset + 4])
            } else {
                BigEndian::read_u16(&self.data[offset..offset + 2]) as u32
            };
            offset += item_id_size;

            let association_count = self.data[offset] as usize;
            offset += 1;

            // Validate all associations fit
            let associations_total = match association_count.checked_mul(prop_size) {
                Some(total) => total,
                None => {
                    errored = true;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: usize::MAX,
                        found: self.data.len(),
                    }));
                }
            };
            if offset + associations_total > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + associations_total,
                    found: self.data.len(),
                }));
            }

            let associations_data = &self.data[offset..offset + associations_total];
            offset += associations_total;

            Some(Ok(ItemPropertyAssociationView {
                item_id,
                associations_data,
                association_count,
                prop_size,
            }))
        })
    }
}

impl std::fmt::Debug for ItemPropertyAssociationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemPropertyAssociationBoxView")
            .field("version", &self.version())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of ItemPropertyAssociationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct ItemPropertyAssociationBoxOwned {
    /// Entries.
    pub entries: Vec<ItemPropertyAssociationOwned>,
}

impl ItemPropertyAssociationBoxOwned {
    /// Creates a new empty ItemPropertyAssociationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    fn requires_large_item_ids(&self) -> bool {
        self.entries.iter().any(|e| e.item_id > u16::MAX as u32)
    }

    fn requires_large_property_indices(&self) -> bool {
        self.entries.iter().any(|e| {
            e.associations.iter().any(|a| a.property_index > 127)
        })
    }

    fn version(&self) -> u8 {
        if self.requires_large_item_ids() { 1 } else { 0 }
    }

    fn compute_flags(&self) -> u32 {
        if self.requires_large_property_indices() { 1 } else { 0 }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let item_id_size = if self.requires_large_item_ids() { 4 } else { 2 };
        let prop_size = if self.requires_large_property_indices() { 2 } else { 1 };

        let mut payload: usize = 4; // entry_count
        for entry in &self.entries {
            payload += item_id_size + 1 + entry.associations.len() * prop_size;
        }
        let payload = payload as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.version();
        let fl = self.compute_flags();
        let size = self.serialized_size();
        let large_item_ids = self.requires_large_item_ids();
        let large_prop_indices = self.requires_large_property_indices();
        write_fullbox_header(writer, size, BOX_TYPE, version, fl)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            if large_item_ids {
                writer.write_u32::<BigEndian>(entry.item_id)?;
            } else {
                writer.write_u16::<BigEndian>(entry.item_id as u16)?;
            }

            writer.write_u8(entry.associations.len() as u8)?;

            for assoc in &entry.associations {
                if large_prop_indices {
                    let val = ((assoc.essential as u16) << 15) | (assoc.property_index & 0x7FFF);
                    writer.write_u16::<BigEndian>(val)?;
                } else {
                    let val = ((assoc.essential as u8) << 7) | ((assoc.property_index & 0x7F) as u8);
                    writer.write_u8(val)?;
                }
            }
        }

        Ok(())
    }
}


impl ItemPropertyAssociationBox for ItemPropertyAssociationBoxOwned {
    type Entry<'a> = &'a ItemPropertyAssociationOwned where Self: 'a;

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
        self.compute_flags()
    }

    fn entry_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        self.entries.iter().map(Ok)
    }
}

impl TryFrom<&ItemPropertyAssociationBoxView<'_>> for ItemPropertyAssociationBoxOwned {
    type Error = ParseError;

    fn try_from(source: &ItemPropertyAssociationBoxView<'_>) -> Result<Self, Self::Error> {
        let entries = ItemPropertyAssociationBox::entries(source)
            .map(|res| res.map(|v| ItemPropertyAssociation::to_owned(&v)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ipma_v0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // size = 8 + 4 + 4 + 2 + 1 + 1
        data.extend_from_slice(b"ipma");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        data.extend_from_slice(&1u16.to_be_bytes()); // item_id
        data.push(1); // association_count
        data.push(0x81); // essential=1, property_index=1
        data
    }

    #[test]
    fn parse_ipma_v0() {
        let data = make_ipma_v0();
        let view = ItemPropertyAssociationBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.entry_count(), 1);

        let entries: Vec<_> =
            ItemPropertyAssociationBox::entries(&view).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].item_id(), 1);
        assert_eq!(entries[0].association_count(), 1);
        let associations: Vec<_> = entries[0].associations().collect();
        assert!(associations[0].essential);
        assert_eq!(associations[0].property_index, 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_ipma_v0();
        let view = ItemPropertyAssociationBoxView::new(&data).unwrap();
        let owned = ItemPropertyAssociationBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
