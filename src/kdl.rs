//! A KDL 2.0 reader with no recursion (#28): Isolex's own, so that how
//! deeply a lexicon nests its blocks costs memory, never the call stack.
//! It reads the whole of KDL 2.0 (kdl.dev, draft-marchan-kdl2), not a
//! subset, into a flat document: every node in one array, each block a
//! list of indices into it. One loop with an explicit stack of open
//! blocks does the parsing.
//!
//! What it keeps is what Isolex reads: node names, entries (arguments and
//! properties, in order), children, type annotations, and the span of
//! each. A span starts where its node or entry starts (a type annotation
//! included) and ends with its last token, never with whitespace after
//! it. Slashdashed nodes, entries and blocks are parsed and checked like
//! any other, then left out.

use std::fmt;

/// A byte range of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    offset: usize,
    len: usize,
}

impl Span {
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

fn span(start: usize, end: usize) -> Span {
    Span {
        offset: start,
        len: end - start,
    }
}

/// A string as written (a node name, a property key, a type) and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    value: String,
    span: Span,
}

impl Ident {
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    /// A number written without a fraction or exponent, in any radix.
    Integer(i128),
    Float(f64),
    Bool(bool),
    Null,
}

impl Value {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

/// A value written as KDL 2.0 that reads back as the same value: a float
/// in the shortest decimal that does (always with a fraction or an
/// exponent, so it stays a float), a string as a bare identifier where one
/// is legal, as a raw string when it holds quotes or backslashes, and
/// quoted with escapes otherwise.
pub fn write_value(value: &Value) -> String {
    match value {
        Value::String(s) => write_string(s),
        Value::Integer(n) => n.to_string(),
        Value::Float(f) if f.is_nan() => "#nan".to_string(),
        Value::Float(f) if *f == f64::INFINITY => "#inf".to_string(),
        Value::Float(f) if *f == f64::NEG_INFINITY => "#-inf".to_string(),
        Value::Float(f) => write_float(*f),
        Value::Bool(true) => "#true".to_string(),
        Value::Bool(false) => "#false".to_string(),
        Value::Null => "#null".to_string(),
    }
}

/// Rust prints the shortest digits that read back as the same `f64`, in
/// plain or exponent form; the shorter of the two is kept.
fn write_float(f: f64) -> String {
    let plain = format!("{f}");
    let plain = if plain.contains(['.', 'e']) {
        plain
    } else {
        format!("{plain}.0")
    };
    let exp = format!("{f:e}");
    if exp.len() < plain.len() {
        exp
    } else {
        plain
    }
}

fn write_string(s: &str) -> String {
    if is_bare_identifier(s) {
        return s.to_string();
    }
    let literal = |c: char| !is_newline(c) && !is_disallowed(c);
    // one that starts with a quote is quoted instead, since #"" followed
    // by a quote opens a multi-line raw string
    if s.contains(['"', '\\']) && !s.starts_with('"') && s.chars().all(literal) {
        // enough hashes that no `"` in the string closes it
        let mut hashes = 1;
        while s.contains(&format!("\"{}", "#".repeat(hashes))) {
            hashes += 1;
        }
        let h = "#".repeat(hashes);
        return format!("{h}\"{s}\"{h}");
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{C}' => out.push_str("\\f"),
            c if is_newline(c) || is_disallowed(c) => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Whether `s` can be written bare: read as [`Parser::identifier`] reads
/// one, and not where an entry would read a number or a keyword instead.
fn is_bare_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let (first, second, third) = (chars.next(), chars.next(), chars.next());
    let Some(first) = first else {
        return false;
    };
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    let looks_like_number = first.is_ascii_digit()
        || (matches!(first, '+' | '-') && (digit(second) || (second == Some('.') && digit(third))))
        || (first == '.' && digit(second));
    s.chars().all(is_identifier_char)
        && !looks_like_number
        && !matches!(s, "true" | "false" | "null" | "inf" | "-inf" | "nan")
}

/// An argument (no name) or a property.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    name: Option<Ident>,
    ty: Option<Ident>,
    value: Value,
    span: Span,
    value_span: Span,
}

impl Entry {
    pub fn name(&self) -> Option<&Ident> {
        self.name.as_ref()
    }
    pub fn ty(&self) -> Option<&Ident> {
        self.ty.as_ref()
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn span(&self) -> Span {
        self.span
    }
    /// The value's literal alone, without a key or type annotation: the
    /// bytes to replace to change the value and nothing else.
    pub fn value_span(&self) -> Span {
        self.value_span
    }
    /// The value's literal as written in `src`, the source this entry was
    /// parsed from.
    pub fn raw<'s>(&self, src: &'s str) -> &'s str {
        &src[self.value_span.offset..self.value_span.offset + self.value_span.len]
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Node {
    name: Ident,
    ty: Option<Ident>,
    entries: Vec<Entry>,
    children: Option<Vec<usize>>,
    span: Span,
    /// Between the braces of its children block, which is where children
    /// go: from just after `{` to `}`.
    body: Option<Span>,
}

/// A parsed document: every node, and which are at the top level.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    nodes: Vec<Node>,
    roots: Vec<usize>,
}

/// A node of a [`Document`], borrowed from it.
#[derive(Debug, Clone, Copy)]
pub struct NodeRef<'a> {
    doc: &'a Document,
    idx: usize,
}

