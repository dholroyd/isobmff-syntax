#![deny(unsafe_code)]
//! ISOBMFF syntax structures for parsing and serializing ISO base media file format data.
//!
//! This crate provides types for working with structures defined in ISO/IEC 14496-12,
//! the ISO base media file format specification.
//!
//! # Overview
//!
//! Each box type provides two representations:
//!
//! - A **borrowing view** (e.g., [`MovieHeaderBoxView`]) that wraps a byte slice and
//!   decodes fields on demand. This is efficient for read-only access without copying.
//!
//! - An **owned struct** (e.g., [`MovieHeaderBoxOwned`]) that stores parsed fields
//!   directly. This allows modification and can serialize back to bytes.
//!
//! Both representations implement a common trait (e.g., [`MovieHeaderBox`]) enabling
//! generic code to work with either representation.
//!
//! # Example
//!
//! ```
//! use isobmff_syntax::boxes::mvhd::{MovieHeaderBox, MovieHeaderBoxView, MovieHeaderBoxOwned};
//!
//! // Parse from bytes
//! # fn example(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//! let view = MovieHeaderBoxView::new(bytes)?;
//! println!("Duration: {} / {}", view.duration(), view.timescale());
//!
//! // Convert to owned for modification
//! let mut owned = MovieHeaderBoxOwned::from(&view);
//! owned.duration *= 2;
//!
//! // Serialize back
//! let mut output = Vec::new();
//! owned.write_to(&mut output)?;
//! # Ok(())
//! # }
//! ```

#[deny(unsafe_code)]
pub mod boxes;
pub mod container;
pub mod entries;
pub mod error;
pub mod header;
#[doc(hidden)]
pub mod macros;
pub mod streaming;
pub mod types;

// Re-export commonly used types
pub use boxes::mvhd::{MovieHeaderBox, MovieHeaderBoxOwned, MovieHeaderBoxView};
pub use container::{BoxIterator, ChildBox, OpaqueBoxOwned, RawBox, TypedBoxView};
pub use error::{BoxReadError, ParseError};
pub use streaming::{BoxInfo, BoxSize, SliceBoxIterator, StreamingBoxIterator};
pub use header::{BoxHeader, FullBoxHeader};
pub use types::{
    FixedPoint16_16, FixedPoint2_30, FixedPoint8_8, IsoLanguageCode, Matrix, Mp4Timestamp,
    NullTerminatedString, UFixedPoint16_16,
};

// Re-export mp4ra_rust types used in public APIs
pub use mp4ra_rust::{BoxCode, BrandCode, FourCC, HandlerCode, SampleEntryCode, TrackReferenceCode};
