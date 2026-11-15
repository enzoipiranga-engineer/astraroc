//! Montagem resolvida: propriedades das peças, grafo de juntas, capacidades e cortes.

use crate::material::MaterialDb;
use crate::model::{Design, JointKind, MotorSpec, PartKind};
use crate::props::{is_airframe, is_external, nose_shape, PartProps};
use astra_aero::{BodyPart, FinPlanform, FinProfile, FinSet, Geometry};
use astra_propulsion::{parse_eng, Motor, WaterBurn, WaterRocket};
use astra_vehicle::Rocket;
use core::f64::consts::PI;
use std::collections::BTreeSet;

/// Como uma junta resiste à carga axial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadMode {
    /// Resiste igualmente a tração e compressão (cola, parafusos).
    Symmetric,
    /// Só resiste à tração; a compressão passa pelo contato entre as peças quando as
    /// duas são estrutura do tubo (pinos, atrito, trava, cordão).
    TensionOnly,
}

/// Junta resolvida.
#[derive(Clone, Debug, PartialEq)]
pub struct JointInfo {
    /// Peças (índices).
    pub a: usize,
    /// Peças (índices).
    pub b: usize,
    /// Modo de carga.
    pub mode: LoadMode,
    /// Capacidade em tração (ou cisalhamento axial), N.
    pub tension: f64,
    /// Capacidade em compressão, N (infinita quando o contato entre tubos a carrega).
    pub compression: f64,
    /// Capacidade de momento fletor na raiz de cada aleta, N·m (só juntas de aleta).
    pub fin_moment: f64,
    /// Cordão (não carrega nada até haver separação).
    pub is_cord: bool,
    /// Como a capacidade foi obtida, em linguagem simples.
    pub basis: String,
    /// Nome para mensagens (o identificador da junta, ou a descrição da junta implícita).
    pub name: String,
    /// Junta implícita: contato entre peças encostadas ou peça confinada num tubo.
    pub implicit: bool,
}

/// Conjunto mínimo de juntas cujo rompimento separa `distal` do resto.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    /// Juntas do corte (índices).
    pub joints: Vec<usize>,
    /// Peças do lado que se solta.
    pub distal: Vec<usize>,
    /// O lado que se solta fica à frente do corte.
    pub distal_forward: bool,
    /// Capacidade de tração do corte, N.
    pub tension: f64,
    /// Capacidade de compressão do corte, N.
    pub compression: f64,
}

/// Carga útil das pastilhas de pólvora negra: R·T dos produtos, J/kg. Valor usual de
/// dimensionamento de cargas de ejeção (FFFFg): R = 22,16 ft·lbf/(lbm·°R) e
/// T = 3307 °R, isto é R·T ≈ 2,19e5 J/kg. Confiabilidade: estimado.
pub const BLACK_POWDER_RT: f64 = 2.19e5;

/// Montagem resolvida de um [`Design`].
#[derive(Clone, Debug)]
pub struct Assembly {
    /// Projeto de origem.
    pub design: Design,
    /// Banco de materiais usado.
    pub db: MaterialDb,
    /// Propriedades por peça (mesma ordem de `design.parts`).
    pub props: Vec<PartProps>,
    /// Juntas resolvidas (mesma ordem de `design.joints`); `None` se a junta cita peça
    /// inexistente.
    pub joints: Vec<Option<JointInfo>>,
    /// Peça que recebe o empuxo (tubo do motor ou o tubo mais traseiro).
    pub root: usize,
    /// Motor.
    pub motor: Motor,
    /// Estação da tubeira.
    pub motor_aft: f64,
    /// Distância do CG do propelente à tubeira, m.
    pub prop_cg_from_nozzle: f64,
    /// Diâmetro de saída do motor, m.
    pub nozzle_exit: f64,
    /// Resultado do motor de água, se houver.
    pub water: Option<WaterBurn>,
    /// Erros de leitura do motor.
    pub motor_error: Option<String>,
}

fn wall_of(k: &PartKind) -> Option<f64> {
    match *k {
        PartKind::BodyTube { wall, .. }
        | PartKind::Coupler { wall, .. }
        | PartKind::MotorMount { wall, .. }
        | PartKind::NoseCone { wall, .. }
        | PartKind::Transition { wall, .. } => Some(wall),
        PartKind::Bulkhead { thickness, .. } | PartKind::CenteringRing { thickness, .. } => {
            Some(thickness)
        }
        _ => None,
    }
}

