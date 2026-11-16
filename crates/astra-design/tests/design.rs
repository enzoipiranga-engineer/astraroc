//! Gêmeo digital: verificação da montagem e falhas simuladas.

use astra_design::*;
use core::f64::consts::PI;

fn db() -> MaterialDb {
    MaterialDb::builtin()
}

fn codes(v: &Validation, sev: Severity) -> Vec<String> {
    v.issues
        .iter()
        .filter(|i| i.severity == sev)
        .map(|i| i.code.clone())
        .collect()
}

fn kinds(o: &SimOutput) -> Vec<&str> {
    o.events.iter().map(|e| e.kind.as_str()).collect()
}

fn set_kind(d: &mut Design, id: &str, f: impl FnOnce(&mut PartKind)) {
    f(&mut d.parts.iter_mut().find(|p| p.id == id).unwrap().kind);
}

#[test]
fn material_database_is_complete_and_sourced() {
    let db = db();
    assert!(db.materials.len() >= 45);
    for m in &db.materials {
        assert!(m.density_kg_m3 > 0.0, "{}", m.id);
        assert!(!m.source.is_empty(), "{} sem fonte", m.id);
    }
    let al = db.get("al-6061-t6").unwrap();
    assert_eq!((al.density_kg_m3, al.yield_strength()), (2700.0, 276e6));
    assert!(MaterialDb::from_json(r#"{"materials":[]}"#)
        .unwrap()
        .materials
        .is_empty());
    let mut db2 = db.clone();
    let mut mine = al.clone();
    mine.id = "meu-aluminio".into();
    mine.reliability = Reliability::Medido;
    db2.upsert(mine);
    assert_eq!(
        db2.get("meu-aluminio").unwrap().reliability,
        Reliability::Medido
    );
}

#[test]
fn design_json_round_trip() {
    for (_, d) in templates::all() {
        let back = Design::from_json(&d.to_json()).unwrap();
        assert_eq!(back, d);
    }
    let mut future = templates::pet_bottle();
    future.version = FORMAT_VERSION + 1;
    assert!(Design::from_json(&future.to_json())
        .unwrap_err()
        .contains("versão"));
}

/// Capacidades à mão: dois pinos de náilon de 1,6 mm (τ = 66 MPa) e a força de ejeção
/// p·A com p = m·R·T/V.
#[test]
fn joint_capacity_and_ejection_force_by_hand() {
    let d = templates::competition_500m();
    let asm = Assembly::new(&d, &db());
    let ji = d.joints.iter().position(|j| j.id == "j-coifa").unwrap();
    let pins = asm.joints[ji].as_ref().unwrap();
    assert!((pins.tension - 2.0 * 66e6 * PI * 0.0016_f64.powi(2) / 4.0).abs() < 1e-9);
    assert_eq!(pins.mode, LoadMode::TensionOnly);
    assert!(
        pins.compression.is_infinite(),
        "coifa encostada no tubo carrega compressão"
    );
    let chute = d.parts.iter().position(|p| p.id == "paraquedas").unwrap();
    let (vol, area, wall) = asm.bay(ji, chute);
    assert_eq!(wall.map(|w| d.parts[w].id.as_str()), Some("antep-sup"));
    assert!((area - PI * 0.0265_f64.powi(2)).abs() < 1e-12);
    let f = asm.ejection_force(ji, chute, 0.0008);
    assert!((f - 0.0008 * 2.19e5 / vol * area).abs() < 1e-9);
}

/// O exemplo de competição passa na verificação, voa perto de 500 m e pousa sob o
/// paraquedas ejetado 1 s depois do apogeu.
#[test]
fn competition_example_is_clean_and_recovers() {
    let d = templates::competition_500m();
    let v = validate(&d, &db(), true);
    assert!(codes(&v, Severity::Error).is_empty(), "{:#?}", v.issues);
    assert!(codes(&v, Severity::Warning).is_empty(), "{:#?}", v.issues);
    let s = &v.summary;
    assert!(s.apogee > 450.0 && s.apogee < 620.0, "apogeu {}", s.apogee);
    assert!(s.static_margin > 1.5 && s.rail_exit_speed > 15.0);
    let o = simulate(&d, &db());
    assert_eq!(
        kinds(&o),
        ["railExit", "burnout", "apogee", "ejection", "impact"]
    );
    let apo = o.events[2].t;
    assert!((o.events[3].t - apo - 1.0).abs() < 1e-6);
    assert_eq!(o.bodies.len(), 1);
    let land = o.bodies[0].landing.unwrap();
    assert!(land[1] > 4.0 && land[1] < 7.0, "pouso a {} m/s", land[1]);
}

/// Critério de aceite: coifa sem fixação. O aviso aparece; ignorado, a coifa pesada
/// (com lastro) sai voando no fim da queima e cada parte cai por conta própria.
#[test]
fn loose_nose_warns_and_then_separates_in_flight() {
    let mut d = templates::pet_bottle();
    d.joints.retain(|j| j.id != "j-coifa");
    let v = validate(&d, &db(), true);
    let nc = v.issues.iter().find(|i| i.code == "notConnected").unwrap();
    assert!(nc.message.contains("coifa") || nc.message.contains("Coifa"));
    assert!(nc.parts.contains(&"coifa".to_owned()) && nc.parts.contains(&"lastro".to_owned()));
    assert!(
        codes(&v, Severity::Error).contains(&"overload".to_owned()),
        "o voo nominal mostra a coifa sendo puxada"
    );

    let o = simulate(&d, &db());
    let brk = o
        .events
        .iter()
        .find(|e| e.kind == "jointBroken")
        .expect("a coifa se solta");
    let burnout_t = 0.11;
    assert!(
        (brk.t - burnout_t).abs() < 0.05,
        "solta perto do fim do empuxo: {}",
        brk.t
    );
    assert_eq!(o.bodies.len(), 2);
    let nose = o.bodies.iter().find(|b| b.name == "Coifa").unwrap();
    let main = &o.bodies[0];
    assert_eq!(nose.parent, Some(main.id));
    // Na separação as duas partes têm a mesma velocidade (mesmo corpo um instante antes).
    let v_main = main.samples.iter().find(|r| r[0] >= brk.t).unwrap()[2];
    assert!((nose.samples[0][2] - v_main).abs() < 1.0);
    // Cada uma segue a própria trajetória até o solo.
    assert!(nose.landing.is_some() && main.landing.is_some());
    let intact = simulate(&templates::pet_bottle(), &db());
    let apo =
        |o: &SimOutput, b: usize| o.bodies[b].samples.iter().map(|r| r[1]).fold(0.0, f64::max);
    assert!(
        apo(&o, main.id) < 0.5 * apo(&intact, 0),
        "sem a coifa o corpo perde o nariz e capota"
    );
}

/// Pinos fortes demais para a carga de ejeção: a baia não abre e o foguete cai sem
/// paraquedas.
#[test]
fn ejection_too_weak_for_shear_pins() {
    let mut d = templates::competition_500m();
    for j in &mut d.joints {
        if j.id == "j-coifa" {
            j.kind = JointKind::ShearPins {
                count: 8,
                diameter: 0.003,
                material: "nylon-66".into(),
            };
        }
    }
    let v = validate(&d, &db(), true);
    assert!(
        codes(&v, Severity::Error).contains(&"weakCharge".to_owned()),
        "{:#?}",
        v.issues
    );
    let o = simulate(&d, &db());
    assert!(kinds(&o).contains(&"ejectionFailed"));
    assert!(
        o.bodies[0].landing.unwrap()[1] > 30.0,
        "sem paraquedas o impacto é balístico"
    );
}

/// Baia fechada (sem respiro) e coifa só no atrito: o ar preso empurra a coifa na
/// subida, o paraquedas sai antes da hora e, com cordão fraco, o cordão rompe.
#[test]
fn sealed_bay_pops_nose_early_and_weak_cord_breaks() {
    let mut d = templates::competition_500m();
    set_kind(&mut d, "tubo-sup", |k| {
        if let PartKind::BodyTube { vent_holes, .. } = k {
            *vent_holes = 0;
        }
    });
    for j in &mut d.joints {
        match j.id.as_str() {
            "j-coifa" => j.kind = JointKind::FrictionFit { holding_force: 6.0 },
            "j-cordao-coifa" | "j-cordao-antep" | "j-paraquedas" => {
                j.kind = JointKind::Cord { strength: 60.0 }
            }
            _ => {}
        }
    }
    let v = validate(&d, &db(), true);
    assert!(codes(&v, Severity::Warning).contains(&"sealedBay".to_owned()));
    let o = simulate(&d, &db());
    let apo_t = o
        .events
        .iter()
        .find(|e| e.kind == "apogee")
        .map(|e| e.t)
        .unwrap();
    let dep = o
        .events
        .iter()
        .find(|e| e.kind == "deploy")
        .expect("paraquedas sai antes da hora");
    assert!(dep.t < apo_t, "abriu em {} s, apogeu em {} s", dep.t, apo_t);
    assert!(dep.speed > 15.0);
    assert!(kinds(&o).contains(&"cordBroken"), "{:#?}", kinds(&o));
    assert!(o.bodies.len() >= 2);
}

/// Aletas finas de balsa entram em flutter: a verificação acusa e, no voo, uma aleta se
/// parte e cai separada.
#[test]
fn thin_balsa_fins_flutter_and_break() {
    let mut d = templates::competition_500m();
    for p in &mut d.parts {
        if p.id == "aletas" {
            p.material = "balsa".into();
            if let PartKind::FinSet { thickness, .. } = &mut p.kind {
                *thickness = 0.0008;
            }
        }
    }
    let v = validate(&d, &db(), true);
    assert!(
        v.issues
            .iter()
            .any(|i| i.code == "overload" && i.message.contains("flutter")),
        "{:#?}",
        v.issues
    );
    let o = simulate(&d, &db());
    assert!(kinds(&o).contains(&"finLost"));
    assert!(o.bodies.iter().any(|b| b.name.starts_with("Aleta solta")));
}

#[test]
fn fit_overlap_and_blocked_chute_are_reported() {
    let mut d = templates::competition_500m();
    set_kind(&mut d, "altimetro", |k| {
        if let PartKind::Mass { diameter, .. } = k {
            *diameter = 0.06;
        }
    });
    d.parts
        .iter_mut()
        .find(|p| p.id == "bateria")
        .unwrap()
        .position = 0.52;
    d.parts.push(Part {
        id: "antep-extra".into(),
        name: "Anteparo extra".into(),
        material: "compensado-betula".into(),
        position: 0.27,
        mass_override: None,
        cost: Some(4.5),
        kind: PartKind::Bulkhead {
            diameter: 0.053,
            thickness: 0.004,
        },
    });
    let v = validate(&d, &db(), false);
    let e = codes(&v, Severity::Error);
    for c in ["doesNotFit", "overlap", "chuteBlocked", "notConnected"] {
        assert!(e.contains(&c.to_owned()), "faltou {c}: {e:?}");
    }
    assert!((v.summary.cost - 4.5).abs() < 1e-12);
}

#[test]
fn instability_is_an_error() {
    let mut d = templates::competition_500m();
    d.parts
        .iter_mut()
        .find(|p| p.id == "aletas")
        .unwrap()
        .position = 0.40;
    let v = validate(&d, &db(), false);
    assert!(v.summary.static_margin < 0.0);
    assert!(codes(&v, Severity::Error).contains(&"unstable".to_owned()));
}

/// Mudar uma peça muda tudo junto: massa, CG, margem e apogeu.
#[test]
fn changing_a_part_propagates() {
    let a = validate(&templates::competition_500m(), &db(), true).summary;
    let mut d = templates::competition_500m();
    d.parts
        .iter_mut()
        .find(|p| p.id == "coifa")
        .unwrap()
        .material = "pla".into();
    d.parts.push(Part {
        id: "lastro".into(),
        name: "Lastro".into(),
        material: "aco-1020".into(),
        position: 0.12,
        mass_override: None,
        cost: None,
        kind: PartKind::Mass {
            mass: 0.1,
            length: 0.03,
            diameter: 0.03,
            needs_static_port: false,
        },
    });
    d.joints.push(Joint {
        id: "j-lastro".into(),
        a: "lastro".into(),
        b: "coifa".into(),
        kind: JointKind::Adhesive {
            adhesive: "epoxi-estrutural".into(),
            area: Some(5e-4),
            fillet: None,
            through_wall: false,
        },
    });
    let b = validate(&d, &db(), true).summary;
    assert!(b.liftoff_mass > a.liftoff_mass + 0.09);
    assert!(b.cg < a.cg && b.static_margin > a.static_margin);
    assert!(b.apogee < a.apogee);
}

/// Vento e data/hora no projeto: deriva para o lado certo, zona proibida acusada,
/// hora sideral aplicada, e as amostras trazem latitude, longitude e eixo do corpo.
#[test]
fn wind_time_and_zones() {
    let mut d = templates::competition_500m();
    d.launch.wind = WindSpec {
        layers: vec![
            WindLayerSpec {
                agl: 0.0,
                speed: 4.0,
                from_deg: 270.0,
            },
            WindLayerSpec {
                agl: 1000.0,
                speed: 8.0,
                from_deg: 270.0,
            },
        ],
        turbulence: Some(TurbulenceSpec {
            sigma: 0.5,
            length: 300.0,
            seed: 7,
        }),
        source: Some("manual".into()),
    };
    d.launch.utc = Some("2026-09-03T14:00:00Z".into());
    let o = simulate(&d, &db());
    let main = &o.bodies[0];
    let land = main.landing.unwrap();
    assert!(
        land[3] > 100.0,
        "paraquedas deriva para leste com vento de oeste: leste = {} m",
        land[3]
    );
    let last = main.samples.last().unwrap();
    assert!(
        (last[10] - d.launch.lat_deg).abs() < 0.05 && (last[11] - d.launch.lon_deg).abs() < 0.05
    );
    assert!((last[12] - d.launch.altitude).abs() < 1.0);
    let first = &main.samples[0];
    let axis = Vec3Like(first[13], first[14], first[15]);
    // No trilho, o eixo aponta para cima (85°): componente radial ≈ sen(85°).
    let up = {
        let (la, lo) = (d.launch.lat_deg.to_radians(), d.launch.lon_deg.to_radians());
        Vec3Like(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin())
    };
    assert!(
        (axis.dot(&up) - 85.0_f64.to_radians().sin()).abs() < 0.01,
        "eixo·cima {}",
        axis.dot(&up)
    );

    // Zona proibida exatamente onde o foguete cai.
    let (lat, lon) = (last[10], last[11]);
    d.zones = vec![Zone {
        name: "Estacionamento".into(),
        lat_deg: lat,
        lon_deg: lon,
        radius: 50.0,
        kind: ZoneKind::KeepOut,
    }];
    let v = validate(&d, &db(), true);
    assert!(
        v.issues
            .iter()
            .any(|i| i.code == "keepOut" && i.message.contains("Estacionamento")),
        "{:#?}",
        v.issues
    );

    // Mesma semente, mesmo voo (determinismo com rajadas).
    let again = simulate(&d, &db());
    assert_eq!(again.bodies[0].landing, o.bodies[0].landing);
}

struct Vec3Like(f64, f64, f64);
impl Vec3Like {
    fn dot(&self, o: &Self) -> f64 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }
}

