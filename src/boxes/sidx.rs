//! Segment Index Box (sidx) parsing and serialization.
//!
//! The Segment Index Box provides a compact index of one media stream within the media segment.
//!
//! ```text
//! aligned(8) class SegmentIndexBox extends FullBox('sidx', version, 0) {
//!    unsigned int(32) reference_ID;
//!    unsigned int(32) timescale;
//!    if (version==0) {
//!       unsigned int(32) earliest_presentation_time;
//!       unsigned int(32) first_offset;
//!    } else {
//!       unsigned int(64) earliest_presentation_time;
//!       unsigned int(64) first_offset;
//!    }
//!    unsigned int(16) reserved = 0;
//!    unsigned int(16) reference_count;
//!    for(i=1; i <= reference_count; i++) {
//!       bit(1) reference_type;
//!       unsigned int(31) referenced_size;
//!       unsigned int(32) subsegment_duration;
//!       bit(1) starts_with_SAP;
//!       unsigned int(3) SAP_type;
//!       unsigned int(28) SAP_delta_time;
//!    }
//! }
//! ```

use crate::entries::FixedSizeEntries;
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SegmentIndexBox.
pub const BOX_TYPE: BoxCode = BoxCode::SIDX;

/// A single reference entry in a segment index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentIndexReference {
    /// Reference type (0 = media, 1 = index).
    pub reference_type: bool,
    /// Referenced size in bytes.
    pub referenced_size: u32,
    /// Subsegment duration in timescale units.
    pub subsegment_duration: u32,
    /// Starts with SAP (Stream Access Point).
    pub starts_with_sap: bool,
    /// SAP type (1-6).
    pub sap_type: u8,
    /// SAP delta time.
    pub sap_delta_time: u32,
}

impl SegmentIndexReference {
    /// Parses a reference from 12 bytes.
    #[inline]
    fn from_bytes(data: &[u8; 12]) -> Self {
        let first = BigEndian::read_u32(&data[0..4]);
        let reference_type = (first >> 31) != 0;
        let referenced_size = first & 0x7FFFFFFF;

        let subsegment_duration = BigEndian::read_u32(&data[4..8]);

        let third = BigEndian::read_u32(&data[8..12]);
        let starts_with_sap = (third >> 31) != 0;
        let sap_type = ((third >> 28) & 0x7) as u8;
        let sap_delta_time = third & 0x0FFFFFFF;

        Self {
            reference_type,
            referenced_size,
            subsegment_duration,
            starts_with_sap,
            sap_type,
            sap_delta_time,
        }
    }

    /// Writes the reference as 12 bytes.
    fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let first = ((self.reference_type as u32) << 31) | (self.referenced_size & 0x7FFFFFFF);
        writer.write_u32::<BigEndian>(first)?;
        writer.write_u32::<BigEndian>(self.subsegment_duration)?;

        let third = ((self.starts_with_sap as u32) << 31)
            | ((self.sap_type as u32 & 0x7) << 28)
            | (self.sap_delta_time & 0x0FFFFFFF);
        writer.write_u32::<BigEndian>(third)?;

        Ok(())
    }
}

/// Common interface for accessing SegmentIndexBox data.
pub trait SegmentIndexBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the reference ID.
    fn reference_id(&self) -> u32;

    /// Returns the timescale.
    fn timescale(&self) -> u32;

    /// Returns the earliest presentation time.
    fn earliest_presentation_time(&self) -> u64;

    /// Returns the first offset.
    fn first_offset(&self) -> u64;

    /// Returns the reference count.
    fn reference_count(&self) -> u16;

    /// Returns an iterator over all references.
    fn references(&self) -> impl Iterator<Item = SegmentIndexReference> + '_;
}

/// A borrowing view over raw SegmentIndexBox bytes.
#[derive(Clone, Copy)]
pub struct SegmentIndexBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    references: FixedSizeEntries<'a, 12>,
}

impl<'a> SegmentIndexBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let header_payload_size = if version == 1 { 28 } else { 20 };
        let fullbox_offset = header.validate(data, BOX_TYPE, None, header_payload_size)?;

        let ref_count_offset = fullbox_offset + 4 + header_payload_size - 2;
        let reference_count = BigEndian::read_u16(&data[ref_count_offset..ref_count_offset + 2]);
        let references_offset = fullbox_offset + 4 + header_payload_size;
        let references = FixedSizeEntries::new(&data[references_offset..], reference_count as u32)?;

        Ok(Self {
            data,
            fullbox_offset,
            version,
            references,
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

    /// Returns the reference at the given index.
    pub fn reference(&self, index: usize) -> Option<SegmentIndexReference> {
        self.references.get(index).map(SegmentIndexReference::from_bytes)
    }

}

impl SegmentIndexBox for SegmentIndexBoxView<'_> {
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
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn reference_id(&self) -> u32 {
        let o = self.payload_offset();
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn timescale(&self) -> u32 {
        let o = self.payload_offset() + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn earliest_presentation_time(&self) -> u64 {
        let o = self.payload_offset() + 8;
        if self.version == 1 {
            BigEndian::read_u64(&self.data[o..o + 8])
        } else {
            BigEndian::read_u32(&self.data[o..o + 4]) as u64
        }
    }

    fn first_offset(&self) -> u64 {
        let o = self.payload_offset() + if self.version == 1 { 16 } else { 12 };
        if self.version == 1 {
            BigEndian::read_u64(&self.data[o..o + 8])
        } else {
            BigEndian::read_u32(&self.data[o..o + 4]) as u64
        }
    }

    fn reference_count(&self) -> u16 {
        self.references.count() as u16
    }

    fn references(&self) -> impl Iterator<Item = SegmentIndexReference> + '_ {
        self.references.iter().map(SegmentIndexReference::from_bytes)
    }
}

impl std::fmt::Debug for SegmentIndexBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentIndexBoxView")
            .field("version", &self.version())
            .field("reference_id", &self.reference_id())
            .field("timescale", &self.timescale())
            .field("reference_count", &self.reference_count())
            .finish()
    }
}

