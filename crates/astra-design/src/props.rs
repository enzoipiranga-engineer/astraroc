//! Massa, CG, inércia e extensão geométrica de cada peça.

use crate::material::MaterialDb;
use crate::model::{NoseShapeSpec, Part, PartKind};
use astra_aero::NoseShape;
use core::f64::consts::PI;

/// Propriedades físicas de uma peça já posicionada.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PartProps {
    /// Massa, kg.
    pub mass: f64,
    /// Estação do CG, m.
    pub cg: f64,
    /// Momento de inércia em torno do eixo do foguete, kg·m².
    pub i_axial: f64,
    /// Momento de inércia transversal em torno do próprio CG, kg·m².
    pub i_trans: f64,
    /// Estação dianteira ocupada, m.
    pub x0: f64,
    /// Estação traseira ocupada, m.
    pub x1: f64,
    /// Raio externo, m.
    pub r_out: f64,
    /// Raio interno livre (tubos), m; 0 para peças maciças.
    pub r_in: f64,
    /// Volume ocupado (para o volume livre das baias), m³.
    pub volume: f64,
}

pub(crate) fn nose_shape(s: NoseShapeSpec) -> NoseShape {
    match s {
        NoseShapeSpec::Conical => NoseShape::Conical,
        NoseShapeSpec::TangentOgive => NoseShape::TangentOgive,
        NoseShapeSpec::Elliptical => NoseShape::Elliptical,
        NoseShapeSpec::Parabolic => NoseShape::Parabolic,
        NoseShapeSpec::VonKarman => NoseShape::VonKarman,
        NoseShapeSpec::LvHaack => NoseShape::LvHaack,
    }
}

/// Casca fina de revolução de espessura `t` e raio `r(x)`, x ∈ [0, L] local:
/// dm = ρ·t·2π·r·ds; I_axial = Σ dm·r²; I_trans = Σ dm·(r²/2 + (x − x_cg)²).
/// Devolve (massa, x_cg local, I_axial, I_trans no CG).
fn thin_shell(rho: f64, t: f64, len: f64, r: impl Fn(f64) -> f64) -> (f64, f64, f64, f64) {
    const N: usize = 240;
    let h = len / N as f64;
    let mut seg = Vec::with_capacity(N);
    for i in 0..N {
        let (xa, xb) = (i as f64 * h, (i + 1) as f64 * h);
        let (ra, rb) = (r(xa), r(xb));
        let ds = h.hypot(rb - ra);
        let rm = 0.5 * (ra + rb);
        seg.push((rho * t * 2.0 * PI * rm * ds, 0.5 * (xa + xb), rm));
    }
    let m: f64 = seg.iter().map(|s| s.0).sum();
    if m <= 0.0 {
        return (0.0, 0.5 * len, 0.0, 0.0);
    }
    let xc = seg.iter().map(|s| s.0 * s.1).sum::<f64>() / m;
    let ia = seg.iter().map(|s| s.0 * s.2 * s.2).sum();
    let it = seg
        .iter()
        .map(|s| s.0 * (0.5 * s.2 * s.2 + (s.1 - xc).powi(2)))
        .sum();
    (m, xc, ia, it)
}

/// Cilindro oco de raios `ro` e `ri` e comprimento `l`:
/// I_axial = m(ro² + ri²)/2, I_trans = m(3(ro² + ri²) + l²)/12.
fn hollow_cylinder(m: f64, ro: f64, ri: f64, l: f64) -> (f64, f64) {
    let s = ro * ro + ri * ri;
    (0.5 * m * s, m * (3.0 * s + l * l) / 12.0)
}

/// Gramatura assumida quando o tecido do paraquedas não informa, kg/m².
const DEFAULT_CANOPY_AREAL: f64 = 0.045;
/// Fator de massa de linhas, costuras e bainhas sobre o tecido do velame (estimado).
const CANOPY_HARDWARE_FACTOR: f64 = 1.3;

