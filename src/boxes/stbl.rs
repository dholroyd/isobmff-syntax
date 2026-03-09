//! Sample Table Box (stbl) parsing and serialization.
//!
//! The Sample Table Box contains all the time and data indexing of the media samples.
//!
//! ```text
//! aligned(8) class SampleTableBox extends Box('stbl') {
//! }
//! ```

use crate::boxes::co64::{self, ChunkLargeOffsetBox as _, ChunkLargeOffsetBoxOwned, ChunkLargeOffsetBoxView};
use crate::boxes::cslg::{self, CompositionToDecodeBox as _, CompositionToDecodeBoxOwned, CompositionToDecodeBoxView};
use crate::boxes::ctts::{self, CompositionTimeToSampleBox as _, CompositionTimeToSampleBoxOwned, CompositionTimeToSampleBoxView};
use crate::boxes::padb::{self, PaddingBitsBox as _, PaddingBitsBoxOwned, PaddingBitsBoxView};
use crate::boxes::saio::{self, SampleAuxiliaryInformationOffsetsBox as _, SampleAuxiliaryInformationOffsetsBoxOwned, SampleAuxiliaryInformationOffsetsBoxView};
use crate::boxes::saiz::{self, SampleAuxiliaryInformationSizesBox as _, SampleAuxiliaryInformationSizesBoxOwned, SampleAuxiliaryInformationSizesBoxView};
use crate::boxes::sbgp::{self, SampleToGroupBox as _, SampleToGroupBoxOwned, SampleToGroupBoxView};
use crate::boxes::sdtp::{self, SampleDependencyTypeBox as _, SampleDependencyTypeBoxOwned, SampleDependencyTypeBoxView};
use crate::boxes::sgpd::{self, SampleGroupDescriptionBox as _, SampleGroupDescriptionBoxOwned, SampleGroupDescriptionBoxView};
use crate::boxes::stco::{self, ChunkOffsetBox as _, ChunkOffsetBoxOwned, ChunkOffsetBoxView};
use crate::boxes::stsc::{self, SampleToChunkBox as _, SampleToChunkBoxOwned, SampleToChunkBoxView};
use crate::boxes::stsd::{self, SampleDescriptionBox as _, SampleDescriptionBoxOwned, SampleDescriptionBoxView};
use crate::boxes::stdp::{self, DegradationPriorityBox as _, DegradationPriorityBoxOwned, DegradationPriorityBoxView};
use crate::boxes::stsh::{self, ShadowSyncSampleBox as _, ShadowSyncSampleBoxOwned, ShadowSyncSampleBoxView};
use crate::boxes::stss::{self, SyncSampleBox as _, SyncSampleBoxOwned, SyncSampleBoxView};
use crate::boxes::stsz::{self, SampleSizeBox as _, SampleSizeBoxOwned, SampleSizeBoxView};
use crate::boxes::stts::{self, TimeToSampleBox as _, TimeToSampleBoxOwned, TimeToSampleBoxView};
use crate::boxes::stz2::{self, CompactSampleSizeBox as _, CompactSampleSizeBoxOwned, CompactSampleSizeBoxView};
use crate::boxes::subs::{self, SubSampleInformationBox as _, SubSampleInformationBoxOwned, SubSampleInformationBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SampleTableBox.
pub const BOX_TYPE: BoxCode = BoxCode::STBL;

/// A typed child of a SampleTableBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleTableChild {
    /// A SampleDescriptionBox child.
    Stsd(SampleDescriptionBoxOwned),
    /// A TimeToSampleBox child.
    Stts(TimeToSampleBoxOwned),
    /// A CompositionTimeToSampleBox child.
    Ctts(CompositionTimeToSampleBoxOwned),
    /// A CompositionToDecodeBox child.
    Cslg(CompositionToDecodeBoxOwned),
    /// A SampleToChunkBox child.
    Stsc(SampleToChunkBoxOwned),
    /// A SampleSizeBox child.
    Stsz(SampleSizeBoxOwned),
    /// A CompactSampleSizeBox child.
    Stz2(CompactSampleSizeBoxOwned),
    /// A ChunkOffsetBox child.
    Stco(ChunkOffsetBoxOwned),
    /// A ChunkLargeOffsetBox child.
    Co64(ChunkLargeOffsetBoxOwned),
    /// A SyncSampleBox child.
    Stss(SyncSampleBoxOwned),
    /// A ShadowSyncSampleBox child.
    Stsh(ShadowSyncSampleBoxOwned),
    /// A PaddingBitsBox child.
    Padb(PaddingBitsBoxOwned),
    /// A DegradationPriorityBox child.
    Stdp(DegradationPriorityBoxOwned),
    /// A SampleDependencyTypeBox child.
    Sdtp(SampleDependencyTypeBoxOwned),
    /// A SampleToGroupBox child.
    Sbgp(SampleToGroupBoxOwned),
    /// A SampleGroupDescriptionBox child.
    Sgpd(SampleGroupDescriptionBoxOwned),
    /// A SubSampleInformationBox child.
    Subs(SubSampleInformationBoxOwned),
    /// A SampleAuxiliaryInformationSizesBox child.
    Saiz(SampleAuxiliaryInformationSizesBoxOwned),
    /// A SampleAuxiliaryInformationOffsetsBox child.
    Saio(SampleAuxiliaryInformationOffsetsBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for SampleTableChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Stsd(_) => stsd::BOX_TYPE,
            Self::Stts(_) => stts::BOX_TYPE,
            Self::Ctts(_) => ctts::BOX_TYPE,
            Self::Cslg(_) => cslg::BOX_TYPE,
            Self::Stsc(_) => stsc::BOX_TYPE,
            Self::Stsz(_) => stsz::BOX_TYPE,
            Self::Stz2(_) => stz2::BOX_TYPE,
            Self::Stco(_) => stco::BOX_TYPE,
            Self::Co64(_) => co64::BOX_TYPE,
            Self::Stss(_) => stss::BOX_TYPE,
            Self::Stsh(_) => stsh::BOX_TYPE,
            Self::Padb(_) => padb::BOX_TYPE,
            Self::Stdp(_) => stdp::BOX_TYPE,
            Self::Sdtp(_) => sdtp::BOX_TYPE,
            Self::Sbgp(_) => sbgp::BOX_TYPE,
            Self::Sgpd(_) => sgpd::BOX_TYPE,
            Self::Subs(_) => subs::BOX_TYPE,
            Self::Saiz(_) => saiz::BOX_TYPE,
            Self::Saio(_) => saio::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Stsd(b) => b.box_size(),
            Self::Stts(b) => b.box_size(),
            Self::Ctts(b) => b.box_size(),
            Self::Cslg(b) => b.box_size(),
            Self::Stsc(b) => b.box_size(),
            Self::Stsz(b) => b.box_size(),
            Self::Stz2(b) => b.box_size(),
            Self::Stco(b) => b.box_size(),
            Self::Co64(b) => b.box_size(),
            Self::Stss(b) => b.box_size(),
            Self::Stsh(b) => b.box_size(),
            Self::Padb(b) => b.box_size(),
            Self::Stdp(b) => b.box_size(),
            Self::Sdtp(b) => b.box_size(),
            Self::Sbgp(b) => b.box_size(),
            Self::Sgpd(b) => b.box_size(),
            Self::Subs(b) => b.box_size(),
            Self::Saiz(b) => b.box_size(),
            Self::Saio(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl SampleTableChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Stsd(b) => b.write_to(writer),
            Self::Stts(b) => b.write_to(writer),
            Self::Ctts(b) => b.write_to(writer),
            Self::Cslg(b) => b.write_to(writer),
            Self::Stsc(b) => b.write_to(writer),
            Self::Stsz(b) => b.write_to(writer),
            Self::Stz2(b) => b.write_to(writer),
            Self::Stco(b) => b.write_to(writer),
            Self::Co64(b) => b.write_to(writer),
            Self::Stss(b) => b.write_to(writer),
            Self::Stsh(b) => b.write_to(writer),
            Self::Padb(b) => b.write_to(writer),
            Self::Stdp(b) => b.write_to(writer),
            Self::Sdtp(b) => b.write_to(writer),
            Self::Sbgp(b) => b.write_to(writer),
            Self::Sgpd(b) => b.write_to(writer),
            Self::Subs(b) => b.write_to(writer),
            Self::Saiz(b) => b.write_to(writer),
            Self::Saio(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for SampleTableChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            stsd::BOX_TYPE => match SampleDescriptionBoxView::new(raw.data()) {
                Ok(v) => Self::Stsd(SampleDescriptionBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stts::BOX_TYPE => match TimeToSampleBoxView::new(raw.data()) {
                Ok(v) => Self::Stts(TimeToSampleBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            ctts::BOX_TYPE => match CompositionTimeToSampleBoxView::new(raw.data()) {
                Ok(v) => Self::Ctts(CompositionTimeToSampleBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            cslg::BOX_TYPE => match CompositionToDecodeBoxView::new(raw.data()) {
                Ok(v) => Self::Cslg(CompositionToDecodeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stsc::BOX_TYPE => match SampleToChunkBoxView::new(raw.data()) {
                Ok(v) => Self::Stsc(SampleToChunkBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stsz::BOX_TYPE => match SampleSizeBoxView::new(raw.data()) {
                Ok(v) => Self::Stsz(SampleSizeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stz2::BOX_TYPE => match CompactSampleSizeBoxView::new(raw.data()) {
                Ok(v) => Self::Stz2(CompactSampleSizeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stco::BOX_TYPE => match ChunkOffsetBoxView::new(raw.data()) {
                Ok(v) => Self::Stco(ChunkOffsetBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            co64::BOX_TYPE => match ChunkLargeOffsetBoxView::new(raw.data()) {
                Ok(v) => Self::Co64(ChunkLargeOffsetBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stss::BOX_TYPE => match SyncSampleBoxView::new(raw.data()) {
                Ok(v) => Self::Stss(SyncSampleBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stsh::BOX_TYPE => match ShadowSyncSampleBoxView::new(raw.data()) {
                Ok(v) => Self::Stsh(ShadowSyncSampleBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            padb::BOX_TYPE => match PaddingBitsBoxView::new(raw.data()) {
                Ok(v) => Self::Padb(PaddingBitsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stdp::BOX_TYPE => match DegradationPriorityBoxView::new(raw.data()) {
                Ok(v) => Self::Stdp(DegradationPriorityBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            sdtp::BOX_TYPE => match SampleDependencyTypeBoxView::new(raw.data()) {
                Ok(v) => Self::Sdtp(SampleDependencyTypeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            sbgp::BOX_TYPE => match SampleToGroupBoxView::new(raw.data()) {
                Ok(v) => Self::Sbgp(SampleToGroupBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            sgpd::BOX_TYPE => match SampleGroupDescriptionBoxView::new(raw.data()) {
                Ok(v) => Self::Sgpd(SampleGroupDescriptionBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            subs::BOX_TYPE => match SubSampleInformationBoxView::new(raw.data()) {
                Ok(v) => match SubSampleInformationBoxOwned::try_from(&v) {
                    Ok(owned) => Self::Subs(owned),
                    Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
                },
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            saiz::BOX_TYPE => match SampleAuxiliaryInformationSizesBoxView::new(raw.data()) {
                Ok(v) => Self::Saiz(SampleAuxiliaryInformationSizesBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            saio::BOX_TYPE => match SampleAuxiliaryInformationOffsetsBoxView::new(raw.data()) {
                Ok(v) => Self::Saio(SampleAuxiliaryInformationOffsetsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&SampleTableChild> for SampleTableChild {
    fn from(source: &SampleTableChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing SampleTableBox data.
pub trait SampleTableBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<SampleTableChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw SampleTableBox bytes.
#[derive(Clone, Copy)]
pub struct SampleTableBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> SampleTableBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 0)?;
        Ok(Self { data, header })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    #[inline]
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.header.header_size as usize..])
    }

    /// Returns the SampleDescriptionBox child, if present.
    pub fn stsd(&self) -> Option<SampleDescriptionBoxView<'a>> {
        self.children()
            .find_as::<SampleDescriptionBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the TimeToSampleBox child, if present.
    pub fn stts(&self) -> Option<TimeToSampleBoxView<'a>> {
        self.children()
            .find_as::<TimeToSampleBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the CompositionTimeToSampleBox child, if present.
    pub fn ctts(&self) -> Option<CompositionTimeToSampleBoxView<'a>> {
        self.children()
            .find_as::<CompositionTimeToSampleBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the CompositionToDecodeBox child, if present.
    pub fn cslg(&self) -> Option<CompositionToDecodeBoxView<'a>> {
        self.children()
            .find_as::<CompositionToDecodeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the SampleToChunkBox child, if present.
    pub fn stsc(&self) -> Option<SampleToChunkBoxView<'a>> {
        self.children()
            .find_as::<SampleToChunkBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the SampleSizeBox child, if present.
    pub fn stsz(&self) -> Option<SampleSizeBoxView<'a>> {
        self.children()
            .find_as::<SampleSizeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the CompactSampleSizeBox child, if present.
    pub fn stz2(&self) -> Option<CompactSampleSizeBoxView<'a>> {
        self.children()
            .find_as::<CompactSampleSizeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ChunkOffsetBox child, if present.
    pub fn stco(&self) -> Option<ChunkOffsetBoxView<'a>> {
        self.children()
            .find_as::<ChunkOffsetBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ChunkLargeOffsetBox child, if present.
    pub fn co64(&self) -> Option<ChunkLargeOffsetBoxView<'a>> {
        self.children()
            .find_as::<ChunkLargeOffsetBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the SyncSampleBox child, if present.
    pub fn stss(&self) -> Option<SyncSampleBoxView<'a>> {
        self.children()
            .find_as::<SyncSampleBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ShadowSyncSampleBox child, if present.
    pub fn stsh(&self) -> Option<ShadowSyncSampleBoxView<'a>> {
        self.children()
            .find_as::<ShadowSyncSampleBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the PaddingBitsBox child, if present.
    pub fn padb(&self) -> Option<PaddingBitsBoxView<'a>> {
        self.children()
            .find_as::<PaddingBitsBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the DegradationPriorityBox child, if present.
    pub fn stdp(&self) -> Option<DegradationPriorityBoxView<'a>> {
        self.children()
            .find_as::<DegradationPriorityBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the SampleDependencyTypeBox child, if present.
    pub fn sdtp(&self) -> Option<SampleDependencyTypeBoxView<'a>> {
        self.children()
            .find_as::<SampleDependencyTypeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns all SampleToGroupBox children.
    pub fn sbgp(&self) -> impl Iterator<Item = SampleToGroupBoxView<'a>> {
        self.children()
            .filter_as::<SampleToGroupBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns all SampleGroupDescriptionBox children.
    pub fn sgpd(&self) -> impl Iterator<Item = SampleGroupDescriptionBoxView<'a>> {
        self.children()
            .filter_as::<SampleGroupDescriptionBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the SubSampleInformationBox child, if present.
    pub fn subs(&self) -> Option<SubSampleInformationBoxView<'a>> {
        self.children()
            .find_as::<SubSampleInformationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns all SampleAuxiliaryInformationSizesBox children.
    pub fn saiz(&self) -> impl Iterator<Item = SampleAuxiliaryInformationSizesBoxView<'a>> {
        self.children()
            .filter_as::<SampleAuxiliaryInformationSizesBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns all SampleAuxiliaryInformationOffsetsBox children.
    pub fn saio(&self) -> impl Iterator<Item = SampleAuxiliaryInformationOffsetsBoxView<'a>> {
        self.children()
            .filter_as::<SampleAuxiliaryInformationOffsetsBoxView>()
            .filter_map(Result::ok)
    }
}

impl<'a> SampleTableBox for SampleTableBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

    fn box_size(&self) -> u64 {
        self.header.size
    }

    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for SampleTableBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self.children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("SampleTableBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of SampleTableBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SampleTableBoxOwned {
    /// Typed child boxes.
    pub children: Vec<SampleTableChild>,
}

impl SampleTableBoxOwned {
    /// Creates a new empty SampleTableBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = self.children.iter().map(|c| c.box_size()).sum::<u64>();
        header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_box_header(writer, size, BOX_TYPE)?;
        for child in &self.children {
            child.write_to(writer)?;
        }
        Ok(())
    }

    /// Returns the SampleDescriptionBox child, if present.
    pub fn stsd(&self) -> Option<&SampleDescriptionBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stsd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the TimeToSampleBox child, if present.
    pub fn stts(&self) -> Option<&TimeToSampleBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stts(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the CompositionTimeToSampleBox child, if present.
    pub fn ctts(&self) -> Option<&CompositionTimeToSampleBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Ctts(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the CompositionToDecodeBox child, if present.
    pub fn cslg(&self) -> Option<&CompositionToDecodeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Cslg(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SampleToChunkBox child, if present.
    pub fn stsc(&self) -> Option<&SampleToChunkBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stsc(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SampleSizeBox child, if present.
    pub fn stsz(&self) -> Option<&SampleSizeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stsz(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the CompactSampleSizeBox child, if present.
    pub fn stz2(&self) -> Option<&CompactSampleSizeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stz2(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ChunkOffsetBox child, if present.
    pub fn stco(&self) -> Option<&ChunkOffsetBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stco(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ChunkLargeOffsetBox child, if present.
    pub fn co64(&self) -> Option<&ChunkLargeOffsetBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Co64(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SyncSampleBox child, if present.
    pub fn stss(&self) -> Option<&SyncSampleBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stss(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ShadowSyncSampleBox child, if present.
    pub fn stsh(&self) -> Option<&ShadowSyncSampleBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stsh(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the PaddingBitsBox child, if present.
    pub fn padb(&self) -> Option<&PaddingBitsBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Padb(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the DegradationPriorityBox child, if present.
    pub fn stdp(&self) -> Option<&DegradationPriorityBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Stdp(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SampleDependencyTypeBox child, if present.
    pub fn sdtp(&self) -> Option<&SampleDependencyTypeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Sdtp(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleToGroupBox children.
    pub fn sbgp(&self) -> impl Iterator<Item = &SampleToGroupBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            SampleTableChild::Sbgp(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleGroupDescriptionBox children.
    pub fn sgpd(&self) -> impl Iterator<Item = &SampleGroupDescriptionBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            SampleTableChild::Sgpd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SubSampleInformationBox child, if present.
    pub fn subs(&self) -> Option<&SubSampleInformationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SampleTableChild::Subs(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleAuxiliaryInformationSizesBox children.
    pub fn saiz(&self) -> impl Iterator<Item = &SampleAuxiliaryInformationSizesBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            SampleTableChild::Saiz(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleAuxiliaryInformationOffsetsBox children.
    pub fn saio(&self) -> impl Iterator<Item = &SampleAuxiliaryInformationOffsetsBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            SampleTableChild::Saio(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a SampleDescriptionBox child.
    pub fn add_stsd(&mut self, b: SampleDescriptionBoxOwned) {
        self.children.push(SampleTableChild::Stsd(b));
    }

    /// Adds a TimeToSampleBox child.
    pub fn add_stts(&mut self, b: TimeToSampleBoxOwned) {
        self.children.push(SampleTableChild::Stts(b));
    }

    /// Adds a CompositionTimeToSampleBox child.
    pub fn add_ctts(&mut self, b: CompositionTimeToSampleBoxOwned) {
        self.children.push(SampleTableChild::Ctts(b));
    }

    /// Adds a CompositionToDecodeBox child.
    pub fn add_cslg(&mut self, b: CompositionToDecodeBoxOwned) {
        self.children.push(SampleTableChild::Cslg(b));
    }

    /// Adds a SampleToChunkBox child.
    pub fn add_stsc(&mut self, b: SampleToChunkBoxOwned) {
        self.children.push(SampleTableChild::Stsc(b));
    }

    /// Adds a SampleSizeBox child.
    pub fn add_stsz(&mut self, b: SampleSizeBoxOwned) {
        self.children.push(SampleTableChild::Stsz(b));
    }

    /// Adds a CompactSampleSizeBox child.
    pub fn add_stz2(&mut self, b: CompactSampleSizeBoxOwned) {
        self.children.push(SampleTableChild::Stz2(b));
    }

    /// Adds a ChunkOffsetBox child.
    pub fn add_stco(&mut self, b: ChunkOffsetBoxOwned) {
        self.children.push(SampleTableChild::Stco(b));
    }

    /// Adds a ChunkLargeOffsetBox child.
    pub fn add_co64(&mut self, b: ChunkLargeOffsetBoxOwned) {
        self.children.push(SampleTableChild::Co64(b));
    }

    /// Adds a SyncSampleBox child.
    pub fn add_stss(&mut self, b: SyncSampleBoxOwned) {
        self.children.push(SampleTableChild::Stss(b));
    }

    /// Adds a ShadowSyncSampleBox child.
    pub fn add_stsh(&mut self, b: ShadowSyncSampleBoxOwned) {
        self.children.push(SampleTableChild::Stsh(b));
    }

    /// Adds a PaddingBitsBox child.
    pub fn add_padb(&mut self, b: PaddingBitsBoxOwned) {
        self.children.push(SampleTableChild::Padb(b));
    }

    /// Adds a DegradationPriorityBox child.
    pub fn add_stdp(&mut self, b: DegradationPriorityBoxOwned) {
        self.children.push(SampleTableChild::Stdp(b));
    }

    /// Adds a SampleDependencyTypeBox child.
    pub fn add_sdtp(&mut self, b: SampleDependencyTypeBoxOwned) {
        self.children.push(SampleTableChild::Sdtp(b));
    }

    /// Adds a SampleToGroupBox child.
    pub fn add_sbgp(&mut self, b: SampleToGroupBoxOwned) {
        self.children.push(SampleTableChild::Sbgp(b));
    }

    /// Adds a SampleGroupDescriptionBox child.
    pub fn add_sgpd(&mut self, b: SampleGroupDescriptionBoxOwned) {
        self.children.push(SampleTableChild::Sgpd(b));
    }

    /// Adds a SubSampleInformationBox child.
    pub fn add_subs(&mut self, b: SubSampleInformationBoxOwned) {
        self.children.push(SampleTableChild::Subs(b));
    }

    /// Adds a SampleAuxiliaryInformationSizesBox child.
    pub fn add_saiz(&mut self, b: SampleAuxiliaryInformationSizesBoxOwned) {
        self.children.push(SampleTableChild::Saiz(b));
    }

    /// Adds a SampleAuxiliaryInformationOffsetsBox child.
    pub fn add_saio(&mut self, b: SampleAuxiliaryInformationOffsetsBoxOwned) {
        self.children.push(SampleTableChild::Saio(b));
    }
}

impl SampleTableBox for SampleTableBoxOwned {
    type Child<'a> = &'a SampleTableChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &SampleTableChild> {
        self.children.iter()
    }
}

impl<T: SampleTableBox> From<&T> for SampleTableBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_stbl() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"stbl");

        let view = SampleTableBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"stbl");
        data.extend_from_slice(&[0u8; 16]);

        let view = SampleTableBoxView::new(&data).unwrap();
        let owned = SampleTableBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
