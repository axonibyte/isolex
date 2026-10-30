//! isolex studio's core as a WebAssembly module, for the studio's
//! interface outside the app (a plain browser, its tests): a library kept
//! in memory, edited, undone and compiled as in the app, without a disk,
//! threads or a clock. Build it with
//!
//! ```sh
//! cargo build --release --locked -p isolex-studio-wasm --target wasm32-unknown-unknown
//! ```
//!
//! It imports nothing. Memory changes hands as with `isolex-wasm`:
//!
//! - `studio_alloc(len) -> ptr` reserves `len` bytes for the host to fill;
//! - `studio_call(ptr, len) -> u64` reads that many bytes of a call as JSON
//!   (see [`call`]), frees them, and returns the reply's JSON as
//!   `ptr << 32 | len`;
//! - `studio_free(ptr, len)` frees what `studio_call` returned.
//!
//! A panic traps.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use isolex_studio_core::session::{Ask, Notice, Session, Sink};
use isolex_studio_core::watch::DiskChange;
use isolex_studio_core::Library;
use serde_json::{json, Value};

/// The notices a call makes, kept until it returns.
#[derive(Default)]
struct Heard(Mutex<Vec<Notice>>);

impl Sink for Heard {
    fn notify(&self, notice: Notice) {
        self.0.lock().expect("the notices").push(notice);
    }
}

thread_local! {
    static OPEN: RefCell<Option<(Session, Arc<Heard>)>> = const { RefCell::new(None) };
}

/// One call, as the shell's commands are: `{"call": "ask", "ask": …,
/// "label": …}`. The reply is `{"result": …, "notices": […]}`, with every
/// notice the call made, or `{"error": …}`.
///
/// Calls: `open` (`files`: `[[path, text], …]`), `view` (`editing`,
/// `icon`, `visible`), `source` (`file`), `outline` (`definition`), `ask`
/// (`ask`, `label`), `choose` (`id`, `option`), `begin_gesture`,
/// `end_gesture`, `undo`, `redo`, `save` and `about`.
pub fn call(input: &str) -> String {
    let reply = match serde_json::from_str::<Value>(input) {
        Ok(v) => answer(&v),
        Err(e) => Err(format!("not JSON: {e}")),
    };
    match reply {
        Ok(v) => v.to_string(),
        Err(e) => json!({ "error": e }).to_string(),
    }
}

