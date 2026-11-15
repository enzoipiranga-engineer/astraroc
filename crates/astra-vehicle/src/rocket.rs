use crate::recovery::Parachute;
use astra_aero::{Aero, FlowState, Geometry, PartDrag};
use astra_dynamics::flight::{
    FlightCtx, Loads, MassProps, PointVehicle, RigidVehicle, Sequencer, Trigger,
};
use astra_math::{Mat3, Vec3};
use astra_propulsion::Motor;

/// De onde vem o coeficiente de arrasto em ângulo de ataque nulo.
#[derive(Clone, Debug, PartialEq)]
pub enum DragModel {
    /// Modelo analítico por componentes ([`Geometry::drag`]).
    Analytic,
    /// Tabelas (Mach, Cd) externas (CFD, túnel de vento, RASAero, voo calibrado), com
    /// motor apagado e aceso. Interpolação linear; fora da tabela vale o valor da ponta.
    Table {
        /// Motor apagado.
        power_off: Vec<(f64, f64)>,
        /// Motor aceso.
        power_on: Vec<(f64, f64)>,
    },
}

fn interp(table: &[(f64, f64)], x: f64) -> f64 {
    match table {
        [] => 0.0,
        [only] => only.1,
        _ => {
            let i = table
                .partition_point(|p| p.0 <= x)
                .clamp(1, table.len() - 1);
            let (a, b) = (table[i - 1], table[i]);
            a.1 + (b.1 - a.1) * ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
        }
    }
}

/// Foguete de um estágio pronto para simular.
#[derive(Clone, Debug)]
pub struct Rocket {
    aero: Aero,
    /// Modelo de arrasto.
    pub drag: DragModel,
    /// Massa sem motor, kg.
    pub dry_mass: f64,
    /// Estação do CG sem motor, m a partir da ponta do nariz.
    pub dry_cg: f64,
    /// Inércia sem motor em torno do próprio CG: (axial, transversal), kg·m².
    pub dry_inertia: (f64, f64),
    /// Motor.
    pub motor: Motor,
    /// Estação do plano de saída da tubeira, m.
    pub nozzle_station: f64,
    /// Distância do CG do motor vazio à tubeira, m.
    pub case_cg_from_nozzle: f64,
    /// Distância do CG do propelente à tubeira, m (fixa durante a queima).
    pub prop_cg_from_nozzle: f64,
    /// Paraquedas.
    pub parachutes: Vec<Parachute>,
}

impl Rocket {
    /// Monta o foguete. O motor é tratado como cilindro homogêneo para a inércia.
    #[must_use]
    pub fn new(
        geometry: Geometry,
        dry_mass: f64,
        dry_cg: f64,
        dry_inertia: (f64, f64),
        motor: Motor,
    ) -> Self {
        let nozzle_station = geometry.length();
        let half = 0.5 * motor.length;
        Self {
            aero: Aero::new(geometry),
            drag: DragModel::Analytic,
            dry_mass,
            dry_cg,
            dry_inertia,
            motor,
            nozzle_station,
            case_cg_from_nozzle: half,
            prop_cg_from_nozzle: half,
            parachutes: Vec::new(),
        }
    }

    /// Geometria externa.
    #[must_use]
    pub fn geometry(&self) -> &Geometry {
        self.aero.geometry()
    }

    /// Modelo aerodinâmico.
    #[must_use]
    pub fn aero(&self) -> &Aero {
        &self.aero
    }

    /// Coeficiente de arrasto de cada peça externa em α = 0. Com tabela de Cd, as
    /// parcelas analíticas são escaladas para somar o valor da tabela.
    #[must_use]
    pub fn part_cd(&self, mach: f64, reynolds_per_m: f64, burning: bool) -> PartDrag {
        let (total, mut parts) = self.aero.drag_parts(&FlowState {
            mach,
            reynolds_per_m,
            burning,
        });
        if let DragModel::Table { .. } = self.drag {
            let k = self.cd(mach, reynolds_per_m, burning) / total.total().max(1e-12);
            parts
                .body
                .iter_mut()
                .chain(parts.fins.iter_mut())
                .for_each(|c| *c *= k);
        }
        parts
    }

    /// Massa total no instante `t`, kg.
    #[must_use]
    pub fn mass(&self, t: f64) -> f64 {
        self.dry_mass + self.motor.dry_mass + self.motor.prop_mass(t)
    }

    /// Estação do CG no instante `t`, m.
    #[must_use]
    pub fn cg(&self, t: f64) -> f64 {
        let (mc, mp) = (self.motor.dry_mass, self.motor.prop_mass(t));
        (self.dry_mass * self.dry_cg
            + mc * (self.nozzle_station - self.case_cg_from_nozzle)
            + mp * (self.nozzle_station - self.prop_cg_from_nozzle))
            / (self.dry_mass + mc + mp)
    }

    /// Inércia (axial, transversal) em torno do CG no instante `t`, kg·m².
    /// Motor vazio e propelente como cilindros homogêneos de raio r e comprimento L:
    /// I_axial = m·r²/2, I_transversal = m·(3r² + L²)/12, mais o teorema dos eixos
    /// paralelos (Steiner) até o CG do conjunto.
    #[must_use]
    pub fn inertia(&self, t: f64) -> (f64, f64) {
        let cg = self.cg(t);
        let (r, l) = (0.5 * self.motor.diameter, self.motor.length);
        let cyl = |m: f64, x: f64| {
            (
                0.5 * m * r * r,
                m * (3.0 * r * r + l * l) / 12.0 + m * (x - cg).powi(2),
            )
        };
        let case = cyl(
            self.motor.dry_mass,
            self.nozzle_station - self.case_cg_from_nozzle,
        );
        let prop = cyl(
            self.motor.prop_mass(t),
            self.nozzle_station - self.prop_cg_from_nozzle,
        );
        (
            self.dry_inertia.0 + case.0 + prop.0,
            self.dry_inertia.1 + self.dry_mass * (self.dry_cg - cg).powi(2) + case.1 + prop.1,
        )
    }

