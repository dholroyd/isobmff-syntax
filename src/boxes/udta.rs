//! User Data Box (udta) parsing and serialization.
//!
//! The User Data Box contains user data such as copyright notices and metadata.
//!
//! ```text
//! aligned(8) class UserDataBox extends Box('udta') {
//! }
//! ```

use crate::boxes::cprt::{self, CopyrightBox as _, CopyrightBoxOwned, CopyrightBoxView};
use crate::boxes::kind::{self, KindBox as _, KindBoxOwned, KindBoxView};
use crate::boxes::meta::{self, MetadataBox as _, MetadataBoxOwned, MetadataBoxView};
use crate::boxes::tsel::{self, TrackSelectionBox as _, TrackSelectionBoxOwned, TrackSelectionBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for UserDataBox.
pub const BOX_TYPE: BoxCode = BoxCode::UDTA;

/// A typed child of a UserDataBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserDataChild {
    /// A CopyrightBox child.
    Cprt(CopyrightBoxOwned),
    /// A TrackSelectionBox child.
    Tsel(TrackSelectionBoxOwned),
    /// A KindBox child.
    Kind(KindBoxOwned),
    /// A MetadataBox child.
    Meta(MetadataBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for UserDataChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Cprt(_) => cprt::BOX_TYPE,
            Self::Tsel(_) => tsel::BOX_TYPE,
            Self::Kind(_) => kind::BOX_TYPE,
            Self::Meta(_) => meta::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Cprt(b) => b.box_size(),
            Self::Tsel(b) => b.box_size(),
            Self::Kind(b) => b.box_size(),
            Self::Meta(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl UserDataChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Cprt(b) => b.write_to(writer),
            Self::Tsel(b) => b.write_to(writer),
            Self::Kind(b) => b.write_to(writer),
            Self::Meta(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for UserDataChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            cprt::BOX_TYPE => match CopyrightBoxView::new(raw.data()) {
                Ok(v) => Self::Cprt(CopyrightBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            tsel::BOX_TYPE => match TrackSelectionBoxView::new(raw.data()) {
                Ok(v) => Self::Tsel(TrackSelectionBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            kind::BOX_TYPE => match KindBoxView::new(raw.data()) {
                Ok(v) => Self::Kind(KindBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            meta::BOX_TYPE => match MetadataBoxView::new(raw.data()) {
                Ok(v) => Self::Meta(MetadataBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&UserDataChild> for UserDataChild {
    fn from(source: &UserDataChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing UserDataBox data.
pub trait UserDataBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<UserDataChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw UserDataBox bytes.
#[derive(Clone, Copy)]
pub struct UserDataBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> UserDataBoxView<'a> {
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

    /// Returns an iterator over all CopyrightBox children.
    pub fn cprt_iter(&self) -> impl Iterator<Item = CopyrightBoxView<'a>> {
        self.children()
            .filter_as::<CopyrightBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the first CopyrightBox child, if present.
    pub fn cprt(&self) -> Option<CopyrightBoxView<'a>> {
        self.cprt_iter().next()
    }

    /// Returns the first TrackSelectionBox child, if present.
    pub fn tsel(&self) -> Option<TrackSelectionBoxView<'a>> {
        self.children()
            .find_as::<TrackSelectionBoxView>()
            .and_then(Result::ok)
    }

    /// Returns an iterator over all KindBox children.
    pub fn kind_iter(&self) -> impl Iterator<Item = KindBoxView<'a>> {
        self.children()
            .filter_as::<KindBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns the first KindBox child, if present.
    pub fn kind(&self) -> Option<KindBoxView<'a>> {
        self.kind_iter().next()
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<MetadataBoxView<'a>> {
        self.children()
            .find_as::<MetadataBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> UserDataBox for UserDataBoxView<'a> {
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

impl std::fmt::Debug for UserDataBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("UserDataBoxView")
            .field("box_size", &self.box_size())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of UserDataBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct UserDataBoxOwned {
    /// Typed child boxes.
    pub children: Vec<UserDataChild>,
}

impl UserDataBoxOwned {
    /// Creates a new empty UserDataBoxOwned.
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

    /// Returns an iterator over all CopyrightBox children.
    pub fn cprt_iter(&self) -> impl Iterator<Item = &CopyrightBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            UserDataChild::Cprt(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first CopyrightBox child, if present.
    pub fn cprt(&self) -> Option<&CopyrightBoxOwned> {
        self.cprt_iter().next()
    }

    /// Returns the first TrackSelectionBox child, if present.
    pub fn tsel(&self) -> Option<&TrackSelectionBoxOwned> {
        self.children.iter().find_map(|c| match c {
            UserDataChild::Tsel(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over all KindBox children.
    pub fn kind_iter(&self) -> impl Iterator<Item = &KindBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            UserDataChild::Kind(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the first KindBox child, if present.
    pub fn kind(&self) -> Option<&KindBoxOwned> {
        self.kind_iter().next()
    }

    /// Returns the first MetadataBox child, if present.
    pub fn meta(&self) -> Option<&MetadataBoxOwned> {
        self.children.iter().find_map(|c| match c {
            UserDataChild::Meta(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a CopyrightBox child.
    pub fn add_cprt(&mut self, b: CopyrightBoxOwned) {
        self.children.push(UserDataChild::Cprt(b));
    }

    /// Adds a TrackSelectionBox child.
    pub fn add_tsel(&mut self, b: TrackSelectionBoxOwned) {
        self.children.push(UserDataChild::Tsel(b));
    }

    /// Adds a KindBox child.
    pub fn add_kind(&mut self, b: KindBoxOwned) {
        self.children.push(UserDataChild::Kind(b));
    }

    /// Adds a MetadataBox child.
    pub fn add_meta(&mut self, b: MetadataBoxOwned) {
        self.children.push(UserDataChild::Meta(b));
    }
}

impl UserDataBox for UserDataBoxOwned {
    type Child<'a> = &'a UserDataChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &UserDataChild> {
        self.children.iter()
    }
}

impl<T: UserDataBox> From<&T> for UserDataBoxOwned {
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
    fn parse_empty_udta() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"udta");

        let view = UserDataBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"udta");

        let view = UserDataBoxView::new(&data).unwrap();
        let owned = UserDataBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
