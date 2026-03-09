//! File Type Box (ftyp) parsing and serialization.
//!
//! The File Type Box identifies the specification to which the file complies,
//! and lists compatible brands.
//!
//! ```text
//! aligned(8) class FileTypeBox
//!    extends Box('ftyp') {
//!    unsigned int(32) major_brand;
//!    unsigned int(32) minor_version;
//!    unsigned int(32) compatible_brands[]; // to end of the box
//! }
//! ```

use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, BrandCode};
use std::io::{self, Write};

/// The box type identifier for FileTypeBox.
pub const BOX_TYPE: BoxCode = BoxCode::FTYP;

/// Common interface for accessing FileTypeBox data.
pub trait FileTypeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the major brand (e.g., "isom", "mp41", "mp42").
    fn major_brand(&self) -> BrandCode;

    /// Returns the minor version.
    fn minor_version(&self) -> u32;

    /// Returns the number of compatible brands.
    fn compatible_brands_count(&self) -> usize;

    /// Returns the compatible brand at the given index.
    fn compatible_brand(&self, index: usize) -> Option<BrandCode>;

    /// Returns an iterator over the compatible brands.
    fn compatible_brands(&self) -> impl Iterator<Item = BrandCode> + '_;
}

/// A borrowing view over raw FileTypeBox bytes.
#[derive(Clone, Copy)]
pub struct FileTypeBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> FileTypeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 8)?;

        // Check that remaining bytes are a multiple of 4 (for compatible brands)
        let min_size = header.header_size as usize + 8;
        let remaining = data.len() - min_size;
        if !remaining.is_multiple_of(4) {
            return Err(ParseError::BufferTooShort {
                expected: min_size + ((remaining / 4 + 1) * 4),
                found: data.len(),
            });
        }

        Ok(Self { data, header })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over the compatible brands.
    pub fn compatible_brands(&self) -> impl Iterator<Item = BrandCode> + '_ {
        let start = self.header.header_size as usize + 8;
        self.data[start..]
            .chunks_exact(4)
            .map(|chunk| BrandCode::new([chunk[0], chunk[1], chunk[2], chunk[3]]))
    }
}

impl FileTypeBox for FileTypeBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn major_brand(&self) -> BrandCode {
        let o = self.header.header_size as usize;
        BrandCode::new([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn minor_version(&self) -> u32 {
        let o = self.header.header_size as usize + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn compatible_brands_count(&self) -> usize {
        let payload_size = self.data.len() - self.header.header_size as usize;
        (payload_size - 8) / 4
    }

    fn compatible_brand(&self, index: usize) -> Option<BrandCode> {
        if index >= self.compatible_brands_count() {
            return None;
        }
        let o = self.header.header_size as usize + 8 + index * 4;
        Some(BrandCode::new([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]]))
    }

    fn compatible_brands(&self) -> impl Iterator<Item = BrandCode> + '_ {
        FileTypeBoxView::compatible_brands(self)
    }
}

impl std::fmt::Debug for FileTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let brands: Vec<_> = self.compatible_brands().collect();
        f.debug_struct("FileTypeBoxView")
            .field("box_size", &self.box_size())
            .field("major_brand", &self.major_brand())
            .field("minor_version", &self.minor_version())
            .field("compatible_brands", &brands)
            .finish()
    }
}

/// An owned representation of FileTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTypeBoxOwned {
    /// The major brand.
    pub major_brand: BrandCode,
    /// The minor version.
    pub minor_version: u32,
    /// The compatible brands.
    pub compatible_brands: Vec<BrandCode>,
}

impl FileTypeBoxOwned {
    /// Creates a new FileTypeBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = 4 + 4 + (self.compatible_brands.len() * 4) as u64;
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        writer.write_all(&self.major_brand.0 .0)?;
        writer.write_u32::<BigEndian>(self.minor_version)?;
        for brand in &self.compatible_brands {
            writer.write_all(&brand.0 .0)?;
        }
        Ok(())
    }
}

impl Default for FileTypeBoxOwned {
    fn default() -> Self {
        Self {
            major_brand: BrandCode::ISOM,
            minor_version: 0x200,
            compatible_brands: vec![BrandCode::ISOM, BrandCode::ISO2, BrandCode::MP41],
        }
    }
}

impl FileTypeBox for FileTypeBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn major_brand(&self) -> BrandCode {
        self.major_brand
    }

    fn minor_version(&self) -> u32 {
        self.minor_version
    }

    fn compatible_brands_count(&self) -> usize {
        self.compatible_brands.len()
    }

    fn compatible_brand(&self, index: usize) -> Option<BrandCode> {
        self.compatible_brands.get(index).copied()
    }

    fn compatible_brands(&self) -> impl Iterator<Item = BrandCode> + '_ {
        self.compatible_brands.iter().copied()
    }
}

impl<T: FileTypeBox> From<&T> for FileTypeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            major_brand: source.major_brand(),
            minor_version: source.minor_version(),
            compatible_brands: source.compatible_brands().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ftyp() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&20u32.to_be_bytes()); // size
        data.extend_from_slice(b"ftyp");              // type
        data.extend_from_slice(b"isom");              // major_brand
        data.extend_from_slice(&0x200u32.to_be_bytes()); // minor_version
        data.extend_from_slice(b"mp41");              // compatible_brand
        data
    }

    #[test]
    fn parse_ftyp() {
        let data = make_ftyp();
        let view = FileTypeBoxView::new(&data).unwrap();

        assert_eq!(view.box_size(), 20);
        assert_eq!(view.major_brand(), BrandCode::ISOM);
        assert_eq!(view.minor_version(), 0x200);
        assert_eq!(view.compatible_brands_count(), 1);
        assert_eq!(view.compatible_brand(0), Some(BrandCode::MP41));
    }

    #[test]
    fn roundtrip() {
        let data = make_ftyp();
        let view = FileTypeBoxView::new(&data).unwrap();
        let owned = FileTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn owned_default() {
        let owned = FileTypeBoxOwned::new();
        assert_eq!(owned.major_brand, BrandCode::ISOM);
        assert_eq!(owned.compatible_brands.len(), 3);
    }
}
