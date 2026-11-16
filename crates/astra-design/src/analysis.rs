//! Análises em lote: Monte Carlo com elipse de dispersão, sensibilidade, otimização de um
//! parâmetro para o apogeu-alvo e calibração com telemetria.

use crate::material::MaterialDb;
use crate::model::{Design, ZoneKind};
use crate::sim::{simulate, SimOutput};
use serde::{Deserialize, Serialize};

/// Desvios-padrão das incertezas (relativos ou em graus).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Dispersion {
    /// Impulso do motor, relativo (NAR/TRA aceitam ±20 % de impulso; lotes reais variam ~3 %).
    pub thrust: f64,
    /// Coeficiente de arrasto, relativo.
    pub cd: f64,
    /// Massa das peças, relativa.
    pub mass: f64,
    /// Velocidade do vento, relativa.
    pub wind_speed: f64,
    /// Direção do vento, graus.
    pub wind_dir_deg: f64,
    /// Elevação do trilho, graus.
    pub elevation_deg: f64,
    /// Azimute do trilho, graus.
    pub azimuth_deg: f64,
}

impl Default for Dispersion {
    fn default() -> Self {
        Self {
            thrust: 0.03,
            cd: 0.05,
            mass: 0.02,
            wind_speed: 0.2,
            wind_dir_deg: 15.0,
            elevation_deg: 0.5,
            azimuth_deg: 2.0,
        }
    }
}

/// Uma rodada do Monte Carlo.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McRun {
    /// Índice (define a semente da rodada).
    pub index: u64,
    /// Fatores sorteados: empuxo, Cd, massa, vento (×), direção (°), elevação (°), azimute (°).
    pub params: [f64; 7],
    /// Apogeu acima do solo, m.
    pub apogee: f64,
    /// Mach máximo.
    pub max_mach: f64,
    /// Velocidade na saída do trilho, m/s.
    pub rail_exit: f64,
    /// Pouso do corpo principal: norte, leste (m), velocidade (m/s).
    pub landing: Option<[f64; 3]>,
    /// Pousos de todos os corpos: norte, leste (m).
    pub landings: Vec<[f64; 2]>,
    /// Alguma falha estrutural ou de recuperação.
    pub failure: bool,
}

/// SplitMix64 (Steele, Lea & Flood, 2014) com Box-Muller.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        #[allow(clippy::cast_precision_loss)]
        let u = (z >> 11) as f64 / (1u64 << 53) as f64;
        u
    }
    fn gauss(&mut self) -> f64 {
        let u = self.next().max(1e-300);
        (-2.0 * u.ln()).sqrt() * (core::f64::consts::TAU * self.next()).cos()
    }
}

/// Aplica um conjunto de perturbações ao projeto.
#[must_use]
pub fn perturb(d: &Design, p: &[f64; 7]) -> Design {
    let mut d = d.clone();
    d.scales.thrust *= p[0];
    d.scales.cd *= p[1];
    d.scales.mass *= p[2];
    for l in &mut d.launch.wind.layers {
        l.speed *= p[3].max(0.0);
        l.from_deg += p[4];
    }
    d.launch.rail_elevation_deg = (d.launch.rail_elevation_deg + p[5]).min(90.0);
    d.launch.rail_azimuth_deg += p[6];
    d
}

fn sample(disp: &Dispersion, seed: u64, index: u64) -> [f64; 7] {
    let mut r = Rng(seed ^ index.wrapping_mul(0xd1b5_4a32_d192_ed03));
    [
        1.0 + disp.thrust * r.gauss(),
        1.0 + disp.cd * r.gauss(),
        1.0 + disp.mass * r.gauss(),
        1.0 + disp.wind_speed * r.gauss(),
        disp.wind_dir_deg * r.gauss(),
        disp.elevation_deg * r.gauss(),
        disp.azimuth_deg * r.gauss(),
    ]
}

fn result(index: u64, params: [f64; 7], out: &SimOutput) -> McRun {
    let main = out.bodies.first();
    McRun {
        index,
        params,
        apogee: main.map_or(0.0, |b| b.samples.iter().map(|r| r[1]).fold(0.0, f64::max)),
        max_mach: main.map_or(0.0, |b| b.samples.iter().map(|r| r[4]).fold(0.0, f64::max)),
        rail_exit: out
            .events
            .iter()
            .find(|e| e.kind == "railExit")
            .map_or(0.0, |e| e.speed),
        landing: main.and_then(|b| b.landing).map(|l| [l[2], l[3], l[1]]),
        landings: out
            .bodies
            .iter()
            .filter_map(|b| b.landing.map(|l| [l[2], l[3]]))
            .collect(),
        failure: out.events.iter().any(|e| {
            matches!(
                e.kind.as_str(),
                "jointBroken" | "finLost" | "buckled" | "cordBroken" | "ejectionFailed" | "noExit"
            )
        }),
    }
}

