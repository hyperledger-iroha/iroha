//! Structured diagnostics shared by the Kotodama compiler, CLI, and language tools.
use crate::source::{SourceFile, TextRange};
use norito::json::{self, Value};
use std::{error::Error as StdError, fmt};
/// Maximum number of diagnostics returned for one compilation request.
///
/// The cap bounds memory and renderer work for adversarial source files while
/// reserving the final slot for an explicit truncation diagnostic.
pub const MAX_DIAGNOSTICS: usize = 64;
/// Compiler phase that produced a diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticPhase {
    /// Tokenization failed.
    Lex,
    /// Parsing failed.
    Parse,
    /// Name binding or module resolution failed.
    Resolve,
    /// Type, effect, or policy analysis failed.
    Semantic,
    /// Typed lowering or optimization failed.
    Lowering,
    /// Artifact construction or verification failed.
    Artifact,
}
impl DiagnosticPhase {
    /// Stable machine-readable phase name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lex => "lex",
            Self::Parse => "parse",
            Self::Resolve => "resolve",
            Self::Semantic => "semantic",
            Self::Lowering => "lowering",
            Self::Artifact => "artifact",
        }
    }
}
/// Diagnostic severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Compilation cannot continue.
    Error,
    /// Compilation can continue but the source should be changed.
    Warning,
}
impl Severity {
    /// Stable machine-readable severity name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
    const fn sarif_level(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}
/// Stable explanation and remediation for one compiler diagnostic code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticExplanation {
    /// Stable diagnostic identifier.
    pub code: &'static str,
    /// Compiler phase that owns the diagnostic.
    pub phase: DiagnosticPhase,
    /// Short description suitable for command-line help and reference tables.
    pub summary: &'static str,
    /// Concrete remediation guidance.
    pub help: &'static str,
}
include!(concat!(env!("OUT_DIR"), "/kotodama_diagnostic_explanations.rs"));
/// Preserve resolver ownership when a later typed-analysis adapter surfaces a
/// diagnostic whose canonical registry entry belongs to resolution.
///
/// The semantic analyzer consumes resolved HIR and can therefore detect a
/// stale or inconsistent resolver result. Other semantic failures retain their
/// actual semantic phase, including cross-phase fanout diagnostics such as
/// `K0004`.
pub(crate) fn phase_for_semantic_failure(code: &str) -> DiagnosticPhase {
    match diagnostic_explanation(code) {
        Some(explanation) if explanation.phase == DiagnosticPhase::Resolve => {
            DiagnosticPhase::Resolve
        }
        _ => DiagnosticPhase::Semantic,
    }
}
/// Look up a canonical diagnostic explanation by case-insensitive code.
#[must_use]
pub fn diagnostic_explanation(code: &str) -> Option<&'static DiagnosticExplanation> {
    DIAGNOSTIC_EXPLANATIONS
        .iter()
        .find(|explanation| explanation.code.eq_ignore_ascii_case(code))
}
/// One source position using one-based line and column numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePosition {
    /// One-based source line.
    pub line: usize,
    /// One-based UTF-8 display column.
    pub column: usize,
}
/// Half-open source range attached to a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    /// Exact locked package identity, when the span belongs to a reusable
    /// package rather than the deployable root.
    pub package_identity: Option<String>,
    /// Logical source path, when known.
    pub source: Option<String>,
    /// First covered position.
    pub start: SourcePosition,
    /// Position immediately after the range.
    pub end: SourcePosition,
    /// Exact half-open UTF-8 byte range when the source text is available.
    ///
    /// Line and column positions are retained for humans and SARIF consumers,
    /// while this range makes diagnostics unambiguous in the presence of
    /// multi-byte Unicode text.
    pub byte_range: Option<TextRange>,
}
impl SourceSpan {
    /// Convert an exact source-file byte range into the canonical diagnostic span.
    #[must_use]
    pub fn from_range(source: &SourceFile, range: TextRange) -> Self {
        let start = source.line_column(range.start);
        let end = source.line_column(range.end);
        Self {
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
        }
    }
}
/// Secondary source label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticLabel {
    /// Labeled range.
    pub span: SourceSpan,
    /// Explanation for the range.
    pub message: String,
}
/// Machine-applicable source replacement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticFix {
    /// Range to replace.
    pub span: SourceSpan,
    /// Replacement text.
    pub replacement: String,
}
/// One stable, structured compiler diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable Kotodama diagnostic code.
    pub code: String,
    /// Severity.
    pub severity: Severity,
    /// Producing phase.
    pub phase: DiagnosticPhase,
    /// Primary human-readable message.
    pub message: String,
    /// Primary source range, when available.
    pub primary_span: Option<SourceSpan>,
    /// Additional labeled ranges.
    pub labels: Vec<DiagnosticLabel>,
    /// Contextual notes.
    pub notes: Vec<String>,
    /// Suggested next action.
    pub help: Option<String>,
    /// Optional machine-applicable replacement.
    pub fix: Option<DiagnosticFix>,
}
impl Diagnostic {
    /// Construct a native compiler error with an explicit stable code and span.
    pub fn error(
        code: impl Into<String>,
        phase: DiagnosticPhase,
        message: impl Into<String>,
        primary_span: Option<SourceSpan>,
    ) -> Self {
        let code = code.into();
        let help = diagnostic_explanation(&code).map(|entry| entry.help.to_owned());
        Self {
            code,
            severity: Severity::Error,
            phase,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            help,
            fix: None,
        }
    }
    /// Construct a non-fatal warning with an explicit stable code and span.
    pub fn warning(
        code: impl Into<String>,
        phase: DiagnosticPhase,
        message: impl Into<String>,
        primary_span: Option<SourceSpan>,
    ) -> Self {
        let code = code.into();
        let help = diagnostic_explanation(&code).map(|entry| entry.help.to_owned());
        Self {
            code,
            severity: Severity::Warning,
            phase,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            help,
            fix: None,
        }
    }
    /// Return the canonical JSON representation used by every diagnostic renderer.
    pub fn to_json_value(&self) -> Value {
        json_object(vec![
            json_entry("code", Value::from(self.code.clone())),
            json_entry("severity", Value::from(self.severity.as_str())),
            json_entry("phase", Value::from(self.phase.as_str())),
            json_entry("message", Value::from(self.message.clone())),
            json_entry(
                "primary_span",
                self.primary_span
                    .as_ref()
                    .map_or(Value::Null, source_span_to_json),
            ),
            json_entry(
                "labels",
                Value::Array(
                    self.labels
                        .iter()
                        .map(|label| {
                            json_object(vec![
                                json_entry("span", source_span_to_json(&label.span)),
                                json_entry("message", Value::from(label.message.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
            json_entry(
                "notes",
                Value::Array(self.notes.iter().cloned().map(Value::from).collect()),
            ),
            json_entry("help", self.help.clone().map_or(Value::Null, Value::from)),
            json_entry(
                "fix",
                self.fix.as_ref().map_or(Value::Null, |fix| {
                    json_object(vec![
                        json_entry("span", source_span_to_json(&fix.span)),
                        json_entry("replacement", Value::from(fix.replacement.clone())),
                    ])
                }),
            ),
        ])
    }
    fn to_sarif_result(&self) -> Value {
        let locations = self.primary_span.as_ref().map_or_else(Vec::new, |span| {
            vec![json_object(vec![json_entry(
                "physicalLocation",
                source_span_to_sarif(span),
            )])]
        });
        let related_locations = self
            .labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                json_object(vec![
                    json_entry("id", Value::from(index as u64 + 1)),
                    json_entry("physicalLocation", source_span_to_sarif(&label.span)),
                    json_entry(
                        "message",
                        json_object(vec![json_entry("text", Value::from(label.message.clone()))]),
                    ),
                ])
            })
            .collect();
        json_object(vec![
            json_entry("ruleId", Value::from(self.code.clone())),
            json_entry("level", Value::from(self.severity.sarif_level())),
            json_entry(
                "message",
                json_object(vec![json_entry("text", Value::from(self.message.clone()))]),
            ),
            json_entry("locations", Value::Array(locations)),
            json_entry("relatedLocations", Value::Array(related_locations)),
            // Keeping the canonical record in SARIF properties guarantees that JSON and
            // SARIF consumers observe exactly the same semantic fields, including fixes.
            json_entry(
                "properties",
                json_object(vec![json_entry("kotodama", self.to_json_value())]),
            ),
        ])
    }
}
fn json_entry(key: impl Into<String>, value: Value) -> (String, Value) {
    (key.into(), value)
}
fn json_object(entries: Vec<(String, Value)>) -> Value {
    json::object(entries).unwrap_or(Value::Null)
}
fn source_position_to_json(position: SourcePosition) -> Value {
    json_object(vec![
        json_entry("line", Value::from(position.line as u64)),
        json_entry("column", Value::from(position.column as u64)),
    ])
}
fn display_source_span(span: &SourceSpan) -> String {
    let source = span.source.as_deref().unwrap_or("<source>");
    span.package_identity.as_ref().map_or_else(
        || source.to_owned(),
        |package| format!("{package}::{source}"),
    )
}
fn source_span_to_json(span: &SourceSpan) -> Value {
    json_object(vec![
        json_entry(
            "package_identity",
            span.package_identity
                .clone()
                .map_or(Value::Null, Value::from),
        ),
        json_entry(
            "source",
            span.source.clone().map_or(Value::Null, Value::from),
        ),
        json_entry("start", source_position_to_json(span.start)),
        json_entry("end", source_position_to_json(span.end)),
        json_entry(
            "byte_range",
            span.byte_range.map_or(Value::Null, |range| {
                json_object(vec![
                    json_entry("start", Value::from(u64::from(range.start))),
                    json_entry("end", Value::from(u64::from(range.end))),
                ])
            }),
        ),
    ])
}
fn source_span_to_sarif(span: &SourceSpan) -> Value {
    let artifact_location = span.source.as_ref().map_or(Value::Null, |source| {
        json_object(vec![json_entry("uri", Value::from(source.clone()))])
    });
    let mut region = vec![
        json_entry("startLine", Value::from(span.start.line as u64)),
        json_entry("startColumn", Value::from(span.start.column as u64)),
        json_entry("endLine", Value::from(span.end.line as u64)),
        json_entry("endColumn", Value::from(span.end.column as u64)),
    ];
    if let Some(range) = span.byte_range {
        region.push(json_entry(
            "byteOffset",
            Value::from(u64::from(range.start)),
        ));
        region.push(json_entry(
            "byteLength",
            Value::from(u64::from(range.len())),
        ));
    }
    json_object(vec![
        json_entry("artifactLocation", artifact_location),
        json_entry("region", json_object(region)),
    ])
}
/// Collection of diagnostics returned by a failed compiler operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticBundle {
    /// Diagnostics in deterministic source order.
    pub diagnostics: Vec<Diagnostic>,
}
impl DiagnosticBundle {
    /// Build a bundle and normalize it into deterministic source order.
    pub fn new(mut diagnostics: Vec<Diagnostic>) -> Self {
        fn compare(left: &Diagnostic, right: &Diagnostic) -> std::cmp::Ordering {
            let left_span = left.primary_span.as_ref();
            let right_span = right.primary_span.as_ref();
            left_span
                .is_none()
                .cmp(&right_span.is_none())
                .then_with(|| {
                    left_span
                        .and_then(|span| span.package_identity.as_deref())
                        .cmp(&right_span.and_then(|span| span.package_identity.as_deref()))
                })
                .then_with(|| {
                    left_span
                        .and_then(|span| span.source.as_deref())
                        .cmp(&right_span.and_then(|span| span.source.as_deref()))
                })
                .then_with(|| {
                    left_span
                        .map(|span| (span.start.line, span.start.column))
                        .cmp(&right_span.map(|span| (span.start.line, span.start.column)))
                })
                .then_with(|| left.phase.as_str().cmp(right.phase.as_str()))
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        }
        if diagnostics.len() > MAX_DIAGNOSTICS {
            let omitted = diagnostics.len() - (MAX_DIAGNOSTICS - 1);
            let has_errors = diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error);
            diagnostics.sort_by(|left, right| {
                let severity_rank = |severity| match severity {
                    Severity::Error => 0_u8,
                    Severity::Warning => 1_u8,
                };
                severity_rank(left.severity)
                    .cmp(&severity_rank(right.severity))
                    .then_with(|| compare(left, right))
            });
            let phase = diagnostics[MAX_DIAGNOSTICS - 1].phase;
            diagnostics.truncate(MAX_DIAGNOSTICS - 1);
            let message = format!(
                "diagnostic limit reached; {omitted} additional diagnostic(s) were omitted"
            );
            diagnostics.push(if has_errors {
                Diagnostic::error("K0004", phase, message, None)
            } else {
                Diagnostic::warning("K0004", phase, message, None)
            });
        }
        diagnostics.sort_by(compare);
        Self { diagnostics }
    }
    /// Build a bundle containing one native compiler error.
    pub fn single(diagnostic: Diagnostic) -> Self {
        Self::new(vec![diagnostic])
    }
    /// Render deterministic human-readable diagnostics.
    pub fn render_human(&self) -> String {
        let mut output = String::new();
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index != 0 {
                output.push('\n');
            }
            use std::fmt::Write as _;
            let _ = write!(
                output,
                "{}[{}] {}: {}",
                diagnostic.severity.as_str(),
                diagnostic.code,
                diagnostic.phase.as_str(),
                diagnostic.message
            );
            if let Some(span) = &diagnostic.primary_span {
                let source = display_source_span(span);
                let _ = write!(
                    output,
                    "\n  --> {source}:{}:{}-{}:{}",
                    span.start.line, span.start.column, span.end.line, span.end.column
                );
                if let Some(range) = span.byte_range {
                    let _ = write!(output, " [bytes {}..{}]", range.start, range.end);
                }
            }
            for label in &diagnostic.labels {
                let source = display_source_span(&label.span);
                let _ = write!(
                    output,
                    "\n  = label: {source}:{}:{}-{}:{}: {}",
                    label.span.start.line,
                    label.span.start.column,
                    label.span.end.line,
                    label.span.end.column,
                    label.message
                );
                if let Some(range) = label.span.byte_range {
                    let _ = write!(output, " [bytes {}..{}]", range.start, range.end);
                }
            }
            for note in &diagnostic.notes {
                let _ = write!(output, "\n  = note: {note}");
            }
            if let Some(help) = &diagnostic.help {
                let _ = write!(output, "\n  = help: {help}");
            }
            if let Some(fix) = &diagnostic.fix {
                let source = display_source_span(&fix.span);
                let _ = write!(
                    output,
                    "\n  = fix: replace {source}:{}:{}-{}:{} with {:?}",
                    fix.span.start.line,
                    fix.span.start.column,
                    fix.span.end.line,
                    fix.span.end.column,
                    fix.replacement
                );
                if let Some(range) = fix.span.byte_range {
                    let _ = write!(output, " [bytes {}..{}]", range.start, range.end);
                }
            }
        }
        output
    }
    /// Render the canonical diagnostic array as pretty JSON.
    pub fn render_json(&self) -> Result<String, json::Error> {
        json::to_string_pretty(&Value::Array(
            self.diagnostics
                .iter()
                .map(Diagnostic::to_json_value)
                .collect(),
        ))
    }
    /// Render SARIF 2.1.0 while preserving the canonical diagnostic records.
    pub fn render_sarif(&self) -> Result<String, json::Error> {
        let rules = self
            .diagnostics
            .iter()
            .map(|diagnostic| {
                json_object(vec![
                    json_entry("id", Value::from(diagnostic.code.clone())),
                    json_entry(
                        "shortDescription",
                        json_object(vec![json_entry(
                            "text",
                            Value::from(diagnostic.message.clone()),
                        )]),
                    ),
                ])
            })
            .collect();
        let results = self
            .diagnostics
            .iter()
            .map(Diagnostic::to_sarif_result)
            .collect();
        let sarif = json_object(vec![
            json_entry("version", Value::from("2.1.0")),
            json_entry(
                "$schema",
                Value::from("https://json.schemastore.org/sarif-2.1.0.json"),
            ),
            json_entry(
                "runs",
                Value::Array(vec![json_object(vec![
                    json_entry(
                        "tool",
                        json_object(vec![json_entry(
                            "driver",
                            json_object(vec![
                                json_entry("name", Value::from("Kotodama")),
                                json_entry("rules", Value::Array(rules)),
                            ]),
                        )]),
                    ),
                    json_entry("results", Value::Array(results)),
                ])]),
            ),
        ]);
        json::to_string_pretty(&sarif)
    }
}
impl fmt::Display for DiagnosticBundle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render_human())
    }
}
impl StdError for DiagnosticBundle {}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explanation_registry_is_unique_and_case_insensitive() {
        let mut codes = std::collections::BTreeSet::new();
        for explanation in DIAGNOSTIC_EXPLANATIONS {
            assert!(
                codes.insert(explanation.code),
                "duplicate explanation for {}",
                explanation.code
            );
            assert!(!explanation.summary.is_empty());
            assert!(!explanation.help.is_empty());
        }
        assert_eq!(
            diagnostic_explanation("k1001").map(|entry| entry.code),
            Some("K1001")
        );
        assert!(diagnostic_explanation("NOT_A_CODE").is_none());
    }
    #[test]
    fn every_frontend_code_literal_has_a_canonical_explanation() {
        fn is_diagnostic_code(candidate: &str) -> bool {
            (candidate.strip_prefix("E_").is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.bytes().all(|byte| {
                        byte.is_ascii_uppercase() || byte == b'_' || byte.is_ascii_digit()
                    })
            })) || (candidate.len() == 5
                && candidate.starts_with('K')
                && candidate[1..].bytes().all(|byte| byte.is_ascii_digit()))
        }
        let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = std::collections::BTreeMap::<String, Vec<String>>::new();
        for entry in std::fs::read_dir(source_dir).expect("read Kotodama frontend sources") {
            let entry = entry.expect("read Kotodama frontend source entry");
            let path = entry.path();
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read Kotodama frontend source");
            for (quote, _) in source.match_indices('"') {
                let remainder = &source[quote + 1..];
                let length = remainder
                    .bytes()
                    .take_while(|byte| {
                        byte.is_ascii_uppercase() || *byte == b'_' || byte.is_ascii_digit()
                    })
                    .count();
                if remainder.as_bytes().get(length) != Some(&b'"') {
                    continue;
                }
                let code = &remainder[..length];
                if !is_diagnostic_code(code)
                    || matches!(code, "E_FIRST" | "E_SECOND" | "E_THIRD")
                    || diagnostic_explanation(code).is_some()
                {
                    continue;
                }
                missing.entry(code.to_owned()).or_default().push(
                    path.file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or("<unknown>")
                        .to_owned(),
                );
            }
        }
        assert!(
            missing.is_empty(),
            "frontend diagnostic codes missing from `koto explain`: {missing:?}"
        );
    }
    #[test]
    fn resolve_explanations_match_public_session_emitters() {
        use crate::session::{CompileRequest, CompilerSession};
        for (code, source) in [
            (
                "K2002",
                "seiyaku Unknown { view fn run() -> int { return missing; } }",
            ),
            (
                "E_DUPLICATE_DECLARATION",
                "seiyaku Duplicate { fn repeated() {} fn repeated() {} }",
            ),
            (
                "E_RESERVED_DECLARATION",
                "seiyaku Reserved { fn account_id(string value) -> int { return 1; } }",
            ),
            (
                "E_LOCAL_SHADOWING",
                "seiyaku Shadow { const int limit = 1; view fn run(int limit) -> int { return limit; } }",
            ),
        ] {
            let diagnostics = CompilerSession::default()
                .check(CompileRequest {
                    source,
                    source_name: Some("phase-parity.ko"),
                })
                .expect_err("resolver fixture must fail");
            let emitted = diagnostics
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == code)
                .unwrap_or_else(|| panic!("fixture did not emit {code}: {diagnostics:?}"));
            let explanation = diagnostic_explanation(code)
                .unwrap_or_else(|| panic!("{code} must work with `koto explain`"));
            assert_eq!(emitted.phase, DiagnosticPhase::Resolve, "{code}");
            assert_eq!(explanation.phase, emitted.phase, "{code}");
        }
    }
    #[test]
    fn fixed_source_graph_and_linker_codes_are_explainable() {
        for code in ["K1004", "E_PACKAGE_BUDGET"] {
            let explanation = diagnostic_explanation(code)
                .unwrap_or_else(|| panic!("{code} must work with `koto explain`"));
            assert_eq!(explanation.phase, DiagnosticPhase::Parse, "{code}");
        }
        for code in [
            "K2002",
            "K2099",
            "E_ROOT_MUST_BE_SEIYAKU",
            "E_DEPENDENCY_MUST_BE_MODULE",
            "E_DUPLICATE_PACKAGE",
            "E_EMPTY_PACKAGE",
            "E_DUPLICATE_MODULE",
            "E_DUPLICATE_IMPORT",
            "E_RESERVED_IMPORT",
            "E_DUPLICATE_DECLARATION",
            "E_UNKNOWN_PACKAGE",
            "E_PACKAGE_IMPORT_CYCLE",
            "E_UNKNOWN_IMPORT_ALIAS",
            "E_MULTIPLE_SEIYAKU_ROOTS",
            "E_PROJECT_MANIFEST_REQUIRED",
            "E_PROJECT_MANIFEST",
            "E_UNEXPORTED_SYMBOL",
            "E_MISSING_EXPORT",
            "E_AMBIGUOUS_EXPORT",
            "E_WILDCARD_IMPORT",
            "E_INVALID_IDENTIFIER",
            "E_RESERVED_DECLARATION",
            "E_INVALID_MODULE_ITEM",
            "E_DUPLICATE_ERROR_CODE",
            "E_DUPLICATE_MESSAGE",
            "E_INVALID_SOURCE_PATH",
            "E_DUPLICATE_SOURCE",
            "E_EMPTY_PACKAGE_GRAPH",
            "E_DUPLICATE_HIR_ID",
            "E_DUPLICATE_SOURCE_ID",
            "E_TEST_TARGET_MISMATCH",
            "E_LOCAL_SHADOWING",
            "E_INTERNAL_RESOLUTION",
        ] {
            let explanation = diagnostic_explanation(code)
                .unwrap_or_else(|| panic!("{code} must work with `koto explain`"));
            assert_eq!(explanation.phase, DiagnosticPhase::Resolve, "{code}");
        }
        assert_eq!(
            diagnostic_explanation("K2098").map(|entry| entry.phase),
            Some(DiagnosticPhase::Semantic),
            "the semantic ABI fallback must not reuse the resolver-owned K2099 code",
        );
    }
    #[test]
    fn v1_data_processing_diagnostics_are_explainable() {
        for code in [
            "E_DIVISION_BY_ZERO",
            "E_REPEATING_DECIMAL",
            "E_EXACT_DIVISION_SCALE_OVERFLOW",
            "E_INEXACT_CONVERSION",
            "E_QUANTITY_UNDERFLOW",
            "E_QUANTITY_REMAINDER",
            "E_QUANTITY_NEGATION",
            "E_QUORUM_RANGE",
            "E_NUMERIC_ROUND_ARITY",
            "E_NUMERIC_ROUND_RECEIVER",
            "E_INVALID_SCALE",
            "E_NUMERIC_ROUNDING_MODE",
            "E_DIVERGING_EXPRESSION_CONTEXT",
            "E_LIST_CONTAINS_COMPARABILITY",
        ] {
            let explanation = diagnostic_explanation(code)
                .unwrap_or_else(|| panic!("{code} must be registered for `koto explain`"));
            assert_eq!(explanation.phase, DiagnosticPhase::Semantic, "{code}");
        }
    }
    #[test]
    fn every_renderer_contains_the_same_canonical_fields() {
        let primary_span = SourceSpan {
            package_identity: Some("std/example@1.0.0".to_owned()),
            source: Some("seiyaku.ko".to_owned()),
            start: SourcePosition { line: 3, column: 5 },
            end: SourcePosition { line: 3, column: 6 },
            byte_range: Some(crate::source::TextRange::new(12, 13)),
        };
        let mut diagnostic = Diagnostic::error(
            "K2001",
            DiagnosticPhase::Semantic,
            "unknown name",
            Some(primary_span),
        );
        diagnostic.labels.push(DiagnosticLabel {
            span: diagnostic.primary_span.clone().expect("span"),
            message: "not declared in this scope".to_owned(),
        });
        diagnostic.notes.push("names are case-sensitive".to_owned());
        diagnostic.help = Some("declare the value before use".to_owned());
        diagnostic.fix = Some(DiagnosticFix {
            span: diagnostic.primary_span.clone().expect("span"),
            replacement: "known_name".to_owned(),
        });
        let bundle = DiagnosticBundle {
            diagnostics: vec![diagnostic.clone()],
        };
        let human = bundle.render_human();
        for expected in [
            "K2001",
            "semantic",
            "seiyaku.ko:3:5",
            "not declared in this scope",
            "names are case-sensitive",
            "declare the value before use",
            "known_name",
        ] {
            assert!(human.contains(expected), "missing {expected:?}: {human}");
        }
        let rendered_json = bundle.render_json().expect("JSON diagnostics");
        let rendered_sarif = bundle.render_sarif().expect("SARIF diagnostics");
        for expected in [
            "K2001",
            "semantic",
            "seiyaku.ko",
            "not declared in this scope",
            "names are case-sensitive",
            "declare the value before use",
            "known_name",
        ] {
            assert!(
                rendered_json.contains(expected),
                "JSON missing {expected:?}"
            );
            assert!(
                rendered_sarif.contains(expected),
                "SARIF missing {expected:?}"
            );
        }
        assert!(rendered_sarif.contains("2.1.0"));
        let json_value: Value =
            json::from_str(&rendered_json).expect("decode canonical JSON diagnostics");
        let sarif_value: Value = json::from_str(&rendered_sarif).expect("decode SARIF diagnostics");
        let canonical = json_value
            .as_array()
            .and_then(|diagnostics| diagnostics.first())
            .expect("one canonical diagnostic");
        let embedded = sarif_value
            .pointer("/runs/0/results/0/properties/kotodama")
            .expect("SARIF embeds the canonical diagnostic");
        assert_eq!(canonical, embedded);
        assert!(
            human.contains("seiyaku.ko:3:5-3:6"),
            "human renderer must preserve the full primary and label range"
        );
    }
    #[test]
    fn bundle_order_and_fanout_are_deterministic_and_bounded() {
        let diagnostics = (0..80)
            .rev()
            .map(|index| {
                Diagnostic::error(
                    "K2002",
                    DiagnosticPhase::Resolve,
                    format!("unknown value {index}"),
                    Some(SourceSpan {
                        package_identity: None,
                        source: Some("fanout.ko".to_owned()),
                        start: SourcePosition {
                            line: index + 1,
                            column: 1,
                        },
                        end: SourcePosition {
                            line: index + 1,
                            column: 2,
                        },
                        byte_range: None,
                    }),
                )
            })
            .collect();
        let bundle = DiagnosticBundle::new(diagnostics);
        assert_eq!(bundle.diagnostics.len(), MAX_DIAGNOSTICS);
        let source_lines = bundle
            .diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.primary_span.as_ref().map(|span| span.start.line))
            .collect::<Vec<_>>();
        assert!(source_lines.windows(2).all(|pair| pair[0] < pair[1]));
        let limit = bundle.diagnostics.last().expect("limit diagnostic");
        assert_eq!(limit.code, "K0004");
        assert!(limit.message.contains("17 additional diagnostic(s)"));
        assert_eq!(limit.severity, Severity::Error);
    }
    #[test]
    fn fanout_retains_errors_ahead_of_warnings_without_making_warning_only_checks_fail() {
        let warnings = (0..MAX_DIAGNOSTICS + 10).map(|index| {
            Diagnostic::warning(
                "K5003",
                DiagnosticPhase::Semantic,
                format!("unused parameter {index}"),
                None,
            )
        });
        let error = Diagnostic::error(
            "K2002",
            DiagnosticPhase::Resolve,
            "unknown value in a later source",
            None,
        );
        let mixed = DiagnosticBundle::new(warnings.clone().chain([error]).collect());
        assert!(
            mixed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "K2002"),
            "warning fanout must never hide a real compiler error",
        );
        let warning_only = DiagnosticBundle::new(warnings.collect());
        let limit = warning_only
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K0004")
            .expect("warning fanout marker");
        assert_eq!(limit.severity, Severity::Warning);
        assert!(
            warning_only
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity == Severity::Warning),
        );
    }
}
