//! Criterion benchmark for parsing a synthetic ISOBMFF file.
//!
//! Constructs a realistic non-fragmented MP4 file using this crate's owned types,
//! then benchmarks parsing via the zero-copy view types and BoxIterator.

use criterion::{Criterion, black_box, criterion_group, criterion_main};

// Owned types for building the synthetic file
use isobmff_syntax::boxes::ctts::{CompositionOffsetEntry, CompositionTimeToSampleBoxOwned};
use isobmff_syntax::boxes::dinf::DataInformationBoxOwned;
use isobmff_syntax::boxes::dref::DataReferenceBoxOwned;
use isobmff_syntax::boxes::edts::EditBoxOwned;
use isobmff_syntax::boxes::elst::{EditListBoxOwned, EditListEntry};
use isobmff_syntax::boxes::ftyp::FileTypeBoxOwned;
use isobmff_syntax::boxes::hdlr::HandlerReferenceBoxOwned;
use isobmff_syntax::boxes::mdat::MediaDataBoxOwned;
use isobmff_syntax::boxes::mdhd::MediaHeaderBoxOwned;
use isobmff_syntax::boxes::mdia::MediaBoxOwned;
use isobmff_syntax::boxes::minf::MediaInformationBoxOwned;
use isobmff_syntax::boxes::moov::MovieBoxOwned;
use isobmff_syntax::boxes::mvhd::MovieHeaderBoxOwned;
use isobmff_syntax::boxes::sample_entry::{AudioSampleEntryOwned, VisualSampleEntryOwned};
use isobmff_syntax::boxes::smhd::SoundMediaHeaderBoxOwned;
use isobmff_syntax::boxes::stbl::SampleTableBoxOwned;
use isobmff_syntax::boxes::stco::ChunkOffsetBoxOwned;
use isobmff_syntax::boxes::stsc::{SampleToChunkBoxOwned, SampleToChunkEntry};
use isobmff_syntax::boxes::stsd::{SampleDescriptionBoxOwned, SampleDescriptionChild};
use isobmff_syntax::boxes::stss::SyncSampleBoxOwned;
use isobmff_syntax::boxes::stsz::SampleSizeBoxOwned;
use isobmff_syntax::boxes::stts::{TimeToSampleBoxOwned, TimeToSampleEntry};
use isobmff_syntax::boxes::tkhd::{self, TrackHeaderBoxOwned};
use isobmff_syntax::boxes::trak::TrackBoxOwned;
use isobmff_syntax::boxes::vmhd::VideoMediaHeaderBoxOwned;
// Owned types for building fragmented MP4
use isobmff_syntax::boxes::mfhd::MovieFragmentHeaderBoxOwned;
use isobmff_syntax::boxes::moof::MovieFragmentBoxOwned;
use isobmff_syntax::boxes::saio::SampleAuxiliaryInformationOffsetsBoxOwned;
use isobmff_syntax::boxes::saiz::{SampleAuxiliaryInformationSizesBoxOwned, SampleInfoSizes};
use isobmff_syntax::boxes::sbgp::{SampleToGroupBoxOwned, SampleToGroupEntry};
use isobmff_syntax::boxes::sdtp::{SampleDependencyFlags, SampleDependencyTypeBoxOwned};
use isobmff_syntax::boxes::mfra::MovieFragmentRandomAccessBoxOwned;
use isobmff_syntax::boxes::mfro::MovieFragmentRandomAccessOffsetBoxOwned;
use isobmff_syntax::boxes::senc::{SampleEncryptionBoxOwned, SampleEncryptionEntry};
use isobmff_syntax::boxes::sidx::{SegmentIndexBoxOwned, SegmentIndexReference};
use isobmff_syntax::boxes::tfra::{RandomAccessEntry, TrackFragmentRandomAccessBoxOwned};
use isobmff_syntax::boxes::tfdt::TrackFragmentBaseMediaDecodeTimeBoxOwned;
use isobmff_syntax::boxes::tfhd::TrackFragmentHeaderBoxOwned;
use isobmff_syntax::boxes::traf::TrackFragmentBoxOwned;
use isobmff_syntax::boxes::trun::{TrackRunBoxOwned, TrackRunSample};
// View types for parsing
use isobmff_syntax::boxes::ctts::CompositionTimeToSampleBoxView;
use isobmff_syntax::boxes::edts::EditBoxView;
use isobmff_syntax::boxes::elst::EditListBoxView;
use isobmff_syntax::boxes::ftyp::FileTypeBoxView;
use isobmff_syntax::boxes::hdlr::HandlerReferenceBoxView;
use isobmff_syntax::boxes::mdhd::MediaHeaderBoxView;
use isobmff_syntax::boxes::mdia::MediaBoxView;
use isobmff_syntax::boxes::minf::MediaInformationBoxView;
use isobmff_syntax::boxes::moof::MovieFragmentBoxView;
use isobmff_syntax::boxes::moov::MovieBoxView;
use isobmff_syntax::boxes::mvhd::MovieHeaderBoxView;
use isobmff_syntax::boxes::saio::SampleAuxiliaryInformationOffsetsBoxView;
use isobmff_syntax::boxes::saiz::SampleAuxiliaryInformationSizesBoxView;
use isobmff_syntax::boxes::sbgp::SampleToGroupBoxView;
use isobmff_syntax::boxes::sdtp::SampleDependencyTypeBoxView;
use isobmff_syntax::boxes::mfra::MovieFragmentRandomAccessBoxView;
use isobmff_syntax::boxes::senc::SampleEncryptionBoxView;
use isobmff_syntax::boxes::sidx::SegmentIndexBoxView;
use isobmff_syntax::boxes::tfra::TrackFragmentRandomAccessBoxView;
use isobmff_syntax::boxes::stbl::SampleTableBoxView;
use isobmff_syntax::boxes::stco::ChunkOffsetBoxView;
use isobmff_syntax::boxes::stsc::SampleToChunkBoxView;
use isobmff_syntax::boxes::stsd::SampleDescriptionBoxView;
use isobmff_syntax::boxes::stss::SyncSampleBoxView;
use isobmff_syntax::boxes::stsz::SampleSizeBoxView;
use isobmff_syntax::boxes::stts::TimeToSampleBoxView;
use isobmff_syntax::boxes::tfdt::TrackFragmentBaseMediaDecodeTimeBoxView;
use isobmff_syntax::boxes::tfhd::TrackFragmentHeaderBoxView;
use isobmff_syntax::boxes::tkhd::TrackHeaderBoxView;
use isobmff_syntax::boxes::traf::TrackFragmentBoxView;
use isobmff_syntax::boxes::trak::TrackBoxView;
use isobmff_syntax::boxes::trun::TrackRunBoxView;
// Traits needed for method access on views returned by dispatch_box!
use isobmff_syntax::boxes::{
    ChunkOffsetBox, CompositionTimeToSampleBox, EditListBox,
    FileTypeBox, HandlerReferenceBox, MediaHeaderBox,
    MovieHeaderBox,
    SampleAuxiliaryInformationOffsetsBox, SampleAuxiliaryInformationSizesBox,
    SampleDescriptionBox, SampleDependencyTypeBox, SampleSizeBox,
    SampleToChunkBox, SampleToGroupBox, SegmentIndexBox,
    SyncSampleBox, TimeToSampleBox, TrackFragmentRandomAccessBox,
    TrackFragmentBaseMediaDecodeTimeBox, TrackFragmentHeaderBox, TrackHeaderBox,
    TrackRunBox,
};

