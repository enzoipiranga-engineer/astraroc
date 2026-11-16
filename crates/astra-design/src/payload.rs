//! Cargas úteis, seus requisitos e o ambiente que elas enfrentam no voo.

use crate::assembly::Assembly;
use crate::model::PartKind;
use crate::sim::SimOutput;
use serde::{Deserialize, Serialize};

/// Tipo de carga útil.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PayloadKind {
    /// CubeSat de `units` U (CubeSat Design Specification, Cal Poly, rev. 14).
    CubeSat {
        /// Unidades (1U = 100 × 100 × 113,5 mm).
        units: u32,
    },
    /// "Payload Cube Unit" da Spaceport America Cup (100 × 100 × 100·n mm).
    CubeUnit {
        /// Unidades (1 a 4).
        units: u32,
    },
    /// CanSat (formato 1C da LASC: Ø 66 × 105 mm).
    CanSat,
    /// PocketQube de `units` P (50 × 50 × 50 mm por unidade).
    PocketQube {
        /// Unidades.
        units: u32,
    },
    /// Experimento científico.
    Experiment,
    /// Câmera.
    Camera,
    /// Pacote de sensores.
    Sensors,
    /// Cápsula (com ou sem paraquedas próprio: o paraquedas é uma peça ligada a ela).
    Capsule,
    /// Outro.
    Custom,
}

/// Requisitos da carga (vazios = sem limite).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Requirements {
    /// Aceleração axial máxima, g.
    pub max_axial_g: Option<f64>,
    /// Aceleração lateral máxima, g.
    pub max_lateral_g: Option<f64>,
    /// Choque máximo (abertura de paraquedas, pouso), g.
    pub max_shock_g: Option<f64>,
    /// Temperatura mínima, °C.
    pub min_temp_c: Option<f64>,
    /// Temperatura máxima, °C.
    pub max_temp_c: Option<f64>,
    /// Taxa máxima de variação de pressão, Pa/s.
    pub max_dp_dt: Option<f64>,
    /// Diferença máxima de pressão entre a baia e o exterior, Pa.
    pub max_bay_dp: Option<f64>,
    /// Velocidade máxima de pouso, m/s.
    pub max_landing_speed: Option<f64>,
    /// Nível acústico máximo, dB.
    pub max_spl_db: Option<f64>,
    /// Nível de vibração aleatória a que a carga foi qualificada, Grms.
    pub qualified_grms: Option<f64>,
}

/// Carga útil instalada numa peça do foguete (do tipo massa).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
    /// Identificador.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Tipo.
    pub kind: PayloadKind,
    /// Peça que a representa (massa, dimensões e posição).
    pub part: String,
    /// Carga funcional (com experimento ativo).
    #[serde(default)]
    pub functional: bool,
    /// Requisitos.
    #[serde(default)]
    pub requirements: Requirements,
}

/// Modelo de carga útil para começar a edição.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadPreset {
    /// Identificador.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Tipo.
    pub kind: PayloadKind,
    /// Massa, kg.
    pub mass: f64,
    /// Comprimento, m.
    pub length: f64,
    /// Largura (ou diâmetro), m.
    pub width: f64,
    /// Requisitos típicos.
    pub requirements: Requirements,
    /// De onde vêm os números.
    pub source: String,
}

