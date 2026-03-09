//! Sample Auxiliary Information Sizes Box (saiz) parsing and serialization.
//!
//! The Sample Auxiliary Information Sizes Box contains the sample auxiliary
//! information sizes for each sample.
//!
//! ```text
//! aligned(8) class SampleAuxiliaryInformationSizesBox
//!    extends FullBox('saiz', version = 0, flags) {
//!    if (flags & 1) {
//!       unsigned int(32) aux_info_type;
//!       unsigned int(32) aux_info_type_parameter;
//!    }
//!    unsigned int(8) default_sample_info_size;
//!    unsigned int(32) sample_count;
//!    if (default_sample_info_size == 0) {
//!       unsigned int(8) sample_info_size[ sample_count ];
//!    }
//! }
//! ```

use crate::error::{ParseError, validate_entry_count};
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::{BoxCode, FourCC};
use std::io::{self, Write};

/// The box type identifier for SampleAuxiliaryInformationSizesBox.
pub const BOX_TYPE: BoxCode = BoxCode::SAIZ;

/// Common interface for accessing SampleAuxiliaryInformationSizesBox data.
pub trait SampleAuxiliaryInformationSizesBox {
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

    /// Returns the default sample info size.
    fn default_sample_info_size(&self) -> u8;

    /// Returns the sample count.
    fn sample_count(&self) -> u32;

    /// Returns all sample info sizes.
    fn sample_info_sizes(&self) -> impl Iterator<Item = u8> + '_;
}

/// A borrowing view over raw SampleAuxiliaryInformationSizesBox bytes.
#[derive(Clone, Copy)]
pub struct SampleAuxiliaryInformationSizesBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    fl: u32,
    default_sample_info_size: u8,
    sample_count: u32,
    sizes_offset: usize,
}

impl<'a> SampleAuxiliaryInformationSizesBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fl = header.flags;

        let mut offset_after_flags = 0;

        // If flags & 1, aux_info_type and aux_info_type_parameter are present
        if fl & 1 != 0 {
            offset_after_flags += 8;
        }

        let fullbox_offset = header.validate(data, BOX_TYPE, None, offset_after_flags + 5)?;

        let mut offset = fullbox_offset + 4 + offset_after_flags;
        let default_sample_info_size = data[offset];
        offset += 1;
        let sample_count = BigEndian::read_u32(&data[offset..offset + 4]);
        offset += 4;

        // When default_sample_info_size is 0, validate per-sample sizes are present
        if default_sample_info_size == 0 {
            validate_entry_count(data, offset, sample_count, 1)?;
        }

        Ok(Self {
            data,
            fullbox_offset,
            fl,
            default_sample_info_size,
            sample_count,
            sizes_offset: offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the size for a specific sample (if default_sample_info_size is 0).
    pub fn sample_info_size(&self, sample: usize) -> Option<u8> {
        if self.default_sample_info_size != 0 {
            return Some(self.default_sample_info_size);
        }

        if sample >= self.sample_count as usize {
            return None;
        }

        let offset = self.sizes_offset + sample;
        if offset < self.data.len() {
            Some(self.data[offset])
        } else {
            None
        }
    }

}

impl<'a> SampleAuxiliaryInformationSizesBox for SampleAuxiliaryInformationSizesBoxView<'a> {
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

    fn default_sample_info_size(&self) -> u8 {
        self.default_sample_info_size
    }

    fn sample_count(&self) -> u32 {
        self.sample_count
    }

    fn sample_info_sizes(&self) -> impl Iterator<Item = u8> + '_ {
        let default = self.default_sample_info_size;
        let count = self.sample_count as usize;
        if default != 0 {
            // Constant size: repeat it count times; slice is empty so
            // chain produces only the repeat part.
            [].iter()
                .copied()
                .chain(std::iter::repeat_n(default, count))
        } else {
            self.data[self.sizes_offset..self.sizes_offset + count]
                .iter()
                .copied()
                .chain(std::iter::repeat_n(0, 0))
        }
    }
}

impl std::fmt::Debug for SampleAuxiliaryInformationSizesBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleAuxiliaryInformationSizesBoxView")
            .field("default_sample_info_size", &self.default_sample_info_size())
            .field("sample_count", &self.sample_count())
            .finish()
    }
}

/// Sample info sizes: either a constant size for all samples, or per-sample sizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleInfoSizes {
    /// All samples have the same info size.
    Default {
        /// The info size for each sample.
        size: u8,
        /// The number of samples.
        count: u32,
    },
    /// Each sample has an individual info size.
    PerSample(Vec<u8>),
}