use isobmff_syntax::{
    BoxCode, BoxIterator, BrandCode, FixedPoint16_16, FixedPoint8_8, IsoLanguageCode, Matrix,
    UFixedPoint16_16, dispatch_box,
};

// ---------------------------------------------------------------------------
// Synthetic file parameters
// ---------------------------------------------------------------------------

/// Simulated duration: 60 seconds of content.
const DURATION_SEC: u64 = 60;
/// Video: 30 fps, 90 000 Hz timescale.
const VIDEO_TIMESCALE: u32 = 90_000;
const VIDEO_FPS: u32 = 30;
/// Audio: AAC 48 kHz, 1024-sample frames.
const AUDIO_TIMESCALE: u32 = 48_000;
const AUDIO_FRAME_DURATION: u32 = 1024;

const VIDEO_SAMPLES: u32 = (DURATION_SEC as u32) * VIDEO_FPS; // 1 800
const AUDIO_SAMPLES: u32 = ((DURATION_SEC * AUDIO_TIMESCALE as u64) / AUDIO_FRAME_DURATION as u64) as u32; // ~2 812

/// Keyframe every 2 seconds.
const KEYFRAME_INTERVAL: u32 = VIDEO_FPS * 2;

// ---------------------------------------------------------------------------
// Build video track
// ---------------------------------------------------------------------------

