//! Documento de projeto. Todas as grandezas em SI (m, kg, s, N, Pa), ângulos em graus
//! onde o nome do campo diz `Deg`. Estações medidas da ponta do nariz para trás.

use serde::{Deserialize, Serialize};

/// Versão atual do formato.
pub const FORMAT_VERSION: u32 = 1;

/// Projeto completo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Design {
    /// Versão do formato.
    pub version: u32,
    /// Nome do projeto.
    pub name: String,
    /// Peças.
    pub parts: Vec<Part>,
    /// Juntas entre peças.
    #[serde(default)]
    pub joints: Vec<Joint>,
    /// Motor.
    pub motor: MotorSpec,
    /// Lançamento.
    pub launch: LaunchSpec,
    /// Rugosidade da superfície externa, m.
    #[serde(default = "default_roughness")]
    pub roughness: f64,
    /// Coeficiente de segurança exigido nas juntas (aviso abaixo dele).
    #[serde(default = "default_safety")]
    pub safety_factor: f64,
    /// Tabela de Cd externa (CFD, túnel, voo calibrado) para o foguete inteiro.
    #[serde(default)]
    pub drag_table: Option<DragTableSpec>,
    /// Materiais criados pelo usuário (somam-se ao banco embutido e o substituem quando o
    /// identificador coincide).
    #[serde(default)]
    pub materials: Vec<crate::material::Material>,
    /// Zonas de segurança e de pouso.
    #[serde(default)]
    pub zones: Vec<Zone>,
    /// Sistema de lançamento alternativo (vazio = trilho do lançamento).
    #[serde(default)]
    pub launcher: Vec<astra_launch::Launcher>,
    /// Eletrônica embarcada.
    #[serde(default)]
    pub electronics: crate::elec::Electronics,
    /// Cargas úteis e seus requisitos.
    #[serde(default)]
    pub payloads: Vec<crate::payload::Payload>,
    /// Regras de competição usadas na avaliação do voo.
    #[serde(default)]
    pub competition: Option<crate::rules::RuleSet>,
    /// Dados financeiros (lista de materiais, orçamento, receitas). O núcleo só os preserva;
    /// os cálculos ficam em `@astra/core-js` (`finance.ts`).
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub finance: serde_json::Value,
    /// Multiplicadores de dispersão e calibração: Cd, empuxo (impulso) e massa das peças.
    #[serde(default)]
    pub scales: Scales,
}

/// Multiplicadores aplicados ao modelo (1 = nominal). Usados pelo Monte Carlo e pela
/// calibração com telemetria.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Scales {
    /// Arrasto.
    pub cd: f64,
    /// Empuxo (com a mesma massa de propelente: impulso total).
    pub thrust: f64,
    /// Massa das peças.
    pub mass: f64,
}

impl Default for Scales {
    fn default() -> Self {
        Self {
            cd: 1.0,
            thrust: 1.0,
            mass: 1.0,
        }
    }
}

fn default_vent_diameter() -> f64 {
    0.003
}

/// Tabela de Cd por Mach.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragTableSpec {
    /// Pares [Mach, Cd] com motor apagado.
    pub power_off: Vec<[f64; 2]>,
    /// Pares [Mach, Cd] com motor aceso.
    pub power_on: Vec<[f64; 2]>,
}

fn default_roughness() -> f64 {
    2e-5
}
fn default_safety() -> f64 {
    1.5
}

/// Perfil de nariz, como em [`astra_aero::NoseShape`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoseShapeSpec {
    /// Cone.
    Conical,
    /// Ogiva tangente.
    TangentOgive,
    /// Elipse.
    Elliptical,
    /// Parábola.
    Parabolic,
    /// Von Kármán (Haack LD).
    VonKarman,
    /// Haack LV.
    LvHaack,
}

/// Seção da aleta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinProfileSpec {
    /// Bordos retos.
    Square,
    /// Bordos arredondados.
    Rounded,
    /// Perfil aerodinâmico.
    Airfoil,
}