/// An owned representation of SegmentIndexBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentIndexBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Reference ID (typically track ID).
    pub reference_id: u32,
    /// Timescale.
    pub timescale: u32,
    /// Earliest presentation time.
    pub earliest_presentation_time: u64,
    /// First offset after this sidx box.
    pub first_offset: u64,
    /// References.
    pub references: Vec<SegmentIndexReference>,
}

impl SegmentIndexBoxOwned {
    /// Creates a new SegmentIndexBoxOwned.
    pub fn new(reference_id: u32, timescale: u32) -> Self {
        Self {
            flags: 0,
            reference_id,
            timescale,
            earliest_presentation_time: 0,
            first_offset: 0,
            references: Vec::new(),
        }
    }

    /// Returns whether version 1 is required.
    fn requires_v1(&self) -> bool {
        self.earliest_presentation_time > u32::MAX as u64 || self.first_offset > u32::MAX as u64
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let version_fields = if self.requires_v1() { 16u64 } else { 8u64 };
        let payload = 4 + 4 + version_fields + 2 + 2 + (self.references.len() as u64 * 12);
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let v1 = self.requires_v1();
        let size = self.serialized_size();
        let version = if v1 { 1u8 } else { 0u8 };
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;
        writer.write_u32::<BigEndian>(self.reference_id)?;
        writer.write_u32::<BigEndian>(self.timescale)?;

        if v1 {
            writer.write_u64::<BigEndian>(self.earliest_presentation_time)?;
            writer.write_u64::<BigEndian>(self.first_offset)?;
        } else {
            writer.write_u32::<BigEndian>(self.earliest_presentation_time as u32)?;
            writer.write_u32::<BigEndian>(self.first_offset as u32)?;
        }

        writer.write_u16::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.references.len() as u16)?;

        for reference in &self.references {
            reference.write_to(writer)?;
        }

        Ok(())
    }
}

impl Default for SegmentIndexBoxOwned {
    fn default() -> Self {
        Self::new(1, 90000)
    }
}

impl SegmentIndexBox for SegmentIndexBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        if self.requires_v1() { 1 } else { 0 }
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn reference_id(&self) -> u32 {
        self.reference_id
    }

    fn timescale(&self) -> u32 {
        self.timescale
    }

    fn earliest_presentation_time(&self) -> u64 {
        self.earliest_presentation_time
    }

    fn first_offset(&self) -> u64 {
        self.first_offset
    }

    fn reference_count(&self) -> u16 {
        self.references.len() as u16
    }

    fn references(&self) -> impl Iterator<Item = SegmentIndexReference> + '_ {
        self.references.iter().copied()
    }
}

impl<T: SegmentIndexBox> From<&T> for SegmentIndexBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            reference_id: source.reference_id(),
            timescale: source.timescale(),
            earliest_presentation_time: source.earliest_presentation_time(),
            first_offset: source.first_offset(),
            references: source.references().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sidx_v0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&44u32.to_be_bytes()); // size = 8 + 4 + 20 + 12
        data.extend_from_slice(b"sidx");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // reference_id
        data.extend_from_slice(&90000u32.to_be_bytes()); // timescale
        data.extend_from_slice(&0u32.to_be_bytes()); // earliest_presentation_time
        data.extend_from_slice(&0u32.to_be_bytes()); // first_offset
        data.extend_from_slice(&0u16.to_be_bytes()); // reserved
        data.extend_from_slice(&1u16.to_be_bytes()); // reference_count

        // Reference: type=0, size=10000, duration=90000, starts_with_sap=1, sap_type=1, sap_delta=0
        let first = 10000u32; // reference_type=0, referenced_size=10000
        data.extend_from_slice(&first.to_be_bytes());
        data.extend_from_slice(&90000u32.to_be_bytes()); // subsegment_duration
        let third = 0x90000000u32; // starts_with_sap=1, sap_type=1, sap_delta_time=0
        data.extend_from_slice(&third.to_be_bytes());

        data
    }

    #[test]
    fn parse_sidx_v0() {
        let data = make_sidx_v0();
        let view = SegmentIndexBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.reference_id(), 1);
        assert_eq!(view.timescale(), 90000);
        assert_eq!(view.reference_count(), 1);

        let reference = view.reference(0).unwrap();
        assert!(!reference.reference_type);
        assert_eq!(reference.referenced_size, 10000);
        assert_eq!(reference.subsegment_duration, 90000);
        assert!(reference.starts_with_sap);
        assert_eq!(reference.sap_type, 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_sidx_v0();
        let view = SegmentIndexBoxView::new(&data).unwrap();
        let owned = SegmentIndexBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
