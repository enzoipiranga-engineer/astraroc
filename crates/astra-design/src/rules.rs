//! Regras de competição configuráveis e pontuação.
//!
//! O ASTRA não decide regras: os presets transcrevem só o que está nos documentos
//! oficiais recebidos (ver docs/PENDENCIAS.md), com a seção de cada item. O usuário edita,
//! acrescenta e remove regras conforme as regras oficiais da sua competição.

use crate::checks::DesignSummary;
use crate::payload::PayloadKind;
use crate::sim::SimOutput;
use serde::{Deserialize, Serialize};

/// Grandeza medida por uma regra.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Metric {
    /// Apogeu acima do solo, m.
    ApogeeAgl,
    /// Erro do apogeu em relação ao alvo, %.
    ApogeeErrorPct,
    /// Velocidade na saída do trilho, m/s.
    RailExitSpeed,
    /// Margem estática na decolagem, calibres.
    StaticMargin,
    /// Empuxo/peso na decolagem.
    ThrustToWeight,
    /// Mach máximo.
    MaxMach,
    /// Massa na decolagem, kg.
    LiftoffMass,
    /// Maior velocidade de pouso entre os corpos, m/s.
    LandingSpeed,
    /// Maior energia cinética de pouso entre os corpos, J.
    LandingEnergy,
    /// Maior velocidade vertical de descida no pouso entre os corpos, m/s (sem a deriva
    /// horizontal do vento, que entra em [`Metric::LandingSpeed`]).
    DescentRate,
    /// Maior distância de pouso ao lançador, m.
    Drift,
    /// Massa total das cargas úteis, kg.
    PayloadMass,
    /// Cargas não funcionais no formato Cube Unit (1 = sim, 0 = não).
    PayloadCubeForm,
    /// Sem falha estrutural no voo (1 = íntegro).
    StructureIntact,
    /// Declaração da equipe (1 = atende), sem cálculo.
    Declared,
}

/// Comparação.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    /// valor ≥ min.
    AtLeast,
    /// valor ≤ max.
    AtMost,
    /// min ≤ valor ≤ max.
    Between,
}

/// Uma regra.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// Identificador.
    pub id: String,
    /// Texto da regra.
    pub text: String,
    /// Grandeza.
    pub metric: Metric,
    /// Comparação.
    pub op: Op,
    /// Mínimo.
    #[serde(default)]
    pub min: Option<f64>,
    /// Máximo.
    #[serde(default)]
    pub max: Option<f64>,
    /// Pontos perdidos se violada.
    #[serde(default)]
    pub penalty: f64,
    /// Violação impede o lançamento (requisito de segurança).
    #[serde(default)]
    pub blocking: bool,
    /// Para regras declaradas: a equipe declara que atende.
    #[serde(default)]
    pub declared: bool,
    /// Seção do documento oficial.
    pub source: String,
}

/// Pontuação do apogeu: pontos = P − P/(janela·alvo)·|alvo − apogeu|, só dentro de
/// ±janela·alvo (SA Cup IREC 2024 v1.4, §2.7.1.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApogeeScoring {
    /// Pontos máximos.
    pub points: f64,
    /// Janela relativa (0,3 = ±30 %).
    pub window: f64,
    /// Fator de tolerância k da fórmula (LASC RCSM: 0,71/0,50/0,29); sem ele, k = janela (IREC).
    #[serde(default)]
    pub k: Option<f64>,
}

/// Bônus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bonus {
    /// Texto.
    pub text: String,
    /// Pontos.
    pub points: f64,
    /// Regra que precisa passar para ganhar o bônus (ou declarado).
    #[serde(default)]
    pub metric: Option<Metric>,
    /// Declarado pela equipe.
    #[serde(default)]
    pub declared: bool,
    /// Seção.
    pub source: String,
}

/// Conjunto de regras de uma competição.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSet {
    /// Nome.
    pub name: String,
    /// Documento e versão.
    pub document: String,
    /// Apogeu-alvo acima do solo, m.
    #[serde(default)]
    pub target_apogee: Option<f64>,
    /// Pontuação do apogeu.
    #[serde(default)]
    pub apogee_scoring: Option<ApogeeScoring>,
    /// Pontos de recuperação (passa/não passa).
    #[serde(default)]
    pub recovery_points: Option<f64>,
    /// Fração do alvo para disputar o pódio da categoria.
    #[serde(default)]
    pub podium_fraction: Option<f64>,
    /// Regras.
    pub rules: Vec<Rule>,
    /// Bônus.
    #[serde(default)]
    pub bonuses: Vec<Bonus>,
    /// Pontos dados pela banca, que a simulação não calcula: (item, pontos máximos).
    #[serde(default)]
    pub judged: Vec<(String, f64)>,
    /// O que ainda falta no documento.
    #[serde(default)]
    pub pending: Vec<String>,
}