/// Quando um paraquedas é ejetado.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum DeploySpec {
    /// No apogeu mais um atraso, s.
    Apogee {
        /// Atraso, s.
        #[serde(default)]
        delay: f64,
    },
    /// Descendo por uma altura acima do solo, m.
    Altitude {
        /// Altura acima do solo, m.
        agl: f64,
    },
    /// Instante fixo após a ignição, s (atraso de motor, por exemplo).
    Time {
        /// Instante, s.
        t: f64,
    },
    /// Quando o firmware dispara o e-match indicado (componente da eletrônica).
    Pyro {
        /// E-match.
        ematch: String,
    },
}

/// Peça física.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// Identificador único.
    pub id: String,
    /// Nome legível.
    pub name: String,
    /// Material (identificador no banco de materiais).
    pub material: String,
    /// Estação da extremidade dianteira, m. Para aletas, estação do bordo de ataque na raiz.
    pub position: f64,
    /// Massa medida, kg; substitui a calculada pelo material e pela geometria.
    #[serde(default)]
    pub mass_override: Option<f64>,
    /// Custo unitário, R$.
    #[serde(default)]
    pub cost: Option<f64>,
    /// Geometria e função.
    pub kind: PartKind,
}

/// Tipos de peça.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PartKind {
    /// Coifa (casca), com ombro que entra no tubo de baixo.
    #[serde(rename_all = "camelCase")]
    NoseCone {
        /// Perfil.
        shape: NoseShapeSpec,
        /// Comprimento exposto, m.
        length: f64,
        /// Diâmetro da base, m.
        diameter: f64,
        /// Espessura da parede, m.
        wall: f64,
        /// Comprimento do ombro, m.
        shoulder_length: f64,
    },
    /// Tubo externo.
    #[serde(rename_all = "camelCase")]
    BodyTube {
        /// Comprimento, m.
        length: f64,
        /// Diâmetro externo, m.
        outer_diameter: f64,
        /// Espessura da parede, m.
        wall: f64,
        /// Número de furos de respiro (pressão estática e alívio).
        #[serde(default)]
        vent_holes: u32,
        /// Diâmetro de cada furo de respiro, m.
        #[serde(default = "default_vent_diameter")]
        vent_diameter: f64,
    },
    /// Transição cônica externa (ombro ou boattail).
    #[serde(rename_all = "camelCase")]
    Transition {
        /// Comprimento, m.
        length: f64,
        /// Diâmetro dianteiro, m.
        d_fore: f64,
        /// Diâmetro traseiro, m.
        d_aft: f64,
        /// Espessura da parede, m.
        wall: f64,
    },
    /// Luva de acoplamento (interna, une dois tubos).
    #[serde(rename_all = "camelCase")]
    Coupler {
        /// Comprimento, m.
        length: f64,
        /// Diâmetro externo, m.
        outer_diameter: f64,
        /// Espessura da parede, m.
        wall: f64,
    },
    /// Tubo do motor (interno).
    #[serde(rename_all = "camelCase")]
    MotorMount {
        /// Comprimento, m.
        length: f64,
        /// Diâmetro externo, m.
        outer_diameter: f64,
        /// Espessura da parede, m.
        wall: f64,
    },
    /// Anteparo (disco cheio).
    #[serde(rename_all = "camelCase")]
    Bulkhead {
        /// Diâmetro, m.
        diameter: f64,
        /// Espessura, m.
        thickness: f64,
    },
    /// Anel de centragem.
    #[serde(rename_all = "camelCase")]
    CenteringRing {
        /// Diâmetro externo, m.
        outer_diameter: f64,
        /// Diâmetro interno, m.
        inner_diameter: f64,
        /// Espessura, m.
        thickness: f64,
    },
    /// Conjunto de aletas trapezoidais.
    #[serde(rename_all = "camelCase")]
    FinSet {
        /// Número de aletas.
        count: u32,
        /// Corda na raiz, m.
        root_chord: f64,
        /// Corda na ponta, m.
        tip_chord: f64,
        /// Recuo do bordo de ataque da ponta, m.
        sweep: f64,
        /// Envergadura, m.
        span: f64,
        /// Espessura, m.
        thickness: f64,
        /// Seção.
        profile: FinProfileSpec,
    },
    /// Paraquedas (dobrado dentro de uma baia).
    #[serde(rename_all = "camelCase")]
    Parachute {
        /// Diâmetro nominal do velame, m.
        diameter: f64,
        /// Coeficiente de arrasto sobre a área nominal (≈ 0,75 a 0,8 para velame
        /// circular plano; Knacke 1992).
        cd: f64,
        /// Comprimento do pacote dobrado, m.
        packed_length: f64,
        /// Diâmetro do pacote dobrado, m.
        packed_diameter: f64,
        /// Quando ejetar.
        deploy: DeploySpec,
        /// Junta que precisa se abrir para o paraquedas sair (baia).
        separation_joint: Option<String>,
        /// Carga de ejeção de pólvora negra, kg.
        #[serde(default)]
        ejection_charge: f64,
    },
    /// Cordão de choque.
    #[serde(rename_all = "camelCase")]
    ShockCord {
        /// Comprimento, m.
        length: f64,
        /// Massa por metro, kg/m.
        linear_density: f64,
        /// Carga de ruptura, N.
        strength: f64,
    },
    /// Componente com massa (aviônica, bateria, lastro, carga útil).
    #[serde(rename_all = "camelCase")]
    Mass {
        /// Massa, kg.
        mass: f64,
        /// Comprimento, m.
        length: f64,
        /// Diâmetro (ou largura) do volume ocupado, m.
        diameter: f64,
        /// Mede pressão estática (altímetro): exige furo de respiro no tubo.
        #[serde(default)]
        needs_static_port: bool,
    },
}

