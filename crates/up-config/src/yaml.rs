//! Strict YAML loader (ARCH-11, ARCH-05, D1-11).
//!
//! Rust (YAML 1.2) and Ansible/PyYAML (YAML 1.1) read some documents
//! differently. The loader removes the differential by refusing, before any
//! deserialization, the constructs the two parsers disagree on. Errors name
//! the construct and where it is, never the scalar values around it, and every
//! piece of text in an error is escaped to printable ASCII.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;

use granit_parser::{Event, Options as ParserOptions, Parser, ScalarStyle, Span};

/// Resource limits applied before and during loading (T-01-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Deepest allowed nesting of mappings and sequences (the root mapping is depth 1).
    pub max_depth: usize,
    /// Most nodes (scalars, sequences, mappings) in one document.
    pub max_nodes: usize,
    /// Longest single scalar, in bytes.
    pub max_scalar_bytes: usize,
    /// Longest input, in bytes. Checked before any parsing.
    pub max_input_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_depth: 64,
            max_nodes: 100_000,
            max_scalar_bytes: 1_048_576,
            max_input_bytes: 4_194_304,
        }
    }
}

/// Why a document was refused.
///
/// Located variants carry the origin (usually a file name), a JSON-pointer
/// style key path, and a 1-based line and column. A line of 0 means the
/// position is not known. Display output is always escaped ASCII.
#[derive(Debug, thiserror::Error)]
pub enum YamlError {
    #[error("{}", self.render())]
    Anchor {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    Alias {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    MergeKey {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    Tag {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    DuplicateKey {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    NonStringKey {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    MultipleDocuments {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    Empty { origin: String },
    #[error("{}", self.render())]
    RootNotMapping {
        origin: String,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    Limit {
        origin: String,
        what: &'static str,
        path: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    Syntax {
        origin: String,
        message: String,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    Deserialize {
        origin: String,
        /// JSON pointer to the containing key; never the offending key or value itself.
        path: String,
        /// Fixed error category; never text taken from the source.
        category: &'static str,
        /// For a missing field, the field's name from the target type (not from the source).
        field: Option<&'static str>,
        line: usize,
        col: usize,
    },
    #[error("{}", self.render())]
    Io {
        origin: String,
        #[source]
        source: std::io::Error,
    },
}

/// Longest library message kept in a Syntax error.
const MAX_MESSAGE_CHARS: usize = 200;

impl YamlError {
    /// Stable machine name of the error class, used by the golden corpus headers.
    pub fn kind(&self) -> &'static str {
        match self {
            YamlError::Anchor { .. } => "anchor",
            YamlError::Alias { .. } => "alias",
            YamlError::MergeKey { .. } => "merge-key",
            YamlError::Tag { .. } => "tag",
            YamlError::DuplicateKey { .. } => "duplicate-key",
            YamlError::NonStringKey { .. } => "non-string-key",
            YamlError::MultipleDocuments { .. } => "multiple-documents",
            YamlError::Empty { .. } => "empty",
            YamlError::RootNotMapping { .. } => "root-not-mapping",
            YamlError::Limit { .. } => "limit",
            YamlError::Syntax { .. } => "syntax",
            YamlError::Deserialize { .. } => "deserialize",
            YamlError::Io { .. } => "io",
        }
    }

    /// The origin the error was raised for, escaped.
    pub fn origin(&self) -> String {
        let origin = match self {
            YamlError::Anchor { origin, .. }
            | YamlError::Alias { origin, .. }
            | YamlError::MergeKey { origin, .. }
            | YamlError::Tag { origin, .. }
            | YamlError::DuplicateKey { origin, .. }
            | YamlError::NonStringKey { origin, .. }
            | YamlError::MultipleDocuments { origin, .. }
            | YamlError::Empty { origin }
            | YamlError::RootNotMapping { origin, .. }
            | YamlError::Limit { origin, .. }
            | YamlError::Syntax { origin, .. }
            | YamlError::Deserialize { origin, .. }
            | YamlError::Io { origin, .. } => origin,
        };
        escape(origin)
    }

    /// The description without the origin prefix, for callers that print their own.
    pub fn detail(&self) -> String {
        match self {
            YamlError::Anchor {
                path, line, col, ..
            } => located("anchor", path, *line, *col),
            YamlError::Alias {
                path, line, col, ..
            } => located("alias", path, *line, *col),
            YamlError::MergeKey {
                path, line, col, ..
            } => located("merge key", path, *line, *col),
            YamlError::Tag {
                path, line, col, ..
            } => located("tag", path, *line, *col),
            YamlError::DuplicateKey {
                path, line, col, ..
            } => located("duplicate key in mapping", path, *line, *col),
            YamlError::NonStringKey {
                path, line, col, ..
            } => located("non-string key", path, *line, *col),
            YamlError::MultipleDocuments {
                path, line, col, ..
            } => located("second document", path, *line, *col),
            YamlError::Empty { .. } => "empty document".to_string(),
            YamlError::RootNotMapping {
                path, line, col, ..
            } => located("root is not a mapping", path, *line, *col),
            YamlError::Limit {
                what,
                path,
                line,
                col,
                ..
            } => located(&format!("limit exceeded: {what}"), path, *line, *col),
            YamlError::Syntax {
                message, line, col, ..
            } => format!("syntax error: {}{}", escape(message), position(*line, *col)),
            YamlError::Deserialize {
                path,
                category,
                field,
                line,
                col,
                ..
            } => {
                let named = field.map(|f| format!(" `{f}`")).unwrap_or_default();
                located(
                    &format!("does not match the expected shape: {category}{named}"),
                    path,
                    *line,
                    *col,
                )
            }
            YamlError::Io { source, .. } => {
                format!("cannot read file: {}", escape(&source.to_string()))
            }
        }
    }

    fn render(&self) -> String {
        format!("{}: {}", self.origin(), self.detail())
    }
}

fn position(line: usize, col: usize) -> String {
    if line == 0 {
        String::new()
    } else {
        format!(" (line {line}, column {col})")
    }
}

fn located(what: &str, path: &str, line: usize, col: usize) -> String {
    let shown = if path.is_empty() { "/" } else { path };
    format!("{what} at {}{}", escape(shown), position(line, col))
}

/// Escape text to printable ASCII: control characters, DEL and every
/// non-ASCII character become `\u{XXXX}`. Idempotent.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == ' ' || c.is_ascii_graphic() {
            out.push(c);
        } else {
            let _ = write!(out, "\\u{{{:04X}}}", c as u32);
        }
    }
    out
}

/// Escape one mapping key for use as a JSON-pointer segment.
fn segment(key: &str) -> String {
    escape(&key.replace('~', "~0").replace('/', "~1"))
}

fn cap(message: &str) -> String {
    message.chars().take(MAX_MESSAGE_CHARS).collect()
}

/// A mapping or sequence being walked by the pre-scan.
enum Frame {
    Map {
        path: String,
        expect_key: bool,
        key: String,
        seen: HashSet<String>,
    },
    Seq {
        path: String,
        index: usize,
    },
}

/// Where a node sits relative to its parent.
enum Slot {
    Root,
    Key { map_path: String },
    Value { path: String },
}

struct Scan<'a> {
    origin: &'a str,
    limits: &'a Limits,
    stack: Vec<Frame>,
    documents: usize,
    nodes: usize,
    root_done: bool,
}

type At = (usize, usize);

fn at(span: &Span) -> At {
    (span.start.line(), span.start.col() + 1)
}

/// Where the tag token is, falling back to the node itself.
fn tag_position(span: &Span) -> At {
    span.tag_start
        .map(|m| (m.line(), m.col() + 1))
        .unwrap_or_else(|| at(span))
}

impl Scan<'_> {
    fn syntax(&self, message: &str, (line, col): At) -> YamlError {
        YamlError::Syntax {
            origin: self.origin.to_string(),
            message: cap(message),
            line,
            col,
        }
    }

    /// Where the next node goes. Does not change any state.
    fn slot(&self) -> Slot {
        match self.stack.last() {
            None => Slot::Root,
            Some(Frame::Map {
                path,
                expect_key: true,
                ..
            }) => Slot::Key {
                map_path: path.clone(),
            },
            Some(Frame::Map { path, key, .. }) => Slot::Value {
                path: format!("{path}/{}", segment(key)),
            },
            Some(Frame::Seq { path, index }) => Slot::Value {
                path: format!("{path}/{index}"),
            },
        }
    }

    /// A node finished: its parent moves on to the next key or item.
    fn node_done(&mut self) {
        match self.stack.last_mut() {
            None => self.root_done = true,
            Some(Frame::Map { expect_key, .. }) => *expect_key = true,
            Some(Frame::Seq { index, .. }) => *index += 1,
        }
    }

    fn located(
        &self,
        kind: fn(String, String, usize, usize) -> YamlError,
        path: &str,
        (line, col): At,
    ) -> YamlError {
        kind(self.origin.to_string(), path.to_string(), line, col)
    }

    fn limit(&self, what: &'static str, path: &str, (line, col): At) -> YamlError {
        YamlError::Limit {
            origin: self.origin.to_string(),
            what,
            path: path.to_string(),
            line,
            col,
        }
    }

    /// Checks shared by every node start. `anchor` is the parser's anchor id (0 = none).
    /// Returns the node's path.
    fn check_node(
        &mut self,
        slot: &Slot,
        anchor: usize,
        tag_at: Option<At>,
        position: At,
    ) -> Result<String, YamlError> {
        let path = match slot {
            Slot::Root => String::new(),
            Slot::Key { map_path } => map_path.clone(),
            Slot::Value { path } => path.clone(),
        };
        if anchor != 0 {
            return Err(self.located(
                |origin, path, line, col| YamlError::Anchor {
                    origin,
                    path,
                    line,
                    col,
                },
                &path,
                position,
            ));
        }
        if let Some(tag_position) = tag_at {
            return Err(self.located(
                |origin, path, line, col| YamlError::Tag {
                    origin,
                    path,
                    line,
                    col,
                },
                &path,
                tag_position,
            ));
        }
        self.nodes += 1;
        if self.nodes > self.limits.max_nodes {
            return Err(self.limit("node count", &path, position));
        }
        Ok(path)
    }

    fn alias(&mut self, span: &Span) -> Result<(), YamlError> {
        let slot = self.slot();
        let path = match &slot {
            Slot::Root => String::new(),
            Slot::Key { map_path } => map_path.clone(),
            Slot::Value { path } => path.clone(),
        };
        Err(self.located(
            |origin, path, line, col| YamlError::Alias {
                origin,
                path,
                line,
                col,
            },
            &path,
            at(span),
        ))
    }

    fn scalar(
        &mut self,
        text: &str,
        style: ScalarStyle,
        anchor: usize,
        tag_at: Option<At>,
        span: &Span,
    ) -> Result<(), YamlError> {
        let slot = self.slot();
        let path = self.check_node(&slot, anchor, tag_at, at(span))?;
        if text.len() > self.limits.max_scalar_bytes {
            return Err(self.limit("scalar size", &path, at(span)));
        }
        match slot {
            Slot::Root => {
                let null_like = style == ScalarStyle::Plain
                    && matches!(text, "" | "~" | "null" | "Null" | "NULL");
                return Err(if null_like {
                    YamlError::Empty {
                        origin: self.origin.to_string(),
                    }
                } else {
                    let (line, col) = at(span);
                    YamlError::RootNotMapping {
                        origin: self.origin.to_string(),
                        path,
                        line,
                        col,
                    }
                });
            }
            Slot::Key { map_path } => {
                if text == "<<" {
                    return Err(self.located(
                        |origin, path, line, col| YamlError::MergeKey {
                            origin,
                            path,
                            line,
                            col,
                        },
                        &format!("{map_path}/{}", segment(text)),
                        at(span),
                    ));
                }
                let fresh = match self.stack.last_mut() {
                    Some(Frame::Map {
                        expect_key,
                        key,
                        seen,
                        ..
                    }) => {
                        *expect_key = false;
                        *key = text.to_string();
                        seen.insert(text.to_string())
                    }
                    _ => true,
                };
                if !fresh {
                    return Err(self.located(
                        |origin, path, line, col| YamlError::DuplicateKey {
                            origin,
                            path,
                            line,
                            col,
                        },
                        &map_path,
                        at(span),
                    ));
                }
            }
            Slot::Value { .. } => self.node_done(),
        }
        Ok(())
    }

    fn open(
        &mut self,
        is_map: bool,
        anchor: usize,
        tag_at: Option<At>,
        span: &Span,
    ) -> Result<(), YamlError> {
        let slot = self.slot();
        let path = self.check_node(&slot, anchor, tag_at, at(span))?;
        if matches!(slot, Slot::Key { .. }) {
            return Err(self.located(
                |origin, path, line, col| YamlError::NonStringKey {
                    origin,
                    path,
                    line,
                    col,
                },
                &path,
                at(span),
            ));
        }
        if self.stack.len() + 1 > self.limits.max_depth {
            return Err(self.limit("depth", &path, at(span)));
        }
        if matches!(slot, Slot::Root) && !is_map {
            let (line, col) = at(span);
            return Err(YamlError::RootNotMapping {
                origin: self.origin.to_string(),
                path,
                line,
                col,
            });
        }
        self.stack.push(if is_map {
            Frame::Map {
                path,
                expect_key: true,
                key: String::new(),
                seen: HashSet::new(),
            }
        } else {
            Frame::Seq { path, index: 0 }
        });
        Ok(())
    }

    fn close(&mut self) {
        self.stack.pop();
        self.node_done();
    }
}

/// Walk the parser events once and refuse anything the strict profile forbids.
/// Reports the first violation in document order.
fn prescan(src: &str, origin: &str, limits: &Limits) -> Result<(), YamlError> {
    let mut options = ParserOptions::default();
    options.set_emit_comments(false);
    let mut parser = Parser::new_from_str_with_options(src, options);
    let mut scan = Scan {
        origin,
        limits,
        stack: Vec::new(),
        documents: 0,
        nodes: 0,
        root_done: false,
    };
    while let Some(item) = parser.next_event() {
        let (event, span) = match item {
            Ok(pair) => pair,
            Err(e) => {
                let marker = e.marker();
                let position = (marker.line(), marker.col() + 1);
                return Err(scan.syntax(&e.info(), position));
            }
        };
        match event {
            Event::DocumentStart(..) => {
                scan.documents += 1;
                if scan.documents > 1 {
                    let (line, col) = at(&span);
                    return Err(YamlError::MultipleDocuments {
                        origin: origin.to_string(),
                        path: String::new(),
                        line,
                        col,
                    });
                }
            }
            Event::Alias(_) => scan.alias(&span)?,
            Event::Scalar(text, style, anchor, tag) => {
                scan.scalar(
                    &text,
                    style,
                    anchor,
                    tag.is_some().then(|| tag_position(&span)),
                    &span,
                )?;
            }
            Event::SequenceStart(_, anchor, tag) => {
                scan.open(
                    false,
                    anchor,
                    tag.is_some().then(|| tag_position(&span)),
                    &span,
                )?;
            }
            Event::MappingStart(_, anchor, tag) => {
                scan.open(
                    true,
                    anchor,
                    tag.is_some().then(|| tag_position(&span)),
                    &span,
                )?;
            }
            Event::SequenceEnd | Event::MappingEnd => scan.close(),
            _ => {}
        }
    }
    if scan.documents == 0 || !scan.root_done {
        return Err(YamlError::Empty {
            origin: origin.to_string(),
        });
    }
    Ok(())
}

fn parse_options(limits: &Limits) -> serde_saphyr::Options {
    let mut options = serde_saphyr::Options::default();
    let mut budget = serde_saphyr::Budget::default();
    budget.max_depth = limits.max_depth;
    budget.max_nodes = limits.max_nodes;
    budget.max_total_scalar_bytes = limits.max_input_bytes;
    options.budget = Some(budget);
    // YAML 1.1 spellings (`yes`, `no`, `on`, `off`) stay strings; only `true`/`false` are booleans.
    options.strict_booleans = true;
    options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::Error;
    options.merge_keys = serde_saphyr::MergeKeyPolicy::Error;
    options.reject_unsupported_tags = true;
    options.with_snippet = false;
    options
}

/// A fixed category for a deserialization failure. Derived from the error's
/// variant only: nothing from the library's message (which can quote scalars
/// and keys from the source) is carried over.
fn deserialize_category(e: &serde_saphyr::Error) -> &'static str {
    use serde_saphyr::Error as E;
    match e {
        E::SerdeUnknownField { .. } => "unknown field",
        E::SerdeUnknownVariant { .. } | E::SerdeVariantId { .. } | E::TaggedEnumMismatch { .. } => {
            "unknown variant"
        }
        E::SerdeMissingField { .. } => "missing field",
        E::DuplicateMappingKey { .. } => "duplicate key",
        E::SerdeInvalidType { .. }
        | E::Unexpected { .. }
        | E::InvalidScalar { .. }
        | E::NullIntoString { .. }
        | E::InvalidBooleanStrict { .. }
        | E::InvalidCharNull { .. }
        | E::InvalidCharNotSingleScalar { .. }
        | E::UnexpectedValueForUnit { .. }
        | E::ExpectedEmptyMappingForUnitStruct { .. }
        | E::ExternallyTaggedEnumExpectedScalarOrMapping { .. }
        | E::UnexpectedValueForUnitEnumVariant { .. } => "invalid type",
        E::SerdeInvalidValue { .. }
        | E::SerdeInvalidLength { .. }
        | E::QuotingRequired { .. }
        | E::NonFiniteFloat { .. } => "invalid value",
        E::AliasError { error, .. } => deserialize_category(error),
        _ => "other",
    }
}

/// The JSON pointer of the node that starts at `target`, for an error raised
/// there. A mapping key resolves to its mapping (never to the key's own text),
/// so an unknown key is not echoed. Empty when no node starts there.
///
/// Runs only after [`prescan`] accepted the document, so it never meets an
/// anchor, alias, tag, merge key or duplicate key.
fn path_at(src: &str, limits: &Limits, target: At) -> String {
    let mut options = ParserOptions::default();
    options.set_emit_comments(false);
    let mut parser = Parser::new_from_str_with_options(src, options);
    let mut scan = Scan {
        origin: "",
        limits,
        stack: Vec::new(),
        documents: 0,
        nodes: 0,
        root_done: false,
    };
    while let Some(Ok((event, span))) = parser.next_event() {
        let here = at(&span);
        let found = |scan: &Scan| match scan.slot() {
            Slot::Root => String::new(),
            Slot::Key { map_path } => map_path,
            Slot::Value { path } => path,
        };
        let is_map = matches!(event, Event::MappingStart(..));
        match event {
            Event::DocumentStart(..) => scan.documents += 1,
            Event::Scalar(text, style, anchor, tag) => {
                if here == target {
                    return found(&scan);
                }
                if scan
                    .scalar(
                        &text,
                        style,
                        anchor,
                        tag.is_some().then(|| tag_position(&span)),
                        &span,
                    )
                    .is_err()
                {
                    break;
                }
            }
            Event::SequenceStart(_, anchor, tag) | Event::MappingStart(_, anchor, tag) => {
                if here == target {
                    return found(&scan);
                }
                if scan
                    .open(
                        is_map,
                        anchor,
                        tag.is_some().then(|| tag_position(&span)),
                        &span,
                    )
                    .is_err()
                {
                    break;
                }
            }
            Event::SequenceEnd | Event::MappingEnd => scan.close(),
            _ => {}
        }
    }
    String::new()
}

fn deserialize_error(
    src: &str,
    origin: &str,
    limits: &Limits,
    e: &serde_saphyr::Error,
) -> YamlError {
    let (line, col) = e
        .location()
        .map(|l| (l.line() as usize, l.column() as usize))
        .unwrap_or((0, 0));
    if matches!(e, serde_saphyr::Error::Budget { .. }) {
        return YamlError::Limit {
            origin: origin.to_string(),
            what: "parser budget",
            path: String::new(),
            line,
            col,
        };
    }
    let path = if line == 0 {
        String::new()
    } else {
        path_at(src, limits, (line, col))
    };
    YamlError::Deserialize {
        origin: origin.to_string(),
        path,
        category: deserialize_category(e),
        field: match e {
            serde_saphyr::Error::SerdeMissingField { field, .. } => Some(field),
            _ => None,
        },
        line,
        col,
    }
}

fn check_size(src: &str, origin: &str, limits: &Limits) -> Result<(), YamlError> {
    if src.len() > limits.max_input_bytes {
        return Err(YamlError::Limit {
            origin: origin.to_string(),
            what: "input size",
            path: String::new(),
            line: 0,
            col: 0,
        });
    }
    Ok(())
}

/// Load a strict single-document YAML mapping into a JSON value, with the default limits.
pub fn load_str(src: &str, origin: &str) -> Result<serde_json::Value, YamlError> {
    load_str_with(src, origin, &Limits::default())
}

/// Like [`load_str`] with explicit limits.
pub fn load_str_with(
    src: &str,
    origin: &str,
    limits: &Limits,
) -> Result<serde_json::Value, YamlError> {
    from_str_with(src, origin, limits)
}

/// Load a strict YAML file. The size cap is checked before the file is read in full.
pub fn load_file(path: &Path) -> Result<serde_json::Value, YamlError> {
    let origin = path.display().to_string();
    let limits = Limits::default();
    let io = |source: std::io::Error| YamlError::Io {
        origin: origin.clone(),
        source,
    };
    let file = std::fs::File::open(path).map_err(io)?;
    let len = file.metadata().map_err(io)?.len();
    let cap_bytes = limits.max_input_bytes as u64;
    if len > cap_bytes {
        return Err(YamlError::Limit {
            origin,
            what: "input size",
            path: String::new(),
            line: 0,
            col: 0,
        });
    }
    // Read at most cap + 1 bytes so a file that grows after the size check is still refused.
    let mut bytes = Vec::new();
    file.take(cap_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    let src = String::from_utf8(bytes).map_err(|_| YamlError::Syntax {
        origin: origin.clone(),
        message: "input is not valid UTF-8".to_string(),
        line: 0,
        col: 0,
    })?;
    load_str_with(&src, &origin, &limits)
}

/// Load a strict YAML document into `T`.
pub fn from_str<T: serde::de::DeserializeOwned>(src: &str, origin: &str) -> Result<T, YamlError> {
    from_str_with(src, origin, &Limits::default())
}

fn from_str_with<T: serde::de::DeserializeOwned>(
    src: &str,
    origin: &str,
    limits: &Limits,
) -> Result<T, YamlError> {
    check_size(src, origin, limits)?;
    prescan(src, origin, limits)?;
    serde_saphyr::from_str_with_options::<T>(src, parse_options(limits))
        .map_err(|e| deserialize_error(src, origin, limits, &e))
}
