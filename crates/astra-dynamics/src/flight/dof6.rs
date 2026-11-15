use super::{
    run_phases, v3, EventKind, FlightCtx, Launch, Nav, Phase, Schedule, SequenceState, Sequencer,
    Start, World,
};
use crate::ode::{OdeError, Options};
use astra_env::{AtmosphereModel, Planet};
use astra_math::{Mat3, Quat, Vec3};

/// Forças e momentos não gravitacionais sobre o veículo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loads {
    /// Força resultante (empuxo + aerodinâmica) nos eixos do corpo, N.
    pub force_body: Vec3,
    /// Momento resultante em torno do CG, nos eixos do corpo, N·m.
    pub moment_body: Vec3,
    /// Vazão mássica de propelente, kg/s (≥ 0).
    pub mdot: f64,
}

/// Propriedades de inércia instantâneas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassProps {
    /// Tensor de inércia em torno do CG, eixos do corpo, kg·m² (positivo definido).
    pub inertia: Mat3,
    /// Derivada temporal do tensor de inércia, kg·m²/s.
    pub inertia_rate: Mat3,
}

/// Veículo rígido para o 6-DOF. Eixo x do corpo = eixo longitudinal, apontando para o nariz.
pub trait RigidVehicle: Sequencer {
    /// Forças, momentos e vazão. `v_air_body` é a velocidade em relação ao ar e
    /// `omega_body` a velocidade angular inercial, ambas nos eixos do corpo.
    fn loads(&self, ctx: &FlightCtx<'_>, v_air_body: Vec3, omega_body: Vec3) -> Loads;
    /// Inércia para a massa e o instante dados.
    fn mass_props(&self, t: f64, mass: f64) -> MassProps;
    /// Margem `index` de limite físico (0 ≤ index < `margin_count()`): positiva enquanto
    /// o limite é respeitado, negativa quando é ultrapassado. Use uma grandeza contínua
    /// no tempo, como (capacidade − carga)/capacidade, para o evento ser localizado.
    fn margin(&self, ctx: &FlightCtx<'_>, v_air_body: Vec3, omega_body: Vec3, index: usize) -> f64 {
        let _ = (ctx, v_air_body, omega_body, index);
        1.0
    }
}

/// Estado inicial de um trecho que não começa no lançador (corpo recém-separado).
#[derive(Clone, Debug, PartialEq)]
pub struct Start6 {
    /// Instante, s.
    pub t: f64,
    /// Posição ECI, m.
    pub r: Vec3,
    /// Velocidade ECI, m/s.
    pub v: Vec3,
    /// Atitude (corpo → ECI).
    pub q: Quat,
    /// Velocidade angular inercial nos eixos do corpo, rad/s.
    pub omega: Vec3,
    /// Massa, kg.
    pub mass: f64,
    /// Motor ainda aceso.
    pub burning: bool,
    /// Instante de disparo de cada gatilho do novo veículo.
    pub fired: Vec<Option<f64>>,
    /// Instante do apogeu, se já ocorreu.
    pub apogee: Option<f64>,
}

/// Entrada de uma simulação 6-DOF.
pub struct Config6<'a> {
    /// Corpo central.
    pub planet: Planet,
    /// Modelo de atmosfera.
    pub atmosphere: AtmosphereModel,
    /// Veículo.
    pub vehicle: &'a dyn RigidVehicle,
    /// Condições de lançamento. O eixo x do corpo começa alinhado ao trilho.
    pub launch: Launch,
    /// Massa inicial, kg.
    pub m0: f64,
    /// Velocidade angular inicial em relação ao solo, eixos do corpo, rad/s.
    pub initial_omega_body: Vec3,
    /// Duração máxima simulada, s.
    pub t_max: f64,
    /// Intervalo entre amostras de saída, s.
    pub output_dt: f64,
    /// Opções do integrador.
    pub ode: Options,
    /// Vento.
    pub wind: astra_env::Wind,
    /// Começar de um estado dado em vez do lançador (`launch` continua definindo o solo).
    pub start: Option<Start6>,
}