impl PartProps {
    /// Calcula as propriedades de uma peça. Material ausente dá massa específica nula
    /// (o validador acusa o erro).
    #[must_use]
    pub fn of(part: &Part, db: &MaterialDb) -> Self {
        let mat = db.get(&part.material);
        let rho = mat.map_or(0.0, |m| m.density_kg_m3);
        let x = part.position;
        let mut p = match part.kind {
            PartKind::NoseCone {
                shape,
                length,
                diameter,
                wall,
                shoulder_length,
            } => {
                let shape = nose_shape(shape);
                let fin = length / diameter;
                let (m1, xc1, ia1, it1) = thin_shell(rho, wall, length, |s| {
                    0.5 * diameter * shape.profile(s / length, fin)
                });
                let (ro, ri) = (0.5 * diameter - wall, 0.5 * diameter - 2.0 * wall);
                let m2 = rho * PI * (ro * ro - ri.max(0.0).powi(2)) * shoulder_length;
                let (ia2, it2) = hollow_cylinder(m2, ro, ri.max(0.0), shoulder_length);
                let xc2 = length + 0.5 * shoulder_length;
                let m = m1 + m2;
                let xc = if m > 0.0 {
                    (m1 * xc1 + m2 * xc2) / m
                } else {
                    0.5 * length
                };
                let it = it1 + m1 * (xc1 - xc).powi(2) + it2 + m2 * (xc2 - xc).powi(2);
                let vol = PI / 3.0 * (0.5 * diameter).powi(2) * length;
                Self {
                    mass: m,
                    cg: x + xc,
                    i_axial: ia1 + ia2,
                    i_trans: it,
                    x0: x,
                    x1: x + length + shoulder_length,
                    r_out: 0.5 * diameter,
                    r_in: 0.5 * diameter - wall,
                    volume: vol,
                }
            }
            PartKind::Transition {
                length,
                d_fore,
                d_aft,
                wall,
            } => {
                let (m, xc, ia, it) = thin_shell(rho, wall, length, |s| {
                    0.5 * (d_fore + (d_aft - d_fore) * s / length)
                });
                Self {
                    mass: m,
                    cg: x + xc,
                    i_axial: ia,
                    i_trans: it,
                    x0: x,
                    x1: x + length,
                    r_out: 0.5 * d_fore.max(d_aft),
                    r_in: 0.5 * d_fore.min(d_aft) - wall,
                    volume: 0.0,
                }
            }
            PartKind::BodyTube {
                length,
                outer_diameter,
                wall,
                ..
            }
            | PartKind::Coupler {
                length,
                outer_diameter,
                wall,
            }
            | PartKind::MotorMount {
                length,
                outer_diameter,
                wall,
            } => {
                let (ro, ri) = (0.5 * outer_diameter, (0.5 * outer_diameter - wall).max(0.0));
                let m = rho * PI * (ro * ro - ri * ri) * length;
                let (ia, it) = hollow_cylinder(m, ro, ri, length);
                let vol = if matches!(part.kind, PartKind::BodyTube { .. }) {
                    0.0
                } else {
                    PI * ro * ro * length
                };
                Self {
                    mass: m,
                    cg: x + 0.5 * length,
                    i_axial: ia,
                    i_trans: it,
                    x0: x,
                    x1: x + length,
                    r_out: ro,
                    r_in: ri,
                    volume: vol,
                }
            }
            PartKind::Bulkhead {
                diameter,
                thickness,
            } => {
                let r = 0.5 * diameter;
                let m = rho * PI * r * r * thickness;
                let (ia, it) = hollow_cylinder(m, r, 0.0, thickness);
                Self {
                    mass: m,
                    cg: x + 0.5 * thickness,
                    i_axial: ia,
                    i_trans: it,
                    x0: x,
                    x1: x + thickness,
                    r_out: r,
                    r_in: 0.0,
                    volume: PI * r * r * thickness,
                }
            }
            PartKind::CenteringRing {
                outer_diameter,
                inner_diameter,
                thickness,
            } => {
                let (ro, ri) = (0.5 * outer_diameter, 0.5 * inner_diameter);
                let m = rho * PI * (ro * ro - ri * ri) * thickness;
                let (ia, it) = hollow_cylinder(m, ro, ri, thickness);
                Self {
                    mass: m,
                    cg: x + 0.5 * thickness,
                    i_axial: ia,
                    i_trans: it,
                    x0: x,
                    x1: x + thickness,
                    r_out: ro,
                    r_in: ri,
                    volume: PI * (ro * ro - ri * ri) * thickness,
                }
            }
            PartKind::FinSet {
                count,
                root_chord: cr,
                tip_chord: ct,
                sweep,
                span,
                thickness,
                ..
            } => {
                // Placa trapezoidal: área (cr + ct)·s/2; centroide da planta a partir do bordo
                // de ataque da raiz, x̄ = [xs(cr + 2ct) + cr² + cr·ct + ct²]/(3(cr + ct)) e
                // ȳ = s(cr + 2ct)/(3(cr + ct)).
                let area = 0.5 * (cr + ct) * span;
                let mf = rho * area * thickness;
                let n = f64::from(count);
                let xb =
                    (sweep * (cr + 2.0 * ct) + cr * cr + cr * ct + ct * ct) / (3.0 * (cr + ct));
                let yb = span * (cr + 2.0 * ct) / (3.0 * (cr + ct));
                // O raio do corpo é desconhecido aqui; o Assembly soma o raio do tubo depois.
                Self {
                    mass: n * mf,
                    cg: x + xb,
                    i_axial: n * mf * yb * yb,
                    i_trans: n * mf * (0.5 * yb * yb + (cr * cr + ct * ct) / 18.0),
                    x0: x,
                    x1: x + cr.max(sweep + ct),
                    r_out: span,
                    r_in: yb,
                    volume: 0.0,
                }
            }
            PartKind::Parachute {
                diameter,
                packed_length,
                packed_diameter,
                ..
            } => {
                let areal = mat
                    .and_then(|m| m.areal_gm2)
                    .map_or(DEFAULT_CANOPY_AREAL, |g| g / 1000.0);
                let m = areal * 0.25 * PI * diameter * diameter * CANOPY_HARDWARE_FACTOR;
                let r = 0.5 * packed_diameter;
                let (ia, it) = hollow_cylinder(m, r, 0.0, packed_length);
                Self {
                    mass: m,
                    cg: x + 0.5 * packed_length,
                    i_axial: ia,
                    i_trans: it,
                    x0: x,
                    x1: x + packed_length,
                    r_out: r,
                    r_in: 0.0,
                    volume: PI * r * r * packed_length,
                }
            }
            PartKind::ShockCord {
                length,
                linear_density,
                ..
            } => {
                let m = length * linear_density;
                Self {
                    mass: m,
                    cg: x + 0.025,
                    i_axial: 0.0,
                    i_trans: 0.0,
                    x0: x,
                    x1: x + 0.05,
                    r_out: 0.0,
                    r_in: 0.0,
                    volume: m / 500.0,
                }
            }
            PartKind::Mass {
                mass,
                length,
                diameter,
                ..
            } => {
                let r = 0.5 * diameter;
                let (ia, it) = hollow_cylinder(mass, r, 0.0, length);
                Self {
                    mass,
                    cg: x + 0.5 * length,
                    i_axial: ia,
                    i_trans: it,
                    x0: x,
                    x1: x + length,
                    r_out: r,
                    r_in: 0.0,
                    volume: PI * r * r * length,
                }
            }
        };
        if let Some(m) = part.mass_override {
            let k = if p.mass > 0.0 { m / p.mass } else { 0.0 };
            p.i_axial *= k;
            p.i_trans *= k;
            p.mass = m;
        }
        p
    }
}

