//! Deterministic formatter for the canonical Kotodama V1 token stream.

use crate::{
    ast::Item,
    diagnostic::{Diagnostic, DiagnosticBundle, DiagnosticPhase, SourcePosition, SourceSpan},
    source::{FrontendBudget, MAX_SOURCE_BYTES, SourceFile},
    syntax::{GreenToken, SyntaxKind},
};

const INDENT: &str = "    ";
const TARGET_COLUMNS: usize = 100;

/// Format one syntactically valid Kotodama V1 source file.
///
/// The formatter consumes the same lossless tokens used to build the compiler
/// AST. Invalid sources are returned as diagnostics rather than being partly
/// rewritten. Comments and literal spelling are preserved byte-for-byte;
/// whitespace between tokens is canonicalized.
pub fn format_source(
    source: &SourceFile,
    budget: FrontendBudget,
) -> Result<String, DiagnosticBundle> {
    let parsed = crate::syntax::parse_program(source, budget);
    let crate::syntax::ProgramParseOutput {
        tree,
        program,
        diagnostics,
        ..
    } = parsed;
    let tokens = tree
        .into_tokens()
        .into_iter()
        .filter(|token| {
            !matches!(
                token.kind,
                SyntaxKind::Whitespace | SyntaxKind::Missing | SyntaxKind::Eof
            )
        })
        .collect::<Vec<_>>();
    let Some(program) = program else {
        return Err(diagnostics);
    };
    debug_assert!(diagnostics.diagnostics.is_empty());
    let struct_names = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(definition) => Some(definition.name.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    crate::ast::drop_program_iterative(program);
    TokenFormatter::new(source, &tokens, &struct_names)
        .format()
        .ok_or_else(|| formatted_source_too_large(source))
}

fn formatted_source_too_large(source: &SourceFile) -> DiagnosticBundle {
    let range = source.full_range();
    let end = source.line_column(range.end);
    let mut diagnostic = Diagnostic::error(
        "K0001",
        DiagnosticPhase::Lex,
        format!(
            "canonical formatting would exceed the {MAX_SOURCE_BYTES}-byte Kotodama V1 source limit"
        ),
        Some(SourceSpan {
            package_identity: source.package_identity().map(str::to_owned),
            source: Some(source.name().to_owned()),
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition {
                line: end.line,
                column: end.column,
            },
            byte_range: Some(range),
        }),
    );
    diagnostic.help = Some(
        "Split the source into typed modules so the formatted deployable source remains within the V1 limit."
            .to_owned(),
    );
    DiagnosticBundle::single(diagnostic)
}

struct TokenFormatter<'source, 'tokens> {
    source: &'source SourceFile,
    tokens: &'tokens [GreenToken],
    output: String,
    indent: usize,
    at_line_start: bool,
    struct_names: &'tokens std::collections::BTreeSet<String>,
    parens: Vec<ParenFormat>,
    braces: Vec<BraceFormat>,
    generic_depth: usize,
    brackets: Vec<BracketFormat>,
    ternaries: Vec<(usize, usize, usize)>,
    previous: Option<SyntaxKind>,
    previous_was_prefix: bool,
    overflowed: bool,
}

#[derive(Clone, Copy)]
enum BraceFormat {
    Block,
    Match,
    StructLiteral {
        paren_depth: usize,
        bracket_depth: usize,
    },
    JsonObject {
        paren_depth: usize,
        bracket_depth: usize,
    },
}

#[derive(Clone, Copy)]
enum ParenFormat {
    Inline,
    Multiline { trailing_comma: bool },
}

#[derive(Clone, Copy)]
enum BracketFormat {
    Attribute,
    Inline,
    Multiline { trailing_comma: bool },
}

impl<'source, 'tokens> TokenFormatter<'source, 'tokens> {
    fn new(
        source: &'source SourceFile,
        tokens: &'tokens [GreenToken],
        struct_names: &'tokens std::collections::BTreeSet<String>,
    ) -> Self {
        Self {
            source,
            tokens,
            output: String::with_capacity(source.text().len()),
            indent: 0,
            at_line_start: true,
            struct_names,
            parens: Vec::new(),
            braces: Vec::new(),
            generic_depth: 0,
            brackets: Vec::new(),
            ternaries: Vec::new(),
            previous: None,
            previous_was_prefix: false,
            overflowed: false,
        }
    }

    fn format(mut self) -> Option<String> {
        for (index, token) in self.tokens.iter().enumerate() {
            let next = self.tokens.get(index + 1).map(|token| token.kind);
            let text = self.source.slice(token.range).unwrap_or_default();
            let previous_text = index
                .checked_sub(1)
                .and_then(|previous| self.tokens.get(previous))
                .and_then(|previous| self.source.slice(previous.range));
            match token.kind {
                SyntaxKind::LineComment => self.line_comment(text),
                SyntaxKind::BlockComment => self.block_comment(text),
                SyntaxKind::LBrace => {
                    let previous_significant_text = self.tokens[..index]
                        .iter()
                        .rev()
                        .find(|token| {
                            !matches!(
                                token.kind,
                                SyntaxKind::LineComment | SyntaxKind::BlockComment
                            )
                        })
                        .and_then(|token| self.source.slice(token.range));
                    let declaration_brace = index.checked_sub(2).is_some_and(|before_name| {
                        matches!(
                            self.tokens[before_name].kind,
                            SyntaxKind::KwStruct
                                | SyntaxKind::KwSeiyaku
                                | SyntaxKind::KwModule
                                | SyntaxKind::KwIf
                                | SyntaxKind::KwFor
                        )
                    });
                    let function_body_brace = self.tokens[..index]
                        .iter()
                        .rev()
                        .take_while(|token| {
                            !matches!(
                                token.kind,
                                SyntaxKind::Semicolon | SyntaxKind::LBrace | SyntaxKind::RBrace
                            )
                        })
                        .any(|token| token.kind == SyntaxKind::Arrow);
                    let json_object = previous_significant_text == Some("json");
                    let struct_literal = !json_object
                        && !declaration_brace
                        && !function_body_brace
                        && previous_text.is_some_and(|name| self.struct_names.contains(name));
                    let match_body = !struct_literal
                        && self.tokens[..index]
                            .iter()
                            .rev()
                            .take_while(|token| {
                                !matches!(
                                    token.kind,
                                    SyntaxKind::Semicolon | SyntaxKind::LBrace | SyntaxKind::RBrace
                                )
                            })
                            .any(|token| token.kind == SyntaxKind::KwMatch);
                    self.open_brace(struct_literal, json_object, match_body);
                }
                SyntaxKind::RBrace => self.close_brace(next),
                SyntaxKind::Semicolon => self.semicolon(),
                SyntaxKind::Comma => self.comma(),
                SyntaxKind::LParen => {
                    let multiline = matches!(
                        self.previous,
                        Some(
                            SyntaxKind::Ident
                                | SyntaxKind::KwAuthorize
                                | SyntaxKind::KwHajimari
                                | SyntaxKind::KwKaizen
                        )
                    ) && self.parentheses_exceed_target(index);
                    let format = if multiline {
                        ParenFormat::Multiline {
                            // `authorize("...")` is a declaration modifier,
                            // not a call argument list, and its grammar does
                            // not admit a trailing comma.
                            trailing_comma: self.previous != Some(SyntaxKind::KwAuthorize),
                        }
                    } else {
                        ParenFormat::Inline
                    };
                    self.ordinary(token.kind, text);
                    self.parens.push(format);
                    if multiline {
                        self.indent = self.indent.saturating_add(1);
                        self.newlines(1);
                    }
                }
                SyntaxKind::RParen => {
                    let format = self.parens.pop().unwrap_or(ParenFormat::Inline);
                    if let ParenFormat::Multiline { trailing_comma } = format {
                        if trailing_comma
                            && !matches!(
                                self.previous,
                                Some(SyntaxKind::LParen | SyntaxKind::Comma)
                            )
                        {
                            self.trim_trailing_spaces();
                            self.write(",");
                        }
                        self.indent = self.indent.saturating_sub(1);
                        self.newlines(1);
                    }
                    self.ordinary(token.kind, text);
                }
                SyntaxKind::LBracket => {
                    let attribute = self.previous == Some(SyntaxKind::Hash);
                    let format = if attribute {
                        BracketFormat::Attribute
                    } else if self.delimited_exceeds_target(
                        index,
                        SyntaxKind::LBracket,
                        SyntaxKind::RBracket,
                    ) {
                        BracketFormat::Multiline {
                            trailing_comma: !self.bracket_contains_top_level_for(index),
                        }
                    } else {
                        BracketFormat::Inline
                    };
                    if previous_text == Some("json") {
                        self.space();
                    }
                    self.ordinary(token.kind, text);
                    self.brackets.push(format);
                    if matches!(format, BracketFormat::Multiline { .. }) {
                        self.indent = self.indent.saturating_add(1);
                        self.newlines(1);
                    }
                }
                SyntaxKind::RBracket => {
                    let format = self.brackets.pop().unwrap_or(BracketFormat::Inline);
                    if let BracketFormat::Multiline { trailing_comma } = format {
                        if trailing_comma
                            && !matches!(
                                self.previous,
                                Some(SyntaxKind::LBracket | SyntaxKind::Comma)
                            )
                        {
                            self.trim_trailing_spaces();
                            self.write(",");
                        }
                        self.indent = self.indent.saturating_sub(1);
                        self.newlines(1);
                    }
                    self.ordinary(token.kind, text);
                    if matches!(format, BracketFormat::Attribute) && next.is_some() {
                        self.newlines(1);
                    }
                }
                SyntaxKind::Less if previous_text.is_some_and(is_generic_type_name) => {
                    self.open_generic()
                }
                SyntaxKind::Greater if self.generic_depth != 0 => self.close_generic(),
                SyntaxKind::Question if next.is_some_and(syntax_kind_starts_expression) => {
                    self.ternary_question()
                }
                SyntaxKind::Colon if self.at_ternary_colon() => self.ternary_colon(),
                _ => self.ordinary(token.kind, text),
            }
        }
        self.newlines(1);
        (!self.overflowed).then_some(self.output)
    }

    fn parentheses_exceed_target(&self, open: usize) -> bool {
        self.delimited_exceeds_target(open, SyntaxKind::LParen, SyntaxKind::RParen)
    }

    fn delimited_exceeds_target(
        &self,
        open: usize,
        opening: SyntaxKind,
        closing: SyntaxKind,
    ) -> bool {
        let mut depth = 0_usize;
        let close = self
            .tokens
            .iter()
            .enumerate()
            .skip(open)
            .find_map(|(index, token)| {
                match token.kind {
                    kind if kind == opening => depth = depth.saturating_add(1),
                    kind if kind == closing => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some(index);
                        }
                    }
                    _ => {}
                }
                None
            });
        let Some(close) = close else {
            return false;
        };
        self.projected_inline_column(open, close)
            .is_none_or(|column| column > TARGET_COLUMNS)
    }

    /// Project the canonical inline rendering through `close`.
    ///
    /// Token ranges include discarded source whitespace and are measured in
    /// bytes, so their raw width is not a formatter column count. This small
    /// projection mirrors the formatter's token spacing and counts Unicode
    /// scalar values, matching [`SourceFile::line_column`]. A construct that
    /// necessarily emits a newline has no inline projection.
    fn projected_inline_column(&self, open: usize, close: usize) -> Option<usize> {
        let current_line = self.output.rsplit('\n').next().unwrap_or_default();
        let initial_column = if self.at_line_start {
            self.indent.saturating_mul(INDENT.chars().count())
        } else {
            current_line.chars().count()
        };
        let trailing_spaces = if self.at_line_start {
            0
        } else {
            current_line
                .chars()
                .rev()
                .take_while(|character| matches!(character, ' ' | '\t' | '\r'))
                .count()
        };
        let mut line = InlineProjection {
            column: initial_column,
            trailing_spaces,
            at_line_start: self.at_line_start,
        };
        let mut previous = self.previous;
        let mut previous_was_prefix = self.previous_was_prefix;
        let mut paren_depth = self.parens.len();
        let brace_depth = self.braces.len();
        let mut bracket_depth = self.brackets.len();
        let mut generic_depth = self.generic_depth;
        let mut ternaries = self.ternaries.clone();

        for index in open..=close {
            let token = &self.tokens[index];
            let text = self.source.slice(token.range).unwrap_or_default();
            if text.contains('\r') || text.contains('\n') {
                return None;
            }
            let next = self.tokens.get(index + 1).map(|token| token.kind);
            let previous_text = index
                .checked_sub(1)
                .and_then(|previous| self.tokens.get(previous))
                .and_then(|previous| self.source.slice(previous.range));

            match token.kind {
                SyntaxKind::LineComment | SyntaxKind::LBrace | SyntaxKind::RBrace => return None,
                SyntaxKind::BlockComment => {
                    line.space();
                    line.write(text);
                    line.space();
                    previous = Some(SyntaxKind::BlockComment);
                    previous_was_prefix = false;
                }
                SyntaxKind::Semicolon => {
                    line.trim_trailing_spaces();
                    line.write(";");
                    previous = Some(SyntaxKind::Semicolon);
                    previous_was_prefix = false;
                    if paren_depth == 0 {
                        return None;
                    }
                    line.space();
                }
                SyntaxKind::Comma => {
                    line.trim_trailing_spaces();
                    line.write(",");
                    line.space();
                    previous = Some(SyntaxKind::Comma);
                    previous_was_prefix = false;
                }
                SyntaxKind::LParen => {
                    project_ordinary(
                        &mut line,
                        &mut previous,
                        &mut previous_was_prefix,
                        token.kind,
                        text,
                    );
                    paren_depth = paren_depth.saturating_add(1);
                }
                SyntaxKind::RParen => {
                    paren_depth = paren_depth.saturating_sub(1);
                    project_ordinary(
                        &mut line,
                        &mut previous,
                        &mut previous_was_prefix,
                        token.kind,
                        text,
                    );
                }
                SyntaxKind::LBracket => {
                    if previous_text == Some("json") {
                        line.space();
                    }
                    project_ordinary(
                        &mut line,
                        &mut previous,
                        &mut previous_was_prefix,
                        token.kind,
                        text,
                    );
                    bracket_depth = bracket_depth.saturating_add(1);
                }
                SyntaxKind::RBracket => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    project_ordinary(
                        &mut line,
                        &mut previous,
                        &mut previous_was_prefix,
                        token.kind,
                        text,
                    );
                }
                SyntaxKind::Less if previous_text.is_some_and(is_generic_type_name) => {
                    line.trim_trailing_spaces();
                    line.write("<");
                    generic_depth = generic_depth.saturating_add(1);
                    previous = Some(SyntaxKind::LBracket);
                    previous_was_prefix = false;
                }
                SyntaxKind::Greater if generic_depth != 0 => {
                    line.trim_trailing_spaces();
                    line.write(">");
                    generic_depth = generic_depth.saturating_sub(1);
                    previous = Some(SyntaxKind::RBracket);
                    previous_was_prefix = false;
                }
                SyntaxKind::Question if next.is_some_and(syntax_kind_starts_expression) => {
                    line.space();
                    line.write("?");
                    line.space();
                    ternaries.push((paren_depth, brace_depth, bracket_depth));
                    previous = Some(SyntaxKind::Question);
                    previous_was_prefix = false;
                }
                SyntaxKind::Colon
                    if ternaries.last().is_some_and(|snapshot| {
                        *snapshot == (paren_depth, brace_depth, bracket_depth)
                    }) =>
                {
                    line.space();
                    line.write(":");
                    line.space();
                    ternaries.pop();
                    previous = Some(SyntaxKind::Colon);
                    previous_was_prefix = false;
                }
                _ => project_ordinary(
                    &mut line,
                    &mut previous,
                    &mut previous_was_prefix,
                    token.kind,
                    text,
                ),
            }
        }

        Some(line.column)
    }

    fn bracket_contains_top_level_for(&self, open: usize) -> bool {
        let mut depth = 0_usize;
        for token in self.tokens.iter().skip(open) {
            match token.kind {
                SyntaxKind::LBracket => depth = depth.saturating_add(1),
                SyntaxKind::RBracket => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return false;
                    }
                }
                SyntaxKind::KwFor if depth == 1 => return true,
                _ => {}
            }
        }
        false
    }

    fn ordinary(&mut self, kind: SyntaxKind, text: &str) {
        let is_prefix = is_prefix_operator(kind) && prefix_position(self.previous);
        if needs_space(self.previous, self.previous_was_prefix, kind, is_prefix) {
            self.space();
        }
        self.write(text);
        self.previous = Some(kind);
        self.previous_was_prefix = is_prefix;
    }

    fn open_brace(&mut self, struct_literal: bool, json_object: bool, match_body: bool) {
        if needs_space(
            self.previous,
            self.previous_was_prefix,
            SyntaxKind::LBrace,
            false,
        ) {
            self.space();
        }
        self.write("{");
        self.braces.push(if struct_literal {
            BraceFormat::StructLiteral {
                paren_depth: self.parens.len(),
                bracket_depth: self.brackets.len(),
            }
        } else if json_object {
            BraceFormat::JsonObject {
                paren_depth: self.parens.len(),
                bracket_depth: self.brackets.len(),
            }
        } else if match_body {
            BraceFormat::Match
        } else {
            BraceFormat::Block
        });
        self.indent = self.indent.saturating_add(1);
        self.previous = Some(SyntaxKind::LBrace);
        self.previous_was_prefix = false;
        self.newlines(1);
    }

    fn close_brace(&mut self, next: Option<SyntaxKind>) {
        let format = self.braces.pop().unwrap_or(BraceFormat::Block);
        if matches!(
            format,
            BraceFormat::StructLiteral { .. } | BraceFormat::JsonObject { .. } | BraceFormat::Match
        ) && !matches!(self.previous, Some(SyntaxKind::LBrace | SyntaxKind::Comma))
        {
            self.trim_trailing_spaces();
            self.write(",");
        }
        self.indent = self.indent.saturating_sub(1);
        if self.previous == Some(SyntaxKind::LBrace) {
            self.trim_trailing_newlines();
        } else {
            self.newlines(1);
        }
        self.write("}");
        self.previous = Some(SyntaxKind::RBrace);
        self.previous_was_prefix = false;

        match next {
            Some(SyntaxKind::KwElse) => self.space(),
            Some(SyntaxKind::RBrace) => self.newlines(1),
            Some(
                SyntaxKind::Semicolon
                | SyntaxKind::Comma
                | SyntaxKind::RParen
                | SyntaxKind::RBracket
                | SyntaxKind::Dot
                | SyntaxKind::Question,
            ) => {}
            Some(_) if self.indent == 1 => self.newlines(2),
            Some(_) => self.newlines(1),
            None => self.newlines(1),
        }
    }

    fn semicolon(&mut self) {
        self.trim_trailing_spaces();
        self.write(";");
        self.previous = Some(SyntaxKind::Semicolon);
        self.previous_was_prefix = false;
        if self.parens.is_empty() {
            self.newlines(1);
        } else {
            self.space();
        }
    }

    fn open_generic(&mut self) {
        self.trim_trailing_spaces();
        self.write("<");
        self.generic_depth = self.generic_depth.saturating_add(1);
        // Generic delimiters have the same spacing behavior as brackets.
        self.previous = Some(SyntaxKind::LBracket);
        self.previous_was_prefix = false;
    }

    fn close_generic(&mut self) {
        self.trim_trailing_spaces();
        self.write(">");
        self.generic_depth = self.generic_depth.saturating_sub(1);
        self.previous = Some(SyntaxKind::RBracket);
        self.previous_was_prefix = false;
    }

    fn comma(&mut self) {
        self.trim_trailing_spaces();
        self.write(",");
        self.previous = Some(SyntaxKind::Comma);
        self.previous_was_prefix = false;
        let multiline_parentheses =
            matches!(self.parens.last(), Some(ParenFormat::Multiline { .. }));
        let named_field = self.braces.last().is_some_and(|format| {
            matches!(
                format,
                BraceFormat::StructLiteral { paren_depth, bracket_depth }
                    | BraceFormat::JsonObject { paren_depth, bracket_depth }
                    if *paren_depth == self.parens.len()
                        && *bracket_depth == self.brackets.len()
            )
        });
        let match_arm = matches!(self.braces.last(), Some(BraceFormat::Match));
        let multiline_bracket =
            matches!(self.brackets.last(), Some(BracketFormat::Multiline { .. }));
        if multiline_parentheses || multiline_bracket || named_field || match_arm {
            self.newlines(1);
        } else {
            self.space();
        }
    }

    fn line_comment(&mut self, text: &str) {
        if !self.at_line_start {
            self.space();
        }
        self.write(text.trim_end_matches(['\r', '\n']));
        self.previous = Some(SyntaxKind::LineComment);
        self.previous_was_prefix = false;
        self.newlines(1);
    }

    fn block_comment(&mut self, text: &str) {
        let standalone = self.at_line_start;
        if !self.at_line_start {
            self.space();
        }
        self.write(text);
        self.previous = Some(SyntaxKind::BlockComment);
        self.previous_was_prefix = false;
        if standalone || text.contains('\n') {
            self.newlines(1);
        } else {
            self.space();
        }
    }

    fn ternary_question(&mut self) {
        self.space();
        self.write("?");
        self.space();
        self.ternaries
            .push((self.parens.len(), self.braces.len(), self.brackets.len()));
        self.previous = Some(SyntaxKind::Question);
        self.previous_was_prefix = false;
    }

    fn at_ternary_colon(&self) -> bool {
        self.ternaries.last().is_some_and(|snapshot| {
            *snapshot == (self.parens.len(), self.braces.len(), self.brackets.len())
        })
    }

    fn ternary_colon(&mut self) {
        self.space();
        self.write(":");
        self.space();
        self.ternaries.pop();
        self.previous = Some(SyntaxKind::Colon);
        self.previous_was_prefix = false;
    }

    fn write(&mut self, text: &str) {
        if self.overflowed {
            return;
        }
        let indentation = if self.at_line_start {
            self.indent.saturating_mul(INDENT.len())
        } else {
            0
        };
        if self
            .output
            .len()
            .saturating_add(indentation)
            .saturating_add(text.len())
            > MAX_SOURCE_BYTES
        {
            self.overflowed = true;
            return;
        }
        if self.at_line_start {
            for _ in 0..self.indent {
                self.output.push_str(INDENT);
            }
            self.at_line_start = false;
        }
        self.output.push_str(text);
        self.at_line_start = self.output.ends_with('\n');
    }

    fn space(&mut self) {
        if !self.at_line_start
            && !self
                .output
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_whitespace)
        {
            if self.output.len() == MAX_SOURCE_BYTES {
                self.overflowed = true;
            } else {
                self.output.push(' ');
            }
        }
    }

    fn newlines(&mut self, count: usize) {
        self.trim_trailing_spaces();
        let existing = self
            .output
            .as_bytes()
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\n')
            .count();
        for _ in existing..count {
            if self.output.len() == MAX_SOURCE_BYTES {
                self.overflowed = true;
                break;
            }
            self.output.push('\n');
        }
        self.at_line_start = true;
    }

    fn trim_trailing_spaces(&mut self) {
        while self
            .output
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        {
            self.output.pop();
        }
    }

    fn trim_trailing_newlines(&mut self) {
        self.trim_trailing_spaces();
        while self.output.ends_with('\n') {
            self.output.pop();
        }
        self.at_line_start = self.output.is_empty();
    }
}

