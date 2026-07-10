//! Lossless, recovering Kotodama lexer.

use crate::{
    diagnostic::{Diagnostic, DiagnosticPhase, SourcePosition, SourceSpan},
    lexer::{TokenKind, v1_keyword_kind},
    source::{FrontendBudget, SourceFile, TextRange},
};

use super::{cst::GreenToken, kind::SyntaxKind};

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
}

impl<'source> Scanner<'source> {
    fn new(source: &'source SourceFile) -> Self {
        Self {
            text: source.text(),
            pos: 0,
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

    /// Scan an integer-looking token and report whether it contains a decimal
    /// fraction, which is invalid in V1.
    fn scan_number(&mut self) -> bool {
        if self.starts_with("0x") || self.starts_with("0X") {
            self.pos += 2;
            while self
                .current()
                .is_some_and(|character| character.is_ascii_hexdigit() || character == '_')
            {
                self.bump();
            }
            return false;
        }
        if self.starts_with("0b") || self.starts_with("0B") {
            self.pos += 2;
            while self
                .current()
                .is_some_and(|character| matches!(character, '0' | '1' | '_'))
            {
                self.bump();
            }
            return false;
        }
        while self
            .current()
            .is_some_and(|character| character.is_ascii_digit() || character == '_')
        {
            self.bump();
        }
        if self.starts_with(".")
            && self.rest()[1..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        {
            self.bump();
            while self
                .current()
                .is_some_and(|character| character.is_ascii_digit() || character == '_')
            {
                self.bump();
            }
            true
        } else {
            false
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
        let is_bytes = if bytes.get(cursor) == Some(&b'b') && bytes.get(cursor + 1) == Some(&b'r') {
            cursor += 2;
            true
        } else if bytes.get(cursor) == Some(&b'r') && bytes.get(cursor + 1) == Some(&b'b') {
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
        for (spelling, kind) in [
            ("+=", SyntaxKind::PlusEqual),
            ("->", SyntaxKind::Arrow),
            ("-=", SyntaxKind::MinusEqual),
            ("*=", SyntaxKind::StarEqual),
            ("/=", SyntaxKind::SlashEqual),
            ("%=", SyntaxKind::PercentEqual),
            ("!=", SyntaxKind::BangEqual),
            ("==", SyntaxKind::EqualEqual),
            ("<=", SyntaxKind::LessEqual),
            (">=", SyntaxKind::GreaterEqual),
            ("&&", SyntaxKind::AndAnd),
            ("||", SyntaxKind::OrOr),
            ("::", SyntaxKind::ColonColon),
        ] {
            if self.starts_with(spelling) {
                self.pos += spelling.len();
                return Some(kind);
            }
        }
        let kind = match self.bump()? {
            '+' => SyntaxKind::Plus,
            '-' => SyntaxKind::Minus,
            '*' => SyntaxKind::Star,
            '/' => SyntaxKind::Slash,
            '%' => SyntaxKind::Percent,
            '!' => SyntaxKind::Bang,
            '=' => SyntaxKind::Equal,
            '<' => SyntaxKind::Less,
            '>' => SyntaxKind::Greater,
            '(' => SyntaxKind::LParen,
            ')' => SyntaxKind::RParen,
            '{' => SyntaxKind::LBrace,
            '}' => SyntaxKind::RBrace,
            '[' => SyntaxKind::LBracket,
            ']' => SyntaxKind::RBracket,
            ';' => SyntaxKind::Semicolon,
            ',' => SyntaxKind::Comma,
            ':' => SyntaxKind::Colon,
            '.' => SyntaxKind::Dot,
            '?' => SyntaxKind::Question,
            '#' => SyntaxKind::Hash,
            _ => return None,
        };
        Some(kind)
    }

    fn next_token(&mut self) -> (GreenToken, Option<&'static str>) {
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
                (!terminated).then_some("unterminated block comment"),
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
                (!terminated).then_some("unterminated raw string literal"),
            )
        } else if self.starts_with("b\"") {
            let terminated = self.scan_quoted(1);
            (
                if terminated {
                    SyntaxKind::Bytes
                } else {
                    SyntaxKind::ErrorToken
                },
                (!terminated).then_some("unterminated byte string literal"),
            )
        } else if character == '"' {
            let terminated = self.scan_quoted(0);
            (
                if terminated {
                    SyntaxKind::String
                } else {
                    SyntaxKind::ErrorToken
                },
                (!terminated).then_some("unterminated string literal"),
            )
        } else if character.is_alphabetic() || character == '_' {
            self.scan_identifier();
            let text = &self.text[start..self.pos];
            let kind = keyword_kind(text);
            if kind == SyntaxKind::Ident && !text.is_ascii() {
                (
                    SyntaxKind::ErrorToken,
                    Some("non-ASCII identifier outside the branded Japanese keyword set"),
                )
            } else {
                (kind, None)
            }
        } else if character.is_ascii_digit() {
            if self.scan_number() {
                (
                    SyntaxKind::ErrorToken,
                    Some("decimal fractions are not part of Kotodama V1"),
                )
            } else {
                (SyntaxKind::Number, None)
            }
        } else if self.starts_with("++") {
            self.pos += 2;
            (SyntaxKind::ErrorToken, Some("invalid Kotodama V1 operator"))
        } else if let Some(kind) = self.punctuation() {
            (kind, None)
        } else {
            // `punctuation` consumed exactly one Unicode scalar before
            // reporting failure.
            (
                SyntaxKind::ErrorToken,
                Some(if character.is_ascii() {
                    "invalid source character"
                } else {
                    "non-ASCII character outside a string or comment"
                }),
            )
        };
        let end = self.pos;
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
        if let Some(message) = lexical_error {
            record_diagnostic(
                &mut diagnostics,
                &mut omitted_diagnostics,
                budget,
                diagnostic(source, "K0100", message, token.range),
            );
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
}
