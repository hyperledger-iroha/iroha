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
            if self.peek(TokenKind::Fn) {
                items.push(self.parse_item()?);
            } else if self.peek(TokenKind::Struct) {
                items.push(self.parse_struct_def()?);
            } else if self.peek(TokenKind::Seiyaku) {
                let mut contract_items = self.parse_contract()?;
                items.append(&mut contract_items);
            } else if self.peek(TokenKind::State) {
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
                    },
                )?);
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
            if self.peek(TokenKind::Meta) {
                self.parse_meta_block()?;
            } else if self.peek(TokenKind::Struct) {
                items.push(self.parse_struct_def()?);
            } else if self.peek(TokenKind::State) {
                items.push(self.parse_state_decl()?);
            } else if self.peek(TokenKind::Fn) {
                self.bump();
                items.push(self.parse_fn_loose(
                    None,
                    FunctionModifiers {
                        visibility: FunctionVisibility::Internal,
                        kind: FunctionKind::Contract,
                        permission: None,
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
                    },
                )?);
            } else if self.peek_ident_n(0, "kotoba") {
                // Explicitly reject kotoba blocks: no i18n in contracts.
                let tok = self.bump();
                return Err(ParseError {
                    message: "kotoba blocks were removed; i18n is not part of contract source"
                        .into(),
                    line: tok.line,
                    column: tok.column,
                    snippet: String::new(),
                });
            } else {
                let tok = self.bump();
                return Err(self.error(tok, "contract item (fn, struct, state, meta)"));
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(items)
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
            // Allow stray semicolons
            if self.peek(TokenKind::Semicolon) {
                self.bump();
                continue;
            }
            // Expect: ident ':' type ';'
            let field_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type_expr()?;
            fields.push((field_name, ty));
            if self.peek(TokenKind::Semicolon) {
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

    fn parse_item(&mut self) -> ParseResult<Item> {
        self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;
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
        let body = self.parse_block()?;
        Ok(Item::Function(Function {
            name,
            params,
            ret_ty,
            body,
            modifiers: FunctionModifiers::default(),
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
                    return Ok(match (target, op_tok) {
                        (Expr::Ident(name), TokenKind::Equal) => {
                            Statement::Assign { name, value: rhs }
                        }
                        (t, TokenKind::Equal) => Statement::AssignExpr {
                            target: t,
                            value: rhs,
                        },
                        (t, TokenKind::PlusEqual) => Statement::AssignExpr {
                            target: t.clone(),
                            value: Expr::Binary {
                                op: BinaryOp::Add,
                                left: Box::new(t),
                                right: Box::new(rhs),
                            },
                        },
                        (t, TokenKind::MinusEqual) => Statement::AssignExpr {
                            target: t.clone(),
                            value: Expr::Binary {
                                op: BinaryOp::Sub,
                                left: Box::new(t),
                                right: Box::new(rhs),
                            },
                        },
                        (t, TokenKind::StarEqual) => Statement::AssignExpr {
                            target: t.clone(),
                            value: Expr::Binary {
                                op: BinaryOp::Mul,
                                left: Box::new(t),
                                right: Box::new(rhs),
                            },
                        },
                        (t, TokenKind::SlashEqual) => Statement::AssignExpr {
                            target: t.clone(),
                            value: Expr::Binary {
                                op: BinaryOp::Div,
                                left: Box::new(t),
                                right: Box::new(rhs),
                            },
                        },
                        (t, TokenKind::PercentEqual) => Statement::AssignExpr {
                            target: t.clone(),
                            value: Expr::Binary {
                                op: BinaryOp::Mod,
                                left: Box::new(t),
                                right: Box::new(rhs),
                            },
                        },
                        _ => unreachable!(),
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
            TokenKind::String(s) => Ok(Expr::String(s.clone())),
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
                let field = self.expect_ident()?;
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

    fn parse_for_each_map(
        &mut self,
    ) -> ParseResult<Option<(String, Option<String>, Expr, Option<usize>)>> {
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
}
