//! Box type definitions for ISOBMFF structures.

/// Implement [`TypedBoxView`](crate::container::TypedBoxView) for the box list
/// supplied by [`for_each_box_view!`]. Each module must expose a `pub const
/// BOX_TYPE`. The owned type is not used here, only matched away.
macro_rules! impl_typed_box_view {
    ( $( $module:ident :: { $View:ident, $Owned:ident } ),* $(,)? ) => {
        $(
            impl<'a> crate::container::TypedBoxView<'a> for $module::$View<'a> {
                const BOX_TYPE: mp4ra_rust::BoxCode = $module::BOX_TYPE;

                fn from_raw_box(data: &'a [u8]) -> Result<Self, crate::error::ParseError> {
                    Self::new(data)
                }
            }
        )*
    };
}

// Container boxes
pub mod dinf;
pub mod edts;
pub mod grpl;
pub mod iprp;
pub mod ipco;
pub mod mdia;
pub mod meta;
pub mod mfra;
pub mod minf;
pub mod moof;
pub mod moov;
pub mod mvex;
pub mod paen;
pub mod stbl;
pub mod strd;
pub mod strk;
pub mod tapt;
pub mod traf;
pub mod trak;
pub mod trep;
pub mod tref;
pub mod trgr;
pub mod udta;

// Full boxes - headers
pub mod dref;
pub mod hdlr;
pub mod hmhd;
pub mod mdhd;
pub mod mvhd;
pub mod nmhd;
pub mod smhd;
pub mod sthd;
pub mod tkhd;
pub mod vmhd;

// Sample entry boxes
pub mod sample_entry;

// Full boxes - sample table
pub mod co64;
pub mod csgp;
pub mod ctts;
pub mod padb;
pub mod saio;
pub mod saiz;
pub mod sbgp;
pub mod sdtp;
pub mod sgpd;
pub mod stco;
pub mod stri;
pub mod stsc;
pub mod stsd;
pub mod stdp;
pub mod stsh;
pub mod stsg;
pub mod stsl;
pub mod stss;
pub mod stsz;
pub mod stvi;
pub mod stz2;
pub mod stts;
pub mod subs;

// SVC/MVC embedded boxes (used by sgpd scif/mvif entries)
pub mod ldep;
pub mod svdr;
pub mod svpr;
pub mod tiri;
pub mod vwid;

// Full boxes - edit list and timing
pub mod cslg;
pub mod elst;

// Full boxes - movie fragments
pub mod leva;
pub mod mehd;
pub mod mfhd;
pub mod mfro;
pub mod tfdt;
pub mod tfhd;
pub mod tfra;
pub mod trex;
pub mod trun;

// Full boxes - segment indexing
pub mod prft;
pub mod sidx;
pub mod ssix;
pub mod styp;

// Protection boxes
pub mod frma;
pub mod pssh;
pub mod rinf;
pub mod schi;
pub mod schm;
pub mod senc;
pub mod sinf;
pub mod tenc;

// File delivery boxes
pub mod fecr;
pub mod gitn;
pub mod segr;

// Metadata boxes
pub mod bxml;
pub mod cprt;
pub mod elng;
pub mod fiin;
pub mod idat;
pub mod iinf;
pub mod iloc;
pub mod infe;
pub mod ipma;
pub mod ipro;
pub mod iref;
pub mod kind;
pub mod mime;
pub mod pitm;
pub mod tsel;
pub mod uri;
pub mod urii;
pub mod xml;

// Image property boxes (HEIF/AVIF)
pub mod auxc;
pub mod btrt;
pub mod clap;
pub mod clli;
pub mod colr;
pub mod imir;
pub mod irot;
pub mod ispe;
pub mod lsel;
pub mod mdcv;
pub mod pasp;
pub mod pixi;
pub mod rloc;

// Codec configuration boxes
pub mod a1lx;
pub mod a1op;
pub mod av1c;
pub mod avcc;
pub mod ccst;
pub mod dfla;
pub mod dops;
pub mod esds;
pub mod hvcc;
pub mod oinf;
pub mod vpcc;

// Entity grouping boxes
pub mod altr;
pub mod ster;

// Additional file delivery boxes
pub mod fire;

// Camera and depth boxes
pub mod cmex;
pub mod cmin;

// Additional codec/image boxes
pub mod icef;
pub mod jpeg;

// Item reference type boxes
pub mod cdsc;
pub mod dimg;
pub mod thmb;

// Timing and hint boxes
pub mod dimm;
pub mod dmed;
pub mod drep;
pub mod hinf;
pub mod hnti;
pub mod maxr;
pub mod npck;
pub mod nump;
pub mod payt;
pub mod pmax;
pub mod sdp;
pub mod snro;
pub mod srat;
pub mod srpp;
pub mod tims;
pub mod tmax;
pub mod tmin;
pub mod totl;
pub mod tpyl;
pub mod trpo;
pub mod trpy;
pub mod tsro;

// Audio boxes
pub mod alou;
pub mod chnl;
pub mod loudness;
pub mod ludt;
pub mod tlou;

// Track type boxes
pub mod ttyp;

// Plain boxes
pub mod free;
pub mod ftyp;
pub mod mdat;
pub mod pdin;
pub mod uuid;

