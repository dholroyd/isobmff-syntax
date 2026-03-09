//! Streaming box iteration for large files.
//!
//! This module provides types for iterating over boxes in a stream without
//! loading entire files into memory. Only box headers are read during iteration;
//! box payloads can be loaded on demand or skipped entirely.
//!
//! # Scope bounds enforcement
//!
//! When iterating within a container (after [`enter_container`]), the parent
//! box's declared size establishes an authoritative boundary for its children.
//! If [`next_info`] encounters a child box whose declared size exceeds the
//! remaining bytes in the parent scope, it returns
//! [`Err(ParseError::BoxExceedsScope)`](crate::error::ParseError::BoxExceedsScope)
//! rather than returning the box or silently truncating it.
//!
//! This is a deliberate design choice. Many MP4 parsers (notably FFmpeg) silently
//! clamp an oversized child to fit the parent and continue. This library takes a
//! stricter approach: a box whose header claims more bytes than are available is
//! assumed to be corrupt, and handing its (necessarily incomplete) payload to the
//! caller would force every consumer to defend against truncated data. Returning
//! an error lets the caller decide how to respond - a linter can report the
//! structural violation, while a player could catch the error and call
//! [`exit_container`] to resume parsing at the parent's next sibling.
//!
//! The parent's size is trusted over the child's because it was already validated
//! against *its* parent (or against the file size at the top level), forming a
//! chain of checked boundaries. The child's size is an unchecked claim that is
//! provably wrong when it exceeds the space available.
//!
//! After a `BoxExceedsScope` error, the iterator advances to the end of the
//! current scope. Subsequent calls to `next_info` will return `Ok(None)`.
//! Call `exit_container` to resume iteration in the parent scope.
//!
//! [`enter_container`]: StreamingBoxIterator::enter_container
//! [`next_info`]: StreamingBoxIterator::next_info
//! [`exit_container`]: StreamingBoxIterator::exit_container

use byteorder::{BigEndian, ByteOrder};
use crate::error::{BoxReadError, ParseError};
use mp4ra_rust::BoxCode;
use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroU64;

/// Maximum container nesting depth allowed by iterators.
///
/// This prevents unbounded `scope_stack` growth from deeply nested containers
/// in crafted files. Real-world ISOBMFF files rarely exceed 10 levels.
const MAX_NESTING_DEPTH: usize = 128;

/// The declared size of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSize {
    /// A known size (from the 32-bit or 64-bit extended size field).
    Known(NonZeroU64),
    /// size=0: box extends to the end of its containing scope.
    #[default]
    ToEnd,
}

/// Information about a box in the file without its payload.
#[derive(Debug, Clone)]
pub struct BoxInfo {
    /// The box type (four-character code).
    pub box_type: BoxCode,
    /// Declared size of the box.
    pub size: BoxSize,
    /// Size of the header (8 for standard, 16 for extended size).
    pub header_size: u8,
    /// Offset of the box start in the file.
    pub offset: u64,
}

impl BoxInfo {
    /// Returns the offset of the payload data.
    pub fn payload_offset(&self) -> u64 {
        self.offset + self.header_size as u64
    }
}

/// An iterator over boxes in a stream, reading only headers.
///
/// This allows iterating over boxes in large files without loading
/// the entire file into memory. Box payloads can be loaded on demand
/// or skipped entirely using [`skip_box`](Self::skip_box).
///
/// # Example
///
/// ```no_run
/// use isobmff_syntax::streaming::StreamingBoxIterator;
/// use std::fs::File;
/// use std::io::BufReader;
///
/// let file = File::open("video.mp4")?;
/// let reader = BufReader::new(file);
/// let mut iter = StreamingBoxIterator::new(reader)?;
///
/// while let Some(info) = iter.next_info()? {
///     println!("Box: {:?} at offset {} (size {:?})", info.box_type, info.offset, info.size);
///     // Skip past this box to continue iteration
///     iter.skip_box(&info)?;
/// }
/// # Ok::<(), isobmff_syntax::error::BoxReadError>(())
/// ```
pub struct StreamingBoxIterator<R> {
    reader: R,
    current_offset: u64,
    file_size: u64,
    /// The end boundary for the current scope (defaults to file_size).
    end_offset: u64,
    /// Stack of parent end_offsets for nested container scoping.
    scope_stack: Vec<u64>,
    /// Maximum size allowed for `load_box`/`load_payload` allocations.
    max_load_size: u64,
}

/// Default maximum size for `load_box`/`load_payload` (256 MB).
const DEFAULT_MAX_LOAD_SIZE: u64 = 256 * 1024 * 1024;

impl<R: Read + Seek> StreamingBoxIterator<R> {
    /// Creates a new streaming iterator.
    ///
    /// The reader's position is reset to the beginning of the stream.
    pub fn new(mut reader: R) -> Result<Self, BoxReadError> {
        // Get file size
        let file_size = reader.seek(SeekFrom::End(0))?;
        // Seek back to start
        reader.seek(SeekFrom::Start(0))?;

        Ok(Self {
            reader,
            current_offset: 0,
            file_size,
            end_offset: file_size,
            scope_stack: Vec::new(),
            max_load_size: DEFAULT_MAX_LOAD_SIZE,
        })
    }

