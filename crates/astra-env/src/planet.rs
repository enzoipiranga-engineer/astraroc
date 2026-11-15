use astra_math::{constants::wgs84, Vec3};
use core::f64::consts::FRAC_PI_4;

/// Posição geodésica sobre o elipsoide do planeta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geodetic {
    /// Latitude geodésica, rad (positiva ao norte).
    pub lat: f64,
    /// Longitude, rad (positiva a leste).
    pub lon: f64,
    /// Altitude acima do elipsoide, m.
    pub h: f64,
}

/// Corpo central: elipsoide de revolução com rotação uniforme em torno de z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Planet {
    /// Parâmetro gravitacional GM, m³/s².
    pub mu: f64,
    /// Raio equatorial, m.
    pub re: f64,
    /// Achatamento (adimensional).
    pub f: f64,
    /// Velocidade angular de rotação, rad/s.
    pub omega: f64,
    /// Harmônico zonal J2 (adimensional).
    pub j2: f64,
}

/// Roda `v` de `angle` rad em torno do eixo z.
#[must_use]
pub fn rot_z(v: Vec3, angle: f64) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z)
}

impl Planet {
    /// Terra WGS 84 (ver `astra_math::constants::wgs84`).
    pub const EARTH: Self = Self {
        mu: wgs84::GM,
        re: wgs84::A,
        f: wgs84::F,
        omega: wgs84::OMEGA,
        j2: wgs84::J2,
    };

    /// Mesmo planeta sem rotação (para testes analíticos em referencial inercial).
    #[must_use]
    pub fn non_rotating(self) -> Self {
        Self { omega: 0.0, ..self }
    }

    /// Mesmo planeta como esfera de massa pontual (sem achatamento e sem J2).
    #[must_use]
    pub fn spherical(self) -> Self {
        Self {
            f: 0.0,
            j2: 0.0,
            ..self
        }
    }

    /// Primeira excentricidade ao quadrado, e² = f(2 − f).
    #[must_use]
    pub fn e2(&self) -> f64 {
        self.f * (2.0 - self.f)
    }

    /// Vetor velocidade angular do planeta (igual em ECEF e ECI).
    #[must_use]
    pub fn omega_vec(&self) -> Vec3 {
        Vec3::new(0.0, 0.0, self.omega)
    }

    /// Geodésico → cartesiano fixo ao planeta (ECEF).
    /// x = (N + h) cosφ cosλ, y = (N + h) cosφ sinλ, z = (N(1 − e²) + h) sinφ,
    /// com N = a / √(1 − e² sin²φ).
    /// Fonte: NGA.STND.0036_1.0.0_WGS84, eq. 4-14.
    #[must_use]
    pub fn geodetic_to_ecef(&self, g: Geodetic) -> Vec3 {
        let (sl, cl) = g.lat.sin_cos();
        let (so, co) = g.lon.sin_cos();
        let n = self.re / (1.0 - self.e2() * sl * sl).sqrt();
        Vec3::new(
            (n + g.h) * cl * co,
            (n + g.h) * cl * so,
            (n * (1.0 - self.e2()) + g.h) * sl,
        )
    }

    /// Cartesiano → geodésico, por iteração de ponto fixo na latitude
    /// (converge por fator ~e² a cada passo; 8 passos chegam ao arredondamento).
    /// Vale também para coordenadas ECI: a longitude devolvida é então a inercial.
    /// Fonte: Hofmann-Wellenhof, Lichtenegger & Collins, "GPS: Theory and Practice", §10.2.
    #[must_use]
    pub fn ecef_to_geodetic(&self, r: Vec3) -> Geodetic {
        let e2 = self.e2();
        let p = r.x.hypot(r.y);
        let lon = r.y.atan2(r.x);
        if p < 1e-9 {
            let b = self.re * (1.0 - self.f);
            return Geodetic {
                lat: core::f64::consts::FRAC_PI_2.copysign(r.z),
                lon: 0.0,
                h: r.z.abs() - b,
            };
        }
        let height = |lat: f64| {
            let s = lat.sin();
            let n = self.re / (1.0 - e2 * s * s).sqrt();
            let h = if lat.abs() < FRAC_PI_4 {
                p / lat.cos() - n
            } else {
                r.z / s - n * (1.0 - e2)
            };
            (n, h)
        };
        let mut lat = r.z.atan2(p * (1.0 - e2));
        for _ in 0..8 {
            let (n, h) = height(lat);
            lat = r.z.atan2(p * (1.0 - e2 * n / (n + h)));
        }
        Geodetic {
            lat,
            lon,
            h: height(lat).1,
        }
    }

