//! Deterministic formatter for the canonical Kotodama V1 token stream.

use crate::{
    diagnostic::{Diagnostic, DiagnosticBundle, DiagnosticPhase, SourcePosition, SourceSpan},
    source::{FrontendBudget, MAX_SOURCE_BYTES, SourceFile},
    syntax::{GreenToken, SyntaxKind},
};

const INDENT: &str = "    ";

/// Format one syntactically valid Kotodama V1 source file.
///
/// The formatter consumes the same lossless tokens used to build the compiler
/// AST. Invalid sources are returned as diagnostics rather than being partly
/// rewritten. Comments and literal spelling are preserved byte-for-byte;
/// whitespace between tokens is canonicalized.
pub fn format_source(
    source: &SourceFile,
    budget: FrontendBudget,
) -> Result<String, DiagnosticBundle> {
    let parsed = crate::syntax::parse_program(source, budget);
    let crate::syntax::ProgramParseOutput {
        tree,
        program,
        diagnostics,
        tokens: _,
    } = parsed;
    let tokens = tree
        .into_tokens()
        .into_iter()
        .filter(|token| {
            !matches!(
                token.kind,
                SyntaxKind::Whitespace | SyntaxKind::Missing | SyntaxKind::Eof
            )
        })
        .collect::<Vec<_>>();
    let Some(program) = program else {
        return Err(diagnostics);
    };
    debug_assert!(diagnostics.diagnostics.is_empty());
    crate::ast::drop_program_iterative(program);
    TokenFormatter::new(source, &tokens)
        .format()
        .ok_or_else(|| formatted_source_too_large(source))
}

fn formatted_source_too_large(source: &SourceFile) -> DiagnosticBundle {
    let range = source.full_range();
    let end = source.line_column(range.end);
    let mut diagnostic = Diagnostic::error(
        "K0001",
        DiagnosticPhase::Lex,
        format!(
            "canonical formatting would exceed the {MAX_SOURCE_BYTES}-byte Kotodama V1 source limit"
        ),
        Some(SourceSpan {
            source: Some(source.name().to_owned()),
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition {
                line: end.line,
                column: end.column,
            },
            byte_range: Some(range),
        }),
    );
    diagnostic.help = Some(
        "Split the source into typed modules so the formatted deployable source remains within the V1 limit."
            .to_owned(),
    );
    DiagnosticBundle::single(diagnostic)
}

struct TokenFormatter<'source, 'tokens> {
    source: &'source SourceFile,
    tokens: &'tokens [GreenToken],
    output: String,
    indent: usize,
    at_line_start: bool,
    paren_depth: usize,
    generic_depth: usize,
    attribute_brackets: Vec<bool>,
    previous: Option<SyntaxKind>,
    previous_was_prefix: bool,
    overflowed: bool,
}

impl<'source, 'tokens> TokenFormatter<'source, 'tokens> {
    fn new(source: &'source SourceFile, tokens: &'tokens [GreenToken]) -> Self {
        Self {
            source,
            tokens,
            output: String::with_capacity(source.text().len()),
            indent: 0,
            at_line_start: true,
            paren_depth: 0,
            generic_depth: 0,
            attribute_brackets: Vec::new(),
            previous: None,
            previous_was_prefix: false,
            overflowed: false,
        }
    }

    fn format(mut self) -> Option<String> {
        for (index, token) in self.tokens.iter().enumerate() {
            let next = self.tokens.get(index + 1).map(|token| token.kind);
            let text = self.source.slice(token.range).unwrap_or_default();
            let previous_text = index
                .checked_sub(1)
                .and_then(|previous| self.tokens.get(previous))
                .and_then(|previous| self.source.slice(previous.range));
            match token.kind {
                SyntaxKind::LineComment => self.line_comment(text),
                SyntaxKind::BlockComment => self.block_comment(text),
                SyntaxKind::LBrace => self.open_brace(),
                SyntaxKind::RBrace => self.close_brace(next),
                SyntaxKind::Semicolon => self.semicolon(),
                SyntaxKind::Comma => self.comma(),
                SyntaxKind::LParen => {
                    self.ordinary(token.kind, text);
                    self.paren_depth = self.paren_depth.saturating_add(1);
                }
                SyntaxKind::RParen => {
                    self.ordinary(token.kind, text);
                    self.paren_depth = self.paren_depth.saturating_sub(1);
                }
                SyntaxKind::LBracket => {
                    let attribute = self.previous == Some(SyntaxKind::Hash);
                    self.ordinary(token.kind, text);
                    self.attribute_brackets.push(attribute);
                }
                SyntaxKind::RBracket => {
                    self.ordinary(token.kind, text);
                    if self.attribute_brackets.pop().unwrap_or(false) && next.is_some() {
                        self.newlines(1);
                    }
                }
                SyntaxKind::Less if previous_text.is_some_and(is_generic_type_name) => {
                    self.open_generic()
                }
                SyntaxKind::Greater if self.generic_depth != 0 => self.close_generic(),
                _ => self.ordinary(token.kind, text),
            }
        }
        self.newlines(1);
        (!self.overflowed).then_some(self.output)
    }

