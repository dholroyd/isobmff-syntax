//! SDP Box (sdp ) parsing and serialization.
//!
//! The SDP Box contains an SDP description for the session.
//!
//! ```text
//! aligned(8) class rtptracksdphintinformation extends Box('sdp ') {
//!    char sdptext[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SDPBox.
pub const BOX_TYPE: BoxCode = BoxCode::SDP;

/// Common interface for accessing SDPBox data.
pub trait SDPBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the SDP text as a string.
    fn sdp_text_str(&self) -> Option<&str>;
}

/// A borrowing view over raw SDPBox bytes.
#[derive(Clone, Copy)]
pub struct SDPBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> SDPBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 0)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the SDP text.
    pub fn sdp_text(&self) -> &'a [u8] {
        &self.data[self.header_size..]
    }

    /// Returns the SDP text as a string.
    pub fn sdp_text_str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.sdp_text()).ok()
    }
}

impl<'a> SDPBox for SDPBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn sdp_text_str(&self) -> Option<&str> {
        self.sdp_text_str()
    }
}

impl std::fmt::Debug for SDPBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SDPBoxView")
            .field("sdp_text_len", &self.sdp_text().len())
            .finish()
    }
}

/// An owned representation of SDPBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SDPBoxOwned {
    /// SDP text.
    pub sdp_text: String,
}

impl SDPBoxOwned {
    /// Creates a new SDPBoxOwned.
    pub fn new(sdp_text: String) -> Self {
        Self { sdp_text }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.sdp_text.len()) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_all(self.sdp_text.as_bytes())?;

        Ok(())
    }
}

impl Default for SDPBoxOwned {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl SDPBox for SDPBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn sdp_text_str(&self) -> Option<&str> {
        Some(&self.sdp_text)
    }
}

impl TryFrom<&SDPBoxView<'_>> for SDPBoxOwned {
    type Error = ParseError;

    fn try_from(source: &SDPBoxView<'_>) -> Result<Self, Self::Error> {
        let sdp_text = source.sdp_text_str()
            .ok_or(ParseError::InvalidStringEncoding { context: "sdp_text" })?
            .to_string();
        Ok(Self { sdp_text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sdp() -> Vec<u8> {
        let mut data = Vec::new();
        let sdp_text = b"v=0";
        data.extend_from_slice(&(8 + sdp_text.len() as u32).to_be_bytes());
        data.extend_from_slice(b"sdp ");
        data.extend_from_slice(sdp_text);
        data
    }

    #[test]
    fn parse_sdp() {
        let data = make_sdp();
        let view = SDPBoxView::new(&data).unwrap();
        assert_eq!(view.sdp_text_str(), Some("v=0"));
    }

    #[test]
    fn roundtrip() {
        let data = make_sdp();
        let view = SDPBoxView::new(&data).unwrap();
        let owned = SDPBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