    /// Sets the maximum size allowed for [`load_box`](Self::load_box) and
    /// [`load_payload`](Self::load_payload) allocations.
    ///
    /// Defaults to 256 MB. Calls that would exceed this limit return
    /// [`ParseError::BoxTooLargeToLoad`].
    pub fn set_max_load_size(&mut self, max: u64) {
        self.max_load_size = max;
    }

    /// Reads the next box header and returns its info.
    ///
    /// This only reads the box header (8-16 bytes). The reader position
    /// is left at the start of the payload. Call [`skip_box`](Self::skip_box)
    /// to advance past the payload, or [`load_box`](Self::load_box) to read it.
    ///
    /// Returns `Ok(None)` when the end of the current scope is reached.
    /// Returns [`Err(ParseError::BoxExceedsScope)`](ParseError::BoxExceedsScope)
    /// if the box's declared size exceeds the remaining bytes in the scope.
    pub fn next_info(&mut self) -> Result<Option<BoxInfo>, BoxReadError> {
        // Check if we've reached the end of the current scope
        if self.current_offset >= self.end_offset {
            return Ok(None);
        }

        // Seek to current position
        self.reader.seek(SeekFrom::Start(self.current_offset))?;

        // Try to read header (minimum 8 bytes)
        let mut header_buf = [0u8; 8];
        let bytes_read = read_exact_or_eof(&mut self.reader, &mut header_buf)?;
        if bytes_read < 8 {
            return Ok(None); // End of file
        }

        // Parse basic header
        let size = BigEndian::read_u32(&header_buf[0..4]);
        let box_type = BoxCode::new([header_buf[4], header_buf[5], header_buf[6], header_buf[7]]);

        let (box_size, header_size): (BoxSize, u8) = if size == 1 {
            // Extended size - read 8 more bytes
            let mut extended_buf = [0u8; 8];
            self.reader.read_exact(&mut extended_buf)?;
            let extended = BigEndian::read_u64(&extended_buf);
            // Extended size must be at least 16 (header size)
            if extended < 16 {
                return Err(ParseError::InvalidBoxSize {
                    box_type,
                    size: extended,
                }.into());
            }
            // Safety: extended >= 16, so NonZeroU64::new always succeeds
            (BoxSize::Known(NonZeroU64::new(extended).unwrap()), 16)
        } else if size == 0 {
            // Box extends to end of current scope
            (BoxSize::ToEnd, 8)
        } else {
            if size < 8 {
                return Err(ParseError::InvalidBoxSize {
                    box_type,
                    size: size as u64,
                }.into());
            }
            // Safety: size >= 8, so NonZeroU64::new always succeeds
            (BoxSize::Known(NonZeroU64::new(size as u64).unwrap()), 8)
        };

        match box_size {
            BoxSize::Known(known) => {
                let actual_size = known.get();
                // Validate that offset + size doesn't overflow u64
                if self.current_offset.checked_add(actual_size).is_none() {
                    return Err(ParseError::InvalidBoxSize {
                        box_type,
                        size: actual_size,
                    }.into());
                }

                // Box exceeds current scope - report error and advance to end of scope
                if self.current_offset + actual_size > self.end_offset {
                    let remaining = self.end_offset - self.current_offset;
                    let offset = self.current_offset;
                    self.current_offset = self.end_offset;
                    return Err(ParseError::BoxExceedsScope {
                        box_type,
                        box_size: actual_size,
                        remaining,
                        box_offset: offset,
                    }.into());
                }
            }
            BoxSize::ToEnd => {
                // size=0 means "extends to end of file" per ISO 14496-12 §4.2.2.
                // Inside a container whose scope doesn't reach end of file, this
                // is invalid - the box would extend beyond its parent.
                if self.end_offset != self.file_size {
                    let file_remaining = self.file_size - self.current_offset;
                    let scope_remaining = self.end_offset - self.current_offset;
                    let offset = self.current_offset;
                    self.current_offset = self.end_offset;
                    return Err(ParseError::BoxExceedsScope {
                        box_type,
                        box_size: file_remaining,
                        remaining: scope_remaining,
                        box_offset: offset,
                    }.into());
                }
            }
        }

        Ok(Some(BoxInfo {
            box_type,
            size: box_size,
            header_size,
            offset: self.current_offset,
        }))
    }

    /// Returns the effective byte size of a box, resolving `BoxSize::ToEnd`
    /// to the remaining bytes in the current scope.
    pub fn effective_size(&self, info: &BoxInfo) -> u64 {
        match info.size {
            BoxSize::Known(n) => n.get(),
            BoxSize::ToEnd => self.end_offset.saturating_sub(info.offset),
        }
    }

    /// Skips past a box without reading its payload.
    ///
    /// After calling this, [`next_info`](Self::next_info) will return the next box.
    pub fn skip_box(&mut self, info: &BoxInfo) -> Result<(), BoxReadError> {
        self.current_offset = info.offset.saturating_add(self.effective_size(info));
        Ok(())
    }

