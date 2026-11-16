//! Projetos de exemplo, prontos para editar.

use crate::model::*;

fn part(id: &str, name: &str, material: &str, position: f64, kind: PartKind) -> Part {
    Part {
        id: id.into(),
        name: name.into(),
        material: material.into(),
        position,
        mass_override: None,
        cost: None,
        kind,
    }
}

fn glue(id: &str, a: &str, b: &str, adhesive: &str) -> Joint {
    Joint {
        id: id.into(),
        a: a.into(),
        b: b.into(),
        kind: JointKind::Adhesive {
            adhesive: adhesive.into(),
            area: None,
            fillet: None,
            through_wall: false,
        },
    }
}

fn cord(id: &str, a: &str, b: &str, strength: f64) -> Joint {
    Joint {
        id: id.into(),
        a: a.into(),
        b: b.into(),
        kind: JointKind::Cord { strength },
    }
}

/// Curva de empuxo de exemplo para um motor de 29 mm classe F. **Não é um motor
/// comercial**: é uma curva inventada, de forma típica, para o modelo de exemplo. Troque
/// pelo arquivo .eng do motor que você vai usar.
pub const EXAMPLE_G_MOTOR: &str = "\
; Curva de EXEMPLO criada para o ASTRA. Não corresponde a nenhum motor comercial.
EXEMPLO-F 29 124 0 0.047 0.120 ASTRA
0.02 117
0.10 109
0.60 90
0.90 74
1.00 31
1.06 0
;
";

/// Garrafa PET de 2 L com coifa impressa, lastro e quatro aletas.
#[must_use]
pub fn pet_bottle() -> Design {
    Design {
        version: FORMAT_VERSION,
        name: "Foguete de garrafa PET (2 L)".into(),
        parts: vec![
            part(
                "coifa",
                "Coifa",
                "pla",
                0.0,
                PartKind::NoseCone {
                    shape: NoseShapeSpec::TangentOgive,
                    length: 0.15,
                    diameter: 0.10,
                    wall: 0.0012,
                    shoulder_length: 0.02,
                },
            ),
            part(
                "lastro",
                "Lastro da coifa",
                "aco-1020",
                0.08,
                PartKind::Mass {
                    mass: 0.15,
                    length: 0.04,
                    diameter: 0.045,
                    needs_static_port: false,
                },
            ),
            part(
                "garrafa",
                "Garrafa PET",
                "pet-garrafa",
                0.15,
                PartKind::BodyTube {
                    length: 0.30,
                    outer_diameter: 0.10,
                    wall: 0.0004,
                    vent_holes: 0,
                    vent_diameter: 0.003,
                },
            ),
            part(
                "gargalo",
                "Gargalo da garrafa",
                "pet-garrafa",
                0.45,
                PartKind::Transition {
                    length: 0.07,
                    d_fore: 0.10,
                    d_aft: 0.03,
                    wall: 0.0006,
                },
            ),
            part(
                "aletas",
                "Aletas",
                "balsa",
                0.38,
                PartKind::FinSet {
                    count: 4,
                    root_chord: 0.16,
                    tip_chord: 0.08,
                    sweep: 0.10,
                    span: 0.16,
                    thickness: 0.004,
                    profile: FinProfileSpec::Square,
                },
            ),
        ],
        joints: vec![
            glue("j-coifa", "coifa", "garrafa", "epoxi-5min"),
            glue("j-lastro", "lastro", "coifa", "epoxi-5min"),
            Joint {
                id: "j-garrafa".into(),
                a: "garrafa".into(),
                b: "gargalo".into(),
                // Mesma peça soprada: representada como colagem de grande área.
                kind: JointKind::Adhesive {
                    adhesive: "epoxi-estrutural".into(),
                    area: Some(0.01),
                    fillet: None,
                    through_wall: false,
                },
            },
            Joint {
                id: "j-aletas".into(),
                a: "aletas".into(),
                b: "garrafa".into(),
                kind: JointKind::Adhesive {
                    adhesive: "epoxi-5min".into(),
                    area: None,
                    fillet: Some(0.004),
                    through_wall: false,
                },
            },
        ],
        motor: MotorSpec::Water {
            bottle_volume: 2.0e-3,
            water_volume: 0.66e-3,
            gauge_pressure: 4.0e5,
            nozzle_diameter: 0.0215,
            max_safe_pressure: 7.0e5,
            aft_station: 0.52,
        },
        launch: LaunchSpec {
            lat_deg: -23.55,
            lon_deg: -46.63,
            altitude: 760.0,
            rail_length: 0.5,
            rail_elevation_deg: 88.0,
            rail_azimuth_deg: 0.0,
            delta_t: 0.0,
            utc: None,
            wind: WindSpec::default(),
        },
        roughness: 5e-6,
        safety_factor: 1.5,
        drag_table: None,
        materials: Vec::new(),
        zones: Vec::new(),
        launcher: Vec::new(),
        electronics: crate::elec::Electronics::default(),
        payloads: Vec::new(),
        competition: None,
        finance: serde_json::Value::Null,
        scales: Scales::default(),
    }
}