struct InlineProjection {
    column: usize,
    trailing_spaces: usize,
    at_line_start: bool,
}

impl InlineProjection {
    fn write(&mut self, text: &str) {
        self.column = self.column.saturating_add(text.chars().count());
        self.trailing_spaces = text
            .chars()
            .rev()
            .take_while(|character| matches!(character, ' ' | '\t' | '\r'))
            .count();
        self.at_line_start = false;
    }

    fn space(&mut self) {
        if !self.at_line_start && self.trailing_spaces == 0 {
            self.column = self.column.saturating_add(1);
            self.trailing_spaces = 1;
        }
    }

    fn trim_trailing_spaces(&mut self) {
        self.column = self.column.saturating_sub(self.trailing_spaces);
        self.trailing_spaces = 0;
    }
}

fn project_ordinary(
    line: &mut InlineProjection,
    previous: &mut Option<SyntaxKind>,
    previous_was_prefix: &mut bool,
    kind: SyntaxKind,
    text: &str,
) {
    let is_prefix = is_prefix_operator(kind) && prefix_position(*previous);
    if needs_space(*previous, *previous_was_prefix, kind, is_prefix) {
        line.space();
    }
    line.write(text);
    *previous = Some(kind);
    *previous_was_prefix = is_prefix;
}

const fn is_word(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Ident
            | SyntaxKind::Number
            | SyntaxKind::Decimal
            | SyntaxKind::String
            | SyntaxKind::Bytes
            | SyntaxKind::KwFn
            | SyntaxKind::KwLet
            | SyntaxKind::KwVar
            | SyntaxKind::KwConst
            | SyntaxKind::KwReturn
            | SyntaxKind::KwBreak
            | SyntaxKind::KwContinue
            | SyntaxKind::KwState
            | SyntaxKind::KwStruct
            | SyntaxKind::KwError
            | SyntaxKind::KwEnum
            | SyntaxKind::KwAuthorize
            | SyntaxKind::KwTrigger
            | SyntaxKind::KwIf
            | SyntaxKind::KwMatch
            | SyntaxKind::KwElse
            | SyntaxKind::KwFor
            | SyntaxKind::KwIn
            | SyntaxKind::KwSeiyaku
            | SyntaxKind::KwModule
            | SyntaxKind::KwKotoage
            | SyntaxKind::KwHajimari
            | SyntaxKind::KwKaizen
            | SyntaxKind::KwView
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
    )
}

