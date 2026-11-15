use crate::{Mat3, Vec3};
use core::ops::Mul;

/// Quaternion unitário de atitude, convenção de Hamilton, parte escalar `w` primeiro.
///
/// `q` representa a rotação do referencial do corpo para o referencial de referência:
/// `v_ref = q ⊗ v_corpo ⊗ q*`.
///
/// Fonte das convenções e equações: Stevens, Lewis & Johnson, "Aircraft Control and
/// Simulation", 3ª ed., §1.8; Diebel, "Representing Attitude" (Stanford, 2006).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    /// Parte escalar.
    pub w: f64,
    /// Parte vetorial, x.
    pub x: f64,
    /// Parte vetorial, y.
    pub y: f64,
    /// Parte vetorial, z.
    pub z: f64,
}

impl Quat {
    /// Rotação identidade.
    pub const IDENTITY: Self = Self {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Cria a partir das componentes (não normaliza).
    #[must_use]
    pub const fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    /// Rotação de `angle` rad em torno de `axis`: q = [cos(θ/2), û·sin(θ/2)].
    /// Retorna a identidade se o eixo for nulo.
    #[must_use]
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Self {
        match axis.normalized() {
            Some(u) => {
                let (s, c) = (0.5 * angle).sin_cos();
                Self::new(c, u.x * s, u.y * s, u.z * s)
            }
            None => Self::IDENTITY,
        }
    }

    /// Conjugado q* (inverso, para quaternion unitário).
    #[must_use]
    pub fn conj(self) -> Self {
        Self::new(self.w, -self.x, -self.y, -self.z)
    }

    /// Norma.
    #[must_use]
    pub fn norm(self) -> f64 {
        (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Renormaliza para norma 1 (corrige a deriva numérica da integração).
    #[must_use]
    pub fn normalized(self) -> Self {
        let n = self.norm();
        Self::new(self.w / n, self.x / n, self.y / n, self.z / n)
    }

    /// Roda um vetor: v' = q ⊗ v ⊗ q*, na forma v' = v + 2w(u×v) + 2u×(u×v),
    /// com u a parte vetorial (Diebel 2006, eq. 125 expandida).
    #[must_use]
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let u = Vec3::new(self.x, self.y, self.z);
        let t = 2.0 * u.cross(v);
        v + self.w * t + u.cross(t)
    }

    /// Matriz de rotação equivalente (colunas = eixos do corpo vistos na referência).
    #[must_use]
    pub fn to_mat3(self) -> Mat3 {
        Mat3::from_cols(
            self.rotate(Vec3::X),
            self.rotate(Vec3::Y),
            self.rotate(Vec3::Z),
        )
    }

    /// Quaternion a partir de uma matriz de rotação própria, pelo método de Shepperd
    /// (escolhe o maior pivô para evitar cancelamento).
    /// Fonte: Shepperd, "Quaternion from Rotation Matrix", J. Guidance and Control 1(3), 1978.
    #[must_use]
    pub fn from_mat3(r: Mat3) -> Self {
        let m = r.m;
        let tr = m[0][0] + m[1][1] + m[2][2];
        let q = if tr > 0.0 {
            let s = 2.0 * (1.0 + tr).sqrt();
            Self::new(
                0.25 * s,
                (m[2][1] - m[1][2]) / s,
                (m[0][2] - m[2][0]) / s,
                (m[1][0] - m[0][1]) / s,
            )
        } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
            let s = 2.0 * (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt();
            Self::new(
                (m[2][1] - m[1][2]) / s,
                0.25 * s,
                (m[0][1] + m[1][0]) / s,
                (m[0][2] + m[2][0]) / s,
            )
        } else if m[1][1] > m[2][2] {
            let s = 2.0 * (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt();
            Self::new(
                (m[0][2] - m[2][0]) / s,
                (m[0][1] + m[1][0]) / s,
                0.25 * s,
                (m[1][2] + m[2][1]) / s,
            )
        } else {
            let s = 2.0 * (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt();
            Self::new(
                (m[1][0] - m[0][1]) / s,
                (m[0][2] + m[2][0]) / s,
                (m[1][2] + m[2][1]) / s,
                0.25 * s,
            )
        };
        q.normalized()
    }

    /// Cinemática de atitude: q̇ = ½ · q ⊗ [0, ω], com ω a velocidade angular
    /// expressa no referencial do corpo, rad/s (Stevens & Lewis, eq. 1.8-15).
    /// O resultado é uma derivada, não um quaternion unitário.
    #[must_use]
    pub fn derivative(self, omega_body: Vec3) -> Self {
        let p = self * Self::new(0.0, omega_body.x, omega_body.y, omega_body.z);
        Self::new(0.5 * p.w, 0.5 * p.x, 0.5 * p.y, 0.5 * p.z)
    }
}

/// Produto de Hamilton: `a * b` aplica `b` primeiro e depois `a`.
impl Mul for Quat {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self::new(
            self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::{FRAC_PI_2, PI};

    fn close(a: Vec3, b: Vec3, tol: f64) -> bool {
        (a - b).norm() < tol
    }

    #[test]
    fn quarter_turn_about_z_maps_x_to_y() {
        let q = Quat::from_axis_angle(Vec3::Z, FRAC_PI_2);
        assert!(close(q.rotate(Vec3::X), Vec3::Y, 1e-15));
    }

    #[test]
    fn rotation_preserves_length_and_inverts() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 2.0, -0.5), 1.234);
        let v = Vec3::new(0.3, -4.0, 2.2);
        let r = q.rotate(v);
        assert!((r.norm() - v.norm()).abs() < 1e-14);
        assert!(close(q.conj().rotate(r), v, 1e-14));
    }

    #[test]
    fn composition_matches_sequential_rotation() {
        let a = Quat::from_axis_angle(Vec3::X, 0.7);
        let b = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 1.0), -1.1);
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!(close((a * b).rotate(v), a.rotate(b.rotate(v)), 1e-14));
    }

