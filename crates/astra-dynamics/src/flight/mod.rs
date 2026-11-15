//! Equações de voo sobre um planeta em rotação, integradas no referencial inercial (ECI).
//!
//! Convenções: estado com posição e velocidade ECI nas seis primeiras componentes;
//! o referencial fixo ao planeta (ECEF) coincide com o ECI girado de θ = θ0 + ω·t em z.
//! Coriolis e centrífuga não aparecem como forças: surgem da rotação do solo, do trilho
//! e da atmosfera em relação ao referencial inercial.

mod dof3;
mod dof6;

pub use dof3::{simulate3, Config3, ConstantVehicle, PointVehicle, Result3, Sample3};
pub use dof6::{simulate6, Config6, Loads, MassProps, Result6, RigidVehicle, Sample6};

use crate::ode::{integrate, Direction, EventSpec, OdeError, Options};
use astra_env::{rot_z, AtmoState, AtmosphereModel, Geodetic, Planet};
use astra_math::Vec3;
use core::f64::consts::PI;

/// Trilho ou torre de lançamento, fixo ao solo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rail {
    /// Comprimento útil de guiagem, m (0 = sem trilho).
    pub length: f64,
    /// Elevação acima do horizonte, rad (π/2 = vertical).
    pub elevation: f64,
    /// Azimute a partir do norte, sentido horário, rad.
    pub azimuth: f64,
    /// Coeficiente de atrito cinético entre guias e trilho (adimensional).
    pub friction: f64,
}

/// Condições de lançamento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    /// Posição inicial do veículo (altitude sobre o elipsoide incluída).
    pub site: Geodetic,
    /// Altitude do solo usada para detectar o impacto, m acima do elipsoide.
    pub ground_h: f64,
    /// Trilho; a direção dele também orienta o veículo no instante inicial.
    pub rail: Rail,
    /// Velocidade inicial em relação ao solo, ao longo do eixo do trilho, m/s.
    pub initial_speed: f64,
    /// Ângulo de rotação do planeta em t = 0, rad (ECEF em relação ao ECI).
    pub theta0: f64,
}

/// Tipo de evento de voo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// O veículo deixou o trilho.
    RailExit,
    /// Fim da queima.
    Burnout,
    /// Velocidade vertical passou de positiva a negativa.
    Apogee,
    /// Altitude cruzou o nível do solo descendo.
    Impact,
    /// Disparou o gatilho de índice dado (ver [`Sequencer::triggers`]).
    Trigger(usize),
}

/// Gatilho de sequência de voo (abertura de paraquedas, por exemplo).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Trigger {
    /// Em um instante fixo, s.
    Time(f64),
    /// `delay` segundos depois do apogeu.
    AfterApogee {
        /// Atraso após o apogeu, s.
        delay: f64,
    },
    /// Ao cruzar, descendo, a altura `agl` acima do solo, m.
    DescendingThrough {
        /// Altura acima do nível do solo, m.
        agl: f64,
    },
}

/// O que o veículo enxerga do voo para calcular forças.
#[derive(Clone, Copy, Debug)]
pub struct FlightCtx<'a> {
    /// Tempo, s.
    pub t: f64,
    /// Massa atual, kg.
    pub mass: f64,
    /// Altitude acima do elipsoide, m.
    pub altitude: f64,
    /// Estado do ar local.
    pub atmo: AtmoState,
    /// Velocidade em relação ao ar, m/s.
    pub speed: f64,
    /// Número de Mach.
    pub mach: f64,
    /// Pressão dinâmica, Pa.
    pub q_dyn: f64,
    /// Verdadeiro enquanto o veículo está preso ao trilho.
    pub on_rail: bool,
    /// Verdadeiro até `burnout_time()`. O veículo deve decidir se há empuxo por este
    /// campo, não comparando `t` com o fim da queima: o integrador avalia as equações
    /// exatamente no instante da descontinuidade, dos dois lados.
    pub burning: bool,
    /// Instante em que cada gatilho disparou (`None` enquanto não disparou), na ordem
    /// de [`Sequencer::triggers`].
    pub fired: &'a [Option<f64>],
}

/// Linha do tempo do veículo: instantes e condições em que as forças mudam de regime.
/// A integração é reiniciada em cada um deles.
pub trait Sequencer {
    /// Instante do fim da queima, s (0 se não há motor).
    fn burnout_time(&self) -> f64;
    /// Instantes em que a curva de empuxo tem quina (pontos da tabela), s.
    fn breakpoints(&self) -> Vec<f64> {
        Vec::new()
    }
    /// Gatilhos de sequência.
    fn triggers(&self) -> Vec<Trigger> {
        Vec::new()
    }
}

