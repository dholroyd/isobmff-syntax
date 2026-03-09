//! Sample group entry types and parsing/serialization functions.
//!
//! Each variant of [`SampleGroupEntry`] corresponds to a specific `grouping_type`
//! FourCC. Unknown grouping types are preserved as [`SampleGroupEntry::Opaque`].

use crate::boxes::ldep::TierDependencyBoxOwned;
use crate::boxes::svdr::SvcDependencyRangeBoxOwned;
use crate::boxes::svpr::PriorityRangeBoxOwned;
use crate::boxes::tiri::TierInfoBoxOwned;
use crate::boxes::vwid::{ViewIdentifierBoxOwned, ViewIdentifierBoxView};
use crate::error::ParseError;
use crate::header::BoxHeader;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::FourCC;
use std::io::{self, Write};

// ---------------------------------------------------------------------------
// Entry structs - simple fixed-size
// ---------------------------------------------------------------------------

/// Roll/pre-roll recovery entry (`roll`, `prol`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollRecoveryEntry {
    pub roll_distance: i16,
}

/// Visual random-access point entry (`rap `).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualRandomAccessEntry {
    pub num_leading_samples_known: bool,
    pub num_leading_samples: u8,
}

/// Temporal level entry (`tele`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemporalLevelEntry {
    pub level_independently_decodable: bool,
}

/// Stream access point entry (`sap `).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SapEntry {
    pub dependent_flag: bool,
    pub sap_type: u8,
}

/// Dependent random-access point entry (`drap`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualDrapEntry {
    /// DRAP type (3 bits).
    pub drap_type: u8,
}

/// Pixel aspect ratio sample group entry (`pasr`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelAspectRatioEntry {
    pub h_spacing: u32,
    pub v_spacing: u32,
}

/// Clean aperture sample group entry (`casg`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanApertureEntry {
    pub clean_aperture_width_n: u32,
    pub clean_aperture_width_d: u32,
    pub clean_aperture_height_n: u32,
    pub clean_aperture_height_d: u32,
    pub horiz_off_n: u32,
    pub horiz_off_d: u32,
    pub vert_off_n: u32,
    pub vert_off_d: u32,
}

/// CENC sample encryption information entry (`seig`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CencSampleEncryptionEntry {
    /// Crypt byte block count (4 bits).
    pub crypt_byte_block: u8,
    /// Skip byte block count (4 bits).
    pub skip_byte_block: u8,
    /// Whether the sample is protected.
    pub is_protected: u8,
    /// Per-sample IV size.
    pub per_sample_iv_size: u8,
    /// Key ID.
    pub kid: [u8; 16],
    /// Constant IV (present when is_protected == 1 && per_sample_iv_size == 0).
    pub constant_iv: Option<Vec<u8>>,
}

/// Sync sample group entry (`sync`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncSampleEntry {
    pub nal_unit_type: u8,
}

/// AVC layer entry (`avll`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvcLayerEntry {
    pub layer_number: u8,
    pub accurate_statistics_flag: bool,
    pub avg_bit_rate: u16,
    pub avg_frame_rate: u16,
}

/// Temporal layer entry (`tscl`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemporalLayerEntry {
    pub temporal_layer_id: u8,
    pub tl_profile_space: u8,
    pub tl_tier_flag: bool,
    pub tl_profile_idc: u8,
    pub tl_profile_compatibility_flags: u32,
    pub tl_constraint_indicator_flags: [u8; 6],
    pub tl_level_idc: u8,
    pub tl_max_bit_rate: u16,
    pub tl_avg_bit_rate: u16,
    pub tl_constant_frame_rate: u8,
    pub tl_avg_frame_duration: u16,
}

/// Subpicture level info entry (`spli`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubpicLevelInfoEntry {
    pub level_idc: u8,
}

/// Parameter set sample group entry (`pss1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsSampleGroupEntry {
    pub sps_present: bool,
    pub pps_present: bool,
    pub aps_present: bool,
}

/// Access unit delimiter sample entry (`aud `).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudSampleEntry {
    pub aud_nal_unit: [u8; 3],
}

/// End of bitstream sample entry (`eob `).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndOfBitstreamSampleEntry {
    pub eob_nal_unit: [u8; 2],
}

// ---------------------------------------------------------------------------
// Entry structs - variable-length (arrays of primitives)
// ---------------------------------------------------------------------------

/// Sample-to-metadata-item entry (`stmi`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleToMetadataItemEntry {
    pub meta_box_handler_type: FourCC,
    pub item_ids: Vec<u32>,
}

/// Picture region replacement entry (`pprr`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PicRegionReplacementEntry {
    /// Region ID type (3 bits).
    pub region_id_type: u8,
    pub region_ids: Vec<u16>,
}

/// Scalable NAL unit map entry (`scnm`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalableNaluMapEntry {
    pub group_ids: Vec<u8>,
}

/// End of sequence sample entry (`eos `).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndOfSequenceSampleEntry {
    pub eos_nal_units: Vec<[u8; 2]>,
}

/// Parameter set NAL unit entry (`pase`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSetNaluEntry {
    pub ps_nal_unit: Vec<u8>,
}

/// VVC subpicture layout map entry (`sulm`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VvcSubpicLayoutMapEntry {
    pub group_id_info_4cc: FourCC,
    pub group_ids: Vec<u16>,
}

// ---------------------------------------------------------------------------
// Entry structs - variable-length with typed sub-structures
// ---------------------------------------------------------------------------

/// A sample pair in an alternative startup entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlternativeStartupSamplePair {
    pub num_output_samples: u16,
    pub num_total_samples: u16,
}

/// Alternative startup entry (`alst`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlternativeStartupEntry {
    pub roll_count: u16,
    pub first_output_sample: u16,
    pub sample_offsets: Vec<u32>,
    pub output_sample_pairs: Vec<AlternativeStartupSamplePair>,
}

/// An operation point in a rate share entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationPoint {
    pub available_bitrate: u32,
    pub target_rate_share: u16,
}

/// Rate share entry (`rash`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateShareEntry {
    pub operation_points: Vec<OperationPoint>,
    pub maximum_bitrate: u32,
    pub minimum_bitrate: u32,
    pub discard_priority: u8,
}

/// A single NALU map group entry within a `NaluMapEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaluMapGroupEntry {
    pub nalu_start_number: Option<u16>,
    pub group_id: u16,
}

/// NALU map entry (`nalm`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaluMapEntry {
    pub large_size: bool,
    pub rle: bool,
    pub entries: Vec<NaluMapGroupEntry>,
}

/// Rectangular region info for `trif` entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RectangularRegionGroupEntry {
    pub group_id: u16,
    pub rect_region_flag: bool,
    pub independent_idc: u8,
    pub full_picture: bool,
    pub filtering_disabled: bool,
    pub has_dependency_list: bool,
    pub horizontal_offset: u16,
    pub vertical_offset: u16,
    pub region_width: u16,
    pub region_height: u16,
    pub dependency_rect_region_group_ids: Vec<u16>,
}

/// A single layer info within a `LayerInfoGroupEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerInfo {
    pub irap_gdr_pics_in_layer_only_flag: bool,
    pub completeness_flag: bool,
    pub layer_id: u8,
    pub min_temporal_id: u8,
    pub max_temporal_id: u8,
    pub sub_layer_presence_flags: u8,
}

/// Layer info group entry (`linf`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerInfoGroupEntry {
    pub layers: Vec<LayerInfo>,
}

/// A dependency info for AVC sub-sequence entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvcDependencyInfo {
    pub sub_sequence_identifier_ref: u16,
    pub layer_number_ref: u8,
    pub direction_flag: bool,
}

/// AVC sub-sequence average rate info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvcSubSequenceAvgRate {
    pub accurate_statistics_flag: bool,
    pub avg_bit_rate: u16,
    pub avg_frame_rate: u16,
}

