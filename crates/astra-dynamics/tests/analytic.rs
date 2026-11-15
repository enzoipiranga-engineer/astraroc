//! Testes da Fase 1 contra soluções analíticas.

use astra_dynamics::flight::*;
use astra_dynamics::ode::{integrate, Options};
use astra_env::{AtmosphereModel, Geodetic, Planet};
use astra_math::{Mat3, Vec3};
use core::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

const NO_MOTOR: ConstantVehicle = ConstantVehicle {
    thrust_vac: 0.0,
    exit_area: 0.0,
    mdot: 0.0,
    burn_time: 0.0,
    cd_a: 0.0,
};

fn tight() -> Options {
    Options {
        rtol: 1e-11,
        atol: 1e-9,
        h_max: 5.0,
        ..Options::default()
    }
}

fn launch(elevation: f64, azimuth: f64, rail: f64, speed: f64) -> Launch {
    Launch {
        site: Geodetic {
            lat: 0.0,
            lon: 0.0,
            h: 0.0,
        },
        ground_h: 0.0,
        rail: Rail {
            length: rail,
            elevation,
            azimuth,
            friction: 0.0,
        },
        initial_speed: speed,
        theta0: 0.0,
    }
}

fn cfg3<'a>(
    planet: Planet,
    vehicle: &'a ConstantVehicle,
    launch: Launch,
    m0: f64,
    t_max: f64,
) -> Config3<'a> {
    Config3 {
        planet,
        atmosphere: AtmosphereModel::Vacuum,
        vehicle,
        launch,
        m0,
        t_max,
        output_dt: 0.1,
        ode: tight(),
        start: None,
    }
}

/// Tiro vertical no vácuo, gravidade central: o apogeu sai da conservação de energia,
/// v²/2 − μ/r0 = −μ/r_máx.
#[test]
fn vertical_shot_reaches_energy_apogee() {
    let p = Planet::EARTH.spherical().non_rotating();
    let v0 = 1000.0;
    let r = simulate3(&cfg3(
        p,
        &NO_MOTOR,
        launch(FRAC_PI_2, 0.0, 0.0, v0),
        1.0,
        400.0,
    ))
    .unwrap();
    let r_max = 1.0 / (1.0 / p.re - v0 * v0 / (2.0 * p.mu));
    let apo = r.event(EventKind::Apogee).unwrap();
    assert!(
        (apo.nav.alt - (r_max - p.re)).abs() < 1e-3,
        "apogeu {} vs {}",
        apo.nav.alt,
        r_max - p.re
    );
    let imp = r.event(EventKind::Impact).unwrap();
    assert!(
        (imp.nav.speed - v0).abs() < 1e-6,
        "volta ao solo com a mesma velocidade"
    );
    assert!(
        (imp.t - 2.0 * apo.t).abs() < 1e-6,
        "subida e descida simétricas"
    );
}

/// Tiro curto a 45° no vácuo: alcance v²·sin(2θ)/g e tempo 2v·sinθ/g.
/// A curvatura e o gradiente de gravidade entram na ordem de (alcance/raio) ~ 1e-5.
#[test]
fn short_ballistic_shot_matches_flat_earth_formulas() {
    let p = Planet::EARTH.spherical().non_rotating();
    let (v0, g) = (30.0, p.mu / (p.re * p.re));
    let r = simulate3(&cfg3(
        p,
        &NO_MOTOR,
        launch(FRAC_PI_4, FRAC_PI_2, 0.0, v0),
        1.0,
        60.0,
    ))
    .unwrap();
    let imp = r.event(EventKind::Impact).unwrap();
    let range = imp.nav.lon * p.re;
    assert!(
        (range / (v0 * v0 / g) - 1.0).abs() < 5e-5,
        "alcance {range}"
    );
    assert!(
        (imp.t / (2.0 * v0 * FRAC_PI_4.sin() / g) - 1.0).abs() < 5e-5,
        "tempo {}",
        imp.t
    );
    assert!(imp.nav.lat.abs() < 1e-12, "tiro para leste fica no equador");
}

