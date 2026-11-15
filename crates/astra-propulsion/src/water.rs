use crate::motor::{Motor, MotorPoint, P_SL};
use core::f64::consts::PI;

/// Constante do ar seco, J/(kg·K).
const R_AIR: f64 = 287.052_87;
/// Razão de calores específicos do ar.
const GAMMA: f64 = 1.4;

/// Foguete de água: garrafa com água e ar comprimido.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterRocket {
    /// Volume interno da garrafa, m³.
    pub bottle_volume: f64,
    /// Volume de água, m³.
    pub water_volume: f64,
    /// Pressão manométrica inicial do ar, Pa.
    pub gauge_pressure: f64,
    /// Diâmetro do bocal, m (gargalo PET padrão: 21,5 mm).
    pub nozzle_diameter: f64,
    /// Diâmetro interno da garrafa, m.
    pub bottle_diameter: f64,
    /// Coeficiente de descarga do bocal (≈ 0,95 a 0,98 para gargalo liso).
    pub discharge_coeff: f64,
    /// Expoente politrópico da expansão do ar durante a saída da água
    /// (1,4 = adiabática, 1,0 = isotérmica).
    pub polytropic_n: f64,
    /// Temperatura inicial do ar, K.
    pub air_temperature: f64,
    /// Pressão ambiente, Pa.
    pub ambient_pressure: f64,
    /// Massa específica da água, kg/m³.
    pub water_density: f64,
    /// Pressão manométrica máxima considerada segura para a garrafa, Pa. Referência:
    /// garrafas PET de refrigerante costumam romper entre 10 e 12 bar; a prática de
    /// clubes limita a operação a cerca de 6 a 7 bar. Informe o limite do seu material.
    pub max_safe_gauge_pressure: f64,
}

/// Resultado da simulação do motor de água.
#[derive(Clone, Debug)]
pub struct WaterBurn {
    /// Motor pronto para o voo.
    pub motor: Motor,
    /// Duração da fase de água, s.
    pub water_phase_time: f64,
    /// Duração da fase de ar, s.
    pub air_phase_time: f64,
    /// Pressão absoluta do ar quando a água acaba, Pa.
    pub pressure_at_water_end: f64,
    /// Água que não saiu porque a pressão caiu à ambiente antes (kg).
    pub water_left: f64,
    /// Avisos em linguagem simples.
    pub warnings: Vec<String>,
}

impl WaterRocket {
    /// Garrafa PET de 2 L com 1/3 de água a 4 bar, valores típicos de sala de aula.
    #[must_use]
    pub fn pet_2l() -> Self {
        Self {
            bottle_volume: 2.0e-3,
            water_volume: 0.66e-3,
            gauge_pressure: 4.0e5,
            nozzle_diameter: 0.0215,
            bottle_diameter: 0.10,
            discharge_coeff: 0.97,
            polytropic_n: 1.4,
            air_temperature: 293.15,
            ambient_pressure: P_SL,
            water_density: 998.2,
            max_safe_gauge_pressure: 7.0e5,
        }
    }

    fn nozzle_area(&self) -> f64 {
        0.25 * PI * self.nozzle_diameter.powi(2)
    }

    /// Pressão absoluta do ar quando ele ocupa o volume `v`:
    /// p·V^n = constante (politrópica).
    fn air_pressure(&self, v: f64) -> f64 {
        let v0 = self.bottle_volume - self.water_volume;
        (self.gauge_pressure + self.ambient_pressure) * (v0 / v).powf(self.polytropic_n)
    }

    /// Velocidade de saída da água para a pressão absoluta `p` do ar, por Bernoulli com
    /// continuidade entre a garrafa e o bocal:
    /// v = √( 2·(p − p_a) / (ρ·(1 − (A/A_g)²)) ).
    /// Despreza a coluna de água e a aceleração do foguete (dezenas de kPa contra
    /// centenas) e as perdas, que entram só pelo coeficiente de descarga.
    /// Fonte: Finney, "Water rocket exhaust simplified", Am. J. Phys. 68 (2000);
    /// White, "Fluid Mechanics", 7ª ed., §3.5.
    fn water_speed(&self, p: f64) -> f64 {
        let beta = (self.nozzle_diameter / self.bottle_diameter).powi(4);
        (2.0 * (p - self.ambient_pressure).max(0.0) / (self.water_density * (1.0 - beta))).sqrt()
    }

