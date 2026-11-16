//! Plataforma giratória, lançamento aéreo, balão, navio e submarino.

use crate::guided::{guided, Guide, Push};
use crate::{direction, Base, Energy, LaunchOutcome, LocalEnv, Payload, Release, TrackPoint};
use astra_dynamics::ode::{integrate, Direction, EventSpec, Options};
use astra_math::Vec3;
use core::f64::consts::PI;
use serde::{Deserialize, Serialize};

/// Plataforma giratória (spin launch): braço que gira num plano vertical e solta o
/// foguete tangencialmente.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spin {
    /// Raio do braço até o foguete, m.
    pub radius: f64,
    /// Massa do braço (barra uniforme dos dois lados, com contrapeso), kg.
    pub arm_mass: f64,
    /// Torque do motor, N·m.
    pub torque: f64,
    /// Velocidade de rotação alvo, rpm.
    pub target_rpm: f64,
    /// Pressão na câmara (0 = vácuo; pressão local = câmara aberta), Pa.
    pub chamber_pressure: f64,
    /// Coeficiente de arrasto × largura do braço (para o torque de arrasto), m.
    pub arm_cd_width: f64,
    /// Elevação da velocidade na liberação, graus.
    pub release_elevation_deg: f64,
    /// Azimute do plano de giro, graus.
    pub azimuth_deg: f64,
    /// Altura do eixo acima da base, m.
    pub hub_height: f64,
    /// Atraso de ignição depois da liberação, s.
    pub ignition_delay: f64,
}

/// Lançamento aéreo: o avião solta o foguete em voo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirLaunch {
    /// Altura de liberação acima da base, m.
    pub altitude: f64,
    /// Velocidade verdadeira do avião, m/s.
    pub speed: f64,
    /// Ângulo de trajetória do avião (subida), graus.
    pub flight_path_deg: f64,
    /// Proa, graus.
    pub heading_deg: f64,
    /// Queda livre antes da ignição, s.
    pub drop_time: f64,
    /// Massa do avião, kg (só para o balanço de quantidade de movimento).
    pub aircraft_mass: f64,
}

/// Balão estratosférico de pressão zero (rockoon).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balloon {
    /// Massa do envelope, kg.
    pub envelope_mass: f64,
    /// Massa de gás, kg.
    pub gas_mass: f64,
    /// Massa molar do gás, kg/kmol (hélio 4,0026; hidrogênio 2,016).
    pub gas_molar_mass: f64,
    /// Massa da gôndola e do lançador, kg.
    pub gondola_mass: f64,
    /// Diâmetro de estouro, m.
    pub burst_diameter: f64,
    /// Coeficiente de arrasto do balão.
    pub cd: f64,
    /// Altura de liberação acima da base, m.
    pub release_altitude: f64,
    /// Elevação do trilho da gôndola, graus.
    pub rail_elevation_deg: f64,
    /// Azimute do trilho, graus.
    pub rail_azimuth_deg: f64,
}

/// Navio com trilho no convés.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ship {
    /// Velocidade do navio, m/s.
    pub speed: f64,
    /// Rumo, graus.
    pub heading_deg: f64,
    /// Amplitude de arfagem (pitch), graus.
    pub pitch_amplitude_deg: f64,
    /// Amplitude de balanço (roll), graus.
    pub roll_amplitude_deg: f64,
    /// Período das ondas, s.
    pub wave_period: f64,
    /// Instante do disparo dentro do ciclo das ondas, s.
    pub launch_phase: f64,
    /// Altura do convés, m.
    pub deck_height: f64,
    /// Trilho no convés.
    pub rail_length: f64,
    /// Elevação do trilho em relação ao convés, graus.
    pub rail_elevation_deg: f64,
}

/// Submarino: ejeção a frio do tubo, subida na água, ignição na superfície.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Submarine {
    /// Profundidade do tubo, m.
    pub depth: f64,
    /// Velocidade de saída do tubo (ejeção por gás), m/s.
    pub ejection_speed: f64,
    /// Cd do foguete na água.
    pub water_cd: f64,
    /// Atraso de ignição após romper a superfície, s.
    pub ignition_delay: f64,
}

const RHO_WATER: f64 = 1_025.0;

