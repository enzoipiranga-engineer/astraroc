//! Guia retilínea: carrinho em rampa, trilho, canhão de gás, railgun, coilgun, mola,
//! estilingue, catapulta e plugins.

use crate::expr::Expr;
use crate::{direction, Base, Energy, LaunchOutcome, LocalEnv, Payload, Release, TrackPoint};
use astra_dynamics::ode::{integrate, Direction, EventSpec, Options};
use astra_math::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Pressão atmosférica padrão, Pa.
const P_ATM: f64 = 101_325.0;

/// Lei da força que empurra o conjunto ao longo da guia.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum Push {
    /// Só o motor do foguete.
    None,
    /// Força constante (cabo, coilgun idealizada), N.
    Constant {
        /// Força, N.
        force: f64,
    },
    /// Motor elétrico/linear de potência limitada: F = min(F_máx, P/v).
    Power {
        /// Potência, W.
        power: f64,
        /// Força máxima, N.
        max_force: f64,
    },
    /// Canhão de gás: expansão adiabática do reservatório, F = (p − p_atm)·A com
    /// p = p0·(V0/(V0 + A·x))^γ (Sutton & Biblarz cap. 3; gás ideal, sem atrito de gás).
    GasGun {
        /// Pressão inicial absoluta, Pa.
        p0: f64,
        /// Volume do reservatório, m³.
        volume: f64,
        /// Diâmetro do cano, m.
        bore: f64,
        /// γ do gás.
        gamma: f64,
    },
    /// Railgun: F = ½·L'·I² (Fair, "Electromagnetic launch science and technology",
    /// IEEE Trans. Magnetics 35, 1999).
    Railgun {
        /// Corrente, A.
        current: f64,
        /// Gradiente de indutância, H/m.
        inductance_gradient: f64,
    },
    /// Mola ou elástico linear: F = k·(curso − x) enquanto x < curso (Hooke).
    Spring {
        /// Rigidez, N/m.
        stiffness: f64,
        /// Curso, m.
        stroke: f64,
    },
    /// Expressão de plugin em x, v, t, m (e parâmetros), N.
    Expr {
        /// Texto (guardado para exportar).
        source: String,
        /// Variáveis fixas (parâmetros do plugin).
        consts: BTreeMap<String, f64>,
    },
}

/// Guia retilínea.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guide {
    /// Nome para a tela (Carrinho em rampa, Canhão de ar...).
    pub label: String,
    /// Comprimento útil, m.
    pub length: f64,
    /// Elevação, graus.
    pub elevation_deg: f64,
    /// Azimute, graus.
    pub azimuth_deg: f64,
    /// Massa do carrinho, pistão ou sabot que acompanha o foguete, kg.
    pub carriage_mass: f64,
    /// Coeficiente de atrito (rolamento ou deslizamento).
    pub friction: f64,
    /// Área de arrasto do carrinho, m².
    pub carriage_cd_a: f64,
    /// Empurrador.
    pub push: Push,
    /// Liberar ao atingir esta velocidade, m/s (senão, no fim da guia).
    #[serde(default)]
    pub release_speed: Option<f64>,
    /// Acender o motor no instante (relativo ao início do lançador), s; ausente = acende
    /// na liberação mais `ignition_delay`.
    #[serde(default)]
    pub ignite_at: Option<f64>,
    /// Atraso de ignição depois da liberação, s.
    #[serde(default)]
    pub ignition_delay: f64,
    /// Desaceleração de frenagem do carrinho depois da liberação, m/s².
    #[serde(default = "default_brake")]
    pub brake: f64,
    /// Altura da guia acima da base, m (cano elevado, rampa sobre torre).
    #[serde(default)]
    pub height: f64,
}

fn default_brake() -> f64 {
    50.0
}