/// AVC sub-sequence entry (`avss`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvcSubSequenceEntry {
    pub sub_sequence_identifier: u16,
    pub layer_number: u8,
    pub duration: Option<u32>,
    pub avg_rate: Option<AvcSubSequenceAvgRate>,
    pub references: Vec<AvcDependencyInfo>,
}

/// Tier retiming info for decode retiming entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TierRetiming {
    pub tier_id: u16,
    pub sample_offset: i16,
}

/// Decode retiming entry (`dtrt`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeRetimingEntry {
    pub entries: Vec<TierRetiming>,
}

/// VVC subpicture ID mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VvcSubpicIdMapping {
    pub subpic_id: u16,
    pub group_id: u16,
}

/// VVC subpicture ID entry (`spid`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VvcSubpicIdEntry {
    pub mappings: Vec<VvcSubpicIdMapping>,
}

/// VVC subpicture ID rewriting info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VvcSubpicIdRewritingInfo {
    pub subpic_id_len: u8,
    pub subpic_id_bit_pos: u16,
    pub start_code_emulation_flag: bool,
    pub pps_sps_subpic_id_signalling_flag: bool,
    pub pps_id: Option<u8>,
    pub sps_id: Option<u8>,
}

/// VVC subpicture order entry (`spor`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VvcSubpicOrderEntry {
    pub subpic_id_info: Vec<VvcSubpicIdRewritingInfo>,
}

// ---------------------------------------------------------------------------
// Entry structs - with embedded box types
// ---------------------------------------------------------------------------

/// Scalable group entry (`scif`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalableGroupEntry {
    pub group_id: u16,
    pub primary_group_id: u16,
    pub tier_info: TierInfoBoxOwned,
    pub dependency_range: Option<SvcDependencyRangeBoxOwned>,
    pub priority_range: Option<PriorityRangeBoxOwned>,
}

/// Multiview group entry (`mvif`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiviewGroupEntry {
    pub group_id: u16,
    pub primary_group_id: u16,
    pub view_id: ViewIdentifierBoxOwned,
    pub tier_info: TierInfoBoxOwned,
    pub tier_dependency: Option<TierDependencyBoxOwned>,
    pub priority_range: Option<PriorityRangeBoxOwned>,
}

// ---------------------------------------------------------------------------
// SampleGroupEntry enum
// ---------------------------------------------------------------------------

/// A typed sample group description entry.
///
/// Each variant maps to a specific `grouping_type` FourCC value.
/// Unknown grouping types are preserved as [`Opaque`](SampleGroupEntry::Opaque).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleGroupEntry {
    // 14496-12 types
    RollRecovery(RollRecoveryEntry),
    VisualRandomAccess(VisualRandomAccessEntry),
    AlternativeStartup(AlternativeStartupEntry),
    TemporalLevel(TemporalLevelEntry),
    Sap(SapEntry),
    SampleToMetadataItem(SampleToMetadataItemEntry),
    VisualDrap(VisualDrapEntry),
    PixelAspectRatio(PixelAspectRatioEntry),
    CleanAperture(CleanApertureEntry),
    RateShare(RateShareEntry),

    // 14496-15 types
    CencSampleEncryption(CencSampleEncryptionEntry),
    SyncSample(SyncSampleEntry),
    NaluMap(NaluMapEntry),
    RectangularRegionGroup(RectangularRegionGroupEntry),
    LayerInfoGroup(LayerInfoGroupEntry),
    PicRegionReplacement(PicRegionReplacementEntry),
    AvcSubSequence(AvcSubSequenceEntry),
    AvcLayer(AvcLayerEntry),
    TemporalLayer(TemporalLayerEntry),
    TemporalSubLayer,
    StepwiseTemporalLayer,
    ParameterSetNalu(ParameterSetNaluEntry),
    AudSample(AudSampleEntry),
    EndOfSequenceSample(EndOfSequenceSampleEntry),
    EndOfBitstreamSample(EndOfBitstreamSampleEntry),
    VvcSubpicId(VvcSubpicIdEntry),
    VvcSubpicOrder(VvcSubpicOrderEntry),
    VvcSubpicLayoutMap(VvcSubpicLayoutMapEntry),
    SubpicLevelInfo(SubpicLevelInfoEntry),
    PsSampleGroup(PsSampleGroupEntry),
    ScalableGroup(ScalableGroupEntry),
    MultiviewGroup(MultiviewGroupEntry),
    ScalableNaluMap(ScalableNaluMapEntry),
    DecodeRetiming(DecodeRetimingEntry),

    /// Unrecognized grouping type - raw bytes preserved.
    Opaque(Vec<u8>),
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parses a single sample group entry from raw bytes given the grouping type.
///
/// Returns `Opaque` for unrecognized grouping types.
pub fn parse_entry(grouping_type: &FourCC, data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    match &grouping_type.0 {
        b"roll" | b"prol" => parse_roll(data),
        b"rap " => parse_rap(data),
        b"alst" => parse_alst(data),
        b"tele" => parse_tele(data),
        b"sap " => parse_sap(data),
        b"stmi" => parse_stmi(data),
        b"drap" => parse_drap(data),
        b"pasr" => parse_pasr(data),
        b"casg" => parse_casg(data),
        b"rash" => parse_rash(data),
        b"seig" => parse_seig(data),
        b"sync" => parse_sync(data),
        b"nalm" => parse_nalm(data),
        b"trif" => parse_trif(data),
        b"linf" => parse_linf(data),
        b"pprr" => parse_pprr(data),
        b"avss" => parse_avss(data),
        b"avll" => parse_avll(data),
        b"tscl" => parse_tscl(data),
        b"tsas" => Ok(SampleGroupEntry::TemporalSubLayer),
        b"stsa" => Ok(SampleGroupEntry::StepwiseTemporalLayer),
        b"pase" => parse_pase(data),
        b"aud " => parse_aud(data),
        b"eos " => parse_eos(data),
        b"eob " => parse_eob(data),
        b"spid" => parse_spid(data),
        b"spor" => parse_spor(data),
        b"sulm" => parse_sulm(data),
        b"spli" => parse_spli(data),
        b"pss1" => parse_pss1(data),
        b"scif" => parse_scif(data),
        b"mvif" => parse_mvif(data),
        b"scnm" => parse_scnm(data),
        b"dtrt" => parse_dtrt(data),
        _ => Ok(SampleGroupEntry::Opaque(data.to_vec())),
    }
}

// --- Individual parse functions ---

fn ensure_len(data: &[u8], min: usize, _ctx: &'static str) -> Result<(), ParseError> {
    if data.len() < min {
        return Err(ParseError::BufferTooShort {
            expected: min,
            found: data.len(),
        });
    }
    Ok(())
}

fn parse_roll(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 2, "roll entry")?;
    Ok(SampleGroupEntry::RollRecovery(RollRecoveryEntry {
        roll_distance: BigEndian::read_i16(data),
    }))
}

fn parse_rap(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "rap entry")?;
    Ok(SampleGroupEntry::VisualRandomAccess(
        VisualRandomAccessEntry {
            num_leading_samples_known: (data[0] >> 7) & 1 == 1,
            num_leading_samples: data[0] & 0x7F,
        },
    ))
}

