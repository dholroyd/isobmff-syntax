//! Partition Entry Box (paen) parsing and serialization.
//!
//! The Partition Entry Box is a container for file partition information.
//!
//! ```text
//! aligned(8) class PartitionEntry extends Box('paen') {
//!    FilePartitionBox blocks_and_symbols;
//!    FECReservoirBox FEC_symbol_locations; //optional
//!    FileReservoirBox File_symbol_locations; //optional
//! }
//! ```

use crate::boxes::fecr::{
    self, FECReservoirBox as _, FECReservoirBoxOwned, FECReservoirBoxView,
};
use crate::boxes::fire::{
    self, FileReservoirBox as _, FileReservoirBoxOwned, FileReservoirBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for PartitionEntryBox.
pub const BOX_TYPE: BoxCode = BoxCode::PAEN;

/// A typed child of a PartitionEntryBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartitionEntryChild {
    /// An FECReservoirBox child.
    Fecr(FECReservoirBoxOwned),
    /// A FileReservoirBox child.
    Fire(FileReservoirBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for PartitionEntryChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Fecr(_) => fecr::BOX_TYPE,
            Self::Fire(_) => fire::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Fecr(b) => b.box_size(),
            Self::Fire(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl PartitionEntryChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Fecr(b) => b.write_to(writer),
            Self::Fire(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for PartitionEntryChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            fecr::BOX_TYPE => match FECReservoirBoxView::new(raw.data()) {
                Ok(v) => Self::Fecr(FECReservoirBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            fire::BOX_TYPE => match FileReservoirBoxView::new(raw.data()) {
                Ok(v) => Self::Fire(FileReservoirBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&PartitionEntryChild> for PartitionEntryChild {
    fn from(source: &PartitionEntryChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing PartitionEntryBox data.
pub trait PartitionEntryBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<PartitionEntryChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw PartitionEntryBox bytes.
#[derive(Clone, Copy)]
pub struct PartitionEntryBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> PartitionEntryBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 0)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.header_size..])
    }

    /// Returns the first FECReservoirBox child, if present.
    pub fn fecr(&self) -> Option<FECReservoirBoxView<'a>> {
        self.children()
            .find_as::<FECReservoirBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first FileReservoirBox child, if present.
    pub fn fire(&self) -> Option<FileReservoirBoxView<'a>> {
        self.children()
            .find_as::<FileReservoirBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> PartitionEntryBox for PartitionEntryBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for PartitionEntryBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartitionEntryBoxView")
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of PartitionEntryBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PartitionEntryBoxOwned {
    /// Typed child boxes.
    pub children: Vec<PartitionEntryChild>,
}

impl PartitionEntryBoxOwned {
    /// Creates a new PartitionEntryBoxOwned.
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

    /// Returns the first FECReservoirBox child, if present.
    pub fn fecr(&self) -> Option<&FECReservoirBoxOwned> {
        self.children.iter().find_map(|c| match c {
            PartitionEntryChild::Fecr(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first FileReservoirBox child, if present.
    pub fn fire(&self) -> Option<&FileReservoirBoxOwned> {
        self.children.iter().find_map(|c| match c {
            PartitionEntryChild::Fire(b) => Some(b),
            _ => None,
        })
    }

    /// Adds an FECReservoirBox child.
    pub fn add_fecr(&mut self, b: FECReservoirBoxOwned) {
        self.children.push(PartitionEntryChild::Fecr(b));
    }

    /// Adds a FileReservoirBox child.
    pub fn add_fire(&mut self, b: FileReservoirBoxOwned) {
        self.children.push(PartitionEntryChild::Fire(b));
    }
}

impl PartitionEntryBox for PartitionEntryBoxOwned {
    type Child<'a> = &'a PartitionEntryChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &PartitionEntryChild> {
        self.children.iter()
    }
}

impl<T: PartitionEntryBox> From<&T> for PartitionEntryBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_paen() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"paen");
        data
    }

    #[test]
    fn parse_empty_paen() {
        let data = make_paen();
        let view = PartitionEntryBoxView::new(&data).unwrap();
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_paen();
        let view = PartitionEntryBoxView::new(&data).unwrap();
        let owned = PartitionEntryBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
