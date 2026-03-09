//! Alternative Entity Group Box (altr) parsing and serialization.
//!
//! The Alternative Entity Group Box marks items as alternatives to each other.
//!
//! ```text
//! aligned(8) class EntityToGroupBox(grouping_type, version, flags)
//!    extends FullBox(grouping_type, version, flags) {
//!    unsigned int(32) group_id;
//!    unsigned int(32) num_entities_in_group;
//!    for(i=0; i<num_entities_in_group; i++)
//!       unsigned int(32) entity_id;
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for AlternativeEntityGroupBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"altr");

/// Common interface for accessing AlternativeEntityGroupBox data.
pub trait AlternativeEntityGroupBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the group ID.
    fn group_id(&self) -> u32;

    /// Returns the number of entities.
    fn num_entities(&self) -> u32;

    /// Returns the entity IDs.
    fn entity_ids(&self) -> Vec<u32>;
}

/// A borrowing view over raw AlternativeEntityGroupBox bytes.
#[derive(Clone, Copy)]
pub struct AlternativeEntityGroupBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    entity_entries: FixedSizeEntries<'a, 4>,
}

impl<'a> AlternativeEntityGroupBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;

        let num_entities = BigEndian::read_u32(&data[fullbox_offset + 8..fullbox_offset + 12]);
        let entity_entries = FixedSizeEntries::new(&data[fullbox_offset + 12..], num_entities)?;

        Ok(Self { data, fullbox_offset, entity_entries })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the entity IDs.
    pub fn entity_ids(&self) -> Vec<u32> {
        self.entity_entries.iter().map(|b| u32::from_be_bytes(*b)).collect()
    }
}

impl<'a> AlternativeEntityGroupBox for AlternativeEntityGroupBoxView<'a> {
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

    fn group_id(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn num_entities(&self) -> u32 {
        self.entity_entries.count()
    }

    fn entity_ids(&self) -> Vec<u32> {
        self.entity_ids()
    }
}

impl std::fmt::Debug for AlternativeEntityGroupBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlternativeEntityGroupBoxView")
            .field("group_id", &self.group_id())
            .field("num_entities", &self.num_entities())
            .finish()
    }
}

/// An owned representation of AlternativeEntityGroupBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlternativeEntityGroupBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Group ID.
    pub group_id: u32,
    /// Entity IDs.
    pub entity_ids: Vec<u32>,
}

impl AlternativeEntityGroupBoxOwned {
    /// Creates a new AlternativeEntityGroupBoxOwned.
    pub fn new(group_id: u32) -> Self {
        Self {
            flags: 0,
            group_id,
            entity_ids: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + 4 + self.entity_ids.len() * 4) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.group_id)?;
        writer.write_u32::<BigEndian>(self.entity_ids.len() as u32)?;
        for &id in &self.entity_ids {
            writer.write_u32::<BigEndian>(id)?;
        }

        Ok(())
    }
}

impl Default for AlternativeEntityGroupBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl AlternativeEntityGroupBox for AlternativeEntityGroupBoxOwned {
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

    fn group_id(&self) -> u32 {
        self.group_id
    }

    fn num_entities(&self) -> u32 {
        self.entity_ids.len() as u32
    }

    fn entity_ids(&self) -> Vec<u32> {
        self.entity_ids.clone()
    }
}

impl<T: AlternativeEntityGroupBox> From<&T> for AlternativeEntityGroupBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            group_id: source.group_id(),
            entity_ids: source.entity_ids(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_altr() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // 8 + 4 + 4 + 4
        data.extend_from_slice(b"altr");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // group_id
        data.extend_from_slice(&0u32.to_be_bytes()); // num_entities
        data
    }

    #[test]
    fn parse_altr() {
        let data = make_altr();
        let view = AlternativeEntityGroupBoxView::new(&data).unwrap();
        assert_eq!(view.group_id(), 1);
        assert_eq!(view.num_entities(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_altr();
        let view = AlternativeEntityGroupBoxView::new(&data).unwrap();
        let owned = AlternativeEntityGroupBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
