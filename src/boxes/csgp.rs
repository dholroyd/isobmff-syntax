//! Compact Sample to Group Box (csgp) parsing and serialization.
//!
//! The Compact Sample to Group Box maps samples to sample group entries
//! using a compact pattern-based encoding.
//!
//! ```text
//! aligned(8) class CompactSampleToGroupBox
//!    extends FullBox('csgp', version, flags) {
//!    unsigned int(32) grouping_type;
//!    if (grouping_type_parameter_present == 1) {
//!       unsigned int(32) grouping_type_parameter;
//!    }
//!    unsigned int(32) pattern_count;
//!    for (i=1; i <= pattern_count; i++) {
//!       unsigned int(f(pattern_size_code)) pattern_length[i];
//!       unsigned int(f(count_size_code)) sample_count[i];
//!    }
//!    for (j=1; j <= pattern_count; j++) {
//!       for (k=1; k <= pattern_length[j]; k++) {
//!          unsigned int(f(index_size_code))
//!             sample_group_description_index[j][k];
//!       }
//!    }
//! }
//! ```
//!
//! Flag bits layout (24-bit flags field):
//! - Bit 0: reserved
//! - Bit 1: index_msb_indicates_fragment_local_description
//! - Bit 2: grouping_type_parameter_present
//! - Bits 3-4: pattern_size_code
//! - Bits 5-6: count_size_code
//! - Bits 7-8: index_size_code
//!
//! Size code to field width mapping (f()):
//! - 0 → 4 bits
//! - 1 → 8 bits
//! - 2 → 16 bits
//! - 3 → 32 bits

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for CompactSampleToGroupBox.
pub const BOX_TYPE: BoxCode = BoxCode::CSGP;

/// A parsed entry from the compact sample to group box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactSampleToGroupEntry {
    /// Number of times this pattern repeats (sample_count).
    pub sample_count: u32,
    /// Group description indices for each sample in the pattern.
    pub group_description_indices: Vec<u32>,
}

/// Maps a 2-bit size code to the field width in bits.
fn size_code_to_bits(code: u8) -> u8 {
    match code & 0x03 {
        0 => 4,
        1 => 8,
        2 => 16,
        3 => 32,
        _ => unreachable!(),
    }
}

/// Reads a value of the given bit width from a byte slice at a bit offset.
/// Returns (value, new_bit_offset).
fn read_field(data: &[u8], bit_offset: usize, bits: u8) -> Result<(u32, usize), ParseError> {
    let byte_offset = bit_offset / 8;
    let bit_within_byte = bit_offset % 8;

    match bits {
        4 => {
            if byte_offset >= data.len() {
                return Err(ParseError::BufferTooShort {
                    expected: byte_offset + 1,
                    found: data.len(),
                });
            }
            let val = if bit_within_byte == 0 {
                (data[byte_offset] >> 4) as u32
            } else {
                (data[byte_offset] & 0x0F) as u32
            };
            Ok((val, bit_offset + 4))
        }
        8 => {
            let bo = bit_offset.div_ceil(8); // align to byte boundary
            if bo >= data.len() {
                return Err(ParseError::BufferTooShort {
                    expected: bo + 1,
                    found: data.len(),
                });
            }
            Ok((data[bo] as u32, (bo + 1) * 8))
        }
        16 => {
            let bo = bit_offset.div_ceil(8);
            if bo + 2 > data.len() {
                return Err(ParseError::BufferTooShort {
                    expected: bo + 2,
                    found: data.len(),
                });
            }
            Ok((BigEndian::read_u16(&data[bo..bo + 2]) as u32, (bo + 2) * 8))
        }
        32 => {
            let bo = bit_offset.div_ceil(8);
            if bo + 4 > data.len() {
                return Err(ParseError::BufferTooShort {
                    expected: bo + 4,
                    found: data.len(),
                });
            }
            Ok((BigEndian::read_u32(&data[bo..bo + 4]), (bo + 4) * 8))
        }
        _ => unreachable!(),
    }
}

