//! Sample Dependency Type Box (sdtp) parsing and serialization.
//!
//! The Sample Dependency Type Box provides information about sample dependencies.
//!
//! ```text
//! aligned(8) class SampleDependencyTypeBox
//!    extends FullBox('sdtp', version = 0, 0) {
//!    for (i=0; i < sample_count; i++) {
//!       unsigned int(2) is_leading;
//!       unsigned int(2) sample_depends_on;
//!       unsigned int(2) sample_is_depended_on;
//!       unsigned int(2) sample_has_redundancy;
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SampleDependencyTypeBox.
pub const BOX_TYPE: BoxCode = BoxCode::SDTP;

/// Sample dependency flags for a single sample, stored as a packed byte.
///
/// ```text
/// bit 7-6: is_leading
/// bit 5-4: sample_depends_on
/// bit 3-2: sample_is_depended_on
/// bit 1-0: sample_has_redundancy
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct SampleDependencyFlags(u8);

impl SampleDependencyFlags {
    /// Creates flags from the four 2-bit field values.
    pub fn new(is_leading: u8, sample_depends_on: u8, sample_is_depended_on: u8, sample_has_redundancy: u8) -> Self {
        Self(
            ((is_leading & 0x3) << 6)
                | ((sample_depends_on & 0x3) << 4)
                | ((sample_is_depended_on & 0x3) << 2)
                | (sample_has_redundancy & 0x3),
        )
    }

    /// Creates flags from a raw packed byte.
    #[inline]
    pub fn from_byte(b: u8) -> Self {
        Self(b)
    }

    /// Returns the raw packed byte.
    #[inline]
    pub fn to_byte(self) -> u8 {
        self.0
    }

    /// Is leading (2 bits).
    #[inline]
    pub fn is_leading(self) -> u8 {
        (self.0 >> 6) & 0x3
    }

    /// Sets the is_leading field (2 bits).
    #[inline]
    pub fn set_is_leading(&mut self, value: u8) {
        self.0 = (self.0 & !0xC0) | ((value & 0x3) << 6);
    }

    /// Sample depends on (2 bits).
    #[inline]
    pub fn sample_depends_on(self) -> u8 {
        (self.0 >> 4) & 0x3
    }

    /// Sets the sample_depends_on field (2 bits).
    #[inline]
    pub fn set_sample_depends_on(&mut self, value: u8) {
        self.0 = (self.0 & !0x30) | ((value & 0x3) << 4);
    }

    /// Sample is depended on (2 bits).
    #[inline]
    pub fn sample_is_depended_on(self) -> u8 {
        (self.0 >> 2) & 0x3
    }

    /// Sets the sample_is_depended_on field (2 bits).
    #[inline]
    pub fn set_sample_is_depended_on(&mut self, value: u8) {
        self.0 = (self.0 & !0x0C) | ((value & 0x3) << 2);
    }

    /// Sample has redundancy (2 bits).
    #[inline]
    pub fn sample_has_redundancy(self) -> u8 {
        self.0 & 0x3
    }

    /// Sets the sample_has_redundancy field (2 bits).
    #[inline]
    pub fn set_sample_has_redundancy(&mut self, value: u8) {
        self.0 = (self.0 & !0x03) | (value & 0x3);
    }
}

impl std::fmt::Debug for SampleDependencyFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleDependencyFlags")
            .field("is_leading", &self.is_leading())
            .field("sample_depends_on", &self.sample_depends_on())
            .field("sample_is_depended_on", &self.sample_is_depended_on())
            .field("sample_has_redundancy", &self.sample_has_redundancy())
            .finish()
    }
}


/// Common interface for accessing SampleDependencyTypeBox data.
pub trait SampleDependencyTypeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the number of samples.
    fn sample_count(&self) -> usize;

    /// Returns an iterator over all sample dependency flags.
    fn all_sample_flags(&self) -> impl Iterator<Item = SampleDependencyFlags> + '_;
}