    /// Margem estática em calibres no instante `t` e Mach `mach` (CP de Barrowman).
    #[must_use]
    pub fn static_margin(&self, t: f64, mach: f64) -> f64 {
        (self.aero.normal_force(mach, 0.0).x_cp - self.cg(t)) / self.geometry().ref_diameter()
    }

    /// Coeficiente de arrasto em α = 0.
    #[must_use]
    pub fn cd(&self, mach: f64, reynolds_per_m: f64, burning: bool) -> f64 {
        match &self.drag {
            DragModel::Analytic => self
                .aero
                .drag(&FlowState {
                    mach,
                    reynolds_per_m,
                    burning,
                })
                .total(),
            DragModel::Table {
                power_off,
                power_on,
            } => interp(if burning { power_on } else { power_off }, mach),
        }
    }

    fn cd_ctx(&self, c: &FlightCtx<'_>) -> f64 {
        let re = if c.atmo.viscosity > 0.0 {
            c.atmo.density * c.speed / c.atmo.viscosity
        } else {
            0.0
        };
        self.cd(c.mach, re, c.burning)
    }

    /// Área de arrasto somada dos paraquedas já disparados, m².
    #[must_use]
    pub fn chute_area(&self, t: f64, fired: &[Option<f64>]) -> f64 {
        self.parachutes
            .iter()
            .zip(fired)
            .filter_map(|(p, f)| f.map(|tf| p.drag_area(t - tf)))
            .sum()
    }
}

impl Sequencer for Rocket {
    fn burnout_time(&self) -> f64 {
        self.motor.burn_time()
    }
    fn breakpoints(&self) -> Vec<f64> {
        self.motor.breakpoints()
    }
    fn triggers(&self) -> Vec<Trigger> {
        self.parachutes.iter().map(|p| p.trigger).collect()
    }
}

impl PointVehicle for Rocket {
    fn thrust(&self, c: &FlightCtx<'_>) -> (f64, f64) {
        (
            self.motor.thrust(c.t, c.atmo.pressure),
            self.motor.mdot(c.t),
        )
    }
    fn drag_area(&self, c: &FlightCtx<'_>) -> f64 {
        self.cd_ctx(c) * self.geometry().ref_area() + self.chute_area(c.t, c.fired)
    }
}

impl RigidVehicle for Rocket {
    /// Forças no corpo (x para o nariz):
    /// - empuxo ao longo de +x;
    /// - arrasto axial −q·A·Cd0·(u/V) (o Cd de α = 0 projetado no eixo);
    /// - força normal q·A·CN(α) oposta à componente lateral da velocidade, aplicada no
    ///   CP, o que gera o momento restaurador em torno do CG;
    /// - amortecimento de arfagem −q·A·Σ CNαᵢ(xᵢ − x_cg)²·ω⊥/V.
    ///
    /// Com paraquedas aberto, o conjunto é tratado como ponto: arrasto do velame e do
    /// corpo oposto à velocidade, sem momentos (a atitude deixa de ser representativa).
    fn loads(&self, c: &FlightCtx<'_>, v: Vec3, w: Vec3) -> Loads {
        let (thrust, mdot) = if c.burning {
            (
                self.motor.thrust(c.t, c.atmo.pressure),
                self.motor.mdot(c.t),
            )
        } else {
            (0.0, 0.0)
        };
        let mut force = Vec3::new(thrust, 0.0, 0.0);
        let mut moment = Vec3::ZERO;
        if c.speed > 1e-6 && c.q_dyn > 0.0 {
            let a_ref = self.geometry().ref_area();
            let qa = c.q_dyn * a_ref;
            let cd = self.cd_ctx(c);
            if c.fired.iter().any(Option::is_some) {
                force -= (c.q_dyn * (cd * a_ref + self.chute_area(c.t, c.fired)) / c.speed) * v;
            } else {
                force.x -= qa * cd * v.x / c.speed;
                let lateral = v.y.hypot(v.z);
                if lateral > 1e-9 {
                    let nf = self.aero.normal_force(c.mach, lateral.atan2(v.x));
                    let f = -qa * nf.cn / lateral;
                    let (fy, fz) = (f * v.y, f * v.z);
                    force.y += fy;
                    force.z += fz;
                    let arm = nf.x_cp - self.cg(c.t);
                    moment += Vec3::new(0.0, arm * fz, -arm * fy);
                }
                let damp = qa * self.aero.pitch_damping_sum(c.mach, self.cg(c.t)) / c.speed;
                moment -= Vec3::new(0.0, damp * w.y, damp * w.z);
            }
        }
        Loads {
            force_body: force,
            moment_body: moment,
            mdot,
        }
    }

    fn mass_props(&self, t: f64, _mass: f64) -> MassProps {
        let (ia, it) = self.inertia(t);
        // Derivada por diferença central; fora da queima a inércia é constante.
        let rate = if t > 0.0 && t < self.motor.burn_time() {
            let h = 1e-4;
            let (a0, t0) = self.inertia(t - h);
            let (a1, t1) = self.inertia(t + h);
            ((a1 - a0) / (2.0 * h), (t1 - t0) / (2.0 * h))
        } else {
            (0.0, 0.0)
        };
        MassProps {
            inertia: Mat3::diag(ia, it, it),
            inertia_rate: Mat3::diag(rate.0, rate.1, rate.1),
        }
    }
}
