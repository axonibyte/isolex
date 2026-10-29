//! A definition's parts, as the Parts panel lists them (#68,
//! docs/studio-interface.md, Parts): its own, in source order, each with
//! its shapes; and those it draws but does not own, from the definitions
//! it extends, each naming where it comes from. With them, what the
//! Design tab shows with nothing selected (#70): its frame, and its
//! params, own and inherited.
//!
//! Read from the source as written, so it is there while the library does
//! not compile, as far as the parser got.

use isolex::ast::{DefKind, Definition, Lexicon, Num, Param, PartDecl, PartOp, Span};
use serde::Serialize;

use crate::compile::Place;

/// One definition's parts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outline {
    pub definition: String,
    /// "base", "icon", "fragment", "axis" or "font".
    pub kind: String,
    pub extends: Option<String>,
    /// The definition's node, where a new part is inserted.
    pub node: Place,
    /// Its own parts, in source order; an axis's variants' after its own.
    pub parts: Vec<Part>,
    /// The parts it draws from the definitions it extends, nearest last,
    /// less those it replaces or removes.
    pub inherited: Vec<Inherited>,
    /// Its `footprint` or `canvas`, its own or the nearest it extends.
    pub frame: Option<Frame>,
    /// Its params: its own in the order written, then those it extends
    /// and does not declare again, nearest first.
    pub params: Vec<ParamRow>,
}

/// A definition's frame, as written.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Frame {
    /// "footprint" or "canvas".
    pub kind: String,
    pub args: Vec<String>,
    /// The definition that writes it, when not this one.
    pub from: Option<String>,
    pub node: Place,
}

/// A param, as written where it is declared.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParamRow {
    pub name: String,
    /// Its default as written: "12", "u * 0.28", or text.
    pub default: String,
    /// The default, when it is written as a number.
    pub value: Option<f64>,
    /// The definition that declares it, when not this one.
    pub from: Option<String>,
    pub node: Place,
}

/// A part the definition writes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Part {
    pub name: String,
    /// "part", "replace" or "remove", as written.
    pub op: String,
    /// The axis variant it is written in, for an axis.
    pub variant: Option<String>,
    pub node: Place,
    /// Its shapes, in source order (painter order: the last is on top).
    pub shapes: Vec<Row>,
}

/// One node in a part: a shape, or a node holding shapes (`at`,
/// `repeat`, `transform`, `turn`) with its own under it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// The node's name as written: "box", "facerect", "repeat"…
    pub kind: String,
    /// A short note on it: "2×1×0.5", "front", "×8", "3 points".
    pub note: String,
    pub node: Place,
    pub children: Vec<Row>,
}

/// A part drawn from a definition this one extends.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Inherited {
    pub name: String,
    /// The definition that writes it as drawn here.
    pub from: String,
    pub shapes: usize,
}

/// Nodes whose children are shapes; a polygon's are its points.
const HOLDERS: [&str; 4] = ["at", "repeat", "transform", "turn"];

/// The outline of `definition` in `files`, or none if there is no such
/// definition.
pub fn outline(files: &[(String, String)], definition: &str) -> Option<Outline> {
    let lexicon = isolex::parser::parse_lexicon(files).lexicon;
    let def = lexicon.defs.iter().find(|d| d.name == definition)?;
    let read = Reader {
        files,
        lexicon: &lexicon,
    };
    let mut parts: Vec<Part> = def.body.parts.iter().map(|p| read.part(p, None)).collect();
    for v in &def.variants {
        parts.extend(v.body.parts.iter().map(|p| read.part(p, Some(&v.name))));
    }
    let mut inherited = Vec::new();
    if let Some(parent) = def.extends.as_deref() {
        let mine = |name: &str| def.body.parts.iter().any(|p| p.name == name);
        for (name, (from, shapes)) in drawn(&lexicon, parent, &mut Vec::new()) {
            if !mine(&name) {
                inherited.push(Inherited { name, from, shapes });
            }
        }
    }
    // the chain, this one first; a cycle the resolver reports ends it
    let mut chain: Vec<&Definition> = vec![def];
    while let Some(parent) = chain.last().and_then(|d| d.extends.as_deref()) {
        match lexicon.defs.iter().find(|d| d.name == parent) {
            Some(d) if !chain.iter().any(|c| c.name == d.name) => chain.push(d),
            _ => break,
        }
    }
    let from = |d: &Definition| (d.name != def.name).then(|| d.name.clone());
    let frame = chain.iter().find_map(|d| {
        let node = lexicon.sources.get(d.span)?.children.iter().find_map(|c| {
            let src = lexicon.sources.get(*c)?;
            (src.name == "footprint" || src.name == "canvas").then_some((*c, src))
        })?;
        Some(Frame {
            kind: node.1.name.clone(),
            args: node.1.args.iter().map(|a| read.value(*a)).collect(),
            from: from(d),
            node: read.place(node.0),
        })
    });
    let mut params: Vec<ParamRow> = Vec::new();
    for d in &chain {
        for (name, param, span) in &d.body.params {
            if params.iter().any(|p| p.name == *name) {
                continue;
            }
            let default = lexicon
                .sources
                .get(*span)
                .and_then(|s| s.args.get(1))
                .map(|a| read.value(*a))
                .unwrap_or_default();
            params.push(ParamRow {
                name: name.clone(),
                default,
                value: match param {
                    Param::Num(Num::Lit(v)) => Some(*v),
                    _ => None,
                },
                from: from(d),
                node: read.place(*span),
            });
        }
    }
    Some(Outline {
        definition: def.name.clone(),
        kind: kind(def),
        extends: def.extends.clone(),
        node: read.place(def.span),
        parts,
        inherited,
        frame,
        params,
    })
}