    /// Loads the entire box (header + payload) into memory.
    ///
    /// After calling this, [`next_info`](Self::next_info) will return the next box.
    pub fn load_box(&mut self, info: &BoxInfo) -> Result<Vec<u8>, BoxReadError> {
        let size = self.effective_size(info);
        if size > self.max_load_size {
            return Err(ParseError::BoxTooLargeToLoad {
                size,
                max: self.max_load_size,
            }.into());
        }
        if size > usize::MAX as u64 {
            return Err(ParseError::BufferTooShort {
                expected: size as usize,
                found: 0,
            }.into());
        }
        let size = size as usize;

        self.reader.seek(SeekFrom::Start(info.offset))?;
        let mut data = Vec::with_capacity(size);
        let n = self.reader.by_ref().take(size as u64).read_to_end(&mut data)?;
        if n != size {
            return Err(ParseError::BufferTooShort {
                expected: size,
                found: n,
            }.into());
        }

        // Advance past this box
        self.current_offset = info.offset.saturating_add(size as u64);

        Ok(data)
    }

    /// Loads only the payload of a box into memory.
    ///
    /// After calling this, [`next_info`](Self::next_info) will return the next box.
    pub fn load_payload(&mut self, info: &BoxInfo) -> Result<Vec<u8>, BoxReadError> {
        let total_size = self.effective_size(info);
        let payload_size = total_size.saturating_sub(info.header_size as u64);
        if payload_size > self.max_load_size {
            return Err(ParseError::BoxTooLargeToLoad {
                size: payload_size,
                max: self.max_load_size,
            }.into());
        }
        if payload_size > usize::MAX as u64 {
            return Err(ParseError::BufferTooShort {
                expected: payload_size as usize,
                found: 0,
            }.into());
        }
        let payload_size = payload_size as usize;

        self.reader.seek(SeekFrom::Start(info.payload_offset()))?;
        let mut data = Vec::with_capacity(payload_size);
        let n = self.reader.by_ref().take(payload_size as u64).read_to_end(&mut data)?;
        if n != payload_size {
            return Err(ParseError::BufferTooShort {
                expected: payload_size,
                found: n,
            }.into());
        }

        // Advance past this box
        self.current_offset = info.offset.saturating_add(total_size);

        Ok(data)
    }

    /// Enters a container box, scoping iteration to its children.
    ///
    /// After calling this, `next_info()` will iterate over child boxes within
    /// the container. Call `exit_container()` when done to resume iteration
    /// after the container.
    ///
    /// `payload_skip` specifies how many bytes to skip at the start of the
    /// payload (e.g., 4 for FullBox containers that have version+flags before children).
    pub fn enter_container(&mut self, info: &BoxInfo, payload_skip: u64) -> Result<(), BoxReadError> {
        if self.scope_stack.len() >= MAX_NESTING_DEPTH {
            return Err(ParseError::NestingTooDeep {
                depth: self.scope_stack.len() + 1,
                max: MAX_NESTING_DEPTH,
            }.into());
        }
        let effective = self.effective_size(info);
        let new_end = info.offset.checked_add(effective).ok_or(ParseError::InvalidBoxSize {
            box_type: info.box_type,
            size: effective,
        })?;
        let new_offset = info.payload_offset().checked_add(payload_skip).ok_or(ParseError::InvalidBoxSize {
            box_type: info.box_type,
            size: effective,
        })?;
        self.scope_stack.push(self.end_offset);
        self.end_offset = new_end;
        self.current_offset = new_offset;
        self.reader.seek(SeekFrom::Start(self.current_offset))?;
        Ok(())
    }

    /// Exits the current container scope, resuming iteration after it.
    ///
    /// Sets the current offset past the container and restores the parent scope.
    pub fn exit_container(&mut self) -> Result<(), BoxReadError> {
        self.current_offset = self.end_offset;
        self.end_offset = self.scope_stack.pop().unwrap_or(self.file_size);
        self.reader.seek(SeekFrom::Start(self.current_offset))?;
        Ok(())
    }

    /// Returns the current nesting depth (0 = top-level).
    pub fn depth(&self) -> usize {
        self.scope_stack.len()
    }

    /// Returns the current offset in the stream.
    pub fn current_offset(&self) -> u64 {
        self.current_offset
    }

    /// Returns the total file size.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Returns a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    /// Returns a mutable reference to the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Consumes the iterator and returns the underlying reader.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

/// A zero-copy box iterator over an in-memory byte slice.
///
/// Unlike [`StreamingBoxIterator`] which reads from a `Read + Seek` stream and
/// copies box data into `Vec<u8>`, this iterator returns `&[u8]` sub-slices of
/// the original data - no allocation or copying is needed.
///
/// # Example
///
/// ```no_run
/// use isobmff_syntax::streaming::SliceBoxIterator;
///
/// fn validate(data: &[u8]) -> Result<(), isobmff_syntax::error::ParseError> {
///     let mut iter = SliceBoxIterator::new(data);
///
///     while let Some(info) = iter.next_info()? {
///         let box_data = iter.load_box(&info);
///         println!("Box: {:?} ({} bytes)", info.box_type, box_data.len());
///     }
///     Ok(())
/// }
/// ```
pub struct SliceBoxIterator<'a> {
    /// Sub-slice of the current scope.
    data: &'a [u8],
    /// Offset within `data` for the next box.
    position: usize,
    /// Absolute file offset of `data[0]`.
    base_offset: u64,
    /// Original data length (for `file_size()`).
    total_size: u64,
    scope_stack: Vec<ScopeContext<'a>>,
}

struct ScopeContext<'a> {
    /// Parent scope's sub-slice.
    data: &'a [u8],
    /// Where to resume in parent (past the container).
    resume_position: usize,
    /// Parent's base_offset.
    base_offset: u64,
}