fn parse_alst(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 4, "alst entry")?;
    let roll_count = BigEndian::read_u16(&data[0..2]);
    let first_output_sample = BigEndian::read_u16(&data[2..4]);

    let offsets_end = 4usize
        .checked_add((roll_count as usize).checked_mul(4).ok_or(
            ParseError::UnexpectedEndOfData {
                context: "alst sample_offsets overflow",
            },
        )?)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "alst sample_offsets overflow",
        })?;
    ensure_len(data, offsets_end, "alst sample_offsets")?;

    let mut sample_offsets = Vec::with_capacity(roll_count as usize);
    let mut off = 4;
    for _ in 0..roll_count {
        sample_offsets.push(BigEndian::read_u32(&data[off..off + 4]));
        off += 4;
    }

    // Remaining bytes are pairs of (num_output_samples, num_total_samples)
    let remaining = data.len() - off;
    let num_pairs = remaining / 4;
    let mut output_sample_pairs = Vec::with_capacity(num_pairs);
    for _ in 0..num_pairs {
        if off + 4 > data.len() {
            break;
        }
        output_sample_pairs.push(AlternativeStartupSamplePair {
            num_output_samples: BigEndian::read_u16(&data[off..off + 2]),
            num_total_samples: BigEndian::read_u16(&data[off + 2..off + 4]),
        });
        off += 4;
    }

    Ok(SampleGroupEntry::AlternativeStartup(
        AlternativeStartupEntry {
            roll_count,
            first_output_sample,
            sample_offsets,
            output_sample_pairs,
        },
    ))
}

fn parse_tele(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "tele entry")?;
    Ok(SampleGroupEntry::TemporalLevel(TemporalLevelEntry {
        level_independently_decodable: (data[0] >> 7) & 1 == 1,
    }))
}

fn parse_sap(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "sap entry")?;
    Ok(SampleGroupEntry::Sap(SapEntry {
        dependent_flag: (data[0] >> 7) & 1 == 1,
        sap_type: data[0] & 0x0F,
    }))
}

fn parse_stmi(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 8, "stmi entry")?;
    let meta_box_handler_type = FourCC([data[0], data[1], data[2], data[3]]);
    let num_items = BigEndian::read_u32(&data[4..8]);
    let items_end = 8usize
        .checked_add((num_items as usize).checked_mul(4).ok_or(
            ParseError::UnexpectedEndOfData {
                context: "stmi item_ids overflow",
            },
        )?)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "stmi item_ids overflow",
        })?;
    ensure_len(data, items_end, "stmi item_ids")?;

    let mut item_ids = Vec::with_capacity(num_items as usize);
    let mut off = 8;
    for _ in 0..num_items {
        item_ids.push(BigEndian::read_u32(&data[off..off + 4]));
        off += 4;
    }

    Ok(SampleGroupEntry::SampleToMetadataItem(
        SampleToMetadataItemEntry {
            meta_box_handler_type,
            item_ids,
        },
    ))
}

fn parse_drap(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 4, "drap entry")?;
    let w = BigEndian::read_u32(data);
    Ok(SampleGroupEntry::VisualDrap(VisualDrapEntry {
        drap_type: ((w >> 29) & 0x07) as u8,
    }))
}

fn parse_pasr(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 8, "pasr entry")?;
    Ok(SampleGroupEntry::PixelAspectRatio(PixelAspectRatioEntry {
        h_spacing: BigEndian::read_u32(&data[0..4]),
        v_spacing: BigEndian::read_u32(&data[4..8]),
    }))
}

fn parse_casg(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 32, "casg entry")?;
    Ok(SampleGroupEntry::CleanAperture(CleanApertureEntry {
        clean_aperture_width_n: BigEndian::read_u32(&data[0..4]),
        clean_aperture_width_d: BigEndian::read_u32(&data[4..8]),
        clean_aperture_height_n: BigEndian::read_u32(&data[8..12]),
        clean_aperture_height_d: BigEndian::read_u32(&data[12..16]),
        horiz_off_n: BigEndian::read_u32(&data[16..20]),
        horiz_off_d: BigEndian::read_u32(&data[20..24]),
        vert_off_n: BigEndian::read_u32(&data[24..28]),
        vert_off_d: BigEndian::read_u32(&data[28..32]),
    }))
}

fn parse_rash(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 10, "rash entry")?;
    let op_count = data[0] as usize;
    let mut off = 1usize;

    let mut operation_points = Vec::with_capacity(op_count);
    if op_count == 1 {
        ensure_len(data, off + 2, "rash single op")?;
        operation_points.push(OperationPoint {
            available_bitrate: 0,
            target_rate_share: BigEndian::read_u16(&data[off..off + 2]),
        });
        off += 2;
    } else {
        let needed = off
            .checked_add(op_count.checked_mul(6).ok_or(ParseError::UnexpectedEndOfData {
                context: "rash operation_points overflow",
            })?)
            .ok_or(ParseError::UnexpectedEndOfData {
                context: "rash operation_points overflow",
            })?;
        ensure_len(data, needed, "rash operation_points")?;
        for _ in 0..op_count {
            operation_points.push(OperationPoint {
                available_bitrate: BigEndian::read_u32(&data[off..off + 4]),
                target_rate_share: BigEndian::read_u16(&data[off + 4..off + 6]),
            });
            off += 6;
        }
    }

    ensure_len(data, off + 9, "rash trailer")?;
    let maximum_bitrate = BigEndian::read_u32(&data[off..off + 4]);
    let minimum_bitrate = BigEndian::read_u32(&data[off + 4..off + 8]);
    let discard_priority = data[off + 8];

    Ok(SampleGroupEntry::RateShare(RateShareEntry {
        operation_points,
        maximum_bitrate,
        minimum_bitrate,
        discard_priority,
    }))
}

fn parse_seig(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 20, "seig entry")?;
    let crypt_byte_block = data[0] & 0x0F;
    let skip_byte_block = data[1] & 0x0F;
    let is_protected = data[2];
    let per_sample_iv_size = data[3];
    let mut kid = [0u8; 16];
    kid.copy_from_slice(&data[4..20]);

    let constant_iv = if is_protected == 1 && per_sample_iv_size == 0 {
        ensure_len(data, 21, "seig constant_iv_size")?;
        let civ_size = data[20] as usize;
        ensure_len(data, 21 + civ_size, "seig constant_iv")?;
        Some(data[21..21 + civ_size].to_vec())
    } else {
        None
    };

    Ok(SampleGroupEntry::CencSampleEncryption(
        CencSampleEncryptionEntry {
            crypt_byte_block,
            skip_byte_block,
            is_protected,
            per_sample_iv_size,
            kid,
            constant_iv,
        },
    ))
}

fn parse_sync(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "sync entry")?;
    Ok(SampleGroupEntry::SyncSample(SyncSampleEntry {
        nal_unit_type: data[0],
    }))
}

fn parse_nalm(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "nalm entry")?;
    let flags_byte = data[0];
    let large_size = (flags_byte >> 1) & 1 == 1;
    let rle = flags_byte & 1 == 1;

    let mut off = 1usize;
    let entry_count = if large_size {
        ensure_len(data, off + 2, "nalm entry_count")?;
        let c = BigEndian::read_u16(&data[off..off + 2]) as usize;
        off += 2;
        c
    } else {
        ensure_len(data, off + 1, "nalm entry_count")?;
        let c = data[off] as usize;
        off += 1;
        c
    };

    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let nalu_start_number = if rle {
            if large_size {
                ensure_len(data, off + 2, "nalm nalu_start_number")?;
                let n = BigEndian::read_u16(&data[off..off + 2]);
                off += 2;
                Some(n)
            } else {
                ensure_len(data, off + 1, "nalm nalu_start_number")?;
                let n = data[off] as u16;
                off += 1;
                Some(n)
            }
        } else {
            None
        };
        ensure_len(data, off + 2, "nalm group_id")?;
        let group_id = BigEndian::read_u16(&data[off..off + 2]);
        off += 2;

        entries.push(NaluMapGroupEntry {
            nalu_start_number,
            group_id,
        });
    }

    Ok(SampleGroupEntry::NaluMap(NaluMapEntry {
        large_size,
        rle,
        entries,
    }))
}

