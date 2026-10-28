//! Compiling the library in the background (#62): every edit recompiles
//! it, off the interface's thread, the newest edit cancelling a compile it
//! makes moot, and only what changed is sent on.
//!
//! The parts that decide are kept apart from the threads, so tests drive
//! them with a clock and a compiler of their own:
//! - [`Scheduler`] waits for a quiet moment after the last edit, then
//!   starts a generation, cancelling the one before;
//! - [`run_generation`] compiles one generation, the icon open in the
//!   editor first, then those the interface shows, then the rest;
//! - [`Tracker`] turns its results into [`Event`]s: an older generation's
//!   are dropped, an icon drawn as before says nothing, and one that broke
//!   keeps its last good drawing;
//! - [`SvgCache`] keeps the SVG drawn for each IR and palette.
//!
//! [`Background`] puts them on threads, with `isolex::Prepared`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use isolex::ir::IrIcon;

use crate::library::hash;

/// How long the library must be quiet after an edit before it compiles.
pub const QUIET: Duration = Duration::from_millis(120);

/// The library's files at one revision, shared without copying.
pub type Snapshot = Arc<Vec<(String, String)>>;

/// One problem, as the compiler located it.
pub type Diag = isolex::parser::Diag;

/// What one icon came to in one generation.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Drawn: its IR, as JSON, and that JSON's hash.
    Drawn { ir: Arc<IrIcon>, hash: u64 },
    /// It does not compile.
    Broken(Vec<Diag>),
}

/// What the compiler has to do, so tests can stand in for it.
pub trait Compiler: Send + Sync + 'static {
    type Prepared: Send + Sync;
    /// Parses and resolves the library once: the lexicon's own errors and
    /// warnings, and the icons it emits, in order.
    fn prepare(&self, files: &[(String, String)]) -> (Self::Prepared, Vec<Diag>, Vec<String>);
    /// One icon, or none when `cancel` was set first.
    fn icon(&self, prepared: &Self::Prepared, name: &str, cancel: &AtomicBool) -> Option<Outcome>;
}

/// The compiler itself.
#[derive(Debug, Clone, Copy, Default)]
pub struct Isolex;

impl Compiler for Isolex {
    type Prepared = isolex::Prepared;

    fn prepare(&self, files: &[(String, String)]) -> (isolex::Prepared, Vec<Diag>, Vec<String>) {
        let prepared = isolex::Prepared::new(files);
        let o = prepared.outcome();
        let mut diags = o.errors.clone();
        diags.extend(o.warnings.iter().cloned());
        let names = isolex::graph(files)
            .definitions
            .into_iter()
            .flat_map(|d| d.emits)
            .collect();
        (prepared, diags, names)
    }

    fn icon(
        &self,
        prepared: &isolex::Prepared,
        name: &str,
        cancel: &AtomicBool,
    ) -> Option<Outcome> {
        let request = isolex::Only {
            icon: name.to_string(),
            params: BTreeMap::new(),
        };
        match prepared.icon(&request, cancel) {
            Err(isolex::Refused::Cancelled) => None,
            Err(isolex::Refused::Request(why)) => Some(Outcome::Broken(vec![Diag {
                file: String::new(),
                line: 0,
                col: 0,
                message: why,
            }])),
            Ok(c) if !c.errors.is_empty() => Some(Outcome::Broken(c.errors)),
            Ok(mut c) => match c.doc.icons.pop() {
                Some(icon) => {
                    let json = serde_json::to_string(&icon).expect("IR serializes");
                    Some(Outcome::Drawn {
                        hash: hash(&json),
                        ir: Arc::new(icon),
                    })
                }
                None => Some(Outcome::Broken(Vec::new())),
            },
        }
    }
}

// --- scheduling ---------------------------------------------------------------

/// Decides when a generation starts: after [`QUIET`] with no edit, for the
/// newest snapshot not yet compiled, cancelling the one compiling before it.
#[derive(Debug, Default)]
pub struct Scheduler {
    latest: Option<(u64, Snapshot, Instant)>,
    started: Option<u64>,
    generation: u64,
    running: Option<Arc<AtomicBool>>,
}

/// A generation to compile: its number, the revision and files, and its
/// cancel flag.
#[derive(Debug, Clone)]
pub struct Generation {
    pub number: u64,
    pub revision: u64,
    pub files: Snapshot,
    pub cancel: Arc<AtomicBool>,
}

impl Scheduler {
    /// The library changed, at `now`.
    pub fn edit(&mut self, revision: u64, files: Snapshot, now: Instant) {
        self.latest = Some((revision, files, now));
    }

    /// When the next generation could start, if one is waiting.
    pub fn due(&self) -> Option<Instant> {
        match &self.latest {
            Some((rev, _, at)) if Some(*rev) != self.started => Some(*at + QUIET),
            _ => None,
        }
    }

