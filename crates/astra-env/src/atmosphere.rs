use astra_math::constants::{G0, GAMMA_AIR};

/// Estado termodinâmico do ar em um ponto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmoState {
    /// Temperatura, K.
    pub temperature: f64,
    /// Pressão estática, Pa.
    pub pressure: f64,
    /// Massa específica, kg/m³.
    pub density: f64,
    /// Velocidade do som, m/s (0 no vácuo).
    pub sound_speed: f64,
    /// Viscosidade dinâmica, Pa·s (0 no vácuo).
    pub viscosity: f64,
}

/// Modelo de atmosfera selecionável.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AtmosphereModel {
    /// Sem atmosfera.
    Vacuum,
    /// U.S. Standard Atmosphere 1976 com desvio de temperatura (ISA + ΔT), K.
    Ussa1976 {
        /// Desvio de temperatura em relação ao padrão, K (aplicado abaixo de 86 km).
        delta_t: f64,
    },
}

/// Raio efetivo da Terra usado pela USSA 1976 para o geopotencial, m.
const R0: f64 = 6_356_766.0;
/// Constante do ar da USSA 1976: R*/M0 = 8314,32 / 28,9644 J/(kg·K).
/// (A USSA usa R* = 8314,32, ligeiramente diferente do valor CODATA atual.)
const R_USSA: f64 = 8_314.32 / 28.964_4;
/// Camadas até 84,852 km geopotenciais: (altitude geopotencial da base em m,
/// gradiente térmico em K/m). Fonte: USSA 1976 (NOAA-S/T 76-1562), Tabela 4.
const LAYERS: [(f64, f64); 7] = [
    (0.0, -6.5e-3),
    (11_000.0, 0.0),
    (20_000.0, 1.0e-3),
    (32_000.0, 2.8e-3),
    (47_000.0, 0.0),
    (51_000.0, -2.8e-3),
    (71_000.0, -2.0e-3),
];
const H_TOP: f64 = 84_852.0;
const T_SL: f64 = 288.15;
const P_SL: f64 = 101_325.0;
/// Temperatura a 86 km geométricos, K (USSA 1976, Tabela I).
const T_86: f64 = 186.867_3;

/// Faixas exponenciais acima de 86 km: (altitude geométrica da base em km,
/// massa específica na base em kg/m³, altura de escala em km).
/// Fonte: Vallado, "Fundamentals of Astrodynamics and Applications", Tabela 8-4
/// (ajuste à USSA 1976 abaixo de 150 km e ao CIRA-72 acima).
const UPPER: [(f64, f64, f64); 20] = [
    (80.0, 1.905e-5, 5.799),
    (90.0, 3.396e-6, 5.382),
    (100.0, 5.297e-7, 5.877),
    (110.0, 9.661e-8, 7.263),
    (120.0, 2.438e-8, 9.473),
    (130.0, 8.484e-9, 12.636),
    (140.0, 3.845e-9, 16.149),
    (150.0, 2.070e-9, 22.523),
    (180.0, 5.464e-10, 29.740),
    (200.0, 2.789e-10, 37.105),
    (250.0, 7.248e-11, 45.546),
    (300.0, 2.418e-11, 53.628),
    (350.0, 9.518e-12, 53.298),
    (400.0, 3.725e-12, 58.515),
    (450.0, 1.585e-12, 60.828),
    (500.0, 6.967e-13, 63.822),
    (600.0, 1.454e-13, 71.835),
    (700.0, 3.614e-14, 88.667),
    (800.0, 1.170e-14, 124.64),
    (900.0, 5.245e-15, 181.05),
];

/// Viscosidade dinâmica do ar pela lei de Sutherland:
/// μ = β·T^1,5 / (T + S), β = 1,458e-6 kg/(m·s·K^0,5), S = 110,4 K.
/// Fonte: USSA 1976, eq. 51.
fn sutherland(t: f64) -> f64 {
    1.458e-6 * t.powf(1.5) / (t + 110.4)
}

fn state(t: f64, p: f64, rho: f64) -> AtmoState {
    AtmoState {
        temperature: t,
        pressure: p,
        density: rho,
        // a = √(γ R T), USSA 1976 eq. 50.
        sound_speed: (GAMMA_AIR * R_USSA * t).sqrt(),
        viscosity: sutherland(t),
    }
}