const fn is_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Plus
            | SyntaxKind::PlusEqual
            | SyntaxKind::Minus
            | SyntaxKind::MinusEqual
            | SyntaxKind::Arrow
            | SyntaxKind::FatArrow
            | SyntaxKind::Star
            | SyntaxKind::StarEqual
            | SyntaxKind::Slash
            | SyntaxKind::SlashEqual
            | SyntaxKind::Percent
            | SyntaxKind::PercentEqual
            | SyntaxKind::Bang
            | SyntaxKind::BangEqual
            | SyntaxKind::Equal
            | SyntaxKind::EqualEqual
            | SyntaxKind::Less
            | SyntaxKind::LessEqual
            | SyntaxKind::Greater
            | SyntaxKind::GreaterEqual
            | SyntaxKind::AndAnd
            | SyntaxKind::OrOr
    )
}

const fn is_prefix_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Bang | SyntaxKind::Minus | SyntaxKind::Plus
    )
}

fn is_generic_type_name(name: &str) -> bool {
    matches!(
        name,
        "Option" | "Result" | "Secret" | "StateMap" | "List" | "QueryPage"
    )
}

const fn syntax_kind_starts_expression(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Ident
            | SyntaxKind::Number
            | SyntaxKind::Decimal
            | SyntaxKind::String
            | SyntaxKind::Bytes
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
            | SyntaxKind::KwIf
            | SyntaxKind::KwMatch
            | SyntaxKind::LParen
            | SyntaxKind::LBracket
            | SyntaxKind::Minus
            | SyntaxKind::Bang
    )
}

