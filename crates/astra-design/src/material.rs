use serde::{Deserialize, Serialize};

/// Confiabilidade de um dado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reliability {
    /// Medido pelo usuário ou em ensaio documentado.
    Medido,
    /// Ficha técnica ou manual publicado.
    Catalogo,
    /// Ordem de grandeza.
    Estimado,
}

/// Material, com as unidades do arquivo (fáceis de editar à mão). Use os métodos para SI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// Identificador.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Categoria (metal, compósito, polímero...).
    pub category: String,
    /// Massa específica, kg/m³.
    #[serde(rename = "densityKgM3")]
    pub density_kg_m3: f64,
    /// Módulo de elasticidade, GPa.
    #[serde(rename = "eGPa")]
    pub e_g_pa: f64,
    /// Módulo de cisalhamento, GPa.
    #[serde(rename = "gGPa")]
    pub g_g_pa: f64,
    /// Tensão de escoamento, MPa.
    #[serde(rename = "yieldMPa")]
    pub yield_m_pa: f64,
    /// Tensão de ruptura, MPa.
    #[serde(rename = "ultimateMPa")]
    pub ultimate_m_pa: f64,
    /// Resistência ao cisalhamento, MPa.
    #[serde(rename = "shearMPa")]
    pub shear_m_pa: f64,
    /// Temperatura máxima de uso, °C.
    #[serde(rename = "maxTempC")]
    pub max_temp_c: f64,
    /// Condutividade térmica, W/(m·K).
    #[serde(rename = "kWmK")]
    pub k_w_m_k: f64,
    /// Calor específico, J/(kg·K).
    #[serde(rename = "cpJkgK")]
    pub cp_j_kg_k: f64,
    /// Dilatação térmica, µm/(m·K).
    #[serde(rename = "cteUmK")]
    pub cte_um_k: f64,
    /// Gramatura (tecidos), g/m².
    #[serde(default)]
    #[serde(rename = "arealGm2")]
    pub areal_gm2: Option<f64>,
    /// Confiabilidade.
    pub reliability: Reliability,
    /// Fonte.
    pub source: String,
    /// Observações.
    #[serde(default)]
    pub notes: Option<String>,
}

impl Material {
    /// Módulo de elasticidade, Pa.
    #[must_use]
    pub fn e(&self) -> f64 {
        self.e_g_pa * 1e9
    }
    /// Módulo de cisalhamento, Pa.
    #[must_use]
    pub fn g(&self) -> f64 {
        self.g_g_pa * 1e9
    }
    /// Escoamento, Pa.
    #[must_use]
    pub fn yield_strength(&self) -> f64 {
        self.yield_m_pa * 1e6
    }
    /// Ruptura, Pa.
    #[must_use]
    pub fn ultimate(&self) -> f64 {
        self.ultimate_m_pa * 1e6
    }
    /// Cisalhamento, Pa.
    #[must_use]
    pub fn shear(&self) -> f64 {
        self.shear_m_pa * 1e6
    }
}

#[derive(Deserialize)]
struct DbFile {
    materials: Vec<Material>,
}

/// Banco de materiais.
#[derive(Clone, Debug, Default)]
pub struct MaterialDb {
    /// Materiais na ordem do arquivo.
    pub materials: Vec<Material>,
}

impl MaterialDb {
    /// Banco embutido (`data/materials.json`).
    ///
    /// # Panics
    /// Nunca: o arquivo embutido é válido (coberto por teste).
    #[must_use]
    pub fn builtin() -> Self {
        Self::from_json(include_str!("../../../data/materials.json"))
            .expect("banco de materiais embutido")
    }

    /// Lê um banco no formato de `data/materials.json`.
    ///
    /// # Errors
    /// Mensagem do leitor JSON ou de identificador repetido.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let f: DbFile =
            serde_json::from_str(text).map_err(|e| format!("banco de materiais inválido: {e}"))?;
        let mut ids: Vec<&str> = f.materials.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        if let Some(w) = ids.windows(2).find(|w| w[0] == w[1]) {
            return Err(format!("material repetido: {}", w[0]));
        }
        Ok(Self {
            materials: f.materials,
        })
    }

    /// Material pelo identificador.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Material> {
        self.materials.iter().find(|m| m.id == id)
    }

    /// Acrescenta ou substitui um material (material novo do usuário).
    pub fn upsert(&mut self, m: Material) {
        match self.materials.iter_mut().find(|x| x.id == m.id) {
            Some(x) => *x = m,
            None => self.materials.push(m),
        }
    }
}