    fn ordinary(&mut self, kind: SyntaxKind, text: &str) {
        let is_prefix = is_prefix_operator(kind) && prefix_position(self.previous);
        if needs_space(self.previous, self.previous_was_prefix, kind, is_prefix) {
            self.space();
        }
        self.write(text);
        self.previous = Some(kind);
        self.previous_was_prefix = is_prefix;
    }

    fn open_brace(&mut self) {
        if needs_space(
            self.previous,
            self.previous_was_prefix,
            SyntaxKind::LBrace,
            false,
        ) {
            self.space();
        }
        self.write("{");
        self.indent = self.indent.saturating_add(1);
        self.previous = Some(SyntaxKind::LBrace);
        self.previous_was_prefix = false;
        self.newlines(1);
    }

    fn close_brace(&mut self, next: Option<SyntaxKind>) {
        self.indent = self.indent.saturating_sub(1);
        if self.previous == Some(SyntaxKind::LBrace) {
            self.trim_trailing_newlines();
        } else {
            self.newlines(1);
        }
        self.write("}");
        self.previous = Some(SyntaxKind::RBrace);
        self.previous_was_prefix = false;

        match next {
            Some(SyntaxKind::KwElse) => self.space(),
            Some(SyntaxKind::RBrace) => self.newlines(1),
            Some(
                SyntaxKind::Semicolon
                | SyntaxKind::Comma
                | SyntaxKind::RParen
                | SyntaxKind::RBracket
                | SyntaxKind::Dot
                | SyntaxKind::Question,
            ) => {}
            Some(_) if self.indent == 1 => self.newlines(2),
            Some(_) => self.newlines(1),
            None => self.newlines(1),
        }
    }

    fn semicolon(&mut self) {
        self.trim_trailing_spaces();
        self.write(";");
        self.previous = Some(SyntaxKind::Semicolon);
        self.previous_was_prefix = false;
        if self.paren_depth == 0 {
            self.newlines(1);
        } else {
            self.space();
        }
    }

    fn open_generic(&mut self) {
        self.trim_trailing_spaces();
        self.write("<");
        self.generic_depth = self.generic_depth.saturating_add(1);
        // Generic delimiters have the same spacing behavior as brackets.
        self.previous = Some(SyntaxKind::LBracket);
        self.previous_was_prefix = false;
    }

    fn close_generic(&mut self) {
        self.trim_trailing_spaces();
        self.write(">");
        self.generic_depth = self.generic_depth.saturating_sub(1);
        self.previous = Some(SyntaxKind::RBracket);
        self.previous_was_prefix = false;
    }

    fn comma(&mut self) {
        self.trim_trailing_spaces();
        self.write(",");
        self.space();
        self.previous = Some(SyntaxKind::Comma);
        self.previous_was_prefix = false;
    }

    fn line_comment(&mut self, text: &str) {
        if !self.at_line_start {
            self.space();
        }
        self.write(text.trim_end_matches(['\r', '\n']));
        self.previous = Some(SyntaxKind::LineComment);
        self.previous_was_prefix = false;
        self.newlines(1);
    }

    fn block_comment(&mut self, text: &str) {
        let standalone = self.at_line_start;
        if !self.at_line_start {
            self.space();
        }
        self.write(text);
        self.previous = Some(SyntaxKind::BlockComment);
        self.previous_was_prefix = false;
        if standalone || text.contains('\n') {
            self.newlines(1);
        } else {
            self.space();
        }
    }

    fn write(&mut self, text: &str) {
        if self.overflowed {
            return;
        }
        let indentation = if self.at_line_start {
            self.indent.saturating_mul(INDENT.len())
        } else {
            0
        };
        if self
            .output
            .len()
            .saturating_add(indentation)
            .saturating_add(text.len())
            > MAX_SOURCE_BYTES
        {
            self.overflowed = true;
            return;
        }
        if self.at_line_start {
            for _ in 0..self.indent {
                self.output.push_str(INDENT);
            }
            self.at_line_start = false;
        }
        self.output.push_str(text);
        self.at_line_start = self.output.ends_with('\n');
    }

