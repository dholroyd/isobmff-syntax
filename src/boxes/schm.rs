//! Scheme Type Box (schm) parsing and serialization.
//!
//! The Scheme Type Box identifies the protection or restriction scheme.
//!
//! ```text
//! aligned(8) class SchemeTypeBox
//!    extends FullBox('schm', 0, flags) {
//!    unsigned int(32) scheme_type; // 4CC identifying the scheme
//!    unsigned int(32) scheme_version; // scheme version
//!    if (flags & 0x000001) {
//!       utf8string scheme_uri; // browser uri
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for SchemeTypeBox.
pub const BOX_TYPE: BoxCode = BoxCode::SCHM;

/// Common interface for accessing SchemeTypeBox data.
pub trait SchemeTypeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the scheme type.
    fn scheme_type(&self) -> FourCC;

    /// Returns the scheme version.
    fn scheme_version(&self) -> u32;

    /// Returns the scheme URI, if present (when flags & 1 != 0).
    fn scheme_uri(&self) -> Option<&[u8]>;
}

/// A borrowing view over raw SchemeTypeBox bytes.
#[derive(Clone, Copy)]
pub struct SchemeTypeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    fl: u32,
}

impl<'a> SchemeTypeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fl = header.flags;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;

        // Validate null terminator in scheme_uri when present
        if fl & 1 != 0 {
            let uri_start = fullbox_offset + 12; // version/flags(4) + scheme_type(4) + scheme_version(4)
            if !data[uri_start..].contains(&0) {
                return Err(ParseError::MissingNullTerminator {
                    field: "scheme_uri",
                });
            }
        }

        Ok(Self {
            data,
            fullbox_offset,
            fl,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }
}

impl<'a> SchemeTypeBox for SchemeTypeBoxView<'a> {
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
        self.fl
    }

    fn scheme_type(&self) -> FourCC {
        let o = self.payload_offset();
        FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn scheme_version(&self) -> u32 {
        let o = self.payload_offset() + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn scheme_uri(&self) -> Option<&[u8]> {
        if self.fl & 1 != 0 {
            let o = self.payload_offset() + 8;
            let uri_data = &self.data[o..];
            // Null terminator validated in constructor
            let end = uri_data.iter().position(|&b| b == 0).unwrap();
            Some(&uri_data[..end])
        } else {
            None
        }
    }
}

impl std::fmt::Debug for SchemeTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemeTypeBoxView")
            .field("scheme_type", &self.scheme_type())
            .field("scheme_version", &self.scheme_version())
            .finish()
    }
}

/// An owned representation of SchemeTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemeTypeBoxOwned {
    /// Scheme type.
    pub scheme_type: FourCC,
    /// Scheme version.
    pub scheme_version: u32,
    /// Scheme URI (optional).
    pub scheme_uri: Option<Vec<u8>>,
}

impl SchemeTypeBoxOwned {
    /// Creates a new SchemeTypeBoxOwned.
    pub fn new(scheme_type: FourCC, scheme_version: u32) -> Self {
        Self {
            scheme_type,
            scheme_version,
            scheme_uri: None,
        }
    }

    fn compute_flags(&self) -> u32 {
        if self.scheme_uri.is_some() { 1 } else { 0 }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 8u64; // scheme_type + scheme_version
        if let Some(uri) = &self.scheme_uri {
            payload += uri.len() as u64 + 1; // +1 for null terminator
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        let fl = self.compute_flags();
        write_fullbox_header(writer, size, BOX_TYPE, 0, fl)?;
        writer.write_all(&self.scheme_type.0)?;
        writer.write_u32::<BigEndian>(self.scheme_version)?;

        if let Some(uri) = &self.scheme_uri {
            writer.write_all(uri)?;
            writer.write_u8(0)?; // null terminator
        }

        Ok(())
    }
}

impl Default for SchemeTypeBoxOwned {
    fn default() -> Self {
        Self::new(FourCC(*b"cenc"), 0x00010000)
    }
}

impl SchemeTypeBox for SchemeTypeBoxOwned {
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
        self.compute_flags()
    }

    fn scheme_type(&self) -> FourCC {
        self.scheme_type
    }

    fn scheme_version(&self) -> u32 {
        self.scheme_version
    }

    fn scheme_uri(&self) -> Option<&[u8]> {
        self.scheme_uri.as_deref()
    }
}

impl<T: SchemeTypeBox> From<&T> for SchemeTypeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            scheme_type: source.scheme_type(),
            scheme_version: source.scheme_version(),
            scheme_uri: source.scheme_uri().map(|s| s.to_vec()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schm_without_uri() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // size = 8 + 4 + 8
        data.extend_from_slice(b"schm");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"cenc"); // scheme_type
        data.extend_from_slice(&0x00010000u32.to_be_bytes()); // scheme_version
        data
    }

    fn make_schm_with_uri() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&28u32.to_be_bytes()); // size = 8 + 4 + 8 + 8
        data.extend_from_slice(b"schm");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 1]); // flags = 1 (uri present)
        data.extend_from_slice(b"cenc"); // scheme_type
        data.extend_from_slice(&0x00010000u32.to_be_bytes()); // scheme_version
        data.extend_from_slice(b"urn:foo"); // scheme_uri
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_schm_without_uri() {
        let data = make_schm_without_uri();
        let view = SchemeTypeBoxView::new(&data).unwrap();

        assert_eq!(view.scheme_type(), FourCC(*b"cenc"));
        assert_eq!(view.scheme_version(), 0x00010000);
        assert!(view.scheme_uri().is_none());
    }

    #[test]
    fn parse_schm_with_uri() {
        let data = make_schm_with_uri();
        let view = SchemeTypeBoxView::new(&data).unwrap();

        assert_eq!(view.scheme_type(), FourCC(*b"cenc"));
        assert_eq!(view.scheme_uri(), Some(b"urn:foo".as_slice()));
    }

    #[test]
    fn roundtrip_without_uri() {
        let data = make_schm_without_uri();
        let view = SchemeTypeBoxView::new(&data).unwrap();
        let owned = SchemeTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_uri() {
        let data = make_schm_with_uri();
        let view = SchemeTypeBoxView::new(&data).unwrap();
        let owned = SchemeTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
