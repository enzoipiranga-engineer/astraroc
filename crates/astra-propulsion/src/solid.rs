use crate::motor::{Motor, MotorPoint, P_SL};
use crate::nozzle::thrust_coefficient;
use core::f64::consts::PI;

/// Trecho da lei de queima de Saint-Robert (Vieille): r = a·p^n.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurnRateRange {
    /// Pressão mínima do trecho, Pa.
    pub p_min: f64,
    /// Coeficiente `a` em mm/s com a pressão em MPa (a unidade usual das tabelas).
    pub a_mm_s_mpa: f64,
    /// Expoente de pressão `n` (adimensional).
    pub n: f64,
}

/// Propelente sólido.
#[derive(Clone, Debug, PartialEq)]
pub struct Propellant {
    /// Nome.
    pub name: String,
    /// Massa específica do grão, kg/m³.
    pub density: f64,
    /// Velocidade característica c*, m/s.
    pub c_star: f64,
    /// Razão de calores específicos dos produtos na tubeira.
    pub gamma: f64,
    /// Lei de queima por trechos de pressão, em ordem crescente de `p_min`.
    pub burn_rate: Vec<BurnRateRange>,
}

impl Propellant {
    /// KNSB (65% nitrato de potássio, 35% sorbitol), moldado.
    ///
    /// Fonte: Richard Nakka, "KNSB Propellant" (nakka-rocketry.net): massa específica
    /// ideal 1841 kg/m³, c* 919 m/s (aqui com 95% de eficiência de combustão não
    /// aplicada), γ = 1,042 (escoamento bifásico) e lei de queima em cinco trechos.
    /// Confiabilidade: catálogo, transcrito sem acesso à página nesta sessão; conferir
    /// antes de projetar um motor real.
    #[must_use]
    pub fn knsb() -> Self {
        let r = |p_mpa: f64, a: f64, n: f64| BurnRateRange {
            p_min: p_mpa * 1e6,
            a_mm_s_mpa: a,
            n,
        };
        Self {
            name: "KNSB".to_owned(),
            density: 1841.0,
            c_star: 919.0,
            gamma: 1.042,
            burn_rate: vec![
                r(0.0, 10.708, 0.625),
                r(0.807, 8.763, -0.314),
                r(1.503, 7.852, -0.013),
                r(3.792, 3.907, 0.535),
                r(7.033, 9.653, 0.064),
            ],
        }
    }

    fn range(&self, p: f64) -> &BurnRateRange {
        self.burn_rate
            .iter()
            .rev()
            .find(|r| p >= r.p_min)
            .unwrap_or(&self.burn_rate[0])
    }

    /// Taxa de regressão na pressão `p` (Pa), m/s.
    #[must_use]
    pub fn burn_rate_at(&self, p: f64) -> f64 {
        let r = self.range(p);
        r.a_mm_s_mpa * 1e-3 * (p / 1e6).powf(r.n)
    }

    /// Pressão de câmara de equilíbrio para a razão Kn = A_queima/A_garganta:
    /// vazão gerada ρ·A_b·a·p^n = vazão pela tubeira p·A_t/c*, logo
    /// p = (Kn·ρ·a·c*)^(1/(1−n)).
    /// Com lei por trechos, vale o trecho cuja faixa contém a própria solução; se as
    /// descontinuidades da tabela deixarem mais de uma (ou nenhuma) solução
    /// consistente, fica a de pressão mais próxima da faixa.
    /// Fonte: Sutton & Biblarz, 9ª ed., cap. 12 (eq. da pressão de câmara em regime).
    #[must_use]
    pub fn chamber_pressure(&self, kn: f64) -> f64 {
        let mut best = (f64::INFINITY, 0.0);
        for (i, r) in self.burn_rate.iter().enumerate() {
            let p_mpa =
                (kn * self.density * self.c_star * r.a_mm_s_mpa * 1e-9).powf(1.0 / (1.0 - r.n));
            let p = p_mpa * 1e6;
            let p_max = self.burn_rate.get(i + 1).map_or(f64::INFINITY, |n| n.p_min);
            let miss = if p < r.p_min {
                r.p_min - p
            } else if p > p_max {
                p - p_max
            } else {
                0.0
            };
            if miss < best.0 {
                best = (miss, p);
            }
        }
        best.1
    }
}

/// Motor de grãos BATES: cilindros ocos, inibidos por fora, queimando pelo furo e pelas
/// duas faces.
#[derive(Clone, Debug, PartialEq)]
pub struct BatesMotor {
    /// Propelente.
    pub propellant: Propellant,
    /// Número de grãos.
    pub grains: u32,
    /// Diâmetro externo do grão, m.
    pub outer_diameter: f64,
    /// Diâmetro inicial do furo, m.
    pub core_diameter: f64,
    /// Comprimento inicial de cada grão, m.
    pub grain_length: f64,
    /// Diâmetro da garganta, m.
    pub throat_diameter: f64,
    /// Diâmetro de saída da tubeira, m.
    pub exit_diameter: f64,
    /// Eficiência global (combustão × tubeira) aplicada ao empuxo, 0 a 1.
    pub efficiency: f64,
    /// Massa do motor sem propelente, kg.
    pub dry_mass: f64,
    /// Diâmetro externo do motor, m.
    pub case_diameter: f64,
    /// Comprimento do motor, m.
    pub case_length: f64,
}

