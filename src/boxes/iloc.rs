//! Item Location Box (iloc) parsing and serialization.
//!
//! The Item Location Box provides a directory of resources in the file.
//!
//! ```text
//! aligned(8) class ItemLocationBox extends FullBox('iloc', version, 0) {
//!    unsigned int(4) offset_size;
//!    unsigned int(4) length_size;
//!    unsigned int(4) base_offset_size;
//!    if ((version == 1) || (version == 2)) {
//!       unsigned int(4) index_size;
//!    } else {
//!       unsigned int(4) reserved;
//!    }
//!    if (version < 2) {
//!       unsigned int(16) item_count;
//!    } else if (version == 2) {
//!       unsigned int(32) item_count;
//!    }
//!    for (i=0; i<item_count; i++) {
//!       if (version < 2) {
//!          unsigned int(16) item_ID;
//!       } else if (version == 2) {
//!          unsigned int(32) item_ID;
//!       }
//!       if ((version == 1) || (version == 2)) {
//!          unsigned int(12) reserved = 0;
//!          unsigned int(4) construction_method;
//!       }
//!       unsigned int(16) data_reference_index;
//!       unsigned int(base_offset_size*8) base_offset;
//!       unsigned int(16) extent_count;
//!       for (j=0; j<extent_count; j++) {
//!          if (((version == 1) || (version == 2)) && (index_size > 0)) {
//!             unsigned int(index_size*8) item_reference_index;
//!          }
//!          unsigned int(offset_size*8) extent_offset;
//!          unsigned int(length_size*8) extent_length;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemLocationBox.
pub const BOX_TYPE: BoxCode = BoxCode::ILOC;

/// A single extent within an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemExtent {
    /// Extent index (version >= 1 only, if index_size > 0).
    pub extent_index: u64,
    /// Offset of the extent.
    pub extent_offset: u64,
    /// Length of the extent.
    pub extent_length: u64,
}

/// Shared interface over an item's location information.
///
/// Implemented by both the borrowing [`ItemLocationView`] and by
/// `&ItemLocationOwned`.
pub trait ItemLocation {
    /// Returns the item ID.
    fn item_id(&self) -> u32;

    /// Returns the construction method (version >= 1; always 0 for version 0).
    fn construction_method(&self) -> u8;

    /// Returns the data reference index.
    fn data_reference_index(&self) -> u16;

    /// Returns the base offset.
    fn base_offset(&self) -> u64;

    /// Returns the number of extents.
    fn extent_count(&self) -> usize;

    /// Returns an iterator over the extents.
    fn extents(&self) -> impl Iterator<Item = ItemExtent> + '_;

    /// Materialises an owned copy of this item.
    fn to_owned(&self) -> ItemLocationOwned {
        ItemLocationOwned {
            item_id: self.item_id(),
            construction_method: self.construction_method(),
            data_reference_index: self.data_reference_index(),
            base_offset: self.base_offset(),
            extents: self.extents().collect(),
        }
    }
}

/// A borrowing view over a single `iloc` item's raw bytes.
#[derive(Clone, Copy)]
pub struct ItemLocationView<'a> {
    item_id: u32,
    construction_method: u8,
    data_reference_index: u16,
    base_offset: u64,
    extents_data: &'a [u8],
    extent_count: usize,
    offset_size: u8,
    length_size: u8,
    index_size: u8,
    version: u8,
}

impl<'a> ItemLocation for ItemLocationView<'a> {
    fn item_id(&self) -> u32 {
        self.item_id
    }

    fn construction_method(&self) -> u8 {
        self.construction_method
    }

    fn data_reference_index(&self) -> u16 {
        self.data_reference_index
    }

    fn base_offset(&self) -> u64 {
        self.base_offset
    }

    fn extent_count(&self) -> usize {
        self.extent_count
    }

    fn extents(&self) -> impl Iterator<Item = ItemExtent> + '_ {
        let offset_size = self.offset_size;
        let length_size = self.length_size;
        let index_size = self.index_size;
        let use_index = self.version >= 1 && index_size > 0;
        let stride = offset_size as usize
            + length_size as usize
            + if use_index { index_size as usize } else { 0 };

