//! Subsegment Index Box (ssix) parsing and serialization.
//!
//! The Subsegment Index Box provides information about subsegments within segments.
//!
//! ```text
//! aligned(8) class SubsegmentIndexBox extends FullBox('ssix', 0, 0) {
//!    unsigned int(32) subsegment_count;
//!    for(i=1; i <= subsegment_count; i++) {
//!       unsigned int(32) range_count;
//!       for (j=1; j <= range_count; j++) {
//!          unsigned int(8) level;
//!          unsigned int(24) range_size;
//!       }
//!    }
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SubsegmentIndexBox.
pub const BOX_TYPE: BoxCode = BoxCode::SSIX;

/// A subsegment range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubsegmentRange {
    /// Level of this range.
    pub level: u8,
    /// Size in bytes of this range.
    pub range_size: u32,
}

/// Shared interface over a single ssix subsegment entry.
///
/// Implemented by both the borrowing [`SubsegmentEntryView`] and by
/// `&SubsegmentEntryOwned`.
pub trait SubsegmentEntry {
    /// Returns the number of ranges in this subsegment.
    fn range_count(&self) -> usize;

    /// Returns an iterator over the ranges.
    fn ranges(&self) -> impl Iterator<Item = SubsegmentRange> + '_;

    /// Materialises an owned copy of this entry.
    fn to_owned(&self) -> SubsegmentEntryOwned {
        SubsegmentEntryOwned {
            ranges: self.ranges().collect(),
        }
    }
}

/// A borrowing view over a single ssix subsegment entry's range bytes.
#[derive(Clone, Copy)]
pub struct SubsegmentEntryView<'a> {
    ranges_data: &'a [u8],
    range_count: usize,
}

impl<'a> SubsegmentEntry for SubsegmentEntryView<'a> {
    fn range_count(&self) -> usize {
        self.range_count
    }

    fn ranges(&self) -> impl Iterator<Item = SubsegmentRange> + '_ {
        self.ranges_data.chunks_exact(4).map(|chunk| {
            let val = BigEndian::read_u32(chunk);
            SubsegmentRange {
                level: (val >> 24) as u8,
                range_size: val & 0x00FFFFFF,
            }
        })
    }
}

/// An owned ssix subsegment entry.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SubsegmentEntryOwned {
    /// Ranges within this subsegment.
    pub ranges: Vec<SubsegmentRange>,
}

impl SubsegmentEntry for &SubsegmentEntryOwned {
    fn range_count(&self) -> usize {
        self.ranges.len()
    }

    fn ranges(&self) -> impl Iterator<Item = SubsegmentRange> + '_ {
        self.ranges.iter().copied()
    }

    fn to_owned(&self) -> SubsegmentEntryOwned {
        (*self).clone()
    }
}

/// Common interface for accessing SubsegmentIndexBox data.
pub trait SubsegmentIndexBox {
    /// The concrete entry type yielded by [`Self::entries`].
    type Entry<'a>: SubsegmentEntry
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the subsegment count.
    fn subsegment_count(&self) -> u32;

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_;
}

/// A borrowing view over raw SubsegmentIndexBox bytes.
#[derive(Clone, Copy)]
pub struct SubsegmentIndexBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    subsegment_count: u32,
}

impl<'a> SubsegmentIndexBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;
        let subsegment_count = BigEndian::read_u32(&data[fullbox_offset + 4..fullbox_offset + 8]);
        Ok(Self { data, fullbox_offset, subsegment_count })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

}

impl<'a> SubsegmentIndexBox for SubsegmentIndexBoxView<'a> {
    type Entry<'b> = SubsegmentEntryView<'b> where Self: 'b;

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

    fn subsegment_count(&self) -> u32 {
        self.subsegment_count
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        let mut offset = self.fullbox_offset + 8;
        let mut remaining = self.subsegment_count;
        let mut errored = false;

        std::iter::from_fn(move || {
            if errored || remaining == 0 {
                return None;
            }
            remaining -= 1;

            if offset + 4 > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + 4,
                    found: self.data.len(),
                }));
            }

            let range_count = BigEndian::read_u32(&self.data[offset..offset + 4]) as usize;
            offset += 4;

            let ranges_total = match range_count.checked_mul(4) {
                Some(v) => v,
                None => {
                    errored = true;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: usize::MAX,
                        found: self.data.len(),
                    }));
                }
            };
            if offset + ranges_total > self.data.len() {
                errored = true;
                return Some(Err(ParseError::BufferTooShort {
                    expected: offset + ranges_total,
                    found: self.data.len(),
                }));
            }

            let ranges_data = &self.data[offset..offset + ranges_total];
            offset += ranges_total;

            Some(Ok(SubsegmentEntryView {
                ranges_data,
                range_count,
            }))
        })
    }
}

impl std::fmt::Debug for SubsegmentIndexBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubsegmentIndexBoxView")
            .field("subsegment_count", &self.subsegment_count())
            .finish()
    }
}

/// An owned representation of SubsegmentIndexBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SubsegmentIndexBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Subsegment entries.
    pub entries: Vec<SubsegmentEntryOwned>,
}

impl SubsegmentIndexBoxOwned {
    /// Creates a new SubsegmentIndexBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let mut payload = 4u64; // subsegment_count
        for entry in &self.entries {
            payload += 4 + entry.ranges.len() as u64 * 4; // ranges_count + ranges
        }
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            writer.write_u32::<BigEndian>(entry.ranges.len() as u32)?;
            for range in &entry.ranges {
                let val = ((range.level as u32) << 24) | (range.range_size & 0x00FFFFFF);
                writer.write_u32::<BigEndian>(val)?;
            }
        }

        Ok(())
    }
}


impl SubsegmentIndexBox for SubsegmentIndexBoxOwned {
    type Entry<'a> = &'a SubsegmentEntryOwned where Self: 'a;

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

    fn subsegment_count(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> impl Iterator<Item = Result<Self::Entry<'_>, ParseError>> + '_ {
        self.entries.iter().map(Ok)
    }
}

impl TryFrom<&SubsegmentIndexBoxView<'_>> for SubsegmentIndexBoxOwned {
    type Error = ParseError;

    fn try_from(source: &SubsegmentIndexBoxView<'_>) -> Result<Self, Self::Error> {
        let entries = SubsegmentIndexBox::entries(source)
            .map(|res| res.map(|v| SubsegmentEntry::to_owned(&v)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            flags: source.flags(),
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ssix() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 4 + 4 = 24 bytes
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"ssix");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // subsegment_count
        data.extend_from_slice(&1u32.to_be_bytes()); // ranges_count
        // level=1, range_size=1000 -> 0x010003E8
        data.extend_from_slice(&0x010003E8u32.to_be_bytes());
        data
    }

    #[test]
    fn parse_ssix() {
        let data = make_ssix();
        let view = SubsegmentIndexBoxView::new(&data).unwrap();

        assert_eq!(view.subsegment_count(), 1);
        let entries: Vec<_> =
            SubsegmentIndexBox::entries(&view).collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].range_count(), 1);
        let ranges: Vec<_> = entries[0].ranges().collect();
        assert_eq!(ranges[0].level, 1);
        assert_eq!(ranges[0].range_size, 1000);
    }

    #[test]
    fn roundtrip() {
        let data = make_ssix();
        let view = SubsegmentIndexBoxView::new(&data).unwrap();
        let owned = SubsegmentIndexBoxOwned::try_from(&view).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