/// Foguete de competição universitária para a faixa de 500 m: coifa com pinos de
/// cisalhamento, baia de paraquedas, baia de aviônica com altímetro, motor de 29 mm.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn competition_500m() -> Design {
    Design {
        version: FORMAT_VERSION,
        name: "Competição 500 m (exemplo)".into(),
        parts: vec![
            part(
                "coifa",
                "Coifa",
                "petg",
                0.0,
                PartKind::NoseCone {
                    shape: NoseShapeSpec::TangentOgive,
                    length: 0.20,
                    diameter: 0.056,
                    wall: 0.0015,
                    shoulder_length: 0.06,
                },
            ),
            part(
                "tubo-sup",
                "Tubo superior",
                "papel-fenolico",
                0.20,
                PartKind::BodyTube {
                    length: 0.35,
                    outer_diameter: 0.056,
                    wall: 0.0015,
                    vent_holes: 1,
                    vent_diameter: 0.003,
                },
            ),
            part(
                "paraquedas",
                "Paraquedas",
                "ripstop-1.1",
                0.30,
                PartKind::Parachute {
                    diameter: 0.7,
                    cd: 0.8,
                    packed_length: 0.09,
                    packed_diameter: 0.045,
                    deploy: DeploySpec::Apogee { delay: 1.0 },
                    separation_joint: Some("j-coifa".into()),
                    ejection_charge: 0.0008,
                },
            ),
            part(
                "cordao",
                "Cordão de choque",
                "kevlar-tecido",
                0.40,
                PartKind::ShockCord {
                    length: 2.0,
                    linear_density: 0.012,
                    strength: 2000.0,
                },
            ),
            part(
                "luva",
                "Luva da aviônica",
                "papel-fenolico",
                0.48,
                PartKind::Coupler {
                    length: 0.15,
                    outer_diameter: 0.053,
                    wall: 0.0015,
                },
            ),
            part(
                "antep-sup",
                "Anteparo superior",
                "compensado-betula",
                0.48,
                PartKind::Bulkhead {
                    diameter: 0.050,
                    thickness: 0.006,
                },
            ),
            part(
                "antep-inf",
                "Anteparo inferior",
                "compensado-betula",
                0.624,
                PartKind::Bulkhead {
                    diameter: 0.050,
                    thickness: 0.006,
                },
            ),
            part(
                "altimetro",
                "Altímetro",
                "g10-fr4",
                0.49,
                PartKind::Mass {
                    mass: 0.030,
                    length: 0.06,
                    diameter: 0.040,
                    needs_static_port: true,
                },
            ),
            part(
                "bateria",
                "Bateria 9 V",
                "aco-1020",
                0.565,
                PartKind::Mass {
                    mass: 0.045,
                    length: 0.05,
                    diameter: 0.030,
                    needs_static_port: false,
                },
            ),
            part(
                "tubo-inf",
                "Tubo inferior",
                "papel-fenolico",
                0.55,
                PartKind::BodyTube {
                    length: 0.45,
                    outer_diameter: 0.056,
                    wall: 0.0015,
                    vent_holes: 3,
                    vent_diameter: 0.003,
                },
            ),
            part(
                "tubo-motor",
                "Tubo do motor",
                "papel-fenolico",
                0.80,
                PartKind::MotorMount {
                    length: 0.20,
                    outer_diameter: 0.031,
                    wall: 0.001,
                },
            ),
            part(
                "anel-1",
                "Anel de centragem dianteiro",
                "compensado-betula",
                0.81,
                PartKind::CenteringRing {
                    outer_diameter: 0.053,
                    inner_diameter: 0.031,
                    thickness: 0.003,
                },
            ),
            part(
                "anel-2",
                "Anel de centragem traseiro",
                "compensado-betula",
                0.987,
                PartKind::CenteringRing {
                    outer_diameter: 0.053,
                    inner_diameter: 0.031,
                    thickness: 0.003,
                },
            ),
            part(
                "aletas",
                "Aletas",
                "g10-fr4",
                0.89,
                PartKind::FinSet {
                    count: 3,
                    root_chord: 0.10,
                    tip_chord: 0.045,
                    sweep: 0.06,
                    span: 0.065,
                    thickness: 0.002,
                    profile: FinProfileSpec::Rounded,
                },
            ),
        ],
        joints: vec![
            Joint {
                id: "j-coifa".into(),
                a: "coifa".into(),
                b: "tubo-sup".into(),
                kind: JointKind::ShearPins {
                    count: 2,
                    diameter: 0.0016,
                    material: "nylon-66".into(),
                },
            },
            cord("j-cordao-coifa", "cordao", "coifa", 1500.0),
            cord("j-cordao-antep", "cordao", "antep-sup", 1500.0),
            cord("j-paraquedas", "paraquedas", "cordao", 1000.0),
            glue("j-luva-sup", "luva", "tubo-sup", "epoxi-estrutural"),
            glue("j-luva-inf", "luva", "tubo-inf", "epoxi-estrutural"),
            glue("j-antep-sup", "antep-sup", "luva", "epoxi-estrutural"),
            glue("j-antep-inf", "antep-inf", "luva", "epoxi-estrutural"),
            Joint {
                id: "j-altimetro".into(),
                a: "altimetro".into(),
                b: "antep-sup".into(),
                kind: JointKind::Screws {
                    count: 2,
                    diameter: 0.003,
                    material: "aco-parafuso-8.8".into(),
                },
            },
            Joint {
                id: "j-bateria".into(),
                a: "bateria".into(),
                b: "antep-inf".into(),
                kind: JointKind::Latch {
                    holding_force: 40.0,
                },
            },
            glue("j-anel-1-motor", "anel-1", "tubo-motor", "epoxi-estrutural"),
            glue("j-anel-2-motor", "anel-2", "tubo-motor", "epoxi-estrutural"),
            glue("j-anel-1-tubo", "anel-1", "tubo-inf", "epoxi-estrutural"),
            glue("j-anel-2-tubo", "anel-2", "tubo-inf", "epoxi-estrutural"),
            Joint {
                id: "j-aletas".into(),
                a: "aletas".into(),
                b: "tubo-inf".into(),
                kind: JointKind::Adhesive {
                    adhesive: "epoxi-estrutural".into(),
                    area: None,
                    fillet: Some(0.005),
                    through_wall: false,
                },
            },
        ],
        motor: MotorSpec::Eng {
            text: EXAMPLE_G_MOTOR.into(),
            aft_station: 1.0,
        },
        launch: LaunchSpec {
            lat_deg: -22.18,
            lon_deg: -49.38,
            altitude: 470.0,
            rail_length: 2.0,
            rail_elevation_deg: 85.0,
            rail_azimuth_deg: 0.0,
            delta_t: 0.0,
            utc: None,
            wind: WindSpec::default(),
        },
        roughness: 3e-5,
        safety_factor: 1.5,
        drag_table: None,
        materials: Vec::new(),
        zones: Vec::new(),
        launcher: Vec::new(),
        electronics: crate::elec::Electronics::default(),
        payloads: Vec::new(),
        competition: None,
        finance: serde_json::Value::Null,
        scales: Scales::default(),
    }
}