        // All size fields may legitimately be zero, in which case each extent
        // occupies no bytes and both its offset and length are implicitly zero.
        // `chunks_exact` cannot express a zero stride, so yield the declared
        // number of zero extents separately and give it an empty slice to walk.
        let implicit_extents = if stride == 0 { self.extent_count } else { 0 };
        let (extents_data, stride) =
            if stride == 0 { (&self.extents_data[..0], 1) } else { (self.extents_data, stride) };

        extents_data
            .chunks_exact(stride)
            .map(move |chunk| {
                let mut pos = 0;
                let extent_index = if use_index {
                    let v = read_sized(&chunk[pos..], index_size);
                    pos += index_size as usize;
                    v
                } else {
                    0
                };
                let extent_offset = read_sized(&chunk[pos..], offset_size);
                pos += offset_size as usize;
                let extent_length = read_sized(&chunk[pos..], length_size);
                let _ = pos;
                ItemExtent {
                    extent_index,
                    extent_offset,
                    extent_length,
                }
            })
            .chain(std::iter::repeat_n(
                ItemExtent { extent_index: 0, extent_offset: 0, extent_length: 0 },
                implicit_extents,
            ))
    }
}

/// An owned `iloc` item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemLocationOwned {
    /// Item ID.
    pub item_id: u32,
    /// Construction method (version >= 1).
    pub construction_method: u8,
    /// Data reference index.
    pub data_reference_index: u16,
    /// Base offset.
    pub base_offset: u64,
    /// Extents.
    pub extents: Vec<ItemExtent>,
}

impl ItemLocation for &ItemLocationOwned {
    fn item_id(&self) -> u32 {
        self.item_id
    }

    fn construction_method(&self) -> u8 {
        self.construction_method
    }

    fn data_reference_index(&self) -> u16 {
        self.data_reference_index
    }

    fn base_offset(&self) -> u64 {
        self.base_offset
    }

    fn extent_count(&self) -> usize {
        self.extents.len()
    }

    fn extents(&self) -> impl Iterator<Item = ItemExtent> + '_ {
        self.extents.iter().copied()
    }

    fn to_owned(&self) -> ItemLocationOwned {
        (*self).clone()
    }
}

#[inline]
fn read_sized(data: &[u8], size: u8) -> u64 {
    match size {
        0 => 0,
        1 => data[0] as u64,
        2 => BigEndian::read_u16(&data[..2]) as u64,
        4 => BigEndian::read_u32(&data[..4]) as u64,
        8 => BigEndian::read_u64(&data[..8]),
        _ => 0,
    }
}

/// Common interface for accessing ItemLocationBox data.
pub trait ItemLocationBox {
    /// The concrete item type yielded by [`Self::items`].
    type Item<'a>: ItemLocation
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

    /// Returns the offset size in bytes.
    fn offset_size(&self) -> u8;

    /// Returns the length size in bytes.
    fn length_size(&self) -> u8;

    /// Returns the base offset size in bytes.
    fn base_offset_size(&self) -> u8;

    /// Returns the index size in bytes (version >= 1).
    fn index_size(&self) -> u8;

    /// Returns the item count.
    fn item_count(&self) -> u32;

    /// Returns an iterator over all items.
    fn items(&self) -> impl Iterator<Item = Result<Self::Item<'_>, ParseError>> + '_;
}

/// A borrowing view over raw ItemLocationBox bytes.
#[derive(Clone, Copy)]
pub struct ItemLocationBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    offset_size: u8,
    length_size: u8,
    base_offset_size: u8,
    index_size: u8,
    item_count: u32,
    items_offset: usize,
}

