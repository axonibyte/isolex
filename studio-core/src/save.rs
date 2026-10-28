//! Saving a library all at once, so its folder is never half-saved (#61).
//!
//! 1. Each file to write is written to a temporary file beside it and
//!    synced to disk.
//! 2. A journal naming every rename and removal still to do is written
//!    and synced.
//! 3. Each temporary file is renamed over its target, and each removed
//!    file is removed.
//! 4. The journal is removed.
//!
//! An interruption before the journal is written leaves the old files and
//! some temporary ones, which [`recover`] removes: the save did not
//! happen. An interruption after it leaves the journal, and [`recover`]
//! finishes what it names: the save happened. Either way the folder is
//! all old or all new, never a mix.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::library::{Change, Library};

/// The journal of a save in progress, at the library's root.
pub const JOURNAL: &str = ".isolex-studio-save";
/// What a file being written is called until it is renamed into place.
pub const TEMP_SUFFIX: &str = ".isolex-studio-tmp";

#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    /// (temporary file, target), library paths.
    renames: Vec<(String, String)>,
    removes: Vec<String>,
}

/// What a save did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub written: Vec<String>,
    pub removed: Vec<String>,
}

/// Where a save was made to fail, for tests of recovery.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Faults {
    /// Fail before the journal is written.
    pub before_journal: bool,
    /// Fail after this many of the renames into place.
    pub after_renames: Option<usize>,
}

impl Library {
    /// Writes every changed file and removes every removed one, at once.
    pub fn save(&mut self) -> io::Result<Saved> {
        self.save_with(Faults::default())
    }

    #[doc(hidden)]
    pub fn save_with(&mut self, faults: Faults) -> io::Result<Saved> {
        let changes = self.changes();
        let root = self.root().to_path_buf();
        let mut journal = Journal {
            renames: Vec::new(),
            removes: Vec::new(),
        };
        // 1. every new text beside its target, on disk
        for change in &changes {
            match change {
                Change::Write(path, text) => {
                    let tmp = format!("{path}{TEMP_SUFFIX}");
                    let at = root.join(&tmp);
                    if let Some(dir) = at.parent() {
                        fs::create_dir_all(dir)?;
                    }
                    let mut f = fs::File::create(&at)?;
                    f.write_all(text.as_bytes())?;
                    f.sync_all()?;
                    journal.renames.push((tmp, path.clone()));
                }
                Change::Remove(path) => journal.removes.push(path.clone()),
            }
        }
        if faults.before_journal {
            return Err(injected("before the journal"));
        }
        // 2. the journal: from here on, the save happens
        write_synced(
            &root.join(JOURNAL),
            serde_json::to_string(&journal)
                .expect("a journal serializes")
                .as_bytes(),
        )?;
        sync_dir(&root);
        // 3. into place
        finish(&root, &journal, faults.after_renames)?;
        // 4. done
        fs::remove_file(root.join(JOURNAL))?;
        sync_dir(&root);
        self.mark_saved();
        Ok(Saved {
            written: journal.renames.into_iter().map(|(_, t)| t).collect(),
            removed: journal.removes,
        })
    }
}

/// Finishes or undoes a save interrupted in `root`: with a journal, does
/// what it names and removes it; without one, removes any temporary file
/// a save left. Returns whether there was anything to do.
pub fn recover(root: &Path) -> io::Result<bool> {
    let journal_path = root.join(JOURNAL);
    match fs::read_to_string(&journal_path) {
        Ok(text) => {
            let journal: Journal = serde_json::from_str(&text).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{} is not a save journal: {e}", journal_path.display()),
                )
            })?;
            finish(root, &journal, None)?;
            fs::remove_file(&journal_path)?;
            sync_dir(root);
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let strays = temporaries(root)?;
            for stray in &strays {
                fs::remove_file(stray)?;
            }
            Ok(!strays.is_empty())
        }
        Err(e) => Err(e),
    }
}

/// Renames and removes what the journal names, skipping what is already
/// done, so it can run again after an interruption of its own.
fn finish(root: &Path, journal: &Journal, fail_after: Option<usize>) -> io::Result<()> {
    for (n, (tmp, target)) in journal.renames.iter().enumerate() {
        if fail_after == Some(n) {
            return Err(injected("during the renames"));
        }
        let from = root.join(tmp);
        if from.exists() {
            rename_retrying(&from, &root.join(target))?;
        }
    }
    for path in &journal.removes {
        let at = root.join(path);
        // On a file system that ignores case, a file renamed from
        // Rack.ilx to rack.ilx is written as rack.ilx and removed as
        // Rack.ilx, which is the same file: removing it would remove what
        // was just written.
        let written_as = journal
            .renames
            .iter()
            .filter(|(_, t)| t.to_lowercase() == path.to_lowercase())
            .any(|(_, t)| same_file(&at, &root.join(t)));
        if written_as {
            continue;
        }
        match fs::remove_file(&at) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    sync_dir(root);
    Ok(())
}

/// Whether two paths name one file on disk.
fn same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(x), Ok(y)) = (fs::metadata(a), fs::metadata(b)) {
            return x.dev() == y.dev() && x.ino() == y.ino();
        }
        false
    }
    #[cfg(not(unix))]
    {
        matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
    }
}

/// Renames over an existing file. Windows refuses for a moment while
/// another program (a virus scanner, an indexer) has the target open, so
/// it is tried a few times before giving up.
fn rename_retrying(from: &Path, to: &Path) -> io::Result<()> {
    let mut wait = std::time::Duration::from_millis(10);
    for _ in 0..5 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                std::thread::sleep(wait);
                wait *= 2;
            }
            Err(e) => return Err(e),
        }
    }
    fs::rename(from, to)
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut f = fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Makes renames in `dir` durable where the system allows it; Windows has
/// no way to sync a directory, and needs none for a rename.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Every temporary file a save left under `root`.
fn temporaries(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                todo.push(path);
            } else if path.to_string_lossy().ends_with(TEMP_SUFFIX) {
                out.push(path);
            }
        }
    }
    Ok(out)
}

fn injected(when: &str) -> io::Error {
    io::Error::other(format!("a failure injected {when}"))
}
