//! Entity to Group Box (grpl) and entity group entry parsing and serialization.
//!
//! The Entity to Group Box is a container box for entity group boxes.
//! Each child is an entity group entry - a FullBox whose box type is the
//! grouping_type (e.g. 'altr', 'ster') and whose payload contains
//! group_id(4) + num_entities_in_group(4) + entity_id(4) * N.
//! See ISO 14496-12 Section 8.18.
//!
//! ```text
//! aligned(8) class GroupsListBox extends Box('grpl') {
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, FullBoxHeader, fullbox_header_size_for_payload, header_size_for_payload, write_box_header, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for EntityToGroupBox.
pub const BOX_TYPE: BoxCode = BoxCode::GRPL;

/// A typed child of an EntityToGroupBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntityToGroupChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for EntityToGroupChild {
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

impl EntityToGroupChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for EntityToGroupChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&EntityToGroupChild> for EntityToGroupChild {
    fn from(source: &EntityToGroupChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing EntityToGroupBox data.
pub trait EntityToGroupBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<EntityToGroupChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw EntityToGroupBox bytes.
#[derive(Clone, Copy)]
pub struct EntityToGroupBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> EntityToGroupBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 0)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.header_size..])
    }
}

impl<'a> EntityToGroupBox for EntityToGroupBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for EntityToGroupBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityToGroupBoxView")
            .finish()
    }
}

/// An owned representation of EntityToGroupBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct EntityToGroupBoxOwned {
    /// Typed child boxes.
    pub children: Vec<EntityToGroupChild>,
}

impl EntityToGroupBoxOwned {
    /// Creates a new EntityToGroupBoxOwned.
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
}


impl EntityToGroupBox for EntityToGroupBoxOwned {
    type Child<'a> = &'a EntityToGroupChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &EntityToGroupChild> {
        self.children.iter()
    }
}

impl<T: EntityToGroupBox> From<&T> for EntityToGroupBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

/// Common interface for accessing an entity group entry (child of grpl).
///
/// All entity group entries share a common base structure defined in
/// Section 8.18: a FullBox with group_id, num_entities_in_group, and
/// an array of entity_id values. The box type is the grouping_type
/// (e.g. 'altr', 'ster').
pub trait EntityGroupEntryBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type (which is the grouping_type).
    fn box_type(&self) -> BoxCode;

    /// Returns the grouping_type as a FourCC.
    fn grouping_type(&self) -> FourCC;

    /// Returns the version.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the group_id.
    fn group_id(&self) -> u32;

    /// Returns the number of entities in the group.
    fn num_entities(&self) -> u32;

    /// Returns an iterator over entity IDs in this group.
    fn entity_ids(&self) -> impl Iterator<Item = u32> + '_;
}

/// A borrowing view over raw entity group entry bytes.
///
/// Unlike type-specific views (e.g. `AlternativeEntityGroupBoxView`),
/// this does not check the box type - any FullBox with the shared
/// entity group base structure is accepted.
#[derive(Clone, Copy)]
pub struct EntityGroupEntryBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> EntityGroupEntryBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;

        if header.size() != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size(),
                actual: data.len(),
            });
        }

        let fullbox_offset = header.box_header.header_size as usize;
        // version/flags(4) + group_id(4) + num_entities(4) = 12 bytes
        let min_size = fullbox_offset + 12;
        if data.len() < min_size {
            return Err(ParseError::BufferTooShort {
                expected: min_size,
                found: data.len(),
            });
        }

        // Validate that num_entities doesn't claim more entries than the data can hold
        let num_entities = BigEndian::read_u32(&data[fullbox_offset + 8..fullbox_offset + 12]);
        let available = (data.len() - min_size) / 4;
        if num_entities as usize > available {
            return Err(ParseError::InvalidEntryCount {
                count: num_entities,
                max_possible: available as u32,
            });
        }

        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl EntityGroupEntryBox for EntityGroupEntryBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BoxCode::new([self.data[4], self.data[5], self.data[6], self.data[7]])
    }

    fn grouping_type(&self) -> FourCC {
        self.box_type().0
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
        let o = self.fullbox_offset + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn entity_ids(&self) -> impl Iterator<Item = u32> + '_ {
        let count = self.num_entities() as usize;
        let start = self.fullbox_offset + 12;
        (0..count).map(move |i| {
            let o = start + i * 4;
            BigEndian::read_u32(&self.data[o..o + 4])
        })
    }
}

