//! Verificação da montagem: estática (geometria, ligações) e com as cargas do voo nominal.

use crate::assembly::{Assembly, LoadMode};
use crate::limits::LimitKind;
use crate::material::MaterialDb;
use crate::model::{Design, PartKind};
use crate::props::{is_airframe, is_external};
use serde::Serialize;

/// Gravidade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Vai dar errado ou o modelo não consegue representar.
    Error,
    /// Pode dar errado; margem pequena.
    Warning,
    /// Informação útil.
    Info,
}

/// Problema encontrado.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Gravidade.
    pub severity: Severity,
    /// Código estável para a interface.
    pub code: String,
    /// Explicação simples.
    pub message: String,
    /// Como corrigir.
    pub suggestion: String,
    /// Peças envolvidas (identificadores).
    pub parts: Vec<String>,
    /// Juntas envolvidas (identificadores).
    pub joints: Vec<String>,
}

/// Números principais do projeto.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignSummary {
    /// Massa na decolagem, kg.
    pub liftoff_mass: f64,
    /// Massa sem motor, kg.
    pub dry_mass: f64,
    /// CG na decolagem, m da ponta do nariz.
    pub cg: f64,
    /// CP de Barrowman em Mach 0,3, m.
    pub cp: f64,
    /// Margem estática na decolagem, calibres.
    pub static_margin: f64,
    /// Comprimento, m.
    pub length: f64,
    /// Diâmetro de referência, m.
    pub diameter: f64,
    /// Empuxo máximo sobre o peso.
    pub thrust_to_weight: f64,
    /// Custo das peças com preço informado, R$.
    pub cost: f64,
    /// Peças sem preço.
    pub parts_without_cost: usize,
    /// Menor fator de segurança estrutural no voo nominal (capacidade/carga), quando calculado.
    pub min_safety_factor: Option<f64>,
    /// Massa de cada peça, kg (mesma ordem de `parts`).
    pub part_masses: Vec<f64>,
    /// Do voo nominal (sem falhas): apogeu acima do solo, m.
    pub apogee: f64,
    /// Velocidade na saída do trilho, m/s.
    pub rail_exit_speed: f64,
    /// Velocidade máxima, m/s.
    pub max_speed: f64,
    /// Mach máximo.
    pub max_mach: f64,
    /// Perfil do corpo para desenho: [estação, raio] da frente para trás.
    pub profile: Vec<[f64; 2]>,
    /// Extensão de cada peça para desenho: [x0, x1, r_in, r_out] na ordem do projeto.
    pub extents: Vec<[f64; 4]>,
}

/// Resultado da verificação.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    /// Problemas, do mais grave ao menos grave.
    pub issues: Vec<Issue>,
    /// Números do projeto.
    pub summary: DesignSummary,
    /// Análise elétrica, quando há eletrônica.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub electronics: Option<crate::elec::ElecReport>,
}

fn issue(
    sev: Severity,
    code: &str,
    message: String,
    suggestion: &str,
    parts: Vec<String>,
    joints: Vec<String>,
) -> Issue {
    Issue {
        severity: sev,
        code: code.to_owned(),
        message,
        suggestion: suggestion.to_owned(),
        parts,
        joints,
    }
}

const TOL: f64 = 1e-3;

