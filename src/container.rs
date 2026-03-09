//! Container box iteration and generic box handling.
//!
//! This module provides infrastructure for working with container boxes
//! that contain child boxes.

use crate::error::ParseError;
use crate::header::BoxHeader;
use mp4ra_rust::BoxCode;
use std::io::{self, Write};

/// A typed box view that can be parsed from raw box bytes.
///
/// This trait connects a view type to its four-character code and provides
/// a uniform constructor. It is implemented for all concrete `*BoxView` types
/// in the [`boxes`](crate::boxes) module.
///
/// # Example
///
/// ```
/// use isobmff_syntax::container::{RawBox, TypedBoxView};
/// use isobmff_syntax::boxes::mvhd::{MovieHeaderBox, MovieHeaderBoxView};
///
/// # fn example(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
/// let raw = RawBox::new(data)?;
/// let mvhd = raw.parse_as::<MovieHeaderBoxView>()?;
/// println!("timescale: {}", mvhd.timescale());
/// # Ok(())
/// # }
/// ```
pub trait TypedBoxView<'a>: Sized {
    /// The four-character code identifying this box type.
    const BOX_TYPE: BoxCode;

    /// Parse from raw box bytes (header + payload).
    fn from_raw_box(data: &'a [u8]) -> Result<Self, ParseError>;
}

/// A raw box wrapper providing access to unparsed box data.
///
/// This type wraps a byte slice containing a complete box and provides
/// access to the header information and payload data.
#[derive(Clone, Copy, Debug)]
pub struct RawBox<'a> {
    data: &'a [u8],
    header: BoxHeader,
}

impl<'a> RawBox<'a> {
    /// Creates a new RawBox from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is too short or the size doesn't match.
    pub fn new(data: &'a [u8]) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(data, data.len())?;

        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }

        Ok(Self { data, header })
    }

    /// Creates a RawBox from a byte slice without validating size.
    ///
    /// This is useful when iterating over boxes where we've already
    /// calculated the box boundaries.
    fn new_unchecked(data: &'a [u8], header: BoxHeader) -> Self {
        Self { data, header }
    }

    /// Returns the box header.
    #[inline]
    pub fn header(&self) -> BoxHeader {
        self.header
    }

    /// Returns the box type.
    #[inline]
    pub fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    /// Returns the total box size.
    #[inline]
    pub fn size(&self) -> u64 {
        self.header.size
    }

    /// Returns the complete box data (including header).
    #[inline]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the payload data (excluding header).
    #[inline]
    pub fn payload(&self) -> &'a [u8] {
        &self.data[self.header.header_size as usize..]
    }

    /// Returns the header size.
    #[inline]
    pub fn header_size(&self) -> usize {
        self.header.header_size as usize
    }

    /// Parse this raw box as a specific typed box view.
    ///
    /// Returns `Err(ParseError::InvalidBoxType)` if this box's four-cc
    /// doesn't match `T::BOX_TYPE`.
    pub fn parse_as<T: TypedBoxView<'a>>(&self) -> Result<T, ParseError> {
        if self.box_type() != T::BOX_TYPE {
            return Err(ParseError::InvalidBoxType {
                expected: T::BOX_TYPE,
                found: self.box_type(),
            });
        }
        T::from_raw_box(self.data())
    }
}

/// An iterator over child boxes within a container box.
///
/// This iterator parses box headers on demand and yields `RawBox` instances
/// for each child box.
#[derive(Clone, Debug)]
pub struct BoxIterator<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> BoxIterator<'a> {
    /// Creates a new iterator over the given data.
    ///
    /// The data should be the payload of a container box (after the container's
    /// own header).
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    /// Returns the remaining data that hasn't been iterated.
    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.offset..]
    }

    /// Finds the first child box with the given type.
    pub fn find_by_type(&mut self, box_type: BoxCode) -> Option<RawBox<'a>> {
        self.find_matching(|b| b.box_type() == box_type)
    }

    /// Finds the first child box matching the predicate.
    pub fn find_matching<F>(&mut self, predicate: F) -> Option<RawBox<'a>>
    where
        F: Fn(&RawBox<'a>) -> bool,
    {
        self.by_ref().find(|&raw_box| predicate(&raw_box))
    }

    /// Collects all child boxes with the given type.
    pub fn filter_by_type(&mut self, box_type: BoxCode) -> Vec<RawBox<'a>> {
        self.filter(|b| b.box_type() == box_type).collect()
    }

    /// Finds the first child box with the given type and parses it as `T`.
    ///
    /// Returns `None` if no child matches `T::BOX_TYPE`, or
    /// `Some(Err(_))` if a matching child is found but fails to parse.
    pub fn find_as<T: TypedBoxView<'a>>(&mut self) -> Option<Result<T, ParseError>> {
        self.find_by_type(T::BOX_TYPE)
            .map(|raw| T::from_raw_box(raw.data()))
    }

    /// Returns an iterator that yields all children parsed as `T`.
    ///
    /// Only children whose type matches `T::BOX_TYPE` are considered.
    /// Parse errors are preserved as `Err` items.
    pub fn filter_as<T: TypedBoxView<'a>>(self) -> impl Iterator<Item = Result<T, ParseError>> + 'a {
        self.filter(move |raw| raw.box_type() == T::BOX_TYPE)
            .map(|raw| T::from_raw_box(raw.data()))
    }
}

