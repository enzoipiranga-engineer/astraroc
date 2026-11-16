//! Simulação de missão 3-DOF: subida com guiagem, separações, órbita.

use crate::orbit::{elements, OrbitElements};
use crate::vehicle::{Mission, Stage};
use astra_dynamics::flight::{
    simulate3, Config3, ConstantVehicle, EventKind, Launch, Rail, Start3,
};
use astra_dynamics::ode::{integrate, Direction, EventSpec, Options};
use astra_env::{rot_z, AtmosphereModel, Geodetic, Planet, Wind};
use astra_math::Vec3;
use serde::Serialize;

/// Trajetória de um corpo (mesmo formato do simulador de foguetes montados).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// Índice.
    pub id: usize,
    /// Nome.
    pub name: String,
    /// Peças (vazio aqui).
    pub parts: Vec<String>,
    /// Modelos usados.
    pub models: Vec<String>,
    /// Corpo de origem.
    pub parent: Option<usize>,
    /// Amostras (16 colunas; ver `astra-design::BodyTrack`).
    pub samples: Vec<[f64; 16]>,
    /// Destino.
    pub fate: String,
    /// Pouso: [t, velocidade, norte, leste].
    pub landing: Option<[f64; 4]>,
}

/// Acontecimento.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionEvent {
    /// Instante, s.
    pub t: f64,
    /// Corpo.
    pub body: usize,
    /// Código.
    pub kind: String,
    /// Descrição.
    pub message: String,
    /// Altura acima do solo, m.
    pub alt: f64,
    /// Velocidade em relação ao ar, m/s.
    pub speed: f64,
}

/// Orçamento de Δv da subida (referencial inercial), m/s.
///
/// Da equação d|v|/dt = (T/m)·cos θ + (D⃗·v̂)/m + g⃗·v̂:
/// ∫T/m dt = Δ|v| + perda por direção + perda por arrasto + perda gravitacional.
#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DvBudget {
    /// Tsiolkovsky com Isp de vácuo, sem perdas.
    pub ideal: f64,
    /// ∫T/m dt efetivamente entregue.
    pub delivered: f64,
    /// Perda por contrapressão atmosférica no bocal: ∫p_a·A_e/m dt.
    pub pressure_loss: f64,
    /// −∫g⃗·v̂ dt.
    pub gravity_loss: f64,
    /// −∫(D⃗·v̂)/m dt.
    pub drag_loss: f64,
    /// ∫(T/m)(1 − cos θ) dt.
    pub steering_loss: f64,
    /// Ganho de velocidade inercial |v_f| − |v_0|.
    pub achieved: f64,
    /// Velocidade inicial dada pela rotação da Terra.
    pub earth_rotation: f64,
}

/// Resultado da missão.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionOutput {
    /// Corpos.
    pub bodies: Vec<Track>,
    /// Acontecimentos.
    pub events: Vec<MissionEvent>,
    /// Avisos.
    pub warnings: Vec<String>,
    /// Órbita no fim da queima (ou trajetória suborbital).
    pub orbit: Option<OrbitElements>,
    /// Atingiu a órbita alvo (perigeu acima de 120 km).
    pub reached_orbit: bool,
    /// Δv.
    pub dv: DvBudget,
    /// Parâmetros da guiagem de tangente linear (tan θ = A − B·τ) e se convergiu.
    pub guidance: Option<(f64, f64, bool)>,
    /// Massa na decolagem, kg.
    pub liftoff_mass: f64,
    /// Pressão dinâmica máxima, Pa, e instante.
    pub max_q: (f64, f64),
    /// Aceleração máxima (empuxo + arrasto)/m, em g.
    pub max_accel_g: f64,
    /// Propelente que sobrou no último estágio ao atingir a órbita, kg.
    pub propellant_margin: f64,
    /// Ângulo da manobra inicial usado, graus.
    pub kick_deg: f64,
}

#[derive(Clone, Copy, Debug)]
enum Steer {
    Vertical,
    Kick {
        t0: f64,
        dur: f64,
        angle: f64,
    },
    GravityTurn,
    Linear {
        t0: f64,
        a: f64,
        b: f64,
        normal: Vec3,
    },
    Coast,
}

