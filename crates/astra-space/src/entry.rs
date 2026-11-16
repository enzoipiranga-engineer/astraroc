//! Reentrada atmosférica planar com sustentação, aquecimento convectivo e paraquedas.

use crate::bodies::body;
use astra_env::AtmosphereModel;
use serde::{Deserialize, Serialize};

const G0: f64 = 9.806_65;
/// Constante de Stefan-Boltzmann, W/(m²·K⁴) (CODATA 2018).
const SIGMA: f64 = 5.670_374_419e-8;

/// Paraquedas de descida.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryChute {
    /// Diâmetro, m.
    pub diameter: f64,
    /// Cd.
    pub cd: f64,
    /// Abre abaixo deste Mach.
    pub deploy_mach: f64,
    /// E abaixo desta altitude, m.
    pub deploy_alt: f64,
}

/// Dados da reentrada.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    /// Corpo.
    pub body: String,
    /// Massa, kg.
    pub mass: f64,
    /// Diâmetro do escudo, m.
    pub diameter: f64,
    /// Cd hipersônico.
    pub cd: f64,
    /// Razão sustentação/arrasto.
    pub lift_to_drag: f64,
    /// Ângulo de rolamento, graus: a sustentação vertical é L·cos(rolamento) (0 = toda para cima).
    #[serde(default)]
    pub bank_deg: f64,
    /// Raio do nariz, m.
    pub nose_radius: f64,
    /// Emissividade da superfície.
    pub emissivity: f64,
    /// Altitude da interface de entrada, m.
    pub entry_alt: f64,
    /// Velocidade na interface, m/s.
    pub entry_speed: f64,
    /// Ângulo de trajetória na interface, graus (negativo = descendo).
    pub gamma_deg: f64,
    /// Paraquedas.
    #[serde(default)]
    pub chute: Option<EntryChute>,
    /// Temperatura máxima da proteção térmica, °C.
    #[serde(default)]
    pub tps_max_c: Option<f64>,
}

/// Linha: t, altitude, velocidade, γ (°), carga (g), fluxo de calor (W/m²), temperatura de
/// equilíbrio radiativo (K), Mach, distância percorrida (m).
pub type EntryRow = [f64; 9];

/// Resultado da reentrada.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryOutput {
    /// Perfil (a cada 0,5 s).
    pub rows: Vec<EntryRow>,
    /// Pico de desaceleração, g.
    pub peak_g: f64,
    /// Altitude do pico, m.
    pub peak_g_alt: f64,
    /// Pico de fluxo de calor no ponto de estagnação, W/m².
    pub peak_heat: f64,
    /// Carga térmica integrada, J/m².
    pub heat_load: f64,
    /// Temperatura máxima de equilíbrio radiativo, K.
    pub max_wall_temp: f64,
    /// Coeficiente balístico m/(Cd·A), kg/m².
    pub ballistic_coefficient: f64,
    /// Pico de desaceleração de Allen-Eggers (entrada balística, atmosfera exponencial), g.
    pub allen_eggers_g: Option<f64>,
    /// Abertura do paraquedas: (t, altitude, Mach).
    pub chute_open: Option<[f64; 3]>,
    /// Pico de desaceleração na abertura do paraquedas, g.
    pub chute_shock_g: f64,
    /// Velocidade ao tocar o solo, m/s.
    pub impact_speed: f64,
    /// Distância percorrida, m.
    pub downrange: f64,
    /// Duração, s.
    pub duration: f64,
    /// Saltou de volta para o espaço.
    pub skipped: bool,
    /// A proteção térmica aguenta.
    pub tps_ok: Option<bool>,
    /// Avisos.
    pub warnings: Vec<String>,
}

