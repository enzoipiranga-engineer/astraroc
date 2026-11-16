//! Validação analítica da mecânica orbital, reentrada e pouso.
use astra_math::Vec3;
use astra_space::body;
use astra_space::entry::{self, EntryInput};
use astra_space::kepler::{coe2rv, period, propagate, rv2coe, Elements};
use astra_space::lagrange::lagrange;
use astra_space::landing::{self, LandingInput};
use astra_space::transfer::{bielliptic, hohmann, interplanetary, lambert};

const MU: f64 = 3.986_004_418e14;

#[test]
fn kepler_roundtrip_and_full_period() {
    let k = Elements {
        a: 8000e3,
        e: 0.2,
        i: 0.5,
        raan: 1.0,
        argp: 2.0,
        nu: 0.7,
    };
    let (r, v) = coe2rv(MU, &k);
    let k2 = rv2coe(MU, r, v);
    for (a, b) in [
        (k.a, k2.a),
        (k.e, k2.e),
        (k.i, k2.i),
        (k.raan, k2.raan),
        (k.argp, k2.argp),
        (k.nu, k2.nu),
    ] {
        assert!((a - b).abs() < 1e-6 * a.abs().max(1.0), "{a} {b}");
    }
    let (r1, v1) = propagate(MU, r, v, period(MU, k.a));
    assert!((r1 - r).norm() < 1e-3, "{:?}", r1 - r);
    assert!((v1 - v).norm() < 1e-6);
    // Hipérbole: energia conservada.
    let (rh, vh) = (Vec3::new(7000e3, 0.0, 0.0), Vec3::new(0.0, 12_000.0, 0.0));
    let (r2, v2) = propagate(MU, rh, vh, 5000.0);
    let e = |r: Vec3, v: Vec3| v.norm_sq() / 2.0 - MU / r.norm();
    assert!((e(r2, v2) - e(rh, vh)).abs() / e(rh, vh).abs() < 1e-9);
}

#[test]
fn hohmann_leo_geo_and_bielliptic() {
    // LEO 300 km → GEO: Δv ≈ 3,89 km/s, ≈ 5,3 h (Curtis, exemplo 6.1 e tabela de transferências).
    let t = hohmann(MU, 6678e3, 42_164e3);
    assert!((t.total - 3893.0).abs() < 10.0, "{}", t.total);
    assert!((t.tof / 3600.0 - 5.27).abs() < 0.05, "{}", t.tof / 3600.0);
    // Para r2/r1 > 15,58 a bi-elíptica com rb grande gasta menos (Curtis, §6.3).
    let (r1, r2) = (7000e3, 20.0 * 7000e3);
    assert!(bielliptic(MU, r1, 100.0 * r1, r2).total < hohmann(MU, r1, r2).total);
    assert!(
        bielliptic(MU, 7000e3, 3.0 * 7000e3, 2.0 * 7000e3).total
            > hohmann(MU, 7000e3, 2.0 * 7000e3).total
    );
}

#[test]
fn lambert_curtis_example_5_2() {
    let mu = 398_600e9;
    let r1 = Vec3::new(5000e3, 10_000e3, 2100e3);
    let r2 = Vec3::new(-14_600e3, 2500e3, 7000e3);
    let s = lambert(mu, r1, r2, 3600.0, true).unwrap();
    let want1 = Vec3::new(-5992.5, 1925.4, 3245.6);
    let want2 = Vec3::new(-3312.5, -4196.6, -385.29);
    assert!((s.v1 - want1).norm() < 1.0, "{:?}", s.v1);
    assert!((s.v2 - want2).norm() < 1.0, "{:?}", s.v2);
    let (r, _) = propagate(mu, r1, s.v1, 3600.0);
    assert!((r - r2).norm() < 10.0);
}

#[test]
fn earth_mars_hohmann_and_windows() {
    let t = interplanetary("terra", "marte", 300e3, 300e3, 0.0, 20).unwrap();
    // Curtis, exemplo 8.4: v∞ ≈ 2,94 km/s na partida, ≈ 2,65 km/s na chegada, ≈ 259 dias.
    assert!((t.v_inf_dep - 2940.0).abs() < 40.0, "{}", t.v_inf_dep);
    assert!((t.v_inf_arr - 2650.0).abs() < 40.0, "{}", t.v_inf_arr);
    assert!((t.tof / 86_400.0 - 259.0).abs() < 3.0);
    assert!(
        (t.phase.to_degrees() - 44.3).abs() < 1.5,
        "{}",
        t.phase.to_degrees()
    );
    assert!((t.synodic / 86_400.0 - 780.0).abs() < 3.0);
    // Janelas reais: 2020 (jul–ago), 2022 (set–out), 2024 (out–nov), 2026 (out–dez).
    // O modelo circular erra semanas: aceita ±60 dias do meio da janela real.
    let jd = |y: i32, m: u32, d: u32| astra_env::time::julian_date(y, m, d, 0, 0, 0.0);
    let s_of = |y: i32, m: u32, d: u32| (jd(y, m, d) - 2_451_545.0) * 86_400.0;
    for (y, m, d) in [(2020, 7, 30), (2022, 9, 20), (2024, 11, 1), (2026, 11, 15)] {
        let real = s_of(y, m, d);
        let near = t
            .windows
            .iter()
            .map(|w| (w - real).abs())
            .fold(f64::INFINITY, f64::min);
        assert!(near < 75.0 * 86_400.0, "{y}: {} dias", near / 86_400.0);
    }
}