    /// Versores (leste, norte, cima) do plano tangente local, no mesmo referencial
    /// cartesiano em que `lat`/`lon` foram medidos.
    #[must_use]
    pub fn enu_basis(lat: f64, lon: f64) -> (Vec3, Vec3, Vec3) {
        let (sl, cl) = lat.sin_cos();
        let (so, co) = lon.sin_cos();
        (
            Vec3::new(-so, co, 0.0),
            Vec3::new(-sl * co, -sl * so, cl),
            Vec3::new(cl * co, cl * so, sl),
        )
    }

    /// Aceleração gravitacional (só atração de massa, sem centrífuga), m/s².
    /// Termo central mais J2:
    /// a = −μ r/r³ · [1 + 1,5·J2·(Re/r)²·(1 − 5z²/r²)] nas componentes x e y,
    /// e (3 − 5z²/r²) na componente z.
    /// Fonte: Vallado, "Fundamentals of Astrodynamics and Applications", 4ª ed., cap. 8
    /// (derivada do potencial U = μ/r·[1 − J2 (Re/r)² P₂(sinφ)]); Bate, Mueller & White §9.
    #[must_use]
    pub fn gravity(&self, r: Vec3) -> Vec3 {
        let r2 = r.norm_sq();
        if r2 == 0.0 {
            return Vec3::ZERO;
        }
        let rn = r2.sqrt();
        let k = -self.mu / (r2 * rn);
        let j = 1.5 * self.j2 * self.re * self.re / r2;
        let z2 = 5.0 * r.z * r.z / r2;
        Vec3::new(
            k * r.x * (1.0 + j * (1.0 - z2)),
            k * r.y * (1.0 + j * (1.0 - z2)),
            k * r.z * (1.0 + j * (3.0 - z2)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geodetic_round_trip() {
        let p = Planet::EARTH;
        for &(lat, lon, h) in &[
            (0.0, 0.0, 0.0),
            (-0.0405, -0.774, 44.0), // Alcântara, aprox.
            (1.2, 2.5, 120_000.0),
            (-1.55, -3.0, 4.0e7),
            (0.7, 1.0, -300.0),
        ] {
            let g = p.ecef_to_geodetic(p.geodetic_to_ecef(Geodetic { lat, lon, h }));
            assert!((g.lat - lat).abs() < 1e-12, "lat {lat}");
            assert!((g.lon - lon).abs() < 1e-12, "lon {lon}");
            assert!((g.h - h).abs() < 1e-6, "h {h}: {}", g.h);
        }
    }

    #[test]
    fn poles_and_equator_radii() {
        let p = Planet::EARTH;
        let eq = p.geodetic_to_ecef(Geodetic {
            lat: 0.0,
            lon: 0.0,
            h: 0.0,
        });
        assert!((eq.x - wgs84::A).abs() < 1e-9);
        let pole = p.geodetic_to_ecef(Geodetic {
            lat: core::f64::consts::FRAC_PI_2,
            lon: 0.0,
            h: 0.0,
        });
        assert!((pole.z - wgs84::B).abs() < 1e-6);
        let g = p.ecef_to_geodetic(Vec3::new(0.0, 0.0, -wgs84::B - 10.0));
        assert!((g.h - 10.0).abs() < 1e-6 && g.lat < 0.0);
    }

    /// Gravidade efetiva (atração + centrífuga) no equador e no polo contra a
    /// gravidade normal WGS 84: γe = 9,7803253359 e γp = 9,8321849379 m/s²
    /// (NGA.STND.0036, Tabela 3.6). Só J2 (sem J4 e superiores) acerta até ~1e-4 m/s².
    #[test]
    fn effective_gravity_matches_wgs84_normal_gravity() {
        let p = Planet::EARTH;
        let re = Vec3::new(wgs84::A, 0.0, 0.0);
        let ge = -(p.gravity(re).x) - p.omega * p.omega * wgs84::A;
        assert!((ge - 9.780_325_335_9).abs() < 2e-4, "equador {ge}");
        let gp = -p.gravity(Vec3::new(0.0, 0.0, wgs84::B)).z;
        assert!((gp - 9.832_184_937_9).abs() < 2e-4, "polo {gp}");
    }

    #[test]
    fn enu_is_right_handed_and_up_is_normal() {
        let (e, n, u) = Planet::enu_basis(0.6, -1.1);
        assert!((e.cross(n) - u).norm() < 1e-15);
        let p = Planet::EARTH;
        let a = p.geodetic_to_ecef(Geodetic {
            lat: 0.6,
            lon: -1.1,
            h: 0.0,
        });
        let b = p.geodetic_to_ecef(Geodetic {
            lat: 0.6,
            lon: -1.1,
            h: 1000.0,
        });
        assert!(((b - a) / 1000.0 - u).norm() < 1e-9);
    }
}