/// Resultado de uma regra.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleResult {
    /// Regra.
    pub id: String,
    /// Texto.
    pub text: String,
    /// Valor obtido.
    pub value: f64,
    /// Passou.
    pub pass: bool,
    /// Pontos perdidos.
    pub penalty: f64,
    /// Impede o lançamento.
    pub blocking: bool,
    /// Seção.
    pub source: String,
}

/// Avaliação do voo contra as regras.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionReport {
    /// Competição.
    pub name: String,
    /// Documento.
    pub document: String,
    /// Regras.
    pub results: Vec<RuleResult>,
    /// Apogeu acima do solo, m.
    pub apogee: f64,
    /// Alvo, m.
    pub target: Option<f64>,
    /// Pontos de apogeu.
    pub apogee_points: Option<f64>,
    /// Pontos de recuperação.
    pub recovery_points: Option<f64>,
    /// Bônus ganhos: (texto, pontos).
    pub bonuses: Vec<(String, f64)>,
    /// Soma das penalidades.
    pub penalties: f64,
    /// Pontos calculados pela simulação (apogeu + recuperação + bônus − penalidades).
    pub flight_score: f64,
    /// Máximo possível nesses itens.
    pub flight_max: f64,
    /// Itens da banca (não calculados).
    pub judged: Vec<(String, f64)>,
    /// Pode disputar o pódio da categoria.
    pub podium_eligible: Option<bool>,
    /// Pode lançar (nenhuma regra de segurança violada).
    pub launch_allowed: bool,
    /// Pendências do documento.
    pub pending: Vec<String>,
}

const FT: f64 = 0.3048;
const LB: f64 = 0.453_592_37;

#[allow(clippy::too_many_arguments)]
fn rule(
    id: &str,
    text: &str,
    metric: Metric,
    op: Op,
    min: Option<f64>,
    max: Option<f64>,
    penalty: f64,
    source: &str,
) -> Rule {
    Rule {
        id: id.into(),
        text: text.into(),
        metric,
        op,
        min,
        max,
        penalty,
        blocking: false,
        declared: false,
        source: source.into(),
    }
}

fn irec(name: &str, target_ft: f64) -> RuleSet {
    let doc = "SA Cup IREC Rules & Requirements 2024 v1.4";
    RuleSet {
        name: name.into(),
        document: doc.into(),
        target_apogee: Some(target_ft * FT),
        apogee_scoring: Some(ApogeeScoring { points: 350.0, window: 0.3, k: None }),
        recovery_points: Some(150.0),
        podium_fraction: Some(0.5),
        rules: vec![
            rule("payload-massa", "Payload de no mínimo 8,8 lb (aceita até 5 % a menos na balança oficial: 8,4 lb)", Metric::PayloadMass, Op::AtLeast, Some(8.4 * LB), None, 100.0, "§2.3.1 e §2.7.1.6"),
            rule("payload-formato", "Payload não funcional em Payload Cube Units (100 × 100 × 100·n mm, ±2 mm)", Metric::PayloadCubeForm, Op::AtLeast, Some(1.0), None, 100.0, "§2.3.5.1 e §2.7.1.6"),
            rule("payload-funcao", "O foguete chega ao apogeu e se recupera independente de qualquer função do payload", Metric::Declared, Op::AtLeast, Some(1.0), None, 100.0, "§2.3.2 e §2.7.1.6"),
            rule("payload-interface", "Payload pesável separado do foguete (sem ligação inextricável)", Metric::Declared, Op::AtLeast, Some(1.0), None, 100.0, "§2.3.3 e §2.7.1.6"),
            rule("payload-materiais", "Sem quantidade significativa de chumbo ou metais pesados, sem material perigoso, sem animais vertebrados vivos", Metric::Declared, Op::AtLeast, Some(1.0), None, 100.0, "§2.3.4 e §2.7.1.6"),
        ],
        bonuses: vec![Bonus { text: "Payload funcional que mesmo assim adota o Payload Cube Unit".into(), points: 50.0, metric: None, declared: false, source: "§2.7.1.7".into() }],
        judged: vec![
            ("Formulário de inscrição e atualizações".into(), 60.0),
            ("Relatório técnico".into(), 200.0),
            ("Projeto e construção".into(), 240.0),
        ],
        pending: vec!["Requisitos técnicos (velocidade de saída do trilho, estabilidade, descida etc.) estão no DTEG, que ainda não foi recebido: acrescente-os aqui.".into()],
    }
}

