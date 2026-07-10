//! Canonical single-pass source parser and lossless CST construction.

use crate::{
    ast::Program,
    diagnostic::{DiagnosticBundle, DiagnosticPhase},
    lexer::lower_lexed,
    source::{FrontendBudget, SourceFile},
};

use super::{
    cst::{Event, GreenToken, SyntaxTree, build_tree},
    kind::SyntaxKind,
    lexer::lex,
};

/// Result of parsing for lossless editor and formatter consumers.
#[derive(Clone, Debug)]
pub struct ParseOutput {
    /// Concrete syntax tree, available even when diagnostics were emitted.
    pub tree: SyntaxTree,
    /// Deterministically ordered, bounded syntax diagnostics.
    pub diagnostics: DiagnosticBundle,
}

impl ParseOutput {
    /// Return whether parsing completed without diagnostics.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.diagnostics.diagnostics.is_empty()
    }
}

/// Canonical compiler parse result.
///
/// `tree` and `program` are derived from the same lossless token stream. A
/// failed parse retains the complete tree and omits the AST, preventing tools
/// from accidentally compiling a source that editor validation rejected.
#[derive(Clone, Debug)]
pub struct ProgramParseOutput {
    /// Complete lossless source tree.
    pub tree: SyntaxTree,
    /// Compiler AST parsed from the spanned tokens, present only after success.
    pub program: Option<Program>,
    /// Deterministically ordered, bounded frontend diagnostics.
    pub diagnostics: DiagnosticBundle,
    /// Significant spanned tokens lowered from the same lossless scan.
    ///
    /// Compiler phases use this private stream for exact semantic source
    /// ranges; exposing it only within the crate prevents a second scanner
    /// from drifting from the CST.
    pub(crate) tokens: Vec<crate::lexer::Token>,
}

impl ProgramParseOutput {
    /// Return whether a valid AST was produced without diagnostics.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.program.is_some() && self.diagnostics.diagnostics.is_empty()
    }
}

/// Parse one source file once, producing both its lossless CST and compiler AST.
#[must_use]
pub fn parse_program(source: &SourceFile, budget: FrontendBudget) -> ProgramParseOutput {
    let lexed = lex(source, budget);
    let lossless_tokens = lexed.tokens.clone();
    let (program, diagnostics, tokens) = match lower_lexed(source, budget, lexed) {
        Ok(tokens) => {
            let (program, diagnostics) =
                match crate::parser::validate_nesting(source, budget, &tokens) {
                    Ok(()) => match crate::parser::parse_tokens(source, &tokens) {
                        Ok(program) => (Some(program), DiagnosticBundle::new(Vec::new())),
                        Err(diagnostics) => (None, diagnostics),
                    },
                    Err(diagnostics) => (None, diagnostics),
                };
            (program, diagnostics, tokens)
        }
        Err(diagnostics) => (None, diagnostics, Vec::new()),
    };
    let tree = recovery_tree(source, &lossless_tokens, &diagnostics);
    ProgramParseOutput {
        tree,
        program,
        diagnostics,
        tokens,
    }
}

/// Parse one source file into the canonical lossless CST.
#[must_use]
pub fn parse(source: &SourceFile, budget: FrontendBudget) -> ParseOutput {
    let output = parse_program(source, budget);
    ParseOutput {
        tree: output.tree,
        diagnostics: output.diagnostics,
    }
}

fn recovery_tree(
    source: &SourceFile,
    tokens: &[GreenToken],
    diagnostics: &DiagnosticBundle,
) -> SyntaxTree {
    let mut recovery_insertions = diagnostics
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.phase == DiagnosticPhase::Parse && diagnostic.code == "K1001"
        })
        .filter_map(|diagnostic| {
            let span = diagnostic.primary_span.as_ref()?;
            tokens
                .iter()
                .find(|token| {
                    let position = source.line_column(token.range.start);
                    position.line == span.start.line && position.column == span.start.column
                })
                .map(|token| RecoveryInsertion {
                    offset: token.range.start,
                    expected: expected_kind(&diagnostic.message),
                })
        })
        .collect::<Vec<_>>();
    recovery_insertions.sort_unstable_by_key(|insertion| insertion.offset);

    let mut parser = StructuralParser::new(source, tokens, recovery_insertions);
    parser.parse_root();
    build_tree(source.id(), tokens, parser.events)
}

