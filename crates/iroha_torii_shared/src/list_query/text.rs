//! Human text form of filter expressions and sort specifications.
//!
//! ```text
//! filter     := or
//! or         := and ("or" and)*
//! and        := unary ("and" unary)*
//! unary      := "not" unary | primary
//! primary    := "(" filter ")" | "exists" "(" path ")" | path predicate
//! predicate  := compare literal | ["not"] "in" list | "is" ["not"] "null"
//! compare    := "=" | "==" | "!=" | "<>" | "<" | "<=" | ">" | ">="
//! list       := "[" literal ("," literal)* [","] "]" | "(" literal ("," literal)* [","] ")"
//! literal    := string | number | "true" | "false" | "null"
//! path       := segment ("." segment)*
//! segment    := [A-Za-z_][A-Za-z0-9_]* | "`" any character except "`" "`"
//! string     := '"' JSON escapes '"' | "'" JSON escapes or \' "'"
//! number     := "-"? ("0" | [1-9][0-9]*) ("." [0-9]+)?
//!
//! sort       := key ("," key)*
//! key        := ["-"] path
//! ```
//!
//! Keywords are case-insensitive. `not` binds tighter than `and`, which binds
//! tighter than `or`. Integers that fit `u64`/`i64` become JSON numbers;
//! decimals and wider integers become exact decimal strings.
use super::{
    filter::{FieldPath, FilterError, FilterExpr},
    sort::{Order, SortKey},
};
use norito::json::Value;
use std::{fmt, str::FromStr};

/// Maximum accepted length of a text filter.
pub const FILTER_TEXT_MAX_BYTES: usize = 32 * 1024;
/// Maximum number of keys in one sort specification.
pub const SORT_MAX_KEYS: usize = 8;
/// Maximum syntactic nesting of parentheses and `not` while parsing.
const PARSE_MAX_NESTING: usize = 64;
const KEYWORDS: [&str; 9] = [
    "and", "or", "not", "in", "is", "null", "true", "false", "exists",
];

/// Syntax error in a text filter or sort specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterSyntaxError {
    /// Byte offset of the offending token.
    pub offset: usize,
    /// 1-based line of the offending token.
    pub line: usize,
    /// 1-based column (in characters) of the offending token.
    pub column: usize,
    /// Human-readable description including a fix where one is obvious.
    pub message: String,
    multiline: bool,
}

impl FilterSyntaxError {
    pub(super) fn at(input: &str, offset: usize, message: impl Into<String>) -> Self {
        let mut offset = offset.min(input.len());
        while !input.is_char_boundary(offset) {
            offset -= 1;
        }
        let before = &input[..offset];
        let line = before.matches('\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        let column = input[line_start..offset].chars().count() + 1;
        Self {
            offset,
            line,
            column,
            message: message.into(),
            multiline: input.contains('\n'),
        }
    }

    fn structure(err: &FilterError) -> Self {
        Self {
            offset: 0,
            line: 1,
            column: 1,
            message: err.to_string(),
            multiline: false,
        }
    }
}

impl fmt::Display for FilterSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.multiline {
            write!(
                f,
                "{} (line {}, column {})",
                self.message, self.line, self.column
            )
        } else {
            write!(f, "{} (column {})", self.message, self.column)
        }
    }
}

impl std::error::Error for FilterSyntaxError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compare {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Word(String),
    Quoted(String),
    Str(String),
    Number(String),
    Compare(Compare),
    Minus,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    End,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    start: usize,
}

impl Kind {
    fn describe(&self) -> String {
        match self {
            Self::Word(word) => format!("`{word}`"),
            Self::Quoted(segment) => format!("`{segment}`"),
            Self::Str(_) => "a string literal".to_owned(),
            Self::Number(number) => format!("the number `{number}`"),
            Self::Compare(_) => "a comparison operator".to_owned(),
            Self::Minus => "`-`".to_owned(),
            Self::LParen => "`(`".to_owned(),
            Self::RParen => "`)`".to_owned(),
            Self::LBracket => "`[`".to_owned(),
            Self::RBracket => "`]`".to_owned(),
            Self::Comma => "`,`".to_owned(),
            Self::Dot => "`.`".to_owned(),
            Self::End => "the end of the input".to_owned(),
        }
    }

    fn keyword(&self) -> Option<&'static str> {
        let Self::Word(word) = self else {
            return None;
        };
        KEYWORDS
            .iter()
            .copied()
            .find(|keyword| word.eq_ignore_ascii_case(keyword))
    }

    fn is_keyword(&self, keyword: &str) -> bool {
        matches!(self, Self::Word(word) if word.eq_ignore_ascii_case(keyword))
    }
}

