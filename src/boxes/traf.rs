//! Track Fragment Box (traf) parsing and serialization.
//!
//! The Track Fragment Box contains fragment-specific information for a track.
//!
//! ```text
//! aligned(8) class TrackFragmentBox extends Box('traf') {
//! }
//! ```

use crate::boxes::saio::{self, SampleAuxiliaryInformationOffsetsBox as _, SampleAuxiliaryInformationOffsetsBoxOwned, SampleAuxiliaryInformationOffsetsBoxView};
use crate::boxes::saiz::{self, SampleAuxiliaryInformationSizesBox as _, SampleAuxiliaryInformationSizesBoxOwned, SampleAuxiliaryInformationSizesBoxView};
use crate::boxes::sbgp::{self, SampleToGroupBox as _, SampleToGroupBoxOwned, SampleToGroupBoxView};
use crate::boxes::sdtp::{self, SampleDependencyTypeBox as _, SampleDependencyTypeBoxOwned, SampleDependencyTypeBoxView};
use crate::boxes::senc::{self, SampleEncryptionBox as _, SampleEncryptionBoxOwned, SampleEncryptionBoxView};
use crate::boxes::sgpd::{self, SampleGroupDescriptionBox as _, SampleGroupDescriptionBoxOwned, SampleGroupDescriptionBoxView};
use crate::boxes::subs::{self, SubSampleInformationBox as _, SubSampleInformationBoxOwned, SubSampleInformationBoxView};
use crate::boxes::tfdt::{self, TrackFragmentBaseMediaDecodeTimeBox as _, TrackFragmentBaseMediaDecodeTimeBoxOwned, TrackFragmentBaseMediaDecodeTimeBoxView};
use crate::boxes::tfhd::{self, TrackFragmentHeaderBox as _, TrackFragmentHeaderBoxOwned, TrackFragmentHeaderBoxView};
use crate::boxes::trun::{self, TrackRunBox as _, TrackRunBoxOwned, TrackRunBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackFragmentBox.
pub const BOX_TYPE: BoxCode = BoxCode::TRAF;

/// A typed child of a TrackFragmentBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackFragmentChild {
    /// A TrackFragmentHeaderBox child.
    Tfhd(TrackFragmentHeaderBoxOwned),
    /// A TrackFragmentBaseMediaDecodeTimeBox child.
    Tfdt(TrackFragmentBaseMediaDecodeTimeBoxOwned),
    /// A TrackRunBox child.
    Trun(TrackRunBoxOwned),
    /// A SampleToGroupBox child.
    Sbgp(SampleToGroupBoxOwned),
    /// A SampleGroupDescriptionBox child.
    Sgpd(SampleGroupDescriptionBoxOwned),
    /// A SampleAuxiliaryInformationSizesBox child.
    Saiz(SampleAuxiliaryInformationSizesBoxOwned),
    /// A SampleAuxiliaryInformationOffsetsBox child.
    Saio(SampleAuxiliaryInformationOffsetsBoxOwned),
    /// A SampleDependencyTypeBox child.
    Sdtp(SampleDependencyTypeBoxOwned),
    /// A SubSampleInformationBox child.
    Subs(SubSampleInformationBoxOwned),
    /// A SampleEncryptionBox child.
    Senc(SampleEncryptionBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for TrackFragmentChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Tfhd(_) => tfhd::BOX_TYPE,
            Self::Tfdt(_) => tfdt::BOX_TYPE,
            Self::Trun(_) => trun::BOX_TYPE,
            Self::Sbgp(_) => sbgp::BOX_TYPE,
            Self::Sgpd(_) => sgpd::BOX_TYPE,
            Self::Saiz(_) => saiz::BOX_TYPE,
            Self::Saio(_) => saio::BOX_TYPE,
            Self::Sdtp(_) => sdtp::BOX_TYPE,
            Self::Subs(_) => subs::BOX_TYPE,
            Self::Senc(_) => senc::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Tfhd(b) => b.box_size(),
            Self::Tfdt(b) => b.box_size(),
            Self::Trun(b) => b.box_size(),
            Self::Sbgp(b) => b.box_size(),
            Self::Sgpd(b) => b.box_size(),
            Self::Saiz(b) => b.box_size(),
            Self::Saio(b) => b.box_size(),
            Self::Sdtp(b) => b.box_size(),
            Self::Subs(b) => b.box_size(),
            Self::Senc(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl TrackFragmentChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Tfhd(b) => b.write_to(writer),
            Self::Tfdt(b) => b.write_to(writer),
            Self::Trun(b) => b.write_to(writer),
            Self::Sbgp(b) => b.write_to(writer),
            Self::Sgpd(b) => b.write_to(writer),
            Self::Saiz(b) => b.write_to(writer),
            Self::Saio(b) => b.write_to(writer),
            Self::Sdtp(b) => b.write_to(writer),
            Self::Subs(b) => b.write_to(writer),
            Self::Senc(b) => b.write_to(writer, 0),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for TrackFragmentChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            tfhd::BOX_TYPE => match TrackFragmentHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Tfhd(TrackFragmentHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            tfdt::BOX_TYPE => match TrackFragmentBaseMediaDecodeTimeBoxView::new(raw.data()) {
                Ok(v) => Self::Tfdt(TrackFragmentBaseMediaDecodeTimeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            trun::BOX_TYPE => match TrackRunBoxView::new(raw.data()) {
                Ok(v) => Self::Trun(TrackRunBoxOwned::from(&v)),
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
            saiz::BOX_TYPE => match SampleAuxiliaryInformationSizesBoxView::new(raw.data()) {
                Ok(v) => Self::Saiz(SampleAuxiliaryInformationSizesBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            saio::BOX_TYPE => match SampleAuxiliaryInformationOffsetsBoxView::new(raw.data()) {
                Ok(v) => Self::Saio(SampleAuxiliaryInformationOffsetsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            sdtp::BOX_TYPE => match SampleDependencyTypeBoxView::new(raw.data()) {
                Ok(v) => Self::Sdtp(SampleDependencyTypeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            subs::BOX_TYPE => match SubSampleInformationBoxView::new(raw.data()) {
                Ok(v) => match SubSampleInformationBoxOwned::try_from(&v) {
                    Ok(owned) => Self::Subs(owned),
                    Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
                },
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            senc::BOX_TYPE => match SampleEncryptionBoxView::new(raw.data()) {
                Ok(v) => Self::Senc(SampleEncryptionBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&TrackFragmentChild> for TrackFragmentChild {
    fn from(source: &TrackFragmentChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing TrackFragmentBox data.
pub trait TrackFragmentBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<TrackFragmentChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw TrackFragmentBox bytes.
#[derive(Clone, Copy)]
pub struct TrackFragmentBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> TrackFragmentBoxView<'a> {
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

    /// Returns the TrackFragmentHeaderBox child, if present.
    pub fn tfhd(&self) -> Option<TrackFragmentHeaderBoxView<'a>> {
        self.children()
            .find_as::<TrackFragmentHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the TrackFragmentBaseMediaDecodeTimeBox child, if present.
    pub fn tfdt(&self) -> Option<TrackFragmentBaseMediaDecodeTimeBoxView<'a>> {
        self.children()
            .find_as::<TrackFragmentBaseMediaDecodeTimeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns an iterator over TrackRunBox children.
    pub fn trun(&self) -> impl Iterator<Item = TrackRunBoxView<'a>> {
        self.children()
            .filter_as::<TrackRunBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over SampleToGroupBox children.
    pub fn sbgp(&self) -> impl Iterator<Item = SampleToGroupBoxView<'a>> {
        self.children()
            .filter_as::<SampleToGroupBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over SampleGroupDescriptionBox children.
    pub fn sgpd(&self) -> impl Iterator<Item = SampleGroupDescriptionBoxView<'a>> {
        self.children()
            .filter_as::<SampleGroupDescriptionBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over SampleAuxiliaryInformationSizesBox children.
    pub fn saiz(&self) -> impl Iterator<Item = SampleAuxiliaryInformationSizesBoxView<'a>> {
        self.children()
            .filter_as::<SampleAuxiliaryInformationSizesBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over SampleAuxiliaryInformationOffsetsBox children.
    pub fn saio(&self) -> impl Iterator<Item = SampleAuxiliaryInformationOffsetsBoxView<'a>> {
        self.children()
            .filter_as::<SampleAuxiliaryInformationOffsetsBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the SampleDependencyTypeBox child, if present.
    pub fn sdtp(&self) -> Option<SampleDependencyTypeBoxView<'a>> {
        self.children()
            .find_as::<SampleDependencyTypeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the SubSampleInformationBox child, if present.
    pub fn subs(&self) -> Option<SubSampleInformationBoxView<'a>> {
        self.children()
            .find_as::<SubSampleInformationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the SampleEncryptionBox child, if present.
    pub fn senc(&self) -> Option<SampleEncryptionBoxView<'a>> {
        self.children()
            .find_as::<SampleEncryptionBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> TrackFragmentBox for TrackFragmentBoxView<'a> {
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

impl std::fmt::Debug for TrackFragmentBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self.children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("TrackFragmentBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of TrackFragmentBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct TrackFragmentBoxOwned {
    /// Typed child boxes.
    pub children: Vec<TrackFragmentChild>,
}

impl TrackFragmentBoxOwned {
    /// Creates a new empty TrackFragmentBoxOwned.
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

    /// Returns the TrackFragmentHeaderBox child, if present.
    pub fn tfhd(&self) -> Option<&TrackFragmentHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackFragmentChild::Tfhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the TrackFragmentBaseMediaDecodeTimeBox child, if present.
    pub fn tfdt(&self) -> Option<&TrackFragmentBaseMediaDecodeTimeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackFragmentChild::Tfdt(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all TrackRunBox children.
    pub fn trun(&self) -> impl Iterator<Item = &TrackRunBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            TrackFragmentChild::Trun(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleToGroupBox children.
    pub fn sbgp(&self) -> impl Iterator<Item = &SampleToGroupBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            TrackFragmentChild::Sbgp(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleGroupDescriptionBox children.
    pub fn sgpd(&self) -> impl Iterator<Item = &SampleGroupDescriptionBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            TrackFragmentChild::Sgpd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleAuxiliaryInformationSizesBox children.
    pub fn saiz(&self) -> impl Iterator<Item = &SampleAuxiliaryInformationSizesBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            TrackFragmentChild::Saiz(b) => Some(b),
            _ => None,
        })
    }

    /// Returns all SampleAuxiliaryInformationOffsetsBox children.
    pub fn saio(&self) -> impl Iterator<Item = &SampleAuxiliaryInformationOffsetsBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            TrackFragmentChild::Saio(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SampleDependencyTypeBox child, if present.
    pub fn sdtp(&self) -> Option<&SampleDependencyTypeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackFragmentChild::Sdtp(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SubSampleInformationBox child, if present.
    pub fn subs(&self) -> Option<&SubSampleInformationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackFragmentChild::Subs(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SampleEncryptionBox child, if present.
    pub fn senc(&self) -> Option<&SampleEncryptionBoxOwned> {
        self.children.iter().find_map(|c| match c {
            TrackFragmentChild::Senc(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a TrackFragmentHeaderBox child.
    pub fn add_tfhd(&mut self, b: TrackFragmentHeaderBoxOwned) {
        self.children.push(TrackFragmentChild::Tfhd(b));
    }

    /// Adds a TrackFragmentBaseMediaDecodeTimeBox child.
    pub fn add_tfdt(&mut self, b: TrackFragmentBaseMediaDecodeTimeBoxOwned) {
        self.children.push(TrackFragmentChild::Tfdt(b));
    }

    /// Adds a TrackRunBox child.
    pub fn add_trun(&mut self, b: TrackRunBoxOwned) {
        self.children.push(TrackFragmentChild::Trun(b));
    }

    /// Adds a SampleToGroupBox child.
    pub fn add_sbgp(&mut self, b: SampleToGroupBoxOwned) {
        self.children.push(TrackFragmentChild::Sbgp(b));
    }

    /// Adds a SampleGroupDescriptionBox child.
    pub fn add_sgpd(&mut self, b: SampleGroupDescriptionBoxOwned) {
        self.children.push(TrackFragmentChild::Sgpd(b));
    }

    /// Adds a SampleAuxiliaryInformationSizesBox child.
    pub fn add_saiz(&mut self, b: SampleAuxiliaryInformationSizesBoxOwned) {
        self.children.push(TrackFragmentChild::Saiz(b));
    }

    /// Adds a SampleAuxiliaryInformationOffsetsBox child.
    pub fn add_saio(&mut self, b: SampleAuxiliaryInformationOffsetsBoxOwned) {
        self.children.push(TrackFragmentChild::Saio(b));
    }

    /// Adds a SampleDependencyTypeBox child.
    pub fn add_sdtp(&mut self, b: SampleDependencyTypeBoxOwned) {
        self.children.push(TrackFragmentChild::Sdtp(b));
    }

    /// Adds a SubSampleInformationBox child.
    pub fn add_subs(&mut self, b: SubSampleInformationBoxOwned) {
        self.children.push(TrackFragmentChild::Subs(b));
    }

    /// Adds a SampleEncryptionBox child.
    pub fn add_senc(&mut self, b: SampleEncryptionBoxOwned) {
        self.children.push(TrackFragmentChild::Senc(b));
    }
}

impl TrackFragmentBox for TrackFragmentBoxOwned {
    type Child<'a> = &'a TrackFragmentChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &TrackFragmentChild> {
        self.children.iter()
    }
}

impl<T: TrackFragmentBox> From<&T> for TrackFragmentBoxOwned {
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
    fn parse_empty_traf() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"traf");

        let view = TrackFragmentBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }
}