/// Roda as rodadas `start..start+count` (cada uma com semente derivada do índice, então
/// pedaços diferentes podem rodar em paralelo e o resultado é o mesmo).
#[must_use]
pub fn monte_carlo(
    design: &Design,
    db: &MaterialDb,
    disp: &Dispersion,
    seed: u64,
    start: u64,
    count: u64,
) -> Vec<McRun> {
    (start..start + count)
        .map(|i| {
            let p = sample(disp, seed, i);
            result(i, p, &simulate(&perturb(design, &p), db))
        })
        .collect()
}

/// Elipse de dispersão.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ellipse {
    /// Centro (norte, leste), m.
    pub center: [f64; 2],
    /// Semieixos maior e menor de 1σ, m.
    pub axes_1sigma: [f64; 2],
    /// Semieixos que contêm 95 % (√5,991·σ, qui-quadrado com 2 graus de liberdade).
    pub axes_95: [f64; 2],
    /// Azimute do eixo maior a partir do norte, graus.
    pub azimuth_deg: f64,
}

/// Resumo estatístico.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McSummary {
    /// Rodadas.
    pub n: usize,
    /// Apogeu: média, desvio, P5, P50, P95.
    pub apogee: [f64; 5],
    /// Histograma do apogeu: (início da faixa, contagem).
    pub histogram: Vec<[f64; 2]>,
    /// Elipse dos pousos do corpo principal.
    pub ellipse: Ellipse,
    /// Fração com falha.
    pub failure_rate: f64,
    /// Fração de pousos (do corpo principal) dentro de zonas de pouso, se houver.
    pub p_landing_zone: Option<f64>,
    /// Fração de rodadas em que algum corpo cai em zona proibida.
    pub p_keep_out: Option<f64>,
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

/// Estatísticas das rodadas, elipse 2D dos pousos (autovalores da covariância) e
/// probabilidades das zonas.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn summarize(design: &Design, runs: &[McRun]) -> McSummary {
    let n = runs.len();
    if n == 0 {
        return McSummary::default();
    }
    let mut ap: Vec<f64> = runs.iter().map(|r| r.apogee).collect();
    ap.sort_by(f64::total_cmp);
    let mean = ap.iter().sum::<f64>() / n as f64;
    let sd = (ap.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / (n.max(2) - 1) as f64).sqrt();
    let (lo, hi) = (ap[0], ap[n - 1]);
    let bins = 12usize;
    let w = ((hi - lo) / bins as f64).max(1e-9);
    let mut hist = vec![[0.0, 0.0]; bins];
    for (k, h) in hist.iter_mut().enumerate() {
        h[0] = lo + w * k as f64;
    }
    for a in &ap {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let k = (((a - lo) / w) as usize).min(bins - 1);
        hist[k][1] += 1.0;
    }
    let pts: Vec<[f64; 2]> = runs
        .iter()
        .filter_map(|r| r.landing.map(|l| [l[0], l[1]]))
        .collect();
    let mut ellipse = Ellipse::default();
    if pts.len() >= 3 {
        let m = pts.len() as f64;
        let (cn, ce) = (
            pts.iter().map(|p| p[0]).sum::<f64>() / m,
            pts.iter().map(|p| p[1]).sum::<f64>() / m,
        );
        let (mut snn, mut see, mut sne) = (0.0, 0.0, 0.0);
        for p in &pts {
            snn += (p[0] - cn).powi(2);
            see += (p[1] - ce).powi(2);
            sne += (p[0] - cn) * (p[1] - ce);
        }
        let k = 1.0 / (m - 1.0);
        let (a, b, c) = (snn * k, see * k, sne * k);
        // Autovalores de [[a, c], [c, b]].
        let tr = 0.5 * (a + b);
        let det = ((0.5 * (a - b)).powi(2) + c * c).sqrt();
        let (l1, l2) = (tr + det, (tr - det).max(0.0));
        // Direção do autovetor maior no plano (norte, leste).
        let ang = 0.5 * (2.0 * c).atan2(a - b);
        let s = 5.991_f64.sqrt();
        ellipse = Ellipse {
            center: [cn, ce],
            axes_1sigma: [l1.sqrt(), l2.sqrt()],
            axes_95: [s * l1.sqrt(), s * l2.sqrt()],
            azimuth_deg: ang.to_degrees().rem_euclid(180.0),
        };
    }
    // Zonas: centro relativo ao lançador no mesmo plano tangente do mapa de recuperação.
    let re = 6_378_137.0;
    let lat0 = design.launch.lat_deg.to_radians();
    let zone_ne = |z: &crate::model::Zone| {
        [
            (z.lat_deg - design.launch.lat_deg).to_radians() * re,
            (z.lon_deg - design.launch.lon_deg).to_radians() * re * lat0.cos(),
        ]
    };
    let inside = |p: [f64; 2], kind: ZoneKind| {
        design.zones.iter().filter(|z| z.kind == kind).any(|z| {
            let c = zone_ne(z);
            (p[0] - c[0]).hypot(p[1] - c[1]) <= z.radius
        })
    };
    let has = |k: ZoneKind| design.zones.iter().any(|z| z.kind == k);
    McSummary {
        n,
        apogee: [mean, sd, pct(&ap, 0.05), pct(&ap, 0.5), pct(&ap, 0.95)],
        histogram: hist,
        ellipse,
        failure_rate: runs.iter().filter(|r| r.failure).count() as f64 / n as f64,
        p_landing_zone: has(ZoneKind::Landing).then(|| {
            pts.iter()
                .filter(|p| inside(**p, ZoneKind::Landing))
                .count() as f64
                / pts.len().max(1) as f64
        }),
        p_keep_out: has(ZoneKind::KeepOut).then(|| {
            runs.iter()
                .filter(|r| r.landings.iter().any(|p| inside(*p, ZoneKind::KeepOut)))
                .count() as f64
                / n as f64
        }),
    }
}

