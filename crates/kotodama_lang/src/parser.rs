//! Canonical grammar parser for the Kotodama compiler AST and lossless CST.
//!
//! One grammar pass consumes the significant view of the lossless lexer tape,
//! constructs the spanned AST, and records the completed syntax-node outline.
//! The CST sink later merges that outline with the original trivia-bearing
//! tape; there is no second structural parser or CST-to-token reparse.

use super::{
    ast::*,
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticFix, DiagnosticPhase, MAX_DIAGNOSTICS,
        SourcePosition, SourceSpan,
    },
    lexer::{Token, TokenKind},
    source::{FrontendBudget, SourceFile, SourceId, SourceRange, TextRange},
    spanned_ast::{
        AstFacts, AstNodeKind, BindingFact, BindingFactKind, CallFact, DeclarationFact,
        DeclarationKind, NodeId, SpannedProgram, TypeUseFact,
    },
    syntax::{
        SyntaxKind,
        cst::{MissingSyntax, SyntaxOutline, SyntaxOutlineBuilder, SyntaxOutlineCheckpoint},
    },
};
use iroha_primitives::{bigint::BigInt, numeric_abi::IntValueV1};

#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    /// Stable machine-readable diagnostic code, independent of message text.
    pub code: &'static str,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub snippet: String,
    /// Exact half-open UTF-8 range of the unexpected token.
    pub range: TextRange,
    /// Optional machine-applicable replacement for the diagnosed source range.
    pub fix: Option<String>,
    /// Exact zero-width CST token expected at this failure, when recovery can
    /// insert one without guessing from diagnostic prose.
    pub expected: Option<SyntaxKind>,
    /// Syntax-outline node that owned `expected` at the failure boundary.
    pub(crate) expected_owner: Option<usize>,
}

type ParseResult<T> = Result<T, Box<ParseError>>;
type ForEachMapBinding = (NodeId, String, Option<String>, Expr);

fn integer_digits(spelling: &str) -> (&str, u32) {
    if let Some(digits) = spelling
        .strip_prefix("0x")
        .or_else(|| spelling.strip_prefix("0X"))
    {
        (digits, 16)
    } else if let Some(digits) = spelling
        .strip_prefix("0b")
        .or_else(|| spelling.strip_prefix("0B"))
    {
        (digits, 2)
    } else {
        (spelling, 10)
    }
}

fn parse_integer_value(spelling: &str, negative: bool) -> Result<BigInt, ()> {
    let (digits, radix_value) = integer_digits(spelling);
    let radix = BigInt::from(radix_value);
    let mut value = BigInt::zero();
    for character in digits.chars().filter(|character| *character != '_') {
        let digit = character.to_digit(radix_value).ok_or(())?;
        value = value.checked_mul(&radix).map_err(|_| ())?;
        value = if negative {
            value.checked_sub(&BigInt::from(digit)).map_err(|_| ())?
        } else {
            value.checked_add(&BigInt::from(digit)).map_err(|_| ())?
        };
    }
    IntValueV1::try_new(value.clone()).map_err(|_| ())?;
    Ok(value)
}

fn parse_bounded_unsigned(spelling: &str, maximum: u64) -> Result<u64, ()> {
    let (digits, radix) = integer_digits(spelling);
    let compact = digits
        .chars()
        .filter(|character| *character != '_')
        .collect::<String>();
    let value = u64::from_str_radix(&compact, radix).map_err(|_| ())?;
    (value <= maximum).then_some(value).ok_or(())
}

fn bigint_literal_expr(value: BigInt) -> Expr {
    Expr::IntLiteral(value)
}

fn retired_numeric_type_replacement(name: &str) -> Option<Option<&'static str>> {
    match name {
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" | "num" | "Int" | "Integer" => Some(Some("int")),
        "float" | "f32" | "f64" | "Decimal" | "Fixed" | "FixedPoint" => Some(Some("decimal")),
        "Amount" | "amount" | "money" | "Quantity" => Some(Some("quantity")),
        "number" => Some(None),
        _ => None,
    }
}

fn expected_syntax_kind(kind: &TokenKind) -> Option<SyntaxKind> {
    Some(match kind {
        TokenKind::Fn => SyntaxKind::KwFn,
        TokenKind::Let => SyntaxKind::KwLet,
        TokenKind::Var => SyntaxKind::KwVar,
        TokenKind::Const => SyntaxKind::KwConst,
        TokenKind::Return => SyntaxKind::KwReturn,
        TokenKind::Break => SyntaxKind::KwBreak,
        TokenKind::Continue => SyntaxKind::KwContinue,
        TokenKind::State => SyntaxKind::KwState,
        TokenKind::Struct => SyntaxKind::KwStruct,
        TokenKind::Error => SyntaxKind::KwError,
        TokenKind::Enum => SyntaxKind::KwEnum,
        TokenKind::Authorize => SyntaxKind::KwAuthorize,
        TokenKind::Trigger => SyntaxKind::KwTrigger,
        TokenKind::If => SyntaxKind::KwIf,
        TokenKind::Match => SyntaxKind::KwMatch,
        TokenKind::Else => SyntaxKind::KwElse,
        TokenKind::For => SyntaxKind::KwFor,
        TokenKind::In => SyntaxKind::KwIn,
        TokenKind::Seiyaku => SyntaxKind::KwSeiyaku,
        TokenKind::Module => SyntaxKind::KwModule,
        TokenKind::Kotoage => SyntaxKind::KwKotoage,
        TokenKind::Hajimari => SyntaxKind::KwHajimari,
        TokenKind::Kaizen => SyntaxKind::KwKaizen,
        TokenKind::View => SyntaxKind::KwView,
        TokenKind::True => SyntaxKind::KwTrue,
        TokenKind::False => SyntaxKind::KwFalse,
        TokenKind::Ident(_) => SyntaxKind::Ident,
        TokenKind::Number(_) => SyntaxKind::Number,
        TokenKind::DecimalLiteral(_) => SyntaxKind::Decimal,
        TokenKind::String(_) => SyntaxKind::String,
        TokenKind::Bytes(_) => SyntaxKind::Bytes,
        TokenKind::Plus => SyntaxKind::Plus,
        TokenKind::PlusEqual => SyntaxKind::PlusEqual,
        TokenKind::Minus => SyntaxKind::Minus,
        TokenKind::MinusEqual => SyntaxKind::MinusEqual,
        TokenKind::Arrow => SyntaxKind::Arrow,
        TokenKind::FatArrow => SyntaxKind::FatArrow,
        TokenKind::Star => SyntaxKind::Star,
        TokenKind::StarEqual => SyntaxKind::StarEqual,
        TokenKind::Slash => SyntaxKind::Slash,
        TokenKind::SlashEqual => SyntaxKind::SlashEqual,
        TokenKind::Percent => SyntaxKind::Percent,
        TokenKind::PercentEqual => SyntaxKind::PercentEqual,
        TokenKind::Bang => SyntaxKind::Bang,
        TokenKind::BangEqual => SyntaxKind::BangEqual,
        TokenKind::Equal => SyntaxKind::Equal,
        TokenKind::EqualEqual => SyntaxKind::EqualEqual,
        TokenKind::Less => SyntaxKind::Less,
        TokenKind::LessEqual => SyntaxKind::LessEqual,
        TokenKind::Greater => SyntaxKind::Greater,
        TokenKind::GreaterEqual => SyntaxKind::GreaterEqual,
        TokenKind::AndAnd => SyntaxKind::AndAnd,
        TokenKind::OrOr => SyntaxKind::OrOr,
        TokenKind::LParen => SyntaxKind::LParen,
        TokenKind::RParen => SyntaxKind::RParen,
        TokenKind::LBrace => SyntaxKind::LBrace,
        TokenKind::RBrace => SyntaxKind::RBrace,
        TokenKind::LBracket => SyntaxKind::LBracket,
        TokenKind::RBracket => SyntaxKind::RBracket,
        TokenKind::Semicolon => SyntaxKind::Semicolon,
        TokenKind::Comma => SyntaxKind::Comma,
        TokenKind::Colon => SyntaxKind::Colon,
        TokenKind::ColonColon => SyntaxKind::ColonColon,
        TokenKind::Dot => SyntaxKind::Dot,
        TokenKind::Question => SyntaxKind::Question,
        TokenKind::Hash => SyntaxKind::Hash,
        TokenKind::EOF => SyntaxKind::Eof,
    })
}

fn map_iteration_has_explicit_bound(expr: &Expr) -> bool {
    matches!(
        expr.kind(),
        Expr::Call {
            name,
            args,
            implicit_receiver: true,
            ..
        }
            if (name == "take" && args.len() == 2)
                || (name == "range" && args.len() == 3)
    )
}

struct ParsedCallArguments {
    args: Vec<Expr>,
    argument_names: Option<Vec<String>>,
    ranges: Vec<TextRange>,
}

enum ParsedBlockElement {
    Statement(Statement),
    Tail(Expr),
}

fn block_element_syntax_kind(element: &ParsedBlockElement) -> SyntaxKind {
    match element {
        ParsedBlockElement::Tail(_) => SyntaxKind::TailExpr,
        ParsedBlockElement::Statement(statement) => match statement.kind() {
            Statement::Let { .. } => SyntaxKind::LetStmt,
            Statement::Return(_) => SyntaxKind::ReturnStmt,
            Statement::Break => SyntaxKind::BreakStmt,
            Statement::Continue => SyntaxKind::ContinueStmt,
            Statement::If { .. } | Statement::IfLet { .. } => SyntaxKind::IfStmt,
            Statement::For { .. } | Statement::ForEachMap { .. } => SyntaxKind::ForStmt,
            _ => SyntaxKind::ExprStmt,
        },
    }
}

#[derive(Default)]
struct FunctionAttributes {
    reads: Vec<String>,
    writes: Vec<String>,
    is_test: bool,
    test_fixture: Option<String>,
}

impl FunctionAttributes {
    fn is_empty(&self) -> bool {
        self.reads.is_empty()
            && self.writes.is_empty()
            && !self.is_test
            && self.test_fixture.is_none()
    }
}

/// Parse a KOTODAMA source string into a [`Program`].
pub fn parse(src: &str) -> Result<Program, String> {
    if src.len() > crate::source::MAX_SOURCE_BYTES {
        return Err(format!(
            "K0001: source contains {} bytes and exceeds the {}-byte Kotodama V1 limit",
            src.len(),
            crate::source::MAX_SOURCE_BYTES
        ));
    }
    let source = SourceFile::new(SourceId(0), "<source>", src);
    parse_source(&source, FrontendBudget::v1()).map_err(|bundle| bundle.render_human())
}

/// Parse one named source file through the canonical V1 token stream.
///
/// The lossless lexer runs exactly once. Its significant tokens feed this AST
/// parser directly, so compilation cannot accept a spelling or token boundary
/// that formatter and CST tooling reject (or vice versa).
pub fn parse_source(
    source: &SourceFile,
    budget: FrontendBudget,
) -> Result<Program, DiagnosticBundle> {
    let output = crate::syntax::parse_program(source, budget);
    output.program.ok_or(output.diagnostics)
}

/// Parse once and retain the exact significant token stream for later
/// resolution/type diagnostics.
pub(crate) fn parse_source_spanned(
    source: &SourceFile,
    budget: FrontendBudget,
) -> Result<(SpannedProgram, Vec<Token>), DiagnosticBundle> {
    crate::syntax::parser::parse_spanned_program(source, budget)
}

pub(crate) struct GrammarParseOutput {
    pub(crate) spanned: Option<SpannedProgram>,
    pub(crate) diagnostics: DiagnosticBundle,
    pub(crate) outline: SyntaxOutline,
    pub(crate) missing: Vec<MissingSyntax>,
}

/// Parse the canonical significant token view once while recording the CST
/// structure chosen by those exact grammar decisions.
pub(crate) fn parse_with_syntax(source: &SourceFile, tokens: &[Token]) -> GrammarParseOutput {
    #[cfg(test)]
    CANONICAL_GRAMMAR_PARSES.with(|count| count.set(count.get().saturating_add(1)));

    let mut parser = CstAstLowerer::new(tokens, source, true);
    let parsed = parser.parse_program();
    let mut errors = std::mem::take(&mut parser.errors);
    if let Err(error) = parsed.as_ref() {
        errors.push(error.as_ref().clone());
    }
    append_forbidden_source_identifier_errors(&parser, tokens, &mut errors);
    errors.sort_by(|left, right| {
        left.range
            .cmp(&right.range)
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    parser.syntax.finish_open_nodes(source.text().len() as u32);
    let outline = std::mem::take(&mut parser.syntax).into_outline();

    let mut missing = errors
        .iter()
        .filter_map(|error| {
            error.expected.map(|expected| MissingSyntax {
                offset: error.range.start,
                expected,
                owner: error.expected_owner,
            })
        })
        .collect::<Vec<_>>();
    missing.sort_unstable_by_key(|missing| {
        (
            missing.offset,
            missing.expected as usize,
            missing.owner.unwrap_or(usize::MAX),
        )
    });
    missing.dedup_by(|right, left| right.offset == left.offset && right.expected == left.expected);
    let diagnostics = parse_diagnostic_bundle(source, errors);
    let spanned = match parsed {
        Ok(program) if diagnostics.diagnostics.is_empty() => Some(SpannedProgram {
            program,
            facts: parser.facts,
        }),
        Ok(_) | Err(_) => None,
    };
    GrammarParseOutput {
        spanned,
        diagnostics,
        outline,
        missing,
    }
}

fn append_forbidden_source_identifier_errors(
    parser: &CstAstLowerer<'_>,
    tokens: &[Token],
    errors: &mut Vec<ParseError>,
) {
    let retired_type_ranges = errors
        .iter()
        .filter(|error| error.code == "E_RETIRED_NUMERIC_TYPE")
        .map(|error| error.range)
        .collect::<std::collections::BTreeSet<_>>();
    for token in tokens {
        let TokenKind::Ident(name) = &token.kind else {
            continue;
        };
        if !crate::semantic::V1_FORBIDDEN_SOURCE_IDENTIFIERS.contains(&name.as_str())
            || retired_type_ranges.contains(&token.range)
        {
            continue;
        }
        errors.push(*parser.coded_error(
            token.clone(),
            "E_FORBIDDEN_SOURCE_IDENTIFIER",
            format!(
                "source identifier `{name}` is not part of Kotodama V1; choose a different identifier (lowercase `amount` remains available)"
            ),
        ));
    }
}

#[cfg(test)]
thread_local! {
    static CANONICAL_GRAMMAR_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_direct_cst_lowering_count() {
    CANONICAL_GRAMMAR_PARSES.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn direct_cst_lowering_count() -> usize {
    CANONICAL_GRAMMAR_PARSES.with(std::cell::Cell::get)
}

pub(crate) fn validate_nesting(
    source: &SourceFile,
    budget: FrontendBudget,
    tokens: &[Token],
) -> Result<(), DiagnosticBundle> {
    let mut delimiter_depth = 0_usize;
    let mut angle_depth = 0_usize;
    let mut prefix_depth = 0_usize;
    let mut conditional_count = 0_usize;
    for token in tokens {
        match &token.kind {
            TokenKind::LBrace => {
                delimiter_depth = delimiter_depth.saturating_add(1);
                angle_depth = 0;
                conditional_count = 0;
                prefix_depth = 0;
            }
            TokenKind::LParen | TokenKind::LBracket => {
                delimiter_depth = delimiter_depth.saturating_add(1);
                prefix_depth = 0;
            }
            TokenKind::RBrace => {
                delimiter_depth = delimiter_depth.saturating_sub(1);
                angle_depth = 0;
                conditional_count = 0;
                prefix_depth = 0;
            }
            TokenKind::RParen | TokenKind::RBracket => {
                delimiter_depth = delimiter_depth.saturating_sub(1);
                prefix_depth = 0;
            }
            TokenKind::Less => {
                angle_depth = angle_depth.saturating_add(1);
                prefix_depth = 0;
            }
            TokenKind::Greater => {
                angle_depth = angle_depth.saturating_sub(1);
                prefix_depth = 0;
            }
            TokenKind::Bang | TokenKind::Minus | TokenKind::Plus => {
                prefix_depth = prefix_depth.saturating_add(1);
            }
            TokenKind::Question => conditional_count = conditional_count.saturating_add(1),
            TokenKind::Semicolon => {
                angle_depth = 0;
                conditional_count = 0;
                prefix_depth = 0;
            }
            _ => prefix_depth = 0,
        }
        if delimiter_depth
            .saturating_add(angle_depth)
            .saturating_add(prefix_depth)
            .saturating_add(conditional_count)
            > budget.max_nesting()
        {
            let start = source.line_column(token.range.start);
            let end = source.line_column(token.range.end);
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "K0003",
                DiagnosticPhase::Parse,
                format!(
                    "source exceeds the {}-level syntactic nesting limit",
                    budget.max_nesting()
                ),
                Some(SourceSpan {
                    package_identity: source.package_identity().map(str::to_owned),
                    source: Some(source.name().to_owned()),
                    start: SourcePosition {
                        line: start.line,
                        column: start.column,
                    },
                    end: SourcePosition {
                        line: end.line,
                        column: end.column,
                    },
                    byte_range: Some(token.range),
                }),
            )));
        }
    }
    Ok(())
}

fn parse_diagnostic_bundle(source: &SourceFile, mut errors: Vec<ParseError>) -> DiagnosticBundle {
    let omitted = errors.len().saturating_sub(MAX_DIAGNOSTICS - 1);
    errors.truncate(MAX_DIAGNOSTICS - 1);
    let mut diagnostics = errors
        .into_iter()
        .map(|error| {
            let start = source.line_column(error.range.start);
            let end = source.line_column(error.range.end);
            let mut diagnostic = Diagnostic::error(
                error.code,
                DiagnosticPhase::Parse,
                error.message,
                Some(SourceSpan {
                    package_identity: source.package_identity().map(str::to_owned),
                    source: Some(source.name().to_owned()),
                    start: SourcePosition {
                        line: start.line,
                        column: start.column,
                    },
                    end: SourcePosition {
                        line: end.line,
                        column: end.column,
                    },
                    byte_range: Some(error.range),
                }),
            );
            if !error.snippet.is_empty() {
                diagnostic.notes.push(error.snippet);
            }
            if let Some(replacement) = error.fix {
                diagnostic.fix = diagnostic
                    .primary_span
                    .clone()
                    .map(|span| DiagnosticFix { span, replacement });
            }
            diagnostic
        })
        .collect::<Vec<_>>();
    if omitted != 0 {
        diagnostics.push(Diagnostic::error(
            "K0004",
            DiagnosticPhase::Parse,
            format!("diagnostic limit reached; {omitted} additional syntax error(s) were omitted"),
            None,
        ));
    }
    DiagnosticBundle::new(diagnostics)
}

/// Wrap a unit-test fragment in a canonical `seiyaku`/`誓約` container before parsing.
#[cfg(test)]
pub(crate) fn parse_test_fragment(src: &str) -> Result<Program, String> {
    let trimmed = src.trim_start();
    if trimmed.starts_with("seiyaku ")
        || trimmed.starts_with("誓約 ")
        || trimmed.starts_with("module ")
    {
        parse(src)
    } else {
        parse(&format!("seiyaku TestContract {{\n{src}\n}}"))
    }
}

struct CstAstLowerer<'a> {
    tokens: &'a [Token],
    pos: usize,
    source: &'a str,
    facts: AstFacts,
    current_function: Option<NodeId>,
    test_target: Option<TestTargetDecl>,
    fixtures: Vec<FixtureDecl>,
    recover: bool,
    errors: Vec<ParseError>,
    allow_struct_literals: bool,
    declared_function_parameters: std::collections::BTreeMap<String, Option<Vec<String>>>,
    syntax: SyntaxOutlineBuilder,
}

impl<'a> CstAstLowerer<'a> {
    fn new(tokens: &'a [Token], source: &'a SourceFile, recover: bool) -> Self {
        let mut syntax = SyntaxOutlineBuilder::default();
        syntax.start(SyntaxKind::Root, 0);
        Self {
            tokens,
            pos: 0,
            source: source.text(),
            facts: AstFacts::new(source.id()),
            current_function: None,
            test_target: None,
            fixtures: Vec::new(),
            recover,
            errors: Vec::new(),
            allow_struct_literals: true,
            declared_function_parameters: std::collections::BTreeMap::new(),
            syntax,
        }
    }

    fn current_start(&self) -> u32 {
        self.tokens
            .get(self.pos)
            .or_else(|| self.tokens.last())
            .map_or(0, |token| token.range.start)
    }

    fn previous_end(&self, fallback: u32) -> u32 {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map_or(fallback, |token| token.range.end)
    }

    fn begin_node(&mut self, kind: AstNodeKind, start: u32) -> NodeId {
        self.facts
            .source_map
            .begin_owned(kind, start, self.current_function)
    }

    fn finish_node(&mut self, node: NodeId) {
        let start = self
            .facts
            .source_map
            .node(node)
            .map_or(0, |entry| entry.range.start);
        let end = self.previous_end(start);
        self.facts.source_map.finish(node, end);
    }

    fn syntax_start(&mut self, kind: SyntaxKind, start: u32) -> usize {
        self.syntax.start(kind, start)
    }

    fn syntax_finish(&mut self, node: usize, fallback: u32) {
        self.syntax.finish(node, self.previous_end(fallback));
    }

    fn syntax_finish_at(&mut self, node: usize, end: u32) {
        self.syntax.finish(node, end);
    }

    fn syntax_set_kind(&mut self, node: usize, kind: SyntaxKind) {
        self.syntax.set_kind(node, kind);
    }

    fn syntax_checkpoint(&self) -> SyntaxOutlineCheckpoint {
        self.syntax.checkpoint()
    }

    fn syntax_rollback(&mut self, checkpoint: SyntaxOutlineCheckpoint) {
        self.syntax.rollback(checkpoint);
    }

