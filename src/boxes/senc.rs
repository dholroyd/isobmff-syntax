//! Sample Encryption Box (senc) parsing and serialization.
//!
//! The Sample Encryption Box contains encryption information for samples.
//!
//! ```text
//! aligned(8) class SampleEncryptionBox
//!    extends FullBox('senc', version, flags) {
//!    unsigned int(32) sample_count;
//!    {
//!       unsigned int(Per_Sample_IV_Size*8) InitializationVector;
//!       if (flags & 0x000002) {
//!          unsigned int(16) subsample_count;
//!          {
//!             unsigned int(16) BytesOfClearData;
//!             unsigned int(32) BytesOfProtectedData;
//!          } [subsample_count]
//!       }
//!    } [sample_count]
//! }
//! ```

use crate::error::ParseError;
use crate::header::{FullBoxHeader, fullbox_header_size_for_payload, write_fullbox_header};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// The box type identifier for SampleEncryptionBox.
pub const BOX_TYPE: BoxCode = BoxCode::SENC;

/// Flag indicating sub-sample encryption.
pub const FLAG_USE_SUBSAMPLE_ENCRYPTION: u32 = 0x000002;

/// A subsample encryption entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubsampleEncryptionEntry {
    /// Number of bytes of clear (unencrypted) data.
    pub bytes_of_clear_data: u16,
    /// Number of bytes of protected (encrypted) data.
    pub bytes_of_protected_data: u32,
}

/// A per-sample encryption entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleEncryptionEntry {
    /// Initialization vector for this sample.
    pub iv: Vec<u8>,
    /// Subsample encryption ranges (empty if subsample encryption is not used).
    pub subsamples: Vec<SubsampleEncryptionEntry>,
}

/// A borrowing view over a single sample encryption entry.
#[derive(Clone, Copy, Debug)]
pub struct SampleEncryptionEntryView<'a> {
    iv: &'a [u8],
    subsamples_data: &'a [u8],
}

impl<'a> SampleEncryptionEntryView<'a> {
    /// Returns the initialization vector for this sample.
    #[inline]
    pub fn iv(&self) -> &'a [u8] {
        self.iv
    }

    /// Returns an iterator over the subsample encryption entries.
    pub fn subsamples(&self) -> impl Iterator<Item = SubsampleEncryptionEntry> + 'a {
        self.subsamples_data.chunks_exact(6).map(|chunk| SubsampleEncryptionEntry {
            bytes_of_clear_data: BigEndian::read_u16(&chunk[..2]),
            bytes_of_protected_data: BigEndian::read_u32(&chunk[2..6]),
        })
    }

    /// Materialises an owned copy of this entry.
    pub fn to_owned(&self) -> SampleEncryptionEntry {
        SampleEncryptionEntry {
            iv: self.iv.to_vec(),
            subsamples: self.subsamples().collect(),
        }
    }
}

/// Iterator over sample encryption entries, yielding borrowing views.
struct SampleEncryptionIter<'a> {
    data: &'a [u8],
    offset: usize,
    remaining: u32,
    iv_size: usize,
    uses_subsamples: bool,
}

impl<'a> Iterator for SampleEncryptionIter<'a> {
    type Item = Result<SampleEncryptionEntryView<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        let iv_end = match self.offset.checked_add(self.iv_size) {
            Some(end) if end <= self.data.len() => end,
            _ => {
                self.remaining = 0;
                return Some(Err(ParseError::BufferTooShort {
                    expected: self.offset.saturating_add(self.iv_size),
                    found: self.data.len(),
                }));
            }
        };
        let iv = &self.data[self.offset..iv_end];
        self.offset = iv_end;