impl<'a> SliceBoxIterator<'a> {
    /// Creates a new iterator over the given byte slice.
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            base_offset: 0,
            total_size: data.len() as u64,
            scope_stack: Vec::new(),
        }
    }

    /// Creates a new iterator with a base offset applied to all reported positions.
    ///
    /// Use this when the slice represents data at a known offset within a larger
    /// file, so that all `BoxInfo::offset` values and error offsets reflect the
    /// true file position rather than being slice-relative.
    pub fn with_base_offset(data: &'a [u8], base_offset: u64) -> Result<Self, ParseError> {
        let total_size = base_offset.checked_add(data.len() as u64).ok_or(
            ParseError::BufferTooShort {
                expected: usize::MAX,
                found: data.len(),
            },
        )?;
        Ok(Self {
            data,
            position: 0,
            base_offset,
            total_size,
            scope_stack: Vec::new(),
        })
    }

    /// Reads the next box header and returns its info.
    ///
    /// Returns `Ok(None)` when the end of the current scope is reached.
    /// Returns [`Err(ParseError::BoxExceedsScope)`](ParseError::BoxExceedsScope)
    /// if the box's declared size exceeds the remaining bytes in the scope.
    pub fn next_info(&mut self) -> Result<Option<BoxInfo>, ParseError> {
        let remaining = self.data.len() - self.position;
        if remaining < 8 {
            return Ok(None);
        }

        let d = &self.data[self.position..];
        let size = BigEndian::read_u32(&d[0..4]);
        let box_type = BoxCode::new([d[4], d[5], d[6], d[7]]);

        let (box_size, header_size): (BoxSize, u8) = if size == 1 {
            if remaining < 16 {
                return Err(ParseError::BufferTooShort {
                    expected: 16,
                    found: remaining,
                });
            }
            let extended = BigEndian::read_u64(&d[8..16]);
            if extended < 16 {
                return Err(ParseError::InvalidBoxSize {
                    box_type,
                    size: extended,
                });
            }
            // Safety: extended >= 16, so NonZeroU64::new always succeeds
            (BoxSize::Known(NonZeroU64::new(extended).unwrap()), 16)
        } else if size == 0 {
            // Box extends to end of current scope
            (BoxSize::ToEnd, 8)
        } else {
            if size < 8 {
                return Err(ParseError::InvalidBoxSize {
                    box_type,
                    size: size as u64,
                });
            }
            // Safety: size >= 8, so NonZeroU64::new always succeeds
            (BoxSize::Known(NonZeroU64::new(size as u64).unwrap()), 8)
        };

        let abs_offset = self.base_offset + self.position as u64;

        match box_size {
            BoxSize::Known(known) => {
                let actual_size = known.get();
                // Validate that absolute offset + size doesn't overflow u64
                if abs_offset.checked_add(actual_size).is_none() {
                    return Err(ParseError::InvalidBoxSize {
                        box_type,
                        size: actual_size,
                    });
                }

                // Box exceeds current scope - report error and advance to end of scope
                if actual_size > remaining as u64 {
                    self.position = self.data.len();
                    return Err(ParseError::BoxExceedsScope {
                        box_type,
                        box_size: actual_size,
                        remaining: remaining as u64,
                        box_offset: abs_offset,
                    });
                }
            }
            BoxSize::ToEnd => {
                // size=0 means "extends to end of file" per ISO 14496-12 §4.2.2.
                // Inside a container whose scope doesn't reach end of file, this
                // is invalid - the box would extend beyond its parent.
                let scope_end = self.base_offset + self.data.len() as u64;
                if scope_end != self.total_size {
                    let file_remaining = self.total_size - abs_offset;
                    let scope_remaining = remaining as u64;
                    self.position = self.data.len();
                    return Err(ParseError::BoxExceedsScope {
                        box_type,
                        box_size: file_remaining,
                        remaining: scope_remaining,
                        box_offset: abs_offset,
                    });
                }
            }
        }

        Ok(Some(BoxInfo {
            box_type,
            size: box_size,
            header_size,
            offset: abs_offset,
        }))
    }

    /// Returns the position within `self.data` where a box starts.
    fn box_start(&self, info: &BoxInfo) -> usize {
        debug_assert!(info.offset >= self.base_offset, "BoxInfo used in wrong scope");
        (info.offset - self.base_offset) as usize
    }

    /// Returns the effective byte size of a box, resolving `BoxSize::ToEnd`
    /// to the remaining bytes in the current scope.
    pub fn effective_size(&self, info: &BoxInfo) -> u64 {
        match info.size {
            BoxSize::Known(n) => n.get(),
            BoxSize::ToEnd => (self.data.len() - self.box_start(info)) as u64,
        }
    }

    /// Skips past a box without reading its data.
    pub fn skip_box(&mut self, info: &BoxInfo) {
        let end = self.box_start(info) + self.effective_size(info) as usize;
        debug_assert!(end <= self.data.len(), "skip_box past end of scope");
        self.position = end;
    }

    /// Returns a sub-slice of the original data for the entire box (header + payload).
    ///
    /// This is zero-copy - no allocation or memcpy is performed.
    /// Advances the iterator past this box.
    pub fn load_box(&mut self, info: &BoxInfo) -> &'a [u8] {
        let start = self.box_start(info);
        let end = start + self.effective_size(info) as usize;
        debug_assert!(end <= self.data.len(), "load_box past end of scope");
        self.position = end;
        &self.data[start..end]
    }

    /// Returns a sub-slice of the original data for the box payload only.
    ///
    /// This is zero-copy - no allocation or memcpy is performed.
    /// Advances the iterator past this box.
    pub fn load_payload(&mut self, info: &BoxInfo) -> &'a [u8] {
        let start = self.box_start(info) + info.header_size as usize;
        let end = self.box_start(info) + self.effective_size(info) as usize;
        debug_assert!(start <= end, "header_size exceeds box size");
        debug_assert!(end <= self.data.len(), "load_payload past end of scope");
        self.position = end;
        &self.data[start..end]
    }

    /// Enters a container box, scoping iteration to its children.
    pub fn enter_container(&mut self, info: &BoxInfo, payload_skip: u64) -> Result<(), ParseError> {
        if self.scope_stack.len() >= MAX_NESTING_DEPTH {
            return Err(ParseError::NestingTooDeep {
                depth: self.scope_stack.len() + 1,
                max: MAX_NESTING_DEPTH,
            });
        }
        let effective = self.effective_size(info) as usize;
        let container_start = self.box_start(info);
        let container_end = container_start + effective;
        debug_assert!(container_end <= self.data.len(), "container extends past end of scope");
        let payload_start = container_start + info.header_size as usize + payload_skip as usize;
        debug_assert!(payload_start >= container_start, "payload_skip overflow");

        if payload_start > container_end {
            return Err(ParseError::InvalidBoxSize {
                box_type: info.box_type,
                size: effective as u64,
            });
        }

        self.scope_stack.push(ScopeContext {
            data: self.data,
            resume_position: container_end,
            base_offset: self.base_offset,
        });

        let new_base_offset = self.base_offset + payload_start as u64;
        self.data = &self.data[payload_start..container_end];
        self.position = 0;
        self.base_offset = new_base_offset;
        Ok(())
    }

    /// Exits the current container scope, resuming iteration after it.
    pub fn exit_container(&mut self) {
        if let Some(ctx) = self.scope_stack.pop() {
            self.data = ctx.data;
            self.position = ctx.resume_position;
            self.base_offset = ctx.base_offset;
        }
    }

    /// Returns the current nesting depth (0 = top-level).
    pub fn depth(&self) -> usize {
        self.scope_stack.len()
    }

    /// Returns the total data length.
    pub fn file_size(&self) -> u64 {
        self.total_size
    }
}

