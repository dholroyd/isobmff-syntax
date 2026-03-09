//! JPEG Codec Configuration Box (jpgC) parsing and serialization.
//!
//! The JPEG Codec Configuration Box specifies JPEG configuration data.
//!
//! ```text
//! class JpegConfigurationBox extends Box('jpgC') {
//!    unsigned int(8) jpeg_prefix[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for JPEGConfigurationBox.
pub const BOX_TYPE: BoxCode = BoxCode::JPGC;

/// Common interface for accessing JPEGConfigurationBox data.
pub trait JPEGConfigurationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the JPEG configuration data.
    fn jpeg_data(&self) -> &[u8];
}

/// A borrowing view over raw JPEGConfigurationBox bytes.
#[derive(Clone, Copy)]
pub struct JPEGConfigurationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> JPEGConfigurationBoxView<'a> {
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

    /// Returns the JPEG configuration data.
    pub fn jpeg_data(&self) -> &'a [u8] {
        &self.data[self.header_size..]
    }
}

impl<'a> JPEGConfigurationBox for JPEGConfigurationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn jpeg_data(&self) -> &[u8] {
        self.jpeg_data()
    }
}

impl std::fmt::Debug for JPEGConfigurationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JPEGConfigurationBoxView")
            .field("data_len", &self.jpeg_data().len())
            .finish()
    }
}

/// An owned representation of JPEGConfigurationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JPEGConfigurationBoxOwned {
    /// JPEG configuration data.
    pub jpeg_data: Vec<u8>,
}

impl JPEGConfigurationBoxOwned {
    /// Creates a new JPEGConfigurationBoxOwned.
    pub fn new(jpeg_data: Vec<u8>) -> Self {
        Self { jpeg_data }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.jpeg_data.len()) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_all(&self.jpeg_data)?;

        Ok(())
    }
}

impl Default for JPEGConfigurationBoxOwned {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl JPEGConfigurationBox for JPEGConfigurationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn jpeg_data(&self) -> &[u8] {
        &self.jpeg_data
    }
}

impl<T: JPEGConfigurationBox> From<&T> for JPEGConfigurationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            jpeg_data: source.jpeg_data().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_jpgc() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"jpgC");
        data
    }

    #[test]
    fn parse_jpgc() {
        let data = make_jpgc();
        let view = JPEGConfigurationBoxView::new(&data).unwrap();
        assert_eq!(view.jpeg_data().len(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_jpgc();
        let view = JPEGConfigurationBoxView::new(&data).unwrap();
        let owned = JPEGConfigurationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
