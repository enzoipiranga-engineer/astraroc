//! Voos completos: foguete montado no 3-DOF e no 6-DOF.

use astra_aero::{BodyPart, FinPlanform, FinProfile, FinSet, Geometry, NoseShape};
use astra_dynamics::flight::*;
use astra_dynamics::ode::Options;
use astra_env::{AtmosphereModel, Geodetic, Planet};
use astra_math::Vec3;
use astra_propulsion::{Motor, WaterRocket};
use astra_vehicle::{templates, FlightSummary, Parachute, Reef, Rocket};

const ATM: AtmosphereModel = AtmosphereModel::Ussa1976 { delta_t: 0.0 };

fn launch(lat_deg: f64, lon_deg: f64, h: f64, rail: f64, elev_deg: f64) -> Launch {
    Launch {
        site: Geodetic {
            lat: lat_deg.to_radians(),
            lon: lon_deg.to_radians(),
            h,
        },
        ground_h: h,
        rail: Rail {
            length: rail,
            elevation: elev_deg.to_radians(),
            azimuth: 0.0,
            friction: 0.0,
        },
        initial_speed: 0.0,
        theta0: 0.0,
    }
}

fn opts() -> Options {
    Options {
        rtol: 1e-8,
        atol: 1e-8,
        h_max: 0.5,
        ..Options::default()
    }
}

fn fly6(r: &Rocket, l: Launch, t_max: f64) -> (Result6, FlightSummary) {
    let cfg = Config6 {
        planet: Planet::EARTH,
        atmosphere: ATM,
        vehicle: r,
        launch: l,
        m0: r.mass(0.0),
        initial_omega_body: Vec3::ZERO,
        t_max,
        output_dt: 0.02,
        ode: opts(),
        start: None,
    };
    let res = simulate6(&cfg).unwrap();
    let samples: Vec<_> = res.samples.iter().map(|s| (s.t, s.nav)).collect();
    let events: Vec<_> = res.events.iter().map(|e| (e.0, e.1.t, e.1.nav)).collect();
    let sum = FlightSummary::new(r, &Planet::EARTH, &l, &samples, &events);
    (res, sum)
}

fn fly3(r: &Rocket, l: Launch, t_max: f64) -> (Result3, FlightSummary) {
    let cfg = Config3 {
        planet: Planet::EARTH,
        atmosphere: ATM,
        vehicle: r,
        launch: l,
        m0: r.mass(0.0),
        t_max,
        output_dt: 0.02,
        ode: opts(),
        start: None,
    };
    let res = simulate3(&cfg).unwrap();
    let samples: Vec<_> = res.samples.iter().map(|s| (s.t, s.nav)).collect();
    let events: Vec<_> = res.events.iter().map(|e| (e.0, e.1.t, e.1.nav)).collect();
    let sum = FlightSummary::new(r, &Planet::EARTH, &l, &samples, &events);
    (res, sum)
}

/// Foguete de alta potência genérico, 3 kg, motor de 320 N·s.
fn sport_rocket() -> Rocket {
    let geometry = Geometry {
        parts: vec![
            BodyPart::Nose {
                shape: NoseShape::TangentOgive,
                length: 0.30,
                diameter: 0.079,
            },
            BodyPart::Tube {
                length: 1.00,
                diameter: 0.079,
            },
        ],
        fins: vec![FinSet {
            count: 3,
            planform: FinPlanform::Trapezoid {
                root_chord: 0.18,
                tip_chord: 0.08,
                sweep: 0.09,
            },
            span: 0.09,
            thickness: 0.004,
            position: 1.10,
            profile: FinProfile::Rounded,
        }],
        roughness: 3e-5,
        nozzle_exit_diameter: 0.02,
    };
    let motor = Motor::from_thrust_curve(
        "H160",
        "teste",
        0.038,
        0.20,
        0.17,
        0.15,
        &[
            (0.02, 200.0),
            (0.3, 190.0),
            (1.6, 150.0),
            (1.95, 60.0),
            (2.0, 0.0),
        ],
    );
    Rocket::new(geometry, 2.6, 0.72, (0.003, 0.42), motor)
}

#[test]
fn mass_cg_and_inertia_by_hand() {
    let r = sport_rocket();
    assert!((r.mass(0.0) - (2.6 + 0.15 + 0.17)).abs() < 1e-12);
    assert!((r.mass(5.0) - 2.75).abs() < 1e-12);
    // Motor centrado em 1,30 − 0,10 = 1,20 m.
    let cg0 = (2.6 * 0.72 + 0.32 * 1.20) / 2.92;
    assert!((r.cg(0.0) - cg0).abs() < 1e-12);
    assert!(
        r.cg(3.0) < r.cg(0.0),
        "queimar propelente na cauda leva o CG para a frente"
    );
    let cyl_t = 0.32 * (3.0 * 0.019_f64.powi(2) + 0.04) / 12.0;
    let it = 0.42 + 2.6 * (0.72 - cg0).powi(2) + cyl_t + 0.32 * (1.20 - cg0).powi(2);
    assert!((r.inertia(0.0).1 - it).abs() < 1e-12);
    assert!((r.inertia(0.0).0 - (0.003 + 0.5 * 0.32 * 0.019_f64.powi(2))).abs() < 1e-15);
    let margin = r.static_margin(0.0, 0.1);
    assert!(margin > 1.0 && margin < 4.0, "margem {margin} calibres");
}