        let subsamples_data = if self.uses_subsamples {
            if self.offset.checked_add(2).is_none_or(|end| end > self.data.len()) {
                self.remaining = 0;
                return Some(Err(ParseError::BufferTooShort {
                    expected: self.offset.saturating_add(2),
                    found: self.data.len(),
                }));
            }
            let subsample_count = BigEndian::read_u16(&self.data[self.offset..self.offset + 2]) as usize;
            self.offset += 2;

            let total = match subsample_count.checked_mul(6) {
                Some(t) => t,
                None => {
                    self.remaining = 0;
                    return Some(Err(ParseError::BufferTooShort {
                        expected: usize::MAX,
                        found: self.data.len(),
                    }));
                }
            };
            if self.offset.checked_add(total).is_none_or(|end| end > self.data.len()) {
                self.remaining = 0;
                return Some(Err(ParseError::BufferTooShort {
                    expected: self.offset.saturating_add(total),
                    found: self.data.len(),
                }));
            }
            let subs = &self.data[self.offset..self.offset + total];
            self.offset += total;
            subs
        } else {
            &[]
        };

        Some(Ok(SampleEncryptionEntryView { iv, subsamples_data }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining as usize))
    }
}

/// Common interface for accessing SampleEncryptionBox data.
pub trait SampleEncryptionBox {
    /// Returns the total size of the box in bytes.
    fn box_size(&self) -> u64;

    /// Returns the box type.
    fn box_type(&self) -> BoxCode;

    /// Returns the version of the box.
    fn version(&self) -> u8;

    /// Returns the flags.
    fn flags(&self) -> u32;

    /// Returns the sample count.
    fn sample_count(&self) -> u32;

    /// Checks if subsample encryption is used (flag 0x2).
    fn uses_subsample_encryption(&self) -> bool {
        self.flags() & FLAG_USE_SUBSAMPLE_ENCRYPTION != 0
    }
}

/// A borrowing view over raw SampleEncryptionBox bytes.
#[derive(Clone, Copy)]
pub struct SampleEncryptionBoxView<'a> {
    data: &'a [u8],
    fullbox_offset: usize,
}

impl<'a> SampleEncryptionBoxView<'a> {
    /// Creates a new view over the given bytes.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = FullBoxHeader::parse(data, data.len())?;
        let fullbox_offset = header.validate(data, BOX_TYPE, None, 4)?;
        Ok(Self { data, fullbox_offset })
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    fn sample_data(&self) -> &'a [u8] {
        let start = self.fullbox_offset + 4 + 4;
        if start < self.data.len() {
            &self.data[start..]
        } else {
            &[]
        }
    }

    /// Returns an iterator over the sample encryption entries.
    ///
    /// `iv_size` is the Per_Sample_IV_Size from the associated TrackEncryptionBox (tenc).
    pub fn samples(&self, iv_size: u8) -> impl Iterator<Item = Result<SampleEncryptionEntryView<'a>, ParseError>> + 'a {
        SampleEncryptionIter {
            data: self.sample_data(),
            offset: 0,
            remaining: self.sample_count(),
            iv_size: iv_size as usize,
            uses_subsamples: self.uses_subsample_encryption(),
        }
    }

}

impl<'a> SampleEncryptionBox for SampleEncryptionBoxView<'a> {
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

    fn sample_count(&self) -> u32 {
        BigEndian::read_u32(&self.data[self.fullbox_offset + 4..self.fullbox_offset + 8])
    }
}

impl std::fmt::Debug for SampleEncryptionBoxView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleEncryptionBoxView")
            .field("sample_count", &self.sample_count())
            .field("uses_subsample_encryption", &self.uses_subsample_encryption())
            .finish()
    }
}

/// An owned representation of SampleEncryptionBox data.
///
/// Supports two modes:
/// - **Raw**: stores the original sample data bytes and sample count for lossless roundtrip
///   when the IV size is not available. Created by the generic `From<&T>` conversion.
/// - **Structured**: stores parsed per-sample entries. Created by `from_view()` when
///   the IV size is known. Use `write_to()` with `iv_size` for serialization.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct SampleEncryptionBoxOwned {
    /// Flags.
    pub flags: u32,
    /// Per-sample encryption entries (populated when parsed with iv_size).
    pub entries: Vec<SampleEncryptionEntry>,
    /// Raw sample data (used for lossless roundtrip when iv_size is unknown).
    sample_data: Vec<u8>,
    /// Sample count from raw data.
    sample_count: u32,
}