/// Todos os exemplos, com identificador.
#[must_use]
pub fn all() -> Vec<(&'static str, Design)> {
    vec![
        ("pet", pet_bottle()),
        ("competicao-500m", competition_500m()),
        ("competicao-avionica", competition_avionics()),
        ("competicao-carga", competition_payload()),
        ("kit-18mm", model_kit()),
        ("nivel-1", level_1()),
        ("nivel-2", level_2()),
        ("nivel-3", level_3()),
    ]
}

/// Como o nariz é preso e como a baia abre.
#[derive(Clone, Copy)]
enum Retention {
    /// Encaixe por atrito (foguetes-modelo), N.
    Friction(f64),
    /// Pinos de cisalhamento de náilon: quantidade e diâmetro, m.
    Pins(u32, f64),
}

/// Parâmetros de um foguete de baia única (nariz, um tubo, motor atrás, paraquedas na frente).
struct Kit {
    name: &'static str,
    diameter: f64,
    wall: f64,
    tube_material: &'static str,
    nose_length: f64,
    nose_material: &'static str,
    tube_length: f64,
    mount_diameter: f64,
    mount_length: f64,
    motor: &'static str,
    /// Aletas: quantidade, raiz, ponta, enflechamento, envergadura, espessura (m).
    fins: (u32, f64, f64, f64, f64, f64),
    fin_material: &'static str,
    main_chute: f64,
    /// Paraquedas principal abre a esta altura (com estabilizador no apogeu), ou None.
    main_altitude: Option<f64>,
    drogue: Option<f64>,
    /// Abertura: tempo desde a ignição (atraso do motor) ou None = no apogeu + 1 s.
    deploy_time: Option<f64>,
    retention: Retention,
    charge: f64,
    cord_strength: f64,
    /// Aviônica: massa (kg), mede pressão.
    avionics: Option<f64>,
}