/// A borrowing view over raw SampleDependencyTypeBox bytes.
#[derive(Clone, Copy)]
pub struct SampleDependencyTypeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    sample_count: usize,
}

impl<'a> SampleDependencyTypeBoxView<'a> {
    /// Creates a new view over the given bytes.
    ///
    /// Note: The sample count is not stored in the box; it must be obtained from
    /// the associated sample table (stsz/stz2).
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        // The sample count is payload_size (one byte per sample)
        let sample_count = data.len() - fullbox_offset - 4;
        Ok(Self { data, fullbox_offset, sample_count })
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

    /// Returns the dependency flags for the sample at the given index.
    pub fn sample_flags(&self, index: usize) -> Option<SampleDependencyFlags> {
        if index >= self.sample_count {
            return None;
        }
        let o = self.payload_offset() + index;
        Some(SampleDependencyFlags::from_byte(self.data[o]))
    }

}

impl SampleDependencyTypeBox for SampleDependencyTypeBoxView<'_> {
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

    fn sample_count(&self) -> usize {
        self.sample_count
    }

    fn all_sample_flags(&self) -> impl Iterator<Item = SampleDependencyFlags> + '_ {
        let start = self.payload_offset();
        self.data[start..start + self.sample_count]
            .iter()
            .map(|&b| SampleDependencyFlags::from_byte(b))
    }
}

impl std::fmt::Debug for SampleDependencyTypeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleDependencyTypeBoxView")
            .field("sample_count", &self.sample_count())
            .finish()
    }
}

/// An owned representation of SampleDependencyTypeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SampleDependencyTypeBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Sample dependency flags.
    pub samples: Vec<SampleDependencyFlags>,
}

impl SampleDependencyTypeBoxOwned {
    /// Creates a new empty SampleDependencyTypeBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = (self.samples.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;

        for sample in &self.samples {
            writer.write_u8(sample.to_byte())?;
        }

        Ok(())
    }
}


impl SampleDependencyTypeBox for SampleDependencyTypeBoxOwned {
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

    fn sample_count(&self) -> usize {
        self.samples.len()
    }

    fn all_sample_flags(&self) -> impl Iterator<Item = SampleDependencyFlags> + '_ {
        self.samples.iter().copied()
    }
}

impl<T: SampleDependencyTypeBox> From<&T> for SampleDependencyTypeBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            samples: source.all_sample_flags().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sdtp() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&14u32.to_be_bytes()); // size = 8 + 4 + 2
        data.extend_from_slice(b"sdtp");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        // Sample 0: is_leading=0, depends_on=2, depended_on=0, redundancy=0 = 0x20
        data.push(0x20);
        // Sample 1: is_leading=0, depends_on=0, depended_on=0, redundancy=0 = 0x00
        data.push(0x00);

        data
    }

    #[test]
    fn parse_sdtp() {
        let data = make_sdtp();
        let view = SampleDependencyTypeBoxView::new(&data).unwrap();

        assert_eq!(view.sample_count(), 2);

        let flags0 = view.sample_flags(0).unwrap();
        assert_eq!(flags0.is_leading(), 0);
        assert_eq!(flags0.sample_depends_on(), 2);
        assert_eq!(flags0.sample_is_depended_on(), 0);
        assert_eq!(flags0.sample_has_redundancy(), 0);

        let flags1 = view.sample_flags(1).unwrap();
        assert_eq!(flags1.sample_depends_on(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_sdtp();
        let view = SampleDependencyTypeBoxView::new(&data).unwrap();
        let owned = SampleDependencyTypeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn flags_encoding() {
        let flags = SampleDependencyFlags::new(1, 2, 1, 3);

        let byte = flags.to_byte();
        let decoded = SampleDependencyFlags::from_byte(byte);

        assert_eq!(flags, decoded);
    }
}