struct Lexer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    position: usize,
    /// Sort specifications lex `-` as a descending marker.
    allow_minus: bool,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str, allow_minus: bool) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            position: 0,
            allow_minus,
        }
    }

    fn error(&self, offset: usize, message: impl Into<String>) -> FilterSyntaxError {
        FilterSyntaxError::at(self.input, offset, message)
    }

    fn tokens(mut self) -> Result<Vec<Token>, FilterSyntaxError> {
        let mut out = Vec::new();
        loop {
            let token = self.next_token()?;
            let end = token.kind == Kind::End;
            out.push(token);
            if end {
                return Ok(out);
            }
        }
    }

    fn peek_byte(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.position + ahead).copied()
    }

    #[allow(clippy::too_many_lines)]
    fn next_token(&mut self) -> Result<Token, FilterSyntaxError> {
        while let Some(byte) = self.peek_byte(0) {
            if matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
                self.position += 1;
            } else {
                break;
            }
        }
        let start = self.position;
        let Some(byte) = self.peek_byte(0) else {
            return Ok(Token {
                kind: Kind::End,
                start,
            });
        };
        let single = |kind: Kind, lexer: &mut Self| {
            lexer.position += 1;
            Ok(Token { kind, start })
        };
        match byte {
            b'(' => single(Kind::LParen, self),
            b')' => single(Kind::RParen, self),
            b'[' => single(Kind::LBracket, self),
            b']' => single(Kind::RBracket, self),
            b',' => single(Kind::Comma, self),
            b'.' => {
                if self.peek_byte(1).is_some_and(|next| next.is_ascii_digit()) {
                    return Err(
                        self.error(start, "decimal literals need a leading digit, e.g. `0.5`")
                    );
                }
                single(Kind::Dot, self)
            }
            b'=' => {
                self.position += if self.peek_byte(1) == Some(b'=') {
                    2
                } else {
                    1
                };
                Ok(Token {
                    kind: Kind::Compare(Compare::Eq),
                    start,
                })
            }
            b'!' => {
                if self.peek_byte(1) == Some(b'=') {
                    self.position += 2;
                    Ok(Token {
                        kind: Kind::Compare(Compare::Ne),
                        start,
                    })
                } else {
                    Err(self.error(start, "use the keyword `not` instead of `!`"))
                }
            }
            b'<' => {
                let (kind, width) = match self.peek_byte(1) {
                    Some(b'=') => (Compare::Lte, 2),
                    Some(b'>') => (Compare::Ne, 2),
                    _ => (Compare::Lt, 1),
                };
                self.position += width;
                Ok(Token {
                    kind: Kind::Compare(kind),
                    start,
                })
            }
            b'>' => {
                let (kind, width) = if self.peek_byte(1) == Some(b'=') {
                    (Compare::Gte, 2)
                } else {
                    (Compare::Gt, 1)
                };
                self.position += width;
                Ok(Token {
                    kind: Kind::Compare(kind),
                    start,
                })
            }
            b'&' => Err(self.error(start, "use the keyword `and` instead of `&` or `&&`")),
            b'|' => Err(self.error(start, "use the keyword `or` instead of `|` or `||`")),
            b'"' | b'\'' => self.string(byte),
            b'`' => self.quoted_segment(),
            b'-' => {
                if self.peek_byte(1).is_some_and(|next| next.is_ascii_digit()) {
                    self.number()
                } else if self.allow_minus {
                    single(Kind::Minus, self)
                } else {
                    Err(self.error(
                        start,
                        "unexpected `-`; quote field names that contain `-` with backticks, e.g. `display-name`",
                    ))
                }
            }
            b'0'..=b'9' => self.number(),
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                let mut end = start + 1;
                while self
                    .bytes
                    .get(end)
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == b'_')
                {
                    end += 1;
                }
                self.position = end;
                if self.peek_byte(0) == Some(b'-')
                    && self
                        .peek_byte(1)
                        .is_some_and(|next| next.is_ascii_alphabetic())
                {
                    let mut word_end = end;
                    while self.bytes.get(word_end).is_some_and(|next| {
                        next.is_ascii_alphanumeric() || matches!(next, b'_' | b'-')
                    }) {
                        word_end += 1;
                    }
                    return Err(self.error(
                        start,
                        format!(
                            "wrap field names containing `-` in backticks, e.g. `{}`",
                            &self.input[start..word_end]
                        ),
                    ));
                }
                Ok(Token {
                    kind: Kind::Word(self.input[start..end].to_owned()),
                    start,
                })
            }
            b':' if self.allow_minus => Err(self.error(
                start,
                "unexpected `:`; write `field` for ascending and `-field` for descending order",
            )),
            b':' => Err(self.error(
                start,
                "unexpected `:`; compare values with `=`, e.g. `status = \"active\"`",
            )),
            _ => {
                let ch = self.input[start..].chars().next().unwrap_or('?');
                Err(self.error(start, format!("unexpected character `{ch}`")))
            }
        }
    }

    fn number(&mut self) -> Result<Token, FilterSyntaxError> {
        let start = self.position;
        let mut end = start;
        if self.bytes.get(end) == Some(&b'-') {
            end += 1;
        }
        let integer_start = end;
        while self.bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end - integer_start > 1 && self.bytes[integer_start] == b'0' {
            return Err(self.error(start, "numbers must not have leading zeros"));
        }
        if self.bytes.get(end) == Some(&b'.') {
            end += 1;
            let fraction_start = end;
            while self.bytes.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
            if end == fraction_start {
                return Err(self.error(start, "decimal literals need digits after `.`"));
            }
        }
        match self.bytes.get(end) {
            Some(b'e' | b'E') => {
                return Err(self.error(
                    start,
                    "exponent notation is not supported; write the full decimal value",
                ));
            }
            Some(next) if next.is_ascii_alphabetic() || *next == b'_' => {
                return Err(self.error(
                    start,
                    "a number cannot be followed directly by letters; quote text values",
                ));
            }
            _ => {}
        }
        self.position = end;
        Ok(Token {
            kind: Kind::Number(self.input[start..end].to_owned()),
            start,
        })
    }

    fn string(&mut self, quote: u8) -> Result<Token, FilterSyntaxError> {
        let start = self.position;
        let mut out = String::new();
        let mut chars = self.input[start + 1..].char_indices();
        loop {
            let Some((index, ch)) = chars.next() else {
                return Err(self.error(start, "unterminated string literal"));
            };
            let absolute = start + 1 + index;
            match ch {
                ch if ch as u32 == u32::from(quote) => {
                    self.position = absolute + 1;
                    return Ok(Token {
                        kind: Kind::Str(out),
                        start,
                    });
                }
                '\\' => {
                    let Some((_, escaped)) = chars.next() else {
                        return Err(self.error(absolute, "unterminated escape sequence"));
                    };
                    match escaped {
                        '"' => out.push('"'),
                        '\'' => out.push('\''),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let decoded = self.unicode_escape(&mut chars, absolute)?;
                            out.push(decoded);
                        }
                        other => {
                            return Err(self
                                .error(absolute, format!("unknown escape sequence `\\{other}`")));
                        }
                    }
                }
                // As in JSON, only U+0000..U+001F must be escaped; DEL and C1
                // characters stay literal, so every rendered literal parses.
                ch if ch < '\u{20}' => {
                    return Err(self.error(
                        absolute,
                        "control characters must be escaped inside string literals",
                    ));
                }
                ch => out.push(ch),
            }
        }
    }

    fn unicode_escape(
        &self,
        chars: &mut std::str::CharIndices<'_>,
        at: usize,
    ) -> Result<char, FilterSyntaxError> {
        let read_unit = |chars: &mut std::str::CharIndices<'_>| -> Option<u32> {
            let mut value = 0u32;
            for _ in 0..4 {
                let (_, digit) = chars.next()?;
                value = value * 16 + digit.to_digit(16)?;
            }
            Some(value)
        };
        let invalid = || self.error(at, "invalid `\\u` escape; expected four hexadecimal digits");
        let first = read_unit(chars).ok_or_else(invalid)?;
        if (0xD800..0xDC00).contains(&first) {
            let backslash = chars.next().map(|(_, ch)| ch);
            let marker = chars.next().map(|(_, ch)| ch);
            if backslash != Some('\\') || marker != Some('u') {
                return Err(self.error(at, "unpaired UTF-16 surrogate in `\\u` escape"));
            }
            let second = read_unit(chars).ok_or_else(invalid)?;
            if !(0xDC00..0xE000).contains(&second) {
                return Err(self.error(at, "unpaired UTF-16 surrogate in `\\u` escape"));
            }
            let combined = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
            return char::from_u32(combined).ok_or_else(invalid);
        }
        char::from_u32(first)
            .ok_or_else(|| self.error(at, "unpaired UTF-16 surrogate in `\\u` escape"))
    }

    fn quoted_segment(&mut self) -> Result<Token, FilterSyntaxError> {
        let start = self.position;
        let Some(length) = self.input[start + 1..].find('`') else {
            return Err(self.error(start, "unterminated backtick-quoted field name"));
        };
        let segment = &self.input[start + 1..start + 1 + length];
        if segment.is_empty() {
            return Err(self.error(start, "backtick-quoted field names must not be empty"));
        }
        if segment.contains('.') {
            return Err(self.error(
                start,
                "a backtick-quoted segment must not contain `.`; quote each segment separately",
            ));
        }
        self.position = start + length + 2;
        Ok(Token {
            kind: Kind::Quoted(segment.to_owned()),
            start,
        })
    }
}

