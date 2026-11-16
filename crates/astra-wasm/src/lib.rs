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
    /// 3 impacto,
    /// 10 + i abertura do paraquedas i), t (s), altitude (m), velocidade (m/s).
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
        start: None,
        wind: astra_env::Wind::calm(),
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
        .flat_map(|(k, s)| [event_code(*k), s.t, s.nav.alt, s.nav.speed])
        .collect();
    Ok(FlightOutput { samples, events })
}

/// Voo 6-DOF do foguete de garrafa PET de exemplo, na Terra, com atmosfera padrão.
///
/// `p` tem 7 valores em SI (ângulos em rad): volume da garrafa (m³), volume de água
/// (m³), pressão manométrica (Pa), latitude, longitude, altitude do local (m),
/// elevação do lançador. Devolve a telemetria no formato de [`FlightOutput`]; os avisos
/// do motor de água (pressão acima do limite, água sobrando) vêm em `warnings`.
///
/// # Errors
/// Mensagem de texto se `p` tiver tamanho errado ou a integração falhar.
#[wasm_bindgen(js_name = waterRocketFlight)]
pub fn water_rocket_flight(p: &[f64]) -> Result<WaterFlight, String> {
    use astra_dynamics::flight::{simulate6, Config6};
    let &[bottle_volume, water_volume, gauge_pressure, lat, lon, h, elevation] = p else {
        return Err(format!("esperados 7 parâmetros, recebidos {}", p.len()));
    };
    let water = astra_propulsion::WaterRocket {
        bottle_volume,
        water_volume,
        gauge_pressure,
        ..astra_propulsion::WaterRocket::pet_2l()
    };
    let warnings = water.simulate().warnings;
    let rocket = astra_vehicle::templates::pet_bottle(&water);
    let cfg = Config6 {
        planet: Planet::EARTH,
        atmosphere: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
        vehicle: &rocket,
        launch: Launch {
            site: Geodetic { lat, lon, h },
            ground_h: h,
            rail: Rail {
                length: 0.5,
                elevation,
                azimuth: 0.0,
                friction: 0.0,
            },
            initial_speed: 0.0,
            theta0: 0.0,
        },
        m0: rocket.mass(0.0),
        initial_omega_body: Vec3::ZERO,
        t_max: 120.0,
        output_dt: 0.02,
        ode: Options {
            rtol: 1e-8,
            atol: 1e-8,
            h_max: 0.5,
            ..Options::default()
        },
        start: None,
        wind: astra_env::Wind::calm(),
    };
    let res = simulate6(&cfg).map_err(|e| e.to_string())?;
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
        .flat_map(|(k, s)| [event_code(*k), s.t, s.nav.alt, s.nav.speed])
        .collect();
    Ok(WaterFlight {
        flight: FlightOutput { samples, events },
        warnings,
    })
}

/// Resultado de [`water_rocket_flight`].
#[wasm_bindgen]
pub struct WaterFlight {
    flight: FlightOutput,
    warnings: Vec<String>,
}

#[wasm_bindgen]
impl WaterFlight {
    /// Amostras (ver [`FlightOutput::samples`]).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn samples(&self) -> Vec<f64> {
        self.flight.samples.clone()
    }

    /// Eventos (ver [`FlightOutput::events`]).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn events(&self) -> Vec<f64> {
        self.flight.events.clone()
    }

    /// Avisos em linguagem simples.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }
}

fn event_code(k: EventKind) -> f64 {
    match k {
        EventKind::RailExit => 0.0,
        EventKind::Burnout => 1.0,
        EventKind::Apogee => 2.0,
        EventKind::Impact => 3.0,
        EventKind::Trigger(i) => 10.0 + i as f64,
        EventKind::Limit(i) => 100.0 + i as f64,
    }
}

/// Projetos de exemplo: JSON `[{"id": ..., "design": {...}}]`.
#[wasm_bindgen(js_name = designTemplates)]
#[must_use]
pub fn design_templates() -> String {
    let list: Vec<serde_json::Value> = astra_design::templates::all()
        .into_iter()
        .map(|(id, d)| serde_json::json!({ "id": id, "design": d }))
        .collect();
    serde_json::Value::Array(list).to_string()
}

/// Banco de materiais embutido, no formato de `data/materials.json`.
#[wasm_bindgen(js_name = materialsJson)]
#[must_use]
pub fn materials_json() -> String {
    let db = astra_design::MaterialDb::builtin();
    serde_json::json!({ "materials": db.materials }).to_string()
}

