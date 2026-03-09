//! Track Group Box (trgr) and Track Group Type Box parsing and serialization.
//!
//! The Track Group Box is a container for track group type boxes.
//! Each child is a TrackGroupTypeBox - a FullBox whose box type is the
//! track_group_type (e.g. 'msrc') and whose payload is a single
//! track_group_id (u32). See ISO 14496-12 Section 8.3.4.
//!
//! ```text
//! aligned(8) class TrackGroupBox extends Box('trgr') {
//! }
//!
//! aligned(8) class TrackGroupTypeBox(track_group_type)
//!    extends FullBox(track_group_type, version = 0, flags = 0) {
//!    unsigned int(32) track_group_id;
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, FullBoxHeader, fullbox_header_size_for_payload, header_size_for_payload, write_box_header, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for TrackGroupBox.
pub const BOX_TYPE: BoxCode = BoxCode::TRGR;

/// A typed child of a TrackGroupBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackGroupChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for TrackGroupChild {
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

impl TrackGroupChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for TrackGroupChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&TrackGroupChild> for TrackGroupChild {
    fn from(source: &TrackGroupChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing TrackGroupBox data.
pub trait TrackGroupBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<TrackGroupChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw TrackGroupBox bytes.
#[derive(Clone, Copy)]
pub struct TrackGroupBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> TrackGroupBoxView<'a> {
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

impl<'a> TrackGroupBox for TrackGroupBoxView<'a> {
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

impl std::fmt::Debug for TrackGroupBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackGroupBoxView")
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of TrackGroupBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TrackGroupBoxOwned {
    /// Typed child boxes.
    pub children: Vec<TrackGroupChild>,
}

impl TrackGroupBoxOwned {
    /// Creates a new TrackGroupBoxOwned.
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


impl TrackGroupBox for TrackGroupBoxOwned {
    type Child<'a> = &'a TrackGroupChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &TrackGroupChild> {
        self.children.iter()
    }
}

impl<T: TrackGroupBox> From<&T> for TrackGroupBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

/// Common interface for accessing TrackGroupTypeBox data.
///
/// A TrackGroupTypeBox is a FullBox child of TrackGroupBox (trgr).
/// Its box type is the track_group_type (e.g. 'msrc'), and its payload
/// is a single track_group_id (u32).
pub trait TrackGroupTypeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type (which is the track_group_type).
    fn box_type(&self) -> BoxCode;

    /// Returns the track_group_type as a FourCC.
    fn track_group_type(&self) -> FourCC;

    /// Returns the version.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the track_group_id.
    fn track_group_id(&self) -> u32;
}

/// A borrowing view over raw TrackGroupTypeBox bytes.
///
/// Unlike most box views, this does not check the box type - any FullBox
/// with a u32 payload is accepted, since the box type itself is the
/// track_group_type identifier.
#[derive(Clone, Copy)]
pub struct TrackGroupTypeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackGroupTypeBoxView<'a> {
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
        // version/flags(4) + track_group_id(4) = 8 bytes after box header
        let min_size = fullbox_offset + 8;
        if data.len() < min_size {
            return Err(ParseError::BufferTooShort {
                expected: min_size,
                found: data.len(),
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

impl TrackGroupTypeBox for TrackGroupTypeBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BoxCode::new([self.data[4], self.data[5], self.data[6], self.data[7]])
    }

    fn track_group_type(&self) -> FourCC {
        self.box_type().0
    }

    fn version(&self) -> u8 {
        self.data[self.fullbox_offset]
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn track_group_id(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }
}

impl std::fmt::Debug for TrackGroupTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackGroupTypeBoxView")
            .field("track_group_type", &self.track_group_type())
            .field("track_group_id", &self.track_group_id())
            .finish()
    }
}

/// An owned representation of TrackGroupTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackGroupTypeBoxOwned {
    /// The track_group_type (stored as the box type).
    pub track_group_type: FourCC,
    /// Flags.
    pub flags: u32,
    /// The track_group_id.
    pub track_group_id: u32,
}

impl TrackGroupTypeBoxOwned {
    /// Creates a new TrackGroupTypeBoxOwned.
    pub fn new(track_group_type: FourCC, track_group_id: u32) -> Self {
        Self {
            track_group_type,
            flags: 0,
            track_group_id,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(4) + 4 // 8 (header) + 4 (version/flags) + 4 (track_group_id)
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BoxCode(self.track_group_type), 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.track_group_id)?;

        Ok(())
    }
}

impl TrackGroupTypeBox for TrackGroupTypeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BoxCode(self.track_group_type)
    }

    fn track_group_type(&self) -> FourCC {
        self.track_group_type
    }

    fn version(&self) -> u8 {
        0
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn track_group_id(&self) -> u32 {
        self.track_group_id
    }
}

impl<T: TrackGroupTypeBox> From<&T> for TrackGroupTypeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            track_group_type: source.track_group_type(),
            flags: source.flags(),
            track_group_id: source.track_group_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trgr() -> Vec<u8> {
        let mut data = Vec::new();
        // Empty trgr: 8 bytes
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"trgr");
        data
    }

    fn make_track_group_type(group_type: &[u8; 4], group_id: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // size
        data.extend_from_slice(group_type);             // box type = track_group_type
        data.push(0);                                   // version
        data.extend_from_slice(&[0, 0, 0]);             // flags
        data.extend_from_slice(&group_id.to_be_bytes()); // track_group_id
        data
    }

    #[test]
    fn parse_empty_trgr() {
        let data = make_trgr();
        let view = TrackGroupBoxView::new(&data).unwrap();

        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_trgr();
        let view = TrackGroupBoxView::new(&data).unwrap();
        let owned = TrackGroupBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn parse_track_group_type() {
        let data = make_track_group_type(b"msrc", 42);
        let view = TrackGroupTypeBoxView::new(&data).unwrap();

        assert_eq!(view.track_group_type(), FourCC(*b"msrc"));
        assert_eq!(view.track_group_id(), 42);
        assert_eq!(view.version(), 0);
        assert_eq!(view.flags(), 0);
    }

    #[test]
    fn track_group_type_roundtrip() {
        let data = make_track_group_type(b"msrc", 7);
        let view = TrackGroupTypeBoxView::new(&data).unwrap();
        let owned = TrackGroupTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
