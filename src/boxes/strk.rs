//! Sub Track Box (strk) parsing and serialization.
//!
//! The Sub Track Box is a container for sub-track information.
//!
//! ```text
//! aligned(8) class SubTrackBox extends Box('strk') {
//! }
//! ```

use crate::boxes::strd::{
    self, SubTrackDefinitionBox as _, SubTrackDefinitionBoxOwned, SubTrackDefinitionBoxView,
};
use crate::boxes::stri::{
    self, SubTrackInformationBox as _, SubTrackInformationBoxOwned, SubTrackInformationBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SubTrackBox.
pub const BOX_TYPE: BoxCode = BoxCode::STRK;

/// A typed child of a SubTrackBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubTrackChild {
    /// A SubTrackInformationBox child.
    Stri(SubTrackInformationBoxOwned),
    /// A SubTrackDefinitionBox child.
    Strd(SubTrackDefinitionBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for SubTrackChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Stri(_) => stri::BOX_TYPE,
            Self::Strd(_) => strd::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Stri(s) => s.box_size(),
            Self::Strd(s) => s.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl SubTrackChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Stri(s) => s.write_to(writer),
            Self::Strd(s) => s.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for SubTrackChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            stri::BOX_TYPE => match SubTrackInformationBoxView::new(raw.data()) {
                Ok(v) => Self::Stri(SubTrackInformationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            strd::BOX_TYPE => match SubTrackDefinitionBoxView::new(raw.data()) {
                Ok(v) => Self::Strd(SubTrackDefinitionBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&SubTrackChild> for SubTrackChild {
    fn from(source: &SubTrackChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing SubTrackBox data.
pub trait SubTrackBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<SubTrackChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw SubTrackBox bytes.
#[derive(Clone, Copy)]
pub struct SubTrackBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> SubTrackBoxView<'a> {
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

    /// Returns the first SubTrackInformationBox child, if present.
    pub fn stri(&self) -> Option<SubTrackInformationBoxView<'a>> {
        self.children()
            .find_as::<SubTrackInformationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the first SubTrackDefinitionBox child, if present.
    pub fn strd(&self) -> Option<SubTrackDefinitionBoxView<'a>> {
        self.children()
            .find_as::<SubTrackDefinitionBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> SubTrackBox for SubTrackBoxView<'a> {
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

impl std::fmt::Debug for SubTrackBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubTrackBoxView")
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of SubTrackBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SubTrackBoxOwned {
    /// Typed child boxes.
    pub children: Vec<SubTrackChild>,
}

impl SubTrackBoxOwned {
    /// Creates a new SubTrackBoxOwned.
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

    /// Returns the first SubTrackInformationBox child, if present.
    pub fn stri(&self) -> Option<&SubTrackInformationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SubTrackChild::Stri(s) => Some(s),
            _ => None,
        })
    }

    /// Returns the first SubTrackDefinitionBox child, if present.
    pub fn strd(&self) -> Option<&SubTrackDefinitionBoxOwned> {
        self.children.iter().find_map(|c| match c {
            SubTrackChild::Strd(s) => Some(s),
            _ => None,
        })
    }
}

impl SubTrackBox for SubTrackBoxOwned {
    type Child<'a> = &'a SubTrackChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &SubTrackChild> {
        self.children.iter()
    }
}

impl<T: SubTrackBox> From<&T> for SubTrackBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_strk() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"strk");
        data
    }

    fn make_stri_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 8 = 20 bytes
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"stri");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1i16.to_be_bytes()); // switch_group
        data.extend_from_slice(&2i16.to_be_bytes()); // alternate_group
        data.extend_from_slice(&100u32.to_be_bytes()); // sub_track_id
        data
    }

    fn make_strd_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"strd");
        data
    }

    fn make_unknown_box() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"xyzw");
        data.extend_from_slice(&[0xAB, 0xCD, 0xEF, 0x01]);
        data
    }

    fn make_strk_with_children(children: &[&[u8]]) -> Vec<u8> {
        let children_len: usize = children.iter().map(|c| c.len()).sum();
        let size = 8 + children_len;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"strk");
        for child in children {
            data.extend_from_slice(child);
        }
        data
    }

    #[test]
    fn parse_empty_strk() {
        let data = make_strk();
        let view = SubTrackBoxView::new(&data).unwrap();
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_strk();
        let view = SubTrackBoxView::new(&data).unwrap();
        let owned = SubTrackBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_stri_and_strd() {
        let stri = make_stri_bytes();
        let strd = make_strd_bytes();
        let data = make_strk_with_children(&[&stri, &strd]);

        let view = SubTrackBoxView::new(&data).unwrap();
        assert!(view.stri().is_some());
        assert!(view.strd().is_some());
        assert_eq!(view.stri().unwrap().sub_track_id(), 100);

        let owned = SubTrackBoxOwned::from(&view);
        assert!(owned.stri().is_some());
        assert!(owned.strd().is_some());
        assert_eq!(owned.stri().unwrap().sub_track_id, 100);
        assert_eq!(owned.children.len(), 2);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_unknown_children() {
        let stri = make_stri_bytes();
        let unknown = make_unknown_box();
        let strd = make_strd_bytes();
        let data = make_strk_with_children(&[&stri, &unknown, &strd]);

        let view = SubTrackBoxView::new(&data).unwrap();
        let owned = SubTrackBoxOwned::from(&view);

        assert!(owned.stri().is_some());
        assert!(owned.strd().is_some());
        assert_eq!(owned.children.len(), 3);
        assert!(matches!(owned.children[1], SubTrackChild::Other(_)));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn view_convenience_methods() {
        let stri = make_stri_bytes();
        let strd = make_strd_bytes();
        let data = make_strk_with_children(&[&strd, &stri]);

        let view = SubTrackBoxView::new(&data).unwrap();
        assert!(view.stri().is_some());
        assert!(view.strd().is_some());
        // Order doesn't affect finding
        assert_eq!(view.stri().unwrap().sub_track_id(), 100);
    }

    #[test]
    fn child_box_types() {
        let stri = make_stri_bytes();
        let strd = make_strd_bytes();
        let unknown = make_unknown_box();
        let data = make_strk_with_children(&[&stri, &strd, &unknown]);

        let view = SubTrackBoxView::new(&data).unwrap();
        let owned = SubTrackBoxOwned::from(&view);

        assert_eq!(ChildBox::box_type(&owned.children[0]), BoxCode::STRI);
        assert_eq!(ChildBox::box_type(&owned.children[1]), BoxCode::STRD);
        assert_eq!(
            ChildBox::box_type(&owned.children[2]),
            BoxCode::new(*b"xyzw")
        );
    }
}
