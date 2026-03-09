//! URI Init Box (uriI) parsing and serialization.
//!
//! The URI Init Box contains initialization data for a URI-based meta box.
//!
//! ```text
//! aligned(8) class URIInitBox
//!    extends FullBox('uriI', version = 0, 0) {
//!    unsigned int(8) uri_initialization_data[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for URIInitBox.
pub const BOX_TYPE: BoxCode = BoxCode::URII;

/// Common interface for accessing URIInitBox data.
pub trait URIInitBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the URI initialization data.
    fn uri_init_data(&self) -> &[u8];
}

/// A borrowing view over raw URIInitBox bytes.
#[derive(Clone, Copy)]
pub struct URIInitBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> URIInitBoxView<'a> {
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

    /// Returns the URI initialization data.
    pub fn uri_init_data(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }
}

impl<'a> URIInitBox for URIInitBoxView<'a> {
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

    fn uri_init_data(&self) -> &[u8] {
        self.uri_init_data()
    }
}

impl std::fmt::Debug for URIInitBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("URIInitBoxView")
            .field("data_len", &self.uri_init_data().len())
            .finish()
    }
}

/// An owned representation of URIInitBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct URIInitBoxOwned {
    /// Flags.
    pub flags: u32,
    /// URI initialization data.
    pub uri_init_data: Vec<u8>,
}

impl URIInitBoxOwned {
    /// Creates a new URIInitBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.uri_init_data.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.uri_init_data)?;

        Ok(())
    }
}


impl URIInitBox for URIInitBoxOwned {
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

    fn uri_init_data(&self) -> &[u8] {
        &self.uri_init_data
    }
}

impl<T: URIInitBox> From<&T> for URIInitBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            uri_init_data: source.uri_init_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_urii() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"uriI");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data
    }

    #[test]
    fn parse_urii() {
        let data = make_urii();
        let view = URIInitBoxView::new(&data).unwrap();
        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_urii();
        let view = URIInitBoxView::new(&data).unwrap();
        let owned = URIInitBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
