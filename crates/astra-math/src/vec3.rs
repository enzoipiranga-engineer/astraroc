use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Vetor 3D cartesiano em `f64`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    /// Componente x.
    pub x: f64,
    /// Componente y.
    pub y: f64,
    /// Componente z.
    pub z: f64,
}

impl Vec3 {
    /// Vetor nulo.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    /// Versor x.
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    /// Versor y.
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    /// Versor z.
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    /// Cria um vetor a partir das componentes.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Produto escalar a·b.
    #[must_use]
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Produto vetorial a×b (regra da mão direita).
    #[must_use]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Norma euclidiana ao quadrado.
    #[must_use]
    pub fn norm_sq(self) -> f64 {
        self.dot(self)
    }

    /// Norma euclidiana.
    #[must_use]
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Versor na mesma direção; `None` se a norma for zero ou não finita.
    #[must_use]
    pub fn normalized(self) -> Option<Self> {
        let n = self.norm();
        (n > 0.0 && n.is_finite()).then(|| self / n)
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}
impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}
impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_follows_right_hand_rule() {
        assert_eq!(Vec3::X.cross(Vec3::Y), Vec3::Z);
        assert_eq!(Vec3::Y.cross(Vec3::Z), Vec3::X);
        assert_eq!(Vec3::Z.cross(Vec3::X), Vec3::Y);
    }

    #[test]
    fn lagrange_identity() {
        // |a×b|² + (a·b)² = |a|²|b|²
        let a = Vec3::new(1.5, -2.0, 0.25);
        let b = Vec3::new(-0.7, 3.1, 4.0);
        let lhs = a.cross(b).norm_sq() + a.dot(b).powi(2);
        assert!((lhs - a.norm_sq() * b.norm_sq()).abs() < 1e-12);
    }

    #[test]
    fn normalized_rejects_zero() {
        assert!(Vec3::ZERO.normalized().is_none());
        let n = Vec3::new(3.0, 4.0, 0.0).normalized().unwrap();
        assert!((n.norm() - 1.0).abs() < 1e-15);
    }
}