    /// The generation to start at `now`, if the library has been quiet long
    /// enough since its last edit; the one before it is cancelled.
    pub fn poll(&mut self, now: Instant) -> Option<Generation> {
        if self.due()? > now {
            return None;
        }
        let (revision, files, _) = self.latest.clone()?;
        if let Some(previous) = self.running.take() {
            previous.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        self.started = Some(revision);
        let cancel = Arc::new(AtomicBool::new(false));
        self.running = Some(cancel.clone());
        Some(Generation {
            number: self.generation,
            revision,
            files,
            cancel,
        })
    }
}

// --- one generation -------------------------------------------------------------

/// What a generation sends, in order: the lexicon first, then its icons.
#[derive(Debug, Clone)]
pub enum Report {
    /// The library parsed and resolved: its errors and warnings, and every
    /// icon it emits.
    Lexicon {
        generation: u64,
        revision: u64,
        diags: Vec<Diag>,
        icons: Vec<String>,
    },
    Icon {
        generation: u64,
        name: String,
        outcome: Outcome,
    },
}

/// The order icons compile in: `first` (the one open), then `visible` (the
/// thumbnails on screen), then the rest, each once.
pub fn order(icons: &[String], first: Option<&str>, visible: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(icons.len());
    let mut add = |n: &str| {
        if icons.iter().any(|i| i == n) && !out.iter().any(|o| o == n) {
            out.push(n.to_string());
        }
    };
    if let Some(f) = first {
        add(f);
    }
    for v in visible {
        add(v);
    }
    for i in icons {
        add(i);
    }
    out
}

/// Compiles one generation on `workers` threads, sending each result as it
/// comes, until it is done or cancelled.
pub fn run_generation<C: Compiler>(
    compiler: &C,
    generation: &Generation,
    first: Option<&str>,
    visible: &[String],
    workers: usize,
    send: &(dyn Fn(Report) + Sync),
) {
    let (prepared, diags, icons) = compiler.prepare(&generation.files);
    if generation.cancel.load(Ordering::Relaxed) {
        return;
    }
    let queue = order(&icons, first, visible);
    send(Report::Lexicon {
        generation: generation.number,
        revision: generation.revision,
        diags,
        icons,
    });
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| loop {
                if generation.cancel.load(Ordering::Relaxed) {
                    return;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(name) = queue.get(i) else { return };
                if let Some(outcome) = compiler.icon(&prepared, name, &generation.cancel) {
                    send(Report::Icon {
                        generation: generation.number,
                        name: name.clone(),
                        outcome,
                    });
                }
            });
        }
    });
}

// --- what the interface hears ----------------------------------------------------

/// What the interface is told.
#[derive(Debug, Clone)]
pub enum Event {
    /// The library's own errors and warnings, and its icons, for a
    /// generation.
    Lexicon {
        generation: u64,
        revision: u64,
        diags: Vec<Diag>,
        icons: Vec<String>,
    },
    /// An icon drawn differently from before (or for the first time).
    Changed {
        name: String,
        ir: Arc<IrIcon>,
        hash: u64,
    },
    /// An icon that does not compile now; `last` is its last good drawing,
    /// which the interface keeps showing.
    Broken {
        name: String,
        diags: Vec<Diag>,
        last: Option<Arc<IrIcon>>,
    },
    /// An icon the library no longer emits.
    Gone { name: String },
}

/// What the tracker knows of one icon.
#[derive(Debug, Default)]
struct Known {
    /// Its last good drawing, and its hash.
    last: Option<(Arc<IrIcon>, u64)>,
    /// It does not compile now.
    broken: bool,
}

/// Turns results into events.
#[derive(Debug, Default)]
pub struct Tracker {
    newest: u64,
    /// Each icon's last good drawing, and whether it is broken now.
    icons: BTreeMap<String, Known>,
}

impl Tracker {
    /// The events `result` makes, if any.
    pub fn accept(&mut self, result: Report) -> Vec<Event> {
        match result {
            Report::Lexicon {
                generation,
                revision,
                diags,
                icons,
            } => {
                if generation < self.newest {
                    return Vec::new();
                }
                self.newest = generation;
                let mut out = Vec::new();
                let gone: Vec<String> = self
                    .icons
                    .keys()
                    .filter(|k| !icons.contains(k))
                    .cloned()
                    .collect();
                for name in gone {
                    self.icons.remove(&name);
                    out.push(Event::Gone { name });
                }
                out.insert(
                    0,
                    Event::Lexicon {
                        generation,
                        revision,
                        diags,
                        icons,
                    },
                );
                out
            }
            Report::Icon {
                generation,
                name,
                outcome,
            } => {
                if generation < self.newest {
                    return Vec::new();
                }
                let entry = self.icons.entry(name.clone()).or_default();
                match outcome {
                    Outcome::Drawn { ir, hash } => {
                        let same = entry.last.as_ref().is_some_and(|(_, h)| *h == hash);
                        let was_broken = entry.broken;
                        entry.last = Some((ir.clone(), hash));
                        entry.broken = false;
                        if same && !was_broken {
                            return Vec::new();
                        }
                        vec![Event::Changed { name, ir, hash }]
                    }
                    Outcome::Broken(diags) => {
                        entry.broken = true;
                        vec![Event::Broken {
                            name,
                            diags,
                            last: entry.last.as_ref().map(|(ir, _)| ir.clone()),
                        }]
                    }
                }
            }
        }
    }
}

