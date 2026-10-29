//! The lexicon's settings and palette as Lexicon Properties lists them
//! (#72): where the `lexicon` and `palette` blocks are written, and each of
//! their nodes with its values as written, so each is set where it is
//! (and a block not yet written is added where `file` says).
//!
//! Read from the source as written, so it is there while the library does
//! not compile, as far as each file parses.

use std::collections::BTreeMap;

use isolex::kdl::{Document, NodeRef};
use serde::Serialize;

use crate::compile::Place;

/// The two blocks, each where it is written, if it is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LexiconRows {
    pub lexicon: Option<Block>,
    pub palette: Option<Block>,
    /// The file a block not yet written goes in: the one the other block
    /// is in, else `lexicon.ilx` if the library has one, else its first.
    pub file: String,
}

/// A block and its nodes, in the order written.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block {
    pub node: Place,
    pub rows: Vec<Setting>,
}

/// One node of a block: `tokens "ink" "outline"`, `shape-limit 400`, or a
/// palette's `ink light="#1f2933" dark="#e4e7eb"`. Values without their
/// quotes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Setting {
    pub name: String,
    pub args: Vec<String>,
    pub props: BTreeMap<String, String>,
    pub node: Place,
}

/// The library's `lexicon` and `palette` blocks.
pub fn lexicon(files: &[(String, String)]) -> LexiconRows {
    let mut lexicon = None;
    let mut palette = None;
    for (file, src) in files {
        let Ok(doc) = Document::parse(src) else {
            continue;
        };
        for node in doc.nodes() {
            let slot = match node.name().value() {
                "lexicon" => &mut lexicon,
                "palette" => &mut palette,
                _ => continue,
            };
            if slot.is_none() {
                *slot = Some(block(file, src, &node));
            }
        }
    }
    let file = [&lexicon, &palette]
        .into_iter()
        .flatten()
        .map(|b: &Block| b.node.file.clone())
        .next()
        .or_else(|| {
            files
                .iter()
                .find(|(f, _)| f == "lexicon.ilx")
                .map(|(f, _)| f.clone())
        })
        .or_else(|| files.first().map(|(f, _)| f.clone()))
        .unwrap_or_else(|| "lexicon.ilx".into());
    LexiconRows {
        lexicon,
        palette,
        file,
    }
}

fn place(file: &str, node: &NodeRef) -> Place {
    Place {
        file: file.to_string(),
        start: node.span().offset(),
        len: node.span().len(),
    }
}

fn block(file: &str, src: &str, node: &NodeRef) -> Block {
    let rows = node
        .children()
        .map(|c| {
            c.nodes()
                .map(|n| {
                    let mut args = Vec::new();
                    let mut props = BTreeMap::new();
                    for e in n.entries() {
                        let raw = e.raw(src);
                        let value = raw
                            .strip_prefix('"')
                            .and_then(|r| r.strip_suffix('"'))
                            .unwrap_or(raw)
                            .to_string();
                        match e.name() {
                            Some(k) => {
                                props.insert(k.value().to_string(), value);
                            }
                            None => args.push(value),
                        }
                    }
                    Setting {
                        name: n.name().value().to_string(),
                        args,
                        props,
                        node: place(file, &n),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Block {
        node: place(file, node),
        rows,
    }
}
