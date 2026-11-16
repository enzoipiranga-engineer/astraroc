//! Importação de projetos do OpenRocket (XML de dentro do arquivo .ork).
//!
//! Formato: o XML que o OpenRocket grava (`<openrocket><rocket><subcomponents><stage>…`), em
//! unidades SI. Os componentes externos de cada estágio são empilhados na ordem do arquivo;
//! os internos são posicionados pelo `<position type=…>` ou `<axialoffset method=…>` em
//! relação ao pai. O que o .ork não traz (juntas, resistência de cordões, curva do motor) é
//! preenchido com valores iniciais e listado nos avisos.

use crate::model::{
    DeploySpec, Design, FinProfileSpec, Joint, JointKind, MotorSpec, NoseShapeSpec, Part, PartKind,
};
use roxmltree::{Document, Node};
use serde::Serialize;

/// Resultado da importação.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    /// Projeto.
    pub design: Design,
    /// O que precisa de atenção.
    pub warnings: Vec<String>,
}

fn child<'a>(n: Node<'a, 'a>, tag: &str) -> Option<Node<'a, 'a>> {
    n.children().find(|c| c.has_tag_name(tag))
}
fn text<'a>(n: Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    child(n, tag).and_then(|c| c.text()).map(str::trim)
}
/// Número, ou None para "auto"/ausente.
fn num(n: Node<'_, '_>, tag: &str) -> Option<f64> {
    text(n, tag)
        .and_then(|t| t.strip_prefix("auto").map_or(Some(t), |_| None))
        .and_then(|t| t.trim().parse().ok())
}

fn material_id(n: Node<'_, '_>, fallback: &str) -> String {
    let Some(m) = child(n, "material") else {
        return fallback.into();
    };
    let name = m.text().unwrap_or("").to_lowercase();
    let density: f64 = m
        .attribute("density")
        .and_then(|d| d.parse().ok())
        .unwrap_or(0.0);
    let rules: [(&[&str], &str); 16] = [
        (&["phenolic", "fenól", "blue tube"], "papel-fenolico"),
        (&["cardboard", "paper", "kraft", "papel"], "papelao-espiral"),
        (&["carbon", "carbono"], "cfrp-qi"),
        (&["g10", "fr4", "fr-4"], "g10-fr4"),
        (&["fiberglass", "fibreglass", "glass", "vidro"], "gfrp-qi"),
        (&["plywood", "birch", "compensado"], "compensado-betula"),
        (&["balsa"], "balsa"),
        (
            &["basswood", "spruce", "pine", "wood", "madeira"],
            "compensado-betula",
        ),
        (&["alumin"], "al-6061-t6"),
        (&["steel", "aço", "aco"], "aco-1020"),
        (&["titanium", "titânio"], "ti-6al-4v"),
        (&["pla"], "pla"),
        (&["petg"], "petg"),
        (&["polystyrene", "abs", "plastic", "plástico"], "abs"),
        (&["nylon"], "nylon-66"),
        (&["polycarbonate", "lexan"], "policarbonato"),
    ];
    for (keys, id) in rules {
        if keys.iter().any(|k| name.contains(k)) {
            return id.into();
        }
    }
    if m.attribute("type") == Some("bulk") && density > 0.0 {
        // Mais próximo pela densidade entre os sólidos estruturais.
        let db = crate::MaterialDb::builtin();
        if let Some(best) = db
            .materials
            .iter()
            .filter(|x| {
                matches!(
                    x.category.as_str(),
                    "metal" | "composito" | "polimero" | "papel" | "madeira" | "espuma"
                )
            })
            .min_by(|a, b| {
                (a.density_kg_m3 - density)
                    .abs()
                    .total_cmp(&(b.density_kg_m3 - density).abs())
            })
        {
            return best.id.clone();
        }
    }
    fallback.into()
}

/// Posição de um componente interno: (método, deslocamento).
fn placement(n: Node<'_, '_>) -> (String, f64) {
    if let Some(p) = child(n, "axialoffset") {
        return (
            p.attribute("method").unwrap_or("top").to_owned(),
            p.text().and_then(|t| t.trim().parse().ok()).unwrap_or(0.0),
        );
    }
    if let Some(p) = child(n, "position") {
        return (
            p.attribute("type").unwrap_or("top").to_owned(),
            p.text().and_then(|t| t.trim().parse().ok()).unwrap_or(0.0),
        );
    }
    ("top".into(), 0.0)
}