/// Grandezas de navegação derivadas do estado, comuns ao 3-DOF e ao 6-DOF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nav {
    /// Latitude geodésica, rad.
    pub lat: f64,
    /// Longitude fixa ao planeta, rad, em (−π, π].
    pub lon: f64,
    /// Altitude acima do elipsoide, m.
    pub alt: f64,
    /// Módulo da velocidade em relação à atmosfera (que gira com o planeta), m/s.
    pub speed: f64,
    /// Componente vertical (geodésica) dessa velocidade, m/s.
    pub vertical_speed: f64,
    /// Número de Mach (0 no vácuo).
    pub mach: f64,
    /// Pressão dinâmica ½ρV², Pa.
    pub q_dyn: f64,
}

/// Ar e velocidade relativa em um ponto do voo.
pub(crate) struct AirData {
    pub geo: Geodetic,
    pub atmo: AtmoState,
    /// Velocidade em relação à atmosfera, em coordenadas ECI.
    pub v_rel: Vec3,
    pub speed: f64,
    pub mach: f64,
    pub q_dyn: f64,
    pub up: Vec3,
}

/// Fase do voo, constante dentro de cada trecho integrado. As descontinuidades
/// (saída do trilho, fim da queima) são fronteiras de trecho: a equação de movimento
/// nunca é avaliada dos dois lados de uma descontinuidade no mesmo passo.
#[derive(Clone, Debug)]
pub(crate) struct Phase {
    pub on_rail: bool,
    pub burning: bool,
    pub fired: Vec<Option<f64>>,
}

pub(crate) struct World {
    pub planet: Planet,
    pub atmosphere: AtmosphereModel,
    pub launch: Launch,
}

pub(crate) fn v3<const N: usize>(y: &[f64; N], i: usize) -> Vec3 {
    Vec3::new(y[i], y[i + 1], y[i + 2])
}

impl World {
    pub fn air(&self, r: Vec3, v: Vec3) -> AirData {
        let geo = self.planet.ecef_to_geodetic(r);
        let atmo = self.atmosphere.at(geo.h);
        let v_rel = v - self.planet.omega_vec().cross(r);
        let speed = v_rel.norm();
        let mach = if atmo.sound_speed > 0.0 {
            speed / atmo.sound_speed
        } else {
            0.0
        };
        let up = Planet::enu_basis(geo.lat, geo.lon).2;
        AirData {
            geo,
            atmo,
            v_rel,
            speed,
            mach,
            q_dyn: 0.5 * atmo.density * speed * speed,
            up,
        }
    }

