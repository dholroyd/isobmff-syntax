//! Sub Track Definition Box (strd) parsing and serialization.
//!
//! The Sub Track Definition Box is a container for sub-track sample group boxes.
//!
//! ```text
//! aligned(8) class SubTrackDefinitionBox extends Box('strd') {
//! }
//! ```

use crate::boxes::stsg::{
    self, SubTrackSampleGroupBox as _, SubTrackSampleGroupBoxOwned, SubTrackSampleGroupBoxView,
};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SubTrackDefinitionBox.
pub const BOX_TYPE: BoxCode = BoxCode::STRD;

/// A typed child of a SubTrackDefinitionBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubTrackDefinitionChild {
    /// A SubTrackSampleGroupBox child.
    Stsg(SubTrackSampleGroupBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for SubTrackDefinitionChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Stsg(_) => stsg::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Stsg(s) => s.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl SubTrackDefinitionChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Stsg(s) => s.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for SubTrackDefinitionChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            stsg::BOX_TYPE => match SubTrackSampleGroupBoxView::new(raw.data()) {
                Ok(v) => Self::Stsg(SubTrackSampleGroupBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&SubTrackDefinitionChild> for SubTrackDefinitionChild {
    fn from(source: &SubTrackDefinitionChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing SubTrackDefinitionBox data.
pub trait SubTrackDefinitionBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<SubTrackDefinitionChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw SubTrackDefinitionBox bytes.
#[derive(Clone, Copy)]
pub struct SubTrackDefinitionBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> SubTrackDefinitionBoxView<'a> {
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

    /// Returns an iterator over SubTrackSampleGroupBox children.
    pub fn stsg_children(&self) -> impl Iterator<Item = SubTrackSampleGroupBoxView<'a>> {
        self.children()
            .filter_as::<SubTrackSampleGroupBoxView>()
            .filter_map(Result::ok)
    }
}

impl<'a> SubTrackDefinitionBox for SubTrackDefinitionBoxView<'a> {
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

impl std::fmt::Debug for SubTrackDefinitionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubTrackDefinitionBoxView")
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of SubTrackDefinitionBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SubTrackDefinitionBoxOwned {
    /// Typed child boxes.
    pub children: Vec<SubTrackDefinitionChild>,
}

impl SubTrackDefinitionBoxOwned {
    /// Creates a new SubTrackDefinitionBoxOwned.
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

    /// Returns an iterator over SubTrackSampleGroupBox children.
    pub fn stsg_children(&self) -> impl Iterator<Item = &SubTrackSampleGroupBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            SubTrackDefinitionChild::Stsg(s) => Some(s),
            _ => None,
        })
    }
}

impl SubTrackDefinitionBox for SubTrackDefinitionBoxOwned {
    type Child<'a> = &'a SubTrackDefinitionChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &SubTrackDefinitionChild> {
        self.children.iter()
    }
}

impl<T: SubTrackDefinitionBox> From<&T> for SubTrackDefinitionBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::stsg::SubTrackSampleGroupBox;
    use mp4ra_rust::FourCC;

    fn make_strd() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"strd");
        data
    }

    fn make_stsg_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 2 = 18 bytes (no indices)
        data.extend_from_slice(&18u32.to_be_bytes());
        data.extend_from_slice(b"stsg");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(b"roll"); // grouping_type
        data.extend_from_slice(&0u16.to_be_bytes()); // item_count
        data
    }

    fn make_unknown_box() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"xyzw");
        data.extend_from_slice(&[0xAB, 0xCD, 0xEF, 0x01]);
        data
    }

    fn make_strd_with_children(children: &[&[u8]]) -> Vec<u8> {
        let children_len: usize = children.iter().map(|c| c.len()).sum();
        let size = 8 + children_len;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"strd");
        for child in children {
            data.extend_from_slice(child);
        }
        data
    }

    #[test]
    fn parse_empty_strd() {
        let data = make_strd();
        let view = SubTrackDefinitionBoxView::new(&data).unwrap();
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_strd();
        let view = SubTrackDefinitionBoxView::new(&data).unwrap();
        let owned = SubTrackDefinitionBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_stsg_children() {
        let stsg = make_stsg_bytes();
        let data = make_strd_with_children(&[&stsg]);

        let view = SubTrackDefinitionBoxView::new(&data).unwrap();
        assert_eq!(view.stsg_children().count(), 1);

        let owned = SubTrackDefinitionBoxOwned::from(&view);
        assert_eq!(owned.stsg_children().count(), 1);
        assert_eq!(
            owned.stsg_children().next().unwrap().grouping_type,
            FourCC(*b"roll")
        );

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_unknown_children() {
        let stsg = make_stsg_bytes();
        let unknown = make_unknown_box();
        let data = make_strd_with_children(&[&stsg, &unknown]);

        let view = SubTrackDefinitionBoxView::new(&data).unwrap();
        let owned = SubTrackDefinitionBoxOwned::from(&view);

        // Known child parsed
        assert_eq!(owned.stsg_children().count(), 1);
        // Total children includes unknown
        assert_eq!(owned.children.len(), 2);
        assert!(matches!(owned.children[1], SubTrackDefinitionChild::Other(_)));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn view_convenience_methods() {
        let stsg = make_stsg_bytes();
        let unknown = make_unknown_box();
        let data = make_strd_with_children(&[&unknown, &stsg]);

        let view = SubTrackDefinitionBoxView::new(&data).unwrap();
        // stsg_children filters to only stsg boxes
        let stsg_views: Vec<_> = view.stsg_children().collect();
        assert_eq!(stsg_views.len(), 1);
        assert_eq!(stsg_views[0].grouping_type(), FourCC(*b"roll"));
    }

    #[test]
    fn child_box_type() {
        let stsg = make_stsg_bytes();
        let unknown = make_unknown_box();
        let data = make_strd_with_children(&[&stsg, &unknown]);

        let view = SubTrackDefinitionBoxView::new(&data).unwrap();
        let owned = SubTrackDefinitionBoxOwned::from(&view);

        assert_eq!(ChildBox::box_type(&owned.children[0]), BoxCode::STSG);
        assert_eq!(
            ChildBox::box_type(&owned.children[1]),
            BoxCode::new(*b"xyzw")
        );
    }
}
