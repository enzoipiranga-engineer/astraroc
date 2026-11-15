//! Foguetes de exemplo.

use crate::{DragModel, Parachute, Rocket};
use astra_aero::{BodyPart, FinPlanform, FinProfile, FinSet, Geometry, NoseShape};
use astra_dynamics::flight::Trigger;
use astra_propulsion::{parse_eng, Motor, WaterRocket};

fn csv_table(text: &str) -> Vec<(f64, f64)> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split(',').map(|v| v.trim().parse::<f64>());
            match (it.next(), it.next()) {
                (Some(Ok(a)), Some(Ok(b))) => Some((a, b)),
                _ => None,
            }
        })
        .collect()
}

/// Calisto, o foguete de exemplo da documentação do RocketPy (equipe Projeto Jupiter),
/// com motor Cesaroni M1670 e as curvas de arrasto fornecidas lá.
///
/// Dados de `docs/user/first_simulation.rst` do RocketPy: massa sem motor 14,426 kg,
/// inércia (6,321; 0,034) kg·m², raio 63,5 mm, nariz von Kármán de 0,55829 m, quatro
/// aletas (raiz 0,120, ponta 0,060, envergadura 0,110 m), cauda cônica de 60 mm até
/// 87 mm de diâmetro, motor seco de 1,815 kg, cinco grãos (2,956 kg de propelente).
/// Posições convertidas para estações a partir da ponta do nariz (CG sem motor em
/// 1,278 m; tubeira em 2,533 m). Paraquedas: drogue Cd·S = 1,0 m² 1,5 s após o apogeu,
/// principal Cd·S = 10 m² a 800 m do solo.
///
/// # Panics
/// Nunca: os arquivos embutidos são válidos (coberto por teste).
#[must_use]
pub fn calisto() -> Rocket {
    let eng = &parse_eng(include_str!("../../../data/motors/Cesaroni_M1670.eng"))
        .expect("arquivo embutido")[0];
    let curve: Vec<(f64, f64)> = eng.points.iter().map(|p| (p.t, p.thrust)).collect();
    let motor = Motor::from_thrust_curve(
        &eng.name,
        &eng.manufacturer,
        0.075,
        0.757,
        2.956,
        1.815,
        &curve,
    );
    let geometry = Geometry {
        parts: vec![
            BodyPart::Nose {
                shape: NoseShape::VonKarman,
                length: 0.558_29,
                diameter: 0.127,
            },
            BodyPart::Tube {
                length: 1.278 + 1.194_656 - 0.558_29,
                diameter: 0.127,
            },
            BodyPart::Transition {
                length: 0.060,
                d_fore: 0.127,
                d_aft: 0.087,
            },
        ],
        fins: vec![FinSet {
            count: 4,
            planform: FinPlanform::Trapezoid {
                root_chord: 0.120,
                tip_chord: 0.060,
                sweep: 0.060,
            },
            span: 0.110,
            thickness: 0.005,
            position: 1.278 + 1.049_56,
            profile: FinProfile::Rounded,
        }],
        roughness: 2e-5,
        nozzle_exit_diameter: 0.066,
    };
    let mut r = Rocket::new(geometry, 14.426, 1.278, (0.034, 6.321), motor);
    r.nozzle_station = 1.278 + 1.255;
    r.case_cg_from_nozzle = 0.317;
    r.prop_cg_from_nozzle = 0.397;
    r.drag = DragModel::Table {
        power_off: csv_table(include_str!(
            "../../../data/rockets/calisto/powerOffDragCurve.csv"
        )),
        power_on: csv_table(include_str!(
            "../../../data/rockets/calisto/powerOnDragCurve.csv"
        )),
    };
    r.parachutes = vec![
        Parachute {
            name: "drogue".into(),
            cd_s: 1.0,
            trigger: Trigger::AfterApogee { delay: 1.5 },
            inflation_time: 0.3,
            reef: None,
        },
        Parachute {
            name: "principal".into(),
            cd_s: 10.0,
            trigger: Trigger::DescendingThrough { agl: 800.0 },
            inflation_time: 1.0,
            reef: None,
        },
    ];
    r
}

/// Foguete de garrafa PET de 2 L com ogiva lastreada e quatro aletas que passam do
/// bocal, sem paraquedas. As dimensões e a massa (150 g vazio) são um exemplo editável,
/// não um projeto medido. As aletas grandes e recuadas são necessárias: com a água
/// junto ao bocal, o CG na decolagem fica bem atrás do CG vazio, e a margem estática
/// cheia é pequena (cerca de meio calibre) até a água sair, em ~0,1 s.
#[must_use]
pub fn pet_bottle(water: &WaterRocket) -> Rocket {
    let d = water.bottle_diameter;
    let geometry = Geometry {
        parts: vec![
            BodyPart::Nose {
                shape: NoseShape::TangentOgive,
                length: 0.15,
                diameter: d,
            },
            BodyPart::Tube {
                length: 0.30,
                diameter: d,
            },
            BodyPart::Transition {
                length: 0.07,
                d_fore: d,
                d_aft: 0.03,
            },
        ],
        fins: vec![FinSet {
            count: 4,
            planform: FinPlanform::Trapezoid {
                root_chord: 0.20,
                tip_chord: 0.10,
                sweep: 0.12,
            },
            span: 0.15,
            thickness: 0.003,
            position: 0.36,
            profile: FinProfile::Square,
        }],
        roughness: 5e-6,
        nozzle_exit_diameter: water.nozzle_diameter,
    };
    let mut r = Rocket::new(
        geometry,
        0.150,
        0.20,
        (1.9e-4, 4.0e-3),
        water.simulate().motor,
    );
    // A água fica junto ao bocal: CG da coluna a meia altura dela, acima do gargalo.
    let column = water.water_volume / (0.25 * core::f64::consts::PI * d * d);
    r.prop_cg_from_nozzle = 0.07 + 0.5 * column;
    r.case_cg_from_nozzle = 0.07;
    r
}