#[derive(Clone, Copy, Debug)]
struct RecoveryInsertion {
    offset: u32,
    expected: SyntaxKind,
}

fn expected_kind(message: &str) -> SyntaxKind {
    for (needle, kind) in [
        ("`;`", SyntaxKind::Semicolon),
        ("semicolon", SyntaxKind::Semicolon),
        ("`)`", SyntaxKind::RParen),
        ("`(`", SyntaxKind::LParen),
        ("`}`", SyntaxKind::RBrace),
        ("`{`", SyntaxKind::LBrace),
        ("`]`", SyntaxKind::RBracket),
        ("`[`", SyntaxKind::LBracket),
        ("`,`", SyntaxKind::Comma),
        ("`:`", SyntaxKind::Colon),
        ("`=`", SyntaxKind::Equal),
    ] {
        if message.contains(needle) {
            return kind;
        }
    }
    SyntaxKind::Ident
}

/// A bounded, error-tolerant structural pass over the canonical lossless
/// token stream. This pass does not decide whether source is compilable: the
/// compiler parser above remains authoritative. Its only job is to give
/// editors stable structure while retaining every token exactly once.
struct StructuralParser<'source> {
    source: &'source SourceFile,
    tokens: &'source [GreenToken],
    pos: usize,
    events: Vec<Event>,
    recovery_insertions: Vec<RecoveryInsertion>,
    next_recovery: usize,
    block_depth: usize,
}

