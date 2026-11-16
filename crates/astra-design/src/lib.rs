//! Gêmeo digital do foguete.
//!
//! Um [`Design`] é o documento de projeto (JSON versionado): peças com material e
//! posição, juntas entre peças, motor, local de lançamento. A partir dele:
//! - [`Assembly`] resolve massas, CG, inércia, geometria aerodinâmica e capacidades das
//!   juntas;
//! - [`validate`] lista problemas de montagem e de resistência, com explicação e sugestão;
//! - [`simulate`] voa o foguete com falhas físicas: junta que passa do limite quebra, o
//!   corpo se divide e cada parte segue a própria trajetória.

mod assembly;
mod checks;
pub mod elec;
mod limits;
mod material;
mod model;
mod props;
mod sim;
pub mod templates;

pub use assembly::{Assembly, Cut, LoadMode};
pub use checks::{validate, Issue, Severity, Validation};
pub use material::{Material, MaterialDb, Reliability};
pub use model::*;
pub use props::PartProps;
pub use sim::{simulate, BodyTrack, FlightEvent, SimOutput};
