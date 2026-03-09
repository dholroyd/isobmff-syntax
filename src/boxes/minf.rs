//! Media Information Box (minf) parsing and serialization.
//!
//! The Media Information Box contains all the objects that declare the media information.
//!
//! ```text
//! aligned(8) class MediaInformationBox extends Box('minf') {
//! }
//! ```

use crate::boxes::dinf::{
    self, DataInformationBox as _, DataInformationBoxOwned, DataInformationBoxView,
};
use crate::boxes::hmhd::{
    self, HintMediaHeaderBox as _, HintMediaHeaderBoxOwned, HintMediaHeaderBoxView,
};
use crate::boxes::nmhd::{
    self, NullMediaHeaderBox as _, NullMediaHeaderBoxOwned, NullMediaHeaderBoxView,
};
use crate::boxes::smhd::{
    self, SoundMediaHeaderBox as _, SoundMediaHeaderBoxOwned, SoundMediaHeaderBoxView,
};
use crate::boxes::stbl::{
    self, SampleTableBox as _, SampleTableBoxOwned, SampleTableBoxView,
};
use crate::boxes::sthd::{
    self, SubtitleMediaHeaderBox as _, SubtitleMediaHeaderBoxOwned, SubtitleMediaHeaderBoxView,
};
use crate::boxes::vmhd::{
    self, VideoMediaHeaderBox as _, VideoMediaHeaderBoxOwned, VideoMediaHeaderBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MediaInformationBox.
pub const BOX_TYPE: BoxCode = BoxCode::MINF;

/// A typed child of a MediaInformationBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaInformationChild {
    /// A VideoMediaHeaderBox child.
    Vmhd(VideoMediaHeaderBoxOwned),
    /// A SoundMediaHeaderBox child.
    Smhd(SoundMediaHeaderBoxOwned),
    /// A HintMediaHeaderBox child.
    Hmhd(HintMediaHeaderBoxOwned),
    /// A SubtitleMediaHeaderBox child.
    Sthd(SubtitleMediaHeaderBoxOwned),
    /// A NullMediaHeaderBox child.
    Nmhd(NullMediaHeaderBoxOwned),
    /// A DataInformationBox child.
    Dinf(DataInformationBoxOwned),
    /// A SampleTableBox child.
    Stbl(SampleTableBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for MediaInformationChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Vmhd(_) => vmhd::BOX_TYPE,
            Self::Smhd(_) => smhd::BOX_TYPE,
            Self::Hmhd(_) => hmhd::BOX_TYPE,
            Self::Sthd(_) => sthd::BOX_TYPE,
            Self::Nmhd(_) => nmhd::BOX_TYPE,
            Self::Dinf(_) => dinf::BOX_TYPE,
            Self::Stbl(_) => stbl::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Vmhd(b) => b.box_size(),
            Self::Smhd(b) => b.box_size(),
            Self::Hmhd(b) => b.box_size(),
            Self::Sthd(b) => b.box_size(),
            Self::Nmhd(b) => b.box_size(),
            Self::Dinf(b) => b.box_size(),
            Self::Stbl(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl MediaInformationChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Vmhd(b) => b.write_to(writer),
            Self::Smhd(b) => b.write_to(writer),
            Self::Hmhd(b) => b.write_to(writer),
            Self::Sthd(b) => b.write_to(writer),
            Self::Nmhd(b) => b.write_to(writer),
            Self::Dinf(b) => b.write_to(writer),
            Self::Stbl(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for MediaInformationChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            vmhd::BOX_TYPE => match VideoMediaHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Vmhd(VideoMediaHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            smhd::BOX_TYPE => match SoundMediaHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Smhd(SoundMediaHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            hmhd::BOX_TYPE => match HintMediaHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Hmhd(HintMediaHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            sthd::BOX_TYPE => match SubtitleMediaHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Sthd(SubtitleMediaHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            nmhd::BOX_TYPE => match NullMediaHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Nmhd(NullMediaHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            dinf::BOX_TYPE => match DataInformationBoxView::new(raw.data()) {
                Ok(v) => Self::Dinf(DataInformationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            stbl::BOX_TYPE => match SampleTableBoxView::new(raw.data()) {
                Ok(v) => Self::Stbl(SampleTableBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&MediaInformationChild> for MediaInformationChild {
    fn from(source: &MediaInformationChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing MediaInformationBox data.
pub trait MediaInformationBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<MediaInformationChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw MediaInformationBox bytes.
#[derive(Clone, Copy)]
pub struct MediaInformationBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> MediaInformationBoxView<'a> {
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

    /// Returns the first VideoMediaHeaderBox child, if present.
    pub fn vmhd(&self) -> Option<VideoMediaHeaderBoxView<'a>> {
        self.children()
            .find_as::<VideoMediaHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first SoundMediaHeaderBox child, if present.
    pub fn smhd(&self) -> Option<SoundMediaHeaderBoxView<'a>> {
        self.children()
            .find_as::<SoundMediaHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first HintMediaHeaderBox child, if present.
    pub fn hmhd(&self) -> Option<HintMediaHeaderBoxView<'a>> {
        self.children()
            .find_as::<HintMediaHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first SubtitleMediaHeaderBox child, if present.
    pub fn sthd(&self) -> Option<SubtitleMediaHeaderBoxView<'a>> {
        self.children()
            .find_as::<SubtitleMediaHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first NullMediaHeaderBox child, if present.
    pub fn nmhd(&self) -> Option<NullMediaHeaderBoxView<'a>> {
        self.children()
            .find_as::<NullMediaHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first DataInformationBox child, if present.
    pub fn dinf(&self) -> Option<DataInformationBoxView<'a>> {
        self.children()
            .find_as::<DataInformationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first SampleTableBox child, if present.
    pub fn stbl(&self) -> Option<SampleTableBoxView<'a>> {
        self.children()
            .find_as::<SampleTableBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> MediaInformationBox for MediaInformationBoxView<'a> {
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

impl std::fmt::Debug for MediaInformationBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("MediaInformationBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of MediaInformationBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MediaInformationBoxOwned {
    /// Typed child boxes.
    pub children: Vec<MediaInformationChild>,
}

impl MediaInformationBoxOwned {
    /// Creates a new empty MediaInformationBoxOwned.
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

    /// Returns the first VideoMediaHeaderBox child, if present.
    pub fn vmhd(&self) -> Option<&VideoMediaHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaInformationChild::Vmhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first SoundMediaHeaderBox child, if present.
    pub fn smhd(&self) -> Option<&SoundMediaHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaInformationChild::Smhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first HintMediaHeaderBox child, if present.
    pub fn hmhd(&self) -> Option<&HintMediaHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaInformationChild::Hmhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first SubtitleMediaHeaderBox child, if present.
    pub fn sthd(&self) -> Option<&SubtitleMediaHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaInformationChild::Sthd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first NullMediaHeaderBox child, if present.
    pub fn nmhd(&self) -> Option<&NullMediaHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaInformationChild::Nmhd(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first DataInformationBox child, if present.
    pub fn dinf(&self) -> Option<&DataInformationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaInformationChild::Dinf(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first SampleTableBox child, if present.
    pub fn stbl(&self) -> Option<&SampleTableBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MediaInformationChild::Stbl(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a VideoMediaHeaderBox child.
    pub fn add_vmhd(&mut self, b: VideoMediaHeaderBoxOwned) {
        self.children.push(MediaInformationChild::Vmhd(b));
    }

    /// Adds a SoundMediaHeaderBox child.
    pub fn add_smhd(&mut self, b: SoundMediaHeaderBoxOwned) {
        self.children.push(MediaInformationChild::Smhd(b));
    }

    /// Adds a HintMediaHeaderBox child.
    pub fn add_hmhd(&mut self, b: HintMediaHeaderBoxOwned) {
        self.children.push(MediaInformationChild::Hmhd(b));
    }

    /// Adds a SubtitleMediaHeaderBox child.
    pub fn add_sthd(&mut self, b: SubtitleMediaHeaderBoxOwned) {
        self.children.push(MediaInformationChild::Sthd(b));
    }

    /// Adds a NullMediaHeaderBox child.
    pub fn add_nmhd(&mut self, b: NullMediaHeaderBoxOwned) {
        self.children.push(MediaInformationChild::Nmhd(b));
    }

    /// Adds a DataInformationBox child.
    pub fn add_dinf(&mut self, b: DataInformationBoxOwned) {
        self.children.push(MediaInformationChild::Dinf(b));
    }

    /// Adds a SampleTableBox child.
    pub fn add_stbl(&mut self, b: SampleTableBoxOwned) {
        self.children.push(MediaInformationChild::Stbl(b));
    }
}

impl MediaInformationBox for MediaInformationBoxOwned {
    type Child<'a> = &'a MediaInformationChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &MediaInformationChild> {
        self.children.iter()
    }
}

impl<T: MediaInformationBox> From<&T> for MediaInformationBoxOwned {
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
    fn parse_empty_minf() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"minf");

        let view = MediaInformationBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"minf");
        data.extend_from_slice(&[0u8; 16]);

        let view = MediaInformationBoxView::new(&data).unwrap();
        let owned = MediaInformationBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
