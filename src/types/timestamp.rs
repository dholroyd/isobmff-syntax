//! MP4 timestamp representation.

use std::fmt;

/// Seconds since the MP4 epoch (1904-01-01 00:00:00 UTC).
///
/// This is the timestamp format used throughout the ISOBMFF specification.
/// The offset from Unix epoch (1970-01-01) to MP4 epoch (1904-01-01) is
/// 2,082,844,800 seconds.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Mp4Timestamp(u64);

impl Mp4Timestamp {
    /// Offset from Unix epoch to MP4 epoch in seconds.
    /// This is the number of seconds between 1904-01-01 and 1970-01-01.
    pub const EPOCH_OFFSET: u64 = 2_082_844_800;

    /// The zero timestamp (1904-01-01 00:00:00 UTC).
    pub const ZERO: Self = Self(0);

    /// Creates a timestamp from seconds since the MP4 epoch.
    #[inline]
    pub const fn from_mp4_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    /// Creates a timestamp from seconds since the Unix epoch.
    ///
    /// Returns `None` if the Unix timestamp predates the MP4 epoch.
    #[inline]
    pub const fn from_unix_seconds(seconds: u64) -> Option<Self> {
        if seconds >= Self::EPOCH_OFFSET {
            Some(Self(seconds - Self::EPOCH_OFFSET))
        } else {
            // The Unix timestamp predates the MP4 epoch, which shouldn't
            // happen for typical media files but we handle it gracefully
            None
        }
    }

    /// Creates a timestamp from seconds since the Unix epoch, saturating
    /// to zero if the timestamp predates the MP4 epoch.
    #[inline]
    pub const fn from_unix_seconds_saturating(seconds: u64) -> Self {
        if seconds >= Self::EPOCH_OFFSET {
            Self(seconds - Self::EPOCH_OFFSET)
        } else {
            Self(0)
        }
    }

    /// Returns the timestamp as seconds since the MP4 epoch.
    #[inline]
    pub const fn as_mp4_seconds(self) -> u64 {
        self.0
    }

    /// Returns the timestamp as seconds since the Unix epoch,
    /// or `None` if the result would overflow `u64`.
    #[inline]
    pub const fn as_unix_seconds(self) -> Option<u64> {
        match self.0.checked_add(Self::EPOCH_OFFSET) {
            Some(v) => Some(v),
            None => None,
        }
    }

    /// Returns true if this timestamp fits in a 32-bit value.
    #[inline]
    pub const fn fits_in_u32(self) -> bool {
        self.0 <= u32::MAX as u64
    }
}

impl From<u64> for Mp4Timestamp {
    fn from(seconds: u64) -> Self {
        Self(seconds)
    }
}

impl From<u32> for Mp4Timestamp {
    fn from(seconds: u32) -> Self {
        Self(seconds as u64)
    }
}

impl From<Mp4Timestamp> for u64 {
    fn from(ts: Mp4Timestamp) -> Self {
        ts.0
    }
}

impl fmt::Debug for Mp4Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mp4Timestamp({})", self.0)
    }
}

impl fmt::Display for Mp4Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero() {
        assert_eq!(Mp4Timestamp::ZERO.as_mp4_seconds(), 0);
    }

    #[test]
    fn unix_conversion() {
        let ts = Mp4Timestamp::from_mp4_seconds(1_000_000);
        assert_eq!(ts.as_unix_seconds(), Some(1_000_000 + Mp4Timestamp::EPOCH_OFFSET));
    }

    #[test]
    fn from_unix() {
        let unix = 3_000_000_000u64; // After 1970 + enough offset
        let ts = Mp4Timestamp::from_unix_seconds(unix).unwrap();
        assert_eq!(ts.as_unix_seconds(), Some(unix));
    }

    #[test]
    fn fits_in_u32() {
        assert!(Mp4Timestamp::from_mp4_seconds(0).fits_in_u32());
        assert!(Mp4Timestamp::from_mp4_seconds(u32::MAX as u64).fits_in_u32());
        assert!(!Mp4Timestamp::from_mp4_seconds(u32::MAX as u64 + 1).fits_in_u32());
    }
}
