//! Compact Sample Size Box (stz2) parsing and serialization.
//!
//! The Compact Sample Size Box stores sample sizes in a more compact format
//! using 4, 8, or 16 bits per sample.
//!
//! ```text
//! aligned(8) class CompactSampleSizeBox
//!    extends FullBox('stz2', version = 0, 0) {
//!    unsigned int(24) reserved = 0;
//!    unsigned int(8) field_size;
//!    unsigned int(32) sample_count;
//!    for (i=1; i <= sample_count; i++) {
//!       unsigned int(field_size) entry_size;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for CompactSampleSizeBox.
pub const BOX_TYPE: BoxCode = BoxCode::STZ2;

/// Common interface for accessing CompactSampleSizeBox data.
pub trait CompactSampleSizeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the field size in bits (4, 8, or 16).
    fn field_size(&self) -> u8;

    /// Returns the sample count.
    fn sample_count(&self) -> u32;

    /// Returns an iterator over all sample sizes.
    fn sample_sizes(&self) -> impl Iterator<Item = u16> + '_;
}

/// A borrowing view over raw CompactSampleSizeBox bytes.
#[derive(Clone, Copy)]
pub struct CompactSampleSizeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    field_size: u8,
    sample_count: u32,
    sizes_offset: usize,
}

impl<'a> CompactSampleSizeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;
        let field_size = data[fullbox_offset + 7];
        let sample_count = BigEndian::read_u32(&data[fullbox_offset + 8..fullbox_offset + 12]);
        let sizes_offset = fullbox_offset + 12;

        // `field_size` is a width in bits, and only these three are defined.
        if !matches!(field_size, 4 | 8 | 16) {
            return Err(ParseError::InvalidFieldSize { field: "field_size", size: field_size });
        }

        // Bound `sample_count` by the bytes actually present, so that a hostile
        // count cannot make `sample_sizes()` iterate billions of times to yield
        // nothing. Two 4-bit entries share a byte.
        let available = data.len() - sizes_offset;
        let max_samples = match field_size {
            4 => available.saturating_mul(2),
            8 => available,
            _ => available / 2,
        };
        if sample_count as usize > max_samples {
            return Err(ParseError::InvalidEntryCount {
                count: sample_count,
                max_possible: max_samples as u32,
            });
        }

        Ok(Self {
            data,
            fullbox_offset,
            field_size,
            sample_count,
            sizes_offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the size of the sample at the given index.
    pub fn sample_size(&self, sample: usize) -> Option<u16> {
        if sample >= self.sample_count as usize {
            return None;
        }

        match self.field_size {
            4 => {
                let byte_index = sample / 2;
                let offset = self.sizes_offset + byte_index;
                if offset >= self.data.len() {
                    return None;
                }
                let byte = self.data[offset];
                if sample.is_multiple_of(2) {
                    Some((byte >> 4) as u16)
                } else {
                    Some((byte & 0x0F) as u16)
                }
            }
            8 => {
                let offset = self.sizes_offset + sample;
                if offset >= self.data.len() {
                    return None;
                }
                Some(self.data[offset] as u16)
            }
            16 => {
                let offset = self.sizes_offset + sample * 2;
                if offset + 2 > self.data.len() {
                    return None;
                }
                Some(BigEndian::read_u16(&self.data[offset..offset + 2]))
            }
            _ => None,
        }
    }

}

impl<'a> CompactSampleSizeBox for CompactSampleSizeBoxView<'a> {
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

    fn field_size(&self) -> u8 {
        self.field_size
    }

    fn sample_count(&self) -> u32 {
        self.sample_count
    }

    fn sample_sizes(&self) -> impl Iterator<Item = u16> + '_ {
        (0..self.sample_count as usize).filter_map(|i| self.sample_size(i))
    }
}

impl std::fmt::Debug for CompactSampleSizeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompactSampleSizeBoxView")
            .field("field_size", &self.field_size())
            .field("sample_count", &self.sample_count())
            .finish()
    }
}

