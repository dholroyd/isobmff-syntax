//! Movie Fragment Box (moof) parsing and serialization.
//!
//! The Movie Fragment Box extends the movie in time.
//!
//! ```text
//! aligned(8) class MovieFragmentBox extends Box('moof') {
//! }
//! ```

use crate::boxes::meta::{
    self, MetadataBox as _, MetadataBoxOwned, MetadataBoxView,
};
use crate::boxes::mfhd::{
    self, MovieFragmentHeaderBox as _, MovieFragmentHeaderBoxOwned, MovieFragmentHeaderBoxView,
};
use crate::boxes::traf::{
    self, TrackFragmentBox as _, TrackFragmentBoxOwned, TrackFragmentBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieFragmentBox.
pub const BOX_TYPE: BoxCode = BoxCode::MOOF;

/// A typed child of a MovieFragmentBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MovieFragmentChild {
    /// A MovieFragmentHeaderBox child.
    Mfhd(MovieFragmentHeaderBoxOwned),
    /// A TrackFragmentBox child.
    Traf(TrackFragmentBoxOwned),
    /// A MetadataBox child.
    Meta(MetadataBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for MovieFragmentChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Mfhd(_) => mfhd::BOX_TYPE,
            Self::Traf(_) => traf::BOX_TYPE,
            Self::Meta(_) => meta::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Mfhd(b) => b.box_size(),
            Self::Traf(b) => b.box_size(),
            Self::Meta(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl MovieFragmentChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Mfhd(b) => b.write_to(writer),
            Self::Traf(b) => b.write_to(writer),
            Self::Meta(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for MovieFragmentChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            mfhd::BOX_TYPE => match MovieFragmentHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Mfhd(MovieFragmentHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            traf::BOX_TYPE => match TrackFragmentBoxView::new(raw.data()) {
                Ok(v) => Self::Traf(TrackFragmentBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            meta::BOX_TYPE => match MetadataBoxView::new(raw.data()) {
                Ok(v) => Self::Meta(MetadataBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&MovieFragmentChild> for MovieFragmentChild {
    fn from(source: &MovieFragmentChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing MovieFragmentBox data.
pub trait MovieFragmentBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<MovieFragmentChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw MovieFragmentBox bytes.
#[derive(Clone, Copy)]
pub struct MovieFragmentBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> MovieFragmentBoxView<'a> {
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

    /// Returns the first MovieFragmentHeaderBox child, if present.
    pub fn mfhd(&self) -> Option<MovieFragmentHeaderBoxView<'a>> {
        self.children()
            .find_as::<MovieFragmentHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns an iterator over TrackFragmentBox children.
    pub fn traf(&self) -> impl Iterator<Item = TrackFragmentBoxView<'a>> {
        self.children()
            .filter_as::<TrackFragmentBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<MetadataBoxView<'a>> {
        self.children()
            .find_as::<MetadataBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> MovieFragmentBox for MovieFragmentBoxView<'a> {
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

impl std::fmt::Debug for MovieFragmentBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("MovieFragmentBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of MovieFragmentBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MovieFragmentBoxOwned {
    /// Typed child boxes.
    pub children: Vec<MovieFragmentChild>,
}

impl MovieFragmentBoxOwned {
    /// Creates a new empty MovieFragmentBoxOwned.
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

    /// Returns the first MovieFragmentHeaderBox child, if present.
    pub fn mfhd(&self) -> Option<&MovieFragmentHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieFragmentChild::Mfhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over TrackFragmentBox children.
    pub fn traf(&self) -> impl Iterator<Item = &TrackFragmentBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            MovieFragmentChild::Traf(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<&MetadataBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieFragmentChild::Meta(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a MovieFragmentHeaderBox child.
    pub fn add_mfhd(&mut self, b: MovieFragmentHeaderBoxOwned) {
        self.children.push(MovieFragmentChild::Mfhd(b));
    }

    /// Adds a TrackFragmentBox child.
    pub fn add_traf(&mut self, b: TrackFragmentBoxOwned) {
        self.children.push(MovieFragmentChild::Traf(b));
    }

    /// Adds a MetadataBox child.
    pub fn add_meta(&mut self, b: MetadataBoxOwned) {
        self.children.push(MovieFragmentChild::Meta(b));
    }
}

impl MovieFragmentBox for MovieFragmentBoxOwned {
    type Child<'a> = &'a MovieFragmentChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &MovieFragmentChild> {
        self.children.iter()
    }
}

impl<T: MovieFragmentBox> From<&T> for MovieFragmentBoxOwned {
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
    fn parse_empty_moof() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"moof");

        let view = MovieFragmentBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }
}
