//! Missões de lançadores multiestágio: veículo, guiagem, simulação até a órbita,
//! estágios descartados, elementos orbitais e orçamento de Δv.

mod orbit;
mod request;
mod sim;
pub mod templates;
mod vehicle;

pub use orbit::{elements, OrbitElements};
pub use request::{EngineOutRequest, MissionRequest};
pub use sim::{simulate, DvBudget, MissionEvent, MissionOutput, Track};
pub use vehicle::{Booster, EngineOut, Fairing, LaunchVehicle, Mission, PitchKick, Stage};