    fn space(&mut self) {
        if !self.at_line_start
            && !self
                .output
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_whitespace)
        {
            if self.output.len() == MAX_SOURCE_BYTES {
                self.overflowed = true;
            } else {
                self.output.push(' ');
            }
        }
    }

    fn newlines(&mut self, count: usize) {
        self.trim_trailing_spaces();
        let existing = self
            .output
            .as_bytes()
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\n')
            .count();
        for _ in existing..count {
            if self.output.len() == MAX_SOURCE_BYTES {
                self.overflowed = true;
                break;
            }
            self.output.push('\n');
        }
        self.at_line_start = true;
    }

    fn trim_trailing_spaces(&mut self) {
        while self
            .output
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        {
            self.output.pop();
        }
    }

    fn trim_trailing_newlines(&mut self) {
        self.trim_trailing_spaces();
        while self.output.ends_with('\n') {
            self.output.pop();
        }
        self.at_line_start = self.output.is_empty();
    }
}

const fn is_word(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Ident
            | SyntaxKind::Number
            | SyntaxKind::String
            | SyntaxKind::Bytes
            | SyntaxKind::KwFn
            | SyntaxKind::KwLet
            | SyntaxKind::KwVar
            | SyntaxKind::KwConst
            | SyntaxKind::KwReturn
            | SyntaxKind::KwBreak
            | SyntaxKind::KwContinue
            | SyntaxKind::KwState
            | SyntaxKind::KwStruct
            | SyntaxKind::KwError
            | SyntaxKind::KwEnum
            | SyntaxKind::KwAuthorize
            | SyntaxKind::KwTrigger
            | SyntaxKind::KwIf
            | SyntaxKind::KwElse
            | SyntaxKind::KwFor
            | SyntaxKind::KwIn
            | SyntaxKind::KwSeiyaku
            | SyntaxKind::KwModule
            | SyntaxKind::KwKotoage
            | SyntaxKind::KwHajimari
            | SyntaxKind::KwKaizen
            | SyntaxKind::KwView
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse
    )
}

const fn is_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Plus
            | SyntaxKind::PlusEqual
            | SyntaxKind::Minus
            | SyntaxKind::MinusEqual
            | SyntaxKind::Arrow
            | SyntaxKind::Star
            | SyntaxKind::StarEqual
            | SyntaxKind::Slash
            | SyntaxKind::SlashEqual
            | SyntaxKind::Percent
            | SyntaxKind::PercentEqual
            | SyntaxKind::Bang
            | SyntaxKind::BangEqual
            | SyntaxKind::Equal
            | SyntaxKind::EqualEqual
            | SyntaxKind::Less
            | SyntaxKind::LessEqual
            | SyntaxKind::Greater
            | SyntaxKind::GreaterEqual
            | SyntaxKind::AndAnd
            | SyntaxKind::OrOr
    )
}

const fn is_prefix_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Bang | SyntaxKind::Minus | SyntaxKind::Plus
    )
}

fn is_generic_type_name(name: &str) -> bool {
    matches!(name, "Option" | "Result" | "Secret" | "StateMap")
}

const fn prefix_position(previous: Option<SyntaxKind>) -> bool {
    match previous {
        None => true,
        Some(kind) => matches!(
            kind,
            SyntaxKind::LParen
                | SyntaxKind::LBracket
                | SyntaxKind::LBrace
                | SyntaxKind::Comma
                | SyntaxKind::Colon
                | SyntaxKind::Semicolon
                | SyntaxKind::Question
                | SyntaxKind::KwReturn
                | SyntaxKind::KwLet
                | SyntaxKind::KwVar
                | SyntaxKind::Equal
                | SyntaxKind::Plus
                | SyntaxKind::PlusEqual
                | SyntaxKind::Minus
                | SyntaxKind::MinusEqual
                | SyntaxKind::Star
                | SyntaxKind::StarEqual
                | SyntaxKind::Slash
                | SyntaxKind::SlashEqual
                | SyntaxKind::Percent
                | SyntaxKind::PercentEqual
                | SyntaxKind::Bang
                | SyntaxKind::BangEqual
                | SyntaxKind::EqualEqual
                | SyntaxKind::Less
                | SyntaxKind::LessEqual
                | SyntaxKind::Greater
                | SyntaxKind::GreaterEqual
                | SyntaxKind::AndAnd
                | SyntaxKind::OrOr
        ),
    }
}