/// Amostra de telemetria 6-DOF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample6 {
    /// Tempo desde o início, s.
    pub t: f64,
    /// Posição ECI, m.
    pub r: Vec3,
    /// Velocidade ECI, m/s.
    pub v: Vec3,
    /// Atitude (corpo → ECI), unitário.
    pub q: Quat,
    /// Velocidade angular inercial nos eixos do corpo, rad/s.
    pub omega: Vec3,
    /// Massa, kg.
    pub mass: f64,
    /// Ângulo de ataque total (entre o eixo x do corpo e a velocidade relativa ao ar), rad.
    pub alpha: f64,
    /// Grandezas de navegação.
    pub nav: Nav,
}

/// Saída de [`simulate6`].
#[derive(Clone, Debug, Default)]
pub struct Result6 {
    /// Amostras em ordem de tempo.
    pub samples: Vec<Sample6>,
    /// Eventos em ordem de ocorrência.
    pub events: Vec<(EventKind, Sample6)>,
    /// Estado da sequência no fim (para continuar após uma quebra).
    pub end: Option<SequenceState>,
}

impl Result6 {
    /// Última amostra (estado final).
    #[must_use]
    pub fn last(&self) -> Option<&Sample6> {
        self.samples.last()
    }

    /// Primeiro evento do tipo dado, se ocorreu.
    #[must_use]
    pub fn event(&self, kind: EventKind) -> Option<&Sample6> {
        self.events.iter().find(|e| e.0 == kind).map(|e| &e.1)
    }
}

fn quat(y: &[f64; 14]) -> Quat {
    Quat::new(y[6], y[7], y[8], y[9])
}

/// Ganho (1/s) do termo que traz |q| de volta a 1 durante a integração.
const QUAT_NORM_GAIN: f64 = 1.0;