/// Plataforma giratória.
///
/// Rotação: (I_braço + m·R²)·ω̇ = τ − τ_arrasto, com I_braço = M·R²/3 (barra uniforme de
/// raio R dos dois lados do eixo) e τ_arrasto = ½ρ·(Cd·b)·ω²·R⁴/2 (placa de largura b,
/// pressão dinâmica integrada ao longo dos dois lados). Ao atingir a rotação alvo o motor
/// mantém ω e o braço gira até o ângulo em que a velocidade tangencial tem a elevação
/// pedida. O foguete sai com v = ω·R e **girando** com ω (gira junto com o braço).
/// Cargas: centrípeta ω²R (lateral ao eixo do foguete, montado tangencialmente) e
/// tangencial R·ω̇ (axial).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn spin(s: &Spin, base: Base, rocket: &dyn Payload, _env: &LocalEnv) -> LaunchOutcome {
    let m = rocket.mass(base.t, f64::INFINITY);
    let r = s.radius;
    let i_total = s.arm_mass * r * r / 3.0 + m * r * r;
    let hub = base.pos + Vec3::new(0.0, 0.0, s.hub_height);
    let rho = if s.chamber_pressure <= 0.0 {
        0.0
    } else {
        s.chamber_pressure / (287.05 * 288.15)
    };
    let w_target = s.target_rpm * 2.0 * PI / 60.0;
    let drag_torque = |w: f64| 0.5 * rho * s.arm_cd_width * w * w * r.powi(4) / 2.0;
    // Estado: [φ, ω, trabalho do motor, energia dissipada].
    let rhs = |_: f64, y: &[f64; 4]| {
        let w = y[1];
        let tau = if w < w_target {
            s.torque
        } else {
            drag_torque(w)
        };
        let wdot = if w < w_target {
            (tau - drag_torque(w)) / i_total
        } else {
            0.0
        };
        [w, wdot, tau * w, drag_torque(w) * w]
    };
    let mut out = LaunchOutcome::default();
    if s.torque <= drag_torque(w_target) {
        out.warnings.push(format!(
            "O torque de {:.0} N·m não vence o arrasto do braço na rotação alvo.",
            s.torque
        ));
        return out;
    }
    let g_spun = |_: f64, y: &[f64; 4]| y[1] - w_target;
    let opts = Options {
        rtol: 1e-10,
        atol: 1e-9,
        h_max: 0.5,
        max_steps: 2_000_000,
    };
    let mut samples: Vec<(f64, [f64; 4])> = vec![(base.t, [0.0; 4])];
    let ev = [EventSpec {
        g: &g_spun,
        direction: Direction::Rising,
        terminal: true,
    }];
    let Ok(up) = integrate(
        &rhs,
        base.t,
        [0.0; 4],
        base.t + 7_200.0,
        &ev,
        &opts,
        |st, tv| samples.push((tv, st.dense(tv))),
    ) else {
        out.warnings.push("Falha numérica no giro.".into());
        return out;
    };
    // Gira até o ângulo de liberação: velocidade tangencial com ângulo φ + 90°.
    let target_phi = (s.release_elevation_deg - 90.0)
        .to_radians()
        .rem_euclid(2.0 * PI);
    let phi_now = up.y[0].rem_euclid(2.0 * PI);
    let dphi = (target_phi - phi_now).rem_euclid(2.0 * PI);
    let t_rel = up.t + dphi / w_target;
    let y_rel = [
        up.y[0] + dphi,
        w_target,
        up.y[2] + drag_torque(w_target) * w_target * (t_rel - up.t),
        up.y[3] + drag_torque(w_target) * w_target * (t_rel - up.t),
    ];
    samples.push((t_rel, y_rel));
    let horiz = direction(0.0, s.azimuth_deg.to_radians());
    let up_v = Vec3::Z;
    let pos_at = |phi: f64| hub + (horiz * phi.cos() + up_v * phi.sin()) * r;
    let tang = |phi: f64| -horiz * phi.sin() + up_v * phi.cos();
    // Trajetória da ponta (amostrada) para a animação.
    let tip: Vec<TrackPoint> = samples
        .windows(2)
        .flat_map(|w| {
            let (ta, ya) = w[0];
            let (tb, yb) = w[1];
            let n = (((yb[0] - ya[0]) / 0.3).ceil() as usize).clamp(1, 400);
            (0..n).map(move |k| {
                let f = k as f64 / n as f64;
                let phi = ya[0] + (yb[0] - ya[0]) * f;
                let wv = ya[1] + (yb[1] - ya[1]) * f;
                (ta + (tb - ta) * f, phi, wv)
            })
        })
        .map(|(t, phi, w)| TrackPoint {
            t,
            pos: pos_at(phi),
            vel: tang(phi) * (w * r),
            axis: tang(phi),
        })
        .collect();
    let max_alpha = s.torque / i_total;
    for p in tip.iter().step_by(10) {
        let w = p.vel.norm() / r;
        out.loads.push((
            p.t,
            (r * if w < w_target { max_alpha } else { 0.0 }) / 9.806_65,
            w * w * r / 9.806_65,
        ));
    }
    out.tracks.push(("Foguete no braço".into(), tip));
    let v_rel = tang(target_phi) * (w_target * r);
    let ke_rot = 0.5 * i_total * w_target * w_target;
    let input = y_rel[2];
    let diss = y_rel[3];
    out.energy = Energy {
        input,
        dissipated: diss,
        kinetic: ke_rot,
        potential: 0.0,
        mass_flux: 0.0,
        residual: ((input - diss) - ke_rot).abs() / input.max(1e-9),
        rocket_kinetic: 0.5 * m * v_rel.norm_sq(),
    };
    let g_side = w_target * w_target * r / 9.806_65;
    out.events.push((up.t, "spunUp".into(), format!("Rotação alvo de {:.0} rpm atingida após {:.0} s; carga centrípeta de {:.0} g no foguete.", s.target_rpm, up.t - base.t, g_side)));
    out.events.push((t_rel, "release".into(), format!("Liberação tangencial a {:.0} m/s, {:.0}° de elevação. O foguete sai girando a {:.1} rad/s; o eixo recebe um impulso de {:.0} N·s.", v_rel.norm(), s.release_elevation_deg, w_target, m * v_rel.norm())));
    if g_side > 1_000.0 {
        out.warnings.push(format!("Carga centrípeta de {g_side:.0} g: só eletrônica e estruturas projetadas para isso sobrevivem."));
    }
    // Rotação: o foguete gira com o braço em torno do eixo normal ao plano de giro.
    let normal = horiz.cross(up_v);
    out.release = Some(Release {
        t: t_rel,
        pos: pos_at(target_phi) + base.vel * (t_rel - base.t),
        vel: v_rel + base.vel,
        axis: tang(target_phi),
        omega: normal * w_target,
        ignition: f64::INFINITY,
        ignition_delay: s.ignition_delay,
        mass: m,
    });
    out
}

