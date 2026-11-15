use core::f64::consts::PI;

/// Perfil longitudinal do nariz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoseShape {
    /// Cone.
    Conical,
    /// Ogiva tangente (arco de círculo tangente ao corpo).
    TangentOgive,
    /// Meia elipse.
    Elliptical,
    /// Parábola tangente ao corpo (K = 1).
    Parabolic,
    /// Série de Haack com C = 0 (LD-Haack, "von Kármán"): arrasto mínimo para
    /// comprimento e diâmetro dados.
    VonKarman,
    /// Série de Haack com C = 1/3 (LV-Haack): arrasto mínimo para comprimento e volume dados.
    LvHaack,
}

impl NoseShape {
    /// Raio adimensional r/R na posição adimensional ξ = x/L ∈ [0, 1], para um nariz
    /// de razão de esbeltez `fineness` = L/(2R) (só a ogiva depende dela).
    /// Fonte: Crowell, "The Descriptive Geometry of Nose Cones" (1996).
    #[must_use]
    pub fn profile(self, xi: f64, fineness: f64) -> f64 {
        let xi = xi.clamp(0.0, 1.0);
        match self {
            Self::Conical => xi,
            Self::TangentOgive => {
                // Em unidades de R: L = 2·fineness, ρ = (1 + L²)/2.
                let l = 2.0 * fineness;
                let rho = 0.5 * (1.0 + l * l);
                (rho * rho - (l * (1.0 - xi)).powi(2)).sqrt() + 1.0 - rho
            }
            Self::Elliptical => (1.0 - (1.0 - xi).powi(2)).sqrt(),
            Self::Parabolic => 2.0 * xi - xi * xi,
            Self::VonKarman | Self::LvHaack => {
                let c = if self == Self::LvHaack {
                    1.0 / 3.0
                } else {
                    0.0
                };
                let th = (1.0 - 2.0 * xi).acos();
                ((th - 0.5 * (2.0 * th).sin() + c * th.sin().powi(3)) / PI)
                    .max(0.0)
                    .sqrt()
            }
        }
    }
}

/// Trecho do corpo, da frente para trás.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyPart {
    /// Nariz (coifa).
    Nose {
        /// Perfil.
        shape: NoseShape,
        /// Comprimento, m.
        length: f64,
        /// Diâmetro da base, m.
        diameter: f64,
    },
    /// Tubo cilíndrico.
    Tube {
        /// Comprimento, m.
        length: f64,
        /// Diâmetro externo, m.
        diameter: f64,
    },
    /// Transição cônica (ombro ou boattail).
    Transition {
        /// Comprimento, m.
        length: f64,
        /// Diâmetro dianteiro, m.
        d_fore: f64,
        /// Diâmetro traseiro, m.
        d_aft: f64,
    },
}

impl BodyPart {
    /// Comprimento, m.
    #[must_use]
    pub fn length(&self) -> f64 {
        match *self {
            Self::Nose { length, .. }
            | Self::Tube { length, .. }
            | Self::Transition { length, .. } => length,
        }
    }

    /// Diâmetros dianteiro e traseiro, m.
    #[must_use]
    pub fn diameters(&self) -> (f64, f64) {
        match *self {
            Self::Nose { diameter, .. } => (0.0, diameter),
            Self::Tube { diameter, .. } => (diameter, diameter),
            Self::Transition { d_fore, d_aft, .. } => (d_fore, d_aft),
        }
    }

    /// Raio externo na posição local `x` (m a partir da frente do trecho).
    #[must_use]
    pub fn radius_at(&self, x: f64) -> f64 {
        let xi = if self.length() > 0.0 {
            (x / self.length()).clamp(0.0, 1.0)
        } else {
            1.0
        };
        match *self {
            Self::Nose {
                shape,
                length,
                diameter,
            } => 0.5 * diameter * shape.profile(xi, length / diameter),
            Self::Tube { diameter, .. } => 0.5 * diameter,
            Self::Transition { d_fore, d_aft, .. } => 0.5 * (d_fore + (d_aft - d_fore) * xi),
        }
    }

    /// Integrais geométricas do trecho por soma de troncos de cone.
    pub(crate) fn integrals(&self) -> PartIntegrals {
        const N: usize = 400;
        let l = self.length();
        let h = l / N as f64;
        let mut out = PartIntegrals::default();
        for i in 0..N {
            let (x0, x1) = (i as f64 * h, (i + 1) as f64 * h);
            let (r0, r1) = (self.radius_at(x0), self.radius_at(x1));
            out.volume += PI * h * (r0 * r0 + r0 * r1 + r1 * r1) / 3.0;
            out.wetted += PI * (r0 + r1) * (h * h + (r1 - r0).powi(2)).sqrt();
            let a = (r0 + r1) * h;
            out.planform += a;
            // Centroide do trapézio de alturas 2r0 e 2r1.
            let xc = if r0 + r1 > 0.0 {
                x0 + h * (r0 + 2.0 * r1) / (3.0 * (r0 + r1))
            } else {
                x0 + 0.5 * h
            };
            out.planform_moment += a * xc;
        }
        out
    }
}

/// Volume (m³), área molhada lateral (m²), área em planta (m²) e momento estático da
/// área em planta em relação à frente do trecho (m³).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PartIntegrals {
    pub volume: f64,
    pub wetted: f64,
    pub planform: f64,
    pub planform_moment: f64,
}

/// Forma em planta de uma aleta.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FinPlanform {
    /// Trapézio.
    Trapezoid {
        /// Corda na raiz, m.
        root_chord: f64,
        /// Corda na ponta, m.
        tip_chord: f64,
        /// Recuo do bordo de ataque da ponta em relação ao da raiz, m.
        sweep: f64,
    },
    /// Meia elipse com o eixo maior na raiz.
    Ellipse {
        /// Corda na raiz, m.
        root_chord: f64,
    },
}