fn build_video_track() -> TrackBoxOwned {
    let video_duration = DURATION_SEC * VIDEO_TIMESCALE as u64;
    let movie_duration = DURATION_SEC * 1000; // movie timescale = 1000

    // tkhd
    let tkhd = {
        let mut b = TrackHeaderBoxOwned::new();
        b.flags = tkhd::flags::TRACK_ENABLED | tkhd::flags::TRACK_IN_MOVIE | tkhd::flags::TRACK_IN_PREVIEW;
        b.track_id = 1;
        b.duration = movie_duration;
        b.width = UFixedPoint16_16::from_int(1920);
        b.height = UFixedPoint16_16::from_int(1080);
        b
    };

    // edts > elst
    let edts = {
        let mut elst = EditListBoxOwned::new();
        elst.entries.push(EditListEntry {
            segment_duration: movie_duration,
            media_time: 0,
            media_rate: FixedPoint16_16::ONE,
        });
        let mut edts = EditBoxOwned::new();
        edts.add_elst(elst);
        edts
    };

    // mdhd
    let mdhd = {
        let mut b = MediaHeaderBoxOwned::new();
        b.timescale = VIDEO_TIMESCALE;
        b.duration = video_duration;
        b.language = IsoLanguageCode::UNDETERMINED;
        b
    };

    // hdlr
    let hdlr = HandlerReferenceBoxOwned::video();

    // stsd – one AVC sample entry (minimal, no codec-specific children needed)
    let stsd = {
        let mut vse = VisualSampleEntryOwned::new(BoxCode::new(*b"avc1"));
        vse.width = 1920;
        vse.height = 1080;
        let mut stsd = SampleDescriptionBoxOwned::new();
        stsd.entries.push(SampleDescriptionChild::Visual(vse));
        stsd
    };

    // stts – constant frame duration
    let stts = {
        let delta = VIDEO_TIMESCALE / VIDEO_FPS; // 3000
        let mut b = TimeToSampleBoxOwned::new();
        b.entries.push(TimeToSampleEntry {
            sample_count: VIDEO_SAMPLES,
            sample_delta: delta,
        });
        b
    };

    // ctts – simulate B-frame reordering with a repeating pattern
    let ctts = {
        let delta = VIDEO_TIMESCALE / VIDEO_FPS;
        let mut b = CompositionTimeToSampleBoxOwned::new();
        // Pattern: I=0, B=2*delta, B=delta, P=0 repeating
        let pattern = [
            (1u32, 0i64),
            (1, 2 * delta as i64),
            (1, delta as i64),
            (1, 0),
        ];
        let repeats = VIDEO_SAMPLES / 4;
        for _ in 0..repeats {
            for &(count, offset) in &pattern {
                b.entries.push(CompositionOffsetEntry {
                    sample_count: count,
                    sample_offset: offset,
                });
            }
        }
        // Handle remainder
        let remainder = VIDEO_SAMPLES % 4;
        for &(count, offset) in pattern.iter().take(remainder as usize) {
            b.entries.push(CompositionOffsetEntry {
                sample_count: count,
                sample_offset: offset,
            });
        }
        b
    };

    // stss – sync samples (keyframes)
    let stss = {
        let mut b = SyncSampleBoxOwned::new();
        let mut sample = 1u32;
        while sample <= VIDEO_SAMPLES {
            b.entries.push(sample);
            sample = sample.saturating_add(KEYFRAME_INTERVAL);
        }
        b
    };

    // stsc – 10 samples per chunk
    let samples_per_chunk = 10u32;
    let num_chunks = VIDEO_SAMPLES.div_ceil(samples_per_chunk);
    let stsc = {
        let mut b = SampleToChunkBoxOwned::new();
        b.entries.push(SampleToChunkEntry {
            first_chunk: 1,
            samples_per_chunk,
            sample_description_index: 1,
        });
        b
    };

    // stsz – variable sizes (simulate realistic video frame sizes)
    let stsz = {
        let mut sizes = Vec::with_capacity(VIDEO_SAMPLES as usize);
        for i in 0..VIDEO_SAMPLES {
            let is_keyframe = i % KEYFRAME_INTERVAL == 0;
            let base = if is_keyframe { 50_000u32 } else { 8_000u32 };
            // Add some variation
            let size = base + (i % 37) * 100;
            sizes.push(size);
        }
        SampleSizeBoxOwned::with_variable_sizes(sizes)
    };

    // stco – one offset per chunk
    let stco = {
        let mut b = ChunkOffsetBoxOwned::new();
        let mut offset = 48u32; // after ftyp
        for _ in 0..num_chunks {
            b.entries.push(offset);
            offset = offset.saturating_add(150_000); // approximate chunk size
        }
        b
    };

    // vmhd
    let vmhd = VideoMediaHeaderBoxOwned::new();

    // dinf > dref
    let dinf = {
        let dref = DataReferenceBoxOwned::new_self_contained();
        let mut dinf = DataInformationBoxOwned::new();
        dinf.add_dref(dref);
        dinf
    };

    // Assemble stbl
    let mut stbl = SampleTableBoxOwned::new();
    stbl.add_stsd(stsd);
    stbl.add_stts(stts);
    stbl.add_ctts(ctts);
    stbl.add_stss(stss);
    stbl.add_stsc(stsc);
    stbl.add_stsz(stsz);
    stbl.add_stco(stco);

    // Assemble minf
    let mut minf = MediaInformationBoxOwned::new();
    minf.add_vmhd(vmhd);
    minf.add_dinf(dinf);
    minf.add_stbl(stbl);

    // Assemble mdia
    let mut mdia = MediaBoxOwned::new();
    mdia.add_mdhd(mdhd);
    mdia.add_hdlr(hdlr);
    mdia.add_minf(minf);

    // Assemble trak
    let mut trak = TrackBoxOwned::new();
    trak.add_tkhd(tkhd);
    trak.add_edts(edts);
    trak.add_mdia(mdia);
    trak
}

