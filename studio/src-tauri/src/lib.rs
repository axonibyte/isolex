//! isolex studio's desktop shell: a window around the interface
//! (`studio/ui`), and the commands it calls, each a thin layer over
//! `isolex-studio-core`'s [`Session`], whose notices reach the window as
//! `isolex` events.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use isolex_studio_core::compile::Drawing;
use isolex_studio_core::export::{Ground, Svg};
use isolex_studio_core::lexicon::LexiconRows;
use isolex_studio_core::library::Start;
use isolex_studio_core::outline::Outline;
use isolex_studio_core::session::{
    Answer, Ask, FontImport, LibraryOp, Notice, Session, Sink, Source,
};
use isolex_studio_core::About;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindowBuilder};

/// The event every notice is sent as.
pub const EVENT: &str = "isolex";

/// The library open in the window, if any.
#[derive(Default)]
struct Open(Mutex<Option<Session>>);

/// Notices, to every window: each shows the one library, and hears every
/// change made in any of them.
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

/// A new library at `path`, empty or a copy of another, opened.
#[tauri::command]
fn new_library(
    app: AppHandle,
    open: State<Open>,
    path: String,
    start: Start,
) -> Result<(), String> {
    isolex_studio_core::library::create(std::path::Path::new(&path), &start)
        .map_err(|e| e.to_string())?;
    self::open(app, open, path)
}

/// A file or definition made, renamed or moved: one undo step.
#[tauri::command]
fn library_op(open: State<Open>, op: LibraryOp) -> Result<Answer, String> {
    with(&open, |s| s.library_op(&op))
}

/// The `lexicon` and `palette` blocks, for Lexicon Properties.
#[tauri::command]
fn lexicon(open: State<Open>) -> Result<LexiconRows, String> {
    with(&open, |s| s.lexicon())
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

/// `icons` (or every icon) as SVG on `ground` (#112), which the interface
/// writes, or draws as PNG.
#[tauri::command]
fn export_svgs(
    open: State<Open>,
    icons: Option<Vec<String>>,
    ground: Ground,
) -> Result<Vec<Svg>, String> {
    with(&open, |s| s.export_svgs(icons.as_deref(), ground))?
}

/// The IR of `icons` (or every icon), as `isolex compile --ir` writes it.
#[tauri::command]
fn export_ir(open: State<Open>, icons: Option<Vec<String>>) -> Result<String, String> {
    with(&open, |s| s.export_ir(icons.as_deref()))?
}

/// The library as it is open, as a zip.
#[tauri::command]
fn export_zip(open: State<Open>) -> Result<Vec<u8>, String> {
    with(&open, |s| s.export_zip())
}

/// A file an export made, written to `out`: only an SVG, PNG, JSON or zip.
#[tauri::command]
fn write_export(out: String, data: Vec<u8>) -> Result<(), String> {
    isolex_studio_core::export::write(std::path::Path::new(&out), &data)
}

/// `ask`, in the definition the asking window edits.
#[tauri::command]
fn ask(
    open: State<Open>,
    ask: Ask,
    label: String,
    editing: Option<String>,
) -> Result<Answer, String> {
    with(&open, |s| s.ask_in(&ask, &label, editing.as_deref()))
}

/// What a window opened on the library open has missed, to start where
/// the others are (#119); none if no library is open.
#[tauri::command]
fn attach(open: State<Open>) -> Result<Option<Vec<Notice>>, String> {
    let guard = open
        .0
        .lock()
        .map_err(|_| "the library is unavailable".to_string())?;
    Ok(guard.as_ref().map(Session::current))
}

/// The windows opened after the first, by label, each with the icon it
/// was opened on until it asks for it.
#[derive(Default)]
struct Windows {
    opened: Mutex<(u32, HashMap<String, Option<String>>)>,
}

/// Every window after the first is labeled so, which the capability's
/// window list matches.
const WINDOW_PREFIX: &str = "studio-";

impl Windows {
    /// A label for another window, which opens on `icon`.
    fn add(&self, icon: Option<String>) -> String {
        let mut opened = self.opened.lock().expect("the windows");
        opened.0 += 1;
        let label = format!("{WINDOW_PREFIX}{}", opened.0 + 1);
        opened.1.insert(label.clone(), icon);
        label
    }

    /// The icon the window `label` was opened on, told once.
    fn take(&self, label: &str) -> Option<String> {
        self.opened
            .lock()
            .expect("the windows")
            .1
            .remove(label)
            .flatten()
    }
}

/// Another window on the library open (#119), showing `icon`: it shares
/// the one library and its undo history. Async, as a window made in a
/// command must be on Windows.
#[tauri::command]
async fn new_window(
    app: AppHandle,
    windows: State<'_, Windows>,
    icon: Option<String>,
) -> Result<(), String> {
    let label = windows.add(icon);
    let window =
        WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App("index.html".into()))
            .title("isolex studio")
            .inner_size(1200.0, 800.0)
            .min_inner_size(900.0, 600.0);
    for_driver(window).build().map_err(|e| e.to_string())?;
    Ok(())
}

