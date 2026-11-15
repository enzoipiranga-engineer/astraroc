//! Integradores de EDO para sistemas de dimensão fixa `N`.

/// Um passo do Runge-Kutta clássico de 4ª ordem com passo `h`.
/// Fonte: Hairer, Nørsett & Wanner, "Solving Ordinary Differential Equations I", §II.1.
pub fn rk4_step<const N: usize>(
    f: &impl Fn(f64, &[f64; N]) -> [f64; N],
    t: f64,
    y: &[f64; N],
    h: f64,
) -> [f64; N] {
    let k1 = f(t, y);
    let k2 = f(t + 0.5 * h, &lin(y, h, &[(0.5, &k1)]));
    let k3 = f(t + 0.5 * h, &lin(y, h, &[(0.5, &k2)]));
    let k4 = f(t + h, &lin(y, h, &[(1.0, &k3)]));
    lin(
        y,
        h,
        &[
            (1.0 / 6.0, &k1),
            (1.0 / 3.0, &k2),
            (1.0 / 3.0, &k3),
            (1.0 / 6.0, &k4),
        ],
    )
}

/// Integra com RK4 de passo fixo de `t0` a `t_end` em `n` passos iguais.
/// Não detecta eventos: para isso use [`integrate`].
pub fn rk4_fixed<const N: usize>(
    f: &impl Fn(f64, &[f64; N]) -> [f64; N],
    t0: f64,
    y0: [f64; N],
    t_end: f64,
    n: usize,
) -> [f64; N] {
    let h = (t_end - t0) / n as f64;
    (0..n).fold(y0, |y, i| rk4_step(f, t0 + i as f64 * h, &y, h))
}

/// y + h·Σ cᵢ·kᵢ
fn lin<const N: usize>(y: &[f64; N], h: f64, terms: &[(f64, &[f64; N])]) -> [f64; N] {
    let mut out = *y;
    for (c, k) in terms {
        for i in 0..N {
            out[i] += h * c * k[i];
        }
    }
    out
}

/// Um passo aceito do Dormand-Prince 5(4), com os coeficientes da saída densa.
#[derive(Clone, Debug)]
pub struct Step<const N: usize> {
    /// Início do passo.
    pub t0: f64,
    /// Fim do passo.
    pub t1: f64,
    /// Estado no início.
    pub y0: [f64; N],
    /// Estado no fim (solução de 5ª ordem).
    pub y1: [f64; N],
    /// Derivada no fim (reaproveitada como primeira etapa do próximo passo, FSAL).
    pub f1: [f64; N],
    rcont: [[f64; N]; 4],
}

impl<const N: usize> Step<N> {
    /// Estado interpolado em `t ∈ [t0, t1]` (interpolante contínuo de 4ª ordem).
    /// Fonte: Hairer, Nørsett & Wanner, §II.6 (rotina CONTD5 do DOPRI5).
    #[must_use]
    pub fn dense(&self, t: f64) -> [f64; N] {
        let th = (t - self.t0) / (self.t1 - self.t0);
        let th1 = 1.0 - th;
        let [r2, r3, r4, r5] = &self.rcont;
        core::array::from_fn(|i| {
            self.y0[i] + th * (r2[i] + th1 * (r3[i] + th * (r4[i] + th1 * r5[i])))
        })
    }
}

