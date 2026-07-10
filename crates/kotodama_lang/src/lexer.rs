//! Tokenizer for the Kotodama language.
//!
//! The lexer converts a source string into a sequence of [`Token`]s.

use crate::{
    diagnostic::{Diagnostic, DiagnosticBundle, DiagnosticPhase, SourcePosition, SourceSpan},
    source::{FrontendBudget, SourceFile, SourceId, TextRange},
    syntax::SyntaxKind,
};

pub use crate::source::{MAX_NESTING_DEPTH, MAX_SOURCE_BYTES, MAX_TOKENS};

macro_rules! define_v1_keywords {
    ($($spelling:literal => $variant:ident),+ $(,)?) => {
        /// Canonical V1 keyword table consumed by the lexer, formatter,
        /// documentation, and LSP tooling.
        ///
        /// Rejected compatibility and policy spellings are intentionally excluded.
        pub const V1_KEYWORDS: &[&str] = &[$($spelling),+];

        pub(crate) fn v1_keyword_kind(spelling: &str) -> Option<TokenKind> {
            Some(match spelling {
                $($spelling => TokenKind::$variant,)+
                _ => return None,
            })
        }

        #[cfg(test)]
        const V1_KEYWORD_TOKEN_KINDS: &[TokenKind] = &[$(TokenKind::$variant),+];
    };
}

/// Canonical V1 operator and punctuation spellings for language tooling.
pub const V1_OPERATORS: &[&str] = &[
    "+", "-", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "&&", "||", "!", "=", "+=", "-=",
    "*=", "/=", "%=", "->", "::", ".", ",", ":", ";", "?", "(", ")", "{", "}", "[", "]",
];

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum TokenKind {
    Fn,
    Let,
    /// Mutable local binding declaration.
    Var,
    Const,
    Return,
    Break,
    Continue,
    State,
    Struct,
    /// Stable contract error declaration (`error enum Name`).
    Error,
    /// Enumeration keyword used after `error`.
    Enum,
    /// Caller-authorization modifier (`authorize("Permission")`).
    Authorize,
    /// Contract-level trigger declaration (`trigger name -> callback { ... }`).
    Trigger,
    If,
    Else,
    For,
    /// Membership keyword used by bounded `for ... in ...` loops.
    In,
    /// Deployable source-unit keyword (`seiyaku` or `誓約`).
    Seiyaku,
    /// Library source-unit keyword (`module`).
    Module,
    /// Public transaction entrypoint modifier (`kotoage` or `言挙げ`).
    Kotoage,
    /// Contract initializer (`hajimari` or `始まり`).
    Hajimari,
    /// Contract upgrade hook (`kaizen` or `改善`).
    Kaizen,
    /// Read-only public function modifier (`view`).
    View,
    True,
    False,
    Arrow,
    Ident(String),
    /// Unsuffixed or explicitly suffixed integer token.
    ///
    /// The lexer retains the full V1 `u128` domain. The parser assigns an
    /// `i64` type to unsuffixed literals and requires the adjacent `u128`
    /// suffix for values of that type.
    Number(u128),
    String(String),
    Bytes(Vec<u8>),
    Plus,
    PlusEqual,
    Minus,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Star,
    Slash,
    Bang,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semicolon,
    Comma,
    Colon,
    ColonColon,
    Percent,
    Dot,
    LBracket,
    RBracket,
    Question,
    Hash,
    EOF,
}

define_v1_keywords! {
    "authorize" => Authorize,
    "break" => Break,
    "const" => Const,
    "continue" => Continue,
    "else" => Else,
    "enum" => Enum,
    "error" => Error,
    "false" => False,
    "fn" => Fn,
    "for" => For,
    "hajimari" => Hajimari,
    "始まり" => Hajimari,
    "if" => If,
    "in" => In,
    "kaizen" => Kaizen,
    "改善" => Kaizen,
    "kotoage" => Kotoage,
    "言挙げ" => Kotoage,
    "let" => Let,
    "module" => Module,
    "return" => Return,
    "seiyaku" => Seiyaku,
    "誓約" => Seiyaku,
    "state" => State,
    "struct" => Struct,
    "trigger" => Trigger,
    "true" => True,
    "var" => Var,
    "view" => View,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
    /// Exact half-open UTF-8 byte range in the source file.
    pub range: TextRange,
}