/// Seção transversal da aleta (afeta só o arrasto de pressão).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinProfile {
    /// Bordos retos.
    Square,
    /// Bordos arredondados.
    Rounded,
    /// Perfil aerodinâmico (bordo de ataque arredondado, bordo de fuga afilado).
    Airfoil,
}

/// Conjunto de aletas idênticas, igualmente espaçadas em torno do corpo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinSet {
    /// Número de aletas.
    pub count: u32,
    /// Forma em planta.
    pub planform: FinPlanform,
    /// Envergadura de uma aleta (da raiz à ponta), m.
    pub span: f64,
    /// Espessura, m.
    pub thickness: f64,
    /// Estação do bordo de ataque na raiz, m a partir da ponta do nariz.
    pub position: f64,
    /// Seção transversal.
    pub profile: FinProfile,
}

impl FinSet {
    /// Área em planta de uma aleta, m².
    #[must_use]
    pub fn area(&self) -> f64 {
        match self.planform {
            FinPlanform::Trapezoid {
                root_chord,
                tip_chord,
                ..
            } => 0.5 * (root_chord + tip_chord) * self.span,
            FinPlanform::Ellipse { root_chord } => 0.25 * PI * root_chord * self.span,
        }
    }

    /// Corda na raiz, m.
    #[must_use]
    pub fn root_chord(&self) -> f64 {
        match self.planform {
            FinPlanform::Trapezoid { root_chord, .. } | FinPlanform::Ellipse { root_chord } => {
                root_chord
            }
        }
    }

    /// Corda média aerodinâmica (m) e recuo do bordo de ataque dela em relação ao da
    /// raiz (m). Trapézio: MAC = (2/3)(cr + ct − cr·ct/(cr + ct)),
    /// x_MAC = xs·(cr + 2ct)/(3(cr + ct)). Elipse: MAC = 8cr/(3π), centrada na raiz.
    /// Fonte: Raymer, "Aircraft Design: A Conceptual Approach", 6ª ed., cap. 7.
    #[must_use]
    pub fn mac(&self) -> (f64, f64) {
        match self.planform {
            FinPlanform::Trapezoid {
                root_chord: cr,
                tip_chord: ct,
                sweep,
            } => (
                2.0 / 3.0 * (cr + ct - cr * ct / (cr + ct)),
                sweep * (cr + 2.0 * ct) / (3.0 * (cr + ct)),
            ),
            FinPlanform::Ellipse { root_chord: cr } => {
                let mac = 8.0 * cr / (3.0 * PI);
                (mac, 0.5 * (cr - mac))
            }
        }
    }

    /// Cosseno do enflechamento da linha de meia corda.
    #[must_use]
    pub fn cos_mid_sweep(&self) -> f64 {
        match self.planform {
            FinPlanform::Trapezoid {
                root_chord,
                tip_chord,
                sweep,
            } => {
                let dx = sweep + 0.5 * tip_chord - 0.5 * root_chord;
                self.span / self.span.hypot(dx)
            }
            FinPlanform::Ellipse { .. } => 1.0,
        }
    }

    /// Cosseno do enflechamento do bordo de ataque.
    #[must_use]
    pub fn cos_le_sweep(&self) -> f64 {
        match self.planform {
            FinPlanform::Trapezoid { sweep, .. } => self.span / self.span.hypot(sweep),
            FinPlanform::Ellipse { .. } => 1.0,
        }
    }
}

/// Geometria externa completa.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// Trechos do corpo, do nariz para trás.
    pub parts: Vec<BodyPart>,
    /// Conjuntos de aletas.
    pub fins: Vec<FinSet>,
    /// Rugosidade superficial (altura média), m. Referências: pintura lisa ~5e-6,
    /// pintura comum ~2e-5 a 6e-5, papelão cru ~1,5e-4 (Hoerner, "Fluid-Dynamic Drag", cap. 5).
    pub roughness: f64,
    /// Diâmetro de saída da tubeira do motor, m (reduz o arrasto de base durante a queima).
    pub nozzle_exit_diameter: f64,
}

impl Geometry {
    /// Comprimento total do corpo, m.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.parts.iter().map(BodyPart::length).sum()
    }

    /// Diâmetro de referência (maior diâmetro do corpo), m.
    #[must_use]
    pub fn ref_diameter(&self) -> f64 {
        self.parts
            .iter()
            .map(|p| p.diameters().0.max(p.diameters().1))
            .fold(0.0, f64::max)
    }

    /// Área de referência, m².
    #[must_use]
    pub fn ref_area(&self) -> f64 {
        0.25 * PI * self.ref_diameter().powi(2)
    }

    /// Trechos com a estação inicial de cada um.
    pub(crate) fn stations(&self) -> impl Iterator<Item = (f64, &BodyPart)> {
        self.parts.iter().scan(0.0, |x, p| {
            let x0 = *x;
            *x += p.length();
            Some((x0, p))
        })
    }

    pub(crate) fn integrals(&self) -> Vec<PartIntegrals> {
        self.parts.iter().map(BodyPart::integrals).collect()
    }

    /// Raio do corpo na estação `x`, m.
    #[must_use]
    pub fn body_radius_at(&self, x: f64) -> f64 {
        let mut last = 0.0;
        for (x0, p) in self.stations() {
            if x <= x0 + p.length() {
                return p.radius_at(x - x0);
            }
            last = p.diameters().1 * 0.5;
        }
        last
    }
}