impl<'a> ItemLocationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;

        // Parse sizes byte
        let item_count_field_size: usize = if version < 2 { 2 } else { 4 };
        let min_payload = 2 + item_count_field_size;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, min_payload)?;

        let sizes_offset = fullbox_offset + 4;
        let sizes1 = data[sizes_offset];
        let sizes2 = data[sizes_offset + 1];
        let offset_size = (sizes1 >> 4) & 0xF;
        let length_size = sizes1 & 0xF;
        let base_offset_size = (sizes2 >> 4) & 0xF;
        let index_size = if version >= 1 { sizes2 & 0xF } else { 0 };

        // Validate field sizes are valid per spec (0, 1, 2, 4, or 8)
        if !matches!(offset_size, 0 | 1 | 2 | 4 | 8) {
            return Err(ParseError::InvalidFieldSize { field: "offset_size", size: offset_size });
        }
        if !matches!(length_size, 0 | 1 | 2 | 4 | 8) {
            return Err(ParseError::InvalidFieldSize { field: "length_size", size: length_size });
        }
        if !matches!(base_offset_size, 0 | 1 | 2 | 4 | 8) {
            return Err(ParseError::InvalidFieldSize { field: "base_offset_size", size: base_offset_size });
        }
        if !matches!(index_size, 0 | 1 | 2 | 4 | 8) {
            return Err(ParseError::InvalidFieldSize { field: "index_size", size: index_size });
        }

        // Parse item count
        let item_count_offset = sizes_offset + 2;
        let (item_count, items_offset) = if version < 2 {
            let count = BigEndian::read_u16(&data[item_count_offset..item_count_offset + 2]) as u32;
            (count, item_count_offset + 2)
        } else {
            let count = BigEndian::read_u32(&data[item_count_offset..item_count_offset + 4]);
            (count, item_count_offset + 4)
        };

        Ok(Self {
            data,
            fullbox_offset,
            version,
            offset_size,
            length_size,
            base_offset_size,
            index_size,
            item_count,
            items_offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    fn read_sized_value(&self, offset: usize, size: u8) -> u64 {
        match size {
            0 => 0,
            1 => self.data[offset] as u64,
            2 => BigEndian::read_u16(&self.data[offset..offset + 2]) as u64,
            4 => BigEndian::read_u32(&self.data[offset..offset + 4]) as u64,
            8 => BigEndian::read_u64(&self.data[offset..offset + 8]),
            _ => 0,
        }
    }

}

impl<'a> ItemLocationBox for ItemLocationBoxView<'a> {
    type Item<'b> = ItemLocationView<'b> where Self: 'b;

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

    fn offset_size(&self) -> u8 {
        self.offset_size
    }

    fn length_size(&self) -> u8 {
        self.length_size
    }

    fn base_offset_size(&self) -> u8 {
        self.base_offset_size
    }

    fn index_size(&self) -> u8 {
        self.index_size
    }

    fn item_count(&self) -> u32 {
        self.item_count
    }

    fn items(&self) -> impl Iterator<Item = Result<Self::Item<'_>, ParseError>> + '_ {
        let item_id_size: usize = if self.version < 2 { 2 } else { 4 };
        let construction_method_size: usize = if self.version >= 1 { 2 } else { 0 };
        let extent_index_size: usize =
            if self.version >= 1 && self.index_size > 0 { self.index_size as usize } else { 0 };
        let per_extent_size =
            extent_index_size + self.offset_size as usize + self.length_size as usize;
        let per_item_fixed =
            item_id_size + construction_method_size + 2 + self.base_offset_size as usize + 2;

        let mut offset = self.items_offset;
        let mut remaining = self.item_count;
        let mut errored = false;
        // Extents whose size fields are all zero occupy no input bytes, so the
        // per-item `extents_total` check cannot bound how many are declared.
        // Allow the box as a whole to describe no more such extents than it has
        // bytes, which keeps what `extents` materialises linear in the box size.
        let mut implicit_extent_budget = self.data.len();

        std::iter::from_fn(move || {
            if errored || remaining == 0 {
                return None;
            }
            remaining -= 1;

            // Validate fixed fields for this item
            if offset + per_item_fixed > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + per_item_fixed,
                    found: self.data.len(),
                }));
            }

            // Read item_id
            let item_id = if self.version < 2 {
                BigEndian::read_u16(&self.data[offset..offset + 2]) as u32
            } else {
                BigEndian::read_u32(&self.data[offset..offset + 4])
            };
            offset += item_id_size;

            // Read construction_method (version >= 1)
            let construction_method = if self.version >= 1 {
                let cm = BigEndian::read_u16(&self.data[offset..offset + 2]);
                offset += 2;
                (cm & 0xF) as u8
            } else {
                0
            };

            // Read data_reference_index
            let data_reference_index = BigEndian::read_u16(&self.data[offset..offset + 2]);
            offset += 2;

            // Read base_offset
            let base_offset = self.read_sized_value(offset, self.base_offset_size);
            offset += self.base_offset_size as usize;

            // Read extent_count
            let extent_count = BigEndian::read_u16(&self.data[offset..offset + 2]) as usize;
            offset += 2;

            if per_extent_size == 0 {
                if extent_count > implicit_extent_budget {
                    errored = true;
                    return Some(Err(ParseError::InvalidEntryCount {
                        count: extent_count as u32,
                        max_possible: implicit_extent_budget as u32,
                    }));
                }
                implicit_extent_budget -= extent_count;
            }

            // Validate all extents fit
            let extents_total = match extent_count.checked_mul(per_extent_size) {
                Some(total) => total,
                None => {
                    errored = true;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: usize::MAX,
                        found: self.data.len(),
                    }));
                }
            };
            if offset + extents_total > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + extents_total,
                    found: self.data.len(),
                }));
            }

            let extents_data = &self.data[offset..offset + extents_total];
            offset += extents_total;

            Some(Ok(ItemLocationView {
                item_id,
                construction_method,
                data_reference_index,
                base_offset,
                extents_data,
                extent_count,
                offset_size: self.offset_size,
                length_size: self.length_size,
                index_size: self.index_size,
                version: self.version,
            }))
        })
    }
}

