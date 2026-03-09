//! Hint Track Information Box (hnti) parsing and serialization.
//!
//! The Hint Track Information Box contains information about the hint track.
//!
//! ```text
//! aligned(8) class moviehintinformation extends Box('hnti') {
//! }
//! ```

use crate::boxes::sdp::{self, SDPBox as _, SDPBoxOwned, SDPBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for HintTrackInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::HNTI;

/// A typed child of a HintTrackInfoBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HintTrackInfoChild {
    /// An SDPBox child.
    Sdp(SDPBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for HintTrackInfoChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Sdp(_) => sdp::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Sdp(s) => s.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl HintTrackInfoChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Sdp(s) => s.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for HintTrackInfoChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            sdp::BOX_TYPE => match SDPBoxView::new(raw.data()) {
                Ok(v) => match SDPBoxOwned::try_from(&v) {
                    Ok(owned) => Self::Sdp(owned),
                    Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
                },
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&HintTrackInfoChild> for HintTrackInfoChild {
    fn from(source: &HintTrackInfoChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing HintTrackInfoBox data.
pub trait HintTrackInfoBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<HintTrackInfoChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw HintTrackInfoBox bytes.
#[derive(Clone, Copy)]
pub struct HintTrackInfoBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> HintTrackInfoBoxView<'a> {
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

    /// Returns the first SDPBox child, if any.
    pub fn sdp(&self) -> Option<SDPBoxView<'a>> {
        self.children()
            .find_as::<SDPBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> HintTrackInfoBox for HintTrackInfoBoxView<'a> {
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

impl std::fmt::Debug for HintTrackInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HintTrackInfoBoxView")
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of HintTrackInfoBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HintTrackInfoBoxOwned {
    /// Typed child boxes.
    pub children: Vec<HintTrackInfoChild>,
}

impl HintTrackInfoBoxOwned {
    /// Creates a new HintTrackInfoBoxOwned.
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

    /// Returns the first SDPBox child, if any.
    pub fn sdp(&self) -> Option<&SDPBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintTrackInfoChild::Sdp(b) => Some(b),
            _ => None,
        })
    }
}

impl HintTrackInfoBox for HintTrackInfoBoxOwned {
    type Child<'a> = &'a HintTrackInfoChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &HintTrackInfoChild> {
        self.children.iter()
    }
}

impl<T: HintTrackInfoBox> From<&T> for HintTrackInfoBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::sdp::SDPBox;

    fn make_hnti() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"hnti");
        data
    }

    fn make_sdp_bytes() -> Vec<u8> {
        let sdp_text = b"v=0";
        let mut data = Vec::new();
        data.extend_from_slice(&(8 + sdp_text.len() as u32).to_be_bytes());
        data.extend_from_slice(b"sdp ");
        data.extend_from_slice(sdp_text);
        data
    }

    fn make_unknown_box() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"xyzw");
        data.extend_from_slice(&[0xAB, 0xCD, 0xEF, 0x01]);
        data
    }

    fn make_hnti_with_children(children: &[&[u8]]) -> Vec<u8> {
        let children_len: usize = children.iter().map(|c| c.len()).sum();
        let size = 8 + children_len;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"hnti");
        for child in children {
            data.extend_from_slice(child);
        }
        data
    }

    #[test]
    fn parse_empty_hnti() {
        let data = make_hnti();
        let view = HintTrackInfoBoxView::new(&data).unwrap();
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_hnti();
        let view = HintTrackInfoBoxView::new(&data).unwrap();
        let owned = HintTrackInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_sdp_children() {
        let sdp = make_sdp_bytes();
        let data = make_hnti_with_children(&[&sdp]);

        let view = HintTrackInfoBoxView::new(&data).unwrap();
        assert!(view.sdp().is_some());
        assert_eq!(view.sdp().unwrap().sdp_text_str(), Some("v=0"));

        let owned = HintTrackInfoBoxOwned::from(&view);
        assert!(owned.sdp().is_some());
        assert_eq!(owned.sdp().unwrap().sdp_text_str(), Some("v=0"));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_unknown_children() {
        let sdp = make_sdp_bytes();
        let unknown = make_unknown_box();
        let data = make_hnti_with_children(&[&sdp, &unknown]);

        let view = HintTrackInfoBoxView::new(&data).unwrap();
        let owned = HintTrackInfoBoxOwned::from(&view);

        // Known child parsed
        assert!(owned.sdp().is_some());
        // Total children includes unknown
        assert_eq!(owned.children.len(), 2);
        assert!(matches!(owned.children[1], HintTrackInfoChild::Other(_)));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn view_convenience_methods() {
        let sdp = make_sdp_bytes();
        let unknown = make_unknown_box();
        let data = make_hnti_with_children(&[&unknown, &sdp]);

        let view = HintTrackInfoBoxView::new(&data).unwrap();
        // sdp() finds the sdp box even when it's not first
        let sdp_view = view.sdp().unwrap();
        assert_eq!(sdp_view.sdp_text_str(), Some("v=0"));
    }

    #[test]
    fn child_box_type() {
        let sdp = make_sdp_bytes();
        let unknown = make_unknown_box();
        let data = make_hnti_with_children(&[&sdp, &unknown]);

        let view = HintTrackInfoBoxView::new(&data).unwrap();
        let owned = HintTrackInfoBoxOwned::from(&view);

        assert_eq!(ChildBox::box_type(&owned.children[0]), BoxCode::SDP);
        assert_eq!(
            ChildBox::box_type(&owned.children[1]),
            BoxCode::new(*b"xyzw")
        );
    }
}