fn answer(v: &Value) -> Result<Value, String> {
    let s = |key: &str| v[key].as_str().map(str::to_string);
    let name = s("call").ok_or("no call")?;
    if name == "about" {
        let a = isolex_studio_core::about();
        return Ok(json!({ "result": a, "notices": [] }));
    }
    if name == "open" {
        let files = serde_json::from_value::<Vec<(String, String)>>(v["files"].clone())
            .map_err(|e| format!("open: {e}"))?;
        let heard = Arc::new(Heard::default());
        let session =
            Session::in_memory(Library::new("", files.into_iter().collect()), heard.clone());
        OPEN.with(|o| *o.borrow_mut() = Some((session, heard)));
        return Ok(json!({ "result": null, "notices": drain() }));
    }
    // nothing open, nothing to attach to
    if name == "attach" && OPEN.with(|o| o.borrow().is_none()) {
        return Ok(json!({ "result": null, "notices": [] }));
    }
    let result = OPEN.with(|o| -> Result<Value, String> {
        let mut o = o.borrow_mut();
        let (session, _) = o.as_mut().ok_or("no library open")?;
        let to = |r: Result<Value, serde_json::Error>| r.map_err(|e| e.to_string());
        match name.as_str() {
            // a second window, opened on the library open (#119)
            "attach" => to(serde_json::to_value(session.current())),
            "view" => {
                let visible = serde_json::from_value(v["visible"].clone()).unwrap_or_default();
                session.view(s("editing"), s("icon"), visible);
                Ok(Value::Null)
            }
            "source" => to(serde_json::to_value(
                session.source(&s("file").unwrap_or_default()),
            )),
            "outline" => to(serde_json::to_value(
                session.outline(&s("definition").unwrap_or_default()),
            )),
            "ask" => {
                let ask: Ask =
                    serde_json::from_value(v["ask"].clone()).map_err(|e| format!("ask: {e}"))?;
                to(serde_json::to_value(session.ask_in(
                    &ask,
                    &s("label").unwrap_or_default(),
                    s("editing").as_deref(),
                )))
            }
            "choose" => {
                let id = v["id"].as_u64().ok_or("choose: no id")?;
                let option = v["option"].as_u64().ok_or("choose: no option")? as usize;
                to(serde_json::to_value(session.choose(id, option)))
            }
            "begin_gesture" => {
                session.begin_gesture();
                Ok(Value::Null)
            }
            "end_gesture" => {
                session.end_gesture();
                Ok(Value::Null)
            }
            "preview" => {
                let params = serde_json::from_value(v["params"].clone())
                    .map_err(|e| format!("preview: {e}"))?;
                to(serde_json::to_value(
                    session
                        .preview(&s("icon").unwrap_or_default(), &params)
                        .ok(),
                ))
            }
            "sketch" => to(serde_json::to_value(
                session.sketch(&s("source").unwrap_or_default()).ok(),
            )),
            "library_op" => {
                let op = serde_json::from_value(v["op"].clone())
                    .map_err(|e| format!("library_op: {e}"))?;
                to(serde_json::to_value(session.library_op(&op)))
            }
            "lexicon" => to(serde_json::to_value(session.lexicon())),
            // exported: the interface hands these to the browser to save
            "export_svgs" => {
                let icons: Option<Vec<String>> = serde_json::from_value(v["icons"].clone())
                    .map_err(|e| format!("export_svgs: {e}"))?;
                let ground = serde_json::from_value(v["ground"].clone())
                    .map_err(|e| format!("export_svgs: {e}"))?;
                to(serde_json::to_value(
                    session.export_svgs(icons.as_deref(), ground)?,
                ))
            }
            "export_ir" => {
                let icons: Option<Vec<String>> = serde_json::from_value(v["icons"].clone())
                    .map_err(|e| format!("export_ir: {e}"))?;
                Ok(json!(session.export_ir(icons.as_deref())?))
            }
            "export_zip" => Ok(json!(session.export_zip())),
            // a file changed "on disk": the library's in memory
            "outside" => {
                let changes: Vec<(String, Option<String>)> =
                    serde_json::from_value(v["changes"].clone())
                        .map_err(|e| format!("outside: {e}"))?;
                session.outside(
                    changes
                        .into_iter()
                        .map(|(p, t)| match t {
                            Some(t) => DiskChange::Written(p, t),
                            None => DiskChange::Removed(p),
                        })
                        .collect(),
                );
                Ok(Value::Null)
            }
            "undo" => Ok(json!(session.undo())),
            "redo" => Ok(json!(session.redo())),
            "reload" => session
                .reload()
                .map(|()| Value::Null)
                .map_err(|e| e.to_string()),
            "save" => session
                .save()
                .map(|()| Value::Null)
                .map_err(|e| e.to_string()),
            other => Err(format!("no call {other}")),
        }
    })?;
    Ok(json!({ "result": result, "notices": drain() }))
}

/// The notices made since last asked, as JSON.
fn drain() -> Value {
    OPEN.with(|o| match &*o.borrow() {
        Some((_, heard)) => {
            let notices = std::mem::take(&mut *heard.0.lock().expect("the notices"));
            serde_json::to_value(notices).expect("notices serialize")
        }
        None => json!([]),
    })
}

#[no_mangle]
pub extern "C" fn studio_alloc(len: u32) -> u32 {
    Box::into_raw(vec![0u8; len as usize].into_boxed_slice()) as *mut u8 as u32
}

/// # Safety
/// `ptr` and `len` must be exactly what `studio_alloc` or `studio_call`
/// handed out, and not already freed.
#[no_mangle]
pub unsafe extern "C" fn studio_free(ptr: u32, len: u32) {
    drop(take(ptr, len));
}

/// # Safety
/// `ptr` must come from `studio_alloc(len)` and hold `len` bytes the host
/// wrote; it is freed here.
#[no_mangle]
pub unsafe extern "C" fn studio_call(ptr: u32, len: u32) -> u64 {
    let input = take(ptr, len);
    let out = call(&String::from_utf8_lossy(&input));
    drop(input);
    let bytes = out.into_bytes().into_boxed_slice();
    let len = bytes.len() as u64;
    let ptr = Box::into_raw(bytes) as *mut u8 as u64;
    (ptr << 32) | len
}

/// Ownership of a buffer handed out by `studio_alloc` or `studio_call`,
/// back.
unsafe fn take(ptr: u32, len: u32) -> Box<[u8]> {
    Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        ptr as *mut u8,
        len as usize,
    ))
}
