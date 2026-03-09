//! Protection Scheme Information Box (sinf) parsing and serialization.
//!
//! The Protection Scheme Information Box contains all the information
//! required both to understand the encryption transform applied and its parameters.
//!
//! ```text
//! aligned(8) class ProtectionSchemeInfoBox(fmt)
//!    extends Box('sinf') {
//!    OriginalFormatBox(fmt) original_format;
//!    SchemeTypeBox scheme_type_box; // optional
//!    SchemeInformationBox info; // optional
//! }
//! ```

use crate::boxes::frma::{self, OriginalFormatBox as _, OriginalFormatBoxOwned, OriginalFormatBoxView};
use crate::boxes::schm::{self, SchemeTypeBox as _, SchemeTypeBoxOwned, SchemeTypeBoxView};
use crate::boxes::schi::{self, SchemeInformationBox as _, SchemeInformationBoxOwned, SchemeInformationBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ProtectionSchemeInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::SINF;

/// A typed child of a ProtectionSchemeInfoBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtectionSchemeInfoChild {
    /// An OriginalFormatBox child.
    Frma(OriginalFormatBoxOwned),
    /// A SchemeTypeBox child.
    Schm(SchemeTypeBoxOwned),
    /// A SchemeInformationBox child.
    Schi(SchemeInformationBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for ProtectionSchemeInfoChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Frma(_) => frma::BOX_TYPE,
            Self::Schm(_) => schm::BOX_TYPE,
            Self::Schi(_) => schi::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Frma(b) => b.box_size(),
            Self::Schm(b) => b.box_size(),
            Self::Schi(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl ProtectionSchemeInfoChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Frma(b) => b.write_to(writer),
            Self::Schm(b) => b.write_to(writer),
            Self::Schi(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for ProtectionSchemeInfoChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            frma::BOX_TYPE => match OriginalFormatBoxView::new(raw.data()) {
                Ok(v) => Self::Frma(OriginalFormatBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            schm::BOX_TYPE => match SchemeTypeBoxView::new(raw.data()) {
                Ok(v) => Self::Schm(SchemeTypeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            schi::BOX_TYPE => match SchemeInformationBoxView::new(raw.data()) {
                Ok(v) => Self::Schi(SchemeInformationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&ProtectionSchemeInfoChild> for ProtectionSchemeInfoChild {
    fn from(source: &ProtectionSchemeInfoChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing ProtectionSchemeInfoBox data.
pub trait ProtectionSchemeInfoBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<ProtectionSchemeInfoChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw ProtectionSchemeInfoBox bytes.
#[derive(Clone, Copy)]
pub struct ProtectionSchemeInfoBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> ProtectionSchemeInfoBoxView<'a> {
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

    /// Returns the first OriginalFormatBox child, if present.
    pub fn frma(&self) -> Option<OriginalFormatBoxView<'a>> {
        self.children()
            .find_as::<OriginalFormatBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first SchemeTypeBox child, if present.
    pub fn schm(&self) -> Option<SchemeTypeBoxView<'a>> {
        self.children()
            .find_as::<SchemeTypeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first SchemeInformationBox child, if present.
    pub fn schi(&self) -> Option<SchemeInformationBoxView<'a>> {
        self.children()
            .find_as::<SchemeInformationBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> ProtectionSchemeInfoBox for ProtectionSchemeInfoBoxView<'a> {
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

impl std::fmt::Debug for ProtectionSchemeInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("ProtectionSchemeInfoBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of ProtectionSchemeInfoBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProtectionSchemeInfoBoxOwned {
    /// Typed child boxes.
    pub children: Vec<ProtectionSchemeInfoChild>,
}

impl ProtectionSchemeInfoBoxOwned {
    /// Creates a new empty ProtectionSchemeInfoBoxOwned.
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

    /// Returns the first OriginalFormatBox child, if present.
    pub fn frma(&self) -> Option<&OriginalFormatBoxOwned> {
        self.children.iter().find_map(|c| match c {
            ProtectionSchemeInfoChild::Frma(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first SchemeTypeBox child, if present.
    pub fn schm(&self) -> Option<&SchemeTypeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            ProtectionSchemeInfoChild::Schm(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first SchemeInformationBox child, if present.
    pub fn schi(&self) -> Option<&SchemeInformationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            ProtectionSchemeInfoChild::Schi(b) => Some(b),
            _ => None,
        })
    }

    /// Adds an OriginalFormatBox child.
    pub fn add_frma(&mut self, b: OriginalFormatBoxOwned) {
        self.children.push(ProtectionSchemeInfoChild::Frma(b));
    }

    /// Adds a SchemeTypeBox child.
    pub fn add_schm(&mut self, b: SchemeTypeBoxOwned) {
        self.children.push(ProtectionSchemeInfoChild::Schm(b));
    }

    /// Adds a SchemeInformationBox child.
    pub fn add_schi(&mut self, b: SchemeInformationBoxOwned) {
        self.children.push(ProtectionSchemeInfoChild::Schi(b));
    }
}

impl ProtectionSchemeInfoBox for ProtectionSchemeInfoBoxOwned {
    type Child<'a> = &'a ProtectionSchemeInfoChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &ProtectionSchemeInfoChild> {
        self.children.iter()
    }
}

impl<T: ProtectionSchemeInfoBox> From<&T> for ProtectionSchemeInfoBoxOwned {
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
    fn parse_empty_sinf() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"sinf");

        let view = ProtectionSchemeInfoBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"sinf");

        let view = ProtectionSchemeInfoBoxView::new(&data).unwrap();
        let owned = ProtectionSchemeInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
