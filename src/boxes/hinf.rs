//! Hint Information Box (hinf) parsing and serialization.
//!
//! The Hint Information Box is a container for hint track statistics.
//!
//! ```text
//! aligned(8) class hintstatisticsbox extends Box('hinf') {
//! }
//! ```

use crate::boxes::dimm::{self, ImmediateDataSizeBox as _, ImmediateDataSizeBoxOwned, ImmediateDataSizeBoxView};
use crate::boxes::dmed::{self, MediaDurationBox as _, MediaDurationBoxOwned, MediaDurationBoxView};
use crate::boxes::drep::{self, RepeatedDataSizeBox as _, RepeatedDataSizeBoxOwned, RepeatedDataSizeBoxView};
use crate::boxes::maxr::{self, MaxDataRateBox as _, MaxDataRateBoxOwned, MaxDataRateBoxView};
use crate::boxes::npck::{self, NumPacketsBox as _, NumPacketsBoxOwned, NumPacketsBoxView};
use crate::boxes::nump::{self, NumRTPPacketsBox as _, NumRTPPacketsBoxOwned, NumRTPPacketsBoxView};
use crate::boxes::pmax::{self, LargestPacketSizeBox as _, LargestPacketSizeBoxOwned, LargestPacketSizeBoxView};
use crate::boxes::tmax::{self, LargestRelativeTimeBox as _, LargestRelativeTimeBoxOwned, LargestRelativeTimeBoxView};
use crate::boxes::tmin::{self, SmallestRelativeTimeBox as _, SmallestRelativeTimeBoxOwned, SmallestRelativeTimeBoxView};
use crate::boxes::totl::{self, TotalMediaBytesBox as _, TotalMediaBytesBoxOwned, TotalMediaBytesBoxView};
use crate::boxes::tpyl::{self, TotalRTPBytesBox as _, TotalRTPBytesBoxOwned, TotalRTPBytesBoxView};
use crate::boxes::trpy::{self, TotalRTPBytesWithHeaderBox as _, TotalRTPBytesWithHeaderBoxOwned, TotalRTPBytesWithHeaderBoxView};
use crate::container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox};
use crate::error::ParseError;
use crate::header::{BoxHeader, header_size_for_payload, write_box_header};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for HintInfoBox.
pub const BOX_TYPE: BoxCode = BoxCode::HINF;

/// A typed child of a HintInfoBox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HintInfoChild {
    /// A TotalMediaBytesBox child.
    Totl(TotalMediaBytesBoxOwned),
    /// A NumRTPPacketsBox child.
    Nump(NumRTPPacketsBoxOwned),
    /// A NumPacketsBox child.
    Npck(NumPacketsBoxOwned),
    /// A TotalRTPBytesBox child.
    Tpyl(TotalRTPBytesBoxOwned),
    /// A TotalRTPBytesWithHeaderBox child.
    Trpy(TotalRTPBytesWithHeaderBoxOwned),
    /// A MaxDataRateBox child.
    Maxr(MaxDataRateBoxOwned),
    /// A MediaDurationBox child.
    Dmed(MediaDurationBoxOwned),
    /// An ImmediateDataSizeBox child.
    Dimm(ImmediateDataSizeBoxOwned),
    /// A RepeatedDataSizeBox child.
    Drep(RepeatedDataSizeBoxOwned),
    /// A SmallestRelativeTimeBox child.
    Tmin(SmallestRelativeTimeBoxOwned),
    /// A LargestRelativeTimeBox child.
    Tmax(LargestRelativeTimeBoxOwned),
    /// A LargestPacketSizeBox child.
    Pmax(LargestPacketSizeBoxOwned),
    /// An unknown or unrecognized child box.
    Other(OpaqueBoxOwned),
}

impl ChildBox for HintInfoChild {
    fn box_type(&self) -> BoxCode {
        match self {
            Self::Totl(_) => totl::BOX_TYPE,
            Self::Nump(_) => nump::BOX_TYPE,
            Self::Npck(_) => npck::BOX_TYPE,
            Self::Tpyl(_) => tpyl::BOX_TYPE,
            Self::Trpy(_) => trpy::BOX_TYPE,
            Self::Maxr(_) => maxr::BOX_TYPE,
            Self::Dmed(_) => dmed::BOX_TYPE,
            Self::Dimm(_) => dimm::BOX_TYPE,
            Self::Drep(_) => drep::BOX_TYPE,
            Self::Tmin(_) => tmin::BOX_TYPE,
            Self::Tmax(_) => tmax::BOX_TYPE,
            Self::Pmax(_) => pmax::BOX_TYPE,
            Self::Other(o) => o.box_type(),
        }
    }

