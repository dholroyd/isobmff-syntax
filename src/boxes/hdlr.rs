//! Handler Reference Box (hdlr) parsing and serialization.
//!
//! The Handler Reference Box declares the type of media data in a track.
//!
//! ```text
//! aligned(8) class HandlerBox extends FullBox('hdlr', version = 0, 0) {
//!    unsigned int(32) pre_defined = 0;
//!    unsigned int(32) handler_type;
//!    const unsigned int(32)[3] reserved = 0;
//!    string name;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use crate::types::NullTerminatedString;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, HandlerCode};
use std::io::{self, Write};

/// The box type identifier for HandlerReferenceBox.
pub const BOX_TYPE: BoxCode = BoxCode::HDLR;

/// Common interface for accessing HandlerReferenceBox data.
pub trait HandlerReferenceBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the handler type (e.g., "vide", "soun").
    fn handler_type(&self) -> HandlerCode;

    /// Returns the handler name.
    fn name(&self) -> &str;
}

/// A borrowing view over raw HandlerReferenceBox bytes.
#[derive(Clone, Copy)]
pub struct HandlerReferenceBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    /// Start and end offsets of the handler name string content (excluding
    /// any length prefix or null terminator).
    name_range: (usize, usize),
}

impl<'a> HandlerReferenceBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 21)?;

        // Parse handler name.
        // ISOBMFF spec says utf8string (null-terminated). QuickTime legacy uses
        // Pascal format (length-prefixed, no null terminator). Following GPAC's
        // approach: if the last byte is 0x00, treat as null-terminated C string;
        // otherwise treat as Pascal string (skip the length prefix byte).
        let name_start = fullbox_offset + 24; // version/flags(4) + pre_defined(4) + handler_type(4) + reserved(12)
        let name_data = &data[name_start..];

        let name_range = if name_data.is_empty() {
            (name_start, name_start)
        } else if name_data[name_data.len() - 1] == 0 {
            // Null-terminated C string: find the first null
            let end = name_data.iter().position(|&b| b == 0).unwrap(); // at least the last byte is 0
            (name_start, name_start + end)
        } else {
            // Pascal string: first byte is length, skip it
            let content_start = name_start + 1;
            let len = name_data[0] as usize;
            let content_end = content_start + len.min(name_data.len() - 1);
            (content_start, content_end)
        };

        // Validate UTF-8
        if std::str::from_utf8(&data[name_range.0..name_range.1]).is_err() {
            return Err(ParseError::InvalidStringEncoding {
                context: "handler_name",
            });
        }

        Ok(Self {
            data,
            fullbox_offset,
            name_range,
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

    /// Returns the pre_defined field value.
    ///
    /// Per the spec this is `unsigned int(32) pre_defined = 0`.
    pub fn pre_defined(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    /// Returns the handler name as a string view.
    pub fn name_string(&self) -> NullTerminatedString<'a> {
        NullTerminatedString::from_bytes(&self.data[self.name_range.0..self.name_range.1])
    }
}

impl HandlerReferenceBox for HandlerReferenceBoxView<'_> {
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

    fn handler_type(&self) -> HandlerCode {
        let o = self.payload_offset() + 4; // After pre_defined
        HandlerCode::new([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn name(&self) -> &str {
        // UTF-8 validated in constructor
        std::str::from_utf8(&self.data[self.name_range.0..self.name_range.1]).unwrap()
    }
}

impl std::fmt::Debug for HandlerReferenceBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerReferenceBoxView")
            .field("box_size", &self.box_size())
            .field("handler_type", &self.handler_type())
            .field("name", &self.name())
            .finish()
    }
}

/// An owned representation of HandlerReferenceBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandlerReferenceBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Handler type (e.g., "vide", "soun").
    pub handler_type: HandlerCode,
    /// Handler name (human-readable).
    pub name: String,
}

impl HandlerReferenceBoxOwned {
    /// Creates a new HandlerReferenceBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a video handler.
    pub fn video() -> Self {
        Self {
            handler_type: HandlerCode::VIDE,
            name: "VideoHandler".to_string(),
            ..Default::default()
        }
    }

    /// Creates an audio handler.
    pub fn audio() -> Self {
        Self {
            handler_type: HandlerCode::SOUN,
            name: "SoundHandler".to_string(),
            ..Default::default()
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
// Header(8) + version/flags(4) + pre_defined(4) + handler_type(4) + reserved(12) + name + null
        fullbox_header_size_for_payload(4 + 4 + 12 + self.name.len() as u64 + 1) + 4 + 4 + 12 + self.name.len() as u64 + 1
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;

        // Payload
        writer.write_u32::<BigEndian>(0)?; // pre_defined
        writer.write_all(&self.handler_type.0 .0)?;
        writer.write_all(&[0u8; 12])?; // reserved
        writer.write_all(self.name.as_bytes())?;
        writer.write_u8(0)?; // null terminator

        Ok(())
    }
}

impl Default for HandlerReferenceBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            handler_type: HandlerCode::VIDE,
            name: String::new(),
        }
    }
}

impl HandlerReferenceBox for HandlerReferenceBoxOwned {
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

    fn handler_type(&self) -> HandlerCode {
        self.handler_type
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl<T: HandlerReferenceBox + ?Sized> From<&T> for HandlerReferenceBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            handler_type: source.handler_type(),
            name: source.name().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hdlr(handler_type: &[u8; 4], name: &str) -> Vec<u8> {
        let size = 8 + 4 + 4 + 4 + 12 + name.len() + 1;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"hdlr");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u32.to_be_bytes()); // pre_defined
        data.extend_from_slice(handler_type);
        data.extend_from_slice(&[0u8; 12]); // reserved
        data.extend_from_slice(name.as_bytes());
        data.push(0); // null terminator
        data
    }

    #[test]
    fn parse_hdlr() {
        let data = make_hdlr(b"vide", "VideoHandler");
        let view = HandlerReferenceBoxView::new(&data).unwrap();

        assert_eq!(view.handler_type(), HandlerCode::VIDE);
        assert_eq!(view.name(), "VideoHandler");
    }

    #[test]
    fn roundtrip() {
        let data = make_hdlr(b"soun", "SoundHandler");
        let view = HandlerReferenceBoxView::new(&data).unwrap();
        let owned = HandlerReferenceBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn video_handler() {
        let hdlr = HandlerReferenceBoxOwned::video();
        assert_eq!(hdlr.handler_type, HandlerCode::VIDE);
    }

    #[test]
    fn parse_pascal_string() {
        // QuickTime-style: length byte prefix, no null terminator
        let name = b"VideoHandler";
        let size = 8 + 4 + 4 + 4 + 12 + 1 + name.len(); // +1 for length byte, no null
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"hdlr");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u32.to_be_bytes()); // pre_defined
        data.extend_from_slice(b"vide");
        data.extend_from_slice(&[0u8; 12]); // reserved
        data.push(name.len() as u8); // Pascal length byte
        data.extend_from_slice(name);
        // No null terminator - last byte is 'r', not 0x00

        let view = HandlerReferenceBoxView::new(&data).unwrap();
        assert_eq!(view.name(), "VideoHandler");
    }

    #[test]
    fn empty_name() {
        // Just a null terminator
        let data = make_hdlr(b"vide", "");
        let view = HandlerReferenceBoxView::new(&data).unwrap();
        assert_eq!(view.name(), "");
    }
}
