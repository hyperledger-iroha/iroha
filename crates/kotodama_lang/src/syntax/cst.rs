//! Immutable lossless concrete syntax tree nodes.
use crate::source::{SourceFile, SourceId, TextRange};
use super::kind::SyntaxKind;
/// One lossless token in a concrete syntax tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GreenToken {
    /// Token kind.
    pub kind: SyntaxKind,
    /// Exact UTF-8 byte range in the source file.
    pub range: TextRange,
    /// Expected token kind for a zero-width recovery token.
    pub expected: Option<SyntaxKind>,
}
impl GreenToken {
    /// Construct a source-backed token.
    #[must_use]
    pub const fn source(kind: SyntaxKind, range: TextRange) -> Self {
        Self {
            kind,
            range,
            expected: None,
        }
    }
    /// Construct a zero-width missing token.
    #[must_use]
    pub const fn missing(offset: u32, expected: SyntaxKind) -> Self {
        Self {
            kind: SyntaxKind::Missing,
            range: TextRange::empty(offset),
            expected: Some(expected),
        }
    }
    /// Return whether this is a parser-inserted missing token.
    #[must_use]
    pub const fn is_missing(&self) -> bool {
        matches!(self.kind, SyntaxKind::Missing)
    }
}
/// A child of a concrete syntax node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GreenElement {
    /// Nested syntax node.
    Node(Box<GreenNode>),
    /// Leaf token.
    Token(GreenToken),
}
impl GreenElement {
    fn range(&self) -> TextRange {
        match self {
            Self::Node(node) => node.range,
            Self::Token(token) => token.range,
        }
    }
}
/// Immutable concrete syntax node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GreenNode {
    /// Node kind.
    pub kind: SyntaxKind,
    /// Range spanning all source-backed descendants.
    pub range: TextRange,
    /// Lossless children in source order.
    pub children: Vec<GreenElement>,
}
impl GreenNode {
    pub(crate) fn new(kind: SyntaxKind, fallback: u32, children: Vec<GreenElement>) -> Self {
        let first = children
            .iter()
            .map(GreenElement::range)
            .find(|range| !range.is_empty());
        let last = children
            .iter()
            .rev()
            .map(GreenElement::range)
            .find(|range| !range.is_empty());
        let range = match (first, last) {
            (Some(first), Some(last)) => TextRange::new(first.start, last.end),
            _ => TextRange::empty(fallback),
        };
        Self {
            kind,
            range,
            children,
        }
    }
}
/// Complete lossless concrete syntax tree for one source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxTree {
    source: SourceId,
    root: GreenNode,
}
impl SyntaxTree {
    pub(crate) fn new(source: SourceId, root: GreenNode) -> Self {
        Self { source, root }
    }
    /// Return the source identifier.
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }
    /// Return the root node.
    #[must_use]
    pub const fn root(&self) -> &GreenNode {
        &self.root
    }
    /// Reconstruct the exact source text represented by the tree.
    #[must_use]
    pub fn text(&self, source: &SourceFile) -> String {
        if source.id() != self.source {
            return String::new();
        }
        let mut output = String::with_capacity(source.text().len());
        let mut pending = self.root.children.iter().rev().collect::<Vec<_>>();
        while let Some(element) = pending.pop() {
            match element {
                GreenElement::Node(node) => pending.extend(node.children.iter().rev()),
                GreenElement::Token(token)
                    if !token.is_missing() && token.kind != SyntaxKind::Eof =>
                {
                    if let Some(text) = source.slice(token.range) {
                        output.push_str(text);
                    }
                }
                GreenElement::Token(_) => {}
            }
        }
        output
    }
    /// Return every leaf token in source order, including trivia, missing
    /// tokens, and end-of-file.
    #[must_use]
    pub fn tokens(&self) -> Vec<&GreenToken> {
        self.token_iter().collect()
    }
    pub(crate) fn token_iter(&self) -> GreenTokenIter<'_> {
        GreenTokenIter {
            pending: self.root.children.iter().rev().collect(),
        }
    }
    /// Consume the tree and return its tokens without recursively dropping
    /// nested green nodes.
    pub(crate) fn into_tokens(self) -> Vec<GreenToken> {
        let Self { source: _, root } = self;
        let mut pending = root.children.into_iter().rev().collect::<Vec<_>>();
        let mut tokens = Vec::new();
        while let Some(element) = pending.pop() {
            match element {
                GreenElement::Node(node) => {
                    let GreenNode { children, .. } = *node;
                    pending.extend(children.into_iter().rev());
                }
                GreenElement::Token(token) => tokens.push(token),
            }
        }
        tokens
    }
}
pub(crate) struct GreenTokenIter<'tree> {
    pending: Vec<&'tree GreenElement>,
}
impl<'tree> Iterator for GreenTokenIter<'tree> {
    type Item = &'tree GreenToken;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(element) = self.pending.pop() {
            match element {
                GreenElement::Node(node) => self.pending.extend(node.children.iter().rev()),
                GreenElement::Token(token) => return Some(token),
            }
        }
        None
    }
}
#[derive(Clone, Debug)]
pub(crate) struct SyntaxOutlineNode {
    pub(crate) kind: SyntaxKind,
    pub(crate) range: TextRange,
    parent: Option<usize>,
    children: Vec<usize>,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct SyntaxOutline {
    nodes: Vec<SyntaxOutlineNode>,
}
/// One parser-inserted token with the syntax node that owned the failed
/// expectation. Retaining the owner avoids guessing at equal child/parent
/// recovery boundaries after the outline has been completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MissingSyntax {
    pub(crate) offset: u32,
    pub(crate) expected: SyntaxKind,
    pub(crate) owner: Option<usize>,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct SyntaxOutlineBuilder {
    outline: SyntaxOutline,
    stack: Vec<usize>,
}
#[derive(Clone, Debug)]
pub(crate) struct SyntaxOutlineCheckpoint {
    node_len: usize,
    stack: Vec<usize>,
    child_lengths: Vec<(usize, usize)>,
}
impl SyntaxOutlineBuilder {
    pub(crate) fn start(&mut self, kind: SyntaxKind, start: u32) -> usize {
        let parent = self.stack.last().copied();
        let id = self.outline.nodes.len();
        self.outline.nodes.push(SyntaxOutlineNode {
            kind,
            range: TextRange::empty(start),
            parent,
            children: Vec::new(),
        });
        if let Some(parent) = parent {
            self.outline.nodes[parent].children.push(id);
        }
        self.stack.push(id);
        id
    }
    /// Complete `id` and any still-open descendants at the same recovery
    /// boundary. Successful parses finish in strict stack order; unwinding
    /// descendants here keeps malformed input structurally balanced.
    pub(crate) fn finish(&mut self, id: usize, end: u32) {
        while let Some(current) = self.stack.pop() {
            if let Some(node) = self.outline.nodes.get_mut(current) {
                node.range.end = end.max(node.range.start);
            }
            if current == id {
                return;
            }
        }
    }
    pub(crate) fn set_kind(&mut self, id: usize, kind: SyntaxKind) {
        if let Some(node) = self.outline.nodes.get_mut(id) {
            node.kind = kind;
        }
    }
    pub(crate) fn current(&self) -> Option<usize> {
        self.stack.last().copied()
    }
    pub(crate) fn checkpoint(&self) -> SyntaxOutlineCheckpoint {
        SyntaxOutlineCheckpoint {
            node_len: self.outline.nodes.len(),
            stack: self.stack.clone(),
            child_lengths: self
                .stack
                .iter()
                .map(|id| (*id, self.outline.nodes[*id].children.len()))
                .collect(),
        }
    }
    pub(crate) fn rollback(&mut self, checkpoint: SyntaxOutlineCheckpoint) {
        self.outline.nodes.truncate(checkpoint.node_len);
        for (id, child_len) in checkpoint.child_lengths {
            if let Some(node) = self.outline.nodes.get_mut(id) {
                node.children.truncate(child_len);
            }
        }
        self.stack = checkpoint.stack;
    }
    pub(crate) fn finish_open_nodes(&mut self, end: u32) {
        while let Some(id) = self.stack.pop() {
            if let Some(node) = self.outline.nodes.get_mut(id) {
                node.range.end = end.max(node.range.start);
            }
        }
    }
    pub(crate) fn into_outline(self) -> SyntaxOutline {
        self.outline
    }
}
/// Build one lossless tree from the structural decisions recorded by the
/// canonical AST parser and the original trivia-bearing lexer tape.
pub(crate) fn build_tree_from_outline(
    source: SourceId,
    tokens: &[GreenToken],
    outline: &SyntaxOutline,
    missing: &[MissingSyntax],
) -> SyntaxTree {
    let Some(root) = outline.nodes.first() else {
        return SyntaxTree::new(
            source,
            GreenNode::new(
                SyntaxKind::Root,
                0,
                tokens.iter().copied().map(GreenElement::Token).collect(),
            ),
        );
    };
    let mut depths = vec![0_usize; outline.nodes.len()];
    for (id, node) in outline.nodes.iter().enumerate() {
        depths[id] = node
            .parent
            .and_then(|parent| depths.get(parent).copied())
            .unwrap_or_default()
            .saturating_add(usize::from(node.parent.is_some()));
    }
    let mut direct_missing = vec![Vec::<(u32, SyntaxKind)>::new(); outline.nodes.len()];
    for missing in missing {
        let owner = missing
            .owner
            .filter(|owner| {
                outline.nodes.get(*owner).is_some_and(|node| {
                    node.range.start <= missing.offset && missing.offset <= node.range.end
                })
            })
            .or_else(|| {
                outline
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| {
                        node.range.start <= missing.offset && missing.offset <= node.range.end
                    })
                    .max_by_key(|(id, node)| {
                        (depths[*id], node.range.start, u32::MAX - node.range.end)
                    })
                    .map(|(id, _)| id)
            })
            .unwrap_or(0);
        direct_missing[owner].push((missing.offset, missing.expected));
    }
    for insertions in &mut direct_missing {
        insertions.sort_unstable_by_key(|(offset, _)| *offset);
    }
    struct Frame {
        node: usize,
        child: usize,
        missing: usize,
    }
    struct DirectEmissionBoundary {
        offset: u32,
        include_missing: bool,
        include_eof: bool,
    }
    fn emit_direct(
        events: &mut Vec<Event>,
        tokens: &[GreenToken],
        token: &mut usize,
        missing: &[(u32, SyntaxKind)],
        missing_index: &mut usize,
        boundary: DirectEmissionBoundary,
    ) {
        loop {
            let next_token = tokens.get(*token).filter(|token| {
                (token.kind == SyntaxKind::Eof && boundary.include_eof)
                    || token.range.start < boundary.offset
            });
            let next_missing = missing.get(*missing_index).filter(|(offset, _)| {
                *offset < boundary.offset
                    || (boundary.include_missing && *offset == boundary.offset)
            });
            match (next_token, next_missing) {
                (Some(token_value), Some((offset, expected)))
                    if *offset <= token_value.range.start =>
                {
                    events.push(Event::Missing {
                        expected: *expected,
                        offset: *offset,
                    });
                    *missing_index = missing_index.saturating_add(1);
                }
                (Some(_), _) => {
                    events.push(Event::Token(*token));
                    *token = token.saturating_add(1);
                }
                (None, Some((offset, expected))) => {
                    events.push(Event::Missing {
                        expected: *expected,
                        offset: *offset,
                    });
                    *missing_index = missing_index.saturating_add(1);
                }
                (None, None) => break,
            }
        }
    }
    let mut events = Vec::with_capacity(tokens.len().saturating_add(outline.nodes.len() * 2));
    events.push(Event::Start {
        kind: root.kind,
        offset: root.range.start,
    });
    let mut frames = vec![Frame {
        node: 0,
        child: 0,
        missing: 0,
    }];
    let mut token = 0_usize;
    while let Some(frame) = frames.last_mut() {
        let node = &outline.nodes[frame.node];
        if let Some(child_id) = node.children.get(frame.child).copied() {
            let child = &outline.nodes[child_id];
            emit_direct(
                &mut events,
                tokens,
                &mut token,
                &direct_missing[frame.node],
                &mut frame.missing,
                DirectEmissionBoundary {
                    offset: child.range.start,
                    include_missing: false,
                    include_eof: false,
                },
            );
            frame.child = frame.child.saturating_add(1);
            events.push(Event::Start {
                kind: child.kind,
                offset: child.range.start,
            });
            frames.push(Frame {
                node: child_id,
                child: 0,
                missing: 0,
            });
            continue;
        }
        let include_eof = frame.node == 0;
        emit_direct(
            &mut events,
            tokens,
            &mut token,
            &direct_missing[frame.node],
            &mut frame.missing,
            DirectEmissionBoundary {
                offset: node.range.end,
                include_missing: true,
                include_eof,
            },
        );
        events.push(Event::Finish {
            offset: node.range.end,
        });
        frames.pop();
    }
    while token < tokens.len() {
        events.insert(events.len().saturating_sub(1), Event::Token(token));
        token = token.saturating_add(1);
    }
    build_tree(source, tokens, events)
}
#[derive(Debug)]
pub(crate) enum Event {
    Start { kind: SyntaxKind, offset: u32 },
    Token(usize),
    Missing { expected: SyntaxKind, offset: u32 },
    Finish { offset: u32 },
}
struct NodeBuilder {
    kind: SyntaxKind,
    offset: u32,
    children: Vec<GreenElement>,
}
pub(crate) fn build_tree(
    source: SourceId,
    tokens: &[GreenToken],
    events: Vec<Event>,
) -> SyntaxTree {
    let mut stack = Vec::<NodeBuilder>::new();
    let mut root = None;
    for event in events {
        match event {
            Event::Start { kind, offset } => stack.push(NodeBuilder {
                kind,
                offset,
                children: Vec::new(),
            }),
            Event::Token(index) => {
                if let (Some(parent), Some(token)) = (stack.last_mut(), tokens.get(index)) {
                    parent.children.push(GreenElement::Token(*token));
                }
            }
            Event::Missing { expected, offset } => {
                if let Some(parent) = stack.last_mut() {
                    parent
                        .children
                        .push(GreenElement::Token(GreenToken::missing(offset, expected)));
                }
            }
            Event::Finish { offset } => {
                let Some(builder) = stack.pop() else {
                    continue;
                };
                let fallback = builder.offset.min(offset);
                let node = GreenNode::new(builder.kind, fallback, builder.children);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(GreenElement::Node(Box::new(node)));
                } else {
                    root = Some(node);
                }
            }
        }
    }
    let root = root.unwrap_or_else(|| GreenNode::new(SyntaxKind::Root, 0, Vec::new()));
    SyntaxTree::new(source, root)
}