/// Lex an entire source string into a vector of [`Token`]s.
pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    if src.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "K0001: source contains {} bytes and exceeds the {MAX_SOURCE_BYTES}-byte Kotodama V1 limit",
            src.len()
        ));
    }
    let source = SourceFile::new(SourceId(0), "<source>", src);
    lex_source(&source, FrontendBudget::v1()).map_err(|bundle| bundle.render_human())
}

/// Lex one source file into the significant, value-carrying token stream used
/// by the AST parser.
///
/// This is an adapter over the lossless lexer, not a second scanner. Formatter,
/// CST, LSP, and compiler consumers therefore agree on token boundaries and
/// resource limits.
pub(crate) fn lex_source(
    source: &SourceFile,
    budget: FrontendBudget,
) -> Result<Vec<Token>, DiagnosticBundle> {
    let lexed = crate::syntax::lexer::lex(source, budget);
    lower_lexed(source, budget, lexed)
}

/// Lower one already-scanned lossless token stream for the AST parser.
pub(crate) fn lower_lexed(
    source: &SourceFile,
    budget: FrontendBudget,
    lexed: crate::syntax::lexer::Lexed,
) -> Result<Vec<Token>, DiagnosticBundle> {
    let retained = budget.max_diagnostics().saturating_sub(1);
    let mut diagnostics = lexed.diagnostics;
    let mut omitted = lexed
        .omitted_diagnostics
        .saturating_add(diagnostics.len().saturating_sub(retained));
    diagnostics.truncate(retained);
    let mut tokens = Vec::with_capacity(lexed.tokens.len());

    for token in lexed.tokens {
        if token.kind.is_trivia() || token.kind == SyntaxKind::ErrorToken {
            continue;
        }
        let text = source.slice(token.range).unwrap_or("");
        match lower_token_kind(token.kind, text) {
            Ok(Some(kind)) => {
                let position = source.line_column(token.range.start);
                tokens.push(Token {
                    kind,
                    line: position.line,
                    column: position.column,
                    range: token.range,
                });
            }
            Ok(None) => {}
            Err(message) => {
                let diagnostic = lexical_diagnostic(source, message, token.range);
                if diagnostics.len() < retained {
                    diagnostics.push(diagnostic);
                } else {
                    omitted = omitted.saturating_add(1);
                }
            }
        }
    }

    if omitted != 0 {
        diagnostics.push(Diagnostic::error(
            "K0004",
            DiagnosticPhase::Lex,
            format!("diagnostic limit reached; {omitted} additional syntax error(s) were omitted"),
            None,
        ));
    }
    if diagnostics.is_empty() {
        Ok(tokens)
    } else {
        Err(DiagnosticBundle::new(diagnostics))
    }
}

fn lexical_diagnostic(
    source: &SourceFile,
    message: impl Into<String>,
    range: TextRange,
) -> Diagnostic {
    let start = source.line_column(range.start);
    let end = source.line_column(range.end);
    Diagnostic::error(
        "K0100",
        DiagnosticPhase::Lex,
        message,
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
            byte_range: Some(range),
        }),
    )
}