/// Writes a value of the given bit width to a byte vector at a bit offset.
/// Returns the new bit offset.
fn write_field(buf: &mut Vec<u8>, bit_offset: usize, bits: u8, value: u32) -> usize {
    match bits {
        4 => {
            let byte_offset = bit_offset / 8;
            let bit_within = bit_offset % 8;
            if bit_within == 0 {
                if byte_offset >= buf.len() {
                    buf.push(0);
                }
                buf[byte_offset] |= ((value & 0x0F) as u8) << 4;
            } else {
                buf[byte_offset] |= (value & 0x0F) as u8;
            }
            bit_offset + 4
        }
        8 => {
            let bo = bit_offset.div_ceil(8);
            while buf.len() <= bo {
                buf.push(0);
            }
            buf[bo] = value as u8;
            (bo + 1) * 8
        }
        16 => {
            let bo = bit_offset.div_ceil(8);
            while buf.len() < bo + 2 {
                buf.push(0);
            }
            buf[bo] = (value >> 8) as u8;
            buf[bo + 1] = value as u8;
            (bo + 2) * 8
        }
        32 => {
            let bo = bit_offset.div_ceil(8);
            while buf.len() < bo + 4 {
                buf.push(0);
            }
            buf[bo] = (value >> 24) as u8;
            buf[bo + 1] = (value >> 16) as u8;
            buf[bo + 2] = (value >> 8) as u8;
            buf[bo + 3] = value as u8;
            (bo + 4) * 8
        }
        _ => unreachable!(),
    }
}

/// Computes the bit offset after reading `count` consecutive fields of a given
/// bit width, accounting for the byte-alignment behaviour of `read_field`.
fn advance_fields(bit_offset: usize, count: usize, bits: u8) -> usize {
    match bits {
        4 => bit_offset + count * 4,
        8 => {
            let start = bit_offset.div_ceil(8);
            (start + count) * 8
        }
        16 => {
            let start = bit_offset.div_ceil(8);
            (start + count * 2) * 8
        }
        32 => {
            let start = bit_offset.div_ceil(8);
            (start + count * 4) * 8
        }
        _ => unreachable!(),
    }
}

/// A borrowing view over a single compact sample-to-group entry.
#[derive(Clone, Copy, Debug)]
pub struct CompactSampleToGroupEntryView<'a> {
    sample_count: u32,
    pattern_length: u32,
    data: &'a [u8],
    index_bit_offset: usize,
    index_bits: u8,
}

impl<'a> CompactSampleToGroupEntryView<'a> {
    /// Returns the number of times this pattern repeats.
    #[inline]
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Returns the number of indices in this pattern.
    #[inline]
    pub fn pattern_length(&self) -> u32 {
        self.pattern_length
    }

    /// Returns an iterator over the group description indices for this pattern.
    pub fn group_description_indices(&self) -> impl Iterator<Item = Result<u32, ParseError>> + 'a {
        GroupDescriptionIndexIter {
            data: self.data,
            bit_offset: self.index_bit_offset,
            remaining: self.pattern_length,
            index_bits: self.index_bits,
        }
    }

    /// Materialises an owned copy of this entry.
    pub fn to_owned(&self) -> Result<CompactSampleToGroupEntry, ParseError> {
        Ok(CompactSampleToGroupEntry {
            sample_count: self.sample_count,
            group_description_indices: self.group_description_indices().collect::<Result<_, _>>()?,
        })
    }
}

/// Iterator over group description indices within a single pattern entry.
struct GroupDescriptionIndexIter<'a> {
    data: &'a [u8],
    bit_offset: usize,
    remaining: u32,
    index_bits: u8,
}

impl Iterator for GroupDescriptionIndexIter<'_> {
    type Item = Result<u32, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        match read_field(self.data, self.bit_offset, self.index_bits) {
            Ok((val, new_offset)) => {
                self.bit_offset = new_offset;
                Some(Ok(val))
            }
            Err(e) => {
                self.remaining = 0;
                Some(Err(e))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining as usize))
    }
}

/// Iterator over compact sample-to-group entries, yielding borrowing views.
struct CompactSampleToGroupIter<'a> {
    data: &'a [u8],
    pattern_bits: u8,
    count_bits: u8,
    index_bits: u8,
    pair_bit_offset: usize,
    index_bit_offset: usize,
    remaining: usize,
    error: Option<ParseError>,
}

