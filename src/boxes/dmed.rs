//! Media Duration Box (dmed) parsing and serialization.
//!
//! The Media Duration Box contains the media bytes for immediate playback.
//!
//! ```text
//! aligned(8) class hintmediaBytesSent extends Box('dmed') {
//!    uint(64) bytessent; // total bytes sent from media tracks
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MediaDurationBox.
pub const BOX_TYPE: BoxCode = BoxCode::new(*b"dmed");

/// Common interface for accessing MediaDurationBox data.
pub trait MediaDurationBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the media bytes.
    fn media_bytes(&self) -> u64;
}

/// A borrowing view over raw MediaDurationBox bytes.
#[derive(Clone, Copy)]
pub struct MediaDurationBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> MediaDurationBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 8)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }
}

impl<'a> MediaDurationBox for MediaDurationBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn media_bytes(&self) -> u64 {
        BigEndian::read_u64(&self.data[self.header_size..self.header_size + 8])
    }
}

impl std::fmt::Debug for MediaDurationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaDurationBoxView")
            .field("media_bytes", &self.media_bytes())
            .finish()
    }
}

/// An owned representation of MediaDurationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaDurationBoxOwned {
    /// Media bytes.
    pub media_bytes: u64,
}

impl MediaDurationBoxOwned {
    /// Creates a new MediaDurationBoxOwned.
    pub fn new(media_bytes: u64) -> Self {
        Self { media_bytes }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        header_size_for_payload(8) + 8 // 8 + 8
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_u64::<BigEndian>(self.media_bytes)?;

        Ok(())
    }
}

impl Default for MediaDurationBoxOwned {
    fn default() -> Self {
        Self::new(0)
    }
}

impl MediaDurationBox for MediaDurationBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn media_bytes(&self) -> u64 {
        self.media_bytes
    }
}

impl<T: MediaDurationBox> From<&T> for MediaDurationBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            media_bytes: source.media_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dmed() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 8
        data.extend_from_slice(b"dmed");
        data.extend_from_slice(&10000u64.to_be_bytes());
        data
    }

    #[test]
    fn parse_dmed() {
        let data = make_dmed();
        let view = MediaDurationBoxView::new(&data).unwrap();
        assert_eq!(view.media_bytes(), 10000);
    }

    #[test]
    fn roundtrip() {
        let data = make_dmed();
        let view = MediaDurationBoxView::new(&data).unwrap();
        let owned = MediaDurationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
