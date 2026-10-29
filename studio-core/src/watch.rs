//! Watching the library's folder for changes made outside the studio:
//! by git, another editor or a script (#63).
//!
//! A [`Watcher`] says which `.ilx` files were added, changed or removed
//! since it last looked. [`Library::on_disk`] decides what that means:
//! - a change that is what the studio itself last saved is nothing;
//! - with no unsaved edits, the change is loaded, as one undoable step,
//!   and the library stays unmodified;
//! - with unsaved edits, the library is out of sync with the disk, and is
//!   left as it is until [`Library::reload_from_disk`] or a save, which
//!   then writes over the outside change too.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::library::{hash, is_library_path, FilePath, Library, Patch, Transaction};

/// One file as the watcher found it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskChange {
    /// Added or changed, with its text now.
    Written(FilePath, String),
    Removed(FilePath),
}

impl DiskChange {
    pub fn path(&self) -> &str {
        match self {
            DiskChange::Written(p, _) | DiskChange::Removed(p) => p,
        }
    }
}

/// Something that says how a library's folder changed since it last
/// looked.
pub trait Watcher {
    fn poll(&mut self) -> io::Result<Vec<DiskChange>>;
}

/// What a library made of changes on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskOutcome {
    /// Nothing to do: none, or only the studio's own saves.
    Nothing,
    /// Loaded, as one undoable step: these files.
    Reloaded(Vec<FilePath>),
    /// The library has unsaved edits, so nothing was loaded: these files
    /// differ on disk. The interface shows the out-of-sync notice.
    OutOfSync(Vec<FilePath>),
}

/// A watcher on `std` alone: it looks at each `.ilx` file's size and
/// modification time, and reads a file only when either changed. A file
/// rewritten with the same size within the file system's time resolution
/// can be missed until it changes again.
pub struct PollingWatcher {
    root: PathBuf,
    seen: BTreeMap<FilePath, (u64, Option<SystemTime>, u64)>,
}

impl PollingWatcher {
    /// Starts watching `root` as it is now: nothing in it is a change.
    pub fn new(root: impl Into<PathBuf>) -> io::Result<PollingWatcher> {
        let mut w = PollingWatcher {
            root: root.into(),
            seen: BTreeMap::new(),
        };
        w.poll()?;
        Ok(w)
    }
}

impl Watcher for PollingWatcher {
    fn poll(&mut self) -> io::Result<Vec<DiskChange>> {
        let mut now = BTreeMap::new();
        let mut out = Vec::new();
        for (path, at) in ilx_files(&self.root)? {
            let meta = std::fs::metadata(&at)?;
            let stamp = (meta.len(), meta.modified().ok());
            match self.seen.get(&path) {
                Some(&(len, time, h)) if (len, time) == stamp => {
                    now.insert(path, (len, time, h));
                }
                before => {
                    let text = match std::fs::read_to_string(&at) {
                        Ok(t) => t,
                        // removed between the listing and the read
                        Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                        Err(e) => return Err(e),
                    };
                    let h = hash(&text);
                    if before.map(|b| b.2) != Some(h) {
                        out.push(DiskChange::Written(path.clone(), text));
                    }
                    now.insert(path, (stamp.0, stamp.1, h));
                }
            }
        }
        for path in self.seen.keys() {
            if !now.contains_key(path) {
                out.push(DiskChange::Removed(path.clone()));
            }
        }
        self.seen = now;
        Ok(out)
    }
}

/// Every `.ilx` file under `root`, by library path, with where it is.
fn ilx_files(root: &Path) -> io::Result<Vec<(FilePath, PathBuf)>> {
    let mut out = Vec::new();
    let mut todo = vec![(String::new(), root.to_path_buf())];
    while let Some((rel, dir)) = todo.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound && !rel.is_empty() => continue,
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let path = entry.path();
            if path.is_dir() {
                todo.push((rel, path));
            } else if is_library_path(&rel) {
                out.push((rel, path));
            }
        }
    }
    Ok(out)
}

