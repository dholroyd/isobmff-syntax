//! AV1 Layer Configuration Box (a1lx) parsing and serialization.
//!
//! The AV1 Layer Configuration Box specifies layer configuration for AV1 images.

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for AV1LayerConfigurationBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"a1lx");

/// Common interface for accessing AV1LayerConfigurationBox data.
pub trait AV1LayerConfigurationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the large_size flag.
    fn large_size(&self) -> bool;

    /// Returns the layer sizes (up to 3 layers).
    fn layer_sizes(&self) -> [u32; 3];
}

/// A borrowing view over raw AV1LayerConfigurationBox bytes.
#[derive(Clone, Copy)]
pub struct AV1LayerConfigurationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> AV1LayerConfigurationBoxView<'a> {
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

impl<'a> AV1LayerConfigurationBox for AV1LayerConfigurationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn large_size(&self) -> bool {
        (self.data[self.header_size] >> 7) & 1 != 0
    }

    fn layer_sizes(&self) -> [u32; 3] {
        let o = self.header_size + 1;
        if self.large_size() {
            // 32-bit sizes
            if self.data.len() >= o + 12 {
                [
                    BigEndian::read_u32(&self.data[o..o + 4]),
                    BigEndian::read_u32(&self.data[o + 4..o + 8]),
                    BigEndian::read_u32(&self.data[o + 8..o + 12]),
                ]
            } else {
                [0, 0, 0]
            }
        } else {
            // 16-bit sizes
            if self.data.len() >= o + 6 {
                [
                    BigEndian::read_u16(&self.data[o..o + 2]) as u32,
                    BigEndian::read_u16(&self.data[o + 2..o + 4]) as u32,
                    BigEndian::read_u16(&self.data[o + 4..o + 6]) as u32,
                ]
            } else {
                [0, 0, 0]
            }
        }
    }
}

impl std::fmt::Debug for AV1LayerConfigurationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AV1LayerConfigurationBoxView")
            .field("large_size", &self.large_size())
            .field("layer_sizes", &self.layer_sizes())
            .finish()
    }
}

/// An owned representation of AV1LayerConfigurationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AV1LayerConfigurationBoxOwned {
    /// Large size flag.
    pub large_size: bool,
    /// Layer sizes.
    pub layer_sizes: [u32; 3],
}

impl AV1LayerConfigurationBoxOwned {
    /// Creates a new AV1LayerConfigurationBoxOwned.
    pub fn new(layer_sizes: [u32; 3]) -> Self {
        let large_size = layer_sizes.iter().any(|&s| s > 0xFFFF);
        Self { large_size, layer_sizes }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = if self.large_size {
            (1 + 12) as u64 // 32-bit sizes
        } else {
            (1 + 6) as u64 // 16-bit sizes
        };
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;

        let flags = if self.large_size { 0x80u8 } else { 0x00u8 };
        writer.write_u8(flags)?;

        if self.large_size {
            for &size in &self.layer_sizes {
                writer.write_u32::<BigEndian>(size)?;
            }
        } else {
            for &size in &self.layer_sizes {
                writer.write_u16::<BigEndian>(size as u16)?;
            }
        }

        Ok(())
    }
}

impl Default for AV1LayerConfigurationBoxOwned {
    fn default() -> Self {
        Self::new([0, 0, 0])
    }
}

impl AV1LayerConfigurationBox for AV1LayerConfigurationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn large_size(&self) -> bool {
        self.large_size
    }

    fn layer_sizes(&self) -> [u32; 3] {
        self.layer_sizes
    }
}

impl<T: AV1LayerConfigurationBox> From<&T> for AV1LayerConfigurationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            large_size: source.large_size(),
            layer_sizes: source.layer_sizes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_a1lx() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&15u32.to_be_bytes()); // 8 + 1 + 6
        data.extend_from_slice(b"a1lx");
        data.push(0x00); // large_size = false
        data.extend_from_slice(&100u16.to_be_bytes()); // layer 0
        data.extend_from_slice(&200u16.to_be_bytes()); // layer 1
        data.extend_from_slice(&300u16.to_be_bytes()); // layer 2
        data
    }

    #[test]
    fn parse_a1lx() {
        let data = make_a1lx();
        let view = AV1LayerConfigurationBoxView::new(&data).unwrap();

        assert!(!view.large_size());
        assert_eq!(view.layer_sizes(), [100, 200, 300]);
    }

    #[test]
    fn roundtrip() {
        let data = make_a1lx();
        let view = AV1LayerConfigurationBoxView::new(&data).unwrap();
        let owned = AV1LayerConfigurationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
