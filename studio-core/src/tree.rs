//! The library as the Tree tab lists it (#71, docs/studio-interface.md,
//! Tree): bases and icons nested by `extends`, each icon's axis variants
//! under it, and fonts in a section of their own. Read from the source as
//! written, through `isolex::graph`, so it is there while the library
//! does not compile, as far as the parser got.

use std::collections::BTreeSet;

use isolex::ast::{DefKind, Definition, Lexicon};
use serde::Serialize;

/// The rows of the Tree tab.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tree {
    /// Bases and icons that extend nothing (or what is not there), each
    /// with what extends it under it, in the order the library is read;
    /// then those of a cycle, which the resolver reports, from the first.
    pub icons: Vec<TreeRow>,
    pub fonts: Vec<TreeRow>,
}

/// One row: a definition, or a variant one emits.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TreeRow {
    /// "base", "icon", "variant" or "font".
    pub kind: String,
    /// What it is called: the definition's name, or the icon a variant
    /// emits ("server-down").
    pub name: String,
    /// The definition a click opens for editing: its own, or a variant's
    /// axis.
    pub definition: String,
    /// The icon drawn for it, as the IR names it: its thumbnail, and what
    /// a click draws. None for a base, a font, or an icon whose axis
    /// does not emit it bare.
    pub icon: Option<String>,
    /// For a variant: the axis and the variant's name.
    pub axis: Option<String>,
    pub variant: Option<String>,
    /// Its variants first, then what extends it.
    pub children: Vec<TreeRow>,
}

/// The tree of `files`.
pub fn tree(files: &[(String, String)]) -> Tree {
    let lexicon = isolex::parser::parse_lexicon(files).lexicon;
    let graph = isolex::graph(files);
    let emits = |name: &str| -> Vec<String> {
        graph
            .definitions
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.emits.clone())
            .unwrap_or_default()
    };
    let drawn = |d: &Definition| matches!(d.kind, DefKind::Base | DefKind::Icon);
    let known: BTreeSet<&str> = lexicon
        .defs
        .iter()
        .filter(|d| drawn(d))
        .map(|d| d.name.as_str())
        .collect();
    let mut placed = BTreeSet::new();
    let mut icons: Vec<TreeRow> = lexicon
        .defs
        .iter()
        .filter(|d| drawn(d) && !d.extends.as_deref().is_some_and(|p| known.contains(p)))
        .map(|d| row(&lexicon, d, &emits, &mut placed))
        .collect();
    // those in a cycle, which no root reaches, from the first of each
    for d in &lexicon.defs {
        if drawn(d) && !placed.contains(&d.name) {
            icons.push(row(&lexicon, d, &emits, &mut placed));
        }
    }
    let fonts = lexicon
        .defs
        .iter()
        .filter(|d| d.kind == DefKind::Font)
        .map(|d| TreeRow {
            kind: "font".into(),
            name: d.name.clone(),
            definition: d.name.clone(),
            icon: None,
            axis: None,
            variant: None,
            children: Vec::new(),
        })
        .collect();
    Tree { icons, fonts }
}

/// A base's or an icon's row, with its variants and what extends it;
/// `placed` keeps a cycle, which the resolver reports, from going round.
fn row(
    lexicon: &Lexicon,
    def: &Definition,
    emits: &dyn Fn(&str) -> Vec<String>,
    placed: &mut BTreeSet<String>,
) -> TreeRow {
    placed.insert(def.name.clone());
    let emitted = emits(&def.name);
    let mut children: Vec<TreeRow> = Vec::new();
    // each emitted name past its own is a variant of one of its axes
    let axes = axes_of(lexicon, def);
    for name in emitted.iter().filter(|n| **n != def.name) {
        let found = axes.iter().find_map(|a| {
            let axis = lexicon
                .defs
                .iter()
                .find(|d| d.kind == DefKind::Axis && d.name == *a)?;
            let v = axis
                .variants
                .iter()
                .find(|v| *name == format!("{}-{}", def.name, v.name))?;
            Some((axis.name.clone(), v.name.clone()))
        });
        let (axis, variant) = found.unzip();
        children.push(TreeRow {
            kind: "variant".into(),
            name: name.clone(),
            definition: axis.clone().unwrap_or_else(|| def.name.clone()),
            icon: Some(name.clone()),
            axis,
            variant,
            children: Vec::new(),
        });
    }
    let extending: Vec<&Definition> = lexicon
        .defs
        .iter()
        .filter(|d| {
            matches!(d.kind, DefKind::Base | DefKind::Icon)
                && d.extends.as_deref() == Some(def.name.as_str())
        })
        .collect();
    for child in extending {
        if !placed.contains(&child.name) {
            children.push(row(lexicon, child, emits, placed));
        }
    }
    TreeRow {
        kind: if def.kind == DefKind::Base {
            "base"
        } else {
            "icon"
        }
        .into(),
        name: def.name.clone(),
        definition: def.name.clone(),
        icon: emitted.contains(&def.name).then(|| def.name.clone()),
        axis: None,
        variant: None,
        children,
    }
}

/// The axes a definition opts into, its own and those of what it
/// extends, as `axes` unions down the chain.
fn axes_of(lexicon: &Lexicon, def: &Definition) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut at = Some(def);
    while let Some(d) = at {
        if seen.contains(&d.name.as_str()) {
            break;
        }
        seen.push(&d.name);
        for a in &d.body.axes {
            if !out.contains(a) {
                out.push(a.clone());
            }
        }
        at = d
            .extends
            .as_deref()
            .and_then(|p| lexicon.defs.iter().find(|x| x.name == p));
    }
    out
}
