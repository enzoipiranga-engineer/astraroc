use super::{
    run_phases, v3, EventKind, FlightCtx, Launch, Nav, Phase, Schedule, Sequencer, Start, World,
};
use crate::ode::{OdeError, Options};
use astra_env::{AtmosphereModel, Planet};
use astra_math::Vec3;

/// Veículo visto como massa pontual: só empuxo, vazão e arrasto.
pub trait PointVehicle: Sequencer {
    /// Empuxo (N) e vazão mássica de propelente (kg/s, ≥ 0). Só é chamado enquanto
    /// `ctx.burning`; depois o simulador assume empuxo e vazão nulos.
    fn thrust(&self, ctx: &FlightCtx<'_>) -> (f64, f64);
    /// Área de arrasto Cd·A (m²).
    fn drag_area(&self, ctx: &FlightCtx<'_>) -> f64;
    /// Margem `index` de limite físico, como em [`super::RigidVehicle::margin`].
    fn margin(&self, ctx: &FlightCtx<'_>, index: usize) -> f64 {
        let _ = (ctx, index);
        1.0
    }
}

/// Motor de empuxo de vácuo e vazão constantes, com arrasto de Cd·A constante.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstantVehicle {
    /// Empuxo no vácuo, N.
    pub thrust_vac: f64,
    /// Área de saída da tubeira, m².
    pub exit_area: f64,
    /// Vazão mássica, kg/s.
    pub mdot: f64,
    /// Duração da queima, s.
    pub burn_time: f64,
    /// Área de arrasto Cd·A, m².
    pub cd_a: f64,
}

impl ConstantVehicle {
    /// Empuxo sob pressão ambiente `p_amb` (Pa): F = F_vác − p_amb·A_e, forma
    /// equivalente a F = ṁ·v_e + (p_e − p_amb)·A_e.
    /// Fonte: Sutton & Biblarz, "Rocket Propulsion Elements", 9ª ed., cap. 2 (equação do empuxo).
    #[must_use]
    pub fn thrust_at(&self, p_amb: f64) -> f64 {
        self.thrust_vac - p_amb * self.exit_area
    }
}

impl Sequencer for ConstantVehicle {
    fn burnout_time(&self) -> f64 {
        self.burn_time
    }
}

impl PointVehicle for ConstantVehicle {
    fn thrust(&self, ctx: &FlightCtx<'_>) -> (f64, f64) {
        (self.thrust_at(ctx.atmo.pressure), self.mdot)
    }
    fn drag_area(&self, _: &FlightCtx<'_>) -> f64 {
        self.cd_a
    }
}

/// Entrada de uma simulação 3-DOF.
pub struct Config3<'a> {
    /// Corpo central.
    pub planet: Planet,
    /// Modelo de atmosfera.
    pub atmosphere: AtmosphereModel,
    /// Veículo.
    pub vehicle: &'a dyn PointVehicle,
    /// Condições de lançamento.
    pub launch: Launch,
    /// Massa inicial, kg.
    pub m0: f64,
    /// Duração máxima simulada, s.
    pub t_max: f64,
    /// Intervalo entre amostras de saída, s.
    pub output_dt: f64,
    /// Opções do integrador.
    pub ode: Options,
    /// Vento.
    pub wind: astra_env::Wind,
    /// Começar de um estado dado em vez do lançador (`launch` continua definindo o solo).
    pub start: Option<Start3>,
}

/// Estado inicial de um trecho 3-DOF que não começa no lançador.
#[derive(Clone, Debug, PartialEq)]
pub struct Start3 {
    /// Instante, s.
    pub t: f64,
    /// Posição ECI, m.
    pub r: Vec3,
    /// Velocidade ECI, m/s.
    pub v: Vec3,
    /// Massa, kg.
    pub mass: f64,
    /// Motor ainda aceso.
    pub burning: bool,
    /// Instante de disparo de cada gatilho.
    pub fired: Vec<Option<f64>>,
    /// Instante do apogeu, se já ocorreu.
    pub apogee: Option<f64>,
}

/// Amostra de telemetria 3-DOF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample3 {
    /// Tempo desde o início, s.
    pub t: f64,
    /// Posição ECI, m.
    pub r: Vec3,
    /// Velocidade ECI, m/s.
    pub v: Vec3,
    /// Massa, kg.
    pub mass: f64,
    /// Grandezas de navegação.
    pub nav: Nav,
}