fn parse_trif(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 3, "trif entry")?;
    let group_id = BigEndian::read_u16(&data[0..2]);
    let flags_byte = data[2];
    let rect_region_flag = (flags_byte >> 7) & 1 == 1;

    if !rect_region_flag {
        return Ok(SampleGroupEntry::RectangularRegionGroup(
            RectangularRegionGroupEntry {
                group_id,
                rect_region_flag: false,
                independent_idc: 0,
                full_picture: false,
                filtering_disabled: false,
                has_dependency_list: false,
                horizontal_offset: 0,
                vertical_offset: 0,
                region_width: 0,
                region_height: 0,
                dependency_rect_region_group_ids: Vec::new(),
            },
        ));
    }

    let independent_idc = (flags_byte >> 5) & 0x03;
    let full_picture = (flags_byte >> 4) & 1 == 1;
    let filtering_disabled = (flags_byte >> 3) & 1 == 1;
    let has_dependency_list = (flags_byte >> 2) & 1 == 1;

    let mut off = 3usize;
    let (horizontal_offset, vertical_offset) = if !full_picture {
        ensure_len(data, off + 4, "trif offsets")?;
        let h = BigEndian::read_u16(&data[off..off + 2]);
        let v = BigEndian::read_u16(&data[off + 2..off + 4]);
        off += 4;
        (h, v)
    } else {
        (0, 0)
    };

    ensure_len(data, off + 4, "trif dimensions")?;
    let region_width = BigEndian::read_u16(&data[off..off + 2]);
    let region_height = BigEndian::read_u16(&data[off + 2..off + 4]);
    off += 4;

    let mut dependency_rect_region_group_ids = Vec::new();
    if has_dependency_list {
        ensure_len(data, off + 2, "trif dependency_count")?;
        let dep_count = BigEndian::read_u16(&data[off..off + 2]) as usize;
        off += 2;
        let needed = off
            .checked_add(dep_count.checked_mul(2).ok_or(
                ParseError::UnexpectedEndOfData {
                    context: "trif dependency overflow",
                },
            )?)
            .ok_or(ParseError::UnexpectedEndOfData {
                context: "trif dependency overflow",
            })?;
        ensure_len(data, needed, "trif dependencies")?;
        for _ in 0..dep_count {
            dependency_rect_region_group_ids
                .push(BigEndian::read_u16(&data[off..off + 2]));
            off += 2;
        }
    }

    Ok(SampleGroupEntry::RectangularRegionGroup(
        RectangularRegionGroupEntry {
            group_id,
            rect_region_flag: true,
            independent_idc,
            full_picture,
            filtering_disabled,
            has_dependency_list,
            horizontal_offset,
            vertical_offset,
            region_width,
            region_height,
            dependency_rect_region_group_ids,
        },
    ))
}

fn parse_linf(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "linf entry")?;
    let num_layers = (data[0] & 0x3F) as usize;
    let mut layers = Vec::with_capacity(num_layers);
    let mut off = 1usize;

    for _ in 0..num_layers {
        ensure_len(data, off + 3, "linf layer")?;
        let b0 = data[off];
        let irap_gdr_pics_in_layer_only_flag = (b0 >> 5) & 1 == 1;
        let completeness_flag = (b0 >> 4) & 1 == 1;
        // layer_id spans b0[3:0] and b1[7:6]
        // Actually: reserved(2) + irap_flag(1) + completeness(1) + layer_id(6) + min_temporal(3) + max_temporal(3) + reserved(1) + sub_layer_presence(7)
        // = 24 bits = 3 bytes
        // byte 0: reserved(2) + irap_flag(1) + completeness(1) + layer_id_high(4)
        // byte 1: layer_id_low(2) + min_temporal_id(3) + max_temporal_id(3)
        // byte 2: reserved(1) + sub_layer_presence_flags(7)
        let layer_id_val = ((b0 & 0x0F) << 2) | ((data[off + 1] >> 6) & 0x03);
        let min_temporal_id = (data[off + 1] >> 3) & 0x07;
        let max_temporal_id = data[off + 1] & 0x07;
        let sub_layer_presence_flags = data[off + 2] & 0x7F;

        layers.push(LayerInfo {
            irap_gdr_pics_in_layer_only_flag,
            completeness_flag,
            layer_id: layer_id_val,
            min_temporal_id,
            max_temporal_id,
            sub_layer_presence_flags,
        });
        off += 3;
    }

    Ok(SampleGroupEntry::LayerInfoGroup(LayerInfoGroupEntry {
        layers,
    }))
}

fn parse_pprr(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 2, "pprr entry")?;
    let region_id_type = data[0] & 0x07;
    let num_region_ids = (data[1] as usize) + 1;
    let needed = 2usize
        .checked_add(num_region_ids.checked_mul(2).ok_or(
            ParseError::UnexpectedEndOfData {
                context: "pprr region_ids overflow",
            },
        )?)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "pprr region_ids overflow",
        })?;
    ensure_len(data, needed, "pprr region_ids")?;

    let mut region_ids = Vec::with_capacity(num_region_ids);
    let mut off = 2;
    for _ in 0..num_region_ids {
        region_ids.push(BigEndian::read_u16(&data[off..off + 2]));
        off += 2;
    }

    Ok(SampleGroupEntry::PicRegionReplacement(
        PicRegionReplacementEntry {
            region_id_type,
            region_ids,
        },
    ))
}

fn parse_avss(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 5, "avss entry")?;
    let sub_sequence_identifier = BigEndian::read_u16(&data[0..2]);
    let layer_number = data[2];
    let duration_flag = data[3] != 0;
    let avg_rate_flag = data[4] != 0;
    let mut off = 5usize;

    let duration = if duration_flag {
        ensure_len(data, off + 4, "avss duration")?;
        let d = BigEndian::read_u32(&data[off..off + 4]);
        off += 4;
        Some(d)
    } else {
        None
    };

    let avg_rate = if avg_rate_flag {
        ensure_len(data, off + 6, "avss avg_rate")?;
        let accurate_statistics_flag = (data[off] >> 7) & 1 == 1;
        let avg_bit_rate = BigEndian::read_u16(&data[off + 2..off + 4]);
        let avg_frame_rate = BigEndian::read_u16(&data[off + 4..off + 6]);
        off += 6;
        Some(AvcSubSequenceAvgRate {
            accurate_statistics_flag,
            avg_bit_rate,
            avg_frame_rate,
        })
    } else {
        None
    };

    ensure_len(data, off + 1, "avss num_references")?;
    let num_references = data[off] as usize;
    off += 1;

    let ref_needed = off
        .checked_add(num_references.checked_mul(4).ok_or(
            ParseError::UnexpectedEndOfData {
                context: "avss references overflow",
            },
        )?)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "avss references overflow",
        })?;
    ensure_len(data, ref_needed, "avss references")?;

    let mut references = Vec::with_capacity(num_references);
    for _ in 0..num_references {
        references.push(AvcDependencyInfo {
            sub_sequence_identifier_ref: BigEndian::read_u16(&data[off..off + 2]),
            layer_number_ref: data[off + 2],
            direction_flag: (data[off + 3] >> 7) & 1 == 1,
        });
        off += 4;
    }

    Ok(SampleGroupEntry::AvcSubSequence(AvcSubSequenceEntry {
        sub_sequence_identifier,
        layer_number,
        duration,
        avg_rate,
        references,
    }))
}

fn parse_avll(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 7, "avll entry")?;
    Ok(SampleGroupEntry::AvcLayer(AvcLayerEntry {
        layer_number: data[0],
        accurate_statistics_flag: (data[1] >> 7) & 1 == 1,
        avg_bit_rate: BigEndian::read_u16(&data[3..5]),
        avg_frame_rate: BigEndian::read_u16(&data[5..7]),
    }))
}

