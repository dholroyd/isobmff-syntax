//! Track Reference Box (tref) parsing and serialization.
//!
//! The Track Reference Box provides references from one track to another.
//! It contains track reference type boxes (sync, hint, cdsc, etc.) which
//! each contain an array of track IDs.
//!
//! ```text
//! aligned(8) class TrackReferenceBox extends Box('tref') {
//! }
//!
//! aligned(8) class TrackReferenceTypeBox (referenceType)
//!    extends Box(referenceType) {
//!    unsigned int(32) track_IDs[];
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackReferenceBox.
pub const BOX_TYPE: BoxCode = BoxCode::TREF;

/// A typed child of a TrackReferenceBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackReferenceChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for TrackReferenceChild {
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

impl TrackReferenceChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for TrackReferenceChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&TrackReferenceChild> for TrackReferenceChild {
    fn from(source: &TrackReferenceChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing TrackReferenceBox data.
pub trait TrackReferenceBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<TrackReferenceChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw TrackReferenceBox bytes.
#[derive(Clone, Copy)]
pub struct TrackReferenceBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> TrackReferenceBoxView<'a> {
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
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.header.header_size as usize..])
    }
}

impl<'a> TrackReferenceBox for TrackReferenceBoxView<'a> {
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

impl std::fmt::Debug for TrackReferenceBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("TrackReferenceBoxView")
            .field("box_size", &self.box_size())
            .field("reference_types", &child_types)
            .finish()
    }
}

/// An owned representation of TrackReferenceBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TrackReferenceBoxOwned {
    /// Typed child boxes.
    pub children: Vec<TrackReferenceChild>,
}

impl TrackReferenceBoxOwned {
    /// Creates a new empty TrackReferenceBoxOwned.
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


impl TrackReferenceBox for TrackReferenceBoxOwned {
    type Child<'a> = &'a TrackReferenceChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &TrackReferenceChild> {
        self.children.iter()
    }
}

impl<T: TrackReferenceBox> From<&T> for TrackReferenceBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

// ============================================================================
// Track Reference Type Box (sync, hint, cdsc, dpnd, ipir, mpod, etc.)
// ============================================================================

/// Common interface for accessing TrackReferenceTypeBox data.
///
/// Track reference type boxes are children of tref and contain an array
/// of track IDs. The box type identifies the reference type (e.g., "sync",
/// "hint", "cdsc", "dpnd", "ipir", "mpod").
pub trait TrackReferenceTypeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type (reference type code).
    fn box_type(&self) -> BoxCode;

    /// Returns the number of track IDs in this reference.
    fn track_id_count(&self) -> u32;

    /// Returns the track ID at the given index (0-based).
    fn track_id(&self, index: usize) -> Option<u32>;

    /// Returns an iterator over the track IDs.
    fn track_ids(&self) -> impl Iterator<Item = u32> + '_;
}

/// A borrowing view over raw TrackReferenceTypeBox bytes.
///
/// This handles any track reference type box (sync, hint, cdsc, etc.).
#[derive(Clone, Copy)]
pub struct TrackReferenceTypeBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> TrackReferenceTypeBoxView<'a> {
    /// Creates a new view over the given bytes.
    ///
    /// Unlike other box views, this accepts any box type since track
    /// reference types are identified by their four-character code.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;

        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }

        Ok(Self { data, header })
    }

    /// Creates a view from a RawBox.
    pub fn from_raw_box(raw: &RawBox<'a>) -> Result<Self, ParseError> {
        Self::new(raw.data())
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the payload data (array of track IDs).
    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        &self.data[self.header.header_size as usize..]
    }

}

impl TrackReferenceTypeBox for TrackReferenceTypeBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn track_id_count(&self) -> u32 {
        (self.payload().len() / 4) as u32
    }

    fn track_id(&self, index: usize) -> Option<u32> {
        let payload = self.payload();
        let offset = index * 4;
        if offset + 4 > payload.len() {
            return None;
        }
        Some(BigEndian::read_u32(&payload[offset..offset + 4]))
    }

    fn track_ids(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.track_id_count() as usize).filter_map(|i| self.track_id(i))
    }
}

impl std::fmt::Debug for TrackReferenceTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let track_ids: Vec<_> = self.track_ids().collect();
        f.debug_struct("TrackReferenceTypeBoxView")
            .field("box_type", &String::from_utf8_lossy(&self.header.box_type.0 .0))
            .field("box_size", &self.box_size())
            .field("track_ids", &track_ids)
            .finish()
    }
}

/// An owned representation of TrackReferenceTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackReferenceTypeBoxOwned {
    /// The reference type (box type code).
    pub reference_type: BoxCode,
    /// The track IDs.
    pub track_ids: Vec<u32>,
}

impl TrackReferenceTypeBoxOwned {
    /// Creates a new TrackReferenceTypeBoxOwned.
    pub fn new(reference_type: BoxCode) -> Self {
        Self {
            reference_type,
            track_ids: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.track_ids.len() as u64) * 4;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, self.reference_type)?;
        for &track_id in &self.track_ids {
            writer.write_u32::<BigEndian>(track_id)?;
        }
        Ok(())
    }
}

impl TrackReferenceTypeBox for TrackReferenceTypeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        self.reference_type
    }

    fn track_id_count(&self) -> u32 {
        self.track_ids.len() as u32
    }

    fn track_id(&self, index: usize) -> Option<u32> {
        self.track_ids.get(index).copied()
    }

    fn track_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.track_ids.iter().copied()
    }
}

impl<T: TrackReferenceTypeBox> From<&T> for TrackReferenceTypeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            reference_type: source.box_type(),
            track_ids: source.track_ids().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mp4ra_rust::FourCC;

    #[test]
    fn parse_empty_tref() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"tref");

        let view = TrackReferenceBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"tref");

        let view = TrackReferenceBoxView::new(&data).unwrap();
        let owned = TrackReferenceBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn parse_track_reference_type_box() {
        // Build a sync reference with two track IDs
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // size: 8 header + 8 payload
        data.extend_from_slice(b"sync");
        data.extend_from_slice(&1u32.to_be_bytes()); // track ID 1
        data.extend_from_slice(&2u32.to_be_bytes()); // track ID 2

        let view = TrackReferenceTypeBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 16);
        assert_eq!(view.box_type(), BoxCode(FourCC(*b"sync")));
        assert_eq!(view.track_id_count(), 2);
        assert_eq!(view.track_id(0), Some(1));
        assert_eq!(view.track_id(1), Some(2));
        assert_eq!(view.track_id(2), None);
    }

    #[test]
    fn track_reference_type_box_iterator() {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // size: 8 + 12
        data.extend_from_slice(b"cdsc");
        data.extend_from_slice(&100u32.to_be_bytes());
        data.extend_from_slice(&200u32.to_be_bytes());
        data.extend_from_slice(&300u32.to_be_bytes());

        let view = TrackReferenceTypeBoxView::new(&data).unwrap();
        let track_ids: Vec<u32> = view.track_ids().collect();
        assert_eq!(track_ids, vec![100, 200, 300]);
    }

    #[test]
    fn track_reference_type_box_roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"hint");
        data.extend_from_slice(&42u32.to_be_bytes());
        data.extend_from_slice(&99u32.to_be_bytes());

        let view = TrackReferenceTypeBoxView::new(&data).unwrap();
        let owned = TrackReferenceTypeBoxOwned::from(&view);

        assert_eq!(owned.reference_type, BoxCode(FourCC(*b"hint")));
        assert_eq!(owned.track_ids, vec![42, 99]);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }
}
