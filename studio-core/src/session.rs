//! A library open in the studio, as the shell serves it (#66): the commands
//! the interface calls, and the notices it is sent.
//!
//! The shell is a few lines over this: each of its commands calls a method
//! here, and a [`Sink`] carries notices to the window. Tests use a sink of
//! their own in place of the window.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use isolex::ir::IrIcon;
use isolex::kdl::Value;
use serde::{Deserialize, Serialize};

use crate::compile::{Background, Diag, Drawing, Event, Inline, Isolex, Origin};
use crate::edit::{self, Choice, Intent, NodeRef, Plan, Request, Slot};
use crate::library::Library;
use crate::outline::Outline;
use crate::watch::{DiskOutcome, PollingWatcher, Watcher};

/// Where notices go: the window, or a test's record of them.
pub trait Sink: Send + Sync + 'static {
    fn notify(&self, notice: Notice);
}

/// What the interface is told, as it arrives.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Notice {
    /// The library's state after anything changed it.
    Library {
        root: String,
        revision: u64,
        modified: bool,
        files: Vec<String>,
        history: Vec<String>,
        can_undo: bool,
        can_redo: bool,
    },
    /// How its definitions refer to each other.
    Graph { graph: isolex::graph::Graph },
    /// A generation compiled the lexicon: its errors and warnings, and the
    /// icons it emits.
    Compiled {
        generation: u64,
        revision: u64,
        errors: Vec<Diag>,
        warnings: Vec<Diag>,
        /// The lexicon's `palette` block, by token, if it has one.
        palette: Option<BTreeMap<String, isolex::Colors>>,
        /// The tokens and stroke widths its `lexicon` block declares, the
        /// only ones paint is chosen from; none if it declares none.
        tokens: Vec<String>,
        stroke_widths: Vec<f64>,
        icons: Vec<String>,
    },
    /// An icon drawn differently than before, with where each of its
    /// shapes is written.
    Icon {
        name: String,
        ir: IrIcon,
        sources: Vec<Origin>,
    },
    /// An icon that does not compile now, with its last good drawing.
    Broken {
        name: String,
        diagnostics: Vec<Diag>,
        last: Option<Drawing>,
    },
    /// An icon no longer emitted.
    Gone { name: String },
    /// Files changed on disk were loaded.
    Reloaded { files: Vec<String> },
    /// Files changed on disk while there are unsaved edits.
    OutOfSync { files: Vec<String> },
}

/// A value from the interface, as JSON gives it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonValue {
    Bool(bool),
    Number(f64),
    Text(String),
    Null,
}

impl From<&JsonValue> for Value {
    fn from(v: &JsonValue) -> Value {
        match v {
            JsonValue::Bool(b) => Value::Bool(*b),
            JsonValue::Number(n) => Value::Float(*n),
            JsonValue::Text(t) => Value::String(t.clone()),
            JsonValue::Null => Value::Null,
        }
    }
}

/// A node, as the interface names it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub file: String,
    pub start: usize,
    pub len: usize,
}

impl From<&Node> for NodeRef {
    fn from(n: &Node) -> NodeRef {
        NodeRef {
            file: n.file.clone(),
            start: n.start,
            len: n.len,
        }
    }
}

/// What the interface asks for; see [`edit::Intent`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Ask {
    SetArg {
        node: Node,
        index: usize,
        value: JsonValue,
    },
    SetProp {
        node: Node,
        key: String,
        value: JsonValue,
    },
    RemoveProp {
        node: Node,
        key: String,
    },
    Insert {
        parent: Node,
        text: String,
        before: Option<Node>,
    },
    Delete {
        node: Node,
    },
    MoveBefore {
        node: Node,
        before: Option<Node>,
    },
    Type {
        file: String,
        at: usize,
        len: usize,
        text: String,
    },
}

impl From<&Ask> for Intent {
    fn from(a: &Ask) -> Intent {
        match a {
            Ask::SetArg { node, index, value } => Intent::SetValue {
                node: node.into(),
                slot: Slot::Arg(*index),
                value: value.into(),
            },
            Ask::SetProp { node, key, value } => Intent::SetValue {
                node: node.into(),
                slot: Slot::Prop(key.clone()),
                value: value.into(),
            },
            Ask::RemoveProp { node, key } => Intent::RemoveProp {
                node: node.into(),
                key: key.clone(),
            },
            Ask::Insert {
                parent,
                text,
                before,
            } => Intent::Insert {
                parent: parent.into(),
                text: text.clone(),
                before: before.as_ref().map(NodeRef::from),
            },
            Ask::Delete { node } => Intent::Delete { node: node.into() },
            Ask::MoveBefore { node, before } => Intent::MoveBefore {
                node: node.into(),
                before: before.as_ref().map(NodeRef::from),
            },
            Ask::Type {
                file,
                at,
                len,
                text,
            } => Intent::Type {
                file: file.clone(),
                at: *at,
                len: *len,
                text: text.clone(),
            },
        }
    }
}

