//! Vento: perfil médio por altitude e turbulência de Dryden.

use astra_math::Vec3;
use core::f64::consts::PI;

/// Camada do perfil de vento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindLayer {
    /// Altura acima do solo, m.
    pub agl: f64,
    /// Velocidade, m/s.
    pub speed: f64,
    /// Direção de onde o vento vem, rad a partir do norte (convenção meteorológica).
    pub from: f64,
}

/// Turbulência como campo congelado fixo à Terra, com espectro de Dryden.
#[derive(Clone, Debug, PartialEq)]
pub struct Turbulence {
    /// Desvio-padrão de cada componente, m/s.
    pub sigma: f64,
    /// Escala de comprimento, m.
    pub length: f64,
    /// Semente (mesma semente, mesmo campo).
    pub seed: u64,
    waves: Vec<Wave>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Wave {
    k: Vec3,
    phase: f64,
    amp: Vec3,
}

/// Gerador pseudoaleatório SplitMix64: determinístico e independente de plataforma.
/// Fonte: Steele, Lea & Flood, "Fast splittable pseudorandom number generators" (2014).
pub(crate) struct SplitMix(u64);

impl SplitMix {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniforme em [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Número de ondas por componente.
const WAVES: usize = 64;

impl Turbulence {
    /// Monta o campo pela representação espectral (soma de ondas planas com fases
    /// aleatórias). Cada componente u, v, w recebe ondas com número de onda κ em escala
    /// logarítmica entre 1e-3/L e 1e2/L, direção aleatória, e amplitude
    /// a = √(2·Φ(κ)·Δκ), com o espectro unidimensional de Dryden
    /// Φ(κ) = σ²·(2L/π)/(1 + (Lκ)²), cuja integral em κ ∈ [0, ∞) vale σ².
    /// Fontes: MIL-HDBK-1797 (forma de Dryden); Shinozuka & Deodatis, "Simulation of
    /// stochastic processes by spectral representation", Appl. Mech. Rev. 44 (1991).
    /// Simplificação: as três componentes usam o mesmo espectro longitudinal e o campo é
    /// isotrópico (sem a forma transversal de Dryden nem a variação com a altitude).
    #[must_use]
    pub fn new(sigma: f64, length: f64, seed: u64) -> Self {
        let mut rng = SplitMix(seed);
        let (lo, hi) = ((1e-3 / length).ln(), (1e2 / length).ln());
        let dln = (hi - lo) / WAVES as f64;
        let phi = |k: f64| sigma * sigma * (2.0 * length / PI) / (1.0 + (length * k).powi(2));
        let mut waves = Vec::with_capacity(3 * WAVES);
        for comp in 0..3 {
            for i in 0..WAVES {
                let k = (lo + (i as f64 + 0.5) * dln).exp();
                let dk = k * dln;
                let a = (2.0 * phi(k) * dk).sqrt();
                // Direção uniforme na esfera.
                let z = 2.0 * rng.uniform() - 1.0;
                let t = 2.0 * PI * rng.uniform();
                let s = (1.0 - z * z).sqrt();
                let dir = Vec3::new(s * t.cos(), s * t.sin(), z);
                let mut amp = Vec3::ZERO;
                match comp {
                    0 => amp.x = a,
                    1 => amp.y = a,
                    _ => amp.z = a,
                }
                waves.push(Wave {
                    k: dir * k,
                    phase: 2.0 * PI * rng.uniform(),
                    amp,
                });
            }
        }
        Self {
            sigma,
            length,
            seed,
            waves,
        }
    }

    /// Rajada (m/s) no ponto `p` (m, coordenadas fixas à Terra), componentes leste,
    /// norte e cima.
    #[must_use]
    pub fn at(&self, p: Vec3) -> Vec3 {
        self.waves.iter().fold(Vec3::ZERO, |acc, w| {
            acc + w.amp * (w.k.dot(p) + w.phase).cos()
        })
    }
}

/// Campo de vento: perfil médio interpolado mais turbulência opcional.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wind {
    /// Camadas em ordem crescente de altura.
    pub layers: Vec<WindLayer>,
    /// Turbulência.
    pub turbulence: Option<Turbulence>,
}

impl Wind {
    /// Sem vento.
    #[must_use]
    pub fn calm() -> Self {
        Self::default()
    }

    /// Vento constante em todas as alturas.
    #[must_use]
    pub fn constant(speed: f64, from: f64) -> Self {
        Self {
            layers: vec![WindLayer {
                agl: 0.0,
                speed,
                from,
            }],
            turbulence: None,
        }
    }

