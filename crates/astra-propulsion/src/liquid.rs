//! Motores de propelente líquido e desempenho ideal de câmara e tubeira.

use crate::nozzle::{exit_pressure_ratio, thrust_coefficient};

/// Aceleração padrão da gravidade, m/s².
const G0: f64 = 9.806_65;
/// Pressão ao nível do mar padrão, Pa.
const P_SL: f64 = 101_325.0;

/// Motor de foguete líquido descrito pelo desempenho (não pela geometria interna).
#[derive(Clone, Debug, PartialEq)]
pub struct LiquidEngine {
    /// Nome.
    pub name: String,
    /// Propelentes.
    pub propellants: String,
    /// Empuxo no vácuo a 100%, N.
    pub thrust_vac: f64,
    /// Impulso específico no vácuo, s.
    pub isp_vac: f64,
    /// Área de saída da tubeira, m². Define a perda de empuxo com a pressão ambiente.
    pub exit_area: f64,
    /// Faixa de aceleração (fração do empuxo nominal).
    pub throttle_min: f64,
    /// Máximo de aceleração (fração).
    pub throttle_max: f64,
    /// Ângulo máximo de gimbal, rad.
    pub gimbal: f64,
    /// Massa seca, kg.
    pub mass: f64,
    /// Fonte dos números.
    pub source: String,
    /// "catalogo" ou "estimado".
    pub reliability: String,
}

impl LiquidEngine {
    /// Monta o motor a partir dos dados usuais de catálogo: empuxo e Isp no vácuo e ao
    /// nível do mar. A área de saída sai de F_vác − F_nm = p_nm·A_e (ṁ igual nos dois).
    /// Fonte: Sutton & Biblarz, 9ª ed., cap. 2 (equação do empuxo).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_catalog(
        name: &str,
        propellants: &str,
        thrust_vac: f64,
        isp_vac: f64,
        thrust_sl: Option<f64>,
        throttle_min: f64,
        gimbal_deg: f64,
        mass: f64,
        source: &str,
        reliability: &str,
    ) -> Self {
        let exit_area = thrust_sl.map_or(0.0, |f| ((thrust_vac - f) / P_SL).max(0.0));
        Self {
            name: name.into(),
            propellants: propellants.into(),
            thrust_vac,
            isp_vac,
            exit_area,
            throttle_min,
            throttle_max: 1.0,
            gimbal: gimbal_deg.to_radians(),
            mass,
            source: source.into(),
            reliability: reliability.into(),
        }
    }

    /// Vazão mássica a 100%, kg/s: ṁ = F_vác/(Isp_vác·g0).
    #[must_use]
    pub fn mdot(&self) -> f64 {
        self.thrust_vac / (self.isp_vac * G0)
    }

    /// Empuxo com aceleração `throttle` sob pressão ambiente `p_amb`:
    /// F = throttle·F_vác − p_amb·A_e (a pressão de saída cai com a vazão; a perda de
    /// pressão ambiente não). Nunca negativo (motor sobre-expandido ao nível do mar é
    /// limitado aqui; a separação do escoamento não é modelada).
    #[must_use]
    pub fn thrust(&self, throttle: f64, p_amb: f64) -> f64 {
        (throttle * self.thrust_vac - p_amb * self.exit_area).max(0.0)
    }

    /// Isp na pressão `p_amb` a 100%, s.
    #[must_use]
    pub fn isp(&self, p_amb: f64) -> f64 {
        self.thrust(1.0, p_amb) / (self.mdot() * G0)
    }
}

/// Biblioteca de motores reais com dados públicos aproximados.
///
/// Todos os valores foram transcritos de memória de fontes públicas (páginas dos
/// fabricantes, NASA e compilações de referência como a Encyclopedia Astronautica e a
/// Wikipédia) e **não foram conferidos nesta sessão**: confiabilidade "estimado".
/// Massas secas e faixas de aceleração são as menos certas.
#[must_use]
pub fn engine_library() -> Vec<LiquidEngine> {
    let e = |n: &str,
             p: &str,
             fv_kn: f64,
             isp_v: f64,
             fs_kn: Option<f64>,
             thr: f64,
             gim: f64,
             m: f64,
             src: &str| {
        LiquidEngine::from_catalog(
            n,
            p,
            fv_kn * 1e3,
            isp_v,
            fs_kn.map(|f| f * 1e3),
            thr,
            gim,
            m,
            src,
            "estimado",
        )
    };
    vec![
        e(
            "Merlin 1D",
            "LOX/RP-1",
            914.0,
            311.0,
            Some(845.0),
            0.57,
            5.0,
            470.0,
            "SpaceX, Falcon 9 User's Guide e página do Falcon 9",
        ),
        e(
            "Merlin 1D Vacuum",
            "LOX/RP-1",
            981.0,
            348.0,
            None,
            0.39,
            5.0,
            490.0,
            "SpaceX, página do Falcon 9",
        ),
        e(
            "Raptor 2",
            "LOX/CH4",
            2_450.0,
            347.0,
            Some(2_256.0),
            0.40,
            15.0,
            1_630.0,
            "SpaceX, apresentações públicas (2022)",
        ),
        e(
            "Raptor Vacuum",
            "LOX/CH4",
            2_530.0,
            380.0,
            None,
            0.40,
            0.0,
            2_100.0,
            "SpaceX, apresentações públicas",
        ),
        e(
            "RS-25",
            "LOX/LH2",
            2_279.0,
            452.3,
            Some(1_860.0),
            0.67,
            10.5,
            3_527.0,
            "NASA/Aerojet Rocketdyne, RS-25 fact sheet",
        ),
        e(
            "F-1",
            "LOX/RP-1",
            7_770.0,
            304.0,
            Some(6_770.0),
            1.0,
            6.0,
            8_400.0,
            "NASA, Saturn V Flight Manual (SA-503)",
        ),
        e(
            "J-2",
            "LOX/LH2",
            1_033.0,
            421.0,
            None,
            1.0,
            7.0,
            1_788.0,
            "NASA, Saturn V Flight Manual (SA-503)",
        ),
        e(
            "RL10B-2",
            "LOX/LH2",
            110.1,
            465.5,
            None,
            1.0,
            4.0,
            277.0,
            "Aerojet Rocketdyne, RL10 data sheet",
        ),
        e(
            "BE-4",
            "LOX/CH4",
            2_600.0,
            340.0,
            Some(2_400.0),
            0.65,
            5.0,
            2_000.0,
            "Blue Origin, dados públicos parciais",
        ),
        e(
            "RD-180",
            "LOX/RP-1",
            4_152.0,
            338.0,
            Some(3_830.0),
            0.47,
            8.0,
            5_480.0,
            "NPO Energomash, ficha do RD-180",
        ),
        e(
            "Vulcain 2",
            "LOX/LH2",
            1_359.0,
            431.0,
            Some(960.0),
            1.0,
            6.0,
            2_100.0,
            "ArianeGroup/ESA",
        ),
        e(
            "Rutherford",
            "LOX/RP-1",
            25.8,
            311.0,
            Some(24.9),
            1.0,
            5.0,
            35.0,
            "Rocket Lab, página do Electron",
        ),
    ]
}

