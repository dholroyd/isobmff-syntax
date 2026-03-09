//! Padding Bits Box (padb) parsing and serialization.
//!
//! The Padding Bits Box specifies padding bits for samples.
//!
//! ```text
//! aligned(8) class PaddingBitsBox
//!    extends FullBox('padb', version = 0, 0) {
//!    unsigned int(32) sample_count;
//!    for (i=0; i < ((sample_count + 1)/2); i++) {
//!       bit(1) reserved = 0;
//!       bit(3) pad1;
//!       bit(1) reserved = 0;
//!       bit(3) pad2;
//!    }
//! }
//! ```

use crate::error::{validate_entry_count, ParseError};
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for PaddingBitsBox.
pub const BOX_TYPE: BoxCode = BoxCode::PADB;

/// Common interface for accessing PaddingBitsBox data.
pub trait PaddingBitsBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the sample count.
    fn sample_count(&self) -> u32;

    /// Returns all padding values.
    fn paddings(&self) -> impl Iterator<Item = (u8, u8)> + '_;
}

/// A borrowing view over raw PaddingBitsBox bytes.
#[derive(Clone, Copy)]
pub struct PaddingBitsBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    sample_count: u32,
}

impl<'a> PaddingBitsBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;
        let sample_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);
        // Two samples share each padding byte. Bound the count by the bytes
        // present, so a hostile count cannot make `paddings()` iterate billions
        // of times to yield nothing.
        validate_entry_count(data, fullbox_offset + 8, sample_count.div_ceil(2), 1)?;
        Ok(Self { data, fullbox_offset, sample_count })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 8
    }

    /// Returns the padding value for a sample pair.
    /// Each byte contains padding for 2 samples: pad1 in high nibble, pad2 in low nibble.
    pub fn padding(&self, index: usize) -> Option<(u8, u8)> {
        let offset = self.payload_offset() + index;
        if offset >= self.data.len() {
            return None;
        }

        let byte = self.data[offset];
        // High nibble: reserved(1 bit) + pad1(3 bits)
        // Low nibble: reserved(1 bit) + pad2(3 bits)
        let pad1 = (byte >> 4) & 0x07;
        let pad2 = byte & 0x07;
        Some((pad1, pad2))
    }

}

impl<'a> PaddingBitsBox for PaddingBitsBoxView<'a> {
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

    fn sample_count(&self) -> u32 {
        self.sample_count
    }

    fn paddings(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        let count = (self.sample_count as usize).div_ceil(2);
        (0..count).filter_map(|i| self.padding(i))
    }
}

impl std::fmt::Debug for PaddingBitsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaddingBitsBoxView")
            .field("sample_count", &self.sample_count())
            .finish()
    }
}

/// An owned representation of PaddingBitsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct PaddingBitsBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Sample count.
    pub sample_count: u32,
    /// Padding values (each tuple has pad1, pad2).
    pub paddings: Vec<(u8, u8)>,
}

impl PaddingBitsBoxOwned {
    /// Creates a new PaddingBitsBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let padding_bytes = (self.sample_count as usize).div_ceil(2);
        let payload = (4 + padding_bytes) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.sample_count)?;

        for (pad1, pad2) in &self.paddings {
            let byte = ((pad1 & 0x07) << 4) | (pad2 & 0x07);
            writer.write_u8(byte)?;
        }

        Ok(())
    }
}


impl PaddingBitsBox for PaddingBitsBoxOwned {
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

    fn sample_count(&self) -> u32 {
        self.sample_count
    }

    fn paddings(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        self.paddings.iter().copied()
    }
}

impl<T: PaddingBitsBox> From<&T> for PaddingBitsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            sample_count: source.sample_count(),
            paddings: source.paddings().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_padb() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 1 = 17 bytes (2 samples)
        data.extend_from_slice(&17u32.to_be_bytes());
        data.extend_from_slice(b"padb");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&2u32.to_be_bytes()); // sample_count
        data.push(0x12); // pad1=1, pad2=2
        data
    }

    #[test]
    fn parse_padb() {
        let data = make_padb();
        let view = PaddingBitsBoxView::new(&data).unwrap();

        assert_eq!(view.sample_count(), 2);
        assert_eq!(view.padding(0), Some((1, 2)));
    }

    #[test]
    fn roundtrip() {
        let data = make_padb();
        let view = PaddingBitsBoxView::new(&data).unwrap();
        let owned = PaddingBitsBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