/// The icon this window was opened on, if it is not the first.
#[tauri::command]
fn opened_on(window: tauri::WebviewWindow, windows: State<Windows>) -> Option<String> {
    windows.take(window.label())
}

/// A window as a driver debugging the app asks for it: WebView2 told what
/// the driver set (only then: otherwise as wry has it).
fn for_driver<'a, R: tauri::Runtime, M: Manager<R>>(
    window: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    match std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS") {
        Ok(extra) => window.additional_browser_args(&browser_args(WEBVIEW2_DEFAULT_ARGS, &extra)),
        Err(_) => window,
    }
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

/// The library folder the app was started with, if any: `isolex-studio
/// <folder>` opens it, as does ISOLEX_STUDIO_LIBRARY when no folder is
/// given (msedgedriver, which starts the app for its end-to-end test on
/// Windows, gives it no command line of its own).
struct Startup(Option<String>);

/// The first argument that is not a flag and is a folder; else `env`, if
/// it is a folder.
fn startup_folder(args: impl IntoIterator<Item = String>, env: Option<String>) -> Option<String> {
    let is_dir = |a: &String| std::path::Path::new(a).is_dir();
    args.into_iter()
        .find(|a| !a.starts_with('-') && is_dir(a))
        .or(env.filter(is_dir))
}

/// What wry gives WebView2 when the app gives it nothing: giving it
/// anything replaces these.
const WEBVIEW2_DEFAULT_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
     --autoplay-policy=no-user-gesture-required";

