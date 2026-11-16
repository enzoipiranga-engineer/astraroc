//! Cargas nas juntas e nas peças durante o voo, e os veículos que as vigiam.

use crate::assembly::{Assembly, Cut};
use crate::model::{DeploySpec, PartKind};
use astra_aero::Geometry;
use astra_dynamics::flight::{
    FlightCtx, Loads, MassProps, PointVehicle, RigidVehicle, Sequencer, Trigger,
};
use astra_math::Vec3;
use astra_vehicle::{Parachute, Rocket};
use core::f64::consts::PI;
use std::cell::RefCell;

/// O que pode passar do limite.
#[derive(Clone, Debug)]
pub(crate) enum LimitKind {
    /// Tração ou compressão num corte de juntas.
    Cut(Cut),
    /// Flexão na raiz de uma aleta (momento admissível por aleta, braço do CP).
    FinRoot {
        part: usize,
        fin: usize,
        cap: f64,
        arm: f64,
    },
    /// Velocidade de flutter de uma aleta.
    Flutter { part: usize, fin: usize },
    /// Flambagem local ou escoamento de um tubo.
    Buckling { part: usize },
    /// Força de abertura do paraquedas contra a junta de cordão mais fraca.
    Cord { joint: usize, strength: f64 },
}

/// Limite vigiado.
#[derive(Clone, Debug)]
pub(crate) struct Limit {
    pub kind: LimitKind,
}

/// Velocidade de flutter de uma aleta trapezoidal, m/s:
/// V_f = a·√( G / [1,337·AR³·p·(λ + 1) / (2·(AR + 2)·(t/c)³)] ),
/// com AR = s²/S (uma aleta), λ = c_t/c_r, t/c sobre a corda da raiz, p a pressão
/// estática e a a velocidade do som locais.
/// Fonte: Martin, NACA TN 4197 (1958), na forma de Apogee Components, "Peak of Flight"
/// nº 291 (2011). Vale para placa isotrópica fina; para madeira e compósito use o G
/// efetivo. Confiabilidade: estimativa de engenharia (±30%).
#[must_use]
pub fn flutter_speed(
    g_mod: f64,
    span: f64,
    root: f64,
    tip: f64,
    thickness: f64,
    pressure: f64,
    sound: f64,
) -> f64 {
    if pressure <= 0.0 || g_mod <= 0.0 {
        return f64::INFINITY;
    }
    let area = 0.5 * (root + tip) * span;
    let ar = span * span / area;
    let lam = tip / root;
    let tc = thickness / root;
    let denom = 1.337 * ar.powi(3) * pressure * (lam + 1.0) / (2.0 * (ar + 2.0) * tc.powi(3));
    sound * (g_mod / denom).sqrt()
}

/// Tensão crítica de flambagem local de casca cilíndrica sob compressão axial, Pa:
/// σ_cr = γ·E·t/(R·√(3(1 − ν²))), ν = 0,3, com o fator de redução empírico
/// γ = 1 − 0,901·(1 − e^(−φ)), φ = √(R/t)/16.
/// Fonte: NASA SP-8007, "Buckling of Thin-Walled Circular Cylinders" (rev. 1968).
#[must_use]
pub fn shell_buckling_stress(e: f64, radius: f64, wall: f64) -> f64 {
    let phi = (radius / wall).sqrt() / 16.0;
    let gamma = 1.0 - 0.901 * (1.0 - (-phi).exp());
    gamma * e * wall / (radius * (3.0 * (1.0 - 0.09_f64)).sqrt())
}

/// Corpo rígido em voo 6-DOF, vigiando os limites das suas juntas e peças.
pub(crate) struct BodyVehicle<'a> {
    pub asm: &'a Assembly,
    pub members: Vec<usize>,
    pub rocket: Rocket,
    pub x0: f64,
    pub ext: Vec<usize>,
    pub fin_parts: Vec<usize>,
    pub fin_loss: Vec<u32>,
    pub limits: Vec<Limit>,
    /// Paraquedas cujos gatilhos este corpo carrega.
    pub chutes: Vec<usize>,
    /// Juntas de baia fechadas e sem respiro: (junta, área interna).
    pub sealed: Vec<(usize, f64)>,
    pub ground_pressure: f64,
    /// Se presente, registra a menor margem de cada limite em vez de parar o voo.
    pub monitor: Option<RefCell<Vec<f64>>>,
}