/// Contexto físico de um trecho.
struct Ctx<'a> {
    planet: Planet,
    atm: AtmosphereModel,
    wind: &'a Wind,
    theta0: f64,
    ground: f64,
    azimuth: f64,
    site: Geodetic,
    /// Massa sem o propelente do estágio ativo e dos boosters, kg.
    fixed: f64,
    stage: Option<(&'a Stage, u32)>,
    booster: Option<(&'a Stage, u32)>,
    steer: Steer,
    area: f64,
    cd: &'a [(f64, f64)],
}

/// Linhas de uma queda livre e, se tocou o solo, (t, velocidade, norte, leste).
type Coasted = (Vec<[f64; 16]>, Option<(f64, f64, f64, f64)>);

struct Forces {
    thrust: Vec3,
    drag: Vec3,
    gravity: Vec3,
    mdot_core: f64,
    mdot_boost: f64,
    pressure_loss: f64,
    q: f64,
    mach: f64,
    v_rel: Vec3,
}

fn interp(t: &[(f64, f64)], x: f64) -> f64 {
    match t {
        [] => 0.3,
        [o] => o.1,
        _ => {
            let i = t.partition_point(|p| p.0 <= x).clamp(1, t.len() - 1);
            let (a, b) = (t[i - 1], t[i]);
            a.1 + (b.1 - a.1) * ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
        }
    }
}

impl Ctx<'_> {
    fn theta(&self, t: f64) -> f64 {
        self.theta0 + self.planet.omega * t
    }

    fn forces(&self, t: f64, y: &[f64; 8]) -> Forces {
        let r = Vec3::new(y[0], y[1], y[2]);
        let v = Vec3::new(y[3], y[4], y[5]);
        let m = self.fixed + y[6].max(0.0) + y[7].max(0.0);
        let geo = self.planet.ecef_to_geodetic(r);
        let atmo = self.atm.at(geo.h);
        let (e, n, u) = Planet::enu_basis(geo.lat, geo.lon);
        let w = if self.wind.is_calm() {
            Vec3::ZERO
        } else {
            let ww = self.wind.at(geo.h - self.ground, rot_z(r, -self.theta(t)));
            e * ww.x + n * ww.y + u * ww.z
        };
        let v_rel = v - self.planet.omega_vec().cross(r) - w;
        let speed = v_rel.norm();
        let mach = if atmo.sound_speed > 0.0 {
            speed / atmo.sound_speed
        } else {
            0.0
        };
        let q = 0.5 * atmo.density * speed * speed;
        let drag = if speed > 0.0 {
            -0.5 * atmo.density * speed * interp(self.cd, mach) * self.area * v_rel
        } else {
            Vec3::ZERO
        };

        // Direção do empuxo.
        let site_up = |t: f64| {
            let (se, sn, su) = Planet::enu_basis(self.site.lat, self.site.lon + self.theta(t));
            (se, sn, su)
        };
        let dir = match self.steer {
            Steer::Vertical | Steer::Coast => u,
            Steer::Kick { t0, dur, angle } => {
                let k = angle * ((t - t0) / dur).clamp(0.0, 1.0);
                let (_, _, _) = site_up(t);
                let horiz = e * self.azimuth.sin() + n * self.azimuth.cos();
                u * k.cos() + horiz * k.sin()
            }
            Steer::GravityTurn => {
                if speed > 1.0 {
                    v_rel / speed
                } else {
                    u
                }
            }
            Steer::Linear { t0, a, b, normal } => {
                let rh = r / r.norm();
                let horiz = normal.cross(rh);
                let th = (a - b * (t - t0)).atan();
                horiz * th.cos() + rh * th.sin()
            }
        };

        // Empuxo, com limites suaves de pressão dinâmica e de aceleração.
        let full = |s: &Stage, out: u32, p: f64, thr: f64| -> (f64, f64, f64) {
            let mut f = 0.0;
            let mut md = 0.0;
            let mut ploss = 0.0;
            for (k, (eng, cnt)) in s.engines.iter().enumerate() {
                let c = f64::from(if k == 0 {
                    cnt.saturating_sub(out)
                } else {
                    *cnt
                });
                f += c * eng.thrust(thr, p);
                md += c * eng.mdot() * thr;
                ploss += c * p * eng.exit_area;
            }
            (f, md, ploss)
        };
        let (mut thrust, mut mdc, mut mdb, mut ploss) = (0.0, 0.0, 0.0, 0.0);
        if !matches!(self.steer, Steer::Coast) {
            if let Some((s, out)) = self.stage.filter(|_| y[6] > 0.0) {
                let mut thr = 1.0_f64;
                if let Some(qm) = s.max_q {
                    thr = thr.min(1.0 - 3.0 * ((q - qm) / qm).max(0.0));
                }
                if let Some(am) = s.max_accel {
                    let (f1, _, _) = full(s, out, atmo.pressure, 1.0);
                    if f1 > 0.0 {
                        thr = thr.min(
                            (am * m
                                + atmo.pressure
                                    * s.engines
                                        .iter()
                                        .map(|(e, c)| e.exit_area * f64::from(*c))
                                        .sum::<f64>())
                                / (f1
                                    + atmo.pressure
                                        * s.engines
                                            .iter()
                                            .map(|(e, c)| e.exit_area * f64::from(*c))
                                            .sum::<f64>()),
                        );
                    }
                }
                let thr = thr.clamp(s.throttle_min(), 1.0);
                let (f, md, pl) = full(s, out, atmo.pressure, thr);
                thrust += f;
                mdc = md;
                ploss += pl;
            }
            if let Some((b, count)) = self.booster.filter(|_| y[7] > 0.0) {
                let (f, md, pl) = full(b, 0, atmo.pressure, 1.0);
                thrust += f64::from(count) * f;
                mdb = f64::from(count) * md;
                ploss += f64::from(count) * pl;
            }
        }
        Forces {
            thrust: dir * thrust,
            drag,
            gravity: self.planet.gravity(r),
            mdot_core: mdc,
            mdot_boost: mdb,
            pressure_loss: ploss / m,
            q,
            mach,
            v_rel,
        }
    }

    fn rhs(&self, t: f64, y: &[f64; 8]) -> [f64; 8] {
        let m = self.fixed + y[6].max(0.0) + y[7].max(0.0);
        let f = self.forces(t, y);
        let a = f.gravity + (f.thrust + f.drag) / m;
        [y[3], y[4], y[5], a.x, a.y, a.z, -f.mdot_core, -f.mdot_boost]
    }
}

const OUT_DT: f64 = 0.5;

