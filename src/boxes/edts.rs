//! Edit Box (edts) parsing and serialization.
//!
//! The Edit Box maps the presentation time-line to the media time-line.
//!
//! ```text
//! aligned(8) class EditBox extends Box('edts') {
//! }
//! ```

use crate::boxes::elst::{self, EditListBox as _, EditListBoxOwned, EditListBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for EditBox.
pub const BOX_TYPE: BoxCode = BoxCode::EDTS;

/// A typed child of an EditBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditChild {
    /// An EditListBox child.
    Elst(EditListBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for EditChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Elst(_) => elst::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Elst(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl EditChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Elst(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for EditChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            elst::BOX_TYPE => match EditListBoxView::new(raw.data()) {
                Ok(v) => Self::Elst(EditListBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&EditChild> for EditChild {
    fn from(source: &EditChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing EditBox data.
pub trait EditBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<EditChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw EditBox bytes.
#[derive(Clone, Copy)]
pub struct EditBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> EditBoxView<'a> {
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

    /// Returns the first EditListBox child, if present.
    pub fn elst(&self) -> Option<EditListBoxView<'a>> {
        self.children()
            .find_as::<EditListBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> EditBox for EditBoxView<'a> {
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

impl std::fmt::Debug for EditBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditBoxView")
            .field("box_size", &self.box_size())
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of EditBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EditBoxOwned {
    /// Typed child boxes.
    pub children: Vec<EditChild>,
}

impl EditBoxOwned {
    /// Creates a new empty EditBoxOwned.
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

    /// Returns the first EditListBox child, if present.
    pub fn elst(&self) -> Option<&EditListBoxOwned> {
        self.children.iter().find_map(|c| match c {
            EditChild::Elst(b) => Some(b),
            _ => None,
        })
    }

    /// Adds an EditListBox child.
    pub fn add_elst(&mut self, b: EditListBoxOwned) {
        self.children.push(EditChild::Elst(b));
    }
}

impl EditBox for EditBoxOwned {
    type Child<'a> = &'a EditChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &EditChild> {
        self.children.iter()
    }
}

impl<T: EditBox> From<&T> for EditBoxOwned {
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
    fn parse_empty_edts() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"edts");

        let view = EditBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"edts");
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"elst");
        data.extend_from_slice(&[0u8; 8]);

        let view = EditBoxView::new(&data).unwrap();
        let owned = EditBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
