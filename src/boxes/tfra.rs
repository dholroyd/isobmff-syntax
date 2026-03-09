//! Track Fragment Random Access Box (tfra) parsing and serialization.
//!
//! The Track Fragment Random Access Box provides random access information
//! for a specific track.
//!
//! ```text
//! aligned(8) class TrackFragmentRandomAccessBox
//!    extends FullBox('tfra', version, 0) {
//!    unsigned int(32) track_ID;
//!    const unsigned int(26) reserved = 0;
//!    unsigned int(2) length_size_of_traf_num;
//!    unsigned int(2) length_size_of_trun_num;
//!    unsigned int(2) length_size_of_sample_num;
//!    unsigned int(32) number_of_entry;
//!    for(i=1; i <= number_of_entry; i++){
//!       if(version==1){
//!          unsigned int(64) time;
//!          unsigned int(64) moof_offset;
//!       }else{
//!          unsigned int(32) time;
//!          unsigned int(32) moof_offset;
//!       }
//!       unsigned int((length_size_of_traf_num+1) * 8) traf_number;
//!       unsigned int((length_size_of_trun_num+1) * 8) trun_number;
//!       unsigned int((length_size_of_sample_num+1) * 8) sample_number;
//!    }
//! }
//! ```

use crate::error::{ParseError, validate_entry_count};
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for TrackFragmentRandomAccessBox.
pub const BOX_TYPE: BoxCode = BoxCode::TFRA;

/// Reads a 1–4 byte big-endian unsigned integer from the start of `data`.
#[inline]
fn read_variable_size(data: &[u8], size: u8) -> u32 {
    match size {
        1 => data[0] as u32,
        2 => BigEndian::read_u16(&data[..2]) as u32,
        3 => BigEndian::read_u24(&data[..3]),
        4 => BigEndian::read_u32(&data[..4]),
        _ => 0,
    }
}

/// A single random access entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomAccessEntry {
    /// Time of the sync sample.
    pub time: u64,
    /// Moof offset.
    pub moof_offset: u64,
    /// TRAF number (1-based).
    pub traf_number: u32,
    /// TRUN number (1-based).
    pub trun_number: u32,
    /// Sample number (1-based).
    pub sample_number: u32,
}

/// Common interface for accessing TrackFragmentRandomAccessBox data.
pub trait TrackFragmentRandomAccessBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the track ID.
    fn track_id(&self) -> u32;

    /// Returns the number of entries.
    fn number_of_entries(&self) -> u32;

    /// Returns an iterator over all entries.
    fn entries(&self) -> impl Iterator<Item = RandomAccessEntry> + '_;
}

/// A borrowing view over raw TrackFragmentRandomAccessBox bytes.
#[derive(Clone, Copy)]
pub struct TrackFragmentRandomAccessBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
    version: u8,
    length_size_of_traf_num: u8,
    length_size_of_trun_num: u8,
    length_size_of_sample_num: u8,
    number_of_entries: u32,
    entries_offset: usize,
}

impl<'a> TrackFragmentRandomAccessBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let version = header.version;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 12)?;

        let lengths_offset = fullbox_offset + 8;
        let lengths = BigEndian::read_u32(&data[lengths_offset..lengths_offset + 4]);
        let length_size_of_traf_num = ((lengths >> 4) & 0x3) as u8 + 1;
        let length_size_of_trun_num = ((lengths >> 2) & 0x3) as u8 + 1;
        let length_size_of_sample_num = (lengths & 0x3) as u8 + 1;

        let number_of_entries =
            BigEndian::read_u32(&data[lengths_offset + 4..lengths_offset + 8]);
        let entries_offset = fullbox_offset + 16;

        // Each entry: time (4 or 8) + moof_offset (4 or 8) + traf + trun + sample
        let time_offset_size = if version == 1 { 16 } else { 8 };
        let entry_size = time_offset_size
            + length_size_of_traf_num as usize
            + length_size_of_trun_num as usize
            + length_size_of_sample_num as usize;
        validate_entry_count(data, entries_offset, number_of_entries, entry_size)?;

        Ok(Self {
            data,
            fullbox_offset,
            version,
            length_size_of_traf_num,
            length_size_of_trun_num,
            length_size_of_sample_num,
            number_of_entries,
            entries_offset,
        })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    fn entry_size(&self) -> usize {
        let time_offset_size = if self.version == 1 { 16 } else { 8 };
        time_offset_size
            + self.length_size_of_traf_num as usize
            + self.length_size_of_trun_num as usize
            + self.length_size_of_sample_num as usize
    }

    fn read_variable_size(&self, offset: usize, size: u8) -> u32 {
        read_variable_size(&self.data[offset..], size)
    }

    /// Returns the entry at the given index.
    pub fn entry(&self, index: usize) -> Option<RandomAccessEntry> {
        if index >= self.number_of_entries as usize {
            return None;
        }

        let entry_size = self.entry_size();
        let mut o = self.entries_offset + index * entry_size;

        let (time, moof_offset) = if self.version == 1 {
            let time = BigEndian::read_u64(&self.data[o..o + 8]);
            o += 8;
            let moof_offset = BigEndian::read_u64(&self.data[o..o + 8]);
            o += 8;
            (time, moof_offset)
        } else {
            let time = BigEndian::read_u32(&self.data[o..o + 4]) as u64;
            o += 4;
            let moof_offset = BigEndian::read_u32(&self.data[o..o + 4]) as u64;
            o += 4;
            (time, moof_offset)
        };

        let traf_number = self.read_variable_size(o, self.length_size_of_traf_num);
        o += self.length_size_of_traf_num as usize;

        let trun_number = self.read_variable_size(o, self.length_size_of_trun_num);
        o += self.length_size_of_trun_num as usize;

        let sample_number = self.read_variable_size(o, self.length_size_of_sample_num);

        Some(RandomAccessEntry {
            time,
            moof_offset,
            traf_number,
            trun_number,
            sample_number,
        })
    }

}

