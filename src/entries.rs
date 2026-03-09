//! Validated fixed-size entry table over a byte slice.
//!
//! Many ISOBMFF boxes contain a `u32` entry count followed by a flat array of
//! fixed-size records. [`FixedSizeEntries`] validates the count against the
//! available data at construction time, so all subsequent access is
//! bounds-check-free.

use crate::error::ParseError;

/// A validated view over `count` entries of `N` bytes each.
///
/// **Invariant:** `data.len() == count * N`, established at construction.
/// After construction, iteration and indexed access cannot fail due to
/// insufficient data.
#[derive(Clone, Copy)]
pub struct FixedSizeEntries<'a, const N: usize> {
    data: &'a [u8],
}

impl<'a, const N: usize> FixedSizeEntries<'a, N> {
    /// Creates a new `FixedSizeEntries` after validating that `data` can hold
    /// exactly `count` entries of `N` bytes each.
    ///
    /// The slice is truncated to `count * N` bytes; any trailing bytes are
    /// ignored (they belong to the parent box, not the entry table).
    pub fn new(data: &'a [u8], count: u32) -> Result<Self, ParseError> {
        let total = (count as usize).checked_mul(N).ok_or(ParseError::InvalidEntryCount {
            count,
            max_possible: (data.len() / N) as u32,
        })?;
        if total > data.len() {
            return Err(ParseError::InvalidEntryCount {
                count,
                max_possible: (data.len() / N) as u32,
            });
        }
        Ok(Self { data: &data[..total] })
    }

    /// Returns the number of entries.
    pub fn count(&self) -> u32 {
        (self.data.len() / N) as u32
    }

    /// Returns the raw bytes of the entry at `index`, or `None` if out of range.
    pub fn get(&self, index: usize) -> Option<&'a [u8; N]> {
        let start = index.checked_mul(N)?;
        self.data.get(start..start + N)?.try_into().ok()
    }

    /// Returns an iterator over the raw `N`-byte entries.
    pub fn iter(&self) -> FixedSizeIter<'a, N> {
        FixedSizeIter { data: self.data }
    }
}

/// Iterator over fixed-size entries, yielding `&[u8; N]` references.
#[derive(Clone)]
pub struct FixedSizeIter<'a, const N: usize> {
    data: &'a [u8],
}

impl<'a, const N: usize> Iterator for FixedSizeIter<'a, N> {
    type Item = &'a [u8; N];

    #[inline]
    fn next(&mut self) -> Option<&'a [u8; N]> {
        let (head, tail) = self.data.split_at_checked(N)?;
        self.data = tail;
        Some(head.try_into().unwrap())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.data.len() / N;
        (len, Some(len))
    }
}

impl<const N: usize> ExactSizeIterator for FixedSizeIter<'_, N> {}