    #[test]
    fn matrix_round_trip_covers_all_pivots() {
        for (axis, ang) in [
            (Vec3::new(1.0, 2.0, -0.5), 0.4),
            (Vec3::X, 3.0),
            (Vec3::Y, 3.1),
            (Vec3::Z, -3.05),
        ] {
            let q = Quat::from_axis_angle(axis, ang);
            let q2 = Quat::from_mat3(q.to_mat3());
            let v = Vec3::new(0.3, -1.0, 2.0);
            assert!(close(q.rotate(v), q2.rotate(v), 1e-13));
        }
    }

    /// Integra q̇ = ½ q⊗ω com RK4 para ω constante e compara com a solução
    /// exata (rotação de |ω|·t em torno de ω̂).
    #[test]
    fn kinematics_integrates_to_exact_rotation() {
        let omega = Vec3::new(0.0, 0.0, PI); // meia volta por segundo em z
        let (mut q, n) = (Quat::IDENTITY, 1000);
        let h = 1.0 / n as f64;
        let axpy = |q: Quat, k: Quat, s: f64| {
            Quat::new(q.w + s * k.w, q.x + s * k.x, q.y + s * k.y, q.z + s * k.z)
        };
        for _ in 0..n {
            let k1 = q.derivative(omega);
            let k2 = axpy(q, k1, h / 2.0).derivative(omega);
            let k3 = axpy(q, k2, h / 2.0).derivative(omega);
            let k4 = axpy(q, k3, h).derivative(omega);
            for (k, s) in [(k1, h / 6.0), (k2, h / 3.0), (k3, h / 3.0), (k4, h / 6.0)] {
                q = axpy(q, k, s);
            }
        }
        assert!((q.norm() - 1.0).abs() < 1e-12, "deriva de norma");
        assert!(close(q.rotate(Vec3::X), -Vec3::X, 1e-10));
    }
}