impl Push {
    fn force(&self, x: f64, v: f64, t: f64, m: f64, compiled: Option<&Expr>) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Constant { force } => *force,
            Self::Power { power, max_force } => max_force.min(power / v.max(1e-6)),
            Self::GasGun {
                p0,
                volume,
                bore,
                gamma,
            } => {
                let a = 0.25 * core::f64::consts::PI * bore * bore;
                let p = p0 * (volume / (volume + a * x)).powf(*gamma);
                ((p - P_ATM) * a).max(0.0)
            }
            Self::Railgun {
                current,
                inductance_gradient,
            } => 0.5 * inductance_gradient * current * current,
            Self::Spring { stiffness, stroke } => (stiffness * (stroke - x)).max(0.0),
            Self::Expr { consts, .. } => {
                let mut env = consts.clone();
                env.insert("x".into(), x);
                env.insert("v".into(), v);
                env.insert("t".into(), t);
                env.insert("m".into(), m);
                compiled.map_or(0.0, |e| e.eval(&env))
            }
        }
    }
}

/// Integra o movimento do conjunto foguete + carrinho ao longo da guia.
///
/// Equação: (m_f + m_c)·ẍ = F_empurrador + T − (m_f + m_c)·g·sen θ − μ·N − ½ρv²(CdA_f + CdA_c),
/// com N = (m_f + m_c)·g·cos θ. Parado, só parte se a resultante for positiva (atrito
/// estático igual ao cinético). O estado carrega os acumuladores de energia para o
/// balanço: trabalho de entrada ∫(F + T)·v dt, dissipação ∫(atrito + arrasto)·v dt e o
/// fluxo de energia da massa expelida ∫ṁ·(v²/2 + g·h) dt.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn guided(gd: &Guide, base: Base, rocket: &dyn Payload, env: &LocalEnv) -> LaunchOutcome {
    let el = gd.elevation_deg.to_radians();
    let dir = direction(el, gd.azimuth_deg.to_radians());
    let ign = gd.ignite_at.map_or(f64::INFINITY, |t| base.t + t);
    let compiled = match &gd.push {
        Push::Expr { source, .. } => match Expr::parse(source) {
            Ok(e) => Some(e),
            Err(e) => {
                return LaunchOutcome {
                    warnings: vec![format!("Expressão da força inválida: {e}")],
                    ..LaunchOutcome::default()
                }
            }
        },
        _ => None,
    };
    let start_pos = base.pos + Vec3::new(0.0, 0.0, gd.height);
    let mc = gd.carriage_mass;
    let mass = |t: f64| rocket.mass(t, ign) + mc;
    let h_of = |x: f64| start_pos.z + x * el.sin();
    let rhs = |t: f64, y: &[f64; 5]| -> [f64; 5] {
        let (x, v) = (y[0], y[1]);
        let m = mass(t);
        let z = h_of(x);
        let push = gd.push.force(x, v, t - base.t, m, compiled.as_ref());
        let thrust = rocket.thrust(t, ign, env.pressure(z));
        let drive = push + thrust - m * env.g * el.sin();
        let fric = gd.friction * m * env.g * el.cos();
        let drag = 0.5 * env.rho(z) * v * v.abs() * (rocket.drag_area() + gd.carriage_cd_a);
        let mut a = (drive - fric * v.signum() - drag) / m;
        if v <= 0.0 && drive <= fric {
            a = 0.0;
        }
        let a = if v <= 0.0 { a.max(0.0) } else { a };
        // dm/dt do foguete (negativo) por diferença central do modelo de massa.
        let h = 1e-4;
        let mdot = (mass(t + h) - mass(t - h)) / (2.0 * h);
        [
            v,
            a,
            (push + thrust) * v,
            (fric * v.signum() + drag) * v,
            mdot * (0.5 * v * v + env.g * (z - start_pos.z)),
        ]
    };
    let len = gd.length;
    let g_end = move |_: f64, y: &[f64; 5]| y[0] - len;
    let rs = gd.release_speed.unwrap_or(f64::INFINITY);
    let g_speed = move |_: f64, y: &[f64; 5]| y[1] - rs;
    let events = [
        EventSpec {
            g: &g_end,
            direction: Direction::Rising,
            terminal: true,
        },
        EventSpec {
            g: &g_speed,
            direction: Direction::Rising,
            terminal: true,
        },
    ];
    let mut pts: Vec<(f64, [f64; 5])> = vec![(base.t, [0.0; 5])];
    let opts = Options {
        rtol: 1e-10,
        atol: 1e-9,
        h_max: 0.01,
        max_steps: 2_000_000,
    };
    let res = integrate(
        &rhs,
        base.t,
        [0.0; 5],
        base.t + 600.0,
        &events,
        &opts,
        |s, tv| {
            let n = 8;
            for k in 1..=n {
                let tt = s.t0 + (tv - s.t0) * f64::from(k) / f64::from(n);
                pts.push((tt, s.dense(tt)));
            }
        },
    );
    let mut out = LaunchOutcome::default();
    let Ok(res) = res else {
        out.warnings
            .push(format!("{}: a integração falhou.", gd.label));
        return out;
    };
    if res.terminated_by.is_none() {
        out.warnings.push(format!(
            "{}: o conjunto não chegou ao fim da guia em 10 minutos (força insuficiente?).",
            gd.label
        ));
        return out;
    }
    let (t1, y1) = (res.t, res.y);
    let m1 = mass(t1);
    let m0 = mass(base.t);
    let v1 = y1[1];
    // Balanço de energia (frame da base).
    let ke = 0.5 * m1 * v1 * v1;
    let pe = m1 * env.g * y1[0] * el.sin();
    // O termo de massa: energia mecânica que sai com o propelente (m·(v²/2 + g h) com
    // dm < 0) entra com sinal; ver doc.
    let pe0 = 0.0;
    let _ = m0;
    let scale = y1[2].abs().max(ke).max(1e-9);
    let residual = ((y1[2] - y1[3] + y1[4]) - (ke + pe - pe0)) / scale;
    out.energy = Energy {
        input: y1[2],
        dissipated: y1[3],
        kinetic: ke,
        potential: pe,
        mass_flux: y1[4],
        residual: residual.abs(),
        rocket_kinetic: 0.5 * rocket.mass(t1, ign) * v1 * v1,
    };
    // Cargas no foguete: aceleração ao longo do eixo e componente lateral da gravidade
    // (reação da guia).
    for w in pts.windows(2) {
        let (ta, ya) = w[0];
        let (tb, yb) = w[1];
        if tb > ta {
            let ax = (yb[1] - ya[1]) / (tb - ta) + env.g * el.sin();
            out.loads
                .push((0.5 * (ta + tb), ax / 9.806_65, el.cos() * env.g / 9.806_65));
        }
    }
    let track: Vec<TrackPoint> = pts
        .iter()
        .map(|(t, y)| TrackPoint {
            t: *t,
            pos: start_pos + dir * y[0] + base.vel * (t - base.t),
            vel: dir * y[1] + base.vel,
            axis: dir,
        })
        .collect();
    // Carrinho freando depois da liberação.
    if mc > 0.0 {
        let stop = v1 / gd.brake.max(1e-3);
        let mut cart = Vec::new();
        for k in 0..=20 {
            let dt = stop * f64::from(k) / 20.0;
            let x = y1[0] + v1 * dt - 0.5 * gd.brake * dt * dt;
            cart.push(TrackPoint {
                t: t1 + dt,
                pos: start_pos + dir * x + base.vel * (t1 + dt - base.t),
                vel: dir * (v1 - gd.brake * dt) + base.vel,
                axis: dir,
            });
        }
        out.events.push((
            t1 + stop,
            "cartStop".into(),
            format!(
                "O carrinho para em {:.0} m de frenagem, dissipando {:.1} kJ.",
                v1 * v1 / (2.0 * gd.brake.max(1e-3)),
                0.5 * mc * v1 * v1 / 1e3
            ),
        ));
        out.tracks
            .push(("Foguete no lançador".into(), track.clone()));
        let mut full = track;
        full.extend(cart);
        out.tracks.push((format!("Carrinho ({})", gd.label), full));
    } else {
        out.tracks.push(("Foguete no lançador".into(), track));
    }
    out.events.push((
        t1,
        "release".into(),
        format!(
            "{}: liberação a {:.1} m/s após {:.2} m de guia.",
            gd.label, v1, y1[0]
        ),
    ));
    out.release = Some(Release {
        t: t1,
        pos: start_pos + dir * y1[0] + base.vel * (t1 - base.t),
        vel: dir * v1 + base.vel,
        axis: dir,
        omega: Vec3::ZERO,
        ignition: ign,
        ignition_delay: gd.ignition_delay,
        mass: rocket.mass(t1, ign),
    });
    out
}
