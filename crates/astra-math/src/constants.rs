//! Constantes físicas e geodésicas, cada uma com a fonte.

/// Aceleração padrão da gravidade, m/s² (valor exato por definição).
/// Fonte: CGPM 1901; BIPM, "The International System of Units (SI)", 9ª ed.
pub const G0: f64 = 9.806_65;

/// Constante universal dos gases, J/(mol·K) (exata desde a redefinição do SI de 2019).
/// Fonte: CODATA 2018, <https://physics.nist.gov/cuu/Constants/>.
pub const R_UNIVERSAL: f64 = 8.314_462_618_153_24;

/// Constante específica do ar seco, J/(kg·K).
/// Fonte: ICAO Doc 7488/3, "Manual of the ICAO Standard Atmosphere", Tabela A.
pub const R_AIR: f64 = 287.052_87;

/// Razão de calores específicos do ar seco (adimensional).
/// Fonte: ICAO Doc 7488/3; U.S. Standard Atmosphere 1976, Tabela 2.
pub const GAMMA_AIR: f64 = 1.4;

/// Parâmetros do elipsoide WGS 84.
/// Fonte: NGA.STND.0036_1.0.0_WGS84 (2014), Tabelas 3.1 e 3.5.
pub mod wgs84 {
    /// Semieixo maior (raio equatorial), m.
    pub const A: f64 = 6_378_137.0;
    /// Inverso do achatamento, 1/f (adimensional).
    pub const INV_F: f64 = 298.257_223_563;
    /// Achatamento f (adimensional).
    pub const F: f64 = 1.0 / INV_F;
    /// Semieixo menor (raio polar), m: b = a·(1 − f).
    pub const B: f64 = A * (1.0 - F);
    /// Primeira excentricidade ao quadrado: e² = f·(2 − f).
    pub const E2: f64 = F * (2.0 - F);
    /// Parâmetro gravitacional GM (inclui a atmosfera), m³/s².
    pub const GM: f64 = 3.986_004_418e14;
    /// Velocidade angular de rotação da Terra, rad/s.
    pub const OMEGA: f64 = 7.292_115e-5;
    /// Harmônico zonal de 2º grau J2 (adimensional), J2 = −√5 · C̄₂,₀.
    pub const J2: f64 = 1.082_629_821_313e-3;
}