/// Equação do foguete sem gravidade nem arrasto: Δv = v_e·ln(m0/mf).
/// Fonte: Tsiolkovsky (1903); Sutton & Biblarz, eq. 4-6.
#[test]
fn tsiolkovsky_delta_v() {
    let p = Planet {
        mu: 0.0,
        ..Planet::EARTH.spherical().non_rotating()
    };
    let v = ConstantVehicle {
        thrust_vac: 3000.0,
        exit_area: 0.0,
        mdot: 1.5,
        burn_time: 40.0,
        cd_a: 0.0,
    };
    let r = simulate3(&cfg3(p, &v, launch(FRAC_PI_2, 0.0, 0.0, 0.0), 100.0, 45.0)).unwrap();
    let bo = r.event(EventKind::Burnout).unwrap();
    let dv = (3000.0 / 1.5) * (100.0_f64 / 40.0).ln();
    assert!((bo.mass - 40.0).abs() < 1e-9);
    assert!(
        (bo.nav.speed - dv).abs() < 1e-6,
        "Δv {} vs {dv}",
        bo.nav.speed
    );
    let last = r.samples.last().unwrap();
    assert!(
        (last.nav.speed - dv).abs() < 1e-6,
        "sem forças após a queima"
    );
}

/// Empuxo cai com a pressão ambiente: F = F_vác − p_amb·A_e.
#[test]
fn thrust_drops_with_ambient_pressure() {
    let v = ConstantVehicle {
        thrust_vac: 1000.0,
        exit_area: 0.002,
        mdot: 0.5,
        burn_time: 1.0,
        cd_a: 0.0,
    };
    assert_eq!(v.thrust_at(0.0), 1000.0);
    assert!((v.thrust_at(101_325.0) - (1000.0 - 202.65)).abs() < 1e-9);
}

/// Trilho inclinado com atrito, massa constante: a = F/m − g·sinθ − μ·g·cosθ e
/// velocidade de saída v = √(2·a·L).
#[test]
fn rail_exit_speed_with_friction() {
    let p = Planet::EARTH.spherical().non_rotating();
    let (el, mu, len, f, m) = (60.0_f64.to_radians(), 0.1, 5.0, 300.0, 10.0);
    let v = ConstantVehicle {
        thrust_vac: f,
        exit_area: 0.0,
        mdot: 0.0,
        burn_time: 10.0,
        cd_a: 0.0,
    };
    let mut l = launch(el, 0.3, len, 0.0);
    l.rail.friction = mu;
    let r = simulate3(&cfg3(p, &v, l, m, 3.0)).unwrap();
    let g = p.mu / (p.re * p.re);
    let a = f / m - g * el.sin() - mu * g * el.cos();
    let ex = r.event(EventKind::RailExit).unwrap();
    assert!(
        (ex.nav.speed / (2.0 * a * len).sqrt() - 1.0).abs() < 1e-5,
        "saída {}",
        ex.nav.speed
    );
    assert!((ex.t / (2.0 * len / a).sqrt() - 1.0).abs() < 1e-5);
    assert!((ex.nav.alt / (len * el.sin()) - 1.0).abs() < 1e-5);
}

/// Empuxo menor que o peso: o foguete não sai da base, e na Terra girando continua
/// parado em relação ao solo.
#[test]
fn underpowered_rocket_stays_on_the_pad() {
    let v = ConstantVehicle {
        thrust_vac: 50.0,
        exit_area: 0.0,
        mdot: 0.0,
        burn_time: 5.0,
        cd_a: 0.0,
    };
    let mut l = launch(FRAC_PI_2, 0.0, 2.0, 0.0);
    l.site = Geodetic {
        lat: -0.4,
        lon: 0.9,
        h: 120.0,
    };
    l.ground_h = 120.0;
    let r = simulate3(&cfg3(Planet::EARTH, &v, l, 10.0, 600.0)).unwrap();
    assert!(r.events.iter().all(|e| e.0 == EventKind::Burnout));
    let s = r.samples.last().unwrap();
    // O vínculo é imposto na aceleração; a posição acumula só o erro de integração
    // (tolerância relativa 1e-11 sobre um raio de 6,4e6 m ≈ 1e-4 m em 10 min).
    assert!(
        (s.nav.alt - 120.0).abs() < 1e-3 && s.nav.speed < 1e-6,
        "alt {} v {}",
        s.nav.alt,
        s.nav.speed
    );
    assert!((s.nav.lat + 0.4).abs() < 1e-9 && (s.nav.lon - 0.9).abs() < 1e-9);
}