/// Foguete estável no 6-DOF: sai do trilho inclinado, o ângulo de ataque fica pequeno
/// e a oscilação é amortecida; o apogeu coincide com o do 3-DOF (que supõe α = 0).
#[test]
fn stable_rocket_flies_straight_and_matches_3dof() {
    let r = sport_rocket();
    let l = launch(-2.3, -44.4, 40.0, 2.0, 85.0);
    let (res6, s6) = fly6(&r, l, 300.0);
    let (_, s3) = fly3(&r, l, 300.0);
    let apo_t = s6.apogee_time;
    let alpha_max = res6
        .samples
        .iter()
        .filter(|s| s.t > 0.5 && s.t < apo_t - 2.0)
        .map(|s| s.alpha)
        .fold(0.0, f64::max);
    assert!(
        alpha_max < 3.0_f64.to_radians(),
        "α máximo em voo {:.2}°",
        alpha_max.to_degrees()
    );
    assert!(
        s6.rail_exit_speed > 15.0,
        "saída do trilho a {}",
        s6.rail_exit_speed
    );
    assert!((s6.rail_exit_speed - s3.rail_exit_speed).abs() < 1e-6);
    assert!(
        (s6.apogee_agl / s3.apogee_agl - 1.0).abs() < 0.01,
        "6-DOF {} vs 3-DOF {}",
        s6.apogee_agl,
        s3.apogee_agl
    );
    assert!(
        s6.apogee_agl > 300.0 && s6.apogee_agl < 1500.0,
        "apogeu {}",
        s6.apogee_agl
    );
    // Sem paraquedas: impacto balístico, a norte (azimute 0), em alta velocidade.
    assert!(s6.impact_speed > 40.0);
    assert!(
        s6.landing_bearing < 0.2 || s6.landing_bearing > 6.08,
        "azimute {}",
        s6.landing_bearing
    );
    assert!(s6.landing_distance > 20.0);
}

/// Foguete instável (CG atrás do CP): o 6-DOF mostra a consequência, o ângulo de
/// ataque diverge logo após o trilho e o apogeu despenca.
#[test]
fn unstable_rocket_tumbles() {
    let mut r = sport_rocket();
    r.dry_cg = 1.25;
    assert!(r.static_margin(0.0, 0.1) < 0.0);
    let l = launch(-2.3, -44.4, 40.0, 2.0, 85.0);
    let (res, s) = fly6(&r, l, 300.0);
    let alpha_max = res
        .samples
        .iter()
        .filter(|x| x.t < 2.0)
        .map(|x| x.alpha)
        .fold(0.0, f64::max);
    assert!(
        alpha_max > 30.0_f64.to_radians(),
        "α {:.1}°",
        alpha_max.to_degrees()
    );
    let (_, good) = fly6(&sport_rocket(), l, 300.0);
    assert!(
        s.apogee_agl < 0.6 * good.apogee_agl,
        "instável {} vs estável {}",
        s.apogee_agl,
        good.apogee_agl
    );
}

/// Dual deploy: drogue no apogeu e principal a 150 m. Velocidades de descida iguais à
/// terminal √(2mg/(ρ·CdS)); a força de abertura supera o peso.
#[test]
fn dual_deploy_descent_rates_and_opening_load() {
    let mut r = sport_rocket();
    r.parachutes = vec![
        Parachute {
            name: "drogue".into(),
            cd_s: 0.12,
            trigger: Trigger::AfterApogee { delay: 1.0 },
            inflation_time: 0.2,
            reef: None,
        },
        Parachute {
            name: "principal".into(),
            cd_s: 1.4,
            trigger: Trigger::DescendingThrough { agl: 150.0 },
            inflation_time: 0.4,
            reef: Some(Reef {
                fraction: 0.3,
                duration: 1.0,
            }),
        },
    ];
    let l = launch(-2.3, -44.4, 40.0, 2.0, 88.0);
    let (res, s) = fly6(&r, l, 600.0);
    let kinds: Vec<EventKind> = res.events.iter().map(|e| e.0).collect();
    assert_eq!(
        kinds,
        [
            EventKind::RailExit,
            EventKind::Burnout,
            EventKind::Apogee,
            EventKind::Trigger(0),
            EventKind::Trigger(1),
            EventKind::Impact
        ]
    );
    let (m, g) = (r.mass(10.0), 9.78);
    let cd_body = r.cd(0.05, 1e6, false) * r.geometry().ref_area();
    // Sob o drogue, pouco antes do principal.
    let main_fire = res.event(EventKind::Trigger(1)).unwrap();
    let rho = ATM.at(main_fire.nav.alt).density;
    let vt = (2.0 * m * g / (rho * (0.12 + cd_body))).sqrt();
    assert!(
        (main_fire.nav.speed / vt - 1.0).abs() < 0.03,
        "drogue: {} vs {vt}",
        main_fire.nav.speed
    );
    assert!(
        (main_fire.nav.alt - 190.0).abs() < 1e-6,
        "principal a 150 m do solo"
    );
    // Sob o principal, no pouso.
    let vt_main = (2.0 * m * g / (ATM.at(40.0).density * (0.12 + 1.4 + cd_body))).sqrt();
    assert!(
        (s.impact_speed / vt_main - 1.0).abs() < 0.03,
        "pouso a {} vs {vt_main}",
        s.impact_speed
    );
    assert!(s.impact_speed < 7.0);
    let main_load = s
        .opening_loads
        .iter()
        .find(|p| p.0 == "principal")
        .unwrap()
        .1;
    assert!(
        main_load > m * g,
        "força de abertura {main_load} N vs peso {} N",
        m * g
    );
    assert!(main_load < 40.0 * m * g);
}