/// Disparo do paraquedas. Para o acionamento pirotécnico, o instante vem do firmware
/// emulado (`Electronics::pyro_events`); sem disparo, o paraquedas nunca abre.
pub(crate) fn trigger_of(d: &DeploySpec, design: &crate::model::Design) -> Trigger {
    match d {
        DeploySpec::Apogee { delay } => Trigger::AfterApogee { delay: *delay },
        DeploySpec::Altitude { agl } => Trigger::DescendingThrough { agl: *agl },
        DeploySpec::Time { t } => Trigger::Time(*t),
        DeploySpec::Pyro { ematch } => {
            Trigger::Time(crate::elec::pyro_time(design, ematch).unwrap_or(f64::INFINITY))
        }
    }
}

impl<'a> BodyVehicle<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asm: &'a Assembly,
        members: Vec<usize>,
        fin_loss: &[u32],
        broken: &std::collections::BTreeSet<usize>,
        chutes: Vec<usize>,
        ground_pressure: f64,
        monitor: bool,
        full: bool,
    ) -> Self {
        let mut rocket = asm.rocket(&members, fin_loss);
        if full {
            if let Some(t) = &asm.design_drag_table() {
                rocket.drag = t.clone();
            }
        }
        let (_, ext, fin_parts) = asm.geometry(&members, fin_loss);
        let x0 = asm.front_station(&members);
        let anchor = if members.contains(&asm.root) {
            asm.root
        } else {
            *members
                .iter()
                .max_by(|&&a, &&b| asm.props[a].mass.total_cmp(&asm.props[b].mass))
                .unwrap_or(&members[0])
        };
        let mut limits: Vec<Limit> = asm
            .cuts(&members, anchor, broken)
            .into_iter()
            .map(|c| Limit {
                kind: LimitKind::Cut(c),
            })
            .collect();
        for (k, &p) in fin_parts.iter().enumerate() {
            if let Some((ji, j)) = asm
                .active(broken)
                .find(|(_, j)| (j.a == p || j.b == p) && j.fin_moment > 0.0)
            {
                let _ = ji;
                let PartKind::FinSet {
                    span,
                    root_chord: cr,
                    tip_chord: ct,
                    ..
                } = asm.design.parts[p].kind
                else {
                    continue;
                };
                let arm = span * (cr + 2.0 * ct) / (3.0 * (cr + ct));
                limits.push(Limit {
                    kind: LimitKind::FinRoot {
                        part: p,
                        fin: k,
                        cap: j.fin_moment,
                        arm,
                    },
                });
            }
            limits.push(Limit {
                kind: LimitKind::Flutter { part: p, fin: k },
            });
        }
        for &p in &members {
            if matches!(asm.design.parts[p].kind, PartKind::BodyTube { .. })
                && asm
                    .db
                    .get(&asm.design.parts[p].material)
                    .is_some_and(|m| m.e() > 0.0)
            {
                limits.push(Limit {
                    kind: LimitKind::Buckling { part: p },
                });
            }
        }
        let mut sealed = Vec::new();
        for &c in &chutes {
            {
                if let Some(ji) = asm.separation_joint(c) {
                    if let Some(j) = &asm.joints[ji] {
                        let tubes = [j.a, j.b];
                        if !tubes.iter().any(|&t| asm.vented(t)) && !broken.contains(&ji) {
                            let (_, area, _) = asm.bay(ji, c);
                            sealed.push((ji, area));
                        }
                    }
                }
            }
        }
        let n = limits.len();
        Self {
            asm,
            members,
            rocket,
            x0: if x0.is_finite() { x0 } else { 0.0 },
            ext,
            fin_parts,
            fin_loss: fin_loss.to_vec(),
            limits,
            chutes,
            sealed,
            ground_pressure,
            monitor: monitor.then(|| RefCell::new(vec![f64::INFINITY; n])),
        }
    }

    fn keep(&self, i: usize) -> f64 {
        match self.asm.design.parts[i].kind {
            PartKind::FinSet { count, .. } => {
                f64::from(count.saturating_sub(self.fin_loss[i])) / f64::from(count.max(1))
            }
            _ => 1.0,
        }
    }

    /// Massa e força aerodinâmica axial (corpo, N) sobre um subconjunto de peças.
    fn subset(&self, parts: &[usize], axial_drag: &dyn Fn(usize) -> f64) -> (f64, f64, f64) {
        let (mut m, mut mx, mut fa) = (0.0, 0.0, 0.0);
        for &i in parts {
            let w = self.asm.props[i].mass * self.keep(i);
            m += w;
            mx += w * self.asm.props[i].cg;
            fa += axial_drag(i);
        }
        (m, if m > 0.0 { mx / m } else { 0.0 }, fa)
    }

    /// Margem de cada limite para o estado dado (e as cargas já calculadas).
    pub fn margins(
        &self,
        ctx: &FlightCtx<'_>,
        v: Vec3,
        loads: &Loads,
        which: Option<usize>,
    ) -> Vec<f64> {
        let a_ref = self.rocket.geometry().ref_area();
        let qa = ctx.q_dyn * a_ref;
        let speed = ctx.speed.max(1e-9);
        let axial = v.x / speed;
        let re = if ctx.atmo.viscosity > 0.0 {
            ctx.atmo.density * ctx.speed / ctx.atmo.viscosity
        } else {
            0.0
        };
        let need_drag = self
            .limits
            .iter()
            .any(|l| matches!(l.kind, LimitKind::Cut(_) | LimitKind::Buckling { .. }));
        let pd = if need_drag && qa > 0.0 {
            Some(self.rocket.part_cd(ctx.mach, re, ctx.burning))
        } else {
            None
        };
        let drag_of = |i: usize| -> f64 {
            let Some(pd) = &pd else { return 0.0 };
            let cd = self
                .ext
                .iter()
                .position(|&e| e == i)
                .map(|k| pd.body[k])
                .or_else(|| {
                    self.fin_parts
                        .iter()
                        .position(|&f| f == i)
                        .map(|k| pd.fins[k])
                })
                .unwrap_or(0.0);
            -qa * cd * axial
        };
        let m_tot = ctx.mass.max(1e-9);
        let a_x = loads.force_body.x / m_tot;
        let a_lat = loads.force_body.y.hypot(loads.force_body.z) / m_tot;
        let alpha = v.y.hypot(v.z).atan2(v.x.abs().max(1e-12));
        let g = self.rocket.geometry();
        let mut out = Vec::with_capacity(self.limits.len());
        for (li, l) in self.limits.iter().enumerate() {
            if which.is_some_and(|w| w != li) {
                out.push(1.0);
                continue;
            }
            let m = match &l.kind {
                LimitKind::Cut(c) => {
                    let (md, _, fa) = self.subset(&c.distal, &drag_of);
                    let f = md * a_x - fa;
                    let mut tension = if c.distal_forward { -f } else { f };
                    for &(ji, area) in &self.sealed {
                        if c.joints.contains(&ji) {
                            tension += (self.ground_pressure - ctx.atmo.pressure).max(0.0) * area;
                        }
                    }
                    let w = (md * 9.806_65).max(1e-6);
                    let mt = (c.tension - tension) / c.tension.max(w);
                    let mc = if c.compression.is_finite() {
                        (c.compression + tension) / c.compression.max(w)
                    } else {
                        1.0
                    };
                    mt.min(mc)
                }
                LimitKind::FinRoot {
                    fin,
                    cap,
                    arm,
                    part,
                } => {
                    let fs = &g.fins[*fin];
                    let r = g.body_radius_at(fs.position + 0.5 * fs.root_chord());
                    let k = 1.0 + r / (fs.span + r);
                    let f = qa * k * fs.single_fin_cn_alpha(ctx.mach, a_ref) * alpha.sin();
                    let _ = part;
                    (cap - f * arm) / cap.max(1e-9)
                }
                LimitKind::Flutter { part, fin } => {
                    let PartKind::FinSet {
                        span,
                        root_chord,
                        tip_chord,
                        thickness,
                        ..
                    } = self.asm.design.parts[*part].kind
                    else {
                        out.push(1.0);
                        continue;
                    };
                    let gm = self
                        .asm
                        .db
                        .get(&self.asm.design.parts[*part].material)
                        .map_or(0.0, crate::Material::g);
                    let _ = fin;
                    let vf = flutter_speed(
                        gm,
                        span,
                        root_chord,
                        tip_chord,
                        thickness,
                        ctx.atmo.pressure,
                        ctx.atmo.sound_speed,
                    );
                    if vf.is_finite() {
                        (vf - ctx.speed) / vf
                    } else {
                        1.0
                    }
                }
                LimitKind::Buckling { part } => {
                    let pr = &self.asm.props[*part];
                    let s = pr.x1;
                    let fwd: Vec<usize> = self
                        .members
                        .iter()
                        .copied()
                        .filter(|&i| self.asm.props[i].cg < s)
                        .collect();
                    let (mf, cgf, fa) = self.subset(&fwd, &drag_of);
                    let n = (mf * a_x - fa).max(0.0);
                    let s_rel = s - self.x0;
                    let lateral: f64 = g
                        .linear_components(ctx.mach)
                        .iter()
                        .filter(|c| c.1 < s_rel)
                        .map(|c| qa * c.0 * alpha.sin() * (s_rel - c.1))
                        .sum();
                    let bend = (lateral - mf * a_lat * (s - cgf)).abs();
                    let (rr, t) = (0.5 * (pr.r_out + pr.r_in), pr.r_out - pr.r_in);
                    let mat = self.asm.db.get(&self.asm.design.parts[*part].material);
                    let (e, sy) = mat.map_or((0.0, 0.0), |m| (m.e(), m.yield_strength()));
                    let sigma = n / (2.0 * PI * rr * t) + bend * rr / (PI * rr.powi(3) * t);
                    let crit = shell_buckling_stress(e, rr, t).min(sy);
                    (crit - sigma) / crit.max(1.0)
                }
                LimitKind::Cord { .. } => 1.0,
            };
            out.push(m);
        }
        out
    }

    fn ctx_loads(&self, ctx: &FlightCtx<'_>, v: Vec3, w: Vec3) -> Loads {
        RigidVehicle::loads(&self.rocket, ctx, v, w)
    }
}