/// Verifica o projeto. Com `with_flight`, roda o voo nominal (sem falhas) para obter as
/// cargas e comparar com a capacidade de cada junta e peça.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn validate(design: &Design, db: &MaterialDb, with_flight: bool) -> Validation {
    let asm = Assembly::new(design, db);
    let parts = &design.parts;
    let mut v: Vec<Issue> = Vec::new();
    let pid = |i: usize| parts[i].id.clone();

    // Identificadores e materiais.
    let mut ids: Vec<&str> = parts.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    for w in ids.windows(2).filter(|w| w[0] == w[1]) {
        v.push(issue(
            Severity::Error,
            "duplicateId",
            format!("Há duas peças com o identificador \"{}\".", w[0]),
            "Renomeie uma delas.",
            vec![w[0].to_owned()],
            vec![],
        ));
    }
    for (i, p) in parts.iter().enumerate() {
        if db.get(&p.material).is_none() {
            v.push(issue(
                Severity::Error,
                "unknownMaterial",
                format!(
                    "{}: material \"{}\" não existe no banco.",
                    p.name, p.material
                ),
                "Escolha um material da lista ou cadastre um novo.",
                vec![pid(i)],
                vec![],
            ));
        }
        if asm.props[i].mass <= 0.0 && db.get(&p.material).is_some() {
            v.push(issue(
                Severity::Error,
                "noMass",
                format!("{}: a peça não tem massa; confira as dimensões.", p.name),
                "Dimensões devem ser positivas.",
                vec![pid(i)],
                vec![],
            ));
        }
    }
    for (ji, j) in design.joints.iter().enumerate() {
        if asm.joints[ji].is_none() {
            v.push(issue(
                Severity::Error,
                "badJoint",
                format!("A junta {} liga uma peça que não existe.", j.id),
                "Refaça a junta escolhendo peças da lista.",
                vec![],
                vec![j.id.clone()],
            ));
        }
    }
    if let Some(e) = &asm.motor_error {
        v.push(issue(
            Severity::Error,
            "motorFile",
            format!("Arquivo do motor: {e}"),
            "Use um arquivo .eng válido (thrustcurve.org).",
            vec![],
            vec![],
        ));
    }
    if let Some(w) = &asm.water {
        for m in &w.warnings {
            v.push(issue(
                Severity::Warning,
                "waterMotor",
                m.clone(),
                "Ajuste água e pressão.",
                vec![],
                vec![],
            ));
        }
    }

    // Casca externa: contínua e sem degraus.
    let mut ext: Vec<usize> = (0..parts.len())
        .filter(|&i| is_external(&parts[i].kind))
        .collect();
    ext.sort_by(|&a, &b| parts[a].position.total_cmp(&parts[b].position));
    if ext.is_empty() {
        v.push(issue(
            Severity::Error,
            "noBody",
            "O foguete não tem corpo externo (coifa, tubo ou transição).".to_owned(),
            "Adicione uma coifa e um tubo.",
            vec![],
            vec![],
        ));
    }
    if let Some(&first) = ext.first() {
        if !matches!(parts[first].kind, PartKind::NoseCone { .. }) {
            v.push(issue(
                Severity::Warning,
                "bluntNose",
                format!(
                    "A frente do foguete é {}, sem coifa: frente rombuda tem muito arrasto.",
                    parts[first].name
                ),
                "Coloque uma coifa na frente.",
                vec![pid(first)],
                vec![],
            ));
        }
    }
    let aero_end = |i: usize| match parts[i].kind {
        PartKind::NoseCone { length, .. } => parts[i].position + length,
        _ => asm.props[i].x1,
    };
    let front_d = |i: usize| match parts[i].kind {
        PartKind::BodyTube { outer_diameter, .. } => outer_diameter,
        PartKind::Transition { d_fore, .. } => d_fore,
        PartKind::NoseCone { .. } => 0.0,
        _ => 0.0,
    };
    let aft_d = |i: usize| match parts[i].kind {
        PartKind::BodyTube { outer_diameter, .. } => outer_diameter,
        PartKind::Transition { d_aft, .. } => d_aft,
        PartKind::NoseCone { diameter, .. } => diameter,
        _ => 0.0,
    };
    for w in ext.windows(2) {
        let (a, b) = (w[0], w[1]);
        let gap = parts[b].position - aero_end(a);
        if gap.abs() > TOL {
            let what = if gap > 0.0 {
                "um vão"
            } else {
                "uma sobreposição"
            };
            v.push(issue(
                Severity::Error,
                "stackGap",
                format!(
                    "Entre {} e {} há {what} de {:.1} mm na casca externa.",
                    parts[a].name,
                    parts[b].name,
                    gap.abs() * 1000.0
                ),
                "Ajuste a posição para as peças se encostarem.",
                vec![pid(a), pid(b)],
                vec![],
            ));
        }
        let (da, db_) = (aft_d(a), front_d(b));
        if (da - db_).abs() > 0.01 * da.max(db_) {
            v.push(issue(
                Severity::Warning,
                "diameterStep",
                format!(
                    "Degrau de diâmetro entre {} ({:.1} mm) e {} ({:.1} mm).",
                    parts[a].name,
                    da * 1000.0,
                    parts[b].name,
                    db_ * 1000.0
                ),
                "Use uma transição ou iguale os diâmetros.",
                vec![pid(a), pid(b)],
                vec![],
            ));
        }
    }

    // Peças internas cabem e não se sobrepõem.
    let internal: Vec<usize> = (0..parts.len())
        .filter(|&i| {
            !is_external(&parts[i].kind)
                && !matches!(
                    parts[i].kind,
                    PartKind::FinSet { .. } | PartKind::ShockCord { .. }
                )
        })
        .collect();
    for &i in &internal {
        let p = &asm.props[i];
        let hosts: Vec<usize> = (0..parts.len())
            .filter(|&h| {
                h != i
                    && (is_external(&parts[h].kind)
                        || matches!(parts[h].kind, PartKind::Coupler { .. }))
                    && asm.props[h].x0 <= p.x1 - TOL
                    && asm.props[h].x1 >= p.x0 + TOL
            })
            .collect();
        let covered_from = hosts
            .iter()
            .map(|&h| asm.props[h].x0)
            .fold(f64::INFINITY, f64::min);
        let covered_to = hosts
            .iter()
            .map(|&h| asm.props[h].x1)
            .fold(f64::NEG_INFINITY, f64::max);
        if hosts.is_empty() || covered_from > p.x0 + TOL || covered_to < p.x1 - TOL {
            v.push(issue(
                Severity::Error,
                "outside",
                format!(
                    "{} está fora do corpo do foguete (de {:.0} a {:.0} mm).",
                    parts[i].name,
                    p.x0 * 1000.0,
                    p.x1 * 1000.0
                ),
                "Mova a peça para dentro de um tubo.",
                vec![pid(i)],
                vec![],
            ));
            continue;
        }
        // Dentro de uma luva, vale o furo da luva; senão o menor furo dos tubos externos.
        let room = hosts
            .iter()
            .filter(|&&h| {
                !(matches!(parts[i].kind, PartKind::Coupler { .. })
                    && matches!(parts[h].kind, PartKind::Coupler { .. }))
            })
            .map(|&h| asm.props[h].r_in)
            .fold(f64::INFINITY, f64::min);
        let is_ring = matches!(
            parts[i].kind,
            PartKind::CenteringRing { .. } | PartKind::Bulkhead { .. } | PartKind::Coupler { .. }
        );
        let room = if is_ring {
            // Anéis e anteparos colam na parede do tubo externo (não da luva ao lado).
            hosts
                .iter()
                .filter(|&&h| {
                    is_external(&parts[h].kind)
                        || matches!(parts[i].kind, PartKind::Bulkhead { .. })
                })
                .map(|&h| asm.props[h].r_in)
                .fold(f64::INFINITY, f64::min)
                .max(room)
        } else {
            room
        };
        if p.r_out > room + 5e-4 {
            v.push(issue(
                Severity::Error,
                "doesNotFit",
                format!(
                    "{} não cabe: tem {:.1} mm de diâmetro e o espaço interno é de {:.1} mm.",
                    parts[i].name,
                    2000.0 * p.r_out,
                    2000.0 * room
                ),
                "Reduza o diâmetro da peça ou use um tubo maior.",
                vec![pid(i)],
                vec![],
            ));
        } else if is_ring && room - p.r_out > 1.5e-3 {
            v.push(issue(
                Severity::Warning,
                "loose",
                format!(
                    "{} tem folga de {:.1} mm no raio: a colagem fica fraca.",
                    parts[i].name,
                    (room - p.r_out) * 1000.0
                ),
                "Ajuste o diâmetro externo ao interno do tubo.",
                vec![pid(i)],
                vec![],
            ));
        }
    }
    let radial = |i: usize| -> (f64, f64) {
        let p = &asm.props[i];
        match parts[i].kind {
            PartKind::Coupler { .. }
            | PartKind::MotorMount { .. }
            | PartKind::CenteringRing { .. } => (p.r_in, p.r_out),
            _ => (0.0, p.r_out),
        }
    };
    for (k, &i) in internal.iter().enumerate() {
        for &j in &internal[k + 1..] {
            let (a, b) = (&asm.props[i], &asm.props[j]);
            let ax = a.x1.min(b.x1) - a.x0.max(b.x0);
            let (ra, rb) = (radial(i), radial(j));
            let rad = ra.1.min(rb.1) - ra.0.max(rb.0);
            if ax > TOL && rad > 5e-4 {
                v.push(issue(
                    Severity::Error,
                    "overlap",
                    format!(
                        "{} e {} ocupam o mesmo espaço ({:.1} mm de sobreposição).",
                        parts[i].name,
                        parts[j].name,
                        ax * 1000.0
                    ),
                    "Mova uma das peças.",
                    vec![pid(i), pid(j)],
                    vec![],
                ));
            }
        }
    }

    // Juntas coladas precisam de contato.
    for (ji, j) in design.joints.iter().enumerate() {
        let Some(info) = &asm.joints[ji] else {
            continue;
        };
        if matches!(j.kind, crate::model::JointKind::Adhesive { area: None, .. })
            && info.tension <= 0.0
        {
            v.push(issue(Severity::Error, "noContact", format!("A colagem {} não tem área: as peças não se tocam ou o adesivo não tem resistência no banco.", j.id), "Posicione as peças em contato ou informe a área colada.", vec![pid(info.a), pid(info.b)], vec![j.id.clone()]));
        }
    }

    // Caminho de carga até a estrutura principal.
    let all: Vec<usize> = (0..parts.len()).collect();
    let none = std::collections::BTreeSet::new();
    let rigid = asm.components(&all, |_, j| !j.is_cord && !j.implicit, &none);
    let tether = asm.components(&all, |_, j| !j.implicit, &none);
    let main_rigid = rigid
        .iter()
        .find(|b| b.contains(&asm.root))
        .cloned()
        .unwrap_or_default();
    let main_tether = tether
        .iter()
        .find(|b| b.contains(&asm.root))
        .cloned()
        .unwrap_or_default();
    // Um aviso por grupo de peças presas entre si, mas não à estrutura principal.
    let mut reported = vec![false; parts.len()];
    for (i, p) in parts.iter().enumerate() {
        if main_rigid.contains(&i) || reported[i] {
            continue;
        }
        let soft = matches!(
            p.kind,
            PartKind::Parachute { .. } | PartKind::ShockCord { .. }
        );
        if soft && main_tether.contains(&i) {
            continue;
        }
        let group = rigid
            .iter()
            .find(|g| g.contains(&i))
            .cloned()
            .unwrap_or_else(|| vec![i]);
        for &g in &group {
            reported[g] = true;
        }
        let head = group
            .iter()
            .copied()
            .find(|&g| matches!(parts[g].kind, PartKind::NoseCone { .. }))
            .or_else(|| group.iter().copied().find(|&g| is_external(&parts[g].kind)))
            .or_else(|| {
                group
                    .iter()
                    .copied()
                    .find(|&g| matches!(parts[g].kind, PartKind::FinSet { .. }))
            })
            .unwrap_or(i);
        let p = &parts[head];
        let others: Vec<&str> = group
            .iter()
            .filter(|&&g| g != head)
            .map(|&g| parts[g].name.as_str())
            .collect();
        let with = if others.is_empty() {
            String::new()
        } else {
            format!(" (junto com {})", others.join(", "))
        };
        let tethered = main_tether.contains(&head);
        let (msg, sug) = match p.kind {
            PartKind::NoseCone { .. } => (
                format!("A coifa \"{}\"{with} não está {} ao resto do foguete: só o contato a segura. Ela sai voando sempre que for puxada: se for pesada ou tiver pouco arrasto, logo após o fim da queima, quando o corpo desacelera mais que ela; ou pela pressão do ar preso na baia.", p.name, if tethered { "presa rigidamente" } else { "conectada" }),
                "Adicione uma junta entre a coifa e o tubo: pinos de cisalhamento (para ejeção), encaixe por atrito ou parafusos.",
            ),
            PartKind::FinSet { .. } => (format!("As aletas \"{}\" não estão coladas nem presas ao tubo: caem na decolagem.", p.name), "Crie uma colagem entre as aletas e o tubo."),
            PartKind::Parachute { .. } | PartKind::ShockCord { .. } => (format!("{} não está amarrado a nada: vai sair voando sozinho.", p.name), "Prenda com uma junta de cordão ao cordão de choque ou a um olhal."),
            _ => (format!("{}{with} não tem caminho de carga até a estrutura principal: fica solta e chacoalha dentro do tubo.", p.name), "Prenda a peça com cola, parafusos ou trava a uma peça já presa."),
        };
        v.push(issue(
            Severity::Error,
            "notConnected",
            msg,
            sug,
            group.iter().map(|&g| pid(g)).collect(),
            vec![],
        ));
    }

    let flying = asm
        .rigid_bodies(&all, &none)
        .into_iter()
        .find(|b| b.contains(&asm.root))
        .unwrap_or_default();
    let mut flying_all = flying.clone();
    for t in asm
        .tethered(&all, &none)
        .into_iter()
        .filter(|t| t.contains(&asm.root))
    {
        for i in t {
            if !flying_all.contains(&i) {
                flying_all.push(i);
            }
        }
    }
    // Recuperação.
    for (c, p) in parts.iter().enumerate() {
        let PartKind::Parachute {
            separation_joint,
            ejection_charge,
            ..
        } = &p.kind
        else {
            continue;
        };
        let Some(id) = separation_joint else {
            v.push(issue(Severity::Error, "chuteNoExit", format!("{}: nenhuma junta de separação foi indicada; o paraquedas não tem por onde sair.", p.name), "Indique a junta que se abre na ejeção (por exemplo, coifa–tubo com pinos).", vec![pid(c)], vec![]));
            continue;
        };
        if !design.joints.iter().any(|j| &j.id == id) {
            let fallback = asm
                .separation_joint(c)
                .and_then(|j| asm.joints[j].as_ref())
                .map(|j| j.name.clone());
            v.push(issue(
                Severity::Error,
                "chuteNoExit",
                format!(
                    "{}: a junta de separação \"{id}\" não existe.{}",
                    p.name,
                    fallback.map_or_else(String::new, |f| format!(
                        " A simulação usa a interface mais próxima ({f})."
                    )),
                ),
                "Escolha uma junta existente.",
                vec![pid(c)],
                vec![],
            ));
        }
        let Some(ji) = asm.separation_joint(c) else {
            continue;
        };
        let (vol, _, wall) = asm.bay(ji, c);
        if let Some(info) = &asm.joints[ji] {
            let (qa, qb) = (&asm.props[info.a], &asm.props[info.b]);
            let plane = 0.5 * (qa.x1.min(qb.x1) + qa.x0.max(qb.x0));
            let x = asm.props[c].cg;
            for (b, bp) in parts.iter().enumerate() {
                if matches!(bp.kind, PartKind::Bulkhead { .. })
                    && asm.props[b].cg > x.min(plane)
                    && asm.props[b].cg < x.max(plane)
                {
                    v.push(issue(Severity::Error, "chuteBlocked", format!("O anteparo {} está entre {} e a junta de separação: bloqueia a saída do paraquedas.", bp.name, p.name), "Mova o paraquedas para o lado da junta ou mude a baia.", vec![pid(c), pid(b)], vec![id.clone()]));
                }
            }
            if wall.is_none() {
                v.push(issue(Severity::Warning, "openBay", format!("A baia de {} não tem anteparo do lado do paraquedas: a carga de ejeção pressuriza o tubo inteiro.", p.name), "Coloque um anteparo fechando a baia.", vec![pid(c)], vec![]));
            }
            let body = flying.clone();
            let cut = asm
                .cuts(&body, asm.root, &none)
                .into_iter()
                .filter(|cu| cu.joints.contains(&ji))
                .min_by_key(|cu| cu.joints.len());
            let force = asm.ejection_force(ji, c, *ejection_charge);
            match cut {
                Some(cu) if *ejection_charge <= 0.0 => v.push(issue(Severity::Error, "noCharge", format!("{}: sem carga de ejeção, a junta de {:.0} N nunca abre.", p.name, cu.tension), "Informe a massa de pólvora da carga de ejeção.", vec![pid(c)], vec![id.clone()])),
                Some(cu) if force <= cu.tension => v.push(issue(Severity::Error, "weakCharge", format!("A carga de ejeção de {:.2} g gera {:.0} N numa baia de {:.0} cm³, menos que os {:.0} N da junta {id}: a baia não abre.", ejection_charge * 1000.0, force, vol * 1e6, cu.tension), "Aumente a carga, reduza o volume da baia ou use menos pinos.", vec![pid(c)], vec![id.clone()])),
                Some(cu) if force < 2.0 * cu.tension => v.push(issue(Severity::Warning, "weakCharge", format!("A carga de ejeção gera {:.0} N, só {:.1} vezes os {:.0} N da junta {id}. O usual é o dobro ou mais.", force, force / cu.tension, cu.tension), "Aumente a carga de ejeção.", vec![pid(c)], vec![id.clone()])),
                None if flying.contains(&info.a) && flying.contains(&info.b) => v.push(issue(Severity::Error, "chuteNoExit", format!("Romper a junta {id} não separa o foguete: há outra ligação rígida entre as duas partes."), "A baia só abre se a junta de separação for a única ligação rígida (o cordão não conta).", vec![pid(c)], vec![id.clone()])),
                _ => {}
            }
            if ![info.a, info.b].iter().any(|&t| asm.vented(t)) {
                let alt_p = astra_env::AtmosphereModel::Ussa1976 { delta_t: 0.0 };
                let dp = alt_p.at(design.launch.altitude).pressure
                    - alt_p.at(design.launch.altitude + 1000.0).pressure;
                v.push(issue(Severity::Warning, "sealedBay", format!("A baia de {} não tem furo de alívio. Na subida o ar preso empurra a junta {id} (cerca de {:.1} kPa a cada 1000 m), e a simulação inclui essa força.", p.name, dp / 1000.0), "Faça um furo de respiro de 2 a 3 mm no tubo da baia.", vec![pid(c)], vec![id.clone()]));
            }
        }
    }
    for (i, p) in parts.iter().enumerate() {
        if let PartKind::Mass {
            needs_static_port: true,
            ..
        } = p.kind
        {
            let x = asm.props[i].cg;
            let vented = parts.iter().enumerate().any(|(t, tp)| {
                matches!(tp.kind, PartKind::BodyTube { .. })
                    && asm.vented(t)
                    && asm.props[t].x0 <= x
                    && x <= asm.props[t].x1
            });
            let in_coupler = parts.iter().enumerate().any(|(t, tp)| {
                matches!(tp.kind, PartKind::Coupler { .. })
                    && asm.props[t].x0 <= x
                    && x <= asm.props[t].x1
            });
            if !vented {
                v.push(issue(Severity::Warning, "noStaticPort", format!("{} mede pressão, mas o tubo onde está não tem furo de respiro: vai ler a pressão da baia fechada e errar o apogeu.", p.name), if in_coupler { "Faça furos de pressão estática na luva e no tubo, alinhados." } else { "Faça 3 ou 4 furos de pressão estática no tubo, longe de degraus." }, vec![pid(i)], vec![]));
            }
        }
    }

    // Resumo e estabilidade.
    let rocket = asm.rocket(
        &main_rigid_or_all(&flying_all, parts.len()),
        &vec![0; parts.len()],
    );
    let x0 = asm.front_station(&flying_all);
    let x0 = if x0.is_finite() { x0 } else { 0.0 };
    let mut s = DesignSummary {
        liftoff_mass: rocket.mass(0.0),
        dry_mass: rocket.dry_mass,
        cg: rocket.cg(0.0) + x0,
        cp: rocket.aero().normal_force(0.3, 0.0).x_cp + x0,
        static_margin: if rocket.geometry().ref_diameter() > 0.0 {
            rocket.static_margin(0.0, 0.3)
        } else {
            0.0
        },
        length: rocket.geometry().length(),
        diameter: rocket.geometry().ref_diameter(),
        thrust_to_weight: asm.motor.peak_thrust() / (rocket.mass(0.0) * 9.806_65),
        cost: parts.iter().filter_map(|p| p.cost).fold(0.0, |a, b| a + b),
        parts_without_cost: parts.iter().filter(|p| p.cost.is_none()).count(),
        part_masses: asm.props.iter().map(|p| p.mass).collect(),
        ..DesignSummary::default()
    };
    let mut prof = Vec::new();
    let g = rocket.geometry();
    let n = 120;
    for k in 0..=n {
        let x = g.length() * k as f64 / n as f64;
        prof.push([x + x0, g.body_radius_at(x)]);
    }
    s.profile = prof;
    s.extents = asm
        .props
        .iter()
        .map(|p| [p.x0, p.x1, p.r_in, p.r_out])
        .collect();
    if s.diameter > 0.0 {
        if s.static_margin < 0.0 {
            v.push(issue(Severity::Error, "unstable", format!("Instável: o CG ({:.0} mm) está atrás do CP ({:.0} mm), margem de {:.2} calibre. O foguete vai capotar ao sair do trilho.", s.cg * 1000.0, s.cp * 1000.0, s.static_margin), "Aumente as aletas, leve-as para trás ou ponha lastro no nariz.", vec![], vec![]));
        } else if s.static_margin < 1.0 {
            v.push(issue(Severity::Warning, "lowMargin", format!("Margem estática de {:.2} calibre na decolagem: abaixo de 1 o foguete fica sensível a vento e a pequenos erros.", s.static_margin), "Busque entre 1 e 2 calibres: aletas maiores ou lastro no nariz.", vec![], vec![]));
        } else if s.static_margin > 4.0 {
            v.push(issue(Severity::Info, "overstable", format!("Margem estática de {:.1} calibres: muito estável, vira bastante contra o vento.", s.static_margin), "Aletas menores reduzem arrasto e desvio com vento.", vec![], vec![]));
        }
    }
    if asm.motor.peak_thrust() > 0.0 && s.thrust_to_weight < 5.0 {
        v.push(issue(
            Severity::Warning,
            "lowThrust",
            format!(
                "Empuxo máximo de só {:.1} vezes o peso; o usual é 5 vezes ou mais.",
                s.thrust_to_weight
            ),
            "Use um motor mais forte ou reduza a massa.",
            vec![],
            vec![],
        ));
    }
    if s.parts_without_cost > 0 {
        v.push(issue(
            Severity::Info,
            "noCost",
            format!(
                "{} peças sem preço; custo parcial de R$ {:.2}.",
                s.parts_without_cost, s.cost
            ),
            "Informe o custo de cada peça para o orçamento.",
            vec![],
            vec![],
        ));
    }

    if with_flight
        && !v.iter().any(|i| {
            i.severity == Severity::Error
                && matches!(
                    i.code.as_str(),
                    "noBody"
                        | "unknownMaterial"
                        | "noMass"
                        | "motorFile"
                        | "duplicateId"
                        | "badJoint"
                )
        })
    {
        let (_, nominal) = crate::sim::run(&asm, true);
        if let Some(nom) = nominal {
            if let Some(b) = nom.out.bodies.first() {
                s.apogee = b.samples.iter().map(|r| r[1]).fold(0.0, f64::max);
                s.max_speed = b.samples.iter().map(|r| r[2]).fold(0.0, f64::max);
                s.max_mach = b.samples.iter().map(|r| r[4]).fold(0.0, f64::max);
            }
            if let Some(e) = nom.out.events.iter().find(|e| e.kind == "railExit") {
                s.rail_exit_speed = e.speed;
                if e.speed < 15.0 {
                    v.push(issue(Severity::Warning, "railExit", format!("Sai do trilho a só {:.1} m/s: as aletas ainda não estabilizam bem (o usual é 15 m/s ou mais).", e.speed), "Trilho mais comprido ou motor com mais empuxo inicial.", vec![], vec![]));
                }
            }
            load_issues(&asm, design, &nom.margins, &mut v);
            // Margem m = (cap − carga)/cap ⇒ fator de segurança = 1/(1 − m).
            s.min_safety_factor = nom
                .margins
                .iter()
                .map(|(_, m)| *m)
                .filter(|m| m.is_finite())
                .map(|m| {
                    if m < 1.0 {
                        1.0 / (1.0 - m)
                    } else {
                        f64::INFINITY
                    }
                })
                .reduce(f64::min)
                .filter(|f| f.is_finite());
            zone_issues(design, &nom.out, &mut v);
            for e in nom
                .out
                .events
                .iter()
                .filter(|e| matches!(e.kind.as_str(), "ejectionFailed" | "noExit"))
            {
                v.push(issue(
                    Severity::Error,
                    "recoveryFails",
                    format!("No voo simulado: {}", e.message),
                    "Revise a carga de ejeção e a junta de separação.",
                    vec![],
                    vec![],
                ));
            }
        }
    }
    // Eletrônica e acionamento pirotécnico.
    let elec = crate::elec::analyze(design, &asm.props);
    v.extend(elec.issues.iter().cloned());
    for p in parts {
        if let PartKind::Parachute {
            deploy: crate::model::DeploySpec::Pyro { ematch },
            ..
        } = &p.kind
        {
            if !design
                .electronics
                .components
                .iter()
                .any(|c| &c.id == ematch)
            {
                v.push(issue(
                    Severity::Error,
                    "pyroMissing",
                    format!(
                        "{}: abre pelo e-match \"{ematch}\", que não existe na eletrônica.",
                        p.name
                    ),
                    "Adicione o e-match na eletrônica ou escolha outro modo de abertura.",
                    vec![p.id.clone()],
                    vec![],
                ));
            } else if design.electronics.firmware.is_none() {
                v.push(issue(
                    Severity::Warning,
                    "pyroNoFirmware",
                    format!("{}: abre pelo firmware, mas nenhum firmware foi carregado; sem ele o paraquedas não abre.", p.name),
                    "Carregue um firmware (.hex) ou use o exemplo.",
                    vec![p.id.clone()],
                    vec![],
                ));
            }
        }
    }
    v.sort_by_key(|i| i.severity);
    Validation {
        issues: v,
        summary: s,
        electronics: (!design.electronics.components.is_empty()).then_some(elec),
    }
}

