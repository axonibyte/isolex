//! The source-edit engine (#64): the interface never writes text. It asks
//! for an [`Intent`] (set a value, insert a node, delete one, move one,
//! type), and [`plan`] turns it into a [`Transaction`] of patches that
//! change only the bytes the intent means to, keeping comments, order and
//! formatting; or, when the intent could mean more than one thing, into
//! the [`Choice`]s the interface offers, each a request ready to send.
//!
//! - **A literal** is spliced with `isolex::kdl::write_value`; one whose
//!   value would not change keeps its spelling.
//! - **An expression** (a number written as a string) is never replaced
//!   silently: the choices are to replace it with a number, and, when it
//!   depends on one param of the definition and linearly, to edit that
//!   param's default where it is declared so the expression comes out
//!   at the value asked for.
//! - **A node of another definition** (inherited by the one being
//!   edited) is not changed silently either: the choices are to edit it
//!   in that definition, naming the icons that follow, or to override
//!   its part in the one being edited.

use std::collections::{BTreeMap, BTreeSet};

use isolex::kdl::{self, Document, Value};

use crate::library::{FilePath, Library, Patch, Transaction};

/// A node of the library, by the file and bytes of its source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeRef {
    pub file: FilePath,
    pub start: usize,
    pub len: usize,
}

/// Where in a node a value is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    /// Its `i`th argument.
    Arg(usize),
    /// Its property of this name (the first, as isolex reads one).
    Prop(String),
}

/// What the interface asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// The value in a slot; a property that is not there is added.
    SetValue {
        node: NodeRef,
        slot: Slot,
        value: Value,
    },
    RemoveProp {
        node: NodeRef,
        key: String,
    },
    /// A node's source (one or more lines) as a child of `parent`, before
    /// the child `before`, or last.
    Insert {
        parent: NodeRef,
        text: String,
        before: Option<NodeRef>,
    },
    Delete {
        node: NodeRef,
    },
    /// A node moved before a sibling, or to the end of its block.
    MoveBefore {
        node: NodeRef,
        before: Option<NodeRef>,
    },
    /// Text typed in the source: `len` bytes of `file` at `at` replaced.
    Type {
        file: FilePath,
        at: usize,
        len: usize,
        text: String,
    },
}

/// An intent, with what it is asked against.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// The library's revision the intent's spans come from.
    pub revision: u64,
    /// The definition open in the editor, if any: a node of another one is
    /// inherited.
    pub editing: Option<String>,
    pub intent: Intent,
    /// The undo history's name for it ("Move box in server-2u").
    pub label: String,
    /// Change an inherited node where it is written, without asking.
    pub in_base: bool,
    /// Replace an expression with the literal asked for, without asking.
    pub replace_expression: bool,
}

impl Request {
    pub fn new(lib: &Library, editing: Option<&str>, label: &str, intent: Intent) -> Request {
        Request {
            revision: lib.revision(),
            editing: editing.map(str::to_string),
            intent,
            label: label.to_string(),
            in_base: false,
            replace_expression: false,
        }
    }
}

/// What a request comes to.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Patches to apply (none when nothing would change).
    Apply(Transaction),
    /// What the interface asks the person, each with the request that
    /// carries it out.
    Choose(Vec<Choice>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    /// Set the param `param` (declared in `definition`) to `value`, which
    /// makes the expression come out at the value asked for.
    EditParam {
        param: String,
        definition: String,
        value: f64,
        then: Request,
    },
    ReplaceWithNumber {
        then: Request,
    },
    /// Change the node where it is written, in `base`, which `affected`
    /// (icons other than the one being edited) draw too.
    EditInBase {
        base: String,
        affected: Vec<String>,
        then: Request,
    },
    /// Copy the node's part into the definition being edited, as a
    /// `replace`, to change it there.
    OverrideHere {
        part: String,
        then: Request,
    },
    /// Declare the param `param` again in `definition`, the one being
    /// edited, with the value asked for: it and what extends it draw with
    /// it, and the base keeps its own.
    DeclareHere {
        param: String,
        definition: String,
        then: Request,
    },
}

