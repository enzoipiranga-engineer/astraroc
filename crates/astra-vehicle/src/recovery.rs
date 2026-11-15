use astra_dynamics::flight::Trigger;

/// Estágio de rizadura (reefing): o velame abre só em parte por um tempo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reef {
    /// Fração da área de arrasto final durante a rizadura, 0 a 1.
    pub fraction: f64,
    /// Tempo, a partir do disparo, em que a rizadura é liberada, s.
    pub duration: f64,
}

/// Paraquedas.
#[derive(Clone, Debug, PartialEq)]
pub struct Parachute {
    /// Nome (drogue, principal...).
    pub name: String,
    /// Área de arrasto totalmente aberto, Cd·S, m².
    pub cd_s: f64,
    /// Quando abre.
    pub trigger: Trigger,
    /// Tempo de enchimento, s (ver [`Parachute::fill_time`]).
    pub inflation_time: f64,
    /// Rizadura opcional.
    pub reef: Option<Reef>,
}

impl Parachute {
    /// Tempo de enchimento de um velame de tecido sólido: t_f = n·D0/v, com n ≈ 8,
    /// D0 o diâmetro nominal e v a velocidade no esticamento das linhas.
    /// Fonte: Knacke, "Parachute Recovery Systems Design Manual" (1992), §5.4.
    #[must_use]
    pub fn fill_time(nominal_diameter: f64, speed: f64) -> f64 {
        8.0 * nominal_diameter / speed.max(1.0)
    }

    /// Área de arrasto (m²) `tau` segundos depois do disparo.
    ///
    /// Durante o enchimento a área cresce com (t/t_f)², forma usual para tecido sólido
    /// no método de Pflanz (Knacke 1992, §5.4). Com rizadura, o velame enche até a
    /// fração rizada, espera, e enche o restante com o mesmo tempo de enchimento.
    /// A força de abertura resulta da dinâmica (massa finita), sem fator de choque extra.
    #[must_use]
    pub fn drag_area(&self, tau: f64) -> f64 {
        if tau <= 0.0 {
            return 0.0;
        }
        let ramp = |t: f64| {
            if self.inflation_time > 0.0 {
                (t / self.inflation_time).clamp(0.0, 1.0).powi(2)
            } else {
                1.0
            }
        };
        match self.reef {
            None => self.cd_s * ramp(tau),
            Some(r) if tau < r.duration => self.cd_s * r.fraction * ramp(tau),
            Some(r) => self.cd_s * (r.fraction + (1.0 - r.fraction) * ramp(tau - r.duration)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflation_and_reefing_profile() {
        let mut p = Parachute {
            name: "principal".into(),
            cd_s: 4.0,
            trigger: Trigger::AfterApogee { delay: 0.0 },
            inflation_time: 0.5,
            reef: None,
        };
        assert_eq!(p.drag_area(-1.0), 0.0);
        assert!((p.drag_area(0.25) - 1.0).abs() < 1e-12);
        assert_eq!(p.drag_area(2.0), 4.0);
        p.reef = Some(Reef {
            fraction: 0.25,
            duration: 3.0,
        });
        assert_eq!(p.drag_area(2.0), 1.0);
        assert!((p.drag_area(3.25) - (1.0 + 3.0 * 0.25)).abs() < 1e-12);
        assert_eq!(p.drag_area(10.0), 4.0);
        assert!((Parachute::fill_time(1.0, 20.0) - 0.4).abs() < 1e-12);
    }
}