/// Resultado da balística interna.
#[derive(Clone, Debug)]
pub struct SolidBurn {
    /// Motor pronto para o voo (empuxo ao nível do mar padrão, com correção de altitude).
    pub motor: Motor,
    /// Pressão de câmara em cada ponto da tabela, Pa.
    pub chamber_pressure: Vec<f64>,
    /// Kn em cada ponto da tabela.
    pub kn: Vec<f64>,
}

impl BatesMotor {
    /// Área de queima após regressão `x` (m):
    /// A_b = N·[π·(d + 2x)·(L − 2x) + (π/2)·(D² − (d + 2x)²)].
    /// Fonte: Nakka, "Solid Rocket Motor Theory: Grain Design" (grão BATES).
    #[must_use]
    pub fn burn_area(&self, x: f64) -> f64 {
        let (d, l) = (self.core_diameter + 2.0 * x, self.grain_length - 2.0 * x);
        f64::from(self.grains) * (PI * d * l + 0.5 * PI * (self.outer_diameter.powi(2) - d * d))
    }

    /// Regressão total até o fim do propelente, m.
    #[must_use]
    pub fn web(&self) -> f64 {
        (0.5 * (self.outer_diameter - self.core_diameter)).min(0.5 * self.grain_length)
    }

    /// Massa inicial de propelente, kg.
    #[must_use]
    pub fn propellant_mass(&self) -> f64 {
        f64::from(self.grains)
            * 0.25
            * PI
            * (self.outer_diameter.powi(2) - self.core_diameter.powi(2))
            * self.grain_length
            * self.propellant.density
    }

    /// Simula a queima em regime quase permanente: a cada regressão x, a pressão de
    /// câmara é a de equilíbrio para o Kn instantâneo, o empuxo é F = η·C_F·p_c·A_t e a
    /// vazão é ṁ = p_c·A_t/c*. O tempo sai de dt = dx/r(p_c).
    ///
    /// Não modela: transitórios de ignição e de cauda, queima erosiva, variação de c*
    /// com a pressão, separação do escoamento na tubeira.
    /// Fonte: Sutton & Biblarz, 9ª ed., caps. 3 e 12.
    #[must_use]
    pub fn simulate(&self) -> SolidBurn {
        const STEPS: usize = 400;
        let at = 0.25 * PI * self.throat_diameter.powi(2);
        let ae = 0.25 * PI * self.exit_diameter.powi(2);
        let web = self.web();
        let state = |x: f64| {
            let kn = self.burn_area(x) / at;
            let pc = self.propellant.chamber_pressure(kn);
            let cf = thrust_coefficient(self.propellant.gamma, ae / at, pc, P_SL);
            (
                kn,
                pc,
                (self.efficiency * cf * pc * at).max(0.0),
                pc * at / self.propellant.c_star,
            )
        };
        let dx = web / STEPS as f64;
        let (mut t, mut burned) = (0.0, 0.0);
        let mut rows = Vec::with_capacity(STEPS + 2);
        let mut prev = state(0.0);
        rows.push((t, prev, burned));
        for i in 1..=STEPS {
            let x = i as f64 * dx;
            let cur = state(x.min(web * (1.0 - 1e-9)));
            let mid = state(x - 0.5 * dx);
            let dt = dx / self.propellant.burn_rate_at(mid.1);
            // Simpson no trecho para a massa expelida.
            burned += dt * (prev.3 + 4.0 * mid.3 + cur.3) / 6.0;
            t += dt;
            rows.push((t, cur, burned));
            prev = cur;
        }
        // A massa geométrica é a referência; a integral numérica é normalizada a ela.
        let mp = self.propellant_mass();
        let scale = mp / burned;
        let mut points: Vec<MotorPoint> = rows
            .iter()
            .map(|&(t, s, b)| MotorPoint {
                t,
                thrust: s.2,
                prop_mass: (mp - b * scale).max(0.0),
            })
            .collect();
        // Fim abrupto da queima: o empuxo cai a zero em 1% do tempo de queima.
        points.push(MotorPoint {
            t: t * 1.01,
            thrust: 0.0,
            prop_mass: 0.0,
        });
        SolidBurn {
            motor: Motor {
                name: format!(
                    "BATES {}x{:.0}mm {}",
                    self.grains,
                    self.outer_diameter * 1000.0,
                    self.propellant.name
                ),
                manufacturer: "ASTRA".to_owned(),
                diameter: self.case_diameter,
                length: self.case_length,
                dry_mass: self.dry_mass,
                points,
                ref_pressure: P_SL,
                exit_area: ae,
            },
            chamber_pressure: rows.iter().map(|r| r.1 .1).chain([0.0]).collect(),
            kn: rows.iter().map(|r| r.1 .0).chain([0.0]).collect(),
        }
    }
}
