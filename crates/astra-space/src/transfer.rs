//! Manobras e transferências: Hohmann, bi-elíptica, mudança de plano, Lambert e
//! transferências interplanetárias por cônicas combinadas (patched conics).

use crate::bodies::{body, Body};
use crate::kepler::stumpff;
use astra_math::Vec3;
use serde::Serialize;

/// Resultado de uma transferência entre órbitas circulares coplanares.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    /// Impulsos, m/s (em ordem).
    pub burns: Vec<f64>,
    /// Δv total, m/s.
    pub total: f64,
    /// Tempo de voo, s.
    pub tof: f64,
}

/// Hohmann entre órbitas circulares de raios r1 e r2 (Curtis, §6.2):
/// a_t = (r1+r2)/2, Δv1 = √(μ/r1)·(√(2r2/(r1+r2)) − 1), Δv2 = √(μ/r2)·(1 − √(2r1/(r1+r2))),
/// t = π·√(a_t³/μ).
#[must_use]
pub fn hohmann(mu: f64, r1: f64, r2: f64) -> Transfer {
    let at = 0.5 * (r1 + r2);
    let dv1 = ((mu / r1).sqrt() * ((2.0 * r2 / (r1 + r2)).sqrt() - 1.0)).abs();
    let dv2 = ((mu / r2).sqrt() * (1.0 - (2.0 * r1 / (r1 + r2)).sqrt())).abs();
    Transfer {
        burns: vec![dv1, dv2],
        total: dv1 + dv2,
        tof: core::f64::consts::PI * (at * at * at / mu).sqrt(),
    }
}

/// Bi-elíptica r1 → rb → r2 (Curtis, §6.3).
#[must_use]
pub fn bielliptic(mu: f64, r1: f64, rb: f64, r2: f64) -> Transfer {
    let v = |r: f64, a: f64| (mu * (2.0 / r - 1.0 / a)).sqrt();
    let (a1, a2) = (0.5 * (r1 + rb), 0.5 * (r2 + rb));
    let dv1 = v(r1, a1) - (mu / r1).sqrt();
    let dv2 = v(rb, a2) - v(rb, a1);
    let dv3 = (v(r2, a2) - (mu / r2).sqrt()).abs();
    let tof = core::f64::consts::PI * ((a1 * a1 * a1 / mu).sqrt() + (a2 * a2 * a2 / mu).sqrt());
    Transfer {
        burns: vec![dv1.abs(), dv2.abs(), dv3],
        total: dv1.abs() + dv2.abs() + dv3,
        tof,
    }
}

/// Mudança de plano pura numa órbita circular: Δv = 2·v·sin(Δi/2).
#[must_use]
pub fn plane_change(v: f64, di: f64) -> f64 {
    2.0 * v * (0.5 * di).sin()
}

/// Solução de Lambert.
#[derive(Clone, Copy, Debug)]
pub struct LambertSolution {
    /// Velocidade na partida.
    pub v1: Vec3,
    /// Velocidade na chegada.
    pub v2: Vec3,
}

/// Problema de Lambert por variáveis universais (Curtis, Algoritmo 5.2), sem voltas
/// completas. `prograde` escolhe o sentido do movimento (h no sentido +z).
///
/// # Errors
/// Quando não converge ou a geometria é degenerada (180° exatos).
pub fn lambert(
    mu: f64,
    r1: Vec3,
    r2: Vec3,
    dt: f64,
    prograde: bool,
) -> Result<LambertSolution, String> {
    let (r1n, r2n) = (r1.norm(), r2.norm());
    let c12 = r1.cross(r2);
    let mut theta = (r1.dot(r2) / (r1n * r2n)).clamp(-1.0, 1.0).acos();
    if (prograde && c12.z < 0.0) || (!prograde && c12.z >= 0.0) {
        theta = core::f64::consts::TAU - theta;
    }
    let a = theta.sin() * (r1n * r2n / (1.0 - theta.cos())).sqrt();
    if !a.is_finite() || a.abs() < 1e-12 {
        return Err("Geometria degenerada (ângulo de transferência de 0° ou 180°).".into());
    }
    let y = |z: f64| {
        let (c, s) = stumpff(z);
        r1n + r2n + a * (z * s - 1.0) / c.sqrt()
    };
    let f = |z: f64| {
        let (c, s) = stumpff(z);
        let yz = y(z);
        (yz / c).powf(1.5) * s + a * yz.sqrt() - mu.sqrt() * dt
    };
    // Encontra z inicial com y(z) > 0 e F mudando de sinal; bisseção robusta.
    let mut lo = -4.0 * core::f64::consts::PI * core::f64::consts::PI;
    while y(lo) < 0.0 {
        lo += 0.1;
    }
    let hi0 = 4.0 * core::f64::consts::PI * core::f64::consts::PI - 1e-6;
    let mut hi = hi0;
    if f(lo).signum() == f(hi).signum() {
        return Err("Lambert sem solução de meia volta para esse tempo de voo.".into());
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid).signum() == f(lo).signum() {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 1e-12 {
            break;
        }
    }
    let z = 0.5 * (lo + hi);
    let yz = y(z);
    let ff = 1.0 - yz / r1n;
    let g = a * (yz / mu).sqrt();
    let gd = 1.0 - yz / r2n;
    Ok(LambertSolution {
        v1: (r2 - r1 * ff) * (1.0 / g),
        v2: (r2 * gd - r1) * (1.0 / g),
    })
}