/// Tenta um passo Dormand-Prince de `t` a `t + h`. Devolve o passo e a norma do erro
/// estimado, escalada pelas tolerâncias (aceitar se ≤ 1).
/// Coeficientes: Dormand & Prince, "A family of embedded Runge-Kutta formulae",
/// J. Comp. Appl. Math. 6 (1980); erro e controle conforme Hairer et al., §II.4.
fn try_step<const N: usize>(
    f: &impl Fn(f64, &[f64; N]) -> [f64; N],
    t: f64,
    y: &[f64; N],
    k1: &[f64; N],
    h: f64,
    rtol: f64,
    atol: f64,
) -> (Step<N>, f64) {
    let k2 = f(t + h / 5.0, &lin(y, h, &[(1.0 / 5.0, k1)]));
    let k3 = f(
        t + 3.0 * h / 10.0,
        &lin(y, h, &[(3.0 / 40.0, k1), (9.0 / 40.0, &k2)]),
    );
    let k4 = f(
        t + 4.0 * h / 5.0,
        &lin(
            y,
            h,
            &[(44.0 / 45.0, k1), (-56.0 / 15.0, &k2), (32.0 / 9.0, &k3)],
        ),
    );
    let k5 = f(
        t + 8.0 * h / 9.0,
        &lin(
            y,
            h,
            &[
                (19372.0 / 6561.0, k1),
                (-25360.0 / 2187.0, &k2),
                (64448.0 / 6561.0, &k3),
                (-212.0 / 729.0, &k4),
            ],
        ),
    );
    let k6 = f(
        t + h,
        &lin(
            y,
            h,
            &[
                (9017.0 / 3168.0, k1),
                (-355.0 / 33.0, &k2),
                (46732.0 / 5247.0, &k3),
                (49.0 / 176.0, &k4),
                (-5103.0 / 18656.0, &k5),
            ],
        ),
    );
    let y1 = lin(
        y,
        h,
        &[
            (35.0 / 384.0, k1),
            (500.0 / 1113.0, &k3),
            (125.0 / 192.0, &k4),
            (-2187.0 / 6784.0, &k5),
            (11.0 / 84.0, &k6),
        ],
    );
    let k7 = f(t + h, &y1);

    const E: [f64; 7] = [
        71.0 / 57600.0,
        0.0,
        -71.0 / 16695.0,
        71.0 / 1920.0,
        -17253.0 / 339200.0,
        22.0 / 525.0,
        -1.0 / 40.0,
    ];
    const D: [f64; 7] = [
        -12715105075.0 / 11282082432.0,
        0.0,
        87487479700.0 / 32700410799.0,
        -10690763975.0 / 1880347072.0,
        701980252875.0 / 199316789632.0,
        -1453857185.0 / 822651844.0,
        69997945.0 / 29380423.0,
    ];
    let ks = [k1, &k2, &k3, &k4, &k5, &k6, &k7];
    let mut err2 = 0.0;
    let mut rcont = [[0.0; N]; 4];
    for i in 0..N {
        let e: f64 = h * (0..7).map(|s| E[s] * ks[s][i]).sum::<f64>();
        let scale = atol + rtol * y[i].abs().max(y1[i].abs());
        err2 += (e / scale) * (e / scale);
        let dy = y1[i] - y[i];
        let bspl = h * k1[i] - dy;
        rcont[0][i] = dy;
        rcont[1][i] = bspl;
        rcont[2][i] = dy - h * k7[i] - bspl;
        rcont[3][i] = h * (0..7).map(|s| D[s] * ks[s][i]).sum::<f64>();
    }
    (
        Step {
            t0: t,
            t1: t + h,
            y0: *y,
            y1,
            f1: k7,
            rcont,
        },
        (err2 / N as f64).sqrt(),
    )
}

/// Sentido do cruzamento por zero que dispara um evento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// g passa de negativo para positivo.
    Rising,
    /// g passa de positivo para negativo.
    Falling,
}

/// Evento: instante em que `g(t, y)` cruza zero no sentido dado.
pub struct EventSpec<'a, const N: usize> {
    /// Função do evento.
    pub g: &'a dyn Fn(f64, &[f64; N]) -> f64,
    /// Sentido do cruzamento.
    pub direction: Direction,
    /// Se verdadeiro, a integração para no evento.
    pub terminal: bool,
}

/// Ocorrência de um evento.
#[derive(Clone, Debug)]
pub struct EventHit<const N: usize> {
    /// Índice do evento na lista passada a [`integrate`].
    pub index: usize,
    /// Instante do cruzamento.
    pub t: f64,
    /// Estado no cruzamento.
    pub y: [f64; N],
}

/// Opções do integrador adaptativo.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Tolerância relativa.
    pub rtol: f64,
    /// Tolerância absoluta (mesma unidade de cada componente do estado).
    pub atol: f64,
    /// Passo máximo, s. Limita também a janela em que dois cruzamentos do mesmo
    /// evento podem passar despercebidos.
    pub h_max: f64,
    /// Número máximo de passos aceitos.
    pub max_steps: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rtol: 1e-9,
            atol: 1e-9,
            h_max: f64::INFINITY,
            max_steps: 2_000_000,
        }
    }
}

/// Falhas do integrador.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OdeError {
    /// O passo ficou menor que a resolução de `t` (problema rígido ou descontínuo).
    StepTooSmall {
        /// Instante em que o passo colapsou.
        t: f64,
    },
    /// Estouro de `max_steps`.
    TooManySteps {
        /// Instante alcançado.
        t: f64,
    },
    /// O estado deixou de ser finito.
    NonFinite {
        /// Instante em que apareceu NaN ou infinito.
        t: f64,
    },
}

impl core::fmt::Display for OdeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StepTooSmall { t } => write!(f, "passo de integração colapsou em t = {t} s"),
            Self::TooManySteps { t } => write!(f, "limite de passos atingido em t = {t} s"),
            Self::NonFinite { t } => write!(f, "estado não finito em t = {t} s"),
        }
    }
}
impl std::error::Error for OdeError {}