// Re-exports - Container boxes
pub use dinf::{DataInformationBox, DataInformationBoxOwned, DataInformationBoxView};
pub use grpl::{
    EntityGroupEntryBox, EntityGroupEntryBoxOwned, EntityGroupEntryBoxView,
    EntityToGroupBox, EntityToGroupBoxOwned, EntityToGroupBoxView,
};
pub use mdia::{MediaBox, MediaBoxOwned, MediaBoxView};
pub use minf::{MediaInformationBox, MediaInformationBoxOwned, MediaInformationBoxView};
pub use moov::{MovieBox, MovieBoxOwned, MovieBoxView};
pub use paen::{PartitionEntryBox, PartitionEntryBoxOwned, PartitionEntryBoxView};
pub use stbl::{SampleTableBox, SampleTableBoxOwned, SampleTableBoxView};
pub use strd::{
    SubTrackDefinitionBox, SubTrackDefinitionBoxOwned, SubTrackDefinitionBoxView,
    SubTrackDefinitionChild,
};
pub use strk::{SubTrackBox, SubTrackBoxOwned, SubTrackBoxView, SubTrackChild};
pub use tapt::{TrackApertureBox, TrackApertureBoxOwned, TrackApertureBoxView};
pub use trak::{TrackBox, TrackBoxOwned, TrackBoxView};
pub use trep::{TrackExtensionPropertiesBox, TrackExtensionPropertiesBoxOwned, TrackExtensionPropertiesBoxView};
pub use trgr::{
    TrackGroupBox, TrackGroupBoxOwned, TrackGroupBoxView,
    TrackGroupTypeBox, TrackGroupTypeBoxOwned, TrackGroupTypeBoxView,
};

// Re-exports - Header boxes
pub use dref::{DataReferenceBox, DataReferenceBoxOwned, DataReferenceBoxView, DataReferenceChild};
pub use hdlr::{HandlerReferenceBox, HandlerReferenceBoxOwned, HandlerReferenceBoxView};
pub use hmhd::{HintMediaHeaderBox, HintMediaHeaderBoxOwned, HintMediaHeaderBoxView};
pub use mdhd::{MediaHeaderBox, MediaHeaderBoxOwned, MediaHeaderBoxView};
pub use mvhd::{MovieHeaderBox, MovieHeaderBoxOwned, MovieHeaderBoxView};
pub use nmhd::{NullMediaHeaderBox, NullMediaHeaderBoxOwned, NullMediaHeaderBoxView};
pub use smhd::{SoundMediaHeaderBox, SoundMediaHeaderBoxOwned, SoundMediaHeaderBoxView};
pub use tkhd::{TrackHeaderBox, TrackHeaderBoxOwned, TrackHeaderBoxView};
pub use vmhd::{VideoMediaHeaderBox, VideoMediaHeaderBoxOwned, VideoMediaHeaderBoxView};
pub use sthd::{SubtitleMediaHeaderBox, SubtitleMediaHeaderBoxOwned, SubtitleMediaHeaderBoxView};

// Re-exports - Sample table boxes
pub use co64::{ChunkLargeOffsetBox, ChunkLargeOffsetBoxOwned, ChunkLargeOffsetBoxView};
pub use ctts::{
    CompositionOffsetEntry, CompositionTimeToSampleBox, CompositionTimeToSampleBoxOwned,
    CompositionTimeToSampleBoxView,
};
pub use saio::{
    SampleAuxiliaryInformationOffsetsBox, SampleAuxiliaryInformationOffsetsBoxOwned,
    SampleAuxiliaryInformationOffsetsBoxView,
};
pub use saiz::{
    SampleAuxiliaryInformationSizesBox, SampleAuxiliaryInformationSizesBoxOwned,
    SampleAuxiliaryInformationSizesBoxView, SampleInfoSizes,
};
pub use stco::{ChunkOffsetBox, ChunkOffsetBoxOwned, ChunkOffsetBoxView};
pub use stsc::{SampleToChunkBox, SampleToChunkBoxOwned, SampleToChunkBoxView, SampleToChunkEntry};
pub use stsd::{SampleDescriptionBox, SampleDescriptionBoxOwned, SampleDescriptionBoxView, SampleDescriptionChild};
pub use sample_entry::{
    AudioSampleEntry, AudioSampleEntryOwned, AudioSampleEntryView,
    SampleEntry, SampleEntryKind, SampleEntryView,
    VisualSampleEntry, VisualSampleEntryOwned, VisualSampleEntryView,
    AUDIO_HEADER_SIZE, BASE_HEADER_SIZE, VISUAL_HEADER_SIZE,
};
pub use stsh::{ShadowSyncSampleBox, ShadowSyncSampleBoxOwned, ShadowSyncSampleBoxView, ShadowSyncEntry};
pub use stss::{SyncSampleBox, SyncSampleBoxOwned, SyncSampleBoxView};
pub use stsz::{SampleSizeBox, SampleSizeBoxOwned, SampleSizeBoxView};
pub use stz2::{CompactSampleSizeBox, CompactSampleSizeBoxOwned, CompactSampleSizeBoxView};
pub use stts::{TimeToSampleBox, TimeToSampleBoxOwned, TimeToSampleBoxView, TimeToSampleEntry};
pub use stsl::{SampleScaleBox, SampleScaleBoxOwned, SampleScaleBoxView};
pub use stvi::{StereoVideoBox, StereoVideoBoxOwned, StereoVideoBoxView};

// Re-exports - Plain boxes
pub use free::{FreeSpaceBox, FreeSpaceBoxOwned, FreeSpaceBoxView};
pub use ftyp::{FileTypeBox, FileTypeBoxOwned, FileTypeBoxView};
pub use mdat::{MediaDataBox, MediaDataBoxOwned, MediaDataBoxRef, MediaDataBoxView};
pub use pdin::{
    ProgressiveDownloadEntry, ProgressiveDownloadInfoBox, ProgressiveDownloadInfoBoxOwned,
    ProgressiveDownloadInfoBoxView,
};

// Re-exports - Edit list boxes
pub use edts::{EditBox, EditBoxOwned, EditBoxView};
pub use elst::{EditListBox, EditListBoxOwned, EditListBoxView, EditListEntry};