#[allow(clippy::too_many_lines)]
fn kit(k: &Kit) -> Design {
    let mut d = competition_500m();
    d.name = k.name.into();
    let (dn, tl) = (k.nose_length, k.tube_length);
    let aft = dn + tl;
    let di = k.diameter - 2.0 * k.wall;
    let ring_t = (0.06 * k.diameter).clamp(0.003, 0.012);
    let bulk_x = aft - k.mount_length - ring_t;
    let chute_len = (0.12 * k.main_chute).clamp(0.04, 0.35);
    let mut parts = vec![
        part(
            "coifa",
            "Coifa",
            k.nose_material,
            0.0,
            PartKind::NoseCone {
                shape: NoseShapeSpec::TangentOgive,
                length: dn,
                diameter: k.diameter,
                wall: (0.03 * k.diameter).clamp(0.001, 0.004),
                shoulder_length: k.diameter,
            },
        ),
        part(
            "tubo",
            "Tubo",
            k.tube_material,
            dn,
            PartKind::BodyTube {
                length: tl,
                outer_diameter: k.diameter,
                wall: k.wall,
                vent_holes: 3,
                vent_diameter: (0.03 * k.diameter).clamp(0.0015, 0.004),
            },
        ),
        part(
            "tubo-motor",
            "Tubo do motor",
            "papel-fenolico",
            aft - k.mount_length,
            PartKind::MotorMount {
                length: k.mount_length,
                outer_diameter: k.mount_diameter,
                wall: 0.001,
            },
        ),
        part(
            "anel-1",
            "Anel de centragem dianteiro",
            "compensado-betula",
            aft - k.mount_length,
            PartKind::CenteringRing {
                outer_diameter: di,
                inner_diameter: k.mount_diameter,
                thickness: ring_t,
            },
        ),
        part(
            "anel-2",
            "Anel de centragem traseiro",
            "compensado-betula",
            aft - ring_t,
            PartKind::CenteringRing {
                outer_diameter: di,
                inner_diameter: k.mount_diameter,
                thickness: ring_t,
            },
        ),
        part(
            "anteparo",
            "Anteparo",
            "compensado-betula",
            bulk_x,
            PartKind::Bulkhead {
                diameter: di,
                thickness: ring_t,
            },
        ),
        part(
            "aletas",
            "Aletas",
            k.fin_material,
            aft - k.fins.1,
            PartKind::FinSet {
                count: k.fins.0,
                root_chord: k.fins.1,
                tip_chord: k.fins.2,
                sweep: k.fins.3,
                span: k.fins.4,
                thickness: k.fins.5,
                profile: FinProfileSpec::Rounded,
            },
        ),
        part(
            "cordao",
            "Cordão de choque",
            "kevlar-tecido",
            dn + 0.02 + chute_len,
            PartKind::ShockCord {
                length: (3.0 * (dn + tl)).max(1.0),
                linear_density: if k.cord_strength > 1000.0 {
                    0.012
                } else {
                    0.003
                },
                strength: k.cord_strength,
            },
        ),
    ];
    let main_deploy = match (k.main_altitude, k.deploy_time) {
        (Some(agl), _) => DeploySpec::Altitude { agl },
        (None, Some(t)) => DeploySpec::Time { t },
        (None, None) => DeploySpec::Apogee { delay: 1.0 },
    };
    parts.push(part(
        "paraquedas",
        "Paraquedas principal",
        "ripstop-1.1",
        dn + 0.02,
        PartKind::Parachute {
            diameter: k.main_chute,
            cd: 0.8,
            packed_length: chute_len,
            packed_diameter: 0.8 * di,
            deploy: main_deploy,
            separation_joint: Some("j-coifa".into()),
            ejection_charge: k.charge,
        },
    ));
    if let Some(dd) = k.drogue {
        parts.push(part(
            "estabilizador",
            "Paraquedas estabilizador",
            "ripstop-1.1",
            dn + 0.02 + chute_len,
            PartKind::Parachute {
                diameter: dd,
                cd: 0.8,
                packed_length: 0.05,
                packed_diameter: 0.6 * di,
                deploy: DeploySpec::Apogee { delay: 1.0 },
                separation_joint: Some("j-coifa".into()),
                ejection_charge: k.charge,
            },
        ));
    }
    let mut joints = vec![
        Joint {
            id: "j-coifa".into(),
            a: "coifa".into(),
            b: "tubo".into(),
            kind: match k.retention {
                Retention::Friction(f) => JointKind::FrictionFit { holding_force: f },
                Retention::Pins(n, dia) => JointKind::ShearPins {
                    count: n,
                    diameter: dia,
                    material: "nylon-66".into(),
                },
            },
        },
        cord("j-cordao-coifa", "cordao", "coifa", k.cord_strength),
        cord("j-cordao-anteparo", "cordao", "anteparo", k.cord_strength),
        cord("j-paraquedas", "paraquedas", "cordao", k.cord_strength),
        glue("j-anel-1-motor", "anel-1", "tubo-motor", "epoxi-estrutural"),
        glue("j-anel-2-motor", "anel-2", "tubo-motor", "epoxi-estrutural"),
        glue("j-anel-1-tubo", "anel-1", "tubo", "epoxi-estrutural"),
        glue("j-anel-2-tubo", "anel-2", "tubo", "epoxi-estrutural"),
        glue("j-anteparo", "anteparo", "tubo", "epoxi-estrutural"),
        Joint {
            id: "j-aletas".into(),
            a: "aletas".into(),
            b: "tubo".into(),
            kind: JointKind::Adhesive {
                adhesive: "epoxi-estrutural".into(),
                area: None,
                fillet: Some((0.1 * k.diameter).clamp(0.003, 0.012)),
                through_wall: false,
            },
        },
    ];
    if k.drogue.is_some() {
        joints.push(cord(
            "j-estabilizador",
            "estabilizador",
            "cordao",
            k.cord_strength,
        ));
    }
    if let Some(m) = k.avionics {
        parts.push(part(
            "avionica",
            "Aviônica (altímetros e bateria)",
            "g10-fr4",
            bulk_x - 0.12,
            PartKind::Mass {
                mass: m,
                length: 0.10,
                diameter: 0.7 * di,
                needs_static_port: true,
            },
        ));
        joints.push(Joint {
            id: "j-avionica".into(),
            a: "avionica".into(),
            b: "anteparo".into(),
            kind: JointKind::Screws {
                count: 4,
                diameter: 0.004,
                material: "aco-parafuso-8.8".into(),
            },
        });
    }
    d.parts = parts;
    d.joints = joints;
    d.motor = MotorSpec::Eng {
        text: k.motor.into(),
        aft_station: aft,
    };
    d
}