/// LASC Rocket Challenge Standards Manual (RCSM), ed. 7, rev. 1 (março de 2026).
fn lasc(name: &str, target: f64, k: f64, min_payload: f64, hybrid: bool) -> RuleSet {
    let blocking = |r: Rule| Rule {
        blocking: true,
        ..r
    };
    let mut rules = vec![
        rule("payload-massa", &format!("Payload de no mínimo {:.0} g (aceita até 5 % a menos na pesagem)", min_payload * 1000.0), Metric::PayloadMass, Op::AtLeast, Some(0.95 * min_payload), None, 0.0, "RKT 1.1.2, tabela 1"),
        blocking(rule("trilho-15", "Velocidade na saída do trilho acima de 15 m/s (abaixo de 30 m/s, só com análise de estabilidade)", Metric::RailExitSpeed, Op::AtLeast, Some(15.0), None, 0.0, "FLT 4.3.4")),
        rule("trilho-30", "Velocidade na saída do trilho de pelo menos 30 m/s (aceita sem análise)", Metric::RailExitSpeed, Op::AtLeast, Some(30.0), None, 0.0, "FLT 4.3.4"),
        blocking(rule("margem", "Margem estática entre 1,5 e 4 calibres (o ASTRA mede na decolagem; a regra vale para toda a subida)", Metric::StaticMargin, Op::Between, Some(1.5), Some(4.0), 0.0, "FLT 4.3.5 e 4.3.6")),
        blocking(rule("tw", "Empuxo/peso de pelo menos 5:1", Metric::ThrustToWeight, Op::AtLeast, Some(5.0), None, 0.0, "PRS 5.1.4")),
        rule("descida", "Velocidade de descida (vertical) abaixo de 10 m/s, sem contar a deriva do vento", Metric::DescentRate, Op::AtMost, None, Some(10.0), 0.0, "REC 8.1.4 e 8.1.6"),
        rule("altimetro", "Altímetro COTS oficial da lista aprovada a bordo", Metric::Declared, Op::AtLeast, Some(1.0), None, 0.0, "ECS 7.1.1"),
        rule("pinos", "Força da ejeção ≥ 3 × a resistência somada dos pinos de cisalhamento", Metric::Declared, Op::AtLeast, Some(1.0), None, 0.0, "STR 6.5.8"),
        rule("flutter", "Velocidade de flutter das aletas ≥ 1,5 × a velocidade máxima", Metric::Declared, Op::AtLeast, Some(1.0), None, 0.0, "STR 6.3.2"),
        rule("lrr", "Revisão de prontidão (LRR) aprovada (verde)", Metric::Declared, Op::AtLeast, Some(1.0), None, 0.0, "FLT 4.1"),
    ];
    if target > 1500.0 {
        rules.push(rule("dual", "Recuperação em dois eventos: estabilizador logo após o apogeu (20–45 m/s) e principal abaixo de 500 m", Metric::Declared, Op::AtLeast, Some(1.0), None, 0.0, "REC 8.1.2 a 8.1.4"));
        rules.push(rule(
            "telemetria",
            "Telemetria COTS da lista aprovada",
            Metric::Declared,
            Op::AtLeast,
            Some(1.0),
            None,
            0.0,
            "ECS 7.1.2",
        ));
    }
    if hybrid {
        rules.push(rule(
            "vasos",
            "Vasos de pressão testados a pelo menos 1,5 × a pressão de operação",
            Metric::Declared,
            Op::AtLeast,
            Some(1.0),
            None,
            0.0,
            "SAF 3.5.4",
        ));
    }
    RuleSet {
        name: name.into(),
        document: "LASC Rocket Challenge Standards Manual (RCSM), ed. 7, rev. 1 (2026)".into(),
        target_apogee: Some(target),
        apogee_scoring: Some(ApogeeScoring { points: 350.0, window: 0.5, k: Some(k) }),
        recovery_points: Some(100.0),
        podium_fraction: None,
        rules,
        bonuses: vec![Bonus { text: "Satélite do Satellite Challenge voando neste foguete (bônus de dois desafios)".into(), points: 25.0, metric: None, declared: false, source: "RCSM, Dual-Challenge Bonus".into() }],
        judged: vec![
            ("Esforço da equipe (atualizações, patch, vídeo, redes)".into(), 100.0),
            ("Relatório da missão".into(), 200.0),
            ("Implementação do projeto".into(), 100.0),
            ("Prontidão operacional (LRR verde + uso do slot)".into(), 100.0),
        ],
        pending: vec!["Penalidades por payload abaixo do mínimo e por atraso não têm valor fixo no RCSM: ficam a critério dos oficiais.".into()],
    }
}

