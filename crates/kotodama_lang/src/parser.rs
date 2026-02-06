//! Parser for the Kotodama language.
//!
//! This module implements a simple recursive descent parser producing an AST.

use super::{
    ast::*,
    lexer::{Token, TokenKind, lex},
};

#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

fn escape_json_string(raw: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                write!(&mut out, "\\u{:04x}", c as u32).expect("write to string");
            }
            c => out.push(c),
        }
    }
    out
}

type ParseResult<T> = Result<T, ParseError>;
type ForEachMapBinding = (String, Option<String>, Expr, Option<usize>);

#[derive(Default)]
struct AccessHints {
    reads: Vec<String>,
    writes: Vec<String>,
}

impl AccessHints {
    fn is_empty(&self) -> bool {
        self.reads.is_empty() && self.writes.is_empty()
    }
}

/// Parse a KOTODAMA source string into a [`Program`].
pub fn parse(src: &str) -> Result<Program, String> {
    let tokens = lex(src)?;
    let mut parser = Parser::new(&tokens, src);
    match parser.parse_program() {
        Ok(p) => Ok(p),
        Err(e) => Err(format!(
            "{} at {}:{}\n{}",
            e.message, e.line, e.column, e.snippet
        )),
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    source: &'a str,
    contract_meta: Option<ContractMeta>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], source: &'a str) -> Self {
        Self {
            tokens,
            pos: 0,
            source,
            contract_meta: None,
        }
    }

    fn parse_macro_invocation(&mut self, ident_token: Token, name: String) -> ParseResult<Expr> {
        self.expect(TokenKind::Bang)?;
        if name == "json" && (self.peek(TokenKind::LBrace) || self.peek(TokenKind::LBracket)) {
            return self.parse_json_macro_literal(&ident_token);
        }
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
        self.rewrite_macro_call(ident_token, name, args)
    }

    fn rewrite_macro_call(
        &mut self,
        ident_token: Token,
        macro_name: String,
        args: Vec<Expr>,
    ) -> ParseResult<Expr> {
        let target = match macro_name.as_str() {
            "account" | "account_id" => Some("account_id"),
            "asset_definition" => Some("asset_definition"),
            "asset_id" => Some("asset_id"),
            "domain" => Some("domain"),
            "domain_id" => Some("domain_id"),
            "name" => Some("name"),
            "json" => Some("json"),
            "nft_id" => Some("nft_id"),
            "blob" => Some("blob"),
            "norito_bytes" => Some("norito_bytes"),
            _ => None,
        };
        if let Some(target_name) = target {
            return self.pointer_macro_call(&ident_token, target_name, args);
        }
        Err(self.error(
            ident_token,
            "unknown macro; supported prelude macros: account!, account_id!, asset_definition!, asset_id!, domain!, domain_id!, name!, json!, nft_id!, blob!, norito_bytes!",
        ))
    }

    fn pointer_macro_call(
        &mut self,
        ident_token: &Token,
        target: &str,
        args: Vec<Expr>,
    ) -> ParseResult<Expr> {
        if args.len() != 1 {
            return Err(self.error(
                ident_token.clone(),
                "prelude macro expects a single string literal argument",
            ));
        }
        let literal = match args.first() {
            Some(Expr::String(s)) => s.clone(),
            _ => {
                return Err(self.error(
                    ident_token.clone(),
                    "prelude macro expects a string literal argument",
                ));
            }
        };
        Ok(Expr::Call {
            name: target.to_string(),
            args: vec![Expr::String(literal)],
        })
    }

    fn parse_json_macro_literal(&mut self, ident_token: &Token) -> ParseResult<Expr> {
        let json_text = self.parse_json_macro_value(ident_token)?;
        Ok(Expr::Call {
            name: "json".to_string(),
            args: vec![Expr::String(json_text)],
        })
    }

    fn parse_json_macro_value(&mut self, ident_token: &Token) -> ParseResult<String> {
        let tok = self.bump();
        match tok.kind.clone() {
            TokenKind::LBrace => self.parse_json_object(ident_token),
            TokenKind::LBracket => self.parse_json_array(ident_token),
            TokenKind::String(s) => Ok(format!("\"{}\"", escape_json_string(&s))),
            TokenKind::Minus => {
                let next = self.bump();
                if let TokenKind::Number(n) = next.kind.clone() {
                    self.consume_integer_suffix()?;
                    let value = self.number_to_i64_neg(&next, n)?;
                    Ok(value.to_string())
                } else {
                    Err(self.error(next, "expected number after '-' in json! literal"))
                }
            }
            TokenKind::Number(n) => {
                self.consume_integer_suffix()?;
                let value = self.number_to_i64(&tok, n)?;
                Ok(value.to_string())
            }
            TokenKind::True => Ok("true".to_string()),
            TokenKind::False => Ok("false".to_string()),
            TokenKind::Ident(ref ident) if ident == "null" => Ok("null".to_string()),
            _ => Err(self.error(tok, "unsupported value in `json!{}` macro")),
        }
    }

    fn parse_json_object(&mut self, ident_token: &Token) -> ParseResult<String> {
        let mut out = String::from("{");
        if self.peek(TokenKind::RBrace) {
            self.bump();
            out.push('}');
            return Ok(out);
        }
        let mut first = true;
        loop {
            if !first {
                if self.peek(TokenKind::Comma) {
                    self.bump();
                    out.push(',');
                } else {
                    self.expect(TokenKind::RBrace)?;
                    out.push('}');
                    break;
                }
            }
            if first {
                first = false;
            }
            let key_tok = self.bump();
            let key = match key_tok.kind {
                TokenKind::String(ref s) => s.clone(),
                TokenKind::Ident(ref s) => s.clone(),
                _ => {
                    return Err(self.error(
                        key_tok,
                        "json! object keys must be identifiers or string literals",
                    ));
                }
            };
            out.push('"');
            out.push_str(&escape_json_string(&key));
            out.push('"');
            self.expect(TokenKind::Colon)?;
            out.push(':');
            let value = self.parse_json_macro_value(ident_token)?;
            out.push_str(&value);
            if self.peek(TokenKind::RBrace) {
                self.bump();
                out.push('}');
                break;
            }
        }
        Ok(out)
    }

    fn parse_json_array(&mut self, ident_token: &Token) -> ParseResult<String> {
        let mut out = String::from("[");
        if self.peek(TokenKind::RBracket) {
            self.bump();
            out.push(']');
            return Ok(out);
        }
        let mut first = true;
        loop {
            if !first {
                if self.peek(TokenKind::Comma) {
                    self.bump();
                    out.push(',');
                } else {
                    self.expect(TokenKind::RBracket)?;
                    out.push(']');
                    break;
                }
            }
            if first {
                first = false;
            }
            let value = self.parse_json_macro_value(ident_token)?;
            out.push_str(&value);
            if self.peek(TokenKind::RBracket) {
                self.bump();
                out.push(']');
                break;
            }
        }
        Ok(out)
    }

    fn parse_program(&mut self) -> ParseResult<Program> {
        let mut items = Vec::new();
        while !self.peek(TokenKind::EOF) {
            let access_hints = self.parse_access_attributes()?;
            if self.peek(TokenKind::Fn) {
                self.bump();
                items.push(self.parse_fn_loose(
                    None,
                    FunctionModifiers {
                        visibility: FunctionVisibility::Internal,
                        kind: FunctionKind::Free,
                        permission: None,
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek(TokenKind::Struct) {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                items.push(self.parse_struct_def()?);
            } else if self.peek(TokenKind::Seiyaku) {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                let mut contract_items = self.parse_contract()?;
                items.append(&mut contract_items);
            } else if self.peek(TokenKind::State) {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                items.push(self.parse_state_decl()?);
            } else if self.peek(TokenKind::Kotoage) && self.peek_n(1, TokenKind::Fn) {
                self.bump(); // kotoage
                self.bump(); // fn
                items.push(self.parse_fn_loose(
                    None,
                    FunctionModifiers {
                        visibility: FunctionVisibility::Public,
                        kind: FunctionKind::Free,
                        permission: None,
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek(TokenKind::Hajimari) {
                self.bump();
                items.push(self.parse_fn_loose(
                    Some("hajimari".to_owned()),
                    FunctionModifiers {
                        visibility: FunctionVisibility::Internal,
                        kind: FunctionKind::Hajimari,
                        permission: None,
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek(TokenKind::Kaizen) {
                self.bump();
                items.push(self.parse_fn_loose(
                    Some("kaizen".to_owned()),
                    FunctionModifiers {
                        visibility: FunctionVisibility::Internal,
                        kind: FunctionKind::Kaizen,
                        permission: None,
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek_ident_n(0, "kotoba") {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                items.push(self.parse_kotoba_block()?);
            } else {
                let tok = self.bump();
                return Err(self.error(tok, "top-level item (fn, struct, state, seiyaku)"));
            }
        }
        Ok(Program {
            items,
            contract_meta: self.contract_meta.clone(),
        })
    }

    fn parse_contract(&mut self) -> ParseResult<Vec<Item>> {
        self.expect(TokenKind::Seiyaku)?;
        let _name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let access_hints = self.parse_access_attributes()?;
            if self.peek(TokenKind::Meta) {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                self.parse_meta_block()?;
            } else if self.peek(TokenKind::Struct) {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                items.push(self.parse_struct_def()?);
            } else if self.peek(TokenKind::State) {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                items.push(self.parse_state_decl()?);
            } else if self.peek_ident_n(0, "register_trigger") || self.peek_ident_n(0, "trigger") {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
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
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek(TokenKind::Kotoage) && self.peek_n(1, TokenKind::Fn) {
                self.bump(); // kotoage
                self.bump(); // fn
                items.push(self.parse_fn_loose(
                    None,
                    FunctionModifiers {
                        visibility: FunctionVisibility::Public,
                        kind: FunctionKind::Contract,
                        permission: None,
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek(TokenKind::Hajimari) {
                self.bump();
                items.push(self.parse_fn_loose(
                    Some(String::from("hajimari")),
                    FunctionModifiers {
                        visibility: FunctionVisibility::Internal,
                        kind: FunctionKind::Hajimari,
                        permission: None,
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek(TokenKind::Kaizen) {
                self.bump();
                items.push(self.parse_fn_loose(
                    Some(String::from("kaizen")),
                    FunctionModifiers {
                        visibility: FunctionVisibility::Internal,
                        kind: FunctionKind::Kaizen,
                        permission: None,
                        access_reads: access_hints.reads,
                        access_writes: access_hints.writes,
                    },
                )?);
            } else if self.peek_ident_n(0, "kotoba") {
                if !access_hints.is_empty() {
                    return Err(self.error(
                        self.tokens[self.pos].clone(),
                        "access attributes must precede a function",
                    ));
                }
                items.push(self.parse_kotoba_block()?);
            } else {
                let tok = self.bump();
                return Err(self.error(tok, "contract item (fn, struct, state, meta)"));
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(items)
    }

    fn parse_kotoba_block(&mut self) -> ParseResult<Item> {
        let tok = self.bump();
        if !matches!(tok.kind, TokenKind::Ident(ref s) if s == "kotoba") {
            return Err(self.error(tok, "kotoba"));
        }
        self.expect(TokenKind::LBrace)?;
        let mut entries = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
                continue;
            }
            let msg_id = self.expect_ident_or_string()?;
            self.expect(TokenKind::Colon)?;
            self.expect(TokenKind::LBrace)?;
            let mut translations = Vec::new();
            while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
                if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                let lang = self.expect_ident_or_string()?;
                self.expect(TokenKind::Colon)?;
                let tok = self.bump();
                let text = match tok.kind {
                    TokenKind::String(s) => s,
                    _ => return Err(self.error(tok, "string literal")),
                };
                translations.push(KotobaTranslation { lang, text });
                if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                    self.bump();
                }
            }
            self.expect(TokenKind::RBrace)?;
            entries.push(KotobaEntry {
                msg_id,
                translations,
            });
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Item::Kotoba(KotobaBlock { entries }))
    }

    fn parse_trigger_decl(&mut self) -> ParseResult<Item> {
        let tok = self.bump();
        let keyword = if let TokenKind::Ident(name) = tok.kind.clone() {
            name
        } else {
            return Err(self.error(tok, "register_trigger"));
        };
        if keyword != "register_trigger" && keyword != "trigger" {
            return Err(self.error(tok, "register_trigger"));
        }
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut call: Option<TriggerCall> = None;
        let mut filter: Option<TriggerFilter> = None;
        let mut repeats: Option<TriggerRepeats> = None;
        let mut authority: Option<String> = None;
        let mut metadata: Vec<TriggerMetadataEntry> = Vec::new();
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            let field_tok = self.bump();
            let field_name = match field_tok.kind.clone() {
                TokenKind::Ident(name) => name,
                TokenKind::Call => "call".to_string(),
                _ => {
                    return Err(self.error(field_tok, "trigger field"));
                }
            };
            match field_name.as_str() {
                "call" => {
                    if call.is_some() {
                        return Err(self.error(field_tok, "duplicate `call` field"));
                    }
                    call = Some(self.parse_trigger_call()?);
                    self.expect(TokenKind::Semicolon)?;
                }
                "on" => {
                    if filter.is_some() {
                        return Err(self.error(field_tok, "duplicate `on` field"));
                    }
                    filter = Some(self.parse_trigger_filter()?);
                    self.expect(TokenKind::Semicolon)?;
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
                        "trigger field (`call`, `on`, `repeats`, `authority`, `metadata`)",
                    ));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;
        let call = call.ok_or_else(|| self.error(tok.clone(), "trigger `call` field"))?;
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
        if self.peek(TokenKind::Colon) && self.peek_n(1, TokenKind::Colon) {
            self.bump();
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
                let next = self.expect_ident()?;
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
        let kind = self.expect_ident()?;
        match kind.as_str() {
            "any" => Ok(TriggerDataFilter::Any),
            _ => Err(self.error(
                self.tokens[self.pos.saturating_sub(1)].clone(),
                "data filter (`any`)",
            )),
        }
    }

    fn parse_trigger_pipeline_filter(&mut self) -> ParseResult<TriggerPipelineFilter> {
        let kind = self.expect_ident()?;
        match kind.as_str() {
            "transaction" => Ok(TriggerPipelineFilter::Transaction),
            "block" => Ok(TriggerPipelineFilter::Block),
            "merge" => Ok(TriggerPipelineFilter::Merge),
            "witness" => Ok(TriggerPipelineFilter::Witness),
            _ => Err(self.error(
                self.tokens[self.pos.saturating_sub(1)].clone(),
                "pipeline filter (`transaction`, `block`, `merge`, or `witness`)",
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
                self.consume_integer_suffix()?;
                Ok(n)
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

    fn parse_access_attributes(&mut self) -> ParseResult<AccessHints> {
        let mut hints = AccessHints::default();
        while self.peek(TokenKind::Hash) {
            self.bump(); // '#'
            self.expect(TokenKind::LBracket)?;
            let attr_tok = self.bump();
            let attr_name = if let TokenKind::Ident(name) = attr_tok.kind.clone() {
                name
            } else {
                return Err(self.error(attr_tok, "expected attribute identifier"));
            };
            if attr_name != "access" {
                return Err(self.error(attr_tok, "expected attribute `access`"));
            }
            self.expect(TokenKind::LParen)?;
            let mut parsed_any = false;
            while !self.peek(TokenKind::RParen) && !self.peek(TokenKind::EOF) {
                let key = self.expect_ident()?;
                self.expect(TokenKind::Equal)?;
                let mut values = self.parse_access_value_list()?;
                match key.as_str() {
                    "read" => hints.reads.append(&mut values),
                    "write" => hints.writes.append(&mut values),
                    _ => {
                        return Err(ParseError {
                            message: format!("unknown access list `{key}`"),
                            line: self.tokens[self.pos.saturating_sub(1)].line,
                            column: self.tokens[self.pos.saturating_sub(1)].column,
                            snippet: String::new(),
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
                });
            }
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::RBracket)?;
        }
        Ok(hints)
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

    fn parse_meta_block(&mut self) -> ParseResult<()> {
        // meta { key: value; ... }
        self.expect(TokenKind::Meta)?;
        self.expect(TokenKind::LBrace)?;
        while !self.peek(TokenKind::RBrace) && !self.peek(TokenKind::EOF) {
            // Expect an identifier key
            let key = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            // Value can be number or bool. Parse into a temporary enum to avoid
            // borrowing `self` mutably while fetching additional tokens.
            enum MetaValue {
                Number(i64, Token),
                Bool(bool, Token),
                FeatureList(Vec<String>, Token),
            }
            let tok = self.bump();
            let value = match tok.kind.clone() {
                TokenKind::Number(n) => {
                    self.consume_integer_suffix()?;
                    let value = self.number_to_i64(&tok, n)?;
                    MetaValue::Number(value, tok.clone())
                }
                TokenKind::Minus => {
                    let next = self.bump();
                    match next.kind.clone() {
                        TokenKind::Number(n) => {
                            self.consume_integer_suffix()?;
                            let value = self.number_to_i64_neg(&next, n)?;
                            MetaValue::Number(value, next.clone())
                        }
                        _ => return Err(self.error(next, "number")),
                    }
                }
                TokenKind::True | TokenKind::False => {
                    MetaValue::Bool(matches!(tok.kind, TokenKind::True), tok.clone())
                }
                TokenKind::LBracket => {
                    let entries = self.parse_string_list()?;
                    MetaValue::FeatureList(entries, tok.clone())
                }
                _ => return Err(self.error(tok, "number, boolean, or string list")),
            };
            match value {
                MetaValue::Number(value, value_tok) => {
                    let meta = self.contract_meta.get_or_insert_with(ContractMeta::default);
                    match key.as_str() {
                        "abi_version" | "abi" => {
                            if !(0..=u8::MAX as i64).contains(&value) {
                                return Err(ParseError {
                                    message: format!(
                                        "meta key '{key}' value {value} out of range (0..=255)"
                                    ),
                                    line: value_tok.line,
                                    column: value_tok.column,
                                    snippet: String::new(),
                                });
                            }
                            meta.abi_version = Some(value as u8);
                        }
                        "vector_length" | "vl" => {
                            if !(0..=u8::MAX as i64).contains(&value) {
                                return Err(ParseError {
                                    message: format!(
                                        "meta key '{key}' value {value} out of range (0..=255)"
                                    ),
                                    line: value_tok.line,
                                    column: value_tok.column,
                                    snippet: String::new(),
                                });
                            }
                            meta.vector_length = Some(value as u8);
                        }
                        "max_cycles" | "cycles" => {
                            if value < 0 {
                                return Err(ParseError {
                                    message: format!(
                                        "meta key '{key}' value {value} must be non-negative"
                                    ),
                                    line: value_tok.line,
                                    column: value_tok.column,
                                    snippet: String::new(),
                                });
                            }
                            meta.max_cycles = Some(value as u64);
                        }
                        other => {
                            return Err(ParseError {
                                message: format!("unknown meta numeric key '{other}'"),
                                line: value_tok.line,
                                column: value_tok.column,
                                snippet: String::new(),
                            });
                        }
                    }
                }
                MetaValue::Bool(val, tok) => {
                    let meta = self.contract_meta.get_or_insert_with(ContractMeta::default);
                    match key.as_str() {
                        "zk" => meta.force_zk = Some(val),
                        "vector" => meta.force_vector = Some(val),
                        other => {
                            return Err(ParseError {
                                message: format!("unknown meta boolean key '{other}'"),
                                line: tok.line,
                                column: tok.column,
                                snippet: String::new(),
                            });
                        }
                    }
                }
                MetaValue::FeatureList(entries, tok) => match key.as_str() {
                    "features" => {
                        let parsed_features = {
                            let mut parsed = Vec::with_capacity(entries.len());
                            for raw in entries {
                                parsed.push(Self::parse_contract_feature(
                                    &raw, tok.line, tok.column,
                                )?);
                            }
                            parsed
                        };
                        let meta = self.contract_meta.get_or_insert_with(ContractMeta::default);
                        if !meta.features.is_empty() {
                            return Err(ParseError {
                                message: "duplicate meta key 'features'".into(),
                                line: tok.line,
                                column: tok.column,
                                snippet: String::new(),
                            });
                        }
                        for feature in parsed_features {
                            if !meta.features.contains(&feature) {
                                meta.features.push(feature);
                            }
                        }
                        if meta
                            .features
                            .iter()
                            .any(|f| matches!(f, ContractFeature::Zk))
                            && meta.force_zk.is_none()
                        {
                            meta.force_zk = Some(true);
                        }
                        if meta
                            .features
                            .iter()
                            .any(|f| matches!(f, ContractFeature::Vector))
                            && meta.force_vector.is_none()
                        {
                            meta.force_vector = Some(true);
                        }
                    }
                    other => {
                        return Err(ParseError {
                            message: format!("unknown meta list key '{other}'"),
                            line: tok.line,
                            column: tok.column,
                            snippet: String::new(),
                        });
                    }
                },
            }
            // Optional separator: ';' or ','
            if self.peek(TokenKind::Semicolon) || self.peek(TokenKind::Comma) {
                self.bump();
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(())
    }

    fn parse_string_list(&mut self) -> ParseResult<Vec<String>> {
        let mut values = Vec::new();
        if self.peek(TokenKind::RBracket) {
            self.bump();
            return Ok(values);
        }
        loop {
            let tok = self.bump();
            let value = match tok.kind {
                TokenKind::String(s) => s,
                TokenKind::Ident(s) => s,
                _ => return Err(self.error(tok, "string literal or identifier")),
            };
            values.push(value);
            if self.peek(TokenKind::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        self.expect(TokenKind::RBracket)?;
        Ok(values)
    }

    fn parse_contract_feature(
        raw: &str,
        line: usize,
        column: usize,
    ) -> ParseResult<ContractFeature> {
        let normalized = raw.to_ascii_lowercase();
        match normalized.as_str() {
            "zk" | "zero_knowledge" => Ok(ContractFeature::Zk),
            "simd" | "vector" => Ok(ContractFeature::Vector),
            other => Err(ParseError {
                message: format!("unknown feature '{other}' in meta features list"),
                line,
                column,
                snippet: String::new(),
            }),
        }
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
        // state Type name;   or   state name: Type;
        self.expect(TokenKind::State)?;
        let next_is_ident = matches!(
            self.tokens.get(self.pos),
            Some(Token {
                kind: TokenKind::Ident(_),
                ..
            })
        );
        let (name, ty) = if next_is_ident && self.peek_n(1, TokenKind::Colon) {
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            (name, ty)
        } else {
            let ty = self.parse_type_expr()?;
            let name = self.expect_ident()?;
            (name, ty)
        };
        if self.peek(TokenKind::Semicolon) {
            self.bump();
        }
        Ok(Item::State(super::ast::StateDecl { name, ty }))
    }

    fn parse_fn_loose(
        &mut self,
        name_override: Option<String>,
        mut modifiers: FunctionModifiers,
    ) -> ParseResult<Item> {
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
        // Optional permission modifiers
        while !self.peek(TokenKind::LBrace) && !self.peek(TokenKind::EOF) {
            if self.peek(TokenKind::Permission) {
                self.bump();
                self.expect(TokenKind::LParen)?;
                let perm = self.expect_ident()?;
                self.expect(TokenKind::RParen)?;
                if modifiers.permission.is_some() {
                    return Err(ParseError {
                        message: "duplicate permission modifier".into(),
                        line: self.tokens[self.pos.saturating_sub(1)].line,
                        column: self.tokens[self.pos.saturating_sub(1)].column,
                        snippet: String::new(),
                    });
                }
                modifiers.permission = Some(perm);
            } else if self.peek(TokenKind::Semicolon) {
                self.bump();
            } else {
                let tok = self.bump();
                return Err(self.error(tok, "`permission(name)` or `{`"));
            }
        }
        let body = self.parse_block()?;
        Ok(Item::Function(Function {
            name,
            params,
            ret_ty,
            body,
            modifiers,
        }))
    }

    fn parse_block(&mut self) -> ParseResult<Block> {
        self.expect(TokenKind::LBrace)?;
        let mut statements = Vec::new();
        while !self.peek(TokenKind::RBrace) {
            statements.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Block { statements })
    }

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        // Sugar: `call <expr>;` must be a call expression
        if self.peek(TokenKind::Call) {
            let call_tok = self.bump();
            let expr = self.parse_expr()?;
            // Ensure it’s a call
            match &expr {
                Expr::Call { .. } => {}
                _ => {
                    let line_text = self
                        .source
                        .lines()
                        .nth(call_tok.line.saturating_sub(1))
                        .unwrap_or("");
                    let caret = " ".repeat(call_tok.column.saturating_sub(1)) + "^";
                    return Err(ParseError {
                        message: "call expects a function call expression".into(),
                        line: call_tok.line,
                        column: call_tok.column,
                        snippet: format!("{line_text}\n{caret}"),
                    });
                }
            }
            self.expect(TokenKind::Semicolon)?;
            return Ok(Statement::Expr(expr));
        }
        if self.peek(TokenKind::Let) {
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
                Some(self.parse_block()?)
            } else {
                None
            };
            Ok(Statement::If {
                cond,
                then_branch,
                else_branch,
            })
        } else if self.peek(TokenKind::While) {
            self.bump();
            let cond = self.parse_expr()?;
            let body = self.parse_block()?;
            Ok(Statement::While { cond, body })
        } else if self.peek(TokenKind::For) {
            let for_line = self.tokens.get(self.pos).map(|t| t.line).unwrap_or(0);
            self.expect(TokenKind::For)?;
            if let Some((k, v_opt, map, bound)) = self.parse_for_each_map()? {
                let body = self.parse_block()?;
                Ok(Statement::ForEachMap {
                    key: k,
                    value: v_opt,
                    map,
                    bound,
                    body,
                })
            } else if let Some((init, cond, step)) = self.parse_for_range()? {
                let body = self.parse_block()?;
                Ok(Statement::For {
                    line: for_line,
                    init: Some(Box::new(init)),
                    cond: Some(cond),
                    step: Some(Box::new(step)),
                    body,
                })
            } else {
                let init = if !self.peek(TokenKind::Semicolon) {
                    let stmt = self.parse_for_clause()?;
                    Some(Box::new(stmt))
                } else {
                    None
                };
                self.expect(TokenKind::Semicolon)?;
                let cond = if !self.peek(TokenKind::Semicolon) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(TokenKind::Semicolon)?;
                let step = if !self.peek(TokenKind::LBrace) {
                    let stmt = self.parse_for_clause()?;
                    Some(Box::new(stmt))
                } else {
                    None
                };
                let body = self.parse_block()?;
                Ok(Statement::For {
                    line: for_line,
                    init,
                    cond,
                    step,
                    body,
                })
            }
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
                    let op_tok = self.bump().kind.clone();
                    let rhs = self.parse_expr()?;
                    self.expect(TokenKind::Semicolon)?;
                    let op = match op_tok {
                        TokenKind::Equal => AssignOp::Set,
                        TokenKind::PlusEqual => AssignOp::Add,
                        TokenKind::MinusEqual => AssignOp::Sub,
                        TokenKind::StarEqual => AssignOp::Mul,
                        TokenKind::SlashEqual => AssignOp::Div,
                        TokenKind::PercentEqual => AssignOp::Mod,
                        _ => unreachable!(),
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

    fn parse_for_clause(&mut self) -> ParseResult<Statement> {
        if self.peek(TokenKind::Let) {
            self.bump();
            let name = self.expect_ident()?;
            self.expect(TokenKind::Equal)?;
            let expr = self.parse_expr()?;
            Ok(Statement::Let {
                pat: Pattern::Name(name),
                ty: None,
                value: expr,
            })
        } else if self.peek(TokenKind::PlusPlus) {
            self.bump();
            let name = self.expect_ident()?;
            Ok(self.inc_statement(name))
        } else if let Some(Token {
            kind: TokenKind::Ident(name),
            ..
        }) = self.tokens.get(self.pos)
        {
            if self.peek_n(1, TokenKind::PlusPlus) {
                let name = name.clone();
                self.bump();
                self.bump();
                Ok(self.inc_statement(name))
            } else {
                let save = self.pos;
                if let Ok(target) = self.try_parse_lvalue_expr()
                    && (self.peek(TokenKind::Equal)
                        || self.peek(TokenKind::PlusEqual)
                        || self.peek(TokenKind::MinusEqual)
                        || self.peek(TokenKind::StarEqual)
                        || self.peek(TokenKind::SlashEqual)
                        || self.peek(TokenKind::PercentEqual))
                {
                    let op_tok = self.bump().kind.clone();
                    let rhs = self.parse_expr()?;
                    let op = match op_tok {
                        TokenKind::Equal => AssignOp::Set,
                        TokenKind::PlusEqual => AssignOp::Add,
                        TokenKind::MinusEqual => AssignOp::Sub,
                        TokenKind::StarEqual => AssignOp::Mul,
                        TokenKind::SlashEqual => AssignOp::Div,
                        TokenKind::PercentEqual => AssignOp::Mod,
                        _ => unreachable!(),
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
                }
                self.pos = save;
                let expr = self.parse_expr()?;
                Ok(Statement::Expr(expr))
            }
        } else {
            let expr = self.parse_expr()?;
            Ok(Statement::Expr(expr))
        }
    }

    fn inc_statement(&self, name: String) -> Statement {
        Statement::Let {
            pat: Pattern::Name(name.clone()),
            ty: None,
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
            && self.peek_ident_n(1, "in")
            && self.peek_ident_n(2, "range")
        {
            self.bump();
            self.bump(); // in
            self.bump(); // range
            self.expect(TokenKind::LParen)?;
            let end = self.parse_expr()?;
            self.expect(TokenKind::RParen)?;
            let init = Statement::Let {
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
        let cond = self.parse_logical_or()?;
        if self.peek(TokenKind::Question) {
            self.bump();
            let then_expr = self.parse_conditional()?;
            self.expect(TokenKind::Colon)?;
            let else_expr = self.parse_conditional()?;
            Ok(Expr::Conditional {
                cond: Box::new(cond),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            })
        } else {
            Ok(cond)
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
        if self.peek(TokenKind::Minus) {
            self.bump();
            if let Some(token) = self.tokens.get(self.pos).cloned()
                && let TokenKind::Number(n) = token.kind.clone()
                && n > i64::MAX as u64
            {
                self.bump();
                self.consume_integer_suffix()?;
                let value = self.number_to_i64_neg(&token, n)?;
                return Ok(Expr::Number(value));
            }
            let expr = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            })
        } else if self.peek(TokenKind::Bang) {
            self.bump();
            let expr = self.parse_unary()?;
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            })
        } else {
            let prim = self.parse_primary()?;
            self.parse_postfix(prim)
        }
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> ParseResult<Expr> {
        loop {
            if self.peek(TokenKind::Dot) {
                self.bump();
                // Accept `ident` or numeric tuple index after '.'
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
                    // Avoid borrowing self immutably and mutably in a single expression
                    let tok = self.bump();
                    return Err(self.error(tok, "identifier or tuple index"));
                };
                // Method-call sugar: `expr.method(args...)` -> `Call { name: method, args: [expr, args...] }`
                if self.peek(TokenKind::LParen) {
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
                    expr = Expr::Call {
                        name: field,
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
            TokenKind::Number(n) => {
                self.consume_integer_suffix()?;
                let value = self.number_to_i64(&tok, *n)?;
                Ok(Expr::Number(value))
            }
            TokenKind::Decimal(raw) => Ok(Expr::Decimal(raw.clone())),
            TokenKind::String(s) => Ok(Expr::String(s.clone())),
            TokenKind::Bytes(bytes) => Ok(Expr::Bytes(bytes.clone())),
            TokenKind::Ident(name0) => {
                let ident_token = tok.clone();
                // Parse namespaced path: ident ('::' ident)*
                let mut name = name0.clone();
                let base_name = name0.clone();
                loop {
                    if self.peek(TokenKind::Colon) && self.peek_n(1, TokenKind::Colon) {
                        // consume '::'
                        self.bump();
                        self.bump();
                        let seg = self.expect_ident()?;
                        name.push_str("::");
                        name.push_str(&seg);
                    } else {
                        break;
                    }
                }
                if self.peek(TokenKind::Bang) && name == base_name {
                    return self.parse_macro_invocation(ident_token, name);
                }
                if self.peek(TokenKind::LParen) {
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
            TokenKind::LParen => {
                if self.peek(TokenKind::RParen) {
                    self.bump();
                    Ok(Expr::Tuple(vec![]))
                } else {
                    let first = self.parse_expr()?;
                    if self.peek(TokenKind::Comma) {
                        let mut elems = vec![first];
                        while self.peek(TokenKind::Comma) {
                            self.bump();
                            elems.push(self.parse_expr()?);
                        }
                        self.expect(TokenKind::RParen)?;
                        Ok(Expr::Tuple(elems))
                    } else {
                        self.expect(TokenKind::RParen)?;
                        Ok(first)
                    }
                }
            }
            _ => Err(self.error(tok, "expression")),
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        // Accept regular identifiers and allow certain keyword tokens
        // (e.g., Hajimari/Kaizen) to be used as names in contexts like
        // `fn hajimari() { ... }`.
        let tok = self.bump();
        match &tok.kind {
            TokenKind::Ident(name) => Ok(name.clone()),
            TokenKind::Hajimari => Ok(String::from("hajimari")),
            TokenKind::Kaizen => Ok(String::from("kaizen")),
            _ => Err(self.error(tok, "identifier")),
        }
    }

    fn parse_type_expr(&mut self) -> ParseResult<TypeExpr> {
        // tuple type: (T1, T2, ...)
        if self.peek(TokenKind::LParen) {
            self.bump();
            let mut tys = Vec::new();
            if !self.peek(TokenKind::RParen) {
                loop {
                    tys.push(self.parse_type_expr()?);
                    if self.peek(TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen)?;
            return Ok(TypeExpr::Tuple(tys));
        }
        // path or generic: Ident ['<' Ty (',' Ty)* '>']
        let base = self.expect_ident()?;
        if self.peek(TokenKind::Less) {
            self.bump();
            let mut args = Vec::new();
            if !self.peek(TokenKind::Greater) {
                loop {
                    args.push(self.parse_type_expr()?);
                    if self.peek(TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(TokenKind::Greater)?;
            Ok(TypeExpr::Generic { base, args })
        } else {
            Ok(TypeExpr::Path(base))
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
            if self.peek_ident_n(0, "in") {
                self.bump();
                let map = self.parse_expr()?;
                let bound = self.parse_optional_bounded_attr()?;
                return Ok(Some((k, Some(v), map, bound)));
            }
        } else if let Some(Token {
            kind: TokenKind::Ident(k),
            ..
        }) = self.tokens.get(self.pos).cloned()
            && self.peek_ident_n(1, "in")
        {
            self.bump();
            self.bump(); // in
            let map = self.parse_expr()?;
            let bound = self.parse_optional_bounded_attr()?;
            return Ok(Some((k, None, map, bound)));
        }
        self.pos = save;
        Ok(None)
    }

    fn parse_optional_bounded_attr(&mut self) -> ParseResult<Option<usize>> {
        if !self.peek(TokenKind::Hash) {
            return Ok(None);
        }
        self.bump(); // '#'
        self.expect(TokenKind::LBracket)?;
        let attr_tok = self.bump();
        let attr_name = if let TokenKind::Ident(name) = attr_tok.kind.clone() {
            name
        } else {
            return Err(self.error(attr_tok, "expected attribute identifier"));
        };
        if attr_name != "bounded" {
            return Err(self.error(attr_tok, "expected attribute `bounded`"));
        }
        self.expect(TokenKind::LParen)?;
        let value_tok = self.bump();
        let bound = match value_tok.kind.clone() {
            TokenKind::Number(n) => {
                self.number_to_usize(&value_tok, n, "`bounded(n)` expects a non-negative")?
            }
            _ => {
                return Err(self.error(
                    value_tok,
                    "`bounded(n)` expects a non-negative integer literal",
                ));
            }
        };
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::RBracket)?;
        Ok(Some(bound))
    }

    fn parse_param(&mut self) -> ParseResult<Param> {
        // Support both `Type name` and `name: Type` forms, or bare `name`.
        let save = self.pos;
        // Try `Type name`
        if let Ok(ty) = self.try_parse_type_then_name() {
            return Ok(ty);
        }
        self.pos = save;
        // Fallback `name [: Type]?`
        let name = self.expect_ident()?;
        let ty = if self.peek(TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        Ok(Param { ty, name })
    }

    fn try_parse_type_then_name(&mut self) -> ParseResult<Param> {
        let ty = self.parse_type_expr()?;
        let name = self.expect_ident()?;
        Ok(Param { ty: Some(ty), name })
    }

    fn expect(&mut self, kind: TokenKind) -> ParseResult<()> {
        let tok = self.bump();
        if tok.kind == kind {
            Ok(())
        } else {
            Err(self.error(tok, &format!("{kind:?}")))
        }
    }

    fn consume_integer_suffix(&mut self) -> ParseResult<()> {
        if let Some(Token {
            kind: TokenKind::Ident(suffix),
            ..
        }) = self.tokens.get(self.pos)
        {
            if suffix == "i64" {
                self.bump();
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
                });
            }
        }
        Ok(())
    }

    fn number_to_i64(&self, token: &Token, value: u64) -> ParseResult<i64> {
        if value <= i64::MAX as u64 {
            Ok(value as i64)
        } else {
            Err(self.range_error(
                token,
                format!("integer literal out of range (max {})", i64::MAX),
            ))
        }
    }

    fn number_to_i64_neg(&self, token: &Token, value: u64) -> ParseResult<i64> {
        let max_plus_one = i64::MAX as u64 + 1;
        if value <= i64::MAX as u64 {
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

    fn number_to_usize(&self, token: &Token, value: u64, context: &str) -> ParseResult<usize> {
        if value <= i64::MAX as u64 && value <= usize::MAX as u64 {
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

    fn bump(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token {
            kind: TokenKind::EOF,
            line: self.tokens.last().map_or(0, |t| t.line),
            column: self.tokens.last().map_or(0, |t| t.column),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_return_statements() {
        let src = "fn f() { return; return 1; }";
        let prog = parse(src).unwrap();
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
    fn parse_bools_and_logical_ops() {
        let src = "fn g() { let x = true && !false; }";
        let prog = parse(src).unwrap();
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_assignment_and_break_continue() {
        let src = "fn h() { let x = 0; x = 1; while x < 10 { if x == 3 { break; } x = x + 1; if x == 5 { continue; } } }";
        let prog = parse(src).unwrap();
        assert_eq!(prog.items.len(), 1);
    }

    #[test]
    fn parse_contract_bodies() {
        let src = r#"
        seiyaku C {
            hajimari() { let a = 1; }
            kotoage fn foo(AccountId who, Amount a) { let b = 2; }
            fn bar(x, y) { let c = x + y; }
        }
        "#;
        let prog = parse(src).unwrap();
        // Flattened into free functions
        assert!(prog.items.len() >= 3);
    }

    #[test]
    fn parse_fn_named_hajimari_is_allowed() {
        // Ensure reserved keyword token can be used as an identifier after `fn`
        let src = "fn hajimari() { let x = 1; }";
        let prog = parse(src).unwrap();
        assert_eq!(prog.items.len(), 1);
        if let Item::Function(f) = &prog.items[0] {
            assert_eq!(f.name, "hajimari");
        } else {
            panic!("expected function item");
        }
    }

    #[test]
    fn parse_for_range_loop() {
        let src = "fn f() { for x in range(6) { let y = x; } }";
        let prog = parse(src).expect("parse failed");
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
    fn parse_contract_meta_rejects_out_of_range_values() {
        let src = r#"
        seiyaku C {
            meta { abi_version: 999; }
        }
        "#;
        let err = parse(src).unwrap_err();
        assert!(err.contains("out of range"));
    }

    #[test]
    fn parse_contract_meta_rejects_negative_max_cycles() {
        let src = r#"
        seiyaku C {
            meta { max_cycles: -1; }
        }
        "#;
        let err = parse(src).unwrap_err();
        assert!(err.contains("must be non-negative"));
    }

    #[test]
    fn parse_contract_meta_features_list_sets_flags() {
        let src = r#"
        seiyaku C {
            meta { features: ["zk", "simd"]; }
        }
        "#;
        let prog = parse(src).expect("parse meta features");
        let meta = prog.contract_meta.expect("meta present");
        assert!(meta.features.contains(&ContractFeature::Zk));
        assert!(meta.features.contains(&ContractFeature::Vector));
        assert_eq!(meta.force_zk, Some(true));
        assert_eq!(meta.force_vector, Some(true));
    }

    #[test]
    fn parse_contract_meta_unknown_feature_errors() {
        let src = r#"
        seiyaku C {
            meta { features: ["unknown"]; }
        }
        "#;
        let err = parse(src).unwrap_err();
        assert!(err.contains("unknown feature"));
    }

    #[test]
    fn parse_reports_unexpected_top_level_tokens() {
        let src = "let orphan = 1;";
        let err = parse(src).unwrap_err();
        assert!(err.contains("top-level item"));
    }

    #[test]
    fn parse_reports_unexpected_contract_items() {
        let src = r#"
        seiyaku C {
            123
        }
        "#;
        let err = parse(src).unwrap_err();
        assert!(err.contains("contract item"));
    }

    #[test]
    fn parse_function_modifiers_are_preserved() {
        let src = r#"
        seiyaku Demo {
            kotoage fn foo() permission(Admin) {}
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
        parse(src).expect("parse i64 suffixed literal");
    }

    #[test]
    fn unknown_integer_literal_suffix_errors() {
        let src = "fn main() { let x = 1i128; }";
        let err = parse(src).unwrap_err();
        assert!(err.contains("unknown integer literal suffix `i128`"));
    }

    #[test]
    fn parse_negative_i64_min_literal() {
        let src = "fn main() { let x = -9223372036854775808; }";
        parse(src).expect("parse i64::MIN literal");
    }

    #[test]
    fn parse_positive_i64_overflow_literal_errors() {
        let src = "fn main() { let x = 9223372036854775808; }";
        let err = parse(src).unwrap_err();
        assert!(err.contains("integer literal out of range"));
    }

    #[test]
    fn parse_json_macro_allows_i64_min_literal() {
        let src = r#"fn main() { let x = json!{ v: -9223372036854775808 }; }"#;
        parse(src).expect("parse json min literal");
    }

    #[test]
    fn parse_contract_meta_rejects_integer_overflow_literal() {
        let src = r#"
        seiyaku C {
            meta { abi_version: 9223372036854775808; }
        }
        "#;
        let err = parse(src).unwrap_err();
        assert!(err.contains("integer literal out of range"));
    }

    #[test]
    fn parse_tuple_index_literal() {
        let src = "fn main() { let t = (1, 2); let x = t.1; }";
        parse(src).expect("parse tuple index");
    }

    #[test]
    fn parse_bounded_attribute_literal() {
        let src = "fn f(m: Map<int, int>) { for (k, v) in m #[bounded(1)] { let z = k; } }";
        parse(src).expect("parse bounded attribute");
    }

    #[test]
    fn parse_compound_assignment_keeps_rhs() {
        let src = "fn f() { m[0] += 1; }";
        let prog = parse(src).expect("parse compound assignment");
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
        let prog = parse(src).expect("parse bytes literal");
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
    fn parse_access_attributes_attach_to_function() {
        let src = r#"
        #[access(read="state:Foo", write=["state:Foo/1", "state:Foo/2"])]
        fn main() {}
        "#;
        let prog = parse(src).expect("parse access attributes");
        let func = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(f) => Some(f),
                _ => None,
            })
            .expect("function present");
        assert_eq!(func.modifiers.access_reads, vec!["state:Foo".to_string()]);
        assert_eq!(
            func.modifiers.access_writes,
            vec!["state:Foo/1".to_string(), "state:Foo/2".to_string()]
        );
    }

    #[test]
    fn parse_kotoba_block_collects_entries() {
        let src = r#"
        kotoba {
            "E0001": { en: "Invalid assets", ja: "無効な資産" },
            err_generic: { en: "Something went wrong" }
        }
        fn main() {}
        "#;
        let prog = parse(src).expect("parse kotoba block");
        let kotoba = prog
            .items
            .iter()
            .find_map(|item| match item {
                Item::Kotoba(block) => Some(block),
                _ => None,
            })
            .expect("kotoba block present");
        assert_eq!(kotoba.entries.len(), 2);
        assert_eq!(kotoba.entries[0].msg_id, "E0001");
        assert_eq!(kotoba.entries[0].translations.len(), 2);
        assert_eq!(kotoba.entries[1].msg_id, "err_generic");
        assert_eq!(kotoba.entries[1].translations[0].lang, "en");
    }

    #[test]
    fn parse_trigger_decl() {
        let src = r#"
        seiyaku C {
            kotoage fn run() {}
            register_trigger wake {
                call run;
                on time pre_commit;
                repeats 3;
                authority "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland";
                metadata { tag: "alpha"; count: 1; enabled: true; }
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
        assert_eq!(trigger.name, "wake");
        assert_eq!(trigger.call.entrypoint, "run");
        assert!(matches!(trigger.filter, TriggerFilter::Time(_)));
        assert_eq!(
            trigger.authority.as_deref(),
            Some(
                "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
            )
        );
        assert_eq!(trigger.metadata.len(), 3);
    }

    #[test]
    fn parse_trigger_decl_with_data_filter() {
        let src = r#"
        seiyaku C {
            kotoage fn run() {}
            register_trigger wake {
                call run;
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

    #[test]
    fn parse_trigger_decl_with_pipeline_filter() {
        let src = r#"
        seiyaku C {
            kotoage fn run() {}
            register_trigger wake {
                call run;
                on pipeline transaction;
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
        assert!(matches!(trigger.filter, TriggerFilter::Pipeline(_)));
    }
}
