//! Movie Extends Box (mvex) parsing and serialization.
//!
//! The Movie Extends Box indicates that movie fragments may follow.
//!
//! ```text
//! aligned(8) class MovieExtendsBox extends Box('mvex') {
//! }
//! ```

use crate::boxes::leva::{
    self, LevelAssignmentBox as _, LevelAssignmentBoxOwned, LevelAssignmentBoxView,
};
use crate::boxes::mehd::{
    self, MovieExtendsHeaderBox as _, MovieExtendsHeaderBoxOwned, MovieExtendsHeaderBoxView,
};
use crate::boxes::trex::{
    self, TrackExtendsBox as _, TrackExtendsBoxOwned, TrackExtendsBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieExtendsBox.
pub const BOX_TYPE: BoxCode = BoxCode::MVEX;

/// A typed child of a MovieExtendsBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MovieExtendsChild {
    /// A MovieExtendsHeaderBox child.
    Mehd(MovieExtendsHeaderBoxOwned),
    /// A TrackExtendsBox child.
    Trex(TrackExtendsBoxOwned),
    /// A LevelAssignmentBox child.
    Leva(LevelAssignmentBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for MovieExtendsChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Mehd(_) => mehd::BOX_TYPE,
            Self::Trex(_) => trex::BOX_TYPE,
            Self::Leva(_) => leva::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Mehd(b) => b.box_size(),
            Self::Trex(b) => b.box_size(),
            Self::Leva(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl MovieExtendsChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Mehd(b) => b.write_to(writer),
            Self::Trex(b) => b.write_to(writer),
            Self::Leva(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for MovieExtendsChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            mehd::BOX_TYPE => match MovieExtendsHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Mehd(MovieExtendsHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            trex::BOX_TYPE => match TrackExtendsBoxView::new(raw.data()) {
                Ok(v) => Self::Trex(TrackExtendsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            leva::BOX_TYPE => match LevelAssignmentBoxView::new(raw.data()) {
                Ok(v) => match LevelAssignmentBoxOwned::try_from(&v) {
                    Ok(owned) => Self::Leva(owned),
                    Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
                },
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&MovieExtendsChild> for MovieExtendsChild {
    fn from(source: &MovieExtendsChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing MovieExtendsBox data.
pub trait MovieExtendsBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<MovieExtendsChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw MovieExtendsBox bytes.
#[derive(Clone, Copy)]
pub struct MovieExtendsBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> MovieExtendsBoxView<'a> {
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

    /// Returns the first MovieExtendsHeaderBox child, if present.
    pub fn mehd(&self) -> Option<MovieExtendsHeaderBoxView<'a>> {
        self.children()
            .find_as::<MovieExtendsHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns an iterator over TrackExtendsBox children.
    pub fn trex(&self) -> impl Iterator<Item = TrackExtendsBoxView<'a>> {
        self.children()
            .filter_as::<TrackExtendsBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the first LevelAssignmentBox child, if present.
    pub fn leva(&self) -> Option<LevelAssignmentBoxView<'a>> {
        self.children()
            .find_as::<LevelAssignmentBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> MovieExtendsBox for MovieExtendsBoxView<'a> {
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

impl std::fmt::Debug for MovieExtendsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MovieExtendsBoxView")
            .field("box_size", &self.box_size())
            .finish()
    }
}

/// An owned representation of MovieExtendsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MovieExtendsBoxOwned {
    /// Typed child boxes.
    pub children: Vec<MovieExtendsChild>,
}

impl MovieExtendsBoxOwned {
    /// Creates a new empty MovieExtendsBoxOwned.
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

    /// Returns the first MovieExtendsHeaderBox child, if present.
    pub fn mehd(&self) -> Option<&MovieExtendsHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieExtendsChild::Mehd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over TrackExtendsBox children.
    pub fn trex(&self) -> impl Iterator<Item = &TrackExtendsBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            MovieExtendsChild::Trex(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first LevelAssignmentBox child, if present.
    pub fn leva(&self) -> Option<&LevelAssignmentBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieExtendsChild::Leva(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a MovieExtendsHeaderBox child.
    pub fn add_mehd(&mut self, b: MovieExtendsHeaderBoxOwned) {
        self.children.push(MovieExtendsChild::Mehd(b));
    }

    /// Adds a TrackExtendsBox child.
    pub fn add_trex(&mut self, b: TrackExtendsBoxOwned) {
        self.children.push(MovieExtendsChild::Trex(b));
    }

    /// Adds a LevelAssignmentBox child.
    pub fn add_leva(&mut self, b: LevelAssignmentBoxOwned) {
        self.children.push(MovieExtendsChild::Leva(b));
    }
}

impl MovieExtendsBox for MovieExtendsBoxOwned {
    type Child<'a> = &'a MovieExtendsChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &MovieExtendsChild> {
        self.children.iter()
    }
}

impl<T: MovieExtendsBox> From<&T> for MovieExtendsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_mvex() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"mvex");

        let view = MovieExtendsBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }
}