/// Lançamento aéreo: o foguete sai com o estado do avião (Galileu: soma à base).
#[must_use]
pub fn air_launch(
    a: &AirLaunch,
    base: Base,
    rocket: &dyn Payload,
    env: &LocalEnv,
) -> LaunchOutcome {
    let m = rocket.mass(base.t, f64::INFINITY);
    let dir = direction(a.flight_path_deg.to_radians(), a.heading_deg.to_radians());
    let v_rel = dir * a.speed;
    let pos = base.pos + Vec3::new(0.0, 0.0, a.altitude);
    let t_rel = base.t;
    // Avião: 60 s de aproximação e 30 s de afastamento, em linha reta.
    let plane: Vec<TrackPoint> = (-60..=30)
        .map(|k| {
            let dt = f64::from(k);
            TrackPoint {
                t: t_rel + dt,
                pos: pos + (v_rel + base.vel) * dt + Vec3::new(0.0, 0.0, 15.0),
                vel: v_rel + base.vel,
                axis: dir,
            }
        })
        .filter(|p| p.t >= base.t - 60.0)
        .collect();
    let ke = 0.5 * m * v_rel.norm_sq();
    let pe = m * env.g * a.altitude;
    let mut out = LaunchOutcome::default();
    out.tracks.push(("Avião".into(), plane));
    out.energy = Energy {
        input: ke + pe,
        dissipated: 0.0,
        kinetic: ke,
        potential: pe,
        mass_flux: 0.0,
        residual: 0.0,
        rocket_kinetic: ke,
    };
    out.loads.push((t_rel, 1.0, 0.0));
    // Soltar a carga não muda a velocidade do avião (a carga sai com a mesma velocidade);
    // o avião só perde peso: a sustentação em excesso m·g o acelera para cima.
    out.events.push((t_rel, "release".into(), format!(
        "Liberação do avião a {:.0} m e {:.0} m/s (Mach {:.2}); queda livre de {:.1} s até a ignição. O avião, {:.1}% mais leve, sobe com {:.2} m/s² até retomar o equilíbrio.",
        a.altitude, a.speed, a.speed / env.atm.at(env.ground + a.altitude).sound_speed.max(1.0), a.drop_time,
        100.0 * m / a.aircraft_mass.max(1.0), m * env.g / a.aircraft_mass.max(1.0)
    )));
    out.release = Some(Release {
        t: t_rel,
        pos,
        vel: base.vel + v_rel,
        axis: dir,
        omega: Vec3::ZERO,
        ignition: f64::INFINITY,
        ignition_delay: a.drop_time,
        mass: m,
    });
    out
}

