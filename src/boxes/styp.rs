//! Segment Type Box (styp) parsing and serialization.
//!
//! The Segment Type Box identifies the specifications to which a segment conforms.
//!
//! ```text
//! aligned(8) class SegmentTypeBox
//!    extends Box('styp') {
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

/// The box type identifier for SegmentTypeBox.
pub const BOX_TYPE: BoxCode = BoxCode::STYP;

/// Common interface for accessing SegmentTypeBox data.
pub trait SegmentTypeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the major brand.
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

/// A borrowing view over raw SegmentTypeBox bytes.
#[derive(Clone, Copy)]
pub struct SegmentTypeBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> SegmentTypeBoxView<'a> {
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

    /// Returns an iterator over all compatible brands.
    pub fn compatible_brands(&self) -> impl Iterator<Item = BrandCode> + '_ {
        (0..self.compatible_brands_count()).filter_map(|i| self.compatible_brand(i))
    }
}

impl SegmentTypeBox for SegmentTypeBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn major_brand(&self) -> BrandCode {
        let o = self.header_size;
        BrandCode::new([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn minor_version(&self) -> u32 {
        let o = self.header_size + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn compatible_brands_count(&self) -> usize {
        let payload_size = self.data.len() - self.header_size - 8;
        payload_size / 4
    }

    fn compatible_brand(&self, index: usize) -> Option<BrandCode> {
        if index >= self.compatible_brands_count() {
            return None;
        }
        let o = self.header_size + 8 + index * 4;
        Some(BrandCode::new([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]]))
    }

    fn compatible_brands(&self) -> impl Iterator<Item = BrandCode> + '_ {
        SegmentTypeBoxView::compatible_brands(self)
    }
}

impl std::fmt::Debug for SegmentTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentTypeBoxView")
            .field("major_brand", &self.major_brand())
            .field("minor_version", &self.minor_version())
            .field("compatible_brands_count", &self.compatible_brands_count())
            .finish()
    }
}

/// An owned representation of SegmentTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentTypeBoxOwned {
    /// Major brand.
    pub major_brand: BrandCode,
    /// Minor version.
    pub minor_version: u32,
    /// Compatible brands.
    pub compatible_brands: Vec<BrandCode>,
}

impl SegmentTypeBoxOwned {
    /// Creates a new SegmentTypeBoxOwned.
    pub fn new(major_brand: BrandCode, minor_version: u32) -> Self {
        Self {
            major_brand,
            minor_version,
            compatible_brands: Vec::new(),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (8 + self.compatible_brands.len() * 4) as u64;
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

impl Default for SegmentTypeBoxOwned {
    fn default() -> Self {
        Self {
            major_brand: BrandCode::MSDH,
            minor_version: 0,
            compatible_brands: vec![BrandCode::MSDH, BrandCode::MSIX],
        }
    }
}

impl SegmentTypeBox for SegmentTypeBoxOwned {
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

impl<T: SegmentTypeBox> From<&T> for SegmentTypeBoxOwned {
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

    fn make_styp() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&24u32.to_be_bytes()); // size
        data.extend_from_slice(b"styp");
        data.extend_from_slice(b"msdh"); // major_brand
        data.extend_from_slice(&0u32.to_be_bytes()); // minor_version
        data.extend_from_slice(b"msdh"); // compatible_brand 0
        data.extend_from_slice(b"msix"); // compatible_brand 1
        data
    }

    #[test]
    fn parse_styp() {
        let data = make_styp();
        let view = SegmentTypeBoxView::new(&data).unwrap();

        assert_eq!(view.major_brand(), BrandCode::MSDH);
        assert_eq!(view.minor_version(), 0);
        assert_eq!(view.compatible_brands_count(), 2);
        assert_eq!(view.compatible_brand(0), Some(BrandCode::MSDH));
        assert_eq!(view.compatible_brand(1), Some(BrandCode::MSIX));
    }

    #[test]
    fn roundtrip() {
        let data = make_styp();
        let view = SegmentTypeBoxView::new(&data).unwrap();
        let owned = SegmentTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