// ---------------------------------------------------------------------------
// Build audio track
// ---------------------------------------------------------------------------

fn build_audio_track() -> TrackBoxOwned {
    let audio_duration = DURATION_SEC * AUDIO_TIMESCALE as u64;
    let movie_duration = DURATION_SEC * 1000;

    // tkhd
    let tkhd = {
        let mut b = TrackHeaderBoxOwned::new();
        b.flags = tkhd::flags::TRACK_ENABLED | tkhd::flags::TRACK_IN_MOVIE | tkhd::flags::TRACK_IN_PREVIEW;
        b.track_id = 2;
        b.duration = movie_duration;
        b.volume = FixedPoint8_8::ONE;
        b
    };

    // edts > elst
    let edts = {
        let mut elst = EditListBoxOwned::new();
        elst.entries.push(EditListEntry {
            segment_duration: movie_duration,
            media_time: 0,
            media_rate: FixedPoint16_16::ONE,
        });
        let mut edts = EditBoxOwned::new();
        edts.add_elst(elst);
        edts
    };

    // mdhd
    let mdhd = {
        let mut b = MediaHeaderBoxOwned::new();
        b.timescale = AUDIO_TIMESCALE;
        b.duration = audio_duration;
        b.language = IsoLanguageCode::UNDETERMINED;
        b
    };

    // hdlr
    let hdlr = HandlerReferenceBoxOwned::audio();

    // stsd – AAC sample entry
    let stsd = {
        let mut ase = AudioSampleEntryOwned::new(BoxCode::new(*b"mp4a"));
        ase.channel_count = 2;
        ase.sample_size = 16;
        ase.sample_rate = UFixedPoint16_16::from_int(48_000);
        let mut stsd = SampleDescriptionBoxOwned::new();
        stsd.entries.push(SampleDescriptionChild::Audio(ase));
        stsd
    };

    // stts – constant frame duration
    let stts = {
        let mut b = TimeToSampleBoxOwned::new();
        b.entries.push(TimeToSampleEntry {
            sample_count: AUDIO_SAMPLES,
            sample_delta: AUDIO_FRAME_DURATION,
        });
        b
    };

    // stsc – 20 samples per chunk
    let samples_per_chunk = 20u32;
    let num_chunks = AUDIO_SAMPLES.div_ceil(samples_per_chunk);
    let stsc = {
        let mut b = SampleToChunkBoxOwned::new();
        b.entries.push(SampleToChunkEntry {
            first_chunk: 1,
            samples_per_chunk,
            sample_description_index: 1,
        });
        b
    };

    // stsz – constant size AAC frames
    let stsz = SampleSizeBoxOwned::with_constant_size(400, AUDIO_SAMPLES);

    // stco
    let stco = {
        let mut b = ChunkOffsetBoxOwned::new();
        let mut offset = 100_000u32;
        for _ in 0..num_chunks {
            b.entries.push(offset);
            offset = offset.saturating_add(8_000);
        }
        b
    };

    // smhd
    let smhd = SoundMediaHeaderBoxOwned::new();

    // dinf > dref
    let dinf = {
        let dref = DataReferenceBoxOwned::new_self_contained();
        let mut dinf = DataInformationBoxOwned::new();
        dinf.add_dref(dref);
        dinf
    };

    // Assemble stbl
    let mut stbl = SampleTableBoxOwned::new();
    stbl.add_stsd(stsd);
    stbl.add_stts(stts);
    stbl.add_stsc(stsc);
    stbl.add_stsz(stsz);
    stbl.add_stco(stco);

    // Assemble minf
    let mut minf = MediaInformationBoxOwned::new();
    minf.add_smhd(smhd);
    minf.add_dinf(dinf);
    minf.add_stbl(stbl);

    // Assemble mdia
    let mut mdia = MediaBoxOwned::new();
    mdia.add_mdhd(mdhd);
    mdia.add_hdlr(hdlr);
    mdia.add_minf(minf);

    // Assemble trak
    let mut trak = TrackBoxOwned::new();
    trak.add_tkhd(tkhd);
    trak.add_edts(edts);
    trak.add_mdia(mdia);
    trak
}