/// Presets de regras: só o que está nos documentos recebidos.
#[must_use]
pub fn presets() -> Vec<(String, RuleSet)> {
    vec![
        (
            "irec-10k".into(),
            irec("Spaceport America Cup — IREC 10.000 ft", 10_000.0),
        ),
        (
            "irec-30k".into(),
            irec("Spaceport America Cup — IREC 30.000 ft", 30_000.0),
        ),
        (
            "lasc-500m".into(),
            lasc("LASC — 0,5 km, propulsão sólida", 500.0, 0.71, 0.400, false),
        ),
        (
            "lasc-1km".into(),
            lasc("LASC — 1 km, propulsão sólida", 1000.0, 0.50, 0.800, false),
        ),
        (
            "lasc-3km".into(),
            lasc("LASC — 3 km, propulsão sólida", 3000.0, 0.29, 4.000, false),
        ),
        (
            "lasc-hibrido-1km".into(),
            lasc(
                "LASC — híbrido/líquido, alvo 1 km",
                1000.0,
                0.50,
                0.400,
                true,
            ),
        ),
        (
            "lasc-hibrido-3km".into(),
            lasc(
                "LASC — híbrido/líquido, alvo 3 km",
                3000.0,
                0.29,
                2.000,
                true,
            ),
        ),
        (
            "vazio".into(),
            RuleSet {
                name: "Regras próprias".into(),
                document: "Informe as regras oficiais da sua competição".into(),
                target_apogee: None,
                apogee_scoring: None,
                recovery_points: None,
                podium_fraction: None,
                rules: vec![],
                bonuses: vec![],
                judged: vec![],
                pending: vec![],
            },
        ),
    ]
}

/// Payload no formato Cube Unit: seção 100 × 100 mm e comprimento 100·n mm, ±2 mm.
fn is_cube_unit(kind: &PayloadKind, length: f64, width: f64) -> bool {
    let n = (length / 0.1).round();
    matches!(kind, PayloadKind::CubeUnit { .. })
        && (1.0..=4.0).contains(&n)
        && (length - 0.1 * n).abs() <= 0.002
        && (width - 0.1).abs() <= 0.002
}

