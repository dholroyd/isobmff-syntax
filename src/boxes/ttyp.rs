//! Track Type Box (ttyp) parsing and serialization.
//!
//! The Track Type Box declares the track's type and brands.
//!
//! ```text
//! aligned(8) class TrackTypeBox
//!    extends FullBox('ttyp', 0, 0) {
//!    unsigned int(32) major_brand;
//!    unsigned int(32) minor_version;
//!    unsigned int(32) compatible_brands[]; // to end of the box
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, BrandCode};
use std::io::{self, Write};

/// The box type identifier for TrackTypeBox.
pub const BOX_TYPE: BoxCode = BoxCode::TTYP;

/// Common interface for accessing TrackTypeBox data.
pub trait TrackTypeBox {
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

    /// Returns all compatible brands.
    fn compatible_brands(&self) -> Vec<BrandCode>;
}

/// A borrowing view over raw TrackTypeBox bytes.
#[derive(Clone, Copy)]
pub struct TrackTypeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> TrackTypeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the compatible brand at the given index.
    pub fn compatible_brand(&self, index: usize) -> Option<BrandCode> {
        if index >= self.compatible_brands_count() {
            return None;
        }

        let offset = self.fullbox_offset + 4 + 8 + index * 4;
        if offset + 4 > self.data.len() {
            return None;
        }

        let mut brand = [0u8; 4];
        brand.copy_from_slice(&self.data[offset..offset + 4]);
        Some(BrandCode::new(brand))
    }

    /// Returns all compatible brands.
    pub fn compatible_brands(&self) -> Vec<BrandCode> {
        (0..self.compatible_brands_count())
            .filter_map(|i| self.compatible_brand(i))
            .collect()
    }
}

impl<'a> TrackTypeBox for TrackTypeBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn major_brand(&self) -> BrandCode {
        let o = self.fullbox_offset + 4; // after version/flags
        BrandCode::new([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn minor_version(&self) -> u32 {
        let o = self.fullbox_offset + 4 + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn compatible_brands_count(&self) -> usize {
        let remaining = self.data.len() - self.fullbox_offset - 4 - 8;
        remaining / 4
    }

    fn compatible_brands(&self) -> Vec<BrandCode> {
        self.compatible_brands()
    }
}

impl std::fmt::Debug for TrackTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackTypeBoxView")
            .field("major_brand", &self.major_brand())
            .field("minor_version", &self.minor_version())
            .finish()
    }
}

/// An owned representation of TrackTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackTypeBoxOwned {
    /// Major brand.
    pub major_brand: BrandCode,
    /// Minor version.
    pub minor_version: u32,
    /// Compatible brands.
    pub compatible_brands: Vec<BrandCode>,
}

impl TrackTypeBoxOwned {
    /// Creates a new TrackTypeBoxOwned.
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
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, 0)?;
        writer.write_all(&self.major_brand.0 .0)?;
        writer.write_u32::<BigEndian>(self.minor_version)?;

        for brand in &self.compatible_brands {
            writer.write_all(&brand.0 .0)?;
        }

        Ok(())
    }
}

impl Default for TrackTypeBoxOwned {
    fn default() -> Self {
        Self::new(BrandCode::new(*b"vide"), 0)
    }
}

impl TrackTypeBox for TrackTypeBoxOwned {
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

    fn compatible_brands(&self) -> Vec<BrandCode> {
        self.compatible_brands.clone()
    }
}

impl<T: TrackTypeBox> From<&T> for TrackTypeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            major_brand: source.major_brand(),
            minor_version: source.minor_version(),
            compatible_brands: source.compatible_brands(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ttyp() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 8 + 4 = 24 bytes
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"ttyp");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"vide"); // major_brand
        data.extend_from_slice(&1u32.to_be_bytes()); // minor_version
        data.extend_from_slice(b"mp41"); // compatible_brand
        data
    }

    #[test]
    fn parse_ttyp() {
        let data = make_ttyp();
        let view = TrackTypeBoxView::new(&data).unwrap();

        assert_eq!(view.major_brand(), BrandCode::new(*b"vide"));
        assert_eq!(view.minor_version(), 1);
        assert_eq!(view.compatible_brands_count(), 1);
        assert_eq!(view.compatible_brand(0), Some(BrandCode::MP41));
    }

    #[test]
    fn roundtrip() {
        let data = make_ttyp();
        let view = TrackTypeBoxView::new(&data).unwrap();
        let owned = TrackTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
