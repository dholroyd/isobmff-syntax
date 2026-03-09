//! Scheme Information Box (schi) parsing and serialization.
//!
//! The Scheme Information Box contains scheme-specific information.
//!
//! ```text
//! aligned(8) class SchemeInformationBox extends Box('schi') {
//!    Box scheme_specific_data[];
//! }
//! ```

use crate::boxes::tenc::{self, TrackEncryptionBox as _, TrackEncryptionBoxOwned, TrackEncryptionBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SchemeInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::SCHI;

/// A typed child of a SchemeInformationBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemeInformationChild {
    /// A TrackEncryptionBox child.
    Tenc(TrackEncryptionBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for SchemeInformationChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Tenc(_) => tenc::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Tenc(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl SchemeInformationChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Tenc(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for SchemeInformationChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            tenc::BOX_TYPE => match TrackEncryptionBoxView::new(raw.data()) {
                Ok(v) => Self::Tenc(TrackEncryptionBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&SchemeInformationChild> for SchemeInformationChild {
    fn from(source: &SchemeInformationChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing SchemeInformationBox data.
pub trait SchemeInformationBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<SchemeInformationChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw SchemeInformationBox bytes.
#[derive(Clone, Copy)]
pub struct SchemeInformationBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> SchemeInformationBoxView<'a> {
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

    /// Returns the TrackEncryptionBox child, if present.
    pub fn tenc(&self) -> Option<TrackEncryptionBoxView<'a>> {
        self.children()
            .find_as::<TrackEncryptionBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> SchemeInformationBox for SchemeInformationBoxView<'a> {
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

impl std::fmt::Debug for SchemeInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("SchemeInformationBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of SchemeInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SchemeInformationBoxOwned {
    /// Typed child boxes.
    pub children: Vec<SchemeInformationChild>,
}

impl SchemeInformationBoxOwned {
    /// Creates a new empty SchemeInformationBoxOwned.
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

    /// Returns the TrackEncryptionBox child, if present.
    pub fn tenc(&self) -> Option<&TrackEncryptionBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SchemeInformationChild::Tenc(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a TrackEncryptionBox child.
    pub fn add_tenc(&mut self, b: TrackEncryptionBoxOwned) {
        self.children.push(SchemeInformationChild::Tenc(b));
    }
}

impl SchemeInformationBox for SchemeInformationBoxOwned {
    type Child<'a> = &'a SchemeInformationChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &SchemeInformationChild> {
        self.children.iter()
    }
}

impl<T: SchemeInformationBox> From<&T> for SchemeInformationBoxOwned {
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
    fn parse_empty_schi() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"schi");

        let view = SchemeInformationBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"schi");

        let view = SchemeInformationBoxView::new(&data).unwrap();
        let owned = SchemeInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