/// Resultado de [`integrate`].
#[derive(Clone, Debug)]
pub struct Outcome<const N: usize> {
    /// Instante final (fim do intervalo ou do evento terminal).
    pub t: f64,
    /// Estado final.
    pub y: [f64; N],
    /// Eventos na ordem em que ocorreram.
    pub events: Vec<EventHit<N>>,
    /// Índice do evento terminal que parou a integração, se houve.
    pub terminated_by: Option<usize>,
    /// Passos aceitos.
    pub steps: usize,
    /// Passos rejeitados.
    pub rejected: usize,
}

/// Passo inicial automático (Hairer et al., §II.4, rotina HINIT simplificada).
fn initial_step<const N: usize>(
    f: &impl Fn(f64, &[f64; N]) -> [f64; N],
    t: f64,
    y: &[f64; N],
    f0: &[f64; N],
    o: &Options,
) -> f64 {
    let norm = |v: &[f64; N], y: &[f64; N]| {
        (v.iter()
            .zip(y)
            .map(|(v, y)| (v / (o.atol + o.rtol * y.abs())).powi(2))
            .sum::<f64>()
            / N as f64)
            .sqrt()
    };
    let (d0, d1) = (norm(y, y), norm(f0, y));
    let h0 = if d0 < 1e-5 || d1 < 1e-5 {
        1e-6
    } else {
        0.01 * d0 / d1
    };
    let y1 = lin(y, h0, &[(1.0, f0)]);
    let f1 = f(t + h0, &y1);
    let diff: [f64; N] = core::array::from_fn(|i| f1[i] - f0[i]);
    let d2 = norm(&diff, y) / h0;
    let h1 = if d1.max(d2) <= 1e-15 {
        (h0 * 1e-3).max(1e-6)
    } else {
        (0.01 / d1.max(d2)).powf(0.2)
    };
    (100.0 * h0).min(h1)
}