fn parse_tscl(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 20, "tscl entry")?;
    let temporal_layer_id = data[0];
    let b1 = data[1];
    let tl_profile_space = (b1 >> 6) & 0x03;
    let tl_tier_flag = (b1 >> 5) & 1 == 1;
    let tl_profile_idc = b1 & 0x1F;
    let tl_profile_compatibility_flags = BigEndian::read_u32(&data[2..6]);
    let mut tl_constraint_indicator_flags = [0u8; 6];
    tl_constraint_indicator_flags.copy_from_slice(&data[6..12]);
    let tl_level_idc = data[12];
    let tl_max_bit_rate = BigEndian::read_u16(&data[13..15]);
    let tl_avg_bit_rate = BigEndian::read_u16(&data[15..17]);
    let tl_constant_frame_rate = data[17];
    let tl_avg_frame_duration = BigEndian::read_u16(&data[18..20]);

    Ok(SampleGroupEntry::TemporalLayer(TemporalLayerEntry {
        temporal_layer_id,
        tl_profile_space,
        tl_tier_flag,
        tl_profile_idc,
        tl_profile_compatibility_flags,
        tl_constraint_indicator_flags,
        tl_level_idc,
        tl_max_bit_rate,
        tl_avg_bit_rate,
        tl_constant_frame_rate,
        tl_avg_frame_duration,
    }))
}

fn parse_pase(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    Ok(SampleGroupEntry::ParameterSetNalu(ParameterSetNaluEntry {
        ps_nal_unit: data.to_vec(),
    }))
}

fn parse_aud(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 3, "aud entry")?;
    let mut aud_nal_unit = [0u8; 3];
    aud_nal_unit.copy_from_slice(&data[0..3]);
    Ok(SampleGroupEntry::AudSample(AudSampleEntry {
        aud_nal_unit,
    }))
}

fn parse_eos(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    let mut eos_nal_units = Vec::new();
    let mut off = 0;
    while off + 2 <= data.len() {
        let mut unit = [0u8; 2];
        unit.copy_from_slice(&data[off..off + 2]);
        eos_nal_units.push(unit);
        off += 2;
    }
    Ok(SampleGroupEntry::EndOfSequenceSample(
        EndOfSequenceSampleEntry { eos_nal_units },
    ))
}

fn parse_eob(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 2, "eob entry")?;
    let mut eob_nal_unit = [0u8; 2];
    eob_nal_unit.copy_from_slice(&data[0..2]);
    Ok(SampleGroupEntry::EndOfBitstreamSample(
        EndOfBitstreamSampleEntry { eob_nal_unit },
    ))
}

fn parse_spid(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    let mut mappings = Vec::new();
    let mut off = 0;
    while off + 4 <= data.len() {
        mappings.push(VvcSubpicIdMapping {
            subpic_id: BigEndian::read_u16(&data[off..off + 2]),
            group_id: BigEndian::read_u16(&data[off + 2..off + 4]),
        });
        off += 4;
    }
    Ok(SampleGroupEntry::VvcSubpicId(VvcSubpicIdEntry {
        mappings,
    }))
}

fn parse_spor(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    let mut subpic_id_info = Vec::new();
    let mut off = 0;
    while off < data.len() {
        ensure_len(data, off + 4, "spor rewriting info")?;
        let subpic_id_len = data[off];
        let subpic_id_bit_pos = BigEndian::read_u16(&data[off + 1..off + 3]);
        let flags_byte = data[off + 3];
        let start_code_emulation_flag = (flags_byte >> 7) & 1 == 1;
        let pps_sps_subpic_id_signalling_flag = (flags_byte >> 6) & 1 == 1;
        off += 4;

        let (pps_id, sps_id) = if pps_sps_subpic_id_signalling_flag {
            ensure_len(data, off + 2, "spor pps/sps ids")?;
            let pps = data[off];
            let sps = data[off + 1];
            off += 2;
            (Some(pps), Some(sps))
        } else {
            (None, None)
        };

        subpic_id_info.push(VvcSubpicIdRewritingInfo {
            subpic_id_len,
            subpic_id_bit_pos,
            start_code_emulation_flag,
            pps_sps_subpic_id_signalling_flag,
            pps_id,
            sps_id,
        });
    }
    Ok(SampleGroupEntry::VvcSubpicOrder(VvcSubpicOrderEntry {
        subpic_id_info,
    }))
}

fn parse_sulm(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 4, "sulm entry")?;
    let group_id_info_4cc = FourCC([data[0], data[1], data[2], data[3]]);
    let mut group_ids = Vec::new();
    let mut off = 4;
    while off + 2 <= data.len() {
        group_ids.push(BigEndian::read_u16(&data[off..off + 2]));
        off += 2;
    }
    Ok(SampleGroupEntry::VvcSubpicLayoutMap(
        VvcSubpicLayoutMapEntry {
            group_id_info_4cc,
            group_ids,
        },
    ))
}

fn parse_spli(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "spli entry")?;
    Ok(SampleGroupEntry::SubpicLevelInfo(SubpicLevelInfoEntry {
        level_idc: data[0],
    }))
}

fn parse_pss1(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "pss1 entry")?;
    Ok(SampleGroupEntry::PsSampleGroup(PsSampleGroupEntry {
        sps_present: (data[0] >> 7) & 1 == 1,
        pps_present: (data[0] >> 6) & 1 == 1,
        aps_present: (data[0] >> 5) & 1 == 1,
    }))
}

fn parse_scnm(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    Ok(SampleGroupEntry::ScalableNaluMap(ScalableNaluMapEntry {
        group_ids: data.to_vec(),
    }))
}

fn parse_dtrt(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    ensure_len(data, 1, "dtrt entry")?;
    let num_entries = data[0] as usize;
    let needed = 1usize
        .checked_add(num_entries.checked_mul(4).ok_or(
            ParseError::UnexpectedEndOfData {
                context: "dtrt entries overflow",
            },
        )?)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "dtrt entries overflow",
        })?;
    ensure_len(data, needed, "dtrt entries")?;

    let mut entries = Vec::with_capacity(num_entries);
    let mut off = 1;
    for _ in 0..num_entries {
        entries.push(TierRetiming {
            tier_id: BigEndian::read_u16(&data[off..off + 2]),
            sample_offset: BigEndian::read_i16(&data[off + 2..off + 4]),
        });
        off += 4;
    }

    Ok(SampleGroupEntry::DecodeRetiming(DecodeRetimingEntry {
        entries,
    }))
}

fn parse_scif(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    use crate::boxes::svdr::SvcDependencyRangeBoxView;
    use crate::boxes::svpr::PriorityRangeBoxView;
    use crate::boxes::tiri::TierInfoBoxView;

    ensure_len(data, 4, "scif entry")?;
    let group_id = BigEndian::read_u16(&data[0..2]);
    let primary_group_id = BigEndian::read_u16(&data[2..4]);
    let mut off = 4usize;

    // Parse TierInfoBox (always present)
    if off + 8 > data.len() {
        return Err(ParseError::UnexpectedEndOfData {
            context: "scif TierInfoBox header",
        });
    }
    let tiri_header = BoxHeader::parse(&data[off..], data.len() - off)?;
    let tiri_end = off
        .checked_add(tiri_header.size as usize)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "scif TierInfoBox size",
        })?;
    if tiri_end > data.len() {
        return Err(ParseError::UnexpectedEndOfData {
            context: "scif TierInfoBox",
        });
    }
    let tiri_view = TierInfoBoxView::new(&data[off..tiri_end])?;
    let tier_info = TierInfoBoxOwned::from(&tiri_view);
    off = tiri_end;

    // Conditional boxes when group_id == primary_group_id
    let (dependency_range, priority_range) = if group_id == primary_group_id && off < data.len() {
        // Parse SvcDependencyRangeBox
        if off + 8 > data.len() {
            return Err(ParseError::UnexpectedEndOfData {
                context: "scif SvcDependencyRangeBox header",
            });
        }
        let svdr_header = BoxHeader::parse(&data[off..], data.len() - off)?;
        let svdr_end = off
            .checked_add(svdr_header.size as usize)
            .ok_or(ParseError::UnexpectedEndOfData {
                context: "scif SvcDependencyRangeBox size",
            })?;
        if svdr_end > data.len() {
            return Err(ParseError::UnexpectedEndOfData {
                context: "scif SvcDependencyRangeBox",
            });
        }
        let svdr_view = SvcDependencyRangeBoxView::new(&data[off..svdr_end])?;
        let dep_range = SvcDependencyRangeBoxOwned::from(&svdr_view);
        off = svdr_end;

        // Parse PriorityRangeBox
        if off + 8 > data.len() {
            return Err(ParseError::UnexpectedEndOfData {
                context: "scif PriorityRangeBox header",
            });
        }
        let svpr_header = BoxHeader::parse(&data[off..], data.len() - off)?;
        let svpr_end = off
            .checked_add(svpr_header.size as usize)
            .ok_or(ParseError::UnexpectedEndOfData {
                context: "scif PriorityRangeBox size",
            })?;
        if svpr_end > data.len() {
            return Err(ParseError::UnexpectedEndOfData {
                context: "scif PriorityRangeBox",
            });
        }
        let svpr_view = PriorityRangeBoxView::new(&data[off..svpr_end])?;
        let prio_range = PriorityRangeBoxOwned::from(&svpr_view);

        (Some(dep_range), Some(prio_range))
    } else {
        (None, None)
    };

    Ok(SampleGroupEntry::ScalableGroup(ScalableGroupEntry {
        group_id,
        primary_group_id,
        tier_info,
        dependency_range,
        priority_range,
    }))
}

