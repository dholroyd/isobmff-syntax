//! SRTP Process Box (srpp) parsing and serialization.
//!
//! The SRTP Process Box contains SRTP processing information for hint tracks.
//!
//! ```text
//! aligned(8) class SRTPProcessBox extends FullBox('srpp', version, 0) {
//!    unsigned int(32) encryption_algorithm_rtp;
//!    unsigned int(32) encryption_algorithm_rtcp;
//!    unsigned int(32) integrity_algorithm_rtp;
//!    unsigned int(32) integrity_algorithm_rtcp;
//!    SchemeTypeBox scheme_type_box;
//!    SchemeInformationBox info;
//! }
//! ```

use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SRTPProcessBox.
pub const BOX_TYPE: BoxCode = BoxCode::SRPP;

/// A typed child of an SRTPProcessBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SRTPProcessChild {
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for SRTPProcessChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Other(o) => o.box_size(),
        }
    }
}

impl SRTPProcessChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for SRTPProcessChild {
    fn from(raw: RawBox<'_>) -> Self {
        Self::Other(OpaqueBoxOwned::from_raw_box(&raw))
    }
}

impl From<&SRTPProcessChild> for SRTPProcessChild {
    fn from(source: &SRTPProcessChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing SRTPProcessBox data.
pub trait SRTPProcessBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<SRTPProcessChild>
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

    /// Returns the RTP encryption algorithm.
    fn encryption_algorithm_rtp(&self) -> u32;

    /// Returns the RTCP encryption algorithm.
    fn encryption_algorithm_rtcp(&self) -> u32;

    /// Returns the RTP integrity algorithm.
    fn integrity_algorithm_rtp(&self) -> u32;

    /// Returns the RTCP integrity algorithm.
    fn integrity_algorithm_rtcp(&self) -> u32;

    /// Returns an iterator over child boxes (SchemeTypeBox + SchemeInformationBox).
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw SRTPProcessBox bytes.
#[derive(Clone, Copy)]
pub struct SRTPProcessBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    children_offset: usize,
}

impl<'a> SRTPProcessBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 16)?;
        let children_offset = fullbox_offset + 4 + 16; // version/flags + 4 algorithm fields
        Ok(Self {
            data,
            fullbox_offset,
            children_offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.children_offset..])
    }
}

impl<'a> SRTPProcessBox for SRTPProcessBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

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

    fn encryption_algorithm_rtp(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn encryption_algorithm_rtcp(&self) -> u32 {
        let o = self.fullbox_offset + 8;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn integrity_algorithm_rtp(&self) -> u32 {
        let o = self.fullbox_offset + 12;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn integrity_algorithm_rtcp(&self) -> u32 {
        let o = self.fullbox_offset + 16;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for SRTPProcessBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SRTPProcessBoxView")
            .field("encryption_algorithm_rtp", &self.encryption_algorithm_rtp())
            .field("encryption_algorithm_rtcp", &self.encryption_algorithm_rtcp())
            .finish()
    }
}

/// An owned representation of SRTPProcessBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SRTPProcessBoxOwned {
    /// Flags.
    pub flags: u32,
    /// RTP encryption algorithm.
    pub encryption_algorithm_rtp: u32,
    /// RTCP encryption algorithm.
    pub encryption_algorithm_rtcp: u32,
    /// RTP integrity algorithm.
    pub integrity_algorithm_rtp: u32,
    /// RTCP integrity algorithm.
    pub integrity_algorithm_rtcp: u32,
    /// Child boxes (SchemeTypeBox + SchemeInformationBox).
    pub children: Vec<SRTPProcessChild>,
}

impl SRTPProcessBoxOwned {
    /// Creates a new SRTPProcessBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let children_size: u64 = self.children.iter().map(|c| c.box_size()).sum();
        let payload = 16 + children_size; // 4 algorithm fields + children
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.encryption_algorithm_rtp)?;
        writer.write_u32::<BigEndian>(self.encryption_algorithm_rtcp)?;
        writer.write_u32::<BigEndian>(self.integrity_algorithm_rtp)?;
        writer.write_u32::<BigEndian>(self.integrity_algorithm_rtcp)?;
        for child in &self.children {
            child.write_to(writer)?;
        }
        Ok(())
    }
}

impl SRTPProcessBox for SRTPProcessBoxOwned {
    type Child<'a> = &'a SRTPProcessChild;

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

    fn encryption_algorithm_rtp(&self) -> u32 {
        self.encryption_algorithm_rtp
    }

    fn encryption_algorithm_rtcp(&self) -> u32 {
        self.encryption_algorithm_rtcp
    }

    fn integrity_algorithm_rtp(&self) -> u32 {
        self.integrity_algorithm_rtp
    }

    fn integrity_algorithm_rtcp(&self) -> u32 {
        self.integrity_algorithm_rtcp
    }

    fn children(&self) -> impl Iterator<Item = &SRTPProcessChild> {
        self.children.iter()
    }
}

impl<T: SRTPProcessBox> From<&T> for SRTPProcessBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            encryption_algorithm_rtp: source.encryption_algorithm_rtp(),
            encryption_algorithm_rtcp: source.encryption_algorithm_rtcp(),
            integrity_algorithm_rtp: source.integrity_algorithm_rtp(),
            integrity_algorithm_rtcp: source.integrity_algorithm_rtcp(),
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_srpp() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&28u32.to_be_bytes()); // 8 + 4 + 16
        data.extend_from_slice(b"srpp");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u32.to_be_bytes()); // encryption_algorithm_rtp
        data.extend_from_slice(&0u32.to_be_bytes()); // encryption_algorithm_rtcp
        data.extend_from_slice(&0u32.to_be_bytes()); // integrity_algorithm_rtp
        data.extend_from_slice(&0u32.to_be_bytes()); // integrity_algorithm_rtcp
        data
    }

    #[test]
    fn parse_srpp() {
        let data = make_srpp();
        let view = SRTPProcessBoxView::new(&data).unwrap();
        assert_eq!(view.encryption_algorithm_rtp(), 0);
        assert_eq!(view.encryption_algorithm_rtcp(), 0);
        assert_eq!(view.integrity_algorithm_rtp(), 0);
        assert_eq!(view.integrity_algorithm_rtcp(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_srpp();
        let view = SRTPProcessBoxView::new(&data).unwrap();
        let owned = SRTPProcessBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
