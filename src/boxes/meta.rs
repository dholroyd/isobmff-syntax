//! Metadata Box (meta) parsing and serialization.
//!
//! The Metadata Box contains metadata for the presentation.
//! Note: meta is a FullBox container, unlike most container boxes.
//!
//! ```text
//! aligned(8) class MetaBox (handler_type)
//!    extends FullBox('meta', version = 0, 0) {
//!    HandlerBox(handler_type) theHandler;
//!    PrimaryItemBox primary_resource; // optional
//!    DataInformationBox file_locations; // optional
//!    ItemLocationBox item_locations; // optional
//!    ItemProtectionBox protections; // optional
//!    ItemInfoBox item_infos; // optional
//!    IPMPControlBox IPMP_control; // optional
//!    ItemReferenceBox item_refs; // optional
//!    ItemDataBox item_data; // optional
//!    Box other_boxes[]; // optional
//! }
//! ```

use crate::boxes::bxml::{self, BinaryXmlBox as _, BinaryXmlBoxOwned, BinaryXmlBoxView};
use crate::boxes::dinf::{self, DataInformationBox as _, DataInformationBoxOwned, DataInformationBoxView};
use crate::boxes::hdlr::{self, HandlerReferenceBox as _, HandlerReferenceBoxOwned, HandlerReferenceBoxView};
use crate::boxes::idat::{self, ItemDataBox as _, ItemDataBoxOwned, ItemDataBoxView};
use crate::boxes::iinf::{self, ItemInfoBox as _, ItemInfoBoxOwned, ItemInfoBoxView};
use crate::boxes::iloc::{self, ItemLocationBox as _, ItemLocationBoxOwned, ItemLocationBoxView};
use crate::boxes::ipro::{self, ItemProtectionBox as _, ItemProtectionBoxOwned, ItemProtectionBoxView};
use crate::boxes::iprp::{self, ItemPropertiesBox as _, ItemPropertiesBoxOwned, ItemPropertiesBoxView};
use crate::boxes::iref::{self, ItemReferenceBox as _, ItemReferenceBoxOwned, ItemReferenceBoxView};
use crate::boxes::pitm::{self, PrimaryItemBox as _, PrimaryItemBoxOwned, PrimaryItemBoxView};
use crate::boxes::xml::{self, XmlBox as _, XmlBoxOwned, XmlBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for MetadataBox.
pub const BOX_TYPE: BoxCode = BoxCode::META;

/// A typed child of a MetadataBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataChild {
    /// A HandlerReferenceBox child.
    Hdlr(HandlerReferenceBoxOwned),
    /// A DataInformationBox child.
    Dinf(DataInformationBoxOwned),
    /// An ItemLocationBox child.
    Iloc(ItemLocationBoxOwned),
    /// An ItemProtectionBox child.
    Ipro(ItemProtectionBoxOwned),
    /// An ItemInfoBox child.
    Iinf(ItemInfoBoxOwned),
    /// An ItemReferenceBox child.
    Iref(ItemReferenceBoxOwned),
    /// An ItemPropertiesBox child.
    Iprp(ItemPropertiesBoxOwned),
    /// An ItemDataBox child.
    Idat(ItemDataBoxOwned),
    /// A PrimaryItemBox child.
    Pitm(PrimaryItemBoxOwned),
    /// A BinaryXmlBox child.
    Bxml(BinaryXmlBoxOwned),
    /// An XmlBox child.
    Xml(XmlBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for MetadataChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Hdlr(_) => hdlr::BOX_TYPE,
            Self::Dinf(_) => dinf::BOX_TYPE,
            Self::Iloc(_) => iloc::BOX_TYPE,
            Self::Ipro(_) => ipro::BOX_TYPE,
            Self::Iinf(_) => iinf::BOX_TYPE,
            Self::Iref(_) => iref::BOX_TYPE,
            Self::Iprp(_) => iprp::BOX_TYPE,
            Self::Idat(_) => idat::BOX_TYPE,
            Self::Pitm(_) => pitm::BOX_TYPE,
            Self::Bxml(_) => bxml::BOX_TYPE,
            Self::Xml(_) => xml::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Hdlr(b) => b.box_size(),
            Self::Dinf(b) => b.box_size(),
            Self::Iloc(b) => b.box_size(),
            Self::Ipro(b) => b.box_size(),
            Self::Iinf(b) => b.box_size(),
            Self::Iref(b) => b.box_size(),
            Self::Iprp(b) => b.box_size(),
            Self::Idat(b) => b.box_size(),
            Self::Pitm(b) => b.box_size(),
            Self::Bxml(b) => b.box_size(),
            Self::Xml(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl MetadataChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Hdlr(b) => b.write_to(writer),
            Self::Dinf(b) => b.write_to(writer),
            Self::Iloc(b) => b.write_to(writer),
            Self::Ipro(b) => b.write_to(writer),
            Self::Iinf(b) => b.write_to(writer),
            Self::Iref(b) => b.write_to(writer),
            Self::Iprp(b) => b.write_to(writer),
            Self::Idat(b) => b.write_to(writer),
            Self::Pitm(b) => b.write_to(writer),
            Self::Bxml(b) => b.write_to(writer),
            Self::Xml(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for MetadataChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            hdlr::BOX_TYPE => match HandlerReferenceBoxView::new(raw.data()) {
                Ok(v) => Self::Hdlr(HandlerReferenceBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            dinf::BOX_TYPE => match DataInformationBoxView::new(raw.data()) {
                Ok(v) => Self::Dinf(DataInformationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            iloc::BOX_TYPE => match ItemLocationBoxView::new(raw.data())
                .and_then(|v| ItemLocationBoxOwned::try_from(&v))
            {
                Ok(owned) => Self::Iloc(owned),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            ipro::BOX_TYPE => match ItemProtectionBoxView::new(raw.data()) {
                Ok(v) => Self::Ipro(ItemProtectionBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            iinf::BOX_TYPE => match ItemInfoBoxView::new(raw.data()) {
                Ok(v) => Self::Iinf(ItemInfoBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            iref::BOX_TYPE => match ItemReferenceBoxView::new(raw.data()) {
                Ok(v) => Self::Iref(ItemReferenceBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            iprp::BOX_TYPE => match ItemPropertiesBoxView::new(raw.data()) {
                Ok(v) => Self::Iprp(ItemPropertiesBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            idat::BOX_TYPE => match ItemDataBoxView::new(raw.data()) {
                Ok(v) => Self::Idat(ItemDataBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            pitm::BOX_TYPE => match PrimaryItemBoxView::new(raw.data()) {
                Ok(v) => Self::Pitm(PrimaryItemBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            bxml::BOX_TYPE => match BinaryXmlBoxView::new(raw.data()) {
                Ok(v) => Self::Bxml(BinaryXmlBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            xml::BOX_TYPE => match XmlBoxView::new(raw.data()) {
                Ok(v) => Self::Xml(XmlBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&MetadataChild> for MetadataChild {
    fn from(source: &MetadataChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing MetadataBox data.
pub trait MetadataBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<MetadataChild>
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

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw MetadataBox bytes.
#[derive(Clone, Copy)]
pub struct MetadataBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> MetadataBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 0)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns an iterator over child boxes.
    #[inline]
    pub fn children(&self) -> BoxIterator<'a> {
        BoxIterator::new(&self.data[self.fullbox_offset + 4..])
    }

    /// Returns the HandlerReferenceBox child, if present.
    pub fn hdlr(&self) -> Option<HandlerReferenceBoxView<'a>> {
        self.children()
            .find_as::<HandlerReferenceBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the DataInformationBox child, if present.
    pub fn dinf(&self) -> Option<DataInformationBoxView<'a>> {
        self.children()
            .find_as::<DataInformationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ItemLocationBox child, if present.
    pub fn iloc(&self) -> Option<ItemLocationBoxView<'a>> {
        self.children()
            .find_as::<ItemLocationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ItemProtectionBox child, if present.
    pub fn ipro(&self) -> Option<ItemProtectionBoxView<'a>> {
        self.children()
            .find_as::<ItemProtectionBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ItemInfoBox child, if present.
    pub fn iinf(&self) -> Option<ItemInfoBoxView<'a>> {
        self.children()
            .find_as::<ItemInfoBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ItemReferenceBox child, if present.
    pub fn iref(&self) -> Option<ItemReferenceBoxView<'a>> {
        self.children()
            .find_as::<ItemReferenceBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ItemPropertiesBox child, if present.
    pub fn iprp(&self) -> Option<ItemPropertiesBoxView<'a>> {
        self.children()
            .find_as::<ItemPropertiesBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ItemDataBox child, if present.
    pub fn idat(&self) -> Option<ItemDataBoxView<'a>> {
        self.children()
            .find_as::<ItemDataBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the PrimaryItemBox child, if present.
    pub fn pitm(&self) -> Option<PrimaryItemBoxView<'a>> {
        self.children()
            .find_as::<PrimaryItemBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the BinaryXmlBox child, if present.
    pub fn bxml(&self) -> Option<BinaryXmlBoxView<'a>> {
        self.children()
            .find_as::<BinaryXmlBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the XmlBox child, if present.
    pub fn xml(&self) -> Option<XmlBoxView<'a>> {
        self.children()
            .find_as::<XmlBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> MetadataBox for MetadataBoxView<'a> {
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

    fn children(&self) -> impl Iterator<Item = RawBox<'_>> {
        self.children()
    }
}

impl std::fmt::Debug for MetadataBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let child_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("MetadataBoxView")
            .field("box_size", &self.box_size())
            .field("version", &self.version())
            .field("children", &child_types)
            .finish()
    }
}

/// An owned representation of MetadataBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct MetadataBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Typed child boxes.
    pub children: Vec<MetadataChild>,
}

impl MetadataBoxOwned {
    /// Creates a new empty MetadataBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let payload = self.children.iter().map(|c| c.box_size()).sum::<u64>();
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let size = self.serialized_size();
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;

        for child in &self.children {
            child.write_to(writer)?;
        }

        Ok(())
    }

    /// Returns the HandlerReferenceBox child, if present.
    pub fn hdlr(&self) -> Option<&HandlerReferenceBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Hdlr(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the DataInformationBox child, if present.
    pub fn dinf(&self) -> Option<&DataInformationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Dinf(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ItemLocationBox child, if present.
    pub fn iloc(&self) -> Option<&ItemLocationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Iloc(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ItemProtectionBox child, if present.
    pub fn ipro(&self) -> Option<&ItemProtectionBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Ipro(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ItemInfoBox child, if present.
    pub fn iinf(&self) -> Option<&ItemInfoBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Iinf(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ItemReferenceBox child, if present.
    pub fn iref(&self) -> Option<&ItemReferenceBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Iref(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ItemPropertiesBox child, if present.
    pub fn iprp(&self) -> Option<&ItemPropertiesBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Iprp(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ItemDataBox child, if present.
    pub fn idat(&self) -> Option<&ItemDataBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Idat(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the PrimaryItemBox child, if present.
    pub fn pitm(&self) -> Option<&PrimaryItemBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Pitm(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the BinaryXmlBox child, if present.
    pub fn bxml(&self) -> Option<&BinaryXmlBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Bxml(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the XmlBox child, if present.
    pub fn xml(&self) -> Option<&XmlBoxOwned> {
        self.children.iter().find_map(|c| match c {
            MetadataChild::Xml(b) => Some(b),
            _ => None,
        })
    }

    /// Adds a HandlerReferenceBox child.
    pub fn add_hdlr(&mut self, b: HandlerReferenceBoxOwned) {
        self.children.push(MetadataChild::Hdlr(b));
    }

    /// Adds a DataInformationBox child.
    pub fn add_dinf(&mut self, b: DataInformationBoxOwned) {
        self.children.push(MetadataChild::Dinf(b));
    }

    /// Adds an ItemLocationBox child.
    pub fn add_iloc(&mut self, b: ItemLocationBoxOwned) {
        self.children.push(MetadataChild::Iloc(b));
    }

    /// Adds an ItemProtectionBox child.
    pub fn add_ipro(&mut self, b: ItemProtectionBoxOwned) {
        self.children.push(MetadataChild::Ipro(b));
    }

    /// Adds an ItemInfoBox child.
    pub fn add_iinf(&mut self, b: ItemInfoBoxOwned) {
        self.children.push(MetadataChild::Iinf(b));
    }

    /// Adds an ItemReferenceBox child.
    pub fn add_iref(&mut self, b: ItemReferenceBoxOwned) {
        self.children.push(MetadataChild::Iref(b));
    }

    /// Adds an ItemPropertiesBox child.
    pub fn add_iprp(&mut self, b: ItemPropertiesBoxOwned) {
        self.children.push(MetadataChild::Iprp(b));
    }

    /// Adds an ItemDataBox child.
    pub fn add_idat(&mut self, b: ItemDataBoxOwned) {
        self.children.push(MetadataChild::Idat(b));
    }

    /// Adds a PrimaryItemBox child.
    pub fn add_pitm(&mut self, b: PrimaryItemBoxOwned) {
        self.children.push(MetadataChild::Pitm(b));
    }

    /// Adds a BinaryXmlBox child.
    pub fn add_bxml(&mut self, b: BinaryXmlBoxOwned) {
        self.children.push(MetadataChild::Bxml(b));
    }

    /// Adds an XmlBox child.
    pub fn add_xml(&mut self, b: XmlBoxOwned) {
        self.children.push(MetadataChild::Xml(b));
    }
}

impl MetadataBox for MetadataBoxOwned {
    type Child<'a> = &'a MetadataChild;

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

    fn children(&self) -> impl Iterator<Item = &MetadataChild> {
        self.children.iter()
    }
}

impl<T: MetadataBox> From<&T> for MetadataBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_meta() {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"meta");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags

        let view = MetadataBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 12);
        assert_eq!(view.version(), 0);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"meta");
        data.push(0);
        data.extend_from_slice(&[0, 0, 0]);

        let view = MetadataBoxView::new(&data).unwrap();
        let owned = MetadataBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