// Re-exports - Movie fragment boxes
pub use leva::{LevelAssignmentBox, LevelAssignmentBoxOwned, LevelAssignmentBoxView, LevelAssignmentEntry};
pub use mehd::{MovieExtendsHeaderBox, MovieExtendsHeaderBoxOwned, MovieExtendsHeaderBoxView};
pub use mfhd::{MovieFragmentHeaderBox, MovieFragmentHeaderBoxOwned, MovieFragmentHeaderBoxView};
pub use moof::{MovieFragmentBox, MovieFragmentBoxOwned, MovieFragmentBoxView};
pub use mvex::{MovieExtendsBox, MovieExtendsBoxOwned, MovieExtendsBoxView};
pub use tfdt::{
    TrackFragmentBaseMediaDecodeTimeBox, TrackFragmentBaseMediaDecodeTimeBoxOwned,
    TrackFragmentBaseMediaDecodeTimeBoxView,
};
pub use tfhd::{TrackFragmentHeaderBox, TrackFragmentHeaderBoxOwned, TrackFragmentHeaderBoxView};
pub use traf::{TrackFragmentBox, TrackFragmentBoxOwned, TrackFragmentBoxView};
pub use trex::{TrackExtendsBox, TrackExtendsBoxOwned, TrackExtendsBoxView};
pub use trun::{TrackRunBox, TrackRunBoxOwned, TrackRunBoxView, TrackRunSample};

// Re-exports - Container boxes (additional)
pub use meta::{MetadataBox, MetadataBoxOwned, MetadataBoxView};
pub use udta::{UserDataBox, UserDataBoxOwned, UserDataBoxView};

// Re-exports - Sample group boxes
pub use csgp::{CompactSampleToGroupBox, CompactSampleToGroupBoxOwned, CompactSampleToGroupBoxView, CompactSampleToGroupEntry, CompactSampleToGroupEntryView};
pub use padb::{PaddingBitsBox, PaddingBitsBoxOwned, PaddingBitsBoxView};
pub use sbgp::{SampleToGroupBox, SampleToGroupBoxOwned, SampleToGroupBoxView, SampleToGroupEntry};
pub use sdtp::{
    SampleDependencyFlags, SampleDependencyTypeBox, SampleDependencyTypeBoxOwned,
    SampleDependencyTypeBoxView,
};
pub use sgpd::{SampleGroupDescriptionBox, SampleGroupDescriptionBoxOwned, SampleGroupDescriptionBoxView, SampleGroupEntry};
pub use ldep::{TierDependencyBox, TierDependencyBoxOwned, TierDependencyBoxView};
pub use svdr::{SvcDependencyRangeBox, SvcDependencyRangeBoxOwned, SvcDependencyRangeBoxView};
pub use svpr::{PriorityRangeBox, PriorityRangeBoxOwned, PriorityRangeBoxView};
pub use tiri::{TierInfoBox, TierInfoBoxOwned, TierInfoBoxView};
pub use vwid::{
    ViewEntry, ViewEntryOwned, ViewEntryView, ViewIdentifierBox, ViewIdentifierBoxOwned,
    ViewIdentifierBoxView,
};
pub use stdp::{DegradationPriorityBox, DegradationPriorityBoxOwned, DegradationPriorityBoxView};
pub use stri::{SubTrackInformationBox, SubTrackInformationBoxOwned, SubTrackInformationBoxView};
pub use stsg::{SubTrackSampleGroupBox, SubTrackSampleGroupBoxOwned, SubTrackSampleGroupBoxView};
pub use subs::{
    SampleSubSampleEntry, SampleSubSampleEntryOwned, SampleSubSampleEntryView, SubSampleEntry,
    SubSampleInformationBox, SubSampleInformationBoxOwned, SubSampleInformationBoxView,
};

// Re-exports - Segment indexing boxes
pub use prft::{ProducerReferenceTimeBox, ProducerReferenceTimeBoxOwned, ProducerReferenceTimeBoxView};
pub use sidx::{SegmentIndexBox, SegmentIndexBoxOwned, SegmentIndexBoxView, SegmentIndexReference};
pub use ssix::{
    SubsegmentEntry, SubsegmentEntryOwned, SubsegmentEntryView, SubsegmentIndexBox,
    SubsegmentIndexBoxOwned, SubsegmentIndexBoxView, SubsegmentRange,
};
pub use styp::{SegmentTypeBox, SegmentTypeBoxOwned, SegmentTypeBoxView};

// Re-exports - Protection boxes
pub use frma::{OriginalFormatBox, OriginalFormatBoxOwned, OriginalFormatBoxView};
pub use pssh::{
    ProtectionSystemSpecificHeaderBox, ProtectionSystemSpecificHeaderBoxOwned,
    ProtectionSystemSpecificHeaderBoxView,
};
pub use rinf::{RestrictedSchemeInfoBox, RestrictedSchemeInfoBoxOwned, RestrictedSchemeInfoBoxView, RestrictedSchemeInfoChild};
pub use schi::{SchemeInformationBox, SchemeInformationBoxOwned, SchemeInformationBoxView};
pub use schm::{SchemeTypeBox, SchemeTypeBoxOwned, SchemeTypeBoxView};
pub use senc::{SampleEncryptionBox, SampleEncryptionBoxOwned, SampleEncryptionBoxView, SampleEncryptionEntry, SampleEncryptionEntryView, SubsampleEncryptionEntry};
pub use sinf::{ProtectionSchemeInfoBox, ProtectionSchemeInfoBoxOwned, ProtectionSchemeInfoBoxView};
pub use tenc::{TrackEncryptionBox, TrackEncryptionBoxOwned, TrackEncryptionBoxView};

// Re-exports - Edit list and timing
pub use cslg::{CompositionToDecodeBox, CompositionToDecodeBoxOwned, CompositionToDecodeBoxView};

