//! Sample Auxiliary Information Offsets Box (saio) parsing and serialization.
//!
//! The Sample Auxiliary Information Offsets Box contains the file offsets
//! of sample auxiliary information.
//!
//! ```text
//! aligned(8) class SampleAuxiliaryInformationOffsetsBox
//!    extends FullBox('saio', version, flags) {
//!    if (flags & 1) {
//!       unsigned int(32) aux_info_type;
//!       unsigned int(32) aux_info_type_parameter;
//!    }
//!    unsigned int(32) entry_count;
//!    if (version == 0) {
//!       unsigned int(32) offset[ entry_count ];
//!    } else {
//!       unsigned int(64) offset[ entry_count ];
//!    }
//! }
//! ```

use crate::error::{ParseError, validate_entry_count};
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for SampleAuxiliaryInformationOffsetsBox.
pub const BOX_TYPE: BoxCode = BoxCode::SAIO;

/// Common interface for accessing SampleAuxiliaryInformationOffsetsBox data.
pub trait SampleAuxiliaryInformationOffsetsBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the auxiliary info type (if flags & 1).
    fn aux_info_type(&self) -> Option<FourCC>;

    /// Returns the auxiliary info type parameter (if flags & 1).
    fn aux_info_type_parameter(&self) -> Option<u32>;

    /// Returns the entry count.
    fn entry_count(&self) -> u32;

    /// Returns an iterator over all offsets.
    fn offsets(&self) -> impl Iterator<Item = u64> + '_;
}

/// A borrowing view over raw SampleAuxiliaryInformationOffsetsBox bytes.
#[derive(Clone, Copy)]
pub struct SampleAuxiliaryInformationOffsetsBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    fl: u32,
    entry_count: u32,
    offsets_offset: usize,
}

impl<'a> SampleAuxiliaryInformationOffsetsBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let fl = header.flags;

        let mut offset_after_flags = 0;

        // If flags & 1, aux_info_type and aux_info_type_parameter are present
        if fl & 1 != 0 {
            offset_after_flags += 8;
        }

        let fullbox_offset = header.validate(data, BOX_TYPE, None, offset_after_flags + 4)?;

        let mut offset = fullbox_offset + 4 + offset_after_flags;
        let entry_count = BigEndian::read_u32(&data[offset..offset + 4]);
        offset += 4;

        let offset_size = if version == 0 { 4 } else { 8 };
        validate_entry_count(data, offset, entry_count, offset_size)?;

        Ok(Self {
            data,
            fullbox_offset,
            version,
            fl,
            entry_count,
            offsets_offset: offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    fn offset_size(&self) -> usize {
        if self.version == 0 { 4 } else { 8 }
    }

    /// Returns the offset at the given index.
    pub fn offset(&self, index: usize) -> Option<u64> {
        if index >= self.entry_count as usize {
            return None;
        }

        let offset_size = self.offset_size();
        let start = self.offsets_offset + index * offset_size;

        if self.version == 0 {
            Some(BigEndian::read_u32(&self.data[start..start + 4]) as u64)
        } else {
            Some(BigEndian::read_u64(&self.data[start..start + 8]))
        }
    }

}

impl<'a> SampleAuxiliaryInformationOffsetsBox for SampleAuxiliaryInformationOffsetsBoxView<'a> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        self.fl
    }

    fn aux_info_type(&self) -> Option<FourCC> {
        if self.fl & 1 != 0 {
            let o = self.fullbox_offset + 4;
            Some(FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]]))
        } else {
            None
        }
    }

    fn aux_info_type_parameter(&self) -> Option<u32> {
        if self.fl & 1 != 0 {
            let o = self.fullbox_offset + 8;
            Some(BigEndian::read_u32(&self.data[o..o + 4]))
        } else {
            None
        }
    }

    fn entry_count(&self) -> u32 {
        self.entry_count
    }

    fn offsets(&self) -> impl Iterator<Item = u64> + '_ {
        let offset_size = self.offset_size();
        let count = self.entry_count as usize;
        let end = self.offsets_offset + count * offset_size;
        let version = self.version;
        self.data[self.offsets_offset..end]
            .chunks_exact(offset_size)
            .map(move |chunk| {
                if version == 0 {
                    BigEndian::read_u32(chunk) as u64
                } else {
                    BigEndian::read_u64(chunk)
                }
            })
    }
}