/// Junta entre duas peças.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Joint {
    /// Identificador único.
    pub id: String,
    /// Uma peça.
    pub a: String,
    /// A outra peça.
    pub b: String,
    /// Como estão unidas.
    pub kind: JointKind,
}

/// Tipos de junta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum JointKind {
    /// Colagem. A área colada é calculada pela geometria quando omitida.
    #[serde(rename_all = "camelCase")]
    Adhesive {
        /// Adesivo (material).
        adhesive: String,
        /// Área colada, m².
        #[serde(default)]
        area: Option<f64>,
        /// Raio do filete (aletas), m.
        #[serde(default)]
        fillet: Option<f64>,
        /// Aleta atravessa o tubo e é colada no tubo do motor.
        #[serde(default)]
        through_wall: bool,
    },
    /// Parafusos atravessando a parede.
    #[serde(rename_all = "camelCase")]
    Screws {
        /// Quantidade.
        count: u32,
        /// Diâmetro nominal, m.
        diameter: f64,
        /// Material do parafuso.
        material: String,
    },
    /// Pinos de cisalhamento (feitos para quebrar na ejeção).
    #[serde(rename_all = "camelCase")]
    ShearPins {
        /// Quantidade.
        count: u32,
        /// Diâmetro efetivo (raiz da rosca), m.
        diameter: f64,
        /// Material do pino.
        material: String,
    },
    /// Encaixe por atrito, com a força de extração medida.
    #[serde(rename_all = "camelCase")]
    FrictionFit {
        /// Força para separar, N.
        holding_force: f64,
    },
    /// Trava ou abraçadeira com força nominal.
    #[serde(rename_all = "camelCase")]
    Latch {
        /// Força nominal, N.
        holding_force: f64,
    },
    /// Cordão ou fita (só tração).
    #[serde(rename_all = "camelCase")]
    Cord {
        /// Carga de ruptura do ponto de fixação, N.
        strength: f64,
    },
}

/// Motor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum MotorSpec {
    /// Sem motor (planador, teste de queda).
    None,
    /// Arquivo RASP (.eng) embutido como texto.
    #[serde(rename_all = "camelCase")]
    Eng {
        /// Conteúdo do arquivo.
        text: String,
        /// Estação da saída da tubeira, m.
        aft_station: f64,
    },
    /// Foguete de água: a garrafa é a peça de corpo indicada.
    #[serde(rename_all = "camelCase")]
    Water {
        /// Volume da garrafa, m³.
        bottle_volume: f64,
        /// Volume de água, m³.
        water_volume: f64,
        /// Pressão manométrica, Pa.
        gauge_pressure: f64,
        /// Diâmetro do bocal, m.
        nozzle_diameter: f64,
        /// Pressão manométrica máxima segura da garrafa, Pa.
        max_safe_pressure: f64,
        /// Estação do bocal, m.
        aft_station: f64,
    },
}