/// Why a request was refused, with nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The library changed since the request's spans were taken.
    Stale {
        asked: u64,
        now: u64,
    },
    NoSuchFile(FilePath),
    /// The file does not parse, so its nodes cannot be found.
    Unreadable(FilePath, String),
    NoSuchNode(NodeRef),
    NoSuchSlot(NodeRef, Slot),
    /// A move to a node that is not the moved node's sibling.
    NotASibling(NodeRef),
    /// A move of a node written on a line with others.
    SharesItsLine(NodeRef),
    BadRange,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Stale { asked, now } => write!(
                f,
                "the library changed since this edit was asked for (revision {asked}, now {now})"
            ),
            Refusal::NoSuchFile(p) => write!(f, "the library has no file {p}"),
            Refusal::Unreadable(p, why) => write!(f, "{p} does not parse: {why}"),
            Refusal::NoSuchNode(n) => write!(f, "{} has no node at {}", n.file, n.start),
            Refusal::NoSuchSlot(n, s) => {
                write!(f, "the node at {}:{} has no {s:?}", n.file, n.start)
            }
            Refusal::NotASibling(n) => {
                write!(f, "the node at {}:{} is not a sibling", n.file, n.start)
            }
            Refusal::SharesItsLine(n) => write!(
                f,
                "the node at {}:{} shares its line with others, so it is not moved",
                n.file, n.start
            ),
            Refusal::BadRange => f.write_str("there is no such text to replace"),
        }
    }
}

/// Plans `req` against `lib`.
pub fn plan(lib: &Library, req: &Request) -> Result<Plan, Refusal> {
    if req.revision != lib.revision() {
        return Err(Refusal::Stale {
            asked: req.revision,
            now: lib.revision(),
        });
    }
    let target = match &req.intent {
        Intent::SetValue { node, .. }
        | Intent::RemoveProp { node, .. }
        | Intent::Delete { node }
        | Intent::MoveBefore { node, .. } => Some(node),
        Intent::Insert { parent, .. } => Some(parent),
        Intent::Type { .. } => None,
    };
    if let (Some(node), Some(editing), false) = (target, &req.editing, req.in_base) {
        if let Some(choices) = inherited(lib, req, node, editing)? {
            return Ok(Plan::Choose(choices));
        }
    }
    match &req.intent {
        Intent::SetValue { node, slot, value } => set_value(lib, req, node, slot, value),
        Intent::RemoveProp { node, key } => {
            let (src, doc) = parse(lib, &node.file)?;
            let n = find(&doc, node)?;
            let entry = n
                .node
                .entries()
                .iter()
                .find(|e| e.name().map(|k| k.value()) == Some(key.as_str()))
                .ok_or_else(|| Refusal::NoSuchSlot(node.clone(), Slot::Prop(key.clone())))?;
            let start = entry.span().offset();
            let end = start + entry.span().len();
            // with the space before it
            let from = src[..start].trim_end_matches([' ', '\t']).len();
            Ok(apply(&req.label, splice(&node.file, src, from, end, "")))
        }
        Intent::Insert {
            parent,
            text,
            before,
        } => insert(lib, req, parent, text, before.as_ref()),
        Intent::Delete { node } => {
            let (src, doc) = parse(lib, &node.file)?;
            let n = find(&doc, node)?;
            let (from, to) = whole_lines(src, n.start, n.end);
            Ok(apply(&req.label, splice(&node.file, src, from, to, "")))
        }
        Intent::MoveBefore { node, before } => move_before(lib, req, node, before.as_ref()),
        Intent::Type {
            file,
            at,
            len,
            text,
        } => {
            let src = lib
                .files()
                .get(file)
                .ok_or_else(|| Refusal::NoSuchFile(file.clone()))?;
            let end = at.checked_add(*len).ok_or(Refusal::BadRange)?;
            if end > src.len() || !src.is_char_boundary(*at) || !src.is_char_boundary(end) {
                return Err(Refusal::BadRange);
            }
            Ok(apply(&req.label, splice(file, src, *at, end, text)))
        }
    }
}

fn apply(label: &str, patches: Vec<Patch>) -> Plan {
    Plan::Apply(Transaction::new(label, patches))
}

/// A splice of `file`'s `from..to` with `text`, or none when it would
/// change nothing.
fn splice(file: &str, src: &str, from: usize, to: usize, text: &str) -> Vec<Patch> {
    if &src[from..to] == text {
        return Vec::new();
    }
    vec![Patch::Splice {
        file: file.to_string(),
        at: from,
        len: to - from,
        text: text.to_string(),
    }]
}

