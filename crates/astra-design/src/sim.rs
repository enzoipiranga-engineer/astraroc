//! Voo com falhas físicas: cada corpo voa até tocar o solo ou até um limite quebrar;
//! então ele é dividido e cada parte continua com a própria trajetória.

use crate::assembly::Assembly;
use crate::limits::{surface_area, BodyVehicle, Limit, LimitKind, PointSystem, TUMBLE_CD};
use crate::material::MaterialDb;
use crate::model::{Design, PartKind};
use astra_dynamics::flight::{
    simulate3, simulate6, Config3, Config6, EndReason, EventKind, Launch, Nav, Rail, Start3, Start6,
};
use astra_dynamics::ode::Options;
use astra_env::{AtmosphereModel, Geodetic, Planet};
use astra_math::{Mat3, Quat, Vec3};
use astra_vehicle::Parachute;
use serde::Serialize;
use std::collections::BTreeSet;

/// Trajetória de um corpo.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyTrack {
    /// Índice do corpo.
    pub id: usize,
    /// Nome legível.
    pub name: String,
    /// Peças do corpo (identificadores).
    pub parts: Vec<String>,
    /// Modelo usado em cada trecho ("6-DOF", "3-DOF capotando", "3-DOF sob paraquedas").
    pub models: Vec<String>,
    /// Corpo de onde saiu.
    pub parent: Option<usize>,
    /// Colunas: t (s), altura acima do solo (m), velocidade em relação ao ar (m/s),
    /// velocidade vertical (m/s), Mach, pressão dinâmica (Pa), norte (m), leste (m),
    /// ângulo de ataque (°), massa (kg), latitude (°), longitude (°), altitude (m), eixo do
    /// corpo em coordenadas fixas à Terra (x, y, z; no 3-DOF, a direção da velocidade).
    pub samples: Vec<Row>,
    /// Como terminou, em linguagem simples.
    pub fate: String,
    /// Pouso: [t, velocidade, norte, leste] quando tocou o solo.
    pub landing: Option<[f64; 4]>,
}

/// Acontecimento do voo.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlightEvent {
    /// Instante, s.
    pub t: f64,
    /// Corpo.
    pub body: usize,
    /// Código: railExit, burnout, apogee, impact, ejection, ejectionFailed, deploy,
    /// jointBroken, finLost, buckled, cordBroken, noExit.
    pub kind: String,
    /// Descrição em linguagem simples.
    pub message: String,
    /// Altura acima do solo, m.
    pub alt: f64,
    /// Velocidade, m/s.
    pub speed: f64,
}

/// Resultado da simulação.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimOutput {
    /// Corpos.
    pub bodies: Vec<BodyTrack>,
    /// Acontecimentos em ordem de tempo.
    pub events: Vec<FlightEvent>,
    /// Avisos do motor e da simulação.
    pub warnings: Vec<String>,
}

pub(crate) fn launch_of(d: &Design) -> Launch {
    let l = &d.launch;
    Launch {
        site: Geodetic {
            lat: l.lat_deg.to_radians(),
            lon: l.lon_deg.to_radians(),
            h: l.altitude,
        },
        ground_h: l.altitude,
        rail: Rail {
            length: l.rail_length,
            elevation: l.rail_elevation_deg.to_radians(),
            azimuth: l.rail_azimuth_deg.to_radians(),
            friction: 0.0,
        },
        initial_speed: 0.0,
        theta0: theta0_of(d),
    }
}

/// Ângulo de rotação da Terra no lançamento (hora sideral de Greenwich), rad; zero sem data.
pub(crate) fn theta0_of(d: &Design) -> f64 {
    d.launch
        .utc
        .as_deref()
        .and_then(|u| astra_env::time::parse_utc(u).ok())
        .map_or(0.0, astra_env::time::gmst)
}

/// Linha de amostra: t, altura acima do solo, velocidade do ar, velocidade vertical, Mach,
/// pressão dinâmica, norte, leste, α (graus), massa, latitude e longitude (graus),
/// altitude (m), eixo do corpo em ECEF (x, y, z).
pub(crate) type Row = [f64; 16];