/// What an ask came to.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Answer {
    /// Done (or nothing to do), as one undoable step.
    Applied { revision: u64 },
    /// A choice for the person: `choose(id, n)` with the option they pick.
    Choose { id: u64, options: Vec<ChoiceOption> },
    /// Not done, and why.
    Refused { reason: String },
}

/// One option of a choice, as the interface shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChoiceOption {
    pub label: String,
    /// More about it: the icons that follow, a param's new value.
    pub detail: String,
}

/// A file's text at a revision.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Source {
    pub revision: u64,
    pub text: String,
}

/// A library open in the studio.
pub struct Session {
    lib: Library,
    sink: Arc<dyn Sink>,
    compiling: Compiling,
    watcher: Option<PollingWatcher>,
    pending: BTreeMap<u64, Vec<Choice>>,
    next_choice: u64,
    editing: Option<String>,
}

impl Session {
    /// Opens the library in `root`, compiling it in the background, with
    /// every notice sent to `sink`.
    pub fn open(root: impl Into<PathBuf>, sink: Arc<dyn Sink>) -> std::io::Result<Session> {
        let root = root.into();
        let lib = Library::open(&root)?;
        let watcher = PollingWatcher::new(&root).ok();
        Ok(Session::with(lib, watcher, sink))
    }

    /// A library already loaded, for tests and for a new library.
    pub fn with(lib: Library, watcher: Option<PollingWatcher>, sink: Arc<dyn Sink>) -> Session {
        let (background, events) = Background::start(Isolex, workers());
        forward(events, sink.clone());
        Session::start(lib, watcher, sink, Compiling::Background(background))
    }

    /// A library kept in memory alone, compiled as it changes before each
    /// call returns, for a host with no threads, clock or disk: the
    /// studio's core in a browser. Saving there marks it saved.
    pub fn in_memory(lib: Library, sink: Arc<dyn Sink>) -> Session {
        Session::start(
            lib,
            None,
            sink,
            Compiling::Inline(Box::new(Inline::new(Isolex))),
        )
    }

    fn start(
        lib: Library,
        watcher: Option<PollingWatcher>,
        sink: Arc<dyn Sink>,
        compiling: Compiling,
    ) -> Session {
        let mut s = Session {
            lib,
            sink,
            compiling,
            watcher,
            pending: BTreeMap::new(),
            next_choice: 0,
            editing: None,
        };
        s.changed();
        s
    }

    pub fn library(&self) -> &Library {
        &self.lib
    }

    /// A file's text now, with the revision it is at, for the Source tab.
    pub fn source(&self, file: &str) -> Option<Source> {
        self.lib.files().get(file).map(|text| Source {
            revision: self.lib.revision(),
            text: text.clone(),
        })
    }

    /// A definition's parts, for the Parts panel.
    pub fn outline(&self, definition: &str) -> Option<Outline> {
        crate::outline::outline(&self.lib.sources(), definition)
    }

    /// What the interface shows: the definition and icon open, and the
    /// thumbnails on screen, which compile first.
    pub fn view(&mut self, editing: Option<String>, open: Option<String>, visible: Vec<String>) {
        self.editing = editing;
        match &mut self.compiling {
            Compiling::Background(b) => b.view(open, visible),
            Compiling::Inline(i) => i.view(open, visible),
        }
    }

    /// Plans and, if it is unambiguous, applies `ask`.
    pub fn ask(&mut self, ask: &Ask, label: &str) -> Answer {
        let req = Request::new(&self.lib, self.editing.as_deref(), label, ask.into());
        self.carry_out(req)
    }

    /// The person picked option `n` of choice `id`.
    pub fn choose(&mut self, id: u64, n: usize) -> Answer {
        let Some(choice) = self.pending.remove(&id).and_then(|c| c.into_iter().nth(n)) else {
            return Answer::Refused {
                reason: "that choice is no longer open".into(),
            };
        };
        let then = match choice {
            Choice::EditParam { then, .. }
            | Choice::ReplaceWithNumber { then }
            | Choice::EditInBase { then, .. }
            | Choice::OverrideHere { then, .. } => then,
        };
        self.carry_out(then)
    }

    fn carry_out(&mut self, req: Request) -> Answer {
        match edit::plan(&self.lib, &req) {
            Err(why) => Answer::Refused {
                reason: why.to_string(),
            },
            Ok(Plan::Choose(choices)) => {
                self.next_choice += 1;
                let options = choices.iter().map(describe).collect();
                self.pending.insert(self.next_choice, choices);
                Answer::Choose {
                    id: self.next_choice,
                    options,
                }
            }
            Ok(Plan::Apply(tx)) => {
                if tx.patches.is_empty() {
                    return Answer::Applied {
                        revision: self.lib.revision(),
                    };
                }
                match self.lib.apply(tx) {
                    Ok(()) => {
                        self.changed();
                        Answer::Applied {
                            revision: self.lib.revision(),
                        }
                    }
                    Err(e) => Answer::Refused {
                        reason: e.to_string(),
                    },
                }
            }
        }
    }

