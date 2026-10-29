//! Renaming definitions, and creating, moving and renaming files and
//! definitions (#65): each a library change like any other, one undoable
//! step, saved with the rest.

use std::collections::{BTreeMap, BTreeSet};

use isolex::kdl::{self, Document, Value};

use crate::library::{is_library_path, FilePath, Library, Patch, Transaction};

/// Why a refactoring was refused, with nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefactorError {
    NoSuchDefinition(String),
    /// The name is taken, by a definition or an icon one emits; `free` is
    /// the first `name-2`, `name-3`, … that is not.
    NameTaken {
        name: String,
        free: String,
    },
    NoSuchFile(FilePath),
    /// A file of that path already exists; `free` is a path that does not.
    FileExists {
        path: FilePath,
        free: FilePath,
    },
    BadPath(FilePath),
    /// A file that does not parse, so its definitions cannot be found.
    Unreadable(FilePath, String),
}

impl std::fmt::Display for RefactorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefactorError::NoSuchDefinition(n) => write!(f, "the library has no definition '{n}'"),
            RefactorError::NameTaken { name, free } => {
                write!(f, "'{name}' is taken; '{free}' is free")
            }
            RefactorError::NoSuchFile(p) => write!(f, "the library has no file {p}"),
            RefactorError::FileExists { path, free } => {
                write!(f, "{path} is taken; {free} is free")
            }
            RefactorError::BadPath(p) => write!(f, "{p:?} is not a path for a file in the library"),
            RefactorError::Unreadable(p, why) => write!(f, "{p} does not parse: {why}"),
        }
    }
}

/// A name as the library writes names: quoted, though KDL would take some
/// bare.
fn quoted(name: &str) -> String {
    let w = kdl::write_value(&Value::String(name.to_string()));
    if w.starts_with('"') || w.starts_with('#') {
        w
    } else {
        // bare, so it holds nothing a quoted string must escape
        format!("\"{w}\"")
    }
}

/// Every name in use: each definition's and each icon's it emits.
fn names_in_use(graph: &isolex::graph::Graph) -> BTreeSet<String> {
    graph
        .definitions
        .iter()
        .flat_map(|d| std::iter::once(d.name.clone()).chain(d.emits.iter().cloned()))
        .collect()
}

/// The first of `name-2`, `name-3`, … that `taken` does not hold.
fn free_name(name: &str, taken: &BTreeSet<String>) -> String {
    (2..)
        .map(|n| format!("{name}-{n}"))
        .find(|c| !taken.contains(c))
        .expect("some number is free")
}

/// Renames definition `from` to `to`, rewriting its name and every
/// `extends`, `insert`, `badge`, `axes` and text `font` that names it, in
/// every file, as one transaction.
pub fn rename_definition(
    lib: &Library,
    from: &str,
    to: &str,
) -> Result<Transaction, RefactorError> {
    let sources = lib.sources();
    let graph = isolex::graph(&sources);
    let def = graph
        .definitions
        .iter()
        .find(|d| d.name == from)
        .ok_or_else(|| RefactorError::NoSuchDefinition(from.to_string()))?;
    let taken = names_in_use(&graph);
    if to != from && taken.contains(to) {
        return Err(RefactorError::NameTaken {
            name: to.to_string(),
            free: free_name(to, &taken),
        });
    }
    let written = quoted(to);
    // the definition's own name: its first argument
    let file = &sources[def.span.file].0;
    let text = &lib.files()[file];
    let doc =
        Document::parse(text).map_err(|e| RefactorError::Unreadable(file.clone(), e.message))?;
    let node = doc
        .nodes()
        .find(|n| n.span().offset() == def.span.start)
        .ok_or_else(|| RefactorError::NoSuchDefinition(from.to_string()))?;
    let name_at = node
        .entries()
        .iter()
        .find(|e| e.name().is_none())
        .map(|e| e.value_span())
        .ok_or_else(|| RefactorError::NoSuchDefinition(from.to_string()))?;
    let mut sites: Vec<(FilePath, usize, usize)> =
        vec![(file.clone(), name_at.offset(), name_at.len())];
    for e in graph.edges.iter().filter(|e| e.to == from) {
        sites.push((sources[e.at.file].0.clone(), e.at.start, e.at.len));
    }
    // last first in each file, so no splice moves another's bytes
    sites.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    sites.dedup();
    let patches = sites
        .into_iter()
        .map(|(file, at, len)| Patch::Splice {
            file,
            at,
            len,
            text: written.clone(),
        })
        .collect();
    Ok(Transaction::new(format!("Rename {from} to {to}"), patches))
}

/// The first path like `path` (`name-2.ilx`, …) the library does not have.
fn free_path(path: &str, files: &BTreeMap<FilePath, String>) -> FilePath {
    let stem = path.strip_suffix(".ilx").unwrap_or(path);
    (2..)
        .map(|n| format!("{stem}-{n}.ilx"))
        .find(|c| !files.contains_key(c))
        .expect("some number is free")
}

fn check_new_path(lib: &Library, path: &str) -> Result<(), RefactorError> {
    if !is_library_path(path) {
        return Err(RefactorError::BadPath(path.to_string()));
    }
    if lib.files().contains_key(path) {
        return Err(RefactorError::FileExists {
            path: path.to_string(),
            free: free_path(path, lib.files()),
        });
    }
    Ok(())
}