#[allow(clippy::too_many_arguments)]
fn row(
    planet: &Planet,
    l: &Launch,
    t: f64,
    nav: &Nav,
    alpha_deg: f64,
    mass: f64,
    axis_eci: Vec3,
) -> Row {
    let (no, ea) = north_east(planet, l, nav);
    let ax = astra_env::rot_z(axis_eci, -(l.theta0 + planet.omega * t));
    [
        t,
        nav.alt - l.ground_h,
        nav.speed,
        nav.vertical_speed,
        nav.mach,
        nav.q_dyn,
        no,
        ea,
        alpha_deg,
        mass,
        nav.lat.to_degrees(),
        nav.lon.to_degrees(),
        nav.alt,
        ax.x,
        ax.y,
        ax.z,
    ]
}

pub(crate) fn opts() -> Options {
    Options {
        rtol: 1e-7,
        atol: 1e-7,
        h_max: 0.25,
        max_steps: 400_000,
    }
}

/// Estado comum no fim de um trecho.
#[derive(Clone, Debug)]
pub(crate) struct State {
    pub t: f64,
    pub r: Vec3,
    pub v: Vec3,
    pub q: Option<Quat>,
    pub omega: Vec3,
    pub apogee: Option<f64>,
}

struct Pending {
    members: Vec<usize>,
    state: Option<State>,
    parent: Option<usize>,
    track: Option<usize>,
}

/// Estado global das falhas e da recuperação.
pub(crate) struct Status {
    pub broken: BTreeSet<usize>,
    pub fin_loss: Vec<u32>,
    pub deployed: Vec<Option<f64>>,
    pub ejected: Vec<bool>,
    pub collapsed: BTreeSet<usize>,
}

fn north_east(planet: &Planet, l: &Launch, n: &Nav) -> (f64, f64) {
    (
        (n.lat - l.site.lat) * planet.re,
        (n.lon - l.site.lon) * planet.re * l.site.lat.cos(),
    )
}

fn chute_canopy(asm: &Assembly, c: usize, t: f64, speed: f64) -> Parachute {
    let PartKind::Parachute {
        diameter,
        cd,
        deploy,
        ..
    } = asm.design.parts[c].kind
    else {
        unreachable!()
    };
    let _ = t;
    Parachute {
        name: asm.design.parts[c].name.clone(),
        cd_s: cd * 0.25 * core::f64::consts::PI * diameter * diameter,
        trigger: crate::limits::trigger_of(deploy),
        inflation_time: Parachute::fill_time(diameter, speed),
        reef: None,
    }
}

fn name_of(asm: &Assembly, members: &[usize]) -> String {
    if members.contains(&asm.root) {
        return "Foguete".to_owned();
    }
    members
        .iter()
        .find(|&&i| crate::props::is_external(&asm.design.parts[i].kind))
        .or_else(|| {
            members
                .iter()
                .filter(|&&i| {
                    !matches!(
                        asm.design.parts[i].kind,
                        PartKind::Parachute { .. } | PartKind::ShockCord { .. }
                    )
                })
                .max_by(|&&a, &&b| asm.props[a].mass.total_cmp(&asm.props[b].mass))
        })
        .or(members.first())
        .map_or_else(|| "Peça".to_owned(), |&i| asm.design.parts[i].name.clone())
}

/// Simula o voo do projeto com as falhas físicas.
#[must_use]
pub fn simulate(design: &Design, db: &MaterialDb) -> SimOutput {
    let asm = Assembly::new(design, db);
    run(&asm, false).0
}