// --- finding nodes --------------------------------------------------------

/// A node of a parsed file, with where it sits.
struct Found<'d> {
    node: kdl::NodeRef<'d>,
    start: usize,
    end: usize,
    /// Its parent's index in the flat list, and the top-level node (the
    /// definition) it is in.
    parent: Option<usize>,
    top: usize,
}

fn parse<'l>(lib: &'l Library, file: &str) -> Result<(&'l str, Document), Refusal> {
    let src = lib
        .files()
        .get(file)
        .ok_or_else(|| Refusal::NoSuchFile(file.to_string()))?;
    let doc = Document::parse(src).map_err(|e| Refusal::Unreadable(file.to_string(), e.message))?;
    Ok((src.as_str(), doc))
}

/// Every node of `doc`, parents before children, without recursion.
fn flatten(doc: &Document) -> Vec<Found<'_>> {
    let mut out: Vec<Found> = Vec::new();
    let mut todo: Vec<(kdl::NodeRef, Option<usize>, Option<usize>)> =
        doc.nodes().map(|n| (n, None, None)).collect();
    todo.reverse();
    while let Some((node, parent, top)) = todo.pop() {
        let idx = out.len();
        let span = node.span();
        out.push(Found {
            node,
            start: span.offset(),
            end: span.offset() + span.len(),
            parent,
            top: top.unwrap_or(idx),
        });
        let top = Some(top.unwrap_or(idx));
        if let Some(children) = node.children() {
            let kids: Vec<_> = children.nodes().collect();
            for child in kids.into_iter().rev() {
                todo.push((child, Some(idx), top));
            }
        }
    }
    out
}

fn find<'d>(doc: &'d Document, at: &NodeRef) -> Result<Found<'d>, Refusal> {
    flatten(doc)
        .into_iter()
        .find(|f| f.start == at.start && f.end - f.start == at.len)
        .ok_or_else(|| Refusal::NoSuchNode(at.clone()))
}

/// The name of the definition a top-level node is (`icon "server-2u"`).
fn definition_name(node: &kdl::NodeRef) -> Option<String> {
    node.entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_string())
        .map(str::to_string)
}

// --- inherited nodes --------------------------------------------------------

/// The choices for a node of a definition other than the one being edited,
/// or none when it is the one being edited (or no definition).
fn inherited(
    lib: &Library,
    req: &Request,
    node: &NodeRef,
    editing: &str,
) -> Result<Option<Vec<Choice>>, Refusal> {
    let (src, doc) = parse(lib, &node.file)?;
    let flat = flatten(&doc);
    let Some(idx) = flat
        .iter()
        .position(|f| f.start == node.start && f.end - f.start == node.len)
    else {
        return Err(Refusal::NoSuchNode(node.clone()));
    };
    let top = &flat[flat[idx].top];
    let Some(owner) = definition_name(&top.node) else {
        return Ok(None);
    };
    if owner == editing {
        return Ok(None);
    }
    let mut choices = vec![Choice::EditInBase {
        base: owner.clone(),
        affected: following(lib, &owner, editing),
        then: Request {
            in_base: true,
            ..req.clone()
        },
    }];
    // a param's default: declared again where it is being edited, before
    // that definition's parts
    let f = &flat[idx];
    if f.parent == Some(f.top) && f.node.name().value() == "param" {
        if let (
            Intent::SetValue {
                slot: Slot::Arg(1),
                value,
                ..
            },
            Some(param),
            Some(here),
        ) = (
            &req.intent,
            definition_name(&f.node),
            definition_at(lib, editing),
        ) {
            choices.push(Choice::DeclareHere {
                param: param.clone(),
                definition: editing.to_string(),
                then: Request {
                    revision: req.revision,
                    editing: Some(editing.to_string()),
                    intent: Intent::Insert {
                        before: first_part(lib, &here),
                        parent: here,
                        text: format!("param {param} {}", kdl::write_value(&tidy(value))),
                    },
                    label: format!("Declare {param} in {editing}"),
                    in_base: false,
                    replace_expression: false,
                },
            });
        }
    }
    // the part it is in, copied into the definition being edited
    let mut at = Some(idx);
    while let Some(i) = at {
        let f = &flat[i];
        if f.node.name().value() == "part" && f.parent == Some(flat[idx].top) {
            if let (Some(name), Some(here)) =
                (definition_name(&f.node), definition_at(lib, editing))
            {
                let body = &src[f.start..f.end];
                let copy = format!("replace{}", &body["part".len()..]);
                let label = format!("Override part \"{name}\" in {editing}");
                choices.push(Choice::OverrideHere {
                    part: name,
                    then: Request {
                        revision: req.revision,
                        editing: Some(editing.to_string()),
                        intent: Intent::Insert {
                            parent: here,
                            text: dedent(&copy),
                            before: None,
                        },
                        label,
                        in_base: true,
                        replace_expression: false,
                    },
                });
            }
            break;
        }
        at = f.parent;
    }
    Ok(Some(choices))
}

