//! Foguete montado: junta geometria aerodinâmica, massas, motor e paraquedas e
//! implementa as interfaces de voo 3-DOF e 6-DOF.

mod recovery;
mod rocket;
mod summary;
pub mod templates;

pub use recovery::{Parachute, Reef};
pub use rocket::{DragModel, Rocket};
pub use summary::{optimal_water_volume, FlightSummary};
