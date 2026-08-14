//! Lossless, recovering Kotodama lexer.
use super::{cst::GreenToken, kind::SyntaxKind};
use crate::{
    diagnostic::{Diagnostic, DiagnosticFix, DiagnosticPhase, SourcePosition, SourceSpan},
    lexer::{TokenKind, V1_PUNCTUATION_KINDS, v1_keyword_kind},
    source::{FrontendBudget, SourceFile, TextRange},
};
/// Lossless lexer output.
#[derive(Clone, Debug)]
pub struct Lexed {
    /// Tokens in source order, including trivia and end-of-file.
    pub tokens: Vec<GreenToken>,
    /// Bounded lexical and budget diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Lexical diagnostics discarded after reaching the fixed V1 cap.
    pub(crate) omitted_diagnostics: usize,
}
fn diagnostic(
    source: &SourceFile,
    code: &'static str,
    message: impl Into<String>,
    range: TextRange,
) -> Diagnostic {
    let start = source.line_column(range.start);
    let end = source.line_column(range.end);
    Diagnostic::error(
        code,
        DiagnosticPhase::Lex,
        message,
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
            byte_range: Some(range),
        }),
    )
}
fn keyword_kind(text: &str) -> SyntaxKind {
    match v1_keyword_kind(text) {
        Some(TokenKind::Fn) => SyntaxKind::KwFn,
        Some(TokenKind::Let) => SyntaxKind::KwLet,
        Some(TokenKind::Var) => SyntaxKind::KwVar,
        Some(TokenKind::Const) => SyntaxKind::KwConst,
        Some(TokenKind::Return) => SyntaxKind::KwReturn,
        Some(TokenKind::Break) => SyntaxKind::KwBreak,
        Some(TokenKind::Continue) => SyntaxKind::KwContinue,
        Some(TokenKind::State) => SyntaxKind::KwState,
        Some(TokenKind::Struct) => SyntaxKind::KwStruct,
        Some(TokenKind::Error) => SyntaxKind::KwError,
        Some(TokenKind::Enum) => SyntaxKind::KwEnum,
        Some(TokenKind::Authorize) => SyntaxKind::KwAuthorize,
        Some(TokenKind::Trigger) => SyntaxKind::KwTrigger,
        Some(TokenKind::If) => SyntaxKind::KwIf,
        Some(TokenKind::Match) => SyntaxKind::KwMatch,
        Some(TokenKind::Else) => SyntaxKind::KwElse,
        Some(TokenKind::For) => SyntaxKind::KwFor,
        Some(TokenKind::In) => SyntaxKind::KwIn,
        Some(TokenKind::Seiyaku) => SyntaxKind::KwSeiyaku,
        Some(TokenKind::Module) => SyntaxKind::KwModule,
        Some(TokenKind::Kotoage) => SyntaxKind::KwKotoage,
        Some(TokenKind::Hajimari) => SyntaxKind::KwHajimari,
        Some(TokenKind::Kaizen) => SyntaxKind::KwKaizen,
        Some(TokenKind::View) => SyntaxKind::KwView,
        Some(TokenKind::True) => SyntaxKind::KwTrue,
        Some(TokenKind::False) => SyntaxKind::KwFalse,
        Some(other) => unreachable!("non-keyword token in V1 keyword table: {other:?}"),
        None => SyntaxKind::Ident,
    }
}
struct Scanner<'source> {
    text: &'source str,
    pos: usize,
    previous_significant: Option<SyntaxKind>,
}
#[derive(Clone, Copy)]
struct LexicalError {
    code: &'static str,
    message: &'static str,
    strip_numeric_suffix: bool,
}
impl LexicalError {
    const fn new(code: &'static str, message: &'static str) -> Self {
        Self {
            code,
            message,
            strip_numeric_suffix: false,
        }
    }
    fn retired_numeric_suffix(suffix: &str) -> Self {
        Self {
            code: "E_RETIRED_NUMERIC_SUFFIX",
            message: "numeric literal suffixes are not part of Kotodama V1; use an unsuffixed literal in an int, decimal, or quantity context",
            strip_numeric_suffix: matches!(suffix, "amt" | "qty"),
        }
    }
}
#[derive(Clone, Copy)]
enum ScannedNumber {
    Integer,
    Decimal,
    Invalid(LexicalError),
}
impl<'source> Scanner<'source> {
    fn new(source: &'source SourceFile) -> Self {
        Self {
            text: source.text(),
            pos: 0,
            previous_significant: None,
        }
    }
    fn rest(&self) -> &'source str {
        &self.text[self.pos..]
    }
    fn current(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let character = self.current()?;
        self.pos += character.len_utf8();
        Some(character)
    }
    fn starts_with(&self, pattern: &str) -> bool {
        self.rest().starts_with(pattern)
    }
    fn scan_whitespace(&mut self) {
        while self.current().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }
    fn scan_line_comment(&mut self) {
        self.pos += 2;
        while let Some(character) = self.bump() {
            if character == '\n' {
                break;
            }
        }
    }
    fn scan_block_comment(&mut self) -> bool {
        self.pos += 2;
        while self.pos < self.text.len() {
            if self.starts_with("*/") {
                self.pos += 2;
                return true;
            }
            self.bump();
        }
        false
    }
    fn scan_identifier(&mut self) {
        while self
            .current()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
        {
            self.bump();
        }
    }
    /// Scan an unsuffixed integer or exact base-10 decimal token.
    fn scan_number(&mut self, tuple_index: bool) -> ScannedNumber {
        if self.starts_with("0x") || self.starts_with("0X") {
            self.pos += 2;
            while self
                .current()
                .is_some_and(|character| character.is_ascii_hexdigit() || character == '_')
            {
                self.bump();
            }
            return self.finish_integer();
        }
        if self.starts_with("0b") || self.starts_with("0B") {
            self.pos += 2;
            while self
                .current()
                .is_some_and(|character| matches!(character, '0' | '1' | '_'))
            {
                self.bump();
            }
            return self.finish_integer();
        }
        while self
            .current()
            .is_some_and(|character| character.is_ascii_digit() || character == '_')
        {
            self.bump();
        }
        let mut has_fraction = false;
        if self.starts_with(".") && !tuple_index {
            let after_dot = self.rest()[1..].chars().next();
            if after_dot.is_some_and(|character| character.is_ascii_digit() || character == '_') {
                has_fraction = true;
            } else if !tuple_index {
                self.bump();
                return ScannedNumber::Invalid(LexicalError::new(
                    "E_DECIMAL_MALFORMED",
                    "decimal literals require at least one digit after `.`",
                ));
            }
        }
        if has_fraction {
            self.bump();
            while self
                .current()
                .is_some_and(|character| character.is_ascii_digit() || character == '_')
            {
                self.bump();
            }
        }
        let mut has_exponent = false;
        if matches!(self.current(), Some('e' | 'E')) {
            has_exponent = true;
            self.bump();
            if matches!(self.current(), Some('+' | '-')) {
                self.bump();
            }
            let exponent_start = self.pos;
            while self
                .current()
                .is_some_and(|character| character.is_ascii_digit() || character == '_')
            {
                self.bump();
            }
            if self.pos == exponent_start {
                return ScannedNumber::Invalid(LexicalError::new(
                    "E_DECIMAL_EXPONENT",
                    "decimal exponent requires at least one digit",
                ));
            }
        }
        if self
            .current()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            let suffix_start = self.pos;
            self.scan_identifier();
            return ScannedNumber::Invalid(LexicalError::retired_numeric_suffix(
                &self.text[suffix_start..self.pos],
            ));
        }
        if has_fraction || has_exponent {
            ScannedNumber::Decimal
        } else {
            ScannedNumber::Integer
        }
    }
    fn finish_integer(&mut self) -> ScannedNumber {
        if self
            .current()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            let suffix_start = self.pos;
            self.scan_identifier();
            ScannedNumber::Invalid(LexicalError::retired_numeric_suffix(
                &self.text[suffix_start..self.pos],
            ))
        } else {
            ScannedNumber::Integer
        }
    }
    fn scan_quoted(&mut self, prefix_bytes: usize) -> bool {
        self.pos += prefix_bytes;
        let Some('"') = self.bump() else {
            return false;
        };
        let mut escaped = false;
        while let Some(character) = self.current() {
            if character == '\n' || character == '\r' {
                return false;
            }
            self.bump();
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                return true;
            }
        }
        false
    }
    fn raw_prefix(&self) -> Option<(bool, usize, usize)> {
        let bytes = self.text.as_bytes();
        let mut cursor = self.pos;
        let is_bytes = if (bytes.get(cursor) == Some(&b'b') && bytes.get(cursor + 1) == Some(&b'r'))
            || (bytes.get(cursor) == Some(&b'r') && bytes.get(cursor + 1) == Some(&b'b'))
        {
            cursor += 2;
            true
        } else if bytes.get(cursor) == Some(&b'r') {
            cursor += 1;
            false
        } else {
            return None;
        };
        let mut hashes = 0_usize;
        while bytes.get(cursor) == Some(&b'#') {
            cursor += 1;
            hashes += 1;
        }
        (bytes.get(cursor) == Some(&b'"')).then_some((is_bytes, cursor + 1, hashes))
    }
    fn scan_raw(&mut self, content_start: usize, hashes: usize) -> bool {
        self.pos = content_start;
        let bytes = self.text.as_bytes();
        while self.pos < bytes.len() {
            if bytes[self.pos] == b'"' {
                let hashes_start = self.pos + 1;
                let hashes_end = hashes_start.saturating_add(hashes);
                if hashes_end <= bytes.len()
                    && bytes[hashes_start..hashes_end]
                        .iter()
                        .all(|byte| *byte == b'#')
                {
                    self.pos = hashes_end;
                    return true;
                }
            }
            self.bump();
        }
        false
    }
    fn punctuation(&mut self) -> Option<SyntaxKind> {
        for &(spelling, kind) in V1_PUNCTUATION_KINDS {
            if self.starts_with(spelling) {
                self.pos += spelling.len();
                return Some(kind);
            }
        }
        // Preserve the scanner's progress guarantee for invalid input.
        self.bump();
        None
    }
    fn next_token(&mut self) -> (GreenToken, Option<LexicalError>) {
        let start = self.pos;
        let Some(character) = self.current() else {
            return (
                GreenToken::source(
                    SyntaxKind::Eof,
                    TextRange::empty(self.text.len().min(u32::MAX as usize) as u32),
                ),
                None,
            );
        };
        let (kind, error) = if character.is_whitespace() {
            self.scan_whitespace();
            (SyntaxKind::Whitespace, None)
        } else if self.starts_with("//") {
            self.scan_line_comment();
            (SyntaxKind::LineComment, None)
        } else if self.starts_with("/*") {
            let terminated = self.scan_block_comment();
            (
                if terminated {
                    SyntaxKind::BlockComment
                } else {
                    SyntaxKind::ErrorToken
                },
                (!terminated).then_some(LexicalError::new("K0100", "unterminated block comment")),
            )
        } else if let Some((is_bytes, content_start, hashes)) = self.raw_prefix() {
            let terminated = self.scan_raw(content_start, hashes);
            (
                if terminated {
                    if is_bytes {
                        SyntaxKind::Bytes
                    } else {
                        SyntaxKind::String
                    }
                } else {
                    SyntaxKind::ErrorToken
                },
                (!terminated).then_some(LexicalError::new(
                    "K0100",
                    "unterminated raw string literal",
                )),
            )
        } else if self.starts_with("b\"") {
            let terminated = self.scan_quoted(1);
            (
                if terminated {
                    SyntaxKind::Bytes
                } else {
                    SyntaxKind::ErrorToken
                },
                (!terminated).then_some(LexicalError::new(
                    "K0100",
                    "unterminated byte string literal",
                )),
            )
        } else if character == '"' {
            let terminated = self.scan_quoted(0);
            (
                if terminated {
                    SyntaxKind::String
                } else {
                    SyntaxKind::ErrorToken
                },
                (!terminated).then_some(LexicalError::new("K0100", "unterminated string literal")),
            )
        } else if character.is_alphabetic() || character == '_' {
            self.scan_identifier();
            let text = &self.text[start..self.pos];
            let kind = keyword_kind(text);
            if kind == SyntaxKind::Ident && !text.is_ascii() {
                (
                    SyntaxKind::ErrorToken,
                    Some(LexicalError::new(
                        "K0100",
                        "non-ASCII identifier outside the branded Japanese keyword set",
                    )),
                )
            } else {
                (kind, None)
            }
        } else if character.is_ascii_digit() {
            let tuple_index = self.previous_significant == Some(SyntaxKind::Dot);
            match self.scan_number(tuple_index) {
                ScannedNumber::Integer => (SyntaxKind::Number, None),
                ScannedNumber::Decimal => (SyntaxKind::Decimal, None),
                ScannedNumber::Invalid(error) => (SyntaxKind::ErrorToken, Some(error)),
            }
        } else if self.starts_with("++") {
            self.pos += 2;
            (
                SyntaxKind::ErrorToken,
                Some(LexicalError::new("K0100", "invalid Kotodama V1 operator")),
            )
        } else if let Some(kind) = self.punctuation() {
            (kind, None)
        } else {
            // `punctuation` consumed exactly one Unicode scalar before
            // reporting failure.
            (
                SyntaxKind::ErrorToken,
                Some(LexicalError::new(
                    "K0100",
                    if character.is_ascii() {
                        "invalid source character"
                    } else {
                        "non-ASCII character outside a string or comment"
                    },
                )),
            )
        };
        let end = self.pos;
        if !kind.is_trivia() && kind != SyntaxKind::Eof {
            self.previous_significant = Some(kind);
        }
        (
            GreenToken::source(
                kind,
                TextRange::new(
                    start.min(u32::MAX as usize) as u32,
                    end.min(u32::MAX as usize) as u32,
                ),
            ),
            error,
        )
    }
}
fn record_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    omitted_diagnostics: &mut usize,
    budget: FrontendBudget,
    diagnostic: Diagnostic,
) {
    if diagnostics.len() < budget.max_diagnostics() {
        diagnostics.push(diagnostic);
    } else {
        *omitted_diagnostics = omitted_diagnostics.saturating_add(1);
    }
}
/// Lex one source file without discarding trivia or malformed text.
#[must_use]
pub fn lex(source: &SourceFile, budget: FrontendBudget) -> Lexed {
    if source.original_len() > budget.max_source_bytes() {
        let range = source.full_range();
        return Lexed {
            tokens: vec![
                GreenToken::source(SyntaxKind::ErrorToken, range),
                GreenToken::source(SyntaxKind::Eof, TextRange::empty(range.end)),
            ],
            diagnostics: vec![diagnostic(
                source,
                "K0001",
                format!(
                    "source contains {} bytes and exceeds the {}-byte compiler limit",
                    source.original_len(),
                    budget.max_source_bytes(),
                ),
                range,
            )],
            omitted_diagnostics: 0,
        };
    }
    let mut scanner = Scanner::new(source);
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut omitted_diagnostics = 0_usize;
    let mut significant_tokens = 0_usize;
    let mut delimiter_depth = 0_usize;
    let mut nesting_reported = false;
    loop {
        let (token, lexical_error) = scanner.next_token();
        if !token.kind.is_trivia() && token.kind != SyntaxKind::Eof {
            if significant_tokens >= budget.max_tokens().saturating_sub(1) {
                let collapsed = TextRange::new(token.range.start, source.full_range().end);
                tokens.push(GreenToken::source(SyntaxKind::ErrorToken, collapsed));
                tokens.push(GreenToken::source(
                    SyntaxKind::Eof,
                    TextRange::empty(collapsed.end),
                ));
                record_diagnostic(
                    &mut diagnostics,
                    &mut omitted_diagnostics,
                    budget,
                    diagnostic(
                        source,
                        "K0002",
                        format!(
                            "source exceeds the {}-token compiler limit",
                            budget.max_tokens()
                        ),
                        collapsed,
                    ),
                );
                break;
            }
            significant_tokens = significant_tokens.saturating_add(1);
        }
        match token.kind {
            SyntaxKind::LParen | SyntaxKind::LBrace | SyntaxKind::LBracket => {
                delimiter_depth = delimiter_depth.saturating_add(1);
                if delimiter_depth > budget.max_nesting() && !nesting_reported {
                    nesting_reported = true;
                    record_diagnostic(
                        &mut diagnostics,
                        &mut omitted_diagnostics,
                        budget,
                        diagnostic(
                            source,
                            "K0003",
                            format!(
                                "source exceeds the {}-level nesting limit",
                                budget.max_nesting()
                            ),
                            token.range,
                        ),
                    );
                }
            }
            SyntaxKind::RParen | SyntaxKind::RBrace | SyntaxKind::RBracket => {
                delimiter_depth = delimiter_depth.saturating_sub(1);
            }
            _ => {}
        }
        if let Some(error) = lexical_error {
            let mut emitted = diagnostic(source, error.code, error.message, token.range);
            if error.strip_numeric_suffix {
                let literal = source
                    .slice(token.range)
                    .expect("scanner token range must remain within the source");
                let replacement = literal
                    .strip_suffix("amt")
                    .or_else(|| literal.strip_suffix("qty"))
                    .expect("numeric suffix fix is only enabled for amt or qty");
                emitted.fix = emitted.primary_span.clone().map(|span| DiagnosticFix {
                    span,
                    replacement: replacement.to_owned(),
                });
            }
            record_diagnostic(&mut diagnostics, &mut omitted_diagnostics, budget, emitted);
        }
        let end = token.kind == SyntaxKind::Eof;
        tokens.push(token);
        if end {
            break;
        }
    }
    Lexed {
        tokens,
        diagnostics,
        omitted_diagnostics,
    }
}
#[cfg(test)]
mod tests {
    use super::lex;
    use crate::{
        lexer::{V1_OPERATORS, V1_PUNCTUATION_KINDS},
        source::{FrontendBudget, SourceFile, SourceId},
        syntax::SyntaxKind,
    };
    #[test]
    fn branded_keywords_are_accepted_in_both_scripts() {
        let source = SourceFile::new(
            SourceId(0),
            "branded-keywords.ko",
            "誓約 Demo { 始まり() {} 言挙げ fn run() authorize(\"Run\") {} 改善() {} }",
        );
        let lexed = lex(&source, FrontendBudget::v1());
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        for expected in [
            SyntaxKind::KwSeiyaku,
            SyntaxKind::KwHajimari,
            SyntaxKind::KwKotoage,
            SyntaxKind::KwKaizen,
        ] {
            assert!(lexed.tokens.iter().any(|token| token.kind == expected));
        }
    }
    #[test]
    fn branded_keywords_do_not_enable_unicode_identifiers() {
        for text in ["利用者", "誓約名", "始まり名", "改善版", "言挙げrun"] {
            let source = SourceFile::new(SourceId(0), "invalid.ko", text);
            let lexed = lex(&source, FrontendBudget::v1());
            assert!(
                !lexed.diagnostics.is_empty(),
                "invalid Unicode identifier `{text}` was accepted"
            );
            assert_eq!(lexed.tokens[0].kind, SyntaxKind::ErrorToken);
        }
    }
    #[test]
    fn retired_words_are_plain_identifiers_and_retired_operators_are_errors() {
        for text in [
            "contract",
            "entry",
            "init",
            "permission",
            "meta",
            "this",
            "upgrade",
            "while",
        ] {
            let source = SourceFile::new(SourceId(0), "retired-word.ko", text);
            let lexed = lex(&source, FrontendBudget::v1());
            assert!(
                lexed.diagnostics.is_empty(),
                "{text}: {:?}",
                lexed.diagnostics
            );
            assert_eq!(lexed.tokens[0].kind, SyntaxKind::Ident, "{text}");
        }
        for text in ["++", "&", "|"] {
            let source = SourceFile::new(SourceId(0), "retired-operator.ko", text);
            let lexed = lex(&source, FrontendBudget::v1());
            assert!(!lexed.diagnostics.is_empty(), "{text} must be rejected");
            assert_eq!(lexed.tokens[0].kind, SyntaxKind::ErrorToken, "{text}");
        }
    }
    #[test]
    fn normative_operator_table_drives_the_lossless_scanner() {
        assert_eq!(V1_OPERATORS.len(), V1_PUNCTUATION_KINDS.len());
        for &spelling in V1_OPERATORS {
            let expected = V1_PUNCTUATION_KINDS
                .iter()
                .find_map(|(candidate, kind)| (*candidate == spelling).then_some(*kind))
                .expect("every documented operator has a generated scanner kind");
            let source = SourceFile::new(SourceId(0), "operator.ko", spelling);
            let lexed = lex(&source, FrontendBudget::v1());
            assert!(
                lexed.diagnostics.is_empty(),
                "{spelling}: {:?}",
                lexed.diagnostics
            );
            assert_eq!(lexed.tokens[0].kind, expected, "{spelling}");
            assert_eq!(source.slice(lexed.tokens[0].range), Some(spelling));
            assert_eq!(lexed.tokens[1].kind, SyntaxKind::Eof, "{spelling}");
        }
    }
    #[test]
    fn decimal_is_one_lossless_token_with_exact_range() {
        let source = SourceFile::new(SourceId(0), "decimal.ko", "  1.250_0 // exact\n");
        let lexed = lex(&source, FrontendBudget::v1());
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let decimal = lexed
            .tokens
            .iter()
            .find(|token| token.kind == SyntaxKind::Decimal)
            .expect("decimal token");
        assert_eq!(source.slice(decimal.range), Some("1.250_0"));
        assert_eq!(decimal.range.start, 2);
        assert_eq!(decimal.range.end, 9);
    }
    #[test]
    fn chained_tuple_projection_keeps_following_dots_as_punctuation() {
        let source = SourceFile::new(SourceId(0), "tuple-projection.ko", "value.0.1.field");
        let lexed = lex(&source, FrontendBudget::v1());
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let kinds = lexed
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                SyntaxKind::Ident,
                SyntaxKind::Dot,
                SyntaxKind::Number,
                SyntaxKind::Dot,
                SyntaxKind::Number,
                SyntaxKind::Dot,
                SyntaxKind::Ident,
                SyntaxKind::Eof,
            ]
        );
    }
    #[test]
    fn comment_period_does_not_turn_a_following_decimal_into_a_tuple_index() {
        let source = SourceFile::new(
            SourceId(0),
            "comment-decimal.ko",
            "let value = // exact.\n  1.25;",
        );
        let lexed = lex(&source, FrontendBudget::v1());
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        assert!(
            lexed
                .tokens
                .iter()
                .any(|token| token.kind == SyntaxKind::Decimal
                    && source.slice(token.range) == Some("1.25"))
        );
    }
    #[test]
    fn decimal_and_retired_suffix_failures_have_dedicated_codes() {
        for (spelling, code) in [
            ("1.amt", "E_DECIMAL_MALFORMED"),
            ("1e", "E_DECIMAL_EXPONENT"),
            ("1.25amt", "E_RETIRED_NUMERIC_SUFFIX"),
            ("1.25qty", "E_RETIRED_NUMERIC_SUFFIX"),
            ("0x10amt", "E_RETIRED_NUMERIC_SUFFIX"),
            ("0x10qty", "E_RETIRED_NUMERIC_SUFFIX"),
        ] {
            let source = SourceFile::new(SourceId(0), "invalid-decimal.ko", spelling);
            let lexed = lex(&source, FrontendBudget::v1());
            assert_eq!(lexed.diagnostics[0].code, code, "`{spelling}`");
            assert_eq!(lexed.tokens[0].kind, SyntaxKind::ErrorToken, "`{spelling}`");
        }
    }
    #[test]
    fn amount_and_quantity_suffixes_offer_unsuffixed_literal_fixes() {
        for (spelling, replacement) in [
            ("1amt", "1"),
            ("1.25amt", "1.25"),
            ("1qty", "1"),
            ("1.25qty", "1.25"),
        ] {
            let source = SourceFile::new(SourceId(0), "retired-suffix.ko", spelling);
            let lexed = lex(&source, FrontendBudget::v1());
            assert_eq!(lexed.diagnostics[0].code, "E_RETIRED_NUMERIC_SUFFIX");
            let fix = lexed.diagnostics[0]
                .fix
                .as_ref()
                .expect("amt and qty suffixes have a safe removal fix");
            assert_eq!(fix.span.byte_range, Some(lexed.tokens[0].range));
            assert_eq!(fix.replacement, replacement);
        }
        for spelling in ["1i64", "1u128"] {
            let source = SourceFile::new(SourceId(0), "retired-suffix.ko", spelling);
            let lexed = lex(&source, FrontendBudget::v1());
            assert_eq!(lexed.diagnostics[0].code, "E_RETIRED_NUMERIC_SUFFIX");
            assert!(lexed.diagnostics[0].fix.is_none());
        }
    }
}
