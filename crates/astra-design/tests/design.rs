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