/// An owned representation of CompactSampleSizeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactSampleSizeBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Field size in bits (4, 8, or 16).
    pub field_size: u8,
    /// Sample sizes.
    pub sample_sizes: Vec<u16>,
}

impl CompactSampleSizeBoxOwned {
    /// Creates a new CompactSampleSizeBoxOwned.
    pub fn new(field_size: u8) -> Self {
        Self {
            flags: 0,
            field_size,
            sample_sizes: Vec::new(),
        }
    }

    fn packed_data_size(&self) -> usize {
        match self.field_size {
            4 => self.sample_sizes.len().div_ceil(2),
            8 => self.sample_sizes.len(),
            16 => self.sample_sizes.len() * 2,
            _ => 0,
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (4 + 4 + self.packed_data_size()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u24::<BigEndian>(0)?; // reserved
        writer.write_u8(self.field_size)?;
        writer.write_u32::<BigEndian>(self.sample_sizes.len() as u32)?;

        match self.field_size {
            4 => {
                for chunk in self.sample_sizes.chunks(2) {
                    let high = (chunk[0] & 0x0F) as u8;
                    let low = chunk.get(1).map(|&v| (v & 0x0F) as u8).unwrap_or(0);
                    writer.write_u8((high << 4) | low)?;
                }
            }
            8 => {
                for &size in &self.sample_sizes {
                    writer.write_u8(size as u8)?;
                }
            }
            16 => {
                for &size in &self.sample_sizes {
                    writer.write_u16::<BigEndian>(size)?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl Default for CompactSampleSizeBoxOwned {
    fn default() -> Self {
        Self::new(8)
    }
}

impl CompactSampleSizeBox for CompactSampleSizeBoxOwned {
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

    fn field_size(&self) -> u8 {
        self.field_size
    }

    fn sample_count(&self) -> u32 {
        self.sample_sizes.len() as u32
    }

    fn sample_sizes(&self) -> impl Iterator<Item = u16> + '_ {
        self.sample_sizes.iter().copied()
    }
}

impl<T: CompactSampleSizeBox> From<&T> for CompactSampleSizeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            field_size: source.field_size(),
            sample_sizes: source.sample_sizes().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stz2_8bit() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 4 + 2 = 22 bytes
        data.extend_from_slice(&22u32.to_be_bytes());
        data.extend_from_slice(b"stz2");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&[0, 0, 0]); // reserved
        data.push(8); // field_size
        data.extend_from_slice(&2u32.to_be_bytes()); // sample_count
        data.push(100); // size[0]
        data.push(200); // size[1]
        data
    }

    #[test]
    fn parse_stz2_8bit() {
        let data = make_stz2_8bit();
        let view = CompactSampleSizeBoxView::new(&data).unwrap();

        assert_eq!(view.field_size(), 8);
        assert_eq!(view.sample_count(), 2);
        assert_eq!(view.sample_size(0), Some(100));
        assert_eq!(view.sample_size(1), Some(200));
    }

    #[test]
    fn parse_stz2_buffer_too_short_for_sample_count() {
        // 16-byte stz2 box: 8-byte header + 8-byte payload.
        // Payload has version/flags(4) + reserved(3) + field_size(1) = 8 bytes,
        // but is missing the 4-byte sample_count field.
        // The old min_size check was fullbox_offset + 8, which incorrectly
        // allowed this box through, causing a panic when reading sample_count.
        let data: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x10, // size = 16
            0x73, 0x74, 0x7a, 0x32, // "stz2"
            0x00, 0x00, 0x00, 0x00, // version + flags
            0x00, 0x00, 0x00, 0x08, // reserved(3) + field_size(1)
        ];
        assert!(CompactSampleSizeBoxView::new(&data).is_err());
    }

    #[test]
    fn roundtrip_8bit() {
        let data = make_stz2_8bit();
        let view = CompactSampleSizeBoxView::new(&data).unwrap();
        let owned = CompactSampleSizeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
