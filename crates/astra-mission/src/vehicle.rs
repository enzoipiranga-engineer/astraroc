use astra_propulsion::LiquidEngine;

/// Falha de motores durante a queima.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineOut {
    /// Instante após a ignição do estágio, s.
    pub after: f64,
    /// Motores perdidos.
    pub engines: u32,
}

/// Estágio (ou booster) de propelente líquido.
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    /// Nome.
    pub name: String,
    /// Massa seca (estrutura, tanques, motores), kg.
    pub dry_mass: f64,
    /// Propelente utilizável, kg.
    pub propellant: f64,
    /// Grupos de motores iguais: (motor, quantidade). A falha de motores tira do
    /// primeiro grupo.
    pub engines: Vec<(LiquidEngine, u32)>,
    /// Diâmetro, m.
    pub diameter: f64,
    /// Comprimento, m.
    pub length: f64,
    /// Pressão dinâmica máxima admitida (reduz a aceleração acima dela), Pa.
    pub max_q: Option<f64>,
    /// Aceleração máxima admitida (reduz o empuxo), m/s².
    pub max_accel: Option<f64>,
    /// Falha de motores.
    pub engine_out: Option<EngineOut>,
    /// Propelente reservado para recuperação (não usado na subida), kg.
    pub reserve: f64,
}

impl Stage {
    /// Massa total abastecida, kg.
    #[must_use]
    pub fn wet_mass(&self) -> f64 {
        self.dry_mass + self.propellant
    }

    /// Propelente utilizável na subida, kg.
    #[must_use]
    pub fn usable(&self) -> f64 {
        (self.propellant - self.reserve).max(0.0)
    }

    /// Empuxo no vácuo a 100% de todos os motores, N.
    #[must_use]
    pub fn thrust_vac(&self) -> f64 {
        self.engines
            .iter()
            .map(|(e, n)| e.thrust_vac * f64::from(*n))
            .sum()
    }

    /// Vazão a 100%, kg/s.
    #[must_use]
    pub fn mdot(&self) -> f64 {
        self.engines
            .iter()
            .map(|(e, n)| e.mdot() * f64::from(*n))
            .sum()
    }

    /// Isp médio no vácuo, s.
    #[must_use]
    pub fn isp_vac(&self) -> f64 {
        self.thrust_vac() / (self.mdot() * 9.806_65)
    }

    /// Menor aceleração admitida entre os motores (fração).
    #[must_use]
    pub fn throttle_min(&self) -> f64 {
        self.engines
            .iter()
            .map(|(e, _)| e.throttle_min)
            .fold(0.0, f64::max)
    }
}

/// Boosters laterais idênticos que acendem com o primeiro estágio e se separam ao fim
/// da queima.
#[derive(Clone, Debug, PartialEq)]
pub struct Booster {
    /// Cada booster.
    pub stage: Stage,
    /// Quantidade.
    pub count: u32,
}

/// Carenagem de carga útil.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fairing {
    /// Massa total, kg.
    pub mass: f64,
    /// Altitude de alijamento, m.
    pub jettison_altitude: f64,
}

/// Lançador.
#[derive(Clone, Debug, PartialEq)]
pub struct LaunchVehicle {
    /// Nome.
    pub name: String,
    /// Estágios em série, do primeiro ao último.
    pub stages: Vec<Stage>,
    /// Boosters.
    pub boosters: Option<Booster>,
    /// Carenagem.
    pub fairing: Option<Fairing>,
    /// Carga útil, kg.
    pub payload: f64,
    /// Tabela Cd por Mach do conjunto (referência: maior diâmetro).
    pub cd_table: Vec<(f64, f64)>,
}

impl LaunchVehicle {
    /// Massa na decolagem, kg.
    #[must_use]
    pub fn liftoff_mass(&self) -> f64 {
        self.stages.iter().map(Stage::wet_mass).sum::<f64>()
            + self
                .boosters
                .as_ref()
                .map_or(0.0, |b| f64::from(b.count) * b.stage.wet_mass())
            + self.fairing.map_or(0.0, |f| f.mass)
            + self.payload
    }

    /// Δv ideal de Tsiolkovsky somado dos estágios no vácuo (sem boosters, sem perdas),
    /// m/s, descontando carenagem só no primeiro estágio (alijada depois).
    #[must_use]
    pub fn ideal_dv(&self) -> f64 {
        let mut above: f64 = self.payload;
        let mut dv = 0.0;
        for (i, s) in self.stages.iter().enumerate().rev() {
            let fairing = if i == 0 {
                self.fairing.map_or(0.0, |f| f.mass)
            } else {
                0.0
            };
            let m0 = above + s.wet_mass() + fairing;
            let mf = m0 - s.usable();
            dv += s.isp_vac() * 9.806_65 * (m0 / mf).ln();
            above += s.wet_mass();
        }
        dv
    }
}

/// Manobra inicial do gravity turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PitchKick {
    /// Início (fim da subida vertical), s.
    pub start: f64,
    /// Duração, s.
    pub duration: f64,
    /// Ângulo a partir da vertical, graus.
    pub angle_deg: f64,
}

/// Missão de lançamento até uma órbita circular.
#[derive(Clone, Debug, PartialEq)]
pub struct Mission {
    /// Veículo.
    pub vehicle: LaunchVehicle,
    /// Local: latitude e longitude (graus) e altitude (m).
    pub site: (f64, f64, f64),
    /// Azimute de lançamento, graus a partir do norte.
    pub azimuth_deg: f64,
    /// Altitude da órbita circular alvo, m.
    pub target_altitude: f64,
    /// Manobra inicial.
    pub kick: PitchKick,
    /// Escolher sozinho o ângulo da manobra inicial (o que deixa mais propelente no corte).
    pub auto_kick: bool,
    /// Espera entre o fim de um estágio e a ignição do seguinte, s (0 = hot staging).
    pub coast: f64,
    /// Data e hora UTC.
    pub utc: Option<String>,
    /// Vento.
    pub wind: astra_env::Wind,
}
