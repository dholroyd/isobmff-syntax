//! Extended Language Tag Box (elng) parsing and serialization.
//!
//! The Extended Language Tag Box contains an extended language tag as defined by BCP 47.
//!
//! ```text
//! aligned(8) class ExtendedLanguageTag
//!    extends FullBox('elng', version = 0, 0) {
//!    utf8string extended_language;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ExtendedLanguageTagBox.
pub const BOX_TYPE: BoxCode = BoxCode::ELNG;

/// Common interface for accessing ExtendedLanguageTagBox data.
pub trait ExtendedLanguageTagBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the extended language tag.
    fn extended_language(&self) -> &[u8];
}

/// A borrowing view over raw ExtendedLanguageTagBox bytes.
#[derive(Clone, Copy)]
pub struct ExtendedLanguageTagBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> ExtendedLanguageTagBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        let payload_start = fullbox_offset + 4;

        // Validate null terminator exists in the extended_language string
        if !data[payload_start..].contains(&0) {
            return Err(ParseError::MissingNullTerminator {
                field: "extended_language",
            });
        }

        Ok(Self { data, fullbox_offset })
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

impl<'a> ExtendedLanguageTagBox for ExtendedLanguageTagBoxView<'a> {
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

    fn extended_language(&self) -> &[u8] {
        let o = self.payload_offset();
        let lang_data = &self.data[o..];
        // Null terminator validated in constructor
        let end = lang_data.iter().position(|&b| b == 0).unwrap();
        &lang_data[..end]
    }
}

impl std::fmt::Debug for ExtendedLanguageTagBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtendedLanguageTagBoxView")
            .field("extended_language", &String::from_utf8_lossy(self.extended_language()))
            .finish()
    }
}

/// An owned representation of ExtendedLanguageTagBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtendedLanguageTagBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Extended language tag (BCP 47).
    pub extended_language: Vec<u8>,
}

impl ExtendedLanguageTagBoxOwned {
    /// Creates a new ExtendedLanguageTagBoxOwned.
    pub fn new(extended_language: Vec<u8>) -> Self {
        Self {
            flags: 0,
            extended_language,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.extended_language.len() + 1) as u64; // +1 for null terminator
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.extended_language)?;
        writer.write_u8(0)?; // null terminator

        Ok(())
    }
}

impl Default for ExtendedLanguageTagBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ExtendedLanguageTagBox for ExtendedLanguageTagBoxOwned {
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

    fn extended_language(&self) -> &[u8] {
        &self.extended_language
    }
}

impl<T: ExtendedLanguageTagBox> From<&T> for ExtendedLanguageTagBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            extended_language: source.extended_language().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_elng() -> Vec<u8> {
        let lang = b"en-US";
        let mut data = Vec::new();
        // 8 + 4 + 5 + 1 = 18 bytes
        data.extend_from_slice(&18u32.to_be_bytes());
        data.extend_from_slice(b"elng");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(lang);
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_elng() {
        let data = make_elng();
        let view = ExtendedLanguageTagBoxView::new(&data).unwrap();

        assert_eq!(view.extended_language(), b"en-US");
    }

    #[test]
    fn roundtrip() {
        let data = make_elng();
        let view = ExtendedLanguageTagBoxView::new(&data).unwrap();
        let owned = ExtendedLanguageTagBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