// ---------------------------------------------------------------------------
// Build complete synthetic MP4 file
// ---------------------------------------------------------------------------

fn build_synthetic_mp4() -> Vec<u8> {
    let mut output = Vec::new();

    // ftyp
    let ftyp = FileTypeBoxOwned {
        major_brand: BrandCode::ISOM,
        minor_version: 0x200,
        compatible_brands: vec![BrandCode::ISOM, BrandCode::ISO2, BrandCode::MP41],
    };
    ftyp.write_to(&mut output).unwrap();

    // moov
    let mvhd = {
        let mut b = MovieHeaderBoxOwned::new();
        b.timescale = 1000;
        b.duration = DURATION_SEC * 1000;
        b.rate = FixedPoint16_16::ONE;
        b.volume = FixedPoint8_8::ONE;
        b.matrix = Matrix::IDENTITY;
        b.next_track_id = 3;
        b
    };

    let video_track = build_video_track();
    let audio_track = build_audio_track();

    let mut moov = MovieBoxOwned::new();
    moov.add_mvhd(mvhd);
    moov.add_trak(video_track);
    moov.add_trak(audio_track);
    moov.write_to(&mut output).unwrap();

    // mdat – small placeholder (the benchmark focuses on metadata parsing)
    let mdat = MediaDataBoxOwned::from_data(vec![0u8; 1024]);
    mdat.write_to(&mut output).unwrap();

    output
}

// ---------------------------------------------------------------------------
// Parsing benchmark: iterate every box and decode all entry-heavy tables
// ---------------------------------------------------------------------------

/// Full parse: iterate top-level boxes, descend into moov, parse every
/// metadata box including iterating all sample-table entries.
fn full_parse(data: &[u8]) {
    let mut total_samples: u64 = 0;
    let mut total_sync: u64 = 0;
    let mut total_chunks: u64 = 0;

    for top in BoxIterator::new(data) {
        dispatch_box!(&top, {
            FileTypeBoxView(ftyp) => {
                black_box(ftyp.major_brand());
                black_box(ftyp.minor_version());
                for i in 0..ftyp.compatible_brands_count() {
                    black_box(ftyp.compatible_brand(i));
                }
            },
            MovieBoxView(moov) => {
                for child in moov.children() {
                    dispatch_box!(&child, {
                        MovieHeaderBoxView(mvhd) => {
                            black_box(mvhd.timescale());
                            black_box(mvhd.duration());
                            black_box(mvhd.rate());
                            black_box(mvhd.matrix());
                            black_box(mvhd.next_track_id());
                        },
                        TrackBoxView(_trak) => {
                            parse_trak(child.data(), &mut total_samples, &mut total_sync, &mut total_chunks);
                        },
                        _ => {},
                        Err(_) => {},
                    });
                }
            },
            _ => {},
            Err(_) => {},
        });
    }

    black_box(total_samples);
    black_box(total_sync);
    black_box(total_chunks);
}