/// Foguete-modelo de kit de 25 mm com motor de 18 mm classe C (curva de exemplo), nariz no
/// atrito e ejeção pelo atraso do motor, como nos kits de modelismo.
#[must_use]
pub fn model_kit() -> Design {
    let mut d = kit(&Kit {
        name: "Foguete-modelo de kit, 18 mm (exemplo)",
        diameter: 0.0248,
        wall: 0.0005,
        tube_material: "papelao-espiral",
        nose_length: 0.075,
        nose_material: "abs",
        tube_length: 0.30,
        mount_diameter: 0.0187,
        mount_length: 0.07,
        motor: include_str!("../../../data/motors/exemplo-c.eng"),
        fins: (3, 0.06, 0.03, 0.035, 0.045, 0.0032),
        fin_material: "balsa",
        main_chute: 0.30,
        main_altitude: None,
        drogue: None,
        deploy_time: Some(2.0 + 5.0),
        retention: Retention::Friction(8.0),
        charge: 0.0002,
        cord_strength: 300.0,
        avionics: None,
    });
    d.launch.rail_length = 0.9;
    d
}

/// Certificação de nível 1 (motor classe H, 29 mm; curva de exemplo).
#[must_use]
pub fn level_1() -> Design {
    kit(&Kit {
        name: "Nível 1: motor H de 29 mm (exemplo)",
        diameter: 0.054,
        wall: 0.0015,
        tube_material: "papel-fenolico",
        nose_length: 0.22,
        nose_material: "petg",
        tube_length: 0.80,
        mount_diameter: 0.0315,
        mount_length: 0.20,
        motor: include_str!("../../../data/motors/exemplo-h.eng"),
        fins: (3, 0.12, 0.05, 0.07, 0.07, 0.003),
        fin_material: "g10-fr4",
        main_chute: 0.9,
        main_altitude: None,
        drogue: None,
        deploy_time: None,
        retention: Retention::Pins(2, 0.002),
        charge: 0.0016,
        cord_strength: 2000.0,
        avionics: Some(0.08),
    })
}

