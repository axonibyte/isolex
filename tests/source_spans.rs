//! Where each piece of a node is written (#53): the parser's source map
//! gives every node's arguments, properties, block and children, so the
//! studio can rewrite one value or insert a child by its bytes. The
//! goldens are untouched (sample_lexicon_matches_golden still compares
//! them byte for byte); these hold the map to the source.

use isolex::ast::{DefBody, Definition, Lexicon, Node, Num, Param, Span};
use isolex::kdl::{Document, Value};
use isolex::parser::parse_lexicon;

const FIXTURES: &[(&str, &str)] = &[
    ("sample.ilx", include_str!("fixtures/sample.ilx")),
    ("transforms.ilx", include_str!("fixtures/transforms.ilx")),
    ("text.ilx", include_str!("fixtures/text.ilx")),
];

fn slice(src: &str, span: Span) -> &str {
    &src[span.start..span.start + span.len]
}

/// The value a literal reads as on its own.
fn read(literal: &str) -> Value {
    let doc = Document::parse(&format!("n {literal}"))
        .unwrap_or_else(|e| panic!("{literal:?} is not a value: {}", e.message));
    let node = doc.nodes().next().unwrap();
    let [entry] = node.entries() else {
        panic!("{literal:?} is more than one value");
    };
    assert!(
        entry.name().is_none() && entry.ty().is_none(),
        "{literal:?} is more than a value"
    );
    entry.value().clone()
}

/// Checks and counts what was held to its source.
struct Check<'a> {
    src: &'a str,
    lexicon: &'a Lexicon,
    literals: usize,
    exprs: usize,
    strings: usize,
}