    fn box_size(&self) -> u64 {
        match self {
            Self::Totl(b) => b.box_size(),
            Self::Nump(b) => b.box_size(),
            Self::Npck(b) => b.box_size(),
            Self::Tpyl(b) => b.box_size(),
            Self::Trpy(b) => b.box_size(),
            Self::Maxr(b) => b.box_size(),
            Self::Dmed(b) => b.box_size(),
            Self::Dimm(b) => b.box_size(),
            Self::Drep(b) => b.box_size(),
            Self::Tmin(b) => b.box_size(),
            Self::Tmax(b) => b.box_size(),
            Self::Pmax(b) => b.box_size(),
            Self::Other(o) => o.box_size(),
        }
    }
}

impl HintInfoChild {
    /// Writes this child box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Totl(b) => b.write_to(writer),
            Self::Nump(b) => b.write_to(writer),
            Self::Npck(b) => b.write_to(writer),
            Self::Tpyl(b) => b.write_to(writer),
            Self::Trpy(b) => b.write_to(writer),
            Self::Maxr(b) => b.write_to(writer),
            Self::Dmed(b) => b.write_to(writer),
            Self::Dimm(b) => b.write_to(writer),
            Self::Drep(b) => b.write_to(writer),
            Self::Tmin(b) => b.write_to(writer),
            Self::Tmax(b) => b.write_to(writer),
            Self::Pmax(b) => b.write_to(writer),
            Self::Other(o) => o.write_to(writer),
        }
    }
}