// --- SVG, kept --------------------------------------------------------------------

/// The SVG drawn for an IR with a palette, kept for the most recent
/// `capacity` of them, so an icon that did not change is not drawn again.
pub struct SvgCache {
    capacity: usize,
    entries: BTreeMap<(u64, u64), Arc<str>>,
    recent: VecDeque<(u64, u64)>,
}

impl SvgCache {
    pub fn new(capacity: usize) -> SvgCache {
        SvgCache {
            capacity: capacity.max(1),
            entries: BTreeMap::new(),
            recent: VecDeque::new(),
        }
    }

    /// The SVG of `ir` (whose hash is `ir_hash`) in `palette` (whose hash is
    /// `palette_hash`), drawn now or kept from before.
    pub fn svg(
        &mut self,
        ir: &IrIcon,
        ir_hash: u64,
        palette: &isolex::render::Palette,
        palette_hash: u64,
    ) -> Arc<str> {
        let key = (ir_hash, palette_hash);
        if let Some(svg) = self.entries.get(&key).cloned() {
            self.recent.retain(|k| *k != key);
            self.recent.push_back(key);
            return svg;
        }
        let svg: Arc<str> = isolex::render::render_icon(ir, palette).into();
        self.entries.insert(key, svg.clone());
        self.recent.push_back(key);
        while self.recent.len() > self.capacity {
            if let Some(old) = self.recent.pop_front() {
                self.entries.remove(&old);
            }
        }
        svg
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// --- on threads -----------------------------------------------------------------------

enum Message {
    Edit(u64, Snapshot),
    View(Option<String>, Vec<String>),
    Stop,
}

/// Compiles the library in the background: edits go in, events come out.
pub struct Background {
    send: Sender<Message>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Background {
    /// Starts compiling with `compiler` on `workers` threads a generation.
    pub fn start<C: Compiler>(compiler: C, workers: usize) -> (Background, Receiver<Event>) {
        let (send, inbox) = std::sync::mpsc::channel::<Message>();
        let (events, out) = std::sync::mpsc::channel::<Event>();
        let thread = std::thread::spawn(move || {
            let compiler = Arc::new(compiler);
            let tracker = Arc::new(std::sync::Mutex::new(Tracker::default()));
            let mut scheduler = Scheduler::default();
            let mut view: (Option<String>, Vec<String>) = (None, Vec::new());
            let mut compiling: Vec<std::thread::JoinHandle<()>> = Vec::new();
            loop {
                let wait = scheduler
                    .due()
                    .map(|d| d.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(3600));
                match inbox.recv_timeout(wait) {
                    Ok(Message::Edit(rev, files)) => scheduler.edit(rev, files, Instant::now()),
                    Ok(Message::View(first, visible)) => view = (first, visible),
                    Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                if let Some(generation) = scheduler.poll(Instant::now()) {
                    let (compiler, tracker, events, view) = (
                        compiler.clone(),
                        tracker.clone(),
                        events.clone(),
                        view.clone(),
                    );
                    compiling.retain(|h| !h.is_finished());
                    compiling.push(std::thread::spawn(move || {
                        let send = |r: Report| {
                            let made = tracker.lock().expect("the tracker").accept(r);
                            for e in made {
                                let _ = events.send(e);
                            }
                        };
                        run_generation(
                            &*compiler,
                            &generation,
                            view.0.as_deref(),
                            &view.1,
                            workers,
                            &send,
                        );
                    }));
                }
            }
            for h in compiling {
                let _ = h.join();
            }
        });
        (
            Background {
                send,
                thread: Some(thread),
            },
            out,
        )
    }

    /// The library is now `files`, at `revision`.
    pub fn edit(&self, revision: u64, files: Snapshot) {
        let _ = self.send.send(Message::Edit(revision, files));
    }

    /// What the interface shows: the icon open, and the thumbnails on
    /// screen, which compile first.
    pub fn view(&self, open: Option<String>, visible: Vec<String>) {
        let _ = self.send.send(Message::View(open, visible));
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        let _ = self.send.send(Message::Stop);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
