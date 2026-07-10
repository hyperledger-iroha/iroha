//! Structured diagnostics shared by the Kotodama compiler, CLI, and language tools.

use std::{error::Error as StdError, fmt};

use norito::json::{self, Value};

use crate::source::{SourceFile, TextRange};

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
    /// Name, type, effect, or policy analysis failed.
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

macro_rules! explanation {
    ($code:literal, $phase:ident, $summary:literal, $help:literal) => {
        DiagnosticExplanation {
            code: $code,
            phase: DiagnosticPhase::$phase,
            summary: $summary,
            help: $help,
        }
    };
}

/// Canonical diagnostic explanation registry used by `koto explain` and docs.
pub const DIAGNOSTIC_EXPLANATIONS: &[DiagnosticExplanation] = &[
    explanation!(
        "K0000",
        Lex,
        "the requested source could not be read",
        "Check that the path exists, is a regular readable file, and contains valid UTF-8."
    ),
    explanation!(
        "K0001",
        Lex,
        "source exceeds the 1 MiB V1 limit",
        "Split the source into typed modules and compile one deployable contract per file."
    ),
    explanation!(
        "K0002",
        Lex,
        "source exceeds the 250,000-token V1 limit",
        "Reduce generated source or move reusable declarations into typed modules."
    ),
    explanation!(
        "K0003",
        Parse,
        "source exceeds the 256-level nesting limit",
        "Flatten nested expressions, types, or blocks."
    ),
    explanation!(
        "K0004",
        Parse,
        "the diagnostic fanout limit was reached",
        "Fix the reported errors, then rerun the compiler to reveal any remaining failures."
    ),
    explanation!(
        "K0100",
        Lex,
        "the source contains an invalid token or character",
        "Use ASCII identifiers except for the exact 誓約, 言挙げ, 始まり, and 改善 keywords; the romanized forms are seiyaku, kotoage, hajimari, and kaizen."
    ),
    explanation!(
        "K1001",
        Parse,
        "the source does not match the V1 grammar",
        "Use the primary span and labels to repair the declaration or expression."
    ),
    explanation!(
        "K2001",
        Semantic,
        "a declaration duplicates or shadows another symbol",
        "Give every declaration and binding a unique, non-reserved name."
    ),
    explanation!(
        "K2002",
        Semantic,
        "a name, type, function, or import could not be resolved",
        "Correct the spelling or add an explicit typed-module import/export."
    ),
    explanation!(
        "K2003",
        Semantic,
        "an expression violates the strict type rules",
        "Use the exact declared type and an explicit conversion operation where one exists."
    ),
    explanation!(
        "K2004",
        Semantic,
        "authorization or view-effect policy was violated",
        "Authorize every kotoage function and keep view call graphs read-only."
    ),
    explanation!(
        "K2005",
        Semantic,
        "durable-state rules were violated",
        "Initialize scalar state in hajimari and access StateMap values through its typed API."
    ),
    explanation!(
        "K2006",
        Semantic,
        "a recursive call graph or recursive value type was found",
        "Refactor the cycle into an iterative, compiler-proven bounded design."
    ),
    explanation!(
        "K2007",
        Semantic,
        "a function exceeds the V1 argument-register word limit",
        "Reduce or regroup parameters so their recursively flattened representation occupies at most 13 words."
    ),
    explanation!(
        "K2099",
        Semantic,
        "semantic analysis rejected the program",
        "Follow the primary diagnostic and its labels; no artifact was emitted."
    ),
    explanation!(
        "K2100",
        Semantic,
        "the program violates the deterministic on-chain profile",
        "Remove test-only or non-deterministic behavior from the deployable contract."
    ),
    explanation!(
        "K3001",
        Lowering,
        "a typed construct has no V1 code-generation form",
        "Use a supported V1 type or operation; this is a compiler defect if the grammar promises the construct."
    ),
    explanation!(
        "K3003",
        Lowering,
        "typed HIR could not be lowered to SSA MIR",
        "Simplify the reported construct and report a compiler defect if valid V1 source triggered it."
    ),
    explanation!(
        "K3099",
        Lowering,
        "bytecode generation or assembly failed",
        "Report the diagnostic with a minimal source reproducer; valid V1 programs must assemble deterministically."
    ),
    explanation!(
        "K4001",
        Artifact,
        "compiler-owned artifact preparation failed",
        "Report a compiler defect; source cannot override ABI or execution-header policy."
    ),
    explanation!(
        "K4002",
        Artifact,
        "manifest or deployable artifact construction failed",
        "Report a compiler defect and do not deploy a partial output."
    ),
    explanation!(
        "K4003",
        Artifact,
        "a reusable module was requested as a deployable artifact",
        "Link the module into exactly one contract root and build the contract."
    ),
    explanation!(
        "K5001",
        Semantic,
        "durable state is declared but never used",
        "Remove the declaration or read/write it from a reachable function."
    ),
    explanation!(
        "K5002",
        Semantic,
        "a declaration shadows durable state",
        "Rename the declaration; strict V1 source should keep every binding unambiguous."
    ),
    explanation!(
        "K5003",
        Semantic,
        "a function parameter is never used",
        "Remove the parameter or use it in the function body."
    ),
    explanation!(
        "K5004",
        Semantic,
        "a statement is unreachable after return",
        "Delete the unreachable statement or move it before the terminating return."
    ),
    explanation!(
        "K5005",
        Semantic,
        "a pointer literal duplicates another literal",
        "Reuse one typed value instead of embedding duplicate pointer payloads."
    ),
    explanation!(
        "K5006",
        Semantic,
        "a constructed pointer value is never used",
        "Remove the constructor call or consume its typed result."
    ),
    explanation!(
        "K5007",
        Semantic,
        "a trigger specification is not statically transparent",
        "Use a canonical literal trigger specification so admission can derive its behavior."
    ),
    explanation!(
        "K5008",
        Semantic,
        "a state path is dynamic without complete access metadata",
        "Use a canonical StateMap operation or a literal state path when scheduler precision matters."
    ),
    explanation!(
        "K5009",
        Semantic,
        "an opaque operation prevents precise access derivation",
        "Prefer typed namespaced operations with derivable access; otherwise execution is serialized conservatively."
    ),
    explanation!(
        "K5099",
        Semantic,
        "a linter finding lacked a dedicated unified diagnostic code",
        "Report the finding so it can receive a stable K5000-series code and remediation."
    ),
    explanation!(
        "K9999",
        Artifact,
        "an unmigrated compiler error lacked a native stable code",
        "Report the error so the remaining adapter can be replaced with a phase-specific diagnostic."
    ),
    explanation!(
        "E_ACCESS_INCOMPLETE",
        Semantic,
        "the compiler could not prove a complete access set",
        "The runtime must serialize conservatively; remove dynamic or opaque access if parallel execution is required."
    ),
    explanation!(
        "E_UNBOUNDED_LOOP",
        Semantic,
        "a loop bound is not compiler-proven",
        "Use a static range or a collection operation whose 64-item cap is proven by the compiler."
    ),
    explanation!(
        "E_UNBOUNDED_ITERATION",
        Semantic,
        "collection iteration is not compiler-proven bounded",
        "Use a canonical bounded StateMap traversal with at most 64 items."
    ),
    explanation!(
        "E_INT_OVERFLOW",
        Semantic,
        "checked integer arithmetic overflowed",
        "Change the inputs or use an explicit wrapping operation when modular arithmetic is intentional."
    ),
    explanation!(
        "E_INTERNAL_BUILTIN",
        Semantic,
        "source attempted to call a compiler-internal raw capability",
        "Use the typed namespaced source API; allocation, pointers, raw syscalls, and opaque instructions are unavailable."
    ),
    explanation!(
        "E_TEST_ONLY_PRODUCTION",
        Semantic,
        "local test syntax or capabilities reached a production compilation",
        "Run the source through `koto test` or explicit CompilerMode::Test; keep deployable production sources free of #[test], fixture, koto_test, and test-only builtin calls."
    ),
    explanation!(
        "E_BREAK_OUTSIDE_LOOP",
        Semantic,
        "break appeared outside an accepted bounded loop",
        "Move break into the body of a compiler-proven bounded for loop."
    ),
    explanation!(
        "E_CONTINUE_OUTSIDE_LOOP",
        Semantic,
        "continue appeared outside an accepted bounded loop",
        "Move continue into the body of a compiler-proven bounded for loop."
    ),
    explanation!(
        "E_STATE_MAP_ALIAS",
        Semantic,
        "a StateMap was treated as an in-memory first-class value",
        "Access the declared StateMap directly through get, set, remove, or bounded iteration."
    ),
    explanation!(
        "E_STATE_MAP_OPTIONAL_READ",
        Semantic,
        "a StateMap read discarded the possibility that its key is absent",
        "Use map.get(key), handle the returned Option<V>, and use map[key] = value only for writes."
    ),
    explanation!(
        "E_STATE_SHADOWED",
        Semantic,
        "a local declaration shadowed durable state",
        "Rename the local declaration; V1 rejects all shadowing."
    ),
    explanation!(
        "E_STATE_INIT_REQUIRED",
        Semantic,
        "scalar durable state was declared without a hajimari/始まり hook",
        "Declare init and initialize every scalar state value before any normal return or fallthrough."
    ),
    explanation!(
        "E_STATE_INIT_INCOMPLETE",
        Semantic,
        "scalar durable state is not initialized on every normal init path",
        "Assign every reported scalar state on all branches and before every early return; loop-only and short-circuited writes are not definite."
    ),
    explanation!(
        "E_DUPLICATE_DECLARATION",
        Semantic,
        "a source-unit declaration name was repeated",
        "Rename or remove the repeated declaration; V1 has one unambiguous namespace per source unit."
    ),
    explanation!(
        "E_RESERVED_DECLARATION",
        Semantic,
        "a declaration collided with a reserved language or builtin name",
        "Choose an ASCII identifier that is not a V1 keyword, type, namespace, or builtin."
    ),
    explanation!(
        "E_IMMUTABLE_ASSIGNMENT",
        Semantic,
        "an immutable binding was assigned after declaration",
        "Declare a mutable local with `var`, or compute the final immutable value before binding it."
    ),
    explanation!(
        "E_ITERATION_LIMIT",
        Semantic,
        "a bounded collection operation exceeded the 64-item V1 limit",
        "Reduce the collection or split the operation into deterministic calls of at most 64 items."
    ),
    explanation!(
        "E_ITER_MUTATION",
        Semantic,
        "iteration attempted to mutate the collection being traversed",
        "Collect the intended changes first, then apply them after canonical iteration completes."
    ),
    explanation!(
        "E_MAP_BOUNDS",
        Semantic,
        "a StateMap operation lacked a compiler-proven deterministic bound",
        "Use direct keyed access or canonical StateMap iteration with the fixed 64-item cap."
    ),
    explanation!(
        "E_SECRET_REQUIRES_ZK",
        Semantic,
        "Secret<T> was used outside a ZK contract build",
        "Compile through a ZK-enabled project policy and keep the value inside approved proof or commitment operations."
    ),
    explanation!(
        "E_SECRET_CONTROL_FLOW",
        Semantic,
        "a secret value influenced public control flow",
        "Remove the secret-dependent branch or loop."
    ),
    explanation!(
        "E_SECRET_PUBLIC_RETURN",
        Semantic,
        "a secret value flowed into a public return",
        "Return only an approved proof, commitment, or public value independent of the secret."
    ),
    explanation!(
        "E_SECRET_STATE_KEY",
        Semantic,
        "a secret value flowed into a durable-state key",
        "Use a public canonical key independent of all private inputs."
    ),
    explanation!(
        "E_SECRET_STATE_WRITE",
        Semantic,
        "a secret value flowed directly into durable state",
        "Persist only an approved commitment or proof output."
    ),
    explanation!(
        "E_SECRET_LOG",
        Semantic,
        "a secret value flowed into a log",
        "Remove the log or log only a public value independent of the secret."
    ),
    explanation!(
        "E_SECRET_ARITHMETIC",
        Semantic,
        "secret data was used by an ordinary arithmetic operation",
        "Use only an approved proof or commitment operation whose secret-flow contract is explicit."
    ),
    explanation!(
        "E_SECRET_HOST_SINK",
        Semantic,
        "secret data reached a public host operation",
        "Pass only approved public proof or commitment outputs to ledger, context, state, and debug APIs."
    ),
    explanation!(
        "E_SECRET_MIXED_COMMITMENT",
        Semantic,
        "a commitment mixed secret data with an unapproved public value",
        "Use the exact approved commitment signature and explicitly public domain-separation inputs."
    ),
    explanation!(
        "E_SECRET_NULLIFIER_DISCLOSURE",
        Semantic,
        "secret nullifier material would be disclosed to a public sink",
        "Publish only the approved derived nullifier output, never its secret input material."
    ),
    explanation!(
        "E_SECRET_PAYLOAD_TYPE",
        Semantic,
        "a private input used a type outside the approved Secret<T> payload set",
        "Choose a supported fixed-layout secret payload type accepted by the ZK proof interface."
    ),
    explanation!(
        "E_SECRET_PRIVATE_INPUT_INDEX",
        Semantic,
        "a private-input index depended on secret or unbounded data",
        "Use a public compile-time-bounded private-input index."
    ),
    explanation!(
        "E_SECRET_PUBLIC_PARAMETER",
        Semantic,
        "a kotoage/view/hajimari/kaizen parameter was declared as Secret<T>",
        "Read private inputs through the ZK-only private-input API; public ABI records cannot contain secrets."
    ),
    explanation!(
        "E_SECRET_STATE_SINK",
        Semantic,
        "secret-tainted data reached durable state",
        "Persist only an approved public commitment or proof result independent of raw secret material."
    ),
    explanation!(
        "E_SECRET_STATE_TYPE",
        Semantic,
        "durable state was declared with a secret-bearing type",
        "Keep Secret<T> ephemeral and store only approved public commitments or proof outputs."
    ),
    explanation!(
        "E_SECRET_UNAPPROVED_OPERATION",
        Semantic,
        "secret data reached an operation without an approved flow specification",
        "Use an operation explicitly listed by the V1 secret-flow policy."
    ),
    explanation!(
        "E_SECRET_UNKNOWN_CALL",
        Semantic,
        "secret data was passed through a call whose flow behavior is unknown",
        "Pass secrets only to compiler-known proof and commitment functions with declared flow behavior."
    ),
];

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

    /// Adapt a legacy compiler error while the remaining phases migrate to native spans.
    pub fn from_legacy(
        phase: DiagnosticPhase,
        source: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        let code = message
            .split_once(':')
            .map(|(candidate, _)| candidate)
            .filter(|candidate| {
                candidate.len() >= 2
                    && matches!(candidate.as_bytes().first(), Some(b'E' | b'K'))
                    && candidate[1..].chars().all(|character| {
                        character.is_ascii_uppercase()
                            || character.is_ascii_digit()
                            || character == '_'
                    })
            })
            .unwrap_or("K9999")
            .to_owned();
        let primary_span = legacy_line_column(&message).map(|position| SourceSpan {
            source: source.map(ToOwned::to_owned),
            start: position,
            end: SourcePosition {
                line: position.line,
                column: position.column.saturating_add(1),
            },
            byte_range: None,
        });
        Self {
            code,
            severity: Severity::Error,
            phase,
            message,
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
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

fn source_span_to_json(span: &SourceSpan) -> Value {
    json_object(vec![
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

fn legacy_line_column(message: &str) -> Option<SourcePosition> {
    let (_, suffix) = message.rsplit_once(" at ")?;
    let location = suffix.lines().next()?;
    let (line, column) = location.split_once(':')?;
    Some(SourcePosition {
        line: line.parse().ok()?,
        column: column.parse().ok()?,
    })
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

    /// Build a one-error bundle from a legacy phase result.
    pub fn from_legacy(
        phase: DiagnosticPhase,
        source: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self::single(Diagnostic::from_legacy(phase, source, message))
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
                let source = span.source.as_deref().unwrap_or("<source>");
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
                let source = label.span.source.as_deref().unwrap_or("<source>");
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
                let source = fix.span.source.as_deref().unwrap_or("<source>");
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
    fn legacy_adapter_extracts_code_and_location() {
        let diagnostic = Diagnostic::from_legacy(
            DiagnosticPhase::Parse,
            Some("contract.ko"),
            "K1001: expected expression at 7:9\nsource",
        );
        assert_eq!(diagnostic.code, "K1001");
        let span = diagnostic.primary_span.expect("source span");
        assert_eq!(span.source.as_deref(), Some("contract.ko"));
        assert_eq!(span.start, SourcePosition { line: 7, column: 9 });
    }

    #[test]
    fn every_renderer_contains_the_same_canonical_fields() {
        let mut diagnostic = Diagnostic::from_legacy(
            DiagnosticPhase::Semantic,
            Some("contract.ko"),
            "K2001: unknown name at 3:5",
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
            "contract.ko:3:5",
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
            "contract.ko",
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
            human.contains("contract.ko:3:5-3:6"),
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
                    DiagnosticPhase::Semantic,
                    format!("unknown value {index}"),
                    Some(SourceSpan {
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
            DiagnosticPhase::Semantic,
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
