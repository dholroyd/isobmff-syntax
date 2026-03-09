//! Fixed-point numeric types used in ISOBMFF structures.

use std::fmt;

/// A 16.16 fixed-point number stored as a signed 32-bit integer.
///
/// The upper 16 bits represent the integer part, and the lower 16 bits
/// represent the fractional part. For example, `0x00010000` represents 1.0.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FixedPoint16_16(i32);

impl FixedPoint16_16 {
    /// The value representing 1.0.
    pub const ONE: Self = Self(0x0001_0000);

    /// The value representing 0.0.
    pub const ZERO: Self = Self(0);

    /// Creates a new fixed-point value from a raw 32-bit integer.
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Creates a fixed-point value from an integer (no fractional part).
    #[inline]
    pub const fn from_int(value: i16) -> Self {
        Self((value as i32) << 16)
    }

    /// Returns the raw 32-bit integer representation.
    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Converts the fixed-point value to a 32-bit float.
    #[inline]
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 65536.0
    }

    /// Converts the fixed-point value to a 64-bit float.
    #[inline]
    pub fn as_f64(self) -> f64 {
        self.0 as f64 / 65536.0
    }
}

impl From<i32> for FixedPoint16_16 {
    #[inline]
    fn from(raw: i32) -> Self {
        Self(raw)
    }
}

impl From<FixedPoint16_16> for i32 {
    #[inline]
    fn from(fp: FixedPoint16_16) -> Self {
        fp.0
    }
}

impl fmt::Debug for FixedPoint16_16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FixedPoint16_16({} = {:.6})", self.0, self.as_f64())
    }
}

impl fmt::Display for FixedPoint16_16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6}", self.as_f64())
    }
}

/// An 8.8 fixed-point number stored as a signed 16-bit integer.
///
/// The upper 8 bits represent the integer part, and the lower 8 bits
/// represent the fractional part. For example, `0x0100` represents 1.0.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FixedPoint8_8(i16);

impl FixedPoint8_8 {
    /// The value representing 1.0.
    pub const ONE: Self = Self(0x0100);

    /// The value representing 0.0.
    pub const ZERO: Self = Self(0);

    /// Creates a new fixed-point value from a raw 16-bit integer.
    #[inline]
    pub const fn from_raw(raw: i16) -> Self {
        Self(raw)
    }

    /// Creates a fixed-point value from an integer (no fractional part).
    #[inline]
    pub const fn from_int(value: i8) -> Self {
        Self((value as i16) << 8)
    }

    /// Returns the raw 16-bit integer representation.
    #[inline]
    pub const fn raw(self) -> i16 {
        self.0
    }

    /// Converts the fixed-point value to a 32-bit float.
    #[inline]
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 256.0
    }

    /// Converts the fixed-point value to a 64-bit float.
    #[inline]
    pub fn as_f64(self) -> f64 {
        self.0 as f64 / 256.0
    }
}

impl From<i16> for FixedPoint8_8 {
    #[inline]
    fn from(raw: i16) -> Self {
        Self(raw)
    }
}

impl From<FixedPoint8_8> for i16 {
    #[inline]
    fn from(fp: FixedPoint8_8) -> Self {
        fp.0
    }
}

impl fmt::Debug for FixedPoint8_8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FixedPoint8_8({} = {:.4})", self.0, self.as_f64())
    }
}

impl fmt::Display for FixedPoint8_8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.as_f64())
    }
}

/// A 2.30 fixed-point number stored as a signed 32-bit integer.
///
/// The upper 2 bits represent the integer part, and the lower 30 bits
/// represent the fractional part. This format is used for matrix
/// u, v, and w values in the transformation matrix.
///
/// For example, `0x40000000` represents 1.0.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FixedPoint2_30(i32);

impl FixedPoint2_30 {
    /// The value representing 1.0 (0x40000000).
    pub const ONE: Self = Self(0x4000_0000);

    /// The value representing 0.0.
    pub const ZERO: Self = Self(0);

    /// Creates a new fixed-point value from a raw 32-bit integer.
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    /// Returns the raw 32-bit integer representation.
    #[inline]
    pub const fn raw(self) -> i32 {
        self.0
    }

    /// Converts the fixed-point value to a 32-bit float.
    #[inline]
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 1073741824.0 // 2^30
    }

    /// Converts the fixed-point value to a 64-bit float.
    #[inline]
    pub fn as_f64(self) -> f64 {
        self.0 as f64 / 1073741824.0 // 2^30
    }
}

impl From<i32> for FixedPoint2_30 {
    #[inline]
    fn from(raw: i32) -> Self {
        Self(raw)
    }
}

impl From<FixedPoint2_30> for i32 {
    #[inline]
    fn from(fp: FixedPoint2_30) -> Self {
        fp.0
    }
}

impl fmt::Debug for FixedPoint2_30 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FixedPoint2_30({} = {:.10})", self.0, self.as_f64())
    }
}