/// Calisto (exemplo do RocketPy) em Spaceport America, atmosfera padrão, sem vento.
/// Referência cruzada: a documentação do RocketPy mostra apogeu da ordem de 3,3 km
/// acima do solo para este foguete (com atmosfera de previsão do dia, não a padrão).
#[test]
fn calisto_benchmark() {
    let r = templates::calisto();
    assert!((r.mass(0.0) - (14.426 + 1.815 + 2.956)).abs() < 1e-9);
    let margin = r.static_margin(0.0, 0.3);
    assert!(margin > 1.5 && margin < 3.5, "margem {margin}");
    let l = launch(32.990_254, -106.974_998, 1400.0, 5.2, 85.0);
    let (_, s) = fly6(&r, l, 600.0);
    println!("Calisto 6-DOF, Cd de tabela: {s:#?}");
    assert!(
        s.apogee_agl > 3000.0 && s.apogee_agl < 3700.0,
        "apogeu {}",
        s.apogee_agl
    );
    assert!(s.rail_exit_speed > 20.0 && s.rail_exit_speed < 35.0);
    assert!(s.max_mach > 0.7 && s.max_mach < 1.0);
    assert!(
        s.impact_speed > 4.0 && s.impact_speed < 7.0,
        "pouso sob o principal: {}",
        s.impact_speed
    );

    // Mesmo foguete com o Cd analítico do ASTRA no lugar da tabela.
    let mut analytic = r.clone();
    analytic.drag = astra_vehicle::DragModel::Analytic;
    let (_, sa) = fly6(&analytic, l, 600.0);
    println!(
        "Calisto com Cd analítico: apogeu {:.0} m ({:+.1}% vs tabela)",
        sa.apogee_agl,
        100.0 * (sa.apogee_agl / s.apogee_agl - 1.0)
    );
    assert!((sa.apogee_agl / s.apogee_agl - 1.0).abs() < 0.15);
}

/// Garrafa PET de 2 L: voo plausível e volume ótimo de água perto de um terço.
#[test]
fn pet_bottle_flight_and_optimal_water() {
    let water = WaterRocket::pet_2l();
    let l = launch(-23.5, -46.6, 760.0, 0.5, 88.0);
    let apogee = |w: &WaterRocket| fly3(&templates::pet_bottle(w), l, 60.0).1.apogee_agl;
    let r = templates::pet_bottle(&water);
    let (full, empty) = (r.static_margin(0.0, 0.05), r.static_margin(1.0, 0.05));
    println!("margem estática: cheia {full:.2}, vazia {empty:.2} calibres");
    assert!(full > 0.3 && empty > 1.5, "margens {full} e {empty}");
    let (_, s6) = fly6(&r, l, 60.0);
    let a3 = apogee(&water);
    println!(
        "PET 2 L, 0,66 L de água, 4 bar: apogeu 6-DOF {:.1} m, 3-DOF {:.1} m, v máx {:.1} m/s",
        s6.apogee_agl, a3, s6.max_speed
    );
    assert!(
        s6.apogee_agl > 25.0 && s6.apogee_agl < 120.0,
        "apogeu {}",
        s6.apogee_agl
    );
    assert!((s6.apogee_agl / a3 - 1.0).abs() < 0.05);
    assert!(s6.flight_time > 3.0 && s6.flight_time < 12.0);

    let (best_v, best_a) = astra_vehicle::optimal_water_volume(&water, apogee);
    let frac = best_v / water.bottle_volume;
    println!(
        "água ótima: {:.2} L ({:.0}% da garrafa), apogeu {:.1} m",
        best_v * 1000.0,
        frac * 100.0,
        best_a
    );
    assert!(frac > 0.2 && frac < 0.45, "fração ótima {frac}");
    assert!(best_a >= a3 - 1e-6);
    assert!(
        best_a
            > apogee(&WaterRocket {
                water_volume: 0.2e-3,
                ..water
            })
    );
    assert!(
        best_a
            > apogee(&WaterRocket {
                water_volume: 1.4e-3,
                ..water
            })
    );
}