// Re-exports - Movie fragment random access
pub use mfra::{
    MovieFragmentRandomAccessBox, MovieFragmentRandomAccessBoxOwned, MovieFragmentRandomAccessBoxView,
};
pub use mfro::{
    MovieFragmentRandomAccessOffsetBox, MovieFragmentRandomAccessOffsetBoxOwned,
    MovieFragmentRandomAccessOffsetBoxView,
};
pub use tfra::{
    RandomAccessEntry, TrackFragmentRandomAccessBox, TrackFragmentRandomAccessBoxOwned,
    TrackFragmentRandomAccessBoxView,
};

// Re-exports - File delivery boxes
pub use fecr::{FECReservoirBox, FECReservoirBoxOwned, FECReservoirBoxView, FECReservoirEntry};
pub use gitn::{
    GroupIdToName, GroupIdToNameBox, GroupIdToNameBoxOwned, GroupIdToNameBoxView,
    GroupIdToNameOwned, GroupIdToNameView,
};
pub use segr::{FDSessionGroupBox, FDSessionGroupBoxOwned, FDSessionGroupBoxView, SessionGroupEntry};

// Re-exports - Metadata boxes
pub use bxml::{BinaryXmlBox, BinaryXmlBoxOwned, BinaryXmlBoxView};
pub use cprt::{CopyrightBox, CopyrightBoxOwned, CopyrightBoxView};
pub use elng::{ExtendedLanguageTagBox, ExtendedLanguageTagBoxOwned, ExtendedLanguageTagBoxView};
pub use fiin::{FDItemInformationBox, FDItemInformationBoxOwned, FDItemInformationBoxView};
pub use idat::{ItemDataBox, ItemDataBoxOwned, ItemDataBoxView};
pub use iinf::{ItemInfoBox, ItemInfoBoxOwned, ItemInfoBoxView};
pub use iloc::{
    ItemExtent, ItemLocation, ItemLocationBox, ItemLocationBoxOwned, ItemLocationBoxView,
    ItemLocationOwned, ItemLocationView,
};
pub use infe::{ItemInfoEntryBox, ItemInfoEntryBoxOwned, ItemInfoEntryBoxView};
pub use ipco::{ItemPropertyContainerBox, ItemPropertyContainerBoxOwned, ItemPropertyContainerBoxView};
pub use ipma::{
    ItemPropertyAssociation, ItemPropertyAssociationBox, ItemPropertyAssociationBoxOwned,
    ItemPropertyAssociationBoxView, ItemPropertyAssociationOwned, ItemPropertyAssociationView,
    PropertyAssociation,
};
pub use iprp::{ItemPropertiesBox, ItemPropertiesBoxOwned, ItemPropertiesBoxView};
pub use ipro::{ItemProtectionBox, ItemProtectionBoxOwned, ItemProtectionBoxView, ItemProtectionChild};
pub use iref::{ItemReferenceBox, ItemReferenceBoxOwned, ItemReferenceBoxView};
pub use pitm::{PrimaryItemBox, PrimaryItemBoxOwned, PrimaryItemBoxView};
pub use tref::{
    TrackReferenceBox, TrackReferenceBoxOwned, TrackReferenceBoxView, TrackReferenceTypeBox,
    TrackReferenceTypeBoxOwned, TrackReferenceTypeBoxView,
};
pub use tsel::{TrackSelectionBox, TrackSelectionBoxOwned, TrackSelectionBoxView};
pub use xml::{XmlBox, XmlBoxOwned, XmlBoxView};
pub use kind::{KindBox, KindBoxOwned, KindBoxView};
pub use mime::{MIMEBox, MIMEBoxOwned, MIMEBoxView};
pub use uri::{URIBox, URIBoxOwned, URIBoxView};
pub use urii::{URIInitBox, URIInitBoxOwned, URIInitBoxView};

// Re-exports - Extension boxes
pub use uuid::{UserExtensionBox, UserExtensionBoxOwned, UserExtensionBoxView};

// Re-exports - Image property boxes (HEIF/AVIF)
pub use auxc::{AuxiliaryTypePropertyBox, AuxiliaryTypePropertyBoxOwned, AuxiliaryTypePropertyBoxView};
pub use btrt::{BitrateBox, BitrateBoxOwned, BitrateBoxView};
pub use clap::{CleanApertureBox, CleanApertureBoxOwned, CleanApertureBoxView};
pub use clli::{ContentLightLevelBox, ContentLightLevelBoxOwned, ContentLightLevelBoxView};
pub use colr::{ColorInformationBox, ColorInformationBoxOwned, ColorInformationBoxView, ColorData, ColorInfo};
pub use imir::{ImageMirrorBox, ImageMirrorBoxOwned, ImageMirrorBoxView};
pub use irot::{ImageRotationBox, ImageRotationBoxOwned, ImageRotationBoxView};
pub use ispe::{ImageSpatialExtentsBox, ImageSpatialExtentsBoxOwned, ImageSpatialExtentsBoxView};
pub use lsel::{LayerSelectionBox, LayerSelectionBoxOwned, LayerSelectionBoxView};
pub use mdcv::{
    MasteringDisplayColourVolumeBox, MasteringDisplayColourVolumeBoxOwned,
    MasteringDisplayColourVolumeBoxView,
};
pub use pasp::{PixelAspectRatioBox, PixelAspectRatioBoxOwned, PixelAspectRatioBoxView};
pub use pixi::{PixelInformationBox, PixelInformationBoxOwned, PixelInformationBoxView};
pub use rloc::{RelativeLocationBox, RelativeLocationBoxOwned, RelativeLocationBoxView};

