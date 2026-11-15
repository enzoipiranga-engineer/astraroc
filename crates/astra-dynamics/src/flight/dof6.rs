use super::{run_phases, v3, EventKind, Launch, Nav, World};
use crate::ode::{OdeError, Options};
use astra_env::{AtmoState, AtmosphereModel, Planet};
use astra_math::{Mat3, Quat, Vec3};

/// O que o veículo enxerga do voo para calcular forças e momentos.
#[derive(Clone, Copy, Debug)]
pub struct Kinematics6 {
    /// Tempo, s.
    pub t: f64,
    /// Massa atual, kg.
    pub mass: f64,
    /// Altitude acima do elipsoide, m.
    pub altitude: f64,
    /// Estado do ar local.
    pub atmo: AtmoState,
    /// Velocidade em relação ao ar, nos eixos do corpo, m/s.
    pub v_air_body: Vec3,
    /// Velocidade angular inercial, nos eixos do corpo, rad/s.
    pub omega_body: Vec3,
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
}

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
pub trait RigidVehicle {
    /// Forças, momentos e vazão.
    fn loads(&self, k: &Kinematics6) -> Loads;
    /// Inércia para a massa e o instante dados.
    fn mass_props(&self, t: f64, mass: f64) -> MassProps;
    /// Instante do fim da queima, s (0 se não há motor).
    fn burnout_time(&self) -> f64;
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
}

impl Result6 {
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
    };
    let rhs = |ph: super::Phase, t: f64, y: &[f64; 14]| -> [f64; 14] {
        let (r, v, q, w, m) = (v3(y, 0), v3(y, 3), quat(y), v3(y, 10), y[13]);
        let qn = q.normalized();
        let air = world.air(r, v);
        let kin = Kinematics6 {
            t,
            mass: m,
            altitude: air.geo.h,
            atmo: air.atmo,
            v_air_body: qn.conj().rotate(air.v_rel),
            omega_body: w,
            mach: air.mach,
            q_dyn: air.q_dyn,
            on_rail: ph.on_rail,
            burning: ph.burning,
        };
        let loads = cfg.vehicle.loads(&kin);
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

    let (r0, v0) = world.initial_rv();
    // Atitude inicial: x do corpo no eixo do trilho; y horizontal; z completa o triedro.
    let (_, axis) = world.rail_eci(0.0);
    let up = Planet::enu_basis(cfg.launch.site.lat, cfg.launch.site.lon + cfg.launch.theta0).2;
    let east = Planet::enu_basis(cfg.launch.site.lat, cfg.launch.site.lon + cfg.launch.theta0).0;
    let yb = up.cross(axis).normalized().unwrap_or(east);
    let q0 = Quat::from_mat3(Mat3::from_cols(axis, yb, axis.cross(yb)));
    let w0 = q0.conj().rotate(cfg.planet.omega_vec()) + cfg.initial_omega_body;
    let y0 = [
        r0.x, r0.y, r0.z, v0.x, v0.y, v0.z, q0.w, q0.x, q0.y, q0.z, w0.x, w0.y, w0.z, cfg.m0,
    ];

    let mut res = Result6::default();
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
        &mut |kind: Option<EventKind>, t, y| {
            let (r, v, q) = (v3(y, 0), v3(y, 3), quat(y).normalized());
            let nav = world.nav(t, r, v);
            let vb = q.conj().rotate(world.air(r, v).v_rel);
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
    Ok(res)
}