/// Local e lançador.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSpec {
    /// Latitude, graus.
    pub lat_deg: f64,
    /// Longitude, graus.
    pub lon_deg: f64,
    /// Altitude do solo, m.
    pub altitude: f64,
    /// Comprimento do trilho, m.
    pub rail_length: f64,
    /// Elevação do trilho, graus.
    pub rail_elevation_deg: f64,
    /// Azimute do trilho, graus a partir do norte.
    pub rail_azimuth_deg: f64,
    /// Desvio de temperatura em relação à atmosfera padrão, K.
    #[serde(default)]
    pub delta_t: f64,
    /// Data e hora do lançamento em UTC ("AAAA-MM-DDThh:mm:ssZ"); define a rotação da
    /// Terra no instante zero e a posição do Sol na visualização.
    #[serde(default)]
    pub utc: Option<String>,
    /// Vento.
    #[serde(default)]
    pub wind: WindSpec,
}

/// Camada do perfil de vento.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindLayerSpec {
    /// Altura acima do solo, m.
    pub agl: f64,
    /// Velocidade, m/s.
    pub speed: f64,
    /// Direção de onde vem, graus a partir do norte.
    pub from_deg: f64,
}

/// Turbulência de Dryden.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurbulenceSpec {
    /// Desvio-padrão de cada componente, m/s.
    pub sigma: f64,
    /// Escala de comprimento, m.
    pub length: f64,
    /// Semente.
    pub seed: u64,
}

/// Vento do projeto.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindSpec {
    /// Perfil por altura (vazio = calmaria).
    #[serde(default)]
    pub layers: Vec<WindLayerSpec>,
    /// Rajadas.
    #[serde(default)]
    pub turbulence: Option<TurbulenceSpec>,
    /// De onde veio o perfil (manual, CSV, Open-Meteo com data), para o relatório.
    #[serde(default)]
    pub source: Option<String>,
}

impl WindSpec {
    /// Campo de vento do núcleo.
    #[must_use]
    pub fn to_wind(&self) -> astra_env::Wind {
        let mut layers: Vec<astra_env::WindLayer> = self
            .layers
            .iter()
            .map(|l| astra_env::WindLayer {
                agl: l.agl,
                speed: l.speed,
                from: l.from_deg.to_radians(),
            })
            .collect();
        layers.sort_by(|a, b| a.agl.total_cmp(&b.agl));
        astra_env::Wind {
            layers,
            turbulence: self
                .turbulence
                .map(|t| astra_env::Turbulence::new(t.sigma, t.length.max(1.0), t.seed)),
        }
    }
}

/// Zona desenhada no mapa de recuperação.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    /// Nome.
    pub name: String,
    /// Centro, latitude em graus.
    pub lat_deg: f64,
    /// Centro, longitude em graus.
    pub lon_deg: f64,
    /// Raio, m.
    pub radius: f64,
    /// Proibida (público, estrada) ou de pouso permitido.
    pub kind: ZoneKind,
}

/// Tipo de zona.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoneKind {
    /// Nada pode cair aqui.
    KeepOut,
    /// Área de pouso prevista.
    Landing,
}

impl Design {
    /// Lê um projeto em JSON.
    ///
    /// # Errors
    /// Mensagem do leitor JSON, ou de versão não suportada.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let d: Self = serde_json::from_str(text).map_err(|e| format!("projeto inválido: {e}"))?;
        if d.version > FORMAT_VERSION {
            return Err(format!(
                "projeto na versão {} do formato; este ASTRA lê até a {FORMAT_VERSION}",
                d.version
            ));
        }
        Ok(d)
    }

    /// Grava o projeto em JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Banco embutido mais os materiais do projeto.
    #[must_use]
    pub fn material_db(&self) -> crate::material::MaterialDb {
        let mut db = crate::material::MaterialDb::builtin();
        for m in &self.materials {
            db.upsert(m.clone());
        }
        db
    }

    /// Peça pelo identificador.
    #[must_use]
    pub fn part(&self, id: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }
}