/// Desempenho ideal de um motor a partir das propriedades dos gases na câmara.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IdealPerformance {
    /// Velocidade característica, m/s.
    pub c_star: f64,
    /// Coeficiente de empuxo.
    pub cf: f64,
    /// Impulso específico, s.
    pub isp: f64,
    /// Pressão de saída, Pa.
    pub p_exit: f64,
}

/// Escoamento ideal congelado:
/// c* = √(γ·R·Tc) / (γ·√((2/(γ+1))^((γ+1)/(γ−1)))), R = 8314,46/M,
/// Isp = c*·C_F/g0, com C_F pela expansão isentrópica até a razão de áreas ε.
/// Fonte: Sutton & Biblarz, 9ª ed., eqs. de c* e C_F do cap. 3.
/// Não substitui o equilíbrio químico (NASA CEA): Tc, M e γ devem vir de uma tabela ou do
/// CEA para a razão de mistura usada.
#[must_use]
pub fn ideal_performance(
    tc: f64,
    molar_mass: f64,
    gamma: f64,
    p_chamber: f64,
    area_ratio: f64,
    p_amb: f64,
) -> IdealPerformance {
    let g = gamma;
    let r = 8_314.462_618 / molar_mass;
    let c_star = (g * r * tc).sqrt() / (g * ((2.0 / (g + 1.0)).powf((g + 1.0) / (g - 1.0))).sqrt());
    let cf = thrust_coefficient(g, area_ratio, p_chamber, p_amb);
    IdealPerformance {
        c_star,
        cf,
        isp: c_star * cf / G0,
        p_exit: exit_pressure_ratio(area_ratio, g) * p_chamber,
    }
}

/// Propriedades típicas dos gases de câmara para pares de propelentes na razão de mistura
/// próxima do ótimo, a ~7 MPa: (nome, O/F, Tc em K, M em kg/kmol, γ).
/// Valores aproximados de tabelas de Sutton & Biblarz (tab. 5-5) e de saídas do NASA CEA
/// publicadas; confiabilidade "estimado".
pub const PROPELLANT_TABLE: [(&str, f64, f64, f64, f64); 4] = [
    ("LOX/RP-1", 2.56, 3_670.0, 23.3, 1.24),
    ("LOX/LH2", 4.83, 3_000.0, 10.0, 1.26),
    ("LOX/CH4", 3.20, 3_530.0, 20.5, 1.20),
    ("N2O4/UDMH", 2.6, 3_400.0, 23.0, 1.25),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_engine_is_consistent() {
        let m = engine_library()
            .into_iter()
            .find(|e| e.name == "Merlin 1D")
            .unwrap();
        assert!(
            (m.thrust(1.0, P_SL) - 845e3).abs() < 1e-6,
            "empuxo ao nível do mar volta ao catálogo"
        );
        assert!((m.isp(0.0) - 311.0).abs() < 1e-9);
        assert!((m.isp(P_SL) - 311.0 * 845.0 / 914.0).abs() < 1e-9);
        assert!((m.mdot() - 914e3 / (311.0 * G0)).abs() < 1e-9);
        assert!(m.thrust(0.5, 0.0) < m.thrust(1.0, 0.0));
        for e in engine_library() {
            assert!(
                e.isp(0.0) >= e.isp(P_SL) && e.mdot() > 0.0 && !e.source.is_empty(),
                "{}",
                e.name
            );
        }
    }

    /// c* à mão para γ = 1,2, Tc = 3000 K, M = 20: R = 415,72; (2/2,2)^11 = 0,35049;
    /// c* = √(1,2·415,72·3000)/(1,2·√0,35049) = 1223,0/0,71043 = 1721,5 m/s.
    #[test]
    fn ideal_c_star_by_hand() {
        let p = ideal_performance(3000.0, 20.0, 1.2, 7e6, 40.0, 0.0);
        assert!((p.c_star - 1721.5).abs() < 0.5, "c* = {}", p.c_star);
        assert!(
            p.isp > 300.0 && p.isp < 380.0,
            "Isp no vácuo plausível: {}",
            p.isp
        );
        let sl = ideal_performance(3000.0, 20.0, 1.2, 7e6, 40.0, P_SL);
        assert!(sl.isp < p.isp);
    }
}