impl<'a> CompactSampleToGroupIter<'a> {
    fn new(
        data: &'a [u8],
        pattern_count: usize,
        pattern_bits: u8,
        count_bits: u8,
        index_bits: u8,
    ) -> Self {
        let mut scan_offset: usize = 0;
        for _ in 0..pattern_count {
            match read_field(data, scan_offset, pattern_bits) {
                Ok((_, o)) => scan_offset = o,
                Err(e) => {
                    return Self {
                        data, pattern_bits, count_bits, index_bits,
                        pair_bit_offset: 0, index_bit_offset: 0,
                        remaining: 0, error: Some(e),
                    };
                }
            }
            match read_field(data, scan_offset, count_bits) {
                Ok((_, o)) => scan_offset = o,
                Err(e) => {
                    return Self {
                        data, pattern_bits, count_bits, index_bits,
                        pair_bit_offset: 0, index_bit_offset: 0,
                        remaining: 0, error: Some(e),
                    };
                }
            }
        }

        Self {
            data, pattern_bits, count_bits, index_bits,
            pair_bit_offset: 0,
            index_bit_offset: scan_offset,
            remaining: pattern_count,
            error: None,
        }
    }
}

impl<'a> Iterator for CompactSampleToGroupIter<'a> {
    type Item = Result<CompactSampleToGroupEntryView<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(e) = self.error.take() {
            return Some(Err(e));
        }
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let (pattern_length, new_offset) = match read_field(self.data, self.pair_bit_offset, self.pattern_bits) {
            Ok(r) => r,
            Err(e) => { self.remaining = 0; return Some(Err(e)); }
        };
        self.pair_bit_offset = new_offset;

        let (sample_count, new_offset) = match read_field(self.data, self.pair_bit_offset, self.count_bits) {
            Ok(r) => r,
            Err(e) => { self.remaining = 0; return Some(Err(e)); }
        };
        self.pair_bit_offset = new_offset;

        let entry_index_offset = self.index_bit_offset;
        self.index_bit_offset = advance_fields(
            self.index_bit_offset,
            pattern_length as usize,
            self.index_bits,
        );

        Some(Ok(CompactSampleToGroupEntryView {
            sample_count,
            pattern_length,
            data: self.data,
            index_bit_offset: entry_index_offset,
            index_bits: self.index_bits,
        }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.error.is_some() {
            (1, Some(1))
        } else {
            (self.remaining, Some(self.remaining))
        }
    }
}

/// Common interface for accessing CompactSampleToGroupBox data.
pub trait CompactSampleToGroupBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the grouping type.
    fn grouping_type(&self) -> FourCC;

    /// Returns whether the grouping type parameter is present.
    fn has_grouping_type_parameter(&self) -> bool;

    /// Returns the grouping type parameter (if present).
    fn grouping_type_parameter(&self) -> Option<u32>;

    /// Returns the pattern count.
    fn pattern_count(&self) -> u32;

    /// Returns the pattern_size_code from the flags.
    fn pattern_size_code(&self) -> u8 {
        ((self.flags() >> 3) & 0x03) as u8
    }

    /// Returns the count_size_code from the flags.
    fn count_size_code(&self) -> u8 {
        ((self.flags() >> 5) & 0x03) as u8
    }

    /// Returns the index_size_code from the flags.
    fn index_size_code(&self) -> u8 {
        ((self.flags() >> 7) & 0x03) as u8
    }
}

/// A borrowing view over raw CompactSampleToGroupBox bytes.
#[derive(Clone, Copy)]
pub struct CompactSampleToGroupBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    flags: u32,
    /// Offset of pattern_count from start of data.
    pattern_count_offset: usize,
    pattern_count: u32,
}

impl<'a> CompactSampleToGroupBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let flags = header.flags;
        let has_gtp = flags & 0x04 != 0; // bit 2
        let min_payload = 4 + if has_gtp { 4 } else { 0 } + 4;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, min_payload)?;

        let pattern_count_offset = fullbox_offset + 4 + 4 + if has_gtp { 4 } else { 0 };
        let pattern_count = BigEndian::read_u32(&data[pattern_count_offset..pattern_count_offset + 4]);

        let data_remaining = data.len().saturating_sub(pattern_count_offset + 4);
        if pattern_count as usize > data_remaining {
            return Err(ParseError::InvalidEntryCount {
                count: pattern_count,
                max_possible: data_remaining as u32,
            });
        }

        Ok(Self {
            data,
            fullbox_offset,
            version,
            flags,
            pattern_count_offset,
            pattern_count,
        })
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

    fn pattern_data(&self) -> &'a [u8] {
        let start = self.pattern_count_offset + 4;
        &self.data[start..]
    }

    /// Returns an iterator over the pattern entries.
    pub fn entries(&self) -> impl Iterator<Item = Result<CompactSampleToGroupEntryView<'a>, ParseError>> + 'a {
        CompactSampleToGroupIter::new(
            self.pattern_data(),
            self.pattern_count as usize,
            size_code_to_bits(self.pattern_size_code()),
            size_code_to_bits(self.count_size_code()),
            size_code_to_bits(self.index_size_code()),
        )
    }
}