/// A new file.
pub fn create_file(lib: &Library, path: &str, text: &str) -> Result<Transaction, RefactorError> {
    check_new_path(lib, path)?;
    Ok(Transaction::new(
        format!("New file {path}"),
        vec![Patch::Create {
            file: path.to_string(),
            text: text.to_string(),
        }],
    ))
}

/// A font's source added to the library, as a file of its own under
/// `fonts/` named for it (the first free one); refused if its name is
/// taken, by a definition or an icon one emits.
pub fn add_font(lib: &Library, text: &str) -> Result<Transaction, RefactorError> {
    let name = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("font \""))
        .and_then(|rest| rest.split('"').next())
        .ok_or_else(|| RefactorError::Unreadable("the font".into(), "it names no font".into()))?
        .to_string();
    let taken = names_in_use(&isolex::graph(&lib.sources()));
    if taken.contains(&name) {
        return Err(RefactorError::NameTaken {
            free: free_name(&name, &taken),
            name,
        });
    }
    let wanted = format!("fonts/{name}.ilx");
    let path = if lib.files().contains_key(&wanted) {
        free_path(&wanted, lib.files())
    } else {
        wanted
    };
    check_new_path(lib, &path)?;
    Ok(Transaction::new(
        format!("Import font {name}"),
        vec![Patch::Create {
            file: path,
            text: text.to_string(),
        }],
    ))
}

/// A file moved or renamed: its definitions keep their names, so nothing
/// that refers to them changes.
pub fn move_file(lib: &Library, from: &str, to: &str) -> Result<Transaction, RefactorError> {
    if !lib.files().contains_key(from) {
        return Err(RefactorError::NoSuchFile(from.to_string()));
    }
    check_new_path(lib, to)?;
    Ok(Transaction::new(
        format!("Move {from} to {to}"),
        vec![Patch::Rename {
            from: from.to_string(),
            to: to.to_string(),
        }],
    ))
}

/// What a new definition starts as: as little as compiles.
fn template(kind: &str, name: &str) -> String {
    let n = quoted(name);
    match kind {
        "icon" => format!("icon {n} {{\n  category \"new\"\n  footprint 1 1 1\n  part \"body\" {{\n    box 0 0 0 1 1 1\n  }}\n}}\n"),
        other => format!("{other} {n} {{\n}}\n"),
    }
}

/// A new definition of `kind` (icon, base, fragment, axis or font) at the
/// end of `file`, which is made if it is not there.
pub fn new_definition(
    lib: &Library,
    file: &str,
    kind: &str,
    name: &str,
) -> Result<Transaction, RefactorError> {
    let taken = names_in_use(&isolex::graph(&lib.sources()));
    if taken.contains(name) {
        return Err(RefactorError::NameTaken {
            name: name.to_string(),
            free: free_name(name, &taken),
        });
    }
    let body = template(kind, name);
    let label = format!("New {kind} {name}");
    match lib.files().get(file) {
        Some(text) => {
            let sep = if text.is_empty() || text.ends_with("\n\n") {
                ""
            } else if text.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            Ok(Transaction::new(
                label,
                vec![Patch::Splice {
                    file: file.to_string(),
                    at: text.len(),
                    len: 0,
                    text: format!("{sep}{body}"),
                }],
            ))
        }
        None => {
            check_new_path(lib, file)?;
            Ok(Transaction::new(
                label,
                vec![Patch::Create {
                    file: file.to_string(),
                    text: body,
                }],
            ))
        }
    }
}

/// A definition moved to the end of another file (made if it is not
/// there), with its comments on the lines just above it.
pub fn move_definition(lib: &Library, name: &str, to: &str) -> Result<Transaction, RefactorError> {
    let sources = lib.sources();
    let graph = isolex::graph(&sources);
    let def = graph
        .definitions
        .iter()
        .find(|d| d.name == name)
        .ok_or_else(|| RefactorError::NoSuchDefinition(name.to_string()))?;
    let from = sources[def.span.file].0.clone();
    if from == to {
        return Ok(Transaction::new(format!("Move {name} to {to}"), Vec::new()));
    }
    let text = &lib.files()[&from];
    // its lines, with the comment lines right above them
    let start = def.span.start;
    let end = def.span.start + def.span.len;
    let mut line = text[..start].rfind('\n').map_or(0, |i| i + 1);
    while line > 0 {
        let prev = text[..line - 1].rfind('\n').map_or(0, |i| i + 1);
        if text[prev..line].trim_start().starts_with("//") {
            line = prev;
        } else {
            break;
        }
    }
    let mut eol = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
    // and one blank line after it, so the gap it leaves closes
    if text[eol..].starts_with('\n') {
        eol += 1;
    } else if text[eol..].starts_with("\r\n") {
        eol += 2;
    }
    let moved = text[line..eol].trim_end().to_string() + "\n";
    let mut patches = vec![Patch::Splice {
        file: from.clone(),
        at: line,
        len: eol - line,
        text: String::new(),
    }];
    match lib.files().get(to) {
        Some(dest) => {
            let sep = if dest.is_empty() || dest.ends_with("\n\n") {
                ""
            } else if dest.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            patches.push(Patch::Splice {
                file: to.to_string(),
                at: dest.len(),
                len: 0,
                text: format!("{sep}{moved}"),
            });
        }
        None => {
            check_new_path(lib, to)?;
            patches.push(Patch::Create {
                file: to.to_string(),
                text: moved,
            });
        }
    }
    Ok(Transaction::new(format!("Move {name} to {to}"), patches))
}