// Re-exports - Codec configuration boxes
pub use a1lx::{AV1LayerConfigurationBox, AV1LayerConfigurationBoxOwned, AV1LayerConfigurationBoxView};
pub use a1op::{
    AV1OperatingPointSelectorBox, AV1OperatingPointSelectorBoxOwned,
    AV1OperatingPointSelectorBoxView,
};
pub use av1c::{AV1CodecConfigurationBox, AV1CodecConfigurationBoxOwned, AV1CodecConfigurationBoxView};
pub use avcc::{AVCConfigurationBox, AVCConfigurationBoxOwned, AVCConfigurationBoxView};
pub use ccst::{CodingConstraintsBox, CodingConstraintsBoxOwned, CodingConstraintsBoxView};
pub use dfla::{FLACSpecificBox, FLACSpecificBoxOwned, FLACSpecificBoxView};
pub use dops::{OpusSpecificBox, OpusSpecificBoxOwned, OpusSpecificBoxView};
pub use esds::{ESDescriptorBox, ESDescriptorBoxOwned, ESDescriptorBoxView};
pub use hvcc::{HEVCConfigurationBox, HEVCConfigurationBoxOwned, HEVCConfigurationBoxView};
pub use oinf::{
    OperatingPointsInformationBox, OperatingPointsInformationBoxOwned,
    OperatingPointsInformationBoxView,
};
pub use vpcc::{VPCodecConfigurationBox, VPCodecConfigurationBoxOwned, VPCodecConfigurationBoxView};

// Re-exports - Entity grouping boxes
pub use altr::{AlternativeEntityGroupBox, AlternativeEntityGroupBoxOwned, AlternativeEntityGroupBoxView};
pub use ster::{StereoEntityGroupBox, StereoEntityGroupBoxOwned, StereoEntityGroupBoxView};

// Re-exports - Additional file delivery boxes
pub use fire::{FileReservoirBox, FileReservoirBoxOwned, FileReservoirBoxView, FileReservoirEntry};

// Re-exports - Camera and depth boxes
pub use cmex::{CameraExtrinsicMatrixBox, CameraExtrinsicMatrixBoxOwned, CameraExtrinsicMatrixBoxView};
pub use cmin::{CameraIntrinsicMatrixBox, CameraIntrinsicMatrixBoxOwned, CameraIntrinsicMatrixBoxView};

// Re-exports - Additional codec/image boxes
pub use icef::{ImageExtentFrameBox, ImageExtentFrameBoxOwned, ImageExtentFrameBoxView};
pub use jpeg::{JPEGConfigurationBox, JPEGConfigurationBoxOwned, JPEGConfigurationBoxView};

// Re-exports - Item reference type boxes
pub use cdsc::{
    ContentDescriptionReferenceBox, ContentDescriptionReferenceBoxOwned,
    ContentDescriptionReferenceBoxView,
};
pub use dimg::{DerivedImageReferenceBox, DerivedImageReferenceBoxOwned, DerivedImageReferenceBoxView};
pub use thmb::{ThumbnailReferenceBox, ThumbnailReferenceBoxOwned, ThumbnailReferenceBoxView};

// Re-exports - Timing and hint boxes
pub use dimm::{ImmediateDataSizeBox, ImmediateDataSizeBoxOwned, ImmediateDataSizeBoxView};
pub use dmed::{MediaDurationBox, MediaDurationBoxOwned, MediaDurationBoxView};
pub use hnti::{HintTrackInfoBox, HintTrackInfoBoxOwned, HintTrackInfoBoxView, HintTrackInfoChild};
pub use maxr::{MaxDataRateBox, MaxDataRateBoxOwned, MaxDataRateBoxView};
pub use npck::{NumPacketsBox, NumPacketsBoxOwned, NumPacketsBoxView};
pub use nump::{NumRTPPacketsBox, NumRTPPacketsBoxOwned, NumRTPPacketsBoxView};
pub use srat::{SamplingRateBox, SamplingRateBoxOwned, SamplingRateBoxView};
pub use srpp::{SRTPProcessBox, SRTPProcessBoxOwned, SRTPProcessBoxView, SRTPProcessChild};
pub use tims::{TimeScaleEntryBox, TimeScaleEntryBoxOwned, TimeScaleEntryBoxView};
pub use totl::{TotalMediaBytesBox, TotalMediaBytesBoxOwned, TotalMediaBytesBoxView};
pub use tpyl::{TotalRTPBytesBox, TotalRTPBytesBoxOwned, TotalRTPBytesBoxView};
pub use trpy::{TotalRTPBytesWithHeaderBox, TotalRTPBytesWithHeaderBoxOwned, TotalRTPBytesWithHeaderBoxView};
pub use tsro::{TimeOffsetBox, TimeOffsetBoxOwned, TimeOffsetBoxView};
pub use drep::{RepeatedDataSizeBox, RepeatedDataSizeBoxOwned, RepeatedDataSizeBoxView};
pub use payt::{PayloadTypeBox, PayloadTypeBoxOwned, PayloadTypeBoxView};
pub use pmax::{LargestPacketSizeBox, LargestPacketSizeBoxOwned, LargestPacketSizeBoxView};
pub use tmax::{LargestRelativeTimeBox, LargestRelativeTimeBoxOwned, LargestRelativeTimeBoxView};
pub use tmin::{SmallestRelativeTimeBox, SmallestRelativeTimeBoxOwned, SmallestRelativeTimeBoxView};
pub use hinf::{HintInfoBox, HintInfoBoxOwned, HintInfoBoxView, HintInfoChild};
pub use sdp::{SDPBox, SDPBoxOwned, SDPBoxView};
pub use snro::{SequenceOffsetBox, SequenceOffsetBoxOwned, SequenceOffsetBoxView};
pub use trpo::{TimestampOffsetBox, TimestampOffsetBoxOwned, TimestampOffsetBoxView};

// Re-exports - Track type boxes
pub use ttyp::{TrackTypeBox, TrackTypeBoxOwned, TrackTypeBoxView};