/// Lançamento aéreo do foguete de competição: parte do estado do avião, acende depois da
/// queda livre e sobe muito mais que do trilho; o relatório compara os dois.
#[test]
fn air_launch_of_the_competition_rocket() {
    let mut d = templates::competition_500m();
    d.launcher = vec![astra_launch::Launcher::Air(astra_launch::AirLaunch {
        altitude: 3_000.0,
        speed: 120.0,
        flight_path_deg: 20.0,
        heading_deg: 0.0,
        drop_time: 1.0,
        aircraft_mass: 2_000.0,
    })];
    let o = simulate(&d, &db());
    let rep = o.launcher.clone().expect("relatório do lançador");
    assert!(rep.released && (rep.release_speed - 120.0).abs() < 1e-6);
    assert!((rep.ignition_time - rep.release_time - 1.0).abs() < 1e-9);
    assert!(
        rep.apogee_with > rep.apogee_without + 2_500.0,
        "com {} sem {}",
        rep.apogee_with,
        rep.apogee_without
    );
    assert!(rep.dv_equivalent > 120.0 && rep.propellant_equivalent > 0.0);
    // O foguete continua sendo o corpo 0 e começa no estado de liberação.
    let first = o.bodies[0].samples[0];
    assert!((first[1] - 3_000.0).abs() < 1.0 && (first[2] - 120.0).abs() < 1.0);
    assert!(o.bodies.iter().any(|b| b.name == "Avião"));
    assert!(o.events.iter().any(|e| e.kind == "launcher:release"));
}

