//! Lançadores: soluções fechadas, balanço de energia e verificação de plugins.

use astra_env::{AtmosphereModel, Wind, WindLayer};
use astra_launch::*;
use astra_math::Vec3;
use std::collections::BTreeMap;

struct Rocket {
    mass: f64,
    thrust: f64,
    mdot: f64,
    burn: f64,
}
impl Payload for Rocket {
    fn mass(&self, t: f64, ign: f64) -> f64 {
        self.mass - self.mdot * (t - ign).clamp(0.0, self.burn)
    }
    fn thrust(&self, t: f64, ign: f64, _: f64) -> f64 {
        if t >= ign && t - ign <= self.burn {
            self.thrust
        } else {
            0.0
        }
    }
    fn drag_area(&self) -> f64 {
        0.004
    }
    fn length(&self) -> f64 {
        1.2
    }
    fn diameter(&self) -> f64 {
        0.08
    }
}
const INERT: Rocket = Rocket {
    mass: 5.0,
    thrust: 0.0,
    mdot: 0.0,
    burn: 0.0,
};

fn vacuum() -> LocalEnv {
    LocalEnv {
        g: 9.806_65,
        ground: 0.0,
        atm: AtmosphereModel::Vacuum,
        wind: Wind::calm(),
    }
}
fn air() -> LocalEnv {
    LocalEnv {
        g: 9.806_65,
        ground: 0.0,
        atm: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
        wind: Wind::calm(),
    }
}

fn guide(push: Push) -> Guide {
    Guide {
        label: "teste".into(),
        length: 10.0,
        elevation_deg: 0.0,
        azimuth_deg: 90.0,
        carriage_mass: 1.0,
        friction: 0.0,
        carriage_cd_a: 0.0,
        push,
        release_speed: None,
        ignite_at: None,
        ignition_delay: 0.0,
        brake: 50.0,
        height: 0.0,
    }
}

fn speed(o: &LaunchOutcome) -> f64 {
    o.release.unwrap().vel.norm()
}

/// Força constante na horizontal, sem perdas: v = √(2FL/m).
#[test]
fn constant_force_and_railgun() {
    let o = guided(
        &guide(Push::Constant { force: 600.0 }),
        Base::default(),
        &INERT,
        &vacuum(),
    );
    assert!((speed(&o) - (2.0 * 600.0 * 10.0 / 6.0_f64).sqrt()).abs() < 1e-6);
    assert!(o.energy.residual < 1e-8);
    let r = o.release.unwrap();
    assert!(
        (r.pos - Vec3::new(10.0, 0.0, 0.0)).norm() < 1e-6,
        "azimute 90° = leste"
    );
    // Railgun: F = ½·L'·I² = ½·0,5e-6·(2e4)² = 100 N.
    let rg = guided(
        &guide(Push::Railgun {
            current: 2e4,
            inductance_gradient: 0.5e-6,
        }),
        Base::default(),
        &INERT,
        &vacuum(),
    );
    assert!((speed(&rg) - (2.0 * 100.0 * 10.0 / 6.0_f64).sqrt()).abs() < 1e-6);
}

/// Mola: toda a energia ½·k·s² vira cinética: v = s·√(k/m).
#[test]
fn spring_energy() {
    let mut g = guide(Push::Spring {
        stiffness: 2000.0,
        stroke: 0.5,
    });
    g.length = 0.5;
    let o = guided(&g, Base::default(), &INERT, &vacuum());
    assert!((speed(&o) - 0.5 * (2000.0 / 6.0_f64).sqrt()).abs() < 1e-6);
    assert!(o.energy.residual < 1e-8);
}

/// Canhão de gás: trabalho W = p0·V0/(γ−1)·(1 − (V0/V1)^(γ−1)) − p_atm·A·L.
#[test]
fn gas_gun_adiabatic_work() {
    let (p0, v0, bore, gamma) = (8e5_f64, 0.01_f64, 0.06_f64, 1.4_f64);
    let mut g = guide(Push::GasGun {
        p0,
        volume: v0,
        bore,
        gamma,
    });
    g.length = 2.0;
    let o = guided(&g, Base::default(), &INERT, &vacuum());
    let a = 0.25 * core::f64::consts::PI * bore * bore;
    let v1 = v0 + a * 2.0;
    let w = p0 * v0 / (gamma - 1.0) * (1.0 - (v0 / v1).powf(gamma - 1.0)) - 101_325.0 * a * 2.0;
    assert!((0.5 * 6.0 * speed(&o).powi(2) / w - 1.0).abs() < 1e-6);
}