/// Balão de pressão zero: o gás fica na pressão e temperatura do ar, V = m_g·R·T/(M·p);
/// empuxo de Arquimedes ρ_ar·V·g; arrasto ½ρ·Cd·A·|v − w|(v − w), com o vento local w.
/// Integra até a altura de liberação ou até o diâmetro passar do de estouro.
/// Fonte: Yajima et al., "Scientific Ballooning" (Springer, 2009), cap. 2.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn balloon(b: &Balloon, base: Base, rocket: &dyn Payload, env: &LocalEnv) -> LaunchOutcome {
    let m_r = rocket.mass(base.t, f64::INFINITY);
    let m_tot = m_r + b.envelope_mass + b.gas_mass + b.gondola_mass;
    let r_gas = 8_314.462_618 / b.gas_molar_mass;
    let volume = |z: f64| {
        let a = env.atm.at(env.ground + z);
        b.gas_mass * r_gas * a.temperature / a.pressure
    };
    let diam = |z: f64| (6.0 * volume(z) / PI).cbrt();
    // Estado: [x, y, z, vx, vy, vz, trabalho do empuxo, dissipação].
    let rhs = |_: f64, y: &[f64; 8]| {
        let p = Vec3::new(y[0], y[1], y[2]);
        let v = Vec3::new(y[3], y[4], y[5]);
        let z = p.z;
        let rho = env.rho(z);
        let buoy = rho * volume(z) * env.g;
        let vr = v - env.wind_at(z);
        let area = 0.25 * PI * diam(z).powi(2);
        let drag = -0.5 * rho * b.cd * area * vr.norm() * vr;
        // Massa aparente (ar deslocado acelerado junto) desprezada.
        let a = Vec3::new(0.0, 0.0, buoy / m_tot - env.g) + drag / m_tot;
        [v.x, v.y, v.z, a.x, a.y, a.z, buoy * v.z, -drag.dot(v)]
    };
    let rel = b.release_altitude;
    let burst = b.burst_diameter;
    let g_alt = move |_: f64, y: &[f64; 8]| y[2] - rel;
    let g_burst = |_: f64, y: &[f64; 8]| diam(y[2]) - burst;
    let g_fall = |_: f64, y: &[f64; 8]| y[5] + 0.01;
    let ev = [
        EventSpec {
            g: &g_alt,
            direction: Direction::Rising,
            terminal: true,
        },
        EventSpec {
            g: &g_burst,
            direction: Direction::Rising,
            terminal: true,
        },
        EventSpec {
            g: &g_fall,
            direction: Direction::Falling,
            terminal: true,
        },
    ];
    let mut out = LaunchOutcome::default();
    let lift0 = env.rho(base.pos.z) * volume(base.pos.z) * env.g - m_tot * env.g;
    if lift0 <= 0.0 {
        out.warnings.push(format!(
            "O balão não sobe: falta {:.1} kg de força de sustentação. Ponha mais gás.",
            -lift0 / env.g
        ));
        return out;
    }
    let mut pts = vec![(
        base.t,
        [
            base.pos.x, base.pos.y, base.pos.z, base.vel.x, base.vel.y, base.vel.z, 0.0, 0.0,
        ],
    )];
    let opts = Options {
        rtol: 1e-9,
        atol: 1e-6,
        h_max: 20.0,
        max_steps: 1_000_000,
    };
    let Ok(res) = integrate(
        &rhs,
        base.t,
        pts[0].1,
        base.t + 6.0 * 3600.0,
        &ev,
        &opts,
        |st, tv| pts.push((tv, st.dense(tv))),
    ) else {
        out.warnings
            .push("Falha numérica na subida do balão.".into());
        return out;
    };
    match res.terminated_by {
        Some(0) => {}
        Some(1) => {
            out.warnings.push(format!(
                "O balão estourou a {:.0} m (diâmetro de {:.1} m), antes da altura de liberação.",
                res.y[2], burst
            ));
            return out;
        }
        _ => {
            out.warnings.push(format!(
                "O balão parou de subir a {:.0} m (equilíbrio), abaixo da altura de liberação.",
                res.y[2]
            ));
            return out;
        }
    }
    let y = res.y;
    let p = Vec3::new(y[0], y[1], y[2]);
    let v = Vec3::new(y[3], y[4], y[5]);
    let ke = 0.5 * m_tot * (v.norm_sq() - base.vel.norm_sq());
    let pe = m_tot * env.g * (p.z - base.pos.z);
    out.energy = Energy {
        input: y[6],
        dissipated: y[7],
        kinetic: ke,
        potential: pe,
        mass_flux: 0.0,
        residual: ((y[6] - y[7]) - (ke + pe)).abs() / y[6].abs().max(1e-9),
        rocket_kinetic: 0.5 * m_r * v.norm_sq(),
    };
    out.tracks.push((
        "Balão".into(),
        pts.iter()
            .map(|(t, y)| TrackPoint {
                t: *t,
                pos: Vec3::new(y[0], y[1], y[2] + diam(y[2])),
                vel: Vec3::new(y[3], y[4], y[5]),
                axis: Vec3::Z,
            })
            .collect(),
    ));
    out.loads.push((res.t, 1.0, 0.0));
    let drift = Vec3::new(p.x - base.pos.x, p.y - base.pos.y, 0.0).norm();
    out.events.push((
        res.t,
        "release".into(),
        format!(
            "Liberação a {:.0} m após {:.0} min de subida; deriva de {:.1} km com o vento.",
            p.z,
            (res.t - base.t) / 60.0,
            drift / 1000.0
        ),
    ));
    out.release = Some(Release {
        t: res.t,
        pos: p,
        vel: v,
        axis: direction(
            b.rail_elevation_deg.to_radians(),
            b.rail_azimuth_deg.to_radians(),
        ),
        omega: Vec3::ZERO,
        ignition: f64::INFINITY,
        ignition_delay: 0.0,
        mass: m_r,
    });
    out
}