/// Desvio para leste de um corpo solto do repouso no equador:
/// d = (1/3)·ω·g·t³ com t = √(2h/g) e g a gravidade efetiva local.
/// Fonte: Goldstein, "Classical Mechanics", 3ª ed., §4.10.
#[test]
fn coriolis_deflects_a_dropped_body_eastward() {
    let p = Planet::EARTH.spherical();
    let h = 100.0;
    let mut l = launch(FRAC_PI_2, 0.0, 0.0, 0.0);
    l.site.h = h;
    let r = simulate3(&cfg3(p, &NO_MOTOR, l, 1.0, 20.0)).unwrap();
    let imp = r.event(EventKind::Impact).unwrap();
    let g = p.mu / (p.re * p.re) - p.omega * p.omega * p.re;
    let t = (2.0 * h / g).sqrt();
    let d = p.omega * g * t.powi(3) / 3.0;
    assert!((imp.t / t - 1.0).abs() < 1e-4);
    assert!(
        (imp.nav.lon * p.re / d - 1.0).abs() < 1e-3,
        "desvio {} vs {d}",
        imp.nav.lon * p.re
    );
}

/// Queda com arrasto: perto do solo a velocidade tende à terminal √(2mg/(ρ·CdA)).
#[test]
fn drag_gives_terminal_velocity() {
    let p = Planet::EARTH.spherical().non_rotating();
    let v = ConstantVehicle {
        cd_a: 0.5,
        ..NO_MOTOR
    };
    let mut l = launch(FRAC_PI_2, 0.0, 0.0, 0.0);
    l.site.h = 3000.0;
    let atm = AtmosphereModel::Ussa1976 { delta_t: 0.0 };
    let mut c = cfg3(p, &v, l, 2.0, 600.0);
    c.atmosphere = atm;
    let r = simulate3(&c).unwrap();
    let imp = r.event(EventKind::Impact).unwrap();
    let vt = (2.0 * 2.0 * (p.mu / (p.re * p.re)) / (atm.at(0.0).density * 0.5)).sqrt();
    assert!(
        (imp.nav.speed / vt - 1.0).abs() < 2e-3,
        "impacto a {} vs terminal {vt}",
        imp.nav.speed
    );
    assert!(
        (imp.nav.q_dyn * 0.5 / (2.0 * 9.798) - 1.0).abs() < 5e-3,
        "arrasto equilibra o peso"
    );
}

fn two_body(p: Planet) -> impl Fn(f64, &[f64; 6]) -> [f64; 6] {
    move |_, y| {
        let a = p.gravity(Vec3::new(y[0], y[1], y[2]));
        [y[3], y[4], y[5], a.x, a.y, a.z]
    }
}

/// Órbita circular: volta ao ponto de partida após T = 2π√(a³/μ), conservando energia
/// e momento angular.
#[test]
fn circular_orbit_closes_after_one_kepler_period() {
    let p = Planet::EARTH.spherical();
    let a = p.re + 400e3;
    let vc = (p.mu / a).sqrt();
    let inc = 51.6_f64.to_radians();
    let y0 = [a, 0.0, 0.0, 0.0, vc * inc.cos(), vc * inc.sin()];
    let period = 2.0 * PI * (a.powi(3) / p.mu).sqrt();
    let o = Options {
        rtol: 1e-12,
        atol: 1e-9,
        ..Options::default()
    };
    let r = integrate(&two_body(p), 0.0, y0, period, &[], &o, |_, _| {}).unwrap();
    let dr = ((r.y[0] - a).powi(2) + r.y[1].powi(2) + r.y[2].powi(2)).sqrt();
    assert!(dr < 0.05, "erro de fechamento {dr} m em {} passos", r.steps);
    let energy = |y: &[f64; 6]| {
        0.5 * (y[3] * y[3] + y[4] * y[4] + y[5] * y[5])
            - p.mu / (y[0] * y[0] + y[1] * y[1] + y[2] * y[2]).sqrt()
    };
    assert!((energy(&r.y) / energy(&y0) - 1.0).abs() < 1e-10);
    let h = |y: &[f64; 6]| Vec3::new(y[0], y[1], y[2]).cross(Vec3::new(y[3], y[4], y[5]));
    assert!((h(&r.y) - h(&y0)).norm() / h(&y0).norm() < 1e-10);
}