/// Reads exactly `buf.len()` bytes, or fewer if EOF is reached.
/// Returns the number of bytes read.
fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        match reader.read(&mut buf[total..])? {
            0 => break, // EOF
            n => total += n,
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn make_box(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = 8 + payload.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(box_type);
        data.extend_from_slice(payload);
        data
    }

    fn make_extended_box(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = 16 + payload.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&1u32.to_be_bytes()); // size = 1 indicates extended size
        data.extend_from_slice(box_type);
        data.extend_from_slice(&(size as u64).to_be_bytes());
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn iterate_empty_stream() {
        let data: Vec<u8> = vec![];
        let cursor = Cursor::new(data);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn iterate_single_box() {
        let ftyp = make_box(b"ftyp", b"isom\x00\x00\x02\x00");
        let cursor = Cursor::new(ftyp.clone());
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let info = iter.next_info().unwrap().unwrap();
        assert_eq!(info.box_type, BoxCode::FTYP);
        assert_eq!(info.offset, 0);
        assert_eq!(iter.effective_size(&info), ftyp.len() as u64);
        assert_eq!(info.header_size, 8);

        iter.skip_box(&info).unwrap();
        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn iterate_multiple_boxes() {
        let mut data = Vec::new();
        data.extend(make_box(b"ftyp", b"isom"));
        data.extend(make_box(b"moov", b"test"));
        data.extend(make_box(b"mdat", b"media data here"));

        let cursor = Cursor::new(data);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        // First box
        let info1 = iter.next_info().unwrap().unwrap();
        assert_eq!(info1.box_type, BoxCode::FTYP);
        assert_eq!(info1.offset, 0);
        iter.skip_box(&info1).unwrap();

        // Second box
        let info2 = iter.next_info().unwrap().unwrap();
        assert_eq!(info2.box_type, BoxCode::MOOV);
        assert_eq!(info2.offset, 12); // ftyp box is 8 + 4 = 12 bytes
        iter.skip_box(&info2).unwrap();

        // Third box
        let info3 = iter.next_info().unwrap().unwrap();
        assert_eq!(info3.box_type, BoxCode::MDAT);
        iter.skip_box(&info3).unwrap();

        // End
        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn load_box_data() {
        let payload = b"test payload data";
        let ftyp = make_box(b"ftyp", payload);
        let cursor = Cursor::new(ftyp.clone());
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let info = iter.next_info().unwrap().unwrap();
        let loaded = iter.load_box(&info).unwrap();

        assert_eq!(loaded, ftyp);
    }

    #[test]
    fn load_payload_only() {
        let payload = b"test payload data";
        let ftyp = make_box(b"ftyp", payload);
        let cursor = Cursor::new(ftyp);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let info = iter.next_info().unwrap().unwrap();
        let loaded = iter.load_payload(&info).unwrap();

        assert_eq!(loaded.as_slice(), payload);
    }

    #[test]
    fn enter_exit_container() {
        // Build: moov { tkhd, mdia }
        let tkhd = make_box(b"tkhd", b"track header data");
        let mdia = make_box(b"mdia", b"media data");
        let mut moov_payload = Vec::new();
        moov_payload.extend(&tkhd);
        moov_payload.extend(&mdia);
        let moov = make_box(b"moov", &moov_payload);

        let cursor = Cursor::new(moov);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();
        assert_eq!(iter.depth(), 0);

        // Read moov header
        let moov_info = iter.next_info().unwrap().unwrap();
        assert_eq!(moov_info.box_type, BoxCode::MOOV);

        // Enter moov
        iter.enter_container(&moov_info, 0).unwrap();
        assert_eq!(iter.depth(), 1);

        // Read tkhd inside moov
        let child1 = iter.next_info().unwrap().unwrap();
        assert_eq!(child1.box_type, BoxCode::TKHD);
        iter.skip_box(&child1).unwrap();

        // Read mdia inside moov
        let child2 = iter.next_info().unwrap().unwrap();
        assert_eq!(child2.box_type, BoxCode::MDIA);
        iter.skip_box(&child2).unwrap();

        // No more children
        assert!(iter.next_info().unwrap().is_none());

        // Exit moov
        iter.exit_container().unwrap();
        assert_eq!(iter.depth(), 0);

        // No more top-level boxes
        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn nested_container_iteration() {
        // Build: moov { trak { tkhd } }
        let tkhd = make_box(b"tkhd", b"header");
        let trak = make_box(b"trak", &tkhd);
        let moov = make_box(b"moov", &trak);

        let cursor = Cursor::new(moov);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let moov_info = iter.next_info().unwrap().unwrap();
        assert_eq!(moov_info.box_type, BoxCode::MOOV);
        iter.enter_container(&moov_info, 0).unwrap();
        assert_eq!(iter.depth(), 1);

        let trak_info = iter.next_info().unwrap().unwrap();
        assert_eq!(trak_info.box_type, BoxCode::TRAK);
        iter.enter_container(&trak_info, 0).unwrap();
        assert_eq!(iter.depth(), 2);

        let tkhd_info = iter.next_info().unwrap().unwrap();
        assert_eq!(tkhd_info.box_type, BoxCode::TKHD);
        iter.skip_box(&tkhd_info).unwrap();

        assert!(iter.next_info().unwrap().is_none());
        iter.exit_container().unwrap();
        assert_eq!(iter.depth(), 1);

        assert!(iter.next_info().unwrap().is_none());
        iter.exit_container().unwrap();
        assert_eq!(iter.depth(), 0);

        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn fullbox_container_skip() {
        // Build a meta box (fullbox container): 4 bytes version+flags, then children
        let child = make_box(b"hdlr", b"handler");
        let mut meta_payload = Vec::new();
        meta_payload.extend_from_slice(&[0, 0, 0, 0]); // version + flags
        meta_payload.extend(&child);
        let meta = make_box(b"meta", &meta_payload);

        let cursor = Cursor::new(meta);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let meta_info = iter.next_info().unwrap().unwrap();
        assert_eq!(meta_info.box_type, BoxCode::META);

        // Enter with payload_skip=4 to skip version+flags
        iter.enter_container(&meta_info, 4).unwrap();

        let child_info = iter.next_info().unwrap().unwrap();
        assert_eq!(child_info.box_type, BoxCode::HDLR);
        iter.skip_box(&child_info).unwrap();

        assert!(iter.next_info().unwrap().is_none());
        iter.exit_container().unwrap();

        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn container_with_sibling() {
        // Build: moov { tkhd } followed by mdat
        let tkhd = make_box(b"tkhd", b"track header");
        let moov = make_box(b"moov", &tkhd);
        let mdat = make_box(b"mdat", b"media data");
        let mut data = Vec::new();
        data.extend(&moov);
        data.extend(&mdat);

        let cursor = Cursor::new(data);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let moov_info = iter.next_info().unwrap().unwrap();
        assert_eq!(moov_info.box_type, BoxCode::MOOV);
        iter.enter_container(&moov_info, 0).unwrap();

        let tkhd_info = iter.next_info().unwrap().unwrap();
        assert_eq!(tkhd_info.box_type, BoxCode::TKHD);
        iter.skip_box(&tkhd_info).unwrap();

        assert!(iter.next_info().unwrap().is_none());
        iter.exit_container().unwrap();

        // Should see the sibling mdat
        let mdat_info = iter.next_info().unwrap().unwrap();
        assert_eq!(mdat_info.box_type, BoxCode::MDAT);
        iter.skip_box(&mdat_info).unwrap();

        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn extended_size_box() {
        let payload = b"extended";
        let box_data = make_extended_box(b"mdat", payload);
        let cursor = Cursor::new(box_data.clone());
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let info = iter.next_info().unwrap().unwrap();
        assert_eq!(info.box_type, BoxCode::MDAT);
        assert_eq!(info.header_size, 16);
        assert_eq!(iter.effective_size(&info), box_data.len() as u64);
    }

    // --- Scope bounds validation tests ---

    /// A box whose declared size exceeds the top-level data produces an error.
    #[test]
    fn streaming_box_exceeds_top_level_scope() {
        // Declare size = 100, but only provide 8 bytes of payload (total 16 bytes of data)
        let mut data = Vec::new();
        data.extend_from_slice(&100u32.to_be_bytes()); // size = 100
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"isom\x00\x00\x02\x00"); // 8 bytes payload
        // Total data: 16 bytes, but box claims 100

        let cursor = Cursor::new(data.clone());
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let err = iter.next_info().unwrap_err();
        assert!(matches!(
            err,
            BoxReadError::Parse(ParseError::BoxExceedsScope {
                box_size: 100,
                remaining: 16,
                box_offset: 0,
                ..
            })
        ));

        // Iterator is now at end of scope; subsequent calls return Ok(None)
        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn slice_box_exceeds_top_level_scope() {
        let mut data = Vec::new();
        data.extend_from_slice(&100u32.to_be_bytes()); // size = 100
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"isom\x00\x00\x02\x00");

        let mut iter = SliceBoxIterator::new(&data);

        let err = iter.next_info().unwrap_err();
        assert!(matches!(
            err,
            ParseError::BoxExceedsScope {
                box_size: 100,
                remaining: 16,
                box_offset: 0,
                ..
            }
        ));

        assert!(iter.next_info().unwrap().is_none());
    }

    /// A child box whose declared size exceeds its parent container produces an error.
    /// The caller can recover by calling exit_container().
    #[test]
    fn streaming_child_exceeds_container_bounds() {
        // Build a moov container with a child that claims to be bigger than the container
        let mut oversized_child = Vec::new();
        oversized_child.extend_from_slice(&200u32.to_be_bytes()); // size = 200
        oversized_child.extend_from_slice(b"tkhd");
        oversized_child.extend_from_slice(b"small payload"); // only 13 bytes

        let moov = make_box(b"moov", &oversized_child);
        let mdat = make_box(b"mdat", b"media");
        let mut data = Vec::new();
        data.extend(&moov);
        data.extend(&mdat);

        let cursor = Cursor::new(data);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let moov_info = iter.next_info().unwrap().unwrap();
        assert_eq!(moov_info.box_type, BoxCode::MOOV);
        iter.enter_container(&moov_info, 0).unwrap();

        // The child claims size=200 but the container payload is only 21 bytes
        let err = iter.next_info().unwrap_err();
        assert!(matches!(err, BoxReadError::Parse(ParseError::BoxExceedsScope { box_size: 200, .. })));

        // Subsequent calls within the same scope return Ok(None)
        assert!(iter.next_info().unwrap().is_none());

        // Recovery: exit_container resumes at parent level
        iter.exit_container().unwrap();
        let mdat_info = iter.next_info().unwrap().unwrap();
        assert_eq!(mdat_info.box_type, BoxCode::MDAT);
    }

    #[test]
    fn slice_child_exceeds_container_bounds() {
        let mut oversized_child = Vec::new();
        oversized_child.extend_from_slice(&200u32.to_be_bytes());
        oversized_child.extend_from_slice(b"tkhd");
        oversized_child.extend_from_slice(b"small payload");

        let moov = make_box(b"moov", &oversized_child);
        let mdat = make_box(b"mdat", b"media");
        let mut data = Vec::new();
        data.extend(&moov);
        data.extend(&mdat);

        let mut iter = SliceBoxIterator::new(&data);

        let moov_info = iter.next_info().unwrap().unwrap();
        assert_eq!(moov_info.box_type, BoxCode::MOOV);
        iter.enter_container(&moov_info, 0).unwrap();

        let err = iter.next_info().unwrap_err();
        assert!(matches!(err, ParseError::BoxExceedsScope { box_size: 200, .. }));

        assert!(iter.next_info().unwrap().is_none());

        iter.exit_container();
        let mdat_info = iter.next_info().unwrap().unwrap();
        assert_eq!(mdat_info.box_type, BoxCode::MDAT);
    }

    /// Verify that load_box inside a container only returns data within the container.
    #[test]
    fn streaming_load_box_respects_container_bounds() {
        // Build: moov { tkhd } followed by mdat
        let tkhd_payload = b"track header";
        let tkhd = make_box(b"tkhd", tkhd_payload);
        let moov = make_box(b"moov", &tkhd);
        let mdat = make_box(b"mdat", b"media data that should not leak");
        let mut data = Vec::new();
        data.extend(&moov);
        data.extend(&mdat);

        let cursor = Cursor::new(data);
        let mut iter = StreamingBoxIterator::new(cursor).unwrap();

        let moov_info = iter.next_info().unwrap().unwrap();
        iter.enter_container(&moov_info, 0).unwrap();

        let tkhd_info = iter.next_info().unwrap().unwrap();
        assert_eq!(tkhd_info.box_type, BoxCode::TKHD);

        let loaded = iter.load_box(&tkhd_info).unwrap();
        assert_eq!(loaded, tkhd);
        assert_eq!(loaded.len(), tkhd.len());

        // No more children - the mdat should NOT appear inside the container
        assert!(iter.next_info().unwrap().is_none());

        iter.exit_container().unwrap();

        // mdat is accessible at the top level
        let mdat_info = iter.next_info().unwrap().unwrap();
        assert_eq!(mdat_info.box_type, BoxCode::MDAT);
    }

    #[test]
    fn slice_load_box_respects_container_bounds() {
        let tkhd_payload = b"track header";
        let tkhd = make_box(b"tkhd", tkhd_payload);
        let moov = make_box(b"moov", &tkhd);
        let mdat = make_box(b"mdat", b"media data that should not leak");
        let mut data = Vec::new();
        data.extend(&moov);
        data.extend(&mdat);

        let mut iter = SliceBoxIterator::new(&data);

        let moov_info = iter.next_info().unwrap().unwrap();
        iter.enter_container(&moov_info, 0).unwrap();

        let tkhd_info = iter.next_info().unwrap().unwrap();
        assert_eq!(tkhd_info.box_type, BoxCode::TKHD);

        let loaded = iter.load_box(&tkhd_info);
        assert_eq!(loaded, tkhd.as_slice());
        assert_eq!(loaded.len(), tkhd.len());

        assert!(iter.next_info().unwrap().is_none());

        iter.exit_container();

        let mdat_info = iter.next_info().unwrap().unwrap();
        assert_eq!(mdat_info.box_type, BoxCode::MDAT);
    }

    // --- SliceBoxIterator basic functionality tests ---

    #[test]
    fn slice_iterate_single_box() {
        let ftyp = make_box(b"ftyp", b"isom\x00\x00\x02\x00");
        let mut iter = SliceBoxIterator::new(&ftyp);

        let info = iter.next_info().unwrap().unwrap();
        assert_eq!(info.box_type, BoxCode::FTYP);
        assert_eq!(info.offset, 0);
        assert_eq!(iter.effective_size(&info), ftyp.len() as u64);
        assert_eq!(info.header_size, 8);

        iter.skip_box(&info);
        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn slice_load_box_and_payload() {
        let payload = b"test payload data";
        let ftyp = make_box(b"ftyp", payload);
        let mut iter = SliceBoxIterator::new(&ftyp);

        let info = iter.next_info().unwrap().unwrap();
        let loaded = iter.load_payload(&info);
        assert_eq!(loaded, payload.as_slice());
    }

    #[test]
    fn slice_enter_exit_container() {
        // Build: moov { tkhd, mdia }
        let tkhd = make_box(b"tkhd", b"track header data");
        let mdia = make_box(b"mdia", b"media data");
        let mut moov_payload = Vec::new();
        moov_payload.extend(&tkhd);
        moov_payload.extend(&mdia);
        let moov = make_box(b"moov", &moov_payload);

        let mut iter = SliceBoxIterator::new(&moov);
        assert_eq!(iter.depth(), 0);

        let moov_info = iter.next_info().unwrap().unwrap();
        assert_eq!(moov_info.box_type, BoxCode::MOOV);

        iter.enter_container(&moov_info, 0).unwrap();
        assert_eq!(iter.depth(), 1);

        let child1 = iter.next_info().unwrap().unwrap();
        assert_eq!(child1.box_type, BoxCode::TKHD);
        iter.skip_box(&child1);

        let child2 = iter.next_info().unwrap().unwrap();
        assert_eq!(child2.box_type, BoxCode::MDIA);
        iter.skip_box(&child2);

        assert!(iter.next_info().unwrap().is_none());

        iter.exit_container();
        assert_eq!(iter.depth(), 0);

        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn slice_nested_containers() {
        // Build: moov { trak { tkhd } }
        let tkhd = make_box(b"tkhd", b"header");
        let trak = make_box(b"trak", &tkhd);
        let moov = make_box(b"moov", &trak);

        let mut iter = SliceBoxIterator::new(&moov);

        let moov_info = iter.next_info().unwrap().unwrap();
        iter.enter_container(&moov_info, 0).unwrap();

        let trak_info = iter.next_info().unwrap().unwrap();
        assert_eq!(trak_info.box_type, BoxCode::TRAK);
        iter.enter_container(&trak_info, 0).unwrap();
        assert_eq!(iter.depth(), 2);

        let tkhd_info = iter.next_info().unwrap().unwrap();
        assert_eq!(tkhd_info.box_type, BoxCode::TKHD);
        iter.skip_box(&tkhd_info);

        assert!(iter.next_info().unwrap().is_none());
        iter.exit_container();

        assert!(iter.next_info().unwrap().is_none());
        iter.exit_container();

        assert!(iter.next_info().unwrap().is_none());
    }

    #[test]
    fn slice_container_with_sibling() {
        let tkhd = make_box(b"tkhd", b"track header");
        let moov = make_box(b"moov", &tkhd);
        let mdat = make_box(b"mdat", b"media data");
        let mut data = Vec::new();
        data.extend(&moov);
        data.extend(&mdat);

        let mut iter = SliceBoxIterator::new(&data);

        let moov_info = iter.next_info().unwrap().unwrap();
        iter.enter_container(&moov_info, 0).unwrap();

        let tkhd_info = iter.next_info().unwrap().unwrap();
        assert_eq!(tkhd_info.box_type, BoxCode::TKHD);
        iter.skip_box(&tkhd_info);

        assert!(iter.next_info().unwrap().is_none());
        iter.exit_container();

        let mdat_info = iter.next_info().unwrap().unwrap();
        assert_eq!(mdat_info.box_type, BoxCode::MDAT);
        iter.skip_box(&mdat_info);

        assert!(iter.next_info().unwrap().is_none());
    }
}