/// The first part a definition writes, where a param declared in it
/// goes before; none if it writes none.
fn first_part(lib: &Library, def: &NodeRef) -> Option<NodeRef> {
    let (_, doc) = parse(lib, &def.file).ok()?;
    let flat = flatten(&doc);
    let top = flat
        .iter()
        .position(|f| f.start == def.start && f.end - f.start == def.len)?;
    flat.iter()
        .find(|f| f.parent == Some(top) && f.node.name().value() == "part")
        .map(|f| NodeRef {
            file: def.file.clone(),
            start: f.start,
            len: f.end - f.start,
        })
}

/// Where a definition is written.
fn definition_at(lib: &Library, name: &str) -> Option<NodeRef> {
    for (file, src) in lib.files() {
        let Ok(doc) = Document::parse(src) else {
            continue;
        };
        for n in doc.nodes() {
            if definition_name(&n).as_deref() == Some(name)
                && matches!(
                    n.name().value(),
                    "icon" | "base" | "fragment" | "axis" | "font"
                )
            {
                let s = n.span();
                return Some(NodeRef {
                    file: file.clone(),
                    start: s.offset(),
                    len: s.len(),
                });
            }
        }
    }
    None
}

/// The icons other than `editing` that draw what `base` defines: those
/// that extend it, however far down, and itself if it is one.
fn following(lib: &Library, base: &str, editing: &str) -> Vec<String> {
    let graph = isolex::graph(&lib.sources());
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in &graph.edges {
        if e.kind == isolex::graph::EdgeKind::Extends {
            children
                .entry(e.to.as_str())
                .or_default()
                .push(e.from.as_str());
        }
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut todo = vec![base];
    while let Some(d) = todo.pop() {
        if seen.insert(d) {
            todo.extend(children.get(d).into_iter().flatten().copied());
        }
    }
    graph
        .definitions
        .iter()
        .filter(|d| seen.contains(d.name.as_str()) && d.name != editing)
        .flat_map(|d| d.emits.iter().cloned())
        .filter(|n| n != editing)
        .collect()
}

// --- values ----------------------------------------------------------------

fn set_value(
    lib: &Library,
    req: &Request,
    node: &NodeRef,
    slot: &Slot,
    value: &Value,
) -> Result<Plan, Refusal> {
    let (src, doc) = parse(lib, &node.file)?;
    let n = find(&doc, node)?;
    let entry = match slot {
        Slot::Arg(i) => n
            .node
            .entries()
            .iter()
            .filter(|e| e.name().is_none())
            .nth(*i),
        Slot::Prop(k) => n
            .node
            .entries()
            .iter()
            .find(|e| e.name().map(|x| x.value()) == Some(k.as_str())),
    };
    let value = tidy(value);
    let Some(entry) = entry else {
        let Slot::Prop(key) = slot else {
            return Err(Refusal::NoSuchSlot(node.clone(), slot.clone()));
        };
        // a property added after the node's last entry (or its name)
        let after = n
            .node
            .entries()
            .last()
            .map(|e| e.span().offset() + e.span().len())
            .unwrap_or(n.node.name().span().offset() + n.node.name().span().len());
        let text = format!(" {key}={}", kdl::write_value(&value));
        return Ok(apply(
            &req.label,
            splice(&node.file, src, after, after, &text),
        ));
    };
    let span = entry.value_span();
    let (from, to) = (span.offset(), span.offset() + span.len());
    // a number asked for where an expression is written
    if let (Some(target), Value::String(expr)) = (number(&value), entry.value()) {
        if !req.replace_expression {
            return Ok(Plan::Choose(expression_choices(
                lib, req, node, expr, target,
            )));
        }
    }
    if same(entry.value(), &value) {
        return Ok(apply(&req.label, Vec::new()));
    }
    Ok(apply(
        &req.label,
        splice(&node.file, src, from, to, &kdl::write_value(&value)),
    ))
}

/// A whole number as an integer, so `2` is written `2`, not `2.0`.
fn tidy(v: &Value) -> Value {
    match v {
        Value::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => Value::Integer(*f as i128),
        other => other.clone(),
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Float(f) => Some(*f),
        Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

fn same(a: &Value, b: &Value) -> bool {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// Replace with a number; and, when the expression depends linearly on
/// one param of the definition the node is in (or one it extends) whose
/// default is a number, set that param so it comes out at `target`.
fn expression_choices(
    lib: &Library,
    req: &Request,
    node: &NodeRef,
    expr: &str,
    target: f64,
) -> Vec<Choice> {
    let mut choices = Vec::new();
    if let Some(choice) = edit_param(lib, req, node, expr, target) {
        choices.push(choice);
    }
    choices.push(Choice::ReplaceWithNumber {
        then: Request {
            replace_expression: true,
            ..req.clone()
        },
    });
    choices
}

fn edit_param(
    lib: &Library,
    req: &Request,
    node: &NodeRef,
    expr: &str,
    target: f64,
) -> Option<Choice> {
    let parsed = isolex::expr::parse_num(expr).ok()?;
    let eval = |name: &str, v: f64| {
        let env: BTreeMap<String, f64> = [(name.to_string(), v)].into_iter().collect();
        isolex::expr::eval(&parsed, &env).ok()
    };
    // the definition the node is in, and those it extends
    let (_, doc) = parse(lib, &node.file).ok()?;
    let flat = flatten(&doc);
    let here = flat
        .iter()
        .find(|f| f.start == node.start && f.end - f.start == node.len)?;
    let owner = definition_name(&flat[here.top].node)?;
    for (definition, param, at) in params_up_from(lib, &owner) {
        // the expression must read this param and nothing else, linearly
        let (Some(y0), Some(y1), Some(y2)) =
            (eval(&param, 0.0), eval(&param, 1.0), eval(&param, 2.0))
        else {
            continue;
        };
        let slope = y1 - y0;
        if slope == 0.0 || ((y2 - y1) - slope).abs() > 1e-9 * slope.abs().max(1.0) {
            continue;
        }
        let value = (target - y0) / slope;
        let then = Request {
            revision: req.revision,
            editing: req.editing.clone(),
            intent: Intent::SetValue {
                node: at,
                slot: Slot::Arg(1),
                value: Value::Float(round_to(value, 1e-6)),
            },
            label: format!("Set param {param} in {definition}"),
            in_base: true,
            replace_expression: false,
        };
        return Some(Choice::EditParam {
            param,
            definition,
            value,
            then,
        });
    }
    None
}

fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// Each param with a number for its default, nearest first: `owner`'s own,
/// then those of what it extends, as (definition, param, its node).
fn params_up_from(lib: &Library, owner: &str) -> Vec<(String, String, NodeRef)> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut at = Some(owner.to_string());
    while let Some(name) = at.take() {
        if !seen.insert(name.clone()) {
            break;
        }
        let Some(def) = definition_at(lib, &name) else {
            break;
        };
        let Ok((_, doc)) = parse(lib, &def.file) else {
            break;
        };
        let flat = flatten(&doc);
        let Some(top) = flat
            .iter()
            .find(|f| f.start == def.start && f.end - f.start == def.len)
        else {
            break;
        };
        let top_idx = flat.iter().position(|f| f.start == top.start).unwrap_or(0);
        for f in flat.iter().filter(|f| f.parent == Some(top_idx)) {
            let name_of = f.node.name().value();
            if name_of == "param" {
                let args: Vec<_> = f
                    .node
                    .entries()
                    .iter()
                    .filter(|e| e.name().is_none())
                    .collect();
                if let [p, v] = args.as_slice() {
                    if let (Some(p), Some(_)) = (p.value().as_string(), number(v.value())) {
                        if !out
                            .iter()
                            .any(|(_, q, _): &(String, String, NodeRef)| q == p)
                        {
                            out.push((
                                name.clone(),
                                p.to_string(),
                                NodeRef {
                                    file: def.file.clone(),
                                    start: f.start,
                                    len: f.end - f.start,
                                },
                            ));
                        }
                    }
                }
            }
            if name_of == "extends" {
                at = f
                    .node
                    .entries()
                    .first()
                    .and_then(|e| e.value().as_string())
                    .map(str::to_string);
            }
        }
        if at.is_none() {
            at = top
                .node
                .entries()
                .iter()
                .find(|e| e.name().map(|k| k.value()) == Some("extends"))
                .and_then(|e| e.value().as_string())
                .map(str::to_string);
        }
    }
    out
}

// --- inserting, deleting and moving ------------------------------------------

/// The leading whitespace of the line `at` is on.
fn indent_at(src: &str, at: usize) -> &str {
    let line = src[..at].rfind('\n').map_or(0, |i| i + 1);
    let rest = &src[line..];
    &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()]
}

fn newline(src: &str) -> &'static str {
    if src.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// One indentation step, as the file writes them.
fn unit(src: &str) -> &'static str {
    if src.lines().any(|l| l.starts_with('\t')) {
        "\t"
    } else {
        "  "
    }
}

/// `text` with the indentation its lines share taken off.
fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let common = lines
        .iter()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(
                l.get(common.min(l.len() - l.trim_start().len())..)
                    .unwrap_or(""),
            );
        } else {
            out.push_str(l.trim_start());
        }
    }
    out
}

