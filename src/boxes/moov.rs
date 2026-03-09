//! Movie Box (moov) parsing and serialization.
//!
//! The Movie Box is a container for all the metadata about a presentation.
//!
//! ```text
//! aligned(8) class MovieBox extends Box('moov') {
//! }
//! ```

use crate::boxes::meta::{
    self, MetadataBox as _, MetadataBoxOwned, MetadataBoxView,
};
use crate::boxes::mvex::{
    self, MovieExtendsBox as _, MovieExtendsBoxOwned, MovieExtendsBoxView,
};
use crate::boxes::mvhd::{
    self, MovieHeaderBox as _, MovieHeaderBoxOwned, MovieHeaderBoxView,
};
use crate::boxes::trak::{
    self, TrackBox as _, TrackBoxOwned, TrackBoxView,
};
use crate::boxes::udta::{
    self, UserDataBox as _, UserDataBoxOwned, UserDataBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieBox.
pub const BOX_TYPE: BoxCode = BoxCode::MOOV;

/// A typed child of a MovieBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MovieChild {
    /// A MovieHeaderBox child.
    Mvhd(MovieHeaderBoxOwned),
    /// A TrackBox child.
    Trak(TrackBoxOwned),
    /// A MetadataBox child.
    Meta(MetadataBoxOwned),
    /// A MovieExtendsBox child.
    Mvex(MovieExtendsBoxOwned),
    /// A UserDataBox child.
    Udta(UserDataBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for MovieChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Mvhd(_) => mvhd::BOX_TYPE,
            Self::Trak(_) => trak::BOX_TYPE,
            Self::Meta(_) => meta::BOX_TYPE,
            Self::Mvex(_) => mvex::BOX_TYPE,
            Self::Udta(_) => udta::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Mvhd(b) => b.box_size(),
            Self::Trak(b) => b.box_size(),
            Self::Meta(b) => b.box_size(),
            Self::Mvex(b) => b.box_size(),
            Self::Udta(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl MovieChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Mvhd(b) => b.write_to(writer),
            Self::Trak(b) => b.write_to(writer),
            Self::Meta(b) => b.write_to(writer),
            Self::Mvex(b) => b.write_to(writer),
            Self::Udta(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for MovieChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            mvhd::BOX_TYPE => match MovieHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Mvhd(MovieHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            trak::BOX_TYPE => match TrackBoxView::new(raw.data()) {
                Ok(v) => Self::Trak(TrackBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            meta::BOX_TYPE => match MetadataBoxView::new(raw.data()) {
                Ok(v) => Self::Meta(MetadataBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            mvex::BOX_TYPE => match MovieExtendsBoxView::new(raw.data()) {
                Ok(v) => Self::Mvex(MovieExtendsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            udta::BOX_TYPE => match UserDataBoxView::new(raw.data()) {
                Ok(v) => Self::Udta(UserDataBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&MovieChild> for MovieChild {
    fn from(source: &MovieChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing MovieBox data.
pub trait MovieBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<MovieChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw MovieBox bytes.
#[derive(Clone, Copy)]
pub struct MovieBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> MovieBoxView<'a> {
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

    /// Returns the first MovieHeaderBox child, if present.
    pub fn mvhd(&self) -> Option<MovieHeaderBoxView<'a>> {
        self.children()
            .find_as::<MovieHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns an iterator over TrackBox children.
    pub fn trak(&self) -> impl Iterator<Item = TrackBoxView<'a>> {
        self.children()
            .filter_as::<TrackBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<MetadataBoxView<'a>> {
        self.children()
            .find_as::<MetadataBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first MovieExtendsBox child, if present.
    pub fn mvex(&self) -> Option<MovieExtendsBoxView<'a>> {
        self.children()
            .find_as::<MovieExtendsBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first UserDataBox child, if present.
    pub fn udta(&self) -> Option<UserDataBoxView<'a>> {
        self.children()
            .find_as::<UserDataBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> MovieBox for MovieBoxView<'a> {
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

impl std::fmt::Debug for MovieBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("MovieBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of MovieBox data.
///
/// This stores the raw child box data. For a fully parsed movie box,
/// use the specific child box types.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MovieBoxOwned {
    /// Typed child boxes.
    pub children: Vec<MovieChild>,
}

impl MovieBoxOwned {
    /// Creates a new empty MovieBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let children_size: u64 = self.children.iter().map(|c| c.box_size()).sum();
        header_size_for_payload(children_size) + children_size
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

    /// Returns the first MovieHeaderBox child, if present.
    pub fn mvhd(&self) -> Option<&MovieHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieChild::Mvhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over TrackBox children.
    pub fn trak(&self) -> impl Iterator<Item = &TrackBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            MovieChild::Trak(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<&MetadataBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieChild::Meta(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first MovieExtendsBox child, if present.
    pub fn mvex(&self) -> Option<&MovieExtendsBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieChild::Mvex(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first UserDataBox child, if present.
    pub fn udta(&self) -> Option<&UserDataBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieChild::Udta(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a MovieHeaderBox child.
    pub fn add_mvhd(&mut self, b: MovieHeaderBoxOwned) {
        self.children.push(MovieChild::Mvhd(b));
    }

    /// Adds a TrackBox child.
    pub fn add_trak(&mut self, b: TrackBoxOwned) {
        self.children.push(MovieChild::Trak(b));
    }

    /// Adds a MetadataBox child.
    pub fn add_meta(&mut self, b: MetadataBoxOwned) {
        self.children.push(MovieChild::Meta(b));
    }

    /// Adds a MovieExtendsBox child.
    pub fn add_mvex(&mut self, b: MovieExtendsBoxOwned) {
        self.children.push(MovieChild::Mvex(b));
    }

    /// Adds a UserDataBox child.
    pub fn add_udta(&mut self, b: UserDataBoxOwned) {
        self.children.push(MovieChild::Udta(b));
    }
}

impl MovieBox for MovieBoxOwned {
    type Child<'a> = &'a MovieChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &MovieChild> {
        self.children.iter()
    }
}

impl<T: MovieBox> From<&T> for MovieBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_moov_with_children(children: &[u8]) -> Vec<u8> {
        let size = 8 + children.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"moov");
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
    fn parse_empty_moov() {
        let data = make_moov_with_children(&[]);
        let view = MovieBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 8);
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn parse_moov_with_children() {
        let mut children = Vec::new();
        children.extend_from_slice(&make_child_box(b"mvhd", &[0u8; 96]));
        children.extend_from_slice(&make_child_box(b"trak", &[0u8; 16]));

        let data = make_moov_with_children(&children);
        let view = MovieBoxView::new(&data).unwrap();

        let child_list: Vec<_> = view.children().collect();
        assert_eq!(child_list.len(), 2);
        assert_eq!(child_list[0].box_type(), BoxCode::MVHD);
        assert_eq!(child_list[1].box_type(), BoxCode::TRAK);
    }

    #[test]
    fn find_mvhd() {
        let mut children = Vec::new();
        children.extend_from_slice(&make_child_box(b"mvhd", &[0u8; 96]));

        let data = make_moov_with_children(&children);
        let view = MovieBoxView::new(&data).unwrap();

        assert!(view.children().any(|c| c.box_type() == BoxCode::MVHD));
    }

    #[test]
    fn roundtrip() {
        let mut children = Vec::new();
        children.extend_from_slice(&make_child_box(b"mvhd", &[0u8; 96]));
        children.extend_from_slice(&make_child_box(b"trak", &[0u8; 16]));

        let data = make_moov_with_children(&children);
        let view = MovieBoxView::new(&data).unwrap();
        let owned = MovieBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
