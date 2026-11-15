use crate::geometry::{BodyPart, FinProfile, Geometry, NoseShape, PartIntegrals};
use core::f64::consts::PI;

/// Condição do escoamento para o cálculo de arrasto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowState {
    /// Número de Mach.
    pub mach: f64,
    /// Número de Reynolds por metro, ρ·V/μ, 1/m.
    pub reynolds_per_m: f64,
    /// Motor aceso (o jato ocupa parte da base).
    pub burning: bool,
}

/// Coeficiente de arrasto em α = 0, por parcela, referido à área de referência.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DragBreakdown {
    /// Atrito de corpo e aletas.
    pub friction: f64,
    /// Pressão no nariz e em transições que aumentam o diâmetro.
    pub forebody: f64,
    /// Base do corpo e boattail.
    pub base: f64,
    /// Pressão nas aletas (bordos de ataque e de fuga).
    pub fins: f64,
}

/// Coeficiente de arrasto de cada peça externa (mesma referência do total).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartDrag {
    /// Um valor por trecho do corpo, na ordem de [`Geometry::parts`].
    pub body: Vec<f64>,
    /// Um valor por conjunto de aletas, na ordem de [`Geometry::fins`].
    pub fins: Vec<f64>,
}

impl DragBreakdown {
    /// Soma das parcelas.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.friction + self.forebody + self.base + self.fins
    }
}

fn lerp(a: f64, b: f64, w: f64) -> f64 {
    a + (b - a) * w.clamp(0.0, 1.0)
}

/// Coeficiente de atrito de placa plana, camada limite toda turbulenta, com limite
/// por rugosidade e correção de compressibilidade.
///
/// Incompressível: Cf = 0,0148 se Re < 1e4; Cf = 1/(1,50·ln Re − 5,6)² se Re < Re_crit;
/// Cf = 0,032·(Rs/L)^0,2 acima de Re_crit = 51·(Rs/L)^−1,039.
/// Compressibilidade: ×(1 − 0,1M²) no subsônico; no supersônico ×(1 + 0,15M²)^−0,58
/// (liso) ou ×1/(1 + 0,18M²) (limitado por rugosidade); interpolação linear em 0,9 < M < 1,1.
/// Fonte: Niskanen, "OpenRocket technical documentation" (2013), cap. 3,
/// baseadas em Barrowman (1967) e Hoerner, "Fluid-Dynamic Drag".
fn skin_friction(re: f64, rel_roughness: f64, mach: f64) -> f64 {
    let re_crit = if rel_roughness > 0.0 {
        51.0 * rel_roughness.powf(-1.039)
    } else {
        f64::INFINITY
    };
    let (cf, rough) = if re < 1e4 {
        (1.48e-2, false)
    } else if re < re_crit {
        (1.0 / (1.5 * re.ln() - 5.6).powi(2), false)
    } else {
        (0.032 * rel_roughness.powf(0.2), true)
    };
    let sub = |m: f64| 1.0 - 0.1 * m * m;
    let sup = |m: f64| {
        if rough {
            1.0 / (1.0 + 0.18 * m * m)
        } else {
            (1.0 + 0.15 * m * m).powf(-0.58)
        }
    };
    cf * if mach <= 0.9 {
        sub(mach)
    } else if mach >= 1.1 {
        sup(mach)
    } else {
        lerp(sub(0.9), sup(1.1), (mach - 0.9) / 0.2)
    }
}

/// Arrasto de base referido à própria área de base: 0,12 + 0,13M² (M < 1), 0,25/M (M ≥ 1).
/// Fonte: Niskanen (2013), cap. 3 (ajuste a dados de Hoerner).
fn base_cd(mach: f64) -> f64 {
    if mach < 1.0 {
        0.12 + 0.13 * mach * mach
    } else {
        0.25 / mach
    }
}