impl Assembly {
    /// Resolve o projeto com o banco de materiais dado.
    #[must_use]
    pub fn new(design: &Design, db: &MaterialDb) -> Self {
        let mut props: Vec<PartProps> = design.parts.iter().map(|p| PartProps::of(p, db)).collect();
        let idx = |id: &str| design.parts.iter().position(|p| p.id == id);

        // Aletas: soma o raio do tubo hospedeiro à inércia axial e transversal.
        for (i, p) in design.parts.iter().enumerate() {
            if let PartKind::FinSet { .. } = p.kind {
                let r = Self::body_radius(design, &props, p.position);
                let pr = &mut props[i];
                let yb = pr.r_in;
                let arm2 = (r + yb).powi(2);
                pr.i_trans += 0.5 * pr.mass * (arm2 - yb * yb);
                pr.i_axial = pr.mass * arm2;
                pr.r_out += r;
            }
        }

        let mut joints: Vec<Option<JointInfo>> = design
            .joints
            .iter()
            .map(|j| {
                let (a, b) = (idx(&j.a)?, idx(&j.b)?);
                let mut info = Self::resolve_joint(design, db, &props, a, b, &j.kind);
                info.name = j.id.clone();
                Some(info)
            })
            .collect();
        Self::add_implicit(design, &props, &mut joints);

        let (motor, motor_aft, prop_cg, nozzle_exit, water, motor_error) =
            Self::resolve_motor(design, &props);
        let root = design
            .parts
            .iter()
            .position(|p| matches!(p.kind, PartKind::MotorMount { .. }))
            .or_else(|| {
                // Tubo externo que contém a tubeira, ou o mais traseiro.
                let ext: Vec<usize> = (0..design.parts.len())
                    .filter(|&i| {
                        matches!(
                            design.parts[i].kind,
                            PartKind::BodyTube { .. } | PartKind::Transition { .. }
                        )
                    })
                    .collect();
                ext.iter()
                    .copied()
                    .find(|&i| props[i].x0 < motor_aft && motor_aft <= props[i].x1 + 1e-6)
                    .or_else(|| {
                        ext.iter()
                            .copied()
                            .max_by(|&a, &b| props[a].x1.total_cmp(&props[b].x1))
                    })
            })
            .unwrap_or(0);
        Self {
            design: design.clone(),
            db: db.clone(),
            props,
            joints,
            root,
            motor,
            motor_aft,
            prop_cg_from_nozzle: prop_cg,
            nozzle_exit,
            water,
            motor_error,
        }
    }