fn parse_trak(data: &[u8], total_samples: &mut u64, total_sync: &mut u64, total_chunks: &mut u64) {
    for child in BoxIterator::new(&data[8..]) {
        dispatch_box!(&child, {
            TrackHeaderBoxView(tkhd) => {
                black_box(tkhd.track_id());
                black_box(tkhd.duration());
                black_box(tkhd.width());
                black_box(tkhd.height());
                black_box(tkhd.matrix());
            },
            EditBoxView(edts) => {
                for edts_child in edts.children() {
                    dispatch_box!(&edts_child, {
                        EditListBoxView(elst) => {
                            for entry in elst.entries() {
                                black_box(entry);
                            }
                        },
                        _ => {},
                        Err(_) => {},
                    });
                }
            },
            MediaBoxView(mdia) => {
                for mdia_child in mdia.children() {
                    dispatch_box!(&mdia_child, {
                        MediaHeaderBoxView(mdhd) => {
                            black_box(mdhd.timescale());
                            black_box(mdhd.duration());
                            black_box(mdhd.language());
                        },
                        HandlerReferenceBoxView(hdlr) => {
                            black_box(hdlr.handler_type());
                            black_box(hdlr.name());
                        },
                        MediaInformationBoxView(minf) => {
                            for minf_child in minf.children() {
                                dispatch_box!(&minf_child, {
                                    SampleTableBoxView(_stbl) => {
                                        parse_stbl(minf_child.data(), total_samples, total_sync, total_chunks);
                                    },
                                    _ => {},
                                    Err(_) => {},
                                });
                            }
                        },
                        _ => {},
                        Err(_) => {},
                    });
                }
            },
            _ => {},
            Err(_) => {},
        });
    }
}

fn parse_stbl(data: &[u8], total_samples: &mut u64, total_sync: &mut u64, total_chunks: &mut u64) {
    for child in BoxIterator::new(&data[8..]) {
        dispatch_box!(&child, {
            SampleDescriptionBoxView(stsd) => {
                black_box(stsd.entry_count());
                for entry in stsd.entries() {
                    black_box(entry.box_type());
                }
            },
            TimeToSampleBoxView(stts) => {
                for entry in stts.entries() {
                    *total_samples += entry.sample_count as u64;
                    black_box(entry.sample_delta);
                }
            },
            CompositionTimeToSampleBoxView(ctts) => {
                for entry in ctts.entries() {
                    black_box(entry.sample_offset);
                }
            },
            SyncSampleBoxView(stss) => {
                for entry in stss.entries() {
                    *total_sync += 1;
                    black_box(entry);
                }
            },
            SampleToChunkBoxView(stsc) => {
                for entry in stsc.entries() {
                    black_box(entry);
                }
            },
            SampleSizeBoxView(stsz) => {
                for size in stsz.entries() {
                    black_box(size);
                }
            },
            ChunkOffsetBoxView(stco) => {
                for offset in stco.entries() {
                    *total_chunks += 1;
                    black_box(offset);
                }
            },
            _ => {},
            Err(_) => {},
        });
    }
}

// ---------------------------------------------------------------------------
// Build synthetic fragmented MP4 segment (moof + mdat)
// ---------------------------------------------------------------------------

/// Number of fragments to generate.
const NUM_FRAGMENTS: u32 = 10;
/// Samples per fragment - chosen so total matches non-fragmented file
/// (VIDEO_SAMPLES + AUDIO_SAMPLES = 4612).
const TOTAL_FRAGMENTED_SAMPLES: u32 = VIDEO_SAMPLES + AUDIO_SAMPLES;
const FRAGMENT_SAMPLES: u32 = TOTAL_FRAGMENTED_SAMPLES / NUM_FRAGMENTS; // 461