fn kind(def: &Definition) -> String {
    match def.kind {
        DefKind::Base => "base",
        DefKind::Icon => "icon",
        DefKind::Fragment => "fragment",
        DefKind::Axis => "axis",
        DefKind::Font => "font",
    }
    .into()
}

/// The parts `name` draws, in order, each with the definition that writes
/// it as drawn and its shape count; `seen` stops a cycle, which the
/// resolver reports.
fn drawn(lexicon: &Lexicon, name: &str, seen: &mut Vec<String>) -> Vec<(String, (String, usize))> {
    let Some(def) = lexicon.defs.iter().find(|d| d.name == name) else {
        return Vec::new();
    };
    if seen.iter().any(|s| s == name) {
        return Vec::new();
    }
    seen.push(name.to_string());
    let mut parts = match def.extends.as_deref() {
        Some(parent) => drawn(lexicon, parent, seen),
        None => Vec::new(),
    };
    for p in &def.body.parts {
        let at = parts.iter().position(|(n, _)| *n == p.name);
        let entry = (p.name.clone(), (def.name.clone(), p.nodes.len()));
        match (p.op, at) {
            (PartOp::Remove, Some(i)) => {
                parts.remove(i);
            }
            (PartOp::Remove, None) => {}
            (_, Some(i)) => parts[i] = entry,
            (_, None) => parts.push(entry),
        }
    }
    parts
}

struct Reader<'a> {
    files: &'a [(String, String)],
    lexicon: &'a Lexicon,
}

impl Reader<'_> {
    fn place(&self, span: Span) -> Place {
        Place {
            file: self
                .files
                .get(span.file)
                .map(|f| f.0.clone())
                .unwrap_or_default(),
            start: span.start,
            len: span.len,
        }
    }

    fn text(&self, span: Span) -> &str {
        self.files
            .get(span.file)
            .and_then(|f| f.1.get(span.start..span.start + span.len))
            .unwrap_or("")
    }

    fn part(&self, p: &PartDecl, variant: Option<&str>) -> Part {
        let shapes = match self.lexicon.sources.get(p.span) {
            Some(src) => src.children.iter().filter_map(|c| self.row(*c)).collect(),
            None => Vec::new(),
        };
        Part {
            name: p.name.clone(),
            op: match p.op {
                PartOp::Add => "part",
                PartOp::Replace => "replace",
                PartOp::Remove => "remove",
            }
            .into(),
            variant: variant.map(str::to_string),
            node: self.place(p.span),
            shapes,
        }
    }

    fn row(&self, span: Span) -> Option<Row> {
        let src = self.lexicon.sources.get(span)?;
        let children = if HOLDERS.contains(&src.name.as_str()) {
            src.children.iter().filter_map(|c| self.row(*c)).collect()
        } else {
            Vec::new()
        };
        let args: Vec<String> = src.args.iter().map(|a| self.value(*a)).collect();
        // a polygon's points, unless a repeat writes some
        let named = |n: &str| {
            src.children
                .iter()
                .filter(|c| self.lexicon.sources.get(**c).is_some_and(|c| c.name == n))
                .count()
        };
        let points = (named("repeat") == 0).then(|| named("pt"));
        Some(Row {
            note: note(&src.name, &args, points),
            kind: src.name.clone(),
            node: self.place(span),
            children,
        })
    }

    /// A value as the note writes it: a string without its quotes.
    fn value(&self, span: Span) -> String {
        let t = self.text(span).trim();
        t.strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .unwrap_or(t)
            .to_string()
    }
}

/// A short note on a node, from its arguments as written.
fn note(kind: &str, args: &[String], points: Option<usize>) -> String {
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("?");
    let size = |from: usize, n: usize| (from..from + n).map(arg).collect::<Vec<_>>().join("×");
    match kind {
        "box" => size(3, 3),
        "tile" => size(3, 2),
        "shadow" => size(2, 2),
        "cylinder" => format!("r {} · h {}", arg(3), arg(4)),
        "facerect" => arg(0).to_string(),
        "repeat" => format!("×{}", arg(0)),
        "at" => format!("at {} {} {}", arg(0), arg(1), arg(2)),
        "circle" => format!("r {}", arg(2)),
        "ellipse" => format!("{}×{}", arg(2), arg(3)),
        "rect" => size(2, 2),
        "insert" | "badge" | "text" => arg(0).to_string(),
        "poly" | "poly3d" => match points {
            Some(1) => "1 point".into(),
            Some(n) => format!("{n} points"),
            None => "points by repeat".into(),
        },
        _ => String::new(),
    }
}