impl<'a> CompactSampleToGroupBox for CompactSampleToGroupBoxView<'a> {
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
        self.flags
    }

    fn grouping_type(&self) -> FourCC {
        let o = self.payload_offset();
        FourCC([self.data[o], self.data[o + 1], self.data[o + 2], self.data[o + 3]])
    }

    fn has_grouping_type_parameter(&self) -> bool {
        self.flags & 0x04 != 0 // bit 2
    }

    fn grouping_type_parameter(&self) -> Option<u32> {
        if self.has_grouping_type_parameter() {
            let o = self.payload_offset() + 4;
            Some(BigEndian::read_u32(&self.data[o..o + 4]))
        } else {
            None
        }
    }

    fn pattern_count(&self) -> u32 {
        self.pattern_count
    }
}

impl std::fmt::Debug for CompactSampleToGroupBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompactSampleToGroupBoxView")
            .field("grouping_type", &self.grouping_type())
            .field("pattern_count", &self.pattern_count())
            .finish()
    }
}

/// An owned representation of CompactSampleToGroupBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactSampleToGroupBoxOwned {
    /// Flags (bits 3-8 encode size codes for serialization).
    pub flags: u32,
    /// Grouping type.
    pub grouping_type: FourCC,
    /// Grouping type parameter (present when flags bit 2 is set).
    pub grouping_type_parameter: Option<u32>,
    /// Parsed pattern entries.
    pub entries: Vec<CompactSampleToGroupEntry>,
}

impl CompactSampleToGroupBoxOwned {
    /// Creates a new CompactSampleToGroupBoxOwned.
    pub fn new(grouping_type: FourCC) -> Self {
        Self {
            flags: 0,
            grouping_type,
            grouping_type_parameter: None,
            entries: Vec::new(),
        }
    }

    /// Returns the flags value, ensuring grouping_type_parameter_present bit is consistent.
    fn effective_flags(&self) -> u32 {
        if self.grouping_type_parameter.is_some() {
            self.flags | 0x04 // bit 2
        } else {
            self.flags & !0x04
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let gtp_size = if self.grouping_type_parameter.is_some() { 4 } else { 0 };
        let payload = (4 + gtp_size + 4 + self.pattern_data_size()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Computes the byte size of the pattern data without serializing.
    fn pattern_data_size(&self) -> usize {
        let pattern_bits = size_code_to_bits(((self.flags >> 3) & 0x03) as u8) as usize;
        let count_bits = size_code_to_bits(((self.flags >> 5) & 0x03) as u8) as usize;
        let index_bits = size_code_to_bits(((self.flags >> 7) & 0x03) as u8) as usize;

        let pairs_bits = self.entries.len() * (pattern_bits + count_bits);
        let indices_bits: usize = self.entries.iter()
            .map(|e| e.group_description_indices.len() * index_bits)
            .sum();
        (pairs_bits + indices_bits).div_ceil(8)
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.effective_flags())?;
        writer.write_all(&self.grouping_type.0)?;
        if let Some(gtp) = self.grouping_type_parameter {
            writer.write_u32::<BigEndian>(gtp)?;
        }
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
        self.write_pattern_data(writer)?;

        Ok(())
    }

    /// Writes the bit-packed pattern data directly to the writer.
    fn write_pattern_data<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let pattern_bits = size_code_to_bits(((self.flags >> 3) & 0x03) as u8);
        let count_bits = size_code_to_bits(((self.flags >> 5) & 0x03) as u8);
        let index_bits = size_code_to_bits(((self.flags >> 7) & 0x03) as u8);

        let mut buf = Vec::new();
        let mut bit_offset: usize = 0;

        // First loop: pattern_length and sample_count pairs
        for entry in &self.entries {
            bit_offset = write_field(&mut buf, bit_offset, pattern_bits, entry.group_description_indices.len() as u32);
            bit_offset = write_field(&mut buf, bit_offset, count_bits, entry.sample_count);
        }

        // Second loop: indices
        for entry in &self.entries {
            for &index in &entry.group_description_indices {
                bit_offset = write_field(&mut buf, bit_offset, index_bits, index);
            }
        }

        // Pad to byte boundary
        let final_byte_len = bit_offset.div_ceil(8);
        buf.resize(final_byte_len, 0);

        writer.write_all(&buf)
    }
}

impl Default for CompactSampleToGroupBoxOwned {
    fn default() -> Self {
        Self::new(FourCC(*b"roll"))
    }
}

impl CompactSampleToGroupBox for CompactSampleToGroupBoxOwned {
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
        self.effective_flags()
    }