fn build_fragment(sequence_number: u32, base_decode_time: u64) -> (MovieFragmentBoxOwned, MediaDataBoxOwned) {
    let mfhd = MovieFragmentHeaderBoxOwned::new(sequence_number);

    let tfhd = {
        let mut b = TrackFragmentHeaderBoxOwned::new(1);
        b.default_sample_duration = Some(3000);
        b.default_sample_size = Some(10_000);
        b.default_sample_flags = Some(0x01010000);
        b.default_base_is_moof = true;
        b
    };

    let tfdt = TrackFragmentBaseMediaDecodeTimeBoxOwned::new(base_decode_time);

    // trun with per-sample sizes and composition offsets
    let trun = {
        let mut b = TrackRunBoxOwned::new();
        b.data_offset = Some(0); // placeholder
        b.first_sample_flags = Some(0x02000000);
        for i in 0..FRAGMENT_SAMPLES {
            b.samples.push(TrackRunSample {
                sample_duration: Some(3000),
                sample_size: Some(8000 + (i % 37) * 100),
                sample_flags: None,
                sample_composition_time_offset: Some(if i % 3 == 0 { 0 } else { 3000 }),
            });
        }
        b
    };

    // sdtp for the fragment
    let sdtp = {
        let mut b = SampleDependencyTypeBoxOwned::new();
        for i in 0..FRAGMENT_SAMPLES {
            b.samples.push(SampleDependencyFlags::new(
                0,
                if i == 0 { 2 } else { 1 },
                if i == 0 { 1 } else { 0 },
                0,
            ));
        }
        b
    };

    // sbgp for the fragment
    let sbgp = {
        let mut b = SampleToGroupBoxOwned::new(mp4ra_rust::FourCC(*b"roll"));
        b.entries.push(SampleToGroupEntry {
            sample_count: FRAGMENT_SAMPLES,
            group_description_index: 1,
        });
        b
    };

    // saiz – per-sample auxiliary info sizes
    let saiz = {
        let mut b = SampleAuxiliaryInformationSizesBoxOwned::new();
        let sizes: Vec<u8> = (0..FRAGMENT_SAMPLES).map(|i| 8 + (i % 4) as u8).collect();
        b.sizes = SampleInfoSizes::PerSample(sizes);
        b
    };

    // saio – one offset per sample
    let saio = {
        let mut b = SampleAuxiliaryInformationOffsetsBoxOwned::new();
        let mut offset = 1000u64;
        for _ in 0..FRAGMENT_SAMPLES {
            b.offsets.push(offset);
            offset += 16;
        }
        b
    };

    // senc – sample encryption data (8-byte IV per sample, no subsamples)
    let senc = {
        let mut b = SampleEncryptionBoxOwned::new();
        for _ in 0..FRAGMENT_SAMPLES {
            b.entries.push(SampleEncryptionEntry {
                iv: vec![0xAA; 8],
                subsamples: Vec::new(),
            });
        }
        b
    };

    // Assemble traf
    let mut traf = TrackFragmentBoxOwned::new();
    traf.add_tfhd(tfhd);
    traf.add_tfdt(tfdt);
    traf.add_trun(trun);
    traf.add_sdtp(sdtp);
    traf.add_sbgp(sbgp);
    traf.add_saiz(saiz);
    traf.add_saio(saio);
    traf.add_senc(senc);

    // Assemble moof
    let mut moof = MovieFragmentBoxOwned::new();
    moof.add_mfhd(mfhd);
    moof.add_traf(traf);

    let mdat = MediaDataBoxOwned::from_data(vec![0u8; 1024]);

    (moof, mdat)
}

fn build_fragmented_mp4() -> Vec<u8> {
    let mut output = Vec::new();

    // ftyp
    let ftyp = FileTypeBoxOwned {
        major_brand: BrandCode::ISOM,
        minor_version: 0x200,
        compatible_brands: vec![BrandCode::ISOM, BrandCode::ISO2],
    };
    ftyp.write_to(&mut output).unwrap();

    // Top-level sidx: indexes all fragments (reference_type = 1 → points to child sidx boxes).
    // In a real multi-hour DASH presentation this can have thousands of entries.
    let fragment_duration = FRAGMENT_SAMPLES as u64 * 3000;
    {
        let mut top_sidx = SegmentIndexBoxOwned::new(1, VIDEO_TIMESCALE);
        top_sidx.earliest_presentation_time = 0;
        for _ in 0..NUM_FRAGMENTS {
            top_sidx.references.push(SegmentIndexReference {
                reference_type: true, // points to child sidx
                referenced_size: 200_000,
                subsegment_duration: fragment_duration as u32,
                starts_with_sap: true,
                sap_type: 1,
                sap_delta_time: 0,
            });
        }
        top_sidx.write_to(&mut output).unwrap();
    }

    // Generate fragments, each preceded by a per-fragment sidx
    for seq in 0..NUM_FRAGMENTS {
        let base_time = seq as u64 * fragment_duration;
        let (moof, mdat) = build_fragment(seq + 1, base_time);

        // Per-fragment sidx: subsegment-level references (reference_type = 0)
        let mut sidx = SegmentIndexBoxOwned::new(1, VIDEO_TIMESCALE);
        sidx.earliest_presentation_time = base_time;
        let subsegment_count = 10u32;
        let subsegment_duration = (fragment_duration / subsegment_count as u64) as u32;
        for _ in 0..subsegment_count {
            sidx.references.push(SegmentIndexReference {
                reference_type: false,
                referenced_size: 50_000,
                subsegment_duration,
                starts_with_sap: true,
                sap_type: 1,
                sap_delta_time: 0,
            });
        }
        sidx.write_to(&mut output).unwrap();

        moof.write_to(&mut output).unwrap();
        mdat.write_to(&mut output).unwrap();
    }

    // mfra at end of file - one tfra per track, one entry per fragment
    {
        let mut tfra = TrackFragmentRandomAccessBoxOwned::new(1);
        let mut moof_offset = 0u64;
        for seq in 0..NUM_FRAGMENTS {
            tfra.entries.push(RandomAccessEntry {
                time: seq as u64 * fragment_duration,
                moof_offset,
                traf_number: 1,
                trun_number: 1,
                sample_number: 1,
            });
            moof_offset += 200_000;
        }

        let mfro = MovieFragmentRandomAccessOffsetBoxOwned::new(0); // placeholder size

        let mut mfra = MovieFragmentRandomAccessBoxOwned::new();
        mfra.add_tfra(tfra);
        mfra.add_mfro(mfro);
        mfra.write_to(&mut output).unwrap();
    }

    output
}

