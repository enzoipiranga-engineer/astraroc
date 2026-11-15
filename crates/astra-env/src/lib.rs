//! Ambiente de voo do ASTRA: planeta (elipsoide, rotação, gravidade), referenciais
//! (geodésico, ECEF, ECI, ENU) e atmosfera.

mod atmosphere;
mod planet;
pub mod time;
mod wind;

pub use atmosphere::{AtmoState, AtmosphereModel};
pub use planet::{rot_z, Geodetic, Planet};
pub use wind::{Turbulence, Wind, WindLayer};