/// Plataforma giratória forte demais para o foguete de papel: o aviso estrutural aparece
/// e o foguete sai girando com o braço (e capota).
#[test]
fn spin_launch_overloads_and_tumbles() {
    let mut d = templates::competition_500m();
    d.launcher = vec![astra_launch::Launcher::Spin(astra_launch::Spin {
        radius: 5.0,
        arm_mass: 200.0,
        torque: 20_000.0,
        target_rpm: 400.0,
        chamber_pressure: 0.0,
        arm_cd_width: 0.2,
        release_elevation_deg: 80.0,
        azimuth_deg: 0.0,
        hub_height: 6.0,
        ignition_delay: 0.5,
    })];
    let o = simulate(&d, &db());
    let rep = o.launcher.clone().unwrap();
    assert!(rep.max_lateral_g > 800.0);
    assert!(
        o.warnings.iter().any(|w| w.contains("lateral")),
        "{:?}",
        o.warnings
    );
    assert!(rep.energy.residual < 1e-8);
    let alpha_max = o.bodies[0]
        .samples
        .iter()
        .take(60)
        .map(|r| r[8])
        .filter(|a| a.is_finite())
        .fold(0.0, f64::max);
    assert!(
        alpha_max > 30.0,
        "sai girando a {:.0} rad/s: α {alpha_max}",
        400.0 * 2.0 * core::f64::consts::PI / 60.0
    );
}