    pub fn ctx<'a>(&self, air: &AirData, t: f64, mass: f64, ph: &'a Phase) -> FlightCtx<'a> {
        FlightCtx {
            t,
            mass,
            altitude: air.geo.h,
            atmo: air.atmo,
            speed: air.speed,
            mach: air.mach,
            q_dyn: air.q_dyn,
            on_rail: ph.on_rail,
            burning: ph.burning,
            fired: &ph.fired,
        }
    }

    pub fn nav(&self, t: f64, r: Vec3, v: Vec3) -> Nav {
        let a = self.air(r, v);
        let lon = (a.geo.lon - self.theta(t) + PI).rem_euclid(2.0 * PI) - PI;
        Nav {
            lat: a.geo.lat,
            lon,
            alt: a.geo.h,
            speed: a.speed,
            vertical_speed: a.v_rel.dot(a.up),
            mach: a.mach,
            q_dyn: a.q_dyn,
        }
    }

    fn theta(&self, t: f64) -> f64 {
        self.launch.theta0 + self.planet.omega * t
    }

    /// Eixo do trilho em ECEF: û = cos(el)·(sin(az)·leste + cos(az)·norte) + sin(el)·cima.
    fn axis_ecef(&self) -> Vec3 {
        let s = self.launch.site;
        let (e, n, u) = Planet::enu_basis(s.lat, s.lon);
        let (sel, cel) = self.launch.rail.elevation.sin_cos();
        let (saz, caz) = self.launch.rail.azimuth.sin_cos();
        cel * (saz * e + caz * n) + sel * u
    }

    /// Base do trilho e eixo do trilho em coordenadas ECI no instante `t`.
    pub fn rail_eci(&self, t: f64) -> (Vec3, Vec3) {
        let th = self.theta(t);
        (
            rot_z(self.planet.geodetic_to_ecef(self.launch.site), th),
            rot_z(self.axis_ecef(), th),
        )
    }

    /// Posição e velocidade ECI iniciais.
    pub fn initial_rv(&self) -> (Vec3, Vec3) {
        let (r0, u) = self.rail_eci(0.0);
        (
            r0,
            self.planet.omega_vec().cross(r0) + self.launch.initial_speed * u,
        )
    }

    /// Aceleração inercial de um veículo preso ao trilho (um grau de liberdade).
    ///
    /// No referencial do solo, a_rel = a_inercial − 2ω×v_rel − ω×(ω×r). O trilho anula
    /// a componente de a_rel perpendicular ao eixo û e responde com força normal
    /// N = m·|a_rel⊥|; o atrito cinético vale μ·N contra o movimento:
    /// s̈ = û·a_rel − μ·|a_rel⊥|.
    /// Parado (ṡ ≤ 0), o veículo só parte se s̈ > 0: o batente da base e o atrito
    /// estático impedem que deslize para trás.
    /// Fonte: Goldstein, "Classical Mechanics", 3ª ed., §4.10 (referenciais girantes);
    /// atrito de Coulomb.
    pub fn rail_accel(&self, a_free: Vec3, r: Vec3, v: Vec3, u: Vec3) -> Vec3 {
        let w = self.planet.omega_vec();
        let v_rel = v - w.cross(r);
        let fict = 2.0 * w.cross(v_rel) + w.cross(w.cross(r));
        let a_rel = a_free - fict;
        let a_par = a_rel.dot(u);
        let normal = (a_rel - a_par * u).norm();
        let mut sdd = a_par - self.launch.rail.friction * normal;
        if v_rel.dot(u) <= 0.0 {
            sdd = sdd.max(0.0);
        }
        sdd * u + fict
    }
}

type EventFn<'a, const N: usize> = dyn Fn(f64, &[f64; N]) -> f64 + 'a;
type Rhs<'a, const N: usize> = dyn Fn(&Phase, f64, &[f64; N]) -> [f64; N] + 'a;
type Emit<'a, const N: usize> = dyn FnMut(Option<EventKind>, f64, &[f64; N]) + 'a;

/// Tempos e opções de uma simulação.
pub(crate) struct Schedule {
    pub burnout: f64,
    pub breakpoints: Vec<f64>,
    pub triggers: Vec<Trigger>,
    pub t_max: f64,
    pub output_dt: f64,
    pub ode: Options,
}

impl Schedule {
    pub fn new(seq: &(impl Sequencer + ?Sized), t_max: f64, output_dt: f64, ode: Options) -> Self {
        Self {
            burnout: seq.burnout_time(),
            breakpoints: seq.breakpoints(),
            triggers: seq.triggers(),
            t_max,
            output_dt,
            ode,
        }
    }
}

#[derive(Clone, Copy)]
enum Tag {
    RailExit,
    Apogee,
    Impact,
    Altitude(usize),
}