fn main_rigid_or_all(main: &[usize], n: usize) -> Vec<usize> {
    if main.is_empty() {
        (0..n).collect()
    } else {
        main.to_vec()
    }
}

fn load_issues(
    asm: &Assembly,
    design: &Design,
    margins: &[(crate::limits::Limit, f64)],
    v: &mut Vec<Issue>,
) {
    let sf = design.safety_factor;
    for (l, m) in margins {
        if !m.is_finite() {
            continue;
        }
        // Margem m = (cap − carga)/cap ⇒ fator de segurança = 1/(1 − m).
        let factor = if *m < 1.0 {
            1.0 / (1.0 - m)
        } else {
            f64::INFINITY
        };
        let (what, parts, joints, fix, broken_text) = match &l.kind {
            LimitKind::Cut(cu) => {
                let names: Vec<String> = cu.joints.iter().map(|&j| asm.joints[j].as_ref().map_or_else(String::new, |x| x.name.clone())).collect();
                let distal: Vec<String> = cu.distal.iter().map(|&p| design.parts[p].name.clone()).collect();
                let tension_only = cu.joints.iter().all(|&j| asm.joints[j].as_ref().is_some_and(|i| i.mode == LoadMode::TensionOnly));
                let airframe = cu.distal.iter().all(|&p| is_airframe(&design.parts[p].kind) || !is_external(&design.parts[p].kind));
                let _ = airframe;
                (
                    format!("na junta {} ({:.0} N)", names.join(" + "), cu.tension),
                    cu.distal.iter().map(|&p| design.parts[p].id.clone()).collect::<Vec<_>>(),
                    names.clone(),
                    if tension_only { "Mais pinos ou pinos mais grossos, ou uma trava; se for a junta de ejeção, confira se a carga de ejeção ainda vence." } else { "Aumente a área colada, use adesivo mais forte ou parafusos." },
                    format!("{} se solta durante o voo", distal.join(", ")),
                )
            }
            LimitKind::FinRoot { part, cap, .. } => (
                format!("na raiz das aletas \"{}\" ({:.1} N·m por aleta)", design.parts[*part].name, cap),
                vec![design.parts[*part].id.clone()],
                vec![],
                "Filetes de cola maiores, aletas atravessando o tubo até o tubo do motor, ou aletas mais espessas.",
                "uma aleta arranca".to_owned(),
            ),
            LimitKind::Flutter { part, .. } => (
                format!("no flutter das aletas \"{}\"", design.parts[*part].name),
                vec![design.parts[*part].id.clone()],
                vec![],
                "Aletas mais espessas, de material mais rígido, ou com menor envergadura.",
                "as aletas entram em flutter e se partem".to_owned(),
            ),
            LimitKind::Buckling { part } => (
                format!("na flambagem do tubo \"{}\"", design.parts[*part].name),
                vec![design.parts[*part].id.clone()],
                vec![],
                "Parede mais espessa, material mais rígido ou reforço interno.",
                "o tubo flamba e a estrutura colapsa".to_owned(),
            ),
            LimitKind::Cord { joint, strength } => (
                format!("no cordão {} ({strength:.0} N), na abertura do paraquedas", asm.joints[*joint].as_ref().map_or_else(String::new, |x| x.name.clone())),
                vec![],
                vec![asm.joints[*joint].as_ref().map_or_else(String::new, |x| x.name.clone())],
                "Cordão mais forte, paraquedas que abre mais devagar (rizadura) ou abertura em velocidade menor.",
                "o cordão rompe na abertura".to_owned(),
            ),
        };
        if *m < 0.0 {
            v.push(Issue {
                severity: Severity::Error,
                code: "overload".to_owned(),
                message: format!("No voo sem falhas a carga passa do limite {what}: {broken_text}. Fator de segurança {:.2}.", factor.max(0.0)),
                suggestion: fix.to_owned(),
                parts,
                joints,
            });
        } else if factor < sf {
            v.push(Issue {
                severity: Severity::Warning,
                code: "lowSafety".to_owned(),
                message: format!(
                    "Fator de segurança de {factor:.2} {what}, abaixo dos {sf:.1} exigidos."
                ),
                suggestion: fix.to_owned(),
                parts,
                joints,
            });
        }
    }
}

