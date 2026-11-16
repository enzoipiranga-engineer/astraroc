//! Pouso propulsivo planar por curva de gravidade (empuxo contra a velocidade), com escolha
//! automática do instante de ignição.

use crate::bodies::body;
use astra_env::AtmosphereModel;
use serde::{Deserialize, Serialize};

const G0: f64 = 9.806_65;

/// Dados do pouso.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandingInput {
    /// Corpo.
    pub body: String,
    /// Altitude inicial, m.
    pub altitude: f64,
    /// Velocidade horizontal inicial, m/s.
    pub vx: f64,
    /// Velocidade vertical inicial, m/s (negativa = descendo).
    pub vz: f64,
    /// Massa inicial, kg.
    pub mass: f64,
    /// Propelente disponível, kg.
    pub propellant: f64,
    /// Empuxo máximo, N.
    pub thrust: f64,
    /// Impulso específico, s.
    pub isp: f64,
    /// Área de arrasto Cd·A, m² (0 sem atmosfera).
    #[serde(default)]
    pub cd_a: f64,
}

/// Linha: t, x, altitude, vx, vz, massa, aceleração de empuxo (g), ligado (0/1).
pub type LandingRow = [f64; 8];

/// Resultado.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandingOutput {
    /// Trajetória (a cada 0,2 s).
    pub rows: Vec<LandingRow>,
    /// Instante de ignição, s.
    pub ignition_time: f64,
    /// Altitude na ignição, m.
    pub ignition_alt: f64,
    /// Velocidade ao tocar o solo, m/s.
    pub touchdown_speed: f64,
    /// Propelente gasto, kg.
    pub propellant_used: f64,
    /// Δv gasto, m/s.
    pub dv: f64,
    /// Pousou com segurança (menos de 3 m/s e propelente suficiente).
    pub success: bool,
    /// Explicação.
    pub message: String,
    /// Nível de empuxo usado (1 = máximo).
    pub throttle: f64,
}

struct Run {
    rows: Vec<LandingRow>,
    end_alt: f64,
    touchdown: f64,
    used: f64,
    empty: bool,
}

fn run(inp: &LandingInput, t_ig: f64, throttle: f64, record: bool) -> Result<Run, String> {
    let b = body(&inp.body).ok_or_else(|| format!("Corpo \"{}\" desconhecido.", inp.body))?;
    let ussa = AtmosphereModel::Ussa1976 { delta_t: 0.0 };
    let rho = |h: f64| match (b.id, b.atmosphere) {
        ("terra", _) => ussa.at(h.max(0.0)).density,
        (_, Some(a)) => a.density(h),
        _ => 0.0,
    };
    let (mut x, mut h, mut vx, mut vz, mut m) = (0.0, inp.altitude, inp.vx, inp.vz, inp.mass);
    let m_dry = inp.mass - inp.propellant;
    let mdot_max = throttle * inp.thrust / (inp.isp * G0);
    let dt = 0.01;
    let mut t = 0.0;
    let mut rows = Vec::new();
    let mut next = 0.0;
    let mut burning = false;
    let mut done_burn = false;
    let mut empty = false;
    loop {
        let r = b.radius + h;
        let g = b.gm / (r * r);
        let v = vx.hypot(vz);
        if !done_burn && t >= t_ig {
            burning = true;
        }
        // Fim da queima: velocidade quase nula (ou já subindo sem velocidade horizontal).
        if burning && (v < 0.5 || (vz >= 0.0 && vx.abs() < 1.0)) {
            burning = false;
            done_burn = true;
            if !record {
                return Ok(Run {
                    rows,
                    end_alt: h,
                    touchdown: (2.0 * g * h.max(0.0)).sqrt(),
                    used: inp.mass - m,
                    empty,
                });
            }
            // Fase terminal: desce a 1 m/s com empuxo igual ao peso até tocar o solo.
            if h > 0.0 && m > m_dry {
                let t_hover = h;
                let mdot = m * g / (inp.isp * G0);
                let used = (mdot * t_hover).min(m - m_dry);
                for k in 1..=((t_hover / 0.2).ceil() as usize) {
                    let tk = (k as f64 * 0.2).min(t_hover);
                    rows.push([
                        t + tk,
                        x,
                        (h - tk).max(0.0),
                        0.0,
                        -1.0,
                        m - used * tk / t_hover,
                        g / G0,
                        1.0,
                    ]);
                }
                return Ok(Run {
                    rows,
                    end_alt: 0.0,
                    touchdown: 1.0,
                    used: inp.mass - m + used,
                    empty: used < mdot * t_hover,
                });
            }
        }
        if burning && m <= m_dry {
            burning = false;
            done_burn = true;
            empty = true;
        }
        let thrust = if burning { throttle * inp.thrust } else { 0.0 };
        let (ux, uz) = if v > 1e-6 {
            (-vx / v, -vz / v)
        } else {
            (0.0, 1.0)
        };
        let drag = 0.5 * rho(h) * v * v * inp.cd_a;
        let ax = (thrust * ux + drag * ux) / m;
        let az = (thrust * uz + drag * uz) / m - g + vx * vx / r;
        if record && t >= next {
            rows.push([
                t,
                x,
                h,
                vx,
                vz,
                m,
                thrust / m / G0,
                f64::from(u8::from(burning)),
            ]);
            next += 0.2;
        }
        // Euler semi-implícito em passos pequenos.
        vx += ax * dt;
        vz += az * dt;
        x += vx * dt;
        h += vz * dt;
        if burning {
            m -= mdot_max * dt;
        }
        t += dt;
        if h <= 0.0 {
            let touchdown = vx.hypot(vz);
            if record {
                rows.push([t, x, 0.0, vx, vz, m, 0.0, 0.0]);
            }
            return Ok(Run {
                rows,
                end_alt: if done_burn || burning { 0.0 } else { -1.0 },
                touchdown,
                used: inp.mass - m,
                empty,
            });
        }
        if t > 7200.0 {
            // Em órbita sem tocar o solo: conta como "parou acima".
            return Ok(Run {
                rows,
                end_alt: f64::INFINITY,
                touchdown: 0.0,
                used: inp.mass - m,
                empty,
            });
        }
    }
}