const fn needs_space(
    previous: Option<SyntaxKind>,
    previous_was_prefix: bool,
    current: SyntaxKind,
    current_is_prefix: bool,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if matches!(
        current,
        SyntaxKind::RParen
            | SyntaxKind::RBracket
            | SyntaxKind::Semicolon
            | SyntaxKind::Comma
            | SyntaxKind::Colon
            | SyntaxKind::ColonColon
            | SyntaxKind::Dot
            | SyntaxKind::Question
    ) || matches!(
        previous,
        SyntaxKind::LParen
            | SyntaxKind::LBracket
            | SyntaxKind::Hash
            | SyntaxKind::ColonColon
            | SyntaxKind::Dot
    ) {
        return false;
    }
    if matches!(current, SyntaxKind::LParen) {
        return !matches!(
            previous,
            SyntaxKind::Ident
                | SyntaxKind::RParen
                | SyntaxKind::RBracket
                | SyntaxKind::KwAuthorize
                | SyntaxKind::KwHajimari
                | SyntaxKind::KwKaizen
        );
    }
    if matches!(current, SyntaxKind::LBracket) {
        return !matches!(
            previous,
            SyntaxKind::Ident | SyntaxKind::RParen | SyntaxKind::RBracket | SyntaxKind::Hash
        );
    }
    if matches!(current, SyntaxKind::LBrace) {
        return !matches!(previous, SyntaxKind::LBrace);
    }
    if matches!(previous, SyntaxKind::Colon) {
        return true;
    }
    if current_is_prefix {
        return is_word(previous);
    }
    if previous_was_prefix {
        return false;
    }
    if is_operator(current) || is_operator(previous) {
        return true;
    }
    is_word(previous) && is_word(current)
        || matches!(previous, SyntaxKind::RParen | SyntaxKind::RBracket) && is_word(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn format(source: &str) -> String {
        let source = SourceFile::new(SourceId(0), "format.ko", source);
        format_source(&source, FrontendBudget::v1()).expect("valid source")
    }

    #[test]
    fn canonicalizes_blocks_operators_and_declarations() {
        let formatted = format(
            "seiyaku Demo{state count:i64;hajimari(){count=0;}kotoage fn bump(value:i64)->i64 authorize(\"Write\"){var total:i64=count+value;if total>10{total=10;}return total;}view fn read()->i64{return count;}}",
        );
        assert_eq!(
            formatted,
            concat!(
                "seiyaku Demo {\n",
                "    state count: i64;\n",
                "    hajimari() {\n",
                "        count = 0;\n",
                "    }\n\n",
                "    kotoage fn bump(value: i64) -> i64 authorize(\"Write\") {\n",
                "        var total: i64 = count + value;\n",
                "        if total > 10 {\n",
                "            total = 10;\n",
                "        }\n",
                "        return total;\n",
                "    }\n\n",
                "    view fn read() -> i64 {\n",
                "        return count;\n",
                "    }\n",
                "}\n",
            )
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn preserves_comments_literals_and_canonical_keywords() {
        let source =
            "seiyaku Demo{/* exact */view fn text()->string{// keep me\nreturn r#\"a  b\"#;}}";
        let formatted = format(source);
        assert!(formatted.contains("/* exact */"));
        assert!(formatted.contains("// keep me"));
        assert!(formatted.contains("r#\"a  b\"#"));
        assert!(formatted.starts_with("seiyaku Demo {\n"));
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn distinguishes_generic_delimiters_from_comparisons() {
        let formatted = format(
            "seiyaku Demo{state values:StateMap<string,Option<i64>>;view fn less(a:i64,b:i64)->bool{return a<b;}}",
        );
        assert!(formatted.contains("StateMap<string, Option<i64>>"));
        assert!(formatted.contains("return a < b;"));
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn refuses_to_rewrite_invalid_sources() {
        let source = SourceFile::new(SourceId(0), "bad.ko", "seiyaku Demo { return ; }");
        let diagnostics = format_source(&source, FrontendBudget::v1())
            .expect_err("invalid source must not be formatted");
        assert!(!diagnostics.diagnostics.is_empty());
    }

    #[test]
    fn refuses_output_expansion_beyond_the_source_budget() {
        let mut text = String::from("seiyaku Demo { view fn run() {");
        for _ in 0..16 {
            text.push_str("if true {");
        }
        for _ in 0..14_000 {
            text.push_str("value = 0;");
        }
        for _ in 0..16 {
            text.push('}');
        }
        text.push_str("} }");
        assert!(text.len() < MAX_SOURCE_BYTES);

        let source = SourceFile::new(SourceId(0), "expansion.ko", text);
        let diagnostics = format_source(&source, FrontendBudget::v1())
            .expect_err("formatter expansion must remain bounded");
        assert_eq!(diagnostics.diagnostics[0].code, "K0001");
    }
}