/// `text` indented by `indent` on every line, with `nl` between lines.
fn indented(text: &str, indent: &str, nl: &str) -> String {
    text.lines()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                format!("{indent}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join(nl)
}

fn insert(
    lib: &Library,
    req: &Request,
    parent: &NodeRef,
    text: &str,
    before: Option<&NodeRef>,
) -> Result<Plan, Refusal> {
    let (src, doc) = parse(lib, &parent.file)?;
    let flat = flatten(&doc);
    let p_idx = flat
        .iter()
        .position(|f| f.start == parent.start && f.end - f.start == parent.len)
        .ok_or_else(|| Refusal::NoSuchNode(parent.clone()))?;
    let p = &flat[p_idx];
    let nl = newline(src);
    let p_indent = indent_at(src, p.start);
    let kids: Vec<&Found> = flat.iter().filter(|f| f.parent == Some(p_idx)).collect();
    let child_indent = kids
        .iter()
        .find(|k| src[..k.start].trim_end_matches([' ', '\t']).ends_with('\n'))
        .map(|k| indent_at(src, k.start).to_string())
        .unwrap_or_else(|| format!("{p_indent}{}", unit(src)));
    let body = indented(&dedent(text), &child_indent, nl);
    let Some(inside) = p.node.body() else {
        // no block yet: one is opened after the node
        let at = p.end;
        let block = format!(" {{{nl}{body}{nl}{p_indent}}}");
        return Ok(apply(&req.label, splice(&parent.file, src, at, at, &block)));
    };
    let (open, close) = (inside.offset(), inside.offset() + inside.len());
    if let Some(b) = before {
        let k = kids
            .iter()
            .find(|k| k.start == b.start && k.end - k.start == b.len)
            .ok_or_else(|| Refusal::NotASibling(b.clone()))?;
        let line = src[..k.start].rfind('\n').map_or(0, |i| i + 1);
        if src[line..k.start].trim().is_empty() {
            let at = line;
            return Ok(apply(
                &req.label,
                splice(&parent.file, src, at, at, &format!("{body}{nl}")),
            ));
        }
        let at = k.start;
        return Ok(apply(
            &req.label,
            splice(&parent.file, src, at, at, &format!("{}; ", dedent(text))),
        ));
    }
    let inner = &src[open..close];
    if inner.contains('\n') {
        // before the line the closing brace is on
        let line = src[..close].rfind('\n').map_or(close, |i| i + 1);
        if src[line..close].trim().is_empty() {
            return Ok(apply(
                &req.label,
                splice(&parent.file, src, line, line, &format!("{body}{nl}")),
            ));
        }
        return Ok(apply(
            &req.label,
            splice(
                &parent.file,
                src,
                close,
                close,
                &format!("{nl}{body}{nl}{p_indent}"),
            ),
        ));
    }
    // a block on one line, `{ … }` or `{}`, opened up
    let existing = inner.trim();
    let mut lines = Vec::new();
    if !existing.is_empty() {
        lines.push(indented(existing, &child_indent, nl));
    }
    lines.push(body);
    let text = format!("{nl}{}{nl}{p_indent}", lines.join(nl));
    Ok(apply(
        &req.label,
        splice(&parent.file, src, open, close, &text),
    ))
}

/// The bytes a node takes: its whole lines when nothing else is on them
/// (a comment after it on its line goes with it), else the node and the
/// `;` and spaces after it.
fn whole_lines(src: &str, start: usize, end: usize) -> (usize, usize) {
    lines_of(src, start, end).unwrap_or_else(|| within_line(src, start, end))
}

/// The node's whole lines, when nothing else is on them.
fn lines_of(src: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let line = src[..start].rfind('\n').map_or(0, |i| i + 1);
    let eol = src[end..].find('\n').map_or(src.len(), |i| end + i + 1);
    let after = src[end..eol].trim();
    let after = after.trim_start_matches(';').trim();
    (src[line..start].trim().is_empty() && (after.is_empty() || after.starts_with("//")))
        .then_some((line, eol))
}

/// The node and the `;` and spaces after it, within its line.
fn within_line(src: &str, start: usize, end: usize) -> (usize, usize) {
    let mut to = end;
    let rest = &src[end..];
    let skip = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    if rest[skip..].starts_with(';') {
        to = end + skip + 1;
        let r = &src[to..];
        to += r.len() - r.trim_start_matches([' ', '\t']).len();
    }
    (start, to)
}

fn move_before(
    lib: &Library,
    req: &Request,
    node: &NodeRef,
    before: Option<&NodeRef>,
) -> Result<Plan, Refusal> {
    let (src, doc) = parse(lib, &node.file)?;
    let flat = flatten(&doc);
    let idx = flat
        .iter()
        .position(|f| f.start == node.start && f.end - f.start == node.len)
        .ok_or_else(|| Refusal::NoSuchNode(node.clone()))?;
    let me = &flat[idx];
    // a node sharing its line with others is not moved: it would take
    // their formatting apart
    let (from, to) =
        lines_of(src, me.start, me.end).ok_or_else(|| Refusal::SharesItsLine(node.clone()))?;
    let moved = &src[from..to];
    let at = match before {
        Some(b) => {
            let k = flat
                .iter()
                .find(|f| f.start == b.start && f.end - f.start == b.len && f.parent == me.parent)
                .ok_or_else(|| Refusal::NotASibling(b.clone()))?;
            src[..k.start].rfind('\n').map_or(0, |i| i + 1)
        }
        None => {
            let parent = me.parent.map(|p| &flat[p]);
            match parent.and_then(|p| p.node.body()) {
                Some(inside) => {
                    let close = inside.offset() + inside.len();
                    src[..close].rfind('\n').map_or(close, |i| i + 1)
                }
                None => src.len(),
            }
        }
    };
    if at >= from && at <= to {
        return Ok(apply(&req.label, Vec::new()));
    }
    // the file as it will be, then one splice over what differs
    let mut text = src.to_string();
    let mut moved_text = moved.to_string();
    if !moved_text.ends_with('\n') {
        moved_text.push_str(newline(src));
    }
    if at > to {
        text.insert_str(at, &moved_text);
        text.replace_range(from..to, "");
    } else {
        text.replace_range(from..to, "");
        text.insert_str(at, &moved_text);
    }
    Ok(apply(&req.label, minimal_splice(&node.file, src, &text)))
}

/// The one splice that turns `old` into `new`: what lies between their
/// common start and common end.
fn minimal_splice(file: &str, old: &str, new: &str) -> Vec<Patch> {
    let mut pre = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(pre) || !new.is_char_boundary(pre) {
        pre -= 1;
    }
    let max_suf = old.len().min(new.len()) - pre;
    let mut suf = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_suf)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - suf) || !new.is_char_boundary(new.len() - suf) {
        suf -= 1;
    }
    splice(file, old, pre, old.len() - suf, &new[pre..new.len() - suf])
}