impl<'source> StructuralParser<'source> {
    fn new(
        source: &'source SourceFile,
        tokens: &'source [GreenToken],
        recovery_insertions: Vec<RecoveryInsertion>,
    ) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            events: Vec::with_capacity(
                tokens
                    .len()
                    .saturating_add(recovery_insertions.len())
                    .saturating_mul(2),
            ),
            recovery_insertions,
            next_recovery: 0,
            block_depth: 0,
        }
    }

    fn parse_root(&mut self) {
        self.start_at(SyntaxKind::Root, 0);
        self.eat_trivia();
        if self.at(SyntaxKind::KwSeiyaku) || self.at(SyntaxKind::KwModule) {
            self.parse_source_unit();
        } else if !self.at(SyntaxKind::Eof) {
            self.parse_root_error();
        }

        loop {
            self.eat_trivia();
            if self.at(SyntaxKind::Eof) {
                break;
            }
            if self.pos >= self.tokens.len() {
                break;
            }
            self.parse_root_error();
        }
        self.eat_trivia();
        if self.raw_kind() == Some(SyntaxKind::Eof) {
            self.bump_raw();
        } else {
            self.missing(SyntaxKind::Eof);
        }
        self.emit_recovery_through(u32::MAX);
        self.finish();
    }

    fn parse_source_unit(&mut self) {
        self.start(SyntaxKind::SourceUnit);
        if self.at(SyntaxKind::KwSeiyaku) {
            self.expect(SyntaxKind::KwSeiyaku);
        } else {
            self.expect(SyntaxKind::KwModule);
        }
        self.expect(SyntaxKind::Ident);
        self.expect(SyntaxKind::LBrace);

        self.start(SyntaxKind::ItemList);
        loop {
            self.eat_trivia();
            if self.at(SyntaxKind::RBrace)
                || self.at(SyntaxKind::Eof)
                || self.pos >= self.tokens.len()
            {
                break;
            }
            let before = self.pos;
            self.parse_item();
            if self.pos == before {
                self.parse_source_error();
            }
        }
        self.finish();
        self.expect(SyntaxKind::RBrace);
        self.finish();
    }

    fn parse_item(&mut self) {
        match self.classify_item() {
            Some(SyntaxKind::FunctionItem) => self.parse_function_item(),
            Some(SyntaxKind::StructItem) => self.parse_braced_item(SyntaxKind::StructItem),
            Some(SyntaxKind::ErrorEnumItem) => {
                self.parse_braced_item(SyntaxKind::ErrorEnumItem);
            }
            Some(SyntaxKind::ConstItem) => self.parse_terminated_item(SyntaxKind::ConstItem),
            Some(SyntaxKind::StateItem) => self.parse_terminated_item(SyntaxKind::StateItem),
            Some(SyntaxKind::TriggerItem) => self.parse_braced_item(SyntaxKind::TriggerItem),
            Some(SyntaxKind::FixtureItem) => self.parse_braced_item(SyntaxKind::FixtureItem),
            Some(SyntaxKind::TestTargetItem) => {
                self.parse_braced_item(SyntaxKind::TestTargetItem);
            }
            _ => self.parse_source_error(),
        }
    }

    fn classify_item(&self) -> Option<SyntaxKind> {
        let mut cursor = self.next_significant(self.pos)?;
        while self.tokens.get(cursor)?.kind == SyntaxKind::Hash {
            cursor = self.after_attribute(cursor)?;
            cursor = self.next_significant(cursor)?;
        }
        self.classify_item_at(cursor)
    }

    fn classify_item_at(&self, cursor: usize) -> Option<SyntaxKind> {
        let token = self.tokens.get(cursor)?;
        match token.kind {
            SyntaxKind::KwFn
            | SyntaxKind::KwKotoage
            | SyntaxKind::KwView
            | SyntaxKind::KwHajimari
            | SyntaxKind::KwKaizen => Some(SyntaxKind::FunctionItem),
            SyntaxKind::KwStruct => Some(SyntaxKind::StructItem),
            SyntaxKind::KwError => Some(SyntaxKind::ErrorEnumItem),
            SyntaxKind::KwConst => Some(SyntaxKind::ConstItem),
            SyntaxKind::KwState => Some(SyntaxKind::StateItem),
            SyntaxKind::KwTrigger => Some(SyntaxKind::TriggerItem),
            SyntaxKind::Ident => match self.source.slice(token.range) {
                Some("fixture") => Some(SyntaxKind::FixtureItem),
                Some("koto_test") => Some(SyntaxKind::TestTargetItem),
                _ => None,
            },
            _ => None,
        }
    }

    fn after_attribute(&self, hash: usize) -> Option<usize> {
        let open = self.next_significant(hash.saturating_add(1))?;
        if self.tokens.get(open)?.kind != SyntaxKind::LBracket {
            return Some(open);
        }
        let mut cursor = open.saturating_add(1);
        let mut depth = 1_usize;
        let mut parenthesis_depth = 0_usize;
        while let Some(index) = self.next_significant(cursor) {
            let kind = self.tokens[index].kind;
            match kind {
                SyntaxKind::LBracket => depth = depth.saturating_add(1),
                SyntaxKind::RBracket => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(index.saturating_add(1));
                    }
                }
                SyntaxKind::LParen => parenthesis_depth = parenthesis_depth.saturating_add(1),
                SyntaxKind::RParen => parenthesis_depth = parenthesis_depth.saturating_sub(1),
                SyntaxKind::RBrace | SyntaxKind::Eof => return Some(index),
                _ if depth == 1
                    && parenthesis_depth == 0
                    && self.classify_item_at(index).is_some() =>
                {
                    return Some(index);
                }
                _ => {}
            }
            cursor = index.saturating_add(1);
        }
        None
    }

    fn parse_attributes(&mut self) {
        while self.at(SyntaxKind::Hash) {
            self.parse_attribute();
            if !self.at(SyntaxKind::Hash) {
                break;
            }
        }
    }

    fn parse_attribute(&mut self) {
        self.start(SyntaxKind::Attribute);
        self.expect(SyntaxKind::Hash);
        if !self.at(SyntaxKind::LBracket) {
            self.missing(SyntaxKind::LBracket);
            self.finish();
            return;
        }
        self.expect(SyntaxKind::LBracket);
        let mut bracket_depth = 1_usize;
        let mut parenthesis_depth = 0_usize;
        let mut saw_body = false;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                self.missing(SyntaxKind::RBracket);
                break;
            };
            match kind {
                SyntaxKind::Eof | SyntaxKind::RBrace => {
                    self.missing(SyntaxKind::RBracket);
                    break;
                }
                SyntaxKind::RBracket if bracket_depth == 1 => {
                    self.bump_raw();
                    break;
                }
                SyntaxKind::LBracket => {
                    bracket_depth = bracket_depth.saturating_add(1);
                    saw_body = true;
                    self.bump_structural_token();
                }
                SyntaxKind::RBracket => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    self.bump_structural_token();
                }
                SyntaxKind::LParen => {
                    parenthesis_depth = parenthesis_depth.saturating_add(1);
                    saw_body = true;
                    self.bump_structural_token();
                }
                SyntaxKind::RParen => {
                    parenthesis_depth = parenthesis_depth.saturating_sub(1);
                    self.bump_structural_token();
                }
                _ if saw_body
                    && bracket_depth == 1
                    && parenthesis_depth == 0
                    && self.classify_item_at(self.pos).is_some() =>
                {
                    self.missing(SyntaxKind::RBracket);
                    break;
                }
                _ => {
                    saw_body = true;
                    self.bump_structural_token();
                }
            }
        }
        self.finish();
    }

    fn parse_function_item(&mut self) {
        self.start(SyntaxKind::FunctionItem);
        self.parse_attributes();
        self.eat_trivia();
        let named = match self.raw_kind() {
            Some(SyntaxKind::KwKotoage | SyntaxKind::KwView) => {
                self.bump_raw();
                self.expect(SyntaxKind::KwFn);
                true
            }
            Some(SyntaxKind::KwFn) => {
                self.bump_raw();
                true
            }
            Some(SyntaxKind::KwHajimari | SyntaxKind::KwKaizen) => {
                self.bump_raw();
                false
            }
            _ => {
                self.parse_item_tail_as_error();
                self.finish();
                return;
            }
        };
        if named {
            self.expect(SyntaxKind::Ident);
        }
        self.parse_param_list();
        self.consume_function_header();
        if self.at(SyntaxKind::LBrace) {
            self.parse_block();
        } else {
            self.parse_missing_block();
        }
        self.finish();
    }

    fn parse_param_list(&mut self) {
        self.eat_trivia();
        self.start(SyntaxKind::ParamList);
        if !self.at(SyntaxKind::LParen) {
            self.missing(SyntaxKind::LParen);
            self.missing(SyntaxKind::RParen);
            self.finish();
            return;
        }
        self.expect(SyntaxKind::LParen);
        let mut depth = 1_usize;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                self.missing(SyntaxKind::RParen);
                break;
            };
            match kind {
                SyntaxKind::Eof | SyntaxKind::RBrace | SyntaxKind::LBrace if depth == 1 => {
                    self.missing(SyntaxKind::RParen);
                    break;
                }
                SyntaxKind::RParen => {
                    depth = depth.saturating_sub(1);
                    self.bump_raw();
                    if depth == 0 {
                        break;
                    }
                }
                SyntaxKind::LParen => {
                    depth = depth.saturating_add(1);
                    self.bump_structural_token();
                }
                _ => self.bump_structural_token(),
            }
        }
        self.finish();
    }

    fn consume_function_header(&mut self) {
        let mut parenthesis_depth = 0_usize;
        let mut bracket_depth = 0_usize;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                return;
            };
            if parenthesis_depth == 0 && bracket_depth == 0 {
                if matches!(
                    kind,
                    SyntaxKind::LBrace | SyntaxKind::RBrace | SyntaxKind::Eof
                ) || self.starts_unattributed_item_at(self.pos)
                {
                    return;
                }
            }
            match kind {
                SyntaxKind::LParen => parenthesis_depth = parenthesis_depth.saturating_add(1),
                SyntaxKind::RParen => parenthesis_depth = parenthesis_depth.saturating_sub(1),
                SyntaxKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                SyntaxKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
            self.bump_structural_token();
        }
    }

    fn parse_block(&mut self) {
        self.start(SyntaxKind::Block);
        self.expect(SyntaxKind::LBrace);
        self.start(SyntaxKind::StatementList);
        self.block_depth = self.block_depth.saturating_add(1);
        loop {
            self.eat_trivia();
            if self.at(SyntaxKind::RBrace)
                || self.at(SyntaxKind::Eof)
                || self.pos >= self.tokens.len()
            {
                break;
            }
            let before = self.pos;
            if self.block_depth > 256 {
                self.parse_statement_error();
            } else {
                self.parse_statement();
            }
            if self.pos == before {
                self.parse_statement_error();
            }
        }
        self.block_depth = self.block_depth.saturating_sub(1);
        self.finish();
        self.expect(SyntaxKind::RBrace);
        self.finish();
    }

    fn parse_missing_block(&mut self) {
        self.start(SyntaxKind::Block);
        self.missing(SyntaxKind::LBrace);
        self.start(SyntaxKind::StatementList);
        self.finish();
        self.missing(SyntaxKind::RBrace);
        self.finish();
    }

    fn parse_statement(&mut self) {
        match self.peek_kind() {
            Some(SyntaxKind::KwLet | SyntaxKind::KwVar) => {
                self.parse_simple_statement(SyntaxKind::LetStmt);
            }
            Some(SyntaxKind::KwReturn) => {
                self.parse_simple_statement(SyntaxKind::ReturnStmt);
            }
            Some(SyntaxKind::KwBreak) => {
                self.parse_simple_statement(SyntaxKind::BreakStmt);
            }
            Some(SyntaxKind::KwContinue) => {
                self.parse_simple_statement(SyntaxKind::ContinueStmt);
            }
            Some(SyntaxKind::KwIf) => self.parse_if_statement(),
            Some(SyntaxKind::KwFor) => self.parse_for_statement(),
            Some(SyntaxKind::LBrace | SyntaxKind::ErrorToken) => {
                self.parse_statement_error();
            }
            Some(_) => self.parse_simple_statement(SyntaxKind::ExprStmt),
            None => {}
        }
    }

    fn parse_simple_statement(&mut self, kind: SyntaxKind) {
        self.start(kind);
        let mut parenthesis_depth = 0_usize;
        let mut bracket_depth = 0_usize;
        let mut brace_depth = 0_usize;
        let mut saw_token = false;
        loop {
            self.eat_trivia();
            let Some(current) = self.raw_kind() else {
                self.missing(SyntaxKind::Semicolon);
                break;
            };
            let at_base = parenthesis_depth == 0 && bracket_depth == 0 && brace_depth == 0;
            if matches!(current, SyntaxKind::Eof)
                || (brace_depth == 0 && current == SyntaxKind::RBrace)
            {
                self.missing_unclosed_delimiters(parenthesis_depth, bracket_depth, brace_depth);
                self.missing(SyntaxKind::Semicolon);
                break;
            }
            if current == SyntaxKind::Semicolon && brace_depth == 0 {
                self.missing_unclosed_delimiters(parenthesis_depth, bracket_depth, brace_depth);
                self.bump_raw();
                break;
            }
            if at_base && saw_token && self.starts_statement_at(self.pos) {
                self.missing(SyntaxKind::Semicolon);
                break;
            }
            match current {
                SyntaxKind::LParen => parenthesis_depth = parenthesis_depth.saturating_add(1),
                SyntaxKind::RParen => parenthesis_depth = parenthesis_depth.saturating_sub(1),
                SyntaxKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                SyntaxKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                SyntaxKind::LBrace => brace_depth = brace_depth.saturating_add(1),
                SyntaxKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
            saw_token = true;
            self.bump_structural_token();
        }
        self.finish();
    }

    fn parse_if_statement(&mut self) {
        self.start(SyntaxKind::IfStmt);
        self.expect(SyntaxKind::KwIf);
        self.consume_control_header();
        if self.at(SyntaxKind::LBrace) {
            self.parse_block();
        } else {
            self.parse_missing_block();
        }
        if self.at(SyntaxKind::KwElse) {
            self.expect(SyntaxKind::KwElse);
            if self.at(SyntaxKind::LBrace) {
                self.parse_block();
            } else if self.at(SyntaxKind::KwIf) {
                self.parse_if_statement();
            } else {
                self.parse_missing_block();
            }
        }
        self.finish();
    }

    fn parse_for_statement(&mut self) {
        self.start(SyntaxKind::ForStmt);
        self.expect(SyntaxKind::KwFor);
        self.consume_control_header();
        if self.at(SyntaxKind::LBrace) {
            self.parse_block();
        } else {
            self.parse_missing_block();
        }
        self.finish();
    }

    fn consume_control_header(&mut self) {
        let mut parenthesis_depth = 0_usize;
        let mut bracket_depth = 0_usize;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                return;
            };
            if parenthesis_depth == 0 && bracket_depth == 0 {
                if matches!(
                    kind,
                    SyntaxKind::LBrace | SyntaxKind::RBrace | SyntaxKind::Eof
                ) || self.starts_statement_at(self.pos)
                {
                    return;
                }
            }
            match kind {
                SyntaxKind::LParen => parenthesis_depth = parenthesis_depth.saturating_add(1),
                SyntaxKind::RParen => parenthesis_depth = parenthesis_depth.saturating_sub(1),
                SyntaxKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                SyntaxKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
            self.bump_structural_token();
        }
    }

    fn parse_braced_item(&mut self, kind: SyntaxKind) {
        self.start(kind);
        self.parse_attributes();
        let mut brace_depth = 0_usize;
        let mut saw_open = false;
        loop {
            self.eat_trivia();
            let Some(current) = self.raw_kind() else {
                break;
            };
            if matches!(current, SyntaxKind::Eof) || (!saw_open && current == SyntaxKind::RBrace) {
                break;
            }
            match current {
                SyntaxKind::LBrace => {
                    brace_depth = brace_depth.saturating_add(1);
                    saw_open = true;
                }
                SyntaxKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
            self.bump_structural_token();
            if saw_open && brace_depth == 0 {
                break;
            }
        }
        if !saw_open {
            self.missing(SyntaxKind::LBrace);
        }
        if !saw_open || brace_depth != 0 {
            self.missing(SyntaxKind::RBrace);
        }
        self.finish();
    }

    fn parse_terminated_item(&mut self, kind: SyntaxKind) {
        self.start(kind);
        self.parse_attributes();
        let mut parenthesis_depth = 0_usize;
        let mut bracket_depth = 0_usize;
        loop {
            self.eat_trivia();
            let Some(current) = self.raw_kind() else {
                self.missing(SyntaxKind::Semicolon);
                break;
            };
            if parenthesis_depth == 0 && bracket_depth == 0 {
                if current == SyntaxKind::Semicolon {
                    self.bump_raw();
                    break;
                }
                if matches!(current, SyntaxKind::RBrace | SyntaxKind::Eof)
                    || self.starts_unattributed_item_at(self.pos)
                {
                    self.missing(SyntaxKind::Semicolon);
                    break;
                }
            }
            match current {
                SyntaxKind::LParen => parenthesis_depth = parenthesis_depth.saturating_add(1),
                SyntaxKind::RParen => parenthesis_depth = parenthesis_depth.saturating_sub(1),
                SyntaxKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                SyntaxKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                _ => {}
            }
            self.bump_structural_token();
        }
        self.finish();
    }

    fn parse_root_error(&mut self) {
        self.start(SyntaxKind::ErrorNode);
        let mut brace_depth = 0_usize;
        let mut consumed = false;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                break;
            };
            if kind == SyntaxKind::Eof {
                break;
            }
            if consumed
                && brace_depth == 0
                && matches!(kind, SyntaxKind::KwSeiyaku | SyntaxKind::KwModule)
            {
                break;
            }
            match kind {
                SyntaxKind::LBrace => brace_depth = brace_depth.saturating_add(1),
                SyntaxKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
            consumed = true;
            self.bump_raw();
            if brace_depth == 0 && kind == SyntaxKind::Semicolon {
                break;
            }
        }
        if !consumed && self.raw_kind() != Some(SyntaxKind::Eof) {
            self.bump_raw();
        }
        self.finish();
    }

    fn parse_source_error(&mut self) {
        self.start(SyntaxKind::ErrorNode);
        let mut delimiter_depth = 0_usize;
        let mut consumed = false;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                break;
            };
            if matches!(kind, SyntaxKind::Eof)
                || (delimiter_depth == 0 && kind == SyntaxKind::RBrace)
                || (consumed
                    && delimiter_depth == 0
                    && (kind == SyntaxKind::Hash || self.classify_item_at(self.pos).is_some()))
            {
                break;
            }
            match kind {
                SyntaxKind::LParen | SyntaxKind::LBracket | SyntaxKind::LBrace => {
                    delimiter_depth = delimiter_depth.saturating_add(1);
                }
                SyntaxKind::RParen | SyntaxKind::RBracket | SyntaxKind::RBrace => {
                    delimiter_depth = delimiter_depth.saturating_sub(1);
                }
                _ => {}
            }
            consumed = true;
            self.bump_raw();
            if delimiter_depth == 0 && kind == SyntaxKind::Semicolon {
                break;
            }
        }
        if !consumed
            && !matches!(
                self.raw_kind(),
                None | Some(SyntaxKind::Eof | SyntaxKind::RBrace)
            )
        {
            self.bump_raw();
        }
        self.finish();
    }

    fn parse_statement_error(&mut self) {
        self.start(SyntaxKind::ErrorNode);
        let mut delimiter_depth = 0_usize;
        let mut consumed = false;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                break;
            };
            if matches!(kind, SyntaxKind::Eof)
                || (delimiter_depth == 0 && kind == SyntaxKind::RBrace)
                || (consumed && delimiter_depth == 0 && self.starts_statement_at(self.pos))
            {
                break;
            }
            match kind {
                SyntaxKind::LParen | SyntaxKind::LBracket | SyntaxKind::LBrace => {
                    delimiter_depth = delimiter_depth.saturating_add(1);
                }
                SyntaxKind::RParen | SyntaxKind::RBracket | SyntaxKind::RBrace => {
                    delimiter_depth = delimiter_depth.saturating_sub(1);
                }
                _ => {}
            }
            consumed = true;
            self.bump_raw();
            if delimiter_depth == 0 && kind == SyntaxKind::Semicolon {
                break;
            }
        }
        if !consumed
            && !matches!(
                self.raw_kind(),
                None | Some(SyntaxKind::Eof | SyntaxKind::RBrace)
            )
        {
            self.bump_raw();
        }
        self.finish();
    }

    fn parse_item_tail_as_error(&mut self) {
        self.start(SyntaxKind::ErrorNode);
        let before = self.pos;
        self.parse_source_error_contents();
        if self.pos == before && self.raw_kind().is_some() {
            self.bump_raw();
        }
        self.finish();
    }

    fn parse_source_error_contents(&mut self) {
        let mut delimiter_depth = 0_usize;
        loop {
            self.eat_trivia();
            let Some(kind) = self.raw_kind() else {
                break;
            };
            if matches!(kind, SyntaxKind::Eof)
                || (delimiter_depth == 0 && kind == SyntaxKind::RBrace)
            {
                break;
            }
            match kind {
                SyntaxKind::LParen | SyntaxKind::LBracket | SyntaxKind::LBrace => {
                    delimiter_depth = delimiter_depth.saturating_add(1);
                }
                SyntaxKind::RParen | SyntaxKind::RBracket | SyntaxKind::RBrace => {
                    delimiter_depth = delimiter_depth.saturating_sub(1);
                }
                _ => {}
            }
            self.bump_raw();
            if delimiter_depth == 0 && kind == SyntaxKind::Semicolon {
                break;
            }
        }
    }

    fn starts_statement_at(&self, cursor: usize) -> bool {
        self.tokens.get(cursor).is_some_and(|token| {
            matches!(
                token.kind,
                SyntaxKind::KwLet
                    | SyntaxKind::KwVar
                    | SyntaxKind::KwReturn
                    | SyntaxKind::KwBreak
                    | SyntaxKind::KwContinue
                    | SyntaxKind::KwIf
                    | SyntaxKind::KwFor
            )
        })
    }

    fn missing_unclosed_delimiters(
        &mut self,
        parenthesis_depth: usize,
        bracket_depth: usize,
        brace_depth: usize,
    ) {
        if parenthesis_depth != 0 {
            self.missing(SyntaxKind::RParen);
        }
        if bracket_depth != 0 {
            self.missing(SyntaxKind::RBracket);
        }
        if brace_depth != 0 {
            self.missing(SyntaxKind::RBrace);
        }
    }

    fn starts_unattributed_item_at(&self, cursor: usize) -> bool {
        self.classify_item_at(cursor).is_some()
    }

    fn next_significant(&self, mut cursor: usize) -> Option<usize> {
        while self.tokens.get(cursor)?.kind.is_trivia() {
            cursor = cursor.saturating_add(1);
        }
        Some(cursor)
    }

    fn peek_kind(&self) -> Option<SyntaxKind> {
        self.next_significant(self.pos)
            .and_then(|index| self.tokens.get(index))
            .map(|token| token.kind)
    }

    fn at(&self, kind: SyntaxKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    fn raw_kind(&self) -> Option<SyntaxKind> {
        self.tokens.get(self.pos).map(|token| token.kind)
    }

    fn expect(&mut self, kind: SyntaxKind) {
        self.eat_trivia();
        if self.raw_kind() == Some(kind) {
            self.bump_raw();
        } else {
            self.missing(kind);
        }
    }

    fn eat_trivia(&mut self) {
        while self.raw_kind().is_some_and(SyntaxKind::is_trivia) {
            self.bump_raw();
        }
    }

    fn bump_structural_token(&mut self) {
        if self.raw_kind() == Some(SyntaxKind::ErrorToken) {
            self.start(SyntaxKind::ErrorNode);
            self.bump_raw();
            self.finish();
        } else {
            self.bump_raw();
        }
    }

    fn bump_raw(&mut self) {
        let Some(token) = self.tokens.get(self.pos) else {
            return;
        };
        self.emit_recovery_through(token.range.start);
        self.events.push(Event::Token(self.pos));
        self.pos = self.pos.saturating_add(1);
    }

    fn emit_recovery_through(&mut self, offset: u32) {
        while let Some(insertion) = self.recovery_insertions.get(self.next_recovery)
            && insertion.offset <= offset
        {
            self.events.push(Event::Missing {
                expected: insertion.expected,
                offset: insertion.offset,
            });
            self.next_recovery = self.next_recovery.saturating_add(1);
        }
    }

    fn missing(&mut self, expected: SyntaxKind) {
        self.events.push(Event::Missing {
            expected,
            offset: self.current_offset(),
        });
    }

    fn current_offset(&self) -> u32 {
        self.tokens
            .get(self.pos)
            .map_or(self.source.full_range().end, |token| token.range.start)
    }

    fn start(&mut self, kind: SyntaxKind) {
        self.start_at(kind, self.current_offset());
    }

    fn start_at(&mut self, kind: SyntaxKind, offset: u32) {
        self.events.push(Event::Start { kind, offset });
    }

    fn finish(&mut self) {
        self.events.push(Event::Finish {
            offset: self.current_offset(),
        });
    }
}