impl From<RawBox<'_>> for HintInfoChild {
    fn from(raw: RawBox<'_>) -> Self {
        match raw.box_type() {
            totl::BOX_TYPE => match TotalMediaBytesBoxView::new(raw.data()) {
                Ok(v) => Self::Totl(TotalMediaBytesBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            nump::BOX_TYPE => match NumRTPPacketsBoxView::new(raw.data()) {
                Ok(v) => Self::Nump(NumRTPPacketsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            npck::BOX_TYPE => match NumPacketsBoxView::new(raw.data()) {
                Ok(v) => Self::Npck(NumPacketsBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            tpyl::BOX_TYPE => match TotalRTPBytesBoxView::new(raw.data()) {
                Ok(v) => Self::Tpyl(TotalRTPBytesBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            trpy::BOX_TYPE => match TotalRTPBytesWithHeaderBoxView::new(raw.data()) {
                Ok(v) => Self::Trpy(TotalRTPBytesWithHeaderBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            maxr::BOX_TYPE => match MaxDataRateBoxView::new(raw.data()) {
                Ok(v) => Self::Maxr(MaxDataRateBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            dmed::BOX_TYPE => match MediaDurationBoxView::new(raw.data()) {
                Ok(v) => Self::Dmed(MediaDurationBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            dimm::BOX_TYPE => match ImmediateDataSizeBoxView::new(raw.data()) {
                Ok(v) => Self::Dimm(ImmediateDataSizeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            drep::BOX_TYPE => match RepeatedDataSizeBoxView::new(raw.data()) {
                Ok(v) => Self::Drep(RepeatedDataSizeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            tmin::BOX_TYPE => match SmallestRelativeTimeBoxView::new(raw.data()) {
                Ok(v) => Self::Tmin(SmallestRelativeTimeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            tmax::BOX_TYPE => match LargestRelativeTimeBoxView::new(raw.data()) {
                Ok(v) => Self::Tmax(LargestRelativeTimeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            pmax::BOX_TYPE => match LargestPacketSizeBoxView::new(raw.data()) {
                Ok(v) => Self::Pmax(LargestPacketSizeBoxOwned::from(&v)),
                Err(_) => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
            },
            _ => Self::Other(OpaqueBoxOwned::from_raw_box(&raw)),
        }
    }
}

impl From<&HintInfoChild> for HintInfoChild {
    fn from(source: &HintInfoChild) -> Self {
        source.clone()
    }
}

/// Common interface for accessing HintInfoBox data.
pub trait HintInfoBox {
    /// The type of child items yielded by the children iterator.
    type Child<'a>: ChildBox + Into<HintInfoChild>
    where
        Self: 'a;

    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns an iterator over child boxes.
    fn children(&self) -> impl Iterator<Item = Self::Child<'_>>;
}

/// A borrowing view over raw HintInfoBox bytes.
#[derive(Clone, Copy)]
pub struct HintInfoBoxView<'a> {
    data: &'a [u8],
    header_size: usize,
}

impl<'a> HintInfoBoxView<'a> {
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

    /// Returns the TotalMediaBytesBox child, if present.
    pub fn totl(&self) -> Option<TotalMediaBytesBoxView<'a>> {
        self.children()
            .find_as::<TotalMediaBytesBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the NumRTPPacketsBox child, if present.
    pub fn nump(&self) -> Option<NumRTPPacketsBoxView<'a>> {
        self.children()
            .find_as::<NumRTPPacketsBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the NumPacketsBox child, if present.
    pub fn npck(&self) -> Option<NumPacketsBoxView<'a>> {
        self.children()
            .find_as::<NumPacketsBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the TotalRTPBytesBox child, if present.
    pub fn tpyl(&self) -> Option<TotalRTPBytesBoxView<'a>> {
        self.children()
            .find_as::<TotalRTPBytesBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the TotalRTPBytesWithHeaderBox child, if present.
    pub fn trpy(&self) -> Option<TotalRTPBytesWithHeaderBoxView<'a>> {
        self.children()
            .find_as::<TotalRTPBytesWithHeaderBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the MaxDataRateBox child, if present.
    pub fn maxr(&self) -> Option<MaxDataRateBoxView<'a>> {
        self.children()
            .find_as::<MaxDataRateBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the MediaDurationBox child, if present.
    pub fn dmed(&self) -> Option<MediaDurationBoxView<'a>> {
        self.children()
            .find_as::<MediaDurationBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the ImmediateDataSizeBox child, if present.
    pub fn dimm(&self) -> Option<ImmediateDataSizeBoxView<'a>> {
        self.children()
            .find_as::<ImmediateDataSizeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the RepeatedDataSizeBox child, if present.
    pub fn drep(&self) -> Option<RepeatedDataSizeBoxView<'a>> {
        self.children()
            .find_as::<RepeatedDataSizeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the SmallestRelativeTimeBox child, if present.
    pub fn tmin(&self) -> Option<SmallestRelativeTimeBoxView<'a>> {
        self.children()
            .find_as::<SmallestRelativeTimeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the LargestRelativeTimeBox child, if present.
    pub fn tmax(&self) -> Option<LargestRelativeTimeBoxView<'a>> {
        self.children()
            .find_as::<LargestRelativeTimeBoxView>()
            .and_then(Result::ok)
    }

    /// Returns the LargestPacketSizeBox child, if present.
    pub fn pmax(&self) -> Option<LargestPacketSizeBoxView<'a>> {
        self.children()
            .find_as::<LargestPacketSizeBoxView>()
            .and_then(Result::ok)
    }
}

impl<'a> HintInfoBox for HintInfoBoxView<'a> {
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

impl std::fmt::Debug for HintInfoBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HintInfoBoxView")
            .field("children_count", &self.children().count())
            .finish()
    }
}

/// An owned representation of HintInfoBox data.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HintInfoBoxOwned {
    /// Typed child boxes.
    pub children: Vec<HintInfoChild>,
}

impl HintInfoBoxOwned {
    /// Creates a new HintInfoBoxOwned.
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

    /// Returns the TotalMediaBytesBox child, if present.
    pub fn totl(&self) -> Option<&TotalMediaBytesBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Totl(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the NumRTPPacketsBox child, if present.
    pub fn nump(&self) -> Option<&NumRTPPacketsBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Nump(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the NumPacketsBox child, if present.
    pub fn npck(&self) -> Option<&NumPacketsBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Npck(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the TotalRTPBytesBox child, if present.
    pub fn tpyl(&self) -> Option<&TotalRTPBytesBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Tpyl(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the TotalRTPBytesWithHeaderBox child, if present.
    pub fn trpy(&self) -> Option<&TotalRTPBytesWithHeaderBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Trpy(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the MaxDataRateBox child, if present.
    pub fn maxr(&self) -> Option<&MaxDataRateBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Maxr(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the MediaDurationBox child, if present.
    pub fn dmed(&self) -> Option<&MediaDurationBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Dmed(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the ImmediateDataSizeBox child, if present.
    pub fn dimm(&self) -> Option<&ImmediateDataSizeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Dimm(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the RepeatedDataSizeBox child, if present.
    pub fn drep(&self) -> Option<&RepeatedDataSizeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Drep(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the SmallestRelativeTimeBox child, if present.
    pub fn tmin(&self) -> Option<&SmallestRelativeTimeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Tmin(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the LargestRelativeTimeBox child, if present.
    pub fn tmax(&self) -> Option<&LargestRelativeTimeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Tmax(b) => Some(b),
            _ => None,
        })
    }

    /// Returns the LargestPacketSizeBox child, if present.
    pub fn pmax(&self) -> Option<&LargestPacketSizeBoxOwned> {
        self.children.iter().find_map(|c| match c {
            HintInfoChild::Pmax(b) => Some(b),
            _ => None,
        })
    }
}

impl HintInfoBox for HintInfoBoxOwned {
    type Child<'a> = &'a HintInfoChild;

    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn children(&self) -> impl Iterator<Item = &HintInfoChild> {
        self.children.iter()
    }
}

impl<T: HintInfoBox> From<&T> for HintInfoBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            children: source.children().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::totl::TotalMediaBytesBox;
    use crate::boxes::maxr::MaxDataRateBox;

    fn make_hinf() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"hinf");
        data
    }

    fn make_totl_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes()); // 8 + 4
        data.extend_from_slice(b"totl");
        data.extend_from_slice(&5000u32.to_be_bytes());
        data
    }

    fn make_maxr_bytes() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 4 + 4
        data.extend_from_slice(b"maxr");
        data.extend_from_slice(&1000u32.to_be_bytes()); // granularity
        data.extend_from_slice(&128000u32.to_be_bytes()); // max_data_rate
        data
    }

    fn make_unknown_box() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&12u32.to_be_bytes());
        data.extend_from_slice(b"xyzw");
        data.extend_from_slice(&[0xAB, 0xCD, 0xEF, 0x01]);
        data
    }

    fn make_hinf_with_children(children: &[&[u8]]) -> Vec<u8> {
        let children_len: usize = children.iter().map(|c| c.len()).sum();
        let size = 8 + children_len;
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(b"hinf");
        for child in children {
            data.extend_from_slice(child);
        }
        data
    }

    #[test]
    fn parse_empty_hinf() {
        let data = make_hinf();
        let view = HintInfoBoxView::new(&data).unwrap();
        assert_eq!(view.children().count(), 0);
    }

    #[test]
    fn roundtrip() {
        let data = make_hinf();
        let view = HintInfoBoxView::new(&data).unwrap();
        let owned = HintInfoBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_children() {
        let totl = make_totl_bytes();
        let maxr = make_maxr_bytes();
        let data = make_hinf_with_children(&[&totl, &maxr]);

        let view = HintInfoBoxView::new(&data).unwrap();
        assert!(view.totl().is_some());
        assert_eq!(view.totl().unwrap().total_bytes(), 5000);
        assert!(view.maxr().is_some());
        assert_eq!(view.maxr().unwrap().period(), 1000);

        let owned = HintInfoBoxOwned::from(&view);
        assert!(owned.totl().is_some());
        assert_eq!(owned.totl().unwrap().total_bytes, 5000);
        assert!(owned.maxr().is_some());
        assert_eq!(owned.maxr().unwrap().period, 1000);
        assert_eq!(owned.maxr().unwrap().bytes, 128000);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_unknown_children() {
        let totl = make_totl_bytes();
        let unknown = make_unknown_box();
        let data = make_hinf_with_children(&[&totl, &unknown]);

        let view = HintInfoBoxView::new(&data).unwrap();
        let owned = HintInfoBoxOwned::from(&view);

        // Known child parsed
        assert!(owned.totl().is_some());
        // Total children includes unknown
        assert_eq!(owned.children.len(), 2);
        assert!(matches!(owned.children[1], HintInfoChild::Other(_)));

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();
        assert_eq!(data, output);
    }

    #[test]
    fn view_convenience_methods() {
        let totl = make_totl_bytes();
        let unknown = make_unknown_box();
        let data = make_hinf_with_children(&[&unknown, &totl]);

        let view = HintInfoBoxView::new(&data).unwrap();
        // totl finds the totl box even after an unknown box
        assert!(view.totl().is_some());
        assert_eq!(view.totl().unwrap().total_bytes(), 5000);
        // nump is not present
        assert!(view.nump().is_none());
    }

    #[test]
    fn child_box_type() {
        let totl = make_totl_bytes();
        let unknown = make_unknown_box();
        let data = make_hinf_with_children(&[&totl, &unknown]);

        let view = HintInfoBoxView::new(&data).unwrap();
        let owned = HintInfoBoxOwned::from(&view);

        assert_eq!(ChildBox::box_type(&owned.children[0]), BoxCode::new(*b"totl"));
        assert_eq!(
            ChildBox::box_type(&owned.children[1]),
            BoxCode::new(*b"xyzw")
        );
    }
}
