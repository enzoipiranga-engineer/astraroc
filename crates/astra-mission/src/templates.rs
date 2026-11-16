//! Lançadores de exemplo a partir de números públicos aproximados.
//!
//! **Confiabilidade: estimado.** Massas e números de motores foram transcritos de memória
//! de fontes públicas (guias de usuário, apresentações e compilações como a Wikipédia) e
//! não foram conferidos nesta sessão. Servem para estudo e ordem de grandeza.

use crate::vehicle::{Fairing, LaunchVehicle, Mission, PitchKick, Stage};
use astra_propulsion::{engine_library, LiquidEngine};

fn engine(name: &str) -> LiquidEngine {
    engine_library()
        .into_iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| engine_library().remove(0))
}

/// Cd por Mach típico de lançador esbelto (forma das curvas publicadas para Saturn V e
/// Atlas: patamar subsônico, pico transônico, queda supersônica). Estimado.
#[must_use]
pub fn generic_cd() -> Vec<(f64, f64)> {
    vec![
        (0.0, 0.30),
        (0.6, 0.30),
        (0.9, 0.40),
        (1.1, 0.62),
        (1.3, 0.60),
        (2.0, 0.45),
        (3.0, 0.36),
        (5.0, 0.30),
        (10.0, 0.27),
    ]
}

/// Falcon 9 Block 5 descartável: 1º estágio com 9 Merlin 1D, 2º com 1 Merlin Vacuum.
/// Massas aproximadas: 1º estágio 25,6 t seco e 411 t de propelente; 2º estágio 4 t seco
/// e 107,5 t; carenagem 1,9 t. Referência pública de desempenho: 22,8 t em LEO
/// (descartável) a partir de Cabo Canaveral.
#[must_use]
pub fn falcon9(payload: f64) -> LaunchVehicle {
    LaunchVehicle {
        name: "Falcon 9".into(),
        stages: vec![
            Stage {
                name: "1º estágio".into(),
                dry_mass: 25_600.0,
                propellant: 411_000.0,
                engines: vec![(engine("Merlin 1D"), 9)],
                diameter: 3.66,
                length: 41.2,
                max_q: Some(32_000.0),
                max_accel: None,
                engine_out: None,
                reserve: 0.0,
            },
            Stage {
                name: "2º estágio".into(),
                dry_mass: 4_000.0,
                propellant: 107_500.0,
                engines: vec![(engine("Merlin 1D Vacuum"), 1)],
                diameter: 3.66,
                length: 13.8,
                max_q: None,
                max_accel: None,
                engine_out: None,
                reserve: 0.0,
            },
        ],
        boosters: None,
        fairing: Some(Fairing {
            mass: 1_900.0,
            jettison_altitude: 110_000.0,
        }),
        payload,
        cd_table: generic_cd(),
    }
}

/// Starship com Super Heavy (versão de 2024–2025, números públicos aproximados):
/// Super Heavy com 33 Raptor 2, ~200 t seco e ~3400 t de propelente; Starship com 3 Raptor
/// de nível do mar e 3 Raptor Vacuum, ~120 t seco e ~1200 t de propelente. Hot staging.
#[must_use]
pub fn starship(payload: f64) -> LaunchVehicle {
    LaunchVehicle {
        name: "Starship".into(),
        stages: vec![
            Stage {
                name: "Super Heavy".into(),
                dry_mass: 200_000.0,
                propellant: 3_400_000.0,
                engines: vec![(engine("Raptor 2"), 33)],
                diameter: 9.0,
                length: 71.0,
                max_q: Some(35_000.0),
                max_accel: None,
                engine_out: None,
                reserve: 0.0,
            },
            Stage {
                name: "Starship".into(),
                dry_mass: 120_000.0,
                propellant: 1_200_000.0,
                engines: vec![(engine("Raptor Vacuum"), 3), (engine("Raptor 2"), 3)],
                diameter: 9.0,
                length: 52.0,
                max_q: None,
                max_accel: Some(4.0 * 9.806_65),
                engine_out: None,
                reserve: 0.0,
            },
        ],
        boosters: None,
        fairing: None,
        payload,
        cd_table: generic_cd(),
    }
}

/// Missão de exemplo a partir de Cabo Canaveral para LEO circular.
#[must_use]
pub fn mission(vehicle: LaunchVehicle, target_altitude: f64) -> Mission {
    let heavy = vehicle.stages[0].propellant > 1e6;
    Mission {
        vehicle,
        site: (28.5618, -80.5772, 3.0),
        azimuth_deg: 90.0,
        target_altitude,
        kick: PitchKick {
            start: if heavy { 12.0 } else { 8.0 },
            duration: 10.0,
            angle_deg: if heavy { 2.0 } else { 3.0 },
        },
        auto_kick: true,
        coast: if heavy { 0.0 } else { 7.0 },
        utc: Some("2026-09-03T14:00:00Z".into()),
        wind: astra_env::Wind::calm(),
    }
}
