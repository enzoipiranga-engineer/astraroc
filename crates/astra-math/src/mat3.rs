use crate::Vec3;
use core::ops::Mul;

/// Matriz 3×3 em `f64`, armazenada por linhas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    /// Elementos `m[linha][coluna]`.
    pub m: [[f64; 3]; 3],
}

impl Mat3 {
    /// Matriz nula.
    pub const ZERO: Self = Self { m: [[0.0; 3]; 3] };
    /// Matriz identidade.
    pub const IDENTITY: Self = Self::diag(1.0, 1.0, 1.0);

    /// Matriz diagonal.
    #[must_use]
    pub const fn diag(a: f64, b: f64, c: f64) -> Self {
        Self {
            m: [[a, 0.0, 0.0], [0.0, b, 0.0], [0.0, 0.0, c]],
        }
    }

    /// Monta a matriz a partir das colunas.
    #[must_use]
    pub const fn from_cols(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self {
            m: [[a.x, b.x, c.x], [a.y, b.y, c.y], [a.z, b.z, c.z]],
        }
    }

    /// Transposta.
    #[must_use]
    pub fn transpose(self) -> Self {
        let m = self.m;
        Self {
            m: [
                [m[0][0], m[1][0], m[2][0]],
                [m[0][1], m[1][1], m[2][1]],
                [m[0][2], m[1][2], m[2][2]],
            ],
        }
    }

    /// Determinante.
    #[must_use]
    pub fn det(self) -> f64 {
        let m = self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// Inversa pela adjunta; `None` se a matriz for singular.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        let d = self.det();
        if d == 0.0 || !d.is_finite() {
            return None;
        }
        let m = self.m;
        let c = |r0: usize, c0: usize, r1: usize, c1: usize| {
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        Some(Self {
            m: [
                [c(1, 1, 2, 2) / d, -c(0, 1, 2, 2) / d, c(0, 1, 1, 2) / d],
                [-c(1, 0, 2, 2) / d, c(0, 0, 2, 2) / d, -c(0, 0, 1, 2) / d],
                [c(1, 0, 2, 1) / d, -c(0, 0, 2, 1) / d, c(0, 0, 1, 1) / d],
            ],
        })
    }
}

impl Mul<Vec3> for Mat3 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        let m = self.m;
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }
}

impl Mul for Mat3 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let mut r = Self::ZERO;
        for i in 0..3 {
            for j in 0..3 {
                r.m[i][j] = (0..3).map(|k| self.m[i][k] * o.m[k][j]).sum();
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_times_matrix_is_identity() {
        let a = Mat3 {
            m: [[2.0, -1.0, 0.5], [0.3, 4.0, -2.0], [1.0, 0.2, 3.0]],
        };
        let p = a.inverse().unwrap() * a;
        for i in 0..3 {
            for j in 0..3 {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!((p.m[i][j] - want).abs() < 1e-14);
            }
        }
        assert!(Mat3::ZERO.inverse().is_none());
    }
}