/// Modelos de carga útil. Massas e dimensões das normas citadas; requisitos vazios para
/// o usuário preencher com os do seu experimento.
#[must_use]
pub fn presets() -> Vec<PayloadPreset> {
    let cds = "CubeSat Design Specification rev. 14 (Cal Poly, 2022): 1U até 2,00 kg, 100 × 100 × 113,5 mm; 2U até 4,00 kg; 3U até 6,00 kg.";
    let irec = "SA Cup IREC Rules 2024 v1.4, §2.3: payload de no mínimo 8,8 lb (3,99 kg) em Cube Units de 100 × 100 × 100·n mm.";
    let mut v = vec![
        PayloadPreset {
            id: "cubesat-1u".into(),
            name: "CubeSat 1U".into(),
            kind: PayloadKind::CubeSat { units: 1 },
            mass: 1.33,
            length: 0.1135,
            width: 0.1,
            requirements: Requirements::default(),
            source: cds.into(),
        },
        PayloadPreset {
            id: "cubesat-2u".into(),
            name: "CubeSat 2U".into(),
            kind: PayloadKind::CubeSat { units: 2 },
            mass: 2.66,
            length: 0.227,
            width: 0.1,
            requirements: Requirements::default(),
            source: cds.into(),
        },
        PayloadPreset {
            id: "cubesat-3u".into(),
            name: "CubeSat 3U".into(),
            kind: PayloadKind::CubeSat { units: 3 },
            mass: 4.0,
            length: 0.3405,
            width: 0.1,
            requirements: Requirements::default(),
            source: cds.into(),
        },
    ];
    for n in 1..=4u32 {
        v.push(PayloadPreset {
            id: format!("irec-cube-{n}"),
            name: format!("Payload Cube Unit {n} (SA Cup)"),
            kind: PayloadKind::CubeUnit { units: n },
            mass: 3.99,
            length: 0.1 * f64::from(n),
            width: 0.1,
            requirements: Requirements::default(),
            source: irec.into(),
        });
    }
    let lasc = "LASC Satellite Challenge Standards Manual ed. 7 rev. 2: CubeSat 1U 700–1330 g, 2U 1400–2660 g, 3U 2800–4000 g; CanSat 1C Ø 66 × 105 mm, 200–400 g; PocketQube 1P 200–250 g, 2P 400–500 g, 3P 600–750 g. Massa inicial = máxima permitida.";
    for (id, name, kind, mass, l, w) in [
        (
            "lasc-cubesat-1u",
            "LASC: CubeSat 1U",
            PayloadKind::CubeSat { units: 1 },
            1.33,
            0.1,
            0.1,
        ),
        (
            "lasc-cubesat-2u",
            "LASC: CubeSat 2U",
            PayloadKind::CubeSat { units: 2 },
            2.66,
            0.2,
            0.1,
        ),
        (
            "lasc-cubesat-3u",
            "LASC: CubeSat 3U",
            PayloadKind::CubeSat { units: 3 },
            4.0,
            0.3,
            0.1,
        ),
        (
            "lasc-cansat",
            "LASC: CanSat 1C",
            PayloadKind::CanSat,
            0.4,
            0.105,
            0.066,
        ),
        (
            "lasc-pocketqube-1p",
            "LASC: PocketQube 1P",
            PayloadKind::PocketQube { units: 1 },
            0.25,
            0.05,
            0.05,
        ),
        (
            "lasc-pocketqube-2p",
            "LASC: PocketQube 2P",
            PayloadKind::PocketQube { units: 2 },
            0.5,
            0.114,
            0.05,
        ),
        (
            "lasc-pocketqube-3p",
            "LASC: PocketQube 3P",
            PayloadKind::PocketQube { units: 3 },
            0.75,
            0.178,
            0.05,
        ),
    ] {
        v.push(PayloadPreset {
            id: id.into(),
            name: name.into(),
            kind,
            mass,
            length: l,
            width: w,
            requirements: Requirements::default(),
            source: lasc.into(),
        });
    }
    for (id, name, kind, mass, l, w) in [
        (
            "camera",
            "Câmera de ação",
            PayloadKind::Camera,
            0.12,
            0.06,
            0.045,
        ),
        (
            "sensores",
            "Pacote de sensores",
            PayloadKind::Sensors,
            0.15,
            0.08,
            0.05,
        ),
        (
            "experimento",
            "Experimento científico",
            PayloadKind::Experiment,
            0.5,
            0.15,
            0.07,
        ),
        ("capsula", "Cápsula", PayloadKind::Capsule, 0.4, 0.12, 0.07),
    ] {
        v.push(PayloadPreset {
            id: id.into(),
            name: name.into(),
            kind,
            mass,
            length: l,
            width: w,
            requirements: Requirements::default(),
            source: "Valores iniciais típicos para edição; troque pelos da sua carga.".into(),
        });
    }
    v
}

/// Linha do ambiente: t (s), aceleração axial (g), lateral (g), temperatura do ar (°C),
/// temperatura de recuperação na pele (°C), pressão externa (Pa), dp/dt (Pa/s), diferença
/// de pressão baia − exterior (Pa), nível acústico (dB).
pub type EnvRow = [f64; 9];