/// Descida guiada a partir da órbita: a componente vertical do empuxo segue
/// v_z,ref = −(h − h_portão)/t_go (t_go = |v_x|/a_max; portão = 5 % da altitude inicial, entre
/// 100 m e 2 km, onde a horizontal deve estar anulada), mais a
/// gravidade menos a aceleração centrípeta; o resto do empuxo freia a horizontal. Quando a
/// horizontal some, passa à descida vertical v_z,ref = −√(2·0,5·(T/m − g)·(h − 3 m)), limitada a
/// −1 m/s perto do solo. Lei de guiagem simplificada (no espírito da descida do Apollo, mas
/// sem a guiagem polinomial do computador de bordo).
fn guided(inp: &LandingInput) -> Result<Run, String> {
    let b = body(&inp.body).ok_or_else(|| format!("Corpo \"{}\" desconhecido.", inp.body))?;
    let ussa = AtmosphereModel::Ussa1976 { delta_t: 0.0 };
    let rho = |h: f64| match (b.id, b.atmosphere) {
        ("terra", _) => ussa.at(h.max(0.0)).density,
        (_, Some(a)) => a.density(h),
        _ => 0.0,
    };
    let (mut x, mut h, mut vx, mut vz, mut m) = (0.0, inp.altitude, inp.vx, inp.vz, inp.mass);
    let m_dry = inp.mass - inp.propellant;
    // Portão: altitude em que a horizontal deve ter sido anulada.
    let gate = (0.05 * inp.altitude).clamp(100.0, 2000.0);
    let dt = 0.01;
    let (mut t, mut next) = (0.0, 0.0);
    let mut rows = Vec::new();
    let mut empty = false;
    loop {
        let r = b.radius + h;
        let g = b.gm / (r * r);
        let v = vx.hypot(vz);
        let amax = if m > m_dry { inp.thrust / m } else { 0.0 };
        empty |= m <= m_dry;
        let net_g = g - vx * vx / r;
        let (ax_t, az_t) = if amax <= 0.0 {
            (0.0, 0.0)
        } else if vx.abs() > 1.0 {
            let tgo = vx.abs() / amax.max(1e-6);
            let vz_ref = -((h - gate) / tgo).max(0.0);
            let az = (net_g + (vz_ref - vz) / 3.0).clamp(0.0, amax);
            let ax = (amax * amax - az * az).max(0.0).sqrt() * -vx.signum();
            // Não inverte a horizontal num passo.
            (
                if (vx + ax * dt).signum() == vx.signum() {
                    ax
                } else {
                    -vx / dt
                },
                az,
            )
        } else {
            let vz_ref = -(2.0 * 0.5 * (amax - g).max(0.0) * (h - 3.0).max(0.0))
                .sqrt()
                .max(1.0);
            (
                (-vx / 1.0).clamp(-amax, amax),
                (g + (vz_ref - vz) / 0.5).clamp(0.0, amax),
            )
        };
        let a_thr = ax_t.hypot(az_t);
        let drag = 0.5 * rho(h) * v * v * inp.cd_a / m;
        let (ux, uz) = if v > 1e-6 {
            (-vx / v, -vz / v)
        } else {
            (0.0, 0.0)
        };
        if t >= next {
            rows.push([
                t,
                x,
                h,
                vx,
                vz,
                m,
                a_thr / G0,
                f64::from(u8::from(a_thr > 0.0)),
            ]);
            next += 0.2;
        }
        vx += (ax_t + drag * ux) * dt;
        vz += (az_t + drag * uz - net_g) * dt;
        x += vx * dt;
        h += vz * dt;
        m -= m * a_thr / (inp.isp * G0) * dt;
        t += dt;
        if h <= 0.0 {
            rows.push([t, x, 0.0, vx, vz, m, 0.0, 0.0]);
            return Ok(Run {
                rows,
                end_alt: 0.0,
                touchdown: vx.hypot(vz),
                used: inp.mass - m,
                empty,
            });
        }
        if t > 7200.0 {
            return Err("A descida guiada não chegou ao solo em 2 h.".into());
        }
    }
}