impl Sequencer for BodyVehicle<'_> {
    fn burnout_time(&self) -> f64 {
        self.rocket.motor.burn_time()
    }
    fn breakpoints(&self) -> Vec<f64> {
        self.rocket.motor.breakpoints()
    }
    fn triggers(&self) -> Vec<Trigger> {
        self.chutes
            .iter()
            .map(|&c| match &self.asm.design.parts[c].kind {
                PartKind::Parachute { deploy, .. } => trigger_of(deploy, &self.asm.design),
                _ => unreachable!(),
            })
            .collect()
    }
    fn margin_count(&self) -> usize {
        if self.monitor.is_some() {
            0
        } else {
            self.limits.len()
        }
    }
    fn stop_on_trigger(&self) -> bool {
        true
    }
}

impl RigidVehicle for BodyVehicle<'_> {
    fn loads(&self, ctx: &FlightCtx<'_>, v: Vec3, w: Vec3) -> Loads {
        let l = self.ctx_loads(ctx, v, w);
        if let Some(mon) = &self.monitor {
            let ms = self.margins(ctx, v, &l, None);
            let mut mon = mon.borrow_mut();
            for (a, b) in mon.iter_mut().zip(ms) {
                *a = a.min(b);
            }
        }
        l
    }
    fn mass_props(&self, t: f64, mass: f64) -> MassProps {
        RigidVehicle::mass_props(&self.rocket, t, mass)
    }
    fn margin(&self, ctx: &FlightCtx<'_>, v: Vec3, w: Vec3, index: usize) -> f64 {
        let l = self.ctx_loads(ctx, v, w);
        self.margins(ctx, v, &l, Some(index))[index]
    }
}

