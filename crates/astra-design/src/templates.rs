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
    ]
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
