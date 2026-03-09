//! Media Data Box (mdat) parsing and serialization.
//!
//! The Media Data Box contains the actual media samples (audio, video, etc.).
//! This box can be very large and is often parsed lazily.
//!
//! ```text
//! aligned(8) class MediaDataBox extends Box('mdat') {
//!    bit(8) data[];
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MediaDataBox.
pub const BOX_TYPE: BoxCode = BoxCode::MDAT;

/// Common interface for accessing MediaDataBox data.
pub trait MediaDataBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the size of the media data (payload size).
    fn data_size(&self) -> u64;

    /// Returns the media data payload.
    fn media_data(&self) -> &[u8];
}

/// A borrowing view over raw MediaDataBox bytes.
///
/// Note: This view requires the entire mdat box data to be in memory.
/// For large files, consider using a streaming approach instead.
#[derive(Clone, Copy)]
pub struct MediaDataBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> MediaDataBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 0)?;
        Ok(Self { data, header })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the media data payload.
    #[inline]
    pub fn media_data(&self) -> &'a [u8] {
        &self.data[self.header.header_size as usize..]
    }

    /// Returns the header size (8 or 16 bytes).
    #[inline]
    pub fn header_size(&self) -> usize {
        self.header.header_size as usize
    }
}

impl MediaDataBox for MediaDataBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn data_size(&self) -> u64 {
        self.header.payload_size()
    }

    fn media_data(&self) -> &[u8] {
        self.media_data()
    }
}

impl std::fmt::Debug for MediaDataBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaDataBoxView")
            .field("box_size", &self.box_size())
            .field("data_size", &self.data_size())
            .finish()
    }
}

/// An owned representation of MediaDataBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MediaDataBoxOwned {
    /// The media data.
    pub data: Vec<u8>,
}

impl MediaDataBoxOwned {
    /// Creates a new empty MediaDataBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a MediaDataBoxOwned from raw media data.
    pub fn from_data(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload_size = self.data.len() as u64;
        header_size_for_payload(payload_size) + payload_size
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}


impl MediaDataBox for MediaDataBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn data_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn media_data(&self) -> &[u8] {
        &self.data
    }
}

impl<T: MediaDataBox> From<&T> for MediaDataBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            data: source.media_data().to_vec(),
        }
    }
}

/// A reference to a MediaDataBox that stores only the header information.
///
/// This is useful for large files where you don't want to load the entire
/// mdat box into memory. The actual media data can be read from the file
/// using the offset and size information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaDataBoxRef {
    /// The offset of the mdat box in the file.
    pub file_offset: u64,
    /// The total size of the mdat box.
    pub box_size: u64,
    /// The size of the header (8 or 16 bytes).
    pub header_size: u8,
}

impl MediaDataBoxRef {
    /// Creates a new MediaDataBoxRef.
    pub fn new(file_offset: u64, box_size: u64, header_size: u8) -> Self {
        Self {
            file_offset,
            box_size,
            header_size,
        }
    }

    /// Returns the offset of the media data in the file.
    #[inline]
    pub fn data_offset(&self) -> u64 {
        self.file_offset + self.header_size as u64
    }

    /// Returns the size of the media data.
    #[inline]
    pub fn data_size(&self) -> u64 {
        self.box_size.saturating_sub(self.header_size as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mdat(data: &[u8]) -> Vec<u8> {
        let size = 8 + data.len();
        let mut buf = Vec::with_capacity(size);
        buf.extend_from_slice(&(size as u32).to_be_bytes());
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(data);
        buf
    }

    #[test]
    fn parse_mdat() {
        let data = make_mdat(&[1, 2, 3, 4, 5]);
        let view = MediaDataBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 13);
        assert_eq!(view.data_size(), 5);
        assert_eq!(view.media_data(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn parse_empty_mdat() {
        let data = make_mdat(&[]);
        let view = MediaDataBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 8);
        assert_eq!(view.data_size(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_mdat(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let view = MediaDataBoxView::new(&data).unwrap();
        let owned = MediaDataBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn mdat_ref() {
        let mdat_ref = MediaDataBoxRef::new(1000, 1000000, 8);
        assert_eq!(mdat_ref.data_offset(), 1008);
        assert_eq!(mdat_ref.data_size(), 999992);
    }
}