/// Coeficiente de arrasto de um corpo capotando, aplicado à área projetada média.
/// Pela fórmula de Cauchy, a área projetada média de um corpo convexo em orientação
/// aleatória é um quarto da área da superfície. Cd ≈ 1 (ordem de grandeza de Hoerner,
/// "Fluid-Dynamic Drag", cap. 3, para cilindros em escoamento cruzado). Estimado.
pub(crate) const TUMBLE_CD: f64 = 1.0;

/// Superfície externa (m²) de uma geometria: corpo, aletas (duas faces) e as bases.
pub(crate) fn surface_area(g: &Geometry) -> f64 {
    let mut s = 0.0;
    for p in &g.parts {
        let l = p.length();
        let n = 50;
        for i in 0..n {
            let (a, b) = (
                p.radius_at(l * i as f64 / n as f64),
                p.radius_at(l * (i + 1) as f64 / n as f64),
            );
            s += PI * (a + b) * (l / n as f64).hypot(b - a);
        }
    }
    if let (Some(f), Some(b)) = (g.parts.first(), g.parts.last()) {
        s += PI * (0.5 * f.diameters().0).powi(2) + PI * (0.5 * b.diameters().1).powi(2);
    }
    s + g
        .fins
        .iter()
        .map(|f| 2.0 * f64::from(f.count) * f.area())
        .sum::<f64>()
}