/// Integra um trecho até `t_end` ou até um dos eventos terminais.
#[allow(clippy::too_many_arguments)]
fn segment(
    ctx: &Ctx<'_>,
    t0: f64,
    y0: [f64; 8],
    t_end: f64,
    target_energy: Option<f64>,
    fairing_alt: Option<f64>,
    rows: &mut Vec<(f64, [f64; 8])>,
) -> (f64, [f64; 8], Option<&'static str>) {
    let f = |t: f64, y: &[f64; 8]| ctx.rhs(t, y);
    let g_core = |_: f64, y: &[f64; 8]| y[6];
    let g_boost = |_: f64, y: &[f64; 8]| y[7];
    let g_ground = |_: f64, y: &[f64; 8]| {
        ctx.planet.ecef_to_geodetic(Vec3::new(y[0], y[1], y[2])).h - ctx.ground
    };
    let mu = ctx.planet.mu;
    let g_cut = |_: f64, y: &[f64; 8]| {
        let r = Vec3::new(y[0], y[1], y[2]).norm();
        let v = Vec3::new(y[3], y[4], y[5]).norm();
        0.5 * v * v - mu / r - target_energy.unwrap_or(f64::INFINITY)
    };
    let g_fair = |_: f64, y: &[f64; 8]| {
        ctx.planet.ecef_to_geodetic(Vec3::new(y[0], y[1], y[2])).h
            - fairing_alt.unwrap_or(f64::INFINITY)
    };
    let mut ev: Vec<(EventSpec<'_, 8>, &'static str)> = Vec::new();
    if ctx.stage.is_some() && y0[6] > 0.0 && !matches!(ctx.steer, Steer::Coast) {
        ev.push((
            EventSpec {
                g: &g_core,
                direction: Direction::Falling,
                terminal: true,
            },
            "core",
        ));
    }
    if ctx.booster.is_some() && y0[7] > 0.0 && !matches!(ctx.steer, Steer::Coast) {
        ev.push((
            EventSpec {
                g: &g_boost,
                direction: Direction::Falling,
                terminal: true,
            },
            "booster",
        ));
    }
    ev.push((
        EventSpec {
            g: &g_ground,
            direction: Direction::Falling,
            terminal: true,
        },
        "impact",
    ));
    if target_energy.is_some() {
        ev.push((
            EventSpec {
                g: &g_cut,
                direction: Direction::Rising,
                terminal: true,
            },
            "cutoff",
        ));
    }
    if fairing_alt.is_some() {
        ev.push((
            EventSpec {
                g: &g_fair,
                direction: Direction::Rising,
                terminal: true,
            },
            "fairing",
        ));
    }
    let specs: Vec<EventSpec<'_, 8>> = ev
        .iter()
        .map(|(e, _)| EventSpec {
            g: e.g,
            direction: e.direction,
            terminal: e.terminal,
        })
        .collect();
    let opts = Options {
        rtol: 1e-8,
        atol: 1e-6,
        h_max: 2.0,
        max_steps: 400_000,
    };
    let mut k = (t0 / OUT_DT).floor() as u64 + 1;
    let res = integrate(&f, t0, y0, t_end, &specs, &opts, |step, tv| {
        while (k as f64) * OUT_DT <= tv {
            let tt = k as f64 * OUT_DT;
            rows.push((tt, step.dense(tt)));
            k += 1;
        }
    });
    match res {
        Ok(o) => (o.t, o.y, o.terminated_by.map(|i| ev[i].1)),
        Err(_) => (t0, y0, Some("error")),
    }
}

/// Monta uma linha de telemetria (mesmo formato de `astra-design`).
fn row(ctx: &Ctx<'_>, t: f64, y: &[f64; 8], site: &Geodetic) -> [f64; 16] {
    let r = Vec3::new(y[0], y[1], y[2]);
    let v = Vec3::new(y[3], y[4], y[5]);
    let f = ctx.forces(t, y);
    let ecef = rot_z(r, -ctx.theta(t));
    let g = ctx.planet.ecef_to_geodetic(ecef);
    let (_, _, up) = Planet::enu_basis(g.lat, g.lon + ctx.theta(t));
    let v_ground = v - ctx.planet.omega_vec().cross(r);
    let north = (g.lat - site.lat) * ctx.planet.re;
    let mut dlon = g.lon - site.lon;
    if dlon > core::f64::consts::PI {
        dlon -= 2.0 * core::f64::consts::PI;
    } else if dlon < -core::f64::consts::PI {
        dlon += 2.0 * core::f64::consts::PI;
    }
    let east = dlon * ctx.planet.re * site.lat.cos();
    let axis = if f.thrust.norm() > 0.0 {
        f.thrust / f.thrust.norm()
    } else {
        f.v_rel / f.v_rel.norm().max(1e-9)
    };
    let ax = rot_z(axis, -ctx.theta(t));
    let m = ctx.fixed + y[6].max(0.0) + y[7].max(0.0);
    [
        t,
        g.h - ctx.ground,
        f.v_rel.norm(),
        v_ground.dot(up),
        f.mach,
        f.q,
        north,
        east,
        f64::NAN,
        m,
        g.lat.to_degrees(),
        g.lon.to_degrees(),
        g.h,
        ax.x,
        ax.y,
        ax.z,
    ]
}

fn flight_path_angle(y: &[f64; 8]) -> f64 {
    let r = Vec3::new(y[0], y[1], y[2]);
    let v = Vec3::new(y[3], y[4], y[5]);
    (r.dot(v) / (r.norm() * v.norm())).clamp(-1.0, 1.0).asin()
}

struct Ascent {
    rows: Vec<(f64, [f64; 8], usize, usize)>,
    events: Vec<(f64, String, String, [f64; 8], usize, usize)>,
    drops: Vec<(String, f64, [f64; 8], f64, f64, f64)>,
    end: (f64, [f64; 8], usize),
    outcome: &'static str,
    upper_start: Option<(f64, [f64; 8])>,
}

#[allow(clippy::too_many_arguments)]
fn snap(out: &mut Ascent, t: f64, si: usize, ph: u8, kind: &str, msg: String, y: [f64; 8]) {
    out.events
        .push((t, kind.to_owned(), msg, y, si, ph as usize));
}

