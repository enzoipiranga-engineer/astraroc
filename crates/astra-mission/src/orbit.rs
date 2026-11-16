use astra_math::Vec3;
use core::f64::consts::PI;
use serde::Serialize;

/// Elementos orbitais clássicos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrbitElements {
    /// Semieixo maior, m (negativo para hipérbole).
    pub a: f64,
    /// Excentricidade.
    pub e: f64,
    /// Inclinação, graus.
    pub inc_deg: f64,
    /// Ascensão reta do nodo ascendente, graus.
    pub raan_deg: f64,
    /// Argumento do perigeu, graus.
    pub argp_deg: f64,
    /// Anomalia verdadeira, graus.
    pub nu_deg: f64,
    /// Altitude do perigeu sobre o raio equatorial, m.
    pub perigee_alt: f64,
    /// Altitude do apogeu sobre o raio equatorial, m (infinita se não for fechada).
    pub apogee_alt: f64,
    /// Período, s (infinito se não for fechada).
    pub period: f64,
}

/// Posição e velocidade inerciais → elementos clássicos.
/// Fonte: Vallado, "Fundamentals of Astrodynamics and Applications", 4ª ed., alg. 9
/// (RV2COE). Casos degenerados (órbita circular ou equatorial) devolvem ângulos 0.
#[must_use]
pub fn elements(r: Vec3, v: Vec3, mu: f64, re: f64) -> OrbitElements {
    let (rn, vn) = (r.norm(), v.norm());
    let h = r.cross(v);
    let n = Vec3::Z.cross(h);
    let ev = ((vn * vn - mu / rn) * r - r.dot(v) * v) / mu;
    let e = ev.norm();
    let energy = 0.5 * vn * vn - mu / rn;
    let a = -mu / (2.0 * energy);
    let inc = (h.z / h.norm()).clamp(-1.0, 1.0).acos();
    let ang = |x: f64| x.clamp(-1.0, 1.0).acos();
    let raan = if n.norm() > 1e-9 {
        let w = ang(n.x / n.norm());
        if n.y < 0.0 {
            2.0 * PI - w
        } else {
            w
        }
    } else {
        0.0
    };
    let argp = if n.norm() > 1e-9 && e > 1e-9 {
        let w = ang(n.dot(ev) / (n.norm() * e));
        if ev.z < 0.0 {
            2.0 * PI - w
        } else {
            w
        }
    } else {
        0.0
    };
    let nu = if e > 1e-9 {
        let w = ang(ev.dot(r) / (e * rn));
        if r.dot(v) < 0.0 {
            2.0 * PI - w
        } else {
            w
        }
    } else {
        0.0
    };
    let closed = e < 1.0;
    OrbitElements {
        a,
        e,
        inc_deg: inc.to_degrees(),
        raan_deg: raan.to_degrees(),
        argp_deg: argp.to_degrees(),
        nu_deg: nu.to_degrees(),
        perigee_alt: a * (1.0 - e) - re,
        apogee_alt: if closed {
            a * (1.0 + e) - re
        } else {
            f64::INFINITY
        },
        period: if closed {
            2.0 * PI * (a.powi(3) / mu).sqrt()
        } else {
            f64::INFINITY
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vallado, exemplo 2-5: r = (6524,834; 6862,875; 6448,296) km,
    /// v = (4,901327; 5,533756; −1,976341) km/s → p = 11067,79 km, a = 36127,343 km,
    /// e = 0,832853, i = 87,870°, Ω = 227,898°, ω = 53,38°, ν = 92,335°.
    #[test]
    fn vallado_example_2_5() {
        let mu = 3.986_004_418e14;
        let r = Vec3::new(6_524.834e3, 6_862.875e3, 6_448.296e3);
        let v = Vec3::new(4.901_327e3, 5.533_756e3, -1.976_341e3);
        let o = elements(r, v, mu, 6_378_137.0);
        assert!((o.a / 36_127.343e3 - 1.0).abs() < 1e-4, "a {}", o.a);
        assert!((o.e - 0.832_853).abs() < 1e-5);
        assert!((o.inc_deg - 87.870).abs() < 1e-3);
        assert!((o.raan_deg - 227.898).abs() < 1e-3);
        assert!((o.argp_deg - 53.38).abs() < 1e-2);
        assert!((o.nu_deg - 92.335).abs() < 1e-3);
    }

    #[test]
    fn circular_orbit() {
        let mu = 3.986_004_418e14;
        let rr = 6_778_137.0;
        let o = elements(
            Vec3::new(rr, 0.0, 0.0),
            Vec3::new(0.0, (mu / rr).sqrt(), 0.0),
            mu,
            6_378_137.0,
        );
        assert!(
            o.e < 1e-12
                && (o.perigee_alt - 400e3).abs() < 1e-3
                && (o.apogee_alt - 400e3).abs() < 1e-3
        );
        assert!((o.period - 2.0 * PI * (rr.powi(3) / mu).sqrt()).abs() < 1e-9);
    }
}
