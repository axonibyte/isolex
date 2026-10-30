//! The library being edited: its files, one undo history over all of
//! them, and whether it differs from what is on disk (#61).
//!
//! The library is edited as one thing. An edit is a [`Transaction`] of
//! patches across files, checked whole before anything changes and kept
//! with its inverse, so one undo step can span a rename that rewrites ten
//! files. Whether the library is modified is a comparison with the hashes
//! of what was last loaded or saved, so undoing back to the saved state
//! leaves it unmodified.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A file's path in the library: relative to its folder, with `/` between
/// its parts, as `isolex::library::compare` orders them.
pub type FilePath = String;

/// One change to the library's files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Patch {
    /// Replace the `len` bytes of `file` at `at` with `text`.
    Splice {
        file: FilePath,
        at: usize,
        len: usize,
        text: String,
    },
    /// A new file holding `text`.
    Create { file: FilePath, text: String },
    /// Remove a file (its text is kept in the inverse).
    Delete { file: FilePath },
    /// Move a file to another path.
    Rename { from: FilePath, to: FilePath },
}

/// Patches applied together, as one step a person can name and undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// What it did, for the undo history ("Move box in server-2u").
    pub label: String,
    pub patches: Vec<Patch>,
}

impl Transaction {
    pub fn new(label: impl Into<String>, patches: Vec<Patch>) -> Transaction {
        Transaction {
            label: label.into(),
            patches,
        }
    }
}

/// Why a transaction was refused; when it is, nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    NoSuchFile(FilePath),
    FileExists(FilePath),
    /// A splice reaching past the end of its file, or cutting a character.
    BadRange {
        file: FilePath,
        at: usize,
        len: usize,
    },
    /// A path that is not a library path: empty, absolute, with `..`, a
    /// backslash or a control character in it, or not an `.ilx` file.
    BadPath(FilePath),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::NoSuchFile(p) => write!(f, "the library has no file {p}"),
            EditError::FileExists(p) => write!(f, "the library already has a file {p}"),
            EditError::BadRange { file, at, len } => {
                write!(f, "{file} has no text at {at}..{} to replace", at + len)
            }
            EditError::BadPath(p) => write!(f, "{p:?} is not a path for a file in the library"),
        }
    }
}

/// A step of the history: what was done, and how to undo it.
#[derive(Debug, Clone)]
struct Step {
    label: String,
    /// Patches added while this gesture lasts join this step.
    gesture: Option<u64>,
    forward: Vec<Patch>,
    /// In the order they undo it: the last patch's inverse first.
    inverse: Vec<Patch>,
}

/// The files of a library, the history of edits to them, and what was
/// last loaded or saved.
#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    files: BTreeMap<FilePath, String>,
    /// The hash of each file as it is on disk, as far as this library
    /// knows: loaded or saved by it.
    saved: BTreeMap<FilePath, u64>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    gesture: Option<u64>,
    next_gesture: u64,
    /// Counts every change to the files, so an edit planned against one
    /// state of them is refused against another.
    revision: u64,
}