const fn prefix_position(previous: Option<SyntaxKind>) -> bool {
    match previous {
        None => true,
        Some(kind) => matches!(
            kind,
            SyntaxKind::LParen
                | SyntaxKind::LBracket
                | SyntaxKind::LBrace
                | SyntaxKind::Comma
                | SyntaxKind::Colon
                | SyntaxKind::Semicolon
                | SyntaxKind::Question
                | SyntaxKind::FatArrow
                | SyntaxKind::KwReturn
                | SyntaxKind::KwLet
                | SyntaxKind::KwVar
                | SyntaxKind::Equal
                | SyntaxKind::Plus
                | SyntaxKind::PlusEqual
                | SyntaxKind::Minus
                | SyntaxKind::MinusEqual
                | SyntaxKind::Star
                | SyntaxKind::StarEqual
                | SyntaxKind::Slash
                | SyntaxKind::SlashEqual
                | SyntaxKind::Percent
                | SyntaxKind::PercentEqual
                | SyntaxKind::Bang
                | SyntaxKind::BangEqual
                | SyntaxKind::EqualEqual
                | SyntaxKind::Less
                | SyntaxKind::LessEqual
                | SyntaxKind::Greater
                | SyntaxKind::GreaterEqual
                | SyntaxKind::AndAnd
                | SyntaxKind::OrOr
        ),
    }
}

