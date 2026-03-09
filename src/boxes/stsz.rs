//! Sample Size Box (stsz) parsing and serialization.
//!
//! The Sample Size Box contains the sample count and size table for the media.
//!
//! ```text
//! aligned(8) class SampleSizeBox extends FullBox('stsz', version = 0, 0) {
//!    unsigned int(32) sample_size;
//!    unsigned int(32) sample_count;
//!    if (sample_size==0) {
//!       for (i=1; i <= sample_count; i++) {
//!          unsigned int(32) entry_size;
//!       }
//!    }
//! }
//! ```

use crate::error::{ParseError, validate_entry_count};
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SampleSizeBox.
pub const BOX_TYPE: BoxCode = BoxCode::STSZ;

/// Common interface for accessing SampleSizeBox data.
pub trait SampleSizeBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the default sample size (0 if sizes vary).
    fn sample_size(&self) -> u32;

    /// Returns the number of samples.
    fn sample_count(&self) -> u32;

    /// Returns the size of the sample at the given index (0-based).
    /// If sample_size is non-zero, returns that constant size.
    /// Returns `None` if `index >= sample_count()`.
    fn entry(&self, index: usize) -> Option<u32>;

    /// Returns an iterator over all sample sizes.
    fn entries(&self) -> impl Iterator<Item = u32> + '_;
}

/// An iterator over sample sizes from a `SampleSizeBoxView`.
///
/// Reads `sample_count`, `sample_size`, and the entry data offset once at
/// construction, then iterates without re-reading those fields per element.
#[derive(Clone)]
pub struct SampleSizeEntries<'a> {
    /// For variable sizes: the remaining entry data (each entry is 4 bytes big-endian u32).
    /// For constant sizes: empty.
    data: &'a [u8],
    /// The constant sample size, or 0 for variable sizes.
    constant_size: u32,
    /// Remaining count (used only for the constant-size path).
    remaining: usize,
}

impl Iterator for SampleSizeEntries<'_> {
    type Item = u32;

    #[inline]
    fn next(&mut self) -> Option<u32> {
        if self.constant_size != 0 {
            if self.remaining == 0 {
                return None;
            }
            self.remaining -= 1;
            Some(self.constant_size)
        } else {
            let (chunk, rest) = self.data.split_at_checked(4)?;
            self.data = rest;
            Some(BigEndian::read_u32(chunk))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = if self.constant_size != 0 {
            self.remaining
        } else {
            self.data.len() / 4
        };
        (len, Some(len))
    }
}

impl ExactSizeIterator for SampleSizeEntries<'_> {}

/// A borrowing view over raw SampleSizeBox bytes.
#[derive(Clone, Copy)]
pub struct SampleSizeBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SampleSizeBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 8)?;

        let view = Self { data, fullbox_offset };

        // If sample_size is 0, we need a table
        if view.sample_size() == 0 {
            let sample_count = view.sample_count();
            let entries_start = fullbox_offset + 12;
            validate_entry_count(data, entries_start, sample_count, 4)?;
        }

        Ok(view)
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

impl SampleSizeBox for SampleSizeBoxView<'_> {
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

    fn sample_size(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn sample_count(&self) -> u32 {
        let o = self.payload_offset() + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn entry(&self, index: usize) -> Option<u32> {
        if index >= self.sample_count() as usize {
            return None;
        }
        let default_size = self.sample_size();
        if default_size != 0 {
            return Some(default_size);
        }
        let o = self.payload_offset() + 8 + index * 4;
        Some(BigEndian::read_u32(&self.data[o..o + 4]))
    }

    fn entries(&self) -> impl Iterator<Item = u32> + '_ {
        let constant_size = self.sample_size();
        let sample_count = self.sample_count() as usize;
        if constant_size != 0 {
            SampleSizeEntries {
                data: &[],
                constant_size,
                remaining: sample_count,
            }
        } else {
            let start = self.payload_offset() + 8;
            SampleSizeEntries {
                data: &self.data[start..start + sample_count * 4],
                constant_size: 0,
                remaining: 0,
            }
        }
    }
}

impl std::fmt::Debug for SampleSizeBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleSizeBoxView")
            .field("box_size", &self.box_size())
            .field("sample_size", &self.sample_size())
            .field("sample_count", &self.sample_count())
            .finish()
    }
}

/// Sample size representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleSizes {
    /// All samples have the same size.
    Constant {
        /// The size of each sample.
        size: u32,
        /// The number of samples.
        count: u32,
    },
    /// Each sample has an individual size.
    Variable(Vec<u32>),
}

impl SampleSizes {
    /// Returns the number of samples.
    pub fn count(&self) -> u32 {
        match self {
            SampleSizes::Constant { count, .. } => *count,
            SampleSizes::Variable(entries) => entries.len() as u32,
        }
    }

    /// Returns the size of the sample at the given index,
    /// or `None` if `index >= self.count()`.
    pub fn get(&self, index: usize) -> Option<u32> {
        match self {
            SampleSizes::Constant { size, count } => {
                if index >= *count as usize {
                    return None;
                }
                Some(*size)
            }
            SampleSizes::Variable(entries) => entries.get(index).copied(),
        }
    }

    /// Returns the constant size if all samples have the same size, or 0 if variable.
    pub fn constant_size(&self) -> u32 {
        match self {
            SampleSizes::Constant { size, .. } => *size,
            SampleSizes::Variable(_) => 0,
        }
    }
}