impl TrackFragmentRandomAccessBox for TrackFragmentRandomAccessBoxView<'_> {
    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn flags(&self) -> u32 {
        BigEndian::read_u24(&self.data[self.fullbox_offset + 1..self.fullbox_offset + 4])
    }

    fn track_id(&self) -> u32 {
        let o = self.fullbox_offset + 4;
        BigEndian::read_u32(&self.data[o..o + 4])
    }

    fn number_of_entries(&self) -> u32 {
        self.number_of_entries
    }

    fn entries(&self) -> impl Iterator<Item = RandomAccessEntry> + '_ {
        let entry_size = self.entry_size();
        let count = self.number_of_entries as usize;
        let version = self.version;
        let traf_size = self.length_size_of_traf_num;
        let trun_size = self.length_size_of_trun_num;
        let sample_size = self.length_size_of_sample_num;

        // Precompute field offsets within each entry.
        let time_field_size = if version == 1 { 8usize } else { 4 };
        let time_off = 0;
        let moof_off = time_field_size;
        let traf_off = moof_off + time_field_size;
        let trun_off = traf_off + traf_size as usize;
        let sample_off = trun_off + trun_size as usize;

        let end = self.entries_offset + count * entry_size;
        self.data[self.entries_offset..end]
            .chunks_exact(entry_size)
            .map(move |chunk| {
                let (time, moof_offset) = if version == 1 {
                    (
                        BigEndian::read_u64(&chunk[time_off..time_off + 8]),
                        BigEndian::read_u64(&chunk[moof_off..moof_off + 8]),
                    )
                } else {
                    (
                        BigEndian::read_u32(&chunk[time_off..time_off + 4]) as u64,
                        BigEndian::read_u32(&chunk[moof_off..moof_off + 4]) as u64,
                    )
                };
                RandomAccessEntry {
                    time,
                    moof_offset,
                    traf_number: read_variable_size(&chunk[traf_off..], traf_size),
                    trun_number: read_variable_size(&chunk[trun_off..], trun_size),
                    sample_number: read_variable_size(&chunk[sample_off..], sample_size),
                }
            })
    }
}

impl std::fmt::Debug for TrackFragmentRandomAccessBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackFragmentRandomAccessBoxView")
            .field("track_id", &self.track_id())
            .field("number_of_entries", &self.number_of_entries())
            .finish()
    }
}

/// An owned representation of TrackFragmentRandomAccessBox data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackFragmentRandomAccessBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Track ID.
    pub track_id: u32,
    /// Entries.
    pub entries: Vec<RandomAccessEntry>,
}

impl TrackFragmentRandomAccessBoxOwned {
    /// Creates a new TrackFragmentRandomAccessBoxOwned.
    pub fn new(track_id: u32) -> Self {
        Self {
            flags: 0,
            track_id,
            entries: Vec::new(),
        }
    }

    fn requires_v1(&self) -> bool {
        self.entries.iter().any(|e| {
            e.time > u32::MAX as u64 || e.moof_offset > u32::MAX as u64
        })
    }

    fn compute_length_sizes(&self) -> (u8, u8, u8) {
        let max_traf = self.entries.iter().map(|e| e.traf_number).max().unwrap_or(0);
        let max_trun = self.entries.iter().map(|e| e.trun_number).max().unwrap_or(0);
        let max_sample = self.entries.iter().map(|e| e.sample_number).max().unwrap_or(0);

        let traf_size = if max_traf <= 0xFF { 1 } else if max_traf <= 0xFFFF { 2 } else { 4 };
        let trun_size = if max_trun <= 0xFF { 1 } else if max_trun <= 0xFFFF { 2 } else { 4 };
        let sample_size = if max_sample <= 0xFF { 1 } else if max_sample <= 0xFFFF { 2 } else { 4 };

        (traf_size, trun_size, sample_size)
    }