    /// Starts a gesture, such as a drag: its edits are one undo step.
    pub fn begin_gesture(&mut self) {
        self.lib.begin_gesture();
    }

    pub fn end_gesture(&mut self) {
        self.lib.end_gesture();
    }

    pub fn undo(&mut self) -> Option<String> {
        let label = self.lib.undo();
        if label.is_some() {
            self.changed();
        }
        label
    }

    pub fn redo(&mut self) -> Option<String> {
        let label = self.lib.redo();
        if label.is_some() {
            self.changed();
        }
        label
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        match self.compiling {
            Compiling::Background(_) => {
                self.lib.save()?;
            }
            Compiling::Inline(_) => self.lib.mark_saved(),
        }
        self.state();
        Ok(())
    }

    /// Discards unsaved edits for what is on disk, undoably.
    pub fn reload(&mut self) -> std::io::Result<()> {
        let files = self.lib.reload_from_disk()?;
        self.changed();
        self.sink.notify(Notice::Reloaded { files });
        Ok(())
    }

    /// Looks for changes on disk; the shell calls it now and then.
    pub fn tick(&mut self) -> std::io::Result<()> {
        let Some(w) = &mut self.watcher else {
            return Ok(());
        };
        let changes = w.poll()?;
        match self.lib.on_disk(changes) {
            DiskOutcome::Nothing => {}
            DiskOutcome::Reloaded(files) => {
                self.changed();
                self.sink.notify(Notice::Reloaded { files });
            }
            DiskOutcome::OutOfSync(files) => {
                self.state();
                self.sink.notify(Notice::OutOfSync { files });
            }
        }
        Ok(())
    }

    /// The library changed: tell the interface, and compile it again.
    fn changed(&mut self) {
        self.pending.clear();
        let sources = self.lib.sources();
        self.sink.notify(Notice::Graph {
            graph: isolex::graph(&sources),
        });
        self.state();
        let (revision, files) = (self.lib.revision(), Arc::new(sources));
        match &mut self.compiling {
            Compiling::Background(b) => b.edit(revision, files),
            Compiling::Inline(i) => {
                for e in i.edit(revision, files) {
                    self.sink.notify(notice(e));
                }
            }
        }
    }

    fn state(&self) {
        self.sink.notify(Notice::Library {
            root: self.lib.root().display().to_string(),
            revision: self.lib.revision(),
            modified: self.lib.is_modified(),
            files: self.lib.files().keys().cloned().collect(),
            history: self.lib.history().iter().map(|s| s.to_string()).collect(),
            can_undo: self.lib.can_undo(),
            can_redo: self.lib.can_redo(),
        });
    }
}

/// How many threads compile icons: all but one of the machine's, so the
/// interface keeps one.
fn workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(2)
}

/// Sends the background compiler's events on as notices, until it stops.
fn forward(events: Receiver<Event>, sink: Arc<dyn Sink>) {
    std::thread::spawn(move || {
        for e in events {
            sink.notify(notice(e));
        }
    });
}

/// How the compiler's event is told to the interface.
fn notice(e: Event) -> Notice {
    match e {
        Event::Lexicon {
            generation,
            revision,
            problems,
            icons,
        } => Notice::Compiled {
            generation,
            revision,
            errors: problems.errors,
            warnings: problems.warnings,
            palette: problems.palette,
            tokens: problems.tokens,
            stroke_widths: problems.stroke_widths,
            icons,
        },
        Event::Changed { name, drawing, .. } => {
            let Drawing { ir, sources } = (*drawing).clone();
            Notice::Icon { name, ir, sources }
        }
        Event::Broken { name, diags, last } => Notice::Broken {
            name,
            diagnostics: diags,
            last: last.map(|l| (*l).clone()),
        },
        Event::Gone { name } => Notice::Gone { name },
    }
}

/// How a session compiles: on threads of its own, or as it is called.
enum Compiling {
    Background(Background),
    Inline(Box<Inline>),
}

/// A choice as the interface shows it.
fn describe(c: &Choice) -> ChoiceOption {
    match c {
        Choice::EditParam {
            param,
            definition,
            value,
            ..
        } => ChoiceOption {
            label: format!("Edit param {param}"),
            detail: format!("Sets it to {value} where {definition} declares it"),
        },
        Choice::ReplaceWithNumber { .. } => ChoiceOption {
            label: "Replace with a number".into(),
            detail: "The expression is written as the number".into(),
        },
        Choice::EditInBase { base, affected, .. } => ChoiceOption {
            label: format!("Edit in {base}"),
            detail: if affected.is_empty() {
                "No other icon follows".into()
            } else {
                format!(
                    "Changes {} other icons: {}",
                    affected.len(),
                    affected.join(", ")
                )
            },
        },
        Choice::OverrideHere { part, .. } => ChoiceOption {
            label: "Override here".into(),
            detail: format!("Copies part \"{part}\" into this definition"),
        },
    }
}