/// Verifica um projeto (JSON). Com `with_flight`, inclui as cargas do voo nominal.
/// Devolve o JSON de `astra_design::Validation`.
///
/// # Errors
/// Mensagem se o JSON do projeto for inválido.
#[wasm_bindgen(js_name = validateDesign)]
pub fn validate_design(json: &str, with_flight: bool) -> Result<String, String> {
    let d = astra_design::Design::from_json(json)?;
    let v = astra_design::validate(&d, &d.material_db(), with_flight);
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

/// Simula um projeto (JSON) com falhas físicas. Devolve o JSON de
/// `astra_design::SimOutput`.
///
/// # Errors
/// Mensagem se o JSON do projeto for inválido.
#[wasm_bindgen(js_name = simulateDesign)]
pub fn simulate_design(json: &str) -> Result<String, String> {
    let d = astra_design::Design::from_json(json)?;
    let o = astra_design::simulate(&d, &d.material_db());
    serde_json::to_string(&o).map_err(|e| e.to_string())
}

/// Simula uma missão orbital (JSON de `astra_mission::MissionRequest`) e devolve o JSON
/// de `astra_mission::MissionOutput`.
///
/// # Errors
/// Mensagem se o pedido for inválido.
#[wasm_bindgen(js_name = simulateMission)]
pub fn simulate_mission(json: &str) -> Result<String, String> {
    let req: astra_mission::MissionRequest =
        serde_json::from_str(json).map_err(|e| format!("pedido inválido: {e}"))?;
    let out = astra_mission::simulate(&req.to_mission()?);
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

/// Biblioteca de motores líquidos (JSON).
#[wasm_bindgen(js_name = engineLibrary)]
#[must_use]
pub fn engine_library_json() -> String {
    let list: Vec<serde_json::Value> = astra_propulsion::engine_library()
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.name, "propellants": e.propellants, "thrustVacKn": e.thrust_vac / 1e3,
                "thrustSlKn": e.thrust(1.0, 101_325.0) / 1e3, "ispVac": e.isp_vac, "ispSl": e.isp(101_325.0),
                "massKg": e.mass, "source": e.source, "reliability": e.reliability
            })
        })
        .collect();
    serde_json::Value::Array(list).to_string()
}

/// Plugins de lançador que acompanham o ASTRA (JSON).
#[wasm_bindgen(js_name = launcherPlugins)]
#[must_use]
pub fn launcher_plugins() -> String {
    serde_json::to_string(&astra_launch::builtin_plugins()).unwrap_or_default()
}

/// Verifica um plugin de lançador (JSON de `PluginSpec`): campos, unidades, expressões,
/// balanço de energia e os testes do pacote. Devolve `{ok, messages}`.
#[wasm_bindgen(js_name = checkPlugin)]
#[must_use]
pub fn check_plugin(json: &str) -> String {
    let check = match serde_json::from_str::<astra_launch::PluginSpec>(json) {
        Ok(p) => p.check(),
        Err(e) => astra_launch::PluginCheck {
            ok: false,
            messages: vec![format!("pacote inválido: {e}")],
        },
    };
    serde_json::to_string(&check).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn self_check_is_tight_natively() {
        assert!(super::self_check() < 1e-8);
    }
}

/// Biblioteca de componentes eletrônicos (`data/electronics.json`).
#[wasm_bindgen(js_name = electronicsLibrary)]
#[must_use]
pub fn electronics_library() -> String {
    serde_json::to_string(&astra_design::elec::library()).unwrap_or_else(|_| "[]".to_owned())
}

/// Firmware de exemplo (Intel HEX) e seu código-fonte em C.
#[wasm_bindgen(js_name = exampleFirmware)]
#[must_use]
pub fn example_firmware() -> String {
    serde_json::json!({
        "name": "apogeu.hex",
        "hex": astra_design::templates::EXAMPLE_FIRMWARE_HEX,
        "source": include_str!("../../../data/firmware/apogeu.c"),
    })
    .to_string()
}

/// Modelos de carga útil.
#[wasm_bindgen(js_name = payloadPresets)]
#[must_use]
pub fn payload_presets() -> String {
    serde_json::to_string(&astra_design::payload::presets()).unwrap_or_else(|_| "[]".to_owned())
}

/// Presets de regras de competição: `[[id, regras], ...]`.
#[wasm_bindgen(js_name = rulePresets)]
#[must_use]
pub fn rule_presets() -> String {
    serde_json::to_string(&astra_design::rules::presets()).unwrap_or_else(|_| "[]".to_owned())
}
