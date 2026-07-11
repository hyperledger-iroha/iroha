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
        "E_AMOUNT_SUFFIX",
        Lex,
        "an Amount literal has a missing or invalid suffix",
        "Write the lowercase `amt` suffix directly after the base-10 digits, for example `10amt` or `1.25amt`."
    ),
    explanation!(
        "E_AMOUNT_SUFFIX_SEPARATED",
        Lex,
        "whitespace separates an Amount value from its suffix",
        "Remove the whitespace so the complete literal is one token, for example `10amt`."
    ),
    explanation!(
        "E_AMOUNT_MALFORMED",
        Lex,
        "an Amount literal has invalid decimal spelling",
        "Use base-10 digits, at most one decimal point, digit-separating underscores, and the exact lowercase `amt` suffix."
    ),
    explanation!(
        "E_AMOUNT_NEGATIVE",
        Parse,
        "an Amount literal is negative",
        "Amounts are non-negative; remove the unary minus or use a different numeric type."
    ),
    explanation!(
        "E_AMOUNT_SCALE_OVERFLOW",
        Semantic,
        "an Amount literal exceeds the 28-digit scale limit",
        "Reduce the significant fractional precision to at most 28 digits. Trailing fractional zeros are canonicalized away before this limit is checked."
    ),
    explanation!(
        "E_AMOUNT_MANTISSA_OVERFLOW",
        Semantic,
        "an Amount literal exceeds the 512-bit mantissa limit",
        "Reduce the magnitude of the literal so its unscaled decimal mantissa fits in 512 bits."
    ),
    explanation!(
        "E_AMOUNT_REMAINDER",
        Semantic,
        "Amount does not define a remainder operation",
        "Use exact division or amount.div_round with an explicit scale and rounding mode."
    ),
    explanation!(
        "E_AMOUNT_CONSTANT_ARITHMETIC",
        Semantic,
        "constant Amount arithmetic is invalid",
        "Change the literal operands so subtraction does not underflow, the mantissa and canonical scale remain in range, and plain division has an exact finite result; use div_round for intentional rounding."
    ),
    explanation!(
        "E_AMOUNT_DIV_ROUND_ARITY",
        Semantic,
        "Amount.div_round has the wrong number of arguments",
        "Pass exactly divisor, scale, and mode, using named arguments where the call policy requires them."
    ),
    explanation!(
        "E_AMOUNT_DIV_ROUND_RECEIVER",
        Semantic,
        "div_round was called on a value that is not Amount",
        "Call div_round on an Amount value and pass an Amount divisor."
    ),
    explanation!(
        "E_AMOUNT_DIV_ROUND_SCALE",
        Semantic,
        "Amount.div_round received a scale outside zero through 28",
        "Choose a compile-time scale from 0 through 28."
    ),
    explanation!(
        "E_AMOUNT_DIV_ROUND_SCALE_TYPE",
        Semantic,
        "Amount.div_round scale is not an i64 constant",
        "Pass the scale as a compile-time i64 value from 0 through 28."
    ),
    explanation!(
        "E_AMOUNT_ROUNDING_MODE",
        Semantic,
        "Amount.div_round received an unknown rounding mode",
        "Use exactly Rounding::floor, Rounding::ceil, or Rounding::nearest_even."
    ),
    explanation!(
        "E_UNSIGNED_NEGATION",
        Semantic,
        "a non-negative numeric type was negated",
        "Remove the negation or choose a signed type for values that may be negative."
    ),
    explanation!(
        "E_LEGACY_SUM_CONSTRUCTOR",
        Parse,
        "source uses a retired placeholder-based Option or Result constructor",
        "Use Option::some(value), contextual Option::none, Result::ok(value), or Result::err(error)."
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
        "Pass named offset: i64 and limit: i64 arguments."
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
        "Use an i64 index with get or try_set."
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
        "source uses the retired Numeric name for the Amount JSON getter",
        "Use value.get_amount(key) or json::get_amount(value, key); every typed JSON getter returns Option<T>."
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
    fn v1_data_processing_diagnostics_are_explainable() {
        for code in [
            "E_AMOUNT_CONSTANT_ARITHMETIC",
            "E_AMOUNT_DIV_ROUND_ARITY",
            "E_AMOUNT_DIV_ROUND_RECEIVER",
            "E_AMOUNT_DIV_ROUND_SCALE",
            "E_AMOUNT_DIV_ROUND_SCALE_TYPE",
            "E_AMOUNT_ROUNDING_MODE",
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