    /// Juntas implícitas, que existem fisicamente mesmo sem estarem no projeto:
    /// - peças externas encostadas sem junta rígida entre elas ficam apoiadas: o contato
    ///   carrega compressão e nenhuma tração (a coifa solta só cai quando é puxada);
    /// - grupos de peças internas sem caminho até uma peça externa ficam presos dentro do
    ///   tubo que os envolve (chacoalham, mas não saem enquanto o tubo estiver fechado).
    fn add_implicit(design: &Design, props: &[PartProps], joints: &mut Vec<Option<JointInfo>>) {
        let n = design.parts.len();
        let rigid_between = |a: usize, b: usize, js: &[Option<JointInfo>]| {
            js.iter()
                .flatten()
                .any(|j| !j.is_cord && ((j.a == a && j.b == b) || (j.a == b && j.b == a)))
        };
        let mut ext: Vec<usize> = (0..n)
            .filter(|&i| is_external(&design.parts[i].kind))
            .collect();
        ext.sort_by(|&a, &b| {
            design.parts[a]
                .position
                .total_cmp(&design.parts[b].position)
        });
        let aero_end = |i: usize| match design.parts[i].kind {
            PartKind::NoseCone { length, .. } => design.parts[i].position + length,
            _ => props[i].x1,
        };
        for w in ext.windows(2) {
            let (a, b) = (w[0], w[1]);
            if (design.parts[b].position - aero_end(a)).abs() < 1e-3 && !rigid_between(a, b, joints)
            {
                joints.push(Some(JointInfo {
                    a,
                    b,
                    mode: LoadMode::TensionOnly,
                    tension: 0.0,
                    compression: f64::INFINITY,
                    fin_moment: 0.0,
                    is_cord: false,
                    basis: "apenas encostadas: só compressão".to_owned(),
                    name: format!("contato {}–{}", design.parts[a].name, design.parts[b].name),
                    implicit: true,
                }));
            }
        }
        // Componentes rígidos sem nenhuma peça externa: confinados no tubo hospedeiro.
        let mut seen = vec![false; n];
        for s in 0..n {
            if seen[s]
                || is_external(&design.parts[s].kind)
                || matches!(
                    design.parts[s].kind,
                    PartKind::FinSet { .. }
                        | PartKind::Parachute { .. }
                        | PartKind::ShockCord { .. }
                )
            {
                continue;
            }
            let mut comp = vec![s];
            seen[s] = true;
            let mut k = 0;
            while k < comp.len() {
                let u = comp[k];
                for j in joints.iter().flatten().filter(|j| !j.is_cord) {
                    let v = if j.a == u {
                        j.b
                    } else if j.b == u {
                        j.a
                    } else {
                        continue;
                    };
                    if !seen[v] {
                        seen[v] = true;
                        comp.push(v);
                    }
                }
                k += 1;
            }
            if comp.iter().any(|&i| is_external(&design.parts[i].kind)) {
                continue;
            }
            let heavy = *comp
                .iter()
                .max_by(|&&a, &&b| props[a].mass.total_cmp(&props[b].mass))
                .unwrap_or(&s);
            let x = props[heavy].cg;
            if let Some(host) = (0..n).find(|&h| {
                is_external(&design.parts[h].kind) && props[h].x0 <= x && x <= props[h].x1
            }) {
                joints.push(Some(JointInfo {
                    a: heavy,
                    b: host,
                    mode: LoadMode::Symmetric,
                    tension: f64::INFINITY,
                    compression: f64::INFINITY,
                    fin_moment: 0.0,
                    is_cord: false,
                    basis: "solta, mas confinada dentro do tubo".to_owned(),
                    name: format!(
                        "confinamento de {} em {}",
                        design.parts[heavy].name, design.parts[host].name
                    ),
                    implicit: true,
                }));
            }
        }
    }

    /// Raio externo do corpo na estação `x` (tubos e transições), m.
    fn body_radius(design: &Design, props: &[PartProps], x: f64) -> f64 {
        design
            .parts
            .iter()
            .zip(props)
            .filter(|(p, pr)| is_external(&p.kind) && pr.x0 <= x && x <= pr.x1)
            .map(|(p, pr)| match p.kind {
                PartKind::Transition {
                    length,
                    d_fore,
                    d_aft,
                    ..
                } => 0.5 * (d_fore + (d_aft - d_fore) * ((x - pr.x0) / length).clamp(0.0, 1.0)),
                _ => pr.r_out,
            })
            .fold(0.0, f64::max)
    }