impl Library {
    /// What `changes` on disk mean for this library; see the module.
    pub fn on_disk(&mut self, changes: Vec<DiskChange>) -> DiskOutcome {
        // the studio's own saves, and changes that leave a file as the
        // library last saved it, are nothing
        let foreign: Vec<DiskChange> = changes
            .into_iter()
            .filter(|c| match c {
                DiskChange::Written(p, t) => self.saved_hashes().get(p) != Some(&hash(t)),
                DiskChange::Removed(p) => self.saved_hashes().contains_key(p),
            })
            .collect();
        if foreign.is_empty() {
            return DiskOutcome::Nothing;
        }
        let paths: Vec<FilePath> = foreign.iter().map(|c| c.path().to_string()).collect();
        if self.is_modified() {
            // the disk holds these now; a save writes over them
            self.saw_on_disk(&foreign);
            return DiskOutcome::OutOfSync(paths);
        }
        let patches = foreign
            .iter()
            .filter_map(|c| self.patch_to(c))
            .collect::<Vec<_>>();
        let label = format!("Changed on disk: {}", paths.join(", "));
        self.end_gesture();
        self.apply(Transaction::new(label, patches))
            .expect("patches made from the library's own files apply");
        self.saw_on_disk(&foreign);
        DiskOutcome::Reloaded(paths)
    }

    /// Discards unsaved edits for what is on disk, as one undoable step.
    pub fn reload_from_disk(&mut self) -> io::Result<Vec<FilePath>> {
        let disk = crate::library::read_folder(self.root())?;
        Ok(self.reload_from(disk))
    }

    /// Discards unsaved edits for `disk`, the files as they are on disk
    /// (or, for a library in memory, as it keeps them), as one undoable
    /// step; the files that differed.
    pub fn reload_from(&mut self, disk: BTreeMap<FilePath, String>) -> Vec<FilePath> {
        let mut changes = Vec::new();
        for (p, t) in &disk {
            if self.files().get(p) != Some(t) {
                changes.push(DiskChange::Written(p.clone(), t.clone()));
            }
        }
        for p in self.files().keys() {
            if !disk.contains_key(p) {
                changes.push(DiskChange::Removed(p.clone()));
            }
        }
        let paths: Vec<FilePath> = changes.iter().map(|c| c.path().to_string()).collect();
        let patches: Vec<Patch> = changes.iter().filter_map(|c| self.patch_to(c)).collect();
        self.end_gesture();
        if !patches.is_empty() {
            self.apply(Transaction::new("Reload from disk", patches))
                .expect("patches made from the library's own files apply");
        }
        self.set_saved(disk.iter().map(|(p, t)| (p.clone(), hash(t))).collect());
        paths
    }

    /// The patch that makes the library's file what `change` says.
    fn patch_to(&self, change: &DiskChange) -> Option<Patch> {
        match change {
            DiskChange::Written(p, t) => Some(match self.files().get(p) {
                Some(old) if old == t => return None,
                Some(old) => Patch::Splice {
                    file: p.clone(),
                    at: 0,
                    len: old.len(),
                    text: t.clone(),
                },
                None => Patch::Create {
                    file: p.clone(),
                    text: t.clone(),
                },
            }),
            DiskChange::Removed(p) => self
                .files()
                .contains_key(p)
                .then(|| Patch::Delete { file: p.clone() }),
        }
    }

    /// Records that the disk now holds what `changes` say.
    fn saw_on_disk(&mut self, changes: &[DiskChange]) {
        let mut saved = self.saved_hashes().clone();
        for c in changes {
            match c {
                DiskChange::Written(p, t) => {
                    saved.insert(p.clone(), hash(t));
                }
                DiskChange::Removed(p) => {
                    saved.remove(p);
                }
            }
        }
        self.set_saved(saved);
    }
}
