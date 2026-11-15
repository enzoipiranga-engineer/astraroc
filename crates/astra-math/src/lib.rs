//! Base matemática do núcleo ASTRA.
//!
//! Regras do núcleo (valem para todos os crates de física):
//! - unidades SI em todas as interfaces públicas (m, kg, s, rad, K, Pa, N);
//! - `f64` em tudo; nenhuma fonte de não determinismo (sem relógio, sem RNG global,
//!   sem `HashMap` com ordem de iteração observável);
//! - toda equação cita a fonte no comentário.

pub mod constants;
mod mat3;
mod quat;
mod vec3;

pub use mat3::Mat3;
pub use quat::Quat;
pub use vec3::Vec3;