/// Sistema em voo de ponto de massa: corpos amarrados, sob paraquedas, ou capotando.
pub(crate) struct PointSystem<'a> {
    pub asm: &'a Assembly,
    /// Corpos rígidos (foguete montado de cada um).
    pub bodies: Vec<Rocket>,
    /// Área de arrasto de cada corpo capotando, m².
    pub tumble_cda: Vec<f64>,
    /// Paraquedas abertos: (peça, instante de abertura, velame).
    pub open: Vec<(usize, f64, Parachute)>,
    /// Paraquedas ainda fechados cujo gatilho este sistema carrega.
    pub pending: Vec<usize>,
    /// Área de arrasto fixa adicional (aletas soltas), m², e massa sem motor.
    pub extra_cda: f64,
    pub limits: Vec<Limit>,
    pub motor_body: Option<usize>,
    pub monitor: Option<RefCell<Vec<f64>>>,
}

impl PointSystem<'_> {
    /// Força total dos paraquedas abertos, N.
    pub fn chute_force(&self, ctx: &FlightCtx<'_>) -> f64 {
        ctx.q_dyn
            * self
                .open
                .iter()
                .map(|(_, t0, p)| p.drag_area(ctx.t - t0))
                .sum::<f64>()
    }
    fn cord_margins(&self, ctx: &FlightCtx<'_>) -> Vec<f64> {
        let f = self.chute_force(ctx);
        self.limits
            .iter()
            .map(|l| match l.kind {
                LimitKind::Cord { strength, .. } => (strength - f) / strength.max(1e-9),
                _ => 1.0,
            })
            .collect()
    }
}

impl Sequencer for PointSystem<'_> {
    fn burnout_time(&self) -> f64 {
        self.motor_body
            .map_or(0.0, |b| self.bodies[b].motor.burn_time())
    }
    fn breakpoints(&self) -> Vec<f64> {
        self.motor_body
            .map_or_else(Vec::new, |b| self.bodies[b].motor.breakpoints())
    }
    fn triggers(&self) -> Vec<Trigger> {
        self.pending
            .iter()
            .map(|&c| match &self.asm.design.parts[c].kind {
                PartKind::Parachute { deploy, .. } => trigger_of(deploy, &self.asm.design),
                _ => unreachable!(),
            })
            .collect()
    }
    fn margin_count(&self) -> usize {
        if self.monitor.is_some() {
            0
        } else {
            self.limits.len()
        }
    }
    fn stop_on_trigger(&self) -> bool {
        true
    }
}

impl PointVehicle for PointSystem<'_> {
    /// Corpo capotando não tem direção de empuxo definida: o empuxo médio é tomado como
    /// nulo (o propelente continua saindo). Simplificação documentada.
    fn thrust(&self, ctx: &FlightCtx<'_>) -> (f64, f64) {
        self.motor_body
            .map_or((0.0, 0.0), |b| (0.0, self.bodies[b].motor.mdot(ctx.t)))
    }
    fn drag_area(&self, ctx: &FlightCtx<'_>) -> f64 {
        if let Some(mon) = &self.monitor {
            let ms = self.cord_margins(ctx);
            let mut mon = mon.borrow_mut();
            for (a, b) in mon.iter_mut().zip(ms) {
                *a = a.min(b);
            }
        }
        let re = if ctx.atmo.viscosity > 0.0 {
            ctx.atmo.density * ctx.speed / ctx.atmo.viscosity
        } else {
            0.0
        };
        let trailing = !self.open.is_empty();
        let bodies: f64 = self
            .bodies
            .iter()
            .zip(&self.tumble_cda)
            .map(|(r, &tumble)| {
                if trailing {
                    r.cd(ctx.mach, re, false) * r.geometry().ref_area()
                } else {
                    tumble
                }
            })
            .sum();
        bodies
            + self.extra_cda
            + self
                .open
                .iter()
                .map(|(_, t0, p)| p.drag_area(ctx.t - t0))
                .sum::<f64>()
    }
    fn margin(&self, ctx: &FlightCtx<'_>, index: usize) -> f64 {
        self.cord_margins(ctx)[index]
    }
}
