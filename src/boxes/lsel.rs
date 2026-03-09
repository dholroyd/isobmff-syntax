//! Layer Selection Box (lsel) parsing and serialization.
//!
//! The Layer Selection Box specifies the layer to be displayed for multi-layer images.
//!
//! ```text
//! aligned(8) class LayerSelectorProperty
//!    extends ItemFullProperty('lsel', version = 0, 0) {
//!    unsigned int(16) layer_id;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for LayerSelectionBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"lsel");

/// Common interface for accessing LayerSelectionBox data.
pub trait LayerSelectionBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the layer ID to be displayed.
    fn layer_id(&self) -> u16;
}

/// A borrowing view over raw LayerSelectionBox bytes.
#[derive(Clone, Copy)]
pub struct LayerSelectionBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> LayerSelectionBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 2)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> LayerSelectionBox for LayerSelectionBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn layer_id(&self) -> u16 {
        let o = self.fullbox_offset + 4; // after version/flags
        BigEndian::read_u16(&self.data[o..o + 2])
    }
}

impl std::fmt::Debug for LayerSelectionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerSelectionBoxView")
            .field("layer_id", &self.layer_id())
            .finish()
    }
}

/// An owned representation of LayerSelectionBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerSelectionBoxOwned {
    /// Layer ID to be displayed.
    pub layer_id: u16,
}

impl LayerSelectionBoxOwned {
    /// Creates a new LayerSelectionBoxOwned.
    pub fn new(layer_id: u16) -> Self {
        Self { layer_id }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(2) + 2 // 12 + 2
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, 0)?;
        writer.write_u16::<BigEndian>(self.layer_id)?;

        Ok(())
    }
}

impl Default for LayerSelectionBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl LayerSelectionBox for LayerSelectionBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn layer_id(&self) -> u16 {
        self.layer_id
    }
}

impl<T: LayerSelectionBox> From<&T> for LayerSelectionBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            layer_id: source.layer_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lsel() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&14u32.to_be_bytes()); // 8 + 4 + 2
        data.extend_from_slice(b"lsel");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u16.to_be_bytes()); // layer_id
        data
    }

    #[test]
    fn parse_lsel() {
        let data = make_lsel();
        let view = LayerSelectionBoxView::new(&data).unwrap();

        assert_eq!(view.layer_id(), 1);
    }

    #[test]
    fn roundtrip() {
        let data = make_lsel();
        let view = LayerSelectionBoxView::new(&data).unwrap();
        let owned = LayerSelectionBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