    /// Vazão (kg/s) e empuxo (N) do ar por um bocal convergente, para ar no reservatório
    /// a `p` (Pa) e `t` (K).
    ///
    /// Bloqueado (p/p_a ≥ ((γ+1)/2)^(γ/(γ−1)) ≈ 1,893): saída sônica,
    /// ṁ = Cd·A·p·√(γ/(R·T))·(2/(γ+1))^((γ+1)/(2(γ−1))),
    /// F = ṁ·v_e + (p_e − p_a)·A, com p_e = p·(2/(γ+1))^(γ/(γ−1)).
    /// Não bloqueado: p_e = p_a e M_e² = (2/(γ−1))·[(p/p_a)^((γ−1)/γ) − 1].
    /// Fonte: Anderson, "Modern Compressible Flow", 3ª ed., cap. 5.
    fn air_flow(&self, p: f64, t: f64) -> (f64, f64) {
        let (g, a, pa) = (GAMMA, self.nozzle_area(), self.ambient_pressure);
        if p <= pa {
            return (0.0, 0.0);
        }
        let crit = ((g + 1.0) / 2.0).powf(g / (g - 1.0));
        if p / pa >= crit {
            let mdot = self.discharge_coeff
                * a
                * p
                * (g / (R_AIR * t)).sqrt()
                * (2.0 / (g + 1.0)).powf((g + 1.0) / (2.0 * (g - 1.0)));
            let ve = (g * R_AIR * t * 2.0 / (g + 1.0)).sqrt();
            (mdot, mdot * ve + (p / crit - pa) * a)
        } else {
            let m2 = 2.0 / (g - 1.0) * ((p / pa).powf((g - 1.0) / g) - 1.0);
            let te = t / (1.0 + 0.5 * (g - 1.0) * m2);
            let ve = (m2 * g * R_AIR * te).sqrt();
            let mdot = self.discharge_coeff * a * pa / (R_AIR * te) * ve;
            (mdot, mdot * ve)
        }
    }