fn parse_mvif(data: &[u8]) -> Result<SampleGroupEntry, ParseError> {
    use crate::boxes::ldep::TierDependencyBoxView;
    use crate::boxes::svpr::PriorityRangeBoxView;
    use crate::boxes::tiri::TierInfoBoxView;

    ensure_len(data, 4, "mvif entry")?;
    let group_id = BigEndian::read_u16(&data[0..2]);
    let primary_group_id = BigEndian::read_u16(&data[2..4]);
    let mut off = 4usize;

    // Parse ViewIdentifierBox (always present)
    if off + 8 > data.len() {
        return Err(ParseError::UnexpectedEndOfData {
            context: "mvif ViewIdentifierBox header",
        });
    }
    let vwid_header = BoxHeader::parse(&data[off..], data.len() - off)?;
    let vwid_end = off
        .checked_add(vwid_header.size as usize)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "mvif ViewIdentifierBox size",
        })?;
    if vwid_end > data.len() {
        return Err(ParseError::UnexpectedEndOfData {
            context: "mvif ViewIdentifierBox",
        });
    }
    let vwid_view = ViewIdentifierBoxView::new(&data[off..vwid_end])?;
    let view_id = ViewIdentifierBoxOwned::try_from(&vwid_view)?;
    off = vwid_end;

    // Parse TierInfoBox (always present)
    if off + 8 > data.len() {
        return Err(ParseError::UnexpectedEndOfData {
            context: "mvif TierInfoBox header",
        });
    }
    let tiri_header = BoxHeader::parse(&data[off..], data.len() - off)?;
    let tiri_end = off
        .checked_add(tiri_header.size as usize)
        .ok_or(ParseError::UnexpectedEndOfData {
            context: "mvif TierInfoBox size",
        })?;
    if tiri_end > data.len() {
        return Err(ParseError::UnexpectedEndOfData {
            context: "mvif TierInfoBox",
        });
    }
    let tiri_view = TierInfoBoxView::new(&data[off..tiri_end])?;
    let tier_info = TierInfoBoxOwned::from(&tiri_view);
    off = tiri_end;

    // Conditional boxes when group_id == primary_group_id
    let (tier_dependency, priority_range) =
        if group_id == primary_group_id && off < data.len() {
            // Parse TierDependencyBox
            if off + 8 > data.len() {
                return Err(ParseError::UnexpectedEndOfData {
                    context: "mvif TierDependencyBox header",
                });
            }
            let ldep_header = BoxHeader::parse(&data[off..], data.len() - off)?;
            let ldep_end = off
                .checked_add(ldep_header.size as usize)
                .ok_or(ParseError::UnexpectedEndOfData {
                    context: "mvif TierDependencyBox size",
                })?;
            if ldep_end > data.len() {
                return Err(ParseError::UnexpectedEndOfData {
                    context: "mvif TierDependencyBox",
                });
            }
            let ldep_view = TierDependencyBoxView::new(&data[off..ldep_end])?;
            let tier_dep = TierDependencyBoxOwned::from(&ldep_view);
            off = ldep_end;

            // Parse PriorityRangeBox
            if off + 8 > data.len() {
                return Err(ParseError::UnexpectedEndOfData {
                    context: "mvif PriorityRangeBox header",
                });
            }
            let svpr_header = BoxHeader::parse(&data[off..], data.len() - off)?;
            let svpr_end = off
                .checked_add(svpr_header.size as usize)
                .ok_or(ParseError::UnexpectedEndOfData {
                    context: "mvif PriorityRangeBox size",
                })?;
            if svpr_end > data.len() {
                return Err(ParseError::UnexpectedEndOfData {
                    context: "mvif PriorityRangeBox",
                });
            }
            let svpr_view = PriorityRangeBoxView::new(&data[off..svpr_end])?;
            let prio_range = PriorityRangeBoxOwned::from(&svpr_view);

            (Some(tier_dep), Some(prio_range))
        } else {
            (None, None)
        };

    Ok(SampleGroupEntry::MultiviewGroup(MultiviewGroupEntry {
        group_id,
        primary_group_id,
        view_id,
        tier_info,
        tier_dependency,
        priority_range,
    }))
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

impl SampleGroupEntry {
    /// Returns the serialized byte size of this entry (excluding any length prefix).
    pub fn serialized_size(&self) -> usize {
        match self {
            Self::RollRecovery(_) => 2,
            Self::VisualRandomAccess(_) => 1,
            Self::AlternativeStartup(e) => {
                4 + e.sample_offsets.len() * 4 + e.output_sample_pairs.len() * 4
            }
            Self::TemporalLevel(_) => 1,
            Self::Sap(_) => 1,
            Self::SampleToMetadataItem(e) => 8 + e.item_ids.len() * 4,
            Self::VisualDrap(_) => 4,
            Self::PixelAspectRatio(_) => 8,
            Self::CleanAperture(_) => 32,
            Self::RateShare(e) => {
                let ops_size = if e.operation_points.len() == 1 {
                    2
                } else {
                    e.operation_points.len() * 6
                };
                1 + ops_size + 9
            }
            Self::CencSampleEncryption(e) => {
                let base = 20;
                match &e.constant_iv {
                    Some(iv) => base + 1 + iv.len(),
                    None => base,
                }
            }
            Self::SyncSample(_) => 1,
            Self::NaluMap(e) => {
                let header = 1; // flags byte
                let count_size = if e.large_size { 2 } else { 1 };
                let per_entry = if e.rle {
                    if e.large_size { 4 } else { 3 }
                } else {
                    2
                };
                header + count_size + e.entries.len() * per_entry
            }
            Self::RectangularRegionGroup(e) => {
                if !e.rect_region_flag {
                    3
                } else {
                    let mut size = 3; // groupID(2) + flags byte(1)
                    if !e.full_picture {
                        size += 4; // offsets
                    }
                    size += 4; // dimensions
                    if e.has_dependency_list {
                        size += 2 + e.dependency_rect_region_group_ids.len() * 2;
                    }
                    size
                }
            }
            Self::LayerInfoGroup(e) => 1 + e.layers.len() * 3,
            Self::PicRegionReplacement(e) => 2 + e.region_ids.len() * 2,
            Self::AvcSubSequence(e) => {
                let mut size = 5; // identifier(2) + layer(1) + duration_flag(1) + avg_rate_flag(1)
                if e.duration.is_some() {
                    size += 4;
                }
                if e.avg_rate.is_some() {
                    size += 6; // accurate_flag+reserved(2) + avg_bit_rate(2) + avg_frame_rate(2)
                }
                size += 1; // num_references
                size += e.references.len() * 4;
                size
            }
            Self::AvcLayer(_) => 7,
            Self::TemporalLayer(_) => 20,
            Self::TemporalSubLayer => 0,
            Self::StepwiseTemporalLayer => 0,
            Self::ParameterSetNalu(e) => e.ps_nal_unit.len(),
            Self::AudSample(_) => 3,
            Self::EndOfSequenceSample(e) => e.eos_nal_units.len() * 2,
            Self::EndOfBitstreamSample(_) => 2,
            Self::VvcSubpicId(e) => e.mappings.len() * 4,
            Self::VvcSubpicOrder(e) => {
                e.subpic_id_info.iter().map(|info| {
                    4 + if info.pps_sps_subpic_id_signalling_flag { 2 } else { 0 }
                }).sum()
            }
            Self::VvcSubpicLayoutMap(e) => 4 + e.group_ids.len() * 2,
            Self::SubpicLevelInfo(_) => 1,
            Self::PsSampleGroup(_) => 1,
            Self::ScalableGroup(e) => {
                let mut size = 4usize; // group_id(2) + primary_group_id(2)
                size += e.tier_info.box_size() as usize;
                if let Some(ref dr) = e.dependency_range {
                    size += dr.box_size() as usize;
                }
                if let Some(ref pr) = e.priority_range {
                    size += pr.box_size() as usize;
                }
                size
            }
            Self::MultiviewGroup(e) => {
                let mut size = 4usize; // group_id(2) + primary_group_id(2)
                size += e.view_id.box_size() as usize;
                size += e.tier_info.box_size() as usize;
                if let Some(ref td) = e.tier_dependency {
                    size += td.box_size() as usize;
                }
                if let Some(ref pr) = e.priority_range {
                    size += pr.box_size() as usize;
                }
                size
            }
            Self::ScalableNaluMap(e) => e.group_ids.len(),
            Self::DecodeRetiming(e) => 1 + e.entries.len() * 4,
            Self::Opaque(data) => data.len(),
        }
    }