/// Simula o pouso. Partindo de trajetória suborbital, a ignição é escolhida por bisseção para
/// que a queima a pleno empuxo anule a velocidade perto do solo (≈ 1 m acima); partindo de
/// órbita, o motor liga já e a bisseção escolhe o nível de empuxo constante: a "queima suicida" da curva de gravidade
/// (Citron, Dunin & Meissinger, "A terminal guidance technique for lunar landing", AIAA J.
/// 2(3), 1964, para a descida por curva de gravidade). Integração de Euler semi-implícito
/// com passo de 0,01 s.
///
/// # Errors
/// Corpo desconhecido.
pub fn simulate(inp: &LandingInput) -> Result<LandingOutput, String> {
    let b = body(&inp.body).ok_or_else(|| format!("Corpo \"{}\" desconhecido.", inp.body))?;
    let g = b.surface_gravity();
    if inp.thrust <= inp.mass * g {
        return Err(format!(
            "Empuxo/peso de {:.2} em {}: o veículo não consegue frear.",
            inp.thrust / (inp.mass * g),
            b.name
        ));
    }
    // Sem motor, toca o solo? (início suborbital). Senão, está em órbita: liga já e escolhe o
    // nível de empuxo.
    let coast = run(inp, f64::INFINITY, 1.0, false)?;
    let suborbital = coast.end_alt.is_finite();
    if !suborbital {
        let r = guided(inp)?;
        let dv = inp.isp * G0 * (inp.mass / (inp.mass - r.used)).ln();
        let success = !r.empty && r.touchdown < 3.0;
        let message = if r.empty {
            "O propelente acabou antes do pouso.".to_owned()
        } else if success {
            format!(
                "Descida guiada da órbita: pouso a {:.1} m/s com {:.0} kg de propelente sobrando.",
                r.touchdown,
                inp.propellant - r.used
            )
        } else {
            format!("Descida guiada toca o solo a {:.1} m/s.", r.touchdown)
        };
        return Ok(LandingOutput {
            ignition_time: 0.0,
            ignition_alt: inp.altitude,
            touchdown_speed: r.touchdown,
            propellant_used: r.used,
            dv,
            success,
            message,
            rows: r.rows,
            throttle: 1.0,
        });
    }
    let stops_above = |t: f64| run(inp, t, 1.0, false).map(|r| r.end_alt > 1.0);
    if !stops_above(0.0)? {
        let r = run(inp, 0.0, 1.0, true)?;
        return Ok(LandingOutput {
            ignition_time: 0.0,
            ignition_alt: inp.altitude,
            touchdown_speed: r.touchdown,
            propellant_used: r.used,
            dv: inp.isp * G0 * (inp.mass / (inp.mass - r.used)).ln(),
            success: false,
            throttle: 1.0,
            message: format!("Mesmo com empuxo máximo desde o início, toca o solo a {:.1} m/s: falta empuxo ou altitude.", r.touchdown),
            rows: r.rows,
        });
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    while run(inp, hi, 1.0, false)?.end_alt > -0.5 && hi < 7200.0 {
        hi *= 2.0;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if stops_above(mid)? {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let k = 1.0;
    let r = run(inp, lo, k, true)?;
    let ig = r
        .rows
        .iter()
        .find(|row| row[7] > 0.5)
        .map_or(inp.altitude, |row| row[2]);
    let dv = inp.isp * G0 * (inp.mass / (inp.mass - r.used)).ln();
    let success = !r.empty && r.touchdown < 3.0;
    let message = if r.empty {
        format!(
            "O propelente acabou antes de parar: faltaram {:.0} kg ou mais.",
            (inp.mass - r.used).min(0.0).abs()
        )
    } else if success {
        format!(
            "Pouso a {:.1} m/s com {:.0} kg de propelente sobrando.",
            r.touchdown,
            inp.propellant - r.used
        )
    } else {
        format!("Toca o solo a {:.1} m/s.", r.touchdown)
    };
    Ok(LandingOutput {
        ignition_time: lo,
        ignition_alt: ig,
        touchdown_speed: r.touchdown,
        propellant_used: r.used,
        dv,
        success,
        message,
        rows: r.rows,
        throttle: k,
    })
}