// Re-exports - Audio boxes
pub use alou::{AlbumLoudnessInfoBox, AlbumLoudnessInfoBoxOwned, AlbumLoudnessInfoBoxView};
pub use chnl::{ChannelLayoutBox, ChannelLayoutBoxOwned, ChannelLayoutBoxView, ChannelLayoutInfo, SpeakerPosition};
pub use ludt::{LoudnessBaseBox, LoudnessBaseBoxOwned, LoudnessBaseBoxView, LoudnessBaseChild};
pub use tlou::{TrackLoudnessInfoBox, TrackLoudnessInfoBoxOwned, TrackLoudnessInfoBoxView};

// TypedBoxView implementations for all concrete box view types.
//
// Excluded:
// - free::FreeSpaceBoxView (accepts both FREE and SKIP box types)
// - sample_entry::{SampleEntryView, VisualSampleEntryView, AudioSampleEntryView} (base types, no single BOX_TYPE)
// - trgr::TrackGroupTypeBoxView (generic, box type varies)
// - tref::TrackReferenceTypeBoxView (generic, box type varies)
// - grpl::EntityGroupEntryBoxView (generic, box type varies)
/// Invokes `$callback!` with the canonical list of every box type that has a
/// [`TypedBoxView`](crate::container::TypedBoxView) implementation, as
/// `module::{ViewType, OwnedType}` pairs.
///
/// The callback receives a comma-separated list in the form
///
/// ```text
/// ftyp::{FileTypeBoxView, FileTypeBoxOwned},
/// moov::{MovieBoxView, MovieBoxOwned},
/// ...
/// ```
///
/// and is expanded at the call site, so the emitted paths are resolved relative
/// to the caller: name the types as `isobmff_syntax::boxes::$module::$Type`.
/// This lets external crates (fuzz harnesses, linters, dumpers) generate
/// exhaustive per-box-type dispatch without maintaining their own copy of the
/// list, which would silently go stale as box types are added. Where only a few
/// box types are of interest, [`dispatch_box!`](crate::dispatch_box) is the
/// simpler choice, since it takes the types to match as arguments.
#[macro_export]
macro_rules! for_each_box_view {
    ( $callback:ident ) => {
        $callback! {
            // Container boxes
            dinf::{DataInformationBoxView, DataInformationBoxOwned},
            edts::{EditBoxView, EditBoxOwned},
            grpl::{EntityToGroupBoxView, EntityToGroupBoxOwned},
            iprp::{ItemPropertiesBoxView, ItemPropertiesBoxOwned},
            ipco::{ItemPropertyContainerBoxView, ItemPropertyContainerBoxOwned},
            mdia::{MediaBoxView, MediaBoxOwned},
            meta::{MetadataBoxView, MetadataBoxOwned},
            mfra::{MovieFragmentRandomAccessBoxView, MovieFragmentRandomAccessBoxOwned},
            minf::{MediaInformationBoxView, MediaInformationBoxOwned},
            moof::{MovieFragmentBoxView, MovieFragmentBoxOwned},
            moov::{MovieBoxView, MovieBoxOwned},
            mvex::{MovieExtendsBoxView, MovieExtendsBoxOwned},
            paen::{PartitionEntryBoxView, PartitionEntryBoxOwned},
            stbl::{SampleTableBoxView, SampleTableBoxOwned},
            strd::{SubTrackDefinitionBoxView, SubTrackDefinitionBoxOwned},
            strk::{SubTrackBoxView, SubTrackBoxOwned},
            tapt::{TrackApertureBoxView, TrackApertureBoxOwned},
            traf::{TrackFragmentBoxView, TrackFragmentBoxOwned},
            trak::{TrackBoxView, TrackBoxOwned},
            trep::{TrackExtensionPropertiesBoxView, TrackExtensionPropertiesBoxOwned},
            tref::{TrackReferenceBoxView, TrackReferenceBoxOwned},
            trgr::{TrackGroupBoxView, TrackGroupBoxOwned},
            udta::{UserDataBoxView, UserDataBoxOwned},
            // Header boxes
            dref::{DataReferenceBoxView, DataReferenceBoxOwned},
            hdlr::{HandlerReferenceBoxView, HandlerReferenceBoxOwned},
            hmhd::{HintMediaHeaderBoxView, HintMediaHeaderBoxOwned},
            mdhd::{MediaHeaderBoxView, MediaHeaderBoxOwned},
            mvhd::{MovieHeaderBoxView, MovieHeaderBoxOwned},
            nmhd::{NullMediaHeaderBoxView, NullMediaHeaderBoxOwned},
            smhd::{SoundMediaHeaderBoxView, SoundMediaHeaderBoxOwned},
            sthd::{SubtitleMediaHeaderBoxView, SubtitleMediaHeaderBoxOwned},
            tkhd::{TrackHeaderBoxView, TrackHeaderBoxOwned},
            vmhd::{VideoMediaHeaderBoxView, VideoMediaHeaderBoxOwned},
            // Sample table boxes
            co64::{ChunkLargeOffsetBoxView, ChunkLargeOffsetBoxOwned},
            csgp::{CompactSampleToGroupBoxView, CompactSampleToGroupBoxOwned},
            ctts::{CompositionTimeToSampleBoxView, CompositionTimeToSampleBoxOwned},
            padb::{PaddingBitsBoxView, PaddingBitsBoxOwned},
            saio::{SampleAuxiliaryInformationOffsetsBoxView, SampleAuxiliaryInformationOffsetsBoxOwned},
            saiz::{SampleAuxiliaryInformationSizesBoxView, SampleAuxiliaryInformationSizesBoxOwned},
            sbgp::{SampleToGroupBoxView, SampleToGroupBoxOwned},
            sdtp::{SampleDependencyTypeBoxView, SampleDependencyTypeBoxOwned},
            sgpd::{SampleGroupDescriptionBoxView, SampleGroupDescriptionBoxOwned},
            stco::{ChunkOffsetBoxView, ChunkOffsetBoxOwned},
            stri::{SubTrackInformationBoxView, SubTrackInformationBoxOwned},
            stsc::{SampleToChunkBoxView, SampleToChunkBoxOwned},
            stsd::{SampleDescriptionBoxView, SampleDescriptionBoxOwned},
            stdp::{DegradationPriorityBoxView, DegradationPriorityBoxOwned},
            stsh::{ShadowSyncSampleBoxView, ShadowSyncSampleBoxOwned},
            stsg::{SubTrackSampleGroupBoxView, SubTrackSampleGroupBoxOwned},
            stsl::{SampleScaleBoxView, SampleScaleBoxOwned},
            stss::{SyncSampleBoxView, SyncSampleBoxOwned},
            stsz::{SampleSizeBoxView, SampleSizeBoxOwned},
            stvi::{StereoVideoBoxView, StereoVideoBoxOwned},
            stz2::{CompactSampleSizeBoxView, CompactSampleSizeBoxOwned},
            stts::{TimeToSampleBoxView, TimeToSampleBoxOwned},
            subs::{SubSampleInformationBoxView, SubSampleInformationBoxOwned},
            // SVC/MVC boxes
            ldep::{TierDependencyBoxView, TierDependencyBoxOwned},
            svdr::{SvcDependencyRangeBoxView, SvcDependencyRangeBoxOwned},
            svpr::{PriorityRangeBoxView, PriorityRangeBoxOwned},
            tiri::{TierInfoBoxView, TierInfoBoxOwned},
            vwid::{ViewIdentifierBoxView, ViewIdentifierBoxOwned},
            // Edit list and timing
            cslg::{CompositionToDecodeBoxView, CompositionToDecodeBoxOwned},
            elst::{EditListBoxView, EditListBoxOwned},
            // Movie fragment boxes
            leva::{LevelAssignmentBoxView, LevelAssignmentBoxOwned},
            mehd::{MovieExtendsHeaderBoxView, MovieExtendsHeaderBoxOwned},
            mfhd::{MovieFragmentHeaderBoxView, MovieFragmentHeaderBoxOwned},
            mfro::{MovieFragmentRandomAccessOffsetBoxView, MovieFragmentRandomAccessOffsetBoxOwned},
            tfdt::{TrackFragmentBaseMediaDecodeTimeBoxView, TrackFragmentBaseMediaDecodeTimeBoxOwned},
            tfhd::{TrackFragmentHeaderBoxView, TrackFragmentHeaderBoxOwned},
            tfra::{TrackFragmentRandomAccessBoxView, TrackFragmentRandomAccessBoxOwned},
            trex::{TrackExtendsBoxView, TrackExtendsBoxOwned},
            trun::{TrackRunBoxView, TrackRunBoxOwned},
            // Segment indexing
            prft::{ProducerReferenceTimeBoxView, ProducerReferenceTimeBoxOwned},
            sidx::{SegmentIndexBoxView, SegmentIndexBoxOwned},
            ssix::{SubsegmentIndexBoxView, SubsegmentIndexBoxOwned},
            styp::{SegmentTypeBoxView, SegmentTypeBoxOwned},
            // Protection boxes
            frma::{OriginalFormatBoxView, OriginalFormatBoxOwned},
            pssh::{ProtectionSystemSpecificHeaderBoxView, ProtectionSystemSpecificHeaderBoxOwned},
            rinf::{RestrictedSchemeInfoBoxView, RestrictedSchemeInfoBoxOwned},
            schi::{SchemeInformationBoxView, SchemeInformationBoxOwned},
            schm::{SchemeTypeBoxView, SchemeTypeBoxOwned},
            senc::{SampleEncryptionBoxView, SampleEncryptionBoxOwned},
            sinf::{ProtectionSchemeInfoBoxView, ProtectionSchemeInfoBoxOwned},
            tenc::{TrackEncryptionBoxView, TrackEncryptionBoxOwned},
            // File delivery boxes
            fecr::{FECReservoirBoxView, FECReservoirBoxOwned},
            gitn::{GroupIdToNameBoxView, GroupIdToNameBoxOwned},
            segr::{FDSessionGroupBoxView, FDSessionGroupBoxOwned},
            // Metadata boxes
            bxml::{BinaryXmlBoxView, BinaryXmlBoxOwned},
            cprt::{CopyrightBoxView, CopyrightBoxOwned},
            elng::{ExtendedLanguageTagBoxView, ExtendedLanguageTagBoxOwned},
            fiin::{FDItemInformationBoxView, FDItemInformationBoxOwned},
            idat::{ItemDataBoxView, ItemDataBoxOwned},
            iinf::{ItemInfoBoxView, ItemInfoBoxOwned},
            iloc::{ItemLocationBoxView, ItemLocationBoxOwned},
            infe::{ItemInfoEntryBoxView, ItemInfoEntryBoxOwned},
            ipma::{ItemPropertyAssociationBoxView, ItemPropertyAssociationBoxOwned},
            ipro::{ItemProtectionBoxView, ItemProtectionBoxOwned},
            iref::{ItemReferenceBoxView, ItemReferenceBoxOwned},
            kind::{KindBoxView, KindBoxOwned},
            mime::{MIMEBoxView, MIMEBoxOwned},
            pitm::{PrimaryItemBoxView, PrimaryItemBoxOwned},
            tsel::{TrackSelectionBoxView, TrackSelectionBoxOwned},
            uri::{URIBoxView, URIBoxOwned},
            urii::{URIInitBoxView, URIInitBoxOwned},
            xml::{XmlBoxView, XmlBoxOwned},
            // Image property boxes
            auxc::{AuxiliaryTypePropertyBoxView, AuxiliaryTypePropertyBoxOwned},
            btrt::{BitrateBoxView, BitrateBoxOwned},
            clap::{CleanApertureBoxView, CleanApertureBoxOwned},
            clli::{ContentLightLevelBoxView, ContentLightLevelBoxOwned},
            colr::{ColorInformationBoxView, ColorInformationBoxOwned},
            imir::{ImageMirrorBoxView, ImageMirrorBoxOwned},
            irot::{ImageRotationBoxView, ImageRotationBoxOwned},
            ispe::{ImageSpatialExtentsBoxView, ImageSpatialExtentsBoxOwned},
            lsel::{LayerSelectionBoxView, LayerSelectionBoxOwned},
            mdcv::{MasteringDisplayColourVolumeBoxView, MasteringDisplayColourVolumeBoxOwned},
            pasp::{PixelAspectRatioBoxView, PixelAspectRatioBoxOwned},
            pixi::{PixelInformationBoxView, PixelInformationBoxOwned},
            rloc::{RelativeLocationBoxView, RelativeLocationBoxOwned},
            // Codec configuration boxes
            a1lx::{AV1LayerConfigurationBoxView, AV1LayerConfigurationBoxOwned},
            a1op::{AV1OperatingPointSelectorBoxView, AV1OperatingPointSelectorBoxOwned},
            av1c::{AV1CodecConfigurationBoxView, AV1CodecConfigurationBoxOwned},
            avcc::{AVCConfigurationBoxView, AVCConfigurationBoxOwned},
            ccst::{CodingConstraintsBoxView, CodingConstraintsBoxOwned},
            dfla::{FLACSpecificBoxView, FLACSpecificBoxOwned},
            dops::{OpusSpecificBoxView, OpusSpecificBoxOwned},
            esds::{ESDescriptorBoxView, ESDescriptorBoxOwned},
            hvcc::{HEVCConfigurationBoxView, HEVCConfigurationBoxOwned},
            oinf::{OperatingPointsInformationBoxView, OperatingPointsInformationBoxOwned},
            vpcc::{VPCodecConfigurationBoxView, VPCodecConfigurationBoxOwned},
            // Entity grouping boxes
            altr::{AlternativeEntityGroupBoxView, AlternativeEntityGroupBoxOwned},
            ster::{StereoEntityGroupBoxView, StereoEntityGroupBoxOwned},
            // Additional file delivery boxes
            fire::{FileReservoirBoxView, FileReservoirBoxOwned},
            // Camera and depth boxes
            cmex::{CameraExtrinsicMatrixBoxView, CameraExtrinsicMatrixBoxOwned},
            cmin::{CameraIntrinsicMatrixBoxView, CameraIntrinsicMatrixBoxOwned},
            // Additional codec/image boxes
            icef::{ImageExtentFrameBoxView, ImageExtentFrameBoxOwned},
            jpeg::{JPEGConfigurationBoxView, JPEGConfigurationBoxOwned},
            // Item reference type boxes
            // cdsc, dimg, thmb excluded: their new() requires a `large_ids` parameter
            // Timing and hint boxes
            dimm::{ImmediateDataSizeBoxView, ImmediateDataSizeBoxOwned},
            dmed::{MediaDurationBoxView, MediaDurationBoxOwned},
            drep::{RepeatedDataSizeBoxView, RepeatedDataSizeBoxOwned},
            hinf::{HintInfoBoxView, HintInfoBoxOwned},
            hnti::{HintTrackInfoBoxView, HintTrackInfoBoxOwned},
            maxr::{MaxDataRateBoxView, MaxDataRateBoxOwned},
            npck::{NumPacketsBoxView, NumPacketsBoxOwned},
            nump::{NumRTPPacketsBoxView, NumRTPPacketsBoxOwned},
            payt::{PayloadTypeBoxView, PayloadTypeBoxOwned},
            pmax::{LargestPacketSizeBoxView, LargestPacketSizeBoxOwned},
            sdp::{SDPBoxView, SDPBoxOwned},
            snro::{SequenceOffsetBoxView, SequenceOffsetBoxOwned},
            srat::{SamplingRateBoxView, SamplingRateBoxOwned},
            srpp::{SRTPProcessBoxView, SRTPProcessBoxOwned},
            tims::{TimeScaleEntryBoxView, TimeScaleEntryBoxOwned},
            tmax::{LargestRelativeTimeBoxView, LargestRelativeTimeBoxOwned},
            tmin::{SmallestRelativeTimeBoxView, SmallestRelativeTimeBoxOwned},
            totl::{TotalMediaBytesBoxView, TotalMediaBytesBoxOwned},
            tpyl::{TotalRTPBytesBoxView, TotalRTPBytesBoxOwned},
            trpo::{TimestampOffsetBoxView, TimestampOffsetBoxOwned},
            trpy::{TotalRTPBytesWithHeaderBoxView, TotalRTPBytesWithHeaderBoxOwned},
            tsro::{TimeOffsetBoxView, TimeOffsetBoxOwned},
            // Audio boxes
            alou::{AlbumLoudnessInfoBoxView, AlbumLoudnessInfoBoxOwned},
            chnl::{ChannelLayoutBoxView, ChannelLayoutBoxOwned},
            ludt::{LoudnessBaseBoxView, LoudnessBaseBoxOwned},
            tlou::{TrackLoudnessInfoBoxView, TrackLoudnessInfoBoxOwned},
            // Track type boxes
            ttyp::{TrackTypeBoxView, TrackTypeBoxOwned},
            // Plain boxes
            ftyp::{FileTypeBoxView, FileTypeBoxOwned},
            mdat::{MediaDataBoxView, MediaDataBoxOwned},
            pdin::{ProgressiveDownloadInfoBoxView, ProgressiveDownloadInfoBoxOwned},
            uuid::{UserExtensionBoxView, UserExtensionBoxOwned},
        }
    };
}

// TypedBoxView implementations for all concrete box view types.
for_each_box_view!(impl_typed_box_view);
