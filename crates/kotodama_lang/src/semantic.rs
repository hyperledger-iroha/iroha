use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use indexmap::{IndexMap, IndexSet};
use iroha_data_model::events::data::prelude::{
    AccountEventFilter, AccountEventSet, AssetDefinitionEventFilter, AssetDefinitionEventSet,
    AssetEventFilter, AssetEventSet, ConfigurationEventFilter, ConfigurationEventSet,
    DomainEventFilter, DomainEventSet, ExecutorEventFilter, ExecutorEventSet, NftEventFilter,
    NftEventSet, PeerEventFilter, PeerEventSet, RoleEventFilter, RoleEventSet, RwaEventFilter,
    RwaEventSet, TriggerEventFilter, TriggerEventSet,
};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    events::{
        EventFilterBox,
        data::DataEventFilter,
        execute_trigger::ExecuteTriggerEventFilter,
        pipeline::{
            BlockEventFilter, BlockStatus, PipelineEventFilterBox, TransactionEventFilter,
            TransactionStatus,
        },
        time::{ExecutionTime, Schedule, TimeEventFilter},
    },
    metadata::Metadata,
    nft::NftId,
    peer::PeerId,
    prelude::Name,
    role::RoleId,
    rwa::RwaId,
    trigger::{TriggerId, action::Repeats},
};
use iroha_primitives::{
    bigint::BigInt,
    json::Json,
    numeric::{MAX_MANTISSA_BYTES, Numeric, NumericError, RoundingMode},
};
use norito::json::{self, native::Number as JsonNumber};

use super::ast::*;
use crate::builtins::{
    Builtin, BuiltinCallPolicy, BuiltinMode, BuiltinSurface, PointerConstructor,
};
use crate::source::{MAX_NESTING_DEPTH, MAX_TOKENS};

/// First-release collection-iteration limit.
///
/// V1 accepts only compiler-proven literal bounds. This cap is part of the
/// language definition and therefore identical in every build.
pub const COLLECTION_ITERATION_LIMIT: i64 = 64;
/// Maximum number of recursively expanded type nodes retained by semantic analysis.
///
/// The limit shares the fixed V1 token budget: a compact DAG of named value
/// types cannot make the compiler allocate more expanded type nodes than a
/// source could contain lexical tokens. Expansion is measured with saturating
/// arithmetic before any recursive type materialization occurs.
pub const MAX_EXPANDED_TYPE_NODES: usize = MAX_TOKENS;
/// Canonical nominal name for the structurally-specialized V1 query page.
const QUERY_PAGE_TYPE_NAME: &str = "QueryPage";
// BEGIN GENERATED: kotodama-v1-semantic-policy
/// Canonical source-level type spellings offered by language tooling.
pub const V1_SOURCE_TYPE_NAMES: &[&str] = &[
    "int",
    "decimal",
    "quantity",
    "bool",
    "string",
    "bytes",
    "Json",
    "AccountId",
    "AssetDefinitionId",
    "AssetId",
    "DomainId",
    "Name",
    "NftId",
    "DataSpaceId",
    "Option",
    "Result",
    "List",
    "StateMap",
    "Secret",
    "AccountView",
    "AssetView",
    "AssetDefinitionView",
    "DomainView",
    "NftView",
    "QueryPage",
];
/// Compiler-owned non-keyword names forbidden for source declarations.
pub const V1_DECLARATION_RESERVED_EXTRA_NAMES: &[&str] = &[
    "AxtDescriptor",
    "AssetHandle",
    "ProofBlob",
    "SoracloudRequest",
    "SoracloudResponse",
    "state_map_get",
    "__kotodama_list_len",
    "__kotodama_list_get",
    "__kotodama_list_try_set",
    "__kotodama_list_try_push",
    "__kotodama_list_pop",
    "__kotodama_list_contains",
    "__kotodama_list_take",
    "__kotodama_list_enumerate",
    "__kotodama_decimal_div_round",
    "__kotodama_quantity_div_round",
    "__kotodama_quantity_ratio_round",
    "__kotodama_decimal_to_int_trunc",
    "__kotodama_decimal_to_int_round",
    "is_some",
    "is_none",
    "is_ok",
    "is_err",
    "unwrap_or",
    "unwrap_err_or",
];
/// Exact canonical scalar types permitted as durable StateMap keys.
pub const V1_STATE_MAP_KEY_TYPE_NAMES: &[&str] = &[
    "int",
    "decimal",
    "quantity",
    "bool",
    "string",
    "bytes",
    "DataSpaceId",
    "AccountId",
    "AssetDefinitionId",
    "AssetId",
    "NftId",
    "DomainId",
    "Name",
];
/// Canonical bounded StateMap scan provenance in manifest order.
pub const V1_DYNAMIC_ACCESS_BOUND_KINDS: &[&str] = &["range", "take"];
/// Maximum keys advertised by one bounded dynamic-access hint.
pub const V1_DYNAMIC_ACCESS_MAX_KEYS: u32 = 64;
/// Canonical prefix for a direct durable StateMap hint base.
pub const V1_DYNAMIC_ACCESS_BASE_PREFIX: &str = "state:";
/// Canonical validation policy for the StateMap base identifier.
pub const V1_DYNAMIC_ACCESS_BASE_IDENTIFIER_POLICY: &str = "state_declaration_identifier";
/// Dynamic hints may refer only to a directly declared top-level StateMap.
pub const V1_DYNAMIC_ACCESS_REQUIRES_DECLARED_STATE_MAP: bool = true;
/// Dynamic hints are advisory and never scheduler-authoritative in V1.
pub const V1_DYNAMIC_ACCESS_SCHEDULER_AUTHORITATIVE: bool = false;
/// Retired pre-release numeric type spellings that remain reserved in V1.
///
/// Keeping these names unavailable to source-unit identities and declared
/// types prevents authenticated metadata from reinterpreting a known retired
/// type spelling. They remain ordinary names in value and function namespaces,
/// including entrypoints.
pub const V1_RETIRED_NUMERIC_TYPE_NAMES: &[&str] = &[
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "num",
    "Int",
    "Integer",
    "float",
    "f32",
    "f64",
    "Decimal",
    "Fixed",
    "FixedPoint",
    "Amount",
    "amount",
    "money",
    "Quantity",
    "number",
];
/// Canonical active-only sum constructor and pattern paths.
pub const V1_SUM_PATHS: &[&str] = &["Option::some", "Option::none", "Result::ok", "Result::err"];
/// Canonical explicit exact-decimal rounding modes.
pub const V1_ROUNDING_PATHS: &[&str] = &[
    "Rounding::toward_zero",
    "Rounding::away_from_zero",
    "Rounding::floor",
    "Rounding::ceil",
    "Rounding::nearest_even",
    "Rounding::nearest_away",
    "Rounding::nearest_toward_zero",
];
/// Canonical bounded-list member API.
pub const V1_LIST_MEMBER_NAMES: &[&str] = &[
    "len",
    "get",
    "try_set",
    "try_push",
    "pop",
    "contains",
    "take",
    "enumerate",
];
// END GENERATED: kotodama-v1-semantic-policy
const LINKED_SYMBOL_PREFIX: &str = "__kotodama_link_";
pub(crate) const LIST_LEN_INTRINSIC: &str = "__kotodama_list_len";
pub(crate) const LIST_GET_INTRINSIC: &str = "__kotodama_list_get";
pub(crate) const LIST_TRY_SET_INTRINSIC: &str = "__kotodama_list_try_set";
pub(crate) const LIST_TRY_PUSH_INTRINSIC: &str = "__kotodama_list_try_push";
pub(crate) const LIST_POP_INTRINSIC: &str = "__kotodama_list_pop";
pub(crate) const LIST_CONTAINS_INTRINSIC: &str = "__kotodama_list_contains";
pub(crate) const LIST_TAKE_INTRINSIC: &str = "__kotodama_list_take";
pub(crate) const LIST_ENUMERATE_INTRINSIC: &str = "__kotodama_list_enumerate";
pub(crate) const DECIMAL_DIV_ROUND_INTRINSIC: &str = "__kotodama_decimal_div_round";
pub(crate) const QUANTITY_DIV_ROUND_INTRINSIC: &str = "__kotodama_quantity_div_round";
pub(crate) const QUANTITY_RATIO_ROUND_INTRINSIC: &str = "__kotodama_quantity_ratio_round";
pub(crate) const DECIMAL_TO_INT_TRUNC_INTRINSIC: &str = "__kotodama_decimal_to_int_trunc";
pub(crate) const DECIMAL_TO_INT_ROUND_INTRINSIC: &str = "__kotodama_decimal_to_int_round";

fn is_list_intrinsic(name: &str) -> bool {
    matches!(
        name,
        LIST_LEN_INTRINSIC
            | LIST_GET_INTRINSIC
            | LIST_TRY_SET_INTRINSIC
            | LIST_TRY_PUSH_INTRINSIC
            | LIST_POP_INTRINSIC
            | LIST_CONTAINS_INTRINSIC
            | LIST_TAKE_INTRINSIC
            | LIST_ENUMERATE_INTRINSIC
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompilerIntrinsicKind {
    StateMap,
    List,
    Numeric,
    Sum,
}

/// Classify calls owned by typed semantic lowering rather than by the source
/// function graph or the public builtin surface.
///
/// Keeping this registry at the semantic boundary makes production projection
/// validate the same internal call vocabulary that semantic analysis emits.
/// It also prevents source declarations from shadowing compiler-owned calls.
fn compiler_intrinsic_kind(name: &str) -> Option<CompilerIntrinsicKind> {
    if name == STATE_MAP_GET_INTRINSIC {
        return Some(CompilerIntrinsicKind::StateMap);
    }
    if is_list_intrinsic(name) {
        return Some(CompilerIntrinsicKind::List);
    }
    if matches!(
        name,
        DECIMAL_DIV_ROUND_INTRINSIC
            | QUANTITY_DIV_ROUND_INTRINSIC
            | QUANTITY_RATIO_ROUND_INTRINSIC
            | DECIMAL_TO_INT_TRUNC_INTRINSIC
            | DECIMAL_TO_INT_ROUND_INTRINSIC
    ) {
        return Some(CompilerIntrinsicKind::Numeric);
    }
    if matches!(
        name,
        "is_some" | "is_none" | "is_ok" | "is_err" | "unwrap_or" | "unwrap_err_or"
    ) {
        return Some(CompilerIntrinsicKind::Sum);
    }
    None
}

/// Return whether a source declaration collides with compiler-owned names.
pub fn is_reserved_source_declaration(name: &str, is_function: bool) -> bool {
    name.starts_with(LINKED_SYMBOL_PREFIX)
        || V1_SOURCE_TYPE_NAMES.contains(&name)
        || V1_DECLARATION_RESERVED_EXTRA_NAMES.contains(&name)
        || (is_function && name == crate::metadata::KOTO_TEST_RETURN_ENTRYPOINT)
        || (is_function
            && (Builtin::from_name(name).is_some() || Builtin::from_source_name(name).is_some()))
}

/// Return whether a declared source type collides with an active or retired
/// compiler-owned type spelling.
///
/// Retired scalar spellings remain forbidden in type position, but they do
/// not poison the value namespace: names such as `amount`, `money`, and
/// `number` are ordinary parameters, locals, functions, and entrypoints.
pub fn is_reserved_source_type_declaration(name: &str) -> bool {
    is_reserved_source_declaration(name, false) || V1_RETIRED_NUMERIC_TYPE_NAMES.contains(&name)
}

fn enforce_static_iteration_limit(form: &str, span: u128) -> Result<(), SemanticError> {
    let limit = u128::try_from(COLLECTION_ITERATION_LIMIT).expect("positive V1 iteration limit");
    if span > limit {
        return Err(SemanticError {
            code: "E_ITERATION_LIMIT",
            message: format!("`{form}` span {span} exceeds the Kotodama V1 limit {limit}"),
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FunctionEffects {
    host_side_effects: bool,
    emits_instructions: bool,
    mutates_durable_state: bool,
}

impl FunctionEffects {
    fn merge_from(&mut self, other: Self) -> bool {
        let merged = Self {
            host_side_effects: self.host_side_effects || other.host_side_effects,
            emits_instructions: self.emits_instructions || other.emits_instructions,
            mutates_durable_state: self.mutates_durable_state || other.mutates_durable_state,
        };
        let changed = *self != merged;
        *self = merged;
        changed
    }

    fn requires_permission(self) -> bool {
        self.host_side_effects || self.emits_instructions || self.mutates_durable_state
    }

    fn forbids_view(self) -> bool {
        self.host_side_effects || self.emits_instructions || self.mutates_durable_state
    }
}

#[derive(Clone, Default)]
struct FunctionSummary {
    direct_effects: FunctionEffects,
    calls: IndexSet<String>,
}

fn collect_source_expr_summary(expr: &Expr, summary: &mut FunctionSummary) {
    match expr {
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            collect_source_expr_summary(expression, summary);
        }
        Expr::Call { name, args, .. } => {
            let normalized = normalize_namespaced(name);
            if let Some(builtin) = Builtin::from_name(&normalized) {
                let effects = builtin.effects();
                summary.direct_effects.merge_from(FunctionEffects {
                    host_side_effects: effects.host_side_effects,
                    emits_instructions: effects.emits_instructions,
                    mutates_durable_state: effects.mutates_durable_state,
                });
            } else {
                summary.calls.insert(normalized);
            }
            for argument in args {
                collect_source_expr_summary(argument, summary);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for field in fields {
                collect_source_expr_summary(&field.value, summary);
            }
        }
        Expr::JsonObject(entries) => {
            for entry in entries {
                collect_source_expr_summary(&entry.value, summary);
            }
        }
        Expr::JsonArray(items) => {
            for item in items {
                collect_source_expr_summary(item, summary);
            }
        }
        Expr::Binary { left, right, .. }
        | Expr::Index {
            target: left,
            index: right,
        } => {
            collect_source_expr_summary(left, summary);
            collect_source_expr_summary(right, summary);
        }
        Expr::Unary { expr, .. }
        | Expr::Member { object: expr, .. }
        | Expr::OptionSome(expr)
        | Expr::ResultOk(expr)
        | Expr::ResultErr(expr)
        | Expr::Propagate(expr) => {
            collect_source_expr_summary(expr, summary);
        }
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_source_expr_summary(cond, summary);
            collect_source_expr_summary(then_expr, summary);
            collect_source_expr_summary(else_expr, summary);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_source_expr_summary(condition, summary);
            collect_source_block_summary(then_branch, summary);
            if let Some(branch) = else_branch {
                collect_source_block_summary(branch, summary);
            }
        }
        Expr::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_source_expr_summary(value, summary);
            collect_source_block_summary(then_branch, summary);
            if let Some(branch) = else_branch {
                collect_source_block_summary(branch, summary);
            }
        }
        Expr::Match { value, arms } => {
            collect_source_expr_summary(value, summary);
            for arm in arms {
                collect_source_block_summary(&arm.body, summary);
            }
        }
        Expr::Tuple(items) | Expr::List(items) => {
            for item in items {
                collect_source_expr_summary(item, summary);
            }
        }
        Expr::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            collect_source_expr_summary(source, summary);
            collect_source_expr_summary(expression, summary);
            if let Some(condition) = condition {
                collect_source_expr_summary(condition, summary);
            }
        }
        Expr::Bool(_)
        | Expr::IntLiteral(_)
        | Expr::DecimalLiteral(_)
        | Expr::OptionNone
        | Expr::String(_)
        | Expr::Bytes(_)
        | Expr::Ident(_) => {}
    }
}

fn collect_source_statement_summary(statement: &Statement, summary: &mut FunctionSummary) {
    match statement.kind() {
        Statement::Source { .. } | Statement::Resolved { .. } => {
            unreachable!("kind() strips provenance wrappers")
        }
        Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => collect_source_expr_summary(value, summary),
        Statement::AssignExpr { target, value, .. } => {
            collect_source_expr_summary(target, summary);
            collect_source_expr_summary(value, summary);
        }
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_source_expr_summary(cond, summary);
            collect_source_block_summary(then_branch, summary);
            if let Some(branch) = else_branch {
                collect_source_block_summary(branch, summary);
            }
        }
        Statement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_source_expr_summary(value, summary);
            collect_source_block_summary(then_branch, summary);
            if let Some(branch) = else_branch {
                collect_source_block_summary(branch, summary);
            }
        }
        Statement::While { cond, body } => {
            collect_source_expr_summary(cond, summary);
            collect_source_block_summary(body, summary);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                collect_source_statement_summary(init, summary);
            }
            if let Some(cond) = cond {
                collect_source_expr_summary(cond, summary);
            }
            if let Some(step) = step {
                collect_source_statement_summary(step, summary);
            }
            collect_source_block_summary(body, summary);
        }
        Statement::ForEachMap { map, body, .. } => {
            collect_source_expr_summary(map, summary);
            collect_source_block_summary(body, summary);
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn collect_source_block_summary(block: &Block, summary: &mut FunctionSummary) {
    for statement in &block.statements {
        collect_source_statement_summary(statement, summary);
    }
    if let Some(tail) = &block.tail {
        collect_source_expr_summary(tail, summary);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedParam {
    pub name: String,
    pub ty: Type,
    pub is_state: bool,
}

/// Resolved type signature made available to a separately analyzed module.
///
/// Module bodies are type checked before linking.  Consequently an imported
/// call needs only the exported signature here; the callee body remains in its
/// own typed HIR until the linker combines both units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    /// Ordered, explicitly typed parameters.
    pub params: Vec<TypedParam>,
    /// Resolved return type (`()` for a function without a return value).
    pub return_type: Type,
    /// Whether every source call must name its supplied arguments.
    ///
    /// This is part of the exported module interface because an importing
    /// source unit cannot inspect the callee body to recover its effects.
    pub requires_named_arguments: bool,
    /// Source-level function kind and authorization retained for test linking.
    pub modifiers: FunctionModifiers,
}

/// Complete typed interface exposed by a deployable target to local test modules.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestTargetEnvironment {
    pub(crate) functions: BTreeMap<String, FunctionSignature>,
    pub(crate) structs: HashMap<String, Vec<(String, Type)>>,
    pub(crate) states: IndexMap<String, Type>,
    pub(crate) consts: IndexMap<String, TypedExpr>,
    pub(crate) error_codes: HashMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Signed adaptive-width integer in `-2^511..=2^511-1`.
    Int,
    /// Exact bounded base-10 decimal.
    Decimal,
    /// Nominal non-negative ledger quantity backed by the decimal representation.
    Quantity,
    Bool,
    String,
    /// First-class raw byte sequence.
    Bytes,
    /// Dataspace identifier used for Nexus/AXT flows.
    DataSpaceId,
    /// Atomic cross-transaction descriptor pointer.
    AxtDescriptor,
    /// Capability handle provided by asset dataspace issuers.
    AssetHandle,
    /// Proof material supplied by dataspace verifiers.
    ProofBlob,
    /// Soracloud host request envelope pointer.
    SoracloudRequest,
    /// Soracloud host response envelope pointer.
    SoracloudResponse,
    AccountId,
    AssetDefinitionId,
    AssetId,
    NftId,
    DomainId,
    Name,
    Json,
    Unit,
    /// Execution-local confidential value available only to ZK contracts.
    Secret(Box<Type>),
    /// Durable key/value state addressed through the canonical StateMap API.
    StateMap(Box<Type>, Box<Type>),
    /// Presence-aware value represented by one active-only compiler-owned sum handle.
    Option(Box<Type>),
    /// Success/error value represented by one active-only compiler-owned sum handle.
    Result(Box<Type>, Box<Type>),
    /// Contiguous compiler-owned list with a compile-time capacity in `1..=64`.
    List(Box<Type>, u8),
    Tuple(Vec<Type>),
    /// User-defined product type with named fields.
    Struct {
        name: String,
        fields: Arc<[(String, Type)]>,
    },
    /// Forward reference to a declared struct, resolved before typed HIR leaves analysis.
    NamedStruct(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub expr: ExprKind,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Binary {
        op: BinaryOp,
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    /// Explicit numeric conversion requested by a canonical source constructor.
    ///
    /// V1 never inserts this node to make otherwise-incompatible operands or
    /// assignments type check.
    NumericCast {
        expr: Box<TypedExpr>,
    },
    /// Recoverable conversion into the nominal `quantity` domain.
    ///
    /// The error payload is the stable numeric-fault tag returned by ABI V1.
    NumericTryCast {
        expr: Box<TypedExpr>,
    },
    /// Ternary conditional expression: `cond ? then : else`.
    Conditional {
        cond: Box<TypedExpr>,
        then_expr: Box<TypedExpr>,
        else_expr: Box<TypedExpr>,
    },
    /// Expression-valued conditional blocks.
    If {
        condition: Box<TypedExpr>,
        then_branch: TypedBlock,
        else_branch: TypedBlock,
    },
    /// Expression-valued sum pattern test.
    IfLet {
        pattern: TypedSumPattern,
        value: Box<TypedExpr>,
        then_branch: TypedBlock,
        else_branch: TypedBlock,
    },
    /// Exhaustive sum match.
    Match {
        value: Box<TypedExpr>,
        arms: Vec<TypedMatchArm>,
    },
    /// Active `Option` payload with no inactive placeholder.
    OptionSome {
        value: Box<TypedExpr>,
    },
    /// Inactive `Option` value with its payload type carried only by `TypedExpr::ty`.
    OptionNone,
    /// Active `Result` success payload with no inactive error placeholder.
    ResultOk {
        value: Box<TypedExpr>,
    },
    /// Active `Result` error payload with no inactive success placeholder.
    ResultErr {
        error: Box<TypedExpr>,
    },
    /// Postfix same-family propagation.
    Propagate {
        value: Box<TypedExpr>,
    },
    Call {
        name: String,
        args: Vec<TypedExpr>,
    },
    /// A named call whose arguments remain stored in declaration order while
    /// `evaluation_order` records parameter slots in source evaluation order.
    ///
    /// Lowering evaluates the recorded slots first and only permutes the
    /// resulting temporary references into ABI order. No runtime permutation
    /// instructions are required.
    NamedCall {
        name: String,
        args: Vec<TypedExpr>,
        evaluation_order: Vec<usize>,
    },
    /// Named struct fields in source evaluation order after validation.
    StructLiteral {
        name: String,
        fields: Vec<(String, TypedExpr)>,
    },
    Tuple(Vec<TypedExpr>),
    /// Bounded list literal stored in one compiler-owned allocation.
    List(Vec<TypedExpr>),
    /// Capacity-proven bounded list comprehension.
    ListComprehension {
        expression: Box<TypedExpr>,
        item: String,
        source: Box<TypedExpr>,
        condition: Option<Box<TypedExpr>>,
    },
    /// Native canonical JSON object with decoded keys in source order.
    JsonObject(Vec<(String, TypedExpr)>),
    /// Native canonical JSON array.
    JsonArray(Vec<TypedExpr>),
    Member {
        object: Box<TypedExpr>,
        field: String,
    },
    Index {
        target: Box<TypedExpr>,
        index: Box<TypedExpr>,
    },
    /// A source `int` literal in the complete signed 512-bit domain.
    IntLiteral(BigInt),
    /// Canonical exact decimal payload paired with its source spelling.
    DecimalLiteral {
        value: Numeric,
        spelling: String,
    },
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    Ident(String),
}

/// Semantically checked sum pattern and its active payload type.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedSumPattern {
    pub pattern: SumPattern,
    pub payload_type: Option<Type>,
}

/// One typed exhaustive match arm.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedMatchArm {
    pub pattern: TypedSumPattern,
    pub body: TypedBlock,
}

/// One typed/effect-analysis failure with an explicit stable identity.
#[derive(Debug, PartialEq, Eq)]
pub struct SemanticError {
    /// Stable machine-readable diagnostic code, independent of message text.
    pub(crate) code: &'static str,
    /// Human-readable diagnostic message without an embedded code prefix.
    pub(crate) message: String,
}

impl SemanticError {
    /// Return the stable machine-readable code.
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Return the human-readable, prefix-free message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for SemanticError {}

#[derive(Debug, PartialEq)]
pub(crate) struct SemanticFailure {
    pub(crate) error: SemanticError,
    pub(crate) location: Option<SourceLocation>,
    pub(crate) diagnostic: Option<crate::semantic_diagnostics::SemanticDiagnostic>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct SemanticFailures {
    pub(crate) failures: Vec<SemanticFailure>,
}

impl From<SemanticError> for SemanticFailures {
    fn from(error: SemanticError) -> Self {
        Self {
            failures: vec![SemanticFailure {
                error,
                location: None,
                diagnostic: None,
            }],
        }
    }
}

impl SemanticFailures {
    fn into_first(self) -> SemanticError {
        self.failures
            .into_iter()
            .next()
            .expect("semantic failure collections are never empty")
            .error
    }
}

fn record_semantic_failure(
    failures: &mut Vec<SemanticFailure>,
    omitted: &mut usize,
    failure: SemanticFailure,
) {
    if failures.len() < crate::diagnostic::MAX_DIAGNOSTICS - 1 {
        failures.push(failure);
    } else {
        *omitted = omitted.saturating_add(1);
    }
}

fn attach_pending_diagnostic(
    failures: &mut SemanticFailures,
    pending: Option<crate::semantic_diagnostics::SemanticDiagnostic>,
) {
    let Some(pending) = pending else {
        return;
    };
    if let Some(failure) = failures
        .failures
        .iter_mut()
        .find(|failure| failure.error.code != "K0004" && failure.diagnostic.is_none())
    {
        failure.diagnostic = Some(pending);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedProgram {
    pub unit: SourceUnit,
    pub items: Vec<TypedItem>,
    pub states: Vec<TypedStateDecl>,
    pub error_codes: Vec<TypedErrorCode>,
    pub triggers: Vec<TypedTrigger>,
    pub message_entries: Vec<MessageEntry>,
    /// Stable typed/effect-HIR metadata keyed independently of Rust addresses.
    pub hir_nodes: BTreeMap<TypedHirNodeId, TypedHirNode>,
    /// Stable immutable source files retained by the typed-HIR graph for exact
    /// diagnostics and hash-keyed debug sidecars.
    pub source_files: BTreeMap<crate::source::SourceId, crate::source::SourceFile>,
    /// Whether this HIR was analyzed with local test capabilities enabled.
    ///
    /// Production artifact builders reject test-capable HIR even when a
    /// caller removes the source-level test declarations after analysis. This
    /// provenance bit keeps the mode boundary fail-closed across typed-module
    /// linking and compiler-internal typed-HIR builds.
    pub test_support_enabled: bool,
}

/// Graph-stable identity of one typed HIR node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypedHirNodeId {
    /// Source unit that owns the local HIR identity.
    pub source: crate::source::SourceId,
    /// Stable source-unit-local identity.
    pub local: HirId,
}

/// Type and resolver target retained for one successfully typed expression.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedHirNode {
    /// Graph-stable identity.
    pub id: TypedHirNodeId,
    /// Exact source range when source-backed.
    pub source: Option<crate::source::SourceRange>,
    /// Authoritative resolver target for named nodes.
    pub target: Option<crate::resolved::ResolvedTarget>,
    /// Final semantic type.
    pub ty: Type,
}

/// Stable source-declared application error emitted in the seiyaku interface.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TypedErrorCode {
    pub namespace: String,
    pub name: String,
    pub code: u32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedStateDecl {
    pub name: String,
    pub ty: Type,
    /// Exact source range of the state declaration, when source-backed.
    pub source: Option<crate::source::SourceRange>,
}

/// Mutable semantic state for exactly one compilation.
///
/// Keeping these registries in an owned context prevents independent compiler
/// sessions from observing one another's declarations while still allowing the
/// semantic pass to build forward-reference tables before checking bodies.
#[derive(Default)]
pub struct SemanticContext {
    structs: RefCell<HashMap<String, Vec<(String, Type)>>>,
    states: RefCell<IndexMap<String, Type>>,
    consts: RefCell<IndexMap<String, TypedExpr>>,
    function_returns: RefCell<HashMap<String, Type>>,
    function_modifiers: RefCell<HashMap<String, FunctionModifiers>>,
    function_params: RefCell<HashMap<String, Vec<TypedParam>>>,
    function_named_only_reasons: RefCell<HashMap<String, &'static str>>,
    function_summaries: RefCell<HashMap<String, FunctionSummary>>,
    global_declarations: RefCell<HashSet<String>>,
    current_function_modifiers: RefCell<Option<FunctionModifiers>>,
    current_function_name: RefCell<Option<String>>,
    current_mutable_bindings: RefCell<HashSet<String>>,
    trigger_callback_functions: RefCell<HashSet<String>>,
    current_state_param_names: RefCell<HashSet<String>>,
    zk_enabled: bool,
    test_builtins_enabled: bool,
    error_codes: RefCell<HashMap<String, u32>>,
    external_functions: RefCell<BTreeMap<String, FunctionSignature>>,
    external_states: RefCell<IndexMap<String, Type>>,
    resolved_arena: RefCell<Option<Arc<crate::resolved::ResolvedArena>>>,
    resolved_binding_types: RefCell<BTreeMap<crate::resolved::BindingId, Type>>,
    typed_hir_nodes: RefCell<BTreeMap<HirId, Type>>,
    pending_diagnostic: RefCell<Option<crate::semantic_diagnostics::SemanticDiagnostic>>,
    required_list_capacity: RefCell<Option<u8>>,
    resolved_named_types: RefCell<HashMap<String, Type>>,
    resolved_named_type_resources: RefCell<HashMap<String, ExpandedTypeResources>>,
    next_synthetic_binding: Cell<usize>,
}

impl SemanticContext {
    /// Construct an empty per-compilation semantic context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a context with ZK-only language capabilities enabled by build policy.
    pub fn with_zk_enabled(zk_enabled: bool) -> Self {
        Self {
            zk_enabled,
            ..Self::default()
        }
    }

    /// Construct a context with compiler-owned execution capabilities.
    pub fn with_capabilities(zk_enabled: bool, test_builtins_enabled: bool) -> Self {
        Self {
            zk_enabled,
            test_builtins_enabled,
            ..Self::default()
        }
    }

    /// Analyze one parsed program using only state owned by this context.
    ///
    /// The context is reset before every call so callers may reuse it
    /// sequentially without leaking declarations between source units.
    pub fn analyze(&self, program: &Program) -> Result<TypedProgram, SemanticError> {
        self.analyze_all(program)
            .map_err(SemanticFailures::into_first)
    }

    /// Analyze one source unit with explicitly resolved imported functions.
    ///
    /// The external names must be fully qualified source names such as
    /// `math::add`. They participate in ordinary type checking but are not
    /// treated as local definitions for recursion or effect analysis. The
    /// typed-HIR linker reruns those whole-program analyses after resolving all
    /// calls to their final linked symbols.
    pub fn analyze_with_external_functions(
        &self,
        program: &Program,
        external_functions: &BTreeMap<String, FunctionSignature>,
    ) -> Result<TypedProgram, SemanticError> {
        self.analyze_all_with_external_functions(program, external_functions)
            .map_err(SemanticFailures::into_first)
    }

    pub(crate) fn analyze_all_with_external_functions(
        &self,
        program: &Program,
        external_functions: &BTreeMap<String, FunctionSignature>,
    ) -> Result<TypedProgram, SemanticFailures> {
        self.analyze_all_with_external_environment(program, external_functions, &IndexMap::new())
    }

    fn analyze_all_with_external_environment(
        &self,
        program: &Program,
        external_functions: &BTreeMap<String, FunctionSignature>,
        external_states: &IndexMap<String, Type>,
    ) -> Result<TypedProgram, SemanticFailures> {
        self.reset();
        self.external_functions.replace(external_functions.clone());
        self.external_states.replace(external_states.clone());
        analyze_with_context(self, program)
    }

    /// Resolve the function interface of one source unit without inspecting
    /// function bodies.
    ///
    /// This is the resolution pass used to make locked module exports
    /// available while every module is still analyzed independently.
    pub fn resolve_function_signatures(
        &self,
        program: &Program,
    ) -> Result<BTreeMap<String, FunctionSignature>, SemanticError> {
        self.reset();
        let struct_names = validate_declaration_uniqueness(program)?;
        self.structs.replace(
            struct_names
                .iter()
                .cloned()
                .map(|name| (name, Vec::new()))
                .collect(),
        );

        let mut structs = HashMap::new();
        for item in &program.items {
            let Item::Struct(definition) = item else {
                continue;
            };
            let mut fields = Vec::with_capacity(definition.fields.len());
            for (name, ty) in &definition.fields {
                fields.push((name.clone(), convert_type_expr(self, ty)?));
            }
            structs.insert(definition.name.clone(), fields);
        }
        self.structs.replace(structs);
        validate_acyclic_value_structs(self, &struct_names)?;
        let resolution_plan = validate_struct_resolution_budget(self, &struct_names)
            .map_err(|failure| failure.error)?;
        install_canonical_struct_types(self, resolution_plan);
        validate_declared_struct_list_schemas(self)?;

        let source_summaries = program
            .items
            .iter()
            .filter_map(|item| {
                let Item::Function(function) = item else {
                    return None;
                };
                let mut summary = FunctionSummary::default();
                collect_source_block_summary(&function.body, &mut summary);
                Some((function.name.clone(), summary))
            })
            .collect::<HashMap<_, _>>();
        let transitive_effects = compute_transitive_effects(&source_summaries);
        let mut signatures = BTreeMap::new();
        for item in &program.items {
            let Item::Function(function) = item else {
                continue;
            };
            let mut params = Vec::with_capacity(function.params.len());
            for param in &function.params {
                params.push(parse_declared_param_type(self, param, &function.modifiers)?);
            }
            let return_type = parse_declared_type(self, &function.ret_ty)?.unwrap_or(Type::Unit);
            let privileged = function.modifiers.permission.is_some()
                || matches!(
                    function.modifiers.kind,
                    FunctionKind::Kotoage | FunctionKind::Hajimari | FunctionKind::Kaizen
                );
            let effectful = transitive_effects
                .get(&function.name)
                .copied()
                .is_some_and(FunctionEffects::requires_permission);
            signatures.insert(
                function.name.clone(),
                FunctionSignature {
                    params,
                    return_type,
                    requires_named_arguments: function.params.len() >= 3
                        && (privileged || effectful),
                    modifiers: function.modifiers.clone(),
                },
            );
        }
        Ok(signatures)
    }

    pub(crate) fn resolve_resolved_function_signatures(
        &self,
        program: &crate::resolved::ResolvedProgram,
    ) -> Result<BTreeMap<String, FunctionSignature>, SemanticError> {
        self.resolve_resolved_function_signatures_all(program)
            .map_err(SemanticFailures::into_first)
    }

    pub(crate) fn resolve_resolved_function_signatures_all(
        &self,
        program: &crate::resolved::ResolvedProgram,
    ) -> Result<BTreeMap<String, FunctionSignature>, SemanticFailures> {
        self.reset();
        self.resolved_arena.replace(Some(program.arena()));
        let result = self.resolve_function_signatures(program.program());
        let pending = self.take_diagnostic();
        self.resolved_arena.borrow_mut().take();
        result.map_err(|error| {
            let mut failures = SemanticFailures::from(error);
            attach_pending_diagnostic(&mut failures, pending);
            failures
        })
    }

    pub(crate) fn analyze_resolved_with_external_functions(
        &self,
        program: &crate::resolved::ResolvedProgram,
        external_functions: &BTreeMap<String, FunctionSignature>,
    ) -> Result<TypedProgram, SemanticFailures> {
        let environment = TestTargetEnvironment {
            functions: external_functions.clone(),
            ..TestTargetEnvironment::default()
        };
        self.analyze_resolved_environment(program, &environment)
    }

    pub(crate) fn analyze_resolved_with_test_target(
        &self,
        program: &crate::resolved::ResolvedProgram,
        environment: &TestTargetEnvironment,
    ) -> Result<TypedProgram, SemanticFailures> {
        self.analyze_resolved_environment(program, environment)
    }

    pub(crate) fn test_target_environment(
        &self,
        functions: BTreeMap<String, FunctionSignature>,
        states: IndexMap<String, Type>,
    ) -> TestTargetEnvironment {
        TestTargetEnvironment {
            functions,
            structs: self.structs.borrow().clone(),
            states,
            consts: self.consts.borrow().clone(),
            error_codes: self.error_codes.borrow().clone(),
        }
    }

    pub(crate) fn analyze_all(&self, program: &Program) -> Result<TypedProgram, SemanticFailures> {
        self.reset();
        analyze_with_context(self, program)
    }

    /// Type and effect-check a program only after fail-closed named-HIR resolution.
    pub(crate) fn analyze_resolved(
        &self,
        program: &crate::resolved::ResolvedProgram,
    ) -> Result<TypedProgram, SemanticFailures> {
        self.analyze_resolved_environment(program, &TestTargetEnvironment::default())
    }

    fn analyze_resolved_environment(
        &self,
        program: &crate::resolved::ResolvedProgram,
        environment: &TestTargetEnvironment,
    ) -> Result<TypedProgram, SemanticFailures> {
        self.reset();
        self.resolved_arena.replace(Some(program.arena()));
        self.external_functions
            .replace(environment.functions.clone());
        self.external_states.replace(environment.states.clone());
        self.structs.replace(environment.structs.clone());
        self.consts.replace(environment.consts.clone());
        self.error_codes.replace(environment.error_codes.clone());
        let mut result = analyze_with_context(self, program.program());
        let pending = self.take_diagnostic();
        if let Err(failures) = &mut result {
            attach_pending_diagnostic(failures, pending);
        }
        self.resolved_arena.borrow_mut().take();
        self.resolved_binding_types.borrow_mut().clear();
        self.typed_hir_nodes.borrow_mut().clear();
        self.required_list_capacity.borrow_mut().take();
        result.map(|mut typed| {
            program.attach_sources(&mut typed);
            typed
        })
    }

    fn expression_source(&self, expression: &Expr) -> Option<crate::source::SourceRange> {
        self.resolved_source(expression.hir_id(), expression.source())
    }

    fn statement_source(&self, statement: &Statement) -> Option<crate::source::SourceRange> {
        self.resolved_source(statement.hir_id(), statement.source())
    }

    fn type_source(&self, ty: &TypeExpr) -> Option<crate::source::SourceRange> {
        self.resolved_source(ty.hir_id(), ty.source())
    }

    fn resolved_source(
        &self,
        id: Option<HirId>,
        raw: Option<crate::source::SourceRange>,
    ) -> Option<crate::source::SourceRange> {
        let arena = self.resolved_arena.borrow();
        let Some(arena) = arena.as_ref() else {
            return raw;
        };
        id.and_then(|id| arena.node(id))
            .and_then(|node| node.source)
    }

    fn resolved_node(
        &self,
        id: Option<HirId>,
        kind: crate::resolved::ResolvedNodeKind,
        source: Option<crate::source::SourceRange>,
    ) -> Result<Option<crate::resolved::ResolvedNode>, SemanticError> {
        let arena = self.resolved_arena.borrow();
        let Some(arena) = arena.as_ref() else {
            return Ok(None);
        };
        let id = id.ok_or_else(|| SemanticError {
            code: "E_INTERNAL_RESOLUTION",
            message: "production semantic input contains an unwrapped AST node".into(),
        })?;
        let node = arena.node(id).cloned().ok_or_else(|| SemanticError {
            code: "E_INTERNAL_RESOLUTION",
            message: format!(
                "resolved-HIR node {} is absent from its authority arena",
                id.0
            ),
        })?;
        if node.kind != kind || node.source != source {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!(
                    "resolved-HIR node {} metadata diverged from its authority arena",
                    id.0
                ),
            });
        }
        Ok(Some(node))
    }

    fn expression_target(
        &self,
        expression: &Expr,
    ) -> Result<Option<crate::resolved::ResolvedTarget>, SemanticError> {
        Ok(self
            .resolved_node(
                expression.hir_id(),
                crate::resolved::ResolvedNodeKind::Expression,
                expression.source(),
            )?
            .and_then(|node| node.target))
    }

    fn validate_statement_node(
        &self,
        statement: &Statement,
    ) -> Result<Option<crate::resolved::ResolvedNode>, SemanticError> {
        self.resolved_node(
            statement.hir_id(),
            crate::resolved::ResolvedNodeKind::Statement,
            statement.source(),
        )
    }

    fn validate_type_node(
        &self,
        ty: &TypeExpr,
    ) -> Result<Option<crate::resolved::ResolvedNode>, SemanticError> {
        self.resolved_node(
            ty.hir_id(),
            crate::resolved::ResolvedNodeKind::Type,
            ty.source(),
        )
    }

    fn validate_value_target(
        &self,
        expression: &Expr,
        name: &str,
        vars: &HashMap<String, Type>,
    ) -> Result<Option<(crate::resolved::ResolvedValueTarget, Option<Type>)>, SemanticError> {
        use crate::resolved::{ResolvedTarget, ResolvedValueTarget};
        let Some(target) = self.expression_target(expression)? else {
            if self.resolved_arena.borrow().is_some() {
                return Err(SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: format!("value `{name}` has no resolver-produced target"),
                });
            }
            return Ok(None);
        };
        let ResolvedTarget::Value(target) = target else {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!("value `{name}` carries a non-value resolver target"),
            });
        };
        let arena = self.resolved_arena.borrow();
        let arena = arena.as_ref().expect("resolved target requires arena");
        let ty = match target {
            ResolvedValueTarget::Binding(binding) => {
                let binding = arena.binding(binding).ok_or_else(|| SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: "value target references an unknown binding".into(),
                })?;
                if binding.name != name {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "value spelling `{name}` diverges from binding `{}`",
                            binding.name
                        ),
                    });
                }
                let node = expression.hir_id().ok_or_else(|| SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: "resolved value use lost its stable HIR identity".into(),
                })?;
                if !arena.binding_visible_at(binding.id, node) {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!("binding `{name}` is outside the resolved lexical scope"),
                    });
                }
                let ty = vars
                    .get(&binding.name)
                    .cloned()
                    .ok_or_else(|| SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "binding `{name}` is not visible in its resolved lexical scope"
                        ),
                    })?;
                let mut binding_types = self.resolved_binding_types.borrow_mut();
                if let Some(previous) = binding_types.insert(binding.id, ty.clone())
                    && previous != ty
                {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!("binding `{name}` acquired inconsistent semantic types"),
                    });
                }
                Some(ty)
            }
            ResolvedValueTarget::State(symbol) | ResolvedValueTarget::Const(symbol) => {
                let symbol = arena.symbol(symbol).ok_or_else(|| SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: "value target references an unknown symbol".into(),
                })?;
                if symbol.name != name {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "value spelling `{name}` diverges from symbol `{}`",
                            symbol.name
                        ),
                    });
                }
                None
            }
            ResolvedValueTarget::ErrorCode(_)
            | ResolvedValueTarget::Intrinsic
            | ResolvedValueTarget::ExternalState
            | ResolvedValueTarget::ExternalConst => None,
        };
        Ok(Some((target, ty)))
    }

    fn list_receiver_is_mutable(
        &self,
        expression: &Expr,
        name: &str,
        vars: &HashMap<String, Type>,
    ) -> Result<bool, SemanticError> {
        use crate::resolved::ResolvedValueTarget;

        let Some((target, _)) = self.validate_value_target(expression, name, vars)? else {
            return Ok(self.current_mutable_bindings.borrow().contains(name));
        };
        let ResolvedValueTarget::Binding(binding) = target else {
            return Ok(false);
        };
        let arena = self.resolved_arena.borrow();
        let binding = arena
            .as_ref()
            .and_then(|arena| arena.binding(binding))
            .ok_or_else(|| SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: "List mutator receiver references an unknown lexical binding".into(),
            })?;
        Ok(binding.mutable)
    }

    fn validate_call_target(
        &self,
        expression: &Expr,
        source_name: &str,
        normalized_name: &str,
        implicit_receiver: bool,
    ) -> Result<(), SemanticError> {
        use crate::resolved::{ResolvedCallTarget, ResolvedSymbolKind, ResolvedTarget};
        let Some(target) = self.expression_target(expression)? else {
            if self.resolved_arena.borrow().is_some() {
                return Err(SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: format!("call `{source_name}` has no resolver-produced target"),
                });
            }
            return Ok(());
        };
        let ResolvedTarget::Call(target) = target else {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!("call `{source_name}` carries a non-call resolver target"),
            });
        };
        let arena = self.resolved_arena.borrow();
        let arena = arena.as_ref().expect("resolved target requires arena");
        match target {
            ResolvedCallTarget::Function(symbol) => {
                let symbol = arena.symbol(symbol).ok_or_else(|| SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: "call references an unknown function symbol".into(),
                })?;
                if symbol.kind != ResolvedSymbolKind::Function || symbol.name != source_name {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "function call `{source_name}` diverges from resolver symbol `{}`",
                            symbol.name
                        ),
                    });
                }
            }
            ResolvedCallTarget::Builtin(builtin) => {
                if Builtin::from_name(normalized_name) != Some(builtin) {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "builtin call `{source_name}` diverges from its registry target"
                        ),
                    });
                }
            }
            ResolvedCallTarget::Method if !implicit_receiver => {
                return Err(SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: format!("method target `{source_name}` lost its receiver"),
                });
            }
            ResolvedCallTarget::Method
            | ResolvedCallTarget::Intrinsic
            | ResolvedCallTarget::External => {}
            ResolvedCallTarget::Struct(symbol) => {
                let symbol = arena.symbol(symbol).ok_or_else(|| SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: "positional struct call references an unknown symbol".into(),
                })?;
                if symbol.kind != ResolvedSymbolKind::Struct || symbol.name != source_name {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "positional struct call `{source_name}` diverges from resolver symbol `{}`",
                            symbol.name
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_assignment_target(
        &self,
        node: Option<&crate::resolved::ResolvedNode>,
        name: &str,
    ) -> Result<(), SemanticError> {
        use crate::resolved::{ResolvedTarget, ResolvedValueTarget};
        let Some(node) = node else {
            return Ok(());
        };
        let Some(target) = node.target.as_ref() else {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!("assignment `{name}` has no resolver-produced target"),
            });
        };
        let ResolvedTarget::Assignment(target) = target else {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!("assignment `{name}` carries a non-assignment resolver target"),
            });
        };
        let arena = self.resolved_arena.borrow();
        let arena = arena.as_ref().expect("resolved target requires arena");
        match target {
            ResolvedValueTarget::Binding(binding) => {
                let binding = arena.binding(*binding).ok_or_else(|| SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: "assignment references an unknown binding".into(),
                })?;
                if binding.name != name || !arena.binding_visible_at(binding.id, node.id) {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "assignment `{name}` diverges from its lexical binding target"
                        ),
                    });
                }
            }
            ResolvedValueTarget::State(symbol) | ResolvedValueTarget::Const(symbol) => {
                let symbol = arena.symbol(*symbol).ok_or_else(|| SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: "assignment references an unknown symbol".into(),
                })?;
                if symbol.name != name {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "assignment `{name}` diverges from resolver symbol `{}`",
                            symbol.name
                        ),
                    });
                }
            }
            ResolvedValueTarget::ExternalState => {}
            ResolvedValueTarget::ExternalConst
            | ResolvedValueTarget::ErrorCode(_)
            | ResolvedValueTarget::Intrinsic => {
                return Err(SemanticError {
                    code: "E_TYPE_ANNOTATION_MISMATCH",
                    message: format!("resolved value `{name}` is not assignable"),
                });
            }
        }
        Ok(())
    }

    fn validate_struct_literal_target(
        &self,
        expression: &Expr,
        name: &str,
    ) -> Result<(), SemanticError> {
        use crate::resolved::{ResolvedSymbolKind, ResolvedTarget};
        let Some(target) = self.expression_target(expression)? else {
            if self.resolved_arena.borrow().is_some() {
                return Err(SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: format!("struct literal `{name}` has no resolver-produced target"),
                });
            }
            return Ok(());
        };
        match target {
            ResolvedTarget::StructLiteral(symbol) => {
                let arena = self.resolved_arena.borrow();
                let symbol = arena
                    .as_ref()
                    .and_then(|arena| arena.symbol(symbol))
                    .ok_or_else(|| SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: "struct literal references an unknown symbol".into(),
                    })?;
                if symbol.kind != ResolvedSymbolKind::Struct || symbol.name != name {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "struct literal `{name}` diverges from resolver symbol `{}`",
                            symbol.name
                        ),
                    });
                }
            }
            ResolvedTarget::ExternalStructLiteral => {
                if !self.structs.borrow().contains_key(name) {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "external struct literal `{name}` is absent from the typed target interface"
                        ),
                    });
                }
            }
            _ => {
                return Err(SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: format!(
                        "struct literal `{name}` carries a non-struct resolver target"
                    ),
                });
            }
        }
        Ok(())
    }

    fn validate_named_type_target(
        &self,
        node: Option<&crate::resolved::ResolvedNode>,
        name: &str,
    ) -> Result<(), SemanticError> {
        use crate::resolved::{ResolvedSymbolKind, ResolvedTarget, ResolvedTypeTarget};
        let Some(node) = node else {
            return Ok(());
        };
        let Some(target) = node.target.as_ref() else {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!("type `{name}` has no resolver-produced target"),
            });
        };
        let ResolvedTarget::Type(target) = target else {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!("named type `{name}` carries a non-type resolver target"),
            });
        };
        match target {
            ResolvedTypeTarget::Builtin if !V1_SOURCE_TYPE_NAMES.contains(&name) => {
                return Err(SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: format!("type `{name}` is not in the canonical V1 builtin type table"),
                });
            }
            ResolvedTypeTarget::Struct(symbol) => {
                let arena = self.resolved_arena.borrow();
                let symbol = arena
                    .as_ref()
                    .and_then(|arena| arena.symbol(*symbol))
                    .ok_or_else(|| SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: "type target references an unknown struct".into(),
                    })?;
                if symbol.kind != ResolvedSymbolKind::Struct || symbol.name != name {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "type `{name}` diverges from resolver symbol `{}`",
                            symbol.name
                        ),
                    });
                }
            }
            ResolvedTypeTarget::ExternalStruct => {
                if !self.structs.borrow().contains_key(name) {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: format!(
                            "external type `{name}` is absent from the typed target interface"
                        ),
                    });
                }
            }
            ResolvedTypeTarget::Builtin => {}
        }
        Ok(())
    }

    fn record_typed_hir_node(&self, expression: &Expr, ty: &Type) -> Result<(), SemanticError> {
        // Explicit annotations are checked while they are resolved, but an
        // inferred tuple or sum can also combine many references to the same
        // named product DAG. Enforce the identical expanded-shape budget at
        // the typed expression boundary before downstream ABI and lowering
        // walks can revisit that graph.
        validate_use_site_type_resolution_budget(self, ty)?;
        let Some(node) = self.resolved_node(
            expression.hir_id(),
            crate::resolved::ResolvedNodeKind::Expression,
            expression.source(),
        )?
        else {
            return Ok(());
        };
        let mut typed = self.typed_hir_nodes.borrow_mut();
        if let Some(previous) = typed.insert(node.id, ty.clone())
            && resolve_struct_type(&previous) != resolve_struct_type(ty)
        {
            return Err(SemanticError {
                code: "E_INTERNAL_RESOLUTION",
                message: format!(
                    "HIR node {} acquired inconsistent semantic types",
                    node.id.0
                ),
            });
        }
        Ok(())
    }

    fn capture_diagnostic(
        &self,
        primary: Option<crate::source::SourceRange>,
        fix: Option<crate::semantic_diagnostics::SemanticFix>,
    ) {
        let Some(primary) = primary else {
            return;
        };
        let mut pending = self.pending_diagnostic.borrow_mut();
        if pending.is_none() {
            *pending = Some(crate::semantic_diagnostics::SemanticDiagnostic {
                primary,
                labels: Vec::new(),
                fix,
            });
        }
    }

    fn replace_diagnostic(
        &self,
        primary: Option<crate::source::SourceRange>,
        fix: Option<crate::semantic_diagnostics::SemanticFix>,
    ) {
        let Some(primary) = primary else {
            return;
        };
        self.pending_diagnostic
            .replace(Some(crate::semantic_diagnostics::SemanticDiagnostic {
                primary,
                labels: Vec::new(),
                fix,
            }));
    }

    fn capture_expression_diagnostic(
        &self,
        expression: &Expr,
        fix: Option<crate::semantic_diagnostics::SemanticFix>,
    ) {
        self.capture_diagnostic(self.expression_source(expression), fix);
    }

    fn capture_statement_diagnostic(
        &self,
        statement: &Statement,
        fix: Option<crate::semantic_diagnostics::SemanticFix>,
    ) {
        self.capture_diagnostic(self.statement_source(statement), fix);
    }

    fn take_diagnostic(&self) -> Option<crate::semantic_diagnostics::SemanticDiagnostic> {
        self.pending_diagnostic.borrow_mut().take()
    }

    fn discard_diagnostic(&self) {
        self.pending_diagnostic.borrow_mut().take();
    }

    fn fresh_aggregate_capture(&self) -> String {
        let index = self.next_synthetic_binding.get();
        self.next_synthetic_binding.set(
            index
                .checked_add(1)
                .expect("aggregate capture counter must not overflow"),
        );
        // NUL cannot occur in a source identifier, so this compiler-owned
        // binding cannot collide with a user local in any nested scope.
        format!("\0aggregate_capture#{index}")
    }

    /// Check whether one expression can initialize `expected` without letting
    /// a speculative error replace the diagnostic for the enclosing invalid
    /// construct. Typed expression analysis is otherwise side-effect free; a
    /// cloned local environment plus restored diagnostic/capacity scratch state
    /// makes this suitable for deciding whether a fix recipe will type-check.
    fn expression_is_assignable(
        &self,
        expression: &Expr,
        vars: &HashMap<String, Type>,
        expected: &Type,
    ) -> bool {
        let pending = self.pending_diagnostic.borrow_mut().take();
        let required_capacity = self.required_list_capacity.borrow_mut().take();
        let mut probe_vars = vars.clone();
        let result = analyze_expr_expected(self, expression, &mut probe_vars, Some(expected))
            .and_then(|mut value| ensure_assignable_and_coerce(expected, &mut value));
        self.pending_diagnostic.replace(pending);
        self.required_list_capacity.replace(required_capacity);
        result.is_ok()
    }

    fn reset(&self) {
        self.structs.borrow_mut().clear();
        self.states.borrow_mut().clear();
        self.consts.borrow_mut().clear();
        self.function_returns.borrow_mut().clear();
        self.function_modifiers.borrow_mut().clear();
        self.function_params.borrow_mut().clear();
        self.function_named_only_reasons.borrow_mut().clear();
        self.function_summaries.borrow_mut().clear();
        self.global_declarations.borrow_mut().clear();
        self.current_function_modifiers.borrow_mut().take();
        self.current_function_name.borrow_mut().take();
        self.current_mutable_bindings.borrow_mut().clear();
        self.trigger_callback_functions.borrow_mut().clear();
        self.current_state_param_names.borrow_mut().clear();
        self.error_codes.borrow_mut().clear();
        self.external_functions.borrow_mut().clear();
        self.external_states.borrow_mut().clear();
        self.resolved_arena.borrow_mut().take();
        self.resolved_binding_types.borrow_mut().clear();
        self.typed_hir_nodes.borrow_mut().clear();
        self.pending_diagnostic.borrow_mut().take();
        self.required_list_capacity.borrow_mut().take();
        self.resolved_named_types.borrow_mut().clear();
        self.resolved_named_type_resources.borrow_mut().clear();
        self.next_synthetic_binding.set(0);
    }
}

fn validate_declaration_uniqueness(program: &Program) -> Result<Vec<String>, SemanticError> {
    let mut functions = HashSet::new();
    let mut types = HashSet::new();
    let mut states = HashSet::new();
    let mut consts = HashSet::new();
    let mut triggers = HashSet::new();
    if is_reserved_source_type_declaration(&program.unit.name) {
        return Err(SemanticError {
            code: "E_RESERVED_DECLARATION",
            message: format!(
                "source unit `{}` uses a compiler-reserved name",
                program.unit.name
            ),
        });
    }
    let mut declarations = HashMap::from([(program.unit.name.clone(), "source unit")]);
    let mut global_error_codes = HashMap::new();
    let mut struct_names = Vec::new();

    let mut register_declaration = |name: &str,
                                    kind: &'static str,
                                    is_function: bool,
                                    is_type: bool|
     -> Result<(), SemanticError> {
        let reserved = if is_type {
            is_reserved_source_type_declaration(name)
        } else {
            is_reserved_source_declaration(name, is_function)
        };
        if reserved {
            return Err(SemanticError {
                code: "E_RESERVED_DECLARATION",
                message: format!("{kind} `{name}` uses a compiler-reserved name"),
            });
        }
        if let Some(previous_kind) = declarations.insert(name.to_owned(), kind) {
            return Err(SemanticError {
                code: "E_DUPLICATE_DECLARATION",
                message: format!("declaration name `{name}` is already used by a {previous_kind}"),
            });
        }
        Ok(())
    };

    for item in &program.items {
        match item {
            Item::Function(function) => {
                if !functions.insert(function.name.as_str()) {
                    return Err(SemanticError {
                        code: "K2001",
                        message: format!("duplicate function `{}`", function.name),
                    });
                }
                register_declaration(&function.name, "function", true, false)?;
                let mut params = HashSet::new();
                for param in &function.params {
                    if !params.insert(param.name.as_str()) {
                        return Err(SemanticError {
                            code: "K2001",
                            message: format!(
                                "duplicate parameter `{}` in function `{}`",
                                param.name, function.name
                            ),
                        });
                    }
                }
            }
            Item::Struct(definition) => {
                if !types.insert(definition.name.as_str()) {
                    return Err(SemanticError {
                        code: "K2001",
                        message: format!("duplicate type `{}`", definition.name),
                    });
                }
                register_declaration(&definition.name, "type", false, true)?;
                let mut fields = HashSet::new();
                for (field, _) in &definition.fields {
                    if !fields.insert(field.as_str()) {
                        return Err(SemanticError {
                            code: "K2001",
                            message: format!(
                                "duplicate field `{field}` in type `{}`",
                                definition.name
                            ),
                        });
                    }
                }
                struct_names.push(definition.name.clone());
            }
            Item::ErrorEnum(definition) => {
                if !types.insert(definition.name.as_str()) {
                    return Err(SemanticError {
                        code: "K2001",
                        message: format!("duplicate type `{}`", definition.name),
                    });
                }
                register_declaration(&definition.name, "type", false, true)?;
                let mut variants = HashSet::new();
                let mut codes = HashSet::new();
                for variant in &definition.variants {
                    if !variants.insert(variant.name.as_str()) {
                        return Err(SemanticError {
                            code: "K2001",
                            message: format!(
                                "duplicate error variant `{}::{}`",
                                definition.name, variant.name
                            ),
                        });
                    }
                    if !codes.insert(variant.code) {
                        return Err(SemanticError {
                            code: "K2001",
                            message: format!(
                                "duplicate error code {} in `{}`",
                                variant.code, definition.name
                            ),
                        });
                    }
                    if let Some(previous) = global_error_codes.insert(
                        variant.code,
                        format!("{}::{}", definition.name, variant.name),
                    ) {
                        return Err(SemanticError {
                            code: "E_DUPLICATE_ERROR_CODE",
                            message: format!(
                                "error code {} is assigned to both `{previous}` and `{}::{}`",
                                variant.code, definition.name, variant.name
                            ),
                        });
                    }
                }
            }
            Item::State(state) => {
                if !states.insert(state.name.as_str()) {
                    return Err(SemanticError {
                        code: "K2001",
                        message: format!("duplicate state `{}`", state.name),
                    });
                }
                register_declaration(&state.name, "state declaration", false, false)?;
            }
            Item::Const(constant) => {
                if !consts.insert(constant.name.as_str()) {
                    return Err(SemanticError {
                        code: "K2001",
                        message: format!("duplicate const `{}`", constant.name),
                    });
                }
                register_declaration(&constant.name, "const declaration", false, false)?;
            }
            Item::Trigger(trigger) => {
                if !triggers.insert(trigger.name.as_str()) {
                    return Err(SemanticError {
                        code: "K2001",
                        message: format!("duplicate trigger `{}`", trigger.name),
                    });
                }
                register_declaration(&trigger.name, "trigger declaration", false, false)?;
            }
        }
    }

    Ok(struct_names)
}

fn collect_struct_dependencies(
    ty: &Type,
    known_structs: &HashSet<String>,
    dependencies: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let mut pending = vec![ty];
    while let Some(current) = pending.pop() {
        match current {
            Type::NamedStruct(name) | Type::Struct { name, .. } => {
                if known_structs.contains(name) && seen.insert(name.clone()) {
                    dependencies.push(name.clone());
                }
            }
            Type::StateMap(key, value) => {
                pending.push(value);
                pending.push(key);
            }
            Type::Secret(inner) => pending.push(inner),
            Type::Option(inner) => pending.push(inner),
            Type::List(element, _) => pending.push(element),
            Type::Result(ok, err) => {
                pending.push(err);
                pending.push(ok);
            }
            Type::Tuple(items) => pending.extend(items.iter().rev()),
            Type::Int
            | Type::Decimal
            | Type::Quantity
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::DataSpaceId
            | Type::AxtDescriptor
            | Type::AssetHandle
            | Type::ProofBlob
            | Type::SoracloudRequest
            | Type::SoracloudResponse
            | Type::AccountId
            | Type::AssetDefinitionId
            | Type::AssetId
            | Type::NftId
            | Type::DomainId
            | Type::Name
            | Type::Json
            | Type::Unit => {}
        }
    }
}

fn value_struct_cycle(context: &SemanticContext, struct_names: &[String]) -> Option<Vec<String>> {
    let definitions = context.structs.borrow().clone();
    let known_structs = struct_names.iter().cloned().collect::<HashSet<_>>();
    let mut graph = HashMap::new();
    for name in struct_names {
        let mut dependencies = Vec::new();
        let mut seen = HashSet::new();
        if let Some(fields) = definitions.get(name) {
            for (_, ty) in fields {
                collect_struct_dependencies(ty, &known_structs, &mut dependencies, &mut seen);
            }
        }
        graph.insert(name.clone(), dependencies);
    }

    // Use an explicit DFS stack so malformed recursive value types cannot
    // overflow the compiler stack before they are rejected.
    let mut visit_state = struct_names
        .iter()
        .cloned()
        .map(|name| (name, 0_u8))
        .collect::<HashMap<_, _>>();
    for root in struct_names {
        if visit_state.get(root).copied().unwrap_or_default() != 0 {
            continue;
        }
        visit_state.insert(root.clone(), 1);
        let mut path = vec![root.clone()];
        let mut stack = vec![(root.clone(), 0_usize)];
        while !stack.is_empty() {
            let next_dependency = {
                let (current, next_index) = stack.last_mut().expect("stack is not empty");
                let dependencies = graph
                    .get(current)
                    .expect("every declared struct has a graph node");
                if let Some(dependency) = dependencies.get(*next_index) {
                    *next_index += 1;
                    Some(dependency.clone())
                } else {
                    None
                }
            };

            if let Some(dependency) = next_dependency {
                match visit_state.get(&dependency).copied().unwrap_or_default() {
                    0 => {
                        visit_state.insert(dependency.clone(), 1);
                        path.push(dependency.clone());
                        stack.push((dependency, 0));
                    }
                    1 => {
                        let cycle_start = path
                            .iter()
                            .position(|name| name == &dependency)
                            .expect("visiting structs are present in the active path");
                        let mut cycle = path[cycle_start..].to_vec();
                        cycle.push(dependency);
                        return Some(cycle);
                    }
                    _ => {}
                }
                continue;
            }

            let (finished, _) = stack.pop().expect("stack is not empty");
            let path_entry = path.pop().expect("active path mirrors DFS stack");
            debug_assert_eq!(finished, path_entry);
            visit_state.insert(finished, 2);
        }
    }

    None
}

fn value_struct_cycle_error(cycle: &[String]) -> SemanticError {
    SemanticError {
        code: "K2006",
        message: format!("cyclic value struct definition: {}", cycle.join(" -> ")),
    }
}

fn validate_acyclic_value_structs(
    context: &SemanticContext,
    struct_names: &[String],
) -> Result<(), SemanticError> {
    value_struct_cycle(context, struct_names)
        .map_or(Ok(()), |cycle| Err(value_struct_cycle_error(&cycle)))
}

#[derive(Clone, Copy, Debug, Default)]
struct ExpandedTypeResources {
    nodes: usize,
    depth: usize,
}

#[derive(Debug)]
struct StructResolutionBudgetError {
    owner: Option<String>,
    error: SemanticError,
}

struct StructResolutionPlan {
    order: Vec<String>,
    resources: HashMap<String, ExpandedTypeResources>,
}

fn capped_expanded_nodes(nodes: usize, additional: usize) -> usize {
    nodes
        .saturating_add(additional)
        .min(MAX_EXPANDED_TYPE_NODES.saturating_add(1))
}

/// Measure one type using already-memoized resources for every named dependency.
///
/// This walk is deliberately iterative. Named structs contribute their
/// memoized expanded shape without visiting it again, so a diamond-shaped DAG
/// takes time proportional to the source graph rather than its expanded tree.
fn measure_expanded_type(
    ty: &Type,
    named: &HashMap<String, ExpandedTypeResources>,
) -> ExpandedTypeResources {
    let mut resources = ExpandedTypeResources::default();
    let mut pending = vec![(ty, 1_usize)];
    while let Some((current, depth)) = pending.pop() {
        match current {
            Type::NamedStruct(name) => {
                let contribution = named
                    .get(name)
                    .copied()
                    .unwrap_or(ExpandedTypeResources { nodes: 1, depth: 1 });
                resources.nodes = capped_expanded_nodes(resources.nodes, contribution.nodes);
                resources.depth = resources
                    .depth
                    .max(depth.saturating_sub(1).saturating_add(contribution.depth));
            }
            Type::StateMap(key, value) | Type::Result(key, value) => {
                resources.nodes = capped_expanded_nodes(resources.nodes, 1);
                resources.depth = resources.depth.max(depth);
                pending.push((value, depth.saturating_add(1)));
                pending.push((key, depth.saturating_add(1)));
            }
            Type::Secret(inner) | Type::Option(inner) | Type::List(inner, _) => {
                resources.nodes = capped_expanded_nodes(resources.nodes, 1);
                resources.depth = resources.depth.max(depth);
                pending.push((inner, depth.saturating_add(1)));
            }
            Type::Tuple(items) => {
                resources.nodes = capped_expanded_nodes(resources.nodes, 1);
                resources.depth = resources.depth.max(depth);
                pending.extend(
                    items
                        .iter()
                        .rev()
                        .map(|item| (item, depth.saturating_add(1))),
                );
            }
            Type::Struct { name, fields } => {
                if let Some(contribution) = named.get(name) {
                    resources.nodes = capped_expanded_nodes(resources.nodes, contribution.nodes);
                    resources.depth = resources
                        .depth
                        .max(depth.saturating_sub(1).saturating_add(contribution.depth));
                    continue;
                }
                resources.nodes = capped_expanded_nodes(resources.nodes, 1);
                resources.depth = resources.depth.max(depth);
                pending.extend(
                    fields
                        .iter()
                        .rev()
                        .map(|(_, field)| (field, depth.saturating_add(1))),
                );
            }
            Type::Int
            | Type::Decimal
            | Type::Quantity
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::DataSpaceId
            | Type::AxtDescriptor
            | Type::AssetHandle
            | Type::ProofBlob
            | Type::SoracloudRequest
            | Type::SoracloudResponse
            | Type::AccountId
            | Type::AssetDefinitionId
            | Type::AssetId
            | Type::NftId
            | Type::DomainId
            | Type::Name
            | Type::Json
            | Type::Unit => {
                resources.nodes = capped_expanded_nodes(resources.nodes, 1);
                resources.depth = resources.depth.max(depth);
            }
        }
    }
    resources
}

/// Prove that materializing the acyclic named-type graph fits fixed V1 limits.
///
/// The dependency graph is processed leaf-first. Each named shape is measured
/// once and memoized; neither a deep chain nor an exponentially branching DAG
/// is recursively expanded during this proof. The existing materializer runs
/// only after the proof, when its output depth and aggregate allocation are
/// known to be bounded.
fn validate_struct_resolution_budget(
    context: &SemanticContext,
    local_struct_names: &[String],
) -> Result<StructResolutionPlan, StructResolutionBudgetError> {
    let definitions = context.structs.borrow();
    let known_structs = definitions.keys().cloned().collect::<HashSet<_>>();
    let mut graph = BTreeMap::<String, Vec<String>>::new();
    for (name, fields) in definitions.iter() {
        let mut dependencies = Vec::new();
        let mut seen = HashSet::new();
        for (_, ty) in fields {
            collect_struct_dependencies(ty, &known_structs, &mut dependencies, &mut seen);
        }
        dependencies.sort();
        graph.insert(name.clone(), dependencies);
    }

    let mut unresolved_dependencies = graph
        .iter()
        .map(|(name, dependencies)| (name.clone(), dependencies.len()))
        .collect::<HashMap<_, _>>();
    let mut dependents = HashMap::<String, Vec<String>>::new();
    for (owner, dependencies) in &graph {
        for dependency in dependencies {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(owner.clone());
        }
    }
    for owners in dependents.values_mut() {
        owners.sort();
    }
    let mut ready = unresolved_dependencies
        .iter()
        .filter_map(|(name, count)| (*count == 0).then_some(name.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(graph.len());
    while let Some(name) = ready.pop_first() {
        order.push(name.clone());
        if let Some(owners) = dependents.get(&name) {
            for owner in owners {
                let Some(remaining) = unresolved_dependencies.get_mut(owner) else {
                    continue;
                };
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    ready.insert(owner.clone());
                }
            }
        }
    }
    if order.len() != graph.len() {
        return Err(StructResolutionBudgetError {
            owner: None,
            error: SemanticError {
                code: "K2006",
                message: "cyclic value struct definition reached named-type resolution".into(),
            },
        });
    }

    let mut measured = HashMap::<String, ExpandedTypeResources>::new();
    for name in &order {
        let mut resources = ExpandedTypeResources { nodes: 1, depth: 1 };
        if let Some(fields) = definitions.get(name) {
            for (_, field_ty) in fields {
                let field = measure_expanded_type(field_ty, &measured);
                resources.nodes = capped_expanded_nodes(resources.nodes, field.nodes);
                resources.depth = resources.depth.max(field.depth.saturating_add(1));
            }
        }
        measured.insert(name.clone(), resources);
    }

    let mut roots = local_struct_names.to_vec();
    roots.sort();
    for name in &roots {
        let resources = measured.get(name).copied().unwrap_or_default();
        if resources.depth > MAX_NESTING_DEPTH {
            return Err(StructResolutionBudgetError {
                owner: Some(name.clone()),
                error: SemanticError {
                    code: "K2008",
                    message: format!(
                        "expanded value type `{name}` exceeds the V1 nesting limit of {MAX_NESTING_DEPTH} levels"
                    ),
                },
            });
        }
        if resources.nodes > MAX_EXPANDED_TYPE_NODES {
            return Err(StructResolutionBudgetError {
                owner: Some(name.clone()),
                error: SemanticError {
                    code: "K2008",
                    message: format!(
                        "expanded value type `{name}` exceeds the V1 resource limit of {MAX_EXPANDED_TYPE_NODES} type nodes"
                    ),
                },
            });
        }
    }

    let total = roots.iter().fold(0_usize, |total, name| {
        capped_expanded_nodes(
            total,
            measured.get(name).map_or(0, |resources| resources.nodes),
        )
    });
    if total > MAX_EXPANDED_TYPE_NODES {
        return Err(StructResolutionBudgetError {
            owner: roots.first().cloned(),
            error: SemanticError {
                code: "K2008",
                message: format!(
                    "expanded value struct declarations exceed the V1 resource limit of {MAX_EXPANDED_TYPE_NODES} type nodes"
                ),
            },
        });
    }
    Ok(StructResolutionPlan {
        order,
        resources: measured,
    })
}

fn canonicalize_named_type(ty: &Type, resolved: &HashMap<String, Type>) -> Type {
    match ty {
        Type::NamedStruct(name) => resolved.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::StateMap(key, value) => Type::StateMap(
            Box::new(canonicalize_named_type(key, resolved)),
            Box::new(canonicalize_named_type(value, resolved)),
        ),
        Type::Option(inner) => Type::Option(Box::new(canonicalize_named_type(inner, resolved))),
        Type::Result(ok, err) => Type::Result(
            Box::new(canonicalize_named_type(ok, resolved)),
            Box::new(canonicalize_named_type(err, resolved)),
        ),
        Type::List(element, capacity) => Type::List(
            Box::new(canonicalize_named_type(element, resolved)),
            *capacity,
        ),
        Type::Secret(inner) => Type::Secret(Box::new(canonicalize_named_type(inner, resolved))),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| canonicalize_named_type(item, resolved))
                .collect(),
        ),
        // Resolved product nodes are immutable and shared. Rewalking their
        // fields would turn a canonical DAG back into an expanded tree.
        Type::Struct { .. } => ty.clone(),
        _ => ty.clone(),
    }
}

fn install_canonical_struct_types(context: &SemanticContext, plan: StructResolutionPlan) {
    let StructResolutionPlan { order, resources } = plan;
    let definitions = context.structs.borrow().clone();
    let mut resolved = HashMap::<String, Type>::new();
    for name in order {
        let fields = definitions.get(&name).map_or_else(Vec::new, |fields| {
            fields
                .iter()
                .map(|(field, ty)| (field.clone(), canonicalize_named_type(ty, &resolved)))
                .collect()
        });
        resolved.insert(
            name.clone(),
            Type::Struct {
                name,
                fields: Arc::from(fields),
            },
        );
    }
    let struct_fields = resolved
        .iter()
        .filter_map(|(name, ty)| {
            let Type::Struct { fields, .. } = ty else {
                return None;
            };
            Some((name.clone(), fields.to_vec()))
        })
        .collect();
    context.structs.replace(struct_fields);
    context.resolved_named_types.replace(resolved);
    context.resolved_named_type_resources.replace(resources);
}

fn type_expr_mentions_name(ty: &TypeExpr, expected: &str) -> bool {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty.kind() {
            TypeExpr::Path(name) if name == expected => return true,
            TypeExpr::Generic { args, .. } | TypeExpr::Tuple(args) => {
                pending.extend(args.iter().rev());
            }
            TypeExpr::Path(_) | TypeExpr::Const(_) => {}
            TypeExpr::Source { .. } | TypeExpr::Resolved { .. } => {
                unreachable!("kind() strips AST and resolved-HIR provenance wrappers")
            }
        }
    }
    false
}

fn recursive_function_call_cycle(context: &SemanticContext) -> Option<Vec<String>> {
    let summaries = context.function_summaries.borrow().clone();
    let mut function_names = summaries.keys().cloned().collect::<Vec<_>>();
    function_names.sort();
    let mut visit_state = function_names
        .iter()
        .cloned()
        .map(|name| (name, 0_u8))
        .collect::<HashMap<_, _>>();

    for root in &function_names {
        if visit_state.get(root).copied().unwrap_or_default() != 0 {
            continue;
        }
        visit_state.insert(root.clone(), 1);
        let mut path = vec![root.clone()];
        let mut stack = vec![(root.clone(), 0_usize)];
        while !stack.is_empty() {
            let next = {
                let (current, index) = stack.last_mut().expect("non-empty DFS stack");
                let calls = &summaries
                    .get(current)
                    .expect("declared function has a summary")
                    .calls;
                if let Some(callee) = calls.get_index(*index) {
                    *index += 1;
                    summaries.contains_key(callee).then(|| callee.clone())
                } else {
                    None
                }
            };

            if let Some(callee) = next {
                match visit_state.get(&callee).copied().unwrap_or_default() {
                    0 => {
                        visit_state.insert(callee.clone(), 1);
                        path.push(callee.clone());
                        stack.push((callee, 0));
                    }
                    1 => {
                        let start = path
                            .iter()
                            .position(|name| name == &callee)
                            .expect("active callee is present in DFS path");
                        let mut cycle = path[start..].to_vec();
                        cycle.push(callee);
                        return Some(cycle);
                    }
                    _ => {}
                }
                continue;
            }

            let (finished, _) = stack.pop().expect("non-empty DFS stack");
            path.pop().expect("DFS path mirrors stack");
            visit_state.insert(finished, 2);
        }
    }
    None
}

fn recursive_function_call_error(cycle: &[String]) -> SemanticError {
    SemanticError {
        code: "K2006",
        message: format!(
            "recursive function calls are not supported in Kotodama V1: {}",
            cycle.join(" -> ")
        ),
    }
}

fn validate_acyclic_function_calls(context: &SemanticContext) -> Result<(), SemanticError> {
    recursive_function_call_cycle(context)
        .map_or(Ok(()), |cycle| Err(recursive_function_call_error(&cycle)))
}

/// Analyze a parsed program in a fresh per-compilation semantic context.
pub fn analyze(program: &Program) -> Result<TypedProgram, SemanticError> {
    SemanticContext::new().analyze(program)
}

fn reject_test_surface_without_test_mode(
    context: &SemanticContext,
    program: &Program,
) -> Result<(), SemanticFailures> {
    if context.test_builtins_enabled {
        return Ok(());
    }

    let mut failures = Vec::new();
    let mut omitted = 0_usize;
    if program.test_target.is_some() {
        record_semantic_failure(
            &mut failures,
            &mut omitted,
            SemanticFailure {
                error: SemanticError {
                    code: "E_TEST_ONLY_PRODUCTION",
                    message: "`koto_test` declarations require explicit compiler test mode".into(),
                },
                location: None,
                diagnostic: None,
            },
        );
    }
    for fixture in &program.fixtures {
        record_semantic_failure(
            &mut failures,
            &mut omitted,
            SemanticFailure {
                error: SemanticError {
                    code: "E_TEST_ONLY_PRODUCTION",
                    message: format!(
                        "fixture `{}` requires explicit compiler test mode",
                        fixture.name
                    ),
                },
                location: None,
                diagnostic: None,
            },
        );
    }
    for item in &program.items {
        let Item::Function(function) = item else {
            continue;
        };
        if function.modifiers.is_test || function.modifiers.test_fixture.is_some() {
            record_semantic_failure(
                &mut failures,
                &mut omitted,
                SemanticFailure {
                    error: SemanticError {
                        code: "E_TEST_ONLY_PRODUCTION",
                        message: format!(
                            "test function `{}` requires explicit compiler test mode",
                            function.name
                        ),
                    },
                    location: Some(function.location),
                    diagnostic: None,
                },
            );
        }
    }
    if omitted != 0 {
        failures.push(SemanticFailure {
            error: SemanticError {
                code: "K0004",
                message: format!("{omitted} additional semantic error(s) were omitted"),
            },
            location: None,
            diagnostic: None,
        });
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(SemanticFailures { failures })
    }
}

/// Revalidate whole-program invariants after independently typed modules have
/// been linked into one HIR program.
///
/// Module analysis deliberately cannot trust an imported callee's body. This
/// pass rebuilds the complete call graph from final linked symbols, rejects
/// cross-module recursion, and propagates view/authorization effects through
/// every linked helper before code generation.
pub fn validate_linked_program(
    program: &TypedProgram,
    zk_enabled: bool,
) -> Result<(), SemanticError> {
    for state in &program.states {
        validate_list_schemas(&state.ty)?;
    }
    for item in &program.items {
        let TypedItem::Function(function) = item;
        validate_list_schemas(function.ret_ty.as_ref().unwrap_or(&Type::Unit))?;
        for parameter in &function.param_types {
            validate_list_schemas(&parameter.ty)?;
        }
    }

    let context = SemanticContext::with_zk_enabled(zk_enabled);
    context.states.replace(
        program
            .states
            .iter()
            .map(|state| (state.name.clone(), state.ty.clone()))
            .collect(),
    );

    let mut returns = HashMap::new();
    for item in &program.items {
        let TypedItem::Function(function) = item;
        if returns
            .insert(
                function.name.clone(),
                function.ret_ty.clone().unwrap_or(Type::Unit),
            )
            .is_some()
        {
            return Err(SemanticError {
                code: "K2001",
                message: format!("duplicate linked function `{}`", function.name),
            });
        }
    }
    context.function_returns.replace(returns);

    for item in &program.items {
        let TypedItem::Function(function) = item;
        context.current_state_param_names.replace(
            function
                .param_types
                .iter()
                .filter(|param| param.is_state)
                .map(|param| param.name.clone())
                .collect(),
        );
        let summary = FunctionSummary {
            direct_effects: FunctionEffects {
                host_side_effects: block_contains_host_side_effects(&function.body),
                emits_instructions: block_contains_instruction_emission(&function.body),
                mutates_durable_state: block_mutates_durable_state(&context, &function.body),
            },
            calls: collect_called_functions(&context, &function.body),
        };
        context
            .function_summaries
            .borrow_mut()
            .insert(function.name.clone(), summary);
    }
    context.current_state_param_names.borrow_mut().clear();

    validate_acyclic_function_calls(&context)?;
    validate_scalar_state_initialization(&context, &program.items, &program.states)?;
    crate::secret::validate_program(program, zk_enabled)?;
    enforce_permission_requirements(&context, &program.items)
}

/// Derive the production HIR from a test-capable target without returning to AST.
///
/// Only declarations originating in the deployable target are supplied here;
/// standalone test-module HIR is linked into the suite separately. The
/// projection removes inline `#[test]` functions, proves that every retained
/// call still resolves, rejects retained test-only builtins, and only then
/// clears test provenance.
pub(crate) fn project_test_target_to_production(
    mut target: TypedProgram,
    zk_enabled: bool,
) -> Result<TypedProgram, SemanticError> {
    let removed = target
        .items
        .iter()
        .filter_map(|item| {
            let TypedItem::Function(function) = item;
            function.modifiers.is_test.then(|| function.name.clone())
        })
        .collect::<HashSet<_>>();
    target.items.retain(|item| {
        let TypedItem::Function(function) = item;
        !function.modifiers.is_test
    });
    let retained = target
        .items
        .iter()
        .map(|item| {
            let TypedItem::Function(function) = item;
            function.name.clone()
        })
        .collect::<HashSet<_>>();
    for item in &target.items {
        let TypedItem::Function(function) = item;
        validate_production_projection_block(
            &function.body,
            &function.name,
            &retained,
            &removed,
            zk_enabled,
        )?;
    }
    target.test_support_enabled = false;
    validate_linked_program(&target, zk_enabled)?;
    Ok(target)
}

fn validate_production_projection_block(
    block: &TypedBlock,
    owner: &str,
    retained: &HashSet<String>,
    removed: &HashSet<String>,
    zk_enabled: bool,
) -> Result<(), SemanticError> {
    for statement in &block.statements {
        validate_production_projection_statement(statement, owner, retained, removed, zk_enabled)?;
    }
    if let Some(tail) = &block.tail {
        validate_production_projection_expr(tail, owner, retained, removed, zk_enabled)?;
    }
    Ok(())
}

fn validate_production_projection_statement(
    statement: &TypedStatement,
    owner: &str,
    retained: &HashSet<String>,
    removed: &HashSet<String>,
    zk_enabled: bool,
) -> Result<(), SemanticError> {
    match statement.kind() {
        TypedStatement::Let { value, .. } | TypedStatement::Expr(value) => {
            validate_production_projection_expr(value, owner, retained, removed, zk_enabled)
        }
        TypedStatement::Return(value) => {
            if let Some(value) = value {
                validate_production_projection_expr(value, owner, retained, removed, zk_enabled)?;
            }
            Ok(())
        }
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            validate_production_projection_expr(cond, owner, retained, removed, zk_enabled)?;
            validate_production_projection_block(
                then_branch,
                owner,
                retained,
                removed,
                zk_enabled,
            )?;
            if let Some(else_branch) = else_branch {
                validate_production_projection_block(
                    else_branch,
                    owner,
                    retained,
                    removed,
                    zk_enabled,
                )?;
            }
            Ok(())
        }
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            validate_production_projection_expr(value, owner, retained, removed, zk_enabled)?;
            validate_production_projection_block(
                then_branch,
                owner,
                retained,
                removed,
                zk_enabled,
            )?;
            if let Some(else_branch) = else_branch {
                validate_production_projection_block(
                    else_branch,
                    owner,
                    retained,
                    removed,
                    zk_enabled,
                )?;
            }
            Ok(())
        }
        TypedStatement::While { cond, body } => {
            validate_production_projection_expr(cond, owner, retained, removed, zk_enabled)?;
            validate_production_projection_block(body, owner, retained, removed, zk_enabled)
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                validate_production_projection_statement(
                    init, owner, retained, removed, zk_enabled,
                )?;
            }
            if let Some(cond) = cond {
                validate_production_projection_expr(cond, owner, retained, removed, zk_enabled)?;
            }
            if let Some(step) = step {
                validate_production_projection_statement(
                    step, owner, retained, removed, zk_enabled,
                )?;
            }
            validate_production_projection_block(body, owner, retained, removed, zk_enabled)
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            validate_production_projection_expr(map, owner, retained, removed, zk_enabled)?;
            validate_production_projection_block(body, owner, retained, removed, zk_enabled)
        }
        TypedStatement::MapSet { map, key, value } => {
            validate_production_projection_expr(map, owner, retained, removed, zk_enabled)?;
            validate_production_projection_expr(key, owner, retained, removed, zk_enabled)?;
            validate_production_projection_expr(value, owner, retained, removed, zk_enabled)
        }
        TypedStatement::Break | TypedStatement::Continue => Ok(()),
    }
}

fn validate_production_projection_expr(
    expression: &TypedExpr,
    owner: &str,
    retained: &HashSet<String>,
    removed: &HashSet<String>,
    zk_enabled: bool,
) -> Result<(), SemanticError> {
    let recurse = |expression: &TypedExpr| {
        validate_production_projection_expr(expression, owner, retained, removed, zk_enabled)
    };
    match expression.kind() {
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
            if let Some(builtin) = Builtin::from_name(name) {
                match builtin.mode() {
                    crate::builtins::BuiltinMode::TestOnly
                    | crate::builtins::BuiltinMode::TestFunctionOnly => {
                        return Err(SemanticError {
                            code: "E_TEST_ONLY_PRODUCTION",
                            message: format!(
                                "retained function `{owner}` calls test-only builtin `{}`",
                                builtin.source_name()
                            ),
                        });
                    }
                    crate::builtins::BuiltinMode::ZkOnly if !zk_enabled => {
                        return Err(SemanticError {
                            code: "E_ZK_MODE_REQUIRED",
                            message: format!(
                                "retained function `{owner}` calls ZK-only builtin `{}` without ZK build policy",
                                builtin.source_name()
                            ),
                        });
                    }
                    _ => {}
                }
            } else if removed.contains(name) {
                return Err(SemanticError {
                    code: "E_TEST_ONLY_PRODUCTION",
                    message: format!(
                        "retained function `{owner}` calls removed test function `{name}`"
                    ),
                });
            } else if !retained.contains(name) && compiler_intrinsic_kind(name).is_none() {
                return Err(SemanticError {
                    code: "K2002",
                    message: format!(
                        "linked function `{owner}` calls unknown function `{name}` after test projection"
                    ),
                });
            }
            for argument in args {
                recurse(argument)?;
            }
            Ok(())
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            target: left,
            index: right,
        } => {
            recurse(left)?;
            recurse(right)
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::Member { object: expr, .. }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => recurse(expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            recurse(cond)?;
            recurse(then_expr)?;
            recurse(else_expr)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            recurse(condition)?;
            validate_production_projection_block(
                then_branch,
                owner,
                retained,
                removed,
                zk_enabled,
            )?;
            validate_production_projection_block(else_branch, owner, retained, removed, zk_enabled)
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            recurse(value)?;
            validate_production_projection_block(
                then_branch,
                owner,
                retained,
                removed,
                zk_enabled,
            )?;
            validate_production_projection_block(else_branch, owner, retained, removed, zk_enabled)
        }
        ExprKind::Match { value, arms } => {
            recurse(value)?;
            for arm in arms {
                validate_production_projection_block(
                    &arm.body, owner, retained, removed, zk_enabled,
                )?;
            }
            Ok(())
        }
        ExprKind::Tuple(items) | ExprKind::List(items) | ExprKind::JsonArray(items) => {
            for item in items {
                recurse(item)?;
            }
            Ok(())
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            recurse(source)?;
            recurse(expression)?;
            if let Some(condition) = condition {
                recurse(condition)?;
            }
            Ok(())
        }
        ExprKind::StructLiteral { fields, .. } | ExprKind::JsonObject(fields) => {
            for (_, value) in fields {
                recurse(value)?;
            }
            Ok(())
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => Ok(()),
    }
}

fn analyze_with_context(
    context: &SemanticContext,
    program: &Program,
) -> Result<TypedProgram, SemanticFailures> {
    reject_test_surface_without_test_mode(context, program)?;
    let external_structs = context.structs.borrow().clone();
    let external_consts = context.consts.borrow().clone();
    let external_error_codes = context.error_codes.borrow().clone();
    // Collect definitions up front so source order does not affect name resolution.
    let mut structs = external_structs.clone();
    let mut state_decls: Vec<(String, TypeExpr)> = Vec::new();
    let mut const_decls: Vec<ConstDecl> = Vec::new();
    let mut fn_returns: HashMap<String, Type> = HashMap::new();
    let mut fn_return_sources = HashMap::new();
    let mut fn_modifiers = context
        .external_functions
        .borrow()
        .iter()
        .map(|(name, signature)| (name.clone(), signature.modifiers.clone()))
        .collect::<HashMap<_, _>>();
    let mut trigger_callbacks: HashSet<String> = HashSet::new();
    let mut error_codes = external_error_codes;
    let mut typed_error_codes = Vec::new();
    let struct_names = validate_declaration_uniqueness(program)?;
    let mut global_declarations = std::iter::once(program.unit.name.clone())
        .chain(program.items.iter().map(|item| match item {
            Item::Function(function) => function.name.clone(),
            Item::Struct(definition) => definition.name.clone(),
            Item::ErrorEnum(definition) => definition.name.clone(),
            Item::Const(constant) => constant.name.clone(),
            Item::State(state) => state.name.clone(),
            Item::Trigger(trigger) => trigger.name.clone(),
        }))
        .collect::<HashSet<_>>();
    global_declarations.extend(context.external_functions.borrow().keys().cloned());
    global_declarations.extend(context.external_states.borrow().keys().cloned());
    global_declarations.extend(external_structs.keys().cloned());
    global_declarations.extend(external_consts.keys().cloned());
    context.global_declarations.replace(global_declarations);
    let mut known_structs = external_structs;
    known_structs.extend(struct_names.iter().cloned().map(|name| (name, Vec::new())));
    context.structs.replace(known_structs);
    for item in &program.items {
        match item {
            Item::Struct(def) => {
                let mut fields = Vec::new();
                for (name, ty_expr) in &def.fields {
                    fields.push((name.clone(), convert_type_expr(context, ty_expr)?));
                }
                structs.insert(def.name.clone(), fields);
            }
            Item::ErrorEnum(definition) => {
                for variant in &definition.variants {
                    error_codes.insert(
                        format!("{}::{}", definition.name, variant.name),
                        variant.code,
                    );
                    typed_error_codes.push(TypedErrorCode {
                        namespace: definition.name.clone(),
                        name: variant.name.clone(),
                        code: variant.code,
                    });
                }
            }
            Item::State(st) => {
                state_decls.push((st.name.clone(), st.ty.clone()));
            }
            Item::Const(decl) => {
                const_decls.push(decl.clone());
            }
            Item::Function(f) => {
                let ret = if let Some(ret_ty) = &f.ret_ty {
                    fn_return_sources.insert(f.name.clone(), context.type_source(ret_ty));
                    convert_type_expr(context, ret_ty)?
                } else {
                    Type::Unit
                };
                fn_returns.insert(f.name.clone(), ret);
                fn_modifiers.insert(f.name.clone(), f.modifiers.clone());
            }
            Item::Trigger(trigger) if trigger.call.namespace.is_none() => {
                trigger_callbacks.insert(trigger.call.entrypoint.clone());
            }
            Item::Trigger(_) => {}
        }
    }
    context.structs.replace(structs);
    context.error_codes.replace(error_codes);
    if let Some(cycle) = value_struct_cycle(context, &struct_names) {
        if let [owner, dependency, ..] = cycle.as_slice()
            && let Some(ty) = program.items.iter().find_map(|item| {
                let Item::Struct(definition) = item else {
                    return None;
                };
                (definition.name == *owner).then(|| {
                    definition
                        .fields
                        .iter()
                        .map(|(_, ty)| ty)
                        .find(|ty| type_expr_mentions_name(ty, dependency))
                })?
            })
        {
            context.capture_diagnostic(context.type_source(ty), None);
        }
        return Err(value_struct_cycle_error(&cycle).into());
    }
    let resolution_plan = match validate_struct_resolution_budget(context, &struct_names) {
        Ok(plan) => plan,
        Err(failure) => {
            if let Some(owner) = failure.owner.as_deref()
                && let Some(ty) = program
                    .items
                    .iter()
                    .find_map(|item| {
                        let Item::Function(function) = item else {
                            return None;
                        };
                        function
                            .params
                            .iter()
                            .filter_map(|param| param.ty.as_ref())
                            .chain(function.ret_ty.iter())
                            .find(|ty| type_expr_mentions_name(ty, owner))
                    })
                    .or_else(|| {
                        program.items.iter().find_map(|item| {
                            let Item::Struct(definition) = item else {
                                return None;
                            };
                            if definition.name == owner {
                                definition.fields.first().map(|(_, ty)| ty)
                            } else {
                                None
                            }
                        })
                    })
            {
                context.capture_diagnostic(context.type_source(ty), None);
            }
            return Err(failure.error.into());
        }
    };
    install_canonical_struct_types(context, resolution_plan);
    validate_declared_struct_list_schemas(context)?;
    let mut fn_returns = fn_returns
        .into_iter()
        .map(|(name, ty)| {
            let ty = resolve_struct_type_with_context(context, &ty).inspect_err(|_| {
                context.capture_diagnostic(fn_return_sources.get(&name).copied().flatten(), None);
            })?;
            Ok((name, ty))
        })
        .collect::<Result<HashMap<_, _>, SemanticError>>()?;
    let mut return_names = fn_returns.keys().collect::<Vec<_>>();
    return_names.sort();
    for name in return_names {
        validate_list_schemas(
            fn_returns
                .get(name)
                .expect("collected function name remains in the return table"),
        )?;
    }
    for (name, signature) in context.external_functions.borrow().iter() {
        validate_list_schemas(&signature.return_type)?;
        for parameter in &signature.params {
            validate_list_schemas(&parameter.ty)?;
        }
        if fn_returns
            .insert(name.clone(), signature.return_type.clone())
            .is_some()
        {
            return Err(SemanticError {
                code: "E_DUPLICATE_DECLARATION",
                message: format!("imported function `{name}` collides with a local function"),
            }
            .into());
        }
    }
    let mut resolved_consts = external_consts;
    for decl in const_decls {
        context.discard_diagnostic();
        let declared = decl.ty.as_ref().ok_or_else(|| SemanticError {
            code: "K2003",
            message: format!("const `{}` requires an explicit type", decl.name),
        })?;
        let expected =
            resolve_struct_type_with_context(context, &convert_type_expr(context, declared)?)
                .inspect_err(|_| context.capture_diagnostic(context.type_source(declared), None))?;
        validate_list_schemas(&expected)?;
        let mut value =
            match analyze_const_expr(context, &decl.value, &resolved_consts, Some(&expected)) {
                Ok(value) => value,
                Err(error) => {
                    return Err(SemanticFailures {
                        failures: vec![SemanticFailure {
                            error,
                            location: None,
                            diagnostic: context.take_diagnostic(),
                        }],
                    });
                }
            };
        ensure_assignable_and_coerce(&expected, &mut value)?;
        if is_numeric_type(&value.ty) {
            value = fold_constant_numeric(&value)?;
        }
        resolved_consts.insert(decl.name, value);
    }
    context.consts.replace(resolved_consts);
    let mut state: IndexMap<String, Type> = IndexMap::new();
    for (name, ty_expr) in state_decls {
        let ty = resolve_struct_type_with_context(context, &convert_type_expr(context, &ty_expr)?)
            .inspect_err(|_| context.capture_diagnostic(context.type_source(&ty_expr), None))?;
        validate_list_schemas(&ty)?;
        if let Err(error) = validate_state_type(&ty) {
            context.capture_diagnostic(context.type_source(&ty_expr), None);
            return Err(error.into());
        }
        state.insert(name, ty);
    }
    let resolved_state: IndexMap<String, Type> = state
        .into_iter()
        .map(|(name, ty)| Ok((name, resolve_struct_type_with_context(context, &ty)?)))
        .collect::<Result<_, SemanticError>>()?;
    let mut all_states = context.external_states.borrow().clone();
    for (name, ty) in &resolved_state {
        if all_states.insert(name.clone(), ty.clone()).is_some() {
            return Err(SemanticError {
                code: "K2005",
                message: format!("target state `{name}` collides with a local state declaration"),
            }
            .into());
        }
    }
    context.states.replace(all_states);
    context.function_returns.replace(fn_returns);
    context.function_modifiers.replace(fn_modifiers.clone());
    context
        .trigger_callback_functions
        .replace(trigger_callbacks);
    let mut fn_params = context
        .external_functions
        .borrow()
        .iter()
        .map(|(name, signature)| (name.clone(), signature.params.clone()))
        .collect::<HashMap<_, _>>();
    for item in &program.items {
        let Item::Function(f) = item else { continue };
        let mut params = Vec::with_capacity(f.params.len());
        for param in &f.params {
            params.push(parse_declared_param_type(context, param, &f.modifiers)?);
        }
        fn_params.insert(f.name.clone(), params);
    }
    context.function_params.replace(fn_params);
    let source_summaries = program
        .items
        .iter()
        .filter_map(|item| {
            let Item::Function(function) = item else {
                return None;
            };
            let mut summary = FunctionSummary::default();
            collect_source_block_summary(&function.body, &mut summary);
            Some((function.name.clone(), summary))
        })
        .collect::<HashMap<_, _>>();
    let transitive_effects = compute_transitive_effects(&source_summaries);
    let mut named_only_reasons = context
        .external_functions
        .borrow()
        .iter()
        .filter_map(|(name, signature)| {
            signature.requires_named_arguments.then_some((
                name.clone(),
                "imported privileged or effectful calls with at least three parameters require names",
            ))
        })
        .collect::<HashMap<_, _>>();
    named_only_reasons.extend(program.items.iter().filter_map(|item| {
        let Item::Function(function) = item else {
            return None;
        };
        if function.params.len() < 3 {
            return None;
        }
        let privileged = function.modifiers.permission.is_some()
            || matches!(
                function.modifiers.kind,
                FunctionKind::Kotoage | FunctionKind::Hajimari | FunctionKind::Kaizen
            );
        let effectful = transitive_effects
            .get(&function.name)
            .copied()
            .is_some_and(FunctionEffects::requires_permission);
        (privileged || effectful).then_some((
            function.name.clone(),
            "privileged or effectful calls with at least three parameters require names",
        ))
    }));
    context
        .function_named_only_reasons
        .replace(named_only_reasons);

    let mut items = Vec::new();
    let states = resolved_state
        .iter()
        .map(|(name, ty)| TypedStateDecl {
            name: name.clone(),
            ty: ty.clone(),
            source: None,
        })
        .collect::<Vec<_>>();
    let mut triggers = Vec::new();
    let mut trigger_names: HashSet<String> = HashSet::new();
    let mut failures = Vec::new();
    let mut omitted_failures = 0_usize;
    for item in &program.items {
        match item {
            Item::Function(f) => match analyze_function(context, f) {
                Ok(function) => {
                    context.discard_diagnostic();
                    context.required_list_capacity.borrow_mut().take();
                    items.push(TypedItem::Function(function));
                }
                Err(error) => record_semantic_failure(
                    &mut failures,
                    &mut omitted_failures,
                    SemanticFailure {
                        error,
                        location: Some(f.location),
                        diagnostic: context.take_diagnostic(),
                    },
                ),
            },
            Item::Trigger(trigger) => {
                if !trigger_names.insert(trigger.name.clone()) {
                    record_semantic_failure(
                        &mut failures,
                        &mut omitted_failures,
                        SemanticFailure {
                            error: SemanticError {
                                code: "K2001",
                                message: format!("duplicate trigger `{}`", trigger.name),
                            },
                            location: Some(trigger.location),
                            diagnostic: None,
                        },
                    );
                    continue;
                }
                match analyze_trigger(trigger, &fn_modifiers) {
                    Ok(trigger) => triggers.push(trigger),
                    Err(error) => record_semantic_failure(
                        &mut failures,
                        &mut omitted_failures,
                        SemanticFailure {
                            error,
                            location: Some(trigger.location),
                            diagnostic: None,
                        },
                    ),
                }
            }
            Item::Struct(_) | Item::ErrorEnum(_) | Item::Const(_) | Item::State(_) => {}
        }
    }
    if omitted_failures != 0 {
        failures.push(SemanticFailure {
            error: SemanticError {
                code: "K0004",
                message: format!("{omitted_failures} additional semantic error(s) were omitted"),
            },
            location: None,
            diagnostic: None,
        });
    }
    if !failures.is_empty() {
        return Err(SemanticFailures { failures });
    }
    if let Some(cycle) = recursive_function_call_cycle(context) {
        let location = cycle.first().and_then(|name| {
            program.items.iter().find_map(|item| {
                let Item::Function(function) = item else {
                    return None;
                };
                (&function.name == name).then_some(function.location)
            })
        });
        return Err(SemanticFailures {
            failures: vec![SemanticFailure {
                error: recursive_function_call_error(&cycle),
                location,
                diagnostic: None,
            }],
        });
    }
    validate_scalar_state_initialization(context, &items, &states)?;
    let hir_nodes = context
        .resolved_arena
        .borrow()
        .as_ref()
        .map(|arena| {
            context
                .typed_hir_nodes
                .borrow()
                .iter()
                .filter_map(|(local, ty)| {
                    let node = arena.node(*local)?;
                    let id = TypedHirNodeId {
                        source: arena.source(),
                        local: *local,
                    };
                    Some((
                        id,
                        TypedHirNode {
                            id,
                            source: node.source,
                            target: node.target,
                            ty: ty.clone(),
                        },
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let typed_program = TypedProgram {
        unit: program.unit.clone(),
        items,
        states,
        error_codes: typed_error_codes,
        triggers,
        message_entries: Vec::new(),
        hir_nodes,
        source_files: BTreeMap::new(),
        test_support_enabled: context.test_builtins_enabled,
    };
    crate::secret::validate_program(&typed_program, context.zk_enabled)?;
    enforce_permission_requirements(context, &typed_program.items)?;
    Ok(typed_program)
}

fn core_query_view_name(ty: &Type) -> Option<&str> {
    let Type::Struct { name, .. } = ty else {
        return None;
    };
    let builtin = match name.as_str() {
        "AccountView" => Builtin::QueryGetAccount,
        "AssetView" => Builtin::QueryGetAsset,
        "AssetDefinitionView" => Builtin::QueryGetAssetDefinition,
        "DomainView" => Builtin::QueryGetDomain,
        "NftView" => Builtin::QueryGetNft,
        _ => return None,
    };
    (core_query_view_type(builtin).as_ref() == Some(ty)).then_some(name.as_str())
}

pub(crate) fn type_name(ty: &Type) -> String {
    match ty {
        Type::Int => "int".into(),
        Type::Decimal => "decimal".into(),
        Type::Quantity => "quantity".into(),
        Type::Bool => "bool".into(),
        Type::String => "string".into(),
        Type::Bytes => "bytes".into(),
        Type::DataSpaceId => "DataSpaceId".into(),
        Type::AxtDescriptor => "AxtDescriptor".into(),
        Type::AssetHandle => "AssetHandle".into(),
        Type::ProofBlob => "ProofBlob".into(),
        Type::SoracloudRequest => "SoracloudRequest".into(),
        Type::SoracloudResponse => "SoracloudResponse".into(),
        Type::AccountId => "AccountId".into(),
        Type::AssetDefinitionId => "AssetDefinitionId".into(),
        Type::AssetId => "AssetId".into(),
        Type::NftId => "NftId".into(),
        Type::DomainId => "DomainId".into(),
        Type::Name => "Name".into(),
        Type::Json => "Json".into(),
        Type::Unit => "()".into(),
        Type::Secret(inner) => format!("Secret<{}>", type_name(inner)),
        Type::StateMap(k, v) => format!("StateMap<{}, {}>", type_name(k), type_name(v)),
        Type::Option(inner) => format!("Option<{}>", type_name(inner)),
        Type::Result(ok, err) => format!("Result<{}, {}>", type_name(ok), type_name(err)),
        Type::List(element, capacity) => {
            format!("List<{}, {capacity}>", type_name(element))
        }
        Type::Tuple(ts) => {
            let parts: Vec<String> = ts.iter().map(type_name).collect();
            format!("({})", parts.join(", "))
        }
        Type::Struct { name, .. } => query_page_view_type(ty)
            .and_then(core_query_view_name)
            .map_or_else(
                || core_query_view_name(ty).map_or_else(|| format!("struct {name}"), str::to_owned),
                |view_name| format!("{QUERY_PAGE_TYPE_NAME}<{view_name}>"),
            ),
        Type::NamedStruct(s) => s.clone(),
    }
}

/// Render `ty` as a valid source annotation.
///
/// ABI descriptors derive their canonical names from their exact recursive
/// schemas at the compiler boundary; this helper deliberately renders ordinary
/// structs without the schema-only `struct ` prefix.
pub fn render_type_name(ty: &Type) -> String {
    render_source_type_name(ty)
}

fn render_source_type_name(ty: &Type) -> String {
    match ty {
        Type::Secret(inner) => format!("Secret<{}>", render_source_type_name(inner)),
        Type::StateMap(key, value) => format!(
            "StateMap<{}, {}>",
            render_source_type_name(key),
            render_source_type_name(value)
        ),
        Type::Option(inner) => format!("Option<{}>", render_source_type_name(inner)),
        Type::Result(ok, error) => format!(
            "Result<{}, {}>",
            render_source_type_name(ok),
            render_source_type_name(error)
        ),
        Type::List(element, capacity) => {
            format!("List<{}, {capacity}>", render_source_type_name(element))
        }
        Type::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(render_source_type_name)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Type::Struct { name, .. } => query_page_view_type(ty)
            .and_then(core_query_view_name)
            .map_or_else(|| name.clone(), |view| format!("QueryPage<{view}>")),
        Type::NamedStruct(name) => name.clone(),
        scalar => type_name(scalar),
    }
}

fn trigger_data_family_name(family: TriggerDataFamily) -> &'static str {
    match family {
        TriggerDataFamily::Peer => "peer",
        TriggerDataFamily::Domain => "domain",
        TriggerDataFamily::Account => "account",
        TriggerDataFamily::Asset => "asset",
        TriggerDataFamily::AssetDefinition => "asset_definition",
        TriggerDataFamily::Nft => "nft",
        TriggerDataFamily::Rwa => "rwa",
        TriggerDataFamily::Trigger => "trigger",
        TriggerDataFamily::Role => "role",
        TriggerDataFamily::Configuration => "configuration",
        TriggerDataFamily::Executor => "executor",
    }
}

fn named_data_event_kind(event: &TriggerDataEventKind) -> Option<&str> {
    match event {
        TriggerDataEventKind::Any => None,
        TriggerDataEventKind::Named(kind) => Some(kind.as_str()),
    }
}

fn duplicate_data_matcher_error(
    trigger_name: &str,
    family: TriggerDataFamily,
    key: &str,
) -> SemanticError {
    SemanticError {
        code: "E_TRIGGER_FILTER_DUPLICATE_MATCHER",
        message: format!(
            "trigger `{trigger_name}` has duplicate `{key}` matcher in `{}` data filter",
            trigger_data_family_name(family)
        ),
    }
}

fn invalid_data_matcher_literal<E>(
    trigger_name: &str,
    family: TriggerDataFamily,
    key: &str,
    raw: &str,
    err: E,
) -> SemanticError
where
    E: std::fmt::Display,
{
    SemanticError {
        code: "E_TRIGGER_FILTER_INVALID_LITERAL",
        message: format!(
            "trigger `{trigger_name}` has invalid `{key}` matcher literal `{raw}` in `{}` data filter: {err}",
            trigger_data_family_name(family)
        ),
    }
}

fn unsupported_data_matcher_error(
    trigger_name: &str,
    family: TriggerDataFamily,
    key: &str,
) -> SemanticError {
    SemanticError {
        code: "E_TRIGGER_FILTER_UNSUPPORTED_MATCHER",
        message: format!(
            "trigger `{trigger_name}` does not support `{key}` matcher in `{}` data filter",
            trigger_data_family_name(family)
        ),
    }
}

fn unsupported_data_event_kind_error(
    trigger_name: &str,
    family: TriggerDataFamily,
    kind: &str,
) -> SemanticError {
    SemanticError {
        code: "E_TRIGGER_FILTER_UNSUPPORTED_EVENT",
        message: format!(
            "trigger `{trigger_name}` does not support `{kind}` event kind for `{}` data filter",
            trigger_data_family_name(family)
        ),
    }
}

fn parse_peer_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<PeerId, SemanticError> {
    raw.parse()
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "peer", raw, err))
}

fn parse_domain_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<DomainId, SemanticError> {
    DomainId::parse_fully_qualified(raw)
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "domain", raw, err))
}

fn parse_account_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<AccountId, SemanticError> {
    AccountId::parse_encoded(raw)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "account", raw, err))
}

fn parse_asset_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<AssetId, SemanticError> {
    raw.parse()
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "asset", raw, err))
}

fn parse_asset_definition_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<AssetDefinitionId, SemanticError> {
    raw.parse().map_err(|err| {
        invalid_data_matcher_literal(trigger_name, family, "asset_definition", raw, err)
    })
}

fn parse_nft_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<NftId, SemanticError> {
    raw.parse()
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "nft", raw, err))
}

fn parse_rwa_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<RwaId, SemanticError> {
    raw.parse()
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "rwa", raw, err))
}

fn parse_trigger_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<TriggerId, SemanticError> {
    raw.parse()
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "trigger", raw, err))
}

fn parse_role_matcher(
    trigger_name: &str,
    family: TriggerDataFamily,
    raw: &str,
) -> Result<RoleId, SemanticError> {
    raw.parse()
        .map_err(|err| invalid_data_matcher_literal(trigger_name, family, "role", raw, err))
}

fn lower_structured_data_filter(
    trigger_name: &str,
    filter: &TriggerStructuredDataFilter,
) -> Result<DataEventFilter, SemanticError> {
    match filter.family {
        TriggerDataFamily::Peer => {
            let mut peer =
                PeerEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => PeerEventSet::all(),
                    Some("added") => PeerEventSet::Added,
                    Some("removed") => PeerEventSet::Removed,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_peer = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "peer" => {
                        if seen_peer {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "peer",
                            ));
                        }
                        peer = peer.for_peer(parse_peer_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_peer = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Peer(peer))
        }
        TriggerDataFamily::Domain => {
            let mut domain =
                DomainEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => DomainEventSet::all(),
                    Some("created") => DomainEventSet::Created,
                    Some("deleted") => DomainEventSet::Deleted,
                    Some("asset_definition") => DomainEventSet::AnyAssetDefinition,
                    Some("nft") => DomainEventSet::AnyNft,
                    Some("account") => DomainEventSet::AnyAccount,
                    Some("account_linked") => DomainEventSet::AccountLinked,
                    Some("account_unlinked") => DomainEventSet::AccountUnlinked,
                    Some("metadata_inserted") => DomainEventSet::MetadataInserted,
                    Some("metadata_removed") => DomainEventSet::MetadataRemoved,
                    Some("owner_changed") => DomainEventSet::OwnerChanged,
                    Some("kaigi_roster_summary") => DomainEventSet::KaigiRosterSummary,
                    Some("kaigi_relay_registered") => DomainEventSet::KaigiRelayRegistered,
                    Some("kaigi_relay_manifest_updated") => {
                        DomainEventSet::KaigiRelayManifestUpdated
                    }
                    Some("kaigi_usage_summary") => DomainEventSet::KaigiUsageSummary,
                    Some("kaigi_relay_health_updated") => DomainEventSet::KaigiRelayHealthUpdated,
                    Some("streaming_ticket_ready") => DomainEventSet::StreamingTicketReady,
                    Some("streaming_ticket_revoked") => DomainEventSet::StreamingTicketRevoked,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_domain = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "domain" => {
                        if seen_domain {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "domain",
                            ));
                        }
                        domain = domain.for_domain(parse_domain_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_domain = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Domain(domain))
        }
        TriggerDataFamily::Account => {
            let mut account =
                AccountEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => AccountEventSet::all(),
                    Some("created") => AccountEventSet::Created,
                    Some("deleted") => AccountEventSet::Deleted,
                    Some("asset") => AccountEventSet::AnyAsset,
                    Some("permission_added") => AccountEventSet::PermissionAdded,
                    Some("permission_removed") => AccountEventSet::PermissionRemoved,
                    Some("role_granted") => AccountEventSet::RoleGranted,
                    Some("role_revoked") => AccountEventSet::RoleRevoked,
                    Some("metadata_inserted") => AccountEventSet::MetadataInserted,
                    Some("metadata_removed") => AccountEventSet::MetadataRemoved,
                    Some("repo") => AccountEventSet::AnyRepo,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_account = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "account" => {
                        if seen_account {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "account",
                            ));
                        }
                        account = account.for_account(parse_account_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_account = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Account(account))
        }
        TriggerDataFamily::Asset => {
            let mut asset =
                AssetEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => AssetEventSet::all(),
                    Some("created") => AssetEventSet::Created,
                    Some("deleted") => AssetEventSet::Deleted,
                    Some("added") => AssetEventSet::Added,
                    Some("removed") => AssetEventSet::Removed,
                    Some("transferred") => AssetEventSet::Transferred,
                    Some("metadata_inserted") => AssetEventSet::MetadataInserted,
                    Some("metadata_removed") => AssetEventSet::MetadataRemoved,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_asset = false;
            let mut seen_asset_definition = false;
            let mut seen_source_account = false;
            let mut seen_destination_account = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "asset" => {
                        if seen_asset {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "asset",
                            ));
                        }
                        asset = asset.for_asset(parse_asset_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_asset = true;
                    }
                    "asset_definition" => {
                        if seen_asset_definition {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "asset_definition",
                            ));
                        }
                        asset = asset.for_asset_definition(parse_asset_definition_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_asset_definition = true;
                    }
                    "source_account" => {
                        if seen_source_account {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "source_account",
                            ));
                        }
                        asset = asset.for_transfer_source_account(parse_account_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_source_account = true;
                    }
                    "destination_account" => {
                        if seen_destination_account {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "destination_account",
                            ));
                        }
                        asset = asset.for_transfer_destination_account(parse_account_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_destination_account = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Asset(asset))
        }
        TriggerDataFamily::AssetDefinition => {
            let mut asset_definition = AssetDefinitionEventFilter::new().for_events(
                match named_data_event_kind(&filter.event) {
                    None => AssetDefinitionEventSet::all(),
                    Some("created") => AssetDefinitionEventSet::Created,
                    Some("deleted") => AssetDefinitionEventSet::Deleted,
                    Some("metadata_inserted") => AssetDefinitionEventSet::MetadataInserted,
                    Some("metadata_removed") => AssetDefinitionEventSet::MetadataRemoved,
                    Some("mintability_changed") => AssetDefinitionEventSet::MintabilityChanged,
                    Some("mintability_changed_detailed") => {
                        AssetDefinitionEventSet::MintabilityChangedDetailed
                    }
                    Some("total_quantity_changed") => AssetDefinitionEventSet::TotalQuantityChanged,
                    Some("owner_changed") => AssetDefinitionEventSet::OwnerChanged,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                },
            );
            let mut seen_asset_definition = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "asset_definition" => {
                        if seen_asset_definition {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "asset_definition",
                            ));
                        }
                        asset_definition =
                            asset_definition.for_asset_definition(parse_asset_definition_matcher(
                                trigger_name,
                                filter.family,
                                &matcher.value,
                            )?);
                        seen_asset_definition = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::AssetDefinition(asset_definition))
        }
        TriggerDataFamily::Nft => {
            let mut nft =
                NftEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => NftEventSet::all(),
                    Some("created") => NftEventSet::Created,
                    Some("deleted") => NftEventSet::Deleted,
                    Some("metadata_inserted") => NftEventSet::MetadataInserted,
                    Some("metadata_removed") => NftEventSet::MetadataRemoved,
                    Some("owner_changed") => NftEventSet::OwnerChanged,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_nft = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "nft" => {
                        if seen_nft {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "nft",
                            ));
                        }
                        nft = nft.for_nft(parse_nft_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_nft = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Nft(nft))
        }
        TriggerDataFamily::Rwa => {
            let mut rwa =
                RwaEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => RwaEventSet::all(),
                    Some("created") => RwaEventSet::Created,
                    Some("metadata_inserted") => RwaEventSet::MetadataInserted,
                    Some("metadata_removed") => RwaEventSet::MetadataRemoved,
                    Some("owner_changed") => RwaEventSet::OwnerChanged,
                    Some("split") => RwaEventSet::Split,
                    Some("merged") => RwaEventSet::Merged,
                    Some("redeemed") => RwaEventSet::Redeemed,
                    Some("frozen") => RwaEventSet::Frozen,
                    Some("unfrozen") => RwaEventSet::Unfrozen,
                    Some("held") => RwaEventSet::Held,
                    Some("released") => RwaEventSet::Released,
                    Some("force_transferred") => RwaEventSet::ForceTransferred,
                    Some("controls_changed") => RwaEventSet::ControlsChanged,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_rwa = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "rwa" => {
                        if seen_rwa {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "rwa",
                            ));
                        }
                        rwa = rwa.for_rwa(parse_rwa_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_rwa = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Rwa(rwa))
        }
        TriggerDataFamily::Trigger => {
            let mut trigger =
                TriggerEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => TriggerEventSet::all(),
                    Some("created") => TriggerEventSet::Created,
                    Some("deleted") => TriggerEventSet::Deleted,
                    Some("extended") => TriggerEventSet::Extended,
                    Some("shortened") => TriggerEventSet::Shortened,
                    Some("metadata_inserted") => TriggerEventSet::MetadataInserted,
                    Some("metadata_removed") => TriggerEventSet::MetadataRemoved,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_trigger = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "trigger" => {
                        if seen_trigger {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "trigger",
                            ));
                        }
                        trigger = trigger.for_trigger(parse_trigger_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_trigger = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Trigger(trigger))
        }
        TriggerDataFamily::Role => {
            let mut role =
                RoleEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => RoleEventSet::all(),
                    Some("created") => RoleEventSet::Created,
                    Some("deleted") => RoleEventSet::Deleted,
                    Some("permission_added") => RoleEventSet::PermissionAdded,
                    Some("permission_removed") => RoleEventSet::PermissionRemoved,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            let mut seen_role = false;
            for matcher in &filter.matchers {
                match matcher.key.as_str() {
                    "role" => {
                        if seen_role {
                            return Err(duplicate_data_matcher_error(
                                trigger_name,
                                filter.family,
                                "role",
                            ));
                        }
                        role = role.for_role(parse_role_matcher(
                            trigger_name,
                            filter.family,
                            &matcher.value,
                        )?);
                        seen_role = true;
                    }
                    key => {
                        return Err(unsupported_data_matcher_error(
                            trigger_name,
                            filter.family,
                            key,
                        ));
                    }
                }
            }
            Ok(DataEventFilter::Role(role))
        }
        TriggerDataFamily::Configuration => {
            let configuration = ConfigurationEventFilter::new().for_events(
                match named_data_event_kind(&filter.event) {
                    None => ConfigurationEventSet::all(),
                    Some("changed") => ConfigurationEventSet::Changed,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                },
            );
            if let Some(matcher) = filter.matchers.first() {
                return Err(unsupported_data_matcher_error(
                    trigger_name,
                    filter.family,
                    &matcher.key,
                ));
            }
            Ok(DataEventFilter::Configuration(configuration))
        }
        TriggerDataFamily::Executor => {
            let executor =
                ExecutorEventFilter::new().for_events(match named_data_event_kind(&filter.event) {
                    None => ExecutorEventSet::all(),
                    Some("upgraded") => ExecutorEventSet::Upgraded,
                    Some(kind) => {
                        return Err(unsupported_data_event_kind_error(
                            trigger_name,
                            filter.family,
                            kind,
                        ));
                    }
                });
            if let Some(matcher) = filter.matchers.first() {
                return Err(unsupported_data_matcher_error(
                    trigger_name,
                    filter.family,
                    &matcher.key,
                ));
            }
            Ok(DataEventFilter::Executor(executor))
        }
    }
}

fn analyze_trigger(
    trigger: &TriggerDecl,
    fn_modifiers: &HashMap<String, FunctionModifiers>,
) -> Result<TypedTrigger, SemanticError> {
    let name =
        <Name as std::str::FromStr>::from_str(&trigger.name).map_err(|err| SemanticError {
            code: "E_TRIGGER_INVALID_NAME",
            message: format!("invalid trigger name `{}`: {}", trigger.name, err),
        })?;
    let id = TriggerId::new(name);

    if trigger.call.namespace.is_none() {
        let entry = &trigger.call.entrypoint;
        let modifiers = fn_modifiers.get(entry).ok_or_else(|| SemanticError {
            code: "K2002",
            message: format!(
                "trigger `{}` targets unknown `kotoage`/`言挙げ` function `{entry}`",
                trigger.name
            ),
        })?;
        if modifiers.kind == FunctionKind::View {
            return Err(SemanticError {
                code: "E_TRIGGER_VIEW_TARGET",
                message: format!(
                    "trigger `{}` cannot target read-only `view fn` function `{entry}`",
                    trigger.name
                ),
            });
        }
        if modifiers.kind != FunctionKind::Kotoage {
            return Err(SemanticError {
                code: "E_TRIGGER_TARGET_KIND",
                message: format!(
                    "trigger `{}` must call a `kotoage`/`言挙げ` function `{entry}`",
                    trigger.name
                ),
            });
        }
    }

    let filter = match &trigger.filter {
        TriggerFilter::Time(time) => {
            let execution = match time {
                TriggerTimeFilter::PreCommit => ExecutionTime::PreCommit,
                TriggerTimeFilter::Schedule {
                    start_ms,
                    period_ms,
                } => {
                    if let Some(period) = period_ms
                        && *period == 0
                    {
                        return Err(SemanticError {
                            code: "E_TRIGGER_SCHEDULE_PERIOD",
                            message: format!(
                                "trigger `{}` schedule period_ms must be non-zero",
                                trigger.name
                            ),
                        });
                    }
                    ExecutionTime::Schedule(Schedule {
                        start_ms: *start_ms,
                        period_ms: *period_ms,
                    })
                }
            };
            EventFilterBox::Time(TimeEventFilter(execution))
        }
        TriggerFilter::Execute { trigger_id } => {
            let target =
                <Name as std::str::FromStr>::from_str(trigger_id).map_err(|err| SemanticError {
                    code: "E_TRIGGER_INVALID_ID",
                    message: format!("invalid execute trigger id `{trigger_id}`: {err}"),
                })?;
            let id = TriggerId::new(target);
            EventFilterBox::ExecuteTrigger(ExecuteTriggerEventFilter::new().for_trigger(id))
        }
        TriggerFilter::Data(data) => {
            let filter = match data {
                TriggerDataFilter::Any => DataEventFilter::Any,
                TriggerDataFilter::Structured(filter) => {
                    lower_structured_data_filter(&trigger.name, filter)?
                }
            };
            EventFilterBox::Data(filter)
        }
        TriggerFilter::Pipeline(pipeline) => {
            let filter = match pipeline {
                TriggerPipelineFilter::TransactionApproved => PipelineEventFilterBox::Transaction(
                    TransactionEventFilter::new().for_status(TransactionStatus::Approved),
                ),
                TriggerPipelineFilter::BlockApproved => PipelineEventFilterBox::Block(
                    BlockEventFilter::new().for_status(BlockStatus::Approved),
                ),
            };
            EventFilterBox::Pipeline(filter)
        }
    };

    let repeats = match trigger
        .repeats
        .clone()
        .unwrap_or(TriggerRepeats::Indefinitely)
    {
        TriggerRepeats::Indefinitely => Repeats::Indefinitely,
        TriggerRepeats::Exactly(count) => Repeats::Exactly(count),
    };

    let authority = match &trigger.authority {
        Some(raw) => Some(
            AccountId::parse_encoded(raw)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .map_err(|err| SemanticError {
                    code: "E_TRIGGER_INVALID_AUTHORITY",
                    message: format!("invalid trigger authority `{raw}`: {err}"),
                })?,
        ),
        None => None,
    };

    let metadata = trigger_metadata_from_entries(&trigger.metadata)?;

    Ok(TypedTrigger {
        id,
        call: trigger.call.clone(),
        filter,
        repeats,
        authority,
        metadata,
    })
}

fn trigger_metadata_from_entries(
    entries: &[TriggerMetadataEntry],
) -> Result<Metadata, SemanticError> {
    let mut metadata = Metadata::default();
    for entry in entries {
        let key =
            <Name as std::str::FromStr>::from_str(&entry.key).map_err(|err| SemanticError {
                code: "E_TRIGGER_INVALID_METADATA_KEY",
                message: format!("invalid trigger metadata key `{}`: {err}", entry.key),
            })?;
        let json = json_from_expr(&entry.value)?;
        if metadata.insert(key, json).is_some() {
            return Err(SemanticError {
                code: "K2001",
                message: format!("duplicate trigger metadata key `{}`", entry.key),
            });
        }
    }
    Ok(metadata)
}

fn parse_json_literal(raw: &str) -> Result<json::Value, SemanticError> {
    json::parse_value(raw).map_err(|error| match error {
        json::Error::DuplicateField { field } => SemanticError {
            code: "E_JSON_DUPLICATE_KEY",
            message: format!(
                "Json::parse object key `{field}` is supplied more than once after string decoding"
            ),
        },
        error => SemanticError {
            code: "E_JSON_LITERAL_INVALID",
            message: format!("invalid Json::parse literal: {error}"),
        },
    })
}

fn json_from_expr(expr: &Expr) -> Result<Json, SemanticError> {
    let value = match expr {
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            return json_from_expr(expression);
        }
        Expr::String(s) => json::Value::String(s.clone()),
        Expr::IntLiteral(value) => {
            let number = value
                .try_to_i64()
                .map(JsonNumber::I64)
                .or_else(|| value.try_to_u64().map(JsonNumber::U64))
                .ok_or_else(|| SemanticError {
                code: "E_TRIGGER_METADATA_VALUE",
                message: "trigger metadata JSON cannot represent this int exactly; use an explicit string or typed state value"
                    .into(),
            })?;
            json::Value::Number(number)
        }
        Expr::DecimalLiteral(_) => {
            return Err(SemanticError {
                code: "E_TRIGGER_METADATA_VALUE",
                message: "quantity trigger metadata requires explicit native JSON construction"
                    .into(),
            });
        }
        Expr::Bool(b) => json::Value::Bool(*b),
        Expr::Ident(ident) if ident == "null" => json::Value::Null,
        Expr::Call {
            name,
            args,
            argument_names,
            implicit_receiver,
        } if name == "Json::parse" => {
            if *implicit_receiver {
                return Err(SemanticError {
                    code: "E_MALFORMED_CALL",
                    message: "Json::parse is a static constructor and has no receiver".into(),
                });
            }
            if args.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: "Json::parse expects one argument".into(),
                });
            }
            let builtin = Builtin::PointerConstructor(PointerConstructor::Json);
            let signature = builtin.signature();
            let parameter_names = signature
                .parameter_names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>();
            let required = signature
                .parameters
                .iter()
                .map(|parameter| !parameter.ends_with('?'))
                .collect::<Vec<_>>();
            let plan = reorder_call_arguments(
                name,
                args,
                argument_names.as_deref(),
                false,
                &parameter_names,
                &required,
                builtin_named_only_reason(builtin, false),
            )?;
            if plan.ordered.len() != 1 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "Json::parse expects one argument".into(),
                });
            }
            let Expr::String(raw) = plan.ordered[0].kind() else {
                return Err(SemanticError {
                    code: "E_TRIGGER_METADATA_VALUE",
                    message: "Json::parse(...) metadata values must be a string literal".into(),
                });
            };
            parse_json_literal(raw)?
        }
        Expr::Call { name, .. } if name == "json" => {
            return Err(SemanticError {
                code: "E_NON_CANONICAL_BUILTIN",
                message: "legacy or non-canonical builtin spelling `json` is not supported; use `Json::parse`"
                    .into(),
            });
        }
        _ => {
            return Err(SemanticError {
                code: "E_TRIGGER_METADATA_VALUE",
                message: "trigger metadata values must be JSON literals".into(),
            });
        }
    };
    Json::from_norito_value_ref(&value).map_err(|err| SemanticError {
        code: "E_TRIGGER_METADATA_VALUE",
        message: format!("invalid trigger metadata value: {err}"),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericKind {
    Int,
    Decimal,
    Quantity,
}

fn numeric_kind(ty: &Type) -> Option<NumericKind> {
    match resolve_struct_type(ty) {
        Type::Int => Some(NumericKind::Int),
        Type::Decimal => Some(NumericKind::Decimal),
        Type::Quantity => Some(NumericKind::Quantity),
        _ => None,
    }
}

fn numeric_kind_to_type(kind: NumericKind) -> Type {
    match kind {
        NumericKind::Int => Type::Int,
        NumericKind::Decimal => Type::Decimal,
        NumericKind::Quantity => Type::Quantity,
    }
}

fn typed_int_literal(value: &BigInt) -> Result<TypedExpr, SemanticError> {
    if value.to_twos_bytes().len() > MAX_MANTISSA_BYTES {
        return Err(SemanticError {
            code: "E_INT_LITERAL_OVERFLOW",
            message: "integer literal is outside the signed 512-bit Kotodama int domain".into(),
        });
    }
    Ok(TypedExpr {
        expr: ExprKind::IntLiteral(value.clone()),
        ty: Type::Int,
    })
}

fn parse_decimal_literal(spelling: &str) -> Result<Numeric, SemanticError> {
    let (coefficient, exponent) = spelling
        .split_once(['e', 'E'])
        .map_or((spelling, "0"), |(coefficient, exponent)| {
            (coefficient, exponent)
        });
    let exponent = exponent.replace('_', "");
    let exponent = exponent.parse::<i64>().map_err(|_| SemanticError {
        code: if exponent.starts_with('-') {
            "E_DECIMAL_SCALE_OVERFLOW"
        } else {
            "E_DECIMAL_MANTISSA_OVERFLOW"
        },
        message: "decimal exponent is outside the representable V1 domain".into(),
    })?;
    let (negative, coefficient) = coefficient
        .strip_prefix('-')
        .map_or((false, coefficient), |coefficient| (true, coefficient));
    let (whole, fractional) = coefficient
        .split_once('.')
        .map_or((coefficient, ""), |(whole, fractional)| (whole, fractional));
    let whole = whole.replace('_', "");
    let fractional = fractional.replace('_', "");
    let combined = format!("{whole}{fractional}");
    let significant = combined.trim_start_matches('0');
    if significant.is_empty() {
        return Ok(Numeric::zero());
    }
    let mut mantissa_spelling = significant.to_owned();
    let mut scale = i64::try_from(fractional.len())
        .map_err(|_| SemanticError {
            code: "E_DECIMAL_SCALE_OVERFLOW",
            message: "decimal literal scale exceeds the V1 maximum of 28".into(),
        })?
        .checked_sub(exponent)
        .ok_or_else(|| SemanticError {
            code: if exponent.is_negative() {
                "E_DECIMAL_SCALE_OVERFLOW"
            } else {
                "E_DECIMAL_MANTISSA_OVERFLOW"
            },
            message: "decimal exponent is outside the representable V1 domain".into(),
        })?;
    if scale < 0 {
        let zeros = usize::try_from(scale.unsigned_abs()).map_err(|_| SemanticError {
            code: "E_DECIMAL_MANTISSA_OVERFLOW",
            message: "decimal literal exceeds the signed 512-bit mantissa domain".into(),
        })?;
        if mantissa_spelling.len().saturating_add(zeros) > 154 {
            return Err(SemanticError {
                code: "E_DECIMAL_MANTISSA_OVERFLOW",
                message: "decimal literal exceeds the signed 512-bit mantissa domain".into(),
            });
        }
        mantissa_spelling.extend(core::iter::repeat_n('0', zeros));
        scale = 0;
    }
    while scale > 0 && mantissa_spelling.ends_with('0') {
        mantissa_spelling.pop();
        scale -= 1;
    }
    let scale = u32::try_from(scale).map_err(|_| SemanticError {
        code: "E_DECIMAL_SCALE_OVERFLOW",
        message: "decimal literal scale exceeds the V1 maximum of 28".into(),
    })?;
    if scale > 28 {
        return Err(SemanticError {
            code: "E_DECIMAL_SCALE_OVERFLOW",
            message: format!("decimal literal has canonical scale {scale}; the V1 maximum is 28"),
        });
    }
    if negative {
        mantissa_spelling.insert(0, '-');
    }
    let mantissa = mantissa_spelling
        .parse::<BigInt>()
        .map_err(|_| SemanticError {
            code: "E_DECIMAL_MANTISSA_OVERFLOW",
            message: "decimal literal exceeds the signed 512-bit mantissa domain".into(),
        })?;
    Numeric::try_new(mantissa, scale)
        .map_err(|error| match error {
            NumericError::MantissaTooLarge => SemanticError {
                code: "E_DECIMAL_MANTISSA_OVERFLOW",
                message: "decimal literal exceeds the signed 512-bit mantissa domain".into(),
            },
            NumericError::ScaleTooLarge => SemanticError {
                code: "E_DECIMAL_SCALE_OVERFLOW",
                message: "decimal literal scale exceeds the V1 maximum of 28".into(),
            },
            NumericError::Malformed => SemanticError {
                code: "E_DECIMAL_MALFORMED",
                message: "invalid decimal literal".into(),
            },
        })?
        .canonicalize_decimal()
        .map_err(|error| SemanticError {
            code: "E_DECIMAL_MALFORMED",
            message: format!("invalid decimal literal: {error}"),
        })
}

fn descriptors_have_confusable_repeats(parameters: &[&str]) -> bool {
    for (index, parameter) in parameters.iter().enumerate() {
        let parameter = parameter.trim_end_matches(['?', '.']);
        for (other_index, other) in parameters.iter().enumerate().skip(index + 1) {
            let other = other.trim_end_matches(['?', '.']);
            if parameter == other
                || (other == "same-as-arg0" && index == 0)
                || (parameter == "same-as-arg0" && other_index == 0)
            {
                return true;
            }
        }
    }
    false
}

fn user_parameters_have_confusable_repeats(parameters: &[TypedParam]) -> bool {
    parameters.iter().enumerate().any(|(index, parameter)| {
        parameters
            .iter()
            .skip(index + 1)
            .any(|other| resolve_struct_type(&parameter.ty) == resolve_struct_type(&other.ty))
    })
}

fn builtin_named_only_reason(builtin: Builtin, implicit_receiver: bool) -> Option<&'static str> {
    let spec = builtin.spec();
    if spec.call_policy == BuiltinCallPolicy::Pagination {
        return Some("pagination calls require explicit `offset` and `limit` names");
    }
    let receiver_count = usize::from(implicit_receiver);
    let user_parameters = spec
        .signature
        .parameters
        .get(receiver_count..)
        .unwrap_or_default();
    let effects = spec.effects;
    if user_parameters.len() >= 3
        && (effects.host_side_effects
            || effects.emits_instructions
            || effects.mutates_durable_state)
    {
        return Some("privileged or effectful calls with at least three parameters require names");
    }
    descriptors_have_confusable_repeats(user_parameters)
        .then_some("repeated parameter types require names to prevent argument transposition")
}

#[derive(Debug)]
struct CallArgumentPlan {
    /// Arguments in declaration/ABI order.
    ordered: Vec<Expr>,
    /// Indices into `ordered` in source evaluation order.
    evaluation_order: Vec<usize>,
    /// Whether the source call used named arguments.
    is_named: bool,
}

fn reorder_call_arguments(
    call_name: &str,
    args: &[Expr],
    argument_names: Option<&[String]>,
    implicit_receiver: bool,
    parameter_names: &[String],
    required: &[bool],
    named_only_reason: Option<&str>,
) -> Result<CallArgumentPlan, SemanticError> {
    let receiver_count = usize::from(implicit_receiver);
    if args.len() < receiver_count {
        return Err(SemanticError {
            code: "E_MALFORMED_CALL",
            message: format!("call `{call_name}` is missing its compiler-inserted receiver"),
        });
    }
    let user_args = &args[receiver_count..];
    let Some(argument_names) = argument_names else {
        if named_only_reason.is_some() && !user_args.is_empty() {
            return Err(SemanticError {
                code: "E_NAMED_ARGUMENTS_REQUIRED",
                message: format!(
                    "call `{call_name}` requires named arguments because {}",
                    named_only_reason.unwrap_or_default()
                ),
            });
        }
        return Ok(CallArgumentPlan {
            ordered: args.to_vec(),
            evaluation_order: (0..args.len()).collect(),
            is_named: false,
        });
    };
    if argument_names.len() != user_args.len() {
        return Err(SemanticError {
            code: "E_MALFORMED_CALL",
            message: format!("call `{call_name}` has inconsistent named-argument metadata"),
        });
    }

    for name in argument_names {
        if !parameter_names.iter().any(|parameter| parameter == name) {
            return Err(SemanticError {
                code: "E_UNKNOWN_NAMED_ARGUMENT",
                message: format!("call `{call_name}` has no parameter named `{name}`"),
            });
        }
    }
    let mut ordered = Vec::with_capacity(args.len());
    let mut ordered_slots = HashMap::with_capacity(parameter_names.len());
    let mut first_omitted_optional = None;
    if implicit_receiver {
        ordered.push(args[0].clone());
    }
    for (index, parameter_name) in parameter_names.iter().enumerate() {
        let positions = argument_names
            .iter()
            .enumerate()
            .filter_map(|(position, name)| (name == parameter_name).then_some(position))
            .collect::<Vec<_>>();
        if positions.len() > 1 {
            return Err(SemanticError {
                code: "E_DUPLICATE_NAMED_ARGUMENT",
                message: format!("named argument `{parameter_name}` is supplied more than once"),
            });
        }
        if let Some(position) = positions.first() {
            if let Some(omitted) = first_omitted_optional
                && !required.get(index).copied().unwrap_or(true)
            {
                return Err(SemanticError {
                    code: "E_NAMED_ARGUMENT_HOLE",
                    message: format!(
                        "optional named argument `{parameter_name}` cannot be supplied while earlier optional parameter `{omitted}` is omitted"
                    ),
                });
            }
            ordered_slots.insert(parameter_name.as_str(), ordered.len());
            ordered.push(user_args[*position].clone());
        } else if required.get(index).copied().unwrap_or(true) {
            return Err(SemanticError {
                code: "E_MISSING_NAMED_ARGUMENT",
                message: format!(
                    "call `{call_name}` is missing required argument `{parameter_name}`"
                ),
            });
        } else if first_omitted_optional.is_none() {
            first_omitted_optional = Some(parameter_name.as_str());
        }
    }
    let mut evaluation_order = Vec::with_capacity(ordered.len());
    if implicit_receiver {
        evaluation_order.push(0);
    }
    for name in argument_names {
        evaluation_order.push(*ordered_slots.get(name.as_str()).ok_or_else(|| SemanticError {
            code: "E_MALFORMED_CALL",
            message: format!(
                "call `{call_name}` lost named argument `{name}` while resolving its parameter slot"
            ),
        })?);
    }
    Ok(CallArgumentPlan {
        ordered,
        evaluation_order,
        is_named: true,
    })
}

fn builtin_instantiation_has_confusable_repeats(
    builtin: Builtin,
    arguments: &[TypedExpr],
    implicit_receiver: bool,
) -> bool {
    if !implicit_receiver
        || !matches!(
            builtin,
            Builtin::GetOrDefault | Builtin::GetOr | Builtin::Ensure
        )
    {
        return false;
    }
    let Some(receiver) = arguments.first() else {
        return false;
    };
    let Type::StateMap(key, value) = resolve_struct_type(&receiver.ty) else {
        return false;
    };
    resolve_struct_type(&key) == resolve_struct_type(&value)
}

fn retain_named_call_evaluation_order(typed: TypedExpr, plan: &CallArgumentPlan) -> TypedExpr {
    if !plan.is_named {
        return typed;
    }
    let TypedExpr { expr, ty } = typed;
    let expr = match expr {
        ExprKind::Call { name, args } if args.len() == plan.ordered.len() => ExprKind::NamedCall {
            name,
            args,
            evaluation_order: plan.evaluation_order.clone(),
        },
        // Some compiler-owned test intrinsics consume literal selector
        // arguments into the canonical callee name. Those total literals no
        // longer have runtime slots, so the source permutation cannot be
        // transferred to the smaller internal call and is unnecessary.
        other => other,
    };
    TypedExpr { expr, ty }
}

pub(crate) fn is_numeric_type(ty: &Type) -> bool {
    numeric_kind(ty).is_some()
}

pub(crate) fn is_wide_numeric_type(ty: &Type) -> bool {
    matches!(
        resolve_struct_type(ty),
        Type::Int | Type::Decimal | Type::Quantity
    )
}

fn is_int_like(ty: &Type) -> bool {
    matches!(resolve_struct_type(ty), Type::Int)
}

fn numeric_result_type(lhs: &Type, rhs: &Type) -> Option<Type> {
    let lhs_resolved = resolve_struct_type(lhs);
    let rhs_resolved = resolve_struct_type(rhs);
    if lhs_resolved == rhs_resolved {
        return numeric_kind(&lhs_resolved).map(numeric_kind_to_type);
    }
    None
}

fn arithmetic_result_type(op: BinaryOp, lhs: &Type, rhs: &Type) -> Option<Type> {
    let lhs = resolve_struct_type(lhs);
    let rhs = resolve_struct_type(rhs);
    match (op, &lhs, &rhs) {
        (_, Type::Int, Type::Int) => Some(Type::Int),
        (BinaryOp::Mod, Type::Decimal, Type::Decimal) => None,
        (_, Type::Decimal, Type::Decimal) => Some(Type::Decimal),
        (BinaryOp::Add | BinaryOp::Sub, Type::Quantity, Type::Quantity) => Some(Type::Quantity),
        (BinaryOp::Mul, Type::Quantity, Type::Decimal)
        | (BinaryOp::Div, Type::Quantity, Type::Decimal) => Some(Type::Quantity),
        (BinaryOp::Div, Type::Quantity, Type::Quantity) => Some(Type::Decimal),
        _ => None,
    }
}

fn literal_int(expr: &TypedExpr) -> Option<BigInt> {
    match expr.kind() {
        ExprKind::IntLiteral(value) => Some(value.clone()),
        ExprKind::NumericCast { expr } => literal_int(expr),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => literal_int(expr).and_then(|value| value.checked_neg().ok()),
        _ => None,
    }
}

fn reject_implicit_int_decimal_mix(lhs: &Type, rhs: &Type) -> Result<(), SemanticError> {
    let lhs = resolve_struct_type(lhs);
    let rhs = resolve_struct_type(rhs);
    if !matches!(
        (&lhs, &rhs),
        (Type::Int, Type::Decimal) | (Type::Decimal, Type::Int)
    ) {
        return Ok(());
    }
    Err(SemanticError {
        code: "E_IMPLICIT_NUMERIC_CONVERSION",
        message: "`int` and `decimal` operands cannot be mixed implicitly; convert the `int` with `decimal::from_int(value)` before arithmetic or comparison"
            .into(),
    })
}

fn explicit_numeric_conversion(
    name: &str,
    args: Vec<TypedExpr>,
) -> Option<Result<TypedExpr, SemanticError>> {
    if name == "decimal::to_int_trunc" {
        return Some((|| {
            if args.len() != 1 || resolve_struct_type(&args[0].ty) != Type::Decimal {
                return Err(SemanticError {
                    code: "K2003",
                    message: "decimal::to_int_trunc expects exactly one decimal argument".into(),
                });
            }
            if let Some(crate::checked_arithmetic::ConstantNumeric::Decimal(value)) =
                crate::checked_arithmetic::evaluate(&args[0]).map_err(|error| SemanticError {
                    code: error.code(),
                    message: error.to_string(),
                })?
            {
                let value = value.decimal_to_int_trunc().map_err(|error| {
                    let error = crate::checked_arithmetic::ConstantNumericError::Numeric(error);
                    SemanticError {
                        code: error.code(),
                        message: error.to_string(),
                    }
                })?;
                return Ok(TypedExpr {
                    expr: ExprKind::IntLiteral(value),
                    ty: Type::Int,
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: DECIMAL_TO_INT_TRUNC_INTRINSIC.to_owned(),
                    args,
                },
                ty: Type::Int,
            })
        })());
    }
    let (source, destination, recoverable) = match name {
        "decimal::from_int" => (Type::Int, Type::Decimal, false),
        "decimal::to_int_exact" => (Type::Decimal, Type::Int, false),
        "quantity::try_from_int" => (Type::Int, Type::Quantity, true),
        "quantity::try_from_decimal" => (Type::Decimal, Type::Quantity, true),
        "decimal::from_quantity" => (Type::Quantity, Type::Decimal, false),
        _ => return None,
    };
    Some((|| {
        if args.len() != 1 || resolve_struct_type(&args[0].ty) != source {
            return Err(SemanticError {
                code: "K2003",
                message: format!("{name} expects exactly one {} argument", type_name(&source)),
            });
        }
        let argument = Box::new(args.into_iter().next().expect("one argument checked"));
        if recoverable {
            return Ok(TypedExpr {
                expr: ExprKind::NumericTryCast { expr: argument },
                ty: Type::Result(Box::new(destination), Box::new(Type::Int)),
            });
        }
        Ok(TypedExpr {
            expr: ExprKind::NumericCast { expr: argument },
            ty: destination,
        })
    })())
}

fn numeric_literal_is_negative(expr: &TypedExpr) -> bool {
    match expr.kind() {
        ExprKind::IntLiteral(value) => value.is_negative(),
        ExprKind::DecimalLiteral { value, .. } => value.mantissa().is_negative(),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => !numeric_literal_is_zero(expr),
        _ => false,
    }
}

fn numeric_literal_is_zero(expr: &TypedExpr) -> bool {
    match expr.kind() {
        ExprKind::IntLiteral(value) => value.is_zero(),
        ExprKind::DecimalLiteral { value, .. } => value.is_zero(),
        _ => false,
    }
}

fn is_supported_durable_value_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        ty if is_numeric_type(&ty) => true,
        Type::Bool | Type::String | Type::Json | Type::Bytes => true,
        other if is_pointer_type(&other) => true,
        Type::Struct { fields, .. } => fields
            .iter()
            .all(|(_, field_ty)| is_supported_durable_value_type(field_ty)),
        Type::Tuple(items) => items.iter().all(is_supported_durable_value_type),
        Type::Option(inner) => is_supported_durable_value_type(&inner),
        Type::Result(ok, err) => {
            is_supported_durable_value_type(&ok) && is_supported_durable_value_type(&err)
        }
        Type::List(element, _) => is_supported_durable_value_type(&element),
        _ => false,
    }
}

fn coerce_exact_numeric_literal_to(
    expr: &mut TypedExpr,
    expected: &Type,
) -> Result<(), SemanticError> {
    if resolve_struct_type(&expr.ty) != resolve_struct_type(expected)
        && exact_numeric_literal_expression(expr)
    {
        ensure_assignable_and_coerce(expected, expr)?;
    }
    Ok(())
}

/// Apply the expected numeric domain only to exact literal operands.
///
/// Runtime values retain their nominal type: this is literal inference, not
/// an implicit conversion between `int`, `decimal`, and `quantity` values.
fn coerce_contextual_numeric_literals(
    op: BinaryOp,
    expected: Option<&Type>,
    left: &mut TypedExpr,
    right: &mut TypedExpr,
) -> Result<(), SemanticError> {
    use BinaryOp::*;

    let comparison = matches!(op, Eq | Ne | Lt | Le | Gt | Ge);
    let left_type = resolve_struct_type(&left.ty);
    let right_type = resolve_struct_type(&right.ty);

    if left_type == Type::Quantity {
        match op {
            Add | Sub | Mod if exact_numeric_literal_expression(right) => {
                coerce_exact_numeric_literal_to(right, &Type::Quantity)?;
            }
            Mul if exact_numeric_literal_expression(right) => {
                coerce_exact_numeric_literal_to(right, &Type::Decimal)?;
            }
            Div if exact_numeric_literal_expression(right) => {
                let divisor_type = if expected
                    .is_some_and(|expected| resolve_struct_type(expected) == Type::Decimal)
                {
                    Type::Quantity
                } else {
                    Type::Decimal
                };
                coerce_exact_numeric_literal_to(right, &divisor_type)?;
            }
            _ if comparison && exact_numeric_literal_expression(right) => {
                coerce_exact_numeric_literal_to(right, &Type::Quantity)?;
            }
            _ => {}
        }
        return Ok(());
    }

    if right_type == Type::Quantity {
        if matches!(op, Add | Sub | Mod) || comparison {
            coerce_exact_numeric_literal_to(left, &Type::Quantity)?;
        }
        return Ok(());
    }

    // A sibling decimal operand provides a compile-time type context for an
    // exact literal. This retags and folds only literal syntax; an existing
    // runtime `int` is never wrapped in a hidden `NumericCast`.
    if left_type == Type::Decimal && exact_numeric_literal_expression(right) {
        coerce_exact_numeric_literal_to(right, &Type::Decimal)?;
    } else if right_type == Type::Decimal && exact_numeric_literal_expression(left) {
        coerce_exact_numeric_literal_to(left, &Type::Decimal)?;
    }

    match expected.map(resolve_struct_type) {
        Some(Type::Decimal) if matches!(op, Add | Sub | Mul | Div | Mod) => {
            coerce_exact_numeric_literal_to(left, &Type::Decimal)?;
            coerce_exact_numeric_literal_to(right, &Type::Decimal)?;
        }
        Some(Type::Quantity) => match op {
            Add | Sub | Mod => {
                coerce_exact_numeric_literal_to(left, &Type::Quantity)?;
                coerce_exact_numeric_literal_to(right, &Type::Quantity)?;
            }
            Mul | Div => {
                coerce_exact_numeric_literal_to(left, &Type::Quantity)?;
                coerce_exact_numeric_literal_to(right, &Type::Decimal)?;
            }
            _ => {}
        },
        _ => {}
    }

    Ok(())
}

fn list_element_contains_resource_handle(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Secret(_) | Type::StateMap(_, _) | Type::AssetHandle => true,
        Type::List(element, _) | Type::Option(element) => {
            list_element_contains_resource_handle(&element)
        }
        Type::Result(ok, err) => {
            list_element_contains_resource_handle(&ok)
                || list_element_contains_resource_handle(&err)
        }
        Type::Tuple(items) => items.iter().any(list_element_contains_resource_handle),
        Type::Struct { fields, .. } => fields
            .iter()
            .any(|(_, field)| list_element_contains_resource_handle(field)),
        _ => false,
    }
}

/// Return the recursively flattened V1 function-ABI word count, capped at one
/// more than `limit`.
///
/// Product fields are visited only until the caller's fixed ABI window is
/// exceeded. This is important for canonical named-struct DAGs: repeatedly
/// referring to the same shared branching type must not restore an expanded
/// tree walk after named-type resolution proved the graph itself was bounded.
pub(crate) fn runtime_value_word_count_bounded(ty: &Type, limit: usize) -> Option<usize> {
    fn count(ty: &Type, limit: usize) -> Option<usize> {
        let children: &[Type] = match ty {
            Type::Tuple(items) => items,
            Type::Struct { fields, .. } => {
                let mut total = 0_usize;
                for (_, field) in fields.iter() {
                    let remaining = limit.saturating_sub(total);
                    let words = count(field, remaining)?;
                    if words > remaining {
                        return Some(limit.saturating_add(1));
                    }
                    total = total.checked_add(words)?;
                }
                return Some(total);
            }
            Type::NamedStruct(_) => return None,
            // Every scalar and every compiler-owned Option, Result, or List
            // handle occupies exactly one function-ABI word. Product types are
            // the only shapes that can flatten to zero words in V1.
            _ => return Some(1),
        };

        let mut total = 0_usize;
        for child in children {
            let remaining = limit.saturating_sub(total);
            let words = count(child, remaining)?;
            if words > remaining {
                return Some(limit.saturating_add(1));
            }
            total = total.checked_add(words)?;
        }
        Some(total)
    }

    count(ty, limit)
}

fn zero_sized_list_element(ty: &Type) -> Option<Type> {
    match resolve_struct_type(ty) {
        Type::List(element, _) => {
            let element = *element;
            if runtime_value_word_count_bounded(&element, 0) == Some(0) {
                Some(element)
            } else {
                zero_sized_list_element(&element)
            }
        }
        Type::Struct { fields, .. } => fields
            .iter()
            .find_map(|(_, field)| zero_sized_list_element(field)),
        Type::Tuple(items) => items
            .into_iter()
            .find_map(|item| zero_sized_list_element(&item)),
        Type::Option(inner) | Type::Secret(inner) => zero_sized_list_element(&inner),
        Type::Result(ok, err) | Type::StateMap(ok, err) => {
            zero_sized_list_element(&ok).or_else(|| zero_sized_list_element(&err))
        }
        _ => None,
    }
}

fn validate_list_schemas(ty: &Type) -> Result<(), SemanticError> {
    let Some(element) = zero_sized_list_element(ty) else {
        return Ok(());
    };
    Err(SemanticError {
        code: "E_LIST_ZERO_SIZED_ELEMENT",
        message: format!(
            "List element type `{}` encodes to zero runtime words; add at least one runtime-valued field because List elements must encode at least one word",
            type_name(&element)
        ),
    })
}

fn validate_declared_struct_list_schemas(context: &SemanticContext) -> Result<(), SemanticError> {
    let structs = context.structs.borrow();
    let mut names = structs.keys().collect::<Vec<_>>();
    names.sort();
    for name in names {
        let fields = structs
            .get(name)
            .expect("collected struct name remains in the declaration table");
        for (_, field) in fields {
            validate_list_schemas(field)?;
        }
    }
    Ok(())
}

fn list_element_is_comparable(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Struct { fields, .. } => fields
            .iter()
            .all(|(_, field)| list_element_is_comparable(field)),
        Type::Tuple(items) => items.iter().all(list_element_is_comparable),
        Type::Option(inner) | Type::List(inner, _) => list_element_is_comparable(&inner),
        Type::Result(ok, err) => {
            list_element_is_comparable(&ok) && list_element_is_comparable(&err)
        }
        Type::Unit
        | Type::Secret(_)
        | Type::StateMap(_, _)
        | Type::AssetHandle
        | Type::NamedStruct(_) => false,
        Type::Int
        | Type::Decimal
        | Type::Quantity
        | Type::Bool
        | Type::String
        | Type::Bytes
        | Type::DataSpaceId
        | Type::AxtDescriptor
        | Type::ProofBlob
        | Type::SoracloudRequest
        | Type::SoracloudResponse
        | Type::AccountId
        | Type::AssetDefinitionId
        | Type::AssetId
        | Type::NftId
        | Type::DomainId
        | Type::Name
        | Type::Json => true,
    }
}

fn is_supported_public_argument_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Int
        | Type::Decimal
        | Type::Quantity
        | Type::Bool
        | Type::String
        | Type::Json
        | Type::Bytes
        | Type::AccountId
        | Type::AssetDefinitionId
        | Type::AssetId
        | Type::DomainId
        | Type::NftId
        | Type::Name
        | Type::DataSpaceId => true,
        Type::Struct { fields, .. } => fields
            .iter()
            .all(|(_, field_ty)| is_supported_public_argument_type(field_ty)),
        Type::Tuple(items) => items.iter().all(is_supported_public_argument_type),
        Type::Option(inner) => is_supported_public_argument_type(&inner),
        Type::Result(ok, err) => {
            is_supported_public_argument_type(&ok) && is_supported_public_argument_type(&err)
        }
        Type::List(element, _) => is_supported_public_argument_type(&element),
        Type::Unit
        | Type::Secret(_)
        | Type::StateMap(_, _)
        | Type::AxtDescriptor
        | Type::AssetHandle
        | Type::ProofBlob
        | Type::SoracloudRequest
        | Type::SoracloudResponse
        | Type::NamedStruct(_) => false,
    }
}

pub(crate) fn is_supported_durable_key_type(ty: &Type) -> bool {
    let resolved = resolve_struct_type(ty);
    let canonical_name = type_name(&resolved);
    V1_STATE_MAP_KEY_TYPE_NAMES.contains(&canonical_name.as_str())
}

fn is_in_memory_map_word_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        ty if is_numeric_type(&ty) => true,
        Type::Bool | Type::String | Type::Bytes | Type::Json => true,
        other if is_pointer_type(&other) => true,
        _ => false,
    }
}

fn ensure_in_memory_map_word_types(
    context: &SemanticContext,
    map_expr: &TypedExpr,
) -> Result<(), SemanticError> {
    if typed_map_expr_is_state(context, map_expr) {
        return Ok(());
    }
    if let Type::StateMap(k, v) = resolve_struct_type(&map_expr.ty) {
        if !is_in_memory_map_word_type(&k) {
            return Err(SemanticError {
                code: "K2003",
                message: format!(
                    "ephemeral map key type `{}` is not supported; use int, decimal, quantity, bool, string, bytes, Json, or typed Iroha IDs",
                    type_name(&k)
                ),
            });
        }
        if !is_in_memory_map_word_type(&v) {
            return Err(SemanticError {
                code: "K2003",
                message: format!(
                    "ephemeral map value type `{}` is not supported; use int, decimal, quantity, bool, string, bytes, Json, or typed Iroha IDs",
                    type_name(&v)
                ),
            });
        }
    }
    Ok(())
}

fn validate_state_type(ty: &Type) -> Result<(), SemanticError> {
    validate_state_type_inner(ty, true)
}

fn validate_state_type_inner(ty: &Type, allow_map: bool) -> Result<(), SemanticError> {
    if crate::secret::type_contains_secret(ty) {
        return Err(SemanticError {
            code: "E_SECRET_STATE_TYPE",
            message: "durable state cannot contain Secret<T>; private inputs are execution-local"
                .into(),
        });
    }
    match resolve_struct_type(ty) {
        Type::StateMap(k, v) => {
            if !allow_map {
                return Err(SemanticError {
                    code: "K2005",
                    message:
                        "nested StateMap is not supported in Kotodama V1; declare each StateMap as top-level state"
                            .into(),
                });
            }
            if !is_supported_durable_key_type(&k) {
                return Err(SemanticError {
                    code: "E_STATE_MAP_KEY_TYPE",
                    message: format!(
                        "StateMap key type `{}` is not supported for durable storage; use a scalar canonical-Norito type",
                        type_name(&k)
                    ),
                });
            }
            if !is_supported_durable_value_type(&v) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "StateMap value type `{}` is not supported for durable storage; use a canonical V1 value type",
                        type_name(&v)
                    ),
                });
            }
            Ok(())
        }
        Type::Struct { fields, .. } => {
            for (_, field_ty) in fields.iter() {
                validate_state_type_inner(field_ty, false)?;
            }
            Ok(())
        }
        Type::Tuple(items) => {
            for item in items {
                validate_state_type_inner(&item, false)?;
            }
            Ok(())
        }
        other => {
            if is_supported_durable_value_type(&other) {
                Ok(())
            } else {
                Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "state type `{}` is not supported for durable storage; use int, decimal, quantity, bool, Json, bytes, typed Iroha IDs, or aggregate V1 types",
                        type_name(&other)
                    ),
                })
            }
        }
    }
}

pub(crate) fn is_blob_like(ty: &Type) -> bool {
    matches!(resolve_struct_type(ty), Type::Bytes)
}

fn pointer_constructor_type(constructor: PointerConstructor) -> Type {
    match constructor {
        PointerConstructor::AccountId => Type::AccountId,
        PointerConstructor::AssetDefinition => Type::AssetDefinitionId,
        PointerConstructor::AssetId => Type::AssetId,
        PointerConstructor::NftId => Type::NftId,
        PointerConstructor::Domain | PointerConstructor::DomainId => Type::DomainId,
        PointerConstructor::Name => Type::Name,
        PointerConstructor::Json => Type::Json,
        PointerConstructor::Blob | PointerConstructor::NoritoBytes => Type::Bytes,
        PointerConstructor::DataSpaceId => Type::DataSpaceId,
        PointerConstructor::AxtDescriptor => Type::AxtDescriptor,
        PointerConstructor::AssetHandle => Type::AssetHandle,
        PointerConstructor::ProofBlob => Type::ProofBlob,
        PointerConstructor::SoracloudRequest => Type::SoracloudRequest,
        PointerConstructor::SoracloudResponse => Type::SoracloudResponse,
    }
}

fn is_eq_comparable_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        ty if is_numeric_type(&ty) => true,
        Type::Bool | Type::String | Type::Bytes | Type::Json => true,
        other if is_pointer_type(&other) => true,
        _ => false,
    }
}

pub fn is_pointer_type(ty: &Type) -> bool {
    matches!(
        resolve_struct_type(ty),
        Type::AccountId
            | Type::AssetDefinitionId
            | Type::AssetId
            | Type::DomainId
            | Type::NftId
            | Type::Name
            | Type::DataSpaceId
            | Type::AxtDescriptor
            | Type::AssetHandle
            | Type::ProofBlob
            | Type::SoracloudRequest
            | Type::SoracloudResponse
    )
}

const TRANSFER_BATCH_SIGNATURE: &str =
    "(AccountId, AccountId, AssetDefinitionId, quantity) tuple entries";

fn is_transfer_batch_entry_tuple(ty: &Type) -> bool {
    match ty {
        Type::Tuple(fields) if fields.len() == 4 => {
            matches!(resolve_struct_type(&fields[0]), Type::AccountId)
                && matches!(resolve_struct_type(&fields[1]), Type::AccountId)
                && matches!(resolve_struct_type(&fields[2]), Type::AssetDefinitionId)
                && matches!(resolve_struct_type(&fields[3]), Type::Quantity)
        }
        _ => false,
    }
}

fn ensure_transfer_batch_args(args: &mut [TypedExpr]) -> Result<(), SemanticError> {
    if args.is_empty() {
        return Err(SemanticError {
            code: "K2003",
            message: "transfer_batch expects at least one entry".into(),
        });
    }
    for argument in args.iter_mut() {
        let ExprKind::Tuple(items) = &mut argument.expr else {
            continue;
        };
        if items.len() != 4 {
            continue;
        }
        coerce_exact_numeric_literal_to(&mut items[3], &Type::Quantity)?;
        argument.ty = Type::Tuple(items.iter().map(|item| item.ty.clone()).collect());
    }
    if args
        .iter()
        .all(|expr| is_transfer_batch_entry_tuple(&expr.ty))
    {
        return Ok(());
    }
    Err(SemanticError {
        code: "K2003",
        message: format!("transfer_batch expects {}", TRANSFER_BATCH_SIGNATURE),
    })
}

/// Recursively bind nested struct fields into `name#i#j` variables for convenient lowering.
fn bind_struct_fields_rec(
    out: &mut Vec<TypedStatement>,
    vars: &mut HashMap<String, Type>,
    base_name: &str,
    base_expr: &TypedExpr,
    ty: &Type,
) {
    let resolved_ty = resolve_struct_type(ty);
    if let Type::Struct { fields, .. } = resolved_ty {
        for (i, (_fname, fty)) in fields.iter().enumerate() {
            // `base_expr` has already been captured by the synthetic binding
            // named `base_name`. Project from that binding so an effectful
            // aggregate expression is never evaluated once per field.
            let captured = TypedExpr {
                expr: ExprKind::Ident(base_name.to_owned()),
                ty: base_expr.ty.clone(),
            };
            let member = TypedExpr {
                expr: ExprKind::Member {
                    object: Box::new(captured),
                    field: i.to_string(),
                },
                ty: resolve_struct_type(fty),
            };
            let sname = format!("{base_name}#{i}");
            let field_ty = resolve_struct_type(fty);
            vars.insert(sname.clone(), field_ty.clone());
            out.push(TypedStatement::Let {
                name: sname.clone(),
                value: member.clone(),
            });
            bind_struct_fields_rec(out, vars, &sname, &member, &field_ty);
        }
    }
}

/// Recursively bind tuple elements into `name#i` variables so older lowering
/// helpers can access flattened names. Nested structs continue to use the
/// existing struct binding helper, and nested tuples recurse naturally.
fn bind_tuple_fields_rec(
    out: &mut Vec<TypedStatement>,
    vars: &mut HashMap<String, Type>,
    base_name: &str,
    base_expr: &TypedExpr,
    ty: &Type,
) {
    if let Type::Tuple(elements) = resolve_struct_type(ty) {
        for (idx, elem_ty) in elements.iter().enumerate() {
            let resolved_elem_ty = resolve_struct_type(elem_ty);
            // Tuple literals and calls are both evaluated by the parent
            // binding. Synthetic flattened names only project that captured
            // value; cloning a literal item here would also duplicate calls
            // nested inside the literal.
            let element_expr = TypedExpr {
                expr: ExprKind::Member {
                    object: Box::new(TypedExpr {
                        expr: ExprKind::Ident(base_name.to_owned()),
                        ty: base_expr.ty.clone(),
                    }),
                    field: idx.to_string(),
                },
                ty: resolved_elem_ty.clone(),
            };
            let child_name = format!("{base_name}#{idx}");
            vars.insert(child_name.clone(), resolved_elem_ty.clone());
            out.push(TypedStatement::Let {
                name: child_name.clone(),
                value: element_expr.clone(),
            });
            bind_tuple_fields_rec(out, vars, &child_name, &element_expr, &resolved_elem_ty);
            bind_struct_fields_rec(out, vars, &child_name, &element_expr, &resolved_elem_ty);
        }
    }
}

fn analyze_function(
    context: &SemanticContext,
    func: &Function,
) -> Result<TypedFunction, SemanticError> {
    context.discard_diagnostic();
    context.required_list_capacity.borrow_mut().take();
    if matches!(
        func.modifiers.kind,
        FunctionKind::Hajimari | FunctionKind::Kaizen
    ) && func.modifiers.permission.is_some()
    {
        return Err(SemanticError {
            code: "E_LIFECYCLE_AUTHORIZATION",
            message: format!(
                "lifecycle function `{}` cannot declare caller authorization; lifecycle authorization is runtime-defined",
                func.name
            ),
        });
    }
    if func.modifiers.is_test {
        if !func.params.is_empty() {
            return Err(SemanticError {
                code: "E_TEST_FUNCTION_SIGNATURE",
                message: format!("test function `{}` must not declare parameters", func.name),
            });
        }
        if func.ret_ty.is_some() {
            return Err(SemanticError {
                code: "K2003",
                message: format!(
                    "test function `{}` must not declare a return type",
                    func.name
                ),
            });
        }
        if func.modifiers.kind != FunctionKind::Private {
            return Err(SemanticError {
                code: "E_TEST_FUNCTION_SIGNATURE",
                message: format!(
                    "test function `{}` must be declared as a local `fn`",
                    func.name
                ),
            });
        }
        if func.modifiers.permission.is_some() {
            return Err(SemanticError {
                code: "K2004",
                message: format!(
                    "test function `{}` cannot declare a permission modifier",
                    func.name
                ),
            });
        }
    }
    let mut vars = HashMap::new();
    let mut mutable_bindings = HashSet::new();
    let mut param_names = Vec::new();
    let mut param_types = Vec::new();
    // Seed variable environment with seiyaku-level state declarations so
    // functions can reference `state` names directly.
    {
        let states = context.states.borrow();
        for (name, ty) in states.iter() {
            vars.insert(name.clone(), ty.clone());
        }
    }
    let mut state_param_names = HashSet::new();
    for param in &func.params {
        ensure_new_local_binding(context, &param.name, &vars)?;
        let typed_param = parse_declared_param_type(context, param, &func.modifiers)?;
        vars.insert(param.name.clone(), typed_param.ty.clone());
        if typed_param.is_state {
            state_param_names.insert(param.name.clone());
        }
        param_names.push(param.name.clone());
        param_types.push(typed_param);
    }
    let expected_ret = parse_declared_type(context, &func.ret_ty)?;
    if func.modifiers.kind != FunctionKind::Private
        && expected_ret
            .as_ref()
            .is_some_and(crate::secret::type_contains_secret)
    {
        context.capture_diagnostic(
            func.ret_ty.as_ref().and_then(|ty| context.type_source(ty)),
            None,
        );
        return Err(SemanticError {
            code: "E_SECRET_PUBLIC_RETURN",
            message: format!(
                "externally callable `{}` cannot return Secret<T>; return an approved commitment or proof result",
                func.name
            ),
        });
    }
    let previous_modifiers = context
        .current_function_modifiers
        .borrow_mut()
        .replace(func.modifiers.clone());
    let previous_name = context
        .current_function_name
        .borrow_mut()
        .replace(func.name.clone());
    let previous_mutable_bindings =
        std::mem::take(&mut *context.current_mutable_bindings.borrow_mut());
    let previous_state_params = std::mem::replace(
        &mut *context.current_state_param_names.borrow_mut(),
        state_param_names.clone(),
    );
    let unit_return = Type::Unit;
    let expected_tail = expected_ret.as_ref().unwrap_or(&unit_return);
    let body_result = analyze_block(
        context,
        &func.body,
        &mut vars,
        &mut mutable_bindings,
        expected_ret.as_ref(),
        Some(expected_tail),
        0,
    );
    *context.current_function_modifiers.borrow_mut() = previous_modifiers;
    *context.current_function_name.borrow_mut() = previous_name;
    *context.current_mutable_bindings.borrow_mut() = previous_mutable_bindings;
    *context.current_state_param_names.borrow_mut() = previous_state_params;
    let body = body_result?;
    // Enforce declared return coverage and shape
    if let Some(t) = &expected_ret {
        if *t != Type::Unit && body.tail.is_none() && !typed_block_diverges(&body) {
            return Err(SemanticError {
                code: "E_MISSING_RETURN",
                message: "not all paths return a value".into(),
            });
        }
    } else {
        // No declared return type: disallow returning a value to avoid ambiguity
        if block_has_return_value(&func.body) {
            return Err(SemanticError {
                code: "K2003",
                message: "function returns a value but has no declared return type".into(),
            });
        }
    }
    let summary = FunctionSummary {
        direct_effects: FunctionEffects {
            host_side_effects: block_contains_host_side_effects(&body),
            emits_instructions: block_contains_instruction_emission(&body),
            mutates_durable_state: block_mutates_durable_state(context, &body),
        },
        calls: collect_called_functions(context, &body),
    };
    context
        .function_summaries
        .borrow_mut()
        .insert(func.name.clone(), summary);
    Ok(TypedFunction {
        name: func.name.clone(),
        params: param_names,
        param_types,
        body,
        ret_ty: expected_ret,
        modifiers: func.modifiers.clone(),
        location: func.location,
        source: None,
        name_source: None,
    })
}

fn reject_public_trigger_event(context: &SemanticContext, name: &str) -> Result<(), SemanticError> {
    let forbidden = context
        .current_function_modifiers
        .borrow()
        .as_ref()
        .is_some_and(|modifiers| match modifiers.kind {
            FunctionKind::View => true,
            FunctionKind::Kotoage | FunctionKind::Hajimari | FunctionKind::Kaizen => {
                !current_public_trigger_callback_allows_payload_helper(context)
            }
            FunctionKind::Private => false,
        });
    if forbidden {
        return Err(SemanticError {
            code: "K2003",
            message: format!(
                "`kotoage`/`言挙げ`, `view fn`, `hajimari`/`始まり`, and `kaizen`/`改善` declarations cannot use `{name}` here; declare typed parameters instead"
            ),
        });
    }
    Ok(())
}

fn current_public_trigger_callback_allows_payload_helper(context: &SemanticContext) -> bool {
    let current = context.current_function_name.borrow().clone();
    let Some(current) = current else {
        return false;
    };
    context
        .trigger_callback_functions
        .borrow()
        .contains(&current)
}

fn current_function_is_test(context: &SemanticContext) -> bool {
    context
        .current_function_modifiers
        .borrow()
        .as_ref()
        .is_some_and(|modifiers| modifiers.is_test)
}

fn function_is_runtime_entrypoint(modifiers: &FunctionModifiers) -> bool {
    modifiers.kind != FunctionKind::Private
}

fn invoke_entrypoint_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            invoke_entrypoint_literal(expression)
        }
        Expr::String(raw) => Some(raw.clone()),
        Expr::Call { name, args, .. }
            if normalize_namespaced(name) == "name" && args.len() == 1 =>
        {
            match args[0].kind() {
                Expr::String(raw) => Some(raw.clone()),
                Expr::Source { .. } | Expr::Resolved { .. } => {
                    unreachable!("kind() strips provenance wrappers")
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn typed_string_literal(value: String) -> TypedExpr {
    TypedExpr {
        expr: ExprKind::String(value),
        ty: Type::String,
    }
}

fn validate_require_error_variant(
    context: &SemanticContext,
    args: &[Expr],
) -> Result<(), SemanticError> {
    if args.len() != 2 {
        return Err(SemanticError {
            code: "K2003",
            message: "require expects (bool, ErrorEnum::Variant)".into(),
        });
    }
    let Expr::Ident(error_variant) = args[1].kind() else {
        return Err(SemanticError {
            code: "K2003",
            message: "require expects a declared error variant as its second argument".into(),
        });
    };
    if !context.error_codes.borrow().contains_key(error_variant) {
        return Err(SemanticError {
            code: "K2002",
            message: format!("unknown error variant `{error_variant}`"),
        });
    }
    Ok(())
}

fn analyze_invoke_entrypoint_call(
    context: &SemanticContext,
    args: &[Expr],
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    if !current_function_is_test(context) {
        return Err(SemanticError {
            code: "E_TEST_BUILTIN_CONTEXT",
            message: "`test::invoke_kotoage` is only available inside #[test] Kotodama functions"
                .into(),
        });
    }
    if args.len() != 2 {
        return Err(SemanticError {
            code: "K2003",
            message: "test::invoke_kotoage expects (string|Name literal, Json)".into(),
        });
    }

    let target_name = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        code: "E_TEST_ENTRYPOINT_LITERAL",
        message:
            "test::invoke_kotoage requires a literal public or lifecycle target such as \"run\" or Name::parse(\"run\")"
                .into(),
    })?;
    let payload = analyze_expr(context, &args[1], vars)?;
    if payload.ty != Type::Json {
        return Err(SemanticError {
            code: "K2003",
            message: "test::invoke_kotoage expects a Json payload as its second argument".into(),
        });
    }

    let ret_ty = runtime_entrypoint_return_type(context, &target_name)?;

    Ok(TypedExpr {
        expr: ExprKind::Call {
            name: format!("__invoke_entrypoint__{target_name}"),
            args: vec![payload],
        },
        ty: ret_ty,
    })
}

fn runtime_entrypoint_return_type(
    context: &SemanticContext,
    target_name: &str,
) -> Result<Type, SemanticError> {
    if let Some(modifiers) = context
        .function_modifiers
        .borrow()
        .get(target_name)
        .cloned()
    {
        if !function_is_runtime_entrypoint(&modifiers) {
            return Err(SemanticError {
                code: "E_TEST_ENTRYPOINT_KIND",
                message: format!(
                    "runtime test helpers may only target kotoage/view/hajimari/kaizen declarations, got `{target_name}`"
                ),
            });
        }
        return Ok(context
            .function_returns
            .borrow()
            .get(target_name)
            .cloned()
            .unwrap_or(Type::Unit));
    }

    if let Some(signature) = context
        .external_functions
        .borrow()
        .get(target_name)
        .cloned()
    {
        if !function_is_runtime_entrypoint(&signature.modifiers) {
            return Err(SemanticError {
                code: "E_TEST_ENTRYPOINT_KIND",
                message: format!(
                    "runtime test helpers may only target kotoage/view/hajimari/kaizen declarations, got `{target_name}`"
                ),
            });
        }
        return Ok(signature.return_type);
    }

    Err(SemanticError {
        code: "K2002",
        message: format!("unknown runtime public or lifecycle target `{target_name}`"),
    })
}

fn analyze_invoke_entrypoint_as_call(
    context: &SemanticContext,
    args: &[Expr],
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    if !current_function_is_test(context) {
        return Err(SemanticError {
            code: "E_TEST_BUILTIN_CONTEXT",
            message:
                "`test::invoke_kotoage_as` is only available inside #[test] Kotodama functions"
                    .into(),
        });
    }
    if args.len() != 3 {
        return Err(SemanticError {
            code: "K2003",
            message: "test::invoke_kotoage_as expects (string|Name literal actor, string|Name literal kotoage, Json)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        code: "E_TEST_ACTOR_LITERAL",
        message: "test::invoke_kotoage_as requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")".into(),
    })?;
    let target_name = invoke_entrypoint_literal(&args[1]).ok_or_else(|| SemanticError {
        code: "E_TEST_ENTRYPOINT_LITERAL",
        message: "test::invoke_kotoage_as requires a literal public or lifecycle target such as \"run\" or Name::parse(\"run\")".into(),
    })?;
    let payload = analyze_expr(context, &args[2], vars)?;
    if payload.ty != Type::Json {
        return Err(SemanticError {
            code: "K2003",
            message: "test::invoke_kotoage_as expects a Json payload as its third argument".into(),
        });
    }
    let ret_ty = runtime_entrypoint_return_type(context, &target_name)?;

    Ok(TypedExpr {
        expr: ExprKind::Call {
            name: "invoke_entrypoint_as".to_string(),
            args: vec![
                typed_string_literal(actor),
                typed_string_literal(target_name),
                payload,
            ],
        },
        ty: ret_ty,
    })
}

fn analyze_expect_reject_as_call(
    context: &SemanticContext,
    args: &[Expr],
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    if !current_function_is_test(context) {
        return Err(SemanticError {
            code: "E_TEST_BUILTIN_CONTEXT",
            message: "`expect_reject_as` is only available inside #[test] Kotodama functions"
                .into(),
        });
    }
    if args.len() != 3 {
        return Err(SemanticError {
            code: "K2003",
            message: "test::expect_reject_as expects (string|Name literal actor, string|Name literal kotoage, Json)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        code: "E_TEST_ACTOR_LITERAL",
        message:
            "expect_reject_as requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")"
                .into(),
    })?;
    let target_name = invoke_entrypoint_literal(&args[1]).ok_or_else(|| SemanticError {
        code: "E_TEST_ENTRYPOINT_LITERAL",
        message:
            "test::expect_reject_as requires a literal public or lifecycle target such as \"run\" or Name::parse(\"run\")"
                .into(),
    })?;
    let payload = analyze_expr(context, &args[2], vars)?;
    if payload.ty != Type::Json {
        return Err(SemanticError {
            code: "K2003",
            message: "test::expect_reject_as expects a Json payload as its third argument".into(),
        });
    }
    let _ = runtime_entrypoint_return_type(context, &target_name)?;

    Ok(TypedExpr {
        expr: ExprKind::Call {
            name: "expect_reject_as".to_string(),
            args: vec![
                typed_string_literal(actor),
                typed_string_literal(target_name),
                payload,
            ],
        },
        ty: Type::Unit,
    })
}

fn analyze_actor_account_call(
    context: &SemanticContext,
    args: &[Expr],
) -> Result<TypedExpr, SemanticError> {
    if !current_function_is_test(context) {
        return Err(SemanticError {
            code: "E_TEST_BUILTIN_CONTEXT",
            message: "`actor_account` is only available inside #[test] Kotodama functions".into(),
        });
    }
    if args.len() != 1 {
        return Err(SemanticError {
            code: "K2003",
            message: "actor_account expects (string|Name literal actor)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        code: "E_TEST_ACTOR_LITERAL",
        message:
            "actor_account requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")"
                .into(),
    })?;
    Ok(TypedExpr {
        expr: ExprKind::Call {
            name: "actor_account".to_string(),
            args: vec![typed_string_literal(actor)],
        },
        ty: Type::AccountId,
    })
}

fn analyze_actor_public_key_call(
    context: &SemanticContext,
    args: &[Expr],
) -> Result<TypedExpr, SemanticError> {
    if !current_function_is_test(context) {
        return Err(SemanticError {
            code: "E_TEST_BUILTIN_CONTEXT",
            message: "`actor_public_key` is only available inside #[test] Kotodama functions"
                .into(),
        });
    }
    if args.len() != 1 {
        return Err(SemanticError {
            code: "K2003",
            message: "actor_public_key expects (string|Name literal actor)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        code: "E_TEST_ACTOR_LITERAL",
        message:
            "actor_public_key requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")"
                .into(),
    })?;
    Ok(TypedExpr {
        expr: ExprKind::Call {
            name: "actor_public_key".to_string(),
            args: vec![typed_string_literal(actor)],
        },
        ty: Type::Bytes,
    })
}

fn analyze_actor_sign_call(
    context: &SemanticContext,
    args: &[Expr],
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    if !current_function_is_test(context) {
        return Err(SemanticError {
            code: "E_TEST_BUILTIN_CONTEXT",
            message: "`actor_sign` is only available inside #[test] Kotodama functions".into(),
        });
    }
    if args.len() != 2 {
        return Err(SemanticError {
            code: "K2003",
            message: "actor_sign expects (string|Name literal actor, bytes)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        code: "E_TEST_ACTOR_LITERAL",
        message: "actor_sign requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")"
            .into(),
    })?;
    let message = analyze_expr(context, &args[1], vars)?;
    if !is_blob_like(&message.ty) {
        return Err(SemanticError {
            code: "K2003",
            message: "actor_sign expects the message as bytes".into(),
        });
    }
    Ok(TypedExpr {
        expr: ExprKind::Call {
            name: "actor_sign".to_string(),
            args: vec![typed_string_literal(actor), message],
        },
        ty: Type::Bytes,
    })
}

fn analyze_block(
    context: &SemanticContext,
    block: &Block,
    vars: &mut HashMap<String, Type>,
    mutable_bindings: &mut HashSet<String>,
    expected_ret: Option<&Type>,
    expected_tail: Option<&Type>,
    loop_depth: usize,
) -> Result<TypedBlock, SemanticError> {
    let previous_mutable_bindings = context
        .current_mutable_bindings
        .replace(mutable_bindings.clone());
    let result = (|| {
        let _ = loop_depth;
        let mut statements = Vec::new();
        for stmt in &block.statements {
            let mut v = analyze_statement(
                context,
                stmt,
                vars,
                mutable_bindings,
                expected_ret,
                loop_depth,
            )?;
            statements.append(&mut v);
        }
        let tail = if let Some(expression) = &block.tail {
            let mut typed = analyze_expr_expected(context, expression, vars, expected_tail)?;
            if let Some(expected) = expected_tail
                && let Err(mut error) = ensure_assignable_and_coerce(expected, &mut typed)
            {
                context.capture_expression_diagnostic(expression, None);
                error.code = "E_TAIL_TYPE_MISMATCH";
                error.message = format!("block tail type mismatch: {}", error.message);
                return Err(error);
            }
            Some(Box::new(typed))
        } else {
            None
        };
        Ok(TypedBlock { statements, tail })
    })();
    context
        .current_mutable_bindings
        .replace(previous_mutable_bindings);
    result
}

fn validate_v1_bounded_for_shape(
    init: &Option<Box<Statement>>,
    cond: &Option<Expr>,
    step: &Option<Box<Statement>>,
) -> Result<(), SemanticError> {
    let Some(init) = init.as_deref() else {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "only `for item in range(non_negative_literal)` is supported in Kotodama V1"
                .into(),
        });
    };
    let Statement::Let {
        mutable: true,
        pat: Pattern::Name(variable),
        ty: None,
        value,
    } = init.kind()
    else {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "only `for item in range(non_negative_literal)` is supported in Kotodama V1"
                .into(),
        });
    };
    if !matches!(value.kind(), Expr::IntLiteral(value) if value.is_zero()) {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "range loops must start from zero".into(),
        });
    }
    let Some(cond) = cond.as_ref() else {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "bounded range loop is missing its compiler-proven condition".into(),
        });
    };
    let Expr::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = cond.kind()
    else {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "bounded range loop is missing its compiler-proven condition".into(),
        });
    };
    if !matches!(left.kind(), Expr::Ident(name) if name == variable)
        || !matches!(right.kind(), Expr::IntLiteral(value) if !value.is_negative())
    {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "range bounds must be non-negative integer literals".into(),
        });
    }
    let Some(step) = step.as_deref() else {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "bounded range loop is missing its canonical step".into(),
        });
    };
    let Statement::Assign {
        name,
        value: step_value,
    } = step.kind()
    else {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "bounded range loop is missing its canonical step".into(),
        });
    };
    let Expr::Binary {
        op: BinaryOp::Add,
        left: step_left,
        right: step_right,
    } = step_value.kind()
    else {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "bounded range loop is missing its canonical increment".into(),
        });
    };
    if name != variable
        || !matches!(step_left.kind(), Expr::Ident(name) if name == variable)
        || !matches!(step_right.kind(), Expr::IntLiteral(value) if value == &BigInt::one())
    {
        return Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "range loop control variables cannot be rewritten".into(),
        });
    }
    Ok(())
}

fn analyze_statement(
    context: &SemanticContext,
    stmt: &Statement,
    vars: &mut HashMap<String, Type>,
    mutable_bindings: &mut HashSet<String>,
    expected_ret: Option<&Type>,
    loop_depth: usize,
) -> Result<Vec<TypedStatement>, SemanticError> {
    let result = analyze_statement_inner(
        context,
        stmt,
        vars,
        mutable_bindings,
        expected_ret,
        loop_depth,
    );
    if result.is_err() {
        context.capture_statement_diagnostic(stmt, None);
    }
    result
}

fn analyze_statement_inner(
    context: &SemanticContext,
    stmt: &Statement,
    vars: &mut HashMap<String, Type>,
    mutable_bindings: &mut HashSet<String>,
    expected_ret: Option<&Type>,
    loop_depth: usize,
) -> Result<Vec<TypedStatement>, SemanticError> {
    let _ = loop_depth;
    let kind = stmt.kind();
    let statement_node = context.validate_statement_node(stmt)?;
    if !matches!(kind, Statement::Assign { .. })
        && statement_node
            .as_ref()
            .is_some_and(|node| node.target.is_some())
    {
        return Err(SemanticError {
            code: "E_INTERNAL_RESOLUTION",
            message: "non-assignment statement carries a resolver assignment target".into(),
        });
    }
    match kind {
        Statement::Source { .. } | Statement::Resolved { .. } => {
            unreachable!("kind() strips AST and resolved-HIR provenance wrappers")
        }
        Statement::Let {
            mutable,
            pat,
            ty,
            value,
        } => {
            let declared = ty
                .as_ref()
                .map(|annotation| convert_type_expr(context, annotation))
                .transpose()?
                .map(|ty| resolve_struct_type_with_context(context, &ty))
                .transpose()?;
            if let Some(declared) = &declared {
                validate_list_schemas(declared)?;
            }
            let mut expr = match analyze_expr_expected(context, value, vars, declared.as_ref()) {
                Ok(expression) => expression,
                Err(error) => {
                    if error.code == "E_LIST_COMPREHENSION_CAPACITY"
                        && matches!(value.kind(), Expr::ListComprehension { .. })
                        && let (Some(annotation), Some(Type::List(element, _)), Some(capacity)) = (
                            ty.as_ref(),
                            declared.as_ref(),
                            context.required_list_capacity.borrow_mut().take(),
                        )
                    {
                        let replacement =
                            render_source_type_name(&Type::List(element.clone(), capacity));
                        context.replace_diagnostic(
                            context.type_source(annotation),
                            Some(crate::semantic_diagnostics::SemanticFix::Replace { replacement }),
                        );
                    }
                    return Err(error);
                }
            };
            if let Some(dt) = &declared {
                apply_map_new_type_hint(&mut expr, dt);
                if let Err(error) = ensure_assignable_and_coerce(dt, &mut expr) {
                    if error.code == "E_QUERY_RESULT_TYPE"
                        && let Some(annotation) = ty.as_ref()
                    {
                        context.replace_diagnostic(
                            context.type_source(annotation),
                            Some(crate::semantic_diagnostics::SemanticFix::Replace {
                                replacement: render_source_type_name(&expr.ty),
                            }),
                        );
                    }
                    return Err(error);
                }
            }
            if is_state_map_expr(context, &expr) {
                return Err(SemanticError {
                    code: "E_STATE_MAP_ALIAS",
                    message: "state maps are not first-class; use the state identifier directly."
                        .into(),
                });
            }
            match pat {
                Pattern::Name(name) => {
                    if name == "_" {
                        return Ok(vec![TypedStatement::Let {
                            name: name.clone(),
                            value: expr,
                        }]);
                    }
                    ensure_new_local_binding(context, name, vars)?;
                    // Bind the name and, if it's a tuple, also synthesize per-field bindings name#i.
                    let mut out = Vec::new();
                    vars.insert(name.clone(), expr.ty.clone());
                    if *mutable {
                        mutable_bindings.insert(name.clone());
                        context
                            .current_mutable_bindings
                            .borrow_mut()
                            .insert(name.clone());
                    }
                    out.push(TypedStatement::Let {
                        name: name.clone(),
                        value: expr.clone(),
                    });
                    match &expr.ty {
                        Type::Tuple(_) => {
                            bind_tuple_fields_rec(&mut out, vars, name, &expr, &expr.ty);
                        }
                        Type::Struct { fields, .. } => {
                            for (i, (_fname, fty)) in fields.iter().enumerate() {
                                let val_expr = TypedExpr {
                                    expr: ExprKind::Member {
                                        object: Box::new(TypedExpr {
                                            expr: ExprKind::Ident(name.clone()),
                                            ty: expr.ty.clone(),
                                        }),
                                        field: i.to_string(),
                                    },
                                    ty: fty.clone(),
                                };
                                let sname = format!("{name}#{i}");
                                let field_ty = resolve_struct_type(fty);
                                vars.insert(sname.clone(), field_ty.clone());
                                out.push(TypedStatement::Let {
                                    name: sname.clone(),
                                    value: val_expr.clone(),
                                });
                                bind_struct_fields_rec(
                                    &mut out, vars, &sname, &val_expr, &field_ty,
                                );
                            }
                        }
                        _ => {}
                    }
                    Ok(out)
                }
                Pattern::Tuple(names) => {
                    let mut out = Vec::new();
                    for name in names.iter() {
                        if name != "_" {
                            ensure_new_local_binding(context, name, vars)?;
                        }
                    }
                    let mut unique_names = HashSet::new();
                    for name in names {
                        if name != "_" && !unique_names.insert(name) {
                            return Err(SemanticError {
                                code: "K2001",
                                message: format!(
                                    "duplicate binding `{name}` in destructuring declaration"
                                ),
                            });
                        }
                    }
                    match &expr.ty {
                        Type::Tuple(ts) => {
                            if names.len() != ts.len() {
                                return Err(SemanticError {
                                    code: "K2003",
                                    message: format!(
                                        "tuple destructuring expects {} bindings, got {}",
                                        ts.len(),
                                        names.len()
                                    ),
                                });
                            }
                            let capture_name = context.fresh_aggregate_capture();
                            let captured = TypedExpr {
                                expr: ExprKind::Ident(capture_name.clone()),
                                ty: expr.ty.clone(),
                            };
                            vars.insert(capture_name.clone(), expr.ty.clone());
                            out.push(TypedStatement::Let {
                                name: capture_name,
                                value: expr.clone(),
                            });
                            // Destructure by emitting member-access typed expressions for each field.
                            for (i, name) in names.iter().enumerate() {
                                let ti = ts.get(i).cloned().expect("tuple arity already validated");
                                let member = TypedExpr {
                                    expr: ExprKind::Member {
                                        object: Box::new(captured.clone()),
                                        field: i.to_string(),
                                    },
                                    ty: ti.clone(),
                                };
                                if name != "_" {
                                    vars.insert(name.clone(), ti.clone());
                                    if *mutable {
                                        mutable_bindings.insert(name.clone());
                                        context
                                            .current_mutable_bindings
                                            .borrow_mut()
                                            .insert(name.clone());
                                    }
                                }
                                out.push(TypedStatement::Let {
                                    name: name.clone(),
                                    value: member,
                                });
                            }
                        }
                        Type::Struct { fields, .. } => {
                            if names.len() != fields.len() {
                                return Err(SemanticError {
                                    code: "K2003",
                                    message: format!(
                                        "struct destructuring expects {} bindings, got {}",
                                        fields.len(),
                                        names.len()
                                    ),
                                });
                            }
                            let capture_name = context.fresh_aggregate_capture();
                            let captured = TypedExpr {
                                expr: ExprKind::Ident(capture_name.clone()),
                                ty: expr.ty.clone(),
                            };
                            vars.insert(capture_name.clone(), expr.ty.clone());
                            out.push(TypedStatement::Let {
                                name: capture_name,
                                value: expr.clone(),
                            });
                            for (i, name) in names.iter().enumerate() {
                                let (_fname, ti) = fields
                                    .get(i)
                                    .cloned()
                                    .expect("struct arity already validated");
                                let val_expr = TypedExpr {
                                    expr: ExprKind::Member {
                                        object: Box::new(captured.clone()),
                                        field: i.to_string(),
                                    },
                                    ty: resolve_struct_type(&ti),
                                };
                                let field_ty = resolve_struct_type(&ti);
                                if name != "_" {
                                    vars.insert(name.clone(), field_ty.clone());
                                    if *mutable {
                                        mutable_bindings.insert(name.clone());
                                        context
                                            .current_mutable_bindings
                                            .borrow_mut()
                                            .insert(name.clone());
                                    }
                                }
                                out.push(TypedStatement::Let {
                                    name: name.clone(),
                                    value: val_expr.clone(),
                                });
                                bind_struct_fields_rec(&mut out, vars, name, &val_expr, &field_ty);
                            }
                        }
                        _ => {
                            return Err(SemanticError {
                                code: "K2003",
                                message: "tuple destructuring expects a tuple or struct".into(),
                            });
                        }
                    }
                    Ok(out)
                }
            }
        }
        Statement::Assign { name, value } => {
            context.validate_assignment_target(statement_node.as_ref(), name)?;
            // Must exist
            let expected = vars.get(name).cloned().ok_or_else(|| SemanticError {
                code: "K2002",
                message: format!("undefined variable {name}"),
            })?;
            if is_state_binding(context, name)
                && matches!(resolve_struct_type(&expected), Type::StateMap(_, _))
            {
                return Err(SemanticError {
                    code: "E_STATE_MAP_ALIAS",
                    message: "state maps cannot be reassigned; use map indexing.".into(),
                });
            }
            ensure_mutable_assignment_target(context, name, mutable_bindings)?;
            let mut expr = analyze_expr_expected(context, value, vars, Some(&expected))?;
            if is_state_binding(context, name) {
                crate::secret::reject_secret_state_value(&expr)?;
            }
            if is_state_map_expr(context, &expr) {
                return Err(SemanticError {
                    code: "E_STATE_MAP_ALIAS",
                    message: "state maps are not first-class; use the state identifier directly."
                        .into(),
                });
            }
            apply_map_new_type_hint(&mut expr, &expected);
            ensure_assignable_and_coerce(&expected, &mut expr)?;
            // Rebind SSA name to new value
            vars.insert(name.clone(), expr.ty.clone());
            let mut out = Vec::new();
            out.push(TypedStatement::Let {
                name: name.clone(),
                value: expr.clone(),
            });
            bind_tuple_fields_rec(&mut out, vars, name, &expr, &expr.ty);
            Ok(out)
        }
        Statement::AssignExpr { target, op, value } => {
            // support map indexing and simple variable rebinding
            match target.kind() {
                Expr::Index { target: map, index } => {
                    let map_t = analyze_expr(context, map, vars)?;
                    let mut key_t = analyze_expr(context, index, vars)?;
                    crate::secret::reject_secret_key(&key_t)?;
                    match map_t.ty.clone() {
                        Type::StateMap(k, v) => {
                            ensure_assignable_and_coerce(&k, &mut key_t)?;
                            ensure_in_memory_map_word_types(context, &map_t)?;
                            if *op == AssignOp::Set {
                                let mut val_t =
                                    analyze_expr_expected(context, value, vars, Some(&v))?;
                                crate::secret::reject_secret_state_value(&val_t)?;
                                ensure_assignable_and_coerce(&v, &mut val_t)?;
                                return Ok(vec![TypedStatement::MapSet {
                                    map: map_t,
                                    key: key_t,
                                    value: Box::new(val_t),
                                }]);
                            }
                            Err(SemanticError {
                                code: "E_STATE_MAP_OPTIONAL_READ",
                                message: "compound StateMap assignment reads a possibly absent key; use `map.get(key)` and handle Option<V> before assigning with `map[key] = value`"
                                .into(),
                            })
                        }
                        Type::List(element, _) => {
                            let receiver_is_mutable = matches!(map.kind(), Expr::Ident(name) if mutable_bindings.contains(name));
                            let fix = if *op == AssignOp::Set
                                && receiver_is_mutable
                                && resolve_struct_type(&key_t.ty) == Type::Int
                                && context.expression_is_assignable(value, vars, &element)
                            {
                                match (
                                    context.expression_source(map),
                                    context.expression_source(index),
                                    context.expression_source(value),
                                ) {
                                    (Some(target), Some(index), Some(value)) => {
                                        Some(crate::semantic_diagnostics::SemanticFix::ListTrySet {
                                            target,
                                            index,
                                            value,
                                        })
                                    }
                                    _ => None,
                                }
                            } else {
                                None
                            };
                            context.capture_statement_diagnostic(stmt, fix);
                            Err(SemanticError {
                                code: "E_LIST_UNSAFE_INDEX",
                                message: "unchecked List writes are not part of Kotodama V1; use `list.try_set(index: index, value: value)`; its bool result reports whether the mutation occurred"
                                    .into(),
                            })
                        }
                        other => Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "map assignment expects StateMap<K,V> target, got {}",
                                type_name(&other)
                            ),
                        }),
                    }
                }
                Expr::Ident(name) => {
                    context.validate_value_target(target, name, vars)?;
                    // Simple compound assignment lowering: rebind SSA name
                    let expected = vars.get(name).cloned().ok_or_else(|| SemanticError {
                        code: "K2002",
                        message: format!("undefined variable {name}"),
                    })?;
                    if is_state_binding(context, name)
                        && matches!(resolve_struct_type(&expected), Type::StateMap(_, _))
                    {
                        return Err(SemanticError {
                            code: "E_STATE_MAP_ALIAS",
                            message: "state maps cannot be reassigned; use map indexing.".into(),
                        });
                    }
                    ensure_mutable_assignment_target(context, name, mutable_bindings)?;
                    let mut expr = if *op == AssignOp::Set {
                        analyze_expr_expected(context, value, vars, Some(&expected))?
                    } else {
                        analyze_expr(context, value, vars)?
                    };
                    if is_state_binding(context, name) {
                        crate::secret::reject_secret_state_value(&expr)?;
                    }
                    if is_state_map_expr(context, &expr) {
                        return Err(SemanticError {
                            code: "E_STATE_MAP_ALIAS",
                            message:
                                "state maps are not first-class; use the state identifier directly."
                                    .into(),
                        });
                    }
                    apply_map_new_type_hint(&mut expr, &expected);
                    if *op == AssignOp::Set {
                        ensure_assignable_and_coerce(&expected, &mut expr)?;
                        vars.insert(name.clone(), expr.ty.clone());
                        let mut out = Vec::new();
                        out.push(TypedStatement::Let {
                            name: name.clone(),
                            value: expr.clone(),
                        });
                        bind_tuple_fields_rec(&mut out, vars, name, &expr, &expr.ty);
                        return Ok(out);
                    }
                    let bin_op = assign_op_to_binary(*op).expect("compound op maps to binary op");
                    let mut left = TypedExpr {
                        expr: ExprKind::Ident(name.clone()),
                        ty: expected.clone(),
                    };
                    coerce_contextual_numeric_literals(
                        bin_op,
                        Some(&expected),
                        &mut left,
                        &mut expr,
                    )?;
                    reject_implicit_int_decimal_mix(&left.ty, &expr.ty)?;
                    let Some(result_ty) = arithmetic_result_type(bin_op, &left.ty, &expr.ty) else {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "compound operator {op:?} is not defined for {} and {}",
                                type_name(&left.ty),
                                type_name(&expr.ty),
                            ),
                        });
                    };
                    if resolve_struct_type(&result_ty) != resolve_struct_type(&expected) {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "compound operator {op:?} produces {}, which cannot be assigned to {} without an explicit conversion",
                                type_name(&result_ty),
                                type_name(&expected),
                            ),
                        });
                    }
                    let value_expr = TypedExpr {
                        expr: ExprKind::Binary {
                            op: bin_op,
                            left: Box::new(left),
                            right: Box::new(expr),
                        },
                        ty: result_ty,
                    };
                    vars.insert(name.clone(), value_expr.ty.clone());
                    let mut out = Vec::new();
                    out.push(TypedStatement::Let {
                        name: name.clone(),
                        value: value_expr.clone(),
                    });
                    bind_tuple_fields_rec(&mut out, vars, name, &value_expr, &value_expr.ty);
                    Ok(out)
                }
                _ => Err(SemanticError {
                    code: "E_INVALID_ASSIGNMENT_TARGET",
                    message: "assignment target must be a variable or map index".into(),
                }),
            }
        }
        Statement::Expr(e) => Ok(vec![TypedStatement::Expr(analyze_expr(context, e, vars)?)]),
        Statement::Return(opt) => {
            let mut tv = if let Some(e) = opt {
                Some(analyze_expr_expected(context, e, vars, expected_ret)?)
            } else {
                None
            };
            if expected_ret.is_none() {
                if tv.is_some() {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "returning a value requires a declared return type".into(),
                    });
                }
            } else if let Some(exp) = expected_ret {
                match tv.as_mut() {
                    None => {
                        if !matches!(exp, Type::Unit) {
                            return Err(SemanticError {
                                code: "K2003",
                                message: "return type mismatch: expected value".into(),
                            });
                        }
                    }
                    Some(expr) => {
                        apply_map_new_type_hint(expr, exp);
                        if matches!(exp, Type::Unit) {
                            return Err(SemanticError {
                                code: "K2003",
                                message: "return type mismatch: unexpected value".into(),
                            });
                        }
                        if let Err(mut err) = ensure_assignable_and_coerce(exp, expr) {
                            err.code = "E_RETURN_TYPE_MISMATCH";
                            err.message = format!("return type mismatch: {}", err.message);
                            return Err(err);
                        }
                    }
                }
            }
            Ok(vec![TypedStatement::Return(tv)])
        }
        Statement::Break => {
            if loop_depth == 0 {
                return Err(SemanticError {
                    code: "E_BREAK_OUTSIDE_LOOP",
                    message: "`break` must appear inside a loop".into(),
                });
            }
            Ok(vec![TypedStatement::Break])
        }
        Statement::Continue => {
            if loop_depth == 0 {
                return Err(SemanticError {
                    code: "E_CONTINUE_OUTSIDE_LOOP",
                    message: "`continue` must appear inside a loop".into(),
                });
            }
            Ok(vec![TypedStatement::Continue])
        }
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let cond_t = analyze_expr(context, cond, vars)?;
            crate::secret::reject_secret_control_flow(&cond_t)?;
            if cond_t.ty != Type::Bool {
                return Err(SemanticError {
                    code: "K2003",
                    message: "if condition must be bool".into(),
                });
            }
            let then_block = analyze_block(
                context,
                then_branch,
                &mut vars.clone(),
                &mut mutable_bindings.clone(),
                expected_ret,
                None,
                loop_depth,
            )?;
            let else_block = if let Some(b) = else_branch {
                Some(analyze_block(
                    context,
                    b,
                    &mut vars.clone(),
                    &mut mutable_bindings.clone(),
                    expected_ret,
                    None,
                    loop_depth,
                )?)
            } else {
                None
            };
            Ok(vec![TypedStatement::If {
                cond: cond_t,
                then_branch: then_block,
                else_branch: else_block,
            }])
        }
        Statement::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => {
            let value = analyze_expr(context, value, vars)?;
            let (pattern, binding) = analyze_sum_pattern(pattern, &value.ty)?;
            let mut then_vars = vars.clone();
            if let Some((name, ty)) = binding {
                ensure_new_local_binding(context, &name, &then_vars)?;
                then_vars.insert(name, ty);
            }
            let then_branch = analyze_block(
                context,
                then_branch,
                &mut then_vars,
                &mut mutable_bindings.clone(),
                expected_ret,
                None,
                loop_depth,
            )?;
            let else_branch = if let Some(block) = else_branch {
                Some(analyze_block(
                    context,
                    block,
                    &mut vars.clone(),
                    &mut mutable_bindings.clone(),
                    expected_ret,
                    None,
                    loop_depth,
                )?)
            } else {
                None
            };
            Ok(vec![TypedStatement::IfLet {
                pattern,
                value,
                then_branch,
                else_branch,
            }])
        }
        Statement::While { .. } => Err(SemanticError {
            code: "E_UNBOUNDED_LOOP",
            message: "`while` is not part of Kotodama V1; use a compiler-proven bounded `for` loop"
                .into(),
        }),
        Statement::For {
            line,
            init,
            cond,
            step,
            body,
        } => {
            validate_v1_bounded_for_shape(init, cond, step)?;
            let mut local = vars.clone();
            let mut local_mutable_bindings = mutable_bindings.clone();
            let init_t = if let Some(s) = init {
                let mut v = analyze_statement(
                    context,
                    s,
                    &mut local,
                    &mut local_mutable_bindings,
                    expected_ret,
                    loop_depth,
                )?;
                if v.len() != 1 {
                    return Err(SemanticError {
                        code: "E_FOR_INITIALIZER",
                        message: "for-loop initializer must be a simple let or expression".into(),
                    });
                }
                Some(Box::new(v.remove(0)))
            } else {
                None
            };
            let loop_env = local.clone();
            let cond_t = if let Some(c) = cond {
                let mut cond_vars = loop_env.clone();
                let t = analyze_expr(context, c, &mut cond_vars)?;
                crate::secret::reject_secret_control_flow(&t)?;
                if t.ty != Type::Bool {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "for condition must be bool".into(),
                    });
                }
                Some(t)
            } else {
                None
            };
            let step_t = if let Some(s) = step {
                let mut step_vars = loop_env.clone();
                let mut v = analyze_statement(
                    context,
                    s,
                    &mut step_vars,
                    &mut local_mutable_bindings.clone(),
                    expected_ret,
                    loop_depth + 1,
                )?;
                if v.len() != 1 {
                    return Err(SemanticError {
                        code: "E_FOR_STEP",
                        message: "for-loop step must be a simple let or expression".into(),
                    });
                }
                Some(Box::new(v.remove(0)))
            } else {
                None
            };
            let body_t = analyze_block(
                context,
                body,
                &mut loop_env.clone(),
                &mut local_mutable_bindings.clone(),
                expected_ret,
                None,
                loop_depth + 1,
            )?;
            *vars = loop_env;
            Ok(vec![TypedStatement::For {
                line: *line,
                init: init_t,
                cond: cond_t,
                step: step_t,
                body: body_t,
            }])
        }
        Statement::ForEachMap {
            key,
            value,
            map,
            body,
        } => {
            // Accept canonical bounded forms: `.take(end)` and `.range(start, end)`.
            // Desugar to a typed for-each with the base map expression and rely on
            // IR lowering to enforce the exact compiler-proven literal bound.
            if let Expr::Call {
                name,
                args,
                implicit_receiver: true,
                ..
            } = map.kind()
            {
                if name == "take" && args.len() == 2 {
                    // Analyze base map expression and infer key/value types
                    let base_map = analyze_expr(context, &args[0], &mut vars.clone())?;
                    // Extend a local scope with loop variables bound to inferred types
                    ensure_state_map_iter_supported(context, &base_map)?;
                    ensure_in_memory_map_word_types(context, &base_map)?;
                    let mut local_vars = vars.clone();
                    let (k_ty, v_ty) = match &base_map.ty {
                        Type::StateMap(k, v) => ((**k).clone(), (**v).clone()),
                        _ => (Type::Int, Type::Int),
                    };
                    ensure_new_local_binding(context, key, &local_vars)?;
                    local_vars.insert(key.clone(), k_ty);
                    if let Some(val_name) = value {
                        ensure_new_local_binding(context, val_name, &local_vars)?;
                        local_vars.insert(val_name.clone(), v_ty);
                    }
                    let body_t = analyze_block(
                        context,
                        body,
                        &mut local_vars,
                        &mut mutable_bindings.clone(),
                        expected_ret,
                        None,
                        loop_depth + 1,
                    )?;
                    let literal_bound = match args[1].kind() {
                        Expr::Source { .. } | Expr::Resolved { .. } => {
                            unreachable!("kind() strips provenance wrappers")
                        }
                        Expr::IntLiteral(n) if !n.is_negative() => {
                            let value = n.try_to_u64().ok_or_else(|| SemanticError {
                                code: "E_UNBOUNDED_ITERATION",
                                message: "`.take(n)` requires an int literal no greater than 64"
                                    .into(),
                            })?;
                            enforce_static_iteration_limit("StateMap.take(N)", u128::from(value))?;
                            Some(usize::try_from(value).expect("V1 iteration bound is at most 64"))
                        }
                        _ => None,
                    };
                    if let Some(bound) = literal_bound {
                        if bound > 1 && !map_expr_is_state(context, &args[0]) {
                            return Err(SemanticError {
                                code: "E_MAP_BOUNDS",
                                message: "ephemeral map iteration supports at most 1 element; reduce the bound or move the map into `state`.".into(),
                            });
                        }
                        // E_ITER_MUTATION: forbid structural modifications to the iterated map inside the loop body
                        if let Expr::Ident(map_name) = args[0].kind()
                            && block_mutates_map(&body_t, map_name)
                        {
                            return Err(SemanticError { code: "E_ITER_MUTATION", message: "structural modifications to the iterated map are forbidden during iteration".into() });
                        }
                        return Ok(vec![TypedStatement::ForEachMap {
                            key: key.clone(),
                            value: value.clone(),
                            map: base_map,
                            body: body_t,
                            start: 0,
                            bound: Some(bound),
                            bound_kind: StateMapIterationBoundKind::Take,
                        }]);
                    }
                    return Err(SemanticError {
                        code: "E_UNBOUNDED_ITERATION",
                        message:
                            "`.take(n)` requires a non-negative int literal no greater than 64"
                                .into(),
                    });
                }
                if name == "range" && args.len() == 3 {
                    // range(start, end)
                    let base_map = analyze_expr(context, &args[0], &mut vars.clone())?;
                    ensure_state_map_iter_supported(context, &base_map)?;
                    ensure_in_memory_map_word_types(context, &base_map)?;
                    let mut local_vars = vars.clone();
                    let (k_ty, v_ty) = match &base_map.ty {
                        Type::StateMap(k, v) => ((**k).clone(), (**v).clone()),
                        _ => (Type::Int, Type::Int),
                    };
                    ensure_new_local_binding(context, key, &local_vars)?;
                    local_vars.insert(key.clone(), k_ty);
                    if let Some(val_name) = value {
                        ensure_new_local_binding(context, val_name, &local_vars)?;
                        local_vars.insert(val_name.clone(), v_ty);
                    }
                    let body_t = analyze_block(
                        context,
                        body,
                        &mut local_vars,
                        &mut mutable_bindings.clone(),
                        expected_ret,
                        None,
                        loop_depth + 1,
                    )?;
                    let start = match args[1].kind() {
                        Expr::Source { .. } | Expr::Resolved { .. } => {
                            unreachable!("kind() strips provenance wrappers")
                        }
                        Expr::IntLiteral(n) if !n.is_negative() => n.try_to_u64(),
                        _ => None,
                    };
                    // Interpret second numeric as end; compute n = end - start
                    let end = match args[2].kind() {
                        Expr::Source { .. } | Expr::Resolved { .. } => {
                            unreachable!("kind() strips provenance wrappers")
                        }
                        Expr::IntLiteral(n) if !n.is_negative() => n.try_to_u64(),
                        _ => None,
                    };
                    if let (Some(start), Some(end)) = (start, end) {
                        if end < start {
                            return Err(SemanticError {
                                code: "E_UNBOUNDED_ITERATION",
                                message: "`.range(start, end)` requires end >= start".into(),
                            });
                        }
                        let span = end - start;
                        enforce_static_iteration_limit(
                            "StateMap.range(start, end)",
                            u128::from(span),
                        )?;
                        if !map_expr_is_state(context, &args[0]) && (start != 0 || span > 1) {
                            return Err(SemanticError {
                                code: "E_MAP_BOUNDS",
                                message: "ephemeral map iteration supports at most 1 element starting at index 0; reduce the range or move the map into `state`."
                                    .into(),
                            });
                        }
                        let static_bound =
                            Some(usize::try_from(span).expect("V1 iteration span is at most 64"));
                        if let Expr::Ident(map_name) = args[0].kind()
                            && block_mutates_map(&body_t, map_name)
                        {
                            return Err(SemanticError { code: "E_ITER_MUTATION", message: "structural modifications to the iterated map are forbidden during iteration".into() });
                        }
                        return Ok(vec![TypedStatement::ForEachMap {
                            key: key.clone(),
                            value: value.clone(),
                            map: base_map,
                            body: body_t,
                            start,
                            bound: static_bound,
                            bound_kind: StateMapIterationBoundKind::Range,
                        }]);
                    }
                    return Err(SemanticError {
                        code: "E_UNBOUNDED_ITERATION",
                        message: "`.range(start, end)` requires non-negative int literals with a span no greater than 64".into(),
                    });
                }
            }
            Err(SemanticError {
                code: "E_UNBOUNDED_ITERATION",
                message: "`for (k, v) in map` requires a literal bound; call `.take(N)` or `.range(start, end)` on the StateMap expression.".into(),
            })
        }
    }
}

fn query_helper_accepts_arg(builtin: Builtin, ty: &Type) -> bool {
    match builtin {
        Builtin::QueryExecuteNorito
        | Builtin::QueryGetContractManifest
        | Builtin::ZkRootsGet
        | Builtin::ZkVoteGetTally
        | Builtin::VrfEpochSeed => is_blob_like(ty),
        Builtin::QueryGetAccount => matches!(ty, Type::AccountId),
        Builtin::QueryGetAsset => matches!(ty, Type::AssetId),
        Builtin::QueryGetAssetDefinition => matches!(ty, Type::AssetDefinitionId),
        Builtin::QueryGetDomain => matches!(ty, Type::DomainId),
        Builtin::QueryGetNft => matches!(ty, Type::NftId),
        Builtin::QueryGetParameter => matches!(ty, Type::Name) || is_blob_like(ty),
        Builtin::QueryGetContractInstance => matches!(ty, Type::Name) || is_blob_like(ty),
        _ => false,
    }
}

fn core_query_view_type(builtin: Builtin) -> Option<Type> {
    let (name, fields) = match builtin {
        Builtin::QueryGetAccount | Builtin::QueryPageAccounts => (
            "AccountView",
            vec![("id", Type::AccountId), ("metadata", Type::Json)],
        ),
        Builtin::QueryGetAsset | Builtin::QueryPageAssets => (
            "AssetView",
            vec![("id", Type::AssetId), ("amount", Type::Quantity)],
        ),
        Builtin::QueryGetAssetDefinition | Builtin::QueryPageAssetDefinitions => (
            "AssetDefinitionView",
            vec![
                ("id", Type::AssetDefinitionId),
                ("name", Type::String),
                ("description", Type::Option(Box::new(Type::String))),
                ("owned_by", Type::AccountId),
                ("total_quantity", Type::Quantity),
                ("metadata", Type::Json),
            ],
        ),
        Builtin::QueryGetDomain | Builtin::QueryPageDomains => (
            "DomainView",
            vec![
                ("id", Type::DomainId),
                ("owned_by", Type::AccountId),
                ("metadata", Type::Json),
            ],
        ),
        Builtin::QueryGetNft | Builtin::QueryPageNfts => (
            "NftView",
            vec![
                ("id", Type::NftId),
                ("owned_by", Type::AccountId),
                ("content", Type::Json),
            ],
        ),
        _ => return None,
    };
    Some(Type::Struct {
        name: name.to_owned(),
        fields: Arc::from(
            fields
                .into_iter()
                .map(|(field, ty)| (field.to_owned(), ty))
                .collect::<Vec<_>>(),
        ),
    })
}

fn query_page_type(view: Type) -> Result<Type, SemanticError> {
    let Type::Struct {
        name: view_name, ..
    } = &view
    else {
        return Err(SemanticError {
            code: "K2003",
            message: "QueryPage<T> requires one of the five declared core query view types".into(),
        });
    };
    if !matches!(
        view_name.as_str(),
        "AccountView" | "AssetView" | "AssetDefinitionView" | "DomainView" | "NftView"
    ) {
        return Err(SemanticError {
            code: "E_QUERY_PAGE_VIEW",
            message: format!(
                "QueryPage<{view_name}> is unsupported; pages are available only for declared core query views"
            ),
        });
    }
    Ok(Type::Struct {
        // The projection specialization is encoded by the recursive List
        // child. Keeping the nominal name canonical avoids smuggling generic
        // syntax into an ABI identifier while preserving exact schema identity.
        name: QUERY_PAGE_TYPE_NAME.to_owned(),
        fields: Arc::from(vec![
            ("items".to_owned(), Type::List(Box::new(view), 64)),
            ("next_offset".to_owned(), Type::Option(Box::new(Type::Int))),
        ]),
    })
}

fn query_page_view_type(ty: &Type) -> Option<&Type> {
    let Type::Struct { name, fields } = ty else {
        return None;
    };
    let [
        (items_name, Type::List(view, capacity)),
        (next_name, Type::Option(next_offset)),
    ] = fields.as_ref()
    else {
        return None;
    };
    (name == QUERY_PAGE_TYPE_NAME
        && items_name == "items"
        && *capacity == 64
        && next_name == "next_offset"
        && next_offset.as_ref() == &Type::Int)
        .then_some(view.as_ref())
}

fn core_query_page_type(builtin: Builtin) -> Type {
    query_page_type(
        core_query_view_type(builtin)
            .expect("only projected plural core-query builtins request QueryPage types"),
    )
    .expect("projected plural core-query builtins use supported view types")
}

fn direct_json_getter_type(builtin: Builtin) -> Option<Type> {
    let payload = match builtin {
        Builtin::JsonGetIntDirect => Type::Int,
        Builtin::JsonGetDecimalDirect => Type::Decimal,
        Builtin::JsonGetQuantityDirect => Type::Quantity,
        Builtin::JsonGetJsonDirect => Type::Json,
        Builtin::JsonGetNameDirect => Type::Name,
        Builtin::JsonGetAccountIdDirect => Type::AccountId,
        Builtin::JsonGetAssetDefinitionIdDirect => Type::AssetDefinitionId,
        Builtin::JsonGetNftIdDirect => Type::NftId,
        Builtin::JsonGetBlobHexDirect => Type::Bytes,
        _ => return None,
    };
    Some(Type::Option(Box::new(payload)))
}

fn canonicalize_builtin_result<T>(
    builtin: Builtin,
    result: Result<T, SemanticError>,
) -> Result<T, SemanticError> {
    result.map_err(|mut error| {
        if builtin.name() != builtin.source_name() {
            error.message =
                replace_identifier_token(&error.message, builtin.name(), builtin.source_name());
        }
        error
    })
}

fn replace_identifier_token(message: &str, needle: &str, replacement: &str) -> String {
    fn is_identifier_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':')
    }

    let mut rewritten = String::with_capacity(message.len() + replacement.len());
    let mut copied_until = 0;
    for (start, _) in message.match_indices(needle) {
        let end = start + needle.len();
        let has_identifier_prefix = message[..start]
            .bytes()
            .next_back()
            .is_some_and(is_identifier_byte);
        let has_identifier_suffix = message[end..]
            .bytes()
            .next()
            .is_some_and(is_identifier_byte);
        if has_identifier_prefix || has_identifier_suffix {
            continue;
        }
        rewritten.push_str(&message[copied_until..start]);
        rewritten.push_str(replacement);
        copied_until = end;
    }
    rewritten.push_str(&message[copied_until..]);
    rewritten
}

fn coerce_builtin_exact_numeric_literals(
    builtin: Builtin,
    arguments: &mut [TypedExpr],
) -> Result<(), SemanticError> {
    for (argument, parameter) in arguments
        .iter_mut()
        .zip(builtin.signature().parameters.iter().copied())
    {
        let expected = match parameter.strip_suffix('?').unwrap_or(parameter) {
            "decimal" => Type::Decimal,
            "quantity" => Type::Quantity,
            _ => continue,
        };
        coerce_exact_numeric_literal_to(argument, &expected)?;
    }
    Ok(())
}

fn analyze_surface_builtin_call(
    context: &SemanticContext,
    builtin: Builtin,
    mut arg_typed: Vec<TypedExpr>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    match builtin.spec().mode {
        BuiltinMode::CompilerInternal => {
            return Err(SemanticError {
                code: "E_INTERNAL_BUILTIN",
                message: format!(
                    "builtin `{}` is compiler-internal and is not available in Kotodama V1 source",
                    builtin.name()
                ),
            });
        }
        BuiltinMode::ZkOnly if !context.zk_enabled => {
            return Err(SemanticError {
                code: "E_ZK_MODE_REQUIRED",
                message: format!(
                    "builtin `{}` requires ZK mode in compiler build configuration",
                    builtin.name()
                ),
            });
        }
        BuiltinMode::TestOnly | BuiltinMode::TestFunctionOnly if !context.test_builtins_enabled => {
            return Err(SemanticError {
                code: "E_TEST_ONLY_PRODUCTION",
                message: format!(
                    "builtin `{}` requires explicit compiler test mode",
                    builtin.source_name()
                ),
            });
        }
        BuiltinMode::TestFunctionOnly if !current_function_is_test(context) => {
            return Err(SemanticError {
                code: "E_TEST_BUILTIN_CONTEXT",
                message: format!(
                    "builtin `{}` is available only inside a #[test] function",
                    builtin.source_name()
                ),
            });
        }
        BuiltinMode::Any
        | BuiltinMode::ZkOnly
        | BuiltinMode::TestOnly
        | BuiltinMode::TestFunctionOnly => {}
    }
    coerce_builtin_exact_numeric_literals(builtin, &mut arg_typed)?;
    crate::secret::validate_builtin_call(builtin, &arg_typed)?;
    match builtin {
        Builtin::ContractInvokeQuantity2 => {
            if arg_typed.len() != 5
                || resolve_struct_type(&arg_typed[0].ty) != Type::Bytes
                || resolve_struct_type(&arg_typed[1].ty) != Type::String
                || resolve_struct_type(&arg_typed[2].ty) != Type::String
                || resolve_struct_type(&arg_typed[3].ty) != Type::Quantity
                || resolve_struct_type(&arg_typed[4].ty) != Type::Quantity
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "contract::invoke requires named `(contract: bytes, entrypoint: string, returns: \"quantity\", amount_in: quantity, min_out: quantity)` arguments"
                        .into(),
                });
            }
            let ExprKind::String(entrypoint) = arg_typed[1].kind() else {
                return Err(SemanticError {
                    code: "E_CONTRACT_ENTRYPOINT_LITERAL",
                    message: "contract::invoke requires a literal `entrypoint` selector".into(),
                });
            };
            if !ivm_abi::entrypoint::is_canonical_kotodama_identifier(entrypoint) {
                return Err(SemanticError {
                    code: "E_CONTRACT_ENTRYPOINT_LITERAL",
                    message: "contract::invoke entrypoint must be a canonical Kotodama identifier"
                        .into(),
                });
            }
            if !matches!(arg_typed[2].kind(), ExprKind::String(value) if value == "quantity") {
                return Err(SemanticError {
                    code: "E_CONTRACT_RETURN_SCHEMA",
                    message:
                        "the first production contract::invoke profile requires literal `returns: \"quantity\"`"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_owned(),
                    args: arg_typed,
                },
                ty: Type::Quantity,
            })
        }
        Builtin::PointerConstructor(constructor) => {
            let name = constructor.name();
            if arg_typed.len() != 1 {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects one argument"),
                });
            }
            let arg_ty = resolve_struct_type(&arg_typed[0].ty);
            let ty = pointer_constructor_type(constructor);
            if arg_ty != Type::String {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects string"),
                });
            }
            if constructor == PointerConstructor::Json
                && let ExprKind::String(raw) = arg_typed[0].kind()
                && let Err(error) = parse_json_literal(raw)
            {
                return Err(error);
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty,
            })
        }
        Builtin::GetOrDefault => {
            if arg_typed.len() != 3 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "get_or_default expects (StateMap<K,V>, K, V)".into(),
                });
            }
            let (key_ty, value_ty) = match &arg_typed[0].ty {
                Type::StateMap(k, v) => (k.as_ref().clone(), v.as_ref().clone()),
                other => {
                    return Err(SemanticError {
                        code: "K2003",
                        message: format!(
                            "get_or_default expects StateMap<K,V> as first arg, got {}",
                            type_name(other)
                        ),
                    });
                }
            };
            ensure_assignable_and_coerce(&key_ty, &mut arg_typed[1])?;
            ensure_assignable_and_coerce(&value_ty, &mut arg_typed[2])?;
            ensure_in_memory_map_word_types(context, &arg_typed[0])?;
            let value_ty = match resolve_struct_type(&arg_typed[0].ty) {
                Type::StateMap(_, v) => *v,
                _ => Type::Int,
            };
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: resolve_struct_type(&value_ty),
            })
        }
        Builtin::Contains => {
            if arg_typed.len() != 2 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "contains expects (StateMap<K,V>, K)".into(),
                });
            }
            match &arg_typed[0].ty {
                Type::StateMap(k, _v) => {
                    ensure_assignable_and_coerce(&k.clone(), &mut arg_typed[1])?;
                    ensure_in_memory_map_word_types(context, &arg_typed[0])?;
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: builtin.name().to_string(),
                            args: arg_typed,
                        },
                        ty: Type::Bool,
                    })
                }
                other => Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "contains expects StateMap<K,V> as first arg, got {}",
                        type_name(other)
                    ),
                }),
            }
        }
        Builtin::GetOr => {
            let original_len = arg_typed.len();
            if original_len != 2 && original_len != 3 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "get_or expects (StateMap<K,V>, K[, V])".into(),
                });
            }
            let mut call_args = arg_typed;
            let map_ty = resolve_struct_type(&call_args[0].ty);
            let (map_key_ty, map_value_ty) = match map_ty {
                Type::StateMap(k, v) => (*k, *v),
                other => {
                    return Err(SemanticError {
                        code: "K2003",
                        message: format!(
                            "get_or expects StateMap<K,V> as first arg, got {}",
                            type_name(&other)
                        ),
                    });
                }
            };
            let resolved_key_ty = resolve_struct_type(&map_key_ty);
            let resolved_value_ty = resolve_struct_type(&map_value_ty);
            ensure_assignable_and_coerce(&resolved_key_ty, &mut call_args[1])?;
            ensure_in_memory_map_word_types(context, &call_args[0])?;

            if original_len == 2 {
                match resolve_struct_type(&resolved_value_ty) {
                    Type::Int => {
                        call_args.push(TypedExpr {
                            expr: ExprKind::IntLiteral(BigInt::zero()),
                            ty: Type::Int,
                        });
                    }
                    other => {
                        if is_pointer_type(&other) {
                            return Err(SemanticError {
                                code: "K2003",
                                message: format!(
                                    "get_or requires an explicit default for pointer-valued maps (value type {})",
                                    type_name(&other)
                                ),
                            });
                        }
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "get_or auto-default is only available for StateMap<*,int>; provide an explicit default for value type {}",
                                type_name(&other)
                            ),
                        });
                    }
                }
            } else {
                ensure_assignable_and_coerce(&resolved_value_ty, &mut call_args[2])?;
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: call_args,
                },
                ty: resolved_value_ty,
            })
        }
        Builtin::Ensure => {
            let in_view = context
                .current_function_modifiers
                .borrow()
                .as_ref()
                .is_some_and(|modifiers| modifiers.kind == FunctionKind::View);
            if in_view {
                return Err(SemanticError {
                    code: "K2004",
                    message: "`view fn` functions cannot use mutating map helper `ensure`; use `get_or` instead".into(),
                });
            }
            let original_len = arg_typed.len();
            if original_len != 2 && original_len != 3 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "ensure expects (StateMap<K,V>, K[, V])".into(),
                });
            }
            let mut call_args = arg_typed;
            let map_ty = resolve_struct_type(&call_args[0].ty);
            let (map_key_ty, map_value_ty) = match map_ty {
                Type::StateMap(k, v) => (*k, *v),
                other => {
                    return Err(SemanticError {
                        code: "K2003",
                        message: format!(
                            "ensure expects StateMap<K,V> as first arg, got {}",
                            type_name(&other)
                        ),
                    });
                }
            };
            let resolved_key_ty = resolve_struct_type(&map_key_ty);
            let resolved_value_ty = resolve_struct_type(&map_value_ty);
            ensure_assignable_and_coerce(&resolved_key_ty, &mut call_args[1])?;
            ensure_in_memory_map_word_types(context, &call_args[0])?;

            if original_len == 2 {
                match resolve_struct_type(&resolved_value_ty) {
                    Type::Int => {
                        call_args.push(TypedExpr {
                            expr: ExprKind::IntLiteral(BigInt::zero()),
                            ty: Type::Int,
                        });
                    }
                    other => {
                        if is_pointer_type(&other) {
                            return Err(SemanticError {
                                code: "K2003",
                                message: format!(
                                    "ensure requires an explicit default for pointer-valued maps (value type {})",
                                    type_name(&other)
                                ),
                            });
                        }
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "ensure auto-default is only available for StateMap<*,int>; provide an explicit default for value type {}",
                                type_name(&other)
                            ),
                        });
                    }
                }
            } else {
                ensure_assignable_and_coerce(&resolved_value_ty, &mut call_args[2])?;
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: call_args,
                },
                ty: resolved_value_ty,
            })
        }
        Builtin::StateMapRemove => {
            if arg_typed.len() != 2 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "StateMap.remove expects exactly one key argument".into(),
                });
            }
            if !typed_map_expr_is_state(context, &arg_typed[0]) {
                return Err(SemanticError {
                    code: "K2005",
                    message: "StateMap.remove is available only on declared durable state maps"
                        .into(),
                });
            }
            let Type::StateMap(key, value) = resolve_struct_type(&arg_typed[0].ty) else {
                return Err(SemanticError {
                    code: "K2003",
                    message: "StateMap.remove receiver must be StateMap<K, V>".into(),
                });
            };
            debug_assert!(is_supported_durable_value_type(&value));
            ensure_assignable_and_coerce(&key, &mut arg_typed[1])?;
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Option(value),
            })
        }
        Builtin::KeysTake2 | Builtin::ValuesTake2 => {
            let name = builtin.name();
            if arg_typed.len() != 3 {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (StateMap<int,int>, int start, int which)"),
                });
            }
            match &arg_typed[0].ty {
                Type::StateMap(k, v)
                    if matches!(resolve_struct_type(k), Type::Int)
                        && matches!(resolve_struct_type(v), Type::Int) => {}
                other => {
                    return Err(SemanticError {
                        code: "K2003",
                        message: format!(
                            "{name} expects StateMap<int,int> as first arg, got {}",
                            type_name(other)
                        ),
                    });
                }
            }
            if !matches!(resolve_struct_type(&arg_typed[1].ty), Type::Int)
                || !matches!(resolve_struct_type(&arg_typed[2].ty), Type::Int)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (StateMap<int,int>, int, int)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::KeysValuesTake2 => {
            if arg_typed.len() != 3 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "keys_values_take2 expects (StateMap<int,int>, int, int)".into(),
                });
            }
            match &arg_typed[0].ty {
                Type::StateMap(k, v)
                    if matches!(resolve_struct_type(k), Type::Int)
                        && matches!(resolve_struct_type(v), Type::Int) => {}
                other => {
                    return Err(SemanticError {
                        code: "K2003",
                        message: format!(
                            "keys_values_take2 expects StateMap<int,int> as first arg, got {}",
                            type_name(other)
                        ),
                    });
                }
            }
            if !matches!(resolve_struct_type(&arg_typed[1].ty), Type::Int)
                || !matches!(resolve_struct_type(&arg_typed[2].ty), Type::Int)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "keys_values_take2 expects (StateMap<int,int>, int, int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Tuple(vec![Type::Int, Type::Int]),
            })
        }
        Builtin::StateGet => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "state_get expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::StateSet => {
            if arg_typed.len() != 2
                || !(arg_typed[0].ty == Type::Name && is_blob_like(&arg_typed[1].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "state_set expects (Name, bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::StateDel => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "state_del expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::StateKeys => {
            if arg_typed.len() != 3
                || arg_typed[0].ty != Type::Name
                || !is_int_like(&arg_typed[1].ty)
                || !is_int_like(&arg_typed[2].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "state_keys expects (Name, int offset, int limit)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::StateHas => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "state_has expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bool,
            })
        }
        Builtin::StateLen => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "state_len expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::StateCount => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "state_count expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::QueryGetAccount
        | Builtin::QueryGetAsset
        | Builtin::QueryGetAssetDefinition
        | Builtin::QueryGetDomain
        | Builtin::QueryGetNft => {
            if arg_typed.len() != 1 || !query_helper_accepts_arg(builtin, &arg_typed[0].ty) {
                let expected = match builtin {
                    Builtin::QueryGetAccount => "AccountId",
                    Builtin::QueryGetAsset => "AssetId",
                    Builtin::QueryGetAssetDefinition => "AssetDefinitionId",
                    Builtin::QueryGetDomain => "DomainId",
                    Builtin::QueryGetNft => "NftId",
                    _ => unreachable!(),
                };
                return Err(SemanticError {
                    code: "E_QUERY_KEY_TYPE",
                    message: format!(
                        "`{}` expects one `{expected}` argument; byte-returning core-query compatibility is not part of Kotodama V1",
                        builtin.source_name()
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Option(Box::new(
                    core_query_view_type(builtin)
                        .expect("singular projected core-query builtin has a view type"),
                )),
            })
        }
        Builtin::QueryPageAccounts
        | Builtin::QueryPageAssets
        | Builtin::QueryPageAssetDefinitions
        | Builtin::QueryPageDomains
        | Builtin::QueryPageNfts => {
            if arg_typed.len() != 2 || arg_typed[0].ty != Type::Int || arg_typed[1].ty != Type::Int
            {
                return Err(SemanticError {
                    code: "E_QUERY_PAGE_ARGUMENTS",
                    message: format!(
                        "`{}` expects named `offset: int` and `limit: int` arguments",
                        builtin.source_name()
                    ),
                });
            }
            if literal_int(&arg_typed[0]).is_some_and(|offset| offset.try_to_u64().is_none()) {
                return Err(SemanticError {
                    code: "E_QUERY_OFFSET",
                    message: "query page offset must be non-negative and fit u64".into(),
                });
            }
            if literal_int(&arg_typed[1]).is_some_and(|limit| {
                limit
                    .try_to_u64()
                    .is_none_or(|limit| !(1..=64).contains(&limit))
            }) {
                return Err(SemanticError {
                    code: "E_QUERY_LIMIT",
                    message: "query page limit must be in 1..=64".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: core_query_page_type(builtin),
            })
        }
        Builtin::QueryExecuteNorito
        | Builtin::QueryGetParameter
        | Builtin::QueryGetContractManifest
        | Builtin::QueryGetContractInstance
        | Builtin::ZkRootsGet
        | Builtin::ZkVoteGetTally
        | Builtin::VrfEpochSeed => {
            if arg_typed.len() != 1 || !query_helper_accepts_arg(builtin, &arg_typed[0].ty) {
                let expected = match builtin {
                    Builtin::QueryExecuteNorito => {
                        "query_execute_norito expects (bytes) pointer to NoritoBytes QueryRequest"
                    }
                    Builtin::QueryGetParameter => "query_get_parameter expects (Name|bytes)",
                    Builtin::QueryGetContractManifest => {
                        "query_get_contract_manifest expects (bytes) Norito Hash"
                    }
                    Builtin::QueryGetContractInstance => {
                        "query_get_contract_instance expects (Name|bytes)"
                    }
                    Builtin::ZkRootsGet => {
                        "zk_roots_get expects (bytes) pointer to NoritoBytes RootsGetRequest"
                    }
                    Builtin::ZkVoteGetTally => {
                        "zk_vote_get_tally expects (bytes) pointer to NoritoBytes VoteGetTallyRequest"
                    }
                    Builtin::VrfEpochSeed => {
                        "vrf_epoch_seed expects (bytes) pointer to NoritoBytes VrfEpochSeedRequest"
                    }
                    _ => unreachable!(),
                };
                return Err(SemanticError {
                    code: "K2003",
                    message: expected.into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::BuildSubmitBallotInline => {
            if arg_typed.len() != 6
                || !(arg_typed[0].ty == Type::String
                    && is_blob_like(&arg_typed[1].ty)
                    && is_blob_like(&arg_typed[2].ty)
                    && arg_typed[3].ty == Type::String
                    && is_blob_like(&arg_typed[4].ty)
                    && is_blob_like(&arg_typed[5].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "build_submit_ballot_inline expects (string election_id, bytes ciphertext, bytes nullifier32, string backend, bytes proof, bytes vk)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::BuildUnshieldInline => {
            let valid_without_outputs = arg_typed.len() == 7
                && arg_typed[0].ty == Type::AssetDefinitionId
                && arg_typed[1].ty == Type::AccountId
                && arg_typed[2].ty == Type::Quantity
                && is_blob_like(&arg_typed[3].ty)
                && arg_typed[4].ty == Type::String
                && is_blob_like(&arg_typed[5].ty)
                && is_blob_like(&arg_typed[6].ty);
            let valid_with_outputs = arg_typed.len() == 8
                && arg_typed[0].ty == Type::AssetDefinitionId
                && arg_typed[1].ty == Type::AccountId
                && arg_typed[2].ty == Type::Quantity
                && is_blob_like(&arg_typed[3].ty)
                && is_blob_like(&arg_typed[4].ty)
                && arg_typed[5].ty == Type::String
                && is_blob_like(&arg_typed[6].ty)
                && is_blob_like(&arg_typed[7].ty);
            if !(valid_without_outputs || valid_with_outputs) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "build_unshield_inline expects (AssetDefinitionId, AccountId, quantity amount, bytes inputs32, [bytes outputs32,] string backend, bytes proof, bytes vk)".into(),
                });
            }
            if let ExprKind::DecimalLiteral { value: amount, .. } = arg_typed[2].kind() {
                if amount.scale() != 0 {
                    return Err(SemanticError {
                        code: "E_UNSHIELD_AMOUNT_RANGE",
                        message:
                            "crypto::zk::build_unshield requires a whole quantity with scale 0"
                                .into(),
                    });
                }
                if amount.try_mantissa_u128().is_none() {
                    return Err(SemanticError {
                        code: "E_UNSHIELD_AMOUNT_RANGE",
                        message:
                            "crypto::zk::build_unshield quantity exceeds the u128 V1 proof-scalar range"
                                .into(),
                    });
                }
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::RecordSccpMessage
        | Builtin::ScExecuteSubmitBallot
        | Builtin::ScExecuteUnshield => {
            let name = builtin.name();
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{name} expects (bytes) where the argument is a pointer to NoritoBytes TLV in INPUT"
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::ExecuteQuery => {
            let name = builtin.name();
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{name} expects (bytes) where the argument is a pointer to NoritoBytes TLV in INPUT"
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::ResolveAccountAlias => {
            if arg_typed.len() != 1
                || !(arg_typed[0].ty == Type::String || is_blob_like(&arg_typed[0].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "resolve_account_alias expects (string|bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::AccountId,
            })
        }
        Builtin::SubscriptionBill | Builtin::SubscriptionRecordUsage => {
            let name = builtin.name();
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects no arguments"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::ZkVerifyTransfer
        | Builtin::ZkVerifyUnshield
        | Builtin::ZkVerifyBatch
        | Builtin::ZkVoteVerifyBallot
        | Builtin::ZkVoteVerifyTally => {
            let name = builtin.name();
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{name} expects (bytes) where the argument is a pointer to NoritoBytes TLV in INPUT"
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::VrfVerify => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "vrf_verify expects one bytes-encoded VrfVerifyRequest".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::VrfVerifyBatch => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "vrf_verify_batch expects (bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::Sm3Hash
        | Builtin::Sha256Hash
        | Builtin::Sha3Hash
        | Builtin::Blake2b256Hash
        | Builtin::Keccak256Hash
        | Builtin::IrohaHash => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects (bytes) argument pointing to INPUT TLV",
                        builtin.name()
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::Sm2Verify => {
            if arg_typed.len() != 3 && arg_typed.len() != 4 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "sm2_verify expects (bytes, bytes, bytes) or (bytes, bytes, bytes, bytes) where arguments reference INPUT TLVs".into(),
                });
            }
            if arg_typed[..3].iter().any(|t| !is_blob_like(&t.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "sm2_verify expects message, signature, and public key as bytes pointers"
                            .into(),
                });
            }
            if arg_typed.len() == 4 && !is_blob_like(&arg_typed[3].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "sm2_verify optional distid must be provided as bytes pointer".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bool,
            })
        }
        Builtin::VerifySignature => {
            if arg_typed.len() != 4 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "verify_signature expects (bytes, bytes, bytes, int) arguments".into(),
                });
            }
            if arg_typed[..3].iter().any(|t| !is_blob_like(&t.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "verify_signature expects message, signature, and public key as bytes pointers"
                        .into(),
                });
            }
            if !is_int_like(&arg_typed[3].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "verify_signature expects scheme code as int".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bool,
            })
        }
        Builtin::Sm4GcmSeal | Builtin::Sm4GcmOpen => {
            if arg_typed.len() != 4 || arg_typed.iter().any(|t| !is_blob_like(&t.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (bytes, bytes, bytes, bytes)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::Sm4CcmSeal | Builtin::Sm4CcmOpen => {
            if arg_typed.len() != 4 && arg_typed.len() != 5 {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects (bytes, bytes, bytes, bytes[, int])",
                        builtin.name()
                    ),
                });
            }
            if arg_typed[..4].iter().any(|t| !is_blob_like(&t.ty)) {
                let data_label = match builtin {
                    Builtin::Sm4CcmSeal => "plaintext",
                    Builtin::Sm4CcmOpen => "ciphertext",
                    _ => unreachable!(),
                };
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects key, nonce, aad, {data_label} as bytes pointers",
                        builtin.name()
                    ),
                });
            }
            if arg_typed.len() == 5 && !is_int_like(&arg_typed[4].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} optional tag length must be int", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::GetAccountBalance => {
            if arg_typed.len() != 2
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::AssetDefinitionId)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "get_account_balance expects (AccountId, AssetDefinitionId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Quantity,
            })
        }
        Builtin::GetPublicInput => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "get_public_input expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::DebugPrint => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "debug_print expects (int value)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::DebugLog => {
            if arg_typed.len() != 1
                || !(arg_typed[0].ty == Type::Json || is_blob_like(&arg_typed[0].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "debug_log expects (Json|bytes payload)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::Assert => {
            let ok = match arg_typed.len() {
                1 => arg_typed[0].ty == Type::Bool,
                2 => {
                    arg_typed[0].ty == Type::Bool
                        && (arg_typed[1].ty == Type::String || is_int_like(&arg_typed[1].ty))
                }
                _ => false,
            };
            if !ok {
                return Err(SemanticError {
                    code: "K2003",
                    message: "assert expects (bool) or (bool, string|int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::Require => {
            if arg_typed.len() != 2 || arg_typed[0].ty != Type::Bool || arg_typed[1].ty != Type::Int
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "require expects (bool, ErrorEnum::Variant)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::Info => {
            if arg_typed.len() != 1
                || !(arg_typed[0].ty == Type::String || is_int_like(&arg_typed[0].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "info expects (string|int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AssertEq => {
            if arg_typed.len() != 2 || !arg_typed.iter().all(|t| is_int_like(&t.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "assert_eq expects two int args".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SetAccountDetail => {
            if arg_typed.len() != 3
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::Name
                    && arg_typed[2].ty == Type::Json)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "set_account_detail expects (AccountId, Name, Json)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::MintAsset | Builtin::BurnAsset => {
            if arg_typed.len() != 3
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::AssetDefinitionId
                    && arg_typed[2].ty == Type::Quantity)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects (AccountId, AssetDefinitionId, quantity)",
                        builtin.name()
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::TransferAsset => {
            if arg_typed.len() != 5
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::AccountId
                    && arg_typed[2].ty == Type::AssetDefinitionId
                    && arg_typed[3].ty == Type::Quantity
                    && arg_typed[4].ty == Type::DataSpaceId)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "transfer_asset expects (AccountId, AccountId, AssetDefinitionId, quantity, DataSpaceId)"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SetAssetTransferAvailability => {
            if arg_typed.len() != 6
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::AssetDefinitionId
                    && arg_typed[2].ty == Type::Int
                    && arg_typed[3].ty == Type::Bool
                    && arg_typed[4].ty == Type::Bool
                    && resolve_struct_type(&arg_typed[5].ty)
                        == Type::Option(Box::new(Type::String)))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "ledger::asset::set_transfer_availability expects (AccountId, AssetDefinitionId, int, bool, bool, Option<string>)"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SetAssetTransferDailyLimit => {
            if arg_typed.len() != 3
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::AssetDefinitionId
                    && resolve_struct_type(&arg_typed[2].ty)
                        == Type::Option(Box::new(Type::Quantity)))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "ledger::asset::set_transfer_daily_limit expects (AccountId, AssetDefinitionId, Option<quantity>)"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SetAssetHoldingLimit => {
            if arg_typed.len() != 3
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::AssetDefinitionId
                    && resolve_struct_type(&arg_typed[2].ty)
                        == Type::Option(Box::new(Type::Quantity)))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "ledger::asset::set_holding_limit expects (AccountId, AssetDefinitionId, Option<quantity>)"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AccountRecoveryPropose => {
            if arg_typed.len() != 2
                || !(arg_typed[0].ty == Type::String && arg_typed[1].ty == Type::AccountId)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "ledger::account::recovery::propose expects (string, AccountId)"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AccountRecoveryApprove
        | Builtin::AccountRecoveryCancel
        | Builtin::AccountRecoveryFinalize => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::String {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (string)", builtin.source_name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::NftMintAsset => {
            if arg_typed.len() != 2
                || !(arg_typed[0].ty == Type::NftId && arg_typed[1].ty == Type::AccountId)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "nft_mint_asset expects (NftId, AccountId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::NftSetMetadata => {
            if arg_typed.len() != 3
                || !(arg_typed[0].ty == Type::NftId
                    && arg_typed[1].ty == Type::Name
                    && arg_typed[2].ty == Type::Json)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "nft_set_metadata expects (NftId, Name, Json)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::NftBurnAsset => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::NftId {
                return Err(SemanticError {
                    code: "K2003",
                    message: "nft_burn_asset expects (NftId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::NftTransferAsset => {
            if arg_typed.len() != 3
                || !(arg_typed[0].ty == Type::AccountId
                    && arg_typed[1].ty == Type::NftId
                    && arg_typed[2].ty == Type::AccountId)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "nft_transfer_asset expects (AccountId, NftId, AccountId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::RegisterDomain | Builtin::UnregisterDomain => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::DomainId {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (DomainId)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::TransferDomain => {
            if arg_typed.len() != 3
                || arg_typed[0].ty != Type::AccountId
                || !matches!(arg_typed[1].ty, Type::DomainId | Type::Name)
                || arg_typed[2].ty != Type::AccountId
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "transfer_domain expects (AccountId, DomainId, AccountId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::RegisterAccount | Builtin::UnregisterAccount => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::AccountId {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (AccountId)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::RegisterAsset => {
            if arg_typed.len() != 4
                || arg_typed[0].ty != Type::AssetDefinitionId
                || arg_typed[1].ty != Type::String
                || !is_int_like(&arg_typed[2].ty)
                || !is_int_like(&arg_typed[3].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "register_asset expects (AssetDefinitionId, string, int, int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::CreateNewAsset => {
            if arg_typed.len() != 5
                || arg_typed[0].ty != Type::AssetDefinitionId
                || arg_typed[1].ty != Type::String
                || !is_int_like(&arg_typed[2].ty)
                || arg_typed[3].ty != Type::AccountId
                || !is_int_like(&arg_typed[4].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "create_new_asset expects (AssetDefinitionId, string, int, AccountId, int)"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::UnregisterAsset => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::AssetDefinitionId {
                return Err(SemanticError {
                    code: "K2003",
                    message: "unregister_asset expects (AssetDefinitionId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::RegisterPeer | Builtin::UnregisterPeer => {
            let name = builtin.name();
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Json {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (Json)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::CreateTrigger | Builtin::RegisterTrigger => {
            let name = builtin.name();
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Json {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (Json)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::RemoveTrigger | Builtin::UnregisterTrigger => {
            let name = builtin.name();
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (Name)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SetTriggerEnabled => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Name
                || !is_int_like(&arg_typed[1].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "set_trigger_enabled expects (Name, int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::CreateRole => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Name
                || arg_typed[1].ty != Type::Json
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "create_role expects (Name, Json)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::DeleteRole => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "delete_role expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::GrantRole | Builtin::RevokeRole => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::AccountId
                || arg_typed[1].ty != Type::Name
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (AccountId, Name)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::GrantPermission | Builtin::RevokePermission => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::AccountId
                || !(arg_typed[1].ty == Type::Name || arg_typed[1].ty == Type::Json)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (AccountId, Name|Json)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::GrantContractEntrypoint | Builtin::RevokeContractEntrypoint => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::AccountId
                || arg_typed[1].ty != Type::String
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (AccountId, string)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::EscrowOpenOffer => {
            if !(arg_typed.len() == 3 || arg_typed.len() == 4)
                || !(arg_typed[0].ty == Type::Name
                    && arg_typed[1].ty == Type::AssetDefinitionId
                    && arg_typed[2].ty == Type::Quantity)
                || (arg_typed.len() == 4 && !is_blob_like(&arg_typed[3].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "escrow_open_offer expects (Name, AssetDefinitionId, quantity[, bytes evidence_hashes])"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::EscrowAccept
        | Builtin::EscrowMarkPaymentSent
        | Builtin::EscrowRelease
        | Builtin::EscrowCancel => {
            let name = builtin.name();
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (Name)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::EscrowOpenDispute => {
            if !(arg_typed.len() == 1 || arg_typed.len() == 2)
                || arg_typed[0].ty != Type::Name
                || (arg_typed.len() == 2 && !is_blob_like(&arg_typed[1].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "escrow_open_dispute expects (Name[, bytes evidence_hashes])".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::EscrowResolveDispute => {
            if !(arg_typed.len() == 3 || arg_typed.len() == 4)
                || !(arg_typed[0].ty == Type::Name
                    && arg_typed[1].ty == Type::Quantity
                    && arg_typed[2].ty == Type::Quantity)
                || (arg_typed.len() == 4 && !is_blob_like(&arg_typed[3].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "escrow_resolve_dispute expects (Name, quantity, quantity[, bytes evidence_hashes])"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AnonymousEscrowOpenOffer
        | Builtin::AnonymousEscrowRelease
        | Builtin::AnonymousEscrowCancel
        | Builtin::AnonymousEscrowResolveDispute => {
            let name = builtin.name();
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (bytes) Norito request payload"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AnonymousEscrowAccept | Builtin::AnonymousEscrowMarkPaymentSent => {
            let name = builtin.name();
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (Name)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AnonymousEscrowOpenDispute => {
            if !(arg_typed.len() == 1 || arg_typed.len() == 2)
                || arg_typed[0].ty != Type::Name
                || (arg_typed.len() == 2 && !is_blob_like(&arg_typed[1].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "anonymous_escrow_open_dispute expects (Name[, bytes evidence_hashes])"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::Alloc => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "alloc expects (int bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::ProveExecution => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: "prove_execution expects no arguments".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::GrowHeap => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "grow_heap expects (int bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::VerifyProof => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "verify_proof expects (bytes) pointer to NoritoBytes OpenVerifyEnvelope"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bool,
            })
        }
        Builtin::GetMerklePath => {
            let valid_arity = (2..=3).contains(&arg_typed.len());
            if !valid_arity || arg_typed.iter().any(|arg| !is_int_like(&arg.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "get_merkle_path expects (int address, int output_ptr[, int root_output_ptr])"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::GetMerkleCompact | Builtin::GetRegisterMerkleCompact => {
            let valid_arity = (2..=4).contains(&arg_typed.len());
            if !valid_arity || arg_typed.iter().any(|arg| !is_int_like(&arg.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects (int address_or_register, int output_ptr[, int max_depth[, int root_output_ptr]])",
                        builtin.name()
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::GetPrivateInput => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (int index)", builtin.source_name()),
                });
            }
            if !context.zk_enabled {
                return Err(SemanticError {
                    code: "E_ZK_MODE_REQUIRED",
                    message: format!(
                        "{} requires ZK mode in compiler build configuration",
                        builtin.source_name()
                    ),
                });
            }
            let payload = match expected.map(resolve_struct_type) {
                Some(Type::Secret(payload))
                    if matches!(payload.as_ref(), Type::Int | Type::Decimal | Type::Quantity) =>
                {
                    *payload
                }
                Some(other) => {
                    return Err(SemanticError {
                        code: "E_SECRET_PRIVATE_INPUT_CONTEXT",
                        message: format!(
                            "{} must initialize an explicitly declared Secret<int>, Secret<decimal>, or Secret<quantity>; found `{}`",
                            builtin.source_name(),
                            type_name(&other)
                        ),
                    });
                }
                None => {
                    return Err(SemanticError {
                        code: "E_SECRET_PRIVATE_INPUT_AMBIGUOUS",
                        message: format!(
                            "{} has no inferable payload type; use a type-first declaration such as `let Secret<int> value = {}(0)`",
                            builtin.source_name(),
                            builtin.source_name()
                        ),
                    });
                }
            };
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Secret(Box::new(payload)),
            })
        }
        Builtin::CommitOutput => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: "commit_output expects no arguments".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::CreateNftsForAllUsers => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: "create_nfts_for_all_users expects no arguments".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SetExecutionDepth => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "set_execution_depth expects one int arg".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::TransferV1BatchBegin | Builtin::TransferV1BatchEnd => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects ()", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::TransferV1BatchApply => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "transfer_v1_batch_apply expects (bytes) Norito TransferAssetBatch"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::TransferBatch => {
            ensure_transfer_batch_args(&mut arg_typed)?;
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AxtBegin => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::AxtDescriptor {
                return Err(SemanticError {
                    code: "K2003",
                    message: "axt_begin expects (AxtDescriptor)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AxtTouch => {
            if arg_typed.is_empty()
                || arg_typed.len() > 2
                || arg_typed[0].ty != Type::DataSpaceId
                || (arg_typed.len() == 2 && !is_blob_like(&arg_typed[1].ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "axt_touch expects (DataSpaceId[, bytes manifest])".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::VerifyDsProof => {
            if arg_typed.is_empty()
                || arg_typed.len() > 2
                || arg_typed[0].ty != Type::DataSpaceId
                || (arg_typed.len() == 2 && arg_typed[1].ty != Type::ProofBlob)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "verify_ds_proof expects (DataSpaceId[, ProofBlob])".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::UseAssetHandle => {
            if arg_typed.len() != 2 && arg_typed.len() != 3 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "use_asset_handle expects (AssetHandle, bytes intent[, ProofBlob])"
                        .into(),
                });
            }
            if arg_typed[0].ty != Type::AssetHandle || !is_blob_like(&arg_typed[1].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "use_asset_handle expects (AssetHandle, bytes intent[, ProofBlob])"
                        .into(),
                });
            }
            if arg_typed.len() == 3 && arg_typed[2].ty != Type::ProofBlob {
                return Err(SemanticError {
                    code: "K2003",
                    message: "use_asset_handle expects (AssetHandle, bytes intent[, ProofBlob])"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::AxtCommit => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: "axt_commit expects no arguments".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::DeactivateContractInstance
        | Builtin::RemoveSmartContractBytes
        | Builtin::RegisterSmartContractCode
        | Builtin::RegisterSmartContractBytes
        | Builtin::ActivateContractInstance => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects (bytes) pointer to NoritoBytes lifecycle request",
                        builtin.name()
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SoracloudReadCommittedState
        | Builtin::SoracloudEmitStateMutation
        | Builtin::SoracloudEmitMailboxMessage
        | Builtin::SoracloudAppendJournal
        | Builtin::SoracloudPublishCheckpoint
        | Builtin::SoracloudReadSecret
        | Builtin::SoracloudReadCredential
        | Builtin::SoracloudEgressFetch
        | Builtin::SoracloudReadConfig
        | Builtin::SoracloudReadSecretEnvelope => {
            if arg_typed.len() != 1
                || resolve_struct_type(&arg_typed[0].ty) != Type::SoracloudRequest
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (SoracloudRequest)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::SoracloudResponse,
            })
        }
        Builtin::AddSignatory | Builtin::RemoveSignatory => {
            if arg_typed.len() != 2
                || !(arg_typed[0].ty == Type::AccountId && arg_typed[1].ty == Type::Json)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (AccountId, Json)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::SetAccountQuorum => {
            if arg_typed.len() != 2
                || !(arg_typed[0].ty == Type::AccountId && arg_typed[1].ty == Type::Int)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "set_account_quorum expects (AccountId, int)".into(),
                });
            }
            if literal_int(&arg_typed[1]).is_some_and(|quorum| {
                quorum
                    .try_to_u64()
                    .is_none_or(|quorum| !(1..=u64::from(u16::MAX)).contains(&quorum))
            }) {
                return Err(SemanticError {
                    code: "E_QUORUM_RANGE",
                    message: "account quorum must be in the protocol range 1..=65535".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::Path => {
            if arg_typed.len() != 2 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "path expects (Name, int|bytes)".into(),
                });
            }
            if !(is_int_like(&arg_typed[1].ty) || is_blob_like(&arg_typed[1].ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "path expects (Name, int|bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Name,
            })
        }
        Builtin::NameDecode => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "name_decode expects (bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Name,
            })
        }
        Builtin::TlvEq => {
            if arg_typed.len() != 2 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "tlv_eq expects (pointer-ABI, pointer-ABI)".into(),
                });
            }
            for arg in &arg_typed {
                let ty = resolve_struct_type(&arg.ty);
                if !(is_pointer_type(&ty) || is_blob_like(&ty) || ty == Type::Json) {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "tlv_eq expects (pointer-ABI, pointer-ABI)".into(),
                    });
                }
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bool,
            })
        }
        Builtin::TlvLen => {
            if arg_typed.len() != 1 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "tlv_len expects one argument".into(),
                });
            }
            let ty = resolve_struct_type(&arg_typed[0].ty);
            if !(is_pointer_type(&ty) || is_blob_like(&ty) || ty == Type::Json) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "tlv_len expects a pointer-ABI type, Json, or bytes argument".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::BytesLen => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "bytes::len expects exactly one bytes argument".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::PointerToNorito => {
            if arg_typed.len() != 1 {
                return Err(SemanticError {
                    code: "K2003",
                    message: "pointer_to_norito expects one argument".into(),
                });
            }
            let ty = resolve_struct_type(&arg_typed[0].ty);
            if !(is_pointer_type(&ty) || is_blob_like(&ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "pointer_to_norito expects a pointer-ABI type or bytes argument"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::JsonObject => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: "json_object expects no arguments".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::JsonSetInt => {
            if arg_typed.len() != 3
                || arg_typed[0].ty != Type::Json
                || arg_typed[1].ty != Type::Name
                || !is_int_like(&arg_typed[2].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "json_set_int expects (Json, Name, int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::JsonSetAccountId => {
            if arg_typed.len() != 3
                || arg_typed[0].ty != Type::Json
                || arg_typed[1].ty != Type::Name
                || arg_typed[2].ty != Type::AccountId
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "json_set_account_id expects (Json, Name, AccountId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::EncodeInt => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "encode_int expects (int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::DecodeInt => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "decode_int expects (bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::EncodeJson => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Json {
                return Err(SemanticError {
                    code: "K2003",
                    message: "encode_json expects (Json)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::DecodeJson => {
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "decode_json expects (bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::JsonSetIntDirect => {
            if arg_typed.len() != 3
                || arg_typed[0].ty != Type::Json
                || arg_typed[1].ty != Type::Name
                || !is_int_like(&arg_typed[2].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "json_set_int_direct expects (Json, Name, int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::JsonSetAccountIdDirect => {
            if arg_typed.len() != 3
                || arg_typed[0].ty != Type::Json
                || arg_typed[1].ty != Type::Name
                || arg_typed[2].ty != Type::AccountId
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "json_set_account_id_direct expects (Json, Name, AccountId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::JsonGetIntDirect
        | Builtin::JsonGetDecimalDirect
        | Builtin::JsonGetQuantityDirect
        | Builtin::JsonGetJsonDirect
        | Builtin::JsonGetNameDirect
        | Builtin::JsonGetAccountIdDirect
        | Builtin::JsonGetAssetDefinitionIdDirect
        | Builtin::JsonGetNftIdDirect
        | Builtin::JsonGetBlobHexDirect => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Json
                || arg_typed[1].ty != Type::Name
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (Json, Name)", builtin.name()),
                });
            }
            let ty = direct_json_getter_type(builtin).expect("direct JSON getter type");
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty,
            })
        }
        Builtin::BuildPathKeyNoritoDirect => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Name
                || !is_blob_like(&arg_typed[1].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "build_path_key_norito_direct expects (Name, bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Name,
            })
        }
        Builtin::SchemaEncode => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Name
                || arg_typed[1].ty != Type::Json
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "encode_schema expects (Name, Json)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::SchemaDecode => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Name
                || !is_blob_like(&arg_typed[1].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "decode_schema expects (Name, bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::SchemaInfo => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "schema_info expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::SchemaEncodeDirect => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Name
                || arg_typed[1].ty != Type::Json
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "encode_schema_direct expects (Name, Json)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::SchemaDecodeDirect => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Name
                || !is_blob_like(&arg_typed[1].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "decode_schema_direct expects (Name, bytes)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::SchemaInfoDirect => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
                    code: "K2003",
                    message: "schema_info_direct expects (Name)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::NumericToInt => {
            if arg_typed.len() != 1 || !is_wide_numeric_type(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "numeric_to_int expects (quantity|int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::NumericNeg => {
            if arg_typed.len() != 1 || !is_wide_numeric_type(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "numeric_neg expects (quantity|int)".into(),
                });
            }
            Err(SemanticError {
                code: "E_QUANTITY_NEGATION",
                message: "numeric::neg is not defined for int or quantity values".into(),
            })
        }
        Builtin::NumericToIntDirect => {
            if arg_typed.len() != 1 || !matches!(resolve_struct_type(&arg_typed[0].ty), Type::Int) {
                return Err(SemanticError {
                    code: "K2003",
                    message:
                        "numeric_to_int_direct expects (int); quantity uses its nominal V1 syscall"
                            .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::NumericNegDirect => {
            if arg_typed.len() != 1 || !is_wide_numeric_type(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "numeric_neg_direct expects (quantity|int)".into(),
                });
            }
            Err(SemanticError {
                code: "E_QUANTITY_NEGATION",
                message: "numeric negation is not defined for int or quantity values".into(),
            })
        }
        Builtin::NumericAdd
        | Builtin::NumericSub
        | Builtin::NumericMul
        | Builtin::NumericDiv
        | Builtin::NumericRem
        | Builtin::NumericAddDirect
        | Builtin::NumericSubDirect
        | Builtin::NumericMulDirect
        | Builtin::NumericDivDirect
        | Builtin::NumericRemDirect => {
            if arg_typed.len() != 2
                || !is_wide_numeric_type(&arg_typed[0].ty)
                || !is_wide_numeric_type(&arg_typed[1].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (quantity|int, quantity|int)", builtin.name()),
                });
            }
            let Some(result_ty) = numeric_result_type(&arg_typed[0].ty, &arg_typed[1].ty) else {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects compatible wide numeric operands",
                        builtin.name()
                    ),
                });
            };
            if !is_wide_numeric_type(&result_ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects wide numeric operands", builtin.name()),
                });
            }
            if matches!(resolve_struct_type(&result_ty), Type::Quantity)
                && matches!(builtin, Builtin::NumericRem | Builtin::NumericRemDirect)
            {
                return Err(SemanticError {
                    code: "E_QUANTITY_REMAINDER",
                    message: "quantity does not support `%`; use exact `/` or quantity.div_round"
                        .into(),
                });
            }
            if matches!(resolve_struct_type(&result_ty), Type::Quantity)
                && matches!(
                    builtin,
                    Builtin::NumericAddDirect
                        | Builtin::NumericSubDirect
                        | Builtin::NumericMulDirect
                        | Builtin::NumericDivDirect
                        | Builtin::NumericRemDirect
                )
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "direct Numeric helpers only accept int; quantity uses its nominal V1 syscalls"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: result_ty,
            })
        }
        Builtin::NumericEq
        | Builtin::NumericNe
        | Builtin::NumericLt
        | Builtin::NumericLe
        | Builtin::NumericGt
        | Builtin::NumericGe
        | Builtin::NumericEqDirect
        | Builtin::NumericNeDirect
        | Builtin::NumericLtDirect
        | Builtin::NumericLeDirect
        | Builtin::NumericGtDirect
        | Builtin::NumericGeDirect => {
            if arg_typed.len() != 2
                || !is_wide_numeric_type(&arg_typed[0].ty)
                || !is_wide_numeric_type(&arg_typed[1].ty)
                || numeric_result_type(&arg_typed[0].ty, &arg_typed[1].ty).is_none()
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects compatible wide numeric operands",
                        builtin.name()
                    ),
                });
            }
            if matches!(resolve_struct_type(&arg_typed[0].ty), Type::Quantity)
                && matches!(
                    builtin,
                    Builtin::NumericEqDirect
                        | Builtin::NumericNeDirect
                        | Builtin::NumericLtDirect
                        | Builtin::NumericLeDirect
                        | Builtin::NumericGtDirect
                        | Builtin::NumericGeDirect
                )
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: "direct Numeric comparisons only accept int; quantity uses its nominal V1 syscalls"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bool,
            })
        }
        Builtin::WrappingNeg => {
            if arg_typed.len() != 1 || !matches!(resolve_struct_type(&arg_typed[0].ty), Type::Int) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "wrapping_neg expects (int)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::WrappingAdd | Builtin::WrappingSub | Builtin::WrappingMul => {
            if arg_typed.len() != 2
                || arg_typed
                    .iter()
                    .any(|argument| !matches!(resolve_struct_type(&argument.ty), Type::Int))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (int, int)", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::Isqrt | Builtin::Abs => {
            let name = builtin.name();
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (int)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::Min | Builtin::Max | Builtin::DivCeil | Builtin::Gcd | Builtin::Mean => {
            let name = builtin.name();
            if arg_typed.len() != 2
                || !is_int_like(&arg_typed[0].ty)
                || !is_int_like(&arg_typed[1].ty)
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{name} expects (int, int)"),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::Poseidon2 => {
            if arg_typed.len() != 2 || !arg_typed.iter().all(|arg| is_int_like(&arg.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects two int arguments", builtin.source_name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::Valcom => {
            if arg_typed.len() != 2
                || !arg_typed
                    .iter()
                    .all(|arg| crate::secret::is_secret_numeric(&arg.ty))
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!(
                        "{} expects two typed Secret<int|decimal|quantity> arguments",
                        builtin.source_name()
                    ),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::Poseidon6 => {
            if arg_typed.len() != 6 || !arg_typed.iter().all(|arg| is_int_like(&arg.ty)) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects six int args", builtin.source_name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::Pubkgen => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects one int arg", builtin.source_name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::SetVl => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "setvl expects one int arg".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Unit,
            })
        }
        Builtin::GetInt
        | Builtin::GetDecimal
        | Builtin::GetQuantity
        | Builtin::GetJson
        | Builtin::GetName
        | Builtin::GetAccountId
        | Builtin::GetAssetDefinitionId
        | Builtin::GetNftId
        | Builtin::GetBlobHex => {
            if arg_typed.len() != 2
                || arg_typed[0].ty != Type::Json
                || arg_typed[1].ty != Type::Name
            {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects (Json, Name)", builtin.name()),
                });
            }
            let payload = match builtin {
                Builtin::GetInt => Type::Int,
                Builtin::GetDecimal => Type::Decimal,
                Builtin::GetQuantity => Type::Quantity,
                Builtin::GetJson => Type::Json,
                Builtin::GetName => Type::Name,
                Builtin::GetAccountId => Type::AccountId,
                Builtin::GetAssetDefinitionId => Type::AssetDefinitionId,
                Builtin::GetNftId => Type::NftId,
                Builtin::GetBlobHex => Type::Bytes,
                _ => unreachable!(),
            };
            let ty = Type::Option(Box::new(payload));
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty,
            })
        }
        Builtin::TriggerEvent => {
            reject_public_trigger_event(context, builtin.name())?;
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: "trigger_event expects no arguments".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Json,
            })
        }
        Builtin::Authority | Builtin::SysvarAuthority | Builtin::ContractSubject => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects no arguments", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::AccountId,
            })
        }
        Builtin::CurrentTimeMs | Builtin::BlockHeight | Builtin::BlockTimeMs => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects no arguments", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Int,
            })
        }
        Builtin::ChainId | Builtin::ContractAddress | Builtin::Entrypoint => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
                    code: "K2003",
                    message: format!("{} expects no arguments", builtin.name()),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Bytes,
            })
        }
        Builtin::TestInvokeEntrypoint
        | Builtin::TestInvokeEntrypointAs
        | Builtin::TestExpectRejectAs
        | Builtin::TestActorAccount
        | Builtin::TestActorPublicKey
        | Builtin::TestActorSign => {
            unreachable!("test helpers are validated before generic builtin analysis")
        }
    }
}

fn enclosing_return_type(context: &SemanticContext) -> Option<Type> {
    context
        .current_function_name
        .borrow()
        .as_ref()
        .and_then(|name| context.function_returns.borrow().get(name).cloned())
}

fn typed_block_value_type(block: &TypedBlock) -> Type {
    block
        .tail
        .as_ref()
        .map_or(Type::Unit, |expression| expression.ty.clone())
}

/// Return whether evaluating `block` can never reach its enclosing expression
/// continuation.
///
/// Divergent expression branches behave like a bottom type: they do not need
/// to synthesize a placeholder value merely to agree with a sibling branch.
/// Keeping divergence as a control-flow property instead of a public V1 type
/// also prevents it from leaking into entrypoint schemas or the pointer ABI.
pub(crate) fn typed_block_diverges(block: &TypedBlock) -> bool {
    block.statements.iter().any(typed_statement_diverges)
        || block.tail.as_deref().is_some_and(typed_expression_diverges)
}

fn typed_statement_diverges(statement: &TypedStatement) -> bool {
    match statement {
        TypedStatement::Return(_) | TypedStatement::Break | TypedStatement::Continue => true,
        TypedStatement::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        }
        | TypedStatement::IfLet {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => typed_block_diverges(then_branch) && typed_block_diverges(else_branch),
        TypedStatement::Let { value, .. } | TypedStatement::Expr(value) => {
            typed_expression_diverges(value)
        }
        TypedStatement::If {
            else_branch: None, ..
        }
        | TypedStatement::IfLet {
            else_branch: None, ..
        }
        | TypedStatement::While { .. }
        | TypedStatement::For { .. }
        | TypedStatement::ForEachMap { .. }
        | TypedStatement::MapSet { .. } => false,
    }
}

fn typed_expression_diverges(expression: &TypedExpr) -> bool {
    match expression.kind() {
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        }
        | ExprKind::IfLet {
            then_branch,
            else_branch,
            ..
        } => typed_block_diverges(then_branch) && typed_block_diverges(else_branch),
        ExprKind::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|arm| typed_block_diverges(&arm.body))
        }
        _ => false,
    }
}

fn analyze_expression_block(
    context: &SemanticContext,
    block: &Block,
    vars: &mut HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<TypedBlock, SemanticError> {
    let return_type = enclosing_return_type(context);
    let mut mutable_bindings = context.current_mutable_bindings.borrow().clone();
    analyze_block(
        context,
        block,
        vars,
        &mut mutable_bindings,
        return_type.as_ref(),
        expected,
        0,
    )
}

fn require_exact_branch_type(expected: &Type, actual: &Type) -> Result<(), SemanticError> {
    if resolve_struct_type(expected) == resolve_struct_type(actual) {
        Ok(())
    } else {
        Err(SemanticError {
            code: "E_BRANCH_TYPE_MISMATCH",
            message: format!(
                "expression branches must have exactly the same type; expected `{}`, found `{}`",
                type_name(expected),
                type_name(actual)
            ),
        })
    }
}

fn normalize_branch_error(error: SemanticError, expected: &Type) -> SemanticError {
    if matches!(
        error.code,
        "E_TAIL_TYPE_MISMATCH" | "E_TYPE_ANNOTATION_MISMATCH"
    ) {
        SemanticError {
            code: "E_BRANCH_TYPE_MISMATCH",
            message: format!(
                "expression branch must have type `{}`: {}",
                type_name(expected),
                error.message
            ),
        }
    } else {
        error
    }
}

fn analyze_expression_branches(
    context: &SemanticContext,
    then_branch: &Block,
    else_branch: &Block,
    vars: &HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<(TypedBlock, TypedBlock, Type), SemanticError> {
    analyze_expression_branches_with_envs(
        context,
        then_branch,
        else_branch,
        &mut vars.clone(),
        &mut vars.clone(),
        expected,
    )
}

fn analyze_expression_branches_with_envs(
    context: &SemanticContext,
    then_branch: &Block,
    else_branch: &Block,
    then_vars: &mut HashMap<String, Type>,
    else_vars: &mut HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<(TypedBlock, TypedBlock, Type), SemanticError> {
    if let Some(expected) = expected {
        let then_typed = analyze_expression_block(context, then_branch, then_vars, Some(expected))
            .map_err(|error| normalize_branch_error(error, expected))?;
        let else_typed = analyze_expression_block(context, else_branch, else_vars, Some(expected))
            .map_err(|error| normalize_branch_error(error, expected))?;
        if !typed_block_diverges(&then_typed) {
            require_exact_branch_type(expected, &typed_block_value_type(&then_typed))?;
        }
        if !typed_block_diverges(&else_typed) {
            require_exact_branch_type(expected, &typed_block_value_type(&else_typed))?;
        }
        return Ok((then_typed, else_typed, expected.clone()));
    }

    match analyze_expression_block(context, then_branch, &mut then_vars.clone(), None) {
        Ok(then_typed) => {
            if typed_block_diverges(&then_typed) {
                let else_typed = analyze_expression_block(context, else_branch, else_vars, None)?;
                if typed_block_diverges(&else_typed) {
                    return Err(SemanticError {
                        code: "E_DIVERGING_EXPRESSION_CONTEXT",
                        message:
                            "an expression whose branches all return requires an exact type context"
                                .into(),
                    });
                }
                let ty = typed_block_value_type(&else_typed);
                return Ok((then_typed, else_typed, ty));
            }
            let ty = typed_block_value_type(&then_typed);
            let else_typed = analyze_expression_block(context, else_branch, else_vars, Some(&ty))?;
            if !typed_block_diverges(&else_typed) {
                require_exact_branch_type(&ty, &typed_block_value_type(&else_typed))?;
            }
            Ok((then_typed, else_typed, ty))
        }
        Err(error) if error.code == "E_SUM_MISSING_CONTEXT" => {
            context.discard_diagnostic();
            let else_typed = analyze_expression_block(context, else_branch, else_vars, None)?;
            if typed_block_diverges(&else_typed) {
                return Err(error);
            }
            let ty = typed_block_value_type(&else_typed);
            let then_typed = analyze_expression_block(context, then_branch, then_vars, Some(&ty))?;
            if !typed_block_diverges(&then_typed) {
                require_exact_branch_type(&ty, &typed_block_value_type(&then_typed))?;
            }
            Ok((then_typed, else_typed, ty))
        }
        Err(error) => Err(error),
    }
}

fn analyze_sum_pattern(
    pattern: &SumPattern,
    value_type: &Type,
) -> Result<(TypedSumPattern, Option<(String, Type)>), SemanticError> {
    let value_type = resolve_struct_type(value_type);
    let payload = match (&value_type, pattern.variant) {
        (Type::Option(payload), SumVariant::OptionSome) => Some(payload.as_ref().clone()),
        (Type::Option(_), SumVariant::OptionNone) => None,
        (Type::Result(payload, _), SumVariant::ResultOk) => Some(payload.as_ref().clone()),
        (Type::Result(_, error), SumVariant::ResultErr) => Some(error.as_ref().clone()),
        (Type::Option(_), _) => {
            return Err(SemanticError {
                code: "E_PATTERN_FAMILY",
                message: "Option values require `Option::some`/`Option::none` patterns".into(),
            });
        }
        (Type::Result(_, _), _) => {
            return Err(SemanticError {
                code: "E_PATTERN_FAMILY",
                message: "Result values require `Result::ok`/`Result::err` patterns".into(),
            });
        }
        (other, _) => {
            return Err(SemanticError {
                code: "E_PATTERN_TYPE",
                message: format!(
                    "sum patterns require Option or Result, found `{}`",
                    type_name(other)
                ),
            });
        }
    };
    let binding = match (&pattern.binding, &payload) {
        (Some(PatternBinding::Name(name)), Some(payload)) => Some((name.clone(), payload.clone())),
        (Some(PatternBinding::Wildcard), Some(_)) | (None, None) => None,
        (None, Some(_)) => {
            return Err(SemanticError {
                code: "E_PATTERN_PAYLOAD",
                message: "active sum patterns require a payload binding or `_`".into(),
            });
        }
        (Some(_), None) => {
            return Err(SemanticError {
                code: "E_PATTERN_PAYLOAD",
                message: "`Option::none` has no payload to bind".into(),
            });
        }
    };
    Ok((
        TypedSumPattern {
            pattern: pattern.clone(),
            payload_type: payload,
        },
        binding,
    ))
}

fn analyze_match_expression(
    context: &SemanticContext,
    value: TypedExpr,
    arms: &[MatchArm],
    vars: &HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    if arms.is_empty() {
        return Err(SemanticError {
            code: "E_MATCH_EMPTY",
            message: "match requires exhaustive arms".into(),
        });
    }
    let mut seen = HashSet::new();
    let mut checked = Vec::with_capacity(arms.len());
    for arm in arms {
        if !seen.insert(arm.pattern.variant) {
            return Err(SemanticError {
                code: "E_MATCH_DUPLICATE_PATTERN",
                message: format!(
                    "duplicate or unreachable `{:?}` match arm",
                    arm.pattern.variant
                ),
            });
        }
        let (pattern, binding) = analyze_sum_pattern(&arm.pattern, &value.ty)?;
        checked.push((arm, pattern, binding));
    }
    let exhaustive = match resolve_struct_type(&value.ty) {
        Type::Option(_) => {
            seen.contains(&SumVariant::OptionSome) && seen.contains(&SumVariant::OptionNone)
        }
        Type::Result(_, _) => {
            seen.contains(&SumVariant::ResultOk) && seen.contains(&SumVariant::ResultErr)
        }
        _ => false,
    };
    if !exhaustive {
        return Err(SemanticError {
            code: "E_MATCH_NON_EXHAUSTIVE",
            message: "match must cover both namespaced variants of its Option or Result value"
                .into(),
        });
    }

    let inferred = if let Some(expected) = expected {
        expected.clone()
    } else {
        let mut inferred = None;
        let mut missing_context = None;
        for (arm, _, binding) in &checked {
            let mut arm_vars = vars.clone();
            if let Some((name, ty)) = binding {
                arm_vars.insert(name.clone(), ty.clone());
            }
            match analyze_expression_block(context, &arm.body, &mut arm_vars, None) {
                Ok(block) => {
                    if !typed_block_diverges(&block) {
                        inferred = Some(typed_block_value_type(&block));
                        break;
                    }
                }
                Err(error) if error.code == "E_SUM_MISSING_CONTEXT" => {
                    context.discard_diagnostic();
                    missing_context.get_or_insert(error);
                }
                Err(error) => return Err(error),
            }
        }
        inferred.ok_or_else(|| {
            missing_context.unwrap_or_else(|| SemanticError {
                code: "E_DIVERGING_EXPRESSION_CONTEXT",
                message: "a match whose arms all return requires an exact type context".into(),
            })
        })?
    };

    let mut typed_arms = Vec::with_capacity(checked.len());
    for (arm, pattern, binding) in checked {
        let mut arm_vars = vars.clone();
        if let Some((name, ty)) = binding {
            ensure_new_local_binding(context, &name, &arm_vars)?;
            arm_vars.insert(name, ty);
        }
        let body = analyze_expression_block(context, &arm.body, &mut arm_vars, Some(&inferred))?;
        if !typed_block_diverges(&body) {
            require_exact_branch_type(&inferred, &typed_block_value_type(&body))?;
        }
        typed_arms.push(TypedMatchArm { pattern, body });
    }
    Ok(TypedExpr {
        expr: ExprKind::Match {
            value: Box::new(value),
            arms: typed_arms,
        },
        ty: inferred,
    })
}

fn analyze_expr(
    context: &SemanticContext,
    expr: &Expr,
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    analyze_expr_expected(context, expr, vars, None)
}

fn analyze_list_literal(
    context: &SemanticContext,
    elements: &[Expr],
    vars: &mut HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    let expected = expected.map(resolve_struct_type);
    let (expected_element, capacity) = match expected {
        Some(Type::List(element, capacity)) => (Some(*element), capacity),
        Some(other) => {
            return Err(SemanticError {
                code: "E_LIST_CONTEXT_TYPE",
                message: format!("a list literal cannot initialize `{}`", type_name(&other)),
            });
        }
        None if elements.is_empty() => {
            return Err(SemanticError {
                code: "E_LIST_EMPTY_CONTEXT",
                message: "an empty list requires an exact `List<T, N>` type context".into(),
            });
        }
        None => {
            let capacity = u8::try_from(elements.len())
                .ok()
                .filter(|capacity| *capacity <= 64)
                .ok_or_else(|| SemanticError {
                    code: "E_LIST_CAPACITY",
                    message: format!(
                        "a list literal with {} elements exceeds the V1 capacity limit of 64",
                        elements.len()
                    ),
                })?;
            (None, capacity)
        }
    };
    if elements.len() > usize::from(capacity) {
        return Err(SemanticError {
            code: "E_LIST_LITERAL_CAPACITY",
            message: format!(
                "list literal has {} elements but its contextual capacity is {capacity}",
                elements.len()
            ),
        });
    }

    let mut typed = Vec::with_capacity(elements.len());
    let mut element_type = expected_element;
    for element in elements {
        let mut value = analyze_expr_expected(context, element, vars, element_type.as_ref())?;
        if let Some(expected_element) = &element_type {
            ensure_assignable_and_coerce(expected_element, &mut value)?;
        } else {
            element_type = Some(resolve_struct_type(&value.ty));
        }
        typed.push(value);
    }
    let element_type = element_type.expect("non-empty uncontextualized list inferred an element");
    if list_element_contains_resource_handle(&element_type) {
        return Err(SemanticError {
            code: "E_LIST_RESOURCE_ELEMENT",
            message: format!(
                "List elements cannot contain resource handle type `{}`",
                type_name(&element_type)
            ),
        });
    }
    let list_type = Type::List(Box::new(element_type), capacity);
    validate_list_schemas(&list_type)?;
    Ok(TypedExpr {
        expr: ExprKind::List(typed),
        ty: list_type,
    })
}

fn analyze_list_comprehension(
    context: &SemanticContext,
    expression: &Expr,
    item: &str,
    source: &Expr,
    condition: Option<&Expr>,
    vars: &mut HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    let source = analyze_expr(context, source, vars)?;
    let Type::List(source_element, source_capacity) = resolve_struct_type(&source.ty) else {
        return Err(SemanticError {
            code: "E_LIST_COMPREHENSION_SOURCE",
            message: format!(
                "list comprehension source must be `List<T, N>`, found `{}`",
                type_name(&source.ty)
            ),
        });
    };
    ensure_new_local_binding(context, item, vars)?;

    let (expected_element, result_capacity) = match expected.map(resolve_struct_type) {
        Some(Type::List(element, capacity)) => {
            if source_capacity > capacity {
                context
                    .required_list_capacity
                    .replace(Some(source_capacity));
                return Err(SemanticError {
                    code: "E_LIST_COMPREHENSION_CAPACITY",
                    message: format!(
                        "source capacity {source_capacity} may exceed contextual capacity {capacity}; filters do not reduce the proven maximum"
                    ),
                });
            }
            (Some(*element), capacity)
        }
        Some(other) => {
            return Err(SemanticError {
                code: "E_LIST_CONTEXT_TYPE",
                message: format!(
                    "a list comprehension cannot initialize `{}`",
                    type_name(&other)
                ),
            });
        }
        None => (None, source_capacity),
    };

    let mut comprehension_vars = vars.clone();
    comprehension_vars.insert(item.to_owned(), (*source_element).clone());
    let mut expression = analyze_expr_expected(
        context,
        expression,
        &mut comprehension_vars,
        expected_element.as_ref(),
    )?;
    if let Some(expected_element) = &expected_element {
        ensure_assignable_and_coerce(expected_element, &mut expression)?;
    }
    let element_type = expected_element.unwrap_or_else(|| resolve_struct_type(&expression.ty));
    if list_element_contains_resource_handle(&element_type) {
        return Err(SemanticError {
            code: "E_LIST_RESOURCE_ELEMENT",
            message: format!(
                "List elements cannot contain resource handle type `{}`",
                type_name(&element_type)
            ),
        });
    }
    let list_type = Type::List(Box::new(element_type), result_capacity);
    validate_list_schemas(&list_type)?;
    let condition = condition
        .map(|condition| analyze_expr(context, condition, &mut comprehension_vars))
        .transpose()?;
    if let Some(condition) = &condition {
        crate::secret::reject_secret_control_flow(condition)?;
        if resolve_struct_type(&condition.ty) != Type::Bool {
            return Err(SemanticError {
                code: "E_LIST_COMPREHENSION_FILTER",
                message: "comprehension filter must be bool".into(),
            });
        }
    }
    Ok(TypedExpr {
        expr: ExprKind::ListComprehension {
            expression: Box::new(expression),
            item: item.to_owned(),
            source: Box::new(source),
            condition: condition.map(Box::new),
        },
        ty: list_type,
    })
}

fn is_native_json_value_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Int
        | Type::Decimal
        | Type::Quantity
        | Type::Bool
        | Type::String
        | Type::Bytes
        | Type::DataSpaceId
        | Type::AccountId
        | Type::AssetDefinitionId
        | Type::AssetId
        | Type::NftId
        | Type::DomainId
        | Type::Name
        | Type::Json => true,
        Type::Option(inner) | Type::List(inner, _) => is_native_json_value_type(&inner),
        Type::Unit
        | Type::AxtDescriptor
        | Type::AssetHandle
        | Type::ProofBlob
        | Type::SoracloudRequest
        | Type::SoracloudResponse
        | Type::Secret(_)
        | Type::StateMap(_, _)
        | Type::Result(_, _)
        | Type::Tuple(_)
        | Type::Struct { .. }
        | Type::NamedStruct(_) => false,
    }
}

fn analyze_native_json_value(
    context: &SemanticContext,
    expression: &Expr,
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    let typed = analyze_expr(context, expression, vars)?;
    if !is_native_json_value_type(&typed.ty) {
        return Err(SemanticError {
            code: "E_JSON_VALUE_TYPE",
            message: format!(
                "native JSON construction cannot convert `{}` implicitly; handle Result and arbitrary structs/tuples explicitly, and keep resource handles outside JSON",
                type_name(&typed.ty)
            ),
        });
    }
    Ok(typed)
}

fn analyze_list_method_call(
    context: &SemanticContext,
    source_name: &str,
    args: &[Expr],
    argument_names: Option<&[String]>,
    implicit_receiver: bool,
    vars: &mut HashMap<String, Type>,
) -> Option<Result<TypedExpr, SemanticError>> {
    if !implicit_receiver || args.is_empty() {
        return None;
    }
    let method = match source_name {
        "len" => (LIST_LEN_INTRINSIC, &[][..]),
        STATE_MAP_GET_INTRINSIC | "get" => (LIST_GET_INTRINSIC, &["index"][..]),
        "try_set" => (LIST_TRY_SET_INTRINSIC, &["index", "value"][..]),
        "try_push" => (LIST_TRY_PUSH_INTRINSIC, &["value"][..]),
        "pop" => (LIST_POP_INTRINSIC, &[][..]),
        "contains" => (LIST_CONTAINS_INTRINSIC, &["value"][..]),
        "take" => (LIST_TAKE_INTRINSIC, &["limit"][..]),
        "enumerate" => (LIST_ENUMERATE_INTRINSIC, &[][..]),
        _ => return None,
    };

    let receiver = match analyze_expr(context, &args[0], vars) {
        Ok(receiver) => receiver,
        Err(error) => return Some(Err(error)),
    };
    let Type::List(element, capacity) = resolve_struct_type(&receiver.ty) else {
        return None;
    };
    if matches!(
        method.0,
        LIST_TRY_SET_INTRINSIC | LIST_TRY_PUSH_INTRINSIC | LIST_POP_INTRINSIC
    ) {
        let Some(Expr::Ident(receiver_name)) = args.first().map(Expr::kind) else {
            return Some(Err(SemanticError {
                code: "E_LIST_MUTABLE_RECEIVER",
                message: format!(
                    "List.{source_name} mutates its receiver; call it on a `var` list binding"
                ),
            }));
        };
        let receiver_is_mutable =
            match context.list_receiver_is_mutable(&args[0], receiver_name, vars) {
                Ok(receiver_is_mutable) => receiver_is_mutable,
                Err(error) => return Some(Err(error)),
            };
        if !receiver_is_mutable {
            return Some(Err(SemanticError {
                code: "E_LIST_MUTABLE_RECEIVER",
                message: format!(
                    "List.{source_name} requires mutable receiver `{receiver_name}`; declare it with `var`"
                ),
            }));
        }
    }
    let parameter_names = method
        .1
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    let named_only_reason = (method.0 == LIST_TRY_SET_INTRINSIC
        && resolve_struct_type(&element) == Type::Int)
        .then_some(
            "the int index and int element value can be transposed, so their names are required",
        );
    let plan = match reorder_call_arguments(
        source_name,
        args,
        argument_names,
        true,
        &parameter_names,
        &vec![true; parameter_names.len()],
        named_only_reason,
    ) {
        Ok(plan) => plan,
        Err(error) => return Some(Err(error)),
    };
    let expected_arity = parameter_names.len() + 1;
    if plan.ordered.len() != expected_arity {
        return Some(Err(SemanticError {
            code: "E_LIST_METHOD_ARITY",
            message: format!(
                "List.{} expects {} argument{}, got {}",
                source_name,
                parameter_names.len(),
                if parameter_names.len() == 1 { "" } else { "s" },
                plan.ordered.len().saturating_sub(1)
            ),
        }));
    }

    let mut typed_slots = (0..plan.ordered.len()).map(|_| None).collect::<Vec<_>>();
    typed_slots[0] = Some(receiver);
    for index in plan
        .evaluation_order
        .iter()
        .copied()
        .filter(|index| *index != 0)
    {
        let argument = &plan.ordered[index];
        let typed = match method.0 {
            LIST_GET_INTRINSIC | LIST_TAKE_INTRINSIC => {
                let argument = match analyze_expr(context, argument, vars) {
                    Ok(argument) => argument,
                    Err(error) => return Some(Err(error)),
                };
                if resolve_struct_type(&argument.ty) != Type::Int {
                    return Some(Err(SemanticError {
                        code: "E_LIST_INDEX_TYPE",
                        message: format!("List.{} expects an int index or limit", source_name),
                    }));
                }
                argument
            }
            LIST_TRY_SET_INTRINSIC if index == 1 => {
                let argument = match analyze_expr(context, argument, vars) {
                    Ok(argument) => argument,
                    Err(error) => return Some(Err(error)),
                };
                if resolve_struct_type(&argument.ty) != Type::Int {
                    return Some(Err(SemanticError {
                        code: "E_LIST_INDEX_TYPE",
                        message: "List.try_set expects an int index".into(),
                    }));
                }
                argument
            }
            LIST_TRY_SET_INTRINSIC | LIST_TRY_PUSH_INTRINSIC | LIST_CONTAINS_INTRINSIC => {
                let mut argument =
                    match analyze_expr_expected(context, argument, vars, Some(&element)) {
                        Ok(argument) => argument,
                        Err(error) => return Some(Err(error)),
                    };
                if let Err(error) = ensure_assignable_and_coerce(&element, &mut argument) {
                    return Some(Err(error));
                }
                argument
            }
            _ => unreachable!("zero-argument methods have no arguments to analyze"),
        };
        typed_slots[index] = Some(typed);
    }
    let typed = match typed_slots
        .into_iter()
        .enumerate()
        .map(|(index, argument)| {
            argument.ok_or_else(|| SemanticError {
                code: "E_MALFORMED_CALL",
                message: format!("List.{source_name} did not analyze argument slot {index}"),
            })
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(typed) => typed,
        Err(error) => return Some(Err(error)),
    };

    let result_type = match method.0 {
        LIST_LEN_INTRINSIC => Type::Int,
        LIST_GET_INTRINSIC | LIST_POP_INTRINSIC => Type::Option(element.clone()),
        LIST_TRY_SET_INTRINSIC | LIST_TRY_PUSH_INTRINSIC => Type::Bool,
        LIST_CONTAINS_INTRINSIC => {
            if !list_element_is_comparable(&element) {
                return Some(Err(SemanticError {
                    code: "E_LIST_CONTAINS_COMPARABILITY",
                    message: format!(
                        "List.contains requires a canonically comparable element type, found `{}`",
                        type_name(&element)
                    ),
                }));
            }
            Type::Bool
        }
        LIST_TAKE_INTRINSIC => {
            let Some(limit) = typed.get(1).and_then(|argument| match argument.kind() {
                ExprKind::IntLiteral(limit) => Some(limit),
                _ => None,
            }) else {
                return Some(Err(SemanticError {
                    code: "E_LIST_TAKE_CONST",
                    message: "List.take limit must be a compile-time integer constant".into(),
                }));
            };
            let limit = match limit
                .try_to_u64()
                .and_then(|limit| u8::try_from(limit).ok())
                .filter(|limit| *limit <= capacity)
            {
                Some(limit) => limit,
                None => {
                    return Some(Err(SemanticError {
                        code: "E_LIST_TAKE_LIMIT",
                        message: format!(
                            "List.take limit {limit} is outside 0..={capacity} for this source List"
                        ),
                    }));
                }
            };
            // V1 List schemas have capacities in 1..=64. `take(0)` is still
            // useful and deterministically produces an empty value, represented
            // by the smallest valid static result capacity.
            Type::List(element.clone(), limit.max(1))
        }
        LIST_ENUMERATE_INTRINSIC => Type::List(
            Box::new(Type::Tuple(vec![Type::Int, (*element).clone()])),
            capacity,
        ),
        _ => unreachable!("known List intrinsic"),
    };
    Some(Ok(retain_named_call_evaluation_order(
        TypedExpr {
            expr: ExprKind::Call {
                name: method.0.to_owned(),
                args: typed,
            },
            ty: result_type,
        },
        &plan,
    )))
}

fn numeric_rounding_mode(expression: &Expr) -> Option<(RoundingMode, i64)> {
    use ivm_abi::numeric::RoundingModeV1 as AbiMode;

    let (mode, tag) = match expression {
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            return numeric_rounding_mode(expression);
        }
        Expr::Ident(name) if name == "Rounding::toward_zero" => {
            (RoundingMode::TowardZero, AbiMode::TowardZero.tag())
        }
        Expr::Ident(name) if name == "Rounding::away_from_zero" => {
            (RoundingMode::AwayFromZero, AbiMode::AwayFromZero.tag())
        }
        Expr::Ident(name) if name == "Rounding::floor" => {
            (RoundingMode::Floor, AbiMode::Floor.tag())
        }
        Expr::Ident(name) if name == "Rounding::ceil" => (RoundingMode::Ceil, AbiMode::Ceil.tag()),
        Expr::Ident(name) if name == "Rounding::nearest_even" => {
            (RoundingMode::NearestEven, AbiMode::NearestEven.tag())
        }
        Expr::Ident(name) if name == "Rounding::nearest_away" => {
            (RoundingMode::NearestAway, AbiMode::NearestAway.tag())
        }
        Expr::Ident(name) if name == "Rounding::nearest_toward_zero" => (
            RoundingMode::NearestTowardZero,
            AbiMode::NearestTowardZero.tag(),
        ),
        _ => return None,
    };
    i64::try_from(tag).ok().map(|tag| (mode, tag))
}

fn analyze_decimal_to_int_round_call(
    context: &SemanticContext,
    source_name: &str,
    args: &[Expr],
    argument_names: Option<&[String]>,
    implicit_receiver: bool,
    vars: &mut HashMap<String, Type>,
) -> Option<Result<TypedExpr, SemanticError>> {
    if source_name != "decimal::to_int_round" || implicit_receiver {
        return None;
    }
    let names = ["value".to_owned(), "mode".to_owned()];
    let plan = match reorder_call_arguments(
        source_name,
        args,
        argument_names,
        false,
        &names,
        &[true, true],
        None,
    ) {
        Ok(plan) => plan,
        Err(error) => return Some(Err(error)),
    };
    if plan.ordered.len() != 2 {
        return Some(Err(SemanticError {
            code: "E_NUMERIC_ROUND_ARITY",
            message: "decimal::to_int_round expects value and mode".into(),
        }));
    }
    let mut value =
        match analyze_expr_expected(context, &plan.ordered[0], vars, Some(&Type::Decimal)) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };
    if let Err(error) = ensure_assignable_and_coerce(&Type::Decimal, &mut value) {
        return Some(Err(error));
    }
    let Some((rounding, tag)) = numeric_rounding_mode(&plan.ordered[1]) else {
        return Some(Err(SemanticError {
            code: "E_NUMERIC_ROUNDING_MODE",
            message: format!(
                "decimal::to_int_round mode must be one of {}",
                V1_ROUNDING_PATHS.join(", ")
            ),
        }));
    };
    if let Ok(Some(crate::checked_arithmetic::ConstantNumeric::Decimal(constant))) =
        crate::checked_arithmetic::evaluate(&value)
    {
        let integer = match constant.decimal_to_int_round(rounding) {
            Ok(integer) => integer,
            Err(error) => {
                let error = crate::checked_arithmetic::ConstantNumericError::Numeric(error);
                return Some(Err(SemanticError {
                    code: error.code(),
                    message: error.to_string(),
                }));
            }
        };
        return Some(Ok(TypedExpr {
            expr: ExprKind::IntLiteral(integer),
            ty: Type::Int,
        }));
    }
    let mode = TypedExpr {
        expr: ExprKind::IntLiteral(BigInt::from(tag)),
        ty: Type::Int,
    };
    Some(Ok(retain_named_call_evaluation_order(
        TypedExpr {
            expr: ExprKind::Call {
                name: DECIMAL_TO_INT_ROUND_INTRINSIC.to_owned(),
                args: vec![value, mode],
            },
            ty: Type::Int,
        },
        &plan,
    )))
}

fn analyze_numeric_round_method_call(
    context: &SemanticContext,
    source_name: &str,
    args: &[Expr],
    argument_names: Option<&[String]>,
    implicit_receiver: bool,
    vars: &mut HashMap<String, Type>,
) -> Option<Result<TypedExpr, SemanticError>> {
    if !implicit_receiver || args.is_empty() || !matches!(source_name, "div_round" | "ratio_round")
    {
        return None;
    }
    let receiver = match analyze_expr(context, &args[0], vars) {
        Ok(receiver) => receiver,
        Err(error) => return Some(Err(error)),
    };
    let receiver_type = resolve_struct_type(&receiver.ty);
    let (divisor_type, result_type, intrinsic, display_name) = match (source_name, &receiver_type) {
        ("div_round", Type::Decimal) => (
            Type::Decimal,
            Type::Decimal,
            DECIMAL_DIV_ROUND_INTRINSIC,
            "decimal.div_round",
        ),
        ("div_round", Type::Quantity) => (
            Type::Decimal,
            Type::Quantity,
            QUANTITY_DIV_ROUND_INTRINSIC,
            "quantity.div_round",
        ),
        ("ratio_round", Type::Quantity) => (
            Type::Quantity,
            Type::Decimal,
            QUANTITY_RATIO_ROUND_INTRINSIC,
            "quantity.ratio_round",
        ),
        _ => {
            return Some(Err(SemanticError {
                code: "E_NUMERIC_ROUND_RECEIVER",
                message: format!(
                    "{source_name} is not defined for receiver type `{}`",
                    type_name(&receiver_type)
                ),
            }));
        }
    };

    let parameter_names = ["divisor", "scale", "mode"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let plan = match reorder_call_arguments(
        display_name,
        args,
        argument_names,
        true,
        &parameter_names,
        &[true, true, true],
        Some("rounded numeric division requires explicit `divisor`, `scale`, and `mode` names"),
    ) {
        Ok(plan) => plan,
        Err(error) => return Some(Err(error)),
    };
    if plan.ordered.len() != 4 {
        return Some(Err(SemanticError {
            code: "E_NUMERIC_ROUND_ARITY",
            message: format!(
                "{display_name} expects divisor, scale, and mode, got {} argument(s)",
                plan.ordered.len().saturating_sub(1)
            ),
        }));
    }

    let mut typed_slots = (0..4).map(|_| None).collect::<Vec<_>>();
    typed_slots[0] = Some(receiver);
    let mut rounding_mode = None;
    for index in plan
        .evaluation_order
        .iter()
        .copied()
        .filter(|index| *index != 0)
    {
        let typed = match index {
            1 => {
                let mut divisor = match analyze_expr_expected(
                    context,
                    &plan.ordered[index],
                    vars,
                    Some(&divisor_type),
                ) {
                    Ok(divisor) => divisor,
                    Err(error) => return Some(Err(error)),
                };
                if let Err(error) = ensure_assignable_and_coerce(&divisor_type, &mut divisor) {
                    return Some(Err(error));
                }
                divisor
            }
            2 => match analyze_expr_expected(context, &plan.ordered[index], vars, Some(&Type::Int))
            {
                Ok(scale) => scale,
                Err(error) => return Some(Err(error)),
            },
            3 => {
                let Some((mode, mode_tag)) = numeric_rounding_mode(&plan.ordered[index]) else {
                    return Some(Err(SemanticError {
                        code: "E_NUMERIC_ROUNDING_MODE",
                        message: format!(
                            "{display_name} mode must be one of {}",
                            V1_ROUNDING_PATHS.join(", ")
                        ),
                    }));
                };
                rounding_mode = Some(mode);
                TypedExpr {
                    expr: ExprKind::IntLiteral(BigInt::from(mode_tag)),
                    ty: Type::Int,
                }
            }
            _ => unreachable!("rounded division has exactly four ABI slots"),
        };
        typed_slots[index] = Some(typed);
    }
    let mut typed = match typed_slots
        .into_iter()
        .enumerate()
        .map(|(index, argument)| {
            argument.ok_or_else(|| SemanticError {
                code: "E_MALFORMED_CALL",
                message: format!("{display_name} did not analyze argument slot {index}"),
            })
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(typed) => typed,
        Err(error) => return Some(Err(error)),
    };
    let receiver = typed.remove(0);
    let divisor = typed.remove(0);
    let scale = typed.remove(0);
    let mode = typed.remove(0);
    if let ExprKind::IntLiteral(scale_value) = scale.kind()
        && scale_value
            .try_to_u64()
            .is_none_or(|scale_value| scale_value > 28)
    {
        return Some(Err(SemanticError {
            code: "E_INVALID_SCALE",
            message: format!("rounded numeric scale {scale_value} is outside 0..=28"),
        }));
    }
    let rounding_mode = rounding_mode.expect("validated rounding mode slot");
    if numeric_literal_is_zero(&divisor) {
        return Some(Err(SemanticError {
            code: "E_DIVISION_BY_ZERO",
            message: format!("{display_name} divisor must not be zero"),
        }));
    }

    let constant_scale = match scale.kind() {
        ExprKind::IntLiteral(scale) => scale
            .try_to_u64()
            .and_then(|scale| u32::try_from(scale).ok()),
        _ => None,
    };
    if let Some(output_scale) = constant_scale {
        use crate::checked_arithmetic::{ConstantNumeric, ConstantNumericError};
        let lhs = match crate::checked_arithmetic::evaluate(&receiver) {
            Ok(value) => value,
            Err(error) => {
                return Some(Err(SemanticError {
                    code: error.code(),
                    message: error.to_string(),
                }));
            }
        };
        let rhs = match crate::checked_arithmetic::evaluate(&divisor) {
            Ok(value) => value,
            Err(error) => {
                return Some(Err(SemanticError {
                    code: error.code(),
                    message: error.to_string(),
                }));
            }
        };
        let folded = match (intrinsic, lhs, rhs) {
            (
                DECIMAL_DIV_ROUND_INTRINSIC,
                Some(ConstantNumeric::Decimal(lhs)),
                Some(ConstantNumeric::Decimal(rhs)),
            ) => lhs
                .try_decimal_div_round(&rhs, output_scale, rounding_mode)
                .map(ConstantNumeric::Decimal),
            (
                QUANTITY_DIV_ROUND_INTRINSIC,
                Some(ConstantNumeric::Quantity(lhs)),
                Some(ConstantNumeric::Decimal(rhs)),
            ) => lhs
                .try_div_decimal_round(&rhs, output_scale, rounding_mode)
                .map(ConstantNumeric::Quantity),
            (
                QUANTITY_RATIO_ROUND_INTRINSIC,
                Some(ConstantNumeric::Quantity(lhs)),
                Some(ConstantNumeric::Quantity(rhs)),
            ) => lhs
                .try_ratio_round(&rhs, output_scale, rounding_mode)
                .map(ConstantNumeric::Decimal),
            (_, None, _) | (_, _, None) => {
                return Some(Ok(retain_named_call_evaluation_order(
                    TypedExpr {
                        expr: ExprKind::Call {
                            name: intrinsic.to_owned(),
                            args: vec![receiver, divisor, scale, mode],
                        },
                        ty: result_type,
                    },
                    &plan,
                )));
            }
            _ => {
                return Some(Err(SemanticError {
                    code: "E_INTERNAL_NUMERIC_MATRIX",
                    message: "rounded numeric operands violate their typed operator matrix".into(),
                }));
            }
        };
        let folded = match folded {
            Ok(folded) => folded,
            Err(error) => {
                let error = ConstantNumericError::Numeric(error);
                return Some(Err(SemanticError {
                    code: error.code(),
                    message: error.to_string(),
                }));
            }
        };
        let value = match folded {
            ConstantNumeric::Decimal(value) => value,
            ConstantNumeric::Quantity(value) => value.into_numeric(),
            ConstantNumeric::Int(_) => unreachable!("rounded division never returns int"),
        };
        return Some(Ok(TypedExpr {
            expr: ExprKind::DecimalLiteral {
                spelling: value.to_string(),
                value,
            },
            ty: result_type,
        }));
    }

    Some(Ok(retain_named_call_evaluation_order(
        TypedExpr {
            expr: ExprKind::Call {
                name: intrinsic.to_owned(),
                args: vec![receiver, divisor, scale, mode],
            },
            ty: result_type,
        },
        &plan,
    )))
}

fn analyze_expr_expected(
    context: &SemanticContext,
    expr: &Expr,
    vars: &mut HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    let result = analyze_expr_expected_inner(context, expr, vars, expected).and_then(|typed| {
        if matches!(
            typed.kind(),
            ExprKind::JsonObject(_) | ExprKind::JsonArray(_)
        ) && let Err(error) = crate::abi_schema::json_construction_schema(&typed)
        {
            return Err(SemanticError {
                code: "E_JSON_SCHEMA_LIMIT",
                message: error.to_string(),
            });
        }
        context.record_typed_hir_node(expr, &typed.ty)?;
        Ok(typed)
    });
    if result.is_err() {
        context.capture_expression_diagnostic(expr, None);
    }
    result
}

fn analyze_expr_expected_inner(
    context: &SemanticContext,
    expr: &Expr,
    vars: &mut HashMap<String, Type>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    match expr.kind() {
        Expr::Source { .. } | Expr::Resolved { .. } => {
            unreachable!("kind() strips AST and resolved-HIR provenance wrappers")
        }
        Expr::OptionSome(value) => {
            let expected_payload = match expected
                .map(|ty| resolve_struct_type_with_context(context, ty))
                .transpose()?
            {
                Some(Type::Option(payload)) => Some(*payload),
                Some(other) => {
                    return Err(SemanticError {
                        code: "E_SUM_CONTEXT_TYPE",
                        message: format!(
                            "`Option::some` cannot initialize `{}`",
                            type_name(&other)
                        ),
                    });
                }
                None => None,
            };
            let mut value = analyze_expr_expected(context, value, vars, expected_payload.as_ref())?;
            if let Some(payload) = &expected_payload {
                ensure_assignable_and_coerce(payload, &mut value)?;
            }
            let payload = resolve_struct_type(&value.ty);
            if !is_supported_sum_payload(&payload) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "Option<T> V1 payloads must be durable-value types".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::OptionSome {
                    value: Box::new(value),
                },
                ty: Type::Option(Box::new(payload)),
            })
        }
        Expr::OptionNone => {
            let payload = match expected
                .map(|ty| resolve_struct_type_with_context(context, ty))
                .transpose()?
            {
                Some(Type::Option(payload)) => payload,
                Some(other) => {
                    return Err(SemanticError {
                        code: "E_SUM_CONTEXT_TYPE",
                        message: format!(
                            "`Option::none` cannot initialize `{}`",
                            type_name(&other)
                        ),
                    });
                }
                None => {
                    return Err(SemanticError {
                        code: "E_SUM_MISSING_CONTEXT",
                        message: "`Option::none` requires an exact `Option<T>` context from an annotation, return type, field, or parameter".into(),
                    });
                }
            };
            if !is_supported_sum_payload(&payload) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "Option<T> V1 payloads must be durable-value types".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::OptionNone,
                ty: Type::Option(payload),
            })
        }
        Expr::ResultOk(value) => {
            let (ok, error) = match expected
                .map(|ty| resolve_struct_type_with_context(context, ty))
                .transpose()?
            {
                Some(Type::Result(ok, error)) => (ok, error),
                Some(other) => {
                    return Err(SemanticError {
                        code: "E_SUM_CONTEXT_TYPE",
                        message: format!("`Result::ok` cannot initialize `{}`", type_name(&other)),
                    });
                }
                None => {
                    return Err(SemanticError {
                        code: "E_SUM_MISSING_CONTEXT",
                        message: "`Result::ok` requires an exact `Result<T, E>` context so the inactive error type is known".into(),
                    });
                }
            };
            let mut value = analyze_expr_expected(context, value, vars, Some(&ok))?;
            ensure_assignable_and_coerce(&ok, &mut value)?;
            if !is_supported_sum_payload(&ok) || !is_supported_sum_payload(&error) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "Result<T, E> V1 payloads must be durable-value types".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::ResultOk {
                    value: Box::new(value),
                },
                ty: Type::Result(ok, error),
            })
        }
        Expr::ResultErr(error) => {
            let (ok, error_ty) = match expected
                .map(|ty| resolve_struct_type_with_context(context, ty))
                .transpose()?
            {
                Some(Type::Result(ok, error_ty)) => (ok, error_ty),
                Some(other) => {
                    return Err(SemanticError {
                        code: "E_SUM_CONTEXT_TYPE",
                        message: format!("`Result::err` cannot initialize `{}`", type_name(&other)),
                    });
                }
                None => {
                    return Err(SemanticError {
                        code: "E_SUM_MISSING_CONTEXT",
                        message: "`Result::err` requires an exact `Result<T, E>` context so the inactive success type is known".into(),
                    });
                }
            };
            let mut error = analyze_expr_expected(context, error, vars, Some(&error_ty))?;
            ensure_assignable_and_coerce(&error_ty, &mut error)?;
            if !is_supported_sum_payload(&ok) || !is_supported_sum_payload(&error_ty) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "Result<T, E> V1 payloads must be durable-value types".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::ResultErr {
                    error: Box::new(error),
                },
                ty: Type::Result(ok, error_ty),
            })
        }
        Expr::Propagate(value) => {
            let value = analyze_expr(context, value, vars)?;
            let function_return = context
                .current_function_name
                .borrow()
                .as_ref()
                .and_then(|name| context.function_returns.borrow().get(name).cloned())
                .unwrap_or(Type::Unit);
            let output = match (
                resolve_struct_type(&value.ty),
                resolve_struct_type(&function_return),
            ) {
                (Type::Option(payload), Type::Option(_)) => *payload,
                (Type::Result(payload, error), Type::Result(_, return_error)) => {
                    if resolve_struct_type(&error) != resolve_struct_type(&return_error) {
                        return Err(SemanticError {
                            code: "E_PROPAGATE_ERROR_TYPE",
                            message: format!(
                                "postfix `?` has error type `{}` but the enclosing function returns `{}`; implicit error conversion is not allowed",
                                type_name(&error),
                                type_name(&return_error)
                            ),
                        });
                    }
                    *payload
                }
                (Type::Option(_), other) => {
                    return Err(SemanticError {
                        code: "E_PROPAGATE_CONTEXT",
                        message: format!(
                            "postfix `?` on Option requires an Option-returning function, found `{}`",
                            type_name(&other)
                        ),
                    });
                }
                (Type::Result(_, _), other) => {
                    return Err(SemanticError {
                        code: "E_PROPAGATE_CONTEXT",
                        message: format!(
                            "postfix `?` on Result requires a Result-returning function, found `{}`",
                            type_name(&other)
                        ),
                    });
                }
                (other, _) => {
                    return Err(SemanticError {
                        code: "E_PROPAGATE_TYPE",
                        message: format!(
                            "postfix `?` expects Option or Result, found `{}`",
                            type_name(&other)
                        ),
                    });
                }
            };
            Ok(TypedExpr {
                expr: ExprKind::Propagate {
                    value: Box::new(value),
                },
                ty: output,
            })
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let condition = analyze_expr(context, condition, vars)?;
            crate::secret::reject_secret_control_flow(&condition)?;
            if condition.ty != Type::Bool {
                return Err(SemanticError {
                    code: "K2003",
                    message: "if condition must be bool".into(),
                });
            }
            let Some(else_branch) = else_branch else {
                return Err(SemanticError {
                    code: "E_IF_EXPRESSION_ELSE",
                    message: "expression-valued `if` requires an `else` block".into(),
                });
            };
            let (then_branch, else_branch, ty) =
                analyze_expression_branches(context, then_branch, else_branch, vars, expected)?;
            Ok(TypedExpr {
                expr: ExprKind::If {
                    condition: Box::new(condition),
                    then_branch,
                    else_branch,
                },
                ty,
            })
        }
        Expr::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => {
            let value = analyze_expr(context, value, vars)?;
            let (pattern, binding) = analyze_sum_pattern(pattern, &value.ty)?;
            let Some(else_branch) = else_branch else {
                return Err(SemanticError {
                    code: "E_IF_LET_EXPRESSION_ELSE",
                    message: "expression-valued `if let` requires an `else` block".into(),
                });
            };
            let mut then_vars = vars.clone();
            if let Some((name, ty)) = binding {
                ensure_new_local_binding(context, &name, &then_vars)?;
                then_vars.insert(name, ty);
            }
            let (then_branch, else_branch, ty) = analyze_expression_branches_with_envs(
                context,
                then_branch,
                else_branch,
                &mut then_vars,
                &mut vars.clone(),
                expected,
            )?;
            Ok(TypedExpr {
                expr: ExprKind::IfLet {
                    pattern,
                    value: Box::new(value),
                    then_branch,
                    else_branch,
                },
                ty,
            })
        }
        Expr::Match { value, arms } => {
            let value = analyze_expr(context, value, vars)?;
            analyze_match_expression(context, value, arms, vars, expected)
        }
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            let c = analyze_expr(context, cond, vars)?;
            crate::secret::reject_secret_control_flow(&c)?;
            if c.ty != Type::Bool {
                return Err(SemanticError {
                    code: "K2003",
                    message: "conditional expects a bool condition".into(),
                });
            }
            let t1 = analyze_expr_expected(context, then_expr, vars, expected)?;
            let t2 = analyze_expr_expected(context, else_expr, vars, Some(&t1.ty))?;
            if t1.ty != t2.ty {
                return Err(SemanticError {
                    code: "K2003",
                    message: "conditional branches must have the same type".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Conditional {
                    cond: Box::new(c),
                    then_expr: Box::new(t1.clone()),
                    else_expr: Box::new(t2.clone()),
                },
                ty: t1.ty,
            })
        }
        Expr::Tuple(elems) => {
            let expected_elements = match expected
                .map(|expected| resolve_struct_type_with_context(context, expected))
                .transpose()?
            {
                Some(Type::Tuple(elements)) if elements.len() == elems.len() => Some(elements),
                _ => None,
            };
            let mut typed = Vec::with_capacity(elems.len());
            for (index, element) in elems.iter().enumerate() {
                let expected_element = expected_elements
                    .as_ref()
                    .and_then(|elements| elements.get(index));
                let mut element = analyze_expr_expected(context, element, vars, expected_element)?;
                if let Some(expected_element) = expected_element {
                    ensure_assignable_and_coerce(expected_element, &mut element)?;
                }
                typed.push(element);
            }
            let tys = typed.iter().map(|t| t.ty.clone()).collect();
            Ok(TypedExpr {
                expr: ExprKind::Tuple(typed),
                ty: Type::Tuple(tys),
            })
        }
        Expr::List(elements) => analyze_list_literal(context, elements, vars, expected),
        Expr::ListComprehension {
            expression,
            item,
            source,
            condition,
        } => analyze_list_comprehension(
            context,
            expression,
            item,
            source,
            condition.as_deref(),
            vars,
            expected,
        ),
        Expr::JsonObject(entries) => {
            if entries.len() > 64 {
                return Err(SemanticError {
                    code: "E_JSON_CAPACITY",
                    message: format!(
                        "native JSON objects contain at most 64 entries per node; this object has {}",
                        entries.len()
                    ),
                });
            }
            let mut keys = HashSet::with_capacity(entries.len());
            let mut typed_entries = Vec::with_capacity(entries.len());
            for entry in entries {
                if !keys.insert(entry.key.clone()) {
                    return Err(SemanticError {
                        code: "E_JSON_DUPLICATE_KEY",
                        message: format!(
                            "native JSON object key `{}` is supplied more than once after string decoding",
                            entry.key
                        ),
                    });
                }
                typed_entries.push((
                    entry.key.clone(),
                    analyze_native_json_value(context, &entry.value, vars)?,
                ));
            }
            Ok(TypedExpr {
                expr: ExprKind::JsonObject(typed_entries),
                ty: Type::Json,
            })
        }
        Expr::JsonArray(elements) => {
            if elements.len() > 64 {
                return Err(SemanticError {
                    code: "E_JSON_CAPACITY",
                    message: format!(
                        "native JSON arrays contain at most 64 elements per node; this array has {}",
                        elements.len()
                    ),
                });
            }
            let elements = elements
                .iter()
                .map(|element| analyze_native_json_value(context, element, vars))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TypedExpr {
                expr: ExprKind::JsonArray(elements),
                ty: Type::Json,
            })
        }
        Expr::IntLiteral(n) => typed_int_literal(n),
        Expr::DecimalLiteral(spelling) => {
            let value = parse_decimal_literal(spelling)?;
            Ok(TypedExpr {
                expr: ExprKind::DecimalLiteral {
                    value,
                    spelling: spelling.clone(),
                },
                ty: Type::Decimal,
            })
        }
        Expr::Bool(b) => Ok(TypedExpr {
            expr: ExprKind::Bool(*b),
            ty: Type::Bool,
        }),
        Expr::String(s) => Ok(TypedExpr {
            expr: ExprKind::String(s.clone()),
            ty: Type::String,
        }),
        Expr::Bytes(bytes) => Ok(TypedExpr {
            expr: ExprKind::Bytes(bytes.clone()),
            ty: Type::Bytes,
        }),
        Expr::Ident(name) => {
            if let Some((target, binding_ty)) = context.validate_value_target(expr, name, vars)? {
                use crate::resolved::ResolvedValueTarget;
                return match target {
                    ResolvedValueTarget::Binding(_) => Ok(TypedExpr {
                        expr: ExprKind::Ident(name.clone()),
                        ty: binding_ty.expect("binding target supplies its semantic type"),
                    }),
                    ResolvedValueTarget::State(_) | ResolvedValueTarget::ExternalState => vars
                        .get(name)
                        .cloned()
                        .map(|ty| TypedExpr {
                            expr: ExprKind::Ident(name.clone()),
                            ty,
                        })
                        .ok_or_else(|| SemanticError {
                            code: "E_INTERNAL_RESOLUTION",
                            message: format!(
                                "resolved state `{name}` is absent from the typed environment"
                            ),
                        }),
                    ResolvedValueTarget::Const(_) | ResolvedValueTarget::ExternalConst => context
                        .consts
                        .borrow()
                        .get(name)
                        .cloned()
                        .ok_or_else(|| SemanticError {
                            code: "E_INTERNAL_RESOLUTION",
                            message: format!(
                                "resolved const `{name}` is absent from the typed environment"
                            ),
                        }),
                    ResolvedValueTarget::ErrorCode(resolved_code) => {
                        let code = context.error_codes.borrow().get(name).copied().ok_or_else(|| {
                            SemanticError {
                                code: "E_INTERNAL_RESOLUTION",
                                message: format!(
                                    "resolved error code `{name}` is absent from the typed environment"
                                ),
                            }
                        })?;
                        if code != resolved_code {
                            return Err(SemanticError {
                                code: "E_INTERNAL_RESOLUTION",
                                message: format!("error code `{name}` changed after resolution"),
                            });
                        }
                        Ok(TypedExpr {
                            expr: ExprKind::IntLiteral(BigInt::from(code)),
                            ty: Type::Int,
                        })
                    }
                    ResolvedValueTarget::Intrinsic => Err(SemanticError {
                        code: "E_INTRINSIC_CONTEXT",
                        message: format!(
                            "intrinsic value `{name}` is only valid in its declared operation context"
                        ),
                    }),
                };
            }
            if let Some(ty) = vars.get(name).cloned() {
                return Ok(TypedExpr {
                    expr: ExprKind::Ident(name.clone()),
                    ty,
                });
            }
            if let Some(value) = context.consts.borrow().get(name).cloned() {
                return Ok(value);
            }
            if let Some(code) = context.error_codes.borrow().get(name).copied() {
                return Ok(TypedExpr {
                    expr: ExprKind::IntLiteral(BigInt::from(code)),
                    ty: Type::Int,
                });
            }
            Err(SemanticError {
                code: "K2002",
                message: format!("undefined variable {name}"),
            })
        }
        Expr::Unary { op, expr: inner } => {
            // A quantity cannot be negated. Keep the operand in its literal
            // domain so the enclosing contextual conversion reports the
            // stable `E_NEGATIVE_QUANTITY` diagnostic for `quantity = -1`
            // instead of treating this as a runtime quantity negation.
            let unary_expected =
                expected.filter(|expected| resolve_struct_type(expected) != Type::Quantity);
            let inner_t = analyze_expr_expected(context, inner, vars, unary_expected)?;
            crate::secret::reject_secret_ordinary_operation(&[&inner_t])?;
            match op {
                UnaryOp::Neg => {
                    let Some(kind) = numeric_kind(&inner_t.ty) else {
                        return Err(SemanticError {
                            code: "K2003",
                            message: "unary '-' expects numeric".into(),
                        });
                    };
                    if !matches!(kind, NumericKind::Int | NumericKind::Decimal) {
                        return Err(SemanticError {
                            code: "E_QUANTITY_NEGATION",
                            message: "unary `-` is supported for int and decimal; quantity is non-negative"
                                .into(),
                        });
                    }
                    let typed = TypedExpr {
                        expr: ExprKind::Unary {
                            op: *op,
                            expr: Box::new(inner_t),
                        },
                        ty: numeric_kind_to_type(kind),
                    };
                    match crate::checked_arithmetic::evaluate(&typed) {
                        Ok(Some(value)) => Ok(value.into_typed_expr()),
                        Ok(None) => Ok(typed),
                        Err(error) => Err(SemanticError {
                            code: error.code(),
                            message: error.to_string(),
                        }),
                    }
                }
                UnaryOp::Not => {
                    if inner_t.ty != Type::Bool {
                        return Err(SemanticError {
                            code: "K2003",
                            message: "unary '!' expects bool".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Unary {
                            op: *op,
                            expr: Box::new(inner_t.clone()),
                        },
                        ty: Type::Bool,
                    })
                }
            }
        }
        Expr::Member { object, field } => {
            let mut obj = analyze_expr(context, object, vars)?;
            let resolved_obj_ty = resolve_struct_type_with_context(context, &obj.ty)?;
            obj.ty = resolved_obj_ty.clone();
            // Tuple numeric indexing
            if let Ok(idx) = field.parse::<usize>() {
                match &resolved_obj_ty {
                    Type::Tuple(ts) => {
                        if let Some(t) = ts.get(idx) {
                            return Ok(TypedExpr {
                                expr: ExprKind::Member {
                                    object: Box::new(obj),
                                    field: field.clone(),
                                },
                                ty: resolve_struct_type(t),
                            });
                        } else {
                            return Err(SemanticError {
                                code: "E_TUPLE_INDEX",
                                message: format!(
                                    "tuple index {} out of bounds (len={})",
                                    idx,
                                    ts.len()
                                ),
                            });
                        }
                    }
                    Type::Struct { name, .. } => {
                        return Err(SemanticError {
                            code: "K2002",
                            message: format!(
                                "tuple index on non-tuple type struct {name}; unknown field '{field}' on struct {name}"
                            ),
                        });
                    }
                    other => {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!("tuple index on non-tuple type {}", type_name(other)),
                        });
                    }
                }
            }
            // Named access on a tuple is invalid
            if matches!(&resolved_obj_ty, Type::Tuple(_)) {
                return Err(SemanticError {
                    code: "K2002",
                    message: format!("unknown field '{field}' on tuple"),
                });
            }
            // Struct named field: map to numeric index for lowering
            if let Type::Struct { name, fields } = &resolved_obj_ty {
                if let Some((idx, (_fname, fty))) = fields
                    .iter()
                    .enumerate()
                    .find(|(_, (fname, _))| fname == field)
                {
                    return Ok(TypedExpr {
                        expr: ExprKind::Member {
                            object: Box::new(obj),
                            field: idx.to_string(),
                        },
                        ty: resolve_struct_type(fty),
                    });
                } else {
                    let avail: Vec<&str> = fields.iter().map(|(f, _)| f.as_str()).collect();
                    return Err(SemanticError {
                        code: "K2002",
                        message: format!(
                            "unknown field '{field}' on struct {name} (available: {})",
                            avail.join(", ")
                        ),
                    });
                }
            }
            // Attempt to resolve to a flattened bound variable like `base#i#j` for nested structs.
            let try_flatten = || -> Option<(String, Type)> {
                fn collect_path(e: &TypedExpr, out: &mut Vec<usize>) -> Option<String> {
                    match e.kind() {
                        ExprKind::Member { object, field } => {
                            let base = collect_path(object, out)?;
                            let i = field.parse::<usize>().ok()?;
                            out.push(i);
                            Some(base)
                        }
                        ExprKind::Ident(nm) => Some(nm.clone()),
                        _ => None,
                    }
                }
                let mut path = Vec::new();
                let base = collect_path(
                    &TypedExpr {
                        expr: ExprKind::Member {
                            object: Box::new(obj.clone()),
                            field: field.clone(),
                        },
                        ty: Type::Int,
                    },
                    &mut path,
                )?;
                if path.is_empty() {
                    return None;
                }
                let mut name = base;
                for i in path.into_iter().rev() {
                    name.push('#');
                    name.push_str(&i.to_string());
                }
                // Look up type from vars
                if let Some(ty) = vars.get(&name).cloned() {
                    return Some((name, ty));
                }
                None
            };
            if let Some((n, ty)) = try_flatten() {
                return Ok(TypedExpr {
                    expr: ExprKind::Ident(n),
                    ty,
                });
            }
            Err(SemanticError {
                code: "K2002",
                message: format!("unknown field '{field}' on type {}", type_name(&obj.ty)),
            })
        }
        Expr::Index { target, index } => {
            let tgt = analyze_expr(context, target, vars)?;
            let mut idx = analyze_expr(context, index, vars)?;
            crate::secret::reject_secret_key(&idx)?;
            match tgt.ty.clone() {
                Type::List(element, _) => {
                    let expected_option = expected
                        .map(|ty| resolve_struct_type_with_context(context, ty))
                        .transpose()?
                        .is_some_and(|expected| {
                            matches!(expected, Type::Option(expected) if *expected == *element)
                        });
                    let fix = if expected_option && resolve_struct_type(&idx.ty) == Type::Int {
                        match (
                            context.expression_source(target),
                            context.expression_source(index),
                        ) {
                            (Some(target), Some(index)) => {
                                Some(crate::semantic_diagnostics::SemanticFix::ListGet {
                                    target,
                                    index,
                                })
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    context.capture_expression_diagnostic(expr, fix);
                    Err(SemanticError {
                        code: "E_LIST_UNSAFE_INDEX",
                        message: "unchecked List indexing is not part of Kotodama V1; use `list.get(index)` and handle `Option<T>`"
                            .into(),
                    })
                }
                Type::StateMap(k, _) => {
                    ensure_assignable_and_coerce(&k, &mut idx)?;
                    ensure_in_memory_map_word_types(context, &tgt)?;
                    Err(SemanticError {
                        code: "E_STATE_MAP_OPTIONAL_READ",
                        message: "StateMap rvalue indexing cannot represent an absent key; use `map.get(key)` and handle Option<V>"
                            .into(),
                    })
                }
                _ => Err(SemanticError {
                    code: "K2003",
                    message: "indexing not supported on this type".into(),
                }),
            }
        }
        Expr::Binary { op, left, right } => {
            let mut left_t = analyze_expr(context, left, vars)?;
            let mut right_t = analyze_expr(context, right, vars)?;
            crate::secret::reject_secret_ordinary_operation(&[&left_t, &right_t])?;
            if *op == BinaryOp::Mod
                && (resolve_struct_type(&left_t.ty) == Type::Quantity
                    || resolve_struct_type(&right_t.ty) == Type::Quantity
                    || expected
                        .is_some_and(|expected| resolve_struct_type(expected) == Type::Quantity))
            {
                return Err(SemanticError {
                    code: "E_QUANTITY_REMAINDER",
                    message: "quantity does not support `%`; use exact `/` or quantity.div_round"
                        .into(),
                });
            }
            coerce_contextual_numeric_literals(*op, expected, &mut left_t, &mut right_t)?;
            use BinaryOp::*;
            match op {
                Add | Sub | Mul | Div | Mod => {
                    reject_implicit_int_decimal_mix(&left_t.ty, &right_t.ty)?;
                    let Some(result_ty) = arithmetic_result_type(*op, &left_t.ty, &right_t.ty)
                    else {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "operator {op:?} is not defined for {} and {}",
                                type_name(&left_t.ty),
                                type_name(&right_t.ty),
                            ),
                        });
                    };
                    let typed = TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: result_ty,
                    };
                    match crate::checked_arithmetic::evaluate(&typed) {
                        Ok(Some(value)) => Ok(value.into_typed_expr()),
                        Ok(None) => Ok(typed),
                        Err(error) => Err(SemanticError {
                            code: error.code(),
                            message: error.to_string(),
                        }),
                    }
                }
                And | Or => {
                    if left_t.ty != Type::Bool || right_t.ty != Type::Bool {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!("{op:?} expects bool operands"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: Type::Bool,
                    })
                }
                Eq | Ne => {
                    reject_implicit_int_decimal_mix(&left_t.ty, &right_t.ty)?;
                    let numeric_result = numeric_result_type(&left_t.ty, &right_t.ty);
                    let numeric_ok = numeric_result.is_some();
                    if left_t.ty != right_t.ty
                        && !(is_blob_like(&left_t.ty) && is_blob_like(&right_t.ty))
                        && !numeric_ok
                    {
                        return Err(SemanticError {
                            code: "K2003",
                            message: "type mismatch in equality".into(),
                        });
                    }
                    if !is_eq_comparable_type(&left_t.ty) {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "equality is not supported for type {}",
                                type_name(&left_t.ty)
                            ),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: Type::Bool,
                    })
                }
                Lt | Le | Gt | Ge => {
                    reject_implicit_int_decimal_mix(&left_t.ty, &right_t.ty)?;
                    let Some(_result_ty) = numeric_result_type(&left_t.ty, &right_t.ty) else {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "comparison {op:?} is not defined for {} and {}",
                                type_name(&left_t.ty),
                                type_name(&right_t.ty),
                            ),
                        });
                    };
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: Type::Bool,
                    })
                }
            }
        }
        Expr::StructLiteral { name, fields } => {
            context.validate_struct_literal_target(expr, name)?;
            let Some(declared_fields) = context.structs.borrow().get(name).cloned() else {
                return Err(SemanticError {
                    code: "E_UNKNOWN_STRUCT",
                    message: format!("unknown struct type `{name}`"),
                });
            };
            for (index, field) in fields.iter().enumerate() {
                if fields[..index]
                    .iter()
                    .any(|previous| previous.name == field.name)
                {
                    return Err(SemanticError {
                        code: "E_DUPLICATE_STRUCT_FIELD",
                        message: format!(
                            "struct field `{}` is supplied more than once",
                            field.name
                        ),
                    });
                }
                if !declared_fields
                    .iter()
                    .any(|(declared, _)| declared == &field.name)
                {
                    return Err(SemanticError {
                        code: "E_UNKNOWN_STRUCT_FIELD",
                        message: format!("struct `{name}` has no field named `{}`", field.name),
                    });
                }
            }
            for (declared_name, _) in &declared_fields {
                if !fields.iter().any(|field| &field.name == declared_name) {
                    return Err(SemanticError {
                        code: "E_MISSING_STRUCT_FIELD",
                        message: format!(
                            "struct `{name}` literal is missing field `{declared_name}`"
                        ),
                    });
                }
            }
            let mut typed_fields = Vec::with_capacity(declared_fields.len());
            for field in fields {
                let (declared_name, declared_ty) = declared_fields
                    .iter()
                    .find(|(declared, _)| declared == &field.name)
                    .expect("unknown struct fields were rejected above");
                let mut value =
                    analyze_expr_expected(context, &field.value, vars, Some(declared_ty))?;
                ensure_assignable_and_coerce(declared_ty, &mut value)?;
                typed_fields.push((declared_name.clone(), value));
            }
            Ok(TypedExpr {
                expr: ExprKind::StructLiteral {
                    name: name.clone(),
                    fields: typed_fields,
                },
                ty: Type::Struct {
                    name: name.clone(),
                    fields: Arc::from(declared_fields),
                },
            })
        }
        Expr::Call {
            name,
            args,
            argument_names,
            implicit_receiver,
        } => {
            let source_name = name.clone();
            let name = normalize_namespaced(name);
            context.validate_call_target(expr, &source_name, &name, *implicit_receiver)?;
            if let Some(result) = analyze_list_method_call(
                context,
                &name,
                args,
                argument_names.as_deref(),
                *implicit_receiver,
                vars,
            ) {
                return result;
            }
            if let Some(result) = analyze_decimal_to_int_round_call(
                context,
                &name,
                args,
                argument_names.as_deref(),
                *implicit_receiver,
                vars,
            ) {
                return result;
            }
            if let Some(result) = analyze_numeric_round_method_call(
                context,
                &name,
                args,
                argument_names.as_deref(),
                *implicit_receiver,
                vars,
            ) {
                return result;
            }
            if context.structs.borrow().contains_key(&name) {
                let fields = context
                    .structs
                    .borrow()
                    .get(&name)
                    .map(|fields| {
                        fields
                            .iter()
                            .map(|(field, _)| field.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let field_names = fields.join(", ");
                let arguments = args
                    .iter()
                    .map(|argument| context.expression_source(argument))
                    .collect::<Option<Vec<_>>>();
                let fix = arguments.map(|arguments| {
                    crate::semantic_diagnostics::SemanticFix::PositionalStruct {
                        name: source_name.clone(),
                        fields,
                        arguments,
                    }
                });
                context.capture_expression_diagnostic(expr, fix);
                return Err(SemanticError {
                    code: "E_POSITIONAL_STRUCT",
                    message: format!(
                        "positional construction `{source_name}(...)` is retired; use `{source_name} {{ {field_names} }}` with named fields"
                    ),
                });
            }

            let mut argument_plan = CallArgumentPlan {
                ordered: args.clone(),
                evaluation_order: (0..args.len()).collect(),
                is_named: false,
            };
            if let Some(builtin) = Builtin::from_name(&name) {
                if builtin == Builtin::VrfVerify
                    && (args.len() != 1
                        || argument_names
                            .as_deref()
                            .is_some_and(|names| !names.iter().map(String::as_str).eq(["request"])))
                {
                    return Err(SemanticError {
                        code: "E_RETIRED_VRF_VERIFY_ARGS",
                        message: "the four-register VRF verify form is retired; pass one bytes-encoded VrfVerifyRequest as `request`".into(),
                    });
                }
                let signature = builtin.signature();
                let receiver_count = usize::from(*implicit_receiver);
                let parameter_names = signature
                    .parameter_names
                    .get(receiver_count..)
                    .unwrap_or_default()
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<_>>();
                let required = signature
                    .parameters
                    .get(receiver_count..)
                    .unwrap_or_default()
                    .iter()
                    .map(|parameter| !parameter.ends_with('?'))
                    .collect::<Vec<_>>();
                argument_plan = reorder_call_arguments(
                    &source_name,
                    args,
                    argument_names.as_deref(),
                    *implicit_receiver,
                    &parameter_names,
                    &required,
                    builtin_named_only_reason(builtin, *implicit_receiver),
                )?;
            } else if let Some(signature) = context.function_params.borrow().get(&name).cloned() {
                let receiver_count = usize::from(*implicit_receiver);
                let user_signature = signature.get(receiver_count..).unwrap_or_default();
                let parameter_names = user_signature
                    .iter()
                    .map(|parameter| parameter.name.clone())
                    .collect::<Vec<_>>();
                let required = vec![true; parameter_names.len()];
                let named_only_reason = context
                    .function_named_only_reasons
                    .borrow()
                    .get(&name)
                    .copied()
                    .filter(|_| user_signature.len() >= 3)
                    .or_else(|| {
                        user_parameters_have_confusable_repeats(user_signature).then_some(
                            "repeated parameter types require names to prevent argument transposition",
                        )
                    });
                argument_plan = reorder_call_arguments(
                    &source_name,
                    args,
                    argument_names.as_deref(),
                    *implicit_receiver,
                    &parameter_names,
                    &required,
                    named_only_reason,
                )?;
            } else if argument_names.is_some() {
                let intrinsic_names: &[&str] = match name.as_str() {
                    "option::some"
                    | "result::ok"
                    | "decimal::from_int"
                    | "decimal::to_int_exact"
                    | "quantity::try_from_int"
                    | "quantity::try_from_decimal"
                    | "decimal::from_quantity" => &["value"],
                    "result::err" => &["error"],
                    "unwrap_or" | "unwrap_err_or" => &["default"],
                    _ => &[],
                };
                if !intrinsic_names.is_empty() {
                    let parameter_names = intrinsic_names
                        .iter()
                        .map(|name| (*name).to_owned())
                        .collect::<Vec<_>>();
                    argument_plan = reorder_call_arguments(
                        &source_name,
                        args,
                        argument_names.as_deref(),
                        *implicit_receiver,
                        &parameter_names,
                        &vec![true; parameter_names.len()],
                        None,
                    )?;
                }
            }
            let args = argument_plan.ordered.as_slice();
            if let Some(builtin) = Builtin::from_name(&name)
                && matches!(
                    builtin,
                    Builtin::TestInvokeEntrypoint
                        | Builtin::TestInvokeEntrypointAs
                        | Builtin::TestExpectRejectAs
                        | Builtin::TestActorAccount
                        | Builtin::TestActorPublicKey
                        | Builtin::TestActorSign
                        | Builtin::Poseidon2
                        | Builtin::Poseidon6
                        | Builtin::Pubkgen
                )
                && source_name != builtin.source_name()
            {
                return Err(SemanticError {
                    code: "E_NON_CANONICAL_BUILTIN",
                    message: format!(
                        "legacy or non-canonical builtin spelling `{source_name}` is not supported; use `{}`",
                        builtin.source_name()
                    ),
                });
            }
            if !context.test_builtins_enabled
                && Builtin::from_name(&name).is_some_and(|builtin| {
                    matches!(
                        builtin.spec().mode,
                        BuiltinMode::TestOnly | BuiltinMode::TestFunctionOnly
                    )
                })
            {
                return Err(SemanticError {
                    code: "E_TEST_ONLY_PRODUCTION",
                    message: format!(
                        "builtin `{source_name}` requires explicit compiler test mode"
                    ),
                });
            }
            if name == "require" {
                validate_require_error_variant(context, args)?;
            }
            if name == "invoke_entrypoint" {
                return canonicalize_builtin_result(
                    Builtin::TestInvokeEntrypoint,
                    analyze_invoke_entrypoint_call(context, args, vars),
                )
                .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            if name == "invoke_entrypoint_as" {
                return canonicalize_builtin_result(
                    Builtin::TestInvokeEntrypointAs,
                    analyze_invoke_entrypoint_as_call(context, args, vars),
                )
                .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            if name == "expect_reject_as" {
                return canonicalize_builtin_result(
                    Builtin::TestExpectRejectAs,
                    analyze_expect_reject_as_call(context, args, vars),
                )
                .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            if name == "actor_account" {
                return canonicalize_builtin_result(
                    Builtin::TestActorAccount,
                    analyze_actor_account_call(context, args),
                )
                .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            if name == "actor_public_key" {
                return canonicalize_builtin_result(
                    Builtin::TestActorPublicKey,
                    analyze_actor_public_key_call(context, args),
                )
                .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            if name == "actor_sign" {
                return canonicalize_builtin_result(
                    Builtin::TestActorSign,
                    analyze_actor_sign_call(context, args, vars),
                )
                .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }

            // analyze builtin calls
            let mut typed_slots = (0..args.len()).map(|_| None).collect::<Vec<_>>();
            let expected_parameters = context.function_params.borrow().get(&name).cloned();
            for index in argument_plan.evaluation_order.iter().copied() {
                let argument = &args[index];
                let expected = expected_parameters
                    .as_ref()
                    .and_then(|parameters| parameters.get(index))
                    .map(|parameter| &parameter.ty);
                typed_slots[index] =
                    Some(analyze_expr_expected(context, argument, vars, expected)?);
            }
            let mut arg_typed = typed_slots
                .into_iter()
                .enumerate()
                .map(|(index, argument)| {
                    argument.ok_or_else(|| SemanticError {
                        code: "E_MALFORMED_CALL",
                        message: format!(
                            "call `{source_name}` did not analyze argument slot {index}"
                        ),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !argument_plan.is_named
                && Builtin::from_name(&name).is_some_and(|builtin| {
                    builtin_instantiation_has_confusable_repeats(
                        builtin,
                        &arg_typed,
                        *implicit_receiver,
                    )
                })
            {
                return Err(SemanticError {
                    code: "E_NAMED_ARGUMENTS_REQUIRED",
                    message: format!(
                        "call `{source_name}` requires named arguments because instantiated parameter types repeat and could be transposed"
                    ),
                });
            }
            if let Some(result) = explicit_numeric_conversion(&name, arg_typed.clone()) {
                return result
                    .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            if let Some(result) = analyze_sum_type_call(context, &name, arg_typed.clone()) {
                return result
                    .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            if let Some(builtin) = Builtin::from_name(&name) {
                let canonical_function_call = source_name == builtin.source_name()
                    && matches!(
                        builtin.spec().surface,
                        BuiltinSurface::Function | BuiltinSurface::FunctionOrMethod
                    );
                let canonical_method_call = source_name == builtin.name()
                    && matches!(
                        builtin.spec().surface,
                        BuiltinSurface::MethodOnly | BuiltinSurface::FunctionOrMethod
                    );
                if builtin.spec().mode != BuiltinMode::CompilerInternal
                    && !canonical_function_call
                    && !canonical_method_call
                {
                    return Err(SemanticError {
                        code: "E_NON_CANONICAL_BUILTIN",
                        message: format!(
                            "legacy or non-canonical builtin spelling `{source_name}` is not supported; use `{}`",
                            builtin.source_name()
                        ),
                    });
                }
                return canonicalize_builtin_result(
                    builtin,
                    analyze_surface_builtin_call(context, builtin, arg_typed, expected),
                )
                .map(|typed| retain_named_call_evaluation_order(typed, &argument_plan));
            }
            match name.as_str() {
                "Map::new" => {
                    Err(SemanticError {
                        code: "K2005",
                        message: "ephemeral maps are not part of Kotodama V1; declare durable `StateMap<K, V>` state instead".into(),
                    })
                }
                _ => {
                    let local_runtime_entrypoint = context
                        .function_modifiers
                        .borrow()
                        .get(&name)
                        .is_some_and(|modifiers| modifiers.kind != FunctionKind::Private);
                    let external_runtime_entrypoint = context
                        .external_functions
                        .borrow()
                        .get(&name)
                        .is_some_and(|signature| {
                            function_is_runtime_entrypoint(&signature.modifiers)
                        });
                    if local_runtime_entrypoint || external_runtime_entrypoint {
                        return Err(SemanticError {
                            code: "K2004",
                            message: format!(
                                "seiyaku runtime function `{name}` cannot be called directly; move shared logic into a private `fn` or use the authorized inter-seiyaku call boundary"
                            ),
                        });
                    }
                    let Some(signature) =
                        context.function_params.borrow().get(&name).cloned()
                    else {
                        return Err(SemanticError {
                            code: "K2002",
                            message: format!("unknown function or builtin `{source_name}`"),
                        });
                    };
                    if signature.len() != arg_typed.len() {
                        return Err(SemanticError {
                            code: "K2003",
                            message: format!(
                                "function `{name}` expects {} arguments, got {}",
                                signature.len(),
                                arg_typed.len()
                            ),
                        });
                    }
                    for (arg, param) in arg_typed.iter_mut().zip(signature.iter()) {
                        if param.is_state {
                            if !is_state_handle_expr(context, arg) {
                                return Err(SemanticError {
                                    code: "K2005",
                                    message: format!(
                                        "state parameter `{}` requires a durable state handle argument",
                                        param.name
                                    ),
                                });
                            }
                        } else if is_state_map_expr(context, arg) {
                            return Err(SemanticError {
                                code: "E_STATE_MAP_ALIAS",
                                message:
                                    "state maps cannot be passed to user-defined functions; access declared state directly."
                                        .into(),
                            });
                        }
                        ensure_assignable_and_coerce(&param.ty, arg)?;
                    }
                    let ret_ty = context
                        .function_returns
                        .borrow()
                        .get(&name)
                        .cloned()
                        .expect("declared function parameters and returns are collected together");
                    Ok(retain_named_call_evaluation_order(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: ret_ty,
                    }, &argument_plan))
                }
            }
        }
    }
}

fn analyze_sum_type_call(
    context: &SemanticContext,
    name: &str,
    mut args: Vec<TypedExpr>,
) -> Option<Result<TypedExpr, SemanticError>> {
    let call = |name: &str, args: Vec<TypedExpr>, ty: Type| {
        Ok(TypedExpr {
            expr: ExprKind::Call {
                name: name.to_owned(),
                args,
            },
            ty,
        })
    };
    let error = |message: &str| {
        Err(SemanticError {
            code: "K2003",
            message: message.to_owned(),
        })
    };

    Some(match name {
        STATE_MAP_GET_INTRINSIC => {
            if args.len() != 2 {
                return Some(error("StateMap.get expects exactly one key argument"));
            }
            if !typed_map_expr_is_state(context, &args[0]) {
                return Some(error(
                    "StateMap.get is available only on declared durable state maps",
                ));
            }
            let Type::StateMap(key, value) = resolve_struct_type(&args[0].ty) else {
                return Some(error("StateMap.get receiver must be StateMap<K, V>"));
            };
            debug_assert!(is_supported_durable_value_type(&value));
            if let Err(err) = ensure_assignable_and_coerce(&key, &mut args[1]) {
                return Some(Err(err));
            }
            call(STATE_MAP_GET_INTRINSIC, args, Type::Option(value))
        }
        "option::some" | "option::none" | "result::ok" | "result::err" => Err(SemanticError {
            code: "E_LEGACY_SUM_CONSTRUCTOR",
            message: "lowercase placeholder-based sum constructors are retired; use active-only `Option::some`, contextual `Option::none`, `Result::ok`, or `Result::err`".to_owned(),
        }),
        "is_some" | "is_none" => {
            if args.len() != 1 || !matches!(resolve_struct_type(&args[0].ty), Type::Option(_)) {
                return Some(error(&format!("{name} expects Option<T>")));
            }
            call(name, args, Type::Bool)
        }
        "is_ok" | "is_err" => {
            if args.len() != 1 || !matches!(resolve_struct_type(&args[0].ty), Type::Result(_, _)) {
                return Some(error(&format!("{name} expects Result<T, E>")));
            }
            call(name, args, Type::Bool)
        }
        "unwrap_or" => {
            if args.len() != 2 {
                return Some(error("unwrap_or expects (Option<T>|Result<T, E>, T)"));
            }
            let value_ty = match resolve_struct_type(&args[0].ty) {
                Type::Option(value) | Type::Result(value, _) => *value,
                _ => {
                    return Some(error(
                        "unwrap_or receiver must be Option<T> or Result<T, E>",
                    ));
                }
            };
            if let Err(err) = ensure_assignable_and_coerce(&value_ty, &mut args[1]) {
                return Some(Err(err));
            }
            call("unwrap_or", args, value_ty)
        }
        "unwrap_err_or" => {
            if args.len() != 2 {
                return Some(error("unwrap_err_or expects (Result<T, E>, E)"));
            }
            let Type::Result(_, error_ty) = resolve_struct_type(&args[0].ty) else {
                return Some(error("unwrap_err_or receiver must be Result<T, E>"));
            };
            if let Err(err) = ensure_assignable_and_coerce(&error_ty, &mut args[1]) {
                return Some(Err(err));
            }
            call("unwrap_err_or", args, *error_ty)
        }
        _ => return None,
    })
}

fn is_supported_sum_payload(ty: &Type) -> bool {
    is_supported_durable_value_type(ty)
}

fn parse_declared_type(
    context: &SemanticContext,
    ty: &Option<TypeExpr>,
) -> Result<Option<Type>, SemanticError> {
    let Some(t) = ty else { return Ok(None) };
    let ty = resolve_struct_type_with_context(context, &convert_type_expr(context, t)?)
        .inspect_err(|_| context.capture_diagnostic(context.type_source(t), None))?;
    validate_list_schemas(&ty)?;
    Ok(Some(ty))
}

fn analyze_const_expr(
    context: &SemanticContext,
    expr: &Expr,
    consts: &IndexMap<String, TypedExpr>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    let result = analyze_const_expr_inner(context, expr, consts, expected);
    if result.is_err() {
        context.capture_expression_diagnostic(expr, None);
    }
    result
}

fn analyze_const_expr_inner(
    context: &SemanticContext,
    expr: &Expr,
    consts: &IndexMap<String, TypedExpr>,
    expected: Option<&Type>,
) -> Result<TypedExpr, SemanticError> {
    match expr.kind() {
        Expr::Source { .. } | Expr::Resolved { .. } => {
            unreachable!("kind() strips AST and resolved-HIR provenance wrappers")
        }
        Expr::IntLiteral(n) => typed_int_literal(n),
        Expr::DecimalLiteral(spelling) => {
            let value = parse_decimal_literal(spelling)?;
            Ok(TypedExpr {
                expr: ExprKind::DecimalLiteral {
                    value,
                    spelling: spelling.clone(),
                },
                ty: Type::Decimal,
            })
        }
        Expr::Bool(value) => Ok(TypedExpr {
            expr: ExprKind::Bool(*value),
            ty: Type::Bool,
        }),
        Expr::String(value) => Ok(TypedExpr {
            expr: ExprKind::String(value.clone()),
            ty: Type::String,
        }),
        Expr::Bytes(value) => Ok(TypedExpr {
            expr: ExprKind::Bytes(value.clone()),
            ty: Type::Bytes,
        }),
        Expr::Ident(name) => {
            if let Some((target, _)) = context.validate_value_target(expr, name, &HashMap::new())?
                && !matches!(
                    target,
                    crate::resolved::ResolvedValueTarget::Const(_)
                        | crate::resolved::ResolvedValueTarget::ExternalConst
                )
            {
                return Err(SemanticError {
                    code: "E_INTERNAL_RESOLUTION",
                    message: format!("const initializer `{name}` carries a non-const target"),
                });
            }
            consts.get(name).cloned().ok_or_else(|| SemanticError {
                code: "K2002",
                message: format!(
                    "const `{name}` is undefined or declared after use; constants must be declared before use"
                ),
            })
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => {
            // Preserve the signed literal expression until assignment so a
            // contextual quantity reports `E_NEGATIVE_QUANTITY`.
            let unary_expected =
                expected.filter(|expected| resolve_struct_type(expected) != Type::Quantity);
            let inner = analyze_const_expr(context, inner, consts, unary_expected)?;
            if !matches!(resolve_struct_type(&inner.ty), Type::Int | Type::Decimal) {
                return Err(SemanticError {
                    code: "K2003",
                    message: "const unary `-` expects int or decimal".into(),
                });
            }
            let ty = inner.ty.clone();
            fold_constant_numeric(&TypedExpr {
                expr: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(inner),
                },
                ty,
            })
        }
        Expr::Binary { op, left, right }
            if matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod
            ) =>
        {
            let mut left = analyze_const_expr(context, left, consts, None)?;
            let mut right = analyze_const_expr(context, right, consts, None)?;
            if *op == BinaryOp::Mod
                && (resolve_struct_type(&left.ty) == Type::Quantity
                    || resolve_struct_type(&right.ty) == Type::Quantity
                    || expected
                        .is_some_and(|expected| resolve_struct_type(expected) == Type::Quantity))
            {
                return Err(SemanticError {
                    code: "E_QUANTITY_REMAINDER",
                    message: "quantity does not support `%`; use exact `/` or quantity.div_round"
                        .into(),
                });
            }
            coerce_contextual_numeric_literals(*op, expected, &mut left, &mut right)?;
            reject_implicit_int_decimal_mix(&left.ty, &right.ty)?;
            let result =
                arithmetic_result_type(*op, &left.ty, &right.ty).ok_or_else(|| SemanticError {
                    code: "K2003",
                    message: format!(
                        "operator {op:?} is not defined for {} and {}",
                        type_name(&left.ty),
                        type_name(&right.ty)
                    ),
                })?;
            fold_constant_numeric(&TypedExpr {
                expr: ExprKind::Binary {
                    op: *op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                ty: result,
            })
        }
        _ => Err(SemanticError {
            code: "E_CONST_INITIALIZER",
            message: "const initializers must be literal values or previously declared constants"
                .into(),
        }),
    }
}

fn fold_constant_numeric(expression: &TypedExpr) -> Result<TypedExpr, SemanticError> {
    match crate::checked_arithmetic::evaluate(expression) {
        Ok(Some(value)) => Ok(value.into_typed_expr()),
        Ok(None) => Err(SemanticError {
            code: "E_CONST_INITIALIZER",
            message: "numeric constant depends on a runtime value".into(),
        }),
        Err(error) => Err(SemanticError {
            code: error.code(),
            message: error.to_string(),
        }),
    }
}

fn parse_declared_param_type(
    context: &SemanticContext,
    param: &Param,
    modifiers: &FunctionModifiers,
) -> Result<TypedParam, SemanticError> {
    let ty = convert_type_expr(
        context,
        param.ty.as_ref().ok_or_else(|| SemanticError {
            code: "K2003",
            message: format!("parameter `{}` requires an explicit type", param.name),
        })?,
    )?;
    let ty = resolve_struct_type_with_context(context, &ty).inspect_err(|_| {
        context.capture_diagnostic(
            param.ty.as_ref().and_then(|ty| context.type_source(ty)),
            None,
        );
    })?;
    validate_list_schemas(&ty)?;
    if modifiers.kind != FunctionKind::Private && crate::secret::type_contains_secret(&ty) {
        context.capture_diagnostic(
            param.ty.as_ref().and_then(|ty| context.type_source(ty)),
            None,
        );
        return Err(SemanticError {
            code: "E_SECRET_PUBLIC_PARAMETER",
            message: format!(
                "externally callable function cannot accept secret parameter `{}`; obtain private inputs with `crypto::private_input`",
                param.name
            ),
        });
    }
    if modifiers.kind != FunctionKind::Private && !is_supported_public_argument_type(&ty) {
        return Err(SemanticError {
            code: "K2003",
            message: format!(
                "public parameter `{}` uses unsupported V1 boundary type `{}`",
                param.name,
                type_name(&ty)
            ),
        });
    }
    if param.is_state {
        if modifiers.kind != FunctionKind::Private {
            return Err(SemanticError {
                code: "K2005",
                message: format!(
                    "state parameter `{}` is only supported on internal helper functions",
                    param.name
                ),
            });
        }
        validate_state_type(&ty)?;
    }
    Ok(TypedParam {
        name: param.name.clone(),
        ty,
        is_state: param.is_state,
    })
}

fn convert_type_expr(context: &SemanticContext, ty: &TypeExpr) -> Result<Type, SemanticError> {
    let result = convert_type_expr_inner(context, ty);
    if result.is_err() {
        context.capture_diagnostic(context.type_source(ty), None);
    }
    result
}

fn convert_type_expr_inner(
    context: &SemanticContext,
    ty: &TypeExpr,
) -> Result<Type, SemanticError> {
    let kind = ty.kind();
    let type_node = context.validate_type_node(ty)?;
    if matches!(kind, TypeExpr::Tuple(_) | TypeExpr::Const(_))
        && type_node.as_ref().is_some_and(|node| node.target.is_some())
    {
        return Err(SemanticError {
            code: "E_INTERNAL_RESOLUTION",
            message: "unnamed type expression carries a resolver name target".into(),
        });
    }
    Ok(match kind {
        TypeExpr::Source { .. } | TypeExpr::Resolved { .. } => {
            unreachable!("kind() strips AST and resolved-HIR provenance wrappers")
        }
        TypeExpr::Path(s) => {
            context.validate_named_type_target(type_node.as_ref(), s)?;
            match s.as_str() {
                "int" => Type::Int,
                "decimal" => Type::Decimal,
                "quantity" => Type::Quantity,
                "bool" => Type::Bool,
                "string" => Type::String,
                "bytes" => Type::Bytes,
                "DataSpaceId" => Type::DataSpaceId,
                // Recognize common Iroha types by name
                "AccountId" => Type::AccountId,
                "AssetDefinitionId" => Type::AssetDefinitionId,
                "AssetId" => Type::AssetId,
                "DomainId" => Type::DomainId,
                "Name" => Type::Name,
                "Json" => Type::Json,
                "NftId" => Type::NftId,
                "AccountView" => core_query_view_type(Builtin::QueryGetAccount)
                    .expect("account core query has a declared view"),
                "AssetView" => core_query_view_type(Builtin::QueryGetAsset)
                    .expect("asset core query has a declared view"),
                "AssetDefinitionView" => core_query_view_type(Builtin::QueryGetAssetDefinition)
                    .expect("asset definition core query has a declared view"),
                "DomainView" => core_query_view_type(Builtin::QueryGetDomain)
                    .expect("domain core query has a declared view"),
                "NftView" => core_query_view_type(Builtin::QueryGetNft)
                    .expect("NFT core query has a declared view"),
                other => {
                    let is_declared_struct = context.structs.borrow().contains_key(other);
                    if !is_declared_struct {
                        return Err(SemanticError {
                            code: "K2002",
                            message: format!("unknown type `{other}`"),
                        });
                    }
                    Type::NamedStruct(other.to_string())
                }
            }
        }
        TypeExpr::Generic { base, args } => {
            context.validate_named_type_target(type_node.as_ref(), base)?;
            if base == "StateMap" {
                if args.len() != 2 {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "StateMap expects two type parameters".into(),
                    });
                }
                let k = convert_type_expr(context, &args[0])?;
                let v = convert_type_expr(context, &args[1])?;
                Type::StateMap(Box::new(k), Box::new(v))
            } else if base == "Secret" {
                if !context.zk_enabled {
                    return Err(SemanticError {
                        code: "E_SECRET_REQUIRES_ZK",
                        message:
                            "Secret<T> is available only when compiler build configuration enables ZK mode"
                                .into(),
                    });
                }
                if args.len() != 1 {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "Secret expects one type parameter".into(),
                    });
                }
                let inner = convert_type_expr(context, &args[0])?;
                if !matches!(inner, Type::Int | Type::Decimal | Type::Quantity) {
                    return Err(SemanticError {
                        code: "E_SECRET_PAYLOAD_TYPE",
                        message: format!(
                            "Secret<{}> is unsupported; the V1 private-input ABI supplies Secret<int>, Secret<decimal>, and Secret<quantity>",
                            type_name(&inner)
                        ),
                    });
                }
                Type::Secret(Box::new(inner))
            } else if base == "Option" {
                if args.len() != 1 {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "Option expects one type parameter".into(),
                    });
                }
                Type::Option(Box::new(convert_type_expr(context, &args[0])?))
            } else if base == "Result" {
                if args.len() != 2 {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "Result expects two type parameters".into(),
                    });
                }
                Type::Result(
                    Box::new(convert_type_expr(context, &args[0])?),
                    Box::new(convert_type_expr(context, &args[1])?),
                )
            } else if base == "List" {
                if args.len() != 2 {
                    return Err(SemanticError {
                        code: "E_LIST_TYPE_ARITY",
                        message: "List expects an element type and capacity".into(),
                    });
                }
                let element = convert_type_expr(context, &args[0])?;
                let capacity_node = context.validate_type_node(&args[1])?;
                if capacity_node
                    .as_ref()
                    .is_some_and(|node| node.target.is_some())
                {
                    return Err(SemanticError {
                        code: "E_INTERNAL_RESOLUTION",
                        message: "List capacity carries a resolver name target".into(),
                    });
                }
                let TypeExpr::Const(capacity) = args[1].kind() else {
                    return Err(SemanticError {
                        code: "E_LIST_CAPACITY_CONST",
                        message: "List capacity must be an integer constant in 1..=64".into(),
                    });
                };
                let capacity = u8::try_from(*capacity)
                    .ok()
                    .filter(|capacity| (1..=64).contains(capacity))
                    .ok_or_else(|| SemanticError {
                        code: "E_LIST_CAPACITY",
                        message: format!("List capacity {capacity} is outside 1..=64"),
                    })?;
                if list_element_contains_resource_handle(&element) {
                    return Err(SemanticError {
                        code: "E_LIST_RESOURCE_ELEMENT",
                        message: format!(
                            "List elements cannot contain resource handle type `{}`",
                            type_name(&element)
                        ),
                    });
                }
                let list = Type::List(Box::new(element), capacity);
                validate_list_schemas(&list)?;
                list
            } else if base == "QueryPage" {
                if args.len() != 1 {
                    return Err(SemanticError {
                        code: "K2003",
                        message: "QueryPage expects one core query view type parameter".into(),
                    });
                }
                query_page_type(convert_type_expr(context, &args[0])?)?
            } else {
                return Err(SemanticError {
                    code: "K2002",
                    message: format!("unknown generic type `{base}`"),
                });
            }
        }
        TypeExpr::Tuple(elems) => {
            if elems.is_empty() {
                Type::Unit
            } else {
                let mut out = Vec::new();
                for e in elems {
                    out.push(convert_type_expr(context, e)?);
                }
                Type::Tuple(out)
            }
        }
        TypeExpr::Const(value) => {
            return Err(SemanticError {
                code: "E_CONST_CAPACITY_CONTEXT",
                message: format!(
                    "compile-time integer `{value}` is only valid as the capacity in List<T, N>"
                ),
            });
        }
    })
}

fn apply_map_new_type_hint(expr: &mut TypedExpr, hint: &Type) {
    let hint = resolve_struct_type(hint);
    if !matches!(hint, Type::StateMap(_, _)) {
        return;
    }
    if let ExprKind::Call { name, .. } | ExprKind::NamedCall { name, .. } = expr.kind()
        && name == "Map::new"
    {
        expr.ty = hint;
    }
}

fn ensure_assignable(expected: &Type, actual: &Type) -> Result<(), SemanticError> {
    let expected = resolve_struct_type(expected);
    let actual = resolve_struct_type(actual);
    if expected == actual {
        return Ok(());
    }
    match (&expected, &actual) {
        (Type::StateMap(ek, ev), Type::StateMap(ak, av)) => {
            ensure_assignable(ek, ak)?;
            ensure_assignable(ev, av)
        }
        (Type::Option(expected), Type::Option(actual)) => ensure_assignable(expected, actual),
        (Type::Result(expected_ok, expected_err), Type::Result(actual_ok, actual_err)) => {
            ensure_assignable(expected_ok, actual_ok)?;
            ensure_assignable(expected_err, actual_err)
        }
        (
            Type::List(expected_element, expected_capacity),
            Type::List(actual_element, actual_capacity),
        ) if expected_capacity == actual_capacity => {
            ensure_assignable(expected_element, actual_element)
        }
        (Type::Tuple(exp_elems), Type::Tuple(act_elems)) => {
            if exp_elems.len() != act_elems.len() {
                return Err(SemanticError {
                    code: "E_TYPE_ANNOTATION_MISMATCH",
                    message: format!(
                        "type annotation mismatch: expected tuple of length {}, got {}",
                        exp_elems.len(),
                        act_elems.len()
                    ),
                });
            }
            for (e, a) in exp_elems.iter().zip(act_elems.iter()) {
                ensure_assignable(e, a)?;
            }
            Ok(())
        }
        // Forward struct references are nominal until their declarations are
        // expanded; unrelated names must never become assignable.
        (Type::NamedStruct(expected_name), Type::NamedStruct(actual_name))
            if expected_name == actual_name =>
        {
            Ok(())
        }
        _ => Err(SemanticError {
            code: "E_TYPE_ANNOTATION_MISMATCH",
            message: format!(
                "type annotation mismatch: expected {}, got {}",
                type_name(&expected),
                type_name(&actual)
            ),
        }),
    }
}

fn ensure_assignable_and_coerce(
    expected: &Type,
    expr: &mut TypedExpr,
) -> Result<(), SemanticError> {
    if let Err(error) = ensure_assignable(expected, &expr.ty) {
        if resolve_struct_type(expected) == Type::Decimal
            && resolve_struct_type(&expr.ty) == Type::Int
            && int_literal_expression(expr)
        {
            let inner = expr.clone();
            let converted = TypedExpr {
                expr: ExprKind::NumericCast {
                    expr: Box::new(inner),
                },
                ty: Type::Decimal,
            };
            *expr = fold_numeric_literal_cast(converted)?;
            return Ok(());
        }
        if resolve_struct_type(expected) == Type::Quantity
            && matches!(resolve_struct_type(&expr.ty), Type::Int | Type::Decimal)
            && exact_numeric_literal_expression(expr)
        {
            if numeric_literal_is_negative(expr) {
                return Err(SemanticError {
                    code: "E_NEGATIVE_QUANTITY",
                    message: "a contextual quantity literal cannot be negative".into(),
                });
            }
            let inner = expr.clone();
            let converted = TypedExpr {
                expr: ExprKind::NumericCast {
                    expr: Box::new(inner),
                },
                ty: Type::Quantity,
            };
            *expr = fold_numeric_literal_cast(converted)?;
            return Ok(());
        }
        if let ExprKind::Call { name, .. } | ExprKind::NamedCall { name, .. } = expr.kind()
            && let Some(builtin) = Builtin::from_name(name)
            && core_query_view_type(builtin).is_some()
        {
            let query_result = match &expr.ty {
                Type::Option(inner) => match inner.as_ref() {
                    Type::Struct { name, .. } | Type::NamedStruct(name) => {
                        format!("Option<{name}>")
                    }
                    other => format!("Option<{}>", type_name(other)),
                },
                Type::Struct { name, .. } | Type::NamedStruct(name) => name.clone(),
                other => type_name(other),
            };
            return Err(SemanticError {
                code: "E_QUERY_RESULT_TYPE",
                message: format!(
                    "typed core query `{}` returns `{}`, not `{}`; byte-returning compatibility is not part of Kotodama V1",
                    builtin.source_name(),
                    query_result,
                    type_name(expected)
                ),
            });
        }
        return Err(error);
    }
    Ok(())
}

fn fold_numeric_literal_cast(converted: TypedExpr) -> Result<TypedExpr, SemanticError> {
    match crate::checked_arithmetic::evaluate(&converted) {
        Ok(Some(value)) => Ok(value.into_typed_expr()),
        Ok(None) => Err(SemanticError {
            code: "E_INTERNAL_NUMERIC_MATRIX",
            message: "contextual numeric literal unexpectedly depends on a runtime value".into(),
        }),
        Err(error) => Err(SemanticError {
            code: error.code(),
            message: error.to_string(),
        }),
    }
}

fn int_literal_expression(expr: &TypedExpr) -> bool {
    match expr.kind() {
        ExprKind::IntLiteral(_) => true,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => int_literal_expression(expr),
        _ => false,
    }
}

fn exact_numeric_literal_expression(expr: &TypedExpr) -> bool {
    match expr.kind() {
        ExprKind::IntLiteral(_) | ExprKind::DecimalLiteral { .. } => true,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => exact_numeric_literal_expression(expr),
        _ => false,
    }
}

fn assign_op_to_binary(op: AssignOp) -> Option<BinaryOp> {
    match op {
        AssignOp::Set => None,
        AssignOp::Add => Some(BinaryOp::Add),
        AssignOp::Sub => Some(BinaryOp::Sub),
        AssignOp::Mul => Some(BinaryOp::Mul),
        AssignOp::Div => Some(BinaryOp::Div),
        AssignOp::Mod => Some(BinaryOp::Mod),
    }
}

pub(crate) fn resolve_struct_type(ty: &Type) -> Type {
    match ty {
        Type::NamedStruct(_) => ty.clone(),
        Type::StateMap(key, value) => Type::StateMap(
            Box::new(resolve_struct_type(key)),
            Box::new(resolve_struct_type(value)),
        ),
        Type::Option(inner) => Type::Option(Box::new(resolve_struct_type(inner))),
        Type::Result(ok, err) => Type::Result(
            Box::new(resolve_struct_type(ok)),
            Box::new(resolve_struct_type(err)),
        ),
        Type::List(element, capacity) => {
            Type::List(Box::new(resolve_struct_type(element)), *capacity)
        }
        Type::Secret(inner) => Type::Secret(Box::new(resolve_struct_type(inner))),
        Type::Tuple(items) => Type::Tuple(items.iter().map(resolve_struct_type).collect()),
        Type::Struct { .. } => ty.clone(),
        _ => ty.clone(),
    }
}

fn resolve_struct_type_with_context(
    context: &SemanticContext,
    ty: &Type,
) -> Result<Type, SemanticError> {
    if let Type::NamedStruct(name) = ty
        && !context.resolved_named_types.borrow().contains_key(name)
    {
        return Err(SemanticError {
            code: "K2002",
            message: format!("unknown canonical struct type `{name}`"),
        });
    }
    validate_use_site_type_resolution_budget(context, ty)?;
    Ok(materialize_struct_type_with_context(context, ty))
}

fn validate_use_site_type_resolution_budget(
    context: &SemanticContext,
    ty: &Type,
) -> Result<(), SemanticError> {
    let resources = measure_expanded_type(ty, &context.resolved_named_type_resources.borrow());
    if resources.depth > MAX_NESTING_DEPTH {
        return Err(SemanticError {
            code: "K2008",
            message: format!(
                "expanded use-site value type exceeds the V1 nesting limit of {MAX_NESTING_DEPTH} levels"
            ),
        });
    }
    if resources.nodes > MAX_EXPANDED_TYPE_NODES {
        return Err(SemanticError {
            code: "K2008",
            message: format!(
                "expanded use-site value type exceeds the V1 resource limit of {MAX_EXPANDED_TYPE_NODES} type nodes"
            ),
        });
    }
    Ok(())
}

fn materialize_struct_type_with_context(context: &SemanticContext, ty: &Type) -> Type {
    match ty {
        Type::NamedStruct(name) => context
            .resolved_named_types
            .borrow()
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Type::StateMap(key, value) => Type::StateMap(
            Box::new(materialize_struct_type_with_context(context, key)),
            Box::new(materialize_struct_type_with_context(context, value)),
        ),
        Type::Option(inner) => Type::Option(Box::new(materialize_struct_type_with_context(
            context, inner,
        ))),
        Type::Result(ok, err) => Type::Result(
            Box::new(materialize_struct_type_with_context(context, ok)),
            Box::new(materialize_struct_type_with_context(context, err)),
        ),
        Type::List(element, capacity) => Type::List(
            Box::new(materialize_struct_type_with_context(context, element)),
            *capacity,
        ),
        Type::Secret(inner) => Type::Secret(Box::new(materialize_struct_type_with_context(
            context, inner,
        ))),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| materialize_struct_type_with_context(context, item))
                .collect(),
        ),
        Type::Struct { .. } => ty.clone(),
        _ => ty.clone(),
    }
}

fn normalize_namespaced(name: &str) -> String {
    if let Some(builtin) = Builtin::from_source_name(name) {
        return builtin.name().to_owned();
    }
    String::from(name)
}

fn block_has_return_value(block: &super::ast::Block) -> bool {
    block.statements.iter().any(stmt_has_return_value)
}

fn stmt_has_return_value(stmt: &super::ast::Statement) -> bool {
    use super::ast::Statement as S;
    match stmt.kind() {
        S::Return(Some(_)) => true,
        S::If {
            then_branch,
            else_branch,
            ..
        } => {
            block_has_return_value(then_branch)
                || else_branch
                    .as_ref()
                    .map(block_has_return_value)
                    .unwrap_or(false)
        }
        S::While { body, .. } => block_has_return_value(body),
        S::For { body, .. } => block_has_return_value(body),
        S::ForEachMap { body, .. } => block_has_return_value(body),
        _ => false,
    }
}

// NOTE: `TypedProgram` is defined earlier in this file with seiyaku metadata.

#[derive(Clone, Debug, PartialEq)]
pub enum TypedItem {
    Function(TypedFunction),
}

#[derive(Debug, PartialEq, Clone)]
pub struct TypedTrigger {
    pub id: TriggerId,
    pub call: TriggerCall,
    pub filter: EventFilterBox,
    pub repeats: Repeats,
    pub authority: Option<AccountId>,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedFunction {
    pub name: String,
    pub params: Vec<String>,
    pub param_types: Vec<TypedParam>,
    pub body: TypedBlock,
    pub ret_ty: Option<Type>,
    pub modifiers: FunctionModifiers,
    pub location: super::ast::SourceLocation,
    /// Exact complete function declaration range, when source-backed.
    pub source: Option<crate::source::SourceRange>,
    /// Exact declared function/lifecycle name range, when source-backed.
    pub name_source: Option<crate::source::SourceRange>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedBlock {
    pub statements: Vec<TypedStatement>,
    /// Final expression without a semicolon.
    pub tail: Option<Box<TypedExpr>>,
}

/// Source operation that supplied a compiler-proven StateMap iteration bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateMapIterationBoundKind {
    /// Half-open `.range(start, end)` with a literal span.
    Range,
    /// Prefix `.take(count)` with a literal count.
    Take,
}

impl StateMapIterationBoundKind {
    /// Canonical manifest spelling retained from the source operation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Range => "range",
            Self::Take => "take",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedStatement {
    Let {
        name: String,
        value: TypedExpr,
    },
    Expr(TypedExpr),
    Return(Option<TypedExpr>),
    Break,
    Continue,
    If {
        cond: TypedExpr,
        then_branch: TypedBlock,
        else_branch: Option<TypedBlock>,
    },
    /// Statement `if let`; unlike the expression form, `else` may be absent.
    IfLet {
        pattern: TypedSumPattern,
        value: TypedExpr,
        then_branch: TypedBlock,
        else_branch: Option<TypedBlock>,
    },
    While {
        cond: TypedExpr,
        body: TypedBlock,
    },
    For {
        line: usize,
        init: Option<Box<TypedStatement>>,
        cond: Option<TypedExpr>,
        step: Option<Box<TypedStatement>>,
        body: TypedBlock,
    },
    /// For-each over a map. Lowers to a deterministic, bounded iteration.
    ForEachMap {
        key: String,
        value: Option<String>,
        map: TypedExpr,
        body: TypedBlock,
        /// Start offset (in buckets) for iteration; 0 when not specified.
        start: u64,
        /// Optional upper bound on iterations (e.g., from `.take(n)`).
        bound: Option<usize>,
        /// Exact source operation that supplied the static bound.
        bound_kind: StateMapIterationBoundKind,
    },
    /// Map set operation: `map[key] = value`.
    MapSet {
        map: TypedExpr,
        key: TypedExpr,
        value: Box<TypedExpr>,
    },
}

impl TypedExpr {
    /// View the typed expression kind.
    #[must_use]
    pub const fn kind(&self) -> &ExprKind {
        &self.expr
    }

    /// Mutably view the typed expression kind.
    #[must_use]
    pub const fn kind_mut(&mut self) -> &mut ExprKind {
        &mut self.expr
    }
}

impl TypedStatement {
    /// View the typed statement.
    #[must_use]
    pub const fn kind(&self) -> &Self {
        self
    }

    /// Mutably view the typed statement.
    #[must_use]
    pub const fn kind_mut(&mut self) -> &mut Self {
        self
    }
}

/// Return call operands with runtime evaluation semantics.
///
/// Active-only sums use dedicated expression nodes, so ordinary calls always
/// evaluate every argument in source order.
fn evaluated_call_args<'a>(_name: &str, args: &'a [TypedExpr]) -> &'a [TypedExpr] {
    args
}

fn expr_mutates_map(expr: &TypedExpr, map_name: &str) -> bool {
    match expr.kind() {
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
            (matches!(
                Builtin::from_name(name),
                Some(Builtin::Ensure | Builtin::StateMapRemove)
            ) && args
                .first()
                .is_some_and(|map| matches!(map.kind(), ExprKind::Ident(name) if name == map_name)))
                || evaluated_call_args(name, args)
                    .iter()
                    .any(|arg| expr_mutates_map(arg, map_name))
        }
        ExprKind::Binary { left, right, .. } => {
            expr_mutates_map(left, map_name) || expr_mutates_map(right, map_name)
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => expr_mutates_map(expr, map_name),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_mutates_map(cond, map_name)
                || expr_mutates_map(then_expr, map_name)
                || expr_mutates_map(else_expr, map_name)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_mutates_map(condition, map_name)
                || block_mutates_map(then_branch, map_name)
                || block_mutates_map(else_branch, map_name)
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_mutates_map(value, map_name)
                || block_mutates_map(then_branch, map_name)
                || block_mutates_map(else_branch, map_name)
        }
        ExprKind::Match { value, arms } => {
            expr_mutates_map(value, map_name)
                || arms
                    .iter()
                    .any(|arm| block_mutates_map(&arm.body, map_name))
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            items.iter().any(|item| expr_mutates_map(item, map_name))
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            expr_mutates_map(source, map_name)
                || expr_mutates_map(expression, map_name)
                || condition
                    .as_deref()
                    .is_some_and(|condition| expr_mutates_map(condition, map_name))
        }
        ExprKind::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_mutates_map(value, map_name)),
        ExprKind::JsonObject(entries) => entries
            .iter()
            .any(|(_, value)| expr_mutates_map(value, map_name)),
        ExprKind::JsonArray(items) => items.iter().any(|item| expr_mutates_map(item, map_name)),
        ExprKind::Member { object, .. } => expr_mutates_map(object, map_name),
        ExprKind::Index { target, index } => {
            expr_mutates_map(target, map_name) || expr_mutates_map(index, map_name)
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => false,
    }
}

fn block_mutates_map(block: &TypedBlock, map_name: &str) -> bool {
    fn stmt_mutates(stmt: &TypedStatement, map_name: &str) -> bool {
        match stmt.kind() {
            TypedStatement::MapSet { map, .. } => {
                matches!(map.kind(), ExprKind::Ident(n) if n == map_name)
            }
            TypedStatement::Expr(expr)
            | TypedStatement::Return(Some(expr))
            | TypedStatement::Let { value: expr, .. } => expr_mutates_map(expr, map_name),
            TypedStatement::If {
                then_branch,
                else_branch,
                ..
            } => {
                block_mutates_map(then_branch, map_name)
                    || else_branch
                        .as_ref()
                        .map(|b| block_mutates_map(b, map_name))
                        .unwrap_or(false)
            }
            TypedStatement::IfLet {
                value,
                then_branch,
                else_branch,
                ..
            } => {
                expr_mutates_map(value, map_name)
                    || block_mutates_map(then_branch, map_name)
                    || else_branch
                        .as_ref()
                        .is_some_and(|block| block_mutates_map(block, map_name))
            }
            TypedStatement::While { body, .. } => block_mutates_map(body, map_name),
            TypedStatement::For { body, .. } => block_mutates_map(body, map_name),
            TypedStatement::ForEachMap { body, .. } => block_mutates_map(body, map_name),
            _ => false,
        }
    }
    block.statements.iter().any(|s| stmt_mutates(s, map_name))
        || block
            .tail
            .as_ref()
            .is_some_and(|tail| expr_mutates_map(tail, map_name))
}

fn block_contains_host_side_effects(block: &TypedBlock) -> bool {
    block
        .statements
        .iter()
        .any(statement_contains_host_side_effects)
        || block
            .tail
            .as_ref()
            .is_some_and(|tail| expr_contains_host_side_effects(tail))
}

fn block_contains_instruction_emission(block: &TypedBlock) -> bool {
    block
        .statements
        .iter()
        .any(statement_contains_instruction_emission)
        || block
            .tail
            .as_ref()
            .is_some_and(|tail| expr_contains_instruction_emission(tail))
}

fn block_mutates_durable_state(context: &SemanticContext, block: &TypedBlock) -> bool {
    block
        .statements
        .iter()
        .any(|statement| statement_mutates_durable_state(context, statement))
        || block
            .tail
            .as_ref()
            .is_some_and(|tail| expr_mutates_durable_state(context, tail))
}

fn is_state_identifier(context: &SemanticContext, name: &str) -> bool {
    context.states.borrow().contains_key(name)
}

fn is_state_param_name(context: &SemanticContext, name: &str) -> bool {
    context.current_state_param_names.borrow().contains(name)
}

fn is_state_binding(context: &SemanticContext, name: &str) -> bool {
    is_state_identifier(context, name) || is_state_param_name(context, name)
}

fn canonical_state_hint(name: &str) -> String {
    let base = name.split('#').next().unwrap_or(name);
    format!("state:{base}")
}

fn mark_state_read(state_names: &HashSet<String>, name: &str, reads: &mut IndexSet<String>) {
    if state_names.contains(name.split('#').next().unwrap_or(name)) {
        reads.insert(canonical_state_hint(name));
    }
}

fn mark_state_write(state_names: &HashSet<String>, name: &str, writes: &mut IndexSet<String>) {
    if state_names.contains(name.split('#').next().unwrap_or(name)) {
        writes.insert(canonical_state_hint(name));
    }
}

fn collect_state_accesses_block(
    state_names: &HashSet<String>,
    block: &TypedBlock,
    reads: &mut IndexSet<String>,
    writes: &mut IndexSet<String>,
) {
    for stmt in &block.statements {
        collect_state_accesses_statement(state_names, stmt, reads, writes);
    }
    if let Some(tail) = &block.tail {
        collect_state_accesses_expr(state_names, tail, reads, writes);
    }
}

fn collect_state_accesses_statement(
    state_names: &HashSet<String>,
    stmt: &TypedStatement,
    reads: &mut IndexSet<String>,
    writes: &mut IndexSet<String>,
) {
    match stmt.kind() {
        TypedStatement::Let { name, value } => {
            collect_state_accesses_expr(state_names, value, reads, writes);
            mark_state_write(state_names, name, writes);
        }
        TypedStatement::Expr(expr) => collect_state_accesses_expr(state_names, expr, reads, writes),
        TypedStatement::Return(Some(expr)) => {
            collect_state_accesses_expr(state_names, expr, reads, writes);
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_state_accesses_expr(state_names, cond, reads, writes);
            collect_state_accesses_block(state_names, then_branch, reads, writes);
            if let Some(b) = else_branch {
                collect_state_accesses_block(state_names, b, reads, writes);
            }
        }
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_state_accesses_expr(state_names, value, reads, writes);
            collect_state_accesses_block(state_names, then_branch, reads, writes);
            if let Some(block) = else_branch {
                collect_state_accesses_block(state_names, block, reads, writes);
            }
        }
        TypedStatement::While { cond, body } => {
            collect_state_accesses_expr(state_names, cond, reads, writes);
            collect_state_accesses_block(state_names, body, reads, writes);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init.as_deref() {
                collect_state_accesses_statement(state_names, init_stmt, reads, writes);
            }
            if let Some(cond_expr) = cond {
                collect_state_accesses_expr(state_names, cond_expr, reads, writes);
            }
            if let Some(step_stmt) = step.as_deref() {
                collect_state_accesses_statement(state_names, step_stmt, reads, writes);
            }
            collect_state_accesses_block(state_names, body, reads, writes);
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            collect_state_accesses_expr(state_names, map, reads, writes);
            collect_state_accesses_block(state_names, body, reads, writes);
        }
        TypedStatement::MapSet { map, key, value } => {
            collect_state_accesses_expr(state_names, map, reads, writes);
            collect_state_accesses_expr(state_names, key, reads, writes);
            collect_state_accesses_expr(state_names, value, reads, writes);
            if let ExprKind::Ident(name) = map.kind() {
                mark_state_write(state_names, name, writes);
            }
        }
    }
}

fn collect_state_accesses_expr(
    state_names: &HashSet<String>,
    expr: &TypedExpr,
    reads: &mut IndexSet<String>,
    writes: &mut IndexSet<String>,
) {
    match expr.kind() {
        ExprKind::Ident(name) => mark_state_read(state_names, name, reads),
        ExprKind::Binary { left, right, .. } => {
            collect_state_accesses_expr(state_names, left, reads, writes);
            collect_state_accesses_expr(state_names, right, reads, writes);
        }
        ExprKind::Unary { expr: inner, .. }
        | ExprKind::NumericCast { expr: inner }
        | ExprKind::NumericTryCast { expr: inner }
        | ExprKind::OptionSome { value: inner }
        | ExprKind::ResultOk { value: inner }
        | ExprKind::ResultErr { error: inner }
        | ExprKind::Propagate { value: inner } => {
            collect_state_accesses_expr(state_names, inner, reads, writes)
        }
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_state_accesses_expr(state_names, cond, reads, writes);
            collect_state_accesses_expr(state_names, then_expr, reads, writes);
            collect_state_accesses_expr(state_names, else_expr, reads, writes);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_state_accesses_expr(state_names, condition, reads, writes);
            collect_state_accesses_block(state_names, then_branch, reads, writes);
            collect_state_accesses_block(state_names, else_branch, reads, writes);
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_state_accesses_expr(state_names, value, reads, writes);
            collect_state_accesses_block(state_names, then_branch, reads, writes);
            collect_state_accesses_block(state_names, else_branch, reads, writes);
        }
        ExprKind::Match { value, arms } => {
            collect_state_accesses_expr(state_names, value, reads, writes);
            for arm in arms {
                collect_state_accesses_block(state_names, &arm.body, reads, writes);
            }
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            for item in items {
                collect_state_accesses_expr(state_names, item, reads, writes);
            }
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            collect_state_accesses_expr(state_names, source, reads, writes);
            collect_state_accesses_expr(state_names, expression, reads, writes);
            if let Some(condition) = condition {
                collect_state_accesses_expr(state_names, condition, reads, writes);
            }
        }
        ExprKind::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_state_accesses_expr(state_names, value, reads, writes);
            }
        }
        ExprKind::JsonObject(entries) => {
            for (_, value) in entries {
                collect_state_accesses_expr(state_names, value, reads, writes);
            }
        }
        ExprKind::JsonArray(items) => {
            for item in items {
                collect_state_accesses_expr(state_names, item, reads, writes);
            }
        }
        ExprKind::Member { object, .. } => {
            collect_state_accesses_expr(state_names, object, reads, writes)
        }
        ExprKind::Index { target, index } => {
            collect_state_accesses_expr(state_names, target, reads, writes);
            collect_state_accesses_expr(state_names, index, reads, writes);
        }
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
            if matches!(
                Builtin::from_name(name),
                Some(Builtin::Ensure | Builtin::StateMapRemove)
            ) && let Some(map_name) = args.first().and_then(|argument| match argument.kind() {
                ExprKind::Ident(map_name) => Some(map_name),
                _ => None,
            }) {
                mark_state_write(state_names, map_name, writes);
            }
            for arg in evaluated_call_args(name, args) {
                collect_state_accesses_expr(state_names, arg, reads, writes);
            }
        }
        ExprKind::Bool(_)
        | ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::String(_)
        | ExprKind::Bytes(_) => {}
    }
}

pub fn function_state_accesses(
    func: &TypedFunction,
    states: &[TypedStateDecl],
) -> (IndexSet<String>, IndexSet<String>) {
    let state_names = states
        .iter()
        .map(|state| state.name.clone())
        .collect::<HashSet<_>>();
    let mut reads = IndexSet::new();
    let mut writes = IndexSet::new();
    collect_state_accesses_block(&state_names, &func.body, &mut reads, &mut writes);
    (reads, writes)
}

fn collect_called_functions(context: &SemanticContext, block: &TypedBlock) -> IndexSet<String> {
    let mut calls = IndexSet::new();
    collect_called_functions_into(context, block, &mut calls);
    calls
}

fn collect_called_functions_into(
    context: &SemanticContext,
    block: &TypedBlock,
    calls: &mut IndexSet<String>,
) {
    for stmt in &block.statements {
        collect_calls_in_statement(context, stmt, calls);
    }
    if let Some(tail) = &block.tail {
        collect_calls_in_expr(context, tail, calls);
    }
}

fn collect_calls_in_statement(
    context: &SemanticContext,
    stmt: &TypedStatement,
    calls: &mut IndexSet<String>,
) {
    match stmt.kind() {
        TypedStatement::Let { value, .. } => collect_calls_in_expr(context, value, calls),
        TypedStatement::Expr(expr) => collect_calls_in_expr(context, expr, calls),
        TypedStatement::Return(Some(expr)) => collect_calls_in_expr(context, expr, calls),
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_calls_in_expr(context, cond, calls);
            collect_called_functions_into(context, then_branch, calls);
            if let Some(b) = else_branch {
                collect_called_functions_into(context, b, calls);
            }
        }
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_calls_in_expr(context, value, calls);
            collect_called_functions_into(context, then_branch, calls);
            if let Some(block) = else_branch {
                collect_called_functions_into(context, block, calls);
            }
        }
        TypedStatement::While { cond, body } => {
            collect_calls_in_expr(context, cond, calls);
            collect_called_functions_into(context, body, calls);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init.as_deref() {
                collect_calls_in_statement(context, init_stmt, calls);
            }
            if let Some(cond_expr) = cond {
                collect_calls_in_expr(context, cond_expr, calls);
            }
            if let Some(step_stmt) = step.as_deref() {
                collect_calls_in_statement(context, step_stmt, calls);
            }
            collect_called_functions_into(context, body, calls);
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            collect_calls_in_expr(context, map, calls);
            collect_called_functions_into(context, body, calls);
        }
        TypedStatement::MapSet { map, key, value } => {
            collect_calls_in_expr(context, map, calls);
            collect_calls_in_expr(context, key, calls);
            collect_calls_in_expr(context, value, calls);
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
    }
}

fn collect_calls_in_expr(
    context: &SemanticContext,
    expr: &TypedExpr,
    calls: &mut IndexSet<String>,
) {
    match expr.kind() {
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
            if is_user_defined_function(context, name) {
                calls.insert(name.clone());
            }
            for arg in evaluated_call_args(name, args) {
                collect_calls_in_expr(context, arg, calls);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_calls_in_expr(context, left, calls);
            collect_calls_in_expr(context, right, calls);
        }
        ExprKind::Unary { expr: inner, .. }
        | ExprKind::NumericCast { expr: inner }
        | ExprKind::NumericTryCast { expr: inner }
        | ExprKind::OptionSome { value: inner }
        | ExprKind::ResultOk { value: inner }
        | ExprKind::ResultErr { error: inner }
        | ExprKind::Propagate { value: inner } => collect_calls_in_expr(context, inner, calls),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_calls_in_expr(context, cond, calls);
            collect_calls_in_expr(context, then_expr, calls);
            collect_calls_in_expr(context, else_expr, calls);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_calls_in_expr(context, condition, calls);
            collect_called_functions_into(context, then_branch, calls);
            collect_called_functions_into(context, else_branch, calls);
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_calls_in_expr(context, value, calls);
            collect_called_functions_into(context, then_branch, calls);
            collect_called_functions_into(context, else_branch, calls);
        }
        ExprKind::Match { value, arms } => {
            collect_calls_in_expr(context, value, calls);
            for arm in arms {
                collect_called_functions_into(context, &arm.body, calls);
            }
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            for item in items {
                collect_calls_in_expr(context, item, calls);
            }
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            collect_calls_in_expr(context, source, calls);
            collect_calls_in_expr(context, expression, calls);
            if let Some(condition) = condition {
                collect_calls_in_expr(context, condition, calls);
            }
        }
        ExprKind::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_calls_in_expr(context, value, calls);
            }
        }
        ExprKind::JsonObject(entries) => {
            for (_, value) in entries {
                collect_calls_in_expr(context, value, calls);
            }
        }
        ExprKind::JsonArray(items) => {
            for item in items {
                collect_calls_in_expr(context, item, calls);
            }
        }
        ExprKind::Member { object, .. } => collect_calls_in_expr(context, object, calls),
        ExprKind::Index { target, index } => {
            collect_calls_in_expr(context, target, calls);
            collect_calls_in_expr(context, index, calls);
        }
        ExprKind::Bool(_)
        | ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => {}
    }
}

fn ensure_state_map_iter_supported(
    _context: &SemanticContext,
    _map_expr: &TypedExpr,
) -> Result<(), SemanticError> {
    Ok(())
}

fn ensure_not_state_shadow(context: &SemanticContext, name: &str) -> Result<(), SemanticError> {
    if is_state_binding(context, name) {
        return Err(SemanticError {
            code: "E_STATE_SHADOWED",
            message: format!("`{name}` shadows a state declaration"),
        });
    }
    Ok(())
}

fn ensure_new_local_binding(
    context: &SemanticContext,
    name: &str,
    vars: &HashMap<String, Type>,
) -> Result<(), SemanticError> {
    ensure_not_state_shadow(context, name)?;
    if vars.contains_key(name) {
        return Err(SemanticError {
            code: "K2001",
            message: format!("local binding `{name}` duplicates or shadows an existing binding"),
        });
    }
    if context.consts.borrow().contains_key(name) {
        return Err(SemanticError {
            code: "K2001",
            message: format!("local binding `{name}` shadows a const declaration"),
        });
    }
    if context.global_declarations.borrow().contains(name) {
        return Err(SemanticError {
            code: "K2001",
            message: format!("local binding `{name}` shadows a source declaration"),
        });
    }
    Ok(())
}

fn ensure_mutable_assignment_target(
    context: &SemanticContext,
    name: &str,
    mutable_bindings: &HashSet<String>,
) -> Result<(), SemanticError> {
    if is_state_binding(context, name) || mutable_bindings.contains(name) {
        return Ok(());
    }
    Err(SemanticError {
        code: "E_IMMUTABLE_ASSIGNMENT",
        message: format!(
            "cannot assign to immutable binding `{name}`; declare a mutable local with `var`"
        ),
    })
}

fn is_state_map_expr(context: &SemanticContext, expr: &TypedExpr) -> bool {
    matches!(resolve_struct_type(&expr.ty), Type::StateMap(_, _))
        && typed_map_expr_is_state(context, expr)
}

/// Return the syntactic root name of a typed state-handle expression.
///
/// Callers must validate that the returned root belongs to the current typed
/// program; this helper deliberately carries no process-global environment.
pub fn typed_state_handle_name(expr: &TypedExpr) -> Option<String> {
    match expr.kind() {
        ExprKind::Ident(name) => Some(name.clone()),
        ExprKind::Member { object, field } => {
            let base = typed_state_handle_name(object)?;
            let idx = field.parse::<usize>().ok()?;
            Some(format!("{base}#{idx}"))
        }
        _ => None,
    }
}

fn is_state_handle_expr(context: &SemanticContext, expr: &TypedExpr) -> bool {
    typed_state_handle_name(expr)
        .as_deref()
        .is_some_and(|name| is_state_binding(context, name.split('#').next().unwrap_or(name)))
}

fn typed_map_expr_is_state(context: &SemanticContext, expr: &TypedExpr) -> bool {
    is_state_handle_expr(context, expr)
}

fn map_expr_is_state(context: &SemanticContext, expr: &Expr) -> bool {
    match expr {
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            map_expr_is_state(context, expression)
        }
        Expr::Ident(name) => is_state_binding(context, name),
        Expr::Member { object, .. } => map_expr_is_state(context, object),
        _ => false,
    }
}

fn is_user_defined_function(context: &SemanticContext, name: &str) -> bool {
    context.function_returns.borrow().contains_key(name)
}

fn compute_transitive_effects(
    summaries: &HashMap<String, FunctionSummary>,
) -> HashMap<String, FunctionEffects> {
    let mut effects = summaries
        .iter()
        .map(|(name, summary)| (name.clone(), summary.direct_effects))
        .collect::<HashMap<_, _>>();
    let mut changed = true;
    while changed {
        changed = false;
        for (name, summary) in summaries {
            let mut aggregate = summary.direct_effects;
            for callee in &summary.calls {
                if let Some(callee_effects) = effects.get(callee).copied() {
                    aggregate.merge_from(callee_effects);
                }
            }
            let slot = effects.entry(name.clone()).or_default();
            changed |= slot.merge_from(aggregate);
        }
    }
    effects
}

fn describe_view_violation(effect: FunctionEffects) -> &'static str {
    if effect.mutates_durable_state {
        "durable state mutation"
    } else if effect.emits_instructions {
        "instruction emission"
    } else {
        "host side effects"
    }
}

fn find_first_view_violation(
    root: &str,
    summaries: &HashMap<String, FunctionSummary>,
    effects: &HashMap<String, FunctionEffects>,
) -> Option<(String, &'static str)> {
    let mut visited = HashSet::new();
    let mut stack = vec![root.to_owned()];
    while let Some(name) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let summary = summaries.get(&name)?;
        if summary.direct_effects.forbids_view() {
            return Some((name, describe_view_violation(summary.direct_effects)));
        }
        let mut callees = summary
            .calls
            .iter()
            .filter(|callee| {
                effects
                    .get(*callee)
                    .copied()
                    .is_some_and(FunctionEffects::forbids_view)
            })
            .cloned()
            .collect::<Vec<_>>();
        callees.reverse();
        stack.extend(callees);
    }
    None
}

fn validate_scalar_state_initialization(
    context: &SemanticContext,
    items: &[TypedItem],
    states: &[TypedStateDecl],
) -> Result<(), SemanticError> {
    // This check is intentionally separate from `function_state_accesses`.
    // Access metadata is a may-analysis (union), whereas initialization is a
    // must-analysis (intersection across every normal control-flow exit).
    // Recheck the call graph here so this security property fails closed even
    // if a future caller invokes it without the ordinary semantic pipeline.
    validate_acyclic_function_calls(context)?;

    let required = states
        .iter()
        .filter(|state| !matches!(&state.ty, Type::StateMap(_, _)))
        .map(|state| state.name.clone())
        .collect::<IndexSet<_>>();
    if required.is_empty() {
        return Ok(());
    }

    let functions = items
        .iter()
        .map(|item| match item {
            TypedItem::Function(function) => function,
        })
        .collect::<Vec<_>>();

    let hajimari = functions
        .iter()
        .find(|function| function.modifiers.kind == FunctionKind::Hajimari)
        .ok_or_else(|| SemanticError {
            code: "E_STATE_HAJIMARI_REQUIRED",
            message: "seiyaku scalar state requires a `hajimari()`/`始まり()` declaration".into(),
        })?;

    let required_set = required.iter().cloned().collect::<HashSet<_>>();
    let summaries = compute_definite_state_write_summaries(&functions, &required_set)?;
    let initialized = summaries.get(&hajimari.name).cloned().unwrap_or_default();
    let missing = required
        .iter()
        .filter(|state| !initialized.contains(*state))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(SemanticError {
            code: "E_STATE_HAJIMARI_INCOMPLETE",
            message: format!(
                "hajimari() must initialize every scalar state on every normal return or fallthrough path; missing: {}",
                missing.join(", ")
            ),
        })
    }
}

type DefiniteStateSet = HashSet<String>;

/// Must-analysis state for one block.
///
/// Every populated exit set is the intersection of initialized states across
/// all paths taking that exit kind. `None` means that exit is unreachable;
/// `Some(empty)` means it is reachable with no proven initialized state.
#[derive(Debug, Default)]
struct DefiniteInitFlow {
    continuing: Option<DefiniteStateSet>,
    returns: Option<DefiniteStateSet>,
    breaks: Option<DefiniteStateSet>,
    continues: Option<DefiniteStateSet>,
}

fn intersect_states(left: &mut DefiniteStateSet, right: &DefiniteStateSet) {
    left.retain(|state| right.contains(state));
}

fn merge_exit(accumulated: &mut Option<DefiniteStateSet>, candidate: Option<DefiniteStateSet>) {
    let Some(candidate) = candidate else {
        return;
    };
    if let Some(accumulated) = accumulated {
        intersect_states(accumulated, &candidate);
    } else {
        *accumulated = Some(candidate);
    }
}

fn merge_alternative_continuations(
    left: Option<DefiniteStateSet>,
    right: Option<DefiniteStateSet>,
) -> Option<DefiniteStateSet> {
    match (left, right) {
        (None, None) => None,
        (Some(state), None) | (None, Some(state)) => Some(state),
        (Some(mut left), Some(right)) => {
            intersect_states(&mut left, &right);
            Some(left)
        }
    }
}

fn evaluate_definite_init_expr(
    expr: &TypedExpr,
    mut initialized: DefiniteStateSet,
    summaries: &HashMap<String, DefiniteStateSet>,
) -> DefiniteStateSet {
    match expr.kind() {
        ExprKind::Binary { op, left, right } => {
            initialized = evaluate_definite_init_expr(left, initialized, summaries);
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                // The RHS of `&&` and `||` is conditional. A write is definite
                // only if it is already present after the always-evaluated LHS.
                let rhs = evaluate_definite_init_expr(right, initialized.clone(), summaries);
                intersect_states(&mut initialized, &rhs);
                initialized
            } else {
                evaluate_definite_init_expr(right, initialized, summaries)
            }
        }
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            let after_cond = evaluate_definite_init_expr(cond, initialized, summaries);
            let mut then_state =
                evaluate_definite_init_expr(then_expr, after_cond.clone(), summaries);
            let else_state = evaluate_definite_init_expr(else_expr, after_cond, summaries);
            intersect_states(&mut then_state, &else_state);
            then_state
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let after_condition = evaluate_definite_init_expr(condition, initialized, summaries);
            let then_flow = analyze_definite_init_block(
                then_branch,
                after_condition.clone(),
                &HashSet::new(),
                summaries,
            );
            let else_flow = analyze_definite_init_block(
                else_branch,
                after_condition,
                &HashSet::new(),
                summaries,
            );
            merge_alternative_continuations(then_flow.continuing, else_flow.continuing)
                .unwrap_or_default()
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            let after_value = evaluate_definite_init_expr(value, initialized, summaries);
            let then_flow = analyze_definite_init_block(
                then_branch,
                after_value.clone(),
                &HashSet::new(),
                summaries,
            );
            let else_flow =
                analyze_definite_init_block(else_branch, after_value, &HashSet::new(), summaries);
            merge_alternative_continuations(then_flow.continuing, else_flow.continuing)
                .unwrap_or_default()
        }
        ExprKind::Match { value, arms } => {
            let after_value = evaluate_definite_init_expr(value, initialized, summaries);
            let mut continuation = None;
            for arm in arms {
                let flow = analyze_definite_init_block(
                    &arm.body,
                    after_value.clone(),
                    &HashSet::new(),
                    summaries,
                );
                continuation = merge_alternative_continuations(continuation, flow.continuing);
            }
            continuation.unwrap_or_default()
        }
        ExprKind::Call { name, args } => {
            // Arguments are evaluated eagerly in source order. The call itself
            // contributes exactly the callee's must-write summary; unknown or
            // external bodies contribute nothing and therefore fail closed.
            for arg in args {
                initialized = evaluate_definite_init_expr(arg, initialized, summaries);
            }
            if let Some(callee_writes) = summaries.get(name) {
                initialized.extend(callee_writes.iter().cloned());
            }
            initialized
        }
        ExprKind::NamedCall {
            name,
            args,
            evaluation_order,
        } => {
            for index in evaluation_order {
                initialized = evaluate_definite_init_expr(&args[*index], initialized, summaries);
            }
            if let Some(callee_writes) = summaries.get(name) {
                initialized.extend(callee_writes.iter().cloned());
            }
            initialized
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            for item in items {
                initialized = evaluate_definite_init_expr(item, initialized, summaries);
            }
            initialized
        }
        ExprKind::ListComprehension { source, .. } => {
            // The source is always evaluated. A bounded source may be empty,
            // so neither the filter nor result expression contributes a
            // definite write.
            evaluate_definite_init_expr(source, initialized, summaries)
        }
        ExprKind::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                initialized = evaluate_definite_init_expr(value, initialized, summaries);
            }
            initialized
        }
        ExprKind::JsonObject(entries) => {
            for (_, value) in entries {
                initialized = evaluate_definite_init_expr(value, initialized, summaries);
            }
            initialized
        }
        ExprKind::JsonArray(items) => {
            for item in items {
                initialized = evaluate_definite_init_expr(item, initialized, summaries);
            }
            initialized
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => {
            evaluate_definite_init_expr(expr, initialized, summaries)
        }
        ExprKind::Member { object, .. } => {
            evaluate_definite_init_expr(object, initialized, summaries)
        }
        ExprKind::Index { target, index } => {
            initialized = evaluate_definite_init_expr(target, initialized, summaries);
            evaluate_definite_init_expr(index, initialized, summaries)
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => initialized,
    }
}

fn analyze_definite_init_block(
    block: &TypedBlock,
    incoming: DefiniteStateSet,
    required: &DefiniteStateSet,
    summaries: &HashMap<String, DefiniteStateSet>,
) -> DefiniteInitFlow {
    let mut flow = DefiniteInitFlow {
        continuing: Some(incoming),
        ..DefiniteInitFlow::default()
    };

    for statement in &block.statements {
        let Some(incoming) = flow.continuing.take() else {
            // Statements following an unconditional control transfer are
            // unreachable and cannot establish a definite write.
            break;
        };
        let statement_flow =
            analyze_definite_init_statement(statement, incoming, required, summaries);
        flow.continuing = statement_flow.continuing;
        merge_exit(&mut flow.returns, statement_flow.returns);
        merge_exit(&mut flow.breaks, statement_flow.breaks);
        merge_exit(&mut flow.continues, statement_flow.continues);
    }

    if let Some(tail) = &block.tail
        && let Some(continuing) = flow.continuing.take()
    {
        flow.continuing = Some(evaluate_definite_init_expr(tail, continuing, summaries));
    }

    flow
}

fn analyze_definite_init_statement(
    statement: &TypedStatement,
    incoming: DefiniteStateSet,
    required: &DefiniteStateSet,
    summaries: &HashMap<String, DefiniteStateSet>,
) -> DefiniteInitFlow {
    match statement.kind() {
        TypedStatement::Let { name, value } => {
            let mut continuing = evaluate_definite_init_expr(value, incoming, summaries);
            let state_name = name.split('#').next().unwrap_or(name);
            if required.contains(state_name) {
                continuing.insert(state_name.to_owned());
            }
            DefiniteInitFlow {
                continuing: Some(continuing),
                ..DefiniteInitFlow::default()
            }
        }
        TypedStatement::Expr(expr) => DefiniteInitFlow {
            continuing: Some(evaluate_definite_init_expr(expr, incoming, summaries)),
            ..DefiniteInitFlow::default()
        },
        TypedStatement::Return(expr) => {
            let returned = expr.as_ref().map_or(incoming.clone(), |expr| {
                evaluate_definite_init_expr(expr, incoming, summaries)
            });
            DefiniteInitFlow {
                returns: Some(returned),
                ..DefiniteInitFlow::default()
            }
        }
        TypedStatement::Break => DefiniteInitFlow {
            breaks: Some(incoming),
            ..DefiniteInitFlow::default()
        },
        TypedStatement::Continue => DefiniteInitFlow {
            continues: Some(incoming),
            ..DefiniteInitFlow::default()
        },
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let after_cond = evaluate_definite_init_expr(cond, incoming, summaries);
            let then_flow =
                analyze_definite_init_block(then_branch, after_cond.clone(), required, summaries);
            let else_flow = if let Some(branch) = else_branch {
                analyze_definite_init_block(branch, after_cond, required, summaries)
            } else {
                DefiniteInitFlow {
                    continuing: Some(after_cond),
                    ..DefiniteInitFlow::default()
                }
            };
            let mut flow = DefiniteInitFlow {
                continuing: merge_alternative_continuations(
                    then_flow.continuing,
                    else_flow.continuing,
                ),
                ..DefiniteInitFlow::default()
            };
            merge_exit(&mut flow.returns, then_flow.returns);
            merge_exit(&mut flow.returns, else_flow.returns);
            merge_exit(&mut flow.breaks, then_flow.breaks);
            merge_exit(&mut flow.breaks, else_flow.breaks);
            merge_exit(&mut flow.continues, then_flow.continues);
            merge_exit(&mut flow.continues, else_flow.continues);
            flow
        }
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            let after_value = evaluate_definite_init_expr(value, incoming, summaries);
            let then_flow =
                analyze_definite_init_block(then_branch, after_value.clone(), required, summaries);
            let else_flow = if let Some(branch) = else_branch {
                analyze_definite_init_block(branch, after_value, required, summaries)
            } else {
                DefiniteInitFlow {
                    continuing: Some(after_value),
                    ..DefiniteInitFlow::default()
                }
            };
            let mut flow = DefiniteInitFlow {
                continuing: merge_alternative_continuations(
                    then_flow.continuing,
                    else_flow.continuing,
                ),
                ..DefiniteInitFlow::default()
            };
            merge_exit(&mut flow.returns, then_flow.returns);
            merge_exit(&mut flow.returns, else_flow.returns);
            merge_exit(&mut flow.breaks, then_flow.breaks);
            merge_exit(&mut flow.breaks, else_flow.breaks);
            merge_exit(&mut flow.continues, then_flow.continues);
            merge_exit(&mut flow.continues, else_flow.continues);
            flow
        }
        TypedStatement::While { cond, body } => {
            let after_cond = evaluate_definite_init_expr(cond, incoming, summaries);
            analyze_may_execute_loop(body, after_cond, None, required, summaries)
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            let mut prefix = if let Some(init) = init.as_deref() {
                analyze_definite_init_statement(init, incoming, required, summaries)
            } else {
                DefiniteInitFlow {
                    continuing: Some(incoming),
                    ..DefiniteInitFlow::default()
                }
            };
            let Some(mut after_prefix) = prefix.continuing.take() else {
                return prefix;
            };
            if let Some(cond) = cond {
                // A C-style loop evaluates its condition once even when its
                // body executes zero times.
                after_prefix = evaluate_definite_init_expr(cond, after_prefix, summaries);
            }
            let mut loop_flow =
                analyze_may_execute_loop(body, after_prefix, step.as_deref(), required, summaries);
            merge_exit(&mut loop_flow.returns, prefix.returns);
            merge_exit(&mut loop_flow.breaks, prefix.breaks);
            merge_exit(&mut loop_flow.continues, prefix.continues);
            loop_flow
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            let after_map = evaluate_definite_init_expr(map, incoming, summaries);
            analyze_may_execute_loop(body, after_map, None, required, summaries)
        }
        TypedStatement::MapSet { map, key, value } => {
            // StateMap roots are deliberately excluded from `required`, but
            // calls nested in their receiver/key/value still execute eagerly.
            let continuing = evaluate_definite_init_expr(map, incoming, summaries);
            let continuing = evaluate_definite_init_expr(key, continuing, summaries);
            let continuing = evaluate_definite_init_expr(value, continuing, summaries);
            DefiniteInitFlow {
                continuing: Some(continuing),
                ..DefiniteInitFlow::default()
            }
        }
    }
}

fn analyze_may_execute_loop(
    body: &TypedBlock,
    before_body: DefiniteStateSet,
    step: Option<&TypedStatement>,
    required: &DefiniteStateSet,
    summaries: &HashMap<String, DefiniteStateSet>,
) -> DefiniteInitFlow {
    // Every V1 loop is treated as possibly executing zero times. Therefore no
    // body or step write can strengthen the normal post-loop state. We still
    // inspect a possible first iteration so `return` paths inside the loop are
    // included in the function's must-analysis.
    let mut body_flow = analyze_definite_init_block(body, before_body.clone(), required, summaries);
    let reaches_step =
        merge_alternative_continuations(body_flow.continuing.take(), body_flow.continues.take());
    if let (Some(step), Some(reaches_step)) = (step, reaches_step) {
        let step_flow = analyze_definite_init_statement(step, reaches_step, required, summaries);
        merge_exit(&mut body_flow.returns, step_flow.returns);
    }

    DefiniteInitFlow {
        continuing: Some(before_body),
        returns: body_flow.returns,
        // `break` exits this loop normally and cannot improve the post-loop
        // state because the zero-iteration path is always present. `continue`
        // remains inside the loop. Both are consumed here.
        breaks: None,
        continues: None,
    }
}

fn definite_writes_on_normal_exit(
    function: &TypedFunction,
    required: &DefiniteStateSet,
    summaries: &HashMap<String, DefiniteStateSet>,
) -> DefiniteStateSet {
    let mut flow =
        analyze_definite_init_block(&function.body, DefiniteStateSet::new(), required, summaries);
    merge_exit(&mut flow.returns, flow.continuing);
    // Top-level break/continue are rejected earlier. If malformed typed HIR
    // reaches this pass, intersecting with an empty set fails closed.
    if flow.breaks.is_some() || flow.continues.is_some() {
        return DefiniteStateSet::new();
    }
    flow.returns.unwrap_or_default()
}

fn compute_definite_state_write_summaries(
    functions: &[&TypedFunction],
    required: &DefiniteStateSet,
) -> Result<HashMap<String, DefiniteStateSet>, SemanticError> {
    let mut summaries = functions
        .iter()
        .map(|function| (function.name.clone(), DefiniteStateSet::new()))
        .collect::<HashMap<_, _>>();

    // The already-validated acyclic call graph has height at most N. Start at
    // the conservative empty summary and iterate to the least fixed point, so
    // calls through arbitrarily ordered helpers are source-order independent.
    for _ in 0..=functions.len() {
        let next = functions
            .iter()
            .map(|function| {
                (
                    function.name.clone(),
                    definite_writes_on_normal_exit(function, required, &summaries),
                )
            })
            .collect::<HashMap<_, _>>();
        if next == summaries {
            return Ok(next);
        }
        summaries = next;
    }

    Err(SemanticError {
        code: "E_STATE_HAJIMARI_INCOMPLETE",
        message: "compiler could not prove complete scalar-state assignment by `hajimari`/`始まり` through the helper call graph"
            .into(),
    })
}

fn enforce_permission_requirements(
    context: &SemanticContext,
    items: &[TypedItem],
) -> Result<(), SemanticError> {
    let summaries = context.function_summaries.borrow().clone();
    let effects = compute_transitive_effects(&summaries);
    for func in items.iter().map(|item| match item {
        TypedItem::Function(func) => func,
    }) {
        if func.modifiers.kind == FunctionKind::View
            && let Some((offender, effect_kind)) =
                find_first_view_violation(&func.name, &summaries, &effects)
        {
            let message = if offender == func.name {
                format!("view function `{}` cannot perform {effect_kind}", func.name)
            } else {
                format!(
                    "view function `{}` cannot call `{offender}` because `{offender}` performs {effect_kind}",
                    func.name
                )
            };
            return Err(SemanticError {
                code: "K2004",
                message,
            });
        }

        if func.modifiers.kind == FunctionKind::Kotoage && func.modifiers.permission.is_none() {
            return Err(SemanticError {
                code: "K2004",
                message: format!(
                    "kotoage function `{}` requires `authorize(\"Permission\")`",
                    func.name
                ),
            });
        }
    }
    Ok(())
}

fn statement_contains_host_side_effects(stmt: &TypedStatement) -> bool {
    match stmt.kind() {
        TypedStatement::Expr(expr)
        | TypedStatement::Return(Some(expr))
        | TypedStatement::Let { value: expr, .. } => expr_contains_host_side_effects(expr),
        TypedStatement::MapSet { map, key, value } => {
            expr_contains_host_side_effects(map)
                || expr_contains_host_side_effects(key)
                || expr_contains_host_side_effects(value)
        }
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_contains_host_side_effects(cond)
                || block_contains_host_side_effects(then_branch)
                || else_branch
                    .as_ref()
                    .map(block_contains_host_side_effects)
                    .unwrap_or(false)
        }
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_host_side_effects(value)
                || block_contains_host_side_effects(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(block_contains_host_side_effects)
        }
        TypedStatement::While { cond, body } => {
            expr_contains_host_side_effects(cond) || block_contains_host_side_effects(body)
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            init.as_deref()
                .map(statement_contains_host_side_effects)
                .unwrap_or(false)
                || cond
                    .as_ref()
                    .map(expr_contains_host_side_effects)
                    .unwrap_or(false)
                || step
                    .as_deref()
                    .map(statement_contains_host_side_effects)
                    .unwrap_or(false)
                || block_contains_host_side_effects(body)
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            expr_contains_host_side_effects(map) || block_contains_host_side_effects(body)
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => false,
    }
}

fn statement_contains_instruction_emission(stmt: &TypedStatement) -> bool {
    match stmt.kind() {
        TypedStatement::Expr(expr)
        | TypedStatement::Return(Some(expr))
        | TypedStatement::Let { value: expr, .. } => expr_contains_instruction_emission(expr),
        TypedStatement::MapSet { map, key, value } => {
            expr_contains_instruction_emission(map)
                || expr_contains_instruction_emission(key)
                || expr_contains_instruction_emission(value)
        }
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_contains_instruction_emission(cond)
                || block_contains_instruction_emission(then_branch)
                || else_branch
                    .as_ref()
                    .map(block_contains_instruction_emission)
                    .unwrap_or(false)
        }
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_instruction_emission(value)
                || block_contains_instruction_emission(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(block_contains_instruction_emission)
        }
        TypedStatement::While { cond, body } => {
            expr_contains_instruction_emission(cond) || block_contains_instruction_emission(body)
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            init.as_deref()
                .map(statement_contains_instruction_emission)
                .unwrap_or(false)
                || cond
                    .as_ref()
                    .map(expr_contains_instruction_emission)
                    .unwrap_or(false)
                || step
                    .as_deref()
                    .map(statement_contains_instruction_emission)
                    .unwrap_or(false)
                || block_contains_instruction_emission(body)
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            expr_contains_instruction_emission(map) || block_contains_instruction_emission(body)
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => false,
    }
}

fn statement_mutates_durable_state(context: &SemanticContext, stmt: &TypedStatement) -> bool {
    match stmt.kind() {
        TypedStatement::Let { name, value } => {
            is_state_binding(context, name) || expr_mutates_durable_state(context, value)
        }
        TypedStatement::Expr(expr) | TypedStatement::Return(Some(expr)) => {
            expr_mutates_durable_state(context, expr)
        }
        TypedStatement::MapSet { map, key, value } => {
            typed_map_expr_is_state(context, map)
                || expr_mutates_durable_state(context, map)
                || expr_mutates_durable_state(context, key)
                || expr_mutates_durable_state(context, value)
        }
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_mutates_durable_state(context, cond)
                || block_mutates_durable_state(context, then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|branch| block_mutates_durable_state(context, branch))
        }
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_mutates_durable_state(context, value)
                || block_mutates_durable_state(context, then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|branch| block_mutates_durable_state(context, branch))
        }
        TypedStatement::While { cond, body } => {
            expr_mutates_durable_state(context, cond) || block_mutates_durable_state(context, body)
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            init.as_deref()
                .is_some_and(|statement| statement_mutates_durable_state(context, statement))
                || cond
                    .as_ref()
                    .is_some_and(|expr| expr_mutates_durable_state(context, expr))
                || step
                    .as_deref()
                    .is_some_and(|statement| statement_mutates_durable_state(context, statement))
                || block_mutates_durable_state(context, body)
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            expr_mutates_durable_state(context, map) || block_mutates_durable_state(context, body)
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => false,
    }
}

fn expr_contains_host_side_effects(expr: &TypedExpr) -> bool {
    match expr.kind() {
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
            Builtin::from_name(name).is_some_and(|builtin| builtin.spec().effects.host_side_effects)
                || evaluated_call_args(name, args)
                    .iter()
                    .any(expr_contains_host_side_effects)
        }
        ExprKind::Binary { left, right, .. } => {
            expr_contains_host_side_effects(left) || expr_contains_host_side_effects(right)
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => expr_contains_host_side_effects(expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_contains_host_side_effects(cond)
                || expr_contains_host_side_effects(then_expr)
                || expr_contains_host_side_effects(else_expr)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_host_side_effects(condition)
                || block_contains_host_side_effects(then_branch)
                || block_contains_host_side_effects(else_branch)
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_host_side_effects(value)
                || block_contains_host_side_effects(then_branch)
                || block_contains_host_side_effects(else_branch)
        }
        ExprKind::Match { value, arms } => {
            expr_contains_host_side_effects(value)
                || arms
                    .iter()
                    .any(|arm| block_contains_host_side_effects(&arm.body))
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            items.iter().any(expr_contains_host_side_effects)
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            expr_contains_host_side_effects(source)
                || expr_contains_host_side_effects(expression)
                || condition
                    .as_deref()
                    .is_some_and(expr_contains_host_side_effects)
        }
        ExprKind::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_contains_host_side_effects(value)),
        ExprKind::JsonObject(entries) => entries
            .iter()
            .any(|(_, value)| expr_contains_host_side_effects(value)),
        ExprKind::JsonArray(items) => items.iter().any(expr_contains_host_side_effects),
        ExprKind::Member { object, .. } => expr_contains_host_side_effects(object),
        ExprKind::Index { target, index } => {
            expr_contains_host_side_effects(target) || expr_contains_host_side_effects(index)
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => false,
    }
}

fn expr_contains_instruction_emission(expr: &TypedExpr) -> bool {
    match expr.kind() {
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
            Builtin::from_name(name)
                .is_some_and(|builtin| builtin.spec().effects.emits_instructions)
                || evaluated_call_args(name, args)
                    .iter()
                    .any(expr_contains_instruction_emission)
        }
        ExprKind::Binary { left, right, .. } => {
            expr_contains_instruction_emission(left) || expr_contains_instruction_emission(right)
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => expr_contains_instruction_emission(expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_contains_instruction_emission(cond)
                || expr_contains_instruction_emission(then_expr)
                || expr_contains_instruction_emission(else_expr)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_instruction_emission(condition)
                || block_contains_instruction_emission(then_branch)
                || block_contains_instruction_emission(else_branch)
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_instruction_emission(value)
                || block_contains_instruction_emission(then_branch)
                || block_contains_instruction_emission(else_branch)
        }
        ExprKind::Match { value, arms } => {
            expr_contains_instruction_emission(value)
                || arms
                    .iter()
                    .any(|arm| block_contains_instruction_emission(&arm.body))
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            items.iter().any(expr_contains_instruction_emission)
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            expr_contains_instruction_emission(source)
                || expr_contains_instruction_emission(expression)
                || condition
                    .as_deref()
                    .is_some_and(expr_contains_instruction_emission)
        }
        ExprKind::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_contains_instruction_emission(value)),
        ExprKind::JsonObject(entries) => entries
            .iter()
            .any(|(_, value)| expr_contains_instruction_emission(value)),
        ExprKind::JsonArray(items) => items.iter().any(expr_contains_instruction_emission),
        ExprKind::Member { object, .. } => expr_contains_instruction_emission(object),
        ExprKind::Index { target, index } => {
            expr_contains_instruction_emission(target) || expr_contains_instruction_emission(index)
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => false,
    }
}

fn expr_mutates_durable_state(context: &SemanticContext, expr: &TypedExpr) -> bool {
    match expr.kind() {
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
            Builtin::from_name(name)
                .is_some_and(|builtin| builtin.spec().effects.mutates_durable_state)
                || (matches!(Builtin::from_name(name), Some(Builtin::Ensure))
                    && args
                        .first()
                        .is_some_and(|arg| typed_map_expr_is_state(context, arg)))
                || evaluated_call_args(name, args)
                    .iter()
                    .any(|arg| expr_mutates_durable_state(context, arg))
        }
        ExprKind::Binary { left, right, .. } => {
            expr_mutates_durable_state(context, left) || expr_mutates_durable_state(context, right)
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => expr_mutates_durable_state(context, expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_mutates_durable_state(context, cond)
                || expr_mutates_durable_state(context, then_expr)
                || expr_mutates_durable_state(context, else_expr)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_mutates_durable_state(context, condition)
                || block_mutates_durable_state(context, then_branch)
                || block_mutates_durable_state(context, else_branch)
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_mutates_durable_state(context, value)
                || block_mutates_durable_state(context, then_branch)
                || block_mutates_durable_state(context, else_branch)
        }
        ExprKind::Match { value, arms } => {
            expr_mutates_durable_state(context, value)
                || arms
                    .iter()
                    .any(|arm| block_mutates_durable_state(context, &arm.body))
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => items
            .iter()
            .any(|item| expr_mutates_durable_state(context, item)),
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            expr_mutates_durable_state(context, source)
                || expr_mutates_durable_state(context, expression)
                || condition
                    .as_deref()
                    .is_some_and(|condition| expr_mutates_durable_state(context, condition))
        }
        ExprKind::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_mutates_durable_state(context, value)),
        ExprKind::JsonObject(entries) => entries
            .iter()
            .any(|(_, value)| expr_mutates_durable_state(context, value)),
        ExprKind::JsonArray(items) => items
            .iter()
            .any(|item| expr_mutates_durable_state(context, item)),
        ExprKind::Member { object, .. } => expr_mutates_durable_state(context, object),
        ExprKind::Index { target, index } => {
            expr_mutates_durable_state(context, target)
                || expr_mutates_durable_state(context, index)
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_test_fragment as parse;

    fn shared_struct_dag_source(levels: usize, repeated_reads: usize) -> String {
        let mut source = String::from("seiyaku SharedTypes {\n");
        for index in 0..levels {
            source.push_str(&format!(
                "struct S{index:03} {{ S{:03} left; S{:03} right; }}\n",
                index + 1,
                index + 1
            ));
        }
        source.push_str(&format!("struct S{levels:03} {{ int value; }}\n"));
        source.push_str("state StateMap<int, S000> records;\nfn repeated_reads() {\n");
        for index in 0..repeated_reads {
            source.push_str(&format!("let value{index} = records.get(0);\n"));
        }
        source.push_str("}\n}\n");
        source
    }

    #[test]
    fn large_shared_struct_references_and_expression_checks_reuse_one_canonical_dag() {
        let source = shared_struct_dag_source(14, 128);
        let program = parse(&source).expect("parse repeated large-struct references");
        let context = SemanticContext::new();
        let typed = context
            .analyze(&program)
            .expect("canonical struct references must not multiply expanded work");

        let canonical = context.resolved_named_types.borrow();
        let Type::Struct {
            fields: canonical_fields,
            ..
        } = canonical.get("S000").expect("canonical root struct")
        else {
            panic!("S000 must resolve to a canonical product type");
        };
        let Type::StateMap(_, state_value) = &typed.states[0].ty else {
            panic!("fixture state must remain a StateMap");
        };
        let Type::Struct {
            fields: state_fields,
            ..
        } = state_value.as_ref()
        else {
            panic!("StateMap value must resolve to S000");
        };
        assert!(Arc::ptr_eq(canonical_fields, state_fields));

        let function = typed
            .items
            .iter()
            .find_map(|item| {
                let TypedItem::Function(function) = item;
                (function.name == "repeated_reads").then_some(function)
            })
            .expect("typed repeated-read function");
        let mut checked = 0_usize;
        for statement in &function.body.statements {
            let TypedStatement::Let { value, .. } = statement else {
                continue;
            };
            let Type::Option(value) = &value.ty else {
                continue;
            };
            let Type::Struct { fields, .. } = value.as_ref() else {
                continue;
            };
            assert!(Arc::ptr_eq(canonical_fields, fields));
            checked += 1;
        }
        assert_eq!(checked, 128);
    }

    #[test]
    fn runtime_word_count_stops_at_the_fixed_register_window_for_shared_dags() {
        let source = shared_struct_dag_source(14, 0);
        let program = parse(&source).expect("parse shared word-count fixture");
        let context = SemanticContext::new();
        context
            .analyze(&program)
            .expect("shared word-count fixture must type-check");
        let canonical = context.resolved_named_types.borrow();
        let root = canonical.get("S000").expect("canonical shared root");

        assert_eq!(
            runtime_value_word_count_bounded(root, crate::regalloc::MAX_ARGUMENT_VALUES),
            Some(crate::regalloc::MAX_ARGUMENT_VALUES + 1),
            "word accounting must stop immediately after crossing the ABI window"
        );
    }

    #[test]
    fn modest_shared_struct_references_preserve_ordinary_semantics() {
        let source = shared_struct_dag_source(8, 16);
        let program = parse(&source).expect("parse modest shared references");
        let typed = analyze(&program).expect("modest shared references must type-check");
        assert_eq!(typed.states.len(), 1);
        assert!(matches!(typed.states[0].ty, Type::StateMap(_, _)));
    }

    #[test]
    fn compiler_owned_test_return_selector_is_reserved_for_functions() {
        assert!(is_reserved_source_declaration(
            crate::metadata::KOTO_TEST_RETURN_ENTRYPOINT,
            true
        ));
        assert!(!is_reserved_source_declaration(
            crate::metadata::KOTO_TEST_RETURN_ENTRYPOINT,
            false
        ));
    }

    #[test]
    fn retired_numeric_spellings_are_reserved_only_for_declared_types() {
        const EXPECTED: &[&str] = &[
            "i8",
            "i16",
            "i32",
            "i64",
            "i128",
            "isize",
            "u8",
            "u16",
            "u32",
            "u64",
            "u128",
            "usize",
            "num",
            "Int",
            "Integer",
            "float",
            "f32",
            "f64",
            "Decimal",
            "Fixed",
            "FixedPoint",
            "Amount",
            "amount",
            "money",
            "Quantity",
            "number",
        ];
        assert_eq!(V1_RETIRED_NUMERIC_TYPE_NAMES, EXPECTED);
        for name in EXPECTED {
            assert!(
                is_reserved_source_type_declaration(name),
                "retired numeric type `{name}` must remain reserved for declared types"
            );
            assert!(
                !is_reserved_source_declaration(name, false),
                "retired numeric type `{name}` must remain available in the value namespace"
            );
            assert!(
                !is_reserved_source_declaration(name, true),
                "retired numeric type `{name}` must remain available in the function namespace"
            );
        }
    }

    #[test]
    fn retired_numeric_spellings_are_rejected_as_source_unit_identities() {
        for (source, name) in [
            ("seiyaku Amount { view fn run() {} }", "Amount"),
            ("module i64 { fn run() {} }", "i64"),
        ] {
            let program = parse(source).expect("retired source-unit identity parses");
            let error = analyze(&program)
                .expect_err("retired numeric spelling must not identify a source unit");
            assert_eq!(error.code, "E_RESERVED_DECLARATION");
            assert_eq!(
                error.message,
                format!("source unit `{name}` uses a compiler-reserved name")
            );
        }
    }

    #[test]
    fn production_projection_accepts_registered_intrinsics_and_rejects_fabricated_calls() {
        let retained = HashSet::new();
        let removed = HashSet::new();
        let registered = [
            STATE_MAP_GET_INTRINSIC,
            LIST_LEN_INTRINSIC,
            LIST_GET_INTRINSIC,
            LIST_TRY_SET_INTRINSIC,
            LIST_TRY_PUSH_INTRINSIC,
            LIST_POP_INTRINSIC,
            LIST_CONTAINS_INTRINSIC,
            LIST_TAKE_INTRINSIC,
            LIST_ENUMERATE_INTRINSIC,
            DECIMAL_DIV_ROUND_INTRINSIC,
            QUANTITY_DIV_ROUND_INTRINSIC,
            QUANTITY_RATIO_ROUND_INTRINSIC,
            DECIMAL_TO_INT_TRUNC_INTRINSIC,
            DECIMAL_TO_INT_ROUND_INTRINSIC,
            "is_some",
            "is_none",
            "is_ok",
            "is_err",
            "unwrap_or",
            "unwrap_err_or",
        ];
        for name in registered {
            assert!(
                compiler_intrinsic_kind(name).is_some(),
                "missing compiler intrinsic registry entry for {name}"
            );
            assert!(
                is_reserved_source_declaration(name, true),
                "compiler intrinsic {name} must not be shadowable by a source function"
            );
            let expression = TypedExpr {
                expr: ExprKind::Call {
                    name: name.to_owned(),
                    args: Vec::new(),
                },
                ty: Type::Int,
            };
            validate_production_projection_expr(
                &expression,
                "retained_helper",
                &retained,
                &removed,
                false,
            )
            .unwrap_or_else(|error| panic!("registered intrinsic {name} was rejected: {error:?}"));
        }

        let fabricated = TypedExpr {
            expr: ExprKind::Call {
                name: "__fabricated_projection_escape".to_owned(),
                args: Vec::new(),
            },
            ty: Type::Int,
        };
        assert!(compiler_intrinsic_kind("__fabricated_projection_escape").is_none());
        assert!(!is_reserved_source_declaration(
            "__fabricated_projection_escape",
            true
        ));
        let error = validate_production_projection_expr(
            &fabricated,
            "retained_helper",
            &retained,
            &removed,
            false,
        )
        .expect_err("unregistered typed calls must fail closed");
        assert_eq!(error.code, "K2002");
        assert!(error.message.contains("__fabricated_projection_escape"));
    }

    #[test]
    fn removed_test_function_cannot_hide_behind_an_intrinsic_name() {
        let retained = HashSet::new();
        let removed = HashSet::from(["is_some".to_owned()]);
        let expression = TypedExpr {
            expr: ExprKind::Call {
                name: "is_some".to_owned(),
                args: Vec::new(),
            },
            ty: Type::Bool,
        };
        let error = validate_production_projection_expr(
            &expression,
            "retained_helper",
            &retained,
            &removed,
            false,
        )
        .expect_err("removed test calls must take precedence over intrinsic classification");
        assert_eq!(error.code, "E_TEST_ONLY_PRODUCTION");
    }

    #[test]
    fn pending_diagnostic_fills_first_spanless_failure_without_masking_structured_failure() {
        let source = crate::source::SourceId(9);
        let structured = crate::semantic_diagnostics::SemanticDiagnostic {
            primary: crate::source::SourceRange::new(source, crate::source::TextRange::new(1, 2)),
            labels: Vec::new(),
            fix: None,
        };
        let pending = crate::semantic_diagnostics::SemanticDiagnostic {
            primary: crate::source::SourceRange::new(source, crate::source::TextRange::new(3, 4)),
            labels: Vec::new(),
            fix: None,
        };
        let failure = |code, diagnostic| SemanticFailure {
            error: SemanticError {
                code,
                message: "localized text".to_owned(),
            },
            location: None,
            diagnostic,
        };
        let mut failures = SemanticFailures {
            failures: vec![
                failure("E_FIRST", Some(structured.clone())),
                failure("E_SECOND", None),
                failure("E_THIRD", None),
            ],
        };

        attach_pending_diagnostic(&mut failures, Some(pending.clone()));

        assert_eq!(failures.failures[0].diagnostic, Some(structured));
        assert_eq!(failures.failures[1].diagnostic, Some(pending));
        assert!(failures.failures[2].diagnostic.is_none());
    }

    fn sample_account_literal() -> String {
        iroha_data_model::account::AccountId::new(
            "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
                .parse()
                .expect("public key"),
        )
        .to_string()
    }

    fn analyze_test(program: &Program) -> Result<TypedProgram, SemanticError> {
        SemanticContext::with_capabilities(false, true).analyze(program)
    }

    fn analyze_error(source: &str) -> SemanticError {
        let program = parse(source).expect("source should parse");
        analyze(&program).expect_err("semantic analysis should reject source")
    }

    fn returned_expr(source: &str) -> TypedExpr {
        let program = parse(source).expect("source should parse");
        let typed = analyze(&program).expect("source should analyze");
        typed
            .items
            .into_iter()
            .find_map(|item| {
                let TypedItem::Function(function) = item;
                function.body.statements.into_iter().find_map(|statement| {
                    if let TypedStatement::Return(Some(expr)) = statement {
                        Some(expr)
                    } else {
                        None
                    }
                })
            })
            .expect("function return expression")
    }

    fn function_tail(source: &str) -> TypedExpr {
        let program = parse(source).expect("source should parse");
        let typed = analyze(&program).expect("source should analyze");
        let TypedItem::Function(function) = typed.items.into_iter().next().expect("function item");
        *function.body.tail.expect("function tail expression")
    }

    #[test]
    fn list_literals_infer_exact_or_contextual_capacity() {
        let exact = function_tail("fn exact() -> List<int, 2> { [1, 2] }");
        assert_eq!(exact.ty, Type::List(Box::new(Type::Int), 2));
        assert!(matches!(exact.expr, ExprKind::List(ref items) if items.len() == 2));

        let contextual = function_tail("fn wider() -> List<int, 8> { [1, 2] }");
        assert_eq!(contextual.ty, Type::List(Box::new(Type::Int), 8));

        let inferred_program =
            parse("fn inferred() { let values = [1, 2, 3]; }").expect("parse inferred List");
        let inferred = analyze(&inferred_program).expect("analyze inferred List");
        let TypedItem::Function(function) = &inferred.items[0];
        let TypedStatement::Let { value, .. } = &function.body.statements[0] else {
            panic!("expected List binding");
        };
        assert_eq!(value.ty, Type::List(Box::new(Type::Int), 3));
    }

    #[test]
    fn empty_and_oversized_list_literals_fail_closed() {
        let empty = function_tail("fn empty() -> List<int, 4> { [] }");
        assert_eq!(empty.ty, Type::List(Box::new(Type::Int), 4));

        let error = analyze_error("fn missing_context() { let values = []; }");
        assert_eq!(error.code, "E_LIST_EMPTY_CONTEXT");

        let values = std::iter::repeat_n("1", 65).collect::<Vec<_>>().join(", ");
        let error = analyze_error(&format!("fn oversized() {{ let values = [{values}]; }}"));
        assert_eq!(error.code, "E_LIST_CAPACITY");
    }

    #[test]
    fn zero_sized_list_elements_are_rejected_at_every_semantic_boundary() {
        for source in [
            "struct Empty {} fn typed() -> List<Empty, 1> { [Empty {}] }",
            "struct Empty {} fn inferred() { let values = [Empty {}]; }",
            "struct Empty {} fn contextual() { let List<Empty, 2> values = []; }",
            "struct Empty {} struct Pair { Empty left, Empty right } fn parameter(List<Pair, 1> values) { let _values = values; }",
            "struct Empty {} fn nested(Option<List<Empty, 1>> value) { let _value = value; }",
            "struct Empty {} struct Holder { List<Empty, 1> invalid } fn unused() { return; }",
            "struct Empty {} fn comprehension() { let source = [1]; let values = [Empty {} for item in source]; }",
        ] {
            let error = analyze_error(source);
            assert_eq!(error.code, "E_LIST_ZERO_SIZED_ELEMENT", "{source}");
            assert!(error.message.contains("at least one word"));
        }
    }

    #[test]
    fn contextual_empty_lists_and_one_word_sum_handles_remain_valid() {
        let ordinary = function_tail("fn empty() -> List<int, 4> { [] }");
        assert_eq!(ordinary.ty, Type::List(Box::new(Type::Int), 4));

        let sum_handle = function_tail(
            "struct Empty {} fn values() -> List<Option<Empty>, 1> { [Option::none] }",
        );
        assert_eq!(
            sum_handle.ty,
            Type::List(
                Box::new(Type::Option(Box::new(Type::Struct {
                    name: "Empty".into(),
                    fields: Arc::from(Vec::new()),
                }))),
                1,
            )
        );

        let forward = function_tail(
            "struct Holder { List<Value, 1> values } struct Value { int item } fn values() -> List<Value, 1> { [Value { item: 1 }] }",
        );
        assert!(matches!(forward.ty, Type::List(_, 1)));
    }

    #[test]
    fn native_json_rejects_decoded_duplicate_keys_and_oversized_nodes() {
        let error = analyze_error(
            r#"fn duplicate(AccountId owner) -> Json {
                json { owner: owner, "owner": owner }
            }"#,
        );
        assert_eq!(error.code, "E_JSON_DUPLICATE_KEY");

        let object_entries = (0..65)
            .map(|index| format!("key{index}: {index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let error = analyze_error(&format!(
            "fn oversized_object() -> Json {{ json {{ {object_entries} }} }}"
        ));
        assert_eq!(error.code, "E_JSON_CAPACITY");

        let array_elements = (0..65)
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let error = analyze_error(&format!(
            "fn oversized_array() -> Json {{ json [{array_elements}] }}"
        ));
        assert_eq!(error.code, "E_JSON_CAPACITY");
    }

    #[test]
    fn json_parse_literals_fail_during_semantic_analysis() {
        let duplicate = analyze_error(
            r#"fn duplicate() -> Json {
                Json::parse("{\"owner\":1,\"owner\":2}")
            }"#,
        );
        assert_eq!(duplicate.code, "E_JSON_DUPLICATE_KEY");
        assert!(duplicate.message.contains("owner"), "{}", duplicate.message);

        let malformed = analyze_error(
            r#"fn malformed() -> Json {
                Json::parse("{\"owner\":")
            }"#,
        );
        assert_eq!(malformed.code, "E_JSON_LITERAL_INVALID");
        assert!(
            malformed.message.contains("Json::parse"),
            "{}",
            malformed.message
        );
    }

    #[test]
    fn json_parse_accepts_dynamic_string_inputs() {
        let program = parse(
            r#"fn decode(string raw) -> Json {
                Json::parse(raw)
            }"#,
        )
        .expect("dynamic Json::parse source should parse");
        analyze(&program).expect("dynamic Json::parse string input should remain legal");
    }

    #[test]
    fn native_json_requires_explicit_result_and_struct_conversion() {
        let result =
            analyze_error("fn invalid(Result<int, int> value) -> Json { json { value: value } }");
        assert_eq!(result.code, "E_JSON_VALUE_TYPE");
        assert!(result.message.contains("Result"), "{}", result.message);

        let structure = analyze_error(
            "struct Payload { int value } fn invalid(Payload value) -> Json { json { value: value } }",
        );
        assert_eq!(structure.code, "E_JSON_VALUE_TYPE");
        assert!(
            structure.message.contains("arbitrary struct"),
            "{}",
            structure.message
        );
    }

    #[test]
    fn native_json_rejects_recursive_schema_limits_during_semantic_checking() {
        let children = std::iter::repeat_n("json [1, 2, 3, 4]", 64)
            .collect::<Vec<_>>()
            .join(", ");
        let expression = format!("json [{children}]");
        let error = analyze_error(&format!(
            "fn recursively_oversized() -> Json {{ {expression} }}"
        ));
        assert_eq!(error.code, "E_JSON_SCHEMA_LIMIT", "{}", error.message);
        assert!(error.message.contains("V1"));

        let long_keys = (0..64)
            .map(|index| format!("\"key{index}{}\": {index}", "x".repeat(1_100)))
            .collect::<Vec<_>>()
            .join(", ");
        let error = analyze_error(&format!(
            "fn byte_oversized() -> Json {{ json {{ {long_keys} }} }}"
        ));
        assert_eq!(error.code, "E_JSON_SCHEMA_LIMIT", "{}", error.message);
        assert!(error.message.contains("byte limit"), "{}", error.message);
    }

    #[test]
    fn list_comprehensions_preserve_the_proven_source_maximum() {
        let expression = function_tail(
            "fn doubled() -> List<int, 8> { let List<int, 8> source = [1, 2]; [value * 2 for value in source if value > 0] }",
        );
        assert_eq!(expression.ty, Type::List(Box::new(Type::Int), 8));
        assert!(matches!(
            expression.expr,
            ExprKind::ListComprehension { .. }
        ));

        let error = analyze_error(
            "fn too_small() -> List<int, 4> { let List<int, 8> source = [1, 2]; [value for value in source if false] }",
        );
        assert_eq!(error.code, "E_LIST_COMPREHENSION_CAPACITY");
        assert!(error.message.contains("filters do not reduce"));
    }

    #[test]
    fn lists_allow_nested_structures_but_reject_resource_handles() {
        let nested = function_tail(
            "struct Pair { int left, bool right } fn nested() -> List<List<Pair, 2>, 2> { [[Pair { left: 1, right: true }]] }",
        );
        assert!(matches!(nested.ty, Type::List(_, 2)));

        let error =
            analyze_error("fn resources(List<Option<StateMap<int, int>>, 2> value) { return; }");
        assert_eq!(error.code, "E_LIST_RESOURCE_ELEMENT");

        let secret_source =
            "fn resources(List<Option<Secret<int>>, 2> value) { let ignored = value; }";
        let secret_program = parse(secret_source).expect("secret List source should parse");
        let error = SemanticContext::with_zk_enabled(true)
            .analyze(&secret_program)
            .expect_err("nested Secret handles must not become List elements");
        assert_eq!(error.code, "E_LIST_RESOURCE_ELEMENT");
    }

    #[test]
    fn every_list_method_has_a_typed_safe_surface() {
        let program = parse(
            "fn methods() -> List<(int, int), 4> {\
                 var List<int, 4> values = [1, 2];\
                 let length = values.len();\
                 let Option<int> first = values.get(0);\
                 let changed = values.try_set(index: 0, value: 3);\
                 let pushed = values.try_push(4);\
                 let has_three = values.contains(3);\
                 let Option<int> removed = values.pop();\
                 let List<int, 2> head = values.take(2);\
                 values.enumerate()\
             }",
        )
        .expect("parse List methods");
        let typed = analyze(&program).expect("analyze List methods");
        let TypedItem::Function(function) = &typed.items[0];
        assert_eq!(
            function.body.tail.as_ref().expect("enumerate tail").ty,
            Type::List(Box::new(Type::Tuple(vec![Type::Int, Type::Int])), 4)
        );

        let error =
            analyze_error("fn immutable() { let List<int, 2> values = [1]; values.try_push(2); }");
        assert_eq!(error.code, "E_LIST_MUTABLE_RECEIVER");

        let error = analyze_error("fn temporary() { let pushed = [1].try_push(2); }");
        assert_eq!(error.code, "E_LIST_MUTABLE_RECEIVER");

        let error = analyze_error(
            "fn ambiguous() { var List<int, 2> values = [1]; values.try_set(0, 1); }",
        );
        assert_eq!(error.code, "E_NAMED_ARGUMENTS_REQUIRED");

        analyze(
            &parse(
                "fn distinct() { var List<string, 2> values = [\"a\"]; values.try_set(0, \"b\"); }",
            )
            .expect("parse distinct List.try_set types"),
        )
        .expect("distinct index/value types may remain positional");
    }

    #[test]
    fn sourced_mutable_list_receiver_retains_its_binding_identity() {
        let source = crate::source::SourceFile::new(
            crate::source::SourceId(41),
            "mutable-list.ko",
            "seiyaku Lists { view fn main() { var List<int, 2> values = [1]; values.try_push(2); } }",
        );
        let (spanned, _) =
            crate::parser::parse_source_spanned(&source, crate::source::FrontendBudget::v1())
                .expect("parse sourced mutable List receiver");
        analyze(&spanned.program).expect("source provenance must not hide the mutable binding");
    }

    #[test]
    fn list_mutability_does_not_leak_between_sibling_lexical_bindings() {
        for (index, body) in [
            "if flag { var List<int, 2> values = [1]; } else { let List<int, 2> values = [1]; values.try_push(2); }",
            "if flag { let List<int, 2> values = [1]; values.try_push(2); } else { var List<int, 2> values = [1]; }",
        ]
        .into_iter()
        .enumerate()
        {
            let text = format!("seiyaku Lists {{ view fn main(bool flag) {{ {body} }} }}");

            let raw = parse(&text).expect("parse raw sibling List bindings");
            let raw_error =
                analyze(&raw).expect_err("raw-AST fallback must preserve lexical mutability");
            assert_eq!(raw_error.code, "E_LIST_MUTABLE_RECEIVER", "{body}");

            let source = crate::source::SourceFile::new(
                crate::source::SourceId(42 + index as u32),
                format!("list-sibling-{index}.ko"),
                text,
            );
            let (spanned, _) =
                crate::parser::parse_source_spanned(&source, crate::source::FrontendBudget::v1())
                    .expect("parse sourced sibling List bindings");
            let resolved =
                crate::resolved::resolve(spanned, &source).expect("resolve sibling bindings");
            let resolved_error = SemanticContext::new()
                .analyze_resolved(&resolved)
                .expect_err("BindingId mutability must reject the immutable sibling");
            assert!(
                resolved_error
                    .failures
                    .iter()
                    .any(|failure| failure.error.code == "E_LIST_MUTABLE_RECEIVER"),
                "{body}: {resolved_error:?}"
            );
        }
    }

    #[test]
    fn list_take_accepts_zero_and_rejects_limits_above_source_capacity() {
        let zero = function_tail(
            "fn zero() -> List<int, 1> { let List<int, 4> values = [1, 2]; values.take(0) }",
        );
        assert_eq!(zero.ty, Type::List(Box::new(Type::Int), 1));

        for (source, code) in [
            (
                "fn above_source() { let List<int, 1> values = [1]; let head = values.take(2); }",
                "E_LIST_TAKE_LIMIT",
            ),
            (
                "fn large() { let values = [1]; let head = values.take(65); }",
                "E_LIST_TAKE_LIMIT",
            ),
            (
                "fn dynamic() { let values = [1]; let limit = 1; let head = values.take(limit); }",
                "E_LIST_TAKE_CONST",
            ),
        ] {
            let error = analyze_error(source);
            assert_eq!(error.code, code, "{}", error.message);
        }
    }

    #[test]
    fn list_contains_accepts_recursive_durable_aggregates() {
        let expression = function_tail(
            r#"
                struct Envelope {
                    List<Option<int>, 2> labels,
                    Result<(int, bool), int> outcome,
                }

                fn contains_nested() -> bool {
                    let List<Envelope, 2> values = [
                        Envelope {
                            labels: [Option::none, Option::some(7)],
                            outcome: Result::ok((9, true)),
                        },
                    ];
                    values.contains(Envelope {
                        labels: [Option::none, Option::some(7)],
                        outcome: Result::ok((9, true)),
                    })
                }
            "#,
        );
        assert_eq!(expression.ty, Type::Bool);
        assert!(
            matches!(expression.expr, ExprKind::Call { ref name, .. } if name == LIST_CONTAINS_INTRINSIC)
        );
    }

    #[test]
    fn unchecked_list_reads_and_writes_have_actionable_diagnostics() {
        let error = analyze_error("fn read() { let values = [1]; let value = values[0]; }");
        assert_eq!(error.code, "E_LIST_UNSAFE_INDEX");
        assert!(
            error.message.contains("values.get(index)")
                || error.message.contains("list.get(index)")
        );

        let error = analyze_error("fn write() { var values = [1]; values[0] = 2; }");
        assert_eq!(error.code, "E_LIST_UNSAFE_INDEX");
        assert!(error.message.contains("try_set"));
    }

    #[test]
    fn decimal_literals_are_exact_canonical_and_preserve_source_spelling() {
        for (spelling, canonical) in [
            ("0.0", "0"),
            ("1.250_0", "1.25"),
            ("1e3", "1000"),
            ("1.5e-3", "0.0015"),
            ("12.00e+2", "1200"),
        ] {
            let expression =
                returned_expr(&format!("fn value() -> decimal {{ return {spelling}; }}"));
            let ExprKind::DecimalLiteral {
                value,
                spelling: retained,
            } = expression.expr
            else {
                panic!("expected exact decimal literal for {spelling}");
            };
            assert_eq!(retained, spelling);
            assert_eq!(value.to_string(), canonical);
        }
    }

    #[test]
    fn decimal_literal_normalizes_before_enforcing_scale_twenty_eight() {
        let removable = format!("0.{}10", "0".repeat(27));
        let expression = returned_expr(&format!("fn value() -> decimal {{ return {removable}; }}"));
        let ExprKind::DecimalLiteral { value, .. } = expression.expr else {
            panic!("expected normalized decimal literal");
        };
        assert_eq!(value.mantissa().to_string(), "1");
        assert_eq!(value.scale(), 28);

        let zero = format!("0.{}", "0".repeat(80));
        let expression = returned_expr(&format!("fn value() -> decimal {{ return {zero}; }}"));
        let ExprKind::DecimalLiteral { value, .. } = expression.expr else {
            panic!("expected canonical zero");
        };
        assert!(value.is_zero());
        assert_eq!(value.scale(), 0);

        let nonremovable = format!("0.{}1", "0".repeat(28));
        let error = analyze_error(&format!(
            "fn value() -> decimal {{ return {nonremovable}; }}"
        ));
        assert_eq!(error.code, "E_DECIMAL_SCALE_OVERFLOW");
    }

    #[test]
    fn int_literal_accepts_both_signed_512_bit_endpoints_and_rejects_neighbors() {
        fn decimal_plus_one(value: &str) -> String {
            let mut digits = value.as_bytes().to_vec();
            let mut carry = true;
            for digit in digits.iter_mut().rev() {
                if !carry {
                    break;
                }
                if *digit == b'9' {
                    *digit = b'0';
                } else {
                    *digit += 1;
                    carry = false;
                }
            }
            if carry {
                digits.insert(0, b'1');
            }
            String::from_utf8(digits).expect("decimal digits")
        }

        let mut maximum_bytes = vec![0xff; 64];
        maximum_bytes[63] = 0x7f;
        let maximum = BigInt::from_twos_bytes(&maximum_bytes).expect("signed 512-bit maximum");
        let mut minimum_bytes = vec![0; 64];
        minimum_bytes[63] = 0x80;
        let minimum = BigInt::from_twos_bytes(&minimum_bytes).expect("signed 512-bit minimum");

        let maximum_expression =
            returned_expr(&format!("fn value() -> int {{ return {maximum}; }}"));
        assert!(matches!(maximum_expression.expr, ExprKind::IntLiteral(value) if value == maximum));
        let minimum_expression =
            returned_expr(&format!("fn value() -> int {{ return {minimum}; }}"));
        assert!(matches!(minimum_expression.expr, ExprKind::IntLiteral(value) if value == minimum));

        let above = decimal_plus_one(&maximum.to_string());
        let below_magnitude = decimal_plus_one(minimum.to_string().trim_start_matches('-'));
        for source in [
            format!("fn value() -> int {{ return {above}; }}"),
            format!("fn value() -> int {{ return -{below_magnitude}; }}"),
        ] {
            let error = parse(&source).expect_err("neighbor outside signed 512-bit range");
            assert!(error.contains("E_INT_LITERAL_OVERFLOW"), "{error}");
        }
    }

    #[test]
    fn decimal_literal_accepts_signed_minimum_after_combining_unary_minus() {
        let mut minimum_bytes = vec![0; MAX_MANTISSA_BYTES];
        minimum_bytes[MAX_MANTISSA_BYTES - 1] = 0x80;
        let minimum = BigInt::from_twos_bytes(&minimum_bytes).expect("signed 512-bit minimum");
        let magnitude = minimum.to_string().trim_start_matches('-').to_owned();

        let expression = returned_expr(&format!(
            "fn value() -> decimal {{ return -{magnitude}.0; }}"
        ));
        assert!(matches!(
            expression.expr,
            ExprKind::DecimalLiteral { ref value, .. }
                if value.mantissa() == &minimum && value.scale() == 0
        ));

        let error = analyze_error(&format!(
            "fn value() -> decimal {{ return {magnitude}.0; }}"
        ));
        assert_eq!(error.code, "E_DECIMAL_MANTISSA_OVERFLOW");
    }

    #[test]
    fn decimal_literal_ignores_leading_zeroes_before_width_checks() {
        let expression = returned_expr(&format!(
            "fn value() -> decimal {{ return {}1e1; }}",
            "0".repeat(1_000)
        ));
        assert!(matches!(
            expression.expr,
            ExprKind::DecimalLiteral { ref value, .. } if value.to_string() == "10"
        ));
    }

    #[test]
    fn exact_constant_numeric_arithmetic_uses_runtime_primitives() {
        for (source, expected) in [
            ("1.20 + 2.3", "3.5"),
            ("5.0 - 1.25", "3.75"),
            ("1.5 * 2.0", "3"),
            ("1.0 / 8.0", "0.125"),
        ] {
            let expression =
                returned_expr(&format!("fn value() -> decimal {{ return {source}; }}"));
            let ExprKind::DecimalLiteral { value, .. } = expression.expr else {
                panic!("constant decimal expression {source} must fold");
            };
            assert_eq!(value.to_string(), expected);
        }

        for (source, expected_code) in [
            (
                "fn value() -> quantity { return 1 - 2; }",
                "E_QUANTITY_UNDERFLOW",
            ),
            (
                "fn value() -> decimal { return 1.0 / 0.0; }",
                "E_DIVISION_BY_ZERO",
            ),
            (
                "fn value() -> decimal { return 1.0 / 3.0; }",
                "E_REPEATING_DECIMAL",
            ),
            (
                "fn value() -> decimal { return 0.000000000000001 * 0.000000000000001; }",
                "E_DECIMAL_SCALE_OVERFLOW",
            ),
        ] {
            let error = analyze_error(source);
            assert_eq!(error.code, expected_code, "{}", error.message);
        }
    }

    #[test]
    fn exact_literals_inherit_decimal_and_quantity_expression_context() {
        let program = parse(
            "const decimal EIGHTH = 1 / 8; \
             fn value(quantity balance) -> (bool, quantity, quantity) { \
                 return (balance == 0, balance + 1, balance * 2); \
             } \
             fn tuple_literals() -> (decimal, quantity) { \
                 return (1 / 8, 2); \
             }",
        )
        .expect("parse contextual numeric literals");
        analyze(&program).expect("exact literals must inherit their numeric expression context");

        let repeating = analyze_error("const decimal THIRD = 1 / 3;");
        assert_eq!(repeating.code, "E_REPEATING_DECIMAL");

        let underflow = analyze_error("const quantity INVALID = 1 - 2;");
        assert_eq!(underflow.code, "E_QUANTITY_UNDERFLOW");

        let negative = analyze_error("const quantity INVALID = -1;");
        assert_eq!(negative.code, "E_NEGATIVE_QUANTITY");

        let scaled =
            returned_expr("fn scaled(quantity balance) -> quantity { return balance / 2; }");
        assert_eq!(scaled.ty, Type::Quantity);
        let ratio = returned_expr("fn ratio(quantity balance) -> decimal { return balance / 2; }");
        assert_eq!(ratio.ty, Type::Decimal);
    }

    #[test]
    fn rounded_decimal_division_supports_every_v1_rounding_mode() {
        use ivm_abi::numeric::RoundingModeV1 as AbiMode;

        for (dividend, mode, expected, abi_mode) in [
            ("1.0", "toward_zero", "0.12", AbiMode::TowardZero),
            ("1.0", "away_from_zero", "0.13", AbiMode::AwayFromZero),
            ("-1.0", "floor", "-0.13", AbiMode::Floor),
            ("-1.0", "ceil", "-0.12", AbiMode::Ceil),
            ("1.0", "nearest_even", "0.12", AbiMode::NearestEven),
            ("1.0", "nearest_away", "0.13", AbiMode::NearestAway),
            (
                "1.0",
                "nearest_toward_zero",
                "0.12",
                AbiMode::NearestTowardZero,
            ),
        ] {
            let expression = returned_expr(&format!(
                "fn value() -> decimal {{ return {dividend}.div_round(\
                    divisor: 8.0, scale: 2, mode: Rounding::{mode}); }}"
            ));
            let ExprKind::DecimalLiteral { value, .. } = expression.expr else {
                panic!("constant rounded division must fold");
            };
            assert_eq!(value.to_string(), expected, "mode={mode}");

            let expression = returned_expr(&format!(
                "fn rounded(decimal value) -> decimal {{ return value.div_round(\
                    divisor: 8.0, scale: 2, mode: Rounding::{mode}); }}"
            ));
            let ExprKind::NamedCall { name, args, .. } = expression.expr else {
                panic!("dynamic rounded division must remain an intrinsic for mode={mode}");
            };
            assert_eq!(name, DECIMAL_DIV_ROUND_INTRINSIC, "mode={mode}");
            assert!(
                matches!(
                    args[3].expr,
                    ExprKind::IntLiteral(ref value)
                        if value.try_to_u64() == Some(abi_mode.tag())
                ),
                "mode={mode} did not lower to ABI tag {}",
                abi_mode.tag(),
            );
        }

        let expression = returned_expr(
            "fn rounded(quantity value, decimal divisor, int scale) -> quantity { \
                return value.div_round( \
                    mode: Rounding::nearest_even, divisor: divisor, scale: scale); }",
        );
        let ExprKind::NamedCall {
            name,
            args,
            evaluation_order,
        } = expression.expr
        else {
            panic!("dynamic rounded division must remain a typed intrinsic");
        };
        assert_eq!(name, QUANTITY_DIV_ROUND_INTRINSIC);
        assert_eq!(args.len(), 4);
        assert_eq!(args[0].ty, Type::Quantity);
        assert_eq!(args[1].ty, Type::Decimal);
        assert_eq!(args[2].ty, Type::Int);
        assert!(matches!(
            args[3].expr,
            ExprKind::IntLiteral(ref value)
                if value.try_to_u64()
                    == Some(ivm_abi::numeric::RoundingModeV1::NearestEven.tag())
        ));
        assert_eq!(evaluation_order, [0, 3, 1, 2]);

        let ratio = returned_expr(
            "fn rounded(quantity value, quantity divisor, int scale) -> decimal { \
                return value.ratio_round( \
                    divisor: divisor, scale: scale, mode: Rounding::floor); }",
        );
        let ExprKind::NamedCall { name, args, .. } = ratio.expr else {
            panic!("dynamic rounded ratio must remain a typed intrinsic");
        };
        assert_eq!(name, QUANTITY_RATIO_ROUND_INTRINSIC);
        assert_eq!(args[0].ty, Type::Quantity);
        assert_eq!(args[1].ty, Type::Quantity);
        assert_eq!(args[2].ty, Type::Int);
        assert_eq!(ratio.ty, Type::Decimal);
    }

    #[test]
    fn unknown_rounding_spellings_are_rejected() {
        for mode in ["nearest", "bankers", "nearest_toward"] {
            let error = analyze_error(&format!(
                "fn value() -> decimal {{ 1.0.div_round(\
                    divisor: 8.0, scale: 2, mode: Rounding::{mode}) }}"
            ));
            assert_eq!(error.code, "E_NUMERIC_ROUNDING_MODE", "mode={mode}");
            assert_eq!(
                error.message,
                format!(
                    "decimal.div_round mode must be one of {}",
                    V1_ROUNDING_PATHS.join(", ")
                ),
                "mode={mode}",
            );
        }
    }

    #[test]
    fn rounded_numeric_methods_reject_noncanonical_signatures() {
        let positional = analyze_error(
            "fn value(decimal input) -> decimal { \
                input.div_round(2.0, 2, Rounding::nearest_even) }",
        );
        assert_eq!(positional.code, "E_NAMED_ARGUMENTS_REQUIRED");

        let int_receiver = analyze_error(
            "fn value(int input) -> decimal { \
                input.div_round( \
                    divisor: 2.0, scale: 2, mode: Rounding::nearest_even) }",
        );
        assert_eq!(int_receiver.code, "E_NUMERIC_ROUND_RECEIVER");

        let decimal_ratio = analyze_error(
            "fn value(decimal input, quantity divisor) -> decimal { \
                input.ratio_round( \
                    divisor: divisor, scale: 2, mode: Rounding::nearest_even) }",
        );
        assert_eq!(decimal_ratio.code, "E_NUMERIC_ROUND_RECEIVER");

        for scale in ["-1", "29"] {
            let error = analyze_error(&format!(
                "fn value(decimal input) -> decimal {{ \
                    input.div_round( \
                        divisor: 2.0, scale: {scale}, mode: Rounding::nearest_even) }}"
            ));
            assert_eq!(error.code, "E_INVALID_SCALE", "scale={scale}");
        }
    }

    #[test]
    fn named_numeric_conversions_preserve_failure_and_rounding_policy() {
        let recoverable = returned_expr(
            "fn convert(decimal value) -> Result<quantity, int> { \
                return quantity::try_from_decimal(value: value); }",
        );
        assert_eq!(
            recoverable.ty,
            Type::Result(Box::new(Type::Quantity), Box::new(Type::Int))
        );
        assert!(matches!(recoverable.expr, ExprKind::NumericTryCast { .. }));

        let truncated =
            returned_expr("fn value() -> int { return decimal::to_int_trunc(value: -1.9); }");
        assert!(
            matches!(truncated.expr, ExprKind::IntLiteral(ref value) if value.try_to_i64() == Some(-1))
        );
        let rounded = returned_expr(
            "fn value() -> int { return decimal::to_int_round(\
                value: 2.5, mode: Rounding::nearest_even); }",
        );
        assert!(
            matches!(rounded.expr, ExprKind::IntLiteral(ref value) if value.try_to_i64() == Some(2))
        );
    }

    #[test]
    fn named_struct_fields_retain_source_evaluation_order() {
        let expr = returned_expr(
            "struct Transfer { int source, string destination, quantity amount } fn build() -> Transfer { return Transfer { amount: 10, destination: \"sink\", source: 7 }; }",
        );
        assert!(matches!(expr.ty, Type::Struct { ref name, .. } if name == "Transfer"));
        let ExprKind::StructLiteral { name, fields } = expr.expr else {
            panic!("expected typed struct literal");
        };
        assert_eq!(name, "Transfer");
        assert_eq!(
            fields
                .iter()
                .map(|(field, _)| field.as_str())
                .collect::<Vec<_>>(),
            ["amount", "destination", "source"]
        );
        assert!(matches!(fields[0].1.expr, ExprKind::DecimalLiteral { .. }));
        assert!(matches!(fields[1].1.expr, ExprKind::String(ref value) if value == "sink"));
        assert!(
            matches!(fields[2].1.expr, ExprKind::IntLiteral(ref value) if value == &BigInt::from(7_i64))
        );
    }

    #[test]
    fn struct_literals_reject_unknown_missing_and_positional_fields() {
        for (source, code) in [
            (
                "struct Pair { int first, string second } fn build() -> Pair { return Pair { first: 1, second: \"two\", third: 3 }; }",
                "E_UNKNOWN_STRUCT_FIELD",
            ),
            (
                "struct Pair { int first, string second } fn build() -> Pair { return Pair { first: 1 }; }",
                "E_MISSING_STRUCT_FIELD",
            ),
            (
                "struct Pair { int first, string second } fn build() -> Pair { return Pair(1, \"two\"); }",
                "E_POSITIONAL_STRUCT",
            ),
        ] {
            let error = analyze_error(source);
            assert_eq!(error.code, code, "{}", error.message);
        }
    }

    #[test]
    fn named_user_call_arguments_are_reordered_to_parameter_order() {
        let program = parse(
            "fn target(int first, string second) -> int { return first; } fn main() -> int { return target(second: \"two\", first: 1); }",
        )
        .expect("parse named user call");
        let typed = analyze(&program).expect("analyze named user call");
        let main = typed
            .items
            .into_iter()
            .find_map(|item| {
                let TypedItem::Function(function) = item;
                (function.name == "main").then_some(function)
            })
            .expect("main function");
        let TypedStatement::Return(Some(call)) = &main.body.statements[0] else {
            panic!("expected returned call");
        };
        let ExprKind::NamedCall {
            args,
            evaluation_order,
            ..
        } = &call.expr
        else {
            panic!("expected typed call");
        };
        assert!(matches!(args[0].expr, ExprKind::IntLiteral(ref value) if value == &BigInt::one()));
        assert!(matches!(args[1].expr, ExprKind::String(ref value) if value == "two"));
        assert_eq!(evaluation_order, &[1, 0]);
    }

    #[test]
    fn named_user_calls_reject_unknown_missing_and_ambiguous_positional_arguments() {
        for (source, code) in [
            (
                "fn target(int first, string second) {} fn main() { target(first: 1, third: \"three\"); }",
                "E_UNKNOWN_NAMED_ARGUMENT",
            ),
            (
                "fn target(int first, string second) {} fn main() { target(first: 1); }",
                "E_MISSING_NAMED_ARGUMENT",
            ),
            (
                "fn target(int left, int right) {} fn main() { target(1, 2); }",
                "E_NAMED_ARGUMENTS_REQUIRED",
            ),
        ] {
            let error = analyze_error(source);
            assert_eq!(error.code, code, "{}", error.message);
        }

        let named =
            parse("fn target(int left, int right) {} fn main() { target(right: 2, left: 1); }")
                .expect("parse repeated-type named call");
        analyze(&named).expect("named repeated-type call should type-check");
    }

    #[test]
    fn named_argument_plans_reject_optional_holes_without_compacting_abi_slots() {
        let parameters = vec![
            "required".to_owned(),
            "first_optional".to_owned(),
            "second_optional".to_owned(),
        ];
        let required = [true, false, false];
        let args = vec![
            Expr::IntLiteral(BigInt::one()),
            Expr::IntLiteral(BigInt::from(3_u32)),
        ];
        let names = vec!["required".to_owned(), "second_optional".to_owned()];
        let error = reorder_call_arguments(
            "internal_optional_fixture",
            &args,
            Some(&names),
            false,
            &parameters,
            &required,
            None,
        )
        .expect_err("a later optional argument cannot occupy an earlier omitted ABI slot");
        assert_eq!(error.code, "E_NAMED_ARGUMENT_HOLE");
        assert!(error.message.contains("first_optional"));
        assert!(error.message.contains("second_optional"));

        let trailing_names = vec!["required".to_owned()];
        let trailing = reorder_call_arguments(
            "internal_optional_fixture",
            &args[..1],
            Some(&trailing_names),
            false,
            &parameters,
            &required,
            None,
        )
        .expect("omitting only a trailing optional suffix remains canonical");
        assert_eq!(trailing.ordered.len(), 1);
        assert_eq!(trailing.evaluation_order, [0]);

        let interior_optional_parameters = vec![
            "required".to_owned(),
            "optional_payload".to_owned(),
            "required_trailer".to_owned(),
        ];
        let interior_required = [true, false, true];
        let interior_args = vec![
            Expr::IntLiteral(BigInt::one()),
            Expr::IntLiteral(BigInt::from(2_u32)),
        ];
        let interior_names = vec!["required".to_owned(), "required_trailer".to_owned()];
        let interior = reorder_call_arguments(
            "compacted_optional_fixture",
            &interior_args,
            Some(&interior_names),
            false,
            &interior_optional_parameters,
            &interior_required,
            None,
        )
        .expect("a required trailer unambiguously follows an omitted optional payload");
        assert_eq!(interior.ordered.len(), 2);
        assert_eq!(interior.evaluation_order, [0, 1]);
    }

    #[test]
    fn privileged_and_effectful_calls_with_three_parameters_require_names() {
        let privileged = analyze_error(
            "kotoage fn publish(int first, string second, bool third) authorize(\"Publish\") {} fn main() { publish(1, \"two\", true); }",
        );
        assert_eq!(privileged.code, "E_NAMED_ARGUMENTS_REQUIRED");

        let effectful = analyze_error(
            "fn main(AccountId account, Name key, Json value) { ledger::account::set_detail(account, key, value); }",
        );
        assert_eq!(effectful.code, "E_NAMED_ARGUMENTS_REQUIRED");

        let transitive = analyze_error(
            "fn sink(AccountId account, Name key, Json value) { ledger::account::set_detail(account: account, key: key, value: value); } fn wrapper(AccountId account, Name key, Json value) { sink(account: account, key: key, value: value); } fn main(AccountId account, Name key, Json value) { wrapper(account, key, value); }",
        );
        assert_eq!(transitive.code, "E_NAMED_ARGUMENTS_REQUIRED");
    }

    #[test]
    fn named_method_arguments_do_not_mix_with_the_receiver() {
        let program = parse(
            "fn lookup(Json object, Name key) -> Option<int> { return object.get_int(key: key); }",
        )
        .expect("parse named method call");
        analyze(&program).expect("implicit receiver must not count as a positional argument");
    }

    #[test]
    fn pagination_calls_require_offset_and_limit_names() {
        let positional =
            analyze_error("fn page(Name path) -> bytes { return state::keys(path, 0, 10); }");
        assert_eq!(positional.code, "E_NAMED_ARGUMENTS_REQUIRED");

        let named = parse(
            "fn page(Name path) -> bytes { return state::keys(limit: 10, path: path, offset: 0); }",
        )
        .expect("parse named pagination call");
        analyze(&named).expect("named pagination call should type-check");
    }

    #[test]
    fn duplicate_top_level_declarations_are_rejected() {
        let cases = [
            (
                "fn repeated() {} fn repeated() {}",
                "duplicate function `repeated`",
            ),
            (
                "struct Repeated { int value; } struct Repeated { int value; }",
                "duplicate type `Repeated`",
            ),
            (
                "state int repeated; state int repeated;",
                "duplicate state `repeated`",
            ),
            (
                "const int repeated = 1; const int repeated = 2;",
                "duplicate const `repeated`",
            ),
        ];

        for (source, expected) in cases {
            let err = analyze_error(source);
            assert_eq!(err.message, expected);
        }
    }

    #[test]
    fn cross_kind_declaration_collisions_are_rejected() {
        let err = analyze_error("struct Shared { int value; } fn Shared() {}");
        assert_eq!(err.code, "E_DUPLICATE_DECLARATION");
        assert_eq!(
            err.message,
            "declaration name `Shared` is already used by a type"
        );
    }

    #[test]
    fn compiler_owned_declaration_names_are_rejected() {
        for (source, expected) in [
            (
                "fn account_id(string value) -> int { return 1; }",
                "function `account_id` uses a compiler-reserved name",
            ),
            (
                "fn __kotodama_link_private() {}",
                "function `__kotodama_link_private` uses a compiler-reserved name",
            ),
            (
                "struct Option { int value; }",
                "type `Option` uses a compiler-reserved name",
            ),
        ] {
            let error = analyze_error(source);
            assert_eq!(error.code, "E_RESERVED_DECLARATION");
            assert_eq!(error.message, expected);
        }
    }

    #[test]
    fn duplicate_function_parameters_are_rejected() {
        let err = analyze_error("fn repeated(int value, bool value) {}");
        assert_eq!(
            err.message,
            "duplicate parameter `value` in function `repeated`"
        );
    }

    #[test]
    fn duplicate_struct_fields_are_rejected() {
        let err = analyze_error("struct Repeated { int value; bool value; }");
        assert_eq!(err.message, "duplicate field `value` in type `Repeated`");
    }

    #[test]
    fn error_codes_are_contract_global_and_require_is_typed() {
        let duplicate = analyze_error(
            "error enum Payment { Unauthorized = 1001 } \
             error enum Settlement { Expired = 1001 }",
        );
        assert_eq!(
            duplicate.message,
            "error code 1001 is assigned to both `Payment::Unauthorized` and `Settlement::Expired`"
        );

        let accepted = parse(
            "error enum Payment { Unauthorized = 1001 } \
             fn pay(bool allowed) { require(allowed, Payment::Unauthorized); }",
        )
        .expect("parse typed require");
        let typed = analyze(&accepted).expect("declared error variant is accepted");
        assert_eq!(typed.error_codes.len(), 1);
        assert_eq!(typed.error_codes[0].namespace, "Payment");
        assert_eq!(typed.error_codes[0].name, "Unauthorized");
        assert_eq!(typed.error_codes[0].code, 1001);

        for invalid in [
            "require(true);",
            "require(true, 1001);",
            "require(true, \"unauthorized\");",
            "require(true, Payment::Missing);",
        ] {
            let program = parse(&format!(
                "error enum Payment {{ Unauthorized = 1001 }} fn pay() {{ {invalid} }}"
            ))
            .expect("invalid require shape still parses");
            let error = analyze(&program).expect_err("untyped require must fail");
            assert!(
                error.message.contains("require") || error.message.contains("error variant"),
                "unexpected error for `{invalid}`: {error:?}"
            );
        }
    }

    #[test]
    fn semantic_analysis_rejects_ast_parameters_without_types() {
        let mut program = parse("fn f(int value) {}").expect("parse typed parameter");
        let Item::Function(function) = &mut program.items[0] else {
            panic!("expected function")
        };
        function.params[0].ty = None;
        let err = analyze(&program).expect_err("typeless parameter AST must be rejected");
        assert_eq!(err.message, "parameter `value` requires an explicit type");
    }

    #[test]
    fn semantic_analysis_rejects_ast_consts_without_types() {
        let mut program = parse("const int VALUE = 1;").expect("parse typed const");
        let Item::Const(declaration) = &mut program.items[0] else {
            panic!("expected const")
        };
        declaration.ty = None;
        let err = analyze(&program).expect_err("typeless const AST must be rejected");
        assert_eq!(err.message, "const `VALUE` requires an explicit type");
    }

    #[test]
    fn unknown_path_and_generic_types_are_rejected() {
        let path_err = analyze_error("fn use_missing(Missing value) {}");
        assert_eq!(path_err.message, "unknown type `Missing`");

        let generic_err = analyze_error("fn generic(Missing<int> value) {}");
        assert_eq!(generic_err.message, "unknown generic type `Missing`");
    }

    #[test]
    fn opaque_host_capability_types_are_not_source_types() {
        for name in [
            "AxtDescriptor",
            "AssetHandle",
            "ProofBlob",
            "SoracloudRequest",
            "SoracloudResponse",
        ] {
            let error = analyze_error(&format!("fn f({name} value) {{}}"));
            assert_eq!(error.message, format!("unknown type `{name}`"));
        }
    }

    #[test]
    fn option_and_result_type_expressions_are_recognized() {
        let context = SemanticContext::new();
        let option = TypeExpr::Generic {
            base: "Option".into(),
            args: vec![TypeExpr::Path("int".into())],
        };
        assert_eq!(
            convert_type_expr(&context, &option).expect("Option type"),
            Type::Option(Box::new(Type::Int))
        );

        let result = TypeExpr::Generic {
            base: "Result".into(),
            args: vec![TypeExpr::Path("int".into()), TypeExpr::Path("bool".into())],
        };
        assert_eq!(
            convert_type_expr(&context, &result).expect("Result type"),
            Type::Result(Box::new(Type::Int), Box::new(Type::Bool))
        );

        let helpers = parse(
            "fn option_helper(Option<int> value) {} \
             fn result_helper(Result<int, bool> value) {}",
        )
        .expect("private helper types parse");
        analyze(&helpers).expect("private helpers accept Option/Result parameters");

        let public = parse(
            "seiyaku Demo { kotoage fn call(Option<int> value, Result<int, bool> outcome) authorize(\"Call\") {} }",
        )
        .expect("public sum parameters parse");
        analyze(&public).expect("one-shot V1 argument records support Option and Result");

        let unsupported = analyze_error(
            "seiyaku Demo { kotoage fn call(StateMap<int, int> value) authorize(\"Call\") {} }",
        );
        assert!(
            unsupported
                .message
                .contains("unsupported V1 boundary type `StateMap<int, int>`"),
            "unexpected error: {}",
            unsupported.message
        );
    }

    #[test]
    fn forward_declared_struct_types_are_accepted() {
        let program = parse(
            "struct First { Second second; } \
             struct Second { int value; } \
             fn read(First first) -> int { return first.second.value; }",
        )
        .expect("source should parse");
        analyze(&program).expect("forward-declared struct references should resolve");
    }

    #[test]
    fn reusable_context_clears_all_declaration_registries() {
        let context = SemanticContext::new();
        let declared = parse(
            "struct SessionOnly { int value; } \
             fn read(SessionOnly value) -> int { return value.value; }",
        )
        .expect("declared source");
        context.analyze(&declared).expect("first analysis");

        let undeclared = parse("fn read(SessionOnly value) -> int { return value.value; }")
            .expect("undeclared source parses");
        let error = context
            .analyze(&undeclared)
            .expect_err("the previous source's type must not leak");
        assert_eq!(error.message, "unknown type `SessionOnly`");

        context
            .analyze(&declared)
            .expect("context remains reusable after a failed analysis");
    }

    #[test]
    fn internal_named_struct_references_are_nominal() {
        let alpha = Type::NamedStruct("Alpha".to_string());
        let another_alpha = Type::NamedStruct("Alpha".to_string());
        let beta = Type::NamedStruct("Beta".to_string());

        ensure_assignable(&alpha, &another_alpha)
            .expect("same named struct reference should be assignable");
        let err = ensure_assignable(&alpha, &beta)
            .expect_err("unrelated named struct references must not be assignable");
        assert!(err.message.contains("expected Alpha, got Beta"));
    }

    #[test]
    fn cyclic_value_structs_are_rejected_before_resolution() {
        let direct = analyze_error("struct Node { Node next; } state Node root;");
        assert_eq!(
            direct.message,
            "cyclic value struct definition: Node -> Node"
        );

        let indirect = analyze_error(
            "struct Left { Right right; } \
             struct Right { Left left; } \
             state Left root;",
        );
        assert_eq!(
            indirect.message,
            "cyclic value struct definition: Left -> Right -> Left"
        );
    }

    #[test]
    fn get_private_input_requires_build_configured_zk_mode() {
        let err = analyze_error("fn read() -> int { return crypto::private_input(0); }");
        assert_eq!(
            err.message,
            "builtin `crypto::private_input` requires ZK mode in compiler build configuration"
        );

        let source = r#"
            seiyaku ZkContract {
                fn read() -> Secret<int> { return crypto::private_input(0); }
            }
            "#;
        let program = parse(source).expect("ZK-enabled source should parse");
        SemanticContext::with_zk_enabled(true)
            .analyze(&program)
            .expect("build-configured ZK mode should permit private input access");
    }

    #[test]
    fn return_type_match() {
        let ok1 = analyze(&parse("fn f() -> bool { return true; } ").unwrap());
        assert!(ok1.is_ok());
        let ok2 = analyze(&parse("fn g() -> int { return 1; } ").unwrap());
        assert!(ok2.is_ok());
        let ok3 = analyze(&parse("fn h() { return; } ").unwrap());
        assert!(ok3.is_ok());
    }

    #[test]
    fn return_type_mismatch() {
        let err = analyze(&parse("fn f() -> bool { return 1; } ").unwrap());
        assert!(err.is_err());
        let err2 = analyze(&parse("fn h() { return 1; } ").unwrap());
        assert!(err2.is_err());
    }

    #[test]
    fn non_unit_must_return_all_paths() {
        let err = analyze(&parse("fn f() -> int { if true { return 1; } } ").unwrap());
        assert!(err.is_err());
        let ok =
            analyze(&parse("fn g() -> int { if true { return 1; } else { return 2; } } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn return_value_requires_declared_type() {
        let err = analyze(&parse("fn f() { return 1; } ").unwrap());
        assert!(err.is_err());
        let ok = analyze(&parse("fn g() { return; } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn param_type_enforcement_primitives() {
        // Boolean-to-integer coercion is intentionally absent from V1.
        let bool_arithmetic = analyze(&parse("fn f(bool x) { let y = x + 1; } ").unwrap());
        assert!(bool_arithmetic.is_err());
        // string param cannot be used in arithmetic
        let err2 = analyze(&parse("fn g(string s) { let y = s + 1; } ").unwrap());
        assert!(err2.is_err());
        // Canonical parameters always declare their type.
        let ok = analyze(&parse("fn h(int x, int y) -> int { return x + y; } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn typed_id_parameters_reject_arithmetic() {
        // Typed ledger identifiers are not numeric.
        let err = analyze(&parse("fn f(AccountId who) { let y = who + 1; } ").unwrap());
        assert!(err.is_err());
        // Equality on same named struct references is allowed
        let ok =
            analyze(&parse("fn g(AccountId a, AccountId b) -> bool { return a == b; } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn tuple_bindings_flatten_members() {
        let program = parse("fn f() { let pair = (1, 2); } ").unwrap();
        let typed = analyze(&program).expect("analysis ok");
        let TypedItem::Function(func) = &typed.items[0];
        let names: Vec<String> = func
            .body
            .statements
            .iter()
            .filter_map(|stmt| match stmt {
                TypedStatement::Let { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        let suffixes: Vec<String> = names
            .into_iter()
            .map(|name| name.rsplit("::").next().unwrap().to_string())
            .collect();
        assert_eq!(suffixes, vec!["pair", "pair#0", "pair#1"]);
    }

    #[test]
    fn struct_destructuring_uses_declaration_order_for_out_of_order_literals() {
        let program = parse(
            "struct Pair { int first, string second } \
             fn f() { \
                 let pair = Pair { second: \"two\", first: 1 }; \
                 let (left, right) = Pair { second: \"four\", first: 3 }; \
             }",
        )
        .expect("parse named struct literals");
        let typed = analyze(&program).expect("analyze named struct literals");
        let TypedItem::Function(function) = &typed.items[0];

        let binding = |suffix: &str| {
            function
                .body
                .statements
                .iter()
                .find_map(|statement| match statement {
                    TypedStatement::Let { name, value }
                        if name.rsplit("::").next() == Some(suffix) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing binding `{suffix}`"))
        };

        let is_projection = |value: &TypedExpr, base: Option<&str>, index: &str| {
            matches!(
                &value.expr,
                ExprKind::Member { object, field }
                    if field == index
                        && matches!(
                            object.kind(),
                            ExprKind::Ident(name)
                                if base.is_none_or(|base| {
                                    name.rsplit("::").next() == Some(base)
                                })
                        )
            )
        };
        assert!(is_projection(binding("pair#0"), Some("pair"), "0"));
        assert!(is_projection(binding("pair#1"), Some("pair"), "1"));
        assert!(is_projection(binding("left"), None, "0"));
        assert!(is_projection(binding("right"), None, "1"));
    }

    #[test]
    fn state_map_iteration_accepts_pointer_keys() {
        let program = parse(
            "state StateMap<Name, int> Items; \
             fn main() { \
                 for (k, v) in Items.take(1) { \
                     let _x = v; \
                 } \
             }",
        )
        .expect("parse state map");
        analyze(&program).expect("canonical StateMap iteration supports typed pointer keys");
    }

    #[test]
    fn static_state_map_iteration_limit_is_inclusive_and_fail_closed() {
        for iteration in ["M.take(64)", "M.range(10, 74)"] {
            let program = parse(&format!(
                "state StateMap<int, int> M; \
                 fn main() {{ for (key, value) in {iteration} {{ let _value = value; }} }}"
            ))
            .expect("boundary iteration source parses");
            analyze(&program).unwrap_or_else(|error| {
                panic!("boundary iteration `{iteration}` must be accepted: {error:?}")
            });
        }

        for (iteration, expected_form) in [
            ("M.take(65)", "StateMap.take(N)"),
            ("M.range(10, 75)", "StateMap.range(start, end)"),
        ] {
            let program = parse(&format!(
                "state StateMap<int, int> M; \
                 fn main() {{ for (key, value) in {iteration} {{ let _value = value; }} }}"
            ))
            .expect("over-limit iteration source parses");
            let error = analyze(&program).expect_err("bound above 64 must fail semantically");
            assert_eq!(error.code, "E_ITERATION_LIMIT");
            assert_eq!(
                error.message,
                format!("`{expected_form}` span 65 exceeds the Kotodama V1 limit 64")
            );
        }
    }

    #[test]
    fn dynamic_map_take_rejects_non_literal_bounds() {
        let program = parse(
            "state StateMap<int, int> M; \
             fn main(int n) { \
                 for (k, v) in M.take(n) { \
                     let _x = v; \
                 } \
             }",
        )
        .expect("parse dynamic take");
        let error = analyze(&program).expect_err("dynamic take must fail closed in V1");
        assert!(
            error
                .message
                .contains("requires a non-negative int literal")
        );
    }

    #[test]
    fn dynamic_map_range_rejects_non_literal_bounds() {
        let program = parse(
            "state StateMap<int, int> M; \
             fn main(int start, int end) { \
                 for (k, v) in M.range(start, end) { \
                     let _x = v; \
                 } \
             }",
        )
        .expect("parse dynamic range");
        let error = analyze(&program).expect_err("dynamic range must fail closed in V1");
        assert!(error.message.contains("requires non-negative int literals"));
    }

    #[test]
    fn state_map_alias_is_rejected() {
        let program = parse(
            "state StateMap<int, int> M; \
             fn main() { \
                 let m = M; \
             }",
        )
        .expect("parse state map alias");
        let err = analyze(&program).expect_err("aliasing a state map should error");
        assert_eq!(err.code, "E_STATE_MAP_ALIAS");
    }

    #[test]
    fn state_map_reassignment_is_rejected() {
        let program = parse(
            "state StateMap<int, int> M; \
             fn main() { \
                 M = StateMap::new(); \
             }",
        )
        .expect("parse state map reassignment");
        let err = analyze(&program).expect_err("reassigning a state map should error");
        assert_eq!(err.code, "E_STATE_MAP_ALIAS");
    }

    #[test]
    fn state_map_cannot_be_passed_to_user_fn() {
        let program = parse(
            "state StateMap<int, int> M; \
             fn f(StateMap<int, int> m) { let _x = 0; } \
             fn main() { f(M); }",
        )
        .expect("parse state map arg");
        let err = analyze(&program).expect_err("passing state map to user fn should error");
        assert_eq!(err.code, "E_STATE_MAP_ALIAS");
    }

    #[test]
    fn scalar_state_requires_hajimari() {
        let err = analyze_error("state int counter; fn read() -> int { return counter; }");
        assert_eq!(err.code, "E_STATE_HAJIMARI_REQUIRED");
        assert_eq!(
            err.message,
            "seiyaku scalar state requires a `hajimari()`/`始まり()` declaration"
        );
    }

    #[test]
    fn scalar_state_hajimari_reports_every_missing_write() {
        let err = analyze_error("state int first; state int second; hajimari() { first = 0; }");
        assert_eq!(err.code, "E_STATE_HAJIMARI_INCOMPLETE");
        assert_eq!(
            err.message,
            "hajimari() must initialize every scalar state on every normal return or fallthrough path; missing: second"
        );
    }

    #[test]
    fn scalar_state_initialization_intersects_conditional_paths() {
        let accepted = parse(
            "state int value; \
             hajimari() { if true { value = 1; } else { value = 2; } }",
        )
        .expect("parse complete conditional hajimari");
        analyze(&accepted).expect("both conditional paths initialize scalar state");

        let err = analyze_error(
            "state int value; \
             hajimari() { if true { value = 1; } }",
        );
        assert_eq!(err.code, "E_STATE_HAJIMARI_INCOMPLETE");
    }

    #[test]
    fn scalar_state_initialization_checks_early_returns() {
        let err = analyze_error(
            "state int value; \
             hajimari() { if true { return; } value = 1; }",
        );
        assert_eq!(err.code, "E_STATE_HAJIMARI_INCOMPLETE");

        let accepted = parse(
            "state int value; \
             hajimari() { if true { value = 1; return; } value = 2; }",
        )
        .expect("parse initialized early return");
        analyze(&accepted).expect("every normal exit initializes scalar state");
    }

    #[test]
    fn scalar_state_initialization_does_not_trust_optional_execution() {
        let loop_error = analyze_error(
            "state int value; \
             hajimari() { for index in range(1) { value = index; } }",
        );
        assert_eq!(loop_error.code, "E_STATE_HAJIMARI_INCOMPLETE");

        let short_circuit_error = analyze_error(
            "state int value; \
             fn seed() -> bool { value = 1; return true; } \
             hajimari() { let ignored = false && seed(); }",
        );
        assert_eq!(short_circuit_error.code, "E_STATE_HAJIMARI_INCOMPLETE");
    }

    #[test]
    fn scalar_state_hajimari_accepts_transitive_complete_initialization() {
        let program = parse(
            "state int counter; \
             struct Ledger { int total; } \
             state Ledger ledger; \
             fn seed() { counter = 0; ledger = Ledger { total: 0 }; } \
             hajimari() { seed(); }",
        )
        .expect("parse transitive scalar hajimari");
        analyze(&program).expect("transitive hajimari writes should initialize every scalar state");
    }

    #[test]
    fn map_assignment_requires_map_target() {
        let program = parse("fn f() { let x = 1; x[0] = 2; }").expect("parse map assignment");
        let err = analyze(&program).expect_err("non-map assignment should error");
        assert!(err.message.contains("map assignment expects StateMap<K,V>"));
    }

    #[test]
    fn assignment_rejects_bool_to_int() {
        let program =
            parse("fn f() { var int x = true; x = false; }").expect("parse bool assignment");
        analyze(&program).expect_err("bool assignment must not coerce to int");
    }

    #[test]
    fn immutable_local_reassignment_is_rejected() {
        let err = analyze_error("fn f() { let value = 1; value = 2; }");
        assert_eq!(err.code, "E_IMMUTABLE_ASSIGNMENT");
        assert_eq!(
            err.message,
            "cannot assign to immutable binding `value`; declare a mutable local with `var`"
        );
    }

    #[test]
    fn mutable_local_reassignment_is_accepted() {
        let program = parse("fn f() -> int { var value = 1; value += 2; return value; }")
            .expect("parse mutable binding");
        analyze(&program).expect("var bindings should permit reassignment");
    }

    #[test]
    fn function_parameters_are_immutable() {
        let err = analyze_error("fn f(int value) { value = 2; }");
        assert_eq!(err.code, "E_IMMUTABLE_ASSIGNMENT");
        assert_eq!(
            err.message,
            "cannot assign to immutable binding `value`; declare a mutable local with `var`"
        );
    }

    #[test]
    fn local_declarations_cannot_duplicate_or_shadow_bindings() {
        for source in [
            "fn f() { let value = 1; let value = 2; }",
            "fn f(int value) { let value = 2; }",
            "fn f() { let (left, left) = (1, 2); }",
        ] {
            analyze_error(source);
        }
    }

    #[test]
    fn parameters_and_locals_cannot_shadow_any_source_declaration() {
        for source in [
            "seiyaku App { fn helper() {} fn inspect(int helper) {} }",
            "seiyaku App { struct Receipt { int value; } fn inspect() { let Receipt = 1; } }",
            "seiyaku App { fn inspect() { let App = 1; } }",
        ] {
            let program = parse(source).expect("parse global shadowing fixture");
            let error = analyze(&program).expect_err("global shadowing must be rejected");
            assert!(
                error.message.contains("shadows a source declaration"),
                "unexpected shadowing error for {source}: {error:?}"
            );
        }
    }

    #[test]
    fn source_unit_identity_cannot_be_redeclared_inside_the_unit() {
        let program = parse("seiyaku App { fn App() {} }").expect("parse identity collision");
        let error = analyze(&program).expect_err("unit identity collision must be rejected");
        assert!(
            error.message.contains("already used by a source unit"),
            "{error:?}"
        );
    }

    #[test]
    fn break_requires_loop_context() {
        let program = parse("fn f() { break; }").expect("parse break");
        let err = analyze(&program).expect_err("break outside loop should error");
        assert_eq!(err.code, "E_BREAK_OUTSIDE_LOOP");
    }

    #[test]
    fn continue_requires_loop_context() {
        let program = parse("fn f() { continue; }").expect("parse continue");
        let err = analyze(&program).expect_err("continue outside loop should error");
        assert_eq!(err.code, "E_CONTINUE_OUTSIDE_LOOP");
    }

    #[test]
    fn state_shadowing_is_rejected_in_let() {
        let program =
            parse("state int counter; fn f() { let counter = 1; }").expect("parse shadowing let");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert_eq!(err.code, "E_STATE_SHADOWED");
    }

    #[test]
    fn state_shadowing_is_rejected_in_params() {
        let program =
            parse("state int counter; fn f(int counter) {}").expect("parse shadowing param");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert_eq!(err.code, "E_STATE_SHADOWED");
    }

    #[test]
    fn state_shadowing_is_rejected_in_map_loop_vars() {
        let program = parse(
            "state int counter; state StateMap<int, int> M; \
             fn f() { for (counter, v) in M.take(1) { let _x = v; } }",
        )
        .expect("parse shadowing loop vars");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert_eq!(err.code, "E_STATE_SHADOWED");
    }

    #[test]
    fn c_style_for_is_rejected_before_semantic_analysis() {
        for source in [
            "fn f() { for let pair = (1, 2); pair.0 < 3; {} }",
            "fn f() { for let i = 0; i < 1; let pair = (1, 2) {} }",
        ] {
            let err = parse(source).expect_err("C-style loops are outside the V1 surface");
            assert!(err.contains("only `for item in range(end)`"));
        }
    }

    #[test]
    fn manually_constructed_while_ast_cannot_bypass_v1_frontend_rules() {
        let mut program = parse("fn f() {}").expect("parse base program");
        let Item::Function(function) = &mut program.items[0] else {
            panic!("expected function item");
        };
        function.body.statements.push(Statement::While {
            cond: Expr::Bool(true),
            body: Block {
                statements: Vec::new(),
                tail: None,
            },
        });
        let error = analyze(&program).expect_err("while AST must fail closed");
        assert_eq!(error.code, "E_UNBOUNDED_LOOP");
    }

    #[test]
    fn manually_constructed_dynamic_for_ast_cannot_bypass_v1_frontend_rules() {
        let mut program = parse("fn f() {}").expect("parse base program");
        let Item::Function(function) = &mut program.items[0] else {
            panic!("expected function item");
        };
        function.body.statements.push(Statement::For {
            line: 1,
            init: Some(Box::new(Statement::Let {
                mutable: true,
                pat: Pattern::Name("i".to_owned()),
                ty: None,
                value: Expr::IntLiteral(BigInt::zero()),
            })),
            cond: Some(Expr::Binary {
                op: BinaryOp::Lt,
                left: Box::new(Expr::Ident("i".to_owned())),
                right: Box::new(Expr::Ident("dynamic_bound".to_owned())),
            }),
            step: Some(Box::new(Statement::Assign {
                name: "i".to_owned(),
                value: Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Ident("i".to_owned())),
                    right: Box::new(Expr::IntLiteral(BigInt::one())),
                },
            })),
            body: Block {
                statements: Vec::new(),
                tail: None,
            },
        });
        let error = analyze(&program).expect_err("dynamic for AST must fail closed");
        assert_eq!(error.code, "E_UNBOUNDED_LOOP");
    }

    #[test]
    fn equality_rejects_tuple_types() {
        let program = parse("fn f() { let a = (1, 2); let b = (1, 2); let _x = a == b; }")
            .expect("parse tuple equality");
        let err = analyze(&program).expect_err("tuple equality should error");
        assert!(err.message.contains("equality is not supported"));
    }

    #[test]
    fn pointer_constructor_accepts_string_binding() {
        let program = parse("fn f() { let s = \"wonderland\"; let _n = Name::parse(s); }")
            .expect("parse pointer constructor");
        analyze(&program).expect("string binding should be allowed");
    }

    #[test]
    fn flat_pointer_constructor_spellings_are_rejected() {
        for (flat, canonical) in [
            ("account_id", "AccountId::parse"),
            ("asset_definition", "AssetDefinitionId::parse"),
            ("asset_id", "AssetId::parse"),
            ("nft_id", "NftId::parse"),
            ("name", "Name::parse"),
            ("json", "Json::parse"),
            ("domain_id", "DomainId::parse"),
            ("dataspace_id", "DataSpaceId::parse"),
        ] {
            let source = format!("fn f() {{ let _value = {flat}(\"x\"); }}");
            let program = parse(&source).expect("flat builtin call parses before resolution");
            let error = analyze(&program).expect_err("flat builtin spelling must fail closed");
            assert!(
                error
                    .message
                    .contains("legacy or non-canonical builtin spelling"),
                "{error:?}"
            );
            assert!(error.message.contains(canonical), "{error:?}");
        }
    }

    #[test]
    fn flat_builtin_spellings_are_rejected_in_favour_of_namespaces() {
        for (flat_call, canonical) in [
            ("wrapping_add(left: 1, right: 2)", "math::wrapping_add"),
            ("info(1)", "debug::info"),
            ("assert(true)", "test::assert"),
            ("assert_eq(actual: 1, expected: 1)", "test::assert_eq"),
            ("actor_account(\"issuer\")", "test::actor_account"),
            (
                "invoke_entrypoint(\"run\", Json::parse(\"{}\"))",
                "test::invoke_kotoage",
            ),
            ("trigger_event()", "context::trigger_event"),
        ] {
            let source = format!("fn f() {{ let _value = {flat_call}; }}");
            let program = parse(&source).expect("flat builtin call parses before resolution");
            let error = SemanticContext::with_capabilities(false, true)
                .analyze(&program)
                .expect_err("flat builtin spelling must fail closed");
            assert!(
                error
                    .message
                    .contains("legacy or non-canonical builtin spelling"),
                "{error:?}"
            );
            assert!(error.message.contains(canonical), "{error:?}");
        }
    }

    #[test]
    fn japanese_branded_capability_segments_normalize_to_the_canonical_registry() {
        let program = parse(
            r#"
            seiyaku BrandedCapabilities {
                kotoage fn inspect() authorize("Inspect") {
                    let _selector = context::言挙げ();
                    ledger::誓約::grant_kotoage(
                        account: context::authority(),
                        言挙げ: "inspect",
                    );
                }
            }
            "#,
        )
        .expect("parse Japanese branded capability path");
        analyze(&program).expect("Japanese capability segments must resolve canonically");
    }

    #[test]
    fn canonical_builtin_diagnostics_replace_only_identifier_tokens() {
        assert_eq!(
            replace_identifier_token(
                "info expects a value; compiler configuration is unchanged",
                "info",
                "debug::info",
            ),
            "debug::info expects a value; compiler configuration is unchanged"
        );
        assert_eq!(
            replace_identifier_token(
                "__invoke_entrypoint__run targets invoke_entrypoint",
                "invoke_entrypoint",
                "test::invoke_kotoage",
            ),
            "__invoke_entrypoint__run targets test::invoke_kotoage"
        );
    }

    #[test]
    fn for_body_bindings_do_not_escape_loop() {
        let program = parse(
            "fn f() { \
                for i in range(1) { \
                    let x = 1; \
                } \
                let _y = x; \
            }",
        )
        .expect("parse for loop");
        let err = analyze(&program).expect_err("body bindings should not escape");
        assert!(err.message.contains("undefined variable"));
    }

    #[test]
    fn tuple_pattern_requires_tuple_type() {
        let program = parse("fn f() { let (a, b) = 1; }").expect("parse tuple pattern");
        let err = analyze(&program).expect_err("non-tuple destructuring should error");
        assert!(err.message.contains("tuple destructuring expects a tuple"));
    }

    #[test]
    fn tuple_pattern_requires_arity_match() {
        let program = parse("fn f() { let (a, b, c) = (1, 2); }").expect("parse tuple pattern");
        let err = analyze(&program).expect_err("tuple arity mismatch should error");
        assert!(
            err.message
                .contains("tuple destructuring expects 2 bindings")
        );
    }

    #[test]
    fn struct_pattern_requires_arity_match() {
        let program = parse(
            "struct Pair { int a, int b } \
             fn f() { let (a) = Pair { a: 1, b: 2 }; }",
        )
        .expect("parse struct pattern");
        let err = analyze(&program).expect_err("struct arity mismatch should error");
        assert!(
            err.message
                .contains("struct destructuring expects 2 bindings")
        );
    }

    #[test]
    fn assert_rejects_extra_args() {
        let program = parse("fn f() { test::assert(true, false); }").expect("parse assert");
        let err = SemanticContext::with_capabilities(false, true)
            .analyze(&program)
            .expect_err("assert message type should error");
        assert!(
            err.message
                .contains("assert expects (bool) or (bool, string|int)")
        );
    }

    #[test]
    fn in_memory_map_constructor_is_rejected() {
        let program = parse("fn f() { let StateMap<Name, int> m = StateMap::new(); let _x = m; }")
            .expect("parse StateMap::new");
        let err = analyze(&program).expect_err("V1 StateMap values must be durable state");
        assert!(
            err.message
                .contains("StateMap values may only refer directly to top-level durable state")
                || err.code == "E_STATE_MAP_ALIAS"
                || err
                    .message
                    .contains("unknown function or builtin `StateMap::new`"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn bytes_equality_is_allowed() {
        let program =
            parse(r#"fn f() { let bytes b = b"hi"; let bytes c = b"hi"; let _x = b == c; }"#)
                .expect("parse bytes equality");
        analyze(&program).expect("bytes equality should be allowed");
    }

    #[test]
    fn bytes_literal_types_as_bytes() {
        let program = parse(r#"fn f() { let bytes b = b"ab"; }"#).expect("parse bytes literal");
        let typed = analyze(&program).expect("analyze bytes literal");
        let TypedItem::Function(f) = &typed.items[0];
        let stmt = f.body.statements.first().expect("statement present");
        match stmt {
            TypedStatement::Let { value, .. } => {
                assert!(matches!(value.expr, ExprKind::Bytes(_)));
                assert_eq!(value.ty, Type::Bytes);
            }
            other => panic!("expected let statement, got {other:?}"),
        }
    }

    #[test]
    fn state_map_key_type_is_validated() {
        let program = parse("state StateMap<Json, int> M; fn f() {}").expect("parse state map");
        let err = analyze(&program).expect_err("state map key should be validated");
        assert!(
            err.message
                .contains("StateMap key type `Json` is not supported"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn durable_state_map_key_domain_matches_generated_policy() {
        let supported = [
            Type::Int,
            Type::Decimal,
            Type::Quantity,
            Type::Bool,
            Type::String,
            Type::Bytes,
            Type::DataSpaceId,
            Type::AccountId,
            Type::AssetDefinitionId,
            Type::AssetId,
            Type::NftId,
            Type::DomainId,
            Type::Name,
        ];
        let expected_names = V1_STATE_MAP_KEY_TYPE_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            supported.iter().map(type_name).collect::<Vec<_>>(),
            expected_names
        );
        assert!(supported.iter().all(is_supported_durable_key_type));

        for unsupported in [
            Type::Json,
            Type::AxtDescriptor,
            Type::AssetHandle,
            Type::ProofBlob,
            Type::SoracloudRequest,
            Type::SoracloudResponse,
        ] {
            assert!(
                !is_supported_durable_key_type(&unsupported),
                "{} must not enter the durable key ABI",
                type_name(&unsupported)
            );
        }
    }

    #[test]
    fn field_assignment_is_rejected() {
        let program = parse("fn f() { let t = (1, 2); t.0 = 3; }").expect("parse field assignment");
        let err = analyze(&program).expect_err("field assignment should error");
        assert!(err.message.contains("assignment target must be"));
    }

    #[test]
    fn info_accepts_int() {
        let program = parse("fn f() { debug::info(42); }").expect("parse info");
        analyze(&program).expect("info should accept int");
    }

    #[test]
    fn view_entrypoints_reject_observable_debug_logging() {
        let program = parse("seiyaku Demo { view fn inspect() { debug::info(42); } }")
            .expect("parse debug logging in a view");
        let error = analyze(&program).expect_err("views must not emit observable logs");
        assert!(
            error.message.contains("view") && error.message.contains("host side effects"),
            "{error:?}"
        );
    }

    #[test]
    fn vector_length_control_is_not_a_source_builtin() {
        let program = parse("fn f() { runtime::set_vector_length(8); }").expect("parse setvl");
        let error = analyze(&program).expect_err("vector metadata is compiler-owned");
        assert!(
            error
                .message
                .contains("unknown function or builtin `runtime::set_vector_length`"),
            "{error:?}"
        );
    }

    #[test]
    fn trigger_event_accepts_no_args() {
        let program = parse(
            "fn f() { let ev = context::trigger_event(); let _kind = ev.get_name(Name::parse(\"kind\")); }",
        )
        .expect("parse trigger_event");
        analyze(&program).expect("trigger_event should type-check");
    }

    #[test]
    fn public_entrypoints_reject_trigger_event() {
        let program = parse(
            "seiyaku Demo { kotoage fn f() authorize(\"InspectTrigger\") { let _ev = context::trigger_event(); } }",
        )
            .expect("parse public trigger_event");
        let err = analyze(&program).expect_err("public trigger_event should fail");
        assert!(
            err.message
                .contains("cannot use `context::trigger_event` here"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn trigger_callbacks_accept_trigger_event_payload_helpers() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run() authorize("RunTrigger") {
                    let ev = context::trigger_event();
                    let _escrow_id = ev.get_name(Name::parse("escrow_id"));
                    let _condition_code = ev.get_int(Name::parse("condition_code"));
                }

                trigger wake -> run {
                    on execute trigger wake;
                }
            }
            "#,
        )
        .expect("parse trigger callback trigger_event");
        analyze(&program).expect("trigger callback trigger_event should type-check");
    }

    #[test]
    fn namespaced_trigger_callback_does_not_require_local_entrypoint() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn arm() authorize("ArmTrigger") {}

                trigger wake -> callee::run {
                    on time pre_commit;
                }
            }
            "#,
        )
        .expect("parse namespaced trigger callback");

        analyze(&program).expect("namespaced trigger callback target is resolved at activation");
    }

    #[test]
    fn namespaced_trigger_callback_does_not_mark_local_function_as_trigger_callback() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn arm() authorize("ArmTrigger") {}
                kotoage fn run() authorize("RunTrigger") {
                    let _ev = context::trigger_event();
                }

                trigger wake -> callee::run {
                    on time pre_commit;
                }
            }
            "#,
        )
        .expect("parse namespaced trigger callback");
        let err = analyze(&program)
            .expect_err("remote trigger callback must not permit local trigger_event access");

        assert!(
            err.message
                .contains("cannot use `context::trigger_event` here"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn invoke_entrypoint_accepts_test_functions() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(int count) -> int authorize("Run") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_kotoage("run", Json::parse("{\"count\": 7}"));
                    test::assert_eq(actual: next, expected: 8);
                }
            }
            "#,
        )
        .expect("parse invoke_entrypoint");
        analyze_test(&program).expect("invoke_entrypoint in tests should type-check");
    }

    #[test]
    fn invoke_entrypoint_rejects_non_test_functions() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(int count) -> int authorize("Run") { return count; }

                fn helper() {
                    let _next = test::invoke_kotoage("run", Json::parse("{\"count\": 7}"));
                }
            }
            "#,
        )
        .expect("parse non-test invoke_entrypoint");
        let err = analyze_test(&program).expect_err("non-test invoke_entrypoint should fail");
        assert!(err.message.contains("only available inside #[test]"));
    }

    #[test]
    fn invoke_entrypoint_accepts_name_literal_target() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(int count) -> int authorize("Run") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_kotoage(Name::parse("run"), Json::parse("{\"count\": 7}"));
                    test::assert_eq(actual: next, expected: 8);
                }
            }
            "#,
        )
        .expect("parse name literal invoke_entrypoint");
        analyze_test(&program).expect("name literal invoke_entrypoint should type-check");
    }

    #[test]
    fn invoke_entrypoint_rejects_non_literal_target() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(int count) -> int authorize("Run") { return count; }

                #[test]
                fn drive_run() {
                    let target = "run";
                    let _next = test::invoke_kotoage(target, Json::parse("{\"count\": 7}"));
                }
            }
            "#,
        )
        .expect("parse dynamic target invoke_entrypoint");
        let err = analyze_test(&program).expect_err("dynamic target should fail");
        assert!(
            err.message
                .contains("requires a literal public or lifecycle target")
        );
    }

    #[test]
    fn invoke_entrypoint_rejects_non_json_payload() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(int count) -> int authorize("Run") { return count; }

                #[test]
                fn drive_run() {
                    let _next = test::invoke_kotoage("run", 7);
                }
            }
            "#,
        )
        .expect("parse non-json payload invoke_entrypoint");
        let err = analyze_test(&program).expect_err("non-json payload should fail");
        assert!(err.message.contains("expects a Json payload"));
    }

    #[test]
    fn invoke_entrypoint_rejects_internal_target() {
        let program = parse(
            r#"
            seiyaku Demo {
                fn helper() -> int { return 7; }

                #[test]
                fn drive_run() {
                    let _next = test::invoke_kotoage("helper", Json::parse("{}"));
                }
            }
            "#,
        )
        .expect("parse internal target invoke_entrypoint");
        let err = analyze_test(&program).expect_err("internal target should fail");
        assert!(
            err.message
                .contains("may only target kotoage/view/hajimari/kaizen")
        );
    }

    #[test]
    fn invoke_entrypoint_as_and_actor_helpers_type_check_in_tests() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(int count) -> int authorize("Run") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_kotoage_as(
                        actor: "issuer",
                        kotoage: "run",
                        arguments: Json::parse("{\"count\": 7}"),
                    );
                    let acct = test::actor_account("issuer");
                    let pk = test::actor_public_key("issuer");
                    let sig = test::actor_sign("issuer", b"demo");
                    test::expect_reject_as(
                        actor: "issuer",
                        kotoage: "run",
                        arguments: Json::parse("{\"count\": -1}"),
                    );
                    let _ = (next, acct, pk, sig);
                }
            }
            "#,
        )
        .expect("parse invoke_entrypoint_as");
        analyze_test(&program).expect("test helpers should type-check");
    }

    #[test]
    fn invoke_entrypoint_as_accepts_tuple_returning_targets() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(int count) -> (int, int) authorize("Run") { return (count, count + 1); }

                #[test]
                fn drive_run() {
                    let _pair = test::invoke_kotoage_as(
                        actor: "issuer",
                        kotoage: "run",
                        arguments: Json::parse("{\"count\": 7}"),
                    );
                }
            }
            "#,
        )
        .expect("parse tuple invoke_entrypoint_as");
        analyze_test(&program).expect("tuple-returning target should type-check");
    }

    #[test]
    fn standalone_test_helpers_preserve_external_entrypoint_kind() {
        let target = parse(
            r#"
            seiyaku Demo {
                hajimari() {}
                kotoage fn run() authorize("Run") {}
                fn helper() {}
            }
            "#,
        )
        .expect("parse target contract");
        let signatures = SemanticContext::with_capabilities(false, true)
            .resolve_function_signatures(&target)
            .expect("resolve target signatures");

        let accepted = parse(
            r#"
            module Tests {
                #[test]
                fn invokes_lifecycle() {
                    test::invoke_kotoage_as(
                        actor: "issuer",
                        kotoage: "hajimari",
                        arguments: Json::parse("{}"),
                    );
                }
            }
            "#,
        )
        .expect("parse standalone test module");
        SemanticContext::with_capabilities(false, true)
            .analyze_with_external_functions(&accepted, &signatures)
            .expect("external lifecycle entrypoint should retain its kind");

        let rejected = parse(
            r#"
            module Tests {
                #[test]
                fn invokes_private_helper() {
                    test::invoke_kotoage_as(
                        actor: "issuer",
                        kotoage: "helper",
                        arguments: Json::parse("{}"),
                    );
                }
            }
            "#,
        )
        .expect("parse private-helper test module");
        let error = SemanticContext::with_capabilities(false, true)
            .analyze_with_external_functions(&rejected, &signatures)
            .expect_err("private target helper must not become an entrypoint");
        assert_eq!(error.code(), "E_TEST_ENTRYPOINT_KIND");

        let direct_call = parse(
            r#"
            module Tests {
                #[test]
                fn bypasses_contract_boundary() {
                    run();
                }
            }
            "#,
        )
        .expect("parse direct external-entrypoint call");
        let error = SemanticContext::with_capabilities(false, true)
            .analyze_with_external_functions(&direct_call, &signatures)
            .expect_err("external entrypoints must retain the contract-call boundary");
        assert_eq!(error.code(), "K2004");
    }

    #[test]
    fn actor_helpers_reject_non_test_functions() {
        let program = parse(
            r#"
            seiyaku Demo {
                fn helper() {
                    let _acct = test::actor_account("issuer");
                }
            }
            "#,
        )
        .expect("parse non-test actor helper");
        let err = analyze_test(&program).expect_err("actor helper outside test should fail");
        assert!(err.message.contains("only available inside #[test]"));
    }

    #[test]
    fn view_entrypoints_accept_explicit_json_getter_on_typed_json_parameter() {
        let program = parse(
            "seiyaku Demo { view fn f(Json ev) -> Option<int> { return ev.get_int(Name::parse(\"n\")); } }",
        )
        .expect("parse view get_int");
        analyze(&program).expect("typed Json parameters may use explicit JSON getters");
    }

    #[test]
    fn view_entrypoints_reject_ensure() {
        let program = parse(
            "seiyaku Demo { state StateMap<int, int> balances; view fn f() -> int { return balances.ensure(key: 7, default: 9); } }",
        )
        .expect("parse ensure");
        let err = analyze(&program).expect_err("view ensure should fail");
        assert!(
            err.message
                .contains("`view fn` functions cannot use mutating map helper `ensure`"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn view_entrypoints_accept_get_or() {
        let program = parse(
            "seiyaku Demo { state StateMap<int, int> balances; view fn f() -> int { return balances.get_or(key: 7, default: 9); } }",
        )
        .expect("parse get_or");
        analyze(&program).expect("view get_or should type-check");
    }

    #[test]
    fn state_map_get_returns_option_without_intercepting_user_get_function() {
        let program = parse(
            "seiyaku Demo { \
                state StateMap<int, int> balances; \
                fn get(int value) -> int { return value; } \
                view fn lookup(int key) -> Option<int> { return balances.get(key); } \
                view fn echo(int value) -> int { return get(value); } \
            }",
        )
        .expect("parse canonical and user-defined get calls");
        let typed = analyze(&program).expect("both get call forms should resolve unambiguously");
        let returns = typed
            .items
            .iter()
            .map(|item| match item {
                TypedItem::Function(function) => (function.name.as_str(), function.ret_ty.clone()),
            })
            .collect::<HashMap<_, _>>();
        assert_eq!(
            returns.get("lookup"),
            Some(&Some(Type::Option(Box::new(Type::Int))))
        );
        assert_eq!(returns.get("echo"), Some(&Some(Type::Int)));
    }

    #[test]
    fn state_map_reads_require_explicit_option_handling() {
        for (source, expected) in [
            (
                "seiyaku Demo { state StateMap<int, int> balances; view fn read() -> int { return balances[1]; } }",
                "E_STATE_MAP_OPTIONAL_READ",
            ),
            (
                "seiyaku Demo { state StateMap<int, int> balances; kotoage fn add() authorize(\"Write\") { balances[1] += 1; } }",
                "E_STATE_MAP_OPTIONAL_READ",
            ),
            (
                "seiyaku Demo { state StateMap<int, int> balances; view fn read() -> Option<int> { return get(balances, 1); } }",
                "unknown function or builtin `get`",
            ),
        ] {
            let program =
                parse(source).expect("invalid StateMap read should parse before resolution");
            let error = analyze(&program).expect_err("invalid StateMap read must fail closed");
            if expected.starts_with("E_") {
                assert_eq!(
                    error.code, expected,
                    "unexpected diagnostic code for `{source}`: {error:?}"
                );
            } else {
                assert!(
                    error.message.contains(expected),
                    "unexpected error for `{source}`: {error:?}"
                );
            }
        }

        let write = parse(
            "seiyaku Demo { state StateMap<int, int> balances; kotoage fn set(int key, int value) authorize(\"Write\") { balances[key] = value; } }",
        )
        .expect("parse indexed StateMap write");
        analyze(&write).expect("simple indexed StateMap assignment must remain valid");
    }

    #[test]
    fn state_map_remove_returns_option_for_scalar_values() {
        let program = parse(
            "seiyaku Demo { state StateMap<Name, int> balances; kotoage fn f(Name key) -> Option<int> authorize(\"WriteState\") { return balances.remove(key); } }",
        )
        .expect("parse StateMap.remove");
        let typed = analyze(&program).expect("scalar StateMap.remove should type-check");
        let function = typed
            .items
            .iter()
            .map(|item| match item {
                TypedItem::Function(function) => function,
            })
            .find(|function| function.name == "f")
            .expect("kotoage function");
        let (reads, writes) = function_state_accesses(function, &typed.states);
        assert!(reads.contains("state:balances"));
        assert!(writes.contains("state:balances"));
    }

    #[test]
    fn view_entrypoints_reject_state_map_remove() {
        let program = parse(
            "seiyaku Demo { state StateMap<int, int> balances; view fn f() -> Option<int> { return balances.remove(7); } }",
        )
        .expect("parse StateMap.remove in view");
        let err = analyze(&program).expect_err("view remove must fail");
        assert!(
            err.message
                .contains("view function `f` cannot perform durable state mutation"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn view_entrypoints_reject_direct_durable_state_assignment() {
        let program = parse(
            "seiyaku Demo { state int counter; hajimari() { counter = 0; } view fn f() -> int { counter = 1; return counter; } }",
        )
        .expect("parse direct durable state assignment");
        let err = analyze(&program).expect_err("view durable state assignment should fail");
        assert!(
            err.message
                .contains("view function `f` cannot perform durable state mutation"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn view_entrypoints_reject_state_map_mutation() {
        let program = parse(
            "seiyaku Demo { state StateMap<int, int> balances; view fn f() -> int { balances[7] = 9; return 1; } }",
        )
        .expect("parse state map mutation");
        let err = analyze(&program).expect_err("view state map mutation should fail");
        assert!(
            err.message
                .contains("view function `f` cannot perform durable state mutation"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn view_entrypoints_reject_transitive_durable_state_mutation() {
        let program = parse(
            "seiyaku Demo { state int counter; hajimari() { counter = 0; } fn helper() { counter = counter + 1; } view fn f() -> int { helper(); return counter; } }",
        )
        .expect("parse transitive durable state mutation");
        let err = analyze(&program).expect_err("view transitive durable mutation should fail");
        assert!(
            err.message.contains(
                "view function `f` cannot call `helper` because `helper` performs durable state mutation"
            ),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn compiler_internal_builtins_are_rejected_from_source() {
        for name in [
            "alloc",
            "domain",
            "blob",
            "norito_bytes",
            "soracloud_request",
            "soracloud_response",
            "grow_heap",
            "debug_print",
            "debug_log",
            "setvl",
            "keys_take2",
            "values_take2",
            "keys_values_take2",
            "get_merkle_path",
            "get_merkle_compact",
            "get_register_merkle_compact",
            "pointer_to_norito",
            "json_set_int_direct",
            "json_set_account_id_direct",
            "json_get_int_direct",
            "json_get_numeric_direct",
            "json_get_json_direct",
            "json_get_name_direct",
            "json_get_account_id_direct",
            "json_get_asset_definition_id_direct",
            "json_get_nft_id_direct",
            "json_get_blob_hex_direct",
            "build_path_key_norito_direct",
            "schema_encode_direct",
            "schema_decode_direct",
            "schema_info_direct",
            "numeric_to_int_direct",
            "numeric_add_direct",
            "numeric_sub_direct",
            "numeric_mul_direct",
            "numeric_div_direct",
            "numeric_rem_direct",
            "numeric_neg_direct",
            "numeric_eq_direct",
            "numeric_ne_direct",
            "numeric_lt_direct",
            "numeric_le_direct",
            "numeric_gt_direct",
            "numeric_ge_direct",
        ] {
            let program = parse(&format!("fn f() {{ {name}(); }}"))
                .expect("compiler-internal builtin name should parse as a call");
            let err = analyze(&program).expect_err("internal builtin must be source-inaccessible");
            assert!(
                err.message.contains("compiler-internal")
                    || err.message.contains("unknown function or builtin"),
                "unexpected error for {name}: {}",
                err.message
            );
        }
    }

    #[test]
    fn generic_execute_instruction_is_not_a_builtin() {
        let program = parse("fn f(bytes payload) { execute_instruction(payload); }")
            .expect("unknown call should parse before semantic resolution");
        let err = analyze(&program).expect_err("generic instruction execution must be unknown");
        assert_eq!(
            err.message,
            "unknown function or builtin `execute_instruction`"
        );
    }

    #[test]
    fn raw_namespaced_host_bridges_are_not_builtins() {
        for source_name in [
            "contract::call",
            "seiyaku::call",
            "runtime::set_vector_length",
            "debug::print_i64",
            "debug::log",
            "axt::begin",
            "axt::touch",
            "soracloud::read_committed_state",
            "soracloud::read_secret",
        ] {
            let Ok(program) = parse(&format!("fn f() {{ {source_name}(); }}")) else {
                assert!(matches!(source_name, "contract::call" | "seiyaku::call"));
                continue;
            };
            let error = analyze(&program).expect_err("raw host bridge must fail closed");
            assert!(
                error.message.contains("unknown function or builtin"),
                "unexpected error for {source_name}: {error:?}"
            );
        }
    }

    #[test]
    fn noncanonical_crypto_aliases_are_rejected() {
        for alias in ["sm::hash", "sm::verify", "sm::seal_gcm", "sm::open_ccm"] {
            let args = match alias {
                "sm::hash" => "b\"x\"",
                "sm::verify" => "b\"m\", b\"s\", b\"k\"",
                _ => "b\"k\", b\"n\", b\"a\", b\"m\"",
            };
            let program = parse(&format!("fn f() {{ let _x = {alias}({args}); }}"))
                .expect("noncanonical alias parses before semantic resolution");
            let error = analyze(&program).expect_err("alias must not bypass canonical resolution");
            assert_eq!(
                error.message,
                format!("unknown function or builtin `{alias}`")
            );
        }
    }

    #[test]
    fn truncated_scalar_crypto_and_ephemeral_nullifier_calls_are_rejected_from_source() {
        for (name, args) in [
            ("crypto::poseidon2", "left: 1, right: 2"),
            ("crypto::poseidon6", "a: 1, b: 2, c: 3, d: 4, e: 5, f: 6"),
            ("crypto::pubkgen", "1"),
            ("crypto::use_nullifier", "1"),
        ] {
            let program = parse(&format!("fn f() {{ let _value = {name}({args}); }}"))
                .expect("retired scalar crypto spelling parses before resolution");
            let error = analyze(&program).expect_err("retired source capability must fail closed");
            assert_eq!(error.code, "K2002", "{name}: {error:?}");
            assert_eq!(
                error.message,
                format!("unknown function or builtin `{name}`")
            );
        }
    }

    #[test]
    fn branded_feature_diagnostics_never_leak_compiler_internal_english_names() {
        for (source, branded, internal) in [
            (
                "fn f() { ledger::query::seiyaku_manifest(true); }",
                "ledger::query::seiyaku_manifest",
                "query_get_contract_manifest",
            ),
            (
                "fn f() { ledger::query::seiyaku_instance(true); }",
                "ledger::query::seiyaku_instance",
                "query_get_contract_instance",
            ),
            (
                "fn f() { ledger::seiyaku::grant_kotoage(1); }",
                "ledger::seiyaku::grant_kotoage",
                "grant_contract_entrypoint",
            ),
            (
                "fn f() { ledger::seiyaku::revoke_kotoage(1); }",
                "ledger::seiyaku::revoke_kotoage",
                "revoke_contract_entrypoint",
            ),
            (
                "fn f() { context::seiyaku_subject(1); }",
                "context::seiyaku_subject",
                "contract_subject",
            ),
            (
                "fn f() { context::seiyaku_address(1); }",
                "context::seiyaku_address",
                "contract_address",
            ),
            (
                "fn f() { context::kotoage(1); }",
                "context::kotoage",
                "entrypoint",
            ),
        ] {
            let program = parse(source).expect("parse branded diagnostic fixture");
            let error = analyze(&program).expect_err("wrong branded call must fail");
            assert!(
                error.message.contains(branded),
                "missing branded spelling `{branded}`: {error:?}"
            );
            assert!(
                !error.message.contains(internal),
                "diagnostic leaked internal spelling `{internal}`: {error:?}"
            );
        }

        for (source, branded, internal) in [
            (
                "seiyaku T { #[test] fn f() { test::invoke_kotoage(1); } }",
                "test::invoke_kotoage",
                "invoke_entrypoint",
            ),
            (
                "seiyaku T { #[test] fn f() { test::invoke_kotoage_as(1); } }",
                "test::invoke_kotoage_as",
                "invoke_entrypoint_as",
            ),
        ] {
            let program = parse(source).expect("parse branded test-helper fixture");
            let error = analyze_test(&program).expect_err("wrong branded test call must fail");
            assert!(error.message.contains(branded), "{error:?}");
            assert!(!error.message.contains(internal), "{error:?}");
        }
    }

    #[test]
    fn language_feature_diagnostics_use_only_branded_terms() {
        fn assert_branded(message: &str) {
            let forbidden = ["contract", "entrypoint", "initialization", "upgrade"];
            for word in message
                .split(|character: char| !character.is_alphanumeric() && character != '_')
                .filter(|word| !word.is_empty())
            {
                assert!(
                    !forbidden.contains(&word.to_ascii_lowercase().as_str()),
                    "diagnostic leaked English language-feature alias `{word}`: {message}"
                );
            }
        }

        for source in [
            "module M { kotoage fn run() authorize(\"Run\") {} }",
            "module M { view fn read() {} }",
            "seiyaku S { fn helper() authorize(\"Run\") {} }",
        ] {
            let message = crate::parser::parse(source)
                .expect_err("invalid declaration must produce a parser diagnostic");
            assert_branded(&message);
        }

        for source in [
            "seiyaku S { trigger wake -> missing { on time pre_commit; } }",
            "seiyaku S { view fn read() {} trigger wake -> read { on time pre_commit; } }",
            "seiyaku S { fn helper() {} trigger wake -> helper { on time pre_commit; } }",
            "seiyaku S { state StateMap<int, int> values; view fn read() -> int { return values.ensure(1, 2); } }",
            "seiyaku S { kotoage fn admin() authorize(\"Admin\") {} kotoage fn run() authorize(\"Run\") { admin(); } }",
            "seiyaku S { state int first; state int second; hajimari() { first = 0; } }",
        ] {
            let program = parse(source).expect("semantic diagnostic fixture must parse");
            let error =
                analyze(&program).expect_err("invalid program must produce a semantic diagnostic");
            assert_branded(&error.message);
        }
    }

    #[test]
    fn public_valcom_operands_are_rejected_in_favour_of_typed_secrets() {
        let program = parse("fn f() -> int { return crypto::valcom(left: 7, right: 11); }")
            .expect("parse public valcom call");
        let error = SemanticContext::with_zk_enabled(true)
            .analyze(&program)
            .expect_err("public scalar commitment must fail closed");
        assert_eq!(error.code, "K2003");
        assert_eq!(
            error.message,
            "crypto::valcom expects two typed Secret<int|decimal|quantity> arguments"
        );
    }

    #[test]
    fn unshield_public_amount_is_a_quantity_with_a_narrow_v1_literal_domain() {
        let source = |declaration: &str, amount: &str| {
            format!(
                r#"
seiyaku UnshieldAmount {{
  {declaration}
  fn build() {{
    let _bytes = crypto::zk::build_unshield(
      asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      destination: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"),
      amount: {amount},
      inputs: b"0123456789abcdef0123456789abcdef",
      backend: "halo2/ipa",
      proof: b"proof",
      verification_key: b"vk",
    );
  }}
}}
"#
            )
        };
        let analyze_zk = |source: &str| {
            let program = parse(source).expect("parse unshield quantity fixture");
            SemanticContext::with_zk_enabled(true).analyze(&program)
        };

        analyze_zk(&source("", "9223372036854775808"))
            .expect("a contextual whole quantity inside u128 must type check");
        analyze_zk(&source("const quantity AMOUNT = 7;", "AMOUNT"))
            .expect("an explicit quantity constant must type check");

        for (amount, code, message) in [
            (
                "1.5",
                "E_UNSHIELD_AMOUNT_RANGE",
                "requires a whole quantity with scale 0",
            ),
            (
                "340282366920938463463374607431768211456",
                "E_UNSHIELD_AMOUNT_RANGE",
                "quantity exceeds the u128 V1 proof-scalar range",
            ),
            (
                "-1",
                "E_NEGATIVE_QUANTITY",
                "contextual quantity literal cannot be negative",
            ),
        ] {
            let error = analyze_zk(&source("", amount))
                .expect_err("invalid unshield quantity must fail semantic validation");
            assert_eq!(error.code(), code, "amount={amount}: {}", error.message);
            assert!(
                error.message.contains(message),
                "amount={amount}: expected `{message}` in `{}`",
                error.message
            );
        }
    }

    #[test]
    fn valcom_registry_rejects_non_zk_analysis_with_the_source_name() {
        let result = analyze_surface_builtin_call(
            &SemanticContext::new(),
            Builtin::Valcom,
            Vec::new(),
            Some(&Type::Int),
        );
        let error = canonicalize_builtin_result(Builtin::Valcom, result)
            .expect_err("the Secret-only commitment requires ZK mode");
        assert_eq!(error.code, "E_ZK_MODE_REQUIRED");
        assert_eq!(
            error.message,
            "builtin `crypto::valcom` requires ZK mode in compiler build configuration"
        );
    }

    #[test]
    fn public_entrypoints_reject_zk_verify_without_permission() {
        let mut program = parse(
            "seiyaku Demo { kotoage fn verify(bytes payload) authorize(\"Verify\") { crypto::zk::verify_unshield(payload); } }",
        )
        .expect("parse public zk verify");
        let function = program
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "verify" => Some(function),
                _ => None,
            })
            .expect("verify function");
        function.modifiers.permission = None;
        let err = SemanticContext::with_zk_enabled(true)
            .analyze(&program)
            .expect_err("a fabricated public zk verifier AST should require permission");
        assert!(
            err.message
                .contains("kotoage function `verify` requires `authorize(\"Permission\")`"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn runtime_entrypoints_cannot_be_direct_call_targets() {
        for (target_declaration, target_name) in [
            ("kotoage fn admin() authorize(\"Admin\") {}", "admin"),
            ("view fn inspect() {}", "inspect"),
        ] {
            let source = format!(
                "seiyaku Demo {{ {target_declaration} kotoage fn run() authorize(\"Run\") {{ {target_name}(); }} }}"
            );
            let error = analyze_error(&source);
            assert!(
                error.message.contains(&format!(
                    "seiyaku runtime function `{target_name}` cannot be called directly"
                )),
                "unexpected direct-entrypoint diagnostic: {error:?}"
            );
        }
    }

    #[test]
    fn privileged_effect_table_covers_release_mutators() {
        for name in [
            "transfer_asset",
            "create_new_asset",
            "create_nfts_for_all_users",
            "transfer_batch",
            "axt_begin",
            "axt_touch",
            "use_asset_handle",
            "axt_commit",
        ] {
            let builtin = Builtin::from_name(name).expect("registered builtin");
            assert!(
                builtin.spec().effects.host_side_effects,
                "privileged builtin `{name}` must be classified as effectful"
            );
        }
    }

    #[test]
    fn canonical_context_and_ledger_namespaces_type_check() {
        let program = parse(
            r#"
            seiyaku Payments {
                kotoage fn transfer(
                    AccountId recipient,
                    AssetDefinitionId asset,
                    quantity amount,
                    DataSpaceId dataspace
                ) authorize("TransferAsset") {
                    let sender = context::authority();
                    ledger::asset::transfer(
                        source: sender,
                        destination: recipient,
                        asset_definition: asset,
                        amount: amount,
                        dataspace: dataspace,
                    );
                }
            }
            "#,
        )
        .expect("parse canonical namespaced calls");
        analyze(&program).expect("canonical context and ledger namespaces should type-check");
    }

    #[test]
    fn escrow_open_offer_signature_matches_the_host_abi() {
        let program = parse(
            r#"
            seiyaku Escrow {
                fn open(
                    Name escrow,
                    AssetDefinitionId asset,
                    quantity amount,
                    bytes evidence
                ) {
                    ledger::escrow::open_offer(
                        offer: escrow,
                        asset_definition: asset,
                        amount: amount,
                    );
                    ledger::escrow::open_offer(
                        offer: escrow,
                        asset_definition: asset,
                        amount: amount,
                        evidence: evidence,
                    );
                }
            }
            "#,
        )
        .expect("parse canonical escrow calls");
        analyze(&program).expect("three required arguments plus optional evidence must type-check");

        let invalid = parse(
            r#"
            seiyaku Escrow {
                fn open(
                    Name escrow,
                    AccountId account,
                    AssetDefinitionId asset,
                    quantity amount
                ) {
                    ledger::escrow::open_offer(
                        offer: escrow,
                        asset_definition: account,
                        amount: account,
                        evidence: asset,
                        unexpected: amount,
                    );
                }
            }
            "#,
        )
        .expect("parse invalid escrow call");
        let error = analyze(&invalid).expect_err("the retired five-argument shape must fail");
        assert_eq!(error.code, "E_UNKNOWN_NAMED_ARGUMENT");
        assert!(
            error.message.contains("unexpected"),
            "unexpected diagnostic: {}",
            error.message
        );
    }

    #[test]
    fn public_entrypoints_reject_state_mutation_without_permission() {
        let mut program = parse(
            "seiyaku Demo { state int counter; hajimari() { counter = 0; } kotoage fn set() authorize(\"Set\") { counter = 1; } }",
        )
        .expect("parse public state mutation");
        let function = program
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "set" => Some(function),
                _ => None,
            })
            .expect("set function");
        function.modifiers.permission = None;
        let err = analyze(&program)
            .expect_err("a fabricated public state-mutation AST should require permission");
        assert!(
            err.message
                .contains("kotoage function `set` requires `authorize(\"Permission\")`"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn recursive_functions_are_rejected() {
        for source in [
            "fn recurse() { recurse(); }",
            "fn first() { second(); } fn second() { first(); }",
        ] {
            let program = parse(source).expect("parse recursive functions");
            let err = analyze(&program).expect_err("function recursion must be rejected");
            assert!(
                err.message
                    .contains("recursive function calls are not supported in Kotodama V1"),
                "unexpected error: {}",
                err.message
            );
        }
    }

    #[test]
    fn view_entrypoints_reject_transitive_zk_verify() {
        let program = parse(
            "seiyaku Demo { fn helper(bytes payload) { crypto::zk::verify_transfer(payload); } view fn f(bytes payload) -> int { helper(payload); return 1; } }",
        )
        .expect("parse transitive zk verify");
        let err = SemanticContext::with_zk_enabled(true)
            .analyze(&program)
            .expect_err("view zk verify should fail");
        assert!(
            err.message.contains(
                "view function `f` cannot call `helper` because `helper` performs host side effects"
            ),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn resolve_account_alias_accepts_canonical_string() {
        let program = parse(
            "fn f() { let _acct = ledger::account::resolve_alias(\"banking@centralbank\"); }",
        )
        .expect("parse resolve_account_alias");
        analyze(&program).expect("resolve_account_alias should type-check");
    }

    #[test]
    fn resolve_account_alias_accepts_alias_bytes() {
        let program = parse(
            r#"fn f() { let alias = b"banking@centralbank"; let _acct = ledger::account::resolve_alias(alias); }"#,
        )
        .expect("parse resolve_account_alias blob");
        analyze(&program).expect("resolve_account_alias blob should type-check");
    }

    #[test]
    fn durable_state_maps_accept_forward_declared_struct_values() {
        let program = parse(
            r#"seiyaku Demo {
                struct Request {
                    int status,
                    bytes alias_blob,
                    bytes requested_by_actor_id,
                    Json requested_by_actor
                }
                state StateMap<Name, Request> Requests;
                kotoage fn create_request(Name proposal_id,
                                          bytes alias_literal,
                                          bytes requested_by_actor_id,
                                          Json requested_by_actor) authorize("CreateRequest") {
                    Requests[proposal_id] = Request {
                        status: 1,
                        alias_blob: alias_literal,
                        requested_by_actor_id,
                        requested_by_actor,
                    };
                }
            }"#,
        )
        .expect("parse durable struct map");
        analyze(&program).expect("durable struct-valued state map should type-check");
    }

    #[test]
    fn equality_between_event_account_and_resolved_alias_type_checks() {
        let program = parse(
            "fn f() { \
                let ev = context::trigger_event(); \
                if let Option::some(dst) = ev.get_account_id(Name::parse(\"account_id\")) { \
                    let sink = ledger::account::resolve_alias(\"banking@centralbank\"); \
                    let _same = dst == sink; \
                } \
            }",
        )
        .expect("parse account equality");
        analyze(&program).expect("account-id equality should type-check");
    }

    #[test]
    fn get_asset_definition_id_accepts_trigger_payloads() {
        let program = parse(
            "fn f() { let ev = context::trigger_event(); let _asset = ev.get_asset_definition_id(Name::parse(\"asset_definition_id\")); }",
        )
        .expect("parse get_asset_definition_id");
        analyze(&program).expect("get_asset_definition_id should type-check");
    }

    #[test]
    fn get_quantity_returns_an_optional_trigger_quantity() {
        let program = parse(
            "fn f() { let ev = context::trigger_event(); let Option<quantity> value = ev.get_quantity(Name::parse(\"amount\")); }",
        )
        .expect("parse get_quantity");
        analyze(&program).expect("get_quantity should type-check as Option<quantity>");
    }

    #[test]
    fn durable_string_state_is_supported() {
        let program = parse(
            r#"seiyaku C {
                state string label;
                hajimari() { label = "ready"; }
            }"#,
        )
        .expect("parse string state");
        analyze(&program).expect("string state should be supported");
    }

    #[test]
    fn durable_struct_string_field_is_supported() {
        let program = parse(
            r#"seiyaku C {
                struct S { string label }
                state S s;
                hajimari() { s = S { label: "ready" }; }
            }"#,
        )
        .expect("parse state struct");
        analyze(&program).expect("string state field should be supported");
    }

    #[test]
    fn nested_state_map_is_rejected() {
        let ty = Type::Struct {
            name: "S".into(),
            fields: Arc::from(vec![(
                "children".into(),
                Type::StateMap(Box::new(Type::Int), Box::new(Type::Int)),
            )]),
        };
        let err = validate_state_type(&ty).expect_err("nested StateMap must be rejected");
        assert!(
            err.message.contains("nested StateMap is not supported"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn durable_option_and_result_accept_aggregate_payloads() {
        let program = parse(
            r#"seiyaku C {
                struct Pair { int count, bool ready }
                state Option<Pair> maybe;
                state Result<Pair, Pair> outcome;
                hajimari() {
                    maybe = Option::none;
                    outcome = Result::ok(Pair { count: 1, ready: true });
                }
            }"#,
        )
        .expect("parse aggregate sum state");
        analyze(&program).expect("aggregate Option/Result state should type-check");
    }

    #[test]
    fn local_sum_annotations_resolve_aggregate_payloads_contextually() {
        let program = parse(
            r#"seiyaku C {
                struct Pair { int count, bool ready }
                fn values() {
                    let Option<Pair> some = Option::some(Pair { count: 1, ready: true });
                    let Option<Pair> none = Option::none;
                    let Result<Pair, Pair> ok = Result::ok(Pair { count: 2, ready: true });
                    let Result<Pair, Pair> err = Result::err(Pair { count: 3, ready: false });
                    var Option<Pair> changing_option = Option::some(Pair { count: 4, ready: true });
                    changing_option = Option::none;
                    var Result<Pair, Pair> changing_result = Result::ok(Pair { count: 5, ready: true });
                    changing_result = Result::err(Pair { count: 6, ready: false });
                }
            }"#,
        )
        .expect("parse aggregate local sums");
        analyze(&program).expect("aggregate local sum annotations should resolve nominal payloads");
    }

    #[test]
    fn explicit_numeric_conversions_preserve_nominal_types() {
        let program = parse(
            "seiyaku C { fn f(int value) -> decimal { \
                return decimal::from_int(value); \
            } }",
        )
        .expect("parse explicit conversions");
        let typed = analyze(&program).expect("analyze explicit conversions");
        let TypedItem::Function(f) = &typed.items[0];
        assert_eq!(f.ret_ty, Some(Type::Decimal));
    }

    #[test]
    fn quantity_remains_nominal_in_mixed_numeric_operations() {
        let program = parse(
            "seiyaku C { fn f(quantity a, int b) { \
                let _x = a + b; \
            } }",
        )
        .expect("parse nominal numeric types");
        let err = analyze(&program).expect_err("mixed numeric types should error");
        assert!(err.message.contains("not defined for quantity and int"));
    }

    #[test]
    fn exact_literals_infer_decimal_without_runtime_conversion() {
        let constant = returned_expr("fn value() -> decimal { return 2 + 0.5; }");
        assert_eq!(constant.ty, Type::Decimal);
        assert!(matches!(
            constant.kind(),
            ExprKind::DecimalLiteral { value, .. } if value.to_string() == "2.5"
        ));

        let arithmetic =
            returned_expr("fn value(decimal fraction) -> decimal { return 2 + fraction; }");
        assert!(matches!(
            arithmetic.kind(),
            ExprKind::Binary { left, right, .. }
                if matches!(left.kind(), ExprKind::DecimalLiteral { .. })
                    && matches!(right.kind(), ExprKind::Ident(_))
        ));
        analyze(
            &parse("fn value(decimal fraction) { let inferred = 2 + fraction; }")
                .expect("parse sibling-inferred decimal literal"),
        )
        .expect("a decimal sibling must infer an exact literal without a return context");

        let comparison =
            returned_expr("fn less(decimal fraction) -> bool { return 2 < fraction; }");
        assert!(matches!(
            comparison.kind(),
            ExprKind::Binary { left, right, .. }
                if matches!(left.kind(), ExprKind::DecimalLiteral { .. })
                    && matches!(right.kind(), ExprKind::Ident(_))
        ));
    }

    #[test]
    fn mixed_runtime_int_decimal_operations_require_explicit_conversion() {
        for source in [
            "fn value(int whole, decimal fraction) -> decimal { return whole + fraction; }",
            "fn less(int whole, decimal fraction) -> bool { return whole < fraction; }",
            "fn equal(int whole, decimal fraction) -> bool { return whole == fraction; }",
            "fn literal(int whole) -> decimal { return whole + 0.5; }",
        ] {
            let error = analyze_error(source);
            assert_eq!(error.code, "E_IMPLICIT_NUMERIC_CONVERSION");
            assert_eq!(
                error.message,
                "`int` and `decimal` operands cannot be mixed implicitly; convert the `int` with `decimal::from_int(value)` before arithmetic or comparison"
            );
        }

        analyze(
            &parse(
                "fn value(int whole, decimal fraction) -> decimal { \
                    return decimal::from_int(whole) + fraction; \
                }",
            )
            .expect("parse explicit conversion"),
        )
        .expect("an explicit int-to-decimal conversion must remain valid");
    }

    #[test]
    fn decimal_compound_assignment_requires_explicit_runtime_conversion() {
        let implicit = analyze_error(
            "fn accumulate(int delta) -> decimal { \
                var decimal value = 1.5; \
                value += delta; \
                return value; \
            }",
        );
        assert_eq!(implicit.code, "E_IMPLICIT_NUMERIC_CONVERSION");
        assert!(implicit.message.contains("decimal::from_int(value)"));

        let source = parse(
            "fn accumulate(int delta) -> decimal { \
                var decimal value = 1.5; \
                value += decimal::from_int(delta); \
                value += 2; \
                return value; \
            }",
        )
        .expect("parse explicit compound assignment conversion");
        let typed = analyze(&source).expect("explicit and contextual literal conversion must pass");
        let TypedItem::Function(function) = &typed.items[0];
        let assignments = function
            .body
            .statements
            .iter()
            .filter_map(|statement| match statement {
                TypedStatement::Let { name, value } if name == "value" => {
                    matches!(value.kind(), ExprKind::Binary { .. }).then_some(value)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(assignments.len(), 2);
        assert!(matches!(
            assignments[0].kind(),
            ExprKind::Binary { right, .. }
                if matches!(right.kind(), ExprKind::NumericCast { .. })
        ));
        assert!(matches!(
            assignments[1].kind(),
            ExprKind::Binary { right, .. }
                if matches!(right.kind(), ExprKind::DecimalLiteral { .. })
        ));
    }

    #[test]
    fn quantity_rejects_remainder_and_negation_surfaces() {
        let remainder = parse("seiyaku C { fn f(quantity a, quantity b) { let _x = a % b; } }")
            .expect("parse quantity remainder");
        let error = analyze(&remainder).expect_err("quantity remainder must fail");
        assert_eq!(error.code, "E_QUANTITY_REMAINDER");

        let negation = parse("seiyaku C { fn f(quantity a) { let _x = -a; } }")
            .expect("parse numeric negation");
        let error = analyze(&negation).expect_err("quantity negation must fail");
        assert_eq!(error.code, "E_QUANTITY_NEGATION");
    }

    #[test]
    fn unsuffixed_whole_literal_is_contextual_in_a_quantity_position() {
        let program = parse("seiyaku C { fn f() -> quantity { return 1; } }")
            .expect("parse unsuffixed literal");
        analyze(&program).expect("whole literal must coerce exactly in a quantity context");
    }

    #[test]
    fn values_wider_than_u128_are_accepted_as_int() {
        let program = parse(
            "seiyaku C { fn wide_value() -> int { \
                return 340282366920938463463374607431768211456; \
            } }",
        )
        .expect("parse adaptive-width int");
        let typed = analyze(&program).expect("analyze adaptive-width int");
        let TypedItem::Function(function) = &typed.items[0];
        assert_eq!(function.ret_ty, Some(Type::Int));
    }

    #[test]
    fn adaptive_int_values_use_width_independent_operators() {
        let program =
            parse("seiyaku C { fn f(int value) -> int { return value < 0 ? -value : value; } }")
                .expect("parse width-independent int expression");
        analyze(&program).expect("ordinary int operators must accept the complete V1 domain");
    }

    #[test]
    fn ledger_quantity_parameters_contextually_accept_whole_literals() {
        let program = parse(
            "seiyaku C { fn f(AccountId account, AssetDefinitionId asset) { \
                ledger::asset::mint(account: account, asset_definition: asset, amount: 1); \
            } }",
        )
        .expect("parse ledger amount call");
        analyze(&program).expect("whole literal must coerce exactly at a quantity boundary");
    }

    #[test]
    fn canonical_trigger_operations_type_check() {
        let program = parse(
            "fn f() { \
                ledger::trigger::register(Json::parse(\"{}\")); \
                ledger::trigger::unregister(Name::parse(\"wake\")); \
            }",
        )
        .expect("parse trigger aliases");
        analyze(&program).expect("analyze trigger aliases");
    }

    #[test]
    fn trigger_decl_builds_typed_metadata() {
        use iroha_data_model::account::AccountId;

        let authority_literal = sample_account_literal();
        let program = parse(&format!(
            r#"
            seiyaku C {{
                kotoage fn run() authorize("RunTrigger") {{}}
                trigger wake -> run {{
                    on time pre_commit;
                    repeats 2;
                    authority "{authority_literal}";
                    metadata {{ tag: "alpha"; count: 1; enabled: true; }}
                }}
            }}
            "#,
        ))
        .expect("parse trigger decl");
        let typed = analyze(&program).expect("analyze trigger decl");
        assert_eq!(typed.triggers.len(), 1);
        let trigger = &typed.triggers[0];
        assert_eq!(trigger.id.to_string(), "wake");
        assert!(matches!(trigger.filter, EventFilterBox::Time(_)));
        assert_eq!(trigger.repeats, Repeats::Exactly(2));
        assert_eq!(
            trigger.authority,
            Some(
                AccountId::parse_encoded(authority_literal.as_str())
                    .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    .expect("authority literal"),
            )
        );
        assert!(!trigger.metadata.is_empty());
    }

    #[test]
    fn trigger_metadata_json_parse_uses_json_literal_diagnostics() {
        let duplicate = analyze_error(
            r#"
            seiyaku C {
                kotoage fn run() authorize("RunTrigger") {}
                trigger wake -> run {
                    on time pre_commit;
                    metadata {
                        payload: Json::parse("{\"owner\":1,\"owner\":2}");
                    }
                }
            }
            "#,
        );
        assert_eq!(duplicate.code, "E_JSON_DUPLICATE_KEY");

        let malformed = analyze_error(
            r#"
            seiyaku C {
                kotoage fn run() authorize("RunTrigger") {}
                trigger wake -> run {
                    on time pre_commit;
                    metadata {
                        payload: Json::parse("{\"owner\":");
                    }
                }
            }
            "#,
        );
        assert_eq!(malformed.code, "E_JSON_LITERAL_INVALID");
    }

    #[test]
    fn trigger_metadata_json_parse_obeys_the_canonical_call_contract() {
        let trigger_source = |value: &str| {
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on time pre_commit;
                        metadata {{ payload: {value}; }}
                    }}
                }}
                "#,
            )
        };

        for value in [r#"Json::parse("{}")"#, r#"Json::parse(value: "{}")"#] {
            let source = trigger_source(value);
            let program = parse(&source).expect("canonical Json::parse metadata should parse");
            analyze(&program).unwrap_or_else(|error| {
                panic!("canonical trigger metadata `{value}` failed: {error:?}")
            });
        }

        for (value, code, message) in [
            (
                r#"Json::parse(raw: "{}")"#,
                "E_UNKNOWN_NAMED_ARGUMENT",
                "call `Json::parse` has no parameter named `raw`",
            ),
            ("Json::parse()", "K2003", "Json::parse expects one argument"),
            (
                r#"Json::parse("{}", "{}")"#,
                "K2003",
                "Json::parse expects one argument",
            ),
            (
                "Json::parse(value: dynamic)",
                "E_TRIGGER_METADATA_VALUE",
                "Json::parse(...) metadata values must be a string literal",
            ),
            (
                r#"json("{}")"#,
                "E_NON_CANONICAL_BUILTIN",
                "legacy or non-canonical builtin spelling `json` is not supported; use `Json::parse`",
            ),
        ] {
            let error = analyze_error(&trigger_source(value));
            assert_eq!(error.code, code, "{value}: {error:?}");
            assert_eq!(error.message, message, "{value}: {error:?}");
        }
    }

    #[test]
    fn trigger_decl_supports_data_filter() {
        let program = parse(
            r#"
            seiyaku C {
                kotoage fn run() authorize("RunTrigger") {}
                trigger wake -> run {
                    on data any;
                }
            }
            "#,
        )
        .expect("parse trigger decl");
        let typed = analyze(&program).expect("analyze trigger decl");
        let trigger = &typed.triggers[0];
        assert!(matches!(
            trigger.filter,
            EventFilterBox::Data(DataEventFilter::Any)
        ));
    }

    #[test]
    fn trigger_decl_supports_structured_asset_data_filter() {
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        );
        let asset_definition_literal = asset_definition.to_string();
        let program = parse(&format!(
            r#"
            seiyaku C {{
                kotoage fn run() authorize("RunTrigger") {{}}
                trigger wake -> run {{
                    on data asset added {{
                        asset_definition "{asset_definition_literal}";
                    }}
                }}
            }}
            "#,
        ))
        .expect("parse trigger decl");
        let typed = analyze(&program).expect("analyze trigger decl");
        let trigger = &typed.triggers[0];
        assert_eq!(
            trigger.filter,
            EventFilterBox::Data(DataEventFilter::Asset(
                AssetEventFilter::new()
                    .for_events(AssetEventSet::Added)
                    .for_asset_definition(asset_definition),
            ))
        );
    }

    #[test]
    fn trigger_decl_supports_transfer_specific_asset_filter() {
        use iroha_data_model::account::ParsedAccountId;

        let source_literal = sample_account_literal();
        let source = AccountId::parse_encoded(source_literal.as_str())
            .map(ParsedAccountId::into_account_id)
            .expect("source account");
        let destination_literal = {
            let key_pair = iroha_crypto::KeyPair::try_random().expect("destination key");
            AccountId::new(key_pair.public_key().clone()).to_string()
        };
        let destination = AccountId::parse_encoded(destination_literal.as_str())
            .map(ParsedAccountId::into_account_id)
            .expect("destination account");
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        );
        let program = parse(&format!(
            r#"
            seiyaku C {{
                kotoage fn run() authorize("RunTrigger") {{}}
                trigger wake -> run {{
                    on data asset transferred {{
                        asset_definition "{asset_definition}";
                        source_account "{source_literal}";
                        destination_account "{destination_literal}";
                    }}
                }}
            }}
            "#
        ))
        .expect("parse transfer trigger");
        let typed = analyze(&program).expect("analyze transfer trigger");
        assert_eq!(
            typed.triggers[0].filter,
            EventFilterBox::Data(DataEventFilter::Asset(
                AssetEventFilter::new()
                    .for_events(AssetEventSet::Transferred)
                    .for_asset_definition(asset_definition)
                    .for_transfer_source_account(source)
                    .for_transfer_destination_account(destination),
            ))
        );
    }

    #[test]
    fn trigger_decl_supports_structured_data_filters_for_core_families() {
        use iroha_data_model::{
            account::{AccountId, ParsedAccountId},
            events::{
                EventFilterBox,
                data::{
                    DataEventFilter,
                    prelude::{
                        AccountEventFilter, AccountEventSet, AssetDefinitionEventFilter,
                        AssetDefinitionEventSet, AssetEventFilter, AssetEventSet,
                        ConfigurationEventFilter, ConfigurationEventSet, DomainEventFilter,
                        DomainEventSet, ExecutorEventFilter, ExecutorEventSet, NftEventFilter,
                        NftEventSet, PeerEventFilter, PeerEventSet, RoleEventFilter, RoleEventSet,
                        TriggerEventFilter, TriggerEventSet,
                    },
                },
            },
            nft::NftId,
            peer::PeerId,
            role::RoleId,
            rwa::RwaId,
            trigger::TriggerId,
        };

        let account_literal = sample_account_literal();
        let account = AccountId::parse_encoded(account_literal.as_str())
            .map(ParsedAccountId::into_account_id)
            .expect("account");
        let peer_literal = "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D";
        let peer: PeerId = peer_literal.parse().expect("peer");
        let domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        );
        let asset = AssetId::new(asset_definition.clone(), account.clone());
        let asset_literal = asset.canonical_literal();
        let nft: NftId = "n0$wonderland.universal".parse().expect("nft");
        let rwa: RwaId = format!(
            "{}$wonderland.universal",
            iroha_crypto::Hash::prehashed([7; iroha_crypto::Hash::LENGTH])
        )
        .parse()
        .expect("rwa");
        let trigger_id: TriggerId = "wake".parse().expect("trigger");
        let role_id: RoleId = "auditor".parse().expect("role");

        let cases = vec![
            (
                format!(
                    r#"
                    seiyaku C {{
                kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data peer added {{
                                peer "{peer_literal}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Peer(
                    PeerEventFilter::new()
                        .for_events(PeerEventSet::Added)
                        .for_peer(peer),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data domain created {{
                                domain "{domain}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Domain(
                    DomainEventFilter::new()
                        .for_events(DomainEventSet::Created)
                        .for_domain(domain.clone()),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data account created {{
                                account "{account_literal}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Account(
                    AccountEventFilter::new()
                        .for_events(AccountEventSet::Created)
                        .for_account(account.clone()),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data asset added {{
                                asset "{asset_literal}";
                                asset_definition "{asset_definition}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Asset(
                    AssetEventFilter::new()
                        .for_events(AssetEventSet::Added)
                        .for_asset(asset.clone())
                        .for_asset_definition(asset_definition.clone()),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data asset_definition created {{
                                asset_definition "{asset_definition}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::AssetDefinition(
                    AssetDefinitionEventFilter::new()
                        .for_events(AssetDefinitionEventSet::Created)
                        .for_asset_definition(asset_definition.clone()),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data nft created {{
                                nft "{nft}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Nft(
                    NftEventFilter::new()
                        .for_events(NftEventSet::Created)
                        .for_nft(nft),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data rwa created {{
                                rwa "{rwa}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Rwa(
                    RwaEventFilter::new()
                        .for_events(RwaEventSet::Created)
                        .for_rwa(rwa),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data trigger created {{
                                trigger "{trigger_id}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Trigger(
                    TriggerEventFilter::new()
                        .for_events(TriggerEventSet::Created)
                        .for_trigger(trigger_id),
                )),
            ),
            (
                format!(
                    r#"
                    seiyaku C {{
                        kotoage fn run() authorize("RunTrigger") {{}}
                        trigger wake -> run {{
                            on data role created {{
                                role "{role_id}";
                            }}
                        }}
                    }}
                    "#
                ),
                EventFilterBox::Data(DataEventFilter::Role(
                    RoleEventFilter::new()
                        .for_events(RoleEventSet::Created)
                        .for_role(role_id),
                )),
            ),
            (
                r#"
                seiyaku C {
                    kotoage fn run() authorize("RunTrigger") {}
                    trigger wake -> run {
                        on data configuration changed {}
                    }
                }
                "#
                .to_string(),
                EventFilterBox::Data(DataEventFilter::Configuration(
                    ConfigurationEventFilter::new().for_events(ConfigurationEventSet::Changed),
                )),
            ),
            (
                r#"
                seiyaku C {
                    kotoage fn run() authorize("RunTrigger") {}
                    trigger wake -> run {
                        on data executor upgraded {}
                    }
                }
                "#
                .to_string(),
                EventFilterBox::Data(DataEventFilter::Executor(
                    ExecutorEventFilter::new().for_events(ExecutorEventSet::Upgraded),
                )),
            ),
        ];

        for (src, expected_filter) in cases {
            let program = parse(&src).expect("parse trigger decl");
            let typed = analyze(&program).expect("analyze trigger decl");
            let trigger = &typed.triggers[0];
            assert_eq!(trigger.filter, expected_filter);
        }
    }

    #[test]
    fn trigger_decl_supports_pipeline_filter() {
        use iroha_data_model::events::pipeline::{BlockEventFilter, BlockStatus};

        let program = parse(
            r#"
            seiyaku C {
                kotoage fn run() authorize("RunTrigger") {}
                trigger wake -> run {
                    on pipeline block approved;
                }
            }
            "#,
        )
        .expect("parse trigger decl");
        let typed = analyze(&program).expect("analyze trigger decl");
        let trigger = &typed.triggers[0];
        assert_eq!(
            trigger.filter,
            EventFilterBox::Pipeline(PipelineEventFilterBox::Block(
                BlockEventFilter::new().for_status(BlockStatus::Approved),
            ))
        );
    }

    #[test]
    fn trigger_decl_supports_pipeline_transaction_approved_filter() {
        use iroha_data_model::events::pipeline::{TransactionEventFilter, TransactionStatus};

        let program = parse(
            r#"
            seiyaku C {
                kotoage fn run() authorize("RunTrigger") {}
                trigger wake -> run {
                    on pipeline transaction approved;
                }
            }
            "#,
        )
        .expect("parse trigger decl");
        let typed = analyze(&program).expect("analyze trigger decl");
        let trigger = &typed.triggers[0];
        assert_eq!(
            trigger.filter,
            EventFilterBox::Pipeline(PipelineEventFilterBox::Transaction(
                TransactionEventFilter::new().for_status(TransactionStatus::Approved),
            ))
        );
    }

    #[test]
    fn trigger_decl_rejects_invalid_data_matcher_literal() {
        let program = parse(
            r#"
            seiyaku C {
                kotoage fn run() authorize("RunTrigger") {}
                trigger wake -> run {
                    on data asset added {
                        asset_definition "not-an-address";
                    }
                }
            }
            "#,
        )
        .expect("parse trigger decl");
        let err = analyze(&program).expect_err("invalid matcher should error");
        assert!(
            err.message
                .contains("invalid `asset_definition` matcher literal")
        );
    }

    #[test]
    fn trigger_decl_rejects_duplicate_data_matchers() {
        let asset_definition_literal = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        )
        .to_string();
        let program = parse(&format!(
            r#"
            seiyaku C {{
                kotoage fn run() authorize("RunTrigger") {{}}
                trigger wake -> run {{
                    on data asset added {{
                        asset_definition "{asset_definition_literal}";
                        asset_definition "{asset_definition_literal}";
                    }}
                }}
            }}
            "#,
        ))
        .expect("parse trigger decl");
        let err = analyze(&program).expect_err("duplicate matcher should error");
        assert!(err.message.contains("duplicate `asset_definition` matcher"));
    }

    #[test]
    fn trigger_decl_rejects_invalid_authority() {
        let program = parse(
            r#"
            seiyaku C {
                kotoage fn run() authorize("RunTrigger") {}
                trigger wake -> run {
                    on time pre_commit;
                    authority "not-an-account";
                }
            }
            "#,
        )
        .expect("parse trigger decl");
        let err = analyze(&program).expect_err("invalid authority should error");
        assert!(err.message.contains("invalid trigger authority"));
    }

    #[test]
    fn trigger_decl_accepts_canonical_domainless_authority() {
        let authority = sample_account_literal();
        let program = parse(&format!(
            r#"
            seiyaku C {{
                kotoage fn run() authorize("RunTrigger") {{}}
                trigger wake -> run {{
                    on time pre_commit;
                    authority "{authority}";
                }}
            }}
            "#,
        ))
        .expect("parse trigger declaration");

        let typed = analyze(&program).expect("canonical domainless authority must type-check");
        assert_eq!(
            typed.triggers[0]
                .authority
                .as_ref()
                .expect("typed trigger authority")
                .to_string(),
            authority,
        );
    }

    #[test]
    fn trigger_decl_requires_kotoage_entrypoint() {
        let program = parse(
            r#"
            seiyaku C {
                fn run() {}
                trigger wake -> run {
                    on time pre_commit;
                }
            }
            "#,
        )
        .expect("parse trigger decl");
        let err = analyze(&program).expect_err("non-kotoage target should error");
        assert!(err.message.contains("`kotoage`/`言挙げ` function"));
    }

    #[test]
    fn trigger_decl_cannot_target_lifecycle_entrypoints_through_constructed_ast() {
        for lifecycle in ["hajimari", "kaizen"] {
            let mut program = parse(
                r#"
                seiyaku C {
                    hajimari() {}
                    kaizen() {}
                    kotoage fn run() authorize("Run") {}
                    trigger wake -> run {
                        on time pre_commit;
                    }
                }
                "#,
            )
            .expect("parse valid trigger declaration");
            let trigger = program
                .items
                .iter_mut()
                .find_map(|item| match item {
                    Item::Trigger(trigger) => Some(trigger),
                    _ => None,
                })
                .expect("trigger declaration");
            trigger.call.entrypoint = lifecycle.to_owned();

            let error = analyze(&program)
                .expect_err("lifecycle entrypoints must never be trigger callbacks");
            assert!(
                error
                    .message
                    .contains("must call a `kotoage`/`言挙げ` function"),
                "unexpected {lifecycle} callback error: {error:?}"
            );
        }
    }

    #[test]
    fn semantic_analysis_defends_against_lifecycle_permission_hints() {
        let mut program = parse("seiyaku Demo { hajimari() {} }").expect("parse hajimari");
        let Item::Function(hajimari) = &mut program.items[0] else {
            panic!("expected hajimari")
        };
        hajimari.modifiers.permission = Some("SourceOwnedPermission".to_owned());

        let error = analyze(&program).expect_err("lifecycle permission must be rejected");
        assert!(
            error
                .message
                .contains("lifecycle authorization is runtime-defined")
        );
    }

    #[test]
    fn typed_core_queries_expose_declared_projection_and_page_types() {
        let program = parse(
            r#"
            seiyaku Queries {
                fn account(AccountId id) -> Option<AccountView> {
                    ledger::query::account(id)
                }

                fn accounts(int offset, int limit) -> QueryPage<AccountView> {
                    ledger::query::accounts(offset: offset, limit: limit)
                }
            }
            "#,
        )
        .expect("parse typed core queries");
        let typed = analyze(&program).expect("typed core query surface should type-check");
        let functions = typed
            .items
            .iter()
            .map(|item| match item {
                TypedItem::Function(function) => function,
            })
            .collect::<Vec<_>>();

        let account = functions
            .iter()
            .find(|function| function.name == "account")
            .expect("singular query helper");
        assert!(matches!(
            account.ret_ty,
            Some(Type::Option(ref view))
                if matches!(view.as_ref(), Type::Struct { name, .. } if name == "AccountView")
        ));

        let accounts = functions
            .iter()
            .find(|function| function.name == "accounts")
            .expect("plural query helper");
        let Some(Type::Struct { name, fields }) = &accounts.ret_ty else {
            panic!("plural query must return QueryPage<AccountView>")
        };
        assert_eq!(name, QUERY_PAGE_TYPE_NAME);
        assert_eq!(
            render_type_name(accounts.ret_ty.as_ref().unwrap()),
            "QueryPage<AccountView>"
        );
        assert!(matches!(
            fields.as_ref(),
            [(items, Type::List(view, 64)), (next, Type::Option(offset))]
                if items == "items"
                    && next == "next_offset"
                    && matches!(view.as_ref(), Type::Struct { name, .. } if name == "AccountView")
                    && offset.as_ref() == &Type::Int
        ));
    }

    #[test]
    fn typed_core_query_pages_require_names_and_bounded_constants() {
        let positional = analyze(
            &parse("fn f() { let _page = ledger::query::accounts(0, 64); }")
                .expect("parse positional page call"),
        )
        .expect_err("pagination calls are named-only");
        assert_eq!(positional.code, "E_NAMED_ARGUMENTS_REQUIRED");

        for (source, code) in [
            (
                "fn f() { let _page = ledger::query::accounts(offset: -1, limit: 64); }",
                "E_QUERY_OFFSET",
            ),
            (
                "fn f() { let _page = ledger::query::accounts(offset: 0, limit: 0); }",
                "E_QUERY_LIMIT",
            ),
            (
                "fn f() { let _page = ledger::query::accounts(offset: 0, limit: 65); }",
                "E_QUERY_LIMIT",
            ),
            (
                "fn f() { let _page = ledger::query::accounts(offset: 18446744073709551616, limit: 64); }",
                "E_QUERY_OFFSET",
            ),
            (
                "fn f() { let _page = ledger::query::accounts(offset: 0, limit: 18446744073709551616); }",
                "E_QUERY_LIMIT",
            ),
        ] {
            let error = analyze(&parse(source).expect("parse invalid page bound"))
                .expect_err("invalid literal page bounds must fail during compilation");
            assert_eq!(error.code, code, "{source}: {}", error.message);
        }
    }

    #[test]
    fn typed_core_singular_queries_reject_raw_bytes() {
        let program = parse("fn account(bytes raw) { let _view = ledger::query::account(raw); }")
            .expect("parse raw-byte core query");
        let error = analyze(&program).expect_err("core queries require their declared typed ID");
        assert_eq!(error.code, "E_QUERY_KEY_TYPE");
    }

    #[test]
    fn tail_sums_matches_if_let_and_propagation_type_check_together() {
        let program = parse(
            r#"
            seiyaku Sums {
                fn option(Option<int> input) -> Option<int> {
                    let value = input?;
                    Option::some(value)
                }

                fn result(Result<int, string> input) -> Result<int, string> {
                    Result::ok(input?)
                }

                fn inspect(Option<int> input) -> int {
                    match input {
                        Option::some(value) => value,
                        Option::none => 0,
                    }
                }

                fn guarded(Option<int> input) -> int {
                    if let Option::some(value) = input { value } else { 0 }
                }

                fn absent() -> Option<int> { Option::none }
            }
            "#,
        )
        .expect("parse active-only sum program");
        analyze(&program).expect("active-only sums and expression control flow must type-check");
    }

    #[test]
    fn divergent_expression_arms_inhabit_the_sibling_value_type() {
        let program = parse(
            r#"
            seiyaku DivergentArms {
                fn result(Result<int, bool> input) -> Result<(int, int), bool> {
                    let payload = match input {
                        Result::ok(payload) => payload,
                        Result::err(failure) => { return Result::err(failure); },
                    };
                    Result::ok((payload, payload))
                }

                fn option(Option<int> input) -> Option<(int, int)> {
                    let payload = match input {
                        Option::some(payload) => payload,
                        Option::none => { return Option::none; },
                    };
                    Option::some((payload, payload))
                }

                fn choose(bool flag) -> int {
                    if flag { 7 } else { return 9; }
                }

                fn choose_if_let(Option<int> input) -> int {
                    if let Option::some(value) = input { value } else { return 0; }
                }

                fn choose_match(Option<int> input) -> int {
                    match input {
                        Option::some(value) => value,
                        Option::none => { return 0; },
                    }
                }
            }
            "#,
        )
        .expect("parse divergent expression arms");
        analyze(&program).expect("a returning arm must not synthesize a unit placeholder value");
    }

    #[test]
    fn discarded_branch_tail_does_not_count_as_function_return_coverage() {
        let program = parse(
            r#"
            fn incomplete(bool flag) -> int {
                if flag { 7 } else { return 9; }
                let still_falls_through = 1;
            }
            "#,
        )
        .expect("parse non-final mixed control flow");
        let error = analyze(&program)
            .expect_err("a discarded branch value cannot satisfy a declared return type");
        assert_eq!(error.code, "E_MISSING_RETURN");
    }

    #[test]
    fn wholly_divergent_expression_without_a_type_context_fails_closed() {
        let program = parse(
            r#"
            fn ambiguous(bool flag) {
                let unreachable = if flag { return; } else { return; };
            }
            "#,
        )
        .expect("parse context-free divergent expression");
        let error =
            analyze(&program).expect_err("bottom-like expressions require a concrete context");
        assert_eq!(error.code, "E_DIVERGING_EXPRESSION_CONTEXT");
    }

    #[test]
    fn sum_and_expression_control_flow_fail_closed() {
        for (source, code) in [
            (
                "fn f() { let _value = Option::none; }",
                "E_SUM_MISSING_CONTEXT",
            ),
            (
                "fn f(int value) -> Option<int> { value?; Option::none }",
                "E_PROPAGATE_TYPE",
            ),
            (
                "fn f(Result<int, string> value) -> Result<int, bytes> { Result::ok(value?) }",
                "E_PROPAGATE_ERROR_TYPE",
            ),
            (
                "fn f(Option<int> value) -> int { match value { Option::some(item) => item, } }",
                "E_MATCH_NON_EXHAUSTIVE",
            ),
            (
                "fn f(Option<int> value) -> int { match value { Option::some(item) => item, Option::some(other) => other, Option::none => 0, } }",
                "E_MATCH_DUPLICATE_PATTERN",
            ),
            (
                "fn f(Option<int> value) -> int { match value { Result::ok(item) => item, Result::err(_) => 0, } }",
                "E_PATTERN_FAMILY",
            ),
            (
                "fn f(bool flag) -> int { if flag { 1 } else { false } }",
                "E_BRANCH_TYPE_MISMATCH",
            ),
        ] {
            let program = parse(source).expect("compile-fail sum fixture must parse");
            let error = analyze(&program).expect_err("invalid sum/control-flow source must fail");
            assert_eq!(error.code, code, "{source}: {}", error.message);
        }
    }
}