    #[allow(clippy::too_many_lines)]
    fn resolve_joint(
        design: &Design,
        db: &MaterialDb,
        props: &[PartProps],
        a: usize,
        b: usize,
        kind: &JointKind,
    ) -> JointInfo {
        let (pa, pb) = (&design.parts[a], &design.parts[b]);
        let (qa, qb) = (&props[a], &props[b]);
        let shear_of = |id: &str| db.get(id).map_or(0.0, crate::Material::shear);
        let both_airframe = is_airframe(&pa.kind) && is_airframe(&pb.kind);
        let overlap = (qa.x1.min(qb.x1) - qa.x0.max(qb.x0)).max(0.0);
        let fin = [a, b]
            .into_iter()
            .find(|&i| matches!(design.parts[i].kind, PartKind::FinSet { .. }));
        let mut info = JointInfo {
            a,
            b,
            mode: LoadMode::Symmetric,
            tension: 0.0,
            compression: 0.0,
            fin_moment: 0.0,
            is_cord: false,
            basis: String::new(),
            name: String::new(),
            implicit: false,
        };
        match kind {
            JointKind::Adhesive {
                adhesive,
                area,
                fillet,
                through_wall,
            } => {
                let tau = shear_of(adhesive)
                    .min(shear_of(&pa.material))
                    .min(shear_of(&pb.material));
                if let Some(f) = fin {
                    let PartKind::FinSet {
                        count,
                        root_chord,
                        thickness,
                        ..
                    } = design.parts[f].kind
                    else {
                        unreachable!()
                    };
                    let rf = fillet.unwrap_or(thickness);
                    let fin_mat = db.get(&design.parts[f].material);
                    // Flexão da própria aleta na raiz: σ_esc·c·t²/6.
                    let plate = fin_mat.map_or(0.0, crate::Material::yield_strength)
                        * root_chord
                        * thickness
                        * thickness
                        / 6.0;
                    // Dois filetes formam um binário: cada um resiste τ·c·r_f, com braço t + r_f.
                    let bond = tau * root_chord * rf * (thickness + rf);
                    info.fin_moment = if *through_wall {
                        plate
                    } else {
                        plate.min(bond)
                    };
                    let bonded =
                        area.unwrap_or(root_chord * (thickness + 2.0 * rf) * f64::from(count));
                    info.tension = tau * bonded;
                    info.basis = format!(
                        "raiz da aleta: {} (filete de {:.1} mm)",
                        if *through_wall {
                            "atravessa o tubo; limita a flexão da placa"
                        } else {
                            "colada na superfície"
                        },
                        rf * 1000.0
                    );
                } else {
                    let r_contact = qa.r_out.min(qb.r_out);
                    let geom = 2.0 * PI * r_contact * overlap;
                    let bonded = area.unwrap_or(geom);
                    info.tension = tau * bonded;
                    info.basis = format!(
                        "colagem: {:.0} cm² × {:.1} MPa (menor resistência ao cisalhamento entre adesivo e peças)",
                        bonded * 1e4,
                        tau / 1e6
                    );
                }
                info.compression = info.tension;
            }
            JointKind::Screws {
                count,
                diameter,
                material,
            } => {
                let screw = db.get(material);
                let shear = screw.map_or(0.0, |m| {
                    if m.shear() > 0.0 {
                        m.shear()
                    } else {
                        0.6 * m.ultimate()
                    }
                });
                let per_shear = shear * PI * diameter * diameter / 4.0;
                // Esmagamento da parede mais fina: σ_esc·d·t.
                let bearing = [a, b]
                    .into_iter()
                    .filter_map(|i| {
                        let w = wall_of(&design.parts[i].kind)?;
                        Some(
                            db.get(&design.parts[i].material)
                                .map_or(0.0, crate::Material::yield_strength)
                                * diameter
                                * w,
                        )
                    })
                    .fold(f64::INFINITY, f64::min);
                let per = per_shear.min(bearing);
                info.tension = f64::from(*count) * per;
                info.compression = info.tension;
                info.basis = format!(
                    "{count} parafusos: {:.0} N cada (menor entre cisalhamento do parafuso e esmagamento da parede)",
                    per
                );
            }
            JointKind::ShearPins {
                count,
                diameter,
                material,
            } => {
                let per = shear_of(material) * PI * diameter * diameter / 4.0;
                info.mode = LoadMode::TensionOnly;
                info.tension = f64::from(*count) * per;
                info.basis = format!("{count} pinos de cisalhamento de {:.0} N cada", per);
            }
            JointKind::FrictionFit { holding_force } => {
                info.mode = LoadMode::TensionOnly;
                info.tension = *holding_force;
                info.basis = format!(
                    "encaixe por atrito, força de extração informada de {holding_force:.0} N"
                );
            }
            JointKind::Latch { holding_force } => {
                info.mode = LoadMode::TensionOnly;
                info.tension = *holding_force;
                info.basis = format!("trava de {holding_force:.0} N");
            }
            JointKind::Cord { strength } => {
                info.mode = LoadMode::TensionOnly;
                info.is_cord = true;
                info.tension = *strength;
                info.basis = format!("cordão de {strength:.0} N");
            }
        }
        if info.mode == LoadMode::TensionOnly {
            info.compression = if both_airframe {
                f64::INFINITY
            } else {
                info.tension
            };
        }
        info
    }

