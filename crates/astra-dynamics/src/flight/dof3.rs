use super::{run_phases, v3, EventKind, Launch, Nav, World};
use crate::ode::{OdeError, Options};
use astra_env::{AtmosphereModel, Planet};
use astra_math::Vec3;

/// Veículo visto como massa pontual: só empuxo, vazão e arrasto.
pub trait PointVehicle {
    /// Empuxo (N) e vazão mássica de propelente (kg/s, ≥ 0) no instante `t`,
    /// para a pressão ambiente `p_amb` (Pa). Só é chamado com `t ≤ burnout_time()`;
    /// depois disso o simulador assume empuxo e vazão nulos.
    fn thrust(&self, t: f64, p_amb: f64) -> (f64, f64);
    /// Área de arrasto Cd·A (m²) no número de Mach dado.
    fn drag_area(&self, mach: f64) -> f64;
    /// Instante do fim da queima, s (0 se não há motor). A integração é reiniciada
    /// nesse instante para não atravessar a descontinuidade de empuxo.
    fn burnout_time(&self) -> f64;
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

impl PointVehicle for ConstantVehicle {
    /// F = F_vác − p_amb·A_e, forma equivalente a F = ṁ·v_e + (p_e − p_amb)·A_e.
    /// Fonte: Sutton & Biblarz, "Rocket Propulsion Elements", 9ª ed., eq. 2-13.
    fn thrust(&self, t: f64, p_amb: f64) -> (f64, f64) {
        if t <= self.burn_time {
            (self.thrust_vac - p_amb * self.exit_area, self.mdot)
        } else {
            (0.0, 0.0)
        }
    }
    fn drag_area(&self, _mach: f64) -> f64 {
        self.cd_a
    }
    fn burnout_time(&self) -> f64 {
        self.burn_time
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
    };
    let rhs = |ph: super::Phase, t: f64, y: &[f64; 7]| -> [f64; 7] {
        let (r, v, m) = (v3(y, 0), v3(y, 3), y[6]);
        let air = world.air(r, v);
        let (_, axis) = world.rail_eci(t);
        let dir = if ph.on_rail || air.speed < 1e-3 {
            axis
        } else {
            air.v_rel / air.speed
        };
        let (thrust, mdot) = if ph.burning {
            cfg.vehicle.thrust(t, air.atmo.pressure)
        } else {
            (0.0, 0.0)
        };
        let drag =
            -0.5 * air.atmo.density * air.speed * cfg.vehicle.drag_area(air.mach) * air.v_rel;
        let a_free = world.planet.gravity(r) + (thrust * dir + drag) / m;
        let a = if ph.on_rail {
            world.rail_accel(a_free, r, v, axis)
        } else {
            a_free
        };
        [v.x, v.y, v.z, a.x, a.y, a.z, -mdot]
    };
    let (r0, v0) = world.initial_rv();
    let y0 = [r0.x, r0.y, r0.z, v0.x, v0.y, v0.z, cfg.m0];
    let mut res = Result3::default();
    run_phases(
        &world,
        &rhs,
        y0,
        &super::Schedule {
            burnout: cfg.vehicle.burnout_time(),
            t_max: cfg.t_max,
            output_dt: cfg.output_dt,
            ode: cfg.ode,
        },
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
    Ok(res)
}