impl std::fmt::Debug for ItemLocationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemLocationBoxView")
            .field("version", &self.version())
            .field("item_count", &self.item_count())
            .finish()
    }
}

/// An owned representation of ItemLocationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemLocationBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Offset size in bytes (0, 1, 2, 4, or 8).
    pub offset_size: u8,
    /// Length size in bytes.
    pub length_size: u8,
    /// Base offset size in bytes.
    pub base_offset_size: u8,
    /// Index size in bytes (version >= 1).
    pub index_size: u8,
    /// Item locations.
    pub items: Vec<ItemLocationOwned>,
}

impl ItemLocationBoxOwned {
    /// Creates a new empty ItemLocationBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    fn version(&self) -> u8 {
        // Use v1 if any item has construction_method set
        // Use v2 if any item_id > 65535
        let needs_v2 = self.items.iter().any(|i| i.item_id > u16::MAX as u32);
        let needs_v1 = self.items.iter().any(|i| i.construction_method > 0) || self.index_size > 0;

        if needs_v2 {
            2
        } else if needs_v1 {
            1
        } else {
            0
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let version = self.version();
        let item_id_size = if version < 2 { 2 } else { 4 };
        let item_count_size = if version < 2 { 2 } else { 4 };

        let mut payload: usize = 2 + item_count_size; // sizes + item_count

        for item in &self.items {
            payload += item_id_size; // item_id
            if version >= 1 {
                payload += 2; // construction_method
            }
            payload += 2; // data_reference_index
            payload += self.base_offset_size as usize;
            payload += 2; // extent_count

            for _ in &item.extents {
                if version >= 1 && self.index_size > 0 {
                    payload += self.index_size as usize;
                }
                payload += self.offset_size as usize;
                payload += self.length_size as usize;
            }
        }

        let payload = payload as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    fn write_sized_value<W: Write>(&self, writer: &mut W, value: u64, size: u8) -> io::Result<()> {
        match size {
            0 => Ok(()),
            1 => writer.write_u8(value as u8),
            2 => writer.write_u16::<BigEndian>(value as u16),
            4 => writer.write_u32::<BigEndian>(value as u32),
            8 => writer.write_u64::<BigEndian>(value),
            _ => Ok(()),
        }
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.version();
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;

        // Write sizes
        let sizes1 = ((self.offset_size & 0xF) << 4) | (self.length_size & 0xF);
        let sizes2 = ((self.base_offset_size & 0xF) << 4) | (if version >= 1 { self.index_size & 0xF } else { 0 });
        writer.write_u8(sizes1)?;
        writer.write_u8(sizes2)?;

        // Write item count
        if version < 2 {
            writer.write_u16::<BigEndian>(self.items.len() as u16)?;
        } else {
            writer.write_u32::<BigEndian>(self.items.len() as u32)?;
        }

        // Write items
        for item in &self.items {
            if version < 2 {
                writer.write_u16::<BigEndian>(item.item_id as u16)?;
            } else {
                writer.write_u32::<BigEndian>(item.item_id)?;
            }

            if version >= 1 {
                writer.write_u16::<BigEndian>(item.construction_method as u16)?;
            }

            writer.write_u16::<BigEndian>(item.data_reference_index)?;
            self.write_sized_value(writer, item.base_offset, self.base_offset_size)?;
            writer.write_u16::<BigEndian>(item.extents.len() as u16)?;

            for extent in &item.extents {
                if version >= 1 && self.index_size > 0 {
                    self.write_sized_value(writer, extent.extent_index, self.index_size)?;
                }
                self.write_sized_value(writer, extent.extent_offset, self.offset_size)?;
                self.write_sized_value(writer, extent.extent_length, self.length_size)?;
            }
        }

        Ok(())
    }
}

impl Default for ItemLocationBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            offset_size: 4,
            length_size: 4,
            base_offset_size: 0,
            index_size: 0,
            items: Vec::new(),
        }
    }
}