/// Transferência interplanetária de Hohmann por cônicas combinadas.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interplanetary {
    /// Excesso hiperbólico na partida, m/s.
    pub v_inf_dep: f64,
    /// Energia de lançamento C3 = v∞², m²/s².
    pub c3: f64,
    /// Δv para sair da órbita circular de estacionamento, m/s.
    pub dv_depart: f64,
    /// Excesso hiperbólico na chegada, m/s.
    pub v_inf_arr: f64,
    /// Δv para capturar numa órbita circular de chegada, m/s.
    pub dv_capture: f64,
    /// Velocidade de entrada na atmosfera do destino (sem captura), m/s.
    pub entry_speed: Option<f64>,
    /// Tempo de voo, s.
    pub tof: f64,
    /// Ângulo de fase na partida (destino à frente da origem), rad.
    pub phase: f64,
    /// Período sinódico, s.
    pub synodic: f64,
    /// Próximas partidas (s desde J2000) no modelo de órbitas circulares coplanares.
    pub windows: Vec<f64>,
}

/// Hohmann heliocêntrica de `from` para `to` com órbitas de estacionamento de altitude
/// `h_dep` e `h_arr` (Curtis, §8.6–8.9: Δv = √(v∞² + 2μ/r) − √(μ/r)). As janelas usam as
/// longitudes médias (órbitas circulares e coplanares): erro típico de algumas semanas.
///
/// # Errors
/// Corpos desconhecidos ou sem órbita heliocêntrica.
pub fn interplanetary(
    from: &str,
    to: &str,
    h_dep: f64,
    h_arr: f64,
    after: f64,
    count: usize,
) -> Result<Interplanetary, String> {
    let get = |id: &str| -> Result<&'static Body, String> {
        body(id)
            .filter(|b| b.helio.is_some())
            .ok_or_else(|| format!("\"{id}\" não é um planeta com órbita em torno do Sol."))
    };
    let (a, b) = (get(from)?, get(to)?);
    let sun = body("sol").ok_or("Sol ausente")?;
    let (ha, hb) = (a.helio.ok_or("")?, b.helio.ok_or("")?);
    let (r1, r2) = (ha.a, hb.a);
    let t = hohmann(sun.gm, r1, r2);
    let (vinf1, vinf2) = (t.burns[0], t.burns[1]);
    let rp1 = a.radius + h_dep;
    let rp2 = b.radius + h_arr;
    let dv_depart = (vinf1 * vinf1 + 2.0 * a.gm / rp1).sqrt() - (a.gm / rp1).sqrt();
    let dv_capture = (vinf2 * vinf2 + 2.0 * b.gm / rp2).sqrt() - (b.gm / rp2).sqrt();
    let entry_speed = b
        .atmosphere
        .map(|_| (vinf2 * vinf2 + 2.0 * b.gm / (b.radius + 125e3)).sqrt());
    // O destino percorre n2·t enquanto a sonda faz 180°: fase = π − n2·t.
    let n1 = ha.l_rate;
    let n2 = hb.l_rate;
    let phase = (core::f64::consts::PI - n2 * t.tof).rem_euclid(core::f64::consts::TAU);
    let synodic = core::f64::consts::TAU / (n1 - n2).abs();
    let mut windows = Vec::new();
    // Fase atual φ(t) = L2 − L1; procura φ = fase desejada.
    let rel = n2 - n1;
    let phi0 = hb.l0 - ha.l0;
    let mut tt = after;
    while windows.len() < count {
        let phi = (phi0 + rel * tt).rem_euclid(core::f64::consts::TAU);
        let mut d = (phase - phi).rem_euclid(core::f64::consts::TAU);
        if rel > 0.0 {
            // A fase cresce: espera d/rel.
        } else {
            d = core::f64::consts::TAU - d;
            if d >= core::f64::consts::TAU {
                d -= core::f64::consts::TAU;
            }
        }
        let wait = d / rel.abs();
        windows.push(tt + wait);
        tt += wait + 0.5 * synodic;
    }
    Ok(Interplanetary {
        v_inf_dep: vinf1,
        c3: vinf1 * vinf1,
        dv_depart,
        v_inf_arr: vinf2,
        dv_capture,
        entry_speed,
        tof: t.tof,
        phase,
        synodic,
        windows,
    })
}