/// Uma verificação de requisito.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// Grandeza.
    pub name: String,
    /// Valor no voo.
    pub value: f64,
    /// Limite.
    pub limit: Option<f64>,
    /// Unidade.
    pub unit: String,
    /// Margem: (limite − valor)/|valor| para máximos, (valor − limite)/|limite| para mínimos.
    pub margin: Option<f64>,
    /// Passou (None = sem requisito, só informativo).
    pub pass: Option<bool>,
    /// Observação.
    pub note: String,
}

/// Dimensionamento dos furos de respiro da baia.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VentSizing {
    /// Volume da baia, m³.
    pub volume: f64,
    /// Diferença de pressão admitida, Pa.
    pub dp_limit: f64,
    /// Área total de respiro necessária, m².
    pub area_required: f64,
    /// Diâmetro de cada furo se forem 3 furos, m.
    pub diameter_for_3: f64,
    /// Área de respiro existente, m².
    pub area_now: f64,
}

/// Ambiente de voo de uma carga útil.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadReport {
    /// Carga.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Perfil ao longo do voo, a cada 0,05 s.
    pub rows: Vec<EnvRow>,
    /// Verificações.
    pub checks: Vec<Check>,
    /// Respiro da baia.
    pub vent: VentSizing,
    /// Todos os requisitos atendidos.
    pub pass: bool,
}

const G0: f64 = 9.806_65;
const R_AIR: f64 = 287.052_87;
/// Coeficiente de descarga de orifício de parede fina (Idelchik, Handbook of Hydraulic
/// Resistance, 3ª ed., diagrama 4-9: ≈ 0,61).
const CD_ORIFICE: f64 = 0.61;
/// Fator de recuperação turbulento r = Pr^(1/3) ≈ 0,89 (White, Viscous Fluid Flow, 3ª ed., §7.8).
const RECOVERY: f64 = 0.89;
/// Pressão flutuante da camada-limite turbulenta colada: p_rms ≈ 0,006·q (NASA SP-8072,
/// "Acoustic Loads Generated by the Propulsion System", e Lowson, NASA CR-1660, para camada
/// colada; a separação e choques elevam muito o valor e não entram).
const TBL_PRMS: f64 = 0.006;
/// Nível de vibração aleatória de "mão de obra" (workmanship) do GEVS, NASA GSFC-STD-7000B: 6,8 Grms.
const GEVS_WORKMANSHIP_GRMS: f64 = 6.8;
/// Distância de parada no pouso em solo de terra, m (hipótese; o usuário ajusta a velocidade).
const LANDING_STOP: f64 = 0.05;

fn check(name: &str, value: f64, limit: Option<f64>, unit: &str, max: bool, note: &str) -> Check {
    // Margem = folga relativa: (limite − valor)/|valor| para máximos e (valor − limite)/|limite|
    // para mínimos (vale com valores negativos, como temperaturas em °C).
    let (margin, pass) = match limit {
        Some(l) if max => (
            Some(if value.abs() > 1e-12 {
                (l - value) / value.abs()
            } else {
                f64::INFINITY
            }),
            Some(value <= l),
        ),
        Some(l) => (
            Some(if l.abs() > 1e-12 {
                (value - l) / l.abs()
            } else {
                f64::INFINITY
            }),
            Some(value >= l),
        ),
        None => (None, None),
    };
    Check {
        name: name.into(),
        value,
        limit,
        unit: unit.into(),
        margin: margin.map(|m| if m.is_finite() { m } else { 99.0 }),
        pass,
        note: note.into(),
    }
}

