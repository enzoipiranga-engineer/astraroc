//! Sistemas de lançamento plugáveis.
//!
//! Contrato: um lançador recebe a base (posição e velocidade de onde parte, num
//! referencial local leste-norte-cima com origem no local de lançamento) e o foguete
//! ([`Payload`]); devolve a liberação ([`Release`]): instante, posição, velocidade,
//! atitude, rotação, instante de ignição do motor e massa, além das trajetórias dos
//! corpos do lançador, das cargas no foguete e do balanço de energia. Cadeias de
//! lançadores passam a liberação de um como base do seguinte.
//!
//! Fase de lançamento em referencial local plano (sem curvatura nem Coriolis): vale para
//! as distâncias de lançadores (até dezenas de quilômetros). A conversão para o voo usa
//! o referencial inercial completo.

pub mod expr;
mod guided;
mod modes;
mod plugin;

pub use guided::{guided, Guide, Push};
pub use modes::{
    air_launch, balloon, ship, spin, submarine, AirLaunch, Balloon, Ship, Spin, Submarine,
};
pub use plugin::{builtin_plugins, ParamSpec, PluginCheck, PluginSpec};

use astra_math::Vec3;
use serde::{Deserialize, Serialize};

/// O foguete visto pelo lançador.
pub trait Payload {
    /// Massa no instante `t` de missão, com ignição em `ignition` (s; infinito = apagado).
    fn mass(&self, t: f64, ignition: f64) -> f64;
    /// Empuxo (N) no instante `t` com ignição em `ignition`, sob pressão `p_amb`.
    fn thrust(&self, t: f64, ignition: f64, p_amb: f64) -> f64;
    /// Área de arrasto axial Cd·A, m².
    fn drag_area(&self) -> f64;
    /// Comprimento, m.
    fn length(&self) -> f64;
    /// Diâmetro, m.
    fn diameter(&self) -> f64;
}

/// Ponto de partida de um lançador (local leste-norte-cima, m e m/s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Base {
    /// Instante, s.
    pub t: f64,
    /// Posição.
    pub pos: Vec3,
    /// Velocidade.
    pub vel: Vec3,
}

/// Estado do foguete ao ser liberado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Release {
    /// Instante, s.
    pub t: f64,
    /// Posição local (leste, norte, cima), m.
    pub pos: Vec3,
    /// Velocidade local, m/s.
    pub vel: Vec3,
    /// Eixo do foguete (unitário, local).
    pub axis: Vec3,
    /// Velocidade angular no referencial local, rad/s.
    pub omega: Vec3,
    /// Instante de ignição do motor (infinito se ainda não definido: então acende na
    /// liberação mais `ignition_delay`).
    pub ignition: f64,
    /// Atraso de ignição depois da liberação, s (usado quando `ignition` é infinito).
    pub ignition_delay: f64,
    /// Massa, kg.
    pub mass: f64,
}

impl Release {
    /// Instante efetivo de ignição.
    #[must_use]
    pub fn ignition_time(&self) -> f64 {
        if self.ignition.is_finite() {
            self.ignition
        } else {
            self.t + self.ignition_delay
        }
    }
    /// Base para o próximo lançador da cadeia.
    #[must_use]
    pub fn as_base(&self) -> Base {
        Base {
            t: self.t,
            pos: self.pos,
            vel: self.vel,
        }
    }
}

/// Balanço de energia da fase de lançamento, J.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Energy {
    /// Trabalho entregue pelo lançador (força, torque, empuxo, empuxo de flutuação).
    pub input: f64,
    /// Energia dissipada (atrito, arrasto).
    pub dissipated: f64,
    /// Variação de energia cinética do conjunto móvel.
    pub kinetic: f64,
    /// Variação de energia potencial.
    pub potential: f64,
    /// Fluxo de energia levado pela massa expelida (motor aceso no lançador).
    pub mass_flux: f64,
    /// Resíduo relativo: (entrada − dissipada + fluxo − Δcinética − Δpotencial)/escala.
    pub residual: f64,
    /// Energia cinética do foguete na liberação, relativa à base.
    pub rocket_kinetic: f64,
}

/// Ponto de trajetória de um corpo do lançador.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackPoint {
    /// Instante, s.
    pub t: f64,
    /// Posição local, m.
    pub pos: Vec3,
    /// Velocidade local, m/s.
    pub vel: Vec3,
    /// Eixo (para desenhar).
    pub axis: Vec3,
}

/// Resultado de um lançador.
#[derive(Clone, Debug, Default)]
pub struct LaunchOutcome {
    /// Liberação (None se o lançador falhou).
    pub release: Option<Release>,
    /// Trajetórias: (nome, pontos). A primeira é o foguete preso ao lançador.
    pub tracks: Vec<(String, Vec<TrackPoint>)>,
    /// Cargas no foguete: (t, aceleração axial em g, lateral em g).
    pub loads: Vec<(f64, f64, f64)>,
    /// Energia.
    pub energy: Energy,
    /// Acontecimentos: (t, código, mensagem).
    pub events: Vec<(f64, String, String)>,
    /// Avisos.
    pub warnings: Vec<String>,
}