fn lower_token_kind(kind: SyntaxKind, text: &str) -> Result<Option<TokenKind>, String> {
    let lowered = match kind {
        SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::BlockComment => {
            return Ok(None);
        }
        SyntaxKind::Ident => TokenKind::Ident(text.to_owned()),
        SyntaxKind::Number => TokenKind::Number(parse_integer_literal(text)?),
        SyntaxKind::String => TokenKind::String(decode_string_literal(text)?),
        SyntaxKind::Bytes => TokenKind::Bytes(decode_byte_literal(text)?),
        SyntaxKind::Eof => TokenKind::EOF,
        SyntaxKind::KwFn => TokenKind::Fn,
        SyntaxKind::KwLet => TokenKind::Let,
        SyntaxKind::KwVar => TokenKind::Var,
        SyntaxKind::KwConst => TokenKind::Const,
        SyntaxKind::KwReturn => TokenKind::Return,
        SyntaxKind::KwBreak => TokenKind::Break,
        SyntaxKind::KwContinue => TokenKind::Continue,
        SyntaxKind::KwState => TokenKind::State,
        SyntaxKind::KwStruct => TokenKind::Struct,
        SyntaxKind::KwError => TokenKind::Error,
        SyntaxKind::KwEnum => TokenKind::Enum,
        SyntaxKind::KwAuthorize => TokenKind::Authorize,
        SyntaxKind::KwTrigger => TokenKind::Trigger,
        SyntaxKind::KwIf => TokenKind::If,
        SyntaxKind::KwElse => TokenKind::Else,
        SyntaxKind::KwFor => TokenKind::For,
        SyntaxKind::KwIn => TokenKind::In,
        SyntaxKind::KwSeiyaku => TokenKind::Seiyaku,
        SyntaxKind::KwModule => TokenKind::Module,
        SyntaxKind::KwKotoage => TokenKind::Kotoage,
        SyntaxKind::KwHajimari => TokenKind::Hajimari,
        SyntaxKind::KwKaizen => TokenKind::Kaizen,
        SyntaxKind::KwView => TokenKind::View,
        SyntaxKind::KwTrue => TokenKind::True,
        SyntaxKind::KwFalse => TokenKind::False,
        SyntaxKind::Plus => TokenKind::Plus,
        SyntaxKind::PlusEqual => TokenKind::PlusEqual,
        SyntaxKind::Minus => TokenKind::Minus,
        SyntaxKind::MinusEqual => TokenKind::MinusEqual,
        SyntaxKind::Arrow => TokenKind::Arrow,
        SyntaxKind::Star => TokenKind::Star,
        SyntaxKind::StarEqual => TokenKind::StarEqual,
        SyntaxKind::Slash => TokenKind::Slash,
        SyntaxKind::SlashEqual => TokenKind::SlashEqual,
        SyntaxKind::Percent => TokenKind::Percent,
        SyntaxKind::PercentEqual => TokenKind::PercentEqual,
        SyntaxKind::Bang => TokenKind::Bang,
        SyntaxKind::BangEqual => TokenKind::BangEqual,
        SyntaxKind::Equal => TokenKind::Equal,
        SyntaxKind::EqualEqual => TokenKind::EqualEqual,
        SyntaxKind::Less => TokenKind::Less,
        SyntaxKind::LessEqual => TokenKind::LessEqual,
        SyntaxKind::Greater => TokenKind::Greater,
        SyntaxKind::GreaterEqual => TokenKind::GreaterEqual,
        SyntaxKind::AndAnd => TokenKind::AndAnd,
        SyntaxKind::OrOr => TokenKind::OrOr,
        SyntaxKind::LParen => TokenKind::LParen,
        SyntaxKind::RParen => TokenKind::RParen,
        SyntaxKind::LBrace => TokenKind::LBrace,
        SyntaxKind::RBrace => TokenKind::RBrace,
        SyntaxKind::LBracket => TokenKind::LBracket,
        SyntaxKind::RBracket => TokenKind::RBracket,
        SyntaxKind::Semicolon => TokenKind::Semicolon,
        SyntaxKind::Comma => TokenKind::Comma,
        SyntaxKind::Colon => TokenKind::Colon,
        SyntaxKind::ColonColon => TokenKind::ColonColon,
        SyntaxKind::Dot => TokenKind::Dot,
        SyntaxKind::Question => TokenKind::Question,
        SyntaxKind::Hash => TokenKind::Hash,
        SyntaxKind::ErrorToken => {
            return Err("invalid Kotodama V1 token".to_owned());
        }
        SyntaxKind::Root
        | SyntaxKind::SourceUnit
        | SyntaxKind::ItemList
        | SyntaxKind::FunctionItem
        | SyntaxKind::StructItem
        | SyntaxKind::ErrorEnumItem
        | SyntaxKind::ConstItem
        | SyntaxKind::StateItem
        | SyntaxKind::TriggerItem
        | SyntaxKind::FixtureItem
        | SyntaxKind::TestTargetItem
        | SyntaxKind::Attribute
        | SyntaxKind::ParamList
        | SyntaxKind::Block
        | SyntaxKind::StatementList
        | SyntaxKind::LetStmt
        | SyntaxKind::ExprStmt
        | SyntaxKind::ReturnStmt
        | SyntaxKind::BreakStmt
        | SyntaxKind::ContinueStmt
        | SyntaxKind::IfStmt
        | SyntaxKind::ForStmt
        | SyntaxKind::ErrorNode
        | SyntaxKind::Missing => {
            return Err("internal frontend node appeared in the token stream".to_owned());
        }
    };
    Ok(Some(lowered))
}

fn parse_integer_literal(text: &str) -> Result<u128, String> {
    let compact = text
        .chars()
        .filter(|character| *character != '_')
        .collect::<String>();
    let (digits, radix) = if let Some(digits) = compact
        .strip_prefix("0x")
        .or_else(|| compact.strip_prefix("0X"))
    {
        (digits, 16)
    } else if let Some(digits) = compact
        .strip_prefix("0b")
        .or_else(|| compact.strip_prefix("0B"))
    {
        (digits, 2)
    } else {
        (compact.as_str(), 10)
    };
    if digits.is_empty() {
        return Err("integer literal requires at least one digit".to_owned());
    }
    u128::from_str_radix(digits, radix).map_err(|_| "numeric literal overflow".to_owned())
}