/// Regressão dos nodos por J2: Ω̇ = −1,5·n·J2·(Re/a)²·cos i (órbita circular).
/// Fonte: Vallado, 4ª ed., eq. 9-37.
#[test]
fn j2_nodal_regression_rate() {
    let p = Planet::EARTH;
    let (a, inc) = (7.0e6_f64, 60.0_f64.to_radians());
    let n = (p.mu / a.powi(3)).sqrt();
    let vc = (p.mu / a).sqrt();
    let y0 = [a, 0.0, 0.0, 0.0, vc * inc.cos(), vc * inc.sin()];
    let node = |y: &[f64; 6]| {
        let h = Vec3::new(y[0], y[1], y[2]).cross(Vec3::new(y[3], y[4], y[5]));
        h.x.atan2(-h.y)
    };
    let t = 30.0 * 2.0 * PI / n;
    let o = Options {
        rtol: 1e-12,
        atol: 1e-9,
        ..Options::default()
    };
    let r = integrate(&two_body(p), 0.0, y0, t, &[], &o, |_, _| {}).unwrap();
    let rate = (node(&r.y) - node(&y0)) / t;
    let want = -1.5 * n * p.j2 * (p.re / a).powi(2) * inc.cos();
    assert!((rate / want - 1.0).abs() < 0.01, "Ω̇ {rate} vs {want}");
}

/// Corpo rígido de teste: empuxo ao longo de x, inércia constante.
struct Rigid {
    thrust: f64,
    mdot: f64,
    burn: f64,
    inertia: Mat3,
}
impl Sequencer for Rigid {
    fn burnout_time(&self) -> f64 {
        self.burn
    }
}
impl RigidVehicle for Rigid {
    fn loads(&self, k: &FlightCtx<'_>, _: Vec3, _: Vec3) -> Loads {
        let on = k.burning;
        Loads {
            force_body: Vec3::new(if on { self.thrust } else { 0.0 }, 0.0, 0.0),
            moment_body: Vec3::ZERO,
            mdot: if on { self.mdot } else { 0.0 },
        }
    }
    fn mass_props(&self, _: f64, _: f64) -> MassProps {
        MassProps {
            inertia: self.inertia,
            inertia_rate: Mat3::ZERO,
        }
    }
}

fn cfg6<'a>(
    planet: Planet,
    vehicle: &'a Rigid,
    launch: Launch,
    m0: f64,
    w0: Vec3,
    t_max: f64,
) -> Config6<'a> {
    Config6 {
        planet,
        atmosphere: AtmosphereModel::Vacuum,
        vehicle,
        launch,
        m0,
        initial_omega_body: w0,
        t_max,
        output_dt: 0.1,
        ode: tight(),
        start: None,
    }
}

/// Pião simétrico sem torque (Ixx = 1, Iyy = Izz = 2): as componentes transversais de ω
/// giram a λ = (It − Ixx)·ωx/It, e o momento angular inercial e a energia se conservam.
/// Fonte: Goldstein, 3ª ed., §5.6.
#[test]
fn torque_free_symmetric_top() {
    let p = Planet {
        mu: 0.0,
        ..Planet::EARTH.spherical().non_rotating()
    };
    let body = Rigid {
        thrust: 0.0,
        mdot: 0.0,
        burn: 0.0,
        inertia: Mat3::diag(1.0, 2.0, 2.0),
    };
    let w0 = Vec3::new(3.0, 0.4, 0.0);
    let r = simulate6(&cfg6(
        p,
        &body,
        launch(FRAC_PI_2, 0.0, 0.0, 0.0),
        1.0,
        w0,
        10.0,
    ))
    .unwrap();
    let lam = (2.0 - 1.0) * 3.0 / 2.0;
    let l0 = r.samples[0].q.rotate(body.inertia * w0);
    for s in &r.samples {
        let want = Vec3::new(3.0, 0.4 * (lam * s.t).cos(), -0.4 * (lam * s.t).sin());
        assert!((s.omega - want).norm() < 1e-8, "ω em t = {}", s.t);
        assert!(
            (s.q.rotate(body.inertia * s.omega) - l0).norm() < 1e-8,
            "L em t = {}",
            s.t
        );
        assert!((s.omega.dot(body.inertia * s.omega) - w0.dot(body.inertia * w0)).abs() < 1e-8);
        assert!((s.q.norm() - 1.0).abs() < 1e-12);
    }
}

/// Eixo intermediário é instável (teorema da raquete de tênis), mas L e T se conservam
/// mesmo com o corpo capotando.
#[test]
fn asymmetric_body_conserves_momentum_while_tumbling() {
    let p = Planet {
        mu: 0.0,
        ..Planet::EARTH.spherical().non_rotating()
    };
    let body = Rigid {
        thrust: 0.0,
        mdot: 0.0,
        burn: 0.0,
        inertia: Mat3::diag(1.0, 2.0, 3.0),
    };
    let w0 = Vec3::new(1e-3, 2.0, 1e-3);
    let r = simulate6(&cfg6(
        p,
        &body,
        launch(FRAC_PI_2, 0.0, 0.0, 0.0),
        1.0,
        w0,
        60.0,
    ))
    .unwrap();
    let l0 = r.samples[0].q.rotate(body.inertia * w0);
    let flipped = r.samples.iter().any(|s| s.omega.y < -1.9);
    assert!(
        flipped,
        "o giro em torno do eixo intermediário deve inverter"
    );
    for s in &r.samples {
        assert!((s.q.rotate(body.inertia * s.omega) - l0).norm() < 1e-7);
    }
}

