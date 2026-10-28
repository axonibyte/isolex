//! A library as a graph (#57): its definitions, and every reference from
//! one to another, with where each is written. The studio draws its tree
//! from this, and a rename rewrites each reference at its span.

use serde::Serialize;

use crate::ast::{DefBody, DefKind, Lexicon, Node, SourceMap, Span};

/// Every definition of a library and every reference between them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Graph {
    /// In the order the library is read.
    pub definitions: Vec<GraphDef>,
    /// In the order they are written, definition by definition.
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GraphDef {
    pub name: String,
    pub kind: DefKind,
    pub file: String,
    pub span: Span,
    /// The icons it emits, as the IR names them: an icon's own name and
    /// its variants'. None for any other kind.
    pub emits: Vec<String>,
}

/// One definition naming another.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Edge {
    pub kind: EdgeKind,
    pub from: String,
    pub to: String,
    /// The name as written where it is referred to: the bytes a rename
    /// replaces.
    pub at: Span,
    /// The variant of `from` (an axis) that refers, when one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// `extends`, from the child to its parent.
    Extends,
    /// `insert`, to the fragment.
    Insert,
    /// `badge`, to the icon drawn as the badge.
    Badge,
    /// `axes`, from an icon, base or font to an axis it opts into.
    Axis,
    /// `text … font=`, to the font it is set in.
    Font,
}

/// The graph of a library given as (file name, source text) pairs, in the
/// order given, whether or not it compiles: a reference to a definition
/// that is not there is still an edge, to the name written.
pub fn graph(files: &[(String, String)]) -> Graph {
    let outcome = crate::parser::parse_lexicon(files);
    let resolved = crate::resolve::resolve(&outcome.lexicon);
    let emitted = crate::expand::emitted(&resolved);
    of_lexicon(&outcome.lexicon, |icon| {
        emitted
            .iter()
            .filter(|(from, _)| from == icon)
            .map(|(_, name)| name.clone())
            .collect()
    })
}

fn of_lexicon(lexicon: &Lexicon, emits: impl Fn(&str) -> Vec<String>) -> Graph {
    let sources = &lexicon.sources;
    let mut edges = Vec::new();
    let definitions = lexicon
        .defs
        .iter()
        .map(|def| {
            let mut edge = |kind, to: &str, at: Option<Span>, variant: Option<&str>| {
                if let Some(at) = at {
                    edges.push(Edge {
                        kind,
                        from: def.name.clone(),
                        to: to.to_string(),
                        at,
                        variant: variant.map(str::to_string),
                    });
                }
            };
            if let Some(parent) = &def.extends {
                edge(
                    EdgeKind::Extends,
                    parent,
                    extends_at(sources, def.span),
                    None,
                );
            }
            body_edges(sources, &def.body, def.span, None, &mut edge);
            for variant in &def.variants {
                body_edges(
                    sources,
                    &variant.body,
                    variant.span,
                    Some(&variant.name),
                    &mut edge,
                );
            }
            for glyph in &def.body.font.glyphs {
                node_edges(sources, &glyph.nodes, None, &mut edge);
            }
            GraphDef {
                name: def.name.clone(),
                kind: def.kind,
                file: lexicon
                    .files
                    .get(def.span.file)
                    .cloned()
                    .unwrap_or_default(),
                span: def.span,
                emits: if def.kind == DefKind::Icon {
                    emits(&def.name)
                } else {
                    Vec::new()
                },
            }
        })
        .collect();
    Graph { definitions, edges }
}

/// The parent's name as written: a property, or an `extends` node.
fn extends_at(sources: &SourceMap, def: Span) -> Option<Span> {
    let node = sources.get(def)?;
    if let Some((_, at)) = node.props.iter().find(|(k, _)| k == "extends") {
        return Some(*at);
    }
    node.children
        .iter()
        .filter_map(|c| sources.get(*c))
        .find(|c| c.name == "extends")
        .and_then(|c| c.args.first().copied())
}

type EdgeFn<'e> = dyn FnMut(EdgeKind, &str, Option<Span>, Option<&str>) + 'e;

/// The edges of a definition's body, or a variant's: its `axes`, and
/// what its parts' nodes refer to.
fn body_edges(
    sources: &SourceMap,
    body: &DefBody,
    at: Span,
    variant: Option<&str>,
    edge: &mut EdgeFn,
) {
    let axes = sources
        .get(at)
        .into_iter()
        .flat_map(|n| n.children.iter())
        .filter_map(|c| sources.get(*c))
        .filter(|c| c.name == "axes")
        .flat_map(|c| c.args.iter().copied());
    for (axis, at) in body.axes.iter().zip(axes) {
        edge(EdgeKind::Axis, axis, Some(at), variant);
    }
    for part in &body.parts {
        node_edges(sources, &part.nodes, variant, edge);
    }
}

/// What the nodes refer to, without recursion (#28).
fn node_edges(sources: &SourceMap, nodes: &[Node], variant: Option<&str>, edge: &mut EdgeFn) {
    let mut todo: Vec<&Node> = nodes.iter().rev().collect();
    while let Some(node) = todo.pop() {
        let arg = |span: Span| sources.get(span).and_then(|n| n.args.first().copied());
        match node {
            Node::Insert { fragment, span, .. } => {
                edge(EdgeKind::Insert, fragment, arg(*span), variant)
            }
            Node::Badge { icon, span, .. } => edge(EdgeKind::Badge, icon, arg(*span), variant),
            Node::Text { spec, span, .. } => {
                let at = sources
                    .get(*span)
                    .and_then(|n| n.props.iter().find(|(k, _)| k == "font").map(|(_, at)| *at));
                edge(EdgeKind::Font, &spec.font, at, variant);
            }
            _ => {}
        }
        todo.extend(node.children().iter().rev());
    }
}