fn raw_literal_contents(text: &str) -> Option<&str> {
    let quote = text.find('"')?;
    let prefix = &text[..quote];
    let hashes = prefix
        .strip_prefix("br")
        .or_else(|| prefix.strip_prefix("rb"))
        .or_else(|| prefix.strip_prefix('r'))?;
    if !hashes.chars().all(|character| character == '#') {
        return None;
    }
    let closing_len = 1_usize.saturating_add(hashes.len());
    let content_end = text.len().checked_sub(closing_len)?;
    (content_end >= quote + 1).then(|| &text[quote + 1..content_end])
}

fn decode_string_literal(text: &str) -> Result<String, String> {
    if let Some(contents) = raw_literal_contents(text) {
        return Ok(contents.to_owned());
    }
    let contents = text
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| "invalid string literal delimiters".to_owned())?;
    decode_escaped_string(contents)
}

fn decode_byte_literal(text: &str) -> Result<Vec<u8>, String> {
    if let Some(contents) = raw_literal_contents(text) {
        return Ok(contents.as_bytes().to_vec());
    }
    let contents = text
        .strip_prefix("b\"")
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| "invalid byte-string delimiters".to_owned())?;
    decode_escaped_bytes(contents)
}

fn decode_escaped_string(contents: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut characters = contents.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escape = characters
            .next()
            .ok_or_else(|| "unterminated string escape".to_owned())?;
        match escape {
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            '0' => output.push('\0'),
            '"' => output.push('"'),
            '\\' => output.push('\\'),
            'x' => output.push(char::from(read_hex_escape(&mut characters, 2)?)),
            'u' => output.push(read_unicode_escape(&mut characters)?),
            other => return Err(format!("unknown escape `\\{other}`")),
        }
    }
    Ok(output)
}

fn decode_escaped_bytes(contents: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut characters = contents.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            let mut buffer = [0_u8; 4];
            output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            continue;
        }
        let escape = characters
            .next()
            .ok_or_else(|| "unterminated byte-string escape".to_owned())?;
        match escape {
            'n' => output.push(b'\n'),
            'r' => output.push(b'\r'),
            't' => output.push(b'\t'),
            '0' => output.push(0),
            '"' => output.push(b'"'),
            '\\' => output.push(b'\\'),
            'x' => output.push(read_hex_escape(&mut characters, 2)?),
            'u' => {
                let character = read_unicode_escape(&mut characters)?;
                let mut buffer = [0_u8; 4];
                output.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
            other => return Err(format!("unknown escape `\\{other}`")),
        }
    }
    Ok(output)
}

fn read_hex_escape(
    characters: &mut impl Iterator<Item = char>,
    digits: usize,
) -> Result<u8, String> {
    let mut value = 0_u8;
    for _ in 0..digits {
        let character = characters
            .next()
            .ok_or_else(|| "incomplete hexadecimal escape".to_owned())?;
        let digit = character
            .to_digit(16)
            .ok_or_else(|| format!("invalid hex digit `{character}` in escape"))?
            as u8;
        value = value
            .checked_mul(16)
            .and_then(|current| current.checked_add(digit))
            .ok_or_else(|| "hexadecimal escape overflow".to_owned())?;
    }
    Ok(value)
}