/// Fator de forma do arrasto de onda do nariz em relação ao cone de mesma esbeltez.
/// Valores estimados a partir das curvas comparativas de Chin, "Missile Configuration
/// Design" (1961), fig. 2-10, e Crowell (1996); confiabilidade baixa (±30%).
fn nose_wave_factor(shape: NoseShape) -> f64 {
    match shape {
        NoseShape::Conical => 1.0,
        NoseShape::TangentOgive => 0.95,
        NoseShape::Parabolic => 0.85,
        NoseShape::LvHaack => 0.78,
        NoseShape::VonKarman => 0.75,
        NoseShape::Elliptical => 1.3,
    }
}

/// Arrasto de pressão de um corpo de frente (nariz ou ombro), referido à área que ele
/// acrescenta. `sin_joint` é o seno do ângulo da superfície com o eixo na junção
/// traseira; `sin_cone` o do cone equivalente (R/√(R² + L²)).
///
/// Subsônico (M ≤ 0,8): 0,8·sin²φ, nulo para perfis tangentes ao corpo.
/// Supersônico (M ≥ 1,3), cone: 2,1·sin²ε + 0,5·sinε/√(M² − 1), vezes o fator de forma.
/// Entre os dois, interpolação linear em Mach.
/// Fonte: Niskanen (2013), cap. 3 apêndice B.
fn forebody_cd(sin_joint: f64, sin_cone: f64, factor: f64, mach: f64) -> f64 {
    let sub = 0.8 * sin_joint * sin_joint;
    let sup = |m: f64| factor * (2.1 * sin_cone * sin_cone + 0.5 * sin_cone / (m * m - 1.0).sqrt());
    if mach <= 0.8 {
        sub
    } else if mach >= 1.3 {
        sup(mach)
    } else {
        lerp(sub, sup(1.3), (mach - 0.8) / 0.5)
    }
}

/// Arrasto do bordo de ataque da aleta, referido à área frontal (espessura × envergadura).
/// Arredondado: (1 − M²)^−0,417 − 1 (M < 0,9); 1 − 1,785(M − 0,9) (0,9 ≤ M < 1);
/// 1,214 − 0,502/M² + 0,1095/M⁴ (M ≥ 1).
/// Reto: 0,85 vezes a pressão de estagnação adimensional, q_stag/q = 1 + M²/4 + M⁴/40
/// (M < 1) ou 1,84 − 0,76/M² + 0,166/M⁴ + 0,035/M⁶ (M ≥ 1).
/// Fonte: Niskanen (2013), cap. 3
fn fin_leading_edge_cd(profile: FinProfile, mach: f64) -> f64 {
    let m2 = mach * mach;
    match profile {
        FinProfile::Square => {
            0.85 * if mach < 1.0 {
                1.0 + m2 / 4.0 + m2 * m2 / 40.0
            } else {
                1.84 - 0.76 / m2 + 0.166 / (m2 * m2) + 0.035 / (m2 * m2 * m2)
            }
        }
        FinProfile::Rounded | FinProfile::Airfoil => {
            if mach < 0.9 {
                (1.0 - m2).powf(-0.417) - 1.0
            } else if mach < 1.0 {
                1.0 - 1.785 * (mach - 0.9)
            } else {
                1.214 - 0.502 / m2 + 0.1095 / (m2 * m2)
            }
        }
    }
}

impl Geometry {
    /// Coeficiente de arrasto em ângulo de ataque nulo, por parcela.
    ///
    /// Atrito: Cf·[(1 + 1/(2·fB))·A_molhada,corpo + (1 + 2t/c)·A_molhada,aletas]/A_ref,
    /// com fB = L/d a esbeltez do corpo (Niskanen 2013, cap. 3).
    /// Boattail: arrasto de base sobre a área removida, vezes γ = 1 se L/Δd < 1,
    /// (3 − L/Δd)/2 se 1 ≤ L/Δd ≤ 3 e 0 acima (idem).
    /// Durante a queima, a área de saída da tubeira é descontada da base.
    #[must_use]
    pub fn drag(&self, flow: &FlowState) -> DragBreakdown {
        self.drag_with(&self.integrals(), flow)
    }

    pub(crate) fn drag_with(&self, ints: &[PartIntegrals], flow: &FlowState) -> DragBreakdown {
        self.drag_split(ints, flow).0
    }