/// Integra `y' = f(t, y)` de `t0` a `t_end` com Dormand-Prince 5(4) adaptativo.
///
/// Eventos são localizados por bisseção sobre a saída densa (resolução de ~2⁻⁵⁰ do
/// passo). `on_step` recebe cada passo aceito e o instante até onde ele vale (menor
/// que `t1` quando um evento terminal ocorre dentro do passo).
///
/// # Errors
/// Ver [`OdeError`].
pub fn integrate<const N: usize>(
    f: &impl Fn(f64, &[f64; N]) -> [f64; N],
    t0: f64,
    y0: [f64; N],
    t_end: f64,
    events: &[EventSpec<'_, N>],
    opts: &Options,
    mut on_step: impl FnMut(&Step<N>, f64),
) -> Result<Outcome<N>, OdeError> {
    let (mut t, mut y) = (t0, y0);
    let mut out = Outcome {
        t,
        y,
        events: Vec::new(),
        terminated_by: None,
        steps: 0,
        rejected: 0,
    };
    if t >= t_end {
        return Ok(out);
    }
    let mut k1 = f(t, &y);
    let mut h = initial_step(f, t, &y, &k1, opts).min(opts.h_max);
    let mut g_prev: Vec<f64> = events.iter().map(|e| (e.g)(t, &y)).collect();

    while t < t_end {
        let h_try = h.min(t_end - t);
        if h_try <= 4.0 * f64::EPSILON * t.abs().max(1.0) {
            return Err(OdeError::StepTooSmall { t });
        }
        let (mut step, err) = try_step(f, t, &y, &k1, h_try, opts.rtol, opts.atol);
        if !err.is_finite() {
            if h_try < 1e-12 {
                return Err(OdeError::NonFinite { t });
            }
            h = 0.1 * h_try;
            out.rejected += 1;
            continue;
        }
        let factor = if err == 0.0 {
            5.0
        } else {
            (0.9 * err.powf(-0.2)).clamp(0.2, 5.0)
        };
        if err > 1.0 {
            h = h_try * factor.min(1.0);
            out.rejected += 1;
            continue;
        }
        if t + h_try >= t_end {
            step.t1 = t_end; // evita resíduo de arredondamento no último passo
        }

        let g_new: Vec<f64> = events.iter().map(|e| (e.g)(step.t1, &step.y1)).collect();
        let mut hits: Vec<EventHit<N>> = Vec::new();
        for (i, e) in events.iter().enumerate() {
            let crossed = match e.direction {
                Direction::Rising => g_prev[i] < 0.0 && g_new[i] >= 0.0,
                Direction::Falling => g_prev[i] > 0.0 && g_new[i] <= 0.0,
            };
            if crossed {
                let (mut a, mut b) = (step.t0, step.t1);
                for _ in 0..50 {
                    let m = 0.5 * (a + b);
                    let gm = (e.g)(m, &step.dense(m));
                    if (gm < 0.0) == (g_prev[i] < 0.0) && gm != 0.0 {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                hits.push(EventHit {
                    index: i,
                    t: b,
                    y: step.dense(b),
                });
            }
        }
        hits.sort_by(|a, b| a.t.total_cmp(&b.t));
        let mut t_valid = step.t1;
        for hit in hits {
            let terminal = events[hit.index].terminal;
            let (idx, th) = (hit.index, hit.t);
            out.events.push(hit);
            if terminal {
                out.terminated_by = Some(idx);
                t_valid = th;
                break;
            }
        }
        on_step(&step, t_valid);
        out.steps += 1;
        if out.terminated_by.is_some() {
            out.t = t_valid;
            out.y = step.dense(t_valid);
            return Ok(out);
        }
        (t, y, k1, g_prev) = (step.t1, step.y1, step.f1, g_new);
        h = (h_try * factor).min(opts.h_max);
        if out.steps >= opts.max_steps {
            return Err(OdeError::TooManySteps { t });
        }
    }
    out.t = t;
    out.y = y;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    fn oscillator(_: f64, y: &[f64; 2]) -> [f64; 2] {
        [y[1], -y[0]]
    }

    #[test]
    fn rk4_converges_at_fourth_order() {
        let err = |n| (rk4_fixed(&oscillator, 0.0, [1.0, 0.0], 2.0, n)[0] - 2.0_f64.cos()).abs();
        let order = (err(50) / err(100)).log2();
        assert!((order - 4.0).abs() < 0.1, "ordem observada {order}");
    }

    #[test]
    fn dopri5_error_tracks_tolerance() {
        for (tol, bound) in [(1e-6, 1e-4), (1e-9, 1e-7), (1e-12, 1e-10)] {
            let o = Options {
                rtol: tol,
                atol: tol,
                ..Options::default()
            };
            let r = integrate(&oscillator, 0.0, [1.0, 0.0], 20.0 * PI, &[], &o, |_, _| {}).unwrap();
            let e = ((r.y[0] - 1.0).powi(2) + r.y[1].powi(2)).sqrt();
            assert!(e < bound, "tol {tol}: erro {e}");
            assert_eq!(r.t, 20.0 * PI);
        }
    }

    #[test]
    fn dense_output_is_accurate_inside_steps() {
        let o = Options {
            rtol: 1e-10,
            atol: 1e-10,
            ..Options::default()
        };
        let mut worst: f64 = 0.0;
        integrate(&oscillator, 0.0, [1.0, 0.0], 10.0, &[], &o, |s, _| {
            for k in 1..10 {
                let t = s.t0 + (s.t1 - s.t0) * f64::from(k) / 10.0;
                worst = worst.max((s.dense(t)[0] - t.cos()).abs());
            }
        })
        .unwrap();
        assert!(worst < 1e-8, "erro da saída densa {worst}");
    }

    /// Queda livre de 100 m com g = 9,80665: impacto em t = √(2h/g), v = −g·t.
    #[test]
    fn terminal_event_is_located_precisely() {
        let g = 9.806_65;
        let f = |_: f64, y: &[f64; 2]| [y[1], -g];
        let ground = |_: f64, y: &[f64; 2]| y[0];
        let apex = |_: f64, y: &[f64; 2]| y[1];
        let ev = [
            EventSpec {
                g: &ground,
                direction: Direction::Falling,
                terminal: true,
            },
            EventSpec {
                g: &apex,
                direction: Direction::Falling,
                terminal: false,
            },
        ];
        // Lançado para cima a 20 m/s de 100 m de altura.
        let r = integrate(
            &f,
            0.0,
            [100.0, 20.0],
            60.0,
            &ev,
            &Options::default(),
            |_, _| {},
        )
        .unwrap();
        let t_imp = (20.0 + (400.0_f64 + 2.0 * g * 100.0).sqrt()) / g;
        assert_eq!(r.terminated_by, Some(0));
        assert!((r.t - t_imp).abs() < 1e-9, "impacto {} vs {t_imp}", r.t);
        assert!(r.y[0].abs() < 1e-8);
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.events[0].index, 1);
        assert!((r.events[0].t - 20.0 / g).abs() < 1e-9);
        assert!((r.events[0].y[0] - (100.0 + 200.0 / g)).abs() < 1e-8);
    }

    #[test]
    fn reports_blow_up_instead_of_looping() {
        // y' = y², y(0) = 1 explode em t = 1.
        let f = |_: f64, y: &[f64; 1]| [y[0] * y[0]];
        let r = integrate(&f, 0.0, [1.0], 2.0, &[], &Options::default(), |_, _| {});
        assert!(r.is_err());
    }
}