/// Navio: trilho no convés que arfa e balança com as ondas (senoides); o foguete percorre
/// o trilho no referencial do navio (aceleração do convés desprezada) e sai com a
/// velocidade do navio somada e a rotação do convés.
#[must_use]
pub fn ship(s: &Ship, base: Base, rocket: &dyn Payload, env: &LocalEnv) -> LaunchOutcome {
    let w = 2.0 * PI / s.wave_period.max(0.1);
    let ph = w * s.launch_phase;
    let pitch = s.pitch_amplitude_deg.to_radians() * ph.sin();
    let roll = s.roll_amplitude_deg.to_radians() * ph.cos();
    let q_rate = s.pitch_amplitude_deg.to_radians() * w * ph.cos();
    let p_rate = -s.roll_amplitude_deg.to_radians() * w * ph.sin();
    let heading = s.heading_deg.to_radians();
    // Trilho apontando para a proa, elevado; arfagem soma à elevação, balanço inclina.
    let elev = s.rail_elevation_deg.to_radians() + pitch;
    let mut g = Guide {
        label: "Trilho no convés".into(),
        length: s.rail_length,
        elevation_deg: elev.to_degrees(),
        azimuth_deg: (heading + roll * elev.cos().max(0.05).recip() * 0.0).to_degrees(),
        carriage_mass: 0.0,
        friction: 0.05,
        carriage_cd_a: 0.0,
        push: Push::None,
        release_speed: None,
        ignite_at: Some(0.0),
        ignition_delay: 0.0,
        brake: 50.0,
        height: s.deck_height,
    };
    // Balanço tomba o trilho de lado: rotação do eixo em torno da proa.
    let fwd = direction(0.0, heading);
    let deck_base = Base {
        t: base.t,
        pos: base.pos,
        vel: base.vel + fwd * s.speed,
    };
    g.azimuth_deg = heading.to_degrees();
    let mut out = guided(&g, deck_base, rocket, env);
    if let Some(r) = out.release.as_mut() {
        let axis = r.axis;
        // Gira o eixo do roll em torno da proa (Rodrigues).
        let k = fwd;
        r.axis =
            axis * roll.cos() + k.cross(axis) * roll.sin() + k * k.dot(axis) * (1.0 - roll.cos());
        let side = Vec3::Z.cross(fwd);
        r.omega = side * (-q_rate) + fwd * p_rate;
    }
    out.events.push((
        base.t,
        "ship".into(),
        format!(
            "Disparo do navio a {:.1} nós com arfagem {:.1}° e balanço {:.1}° no instante.",
            s.speed / 0.514_444,
            pitch.to_degrees(),
            roll.to_degrees()
        ),
    ));
    out
}

