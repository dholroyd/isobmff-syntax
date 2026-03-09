//! AV1 Operating Point Selector Box (a1op) parsing and serialization.
//!
//! The AV1 Operating Point Selector Box specifies the operating point for AV1 images.

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::WriteBytesExt;
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for AV1OperatingPointSelectorBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"a1op");

/// Common interface for accessing AV1OperatingPointSelectorBox data.
pub trait AV1OperatingPointSelectorBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the operating point index.
    fn op_index(&self) -> u8;
}

/// A borrowing view over raw AV1OperatingPointSelectorBox bytes.
#[derive(Clone, Copy)]
pub struct AV1OperatingPointSelectorBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> AV1OperatingPointSelectorBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 1)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> AV1OperatingPointSelectorBox for AV1OperatingPointSelectorBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn op_index(&self) -> u8 {
        self.data[self.header_size]
    }
}

impl std::fmt::Debug for AV1OperatingPointSelectorBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AV1OperatingPointSelectorBoxView")
            .field("op_index", &self.op_index())
            .finish()
    }
}

/// An owned representation of AV1OperatingPointSelectorBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AV1OperatingPointSelectorBoxOwned {
    /// Operating point index.
    pub op_index: u8,
}

impl AV1OperatingPointSelectorBoxOwned {
    /// Creates a new AV1OperatingPointSelectorBoxOwned.
    pub fn new(op_index: u8) -> Self {
        Self { op_index }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(1) + 1 // 8 + 1
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_u8(self.op_index)?;

        Ok(())
    }
}

impl Default for AV1OperatingPointSelectorBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl AV1OperatingPointSelectorBox for AV1OperatingPointSelectorBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn op_index(&self) -> u8 {
        self.op_index
    }
}

impl<T: AV1OperatingPointSelectorBox> From<&T> for AV1OperatingPointSelectorBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            op_index: source.op_index(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_a1op() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&9u32.to_be_bytes());
        data.extend_from_slice(b"a1op");
        data.push(0); // op_index
        data
    }

    #[test]
    fn parse_a1op() {
        let data = make_a1op();
        let view = AV1OperatingPointSelectorBoxView::new(&data).unwrap();
        assert_eq!(view.op_index(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_a1op();
        let view = AV1OperatingPointSelectorBoxView::new(&data).unwrap();
        let owned = AV1OperatingPointSelectorBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
