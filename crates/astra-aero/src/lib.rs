//! Aerodinâmica do ASTRA: geometria externa do foguete, força normal e centro de
//! pressão pelo método de Barrowman estendido, e coeficiente de arrasto por componentes.
//!
//! Estações (`x`) são medidas a partir da ponta do nariz, positivas para trás, em metros.
//! Coeficientes usam como referência a área da maior seção do corpo.

mod drag;
mod geometry;
mod stability;

pub use drag::{DragBreakdown, FlowState};
pub use geometry::{BodyPart, FinPlanform, FinProfile, FinSet, Geometry, NoseShape};
pub use stability::NormalForce;

/// Geometria com as integrais de forma já calculadas, para uso repetido durante o voo.
#[derive(Clone, Debug)]
pub struct Aero {
    geometry: Geometry,
    ints: Vec<geometry::PartIntegrals>,
}

impl Aero {
    /// Prepara a geometria.
    #[must_use]
    pub fn new(geometry: Geometry) -> Self {
        let ints = geometry.integrals();
        Self { geometry, ints }
    }

    /// Geometria de origem.
    #[must_use]
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    /// Ver [`Geometry::normal_force`].
    #[must_use]
    pub fn normal_force(&self, mach: f64, alpha: f64) -> NormalForce {
        self.geometry.normal_with(&self.ints, mach, alpha)
    }

    /// Ver [`Geometry::drag`].
    #[must_use]
    pub fn drag(&self, flow: &FlowState) -> DragBreakdown {
        self.geometry.drag_with(&self.ints, flow)
    }

    /// Ver [`Geometry::pitch_damping_sum`].
    #[must_use]
    pub fn pitch_damping_sum(&self, mach: f64, x_cg: f64) -> f64 {
        self.geometry
            .linear_with(&self.ints, mach)
            .iter()
            .map(|c| c.0 * (c.1 - x_cg).powi(2))
            .sum()
    }
}