/// FNV-1a, 64 bits: enough to tell a file from how it was saved.
pub(crate) fn hash(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Whether `p` names a file a library can hold.
pub fn is_library_path(p: &str) -> bool {
    !p.is_empty()
        && p.ends_with(".ilx")
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.contains(':')
        && !p.chars().any(char::is_control)
        && p.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

impl Library {
    /// A library of these files, as if just loaded from `root`.
    pub fn new(root: impl Into<PathBuf>, files: BTreeMap<FilePath, String>) -> Library {
        let saved = files.iter().map(|(p, t)| (p.clone(), hash(t))).collect();
        Library {
            root: root.into(),
            files,
            saved,
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            next_gesture: 0,
            revision: 0,
        }
    }

    /// Reads every `.ilx` file under `root`, after finishing or undoing a
    /// save that was interrupted there (see [`crate::save::recover`]).
    pub fn open(root: impl Into<PathBuf>) -> std::io::Result<Library> {
        let root = root.into();
        crate::save::recover(&root)?;
        let files = read_folder(&root)?;
        Ok(Library::new(root, files))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Each file's text, by path.
    pub fn files(&self) -> &BTreeMap<FilePath, String> {
        &self.files
    }

    /// The files as the compiler takes them: named by their paths, in the
    /// order a lexicon is read (SPEC §1).
    pub fn sources(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .files
            .iter()
            .map(|(p, t)| (p.clone(), t.clone()))
            .collect();
        out.sort_by(|a, b| isolex::library::compare(&a.0, &b.0));
        out
    }

    /// Whether the files differ from what is on disk.
    pub fn is_modified(&self) -> bool {
        !self.changes().is_empty()
    }

    /// What saving would do: each file to write, with its text, and each
    /// to remove, in path order.
    pub fn changes(&self) -> Vec<Change> {
        let mut out = Vec::new();
        for (p, t) in &self.files {
            if self.saved.get(p) != Some(&hash(t)) {
                out.push(Change::Write(p.clone(), t.clone()));
            }
        }
        for p in self.saved.keys() {
            if !self.files.contains_key(p) {
                out.push(Change::Remove(p.clone()));
            }
        }
        out.sort_by(|a, b| a.path().cmp(b.path()));
        out
    }

    /// What is now on disk: the files as they are.
    pub(crate) fn mark_saved(&mut self) {
        self.saved = self
            .files
            .iter()
            .map(|(p, t)| (p.clone(), hash(t)))
            .collect();
    }

    pub(crate) fn set_saved(&mut self, saved: BTreeMap<FilePath, u64>) {
        self.saved = saved;
    }

    /// The hashes this library last loaded or saved, by path: what it
    /// knows the disk holds, to tell its own writes from others'.
    pub fn saved_hashes(&self) -> &BTreeMap<FilePath, u64> {
        &self.saved
    }

    /// Applies `tx` as one undo step, or refuses it whole.
    pub fn apply(&mut self, tx: Transaction) -> Result<(), EditError> {
        let inverse = self.apply_patches(&tx.patches)?;
        self.redo.clear();
        if let (Some(g), Some(last)) = (self.gesture, self.undo.last_mut()) {
            if last.gesture == Some(g) {
                last.forward.extend(tx.patches);
                let mut inv = inverse;
                inv.extend(std::mem::take(&mut last.inverse));
                last.inverse = inv;
                return Ok(());
            }
        }
        self.undo.push(Step {
            label: tx.label,
            gesture: self.gesture,
            forward: tx.patches,
            inverse,
        });
        Ok(())
    }

    /// Starts a gesture, such as a drag: until it ends, every transaction
    /// joins the first one's undo step.
    pub fn begin_gesture(&mut self) {
        self.next_gesture += 1;
        self.gesture = Some(self.next_gesture);
    }

    pub fn end_gesture(&mut self) {
        self.gesture = None;
    }

    /// Undoes the last step, returning its label; none when there is none.
    pub fn undo(&mut self) -> Option<String> {
        self.gesture = None;
        let step = self.undo.pop()?;
        self.apply_patches(&step.inverse)
            .expect("an inverse applies to what its step made");
        let label = step.label.clone();
        self.redo.push(step);
        Some(label)
    }

    /// Redoes the last undone step, returning its label.
    pub fn redo(&mut self) -> Option<String> {
        self.gesture = None;
        let step = self.redo.pop()?;
        self.apply_patches(&step.forward)
            .expect("a step reapplies to what its undo left");
        let label = step.label.clone();
        self.undo.push(step);
        Some(label)
    }

    /// The labels of the steps that can be undone, oldest first.
    pub fn history(&self) -> Vec<&str> {
        self.undo.iter().map(|s| s.label.as_str()).collect()
    }

    /// The labels of the steps that can be redone, the next first.
    pub fn future(&self) -> Vec<&str> {
        self.redo.iter().rev().map(|s| s.label.as_str()).collect()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Which state of the files this is: it changes with every edit, undo
    /// and redo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies the patches in order to a copy, and only if every one
    /// applies, keeps it: returns their inverses, last first.
    fn apply_patches(&mut self, patches: &[Patch]) -> Result<Vec<Patch>, EditError> {
        let mut files = self.files.clone();
        let mut inverse = Vec::with_capacity(patches.len());
        for patch in patches {
            inverse.push(apply_one(&mut files, patch)?);
        }
        inverse.reverse();
        self.files = files;
        self.revision += 1;
        Ok(inverse)
    }
}

/// Applies one patch to `files`, returning its inverse.
fn apply_one(files: &mut BTreeMap<FilePath, String>, patch: &Patch) -> Result<Patch, EditError> {
    let check = |p: &FilePath| {
        if is_library_path(p) {
            Ok(())
        } else {
            Err(EditError::BadPath(p.clone()))
        }
    };
    match patch {
        Patch::Splice {
            file,
            at,
            len,
            text,
        } => {
            let body = files
                .get_mut(file)
                .ok_or_else(|| EditError::NoSuchFile(file.clone()))?;
            let end = at.checked_add(*len).filter(|e| *e <= body.len());
            let Some(end) = end.filter(|e| body.is_char_boundary(*at) && body.is_char_boundary(*e))
            else {
                return Err(EditError::BadRange {
                    file: file.clone(),
                    at: *at,
                    len: *len,
                });
            };
            let old = body[*at..end].to_string();
            body.replace_range(*at..end, text);
            Ok(Patch::Splice {
                file: file.clone(),
                at: *at,
                len: text.len(),
                text: old,
            })
        }
        Patch::Create { file, text } => {
            check(file)?;
            if files.contains_key(file) {
                return Err(EditError::FileExists(file.clone()));
            }
            files.insert(file.clone(), text.clone());
            Ok(Patch::Delete { file: file.clone() })
        }
        Patch::Delete { file } => {
            let text = files
                .remove(file)
                .ok_or_else(|| EditError::NoSuchFile(file.clone()))?;
            Ok(Patch::Create {
                file: file.clone(),
                text,
            })
        }
        Patch::Rename { from, to } => {
            check(to)?;
            if files.contains_key(to) {
                return Err(EditError::FileExists(to.clone()));
            }
            let text = files
                .remove(from)
                .ok_or_else(|| EditError::NoSuchFile(from.clone()))?;
            files.insert(to.clone(), text);
            Ok(Patch::Rename {
                from: to.clone(),
                to: from.clone(),
            })
        }
    }
}

/// What saving does to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Write(FilePath, String),
    Remove(FilePath),
}

impl Change {
    pub fn path(&self) -> &str {
        match self {
            Change::Write(p, _) | Change::Remove(p) => p,
        }
    }
}

/// Every `.ilx` file under `root`, by its library path.
pub(crate) fn read_folder(root: &Path) -> std::io::Result<BTreeMap<FilePath, String>> {
    let mut files = BTreeMap::new();
    let mut todo = vec![(String::new(), root.to_path_buf())];
    while let Some((rel, dir)) = todo.pop() {
        for entry in std::fs::read_dir(&dir)? {
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
                files.insert(rel, std::fs::read_to_string(&path)?);
            }
        }
    }
    Ok(files)
}

/// What a new library starts from.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "from", rename_all = "kebab-case")]
pub enum Start {
    /// A lexicon with the standard tokens, a palette and one box.
    Empty,
    /// The `.ilx` files of another library, as they are on disk.
    Copy { of: PathBuf },
    /// The `.ilx` files of a library exported as a zip.
    Zip { of: PathBuf },
}

/// A new library in `root`, made if it is not there, from `start`; refused
/// if `root` holds a library already.
pub fn create(root: &Path, start: &Start) -> std::io::Result<()> {
    if root.exists() && !read_folder(root)?.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} holds a library already", root.display()),
        ));
    }
    let files: BTreeMap<FilePath, String> = match start {
        Start::Empty => [
            ("lexicon.ilx".to_string(), STARTER_LEXICON.to_string()),
            ("icons.ilx".to_string(), STARTER_ICONS.to_string()),
        ]
        .into_iter()
        .collect(),
        Start::Copy { of } => read_folder(of)?,
        Start::Zip { of } => crate::export::unzip(&std::fs::read(of)?)
            .map_err(|why| std::io::Error::new(std::io::ErrorKind::InvalidData, why))?,
    };
    for (path, text) in &files {
        let at = root.join(path);
        if let Some(dir) = at.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(at, text)?;
    }
    Ok(())
}

/// A new library's vocabulary and colors: isolex's standard tokens.
const STARTER_LEXICON: &str = r##"// What this library's icons paint with (SPEC §2), and its colors.
lexicon {
  tokens "ink" "ink-muted" "outline" "body-top" "body-left" "body-right" "shadow" "virt" "accent"
  stroke-widths 1.2 1.4
}

palette {
  ink light="#1f2933" dark="#e4e7eb"
  ink-muted light="#7b8794" dark="#9aa5b1"
  outline light="#1f2933" dark="#0b0d10"
  body-top light="#e4e7eb" dark="#52606d"
  body-left light="#cbd2d9" dark="#3e4c59"
  body-right light="#9aa5b1" dark="#323f4b"
  shadow light="#000000" dark="#000000"
  virt light="#d9e2ec" dark="#3e4c59"
  accent light="#d64545" dark="#ef8a8a"
}
"##;

/// A new library's first icon: one box to draw from.
const STARTER_ICONS: &str = r#"icon "box" {
  category "basic"
  footprint 1 1 1
  part "body" {
    box 0 0 0 1 1 1
  }
}
"#;