    #[allow(clippy::type_complexity)]
    fn resolve_motor(
        design: &Design,
        props: &[PartProps],
    ) -> (Motor, f64, f64, f64, Option<WaterBurn>, Option<String>) {
        let aft_ext = props
            .iter()
            .zip(&design.parts)
            .filter(|(_, p)| is_external(&p.kind))
            .map(|(q, _)| q.x1)
            .fold(0.0, f64::max);
        match &design.motor {
            MotorSpec::None => (Motor::none(), aft_ext, 0.0, 0.0, None, None),
            MotorSpec::Eng { text, aft_station } => match parse_eng(text) {
                Ok(mut v) => {
                    let m = v.swap_remove(0);
                    let half = 0.5 * m.length;
                    (m, *aft_station, half, 0.0, None, None)
                }
                Err(e) => (Motor::none(), *aft_station, 0.0, 0.0, None, Some(e)),
            },
            MotorSpec::Water {
                bottle_volume,
                water_volume,
                gauge_pressure,
                nozzle_diameter,
                max_safe_pressure,
                aft_station,
            } => {
                // Garrafa = tubo externo mais traseiro à frente do bocal.
                let bottle = design
                    .parts
                    .iter()
                    .zip(props)
                    .filter(|(p, q)| {
                        matches!(p.kind, PartKind::BodyTube { .. }) && q.x1 <= aft_station + 1e-6
                    })
                    .max_by(|a, b| a.1.x1.total_cmp(&b.1.x1));
                let (r_in, tube_end) = bottle.map_or((0.05, *aft_station), |(_, q)| (q.r_in, q.x1));
                let w = WaterRocket {
                    bottle_volume: *bottle_volume,
                    water_volume: *water_volume,
                    gauge_pressure: *gauge_pressure,
                    nozzle_diameter: *nozzle_diameter,
                    bottle_diameter: 2.0 * r_in,
                    max_safe_gauge_pressure: *max_safe_pressure,
                    ..WaterRocket::pet_2l()
                };
                let burn = w.simulate();
                let column = water_volume / (PI * r_in * r_in);
                let prop_cg = (aft_station - tube_end) + 0.5 * column;
                (
                    burn.motor.clone(),
                    *aft_station,
                    prop_cg,
                    *nozzle_diameter,
                    Some(burn),
                    None,
                )
            }
        }
    }