/// Rampa inclinada com atrito: a = F/m − g·sen θ − μ·g·cos θ.
#[test]
fn inclined_ramp_with_friction() {
    let mut g = guide(Push::Constant { force: 400.0 });
    g.elevation_deg = 30.0;
    g.friction = 0.1;
    let o = guided(&g, Base::default(), &INERT, &vacuum());
    let (s, c) = 30.0_f64.to_radians().sin_cos();
    let a = 400.0 / 6.0 - 9.806_65 * s - 0.1 * 9.806_65 * c;
    assert!((speed(&o) - (2.0 * a * 10.0).sqrt()).abs() < 1e-6);
    assert!(o.energy.residual < 1e-8 && o.energy.dissipated > 0.0);
    assert!((o.release.unwrap().pos.z - 5.0).abs() < 1e-6);
}

/// Motor aceso no carrinho, com ar: o balanço fecha incluindo a energia levada pelo
/// propelente expelido.
#[test]
fn rocket_on_cart_energy_balance() {
    let r = Rocket {
        mass: 20.0,
        thrust: 2000.0,
        mdot: 1.0,
        burn: 5.0,
    };
    let mut g = guide(Push::Constant { force: 500.0 });
    g.elevation_deg = 10.0;
    g.length = 40.0;
    g.friction = 0.02;
    g.carriage_cd_a = 0.05;
    g.ignite_at = Some(0.0);
    let o = guided(&g, Base::default(), &r, &air());
    assert!(o.energy.residual < 1e-6, "resíduo {}", o.energy.residual);
    assert!(o.energy.mass_flux < 0.0);
    let rel = o.release.unwrap();
    assert!(rel.mass < 20.0 && rel.ignition == 0.0);
    assert!(o.tracks.iter().any(|t| t.0.starts_with("Carrinho")));
}

/// Plataforma giratória no vácuo: tempo de aceleração I·ω/τ, v = ω·R, energia fechada,
/// saída tangencial na elevação pedida, girando com ω.
#[test]
fn spin_launcher() {
    let s = Spin {
        radius: 10.0,
        arm_mass: 300.0,
        torque: 5_000.0,
        target_rpm: 300.0,
        chamber_pressure: 0.0,
        arm_cd_width: 0.3,
        release_elevation_deg: 60.0,
        azimuth_deg: 0.0,
        hub_height: 12.0,
        ignition_delay: 1.0,
    };
    let o = spin(&s, Base::default(), &INERT, &vacuum());
    let w = 300.0 * 2.0 * core::f64::consts::PI / 60.0;
    let i = 300.0 * 100.0 / 3.0 + 5.0 * 100.0;
    let r = o.release.unwrap();
    assert!((r.vel.norm() - w * 10.0).abs() < 1e-6);
    let elev = r.vel.z.atan2(r.vel.x.hypot(r.vel.y)).to_degrees();
    assert!((elev - 60.0).abs() < 1e-6, "elevação {elev}");
    assert!((r.omega.norm() - w).abs() < 1e-9);
    assert!(o.energy.residual < 1e-8);
    let spun = o.events.iter().find(|e| e.1 == "spunUp").unwrap().0;
    assert!(
        (spun - i * w / 5_000.0).abs() < 1e-6,
        "tempo de aceleração {spun}"
    );
    assert!(o
        .loads
        .iter()
        .any(|l| (l.2 - w * w * 10.0 / 9.806_65).abs() < 1e-6));
    assert!((r.ignition_time() - (r.t + 1.0)).abs() < 1e-12);
}

/// Balão: sobe até a altura de liberação, deriva com o vento e o balanço fecha.
#[test]
fn balloon_rockoon_drift() {
    let b = Balloon {
        envelope_mass: 3.0,
        gas_mass: 2.0,
        gas_molar_mass: 4.0026,
        gondola_mass: 2.0,
        burst_diameter: 12.0,
        cd: 0.3,
        release_altitude: 15_000.0,
        rail_elevation_deg: 85.0,
        rail_azimuth_deg: 0.0,
    };
    let mut env = air();
    env.wind = Wind {
        layers: vec![WindLayer {
            agl: 0.0,
            speed: 8.0,
            from: 270.0_f64.to_radians(),
        }],
        turbulence: None,
    };
    let o = balloon(&b, Base::default(), &INERT, &env);
    let r = o.release.unwrap_or_else(|| panic!("{:?}", o.warnings));
    assert!((r.pos.z - 15_000.0).abs() < 1e-3);
    assert!(
        r.pos.x > 0.5 * 8.0 * (r.t - 0.0),
        "deriva para leste com vento de oeste"
    );
    assert!((r.vel.x - 8.0).abs() < 0.5, "move-se com o vento");
    assert!(o.energy.residual < 1e-5, "resíduo {}", o.energy.residual);
    // Pouco gás: não sobe.
    let weak = balloon(
        &Balloon {
            gas_mass: 0.3,
            ..b.clone()
        },
        Base::default(),
        &INERT,
        &env,
    );
    assert!(weak.release.is_none() && weak.warnings[0].contains("não sobe"));
    // Estoura antes.
    let small = balloon(
        &Balloon {
            burst_diameter: 4.0,
            ..b
        },
        Base::default(),
        &INERT,
        &env,
    );
    assert!(small.warnings.iter().any(|w| w.contains("estourou")));
}