/// A node's children block, borrowed from its document.
#[derive(Debug, Clone, Copy)]
pub struct Children<'a> {
    doc: &'a Document,
    nodes: &'a [usize],
}

impl Document {
    pub fn parse(src: &str) -> Result<Document, Error> {
        Parser::new(src).document()
    }

    pub fn nodes(&self) -> impl Iterator<Item = NodeRef<'_>> {
        self.roots
            .iter()
            .map(move |&idx| NodeRef { doc: self, idx })
    }
}

impl<'a> Children<'a> {
    pub fn nodes(&self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let doc = self.doc;
        self.nodes.iter().map(move |&idx| NodeRef { doc, idx })
    }
}

impl<'a> NodeRef<'a> {
    fn node(&self) -> &'a Node {
        &self.doc.nodes[self.idx]
    }
    pub fn name(&self) -> &'a Ident {
        &self.node().name
    }
    pub fn ty(&self) -> Option<&'a Ident> {
        self.node().ty.as_ref()
    }
    pub fn entries(&self) -> &'a [Entry] {
        &self.node().entries
    }
    pub fn children(&self) -> Option<Children<'a>> {
        self.node().children.as_ref().map(|nodes| Children {
            doc: self.doc,
            nodes,
        })
    }
    pub fn span(&self) -> Span {
        self.node().span
    }
    /// The inside of its children block, between the braces, when it has
    /// one (a slashdashed block is not one).
    pub fn body(&self) -> Option<Span> {
        self.node().body
    }
}

/// The first syntax error, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

fn error<T>(offset: usize, message: impl Into<String>) -> Result<T, Error> {
    Err(Error {
        offset,
        message: message.into(),
    })
}

// --- characters ----------------------------------------------------------

/// Whitespace that is not a newline (the spec's table).
fn is_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// Newline characters other than the CR LF pair, which [`Parser::newline`]
/// reads as one.
fn is_newline(c: char) -> bool {
    matches!(
        c,
        '\r' | '\n' | '\u{85}' | '\u{B}' | '\u{C}' | '\u{2028}' | '\u{2029}'
    )
}

/// Code points that may not appear literally anywhere in a document.
fn is_disallowed(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}'
            | '\u{E}'..='\u{1F}'
            | '\u{7F}'
            | '\u{200E}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

fn is_identifier_char(c: char) -> bool {
    !is_space(c)
        && !is_newline(c)
        && !is_disallowed(c)
        && !matches!(
            c,
            '\\' | '/' | '(' | ')' | '{' | '}' | ';' | '[' | ']' | '"' | '#' | '='
        )
}

// --- the parser ----------------------------------------------------------

/// A block that is open: the document itself, or a node's `{ … }`.
struct Level {
    /// Where its `{` is, for the error when it never closes; `None` for
    /// the document.
    open: Option<usize>,
    /// Its contents are slashdashed away: parsed, and left out.
    discard: bool,
    /// The block itself is slashdashed (`/- { … }`), as opposed to
    /// belonging to a node that is.
    slashdashed: bool,
    /// The node it belongs to, to go back to when it closes.
    owner: Option<Open>,
}