/// An owned representation of SampleSizeBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleSizeBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Sample sizes (either constant or variable).
    pub sizes: SampleSizes,
}

impl SampleSizeBoxOwned {
    /// Creates a new empty SampleSizeBoxOwned with no samples.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a SampleSizeBoxOwned with a constant sample size.
    pub fn with_constant_size(size: u32, count: u32) -> Self {
        Self {
            flags: 0,
            sizes: SampleSizes::Constant { size, count },
        }
    }

    /// Creates a SampleSizeBoxOwned with variable sample sizes.
    pub fn with_variable_sizes(entries: Vec<u32>) -> Self {
        Self {
            flags: 0,
            sizes: SampleSizes::Variable(entries),
        }
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
let table_size = match &self.sizes {
            SampleSizes::Constant { .. } => 0,
            SampleSizes::Variable(entries) => entries.len() as u64 * 4,
        };
        fullbox_header_size_for_payload(8 + table_size) + 8 + table_size
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.sizes.constant_size())?;
        writer.write_u32::<BigEndian>(self.sizes.count())?;

        if let SampleSizes::Variable(entries) = &self.sizes {
            for &entry_size in entries {
                writer.write_u32::<BigEndian>(entry_size)?;
            }
        }

        Ok(())
    }
}

impl Default for SampleSizeBoxOwned {
    fn default() -> Self {
        Self {
            flags: 0,
            sizes: SampleSizes::Variable(Vec::new()),
        }
    }
}

impl SampleSizeBox for SampleSizeBoxOwned {
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

    fn sample_size(&self) -> u32 {
        self.sizes.constant_size()
    }

    fn sample_count(&self) -> u32 {
        self.sizes.count()
    }

    fn entry(&self, index: usize) -> Option<u32> {
        self.sizes.get(index)
    }

    fn entries(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.sample_count() as usize).filter_map(|i| self.entry(i))
    }
}

impl<T: SampleSizeBox> From<&T> for SampleSizeBoxOwned {
    fn from(source: &T) -> Self {
        let sample_size = source.sample_size();
        let sizes = if sample_size != 0 {
            SampleSizes::Constant {
                size: sample_size,
                count: source.sample_count(),
            }
        } else {
            SampleSizes::Variable(source.entries().collect())
        };
        Self {
            flags: source.flags(),
            sizes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stsz_variable(sizes: &[u32]) -> Vec<u8> {
        let size = 8 + 4 + 8 + sizes.len() * 4;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"stsz");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&0u32.to_be_bytes()); // sample_size = 0 (variable)
        data.extend_from_slice(&(sizes.len() as u32).to_be_bytes());
        for &s in sizes {
            data.extend_from_slice(&s.to_be_bytes());
        }
        data
    }

    fn make_stsz_constant(size: u32, count: u32) -> Vec<u8> {
        let total = 8 + 4 + 8;
        let mut data = Vec::with_capacity(total);
        data.extend_from_slice(&(total as u32).to_be_bytes());
        data.extend_from_slice(b"stsz");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&size.to_be_bytes());
        data.extend_from_slice(&count.to_be_bytes());
        data
    }

    #[test]
    fn parse_variable_sizes() {
        let sizes = vec![100, 200, 150];
        let data = make_stsz_variable(&sizes);
        let view = SampleSizeBoxView::new(&data).unwrap();

        assert_eq!(view.sample_size(), 0);
        assert_eq!(view.sample_count(), 3);
        assert_eq!(view.entry(0), Some(100));
        assert_eq!(view.entry(1), Some(200));
        assert_eq!(view.entry(2), Some(150));
        assert_eq!(view.entry(3), None);
    }

    #[test]
    fn parse_constant_size() {
        let data = make_stsz_constant(1024, 100);
        let view = SampleSizeBoxView::new(&data).unwrap();

        assert_eq!(view.sample_size(), 1024);
        assert_eq!(view.sample_count(), 100);
        assert_eq!(view.entry(0), Some(1024));
        assert_eq!(view.entry(99), Some(1024));
        assert_eq!(view.entry(100), None);
    }

    #[test]
    fn roundtrip_variable() {
        let sizes = vec![100, 200, 150];
        let data = make_stsz_variable(&sizes);
        let view = SampleSizeBoxView::new(&data).unwrap();
        let owned = SampleSizeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_constant() {
        let data = make_stsz_constant(1024, 100);
        let view = SampleSizeBoxView::new(&data).unwrap();
        let owned = SampleSizeBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn construct_with_constant_size() {
        let owned = SampleSizeBoxOwned::with_constant_size(512, 50);
        assert_eq!(owned.sample_size(), 512);
        assert_eq!(owned.sample_count(), 50);
        assert_eq!(owned.entry(0), Some(512));
        assert_eq!(owned.entry(49), Some(512));
        assert_eq!(owned.entry(50), None);
    }

    #[test]
    fn construct_with_variable_sizes() {
        let owned = SampleSizeBoxOwned::with_variable_sizes(vec![100, 200, 300]);
        assert_eq!(owned.sample_size(), 0);
        assert_eq!(owned.sample_count(), 3);
        assert_eq!(owned.entry(0), Some(100));
        assert_eq!(owned.entry(1), Some(200));
        assert_eq!(owned.entry(2), Some(300));
        assert_eq!(owned.entry(3), None);
    }
}