/// USSA 1976 abaixo de 86 km. Temperatura linear por camada em altitude geopotencial
/// H = r0·z/(r0 + z) (eq. 18); pressão pela hidrostática integrada:
/// P = Pb·(Tb/T)^(g0/(R·L)) se L ≠ 0, ou P = Pb·exp(−g0·(H − Hb)/(R·Tb)) se L = 0
/// (eqs. 33a e 33b).
fn lower(z: f64, delta_t: f64) -> AtmoState {
    let h = (R0 * z / (R0 + z)).min(H_TOP);
    let (mut tb, mut pb) = (T_SL, P_SL);
    for (i, &(hb, lapse)) in LAYERS.iter().enumerate() {
        let top = LAYERS.get(i + 1).map_or(H_TOP, |l| l.0);
        let dh = h.min(top) - hb;
        let t = tb + lapse * dh;
        let p = if lapse == 0.0 {
            pb * (-G0 * dh / (R_USSA * tb)).exp()
        } else {
            pb * (tb / t).powf(G0 / (R_USSA * lapse))
        };
        if h <= top {
            let t_real = t + delta_t;
            return state(t_real, p, p / (R_USSA * t_real));
        }
        (tb, pb) = (t, p);
    }
    unreachable!("h é limitado a H_TOP, coberto pela última camada")
}

impl AtmosphereModel {
    /// Estado do ar na altitude geométrica `z` (m acima do elipsoide).
    #[must_use]
    pub fn at(&self, z: f64) -> AtmoState {
        match *self {
            Self::Vacuum => AtmoState {
                temperature: 0.0,
                pressure: 0.0,
                density: 0.0,
                sound_speed: 0.0,
                viscosity: 0.0,
            },
            Self::Ussa1976 { delta_t } => {
                if z <= 86_000.0 {
                    return lower(z, delta_t);
                }
                let km = z / 1000.0;
                let &(h0, rho0, scale) =
                    UPPER.iter().rev().find(|b| km >= b.0).unwrap_or(&UPPER[0]);
                let rho = rho0 * (-(km - h0) / scale).exp();
                state(T_86, rho * R_USSA * T_86, rho)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometric(h_geopot: f64) -> f64 {
        R0 * h_geopot / (R0 - h_geopot)
    }

    /// Valores nas bases das camadas: USSA 1976, Tabela 4 (T e P), e ρ ao nível do mar.
    #[test]
    fn matches_ussa1976_layer_table() {
        let atm = AtmosphereModel::Ussa1976 { delta_t: 0.0 };
        let table = [
            (0.0, 288.15, 101_325.0),
            (11_000.0, 216.65, 22_632.06),
            (20_000.0, 216.65, 5_474.889),
            (32_000.0, 228.65, 868.018_7),
            (47_000.0, 270.65, 110.906_3),
            (51_000.0, 270.65, 66.938_87),
            (71_000.0, 214.65, 3.956_420),
            (84_852.0, 186.946, 0.373_383_6),
        ];
        for (h, t, p) in table {
            let s = atm.at(geometric(h));
            assert!(
                (s.temperature - t).abs() < 1e-3,
                "T em {h}: {}",
                s.temperature
            );
            assert!(
                (s.pressure / p - 1.0).abs() < 2e-5,
                "P em {h}: {}",
                s.pressure
            );
        }
        let sl = atm.at(0.0);
        assert!((sl.density - 1.225).abs() < 1e-4);
        assert!((sl.sound_speed - 340.294).abs() < 1e-2);
        assert!((sl.viscosity - 1.7894e-5).abs() < 1e-9);
    }

    #[test]
    fn hot_day_lowers_density_but_not_pressure() {
        let std = AtmosphereModel::Ussa1976 { delta_t: 0.0 }.at(1000.0);
        let hot = AtmosphereModel::Ussa1976 { delta_t: 20.0 }.at(1000.0);
        assert_eq!(std.pressure, hot.pressure);
        assert!((hot.density / std.density - std.temperature / hot.temperature).abs() < 1e-12);
    }

    #[test]
    fn upper_atmosphere_is_monotonic_and_joins_within_five_percent() {
        let atm = AtmosphereModel::Ussa1976 { delta_t: 0.0 };
        let below = atm.at(86_000.0).density;
        let above = atm.at(86_000.1).density;
        assert!(
            (above / below - 1.0).abs() < 0.05,
            "salto em 86 km: {below} → {above}"
        );
        let mut prev = above;
        for km in (90..=1200).step_by(10) {
            let rho = atm.at(f64::from(km) * 1000.0).density;
            assert!(rho < prev && rho > 0.0, "{km} km");
            prev = rho;
        }
        // ISS (~400 km): ordem de 1e-12 kg/m³.
        let iss = atm.at(400_000.0).density;
        assert!(iss > 1e-12 && iss < 1e-11);
    }

    #[test]
    fn vacuum_is_empty() {
        assert_eq!(AtmosphereModel::Vacuum.at(0.0).density, 0.0);
    }
}
