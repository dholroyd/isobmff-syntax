//! Movie Fragment Random Access Box (mfra) parsing and serialization.
//!
//! The Movie Fragment Random Access Box provides random access points
//! within a fragmented movie.
//!
//! ```text
//! aligned(8) class MovieFragmentRandomAccessBox
//!    extends Box('mfra') {
//! }
//! ```

use crate::boxes::mfro::{
    self, MovieFragmentRandomAccessOffsetBox as _, MovieFragmentRandomAccessOffsetBoxOwned,
    MovieFragmentRandomAccessOffsetBoxView,
};
use crate::boxes::tfra::{
    self, TrackFragmentRandomAccessBox as _, TrackFragmentRandomAccessBoxOwned,
    TrackFragmentRandomAccessBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MovieFragmentRandomAccessBox.
pub const BOX_TYPE: BoxCode = BoxCode::MFRA;

/// A typed child of a MovieFragmentRandomAccessBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MovieFragmentRandomAccessChild {
    /// A TrackFragmentRandomAccessBox child.
    Tfra(TrackFragmentRandomAccessBoxOwned),
    /// A MovieFragmentRandomAccessOffsetBox child.
    Mfro(MovieFragmentRandomAccessOffsetBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for MovieFragmentRandomAccessChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Tfra(_) => tfra::BOX_TYPE,
            Self::Mfro(_) => mfro::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Tfra(b) => b.box_size(),
            Self::Mfro(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl MovieFragmentRandomAccessChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Tfra(b) => b.write_to(writer),
            Self::Mfro(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for MovieFragmentRandomAccessChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            tfra::BOX_TYPE => match TrackFragmentRandomAccessBoxView::new(raw.data()) {
                Ok(v) => Self::Tfra(TrackFragmentRandomAccessBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            mfro::BOX_TYPE => match MovieFragmentRandomAccessOffsetBoxView::new(raw.data()) {
                Ok(v) => Self::Mfro(MovieFragmentRandomAccessOffsetBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&MovieFragmentRandomAccessChild> for MovieFragmentRandomAccessChild {
    fn from(source: &MovieFragmentRandomAccessChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing MovieFragmentRandomAccessBox data.
pub trait MovieFragmentRandomAccessBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<MovieFragmentRandomAccessChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw MovieFragmentRandomAccessBox bytes.
#[derive(Clone, Copy)]
pub struct MovieFragmentRandomAccessBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> MovieFragmentRandomAccessBoxView<'a> {
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

    /// Returns all TrackFragmentRandomAccessBox children.
    pub fn tfras(&self) -> impl Iterator<Item = TrackFragmentRandomAccessBoxView<'a>> {
        self.children()
            .filter_as::<TrackFragmentRandomAccessBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the first MovieFragmentRandomAccessOffsetBox child, if present.
    pub fn mfro(&self) -> Option<MovieFragmentRandomAccessOffsetBoxView<'a>> {
        self.children()
            .find_as::<MovieFragmentRandomAccessOffsetBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> MovieFragmentRandomAccessBox for MovieFragmentRandomAccessBoxView<'a> {
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

impl std::fmt::Debug for MovieFragmentRandomAccessBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self.children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("MovieFragmentRandomAccessBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of MovieFragmentRandomAccessBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MovieFragmentRandomAccessBoxOwned {
    /// Typed child boxes.
    pub children: Vec<MovieFragmentRandomAccessChild>,
}

impl MovieFragmentRandomAccessBoxOwned {
    /// Creates a new empty MovieFragmentRandomAccessBoxOwned.
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

    /// Returns all TrackFragmentRandomAccessBox children.
    pub fn tfras(&self) -> impl Iterator<Item = &TrackFragmentRandomAccessBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            MovieFragmentRandomAccessChild::Tfra(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first MovieFragmentRandomAccessOffsetBox child, if present.
    pub fn mfro(&self) -> Option<&MovieFragmentRandomAccessOffsetBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MovieFragmentRandomAccessChild::Mfro(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a TrackFragmentRandomAccessBox child.
    pub fn add_tfra(&mut self, b: TrackFragmentRandomAccessBoxOwned) {
        self.children.push(MovieFragmentRandomAccessChild::Tfra(b));
    }

    /// Adds a MovieFragmentRandomAccessOffsetBox child.
    pub fn add_mfro(&mut self, b: MovieFragmentRandomAccessOffsetBoxOwned) {
        self.children.push(MovieFragmentRandomAccessChild::Mfro(b));
    }
}

impl MovieFragmentRandomAccessBox for MovieFragmentRandomAccessBoxOwned {
    type Child<'a> = &'a MovieFragmentRandomAccessChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &MovieFragmentRandomAccessChild> {
        self.children.iter()
    }
}

impl<T: MovieFragmentRandomAccessBox> From<&T> for MovieFragmentRandomAccessBoxOwned {
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
    fn parse_empty_mfra() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"mfra");

        let view = MovieFragmentRandomAccessBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"mfra");

        let view = MovieFragmentRandomAccessBoxView::new(&data).unwrap();
        let owned = MovieFragmentRandomAccessBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
