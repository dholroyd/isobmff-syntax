//! Original Format Box (frma) parsing and serialization.
//!
//! The Original Format Box contains the original format of the media data.
//!
//! ```text
//! aligned(8) class OriginalFormatBox(codingname)
//!    extends Box('frma') {
//!    unsigned int(32) data_format = codingname;
//!       // format of decrypted, encoded data (in case of protection)
//!       // or un-transformed sample entry (in case of restriction)
//!       // and complete track information)
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for OriginalFormatBox.
pub const BOX_TYPE: BoxCode = BoxCode::FRMA;

/// Common interface for accessing OriginalFormatBox data.
pub trait OriginalFormatBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the original format (data format code).
    fn data_format(&self) -> FourCC;
}

/// A borrowing view over raw OriginalFormatBox bytes.
#[derive(Clone, Copy)]
pub struct OriginalFormatBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> OriginalFormatBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 4)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl OriginalFormatBox for OriginalFormatBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn data_format(&self) -> FourCC {
        let o = self.header_size;
        FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }
}

impl std::fmt::Debug for OriginalFormatBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OriginalFormatBoxView")
            .field("data_format", &self.data_format())
            .finish()
    }
}

/// An owned representation of OriginalFormatBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginalFormatBoxOwned {
    /// Original data format.
    pub data_format: FourCC,
}

impl OriginalFormatBoxOwned {
    /// Creates a new OriginalFormatBoxOwned.
    pub fn new(data_format: FourCC) -> Self {
        Self { data_format }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(4) + 4 // 8 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_box_header(writer, self.serialized_size(), BOX_TYPE)?;
        writer.write_all(&self.data_format.0)?;
        Ok(())
    }
}

impl Default for OriginalFormatBoxOwned {
    fn default() -> Self {
        Self::new(FourCC(*b"mp4a"))
    }
}

impl OriginalFormatBox for OriginalFormatBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn data_format(&self) -> FourCC {
        self.data_format
    }
}

impl<T: OriginalFormatBox> From<&T> for OriginalFormatBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            data_format: source.data_format(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frma() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"frma");
        data.extend_from_slice(b"avc1");
        data
    }

    #[test]
    fn parse_frma() {
        let data = make_frma();
        let view = OriginalFormatBoxView::new(&data).unwrap();

        assert_eq!(view.data_format(), FourCC(*b"avc1"));
    }

    #[test]
    fn roundtrip() {
        let data = make_frma();
        let view = OriginalFormatBoxView::new(&data).unwrap();
        let owned = OriginalFormatBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