#[test]
fn submarine_and_ship() {
    let s = Submarine {
        depth: 20.0,
        ejection_speed: 25.0,
        water_cd: 0.3,
        ignition_delay: 0.5,
    };
    let o = submarine(&s, Base::default(), &INERT, &air());
    let r = o.release.unwrap();
    assert!(r.vel.z > 0.0 && r.vel.z < 25.0 && o.energy.residual < 1e-6);
    let ship_out = ship(
        &Ship {
            speed: 10.0,
            heading_deg: 0.0,
            pitch_amplitude_deg: 5.0,
            roll_amplitude_deg: 8.0,
            wave_period: 8.0,
            launch_phase: 1.0,
            deck_height: 6.0,
            rail_length: 3.0,
            rail_elevation_deg: 80.0,
        },
        Base::default(),
        &Rocket {
            mass: 20.0,
            thrust: 3000.0,
            mdot: 1.0,
            burn: 3.0,
        },
        &air(),
    );
    let rs = ship_out.release.unwrap();
    assert!(rs.vel.y > 10.0, "leva a velocidade do navio (rumo norte)");
    assert!(rs.omega.norm() > 0.0, "sai com a rotação do convés");
    assert!(rs.pos.z > 6.0);
}

/// Cadeia: avião e, nele, um canhão de mola: as velocidades se somam (Galileu).
#[test]
fn chain_adds_velocities() {
    let plane = Launcher::Air(AirLaunch {
        altitude: 10_000.0,
        speed: 200.0,
        flight_path_deg: 0.0,
        heading_deg: 90.0,
        drop_time: 0.0,
        aircraft_mass: 30_000.0,
    });
    let mut g = guide(Push::Spring {
        stiffness: 2000.0,
        stroke: 0.5,
    });
    g.length = 0.5;
    g.azimuth_deg = 90.0;
    let o = run_chain(&[plane, Launcher::Guided(g)], &INERT, &vacuum());
    let r = o.release.unwrap();
    assert!((r.vel.x - (200.0 + 0.5 * (2000.0 / 6.0_f64).sqrt())).abs() < 1e-6);
    assert!((r.pos.z - 10_000.0).abs() < 1e-6);
    assert_eq!(o.tracks.iter().filter(|t| t.0 == "Avião").count(), 1);
}

/// Plugins: os de exemplo passam na verificação; erros de pacote são apontados; a
/// expressão só enxerga as variáveis permitidas.
#[test]
fn plugin_verification() {
    for p in builtin_plugins() {
        let c = p.check();
        assert!(c.ok, "{}: {:?}", p.name, c.messages);
    }
    let mut bad = builtin_plugins().remove(0);
    bad.force = "F0 * 2".into();
    assert!(bad
        .check()
        .messages
        .iter()
        .any(|m| m.contains("desconhecida")));
    let mut out_of_range = builtin_plugins().remove(0);
    out_of_range.params[0].value = -5.0;
    assert!(!out_of_range.check().ok);
    let mut values = BTreeMap::new();
    values.insert("elev".to_owned(), 120.0);
    assert!(builtin_plugins()[0]
        .to_guide(&values)
        .unwrap_err()
        .contains("fora da faixa"));
    // Pacote completo ida e volta em JSON.
    let json = serde_json::to_string(&builtin_plugins()[1]).unwrap();
    let back: PluginSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(back, builtin_plugins()[1]);
    // O plugin roda pela interface comum de lançadores.
    let l = Launcher::Plugin {
        spec: Box::new(back),
        values: BTreeMap::new(),
    };
    assert!(l.run(Base::default(), &INERT, &air()).release.is_some());
}

/// Formato JSON da interface: campos em camelCase também dentro das variantes.
#[test]
fn json_field_names() {
    let l = Launcher::Guided(guide(Push::Power {
        power: 1e5,
        max_force: 1e4,
    }));
    let j = serde_json::to_string(&l).unwrap();
    assert!(
        j.contains("\"maxForce\"")
            && j.contains("\"type\":\"power\"")
            && j.contains("\"type\":\"guided\""),
        "{j}"
    );
    let s: Launcher = serde_json::from_str(r#"{"type":"spin","radius":5,"armMass":1,"torque":1,"targetRpm":1,"chamberPressure":0,"armCdWidth":0.1,"releaseElevationDeg":45,"azimuthDeg":0,"hubHeight":1,"ignitionDelay":0}"#).unwrap();
    assert!(matches!(s, Launcher::Spin(_)));
}
