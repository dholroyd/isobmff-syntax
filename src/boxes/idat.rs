//! Item Data Box (idat) parsing and serialization.
//!
//! The Item Data Box contains data for items that are stored within the metadata.
//!
//! ```text
//! aligned(8) class ItemDataBox extends Box('idat') {
//!    bit(8) data[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemDataBox.
pub const BOX_TYPE: BoxCode = BoxCode::IDAT;

/// Common interface for accessing ItemDataBox data.
pub trait ItemDataBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the data contained in the box.
    fn data(&self) -> &[u8];
}

/// A borrowing view over raw ItemDataBox bytes.
#[derive(Clone, Copy)]
pub struct ItemDataBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> ItemDataBoxView<'a> {
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
}

impl<'a> ItemDataBox for ItemDataBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn data(&self) -> &[u8] {
        &self.data[self.header_size..]
    }
}

impl std::fmt::Debug for ItemDataBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemDataBoxView")
            .field("data_len", &self.data().len())
            .finish()
    }
}

/// An owned representation of ItemDataBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemDataBoxOwned {
    /// The contained data.
    pub data: Vec<u8>,
}

impl ItemDataBoxOwned {
    /// Creates a new ItemDataBoxOwned.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.data.len()) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_all(&self.data)?;

        Ok(())
    }
}

impl Default for ItemDataBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ItemDataBox for ItemDataBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

impl<T: ItemDataBox> From<&T> for ItemDataBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            data: source.data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_idat() -> Vec<u8> {
        let payload = b"hello world";
        let mut data = Vec::new();
        data.extend_from_slice(&((8 + payload.len()) as u32).to_be_bytes());
        data.extend_from_slice(b"idat");
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn parse_idat() {
        let data = make_idat();
        let view = ItemDataBoxView::new(&data).unwrap();

        assert_eq!(view.data(), b"hello world");
    }

    #[test]
    fn roundtrip() {
        let data = make_idat();
        let view = ItemDataBoxView::new(&data).unwrap();
        let owned = ItemDataBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