impl SampleEncryptionBoxOwned {
    /// Creates a new SampleEncryptionBoxOwned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an owned representation from a view, parsing entries with the given IV size.
    pub fn from_view(source: &SampleEncryptionBoxView<'_>, iv_size: u8) -> Result<Self, ParseError> {
        let entries: Vec<SampleEncryptionEntry> = source.samples(iv_size)
            .map(|r| r.map(|v| v.to_owned()))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            flags: source.flags(),
            entries,
            sample_data: Vec::new(),
            sample_count: 0,
        })
    }

    /// Returns the serialized size of the box.
    fn serialized_size_raw(&self) -> u64 {
        let payload = (4 + self.sample_data.len()) as u64;
        fullbox_header_size_for_payload(payload) + payload
    }

    /// Returns the serialized payload size for the given IV size (structured mode).
    fn payload_size_structured(&self, iv_size: u8) -> u64 {
        let iv_size = iv_size as u64;
        let uses_subsamples = self.flags & FLAG_USE_SUBSAMPLE_ENCRYPTION != 0;
        let mut size: u64 = 4; // sample_count
        for entry in &self.entries {
            size += iv_size;
            if uses_subsamples {
                size += 2; // subsample_count
                size += entry.subsamples.len() as u64 * 6;
            }
        }
        size
    }

    /// Returns the serialized size of the box for the given IV size (structured mode).
    pub fn serialized_size(&self, iv_size: u8) -> u64 {
        let payload = self.payload_size_structured(iv_size);
        fullbox_header_size_for_payload(payload) + payload
    }

    fn is_structured(&self) -> bool {
        !self.entries.is_empty() || self.sample_data.is_empty()
    }

    /// Writes the box to the given writer.
    ///
    /// If the box was created with `from_view()` (structured mode), `iv_size` determines
    /// the length of each IV. If created via `From<&T>` (raw mode), the raw bytes are
    /// written directly and `iv_size` is ignored.
    pub fn write_to<W: Write>(&self, writer: &mut W, iv_size: u8) -> io::Result<()> {
        if !self.is_structured() {
            // Raw mode: write original bytes
            let size = self.serialized_size_raw();
            write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
            writer.write_u32::<BigEndian>(self.sample_count)?;
            writer.write_all(&self.sample_data)?;
            return Ok(());
        }

        // Structured mode
        let size = self.serialized_size(iv_size);
        write_fullbox_header(writer, size, BOX_TYPE, 0, self.flags)?;
        writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

        let uses_subsamples = self.flags & FLAG_USE_SUBSAMPLE_ENCRYPTION != 0;
        let iv_len = iv_size as usize;

        for entry in &self.entries {
            if entry.iv.len() >= iv_len {
                writer.write_all(&entry.iv[..iv_len])?;
            } else {
                writer.write_all(&entry.iv)?;
                for _ in 0..iv_len - entry.iv.len() {
                    writer.write_u8(0)?;
                }
            }

            if uses_subsamples {
                writer.write_u16::<BigEndian>(entry.subsamples.len() as u16)?;
                for sub in &entry.subsamples {
                    writer.write_u16::<BigEndian>(sub.bytes_of_clear_data)?;
                    writer.write_u32::<BigEndian>(sub.bytes_of_protected_data)?;
                }
            }
        }

        Ok(())
    }
}

