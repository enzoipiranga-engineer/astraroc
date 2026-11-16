//! ASTRA para computador: abre a interface web empacotada numa janela nativa (Tauri).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("não foi possível abrir o ASTRA");
}