impl Check<'_> {
    fn num(&mut self, at: Span, num: &Num, what: &str) {
        let text = slice(self.src, at);
        match (num, read(text)) {
            (Num::Lit(f), Value::Float(g)) if *f == g => self.literals += 1,
            (Num::Lit(f), Value::Integer(n)) if *f == n as f64 => self.literals += 1,
            (Num::Expr { src, .. }, Value::String(s)) if *src == s => self.exprs += 1,
            (num, v) => panic!("{what}: {text:?} reads as {v:?}, but the AST has {num:?}"),
        }
    }

    fn string(&mut self, at: Span, want: &str, what: &str) {
        let text = slice(self.src, at);
        assert_eq!(
            read(text),
            Value::String(want.to_string()),
            "{what}: {text:?}"
        );
        self.strings += 1;
    }

    fn source(&self, span: Span, what: &str) -> &isolex::ast::NodeSource {
        let s = self
            .lexicon
            .sources
            .get(span)
            .unwrap_or_else(|| panic!("{what} at {span:?} is not in the source map"));
        assert_eq!(s.span, span);
        s
    }

    fn block(&self, span: Span, what: &str) {
        let body = self
            .source(span, what)
            .body
            .unwrap_or_else(|| panic!("{what} has no block"));
        assert_eq!(
            &self.src[body.start - 1..body.start],
            "{",
            "{what}'s block opens"
        );
        assert_eq!(
            &self.src[body.start + body.len..][..1],
            "}",
            "{what}'s block closes"
        );
    }

    fn prop(&mut self, span: Span, key: &str, num: Option<&Num>, what: &str) {
        let Some(num) = num else { return };
        let props = &self.source(span, what).props;
        let at = props
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("{what} has no {key} in the map"))
            .1;
        self.num(at, num, &format!("{what} {key}"));
    }

    fn node(&mut self, node: &Node) {
        let (span, offset, nums, when): (Span, usize, Vec<&Num>, &Option<Num>) = match node {
            Node::Box3 {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Tile {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Cylinder {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::FaceRect {
                args, when, span, ..
            } => (*span, 1, args.iter().collect(), when),
            Node::Poly3d { when, span, .. } => (*span, 0, vec![], when),
            Node::Shadow { args, when, span } => (*span, 0, args.iter().collect(), when),
            Node::Seg {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::At {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Line {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Circle {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Ellipse {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Rect2d {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Arc {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
            Node::Path { when, span, .. } => (*span, 0, vec![], when),
            Node::Poly2d { when, span, .. } => (*span, 0, vec![], when),
            Node::Pt3 { args, span } => (*span, 0, args.iter().collect(), &None),
            Node::Pt2 { args, span } => (*span, 0, args.iter().collect(), &None),
            Node::Repeat {
                count, when, span, ..
            } => (*span, 0, vec![count], when),
            Node::Insert { when, span, .. } => (*span, 0, vec![], when),
            Node::Badge { when, span, .. } => (*span, 0, vec![], when),
            Node::Transform { when, span, .. } => (*span, 0, vec![], when),
            Node::Turn { by, when, span, .. } => match by {
                isolex::ast::TurnBy::Angle(a) => (*span, 0, vec![a], when),
                _ => (*span, 0, vec![], when),
            },
            Node::Text {
                args, when, span, ..
            } => (*span, 0, args.iter().collect(), when),
        };
        let what = format!("{} at {}", self.source(span, "a node").name, span.start);
        let args = self.source(span, &what).args.clone();
        for (i, num) in nums.into_iter().enumerate() {
            self.num(
                args[offset + i],
                num,
                &format!("{what} argument {}", offset + i),
            );
        }
        self.prop(span, "when", when.as_ref(), &what);
        match node {
            Node::Insert { fragment, .. } => self.string(args[0], fragment, &what),
            Node::Badge { icon, scale, .. } => {
                self.string(args[0], icon, &what);
                self.prop(span, "scale", scale.as_ref(), &what);
            }
            Node::Repeat { var, .. } => self.string(args[1], var, &what),
            Node::Rect2d { rx, .. } => self.prop(span, "rx", rx.as_ref(), &what),
            Node::Box3 { opacity, .. }
            | Node::Cylinder { opacity, .. }
            | Node::FaceRect { opacity, .. } => self.prop(span, "opacity", opacity.as_ref(), &what),
            _ => {}
        }
        if !node.children().is_empty() {
            self.block(span, &what);
            let listed = &self.source(span, &what).children;
            for child in node.children() {
                let at = match child {
                    Node::Pt3 { span, .. } | Node::Pt2 { span, .. } => *span,
                    _ => continue,
                };
                assert!(
                    listed.contains(&at),
                    "{what} lists its point at {}",
                    at.start
                );
            }
        }
        for child in node.children() {
            self.node(child);
        }
    }

    fn body(&mut self, body: &DefBody, what: &str) {
        for (name, param, at) in &body.params {
            let s = self.source(*at, what).clone();
            self.string(s.args[0], name, &format!("{what} param"));
            if let Param::Num(num) = param {
                self.num(s.args[1], num, &format!("{what} param {name}"));
            }
        }
        for part in &body.parts {
            let s = self.source(part.span, what).clone();
            self.string(s.args[0], &part.name, &format!("{what} part"));
            if !part.nodes.is_empty() {
                self.block(part.span, &format!("{what} part {}", part.name));
            }
            for node in &part.nodes {
                self.node(node);
            }
        }
    }

    fn definition(&mut self, def: &Definition) {
        let s = self.source(def.span, &def.name).clone();
        self.string(s.args[0], &def.name, "a definition's name");
        if let Some(base) = &def.extends {
            // a property, or a node in its block
            let at = match s.props.iter().find(|(k, _)| k == "extends") {
                Some((_, at)) => *at,
                None => {
                    let node = s
                        .children
                        .iter()
                        .map(|c| self.source(*c, &def.name))
                        .find(|c| c.name == "extends")
                        .expect("an extends node");
                    node.args[0]
                }
            };
            self.string(at, base, &format!("{} extends", def.name));
        }
        self.block(def.span, &def.name);
        self.body(&def.body, &def.name);
        for variant in &def.variants {
            let v = self.source(variant.span, &def.name).clone();
            self.string(v.args[0], &variant.name, "a variant's name");
            self.body(
                &variant.body,
                &format!("{} variant {}", def.name, variant.name),
            );
        }
        for glyph in &def.body.font.glyphs {
            for node in &glyph.nodes {
                self.node(node);
            }
        }
    }
}

#[test]
fn every_value_in_the_fixtures_is_where_the_map_says() {
    let (mut literals, mut exprs, mut strings) = (0, 0, 0);
    for (name, src) in FIXTURES {
        let outcome = parse_lexicon(&[(name.to_string(), src.to_string())]);
        assert_eq!(outcome.diags, vec![], "{name} parses clean");
        let mut check = Check {
            src,
            lexicon: &outcome.lexicon,
            literals: 0,
            exprs: 0,
            strings: 0,
        };
        for def in &outcome.lexicon.defs {
            check.definition(def);
        }
        literals += check.literals;
        exprs += check.exprs;
        strings += check.strings;
        // every node of the file, not only those the AST keeps
        let doc = Document::parse(src).unwrap();
        let mut todo: Vec<_> = doc.nodes().collect();
        let mut count = 0;
        while let Some(n) = todo.pop() {
            count += 1;
            todo.extend(n.children().into_iter().flat_map(|c| c.nodes()));
        }
        assert_eq!(
            outcome.lexicon.sources.len(),
            count,
            "{name}: one entry a node"
        );
    }
    assert!(
        literals > 400 && exprs > 15 && strings > 100,
        "{literals} {exprs} {strings}"
    );
}

#[test]
fn the_map_has_every_node_and_each_block_inside_its_braces() {
    let src = "icon \"a\" extends=\"b\" {\n  describe \"x\"\n  part \"p\" { box 0 0 0 1 1 \"h\" fill=\"ink\" }\n}\n";
    let outcome = parse_lexicon(&[("a.ilx".to_string(), src.to_string())]);
    let sources = &outcome.lexicon.sources;
    assert_eq!(sources.len(), 4, "icon, describe, part and box");
    let def = &outcome.lexicon.defs[0];
    let icon = sources.get(def.span).unwrap();
    assert_eq!(slice(src, icon.name_span), "icon");
    assert_eq!(slice(src, icon.args[0]), "\"a\"");
    assert_eq!(icon.props.len(), 1);
    assert_eq!(slice(src, icon.props[0].1), "\"b\"");
    assert_eq!(icon.children.len(), 2);
    let part = sources.get(def.body.parts[0].span).unwrap();
    assert_eq!(
        slice(src, part.body.unwrap()),
        " box 0 0 0 1 1 \"h\" fill=\"ink\" "
    );
    let boxed = sources.get(part.children[0]).unwrap();
    let args: Vec<&str> = boxed.args.iter().map(|a| slice(src, *a)).collect();
    assert_eq!(args, ["0", "0", "0", "1", "1", "\"h\""]);
    assert_eq!(slice(src, boxed.props[0].1), "\"ink\"");
    assert!(boxed.body.is_none());
    // a span that is not a node's is not found
    let wrong = Span {
        len: part.span.len + 1,
        ..part.span
    };
    assert!(sources.get(wrong).is_none());
}

#[test]
fn a_slashdashed_block_is_not_a_block() {
    let src = "icon \"a\" /-{ part \"p\" }\n";
    let outcome = parse_lexicon(&[("a.ilx".to_string(), src.to_string())]);
    let icon = outcome
        .lexicon
        .sources
        .get(outcome.lexicon.defs[0].span)
        .unwrap();
    assert_eq!(icon.body, None);
    assert!(icon.children.is_empty());
}