impl SampleInfoSizes {
    /// Returns the number of samples.
    pub fn count(&self) -> u32 {
        match self {
            SampleInfoSizes::Default { count, .. } => *count,
            SampleInfoSizes::PerSample(sizes) => sizes.len() as u32,
        }
    }

    /// Returns the default size (non-zero), or 0 if per-sample.
    pub fn default_size(&self) -> u8 {
        match self {
            SampleInfoSizes::Default { size, .. } => *size,
            SampleInfoSizes::PerSample(_) => 0,
        }
    }
}

impl Default for SampleInfoSizes {
    fn default() -> Self {
        SampleInfoSizes::PerSample(Vec::new())
    }
}

/// An owned representation of SampleAuxiliaryInformationSizesBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SampleAuxiliaryInformationSizesBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Auxiliary info type (if flags & 1).
    pub aux_info_type: Option<FourCC>,
    /// Auxiliary info type parameter (if flags & 1).
    pub aux_info_type_parameter: Option<u32>,
    /// Sample info sizes.
    pub sizes: SampleInfoSizes,
}

impl SampleAuxiliaryInformationSizesBoxOwned {
    /// Creates a new SampleAuxiliaryInformationSizesBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = (1 + 4) as u64; // default_size + sample_count
        if self.flags & 1 != 0 {
            payload += 8; // aux_info_type + aux_info_type_parameter
        }
        if let SampleInfoSizes::PerSample(ref sizes) = self.sizes {
            payload += sizes.len() as u64;
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;

        if self.flags & 1 != 0 {
            writer.write_all(&self.aux_info_type.unwrap_or(FourCC([0; 4])).0)?;
            writer.write_u32::<BigEndian>(self.aux_info_type_parameter.unwrap_or(0))?;
        }

        writer.write_u8(self.sizes.default_size())?;
        writer.write_u32::<BigEndian>(self.sizes.count())?;

        if let SampleInfoSizes::PerSample(ref sizes) = self.sizes {
            writer.write_all(sizes)?;
        }

        Ok(())
    }
}


impl SampleAuxiliaryInformationSizesBox for SampleAuxiliaryInformationSizesBoxOwned {
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

    fn aux_info_type(&self) -> Option<FourCC> {
        self.aux_info_type
    }

    fn aux_info_type_parameter(&self) -> Option<u32> {
        self.aux_info_type_parameter
    }

    fn default_sample_info_size(&self) -> u8 {
        self.sizes.default_size()
    }

    fn sample_count(&self) -> u32 {
        self.sizes.count()
    }

    fn sample_info_sizes(&self) -> impl Iterator<Item = u8> + '_ {
        let count = self.sizes.count() as usize;
        (0..count).map(move |i| match &self.sizes {
            SampleInfoSizes::Default { size, .. } => *size,
            SampleInfoSizes::PerSample(sizes) => sizes[i],
        })
    }
}

impl<T: SampleAuxiliaryInformationSizesBox> From<&T> for SampleAuxiliaryInformationSizesBoxOwned {
    fn from(source: &T) -> Self {
        let sizes = if source.default_sample_info_size() != 0 {
            SampleInfoSizes::Default {
                size: source.default_sample_info_size(),
                count: source.sample_count(),
            }
        } else {
            SampleInfoSizes::PerSample(source.sample_info_sizes().collect())
        };
        Self {
            flags: source.flags(),
            aux_info_type: source.aux_info_type(),
            aux_info_type_parameter: source.aux_info_type_parameter(),
            sizes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_saiz() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 1 + 4 + 2 = 19 bytes (no aux info type, default_size=0, 2 per-sample sizes)
        data.extend_from_slice(&19u32.to_be_bytes());
        data.extend_from_slice(b"saiz");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.push(0); // default_sample_info_size
        data.extend_from_slice(&2u32.to_be_bytes()); // sample_count
        data.push(16); // size[0]
        data.push(24); // size[1]
        data
    }

    #[test]
    fn parse_saiz() {
        let data = make_saiz();
        let view = SampleAuxiliaryInformationSizesBoxView::new(&data).unwrap();

        assert_eq!(view.default_sample_info_size(), 0);
        assert_eq!(view.sample_count(), 2);
        assert_eq!(view.sample_info_size(0), Some(16));
        assert_eq!(view.sample_info_size(1), Some(24));
    }

    #[test]
    fn roundtrip() {
        let data = make_saiz();
        let view = SampleAuxiliaryInformationSizesBoxView::new(&data).unwrap();
        let owned = SampleAuxiliaryInformationSizesBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