/// Simula a reentrada. Equações planares sobre planeta esférico sem rotação (Vinh, Busemann &
/// Culp, Hypersonic and Planetary Entry Flight Mechanics, 1980, eq. 2.10):
/// v̇ = −D/m − g·sin γ; γ̇ = L/(m·v) − (g/v − v/r)·cos γ; ḣ = v·sin γ; ṡ = v·cos γ·R/r.
/// Aquecimento de estagnação de Sutton-Graves q = k·√(ρ/r_n)·v³ (NASA TR R-376) e
/// temperatura de equilíbrio radiativo T = (q/(ε·σ))^¼.
///
/// # Errors
/// Corpo desconhecido ou sem atmosfera.
#[allow(clippy::too_many_lines)]
pub fn simulate(inp: &EntryInput) -> Result<EntryOutput, String> {
    let b = body(&inp.body).ok_or_else(|| format!("Corpo \"{}\" desconhecido.", inp.body))?;
    let atm = b
        .atmosphere
        .ok_or_else(|| format!("{} não tem atmosfera: use o pouso propulsivo.", b.name))?;
    let earth = b.id == "terra";
    let ussa = AtmosphereModel::Ussa1976 { delta_t: 0.0 };
    let air = |h: f64| -> (f64, f64) {
        if earth {
            let s = ussa.at(h.max(0.0));
            (s.density, s.sound_speed)
        } else {
            (atm.density(h), atm.sound_speed)
        }
    };
    let area = core::f64::consts::FRAC_PI_4 * inp.diameter * inp.diameter;
    let beta = inp.mass / (inp.cd * area);
    let mut chute_cda = 0.0;
    let mut warnings = Vec::new();
    // Estado: v, γ, h, s.
    let mut y = [
        inp.entry_speed,
        inp.gamma_deg.to_radians(),
        inp.entry_alt,
        0.0,
    ];
    let f = |y: &[f64; 4], cda_extra: f64| -> [f64; 4] {
        let (v, g, h) = (y[0].max(1e-3), y[1], y[2]);
        let r = b.radius + h;
        let grav = b.gm / (r * r);
        let (rho, _) = air(h);
        let q = 0.5 * rho * v * v;
        let d = q * (inp.cd * area + cda_extra) / inp.mass;
        let l = if cda_extra > 0.0 {
            0.0
        } else {
            inp.lift_to_drag * inp.bank_deg.to_radians().cos() * q * inp.cd * area / inp.mass
        };
        [
            -d - grav * g.sin(),
            l / v - (grav / v - v / r) * g.cos(),
            v * g.sin(),
            v * g.cos() * b.radius / r,
        ]
    };
    let mut t = 0.0;
    let dt = 0.02;
    let (mut peak_g, mut peak_g_alt, mut peak_heat, mut heat_load, mut max_tw) =
        (0.0f64, 0.0, 0.0f64, 0.0, 0.0f64);
    let mut rows = Vec::new();
    let mut next_row = 0.0;
    let mut chute_open: Option<[f64; 3]> = None;
    let mut chute_full = 0.0;
    let mut fill_time = 0.0;
    let mut chute_shock: f64 = 0.0;
    let mut skipped = false;
    while y[2] > 0.0 && t < 4.0 * 3600.0 {
        let (rho, a) = air(y[2]);
        let v = y[0];
        let mach = v / a;
        let qdot = atm.sutton_graves * (rho / inp.nose_radius).sqrt() * v * v * v;
        let tw = (qdot / (inp.emissivity * SIGMA)).powf(0.25);
        let qd = 0.5 * rho * v * v;
        let load = qd * (inp.cd * area + chute_cda) * (1.0 + inp.lift_to_drag.powi(2)).sqrt()
            / inp.mass
            / G0;
        if chute_open.is_some() {
            chute_shock = chute_shock.max(load);
        } else if load > peak_g {
            peak_g = load;
            peak_g_alt = y[2];
        }
        if chute_cda == 0.0 {
            peak_heat = peak_heat.max(qdot);
            max_tw = max_tw.max(tw);
        }
        heat_load += qdot * dt;
        if t >= next_row {
            rows.push([t, y[2], v, y[1].to_degrees(), load, qdot, tw, mach, y[3]]);
            next_row += 0.5;
        }
        if let Some(c) = &inp.chute {
            if chute_open.is_none() && mach < c.deploy_mach && y[2] < c.deploy_alt {
                chute_full = c.cd * core::f64::consts::FRAC_PI_4 * c.diameter * c.diameter;
                // Tempo de enchimento t_f = n·D/v, n ≈ 8 para velame sólido (Knacke,
                // Parachute Recovery Systems Design Manual, NWC TP 6575, §5.3); área ∝ (t/t_f)².
                fill_time = 8.0 * c.diameter / v.max(1.0);
                chute_open = Some([t, y[2], mach]);
            }
            if let Some(o) = chute_open {
                let f = ((t - o[0]) / fill_time).clamp(0.0, 1.0);
                chute_cda = (chute_full * f * f).max(1e-9);
            }
        }
        if y[2] > inp.entry_alt + 1.0 && y[1] > 0.0 {
            skipped = true;
            warnings.push("A cápsula ricocheteou na atmosfera e voltou ao espaço: ângulo de entrada raso demais.".into());
            break;
        }
        // RK4.
        let k1 = f(&y, chute_cda);
        let add = |y: &[f64; 4], k: &[f64; 4], s: f64| {
            [
                y[0] + s * k[0],
                y[1] + s * k[1],
                y[2] + s * k[2],
                y[3] + s * k[3],
            ]
        };
        let k2 = f(&add(&y, &k1, dt / 2.0), chute_cda);
        let k3 = f(&add(&y, &k2, dt / 2.0), chute_cda);
        let k4 = f(&add(&y, &k3, dt), chute_cda);
        for i in 0..4 {
            y[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        t += dt;
    }
    rows.push([
        t,
        y[2].max(0.0),
        y[0],
        y[1].to_degrees(),
        0.0,
        0.0,
        0.0,
        y[0] / air(0.0).1,
        y[3],
    ]);
    // Allen-Eggers: n_max = V_E²·sin|γ_E| / (2·e·H·g0), válido para entrada balística íngreme.
    let allen = (inp.lift_to_drag.abs() < 1e-9 && inp.gamma_deg.abs() > 5.0).then(|| {
        inp.entry_speed.powi(2) * inp.gamma_deg.to_radians().sin().abs()
            / (2.0 * core::f64::consts::E * atm.scale_height * G0)
    });
    if peak_g > 15.0 {
        warnings.push(format!("Pico de {peak_g:.0} g: acima do que tripulantes suportam (≈ 15 g por poucos segundos)."));
    }
    let tps_ok = inp.tps_max_c.map(|c| max_tw - 273.15 <= c);
    if tps_ok == Some(false) {
        warnings.push(format!(
            "A parede chegaria a {:.0} °C: acima do limite da proteção térmica.",
            max_tw - 273.15
        ));
    }
    Ok(EntryOutput {
        rows,
        peak_g,
        peak_g_alt,
        peak_heat,
        heat_load,
        max_wall_temp: max_tw,
        ballistic_coefficient: beta,
        allen_eggers_g: allen,
        chute_open,
        chute_shock_g: chute_shock,
        impact_speed: y[0],
        downrange: y[3],
        duration: t,
        skipped,
        tps_ok,
        warnings,
    })
}