fn read_unicode_escape(characters: &mut impl Iterator<Item = char>) -> Result<char, String> {
    if characters.next() != Some('{') {
        return Err("Unicode escape must start with `{`".to_owned());
    }
    let mut digits = String::new();
    for character in characters.by_ref() {
        if character == '}' {
            if digits.is_empty() {
                return Err("empty Unicode escape".to_owned());
            }
            let value = u32::from_str_radix(&digits, 16)
                .map_err(|_| "invalid Unicode escape".to_owned())?;
            return char::from_u32(value).ok_or_else(|| "invalid Unicode scalar value".to_owned());
        }
        if !character.is_ascii_hexdigit() {
            return Err(format!("invalid hex digit `{character}` in Unicode escape"));
        }
        digits.push(character);
        if digits.len() > 6 {
            return Err("Unicode escape is longer than six digits".to_owned());
        }
    }
    Err("unterminated Unicode escape".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_NESTING_DEPTH, MAX_SOURCE_BYTES, MAX_TOKENS, TokenKind, V1_KEYWORD_TOKEN_KINDS,
        V1_KEYWORDS, lex, lex_source,
    };
    use crate::{
        source::{FrontendBudget, SourceFile, SourceId},
        syntax::SyntaxKind,
    };

    #[test]
    fn canonical_keyword_table_drives_lexer_tokens_without_drift() {
        assert_eq!(V1_KEYWORDS.len(), V1_KEYWORD_TOKEN_KINDS.len());
        for (spelling, expected) in V1_KEYWORDS.iter().zip(V1_KEYWORD_TOKEN_KINDS) {
            let tokens = lex(spelling).expect("canonical keyword must lex");
            assert_eq!(&tokens[0].kind, expected, "keyword `{spelling}`");
        }
        for rejected in [
            "contract",
            "entry",
            "init",
            "meta",
            "permission",
            "this",
            "upgrade",
            "while",
        ] {
            assert!(
                !V1_KEYWORDS.contains(&rejected),
                "rejected spelling `{rejected}` leaked into the V1 table"
            );
            let tokens = lex(rejected).expect("retired spelling is an ordinary identifier");
            assert!(
                matches!(&tokens[0].kind, TokenKind::Ident(name) if name == rejected),
                "retired spelling `{rejected}` must not have a dedicated token"
            );
        }
    }

    #[test]
    fn retired_operators_are_rejected_without_semantic_tokens() {
        for spelling in ["++", "&", "|"] {
            let error = lex(spelling).expect_err("retired operator must be rejected lexically");
            assert!(
                error.contains("invalid Kotodama V1 operator")
                    || error.contains("invalid source character"),
                "unexpected diagnostic for `{spelling}`: {error}"
            );
        }

        for spelling in ["&&", "||"] {
            lex(spelling).expect("canonical short-circuit operator must remain supported");
        }
    }

    #[test]
    fn source_size_limit_is_enforced_before_lexing() {
        let source = " ".repeat(MAX_SOURCE_BYTES + 1);
        let err = lex(&source).expect_err("oversized source must fail");
        assert!(err.contains("K0001"));
    }

    #[test]
    fn token_limit_is_enforced() {
        let source = "a ".repeat(MAX_TOKENS);
        let err = lex(&source).expect_err("excess token count must fail");
        assert!(err.contains("K0002"));
    }

    #[test]
    fn nesting_limit_is_enforced() {
        let source = format!(
            "{}0{}",
            "(".repeat(MAX_NESTING_DEPTH + 1),
            ")".repeat(MAX_NESTING_DEPTH + 1)
        );
        let err = lex(&source).expect_err("excess nesting must fail");
        assert!(err.contains("K0003"));
    }

    #[test]
    fn decimal_literal_overflow_is_reported() {
        let err = lex("340282366920938463463374607431768211456").unwrap_err();
        assert!(err.contains("overflow"));
    }

    #[test]
    fn complete_u128_domain_is_tokenized() {
        let tokens = lex("340282366920938463463374607431768211455u128").expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::Number(n) if n == u128::MAX),
            "expected u128 literal token, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn decimal_fraction_literal_is_rejected() {
        let error = lex("1_234.50_0").expect_err("fractional literal must be rejected");
        assert!(
            error.contains("decimal fractions are not part of Kotodama V1"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn var_is_a_dedicated_keyword() {
        let tokens = lex("var value = 1;").expect("lex var binding");
        assert!(matches!(tokens[0].kind, TokenKind::Var));
    }

    #[test]
    fn branded_declaration_keywords_are_reserved_in_both_scripts() {
        for (spelling, expected) in [
            ("seiyaku", TokenKind::Seiyaku),
            ("誓約", TokenKind::Seiyaku),
            ("kotoage", TokenKind::Kotoage),
            ("言挙げ", TokenKind::Kotoage),
            ("hajimari", TokenKind::Hajimari),
            ("始まり", TokenKind::Hajimari),
            ("kaizen", TokenKind::Kaizen),
            ("改善", TokenKind::Kaizen),
        ] {
            let tokens = lex(spelling).expect("branded keyword must lex");
            assert_eq!(tokens[0].kind, expected, "keyword `{spelling}`");
        }
    }

    #[test]
    fn non_ascii_identifiers_are_rejected() {
        for spelling in ["café", "誓約名", "利用者", "言挙げrun"] {
            let error = lex(spelling).expect_err("V1 identifiers must be ASCII");
            assert!(
                error.contains("non-ASCII"),
                "unexpected diagnostic for `{spelling}`: {error}"
            );
        }
    }

    #[test]
    fn hex_literal_overflow_is_reported() {
        let err = lex("0x1_0000_0000_0000_0000_0000_0000_0000_0000").unwrap_err();
        assert!(err.contains("overflow"));
    }

    #[test]
    fn binary_literal_overflow_is_reported() {
        let literal = format!("0b1{}", "0".repeat(128));
        let err = lex(&literal).unwrap_err();
        assert!(err.contains("overflow"));
    }

    #[test]
    fn unterminated_block_comment_errors() {
        let err = lex("/* never ends").unwrap_err();
        assert!(err.contains("unterminated block comment"));
    }

    #[test]
    fn unterminated_string_detected() {
        let err = lex("\"hello").unwrap_err();
        assert!(err.contains("unterminated string literal"));
    }

    #[test]
    fn newline_in_string_is_rejected() {
        let err = lex("\"hello\nworld\"").unwrap_err();
        assert!(err.contains("unterminated string literal"));
    }

    #[test]
    fn string_hex_and_unicode_escapes_are_parsed() {
        let tokens = lex("\"A\\x42\\u{43}\"").expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::String(ref s) if s == "ABC"),
            "expected ABC, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn raw_string_preserves_backslashes() {
        let tokens = lex(r#"r"hello\n""#).expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::String(ref s) if s == "hello\\n"),
            "expected raw string literal, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn raw_string_with_hashes_allows_quotes() {
        let tokens = lex(r##"r#"a "quote""#"##).expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::String(ref s) if s == "a \"quote\""),
            "expected raw string with quotes, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn byte_string_parses_escapes() {
        let tokens = lex("b\"ab\\x41\"").expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::Bytes(ref b) if b == b"abA"),
            "expected byte literal, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn raw_byte_string_ignores_escapes() {
        let tokens = lex(r#"br"ab\n""#).expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::Bytes(ref b) if b == b"ab\\n"),
            "expected raw byte literal, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn invalid_hex_escape_reports_error() {
        let err = lex("\"\\xG1\"").unwrap_err();
        assert!(err.contains("invalid hex digit"));
    }

    #[test]
    fn invalid_unicode_escape_reports_error() {
        let err = lex("\"\\u{}\"").unwrap_err();
        assert!(err.contains("empty Unicode escape"));
    }

    #[test]
    fn semantic_tokens_reuse_lossless_token_boundaries() {
        let text = r##"seiyaku Demo { // trivia
            kotoage fn run(value: i64) authorize("Run") {
                let raw: string = r#"日本語"#;
                let data: bytes = br"a\n";
            }
        }"##;
        let source = SourceFile::new(SourceId(41), "boundaries.ko", text);
        let lossless = crate::syntax::lexer::lex(&source, FrontendBudget::v1());
        let expected = lossless
            .tokens
            .iter()
            .filter(|token| !token.kind.is_trivia() && token.kind != SyntaxKind::ErrorToken)
            .map(|token| token.range)
            .collect::<Vec<_>>();
        let actual = lex_source(&source, FrontendBudget::v1())
            .expect("lower canonical lossless token stream")
            .into_iter()
            .map(|token| token.range)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn typed_constructor_path_is_one_spanned_token_stream() {
        let text = r#"module Typed {
            fn parse_id() { let id = AccountId::parse("alice@wonderland"); }
        }"#;
        let source = SourceFile::new(SourceId(42), "typed-path.ko", text);
        let tokens = lex_source(&source, FrontendBudget::v1()).expect("lex typed path");
        let separator = tokens
            .iter()
            .find(|token| token.kind == TokenKind::ColonColon)
            .expect("typed path separator");
        assert_eq!(source.slice(separator.range), Some("::"));
        let start = source.line_column(separator.range.start);
        let end = source.line_column(separator.range.end);
        assert_eq!(start.line, 2);
        assert_eq!(end.column, start.column + 2);

        let program = crate::parser::parse_source(&source, FrontendBudget::v1())
            .expect("parse uppercase typed constructor");
        let crate::ast::Item::Function(function) = &program.items[0] else {
            panic!("expected function item")
        };
        let crate::ast::Statement::Let { value, .. } = &function.body.statements[0] else {
            panic!("expected constructor binding")
        };
        assert!(matches!(
            value,
            crate::ast::Expr::Call { name, .. } if name == "AccountId::parse"
        ));
    }
}
