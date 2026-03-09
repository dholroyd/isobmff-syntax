//! Track Aperture Mode Dimensions Box (tapt) parsing and serialization.
//!
//! The Track Aperture Mode Dimensions Box is a container for aperture mode
//! dimension boxes (clef, prof, enof).
//!
//! ```text
//! aligned(8) class TrackApertureModeDimensionsBox
//!    extends Box('tapt') {
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackApertureBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"tapt");

/// A typed child of a TrackApertureBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackApertureChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for TrackApertureChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Other(o) => o.box_size(),
        }
    }
}

impl TrackApertureChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for TrackApertureChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&TrackApertureChild> for TrackApertureChild {
    fn from(source: &TrackApertureChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing TrackApertureBox data.
pub trait TrackApertureBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<TrackApertureChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw TrackApertureBox bytes.
#[derive(Clone, Copy)]
pub struct TrackApertureBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> TrackApertureBoxView<'a> {
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
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.header.header_size as usize..])
    }
}

impl<'a> TrackApertureBox for TrackApertureBoxView<'a> {
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

impl std::fmt::Debug for TrackApertureBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackApertureBoxView")
            .field("box_size", &self.box_size())
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of TrackApertureBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TrackApertureBoxOwned {
    /// Typed child boxes.
    pub children: Vec<TrackApertureChild>,
}

impl TrackApertureBoxOwned {
    /// Creates a new empty TrackApertureBoxOwned.
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
}

impl TrackApertureBox for TrackApertureBoxOwned {
    type Child<'a> = &'a TrackApertureChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &TrackApertureChild> {
        self.children.iter()
    }
}

impl<T: TrackApertureBox> From<&T> for TrackApertureBoxOwned {
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
    fn parse_empty_tapt() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"tapt");

        let view = TrackApertureBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"tapt");

        let view = TrackApertureBoxView::new(&data).unwrap();
        let owned = TrackApertureBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