/// Ambiente local da fase de lançamento.
#[derive(Clone, Debug)]
pub struct LocalEnv {
    /// Gravidade efetiva local, m/s².
    pub g: f64,
    /// Altitude do solo (acima do elipsoide), m.
    pub ground: f64,
    /// Atmosfera.
    pub atm: astra_env::AtmosphereModel,
    /// Vento.
    pub wind: astra_env::Wind,
}

impl LocalEnv {
    /// Massa específica do ar na altura local `z` (m acima do solo).
    #[must_use]
    pub fn rho(&self, z: f64) -> f64 {
        self.atm.at(self.ground + z).density
    }
    /// Pressão na altura local `z`.
    #[must_use]
    pub fn pressure(&self, z: f64) -> f64 {
        self.atm.at(self.ground + z).pressure
    }
    /// Vento (leste, norte, cima) na altura local `z`.
    #[must_use]
    pub fn wind_at(&self, z: f64) -> Vec3 {
        self.wind.mean(z)
    }
}

/// Direção unitária local a partir de elevação e azimute (rad).
#[must_use]
pub fn direction(elevation: f64, azimuth: f64) -> Vec3 {
    let (se, ce) = elevation.sin_cos();
    Vec3::new(ce * azimuth.sin(), ce * azimuth.cos(), se)
}

/// Lançador escolhido pelo usuário.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Launcher {
    /// Guia retilínea com empurrador (carrinho em rampa, canhão, railgun, mola...).
    Guided(Guide),
    /// Plataforma giratória.
    Spin(Spin),
    /// Lançamento aéreo.
    Air(AirLaunch),
    /// Balão estratosférico.
    Balloon(Balloon),
    /// Navio.
    Ship(Ship),
    /// Submarino (lançamento a frio).
    Submarine(Submarine),
    /// Lançador da comunidade (JSON declarativo).
    Plugin {
        /// Especificação.
        spec: Box<PluginSpec>,
        /// Valores dos parâmetros (sobrepõem os padrões).
        #[serde(default)]
        values: std::collections::BTreeMap<String, f64>,
    },
}

impl Launcher {
    /// Nome curto.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Guided(g) => g.label.clone(),
            Self::Spin(_) => "Plataforma giratória".into(),
            Self::Air(_) => "Lançamento aéreo".into(),
            Self::Balloon(_) => "Balão (rockoon)".into(),
            Self::Ship(_) => "Navio".into(),
            Self::Submarine(_) => "Submarino".into(),
            Self::Plugin { spec, .. } => spec.name.clone(),
        }
    }

    /// Executa o lançador a partir da base.
    #[must_use]
    pub fn run(&self, base: Base, rocket: &dyn Payload, env: &LocalEnv) -> LaunchOutcome {
        match self {
            Self::Guided(g) => guided(g, base, rocket, env),
            Self::Spin(s) => spin(s, base, rocket, env),
            Self::Air(a) => air_launch(a, base, rocket, env),
            Self::Balloon(b) => balloon(b, base, rocket, env),
            Self::Ship(s) => ship(s, base, rocket, env),
            Self::Submarine(s) => submarine(s, base, rocket, env),
            Self::Plugin { spec, values } => match spec.to_guide(values) {
                Ok(g) => guided(&g, base, rocket, env),
                Err(e) => LaunchOutcome {
                    warnings: vec![format!("Plugin {}: {e}", spec.name)],
                    ..LaunchOutcome::default()
                },
            },
        }
    }
}

/// Executa uma cadeia de lançadores: a liberação de cada um é a base do seguinte. A
/// massa e o motor do foguete passam adiante; trajetórias, cargas e eventos se somam.
#[must_use]
pub fn run_chain(chain: &[Launcher], rocket: &dyn Payload, env: &LocalEnv) -> LaunchOutcome {
    let mut base = Base::default();
    let mut total = LaunchOutcome::default();
    let mut ignition = f64::INFINITY;
    for (k, l) in chain.iter().enumerate() {
        let mut o = l.run(base, rocket, env);
        let Some(mut rel) = o.release else {
            total.warnings.append(&mut o.warnings);
            total.warnings.push(format!(
                "O lançador {} da cadeia ({}) não liberou o foguete.",
                k + 1,
                l.label()
            ));
            total.release = None;
            return total;
        };
        if ignition.is_finite() && !rel.ignition.is_finite() {
            rel.ignition = ignition;
        }
        ignition = rel.ignition;
        total.tracks.append(&mut o.tracks);
        total.loads.append(&mut o.loads);
        total.events.append(&mut o.events);
        total.warnings.append(&mut o.warnings);
        let e = &mut total.energy;
        e.input += o.energy.input;
        e.dissipated += o.energy.dissipated;
        e.kinetic += o.energy.kinetic;
        e.potential += o.energy.potential;
        e.mass_flux += o.energy.mass_flux;
        e.residual = e.residual.max(o.energy.residual);
        e.rocket_kinetic = o.energy.rocket_kinetic;
        base = rel.as_base();
        total.release = Some(rel);
    }
    total
}