    fn with_syntax<T>(
        &mut self,
        kind: SyntaxKind,
        start: u32,
        parse: impl FnOnce(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<T> {
        let node = self.syntax_start(kind, start);
        let result = parse(self);
        self.syntax_finish(node, start);
        result
    }

    fn syntax_item_kind(&self, start: usize) -> SyntaxKind {
        let mut cursor = start;
        while matches!(
            self.tokens.get(cursor).map(|token| &token.kind),
            Some(TokenKind::Hash)
        ) {
            let mut bracket_depth = 0_usize;
            while let Some(token) = self.tokens.get(cursor) {
                if bracket_depth != 0
                    && matches!(
                        token.kind,
                        TokenKind::Fn
                            | TokenKind::Kotoage
                            | TokenKind::View
                            | TokenKind::Hajimari
                            | TokenKind::Kaizen
                            | TokenKind::Struct
                            | TokenKind::Error
                            | TokenKind::Const
                            | TokenKind::State
                            | TokenKind::Trigger
                    )
                {
                    break;
                }
                cursor = cursor.saturating_add(1);
                match token.kind {
                    TokenKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                    TokenKind::RBracket => {
                        bracket_depth = bracket_depth.saturating_sub(1);
                        if bracket_depth == 0 {
                            break;
                        }
                    }
                    TokenKind::EOF | TokenKind::RBrace => break,
                    _ => {}
                }
            }
        }
        match self.tokens.get(cursor).map(|token| &token.kind) {
            Some(
                TokenKind::Fn
                | TokenKind::Kotoage
                | TokenKind::View
                | TokenKind::Hajimari
                | TokenKind::Kaizen,
            ) => SyntaxKind::FunctionItem,
            Some(TokenKind::Struct) => SyntaxKind::StructItem,
            Some(TokenKind::Error) => SyntaxKind::ErrorEnumItem,
            Some(TokenKind::Const) => SyntaxKind::ConstItem,
            Some(TokenKind::State) => SyntaxKind::StateItem,
            Some(TokenKind::Trigger) => SyntaxKind::TriggerItem,
            Some(TokenKind::Ident(name)) if name == "fixture" => SyntaxKind::FixtureItem,
            Some(TokenKind::Ident(name)) if name == "koto_test" => SyntaxKind::TestTargetItem,
            _ => SyntaxKind::ErrorNode,
        }
    }

    fn syntax_statement_kind(&self, start: usize) -> SyntaxKind {
        match self.tokens.get(start).map(|token| &token.kind) {
            Some(TokenKind::Let | TokenKind::Var) => SyntaxKind::LetStmt,
            Some(TokenKind::Return) => SyntaxKind::ReturnStmt,
            Some(TokenKind::Break) => SyntaxKind::BreakStmt,
            Some(TokenKind::Continue) => SyntaxKind::ContinueStmt,
            Some(TokenKind::If) => SyntaxKind::IfStmt,
            Some(TokenKind::For) => SyntaxKind::ForStmt,
            _ => SyntaxKind::ExprStmt,
        }
    }

    fn record_declaration(
        &mut self,
        node: NodeId,
        name: String,
        name_range: TextRange,
        kind: DeclarationKind,
        owner: Option<NodeId>,
    ) {
        let name_node = self
            .facts
            .source_map
            .allocate_owned(AstNodeKind::Name, name_range, owner);
        self.facts.declarations.push(DeclarationFact {
            node,
            name_node,
            owner,
            name,
            kind,
        });
    }

    fn record_type_use(&mut self, name: String, range: TextRange) {
        let node =
            self.facts
                .source_map
                .allocate_owned(AstNodeKind::Type, range, self.current_function);
        self.facts.type_uses.push(TypeUseFact {
            node,
            owner: self.current_function,
            name,
        });
    }

    fn source_expression(&mut self, kind: AstNodeKind, range: TextRange, expression: Expr) -> Expr {
        let node = self
            .facts
            .source_map
            .allocate_owned(kind, range, self.current_function);
        Expr::Source {
            node,
            source: SourceRange::new(self.facts.source_map.source(), range),
            expression: Box::new(expression),
        }
    }

    fn source_expression_from(&mut self, start: u32, expression: Expr) -> Expr {
        let range = TextRange::new(start, self.previous_end(start));
        if expression
            .source()
            .is_some_and(|source| source.range == range)
        {
            expression
        } else {
            self.source_expression(AstNodeKind::Expression, range, expression)
        }
    }

    fn source_statement(&mut self, range: TextRange, statement: Statement) -> Statement {
        let node = self.facts.source_map.allocate_owned(
            AstNodeKind::Statement,
            range,
            self.current_function,
        );
        Statement::Source {
            node,
            source: SourceRange::new(self.facts.source_map.source(), range),
            statement: Box::new(statement),
        }
    }

    fn finish_owned_expression(
        &mut self,
        owner: NodeId,
        kind: AstNodeKind,
        range: TextRange,
        expression: Expr,
    ) -> Expr {
        self.facts.source_map.set_kind(owner, kind);
        self.facts.source_map.finish(owner, range.end);
        Expr::Source {
            node: owner,
            source: SourceRange::new(self.facts.source_map.source(), range),
            expression: Box::new(expression),
        }
    }

    fn finish_owned_statement(
        &mut self,
        owner: NodeId,
        range: TextRange,
        statement: Statement,
    ) -> Statement {
        self.facts
            .source_map
            .set_kind(owner, AstNodeKind::Statement);
        self.facts.source_map.finish(owner, range.end);
        Statement::Source {
            node: owner,
            source: SourceRange::new(self.facts.source_map.source(), range),
            statement: Box::new(statement),
        }
    }

    fn record_binding(
        &mut self,
        owner: NodeId,
        ordinal: usize,
        name: String,
        range: TextRange,
        kind: BindingFactKind,
    ) {
        let ordinal = u16::try_from(ordinal).expect("one node's binding budget fits u16");
        let name_node =
            self.facts
                .source_map
                .allocate_owned(AstNodeKind::Name, range, self.current_function);
        self.facts.bindings.push(BindingFact {
            owner,
            ordinal,
            name_node,
            name,
            kind,
        });
    }

    fn source_type(&mut self, range: TextRange, ty: TypeExpr) -> TypeExpr {
        let node =
            self.facts
                .source_map
                .allocate_owned(AstNodeKind::Type, range, self.current_function);
        TypeExpr::Source {
            node,
            source: SourceRange::new(self.facts.source_map.source(), range),
            ty: Box::new(ty),
        }
    }

    fn record_call(
        &mut self,
        name: String,
        name_range: TextRange,
        call_range: TextRange,
        implicit_receiver: bool,
    ) -> (NodeId, SourceRange) {
        let node = self.facts.source_map.allocate_owned(
            AstNodeKind::Call,
            call_range,
            self.current_function,
        );
        let name_node = self.facts.source_map.allocate_owned(
            AstNodeKind::Name,
            name_range,
            self.current_function,
        );
        self.facts.calls.push(CallFact {
            node,
            name_node,
            owner: self.current_function,
            name,
            implicit_receiver,
        });
        (
            node,
            SourceRange::new(self.facts.source_map.source(), call_range),
        )
    }

    fn parse_program(&mut self) -> ParseResult<Program> {
        let kind = if self.peek(TokenKind::Seiyaku) {
            SourceUnitKind::Seiyaku
        } else if self.peek(TokenKind::Module) {
            SourceUnitKind::Module
        } else {
            let token = self.bump();
            return Err(self.error(
                token,
                "exactly one `seiyaku Name { ... }`/`誓約 Name { ... }` or `module Name { ... }` source unit",
            ));
        };
        let (unit, items) = self.parse_source_unit(kind)?;
        if !self.peek(TokenKind::EOF) {
            let token = self.bump();
            return Err(self.error(
                token,
                "exactly one seiyaku or module is allowed per source file",
            ));
        }
        Ok(Program {
            unit,
            items,
            test_target: self.test_target.clone(),
            fixtures: self.fixtures.clone(),
        })
    }

    fn parse_source_unit(&mut self, kind: SourceUnitKind) -> ParseResult<(SourceUnit, Vec<Item>)> {
        let start = self.current_start();
        let syntax_unit = self.syntax_start(SyntaxKind::SourceUnit, start);
        let node = self.begin_node(AstNodeKind::SourceUnit, start);
        self.bump(); // `seiyaku`/`誓約` or `module`
        let (name, name_token) = self.expect_ident_token()?;
        self.record_declaration(
            node,
            name.clone(),
            name_token.range,
            DeclarationKind::SourceUnit,
            None,
        );
        self.expect(TokenKind::LBrace)?;
        let syntax_items = self.syntax_start(SyntaxKind::ItemList, self.previous_end(start));
        let mut items = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let item_start = self.pos;
            let Some(item_token) = self.tokens.get(item_start) else {
                break;
            };
            let declaration_start = item_token.range.start;
            let item_kind = self.syntax_item_kind(item_start);
            let syntax_item = self.syntax_start(item_kind, declaration_start);
            let result = (|| -> ParseResult<()> {
                let attrs = self.parse_function_attributes()?;
                if self.peek(TokenKind::Struct) {
                    if !attrs.is_empty() {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "function attributes must precede a function",
                        ));
                    }
                    items.push(self.parse_struct_def()?);
                } else if self.peek(TokenKind::Error) {
                    if !attrs.is_empty() {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "function attributes must precede a function",
                        ));
                    }
                    items.push(self.parse_error_enum_def()?);
                } else if self.peek(TokenKind::Const) {
                    if !attrs.is_empty() {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "function attributes must precede a function",
                        ));
                    }
                    items.push(self.parse_const_decl()?);
                } else if self.peek(TokenKind::State) {
                    if !attrs.is_empty() {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "function attributes must precede a function",
                        ));
                    }
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare durable state",
                        ));
                    }
                    items.push(self.parse_state_decl()?);
                } else if self.peek(TokenKind::Trigger) {
                    if !attrs.is_empty() {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "function attributes must precede a function",
                        ));
                    }
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare triggers",
                        ));
                    }
                    items.push(self.parse_trigger_decl()?);
                } else if self.peek(TokenKind::Fn) {
                    self.bump();
                    items.push(self.parse_fn_loose(
                        None,
                        FunctionModifiers {
                            kind: FunctionKind::Private,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                        declaration_start,
                    )?);
                } else if self.peek(TokenKind::Kotoage) && self.peek_n(1, TokenKind::Fn) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare `kotoage`/`言挙げ` functions",
                        ));
                    }
                    self.bump(); // kotoage / 言挙げ
                    self.bump(); // fn
                    items.push(self.parse_fn_loose(
                        None,
                        FunctionModifiers {
                            kind: FunctionKind::Kotoage,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                        declaration_start,
                    )?);
                } else if self.peek(TokenKind::View) && self.peek_n(1, TokenKind::Fn) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare `view fn` functions",
                        ));
                    }
                    self.bump(); // view
                    self.bump(); // fn
                    items.push(self.parse_fn_loose(
                        None,
                        FunctionModifiers {
                            kind: FunctionKind::View,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                        declaration_start,
                    )?);
                } else if self.peek(TokenKind::Hajimari) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare `hajimari`/`始まり`",
                        ));
                    }
                    self.bump();
                    items.push(self.parse_fn_loose(
                        Some(String::from("hajimari")),
                        FunctionModifiers {
                            kind: FunctionKind::Hajimari,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                        declaration_start,
                    )?);
                } else if self.peek(TokenKind::Kaizen) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare a `kaizen`/`改善` hook",
                        ));
                    }
                    self.bump();
                    items.push(self.parse_fn_loose(
                        Some(String::from("kaizen")),
                        FunctionModifiers {
                            kind: FunctionKind::Kaizen,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                        declaration_start,
                    )?);
                } else if self.peek_ident_n(0, "meta") {
                    let token = self.bump();
                    return Err(self.error(
                        token,
                        "source-level `meta { ... }` is not supported; select execution capabilities and the cycle ceiling in compiler build configuration",
                    ));
                } else if self.peek_ident_n(0, "fixture") {
                    if !attrs.is_empty() {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "function attributes must precede a function",
                        ));
                    }
                    let fixture = self.parse_fixture_decl()?;
                    self.fixtures.push(fixture);
                } else if self.peek_ident_n(0, "koto_test") {
                    if !attrs.is_empty() {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "function attributes must precede a function",
                        ));
                    }
                    self.parse_test_target_decl()?;
                } else if self.peek(TokenKind::Seiyaku) || self.peek(TokenKind::Module) {
                    let token = self.bump();
                    return Err(self.error(
                        token,
                        "nested or additional seiyaku/module units are not allowed",
                    ));
                } else {
                    let tok = self.bump();
                    return Err(self.error(
                        tok,
                        "source-unit item (fn, kotoage fn, view fn, hajimari, kaizen, trigger, struct, error enum, const, state)",
                    ));
                }
                Ok(())
            })();
            if let Err(error) = result {
                if !self.recover {
                    self.syntax_finish(syntax_item, declaration_start);
                    return Err(error);
                }
                let recovery_start = error.range.start.max(declaration_start);
                self.errors.push(*error);
                let syntax_error = (item_kind != SyntaxKind::ErrorNode)
                    .then(|| self.syntax_start(SyntaxKind::ErrorNode, recovery_start));
                self.synchronize_source_item(item_start);
                if let Some(syntax_error) = syntax_error {
                    self.syntax_finish(syntax_error, recovery_start);
                }
            }
            self.syntax_finish(syntax_item, declaration_start);
        }
        self.syntax_finish_at(syntax_items, self.current_start());
        self.expect(TokenKind::RBrace)?;
        self.finish_node(node);
        self.syntax_finish(syntax_unit, start);
        Ok((SourceUnit { kind, name }, items))
    }

    fn parse_error_enum_def(&mut self) -> ParseResult<Item> {
        let node = self.begin_node(AstNodeKind::ErrorEnum, self.current_start());
        self.expect(TokenKind::Error)?;
        self.expect(TokenKind::Enum)?;
        let (name, name_token) = self.expect_ident_token()?;
        self.record_declaration(
            node,
            name.clone(),
            name_token.range,
            DeclarationKind::ErrorEnum,
            None,
        );
        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();
        let mut names = std::collections::HashSet::new();
        let mut codes = std::collections::HashSet::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let variant_token = self.tokens[self.pos].clone();
            let variant_name = self.expect_ident()?;
            if !names.insert(variant_name.clone()) {
                return Err(self.error(variant_token, "unique error variant name"));
            }
            self.expect(TokenKind::Equal)?;
            let code_token = self.bump();
            let code = match &code_token.kind {
                TokenKind::Number(value) => parse_bounded_unsigned(value, u64::from(u32::MAX))
                    .ok()
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|value| *value != 0)
                    .ok_or_else(|| {
                        self.error(
                            code_token.clone(),
                            "explicit error code in the range 1..=4294967295",
                        )
                    })?,
                _ => {
                    return Err(self.error(
                        code_token,
                        "explicit error code in the range 1..=4294967295",
                    ));
                }
            };
            if !codes.insert(code) {
                return Err(self.error(variant_token, "unique error code"));
            }
            variants.push(ErrorVariant {
                name: variant_name,
                code,
            });
            if self.peek(TokenKind::Comma) || self.peek(TokenKind::Semicolon) {
                self.bump();
            } else if !self.peek(TokenKind::RBrace) {
                let token = self.tokens[self.pos].clone();
                return Err(self.error(token, "`,` or `}` after error variant"));
            }
        }
        self.expect(TokenKind::RBrace)?;
        if variants.is_empty() {
            let token = self.tokens[self.pos.saturating_sub(1)].clone();
            return Err(self.error(token, "at least one explicitly numbered error variant"));
        }
        self.finish_node(node);
        Ok(Item::ErrorEnum(ErrorEnumDef { name, variants }))
    }

    fn parse_trigger_decl(&mut self) -> ParseResult<Item> {
        let node = self.begin_node(AstNodeKind::Trigger, self.current_start());
        let tok = self.bump();
        debug_assert!(matches!(tok.kind, TokenKind::Trigger));
        let (name, name_token) = self.expect_ident_token()?;
        self.record_declaration(
            node,
            name.clone(),
            name_token.range,
            DeclarationKind::Trigger,
            None,
        );
        self.expect(TokenKind::Arrow)?;
        let call = self.parse_trigger_call()?;
        self.expect(TokenKind::LBrace)?;
        let mut filter: Option<TriggerFilter> = None;
        let mut repeats: Option<TriggerRepeats> = None;
        let mut authority: Option<String> = None;
        let mut metadata: Vec<TriggerMetadataEntry> = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let field_tok = self.bump();
            let field_name = match field_tok.kind.clone() {
                TokenKind::Ident(name) => name,
                _ => {
                    return Err(self.error(field_tok, "trigger field"));
                }
            };
            match field_name.as_str() {
                "on" => {
                    if filter.is_some() {
                        return Err(self.error(field_tok, "duplicate `on` field"));
                    }
                    filter = Some(self.parse_trigger_filter()?);
                    if self.peek(TokenKind::Semicolon) {
                        self.bump();
                    }
                }
                "repeats" => {
                    if repeats.is_some() {
                        return Err(self.error(field_tok, "duplicate `repeats` field"));
                    }
                    repeats = Some(self.parse_trigger_repeats()?);
                    self.expect(TokenKind::Semicolon)?;
                }
                "authority" => {
                    if authority.is_some() {
                        return Err(self.error(field_tok, "duplicate `authority` field"));
                    }
                    authority = Some(self.expect_ident_or_string()?);
                    self.expect(TokenKind::Semicolon)?;
                }
                "metadata" => {
                    metadata = self.parse_trigger_metadata_block()?;
                    if self.peek(TokenKind::Semicolon) {
                        self.bump();
                    }
                }
                _ => {
                    return Err(self.error(
                        field_tok,
                        "trigger field (`on`, `repeats`, `authority`, `metadata`)",
                    ));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;
        let filter = filter.ok_or_else(|| self.error(tok, "trigger `on` field"))?;
        self.finish_node(node);
        Ok(Item::Trigger(TriggerDecl {
            name,
            location: SourceLocation {
                line: name_token.line,
                column: name_token.column,
            },
            call,
            filter,
            repeats,
            authority,
            metadata,
        }))
    }

    fn parse_trigger_call(&mut self) -> ParseResult<TriggerCall> {
        let first = self.expect_ident()?;
        if self.peek(TokenKind::ColonColon) {
            self.bump();
            let entrypoint = self.expect_ident()?;
            Ok(TriggerCall {
                namespace: Some(first),
                entrypoint,
            })
        } else {
            Ok(TriggerCall {
                namespace: None,
                entrypoint: first,
            })
        }
    }

    fn parse_trigger_filter(&mut self) -> ParseResult<TriggerFilter> {
        let kind = self.expect_ident()?;
        match kind.as_str() {
            "time" => Ok(TriggerFilter::Time(self.parse_trigger_time_filter()?)),
            "execute" => {
                let next = self.expect_trigger_context_ident()?;
                if next != "trigger" {
                    return Err(self.error(
                        self.tokens[self.pos.saturating_sub(1)].clone(),
                        "execute trigger <name>",
                    ));
                }
                let trigger_id = self.expect_ident_or_string()?;
                Ok(TriggerFilter::Execute { trigger_id })
            }
            "data" => Ok(TriggerFilter::Data(self.parse_trigger_data_filter()?)),
            "pipeline" => Ok(TriggerFilter::Pipeline(
                self.parse_trigger_pipeline_filter()?,
            )),
            _ => Err(self.error(
                self.tokens[self.pos.saturating_sub(1)].clone(),
                "trigger filter (`time`, `execute`, `data`, or `pipeline`)",
            )),
        }
    }

    fn parse_trigger_data_filter(&mut self) -> ParseResult<TriggerDataFilter> {
        let kind = self.expect_trigger_context_ident()?;
        match kind.as_str() {
            "any" => Ok(TriggerDataFilter::Any),
            _ => {
                let family = self.parse_trigger_data_family_keyword(&kind)?;
                let event = match self.expect_ident()?.as_str() {
                    "any" => TriggerDataEventKind::Any,
                    other => TriggerDataEventKind::Named(other.to_string()),
                };
                let matchers = self.parse_trigger_data_matcher_block()?;
                Ok(TriggerDataFilter::Structured(TriggerStructuredDataFilter {
                    family,
                    event,
                    matchers,
                }))
            }
        }
    }

    fn parse_trigger_data_family_keyword(&self, family: &str) -> ParseResult<TriggerDataFamily> {
        match family {
            "peer" => Ok(TriggerDataFamily::Peer),
            "domain" => Ok(TriggerDataFamily::Domain),
            "account" => Ok(TriggerDataFamily::Account),
            "asset" => Ok(TriggerDataFamily::Asset),
            "asset_definition" => Ok(TriggerDataFamily::AssetDefinition),
            "nft" => Ok(TriggerDataFamily::Nft),
            "rwa" => Ok(TriggerDataFamily::Rwa),
            "trigger" => Ok(TriggerDataFamily::Trigger),
            "role" => Ok(TriggerDataFamily::Role),
            "configuration" => Ok(TriggerDataFamily::Configuration),
            "executor" => Ok(TriggerDataFamily::Executor),
            _ => Err(self.error(
                self.tokens[self.pos.saturating_sub(1)].clone(),
                "data family (`any`, `peer`, `domain`, `account`, `asset`, `asset_definition`, `nft`, `rwa`, `trigger`, `role`, `configuration`, or `executor`)",
            )),
        }
    }

    fn parse_trigger_data_matcher_block(&mut self) -> ParseResult<Vec<TriggerDataMatcher>> {
        self.expect(TokenKind::LBrace)?;
        let mut matchers = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let key = self.expect_trigger_context_ident()?;
            let value = self.expect_ident_or_string()?;
            self.expect(TokenKind::Semicolon)?;
            matchers.push(TriggerDataMatcher { key, value });
        }
        self.expect(TokenKind::RBrace)?;
        Ok(matchers)
    }

    fn parse_trigger_pipeline_filter(&mut self) -> ParseResult<TriggerPipelineFilter> {
        let kind = self.expect_ident()?;
        match kind.as_str() {
            "transaction" => {
                if self.peek_ident_n(0, "approved") {
                    self.bump();
                }
                Ok(TriggerPipelineFilter::TransactionApproved)
            }
            "block" => {
                if self.peek_ident_n(0, "approved") {
                    self.bump();
                }
                Ok(TriggerPipelineFilter::BlockApproved)
            }
            _ => Err(self.error(
                self.tokens[self.pos.saturating_sub(1)].clone(),
                "pipeline filter (`transaction [approved]` or `block [approved]`)",
            )),
        }
    }

    fn parse_trigger_time_filter(&mut self) -> ParseResult<TriggerTimeFilter> {
        let kind = self.expect_ident()?;
        match kind.as_str() {
            "pre_commit" => Ok(TriggerTimeFilter::PreCommit),
            "schedule" => {
                self.expect(TokenKind::LParen)?;
                let start_ms = self.parse_u64_literal("schedule start_ms")?;
                let period_ms = if self.peek(TokenKind::Comma) {
                    self.bump();
                    Some(self.parse_u64_literal("schedule period_ms")?)
                } else {
                    None
                };
                self.expect(TokenKind::RParen)?;
                Ok(TriggerTimeFilter::Schedule {
                    start_ms,
                    period_ms,
                })
            }
            _ => Err(self.error(
                self.tokens[self.pos.saturating_sub(1)].clone(),
                "time filter (`pre_commit` or `schedule`)",
            )),
        }
    }

    fn parse_trigger_repeats(&mut self) -> ParseResult<TriggerRepeats> {
        if self.peek_ident_n(0, "indefinitely") {
            self.bump();
            return Ok(TriggerRepeats::Indefinitely);
        }
        let value = self.parse_u64_literal("repeats")?;
        let count = u32::try_from(value).map_err(|_| {
            self.range_error(
                &self.tokens[self.pos.saturating_sub(1)],
                "repeats integer literal out of range".to_string(),
            )
        })?;
        Ok(TriggerRepeats::Exactly(count))
    }

    fn parse_trigger_metadata_block(&mut self) -> ParseResult<Vec<TriggerMetadataEntry>> {
        self.expect(TokenKind::LBrace)?;
        let mut entries = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let key_tok = self.bump();
            let key = match key_tok.kind {
                TokenKind::Ident(ref s) => s.clone(),
                TokenKind::String(ref s) => s.clone(),
                _ => {
                    return Err(self.error(key_tok, "metadata key (identifier or string literal)"));
                }
            };
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr()?;
            self.expect(TokenKind::Semicolon)?;
            entries.push(TriggerMetadataEntry { key, value });
        }
        self.expect(TokenKind::RBrace)?;
        Ok(entries)
    }

    fn parse_u64_literal(&mut self, context: &str) -> ParseResult<u64> {
        let tok = self.bump();
        match tok.kind.clone() {
            TokenKind::Number(n) => parse_bounded_unsigned(&n, u64::MAX).map_err(|_| {
                self.range_error(&tok, format!("{context} integer literal out of range"))
            }),
            TokenKind::Minus => Err(self.error(
                tok,
                &format!("{context} expects a non-negative integer literal"),
            )),
            _ => Err(self.error(
                tok,
                &format!("{context} expects a non-negative integer literal"),
            )),
        }
    }

    fn expect_ident_or_string(&mut self) -> ParseResult<String> {
        let tok = self.bump();
        match tok.kind.clone() {
            TokenKind::Ident(s) => Ok(s),
            TokenKind::String(s) => Ok(s),
            _ => Err(self.error(tok, "identifier or string literal")),
        }
    }

    fn parse_function_attributes(&mut self) -> ParseResult<FunctionAttributes> {
        let mut attrs = FunctionAttributes::default();
        while self.peek(TokenKind::Hash) {
            let attribute_start = self.current_start();
            let syntax_attribute = self.syntax_start(SyntaxKind::Attribute, attribute_start);
            let result = (|| -> ParseResult<()> {
                self.bump(); // '#'
                self.expect(TokenKind::LBracket)?;
                let attr_tok = self.bump();
                let attr_name = if let TokenKind::Ident(name) = attr_tok.kind.clone() {
                    name
                } else {
                    return Err(self.error(attr_tok, "expected attribute identifier"));
                };
                match attr_name.as_str() {
                    "access" => {
                        return Err(self.error(
                            attr_tok,
                            "manual `#[access(...)]` hints are not supported in first-release Kotodama; access metadata is generated by the compiler",
                        ));
                    }
                    "test" => self.parse_test_attribute_body(&mut attrs)?,
                    _ => {
                        return Err(self.error(attr_tok, "expected attribute `test`"));
                    }
                }
                let next_item = self
                    .tokens
                    .get(self.pos)
                    .is_some_and(Self::token_starts_source_item);
                self.expect_or_insert(TokenKind::RBracket, next_item)?;
                Ok(())
            })();
            self.syntax_finish(syntax_attribute, attribute_start);
            result?;
        }
        Ok(attrs)
    }

    fn parse_access_value_list(&mut self) -> ParseResult<Vec<String>> {
        if self.peek(TokenKind::LBracket) {
            self.bump();
            let mut values = Vec::new();
            while !self.peek(TokenKind::RBracket) && !self.peek(TokenKind::EOF) {
                let tok = self.bump();
                match tok.kind {
                    TokenKind::String(s) => values.push(s),
                    _ => return Err(self.error(tok, "string literal")),
                }
                if self.peek(TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect(TokenKind::RBracket)?;
            return Ok(values);
        }
        let tok = self.bump();
        match tok.kind {
            TokenKind::String(s) => Ok(vec![s]),
            _ => Err(self.error(tok, "string literal")),
        }
    }

    #[allow(dead_code)]
    fn parse_access_attribute_body(&mut self, attrs: &mut FunctionAttributes) -> ParseResult<()> {
        self.expect(TokenKind::LParen)?;
        let mut parsed_any = false;
        while !self.peek(TokenKind::RParen) && !self.peek(TokenKind::EOF) {
            let key = self.expect_ident()?;
            self.expect(TokenKind::Equal)?;
            let mut values = self.parse_access_value_list()?;
            match key.as_str() {
                "read" => attrs.reads.append(&mut values),
                "write" => attrs.writes.append(&mut values),
                _ => {
                    return Err(Box::new(ParseError {
                        code: "K1001",
                        message: format!("unknown access list `{key}`"),
                        line: self.tokens[self.pos.saturating_sub(1)].line,
                        column: self.tokens[self.pos.saturating_sub(1)].column,
                        snippet: String::new(),
                        range: self.tokens[self.pos.saturating_sub(1)].range,
                        fix: None,
                        expected: None,
                        expected_owner: None,
                    }));
                }
            }
            parsed_any = true;
            if self.peek(TokenKind::Comma) {
                self.bump();
            }
        }
        if !parsed_any {
            return Err(Box::new(ParseError {
                code: "K1001",
                message: "access attribute must include read/write entries".into(),
                line: self.tokens[self.pos.saturating_sub(1)].line,
                column: self.tokens[self.pos.saturating_sub(1)].column,
                snippet: String::new(),
                range: self.tokens[self.pos.saturating_sub(1)].range,
                fix: None,
                expected: None,
                expected_owner: None,
            }));
        }
        self.expect(TokenKind::RParen)?;
        Ok(())
    }

    fn parse_test_attribute_body(&mut self, attrs: &mut FunctionAttributes) -> ParseResult<()> {
        attrs.is_test = true;
        if !self.peek(TokenKind::LParen) {
            return Ok(());
        }
        self.bump(); // '('
        while !self.peek(TokenKind::RParen) && !self.peek(TokenKind::EOF) {
            let key = self.expect_ident()?;
            self.expect(TokenKind::Equal)?;
            match key.as_str() {
                "fixture" => {
                    if attrs.test_fixture.is_some() {
                        return Err(Box::new(ParseError {
                            code: "K1001",
                            message: "duplicate fixture binding in test attribute".into(),
                            line: self.tokens[self.pos.saturating_sub(1)].line,
                            column: self.tokens[self.pos.saturating_sub(1)].column,
                            snippet: String::new(),
                            range: self.tokens[self.pos.saturating_sub(1)].range,
                            fix: None,
                            expected: None,
                            expected_owner: None,
                        }));
                    }
                    attrs.test_fixture = Some(self.expect_ident_or_string()?);
                }
                _ => {
                    return Err(Box::new(ParseError {
                        code: "K1001",
                        message: format!("unknown test attribute option `{key}`"),
                        line: self.tokens[self.pos.saturating_sub(1)].line,
                        column: self.tokens[self.pos.saturating_sub(1)].column,
                        snippet: String::new(),
                        range: self.tokens[self.pos.saturating_sub(1)].range,
                        fix: None,
                        expected: None,
                        expected_owner: None,
                    }));
                }
            }
            if self.peek(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(())
    }

    fn parse_test_target_decl(&mut self) -> ParseResult<()> {
        let tok = self.bump();
        if !matches!(tok.kind, TokenKind::Ident(ref s) if s == "koto_test") {
            return Err(self.error(tok, "koto_test"));
        }
        self.expect(TokenKind::LBrace)?;
        let mut target = None;
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let key = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            match key.as_str() {
                "target" => target = Some(self.expect_ident_or_string()?),
                _ => {
                    return Err(
                        self.error(self.tokens[self.pos.saturating_sub(1)].clone(), "target")
                    );
                }
            }
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
            }
        }
        self.expect(TokenKind::RBrace)?;
        let target = target.ok_or_else(|| ParseError {
            code: "K1001",
            message: "koto_test block requires `target: \"...\"`".into(),
            line: tok.line,
            column: tok.column,
            snippet: String::new(),
            range: tok.range,
            fix: None,
            expected: None,
            expected_owner: None,
        })?;
        self.test_target = Some(TestTargetDecl { target });
        Ok(())
    }

    fn parse_fixture_decl(&mut self) -> ParseResult<FixtureDecl> {
        let tok = self.bump();
        if !matches!(tok.kind, TokenKind::Ident(ref s) if s == "fixture") {
            return Err(self.error(tok, "fixture"));
        }
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut actions = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let action_name = self.expect_ident()?;
            self.expect(TokenKind::LParen)?;
            let mut args = Vec::new();
            if !self.peek(TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if self.peek(TokenKind::Comma) {
                        self.bump();
                        if self.peek(TokenKind::RParen) {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen)?;
            if self.peek(TokenKind::Semicolon) {
                self.bump();
            }
            actions.push(FixtureAction {
                name: action_name,
                args,
            });
        }
        self.expect(TokenKind::RBrace)?;
        Ok(FixtureDecl { name, actions })
    }

    fn parse_struct_def(&mut self) -> ParseResult<Item> {
        // struct Name { Type field; ... }
        let node = self.begin_node(AstNodeKind::Struct, self.current_start());
        self.expect(TokenKind::Struct)?;
        let (name, name_token) = self.expect_ident_token()?;
        self.record_declaration(
            node,
            name.clone(),
            name_token.range,
            DeclarationKind::Struct,
            None,
        );
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            // Allow stray separators.
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
                continue;
            }
            let ty = self.parse_type_expr()?;
            let field_name = if self.peek(TokenKind::Colon) {
                let token = self.bump();
                return Err(self.coded_error(
                    token,
                    "E_RETIRED_DECLARATION_ORDER",
                    "Kotodama V1 struct fields are type-first: write `int field;`, not `field: int;`",
                ));
            } else {
                self.expect_ident()?
            };
            fields.push((field_name, ty));
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
            }
        }
        self.expect(TokenKind::RBrace)?;
        self.finish_node(node);
        Ok(Item::Struct(super::ast::StructDef { name, fields }))
    }

    fn parse_state_decl(&mut self) -> ParseResult<Item> {
        // Canonical V1 form: `state Type name;`.
        let node = self.begin_node(AstNodeKind::State, self.current_start());
        self.expect(TokenKind::State)?;
        let ty = self.parse_type_expr()?;
        if self.peek(TokenKind::Colon) {
            let token = self.bump();
            return Err(self.coded_error(
                token,
                "E_RETIRED_DECLARATION_ORDER",
                "Kotodama V1 state declarations are type-first: write `state int value;`, not `state value: int;`",
            ));
        }
        let (name, name_token) = self.expect_ident_token()?;
        self.record_declaration(
            node,
            name.clone(),
            name_token.range,
            DeclarationKind::State,
            None,
        );
        self.expect(TokenKind::Semicolon)?;
        self.finish_node(node);
        Ok(Item::State(super::ast::StateDecl { name, ty }))
    }

    fn parse_const_decl(&mut self) -> ParseResult<Item> {
        let node = self.begin_node(AstNodeKind::Const, self.current_start());
        self.expect(TokenKind::Const)?;
        let ty = Some(self.parse_type_expr()?);
        if self.peek(TokenKind::Colon) {
            let token = self.bump();
            return Err(self.coded_error(
                token,
                "E_RETIRED_DECLARATION_ORDER",
                "Kotodama V1 constants are type-first: write `const int limit = 1;`, not `const limit: int = 1;`",
            ));
        }
        let (name, name_token) = self.expect_ident_token()?;
        self.record_declaration(
            node,
            name.clone(),
            name_token.range,
            DeclarationKind::Const,
            None,
        );
        self.expect(TokenKind::Equal)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semicolon)?;
        self.finish_node(node);
        Ok(Item::Const(super::ast::ConstDecl { name, ty, value }))
    }

    fn parse_fn_loose(
        &mut self,
        name_override: Option<String>,
        mut modifiers: FunctionModifiers,
        declaration_start: u32,
    ) -> ParseResult<Item> {
        let (location, name, name_range) = if let Some(name) = name_override {
            let token = self.tokens[self.pos.saturating_sub(1)].clone();
            (
                SourceLocation {
                    line: token.line,
                    column: token.column,
                },
                name,
                token.range,
            )
        } else {
            let (name, token) = self.expect_ident_token()?;
            (
                SourceLocation {
                    line: token.line,
                    column: token.column,
                },
                name,
                token.range,
            )
        };
        let node = self.begin_node(AstNodeKind::Function, declaration_start);
        self.record_declaration(
            node,
            name.clone(),
            name_range,
            DeclarationKind::Function,
            None,
        );
        let previous_function = self.current_function.replace(node);
        let result = (|| -> ParseResult<Item> {
            let params_start = self.current_start();
            let params = self.with_syntax(SyntaxKind::ParamList, params_start, |this| {
                this.expect(TokenKind::LParen)?;
                let mut params = Vec::new();
                if !this.peek(TokenKind::RParen) {
                    loop {
                        params.push(this.parse_param()?);
                        if this.peek(TokenKind::Comma) {
                            this.bump();
                            if this.peek(TokenKind::RParen) {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                let function_body_or_modifier = this.peek(TokenKind::LBrace)
                    || this.peek(TokenKind::Arrow)
                    || this.peek(TokenKind::Authorize);
                this.expect_or_insert(TokenKind::RParen, function_body_or_modifier)?;
                Ok(params)
            });
            let params = params?;
            let parameter_names = params
                .iter()
                .map(|parameter| parameter.name.clone())
                .collect::<Vec<_>>();
            self.declared_function_parameters
                .entry(name.clone())
                .and_modify(|known| *known = None)
                .or_insert_with(|| Some(parameter_names));
            let mut ret_ty = None;
            if self.peek(TokenKind::Arrow) {
                self.bump();
                ret_ty = Some(self.parse_type_expr()?);
            }
            // Caller authorization is mandatory for mutating public kotoage
            // and optional for read-only views.
            while !self.peek(TokenKind::LBrace) && !self.peek(TokenKind::EOF) {
                if self.peek(TokenKind::Authorize) {
                    if matches!(
                        modifiers.kind,
                        FunctionKind::Hajimari | FunctionKind::Kaizen
                    ) {
                        let token = self.bump();
                        return Err(self.error(
                            token,
                            "lifecycle authorization is runtime-defined; `hajimari`/`始まり` and `kaizen`/`改善` cannot declare `authorize(...)`",
                        ));
                    }
                    self.bump();
                    self.expect(TokenKind::LParen)?;
                    let permission_token = self.bump();
                    let perm = match permission_token.kind.clone() {
                        TokenKind::String(permission) if !permission.trim().is_empty() => {
                            permission
                        }
                        TokenKind::String(_) => {
                            return Err(
                                self.error(permission_token, "non-empty permission string literal")
                            );
                        }
                        _ => return Err(self.error(permission_token, "permission string literal")),
                    };
                    self.expect(TokenKind::RParen)?;
                    if !matches!(modifiers.kind, FunctionKind::Kotoage | FunctionKind::View) {
                        return Err(Box::new(ParseError {
                            code: "K1001",
                            message: "`authorize(...)` is only valid on `kotoage`/`言挙げ` and `view fn` declarations".into(),
                            line: self.tokens[self.pos.saturating_sub(1)].line,
                            column: self.tokens[self.pos.saturating_sub(1)].column,
                            snippet: String::new(),
                            range: self.tokens[self.pos.saturating_sub(1)].range,
                            fix: None,
                            expected: None,
                            expected_owner: None,
                        }));
                    }
                    if modifiers.permission.is_some() {
                        return Err(Box::new(ParseError {
                            code: "K1001",
                            message: "duplicate authorize modifier".into(),
                            line: self.tokens[self.pos.saturating_sub(1)].line,
                            column: self.tokens[self.pos.saturating_sub(1)].column,
                            snippet: String::new(),
                            range: self.tokens[self.pos.saturating_sub(1)].range,
                            fix: None,
                            expected: None,
                            expected_owner: None,
                        }));
                    }
                    modifiers.permission = Some(perm);
                } else {
                    let tok = self.bump();
                    return Err(self.error(tok, "`authorize(\"Permission\")` or `{`"));
                }
            }
            if modifiers.kind == FunctionKind::Kotoage && modifiers.permission.is_none() {
                return Err(self.coded_error(
                    self.tokens[self.pos].clone(),
                    "K1001",
                    format!(
                        "kotoage function `{name}` requires `authorize(\"Permission\")` before its body"
                    ),
                ));
            }
            let body = self.parse_block()?;
            Ok(Item::Function(Function {
                name,
                params,
                ret_ty,
                body,
                modifiers,
                location,
            }))
        })();
        self.current_function = previous_function;
        if result.is_ok() {
            self.finish_node(node);
        }
        result
    }

    fn parse_block(&mut self) -> ParseResult<Block> {
        let block_start = self.current_start();
        let syntax_block = self.syntax_start(SyntaxKind::Block, block_start);
        self.expect(TokenKind::LBrace)?;
        let syntax_statements =
            self.syntax_start(SyntaxKind::StatementList, self.previous_end(block_start));
        let mut statements = Vec::new();
        let mut tail = None;
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let statement_start = self.pos;
            let start = self.tokens[statement_start].range.start;
            let initial_kind = self.syntax_statement_kind(statement_start);
            let syntax_statement = self.syntax_start(initial_kind, start);
            match self.parse_block_element() {
                Ok(element @ ParsedBlockElement::Statement(_)) => {
                    self.syntax_set_kind(syntax_statement, block_element_syntax_kind(&element));
                    let ParsedBlockElement::Statement(statement) = element else {
                        unreachable!("matched statement block element")
                    };
                    let start = self.tokens[statement_start].range.start;
                    let end = self.previous_end(start);
                    statements.push(if statement.source_node().is_some() {
                        statement
                    } else {
                        self.source_statement(TextRange::new(start, end), statement)
                    });
                    self.syntax_finish(syntax_statement, start);
                }
                Ok(element @ ParsedBlockElement::Tail(_)) => {
                    self.syntax_set_kind(syntax_statement, block_element_syntax_kind(&element));
                    let ParsedBlockElement::Tail(expression) = element else {
                        unreachable!("matched tail block element")
                    };
                    tail = Some(Box::new(expression));
                    self.syntax_finish(syntax_statement, start);
                    break;
                }
                Err(error) if self.recover => {
                    let recovery_start = error.range.start.max(start);
                    self.errors.push(*error);
                    let syntax_error = self.syntax_start(SyntaxKind::ErrorNode, recovery_start);
                    self.synchronize_statement(statement_start);
                    self.syntax_finish(syntax_error, recovery_start);
                    self.syntax_finish(syntax_statement, start);
                }
                Err(error) => {
                    self.syntax_finish(syntax_statement, start);
                    return Err(error);
                }
            }
        }
        self.syntax_finish_at(syntax_statements, self.current_start());
        self.expect(TokenKind::RBrace)?;
        self.syntax_finish(syntax_block, block_start);
        Ok(Block { statements, tail })
    }

    fn parse_block_element(&mut self) -> ParseResult<ParsedBlockElement> {
        if self.peek(TokenKind::Let) || self.peek(TokenKind::Var) {
            let statement_start = self.current_start();
            let owner = self.begin_node(AstNodeKind::Statement, statement_start);
            let mutable = self.peek(TokenKind::Var);
            self.bump();
            let ty = if self.typed_local_starts_here() {
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            // pattern
            let pat = if self.peek(TokenKind::LParen) {
                self.bump();
                let mut names = Vec::new();
                loop {
                    let (name, token) = self.expect_ident_token()?;
                    self.record_binding(
                        owner,
                        names.len(),
                        name.clone(),
                        token.range,
                        BindingFactKind::Local,
                    );
                    names.push(name);
                    if self.peek(TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Pattern::Tuple(names)
            } else {
                let (name, token) = self.expect_ident_token()?;
                self.record_binding(owner, 0, name.clone(), token.range, BindingFactKind::Local);
                Pattern::Name(name)
            };
            if self.peek(TokenKind::Colon) {
                let token = self.bump();
                return Err(self.coded_error(
                    token,
                    "E_RETIRED_DECLARATION_ORDER",
                    "Kotodama V1 typed locals are type-first: write `let int value = ...;`; omit the type entirely to use inference",
                ));
            }
            self.expect(TokenKind::Equal)?;
            let expr = self.parse_expr()?;
            self.expect(TokenKind::Semicolon)?;
            let range = TextRange::new(statement_start, self.previous_end(statement_start));
            Ok(ParsedBlockElement::Statement(self.finish_owned_statement(
                owner,
                range,
                Statement::Let {
                    mutable,
                    pat,
                    ty,
                    value: expr,
                },
            )))
        } else if self.peek(TokenKind::Return) {
            self.bump();
            if self.peek(TokenKind::Semicolon) {
                self.bump();
                Ok(ParsedBlockElement::Statement(Statement::Return(None)))
            } else if self.peek(TokenKind::RBrace) {
                // `return` is a statement even when it has no value. At the
                // block boundary the only possible continuation is its
                // required semicolon, so recover that exact token instead of
                // fabricating a missing expression.
                self.expect(TokenKind::Semicolon)?;
                unreachable!("a missing return semicolon always reports an error")
            } else {
                let expr = self.parse_expr()?;
                self.expect(TokenKind::Semicolon)?;
                Ok(ParsedBlockElement::Statement(Statement::Return(Some(expr))))
            }
        } else if self.peek(TokenKind::Break) {
            self.bump();
            self.expect(TokenKind::Semicolon)?;
            Ok(ParsedBlockElement::Statement(Statement::Break))
        } else if self.peek(TokenKind::Continue) {
            self.bump();
            self.expect(TokenKind::Semicolon)?;
            Ok(ParsedBlockElement::Statement(Statement::Continue))
        } else if self.peek(TokenKind::If) {
            let expression = self.parse_if_expression()?;
            self.finish_block_expression(expression)
        } else if self.peek(TokenKind::Match) {
            let expression = self.parse_match_expression()?;
            self.finish_block_expression(expression)
        } else if self.peek(TokenKind::For) {
            let for_line = self.tokens.get(self.pos).map(|t| t.line).unwrap_or(0);
            let for_start = self.current_start();
            self.expect(TokenKind::For)?;
            if let Some((init, cond, step)) = self.parse_for_range()? {
                let body = self.parse_block()?;
                Ok(ParsedBlockElement::Statement(Statement::For {
                    line: for_line,
                    init: Some(Box::new(init)),
                    cond: Some(cond),
                    step: Some(Box::new(step)),
                    body,
                }))
            } else if let Some((owner, k, v_opt, map)) = self.parse_for_each_map(for_start)? {
                if !map_iteration_has_explicit_bound(&map) {
                    return Err(self.error(
                        self.tokens[self.pos.saturating_sub(1)].clone(),
                        "StateMap iteration requires `.take(N)` or `.range(start, end)` with int literals",
                    ));
                }
                let body = self.parse_block()?;
                let range = TextRange::new(for_start, self.previous_end(for_start));
                Ok(ParsedBlockElement::Statement(self.finish_owned_statement(
                    owner,
                    range,
                    Statement::ForEachMap {
                        key: k,
                        value: v_opt,
                        map,
                        body,
                    },
                )))
            } else {
                let token = self.tokens[self.pos.saturating_sub(1)].clone();
                Err(self.error(
                    token,
                    "only `for item in range(end)` and StateMap iteration through `.take(literal)` or `.range(literal, literal)` are supported",
                ))
            }
        } else if self.peek_ident_n(0, "while") {
            let token = self.bump();
            Err(self.error(
                token,
                "`while` is not supported in Kotodama V1; use a compiler-proven bounded `for` loop",
            ))
        } else {
            // Try assignments including compound ops and field/indexed lvalues
            let save = self.pos;
            let syntax_checkpoint = self.syntax_checkpoint();
            if let Ok(target) = self.try_parse_lvalue_expr()
                && (self.peek(TokenKind::Equal)
                    || self.peek(TokenKind::PlusEqual)
                    || self.peek(TokenKind::MinusEqual)
                    || self.peek(TokenKind::StarEqual)
                    || self.peek(TokenKind::SlashEqual)
                    || self.peek(TokenKind::PercentEqual))
            {
                let op_tok = self.bump();
                let rhs = self.parse_expr()?;
                self.expect(TokenKind::Semicolon)?;
                let op = match op_tok.kind {
                    TokenKind::Equal => AssignOp::Set,
                    TokenKind::PlusEqual => AssignOp::Add,
                    TokenKind::MinusEqual => AssignOp::Sub,
                    TokenKind::StarEqual => AssignOp::Mul,
                    TokenKind::SlashEqual => AssignOp::Div,
                    TokenKind::PercentEqual => AssignOp::Mod,
                    _ => {
                        return Err(self.error(op_tok, "expected one of: =, +=, -=, *=, /=, %="));
                    }
                };
                return Ok(match (target, op) {
                    (Expr::Ident(name), AssignOp::Set) => {
                        ParsedBlockElement::Statement(Statement::Assign { name, value: rhs })
                    }
                    (t, op) => ParsedBlockElement::Statement(Statement::AssignExpr {
                        target: t,
                        op,
                        value: rhs,
                    }),
                });
            }
            // Not an assignment (or not an lvalue); rewind both the token view
            // and syntax events before parsing the expression authoritatively.
            self.pos = save;
            self.syntax_rollback(syntax_checkpoint);
            let expr = self.parse_expr()?;
            self.finish_block_expression(expr)
        }
    }

    fn finish_block_expression(&mut self, expression: Expr) -> ParseResult<ParsedBlockElement> {
        if self.peek(TokenKind::Semicolon) {
            self.bump();
            return Ok(ParsedBlockElement::Statement(Statement::Expr(expression)));
        }

        let missing_else = matches!(
            expression.kind(),
            Expr::If {
                else_branch: None,
                ..
            } | Expr::IfLet {
                else_branch: None,
                ..
            }
        );
        if self.peek(TokenKind::RBrace)
            && !missing_else
            && block_expression_flow(&expression) != BlockExpressionFlow::Unit
        {
            return Ok(ParsedBlockElement::Tail(expression));
        }

        if matches!(expression.kind(), Expr::If { .. } | Expr::IfLet { .. }) {
            return Ok(ParsedBlockElement::Statement(
                self.if_expression_statement(expression),
            ));
        }
        if matches!(expression.kind(), Expr::Match { .. }) {
            return Ok(ParsedBlockElement::Statement(Statement::Expr(expression)));
        }

        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .unwrap_or_else(|| self.bump());
        Err(self.error(token, "`;` or the end of the enclosing block"))
    }

    fn if_expression_statement(&mut self, expression: Expr) -> Statement {
        let mut expression = expression;
        let mut wrappers = Vec::new();
        while let Expr::Source {
            node,
            source,
            expression: inner,
        } = expression
        {
            wrappers.push((node, source));
            expression = *inner;
        }
        assert!(
            !wrappers.is_empty(),
            "direct if-expression parser always returns a source owner"
        );
        let mut statement = if_expression_statement_inner(expression);
        for (node, source) in wrappers.into_iter().rev() {
            self.facts.source_map.set_kind(node, AstNodeKind::Statement);
            statement = Statement::Source {
                node,
                source,
                statement: Box::new(statement),
            };
        }
        statement
    }

    fn parse_if_expression(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let owner = self.begin_node(AstNodeKind::Expression, start);
        let expression = self.with_syntax(SyntaxKind::IfExpr, start, |parser| {
            parser.parse_if_expression_inner(owner)
        })?;
        let range = TextRange::new(start, self.previous_end(start));
        Ok(self.finish_owned_expression(owner, AstNodeKind::Expression, range, expression))
    }

    fn parse_if_expression_inner(&mut self, owner: NodeId) -> ParseResult<Expr> {
        self.expect(TokenKind::If)?;
        let (pattern, value, condition) = if self.peek(TokenKind::Let) {
            self.bump();
            let pattern = self.parse_sum_pattern(owner, 0)?;
            self.expect(TokenKind::Equal)?;
            let value = self.parse_expr_before_block()?;
            (Some(pattern), Some(value), None)
        } else {
            (None, None, Some(self.parse_expr_before_block()?))
        };
        let then_branch = self.parse_block()?;
        let else_branch = if self.peek(TokenKind::Else) {
            self.bump();
            if self.peek(TokenKind::If) {
                let nested = self.parse_if_expression()?;
                if block_expression_flow(&nested) != BlockExpressionFlow::Unit {
                    Some(Block {
                        statements: Vec::new(),
                        tail: Some(Box::new(nested)),
                    })
                } else {
                    let statement = self.if_expression_statement(nested);
                    Some(Block {
                        statements: vec![statement],
                        tail: None,
                    })
                }
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        if let Some(pattern) = pattern {
            Ok(Expr::IfLet {
                pattern,
                value: Box::new(value.expect("if let value")),
                then_branch,
                else_branch,
            })
        } else {
            Ok(Expr::If {
                condition: Box::new(condition.expect("if condition")),
                then_branch,
                else_branch,
            })
        }
    }

    fn parse_match_expression(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let owner = self.begin_node(AstNodeKind::Expression, start);
        let expression = self.with_syntax(SyntaxKind::MatchExpr, start, |parser| {
            parser.parse_match_expression_inner(owner)
        })?;
        let range = TextRange::new(start, self.previous_end(start));
        Ok(self.finish_owned_expression(owner, AstNodeKind::Expression, range, expression))
    }

    fn parse_match_expression_inner(&mut self, owner: NodeId) -> ParseResult<Expr> {
        self.expect(TokenKind::Match)?;
        let value = self.parse_expr_before_block()?;
        self.expect(TokenKind::LBrace)?;
        let mut arms = Vec::new();
        let mut binding_ordinal = 0_usize;
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let arm_start = self.current_start();
            let syntax_arm = self.syntax_start(SyntaxKind::MatchArm, arm_start);
            let arm = (|| -> ParseResult<(SumPattern, Block)> {
                let pattern = self.parse_sum_pattern(owner, binding_ordinal)?;
                if matches!(&pattern.binding, Some(PatternBinding::Name(_))) {
                    binding_ordinal = binding_ordinal.saturating_add(1);
                }
                self.expect(TokenKind::FatArrow)?;
                let body = if self.peek(TokenKind::LBrace) {
                    self.parse_block()?
                } else {
                    Block {
                        statements: Vec::new(),
                        tail: Some(Box::new(self.parse_expr()?)),
                    }
                };
                Ok((pattern, body))
            })();
            self.syntax_finish(syntax_arm, arm_start);
            let (pattern, body) = arm?;
            arms.push(MatchArm { pattern, body });
            if !self.peek(TokenKind::Comma) {
                if !self.peek(TokenKind::RBrace) {
                    let token = self
                        .tokens
                        .get(self.pos)
                        .cloned()
                        .unwrap_or_else(|| self.bump());
                    return Err(self.error(token, "`,` or `}` after match arm"));
                }
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Expr::Match {
            value: Box::new(value),
            arms,
        })
    }

    fn parse_sum_pattern(&mut self, owner: NodeId, ordinal: usize) -> ParseResult<SumPattern> {
        let start = self.current_start();
        self.with_syntax(SyntaxKind::SumPattern, start, |parser| {
            parser.parse_sum_pattern_inner(owner, ordinal)
        })
    }

    fn parse_sum_pattern_inner(
        &mut self,
        owner: NodeId,
        ordinal: usize,
    ) -> ParseResult<SumPattern> {
        let namespace_token = self.bump();
        let TokenKind::Ident(namespace) = namespace_token.kind.clone() else {
            return Err(self.error(namespace_token, "`Option` or `Result` pattern namespace"));
        };
        self.expect(TokenKind::ColonColon)?;
        let variant_token = self.bump();
        let TokenKind::Ident(variant_name) = variant_token.kind.clone() else {
            return Err(self.error(variant_token, "namespaced sum variant"));
        };
        if namespace == "option" || namespace == "result" {
            let replacement = if namespace == "option" {
                "Option"
            } else {
                "Result"
            };
            let mut error = self.coded_error(
                namespace_token,
                "E_LEGACY_SUM_CONSTRUCTOR",
                format!(
                    "lowercase `{namespace}` pattern namespace is retired; use `{replacement}`"
                ),
            );
            error.fix = Some(replacement.to_owned());
            return Err(error);
        }
        let variant = match (namespace.as_str(), variant_name.as_str()) {
            ("Option", "some") => SumVariant::OptionSome,
            ("Option", "none") => SumVariant::OptionNone,
            ("Result", "ok") => SumVariant::ResultOk,
            ("Result", "err") => SumVariant::ResultErr,
            _ => {
                return Err(self.error(
                    variant_token,
                    "one of `Option::some`, `Option::none`, `Result::ok`, or `Result::err`",
                ));
            }
        };
        let binding = if variant == SumVariant::OptionNone {
            if self.peek(TokenKind::LParen) {
                let token = self.bump();
                return Err(self.error(token, "`Option::none` without a payload pattern"));
            }
            None
        } else {
            self.expect(TokenKind::LParen)?;
            let token = self.bump();
            let TokenKind::Ident(name) = token.kind.clone() else {
                return Err(self.error(token, "payload binding or `_`"));
            };
            let binding = if name == "_" {
                PatternBinding::Wildcard
            } else {
                self.record_binding(
                    owner,
                    ordinal,
                    name.clone(),
                    token.range,
                    BindingFactKind::Pattern,
                );
                PatternBinding::Name(name)
            };
            self.expect(TokenKind::RParen)?;
            Some(binding)
        };
        Ok(SumPattern { variant, binding })
    }

    fn inc_statement(&mut self, name: String, range: TextRange) -> Statement {
        let left =
            self.source_expression(AstNodeKind::Expression, range, Expr::Ident(name.clone()));
        let right = self.source_expression(
            AstNodeKind::Expression,
            range,
            Expr::IntLiteral(BigInt::one()),
        );
        let value = self.source_expression(
            AstNodeKind::Expression,
            range,
            Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(left),
                right: Box::new(right),
            },
        );
        self.source_statement(range, Statement::Assign { name, value })
    }

    fn parse_for_range(&mut self) -> ParseResult<Option<(Statement, Expr, Statement)>> {
        let save = self.pos;
        let header_start = self.current_start();
        let syntax_checkpoint = self.syntax_checkpoint();
        if let Some(var_token) = self.tokens.get(self.pos).cloned()
            && matches!(&var_token.kind, TokenKind::Ident(_))
            && self.peek_n(1, TokenKind::In)
            && self.peek_ident_n(2, "range")
        {
            let Token {
                kind: TokenKind::Ident(var),
                range: var_range,
                ..
            } = var_token
            else {
                unreachable!("the let-chain established an identifier token")
            };
            let init_owner = self.begin_node(AstNodeKind::Statement, header_start);
            self.bump();
            // The range syntax lowers to a direct `let` binding in the AST, so
            // its parser fact must carry the same binding role consumed by HIR.
            self.record_binding(
                init_owner,
                0,
                var.clone(),
                var_range,
                BindingFactKind::Local,
            );
            self.bump(); // in
            self.bump(); // range
            self.expect(TokenKind::LParen)?;
            let end = self.parse_expr()?;
            self.expect(TokenKind::RParen)?;
            if !matches!(end.kind(), Expr::IntLiteral(value) if !value.is_negative()) {
                return Err(self.coded_error(
                    self.tokens[self.pos.saturating_sub(1)].clone(),
                    "E_UNBOUNDED_LOOP",
                    "numeric range bounds must be non-negative integer literals",
                ));
            }
            let range = TextRange::new(header_start, self.previous_end(header_start));
            let zero = self.source_expression(
                AstNodeKind::Expression,
                range,
                Expr::IntLiteral(BigInt::zero()),
            );
            let init = self.finish_owned_statement(
                init_owner,
                range,
                Statement::Let {
                    mutable: true,
                    pat: Pattern::Name(var.clone()),
                    ty: None,
                    value: zero,
                },
            );
            let left =
                self.source_expression(AstNodeKind::Expression, range, Expr::Ident(var.clone()));
            let cond = self.source_expression(
                AstNodeKind::Expression,
                range,
                Expr::Binary {
                    op: BinaryOp::Lt,
                    left: Box::new(left),
                    right: Box::new(end),
                },
            );
            let step = self.inc_statement(var.clone(), range);
            return Ok(Some((init, cond, step)));
        }
        self.pos = save;
        self.syntax_rollback(syntax_checkpoint);
        Ok(None)
    }

    fn parse_expr(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let expression = self.parse_conditional()?;
        let end = self.previous_end(start);
        let range = TextRange::new(start, end);
        if expression
            .source()
            .is_some_and(|source| source.range == range)
        {
            Ok(expression)
        } else {
            Ok(self.source_expression(AstNodeKind::Expression, range, expression))
        }
    }

    fn parse_expr_before_block(&mut self) -> ParseResult<Expr> {
        let previous = std::mem::replace(&mut self.allow_struct_literals, false);
        let result = self.parse_expr();
        self.allow_struct_literals = previous;
        result
    }

    fn parse_conditional(&mut self) -> ParseResult<Expr> {
        enum Frame {
            Then {
                start: u32,
                condition: Expr,
            },
            Else {
                start: u32,
                condition: Expr,
                then_expr: Expr,
            },
        }

        let mut frames = Vec::new();
        let mut current = self.parse_logical_or()?;
        loop {
            if self.peek(TokenKind::Question) && self.question_starts_ternary() {
                self.bump();
                let start = current
                    .source()
                    .map_or_else(|| self.current_start(), |source| source.range.start);
                frames.push(Frame::Then {
                    start,
                    condition: current,
                });
                current = self.parse_logical_or()?;
                continue;
            }

            match frames.pop() {
                Some(Frame::Then { start, condition }) => {
                    self.expect(TokenKind::Colon)?;
                    frames.push(Frame::Else {
                        start,
                        condition,
                        then_expr: current,
                    });
                    current = self.parse_logical_or()?;
                }
                Some(Frame::Else {
                    start,
                    condition,
                    then_expr,
                }) => {
                    current = self.source_expression_from(
                        start,
                        Expr::Conditional {
                            cond: Box::new(condition),
                            then_expr: Box::new(then_expr),
                            else_expr: Box::new(current),
                        },
                    );
                }
                None => return Ok(current),
            }
        }
    }

    fn parse_logical_or(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let mut expr = self.parse_logical_and()?;
        loop {
            if self.peek(TokenKind::OrOr) {
                self.bump();
                let rhs = self.parse_logical_and()?;
                expr = self.source_expression_from(
                    start,
                    Expr::Binary {
                        op: BinaryOp::Or,
                        left: Box::new(expr),
                        right: Box::new(rhs),
                    },
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let mut expr = self.parse_comparison()?;
        loop {
            if self.peek(TokenKind::AndAnd) {
                self.bump();
                let rhs = self.parse_comparison()?;
                expr = self.source_expression_from(
                    start,
                    Expr::Binary {
                        op: BinaryOp::And,
                        left: Box::new(expr),
                        right: Box::new(rhs),
                    },
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let mut expr = self.parse_term()?;
        loop {
            let op = if self.peek(TokenKind::EqualEqual) {
                self.bump();
                Some(BinaryOp::Eq)
            } else if self.peek(TokenKind::BangEqual) {
                self.bump();
                Some(BinaryOp::Ne)
            } else if self.peek(TokenKind::LessEqual) {
                self.bump();
                Some(BinaryOp::Le)
            } else if self.peek(TokenKind::Less) {
                self.bump();
                Some(BinaryOp::Lt)
            } else if self.peek(TokenKind::GreaterEqual) {
                self.bump();
                Some(BinaryOp::Ge)
            } else if self.peek(TokenKind::Greater) {
                self.bump();
                Some(BinaryOp::Gt)
            } else {
                None
            };
            if let Some(op) = op {
                let rhs = self.parse_term()?;
                expr = self.source_expression_from(
                    start,
                    Expr::Binary {
                        op,
                        left: Box::new(expr),
                        right: Box::new(rhs),
                    },
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let mut expr = self.parse_factor()?;
        loop {
            let op = if self.peek(TokenKind::Plus) {
                self.bump();
                Some(BinaryOp::Add)
            } else if self.peek(TokenKind::Minus) {
                self.bump();
                Some(BinaryOp::Sub)
            } else {
                None
            };
            if let Some(op) = op {
                let rhs = self.parse_factor()?;
                expr = self.source_expression_from(
                    start,
                    Expr::Binary {
                        op,
                        left: Box::new(expr),
                        right: Box::new(rhs),
                    },
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> ParseResult<Expr> {
        let start = self.current_start();
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.peek(TokenKind::Star) {
                self.bump();
                Some(BinaryOp::Mul)
            } else if self.peek(TokenKind::Slash) {
                self.bump();
                Some(BinaryOp::Div)
            } else if self.peek(TokenKind::Percent) {
                self.bump();
                Some(BinaryOp::Mod)
            } else {
                None
            };
            if let Some(op) = op {
                let rhs = self.parse_unary()?;
                expr = self.source_expression_from(
                    start,
                    Expr::Binary {
                        op,
                        left: Box::new(expr),
                        right: Box::new(rhs),
                    },
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        let mut prefixes: Vec<(UnaryOp, Token)> = Vec::new();
        loop {
            if self.peek(TokenKind::Minus) {
                let minus = self.bump();
                if let Some(token) = self.tokens.get(self.pos).cloned()
                    && let TokenKind::Number(spelling) = token.kind.clone()
                {
                    self.bump();
                    let value = parse_integer_value(&spelling, true).map_err(|_| {
                        self.coded_error(
                            token.clone(),
                            "E_INT_LITERAL_OVERFLOW",
                            "integer literal is outside the signed Kotodama int domain",
                        )
                    })?;
                    let expr =
                        self.source_expression_from(minus.range.start, bigint_literal_expr(value));
                    let mut expr = self.parse_postfix(expr, minus.range.start)?;
                    for (op, token) in prefixes.into_iter().rev() {
                        expr = self.source_expression_from(
                            token.range.start,
                            Expr::Unary {
                                op,
                                expr: Box::new(expr),
                            },
                        );
                    }
                    return Ok(expr);
                }
                if let Some(token) = self.tokens.get(self.pos).cloned()
                    && let TokenKind::DecimalLiteral(spelling) = token.kind.clone()
                {
                    self.bump();
                    let range = TextRange::new(minus.range.start, token.range.end);
                    let node = self.facts.source_map.allocate_owned(
                        AstNodeKind::DecimalLiteral,
                        range,
                        self.current_function,
                    );
                    let expr = Expr::Source {
                        node,
                        source: SourceRange::new(self.facts.source_map.source(), range),
                        expression: Box::new(Expr::DecimalLiteral(format!("-{spelling}"))),
                    };
                    let mut expr = self.parse_postfix(expr, minus.range.start)?;
                    for (op, token) in prefixes.into_iter().rev() {
                        expr = self.source_expression_from(
                            token.range.start,
                            Expr::Unary {
                                op,
                                expr: Box::new(expr),
                            },
                        );
                    }
                    return Ok(expr);
                }
                prefixes.push((UnaryOp::Neg, minus));
            } else if self.peek(TokenKind::Bang) {
                prefixes.push((UnaryOp::Not, self.bump()));
            } else {
                break;
            }
        }

        let postfix_start = self.current_start();
        let primary = self.parse_primary()?;
        let mut expr = self.parse_postfix(primary, postfix_start)?;
        for (op, token) in prefixes.into_iter().rev() {
            expr = self.source_expression_from(
                token.range.start,
                Expr::Unary {
                    op,
                    expr: Box::new(expr),
                },
            );
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self, mut expr: Expr, expression_start: u32) -> ParseResult<Expr> {
        loop {
            if self.peek(TokenKind::Dot) {
                self.bump();
                // Accept `ident` or numeric tuple index after '.'
                let (field, field_token) = if let Some(token) = self.tokens.get(self.pos).cloned() {
                    match token.kind.clone() {
                        TokenKind::Ident(s) => {
                            self.bump();
                            (s, Some(token))
                        }
                        TokenKind::Number(n) => {
                            self.bump();
                            let index = self.number_to_usize(&token, &n, "tuple index")?;
                            (index.to_string(), None)
                        }
                        _ => {
                            // Avoid borrowing self immutably and mutably in a single expression
                            let tok = self.bump();
                            return Err(self.error(tok, "identifier or tuple index"));
                        }
                    }
                } else {
                    let tok = self.bump();
                    return Err(self.error(tok, "identifier or tuple index"));
                };
                // Method-call sugar: `expr.method(args...)` -> `Call { name: method, args: [expr, args...] }`
                if self.peek(TokenKind::LParen) {
                    if let Some(token) = field_token.as_ref()
                        && let Some(message) = removed_method_helper_message(&field)
                    {
                        return Err(self.coded_error(
                            token.clone(),
                            removed_method_helper_code(&field),
                            message,
                        ));
                    }
                    self.bump();
                    let parameter_names = self.call_parameter_names(&field, true);
                    let ParsedCallArguments {
                        mut args,
                        argument_names,
                        ..
                    } = self.parse_call_arguments(parameter_names.as_deref())?;
                    self.expect(TokenKind::RParen)?;
                    // Prepend the receiver as the first argument
                    let mut full_args = Vec::with_capacity(args.len() + 1);
                    full_args.push(expr);
                    full_args.append(&mut args);
                    if let Some(token) = field_token.as_ref() {
                        let call_end = self.previous_end(token.range.end);
                        let (node, source) = self.record_call(
                            field.clone(),
                            token.range,
                            TextRange::new(expression_start, call_end),
                            true,
                        );
                        let call_name = match field.as_str() {
                            "get" => STATE_MAP_GET_INTRINSIC.to_owned(),
                            _ => field,
                        };
                        expr = Expr::Source {
                            node,
                            source,
                            expression: Box::new(Expr::Call {
                                name: call_name,
                                args: full_args,
                                argument_names,
                                implicit_receiver: true,
                            }),
                        };
                        continue;
                    }
                    let call_name = match field.as_str() {
                        "get" => STATE_MAP_GET_INTRINSIC.to_owned(),
                        _ => field,
                    };
                    expr = self.source_expression_from(
                        expression_start,
                        Expr::Call {
                            name: call_name,
                            args: full_args,
                            argument_names,
                            implicit_receiver: true,
                        },
                    );
                } else {
                    expr = self.source_expression_from(
                        expression_start,
                        Expr::Member {
                            object: Box::new(expr),
                            field,
                        },
                    );
                }
            } else if self.peek(TokenKind::LBracket) {
                self.bump();
                let idx = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                let range = TextRange::new(expression_start, self.previous_end(expression_start));
                let node = self.facts.source_map.allocate_owned(
                    AstNodeKind::IndexExpression,
                    range,
                    self.current_function,
                );
                expr = Expr::Source {
                    node,
                    source: SourceRange::new(self.facts.source_map.source(), range),
                    expression: Box::new(Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(idx),
                    }),
                };
            } else if self.peek(TokenKind::Question) && !self.question_starts_ternary() {
                self.bump();
                expr =
                    self.source_expression_from(expression_start, Expr::Propagate(Box::new(expr)));
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        let tok = self.bump();
        let expression = match &tok.kind {
            TokenKind::True => Expr::Bool(true),
            TokenKind::False => Expr::Bool(false),
            TokenKind::Number(spelling) => {
                bigint_literal_expr(parse_integer_value(spelling, false).map_err(|_| {
                    self.coded_error(
                        tok.clone(),
                        "E_INT_LITERAL_OVERFLOW",
                        "integer literal is outside the signed Kotodama int domain",
                    )
                })?)
            }
            TokenKind::DecimalLiteral(spelling) => {
                let range = tok.range;
                let node = self.facts.source_map.allocate_owned(
                    AstNodeKind::DecimalLiteral,
                    range,
                    self.current_function,
                );
                Expr::Source {
                    node,
                    source: SourceRange::new(self.facts.source_map.source(), range),
                    expression: Box::new(Expr::DecimalLiteral(spelling.clone())),
                }
            }
            TokenKind::String(s) => Expr::String(s.clone()),
            TokenKind::Bytes(bytes) => Expr::Bytes(bytes.clone()),
            TokenKind::Ident(name) => self.parse_named_primary(tok.clone(), name.clone())?,
            TokenKind::If => {
                self.pos = self.pos.saturating_sub(1);
                self.parse_if_expression()?
            }
            TokenKind::Match => {
                self.pos = self.pos.saturating_sub(1);
                self.parse_match_expression()?
            }
            TokenKind::State if self.peek(TokenKind::ColonColon) => {
                self.parse_named_primary(tok.clone(), "state".to_owned())?
            }
            TokenKind::LParen => self.parse_parenthesized(tok.clone())?,
            TokenKind::LBracket => self.parse_list_expression(tok.clone())?,
            _ => {
                // An absent expression has no single punctuation token to
                // name, but the lossless CST still needs a concrete,
                // zero-width recovery token at the failed primary.  An
                // identifier is the canonical side-effect-free expression
                // placeholder and, unlike deriving recovery from diagnostic
                // prose, keeps editor recovery stable when messages change.
                let mut error = self.error(tok, "expression");
                error.expected = Some(SyntaxKind::Ident);
                error.expected_owner = self.syntax.current();
                return Err(error);
            }
        };
        let range = TextRange::new(tok.range.start, self.previous_end(tok.range.end));
        if expression
            .source()
            .is_some_and(|source| source.range == range)
        {
            Ok(expression)
        } else {
            Ok(self.source_expression(AstNodeKind::Expression, range, expression))
        }
    }

    fn parse_list_expression(&mut self, opening: Token) -> ParseResult<Expr> {
        let start = opening.range.start;
        let syntax_list = self.syntax_start(SyntaxKind::ListExpr, start);
        let result = self.parse_list_expression_inner(opening);
        if result
            .as_ref()
            .is_ok_and(|expression| matches!(expression.kind(), Expr::ListComprehension { .. }))
        {
            self.syntax_set_kind(syntax_list, SyntaxKind::ListComprehension);
        }
        self.syntax_finish(syntax_list, start);
        result
    }
    fn parse_list_expression_inner(&mut self, opening: Token) -> ParseResult<Expr> {
        if self.peek(TokenKind::RBracket) {
            self.bump();
            return Ok(Expr::List(Vec::new()));
        }

        let first = self.parse_expr()?;
        if self.peek(TokenKind::For) {
            self.bump();
            let owner = self.begin_node(AstNodeKind::ListComprehension, opening.range.start);
            let (item, item_token) = self.expect_ident_token()?;
            self.record_binding(
                owner,
                0,
                item.clone(),
                item_token.range,
                BindingFactKind::Comprehension,
            );
            self.expect(TokenKind::In)?;
            let source = self.parse_expr()?;
            let condition = if self.peek(TokenKind::If) {
                self.bump();
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            self.expect(TokenKind::RBracket)?;
            let range = TextRange::new(opening.range.start, self.previous_end(opening.range.end));
            let expression = Expr::ListComprehension {
                expression: Box::new(first),
                item,
                source: Box::new(source),
                condition,
            };
            return Ok(self.finish_owned_expression(
                owner,
                AstNodeKind::ListComprehension,
                range,
                expression,
            ));
        }

        let mut elements = vec![first];
        while self.peek(TokenKind::Comma) {
            self.bump();
            if self.peek(TokenKind::RBracket) {
                break;
            }
            elements.push(self.parse_expr()?);
        }
        self.expect(TokenKind::RBracket)?;
        Ok(Expr::List(elements))
    }

    fn parse_parenthesized(&mut self, opening: Token) -> ParseResult<Expr> {
        let mut openings = vec![opening];
        while self.peek(TokenKind::LParen) {
            openings.push(self.bump());
        }
        if self.peek(TokenKind::RParen) {
            let opening = openings.last().expect("at least one opening parenthesis");
            let closing = self.bump();
            let line_text = self
                .source
                .lines()
                .nth(opening.line.saturating_sub(1))
                .unwrap_or("");
            let caret = " ".repeat(opening.column.saturating_sub(1)) + "^";
            return Err(Box::new(ParseError {
                code: "K1001",
                message: "source-level unit value `()` is not part of Kotodama V1; omit a return value instead"
                    .into(),
                line: opening.line,
                column: opening.column,
                snippet: format!("{line_text}\n{caret}"),
                range: TextRange::new(opening.range.start, closing.range.end),
                fix: None,
                expected: None,
                expected_owner: None,
            }));
        }

        let mut expression = self.parse_expr()?;
        for _ in openings.iter().rev() {
            if self.peek(TokenKind::Comma) {
                let mut elements = vec![expression];
                while self.peek(TokenKind::Comma) {
                    self.bump();
                    elements.push(self.parse_expr()?);
                }
                expression = Expr::Tuple(elements);
            }
            self.expect(TokenKind::RParen)?;
        }
        Ok(expression)
    }

    fn parse_named_primary(&mut self, ident_token: Token, mut name: String) -> ParseResult<Expr> {
        // Keyword tokens stay reserved as bindings and declarations. Canonical
        // V1 capability paths that intentionally use branded keywords admit
        // them only after `::`; they never become ordinary identifiers.
        while self.peek(TokenKind::ColonColon) {
            self.bump();
            let segment = self.expect_namespace_segment()?;
            name.push_str("::");
            name.push_str(&segment);
        }
        let name_end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or(ident_token.range.end, |token| token.range.end);
        let name_range = TextRange::new(ident_token.range.start, name_end);
        if name == "json" {
            if self.peek(TokenKind::LBrace) {
                return self.parse_json_object(ident_token.range.start);
            }
            if self.peek(TokenKind::LBracket) {
                return self.parse_json_array(ident_token.range.start);
            }
        }
        if self.peek(TokenKind::Bang) {
            return Err(self.error(
                ident_token,
                "macros are not part of Kotodama V1; use an ordinary typed constructor such as `AccountId::parse(\"...\")`, `Json::parse(\"{...}\")`, or a `b\"...\"` bytes literal",
            ));
        }
        if matches!(
            name.as_str(),
            "option::some" | "option::none" | "result::ok" | "result::err"
        ) && self.peek(TokenKind::LParen)
        {
            self.bump();
            let parameter_names = self.call_parameter_names(&name, false);
            let parsed = self.parse_call_arguments(parameter_names.as_deref())?;
            self.expect(TokenKind::RParen)?;
            let end = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map_or(ident_token.range.end, |token| token.range.end);
            let range = TextRange::new(ident_token.range.start, end);
            let replacement = self.legacy_sum_replacement(&name, &parsed);
            let mut error = self.coded_error(
                ident_token,
                "E_LEGACY_SUM_CONSTRUCTOR",
                format!(
                    "`{name}` is retired; use the canonical active-only `Option`/`Result` constructor"
                ),
            );
            error.range = range;
            error.fix = replacement;
            return Err(error);
        }
        if name == "Option::none" {
            if self.peek(TokenKind::LParen) {
                let opening = self.bump();
                let parameter_names = self.call_parameter_names(&name, false);
                let parsed = self.parse_call_arguments(parameter_names.as_deref())?;
                self.expect(TokenKind::RParen)?;
                let end = self
                    .tokens
                    .get(self.pos.saturating_sub(1))
                    .map_or(opening.range.end, |token| token.range.end);
                let mut error = self.coded_error(
                    opening,
                    "E_SUM_CONSTRUCTOR_FORM",
                    "`Option::none` is a contextual value path; remove the parentheses and inactive placeholder",
                );
                error.range = TextRange::new(ident_token.range.start, end);
                if parsed.argument_names.is_none() {
                    error.fix = Some("Option::none".into());
                }
                return Err(error);
            }
            return Ok(Expr::OptionNone);
        }
        if matches!(name.as_str(), "Option::some" | "Result::ok" | "Result::err") {
            if !self.peek(TokenKind::LParen) {
                return Err(self.error(
                    ident_token,
                    "constructor call with exactly one active payload",
                ));
            }
            self.bump();
            let parameter_names = self.call_parameter_names(&name, false);
            let ParsedCallArguments {
                mut args,
                argument_names,
                ..
            } = self.parse_call_arguments(parameter_names.as_deref())?;
            self.expect(TokenKind::RParen)?;
            if argument_names.is_some() || args.len() != 1 {
                let error = self.coded_error(
                    ident_token,
                    "E_SUM_CONSTRUCTOR_ARITY",
                    format!("`{name}` expects exactly one positional active payload"),
                );
                return Err(error);
            }
            let payload = Box::new(args.pop().expect("one constructor argument"));
            return Ok(match name.as_str() {
                "Option::some" => Expr::OptionSome(payload),
                "Result::ok" => Expr::ResultOk(payload),
                "Result::err" => Expr::ResultErr(payload),
                _ => unreachable!("matched canonical constructor"),
            });
        }
        if self.allow_struct_literals && self.peek(TokenKind::LBrace) {
            let start = ident_token.range.start;
            let syntax_literal = self.syntax_start(SyntaxKind::StructLiteral, start);
            self.bump();
            let result = (|| -> ParseResult<Expr> {
                let fields = self.parse_struct_literal_fields()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Expr::StructLiteral { name, fields })
            })();
            self.syntax_finish(syntax_literal, start);
            result
        } else if self.peek(TokenKind::LParen) {
            if let Some(message) = removed_free_helper_message(&name) {
                return Err(self.coded_error(
                    ident_token,
                    removed_free_helper_code(&name),
                    message,
                ));
            }
            self.bump();
            let parameter_names = self.call_parameter_names(&name, false);
            let ParsedCallArguments {
                args,
                argument_names,
                ..
            } = self.parse_call_arguments(parameter_names.as_deref())?;
            self.expect(TokenKind::RParen)?;
            let call_end = self.previous_end(name_range.end);
            let (node, source) = self.record_call(
                name.clone(),
                name_range,
                TextRange::new(ident_token.range.start, call_end),
                false,
            );
            Ok(Expr::Source {
                node,
                source,
                expression: Box::new(Expr::Call {
                    name,
                    args,
                    argument_names,
                    implicit_receiver: false,
                }),
            })
        } else {
            Ok(Expr::Ident(name))
        }
    }

    fn parse_json_object(&mut self, start: u32) -> ParseResult<Expr> {
        self.with_syntax(
            SyntaxKind::JsonObjectExpr,
            start,
            Self::parse_json_object_inner,
        )
    }

    fn parse_json_object_inner(&mut self) -> ParseResult<Expr> {
        self.expect(TokenKind::LBrace)?;
        let mut entries = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let entry_start = self.current_start();
            let entry = self.with_syntax(SyntaxKind::JsonObjectEntry, entry_start, |this| {
                let key_token = this.bump();
                let key = match &key_token.kind {
                    TokenKind::Ident(key) | TokenKind::String(key) => key.clone(),
                    _ => {
                        return Err(this.error(
                            key_token,
                            "JSON object key as an identifier or quoted string",
                        ));
                    }
                };
                let key_spelling = this
                    .source
                    .get(key_token.range.start as usize..key_token.range.end as usize)
                    .unwrap_or_default()
                    .to_owned();
                this.expect(TokenKind::Colon)?;
                let value = this.parse_expr()?;
                Ok(crate::ast::JsonObjectEntry {
                    key,
                    key_spelling,
                    key_range: key_token.range,
                    value,
                })
            })?;
            entries.push(crate::ast::JsonObjectEntry {
                key: entry.key,
                key_spelling: entry.key_spelling,
                key_range: entry.key_range,
                value: entry.value,
            });
            if !self.peek(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Expr::JsonObject(entries))
    }

    fn parse_json_array(&mut self, start: u32) -> ParseResult<Expr> {
        self.with_syntax(
            SyntaxKind::JsonArrayExpr,
            start,
            Self::parse_json_array_inner,
        )
    }

    fn parse_json_array_inner(&mut self) -> ParseResult<Expr> {
        self.expect(TokenKind::LBracket)?;
        let mut elements = Vec::new();
        while !self.peek(TokenKind::RBracket) && !self.peek(TokenKind::EOF) {
            elements.push(self.parse_expr()?);
            if !self.peek(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RBracket)?;
        Ok(Expr::JsonArray(elements))
    }

    fn call_parameter_names(&self, name: &str, implicit_receiver: bool) -> Option<Vec<String>> {
        if !implicit_receiver {
            if let Some(parameters) = self.declared_function_parameters.get(name) {
                return parameters.clone();
            }
            if let Some(builtin) = crate::builtins::Builtin::from_source_name(name) {
                return Some(
                    builtin
                        .signature()
                        .parameter_names
                        .iter()
                        .map(|name| (*name).to_owned())
                        .collect(),
                );
            }
        }

        let parameters: &[&str] = match name {
            "Option::some"
            | "option::some"
            | "decimal::from_int"
            | "decimal::to_int_exact"
            | "decimal::to_int_trunc"
            | "quantity::try_from_int"
            | "quantity::try_from_decimal"
            | "decimal::from_quantity"
            | "try_push"
            | "contains" => &["value"],
            "Result::ok" | "result::ok" => &["value"],
            "Result::err" | "result::err" => &["error"],
            "get" => &["index"],
            "try_set" => &["index", "value"],
            "take" => &["limit"],
            "div_round" => &["divisor", "scale", "mode"],
            "ratio_round" => &["divisor", "scale", "mode"],
            "decimal::to_int_round" => &["value", "mode"],
            "get_bool" | "get_int" | "get_decimal" | "get_quantity" | "get_string"
            | "get_bytes" | "get_json" => &["key"],
            _ => return None,
        };
        Some(parameters.iter().map(|name| (*name).to_owned()).collect())
    }

    fn parse_call_arguments(
        &mut self,
        parameter_names: Option<&[String]>,
    ) -> ParseResult<ParsedCallArguments> {
        let start = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map_or_else(|| self.current_start(), |token| token.range.start);
        let syntax_arguments = self.syntax_start(SyntaxKind::ArgumentList, start);
        let result = self.parse_call_arguments_inner(parameter_names);
        let end = self
            .tokens
            .get(self.pos)
            .filter(|token| matches!(token.kind, TokenKind::RParen))
            .map_or_else(|| self.previous_end(start), |token| token.range.end);
        self.syntax_finish_at(syntax_arguments, end);
        result
    }

    fn parse_call_arguments_inner(
        &mut self,
        parameter_names: Option<&[String]>,
    ) -> ParseResult<ParsedCallArguments> {
        let mut args = Vec::new();
        let mut names = Vec::new();
        let mut ranges: Vec<TextRange> = Vec::new();
        let mut named_mode = None;
        while !self.peek(TokenKind::RParen) {
            let is_named = matches!(
                self.tokens.get(self.pos).map(|token| &token.kind),
                Some(TokenKind::Ident(_) | TokenKind::Kotoage)
            ) && self.peek_n(1, TokenKind::Colon);
            if let Some(expected_named) = named_mode
                && expected_named != is_named
            {
                let token = self.bump();
                let mut error = self.coded_error(
                    token.clone(),
                    "E_MIXED_CALL_ARGUMENTS",
                    "calls must use either all positional or all named source arguments",
                );
                if let Some(parameter_names) = parameter_names {
                    if is_named {
                        let current_name = match &token.kind {
                            TokenKind::Ident(name) => name.as_str(),
                            TokenKind::Kotoage => "kotoage",
                            _ => unreachable!(
                                "named argument lookahead requires an identifier or contextual kotoage"
                            ),
                        };
                        let positional_names = parameter_names.get(..args.len());
                        if let Some(positional_names) = positional_names
                            && !positional_names.iter().any(|name| name == current_name)
                            && let (Some(first), Some(last)) = (ranges.first(), ranges.last())
                        {
                            let range = TextRange::new(first.start, last.end);
                            let mut replacement = String::new();
                            let mut cursor = range.start as usize;
                            for (argument, parameter) in ranges.iter().zip(positional_names) {
                                let start = argument.start as usize;
                                replacement.push_str(&self.source[cursor..start]);
                                replacement.push_str(parameter);
                                replacement.push_str(": ");
                                cursor = start;
                            }
                            replacement.push_str(&self.source[cursor..range.end as usize]);
                            error.range = range;
                            error.fix = Some(replacement);
                        }
                    } else if let Some(parameter) = parameter_names.get(args.len())
                        && !names.iter().any(|name| name == parameter)
                    {
                        error.range = TextRange::empty(token.range.start);
                        error.fix = Some(format!("{parameter}: "));
                    }
                }
                return Err(error);
            }
            named_mode = Some(is_named);
            let argument_start = self
                .tokens
                .get(self.pos)
                .map_or(0, |token| token.range.start);
            let syntax_named =
                is_named.then(|| self.syntax_start(SyntaxKind::NamedArgument, argument_start));
            let parsed_argument = (|| -> ParseResult<(Option<String>, Expr)> {
                let name = if is_named {
                    let token = self.bump();
                    let name = match token.kind.clone() {
                        TokenKind::Ident(name) => name,
                        TokenKind::Kotoage => "kotoage".to_owned(),
                        _ => unreachable!(
                            "named argument lookahead requires an identifier or contextual kotoage"
                        ),
                    };
                    if names.contains(&name) {
                        return Err(self.coded_error(
                            token,
                            "E_DUPLICATE_NAMED_ARGUMENT",
                            format!("named argument `{name}` is supplied more than once"),
                        ));
                    }
                    self.expect(TokenKind::Colon)?;
                    Some(name)
                } else {
                    None
                };
                Ok((name, self.parse_expr()?))
            })();
            if let Some(syntax_named) = syntax_named {
                self.syntax_finish(syntax_named, argument_start);
            }
            let (name, argument) = parsed_argument?;
            if let Some(name) = name {
                names.push(name);
            }
            args.push(argument);
            let argument_end = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map_or(argument_start, |token| token.range.end);
            ranges.push(TextRange::new(argument_start, argument_end));
            if !self.peek(TokenKind::Comma) {
                break;
            }
            self.bump();
            if self.peek(TokenKind::RParen) {
                break;
            }
        }
        Ok(ParsedCallArguments {
            args,
            argument_names: named_mode.unwrap_or(false).then_some(names),
            ranges,
        })
    }

    fn legacy_sum_replacement(&self, name: &str, parsed: &ParsedCallArguments) -> Option<String> {
        if parsed.argument_names.is_some() {
            return None;
        }
        let source_argument = |index: usize| {
            let range = parsed.ranges.get(index)?;
            self.source
                .get(range.start as usize..range.end as usize)
                .map(str::trim)
                .filter(|text| !text.contains("//") && !text.contains("/*"))
        };
        match name {
            "option::some" if parsed.args.len() == 1 => {
                Some(format!("Option::some({})", source_argument(0)?))
            }
            "option::none" if parsed.args.len() == 1 => Some("Option::none".into()),
            "result::ok" if parsed.args.len() == 2 => {
                Some(format!("Result::ok({})", source_argument(0)?))
            }
            "result::err" if parsed.args.len() == 2 => {
                Some(format!("Result::err({})", source_argument(1)?))
            }
            _ => None,
        }
    }

    fn parse_struct_literal_fields(&mut self) -> ParseResult<Vec<StructLiteralField>> {
        let mut fields = Vec::<StructLiteralField>::new();
        while !self.peek(TokenKind::RBrace) {
            let field_start = self.current_start();
            let field = self.with_syntax(SyntaxKind::StructLiteralField, field_start, |this| {
                let token = this.bump();
                let TokenKind::Ident(name) = token.kind.clone() else {
                    return Err(this.error(token, "named struct field"));
                };
                if fields.iter().any(|field| field.name == name) {
                    return Err(this.coded_error(
                        token,
                        "E_DUPLICATE_STRUCT_FIELD",
                        format!("struct field `{name}` is supplied more than once"),
                    ));
                }
                let (value, shorthand) = if this.peek(TokenKind::Colon) {
                    this.bump();
                    (this.parse_expr()?, false)
                } else {
                    (
                        this.source_expression(
                            AstNodeKind::Expression,
                            token.range,
                            Expr::Ident(name.clone()),
                        ),
                        true,
                    )
                };
                Ok(StructLiteralField {
                    name,
                    value,
                    shorthand,
                })
            })?;
            fields.push(field);
            if !self.peek(TokenKind::Comma) {
                break;
            }
            self.bump();
            if self.peek(TokenKind::RBrace) {
                break;
            }
        }
        Ok(fields)
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        self.expect_ident_token().map(|(name, _)| name)
    }

    fn expect_ident_token(&mut self) -> ParseResult<(String, Token)> {
        let tok = self.bump();
        match &tok.kind {
            TokenKind::Ident(name) => Ok((name.clone(), tok.clone())),
            _ => {
                let mut error = self.error(tok, "identifier");
                error.expected = Some(SyntaxKind::Ident);
                Err(error)
            }
        }
    }

    /// Admit the reserved `trigger` spelling only where trigger-filter grammar
    /// defines it as a data-family or matcher keyword.
    fn expect_trigger_context_ident(&mut self) -> ParseResult<String> {
        let tok = self.bump();
        match &tok.kind {
            TokenKind::Ident(name) => Ok(name.clone()),
            TokenKind::Trigger => Ok("trigger".to_owned()),
            _ => Err(self.error(tok, "trigger-filter identifier")),
        }
    }

    fn expect_namespace_segment(&mut self) -> ParseResult<String> {
        let tok = self.bump();
        match &tok.kind {
            TokenKind::Ident(name) => Ok(name.clone()),
            TokenKind::Trigger if self.peek(TokenKind::ColonColon) => Ok("trigger".to_owned()),
            TokenKind::Seiyaku if self.peek(TokenKind::ColonColon) => Ok("seiyaku".to_owned()),
            TokenKind::Kotoage => Ok("kotoage".to_owned()),
            _ => {
                let mut error = self.error(tok, "namespace segment");
                error.expected = Some(SyntaxKind::Ident);
                Err(error)
            }
        }
    }

    fn parse_type_expr(&mut self) -> ParseResult<TypeExpr> {
        let start = self.current_start();
        let ty = self.parse_type_expr_inner()?;
        let end = self.previous_end(start);
        let node = self.facts.source_map.allocate_owned(
            AstNodeKind::Type,
            TextRange::new(start, end),
            self.current_function,
        );
        Ok(TypeExpr::Source {
            node,
            source: SourceRange::new(self.facts.source_map.source(), TextRange::new(start, end)),
            ty: Box::new(ty),
        })
    }

    fn parse_type_expr_inner(&mut self) -> ParseResult<TypeExpr> {
        enum Frame {
            Generic {
                start: u32,
                base: String,
                args: Vec<TypeExpr>,
            },
            Tuple {
                opening: Token,
                args: Vec<TypeExpr>,
            },
        }

        let mut frames = Vec::new();
        'next_type: loop {
            let mut current = loop {
                if self.peek(TokenKind::LParen) {
                    let opening = self.bump();
                    if self.peek(TokenKind::RParen) {
                        let closing = self.bump();
                        return Err(self.tuple_type_arity_error(&opening, &closing));
                    }
                    frames.push(Frame::Tuple {
                        opening,
                        args: Vec::new(),
                    });
                    continue;
                }

                if let Some(Token {
                    kind: TokenKind::Number(value),
                    ..
                }) = self.tokens.get(self.pos).cloned()
                {
                    let token = self.bump();
                    let value = parse_bounded_unsigned(&value, u64::MAX).map_err(|_| {
                        self.range_error(
                            &token,
                            "compile-time integer type argument is outside the u64 range"
                                .to_owned(),
                        )
                    })?;
                    break self.source_type(token.range, TypeExpr::Const(value));
                }

                let (base, base_token) = self.expect_ident_token()?;
                if let Some(replacement) = retired_numeric_type_replacement(&base) {
                    let replacement_message = replacement.map_or_else(
                        || {
                            "use `int`, `decimal`, or `quantity` according to the value's domain"
                                .to_owned()
                        },
                        |replacement| format!("use `{replacement}`"),
                    );
                    let mut error = self.coded_error(
                        base_token.clone(),
                        "E_RETIRED_NUMERIC_TYPE",
                        format!(
                            "numeric type `{base}` is not part of Kotodama V1; {replacement_message}"
                        ),
                    );
                    error.fix = replacement.map(str::to_owned);
                    if self.recover {
                        self.errors.push(*error);
                    } else {
                        return Err(error);
                    }
                }
                self.record_type_use(base.clone(), base_token.range);
                if self.peek(TokenKind::Less) {
                    self.bump();
                    if self.peek(TokenKind::Greater) {
                        self.bump();
                        let range = TextRange::new(
                            base_token.range.start,
                            self.previous_end(base_token.range.end),
                        );
                        break self.source_type(
                            range,
                            TypeExpr::Generic {
                                base,
                                args: Vec::new(),
                            },
                        );
                    }
                    frames.push(Frame::Generic {
                        start: base_token.range.start,
                        base,
                        args: Vec::new(),
                    });
                    continue;
                }
                break self.source_type(base_token.range, TypeExpr::Path(base));
            };

            loop {
                let Some(frame) = frames.pop() else {
                    return Ok(current);
                };
                match frame {
                    Frame::Generic {
                        start,
                        base,
                        mut args,
                    } => {
                        args.push(current);
                        if self.peek(TokenKind::Comma) {
                            self.bump();
                            frames.push(Frame::Generic { start, base, args });
                            continue 'next_type;
                        }
                        self.expect(TokenKind::Greater)?;
                        current = self.source_type(
                            TextRange::new(start, self.previous_end(start)),
                            TypeExpr::Generic { base, args },
                        );
                    }
                    Frame::Tuple { opening, mut args } => {
                        args.push(current);
                        if self.peek(TokenKind::Comma) {
                            self.bump();
                            frames.push(Frame::Tuple { opening, args });
                            continue 'next_type;
                        }
                        self.expect(TokenKind::RParen)?;
                        let closing = &self.tokens[self.pos.saturating_sub(1)];
                        if args.len() < 2 {
                            return Err(self.tuple_type_arity_error(&opening, closing));
                        }
                        current = self.source_type(
                            TextRange::new(opening.range.start, closing.range.end),
                            TypeExpr::Tuple(args),
                        );
                    }
                }
            }
        }
    }

    fn tuple_type_arity_error(&self, opening: &Token, closing: &Token) -> Box<ParseError> {
        let line_text = self
            .source
            .lines()
            .nth(opening.line.saturating_sub(1))
            .unwrap_or("");
        let caret = " ".repeat(opening.column.saturating_sub(1)) + "^";
        Box::new(ParseError {
            code: "K1001",
            message: "tuple types require at least two elements; omit the return type for Unit"
                .into(),
            line: opening.line,
            column: opening.column,
            snippet: format!("{line_text}\n{caret}"),
            range: TextRange::new(opening.range.start, closing.range.end),
            fix: None,
            expected: None,
            expected_owner: None,
        })
    }

    fn try_parse_lvalue_expr(&mut self) -> ParseResult<Expr> {
        // Parse an identifier then tail of member/index chains
        let expression_start = self.current_start();
        let (name, name_token) = self.expect_ident_token()?;
        let node = self.facts.source_map.allocate_owned(
            AstNodeKind::Expression,
            name_token.range,
            self.current_function,
        );
        let mut expr = Expr::Source {
            node,
            source: SourceRange::new(self.facts.source_map.source(), name_token.range),
            expression: Box::new(Expr::Ident(name)),
        };
        loop {
            if self.peek(TokenKind::Dot) {
                self.bump();
                let field = if let Some(Token {
                    kind: TokenKind::Ident(s),
                    ..
                }) = self.tokens.get(self.pos)
                {
                    let s = s.clone();
                    self.bump();
                    s
                } else if let Some(token) = self.tokens.get(self.pos).cloned()
                    && let TokenKind::Number(n) = token.kind.clone()
                {
                    self.bump();
                    let index = self.number_to_usize(&token, &n, "tuple index")?;
                    index.to_string()
                } else {
                    let tok = self.bump();
                    return Err(self.error(tok, "identifier or tuple index"));
                };
                let range = TextRange::new(expression_start, self.previous_end(expression_start));
                let node = self.facts.source_map.allocate_owned(
                    AstNodeKind::Expression,
                    range,
                    self.current_function,
                );
                expr = Expr::Source {
                    node,
                    source: SourceRange::new(self.facts.source_map.source(), range),
                    expression: Box::new(Expr::Member {
                        object: Box::new(expr),
                        field,
                    }),
                };
            } else if self.peek(TokenKind::LBracket) {
                self.bump();
                let idx = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                let range = TextRange::new(expression_start, self.previous_end(expression_start));
                let node = self.facts.source_map.allocate_owned(
                    AstNodeKind::IndexExpression,
                    range,
                    self.current_function,
                );
                expr = Expr::Source {
                    node,
                    source: SourceRange::new(self.facts.source_map.source(), range),
                    expression: Box::new(Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(idx),
                    }),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_for_each_map(
        &mut self,
        statement_start: u32,
    ) -> ParseResult<Option<ForEachMapBinding>> {
        // Patterns: (k, v) in <expr>  OR  k in <expr>
        let save = self.pos;
        let syntax_checkpoint = self.syntax_checkpoint();
        if self.peek(TokenKind::LParen) {
            self.bump();
            let (k, key_token) = self.expect_ident_token()?;
            self.expect(TokenKind::Comma)?;
            let (v, value_token) = self.expect_ident_token()?;
            self.expect(TokenKind::RParen)?;
            if self.peek(TokenKind::In) {
                self.bump();
                let owner = self.begin_node(AstNodeKind::Statement, statement_start);
                self.record_binding(
                    owner,
                    0,
                    k.clone(),
                    key_token.range,
                    BindingFactKind::Iterator,
                );
                self.record_binding(
                    owner,
                    1,
                    v.clone(),
                    value_token.range,
                    BindingFactKind::Iterator,
                );
                let map = self.parse_expr()?;
                return Ok(Some((owner, k, Some(v), map)));
            }
        } else if let Some(Token {
            kind: TokenKind::Ident(k),
            range,
            ..
        }) = self.tokens.get(self.pos).cloned()
            && self.peek_n(1, TokenKind::In)
        {
            let owner = self.begin_node(AstNodeKind::Statement, statement_start);
            self.bump();
            self.record_binding(owner, 0, k.clone(), range, BindingFactKind::Iterator);
            self.bump(); // in
            let map = self.parse_expr()?;
            return Ok(Some((owner, k, None, map)));
        }
        self.pos = save;
        self.syntax_rollback(syntax_checkpoint);
        Ok(None)
    }

    fn parse_param_type_annotation(&mut self) -> ParseResult<(bool, TypeExpr)> {
        if self.peek(TokenKind::State) {
            let token = self.bump();
            return Err(self.error(
                token,
                "state handles are not first-class parameters; access declared state directly",
            ));
        }
        let ty = self.parse_type_expr()?;
        Ok((false, ty))
    }

    fn parse_param(&mut self) -> ParseResult<Param> {
        // Canonical V1 form: `Type name`.
        let (is_state, ty) = self.parse_param_type_annotation()?;
        if self.peek(TokenKind::Colon) {
            let token = self.bump();
            return Err(self.coded_error(
                token,
                "E_RETIRED_DECLARATION_ORDER",
                "Kotodama V1 parameters are type-first: write `int value`, not `value: int`",
            ));
        }
        let (name, name_token) = self.expect_ident_token()?;
        let node = self.begin_node(AstNodeKind::Parameter, name_token.range.start);
        self.record_declaration(
            node,
            name.clone(),
            name_token.range,
            DeclarationKind::Parameter,
            self.current_function,
        );
        self.finish_node(node);
        Ok(Param {
            ty: Some(ty),
            name,
            is_state,
        })
    }

    fn expect(&mut self, kind: TokenKind) -> ParseResult<()> {
        let expected = expected_syntax_kind(&kind);
        let tok = self.tokens.get(self.pos).cloned().unwrap_or_else(|| Token {
            kind: TokenKind::EOF,
            line: self.tokens.last().map_or(1, |token| token.line),
            column: self.tokens.last().map_or(1, |token| token.column),
            range: self.tokens.last().map_or(TextRange::empty(0), |token| {
                TextRange::empty(token.range.end)
            }),
        });
        if tok.kind == kind {
            self.bump();
            Ok(())
        } else {
            let mut error = self.error(tok, &format!("{kind:?}"));
            error.expected = expected;
            error.expected_owner = self.syntax.current();
            Err(error)
        }
    }

    fn expect_or_insert(
        &mut self,
        kind: TokenKind,
        insertion_is_unambiguous: bool,
    ) -> ParseResult<()> {
        if self.peek(kind.clone()) {
            self.bump();
            return Ok(());
        }
        if self.recover && insertion_is_unambiguous {
            let token = self.tokens.get(self.pos).cloned().unwrap_or_else(|| Token {
                kind: TokenKind::EOF,
                line: self.tokens.last().map_or(1, |token| token.line),
                column: self.tokens.last().map_or(1, |token| token.column),
                range: self.tokens.last().map_or(TextRange::empty(0), |token| {
                    TextRange::empty(token.range.end)
                }),
            });
            let mut error = self.error(token, &format!("{kind:?}"));
            error.expected = expected_syntax_kind(&kind);
            error.expected_owner = self.syntax.current();
            self.errors.push(*error);
            return Ok(());
        }
        self.expect(kind)
    }

    fn number_to_usize(&self, token: &Token, value: &str, context: &str) -> ParseResult<usize> {
        parse_bounded_unsigned(value, usize::MAX as u64)
            .and_then(|value| usize::try_from(value).map_err(|_| ()))
            .map_err(|()| {
                self.range_error(token, format!("{context} integer literal out of range"))
            })
    }

    fn range_error(&self, token: &Token, message: String) -> Box<ParseError> {
        let line_text = self
            .source
            .lines()
            .nth(token.line.saturating_sub(1))
            .unwrap_or("");
        let caret = " ".repeat(token.column.saturating_sub(1)) + "^";
        Box::new(ParseError {
            code: "K1001",
            message,
            line: token.line,
            column: token.column,
            snippet: format!("{line_text}\n{caret}"),
            range: token.range,
            fix: None,
            expected: None,
            expected_owner: None,
        })
    }

    fn peek(&self, kind: TokenKind) -> bool {
        self.tokens.get(self.pos).map(|t| t.kind.clone()) == Some(kind)
    }

    fn peek_n(&self, offset: usize, kind: TokenKind) -> bool {
        self.tokens.get(self.pos + offset).map(|t| t.kind.clone()) == Some(kind)
    }

    fn peek_ident_n(&self, offset: usize, name: &str) -> bool {
        matches!(
            self.tokens.get(self.pos + offset),
            Some(Token { kind: TokenKind::Ident(s), .. }) if s == name
        )
    }

    fn typed_local_starts_here(&self) -> bool {
        if matches!(
            (self.tokens.get(self.pos), self.tokens.get(self.pos + 1)),
            (
                Some(Token {
                    kind: TokenKind::Ident(_),
                    ..
                }),
                Some(
                    Token {
                        kind: TokenKind::Ident(_),
                        ..
                    } | Token {
                        kind: TokenKind::Less,
                        ..
                    }
                )
            )
        ) {
            return true;
        }
        if !self.peek(TokenKind::LParen) {
            return false;
        }
        let mut depth = 0_usize;
        for (offset, token) in self.tokens[self.pos..].iter().enumerate() {
            match &token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(self.pos + offset + 1),
                            Some(Token {
                                kind: TokenKind::Ident(_),
                                ..
                            })
                        );
                    }
                }
                TokenKind::EOF | TokenKind::Equal if depth == 1 => return false,
                _ => {}
            }
        }
        false
    }

    fn question_starts_ternary(&self) -> bool {
        if !self.peek(TokenKind::Question)
            || !self
                .tokens
                .get(self.pos.saturating_add(1))
                .is_some_and(|token| Self::token_starts_expression(&token.kind))
        {
            return false;
        }

        // `value?[index]` is postfix propagation followed by indexing, while
        // `flag ? [value] : [fallback]` is a conditional whose first branch is
        // a list literal. Look for the conditional's top-level `:` instead of
        // deciding from `[` alone. Colons inside calls, structs, JSON, lists,
        // or parenthesized expressions cannot terminate the true branch.
        let mut paren_depth = 0_usize;
        let mut bracket_depth = 0_usize;
        let mut brace_depth = 0_usize;
        for token in self.tokens.iter().skip(self.pos.saturating_add(1)) {
            let at_top_level = paren_depth == 0 && bracket_depth == 0 && brace_depth == 0;
            match token.kind {
                TokenKind::Colon if at_top_level => return true,
                TokenKind::LParen => paren_depth = paren_depth.saturating_add(1),
                TokenKind::LBracket => bracket_depth = bracket_depth.saturating_add(1),
                TokenKind::LBrace => brace_depth = brace_depth.saturating_add(1),
                TokenKind::RParen => {
                    if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
                        return false;
                    }
                    paren_depth = paren_depth.saturating_sub(1);
                }
                TokenKind::RBracket => {
                    if bracket_depth == 0 && paren_depth == 0 && brace_depth == 0 {
                        return false;
                    }
                    bracket_depth = bracket_depth.saturating_sub(1);
                }
                TokenKind::RBrace => {
                    if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 {
                        return false;
                    }
                    brace_depth = brace_depth.saturating_sub(1);
                }
                TokenKind::Semicolon | TokenKind::Comma | TokenKind::EOF if at_top_level => {
                    return false;
                }
                _ => {}
            }
        }
        false
    }

    fn token_starts_expression(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::True
                | TokenKind::False
                | TokenKind::Number(_)
                | TokenKind::DecimalLiteral(_)
                | TokenKind::String(_)
                | TokenKind::Bytes(_)
                | TokenKind::Ident(_)
                | TokenKind::State
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::If
                | TokenKind::Match
                | TokenKind::Minus
                | TokenKind::Bang
        )
    }

    fn synchronize_source_item(&mut self, item_start: usize) {
        let start_column = self
            .tokens
            .get(item_start)
            .map_or(usize::MAX, |token| token.column);
        self.pos = item_start.saturating_add(1).min(self.tokens.len());
        let mut brace_depth = 0_usize;
        while let Some(token) = self.tokens.get(self.pos) {
            match &token.kind {
                TokenKind::LBrace => brace_depth = brace_depth.saturating_add(1),
                TokenKind::RBrace if brace_depth == 0 => return,
                TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                _ if Self::token_starts_source_item(token) && token.column <= start_column => {
                    return;
                }
                _ => {}
            }
            self.pos = self.pos.saturating_add(1);
        }
    }

    fn synchronize_statement(&mut self, statement_start: usize) {
        if self.pos > statement_start
            && self
                .tokens
                .get(self.pos.saturating_sub(1))
                .is_some_and(|token| matches!(&token.kind, TokenKind::Semicolon))
        {
            return;
        }
        let start_column = self
            .tokens
            .get(statement_start)
            .map_or(usize::MAX, |token| token.column);
        let mut delimiter_depth = 0_usize;
        while let Some(token) = self.tokens.get(self.pos) {
            if delimiter_depth == 0 {
                if matches!(&token.kind, TokenKind::RBrace | TokenKind::EOF) {
                    return;
                }
                if self.pos != statement_start
                    && token.column <= start_column
                    && Self::token_starts_statement(token)
                {
                    return;
                }
            }
            match &token.kind {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                    delimiter_depth = delimiter_depth.saturating_add(1);
                }
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    delimiter_depth = delimiter_depth.saturating_sub(1);
                }
                TokenKind::Semicolon if delimiter_depth == 0 => {
                    self.pos = self.pos.saturating_add(1);
                    return;
                }
                _ => {}
            }
            self.pos = self.pos.saturating_add(1);
        }
    }

    fn token_starts_statement(token: &Token) -> bool {
        matches!(
            &token.kind,
            TokenKind::Let
                | TokenKind::Var
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::If
                | TokenKind::For
                | TokenKind::LBrace
                | TokenKind::State
                | TokenKind::Ident(_)
        )
    }

    fn token_starts_source_item(token: &Token) -> bool {
        matches!(
            &token.kind,
            TokenKind::Hash
                | TokenKind::Struct
                | TokenKind::Error
                | TokenKind::Const
                | TokenKind::State
                | TokenKind::Trigger
                | TokenKind::Fn
                | TokenKind::Kotoage
                | TokenKind::View
                | TokenKind::Hajimari
                | TokenKind::Kaizen
                | TokenKind::Seiyaku
                | TokenKind::Module
        ) || matches!(
            &token.kind,
            TokenKind::Ident(name) if matches!(name.as_str(), "fixture" | "koto_test")
        )
    }

    fn bump(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token {
            kind: TokenKind::EOF,
            line: self.tokens.last().map_or(0, |t| t.line),
            column: self.tokens.last().map_or(0, |t| t.column),
            range: self.tokens.last().map_or(TextRange::empty(0), |token| {
                TextRange::empty(token.range.end)
            }),
        });
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn error(&self, token: Token, expected: &str) -> Box<ParseError> {
        let line_text = self.source.lines().nth(token.line - 1).unwrap_or("");
        let caret = " ".repeat(token.column.saturating_sub(1)) + "^";
        let message = format!("expected {expected} but found {kind:?}", kind = token.kind);
        Box::new(ParseError {
            code: "K1001",
            message,
            line: token.line,
            column: token.column,
            snippet: format!("{line_text}\n{caret}"),
            range: token.range,
            fix: None,
            expected: None,
            expected_owner: None,
        })
    }

    fn coded_error(
        &self,
        token: Token,
        code: &'static str,
        message: impl Into<String>,
    ) -> Box<ParseError> {
        let mut error = self.error(token, "valid source");
        error.code = code;
        error.message = message.into();
        error
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockExpressionFlow {
    Unit,
    Value,
    Diverges,
}

fn combine_expression_branch_flows(
    branches: impl IntoIterator<Item = BlockExpressionFlow>,
) -> BlockExpressionFlow {
    let mut saw_branch = false;
    let mut saw_value = false;
    for branch in branches {
        saw_branch = true;
        match branch {
            BlockExpressionFlow::Unit => return BlockExpressionFlow::Unit,
            BlockExpressionFlow::Value => saw_value = true,
            BlockExpressionFlow::Diverges => {}
        }
    }
    if !saw_branch {
        BlockExpressionFlow::Unit
    } else if saw_value {
        BlockExpressionFlow::Value
    } else {
        BlockExpressionFlow::Diverges
    }
}

fn block_flow(block: &Block) -> BlockExpressionFlow {
    if block.statements.iter().any(statement_diverges) {
        return BlockExpressionFlow::Diverges;
    }
    block
        .tail
        .as_deref()
        .map_or(BlockExpressionFlow::Unit, block_expression_flow)
}

fn statement_diverges(statement: &Statement) -> bool {
    match statement.kind() {
        Statement::Return(_) | Statement::Break | Statement::Continue => true,
        Statement::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        }
        | Statement::IfLet {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => {
            block_flow(then_branch) == BlockExpressionFlow::Diverges
                && block_flow(else_branch) == BlockExpressionFlow::Diverges
        }
        Statement::Expr(expression)
        | Statement::Let {
            value: expression, ..
        }
        | Statement::Assign {
            value: expression, ..
        } => block_expression_flow(expression) == BlockExpressionFlow::Diverges,
        Statement::AssignExpr { target, value, .. } => {
            block_expression_flow(target) == BlockExpressionFlow::Diverges
                || block_expression_flow(value) == BlockExpressionFlow::Diverges
        }
        Statement::Source { .. } | Statement::Resolved { .. } => {
            unreachable!("kind() strips provenance wrappers")
        }
        Statement::If {
            else_branch: None, ..
        }
        | Statement::IfLet {
            else_branch: None, ..
        }
        | Statement::While { .. }
        | Statement::For { .. }
        | Statement::ForEachMap { .. } => false,
    }
}

fn block_expression_flow(expression: &Expr) -> BlockExpressionFlow {
    match expression.kind() {
        Expr::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        }
        | Expr::IfLet {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => combine_expression_branch_flows([block_flow(then_branch), block_flow(else_branch)]),
        Expr::Match { arms, .. } => {
            combine_expression_branch_flows(arms.iter().map(|arm| block_flow(&arm.body)))
        }
        Expr::If {
            else_branch: None, ..
        }
        | Expr::IfLet {
            else_branch: None, ..
        } => BlockExpressionFlow::Unit,
        _ => BlockExpressionFlow::Value,
    }
}

fn if_expression_statement_inner(expression: Expr) -> Statement {
    match expression.into_kind() {
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => Statement::If {
            cond: *condition,
            then_branch,
            else_branch,
        },
        Expr::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => Statement::IfLet {
            pattern,
            value: *value,
            then_branch,
            else_branch,
        },
        _ => unreachable!("else-if parsing produces an if expression"),
    }
}

fn removed_method_helper_message(name: &str) -> Option<&'static str> {
    match name {
        "account_id" | "asset_definition" | "asset_id" | "nft_id" | "name" | "json" | "domain"
        | "domain_id" | "blob" | "norito_bytes" | "dataspace_id" | "axt_descriptor"
        | "asset_handle" | "proof_blob" | "soracloud_request" | "soracloud_response" => Some(
            "constructor method aliases were removed; call the canonical constructor explicitly",
        ),
        "has" => Some("`map.has(key)` was removed; use `map.contains(key)`"),
        "get_or_insert_default" => Some(
            "`map.get_or_insert_default(key, default)` was removed; use `map.ensure(key, default)`",
        ),
        "path_map_key" | "path_map_key_norito" => {
            Some("`base.path_map_key(segment)` was removed; use `base.path(segment)`")
        }
        "json_get_int" => Some("`json.json_get_int(key)` was removed; use `json.get_int(key)`"),
        "get_amount" | "json_get_amount" | "get_numeric" | "json_get_numeric" => {
            Some("legacy numeric JSON getters were retired; use `.get_quantity(key)`")
        }
        "json_get_json" => Some("`json.json_get_json(key)` was removed; use `json.get_json(key)`"),
        "json_get_name" => Some("`json.json_get_name(key)` was removed; use `json.get_name(key)`"),
        "json_get_account_id" => {
            Some("`json.json_get_account_id(key)` was removed; use `json.get_account_id(key)`")
        }
        "json_get_asset_definition_id" => Some(
            "`json.json_get_asset_definition_id(key)` was removed; use `json.get_asset_definition_id(key)`",
        ),
        "json_get_nft_id" => {
            Some("`json.json_get_nft_id(key)` was removed; use `json.get_nft_id(key)`")
        }
        "json_get_blob_hex" => {
            Some("`json.json_get_blob_hex(key)` was removed; use `json.get_blob_hex(key)`")
        }
        _ => None,
    }
}

fn removed_method_helper_code(name: &str) -> &'static str {
    if matches!(
        name,
        "get_amount" | "json_get_amount" | "get_numeric" | "json_get_numeric"
    ) {
        "E_LEGACY_JSON_GETTER"
    } else {
        "K1001"
    }
}

fn removed_free_helper_message(name: &str) -> Option<&'static str> {
    match name {
        "json::set_i64" | "json::set_int" => Some(
            "scalar JSON setters are not part of Kotodama V1; use native `json { key: value }` construction so adaptive-width int values remain exact",
        ),
        "numeric::to_i64" | "numeric::neg" | "numeric::add" | "numeric::sub" | "numeric::mul"
        | "numeric::div" | "numeric::rem" | "numeric::eq" | "numeric::ne" | "numeric::lt"
        | "numeric::le" | "numeric::gt" | "numeric::ge" | "math::isqrt" | "math::abs"
        | "math::min" | "math::max" | "math::div_ceil" | "math::gcd" | "math::mean" => Some(
            "generic numeric helpers are not part of Kotodama V1; use operators and the named int, decimal, or quantity conversions",
        ),
        "contains" | "std::map::contains" | "has" | "std::map::has" => {
            Some("`contains(...)` was removed; use `map.contains(key)`")
        }
        "get_or" | "std::map::get_or" => {
            Some("`get_or(...)` was removed; use `map.get_or(key, default)`")
        }
        "get_or_insert_default"
        | "std::map::get_or_insert_default"
        | "ensure"
        | "std::map::ensure" => {
            Some("`ensure(...)` was removed as a free helper; use `map.ensure(key, default)`")
        }
        "remove" | "std::map::remove" => {
            Some("`remove(...)` is not a free helper; use `map.remove(key)`")
        }
        "path"
        | "path_map_key"
        | "path_map_key_norito"
        | "host::path"
        | "host::path_map_key"
        | "host::path_map_key_norito" => {
            Some("`path(...)` was removed as a free helper; use `base.path(segment)`")
        }
        "get_int" | "json_get_int" | "json::get_int" => {
            Some("`get_int(...)` was removed as a free helper; use `json.get_int(key)`")
        }
        "get_amount" | "json_get_amount" | "json::get_amount" | "get_numeric"
        | "json_get_numeric" | "json::get_numeric" => {
            Some("legacy numeric JSON getters were retired; use `value.get_quantity(key)`")
        }
        "get_json" | "json_get_json" | "json::get_json" => {
            Some("`get_json(...)` was removed as a free helper; use `json.get_json(key)`")
        }
        "get_name" | "json_get_name" | "json::get_name" => {
            Some("`get_name(...)` was removed as a free helper; use `json.get_name(key)`")
        }
        "get_account_id" | "json_get_account_id" | "json::get_account_id" => Some(
            "`get_account_id(...)` was removed as a free helper; use `json.get_account_id(key)`",
        ),
        "get_asset_definition_id"
        | "json_get_asset_definition_id"
        | "json::get_asset_definition_id" => Some(
            "`get_asset_definition_id(...)` was removed as a free helper; use `json.get_asset_definition_id(key)`",
        ),
        "get_nft_id" | "json_get_nft_id" | "json::get_nft_id" => {
            Some("`get_nft_id(...)` was removed as a free helper; use `json.get_nft_id(key)`")
        }
        "get_blob_hex" | "json_get_blob_hex" | "json::get_blob_hex" => {
            Some("`get_blob_hex(...)` was removed as a free helper; use `json.get_blob_hex(key)`")
        }
        "state_map_get" => Some("`state_map_get(...)` is compiler-internal; use `map.get(key)`"),
        "is_some" | "is_none" | "is_ok" | "is_err" | "unwrap_or" | "unwrap_err_or" => {
            Some("Option/Result inspection is method-only; call the method on the value")
        }
        "option_some" | "option_none" | "result_ok" | "result_err" => Some(
            "flat Option/Result constructors are not part of Kotodama V1; obtain typed values from parameters or APIs",
        ),
        _ => None,
    }
}

fn removed_free_helper_code(name: &str) -> &'static str {
    if matches!(
        name,
        "json::set_i64"
            | "json::set_int"
            | "numeric::to_i64"
            | "numeric::neg"
            | "numeric::add"
            | "numeric::sub"
            | "numeric::mul"
            | "numeric::div"
            | "numeric::rem"
            | "numeric::eq"
            | "numeric::ne"
            | "numeric::lt"
            | "numeric::le"
            | "numeric::gt"
            | "numeric::ge"
            | "math::isqrt"
            | "math::abs"
            | "math::min"
            | "math::max"
            | "math::div_ceil"
            | "math::gcd"
            | "math::mean"
    ) {
        "E_RETIRED_NUMERIC_HELPER"
    } else if matches!(
        name,
        "get_amount"
            | "json_get_amount"
            | "json::get_amount"
            | "get_numeric"
            | "json_get_numeric"
            | "json::get_numeric"
    ) {
        "E_LEGACY_JSON_GETTER"
    } else {
        "K1001"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::DomainId;

    fn parse_module(body: &str) -> Result<Program, String> {
        parse(&format!("module TestModule {{ {body} }}"))
    }

    fn sample_account_literal() -> String {
        iroha_data_model::account::AccountId::new(
            "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
                .parse()
                .expect("public key"),
        )
        .to_string()
    }

    #[test]
    fn parse_return_statements() {
        let src = "fn f() { return; return 1; }";
        let prog = parse_module(src).unwrap();
        assert_eq!(prog.items.len(), 1);
        let f = match &prog.items[0] {
            Item::Function(f) => f,
            _ => panic!("expected function item"),
        };
        assert_eq!(f.body.statements.len(), 2);
        match f.body.statements[0].kind() {
            Statement::Return(None) => {}
            _ => panic!("no return;"),
        }
        match f.body.statements[1].kind() {
            Statement::Return(Some(_)) => {}
            _ => panic!("no return expr"),
        }
    }

    #[test]
    fn conditional_parser_preserves_nested_then_and_else_associativity() {
        let program = parse_module("fn f() { return a ? b ? c : d : e ? f : g; }")
            .expect("parse nested conditional");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function item");
        };
        let Statement::Return(Some(expression)) = function.body.statements[0].kind() else {
            panic!("expected return expression");
        };
        let Expr::Conditional {
            then_expr,
            else_expr,
            ..
        } = expression.kind()
        else {
            panic!("expected outer conditional return");
        };
        assert!(
            matches!(then_expr.kind(), Expr::Conditional { .. }),
            "the nested then arm must bind to the outer conditional"
        );
        assert!(
            matches!(else_expr.kind(), Expr::Conditional { .. }),
            "the nested else arm must bind to the outer conditional"
        );
    }

    #[test]
    fn iterative_type_parser_preserves_nested_generic_and_tuple_shapes() {
        let program = parse_module("struct Wrapper { Result<Option<int>, (bool, string)> value }")
            .expect("parse nested type");
        let Item::Struct(definition) = &program.items[0] else {
            panic!("expected struct item");
        };
        let TypeExpr::Generic { base, args } = definition.fields[0].1.kind() else {
            panic!("expected Result generic");
        };
        assert_eq!(base, "Result");
        let TypeExpr::Generic {
            base: option_base,
            args: option_args,
        } = args[0].kind()
        else {
            panic!("expected Option generic");
        };
        assert_eq!(option_base, "Option");
        assert!(matches!(option_args[0].kind(), TypeExpr::Path(path) if path == "int"));
        let TypeExpr::Tuple(elements) = args[1].kind() else {
            panic!("expected tuple type");
        };
        assert!(matches!(elements[0].kind(), TypeExpr::Path(path) if path == "bool"));
        assert!(matches!(elements[1].kind(), TypeExpr::Path(path) if path == "string"));
    }

    #[test]
    fn parses_list_literals_and_filtered_comprehensions() {
        let program = parse_module(
            "fn lists() { let values = [1, 2,]; let doubled = [value * 2 for value in values if value > 0]; }",
        )
        .expect("parse bounded List forms");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function item");
        };
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected literal binding");
        };
        assert!(matches!(value.kind(), Expr::List(items) if items.len() == 2));
        let Statement::Let { value, .. } = function.body.statements[1].kind() else {
            panic!("expected comprehension binding");
        };
        assert!(matches!(
            value.kind(),
            Expr::ListComprehension {
                item,
                condition: Some(_),
                ..
            } if item == "value"
        ));
    }

    #[test]
    fn canonical_public_parse_output_contains_no_provenance_wrappers() {
        let program = parse_module(
            "fn clean(List<int, 4> values) -> bool { let copy = [item for item in values if true]; copy.contains(1) }",
        )
        .expect("parse representative source-backed tree");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function item")
        };

        let parameter_ty = function.params[0].ty.as_ref().expect("parameter type");
        assert!(parameter_ty.source().is_none());
        let TypeExpr::Generic { args, .. } = parameter_ty else {
            panic!("List type")
        };
        assert!(args.iter().all(|ty| ty.source().is_none()));

        let statement = &function.body.statements[0];
        assert!(statement.source().is_none());
        let Statement::Let { value, .. } = statement else {
            panic!("comprehension binding")
        };
        assert!(value.source().is_none());
        let Expr::ListComprehension {
            expression,
            source,
            condition: Some(condition),
            ..
        } = value
        else {
            panic!("filtered comprehension")
        };
        assert!(expression.source().is_none());
        assert!(source.source().is_none());
        assert!(condition.source().is_none());

        let tail = function.body.tail.as_deref().expect("call tail");
        assert!(tail.source().is_none());
        let Expr::Call { args, .. } = tail else {
            panic!("method call")
        };
        assert!(args.iter().all(|argument| argument.source().is_none()));
    }

    #[test]
    fn list_type_capacity_is_preserved_as_a_constant_argument() {
        let program = parse_module("fn values(List<Option<int>, 64> input) {}").expect("List type");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function item");
        };
        let Some(parameter_type) = &function.params[0].ty else {
            panic!("expected parameter type");
        };
        let TypeExpr::Generic { base, args } = parameter_type.kind() else {
            panic!("expected generic List type");
        };
        assert_eq!(base, "List");
        assert!(matches!(args[0].kind(), TypeExpr::Generic { base, .. } if base == "Option"));
        assert!(matches!(args[1].kind(), TypeExpr::Const(64)));
    }

    #[test]
    fn malformed_list_expression_reports_the_closing_delimiter() {
        let error = parse_module("fn invalid() { let values = [1, 2; }")
            .expect_err("unterminated List must fail");
        assert!(error.contains("RBracket"), "{error}");
    }

    #[test]
    fn rejects_source_unit_values_and_degenerate_tuple_types() {
        for (body, expected) in [
            (
                "fn invalid() { let value = (); }",
                "source-level unit value `()` is not part of Kotodama V1",
            ),
            (
                "fn invalid(() value) {}",
                "tuple types require at least two elements",
            ),
            (
                "fn invalid((int) value) {}",
                "tuple types require at least two elements",
            ),
            (
                "fn invalid() -> () { return; }",
                "tuple types require at least two elements",
            ),
        ] {
            let error = parse_module(body).expect_err("invalid Unit/tuple surface must fail");
            assert!(error.contains(expected), "unexpected error: {error}");
        }

        let grouped = parse_module(
            "fn grouped() -> int { return (1); } fn pair((int, bool) value) -> (int, bool) { return (1, true); } fn omitted() { return; }",
        )
        .expect("grouping, real tuples, and omitted Unit returns remain valid");
        let Item::Function(grouped_function) = &grouped.items[0] else {
            panic!("expected grouped function")
        };
        assert!(matches!(
            grouped_function.body.statements[0].kind(),
            Statement::Return(Some(value)) if matches!(value.kind(), Expr::IntLiteral(value) if value == &BigInt::one())
        ));
    }

    #[test]
    fn else_if_is_represented_as_a_nested_if_in_the_else_block() {
        let program = parse_module(
            "fn classify(int value) -> int { if value < 0 { return -1; } else if value == 0 { return 0; } else { return 1; } }",
        )
        .expect("parse documented else-if chain");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function")
        };
        let Expr::If {
            else_branch: Some(outer_else),
            ..
        } = function
            .body
            .tail
            .as_deref()
            .expect("divergent if tail")
            .kind()
        else {
            panic!("expected outer divergent if tail with else")
        };
        let Expr::If {
            else_branch: Some(inner_else),
            ..
        } = outer_else
            .tail
            .as_deref()
            .expect("nested divergent if tail")
            .kind()
        else {
            panic!("else-if must remain one nested divergent if expression")
        };
        assert_eq!(inner_else.statements.len(), 1);
    }

    #[test]
    fn mixed_value_and_divergent_control_flow_remains_a_tail_expression() {
        let program = parse_module(
            r#"
            fn via_if(bool flag) -> int {
                if flag { 7 } else { return 9; }
            }
            fn via_if_let(Option<int> value) -> int {
                if let Option::some(item) = value { item } else { return 0; }
            }
            fn via_match(Option<int> value) -> int {
                match value {
                    Option::some(item) => item,
                    Option::none => { return 0; },
                }
            }
            "#,
        )
        .expect("parse mixed value/divergent tails");

        for (function, expected) in program.items.iter().zip(["if", "if let", "match"]) {
            let Item::Function(function) = function else {
                panic!("expected function")
            };
            assert!(
                function.body.statements.is_empty(),
                "{expected} tail must not be demoted to a statement"
            );
            let tail = function.body.tail.as_deref().expect("control-flow tail");
            assert!(
                matches!(
                    (expected, tail.kind()),
                    ("if", Expr::If { .. })
                        | ("if let", Expr::IfLet { .. })
                        | ("match", Expr::Match { .. })
                ),
                "unexpected {expected} tail: {tail:?}"
            );
        }
    }

    #[test]
    fn list_literal_ternary_is_distinct_from_propagation_followed_by_indexing() {
        let program = parse_module(
            r#"
            fn choose(bool flag) -> List<int, 1> { flag ? [1] : [2] }
            fn index_after_propagation(Option<List<int, 1>> value) -> int { value?[0] }
            "#,
        )
        .expect("parse list ternary and propagation-index adjacency");

        let Item::Function(choose) = &program.items[0] else {
            panic!("expected choose function")
        };
        assert!(matches!(
            choose.body.tail.as_deref().map(Expr::kind),
            Some(Expr::Conditional {
                then_expr,
                else_expr,
                ..
            }) if matches!(then_expr.kind(), Expr::List(_))
                && matches!(else_expr.kind(), Expr::List(_))
        ));

        let Item::Function(index) = &program.items[1] else {
            panic!("expected index function")
        };
        assert!(matches!(
            index.body.tail.as_deref().map(Expr::kind),
            Some(Expr::Index { target, .. })
                if matches!(target.kind(), Expr::Propagate(_))
        ));
    }

    #[test]
    fn parses_value_tails_and_expression_oriented_control_flow() {
        let program = parse_module(
            r#"
            fn identity(int value) -> int { value }
            fn choose(bool flag) -> int { if flag { 1 } else { 2 } }
            fn unwrap(Option<int> value) -> int {
                match value {
                    Option::some(item) => item,
                    Option::none => 0,
                }
            }
            fn observe(Option<int> value) {
                if let Option::some(item) = value { let _seen = item; }
            }
            "#,
        )
        .expect("parse expression-oriented V1 control flow");

        let functions = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            functions[0].body.tail.as_deref().map(Expr::kind),
            Some(Expr::Ident(name)) if name == "value"
        ));
        assert!(matches!(
            functions[1].body.tail.as_deref().map(Expr::kind),
            Some(Expr::If {
                else_branch: Some(_),
                ..
            })
        ));
        assert!(matches!(
            functions[2].body.tail.as_deref().map(Expr::kind),
            Some(Expr::Match { arms, .. }) if arms.len() == 2
        ));
        assert_eq!(functions[3].body.statements.len(), 1);
        assert!(matches!(
            functions[3].body.statements[0].kind(),
            Statement::IfLet {
                else_branch: None,
                ..
            }
        ));
    }

    #[test]
    fn postfix_propagation_binds_tighter_than_ternary() {
        let program = parse_module(
            "fn choose(bool condition, Option<int> maybe, int fallback) -> int { condition ? maybe? : fallback }",
        )
        .expect("parse ternary containing postfix propagation");
        let Item::Function(function) = &program.items[0] else {
            panic!("function item")
        };
        assert!(matches!(
            function.body.tail.as_deref().map(Expr::kind),
            Some(Expr::Conditional { then_expr, .. })
                if matches!(then_expr.kind(), Expr::Propagate(value)
                    if matches!(value.kind(), Expr::Ident(name) if name == "maybe"))
        ));
    }

    #[test]
    fn active_only_sum_constructors_have_no_placeholder_payloads() {
        let program = parse_module(
            r#"
            fn some(int value) -> Option<int> { Option::some(value) }
            fn none() -> Option<int> { Option::none }
            fn ok(int value) -> Result<int, string> { Result::ok(value) }
            fn err(string message) -> Result<int, string> { Result::err(message) }
            "#,
        )
        .expect("parse canonical active-only constructors");
        let tails = program.items.iter().filter_map(|item| match item {
            Item::Function(function) => function.body.tail.as_deref().map(Expr::kind),
            _ => None,
        });
        assert!(matches!(
            tails.collect::<Vec<_>>().as_slice(),
            [
                Expr::OptionSome(_),
                Expr::OptionNone,
                Expr::ResultOk(_),
                Expr::ResultErr(_),
            ]
        ));

        for (source, replacement) in [
            ("fn f() -> Option<int> { option::none(0) }", "Option::none"),
            (
                "fn f() -> Result<int, string> { result::ok(1, \"unused\") }",
                "Result::ok(1)",
            ),
        ] {
            let error = parse_module(source).expect_err("legacy constructor must be rejected");
            assert!(error.contains("E_LEGACY_SUM_CONSTRUCTOR"), "{error}");
            assert!(error.contains(replacement), "{error}");
        }
    }

    #[test]
    fn mutable_bindings_still_require_initializers() {
        let error = parse_module("fn invalid() { var int value; }")
            .expect_err("uninitialized locals are not part of V1");
        assert!(error.contains("Equal"), "unexpected error: {error}");
    }

    #[test]
    fn error_enum_requires_explicit_unique_nonzero_u32_codes() {
        let program =
            parse("seiyaku Errors { error enum Payment { Unauthorized = 1001, Expired = 1002 } }")
                .expect("parse stable error enum");
        let Item::ErrorEnum(errors) = &program.items[0] else {
            panic!("expected error enum")
        };
        assert_eq!(errors.name, "Payment");
        assert_eq!(errors.variants[0].name, "Unauthorized");
        assert_eq!(errors.variants[0].code, 1001);

        for body in [
            "error enum Empty {}",
            "error enum Zero { Invalid = 0 }",
            "error enum Missing { Invalid }",
            "error enum Duplicate { First = 7, Second = 7 }",
            "error enum Overflow { Invalid = 4294967296 }",
        ] {
            let error = parse(&format!("seiyaku Errors {{ {body} }}"))
                .expect_err("invalid error enum must fail parsing");
            assert!(!error.is_empty(), "empty diagnostic for `{body}`");
        }
    }

    #[test]
    fn parse_bools_and_logical_ops() {
        let src = "fn g() { let x = true && !false; }";
        let prog = parse_module(src).unwrap();
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_assignment_and_break_continue() {
        let src = "fn h() { var x = 0; x = 1; for i in range(10) { if i == 3 { break; } if i == 5 { continue; } } }";
        let prog = parse_module(src).unwrap();
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_preserves_local_binding_mutability() {
        let program = parse_module("fn f() { let fixed = 1; var int changing = 2; }")
            .expect("parse let and var bindings");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function")
        };
        assert!(matches!(
            function.body.statements[0].kind(),
            Statement::Let { mutable: false, .. }
        ));
        assert!(matches!(
            function.body.statements[1].kind(),
            Statement::Let { mutable: true, .. }
        ));
    }

    #[test]
    fn parse_canonical_seiyaku_surface_and_preserve_identity() {
        let src = r#"
        seiyaku Payments {
            state int counter;
            struct Pair { int left; int right; }
            hajimari() { counter = 0; }
            kaizen() {}
            kotoage fn submit(AccountId who, quantity amount) authorize("Submit") {}
            view fn read(Name key) -> int { return counter; }
            fn helper(int left, int right) -> int { return left + right; }
        }
        "#;
        let prog = parse(src).unwrap();
        assert_eq!(prog.unit.kind, SourceUnitKind::Seiyaku);
        assert_eq!(prog.unit.name, "Payments");
        let functions = prog
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(functions.len(), 5);
        assert_eq!(functions[0].name, "hajimari");
        assert_eq!(functions[0].modifiers.kind, FunctionKind::Hajimari);
        assert_eq!(functions[0].modifiers.permission, None);
        assert_eq!(functions[1].name, "kaizen");
        assert_eq!(functions[1].modifiers.kind, FunctionKind::Kaizen);
        assert_eq!(functions[1].modifiers.permission, None);
        assert_eq!(functions[2].modifiers.kind, FunctionKind::Kotoage);
        assert_eq!(functions[2].modifiers.permission.as_deref(), Some("Submit"));
        assert_eq!(functions[3].modifiers.kind, FunctionKind::View);
        assert_eq!(functions[4].modifiers.kind, FunctionKind::Private);
    }

    #[test]
    fn lifecycle_declarations_reject_source_authorization() {
        for source in [
            "seiyaku Demo { hajimari() authorize(\"HajimariPermission\") {} }",
            "seiyaku Demo { kaizen() authorize(\"KaizenPermission\") {} }",
        ] {
            let error = parse(source).expect_err("lifecycle authorization is runtime-owned");
            assert!(
                error.contains("lifecycle authorization is runtime-defined"),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn parse_canonical_module_surface_and_preserve_identity() {
        let prog =
            parse("module Math { fn add(int left, int right) -> int { return left + right; } }")
                .expect("parse module");
        assert_eq!(prog.unit.kind, SourceUnitKind::Module);
        assert_eq!(prog.unit.name, "Math");
    }

    #[test]
    fn parse_canonical_context_and_ledger_namespaces() {
        let program = parse(
            r#"
            seiyaku Payments {
                kotoage fn transfer(
                    AccountId recipient,
                    AssetDefinitionId asset,
                    quantity amount,
                    DataSpaceId dataspace
                ) authorize("TransferAsset") {
                    let sender = context::authority();
                    ledger::asset::transfer(
                        source: sender,
                        destination: recipient,
                        asset_definition: asset,
                        amount: amount,
                        dataspace: dataspace,
                    );
                }
            }
            "#,
        )
        .expect("parse canonical namespaces");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function")
        };
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected authority binding")
        };
        assert!(matches!(
            value.kind(),
            Expr::Call { name, .. } if name == "context::authority"
        ));
        let Statement::Expr(call) = function.body.statements[1].kind() else {
            panic!("expected ledger call statement");
        };
        assert!(
            matches!(call.kind(), Expr::Call { name, .. } if name == "ledger::asset::transfer")
        );
    }

    #[test]
    fn keyword_tokens_are_admitted_only_in_required_namespace_positions() {
        let program = parse(
            r#"
            seiyaku Controls {
                kotoage fn update(Name path, Name trigger_id) authorize("Control") {
                    state::set(path, 1);
                    ledger::trigger::set_enabled(trigger_id, 1);
                }
            }
            "#,
        )
        .expect("parse keyword-backed V1 namespaces");
        let function = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "update" => Some(function),
                _ => None,
            })
            .expect("update entrypoint");
        let Statement::Expr(state_call) = function.body.statements[0].kind() else {
            panic!("expected state call statement");
        };
        assert!(matches!(state_call.kind(), Expr::Call { name, .. } if name == "state::set"));
        let Statement::Expr(trigger_call) = function.body.statements[1].kind() else {
            panic!("expected trigger call statement");
        };
        assert!(
            matches!(trigger_call.kind(), Expr::Call { name, .. } if name == "ledger::trigger::set_enabled")
        );

        for binding in ["state", "trigger"] {
            let source = format!("seiyaku Reserved {{ fn bad() {{ let {binding} = 1; }} }}");
            parse(&source).expect_err("keyword must remain unavailable as a binding");
        }
        parse("seiyaku Reserved { fn bad() { seiyaku::register_code(); } }")
            .expect_err("declaration keywords must remain unavailable as namespace roots");
    }

    #[test]
    fn english_declaration_spellings_are_rejected() {
        for source in [
            "contract Legacy {}",
            "seiyaku Legacy { entry fn run() authorize(\"Run\") {} }",
            "seiyaku Legacy { init() {} }",
            "seiyaku Legacy { upgrade() {} }",
            "seiyaku Legacy { kotoage fn run() permission(Admin) {} }",
        ] {
            parse(source).expect_err("English declaration spelling must be rejected");
        }
    }

    #[test]
    fn branded_keywords_are_contextual_namespace_segments_only() {
        for source in [
            "module M { fn f() { context::kotoage(); ledger::seiyaku::grant_kotoage(); test::invoke_kotoage(kotoage: \"run\", arguments: Json::parse(\"{}\")); } }",
            "module M { fn f() { context::言挙げ(); ledger::誓約::grant_kotoage(); test::invoke_kotoage(言挙げ: \"run\", arguments: Json::parse(\"{}\")); } }",
        ] {
            parse(source).expect("branded capability path must parse");
        }

        for source in [
            "module M { fn f() { kotoage(); } }",
            "module M { fn f() { seiyaku::grant_kotoage(); } }",
            "module M { fn f() { let kotoage = 1; } }",
            "module M { fn f() { let seiyaku = 1; } }",
        ] {
            parse(source).expect_err("branded declaration keyword must stay reserved");
        }
    }

    #[test]
    fn branded_japanese_declaration_keywords_are_first_class() {
        for source in [
            "誓約 Demo {}",
            "seiyaku Demo { 始まり() {} }",
            "seiyaku Demo { 言挙げ fn run() authorize(\"Run\") {} }",
            "seiyaku Demo { 改善() {} }",
        ] {
            parse(source).expect("branded Japanese declaration syntax must parse");
        }
    }

    #[test]
    fn exactly_one_named_source_unit_is_required() {
        for source in [
            "fn main() {}",
            "seiyaku First {} seiyaku Second {}",
            "module First {} module Second {}",
        ] {
            parse(source).expect_err("invalid source-unit cardinality must fail");
        }
    }

    #[test]
    fn canonical_declaration_shapes_are_required() {
        for source in [
            "module M { fn f(value: int) {} }",
            "module M { fn f(value) {} }",
            "module M { const VALUE = 1; }",
            "seiyaku C { state value: int; }",
            "module M { struct Pair { value: int; } }",
            "seiyaku C { kotoage fn f() authorize(Admin) {} }",
        ] {
            parse(source).expect_err("legacy declaration shape must fail");
        }
    }

    #[test]
    fn retired_colon_declarations_report_the_type_first_replacement() {
        for (source, replacement) in [
            ("module M { fn f(value: int) {} }", "`int value`"),
            (
                "module M { const limit: int = 1; }",
                "`const int limit = 1;`",
            ),
            ("seiyaku C { state value: int; }", "`state int value;`"),
            ("module M { struct Pair { value: int; } }", "`int field;`"),
            (
                "module M { fn f() { let value: int = 1; } }",
                "`let int value = ...;`",
            ),
        ] {
            let error = parse(source).expect_err("retired declaration order must fail closed");
            assert!(error.contains("E_RETIRED_DECLARATION_ORDER"), "{error}");
            assert!(error.contains(replacement), "{error}");
        }
    }

    #[test]
    fn retired_numeric_type_spellings_are_rejected_with_replacements() {
        for legacy in crate::semantic::V1_RETIRED_NUMERIC_TYPE_NAMES {
            let source = format!("module Types {{ fn use_type({legacy} value) {{}} }}");
            let error = parse(&source).expect_err("retired numeric type must fail closed");
            assert!(
                error.contains("E_RETIRED_NUMERIC_TYPE"),
                "unexpected diagnostic for `{legacy}`: {error}"
            );
        }
    }

    #[test]
    fn exact_amount_is_rejected_in_every_identifier_context() {
        for source in [
            "module Amount { fn f() {} }",
            "module M { fn Amount() {} }",
            "module M { fn f(int Amount) {} }",
            "module M { fn f() { let int Amount = 1; } }",
            "module M { struct Record { int Amount; } }",
            "module M { struct Record { int value; } fn f() { let item = Record { Amount: 1 }; } }",
            "module M { fn f() { for Amount in range(1) {} } }",
            "module M { fn f() { let values = [1]; let copy = [item for Amount in values]; } }",
            "module M { fn f(Option<int> value) { if let Option::some(Amount) = value {} } }",
            "module M { fn target(int value) {} fn f() { target(Amount: 1); } }",
            "module M { fn f(Json value) { let found = value.Amount; } }",
            "module M { fn f() { Amount::call(); } }",
            "module M { fn f() { let payload = json { Amount: 1 }; } }",
        ] {
            let error = parse(source).expect_err("exact `Amount` identifier must fail closed");
            assert!(
                error.contains("E_FORBIDDEN_SOURCE_IDENTIFIER"),
                "unexpected diagnostic for `{source}`: {error}"
            );
        }
    }

    #[test]
    fn lowercase_amount_and_non_identifier_amount_text_remain_valid() {
        parse(
            r#"module AmountText {
                struct Record { int amount; }
                fn target(int amount) {}
                fn amount(Json value) {
                    let int amount = 1;
                    for amount_item in range(1) {
                        target(amount: amount_item);
                        let member = value.amount;
                        let payload = json { amount: amount_item, "Amount": 1 };
                        let text = "Amount";
                        // Amount remains legal documentation text.
                    }
                }
            }"#,
        )
        .expect("lowercase `amount`, strings, comments, and quoted JSON keys remain valid");
    }

    #[test]
    fn retired_amount_type_keeps_its_quantity_fix_diagnostic_only() {
        let error = parse("module M { fn f(Amount value) {} }")
            .expect_err("retired `Amount` type must fail closed");
        assert!(error.contains("E_RETIRED_NUMERIC_TYPE"), "{error}");
        assert!(error.contains("use `quantity`"), "{error}");
        assert!(!error.contains("E_FORBIDDEN_SOURCE_IDENTIFIER"), "{error}");
    }

    #[test]
    fn every_retired_amount_type_keeps_its_quantity_fix_during_recovery() {
        for (index, (source, retired_count, forbidden_count)) in [
            ("module M { fn f(Amount a, Amount b) {} }", 2, 0),
            ("module M { fn f(Result<Amount, Amount> value) {} }", 2, 0),
            ("module M { fn f(Amount type_value, int Amount) {} }", 1, 1),
        ]
        .into_iter()
        .enumerate()
        {
            let source_file = SourceFile::new(
                SourceId(40 + index as u32),
                "retired-amount-recovery.ko",
                source,
            );
            let diagnostics = parse_source(&source_file, FrontendBudget::v1())
                .expect_err("every exact `Amount` occurrence must fail closed");
            let retired = diagnostics
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "E_RETIRED_NUMERIC_TYPE")
                .collect::<Vec<_>>();
            assert_eq!(retired.len(), retired_count, "{source}");
            for diagnostic in retired {
                assert_eq!(
                    diagnostic.fix.as_ref().map(|fix| fix.replacement.as_str()),
                    Some("quantity"),
                    "{source}"
                );
            }
            assert_eq!(
                diagnostics
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.code == "E_FORBIDDEN_SOURCE_IDENTIFIER")
                    .count(),
                forbidden_count,
                "{source}"
            );
        }
    }

    #[test]
    fn retired_numeric_helpers_are_rejected_before_resolution() {
        for source in [
            "module M { fn f() { let value = numeric::add(left: 1, right: 2); } }",
            "module M { fn f() { let value = numeric::to_i64(1); } }",
            "module M { fn f() { let value = math::isqrt(9); } }",
            "module M { fn f() { let value = json::set_i64(json::object(), Name::parse(\"n\"), 1); } }",
            "module M { fn f() { let value = json::set_int(json::object(), Name::parse(\"n\"), 1); } }",
        ] {
            let error = parse(source).expect_err("retired numeric helper must fail closed");
            assert!(error.contains("E_RETIRED_NUMERIC_HELPER"), "{error}");
        }
    }

    #[test]
    fn modules_reject_deployable_contract_items() {
        for body in [
            "kotoage fn run() {}",
            "view fn read() -> int { return 1; }",
            "hajimari() {}",
            "kaizen() {}",
            "state int value;",
            "meta { abi_version: 1; }",
        ] {
            let source = format!("module Library {{ {body} }}");
            parse(&source).expect_err("module must reject deployable item");
        }
    }

    #[test]
    fn while_and_unbounded_for_forms_are_rejected() {
        for body in [
            "fn f() { while true {} }",
            "fn f() { for let i = 0; i < 3; i = i + 1 {} }",
            "fn f(int n) { for i in range(n) {} }",
            "fn f(StateMap<int, int> values) { for (key, value) in values {} }",
        ] {
            parse_module(body).expect_err("unbounded loop form must fail");
        }
    }

    #[test]
    fn parse_for_range_loop() {
        let src = "fn f() { for x in range(6) { let y = x; } }";
        let prog = parse_module(src).expect("parse failed");
        let func = prog
            .items
            .iter()
            .find_map(|it| match it {
                Item::Function(f) => Some(f),
                _ => None,
            })
            .expect("function present");
        assert!(!func.body.statements.is_empty());
    }

    #[test]
    fn source_meta_is_rejected_in_favor_of_build_configuration() {
        for body in [
            "zk: true",
            "zk: false",
            "abi_version: 1",
            "vector_length: 4",
            "vector: true",
            "features: [\"zk\"]",
            "max_cycles: 1000",
        ] {
            let source = format!("seiyaku C {{ meta {{ {body}; }} }}");
            let err = parse(&source).expect_err("source policy toggle must be rejected");
            assert!(
                err.contains("source-level `meta { ... }` is not supported")
                    && err.contains("compiler build configuration"),
                "unexpected error for {body}: {err}"
            );
        }
    }

    #[test]
    fn parse_reports_unexpected_top_level_tokens() {
        let src = "let orphan = 1;";
        let err = parse(src).unwrap_err();
        assert!(err.contains("exactly one"));
    }

    #[test]
    fn parse_reports_unexpected_contract_items() {
        let src = r#"
        seiyaku C {
            123
        }
        "#;
        let err = parse(src).unwrap_err();
        assert!(err.contains("source-unit item"));
    }

    #[test]
    fn parse_function_modifiers_are_preserved() {
        let src = r#"
        seiyaku Demo {
            kotoage fn foo() authorize("Admin") {}
        }
        "#;
        let prog = parse(src).expect("parse modifiers");
        let func = prog
            .items
            .into_iter()
            .find_map(|item| match item {
                Item::Function(f) => Some(f),
                _ => None,
            })
            .expect("function present");
        assert_eq!(func.name, "foo");
        assert_eq!(func.modifiers.kind, FunctionKind::Kotoage);
        assert_eq!(func.modifiers.permission.as_deref(), Some("Admin"));
    }

    #[test]
    fn kotoage_authorization_is_a_parse_time_grammar_requirement() {
        for source in [
            "seiyaku Demo { kotoage fn run() {} }",
            "誓約 Demo { 言挙げ fn run() {} }",
            "seiyaku Demo { kotoage fn run() -> int { return 1; } }",
        ] {
            let error = parse(source).expect_err("kotoage without authorization must not parse");
            assert!(error.contains("K1001"), "{error}");
            assert!(
                error.contains("requires `authorize(\"Permission\")` before its body"),
                "{error}"
            );
            assert!(!error.contains("K2004"), "{error}");
        }

        parse("seiyaku Demo { view fn read() -> int { return 1; } }")
            .expect("public views remain valid without source authorization");
    }

    #[test]
    fn retired_numeric_literal_suffixes_are_rejected() {
        for suffix in ["i64", "u128", "amt", "qty", "float", "money"] {
            let source = format!("fn main() {{ let value = 1{suffix}; }}");
            let error = parse_module(&source).expect_err("numeric suffix must fail closed");
            assert!(
                error.contains("E_RETIRED_NUMERIC_SUFFIX"),
                "unexpected diagnostic for `{suffix}`: {error}"
            );
        }
    }

    #[test]
    fn adaptive_width_int_literal_is_allowed_without_a_suffix() {
        let src = "fn main() { let int x = 340282366920938463463374607431768211455; }";
        let program = parse_module(src).expect("parse adaptive-width int literal");
        let function = program
            .items
            .into_iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function present");
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected let statement");
        };
        assert!(matches!(
            value.kind(),
            Expr::IntLiteral(value) if value.to_string() == "340282366920938463463374607431768211455"
        ));
    }

    #[test]
    fn decimal_literal_ast_retains_exact_source_spelling() {
        let program = parse_module("fn main() { let decimal value = 1.250_0; }")
            .expect("parse decimal literal");
        let function = program
            .items
            .into_iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function present");
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected let statement");
        };
        assert!(matches!(value.kind(), Expr::DecimalLiteral(value) if value == "1.250_0"));
    }

    #[test]
    fn decimal_literals_follow_existing_expression_precedence() {
        let program = parse_module("fn main() { let value = true ? 1.0 : 2.0 + 3.0 * 4.0; }")
            .expect("parse decimal expression");
        let function = program
            .items
            .into_iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function present");
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected let statement");
        };
        let Expr::Conditional { else_expr, .. } = value.kind() else {
            panic!("expected conditional expression");
        };
        let Expr::Binary {
            op: BinaryOp::Add,
            right,
            ..
        } = else_expr.kind()
        else {
            panic!("expected addition in false branch");
        };
        assert!(matches!(
            right.kind(),
            Expr::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }

    #[test]
    fn signed_literals_retain_postfix_calls_after_atomic_range_parsing() {
        for literal in ["-1", "-1.0"] {
            let receiver = if literal == "-1" {
                format!("({literal})")
            } else {
                literal.to_owned()
            };
            parse_module(&format!(
                "fn main() {{ let value = {receiver}.operation(argument: 2); }}"
            ))
            .unwrap_or_else(|error| panic!("signed postfix `{literal}` failed: {error}"));
        }
    }

    #[test]
    fn native_json_preserves_decoded_keys_and_exact_source_spelling() {
        let program = parse_module(
            r#"fn build(string label) -> Json {
                json { owner: label, "owner-alias": json [label] }
            }"#,
        )
        .expect("parse native JSON object and array");
        let function = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function");
        let Expr::JsonObject(entries) = function.body.tail.as_deref().expect("JSON tail").kind()
        else {
            panic!("expected native JSON object");
        };
        assert_eq!(entries[0].key, "owner");
        assert_eq!(entries[0].key_spelling, "owner");
        assert_eq!(entries[1].key, "owner-alias");
        assert_eq!(entries[1].key_spelling, "\"owner-alias\"");
        assert!(matches!(entries[1].value.kind(), Expr::JsonArray(items) if items.len() == 1));
    }

    #[test]
    fn named_calls_preserve_source_names_and_trailing_comma() {
        let program = parse_module(
            "fn target(int first, string second) {} fn main() { target(second: \"two\", first: 1,); }",
        )
        .expect("parse named call");
        let main = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "main" => Some(function),
                _ => None,
            })
            .expect("main function");
        let Statement::Expr(call) = main.body.statements[0].kind() else {
            panic!("expected call statement");
        };
        let Expr::Call {
            args,
            argument_names,
            implicit_receiver,
            ..
        } = call.kind()
        else {
            panic!("expected call expression");
        };
        assert_eq!(args.len(), 2);
        assert_eq!(
            argument_names.as_deref(),
            Some(["second".to_owned(), "first".to_owned()].as_slice())
        );
        assert!(!implicit_receiver);
    }

    #[test]
    fn method_named_arguments_exclude_the_implicit_receiver() {
        let program =
            parse_module("fn main(Json value, Name key) { let found = value.get_int(key: key); }")
                .expect("parse named method call");
        let function = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function");
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected binding");
        };
        let Expr::Call {
            args,
            argument_names,
            implicit_receiver,
            ..
        } = value.kind()
        else {
            panic!("expected method call");
        };
        assert_eq!(args.len(), 2);
        assert_eq!(
            argument_names.as_deref(),
            Some(["key".to_owned()].as_slice())
        );
        assert!(implicit_receiver);
    }

    #[test]
    fn quantity_json_getter_uses_canonical_source_name_and_rejects_legacy_names() {
        let program =
            parse_module("fn main(Json value, Name key) { let found = value.get_quantity(key); }")
                .expect("parse canonical quantity JSON getter");
        let function = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function");
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected quantity getter binding");
        };
        let Expr::Call {
            name,
            implicit_receiver,
            ..
        } = value.kind()
        else {
            panic!("expected quantity getter call");
        };
        assert_eq!(name, "get_quantity");
        assert!(implicit_receiver);

        for legacy in ["get_amount", "get_numeric"] {
            let source =
                format!("fn main(Json value, Name key) {{ let found = value.{legacy}(key); }}");
            let error = parse_module(&source).expect_err("retired JSON getter must fail");
            assert!(error.contains("E_LEGACY_JSON_GETTER"), "{error}");
        }
    }

    #[test]
    fn mixed_and_duplicate_named_call_arguments_are_rejected() {
        for (source, code) in [
            (
                "fn main() { target(1, second: 2); }",
                "E_MIXED_CALL_ARGUMENTS",
            ),
            (
                "fn main() { target(first: 1, 2); }",
                "E_MIXED_CALL_ARGUMENTS",
            ),
            (
                "fn main() { target(first: 1, first: 2); }",
                "E_DUPLICATE_NAMED_ARGUMENT",
            ),
        ] {
            let error = parse_module(source).expect_err("invalid call style must fail");
            assert!(error.contains(code), "unexpected error: {error}");
        }
    }

    #[test]
    fn unshield_parser_registry_has_no_guest_output_parameter() {
        let source = SourceFile::new(SourceId(11), "unshield-parameters.ko", String::new());
        let parser = CstAstLowerer::new(&[], &source, false);
        assert_eq!(
            parser.call_parameter_names("crypto::zk::build_unshield", false),
            Some(
                [
                    "asset_definition",
                    "destination",
                    "amount",
                    "inputs",
                    "backend",
                    "proof",
                    "verification_key",
                ]
                .map(str::to_owned)
                .to_vec()
            )
        );
    }

    #[test]
    fn mixed_call_fixes_use_the_declared_parameter_mapping_in_both_directions() {
        for (id, call, original, replacement) in [
            (7, "target(1, second: 2)", "1", "first: 1"),
            (8, "target(first: 1, 2)", "", "second: "),
        ] {
            let text = format!(
                "seiyaku C {{ fn target(int first, int second) {{}} fn main() {{ {call}; }} }}"
            );
            let source = SourceFile::new(SourceId(id), "mixed.ko", text.clone());
            let diagnostics = parse_source(&source, FrontendBudget::v1())
                .expect_err("mixed call style must fail");
            let diagnostic = diagnostics
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == "E_MIXED_CALL_ARGUMENTS")
                .expect("mixed-call diagnostic");
            let fix = diagnostic.fix.as_ref().expect("contextual safe fix");
            let range = fix.span.byte_range.expect("exact fix range");
            assert_eq!(&text[range.start as usize..range.end as usize], original);
            assert_eq!(fix.replacement, replacement);

            let mut repaired = text;
            repaired.replace_range(range.start as usize..range.end as usize, &fix.replacement);
            parse(&repaired).expect("the machine fix must produce one named call style");
        }
    }

    #[test]
    fn unresolved_mixed_calls_do_not_guess_parameter_names() {
        for (id, call) in [(9, "target(1, second: 2)"), (10, "target(first: 1, 2)")] {
            let source = SourceFile::new(
                SourceId(id),
                "mixed-unknown.ko",
                format!("seiyaku C {{ fn main() {{ {call}; }} }}"),
            );
            let diagnostics = parse_source(&source, FrontendBudget::v1())
                .expect_err("mixed call style must fail before name resolution");
            let diagnostic = diagnostics
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == "E_MIXED_CALL_ARGUMENTS")
                .expect("mixed-call diagnostic");
            assert!(
                diagnostic.fix.is_none(),
                "the parser must not invent an unresolved callee's parameter mapping"
            );
            assert!(
                diagnostic
                    .help
                    .as_deref()
                    .is_some_and(|help| help.contains("does not guess"))
            );
        }
    }

    #[test]
    fn named_struct_literals_support_shorthand_and_trailing_comma() {
        let program = parse_module(
            "struct Transfer { int source, int destination, quantity amount } fn main(int source, int destination) { let value = Transfer { amount: 10, source, destination, }; }",
        )
        .expect("parse named struct literal");
        let function = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function");
        let Statement::Let { value, .. } = function.body.statements[0].kind() else {
            panic!("expected binding");
        };
        let Expr::StructLiteral { name, fields } = value.kind() else {
            panic!("expected struct literal");
        };
        assert_eq!(name, "Transfer");
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["amount", "source", "destination"]
        );
        assert!(!fields[0].shorthand);
        assert!(fields[1].shorthand && fields[2].shorthand);
    }

    #[test]
    fn duplicate_struct_literal_fields_are_rejected() {
        let error = parse_module(
            "struct Pair { int first, int second } fn main() { let pair = Pair { first: 1, first: 2, second: 3 }; }",
        )
        .expect_err("duplicate field must fail");
        assert!(error.contains("E_DUPLICATE_STRUCT_FIELD"), "{error}");
    }

    #[test]
    fn control_flow_block_is_not_parsed_as_a_struct_literal() {
        parse_module("fn main(bool ready) { if ready {} }")
            .expect("if block must remain unambiguous");
    }

    #[test]
    fn negative_unsuffixed_literal_remains_available_for_semantic_quantity_validation() {
        parse_module("fn main() { let quantity value = -10; }")
            .expect("the parser leaves nominal quantity validation to semantics");
    }

    #[test]
    fn signed_512_bit_integer_endpoints_are_accepted() {
        for spelling in [
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047",
            "-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048",
        ] {
            parse_module(&format!("fn main() {{ let int value = {spelling}; }}"))
                .expect("signed 512-bit endpoint must parse");
        }
    }

    #[test]
    fn signed_512_bit_integer_neighbors_are_rejected() {
        for spelling in [
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048",
            "-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042049",
        ] {
            let error = parse_module(&format!("fn main() {{ let int value = {spelling}; }}"))
                .expect_err("out-of-domain int literal must fail");
            assert!(error.contains("E_INT_LITERAL_OVERFLOW"), "{error}");
        }
    }

    #[test]
    fn radix_literals_use_the_same_signed_512_bit_domain() {
        let maximum_hex = format!("0x7{}", "f".repeat(127));
        let minimum_hex = format!("-0x8{}", "0".repeat(127));
        let maximum_binary = format!("0b{}", "1".repeat(511));
        let minimum_binary = format!("-0b1{}", "0".repeat(511));
        for spelling in [maximum_hex, minimum_hex, maximum_binary, minimum_binary] {
            parse_module(&format!("fn main() {{ let int value = {spelling}; }}"))
                .unwrap_or_else(|error| panic!("signed endpoint `{spelling}` failed: {error}"));
        }

        let positive_neighbor_hex = format!("0x8{}", "0".repeat(127));
        let negative_neighbor_hex = format!("-0x8{}1", "0".repeat(126));
        let positive_neighbor_binary = format!("0b1{}", "0".repeat(511));
        let negative_neighbor_binary = format!("-0b1{}1", "0".repeat(510));
        for spelling in [
            positive_neighbor_hex,
            negative_neighbor_hex,
            positive_neighbor_binary,
            negative_neighbor_binary,
        ] {
            let error = parse_module(&format!("fn main() {{ let int value = {spelling}; }}"))
                .expect_err("neighbor outside the signed domain must fail");
            assert!(
                error.contains("E_INT_LITERAL_OVERFLOW"),
                "{spelling}: {error}"
            );
        }
    }

    #[test]
    fn source_macros_are_rejected_without_ast_rewriting() {
        for src in [
            r#"fn main() { let x = account!("alice"); }"#,
            r#"fn main() { let x = json!{ value: 1 }; }"#,
            r#"fn main() { let x = blob!("bytes"); }"#,
        ] {
            let err = parse_module(src).expect_err("V1 source macro must be rejected");
            assert!(
                err.contains("macros are not part of Kotodama V1"),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn parse_tuple_index_literal() {
        let src = "fn main() { let t = (1, 2); let x = t.1; }";
        parse_module(src).expect("parse tuple index");
    }

    #[test]
    fn bounded_collection_attribute_is_rejected() {
        let src = "fn f(StateMap<int, int> m) { for (k, v) in m #[bounded(1)] { let z = k; } }";
        let error = parse_module(src).expect_err("#[bounded] is not Kotodama V1 syntax");
        assert!(error.contains("`.take(N)` or `.range(start, end)`"));
    }

    #[test]
    fn free_calls_cannot_claim_postfix_state_map_bounds() {
        for iterator in ["take(m, 1)", "range(m, 0, 1)"] {
            let source = format!(
                "fn f(StateMap<int, int> m) {{ for (k, v) in {iterator} {{ let z = k; }} }}"
            );
            let error =
                parse_module(&source).expect_err("a free call is not a StateMap bound source");
            assert!(
                error.contains(
                    "StateMap iteration requires `.take(N)` or `.range(start, end)` with int literals"
                ),
                "{iterator}: {error}"
            );
        }
    }

    #[test]
    fn parse_compound_assignment_keeps_rhs() {
        let src = "fn f() { m[0] += 1; }";
        let prog = parse_module(src).expect("parse compound assignment");
        let func = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(f) => Some(f),
                _ => None,
            })
            .expect("function present");
        let stmt = func.body.statements.first().expect("statement present");
        match stmt.kind() {
            Statement::AssignExpr { op, value, .. } => {
                assert_eq!(*op, AssignOp::Add);
                assert!(matches!(value.kind(), Expr::IntLiteral(value) if value == &BigInt::one()));
            }
            other => panic!("expected compound assignment, got {other:?}"),
        }
    }

    #[test]
    fn parse_bytes_literal() {
        let src = r#"fn main() { let b = b"ab"; }"#;
        let prog = parse_module(src).expect("parse bytes literal");
        let func = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(f) => Some(f),
                _ => None,
            })
            .expect("function present");
        let stmt = func.body.statements.first().expect("statement present");
        match stmt.kind() {
            Statement::Let { value, .. } => match value.kind() {
                Expr::Bytes(bytes) => assert_eq!(bytes, b"ab"),
                other => panic!("expected bytes literal, got {other:?}"),
            },
            other => panic!("expected let statement, got {other:?}"),
        }
    }

    #[test]
    fn parse_access_attributes_are_rejected() {
        let src = r#"
        #[access(read="state:Foo", write=["state:Foo/1", "state:Foo/2"])]
        fn main() {}
        "#;
        let err = parse_module(src).expect_err("manual access attributes should be rejected");
        assert!(err.contains("access metadata is generated by the compiler"));
    }

    #[test]
    fn parse_rejects_state_parameter_annotations() {
        let src = r#"
        fn helper(state StateMap<Name, int> balances, Name key) {}
        "#;
        let err = parse_module(src).expect_err("state parameters must be rejected");
        assert!(err.contains("state handles are not first-class parameters"));
    }

    #[test]
    fn parse_rejects_removed_free_map_helpers() {
        let err = parse_module("fn f(StateMap<int, int> m) { let _x = get_or(m, 1, 7); }")
            .expect_err("free get_or should be rejected");
        assert!(
            err.contains("map.get_or(key, default)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn state_map_get_method_preserves_call_form_for_resolution() {
        let program = parse_module(
            "fn get(int value) -> int { return value; } \
             fn use_get(StateMap<int, int> map) { \
                 let optional = map.get(1); \
                 let ordinary = get(1); \
             }",
        )
        .expect("method and free calls should parse");
        let Item::Function(function) = &program.items[1] else {
            panic!("expected use_get function");
        };
        let Statement::Let {
            value: method_call, ..
        } = function.body.statements[0].kind()
        else {
            panic!("expected StateMap.get call");
        };
        let Expr::Call { name: method, .. } = method_call.kind() else {
            panic!("expected StateMap.get call expression");
        };
        let Statement::Let {
            value: free_call, ..
        } = function.body.statements[1].kind()
        else {
            panic!("expected free get call");
        };
        let Expr::Call { name: free, .. } = free_call.kind() else {
            panic!("expected free get call expression");
        };
        assert_eq!(method, STATE_MAP_GET_INTRINSIC);
        assert_eq!(free, "get");
    }

    #[test]
    fn parse_rejects_removed_free_json_helpers() {
        let err = parse_module("fn f(Json ev) { let _x = get_int(ev, Name::parse(\"n\")); }")
            .expect_err("free get_int should be rejected");
        assert!(err.contains("json.get_int(key)"), "unexpected error: {err}");
    }

    #[test]
    fn parse_rejects_free_sum_type_helpers() {
        for expression in [
            "is_some(value)",
            "unwrap_or(value, 0)",
            "option_some(1)",
            "result_err(1)",
            "state_map_get(map, 1)",
        ] {
            let error = parse_module(&format!(
                "fn f(Option<int> value, StateMap<int, int> map) {{ let _x = {expression}; }}"
            ))
            .expect_err("flat sum/state helper must be rejected by the V1 parser");
            assert!(
                error.contains("method-only")
                    || error.contains("not part of Kotodama V1")
                    || error.contains("compiler-internal"),
                "unexpected error for `{expression}`: {error}"
            );
        }
    }

    #[test]
    fn parse_rejects_removed_method_map_aliases() {
        let err = parse_module("fn f(StateMap<int, int> m) { let _x = m.has(1); }")
            .expect_err("method has should be rejected");
        assert!(err.contains("map.contains(key)"), "unexpected error: {err}");

        let err =
            parse_module("fn f(StateMap<int, int> m) { let _x = m.get_or_insert_default(1, 7); }")
                .expect_err("method get_or_insert_default should be rejected");
        assert!(
            err.contains("map.ensure(key, default)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_rejects_removed_method_path_and_json_aliases() {
        let err = parse_module("fn f(Name base) { let _x = base.path_map_key(7); }")
            .expect_err("method path_map_key should be rejected");
        assert!(
            err.contains("base.path(segment)"),
            "unexpected error: {err}"
        );

        let err = parse_module("fn f(Json ev) { let _x = ev.json_get_int(Name::parse(\"n\")); }")
            .expect_err("method json_get_int should be rejected");
        assert!(err.contains("json.get_int(key)"), "unexpected error: {err}");
    }

    #[test]
    fn parse_rejects_constructor_method_aliases() {
        for source in [
            r#"module M { fn f(string value) { let _id = value.account_id(); } }"#,
            r#"module M { fn f(string value) { let _name = value.name(); } }"#,
            r#"module M { fn f(string value) { let _json = value.json(); } }"#,
            r#"module M { fn f(bytes value) { let _raw = value.norito_bytes(); } }"#,
        ] {
            let error = parse(source).expect_err("constructor method alias must be rejected");
            assert!(
                error.contains("constructor method aliases were removed"),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn source_localization_tables_are_not_part_of_v1() {
        for spelling in ["messages", "kotoba"] {
            let source =
                format!(r#"module Localization {{ {spelling} {{ key: {{ en: "value" }} }} }}"#);
            let error = parse(&source).expect_err("source localization tables must be rejected");
            assert!(
                error.contains("source-unit item"),
                "unexpected diagnostic for {spelling}: {error}"
            );
        }
    }

    #[test]
    fn parse_trigger_decl() {
        let authority = sample_account_literal();
        let src = format!(
            r#"
        seiyaku C {{
            kotoage fn run() authorize("Run") {{}}
            trigger wake -> run {{
                on time pre_commit;
                repeats 3;
                authority "{authority}";
                metadata {{ tag: "alpha"; count: 1; enabled: true; }}
            }}
        }}
        "#
        );
        let prog = parse(&src).expect("parse trigger decl");
        let trigger = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Trigger(t) => Some(t),
                _ => None,
            })
            .expect("trigger present");
        assert_eq!(trigger.name, "wake");
        assert_eq!(trigger.call.entrypoint, "run");
        assert!(matches!(trigger.filter, TriggerFilter::Time(_)));
        assert_eq!(trigger.authority.as_deref(), Some(authority.as_str()));
        assert_eq!(trigger.metadata.len(), 3);
    }

    #[test]
    fn trigger_declarations_require_arrow_target_syntax() {
        for source in [
            "seiyaku Demo { register_trigger wake { on execute Name::parse(\"tick\"); } }",
            "seiyaku Demo { trigger wake { call run; on execute Name::parse(\"tick\"); } }",
        ] {
            parse(source).expect_err("retired trigger declaration syntax must fail");
        }
    }

    #[test]
    fn call_statement_sugar_is_rejected() {
        parse("seiyaku Demo { fn run() { call helper(); } fn helper() {} }")
            .expect_err("statement-level call sugar must fail");
    }

    #[test]
    fn parse_trigger_decl_rejects_duplicate_control_fields() {
        for (field, duplicate_line, expected) in [
            ("on", "on time pre_commit;", "duplicate `on` field"),
            ("repeats", "repeats 2;", "duplicate `repeats` field"),
            (
                "authority",
                r#"authority "alice";"#,
                "duplicate `authority` field",
            ),
        ] {
            let src = format!(
                r#"
            seiyaku C {{
                kotoage fn run() authorize("Run") {{}}
                trigger wake -> run {{
                    on time pre_commit;
                    repeats 1;
                    authority "bob";
                    {duplicate_line}
                }}
            }}
            "#
            );
            let err = parse(&src).unwrap_err();
            assert!(err.contains(expected), "{field}: unexpected error: {err}");
        }
    }

    #[test]
    fn parse_trigger_decl_rejects_negative_and_overflow_repeats() {
        for (repeats, expected) in [
            ("-1", "repeats expects a non-negative integer literal"),
            ("4294967296", "repeats integer literal out of range"),
        ] {
            let src = format!(
                r#"
            seiyaku C {{
                kotoage fn run() authorize("Run") {{}}
                trigger wake -> run {{
                    on time pre_commit;
                    repeats {repeats};
                }}
            }}
            "#
            );
            let err = parse(&src).unwrap_err();
            assert!(err.contains(expected), "unexpected error: {err}");
        }
    }

    #[test]
    fn parse_trigger_decl_with_data_filter() {
        let src = r#"
        seiyaku C {
            kotoage fn run() authorize("Run") {}
            trigger wake -> run {
                on data any;
            }
        }
        "#;
        let prog = parse(src).expect("parse trigger decl");
        let trigger = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Trigger(t) => Some(t),
                _ => None,
            })
            .expect("trigger present");
        assert!(matches!(trigger.filter, TriggerFilter::Data(_)));
    }

    fn sample_asset_definition_literal() -> String {
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        )
        .to_string()
    }

    #[test]
    fn parse_trigger_decl_with_structured_data_filter() {
        let asset_definition = sample_asset_definition_literal();
        let src = format!(
            r#"
        seiyaku C {{
            kotoage fn run() authorize("Run") {{}}
            trigger wake -> run {{
                on data asset added {{
                    asset_definition "{asset_definition}";
                }}
            }}
        }}
        "#
        );
        let prog = parse(&src).expect("parse trigger decl");
        let trigger = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Trigger(t) => Some(t),
                _ => None,
            })
            .expect("trigger present");
        let TriggerFilter::Data(TriggerDataFilter::Structured(filter)) = &trigger.filter else {
            panic!("expected structured data filter");
        };
        assert_eq!(filter.family, TriggerDataFamily::Asset);
        assert_eq!(
            filter.event,
            TriggerDataEventKind::Named("added".to_string())
        );
        assert_eq!(filter.matchers.len(), 1);
        assert_eq!(filter.matchers[0].key, "asset_definition");
        assert_eq!(filter.matchers[0].value, asset_definition);
    }

    include!("parser/tests/trigger_filter_core_families.rs");

    #[test]
    fn parse_trigger_decl_rejects_nondeterministic_pipeline_filter() {
        let src = r#"
        seiyaku C {
            kotoage fn run() authorize("Run") {}
            trigger wake -> run {
                on pipeline merge;
            }
        }
        "#;
        let err = parse(src).expect_err("parse should reject unsupported pipeline filter");
        assert!(err.contains("transaction [approved]"));
    }

    #[test]
    fn parse_koto_test_target_fixture_and_test_binding() {
        let src = r#"
        module ContractTests {
            koto_test { target: "contracts/demo.ko" }

            fixture seeded {
                caller(AccountId::parse("alice@wonderland"));
                grant_permission("register_domain");
            }

            #[test(fixture="seeded")]
            fn smoke() {}
        }
        "#;
        let prog = parse(src).expect("parse koto_test program");
        assert_eq!(
            prog.test_target
                .as_ref()
                .map(|target| target.target.as_str()),
            Some("contracts/demo.ko")
        );
        assert_eq!(prog.fixtures.len(), 1);
        assert_eq!(prog.fixtures[0].name, "seeded");
        assert_eq!(prog.fixtures[0].actions.len(), 2);

        let func = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(f) => Some(f),
                _ => None,
            })
            .expect("function present");
        assert!(func.modifiers.is_test);
        assert_eq!(func.modifiers.test_fixture.as_deref(), Some("seeded"));
    }

    include!("parser/tests/tail_fixtures.rs");
}