    /// Juntas ativas (resolvidas e não rompidas).
    pub(crate) fn active<'a>(
        &'a self,
        broken: &'a BTreeSet<usize>,
    ) -> impl Iterator<Item = (usize, &'a JointInfo)> + 'a {
        self.joints
            .iter()
            .enumerate()
            .filter_map(move |(i, j)| j.as_ref().filter(|_| !broken.contains(&i)).map(|j| (i, j)))
    }

    /// Componentes conexos entre as peças `within`, usando só as juntas aceitas por `use_joint`.
    pub(crate) fn components(
        &self,
        within: &[usize],
        use_joint: impl Fn(usize, &JointInfo) -> bool,
        broken: &BTreeSet<usize>,
    ) -> Vec<Vec<usize>> {
        let set: BTreeSet<usize> = within.iter().copied().collect();
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for &s in within {
            if !seen.insert(s) {
                continue;
            }
            let mut comp = vec![s];
            let mut k = 0;
            while k < comp.len() {
                let u = comp[k];
                for (ji, j) in self.active(broken) {
                    if !use_joint(ji, j) {
                        continue;
                    }
                    let v = if j.a == u {
                        j.b
                    } else if j.b == u {
                        j.a
                    } else {
                        continue;
                    };
                    if set.contains(&v) && seen.insert(v) {
                        comp.push(v);
                    }
                }
                k += 1;
            }
            comp.sort_unstable();
            out.push(comp);
        }
        out
    }

    /// Corpos rígidos (juntas que não são cordão) dentro de `within`.
    #[must_use]
    pub fn rigid_bodies(&self, within: &[usize], broken: &BTreeSet<usize>) -> Vec<Vec<usize>> {
        self.components(within, |_, j| !j.is_cord, broken)
    }

    /// Sistemas amarrados (qualquer junta) dentro de `within`.
    #[must_use]
    pub fn tethered(&self, within: &[usize], broken: &BTreeSet<usize>) -> Vec<Vec<usize>> {
        self.components(within, |_, _| true, broken)
    }

    fn cg_of(&self, parts: &[usize]) -> f64 {
        let m: f64 = parts.iter().map(|&i| self.props[i].mass).sum();
        if m > 0.0 {
            parts
                .iter()
                .map(|&i| self.props[i].mass * self.props[i].cg)
                .sum::<f64>()
                / m
        } else {
            0.0
        }
    }

    /// Cortes mínimos (até três juntas rígidas) do corpo rígido `body`, a partir de `anchor`.
    #[must_use]
    pub fn cuts(&self, body: &[usize], anchor: usize, broken: &BTreeSet<usize>) -> Vec<Cut> {
        // Só o corpo rígido que contém a âncora (paraquedas e cordões ficam de fora).
        let body = self
            .rigid_bodies(body, broken)
            .into_iter()
            .find(|c| c.contains(&anchor))
            .unwrap_or_default();
        let body = body.as_slice();
        let edges: Vec<usize> = self
            .active(broken)
            .filter(|(_, j)| !j.is_cord && body.contains(&j.a) && body.contains(&j.b))
            .map(|(i, _)| i)
            .collect();
        let unbreakable = |e: usize| {
            self.joints[e]
                .as_ref()
                .is_some_and(|j| j.tension.is_infinite() && j.compression.is_infinite())
        };
        let mut found: Vec<Vec<usize>> = Vec::new();
        let mut out = Vec::new();
        let n = edges.len();
        let mut combos: Vec<Vec<usize>> = Vec::new();
        for &e in &edges {
            combos.push(vec![e]);
        }
        for i in 0..n {
            for k in i + 1..n {
                combos.push(vec![edges[i], edges[k]]);
            }
        }
        if n <= 40 {
            for i in 0..n {
                for k in i + 1..n {
                    for l in k + 1..n {
                        combos.push(vec![edges[i], edges[k], edges[l]]);
                    }
                }
            }
        }
        for combo in combos {
            if found.iter().any(|f| f.iter().all(|e| combo.contains(e)))
                || combo.iter().any(|&e| unbreakable(e))
            {
                continue;
            }
            let mut b2 = broken.clone();
            b2.extend(combo.iter().copied());
            let comps = self.rigid_bodies(body, &b2);
            if comps.len() != 2 {
                continue;
            }
            let distal = comps
                .into_iter()
                .find(|c| !c.contains(&anchor))
                .unwrap_or_default();
            let rest: Vec<usize> = body
                .iter()
                .copied()
                .filter(|i| !distal.contains(i))
                .collect();
            let (tension, compression) = combo.iter().fold((0.0, 0.0), |acc, &e| {
                let j = self.joints[e]
                    .as_ref()
                    .map_or((0.0, 0.0), |j| (j.tension, j.compression));
                (acc.0 + j.0, acc.1 + j.1)
            });
            let distal_forward = self.cg_of(&distal) < self.cg_of(&rest);
            found.push(combo.clone());
            out.push(Cut {
                joints: combo,
                distal,
                distal_forward,
                tension,
                compression,
            });
        }
        out
    }

    /// Geometria externa das peças `parts` (na ordem das estações) e os índices das peças
    /// que viraram trechos do corpo e conjuntos de aletas. `fin_loss[i]` aletas perdidas.
    #[must_use]
    pub fn geometry(
        &self,
        parts: &[usize],
        fin_loss: &[u32],
    ) -> (Geometry, Vec<usize>, Vec<usize>) {
        let mut ext: Vec<usize> = parts
            .iter()
            .copied()
            .filter(|&i| is_external(&self.design.parts[i].kind))
            .collect();
        ext.sort_by(|&a, &b| {
            self.design.parts[a]
                .position
                .total_cmp(&self.design.parts[b].position)
        });
        let mut body = Vec::new();
        for &i in &ext {
            body.push(match self.design.parts[i].kind {
                PartKind::NoseCone {
                    shape,
                    length,
                    diameter,
                    ..
                } => BodyPart::Nose {
                    shape: nose_shape(shape),
                    length,
                    diameter,
                },
                PartKind::BodyTube {
                    length,
                    outer_diameter,
                    ..
                } => BodyPart::Tube {
                    length,
                    diameter: outer_diameter,
                },
                PartKind::Transition {
                    length,
                    d_fore,
                    d_aft,
                    ..
                } => BodyPart::Transition {
                    length,
                    d_fore,
                    d_aft,
                },
                _ => unreachable!(),
            });
        }
        let mut fins = Vec::new();
        let mut fin_parts = Vec::new();
        for &i in parts {
            if let PartKind::FinSet {
                count,
                root_chord,
                tip_chord,
                sweep,
                span,
                thickness,
                profile,
            } = self.design.parts[i].kind
            {
                let left = count.saturating_sub(fin_loss.get(i).copied().unwrap_or(0));
                if left == 0 {
                    continue;
                }
                fins.push(FinSet {
                    count: left,
                    planform: FinPlanform::Trapezoid {
                        root_chord,
                        tip_chord,
                        sweep,
                    },
                    span,
                    thickness,
                    position: self.design.parts[i].position,
                    profile: match profile {
                        crate::model::FinProfileSpec::Square => FinProfile::Square,
                        crate::model::FinProfileSpec::Rounded => FinProfile::Rounded,
                        crate::model::FinProfileSpec::Airfoil => FinProfile::Airfoil,
                    },
                });
                fin_parts.push(i);
            }
        }
        // Geometria relativa à primeira estação externa (a aerodinâmica mede do nariz).
        let x0 = ext.first().map_or(0.0, |&i| self.design.parts[i].position);
        for f in &mut fins {
            f.position -= x0;
        }
        let geometry = Geometry {
            parts: body,
            fins,
            roughness: self.design.roughness,
            nozzle_exit_diameter: self.nozzle_exit,
        };
        (geometry, ext, fin_parts)
    }

    /// Estação de referência (frente) da geometria de `parts`.
    #[must_use]
    pub fn front_station(&self, parts: &[usize]) -> f64 {
        parts
            .iter()
            .filter(|&&i| is_external(&self.design.parts[i].kind))
            .map(|&i| self.design.parts[i].position)
            .fold(f64::INFINITY, f64::min)
    }

    /// Massa sem motor de `parts`, com aletas perdidas descontadas, kg.
    #[must_use]
    pub fn dry_mass(&self, parts: &[usize], fin_loss: &[u32]) -> f64 {
        parts
            .iter()
            .map(|&i| self.props[i].mass * self.fin_keep(i, fin_loss))
            .sum()
    }

    fn fin_keep(&self, i: usize, fin_loss: &[u32]) -> f64 {
        match self.design.parts[i].kind {
            PartKind::FinSet { count, .. } => {
                f64::from(count.saturating_sub(fin_loss.get(i).copied().unwrap_or(0)))
                    / f64::from(count.max(1))
            }
            _ => 1.0,
        }
    }

    /// Monta um [`Rocket`] com as peças `parts`. As estações do foguete ficam relativas à
    /// frente da geometria. O motor entra se a peça raiz estiver no conjunto.
    #[must_use]
    pub fn rocket(&self, parts: &[usize], fin_loss: &[u32]) -> Rocket {
        let (geometry, _, _) = self.geometry(parts, fin_loss);
        let x0 = self.front_station(parts);
        let x0 = if x0.is_finite() { x0 } else { 0.0 };
        let (mut m, mut mx) = (0.0, 0.0);
        for &i in parts {
            let k = self.fin_keep(i, fin_loss);
            m += self.props[i].mass * k;
            mx += self.props[i].mass * k * (self.props[i].cg - x0);
        }
        let cg = if m > 0.0 { mx / m } else { 0.0 };
        let (mut ia, mut it) = (0.0, 0.0);
        for &i in parts {
            let (k, p) = (self.fin_keep(i, fin_loss), &self.props[i]);
            ia += p.i_axial * k;
            it += k * (p.i_trans + p.mass * (p.cg - x0 - cg).powi(2));
        }
        let with_motor = parts.contains(&self.root);
        let motor = if with_motor {
            self.motor.clone()
        } else {
            Motor::none()
        };
        // Corpo sem massa (peça solta sem material) recebe um mínimo para a dinâmica.
        let m = m.max(1e-4);
        let mut r = Rocket::new(geometry, m, cg, (ia.max(1e-9), it.max(1e-9)), motor);
        r.nozzle_station = self.motor_aft - x0;
        r.case_cg_from_nozzle = 0.5 * r.motor.length;
        r.prop_cg_from_nozzle = if with_motor {
            self.prop_cg_from_nozzle
        } else {
            0.0
        };
        r
    }

    /// Volume livre (m³) e área interna (m²) da baia aberta pela junta `joint` para o
    /// paraquedas `chute`: do plano da junta até o anteparo mais próximo do lado do
    /// paraquedas, descontando o volume das peças internas.
    #[must_use]
    pub fn bay(&self, joint: usize, chute: usize) -> (f64, f64, Option<usize>) {
        let Some(j) = self.joints.get(joint).and_then(Option::as_ref) else {
            return (0.0, 0.0, None);
        };
        let (qa, qb) = (&self.props[j.a], &self.props[j.b]);
        let plane = 0.5 * (qa.x1.min(qb.x1) + qa.x0.max(qb.x0));
        let c = &self.props[chute];
        let forward = c.cg < plane;
        let r_in = [j.a, j.b]
            .into_iter()
            .filter(|&i| matches!(self.design.parts[i].kind, PartKind::BodyTube { .. }))
            .map(|i| self.props[i].r_in)
            .fold(0.0, f64::max);
        let r_in = if r_in > 0.0 {
            r_in
        } else {
            qa.r_in.max(qb.r_in)
        };
        // Anteparo mais próximo, do lado do paraquedas.
        let mut wall: Option<(usize, f64)> = None;
        for (i, p) in self.design.parts.iter().enumerate() {
            if !matches!(p.kind, PartKind::Bulkhead { .. }) {
                continue;
            }
            let x = self.props[i].cg;
            let ok = if forward { x < plane } else { x > plane };
            if ok && wall.is_none_or(|(_, w)| (x - plane).abs() < (w - plane).abs()) {
                wall = Some((i, x));
            }
        }
        let far = wall.map_or_else(
            || {
                let ext = self
                    .design
                    .parts
                    .iter()
                    .zip(&self.props)
                    .filter(|(p, _)| is_external(&p.kind));
                if forward {
                    ext.map(|(_, q)| q.x0).fold(f64::INFINITY, f64::min)
                } else {
                    ext.map(|(_, q)| q.x1).fold(0.0, f64::max)
                }
            },
            |(_, x)| x,
        );
        let (lo, hi) = if forward { (far, plane) } else { (plane, far) };
        let gross = PI * r_in * r_in * (hi - lo).max(0.0);
        let occupied: f64 = self
            .props
            .iter()
            .enumerate()
            .filter(|(i, p)| !is_airframe(&self.design.parts[*i].kind) && p.cg > lo && p.cg < hi)
            .map(|(_, p)| p.volume)
            .sum();
        (
            (gross - occupied).max(1e-6),
            PI * r_in * r_in,
            wall.map(|w| w.0),
        )
    }

    /// Força da carga de ejeção de massa `charge` (kg) na baia: p = m·R·T/V e F = p·A.
    #[must_use]
    pub fn ejection_force(&self, joint: usize, chute: usize, charge: f64) -> f64 {
        let (v, a, _) = self.bay(joint, chute);
        charge * BLACK_POWDER_RT / v * a
    }

    /// Peças com furo de respiro: os tubos com `vent_holes > 0`.
    #[must_use]
    pub fn vented(&self, part: usize) -> bool {
        matches!(self.design.parts[part].kind, PartKind::BodyTube { vent_holes, .. } if vent_holes > 0)
    }
}