/// Calcula o ambiente de voo de cada carga útil a partir da verdade a 100 Hz da peça.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn environment(asm: &Assembly, out: &SimOutput) -> Vec<PayloadReport> {
    let d = &asm.design;
    let mut reports = Vec::new();
    for pl in &d.payloads {
        let Some(pi) = d.parts.iter().position(|p| p.id == pl.part) else {
            continue;
        };
        let Some((_, truth)) = out.truth.iter().find(|(k, _)| *k == pi) else {
            continue;
        };
        if truth.len() < 3 {
            continue;
        }
        let req = &pl.requirements;
        // Baia: o tubo que envolve a carga.
        let x = asm.props[pi].cg;
        let tube =
            d.parts.iter().zip(&asm.props).find(|(p, q)| {
                matches!(p.kind, PartKind::BodyTube { .. }) && q.x0 <= x && x <= q.x1
            });
        let (volume, area_now) = tube.map_or((0.0, 0.0), |(p, q)| match p.kind {
            PartKind::BodyTube {
                outer_diameter,
                wall,
                vent_holes,
                vent_diameter,
                ..
            } => {
                let di = outer_diameter - 2.0 * wall;
                (
                    core::f64::consts::FRAC_PI_4 * di * di * (q.x1 - q.x0),
                    f64::from(vent_holes)
                        * core::f64::consts::FRAC_PI_4
                        * vent_diameter
                        * vent_diameter,
                )
            }
            _ => (0.0, 0.0),
        });
        let dp_limit = req.max_bay_dp.unwrap_or(1000.0);
        // Pressão na baia: escoamento por orifício, ṁ = Cd·A·√(2ρ|Δp|), e baia isotérmica,
        // dp_b/dt = R·T·ṁ / V. Sem respiro, a baia guarda a pressão do solo.
        let mut pb = truth[0][1];
        let mut area_req: f64 = 0.0;
        let mut rows = Vec::new();
        let mut next = truth[0][0];
        let (mut max_ax, mut max_lat, mut max_dpdt, mut max_bay, mut max_spl): (
            f64,
            f64,
            f64,
            f64,
            f64,
        ) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut t_min, mut t_max) = (f64::INFINITY, f64::NEG_INFINITY);
        for i in 0..truth.len() {
            let r = &truth[i];
            let (a, b) = (i.saturating_sub(1), (i + 1).min(truth.len() - 1));
            let dt_c = truth[b][0] - truth[a][0];
            let dpdt = if dt_c > 1e-9 {
                (truth[b][1] - truth[a][1]) / dt_c
            } else {
                0.0
            };
            let (p, temp) = (r[1], r[2]);
            if i > 0 {
                let dt = r[0] - truth[i - 1][0];
                if volume > 0.0 && area_now > 0.0 && dt > 0.0 {
                    let n = 50;
                    let h = dt / f64::from(n);
                    let p0 = truth[i - 1][1];
                    for k in 1..=n {
                        // Pressão externa interpolada dentro do intervalo.
                        let pe = p0 + (p - p0) * f64::from(k) / f64::from(n);
                        let dp = pe - pb;
                        let rho = pe.max(pb) / (R_AIR * temp);
                        let mdot =
                            CD_ORIFICE * area_now * (2.0 * rho * dp.abs()).sqrt() * dp.signum();
                        let next_pb = pb + R_AIR * temp * mdot / volume * h;
                        // Sem ultrapassar a pressão externa (o passo explícito pode oscilar).
                        pb = if (next_pb - pe) * (pb - pe) < 0.0 {
                            pe
                        } else {
                            next_pb
                        };
                    }
                } else if volume > 0.0 && area_now <= 0.0 {
                    // Selada: guarda a pressão do solo.
                } else {
                    pb = p;
                }
            }
            if volume > 0.0 {
                let rho = p / (R_AIR * temp);
                area_req = area_req.max(
                    volume * dpdt.abs()
                        / (R_AIR * temp * CD_ORIFICE * (2.0 * rho * dp_limit).sqrt()),
                );
            }
            let axial = r[6] / G0;
            let lateral = r[7].hypot(r[8]) / G0;
            let t_c = temp - 273.15;
            let t_rec = temp * (1.0 + RECOVERY * 0.2 * r[4] * r[4]) - 273.15;
            let spl = if r[5] > 0.0 {
                20.0 * (TBL_PRMS * r[5] / 2e-5).log10()
            } else {
                0.0
            };
            max_ax = max_ax.max(axial.abs());
            max_lat = max_lat.max(lateral);
            max_dpdt = max_dpdt.max(dpdt.abs());
            max_bay = max_bay.max((pb - p).abs());
            max_spl = max_spl.max(spl);
            t_min = t_min.min(t_c);
            t_max = t_max.max(t_c);
            if r[0] >= next - 1e-9 {
                next = r[0] + 0.05;
                rows.push([r[0], axial, lateral, t_c, t_rec, p, dpdt, pb - p, spl]);
            }
        }
        // Choque: maior aceleração nos 2 s após ejeções e aberturas de paraquedas.
        let mut shock: f64 = 0.0;
        for e in out
            .events
            .iter()
            .filter(|e| matches!(e.kind.as_str(), "ejection" | "deploy"))
        {
            for r in truth.iter().filter(|r| r[0] >= e.t && r[0] <= e.t + 2.0) {
                shock = shock.max(r[6].hypot(r[7]).hypot(r[8]) / G0);
            }
        }
        // Pouso do corpo que leva a carga.
        let body = out
            .bodies
            .iter()
            .find(|b| b.parts.iter().any(|p| p == &pl.part));
        let landing = body.and_then(|b| b.landing).map_or(f64::NAN, |l| l[1]);
        let mass = body
            .and_then(|b| b.samples.last())
            .map_or(f64::NAN, |s| s[9]);
        let landing_g = if landing.is_finite() {
            landing * landing / (2.0 * LANDING_STOP) / G0
        } else {
            0.0
        };
        let mut checks = vec![
            check("Aceleração axial máxima", max_ax, req.max_axial_g, "g", true, "Força específica ao longo do eixo (o que a carga sente), incluindo a queima e a abertura do paraquedas."),
            check("Aceleração lateral máxima", max_lat, req.max_lateral_g, "g", true, "Perpendicular ao eixo: rajadas, ângulo de ataque e o tranco do paraquedas."),
            check("Choque de abertura do paraquedas", shock, req.max_shock_g, "g", true, "Pico nos 2 s após cada ejeção, a 100 Hz (pirochoque da carga de ejeção não entra)."),
            check("Choque no pouso (parada em 5 cm)", landing_g, req.max_shock_g, "g", true, "Desaceleração média v²/(2·s) supondo 5 cm de parada em solo de terra."),
            check("Temperatura mínima do ar", t_min, req.min_temp_c, "°C", false, "Ar externo na altitude; dentro da baia a temperatura muda mais devagar."),
            check("Temperatura máxima do ar", t_max, req.max_temp_c, "°C", true, "Ar externo; a pele do foguete chega à temperatura de recuperação (coluna do gráfico)."),
            check("Variação de pressão", max_dpdt, req.max_dp_dt, "Pa/s", true, "Taxa máxima de queda ou subida da pressão externa."),
            check("Diferença de pressão na baia", max_bay, Some(dp_limit), "Pa", true, if area_now > 0.0 { "Com os furos de respiro atuais (orifício, Cd = 0,61)." } else { "Baia selada: guarda a pressão do solo." }),
            check("Velocidade de pouso", landing, req.max_landing_speed, "m/s", true, &format!("Energia no pouso: {:.0} J.", 0.5 * mass * landing * landing)),
            check("Ruído acústico (camada-limite)", max_spl, req.max_spl_db, "dB", true, "p_rms ≈ 0,006·q da camada-limite turbulenta colada; ruído do motor e separação não entram."),
        ];
        checks.push(match req.qualified_grms {
            Some(g) => check("Vibração aleatória qualificada", g, Some(GEVS_WORKMANSHIP_GRMS), "Grms", false, "Comparada ao nível de mão de obra do GEVS (NASA GSFC-STD-7000B). O ASTRA não prevê o espectro de vibração do seu foguete."),
            None => Check { name: "Vibração aleatória".into(), value: f64::NAN, limit: Some(GEVS_WORKMANSHIP_GRMS), unit: "Grms".into(), margin: None, pass: None, note: "Sem modelo preditivo confiável para foguetes pequenos: qualifique a carga num ensaio (referência: 6,8 Grms de mão de obra do GEVS).".into() },
        });
        let pass = checks.iter().all(|c| c.pass != Some(false));
        reports.push(PayloadReport {
            id: pl.id.clone(),
            name: pl.name.clone(),
            rows,
            checks,
            vent: VentSizing {
                volume,
                dp_limit,
                area_required: area_req,
                diameter_for_3: (4.0 * area_req / (3.0 * core::f64::consts::PI)).sqrt(),
                area_now,
            },
            pass,
        });
    }
    reports
}
