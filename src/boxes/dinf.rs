//! Data Information Box (dinf) parsing and serialization.
//!
//! The Data Information Box contains objects that declare the location of media data.
//!
//! ```text
//! aligned(8) class DataInformationBox extends Box('dinf') {
//! }
//! ```

use crate::boxes::dref::{self, DataReferenceBox as _, DataReferenceBoxOwned, DataReferenceBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for DataInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::DINF;

/// A typed child of a DataInformationBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataInformationChild {
    /// A DataReferenceBox child.
    Dref(DataReferenceBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for DataInformationChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Dref(_) => dref::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Dref(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl DataInformationChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Dref(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for DataInformationChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            dref::BOX_TYPE => match DataReferenceBoxView::new(raw.data()) {
                Ok(v) => Self::Dref(DataReferenceBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&DataInformationChild> for DataInformationChild {
    fn from(source: &DataInformationChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing DataInformationBox data.
pub trait DataInformationBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<DataInformationChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw DataInformationBox bytes.
#[derive(Clone, Copy)]
pub struct DataInformationBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> DataInformationBoxView<'a> {
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

    /// Returns the first DataReferenceBox child, if present.
    pub fn dref(&self) -> Option<DataReferenceBoxView<'a>> {
        self.children()
            .find_as::<DataReferenceBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> DataInformationBox for DataInformationBoxView<'a> {
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

impl std::fmt::Debug for DataInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataInformationBoxView")
            .field("box_size", &self.box_size())
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of DataInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DataInformationBoxOwned {
    /// Typed child boxes.
    pub children: Vec<DataInformationChild>,
}

impl DataInformationBoxOwned {
    /// Creates a new empty DataInformationBoxOwned.
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

    /// Returns the first DataReferenceBox child, if present.
    pub fn dref(&self) -> Option<&DataReferenceBoxOwned> {
        self.children.iter().find_map(|c| match c {
            DataInformationChild::Dref(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a DataReferenceBox child.
    pub fn add_dref(&mut self, b: DataReferenceBoxOwned) {
        self.children.push(DataInformationChild::Dref(b));
    }
}

impl DataInformationBox for DataInformationBoxOwned {
    type Child<'a> = &'a DataInformationChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &DataInformationChild> {
        self.children.iter()
    }
}

impl<T: DataInformationBox> From<&T> for DataInformationBoxOwned {
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
    fn parse_empty_dinf() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"dinf");

        let view = DataInformationBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"dinf");
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"dref");
        data.extend_from_slice(&[0u8; 8]);

        let view = DataInformationBoxView::new(&data).unwrap();
        let owned = DataInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