impl fmt::Display for FixedPoint2_30 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.10}", self.as_f64())
    }
}

/// An unsigned 16.16 fixed-point number stored as an unsigned 32-bit integer.
///
/// The upper 16 bits represent the integer part, and the lower 16 bits
/// represent the fractional part. For example, `0x00010000` represents 1.0.
///
/// This is used for spec fields declared as `unsigned int(32)` that represent
/// unsigned 16.16 fixed-point values (e.g., tkhd width/height, sample entry
/// resolutions and sample rate).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct UFixedPoint16_16(u32);

impl UFixedPoint16_16 {
    /// The value representing 1.0.
    pub const ONE: Self = Self(0x0001_0000);

    /// The value representing 0.0.
    pub const ZERO: Self = Self(0);

    /// Creates a new fixed-point value from a raw 32-bit unsigned integer.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Creates a fixed-point value from an unsigned integer (no fractional part).
    #[inline]
    pub const fn from_int(value: u16) -> Self {
        Self((value as u32) << 16)
    }

    /// Creates a fixed-point value from separate integer and fractional parts.
    #[inline]
    pub const fn from_parts(integer: u16, fraction: u16) -> Self {
        Self((integer as u32) << 16 | fraction as u32)
    }

    /// Creates a fixed-point value from an `f32`.
    #[inline]
    pub fn from_f32(value: f32) -> Self {
        Self((value * 65536.0) as u32)
    }

    /// Returns the raw 32-bit unsigned integer representation.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Converts the fixed-point value to a 32-bit float.
    #[inline]
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 65536.0
    }

    /// Converts the fixed-point value to a 64-bit float.
    #[inline]
    pub fn as_f64(self) -> f64 {
        self.0 as f64 / 65536.0
    }
}

impl From<u32> for UFixedPoint16_16 {
    #[inline]
    fn from(raw: u32) -> Self {
        Self(raw)
    }
}

impl From<UFixedPoint16_16> for u32 {
    #[inline]
    fn from(fp: UFixedPoint16_16) -> Self {
        fp.0
    }
}

impl fmt::Debug for UFixedPoint16_16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UFixedPoint16_16({} = {:.6})", self.0, self.as_f64())
    }
}

impl fmt::Display for UFixedPoint16_16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6}", self.as_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_16_16_one() {
        let one = FixedPoint16_16::ONE;
        assert_eq!(one.raw(), 0x0001_0000);
        assert!((one.as_f32() - 1.0).abs() < f32::EPSILON);
        assert!((one.as_f64() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fixed_16_16_from_int() {
        let two = FixedPoint16_16::from_int(2);
        assert_eq!(two.raw(), 0x0002_0000);
        assert!((two.as_f32() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fixed_16_16_half() {
        let half = FixedPoint16_16::from_raw(0x0000_8000);
        assert!((half.as_f64() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn fixed_8_8_one() {
        let one = FixedPoint8_8::ONE;
        assert_eq!(one.raw(), 0x0100);
        assert!((one.as_f32() - 1.0).abs() < f32::EPSILON);
        assert!((one.as_f64() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fixed_8_8_half() {
        let half = FixedPoint8_8::from_raw(0x0080);
        assert!((half.as_f64() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn fixed_2_30_one() {
        let one = FixedPoint2_30::ONE;
        assert_eq!(one.raw(), 0x4000_0000);
        assert!((one.as_f32() - 1.0).abs() < f32::EPSILON);
        assert!((one.as_f64() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fixed_2_30_half() {
        let half = FixedPoint2_30::from_raw(0x2000_0000);
        assert!((half.as_f64() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ufixed_16_16_one() {
        let one = UFixedPoint16_16::ONE;
        assert_eq!(one.raw(), 0x0001_0000);
        assert!((one.as_f32() - 1.0).abs() < f32::EPSILON);
        assert!((one.as_f64() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ufixed_16_16_from_int() {
        let two = UFixedPoint16_16::from_int(2);
        assert_eq!(two.raw(), 0x0002_0000);
        assert!((two.as_f32() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn ufixed_16_16_half() {
        let half = UFixedPoint16_16::from_raw(0x0000_8000);
        assert!((half.as_f64() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn ufixed_16_16_from_parts() {
        let val = UFixedPoint16_16::from_parts(1920, 0);
        assert_eq!(val.raw(), 1920 << 16);
        assert!((val.as_f32() - 1920.0).abs() < f32::EPSILON);

        let val = UFixedPoint16_16::from_parts(1, 0x8000);
        assert!((val.as_f64() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn ufixed_16_16_from_f32() {
        let val = UFixedPoint16_16::from_f32(1920.0);
        assert_eq!(val.raw(), 1920 << 16);

        let val = UFixedPoint16_16::from_f32(1.5);
        assert_eq!(val.raw(), 0x0001_8000);
    }
}