impl std::fmt::Debug for SampleAuxiliaryInformationOffsetsBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleAuxiliaryInformationOffsetsBoxView")
            .field("version", &self.version())
            .field("entry_count", &self.entry_count())
            .finish()
    }
}

/// An owned representation of SampleAuxiliaryInformationOffsetsBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SampleAuxiliaryInformationOffsetsBoxOwned {
    /// Version (0 uses 32-bit offsets, 1 uses 64-bit).
    pub version: u8,
    /// Flags.
    pub flags: u32,
    /// Auxiliary info type (if flags & 1).
    pub aux_info_type: Option<FourCC>,
    /// Auxiliary info type parameter (if flags & 1).
    pub aux_info_type_parameter: Option<u32>,
    /// Offsets.
    pub offsets: Vec<u64>,
}

impl SampleAuxiliaryInformationOffsetsBoxOwned {
    /// Creates a new SampleAuxiliaryInformationOffsetsBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    fn requires_v1(&self) -> bool {
        self.offsets.iter().any(|&o| o > u32::MAX as u64)
    }

    fn actual_version(&self) -> u8 {
        if self.requires_v1() { 1 } else { self.version }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let offset_size: u64 = if self.actual_version() == 0 { 4 } else { 8 };
        let mut payload = 4u64 + self.offsets.len() as u64 * offset_size; // entry_count + offsets
        if self.flags & 1 != 0 {
            payload += 8;
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let version = self.actual_version();
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;

        if self.flags & 1 != 0 {
            writer.write_all(&self.aux_info_type.unwrap_or(FourCC([0; 4])).0)?;
            writer.write_u32::<BigEndian>(self.aux_info_type_parameter.unwrap_or(0))?;
        }

        writer.write_u32::<BigEndian>(self.offsets.len() as u32)?;

        for offset in &self.offsets {
            if version == 0 {
                writer.write_u32::<BigEndian>(*offset as u32)?;
            } else {
                writer.write_u64::<BigEndian>(*offset)?;
            }
        }

        Ok(())
    }
}


impl SampleAuxiliaryInformationOffsetsBox for SampleAuxiliaryInformationOffsetsBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.actual_version()
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn aux_info_type(&self) -> Option<FourCC> {
        self.aux_info_type
    }

    fn aux_info_type_parameter(&self) -> Option<u32> {
        self.aux_info_type_parameter
    }

    fn entry_count(&self) -> u32 {
        self.offsets.len() as u32
    }

    fn offsets(&self) -> impl Iterator<Item = u64> + '_ {
        self.offsets.iter().copied()
    }
}

impl<T: SampleAuxiliaryInformationOffsetsBox> From<&T> for SampleAuxiliaryInformationOffsetsBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            version: source.version(),
            flags: source.flags(),
            aux_info_type: source.aux_info_type(),
            aux_info_type_parameter: source.aux_info_type_parameter(),
            offsets: source.offsets().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_saio_v0() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 4 = 20 bytes (no aux info type, 1 offset)
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"saio");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // entry_count
        data.extend_from_slice(&1000u32.to_be_bytes()); // offset[0]
        data
    }

    #[test]
    fn parse_saio_v0() {
        let data = make_saio_v0();
        let view = SampleAuxiliaryInformationOffsetsBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.entry_count(), 1);
        assert_eq!(view.offset(0), Some(1000));
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_saio_v0();
        let view = SampleAuxiliaryInformationOffsetsBoxView::new(&data).unwrap();
        let owned = SampleAuxiliaryInformationOffsetsBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