struct Parser<'a> {
    input: &'a str,
    tokens: Vec<Token>,
    position: usize,
    nesting: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, allow_minus: bool) -> Result<Self, FilterSyntaxError> {
        let tokens = Lexer::new(input, allow_minus).tokens()?;
        Ok(Self {
            input,
            tokens,
            position: 0,
            nesting: 0,
        })
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.position.min(self.tokens.len() - 1)]
    }

    fn peek_kind_at(&self, ahead: usize) -> &Kind {
        &self.tokens[(self.position + ahead).min(self.tokens.len() - 1)].kind
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        if token.kind != Kind::End {
            self.position += 1;
        }
        token
    }

    fn error_at(&self, token: &Token, message: impl Into<String>) -> FilterSyntaxError {
        FilterSyntaxError::at(self.input, token.start, message)
    }

    fn enter(&mut self, token: &Token) -> Result<(), FilterSyntaxError> {
        self.nesting += 1;
        if self.nesting > PARSE_MAX_NESTING {
            return Err(self.error_at(token, "filter nests too deeply"));
        }
        Ok(())
    }

    fn filter(&mut self) -> Result<FilterExpr, FilterSyntaxError> {
        let first = self.and()?;
        if !self.peek().kind.is_keyword("or") {
            return Ok(first);
        }
        let mut operands = vec![first];
        while self.peek().kind.is_keyword("or") {
            self.advance();
            operands.push(self.and()?);
        }
        Ok(FilterExpr::Or(operands))
    }

    fn and(&mut self) -> Result<FilterExpr, FilterSyntaxError> {
        let first = self.unary()?;
        if !self.peek().kind.is_keyword("and") {
            return Ok(first);
        }
        let mut operands = vec![first];
        while self.peek().kind.is_keyword("and") {
            self.advance();
            operands.push(self.unary()?);
        }
        Ok(FilterExpr::And(operands))
    }

    fn unary(&mut self) -> Result<FilterExpr, FilterSyntaxError> {
        if self.peek().kind.is_keyword("not") {
            let token = self.advance();
            self.enter(&token)?;
            let inner = self.unary()?;
            self.nesting -= 1;
            return Ok(FilterExpr::Not(Box::new(inner)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<FilterExpr, FilterSyntaxError> {
        let token = self.peek().clone();
        match &token.kind {
            Kind::LParen => {
                self.advance();
                self.enter(&token)?;
                let inner = self.filter()?;
                self.nesting -= 1;
                self.expect_close(&Kind::RParen, &token)?;
                Ok(inner)
            }
            Kind::Word(_)
                if token.kind.is_keyword("exists") && *self.peek_kind_at(1) == Kind::LParen =>
            {
                self.advance();
                let open = self.advance();
                let field = self.path()?;
                self.expect_close(&Kind::RParen, &open)?;
                Ok(FilterExpr::Exists(field))
            }
            Kind::Word(_) | Kind::Quoted(_) => {
                let field = self.path()?;
                self.predicate(field)
            }
            Kind::Str(_) | Kind::Number(_) => Err(self.error_at(
                &token,
                "expected a field name on the left-hand side, e.g. `quantity > 5`",
            )),
            Kind::End => Err(self.error_at(&token, "expected a filter expression")),
            other => Err(self.error_at(
                &token,
                format!("expected a field name, found {}", other.describe()),
            )),
        }
    }

    fn expect_close(&mut self, close: &Kind, open: &Token) -> Result<(), FilterSyntaxError> {
        let token = self.advance();
        if token.kind == *close {
            return Ok(());
        }
        let (symbol, opened) = if *close == Kind::RParen {
            ("`)`", "`(`")
        } else {
            ("`]`", "`[`")
        };
        let column = FilterSyntaxError::at(self.input, open.start, "").column;
        Err(self.error_at(
            &token,
            format!(
                "expected {symbol} to close the {opened} at column {column}, found {}",
                token.kind.describe()
            ),
        ))
    }

    fn path(&mut self) -> Result<FieldPath, FilterSyntaxError> {
        let first = self.advance();
        let mut path = match &first.kind {
            Kind::Word(word) => {
                if let Some(keyword) = first.kind.keyword() {
                    return Err(self.error_at(
                        &first,
                        format!(
                            "expected a field name, found the keyword `{keyword}`; quote a field with this name as `{word}` in backticks"
                        ),
                    ));
                }
                word.clone()
            }
            Kind::Quoted(segment) => segment.clone(),
            other => {
                return Err(self.error_at(
                    &first,
                    format!("expected a field name, found {}", other.describe()),
                ));
            }
        };
        while self.peek().kind == Kind::Dot {
            self.advance();
            let segment = self.advance();
            match segment.kind {
                Kind::Word(ref word) | Kind::Quoted(ref word) => {
                    path.push('.');
                    path.push_str(word);
                }
                ref other => {
                    return Err(self.error_at(
                        &segment,
                        format!(
                            "expected a field name after `.`, found {}",
                            other.describe()
                        ),
                    ));
                }
            }
        }
        let field = FieldPath(path);
        field
            .validate()
            .map_err(|err| self.error_at(&first, err.to_string()))?;
        Ok(field)
    }

    fn predicate(&mut self, field: FieldPath) -> Result<FilterExpr, FilterSyntaxError> {
        let token = self.advance();
        match token.kind {
            Kind::Compare(compare) => {
                let literal = self.literal()?;
                Ok(match compare {
                    Compare::Eq => FilterExpr::Eq(field, literal),
                    Compare::Ne => FilterExpr::Ne(field, literal),
                    Compare::Lt => FilterExpr::Lt(field, literal),
                    Compare::Lte => FilterExpr::Lte(field, literal),
                    Compare::Gt => FilterExpr::Gt(field, literal),
                    Compare::Gte => FilterExpr::Gte(field, literal),
                })
            }
            ref kind if kind.is_keyword("in") => Ok(FilterExpr::In(field, self.list()?)),
            ref kind if kind.is_keyword("not") => {
                let next = self.advance();
                if next.kind.is_keyword("in") {
                    Ok(FilterExpr::Nin(field, self.list()?))
                } else {
                    Err(self.error_at(&next, "expected `in` after `not` (as in `field not in [...]`)"))
                }
            }
            ref kind if kind.is_keyword("is") => {
                let next = self.advance();
                if next.kind.is_keyword("null") {
                    return Ok(FilterExpr::IsNull(field));
                }
                if next.kind.is_keyword("not") {
                    let null = self.advance();
                    if null.kind.is_keyword("null") {
                        return Ok(FilterExpr::Not(Box::new(FilterExpr::IsNull(field))));
                    }
                    return Err(self.error_at(&null, "expected `null` after `is not`"));
                }
                Err(self.error_at(
                    &next,
                    "expected `null` or `not null` after `is`; compare values with `=`",
                ))
            }
            Kind::End => Err(self.error_at(
                &token,
                format!("expected an operator after `{field}` (=, !=, <, <=, >, >=, in, not in, is null)"),
            )),
            ref other => Err(self.error_at(
                &token,
                format!(
                    "expected an operator after `{field}` (=, !=, <, <=, >, >=, in, not in, is null), found {}",
                    other.describe()
                ),
            )),
        }
    }

    fn list(&mut self) -> Result<Vec<Value>, FilterSyntaxError> {
        let open = self.advance();
        let close = match open.kind {
            Kind::LBracket => Kind::RBracket,
            Kind::LParen => Kind::RParen,
            ref other => {
                return Err(self.error_at(
                    &open,
                    format!(
                        "expected `[` to start a value list, found {}",
                        other.describe()
                    ),
                ));
            }
        };
        let mut values = Vec::new();
        loop {
            if self.peek().kind == close {
                if values.is_empty() {
                    return Err(self.error_at(self.peek(), "value lists must not be empty"));
                }
                self.advance();
                return Ok(values);
            }
            values.push(self.literal()?);
            let separator = self.peek().clone();
            match separator.kind {
                Kind::Comma => {
                    self.advance();
                }
                ref kind if *kind == close => {}
                _ => return self.expect_close(&close, &open).map(|()| values),
            }
        }
    }

    fn literal(&mut self) -> Result<Value, FilterSyntaxError> {
        let token = self.advance();
        match token.kind {
            Kind::Str(text) => Ok(Value::String(text)),
            Kind::Number(raw) => Ok(number_value(&raw)),
            ref kind if kind.is_keyword("true") => Ok(Value::Bool(true)),
            ref kind if kind.is_keyword("false") => Ok(Value::Bool(false)),
            ref kind if kind.is_keyword("null") => Ok(Value::Null),
            Kind::Word(ref word) => Err(self.error_at(
                &token,
                format!(
                    "expected a literal value, found `{word}`; quote text values, e.g. \"{word}\""
                ),
            )),
            ref other => Err(self.error_at(
                &token,
                format!("expected a literal value, found {}", other.describe()),
            )),
        }
    }

    fn finish(&mut self) -> Result<(), FilterSyntaxError> {
        let token = self.peek().clone();
        if token.kind == Kind::End {
            return Ok(());
        }
        let hint = if matches!(token.kind, Kind::Word(_) | Kind::Quoted(_)) {
            "; combine conditions with `and` or `or`"
        } else {
            ""
        };
        Err(self.error_at(
            &token,
            format!(
                "unexpected {} after a complete filter{hint}",
                token.kind.describe()
            ),
        ))
    }
}

pub(super) fn number_value(raw: &str) -> Value {
    if !raw.contains('.') {
        if let Ok(unsigned) = raw.parse::<u64>() {
            return Value::from(unsigned);
        }
        if let Ok(signed) = raw.parse::<i64>() {
            return Value::from(signed);
        }
    }
    Value::String(raw.to_owned())
}

impl FilterExpr {
    /// Parse the human text form, e.g. `owned_by = "alice" and quantity >= 10`.
    ///
    /// # Errors
    /// Returns a [`FilterSyntaxError`] with the position of the offending token
    /// and a suggested fix when one is obvious.
    pub fn parse(text: &str) -> Result<Self, FilterSyntaxError> {
        if text.len() > FILTER_TEXT_MAX_BYTES {
            return Err(FilterSyntaxError::at(
                text,
                FILTER_TEXT_MAX_BYTES,
                format!("filters must not exceed {FILTER_TEXT_MAX_BYTES} bytes"),
            ));
        }
        if text.trim().is_empty() {
            return Err(FilterSyntaxError::at(
                text,
                0,
                "expected a filter expression",
            ));
        }
        let mut parser = Parser::new(text, false)?;
        let expr = parser.filter()?;
        parser.finish()?;
        expr.validate()
            .map_err(|err| FilterSyntaxError::structure(&err))?;
        Ok(expr)
    }
}

impl FromStr for FilterExpr {
    type Err = FilterSyntaxError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

/// Parse a sort specification such as `-quantity,id`.
///
/// # Errors
/// Returns a [`FilterSyntaxError`] for malformed, empty or duplicate keys.
pub fn parse_sort(text: &str) -> Result<Vec<SortKey>, FilterSyntaxError> {
    if text.trim().is_empty() {
        return Err(FilterSyntaxError::at(
            text,
            0,
            "expected at least one sort key",
        ));
    }
    let mut parser = Parser::new(text, true)?;
    let mut keys: Vec<SortKey> = Vec::new();
    loop {
        let token = parser.peek().clone();
        let order = if token.kind == Kind::Minus {
            parser.advance();
            Order::Desc
        } else {
            Order::Asc
        };
        let key = parser.path()?;
        if keys.iter().any(|existing| existing.key == key) {
            return Err(parser.error_at(&token, format!("sort key `{key}` appears more than once")));
        }
        keys.push(SortKey { key, order });
        if keys.len() > SORT_MAX_KEYS {
            return Err(parser.error_at(
                &token,
                format!("sort specifications accept at most {SORT_MAX_KEYS} keys"),
            ));
        }
        let next = parser.advance();
        match next.kind {
            Kind::End => return Ok(keys),
            Kind::Comma => {}
            Kind::Word(ref word)
                if word.eq_ignore_ascii_case("asc") || word.eq_ignore_ascii_case("desc") =>
            {
                return Err(parser.error_at(
                    &next,
                    "write `field` for ascending and `-field` for descending order",
                ));
            }
            ref other => {
                return Err(parser.error_at(
                    &next,
                    format!("expected `,` between sort keys, found {}", other.describe()),
                ));
            }
        }
    }
}

fn is_bare_segment(segment: &str, first: bool) -> bool {
    let mut chars = segment.chars();
    let starts_well = chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
    starts_well
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && !(first
            && KEYWORDS
                .iter()
                .any(|keyword| segment.eq_ignore_ascii_case(keyword)))
}

pub(super) fn write_field_path(path: &FieldPath, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (index, segment) in path.segments().enumerate() {
        if index > 0 {
            f.write_str(".")?;
        }
        if is_bare_segment(segment, index == 0) {
            f.write_str(segment)?;
        } else {
            write!(f, "`{segment}`")?;
        }
    }
    Ok(())
}

fn write_literal(value: &Value, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut out = String::new();
    norito::json::JsonSerialize::json_serialize(value, &mut out);
    f.write_str(&out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Parent {
    Root,
    Or,
    And,
    Not,
}

fn write_expr(expr: &FilterExpr, parent: Parent, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let comparison = |field: &FieldPath, op: &str, value: &Value, f: &mut fmt::Formatter<'_>| {
        write!(f, "{field} {op} ")?;
        write_literal(value, f)
    };
    let list = |field: &FieldPath, op: &str, values: &[Value], f: &mut fmt::Formatter<'_>| {
        write!(f, "{field} {op} [")?;
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write_literal(value, f)?;
        }
        f.write_str("]")
    };
    match expr {
        FilterExpr::Or(operands) | FilterExpr::And(operands) => {
            let (keyword, me, needs_parens) = if matches!(expr, FilterExpr::Or(_)) {
                ("or", Parent::Or, parent != Parent::Root)
            } else {
                (
                    "and",
                    Parent::And,
                    matches!(parent, Parent::And | Parent::Not),
                )
            };
            if needs_parens {
                f.write_str("(")?;
            }
            for (index, operand) in operands.iter().enumerate() {
                if index > 0 {
                    write!(f, " {keyword} ")?;
                }
                write_expr(operand, me, f)?;
            }
            if needs_parens {
                f.write_str(")")?;
            }
            Ok(())
        }
        FilterExpr::Not(inner) => {
            if let FilterExpr::IsNull(field) = inner.as_ref() {
                return write!(f, "{field} is not null");
            }
            f.write_str("not ")?;
            write_expr(inner, Parent::Not, f)
        }
        FilterExpr::Eq(field, value) => comparison(field, "=", value, f),
        FilterExpr::Ne(field, value) => comparison(field, "!=", value, f),
        FilterExpr::Lt(field, value) => comparison(field, "<", value, f),
        FilterExpr::Lte(field, value) => comparison(field, "<=", value, f),
        FilterExpr::Gt(field, value) => comparison(field, ">", value, f),
        FilterExpr::Gte(field, value) => comparison(field, ">=", value, f),
        FilterExpr::In(field, values) => list(field, "in", values, f),
        FilterExpr::Nin(field, values) => list(field, "not in", values, f),
        FilterExpr::Exists(field) => write!(f, "exists({field})"),
        FilterExpr::IsNull(field) => write!(f, "{field} is null"),
    }
}

impl fmt::Display for FilterExpr {
    /// Canonical text form; `FilterExpr::parse` reads it back to the same tree.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_expr(self, Parent::Root, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> FilterExpr {
        FilterExpr::parse(text).unwrap_or_else(|err| panic!("`{text}` should parse: {err}"))
    }

    fn parse_err(text: &str) -> FilterSyntaxError {
        FilterExpr::parse(text).expect_err("should not parse")
    }

    #[test]
    fn parses_comparisons_and_literals() {
        assert_eq!(
            parse(r#"owned_by = "alice""#),
            FilterExpr::Eq("owned_by".into(), Value::from("alice"))
        );
        assert_eq!(
            parse("a == 1"),
            FilterExpr::Eq("a".into(), Value::from(1u64))
        );
        assert_eq!(
            parse("a != -2"),
            FilterExpr::Ne("a".into(), Value::from(-2i64))
        );
        assert_eq!(
            parse("a <> true"),
            FilterExpr::Ne("a".into(), Value::Bool(true))
        );
        assert_eq!(
            parse("a < 10.5"),
            FilterExpr::Lt("a".into(), Value::from("10.5"))
        );
        assert_eq!(
            parse("a <= 0"),
            FilterExpr::Lte("a".into(), Value::from(0u64))
        );
        assert!(parse_err("a <= null").message.contains("range comparisons"));
        assert_eq!(
            parse("a > 'x'"),
            FilterExpr::Gt("a".into(), Value::from("x"))
        );
        assert_eq!(
            parse("a >= 340282366920938463463374607431768211455"),
            FilterExpr::Gte(
                "a".into(),
                Value::from("340282366920938463463374607431768211455")
            )
        );
    }

    #[test]
    fn parses_membership_presence_and_null_checks() {
        assert_eq!(
            parse(r#"status in ["a", "b",]"#),
            FilterExpr::In("status".into(), vec![Value::from("a"), Value::from("b")])
        );
        assert_eq!(
            parse("tier NOT IN (1, 2)"),
            FilterExpr::Nin("tier".into(), vec![Value::from(1u64), Value::from(2u64)])
        );
        assert_eq!(
            parse("exists(metadata.x)"),
            FilterExpr::Exists("metadata.x".into())
        );
        assert_eq!(parse("a is null"), FilterExpr::IsNull("a".into()));
        assert_eq!(
            parse("a IS NOT NULL"),
            FilterExpr::Not(Box::new(FilterExpr::IsNull("a".into())))
        );
    }

    #[test]
    fn precedence_is_not_then_and_then_or() {
        let expr = parse("a = 1 or not b = 2 and c = 3");
        assert_eq!(
            expr,
            FilterExpr::Or(vec![
                FilterExpr::Eq("a".into(), Value::from(1u64)),
                FilterExpr::And(vec![
                    FilterExpr::Not(Box::new(FilterExpr::Eq("b".into(), Value::from(2u64)))),
                    FilterExpr::Eq("c".into(), Value::from(3u64)),
                ]),
            ])
        );
        let grouped = parse("(a = 1 or b = 2) and c = 3");
        assert!(
            matches!(grouped, FilterExpr::And(ref list) if matches!(list[0], FilterExpr::Or(_)))
        );
    }

    #[test]
    fn paths_support_dots_and_backticks() {
        assert_eq!(
            parse("metadata.`display-name` = \"x\""),
            FilterExpr::Eq("metadata.display-name".into(), Value::from("x"))
        );
        assert_eq!(
            parse("`and` = 1"),
            FilterExpr::Eq("and".into(), Value::from(1u64))
        );
        assert_eq!(
            parse("metadata.null = 1"),
            FilterExpr::Eq("metadata.null".into(), Value::from(1u64))
        );
    }

    #[test]
    fn rendered_strings_with_del_and_c1_parse_back() {
        let expr = FilterExpr::Eq("a".into(), Value::from("x\u{7f}y\u{85}z\n"));
        assert_eq!(parse(&expr.to_string()), expr);
        assert!(FilterExpr::parse("a = \"x\u{1}y\"").is_err());
    }

    #[test]
    fn strings_decode_escapes() {
        assert_eq!(
            parse(r#"a = "q\"\\\né😀""#),
            FilterExpr::Eq("a".into(), Value::from("q\"\\\n\u{e9}\u{1f600}"))
        );
        assert_eq!(
            parse(r"a = 'it\'s'"),
            FilterExpr::Eq("a".into(), Value::from("it's"))
        );
    }

    #[test]
    fn display_roundtrips_to_the_same_tree() {
        for text in [
            r#"owned_by = "alice" and quantity >= "10.5""#,
            "a = 1 or not b = 2 and c = 3",
            "(a = 1 or b = 2) and (c = 3 or d = 4)",
            "not (a = 1 and b = 2)",
            "not not a = 1",
            "tier not in [1, 2] and exists(metadata.x) and y is null and z is not null",
            "metadata.`display-name` != \"x\" or `in` = true",
            "((a = 1 and b = 2) and c = 3)",
        ] {
            let parsed = parse(text);
            let rendered = parsed.to_string();
            assert_eq!(parse(&rendered), parsed, "{text} -> {rendered}");
        }
        assert_eq!(
            parse("owned_by = 'alice'   AND quantity>=10.5").to_string(),
            r#"owned_by = "alice" and quantity >= "10.5""#
        );
    }

    #[test]
    fn errors_point_at_the_problem() {
        let err = parse_err(r#"owned_by == "x" && quantity > 1"#);
        assert_eq!(err.column, 17);
        assert!(err.message.contains("`and`"), "{err}");

        let err = parse_err("quantity >");
        assert!(err.message.contains("expected a literal value"), "{err}");
        assert_eq!(err.column, 11);

        let err = parse_err("status = active");
        assert!(err.message.contains("quote text values"), "{err}");

        let err = parse_err("5 < quantity");
        assert!(err.message.contains("left-hand side"), "{err}");

        let err = parse_err("a in [1, 2");
        assert!(
            err.message.contains("to close the `[` at column 6"),
            "{err}"
        );

        let err = parse_err("a = 1 b = 2");
        assert!(err.message.contains("combine conditions"), "{err}");

        let err = parse_err("display-name = 1");
        assert!(err.message.contains("backticks"), "{err}");

        let err = parse_err("a = 1e5");
        assert!(err.message.contains("exponent"), "{err}");

        let err = parse_err("a = 007");
        assert!(err.message.contains("leading zeros"), "{err}");

        let err = parse_err(r#"a = "open"#);
        assert!(err.message.contains("unterminated string"), "{err}");

        let err = parse_err("status:active");
        assert!(err.message.contains("compare values with `=`"), "{err}");

        let err = parse_err("a in []");
        assert!(err.message.contains("must not be empty"), "{err}");

        let err = parse_err("a is 1");
        assert!(err.message.contains("`null` or `not null`"), "{err}");

        let err = parse_err("a = 1\nand b ~ 2");
        assert_eq!((err.line, err.column), (2, 7));
        assert!(err.to_string().contains("line 2, column 7"), "{err}");

        assert!(parse_err("").message.contains("expected a filter"));
        assert!(parse_err("and = 1").message.contains("keyword `and`"));
    }

    #[test]
    fn oversized_input_reports_a_char_boundary() {
        let text = format!("a = \"{}\"", "\u{e9}".repeat(16_400));
        let err = parse_err(&text);
        assert!(err.message.contains("must not exceed"), "{err}");
        assert!(text.is_char_boundary(err.offset));
    }

    #[test]
    fn structural_limits_apply_to_text() {
        let mut text = "a = 1".to_owned();
        for _ in 0..12 {
            text = format!("not ({text} and b = 2)");
        }
        let err = parse_err(&text);
        assert!(err.message.contains("nesting depth"), "{err}");

        let duplicate = parse_err("a in [1, 1]");
        assert!(duplicate.message.contains("unique"), "{duplicate}");

        let deep_parens = format!("{}a = 1{}", "(".repeat(100), ")".repeat(100));
        assert!(parse_err(&deep_parens).message.contains("nests too deeply"));
    }

    #[test]
    fn sort_specifications() {
        let keys = parse_sort("-quantity, id ,metadata.`ui-order`").expect("sort");
        assert_eq!(
            keys,
            vec![
                SortKey::desc("quantity"),
                SortKey::asc("id"),
                SortKey::asc("metadata.ui-order"),
            ]
        );
        assert!(
            parse_sort("id:desc")
                .unwrap_err()
                .message
                .contains("`-field`")
        );
        assert!(
            parse_sort("id desc")
                .unwrap_err()
                .message
                .contains("`-field`")
        );
        assert!(
            parse_sort("id,-id")
                .unwrap_err()
                .message
                .contains("more than once")
        );
        assert!(parse_sort("").is_err());
        assert!(parse_sort("id,").is_err());
        let many = (0..=SORT_MAX_KEYS)
            .map(|i| format!("k{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(parse_sort(&many).unwrap_err().message.contains("at most"));
    }
}
