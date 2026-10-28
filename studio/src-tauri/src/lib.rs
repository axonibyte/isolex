//! isolex studio's desktop shell: a window around the interface
//! (`studio/ui`), and the commands it calls, each a thin layer over
//! `isolex-studio-core`.

use isolex_studio_core::About;

/// This build's versions, the studio's and the compiler's.
#[tauri::command]
fn about() -> About {
    isolex_studio_core::about()
}

/// Opens the window and serves the interface's commands until it closes.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![about])
        .run(tauri::generate_context!())
        .expect("isolex studio could not start");
}

#[cfg(test)]
mod tests {
    #[test]
    fn about_names_the_compiler_it_is_built_with() {
        let about = super::about();
        assert_eq!(about, isolex_studio_core::about());
        let json = serde_json::to_value(&about).unwrap();
        assert!(json["isolex"].as_str().is_some_and(|v| !v.is_empty()));
    }
}