/// Gatilhos: por tempo, após o apogeu e por altitude na descida disparam no instante
/// certo e na ordem certa.
#[test]
fn triggers_fire_at_the_right_moments() {
    struct Seq(ConstantVehicle);
    impl Sequencer for Seq {
        fn burnout_time(&self) -> f64 {
            self.0.burn_time
        }
        fn triggers(&self) -> Vec<Trigger> {
            vec![
                Trigger::DescendingThrough { agl: 200.0 },
                Trigger::AfterApogee { delay: 1.5 },
                Trigger::Time(0.7),
            ]
        }
    }
    impl PointVehicle for Seq {
        fn thrust(&self, c: &FlightCtx<'_>) -> (f64, f64) {
            self.0.thrust(c)
        }
        fn drag_area(&self, c: &FlightCtx<'_>) -> f64 {
            // Paraquedas "abre" 1,5 s depois do apogeu.
            if c.fired[1].is_some() {
                1.0
            } else {
                0.0
            }
        }
    }
    let v = Seq(ConstantVehicle {
        thrust_vac: 200.0,
        exit_area: 0.0,
        mdot: 0.0,
        burn_time: 3.0,
        cd_a: 0.0,
    });
    let p = Planet::EARTH.spherical().non_rotating();
    let c = Config3 {
        planet: p,
        atmosphere: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
        vehicle: &v,
        launch: launch(FRAC_PI_2, 0.0, 0.0, 0.0),
        m0: 5.0,
        t_max: 600.0,
        output_dt: 0.1,
        ode: tight(),
        start: None,
    };
    let r = simulate3(&c).unwrap();
    let kinds: Vec<EventKind> = r.events.iter().map(|e| e.0).collect();
    assert_eq!(
        kinds,
        [
            EventKind::Trigger(2),
            EventKind::Burnout,
            EventKind::Apogee,
            EventKind::Trigger(1),
            EventKind::Trigger(0),
            EventKind::Impact
        ]
    );
    let at = |k| r.event(k).unwrap();
    assert!((at(EventKind::Trigger(2)).t - 0.7).abs() < 1e-12);
    assert!((at(EventKind::Trigger(1)).t - at(EventKind::Apogee).t - 1.5).abs() < 1e-9);
    assert!((at(EventKind::Trigger(0)).nav.alt - 200.0).abs() < 1e-6);
    let vt = (2.0_f64 * 5.0 * 9.798 / 1.225).sqrt();
    assert!(
        (at(EventKind::Impact).nav.speed / vt - 1.0).abs() < 0.01,
        "desce na velocidade terminal"
    );
}

