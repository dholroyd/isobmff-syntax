//! Transformation matrix type used in ISOBMFF structures.

use super::fixed_point::{FixedPoint16_16, FixedPoint2_30};
use std::fmt;
use std::ops::Index;

/// A 3x3 transformation matrix as defined in ISO 14496-12.
///
/// The matrix is stored in row-major order as 9 raw `i32` values.
/// Elements have mixed fixed-point types per the spec:
/// - Elements [0],[1],[3],[4],[6],[7] (a, b, c, d, x, y) are **16.16 fixed-point**
/// - Elements [2],[5],[8] (u, v, w) are **2.30 fixed-point**
///
/// ```text
/// | a  b  u |     | [0] [1] [2] |
/// | c  d  v |  =  | [3] [4] [5] |
/// | x  y  w |     | [6] [7] [8] |
/// ```
///
/// The identity matrix is:
/// ```text
/// | 1.0  0.0  0.0 |
/// | 0.0  1.0  0.0 |
/// | 0.0  0.0  1.0 |
/// ```
/// which is represented as `[0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000]`
/// in the specification (a,b,c,d,x,y use 16.16 where 1.0 = 0x10000;
/// u,v,w use 2.30 where 1.0 = 0x40000000).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Matrix([i32; 9]);

impl Matrix {
    /// The identity matrix.
    pub const IDENTITY: Self = Self([
        0x0001_0000, // a = 1.0 (16.16)
        0,           // b = 0.0 (16.16)
        0,           // u = 0.0 (2.30)
        0,           // c = 0.0 (16.16)
        0x0001_0000, // d = 1.0 (16.16)
        0,           // v = 0.0 (2.30)
        0,           // x = 0.0 (16.16)
        0,           // y = 0.0 (16.16)
        0x4000_0000, // w = 1.0 (2.30)
    ]);

    /// Creates a matrix from raw 32-bit integer values.
    pub fn from_raw(values: [i32; 9]) -> Self {
        Self(values)
    }

    /// Returns the matrix elements as raw 32-bit integers.
    #[inline]
    pub const fn as_raw(&self) -> &[i32; 9] {
        &self.0
    }

    /// Returns the raw element at the given row and column (0-indexed).
    ///
    /// # Panics
    ///
    /// Panics if `row >= 3` or `col >= 3`.
    #[inline]
    pub const fn get_raw(&self, row: usize, col: usize) -> i32 {
        self.0[row * 3 + col]
    }

    /// Returns element `a` (row 0, col 0) as 16.16 fixed-point.
    #[inline]
    pub const fn a(&self) -> FixedPoint16_16 {
        FixedPoint16_16::from_raw(self.0[0])
    }

    /// Returns element `b` (row 0, col 1) as 16.16 fixed-point.
    #[inline]
    pub const fn b(&self) -> FixedPoint16_16 {
        FixedPoint16_16::from_raw(self.0[1])
    }

    /// Returns element `u` (row 0, col 2) as 2.30 fixed-point.
    #[inline]
    pub const fn u(&self) -> FixedPoint2_30 {
        FixedPoint2_30::from_raw(self.0[2])
    }

    /// Returns element `c` (row 1, col 0) as 16.16 fixed-point.
    #[inline]
    pub const fn c(&self) -> FixedPoint16_16 {
        FixedPoint16_16::from_raw(self.0[3])
    }

    /// Returns element `d` (row 1, col 1) as 16.16 fixed-point.
    #[inline]
    pub const fn d(&self) -> FixedPoint16_16 {
        FixedPoint16_16::from_raw(self.0[4])
    }

    /// Returns element `v` (row 1, col 2) as 2.30 fixed-point.
    #[inline]
    pub const fn v(&self) -> FixedPoint2_30 {
        FixedPoint2_30::from_raw(self.0[5])
    }

    /// Returns element `x` (row 2, col 0) as 16.16 fixed-point.
    #[inline]
    pub const fn x(&self) -> FixedPoint16_16 {
        FixedPoint16_16::from_raw(self.0[6])
    }

    /// Returns element `y` (row 2, col 1) as 16.16 fixed-point.
    #[inline]
    pub const fn y(&self) -> FixedPoint16_16 {
        FixedPoint16_16::from_raw(self.0[7])
    }

    /// Returns element `w` (row 2, col 2) as 2.30 fixed-point.
    #[inline]
    pub const fn w(&self) -> FixedPoint2_30 {
        FixedPoint2_30::from_raw(self.0[8])
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<[i32; 9]> for Matrix {
    fn from(values: [i32; 9]) -> Self {
        Self(values)
    }
}

impl From<Matrix> for [i32; 9] {
    fn from(matrix: Matrix) -> Self {
        matrix.0
    }
}

impl Index<usize> for Matrix {
    type Output = i32;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

/// Helper to convert a raw i32 to f64 using the correct fixed-point type for
/// the given matrix element index.
fn element_as_f64(index: usize, raw: i32) -> f64 {
    match index {
        2 | 5 | 8 => FixedPoint2_30::from_raw(raw).as_f64(),
        _ => FixedPoint16_16::from_raw(raw).as_f64(),
    }
}

impl fmt::Debug for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Matrix([{}, {}, {}, {}, {}, {}, {}, {}, {}])",
            element_as_f64(0, self.0[0]),
            element_as_f64(1, self.0[1]),
            element_as_f64(2, self.0[2]),
            element_as_f64(3, self.0[3]),
            element_as_f64(4, self.0[4]),
            element_as_f64(5, self.0[5]),
            element_as_f64(6, self.0[6]),
            element_as_f64(7, self.0[7]),
            element_as_f64(8, self.0[8]),
        )
    }
}

impl fmt::Display for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "| {:>8.4} {:>8.4} {:>8.4} |",
            element_as_f64(0, self.0[0]),
            element_as_f64(1, self.0[1]),
            element_as_f64(2, self.0[2]),
        )?;
        writeln!(
            f,
            "| {:>8.4} {:>8.4} {:>8.4} |",
            element_as_f64(3, self.0[3]),
            element_as_f64(4, self.0[4]),
            element_as_f64(5, self.0[5]),
        )?;
        write!(
            f,
            "| {:>8.4} {:>8.4} {:>8.4} |",
            element_as_f64(6, self.0[6]),
            element_as_f64(7, self.0[7]),
            element_as_f64(8, self.0[8]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_matrix() {
        let identity = Matrix::IDENTITY;
        assert_eq!(identity.get_raw(0, 0), 0x0001_0000);
        assert_eq!(identity.get_raw(0, 1), 0);
        assert_eq!(identity.get_raw(1, 1), 0x0001_0000);
        assert_eq!(identity.get_raw(2, 2), 0x4000_0000);
        assert_eq!(identity.a(), FixedPoint16_16::ONE);
        assert_eq!(identity.d(), FixedPoint16_16::ONE);
        assert_eq!(identity.w(), FixedPoint2_30::ONE);
    }

    #[test]
    fn from_raw() {
        let raw = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000];
        let matrix = Matrix::from_raw(raw);
        assert_eq!(matrix, Matrix::IDENTITY);
    }

    #[test]
    fn index_access() {
        let matrix = Matrix::IDENTITY;
        assert_eq!(matrix[0], 0x0001_0000);
        assert_eq!(matrix[4], 0x0001_0000);
        assert_eq!(matrix[8], 0x4000_0000);
    }
}