const fn needs_space(
    previous: Option<SyntaxKind>,
    previous_was_prefix: bool,
    current: SyntaxKind,
    current_is_prefix: bool,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if matches!(
        current,
        SyntaxKind::RParen
            | SyntaxKind::RBracket
            | SyntaxKind::Semicolon
            | SyntaxKind::Comma
            | SyntaxKind::Colon
            | SyntaxKind::ColonColon
            | SyntaxKind::Dot
            | SyntaxKind::Question
    ) || matches!(
        previous,
        SyntaxKind::LParen
            | SyntaxKind::LBracket
            | SyntaxKind::Hash
            | SyntaxKind::ColonColon
            | SyntaxKind::Dot
    ) {
        return false;
    }
    if matches!(current, SyntaxKind::LParen) {
        return !matches!(
            previous,
            SyntaxKind::Ident
                | SyntaxKind::RParen
                | SyntaxKind::RBracket
                | SyntaxKind::KwAuthorize
                | SyntaxKind::KwHajimari
                | SyntaxKind::KwKaizen
        );
    }
    if matches!(current, SyntaxKind::LBracket) {
        return !matches!(
            previous,
            SyntaxKind::Ident | SyntaxKind::RParen | SyntaxKind::RBracket | SyntaxKind::Hash
        );
    }
    if matches!(current, SyntaxKind::LBrace) {
        return !matches!(previous, SyntaxKind::LBrace);
    }
    if matches!(previous, SyntaxKind::Colon) {
        return true;
    }
    if current_is_prefix {
        return is_word(previous);
    }
    if previous_was_prefix {
        return false;
    }
    if is_operator(current) || is_operator(previous) {
        return true;
    }
    is_word(previous) && is_word(current)
        || matches!(previous, SyntaxKind::RParen | SyntaxKind::RBracket) && is_word(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn format(source: &str) -> String {
        let source = SourceFile::new(SourceId(0), "format.ko", source);
        format_source(&source, FrontendBudget::v1()).expect("valid source")
    }

    #[test]
    fn canonicalizes_blocks_operators_and_declarations() {
        let formatted = format(
            "seiyaku Demo{state int count;hajimari(){count=0;}kotoage fn bump(int value)->int authorize(\"Write\"){var int total=count+value;if total>10{total=10;}return total;}view fn read()->int{return count;}}",
        );
        assert_eq!(
            formatted,
            concat!(
                "seiyaku Demo {\n",
                "    state int count;\n",
                "    hajimari() {\n",
                "        count = 0;\n",
                "    }\n\n",
                "    kotoage fn bump(int value) -> int authorize(\"Write\") {\n",
                "        var int total = count + value;\n",
                "        if total > 10 {\n",
                "            total = 10;\n",
                "        }\n",
                "        return total;\n",
                "    }\n\n",
                "    view fn read() -> int {\n",
                "        return count;\n",
                "    }\n",
                "}\n",
            )
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn preserves_comments_literals_and_canonical_keywords() {
        let source =
            "seiyaku Demo{/* exact */view fn text()->string{// keep me\nreturn r#\"a  b\"#;}}";
        let formatted = format(source);
        assert!(formatted.contains("/* exact */"));
        assert!(formatted.contains("// keep me"));
        assert!(formatted.contains("r#\"a  b\"#"));
        assert!(formatted.starts_with("seiyaku Demo {\n"));
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn preserves_decimal_literal_spelling_idempotently() {
        let formatted = format("seiyaku Demo{view fn value()->decimal{return 1.250_0;}}");
        assert!(formatted.contains("return 1.250_0;"));
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_named_struct_literals_with_multiline_trailing_commas() {
        let formatted = format(
            "seiyaku Demo{struct Transfer{int source,string destination,quantity amount}fn build(int source,string destination)->Transfer{return Transfer{amount:10,source,destination};}}",
        );
        assert!(
            formatted.contains(concat!(
                "return Transfer {\n",
                "            amount: 10,\n",
                "            source,\n",
                "            destination,\n",
                "        };",
            )),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn wraps_long_named_calls_at_one_hundred_columns_with_trailing_comma() {
        let formatted = format(
            "seiyaku Demo{fn target(string first,string second,string third){}fn run(){target(first:\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",second:\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",third:\"cccccccccccccccccccccccccccccccccccccccc\");}}",
        );
        assert!(formatted.contains("target(\n"), "{formatted}");
        assert!(formatted.contains("third: \"cccccccccccccccccccccccccccccccccccccccc\",\n"));
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn wraps_when_canonical_spaces_cross_the_compressed_source_boundary() {
        // With nineteen-character literals the compressed source span ends at
        // column 100 exactly. Canonical spaces after the two commas and three
        // colons take the inline rendering to column 105.
        let formatted = format(
            "seiyaku Demo{fn target(string first,string second,string third){}fn run(){target(first:\"aaaaaaaaaaaaaaaaaaa\",second:\"bbbbbbbbbbbbbbbbbbb\",third:\"ccccccccccccccccccc\");}}",
        );

        assert!(formatted.contains("target(\n"), "{formatted}");
        assert!(
            formatted
                .lines()
                .all(|line| line.chars().count() <= TARGET_COLUMNS),
            "formatter exceeded the {TARGET_COLUMNS}-column target:\n{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn unicode_literal_bytes_do_not_cause_spurious_wrapping() {
        let snow = "雪".repeat(30);
        let source = format!(
            "seiyaku Demo{{fn target(string value){{}}fn run(){{target(value:\"{snow}\");}}}}"
        );
        let formatted = format(&source);

        assert!(
            formatted.contains(&format!("target(value: \"{snow}\");")),
            "{formatted}"
        );
        assert!(!formatted.contains("target(\n"), "{formatted}");
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn branded_unicode_and_comments_remain_stable_in_multiline_calls() {
        let formatted = format(
            "誓約 Branding{始まり(){}言挙げ fn run()authorize(\"Run\"){target(first:\"雪\",// 保持\nsecond:\"月\",third:\"星\");}改善(){}}",
        );

        for spelling in [
            "誓約",
            "始まり",
            "言挙げ",
            "改善",
            "// 保持",
            "\"雪\"",
            "\"月\"",
            "\"星\"",
        ] {
            assert!(
                formatted.contains(spelling),
                "missing `{spelling}`:\n{formatted}"
            );
        }
        assert!(formatted.contains("target(\n"), "{formatted}");
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn wraps_long_list_literals_with_trailing_commas_idempotently() {
        let source = concat!(
            "seiyaku Lists{fn labels()->List<string,8>{[",
            "\"primary-label-with-a-deliberately-long-stable-spelling\",",
            "\"secondary-label-with-a-deliberately-long-stable-spelling\",",
            "\"tertiary-label-with-a-deliberately-long-stable-spelling\"",
            "]}}"
        );
        let formatted = format(source);
        assert!(formatted.contains("[\n"), "{formatted}");
        assert!(
            formatted.contains("\"tertiary-label-with-a-deliberately-long-stable-spelling\",\n"),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn preserves_list_comprehension_comments_and_literal_spelling() {
        let source = "seiyaku Lists{fn values()->List<int,4>{let List<int,4> source = [1,2];[value*10 for value in source if value>0]// stable\n}}";
        let formatted = format(source);
        assert!(
            formatted.contains("[value * 10 for value in source if value > 0]"),
            "{formatted}"
        );
        assert!(formatted.contains("// stable"));
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_native_json_with_stable_keys_literals_and_trailing_commas() {
        let formatted = format(
            r#"seiyaku JsonDemo{fn build(string label)->Json{json{owner:"alice","exact-key":label,amount:1.250_0,labels:json["primary",label]}}}"#,
        );
        assert!(
            formatted.contains(concat!(
                "json {\n",
                "            owner: \"alice\",\n",
                "            \"exact-key\": label,\n",
                "            amount: 1.250_0,\n",
                "            labels: json [\"primary\", label],\n",
                "        }",
            )),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_amount_div_round_named_arguments_within_the_target() {
        let formatted = format(
            "seiyaku Amounts{fn rounded(quantity very_long_dividend_value,quantity very_long_divisor_value)->quantity{very_long_dividend_value.div_round(divisor:very_long_divisor_value,scale:28,mode:Rounding::nearest_even)}}",
        );
        assert!(formatted.contains(".div_round(\n"), "{formatted}");
        assert!(
            formatted.contains("mode: Rounding::nearest_even,\n"),
            "{formatted}"
        );
        assert!(
            formatted.lines().all(|line| line.chars().count() <= 100),
            "formatter exceeded the 100-column target:\n{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn multiline_authorize_modifier_never_gains_a_trailing_comma() {
        let formatted = format(
            "seiyaku Demo{kotoage fn settle()authorize(\"ThisRoleNameIsDeliberatelyLongEnoughToRequireMultilineAuthorizeFormattingWithoutChangingItsGrammar\"){}}",
        );
        assert!(formatted.contains("authorize(\n"), "{formatted}");
        assert!(
            formatted.contains("WithoutChangingItsGrammar\"\n"),
            "{formatted}"
        );
        assert!(
            !formatted.contains("WithoutChangingItsGrammar\",\n"),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn distinguishes_generic_delimiters_from_comparisons() {
        let formatted = format(
            "seiyaku Demo{state StateMap<string,Option<int>> values;view fn less(int a,int b)->bool{return a<b;}}",
        );
        assert!(formatted.contains("StateMap<string, Option<int>>"));
        assert!(formatted.contains("return a < b;"));
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn formats_tail_match_and_postfix_propagation_idempotently() {
        let formatted = format(
            "seiyaku Demo{fn unwrap(Option<int> value)->int{match value{Option::some(item)=>item,Option::none=>0}}fn choose(bool flag,Option<int> value,int fallback)->int{flag?value?:fallback}}",
        );
        assert!(
            formatted.contains(concat!(
                "match value {\n",
                "            Option::some(item) => item,\n",
                "            Option::none => 0,\n",
                "        }",
            )),
            "{formatted}"
        );
        assert!(
            formatted.contains("flag ? value? : fallback"),
            "{formatted}"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn refuses_to_rewrite_invalid_sources() {
        let source = SourceFile::new(SourceId(0), "bad.ko", "seiyaku Demo { return ; }");
        let diagnostics = format_source(&source, FrontendBudget::v1())
            .expect_err("invalid source must not be formatted");
        assert!(!diagnostics.diagnostics.is_empty());
    }

    #[test]
    fn refuses_output_expansion_beyond_the_source_budget() {
        let mut text = String::from("seiyaku Demo { view fn run() {");
        for _ in 0..16 {
            text.push_str("if true {");
        }
        for _ in 0..14_000 {
            text.push_str("value = 0;");
        }
        for _ in 0..16 {
            text.push('}');
        }
        text.push_str("} }");
        assert!(text.len() < MAX_SOURCE_BYTES);

        let source = SourceFile::new(SourceId(0), "expansion.ko", text);
        let diagnostics = format_source(&source, FrontendBudget::v1())
            .expect_err("formatter expansion must remain bounded");
        assert_eq!(diagnostics.diagnostics[0].code, "K0001");
    }
}
