//! Escoamento isentrópico em tubeira convergente-divergente.

/// Razão de pressão p_e/p_c na saída para a razão de áreas `area_ratio` = A_e/A_t ≥ 1
/// (ramo supersônico). Resolve por bisseção em Mach a relação
/// A/A* = (1/M)·[(2/(γ+1))·(1 + (γ−1)/2·M²)]^((γ+1)/(2(γ−1))) e aplica
/// p/p0 = (1 + (γ−1)/2·M²)^(−γ/(γ−1)).
/// Fonte: Sutton & Biblarz, 9ª ed., cap. 3; Anderson, "Modern Compressible
/// Flow", 3ª ed., cap. 5.
#[must_use]
pub fn exit_pressure_ratio(area_ratio: f64, gamma: f64) -> f64 {
    let g = gamma;
    let area = |m: f64| {
        (1.0 / m)
            * ((2.0 / (g + 1.0)) * (1.0 + 0.5 * (g - 1.0) * m * m))
                .powf((g + 1.0) / (2.0 * (g - 1.0)))
    };
    let (mut lo, mut hi) = (1.0, 50.0);
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if area(mid) < area_ratio {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let m = 0.5 * (lo + hi);
    (1.0 + 0.5 * (g - 1.0) * m * m).powf(-g / (g - 1.0))
}

/// Coeficiente de empuxo ideal:
/// C_F = √{ [2γ²/(γ−1)]·[2/(γ+1)]^((γ+1)/(γ−1))·[1 − (p_e/p_c)^((γ−1)/γ)] }
///       + (p_e − p_a)/p_c · A_e/A_t.
/// Não modela separação do escoamento (tubeira muito sobre-expandida).
/// Fonte: Sutton & Biblarz, 9ª ed., cap. 3 (coeficiente de empuxo).
#[must_use]
pub fn thrust_coefficient(gamma: f64, area_ratio: f64, p_chamber: f64, p_ambient: f64) -> f64 {
    let g = gamma;
    let pr = exit_pressure_ratio(area_ratio, g);
    let momentum = (2.0 * g * g / (g - 1.0)
        * (2.0 / (g + 1.0)).powf((g + 1.0) / (g - 1.0))
        * (1.0 - pr.powf((g - 1.0) / g)))
    .sqrt();
    momentum + (pr - p_ambient / p_chamber) * area_ratio
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tabelas isentrópicas (γ = 1,4): A/A* = 4,2346 em M = 3, onde p/p0 = 0,02722.
    /// Fonte: Anderson, "Modern Compressible Flow", Tabela A.1.
    #[test]
    fn matches_isentropic_table() {
        assert!((exit_pressure_ratio(4.2346, 1.4) - 0.027_22).abs() < 2e-5);
    }

    /// Tubeira adaptada (p_e = p_a) com γ = 1,2 e p_c/p_a = 68, à mão:
    /// 2γ²/(γ−1) = 14,4; (2/2,2)^11 = 0,35049; 1 − 68^(−1/6) = 0,50503;
    /// C_F = √(14,4 · 0,35049 · 0,50503) = 1,5965.
    #[test]
    fn optimum_thrust_coefficient() {
        let g: f64 = 1.2;
        // Razão de áreas que dá p_e/p_c = 1/68.
        let (mut lo, mut hi) = (1.0, 50.0);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if exit_pressure_ratio(mid, g) > 1.0 / 68.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let cf = thrust_coefficient(g, lo, 68.0, 1.0);
        assert!((cf - 1.5965).abs() < 2e-4, "C_F = {cf}");
        // No vácuo o mesmo bocal rende mais.
        assert!(thrust_coefficient(g, lo, 68.0, 0.0) > cf);
    }
}