/// Distância (m) entre dois pontos pela aproximação equirretangular (erro < 0,1% abaixo
/// de alguns quilômetros).
fn ground_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let re = 6_371_008.8;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let x = (lon2 - lon1).to_radians() * (0.5 * (p1 + p2)).cos();
    re * x.hypot(p2 - p1)
}

fn zone_issues(design: &Design, out: &crate::sim::SimOutput, v: &mut Vec<Issue>) {
    use crate::model::ZoneKind;
    if design.zones.is_empty() {
        return;
    }
    let landing_zones: Vec<_> = design
        .zones
        .iter()
        .filter(|z| z.kind == ZoneKind::Landing)
        .collect();
    for b in &out.bodies {
        let Some(last) = b.samples.last() else {
            continue;
        };
        if b.landing.is_none() {
            continue;
        }
        let (lat, lon) = (last[10], last[11]);
        for z in design.zones.iter().filter(|z| z.kind == ZoneKind::KeepOut) {
            let d = ground_distance(lat, lon, z.lat_deg, z.lon_deg);
            if d <= z.radius {
                v.push(Issue {
                    severity: Severity::Error,
                    code: "keepOut".to_owned(),
                    message: format!("{} cai dentro da zona proibida \"{}\" (a {:.0} m do centro, raio de {:.0} m).", b.name, z.name, d, z.radius),
                    suggestion: "Mude o azimute ou a elevação do trilho, o vento de projeto ou o local de lançamento.".to_owned(),
                    parts: vec![],
                    joints: vec![],
                });
            }
        }
        if !landing_zones.is_empty()
            && !landing_zones
                .iter()
                .any(|z| ground_distance(lat, lon, z.lat_deg, z.lon_deg) <= z.radius)
        {
            v.push(Issue {
                severity: Severity::Warning,
                code: "outsideLanding".to_owned(),
                message: format!("{} cai fora das áreas de pouso previstas.", b.name),
                suggestion: "Ajuste o lançador ou a recuperação (paraquedas menor deriva menos)."
                    .to_owned(),
                parts: vec![],
                joints: vec![],
            });
        }
    }
}
