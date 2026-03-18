//! Tokenizer for the Kotodama language.
//!
//! The lexer converts a source string into a sequence of [`Token`]s.

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum TokenKind {
    Fn,
    Let,
    Return,
    Break,
    Continue,
    State,
    Struct,
    Permission,
    /// Sugar keyword for explicit function call statements: `call name(args...)`.
    Call,
    Meta,
    If,
    Else,
    While,
    For,
    /// Contract keyword ("seiyaku" or "誓約")
    Seiyaku,
    /// Contract initializer ("hajimari" or "始まり")
    Hajimari,
    /// Contract upgrade hook ("kaizen" or "改善")
    Kaizen,
    /// Public function modifier ("kotoage" or "言挙げ")
    Kotoage,
    True,
    False,
    Arrow,
    This,
    Ident(String),
    Number(u64),
    Decimal(String),
    String(String),
    Bytes(Vec<u8>),
    Plus,
    PlusPlus,
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
    Percent,
    Dot,
    Ampersand,
    LBracket,
    RBracket,
    Pipe,
    Question,
    Hash,
    EOF,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

/// Lex an entire source string into a vector of [`Token`]s.
pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let mut lexer = Lexer::new(src);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token()?;
        let end = matches!(tok.kind, TokenKind::EOF);
        tokens.push(tok);
        if end {
            break;
        }
    }
    Ok(tokens)
}