/// Plugin da comunidade dentro do projeto, ida e volta pelo JSON.
#[test]
fn plugin_launcher_in_design() {
    let mut d = templates::competition_500m();
    let spec = astra_launch::builtin_plugins().remove(0);
    let mut values = std::collections::BTreeMap::new();
    values.insert("mc".to_owned(), 5.0);
    d.launcher = vec![astra_launch::Launcher::Plugin {
        spec: Box::new(spec),
        values,
    }];
    let back = Design::from_json(&d.to_json()).unwrap();
    assert_eq!(back, d);
    let o = simulate(&back, &db());
    let rep = o.launcher.unwrap();
    assert!(
        rep.release_speed > 50.0 && rep.apogee_with > rep.apogee_without,
        "{} {}",
        rep.apogee_with,
        rep.apogee_without
    );
}

mod electronics {
    use astra_design::elec::{analyze, library, PyroEvent};
    use astra_design::{simulate, templates, validate, Assembly, MaterialDb};

    #[test]
    fn library_loads_and_example_is_clean() {
        let lib = library();
        assert!(lib.len() >= 25);
        let db = MaterialDb::builtin();
        let d = templates::competition_avionics();
        let v = validate(&d, &db, false);
        let elec: Vec<_> = v
            .issues
            .iter()
            .filter(|i| i.code != "pyroNoFirmware")
            .map(|i| (i.code.clone(), i.message.clone()))
            .collect();
        let rep = v.electronics.expect("relatório elétrico");
        assert_eq!(rep.pyro.len(), 1);
        assert!(rep.pyro[0].fires, "{:?}", rep.pyro[0]);
        // I = 7,4 V / (1,6 + 0,06 + 0,025) Ω ≈ 4,39 A.
        assert!(
            (rep.pyro[0].current - 4.39).abs() < 0.02,
            "{}",
            rep.pyro[0].current
        );
        assert!(!rep.pyro[0].brownout);
        assert!(rep
            .devices
            .iter()
            .any(|x| x.lib == "bmp280-modulo" && x.on_bus && x.address == Some(0x76)));
        assert!(
            !elec.iter().any(|(c, _)| [
                "ematchOnPin",
                "ematchCurrent",
                "unpowered",
                "noGround",
                "i2cConflict",
                "brownout",
                "battery"
            ]
            .contains(&c.as_str())),
            "{elec:?}"
        );
        // 500 mAh / (20 + 1) mA ≈ 23,8 h.
        assert!((rep.autonomy_h - 500.0 / 21.0).abs() < 0.1);
    }

