use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
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
use iroha_primitives::json::Json;
use norito::json::{self, native::Number as JsonNumber};

use super::ast::*;
use crate::builtins::{Builtin, BuiltinMode, BuiltinSurface, PointerConstructor};

/// First-release collection-iteration limit.
///
/// V1 accepts only compiler-proven literal bounds. This cap is part of the
/// language definition and therefore identical in every build.
pub const COLLECTION_ITERATION_LIMIT: i64 = 64;
const LINKED_SYMBOL_PREFIX: &str = "__kotodama_link_";

fn is_canonical_type_spelling(name: &str) -> bool {
    matches!(
        name,
        "i64"
            | "u128"
            | "bool"
            | "string"
            | "bytes"
            | "Amount"
            | "Json"
            | "AccountId"
            | "AssetDefinitionId"
            | "AssetId"
            | "DomainId"
            | "Name"
            | "NftId"
            | "DataSpaceId"
            | "AxtDescriptor"
            | "AssetHandle"
            | "ProofBlob"
            | "SoracloudRequest"
            | "SoracloudResponse"
            | "Option"
            | "Result"
            | "StateMap"
            | "Secret"
    )
}

/// Return whether a source declaration collides with compiler-owned names.
pub(crate) fn is_reserved_source_declaration(name: &str, is_function: bool) -> bool {
    name.starts_with(LINKED_SYMBOL_PREFIX)
        || name == STATE_MAP_GET_INTRINSIC
        || is_canonical_type_spelling(name)
        || (is_function
            && (Builtin::from_name(name).is_some() || Builtin::from_source_name(name).is_some()))
}

