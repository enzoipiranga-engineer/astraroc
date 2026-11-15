use crate::geometry::{FinPlanform, FinSet, Geometry, PartIntegrals};
use core::f64::consts::PI;

/// Força normal total e centro de pressão em uma condição de voo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalForce {
    /// Coeficiente de força normal CN (referido à área de referência).
    pub cn: f64,
    /// Derivada CNα em α → 0, 1/rad (só os termos lineares de Barrowman).
    pub cn_alpha: f64,
    /// Estação do centro de pressão, m a partir da ponta do nariz.
    pub x_cp: f64,
}

/// Constante de sustentação de corpo (escoamento cruzado), K ≈ 1,1.
/// Fonte: Galejs, "Wind Instability: What Barrowman Left Out" (1999).
const BODY_LIFT_K: f64 = 1.1;

/// Posição do centro de pressão da aleta como fração da corda média aerodinâmica:
/// 25% no subsônico (M ≤ 0,5), 50% no supersônico (M ≥ 2), interpolação linear entre os dois.
/// Fonte dos limites: Barrowman (1967) para o subsônico; teoria linear supersônica
/// (Anderson, "Fundamentals of Aerodynamics", §12.3) para o supersônico.
fn fin_cp_fraction(mach: f64) -> f64 {
    0.25 + 0.25 * ((mach - 0.5) / 1.5).clamp(0.0, 1.0)
}

impl FinSet {
    /// CNα de uma única aleta isolada, referido a `a_ref`.
    ///
    /// Subsônico (M ≤ 0,9), Diederich com correção de Prandtl-Glauert:
    /// CNα = 2π·AR·(A_f/A_ref) / (2 + √(4 + (β·AR/cosΓc)²)), β = √(1 − M²), AR = 2s²/A_f.
    /// Em M = 0 isso é exatamente a fórmula de Barrowman.
    /// Supersônico (M ≥ 1,5), teoria linear de placa plana: CNα = (4/√(M² − 1))·A_f/A_ref.
    /// Entre 0,9 e 1,5, interpolação linear em Mach.
    /// Fontes: Barrowman, "The Practical Calculation of the Aerodynamic Characteristics
    /// of Slender Finned Vehicles" (1967); Diederich, NACA TN-2335 (1951);
    /// Anderson, "Fundamentals of Aerodynamics", cap. 12.
    #[must_use]
    pub fn single_fin_cn_alpha(&self, mach: f64, a_ref: f64) -> f64 {
        let af = self.area();
        let ar = 2.0 * self.span * self.span / af;
        let sub = |m: f64| {
            let beta = (1.0 - m * m).sqrt();
            2.0 * PI * ar * (af / a_ref)
                / (2.0 + (4.0 + (beta * ar / self.cos_mid_sweep()).powi(2)).sqrt())
        };
        let sup = |m: f64| 4.0 / (m * m - 1.0).sqrt() * af / a_ref;
        if mach <= 0.9 {
            sub(mach)
        } else if mach >= 1.5 {
            sup(mach)
        } else {
            let w = (mach - 0.9) / 0.6;
            sub(0.9) * (1.0 - w) + sup(1.5) * w
        }
    }

    /// Estação do centro de pressão do conjunto, m.
    #[must_use]
    pub fn x_cp(&self, mach: f64) -> f64 {
        let f = fin_cp_fraction(mach);
        match self.planform {
            FinPlanform::Trapezoid { .. } => {
                let (mac, x_mac) = self.mac();
                self.position + x_mac + f * mac
            }
            // Barrowman dá 0,288·cr para a aleta elíptica no subsônico.
            FinPlanform::Ellipse { root_chord } => {
                self.position + root_chord * (0.288 + (0.5 - 0.288) * (f - 0.25) / 0.25)
            }
        }
    }
}

impl Geometry {
    /// Contribuições lineares (CNα, estação do CP) de cada componente.
    ///
    /// Corpo (nariz, transições), teoria de corpo esbelto:
    /// CNα = 2·(A_tras − A_frente)/A_ref e x_cp = (L·A_tras − Vol)/(A_tras − A_frente).
    /// Aletas: N/2 aletas efetivas (exato para N ≥ 3 por simetria), com o fator de
    /// interferência aleta-corpo K = 1 + r/(s + r).
    /// Fonte: Barrowman (1967), cap. 3.
    #[must_use]
    pub fn linear_components(&self, mach: f64) -> Vec<(f64, f64)> {
        self.linear_with(&self.integrals(), mach)
    }