/// Saída de [`simulate3`]: amostras na grade de tempo (mais a final) e eventos.
#[derive(Clone, Debug, Default)]
pub struct Result3 {
    /// Amostras em ordem de tempo.
    pub samples: Vec<Sample3>,
    /// Eventos em ordem de ocorrência.
    pub events: Vec<(EventKind, Sample3)>,
    /// Estado da sequência no fim.
    pub end: Option<super::SequenceState>,
}

impl Result3 {
    /// Primeiro evento do tipo dado, se ocorreu.
    #[must_use]
    pub fn event(&self, kind: EventKind) -> Option<&Sample3> {
        self.events.iter().find(|e| e.0 == kind).map(|e| &e.1)
    }
}

/// Simula o voo de uma massa pontual.
///
/// Estado: [r (3), v (3), m] em ECI. Equações:
/// ṙ = v; v̇ = g(r) + (F·d̂ − ½ρ|v_rel|·CdA·v_rel)/m; ṁ = −ṁ_prop,
/// com v_rel = v − ω×r (ar girando com o planeta).
/// Direção do empuxo d̂: eixo do trilho enquanto preso ou quase parado; depois,
/// a da velocidade relativa (ângulo de ataque nulo, "gravity turn").
/// Fonte: Zipfel, "Modeling and Simulation of Aerospace Vehicle Dynamics", 2ª ed., cap. 8.
///
/// # Errors
/// Propaga falhas do integrador ([`OdeError`]).
pub fn simulate3(cfg: &Config3<'_>) -> Result<Result3, OdeError> {
    let world = World {
        planet: cfg.planet,
        atmosphere: cfg.atmosphere,
        launch: cfg.launch,
        wind: cfg.wind.clone(),
    };
    let rhs = |ph: &Phase, t: f64, y: &[f64; 7]| -> [f64; 7] {
        let (r, v, m) = (v3(y, 0), v3(y, 3), y[6]);
        let air = world.air(t, r, v);
        let (_, axis) = world.rail_eci(t);
        let dir = if ph.on_rail || air.speed < 1e-3 {
            axis
        } else {
            air.v_rel / air.speed
        };
        let ctx = world.ctx(&air, t, m, ph);
        let (thrust, mdot) = if ph.burning {
            cfg.vehicle.thrust(&ctx)
        } else {
            (0.0, 0.0)
        };
        let drag = -0.5 * air.atmo.density * air.speed * cfg.vehicle.drag_area(&ctx) * air.v_rel;
        let a_free = world.planet.gravity(r) + (thrust * dir + drag) / m;
        let a = if ph.on_rail {
            world.rail_accel(a_free, r, v, axis)
        } else {
            a_free
        };
        [v.x, v.y, v.z, a.x, a.y, a.z, -mdot]
    };
    let (r0, v0) = world.initial_rv();
    let margin = |ph: &Phase, t: f64, y: &[f64; 7], i: usize| -> f64 {
        let air = world.air(t, v3(y, 0), v3(y, 3));
        cfg.vehicle.margin(&world.ctx(&air, t, y[6], ph), i)
    };
    let n_margins = cfg.vehicle.margin_count();
    let start = match &cfg.start {
        None => Start {
            t: 0.0,
            y: [r0.x, r0.y, r0.z, v0.x, v0.y, v0.z, cfg.m0],
            on_rail: cfg.launch.rail.length > 0.0,
            burning: cfg.vehicle.burnout_time() > 0.0,
            fired: Vec::new(),
            apogee: None,
        },
        Some(s) => Start {
            t: s.t,
            y: [s.r.x, s.r.y, s.r.z, s.v.x, s.v.y, s.v.z, s.mass],
            on_rail: false,
            burning: s.burning && s.t < cfg.vehicle.burnout_time(),
            fired: s.fired.clone(),
            apogee: s.apogee,
        },
    };
    let mut res = Result3::default();
    let (end, _) = run_phases(
        &world,
        &rhs,
        start,
        &Schedule::new(cfg.vehicle, cfg.t_max, cfg.output_dt, cfg.ode),
        (n_margins > 0).then_some((&margin as &super::Margins<'_, 7>, n_margins)),
        &mut |kind, t, y| {
            let (r, v) = (v3(y, 0), v3(y, 3));
            let s = Sample3 {
                t,
                r,
                v,
                mass: y[6],
                nav: world.nav(t, r, v),
            };
            match kind {
                Some(k) => res.events.push((k, s)),
                None => res.samples.push(s),
            }
        },
    )?;
    res.end = Some(end);
    Ok(res)
}