    #[test]
    fn wiring_mistakes_are_found() {
        let db = MaterialDb::builtin();
        let base = templates::competition_avionics();
        let codes = |d: &astra_design::Design| {
            let a = Assembly::new(d, &db);
            analyze(d, &a.props)
                .issues
                .into_iter()
                .map(|i| i.code)
                .collect::<Vec<_>>()
        };
        // E-match direto no pino D3, sem MOSFET.
        let mut d = base.clone();
        d.electronics
            .wires
            .retain(|w| !w.a.starts_with("q1") && !w.b.starts_with("q1"));
        d.electronics.wires.push(astra_design::elec::Wire {
            a: "em1:B".into(),
            b: "nano:D3".into(),
            loose_g: None,
        });
        assert!(codes(&d).contains(&"ematchOnPin".to_owned()));
        // Bateria 9 V comum: resistência interna derruba a tensão no disparo.
        let mut d = base.clone();
        d.electronics.components[2].lib = "bateria-9v".into();
        assert!(codes(&d).contains(&"brownout".to_owned()));
        // BMP280 nu (3,6 V máx.) ligado no 5 V.
        let mut d = base.clone();
        d.electronics.components[1].lib = "bmp280".into();
        assert!(codes(&d).contains(&"overvoltage".to_owned()));
        // Dois sensores no mesmo endereço.
        let mut d = base.clone();
        d.electronics.components.push(astra_design::elec::EComp {
            id: "baro2".into(),
            lib: "bmp280-modulo".into(),
            part: "altimetro".into(),
            i2c_address: None,
            charge: None,
        });
        for (a, b) in [
            ("baro2:VCC", "nano:5V"),
            ("baro2:GND", "nano:GND"),
            ("baro2:SDA", "nano:A4"),
            ("baro2:SCL", "nano:A5"),
        ] {
            d.electronics.wires.push(astra_design::elec::Wire {
                a: a.into(),
                b: b.into(),
                loose_g: None,
            });
        }
        assert!(codes(&d).contains(&"i2cConflict".to_owned()));
        // Sem GND no sensor.
        let mut d = base.clone();
        d.electronics.wires.retain(|w| w.a != "baro:GND");
        assert!(codes(&d).contains(&"noGround".to_owned()));
        // Bateria quase vazia.
        let mut d = base;
        d.electronics.components[2].charge = Some(0.02);
        assert!(codes(&d).contains(&"battery".to_owned()));
    }