/// Avalia o voo.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn evaluate(
    set: &RuleSet,
    design: &crate::Design,
    out: &SimOutput,
    summary: &DesignSummary,
    payload_mass: &[(f64, f64, f64)],
) -> CompetitionReport {
    let main = out.bodies.first();
    let apogee = main.map_or(0.0, |b| b.samples.iter().map(|r| r[1]).fold(0.0, f64::max));
    let landing = |f: &dyn Fn(&crate::BodyTrack, [f64; 4]) -> f64| {
        out.bodies
            .iter()
            .filter_map(|b| b.landing.map(|l| f(b, l)))
            .fold(0.0, f64::max)
    };
    let intact = !out.events.iter().any(|e| {
        matches!(
            e.kind.as_str(),
            "jointBroken" | "finLost" | "buckled" | "cordBroken" | "ejectionFailed" | "noExit"
        )
    });
    let cube_ok = design
        .payloads
        .iter()
        .zip(payload_mass)
        .all(|(p, &(_, l, w))| p.functional || is_cube_unit(&p.kind, l, w));
    let value = |m: Metric, declared: bool| -> f64 {
        match m {
            Metric::ApogeeAgl => apogee,
            Metric::ApogeeErrorPct => set
                .target_apogee
                .map_or(f64::NAN, |t| 100.0 * (apogee - t).abs() / t),
            Metric::RailExitSpeed => summary.rail_exit_speed,
            Metric::StaticMargin => summary.static_margin,
            Metric::ThrustToWeight => summary.thrust_to_weight,
            Metric::MaxMach => summary.max_mach,
            Metric::LiftoffMass => summary.liftoff_mass,
            Metric::LandingSpeed => landing(&|_, l| l[1]),
            Metric::LandingEnergy => {
                landing(&|b, l| 0.5 * b.samples.last().map_or(0.0, |s| s[9]) * l[1] * l[1])
            }
            Metric::DescentRate => landing(&|b, _| b.samples.last().map_or(0.0, |s| s[3].abs())),
            Metric::Drift => landing(&|_, l| l[2].hypot(l[3])),
            Metric::PayloadMass => payload_mass.iter().map(|x| x.0).sum(),
            Metric::PayloadCubeForm => f64::from(u8::from(!design.payloads.is_empty() && cube_ok)),
            Metric::StructureIntact => f64::from(u8::from(intact)),
            Metric::Declared => f64::from(u8::from(declared)),
        }
    };
    let results: Vec<RuleResult> = set
        .rules
        .iter()
        .map(|r| {
            let v = value(r.metric, r.declared);
            let pass = v.is_finite()
                && match r.op {
                    Op::AtLeast => r.min.is_none_or(|m| v >= m),
                    Op::AtMost => r.max.is_none_or(|m| v <= m),
                    Op::Between => r.min.is_none_or(|m| v >= m) && r.max.is_none_or(|m| v <= m),
                };
            RuleResult {
                id: r.id.clone(),
                text: r.text.clone(),
                value: v,
                pass,
                penalty: if pass { 0.0 } else { r.penalty },
                blocking: r.blocking,
                source: r.source.clone(),
            }
        })
        .collect();
    let apogee_points = match (&set.apogee_scoring, set.target_apogee) {
        // SA Cup IREC 2024 v1.4, §2.7.1.4: 350 − 350/(0,3·alvo)·|alvo − apogeu|, zero fora de ±30 %.
        // LASC RCSM ed. 7, apêndice B: 350 − 350/(k·alvo)·|alvo − apogeu|, só dentro de ±50 %,
        // negativo vira zero.
        (Some(s), Some(t)) => {
            let k = s.k.unwrap_or(s.window);
            Some(if (apogee - t).abs() <= s.window * t {
                (s.points - s.points / (k * t) * (apogee - t).abs()).max(0.0)
            } else {
                0.0
            })
        }
        _ => None,
    };
    // Recuperação: passa se nada quebrou e cada regra de pouso passou.
    let landing_rules_ok = results
        .iter()
        .zip(&set.rules)
        .filter(|(_, r)| {
            matches!(
                r.metric,
                Metric::LandingSpeed | Metric::LandingEnergy | Metric::DescentRate
            )
        })
        .all(|(x, _)| x.pass);
    let recovery_points = set.recovery_points.map(|p| {
        if intact && landing_rules_ok && out.bodies.iter().all(|b| b.landing.is_some()) {
            p
        } else {
            0.0
        }
    });
    let bonuses: Vec<(String, f64)> = set
        .bonuses
        .iter()
        .filter(|b| match b.metric {
            Some(m) => value(m, b.declared) >= 1.0,
            None if b.source.contains("2.7.1.7") => design
                .payloads
                .iter()
                .zip(payload_mass)
                .any(|(p, &(_, l, w))| p.functional && is_cube_unit(&p.kind, l, w)),
            None => b.declared,
        })
        .map(|b| (b.text.clone(), b.points))
        .collect();
    let penalties: f64 = results.iter().map(|r| r.penalty).sum();
    let flight_score = apogee_points.unwrap_or(0.0)
        + recovery_points.unwrap_or(0.0)
        + bonuses.iter().map(|b| b.1).sum::<f64>()
        - penalties;
    let flight_max = set.apogee_scoring.as_ref().map_or(0.0, |s| s.points)
        + set.recovery_points.unwrap_or(0.0)
        + set.bonuses.iter().map(|b| b.points).sum::<f64>();
    CompetitionReport {
        name: set.name.clone(),
        document: set.document.clone(),
        apogee,
        target: set.target_apogee,
        apogee_points,
        recovery_points,
        bonuses,
        penalties,
        flight_score,
        flight_max,
        judged: set.judged.clone(),
        podium_eligible: set
            .podium_fraction
            .zip(set.target_apogee)
            .map(|(f, t)| apogee >= f * t),
        launch_allowed: !results.iter().any(|r| r.blocking && !r.pass),
        results,
        pending: set.pending.clone(),
    }
}
