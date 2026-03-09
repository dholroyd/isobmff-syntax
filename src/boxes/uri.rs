//! URI Box (uri ) parsing and serialization.
//!
//! The URI Box contains a URI declaration.
//!
//! ```text
//! aligned(8) class URIBox
//!    extends FullBox('uri ', version = 0, 0) {
//!    utf8string theURI;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for URIBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"uri ");

/// Common interface for accessing URIBox data.
pub trait URIBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the URI as a string.
    fn the_uri_str(&self) -> Option<&str>;
}

/// A borrowing view over raw URIBox bytes.
#[derive(Clone, Copy)]
pub struct URIBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> URIBoxView<'a> {
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

    /// Returns the URI string (null-terminated in the data).
    pub fn the_uri(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start < self.data.len() {
            // Find null terminator
            let slice = &self.data[start..];
            if let Some(pos) = slice.iter().position(|&b| b == 0) {
                &slice[..pos]
            } else {
                slice
            }
        } else {
            &[]
        }
    }

    /// Returns the URI as a string.
    pub fn the_uri_str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.the_uri()).ok()
    }
}

impl<'a> URIBox for URIBoxView<'a> {
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

    fn the_uri_str(&self) -> Option<&str> {
        self.the_uri_str()
    }
}

impl std::fmt::Debug for URIBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("URIBoxView")
            .field("uri", &self.the_uri_str())
            .finish()
    }
}

/// An owned representation of URIBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct URIBoxOwned {
    /// Flags.
    pub flags: u32,
    /// URI string (without null terminator).
    pub the_uri: String,
}

impl URIBoxOwned {
    /// Creates a new URIBoxOwned.
    pub fn new(uri: String) -> Self {
        Self {
            flags: 0,
            the_uri: uri,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.the_uri.len() + 1) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(self.the_uri.as_bytes())?;
        writer.write_u8(0)?; // null terminator

        Ok(())
    }
}

impl Default for URIBoxOwned {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl URIBox for URIBoxOwned {
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

    fn the_uri_str(&self) -> Option<&str> {
        Some(&self.the_uri)
    }
}

impl TryFrom<&URIBoxView<'_>> for URIBoxOwned {
    type Error = ParseError;

    fn try_from(source: &URIBoxView<'_>) -> Result<Self, Self::Error> {
        let the_uri = source.the_uri_str()
            .ok_or(ParseError::InvalidStringEncoding { context: "the_uri" })?
            .to_string();
        Ok(Self {
            flags: source.flags(),
            the_uri,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_uri() -> Vec<u8> {
        let mut data = Vec::new();
        let uri = b"urn:example";
        data.extend_from_slice(&(8 + 4 + uri.len() as u32 + 1).to_be_bytes());
        data.extend_from_slice(b"uri ");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(uri);
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_uri() {
        let data = make_uri();
        let view = URIBoxView::new(&data).unwrap();
        assert_eq!(view.the_uri_str(), Some("urn:example"));
    }

    #[test]
    fn roundtrip() {
        let data = make_uri();
        let view = URIBoxView::new(&data).unwrap();
        let owned = URIBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
