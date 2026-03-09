//! Derived Image Reference Box (dimg) parsing and serialization.
//!
//! The Derived Image Reference Box indicates that an item is derived from another item.
//!
//! ```text
//! // Item reference type 'dimg' used within ItemReferenceBox.
//! // Uses SingleItemTypeReferenceBox or SingleItemTypeReferenceBoxLarge syntax.
//! aligned(8) class SingleItemTypeReferenceBox(referenceType='dimg')
//!    extends Box('dimg') {
//!    unsigned int(16) from_item_ID;
//!    unsigned int(16) reference_count;
//!    for (j=0; j<reference_count; j++) {
//!       unsigned int(16) to_item_ID;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for DerivedImageReferenceBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"dimg");

/// Common interface for accessing DerivedImageReferenceBox data.
pub trait DerivedImageReferenceBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the source_item_id.
    fn source_item_id(&self) -> u32;

    /// Returns the reference count.
    fn reference_count(&self) -> u16;

    /// Returns the referenced item IDs.
    fn to_item_ids(&self) -> impl Iterator<Item = u32> + '_;
}

/// A borrowing view over raw DerivedImageReferenceBox bytes.
#[derive(Clone, Copy)]
pub struct DerivedImageReferenceBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
    large_ids: bool,
}

impl<'a> DerivedImageReferenceBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8], large_ids: bool) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;

        if header.box_type != BOX_TYPE {
            return Err(ParseError::InvalidBoxType {
                expected: BOX_TYPE,
                found: header.box_type,
            });
        }

        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }

        let header_size = header.header_size as usize;

        let id_size = if large_ids { 4 } else { 2 };
        let min_size = header_size + id_size + 2; // from_item_id + reference_count
        if data.len() < min_size {
            return Err(ParseError::BufferTooShort {
                expected: min_size,
                found: data.len(),
            });
        }

        let reference_count = BigEndian::read_u16(&data[header_size + id_size..header_size + id_size + 2]);
        let entries_offset = header_size + id_size + 2;
        let max_possible = ((data.len() - entries_offset) / id_size) as u32;
        if reference_count as u32 > max_possible {
            return Err(ParseError::InvalidEntryCount {
                count: reference_count as u32,
                max_possible,
            });
        }

        Ok(Self {
            data,
            header_size,
            large_ids,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the referenced item IDs.
    pub fn to_item_ids(&self) -> impl Iterator<Item = u32> + 'a {
        let id_size = if self.large_ids { 4 } else { 2 };
        let count = self.reference_count() as usize;
        let start = self.header_size + id_size + 2;
        let end = start + count * id_size;
        let data = &self.data[start..end];
        let large_ids = self.large_ids;
        data.chunks_exact(id_size).map(move |chunk| {
            if large_ids {
                BigEndian::read_u32(chunk)
            } else {
                BigEndian::read_u16(chunk) as u32
            }
        })
    }
}

impl<'a> DerivedImageReferenceBox for DerivedImageReferenceBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn source_item_id(&self) -> u32 {
        let o = self.header_size;
        if self.large_ids {
            BigEndian::read_u32(&self.data[o..o + 4])
        } else {
            BigEndian::read_u16(&self.data[o..o + 2]) as u32
        }
    }

    fn reference_count(&self) -> u16 {
        let id_size = if self.large_ids { 4 } else { 2 };
        let o = self.header_size + id_size;
        BigEndian::read_u16(&self.data[o..o + 2])
    }

    fn to_item_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.to_item_ids()
    }
}

impl std::fmt::Debug for DerivedImageReferenceBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedImageReferenceBoxView")
            .field("source_item_id", &self.source_item_id())
            .field("reference_count", &self.reference_count())
            .finish()
    }
}

/// An owned representation of DerivedImageReferenceBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedImageReferenceBoxOwned {
    /// Source item ID.
    pub source_item_id: u32,
    /// To item IDs.
    pub to_item_ids: Vec<u32>,
}

impl DerivedImageReferenceBoxOwned {
    /// Creates a new DerivedImageReferenceBoxOwned.
    pub fn new(source_item_id: u32) -> Self {
        Self {
            source_item_id,
            to_item_ids: Vec::new(),
        }
    }

    fn needs_large_ids(&self) -> bool {
        self.source_item_id > 0xFFFF || self.to_item_ids.iter().any(|&id| id > 0xFFFF)
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let id_size = if self.needs_large_ids() { 4 } else { 2 };
        let payload = (id_size + 2 + self.to_item_ids.len() * id_size) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let large_ids = self.needs_large_ids();
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;

        if large_ids {
            writer.write_u32::<BigEndian>(self.source_item_id)?;
        } else {
            writer.write_u16::<BigEndian>(self.source_item_id as u16)?;
        }

        writer.write_u16::<BigEndian>(self.to_item_ids.len() as u16)?;

        for &id in &self.to_item_ids {
            if large_ids {
                writer.write_u32::<BigEndian>(id)?;
            } else {
                writer.write_u16::<BigEndian>(id as u16)?;
            }
        }

        Ok(())
    }
}

impl Default for DerivedImageReferenceBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl DerivedImageReferenceBox for DerivedImageReferenceBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn source_item_id(&self) -> u32 {
        self.source_item_id
    }

    fn reference_count(&self) -> u16 {
        self.to_item_ids.len() as u16
    }

    fn to_item_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.to_item_ids.iter().copied()
    }
}

impl<T: DerivedImageReferenceBox> From<&T> for DerivedImageReferenceBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            source_item_id: source.source_item_id(),
            to_item_ids: source.to_item_ids().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dimg() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 2 + 2
        data.extend_from_slice(b"dimg");
        data.extend_from_slice(&1u16.to_be_bytes()); // from_item_id
        data.extend_from_slice(&0u16.to_be_bytes()); // reference_count
        data
    }

    #[test]
    fn parse_dimg() {
        let data = make_dimg();
        let view = DerivedImageReferenceBoxView::new(&data, false).unwrap();
        assert_eq!(view.source_item_id(), 1);
        assert_eq!(view.reference_count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_dimg();
        let view = DerivedImageReferenceBoxView::new(&data, false).unwrap();
        let owned = DerivedImageReferenceBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
