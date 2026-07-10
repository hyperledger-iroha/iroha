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
        let mut tokens = Vec::new();
        let mut pending = self.root.children.iter().rev().collect::<Vec<_>>();
        while let Some(element) = pending.pop() {
            match element {
                GreenElement::Node(node) => pending.extend(node.children.iter().rev()),
                GreenElement::Token(token) => tokens.push(token),
            }
        }
        tokens
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
                    parent.children.push(GreenElement::Token(token.clone()));
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