#[test]
fn lagrange_points_earth_moon() {
    let (e, m) = (body("terra").unwrap(), body("lua").unwrap());
    let l = lagrange(e.gm, m.gm, 384_400e3);
    let from_earth = |p: [f64; 2]| ((p[0] + l.mu * 384_400e3).powi(2) + p[1].powi(2)).sqrt();
    // L1 a ≈ 326 400 km da Terra, L2 a ≈ 448 900 km; L4 equilátero.
    assert!(
        (from_earth(l.points[0]) / 1e3 - 326_400.0).abs() < 1500.0,
        "{}",
        from_earth(l.points[0])
    );
    assert!(
        (from_earth(l.points[1]) / 1e3 - 448_900.0).abs() < 1500.0,
        "{}",
        from_earth(l.points[1])
    );
    assert!((from_earth(l.points[3]) - 384_400e3).abs() < 1.0);
    // Jacobi de L1 > L2 > L3 > L4 = L5.
    assert!(l.jacobi[0] > l.jacobi[1] && l.jacobi[1] > l.jacobi[2] && l.jacobi[2] > l.jacobi[3]);
}

#[test]
fn ballistic_entry_matches_allen_eggers() {
    // Marte tem atmosfera exponencial no modelo: comparação direta com Allen-Eggers.
    let inp = EntryInput {
        body: "marte".into(),
        mass: 600.0,
        diameter: 2.65,
        cd: 1.6,
        lift_to_drag: 0.0,
        bank_deg: 0.0,
        nose_radius: 0.66,
        emissivity: 0.85,
        entry_alt: 125e3,
        entry_speed: 6000.0,
        gamma_deg: -30.0,
        chute: None,
        tps_max_c: None,
    };
    let out = entry::simulate(&inp).unwrap();
    let ae = out.allen_eggers_g.unwrap();
    assert!(
        (out.peak_g - ae).abs() / ae < 0.08,
        "{} vs {}",
        out.peak_g,
        ae
    );
    assert!(out.heat_load > 0.0 && out.max_wall_temp > 1000.0);
}

#[test]
fn lunar_return_entry_and_chutes() {
    let inp = EntryInput {
        body: "terra".into(),
        mass: 5500.0,
        diameter: 3.9,
        cd: 1.3,
        lift_to_drag: 0.3,
        bank_deg: 45.0,
        nose_radius: 4.7,
        emissivity: 0.85,
        entry_alt: 122e3,
        entry_speed: 11_000.0,
        gamma_deg: -6.5,
        chute: Some(astra_space::entry::EntryChute {
            diameter: 25.0,
            cd: 0.8,
            deploy_mach: 0.8,
            deploy_alt: 7000.0,
        }),
        tps_max_c: Some(2700.0),
    };
    let out = entry::simulate(&inp).unwrap();
    assert!(!out.skipped);
    assert!(out.peak_g > 3.0 && out.peak_g < 12.0, "{}", out.peak_g);
    assert!(out.peak_heat > 1e6, "{}", out.peak_heat);
    assert!(out.chute_open.is_some());
    assert!(
        out.chute_shock_g > 1.0 && out.chute_shock_g < 8.0,
        "{}",
        out.chute_shock_g
    );
    // Um velame de 25 m para 5,5 t: v_terminal ≈ √(2mg/(ρ·Cd·S)) ≈ 15 m/s ao nível do mar.
    assert!(
        (out.impact_speed - 15.0).abs() < 1.5,
        "{}",
        out.impact_speed
    );
    // Raso demais: ricocheteia.
    let skip = entry::simulate(&EntryInput {
        gamma_deg: -1.0,
        lift_to_drag: 0.4,
        bank_deg: 0.0,
        chute: None,
        ..inp
    })
    .unwrap();
    assert!(skip.skipped);
}

#[test]
fn powered_landings() {
    // Vertical na Lua: altitude de ignição perto de v²/(2·(T/m − g)) com massa inicial.
    let inp = LandingInput {
        body: "lua".into(),
        altitude: 3000.0,
        vx: 0.0,
        vz: -100.0,
        mass: 1000.0,
        propellant: 300.0,
        thrust: 5000.0,
        isp: 300.0,
        cd_a: 0.0,
    };
    let out = landing::simulate(&inp).unwrap();
    assert!(out.success, "{}", out.message);
    let g = body("lua").unwrap().surface_gravity();
    let v_ig = (100f64.powi(2) + 2.0 * g * (3000.0 - out.ignition_alt)).sqrt();
    let analytic = v_ig * v_ig / (2.0 * (5.0 - g));
    assert!(
        out.ignition_alt < analytic * 1.02 && out.ignition_alt > analytic * 0.8,
        "{} vs {}",
        out.ignition_alt,
        analytic
    );
    // Descida da órbita lunar baixa por curva de gravidade.
    let lm = LandingInput {
        body: "lua".into(),
        altitude: 15_000.0,
        vx: 1690.0,
        vz: 0.0,
        mass: 15_000.0,
        propellant: 8_200.0,
        thrust: 45_000.0,
        isp: 311.0,
        cd_a: 0.0,
    };
    let out = landing::simulate(&lm).unwrap();
    assert!(out.success, "{}", out.message);
    assert!(out.dv > 1690.0 && out.dv < 2300.0, "{}", out.dv);
    // Pouco propelente: falha.
    let out = landing::simulate(&LandingInput {
        propellant: 3000.0,
        ..lm
    })
    .unwrap();
    assert!(!out.success);
    // Marte com arrasto.
    let mars = LandingInput {
        body: "marte".into(),
        altitude: 2000.0,
        vx: 0.0,
        vz: -90.0,
        mass: 1000.0,
        propellant: 250.0,
        thrust: 12_000.0,
        isp: 220.0,
        cd_a: 4.0,
    };
    assert!(landing::simulate(&mars).unwrap().success);
}