/// Cargas mínimas (margens) registradas no voo nominal: por peça de limite.
pub(crate) struct Nominal {
    pub out: SimOutput,
    /// (limite, menor margem) do corpo inteiro na subida e da descida sob paraquedas.
    pub margins: Vec<(Limit, f64)>,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn run(asm: &Assembly, nominal: bool) -> (SimOutput, Option<Nominal>) {
    let planet = Planet::EARTH;
    let atmosphere = AtmosphereModel::Ussa1976 {
        delta_t: asm.design.launch.delta_t,
    };
    let launch = launch_of(&asm.design);
    let wind = asm.design.launch.wind.to_wind();
    let p_ground = atmosphere.at(launch.ground_h).pressure;
    let n = asm.design.parts.len();
    let mut st = Status {
        broken: BTreeSet::new(),
        fin_loss: vec![0; n],
        deployed: vec![None; n],
        ejected: vec![false; n],
        collapsed: BTreeSet::new(),
    };
    let mut out = SimOutput {
        bodies: Vec::new(),
        events: Vec::new(),
        warnings: Vec::new(),
    };
    if let Some(e) = &asm.motor_error {
        out.warnings.push(format!("Motor: {e}"));
    }
    if let Some(w) = &asm.water {
        out.warnings.extend(w.warnings.iter().cloned());
    }
    let mut nominal_margins: Vec<(Limit, f64)> = Vec::new();

    // Começa pelo sistema que contém a peça raiz; o que não estiver amarrado a ele
    // nem rigidamente preso já cai como peça solta no lançador (fora do escopo do voo).
    let all: Vec<usize> = (0..n).collect();
    let systems = asm.tethered(&all, &st.broken);
    let main = systems
        .iter()
        .find(|s| s.contains(&asm.root))
        .cloned()
        .unwrap_or_default();
    for s in systems.iter().filter(|s| !s.contains(&asm.root)) {
        let names: Vec<&str> = s
            .iter()
            .map(|&i| asm.design.parts[i].name.as_str())
            .collect();
        out.warnings.push(format!(
            "Peças sem nenhuma ligação com o foguete ficaram no lançador: {}.",
            names.join(", ")
        ));
    }
    let mut queue = vec![Pending {
        members: main,
        state: None,
        parent: None,
        track: None,
    }];
    let mut guard = 0;
    while let Some(job) = queue.pop() {
        guard += 1;
        if guard > 120 {
            out.warnings
                .push("Simulação interrompida: número excessivo de separações.".to_owned());
            break;
        }
        let members = job.members;
        let track = job.track.unwrap_or_else(|| {
            out.bodies.push(BodyTrack {
                id: out.bodies.len(),
                name: name_of(asm, &members),
                parts: members
                    .iter()
                    .map(|&i| asm.design.parts[i].id.clone())
                    .collect(),
                models: Vec::new(),
                parent: job.parent,
                samples: Vec::new(),
                fate: String::new(),
                landing: None,
            });
            out.bodies.len() - 1
        });
        let rigid = asm.rigid_bodies(&members, &st.broken);
        let chutes_here: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| matches!(asm.design.parts[i].kind, PartKind::Parachute { .. }))
            .collect();
        let open: Vec<usize> = chutes_here
            .iter()
            .copied()
            .filter(|&c| st.deployed[c].is_some())
            .collect();
        let pending_chutes: Vec<usize> = chutes_here
            .iter()
            .copied()
            .filter(|&c| st.deployed[c].is_none() && !st.ejected[c])
            .collect();
        let structural: Vec<Vec<usize>> = rigid
            .iter()
            .filter(|b| {
                b.iter().any(|&i| {
                    !matches!(
                        asm.design.parts[i].kind,
                        PartKind::Parachute { .. } | PartKind::ShockCord { .. }
                    )
                })
            })
            .cloned()
            .collect();
        let single = structural.len() == 1
            && open.is_empty()
            && !structural[0].iter().any(|i| st.collapsed.contains(i));
        // No lançamento o corpo inteiro voa em 6-DOF mesmo instável (é assim que a
        // capotagem aparece). Partes soltas depois só voltam ao 6-DOF se tiverem nariz e
        // forem estáveis; senão capotam como ponto de massa.
        let six = single
            && (job.state.is_none() || {
                let body = &structural[0];
                let r = asm.rocket(body, &st.fin_loss);
                let t = job.state.as_ref().map_or(0.0, |s| s.t);
                r.geometry()
                    .parts
                    .first()
                    .is_some_and(|p| matches!(p, astra_aero::BodyPart::Nose { .. }))
                    && r.static_margin(t, 0.3) > 0.3
            });
        // Peças sem estrutura (paraquedas, cordão) vão junto no corpo de 6-DOF como massa.
        let push_samples =
            |out: &mut SimOutput, rows: Vec<Row>| out.bodies[track].samples.extend(rows);
        let end_state: State;
        let reason: EndReason;
        let final_nav: Option<Nav>;
        let mut sys_limits: Vec<Limit> = Vec::new();
        let kind_names: Vec<(EventKind, f64, Nav)>;
        if six {
            let mut body_members = structural[0].clone();
            for &m in &members {
                if !body_members.contains(&m) {
                    body_members.push(m);
                }
            }
            let veh = BodyVehicle::new(
                asm,
                body_members.clone(),
                &st.fin_loss,
                &st.broken,
                pending_chutes.clone(),
                p_ground,
                nominal && job.state.is_none(),
                job.state.is_none(),
            );
            let start = job.state.as_ref().map(|s| {
                let fired = vec![None; pending_chutes.len()];
                Start6 {
                    t: s.t,
                    r: s.r,
                    v: s.v,
                    q: s.q
                        .unwrap_or_else(|| align_to(s.v - planet.omega_vec().cross(s.r))),
                    omega: s.omega,
                    mass: veh.rocket.mass(s.t),
                    burning: s.t < veh.rocket.motor.burn_time(),
                    fired,
                    apogee: s.apogee,
                }
            });
            let cfg = Config6 {
                planet,
                atmosphere,
                vehicle: &veh,
                launch,
                m0: veh.rocket.mass(0.0),
                initial_omega_body: Vec3::ZERO,
                t_max: 1800.0,
                output_dt: 0.05,
                ode: opts(),
                start,
                wind: wind.clone(),
            };
            if out.bodies[track].models.last().map(String::as_str) != Some("6-DOF") {
                out.bodies[track].models.push("6-DOF".to_owned());
            }
            match simulate6(&cfg) {
                Ok(res) => {
                    let rows = res
                        .samples
                        .iter()
                        .map(|s| {
                            row(
                                &planet,
                                &launch,
                                s.t,
                                &s.nav,
                                s.alpha.to_degrees(),
                                s.mass,
                                s.q.rotate(Vec3::X),
                            )
                        })
                        .collect();
                    push_samples(&mut out, rows);
                    kind_names = res.events.iter().map(|e| (e.0, e.1.t, e.1.nav)).collect();
                    let last = res.last().cloned();
                    let end = res.end.clone();
                    if let Some(mon) = &veh.monitor {
                        for (l, m) in veh.limits.iter().zip(mon.borrow().iter()) {
                            nominal_margins.push((l.clone(), *m));
                        }
                    }
                    sys_limits = veh.limits.clone();
                    let (Some(last), Some(end)) = (last, end) else {
                        continue;
                    };
                    final_nav = Some(last.nav);
                    end_state = State {
                        t: last.t,
                        r: last.r,
                        v: last.v,
                        q: Some(last.q),
                        omega: last.omega,
                        apogee: end.apogee,
                    };
                    reason = match end.reason {
                        EndReason::Trigger(k) => EndReason::Trigger(pending_chutes[k]),
                        r => r,
                    };
                }
                Err(e) => {
                    out.warnings.push(format!(
                        "Falha numérica no voo de {}: {e}",
                        out.bodies[track].name
                    ));
                    out.bodies[track].fate = "simulação interrompida".to_owned();
                    continue;
                }
            }
        } else {
            let bodies: Vec<astra_vehicle::Rocket> = structural
                .iter()
                .map(|b| asm.rocket(b, &st.fin_loss))
                .collect();
            let tumble: Vec<f64> = bodies
                .iter()
                .map(|r| TUMBLE_CD * surface_area(r.geometry()) / 4.0)
                .collect();
            let motor_body = structural.iter().position(|b| b.contains(&asm.root));
            let t_now = job.state.as_ref().map_or(0.0, |s| s.t);
            let speed_now = job
                .state
                .as_ref()
                .map_or(1.0, |s| (s.v - planet.omega_vec().cross(s.r)).norm());
            let open_list: Vec<(usize, f64, Parachute)> = open
                .iter()
                .map(|&c| {
                    (
                        c,
                        st.deployed[c].unwrap_or(t_now),
                        chute_canopy(asm, c, t_now, speed_now),
                    )
                })
                .collect();
            // Cordão mais fraco do sistema (juntas de cordão e cordões de choque).
            let mut weakest: Option<(usize, f64)> = None;
            for (ji, j) in asm.active(&st.broken) {
                if !j.is_cord || !members.contains(&j.a) || !members.contains(&j.b) {
                    continue;
                }
                let cord_part =
                    [j.a, j.b]
                        .into_iter()
                        .find_map(|i| match asm.design.parts[i].kind {
                            PartKind::ShockCord { strength, .. } => Some(strength),
                            _ => None,
                        });
                let s = cord_part.map_or(j.tension, |c| c.min(j.tension));
                if weakest.is_none_or(|w| s < w.1) {
                    weakest = Some((ji, s));
                }
            }
            if let (Some((ji, s)), false) = (weakest, open_list.is_empty()) {
                sys_limits.push(Limit {
                    kind: LimitKind::Cord {
                        joint: ji,
                        strength: s,
                    },
                });
            }
            let mut extra = 0.0;
            let mut label = if open_list.is_empty() {
                "3-DOF capotando"
            } else {
                "3-DOF sob paraquedas"
            }
            .to_owned();
            if structural.is_empty() {
                // Só paraquedas e cordões: caem como tecido.
                extra = 0.05;
                label = "3-DOF (tecido)".to_owned();
            }
            let sys = PointSystem {
                asm,
                bodies,
                tumble_cda: tumble,
                open: open_list,
                pending: pending_chutes.clone(),
                extra_cda: extra,
                limits: sys_limits.clone(),
                motor_body,
                monitor: nominal
                    .then(|| std::cell::RefCell::new(vec![f64::INFINITY; sys_limits.len()])),
            };
            let mass: f64 = members.iter().map(|&i| asm.props[i].mass).sum::<f64>()
                + motor_body.map_or(0.0, |_| asm.motor.dry_mass + asm.motor.prop_mass(t_now))
                - structural
                    .iter()
                    .flatten()
                    .filter_map(|&i| fin_lost_mass(asm, i, &st.fin_loss))
                    .sum::<f64>();
            let s = job.state.clone().unwrap_or(State {
                t: 0.0,
                r: Vec3::ZERO,
                v: Vec3::ZERO,
                q: None,
                omega: Vec3::ZERO,
                apogee: None,
            });
            let cfg = Config3 {
                planet,
                atmosphere,
                vehicle: &sys,
                launch,
                m0: mass.max(1e-4),
                t_max: 3600.0,
                output_dt: 0.1,
                ode: opts(),
                start: job.state.as_ref().map(|_| Start3 {
                    t: s.t,
                    r: s.r,
                    v: s.v,
                    mass: mass.max(1e-4),
                    burning: motor_body.is_some() && s.t < asm.motor.burn_time(),
                    fired: vec![None; pending_chutes.len()],
                    apogee: s.apogee,
                }),
                wind: wind.clone(),
            };
            if out.bodies[track].models.last() != Some(&label) {
                out.bodies[track].models.push(label);
            }
            match simulate3(&cfg) {
                Ok(res) => {
                    let rows = res
                        .samples
                        .iter()
                        .map(|x| {
                            row(
                                &planet,
                                &launch,
                                x.t,
                                &x.nav,
                                f64::NAN,
                                x.mass,
                                (x.v - planet.omega_vec().cross(x.r))
                                    .normalized()
                                    .unwrap_or(Vec3::Z),
                            )
                        })
                        .collect();
                    push_samples(&mut out, rows);
                    kind_names = res.events.iter().map(|e| (e.0, e.1.t, e.1.nav)).collect();
                    if let Some(mon) = &sys.monitor {
                        for (l, m) in sys.limits.iter().zip(mon.borrow().iter()) {
                            nominal_margins.push((l.clone(), *m));
                        }
                    }
                    let (Some(last), Some(end)) = (res.samples.last().cloned(), res.end.clone())
                    else {
                        continue;
                    };
                    final_nav = Some(last.nav);
                    end_state = State {
                        t: last.t,
                        r: last.r,
                        v: last.v,
                        q: None,
                        omega: Vec3::ZERO,
                        apogee: end.apogee,
                    };
                    reason = match end.reason {
                        EndReason::Trigger(k) => EndReason::Trigger(pending_chutes[k]),
                        r => r,
                    };
                }
                Err(e) => {
                    out.warnings.push(format!(
                        "Falha numérica no voo de {}: {e}",
                        out.bodies[track].name
                    ));
                    out.bodies[track].fate = "simulação interrompida".to_owned();
                    continue;
                }
            }
        }

        // Eventos do trecho.
        for (k, t, nav) in &kind_names {
            let (kind, msg) = match k {
                EventKind::RailExit => (
                    "railExit",
                    format!("Saiu do trilho a {:.1} m/s.", nav.speed),
                ),
                EventKind::Burnout => ("burnout", "Fim da queima do motor.".to_owned()),
                EventKind::Apogee => (
                    "apogee",
                    format!("Apogeu a {:.1} m acima do solo.", nav.alt - launch.ground_h),
                ),
                EventKind::Impact => (
                    "impact",
                    format!("Tocou o solo a {:.1} m/s.", nav.ground_speed),
                ),
                _ => continue,
            };
            out.events.push(FlightEvent {
                t: *t,
                body: track,
                kind: kind.to_owned(),
                message: msg,
                alt: nav.alt - launch.ground_h,
                speed: nav.speed,
            });
        }
        let nav = final_nav.unwrap_or(Nav {
            ground_speed: 0.0,
            lat: 0.0,
            lon: 0.0,
            alt: 0.0,
            speed: 0.0,
            vertical_speed: 0.0,
            mach: 0.0,
            q_dyn: 0.0,
        });
        let ev = |out: &mut SimOutput, kind: &str, message: String| {
            out.events.push(FlightEvent {
                t: end_state.t,
                body: track,
                kind: kind.to_owned(),
                message,
                alt: nav.alt - launch.ground_h,
                speed: nav.speed,
            });
        };

        // Consequências.
        match reason {
            EndReason::Impact => {
                let (no, ea) = north_east(&planet, &launch, &nav);
                let b = &mut out.bodies[track];
                b.landing = Some([end_state.t, nav.ground_speed, no, ea]);
                b.fate = if nav.ground_speed > 15.0 {
                    format!(
                        "Impacto forte a {:.0} m/s: risco de dano à peça e a quem estiver perto.",
                        nav.ground_speed
                    )
                } else {
                    format!("Pouso a {:.1} m/s.", nav.ground_speed)
                };
                continue;
            }
            EndReason::TimeLimit => {
                out.bodies[track].fate = "Ainda em voo no fim do tempo simulado.".to_owned();
                continue;
            }
            EndReason::Trigger(c) => {
                st.ejected[c] = true;
                let PartKind::Parachute {
                    ejection_charge, ..
                } = asm.design.parts[c].kind
                else {
                    unreachable!()
                };
                let cname = &asm.design.parts[c].name;
                let sep = asm.separation_joint(c);
                match sep {
                    None => ev(&mut out, "noExit", format!("{cname}: sem junta de separação definida, a carga de ejeção não tem por onde abrir o foguete. O paraquedas ficou dentro.")),
                    Some(ji) if st.broken.contains(&ji) => {
                        st.deployed[c] = Some(end_state.t);
                        ev(&mut out, "deploy", format!("{cname} abriu (a baia já estava aberta)."));
                    }
                    Some(ji) => {
                        let body = rigid.iter().find(|b| b.contains(&asm.joints[ji].as_ref().map_or(usize::MAX, |j| j.a))).cloned().unwrap_or_default();
                        let anchor = if body.contains(&asm.root) { asm.root } else { body.first().copied().unwrap_or(0) };
                        let cut = asm.cuts(&body, anchor, &st.broken).into_iter().filter(|cu| cu.joints.contains(&ji)).min_by_key(|cu| cu.joints.len());
                        let force = if ejection_charge > 0.0 { asm.ejection_force(ji, c, ejection_charge) } else { 0.0 };
                        match cut {
                            Some(cu) if force > cu.tension => {
                                st.broken.extend(cu.joints.iter().copied());
                                st.deployed[c] = Some(end_state.t);
                                ev(&mut out, "ejection", format!("Carga de ejeção de {:.2} g gerou {:.0} N e venceu os {:.0} N da junta: baia aberta, {cname} saiu.", ejection_charge * 1000.0, force, cu.tension));
                            }
                            Some(cu) => ev(&mut out, "ejectionFailed", format!("Carga de ejeção de {:.2} g gerou só {:.0} N, menos que os {:.0} N da junta: a baia não abriu e {cname} ficou preso.", ejection_charge * 1000.0, force, cu.tension)),
                            None => ev(&mut out, "ejectionFailed", format!("{cname}: a junta de separação não divide o foguete em duas partes (há outra ligação rígida). A baia não abre.")),
                        }
                    }
                }
            }
            EndReason::Limit(i) => {
                let l = &sys_limits[i];
                match &l.kind {
                    LimitKind::Cut(cu) => {
                        st.broken.extend(cu.joints.iter().copied());
                        let names: Vec<String> = cu
                            .joints
                            .iter()
                            .map(|&j| {
                                asm.joints[j]
                                    .as_ref()
                                    .map_or_else(String::new, |x| x.name.clone())
                            })
                            .collect();
                        let distal: Vec<&str> = cu
                            .distal
                            .iter()
                            .map(|&p| asm.design.parts[p].name.as_str())
                            .collect();
                        let msg = if cu.tension <= 0.0 {
                            format!("{} estava só encostada ({}): começou a ser puxada e se soltou do resto.", distal.join(", "), names.join(" + "))
                        } else {
                            format!("A junta {} rompeu ({:.0} N de capacidade em tração): {} se soltou do resto.", names.join(" + "), cu.tension, distal.join(", "))
                        };
                        ev(&mut out, "jointBroken", msg);
                        // Paraquedas cuja baia abriu antes da hora.
                        for &c in &chutes_here {
                            {
                                let opened = asm
                                    .separation_joint(c)
                                    .is_some_and(|ji| cu.joints.contains(&ji));
                                if opened && st.deployed[c].is_none() {
                                    st.deployed[c] = Some(end_state.t);
                                    let cname = asm.design.parts[c].name.clone();
                                    ev(&mut out, "deploy", format!("{cname} foi puxado para fora pela separação e abriu antes da hora, a {:.0} m/s.", nav.speed));
                                }
                            }
                        }
                    }
                    LimitKind::FinRoot { part, .. } | LimitKind::Flutter { part, .. } => {
                        st.fin_loss[*part] += 1;
                        let why = if matches!(l.kind, LimitKind::Flutter { .. }) {
                            "entrou em flutter e se partiu"
                        } else {
                            "arrancou pela raiz (flexão acima da capacidade da colagem)"
                        };
                        ev(
                            &mut out,
                            "finLost",
                            format!(
                                "Uma aleta de {} {why} a {:.0} m/s.",
                                asm.design.parts[*part].name, nav.speed
                            ),
                        );
                        // A aleta solta cai como placa.
                        if let PartKind::FinSet {
                            root_chord,
                            tip_chord,
                            span,
                            ..
                        } = asm.design.parts[*part].kind
                        {
                            let area = 0.5 * (root_chord + tip_chord) * span;
                            let count = match asm.design.parts[*part].kind {
                                PartKind::FinSet { count, .. } => count,
                                _ => 1,
                            };
                            let mf = asm.props[*part].mass / f64::from(count.max(1));
                            let id = out.bodies.len();
                            let rows = fall_plate(
                                &planet,
                                &atmosphere,
                                &wind,
                                &launch,
                                &end_state,
                                mf,
                                TUMBLE_CD * 2.0 * area / 4.0,
                            );
                            let landing = rows.last().map(|r| [r[0], r[2], r[6], r[7]]);
                            out.bodies.push(BodyTrack {
                                id,
                                name: format!("Aleta solta ({})", asm.design.parts[*part].name),
                                parts: vec![asm.design.parts[*part].id.clone()],
                                models: vec!["3-DOF capotando".to_owned()],
                                parent: Some(track),
                                samples: rows,
                                fate: "Caiu solta.".to_owned(),
                                landing,
                            });
                        }
                    }
                    LimitKind::Buckling { part } => {
                        for &m in &members {
                            st.collapsed.insert(m);
                        }
                        ev(&mut out, "buckled", format!("O tubo {} flambou sob compressão e flexão: a estrutura colapsou e o foguete passou a capotar.", asm.design.parts[*part].name));
                    }
                    LimitKind::Cord { joint, strength } => {
                        st.broken.insert(*joint);
                        let jn = asm.joints[*joint]
                            .as_ref()
                            .map_or_else(String::new, |x| x.name.clone());
                        ev(&mut out, "cordBroken", format!("A força de abertura passou dos {strength:.0} N do cordão ({jn}): as partes se separaram."));
                    }
                }
            }
        }

        // Novos sistemas a partir do estado final. Quem fica com o motor (ou, sem ele, o
        // mais pesado) continua a mesma trajetória; os outros ganham trajetória própria.
        let systems = asm.tethered(&members, &st.broken);
        let same = systems.len() == 1;
        if !same {
            out.bodies[track].fate = format!("Dividiu-se em {} partes.", systems.len());
        }
        let mass_of = |sys: &Vec<usize>| sys.iter().map(|&i| asm.props[i].mass).sum::<f64>();
        let keeper = systems
            .iter()
            .position(|s| s.contains(&asm.root))
            .or_else(|| {
                (0..systems.len())
                    .max_by(|&a, &b| mass_of(&systems[a]).total_cmp(&mass_of(&systems[b])))
            });
        let x_cg = cg_station(asm, &members);
        for (k, sys) in systems.into_iter().enumerate() {
            let mut s = end_state.clone();
            if let (Some(q), false) = (s.q, same) {
                let d = Vec3::X * (x_cg - cg_station(asm, &sys));
                s.r += q.rotate(d);
                s.v += q.rotate(s.omega.cross(d));
            }
            let keep = Some(k) == keeper;
            queue.push(Pending {
                members: sys,
                state: Some(s),
                parent: (!keep).then_some(track),
                track: keep.then_some(track),
            });
        }
    }
    out.events.sort_by(|a, b| a.t.total_cmp(&b.t));
    let nominal_out = nominal.then(|| Nominal {
        out: out.clone(),
        margins: nominal_margins,
    });
    (out, nominal_out)
}