/// Integra as fases do voo e entrega amostras e eventos.
///
/// O voo é cortado em trechos nos instantes conhecidos (fim da queima, quinas da curva
/// de empuxo, gatilhos por tempo) e nos eventos de estado (saída do trilho, apogeu,
/// gatilhos por altitude, impacto). Dentro de um trecho as equações são suaves.
/// `rhs(fase, t, y)` é a equação de movimento; `emit(kind, t, y)` recebe as amostras
/// na grade de `output_dt` (`kind = None`) e os eventos (`kind = Some`).
pub(crate) fn run_phases<const N: usize>(
    world: &World,
    rhs: &Rhs<'_, N>,
    y0: [f64; N],
    sched: &Schedule,
    emit: &mut Emit<'_, N>,
) -> Result<(), OdeError> {
    let rail_len = world.launch.rail.length;
    let ground = world.launch.ground_h;
    let g_rail = |t: f64, y: &[f64; N]| {
        let (r0, u) = world.rail_eci(t);
        (v3(y, 0) - r0).dot(u) - rail_len
    };
    let g_apogee = |_: f64, y: &[f64; N]| {
        let a = world.air(v3(y, 0), v3(y, 3));
        a.v_rel.dot(a.up)
    };
    let g_impact = |_: f64, y: &[f64; N]| world.planet.ecef_to_geodetic(v3(y, 0)).h - ground;
    let g_alt: Vec<Option<Box<EventFn<'_, N>>>> = sched
        .triggers
        .iter()
        .map(|tr| match *tr {
            Trigger::DescendingThrough { agl } => Some(Box::new(move |_: f64, y: &[f64; N]| {
                world.planet.ecef_to_geodetic(v3(y, 0)).h - ground - agl
            })
                as Box<dyn Fn(f64, &[f64; N]) -> f64>),
            _ => None,
        })
        .collect();

    let (mut t, mut y) = (0.0, y0);
    let mut ph = Phase {
        on_rail: rail_len > 0.0,
        burning: sched.burnout > 0.0,
        fired: vec![None; sched.triggers.len()],
    };
    let mut apogee_t: Option<f64> = None;
    let mut k_out: u64 = 1;
    emit(None, t, &y);
    loop {
        // Fronteiras de tempo pendentes.
        let mut seg_end = sched.t_max;
        if ph.burning {
            if sched.burnout <= t {
                ph.burning = false;
                emit(Some(EventKind::Burnout), t, &y);
                continue;
            }
            seg_end = seg_end.min(sched.burnout);
        }
        let mut fired_now = false;
        for (i, tr) in sched.triggers.iter().enumerate() {
            if ph.fired[i].is_some() {
                continue;
            }
            let due = match *tr {
                Trigger::Time(tt) => Some(tt),
                Trigger::AfterApogee { delay } => apogee_t.map(|ta| ta + delay),
                Trigger::DescendingThrough { .. } => None,
            };
            if let Some(due) = due {
                if due <= t {
                    ph.fired[i] = Some(t);
                    emit(Some(EventKind::Trigger(i)), t, &y);
                    fired_now = true;
                } else {
                    seg_end = seg_end.min(due);
                }
            }
        }
        if fired_now {
            continue;
        }
        if ph.burning {
            for &bp in &sched.breakpoints {
                if bp > t + 1e-12 {
                    seg_end = seg_end.min(bp);
                }
            }
        }
        if t >= sched.t_max {
            break;
        }

        let mut events: Vec<EventSpec<'_, N>> = Vec::new();
        let mut tags: Vec<Tag> = Vec::new();
        if ph.on_rail {
            events.push(EventSpec {
                g: &g_rail,
                direction: Direction::Rising,
                terminal: true,
            });
            tags.push(Tag::RailExit);
        } else {
            if apogee_t.is_none() {
                events.push(EventSpec {
                    g: &g_apogee,
                    direction: Direction::Falling,
                    terminal: true,
                });
                tags.push(Tag::Apogee);
            }
            events.push(EventSpec {
                g: &g_impact,
                direction: Direction::Falling,
                terminal: true,
            });
            tags.push(Tag::Impact);
            for (i, g) in g_alt.iter().enumerate() {
                if let (Some(g), None) = (g, ph.fired[i]) {
                    events.push(EventSpec {
                        g: g.as_ref(),
                        direction: Direction::Falling,
                        terminal: true,
                    });
                    tags.push(Tag::Altitude(i));
                }
            }
        }
        let f = |t: f64, y: &[f64; N]| rhs(&ph, t, y);
        let mut grid: Vec<(f64, [f64; N])> = Vec::new();
        let out = integrate(&f, t, y, seg_end, &events, &sched.ode, |step, t_valid| {
            while (k_out as f64) * sched.output_dt <= t_valid {
                let to = k_out as f64 * sched.output_dt;
                grid.push((to, step.dense(to)));
                k_out += 1;
            }
        })?;
        for (to, yo) in &grid {
            emit(None, *to, yo);
        }
        (t, y) = (out.t, out.y);
        if let Some(idx) = out.terminated_by {
            match tags[idx] {
                Tag::RailExit => {
                    ph.on_rail = false;
                    emit(Some(EventKind::RailExit), t, &y);
                }
                Tag::Apogee => {
                    apogee_t = Some(t);
                    emit(Some(EventKind::Apogee), t, &y);
                }
                Tag::Altitude(i) => {
                    ph.fired[i] = Some(t);
                    emit(Some(EventKind::Trigger(i)), t, &y);
                }
                Tag::Impact => {
                    emit(Some(EventKind::Impact), t, &y);
                    break;
                }
            }
        }
    }
    emit(None, t, &y);
    Ok(())
}
