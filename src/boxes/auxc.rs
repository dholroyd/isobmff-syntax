//! Auxiliary Type Property Box (auxC) parsing and serialization.
//!
//! The Auxiliary Type Property Box specifies the type of an auxiliary image.
//!
//! ```text
//! aligned(8) class AuxiliaryTypeProperty
//!    extends ItemFullProperty('auxC', version = 0, 0) {
//!    utf8string aux_type;
//!    unsigned int(8) aux_subtype[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for AuxiliaryTypePropertyBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"auxC");

/// Common interface for accessing AuxiliaryTypePropertyBox data.
pub trait AuxiliaryTypePropertyBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the aux_type as a byte slice (without null terminator).
    fn aux_type(&self) -> &[u8];

    /// Returns the aux_subtype data after the null-terminated aux_type.
    fn aux_subtype(&self) -> &[u8];
}

/// A borrowing view over raw AuxiliaryTypePropertyBox bytes.
#[derive(Clone, Copy)]
pub struct AuxiliaryTypePropertyBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> AuxiliaryTypePropertyBoxView<'a> {
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

    /// Returns the aux_type as a null-terminated string.
    pub fn aux_type(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start >= self.data.len() {
            return &[];
        }

        // Find null terminator
        let end = self.data[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| start + p)
            .unwrap_or(self.data.len());

        &self.data[start..end]
    }

    /// Returns the aux_type as a string.
    pub fn aux_type_str(&self) -> Option<&str> {
        std::str::from_utf8(self.aux_type()).ok()
    }

    /// Returns the aux_subtype data after the null-terminated aux_type.
    pub fn aux_subtype(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4;
        if start >= self.data.len() {
            return &[];
        }

        // Find null terminator
        if let Some(pos) = self.data[start..].iter().position(|&b| b == 0) {
            let after_null = start + pos + 1;
            if after_null < self.data.len() {
                return &self.data[after_null..];
            }
        }

        &[]
    }
}

impl<'a> AuxiliaryTypePropertyBox for AuxiliaryTypePropertyBoxView<'a> {
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

    fn aux_type(&self) -> &[u8] {
        self.aux_type()
    }

    fn aux_subtype(&self) -> &[u8] {
        self.aux_subtype()
    }
}

impl std::fmt::Debug for AuxiliaryTypePropertyBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuxiliaryTypePropertyBoxView")
            .field("aux_type", &self.aux_type_str())
            .finish()
    }
}

/// An owned representation of AuxiliaryTypePropertyBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuxiliaryTypePropertyBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Auxiliary type URN.
    pub aux_type: Vec<u8>,
    /// Auxiliary subtype data.
    pub aux_subtype: Vec<u8>,
}

impl AuxiliaryTypePropertyBoxOwned {
    /// Creates a new AuxiliaryTypePropertyBoxOwned.
    pub fn new(aux_type: Vec<u8>) -> Self {
        Self {
            flags: 0,
            aux_type,
            aux_subtype: Vec::new(),
        }
    }

    /// Creates a new AuxiliaryTypePropertyBoxOwned from a string.
    pub fn from_aux_type_str(aux_type: &str) -> Self {
        Self::new(aux_type.as_bytes().to_vec())
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.aux_type.len() + 1 + self.aux_subtype.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.aux_type)?;
        writer.write_u8(0)?; // null terminator
        writer.write_all(&self.aux_subtype)?;

        Ok(())
    }
}

impl Default for AuxiliaryTypePropertyBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl AuxiliaryTypePropertyBox for AuxiliaryTypePropertyBoxOwned {
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

    fn aux_type(&self) -> &[u8] {
        &self.aux_type
    }

    fn aux_subtype(&self) -> &[u8] {
        &self.aux_subtype
    }
}

impl<T: AuxiliaryTypePropertyBox> From<&T> for AuxiliaryTypePropertyBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            aux_type: source.aux_type().to_vec(),
            aux_subtype: source.aux_subtype().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_auxc() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 1 = 17 bytes
        data.extend_from_slice(&17u32.to_be_bytes());
        data.extend_from_slice(b"auxC");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"urn:"); // aux_type
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_auxc() {
        let data = make_auxc();
        let view = AuxiliaryTypePropertyBoxView::new(&data).unwrap();
        assert_eq!(view.aux_type_str(), Some("urn:"));
    }

    #[test]
    fn roundtrip() {
        let data = make_auxc();
        let view = AuxiliaryTypePropertyBoxView::new(&data).unwrap();
        let owned = AuxiliaryTypePropertyBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
