//! isolex studio's desktop shell: a window around the interface
//! (`studio/ui`), and the commands it calls, each a thin layer over
//! `isolex-studio-core`'s [`Session`], whose notices reach the window as
//! `isolex` events.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use isolex_studio_core::compile::Drawing;
use isolex_studio_core::outline::Outline;
use isolex_studio_core::session::{Answer, Ask, FontImport, Notice, Session, Sink, Source};
use isolex_studio_core::About;
use tauri::{AppHandle, Emitter, Manager, State};

/// The event every notice is sent as.
pub const EVENT: &str = "isolex";

/// The library open in the window, if any.
#[derive(Default)]
struct Open(Mutex<Option<Session>>);

/// Notices, to the window.
struct Window(AppHandle);

impl Sink for Window {
    fn notify(&self, notice: Notice) {
        let _ = self.0.emit(EVENT, notice);
    }
}

/// Runs `f` on the open library, or says none is open.
fn with<T>(open: &State<Open>, f: impl FnOnce(&mut Session) -> T) -> Result<T, String> {
    let mut guard = open
        .0
        .lock()
        .map_err(|_| "the library is unavailable".to_string())?;
    let session = guard
        .as_mut()
        .ok_or_else(|| "no library is open".to_string())?;
    Ok(f(session))
}

/// This build's versions, the studio's and the compiler's.
#[tauri::command]
fn about() -> About {
    isolex_studio_core::about()
}

/// Opens the library folder at `path`, closing any other.
#[tauri::command]
fn open(app: AppHandle, open: State<Open>, path: String) -> Result<(), String> {
    let session =
        Session::open(&path, Arc::new(Window(app))).map_err(|e| format!("{path}: {e}"))?;
    *open
        .0
        .lock()
        .map_err(|_| "the library is unavailable".to_string())? = Some(session);
    Ok(())
}

/// What the interface shows: the definition and icon open, and the
/// thumbnails on screen.
#[tauri::command]
fn view(
    open: State<Open>,
    editing: Option<String>,
    icon: Option<String>,
    visible: Vec<String>,
) -> Result<(), String> {
    with(&open, |s| s.view(editing, icon, visible))
}

/// A file's text now, for the Source tab.
#[tauri::command]
fn source(open: State<Open>, file: String) -> Result<Option<Source>, String> {
    with(&open, |s| s.source(&file))
}

/// A definition's parts, for the Parts panel.
#[tauri::command]
fn outline(open: State<Open>, definition: String) -> Result<Option<Outline>, String> {
    with(&open, |s| s.outline(&definition))
}

/// The icon drawn with other params, or none if it does not compile so.
#[tauri::command]
fn preview(
    open: State<Open>,
    icon: String,
    params: BTreeMap<String, f64>,
) -> Result<Option<Drawing>, String> {
    with(&open, |s| s.preview(&icon, &params).ok())
}

/// An icon the interface writes, compiled with the library: how a font is
/// shown. None if it does not compile.
#[tauri::command]
fn sketch(open: State<Open>, source: String) -> Result<Option<Drawing>, String> {
    with(&open, |s| s.sketch(&source).ok())
}

/// A font file read into the library, one undo step.
#[tauri::command]
fn import_font(open: State<Open>, font: FontImport) -> Result<Answer, String> {
    with(&open, |s| s.import_font(&font))
}

/// A font written as TrueType, or a UFO, at `out`.
#[tauri::command]
fn export_font(
    open: State<Open>,
    font: String,
    styles: Vec<String>,
    out: String,
    ufo: bool,
) -> Result<String, String> {
    with(&open, |s| s.export_font(&font, &styles, &out, ufo))?
}

#[tauri::command]
fn ask(open: State<Open>, ask: Ask, label: String) -> Result<Answer, String> {
    with(&open, |s| s.ask(&ask, &label))
}

#[tauri::command]
fn choose(open: State<Open>, id: u64, option: usize) -> Result<Answer, String> {
    with(&open, |s| s.choose(id, option))
}

#[tauri::command]
fn begin_gesture(open: State<Open>) -> Result<(), String> {
    with(&open, |s| s.begin_gesture())
}

#[tauri::command]
fn end_gesture(open: State<Open>) -> Result<(), String> {
    with(&open, |s| s.end_gesture())
}

#[tauri::command]
fn undo(open: State<Open>) -> Result<Option<String>, String> {
    with(&open, |s| s.undo())
}

#[tauri::command]
fn redo(open: State<Open>) -> Result<Option<String>, String> {
    with(&open, |s| s.redo())
}

#[tauri::command]
fn save(open: State<Open>) -> Result<(), String> {
    with(&open, |s| s.save().map_err(|e| e.to_string()))?
}

#[tauri::command]
fn reload(open: State<Open>) -> Result<(), String> {
    with(&open, |s| s.reload().map_err(|e| e.to_string()))?
}

/// Opens the window and serves the interface's commands until it closes;
/// the folder is looked at for outside changes every second.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Open::default())
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let open = handle.state::<Open>();
                let Ok(mut guard) = open.0.lock() else {
                    continue;
                };
                if let Some(s) = guard.as_mut() {
                    let _ = s.tick();
                }
                drop(guard);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            about,
            open,
            view,
            source,
            outline,
            preview,
            sketch,
            import_font,
            export_font,
            ask,
            choose,
            begin_gesture,
            end_gesture,
            undo,
            redo,
            save,
            reload
        ])
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