    /// Writes the serialized entry bytes to the writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::RollRecovery(e) => {
                writer.write_i16::<BigEndian>(e.roll_distance)?;
            }
            Self::VisualRandomAccess(e) => {
                let b = (u8::from(e.num_leading_samples_known) << 7)
                    | (e.num_leading_samples & 0x7F);
                writer.write_u8(b)?;
            }
            Self::AlternativeStartup(e) => {
                writer.write_u16::<BigEndian>(e.roll_count)?;
                writer.write_u16::<BigEndian>(e.first_output_sample)?;
                for &offset in &e.sample_offsets {
                    writer.write_u32::<BigEndian>(offset)?;
                }
                for pair in &e.output_sample_pairs {
                    writer.write_u16::<BigEndian>(pair.num_output_samples)?;
                    writer.write_u16::<BigEndian>(pair.num_total_samples)?;
                }
            }
            Self::TemporalLevel(e) => {
                let b = u8::from(e.level_independently_decodable) << 7;
                writer.write_u8(b)?;
            }
            Self::Sap(e) => {
                let b = (u8::from(e.dependent_flag) << 7) | (e.sap_type & 0x0F);
                writer.write_u8(b)?;
            }
            Self::SampleToMetadataItem(e) => {
                writer.write_all(&e.meta_box_handler_type.0)?;
                writer.write_u32::<BigEndian>(e.item_ids.len() as u32)?;
                for &id in &e.item_ids {
                    writer.write_u32::<BigEndian>(id)?;
                }
            }
            Self::VisualDrap(e) => {
                let w = (e.drap_type as u32 & 0x07) << 29;
                writer.write_u32::<BigEndian>(w)?;
            }
            Self::PixelAspectRatio(e) => {
                writer.write_u32::<BigEndian>(e.h_spacing)?;
                writer.write_u32::<BigEndian>(e.v_spacing)?;
            }
            Self::CleanAperture(e) => {
                writer.write_u32::<BigEndian>(e.clean_aperture_width_n)?;
                writer.write_u32::<BigEndian>(e.clean_aperture_width_d)?;
                writer.write_u32::<BigEndian>(e.clean_aperture_height_n)?;
                writer.write_u32::<BigEndian>(e.clean_aperture_height_d)?;
                writer.write_u32::<BigEndian>(e.horiz_off_n)?;
                writer.write_u32::<BigEndian>(e.horiz_off_d)?;
                writer.write_u32::<BigEndian>(e.vert_off_n)?;
                writer.write_u32::<BigEndian>(e.vert_off_d)?;
            }
            Self::RateShare(e) => {
                writer.write_u8(e.operation_points.len() as u8)?;
                if e.operation_points.len() == 1 {
                    writer.write_u16::<BigEndian>(e.operation_points[0].target_rate_share)?;
                } else {
                    for op in &e.operation_points {
                        writer.write_u32::<BigEndian>(op.available_bitrate)?;
                        writer.write_u16::<BigEndian>(op.target_rate_share)?;
                    }
                }
                writer.write_u32::<BigEndian>(e.maximum_bitrate)?;
                writer.write_u32::<BigEndian>(e.minimum_bitrate)?;
                writer.write_u8(e.discard_priority)?;
            }
            Self::CencSampleEncryption(e) => {
                writer.write_u8(e.crypt_byte_block & 0x0F)?;
                writer.write_u8(e.skip_byte_block & 0x0F)?;
                writer.write_u8(e.is_protected)?;
                writer.write_u8(e.per_sample_iv_size)?;
                writer.write_all(&e.kid)?;
                if let Some(ref iv) = e.constant_iv {
                    writer.write_u8(iv.len() as u8)?;
                    writer.write_all(iv)?;
                }
            }
            Self::SyncSample(e) => {
                writer.write_u8(e.nal_unit_type)?;
            }
            Self::NaluMap(e) => {
                let flags = (u8::from(e.large_size) << 1) | u8::from(e.rle);
                writer.write_u8(flags)?;
                if e.large_size {
                    writer.write_u16::<BigEndian>(e.entries.len() as u16)?;
                } else {
                    writer.write_u8(e.entries.len() as u8)?;
                }
                for entry in &e.entries {
                    if e.rle
                        && let Some(start) = entry.nalu_start_number
                    {
                        if e.large_size {
                            writer.write_u16::<BigEndian>(start)?;
                        } else {
                            writer.write_u8(start as u8)?;
                        }
                    }
                    writer.write_u16::<BigEndian>(entry.group_id)?;
                }
            }
            Self::RectangularRegionGroup(e) => {
                writer.write_u16::<BigEndian>(e.group_id)?;
                if !e.rect_region_flag {
                    writer.write_u8(0)?; // rect_region_flag=0 + reserved
                } else {
                    let flags = (1u8 << 7) // rect_region_flag
                        | ((e.independent_idc & 0x03) << 5)
                        | (u8::from(e.full_picture) << 4)
                        | (u8::from(e.filtering_disabled) << 3)
                        | (u8::from(e.has_dependency_list) << 2);
                    writer.write_u8(flags)?;
                    if !e.full_picture {
                        writer.write_u16::<BigEndian>(e.horizontal_offset)?;
                        writer.write_u16::<BigEndian>(e.vertical_offset)?;
                    }
                    writer.write_u16::<BigEndian>(e.region_width)?;
                    writer.write_u16::<BigEndian>(e.region_height)?;
                    if e.has_dependency_list {
                        writer.write_u16::<BigEndian>(
                            e.dependency_rect_region_group_ids.len() as u16,
                        )?;
                        for &id in &e.dependency_rect_region_group_ids {
                            writer.write_u16::<BigEndian>(id)?;
                        }
                    }
                }
            }
            Self::LayerInfoGroup(e) => {
                let b = (e.layers.len() as u8) & 0x3F;
                writer.write_u8(b)?;
                for layer in &e.layers {
                    // byte 0: reserved(2) + irap_flag(1) + completeness(1) + layer_id_high(4)
                    let b0 = (u8::from(layer.irap_gdr_pics_in_layer_only_flag) << 5)
                        | (u8::from(layer.completeness_flag) << 4)
                        | ((layer.layer_id >> 2) & 0x0F);
                    writer.write_u8(b0)?;
                    // byte 1: layer_id_low(2) + min_temporal_id(3) + max_temporal_id(3)
                    let b1 = ((layer.layer_id & 0x03) << 6)
                        | ((layer.min_temporal_id & 0x07) << 3)
                        | (layer.max_temporal_id & 0x07);
                    writer.write_u8(b1)?;
                    // byte 2: reserved(1) + sub_layer_presence_flags(7)
                    writer.write_u8(layer.sub_layer_presence_flags & 0x7F)?;
                }
            }
            Self::PicRegionReplacement(e) => {
                writer.write_u8(e.region_id_type & 0x07)?;
                let count_minus1 = if e.region_ids.is_empty() {
                    0u8
                } else {
                    (e.region_ids.len() - 1) as u8
                };
                writer.write_u8(count_minus1)?;
                for &id in &e.region_ids {
                    writer.write_u16::<BigEndian>(id)?;
                }
            }
            Self::AvcSubSequence(e) => {
                writer.write_u16::<BigEndian>(e.sub_sequence_identifier)?;
                writer.write_u8(e.layer_number)?;
                writer.write_u8(u8::from(e.duration.is_some()))?;
                writer.write_u8(u8::from(e.avg_rate.is_some()))?;
                if let Some(duration) = e.duration {
                    writer.write_u32::<BigEndian>(duration)?;
                }
                if let Some(ref avg_rate) = e.avg_rate {
                    let b0 = u8::from(avg_rate.accurate_statistics_flag) << 7;
                    writer.write_u8(b0)?;
                    writer.write_u8(0)?; // reserved byte (lower byte of 16-bit field)
                    writer.write_u16::<BigEndian>(avg_rate.avg_bit_rate)?;
                    writer.write_u16::<BigEndian>(avg_rate.avg_frame_rate)?;
                }
                writer.write_u8(e.references.len() as u8)?;
                for r in &e.references {
                    writer.write_u16::<BigEndian>(r.sub_sequence_identifier_ref)?;
                    writer.write_u8(r.layer_number_ref)?;
                    let dir_byte = u8::from(r.direction_flag) << 7;
                    writer.write_u8(dir_byte)?;
                }
            }
            Self::AvcLayer(e) => {
                writer.write_u8(e.layer_number)?;
                let b1 = u8::from(e.accurate_statistics_flag) << 7;
                writer.write_u8(b1)?;
                writer.write_u8(0)?; // reserved byte (lower byte of 16-bit field)
                writer.write_u16::<BigEndian>(e.avg_bit_rate)?;
                writer.write_u16::<BigEndian>(e.avg_frame_rate)?;
            }
            Self::TemporalLayer(e) => {
                writer.write_u8(e.temporal_layer_id)?;
                let b1 = ((e.tl_profile_space & 0x03) << 6)
                    | (u8::from(e.tl_tier_flag) << 5)
                    | (e.tl_profile_idc & 0x1F);
                writer.write_u8(b1)?;
                writer.write_u32::<BigEndian>(e.tl_profile_compatibility_flags)?;
                writer.write_all(&e.tl_constraint_indicator_flags)?;
                writer.write_u8(e.tl_level_idc)?;
                writer.write_u16::<BigEndian>(e.tl_max_bit_rate)?;
                writer.write_u16::<BigEndian>(e.tl_avg_bit_rate)?;
                writer.write_u8(e.tl_constant_frame_rate)?;
                writer.write_u16::<BigEndian>(e.tl_avg_frame_duration)?;
            }
            Self::TemporalSubLayer | Self::StepwiseTemporalLayer => {
                // Empty entries - no payload
            }
            Self::ParameterSetNalu(e) => {
                writer.write_all(&e.ps_nal_unit)?;
            }
            Self::AudSample(e) => {
                writer.write_all(&e.aud_nal_unit)?;
            }
            Self::EndOfSequenceSample(e) => {
                for unit in &e.eos_nal_units {
                    writer.write_all(unit)?;
                }
            }
            Self::EndOfBitstreamSample(e) => {
                writer.write_all(&e.eob_nal_unit)?;
            }
            Self::VvcSubpicId(e) => {
                for m in &e.mappings {
                    writer.write_u16::<BigEndian>(m.subpic_id)?;
                    writer.write_u16::<BigEndian>(m.group_id)?;
                }
            }
            Self::VvcSubpicOrder(e) => {
                for info in &e.subpic_id_info {
                    writer.write_u8(info.subpic_id_len)?;
                    writer.write_u16::<BigEndian>(info.subpic_id_bit_pos)?;
                    let flags = (u8::from(info.start_code_emulation_flag) << 7)
                        | (u8::from(info.pps_sps_subpic_id_signalling_flag) << 6);
                    writer.write_u8(flags)?;
                    if info.pps_sps_subpic_id_signalling_flag {
                        writer.write_u8(info.pps_id.unwrap_or(0))?;
                        writer.write_u8(info.sps_id.unwrap_or(0))?;
                    }
                }
            }
            Self::VvcSubpicLayoutMap(e) => {
                writer.write_all(&e.group_id_info_4cc.0)?;
                for &id in &e.group_ids {
                    writer.write_u16::<BigEndian>(id)?;
                }
            }
            Self::SubpicLevelInfo(e) => {
                writer.write_u8(e.level_idc)?;
            }
            Self::PsSampleGroup(e) => {
                let b = (u8::from(e.sps_present) << 7)
                    | (u8::from(e.pps_present) << 6)
                    | (u8::from(e.aps_present) << 5);
                writer.write_u8(b)?;
            }
            Self::ScalableGroup(e) => {
                writer.write_u16::<BigEndian>(e.group_id)?;
                writer.write_u16::<BigEndian>(e.primary_group_id)?;
                e.tier_info.write_to(writer)?;
                if let Some(ref dr) = e.dependency_range {
                    dr.write_to(writer)?;
                }
                if let Some(ref pr) = e.priority_range {
                    pr.write_to(writer)?;
                }
            }
            Self::MultiviewGroup(e) => {
                writer.write_u16::<BigEndian>(e.group_id)?;
                writer.write_u16::<BigEndian>(e.primary_group_id)?;
                e.view_id.write_to(writer)?;
                e.tier_info.write_to(writer)?;
                if let Some(ref td) = e.tier_dependency {
                    td.write_to(writer)?;
                }
                if let Some(ref pr) = e.priority_range {
                    pr.write_to(writer)?;
                }
            }
            Self::ScalableNaluMap(e) => {
                writer.write_all(&e.group_ids)?;
            }
            Self::DecodeRetiming(e) => {
                writer.write_u8(e.entries.len() as u8)?;
                for entry in &e.entries {
                    writer.write_u16::<BigEndian>(entry.tier_id)?;
                    writer.write_i16::<BigEndian>(entry.sample_offset)?;
                }
            }
            Self::Opaque(data) => {
                writer.write_all(data)?;
            }
        }
        Ok(())
    }
}

use crate::boxes::ldep;
use crate::boxes::svdr;
use crate::boxes::svpr;
use crate::boxes::tiri;
use crate::boxes::vwid;

// Trait imports for box_size() method
use ldep::TierDependencyBox as _;
use svdr::SvcDependencyRangeBox as _;
use svpr::PriorityRangeBox as _;
use tiri::TierInfoBox as _;
use vwid::ViewIdentifierBox as _;
