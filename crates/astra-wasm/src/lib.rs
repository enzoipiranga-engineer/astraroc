//! Fronteira WebAssembly do núcleo ASTRA.
//!
//! Este crate só traduz tipos entre JS e Rust. Nenhuma equação de física mora aqui.

use astra_dynamics::flight::{simulate3, Config3, ConstantVehicle, EventKind, Launch, Rail};
use astra_dynamics::ode::Options;
use astra_env::{AtmosphereModel, Geodetic, Planet};
use astra_math::{constants, Quat, Vec3};
use wasm_bindgen::prelude::*;

/// Versão do núcleo (a do workspace Cargo).
#[wasm_bindgen(js_name = coreVersion)]
#[must_use]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Autoteste numérico executado dentro do runtime WASM do usuário.
///
/// Roda um vetor por um quaternion e desfaz a rotação; devolve o erro absoluto (m)
/// de ida e volta. Serve para confirmar que o núcleo carregado produz aritmética
/// IEEE 754 de dupla precisão como no build nativo (esperado: < 1e-12).
#[wasm_bindgen(js_name = selfCheck)]
#[must_use]
pub fn self_check() -> f64 {
    let q = Quat::from_axis_angle(Vec3::new(1.0, 2.0, -0.5), 1.234);
    let v = Vec3::new(constants::wgs84::A, -4.0e3, 2.2e5);
    (q.conj().rotate(q.rotate(v)) - v).norm()
}

/// Estado da atmosfera padrão (USSA 1976) na altitude geométrica `altitude` (m), com
/// desvio de temperatura `delta_t` (K). Devolve [T (K), p (Pa), ρ (kg/m³), a (m/s), μ (Pa·s)].
#[wasm_bindgen(js_name = atmosphere)]
#[must_use]
pub fn atmosphere(altitude: f64, delta_t: f64) -> Vec<f64> {
    let s = AtmosphereModel::Ussa1976 { delta_t }.at(altitude);
    vec![
        s.temperature,
        s.pressure,
        s.density,
        s.sound_speed,
        s.viscosity,
    ]
}

/// Telemetria de um voo, em colunas achatadas.
#[wasm_bindgen]
pub struct FlightOutput {
    samples: Vec<f64>,
    events: Vec<f64>,
}

#[wasm_bindgen]
impl FlightOutput {
    /// Amostras, 8 valores por linha: t (s), altitude (m), velocidade (m/s), velocidade
    /// vertical (m/s), Mach, pressão dinâmica (Pa), latitude (rad), longitude (rad).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn samples(&self) -> Vec<f64> {
        self.samples.clone()
    }

    /// Eventos, 4 valores por linha: tipo (0 saída do trilho, 1 fim da queima, 2 apogeu,
    /// 3 impacto), t (s), altitude (m), velocidade (m/s).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn events(&self) -> Vec<f64> {
        self.events.clone()
    }
}

/// Voo 3-DOF de empuxo constante na Terra (WGS 84, USSA 1976).
///
/// `p` tem 14 valores em SI (ângulos em rad): massa inicial, empuxo no vácuo, duração da
/// queima, vazão, área de saída, Cd·A, latitude, longitude, altitude do local,
/// comprimento do trilho, elevação, azimute, duração máxima, intervalo de saída.
///
/// # Errors
/// Mensagem de texto se `p` tiver tamanho errado ou a integração falhar.
#[wasm_bindgen(js_name = flight3Constant)]
pub fn flight3_constant(p: &[f64]) -> Result<FlightOutput, String> {
    let &[m0, thrust_vac, burn_time, mdot, exit_area, cd_a, lat, lon, h, length, elevation, azimuth, t_max, output_dt] =
        p
    else {
        return Err(format!("esperados 14 parâmetros, recebidos {}", p.len()));
    };
    let vehicle = ConstantVehicle {
        thrust_vac,
        exit_area,
        mdot,
        burn_time,
        cd_a,
    };
    let cfg = Config3 {
        planet: Planet::EARTH,
        atmosphere: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
        vehicle: &vehicle,
        launch: Launch {
            site: Geodetic { lat, lon, h },
            ground_h: h,
            rail: Rail {
                length,
                elevation,
                azimuth,
                friction: 0.0,
            },
            initial_speed: 0.0,
            theta0: 0.0,
        },
        m0,
        t_max,
        output_dt,
        ode: Options {
            h_max: 1.0,
            ..Options::default()
        },
    };
    let res = simulate3(&cfg).map_err(|e| e.to_string())?;
    let samples = res
        .samples
        .iter()
        .flat_map(|s| {
            let n = s.nav;
            [
                s.t,
                n.alt,
                n.speed,
                n.vertical_speed,
                n.mach,
                n.q_dyn,
                n.lat,
                n.lon,
            ]
        })
        .collect();
    let events = res
        .events
        .iter()
        .flat_map(|(k, s)| {
            let code = match k {
                EventKind::RailExit => 0.0,
                EventKind::Burnout => 1.0,
                EventKind::Apogee => 2.0,
                EventKind::Impact => 3.0,
            };
            [code, s.t, s.nav.alt, s.nav.speed]
        })
        .collect();
    Ok(FlightOutput { samples, events })
}

#[cfg(test)]
mod tests {
    #[test]
    fn self_check_is_tight_natively() {
        assert!(super::self_check() < 1e-8);
    }
}