// ---------------------------------------------------------------------------
// Parse fragmented MP4
// ---------------------------------------------------------------------------

fn parse_fragmented(data: &[u8]) {
    let mut total_samples: u64 = 0;

    for top in BoxIterator::new(data) {
        dispatch_box!(&top, {
            SegmentIndexBoxView(sidx) => {
                black_box(sidx.reference_id());
                black_box(sidx.timescale());
                black_box(sidx.earliest_presentation_time());
                for r in sidx.references() {
                    black_box(r);
                }
            },
            MovieFragmentBoxView(moof) => {
                for child in moof.children() {
                    dispatch_box!(&child, {
                        TrackFragmentBoxView(traf) => {
                            parse_traf(child.data(), &mut total_samples);
                        },
                        _ => {},
                        Err(_) => {},
                    });
                }
            },
            MovieFragmentRandomAccessBoxView(mfra) => {
                for child in mfra.children() {
                    dispatch_box!(&child, {
                        TrackFragmentRandomAccessBoxView(tfra) => {
                            black_box(tfra.track_id());
                            for entry in tfra.entries() {
                                black_box(entry);
                            }
                        },
                        _ => {},
                        Err(_) => {},
                    });
                }
            },
            _ => {},
            Err(_) => {},
        });
    }

    black_box(total_samples);
}

fn parse_traf(data: &[u8], total_samples: &mut u64) {
    for child in BoxIterator::new(&data[8..]) {
        dispatch_box!(&child, {
            TrackFragmentHeaderBoxView(tfhd) => {
                black_box(tfhd.track_id());
                black_box(tfhd.default_sample_duration());
                black_box(tfhd.default_sample_size());
            },
            TrackFragmentBaseMediaDecodeTimeBoxView(tfdt) => {
                black_box(tfdt.base_media_decode_time());
            },
            TrackRunBoxView(trun) => {
                for sample in trun.samples() {
                    *total_samples += 1;
                    black_box(sample);
                }
            },
            SampleDependencyTypeBoxView(sdtp) => {
                for flags in sdtp.all_sample_flags() {
                    black_box(flags);
                }
            },
            SampleToGroupBoxView(sbgp) => {
                for entry in sbgp.entries() {
                    black_box(entry);
                }
            },
            SampleAuxiliaryInformationSizesBoxView(saiz) => {
                for s in saiz.sample_info_sizes() {
                    black_box(s);
                }
            },
            SampleAuxiliaryInformationOffsetsBoxView(saio) => {
                for o in saio.offsets() {
                    black_box(o);
                }
            },
            SampleEncryptionBoxView(senc) => {
                for sample in senc.samples(8).flatten() {
                    black_box(sample);
                }
            },
            _ => {},
            Err(_) => {},
        });
    }
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

fn bench_parse(c: &mut Criterion) {
    let data = build_synthetic_mp4();

    let mut group = c.benchmark_group("parse_synthetic_mp4");
    group.throughput(criterion::Throughput::Bytes(data.len() as u64));

    group.bench_function("full_parse", |b| {
        b.iter(|| full_parse(black_box(&data)));
    });

    group.finish();

    let frag_data = build_fragmented_mp4();

    let mut frag_group = c.benchmark_group("parse_fragmented_mp4");
    frag_group.throughput(criterion::Throughput::Bytes(frag_data.len() as u64));

    frag_group.bench_function("full_parse", |b| {
        b.iter(|| parse_fragmented(black_box(&frag_data)));
    });

    frag_group.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
