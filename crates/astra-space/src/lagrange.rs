//! Pontos de Lagrange do problema restrito de três corpos circular (CR3BP).

use serde::Serialize;

/// Posições dos pontos L1 a L5 no referencial girante (primário em −μ·d, secundário em
/// (1−μ)·d), m, e constante de Jacobi de cada um.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LagrangePoints {
    /// Razão de massa μ = m2/(m1+m2).
    pub mu: f64,
    /// (x, y) de L1..L5, m.
    pub points: [[f64; 2]; 5],
    /// Constante de Jacobi adimensional de L1..L5.
    pub jacobi: [f64; 5],
}

/// Resolve os pontos colineares (raízes de ∂Ω/∂x = 0 por Newton) e os triangulares
/// (vértices de triângulos equiláteros). Curtis, §2.12; Szebehely, Theory of Orbits (1967).
#[must_use]
pub fn lagrange(gm1: f64, gm2: f64, distance: f64) -> LagrangePoints {
    let mu = gm2 / (gm1 + gm2);
    let dx = |x: f64| {
        let r1 = (x + mu).abs();
        let r2 = (x - 1.0 + mu).abs();
        x - (1.0 - mu) * (x + mu) / r1.powi(3) - mu * (x - 1.0 + mu) / r2.powi(3)
    };
    let solve = |mut x: f64| {
        for _ in 0..100 {
            let h = 1e-7;
            let d = (dx(x + h) - dx(x - h)) / (2.0 * h);
            let s = dx(x) / d;
            x -= s;
            if s.abs() < 1e-14 {
                break;
            }
        }
        x
    };
    let hill = (mu / 3.0).cbrt();
    let l1 = solve(1.0 - mu - hill);
    let l2 = solve(1.0 - mu + hill);
    let l3 = solve(-1.0 - 5.0 * mu / 12.0);
    let tri = [0.5 - mu, 3f64.sqrt() / 2.0];
    let pts = [[l1, 0.0], [l2, 0.0], [l3, 0.0], tri, [tri[0], -tri[1]]];
    let jac = |p: [f64; 2]| {
        let r1 = ((p[0] + mu).powi(2) + p[1] * p[1]).sqrt();
        let r2 = ((p[0] - 1.0 + mu).powi(2) + p[1] * p[1]).sqrt();
        p[0] * p[0] + p[1] * p[1] + 2.0 * (1.0 - mu) / r1 + 2.0 * mu / r2
    };
    LagrangePoints {
        mu,
        points: pts.map(|p| [p[0] * distance, p[1] * distance]),
        jacobi: pts.map(jac),
    }
}
