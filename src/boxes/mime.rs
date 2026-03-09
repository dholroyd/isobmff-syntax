//! MIME Type Box (mime) parsing and serialization.
//!
//! The MIME Type Box declares the MIME type for a meta box.
//!
//! ```text
//! aligned(8) class MIMEBox()
//!    extends FullBox('mime', version = 0, 0) {
//!    utf8string content_type;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MIMEBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"mime");

/// Common interface for accessing MIMEBox data.
pub trait MIMEBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the content type as a string.
    fn content_type_str(&self) -> Option<&str>;
}

/// A borrowing view over raw MIMEBox bytes.
#[derive(Clone, Copy)]
pub struct MIMEBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> MIMEBoxView<'a> {
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

    /// Returns the content type string (null-terminated in the data).
    pub fn content_type(&self) -> &'a [u8] {
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

    /// Returns the content type as a string.
    pub fn content_type_str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.content_type()).ok()
    }
}

impl<'a> MIMEBox for MIMEBoxView<'a> {
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

    fn content_type_str(&self) -> Option<&str> {
        self.content_type_str()
    }
}

impl std::fmt::Debug for MIMEBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MIMEBoxView")
            .field("content_type", &self.content_type_str())
            .finish()
    }
}

/// An owned representation of MIMEBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MIMEBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Content type (without null terminator).
    pub content_type: String,
}

impl MIMEBoxOwned {
    /// Creates a new MIMEBoxOwned.
    pub fn new(content_type: String) -> Self {
        Self {
            flags: 0,
            content_type,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.content_type.len() + 1) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(self.content_type.as_bytes())?;
        writer.write_u8(0)?; // null terminator

        Ok(())
    }
}

impl Default for MIMEBoxOwned {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl MIMEBox for MIMEBoxOwned {
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

    fn content_type_str(&self) -> Option<&str> {
        Some(&self.content_type)
    }
}

impl TryFrom<&MIMEBoxView<'_>> for MIMEBoxOwned {
    type Error = ParseError;

    fn try_from(source: &MIMEBoxView<'_>) -> Result<Self, Self::Error> {
        let content_type = source.content_type_str()
            .ok_or(ParseError::InvalidStringEncoding { context: "content_type" })?
            .to_string();
        Ok(Self {
            flags: source.flags(),
            content_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mime() -> Vec<u8> {
        let mut data = Vec::new();
        let content_type = b"text/plain";
        data.extend_from_slice(&(8 + 4 + content_type.len() as u32 + 1).to_be_bytes());
        data.extend_from_slice(b"mime");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(content_type);
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_mime() {
        let data = make_mime();
        let view = MIMEBoxView::new(&data).unwrap();
        assert_eq!(view.content_type_str(), Some("text/plain"));
    }

    #[test]
    fn roundtrip() {
        let data = make_mime();
        let view = MIMEBoxView::new(&data).unwrap();
        let owned = MIMEBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