    #[test]
    fn pyro_deploys_only_when_fired_and_writes_sensor_truth() {
        let db = MaterialDb::builtin();
        let mut d = templates::competition_avionics();
        let never = simulate(&d, &db);
        assert!(
            !never
                .events
                .iter()
                .any(|e| e.kind.contains("paraquedas") || e.kind == "deploy"),
            "{:?}",
            never.events.iter().map(|e| &e.kind).collect::<Vec<_>>()
        );
        let apo = never
            .events
            .iter()
            .find(|e| e.kind == "apogee")
            .map(|e| e.t)
            .unwrap();
        // Verdade dos sensores a 100 Hz.
        assert!(never.sensors.len() > 100);
        let dt = never.sensors[2][0] - never.sensors[1][0];
        assert!((dt - 0.01).abs() < 1e-6, "{dt}");
        // Na ignição o empuxo já supera o peso: força específica axial > 1 g.
        let f0 = never.sensors[0];
        assert!(f0[6] > 9.8, "{f0:?}");
        // No voo livre (depois da queima, antes do apogeu) só o arrasto age: o acelerômetro
        // mede desaceleração axial, e a pressão cai com a altura.
        let burnout = never
            .events
            .iter()
            .find(|e| e.kind == "burnout")
            .map(|e| e.t)
            .unwrap();
        let coast = never.sensors.iter().find(|r| r[0] > burnout + 0.5).unwrap();
        assert!(coast[6] < -1.0 && coast[6] > -40.0, "{coast:?}");
        assert!(coast[7].abs() < 0.5 && coast[8].abs() < 0.5, "{coast:?}");
        assert!(coast[1] < f0[1]);
        d.electronics.pyro_events = vec![PyroEvent {
            ematch: "em1".into(),
            t: apo + 0.5,
        }];
        let fired = simulate(&d, &db);
        let land = |o: &astra_design::SimOutput| {
            o.bodies
                .iter()
                .filter_map(|b| b.landing)
                .map(|l| l[1])
                .fold(0.0, f64::max)
        };
        assert!(
            land(&fired) < 0.6 * land(&never),
            "{} vs {}",
            land(&fired),
            land(&never)
        );
    }
}