struct Ext {
    tag: String,
    x0: f64,
    len: f64,
    fore: Option<f64>,
    aft: Option<f64>,
    node_index: usize,
}

/// Importa o XML de um .ork.
///
/// # Errors
/// XML inválido ou sem foguete.
#[allow(clippy::too_many_lines)]
pub fn import(xml: &str) -> Result<Imported, String> {
    let doc = Document::parse(xml).map_err(|e| format!("XML inválido: {e}"))?;
    let rocket = doc
        .descendants()
        .find(|n| n.has_tag_name("rocket"))
        .ok_or("O arquivo não tem <rocket>: é um .ork do OpenRocket?")?;
    let mut warnings = Vec::new();
    let mut d = crate::templates::competition_500m();
    d.name = text(rocket, "name")
        .unwrap_or("Importado do OpenRocket")
        .to_owned();
    d.parts.clear();
    d.joints.clear();
    d.motor = MotorSpec::None;
    d.drag_table = None;
    let stages: Vec<Node> = child(rocket, "subcomponents")
        .map(|s| s.children().filter(|c| c.has_tag_name("stage")).collect())
        .unwrap_or_default();
    if stages.is_empty() {
        return Err("Nenhum estágio no foguete.".into());
    }
    if stages.len() > 1 {
        warnings.push(format!(
            "{} estágios: foram empilhados num só corpo (separação de estágios não é importada).",
            stages.len()
        ));
    }
    let externals: Vec<Node> = stages
        .iter()
        .flat_map(|s| {
            child(*s, "subcomponents")
                .into_iter()
                .flat_map(|c| c.children().filter(Node::is_element))
        })
        .collect();
    // Primeira passada: comprimentos e raios.
    let mut ext: Vec<Ext> = Vec::new();
    let mut x = 0.0;
    for (i, n) in externals.iter().enumerate() {
        let tag = n.tag_name().name().to_owned();
        let len = num(*n, "length").unwrap_or(0.0);
        let (fore, aft) = match tag.as_str() {
            "nosecone" => (Some(0.0), num(*n, "aftradius")),
            "bodytube" => (
                num(*n, "radius").or_else(|| num(*n, "outerradius")),
                num(*n, "radius").or_else(|| num(*n, "outerradius")),
            ),
            "transition" => (num(*n, "foreradius"), num(*n, "aftradius")),
            other => {
                warnings.push(format!(
                    "Componente externo <{other}> não suportado: ignorado."
                ));
                continue;
            }
        };
        ext.push(Ext {
            tag,
            x0: x,
            len,
            fore,
            aft,
            node_index: i,
        });
        x += len;
    }
    // Raios "auto": herdam do vizinho.
    for _ in 0..4 {
        for i in 0..ext.len() {
            let prev_aft = (i > 0).then(|| ext[i - 1].aft).flatten();
            let next_fore = ext.get(i + 1).and_then(|e| e.fore);
            let e = &mut ext[i];
            if e.tag == "bodytube" {
                let r = e.fore.or(prev_aft).or(next_fore);
                e.fore = r;
                e.aft = r;
            } else {
                if e.fore.is_none() {
                    e.fore = prev_aft;
                }
                if e.aft.is_none() {
                    e.aft = next_fore;
                }
            }
        }
    }
    let mut used = std::collections::BTreeSet::new();
    let mut mk_id = |name: &str| {
        let base: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_owned();
        let base = if base.is_empty() {
            "peca".to_owned()
        } else {
            base
        };
        let mut id = base.clone();
        let mut k = 2;
        while !used.insert(id.clone()) {
            id = format!("{base}-{k}");
            k += 1;
        }
        id
    };
    let mut parts: Vec<Part> = Vec::new();
    let mut joints: Vec<Joint> = Vec::new();
    let glue = |a: &str, b: &str| Joint {
        id: format!("j-{a}-{b}"),
        a: a.into(),
        b: b.into(),
        kind: JointKind::Adhesive {
            adhesive: "epoxi-estrutural".into(),
            area: None,
            fillet: None,
            through_wall: false,
        },
    };
    let mut prev_ext: Option<String> = None;
    let mut nose_id: Option<String> = None;
    let mut chutes_without_cord = Vec::new();
    for e in &ext {
        let n = externals[e.node_index];
        let name = text(n, "name").unwrap_or(&e.tag).to_owned();
        let id = mk_id(&name);
        let thick = num(n, "thickness").unwrap_or(0.002);
        let (fore, aft) = (e.fore.unwrap_or(0.0), e.aft.unwrap_or(0.0));
        let kind = match e.tag.as_str() {
            "nosecone" => {
                let shape = match text(n, "shape").unwrap_or("ogive") {
                    "conical" => NoseShapeSpec::Conical,
                    "ellipsoid" => NoseShapeSpec::Elliptical,
                    "parabolic" | "power" => NoseShapeSpec::Parabolic,
                    "haack" => {
                        if num(n, "shapeparameter").unwrap_or(0.0) > 0.2 {
                            NoseShapeSpec::LvHaack
                        } else {
                            NoseShapeSpec::VonKarman
                        }
                    }
                    _ => NoseShapeSpec::TangentOgive,
                };
                nose_id = Some(id.clone());
                PartKind::NoseCone {
                    shape,
                    length: e.len,
                    diameter: 2.0 * aft,
                    wall: thick,
                    shoulder_length: num(n, "aftshoulderlength").unwrap_or(0.0),
                }
            }
            "bodytube" => PartKind::BodyTube {
                length: e.len,
                outer_diameter: 2.0 * fore,
                wall: thick,
                vent_holes: 0,
                vent_diameter: 0.003,
            },
            _ => PartKind::Transition {
                length: e.len,
                d_fore: 2.0 * fore,
                d_aft: 2.0 * aft,
                wall: thick,
            },
        };
        if fore <= 0.0 && aft <= 0.0 {
            warnings.push(format!("{name}: raio indefinido no arquivo."));
        }
        parts.push(Part {
            id: id.clone(),
            name: name.clone(),
            material: material_id(n, "papelao-espiral"),
            position: e.x0,
            mass_override: num(n, "overridemass"),
            cost: None,
            kind,
        });
        if let Some(p) = &prev_ext {
            if Some(p) == nose_id.as_ref() {
                joints.push(Joint {
                    id: format!("j-{p}-{id}"),
                    a: p.clone(),
                    b: id.clone(),
                    kind: JointKind::FrictionFit {
                        holding_force: 20.0,
                    },
                });
                warnings.push(format!("Junta {p} — {id}: encaixe por atrito de 20 N (valor inicial; o .ork não diz como o nariz é preso)."));
            } else {
                joints.push(glue(p, &id));
            }
        }
        prev_ext = Some(id.clone());
        // Internos.
        let inner_r = fore.max(aft) - thick;
        let (px0, px1) = (e.x0, e.x0 + e.len);
        let Some(subs) = child(n, "subcomponents") else {
            continue;
        };
        let mut after_cursor = px0;
        let mount_r = subs
            .children()
            .find(|c| c.has_tag_name("innertube"))
            .and_then(|c| num(c, "outerradius"));
        for c in subs.children().filter(Node::is_element) {
            let tag = c.tag_name().name();
            let cname = text(c, "name").unwrap_or(tag).to_owned();
            let len = match tag {
                "trapezoidfinset" | "ellipticalfinset" | "freeformfinset" => {
                    num(c, "rootchord").unwrap_or(0.0)
                }
                "parachute" | "shockcord" | "masscomponent" => {
                    num(c, "packedlength").unwrap_or(num(c, "length").unwrap_or(0.02))
                }
                _ => num(c, "length").unwrap_or(0.0),
            };
            let (method, off) = placement(c);
            let pos = match method.as_str() {
                "bottom" => px1 - len + off,
                "middle" => px0 + 0.5 * (e.len - len) + off,
                "after" => after_cursor + off,
                "absolute" => off,
                _ => px0 + off,
            };
            after_cursor = pos + len;
            let cid = mk_id(&cname);
            let ckind = match tag {
                "innertube" => {
                    let ro = num(c, "outerradius").unwrap_or(0.5 * inner_r);
                    if let Some(m) = child(c, "motormount").and_then(|m| child(m, "motor")) {
                        let des = text(m, "designation").unwrap_or("?");
                        let man = text(m, "manufacturer").unwrap_or("");
                        warnings.push(format!("Motor {man} {des}: o .ork só traz o nome. Carregue a curva (.eng) em Motor e trilho."));
                    }
                    PartKind::MotorMount {
                        length: len,
                        outer_diameter: 2.0 * ro,
                        wall: num(c, "thickness").unwrap_or(0.0005),
                    }
                }
                "tubecoupler" => PartKind::Coupler {
                    length: len,
                    outer_diameter: 2.0 * num(c, "outerradius").unwrap_or(inner_r),
                    wall: num(c, "thickness").unwrap_or(0.001),
                },
                "centeringring" => {
                    let ro = num(c, "outerradius").unwrap_or(inner_r);
                    PartKind::CenteringRing {
                        outer_diameter: 2.0 * ro,
                        inner_diameter: 2.0 * num(c, "innerradius").or(mount_r).unwrap_or(0.5 * ro),
                        thickness: len.max(0.001),
                    }
                }
                "bulkhead" => PartKind::Bulkhead {
                    diameter: 2.0 * num(c, "outerradius").unwrap_or(inner_r),
                    thickness: len.max(0.001),
                },
                "trapezoidfinset" => PartKind::FinSet {
                    count: num(c, "fincount").unwrap_or(3.0).round().max(1.0) as u32,
                    root_chord: len,
                    tip_chord: num(c, "tipchord").unwrap_or(0.0),
                    sweep: num(c, "sweeplength").unwrap_or(0.0),
                    span: num(c, "height").unwrap_or(0.0),
                    thickness: num(c, "thickness").unwrap_or(0.003),
                    profile: match text(c, "crosssection") {
                        Some("airfoil") => FinProfileSpec::Airfoil,
                        Some("rounded") => FinProfileSpec::Rounded,
                        _ => FinProfileSpec::Square,
                    },
                },
                "ellipticalfinset" => {
                    // Trapézio de mesma área (π/4·c·s): ponta t = (π/2 − 1)·c, simétrico.
                    let t = (core::f64::consts::FRAC_PI_2 - 1.0) * len;
                    warnings.push(format!(
                        "{cname}: aleta elíptica aproximada por trapézio de mesma área."
                    ));
                    PartKind::FinSet {
                        count: num(c, "fincount").unwrap_or(3.0).round().max(1.0) as u32,
                        root_chord: len,
                        tip_chord: t,
                        sweep: 0.5 * (len - t),
                        span: num(c, "height").unwrap_or(0.0),
                        thickness: num(c, "thickness").unwrap_or(0.003),
                        profile: FinProfileSpec::Rounded,
                    }
                }
                "freeformfinset" => {
                    let pts: Vec<(f64, f64)> = child(c, "finpoints")
                        .map(|fp| {
                            fp.children()
                                .filter(|p| p.has_tag_name("point"))
                                .filter_map(|p| {
                                    Some((
                                        p.attribute("x")?.parse().ok()?,
                                        p.attribute("y")?.parse().ok()?,
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let span = pts.iter().map(|p| p.1).fold(0.0, f64::max);
                    let root = pts.iter().map(|p| p.0).fold(0.0, f64::max)
                        - pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                    let area = 0.5
                        * pts
                            .iter()
                            .zip(pts.iter().cycle().skip(1))
                            .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
                            .sum::<f64>()
                            .abs();
                    let tip = if span > 0.0 {
                        (2.0 * area / span - root).max(0.0)
                    } else {
                        0.0
                    };
                    let sweep = pts
                        .iter()
                        .filter(|p| (p.1 - span).abs() < 1e-9)
                        .map(|p| p.0)
                        .fold(f64::INFINITY, f64::min);
                    warnings.push(format!("{cname}: aleta de forma livre aproximada por trapézio de mesma área e envergadura."));
                    PartKind::FinSet {
                        count: num(c, "fincount").unwrap_or(3.0).round().max(1.0) as u32,
                        root_chord: root.max(len),
                        tip_chord: tip,
                        sweep: if sweep.is_finite() { sweep } else { 0.0 },
                        span,
                        thickness: num(c, "thickness").unwrap_or(0.003),
                        profile: FinProfileSpec::Square,
                    }
                }
                "parachute" | "streamer" => {
                    let deploy = match text(c, "deployevent").unwrap_or("ejection") {
                        "altitude" => DeploySpec::Altitude {
                            agl: num(c, "deployaltitude").unwrap_or(150.0),
                        },
                        "launch" => DeploySpec::Time {
                            t: num(c, "deploydelay").unwrap_or(0.0),
                        },
                        _ => DeploySpec::Apogee {
                            delay: num(c, "deploydelay").unwrap_or(0.0),
                        },
                    };
                    let (dia, cd) = if tag == "streamer" {
                        // Fita: área equivalente de um paraquedas com Cd de fita (≈ 0,3 por área da fita).
                        let a = num(c, "striplength").unwrap_or(1.0)
                            * num(c, "stripwidth").unwrap_or(0.05);
                        warnings.push(format!(
                            "{cname}: fita convertida num paraquedas de mesma área de arrasto."
                        ));
                        (
                            (4.0 * a / core::f64::consts::PI).sqrt(),
                            num(c, "cd").unwrap_or(0.3),
                        )
                    } else {
                        (
                            num(c, "diameter").unwrap_or(0.3),
                            num(c, "cd").unwrap_or(0.8),
                        )
                    };
                    chutes_without_cord.push(cid.clone());
                    PartKind::Parachute {
                        diameter: dia,
                        cd,
                        packed_length: len,
                        packed_diameter: 2.0 * num(c, "packedradius").unwrap_or(0.4 * inner_r),
                        deploy,
                        separation_joint: None,
                        ejection_charge: 0.0,
                    }
                }
                "shockcord" => {
                    warnings.push(format!(
                        "{cname}: resistência do cordão não vem do .ork; usei 1000 N."
                    ));
                    PartKind::ShockCord {
                        length: num(c, "cordlength").unwrap_or(1.0),
                        linear_density: child(c, "material")
                            .and_then(|m| m.attribute("density"))
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0.005),
                        strength: 1000.0,
                    }
                }
                "masscomponent" => PartKind::Mass {
                    mass: num(c, "mass").unwrap_or(0.0),
                    length: len.max(0.005),
                    diameter: 2.0 * num(c, "packedradius").unwrap_or(0.5 * inner_r),
                    needs_static_port: text(c, "masscomponenttype") == Some("altimeter"),
                },
                "launchlug" | "railbutton" => {
                    warnings.push(format!(
                        "{cname}: guia de trilho ignorada (o arrasto dela não entra)."
                    ));
                    continue;
                }
                other => {
                    warnings.push(format!("Componente interno <{other}> ignorado."));
                    continue;
                }
            };
            let fallback = match &ckind {
                PartKind::Parachute { .. } => "ripstop-1.1",
                PartKind::ShockCord { .. } => "kevlar-tecido",
                PartKind::FinSet { .. } => "compensado-betula",
                _ => "compensado-betula",
            };
            let is_cord = matches!(ckind, PartKind::ShockCord { .. });
            let is_chute = matches!(ckind, PartKind::Parachute { .. });
            parts.push(Part {
                id: cid.clone(),
                name: cname,
                material: material_id(c, fallback),
                position: pos,
                mass_override: num(c, "overridemass"),
                cost: None,
                kind: ckind,
            });
            if is_cord {
                joints.push(Joint {
                    id: format!("j-{cid}-{id}"),
                    a: cid.clone(),
                    b: id.clone(),
                    kind: JointKind::Cord { strength: 1000.0 },
                });
                if let Some(nid) = &nose_id {
                    joints.push(Joint {
                        id: format!("j-{cid}-{nid}"),
                        a: cid.clone(),
                        b: nid.clone(),
                        kind: JointKind::Cord { strength: 1000.0 },
                    });
                }
                for ch in chutes_without_cord.drain(..) {
                    joints.push(Joint {
                        id: format!("j-{ch}-{cid}"),
                        a: ch,
                        b: cid.clone(),
                        kind: JointKind::Cord { strength: 1000.0 },
                    });
                }
            } else if is_chute {
                // Ligado ao cordão quando ele aparecer; senão, ao tubo no fim.
            } else {
                joints.push(glue(&cid, &id));
            }
        }
        for ch in chutes_without_cord.drain(..) {
            joints.push(Joint {
                id: format!("j-{ch}-{id}"),
                a: ch,
                b: id.clone(),
                kind: JointKind::Cord { strength: 1000.0 },
            });
        }
    }
    if parts.is_empty() {
        return Err("Nenhuma peça reconhecida no arquivo.".into());
    }
    // No OpenRocket a carga de ejeção separa o nariz: é a junta de separação dos paraquedas.
    if let Some(sep) = joints
        .iter()
        .find(|j| matches!(j.kind, JointKind::FrictionFit { .. }))
        .map(|j| j.id.clone())
    {
        for p in &mut parts {
            if let PartKind::Parachute {
                separation_joint, ..
            } = &mut p.kind
            {
                *separation_joint = Some(sep.clone());
            }
        }
    }
    if parts
        .iter()
        .any(|p| matches!(p.kind, PartKind::Parachute { .. }))
    {
        warnings.push("Carga de ejeção: no OpenRocket ela vem do motor; informe a massa de pólvora em cada paraquedas.".into());
    }
    warnings.push(
        "Juntas criadas automaticamente (colagem, cordões e encaixe do nariz): confira no editor."
            .into(),
    );
    d.parts = parts;
    d.joints = joints;
    Ok(Imported {
        design: d,
        warnings,
    })
}