/// A node being read: its entries, then any children blocks.
#[derive(Clone)]
struct Open {
    /// Its index, or `None` when it is slashdashed away or inside
    /// something that is.
    node: Option<usize>,
    /// Everything in it is being left out.
    discard: bool,
    start: usize,
    /// Where its last kept token ends.
    end: usize,
    /// A children block has been read (slashdashed or not): no more
    /// entries may follow.
    block: bool,
    /// Its one real (not slashdashed) children block has been read.
    real_block: bool,
}

struct Parser<'s> {
    src: &'s str,
    i: usize,
    doc: Document,
}

impl<'s> Parser<'s> {
    fn new(src: &'s str) -> Self {
        Parser {
            src,
            i: 0,
            doc: Document::default(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn peek_at(&self, at: usize) -> Option<char> {
        self.src.get(at..).and_then(|s| s.chars().next())
    }

    fn rest(&self) -> &'s str {
        &self.src[self.i..]
    }

    fn eof(&self) -> bool {
        self.i >= self.src.len()
    }

    /// The length of a newline at `at` (CR LF is one), if there is one.
    fn newline_at(&self, at: usize) -> Option<usize> {
        let s = &self.src[at..];
        if s.starts_with("\r\n") {
            return Some(2);
        }
        s.chars()
            .next()
            .filter(|&c| is_newline(c))
            .map(|c| c.len_utf8())
    }

    fn newline(&mut self) -> bool {
        match self.newline_at(self.i) {
            Some(n) => {
                self.i += n;
                true
            }
            None => false,
        }
    }

    // --- whitespace and comments -----------------------------------------

    /// `/* … */`, which nests.
    fn multi_line_comment(&mut self) -> Result<(), Error> {
        let start = self.i;
        self.i += 2;
        let mut depth = 1usize;
        while depth > 0 {
            let rest = self.rest();
            if rest.starts_with("/*") {
                depth += 1;
                self.i += 2;
            } else if rest.starts_with("*/") {
                depth -= 1;
                self.i += 2;
            } else if let Some(c) = rest.chars().next() {
                self.i += c.len_utf8();
            } else {
                return error(start, "a /* comment is never closed with */");
            }
        }
        Ok(())
    }

    /// `//` to the end of the line, the newline included.
    fn single_line_comment(&mut self) {
        while !self.eof() {
            if self.newline() {
                return;
            }
            self.i += self.peek().map_or(1, char::len_utf8);
        }
    }

    /// `ws`: spaces and multi-line comments. True if anything was read.
    fn ws(&mut self) -> Result<bool, Error> {
        let start = self.i;
        loop {
            match self.peek() {
                Some(c) if is_space(c) => self.i += c.len_utf8(),
                Some('/') if self.rest().starts_with("/*") => self.multi_line_comment()?,
                _ => return Ok(self.i > start),
            }
        }
    }

    /// `node-space`: whitespace within a node, newlines only after a `\`
    /// line continuation. True if anything was read.
    fn node_space(&mut self) -> Result<bool, Error> {
        let start = self.i;
        loop {
            self.ws()?;
            if self.peek() != Some('\\') {
                return Ok(self.i > start);
            }
            let at = self.i;
            self.i += 1;
            self.ws()?;
            if self.rest().starts_with("//") {
                self.single_line_comment();
            } else if !self.newline() && !self.eof() {
                return error(
                    at,
                    "a \\ continues a node on the next line, so only whitespace or a comment may follow it on this one",
                );
            }
        }
    }

    /// `line-space`: whitespace, newlines and single-line comments.
    fn line_space(&mut self) -> Result<(), Error> {
        loop {
            let before = self.i;
            self.node_space()?;
            self.newline();
            if self.rest().starts_with("//") {
                self.single_line_comment();
            }
            if self.i == before {
                return Ok(());
            }
        }
    }

    /// After `/-`: whitespace, newlines and comments, but not another
    /// slashdash.
    fn after_slashdash(&mut self) -> Result<(), Error> {
        self.i += 2;
        self.line_space()?;
        if self.rest().starts_with("/-") {
            return error(self.i, "a slashdash cannot comment out another slashdash");
        }
        Ok(())
    }

    // --- strings -----------------------------------------------------------

    /// Any string: identifier, quoted, multi-line or raw.
    fn string(&mut self) -> Result<Ident, Error> {
        let start = self.i;
        let value = match self.peek() {
            Some('"') => self.quoted()?,
            Some('#') if matches!(self.peek_at(self.i + 1), Some('#' | '"')) => self.raw()?,
            Some(_) => self.identifier()?,
            None => return error(start, "expected a string, found the end of the document"),
        };
        Ok(Ident {
            value,
            span: span(start, self.i),
        })
    }

    fn identifier(&mut self) -> Result<String, Error> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if !is_identifier_char(c) {
                break;
            }
            self.i += c.len_utf8();
        }
        let text = &self.src[start..self.i];
        if text.is_empty() {
            return match self.peek() {
                Some(c) => error(start, format!("expected a string, found '{c}'")),
                None => error(start, "expected a string, found the end of the document"),
            };
        }
        let mut chars = text.chars();
        let first = chars.next().expect("not empty");
        let second = chars.next();
        let third = chars.next();
        let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
        let looks_like_number = first.is_ascii_digit()
            || (matches!(first, '+' | '-')
                && (digit(second) || (second == Some('.') && digit(third))))
            || (first == '.' && digit(second));
        if looks_like_number {
            return error(
                start,
                format!(
                    "'{text}' looks like a number but is not one; quote it to make it a string"
                ),
            );
        }
        if matches!(text, "true" | "false" | "null" | "inf" | "-inf" | "nan") {
            return error(
                start,
                format!(
                    "'{text}' is a keyword and is written #{text}; quote it to make it a string"
                ),
            );
        }
        Ok(text.to_string())
    }