struct Lexer<'a> {
    src: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            _marker: std::marker::PhantomData,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src.get(self.pos).copied()
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.src.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.src.get(self.pos).copied();
        if let Some(c) = ch {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }

    fn next_token(&mut self) -> Result<Token, String> {
        self.skip_ws_and_comments()?;
        let line = self.line;
        let col = self.col;
        let ch = match self.peek() {
            Some(c) => c,
            None => {
                return Ok(Token {
                    kind: TokenKind::EOF,
                    line,
                    column: col,
                });
            }
        };
        if matches!(ch, 'r' | 'b')
            && let Some(tok) = self.lex_prefixed_string()?
        {
            return Ok(tok);
        }
        match ch {
            c if c.is_alphabetic() || c == '_' => self.lex_ident_or_keyword(),
            c if c.is_ascii_digit() => self.lex_number(),
            '"' => self.lex_string(),
            '+' => {
                self.bump();
                if self.peek() == Some('+') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::PlusPlus,
                        line,
                        column: col,
                    })
                } else if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::PlusEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Plus,
                        line,
                        column: col,
                    })
                }
            }
            '-' => {
                self.bump();
                if self.peek() == Some('>') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::Arrow,
                        line,
                        column: col,
                    })
                } else if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::MinusEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Minus,
                        line,
                        column: col,
                    })
                }
            }
            '*' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::StarEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Star,
                        line,
                        column: col,
                    })
                }
            }
            '/' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::SlashEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Slash,
                        line,
                        column: col,
                    })
                }
            }
            '=' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::EqualEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Equal,
                        line,
                        column: col,
                    })
                }
            }
            '!' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::BangEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Bang,
                        line,
                        column: col,
                    })
                }
            }
            '<' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::LessEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Less,
                        line,
                        column: col,
                    })
                }
            }
            '>' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::GreaterEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Greater,
                        line,
                        column: col,
                    })
                }
            }
            '&' => {
                self.bump();
                if self.peek() == Some('&') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::AndAnd,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Ampersand,
                        line,
                        column: col,
                    })
                }
            }
            '(' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::LParen,
                    line,
                    column: col,
                })
            }
            ')' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::RParen,
                    line,
                    column: col,
                })
            }
            '{' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::LBrace,
                    line,
                    column: col,
                })
            }
            '}' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::RBrace,
                    line,
                    column: col,
                })
            }
            ';' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::Semicolon,
                    line,
                    column: col,
                })
            }
            ',' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::Comma,
                    line,
                    column: col,
                })
            }
            ':' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::Colon,
                    line,
                    column: col,
                })
            }
            '%' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::PercentEqual,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Percent,
                        line,
                        column: col,
                    })
                }
            }
            '.' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::Dot,
                    line,
                    column: col,
                })
            }
            '[' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::LBracket,
                    line,
                    column: col,
                })
            }
            ']' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::RBracket,
                    line,
                    column: col,
                })
            }
            '#' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::Hash,
                    line,
                    column: col,
                })
            }
            '|' => {
                self.bump();
                if self.peek() == Some('|') {
                    self.bump();
                    Ok(Token {
                        kind: TokenKind::OrOr,
                        line,
                        column: col,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Pipe,
                        line,
                        column: col,
                    })
                }
            }
            '?' => {
                self.bump();
                Ok(Token {
                    kind: TokenKind::Question,
                    line,
                    column: col,
                })
            }
            _ => Err(format!("Unexpected character '{ch}' at {line}:{col}")),
        }
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), String> {
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\r' | '\n') => {
                    self.bump();
                }
                Some('/') if self.src.get(self.pos + 1) == Some(&'/') => {
                    while let Some(c) = self.bump() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some('/') if self.src.get(self.pos + 1) == Some(&'*') => {
                    // Block comments: /* ... */ (no nesting)
                    let start_line = self.line;
                    let start_col = self.col;
                    self.bump(); // '/'
                    self.bump(); // '*'
                    loop {
                        match self.bump() {
                            Some('*') if self.peek() == Some('/') => {
                                self.bump(); // consume '/'
                                break;
                            }
                            Some(_) => {}
                            None => {
                                return Err(format!(
                                    "unterminated block comment starting at {start_line}:{start_col}"
                                ));
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn lex_ident_or_keyword(&mut self) -> Result<Token, String> {
        let line = self.line;
        let col = self.col;
        let mut ident = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                ident.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let kind = match ident.as_str() {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "state" => TokenKind::State,
            "struct" => TokenKind::Struct,
            "permission" => TokenKind::Permission,
            "call" => TokenKind::Call,
            "meta" => TokenKind::Meta,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "seiyaku" | "誓約" => TokenKind::Seiyaku,
            "hajimari" | "始まり" => TokenKind::Hajimari,
            "kaizen" | "改善" => TokenKind::Kaizen,
            "kotoage" | "言挙げ" => TokenKind::Kotoage,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "this" => TokenKind::This,
            _ => TokenKind::Ident(ident),
        };
        Ok(Token {
            kind,
            line,
            column: col,
        })
    }

    fn lex_number(&mut self) -> Result<Token, String> {
        let line = self.line;
        let col = self.col;
        // Support 0x.. (hex), 0b.. (binary), and underscores in literals
        let mut num = 0u64;
        // Lookahead for 0x/0b prefixes
        if self.peek() == Some('0') {
            if self.src.get(self.pos + 1) == Some(&'x') || self.src.get(self.pos + 1) == Some(&'X')
            {
                // consume 0x
                self.bump();
                self.bump();
                let base = 16u64;
                let mut saw_digit = false;
                while let Some(c) = self.peek() {
                    let v = match c {
                        '0'..='9' => Some((c as u8 - b'0') as u64),
                        'a'..='f' => Some((c as u8 - b'a' + 10) as u64),
                        'A'..='F' => Some((c as u8 - b'A' + 10) as u64),
                        '_' => {
                            self.bump();
                            continue;
                        }
                        _ => None,
                    };
                    if let Some(d) = v {
                        saw_digit = true;
                        num = num
                            .checked_mul(base)
                            .and_then(|n| n.checked_add(d))
                            .ok_or_else(|| format!("numeric literal overflow at {line}:{col}"))?;
                        self.bump();
                    } else {
                        break;
                    }
                }
                if !saw_digit {
                    return Err(format!(
                        "expected hexadecimal digits after 0x at {line}:{col}"
                    ));
                }
                return Ok(Token {
                    kind: TokenKind::Number(num),
                    line,
                    column: col,
                });
            } else if self.src.get(self.pos + 1) == Some(&'b')
                || self.src.get(self.pos + 1) == Some(&'B')
            {
                // consume 0b
                self.bump();
                self.bump();
                let base = 2u64;
                let mut saw_digit = false;
                while let Some(c) = self.peek() {
                    match c {
                        '0' | '1' => {
                            saw_digit = true;
                            let bit = if c == '1' { 1u64 } else { 0u64 };
                            num = num
                                .checked_mul(base)
                                .and_then(|n| n.checked_add(bit))
                                .ok_or_else(|| {
                                    format!("numeric literal overflow at {line}:{col}")
                                })?;
                            self.bump();
                        }
                        '_' => {
                            self.bump();
                        }
                        _ => break,
                    }
                }
                if !saw_digit {
                    return Err(format!("expected binary digits after 0b at {line}:{col}"));
                }
                return Ok(Token {
                    kind: TokenKind::Number(num),
                    line,
                    column: col,
                });
            }
        }
        // Decimal with optional underscores, plus optional fractional part.
        let mut saw_digit = false;
        let mut digits = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                saw_digit = true;
                digits.push(c);
                self.bump();
            } else if c == '_' {
                self.bump();
            } else {
                break;
            }
        }
        if !saw_digit {
            return Err(format!("expected number at {line}:{col}"));
        }
        if self.peek() == Some('.') && self.peek_n(1).is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
            let mut frac_digits = String::new();
            let mut saw_frac = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    saw_frac = true;
                    frac_digits.push(c);
                    self.bump();
                } else if c == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
            if !saw_frac {
                return Err(format!("expected fractional digits at {line}:{col}"));
            }
            let mut raw = digits;
            raw.push('.');
            raw.push_str(&frac_digits);
            return Ok(Token {
                kind: TokenKind::Decimal(raw),
                line,
                column: col,
            });
        }
        let num = digits
            .parse::<u64>()
            .map_err(|_| format!("numeric literal overflow at {line}:{col}"))?;
        Ok(Token {
            kind: TokenKind::Number(num),
            line,
            column: col,
        })
    }

    fn lex_prefixed_string(&mut self) -> Result<Option<Token>, String> {
        match self.peek() {
            Some('r') => {
                if self.peek_n(1) == Some('b') && matches!(self.peek_n(2), Some('"' | '#')) {
                    return self
                        .lex_raw_string(true, 2)
                        .map(Some)
                        .map_err(|e| e.to_string());
                }
                if matches!(self.peek_n(1), Some('"' | '#')) {
                    return self
                        .lex_raw_string(false, 1)
                        .map(Some)
                        .map_err(|e| e.to_string());
                }
            }
            Some('b') => {
                if self.peek_n(1) == Some('r') && matches!(self.peek_n(2), Some('"' | '#')) {
                    return self
                        .lex_raw_string(true, 2)
                        .map(Some)
                        .map_err(|e| e.to_string());
                }
                if self.peek_n(1) == Some('"') {
                    return self.lex_byte_string().map(Some).map_err(|e| e.to_string());
                }
            }
            _ => {}
        }
        // Not a prefixed literal; return None so caller can lex identifier.
        Ok(None)
    }

    fn lex_raw_string(&mut self, is_bytes: bool, prefix_len: usize) -> Result<Token, String> {
        let line = self.line;
        let col = self.col;
        for _ in 0..prefix_len {
            self.bump();
        }
        let mut hashes = 0usize;
        while self.peek() == Some('#') {
            self.bump();
            hashes += 1;
        }
        if self.peek() != Some('"') {
            return Err(format!(
                "expected '\"' after raw string prefix at {line}:{col}"
            ));
        }
        self.bump(); // opening quote
        let mut out = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(format!(
                        "unterminated raw string literal at {line}:{col}: missing closing delimiter"
                    ));
                }
                Some('"') => {
                    if self.is_raw_terminator(hashes) {
                        self.bump(); // closing quote
                        for _ in 0..hashes {
                            self.bump();
                        }
                        break;
                    }
                    out.push('"');
                    self.bump();
                }
                Some(c) => {
                    out.push(c);
                    self.bump();
                }
            }
        }
        let kind = if is_bytes {
            TokenKind::Bytes(out.into_bytes())
        } else {
            TokenKind::String(out)
        };
        Ok(Token {
            kind,
            line,
            column: col,
        })
    }

    fn is_raw_terminator(&self, hashes: usize) -> bool {
        if hashes == 0 {
            return true;
        }
        for idx in 1..=hashes {
            if self.peek_n(idx) != Some('#') {
                return false;
            }
        }
        true
    }

    fn lex_byte_string(&mut self) -> Result<Token, String> {
        let line = self.line;
        let col = self.col;
        self.bump(); // 'b'
        self.bump(); // opening quote
        let mut bytes = Vec::new();
        let mut terminated = false;
        while let Some(c) = self.peek() {
            match c {
                '"' => {
                    self.bump();
                    terminated = true;
                    break;
                }
                '\n' => {
                    return Err(format!(
                        "unterminated byte string literal at {line}:{col}: newline before closing quote"
                    ));
                }
                '\\' => {
                    self.bump(); // consume '\\'
                    let escaped = self.read_escape_bytes(line, col)?;
                    bytes.extend_from_slice(&escaped);
                }
                ch => {
                    bytes.extend_from_slice(ch.encode_utf8(&mut [0u8; 4]).as_bytes());
                    self.bump();
                }
            }
        }
        if !terminated {
            return Err(format!(
                "unterminated byte string literal at {line}:{col}: missing closing quote"
            ));
        }
        Ok(Token {
            kind: TokenKind::Bytes(bytes),
            line,
            column: col,
        })
    }

    fn lex_string(&mut self) -> Result<Token, String> {
        let line = self.line;
        let col = self.col;
        self.bump(); // opening quote
        let mut s = String::new();
        let mut terminated = false;
        while let Some(c) = self.peek() {
            match c {
                '"' => {
                    self.bump();
                    terminated = true;
                    break;
                }
                '\n' => {
                    return Err(format!(
                        "unterminated string literal at {line}:{col}: newline before closing quote"
                    ));
                }
                '\\' => {
                    self.bump(); // consume '\\'
                    let escaped = self.read_escape_char(line, col)?;
                    s.push(escaped);
                }
                ch => {
                    s.push(ch);
                    self.bump();
                }
            }
        }
        if !terminated {
            return Err(format!(
                "unterminated string literal at {line}:{col}: missing closing quote"
            ));
        }
        Ok(Token {
            kind: TokenKind::String(s),
            line,
            column: col,
        })
    }

    fn read_escape_char(&mut self, line: usize, col: usize) -> Result<char, String> {
        let Some(esc) = self.bump() else {
            return Err(format!("unterminated escape at {line}:{col}"));
        };
        match esc {
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            '0' => Ok('\0'),
            '"' => Ok('"'),
            '\\' => Ok('\\'),
            'x' => {
                let byte = self.read_hex_escape(line, col, 2)?;
                Ok(byte as char)
            }
            'u' => self.read_unicode_escape(line, col),
            other => Err(format!("unknown escape \\{other} at {line}:{col}")),
        }
    }

    fn read_escape_bytes(&mut self, line: usize, col: usize) -> Result<Vec<u8>, String> {
        let Some(esc) = self.bump() else {
            return Err(format!("unterminated escape at {line}:{col}"));
        };
        match esc {
            'n' => Ok(vec![b'\n']),
            'r' => Ok(vec![b'\r']),
            't' => Ok(vec![b'\t']),
            '0' => Ok(vec![0]),
            '"' => Ok(vec![b'"']),
            '\\' => Ok(vec![b'\\']),
            'x' => Ok(vec![self.read_hex_escape(line, col, 2)?]),
            'u' => {
                let ch = self.read_unicode_escape(line, col)?;
                let mut buf = [0u8; 4];
                let encoded = ch.encode_utf8(&mut buf);
                Ok(encoded.as_bytes().to_vec())
            }
            other => Err(format!("unknown escape \\{other} at {line}:{col}")),
        }
    }

    fn read_hex_escape(&mut self, line: usize, col: usize, digits: usize) -> Result<u8, String> {
        let mut value: u32 = 0;
        for _ in 0..digits {
            let Some(c) = self.bump() else {
                return Err(format!("incomplete hex escape at {line}:{col}"));
            };
            let digit = c
                .to_digit(16)
                .ok_or_else(|| format!("invalid hex digit '{c}' in escape at {line}:{col}"))?;
            value = value
                .checked_mul(16)
                .and_then(|v| v.checked_add(digit))
                .ok_or_else(|| format!("hex escape overflow at {line}:{col}"))?;
        }
        Ok(value as u8)
    }

    fn read_unicode_escape(&mut self, line: usize, col: usize) -> Result<char, String> {
        let Some(open) = self.bump() else {
            return Err(format!("incomplete unicode escape at {line}:{col}"));
        };
        if open != '{' {
            return Err(format!(
                "unicode escape at {line}:{col} must start with '{{'"
            ));
        }
        let mut value: u32 = 0;
        let mut digits = 0usize;
        loop {
            let Some(c) = self.peek() else {
                return Err(format!("unterminated unicode escape at {line}:{col}"));
            };
            if c == '}' {
                self.bump();
                break;
            }
            let digit = c.to_digit(16).ok_or_else(|| {
                format!("invalid hex digit '{c}' in unicode escape at {line}:{col}")
            })?;
            value = value
                .checked_mul(16)
                .and_then(|v| v.checked_add(digit))
                .ok_or_else(|| format!("unicode escape overflow at {line}:{col}"))?;
            digits += 1;
            if digits > 6 {
                return Err(format!("unicode escape too long at {line}:{col}"));
            }
            self.bump();
        }
        if digits == 0 {
            return Err(format!("empty unicode escape at {line}:{col}"));
        }
        if value > 0x10FFFF || (0xD800..=0xDFFF).contains(&value) {
            return Err(format!("invalid unicode scalar value at {line}:{col}"));
        }
        char::from_u32(value).ok_or_else(|| format!("invalid unicode scalar value at {line}:{col}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{TokenKind, lex};

    #[test]
    fn decimal_literal_overflow_is_reported() {
        let err = lex("18446744073709551616").unwrap_err();
        assert!(err.contains("overflow"));
    }

    #[test]
    fn decimal_literal_max_plus_one_is_tokenized() {
        let tokens = lex("9223372036854775808").expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::Number(n) if n == 9_223_372_036_854_775_808),
            "expected u64 literal token, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn decimal_fraction_literal_is_tokenized() {
        let tokens = lex("1_234.50_0").expect("lex");
        assert!(
            matches!(tokens[0].kind, TokenKind::Decimal(ref s) if s == "1234.500"),
            "expected decimal literal token, got {:?}",
            tokens[0].kind
        );
    }

    #[test]
    fn hex_literal_overflow_is_reported() {
        let err = lex("0x1_0000_0000_0000_0000").unwrap_err();
        assert!(err.contains("overflow"));
    }

    #[test]
    fn binary_literal_overflow_is_reported() {
        let err = lex(
            "0b1_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000",
        )
        .unwrap_err();
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
        assert!(err.contains("newline"));
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
        assert!(err.contains("empty unicode escape"));
    }
}