impl ItemLocationBox for ItemLocationBoxOwned {
    type Item<'a> = &'a ItemLocationOwned where Self: 'a;

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

    fn offset_size(&self) -> u8 {
        self.offset_size
    }

    fn length_size(&self) -> u8 {
        self.length_size
    }

    fn base_offset_size(&self) -> u8 {
        self.base_offset_size
    }

    fn index_size(&self) -> u8 {
        self.index_size
    }

    fn item_count(&self) -> u32 {
        self.items.len() as u32
    }

    fn items(&self) -> impl Iterator<Item = Result<Self::Item<'_>, ParseError>> + '_ {
        self.items.iter().map(Ok)
    }
}

impl TryFrom<&ItemLocationBoxView<'_>> for ItemLocationBoxOwned {
    type Error = ParseError;

    fn try_from(source: &ItemLocationBoxView<'_>) -> Result<Self, Self::Error> {
        let items = ItemLocationBox::items(source)
            .map(|res| res.map(|v| ItemLocation::to_owned(&v)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            flags: source.flags(),
            offset_size: source.offset_size(),
            length_size: source.length_size(),
            base_offset_size: source.base_offset_size(),
            index_size: source.index_size(),
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_iloc_v0() -> Vec<u8> {
        let mut data = Vec::new();
        // v0 with offset_size=4, length_size=4, base_offset_size=0, 1 item, 1 extent
        // size = 8 + 4 + 2 + (2 + 2 + 2) + (4 + 4) = 8 + 4 + 2 + 6 + 8 = 28
        // But also need to account for the fullbox header: 8 + 4(version+flags) + 2(sizes) + 2(item_count) + 6 + 8 = 30
        data.extend_from_slice(&30u32.to_be_bytes()); // size
        data.extend_from_slice(b"iloc");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(0x44); // offset_size=4, length_size=4
        data.push(0x00); // base_offset_size=0, index_size=0
        data.extend_from_slice(&1u16.to_be_bytes()); // item_count

        // Item 1
        data.extend_from_slice(&1u16.to_be_bytes()); // item_id
        data.extend_from_slice(&0u16.to_be_bytes()); // data_reference_index
        data.extend_from_slice(&1u16.to_be_bytes()); // extent_count
        data.extend_from_slice(&100u32.to_be_bytes()); // extent_offset
        data.extend_from_slice(&500u32.to_be_bytes()); // extent_length

        data
    }

    #[test]
    fn parse_iloc_v0() {
        let data = make_iloc_v0();
        let view = ItemLocationBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.offset_size(), 4);
        assert_eq!(view.length_size(), 4);
        assert_eq!(view.item_count(), 1);

        let items: Vec<_> =
            ItemLocationBox::items(&view).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_id(), 1);
        assert_eq!(items[0].extent_count(), 1);
        let extents: Vec<_> = items[0].extents().collect();
        assert_eq!(extents[0].extent_offset, 100);
        assert_eq!(extents[0].extent_length, 500);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_iloc_v0();
        let view = ItemLocationBoxView::new(&data).unwrap();
        let owned = ItemLocationBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
