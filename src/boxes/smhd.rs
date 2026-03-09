//! Sound Media Header Box (smhd) parsing and serialization.
//!
//! The Sound Media Header Box contains general presentation information for audio.
//!
//! ```text
//! aligned(8) class SoundMediaHeaderBox
//!    extends FullBox('smhd', version = 0, 0) {
//!    template int(16) balance = 0;
//!    const unsigned int(16) reserved = 0;
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use crate::types::FixedPoint8_8;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SoundMediaHeaderBox.
pub const BOX_TYPE: BoxCode = BoxCode::SMHD;

/// Common interface for accessing SoundMediaHeaderBox data.
pub trait SoundMediaHeaderBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the audio balance (0 = center, -1.0 = left, +1.0 = right).
    fn balance(&self) -> FixedPoint8_8;
}

/// A borrowing view over raw SoundMediaHeaderBox bytes.
#[derive(Clone, Copy)]
pub struct SoundMediaHeaderBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SoundMediaHeaderBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    fn payload_offset(&self) -> usize {
        self.fullbox_offset + 4
    }
}

impl SoundMediaHeaderBox for SoundMediaHeaderBoxView<'_> {
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

    fn balance(&self) -> FixedPoint8_8 {
        let o = self.payload_offset();
        FixedPoint8_8::from_raw(BigEndian::read_i16(&self.data[o..o + 2]))
    }
}

impl std::fmt::Debug for SoundMediaHeaderBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoundMediaHeaderBoxView")
            .field("box_size", &self.box_size())
            .field("balance", &self.balance())
            .finish()
    }
}

/// An owned representation of SoundMediaHeaderBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundMediaHeaderBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Audio balance.
    pub balance: FixedPoint8_8,
}

impl SoundMediaHeaderBoxOwned {
    /// Creates a new SoundMediaHeaderBoxOwned with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        fullbox_header_size_for_payload(4) + 4 // 8 + 4 + 4
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        write_fullbox_header(writer, self.serialized_size(), BOX_TYPE, 0, self.flags)?;
        writer.write_i16::<BigEndian>(self.balance.raw())?;
        writer.write_u16::<BigEndian>(0)?; // reserved
        Ok(())
    }
}

impl Default for SoundMediaHeaderBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            balance: FixedPoint8_8::ZERO,
        }
    }
}

impl SoundMediaHeaderBox for SoundMediaHeaderBoxOwned {
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

    fn balance(&self) -> FixedPoint8_8 {
        self.balance
    }
}

impl<T: SoundMediaHeaderBox> From<&T> for SoundMediaHeaderBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            balance: source.balance(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_smhd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"smhd");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0i16.to_be_bytes()); // balance = 0.0
        data.extend_from_slice(&0u16.to_be_bytes()); // reserved
        data
    }

    #[test]
    fn parse_smhd() {
        let data = make_smhd();
        let view = SoundMediaHeaderBoxView::new(&data).unwrap();

        assert_eq!(view.flags(), 0);
        assert_eq!(view.balance(), FixedPoint8_8::ZERO);
    }

    #[test]
    fn roundtrip() {
        let data = make_smhd();
        let view = SoundMediaHeaderBoxView::new(&data).unwrap();
        let owned = SoundMediaHeaderBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
