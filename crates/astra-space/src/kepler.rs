//! Problema de dois corpos: elementos ↔ estado e propagação por variáveis universais.

use astra_math::Vec3;
use serde::{Deserialize, Serialize};

/// Elementos clássicos.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Elements {
    /// Semieixo maior, m (negativo na hipérbole).
    pub a: f64,
    /// Excentricidade.
    pub e: f64,
    /// Inclinação, rad.
    pub i: f64,
    /// Longitude do nó ascendente, rad.
    pub raan: f64,
    /// Argumento do periapside, rad.
    pub argp: f64,
    /// Anomalia verdadeira, rad.
    pub nu: f64,
}

/// Estado a partir dos elementos (Curtis, Algoritmo 4.5).
#[must_use]
pub fn coe2rv(mu: f64, k: &Elements) -> (Vec3, Vec3) {
    let p = k.a * (1.0 - k.e * k.e);
    let (s, c) = k.nu.sin_cos();
    let r_pf = Vec3::new(p * c / (1.0 + k.e * c), p * s / (1.0 + k.e * c), 0.0);
    let v_pf = Vec3::new(-(mu / p).sqrt() * s, (mu / p).sqrt() * (k.e + c), 0.0);
    let rot = |v: Vec3| {
        let (so, co) = k.raan.sin_cos();
        let (si, ci) = k.i.sin_cos();
        let (sw, cw) = k.argp.sin_cos();
        Vec3::new(
            (co * cw - so * sw * ci) * v.x + (-co * sw - so * cw * ci) * v.y,
            (so * cw + co * sw * ci) * v.x + (-so * sw + co * cw * ci) * v.y,
            (sw * si) * v.x + (cw * si) * v.y,
        )
    };
    (rot(r_pf), rot(v_pf))
}

/// Elementos a partir do estado (Curtis, Algoritmo 4.2).
#[must_use]
pub fn rv2coe(mu: f64, r: Vec3, v: Vec3) -> Elements {
    let rn = r.norm();
    let h = r.cross(v);
    let hn = h.norm();
    let i = (h.z / hn).clamp(-1.0, 1.0).acos();
    let n = Vec3::new(-h.y, h.x, 0.0);
    let nn = n.norm();
    let ev = (r * (v.norm_sq() - mu / rn) - v * r.dot(v)) * (1.0 / mu);
    let e = ev.norm();
    let raan = if nn > 1e-12 {
        let o = (n.x / nn).clamp(-1.0, 1.0).acos();
        if n.y < 0.0 {
            core::f64::consts::TAU - o
        } else {
            o
        }
    } else {
        0.0
    };
    let argp = if nn > 1e-12 && e > 1e-12 {
        let w = (n.dot(ev) / (nn * e)).clamp(-1.0, 1.0).acos();
        if ev.z < 0.0 {
            core::f64::consts::TAU - w
        } else {
            w
        }
    } else {
        0.0
    };
    let nu = if e > 1e-12 {
        let t = (ev.dot(r) / (e * rn)).clamp(-1.0, 1.0).acos();
        if r.dot(v) < 0.0 {
            core::f64::consts::TAU - t
        } else {
            t
        }
    } else {
        let t = (r.x / rn).clamp(-1.0, 1.0).acos();
        if r.y < 0.0 {
            core::f64::consts::TAU - t
        } else {
            t
        }
    };
    let energy = v.norm_sq() / 2.0 - mu / rn;
    Elements {
        a: -mu / (2.0 * energy),
        e,
        i,
        raan,
        argp,
        nu,
    }
}

/// Funções de Stumpff C(z) e S(z) (Curtis, eq. 3.52).
#[must_use]
pub fn stumpff(z: f64) -> (f64, f64) {
    if z > 1e-8 {
        let s = z.sqrt();
        ((1.0 - s.cos()) / z, (s - s.sin()) / (s * s * s))
    } else if z < -1e-8 {
        let s = (-z).sqrt();
        ((s.cosh() - 1.0) / -z, (s.sinh() - s) / (s * s * s))
    } else {
        (0.5 - z / 24.0, 1.0 / 6.0 - z / 120.0)
    }
}

/// Propaga o estado por `dt` com variáveis universais (Curtis, Algoritmos 3.3 e 3.4).
#[must_use]
pub fn propagate(mu: f64, r0: Vec3, v0: Vec3, dt: f64) -> (Vec3, Vec3) {
    let r0n = r0.norm();
    let vr0 = r0.dot(v0) / r0n;
    let alpha = 2.0 / r0n - v0.norm_sq() / mu;
    let smu = mu.sqrt();
    let mut x = smu * alpha.abs() * dt;
    for _ in 0..100 {
        let z = alpha * x * x;
        let (c, s) = stumpff(z);
        let f =
            r0n * vr0 / smu * x * x * c + (1.0 - alpha * r0n) * x * x * x * s + r0n * x - smu * dt;
        let df =
            r0n * vr0 / smu * x * (1.0 - alpha * x * x * s) + (1.0 - alpha * r0n) * x * x * c + r0n;
        let dx = f / df;
        x -= dx;
        if dx.abs() < 1e-10 * (1.0 + x.abs()) {
            break;
        }
    }
    let z = alpha * x * x;
    let (c, s) = stumpff(z);
    let f = 1.0 - x * x / r0n * c;
    let g = dt - x * x * x / smu * s;
    let r = r0 * f + v0 * g;
    let rn = r.norm();
    let fd = smu / (rn * r0n) * (alpha * x * x * x * s - x);
    let gd = 1.0 - x * x / rn * c;
    (r, r0 * fd + v0 * gd)
}

/// Período orbital, s: T = 2π·√(a³/μ).
#[must_use]
pub fn period(mu: f64, a: f64) -> f64 {
    core::f64::consts::TAU * (a * a * a / mu).sqrt()
}