/// Submarino: subida na água com empuxo hidrostático e arrasto, a partir da velocidade de
/// ejeção; ignição depois de romper a superfície.
#[must_use]
pub fn submarine(s: &Submarine, base: Base, rocket: &dyn Payload, env: &LocalEnv) -> LaunchOutcome {
    let m = rocket.mass(base.t, f64::INFINITY);
    let d = rocket.diameter();
    let vol = 0.25 * PI * d * d * rocket.length() * 0.85;
    let area = 0.25 * PI * d * d;
    // Massa adicional de água (cilindro deslocando-se axialmente, desprezível) ignorada.
    let rhs = |_: f64, y: &[f64; 4]| {
        let v = y[1];
        let buoy = RHO_WATER * vol * env.g;
        let drag = 0.5 * RHO_WATER * s.water_cd * area * v * v.abs();
        [v, (buoy - m * env.g - drag) / m, buoy * v, drag * v]
    };
    let depth = s.depth;
    let g_surf = move |_: f64, y: &[f64; 4]| y[0] - depth;
    let ev = [EventSpec {
        g: &g_surf,
        direction: Direction::Rising,
        terminal: true,
    }];
    let mut pts = vec![(base.t, [0.0, s.ejection_speed, 0.0, 0.0])];
    let opts = Options {
        rtol: 1e-10,
        atol: 1e-9,
        h_max: 0.05,
        max_steps: 1_000_000,
    };
    let mut out = LaunchOutcome::default();
    let Ok(res) = integrate(
        &rhs,
        base.t,
        pts[0].1,
        base.t + 300.0,
        &ev,
        &opts,
        |st, tv| pts.push((tv, st.dense(tv))),
    ) else {
        out.warnings
            .push("Falha numérica na subida submersa.".into());
        return out;
    };
    if res.terminated_by.is_none() || res.y[1] <= 0.0 {
        out.warnings
            .push("O foguete não chegou à superfície: aumente a velocidade de ejeção.".into());
        return out;
    }
    let v = res.y[1];
    let ke = 0.5 * m * (v * v - s.ejection_speed * s.ejection_speed);
    let pe = m * env.g * depth;
    out.energy = Energy {
        input: res.y[2] + 0.5 * m * s.ejection_speed * s.ejection_speed,
        dissipated: res.y[3],
        kinetic: 0.5 * m * v * v,
        potential: pe,
        mass_flux: 0.0,
        residual: ((res.y[2] - res.y[3]) - (ke + pe)).abs() / res.y[2].abs().max(1e-9),
        rocket_kinetic: 0.5 * m * v * v,
    };
    out.tracks.push((
        "Foguete submerso".into(),
        pts.iter()
            .map(|(t, y)| TrackPoint {
                t: *t,
                pos: base.pos + Vec3::new(0.0, 0.0, y[0] - depth),
                vel: Vec3::new(0.0, 0.0, y[1]),
                axis: Vec3::Z,
            })
            .collect(),
    ));
    out.events.push((
        res.t,
        "release".into(),
        format!(
            "Rompe a superfície a {:.1} m/s depois de {:.1} s na água.",
            v,
            res.t - base.t
        ),
    ));
    out.release = Some(Release {
        t: res.t,
        pos: base.pos,
        vel: base.vel + Vec3::new(0.0, 0.0, v),
        axis: Vec3::Z,
        omega: Vec3::ZERO,
        ignition: f64::INFINITY,
        ignition_delay: s.ignition_delay,
        mass: m,
    });
    out
}