    /// Arrasto por parcela e, ao mesmo tempo, por peça: coeficiente de cada trecho do
    /// corpo e de cada conjunto de aletas (somam o total). O atrito do corpo é repartido
    /// pela área molhada; pressão, base e boattail ficam no trecho que os gera.
    pub(crate) fn drag_split(
        &self,
        ints: &[PartIntegrals],
        flow: &FlowState,
    ) -> (DragBreakdown, PartDrag) {
        let (a_ref, d_ref, len) = (self.ref_area(), self.ref_diameter(), self.length());
        let m = flow.mach;
        let cf = skin_friction(flow.reynolds_per_m * len, self.roughness / len, m);
        let body_form = 1.0 + d_ref / (2.0 * len);

        let mut out = DragBreakdown::default();
        let mut parts = PartDrag {
            body: vec![0.0; self.parts.len()],
            fins: vec![0.0; self.fins.len()],
        };
        for (i, (p, int)) in self.parts.iter().zip(ints).enumerate() {
            let (d0, d1) = p.diameters();
            let (r0, r1, l) = (0.5 * d0, 0.5 * d1, p.length());
            let da = PI * (r1 * r1 - r0 * r0);
            let friction = cf * body_form * int.wetted / a_ref;
            let (mut fore, mut base) = (0.0, 0.0);
            match *p {
                BodyPart::Nose { shape, .. } => {
                    let sin_cone = r1 / r1.hypot(l);
                    let sin_joint = if shape == NoseShape::Conical {
                        sin_cone
                    } else {
                        0.0
                    };
                    fore +=
                        forebody_cd(sin_joint, sin_cone, nose_wave_factor(shape), m) * da / a_ref;
                }
                BodyPart::Transition { .. } if da > 0.0 => {
                    let s = (r1 - r0) / (r1 - r0).hypot(l);
                    fore += forebody_cd(s, s, 1.0, m) * da / a_ref;
                }
                BodyPart::Transition { .. } if da < 0.0 => {
                    let ratio = l / (d0 - d1);
                    let gamma = ((3.0 - ratio) / 2.0).clamp(0.0, 1.0);
                    base += base_cd(m) * gamma * (-da) / a_ref;
                }
                _ => {}
            }
            // Frente rombuda (corpo sem nariz, por exemplo depois de perder a coifa):
            // face plana com Cd = 0,85·q_estagnação/q sobre a área frontal, a mesma forma
            // usada para bordo de ataque reto. Fonte: Hoerner, "Fluid-Dynamic Drag", cap. 3.
            if i == 0 && d0 > 0.0 {
                fore += fin_leading_edge_cd(FinProfile::Square, m) * 0.25 * PI * d0 * d0 / a_ref;
            }
            out.friction += friction;
            out.forebody += fore;
            out.base += base;
            parts.body[i] += friction + fore + base;
        }
        for (k, f) in self.fins.iter().enumerate() {
            let n = f64::from(f.count);
            let (mac, _) = f.mac();
            let friction = cf * (1.0 + 2.0 * f.thickness / mac) * 2.0 * n * f.area() / a_ref;
            let frontal = n * f.thickness * f.span / a_ref;
            let te = match f.profile {
                FinProfile::Square => base_cd(m),
                FinProfile::Rounded => 0.5 * base_cd(m),
                FinProfile::Airfoil => 0.0,
            };
            let pressure =
                frontal * (fin_leading_edge_cd(f.profile, m) * f.cos_le_sweep().powi(2) + te);
            out.friction += friction;
            out.fins += pressure;
            parts.fins[k] = friction + pressure;
        }
        let d_base = self.parts.last().map_or(0.0, |p| p.diameters().1);
        let mut a_base = 0.25 * PI * d_base * d_base;
        if flow.burning {
            a_base = (a_base - 0.25 * PI * self.nozzle_exit_diameter.powi(2)).max(0.0);
        }
        let base = base_cd(m) * a_base / a_ref;
        out.base += base;
        if let Some(last) = parts.body.last_mut() {
            *last += base;
        }
        (out, parts)
    }
}
