//! Media Box (mdia) parsing and serialization.
//!
//! The Media Box contains all the objects that declare information about the media data.
//!
//! ```text
//! aligned(8) class MediaBox extends Box('mdia') {
//! }
//! ```

use crate::boxes::elng::{
    self, ExtendedLanguageTagBox as _, ExtendedLanguageTagBoxOwned, ExtendedLanguageTagBoxView,
};
use crate::boxes::hdlr::{
    self, HandlerReferenceBox as _, HandlerReferenceBoxOwned, HandlerReferenceBoxView,
};
use crate::boxes::mdhd::{self, MediaHeaderBox as _, MediaHeaderBoxOwned, MediaHeaderBoxView};
use crate::boxes::minf::{
    self, MediaInformationBox as _, MediaInformationBoxOwned, MediaInformationBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MediaBox.
pub const BOX_TYPE: BoxCode = BoxCode::MDIA;

/// A typed child of a MediaBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaChild {
    /// A MediaHeaderBox child.
    Mdhd(MediaHeaderBoxOwned),
    /// A HandlerReferenceBox child.
    Hdlr(HandlerReferenceBoxOwned),
    /// A MediaInformationBox child.
    Minf(MediaInformationBoxOwned),
    /// An ExtendedLanguageTagBox child.
    Elng(ExtendedLanguageTagBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for MediaChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Mdhd(_) => mdhd::BOX_TYPE,
            Self::Hdlr(_) => hdlr::BOX_TYPE,
            Self::Minf(_) => minf::BOX_TYPE,
            Self::Elng(_) => elng::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Mdhd(b) => b.box_size(),
            Self::Hdlr(b) => b.box_size(),
            Self::Minf(b) => b.box_size(),
            Self::Elng(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl MediaChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Mdhd(b) => b.write_to(writer),
            Self::Hdlr(b) => b.write_to(writer),
            Self::Minf(b) => b.write_to(writer),
            Self::Elng(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for MediaChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            mdhd::BOX_TYPE => match MediaHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Mdhd(MediaHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            hdlr::BOX_TYPE => match HandlerReferenceBoxView::new(raw.data()) {
                Ok(v) => Self::Hdlr(HandlerReferenceBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            minf::BOX_TYPE => match MediaInformationBoxView::new(raw.data()) {
                Ok(v) => Self::Minf(MediaInformationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            elng::BOX_TYPE => match ExtendedLanguageTagBoxView::new(raw.data()) {
                Ok(v) => Self::Elng(ExtendedLanguageTagBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&MediaChild> for MediaChild {
    fn from(source: &MediaChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing MediaBox data.
pub trait MediaBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<MediaChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw MediaBox bytes.
#[derive(Clone, Copy)]
pub struct MediaBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> MediaBoxView<'a> {
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

    /// Returns the first MediaHeaderBox child, if present.
    pub fn mdhd(&self) -> Option<MediaHeaderBoxView<'a>> {
        self.children()
            .find_as::<MediaHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first HandlerReferenceBox child, if present.
    pub fn hdlr(&self) -> Option<HandlerReferenceBoxView<'a>> {
        self.children()
            .find_as::<HandlerReferenceBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first MediaInformationBox child, if present.
    pub fn minf(&self) -> Option<MediaInformationBoxView<'a>> {
        self.children()
            .find_as::<MediaInformationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first ExtendedLanguageTagBox child, if present.
    pub fn elng(&self) -> Option<ExtendedLanguageTagBoxView<'a>> {
        self.children()
            .find_as::<ExtendedLanguageTagBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> MediaBox for MediaBoxView<'a> {
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

impl std::fmt::Debug for MediaBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("MediaBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of MediaBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MediaBoxOwned {
    /// Typed child boxes.
    pub children: Vec<MediaChild>,
}

impl MediaBoxOwned {
    /// Creates a new empty MediaBoxOwned.
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

    /// Returns the first MediaHeaderBox child, if present.
    pub fn mdhd(&self) -> Option<&MediaHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaChild::Mdhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first HandlerReferenceBox child, if present.
    pub fn hdlr(&self) -> Option<&HandlerReferenceBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaChild::Hdlr(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first MediaInformationBox child, if present.
    pub fn minf(&self) -> Option<&MediaInformationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaChild::Minf(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first ExtendedLanguageTagBox child, if present.
    pub fn elng(&self) -> Option<&ExtendedLanguageTagBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaChild::Elng(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a MediaHeaderBox child.
    pub fn add_mdhd(&mut self, b: MediaHeaderBoxOwned) {
        self.children.push(MediaChild::Mdhd(b));
    }

    /// Adds a HandlerReferenceBox child.
    pub fn add_hdlr(&mut self, b: HandlerReferenceBoxOwned) {
        self.children.push(MediaChild::Hdlr(b));
    }

    /// Adds a MediaInformationBox child.
    pub fn add_minf(&mut self, b: MediaInformationBoxOwned) {
        self.children.push(MediaChild::Minf(b));
    }

    /// Adds an ExtendedLanguageTagBox child.
    pub fn add_elng(&mut self, b: ExtendedLanguageTagBoxOwned) {
        self.children.push(MediaChild::Elng(b));
    }
}

impl MediaBox for MediaBoxOwned {
    type Child<'a> = &'a MediaChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &MediaChild> {
        self.children.iter()
    }
}

impl<T: MediaBox> From<&T> for MediaBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mdia_with_children(children: &[u8]) -> Vec<u8> {
        let size = 8 + children.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"mdia");
        data.extend_from_slice(children);
        data
    }

    #[test]
    fn parse_empty_mdia() {
        let data = make_mdia_with_children(&[]);
        let view = MediaBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let data = make_mdia_with_children(&[0u8; 16]);
        let view = MediaBoxView::new(&data).unwrap();
        let owned = MediaBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
