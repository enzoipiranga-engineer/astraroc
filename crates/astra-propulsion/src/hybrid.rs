//! Motor híbrido: oxidante líquido ou gasoso sobre grão sólido de combustível com um
//! canal central.

use crate::motor::{Motor, MotorPoint, P_SL};
use crate::nozzle::thrust_coefficient;
use core::f64::consts::PI;

/// Motor híbrido de canal cilíndrico único.
#[derive(Clone, Debug, PartialEq)]
pub struct HybridMotor {
    /// Vazão de oxidante, kg/s (constante: alimentação por tanque regulado).
    pub ox_flow: f64,
    /// Massa de oxidante, kg.
    pub ox_mass: f64,
    /// Massa específica do combustível, kg/m³.
    pub fuel_density: f64,
    /// Coeficiente `a` da regressão ṙ = a·G_ox^n, em m/s com G em kg/(m²·s).
    pub a: f64,
    /// Expoente `n`.
    pub n: f64,
    /// Comprimento do grão, m.
    pub length: f64,
    /// Diâmetro inicial do canal, m.
    pub port_diameter: f64,
    /// Diâmetro externo do grão, m.
    pub outer_diameter: f64,
    /// Velocidade característica efetiva, m/s (tomada constante).
    pub c_star: f64,
    /// γ dos produtos.
    pub gamma: f64,
    /// Diâmetro da garganta, m.
    pub throat_diameter: f64,
    /// Diâmetro de saída, m.
    pub exit_diameter: f64,
    /// Massa sem propelente, kg.
    pub dry_mass: f64,
}

/// Resultado da balística interna.
#[derive(Clone, Debug)]
pub struct HybridBurn {
    /// Motor pronto para o voo.
    pub motor: Motor,
    /// Razão oxidante/combustível em cada ponto.
    pub of_ratio: Vec<f64>,
    /// Pressão de câmara em cada ponto, Pa.
    pub chamber_pressure: Vec<f64>,
    /// Diâmetro final do canal, m.
    pub final_port: f64,
    /// Combustível queimado, kg.
    pub fuel_burned: f64,
}

impl HybridMotor {
    /// Exemplo ilustrativo de N2O sobre parafina, em ordem de grandeza (não é um motor
    /// real). Regressão da parafina na faixa citada por Karabeyoglu et al., "Scale-up
    /// tests of high regression rate paraffin-based hybrid rocket fuels", J. Propulsion
    /// and Power 20 (2004); confiabilidade "estimado".
    #[must_use]
    pub fn example_paraffin_n2o() -> Self {
        Self {
            ox_flow: 0.8,
            ox_mass: 4.0,
            fuel_density: 900.0,
            a: 1.5e-4,
            n: 0.5,
            length: 0.45,
            port_diameter: 0.035,
            outer_diameter: 0.090,
            c_star: 1_480.0,
            gamma: 1.22,
            throat_diameter: 0.022,
            exit_diameter: 0.050,
            dry_mass: 6.0,
        }
    }