/// Certificação de nível 2 (motor classe J, 38 mm; curva de exemplo), fibra de vidro.
#[must_use]
pub fn level_2() -> Design {
    kit(&Kit {
        name: "Nível 2: motor J de 38 mm (exemplo)",
        diameter: 0.098,
        wall: 0.002,
        tube_material: "gfrp-qi",
        nose_length: 0.40,
        nose_material: "gfrp-qi",
        tube_length: 1.30,
        mount_diameter: 0.040,
        mount_length: 0.40,
        motor: include_str!("../../../data/motors/exemplo-j.eng"),
        fins: (4, 0.22, 0.08, 0.14, 0.11, 0.004),
        fin_material: "g10-fr4",
        main_chute: 1.5,
        main_altitude: None,
        drogue: None,
        deploy_time: None,
        retention: Retention::Pins(3, 0.002),
        charge: 0.004,
        cord_strength: 4000.0,
        avionics: Some(0.2),
    })
}

/// Certificação de nível 3 com o Cesaroni M1670 (curva real do fabricante), 152 mm, dois
/// paraquedas (estabilizador no apogeu, principal a 300 m).
#[must_use]
pub fn level_3() -> Design {
    kit(&Kit {
        name: "Nível 3: Cesaroni M1670 em 152 mm",
        diameter: 0.152,
        wall: 0.003,
        tube_material: "gfrp-qi",
        nose_length: 0.60,
        nose_material: "gfrp-qi",
        tube_length: 2.00,
        mount_diameter: 0.080,
        mount_length: 0.78,
        motor: include_str!("../../../data/motors/Cesaroni_M1670.eng"),
        fins: (4, 0.35, 0.12, 0.22, 0.16, 0.006),
        fin_material: "g10-fr4",
        main_chute: 2.4,
        main_altitude: Some(300.0),
        drogue: Some(0.6),
        deploy_time: None,
        retention: Retention::Pins(3, 0.002),
        charge: 0.006,
        cord_strength: 9000.0,
        avionics: Some(0.35),
    })
}

