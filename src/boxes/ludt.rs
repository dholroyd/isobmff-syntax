//! Loudness Base Box (ludt) parsing and serialization.
//!
//! The Loudness Base Box is a container for loudness information.
//!
//! ```text
//! aligned(8) class LoudnessBox extends Box('ludt') {
//!    // not more than one TrackLoudnessInfo box with version>=1 is allowed
//!    loudness TrackLoudnessInfo[];
//!    // not more than one AlbumLoudnessInfo box with version>=1 is allowed
//!    albumLoudness AlbumLoudnessInfo[];
//! }
//! ```

use crate::boxes::alou::{
    self, AlbumLoudnessInfoBox as _, AlbumLoudnessInfoBoxOwned, AlbumLoudnessInfoBoxView,
};
use crate::boxes::tlou::{
    self, TrackLoudnessInfoBox as _, TrackLoudnessInfoBoxOwned, TrackLoudnessInfoBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for LoudnessBaseBox.
pub const BOX_TYPE: BoxCode = BoxCode::LUDT;

/// A typed child of a LoudnessBaseBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoudnessBaseChild {
    /// A TrackLoudnessInfoBox child.
    Tlou(TrackLoudnessInfoBoxOwned),
    /// An AlbumLoudnessInfoBox child.
    Alou(AlbumLoudnessInfoBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for LoudnessBaseChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Tlou(_) => tlou::BOX_TYPE,
            Self::Alou(_) => alou::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Tlou(t) => t.box_size(),
            Self::Alou(a) => a.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl LoudnessBaseChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Tlou(t) => t.write_to(writer),
            Self::Alou(a) => a.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for LoudnessBaseChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            tlou::BOX_TYPE => match TrackLoudnessInfoBoxView::new(raw.data()) {
                Ok(v) => Self::Tlou(TrackLoudnessInfoBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            alou::BOX_TYPE => match AlbumLoudnessInfoBoxView::new(raw.data()) {
                Ok(v) => Self::Alou(AlbumLoudnessInfoBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&LoudnessBaseChild> for LoudnessBaseChild {
    fn from(source: &LoudnessBaseChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing LoudnessBaseBox data.
pub trait LoudnessBaseBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<LoudnessBaseChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw LoudnessBaseBox bytes.
#[derive(Clone, Copy)]
pub struct LoudnessBaseBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> LoudnessBaseBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;
        header.validate(data, BOX_TYPE, 0)?;
        Ok(Self { data, header_size: header.header_size as usize })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.header_size..])
    }

    /// Returns the first TrackLoudnessInfoBox child, if any.
    pub fn tlou(&self) -> Option<TrackLoudnessInfoBoxView<'a>> {
        self.children()
            .find_as::<TrackLoudnessInfoBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first AlbumLoudnessInfoBox child, if any.
    pub fn alou(&self) -> Option<AlbumLoudnessInfoBoxView<'a>> {
        self.children()
            .find_as::<AlbumLoudnessInfoBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> LoudnessBaseBox for LoudnessBaseBoxView<'a> {
    type Child<'b> = RawBox<'b> where Self: 'b;

    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for LoudnessBaseBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoudnessBaseBoxView")
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of LoudnessBaseBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LoudnessBaseBoxOwned {
    /// Typed child boxes.
    pub children: Vec<LoudnessBaseChild>,
}

impl LoudnessBaseBoxOwned {
    /// Creates a new LoudnessBaseBoxOwned.
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

    /// Returns the first TrackLoudnessInfoBox child, if any.
    pub fn tlou(&self) -> Option<&TrackLoudnessInfoBoxOwned> {
        self.children.iter().find_map(|c| match c {
            LoudnessBaseChild::Tlou(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first AlbumLoudnessInfoBox child, if any.
    pub fn alou(&self) -> Option<&AlbumLoudnessInfoBoxOwned> {
        self.children.iter().find_map(|c| match c {
            LoudnessBaseChild::Alou(b) => Some(b),
            _ => None,
        })
    }
}

impl LoudnessBaseBox for LoudnessBaseBoxOwned {
    type Child<'a> = &'a LoudnessBaseChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &LoudnessBaseChild> {
        self.children.iter()
    }
}

impl<T: LoudnessBaseBox> From<&T> for LoudnessBaseBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::loudness::{LoudnessData, loudness_payload_size, write_loudness_payload};
    use crate::boxes::tlou::TrackLoudnessInfoBox;

    fn make_ludt() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"ludt");
        data
    }

    fn make_loudness_box(box_type: &[u8; 4]) -> Vec<u8> {
        let loudness = LoudnessData::default();
        let payload_size = loudness_payload_size(0, &loudness);
        let total = 12 + payload_size as usize; // 8 (header) + 4 (version/flags) + payload
        let mut data = Vec::with_capacity(total);
        data.extend_from_slice(&(total as u32).to_be_bytes());
        data.extend_from_slice(box_type);
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        let mut payload = Vec::new();
        write_loudness_payload(0, &loudness, &mut payload).unwrap();
        data.extend_from_slice(&payload);
        data
    }

    fn make_tlou_bytes() -> Vec<u8> {
        make_loudness_box(b"tlou")
    }

    fn make_alou_bytes() -> Vec<u8> {
        make_loudness_box(b"alou")
    }

    fn make_unknown_box() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"xyzw");
        data.extend_from_slice(&[0xAB, 0xCD, 0xEF, 0x01]);
        data
    }

    fn make_ludt_with_children(children: &[&[u8]]) -> Vec<u8> {
        let children_len: usize = children.iter().map(|c| c.len()).sum();
        let size = 8 + children_len;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"ludt");
        for child in children {
            data.extend_from_slice(child);
        }
        data
    }

    #[test]
    fn parse_empty_ludt() {
        let data = make_ludt();
        let view = LoudnessBaseBoxView::new(&data).unwrap();
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_ludt();
        let view = LoudnessBaseBoxView::new(&data).unwrap();
        let owned = LoudnessBaseBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_tlou_children() {
        let tlou = make_tlou_bytes();
        let data = make_ludt_with_children(&[&tlou]);

        let view = LoudnessBaseBoxView::new(&data).unwrap();
        assert!(view.tlou().is_some());

        let owned = LoudnessBaseBoxOwned::from(&view);
        assert!(owned.tlou().is_some());
        assert_eq!(owned.tlou().unwrap().version(), 0);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_alou_children() {
        let alou = make_alou_bytes();
        let data = make_ludt_with_children(&[&alou]);

        let view = LoudnessBaseBoxView::new(&data).unwrap();
        assert!(view.alou().is_some());

        let owned = LoudnessBaseBoxOwned::from(&view);
        assert!(owned.alou().is_some());

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_unknown_children() {
        let tlou = make_tlou_bytes();
        let unknown = make_unknown_box();
        let data = make_ludt_with_children(&[&tlou, &unknown]);

        let view = LoudnessBaseBoxView::new(&data).unwrap();
        let owned = LoudnessBaseBoxOwned::from(&view);

        // Known child parsed
        assert!(owned.tlou().is_some());
        // Total children includes unknown
        assert_eq!(owned.children.len(), 2);
        assert!(matches!(owned.children[1], LoudnessBaseChild::Other(_)));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn view_convenience_methods() {
        let tlou = make_tlou_bytes();
        let alou = make_alou_bytes();
        let unknown = make_unknown_box();
        let data = make_ludt_with_children(&[&unknown, &tlou, &alou]);

        let view = LoudnessBaseBoxView::new(&data).unwrap();
        // tlou/alou convenience methods filter to the correct boxes
        assert!(view.tlou().is_some());
        assert_eq!(view.tlou().unwrap().version(), 0);
        assert!(view.alou().is_some());
    }

    #[test]
    fn child_box_type() {
        let tlou = make_tlou_bytes();
        let alou = make_alou_bytes();
        let unknown = make_unknown_box();
        let data = make_ludt_with_children(&[&tlou, &alou, &unknown]);

        let view = LoudnessBaseBoxView::new(&data).unwrap();
        let owned = LoudnessBaseBoxOwned::from(&view);

        assert_eq!(ChildBox::box_type(&owned.children[0]), BoxCode::TLOU);
        assert_eq!(ChildBox::box_type(&owned.children[1]), BoxCode::ALOU);
        assert_eq!(
            ChildBox::box_type(&owned.children[2]),
            BoxCode::new(*b"xyzw")
        );
    }
}
