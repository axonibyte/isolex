//! A lexicon's files as a folder holds them (SPEC §1): which files, and
//! in what order. The command line and the studio both read a folder
//! through here, so they read it the same way.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// SPEC §1's order of two paths relative to the lexicon's folder, with
/// `/` between their parts: directory by directory, each directory's
/// entries in byte order of their names, a subdirectory's files at the
/// place its name sorts. So `a/x.ilx` comes before `a-b/x.ilx`, which a
/// comparison of whole strings would put first.
pub fn compare(a: &str, b: &str) -> Ordering {
    a.split('/')
        .map(str::as_bytes)
        .cmp(b.split('/').map(str::as_bytes))
}

/// The paths in the order a lexicon is read (SPEC §1).
pub fn order<S: AsRef<str>>(mut paths: Vec<S>) -> Vec<S> {
    paths.sort_by(|a, b| compare(a.as_ref(), b.as_ref()));
    paths
}

/// Every `.ilx` file under `dir`, in the order a lexicon is read, with its
/// text: each named as `dir` joined with its path, which is how
/// diagnostics name it.
pub fn load(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let cannot = |e: std::io::Error| format!("cannot read {}: {e}", dir.display());
    // (path relative to dir, with / between its parts; where it is)
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let mut todo = vec![(String::new(), dir.to_path_buf())];
    while let Some((rel, at)) = todo.pop() {
        for entry in std::fs::read_dir(&at).map_err(cannot)? {
            let entry = entry.map_err(cannot)?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            if path.is_dir() {
                todo.push((rel, path));
            } else if path.extension().is_some_and(|e| e == "ilx") {
                found.push((rel, path));
            }
        }
    }
    if found.is_empty() {
        return Err(format!("no .ilx files under {}", dir.display()));
    }
    found.sort_by(|a, b| compare(&a.0, &b.0));
    let mut files = Vec::with_capacity(found.len());
    for (_, path) in found {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        files.push((path.display().to_string(), text));
    }
    Ok(files)
}