/// Simula a subida completa com os parâmetros de guiagem dados (A, B).
#[allow(clippy::too_many_lines)]
fn ascent(m: &Mission, base: &CtxBase, ab: Option<(f64, f64)>, from: Option<&Ascent>) -> Ascent {
    let veh = &m.vehicle;
    let n_st = veh.stages.len();
    let mut out = Ascent {
        rows: Vec::new(),
        events: Vec::new(),
        drops: Vec::new(),
        end: (0.0, [0.0; 8], 0),
        outcome: "",
        upper_start: None,
    };
    // Massa fixa inicial.
    let fairing = veh.fairing.map_or(0.0, |f| f.mass);
    let booster_dry = veh.boosters.as_ref().map_or(0.0, |b| {
        f64::from(b.count) * (b.stage.dry_mass + b.stage.reserve)
    });
    let upper_wet: f64 = veh.stages.iter().skip(1).map(Stage::wet_mass).sum();
    let s0 = &veh.stages[0];
    let mut fixed = veh.payload + fairing + booster_dry + upper_wet + s0.dry_mass + s0.reserve;
    let mut fairing_on = veh.fairing.is_some();
    let (mut t, mut y, mut si, mut booster_on, mut steer_phase): (f64, [f64; 8], usize, bool, u8);
    if let (Some(prev), Some((ts, ys))) = (from, from.and_then(|p| p.upper_start)) {
        // Reaproveita a subida do primeiro estágio (não depende da guiagem).
        out.rows = prev.rows.iter().filter(|r| r.0 <= ts).cloned().collect();
        out.events = prev.events.iter().filter(|e| e.0 <= ts).cloned().collect();
        out.drops = prev.drops.iter().filter(|d| d.1 <= ts).cloned().collect();
        out.upper_start = prev.upper_start;
        t = ts;
        y = ys;
        si = 1;
        booster_on = false;
        steer_phase = 3;
        fixed = veh.payload + upper_wet - veh.stages[1].usable();
        // Carenagem já alijada?
        fairing_on =
            veh.fairing.is_some() && !prev.events.iter().any(|e| e.1 == "fairing" && e.0 <= ts);
        if fairing_on {
            fixed += fairing;
        }
    } else {
        let site = base.site;
        let r0 = base.planet.geodetic_to_ecef(site);
        let r0 = rot_z(r0, base.theta0);
        let v0 = base.planet.omega_vec().cross(r0);
        y = [
            r0.x,
            r0.y,
            r0.z,
            v0.x,
            v0.y,
            v0.z,
            s0.usable(),
            veh.boosters
                .as_ref()
                .map_or(0.0, |b| f64::from(b.count) * b.stage.usable()),
        ];
        t = 0.0;
        si = 0;
        booster_on = veh.boosters.is_some();
        steer_phase = 0;
    }
    let mut stage_t0 = t;
    let mut engines_out = 0u32;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 60 {
            out.outcome = "error";
            break;
        }
        let stage = &veh.stages[si];
        let steer = match steer_phase {
            0 => Steer::Vertical,
            1 => Steer::Kick {
                t0: m.kick.start,
                dur: m.kick.duration,
                angle: m.kick.angle_deg.to_radians(),
            },
            2 => Steer::GravityTurn,
            4 => Steer::Coast,
            _ => {
                let (a, b) = ab.unwrap_or((0.3, 0.001));
                let r = Vec3::new(y[0], y[1], y[2]);
                let v = Vec3::new(y[3], y[4], y[5]);
                let normal = out.upper_start.map_or(r.cross(v), |(_, ys)| {
                    Vec3::new(ys[0], ys[1], ys[2]).cross(Vec3::new(ys[3], ys[4], ys[5]))
                });
                let normal = normal / normal.norm();
                Steer::Linear {
                    t0: out.upper_start.map_or(t, |u| u.0),
                    a,
                    b,
                    normal,
                }
            }
        };
        let booster = veh
            .boosters
            .as_ref()
            .filter(|_| booster_on)
            .map(|b| (&b.stage, b.count));
        let ctx = base.ctx(fixed, Some((stage, engines_out)), booster, steer);
        let mut t_end = 4.0 * 3600.0;
        if steer_phase == 0 {
            t_end = m.kick.start;
        } else if steer_phase == 1 {
            t_end = m.kick.start + m.kick.duration;
        } else if steer_phase == 4 {
            t_end = t + m.coast;
        }
        if let Some(eo) = stage.engine_out {
            if engines_out == 0 && stage_t0 + eo.after > t + 1e-9 {
                t_end = t_end.min(stage_t0 + eo.after);
            }
        }
        let last = si + 1 == n_st;
        let target = (last && steer_phase == 3)
            .then(|| -base.planet.mu / (2.0 * (base.planet.re + m.target_altitude)));
        let fair_alt =
            fairing_on.then(|| veh.fairing.map_or(f64::INFINITY, |f| f.jettison_altitude));
        let mut seg_rows = Vec::new();
        let (t1, y1, why) = segment(&ctx, t, y, t_end, target, fair_alt, &mut seg_rows);
        out.rows.extend(
            seg_rows
                .into_iter()
                .map(|(tt, yy)| (tt, yy, si, steer_phase as usize)),
        );
        t = t1;
        y = y1;
        match why {
            Some("impact") => {
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "impact",
                    "O veículo caiu antes de chegar à órbita.".into(),
                    y,
                );
                out.outcome = "impact";
                break;
            }
            Some("error") => {
                out.outcome = "error";
                break;
            }
            Some("cutoff") => {
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "seco",
                    format!("Corte do motor do {}: órbita alvo atingida.", stage.name),
                    y,
                );
                out.outcome = "cutoff";
                break;
            }
            Some("fairing") => {
                fairing_on = false;
                fixed -= veh.fairing.map_or(0.0, |f| f.mass);
                out.drops.push((
                    "Carenagem".into(),
                    t,
                    y,
                    veh.fairing.map_or(0.0, |f| f.mass),
                    5.0,
                    6.0,
                ));
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "fairing",
                    "Alijamento da carenagem.".into(),
                    y,
                );
                continue;
            }
            Some("booster") => {
                let b = veh.boosters.as_ref().map(|b| (b.stage.clone(), b.count));
                if let Some((bs, count)) = b {
                    booster_on = false;
                    y[7] = 0.0;
                    fixed -= f64::from(count) * (bs.dry_mass + bs.reserve);
                    for k in 0..count {
                        out.drops.push((
                            format!("{} {}", bs.name, k + 1),
                            t,
                            y,
                            bs.dry_mass + bs.reserve,
                            bs.diameter,
                            bs.length,
                        ));
                    }
                    snap(
                        &mut out,
                        t,
                        si,
                        steer_phase,
                        "boosterSep",
                        format!("Separação de {} {}.", count, bs.name),
                        y,
                    );
                }
                continue;
            }
            Some("core") => {
                y[6] = 0.0;
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "burnout",
                    format!("Fim da queima do {}.", stage.name),
                    y,
                );
                if last {
                    out.outcome = "depleted";
                    break;
                }
                // Separação.
                fixed -= stage.dry_mass + stage.reserve;
                out.drops.push((
                    stage.name.clone(),
                    t,
                    y,
                    stage.dry_mass + stage.reserve,
                    stage.diameter,
                    stage.length,
                ));
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "stageSep",
                    format!("Separação do {}.", stage.name),
                    y,
                );
                if booster_on {
                    booster_on = false;
                    y[7] = 0.0;
                }
                si += 1;
                engines_out = 0;
                let next = &veh.stages[si];
                fixed -= next.usable();
                y[6] = next.usable();
                if m.coast > 0.0 {
                    steer_phase = 4;
                    continue;
                }
                steer_phase = 3;
                stage_t0 = t;
                if out.upper_start.is_none() {
                    out.upper_start = Some((t, y));
                }
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "ignition",
                    format!(
                        "Ignição do {}{}.",
                        next.name,
                        if m.coast == 0.0 { " (hot staging)" } else { "" }
                    ),
                    y,
                );
                continue;
            }
            _ => {}
        }
        // Fronteiras de tempo.
        if let Some(eo) = stage.engine_out {
            if engines_out == 0 && (t - (stage_t0 + eo.after)).abs() < 1e-6 {
                engines_out = eo.engines;
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "engineOut",
                    format!("Falha de {} motor(es) do {}.", eo.engines, stage.name),
                    y,
                );
                continue;
            }
        }
        match steer_phase {
            0 => steer_phase = 1,
            1 => {
                steer_phase = 2;
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "gravityTurn",
                    "Fim da manobra inicial: início do gravity turn.".into(),
                    y,
                );
            }
            4 => {
                steer_phase = 3;
                stage_t0 = t;
                if out.upper_start.is_none() {
                    out.upper_start = Some((t, y));
                }
                snap(
                    &mut out,
                    t,
                    si,
                    steer_phase,
                    "ignition",
                    format!("Ignição do {}.", stage.name),
                    y,
                );
            }
            _ => {
                out.outcome = "timeout";
                break;
            }
        }
    }
    out.end = (t, y, si);
    out
}

