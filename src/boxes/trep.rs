//! Track Extension Properties Box (trep) parsing and serialization.
//!
//! The Track Extension Properties Box contains properties for a track that
//! extend beyond the movie box. It can contain cslg (Composition to Decode
//! Timeline Mapping) and other boxes.
//!
//! ```text
//! class TrackExtensionPropertiesBox extends FullBox('trep', 0, 0) {
//!    unsigned int(32) track_ID;
//!    // Any number of boxes may follow
//! }
//! ```

use crate::boxes::cslg::{
    self, CompositionToDecodeBox as _, CompositionToDecodeBoxOwned, CompositionToDecodeBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackExtensionPropertiesBox.
pub const BOX_TYPE: BoxCode = BoxCode::TREP;

/// A typed child of a TrackExtensionPropertiesBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackExtensionPropertiesChild {
    /// A CompositionToDecodeBox child.
    Cslg(CompositionToDecodeBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for TrackExtensionPropertiesChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Cslg(_) => cslg::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Cslg(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl TrackExtensionPropertiesChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Cslg(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for TrackExtensionPropertiesChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            cslg::BOX_TYPE => match CompositionToDecodeBoxView::new(raw.data()) {
                Ok(v) => Self::Cslg(CompositionToDecodeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&TrackExtensionPropertiesChild> for TrackExtensionPropertiesChild {
    fn from(source: &TrackExtensionPropertiesChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing TrackExtensionPropertiesBox data.
pub trait TrackExtensionPropertiesBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<TrackExtensionPropertiesChild>
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

    /// Returns the track ID.
    fn track_id(&self) -> u32;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw TrackExtensionPropertiesBox bytes.
#[derive(Clone, Copy)]
pub struct TrackExtensionPropertiesBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackExtensionPropertiesBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.fullbox_offset + 4 + 4..])
    }

    /// Returns the first CompositionToDecodeBox child, if present.
    pub fn cslg(&self) -> Option<CompositionToDecodeBoxView<'a>> {
        self.children()
            .find_as::<CompositionToDecodeBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> TrackExtensionPropertiesBox for TrackExtensionPropertiesBoxView<'a> {
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

    fn track_id(&self) -> u32 {
        BigEndian::read_u32(&self.data[self.fullbox_offset + 4..self.fullbox_offset + 8])
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for TrackExtensionPropertiesBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackExtensionPropertiesBoxView")
            .field("box_size", &self.box_size())
            .field("track_id", &self.track_id())
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of TrackExtensionPropertiesBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TrackExtensionPropertiesBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Track ID.
    pub track_id: u32,
    /// Typed child boxes.
    pub children: Vec<TrackExtensionPropertiesChild>,
}

impl TrackExtensionPropertiesBoxOwned {
    /// Creates a new empty TrackExtensionPropertiesBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 4 + self.children.iter().map(|c| c.box_size()).sum::<u64>();
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.track_id)?;

        for child in &self.children {
            child.write_to(writer)?;
        }

        Ok(())
    }

    /// Returns the first CompositionToDecodeBox child, if present.
    pub fn cslg(&self) -> Option<&CompositionToDecodeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackExtensionPropertiesChild::Cslg(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a CompositionToDecodeBox child.
    pub fn add_cslg(&mut self, b: CompositionToDecodeBoxOwned) {
        self.children.push(TrackExtensionPropertiesChild::Cslg(b));
    }
}

impl TrackExtensionPropertiesBox for TrackExtensionPropertiesBoxOwned {
    type Child<'a> = &'a TrackExtensionPropertiesChild;

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

    fn track_id(&self) -> u32 {
        self.track_id
    }

    fn children(&self) -> impl Iterator<Item = &TrackExtensionPropertiesChild> {
        self.children.iter()
    }
}

impl<T: TrackExtensionPropertiesBox> From<&T> for TrackExtensionPropertiesBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            track_id: source.track_id(),
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trep(track_id: u32) -> Vec<u8> {
        let size = 16u32; // 8 header + 4 version/flags + 4 track_id
        let mut data = Vec::new();
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(b"trep");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&track_id.to_be_bytes());
        data
    }

    #[test]
    fn parse_trep() {
        let data = make_trep(1);
        let view = TrackExtensionPropertiesBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 16);
        assert_eq!(view.track_id(), 1);
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_trep(2);
        let view = TrackExtensionPropertiesBoxView::new(&data).unwrap();
        let owned = TrackExtensionPropertiesBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
