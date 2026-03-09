//! Track Box (trak) parsing and serialization.
//!
//! The Track Box is a container for a single track of a presentation.
//!
//! ```text
//! aligned(8) class TrackBox extends Box('trak') {
//! }
//! ```

use crate::boxes::edts::{self, EditBox as _, EditBoxOwned, EditBoxView};
use crate::boxes::mdia::{self, MediaBox as _, MediaBoxOwned, MediaBoxView};
use crate::boxes::meta::{self, MetadataBox as _, MetadataBoxOwned, MetadataBoxView};
use crate::boxes::strk::{self, SubTrackBox as _, SubTrackBoxOwned, SubTrackBoxView};
use crate::boxes::tkhd::{self, TrackHeaderBox as _, TrackHeaderBoxOwned, TrackHeaderBoxView};
use crate::boxes::tref::{self, TrackReferenceBox as _, TrackReferenceBoxOwned, TrackReferenceBoxView};
use crate::boxes::trgr::{self, TrackGroupBox as _, TrackGroupBoxOwned, TrackGroupBoxView};
use crate::boxes::udta::{self, UserDataBox as _, UserDataBoxOwned, UserDataBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackBox.
pub const BOX_TYPE: BoxCode = BoxCode::TRAK;

/// A typed child of a TrackBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackChild {
    /// A TrackHeaderBox child.
    Tkhd(TrackHeaderBoxOwned),
    /// A TrackReferenceBox child.
    Tref(TrackReferenceBoxOwned),
    /// A TrackGroupBox child.
    Trgr(TrackGroupBoxOwned),
    /// An EditBox child.
    Edts(EditBoxOwned),
    /// A MediaBox child.
    Mdia(MediaBoxOwned),
    /// A UserDataBox child.
    Udta(UserDataBoxOwned),
    /// A MetadataBox child.
    Meta(MetadataBoxOwned),
    /// A SubTrackBox child.
    Strk(SubTrackBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for TrackChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Tkhd(_) => tkhd::BOX_TYPE,
            Self::Tref(_) => tref::BOX_TYPE,
            Self::Trgr(_) => trgr::BOX_TYPE,
            Self::Edts(_) => edts::BOX_TYPE,
            Self::Mdia(_) => mdia::BOX_TYPE,
            Self::Udta(_) => udta::BOX_TYPE,
            Self::Meta(_) => meta::BOX_TYPE,
            Self::Strk(_) => strk::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Tkhd(b) => b.box_size(),
            Self::Tref(b) => b.box_size(),
            Self::Trgr(b) => b.box_size(),
            Self::Edts(b) => b.box_size(),
            Self::Mdia(b) => b.box_size(),
            Self::Udta(b) => b.box_size(),
            Self::Meta(b) => b.box_size(),
            Self::Strk(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl TrackChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Tkhd(b) => b.write_to(writer),
            Self::Tref(b) => b.write_to(writer),
            Self::Trgr(b) => b.write_to(writer),
            Self::Edts(b) => b.write_to(writer),
            Self::Mdia(b) => b.write_to(writer),
            Self::Udta(b) => b.write_to(writer),
            Self::Meta(b) => b.write_to(writer),
            Self::Strk(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for TrackChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            tkhd::BOX_TYPE => match TrackHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Tkhd(TrackHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            tref::BOX_TYPE => match TrackReferenceBoxView::new(raw.data()) {
                Ok(v) => Self::Tref(TrackReferenceBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            trgr::BOX_TYPE => match TrackGroupBoxView::new(raw.data()) {
                Ok(v) => Self::Trgr(TrackGroupBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            edts::BOX_TYPE => match EditBoxView::new(raw.data()) {
                Ok(v) => Self::Edts(EditBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            mdia::BOX_TYPE => match MediaBoxView::new(raw.data()) {
                Ok(v) => Self::Mdia(MediaBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            udta::BOX_TYPE => match UserDataBoxView::new(raw.data()) {
                Ok(v) => Self::Udta(UserDataBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            meta::BOX_TYPE => match MetadataBoxView::new(raw.data()) {
                Ok(v) => Self::Meta(MetadataBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            strk::BOX_TYPE => match SubTrackBoxView::new(raw.data()) {
                Ok(v) => Self::Strk(SubTrackBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&TrackChild> for TrackChild {
    fn from(source: &TrackChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing TrackBox data.
pub trait TrackBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<TrackChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw TrackBox bytes.
#[derive(Clone, Copy)]
pub struct TrackBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> TrackBoxView<'a> {
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

    /// Returns the first TrackHeaderBox child, if present.
    pub fn tkhd(&self) -> Option<TrackHeaderBoxView<'a>> {
        self.children()
            .find_as::<TrackHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first TrackReferenceBox child, if present.
    pub fn tref(&self) -> Option<TrackReferenceBoxView<'a>> {
        self.children()
            .find_as::<TrackReferenceBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first TrackGroupBox child, if present.
    pub fn trgr(&self) -> Option<TrackGroupBoxView<'a>> {
        self.children()
            .find_as::<TrackGroupBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first EditBox child, if present.
    pub fn edts(&self) -> Option<EditBoxView<'a>> {
        self.children()
            .find_as::<EditBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first MediaBox child, if present.
    pub fn mdia(&self) -> Option<MediaBoxView<'a>> {
        self.children()
            .find_as::<MediaBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first UserDataBox child, if present.
    pub fn udta(&self) -> Option<UserDataBoxView<'a>> {
        self.children()
            .find_as::<UserDataBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<MetadataBoxView<'a>> {
        self.children()
            .find_as::<MetadataBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first SubTrackBox child, if present.
    pub fn strk(&self) -> Option<SubTrackBoxView<'a>> {
        self.children()
            .find_as::<SubTrackBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> TrackBox for TrackBoxView<'a> {
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

impl std::fmt::Debug for TrackBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("TrackBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of TrackBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TrackBoxOwned {
    /// Typed child boxes.
    pub children: Vec<TrackChild>,
}

impl TrackBoxOwned {
    /// Creates a new empty TrackBoxOwned.
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

    /// Returns the first TrackHeaderBox child, if present.
    pub fn tkhd(&self) -> Option<&TrackHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Tkhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first TrackReferenceBox child, if present.
    pub fn tref(&self) -> Option<&TrackReferenceBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Tref(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first TrackGroupBox child, if present.
    pub fn trgr(&self) -> Option<&TrackGroupBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Trgr(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first EditBox child, if present.
    pub fn edts(&self) -> Option<&EditBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Edts(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first MediaBox child, if present.
    pub fn mdia(&self) -> Option<&MediaBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Mdia(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first UserDataBox child, if present.
    pub fn udta(&self) -> Option<&UserDataBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Udta(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<&MetadataBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Meta(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first SubTrackBox child, if present.
    pub fn strk(&self) -> Option<&SubTrackBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackChild::Strk(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a TrackHeaderBox child.
    pub fn add_tkhd(&mut self, b: TrackHeaderBoxOwned) {
        self.children.push(TrackChild::Tkhd(b));
    }

    /// Adds a TrackReferenceBox child.
    pub fn add_tref(&mut self, b: TrackReferenceBoxOwned) {
        self.children.push(TrackChild::Tref(b));
    }

    /// Adds a TrackGroupBox child.
    pub fn add_trgr(&mut self, b: TrackGroupBoxOwned) {
        self.children.push(TrackChild::Trgr(b));
    }

    /// Adds an EditBox child.
    pub fn add_edts(&mut self, b: EditBoxOwned) {
        self.children.push(TrackChild::Edts(b));
    }

    /// Adds a MediaBox child.
    pub fn add_mdia(&mut self, b: MediaBoxOwned) {
        self.children.push(TrackChild::Mdia(b));
    }

    /// Adds a UserDataBox child.
    pub fn add_udta(&mut self, b: UserDataBoxOwned) {
        self.children.push(TrackChild::Udta(b));
    }

    /// Adds a MetadataBox child.
    pub fn add_meta(&mut self, b: MetadataBoxOwned) {
        self.children.push(TrackChild::Meta(b));
    }

    /// Adds a SubTrackBox child.
    pub fn add_strk(&mut self, b: SubTrackBoxOwned) {
        self.children.push(TrackChild::Strk(b));
    }
}

impl TrackBox for TrackBoxOwned {
    type Child<'a> = &'a TrackChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &TrackChild> {
        self.children.iter()
    }
}

impl<T: TrackBox> From<&T> for TrackBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trak_with_children(children: &[u8]) -> Vec<u8> {
        let size = 8 + children.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"trak");
        data.extend_from_slice(children);
        data
    }

    fn make_child_box(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = 8 + payload.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(box_type);
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn parse_empty_trak() {
        let data = make_trak_with_children(&[]);
        let view = TrackBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn parse_trak_with_children() {
        let mut children = Vec::new();
        children.extend_from_slice(&make_child_box(b"tkhd", &[0u8; 84]));
        children.extend_from_slice(&make_child_box(b"mdia", &[0u8; 16]));

        let data = make_trak_with_children(&children);
        let view = TrackBoxView::new(&data).unwrap();

        assert!(view.children().any(|c| c.box_type() == BoxCode::TKHD));
        assert!(view.children().any(|c| c.box_type() == BoxCode::MDIA));
    }

    #[test]
    fn roundtrip() {
        let mut children = Vec::new();
        children.extend_from_slice(&make_child_box(b"tkhd", &[0u8; 84]));

        let data = make_trak_with_children(&children);
        let view = TrackBoxView::new(&data).unwrap();
        let owned = TrackBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