impl SampleEncryptionBox for SampleEncryptionBoxOwned {
    fn box_size(&self) -> u64 {
        if self.is_structured() {
            self.serialized_size(8) // default assumption
        } else {
            self.serialized_size_raw()
        }
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

    fn sample_count(&self) -> u32 {
        if self.is_structured() {
            self.entries.len() as u32
        } else {
            self.sample_count
        }
    }
}

impl From<&SampleEncryptionBoxView<'_>> for SampleEncryptionBoxOwned {
    fn from(source: &SampleEncryptionBoxView<'_>) -> Self {
        Self {
            flags: source.flags(),
            entries: Vec::new(),
            sample_data: source.sample_data().to_vec(),
            sample_count: source.sample_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_senc() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes()); // 8 + 4 + 4
        data.extend_from_slice(b"senc");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags
        data.extend_from_slice(&0u32.to_be_bytes()); // sample_count
        data
    }

    fn make_senc_with_samples() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 (header) + 4 (version/flags) + 4 (sample_count) + 8 (IV) = 24
        data.extend_from_slice(&24u32.to_be_bytes());
        data.extend_from_slice(b"senc");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 0]); // flags (no subsamples)
        data.extend_from_slice(&1u32.to_be_bytes()); // sample_count = 1
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]); // 8-byte IV
        data
    }

    fn make_senc_with_subsamples() -> Vec<u8> {
        let mut data = Vec::new();
        // 8 + 4 + 4 + 8 + 2 + 6 = 32
        data.extend_from_slice(&32u32.to_be_bytes());
        data.extend_from_slice(b"senc");
        data.push(0); // version
        data.extend_from_slice(&[0, 0, 2]); // flags = subsample encryption
        data.extend_from_slice(&1u32.to_be_bytes()); // sample_count = 1
        data.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]); // 8-byte IV
        data.extend_from_slice(&1u16.to_be_bytes()); // subsample_count = 1
        data.extend_from_slice(&100u16.to_be_bytes()); // BytesOfClearData
        data.extend_from_slice(&200u32.to_be_bytes()); // BytesOfProtectedData
        data
    }

    #[test]
    fn parse_senc() {
        let data = make_senc();
        let view = SampleEncryptionBoxView::new(&data).unwrap();
        assert_eq!(view.sample_count(), 0);
        assert!(!view.uses_subsample_encryption());
    }

    #[test]
    fn parse_samples() {
        let data = make_senc_with_samples();
        let view = SampleEncryptionBoxView::new(&data).unwrap();
        let entries: Vec<_> = view.samples(8).collect::<Result<_, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].iv(), &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(entries[0].subsamples().count(), 0);
    }

    #[test]
    fn parse_samples_with_subsamples() {
        let data = make_senc_with_subsamples();
        let view = SampleEncryptionBoxView::new(&data).unwrap();
        assert!(view.uses_subsample_encryption());
        let entries: Vec<_> = view.samples(8).collect::<Result<_, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].iv(), &[1, 2, 3, 4, 5, 6, 7, 8]);
        let subs: Vec<_> = entries[0].subsamples().collect();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].bytes_of_clear_data, 100);
        assert_eq!(subs[0].bytes_of_protected_data, 200);
    }

    #[test]
    fn roundtrip_empty() {
        let data = make_senc();
        let view = SampleEncryptionBoxView::new(&data).unwrap();
        let owned = SampleEncryptionBoxOwned::from_view(&view, 8).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output, 8).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_samples() {
        let data = make_senc_with_samples();
        let view = SampleEncryptionBoxView::new(&data).unwrap();
        let owned = SampleEncryptionBoxOwned::from_view(&view, 8).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output, 8).unwrap();

        assert_eq!(data, output);
    }

    #[test]
    fn roundtrip_with_subsamples() {
        let data = make_senc_with_subsamples();
        let view = SampleEncryptionBoxView::new(&data).unwrap();
        let owned = SampleEncryptionBoxOwned::from_view(&view, 8).unwrap();

        let mut output = Vec::new();
        owned.write_to(&mut output, 8).unwrap();

        assert_eq!(data, output);
    }
}
