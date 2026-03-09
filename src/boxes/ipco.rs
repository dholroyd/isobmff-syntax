//! Item Property Container Box (ipco) parsing and serialization.
//!
//! The Item Property Container Box contains a list of item properties.
//!
//! ```text
//! aligned(8) class ItemPropertyContainerBox
//!    extends Box('ipco') {
//!    Box properties[]; // boxes derived from
//!       // ItemProperty or ItemFullProperty, or FreeSpaceBox(es)
//!       // to fill the box
//! }
//! ```

use crate::boxes::av1c::{self, AV1CodecConfigurationBox as _, AV1CodecConfigurationBoxOwned, AV1CodecConfigurationBoxView};
use crate::boxes::auxc::{self, AuxiliaryTypePropertyBox as _, AuxiliaryTypePropertyBoxOwned, AuxiliaryTypePropertyBoxView};
use crate::boxes::avcc::{self, AVCConfigurationBox as _, AVCConfigurationBoxOwned, AVCConfigurationBoxView};
use crate::boxes::btrt::{self, BitrateBox as _, BitrateBoxOwned, BitrateBoxView};
use crate::boxes::ccst::{self, CodingConstraintsBox as _, CodingConstraintsBoxOwned, CodingConstraintsBoxView};
use crate::boxes::clap::{self, CleanApertureBox as _, CleanApertureBoxOwned, CleanApertureBoxView};
use crate::boxes::clli::{self, ContentLightLevelBox as _, ContentLightLevelBoxOwned, ContentLightLevelBoxView};
use crate::boxes::colr::{self, ColorInformationBox as _, ColorInformationBoxOwned, ColorInformationBoxView};
use crate::boxes::hvcc::{self, HEVCConfigurationBox as _, HEVCConfigurationBoxOwned, HEVCConfigurationBoxView};
use crate::boxes::imir::{self, ImageMirrorBox as _, ImageMirrorBoxOwned, ImageMirrorBoxView};
use crate::boxes::irot::{self, ImageRotationBox as _, ImageRotationBoxOwned, ImageRotationBoxView};
use crate::boxes::ispe::{self, ImageSpatialExtentsBox as _, ImageSpatialExtentsBoxOwned, ImageSpatialExtentsBoxView};
use crate::boxes::mdcv::{self, MasteringDisplayColourVolumeBox as _, MasteringDisplayColourVolumeBoxOwned, MasteringDisplayColourVolumeBoxView};
use crate::boxes::pasp::{self, PixelAspectRatioBox as _, PixelAspectRatioBoxOwned, PixelAspectRatioBoxView};
use crate::boxes::pixi::{self, PixelInformationBox as _, PixelInformationBoxOwned, PixelInformationBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for ItemPropertyContainerBox.
pub const BOX_TYPE: BoxCode = BoxCode::IPCO;

/// A typed child of an ItemPropertyContainerBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemPropertyContainerChild {
    /// An ImageSpatialExtentsBox child.
    Ispe(ImageSpatialExtentsBoxOwned),
    /// A ColorInformationBox child.
    Colr(ColorInformationBoxOwned),
    /// A PixelInformationBox child.
    Pixi(PixelInformationBoxOwned),
    /// An AuxiliaryTypePropertyBox child.
    Auxc(AuxiliaryTypePropertyBoxOwned),
    /// A CleanApertureBox child.
    Clap(CleanApertureBoxOwned),
    /// An ImageRotationBox child.
    Irot(ImageRotationBoxOwned),
    /// An ImageMirrorBox child.
    Imir(ImageMirrorBoxOwned),
    /// An AV1CodecConfigurationBox child.
    Av1c(AV1CodecConfigurationBoxOwned),
    /// A HEVCConfigurationBox child.
    Hvcc(HEVCConfigurationBoxOwned),
    /// An AVCConfigurationBox child.
    Avcc(AVCConfigurationBoxOwned),
    /// A BitrateBox child.
    Btrt(BitrateBoxOwned),
    /// A PixelAspectRatioBox child.
    Pasp(PixelAspectRatioBoxOwned),
    /// A ContentLightLevelBox child.
    Clli(ContentLightLevelBoxOwned),
    /// A MasteringDisplayColourVolumeBox child.
    Mdcv(MasteringDisplayColourVolumeBoxOwned),
    /// A CodingConstraintsBox child.
    Ccst(CodingConstraintsBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for ItemPropertyContainerChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Ispe(_) => ispe::BOX_TYPE,
            Self::Colr(_) => colr::BOX_TYPE,
            Self::Pixi(_) => pixi::BOX_TYPE,
            Self::Auxc(_) => auxc::BOX_TYPE,
            Self::Clap(_) => clap::BOX_TYPE,
            Self::Irot(_) => irot::BOX_TYPE,
            Self::Imir(_) => imir::BOX_TYPE,
            Self::Av1c(_) => av1c::BOX_TYPE,
            Self::Hvcc(_) => hvcc::BOX_TYPE,
            Self::Avcc(_) => avcc::BOX_TYPE,
            Self::Btrt(_) => btrt::BOX_TYPE,
            Self::Pasp(_) => pasp::BOX_TYPE,
            Self::Clli(_) => clli::BOX_TYPE,
            Self::Mdcv(_) => mdcv::BOX_TYPE,
            Self::Ccst(_) => ccst::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Ispe(b) => b.box_size(),
            Self::Colr(b) => b.box_size(),
            Self::Pixi(b) => b.box_size(),
            Self::Auxc(b) => b.box_size(),
            Self::Clap(b) => b.box_size(),
            Self::Irot(b) => b.box_size(),
            Self::Imir(b) => b.box_size(),
            Self::Av1c(b) => b.box_size(),
            Self::Hvcc(b) => b.box_size(),
            Self::Avcc(b) => b.box_size(),
            Self::Btrt(b) => b.box_size(),
            Self::Pasp(b) => b.box_size(),
            Self::Clli(b) => b.box_size(),
            Self::Mdcv(b) => b.box_size(),
            Self::Ccst(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl ItemPropertyContainerChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Ispe(b) => b.write_to(writer),
            Self::Colr(b) => b.write_to(writer),
            Self::Pixi(b) => b.write_to(writer),
            Self::Auxc(b) => b.write_to(writer),
            Self::Clap(b) => b.write_to(writer),
            Self::Irot(b) => b.write_to(writer),
            Self::Imir(b) => b.write_to(writer),
            Self::Av1c(b) => b.write_to(writer),
            Self::Hvcc(b) => b.write_to(writer),
            Self::Avcc(b) => b.write_to(writer),
            Self::Btrt(b) => b.write_to(writer),
            Self::Pasp(b) => b.write_to(writer),
            Self::Clli(b) => b.write_to(writer),
            Self::Mdcv(b) => b.write_to(writer),
            Self::Ccst(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for ItemPropertyContainerChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            ispe::BOX_TYPE => match ImageSpatialExtentsBoxView::new(raw.data()) {
                Ok(v) => Self::Ispe(ImageSpatialExtentsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            colr::BOX_TYPE => match ColorInformationBoxView::new(raw.data()) {
                Ok(v) => Self::Colr(ColorInformationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            pixi::BOX_TYPE => match PixelInformationBoxView::new(raw.data()) {
                Ok(v) => Self::Pixi(PixelInformationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            auxc::BOX_TYPE => match AuxiliaryTypePropertyBoxView::new(raw.data()) {
                Ok(v) => Self::Auxc(AuxiliaryTypePropertyBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            clap::BOX_TYPE => match CleanApertureBoxView::new(raw.data()) {
                Ok(v) => Self::Clap(CleanApertureBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            irot::BOX_TYPE => match ImageRotationBoxView::new(raw.data()) {
                Ok(v) => Self::Irot(ImageRotationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            imir::BOX_TYPE => match ImageMirrorBoxView::new(raw.data()) {
                Ok(v) => Self::Imir(ImageMirrorBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            av1c::BOX_TYPE => match AV1CodecConfigurationBoxView::new(raw.data()) {
                Ok(v) => Self::Av1c(AV1CodecConfigurationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            hvcc::BOX_TYPE => match HEVCConfigurationBoxView::new(raw.data()) {
                Ok(v) => Self::Hvcc(HEVCConfigurationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            avcc::BOX_TYPE => match AVCConfigurationBoxView::new(raw.data()) {
                Ok(v) => match AVCConfigurationBoxOwned::try_from(&v) {
                    Ok(owned) => Self::Avcc(owned),
                    Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
                },
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            btrt::BOX_TYPE => match BitrateBoxView::new(raw.data()) {
                Ok(v) => Self::Btrt(BitrateBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            pasp::BOX_TYPE => match PixelAspectRatioBoxView::new(raw.data()) {
                Ok(v) => Self::Pasp(PixelAspectRatioBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            clli::BOX_TYPE => match ContentLightLevelBoxView::new(raw.data()) {
                Ok(v) => Self::Clli(ContentLightLevelBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            mdcv::BOX_TYPE => match MasteringDisplayColourVolumeBoxView::new(raw.data()) {
                Ok(v) => Self::Mdcv(MasteringDisplayColourVolumeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            ccst::BOX_TYPE => match CodingConstraintsBoxView::new(raw.data()) {
                Ok(v) => Self::Ccst(CodingConstraintsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&ItemPropertyContainerChild> for ItemPropertyContainerChild {
    fn from(source: &ItemPropertyContainerChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing ItemPropertyContainerBox data.
pub trait ItemPropertyContainerBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<ItemPropertyContainerChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw ItemPropertyContainerBox bytes.
#[derive(Clone, Copy)]
pub struct ItemPropertyContainerBoxView<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> ItemPropertyContainerBoxView<'a> {
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

    /// Returns the number of properties.
    pub fn property_count(&self) -> usize {
        self.children().count()
    }

    /// Returns an iterator over ImageSpatialExtentsBox children.
    pub fn ispe_boxes(&self) -> impl Iterator<Item = ImageSpatialExtentsBoxView<'a>> {
        self.children()
            .filter_as::<ImageSpatialExtentsBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over ColorInformationBox children.
    pub fn colr_boxes(&self) -> impl Iterator<Item = ColorInformationBoxView<'a>> {
        self.children()
            .filter_as::<ColorInformationBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over PixelInformationBox children.
    pub fn pixi_boxes(&self) -> impl Iterator<Item = PixelInformationBoxView<'a>> {
        self.children()
            .filter_as::<PixelInformationBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over AuxiliaryTypePropertyBox children.
    pub fn auxc_boxes(&self) -> impl Iterator<Item = AuxiliaryTypePropertyBoxView<'a>> {
        self.children()
            .filter_as::<AuxiliaryTypePropertyBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over CleanApertureBox children.
    pub fn clap_boxes(&self) -> impl Iterator<Item = CleanApertureBoxView<'a>> {
        self.children()
            .filter_as::<CleanApertureBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over ImageRotationBox children.
    pub fn irot_boxes(&self) -> impl Iterator<Item = ImageRotationBoxView<'a>> {
        self.children()
            .filter_as::<ImageRotationBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over ImageMirrorBox children.
    pub fn imir_boxes(&self) -> impl Iterator<Item = ImageMirrorBoxView<'a>> {
        self.children()
            .filter_as::<ImageMirrorBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over AV1CodecConfigurationBox children.
    pub fn av1c_boxes(&self) -> impl Iterator<Item = AV1CodecConfigurationBoxView<'a>> {
        self.children()
            .filter_as::<AV1CodecConfigurationBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over HEVCConfigurationBox children.
    pub fn hvcc_boxes(&self) -> impl Iterator<Item = HEVCConfigurationBoxView<'a>> {
        self.children()
            .filter_as::<HEVCConfigurationBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over AVCConfigurationBox children.
    pub fn avcc_boxes(&self) -> impl Iterator<Item = AVCConfigurationBoxView<'a>> {
        self.children()
            .filter_as::<AVCConfigurationBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over BitrateBox children.
    pub fn btrt_boxes(&self) -> impl Iterator<Item = BitrateBoxView<'a>> {
        self.children()
            .filter_as::<BitrateBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over PixelAspectRatioBox children.
    pub fn pasp_boxes(&self) -> impl Iterator<Item = PixelAspectRatioBoxView<'a>> {
        self.children()
            .filter_as::<PixelAspectRatioBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over ContentLightLevelBox children.
    pub fn clli_boxes(&self) -> impl Iterator<Item = ContentLightLevelBoxView<'a>> {
        self.children()
            .filter_as::<ContentLightLevelBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over MasteringDisplayColourVolumeBox children.
    pub fn mdcv_boxes(&self) -> impl Iterator<Item = MasteringDisplayColourVolumeBoxView<'a>> {
        self.children()
            .filter_as::<MasteringDisplayColourVolumeBoxView>()
            .filter_map(Result::ok)
    }

    /// Returns an iterator over CodingConstraintsBox children.
    pub fn ccst_boxes(&self) -> impl Iterator<Item = CodingConstraintsBoxView<'a>> {
        self.children()
            .filter_as::<CodingConstraintsBoxView>()
            .filter_map(Result::ok)
    }
}

impl<'a> ItemPropertyContainerBox for ItemPropertyContainerBoxView<'a> {
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

impl std::fmt::Debug for ItemPropertyContainerBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prop_types: Vec<_> = self
            .children()
            .map(|c| String::from_utf8_lossy(&c.header().box_type_bytes()).to_string())
            .collect();
        f.debug_struct("ItemPropertyContainerBoxView")
            .field("box_size", &self.box_size())
            .field("properties", &prop_types)
            .finish()
    }
}

/// An owned representation of ItemPropertyContainerBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ItemPropertyContainerBoxOwned {
    /// Typed child boxes.
    pub children: Vec<ItemPropertyContainerChild>,
}

impl ItemPropertyContainerBoxOwned {
    /// Creates a new empty ItemPropertyContainerBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of properties.
    pub fn property_count(&self) -> usize {
        self.children.len()
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

    /// Returns an iterator over ImageSpatialExtentsBox children.
    pub fn ispe_boxes(&self) -> impl Iterator<Item = &ImageSpatialExtentsBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Ispe(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over ColorInformationBox children.
    pub fn colr_boxes(&self) -> impl Iterator<Item = &ColorInformationBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Colr(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over PixelInformationBox children.
    pub fn pixi_boxes(&self) -> impl Iterator<Item = &PixelInformationBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Pixi(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over AuxiliaryTypePropertyBox children.
    pub fn auxc_boxes(&self) -> impl Iterator<Item = &AuxiliaryTypePropertyBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Auxc(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over CleanApertureBox children.
    pub fn clap_boxes(&self) -> impl Iterator<Item = &CleanApertureBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Clap(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over ImageRotationBox children.
    pub fn irot_boxes(&self) -> impl Iterator<Item = &ImageRotationBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Irot(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over ImageMirrorBox children.
    pub fn imir_boxes(&self) -> impl Iterator<Item = &ImageMirrorBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Imir(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over AV1CodecConfigurationBox children.
    pub fn av1c_boxes(&self) -> impl Iterator<Item = &AV1CodecConfigurationBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Av1c(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over HEVCConfigurationBox children.
    pub fn hvcc_boxes(&self) -> impl Iterator<Item = &HEVCConfigurationBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Hvcc(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over AVCConfigurationBox children.
    pub fn avcc_boxes(&self) -> impl Iterator<Item = &AVCConfigurationBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Avcc(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over BitrateBox children.
    pub fn btrt_boxes(&self) -> impl Iterator<Item = &BitrateBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Btrt(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over PixelAspectRatioBox children.
    pub fn pasp_boxes(&self) -> impl Iterator<Item = &PixelAspectRatioBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Pasp(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over ContentLightLevelBox children.
    pub fn clli_boxes(&self) -> impl Iterator<Item = &ContentLightLevelBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Clli(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over MasteringDisplayColourVolumeBox children.
    pub fn mdcv_boxes(&self) -> impl Iterator<Item = &MasteringDisplayColourVolumeBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Mdcv(b) => Some(b),
            _ => None,
        })
    }

    /// Returns an iterator over CodingConstraintsBox children.
    pub fn ccst_boxes(&self) -> impl Iterator<Item = &CodingConstraintsBoxOwned> {
        self.children.iter().filter_map(|c| match c {
            ItemPropertyContainerChild::Ccst(b) => Some(b),
            _ => None,
        })
    }

    /// Adds an ImageSpatialExtentsBox child.
    pub fn add_ispe(&mut self, b: ImageSpatialExtentsBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Ispe(b));
    }

    /// Adds a ColorInformationBox child.
    pub fn add_colr(&mut self, b: ColorInformationBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Colr(b));
    }

    /// Adds a PixelInformationBox child.
    pub fn add_pixi(&mut self, b: PixelInformationBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Pixi(b));
    }

    /// Adds an AuxiliaryTypePropertyBox child.
    pub fn add_auxc(&mut self, b: AuxiliaryTypePropertyBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Auxc(b));
    }

    /// Adds a CleanApertureBox child.
    pub fn add_clap(&mut self, b: CleanApertureBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Clap(b));
    }

    /// Adds an ImageRotationBox child.
    pub fn add_irot(&mut self, b: ImageRotationBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Irot(b));
    }

    /// Adds an ImageMirrorBox child.
    pub fn add_imir(&mut self, b: ImageMirrorBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Imir(b));
    }

    /// Adds an AV1CodecConfigurationBox child.
    pub fn add_av1c(&mut self, b: AV1CodecConfigurationBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Av1c(b));
    }

    /// Adds a HEVCConfigurationBox child.
    pub fn add_hvcc(&mut self, b: HEVCConfigurationBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Hvcc(b));
    }

    /// Adds an AVCConfigurationBox child.
    pub fn add_avcc(&mut self, b: AVCConfigurationBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Avcc(b));
    }

    /// Adds a BitrateBox child.
    pub fn add_btrt(&mut self, b: BitrateBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Btrt(b));
    }

    /// Adds a PixelAspectRatioBox child.
    pub fn add_pasp(&mut self, b: PixelAspectRatioBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Pasp(b));
    }

    /// Adds a ContentLightLevelBox child.
    pub fn add_clli(&mut self, b: ContentLightLevelBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Clli(b));
    }

    /// Adds a MasteringDisplayColourVolumeBox child.
    pub fn add_mdcv(&mut self, b: MasteringDisplayColourVolumeBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Mdcv(b));
    }

    /// Adds a CodingConstraintsBox child.
    pub fn add_ccst(&mut self, b: CodingConstraintsBoxOwned) {
        self.children.push(ItemPropertyContainerChild::Ccst(b));
    }
}

impl ItemPropertyContainerBox for ItemPropertyContainerBoxOwned {
    type Child<'a> = &'a ItemPropertyContainerChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &ItemPropertyContainerChild> {
        self.children.iter()
    }
}

impl<T: ItemPropertyContainerBox> From<&T> for ItemPropertyContainerBoxOwned {
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
    fn parse_empty_ipco() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"ipco");

        let view = ItemPropertyContainerBoxView::new(&data).unwrap();
        assert_eq!(view.box_size(), 8);
        assert_eq!(view.property_count(), 0);
    }

    #[test]
    fn roundtrip() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"ipco");

        let view = ItemPropertyContainerBoxView::new(&data).unwrap();
        let owned = ItemPropertyContainerBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
