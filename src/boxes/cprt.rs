//! Copyright Box (cprt) parsing and serialization.
//!
//! The Copyright Box contains a copyright declaration.
//!
//! ```text
//! aligned(8) class CopyrightBox
//!    extends FullBox('cprt', version = 0, 0) {
//!    const bit(1) pad = 0;
//!    unsigned int(5)[3] language; // ISO-639-2/T language code
//!    utf8string notice;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use crate::types::IsoLanguageCode;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CopyrightBox.
pub const BOX_TYPE: BoxCode = BoxCode::CPRT;

/// Common interface for accessing CopyrightBox data.
pub trait CopyrightBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the language code.
    fn language(&self) -> IsoLanguageCode;

    /// Returns the copyright notice.
    fn notice(&self) -> &[u8];
}

/// A borrowing view over raw CopyrightBox bytes.
#[derive(Clone, Copy)]
pub struct CopyrightBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> CopyrightBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 3)?;

        // Validate null terminator exists in the notice string
        let notice_start = fullbox_offset + 6; // version/flags(4) + language(2)
        if !data[notice_start..].contains(&0) {
            return Err(ParseError::MissingNullTerminator {
                field: "notice",
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

impl<'a> CopyrightBox for CopyrightBoxView<'a> {
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

    fn language(&self) -> IsoLanguageCode {
        let o = self.payload_offset();
        IsoLanguageCode::from_raw(BigEndian::read_u16(&self.data[o..o + 2]))
    }

    fn notice(&self) -> &[u8] {
        let o = self.payload_offset() + 2;
        let notice_data = &self.data[o..];
        // Null terminator validated in constructor
        let end = notice_data.iter().position(|&b| b == 0).unwrap();
        &notice_data[..end]
    }
}

impl std::fmt::Debug for CopyrightBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyrightBoxView")
            .field("language", &self.language().to_string())
            .field("notice", &String::from_utf8_lossy(self.notice()))
            .finish()
    }
}

/// An owned representation of CopyrightBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyrightBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Language code.
    pub language: IsoLanguageCode,
    /// Copyright notice (UTF-8 or UTF-16).
    pub notice: Vec<u8>,
}

impl CopyrightBoxOwned {
    /// Creates a new CopyrightBoxOwned.
    pub fn new(language: IsoLanguageCode, notice: Vec<u8>) -> Self {
        Self {
            flags: 0,
            language,
            notice,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (2 + self.notice.len() + 1) as u64; // language + notice + null terminator
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u16::<BigEndian>(self.language.raw())?;
        writer.write_all(&self.notice)?;
        writer.write_u8(0)?; // null terminator

        Ok(())
    }
}

impl Default for CopyrightBoxOwned {
    fn default() -> Self {
        Self::new(IsoLanguageCode::UNDETERMINED, Vec::new())
    }
}

impl CopyrightBox for CopyrightBoxOwned {
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

    fn language(&self) -> IsoLanguageCode {
        self.language
    }

    fn notice(&self) -> &[u8] {
        &self.notice
    }
}

impl<T: CopyrightBox> From<&T> for CopyrightBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            language: source.language(),
            notice: source.notice().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cprt() -> Vec<u8> {
        let notice = b"(C) 2024 Test";
        let mut data = Vec::new();
        data.extend_from_slice(&(8 + 4 + 2 + notice.len() as u32 + 1).to_be_bytes());
        data.extend_from_slice(b"cprt");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        // "eng" = 0x15C7 (packed ISO 639-2)
        data.extend_from_slice(&0x15C7u16.to_be_bytes());
        data.extend_from_slice(notice);
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_cprt() {
        let data = make_cprt();
        let view = CopyrightBoxView::new(&data).unwrap();

        assert_eq!(view.language().to_string(), "eng");
        assert_eq!(view.notice(), b"(C) 2024 Test");
    }

    #[test]
    fn roundtrip() {
        let data = make_cprt();
        let view = CopyrightBoxView::new(&data).unwrap();
        let owned = CopyrightBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