/// Simula o voo de um corpo rígido de massa variável.
///
/// Estado: [r (3), v (3), q (4), ω (3), m]; r e v em ECI, q do corpo para o ECI,
/// ω inercial nos eixos do corpo. Equações:
/// - ṙ = v; v̇ = g(r) + q·F_corpo/m;
/// - q̇ = ½·q⊗[0, ω] + k·(1 − |q|²)·q (o segundo termo só corrige a deriva de norma);
/// - I·ω̇ = M − ω×(I·ω) − İ·ω;
/// - ṁ = −ṁ_prop.
///
/// Fonte: Stevens, Lewis & Johnson, "Aircraft Control and Simulation", 3ª ed., eqs.
/// 1.7-5, 1.8-15 e 1.8-16; termo İ·ω de massa variável conforme Zipfel, 2ª ed., §6.3.
/// No trilho, a translação segue [`World::rail_accel`] e ω̇ = 0 (atitude presa ao solo).
///
/// # Errors
/// Propaga falhas do integrador ([`OdeError`]); um tensor de inércia singular aparece
/// como [`OdeError::NonFinite`].
pub fn simulate6(cfg: &Config6<'_>) -> Result<Result6, OdeError> {
    let world = World {
        planet: cfg.planet,
        atmosphere: cfg.atmosphere,
        launch: cfg.launch,
        wind: cfg.wind.clone(),
    };
    let rhs = |ph: &Phase, t: f64, y: &[f64; 14]| -> [f64; 14] {
        let (r, v, q, w, m) = (v3(y, 0), v3(y, 3), quat(y), v3(y, 10), y[13]);
        let qn = q.normalized();
        let air = world.air(t, r, v);
        let ctx = world.ctx(&air, t, m, ph);
        let loads = cfg.vehicle.loads(&ctx, qn.conj().rotate(air.v_rel), w);
        let a_free = world.planet.gravity(r) + qn.rotate(loads.force_body) / m;
        let (a, wdot) = if ph.on_rail {
            (
                world.rail_accel(a_free, r, v, world.rail_eci(t).1),
                Vec3::ZERO,
            )
        } else {
            let mp = cfg.vehicle.mass_props(t, m);
            let rhs_w = loads.moment_body - w.cross(mp.inertia * w) - mp.inertia_rate * w;
            let wdot = mp
                .inertia
                .inverse()
                .map_or(Vec3::new(f64::NAN, f64::NAN, f64::NAN), |inv| inv * rhs_w);
            (a_free, wdot)
        };
        let qd = q.derivative(w);
        let fix = QUAT_NORM_GAIN * (1.0 - q.norm() * q.norm());
        [
            v.x,
            v.y,
            v.z,
            a.x,
            a.y,
            a.z,
            qd.w + fix * q.w,
            qd.x + fix * q.x,
            qd.y + fix * q.y,
            qd.z + fix * q.z,
            wdot.x,
            wdot.y,
            wdot.z,
            -loads.mdot,
        ]
    };

    let margin = |ph: &Phase, t: f64, y: &[f64; 14], i: usize| -> f64 {
        let (r, v, q, w, m) = (v3(y, 0), v3(y, 3), quat(y).normalized(), v3(y, 10), y[13]);
        let air = world.air(t, r, v);
        let ctx = world.ctx(&air, t, m, ph);
        cfg.vehicle.margin(&ctx, q.conj().rotate(air.v_rel), w, i)
    };
    let n_margins = cfg.vehicle.margin_count();
    let (r0, v0) = world.initial_rv();
    // Atitude inicial: x do corpo no eixo do trilho; y horizontal; z completa o triedro.
    let (_, axis) = world.rail_eci(0.0);
    let up = Planet::enu_basis(cfg.launch.site.lat, cfg.launch.site.lon + cfg.launch.theta0).2;
    let east = Planet::enu_basis(cfg.launch.site.lat, cfg.launch.site.lon + cfg.launch.theta0).0;
    // Eixo y horizontal. Com trilho quase vertical, up × eixo é quase nulo e perde a
    // ortogonalidade por arredondamento: nesse caso usa o leste, e em todo caso
    // ortogonaliza contra o eixo (Gram-Schmidt) antes de montar a matriz.
    let cross = up.cross(axis);
    let seed = if cross.norm() > 1e-6 { cross } else { east };
    let yb = (seed - axis * seed.dot(axis)).normalized().unwrap_or(east);
    let q0 = Quat::from_mat3(Mat3::from_cols(axis, yb, axis.cross(yb)));
    let w0 = q0.conj().rotate(cfg.planet.omega_vec()) + cfg.initial_omega_body;
    let start = match &cfg.start {
        None => Start {
            t: 0.0,
            y: [
                r0.x, r0.y, r0.z, v0.x, v0.y, v0.z, q0.w, q0.x, q0.y, q0.z, w0.x, w0.y, w0.z,
                cfg.m0,
            ],
            on_rail: cfg.launch.rail.length > 0.0,
            burning: cfg.vehicle.burnout_time() > 0.0,
            fired: Vec::new(),
            apogee: None,
        },
        Some(s) => Start {
            t: s.t,
            y: [
                s.r.x, s.r.y, s.r.z, s.v.x, s.v.y, s.v.z, s.q.w, s.q.x, s.q.y, s.q.z, s.omega.x,
                s.omega.y, s.omega.z, s.mass,
            ],
            on_rail: false,
            burning: s.burning && s.t < cfg.vehicle.burnout_time(),
            fired: s.fired.clone(),
            apogee: s.apogee,
        },
    };

    let mut res = Result6::default();
    let (end, _) = run_phases(
        &world,
        &rhs,
        start,
        &Schedule::new(cfg.vehicle, cfg.t_max, cfg.output_dt, cfg.ode),
        (n_margins > 0).then_some((&margin as &super::Margins<'_, 14>, n_margins)),
        &mut |kind: Option<EventKind>, t, y| {
            let (r, v, q) = (v3(y, 0), v3(y, 3), quat(y).normalized());
            let nav = world.nav(t, r, v);
            let vb = q.conj().rotate(world.air(t, r, v).v_rel);
            let alpha = if nav.speed > 1e-6 {
                vb.y.hypot(vb.z).atan2(vb.x)
            } else {
                0.0
            };
            let s = Sample6 {
                t,
                r,
                v,
                q,
                omega: v3(y, 10),
                mass: y[13],
                alpha,
                nav,
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