    pub(crate) fn linear_with(&self, ints: &[PartIntegrals], mach: f64) -> Vec<(f64, f64)> {
        let a_ref = self.ref_area();
        let mut out = Vec::new();
        for ((x0, p), int) in self.stations().zip(ints) {
            let (d0, d1) = p.diameters();
            let (a0, a1) = (0.25 * PI * d0 * d0, 0.25 * PI * d1 * d1);
            if (a1 - a0).abs() > 1e-12 * a_ref {
                let vol = int.volume;
                out.push((
                    2.0 * (a1 - a0) / a_ref,
                    x0 + (p.length() * a1 - vol) / (a1 - a0),
                ));
            }
        }
        for f in &self.fins {
            let r = self.body_radius_at(f.position + 0.5 * f.root_chord());
            let k = 1.0 + r / (f.span + r);
            let n_eff = if f.count >= 3 {
                0.5 * f64::from(f.count)
            } else {
                1.0
            };
            out.push((k * n_eff * f.single_fin_cn_alpha(mach, a_ref), f.x_cp(mach)));
        }
        out
    }

    /// Força normal e centro de pressão em Mach `mach` e ângulo de ataque `alpha` (rad).
    ///
    /// CN = sinα·ΣCNαᵢ + sin²α·K·A_planta/A_ref. O segundo termo é a sustentação de corpo
    /// de Galejs, aplicada no centroide da área em planta; ela desloca o CP para a frente
    /// com o aumento de α. Em α = 0 o resultado é o de Barrowman.
    #[must_use]
    pub fn normal_force(&self, mach: f64, alpha: f64) -> NormalForce {
        self.normal_with(&self.integrals(), mach, alpha)
    }

    pub(crate) fn normal_with(&self, ints: &[PartIntegrals], mach: f64, alpha: f64) -> NormalForce {
        let lin = self.linear_with(ints, mach);
        let cn_alpha: f64 = lin.iter().map(|c| c.0).sum();
        let lin_moment: f64 = lin.iter().map(|c| c.0 * c.1).sum();
        let (mut plan, mut plan_moment) = (0.0, 0.0);
        for ((x0, _), i) in self.stations().zip(ints) {
            plan += i.planform;
            plan_moment += i.planform_moment + x0 * i.planform;
        }
        let (s, kb) = (alpha.sin().abs(), BODY_LIFT_K / self.ref_area());
        let cn = s * cn_alpha + s * s * kb * plan;
        // Em α → 0 o termo de corpo some mais rápido que o linear.
        let (num, den) = (lin_moment + s * kb * plan_moment, cn_alpha + s * kb * plan);
        NormalForce {
            cn,
            cn_alpha,
            x_cp: if den.abs() > 0.0 { num / den } else { 0.0 },
        }
    }

    /// Margem estática em calibres: (x_cp − x_cg)/d, com o CP de Barrowman (α = 0).
    #[must_use]
    pub fn static_margin(&self, mach: f64, x_cg: f64) -> f64 {
        (self.normal_force(mach, 0.0).x_cp - x_cg) / self.ref_diameter()
    }

    /// Soma de amortecimento de arfagem, Σ CNαᵢ·(xᵢ − x_cg)², em m².
    ///
    /// Uma rotação ω dá a cada componente um ângulo de ataque local ω·(xᵢ − x_cg)/V; a
    /// força normal resultante gera o momento restaurador
    /// M = −q·A_ref·Σ CNαᵢ·(xᵢ − x_cg)²·ω/V.
    /// Fonte: Mandell, Caporaso & Bengen, "Topics in Advanced Model Rocketry" (1973), cap. 2.
    #[must_use]
    pub fn pitch_damping_sum(&self, mach: f64, x_cg: f64) -> f64 {
        self.linear_components(mach)
            .iter()
            .map(|c| c.0 * (c.1 - x_cg).powi(2))
            .sum()
    }
}
