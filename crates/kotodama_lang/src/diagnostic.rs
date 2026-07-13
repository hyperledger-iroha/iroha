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
        "Split the source into typed modules and compile one deployable seiyaku per file."
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
        "K1004",
        Parse,
        "a source package or test graph exceeds a fixed V1 frontend budget",
        "Reduce the number or aggregate byte size of the source files in the graph."
    ),
    explanation!(
        "K1099",
        Parse,
        "the parser could not bind its internal source-origin table",
        "Report this compiler bug with the source file; compilation stops without producing typed HIR or bytecode."
    ),
    explanation!(
        "K2001",
        Semantic,
        "a declaration duplicates or shadows another symbol",
        "Give every declaration and binding a unique, non-reserved name."
    ),
    explanation!(
        "K2002",
        Resolve,
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
        "K2008",
        Semantic,
        "a named value type exceeds a fixed V1 resolution budget",
        "Shorten deeply nested value types or replace repeatedly branching product fields with a flatter bounded schema."
    ),
    explanation!(
        "K2098",
        Semantic,
        "an accepted parameter type retained no bounded ABI representation",
        "Report this compiler defect with the source; semantic analysis must assign every parameter an exact bounded V1 ABI representation."
    ),
    explanation!(
        "K2099",
        Resolve,
        "resolved-HIR construction or integrity validation failed",
        "Report this compiler defect with the source; resolution stopped before typed/effect analysis."
    ),
    explanation!(
        "K2100",
        Semantic,
        "the program violates the deterministic on-chain profile",
        "Remove test-only or non-deterministic behavior from the deployable seiyaku."
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
        "K3004",
        Lowering,
        "SSA optimization failed to preserve a valid deterministic control-flow graph",
        "Report this compiler defect with a minimal source reproducer; no artifact is emitted after an optimizer failure."
    ),
    explanation!(
        "K3005",
        Lowering,
        "lowered IR could not be converted into validated SSA form",
        "Report this compiler defect with a minimal source reproducer; valid typed HIR must always produce validated SSA."
    ),
    explanation!(
        "K3006",
        Lowering,
        "optimized SSA could not be converted back into validated lowering IR",
        "Report this compiler defect with a minimal source reproducer; no bytecode is emitted from an invalid optimized program."
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
        "Link the module into exactly one seiyaku root and build that seiyaku."
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
        "E_PACKAGE_BUDGET",
        Parse,
        "a typed-module package graph exceeds a fixed V1 frontend budget",
        "Reduce the package source count or aggregate source bytes before linking."
    ),
    explanation!(
        "E_ROOT_MUST_BE_SEIYAKU",
        Resolve,
        "the deployable package root is not a seiyaku/誓約 source unit",
        "Declare exactly one seiyaku/誓約 in the root and keep reusable dependencies as modules."
    ),
    explanation!(
        "E_DEPENDENCY_MUST_BE_MODULE",
        Resolve,
        "a reusable dependency is not a module source unit",
        "Change the dependency to exactly one module declaration; only the package root may be deployable."
    ),
    explanation!(
        "E_DUPLICATE_PACKAGE",
        Resolve,
        "the locked graph contains the same package identity more than once",
        "Keep one canonical record for each package identity in the locked dependency graph."
    ),
    explanation!(
        "E_EMPTY_PACKAGE",
        Resolve,
        "a locked package contains no Kotodama modules",
        "Add at least one module source or remove the empty package from the locked graph."
    ),
    explanation!(
        "E_DUPLICATE_MODULE",
        Resolve,
        "one package declares the same module name more than once",
        "Give every module in the package a unique canonical name."
    ),
    explanation!(
        "E_DUPLICATE_IMPORT",
        Resolve,
        "an import alias is repeated in one scope",
        "Keep one explicit package binding for each import alias."
    ),
    explanation!(
        "E_RESERVED_IMPORT",
        Resolve,
        "an import alias collides with a compiler-owned capability namespace",
        "Choose a non-reserved explicit alias for the imported package."
    ),
    explanation!(
        "E_UNKNOWN_PACKAGE",
        Resolve,
        "an import names a package absent from the locked graph",
        "Add the exact package identity to the lock graph or correct the import."
    ),
    explanation!(
        "E_PACKAGE_IMPORT_CYCLE",
        Resolve,
        "locked package imports form a dependency cycle",
        "Break the cycle and keep the typed-module package graph acyclic."
    ),
    explanation!(
        "E_UNKNOWN_IMPORT_ALIAS",
        Resolve,
        "a qualified call uses an alias not imported by its source package",
        "Add one explicit import binding for the alias or correct the qualified call."
    ),
    explanation!(
        "E_MULTIPLE_SEIYAKU_ROOTS",
        Resolve,
        "one diagnostics request contains more than one deployable seiyaku root",
        "Keep one project root; check unrelated seiyaku roots in separate requests."
    ),
    explanation!(
        "E_PROJECT_MANIFEST_REQUIRED",
        Resolve,
        "positional source paths cannot grant module linking authority",
        "Pass --project with a versioned manifest that declares exact imports, locked packages, modules, and exports."
    ),
    explanation!(
        "E_PROJECT_MANIFEST",
        Resolve,
        "an explicit Kotodama project manifest is malformed or unsafe",
        "Use version 1 canonical Norito JSON with relative in-project source paths and complete explicit graph fields."
    ),
    explanation!(
        "E_UNEXPORTED_SYMBOL",
        Resolve,
        "a qualified call targets a function absent from the package export table",
        "Export the exact function from one module or call a declared exported symbol."
    ),
    explanation!(
        "E_MISSING_EXPORT",
        Resolve,
        "a package export names no module function",
        "Define the exported function in exactly one module or remove the stale export."
    ),
    explanation!(
        "E_AMBIGUOUS_EXPORT",
        Resolve,
        "multiple modules define the same declared package export",
        "Keep exactly one module definition for each exported function."
    ),
    explanation!(
        "E_WILDCARD_IMPORT",
        Resolve,
        "a module graph requests a wildcard import",
        "Replace the wildcard with explicit package aliases and exported function names."
    ),
    explanation!(
        "E_INVALID_IDENTIFIER",
        Resolve,
        "a package, module, alias, or symbol name is not a strict V1 identifier",
        "Use a valid unambiguous Kotodama V1 identifier in the reported graph location."
    ),
    explanation!(
        "E_INVALID_MODULE_ITEM",
        Resolve,
        "a reusable module contains a deployable-only declaration",
        "Move state, triggers, `kotoage`/`言挙げ`, `view fn`, `hajimari`/`始まり`, or `kaizen`/`改善` declarations into the seiyaku root."
    ),
    explanation!(
        "E_DUPLICATE_ERROR_CODE",
        Resolve,
        "linked modules assign the same stable seiyaku error code more than once",
        "Assign a unique stable numeric code to every linked error variant."
    ),
    explanation!(
        "E_DUPLICATE_MESSAGE",
        Resolve,
        "linked modules define the same localization message key more than once",
        "Keep one canonical message for each localization key in the linked graph."
    ),
    explanation!(
        "E_INVALID_SOURCE_PATH",
        Resolve,
        "a logical module source path is non-canonical or escapes its package",
        "Use a normalized relative logical path contained by the package source root."
    ),
    explanation!(
        "E_DUPLICATE_SOURCE",
        Resolve,
        "a package graph contains duplicate normalized logical source paths",
        "Remove the duplicate or give each source a unique normalized logical path."
    ),
    explanation!(
        "E_EMPTY_PACKAGE_GRAPH",
        Resolve,
        "typed linking produced no root or reusable module program",
        "Provide one seiyaku root and every explicitly imported non-empty module package."
    ),
    explanation!(
        "E_DUPLICATE_HIR_ID",
        Resolve,
        "linked typed modules reused a compiler-owned HIR identity",
        "Report this compiler defect with the complete source graph; source cannot assign HIR identities."
    ),
    explanation!(
        "E_DUPLICATE_SOURCE_ID",
        Resolve,
        "different logical sources received the same compiler-owned source identity",
        "Report this compiler defect with the complete normalized source graph."
    ),
    explanation!(
        "E_TEST_TARGET_MISMATCH",
        Resolve,
        "a standalone test module resolves to a different target source",
        "Correct koto_test.target so its normalized path names the graph's exact seiyaku target."
    ),
    explanation!(
        "E_LOCAL_SHADOWING",
        Resolve,
        "a local binding duplicates or shadows another visible symbol",
        "Rename the binding so every local and source-unit declaration remains unambiguous."
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
        "E_INT_LITERAL_OVERFLOW",
        Parse,
        "an integer literal is outside the signed 512-bit domain",
        "Use a value from -2^511 through 2^511 - 1."
    ),
    explanation!(
        "E_RETIRED_NUMERIC_SUFFIX",
        Lex,
        "a numeric literal uses a retired suffix",
        "Use an unsuffixed literal in an explicit int, decimal, or quantity context; Kotodama V1 has no numeric literal suffixes."
    ),
    explanation!(
        "E_RETIRED_NUMERIC_HELPER",
        Parse,
        "source calls a retired width-specific or generic numeric helper",
        "Use exact operators, named int/decimal/quantity conversions, or native `json { ... }` construction."
    ),
    explanation!(
        "E_DECIMAL_MALFORMED",
        Lex,
        "an exact decimal literal has invalid spelling",
        "Use base-10 digits, at most one decimal point, an optional decimal exponent, and underscores only between digits."
    ),
    explanation!(
        "E_DECIMAL_EXPONENT",
        Lex,
        "an exact decimal exponent has invalid spelling",
        "Write at least one base-10 digit after `e` or `E`; a leading exponent sign is allowed."
    ),
    explanation!(
        "E_RETIRED_NUMERIC_TYPE",
        Parse,
        "a declaration uses a retired numeric type name",
        "Use `int`, `decimal`, or the nominal non-negative `quantity` type."
    ),
    explanation!(
        "E_RETIRED_DECLARATION_ORDER",
        Parse,
        "a typed declaration uses the retired name-colon-type order",
        "Place the type before the name, for example `const int limit`, `state int value`, `let int count`, or `fn add(int lhs)`."
    ),
    explanation!(
        "E_NEGATIVE_QUANTITY",
        Semantic,
        "a value converted to quantity is negative",
        "Use decimal when the value may be negative, or prove the value non-negative before an explicit quantity conversion."
    ),
    explanation!(
        "E_UNSHIELD_AMOUNT_RANGE",
        Semantic,
        "an unshield public amount is outside its protocol field domain",
        "Use a whole-unit quantity with canonical scale 0 and value no greater than 2^128 - 1. This narrower bound belongs only to the V1 unshield proof scalar."
    ),
    explanation!(
        "E_QUORUM_RANGE",
        Semantic,
        "an account quorum is outside the protocol's nonzero 16-bit domain",
        "Use an int from 1 through 65535. This is a protocol-field bound, not the range of Kotodama int."
    ),
    explanation!(
        "E_DECIMAL_SCALE_OVERFLOW",
        Semantic,
        "an exact decimal value exceeds the 28-digit canonical scale limit",
        "Reduce the significant fractional precision to at most 28 digits. Trailing fractional zeros are canonicalized away before this limit is checked."
    ),
    explanation!(
        "E_DECIMAL_MANTISSA_OVERFLOW",
        Semantic,
        "an exact decimal value exceeds the signed 512-bit mantissa limit",
        "Reduce the magnitude so the unscaled canonical mantissa fits the signed 512-bit domain."
    ),
    explanation!(
        "E_QUANTITY_REMAINDER",
        Semantic,
        "quantity does not define a remainder operation",
        "Use exact division, quantity.div_round, or quantity.ratio_round with an explicit scale and rounding mode."
    ),
    explanation!(
        "E_QUANTITY_UNDERFLOW",
        Semantic,
        "quantity subtraction would produce a negative result",
        "Change the operands, or convert to decimal explicitly when a negative result is part of the domain."
    ),
    explanation!(
        "E_QUANTITY_NEGATION",
        Semantic,
        "the nominal `quantity` type cannot be negated",
        "Use `int` or `decimal` for signed values."
    ),
    explanation!(
        "E_DIVISION_BY_ZERO",
        Semantic,
        "a numeric divisor is zero",
        "Use a nonzero divisor or validate it before the operation."
    ),
    explanation!(
        "E_REPEATING_DECIMAL",
        Semantic,
        "exact decimal division has a nonterminating result",
        "Use `div_round` with an explicit output scale and rounding mode when approximation is intentional."
    ),
    explanation!(
        "E_EXACT_DIVISION_SCALE_OVERFLOW",
        Semantic,
        "an exact terminating quotient needs more than 28 fractional digits",
        "Use `div_round` with an explicit representable scale, or change the operands."
    ),
    explanation!(
        "E_INEXACT_CONVERSION",
        Semantic,
        "an exact numeric conversion would discard a fractional part",
        "Use an explicitly truncating or rounded conversion when loss of precision is intentional."
    ),
    explanation!(
        "E_IMPLICIT_NUMERIC_CONVERSION",
        Semantic,
        "runtime int and decimal values were mixed without a named conversion",
        "Convert the int with decimal::from_int(value) before decimal arithmetic or comparison. Exact numeric literals may still infer their type contextually at compile time."
    ),
    explanation!(
        "E_NUMERIC_ROUND_ARITY",
        Semantic,
        "an explicitly rounded numeric operation has the wrong number of arguments",
        "Pass every declared argument with its canonical name; rounded division requires divisor, scale, and mode."
    ),
    explanation!(
        "E_NUMERIC_ROUND_RECEIVER",
        Semantic,
        "an explicitly rounded numeric method was called on an unsupported receiver",
        "Use decimal.div_round, quantity.div_round, or quantity.ratio_round with the operand types declared by the V1 numeric matrix."
    ),
    explanation!(
        "E_INVALID_SCALE",
        Semantic,
        "an exact numeric operation received a scale outside zero through 28",
        "Choose an int scale from 0 through 28."
    ),
    explanation!(
        "E_NUMERIC_ROUNDING_MODE",
        Semantic,
        "an explicitly rounded numeric operation received an unknown rounding mode",
        "Use one of the seven V1 modes: toward_zero, away_from_zero, floor, ceil, nearest_even, nearest_away, or nearest_toward_zero."
    ),
    explanation!(
        "E_LEGACY_SUM_CONSTRUCTOR",
        Parse,
        "source uses a retired placeholder-based Option or Result constructor",
        "Use Option::some(value), contextual Option::none, Result::ok(value), or Result::err(value)."
    ),
    explanation!(
        "E_SUM_CONSTRUCTOR_FORM",
        Parse,
        "an active-only sum constructor has invalid source form",
        "Write Option::none without parentheses and give active constructors exactly one positional payload."
    ),
    explanation!(
        "E_SUM_CONSTRUCTOR_ARITY",
        Parse,
        "an active-only sum constructor has the wrong payload count",
        "Supply exactly one active payload to Option::some, Result::ok, or Result::err."
    ),
    explanation!(
        "E_SUM_MISSING_CONTEXT",
        Semantic,
        "an inactive or partially inferred sum value lacks an exact type context",
        "Add an Option<T> or Result<T, E> annotation so every generic payload type is known."
    ),
    explanation!(
        "E_SUM_CONTEXT_TYPE",
        Semantic,
        "a sum constructor conflicts with its contextual type",
        "Use the matching Option or Result constructor and an exactly assignable active payload."
    ),
    explanation!(
        "E_PROPAGATE_CONTEXT",
        Semantic,
        "postfix propagation is used outside a matching sum-returning function",
        "Return the same Option or Result family from the enclosing function, or handle the value explicitly."
    ),
    explanation!(
        "E_PROPAGATE_ERROR_TYPE",
        Semantic,
        "Result propagation would require implicit error conversion",
        "Use the exact enclosing Result error type or convert the error explicitly before propagation."
    ),
    explanation!(
        "E_PROPAGATE_TYPE",
        Semantic,
        "postfix propagation was applied to a non-sum value",
        "Apply ? only to Option<T> or Result<T, E>."
    ),
    explanation!(
        "E_IF_EXPRESSION_ELSE",
        Semantic,
        "an expression-valued if omits its else branch",
        "Add an else block whose tail has exactly the same type as the then block tail."
    ),
    explanation!(
        "E_IF_LET_EXPRESSION_ELSE",
        Semantic,
        "an expression-valued if let omits its else branch",
        "Add an else block, or use if let as a statement when no value is needed."
    ),
    explanation!(
        "E_BRANCH_TYPE_MISMATCH",
        Semantic,
        "expression branches have different types",
        "Make every if or match branch tail produce exactly the same type."
    ),
    explanation!(
        "E_DIVERGING_EXPRESSION_CONTEXT",
        Semantic,
        "an expression whose branches all diverge lacks an exact result type",
        "Provide an exact contextual type, or use the construct as a statement when every branch returns from the enclosing function."
    ),
    explanation!(
        "E_PATTERN_FAMILY",
        Semantic,
        "a namespaced pattern belongs to the wrong sum family",
        "Match Option values with Option::some/none and Result values with Result::ok/err."
    ),
    explanation!(
        "E_PATTERN_PAYLOAD",
        Semantic,
        "a sum pattern binds a payload on an inactive variant or omits an active payload binding",
        "Bind or explicitly ignore active payloads; write Option::none without a payload."
    ),
    explanation!(
        "E_PATTERN_TYPE",
        Semantic,
        "a sum pattern was applied to a non-Option/non-Result value",
        "Use namespaced sum patterns only with Option<T> or Result<T, E>."
    ),
    explanation!(
        "E_MATCH_EMPTY",
        Semantic,
        "a match expression has no arms",
        "Add the complete namespaced pattern set for the matched Option or Result."
    ),
    explanation!(
        "E_MATCH_DUPLICATE_PATTERN",
        Semantic,
        "a match expression repeats a variant pattern",
        "Keep exactly one arm for each variant in the matched sum family."
    ),
    explanation!(
        "E_MATCH_NON_EXHAUSTIVE",
        Semantic,
        "a sum match omits an active or inactive variant",
        "Cover both Option variants or both Result variants explicitly."
    ),
    explanation!(
        "E_MATCH_TYPE_CONTEXT",
        Semantic,
        "match arm types cannot be inferred consistently",
        "Add a result type context or make every arm tail have exactly the same type."
    ),
    explanation!(
        "E_QUERY_KEY_TYPE",
        Semantic,
        "a typed core query received bytes or the wrong identifier type",
        "Pass the exact AccountId, AssetId, AssetDefinitionId, DomainId, or NftId required by the query."
    ),
    explanation!(
        "E_QUERY_PAGE_ARGUMENTS",
        Semantic,
        "a typed query page has invalid offset or limit arguments",
        "Pass named offset: int and limit: int arguments."
    ),
    explanation!(
        "E_QUERY_RESULT_TYPE",
        Semantic,
        "a typed core-query projection was assigned to an incompatible result type",
        "Receive singular queries as Option<View> and plural queries as QueryPage<View>; raw byte compatibility is not part of the five V1 core-query families."
    ),
    explanation!(
        "E_QUERY_OFFSET",
        Semantic,
        "a typed query page offset is negative",
        "Use a non-negative canonical-order offset."
    ),
    explanation!(
        "E_QUERY_LIMIT",
        Semantic,
        "a typed query page limit is outside 1 through 64",
        "Choose a page limit from 1 through 64."
    ),
    explanation!(
        "E_LIST_TYPE_ARITY",
        Semantic,
        "List has an invalid number of type arguments",
        "Declare List<T, N> with one element type and one capacity constant."
    ),
    explanation!(
        "E_LIST_CAPACITY_CONST",
        Semantic,
        "a List capacity is not a compile-time integer",
        "Use an integer capacity constant from 1 through 64."
    ),
    explanation!(
        "E_LIST_CAPACITY",
        Semantic,
        "a List capacity is outside 1 through 64",
        "Choose a compile-time capacity from 1 through 64."
    ),
    explanation!(
        "E_LIST_RESOURCE_ELEMENT",
        Semantic,
        "a List element contains a resource handle",
        "Keep StateMap, Secret, and other resource handles outside List elements."
    ),
    explanation!(
        "E_LIST_ZERO_SIZED_ELEMENT",
        Semantic,
        "a List element schema encodes to zero runtime words",
        "Add at least one runtime-valued field; every List element must encode at least one word."
    ),
    explanation!(
        "E_LIST_EMPTY_CONTEXT",
        Semantic,
        "an empty list literal has no element or capacity context",
        "Add a List<T, N> annotation to the binding, argument, or return position."
    ),
    explanation!(
        "E_LIST_LITERAL_CAPACITY",
        Semantic,
        "a list literal exceeds its contextual capacity",
        "Increase the declared capacity up to 64 or reduce the literal element count."
    ),
    explanation!(
        "E_LIST_CONTEXT_TYPE",
        Semantic,
        "a list expression conflicts with its contextual type",
        "Use a matching List<T, N> context and exactly assignable element values."
    ),
    explanation!(
        "E_LIST_COMPREHENSION_SOURCE",
        Semantic,
        "a list comprehension source is not a bounded List",
        "Iterate a List<T, N> so the compiler can prove the maximum result size."
    ),
    explanation!(
        "E_LIST_COMPREHENSION_FILTER",
        Semantic,
        "a list comprehension filter is not boolean",
        "Use a bool expression after if; filters do not reduce the proven capacity."
    ),
    explanation!(
        "E_LIST_COMPREHENSION_CAPACITY",
        Semantic,
        "a list comprehension may exceed its contextual or V1 capacity",
        "Reduce the source capacity or use a context large enough for the full proven maximum, up to 64."
    ),
    explanation!(
        "E_LIST_UNSAFE_INDEX",
        Semantic,
        "source attempted an unchecked List index operation",
        "Use get(index) for reads and try_set(index, value) for writes."
    ),
    explanation!(
        "E_LIST_INDEX_TYPE",
        Semantic,
        "a List index has the wrong type",
        "Use an int index with get or try_set."
    ),
    explanation!(
        "E_LIST_METHOD_ARITY",
        Semantic,
        "a bounded List method has the wrong argument count",
        "Use the declared method signature and account for the implicit receiver."
    ),
    explanation!(
        "E_LIST_CONTAINS_COMPARABILITY",
        Semantic,
        "List.contains requires canonically comparable elements",
        "Use an element type with deterministic structural equality; resource handles and other non-comparable values cannot be searched with contains."
    ),
    explanation!(
        "E_LIST_MUTABLE_RECEIVER",
        Semantic,
        "a mutating List method was called on a non-mutable receiver",
        "Bind the List with var before calling try_set, try_push, or pop."
    ),
    explanation!(
        "E_LIST_TAKE_CONST",
        Semantic,
        "List.take received a dynamic limit",
        "Pass a compile-time integer limit so the result capacity remains proven."
    ),
    explanation!(
        "E_LIST_TAKE_LIMIT",
        Semantic,
        "List.take received a limit outside its source capacity",
        "Use a constant limit from 0 through the source List capacity."
    ),
    explanation!(
        "E_JSON_CAPACITY",
        Semantic,
        "a native JSON object or array exceeds the per-node V1 bound",
        "Use at most 64 object entries or array elements at each JSON node; split larger data into bounded nested nodes."
    ),
    explanation!(
        "E_JSON_DUPLICATE_KEY",
        Semantic,
        "a native JSON object repeats a decoded key",
        "Keep each decoded key exactly once; identifier and quoted spellings of the same key still collide."
    ),
    explanation!(
        "E_JSON_SCHEMA_LIMIT",
        Semantic,
        "a native JSON construction exceeds a recursive V1 ABI bound",
        "Reduce recursive object, array, Option, or List structure so the canonical construction schema fits its node, word, depth, and encoded-byte limits."
    ),
    explanation!(
        "E_JSON_VALUE_TYPE",
        Semantic,
        "a value cannot be converted by native JSON construction",
        "Handle Result and arbitrary structs explicitly, and keep StateMap, Secret, and other resource handles outside JSON."
    ),
    explanation!(
        "E_LEGACY_JSON_GETTER",
        Parse,
        "source uses a retired name for the quantity JSON getter",
        "Use value.get_quantity(key) or json::get_quantity(value, key); every typed JSON getter returns Option<T>."
    ),
    explanation!(
        "E_MIXED_CALL_ARGUMENTS",
        Parse,
        "a call mixes positional and named arguments",
        "Use the contextual named-argument fix when offered. If the callee is unresolved, add the declared parameter names manually; the compiler does not guess a parameter mapping. Method receivers are implicit."
    ),
    explanation!(
        "E_DUPLICATE_NAMED_ARGUMENT",
        Parse,
        "a named call argument is repeated",
        "Supply each declared parameter exactly once."
    ),
    explanation!(
        "E_UNKNOWN_NAMED_ARGUMENT",
        Semantic,
        "a named argument does not match a declared parameter",
        "Use one of the callee's declared parameter names."
    ),
    explanation!(
        "E_MISSING_NAMED_ARGUMENT",
        Semantic,
        "a named call omits a required parameter",
        "Supply every required parameter by its declared name."
    ),
    explanation!(
        "E_NAMED_ARGUMENTS_REQUIRED",
        Semantic,
        "a safety-sensitive call uses positional arguments",
        "Name every source argument to make pagination, effects, or repeated parameter types unambiguous."
    ),
    explanation!(
        "E_POSITIONAL_STRUCT",
        Semantic,
        "a struct uses retired positional construction",
        "Use `Type { field: value, shorthand_field }`; apply the diagnostic fix when one is available."
    ),
    explanation!(
        "E_DUPLICATE_STRUCT_FIELD",
        Parse,
        "a struct literal field is repeated",
        "Supply each declared struct field exactly once."
    ),
    explanation!(
        "E_UNKNOWN_STRUCT_FIELD",
        Semantic,
        "a struct literal names an undeclared field",
        "Use only fields declared by the struct type."
    ),
    explanation!(
        "E_MISSING_STRUCT_FIELD",
        Semantic,
        "a struct literal omits a declared field",
        "Supply every declared field; field order in source is unrestricted."
    ),
    explanation!(
        "E_UNKNOWN_STRUCT",
        Semantic,
        "a struct literal refers to an unknown type",
        "Declare the struct in this source unit or import its typed module declaration."
    ),
    explanation!(
        "E_INTERNAL_BUILTIN",
        Semantic,
        "source attempted to call a compiler-internal raw capability",
        "Use the typed namespaced source API; allocation, pointers, raw syscalls, and opaque instructions are unavailable."
    ),
    explanation!(
        "E_INTERNAL_RESOLUTION",
        Resolve,
        "typed analysis received inconsistent or incomplete resolved-HIR metadata",
        "Report this compiler defect with the source; resolution must assign stable identities and targets before typed analysis."
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
        "E_STATE_HAJIMARI_REQUIRED",
        Semantic,
        "scalar durable state was declared without a hajimari/始まり hook",
        "Declare hajimari/始まり and initialize every scalar state value before any normal return or fallthrough."
    ),
    explanation!(
        "E_STATE_HAJIMARI_INCOMPLETE",
        Semantic,
        "scalar durable state is not initialized on every normal `hajimari`/`始まり` path",
        "Assign every reported scalar state on all branches and before every early return; loop-only and short-circuited writes are not definite."
    ),
    explanation!(
        "E_DUPLICATE_DECLARATION",
        Resolve,
        "a source-unit declaration name was repeated",
        "Rename or remove the repeated declaration; V1 has one unambiguous namespace per source unit."
    ),
    explanation!(
        "E_RESERVED_DECLARATION",
        Resolve,
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
        "Secret<T> was used outside a ZK seiyaku build",
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
    explanation!(
        "E_CONST_CAPACITY_CONTEXT",
        Semantic,
        "a compile-time integer appeared outside a bounded List capacity",
        "Use the compile-time integer only as the second argument of List<T, N>."
    ),
    explanation!(
        "E_CONST_INITIALIZER",
        Semantic,
        "a const initializer depended on runtime evaluation",
        "Initialize the const from literals and previously declared constants only."
    ),
    explanation!(
        "E_FOR_INITIALIZER",
        Semantic,
        "a bounded for-loop initializer lowered to more than one operation",
        "Use one simple let binding or expression as the for-loop initializer."
    ),
    explanation!(
        "E_FOR_STEP",
        Semantic,
        "a bounded for-loop step lowered to more than one operation",
        "Use one simple let binding or expression as the for-loop step."
    ),
    explanation!(
        "E_INTERNAL_NUMERIC_MATRIX",
        Semantic,
        "typed numeric operands violated the compiler's canonical operator matrix",
        "Report this compiler defect with a minimal source reproducer; no artifact is emitted from inconsistent typed HIR."
    ),
    explanation!(
        "E_INTRINSIC_CONTEXT",
        Semantic,
        "a compiler intrinsic was used outside its declared operation context",
        "Use the public namespaced operation that owns the intrinsic instead of referring to the compiler-owned value."
    ),
    explanation!(
        "E_INVALID_ASSIGNMENT_TARGET",
        Semantic,
        "an assignment targeted a non-assignable expression",
        "Assign only to a mutable var binding or through the typed StateMap mutation API."
    ),
    explanation!(
        "E_LIFECYCLE_AUTHORIZATION",
        Semantic,
        "a hajimari or kaizen declaration attempted to set caller authorization",
        "Remove authorize from hajimari and kaizen; lifecycle authorization is defined by the runtime."
    ),
    explanation!(
        "E_MALFORMED_CALL",
        Semantic,
        "resolved call metadata was internally inconsistent",
        "Report this compiler defect with a minimal source reproducer; malformed resolved calls never produce artifacts."
    ),
    explanation!(
        "E_MISSING_RETURN",
        Semantic,
        "a value-returning function does not return on every path",
        "Return the declared type on every reachable path or use one final block-tail expression."
    ),
    explanation!(
        "E_NON_CANONICAL_BUILTIN",
        Semantic,
        "source used a retired, flat, or compiler-internal builtin spelling",
        "Use the canonical V1 namespaced builtin shown by the diagnostic."
    ),
    explanation!(
        "E_NON_CANONICAL_NUMERIC",
        Semantic,
        "a numeric value was not in canonical int, decimal, or quantity form",
        "Construct the value with the exact canonical numeric type and representation."
    ),
    explanation!(
        "E_QUERY_PAGE_VIEW",
        Semantic,
        "QueryPage was instantiated with a view outside the declared core query set",
        "Use QueryPage only with a canonical core query view supported by ABI V1."
    ),
    explanation!(
        "E_RETIRED_VRF_VERIFY_ARGS",
        Semantic,
        "source used the retired multi-argument VRF verification form",
        "Encode one VrfVerifyRequest as bytes and pass it through the named request argument."
    ),
    explanation!(
        "E_RETURN_TYPE_MISMATCH",
        Semantic,
        "a returned value does not exactly match the declared return type",
        "Return the exact declared type; use an explicit conversion where the V1 API provides one."
    ),
    explanation!(
        "E_STATE_MAP_KEY_TYPE",
        Semantic,
        "a StateMap key lacks a canonical deterministic Norito representation",
        "Choose a supported scalar canonical-Norito key type."
    ),
    explanation!(
        "E_TAIL_TYPE_MISMATCH",
        Semantic,
        "a block-tail expression does not exactly match its required type",
        "Make the final expression the exact required type or terminate it with a semicolon when the block returns unit."
    ),
    explanation!(
        "E_TUPLE_INDEX",
        Semantic,
        "a tuple member index is outside the tuple's declared fields",
        "Use a zero-based tuple index smaller than the tuple length."
    ),
    explanation!(
        "E_TYPE_ANNOTATION_MISMATCH",
        Semantic,
        "an explicitly typed binding received a value of another type",
        "Keep type-first annotations exact and use only explicit V1 conversions."
    ),
    explanation!(
        "E_ZK_MODE_REQUIRED",
        Semantic,
        "a ZK-only operation was retained without ZK compilation policy",
        "Build the seiyaku with explicit ZK project policy or remove the ZK-only operation."
    ),
    explanation!(
        "E_SECRET_FULL_WIDTH_CRYPTO_REQUIRED",
        Semantic,
        "a scalar-register crypto operation attempted to consume Secret<T>",
        "Use crypto::valcom or another approved full-width proof operation that declares a Secret<T> flow."
    ),
    explanation!(
        "E_SECRET_PRIVATE_INPUT_AMBIGUOUS",
        Semantic,
        "a private input had no inferable Secret<T> payload type",
        "Use a type-first binding such as `let Secret<int> value = crypto::private_input(0);`."
    ),
    explanation!(
        "E_SECRET_PRIVATE_INPUT_CONTEXT",
        Semantic,
        "a private-input operation did not initialize an approved Secret<T> binding",
        "Initialize an explicit Secret<int>, Secret<decimal>, or Secret<quantity> binding."
    ),
    explanation!(
        "E_TEST_ACTOR_LITERAL",
        Semantic,
        "a Kotodama test actor was not a static literal alias",
        "Pass a string literal or Name::parse literal actor alias to the test helper."
    ),
    explanation!(
        "E_TEST_BUILTIN_CONTEXT",
        Semantic,
        "a test-only builtin was called outside a #[test] function",
        "Move the call into a local #[test] fn in a standalone Kotodama test module."
    ),
    explanation!(
        "E_TEST_ENTRYPOINT_KIND",
        Semantic,
        "a runtime test helper targeted a private function",
        "Target a kotoage, view, hajimari, or kaizen declaration."
    ),
    explanation!(
        "E_TEST_ENTRYPOINT_LITERAL",
        Semantic,
        "a runtime test helper target was not a static literal",
        "Pass the kotoage, view, hajimari, or kaizen name as a string or Name::parse literal."
    ),
    explanation!(
        "E_TEST_FUNCTION_SIGNATURE",
        Semantic,
        "a #[test] function has a public, parameterized, or value-returning signature",
        "Declare each test as a local parameterless `#[test] fn` returning unit."
    ),
    explanation!(
        "E_TEST_MODULE_ITEM",
        Semantic,
        "a standalone Kotodama test module contains a deployable-only item",
        "Keep standalone test modules limited to their target, fixtures, helpers, and #[test] functions."
    ),
    explanation!(
        "E_TEST_MODULE_KIND",
        Semantic,
        "a standalone Kotodama test file did not declare a module",
        "Declare a module source unit and identify the tested seiyaku with koto_test."
    ),
    explanation!(
        "E_TEST_TARGET_REQUIRED",
        Semantic,
        "a standalone Kotodama test module omitted its tested seiyaku",
        "Add one koto_test declaration naming the seiyaku under test."
    ),
    explanation!(
        "E_TRIGGER_FILTER_DUPLICATE_MATCHER",
        Semantic,
        "a trigger data filter repeats one matcher key",
        "Keep at most one matcher for each supported key in the trigger filter."
    ),
    explanation!(
        "E_TRIGGER_FILTER_INVALID_LITERAL",
        Semantic,
        "a trigger data-filter matcher contains an invalid typed identifier literal",
        "Replace the matcher value with a canonical literal of the identifier type named by the diagnostic."
    ),
    explanation!(
        "E_TRIGGER_FILTER_UNSUPPORTED_EVENT",
        Semantic,
        "a trigger data family does not support the requested event kind",
        "Choose one event kind supported by that data family or omit the event restriction."
    ),
    explanation!(
        "E_TRIGGER_FILTER_UNSUPPORTED_MATCHER",
        Semantic,
        "a trigger data family does not support the requested matcher",
        "Remove the matcher or use a matcher supported by the selected data family."
    ),
    explanation!(
        "E_TRIGGER_INVALID_AUTHORITY",
        Semantic,
        "a trigger authority is not a canonical AccountId",
        "Use a canonical domainless AccountId for trigger authority."
    ),
    explanation!(
        "E_TRIGGER_INVALID_ID",
        Semantic,
        "an execute-trigger target is not a canonical TriggerId name",
        "Use a valid canonical Name for the referenced trigger id."
    ),
    explanation!(
        "E_TRIGGER_INVALID_METADATA_KEY",
        Semantic,
        "a trigger metadata key is not a canonical Name",
        "Replace the metadata key with a valid canonical Name."
    ),
    explanation!(
        "E_TRIGGER_INVALID_NAME",
        Semantic,
        "a trigger declaration name is not canonical",
        "Rename the trigger using a valid canonical Name."
    ),
    explanation!(
        "E_TRIGGER_METADATA_VALUE",
        Semantic,
        "a trigger metadata expression cannot be represented as canonical JSON",
        "Use an exactly representable literal or explicit native Json construction."
    ),
    explanation!(
        "E_TRIGGER_SCHEDULE_PERIOD",
        Semantic,
        "a scheduled trigger declares a zero repeat period",
        "Use a non-zero deterministic period_ms value."
    ),
    explanation!(
        "E_TRIGGER_TARGET_KIND",
        Semantic,
        "a trigger callback does not target a kotoage/言挙げ function",
        "Point the trigger at a declared kotoage/言挙げ function."
    ),
    explanation!(
        "E_TRIGGER_VIEW_TARGET",
        Semantic,
        "a trigger callback targets a read-only view function",
        "Point the trigger at a declared kotoage/言挙げ function instead."
    ),
];

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
            "E_UNSHIELD_AMOUNT_RANGE",
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