    /// Perfil de lei de potência (camada limite atmosférica):
    /// v(h) = v_ref·(h/h_ref)^α, com α ≈ 1/7 em terreno aberto, de 2 m até `top`.
    /// Fonte: Hellmann (1916); ESDU 82026 para os valores de α.
    #[must_use]
    pub fn power_law(v_ref: f64, h_ref: f64, alpha: f64, from: f64, top: f64) -> Self {
        let mut layers = Vec::new();
        let mut h = 2.0;
        while h < top {
            layers.push(WindLayer {
                agl: h,
                speed: v_ref * (h / h_ref).powf(alpha),
                from,
            });
            h *= 1.5;
        }
        layers.push(WindLayer {
            agl: top,
            speed: v_ref * (top / h_ref).powf(alpha),
            from,
        });
        Self {
            layers,
            turbulence: None,
        }
    }

    /// Velocidade do vento médio (leste, norte, cima), m/s, na altura `agl`. Interpola as
    /// componentes (não o ângulo) para não dar volta pelo lado errado; fora do perfil
    /// vale a camada da ponta.
    #[must_use]
    pub fn mean(&self, agl: f64) -> Vec3 {
        let comp = |l: &WindLayer| Vec3::new(-l.speed * l.from.sin(), -l.speed * l.from.cos(), 0.0);
        match self.layers.as_slice() {
            [] => Vec3::ZERO,
            [only] => comp(only),
            ls => {
                let i = ls.partition_point(|l| l.agl <= agl).clamp(1, ls.len() - 1);
                let (a, b) = (&ls[i - 1], &ls[i]);
                let w = ((agl - a.agl) / (b.agl - a.agl)).clamp(0.0, 1.0);
                comp(a) * (1.0 - w) + comp(b) * w
            }
        }
    }

    /// Vento total (leste, norte, cima), m/s, na altura `agl` e no ponto `p` fixo à Terra.
    #[must_use]
    pub fn at(&self, agl: f64, p: Vec3) -> Vec3 {
        let m = self.mean(agl);
        match &self.turbulence {
            Some(t) if agl > 0.0 => m + t.at(p),
            _ => m,
        }
    }

    /// Verdadeiro se não há vento algum.
    #[must_use]
    pub fn is_calm(&self) -> bool {
        self.turbulence.is_none() && self.layers.iter().all(|l| l.speed == 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meteorological_convention_and_interpolation() {
        // Vento de norte (from = 0) sopra para o sul.
        let w = Wind::constant(10.0, 0.0);
        assert!((w.mean(500.0) - Vec3::new(0.0, -10.0, 0.0)).norm() < 1e-12);
        // De leste (90°) sopra para oeste.
        let e = Wind::constant(5.0, core::f64::consts::FRAC_PI_2);
        assert!((e.mean(0.0) - Vec3::new(-5.0, 0.0, 0.0)).norm() < 1e-12);
        let p = Wind {
            layers: vec![
                WindLayer {
                    agl: 0.0,
                    speed: 2.0,
                    from: 0.0,
                },
                WindLayer {
                    agl: 1000.0,
                    speed: 12.0,
                    from: 0.0,
                },
            ],
            turbulence: None,
        };
        assert!((p.mean(500.0).y + 7.0).abs() < 1e-12);
        assert!((p.mean(5000.0).y + 12.0).abs() < 1e-12);
        let pl = Wind::power_law(5.0, 10.0, 1.0 / 7.0, 0.0, 300.0);
        assert!((pl.mean(10.0).norm() - 5.0).abs() < 0.05);
        assert!((pl.mean(300.0).norm() - 5.0 * 30.0_f64.powf(1.0 / 7.0)).abs() < 1e-9);
    }

    /// Estatística do campo: média nula e variância σ² por componente, medida ao longo de
    /// uma linha longa (ergodicidade espacial), e mesma semente = mesmo campo.
    #[test]
    fn turbulence_has_the_requested_variance() {
        let t = Turbulence::new(2.0, 300.0, 42);
        let n = 200_000;
        let (mut s, mut s2) = (Vec3::ZERO, Vec3::ZERO);
        for i in 0..n {
            let p = Vec3::new(1.37, 0.71, 0.53) * (i as f64 * 5.0);
            let g = t.at(p);
            s += g;
            s2 += Vec3::new(g.x * g.x, g.y * g.y, g.z * g.z);
        }
        let mean = s / n as f64;
        let var = s2 / n as f64;
        assert!(mean.norm() < 0.3, "média {mean:?}");
        for v in [var.x, var.y, var.z] {
            assert!((v / 4.0 - 1.0).abs() < 0.25, "variância {v}");
        }
        assert_eq!(
            Turbulence::new(2.0, 300.0, 42).at(Vec3::X * 123.0),
            t.at(Vec3::X * 123.0)
        );
        assert_ne!(
            Turbulence::new(2.0, 300.0, 43).at(Vec3::X * 123.0),
            t.at(Vec3::X * 123.0)
        );
    }
}