fn fin_lost_mass(asm: &Assembly, i: usize, fin_loss: &[u32]) -> Option<f64> {
    match asm.design.parts[i].kind {
        PartKind::FinSet { count, .. } if fin_loss[i] > 0 => {
            Some(asm.props[i].mass * f64::from(fin_loss[i].min(count)) / f64::from(count.max(1)))
        }
        _ => None,
    }
}

fn cg_station(asm: &Assembly, parts: &[usize]) -> f64 {
    let m: f64 = parts.iter().map(|&i| asm.props[i].mass).sum();
    if m > 0.0 {
        parts
            .iter()
            .map(|&i| asm.props[i].mass * asm.props[i].cg)
            .sum::<f64>()
            / m
    } else {
        0.0
    }
}

/// Atitude com o eixo x do corpo ao longo de `dir` (para corpos que voltam ao 6-DOF).
fn align_to(dir: Vec3) -> Quat {
    let x = dir.normalized().unwrap_or(Vec3::Z);
    let helper = if x.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    let y = helper.cross(x).normalized().unwrap_or(Vec3::Y);
    Quat::from_mat3(Mat3::from_cols(x, y, x.cross(y)))
}

/// Queda de uma placa solta como massa pontual de área de arrasto fixa.
fn fall_plate(
    planet: &Planet,
    atm: &AtmosphereModel,
    wind: &astra_env::Wind,
    launch: &Launch,
    s: &State,
    mass: f64,
    cda: f64,
) -> Vec<Row> {
    let v = astra_dynamics::flight::ConstantVehicle {
        thrust_vac: 0.0,
        exit_area: 0.0,
        mdot: 0.0,
        burn_time: 0.0,
        cd_a: cda,
    };
    let cfg = Config3 {
        planet: *planet,
        atmosphere: *atm,
        vehicle: &v,
        launch: *launch,
        m0: mass.max(1e-4),
        t_max: 3600.0,
        output_dt: 0.1,
        ode: opts(),
        start: Some(Start3 {
            t: s.t,
            r: s.r,
            v: s.v,
            mass: mass.max(1e-4),
            burning: false,
            fired: Vec::new(),
            apogee: s.apogee,
        }),
        wind: wind.clone(),
    };
    simulate3(&cfg).map_or_else(
        |_| Vec::new(),
        |res| {
            res.samples
                .iter()
                .map(|x| {
                    row(
                        planet,
                        launch,
                        x.t,
                        &x.nav,
                        f64::NAN,
                        x.mass,
                        (x.v - planet.omega_vec().cross(x.r))
                            .normalized()
                            .unwrap_or(Vec3::Z),
                    )
                })
                .collect()
        },
    )
}