    fn grouping_type(&self) -> FourCC {
        self.grouping_type
    }

    fn has_grouping_type_parameter(&self) -> bool {
        self.grouping_type_parameter.is_some()
    }

    fn grouping_type_parameter(&self) -> Option<u32> {
        self.grouping_type_parameter
    }

    fn pattern_count(&self) -> u32 {
        self.entries.len() as u32
    }
}

impl TryFrom<&CompactSampleToGroupBoxView<'_>> for CompactSampleToGroupBoxOwned {
    type Error = ParseError;

    fn try_from(source: &CompactSampleToGroupBoxView<'_>) -> Result<Self, ParseError> {
        let entries: Vec<CompactSampleToGroupEntry> = source.entries()
            .map(|r| r.and_then(|v| v.to_owned()))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            flags: source.flags(),
            grouping_type: source.grouping_type(),
            grouping_type_parameter: source.grouping_type_parameter(),
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_csgp_empty() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 8 = 20 bytes (empty patterns)
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"csgp");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags (all size codes = 0 → 4-bit)
        data.extend_from_slice(b"roll"); // grouping_type
        data.extend_from_slice(&0u32.to_be_bytes()); // pattern_count
        data
    }

    fn make_csgp_8bit() -> Vec<u8> {
        // size codes: pattern=1(8bit), count=1(8bit), index=1(8bit)
        // flags bits 3-4=01, bits 5-6=01, bits 7-8=01
        // = 0b0_01_01_01_0_0_0 = bits: reserved(0) index_msb(0) gtp(0) pattern(01) count(01) index(01)
        // = 0x00_00_AA ... let me calculate:
        // bit 0: reserved = 0
        // bit 1: index_msb = 0
        // bit 2: gtp = 0
        // bits 3-4: pattern_size_code = 01 = 0x08
        // bits 5-6: count_size_code = 01 = 0x20
        // bits 7-8: index_size_code = 01 = 0x80
        // flags = 0x08 | 0x20 | 0x80 = 0xA8
        let flags: u32 = 0x0000A8;

        let mut data = Vec::new();
        // 8 + 4 + 4 + 4 + 2 + 2 = 24 bytes
        // pattern_data: 1 pattern × (1 byte pattern_length + 1 byte sample_count) + 2 × 1 byte index = 4 bytes
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"csgp");
        data.push(0); // version
        // flags as 3 bytes big-endian
        data.push(0);
        data.push(0);
        data.push(flags as u8);
        data.extend_from_slice(b"roll"); // grouping_type
        data.extend_from_slice(&1u32.to_be_bytes()); // pattern_count = 1

        // pattern_length=2, sample_count=5
        data.push(2); // pattern_length (8-bit)
        data.push(5); // sample_count (8-bit)
        // indices: [10, 20]
        data.push(10);
        data.push(20);
        data
    }

    #[test]
    fn parse_csgp_empty() {
        let data = make_csgp_empty();
        let view = CompactSampleToGroupBoxView::new(&data).unwrap();

        assert_eq!(view.grouping_type(), FourCC(*b"roll"));
        assert_eq!(view.pattern_count(), 0);
        let entries: Vec<_> = view.entries().collect::<Result<_, _>>().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn parse_csgp_8bit() {
        let data = make_csgp_8bit();
        let view = CompactSampleToGroupBoxView::new(&data).unwrap();

        assert_eq!(view.pattern_count(), 1);
        assert_eq!(view.pattern_size_code(), 1);
        assert_eq!(view.count_size_code(), 1);
        assert_eq!(view.index_size_code(), 1);

        let entries: Vec<_> = view.entries()
            .map(|r| r.and_then(|v| v.to_owned()))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sample_count, 5);
        assert_eq!(entries[0].group_description_indices, vec![10, 20]);
    }

    #[test]
    fn roundtrip_empty() {
        let data = make_csgp_empty();
        let view = CompactSampleToGroupBoxView::new(&data).unwrap();
        let owned = CompactSampleToGroupBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_8bit() {
        let data = make_csgp_8bit();
        let view = CompactSampleToGroupBoxView::new(&data).unwrap();
        let owned = CompactSampleToGroupBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
