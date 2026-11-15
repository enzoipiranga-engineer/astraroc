//! Fronteira WebAssembly do núcleo ASTRA.
//!
//! Este crate só traduz tipos entre JS e Rust. Nenhuma equação de física mora aqui.

use astra_math::{constants, Quat, Vec3};
use wasm_bindgen::prelude::*;

/// Versão do núcleo (a do workspace Cargo).
#[wasm_bindgen(js_name = coreVersion)]
#[must_use]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Autoteste numérico executado dentro do runtime WASM do usuário.
///
/// Roda um vetor por um quaternion e desfaz a rotação; devolve o erro absoluto (m)
/// de ida e volta. Serve para confirmar que o núcleo carregado produz aritmética
/// IEEE 754 de dupla precisão como no build nativo (esperado: < 1e-12).
#[wasm_bindgen(js_name = selfCheck)]
#[must_use]
pub fn self_check() -> f64 {
    let q = Quat::from_axis_angle(Vec3::new(1.0, 2.0, -0.5), 1.234);
    let v = Vec3::new(constants::wgs84::A, -4.0e3, 2.2e5);
    (q.conj().rotate(q.rotate(v)) - v).norm()
}

#[cfg(test)]
mod tests {
    #[test]
    fn self_check_is_tight_natively() {
        assert!(super::self_check() < 1e-8);
    }
}