fn enforce_static_iteration_limit(form: &str, span: u128) -> Result<(), SemanticError> {
    let limit = u128::try_from(COLLECTION_ITERATION_LIMIT).expect("positive V1 iteration limit");
    if span > limit {
        return Err(SemanticError {
            message: format!(
                "E_ITERATION_LIMIT: `{form}` span {span} exceeds the Kotodama V1 limit {limit}"
            ),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    FixedU128,
    Amount,
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
    /// Presence-aware value represented as `(is_some, value)` in V1 IR.
    Option(Box<Type>),
    /// Success/error value represented as `(is_ok, ok_value, err_value)` in V1 IR.
    Result(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    /// User-defined product type with named fields.
    Struct {
        name: String,
        fields: Vec<(String, Type)>,
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
    /// Ternary conditional expression: `cond ? then : else`.
    Conditional {
        cond: Box<TypedExpr>,
        then_expr: Box<TypedExpr>,
        else_expr: Box<TypedExpr>,
    },
    Call {
        name: String,
        args: Vec<TypedExpr>,
    },
    Tuple(Vec<TypedExpr>),
    Member {
        object: Box<TypedExpr>,
        field: String,
    },
    Index {
        target: Box<TypedExpr>,
        index: Box<TypedExpr>,
    },
    Number(i64),
    Decimal(String),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
    Ident(String),
}

#[derive(Debug, PartialEq)]
pub struct SemanticError {
    pub message: String,
}

#[derive(Debug, PartialEq)]
pub(crate) struct SemanticFailure {
    pub(crate) error: SemanticError,
    pub(crate) location: Option<SourceLocation>,
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

#[derive(Debug, PartialEq)]
pub struct TypedProgram {
    pub unit: SourceUnit,
    pub items: Vec<TypedItem>,
    pub states: Vec<TypedStateDecl>,
    pub error_codes: Vec<TypedErrorCode>,
    pub triggers: Vec<TypedTrigger>,
    pub message_entries: Vec<MessageEntry>,
    /// Whether this HIR was analyzed with local test capabilities enabled.
    ///
    /// Production artifact builders reject test-capable HIR even when a
    /// caller removes the source-level test declarations after analysis. This
    /// provenance bit keeps the mode boundary fail-closed across typed-module
    /// linking and direct `build_typed_program` calls.
    pub test_support_enabled: bool,
}

/// Stable source-declared application error emitted in the contract interface.
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
    function_summaries: RefCell<HashMap<String, FunctionSummary>>,
    global_declarations: RefCell<HashSet<String>>,
    current_function_modifiers: RefCell<Option<FunctionModifiers>>,
    current_function_name: RefCell<Option<String>>,
    trigger_callback_functions: RefCell<HashSet<String>>,
    current_state_param_names: RefCell<HashSet<String>>,
    zk_enabled: bool,
    test_builtins_enabled: bool,
    error_codes: RefCell<HashMap<String, u32>>,
    external_functions: RefCell<BTreeMap<String, FunctionSignature>>,
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
        self.reset();
        self.external_functions.replace(external_functions.clone());
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
        let resolved_structs = self
            .structs
            .borrow()
            .clone()
            .into_iter()
            .map(|(name, fields)| {
                let fields = fields
                    .into_iter()
                    .map(|(field_name, field_ty)| {
                        (
                            field_name,
                            resolve_struct_type_with_context(self, &field_ty),
                        )
                    })
                    .collect();
                (name, fields)
            })
            .collect();
        self.structs.replace(resolved_structs);

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
            signatures.insert(
                function.name.clone(),
                FunctionSignature {
                    params,
                    return_type,
                },
            );
        }
        Ok(signatures)
    }

    pub(crate) fn analyze_all(&self, program: &Program) -> Result<TypedProgram, SemanticFailures> {
        self.reset();
        analyze_with_context(self, program)
    }

    fn reset(&self) {
        self.structs.borrow_mut().clear();
        self.states.borrow_mut().clear();
        self.consts.borrow_mut().clear();
        self.function_returns.borrow_mut().clear();
        self.function_modifiers.borrow_mut().clear();
        self.function_params.borrow_mut().clear();
        self.function_summaries.borrow_mut().clear();
        self.global_declarations.borrow_mut().clear();
        self.current_function_modifiers.borrow_mut().take();
        self.current_function_name.borrow_mut().take();
        self.trigger_callback_functions.borrow_mut().clear();
        self.current_state_param_names.borrow_mut().clear();
        self.error_codes.borrow_mut().clear();
        self.external_functions.borrow_mut().clear();
    }
}

fn validate_declaration_uniqueness(program: &Program) -> Result<Vec<String>, SemanticError> {
    let mut functions = HashSet::new();
    let mut types = HashSet::new();
    let mut states = HashSet::new();
    let mut consts = HashSet::new();
    let mut triggers = HashSet::new();
    if is_reserved_source_declaration(&program.unit.name, false) {
        return Err(SemanticError {
            message: format!(
                "E_RESERVED_DECLARATION: source unit `{}` uses a compiler-reserved name",
                program.unit.name
            ),
        });
    }
    let mut declarations = HashMap::from([(program.unit.name.clone(), "source unit")]);
    let mut global_error_codes = HashMap::new();
    let mut struct_names = Vec::new();

    let mut register_declaration = |name: &str,
                                    kind: &'static str,
                                    is_function: bool|
     -> Result<(), SemanticError> {
        if is_reserved_source_declaration(name, is_function) {
            return Err(SemanticError {
                message: format!(
                    "E_RESERVED_DECLARATION: {kind} `{name}` uses a compiler-reserved name"
                ),
            });
        }
        if let Some(previous_kind) = declarations.insert(name.to_owned(), kind) {
            return Err(SemanticError {
                message: format!(
                    "E_DUPLICATE_DECLARATION: declaration name `{name}` is already used by a {previous_kind}"
                ),
            });
        }
        Ok(())
    };

    for item in &program.items {
        match item {
            Item::Function(function) => {
                if !functions.insert(function.name.as_str()) {
                    return Err(SemanticError {
                        message: format!("duplicate function `{}`", function.name),
                    });
                }
                register_declaration(&function.name, "function", true)?;
                let mut params = HashSet::new();
                for param in &function.params {
                    if !params.insert(param.name.as_str()) {
                        return Err(SemanticError {
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
                        message: format!("duplicate type `{}`", definition.name),
                    });
                }
                register_declaration(&definition.name, "type", false)?;
                let mut fields = HashSet::new();
                for (field, _) in &definition.fields {
                    if !fields.insert(field.as_str()) {
                        return Err(SemanticError {
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
                        message: format!("duplicate type `{}`", definition.name),
                    });
                }
                register_declaration(&definition.name, "type", false)?;
                let mut variants = HashSet::new();
                let mut codes = HashSet::new();
                for variant in &definition.variants {
                    if !variants.insert(variant.name.as_str()) {
                        return Err(SemanticError {
                            message: format!(
                                "duplicate error variant `{}::{}`",
                                definition.name, variant.name
                            ),
                        });
                    }
                    if !codes.insert(variant.code) {
                        return Err(SemanticError {
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
                        message: format!("duplicate state `{}`", state.name),
                    });
                }
                register_declaration(&state.name, "state declaration", false)?;
            }
            Item::Const(constant) => {
                if !consts.insert(constant.name.as_str()) {
                    return Err(SemanticError {
                        message: format!("duplicate const `{}`", constant.name),
                    });
                }
                register_declaration(&constant.name, "const declaration", false)?;
            }
            Item::Trigger(trigger) => {
                if !triggers.insert(trigger.name.as_str()) {
                    return Err(SemanticError {
                        message: format!("duplicate trigger `{}`", trigger.name),
                    });
                }
                register_declaration(&trigger.name, "trigger declaration", false)?;
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
            Type::Result(ok, err) => {
                pending.push(err);
                pending.push(ok);
            }
            Type::Tuple(items) => pending.extend(items.iter().rev()),
            Type::Int
            | Type::FixedU128
            | Type::Amount
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

fn validate_acyclic_value_structs(
    context: &SemanticContext,
    struct_names: &[String],
) -> Result<(), SemanticError> {
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
                        return Err(SemanticError {
                            message: format!(
                                "cyclic value struct definition: {}",
                                cycle.join(" -> ")
                            ),
                        });
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

    Ok(())
}

fn validate_acyclic_function_calls(context: &SemanticContext) -> Result<(), SemanticError> {
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
                        return Err(SemanticError {
                            message: format!(
                                "recursive function calls are not supported in Kotodama V1: {}",
                                cycle.join(" -> ")
                            ),
                        });
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
    Ok(())
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
                    message: "E_TEST_ONLY_PRODUCTION: `koto_test` declarations require explicit compiler test mode"
                        .into(),
                },
                location: None,
            },
        );
    }
    for fixture in &program.fixtures {
        record_semantic_failure(
            &mut failures,
            &mut omitted,
            SemanticFailure {
                error: SemanticError {
                    message: format!(
                        "E_TEST_ONLY_PRODUCTION: fixture `{}` requires explicit compiler test mode",
                        fixture.name
                    ),
                },
                location: None,
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
                        message: format!(
                            "E_TEST_ONLY_PRODUCTION: test function `{}` requires explicit compiler test mode",
                            function.name
                        ),
                    },
                    location: Some(function.location),
                },
            );
        }
    }
    if omitted != 0 {
        failures.push(SemanticFailure {
            error: SemanticError {
                message: format!("K0004: {omitted} additional semantic error(s) were omitted"),
            },
            location: None,
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

fn analyze_with_context(
    context: &SemanticContext,
    program: &Program,
) -> Result<TypedProgram, SemanticFailures> {
    reject_test_surface_without_test_mode(context, program)?;
    // Collect definitions up front so source order does not affect name resolution.
    let mut structs: HashMap<String, Vec<(String, Type)>> = HashMap::new();
    let mut state_decls: Vec<(String, TypeExpr)> = Vec::new();
    let mut const_decls: Vec<ConstDecl> = Vec::new();
    let mut fn_returns: HashMap<String, Type> = HashMap::new();
    let mut fn_modifiers: HashMap<String, FunctionModifiers> = HashMap::new();
    let mut trigger_callbacks: HashSet<String> = HashSet::new();
    let mut error_codes = HashMap::new();
    let mut typed_error_codes = Vec::new();
    let struct_names = validate_declaration_uniqueness(program)?;
    context.global_declarations.replace(
        std::iter::once(program.unit.name.clone())
            .chain(program.items.iter().map(|item| match item {
                Item::Function(function) => function.name.clone(),
                Item::Struct(definition) => definition.name.clone(),
                Item::ErrorEnum(definition) => definition.name.clone(),
                Item::Const(constant) => constant.name.clone(),
                Item::State(state) => state.name.clone(),
                Item::Trigger(trigger) => trigger.name.clone(),
            }))
            .collect(),
    );
    context.structs.replace(
        struct_names
            .iter()
            .cloned()
            .map(|name| (name, Vec::new()))
            .collect(),
    );
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
    validate_acyclic_value_structs(context, &struct_names)?;
    let resolved_structs = context
        .structs
        .borrow()
        .clone()
        .into_iter()
        .map(|(name, fields)| {
            let fields = fields
                .into_iter()
                .map(|(field_name, field_ty)| {
                    (
                        field_name,
                        resolve_struct_type_with_context(context, &field_ty),
                    )
                })
                .collect();
            (name, fields)
        })
        .collect();
    context.structs.replace(resolved_structs);
    let mut fn_returns = fn_returns
        .into_iter()
        .map(|(name, ty)| (name, resolve_struct_type_with_context(context, &ty)))
        .collect::<HashMap<_, _>>();
    for (name, signature) in context.external_functions.borrow().iter() {
        if fn_returns
            .insert(name.clone(), signature.return_type.clone())
            .is_some()
        {
            return Err(SemanticError {
                message: format!("imported function `{name}` collides with a local function"),
            }
            .into());
        }
    }
    let mut resolved_consts: IndexMap<String, TypedExpr> = IndexMap::new();
    for decl in const_decls {
        let mut value = analyze_const_expr(&decl.value, &resolved_consts)?;
        let declared = decl.ty.as_ref().ok_or_else(|| SemanticError {
            message: format!("const `{}` requires an explicit type", decl.name),
        })?;
        let expected =
            resolve_struct_type_with_context(context, &convert_type_expr(context, declared)?);
        ensure_assignable_and_coerce(&expected, &mut value)?;
        resolved_consts.insert(decl.name, value);
    }
    context.consts.replace(resolved_consts);
    let mut state: IndexMap<String, Type> = IndexMap::new();
    for (name, ty_expr) in state_decls {
        let ty = resolve_struct_type_with_context(context, &convert_type_expr(context, &ty_expr)?);
        validate_state_type(&ty)?;
        state.insert(name, ty);
    }
    let resolved_state: IndexMap<String, Type> = state
        .into_iter()
        .map(|(name, ty)| (name, resolve_struct_type_with_context(context, &ty)))
        .collect();
    context.states.replace(resolved_state);
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

    let mut items = Vec::new();
    let states = context
        .states
        .borrow()
        .iter()
        .map(|(name, ty)| TypedStateDecl {
            name: name.clone(),
            ty: ty.clone(),
        })
        .collect::<Vec<_>>();
    let mut triggers = Vec::new();
    let mut trigger_names: HashSet<String> = HashSet::new();
    let mut failures = Vec::new();
    let mut omitted_failures = 0_usize;
    for item in &program.items {
        match item {
            Item::Function(f) => match analyze_function(context, f) {
                Ok(function) => items.push(TypedItem::Function(function)),
                Err(error) => record_semantic_failure(
                    &mut failures,
                    &mut omitted_failures,
                    SemanticFailure {
                        error,
                        location: Some(f.location),
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
                                message: format!("duplicate trigger `{}`", trigger.name),
                            },
                            location: None,
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
                            location: None,
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
                message: format!(
                    "K0004: {omitted_failures} additional semantic error(s) were omitted"
                ),
            },
            location: None,
        });
    }
    if !failures.is_empty() {
        return Err(SemanticFailures { failures });
    }
    validate_acyclic_function_calls(context)?;
    validate_scalar_state_initialization(context, &items, &states)?;
    let typed_program = TypedProgram {
        unit: program.unit.clone(),
        items,
        states,
        error_codes: typed_error_codes,
        triggers,
        message_entries: Vec::new(),
        test_support_enabled: context.test_builtins_enabled,
    };
    crate::secret::validate_program(&typed_program, context.zk_enabled)?;
    enforce_permission_requirements(context, &typed_program.items)?;
    Ok(typed_program)
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Int => "i64".into(),
        Type::FixedU128 => "u128".into(),
        Type::Amount => "Amount".into(),
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
        Type::Tuple(ts) => {
            let parts: Vec<String> = ts.iter().map(type_name).collect();
            format!("({})", parts.join(", "))
        }
        Type::Struct { name, .. } => format!("struct {name}"),
        Type::NamedStruct(s) => s.clone(),
    }
}

pub fn render_type_name(ty: &Type) -> String {
    type_name(ty)
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
            message: format!("invalid trigger name `{}`: {}", trigger.name, err),
        })?;
    let id = TriggerId::new(name);

    if trigger.call.namespace.is_none() {
        let entry = &trigger.call.entrypoint;
        let modifiers = fn_modifiers.get(entry).ok_or_else(|| SemanticError {
            message: format!(
                "trigger `{}` targets unknown entrypoint `{entry}`",
                trigger.name
            ),
        })?;
        if modifiers.kind == FunctionKind::View {
            return Err(SemanticError {
                message: format!(
                    "trigger `{}` cannot target read-only view entrypoint `{entry}`",
                    trigger.name
                ),
            });
        }
        if modifiers.visibility != FunctionVisibility::Public
            || modifiers.kind != FunctionKind::Contract
        {
            return Err(SemanticError {
                message: format!(
                    "trigger `{}` must call kotoage entrypoint `{entry}`",
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
                message: format!("invalid trigger metadata key `{}`: {err}", entry.key),
            })?;
        let json = json_from_expr(&entry.value)?;
        if metadata.insert(key, json).is_some() {
            return Err(SemanticError {
                message: format!("duplicate trigger metadata key `{}`", entry.key),
            });
        }
    }
    Ok(metadata)
}

fn json_from_expr(expr: &Expr) -> Result<Json, SemanticError> {
    let value = match expr {
        Expr::String(s) => json::Value::String(s.clone()),
        Expr::Number(n) => json::Value::Number(JsonNumber::I64(*n)),
        Expr::Decimal(raw) => {
            let value = raw.parse::<f64>().map_err(|err| SemanticError {
                message: format!("invalid decimal metadata literal `{raw}`: {err}"),
            })?;
            let number = JsonNumber::from_f64(value).ok_or_else(|| SemanticError {
                message: format!("invalid decimal metadata literal `{raw}`: not finite"),
            })?;
            json::Value::Number(number)
        }
        Expr::Bool(b) => json::Value::Bool(*b),
        Expr::Ident(ident) if ident == "null" => json::Value::Null,
        Expr::Call { name, args } if name == "json" => {
            let raw = match args.as_slice() {
                [Expr::String(raw)] => raw,
                _ => {
                    return Err(SemanticError {
                        message: "Json::parse(...) metadata values must be a string literal".into(),
                    });
                }
            };
            json::parse_value(raw).map_err(|err| SemanticError {
                message: format!("invalid json metadata literal: {err}"),
            })?
        }
        _ => {
            return Err(SemanticError {
                message: "trigger metadata values must be JSON literals".into(),
            });
        }
    };
    Json::from_norito_value_ref(&value).map_err(|err| SemanticError {
        message: format!("invalid trigger metadata value: {err}"),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericKind {
    Int,
    FixedU128,
    Amount,
}

fn numeric_kind(ty: &Type) -> Option<NumericKind> {
    match resolve_struct_type(ty) {
        Type::Int => Some(NumericKind::Int),
        Type::FixedU128 => Some(NumericKind::FixedU128),
        Type::Amount => Some(NumericKind::Amount),
        _ => None,
    }
}

fn numeric_kind_to_type(kind: NumericKind) -> Type {
    match kind {
        NumericKind::Int => Type::Int,
        NumericKind::FixedU128 => Type::FixedU128,
        NumericKind::Amount => Type::Amount,
    }
}

pub(crate) fn is_numeric_type(ty: &Type) -> bool {
    numeric_kind(ty).is_some()
}

pub(crate) fn is_wide_numeric_type(ty: &Type) -> bool {
    matches!(resolve_struct_type(ty), Type::FixedU128 | Type::Amount)
}

fn is_int_like(ty: &Type) -> bool {
    matches!(resolve_struct_type(ty), Type::Int)
}

fn numeric_result_type(lhs: &Type, rhs: &Type) -> Option<Type> {
    let lhs_resolved = resolve_struct_type(lhs);
    let rhs_resolved = resolve_struct_type(rhs);
    if lhs_resolved != rhs_resolved {
        return None;
    }
    numeric_kind(&lhs_resolved).map(numeric_kind_to_type)
}

fn literal_i64(expr: &TypedExpr) -> Option<i64> {
    match &expr.expr {
        ExprKind::Number(n) => Some(*n),
        ExprKind::NumericCast { expr } => literal_i64(expr),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => literal_i64(expr).and_then(|v| v.checked_neg()),
        _ => None,
    }
}

fn require_same_numeric_type(expr: &TypedExpr, expected: &Type) -> Result<(), SemanticError> {
    let expected = resolve_struct_type(expected);
    let actual = resolve_struct_type(&expr.ty);
    if expected == actual {
        return Ok(());
    }
    Err(SemanticError {
        message: format!(
            "numeric type mismatch: expected {}, got {}; implicit conversions are not part of Kotodama V1",
            type_name(&expected),
            type_name(&actual)
        ),
    })
}

fn explicit_numeric_conversion(
    name: &str,
    args: Vec<TypedExpr>,
) -> Option<Result<TypedExpr, SemanticError>> {
    let (source, destination) = match name {
        "u128::from_i64" => (Type::Int, Type::FixedU128),
        "Amount::from_i64" => (Type::Int, Type::Amount),
        "Amount::from_u128" => (Type::FixedU128, Type::Amount),
        _ => return None,
    };
    Some((|| {
        if args.len() != 1 || resolve_struct_type(&args[0].ty) != source {
            return Err(SemanticError {
                message: format!("{name} expects exactly one {} argument", type_name(&source)),
            });
        }
        if matches!(source, Type::Int) && literal_i64(&args[0]).is_some_and(|value| value < 0) {
            return Err(SemanticError {
                message: format!("{name} cannot convert a negative i64"),
            });
        }
        Ok(TypedExpr {
            expr: ExprKind::NumericCast {
                expr: Box::new(args.into_iter().next().expect("one argument checked")),
            },
            ty: destination,
        })
    })())
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
        _ => false,
    }
}

fn is_supported_public_argument_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Int
        | Type::FixedU128
        | Type::Amount
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
    match resolve_struct_type(ty) {
        ty if is_numeric_type(&ty) => true,
        Type::Bool | Type::String | Type::Bytes => true,
        other if is_pointer_type(&other) => true,
        _ => false,
    }
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
                message: format!(
                    "ephemeral map key type `{}` is not supported; use i64, bool, string, bytes, Json, or typed Iroha IDs",
                    type_name(&k)
                ),
            });
        }
        if !is_in_memory_map_word_type(&v) {
            return Err(SemanticError {
                message: format!(
                    "ephemeral map value type `{}` is not supported; use i64, bool, string, bytes, Json, or typed Iroha IDs",
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
            message:
                "E_SECRET_STATE_TYPE: durable state cannot contain Secret<T>; private inputs are execution-local"
                    .into(),
        });
    }
    match resolve_struct_type(ty) {
        Type::StateMap(k, v) => {
            if !allow_map {
                return Err(SemanticError {
                    message:
                        "nested StateMap is not supported in Kotodama V1; declare each StateMap as top-level state"
                            .into(),
                });
            }
            if !is_supported_durable_key_type(&k) {
                return Err(SemanticError {
                    message: format!(
                        "StateMap key type `{}` is not supported for durable storage; use a scalar canonical-Norito type",
                        type_name(&k)
                    ),
                });
            }
            if !is_supported_durable_value_type(&v) {
                return Err(SemanticError {
                    message: format!(
                        "StateMap value type `{}` is not supported for durable storage; use a canonical V1 value type",
                        type_name(&v)
                    ),
                });
            }
            Ok(())
        }
        Type::Struct { fields, .. } => {
            for (_, field_ty) in fields {
                validate_state_type_inner(&field_ty, false)?;
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
                    message: format!(
                        "state type `{}` is not supported for durable storage; use i64, u128, Amount, bool, Json, bytes, typed Iroha IDs, or aggregate V1 types",
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
    "(AccountId, AccountId, AssetDefinitionId, Amount) tuple entries";

fn is_transfer_batch_entry_tuple(ty: &Type) -> bool {
    match ty {
        Type::Tuple(fields) if fields.len() == 4 => {
            matches!(resolve_struct_type(&fields[0]), Type::AccountId)
                && matches!(resolve_struct_type(&fields[1]), Type::AccountId)
                && matches!(resolve_struct_type(&fields[2]), Type::AssetDefinitionId)
                && matches!(resolve_struct_type(&fields[3]), Type::Amount)
        }
        _ => false,
    }
}

fn ensure_transfer_batch_args(args: &[TypedExpr]) -> Result<(), SemanticError> {
    if args.is_empty() {
        return Err(SemanticError {
            message: "transfer_batch expects at least one entry".into(),
        });
    }
    if args
        .iter()
        .all(|expr| is_transfer_batch_entry_tuple(&expr.ty))
    {
        return Ok(());
    }
    Err(SemanticError {
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
            let member = TypedExpr {
                expr: ExprKind::Member {
                    object: Box::new(base_expr.clone()),
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
            let element_expr = if let ExprKind::Tuple(items) = &base_expr.expr {
                if let Some(item) = items.get(idx) {
                    item.clone()
                } else {
                    TypedExpr {
                        expr: ExprKind::Member {
                            object: Box::new(base_expr.clone()),
                            field: idx.to_string(),
                        },
                        ty: resolved_elem_ty.clone(),
                    }
                }
            } else {
                TypedExpr {
                    expr: ExprKind::Member {
                        object: Box::new(base_expr.clone()),
                        field: idx.to_string(),
                    },
                    ty: resolved_elem_ty.clone(),
                }
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
    if matches!(
        func.modifiers.kind,
        FunctionKind::Init | FunctionKind::Upgrade
    ) && func.modifiers.permission.is_some()
    {
        return Err(SemanticError {
            message: format!(
                "lifecycle function `{}` cannot declare caller authorization; lifecycle authorization is runtime-defined",
                func.name
            ),
        });
    }
    if func.modifiers.is_test {
        if !func.params.is_empty() {
            return Err(SemanticError {
                message: format!("test function `{}` must not declare parameters", func.name),
            });
        }
        if func.ret_ty.is_some() {
            return Err(SemanticError {
                message: format!(
                    "test function `{}` must not declare a return type",
                    func.name
                ),
            });
        }
        if func.modifiers.visibility != FunctionVisibility::Internal
            || !matches!(
                func.modifiers.kind,
                FunctionKind::Free | FunctionKind::Contract
            )
        {
            return Err(SemanticError {
                message: format!(
                    "test function `{}` must be declared as a local `fn`",
                    func.name
                ),
            });
        }
        if func.modifiers.permission.is_some() {
            return Err(SemanticError {
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
    // Seed variable environment with contract-level state declarations so
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
    let previous_modifiers = context
        .current_function_modifiers
        .borrow_mut()
        .replace(func.modifiers.clone());
    let previous_name = context
        .current_function_name
        .borrow_mut()
        .replace(func.name.clone());
    let previous_state_params = std::mem::replace(
        &mut *context.current_state_param_names.borrow_mut(),
        state_param_names.clone(),
    );
    let body_result = analyze_block(
        context,
        &func.body,
        &mut vars,
        &mut mutable_bindings,
        expected_ret.as_ref(),
        0,
    );
    *context.current_function_modifiers.borrow_mut() = previous_modifiers;
    *context.current_function_name.borrow_mut() = previous_name;
    *context.current_state_param_names.borrow_mut() = previous_state_params;
    let body = body_result?;
    // Enforce declared return coverage and shape
    if let Some(t) = &expected_ret {
        if *t != Type::Unit && !block_returns_all_paths(&func.body) {
            return Err(SemanticError {
                message: "not all paths return a value".into(),
            });
        }
    } else {
        // No declared return type: disallow returning a value to avoid ambiguity
        if block_has_return_value(&func.body) {
            return Err(SemanticError {
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
    })
}

fn reject_public_trigger_event(context: &SemanticContext, name: &str) -> Result<(), SemanticError> {
    let forbidden = context
        .current_function_modifiers
        .borrow()
        .as_ref()
        .is_some_and(|modifiers| {
            if modifiers.kind == FunctionKind::View {
                return true;
            }
            modifiers.visibility == FunctionVisibility::Public
                && !current_public_trigger_callback_allows_payload_helper(context)
        });
    if forbidden {
        return Err(SemanticError {
            message: format!(
                "public and view entrypoints cannot use `{name}` here; declare typed parameters instead"
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
    modifiers.visibility == FunctionVisibility::Public
        || matches!(
            modifiers.kind,
            FunctionKind::View | FunctionKind::Init | FunctionKind::Upgrade
        )
}

fn invoke_entrypoint_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::String(raw) => Some(raw.clone()),
        Expr::Call { name, args } if normalize_namespaced(name) == "name" && args.len() == 1 => {
            match &args[0] {
                Expr::String(raw) => Some(raw.clone()),
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
            message: "require expects (bool, ErrorEnum::Variant)".into(),
        });
    }
    let Expr::Ident(error_variant) = &args[1] else {
        return Err(SemanticError {
            message: "require expects a declared error variant as its second argument".into(),
        });
    };
    if !context.error_codes.borrow().contains_key(error_variant) {
        return Err(SemanticError {
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
            message: "`invoke_entrypoint` is only available inside #[test] Kotodama functions"
                .into(),
        });
    }
    if args.len() != 2 {
        return Err(SemanticError {
            message: "invoke_entrypoint expects (string|Name literal, Json)".into(),
        });
    }

    let target_name = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        message:
            "invoke_entrypoint requires a literal entrypoint name such as \"run\" or Name::parse(\"run\")"
                .into(),
    })?;
    let payload = analyze_expr(context, &args[1], vars)?;
    if payload.ty != Type::Json {
        return Err(SemanticError {
            message: "invoke_entrypoint expects a Json payload as its second argument".into(),
        });
    }

    let Some(modifiers) = context
        .function_modifiers
        .borrow()
        .get(&target_name)
        .cloned()
    else {
        return Err(SemanticError {
            message: format!("invoke_entrypoint targets unknown function `{target_name}`"),
        });
    };
    if !function_is_runtime_entrypoint(&modifiers) {
        return Err(SemanticError {
            message: format!(
                "invoke_entrypoint may only target kotoage/view/hajimari/kaizen entrypoints, got `{target_name}`"
            ),
        });
    }

    let ret_ty = context
        .function_returns
        .borrow()
        .get(&target_name)
        .cloned()
        .unwrap_or(Type::Unit);

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
    let Some(modifiers) = context
        .function_modifiers
        .borrow()
        .get(target_name)
        .cloned()
    else {
        return Err(SemanticError {
            message: format!("unknown runtime entrypoint `{target_name}`"),
        });
    };
    if !function_is_runtime_entrypoint(&modifiers) {
        return Err(SemanticError {
            message: format!(
                "runtime test helpers may only target kotoage/view/hajimari/kaizen entrypoints, got `{target_name}`"
            ),
        });
    }
    Ok(context
        .function_returns
        .borrow()
        .get(target_name)
        .cloned()
        .unwrap_or(Type::Unit))
}

fn analyze_invoke_entrypoint_as_call(
    context: &SemanticContext,
    args: &[Expr],
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    if !current_function_is_test(context) {
        return Err(SemanticError {
            message: "`invoke_entrypoint_as` is only available inside #[test] Kotodama functions"
                .into(),
        });
    }
    if args.len() != 3 {
        return Err(SemanticError {
            message: "invoke_entrypoint_as expects (string|Name literal actor, string|Name literal entrypoint, Json)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        message: "invoke_entrypoint_as requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")".into(),
    })?;
    let target_name = invoke_entrypoint_literal(&args[1]).ok_or_else(|| SemanticError {
        message: "invoke_entrypoint_as requires a literal entrypoint name such as \"run\" or Name::parse(\"run\")".into(),
    })?;
    let payload = analyze_expr(context, &args[2], vars)?;
    if payload.ty != Type::Json {
        return Err(SemanticError {
            message: "invoke_entrypoint_as expects a Json payload as its third argument".into(),
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
            message: "`expect_reject_as` is only available inside #[test] Kotodama functions"
                .into(),
        });
    }
    if args.len() != 3 {
        return Err(SemanticError {
            message: "expect_reject_as expects (string|Name literal actor, string|Name literal entrypoint, Json)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        message:
            "expect_reject_as requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")"
                .into(),
    })?;
    let target_name = invoke_entrypoint_literal(&args[1]).ok_or_else(|| SemanticError {
        message:
            "expect_reject_as requires a literal entrypoint name such as \"run\" or Name::parse(\"run\")"
                .into(),
    })?;
    let payload = analyze_expr(context, &args[2], vars)?;
    if payload.ty != Type::Json {
        return Err(SemanticError {
            message: "expect_reject_as expects a Json payload as its third argument".into(),
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
            message: "`actor_account` is only available inside #[test] Kotodama functions".into(),
        });
    }
    if args.len() != 1 {
        return Err(SemanticError {
            message: "actor_account expects (string|Name literal actor)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
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
            message: "`actor_public_key` is only available inside #[test] Kotodama functions"
                .into(),
        });
    }
    if args.len() != 1 {
        return Err(SemanticError {
            message: "actor_public_key expects (string|Name literal actor)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
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
            message: "`actor_sign` is only available inside #[test] Kotodama functions".into(),
        });
    }
    if args.len() != 2 {
        return Err(SemanticError {
            message: "actor_sign expects (string|Name literal actor, bytes)".into(),
        });
    }
    let actor = invoke_entrypoint_literal(&args[0]).ok_or_else(|| SemanticError {
        message: "actor_sign requires a literal actor alias such as \"issuer\" or Name::parse(\"issuer\")"
            .into(),
    })?;
    let message = analyze_expr(context, &args[1], vars)?;
    if !is_blob_like(&message.ty) {
        return Err(SemanticError {
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
    loop_depth: usize,
) -> Result<TypedBlock, SemanticError> {
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
    Ok(TypedBlock { statements })
}

fn validate_v1_bounded_for_shape(
    init: &Option<Box<Statement>>,
    cond: &Option<Expr>,
    step: &Option<Box<Statement>>,
) -> Result<(), SemanticError> {
    let Some(Statement::Let {
        mutable: true,
        pat: Pattern::Name(variable),
        ty: None,
        value: Expr::Number(0),
    }) = init.as_deref()
    else {
        return Err(SemanticError {
            message: "E_UNBOUNDED_LOOP: only `for item in range(non_negative_literal)` is supported in Kotodama V1"
                .into(),
        });
    };
    let Some(Expr::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    }) = cond
    else {
        return Err(SemanticError {
            message:
                "E_UNBOUNDED_LOOP: bounded range loop is missing its compiler-proven condition"
                    .into(),
        });
    };
    if !matches!(left.as_ref(), Expr::Ident(name) if name == variable)
        || !matches!(right.as_ref(), Expr::Number(value) if *value >= 0)
    {
        return Err(SemanticError {
            message: "E_UNBOUNDED_LOOP: range bounds must be non-negative integer literals".into(),
        });
    }
    let Some(Statement::Assign {
        name,
        value:
            Expr::Binary {
                op: BinaryOp::Add,
                left: step_left,
                right: step_right,
            },
    }) = step.as_deref()
    else {
        return Err(SemanticError {
            message: "E_UNBOUNDED_LOOP: bounded range loop is missing its canonical step".into(),
        });
    };
    if name != variable
        || !matches!(step_left.as_ref(), Expr::Ident(name) if name == variable)
        || !matches!(step_right.as_ref(), Expr::Number(1))
    {
        return Err(SemanticError {
            message: "E_UNBOUNDED_LOOP: range loop control variables cannot be rewritten".into(),
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
    let _ = loop_depth;
    match stmt {
        Statement::Let {
            mutable,
            pat,
            ty,
            value,
        } => {
            let mut expr = analyze_expr(context, value, vars)?;
            if let Some(tann) = ty {
                let dt = convert_type_expr(context, tann)?;
                apply_map_new_type_hint(&mut expr, &dt);
                ensure_assignable_and_coerce(&dt, &mut expr)?;
            }
            if is_state_map_expr(context, &expr) {
                return Err(SemanticError {
                    message: "E_STATE_MAP_ALIAS: state maps are not first-class; use the state identifier directly.".into(),
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
                                // If the RHS is a struct constructor (tuple of field exprs), bind directly.
                                let val_expr = if let ExprKind::Tuple(ts) = &expr.expr {
                                    ts.get(i).cloned().unwrap_or(TypedExpr {
                                        expr: ExprKind::Number(0),
                                        ty: fty.clone(),
                                    })
                                } else {
                                    TypedExpr {
                                        expr: ExprKind::Member {
                                            object: Box::new(expr.clone()),
                                            field: i.to_string(),
                                        },
                                        ty: fty.clone(),
                                    }
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
                                    message: format!(
                                        "tuple destructuring expects {} bindings, got {}",
                                        ts.len(),
                                        names.len()
                                    ),
                                });
                            }
                            // Destructure by emitting member-access typed expressions for each field.
                            for (i, name) in names.iter().enumerate() {
                                let ti = ts.get(i).cloned().expect("tuple arity already validated");
                                let member = TypedExpr {
                                    expr: ExprKind::Member {
                                        object: Box::new(expr.clone()),
                                        field: i.to_string(),
                                    },
                                    ty: ti.clone(),
                                };
                                if name != "_" {
                                    vars.insert(name.clone(), ti.clone());
                                    if *mutable {
                                        mutable_bindings.insert(name.clone());
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
                                    message: format!(
                                        "struct destructuring expects {} bindings, got {}",
                                        fields.len(),
                                        names.len()
                                    ),
                                });
                            }
                            for (i, name) in names.iter().enumerate() {
                                let (_fname, ti) = fields
                                    .get(i)
                                    .cloned()
                                    .expect("struct arity already validated");
                                let val_expr = if let ExprKind::Tuple(ts) = &expr.expr {
                                    ts.get(i).cloned().unwrap_or(TypedExpr {
                                        expr: ExprKind::Number(0),
                                        ty: resolve_struct_type(&ti),
                                    })
                                } else {
                                    TypedExpr {
                                        expr: ExprKind::Member {
                                            object: Box::new(expr.clone()),
                                            field: i.to_string(),
                                        },
                                        ty: resolve_struct_type(&ti),
                                    }
                                };
                                let field_ty = resolve_struct_type(&ti);
                                if name != "_" {
                                    vars.insert(name.clone(), field_ty.clone());
                                    if *mutable {
                                        mutable_bindings.insert(name.clone());
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
                                message: "tuple destructuring expects a tuple or struct".into(),
                            });
                        }
                    }
                    Ok(out)
                }
            }
        }
        Statement::Assign { name, value } => {
            // Must exist
            let expected = vars.get(name).cloned().ok_or_else(|| SemanticError {
                message: format!("undefined variable {name}"),
            })?;
            if is_state_binding(context, name)
                && matches!(resolve_struct_type(&expected), Type::StateMap(_, _))
            {
                return Err(SemanticError {
                    message:
                        "E_STATE_MAP_ALIAS: state maps cannot be reassigned; use map indexing."
                            .into(),
                });
            }
            ensure_mutable_assignment_target(context, name, mutable_bindings)?;
            let mut expr = analyze_expr(context, value, vars)?;
            if is_state_binding(context, name) {
                crate::secret::reject_secret_state_value(&expr)?;
            }
            if is_state_map_expr(context, &expr) {
                return Err(SemanticError {
                    message: "E_STATE_MAP_ALIAS: state maps are not first-class; use the state identifier directly.".into(),
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
            match target {
                Expr::Index { target: map, index } => {
                    let map_t = analyze_expr(context, map, vars)?;
                    let mut key_t = analyze_expr(context, index, vars)?;
                    crate::secret::reject_secret_key(&key_t)?;
                    match map_t.ty.clone() {
                        Type::StateMap(k, v) => {
                            ensure_assignable_and_coerce(&k, &mut key_t)?;
                            ensure_in_memory_map_word_types(context, &map_t)?;
                            if *op == AssignOp::Set {
                                let mut val_t = analyze_expr(context, value, vars)?;
                                crate::secret::reject_secret_state_value(&val_t)?;
                                ensure_assignable_and_coerce(&v, &mut val_t)?;
                                return Ok(vec![TypedStatement::MapSet {
                                    map: map_t,
                                    key: key_t,
                                    value: val_t,
                                }]);
                            }
                            Err(SemanticError {
                                message: "E_STATE_MAP_OPTIONAL_READ: compound StateMap assignment reads a possibly absent key; use `map.get(key)` and handle Option<V> before assigning with `map[key] = value`"
                                    .into(),
                            })
                        }
                        other => Err(SemanticError {
                            message: format!(
                                "map assignment expects StateMap<K,V> target, got {}",
                                type_name(&other)
                            ),
                        }),
                    }
                }
                Expr::Ident(name) => {
                    // Simple compound assignment lowering: rebind SSA name
                    let expected = vars.get(name).cloned().ok_or_else(|| SemanticError {
                        message: format!("undefined variable {name}"),
                    })?;
                    if is_state_binding(context, name)
                        && matches!(resolve_struct_type(&expected), Type::StateMap(_, _))
                    {
                        return Err(SemanticError {
                            message:
                                "E_STATE_MAP_ALIAS: state maps cannot be reassigned; use map indexing."
                                    .into(),
                        });
                    }
                    ensure_mutable_assignment_target(context, name, mutable_bindings)?;
                    let mut expr = analyze_expr(context, value, vars)?;
                    if is_state_binding(context, name) {
                        crate::secret::reject_secret_state_value(&expr)?;
                    }
                    if is_state_map_expr(context, &expr) {
                        return Err(SemanticError {
                            message: "E_STATE_MAP_ALIAS: state maps are not first-class; use the state identifier directly.".into(),
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
                    if numeric_result_type(&expected, &expr.ty).is_none() {
                        return Err(SemanticError {
                            message: format!(
                                "{op:?} requires identical numeric operand types; implicit conversions are not part of Kotodama V1"
                            ),
                        });
                    }
                    require_same_numeric_type(&expr, &expected)?;
                    let result_ty = expected.clone();
                    let left = TypedExpr {
                        expr: ExprKind::Ident(name.clone()),
                        ty: expected.clone(),
                    };
                    let bin_op = assign_op_to_binary(*op).expect("compound op maps to binary op");
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
                    message: "assignment target must be a variable or map index".into(),
                }),
            }
        }
        Statement::Expr(e) => Ok(vec![TypedStatement::Expr(analyze_expr(context, e, vars)?)]),
        Statement::Return(opt) => {
            let mut tv = if let Some(e) = opt {
                Some(analyze_expr(context, e, vars)?)
            } else {
                None
            };
            if expected_ret.is_none() {
                if tv.is_some() {
                    return Err(SemanticError {
                        message: "returning a value requires a declared return type".into(),
                    });
                }
            } else if let Some(exp) = expected_ret {
                match tv.as_mut() {
                    None => {
                        if !matches!(exp, Type::Unit) {
                            return Err(SemanticError {
                                message: "return type mismatch: expected value".into(),
                            });
                        }
                    }
                    Some(expr) => {
                        apply_map_new_type_hint(expr, exp);
                        if matches!(exp, Type::Unit) {
                            return Err(SemanticError {
                                message: "return type mismatch: unexpected value".into(),
                            });
                        }
                        if let Err(mut err) = ensure_assignable_and_coerce(exp, expr) {
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
                    message: "E_BREAK_OUTSIDE_LOOP: `break` must appear inside a loop".into(),
                });
            }
            Ok(vec![TypedStatement::Break])
        }
        Statement::Continue => {
            if loop_depth == 0 {
                return Err(SemanticError {
                    message: "E_CONTINUE_OUTSIDE_LOOP: `continue` must appear inside a loop".into(),
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
                    message: "if condition must be bool".into(),
                });
            }
            let then_block = analyze_block(
                context,
                then_branch,
                &mut vars.clone(),
                &mut mutable_bindings.clone(),
                expected_ret,
                loop_depth,
            )?;
            let else_block = if let Some(b) = else_branch {
                Some(analyze_block(
                    context,
                    b,
                    &mut vars.clone(),
                    &mut mutable_bindings.clone(),
                    expected_ret,
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
        Statement::While { .. } => Err(SemanticError {
            message: "E_UNBOUNDED_LOOP: `while` is not part of Kotodama V1; use a compiler-proven bounded `for` loop"
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
                        message: "E0005: for-loop initializer must be a simple let or expression"
                            .into(),
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
                        message: "E0006: for-loop step must be a simple let or expression".into(),
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
            if let Expr::Call { name, args } = map {
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
                        loop_depth + 1,
                    )?;
                    let literal_bound = match &args[1] {
                        Expr::Number(n) if *n >= 0 => {
                            enforce_static_iteration_limit("StateMap.take(N)", *n as u128)?;
                            Some(usize::try_from(*n).expect("V1 iteration bound is at most 64"))
                        }
                        _ => None,
                    };
                    if let Some(bound) = literal_bound {
                        if bound > 1 && !map_expr_is_state(context, &args[0]) {
                            return Err(SemanticError {
                                message: "E_MAP_BOUNDS: ephemeral map iteration supports at most 1 element; reduce the bound or move the map into `state`.".into(),
                            });
                        }
                        // E_ITER_MUTATION: forbid structural modifications to the iterated map inside the loop body
                        if let Expr::Ident(map_name) = &args[0]
                            && block_mutates_map(&body_t, map_name)
                        {
                            return Err(SemanticError { message: "E_ITER_MUTATION: structural modifications to the iterated map are forbidden during iteration".into() });
                        }
                        return Ok(vec![TypedStatement::ForEachMap {
                            key: key.clone(),
                            value: value.clone(),
                            map: base_map,
                            body: body_t,
                            start: 0,
                            bound: Some(bound),
                        }]);
                    }
                    return Err(SemanticError {
                        message: "E_UNBOUNDED_ITERATION: `.take(n)` requires a non-negative i64 literal no greater than 64".into(),
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
                        loop_depth + 1,
                    )?;
                    let start = match &args[1] {
                        Expr::Number(n) if *n >= 0 => Some(*n as usize),
                        _ => None,
                    };
                    // Interpret second numeric as end; compute n = end - start
                    let end = match &args[2] {
                        Expr::Number(n) if *n >= 0 => Some(*n as usize),
                        _ => None,
                    };
                    if let (Some(start), Some(end)) = (start, end) {
                        if end < start {
                            return Err(SemanticError {
                                message:
                                    "E_UNBOUNDED_ITERATION: `.range(start, end)` requires end >= start"
                                        .into(),
                            });
                        }
                        let span = end - start;
                        enforce_static_iteration_limit("StateMap.range(start, end)", span as u128)?;
                        if !map_expr_is_state(context, &args[0]) && (start != 0 || span > 1) {
                            return Err(SemanticError {
                                message: "E_MAP_BOUNDS: ephemeral map iteration supports at most 1 element starting at index 0; reduce the range or move the map into `state`."
                                    .into(),
                            });
                        }
                        let static_bound = Some(span);
                        if let Expr::Ident(map_name) = &args[0]
                            && block_mutates_map(&body_t, map_name)
                        {
                            return Err(SemanticError { message: "E_ITER_MUTATION: structural modifications to the iterated map are forbidden during iteration".into() });
                        }
                        return Ok(vec![TypedStatement::ForEachMap {
                            key: key.clone(),
                            value: value.clone(),
                            map: base_map,
                            body: body_t,
                            start,
                            bound: static_bound,
                        }]);
                    }
                    return Err(SemanticError {
                        message: "E_UNBOUNDED_ITERATION: `.range(start, end)` requires non-negative i64 literals with a span no greater than 64".into(),
                    });
                }
            }
            Err(SemanticError {
                message: "E_UNBOUNDED_ITERATION: `for (k, v) in map` requires a literal bound; call `.take(N)` or `.range(start, end)` on the StateMap expression.".into(),
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
        Builtin::QueryGetAccount => matches!(ty, Type::AccountId) || is_blob_like(ty),
        Builtin::QueryGetAsset => matches!(ty, Type::AssetId) || is_blob_like(ty),
        Builtin::QueryGetAssetDefinition => {
            matches!(ty, Type::AssetDefinitionId) || is_blob_like(ty)
        }
        Builtin::QueryGetDomain => matches!(ty, Type::DomainId) || is_blob_like(ty),
        Builtin::QueryGetNft => matches!(ty, Type::NftId) || is_blob_like(ty),
        Builtin::QueryGetParameter => matches!(ty, Type::Name) || is_blob_like(ty),
        Builtin::QueryGetContractInstance => matches!(ty, Type::Name) || is_blob_like(ty),
        _ => false,
    }
}

fn direct_json_getter_type(builtin: Builtin) -> Option<Type> {
    Some(match builtin {
        Builtin::JsonGetIntDirect => Type::Int,
        Builtin::JsonGetNumericDirect => Type::Amount,
        Builtin::JsonGetJsonDirect => Type::Json,
        Builtin::JsonGetNameDirect => Type::Name,
        Builtin::JsonGetAccountIdDirect => Type::AccountId,
        Builtin::JsonGetAssetDefinitionIdDirect => Type::AssetDefinitionId,
        Builtin::JsonGetNftIdDirect => Type::NftId,
        Builtin::JsonGetBlobHexDirect => Type::Bytes,
        _ => return None,
    })
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

fn analyze_surface_builtin_call(
    context: &SemanticContext,
    builtin: Builtin,
    mut arg_typed: Vec<TypedExpr>,
) -> Result<TypedExpr, SemanticError> {
    match builtin.spec().mode {
        BuiltinMode::CompilerInternal => {
            return Err(SemanticError {
                message: format!(
                    "E_INTERNAL_BUILTIN: builtin `{}` is compiler-internal and is not available in Kotodama V1 source",
                    builtin.name()
                ),
            });
        }
        BuiltinMode::ZkOnly if !context.zk_enabled => {
            return Err(SemanticError {
                message: format!(
                    "builtin `{}` requires ZK mode in compiler build configuration",
                    builtin.name()
                ),
            });
        }
        BuiltinMode::TestOnly | BuiltinMode::TestFunctionOnly if !context.test_builtins_enabled => {
            return Err(SemanticError {
                message: format!(
                    "E_TEST_ONLY_PRODUCTION: builtin `{}` requires explicit compiler test mode",
                    builtin.source_name()
                ),
            });
        }
        BuiltinMode::TestFunctionOnly if !current_function_is_test(context) => {
            return Err(SemanticError {
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
    crate::secret::validate_builtin_call(builtin, &arg_typed)?;
    match builtin {
        Builtin::PointerConstructor(constructor) => {
            let name = constructor.name();
            if arg_typed.len() != 1 {
                return Err(SemanticError {
                    message: format!("{name} expects one argument"),
                });
            }
            let arg_ty = resolve_struct_type(&arg_typed[0].ty);
            let ty = pointer_constructor_type(constructor);
            if arg_ty != Type::String {
                return Err(SemanticError {
                    message: format!("{name} expects string"),
                });
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
                    message: "get_or_default expects (StateMap<K,V>, K, V)".into(),
                });
            }
            let (key_ty, value_ty) = match &arg_typed[0].ty {
                Type::StateMap(k, v) => (k.as_ref().clone(), v.as_ref().clone()),
                other => {
                    return Err(SemanticError {
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
                    message: "get_or expects (StateMap<K,V>, K[, V])".into(),
                });
            }
            let mut call_args = arg_typed;
            let map_ty = resolve_struct_type(&call_args[0].ty);
            let (map_key_ty, map_value_ty) = match map_ty {
                Type::StateMap(k, v) => (*k, *v),
                other => {
                    return Err(SemanticError {
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
                            expr: ExprKind::Number(0),
                            ty: Type::Int,
                        });
                    }
                    other => {
                        if is_pointer_type(&other) {
                            return Err(SemanticError {
                                message: format!(
                                    "get_or requires an explicit default for pointer-valued maps (value type {})",
                                    type_name(&other)
                                ),
                            });
                        }
                        return Err(SemanticError {
                            message: format!(
                                "get_or auto-default is only available for StateMap<*,i64>; provide an explicit default for value type {}",
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
                    message: "view entrypoints cannot use mutating map helper `ensure`; use `get_or` instead".into(),
                });
            }
            let original_len = arg_typed.len();
            if original_len != 2 && original_len != 3 {
                return Err(SemanticError {
                    message: "ensure expects (StateMap<K,V>, K[, V])".into(),
                });
            }
            let mut call_args = arg_typed;
            let map_ty = resolve_struct_type(&call_args[0].ty);
            let (map_key_ty, map_value_ty) = match map_ty {
                Type::StateMap(k, v) => (*k, *v),
                other => {
                    return Err(SemanticError {
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
                            expr: ExprKind::Number(0),
                            ty: Type::Int,
                        });
                    }
                    other => {
                        if is_pointer_type(&other) {
                            return Err(SemanticError {
                                message: format!(
                                    "ensure requires an explicit default for pointer-valued maps (value type {})",
                                    type_name(&other)
                                ),
                            });
                        }
                        return Err(SemanticError {
                            message: format!(
                                "ensure auto-default is only available for StateMap<*,i64>; provide an explicit default for value type {}",
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
                    message: "StateMap.remove expects exactly one key argument".into(),
                });
            }
            if !typed_map_expr_is_state(context, &arg_typed[0]) {
                return Err(SemanticError {
                    message: "StateMap.remove is available only on declared durable state maps"
                        .into(),
                });
            }
            let Type::StateMap(key, value) = resolve_struct_type(&arg_typed[0].ty) else {
                return Err(SemanticError {
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
                    message: format!("{name} expects (StateMap<i64,i64>, i64 start, i64 which)"),
                });
            }
            match &arg_typed[0].ty {
                Type::StateMap(k, v)
                    if matches!(resolve_struct_type(k), Type::Int)
                        && matches!(resolve_struct_type(v), Type::Int) => {}
                other => {
                    return Err(SemanticError {
                        message: format!(
                            "{name} expects StateMap<i64,i64> as first arg, got {}",
                            type_name(other)
                        ),
                    });
                }
            }
            if !matches!(resolve_struct_type(&arg_typed[1].ty), Type::Int)
                || !matches!(resolve_struct_type(&arg_typed[2].ty), Type::Int)
            {
                return Err(SemanticError {
                    message: format!("{name} expects (StateMap<i64,i64>, i64, i64)"),
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
                    message: "keys_values_take2 expects (StateMap<i64,i64>, i64, i64)".into(),
                });
            }
            match &arg_typed[0].ty {
                Type::StateMap(k, v)
                    if matches!(resolve_struct_type(k), Type::Int)
                        && matches!(resolve_struct_type(v), Type::Int) => {}
                other => {
                    return Err(SemanticError {
                        message: format!(
                            "keys_values_take2 expects StateMap<i64,i64> as first arg, got {}",
                            type_name(other)
                        ),
                    });
                }
            }
            if !matches!(resolve_struct_type(&arg_typed[1].ty), Type::Int)
                || !matches!(resolve_struct_type(&arg_typed[2].ty), Type::Int)
            {
                return Err(SemanticError {
                    message: "keys_values_take2 expects (StateMap<i64,i64>, i64, i64)".into(),
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
                    message: "state_keys expects (Name, i64 offset, i64 limit)".into(),
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
        Builtin::QueryExecuteNorito
        | Builtin::QueryGetAccount
        | Builtin::QueryGetAsset
        | Builtin::QueryGetAssetDefinition
        | Builtin::QueryGetDomain
        | Builtin::QueryGetNft
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
                    Builtin::QueryGetAccount => "query_get_account expects (AccountId|bytes)",
                    Builtin::QueryGetAsset => "query_get_asset expects (AssetId|bytes)",
                    Builtin::QueryGetAssetDefinition => {
                        "query_get_asset_definition expects (AssetDefinitionId|bytes)"
                    }
                    Builtin::QueryGetDomain => "query_get_domain expects (DomainId|bytes)",
                    Builtin::QueryGetNft => "query_get_nft expects (NftId|bytes)",
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
                && is_int_like(&arg_typed[2].ty)
                && is_blob_like(&arg_typed[3].ty)
                && arg_typed[4].ty == Type::String
                && is_blob_like(&arg_typed[5].ty)
                && is_blob_like(&arg_typed[6].ty);
            let valid_with_outputs = arg_typed.len() == 8
                && arg_typed[0].ty == Type::AssetDefinitionId
                && arg_typed[1].ty == Type::AccountId
                && is_int_like(&arg_typed[2].ty)
                && is_blob_like(&arg_typed[3].ty)
                && is_blob_like(&arg_typed[4].ty)
                && arg_typed[5].ty == Type::String
                && is_blob_like(&arg_typed[6].ty)
                && is_blob_like(&arg_typed[7].ty);
            if !(valid_without_outputs || valid_with_outputs) {
                return Err(SemanticError {
                    message: "build_unshield_inline expects (AssetDefinitionId, AccountId, i64 amount, bytes inputs32, [bytes outputs32,] string backend, bytes proof, bytes vk)".into(),
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
        Builtin::RecordSccpMessage
        | Builtin::ScExecuteSubmitBallot
        | Builtin::ScExecuteUnshield => {
            let name = builtin.name();
            if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                return Err(SemanticError {
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
        Builtin::CallContract => {
            if arg_typed.len() != 3
                || !(arg_typed[0].ty == Type::String || is_blob_like(&arg_typed[0].ty))
                || !(arg_typed[1].ty == Type::String || is_blob_like(&arg_typed[1].ty))
                || arg_typed[2].ty != Type::Json
            {
                return Err(SemanticError {
                    message: "call_contract expects (string|bytes, string|bytes, Json)".into(),
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
        Builtin::ResolveAccountAlias => {
            if arg_typed.len() != 1
                || !(arg_typed[0].ty == Type::String || is_blob_like(&arg_typed[0].ty))
            {
                return Err(SemanticError {
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
            if arg_typed.len() != 4 {
                return Err(SemanticError {
                    message: "vrf_verify expects (bytes, bytes, bytes, i64 variant)".into(),
                });
            }
            if arg_typed[..3].iter().any(|t| !is_blob_like(&t.ty)) || !is_int_like(&arg_typed[3].ty)
            {
                return Err(SemanticError {
                    message: "vrf_verify expects (bytes, bytes, bytes, i64 variant)".into(),
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
                    message: "sm2_verify expects (bytes, bytes, bytes) or (bytes, bytes, bytes, bytes) where arguments reference INPUT TLVs".into(),
                });
            }
            if arg_typed[..3].iter().any(|t| !is_blob_like(&t.ty)) {
                return Err(SemanticError {
                    message:
                        "sm2_verify expects message, signature, and public key as bytes pointers"
                            .into(),
                });
            }
            if arg_typed.len() == 4 && !is_blob_like(&arg_typed[3].ty) {
                return Err(SemanticError {
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
                    message: "verify_signature expects (bytes, bytes, bytes, i64) arguments".into(),
                });
            }
            if arg_typed[..3].iter().any(|t| !is_blob_like(&t.ty)) {
                return Err(SemanticError {
                    message: "verify_signature expects message, signature, and public key as bytes pointers"
                        .into(),
                });
            }
            if !is_int_like(&arg_typed[3].ty) {
                return Err(SemanticError {
                    message: "verify_signature expects scheme code as i64".into(),
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
                    message: format!(
                        "{} expects (bytes, bytes, bytes, bytes[, i64])",
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
                    message: format!(
                        "{} expects key, nonce, aad, {data_label} as bytes pointers",
                        builtin.name()
                    ),
                });
            }
            if arg_typed.len() == 5 && !is_int_like(&arg_typed[4].ty) {
                return Err(SemanticError {
                    message: format!("{} optional tag length must be i64", builtin.name()),
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
                    message: "get_account_balance expects (AccountId, AssetDefinitionId)".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Amount,
            })
        }
        Builtin::GetPublicInput => {
            if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                return Err(SemanticError {
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
                    message: "debug_print expects (i64 value)".into(),
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
                    message: "assert expects (bool) or (bool, string|i64)".into(),
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
                    message: "info expects (string|i64)".into(),
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
                    message: "assert_eq expects two i64 args".into(),
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
                    && arg_typed[2].ty == Type::Amount)
            {
                return Err(SemanticError {
                    message: format!(
                        "{} expects (AccountId, AssetDefinitionId, Amount)",
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
                    && arg_typed[3].ty == Type::Amount
                    && arg_typed[4].ty == Type::DataSpaceId)
            {
                return Err(SemanticError {
                    message:
                        "transfer_asset expects (AccountId, AccountId, AssetDefinitionId, Amount, DataSpaceId)"
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
        Builtin::NftMintAsset => {
            if arg_typed.len() != 2
                || !(arg_typed[0].ty == Type::NftId && arg_typed[1].ty == Type::AccountId)
            {
                return Err(SemanticError {
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
                    message: "register_asset expects (AssetDefinitionId, string, i64, i64)".into(),
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
                    message:
                        "create_new_asset expects (AssetDefinitionId, string, i64, AccountId, i64)"
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
                    message: "set_trigger_enabled expects (Name, i64)".into(),
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
        Builtin::EscrowOpenOffer => {
            if !(arg_typed.len() == 3 || arg_typed.len() == 4)
                || !(arg_typed[0].ty == Type::Name
                    && arg_typed[1].ty == Type::AssetDefinitionId
                    && arg_typed[2].ty == Type::Amount)
                || (arg_typed.len() == 4 && !is_blob_like(&arg_typed[3].ty))
            {
                return Err(SemanticError {
                    message:
                        "escrow_open_offer expects (Name, AssetDefinitionId, Amount[, bytes evidence_hashes])"
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
                    && arg_typed[1].ty == Type::Amount
                    && arg_typed[2].ty == Type::Amount)
                || (arg_typed.len() == 4 && !is_blob_like(&arg_typed[3].ty))
            {
                return Err(SemanticError {
                    message: "escrow_resolve_dispute expects (Name, Amount, Amount[, bytes evidence_hashes])"
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
                    message: "alloc expects (i64 bytes)".into(),
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
                    message: "grow_heap expects (i64 bytes)".into(),
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
                    message:
                        "get_merkle_path expects (i64 address, i64 output_ptr[, i64 root_output_ptr])"
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
                    message: format!(
                        "{} expects (i64 address_or_register, i64 output_ptr[, i64 max_depth[, i64 root_output_ptr]])",
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
                    message: "get_private_input expects (i64 index)".into(),
                });
            }
            if !context.zk_enabled {
                return Err(SemanticError {
                    message: "get_private_input requires ZK mode in compiler build configuration"
                        .into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed,
                },
                ty: Type::Secret(Box::new(Type::Int)),
            })
        }
        Builtin::UseNullifier => {
            if arg_typed.len() != 1 || !is_int_like(&arg_typed[0].ty) {
                return Err(SemanticError {
                    message: "use_nullifier expects (i64 nullifier)".into(),
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
        Builtin::CommitOutput => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
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
                    message: "set_execution_depth expects one i64 arg".into(),
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
            ensure_transfer_batch_args(&arg_typed)?;
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
                    message: "use_asset_handle expects (AssetHandle, bytes intent[, ProofBlob])"
                        .into(),
                });
            }
            if arg_typed[0].ty != Type::AssetHandle || !is_blob_like(&arg_typed[1].ty) {
                return Err(SemanticError {
                    message: "use_asset_handle expects (AssetHandle, bytes intent[, ProofBlob])"
                        .into(),
                });
            }
            if arg_typed.len() == 3 && arg_typed[2].ty != Type::ProofBlob {
                return Err(SemanticError {
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
                || !(arg_typed[0].ty == Type::AccountId && arg_typed[1].ty == Type::Amount)
            {
                return Err(SemanticError {
                    message: "set_account_quorum expects (AccountId, Amount)".into(),
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
                    message: "path expects (Name, i64|bytes)".into(),
                });
            }
            if !(is_int_like(&arg_typed[1].ty) || is_blob_like(&arg_typed[1].ty)) {
                return Err(SemanticError {
                    message: "path expects (Name, i64|bytes)".into(),
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
                    message: "tlv_eq expects (pointer-ABI, pointer-ABI)".into(),
                });
            }
            for arg in &arg_typed {
                let ty = resolve_struct_type(&arg.ty);
                if !(is_pointer_type(&ty) || is_blob_like(&ty) || ty == Type::Json) {
                    return Err(SemanticError {
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
                    message: "tlv_len expects one argument".into(),
                });
            }
            let ty = resolve_struct_type(&arg_typed[0].ty);
            if !(is_pointer_type(&ty) || is_blob_like(&ty) || ty == Type::Json) {
                return Err(SemanticError {
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
        Builtin::PointerToNorito => {
            if arg_typed.len() != 1 {
                return Err(SemanticError {
                    message: "pointer_to_norito expects one argument".into(),
                });
            }
            let ty = resolve_struct_type(&arg_typed[0].ty);
            if !(is_pointer_type(&ty) || is_blob_like(&ty)) {
                return Err(SemanticError {
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
                    message: "json_set_int expects (Json, Name, i64)".into(),
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
                    message: "encode_int expects (i64)".into(),
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
                    message: "json_set_int_direct expects (Json, Name, i64)".into(),
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
        | Builtin::JsonGetNumericDirect
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
                    message: "numeric_to_int expects (Amount|u128)".into(),
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
                    message: "numeric_neg expects (Amount|u128)".into(),
                });
            }
            if matches!(resolve_struct_type(&arg_typed[0].ty), Type::FixedU128) {
                return Err(SemanticError {
                    message: "numeric::neg is not defined for unsigned u128 values".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed.clone(),
                },
                ty: resolve_struct_type(&arg_typed[0].ty),
            })
        }
        Builtin::NumericToIntDirect => {
            if arg_typed.len() != 1 || !is_wide_numeric_type(&arg_typed[0].ty) {
                return Err(SemanticError {
                    message: "numeric_to_int_direct expects (Amount|u128)".into(),
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
                    message: "numeric_neg_direct expects (Amount|u128)".into(),
                });
            }
            if matches!(resolve_struct_type(&arg_typed[0].ty), Type::FixedU128) {
                return Err(SemanticError {
                    message: "numeric negation is not defined for unsigned u128 values".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Call {
                    name: builtin.name().to_string(),
                    args: arg_typed.clone(),
                },
                ty: resolve_struct_type(&arg_typed[0].ty),
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
                    message: format!("{} expects (Amount|u128, Amount|u128)", builtin.name()),
                });
            }
            let Some(result_ty) = numeric_result_type(&arg_typed[0].ty, &arg_typed[1].ty) else {
                return Err(SemanticError {
                    message: format!(
                        "{} expects compatible wide numeric operands",
                        builtin.name()
                    ),
                });
            };
            if !is_wide_numeric_type(&result_ty) {
                return Err(SemanticError {
                    message: format!("{} expects wide numeric operands", builtin.name()),
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
                    message: format!(
                        "{} expects compatible wide numeric operands",
                        builtin.name()
                    ),
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
                    message: "wrapping_neg expects (i64)".into(),
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
                    message: format!("{} expects (i64, i64)", builtin.name()),
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
                    message: format!("{name} expects (i64)"),
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
                    message: format!("{name} expects (i64, i64)"),
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
        Builtin::Poseidon2 | Builtin::Valcom => {
            let name = builtin.name();
            let message = if builtin == Builtin::Poseidon2 {
                "poseidon2 expects two i64 args"
            } else {
                "valcom expects two i64 args"
            };
            if arg_typed.len() != 2
                || !arg_typed
                    .iter()
                    .all(|arg| is_int_like(&arg.ty) || crate::secret::is_secret_int(&arg.ty))
            {
                return Err(SemanticError {
                    message: message.into(),
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
        Builtin::Poseidon6 => {
            if arg_typed.len() != 6
                || !arg_typed
                    .iter()
                    .all(|arg| is_int_like(&arg.ty) || crate::secret::is_secret_int(&arg.ty))
            {
                return Err(SemanticError {
                    message: "poseidon6 expects six i64 args".into(),
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
            if arg_typed.len() != 1
                || !(is_int_like(&arg_typed[0].ty)
                    || crate::secret::is_secret_int(&arg_typed[0].ty))
            {
                return Err(SemanticError {
                    message: "pubkgen expects one i64 arg".into(),
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
                    message: "setvl expects one i64 arg".into(),
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
        | Builtin::GetNumeric
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
                    message: format!("{} expects (Json, Name)", builtin.name()),
                });
            }
            let ty = match builtin {
                Builtin::GetInt => Type::Int,
                Builtin::GetNumeric => Type::Amount,
                Builtin::GetJson => Type::Json,
                Builtin::GetName => Type::Name,
                Builtin::GetAccountId => Type::AccountId,
                Builtin::GetAssetDefinitionId => Type::AssetDefinitionId,
                Builtin::GetNftId => Type::NftId,
                Builtin::GetBlobHex => Type::Bytes,
                _ => unreachable!(),
            };
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
        Builtin::Authority | Builtin::SysvarAuthority => {
            if !arg_typed.is_empty() {
                return Err(SemanticError {
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

fn analyze_expr(
    context: &SemanticContext,
    expr: &Expr,
    vars: &mut HashMap<String, Type>,
) -> Result<TypedExpr, SemanticError> {
    match expr {
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            let c = analyze_expr(context, cond, vars)?;
            crate::secret::reject_secret_control_flow(&c)?;
            if c.ty != Type::Bool {
                return Err(SemanticError {
                    message: "conditional expects a bool condition".into(),
                });
            }
            let t1 = analyze_expr(context, then_expr, vars)?;
            let t2 = analyze_expr(context, else_expr, vars)?;
            if t1.ty != t2.ty {
                return Err(SemanticError {
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
            let mut typed = Vec::new();
            for e in elems {
                typed.push(analyze_expr(context, e, vars)?);
            }
            let tys = typed.iter().map(|t| t.ty.clone()).collect();
            Ok(TypedExpr {
                expr: ExprKind::Tuple(typed),
                ty: Type::Tuple(tys),
            })
        }
        Expr::Number(n) => Ok(TypedExpr {
            expr: ExprKind::Number(*n),
            ty: Type::Int,
        }),
        Expr::Decimal(raw) => {
            raw.parse::<u128>().map_err(|_| SemanticError {
                message: format!("u128 literal `{raw}` is outside 0..={}", u128::MAX),
            })?;
            Ok(TypedExpr {
                expr: ExprKind::Decimal(raw.clone()),
                ty: Type::FixedU128,
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
                    expr: ExprKind::Number(i64::from(code)),
                    ty: Type::Int,
                });
            }
            Err(SemanticError {
                message: format!("undefined variable {name}"),
            })
        }
        Expr::Unary { op, expr: inner } => {
            let inner_t = analyze_expr(context, inner, vars)?;
            crate::secret::reject_secret_ordinary_operation(&[&inner_t])?;
            match op {
                UnaryOp::Neg => {
                    let Some(kind) = numeric_kind(&inner_t.ty) else {
                        return Err(SemanticError {
                            message: "unary '-' expects numeric".into(),
                        });
                    };
                    if kind != NumericKind::Int {
                        return Err(SemanticError {
                            message:
                                "unary '-' is only supported for i64; numeric aliases are unsigned"
                                    .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Unary {
                            op: *op,
                            expr: Box::new(inner_t.clone()),
                        },
                        ty: numeric_kind_to_type(kind),
                    })
                }
                UnaryOp::Not => {
                    if inner_t.ty != Type::Bool {
                        return Err(SemanticError {
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
            let resolved_obj_ty = resolve_struct_type_with_context(context, &obj.ty);
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
                            message: format!(
                                "tuple index on non-tuple type struct {name}; unknown field '{field}' on struct {name}"
                            ),
                        });
                    }
                    other => {
                        return Err(SemanticError {
                            message: format!("tuple index on non-tuple type {}", type_name(other)),
                        });
                    }
                }
            }
            // Named access on a tuple is invalid
            if matches!(&resolved_obj_ty, Type::Tuple(_)) {
                return Err(SemanticError {
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
                    match &e.expr {
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
                message: format!("unknown field '{field}' on type {}", type_name(&obj.ty)),
            })
        }
        Expr::Index { target, index } => {
            let tgt = analyze_expr(context, target, vars)?;
            let mut idx = analyze_expr(context, index, vars)?;
            crate::secret::reject_secret_key(&idx)?;
            match tgt.ty.clone() {
                Type::StateMap(k, _) => {
                    ensure_assignable_and_coerce(&k, &mut idx)?;
                    ensure_in_memory_map_word_types(context, &tgt)?;
                    Err(SemanticError {
                        message: "E_STATE_MAP_OPTIONAL_READ: StateMap rvalue indexing cannot represent an absent key; use `map.get(key)` and handle Option<V>"
                            .into(),
                    })
                }
                _ => Err(SemanticError {
                    message: "indexing not supported on this type".into(),
                }),
            }
        }
        Expr::Binary { op, left, right } => {
            let left_t = analyze_expr(context, left, vars)?;
            let right_t = analyze_expr(context, right, vars)?;
            crate::secret::reject_secret_ordinary_operation(&[&left_t, &right_t])?;
            use BinaryOp::*;
            match op {
                Add | Sub | Mul | Div | Mod => {
                    let Some(result_ty) = numeric_result_type(&left_t.ty, &right_t.ty) else {
                        return Err(SemanticError {
                            message: format!(
                                "{op:?} requires identical numeric operand types; implicit conversions are not part of Kotodama V1"
                            ),
                        });
                    };
                    require_same_numeric_type(&left_t, &result_ty)?;
                    require_same_numeric_type(&right_t, &result_ty)?;
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: result_ty,
                    })
                }
                And | Or => {
                    if left_t.ty != Type::Bool || right_t.ty != Type::Bool {
                        return Err(SemanticError {
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
                    let numeric_result = numeric_result_type(&left_t.ty, &right_t.ty);
                    let numeric_ok = numeric_result.is_some();
                    if left_t.ty != right_t.ty
                        && !(is_blob_like(&left_t.ty) && is_blob_like(&right_t.ty))
                        && !numeric_ok
                    {
                        return Err(SemanticError {
                            message: "type mismatch in equality".into(),
                        });
                    }
                    if !is_eq_comparable_type(&left_t.ty) {
                        return Err(SemanticError {
                            message: format!(
                                "equality is not supported for type {}",
                                type_name(&left_t.ty)
                            ),
                        });
                    }
                    if let Some(result_ty) = numeric_result {
                        require_same_numeric_type(&left_t, &result_ty)?;
                        require_same_numeric_type(&right_t, &result_ty)?;
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
                    let Some(result_ty) = numeric_result_type(&left_t.ty, &right_t.ty) else {
                        return Err(SemanticError {
                            message: format!(
                                "{op:?} requires identical numeric operand types; implicit conversions are not part of Kotodama V1"
                            ),
                        });
                    };
                    require_same_numeric_type(&left_t, &result_ty)?;
                    require_same_numeric_type(&right_t, &result_ty)?;
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
        Expr::Call { name, args } => {
            let source_name = name.clone();
            let name = normalize_namespaced(name);
            if let Some(builtin) = Builtin::from_name(&name)
                && matches!(
                    builtin,
                    Builtin::TestInvokeEntrypoint
                        | Builtin::TestInvokeEntrypointAs
                        | Builtin::TestExpectRejectAs
                        | Builtin::TestActorAccount
                        | Builtin::TestActorPublicKey
                        | Builtin::TestActorSign
                )
                && source_name != builtin.source_name()
            {
                return Err(SemanticError {
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
                    message: format!(
                        "E_TEST_ONLY_PRODUCTION: builtin `{source_name}` requires explicit compiler test mode"
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
                );
            }
            if name == "invoke_entrypoint_as" {
                return canonicalize_builtin_result(
                    Builtin::TestInvokeEntrypointAs,
                    analyze_invoke_entrypoint_as_call(context, args, vars),
                );
            }
            if name == "expect_reject_as" {
                return canonicalize_builtin_result(
                    Builtin::TestExpectRejectAs,
                    analyze_expect_reject_as_call(context, args, vars),
                );
            }
            if name == "actor_account" {
                return canonicalize_builtin_result(
                    Builtin::TestActorAccount,
                    analyze_actor_account_call(context, args),
                );
            }
            if name == "actor_public_key" {
                return canonicalize_builtin_result(
                    Builtin::TestActorPublicKey,
                    analyze_actor_public_key_call(context, args),
                );
            }
            if name == "actor_sign" {
                return canonicalize_builtin_result(
                    Builtin::TestActorSign,
                    analyze_actor_sign_call(context, args, vars),
                );
            }

            // Struct constructor call: `StructName(arg1, arg2, ...)`
            if let Some(fields) = context.structs.borrow().get(&name).cloned() {
                let mut arg_typed = Vec::new();
                for a in args {
                    arg_typed.push(analyze_expr(context, a, vars)?);
                }
                if arg_typed.len() != fields.len() {
                    return Err(SemanticError {
                        message: format!(
                            "{} expects {} fields, got {}",
                            name,
                            fields.len(),
                            arg_typed.len()
                        ),
                    });
                }
                for (i, (_fname, fty)) in fields.iter().enumerate() {
                    ensure_assignable_and_coerce(fty, &mut arg_typed[i])?;
                }
                return Ok(TypedExpr {
                    expr: ExprKind::Tuple(arg_typed),
                    ty: Type::Struct {
                        name: name.clone(),
                        fields,
                    },
                });
            }

            // analyze builtin calls
            let mut arg_typed = Vec::new();
            for a in args {
                arg_typed.push(analyze_expr(context, a, vars)?);
            }
            if let Some(result) = explicit_numeric_conversion(&name, arg_typed.clone()) {
                return result;
            }
            if let Some(result) = analyze_sum_type_call(context, &name, arg_typed.clone()) {
                return result;
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
                        message: format!(
                            "legacy or non-canonical builtin spelling `{source_name}` is not supported; use `{}`",
                            builtin.source_name()
                        ),
                    });
                }
                return canonicalize_builtin_result(
                    builtin,
                    analyze_surface_builtin_call(context, builtin, arg_typed),
                );
            }
            match name.as_str() {
                "Map::new" => {
                    Err(SemanticError {
                        message: "ephemeral maps are not part of Kotodama V1; declare durable `StateMap<K, V>` state instead".into(),
                    })
                }
                _ => {
                    let Some(signature) =
                        context.function_params.borrow().get(&name).cloned()
                    else {
                        return Err(SemanticError {
                            message: format!("unknown function or builtin `{source_name}`"),
                        });
                    };
                    if signature.len() != arg_typed.len() {
                        return Err(SemanticError {
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
                                    message: format!(
                                        "state parameter `{}` requires a durable state handle argument",
                                        param.name
                                    ),
                                });
                            }
                        } else if is_state_map_expr(context, arg) {
                            return Err(SemanticError {
                                message:
                                    "E_STATE_MAP_ALIAS: state maps cannot be passed to user-defined functions; access declared state directly."
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
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: ret_ty,
                    })
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
        "option::some" => {
            if args.len() != 1 {
                return Some(error("option::some expects one value"));
            }
            let payload = resolve_struct_type(&args[0].ty);
            if !is_supported_sum_payload(&payload) {
                return Some(error("Option<T> V1 payloads must be durable-value types"));
            }
            let ty = Type::Option(Box::new(payload));
            call("option_some", args, ty)
        }
        "option::none" => {
            if args.len() != 1 {
                return Some(error("option::none expects one typed fallback placeholder"));
            }
            let payload = resolve_struct_type(&args[0].ty);
            if !is_supported_sum_payload(&payload) {
                return Some(error("Option<T> V1 payloads must be durable-value types"));
            }
            let ty = Type::Option(Box::new(payload));
            call("option_none", args, ty)
        }
        "result::ok" => {
            if args.len() != 2 {
                return Some(error("result::ok expects (value, error_placeholder)"));
            }
            let ok = resolve_struct_type(&args[0].ty);
            let err = resolve_struct_type(&args[1].ty);
            if !is_supported_sum_payload(&ok) || !is_supported_sum_payload(&err) {
                return Some(error(
                    "Result<T, E> V1 payloads must be durable-value types",
                ));
            }
            let ty = Type::Result(Box::new(ok), Box::new(err));
            call("result_ok", args, ty)
        }
        "result::err" => {
            if args.len() != 2 {
                return Some(error("result::err expects (value_placeholder, error)"));
            }
            let ok = resolve_struct_type(&args[0].ty);
            let err = resolve_struct_type(&args[1].ty);
            if !is_supported_sum_payload(&ok) || !is_supported_sum_payload(&err) {
                return Some(error(
                    "Result<T, E> V1 payloads must be durable-value types",
                ));
            }
            let ty = Type::Result(Box::new(ok), Box::new(err));
            call("result_err", args, ty)
        }
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
    convert_type_expr(context, t).map(|ty| Some(resolve_struct_type_with_context(context, &ty)))
}

fn analyze_const_expr(
    expr: &Expr,
    consts: &IndexMap<String, TypedExpr>,
) -> Result<TypedExpr, SemanticError> {
    match expr {
        Expr::Number(n) => Ok(TypedExpr {
            expr: ExprKind::Number(*n),
            ty: Type::Int,
        }),
        Expr::Decimal(raw) => {
            raw.parse::<u128>().map_err(|_| SemanticError {
                message: format!("u128 literal `{raw}` is outside 0..={}", u128::MAX),
            })?;
            Ok(TypedExpr {
                expr: ExprKind::Decimal(raw.clone()),
                ty: Type::FixedU128,
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
        Expr::Ident(name) => consts.get(name).cloned().ok_or_else(|| SemanticError {
            message: format!(
                "const `{name}` is undefined or declared after use; constants must be declared before use"
            ),
        }),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => {
            let inner = analyze_const_expr(inner, consts)?;
            match inner.expr {
                ExprKind::Number(value) => value
                    .checked_neg()
                    .map(|value| TypedExpr {
                        expr: ExprKind::Number(value),
                        ty: Type::Int,
                    })
                    .ok_or_else(|| SemanticError {
                        message: format!(
                            "E_INT_OVERFLOW: negating {value} is outside the i64 range"
                        ),
                    }),
                _ => Err(SemanticError {
                    message: "const unary '-' expects an integer literal or integer const".into(),
                }),
            }
        }
        _ => Err(SemanticError {
            message:
                "const initializers must be literal values or previously declared constants".into(),
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
            message: format!("parameter `{}` requires an explicit type", param.name),
        })?,
    )?;
    let ty = resolve_struct_type_with_context(context, &ty);
    if modifiers.visibility == FunctionVisibility::Public && !is_supported_public_argument_type(&ty)
    {
        return Err(SemanticError {
            message: format!(
                "public parameter `{}` uses unsupported V1 boundary type `{}`",
                param.name,
                type_name(&ty)
            ),
        });
    }
    if param.is_state {
        if modifiers.visibility != FunctionVisibility::Internal
            || matches!(
                modifiers.kind,
                FunctionKind::View | FunctionKind::Init | FunctionKind::Upgrade
            )
        {
            return Err(SemanticError {
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
    Ok(match ty {
        TypeExpr::Path(s) => match s.as_str() {
            "i64" => Type::Int,
            "u128" => Type::FixedU128,
            "Amount" => Type::Amount,
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
            other => {
                let is_declared_struct = context.structs.borrow().contains_key(other);
                if !is_declared_struct {
                    return Err(SemanticError {
                        message: format!("unknown type `{other}`"),
                    });
                }
                Type::NamedStruct(other.to_string())
            }
        },
        TypeExpr::Generic { base, args } => {
            if base == "StateMap" {
                if args.len() != 2 {
                    return Err(SemanticError {
                        message: "StateMap expects two type parameters".into(),
                    });
                }
                let k = convert_type_expr(context, &args[0])?;
                let v = convert_type_expr(context, &args[1])?;
                Type::StateMap(Box::new(k), Box::new(v))
            } else if base == "Secret" {
                if args.len() != 1 {
                    return Err(SemanticError {
                        message: "Secret expects one type parameter".into(),
                    });
                }
                let inner = convert_type_expr(context, &args[0])?;
                if inner != Type::Int {
                    return Err(SemanticError {
                        message: format!(
                            "E_SECRET_PAYLOAD_TYPE: Secret<{}> is unsupported; the V1 private-input ABI supplies Secret<i64>",
                            type_name(&inner)
                        ),
                    });
                }
                Type::Secret(Box::new(inner))
            } else if base == "Option" {
                if args.len() != 1 {
                    return Err(SemanticError {
                        message: "Option expects one type parameter".into(),
                    });
                }
                Type::Option(Box::new(convert_type_expr(context, &args[0])?))
            } else if base == "Result" {
                if args.len() != 2 {
                    return Err(SemanticError {
                        message: "Result expects two type parameters".into(),
                    });
                }
                Type::Result(
                    Box::new(convert_type_expr(context, &args[0])?),
                    Box::new(convert_type_expr(context, &args[1])?),
                )
            } else {
                return Err(SemanticError {
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
    })
}

fn apply_map_new_type_hint(expr: &mut TypedExpr, hint: &Type) {
    let hint = resolve_struct_type(hint);
    if !matches!(hint, Type::StateMap(_, _)) {
        return;
    }
    if let ExprKind::Call { name, .. } = &expr.expr
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
        (Type::Tuple(exp_elems), Type::Tuple(act_elems)) => {
            if exp_elems.len() != act_elems.len() {
                return Err(SemanticError {
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
    ensure_assignable(expected, &expr.ty)?;
    Ok(())
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
        Type::Secret(inner) => Type::Secret(Box::new(resolve_struct_type(inner))),
        Type::Tuple(items) => Type::Tuple(items.iter().map(resolve_struct_type).collect()),
        Type::Struct { name, fields } => Type::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(field_name, field_ty)| (field_name.clone(), resolve_struct_type(field_ty)))
                .collect(),
        },
        _ => ty.clone(),
    }
}

fn resolve_struct_type_with_context(context: &SemanticContext, ty: &Type) -> Type {
    match ty {
        Type::NamedStruct(name) => context
            .structs
            .borrow()
            .get(name)
            .map(|fields| Type::Struct {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|(field_name, field_ty)| {
                        (
                            field_name.clone(),
                            resolve_struct_type_with_context(context, field_ty),
                        )
                    })
                    .collect(),
            })
            .unwrap_or_else(|| ty.clone()),
        Type::StateMap(key, value) => Type::StateMap(
            Box::new(resolve_struct_type_with_context(context, key)),
            Box::new(resolve_struct_type_with_context(context, value)),
        ),
        Type::Option(inner) => {
            Type::Option(Box::new(resolve_struct_type_with_context(context, inner)))
        }
        Type::Result(ok, err) => Type::Result(
            Box::new(resolve_struct_type_with_context(context, ok)),
            Box::new(resolve_struct_type_with_context(context, err)),
        ),
        Type::Secret(inner) => {
            Type::Secret(Box::new(resolve_struct_type_with_context(context, inner)))
        }
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| resolve_struct_type_with_context(context, item))
                .collect(),
        ),
        Type::Struct { name, fields } => Type::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(field_name, field_ty)| {
                    (
                        field_name.clone(),
                        resolve_struct_type_with_context(context, field_ty),
                    )
                })
                .collect(),
        },
        _ => ty.clone(),
    }
}

fn normalize_namespaced(name: &str) -> String {
    if let Some(builtin) = Builtin::from_source_name(name) {
        return builtin.name().to_owned();
    }
    String::from(name)
}

// Return coverage and shape analysis on the AST (conservative)
fn block_returns_all_paths(block: &super::ast::Block) -> bool {
    let mut always = false;
    for stmt in &block.statements {
        if stmt_returns_all_paths(stmt) {
            always = true;
            break;
        }
    }
    always
}

fn stmt_returns_all_paths(stmt: &super::ast::Statement) -> bool {
    use super::ast::Statement as S;
    match stmt {
        S::Return(_) => true,
        S::If {
            then_branch,
            else_branch,
            ..
        } => else_branch
            .as_ref()
            .map(|else_b| block_returns_all_paths(then_branch) && block_returns_all_paths(else_b))
            .unwrap_or(false),
        S::While { .. } | S::For { .. } => false,
        _ => false,
    }
}

fn block_has_return_value(block: &super::ast::Block) -> bool {
    block.statements.iter().any(stmt_has_return_value)
}

fn stmt_has_return_value(stmt: &super::ast::Statement) -> bool {
    use super::ast::Statement as S;
    match stmt {
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

// NOTE: `TypedProgram` is defined earlier in this file with contract metadata.

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub struct TypedFunction {
    pub name: String,
    pub params: Vec<String>,
    pub param_types: Vec<TypedParam>,
    pub body: TypedBlock,
    pub ret_ty: Option<Type>,
    pub modifiers: FunctionModifiers,
    pub location: super::ast::SourceLocation,
}

#[derive(Debug, PartialEq)]
pub struct TypedBlock {
    pub statements: Vec<TypedStatement>,
}

#[derive(Debug, PartialEq)]
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
        start: usize,
        /// Optional upper bound on iterations (e.g., from `.take(n)`).
        bound: Option<usize>,
    },
    /// Map set operation: `map[key] = value`.
    MapSet {
        map: TypedExpr,
        key: TypedExpr,
        value: TypedExpr,
    },
}

fn expr_mutates_map(expr: &TypedExpr, map_name: &str) -> bool {
    match &expr.expr {
        ExprKind::Call { name, args } => {
            (matches!(
                Builtin::from_name(name),
                Some(Builtin::Ensure | Builtin::StateMapRemove)
            ) && args
                .first()
                .is_some_and(|map| matches!(&map.expr, ExprKind::Ident(name) if name == map_name)))
                || args.iter().any(|arg| expr_mutates_map(arg, map_name))
        }
        ExprKind::Binary { left, right, .. } => {
            expr_mutates_map(left, map_name) || expr_mutates_map(right, map_name)
        }
        ExprKind::Unary { expr, .. } | ExprKind::NumericCast { expr } => {
            expr_mutates_map(expr, map_name)
        }
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_mutates_map(cond, map_name)
                || expr_mutates_map(then_expr, map_name)
                || expr_mutates_map(else_expr, map_name)
        }
        ExprKind::Tuple(items) => items.iter().any(|item| expr_mutates_map(item, map_name)),
        ExprKind::Member { object, .. } => expr_mutates_map(object, map_name),
        ExprKind::Index { target, index } => {
            expr_mutates_map(target, map_name) || expr_mutates_map(index, map_name)
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => false,
    }
}

fn block_mutates_map(block: &TypedBlock, map_name: &str) -> bool {
    fn stmt_mutates(stmt: &TypedStatement, map_name: &str) -> bool {
        match stmt {
            TypedStatement::MapSet { map, .. } => {
                matches!(&map.expr, ExprKind::Ident(n) if n == map_name)
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
            TypedStatement::While { body, .. } => block_mutates_map(body, map_name),
            TypedStatement::For { body, .. } => block_mutates_map(body, map_name),
            TypedStatement::ForEachMap { body, .. } => block_mutates_map(body, map_name),
            _ => false,
        }
    }
    block.statements.iter().any(|s| stmt_mutates(s, map_name))
}

fn block_contains_host_side_effects(block: &TypedBlock) -> bool {
    block
        .statements
        .iter()
        .any(statement_contains_host_side_effects)
}

fn block_contains_instruction_emission(block: &TypedBlock) -> bool {
    block
        .statements
        .iter()
        .any(statement_contains_instruction_emission)
}

fn block_mutates_durable_state(context: &SemanticContext, block: &TypedBlock) -> bool {
    block
        .statements
        .iter()
        .any(|statement| statement_mutates_durable_state(context, statement))
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
}

fn collect_state_accesses_statement(
    state_names: &HashSet<String>,
    stmt: &TypedStatement,
    reads: &mut IndexSet<String>,
    writes: &mut IndexSet<String>,
) {
    match stmt {
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
            if let ExprKind::Ident(name) = &map.expr {
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
    match &expr.expr {
        ExprKind::Ident(name) => mark_state_read(state_names, name, reads),
        ExprKind::Binary { left, right, .. } => {
            collect_state_accesses_expr(state_names, left, reads, writes);
            collect_state_accesses_expr(state_names, right, reads, writes);
        }
        ExprKind::Unary { expr: inner, .. } | ExprKind::NumericCast { expr: inner } => {
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
        ExprKind::Tuple(items) => {
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
        ExprKind::Call { name, args } => {
            if matches!(
                Builtin::from_name(name),
                Some(Builtin::Ensure | Builtin::StateMapRemove)
            ) && let Some(TypedExpr {
                expr: ExprKind::Ident(map_name),
                ..
            }) = args.first()
            {
                mark_state_write(state_names, map_name, writes);
            }
            for arg in args {
                collect_state_accesses_expr(state_names, arg, reads, writes);
            }
        }
        ExprKind::Bool(_)
        | ExprKind::Number(_)
        | ExprKind::Decimal(_)
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
}

fn collect_calls_in_statement(
    context: &SemanticContext,
    stmt: &TypedStatement,
    calls: &mut IndexSet<String>,
) {
    match stmt {
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
    match &expr.expr {
        ExprKind::Call { name, args } => {
            if is_user_defined_function(context, name) {
                calls.insert(name.clone());
            }
            for arg in args {
                collect_calls_in_expr(context, arg, calls);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_calls_in_expr(context, left, calls);
            collect_calls_in_expr(context, right, calls);
        }
        ExprKind::Unary { expr: inner, .. } => collect_calls_in_expr(context, inner, calls),
        ExprKind::NumericCast { expr } => collect_calls_in_expr(context, expr, calls),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_calls_in_expr(context, cond, calls);
            collect_calls_in_expr(context, then_expr, calls);
            collect_calls_in_expr(context, else_expr, calls);
        }
        ExprKind::Tuple(items) => {
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
        | ExprKind::Number(_)
        | ExprKind::Decimal(_)
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
            message: format!("E_STATE_SHADOWED: `{name}` shadows a state declaration"),
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
            message: format!("local binding `{name}` duplicates or shadows an existing binding"),
        });
    }
    if context.consts.borrow().contains_key(name) {
        return Err(SemanticError {
            message: format!("local binding `{name}` shadows a const declaration"),
        });
    }
    if context.global_declarations.borrow().contains(name) {
        return Err(SemanticError {
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
    match &expr.expr {
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

    let initializer = functions
        .iter()
        .find(|function| function.modifiers.kind == FunctionKind::Init)
        .ok_or_else(|| SemanticError {
            message:
                "E_STATE_INIT_REQUIRED: contract scalar state requires an `hajimari()` initializer"
                    .into(),
        })?;

    let required_set = required.iter().cloned().collect::<HashSet<_>>();
    let summaries = compute_definite_state_write_summaries(&functions, &required_set)?;
    let initialized = summaries
        .get(&initializer.name)
        .cloned()
        .unwrap_or_default();
    let missing = required
        .iter()
        .filter(|state| !initialized.contains(*state))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(SemanticError {
            message: format!(
                "E_STATE_INIT_INCOMPLETE: hajimari() must initialize every scalar state on every normal return or fallthrough path; missing: {}",
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
    match &expr.expr {
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
        ExprKind::Tuple(items) => {
            for item in items {
                initialized = evaluate_definite_init_expr(item, initialized, summaries);
            }
            initialized
        }
        ExprKind::Unary { expr, .. } | ExprKind::NumericCast { expr } => {
            evaluate_definite_init_expr(expr, initialized, summaries)
        }
        ExprKind::Member { object, .. } => {
            evaluate_definite_init_expr(object, initialized, summaries)
        }
        ExprKind::Index { target, index } => {
            initialized = evaluate_definite_init_expr(target, initialized, summaries);
            evaluate_definite_init_expr(index, initialized, summaries)
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
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

    flow
}

fn analyze_definite_init_statement(
    statement: &TypedStatement,
    incoming: DefiniteStateSet,
    required: &DefiniteStateSet,
    summaries: &HashMap<String, DefiniteStateSet>,
) -> DefiniteInitFlow {
    match statement {
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
        message: "E_STATE_INIT_INCOMPLETE: compiler could not prove scalar-state initialization through the complete helper call graph"
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
            return Err(SemanticError { message });
        }

        let needs_permission = effects
            .get(&func.name)
            .copied()
            .unwrap_or_default()
            .requires_permission();
        let is_transaction_entry = func.modifiers.visibility == FunctionVisibility::Public
            && func.modifiers.kind == FunctionKind::Contract;
        let lifecycle_entry = matches!(
            func.modifiers.kind,
            FunctionKind::Init | FunctionKind::Upgrade
        );
        if (is_transaction_entry || (needs_permission && !lifecycle_entry))
            && func.modifiers.visibility == FunctionVisibility::Public
            && func.modifiers.permission.is_none()
        {
            return Err(SemanticError {
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
    match stmt {
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
    match stmt {
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
    match stmt {
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
    match &expr.expr {
        ExprKind::Call { name, args } => {
            Builtin::from_name(name).is_some_and(|builtin| builtin.spec().effects.host_side_effects)
                || args.iter().any(expr_contains_host_side_effects)
        }
        ExprKind::Binary { left, right, .. } => {
            expr_contains_host_side_effects(left) || expr_contains_host_side_effects(right)
        }
        ExprKind::Unary { expr, .. } => expr_contains_host_side_effects(expr),
        ExprKind::NumericCast { expr } => expr_contains_host_side_effects(expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_contains_host_side_effects(cond)
                || expr_contains_host_side_effects(then_expr)
                || expr_contains_host_side_effects(else_expr)
        }
        ExprKind::Tuple(items) => items.iter().any(expr_contains_host_side_effects),
        ExprKind::Member { object, .. } => expr_contains_host_side_effects(object),
        ExprKind::Index { target, index } => {
            expr_contains_host_side_effects(target) || expr_contains_host_side_effects(index)
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => false,
    }
}

fn expr_contains_instruction_emission(expr: &TypedExpr) -> bool {
    match &expr.expr {
        ExprKind::Call { name, args } => {
            Builtin::from_name(name)
                .is_some_and(|builtin| builtin.spec().effects.emits_instructions)
                || args.iter().any(expr_contains_instruction_emission)
        }
        ExprKind::Binary { left, right, .. } => {
            expr_contains_instruction_emission(left) || expr_contains_instruction_emission(right)
        }
        ExprKind::Unary { expr, .. } => expr_contains_instruction_emission(expr),
        ExprKind::NumericCast { expr } => expr_contains_instruction_emission(expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_contains_instruction_emission(cond)
                || expr_contains_instruction_emission(then_expr)
                || expr_contains_instruction_emission(else_expr)
        }
        ExprKind::Tuple(items) => items.iter().any(expr_contains_instruction_emission),
        ExprKind::Member { object, .. } => expr_contains_instruction_emission(object),
        ExprKind::Index { target, index } => {
            expr_contains_instruction_emission(target) || expr_contains_instruction_emission(index)
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => false,
    }
}

fn expr_mutates_durable_state(context: &SemanticContext, expr: &TypedExpr) -> bool {
    match &expr.expr {
        ExprKind::Call { name, args } => {
            Builtin::from_name(name)
                .is_some_and(|builtin| builtin.spec().effects.mutates_durable_state)
                || (matches!(Builtin::from_name(name), Some(Builtin::Ensure))
                    && args
                        .first()
                        .is_some_and(|arg| typed_map_expr_is_state(context, arg)))
                || args
                    .iter()
                    .any(|arg| expr_mutates_durable_state(context, arg))
        }
        ExprKind::Binary { left, right, .. } => {
            expr_mutates_durable_state(context, left) || expr_mutates_durable_state(context, right)
        }
        ExprKind::Unary { expr, .. } => expr_mutates_durable_state(context, expr),
        ExprKind::NumericCast { expr } => expr_mutates_durable_state(context, expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_mutates_durable_state(context, cond)
                || expr_mutates_durable_state(context, then_expr)
                || expr_mutates_durable_state(context, else_expr)
        }
        ExprKind::Tuple(items) => items
            .iter()
            .any(|item| expr_mutates_durable_state(context, item)),
        ExprKind::Member { object, .. } => expr_mutates_durable_state(context, object),
        ExprKind::Index { target, index } => {
            expr_mutates_durable_state(context, target)
                || expr_mutates_durable_state(context, index)
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
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

    fn sample_account_literal() -> String {
        iroha_data_model::account::AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
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

    #[test]
    fn duplicate_top_level_declarations_are_rejected() {
        let cases = [
            (
                "fn repeated() {} fn repeated() {}",
                "duplicate function `repeated`",
            ),
            (
                "struct Repeated { value: i64; } struct Repeated { value: i64; }",
                "duplicate type `Repeated`",
            ),
            (
                "state repeated: i64; state repeated: i64;",
                "duplicate state `repeated`",
            ),
            (
                "const repeated: i64 = 1; const repeated: i64 = 2;",
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
        let err = analyze_error("struct Shared { value: i64; } fn Shared() {}");
        assert_eq!(
            err.message,
            "E_DUPLICATE_DECLARATION: declaration name `Shared` is already used by a type"
        );
    }

    #[test]
    fn compiler_owned_declaration_names_are_rejected() {
        for (source, expected) in [
            (
                "fn account_id(value: string) -> i64 { return 1; }",
                "E_RESERVED_DECLARATION: function `account_id` uses a compiler-reserved name",
            ),
            (
                "fn __kotodama_link_private() {}",
                "E_RESERVED_DECLARATION: function `__kotodama_link_private` uses a compiler-reserved name",
            ),
            (
                "struct Option { value: i64; }",
                "E_RESERVED_DECLARATION: type `Option` uses a compiler-reserved name",
            ),
        ] {
            assert_eq!(analyze_error(source).message, expected);
        }
    }

    #[test]
    fn duplicate_function_parameters_are_rejected() {
        let err = analyze_error("fn repeated(value: i64, value: bool) {}");
        assert_eq!(
            err.message,
            "duplicate parameter `value` in function `repeated`"
        );
    }

    #[test]
    fn duplicate_struct_fields_are_rejected() {
        let err = analyze_error("struct Repeated { value: i64; value: bool; }");
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
             fn pay(allowed: bool) { require(allowed, Payment::Unauthorized); }",
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
        let mut program = parse("fn f(value: i64) {}").expect("parse typed parameter");
        let Item::Function(function) = &mut program.items[0] else {
            panic!("expected function")
        };
        function.params[0].ty = None;
        let err = analyze(&program).expect_err("typeless parameter AST must be rejected");
        assert_eq!(err.message, "parameter `value` requires an explicit type");
    }

    #[test]
    fn semantic_analysis_rejects_ast_consts_without_types() {
        let mut program = parse("const VALUE: i64 = 1;").expect("parse typed const");
        let Item::Const(declaration) = &mut program.items[0] else {
            panic!("expected const")
        };
        declaration.ty = None;
        let err = analyze(&program).expect_err("typeless const AST must be rejected");
        assert_eq!(err.message, "const `VALUE` requires an explicit type");
    }

    #[test]
    fn unknown_path_and_generic_types_are_rejected() {
        let path_err = analyze_error("fn use_missing(value: Missing) {}");
        assert_eq!(path_err.message, "unknown type `Missing`");

        let generic_err = analyze_error("fn generic(value: Missing<i64>) {}");
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
            let error = analyze_error(&format!("fn f(value: {name}) {{}}"));
            assert_eq!(error.message, format!("unknown type `{name}`"));
        }
    }

    #[test]
    fn option_and_result_type_expressions_are_recognized() {
        let context = SemanticContext::new();
        let option = TypeExpr::Generic {
            base: "Option".into(),
            args: vec![TypeExpr::Path("i64".into())],
        };
        assert_eq!(
            convert_type_expr(&context, &option).expect("Option type"),
            Type::Option(Box::new(Type::Int))
        );

        let result = TypeExpr::Generic {
            base: "Result".into(),
            args: vec![TypeExpr::Path("i64".into()), TypeExpr::Path("bool".into())],
        };
        assert_eq!(
            convert_type_expr(&context, &result).expect("Result type"),
            Type::Result(Box::new(Type::Int), Box::new(Type::Bool))
        );

        let helpers = parse(
            "fn option_helper(value: Option<i64>) {} \
             fn result_helper(value: Result<i64, bool>) {}",
        )
        .expect("private helper types parse");
        analyze(&helpers).expect("private helpers accept Option/Result parameters");

        let public = parse(
            "seiyaku Demo { kotoage fn call(value: Option<i64>, outcome: Result<i64, bool>) authorize(\"Call\") {} }",
        )
        .expect("public sum parameters parse");
        analyze(&public).expect("one-shot V1 argument records support Option and Result");

        let unsupported = analyze_error(
            "seiyaku Demo { kotoage fn call(value: StateMap<i64, i64>) authorize(\"Call\") {} }",
        );
        assert!(
            unsupported
                .message
                .contains("unsupported V1 boundary type `StateMap<i64, i64>`"),
            "unexpected error: {}",
            unsupported.message
        );
    }

    #[test]
    fn forward_declared_struct_types_are_accepted() {
        let program = parse(
            "struct First { second: Second; } \
             struct Second { value: i64; } \
             fn read(first: First) -> i64 { return first.second.value; }",
        )
        .expect("source should parse");
        analyze(&program).expect("forward-declared struct references should resolve");
    }

    #[test]
    fn reusable_context_clears_all_declaration_registries() {
        let context = SemanticContext::new();
        let declared = parse(
            "struct SessionOnly { value: i64; } \
             fn read(value: SessionOnly) -> i64 { return value.value; }",
        )
        .expect("declared source");
        context.analyze(&declared).expect("first analysis");

        let undeclared = parse("fn read(value: SessionOnly) -> i64 { return value.value; }")
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
        let direct = analyze_error("struct Node { next: Node; } state root: Node;");
        assert_eq!(
            direct.message,
            "cyclic value struct definition: Node -> Node"
        );

        let indirect = analyze_error(
            "struct Left { right: Right; } \
             struct Right { left: Left; } \
             state root: Left;",
        );
        assert_eq!(
            indirect.message,
            "cyclic value struct definition: Left -> Right -> Left"
        );
    }

    #[test]
    fn get_private_input_requires_build_configured_zk_mode() {
        let err = analyze_error("fn read() -> i64 { return crypto::private_input(0); }");
        assert_eq!(
            err.message,
            "builtin `crypto::private_input` requires ZK mode in compiler build configuration"
        );

        let source = r#"
            seiyaku ZkContract {
                fn read() -> Secret<i64> { return crypto::private_input(0); }
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
        let ok2 = analyze(&parse("fn g() -> i64 { return 1; } ").unwrap());
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
        let err = analyze(&parse("fn f() -> i64 { if true { return 1; } } ").unwrap());
        assert!(err.is_err());
        let ok =
            analyze(&parse("fn g() -> i64 { if true { return 1; } else { return 2; } } ").unwrap());
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
        let bool_arithmetic = analyze(&parse("fn f(x: bool) { let y = x + 1; } ").unwrap());
        assert!(bool_arithmetic.is_err());
        // string param cannot be used in arithmetic
        let err2 = analyze(&parse("fn g(s: string) { let y = s + 1; } ").unwrap());
        assert!(err2.is_err());
        // Canonical parameters always declare their type.
        let ok = analyze(&parse("fn h(x: i64, y: i64) -> i64 { return x + y; } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn typed_id_parameters_reject_arithmetic() {
        // Typed ledger identifiers are not numeric.
        let err = analyze(&parse("fn f(who: AccountId) { let y = who + 1; } ").unwrap());
        assert!(err.is_err());
        // Equality on same named struct references is allowed
        let ok = analyze(
            &parse("fn g(a: AccountId, b: AccountId) -> bool { return a == b; } ").unwrap(),
        );
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
    fn state_map_iteration_accepts_pointer_keys() {
        let program = parse(
            "state Items: StateMap<Name, i64>; \
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
                "state M: StateMap<i64, i64>; \
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
                "state M: StateMap<i64, i64>; \
                 fn main() {{ for (key, value) in {iteration} {{ let _value = value; }} }}"
            ))
            .expect("over-limit iteration source parses");
            let error = analyze(&program).expect_err("bound above 64 must fail semantically");
            assert_eq!(
                error.message,
                format!(
                    "E_ITERATION_LIMIT: `{expected_form}` span 65 exceeds the Kotodama V1 limit 64"
                )
            );
        }
    }

    #[test]
    fn dynamic_map_take_rejects_non_literal_bounds() {
        let program = parse(
            "state M: StateMap<i64, i64>; \
             fn main(n: i64) { \
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
                .contains("requires a non-negative i64 literal")
        );
    }

    #[test]
    fn dynamic_map_range_rejects_non_literal_bounds() {
        let program = parse(
            "state M: StateMap<i64, i64>; \
             fn main(start: i64, end: i64) { \
                 for (k, v) in M.range(start, end) { \
                     let _x = v; \
                 } \
             }",
        )
        .expect("parse dynamic range");
        let error = analyze(&program).expect_err("dynamic range must fail closed in V1");
        assert!(error.message.contains("requires non-negative i64 literals"));
    }

    #[test]
    fn state_map_alias_is_rejected() {
        let program = parse(
            "state M: StateMap<i64, i64>; \
             fn main() { \
                 let m = M; \
             }",
        )
        .expect("parse state map alias");
        let err = analyze(&program).expect_err("aliasing a state map should error");
        assert!(err.message.contains("E_STATE_MAP_ALIAS"));
    }

    #[test]
    fn state_map_reassignment_is_rejected() {
        let program = parse(
            "state M: StateMap<i64, i64>; \
             fn main() { \
                 M = StateMap::new(); \
             }",
        )
        .expect("parse state map reassignment");
        let err = analyze(&program).expect_err("reassigning a state map should error");
        assert!(err.message.contains("E_STATE_MAP_ALIAS"));
    }

    #[test]
    fn state_map_cannot_be_passed_to_user_fn() {
        let program = parse(
            "state M: StateMap<i64, i64>; \
             fn f(m: StateMap<i64, i64>) { let _x = 0; } \
             fn main() { f(M); }",
        )
        .expect("parse state map arg");
        let err = analyze(&program).expect_err("passing state map to user fn should error");
        assert!(err.message.contains("E_STATE_MAP_ALIAS"));
    }

    #[test]
    fn scalar_state_requires_init() {
        let err = analyze_error("state counter: i64; fn read() -> i64 { return counter; }");
        assert_eq!(
            err.message,
            "E_STATE_INIT_REQUIRED: contract scalar state requires an `hajimari()` initializer"
        );
    }

    #[test]
    fn scalar_state_init_reports_every_missing_write() {
        let err = analyze_error("state first: i64; state second: i64; hajimari() { first = 0; }");
        assert_eq!(
            err.message,
            "E_STATE_INIT_INCOMPLETE: hajimari() must initialize every scalar state on every normal return or fallthrough path; missing: second"
        );
    }

    #[test]
    fn scalar_state_initialization_intersects_conditional_paths() {
        let accepted = parse(
            "state value: i64; \
             hajimari() { if true { value = 1; } else { value = 2; } }",
        )
        .expect("parse complete conditional initializer");
        analyze(&accepted).expect("both conditional paths initialize scalar state");

        let err = analyze_error(
            "state value: i64; \
             hajimari() { if true { value = 1; } }",
        );
        assert!(
            err.message.starts_with("E_STATE_INIT_INCOMPLETE:"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn scalar_state_initialization_checks_early_returns() {
        let err = analyze_error(
            "state value: i64; \
             hajimari() { if true { return; } value = 1; }",
        );
        assert!(
            err.message.starts_with("E_STATE_INIT_INCOMPLETE:"),
            "an early return must not bypass initialization: {err:?}"
        );

        let accepted = parse(
            "state value: i64; \
             hajimari() { if true { value = 1; return; } value = 2; }",
        )
        .expect("parse initialized early return");
        analyze(&accepted).expect("every normal exit initializes scalar state");
    }

    #[test]
    fn scalar_state_initialization_does_not_trust_optional_execution() {
        let loop_error = analyze_error(
            "state value: i64; \
             hajimari() { for index in range(1) { value = index; } }",
        );
        assert!(
            loop_error.message.starts_with("E_STATE_INIT_INCOMPLETE:"),
            "a loop body is not a definite write: {loop_error:?}"
        );

        let short_circuit_error = analyze_error(
            "state value: i64; \
             fn seed() -> bool { value = 1; return true; } \
             hajimari() { let ignored = false && seed(); }",
        );
        assert!(
            short_circuit_error
                .message
                .starts_with("E_STATE_INIT_INCOMPLETE:"),
            "a short-circuited helper is not a definite write: {short_circuit_error:?}"
        );
    }

    #[test]
    fn scalar_state_init_accepts_transitive_complete_initialization() {
        let program = parse(
            "state counter: i64; \
             struct Ledger { total: i64; } \
             state ledger: Ledger; \
             fn seed() { counter = 0; ledger = Ledger(0); } \
             hajimari() { seed(); }",
        )
        .expect("parse transitive scalar initializer");
        analyze(&program).expect("transitive init writes should initialize every scalar state");
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
            parse("fn f() { var x: i64 = true; x = false; }").expect("parse bool assignment");
        analyze(&program).expect_err("bool assignment must not coerce to i64");
    }

    #[test]
    fn immutable_local_reassignment_is_rejected() {
        let err = analyze_error("fn f() { let value = 1; value = 2; }");
        assert_eq!(
            err.message,
            "cannot assign to immutable binding `value`; declare a mutable local with `var`"
        );
    }

    #[test]
    fn mutable_local_reassignment_is_accepted() {
        let program = parse("fn f() -> i64 { var value = 1; value += 2; return value; }")
            .expect("parse mutable binding");
        analyze(&program).expect("var bindings should permit reassignment");
    }

    #[test]
    fn function_parameters_are_immutable() {
        let err = analyze_error("fn f(value: i64) { value = 2; }");
        assert_eq!(
            err.message,
            "cannot assign to immutable binding `value`; declare a mutable local with `var`"
        );
    }

    #[test]
    fn local_declarations_cannot_duplicate_or_shadow_bindings() {
        for source in [
            "fn f() { let value = 1; let value = 2; }",
            "fn f(value: i64) { let value = 2; }",
            "fn f() { let (left, left) = (1, 2); }",
        ] {
            analyze_error(source);
        }
    }

    #[test]
    fn parameters_and_locals_cannot_shadow_any_source_declaration() {
        for source in [
            "seiyaku App { fn helper() {} fn inspect(helper: i64) {} }",
            "seiyaku App { struct Receipt { value: i64; } fn inspect() { let Receipt = 1; } }",
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
        assert!(err.message.contains("E_BREAK_OUTSIDE_LOOP"));
    }

    #[test]
    fn continue_requires_loop_context() {
        let program = parse("fn f() { continue; }").expect("parse continue");
        let err = analyze(&program).expect_err("continue outside loop should error");
        assert!(err.message.contains("E_CONTINUE_OUTSIDE_LOOP"));
    }

    #[test]
    fn state_shadowing_is_rejected_in_let() {
        let program =
            parse("state counter: i64; fn f() { let counter = 1; }").expect("parse shadowing let");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert!(err.message.contains("E_STATE_SHADOWED"));
    }

    #[test]
    fn state_shadowing_is_rejected_in_params() {
        let program =
            parse("state counter: i64; fn f(counter: i64) {}").expect("parse shadowing param");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert!(err.message.contains("E_STATE_SHADOWED"));
    }

    #[test]
    fn state_shadowing_is_rejected_in_map_loop_vars() {
        let program = parse(
            "state counter: i64; state M: StateMap<i64, i64>; \
             fn f() { for (counter, v) in M.take(1) { let _x = v; } }",
        )
        .expect("parse shadowing loop vars");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert!(err.message.contains("E_STATE_SHADOWED"));
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
            },
        });
        let error = analyze(&program).expect_err("while AST must fail closed");
        assert!(error.message.contains("E_UNBOUNDED_LOOP"), "{error:?}");
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
                value: Expr::Number(0),
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
                    right: Box::new(Expr::Number(1)),
                },
            })),
            body: Block {
                statements: Vec::new(),
            },
        });
        let error = analyze(&program).expect_err("dynamic for AST must fail closed");
        assert!(error.message.contains("E_UNBOUNDED_LOOP"), "{error:?}");
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
            ("wrapping_add(1, 2)", "math::wrapping_add"),
            ("abs(-1)", "math::abs"),
            ("info(1)", "debug::info"),
            ("assert(true)", "test::assert"),
            ("assert_eq(1, 1)", "test::assert_eq"),
            ("actor_account(\"issuer\")", "test::actor_account"),
            (
                "invoke_entrypoint(\"run\", Json::parse(\"{}\"))",
                "test::invoke_entrypoint",
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
                "test::invoke_entrypoint",
            ),
            "__invoke_entrypoint__run targets test::invoke_entrypoint"
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
            "struct Pair { a: i64, b: i64 } \
             fn f() { let (a) = Pair(1, 2); }",
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
                .contains("assert expects (bool) or (bool, string|i64)")
        );
    }

    #[test]
    fn in_memory_map_constructor_is_rejected() {
        let program = parse("fn f() { let m: StateMap<Name, i64> = StateMap::new(); let _x = m; }")
            .expect("parse StateMap::new");
        let err = analyze(&program).expect_err("V1 StateMap values must be durable state");
        assert!(
            err.message
                .contains("StateMap values may only refer directly to top-level durable state")
                || err.message.contains("E_STATE_MAP_ALIAS")
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
            parse(r#"fn f() { let b: bytes = b"hi"; let c: bytes = b"hi"; let _x = b == c; }"#)
                .expect("parse bytes equality");
        analyze(&program).expect("bytes equality should be allowed");
    }

    #[test]
    fn bytes_literal_types_as_bytes() {
        let program = parse(r#"fn f() { let b: bytes = b"ab"; }"#).expect("parse bytes literal");
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
        let program = parse("state M: StateMap<Json, i64>; fn f() {}").expect("parse state map");
        let err = analyze(&program).expect_err("state map key should be validated");
        assert!(
            err.message
                .contains("StateMap key type `Json` is not supported"),
            "unexpected error: {}",
            err.message
        );
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
        analyze(&program).expect("info should accept i64");
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
                kotoage fn run(count: i64) -> i64 authorize("Run") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_entrypoint("run", Json::parse("{\"count\": 7}"));
                    test::assert_eq(next, 8);
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
                kotoage fn run(count: i64) -> i64 authorize("Run") { return count; }

                fn helper() {
                    let _next = test::invoke_entrypoint("run", Json::parse("{\"count\": 7}"));
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
                kotoage fn run(count: i64) -> i64 authorize("Run") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_entrypoint(Name::parse("run"), Json::parse("{\"count\": 7}"));
                    test::assert_eq(next, 8);
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
                kotoage fn run(count: i64) -> i64 authorize("Run") { return count; }

                #[test]
                fn drive_run() {
                    let target = "run";
                    let _next = test::invoke_entrypoint(target, Json::parse("{\"count\": 7}"));
                }
            }
            "#,
        )
        .expect("parse dynamic target invoke_entrypoint");
        let err = analyze_test(&program).expect_err("dynamic target should fail");
        assert!(err.message.contains("requires a literal entrypoint name"));
    }

    #[test]
    fn invoke_entrypoint_rejects_non_json_payload() {
        let program = parse(
            r#"
            seiyaku Demo {
                kotoage fn run(count: i64) -> i64 authorize("Run") { return count; }

                #[test]
                fn drive_run() {
                    let _next = test::invoke_entrypoint("run", 7);
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
                fn helper() -> i64 { return 7; }

                #[test]
                fn drive_run() {
                    let _next = test::invoke_entrypoint("helper", Json::parse("{}"));
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
                kotoage fn run(count: i64) -> i64 authorize("Run") { return count + 1; }

                #[test]
                fn drive_run() {
                    let next = test::invoke_entrypoint_as("issuer", "run", Json::parse("{\"count\": 7}"));
                    let acct = test::actor_account("issuer");
                    let pk = test::actor_public_key("issuer");
                    let sig = test::actor_sign("issuer", b"demo");
                    test::expect_reject_as("issuer", "run", Json::parse("{\"count\": -1}"));
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
                kotoage fn run(count: i64) -> (i64, i64) authorize("Run") { return (count, count + 1); }

                #[test]
                fn drive_run() {
                    let _pair = test::invoke_entrypoint_as("issuer", "run", Json::parse("{\"count\": 7}"));
                }
            }
            "#,
        )
        .expect("parse tuple invoke_entrypoint_as");
        analyze_test(&program).expect("tuple-returning target should type-check");
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
            "seiyaku Demo { view fn f(ev: Json) -> i64 { return ev.get_int(Name::parse(\"n\")); } }",
        )
        .expect("parse view get_int");
        analyze(&program).expect("typed Json parameters may use explicit JSON getters");
    }

    #[test]
    fn view_entrypoints_reject_ensure() {
        let program = parse(
            "seiyaku Demo { state balances: StateMap<i64, i64>; view fn f() -> i64 { return balances.ensure(7, 9); } }",
        )
        .expect("parse ensure");
        let err = analyze(&program).expect_err("view ensure should fail");
        assert!(
            err.message
                .contains("view entrypoints cannot use mutating map helper `ensure`"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn view_entrypoints_accept_get_or() {
        let program = parse(
            "seiyaku Demo { state balances: StateMap<i64, i64>; view fn f() -> i64 { return balances.get_or(7, 9); } }",
        )
        .expect("parse get_or");
        analyze(&program).expect("view get_or should type-check");
    }

    #[test]
    fn state_map_get_returns_option_without_intercepting_user_get_function() {
        let program = parse(
            "seiyaku Demo { \
                state balances: StateMap<i64, i64>; \
                fn get(value: i64) -> i64 { return value; } \
                view fn lookup(key: i64) -> Option<i64> { return balances.get(key); } \
                view fn echo(value: i64) -> i64 { return get(value); } \
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
                "seiyaku Demo { state balances: StateMap<i64, i64>; view fn read() -> i64 { return balances[1]; } }",
                "E_STATE_MAP_OPTIONAL_READ",
            ),
            (
                "seiyaku Demo { state balances: StateMap<i64, i64>; kotoage fn add() authorize(\"Write\") { balances[1] += 1; } }",
                "E_STATE_MAP_OPTIONAL_READ",
            ),
            (
                "seiyaku Demo { state balances: StateMap<i64, i64>; view fn read() -> Option<i64> { return get(balances, 1); } }",
                "unknown function or builtin `get`",
            ),
        ] {
            let program =
                parse(source).expect("invalid StateMap read should parse before resolution");
            let error = analyze(&program).expect_err("invalid StateMap read must fail closed");
            assert!(
                error.message.contains(expected),
                "unexpected error for `{source}`: {error:?}"
            );
        }

        let write = parse(
            "seiyaku Demo { state balances: StateMap<i64, i64>; kotoage fn set(key: i64, value: i64) authorize(\"Write\") { balances[key] = value; } }",
        )
        .expect("parse indexed StateMap write");
        analyze(&write).expect("simple indexed StateMap assignment must remain valid");
    }

    #[test]
    fn state_map_remove_returns_option_for_scalar_values() {
        let program = parse(
            "seiyaku Demo { state balances: StateMap<Name, i64>; kotoage fn f(key: Name) -> Option<i64> authorize(\"WriteState\") { return balances.remove(key); } }",
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
            "seiyaku Demo { state balances: StateMap<i64, i64>; view fn f() -> Option<i64> { return balances.remove(7); } }",
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
            "seiyaku Demo { state counter: i64; hajimari() { counter = 0; } view fn f() -> i64 { counter = 1; return counter; } }",
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
            "seiyaku Demo { state balances: StateMap<i64, i64>; view fn f() -> i64 { balances[7] = 9; return 1; } }",
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
            "seiyaku Demo { state counter: i64; hajimari() { counter = 0; } fn helper() { counter = counter + 1; } view fn f() -> i64 { helper(); return counter; } }",
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
            "call_contract",
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
                err.message.contains("compiler-internal"),
                "unexpected error for {name}: {}",
                err.message
            );
        }
    }

    #[test]
    fn generic_execute_instruction_is_not_a_builtin() {
        let program = parse("fn f(payload: bytes) { execute_instruction(payload); }")
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
            "runtime::set_vector_length",
            "debug::print_i64",
            "debug::log",
            "axt::begin",
            "axt::touch",
            "soracloud::read_committed_state",
            "soracloud::read_secret",
        ] {
            let Ok(program) = parse(&format!("fn f() {{ {source_name}(); }}")) else {
                assert_eq!(source_name, "contract::call");
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
    fn public_entrypoints_reject_zk_verify_without_permission() {
        let program = parse(
            "seiyaku Demo { kotoage fn verify(payload: bytes) { crypto::zk::verify_unshield(payload); } }",
        )
        .expect("parse public zk verify");
        let err = SemanticContext::with_zk_enabled(true)
            .analyze(&program)
            .expect_err("public zk verify should require permission");
        assert!(
            err.message
                .contains("kotoage function `verify` requires `authorize(\"Permission\")`"),
            "unexpected error message: {}",
            err.message
        );
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
                    recipient: AccountId,
                    asset: AssetDefinitionId,
                    amount: Amount,
                    dataspace: DataSpaceId
                ) authorize("TransferAsset") {
                    let sender = context::authority();
                    ledger::asset::transfer(sender, recipient, asset, amount, dataspace);
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
                    escrow: Name,
                    asset: AssetDefinitionId,
                    amount: Amount,
                    evidence: bytes
                ) {
                    ledger::escrow::open_offer(escrow, asset, amount);
                    ledger::escrow::open_offer(escrow, asset, amount, evidence);
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
                    escrow: Name,
                    account: AccountId,
                    asset: AssetDefinitionId,
                    amount: Amount
                ) {
                    ledger::escrow::open_offer(escrow, account, account, asset, amount);
                }
            }
            "#,
        )
        .expect("parse invalid escrow call");
        let error = analyze(&invalid).expect_err("the retired five-argument shape must fail");
        assert!(
            error
                .message
                .contains("expects (Name, AssetDefinitionId, Amount[, bytes evidence_hashes])"),
            "unexpected diagnostic: {}",
            error.message
        );
    }

    #[test]
    fn public_entrypoints_reject_state_mutation_without_permission() {
        let program = parse(
            "seiyaku Demo { state counter: i64; hajimari() { counter = 0; } kotoage fn set() { counter = 1; } }",
        )
        .expect("parse public state mutation");
        let err = analyze(&program).expect_err("public state mutation should require permission");
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
            "seiyaku Demo { fn helper(payload: bytes) { crypto::zk::verify_transfer(payload); } view fn f(payload: bytes) -> i64 { helper(payload); return 1; } }",
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
                    status: i64,
                    alias_blob: bytes,
                    requested_by_actor_id: bytes,
                    requested_by_actor: Json
                }
                state Requests: StateMap<Name, Request>;
                kotoage fn create_request(proposal_id: Name,
                                          alias_literal: bytes,
                                          requested_by_actor_id: bytes,
                                          requested_by_actor: Json) authorize("CreateRequest") {
                    Requests[proposal_id] = Request(
                        1,
                        alias_literal,
                        requested_by_actor_id,
                        requested_by_actor
                    );
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
                let dst = ev.get_account_id(Name::parse(\"account_id\")); \
                let sink = ledger::account::resolve_alias(\"banking@centralbank\"); \
                let _same = dst == sink; \
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
    fn get_numeric_accepts_trigger_amounts() {
        let program = parse(
            "fn f() { let ev = context::trigger_event(); let _amount: Amount = ev.get_numeric(Name::parse(\"amount\")); }",
        )
        .expect("parse get_numeric");
        analyze(&program).expect("get_numeric should type-check");
    }

    #[test]
    fn durable_string_state_is_supported() {
        let program = parse(
            r#"seiyaku C {
                state label: string;
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
                struct S { label: string }
                state s: S;
                hajimari() { s = S("ready"); }
            }"#,
        )
        .expect("parse state struct");
        analyze(&program).expect("string state field should be supported");
    }

    #[test]
    fn nested_state_map_is_rejected() {
        let ty = Type::Struct {
            name: "S".into(),
            fields: vec![(
                "children".into(),
                Type::StateMap(Box::new(Type::Int), Box::new(Type::Int)),
            )],
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
                struct Pair { count: i64, ready: bool }
                state maybe: Option<Pair>;
                state outcome: Result<Pair, Pair>;
                hajimari() {
                    maybe = option::none(Pair(0, false));
                    outcome = result::ok(Pair(1, true), Pair(0, false));
                }
            }"#,
        )
        .expect("parse aggregate sum state");
        analyze(&program).expect("aggregate Option/Result state should type-check");
    }

    #[test]
    fn explicit_numeric_conversions_preserve_nominal_types() {
        let program = parse(
            "seiyaku C { fn f(value: i64) -> Amount { \
                let wide: u128 = u128::from_i64(value); \
                let amount: Amount = Amount::from_u128(wide); \
                return amount; \
            } }",
        )
        .expect("parse explicit conversions");
        let typed = analyze(&program).expect("analyze explicit conversions");
        let TypedItem::Function(f) = &typed.items[0];
        assert_eq!(f.ret_ty, Some(Type::Amount));
    }

    #[test]
    fn numeric_types_do_not_mix_implicitly() {
        let program = parse(
            "seiyaku C { fn f(a: Amount, b: u128, c: i64) { \
                let _x = a + b; \
                let _y: u128 = c; \
            } }",
        )
        .expect("parse nominal numeric types");
        let err = analyze(&program).expect_err("mixed numeric types should error");
        assert!(
            err.message.contains("identical numeric operand types")
                || err.message.contains("expected u128, got i64"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn unsuffixed_integer_is_not_an_implicit_u128_literal() {
        let program = parse("seiyaku C { fn f() { let value: u128 = 1; } }")
            .expect("parse unsuffixed literal");
        let error = analyze(&program).expect_err("implicit i64-to-u128 conversion must fail");
        assert!(
            error.message.contains("expected u128, got i64"),
            "unexpected error: {}",
            error.message
        );
    }

    #[test]
    fn u128_max_literal_is_accepted_and_typed_as_u128() {
        let program = parse(
            "seiyaku C { fn wide_value() -> u128 { \
                return 340282366920938463463374607431768211455u128; \
            } }",
        )
        .expect("parse u128::MAX");
        let typed = analyze(&program).expect("analyze u128::MAX");
        let TypedItem::Function(function) = &typed.items[0];
        assert_eq!(function.ret_ty, Some(Type::FixedU128));
    }

    #[test]
    fn u128_negation_builtin_is_rejected() {
        let program =
            parse("seiyaku C { fn f(value: u128) -> u128 { return numeric::neg(value); } }")
                .expect("parse u128 negation");
        let error = analyze(&program).expect_err("numeric::neg(u128) must fail");
        assert!(
            error.message.contains("not defined for unsigned u128"),
            "unexpected error: {}",
            error.message
        );
    }

    #[test]
    fn explicit_i64_to_u128_rejects_negative_literal() {
        let program = parse("seiyaku C { fn f() -> u128 { return u128::from_i64(-1); } }")
            .expect("parse explicit conversion");
        let error = analyze(&program).expect_err("negative conversion must fail");
        assert!(
            error.message.contains("cannot convert a negative i64"),
            "unexpected error: {}",
            error.message
        );
    }

    #[test]
    fn wide_values_are_not_implicitly_narrowed_for_i64_builtins() {
        let program = parse("seiyaku C { fn f(value: u128) -> i64 { return math::abs(value); } }")
            .expect("parse wide math argument");
        let error = analyze(&program).expect_err("u128-to-i64 builtin coercion must fail");
        assert!(
            error.message.contains("expects (i64)"),
            "unexpected error: {}",
            error.message
        );
    }

    #[test]
    fn ledger_amount_parameters_reject_implicit_i64_conversion() {
        let program = parse(
            "seiyaku C { fn f(account: AccountId, asset: AssetDefinitionId) { \
                ledger::asset::mint(account, asset, 1); \
            } }",
        )
        .expect("parse ledger amount call");
        let error = analyze(&program).expect_err("i64-to-Amount builtin coercion must fail");
        assert!(
            error.message.contains("AssetDefinitionId, Amount"),
            "unexpected error: {}",
            error.message
        );
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
        assert!(err.message.contains("kotoage entrypoint"));
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
                error.message.contains("must call kotoage entrypoint"),
                "unexpected {lifecycle} callback error: {error:?}"
            );
        }
    }

    #[test]
    fn semantic_analysis_defends_against_lifecycle_permission_hints() {
        let mut program = parse("seiyaku Demo { hajimari() {} }").expect("parse initializer");
        let Item::Function(initializer) = &mut program.items[0] else {
            panic!("expected initializer")
        };
        initializer.modifiers.permission = Some("SourceOwnedPermission".to_owned());

        let error = analyze(&program).expect_err("lifecycle permission must be rejected");
        assert!(
            error
                .message
                .contains("lifecycle authorization is runtime-defined")
        );
    }
}