impl std::fmt::Debug for EntityGroupEntryBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityGroupEntryBoxView")
            .field("grouping_type", &self.grouping_type())
            .field("group_id", &self.group_id())
            .field("num_entities", &self.num_entities())
            .finish()
    }
}

/// An owned representation of an entity group entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityGroupEntryBoxOwned {
    /// The grouping_type (stored as the box type).
    pub grouping_type: FourCC,
    /// Flags.
    pub flags: u32,
    /// The group_id.
    pub group_id: u32,
    /// Entity IDs.
    pub entity_ids: Vec<u32>,
}

impl EntityGroupEntryBoxOwned {
    /// Creates a new EntityGroupEntryBoxOwned.
    pub fn new(grouping_type: FourCC, group_id: u32) -> Self {
        Self {
            grouping_type,
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
        write_fullbox_header(writer, self.serialized_size(), BoxCode(self.grouping_type), 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.group_id)?;
        writer.write_u32::<BigEndian>(self.entity_ids.len() as u32)?;
        for &id in &self.entity_ids {
            writer.write_u32::<BigEndian>(id)?;
        }

        Ok(())
    }
}

impl EntityGroupEntryBox for EntityGroupEntryBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BoxCode(self.grouping_type)
    }

    fn grouping_type(&self) -> FourCC {
        self.grouping_type
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

    fn entity_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.entity_ids.iter().copied()
    }
}

impl<T: EntityGroupEntryBox> From<&T> for EntityGroupEntryBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            grouping_type: source.grouping_type(),
            flags: source.flags(),
            group_id: source.group_id(),
            entity_ids: source.entity_ids().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grpl() -> Vec<u8> {
        let mut data = Vec::new();
        // Empty grpl: 8 bytes (plain Box header only)
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"grpl");
        data
    }

    fn make_entity_group_entry(group_type: &[u8; 4], group_id: u32, entity_ids: &[u32]) -> Vec<u8> {
        let size = 20 + entity_ids.len() * 4;
        let mut data = Vec::new();
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(group_type);
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&group_id.to_be_bytes());
        data.extend_from_slice(&(entity_ids.len() as u32).to_be_bytes());
        for &id in entity_ids {
            data.extend_from_slice(&id.to_be_bytes());
        }
        data
    }

    #[test]
    fn parse_empty_grpl() {
        let data = make_grpl();
        let view = EntityToGroupBoxView::new(&data).unwrap();

        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_grpl();
        let view = EntityToGroupBoxView::new(&data).unwrap();
        let owned = EntityToGroupBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn parse_entity_group_entry() {
        let data = make_entity_group_entry(b"altr", 5, &[10, 20, 30]);
        let view = EntityGroupEntryBoxView::new(&data).unwrap();

        assert_eq!(view.grouping_type(), FourCC(*b"altr"));
        assert_eq!(view.group_id(), 5);
        assert_eq!(view.num_entities(), 3);
        assert_eq!(view.entity_ids().collect::<Vec<_>>(), vec![10, 20, 30]);
    }

    #[test]
    fn parse_entity_group_entry_empty() {
        let data = make_entity_group_entry(b"ster", 1, &[]);
        let view = EntityGroupEntryBoxView::new(&data).unwrap();

        assert_eq!(view.grouping_type(), FourCC(*b"ster"));
        assert_eq!(view.group_id(), 1);
        assert_eq!(view.num_entities(), 0);
        assert_eq!(view.entity_ids().collect::<Vec<_>>(), Vec::<u32>::new());
    }

    #[test]
    fn entity_group_entry_num_entities_exceeds_data() {
        // num_entities claims 3 entries but only 2 entity IDs worth of data
        // is present - previously caused an out-of-bounds panic in entity_ids()
        let mut data = Vec::new();
        let size: u32 = 28; // header(8) + version/flags(4) + group_id(4) + num_entities(4) + 2 entity IDs(8)
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(b"altr");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // group_id
        data.extend_from_slice(&3u32.to_be_bytes()); // num_entities = 3, but only 2 IDs follow
        data.extend_from_slice(&10u32.to_be_bytes());
        data.extend_from_slice(&20u32.to_be_bytes());

        let err = EntityGroupEntryBoxView::new(&data).unwrap_err();
        assert!(matches!(err, ParseError::InvalidEntryCount { count: 3, max_possible: 2 }));
    }

    #[test]
    fn entity_group_entry_roundtrip() {
        let data = make_entity_group_entry(b"altr", 3, &[1, 2]);
        let view = EntityGroupEntryBoxView::new(&data).unwrap();
        let owned = EntityGroupEntryBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