/// Peça que forma a superfície externa (entra na aerodinâmica).
#[must_use]
pub fn is_external(k: &PartKind) -> bool {
    matches!(
        k,
        PartKind::NoseCone { .. } | PartKind::BodyTube { .. } | PartKind::Transition { .. }
    )
}

/// Peça que é parte da estrutura do tubo (casca externa, luva, tubo do motor).
#[must_use]
pub fn is_airframe(k: &PartKind) -> bool {
    is_external(k) || matches!(k, PartKind::Coupler { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NoseShapeSpec;

    fn part(kind: PartKind) -> Part {
        Part {
            id: "p".into(),
            name: "p".into(),
            material: "al-6061-t6".into(),
            position: 1.0,
            mass_override: None,
            cost: None,
            kind,
        }
    }

    #[test]
    fn tube_and_cone_shell_by_hand() {
        let db = MaterialDb::builtin();
        let t = PartProps::of(
            &part(PartKind::BodyTube {
                length: 1.0,
                outer_diameter: 0.1,
                wall: 0.002,
                vent_holes: 0,
                vent_diameter: 0.003,
            }),
            &db,
        );
        let m = 2700.0 * PI * (0.05_f64.powi(2) - 0.048_f64.powi(2));
        assert!((t.mass - m).abs() < 1e-12 && (t.cg - 1.5).abs() < 1e-12);
        // Casca cônica: centroide da superfície lateral a 2L/3 do vértice; massa ρ·t·π·R·g.
        let c = PartProps::of(
            &part(PartKind::NoseCone {
                shape: NoseShapeSpec::Conical,
                length: 0.3,
                diameter: 0.1,
                wall: 0.002,
                shoulder_length: 0.0,
            }),
            &db,
        );
        let g = 0.3_f64.hypot(0.05);
        assert!((c.mass / (2700.0 * 0.002 * PI * 0.05 * g) - 1.0).abs() < 1e-6);
        assert!((c.cg - (1.0 + 0.2)).abs() < 1e-6);
        let ov = Part {
            mass_override: Some(0.5),
            ..part(PartKind::Bulkhead {
                diameter: 0.1,
                thickness: 0.006,
            })
        };
        assert_eq!(PartProps::of(&ov, &db).mass, 0.5);
    }
}