impl Assembly {
    /// Tabela de Cd do projeto, quando houver (vale só para o foguete inteiro).
    #[must_use]
    pub fn design_drag_table(&self) -> Option<astra_vehicle::DragModel> {
        self.design
            .drag_table
            .as_ref()
            .map(|t| astra_vehicle::DragModel::Table {
                power_off: t.power_off.iter().map(|p| (p[0], p[1])).collect(),
                power_on: t.power_on.iter().map(|p| (p[0], p[1])).collect(),
            })
    }
}

impl Assembly {
    /// Junta que abre a baia do paraquedas `chute`: a indicada no projeto ou, se ela não
    /// existir (por exemplo, a fixação da coifa foi apagada), a interface entre peças da
    /// estrutura do tubo mais próxima do paraquedas, incluindo o simples contato.
    #[must_use]
    pub fn separation_joint(&self, chute: usize) -> Option<usize> {
        let PartKind::Parachute {
            separation_joint, ..
        } = &self.design.parts[chute].kind
        else {
            return None;
        };
        if let Some(i) = separation_joint
            .as_ref()
            .and_then(|id| self.design.joints.iter().position(|j| &j.id == id))
        {
            return Some(i);
        }
        separation_joint.as_ref()?;
        let x = self.props[chute].cg;
        self.joints
            .iter()
            .enumerate()
            .filter_map(|(i, j)| j.as_ref().map(|j| (i, j)))
            .filter(|(_, j)| {
                !j.is_cord
                    && is_airframe(&self.design.parts[j.a].kind)
                    && is_airframe(&self.design.parts[j.b].kind)
                    && j.tension.is_finite()
            })
            .min_by(|(_, a), (_, b)| {
                let plane = |j: &JointInfo| {
                    let (qa, qb) = (&self.props[j.a], &self.props[j.b]);
                    0.5 * (qa.x1.min(qb.x1) + qa.x0.max(qb.x0))
                };
                (plane(a) - x).abs().total_cmp(&(plane(b) - x).abs())
            })
            .map(|(i, _)| i)
    }
}