/// O foguete de competição levando um pacote de sensores como carga útil, avaliado pelas
/// regras da categoria de 0,5 km da LASC (só o que o LGR define).
#[must_use]
pub fn competition_payload() -> Design {
    use crate::payload::{Payload, PayloadKind, Requirements};
    let mut d = competition_500m();
    d.name = "Competição 500 m com carga útil (exemplo)".into();
    d.parts.push(part(
        "carga",
        "Carga útil (pacote de sensores)",
        "g10-fr4",
        0.50,
        PartKind::Mass {
            mass: 0.15,
            length: 0.04,
            diameter: 0.045,
            needs_static_port: false,
        },
    ));
    d.joints.push(Joint {
        id: "j-carga".into(),
        a: "carga".into(),
        b: "antep-sup".into(),
        kind: JointKind::Screws {
            count: 2,
            diameter: 0.003,
            material: "aco-parafuso-8.8".into(),
        },
    });
    d.payloads.push(Payload {
        id: "sensores".into(),
        name: "Pacote de sensores".into(),
        kind: PayloadKind::Sensors,
        part: "carga".into(),
        functional: true,
        requirements: Requirements {
            max_axial_g: Some(30.0),
            max_lateral_g: Some(10.0),
            max_shock_g: Some(50.0),
            min_temp_c: Some(-10.0),
            max_temp_c: Some(60.0),
            max_bay_dp: Some(1000.0),
            max_landing_speed: Some(8.0),
            ..Requirements::default()
        },
    });
    d.competition = crate::rules::presets()
        .into_iter()
        .find(|(id, _)| id == "lasc-500m")
        .map(|(_, r)| r);
    d
}

/// Firmware de exemplo (`data/firmware/apogeu.c`) compilado para ATmega328P.
pub const EXAMPLE_FIRMWARE_HEX: &str = include_str!("../../../data/firmware/apogeu.hex");

/// O foguete de competição com aviônica própria: Arduino Nano + BMP280 detectam o apogeu
/// e acionam o e-match por um MOSFET, alimentados por uma LiPo 2S com chave de armar.
#[must_use]
pub fn competition_avionics() -> Design {
    use crate::elec::{EComp, Electronics, Firmware, Wire};
    let mut d = competition_500m();
    d.name = "Competição 500 m com aviônica (exemplo)".into();
    for p in &mut d.parts {
        match p.id.as_str() {
            "altimetro" => p.name = "Placa de aviônica (Nano + BMP280)".into(),
            "bateria" => {
                p.name = "LiPo 2S 500 mAh".into();
                if let PartKind::Mass { mass, .. } = &mut p.kind {
                    *mass = 0.030;
                }
            }
            _ => {}
        }
        if let PartKind::Parachute { deploy, .. } = &mut p.kind {
            *deploy = DeploySpec::Pyro {
                ematch: "em1".into(),
            };
        }
    }
    let c = |id: &str, lib: &str, part: &str| EComp {
        id: id.into(),
        lib: lib.into(),
        part: part.into(),
        i2c_address: None,
        charge: None,
    };
    let w = |a: &str, b: &str| Wire {
        a: a.into(),
        b: b.into(),
        loose_g: None,
    };
    d.electronics = Electronics {
        components: vec![
            c("nano", "arduino-nano", "altimetro"),
            c("baro", "bmp280-modulo", "altimetro"),
            c("lipo", "lipo-2s-500", "bateria"),
            c("chave", "chave", "bateria"),
            c("q1", "mosfet-irlz44n", "altimetro"),
            c("em1", "ematch", "paraquedas"),
        ],
        wires: vec![
            w("lipo:+", "chave:A"),
            w("chave:B", "nano:VIN"),
            w("lipo:-", "nano:GND"),
            w("nano:5V", "baro:VCC"),
            w("baro:GND", "nano:GND"),
            w("baro:SDA", "nano:A4"),
            w("baro:SCL", "nano:A5"),
            w("em1:A", "chave:B"),
            w("em1:B", "q1:D"),
            w("q1:S", "nano:GND"),
            w("q1:G", "nano:D3"),
        ],
        firmware: Some(Firmware {
            board: "nano".into(),
            name: "apogeu.hex".into(),
            hex: EXAMPLE_FIRMWARE_HEX.into(),
        }),
        standby_hours: 1.0,
        pyro_events: vec![],
    };
    d
}