    /// Simula as duas fases e devolve a tabela de empuxo e massa.
    ///
    /// Fase de água: o volume de ar cresce a dV/dt = Cd·A·v e o empuxo é
    /// F = ṁ·v = Cd·ρ·A·v². Fase de ar: esvaziamento isentrópico do reservatório,
    /// p ∝ m^γ, T ∝ m^(γ−1), até a pressão ambiente.
    /// O empuxo é calculado para a pressão ambiente informada e não é corrigido com a
    /// altitude (a queima dura décimos de segundo).
    #[must_use]
    pub fn simulate(&self) -> WaterBurn {
        let mut warnings = Vec::new();
        if self.gauge_pressure > self.max_safe_gauge_pressure {
            warnings.push(format!(
                "Pressão de {:.1} bar acima do limite de segurança de {:.1} bar: a garrafa pode romper. Reduza a pressão.",
                self.gauge_pressure / 1e5,
                self.max_safe_gauge_pressure / 1e5
            ));
        }
        let a = self.nozzle_area();
        let v0 = self.bottle_volume - self.water_volume;

        // Fase de água: quadratura em V, do volume inicial de ar até a garrafa cheia de ar
        // ou até a pressão igualar a ambiente.
        let p0 = self.gauge_pressure + self.ambient_pressure;
        let v_equal = v0 * (p0 / self.ambient_pressure).powf(1.0 / self.polytropic_n);
        let v_end = self.bottle_volume.min(v_equal * (1.0 - 1e-9));
        let water_left = (self.bottle_volume - v_end).max(0.0) * self.water_density;
        const NW: usize = 240;
        let mut pts: Vec<(f64, f64, f64)> = Vec::new(); // (t, empuxo, água restante em kg)
        let mut t = 0.0;
        if self.water_volume > 0.0 && v_end > v0 {
            let dv = (v_end - v0) / NW as f64;
            let rate = |v: f64| self.discharge_coeff * a * self.water_speed(self.air_pressure(v));
            let thrust = |v: f64| {
                self.discharge_coeff
                    * self.water_density
                    * a
                    * self.water_speed(self.air_pressure(v)).powi(2)
            };
            pts.push((
                0.0,
                thrust(v0),
                (self.bottle_volume - v0) * self.water_density,
            ));
            for i in 1..=NW {
                let (va, vb) = (v0 + (i - 1) as f64 * dv, v0 + i as f64 * dv);
                // Simpson em 1/(dV/dt); o último trecho pode ter dV/dt → 0 no fim.
                let inv = |v: f64| 1.0 / rate(v).max(1e-12);
                t += dv
                    * (inv(va) + 4.0 * inv(0.5 * (va + vb)) + inv(vb.min(v_end * (1.0 - 1e-12))))
                    / 6.0;
                pts.push((
                    t,
                    thrust(vb),
                    (self.bottle_volume - vb) * self.water_density,
                ));
            }
        }
        let water_phase_time = t;
        let p1 = self.air_pressure(v_end);

        // Fase de ar.
        let t1 = self.air_temperature * (v0 / v_end).powf(self.polytropic_n - 1.0);
        let m1 = p1 * self.bottle_volume / (R_AIR * t1);
        let m_final = m1 * (self.ambient_pressure / p1).powf(1.0 / GAMMA);
        let mut air: Vec<(f64, f64, f64)> = Vec::new(); // (t, empuxo, ar a expelir em kg)
        if water_left == 0.0 && p1 > self.ambient_pressure * 1.0001 {
            let state = |m: f64| (p1 * (m / m1).powf(GAMMA), t1 * (m / m1).powf(GAMMA - 1.0));
            let dm = |m: f64| {
                let (p, tt) = state(m);
                -self.air_flow(p, tt).0
            };
            // Constante de tempo do esvaziamento: massa / vazão inicial.
            let h = 0.002 * m1 / self.air_flow(p1, t1).0;
            let mut m = m1;
            let mut ta = 0.0;
            air.push((0.0, self.air_flow(p1, t1).1, m1 - m_final));
            for step in 0..20_000 {
                let k1 = dm(m);
                let k2 = dm(m + 0.5 * h * k1);
                let k3 = dm(m + 0.5 * h * k2);
                let k4 = dm(m + h * k3);
                m = (m + h * (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0).max(m_final);
                ta += h;
                let (p, tt) = state(m);
                if step % 5 == 4 || p <= self.ambient_pressure * 1.002 {
                    air.push((ta, self.air_flow(p, tt).1, m - m_final));
                }
                if p <= self.ambient_pressure * 1.002 {
                    break;
                }
            }
        } else if water_left > 0.0 {
            warnings.push(format!(
                "A pressão acaba antes da água: {:.0} g de água ficam na garrafa como peso morto. Use menos água ou mais pressão.",
                water_left * 1000.0
            ));
        }
        let air_phase_time = air.last().map_or(0.0, |p| p.0);
        let air_mass = air.first().map_or(0.0, |p| p.2);

        let mut points: Vec<MotorPoint> = pts
            .iter()
            .map(|&(t, f, w)| MotorPoint {
                t,
                thrust: f,
                prop_mass: w - water_left + air_mass,
            })
            .collect();
        // A troca de água por ar no bocal é um degrau de empuxo: ponto extra 1 µs depois.
        for (i, &(ta, f, m)) in air.iter().enumerate() {
            let shift = if i == 0 && !points.is_empty() {
                1e-6
            } else {
                0.0
            };
            points.push(MotorPoint {
                t: water_phase_time + ta + shift,
                thrust: f,
                prop_mass: m,
            });
        }
        let end = points.last().map_or(0.0, |p| p.t);
        if points.last().is_some_and(|p| p.thrust > 0.0) {
            points.push(MotorPoint {
                t: end + 1e-4,
                thrust: 0.0,
                prop_mass: 0.0,
            });
        }
        if let Some(last) = points.last_mut() {
            last.prop_mass = 0.0;
        }
        WaterBurn {
            motor: Motor {
                name: format!(
                    "Água {:.2} L @ {:.1} bar",
                    self.water_volume * 1000.0,
                    self.gauge_pressure / 1e5
                ),
                manufacturer: "ASTRA".to_owned(),
                diameter: self.bottle_diameter,
                length: self.bottle_volume / (0.25 * PI * self.bottle_diameter.powi(2)),
                dry_mass: water_left,
                points,
                ref_pressure: self.ambient_pressure,
                exit_area: 0.0,
            },
            water_phase_time,
            air_phase_time,
            pressure_at_water_end: p1,
            water_left,
            warnings,
        }
    }
}