    /// Returns the serialized size of the box.
    fn serialized_size(&self) -> u64 {
        let v1 = self.requires_v1();
        let (traf_size, trun_size, sample_size) = self.compute_length_sizes();
        let time_offset_size = if v1 { 16 } else { 8 };
        let entry_size = time_offset_size + traf_size as usize + trun_size as usize + sample_size as usize;
        let payload = (12 + self.entries.len() * entry_size) as u64; // track_id(4) + lengths(4) + number_of_entries(4) + entries
        fullbox_header_size_for_payload(payload) + payload
    }

    fn write_variable_size<W: Write>(&self, writer: &mut W, value: u32, size: u8) -> io::Result<()> {
        match size {
            1 => writer.write_u8(value as u8),
            2 => writer.write_u16::<BigEndian>(value as u16),
            3 => writer.write_u24::<BigEndian>(value),
            4 => writer.write_u32::<BigEndian>(value),
            _ => Ok(()),
        }
    }

    /// Writes the box to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let v1 = self.requires_v1();
        let size = self.serialized_size();
        let version = if v1 { 1 } else { 0 };
        let (traf_size, trun_size, sample_size) = self.compute_length_sizes();
        write_fullbox_header(writer, size, BOX_TYPE, version, self.flags)?;
        writer.write_u32::<BigEndian>(self.track_id)?;

        // reserved(26 bits) + length_size_of_traf_num(2) + length_size_of_trun_num(2) + length_size_of_sample_num(2)
        let lengths = ((traf_size - 1) as u32 & 0x3) << 4
            | ((trun_size - 1) as u32 & 0x3) << 2
            | ((sample_size - 1) as u32 & 0x3);
        writer.write_u32::<BigEndian>(lengths)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        for entry in &self.entries {
            if v1 {
                writer.write_u64::<BigEndian>(entry.time)?;
                writer.write_u64::<BigEndian>(entry.moof_offset)?;
            } else {
                writer.write_u32::<BigEndian>(entry.time as u32)?;
                writer.write_u32::<BigEndian>(entry.moof_offset as u32)?;
            }
            self.write_variable_size(writer, entry.traf_number, traf_size)?;
            self.write_variable_size(writer, entry.trun_number, trun_size)?;
            self.write_variable_size(writer, entry.sample_number, sample_size)?;
        }

        Ok(())
    }
}

impl Default for TrackFragmentRandomAccessBoxOwned {
    fn default() -> Self {
        Self::new(1)
    }
}

impl TrackFragmentRandomAccessBox for TrackFragmentRandomAccessBoxOwned {
    fn box_size(&self) -> u64 {
        self.serialized_size()
    }

    fn box_type(&self) -> BoxCode {
        BOX_TYPE
    }

    fn version(&self) -> u8 {
        if self.requires_v1() { 1 } else { 0 }
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn track_id(&self) -> u32 {
        self.track_id
    }

    fn number_of_entries(&self) -> u32 {
        self.entries.len() as u32
    }

    fn entries(&self) -> impl Iterator<Item = RandomAccessEntry> + '_ {
        self.entries.iter().copied()
    }
}

impl<T: TrackFragmentRandomAccessBox> From<&T> for TrackFragmentRandomAccessBoxOwned {
    fn from(source: &T) -> Self {
        Self {
            flags: source.flags(),
            track_id: source.track_id(),
            entries: source.entries().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tfra_v0() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&35u32.to_be_bytes()); // size = 8 + 4 + 12 + 11
        data.extend_from_slice(b"tfra");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&1u32.to_be_bytes()); // track_id
        // lengths: traf=1, trun=1, sample=1 (all encoded as 0 meaning size 1)
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes()); // number_of_entries

        // Entry: time(4) + moof_offset(4) + traf(1) + trun(1) + sample(1)
        data.extend_from_slice(&1000u32.to_be_bytes()); // time
        data.extend_from_slice(&5000u32.to_be_bytes()); // moof_offset
        data.push(1); // traf_number
        data.push(1); // trun_number
        data.push(1); // sample_number

        data
    }

    #[test]
    fn parse_tfra_v0() {
        let data = make_tfra_v0();
        let view = TrackFragmentRandomAccessBoxView::new(&data).unwrap();

        assert_eq!(view.version(), 0);
        assert_eq!(view.track_id(), 1);
        assert_eq!(view.number_of_entries(), 1);

        let entry = view.entry(0).unwrap();
        assert_eq!(entry.time, 1000);
        assert_eq!(entry.moof_offset, 5000);
        assert_eq!(entry.traf_number, 1);
        assert_eq!(entry.trun_number, 1);
        assert_eq!(entry.sample_number, 1);
    }

    #[test]
    fn roundtrip_v0() {
        let data = make_tfra_v0();
        let view = TrackFragmentRandomAccessBoxView::new(&data).unwrap();
        let owned = TrackFragmentRandomAccessBoxOwned::from(&view);

        let mut output = Vec::new();
        owned.write_to(&mut output).unwrap();

        assert_eq!(data, output);
    }
}
