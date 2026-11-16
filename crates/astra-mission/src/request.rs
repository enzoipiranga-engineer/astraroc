//! Pedido de missão vindo da interface (JSON).

use crate::templates;
use crate::vehicle::{EngineOut, Mission, PitchKick};
use serde::{Deserialize, Serialize};

/// Falha de motor pedida.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineOutRequest {
    /// Estágio (0 = primeiro).
    pub stage: usize,
    /// Segundos após a ignição do estágio.
    pub after: f64,
    /// Motores perdidos.
    pub engines: u32,
}

/// Pedido de missão a partir de um lançador de exemplo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionRequest {
    /// "falcon9", "starship" ou "saturnv".
    pub vehicle: String,
    /// Carga útil, kg.
    pub payload: f64,
    /// Altitude da órbita circular alvo, m.
    pub target_altitude: f64,
    /// Azimute de lançamento, graus.
    pub azimuth_deg: f64,
    /// Latitude do local, graus.
    pub lat_deg: f64,
    /// Longitude do local, graus.
    pub lon_deg: f64,
    /// Altitude do local, m.
    pub altitude: f64,
    /// Data e hora UTC.
    #[serde(default)]
    pub utc: Option<String>,
    /// Falha de motor.
    #[serde(default)]
    pub engine_out: Option<EngineOutRequest>,
    /// Ângulo da manobra inicial (graus); ausente = automático.
    #[serde(default)]
    pub kick_deg: Option<f64>,
    /// Propelente reservado no primeiro estágio para recuperação, fração (0 a 0,3).
    #[serde(default)]
    pub recovery_reserve: f64,
}

impl MissionRequest {
    /// Monta a missão.
    ///
    /// # Errors
    /// Mensagem quando o lançador não existe.
    pub fn to_mission(&self) -> Result<Mission, String> {
        let mut v = match self.vehicle.as_str() {
            "falcon9" => templates::falcon9(self.payload),
            "starship" => templates::starship(self.payload),
            "saturnv" => templates::saturn_v(self.payload),
            other => return Err(format!("lançador desconhecido: {other}")),
        };
        let reserve = self.recovery_reserve.clamp(0.0, 0.3);
        v.stages[0].reserve = reserve * v.stages[0].propellant;
        if let Some(e) = self.engine_out {
            if let Some(s) = v.stages.get_mut(e.stage) {
                s.engine_out = Some(EngineOut {
                    after: e.after,
                    engines: e.engines,
                });
            }
        }
        let mut m = templates::mission(v, self.target_altitude);
        m.site = (self.lat_deg, self.lon_deg, self.altitude);
        m.azimuth_deg = self.azimuth_deg;
        m.utc.clone_from(&self.utc);
        if let Some(k) = self.kick_deg {
            m.auto_kick = false;
            m.kick = PitchKick {
                angle_deg: k,
                ..m.kick
            };
        }
        Ok(m)
    }
}