/// WebView2's browser arguments: `base`, and `extra` from
/// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, as msedgedriver sets it to
/// debug the app. Some WebView2 runtimes take the app's arguments instead
/// of that variable's, never both, so the app passes both itself; the
/// lists of disabled features are joined, since only one would count.
fn browser_args(base: &str, extra: &str) -> String {
    const DISABLE: &str = "--disable-features=";
    let mut features: Vec<&str> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for arg in base.split_whitespace().chain(extra.split_whitespace()) {
        match arg.strip_prefix(DISABLE) {
            Some(list) => {
                for f in list.split(',').filter(|f| !f.is_empty()) {
                    if !features.contains(&f) {
                        features.push(f);
                    }
                }
            }
            None => rest.push(arg),
        }
    }
    let disable = (!features.is_empty()).then(|| format!("{DISABLE}{}", features.join(",")));
    disable
        .into_iter()
        .chain(rest.into_iter().map(String::from))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The folder the app was started with, which the interface opens first.
#[tauri::command]
fn startup(start: State<Startup>) -> Option<String> {
    start.0.clone()
}

/// Opens the window and serves the interface's commands until it closes;
/// the folder is looked at for outside changes every second.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Open::default())
        .manage(Windows::default())
        .manage(Startup(startup_folder(
            std::env::args().skip(1),
            std::env::var("ISOLEX_STUDIO_LIBRARY").ok(),
        )))
        .setup(|app| {
            // the window, from tauri.conf.json, with WebView2 told what a
            // driver asks of it (only then: otherwise as wry has it)
            let config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .ok_or("tauri.conf.json declares no window")?;
            for_driver(WebviewWindowBuilder::from_config(app.handle(), &config)?).build()?;

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
            startup,
            open,
            new_library,
            library_op,
            lexicon,
            view,
            source,
            outline,
            preview,
            sketch,
            import_font,
            export_font,
            export_svgs,
            export_ir,
            export_zip,
            write_export,
            ask,
            attach,
            new_window,
            opened_on,
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
    /// Every dialog the interface asks for is one its window may open:
    /// without `dialog:allow-save`, a font's *Export…* failed in the app
    /// and nowhere else.
    #[test]
    fn the_window_may_open_every_dialog_the_interface_asks_for() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let granted: Vec<&str> = capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.as_str())
            .collect();
        let backend = include_str!("../../ui/src/lib/backend.ts");
        for (call, permission) in [
            ("dialog.open(", "dialog:allow-open"),
            ("dialog.save(", "dialog:allow-save"),
        ] {
            assert!(
                backend.contains(call),
                "the interface no longer calls {call}"
            );
            assert!(
                granted.contains(&permission),
                "the interface calls {call} and the window is not granted {permission}"
            );
        }
    }

    /// A window after the first is one the capability lets call the
    /// studio's commands, and is told the icon it opened on once.
    #[test]
    fn another_window_may_call_the_studio_and_opens_on_its_icon() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let windows: Vec<&str> = capability["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|w| w.as_str())
            .collect();
        let windows_ = super::Windows::default();
        let second = windows_.add(Some("switch-poe".into()));
        let third = windows_.add(None);
        assert_eq!((second.as_str(), third.as_str()), ("studio-2", "studio-3"));
        assert!(windows.contains(&"main"));
        assert!(
            windows.contains(&format!("{}*", super::WINDOW_PREFIX).as_str()),
            "the capability does not cover {second}: {windows:?}"
        );
        assert_eq!(windows_.take(&second).as_deref(), Some("switch-poe"));
        assert_eq!(windows_.take(&second), None);
        assert_eq!(windows_.take(&third), None);
        assert_eq!(windows_.take("main"), None);
    }

    #[test]
    fn about_names_the_compiler_it_is_built_with() {
        let about = super::about();
        assert_eq!(about, isolex_studio_core::about());
        let json = serde_json::to_value(&about).unwrap();
        assert!(json["isolex"].as_str().is_some_and(|v| !v.is_empty()));
    }

    #[test]
    fn a_folder_on_the_command_line_is_the_library_to_open() {
        let here = env!("CARGO_MANIFEST_DIR").to_string();
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            super::startup_folder(args(&["--flag", &here]), None),
            Some(here.clone())
        );
        assert_eq!(super::startup_folder(args(&["not-a-folder"]), None), None);
        assert_eq!(super::startup_folder(args(&[]), None), None);
    }

    #[test]
    fn without_one_isolex_studio_library_is_the_library_to_open() {
        let here = env!("CARGO_MANIFEST_DIR").to_string();
        let src = format!("{here}/src");
        // the variable, when nothing is given
        assert_eq!(
            super::startup_folder(Vec::new(), Some(here.clone())),
            Some(here.clone())
        );
        // the command line first
        assert_eq!(
            super::startup_folder(vec![src.clone()], Some(here.clone())),
            Some(src)
        );
        // and never a folder that is not there
        assert_eq!(
            super::startup_folder(Vec::new(), Some("not-a-folder".into())),
            None
        );
    }

    #[test]
    fn a_drivers_webview2_arguments_join_the_apps() {
        let joined = super::browser_args(
            super::WEBVIEW2_DEFAULT_ARGS,
            "--enable-automation --disable-features=IgnoreDuplicateNavs,Prewarm --remote-debugging-port=0",
        );
        assert_eq!(
            joined,
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,IgnoreDuplicateNavs,Prewarm \
             --autoplay-policy=no-user-gesture-required --enable-automation --remote-debugging-port=0"
        );
        // nothing extra: the defaults, as wry has them
        assert_eq!(
            super::browser_args(super::WEBVIEW2_DEFAULT_ARGS, ""),
            super::WEBVIEW2_DEFAULT_ARGS
        );
        // a feature named twice is disabled once
        assert_eq!(
            super::browser_args("--disable-features=A,B", "--disable-features=B,C"),
            "--disable-features=A,B,C"
        );
    }
}
