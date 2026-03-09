//! XML Box (xml) parsing and serialization.
//!
//! The XML Box contains an XML document.
//!
//! ```text
//! aligned(8) class XMLBox
//!    extends FullBox('xml ', version = 0, 0) {
//!    utf8string xml;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for XmlBox.
pub const BOX_TYPE: BoxCode = BoxCode::XML;

/// Common interface for accessing XmlBox data.
pub trait XmlBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the XML data.
    fn xml(&self) -> &[u8];
}

/// A borrowing view over raw XmlBox bytes.
#[derive(Clone, Copy)]
pub struct XmlBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> XmlBoxView<'a> {
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

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }
}

impl<'a> XmlBox for XmlBoxView<'a> {
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

    fn xml(&self) -> &[u8] {
        &self.data[self.payload_offset()..]
    }
}

impl std::fmt::Debug for XmlBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XmlBoxView")
            .field("xml_len", &self.xml().len())
            .finish()
    }
}

/// An owned representation of XmlBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlBoxOwned {
    /// Flags.
    pub flags: u32,
    /// The XML document.
    pub xml: Vec<u8>,
}

impl XmlBoxOwned {
    /// Creates a new XmlBoxOwned.
    pub fn new(xml: Vec<u8>) -> Self {
        Self { flags: 0, xml }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.xml.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_all(&self.xml)?;

        Ok(())
    }
}

impl Default for XmlBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl XmlBox for XmlBoxOwned {
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

    fn xml(&self) -> &[u8] {
        &self.xml
    }
}

impl<T: XmlBox> From<&T> for XmlBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            xml: source.xml().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_xml() -> Vec<u8> {
        let xml_data = b"<root><item>test</item></root>";
        let mut data = Vec::new();
        data.extend_from_slice(&((8 + 4 + xml_data.len()) as u32).to_be_bytes());
        data.extend_from_slice(b"xml ");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(xml_data);
        data
    }

    #[test]
    fn parse_xml() {
        let data = make_xml();
        let view = XmlBoxView::new(&data).unwrap();

        assert_eq!(view.xml(), b"<root><item>test</item></root>");
    }

    #[test]
    fn roundtrip() {
        let data = make_xml();
        let view = XmlBoxView::new(&data).unwrap();
        let owned = XmlBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