impl<'a> Iterator for BoxIterator<'a> {
    type Item = RawBox<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }

        let remaining = &self.data[self.offset..];
        if remaining.len() < 8 {
            // Not enough data for a box header
            return None;
        }

        let header = BoxHeader::parse(remaining, remaining.len()).ok()?;

        if header.size > remaining.len() as u64 || header.size < header.header_size as u64 {
            // Invalid box size
            return None;
        }
        let box_size = header.size as usize;

        let box_data = &remaining[..box_size];
        self.offset += box_size;

        Some(RawBox::new_unchecked(box_data, header))
    }
}

/// Trait for items yielded by container box child iterators.
pub trait ChildBox {
    /// Returns the box type code of this child.
    fn box_type(&self) -> BoxCode;

    /// Returns the total size of this child box in bytes.
    fn box_size(&self) -> u64;
}

impl ChildBox for RawBox<'_> {
    fn box_type(&self) -> BoxCode {
        self.box_type()
    }

    fn box_size(&self) -> u64 {
        self.size()
    }
}

impl<T: ChildBox> ChildBox for &T {
    fn box_type(&self) -> BoxCode {
        (**self).box_type()
    }

    fn box_size(&self) -> u64 {
        (**self).box_size()
    }
}

/// An owned box whose internal structure is opaque (not parsed).
///
/// Used for children of unknown or unrecognized box types within a container.
/// Stores the complete box bytes (header + payload).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueBoxOwned {
    data: Vec<u8>,
    header: BoxHeader,
}

impl OpaqueBoxOwned {
    /// Creates a new `OpaqueBoxOwned` from complete box bytes.
    ///
    /// Returns an error if the data is too short to contain a valid box header
    /// or if the declared size does not match the data length.
    pub fn new(data: Vec<u8>) -> Result<Self, ParseError> {
        let header = BoxHeader::parse(&data, data.len())?;
        if header.size != data.len() as u64 {
            return Err(ParseError::SizeMismatch {
                declared: header.size,
                actual: data.len(),
            });
        }
        Ok(Self { data, header })
    }

    /// Creates an `OpaqueBoxOwned` from a `RawBox` reference.
    pub fn from_raw_box(raw: &RawBox<'_>) -> Self {
        Self {
            data: raw.data().to_vec(),
            header: raw.header(),
        }
    }

    /// Returns the complete box bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Writes the complete box bytes to the given writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.data)
    }
}

impl ChildBox for OpaqueBoxOwned {
    fn box_type(&self) -> BoxCode {
        self.header.box_type
    }

    fn box_size(&self) -> u64 {
        self.data.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_box(box_type: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = 8 + payload.len();
        let mut data = Vec::with_capacity(size);
        data.extend_from_slice(&(size as u32).to_be_bytes());
        data.extend_from_slice(box_type);
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn raw_box_parse() {
        let data = make_box(b"test", &[1, 2, 3, 4]);
        let raw = RawBox::new(&data).unwrap();
        assert_eq!(raw.box_type(), BoxCode::new(*b"test"));
        assert_eq!(raw.size(), 12);
        assert_eq!(raw.payload(), &[1, 2, 3, 4]);
    }

    #[test]
    fn raw_box_size_mismatch() {
        let mut data = make_box(b"test", &[1, 2, 3, 4]);
        data.push(0); // Extra byte
        let result = RawBox::new(&data);
        assert!(matches!(result, Err(ParseError::SizeMismatch { .. })));
    }

    #[test]
    fn box_iterator() {
        let mut container = Vec::new();
        container.extend_from_slice(&make_box(b"box1", &[1, 2]));
        container.extend_from_slice(&make_box(b"box2", &[3, 4]));
        container.extend_from_slice(&make_box(b"box3", &[5, 6]));

        let boxes: Vec<_> = BoxIterator::new(&container).collect();
        assert_eq!(boxes.len(), 3);
        assert_eq!(boxes[0].box_type(), BoxCode::new(*b"box1"));
        assert_eq!(boxes[1].box_type(), BoxCode::new(*b"box2"));
        assert_eq!(boxes[2].box_type(), BoxCode::new(*b"box3"));
    }

    #[test]
    fn box_iterator_find_by_type() {
        let mut container = Vec::new();
        container.extend_from_slice(&make_box(b"box1", &[1]));
        container.extend_from_slice(&make_box(b"trak", &[2]));
        container.extend_from_slice(&make_box(b"box3", &[3]));

        let mut iter = BoxIterator::new(&container);
        let found = iter.find_by_type(BoxCode::TRAK);
        assert!(found.is_some());
        assert_eq!(found.unwrap().payload(), &[2]);
    }

    #[test]
    fn box_iterator_empty() {
        let boxes: Vec<_> = BoxIterator::new(&[]).collect();
        assert!(boxes.is_empty());
    }

    #[test]
    fn box_iterator_truncated() {
        let data = [0x00, 0x00, 0x00, 0x10, b't', b'e', b's', b't'];
        // Box declares 16 bytes but only 8 are present
        let boxes: Vec<_> = BoxIterator::new(&data).collect();
        assert!(boxes.is_empty());
    }
}