    /// `"…"` or `"""…"""`.
    fn quoted(&mut self) -> Result<String, Error> {
        let start = self.i;
        if self.rest().starts_with("\"\"\"") {
            self.i += 3;
            return self.multi_line(start, 0);
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return error(start, "a string is never closed with \""),
                Some('"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some('\\') => self.escape(&mut out)?,
                Some(c) if is_newline(c) => {
                    return error(
                        self.i,
                        "a quoted string cannot hold a newline; write \\n, or use a \"\"\" multi-line string",
                    )
                }
                Some(c) => {
                    out.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }

    /// One escape at `\`, pushed onto `out` (whitespace escapes push
    /// nothing).
    fn escape(&mut self, out: &mut String) -> Result<(), Error> {
        let at = self.i;
        self.i += 1;
        let Some(c) = self.peek() else {
            return error(at, "a string is never closed");
        };
        if is_space(c) || is_newline(c) {
            // a whitespace escape: it and all the whitespace after go
            while let Some(c) = self.peek() {
                if is_space(c) {
                    self.i += c.len_utf8();
                } else if !self.newline() {
                    break;
                }
            }
            return Ok(());
        }
        self.i += c.len_utf8();
        out.push(match c {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '\\' => '\\',
            '"' => '"',
            'b' => '\u{8}',
            'f' => '\u{C}',
            's' => ' ',
            'u' => {
                if self.peek() != Some('{') {
                    return error(at, "a \\u escape is written \\u{…} with 1 to 6 hex digits");
                }
                self.i += 1;
                let hex_start = self.i;
                while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                    self.i += 1;
                }
                let hex = &self.src[hex_start..self.i];
                if self.peek() != Some('}') || hex.is_empty() || hex.len() > 6 {
                    return error(at, "a \\u escape is written \\u{…} with 1 to 6 hex digits");
                }
                self.i += 1;
                match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                    Some(c) => c,
                    None => {
                        return error(at, format!("\\u{{{hex}}} is not a Unicode scalar value"))
                    }
                }
            }
            other => return error(at, format!("\\{other} is not an escape")),
        });
        Ok(())
    }

    /// `#"…"#`, `##"…"##` and their multi-line forms: no escapes.
    fn raw(&mut self) -> Result<String, Error> {
        let start = self.i;
        let hashes = self.rest().chars().take_while(|&c| c == '#').count();
        self.i += hashes;
        if self.peek() != Some('"') {
            return error(start, "a raw string is written #\"…\"#");
        }
        let close: String = std::iter::once('"')
            .chain(std::iter::repeat_n('#', hashes))
            .collect();
        if self.rest().starts_with("\"\"\"") {
            self.i += 3;
            return self.multi_line(start, hashes);
        }
        self.i += 1;
        let body_start = self.i;
        loop {
            if self.rest().starts_with(&close) {
                let body = self.src[body_start..self.i].to_string();
                self.i += close.len();
                return Ok(body);
            }
            match self.peek() {
                None => return error(start, format!("a raw string is never closed with {close}")),
                Some(c) if is_newline(c) => {
                    return error(
                        self.i,
                        "a raw string cannot hold a newline; use a #\"\"\" multi-line raw string",
                    )
                }
                Some(c) => self.i += c.len_utf8(),
            }
        }
    }

    /// The body of a multi-line string after its opening `"""` (with
    /// `hashes` `#`s for a raw one): newline first, then lines dedented by
    /// the final line's whitespace, whitespace escapes resolved before the
    /// dedent and other escapes after.
    fn multi_line(&mut self, start: usize, hashes: usize) -> Result<String, Error> {
        let raw = hashes > 0;
        if !self.newline() {
            return error(
                start,
                "a multi-line string must start a new line right after its opening \"\"\"",
            );
        }
        let close: String = "\"\"\""
            .chars()
            .chain(std::iter::repeat_n('#', hashes))
            .collect();
        // The body with whitespace escapes resolved and every newline as
        // \n; other escapes stay as written, resolved after the dedent.
        let mut body = String::new();
        loop {
            if self.rest().starts_with(&close) {
                self.i += close.len();
                break;
            }
            if let Some(n) = self.newline_at(self.i) {
                body.push('\n');
                self.i += n;
                continue;
            }
            match self.peek() {
                None => {
                    return error(
                        start,
                        format!("a multi-line string is never closed with {close}"),
                    )
                }
                Some('\\') if !raw => {
                    let next = self.peek_at(self.i + 1);
                    if next.is_some_and(|c| is_space(c) || is_newline(c)) {
                        self.i += 1;
                        while let Some(c) = self.peek() {
                            if is_space(c) {
                                self.i += c.len_utf8();
                            } else if !self.newline() {
                                break;
                            }
                        }
                    } else {
                        // kept for later, the escaped character with it, so
                        // an escaped quote cannot close the string
                        body.push('\\');
                        self.i += 1;
                        if let Some(c) = self.peek() {
                            body.push(c);
                            self.i += c.len_utf8();
                        }
                    }
                }
                Some(c) => {
                    body.push(c);
                    self.i += c.len_utf8();
                }
            }
        }

        // the last line is the indentation to remove, and only that
        let (lines, prefix) = match body.rfind('\n') {
            Some(at) => (Some(&body[..at]), &body[at + 1..]),
            None => (None, body.as_str()),
        };
        if !prefix.chars().all(is_space) {
            return error(
                start,
                "a multi-line string's closing \"\"\" must be on a line of its own, after only whitespace",
            );
        }
        let mut dedented = Vec::new();
        for line in lines.into_iter().flat_map(|l| l.split('\n')) {
            if line.chars().all(is_space) {
                dedented.push("");
            } else if let Some(rest) = line.strip_prefix(prefix) {
                dedented.push(rest);
            } else {
                return error(
                    start,
                    "every line of a multi-line string must start with the same whitespace as its closing line",
                );
            }
        }
        let joined = dedented.join("\n");
        if raw {
            return Ok(joined);
        }
        // the remaining escapes, now the indentation is gone
        let mut sub = Parser::new(&joined);
        let mut out = String::new();
        while let Some(c) = sub.peek() {
            if c == '\\' {
                sub.escape(&mut out).map_err(|e| Error {
                    offset: start,
                    message: e.message,
                })?;
            } else {
                out.push(c);
                sub.i += c.len_utf8();
            }
        }
        Ok(out)
    }

    // --- values ------------------------------------------------------------

    /// `(type)`, if there is one.
    fn type_annotation(&mut self) -> Result<Option<Ident>, Error> {
        if self.peek() != Some('(') {
            return Ok(None);
        }
        let start = self.i;
        self.i += 1;
        self.node_space()?;
        let mut ty = self.string()?;
        self.node_space()?;
        if self.peek() != Some(')') {
            return error(
                start,
                "a type annotation is written (type), and this one is not closed",
            );
        }
        self.i += 1;
        ty.span = span(start, self.i);
        Ok(Some(ty))
    }

    /// A keyword (`#true`, `#null`, `#inf`, …) or a raw string, at `#`.
    fn keyword_or_raw(&mut self) -> Result<Value, Error> {
        if matches!(self.peek_at(self.i + 1), Some('#' | '"')) {
            return Ok(Value::String(self.raw()?));
        }
        let start = self.i;
        self.i += 1;
        while self.peek().is_some_and(is_identifier_char) {
            self.i += self.peek().map_or(1, char::len_utf8);
        }
        Ok(match &self.src[start + 1..self.i] {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "null" => Value::Null,
            "inf" => Value::Float(f64::INFINITY),
            "-inf" => Value::Float(f64::NEG_INFINITY),
            "nan" => Value::Float(f64::NAN),
            other => {
                return error(
                    start,
                    format!("#{other} is not a keyword (#true, #false, #null, #inf, #-inf, #nan)"),
                )
            }
        })
    }

    /// A number, at a digit or a sign before one.
    fn number(&mut self) -> Result<Value, Error> {
        let start = self.i;
        while self.peek().is_some_and(is_identifier_char) {
            self.i += self.peek().map_or(1, char::len_utf8);
        }
        let text = &self.src[start..self.i];
        let bad = || error(start, format!("'{text}' is not a number"));
        let (negative, unsigned) = match text.as_bytes()[0] {
            b'-' => (true, &text[1..]),
            b'+' => (false, &text[1..]),
            _ => (false, text),
        };
        let radix = match unsigned.get(..2) {
            Some("0x") => Some(16),
            Some("0o") => Some(8),
            Some("0b") => Some(2),
            _ => None,
        };
        if let Some(radix) = radix {
            let digits = &unsigned[2..];
            let ok = digits.chars().next().is_some_and(|c| c.is_digit(radix))
                && digits.chars().all(|c| c == '_' || c.is_digit(radix));
            if !ok {
                return bad();
            }
            let clean: String = digits.chars().filter(|&c| c != '_').collect();
            return match i128::from_str_radix(&clean, radix) {
                Ok(n) => Ok(Value::Integer(if negative { -n } else { n })),
                Err(_) => error(start, format!("'{text}' is too large a number")),
            };
        }
        // decimal: integer ('.' integer)? exponent?, where an integer is a
        // digit then digits and underscores
        let b = unsigned.as_bytes();
        let mut k = 0;
        let integer = |k: &mut usize| -> bool {
            if *k >= b.len() || !b[*k].is_ascii_digit() {
                return false;
            }
            while *k < b.len() && (b[*k].is_ascii_digit() || b[*k] == b'_') {
                *k += 1;
            }
            true
        };
        if !integer(&mut k) {
            return bad();
        }
        let mut float = false;
        if k < b.len() && b[k] == b'.' {
            k += 1;
            if !integer(&mut k) {
                return bad();
            }
            float = true;
        }
        if k < b.len() && matches!(b[k], b'e' | b'E') {
            k += 1;
            if k < b.len() && matches!(b[k], b'+' | b'-') {
                k += 1;
            }
            if !integer(&mut k) {
                return bad();
            }
            float = true;
        }
        if k != b.len() {
            return bad();
        }
        let clean: String = text.chars().filter(|&c| c != '_').collect();
        if float {
            match clean.parse::<f64>() {
                Ok(f) => Ok(Value::Float(f)),
                Err(_) => bad(),
            }
        } else {
            match clean.parse::<i128>() {
                Ok(n) => Ok(Value::Integer(n)),
                Err(_) => error(start, format!("'{text}' is too large a number")),
            }
        }
    }

    /// A value: a typed or untyped string, number or keyword, and the
    /// span of its literal.
    fn value(&mut self) -> Result<(Option<Ident>, Value, Span), Error> {
        let ty = self.type_annotation()?;
        if ty.is_some() {
            self.node_space()?;
        }
        let start = self.i;
        let value = match self.peek() {
            Some('#') => self.keyword_or_raw()?,
            Some('"') => Value::String(self.quoted()?),
            Some(c) if c.is_ascii_digit() => self.number()?,
            Some('+' | '-') if self.peek_at(self.i + 1).is_some_and(|c| c.is_ascii_digit()) => {
                self.number()?
            }
            _ => Value::String(self.identifier()?),
        };
        Ok((ty, value, span(start, self.i)))
    }

    /// An entry: a property (`key=value`) or an argument.
    fn entry(&mut self) -> Result<Entry, Error> {
        let start = self.i;
        let string_start = matches!(self.peek(), Some('"'))
            || (self.peek() == Some('#') && matches!(self.peek_at(self.i + 1), Some('#' | '"')))
            || self.peek().is_some_and(|c| {
                is_identifier_char(c)
                    && !c.is_ascii_digit()
                    && !(matches!(c, '+' | '-')
                        && self.peek_at(self.i + 1).is_some_and(|d| d.is_ascii_digit()))
            });
        if string_start {
            let key = self.string()?;
            let after_key = self.i;
            self.node_space()?;
            if self.peek() == Some('=') {
                self.i += 1;
                self.node_space()?;
                let (ty, value, value_span) = self.value()?;
                return Ok(Entry {
                    name: Some(key),
                    ty,
                    value,
                    span: span(start, self.i),
                    value_span,
                });
            }
            // an argument after all; what followed is the separator
            self.i = after_key;
            return Ok(Entry {
                name: None,
                ty: None,
                value: Value::String(key.value),
                span: span(start, after_key),
                value_span: span(start, after_key),
            });
        }
        let (ty, value, value_span) = self.value()?;
        Ok(Entry {
            name: None,
            ty,
            value,
            span: span(start, self.i),
            value_span,
        })
    }

    // --- nodes -------------------------------------------------------------

    fn document(mut self) -> Result<Document, Error> {
        if let Some((at, c)) = self
            .src
            .char_indices()
            .find(|&(at, c)| is_disallowed(c) && !(at == 0 && c == '\u{FEFF}'))
        {
            return error(
                at,
                format!("U+{:04X} may not appear in a KDL document", c as u32),
            );
        }
        if self.src.starts_with('\u{FEFF}') {
            self.i = 3;
        }
        let mut levels = vec![Level {
            open: None,
            discard: false,
            slashdashed: false,
            owner: None,
        }];
        // a node being read, when not between nodes
        let mut open: Option<Open> = None;

        loop {
            let Some(mut node) = open.take() else {
                // between nodes, in the innermost open block
                self.line_space()?;
                let level = levels.last().expect("the document is a level");
                match self.peek() {
                    None => {
                        if let Some(at) = level.open {
                            return error(at, "a { block is never closed with }");
                        }
                        return Ok(self.doc);
                    }
                    Some('}') => {
                        let Some(brace) = level.open else {
                            return error(self.i, "a } closes no block");
                        };
                        let close = self.i;
                        self.i += 1;
                        let closed = levels.pop().expect("an open block");
                        let mut owner = closed.owner.expect("a block belongs to a node");
                        owner.block = true;
                        if !closed.slashdashed {
                            owner.real_block = true;
                            if !owner.discard {
                                owner.end = self.i;
                            }
                            if let Some(idx) = owner.node {
                                self.doc.nodes[idx].body = Some(span(brace + 1, close));
                            }
                        }
                        open = Some(owner);
                        continue;
                    }
                    Some(_) => {}
                }
                let discard_here = level.discard;
                let mut slashdashed = false;
                if self.rest().starts_with("/-") {
                    self.after_slashdash()?;
                    slashdashed = true;
                }
                let node_start = self.i;
                let ty = self.type_annotation()?;
                if ty.is_some() {
                    self.node_space()?;
                }
                if matches!(self.peek(), Some('{' | '}' | ';') | None)
                    || self.newline_at(self.i).is_some()
                {
                    return error(self.i, "expected a node name");
                }
                let name = self.string()?;
                let discard = discard_here || slashdashed;
                let idx = if discard {
                    None
                } else {
                    let idx = self.doc.nodes.len();
                    self.doc.nodes.push(Node {
                        name,
                        ty,
                        entries: Vec::new(),
                        children: None,
                        span: span(node_start, self.i),
                        body: None,
                    });
                    match levels
                        .last()
                        .and_then(|l| l.owner.as_ref())
                        .and_then(|o| o.node)
                    {
                        Some(parent) => self.doc.nodes[parent]
                            .children
                            .get_or_insert_with(Vec::new)
                            .push(idx),
                        None => self.doc.roots.push(idx),
                    }
                    Some(idx)
                };
                open = Some(Open {
                    node: idx,
                    discard,
                    start: node_start,
                    end: self.i,
                    block: false,
                    real_block: false,
                });
                continue;
            };

            // inside a node: entries, then children blocks, then its end
            let spaced = self.node_space()?;
            let at = self.i;
            match self.peek() {
                None | Some(';' | '}') => {}
                Some(_) if self.newline_at(at).is_some() || self.rest().starts_with("//") => {}
                Some('{') => {
                    if node.real_block {
                        return error(
                            at,
                            "a node has one children block (more may be slashdashed out)",
                        );
                    }
                    self.i += 1;
                    if let Some(idx) = node.node {
                        self.doc.nodes[idx].children = Some(Vec::new());
                    }
                    let discard = node.discard;
                    levels.push(Level {
                        open: Some(at),
                        discard,
                        slashdashed: false,
                        owner: Some(node),
                    });
                    continue;
                }
                Some('/') if self.rest().starts_with("/-") => {
                    self.after_slashdash()?;
                    if self.peek() == Some('{') {
                        let brace = self.i;
                        self.i += 1;
                        levels.push(Level {
                            open: Some(brace),
                            discard: true,
                            slashdashed: true,
                            owner: Some(node),
                        });
                        continue;
                    }
                    if node.block {
                        return error(at, "an entry cannot follow a children block");
                    }
                    self.entry()?;
                    open = Some(node);
                    continue;
                }
                Some(c) => {
                    if node.block {
                        return error(at, "an entry cannot follow a children block");
                    }
                    if !spaced {
                        return error(at, format!("expected whitespace before '{c}'"));
                    }
                    let entry = self.entry()?;
                    if let Some(idx) = node.node {
                        node.end = self.i;
                        self.doc.nodes[idx].entries.push(entry);
                    }
                    open = Some(node);
                    continue;
                }
            }
            // the node ends: by ';', a newline, a comment, the end of its
            // block or of the document
            if let Some(idx) = node.node {
                self.doc.nodes[idx].span = span(node.start, node.end);
            }
            if self.peek() == Some(';') {
                self.i += 1;
            } else if self.rest().starts_with("//") {
                self.single_line_comment();
            } else {
                self.newline();
            }
            open = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(doc: &Document) -> Vec<String> {
        doc.nodes().map(|n| n.name().value().to_string()).collect()
    }

    #[test]
    fn reads_nodes_entries_and_children() {
        let doc = Document::parse("a 1 b=\"x\" {\n  c #true; d\n}\ne").unwrap();
        assert_eq!(names(&doc), ["a", "e"]);
        let a = doc.nodes().next().unwrap();
        assert_eq!(a.entries()[0].value(), &Value::Integer(1));
        assert_eq!(a.entries()[1].name().unwrap().value(), "b");
        assert_eq!(a.entries()[1].value().as_string(), Some("x"));
        let kids: Vec<_> = a
            .children()
            .unwrap()
            .nodes()
            .map(|n| n.name().value().to_string())
            .collect();
        assert_eq!(kids, ["c", "d"]);
    }

    #[test]
    fn nesting_costs_no_stack() {
        const N: usize = 200_000;
        let src = format!("{}leaf{}", "n {\n".repeat(N), "\n}".repeat(N));
        let doc = Document::parse(&src).unwrap();
        let mut node = doc.nodes().next().unwrap();
        let mut depth = 1;
        while let Some(kids) = node.children() {
            node = kids.nodes().next().unwrap();
            depth += 1;
        }
        assert_eq!(depth, N + 1);
        assert_eq!(node.name().value(), "leaf");
        let copy = doc.clone();
        assert_eq!(copy, doc);
    }
}