    /// Integra a queima até acabar o oxidante (ou o grão):
    /// G_ox = ṁ_ox/(π·r²), ṙ = a·G_ox^n, ṁ_f = ρ_f·2π·r·L·ṙ, p_c = (ṁ_ox + ṁ_f)·c*/A_t,
    /// F = C_F·p_c·A_t. RK4 em r com passo fixo.
    /// Fonte: Marxman, Wooldridge & Muzzy, "Fundamental Processes in Hybrid Rocket
    /// Combustion", AIAA Progress 15 (1964); Sutton & Biblarz, 9ª ed., cap. 16.
    /// Não modela: variação de c* com O/F, transitórios, queima na ponta do grão.
    #[must_use]
    pub fn simulate(&self) -> HybridBurn {
        let at = 0.25 * PI * self.throat_diameter.powi(2);
        let ae = 0.25 * PI * self.exit_diameter.powi(2);
        let rdot = |r: f64| self.a * (self.ox_flow / (PI * r * r)).powf(self.n);
        let burn_time = self.ox_mass / self.ox_flow;
        const N: usize = 400;
        let h = burn_time / N as f64;
        let r_max = 0.5 * self.outer_diameter;
        let mut r = 0.5 * self.port_diameter;
        let fuel0 = self.fuel_density * PI * (r_max * r_max - r * r) * self.length;
        let mut rows = Vec::with_capacity(N + 1);
        let state = |r: f64| {
            let mf = if r < r_max {
                self.fuel_density * 2.0 * PI * r * self.length * rdot(r)
            } else {
                0.0
            };
            let pc = (self.ox_flow + mf) * self.c_star / at;
            let f = thrust_coefficient(self.gamma, ae / at, pc, P_SL) * pc * at;
            (mf, pc, f.max(0.0))
        };
        let mut t = 0.0;
        for i in 0..=N {
            let (mf, pc, f) = state(r);
            rows.push((t, f, mf, pc, r));
            if i == N {
                break;
            }
            let k1 = rdot(r);
            let k2 = rdot(r + 0.5 * h * k1);
            let k3 = rdot(r + 0.5 * h * k2);
            let k4 = rdot(r + h * k3);
            r = (r + h * (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0).min(r_max);
            t += h;
        }
        let final_port = 2.0 * r;
        let fuel_burned =
            self.fuel_density * PI * (r * r - 0.25 * self.port_diameter.powi(2)) * self.length;
        let total0 = self.ox_mass + fuel_burned;
        // Massa restante: oxidante linear + combustível pela geometria do canal.
        let points: Vec<MotorPoint> = rows
            .iter()
            .map(|&(t, f, _, _, rr)| {
                let fuel_left = self.fuel_density
                    * PI
                    * (rr * rr - 0.25 * self.port_diameter.powi(2))
                    * self.length;
                MotorPoint {
                    t,
                    thrust: f,
                    prop_mass: (total0 - self.ox_flow * t - fuel_left).max(0.0),
                }
            })
            .chain([MotorPoint {
                t: burn_time * 1.01,
                thrust: 0.0,
                prop_mass: 0.0,
            }])
            .collect();
        let _ = fuel0;
        HybridBurn {
            motor: Motor {
                name: "Híbrido".into(),
                manufacturer: "ASTRA".into(),
                diameter: self.outer_diameter + 0.01,
                length: self.length * 2.5,
                dry_mass: self.dry_mass + (fuel0 - fuel_burned).max(0.0),
                points,
                ref_pressure: P_SL,
                exit_area: ae,
            },
            of_ratio: rows
                .iter()
                .map(|r| {
                    if r.2 > 0.0 {
                        self.ox_flow / r.2
                    } else {
                        f64::INFINITY
                    }
                })
                .collect(),
            chamber_pressure: rows.iter().map(|r| r.3).collect(),
            final_port,
            fuel_burned,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Com n = 0,5 a regressão tem solução fechada: dr/dt = a·(ṁ/π)^0,5/r, logo
    /// r² = r0² + 2a·(ṁ/π)^0,5·t.
    #[test]
    fn port_growth_matches_closed_form_and_mass_is_conserved() {
        let h = HybridMotor::example_paraffin_n2o();
        let b = h.simulate();
        let tb = h.ox_mass / h.ox_flow;
        let r0 = 0.5 * h.port_diameter;
        let r = (r0 * r0 + 2.0 * h.a * (h.ox_flow / PI).sqrt() * tb).sqrt();
        assert!(
            (b.final_port - 2.0 * r).abs() < 1e-9,
            "{} vs {}",
            b.final_port,
            2.0 * r
        );
        let m = &b.motor;
        assert!((m.initial_prop_mass() - (h.ox_mass + b.fuel_burned)).abs() < 1e-9);
        assert_eq!(m.prop_mass(m.burn_time()), 0.0);
        // ṁ_f = ρ·2πrL·a·(ṁ/πr²)^n ∝ r^(1−2n): com n = 0,5 a vazão de combustível e a
        // razão O/F ficam constantes enquanto o canal abre.
        assert!((b.of_ratio[0] / b.of_ratio[300] - 1.0).abs() < 1e-6);
        let isp = m.specific_impulse();
        assert!(isp > 150.0 && isp < 280.0, "Isp {isp}");
    }

    #[test]
    fn of_ratio_shifts_when_n_differs_from_half() {
        let h = HybridMotor {
            n: 0.62,
            ..HybridMotor::example_paraffin_n2o()
        };
        let b = h.simulate();
        assert!(
            b.of_ratio[350] > b.of_ratio[0],
            "com n > 0,5 a mistura enriquece em oxidante"
        );
    }
}
