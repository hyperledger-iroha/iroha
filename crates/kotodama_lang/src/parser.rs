//! Parser for the Kotodama language.
//!
//! This module implements a simple recursive descent parser producing an AST.

use super::{
    ast::*,
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticPhase, MAX_DIAGNOSTICS, SourcePosition, SourceSpan,
    },
    lexer::{Token, TokenKind},
    source::{FrontendBudget, SourceFile, SourceId, TextRange},
};

#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub snippet: String,
    /// Exact half-open UTF-8 range of the unexpected token.
    pub range: TextRange,
}

type ParseResult<T> = Result<T, ParseError>;
type ForEachMapBinding = (String, Option<String>, Expr);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum IntegerSuffix {
    #[default]
    None,
    I64,
    U128,
}

fn map_iteration_has_explicit_bound(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { name, args }
            if (name == "take" && args.len() == 2)
                || (name == "range" && args.len() == 3)
    )
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
) -> Result<(Program, Vec<Token>), DiagnosticBundle> {
    let output = crate::syntax::parse_program(source, budget);
    output
        .program
        .map(|program| (program, output.tokens))
        .ok_or(output.diagnostics)
}

pub(crate) fn parse_tokens(
    source: &SourceFile,
    tokens: &[Token],
) -> Result<Program, DiagnosticBundle> {
    let mut parser = Parser::new(tokens, source.text(), true);
    let parsed = parser.parse_program();
    let mut errors = parser.errors;
    match parsed {
        Ok(program) if errors.is_empty() => Ok(program),
        Ok(_) => Err(parse_diagnostic_bundle(source, errors)),
        Err(error) => {
            errors.push(error);
            Err(parse_diagnostic_bundle(source, errors))
        }
    }
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
                "K1001",
                DiagnosticPhase::Parse,
                error.message,
                Some(SourceSpan {
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

/// Wrap a unit-test fragment in a canonical contract container before parsing.
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

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    source: &'a str,
    test_target: Option<TestTargetDecl>,
    fixtures: Vec<FixtureDecl>,
    recover: bool,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], source: &'a str, recover: bool) -> Self {
        Self {
            tokens,
            pos: 0,
            source,
            test_target: None,
            fixtures: Vec::new(),
            recover,
            errors: Vec::new(),
        }
    }

    fn parse_program(&mut self) -> ParseResult<Program> {
        let kind = if self.peek(TokenKind::Seiyaku) {
            SourceUnitKind::Contract
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
        self.bump(); // `seiyaku`/`誓約` or `module`
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let item_start = self.pos;
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
                            visibility: FunctionVisibility::Internal,
                            kind: FunctionKind::Contract,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                    )?);
                } else if self.peek(TokenKind::Kotoage) && self.peek_n(1, TokenKind::Fn) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare entrypoints",
                        ));
                    }
                    self.bump(); // kotoage / 言挙げ
                    self.bump(); // fn
                    items.push(self.parse_fn_loose(
                        None,
                        FunctionModifiers {
                            visibility: FunctionVisibility::Public,
                            kind: FunctionKind::Contract,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                    )?);
                } else if self.peek(TokenKind::View) && self.peek_n(1, TokenKind::Fn) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare entrypoints",
                        ));
                    }
                    self.bump(); // view
                    self.bump(); // fn
                    items.push(self.parse_fn_loose(
                        None,
                        FunctionModifiers {
                            visibility: FunctionVisibility::Public,
                            kind: FunctionKind::View,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                    )?);
                } else if self.peek(TokenKind::Hajimari) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare an initializer",
                        ));
                    }
                    self.bump();
                    items.push(self.parse_fn_loose(
                        Some(String::from("hajimari")),
                        FunctionModifiers {
                            visibility: FunctionVisibility::Public,
                            kind: FunctionKind::Init,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
                    )?);
                } else if self.peek(TokenKind::Kaizen) {
                    if kind == SourceUnitKind::Module {
                        return Err(self.error(
                            self.tokens[self.pos].clone(),
                            "module units cannot declare a kaizen hook",
                        ));
                    }
                    self.bump();
                    items.push(self.parse_fn_loose(
                        Some(String::from("kaizen")),
                        FunctionModifiers {
                            visibility: FunctionVisibility::Public,
                            kind: FunctionKind::Upgrade,
                            permission: None,
                            access_reads: attrs.reads,
                            access_writes: attrs.writes,
                            is_test: attrs.is_test,
                            test_fixture: attrs.test_fixture,
                        },
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
                    return Err(error);
                }
                self.errors.push(error);
                self.synchronize_source_item(item_start);
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok((SourceUnit { kind, name }, items))
    }

    fn parse_error_enum_def(&mut self) -> ParseResult<Item> {
        self.expect(TokenKind::Error)?;
        self.expect(TokenKind::Enum)?;
        let name = self.expect_ident()?;
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
            let code = match code_token.kind {
                TokenKind::Number(value) if (1..=u128::from(u32::MAX)).contains(&value) => {
                    value as u32
                }
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
        Ok(Item::ErrorEnum(ErrorEnumDef { name, variants }))
    }

    fn parse_trigger_decl(&mut self) -> ParseResult<Item> {
        let tok = self.bump();
        debug_assert!(matches!(tok.kind, TokenKind::Trigger));
        let name = self.expect_ident()?;
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
        Ok(Item::Trigger(TriggerDecl {
            name,
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
            TokenKind::Number(n) => {
                if self.consume_integer_suffix()? == IntegerSuffix::U128 {
                    return Err(self.error(
                        tok,
                        &format!("{context} expects an i64-domain integer literal"),
                    ));
                }
                u64::try_from(n).map_err(|_| {
                    self.range_error(&tok, format!("{context} integer literal out of range"))
                })
            }
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
            self.expect(TokenKind::RBracket)?;
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
                    return Err(ParseError {
                        message: format!("unknown access list `{key}`"),
                        line: self.tokens[self.pos.saturating_sub(1)].line,
                        column: self.tokens[self.pos.saturating_sub(1)].column,
                        snippet: String::new(),
                        range: self.tokens[self.pos.saturating_sub(1)].range,
                    });
                }
            }
            parsed_any = true;
            if self.peek(TokenKind::Comma) {
                self.bump();
            }
        }
        if !parsed_any {
            return Err(ParseError {
                message: "access attribute must include read/write entries".into(),
                line: self.tokens[self.pos.saturating_sub(1)].line,
                column: self.tokens[self.pos.saturating_sub(1)].column,
                snippet: String::new(),
                range: self.tokens[self.pos.saturating_sub(1)].range,
            });
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
                        return Err(ParseError {
                            message: "duplicate fixture binding in test attribute".into(),
                            line: self.tokens[self.pos.saturating_sub(1)].line,
                            column: self.tokens[self.pos.saturating_sub(1)].column,
                            snippet: String::new(),
                            range: self.tokens[self.pos.saturating_sub(1)].range,
                        });
                    }
                    attrs.test_fixture = Some(self.expect_ident_or_string()?);
                }
                _ => {
                    return Err(ParseError {
                        message: format!("unknown test attribute option `{key}`"),
                        line: self.tokens[self.pos.saturating_sub(1)].line,
                        column: self.tokens[self.pos.saturating_sub(1)].column,
                        snippet: String::new(),
                        range: self.tokens[self.pos.saturating_sub(1)].range,
                    });
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
            message: "koto_test block requires `target: \"...\"`".into(),
            line: tok.line,
            column: tok.column,
            snippet: String::new(),
            range: tok.range,
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
        // struct Name { field: Type, ... }
        self.expect(TokenKind::Struct)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            // Allow stray separators.
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
                continue;
            }
            // Expect: ident ':' type ';'
            let field_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            fields.push((field_name, ty));
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Item::Struct(super::ast::StructDef { name, fields }))
    }

    fn parse_state_decl(&mut self) -> ParseResult<Item> {
        // Canonical V1 form: `state name: Type;`.
        self.expect(TokenKind::State)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type_expr()?;
        self.expect(TokenKind::Semicolon)?;
        Ok(Item::State(super::ast::StateDecl { name, ty }))
    }

    fn parse_const_decl(&mut self) -> ParseResult<Item> {
        self.expect(TokenKind::Const)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = Some(self.parse_type_expr()?);
        self.expect(TokenKind::Equal)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semicolon)?;
        Ok(Item::Const(super::ast::ConstDecl { name, ty, value }))
    }

    fn parse_fn_loose(
        &mut self,
        name_override: Option<String>,
        mut modifiers: FunctionModifiers,
    ) -> ParseResult<Item> {
        let location = if name_override.is_some() {
            let tok = &self.tokens[self.pos.saturating_sub(1)];
            SourceLocation {
                line: tok.line,
                column: tok.column,
            }
        } else {
            let tok = &self.tokens[self.pos];
            SourceLocation {
                line: tok.line,
                column: tok.column,
            }
        };
        let name = match name_override {
            Some(n) => n,
            None => self.expect_ident()?,
        };
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.peek(TokenKind::RParen) {
            loop {
                params.push(self.parse_param()?);
                if self.peek(TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen)?;
        let mut ret_ty = None;
        if self.peek(TokenKind::Arrow) {
            self.bump();
            ret_ty = Some(self.parse_type_expr()?);
        }
        // Optional caller-authorization modifier.
        while !self.peek(TokenKind::LBrace) && !self.peek(TokenKind::EOF) {
            if self.peek(TokenKind::Authorize) {
                if matches!(modifiers.kind, FunctionKind::Init | FunctionKind::Upgrade) {
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
                    TokenKind::String(permission) if !permission.trim().is_empty() => permission,
                    TokenKind::String(_) => {
                        return Err(
                            self.error(permission_token, "non-empty permission string literal")
                        );
                    }
                    _ => return Err(self.error(permission_token, "permission string literal")),
                };
                self.expect(TokenKind::RParen)?;
                if modifiers.visibility != FunctionVisibility::Public {
                    return Err(ParseError {
                        message: "`authorize(...)` is only valid on entrypoints".into(),
                        line: self.tokens[self.pos.saturating_sub(1)].line,
                        column: self.tokens[self.pos.saturating_sub(1)].column,
                        snippet: String::new(),
                        range: self.tokens[self.pos.saturating_sub(1)].range,
                    });
                }
                if modifiers.permission.is_some() {
                    return Err(ParseError {
                        message: "duplicate authorize modifier".into(),
                        line: self.tokens[self.pos.saturating_sub(1)].line,
                        column: self.tokens[self.pos.saturating_sub(1)].column,
                        snippet: String::new(),
                        range: self.tokens[self.pos.saturating_sub(1)].range,
                    });
                }
                modifiers.permission = Some(perm);
            } else {
                let tok = self.bump();
                return Err(self.error(tok, "`authorize(\"Permission\")` or `{`"));
            }
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
    }

    fn parse_block(&mut self) -> ParseResult<Block> {
        self.expect(TokenKind::LBrace)?;
        let mut statements = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let statement_start = self.pos;
            match self.parse_statement() {
                Ok(statement) => statements.push(statement),
                Err(error) if self.recover => {
                    self.errors.push(error);
                    self.synchronize_statement(statement_start);
                }
                Err(error) => return Err(error),
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Block { statements })
    }

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        if self.peek(TokenKind::Let) || self.peek(TokenKind::Var) {
            let mutable = self.peek(TokenKind::Var);
            self.bump();
            // pattern
            let pat = if self.peek(TokenKind::LParen) {
                self.bump();
                let mut names = Vec::new();
                loop {
                    names.push(self.expect_ident()?);
                    if self.peek(TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Pattern::Tuple(names)
            } else {
                Pattern::Name(self.expect_ident()?)
            };
            // optional type
            let ty = if self.peek(TokenKind::Colon) {
                self.bump();
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            self.expect(TokenKind::Equal)?;
            let expr = self.parse_expr()?;
            self.expect(TokenKind::Semicolon)?;
            Ok(Statement::Let {
                mutable,
                pat,
                ty,
                value: expr,
            })
        } else if self.peek(TokenKind::Return) {
            self.bump();
            if self.peek(TokenKind::Semicolon) {
                self.bump();
                Ok(Statement::Return(None))
            } else {
                let expr = self.parse_expr()?;
                self.expect(TokenKind::Semicolon)?;
                Ok(Statement::Return(Some(expr)))
            }
        } else if self.peek(TokenKind::Break) {
            self.bump();
            self.expect(TokenKind::Semicolon)?;
            Ok(Statement::Break)
        } else if self.peek(TokenKind::Continue) {
            self.bump();
            self.expect(TokenKind::Semicolon)?;
            Ok(Statement::Continue)
        } else if self.peek(TokenKind::If) {
            self.bump();
            let cond = self.parse_expr()?;
            let then_branch = self.parse_block()?;
            let else_branch = if self.peek(TokenKind::Else) {
                self.bump();
                if self.peek(TokenKind::If) {
                    Some(Block {
                        statements: vec![self.parse_statement()?],
                    })
                } else {
                    Some(self.parse_block()?)
                }
            } else {
                None
            };
            Ok(Statement::If {
                cond,
                then_branch,
                else_branch,
            })
        } else if self.peek(TokenKind::For) {
            let for_line = self.tokens.get(self.pos).map(|t| t.line).unwrap_or(0);
            self.expect(TokenKind::For)?;
            if let Some((init, cond, step)) = self.parse_for_range()? {
                let body = self.parse_block()?;
                Ok(Statement::For {
                    line: for_line,
                    init: Some(Box::new(init)),
                    cond: Some(cond),
                    step: Some(Box::new(step)),
                    body,
                })
            } else if let Some((k, v_opt, map)) = self.parse_for_each_map()? {
                if !map_iteration_has_explicit_bound(&map) {
                    return Err(self.error(
                        self.tokens[self.pos.saturating_sub(1)].clone(),
                        "StateMap iteration requires `.take(N)` or `.range(start, end)` with i64 literals",
                    ));
                }
                let body = self.parse_block()?;
                Ok(Statement::ForEachMap {
                    key: k,
                    value: v_opt,
                    map,
                    body,
                })
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
            if let Ok(target) = self.try_parse_lvalue_expr() {
                if self.peek(TokenKind::Equal)
                    || self.peek(TokenKind::PlusEqual)
                    || self.peek(TokenKind::MinusEqual)
                    || self.peek(TokenKind::StarEqual)
                    || self.peek(TokenKind::SlashEqual)
                    || self.peek(TokenKind::PercentEqual)
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
                            return Err(
                                self.error(op_tok, "expected one of: =, +=, -=, *=, /=, %=")
                            );
                        }
                    };
                    return Ok(match (target, op) {
                        (Expr::Ident(name), AssignOp::Set) => {
                            Statement::Assign { name, value: rhs }
                        }
                        (t, op) => Statement::AssignExpr {
                            target: t,
                            op,
                            value: rhs,
                        },
                    });
                } else {
                    // Not an assignment; rewind and continue parsing as expression
                    self.pos = save;
                }
            }
            self.pos = save;
            let expr = self.parse_expr()?;
            self.expect(TokenKind::Semicolon)?;
            Ok(Statement::Expr(expr))
        }
    }

    fn inc_statement(&self, name: String) -> Statement {
        Statement::Assign {
            name: name.clone(),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Ident(name.clone())),
                right: Box::new(Expr::Number(1)),
            },
        }
    }

    fn parse_for_range(&mut self) -> ParseResult<Option<(Statement, Expr, Statement)>> {
        let save = self.pos;
        if let Some(Token {
            kind: TokenKind::Ident(var),
            ..
        }) = self.tokens.get(self.pos).cloned()
            && self.peek_n(1, TokenKind::In)
            && self.peek_ident_n(2, "range")
        {
            self.bump();
            self.bump(); // in
            self.bump(); // range
            self.expect(TokenKind::LParen)?;
            let end = self.parse_expr()?;
            self.expect(TokenKind::RParen)?;
            if !matches!(end, Expr::Number(value) if value >= 0) {
                return Err(self.error(
                    self.tokens[self.pos.saturating_sub(1)].clone(),
                    "E_UNBOUNDED_LOOP: numeric range bounds must be non-negative integer literals",
                ));
            }
            let init = Statement::Let {
                mutable: true,
                pat: Pattern::Name(var.clone()),
                ty: None,
                value: Expr::Number(0),
            };
            let cond = Expr::Binary {
                op: BinaryOp::Lt,
                left: Box::new(Expr::Ident(var.clone())),
                right: Box::new(end),
            };
            let step = self.inc_statement(var);
            return Ok(Some((init, cond, step)));
        }
        self.pos = save;
        Ok(None)
    }

    fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.parse_conditional()
    }

    fn parse_conditional(&mut self) -> ParseResult<Expr> {
        enum Frame {
            Then { condition: Expr },
            Else { condition: Expr, then_expr: Expr },
        }

        let mut frames = Vec::new();
        let mut current = self.parse_logical_or()?;
        loop {
            if self.peek(TokenKind::Question) {
                self.bump();
                frames.push(Frame::Then { condition: current });
                current = self.parse_logical_or()?;
                continue;
            }

            match frames.pop() {
                Some(Frame::Then { condition }) => {
                    self.expect(TokenKind::Colon)?;
                    frames.push(Frame::Else {
                        condition,
                        then_expr: current,
                    });
                    current = self.parse_logical_or()?;
                }
                Some(Frame::Else {
                    condition,
                    then_expr,
                }) => {
                    current = Expr::Conditional {
                        cond: Box::new(condition),
                        then_expr: Box::new(then_expr),
                        else_expr: Box::new(current),
                    };
                }
                None => return Ok(current),
            }
        }
    }

    fn parse_logical_or(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_logical_and()?;
        loop {
            if self.peek(TokenKind::OrOr) {
                self.bump();
                let rhs = self.parse_logical_and()?;
                expr = Expr::Binary {
                    op: BinaryOp::Or,
                    left: Box::new(expr),
                    right: Box::new(rhs),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> ParseResult<Expr> {
        let mut expr = self.parse_comparison()?;
        loop {
            if self.peek(TokenKind::AndAnd) {
                self.bump();
                let rhs = self.parse_comparison()?;
                expr = Expr::Binary {
                    op: BinaryOp::And,
                    left: Box::new(expr),
                    right: Box::new(rhs),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> ParseResult<Expr> {
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
                expr = Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(rhs),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> ParseResult<Expr> {
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
                expr = Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(rhs),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> ParseResult<Expr> {
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
                expr = Expr::Binary {
                    op,
                    left: Box::new(expr),
                    right: Box::new(rhs),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        let mut prefixes = Vec::new();
        loop {
            if self.peek(TokenKind::Minus) {
                let minus = self.bump();
                if let Some(token) = self.tokens.get(self.pos).cloned()
                    && let TokenKind::Number(n) = token.kind.clone()
                    && n > i64::MAX as u128
                {
                    self.bump();
                    if self.consume_integer_suffix()? == IntegerSuffix::U128 {
                        return Err(
                            self.error(token, "u128 literals cannot be negated; u128 is unsigned")
                        );
                    }
                    let value = self.number_to_i64_neg(&token, n)?;
                    let mut expr = Expr::Number(value);
                    for (op, token) in prefixes.into_iter().rev() {
                        if op == UnaryOp::Neg && matches!(expr, Expr::Decimal(_)) {
                            return Err(self.error(
                                token,
                                "u128 literals cannot be negated; u128 is unsigned",
                            ));
                        }
                        expr = Expr::Unary {
                            op,
                            expr: Box::new(expr),
                        };
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

        let primary = self.parse_primary()?;
        let mut expr = self.parse_postfix(primary)?;
        for (op, token) in prefixes.into_iter().rev() {
            if op == UnaryOp::Neg && matches!(expr, Expr::Decimal(_)) {
                return Err(self.error(token, "u128 literals cannot be negated; u128 is unsigned"));
            }
            expr = Expr::Unary {
                op,
                expr: Box::new(expr),
            };
        }
        Ok(expr)
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> ParseResult<Expr> {
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
                            let index = self.number_to_usize(&token, n, "tuple index")?;
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
                        return Err(self.error(token.clone(), message));
                    }
                    self.bump();
                    let mut args = Vec::new();
                    if !self.peek(TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.peek(TokenKind::Comma) {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    // Prepend the receiver as the first argument
                    let mut full_args = Vec::with_capacity(args.len() + 1);
                    full_args.push(expr);
                    full_args.extend(args);
                    let call_name = if field == "get" {
                        STATE_MAP_GET_INTRINSIC.to_owned()
                    } else {
                        field
                    };
                    expr = Expr::Call {
                        name: call_name,
                        args: full_args,
                    };
                } else {
                    expr = Expr::Member {
                        object: Box::new(expr),
                        field,
                    };
                }
            } else if self.peek(TokenKind::LBracket) {
                self.bump();
                let idx = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(idx),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expr> {
        let tok = self.bump();
        match &tok.kind {
            TokenKind::True => Ok(Expr::Bool(true)),
            TokenKind::False => Ok(Expr::Bool(false)),
            TokenKind::Number(n) => match self.consume_integer_suffix()? {
                IntegerSuffix::U128 => Ok(Expr::Decimal(n.to_string())),
                IntegerSuffix::None | IntegerSuffix::I64 => {
                    let value = self.number_to_i64(&tok, *n)?;
                    Ok(Expr::Number(value))
                }
            },
            TokenKind::String(s) => Ok(Expr::String(s.clone())),
            TokenKind::Bytes(bytes) => Ok(Expr::Bytes(bytes.clone())),
            TokenKind::Ident(name) => self.parse_named_primary(tok.clone(), name.clone()),
            TokenKind::State if self.peek(TokenKind::ColonColon) => {
                self.parse_named_primary(tok.clone(), "state".to_owned())
            }
            TokenKind::LParen => self.parse_parenthesized(tok),
            _ => Err(self.error(tok, "expression")),
        }
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
            return Err(ParseError {
                message: "source-level unit value `()` is not part of Kotodama V1; omit a return value instead"
                    .into(),
                line: opening.line,
                column: opening.column,
                snippet: format!("{line_text}\n{caret}"),
                range: TextRange::new(opening.range.start, closing.range.end),
            });
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
        // Keyword tokens stay reserved as bindings and declarations. The two
        // canonical V1 namespace positions that intentionally use keywords are
        // admitted only while parsing a `::` path.
        while self.peek(TokenKind::ColonColon) {
            self.bump();
            let segment = self.expect_namespace_segment()?;
            name.push_str("::");
            name.push_str(&segment);
        }
        if self.peek(TokenKind::Bang) {
            return Err(self.error(
                ident_token,
                "macros are not part of Kotodama V1; use an ordinary typed constructor such as `AccountId::parse(\"...\")`, `Json::parse(\"{...}\")`, or a `b\"...\"` bytes literal",
            ));
        }
        if self.peek(TokenKind::LParen) {
            if let Some(message) = removed_free_helper_message(&name) {
                return Err(self.error(ident_token, message));
            }
            self.bump();
            let mut args = Vec::new();
            if !self.peek(TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if self.peek(TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen)?;
            Ok(Expr::Call { name, args })
        } else {
            Ok(Expr::Ident(name))
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        let tok = self.bump();
        match &tok.kind {
            TokenKind::Ident(name) => Ok(name.clone()),
            _ => Err(self.error(tok, "identifier")),
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
            _ => Err(self.error(tok, "namespace segment")),
        }
    }

    fn parse_type_expr(&mut self) -> ParseResult<TypeExpr> {
        enum Frame {
            Generic { base: String, args: Vec<TypeExpr> },
            Tuple { opening: Token, args: Vec<TypeExpr> },
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

                let base = self.expect_ident()?;
                if self.peek(TokenKind::Less) {
                    self.bump();
                    if self.peek(TokenKind::Greater) {
                        self.bump();
                        break TypeExpr::Generic {
                            base,
                            args: Vec::new(),
                        };
                    }
                    frames.push(Frame::Generic {
                        base,
                        args: Vec::new(),
                    });
                    continue;
                }
                break TypeExpr::Path(base);
            };

            loop {
                let Some(frame) = frames.pop() else {
                    return Ok(current);
                };
                match frame {
                    Frame::Generic { base, mut args } => {
                        args.push(current);
                        if self.peek(TokenKind::Comma) {
                            self.bump();
                            frames.push(Frame::Generic { base, args });
                            continue 'next_type;
                        }
                        self.expect(TokenKind::Greater)?;
                        current = TypeExpr::Generic { base, args };
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
                        current = TypeExpr::Tuple(args);
                    }
                }
            }
        }
    }

    fn tuple_type_arity_error(&self, opening: &Token, closing: &Token) -> ParseError {
        let line_text = self
            .source
            .lines()
            .nth(opening.line.saturating_sub(1))
            .unwrap_or("");
        let caret = " ".repeat(opening.column.saturating_sub(1)) + "^";
        ParseError {
            message: "tuple types require at least two elements; omit the return type for Unit"
                .into(),
            line: opening.line,
            column: opening.column,
            snippet: format!("{line_text}\n{caret}"),
            range: TextRange::new(opening.range.start, closing.range.end),
        }
    }

    fn try_parse_lvalue_expr(&mut self) -> ParseResult<Expr> {
        // Parse an identifier then tail of member/index chains
        let name = self.expect_ident()?;
        let mut expr = Expr::Ident(name);
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
                    let index = self.number_to_usize(&token, n, "tuple index")?;
                    index.to_string()
                } else {
                    let tok = self.bump();
                    return Err(self.error(tok, "identifier or tuple index"));
                };
                expr = Expr::Member {
                    object: Box::new(expr),
                    field,
                };
            } else if self.peek(TokenKind::LBracket) {
                self.bump();
                let idx = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(idx),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_for_each_map(&mut self) -> ParseResult<Option<ForEachMapBinding>> {
        // Patterns: (k, v) in <expr>  OR  k in <expr>
        let save = self.pos;
        if self.peek(TokenKind::LParen) {
            self.bump();
            let k = self.expect_ident()?;
            self.expect(TokenKind::Comma)?;
            let v = self.expect_ident()?;
            self.expect(TokenKind::RParen)?;
            if self.peek(TokenKind::In) {
                self.bump();
                let map = self.parse_expr()?;
                return Ok(Some((k, Some(v), map)));
            }
        } else if let Some(Token {
            kind: TokenKind::Ident(k),
            ..
        }) = self.tokens.get(self.pos).cloned()
            && self.peek_n(1, TokenKind::In)
        {
            self.bump();
            self.bump(); // in
            let map = self.parse_expr()?;
            return Ok(Some((k, None, map)));
        }
        self.pos = save;
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
        // Canonical V1 form: `name: Type`.
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let (is_state, ty) = self.parse_param_type_annotation()?;
        Ok(Param {
            ty: Some(ty),
            name,
            is_state,
        })
    }

    fn expect(&mut self, kind: TokenKind) -> ParseResult<()> {
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
            Err(self.error(tok, &format!("{kind:?}")))
        }
    }

    fn consume_integer_suffix(&mut self) -> ParseResult<IntegerSuffix> {
        if let Some(Token {
            kind: TokenKind::Ident(suffix),
            range,
            ..
        }) = self.tokens.get(self.pos)
            && self
                .tokens
                .get(self.pos.saturating_sub(1))
                .is_some_and(|previous| previous.range.end == range.start)
        {
            if suffix == "i64" {
                self.bump();
                return Ok(IntegerSuffix::I64);
            } else if suffix == "u128" {
                self.bump();
                return Ok(IntegerSuffix::U128);
            } else if suffix.starts_with('i') || suffix.starts_with('u') {
                let tok = self.bump();
                let line_text = self
                    .source
                    .lines()
                    .nth(tok.line.saturating_sub(1))
                    .unwrap_or("");
                let caret = " ".repeat(tok.column.saturating_sub(1)) + "^";
                return Err(ParseError {
                    message: format!("unknown integer literal suffix `{suffix}`"),
                    line: tok.line,
                    column: tok.column,
                    snippet: format!("{line_text}\n{caret}"),
                    range: tok.range,
                });
            }
        }
        Ok(IntegerSuffix::None)
    }

    fn number_to_i64(&self, token: &Token, value: u128) -> ParseResult<i64> {
        if value <= i64::MAX as u128 {
            Ok(value as i64)
        } else {
            Err(self.range_error(
                token,
                format!("integer literal out of range (max {})", i64::MAX),
            ))
        }
    }

    fn number_to_i64_neg(&self, token: &Token, value: u128) -> ParseResult<i64> {
        let max_plus_one = i64::MAX as u128 + 1;
        if value <= i64::MAX as u128 {
            Ok(-(value as i64))
        } else if value == max_plus_one {
            Ok(i64::MIN)
        } else {
            Err(self.range_error(
                token,
                format!("integer literal out of range (min {})", i64::MIN),
            ))
        }
    }

    fn number_to_usize(&self, token: &Token, value: u128, context: &str) -> ParseResult<usize> {
        if value <= i64::MAX as u128 && value <= usize::MAX as u128 {
            Ok(value as usize)
        } else {
            Err(self.range_error(token, format!("{context} integer literal out of range")))
        }
    }

    fn range_error(&self, token: &Token, message: String) -> ParseError {
        let line_text = self
            .source
            .lines()
            .nth(token.line.saturating_sub(1))
            .unwrap_or("");
        let caret = " ".repeat(token.column.saturating_sub(1)) + "^";
        ParseError {
            message,
            line: token.line,
            column: token.column,
            snippet: format!("{line_text}\n{caret}"),
            range: token.range,
        }
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
            TokenKind::Struct
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

    fn error(&self, token: Token, expected: &str) -> ParseError {
        let line_text = self.source.lines().nth(token.line - 1).unwrap_or("");
        let caret = " ".repeat(token.column.saturating_sub(1)) + "^";
        ParseError {
            message: format!("expected {expected} but found {kind:?}", kind = token.kind),
            line: token.line,
            column: token.column,
            snippet: format!("{line_text}\n{caret}"),
            range: token.range,
        }
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
        "json_get_numeric" => {
            Some("`json.json_get_numeric(key)` was removed; use `json.get_numeric(key)`")
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

fn removed_free_helper_message(name: &str) -> Option<&'static str> {
    match name {
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
        "get_numeric" | "json_get_numeric" | "json::get_numeric" => {
            Some("`get_numeric(...)` was removed as a free helper; use `json.get_numeric(key)`")
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

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::DomainId;

    fn parse_module(body: &str) -> Result<Program, String> {
        parse(&format!("module TestModule {{ {body} }}"))
    }

    fn sample_account_literal() -> String {
        iroha_data_model::account::AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
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
        match &f.body.statements[0] {
            Statement::Return(None) => {}
            _ => panic!("no return;"),
        }
        match &f.body.statements[1] {
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
        let Statement::Return(Some(Expr::Conditional {
            then_expr,
            else_expr,
            ..
        })) = &function.body.statements[0]
        else {
            panic!("expected outer conditional return");
        };
        assert!(
            matches!(then_expr.as_ref(), Expr::Conditional { .. }),
            "the nested then arm must bind to the outer conditional"
        );
        assert!(
            matches!(else_expr.as_ref(), Expr::Conditional { .. }),
            "the nested else arm must bind to the outer conditional"
        );
    }

    #[test]
    fn iterative_type_parser_preserves_nested_generic_and_tuple_shapes() {
        let program = parse_module("struct Wrapper { value: Result<Option<i64>, (bool, string)> }")
            .expect("parse nested type");
        let Item::Struct(definition) = &program.items[0] else {
            panic!("expected struct item");
        };
        let TypeExpr::Generic { base, args } = &definition.fields[0].1 else {
            panic!("expected Result generic");
        };
        assert_eq!(base, "Result");
        assert!(matches!(
            &args[0],
            TypeExpr::Generic { base, args }
                if base == "Option" && matches!(args.as_slice(), [TypeExpr::Path(path)] if path == "i64")
        ));
        assert!(matches!(
            &args[1],
            TypeExpr::Tuple(elements)
                if matches!(elements.as_slice(), [TypeExpr::Path(left), TypeExpr::Path(right)] if left == "bool" && right == "string")
        ));
    }

    #[test]
    fn rejects_source_unit_values_and_degenerate_tuple_types() {
        for (body, expected) in [
            (
                "fn invalid() { let value = (); }",
                "source-level unit value `()` is not part of Kotodama V1",
            ),
            (
                "fn invalid(value: ()) {}",
                "tuple types require at least two elements",
            ),
            (
                "fn invalid(value: (i64)) {}",
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
            "fn grouped() -> i64 { return (1); } fn pair(value: (i64, bool)) -> (i64, bool) { return (1, true); } fn omitted() { return; }",
        )
        .expect("grouping, real tuples, and omitted Unit returns remain valid");
        let Item::Function(grouped_function) = &grouped.items[0] else {
            panic!("expected grouped function")
        };
        assert!(matches!(
            &grouped_function.body.statements[0],
            Statement::Return(Some(Expr::Number(1)))
        ));
    }

    #[test]
    fn else_if_is_represented_as_a_nested_if_in_the_else_block() {
        let program = parse_module(
            "fn classify(value: i64) -> i64 { if value < 0 { return -1; } else if value == 0 { return 0; } else { return 1; } }",
        )
        .expect("parse documented else-if chain");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function")
        };
        let Statement::If {
            else_branch: Some(outer_else),
            ..
        } = &function.body.statements[0]
        else {
            panic!("expected outer if with else")
        };
        assert_eq!(outer_else.statements.len(), 1);
        let Statement::If {
            else_branch: Some(inner_else),
            ..
        } = &outer_else.statements[0]
        else {
            panic!("else-if must lower to one nested if statement")
        };
        assert_eq!(inner_else.statements.len(), 1);
    }

    #[test]
    fn mutable_bindings_still_require_initializers() {
        let error = parse_module("fn invalid() { var value: i64; }")
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
        let program = parse_module("fn f() { let fixed = 1; var changing: i64 = 2; }")
            .expect("parse let and var bindings");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function")
        };
        assert!(matches!(
            &function.body.statements[0],
            Statement::Let { mutable: false, .. }
        ));
        assert!(matches!(
            &function.body.statements[1],
            Statement::Let { mutable: true, .. }
        ));
    }

    #[test]
    fn parse_canonical_contract_surface_and_preserve_identity() {
        let src = r#"
        seiyaku Payments {
            state counter: i64;
            struct Pair { left: i64; right: i64; }
            hajimari() { counter = 0; }
            kaizen() {}
            kotoage fn submit(who: AccountId, amount: Amount) authorize("Submit") {}
            view fn read(key: Name) -> i64 { return counter; }
            fn helper(left: i64, right: i64) -> i64 { return left + right; }
        }
        "#;
        let prog = parse(src).unwrap();
        assert_eq!(prog.unit.kind, SourceUnitKind::Contract);
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
        assert_eq!(functions[0].modifiers.kind, FunctionKind::Init);
        assert_eq!(functions[0].modifiers.permission, None);
        assert_eq!(functions[1].name, "kaizen");
        assert_eq!(functions[1].modifiers.kind, FunctionKind::Upgrade);
        assert_eq!(functions[1].modifiers.permission, None);
        assert_eq!(
            functions[2].modifiers.visibility,
            FunctionVisibility::Public
        );
        assert_eq!(functions[2].modifiers.permission.as_deref(), Some("Submit"));
        assert_eq!(functions[3].modifiers.kind, FunctionKind::View);
        assert_eq!(
            functions[4].modifiers.visibility,
            FunctionVisibility::Internal
        );
    }

    #[test]
    fn lifecycle_declarations_reject_source_authorization() {
        for source in [
            "seiyaku Demo { hajimari() authorize(\"Initialize\") {} }",
            "seiyaku Demo { kaizen() authorize(\"Upgrade\") {} }",
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
            parse("module Math { fn add(left: i64, right: i64) -> i64 { return left + right; } }")
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
                    recipient: AccountId,
                    asset: AssetDefinitionId,
                    amount: Amount,
                    dataspace: DataSpaceId
                ) authorize("TransferAsset") {
                    let sender = context::authority();
                    ledger::asset::transfer(sender, recipient, asset, amount, dataspace);
                }
            }
            "#,
        )
        .expect("parse canonical namespaces");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function")
        };
        let Statement::Let { value, .. } = &function.body.statements[0] else {
            panic!("expected authority binding")
        };
        assert!(matches!(
            value,
            Expr::Call { name, .. } if name == "context::authority"
        ));
        assert!(matches!(
            &function.body.statements[1],
            Statement::Expr(Expr::Call { name, .. }) if name == "ledger::asset::transfer"
        ));
    }

    #[test]
    fn keyword_tokens_are_admitted_only_in_required_namespace_positions() {
        let program = parse(
            r#"
            seiyaku Controls {
                kotoage fn update(path: Name, trigger_id: Name) authorize("Control") {
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
        assert!(matches!(
            &function.body.statements[0],
            Statement::Expr(Expr::Call { name, .. }) if name == "state::set"
        ));
        assert!(matches!(
            &function.body.statements[1],
            Statement::Expr(Expr::Call { name, .. }) if name == "ledger::trigger::set_enabled"
        ));

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
            "module M { fn f(i64 value) {} }",
            "module M { fn f(value) {} }",
            "module M { const VALUE = 1; }",
            "seiyaku C { state i64 value; }",
            "module M { struct Pair { i64 value; } }",
            "seiyaku C { kotoage fn f() authorize(Admin) {} }",
        ] {
            parse(source).expect_err("legacy declaration shape must fail");
        }
    }

    #[test]
    fn retired_type_spellings_are_unreserved_type_paths() {
        for legacy in [
            "int",
            "number",
            "fixed_u128",
            "String",
            "Blob",
            "Bytes",
            "Balance",
            "Map<i64, i64>",
            "unit",
        ] {
            let source = format!("module Types {{ fn use_type(value: {legacy}) {{}} }}");
            parse(&source).expect(
                "the parser must not recognize or rewrite retired type names; resolution rejects unknown types",
            );
        }
    }

    #[test]
    fn modules_reject_deployable_contract_items() {
        for body in [
            "kotoage fn run() {}",
            "view fn read() -> i64 { return 1; }",
            "hajimari() {}",
            "kaizen() {}",
            "state value: i64;",
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
            "fn f(n: i64) { for i in range(n) {} }",
            "fn f(values: StateMap<i64, i64>) { for (key, value) in values {} }",
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
        assert_eq!(func.modifiers.visibility, FunctionVisibility::Public);
        assert_eq!(func.modifiers.kind, FunctionKind::Contract);
        assert_eq!(func.modifiers.permission.as_deref(), Some("Admin"));
    }

    #[test]
    fn integer_literal_i64_suffix_is_allowed() {
        let src = "fn main() { let x = 1i64; }";
        parse_module(src).expect("parse i64 suffixed literal");
    }

    #[test]
    fn integer_literal_complete_u128_domain_is_allowed_with_suffix() {
        let src = "fn main() { let x: u128 = 340282366920938463463374607431768211455u128; }";
        let program = parse_module(src).expect("parse u128::MAX literal");
        let function = program
            .items
            .into_iter()
            .find_map(|item| match item {
                Item::Function(function) => Some(function),
                _ => None,
            })
            .expect("function present");
        let Statement::Let { value, .. } = &function.body.statements[0] else {
            panic!("expected let statement");
        };
        assert_eq!(
            value,
            &Expr::Decimal("340282366920938463463374607431768211455".into())
        );
    }

    #[test]
    fn integer_literal_u128_max_plus_one_is_rejected() {
        let src = "fn main() { let x: u128 = 340282366920938463463374607431768211456u128; }";
        let error = parse_module(src).expect_err("u128 overflow must fail");
        assert!(error.contains("numeric literal overflow"), "{error}");
    }

    #[test]
    fn u128_suffix_must_be_adjacent() {
        let error = parse_module("fn main() { let x: u128 = 1 u128; }")
            .expect_err("separated suffix must not be accepted");
        assert!(!error.is_empty());
    }

    #[test]
    fn negative_u128_literal_is_rejected() {
        let error =
            parse_module("fn main() { let x: u128 = -1u128; }").expect_err("u128 is unsigned");
        assert!(error.contains("u128"), "{error}");
    }

    #[test]
    fn unknown_integer_literal_suffix_errors() {
        let src = "fn main() { let x = 1i128; }";
        let err = parse_module(src).unwrap_err();
        assert!(err.contains("unknown integer literal suffix `i128`"));
    }

    #[test]
    fn parse_negative_i64_min_literal() {
        let src = "fn main() { let x = -9223372036854775808; }";
        parse_module(src).expect("parse i64::MIN literal");
    }

    #[test]
    fn parse_positive_i64_overflow_literal_errors() {
        let src = "fn main() { let x = 9223372036854775808; }";
        let err = parse_module(src).unwrap_err();
        assert!(err.contains("integer literal out of range"));
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
        let src = "fn f(m: StateMap<i64, i64>) { for (k, v) in m #[bounded(1)] { let z = k; } }";
        let error = parse_module(src).expect_err("#[bounded] is not Kotodama V1 syntax");
        assert!(error.contains("`.take(N)` or `.range(start, end)`"));
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
        match stmt {
            Statement::AssignExpr { op, value, .. } => {
                assert_eq!(*op, AssignOp::Add);
                assert!(matches!(value, Expr::Number(1)));
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
        match stmt {
            Statement::Let { value, .. } => match value {
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
        fn helper(balances: state StateMap<Name, i64>, key: Name) {}
        "#;
        let err = parse_module(src).expect_err("state parameters must be rejected");
        assert!(err.contains("state handles are not first-class parameters"));
    }

    #[test]
    fn parse_rejects_removed_free_map_helpers() {
        let err = parse_module("fn f(m: StateMap<i64, i64>) { let _x = get_or(m, 1, 7); }")
            .expect_err("free get_or should be rejected");
        assert!(
            err.contains("map.get_or(key, default)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn state_map_get_method_preserves_call_form_for_resolution() {
        let program = parse_module(
            "fn get(value: i64) -> i64 { return value; } \
             fn use_get(map: StateMap<i64, i64>) { \
                 let optional = map.get(1); \
                 let ordinary = get(1); \
             }",
        )
        .expect("method and free calls should parse");
        let Item::Function(function) = &program.items[1] else {
            panic!("expected use_get function");
        };
        let Statement::Let {
            value: Expr::Call { name: method, .. },
            ..
        } = &function.body.statements[0]
        else {
            panic!("expected StateMap.get call");
        };
        let Statement::Let {
            value: Expr::Call { name: free, .. },
            ..
        } = &function.body.statements[1]
        else {
            panic!("expected free get call");
        };
        assert_eq!(method, STATE_MAP_GET_INTRINSIC);
        assert_eq!(free, "get");
    }

    #[test]
    fn parse_rejects_removed_free_json_helpers() {
        let err = parse_module("fn f(ev: Json) { let _x = get_int(ev, Name::parse(\"n\")); }")
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
                "fn f(value: Option<i64>, map: StateMap<i64, i64>) {{ let _x = {expression}; }}"
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
        let err = parse_module("fn f(m: StateMap<i64, i64>) { let _x = m.has(1); }")
            .expect_err("method has should be rejected");
        assert!(err.contains("map.contains(key)"), "unexpected error: {err}");

        let err =
            parse_module("fn f(m: StateMap<i64, i64>) { let _x = m.get_or_insert_default(1, 7); }")
                .expect_err("method get_or_insert_default should be rejected");
        assert!(
            err.contains("map.ensure(key, default)"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_rejects_removed_method_path_and_json_aliases() {
        let err = parse_module("fn f(base: Name) { let _x = base.path_map_key(7); }")
            .expect_err("method path_map_key should be rejected");
        assert!(
            err.contains("base.path(segment)"),
            "unexpected error: {err}"
        );

        let err = parse_module("fn f(ev: Json) { let _x = ev.json_get_int(Name::parse(\"n\")); }")
            .expect_err("method json_get_int should be rejected");
        assert!(err.contains("json.get_int(key)"), "unexpected error: {err}");
    }

    #[test]
    fn parse_rejects_constructor_method_aliases() {
        for source in [
            r#"module M { fn f(value: string) { let _id = value.account_id(); } }"#,
            r#"module M { fn f(value: string) { let _name = value.name(); } }"#,
            r#"module M { fn f(value: string) { let _json = value.json(); } }"#,
            r#"module M { fn f(value: bytes) { let _raw = value.norito_bytes(); } }"#,
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
            kotoage fn run() {{}}
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
                kotoage fn run() {{}}
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
                kotoage fn run() {{}}
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
            kotoage fn run() {}
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
        iroha_data_model::asset::AssetDefinitionId::new(
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
            kotoage fn run() {{}}
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

    #[test]
    fn parse_trigger_decl_with_structured_data_filters_for_core_families() {
        let account = sample_account_literal();
        let peer =
            "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D".to_string();
        let domain = "wonderland".to_string();
        let asset_definition = sample_asset_definition_literal();
        let nft = "n0$wonderland.universal".to_string();
        let rwa = format!(
            "{}$wonderland.universal",
            iroha_crypto::Hash::prehashed([7; iroha_crypto::Hash::LENGTH])
        );
        let trigger = "wake".to_string();
        let role = "auditor".to_string();
        let asset = {
            let account_id = iroha_data_model::account::AccountId::parse_encoded(&account)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("account");
            let definition_id: iroha_data_model::asset::AssetDefinitionId =
                asset_definition.parse().expect("asset definition");
            iroha_data_model::asset::AssetId::new(definition_id, account_id).canonical_literal()
        };

        let cases = vec![
            (
                TriggerDataFamily::Peer,
                "added",
                vec![("peer".to_string(), peer)],
            ),
            (
                TriggerDataFamily::Domain,
                "created",
                vec![("domain".to_string(), domain)],
            ),
            (
                TriggerDataFamily::Account,
                "created",
                vec![("account".to_string(), account.clone())],
            ),
            (
                TriggerDataFamily::Asset,
                "added",
                vec![
                    ("asset".to_string(), asset),
                    ("asset_definition".to_string(), asset_definition.clone()),
                ],
            ),
            (
                TriggerDataFamily::AssetDefinition,
                "created",
                vec![("asset_definition".to_string(), asset_definition)],
            ),
            (
                TriggerDataFamily::Nft,
                "created",
                vec![("nft".to_string(), nft)],
            ),
            (
                TriggerDataFamily::Rwa,
                "created",
                vec![("rwa".to_string(), rwa)],
            ),
            (
                TriggerDataFamily::Trigger,
                "created",
                vec![("trigger".to_string(), trigger)],
            ),
            (
                TriggerDataFamily::Role,
                "created",
                vec![("role".to_string(), role)],
            ),
            (TriggerDataFamily::Configuration, "changed", vec![]),
            (TriggerDataFamily::Executor, "upgraded", vec![]),
        ];

        for (family, event, expected_matchers) in cases {
            let family_literal = match family {
                TriggerDataFamily::Peer => "peer",
                TriggerDataFamily::Domain => "domain",
                TriggerDataFamily::Account => "account",
                TriggerDataFamily::Asset => "asset",
                TriggerDataFamily::AssetDefinition => "asset_definition",
                TriggerDataFamily::Nft => "nft",
                TriggerDataFamily::Rwa => "rwa",
                TriggerDataFamily::Trigger => "trigger",
                TriggerDataFamily::Role => "role",
                TriggerDataFamily::Configuration => "configuration",
                TriggerDataFamily::Executor => "executor",
            };
            let matcher_block = expected_matchers
                .iter()
                .map(|(key, value)| format!("                    {key} \"{value}\";\n"))
                .collect::<String>();
            let src = format!(
                r#"
                seiyaku C {{
                    kotoage fn run() {{}}
                    trigger wake -> run {{
                        on data {family_literal} {event} {{
{matcher_block}                        }}
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
            assert_eq!(filter.family, family);
            assert_eq!(filter.event, TriggerDataEventKind::Named(event.to_string()));
            let actual_matchers = filter
                .matchers
                .iter()
                .map(|matcher| (matcher.key.clone(), matcher.value.clone()))
                .collect::<Vec<_>>();
            assert_eq!(actual_matchers, expected_matchers);
        }
    }

    #[test]
    fn parse_trigger_decl_with_pipeline_filter() {
        for (source_filter, expected_filter) in [
            ("transaction", TriggerPipelineFilter::TransactionApproved),
            (
                "transaction approved",
                TriggerPipelineFilter::TransactionApproved,
            ),
            ("block", TriggerPipelineFilter::BlockApproved),
            ("block approved", TriggerPipelineFilter::BlockApproved),
        ] {
            let src = format!(
                r#"
            seiyaku C {{
                kotoage fn run() {{}}
                trigger wake -> run {{
                    on pipeline {source_filter};
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
            assert_eq!(trigger.filter, TriggerFilter::Pipeline(expected_filter));
        }
    }

    #[test]
    fn parse_trigger_decl_rejects_nondeterministic_pipeline_filter() {
        let src = r#"
        seiyaku C {
            kotoage fn run() {}
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

    #[test]
    fn rejects_unregistered_unicode_attributes() {
        let src = r#"
        module ContractTests {
            #[テスト]
            fn smoke() {}
        }
        "#;
        let error = parse(src).expect_err("unregistered Unicode attributes are invalid");
        assert!(error.contains("non-ASCII"), "{error}");
    }
}
