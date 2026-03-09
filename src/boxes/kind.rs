//! Kind Box (kind) parsing and serialization.
//!
//! The Kind Box specifies the kind or category of the track.
//!
//! ```text
//! aligned(8) class KindBox
//!    extends FullBox('kind', version = 0, 0) {
//!    utf8string schemeURI;
//!    utf8string value;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for KindBox.
pub const BOX_TYPE: BoxCode = BoxCode::KIND;

/// Common interface for accessing KindBox data.
pub trait KindBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the scheme URI.
    fn scheme_uri(&self) -> &[u8];

    /// Returns the value.
    fn value(&self) -> &[u8];
}

/// A borrowing view over raw KindBox bytes.
#[derive(Clone, Copy)]
pub struct KindBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    scheme_uri_end: usize,
}

impl<'a> KindBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        let payload_start = fullbox_offset + 4;

        // Find end of scheme_uri (null terminated)
        let scheme_uri_end = match data[payload_start..].iter().position(|&b| b == 0) {
            Some(p) => payload_start + p,
            None => {
                return Err(ParseError::MissingNullTerminator {
                    field: "schemeURI",
                });
            }
        };

        // Validate null terminator exists in value string
        let value_start = scheme_uri_end + 1;
        if value_start < data.len() && !data[value_start..].contains(&0) {
            return Err(ParseError::MissingNullTerminator {
                field: "value",
            });
        }

        Ok(Self {
            data,
            fullbox_offset,
            scheme_uri_end,
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

impl<'a> KindBox for KindBoxView<'a> {
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

    fn scheme_uri(&self) -> &[u8] {
        &self.data[self.payload_offset()..self.scheme_uri_end]
    }

    fn value(&self) -> &[u8] {
        let value_start = self.scheme_uri_end + 1; // skip null terminator
        if value_start >= self.data.len() {
            return &[];
        }
        let value_data = &self.data[value_start..];
        // Null terminator validated in constructor
        let end = value_data.iter().position(|&b| b == 0).unwrap();
        &value_data[..end]
    }
}

impl std::fmt::Debug for KindBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KindBoxView")
            .field("scheme_uri", &String::from_utf8_lossy(self.scheme_uri()))
            .field("value", &String::from_utf8_lossy(self.value()))
            .finish()
    }
}

/// An owned representation of KindBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Scheme URI.
    pub scheme_uri: Vec<u8>,
    /// Value.
    pub value: Vec<u8>,
}

impl KindBoxOwned {
    /// Creates a new KindBoxOwned.
    pub fn new(scheme_uri: Vec<u8>, value: Vec<u8>) -> Self {
        Self {
            flags: 0,
            scheme_uri,
            value,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.scheme_uri.len() + 1 + self.value.len() + 1) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.scheme_uri)?;
        writer.write_u8(0)?; // null terminator
        writer.write_all(&self.value)?;
        writer.write_u8(0)?; // null terminator

        Ok(())
    }
}

impl Default for KindBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new(), Vec::new())
    }
}

impl KindBox for KindBoxOwned {
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

    fn scheme_uri(&self) -> &[u8] {
        &self.scheme_uri
    }

    fn value(&self) -> &[u8] {
        &self.value
    }
}

impl<T: KindBox> From<&T> for KindBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            scheme_uri: source.scheme_uri().to_vec(),
            value: source.value().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_kind() -> Vec<u8> {
        let scheme = b"urn:mpeg:dash:role:2011";
        let value = b"main";
        let mut data = Vec::new();
        // 8 + 4 + 23 + 1 + 4 + 1 = 41 bytes
        data.extend_from_slice(&41u32.to_be_bytes());
        data.extend_from_slice(b"kind");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(scheme);
        data.push(0); // null terminator
        data.extend_from_slice(value);
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_kind() {
        let data = make_kind();
        let view = KindBoxView::new(&data).unwrap();

        assert_eq!(view.scheme_uri(), b"urn:mpeg:dash:role:2011");
        assert_eq!(view.value(), b"main");
    }

    #[test]
    fn roundtrip() {
        let data = make_kind();
        let view = KindBoxView::new(&data).unwrap();
        let owned = KindBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