/// Efeito de cada incerteza (±1σ, uma de cada vez) no apogeu e na distância de pouso.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sensitivity {
    /// Parâmetro.
    pub name: String,
    /// Variação do apogeu de −1σ a +1σ, m.
    pub apogee: f64,
    /// Variação da distância de pouso de −1σ a +1σ, m.
    pub drift: f64,
}

/// Sensibilidade um-de-cada-vez.
#[must_use]
pub fn sensitivity(design: &Design, db: &MaterialDb, disp: &Dispersion) -> Vec<Sensitivity> {
    let names = [
        "Impulso do motor",
        "Arrasto (Cd)",
        "Massa",
        "Velocidade do vento",
        "Direção do vento",
        "Elevação do trilho",
        "Azimute do trilho",
    ];
    let sig = [
        disp.thrust,
        disp.cd,
        disp.mass,
        disp.wind_speed,
        disp.wind_dir_deg,
        disp.elevation_deg,
        disp.azimuth_deg,
    ];
    let base = [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
    let eval = |p: &[f64; 7]| {
        let r = result(0, *p, &simulate(&perturb(design, p), db));
        (r.apogee, r.landing.map_or(0.0, |l| l[0].hypot(l[1])))
    };
    let mut v: Vec<Sensitivity> = (0..7)
        .map(|k| {
            let (mut lo, mut hi) = (base, base);
            lo[k] -= sig[k];
            hi[k] += sig[k];
            let (a, b) = (eval(&lo), eval(&hi));
            Sensitivity {
                name: names[k].into(),
                apogee: b.0 - a.0,
                drift: b.1 - a.1,
            }
        })
        .collect();
    v.sort_by(|a, b| b.apogee.abs().total_cmp(&a.apogee.abs()));
    v
}

/// Parâmetro numérico de uma peça (campo de `PartKind` ou "massOverride").
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Knob {
    /// Peça.
    pub part: String,
    /// Campo (ex.: "mass" de uma massa, "span" de aletas, "length" de tubo).
    pub field: String,
    /// Mínimo.
    pub min: f64,
    /// Máximo.
    pub max: f64,
}

/// Aplica um valor a um parâmetro.
///
/// # Errors
/// Peça ou campo inexistente.
pub fn set_knob(d: &Design, k: &Knob, value: f64) -> Result<Design, String> {
    let mut d = d.clone();
    let p = d
        .parts
        .iter_mut()
        .find(|p| p.id == k.part)
        .ok_or_else(|| format!("Peça \"{}\" não existe.", k.part))?;
    if k.field == "massOverride" {
        p.mass_override = Some(value);
        return Ok(d);
    }
    let mut v = serde_json::to_value(&p.kind).map_err(|e| e.to_string())?;
    let slot = v
        .get_mut(&k.field)
        .filter(|x| x.is_number())
        .ok_or_else(|| {
            format!(
                "A peça \"{}\" não tem o campo numérico \"{}\".",
                k.part, k.field
            )
        })?;
    *slot = serde_json::json!(value);
    p.kind = serde_json::from_value(v).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Resultado da otimização.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Optimum {
    /// Valor escolhido.
    pub value: f64,
    /// Apogeu obtido, m.
    pub apogee: f64,
    /// Pontos avaliados: (valor, apogeu).
    pub evaluations: Vec<[f64; 2]>,
    /// Atingiu o alvo dentro de 0,5 %.
    pub reached: bool,
    /// Projeto com o valor aplicado.
    pub design: Design,
}

fn apogee_of(d: &Design, db: &MaterialDb) -> f64 {
    simulate(d, db)
        .bodies
        .first()
        .map_or(0.0, |b| b.samples.iter().map(|r| r[1]).fold(0.0, f64::max))
}

/// Ajusta um parâmetro para o apogeu-alvo: varredura de 7 pontos para achar a mudança de
/// lado e depois bisseção (o apogeu pode não ser monotônico no intervalo todo).
///
/// # Errors
/// Parâmetro inválido.
pub fn optimize(
    design: &Design,
    db: &MaterialDb,
    knob: &Knob,
    target: f64,
) -> Result<Optimum, String> {
    let mut evals = Vec::new();
    let mut f = |x: f64| -> Result<f64, String> {
        let a = apogee_of(&set_knob(design, knob, x)?, db);
        evals.push([x, a]);
        Ok(a - target)
    };
    let xs: Vec<f64> = (0..7)
        .map(|i| knob.min + (knob.max - knob.min) * f64::from(i) / 6.0)
        .collect();
    let mut ys = Vec::new();
    for &x in &xs {
        ys.push(f(x)?);
    }
    let bracket = (0..6).find(|&i| ys[i].signum() != ys[i + 1].signum());
    let best = if let Some(i) = bracket {
        let (mut lo, mut hi, mut flo) = (xs[i], xs[i + 1], ys[i]);
        for _ in 0..25 {
            let mid = 0.5 * (lo + hi);
            let fm = f(mid)?;
            if fm.signum() == flo.signum() {
                lo = mid;
                flo = fm;
            } else {
                hi = mid;
            }
            if (hi - lo).abs() < 1e-6 * (knob.max - knob.min).abs() {
                break;
            }
        }
        0.5 * (lo + hi)
    } else {
        // Sem mudança de lado: o mais perto do alvo.
        xs[(0..7)
            .min_by(|&a, &b| ys[a].abs().total_cmp(&ys[b].abs()))
            .unwrap_or(0)]
    };
    let d = set_knob(design, knob, best)?;
    let apogee = apogee_of(&d, db);
    evals.push([best, apogee]);
    Ok(Optimum {
        value: best,
        apogee,
        reached: (apogee - target).abs() <= 0.005 * target,
        evaluations: evals,
        design: d,
    })
}

/// Resultado da calibração.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    /// Multiplicador de Cd ajustado.
    pub cd: f64,
    /// Multiplicador de empuxo ajustado.
    pub thrust: f64,
    /// Erro RMS antes, m.
    pub rms_before: f64,
    /// Erro RMS depois, m.
    pub rms_after: f64,
    /// Apogeu medido, simulado antes e depois, m.
    pub apogee: [f64; 3],
    /// Curva simulada depois do ajuste: (t, altura).
    pub fitted: Vec<[f64; 2]>,
}

fn rms(d: &Design, db: &MaterialDb, data: &[[f64; 2]]) -> (f64, Vec<[f64; 2]>) {
    let out = simulate(d, db);
    let Some(b) = out.bodies.first() else {
        return (f64::INFINITY, vec![]);
    };
    let s: Vec<[f64; 2]> = b.samples.iter().map(|r| [r[0], r[1]]).collect();
    let at = |t: f64| {
        let i = s.partition_point(|p| p[0] < t);
        if i == 0 {
            return s.first().map_or(0.0, |p| p[1]);
        }
        if i >= s.len() {
            return s.last().map_or(0.0, |p| p[1]);
        }
        let (a, c) = (s[i - 1], s[i]);
        a[1] + (c[1] - a[1]) * (t - a[0]) / (c[0] - a[0]).max(1e-12)
    };
    #[allow(clippy::cast_precision_loss)]
    let e = (data.iter().map(|p| (at(p[0]) - p[1]).powi(2)).sum::<f64>()
        / data.len().max(1) as f64)
        .sqrt();
    (e, s)
}

/// Ajusta os multiplicadores de Cd (e, se pedido, de empuxo) para minimizar o erro RMS entre a
/// altura simulada e a medida (pares t, altura acima do solo), pelo método de Nelder-Mead
/// (Nelder & Mead, Computer Journal 7, 1965), com coeficientes padrão 1, 2, 0,5, 0,5.
#[must_use]
pub fn calibrate(
    design: &Design,
    db: &MaterialDb,
    data: &[[f64; 2]],
    fit_thrust: bool,
) -> Calibration {
    let cost = |x: &[f64; 2]| {
        let mut d = design.clone();
        d.scales.cd = design.scales.cd * x[0].clamp(0.2, 5.0);
        d.scales.thrust = design.scales.thrust
            * if fit_thrust {
                x[1].clamp(0.5, 1.5)
            } else {
                1.0
            };
        rms(&d, db, data).0
    };
    let mut simplex = [
        [1.0, 1.0],
        [1.15, 1.0],
        [1.0, if fit_thrust { 1.08 } else { 1.0 }],
    ];
    if !fit_thrust {
        simplex[2] = [0.85, 1.0];
    }
    let mut fs: Vec<f64> = simplex.iter().map(&cost).collect();
    for _ in 0..60 {
        let mut idx = [0usize, 1, 2];
        idx.sort_by(|&a, &b| fs[a].total_cmp(&fs[b]));
        let (b, m, w) = (idx[0], idx[1], idx[2]);
        if (fs[w] - fs[b]).abs() < 1e-3 {
            break;
        }
        let c = [
            (simplex[b][0] + simplex[m][0]) / 2.0,
            (simplex[b][1] + simplex[m][1]) / 2.0,
        ];
        let pt = |k: f64| {
            [
                c[0] + k * (simplex[w][0] - c[0]),
                c[1] + k * (simplex[w][1] - c[1]),
            ]
        };
        let r = pt(-1.0);
        let fr = cost(&r);
        if fr < fs[b] {
            let e = pt(-2.0);
            let fe = cost(&e);
            if fe < fr {
                simplex[w] = e;
                fs[w] = fe;
            } else {
                simplex[w] = r;
                fs[w] = fr;
            }
        } else if fr < fs[m] {
            simplex[w] = r;
            fs[w] = fr;
        } else {
            let k = pt(0.5);
            let fk = cost(&k);
            if fk < fs[w] {
                simplex[w] = k;
                fs[w] = fk;
            } else {
                for i in [m, w] {
                    simplex[i] = [
                        (simplex[i][0] + simplex[b][0]) / 2.0,
                        (simplex[i][1] + simplex[b][1]) / 2.0,
                    ];
                    fs[i] = cost(&simplex[i]);
                }
            }
        }
        if !fit_thrust {
            for s in &mut simplex {
                s[1] = 1.0;
            }
        }
    }
    let bi = (0..3).min_by(|&a, &b| fs[a].total_cmp(&fs[b])).unwrap_or(0);
    let best = simplex[bi];
    let mut fitted = design.clone();
    fitted.scales.cd = design.scales.cd * best[0].clamp(0.2, 5.0);
    fitted.scales.thrust = design.scales.thrust
        * if fit_thrust {
            best[1].clamp(0.5, 1.5)
        } else {
            1.0
        };
    let (before, s0) = rms(design, db, data);
    let (after, s1) = rms(&fitted, db, data);
    let peak = |s: &[[f64; 2]]| s.iter().map(|p| p[1]).fold(0.0, f64::max);
    Calibration {
        cd: fitted.scales.cd,
        thrust: fitted.scales.thrust,
        rms_before: before,
        rms_after: after,
        apogee: [
            data.iter().map(|p| p[1]).fold(0.0, f64::max),
            peak(&s0),
            peak(&s1),
        ],
        fitted: s1,
    }
}