/// Partes do contexto que não mudam entre trechos.
struct CtxBase<'a> {
    planet: Planet,
    atm: AtmosphereModel,
    wind: &'a Wind,
    theta0: f64,
    ground: f64,
    azimuth: f64,
    site: Geodetic,
    area: f64,
    cd: &'a [(f64, f64)],
}

impl<'a> CtxBase<'a> {
    fn ctx(
        &self,
        fixed: f64,
        stage: Option<(&'a Stage, u32)>,
        booster: Option<(&'a Stage, u32)>,
        steer: Steer,
    ) -> Ctx<'a> {
        Ctx {
            planet: self.planet,
            atm: self.atm,
            wind: self.wind,
            theta0: self.theta0,
            ground: self.ground,
            azimuth: self.azimuth,
            site: self.site,
            fixed,
            stage,
            booster,
            steer,
            area: self.area,
            cd: self.cd,
        }
    }
}

/// Simula a missão. Com `auto_kick`, testa ângulos de manobra inicial e fica com o que
/// chega à órbita com mais propelente sobrando (ou, se nenhum chegar, com maior energia
/// orbital no fim).
#[must_use]
pub fn simulate(m: &Mission) -> MissionOutput {
    if !m.auto_kick {
        return simulate_fixed(m);
    }
    let score = |o: &MissionOutput| {
        if o.reached_orbit {
            1e12 + o.propellant_margin
        } else {
            o.orbit
                .map_or(f64::NEG_INFINITY, |e| -Planet::EARTH.mu / (2.0 * e.a))
        }
    };
    let mut best: Option<MissionOutput> = None;
    for k in [0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.5, 8.0, 10.0, 12.0] {
        let mut mm = m.clone();
        mm.kick.angle_deg = k;
        let o = simulate_fixed(&mm);
        if best.as_ref().is_none_or(|b| score(&o) > score(b)) {
            best = Some(o);
        }
    }
    best.unwrap_or_else(|| simulate_fixed(m))
}

/// Simula a missão com a manobra inicial dada: ajusta a guiagem do(s) estágio(s) superior(es) por tiro (Newton com
/// diferenças finitas sobre altitude e ângulo de trajetória no corte), depois produz a
/// trajetória, os estágios descartados até o impacto, a órbita e o orçamento de Δv.
#[must_use]
#[allow(clippy::too_many_lines)]
fn simulate_fixed(m: &Mission) -> MissionOutput {
    let planet = Planet::EARTH;
    let theta0 = m
        .utc
        .as_deref()
        .and_then(|u| astra_env::time::parse_utc(u).ok())
        .map_or(0.0, astra_env::time::gmst);
    let site = Geodetic {
        lat: m.site.0.to_radians(),
        lon: m.site.1.to_radians(),
        h: m.site.2,
    };
    let dmax = m
        .vehicle
        .stages
        .iter()
        .map(|s| s.diameter)
        .fold(0.0, f64::max);
    let base = CtxBase {
        planet,
        atm: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
        wind: &m.wind,
        theta0,
        ground: m.site.2,
        azimuth: m.azimuth_deg.to_radians(),
        site,
        area: 0.25 * core::f64::consts::PI * dmax * dmax,
        cd: &m.vehicle.cd_table,
    };
    let mut warnings = Vec::new();
    let r_t = planet.re + m.target_altitude;

    // Primeira subida (guiagem inicial) e ajuste.
    let first = ascent(m, &base, None, None);
    let residual = |a: &Ascent| -> Option<[f64; 2]> {
        let (_, y, _) = a.end;
        let r = Vec3::new(y[0], y[1], y[2]).norm();
        Some([(r - r_t) / 1e5, flight_path_angle(&y) * 10.0])
    };
    let mut guidance = None;
    let mut best = first;
    if m.vehicle.stages.len() > 1 && best.upper_start.is_some() {
        // Incógnitas: ângulo de arfagem no início (θ0) e previsto no fim (θ1) da queima dos
        // estágios superiores; tan θ = A − B·τ com A = tan θ0 e B = (tan θ0 − tan θ1)/T,
        // T = tempo de queima nominal somado dos estágios superiores.
        let t_burn: f64 = m
            .vehicle
            .stages
            .iter()
            .skip(1)
            .map(|s| s.usable() / s.mdot().max(1e-9))
            .sum::<f64>()
            + m.coast * (m.vehicle.stages.len() as f64 - 2.0).max(0.0);
        let to_ab = |p: [f64; 2]| (p[0].tan(), (p[0].tan() - p[1].tan()) / t_burn.max(1.0));
        let eval = |p: [f64; 2], prev: &Ascent| {
            let (a, b) = to_ab(p);
            let run = ascent(m, &base, Some((a, b)), Some(prev));
            let f = residual(&run).unwrap_or([1e3, 1e3]);
            (run, f)
        };
        let norm = |f: [f64; 2]| f[0].hypot(f[1]);
        let (_, ys) = best.upper_start.unwrap_or((0.0, [0.0; 8]));
        let gamma0 = flight_path_angle(&ys);
        // Busca inicial em grade.
        let mut p = [gamma0, 0.0];
        let mut fbest = [f64::INFINITY; 2];
        for d0 in [-15.0_f64, -5.0, 5.0, 15.0, 30.0] {
            for th1 in [-20.0_f64, -5.0, 10.0] {
                let q = [
                    (gamma0.to_degrees() + d0).clamp(-60.0, 80.0).to_radians(),
                    th1.to_radians(),
                ];
                let (run, f) = eval(q, &best);
                if norm(f) < norm(fbest) {
                    fbest = f;
                    p = q;
                    best = run;
                }
            }
        }
        let mut converged = false;
        for _ in 0..40 {
            if best.outcome == "cutoff" && fbest[0].abs() < 0.02 && fbest[1].abs() < 0.02 {
                converged = true;
                break;
            }
            let h = 1e-3;
            let mut jac = [[0.0; 2]; 2];
            for k in 0..2 {
                let mut q = p;
                q[k] += h;
                let (_, fk) = eval(q, &best);
                jac[0][k] = (fk[0] - fbest[0]) / h;
                jac[1][k] = (fk[1] - fbest[1]) / h;
            }
            let det = jac[0][0] * jac[1][1] - jac[0][1] * jac[1][0];
            if det.abs() < 1e-14 {
                break;
            }
            let dx = [
                (-fbest[0] * jac[1][1] + fbest[1] * jac[0][1]) / det,
                (-fbest[1] * jac[0][0] + fbest[0] * jac[1][0]) / det,
            ];
            // Passo limitado a 10° e busca linear por redução do resíduo.
            let mut lam = (0.17 / dx[0].abs().max(dx[1].abs()).max(1e-12)).min(1.0);
            let mut improved = false;
            for _ in 0..8 {
                let q = [p[0] + lam * dx[0], p[1] + lam * dx[1]];
                let (run, f) = eval(q, &best);
                if norm(f) < norm(fbest) {
                    p = q;
                    fbest = f;
                    best = run;
                    improved = true;
                    break;
                }
                lam *= 0.5;
            }
            if !improved {
                break;
            }
        }
        if !converged {
            warnings.push("A guiagem do estágio superior não chegou à órbita alvo: o veículo pode não ter Δv suficiente para essa carga útil e altitude.".to_owned());
        }
        let (a, b) = to_ab(p);
        guidance = Some((a, b, converged));
    }
    let asc = best;

    // Monta as trajetórias.
    let main_ctx = |fixed: f64| base.ctx(fixed, None, None, Steer::Coast);
    let mut bodies = vec![Track {
        id: 0,
        name: format!("{} + carga útil", m.vehicle.name),
        parts: Vec::new(),
        models: vec!["3-DOF guiado".into()],
        parent: None,
        samples: Vec::new(),
        fate: String::new(),
        landing: None,
    }];
    // Reconstrução das linhas com o contexto de cada instante (massa e empuxo).
    let veh = &m.vehicle;
    let fixed_at = |t: f64, si: usize| -> f64 {
        let mut f = veh.payload
            + veh
                .stages
                .iter()
                .skip(si + 1)
                .map(Stage::wet_mass)
                .sum::<f64>()
            + veh.stages[si].dry_mass
            + veh.stages[si].reserve;
        if veh.fairing.is_some() && !asc.events.iter().any(|e| e.1 == "fairing" && e.0 <= t) {
            f += veh.fairing.map_or(0.0, |x| x.mass);
        }
        if si == 0 && !asc.events.iter().any(|e| e.1 == "boosterSep" && e.0 <= t) {
            f += veh.boosters.as_ref().map_or(0.0, |b| {
                f64::from(b.count) * (b.stage.dry_mass + b.stage.reserve)
            });
        }
        f
    };
    let mut dv = DvBudget {
        ideal: veh.ideal_dv(),
        ..DvBudget::default()
    };
    let mut max_q = (0.0, 0.0);
    let mut max_a: f64 = 0.0;
    let mut prev: Option<(f64, f64, f64, f64, f64, f64)> = None;
    for (t, y, si, ph) in &asc.rows {
        let stage = &veh.stages[*si];
        let booster = veh
            .boosters
            .as_ref()
            .filter(|_| y[7] > 0.0)
            .map(|b| (&b.stage, b.count));
        let eo = stage
            .engine_out
            .filter(|_| {
                asc.events
                    .iter()
                    .any(|ev| ev.1 == "engineOut" && ev.4 == *si && ev.0 <= *t)
            })
            .map_or(0, |e| e.engines);
        let steer = match ph {
            0 => Steer::Vertical,
            1 => Steer::Kick {
                t0: m.kick.start,
                dur: m.kick.duration,
                angle: m.kick.angle_deg.to_radians(),
            },
            2 => Steer::GravityTurn,
            4 => Steer::Coast,
            _ => {
                let (a, b, _) = guidance.unwrap_or((0.3, 0.001, false));
                let (t0, ys) = asc.upper_start.unwrap_or((*t, *y));
                let n = Vec3::new(ys[0], ys[1], ys[2]).cross(Vec3::new(ys[3], ys[4], ys[5]));
                Steer::Linear {
                    t0,
                    a,
                    b,
                    normal: n / n.norm(),
                }
            }
        };
        let ctx = base.ctx(fixed_at(*t, *si), Some((stage, eo)), booster, steer);
        let f = ctx.forces(*t, y);
        let mass = ctx.fixed + y[6] + y[7];
        let v = Vec3::new(y[3], y[4], y[5]);
        let vh = v / v.norm();
        let tm = f.thrust.norm() / mass;
        let cos = if f.thrust.norm() > 0.0 {
            f.thrust.dot(vh) / f.thrust.norm()
        } else {
            1.0
        };
        let cur = (
            tm,
            tm * (1.0 - cos),
            -f.drag.dot(vh) / mass,
            -f.gravity.dot(vh),
            f.pressure_loss,
            *t,
        );
        if let Some(p) = prev {
            let h = cur.5 - p.5;
            dv.delivered += 0.5 * h * (cur.0 + p.0);
            dv.steering_loss += 0.5 * h * (cur.1 + p.1);
            dv.drag_loss += 0.5 * h * (cur.2 + p.2);
            dv.gravity_loss += 0.5 * h * (cur.3 + p.3);
            dv.pressure_loss += 0.5 * h * (cur.4 + p.4);
        }
        prev = Some(cur);
        if f.q > max_q.0 {
            max_q = (f.q, *t);
        }
        max_a = max_a.max((f.thrust + f.drag).norm() / mass / 9.806_65);
        bodies[0].samples.push(row(&ctx, *t, y, &site));
    }
    let (t_end, y_end, si_end) = asc.end;
    let r0 = rot_z(planet.geodetic_to_ecef(site), theta0);
    dv.earth_rotation = planet.omega_vec().cross(r0).norm();
    dv.achieved = Vec3::new(y_end[3], y_end[4], y_end[5]).norm() - dv.earth_rotation;
    let end_ctx = main_ctx(fixed_at(t_end, si_end));
    bodies[0].samples.push(row(&end_ctx, t_end, &y_end, &site));

    let mut events: Vec<MissionEvent> = asc
        .events
        .iter()
        .map(|(t, k, msg, y, si, _)| {
            let ctx = base.ctx(fixed_at(*t, *si), None, None, Steer::Coast);
            let r = row(&ctx, *t, y, &site);
            MissionEvent {
                t: *t,
                body: 0,
                kind: k.clone(),
                message: msg.clone(),
                alt: r[1],
                speed: r[2],
            }
        })
        .collect();
    if max_q.0 > 0.0 {
        let r = bodies[0]
            .samples
            .iter()
            .find(|r| r[0] >= max_q.1)
            .copied()
            .unwrap_or([0.0; 16]);
        events.push(MissionEvent {
            t: max_q.1,
            body: 0,
            kind: "maxQ".into(),
            message: format!("Max-Q: {:.1} kPa.", max_q.0 / 1000.0),
            alt: r[1],
            speed: r[2],
        });
    }

    // Órbita no corte.
    let rv = (
        Vec3::new(y_end[0], y_end[1], y_end[2]),
        Vec3::new(y_end[3], y_end[4], y_end[5]),
    );
    let orbit = elements(rv.0, rv.1, planet.mu, planet.re);
    let reached = asc.outcome == "cutoff" && orbit.perigee_alt > 120e3;
    if asc.outcome == "depleted" {
        warnings.push(format!(
            "O último estágio esgotou o propelente antes da órbita alvo (perigeu de {:.0} km). Reduza a carga útil ou a altitude.",
            orbit.perigee_alt / 1000.0
        ));
    }
    if asc.outcome == "impact" {
        warnings.push(
            "O veículo caiu durante a subida: confira a manobra inicial e o empuxo/peso.".into(),
        );
    }

    // Propaga o estágio final por uma volta (ou até cair), para o globo.
    let launch = Launch {
        site,
        ground_h: m.site.2,
        rail: Rail {
            length: 0.0,
            elevation: 0.0,
            azimuth: 0.0,
            friction: 0.0,
        },
        initial_speed: 0.0,
        theta0,
    };
    let coast =
        |name: &str, t0: f64, y: [f64; 8], mass: f64, cda: f64, t_max: f64, dt: f64| -> Coasted {
            let v = ConstantVehicle {
                thrust_vac: 0.0,
                exit_area: 0.0,
                mdot: 0.0,
                burn_time: 0.0,
                cd_a: cda,
            };
            let cfg = Config3 {
                planet,
                atmosphere: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
                vehicle: &v,
                launch,
                m0: mass,
                t_max,
                output_dt: dt,
                ode: Options {
                    rtol: 1e-8,
                    atol: 1e-6,
                    h_max: 30.0,
                    max_steps: 400_000,
                },
                wind: m.wind.clone(),
                start: Some(Start3 {
                    t: t0,
                    r: Vec3::new(y[0], y[1], y[2]),
                    v: Vec3::new(y[3], y[4], y[5]),
                    mass,
                    burning: false,
                    fired: Vec::new(),
                    apogee: None,
                }),
            };
            let _ = name;
            match simulate3(&cfg) {
                Ok(res) => {
                    let rows = res
                        .samples
                        .iter()
                        .map(|s| {
                            let yy = [s.r.x, s.r.y, s.r.z, s.v.x, s.v.y, s.v.z, 0.0, 0.0];
                            let c = base.ctx(mass, None, None, Steer::Coast);
                            row(&c, s.t, &yy, &site)
                        })
                        .collect::<Vec<_>>();
                    let land = res.event(EventKind::Impact).map(|e| {
                        let rr = rows.last().copied().unwrap_or([0.0; 16]);
                        (e.t, e.nav.ground_speed, rr[6], rr[7])
                    });
                    (rows, land)
                }
                Err(_) => (Vec::new(), None),
            }
        };
    let last_stage = &veh.stages[si_end];
    let final_mass = fixed_at(t_end, si_end) + y_end[6].max(0.0);
    let period = if orbit.period.is_finite() {
        orbit.period
    } else {
        3600.0
    };
    let cda_final = 0.3 * 0.25 * core::f64::consts::PI * last_stage.diameter * last_stage.diameter;
    let (orbit_rows, land) = coast(
        "orbita",
        t_end,
        y_end,
        final_mass,
        cda_final,
        t_end + period.min(3.0 * 3600.0),
        10.0,
    );
    bodies[0].samples.extend(orbit_rows.into_iter().skip(1));
    if reached {
        bodies[0].fate = format!(
            "Em órbita: perigeu {:.0} km, apogeu {:.0} km, inclinação {:.2}°.",
            orbit.perigee_alt / 1000.0,
            orbit.apogee_alt / 1000.0,
            orbit.inc_deg
        );
        bodies[0]
            .models
            .push("órbita (2 corpos + J2 + arrasto)".into());
    } else if let Some((tl, vl, n, e)) = land {
        bodies[0].fate = format!(
            "Não atingiu a órbita: caiu a {:.0} km do lançamento.",
            n.hypot(e) / 1000.0
        );
        bodies[0].landing = Some([tl, vl, n, e]);
    } else {
        bodies[0].fate = "Não atingiu a órbita alvo.".into();
    }

    // Estágios, boosters e carenagem descartados: queda balística capotando até o solo.
    for (name, t0, y, mass, d, l) in &asc.drops {
        let area = core::f64::consts::PI * d * l + 0.5 * core::f64::consts::PI * d * d;
        let (rows, land) = coast(name, *t0, *y, *mass, area / 4.0, t0 + 3.0 * 3600.0, 1.0);
        let id = bodies.len();
        let fate = land.map_or_else(
            || "Sem impacto no tempo simulado.".to_owned(),
            |(_, _, n, e)| {
                format!(
                    "Cai a {:.0} km do lançamento (zona de queda).",
                    n.hypot(e) / 1000.0
                )
            },
        );
        if let Some((tl, vl, n, e)) = land {
            events.push(MissionEvent {
                t: tl,
                body: id,
                kind: "impact".into(),
                message: format!("{name} toca a superfície a {:.0} km.", n.hypot(e) / 1000.0),
                alt: 0.0,
                speed: vl,
            });
        }
        bodies.push(Track {
            id,
            name: name.clone(),
            parts: Vec::new(),
            models: vec!["3-DOF capotando".into()],
            parent: Some(0),
            samples: rows,
            fate,
            landing: land.map(|(tl, vl, n, e)| [tl, vl, n, e]),
        });
    }
    events.sort_by(|a, b| a.t.total_cmp(&b.t));
    MissionOutput {
        bodies,
        events,
        warnings,
        orbit: Some(orbit),
        reached_orbit: reached,
        dv,
        guidance,
        liftoff_mass: veh.liftoff_mass(),
        max_q,
        max_accel_g: max_a,
        propellant_margin: if reached { y_end[6].max(0.0) } else { 0.0 },
        kick_deg: m.kick.angle_deg,
    }
}
