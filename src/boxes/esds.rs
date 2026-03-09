//! Elementary Stream Descriptor Box (esds) parsing and serialization.
//!
//! The Elementary Stream Descriptor Box contains MPEG-4 Systems descriptors.
//!
//! ```text
//! aligned(8) class ESDBox
//!    extends FullBox('esds', version = 0, 0) {
//!    ES_Descriptor ES;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ESDescriptorBox.
pub const BOX_TYPE: BoxCode = BoxCode::ESDS;

/// Common interface for accessing ESDescriptorBox data.
pub trait ESDescriptorBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the ES descriptor data.
    fn es_descriptor(&self) -> &[u8];
}

/// A borrowing view over raw ESDescriptorBox bytes.
#[derive(Clone, Copy)]
pub struct ESDescriptorBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> ESDescriptorBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the ES descriptor data.
    pub fn es_descriptor(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl<'a> ESDescriptorBox for ESDescriptorBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.data[self.fullbox_offset]
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn es_descriptor(&self) -> &[u8] {
        self.es_descriptor()
    }
}

impl std::fmt::Debug for ESDescriptorBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ESDescriptorBoxView")
            .field("es_descriptor_len", &self.es_descriptor().len())
            .finish()
    }
}

/// An owned representation of ESDescriptorBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ESDescriptorBoxOwned {
    /// Flags.
    pub flags: u32,
    /// ES descriptor data.
    pub es_descriptor: Vec<u8>,
}

impl ESDescriptorBoxOwned {
    /// Creates a new ESDescriptorBoxOwned.
    pub fn new(es_descriptor: Vec<u8>) -> Self {
        Self {
            flags: 0,
            es_descriptor,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.es_descriptor.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.es_descriptor)?;

        Ok(())
    }
}

impl Default for ESDescriptorBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ESDescriptorBox for ESDescriptorBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        0
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn es_descriptor(&self) -> &[u8] {
        &self.es_descriptor
    }
}

impl<T: ESDescriptorBox> From<&T> for ESDescriptorBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            es_descriptor: source.es_descriptor().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_esds() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"esds");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_esds() {
        let data = make_esds();
        let view = ESDescriptorBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_esds();
        let view = ESDescriptorBoxView::new(&data).unwrap();
        let owned = ESDescriptorBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