/// 6-DOF sem momentos reproduz o 3-DOF: mesmo foguete, Terra girando, trilho vertical.
#[test]
fn six_dof_matches_three_dof_without_moments() {
    let l = Launch {
        site: Geodetic {
            lat: -0.04,
            lon: -0.77,
            h: 40.0,
        },
        ground_h: 40.0,
        rail: Rail {
            length: 6.0,
            elevation: 85.0_f64.to_radians(),
            azimuth: 1.0,
            friction: 0.05,
        },
        initial_speed: 0.0,
        theta0: 0.3,
    };
    let v3 = ConstantVehicle {
        thrust_vac: 2000.0,
        exit_area: 0.0,
        mdot: 1.0,
        burn_time: 8.0,
        cd_a: 0.0,
    };
    let v6 = Rigid {
        thrust: 2000.0,
        mdot: 1.0,
        burn: 8.0,
        inertia: Mat3::diag(0.05, 4.0, 4.0),
    };
    let a = simulate3(&cfg3(Planet::EARTH, &v3, l, 30.0, 400.0)).unwrap();
    let b = simulate6(&cfg6(Planet::EARTH, &v6, l, 30.0, Vec3::ZERO, 400.0)).unwrap();
    let (ra, rb) = (
        a.event(EventKind::RailExit).unwrap(),
        b.event(EventKind::RailExit).unwrap(),
    );
    assert!((ra.t - rb.t).abs() < 1e-9 && (ra.nav.speed - rb.nav.speed).abs() < 1e-8);
    // Fora do trilho, o 3-DOF alinha o empuxo à velocidade e o 6-DOF mantém a atitude
    // (sem momentos), então as trajetórias só coincidem até a saída do trilho; depois,
    // comparamos o que não depende da direção: a massa e o momento angular inercial,
    // que sem momentos externos fica igual ao herdado da rotação da Terra no trilho.
    let (ba, bb) = (
        a.event(EventKind::Burnout).unwrap(),
        b.event(EventKind::Burnout).unwrap(),
    );
    assert!((ba.mass - bb.mass).abs() < 1e-9);
    assert!(
        bb.alpha > 0.0,
        "sem estabilidade aerodinâmica aparece ângulo de ataque"
    );
    let ang_mom = |s: &Sample6| s.q.rotate(v6.inertia * s.omega);
    assert!(
        (ang_mom(bb) - ang_mom(rb)).norm() < 1e-12,
        "momento angular preservado"
    );
    assert!(
        (rb.q.rotate(rb.omega) - Planet::EARTH.omega_vec()).norm() < 1e-12,
        "no trilho gira com a Terra"
    );
    assert!(b.event(EventKind::Apogee).is_some() && b.event(EventKind::Impact).is_some());
}

/// Limite físico: a margem cruza zero em t = 3 s, o voo para ali, e retomar do estado
/// final reproduz o voo sem interrupção.
#[test]
fn limit_event_stops_and_restart_is_seamless() {
    struct Timed(Rigid, f64);
    impl Sequencer for Timed {
        fn burnout_time(&self) -> f64 {
            self.0.burn
        }
        fn margin_count(&self) -> usize {
            1
        }
    }
    impl RigidVehicle for Timed {
        fn loads(&self, k: &FlightCtx<'_>, a: Vec3, b: Vec3) -> Loads {
            self.0.loads(k, a, b)
        }
        fn mass_props(&self, t: f64, m: f64) -> MassProps {
            self.0.mass_props(t, m)
        }
        fn margin(&self, k: &FlightCtx<'_>, _: Vec3, _: Vec3, _: usize) -> f64 {
            self.1 - k.t
        }
    }
    let p = Planet::EARTH;
    let body = Timed(
        Rigid {
            thrust: 600.0,
            mdot: 0.5,
            burn: 5.0,
            inertia: Mat3::diag(0.1, 3.0, 3.0),
        },
        3.0,
    );
    let l = launch(80.0_f64.to_radians(), 0.5, 3.0, 0.0);
    fn mk(p: Planet, l: Launch, v: &Timed, start: Option<Start6>) -> Config6<'_> {
        Config6 {
            planet: p,
            atmosphere: AtmosphereModel::Vacuum,
            vehicle: v,
            launch: l,
            m0: 20.0,
            initial_omega_body: Vec3::new(0.0, 0.01, 0.0),
            t_max: 400.0,
            output_dt: 0.1,
            ode: tight(),
            start,
        }
    }
    let first = simulate6(&mk(p, l, &body, None)).unwrap();
    let end = first.end.clone().unwrap();
    assert_eq!(end.reason, EndReason::Limit(0));
    assert!((end.t - 3.0).abs() < 1e-9 && end.burning);
    let s = first.last().unwrap();
    let never = Timed(
        Rigid {
            thrust: 600.0,
            mdot: 0.5,
            burn: 5.0,
            inertia: Mat3::diag(0.1, 3.0, 3.0),
        },
        1e9,
    );
    let start = Start6 {
        t: s.t,
        r: s.r,
        v: s.v,
        q: s.q,
        omega: s.omega,
        mass: s.mass,
        burning: end.burning,
        fired: end.fired.clone(),
        apogee: end.apogee,
    };
    let rest = simulate6(&mk(p, l, &never, Some(start))).unwrap();
    let whole = simulate6(&mk(p, l, &never, None)).unwrap();
    let (a, b) = (
        rest.event(EventKind::Impact).unwrap(),
        whole.event(EventKind::Impact).unwrap(),
    );
    assert!((a.t - b.t).abs() < 1e-6, "impacto {} vs {}", a.t, b.t);
    assert!(
        (a.r - b.r).norm() < 1e-3,
        "posição de impacto difere {} m",
        (a.r - b.r).norm()
    );
    assert_eq!(rest.end.unwrap().reason, EndReason::Impact);
}
