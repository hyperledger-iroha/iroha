//! Abstract syntax tree definitions for KOTODAMA.
//!
//! These structures represent the parsed Kotodama source surface accepted by
//! the compiler.

/// Compiler-owned call name used after parsing the canonical
/// `StateMap.get(key)` method form.
///
/// Source code cannot call this name directly. Keeping the marker distinct
/// from the source spelling preserves enough call-form information for name
/// resolution to distinguish a user function named `get` from the StateMap
/// intrinsic.
pub(crate) const STATE_MAP_GET_INTRINSIC: &str = "state_map_get";

#[derive(Debug, PartialEq, Clone)]
pub struct Program {
    /// The single named source unit declared by this file.
    pub unit: SourceUnit,
    pub items: Vec<Item>,
    /// Optional standalone test-file target declaration.
    pub test_target: Option<TestTargetDecl>,
    /// Optional local test fixtures available to `#[test(...)]` functions.
    pub fixtures: Vec<FixtureDecl>,
}

/// Whether a source file declares deployable contract code or a library module.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SourceUnitKind {
    /// A deployable contract unit.
    Contract,
    /// A non-deployable library unit.
    Module,
}

/// Identity of the single top-level source unit.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceUnit {
    /// Unit category.
    pub kind: SourceUnitKind,
    /// Source-level contract or module name.
    pub name: String,
}

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

/// Visibility of a function when exposed to the host/runtime.
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum FunctionVisibility {
    /// Callable only from within the contract module (default).
    #[default]
    Internal,
    /// Exposed as a `kotoage fn` entrypoint.
    Public,
}

/// Logical role of a function inside the contract.
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum FunctionKind {
    /// Legacy internal category retained for compiler-created helpers.
    #[default]
    Free,
    /// Function defined inside a `contract` or `module` body.
    Contract,
    /// Seiyaku initializer (`hajimari` / `始まり`).
    Init,
    /// Seiyaku upgrade hook (`kaizen` / `改善`).
    Upgrade,
    /// Read-only public query entrypoint (`view fn`).
    View,
}

/// Parsed modifiers associated with a function.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct FunctionModifiers {
    pub visibility: FunctionVisibility,
    pub kind: FunctionKind,
    /// Optional caller authorization declared with `authorize("Permission")`.
    pub permission: Option<String>,
    /// Reserved parser storage for access hints; first-release Kotodama rejects
    /// user-written access attributes before lowering.
    pub access_reads: Vec<String>,
    /// Reserved parser storage for access hints; first-release Kotodama rejects
    /// user-written access attributes before lowering.
    pub access_writes: Vec<String>,
    /// Marks a function as a local-only Kotodama test.
    pub is_test: bool,
    /// Optional fixture bound to a Kotodama test function.
    pub test_fixture: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Item {
    Function(Function),
    /// User-defined product type with named fields.
    Struct(StructDef),
    /// Stable contract error codes used by `require`.
    ErrorEnum(ErrorEnumDef),
    /// Contract-level constant declaration.
    Const(ConstDecl),
    /// Contract-level durable state declaration lowered to host-backed state
    /// paths, including flattened singleton struct/tuple children.
    State(StateDecl),
    /// Contract-level trigger declaration (manifest-only metadata).
    Trigger(TriggerDecl),
}

/// A syntactic type expression as written by the user.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// A path or simple identifier, e.g. `i64`, `AccountId`.
    Path(String),
    /// A generic type, such as `StateMap<K, V>`.
    Generic { base: String, args: Vec<TypeExpr> },
    /// A tuple type, e.g. `(i64, bool)`.
    Tuple(Vec<TypeExpr>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Param {
    pub ty: Option<TypeExpr>,
    pub name: String,
    /// Internal state-handle marker; canonical V1 source parameters always set this to `false`.
    pub is_state: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Option<TypeExpr>,
    pub body: Block,
    pub modifiers: FunctionModifiers,
    pub location: SourceLocation,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
}

/// A user-defined struct with named fields.
#[derive(Debug, PartialEq, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, TypeExpr)>,
}

/// Declared stable error-code namespace.
#[derive(Debug, PartialEq, Clone)]
pub struct ErrorEnumDef {
    pub name: String,
    pub variants: Vec<ErrorVariant>,
}

/// One explicitly numbered contract error.
#[derive(Debug, PartialEq, Clone)]
pub struct ErrorVariant {
    pub name: String,
    pub code: u32,
}

/// A contract-level `const` declaration: `const NAME: Type = expr;`.
#[derive(Debug, PartialEq, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub value: Expr,
}

/// A contract-level `state` declaration: `state name: Type;`.
#[derive(Debug, PartialEq, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: TypeExpr,
}

/// Standalone Kotodama test-file declaration identifying the contract under test.
#[derive(Debug, PartialEq, Clone)]
pub struct TestTargetDecl {
    pub target: String,
}

/// Declarative local fixture used by `koto_test`.
#[derive(Debug, PartialEq, Clone)]
pub struct FixtureDecl {
    pub name: String,
    pub actions: Vec<FixtureAction>,
}

/// One fixture action expressed as a function-style command, for example
/// `caller(account("alice@wonderland"))`.
#[derive(Debug, PartialEq, Clone)]
pub struct FixtureAction {
    pub name: String,
    pub args: Vec<Expr>,
}

/// Contract-level trigger declaration.
#[derive(Debug, PartialEq, Clone)]
pub struct TriggerDecl {
    pub name: String,
    pub call: TriggerCall,
    pub filter: TriggerFilter,
    pub repeats: Option<TriggerRepeats>,
    pub authority: Option<String>,
    pub metadata: Vec<TriggerMetadataEntry>,
}

/// Trigger callback target.
#[derive(Debug, PartialEq, Clone)]
pub struct TriggerCall {
    pub namespace: Option<String>,
    pub entrypoint: String,
}

/// Trigger filter definition.
#[derive(Debug, PartialEq, Clone)]
pub enum TriggerFilter {
    Time(TriggerTimeFilter),
    Execute { trigger_id: String },
    Data(TriggerDataFilter),
    Pipeline(TriggerPipelineFilter),
}

/// Data trigger filter variants.
#[derive(Debug, PartialEq, Clone)]
pub enum TriggerDataFilter {
    Any,
    Structured(TriggerStructuredDataFilter),
}

/// Structured data trigger filter block.
#[derive(Debug, PartialEq, Clone)]
pub struct TriggerStructuredDataFilter {
    pub family: TriggerDataFamily,
    pub event: TriggerDataEventKind,
    pub matchers: Vec<TriggerDataMatcher>,
}

/// Supported data-event families for contract trigger declarations.
#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub enum TriggerDataFamily {
    Peer,
    Domain,
    Account,
    Asset,
    AssetDefinition,
    Nft,
    Rwa,
    Trigger,
    Role,
    Configuration,
    Executor,
}

/// Event kind selector inside a structured data trigger.
#[derive(Debug, PartialEq, Clone)]
pub enum TriggerDataEventKind {
    Any,
    Named(String),
}

/// Matcher entry inside a structured data trigger block.
#[derive(Debug, PartialEq, Clone)]
pub struct TriggerDataMatcher {
    pub key: String,
    pub value: String,
}

/// Pipeline trigger filter variants.
#[derive(Debug, PartialEq, Clone)]
pub enum TriggerPipelineFilter {
    TransactionApproved,
    BlockApproved,
}

/// Time trigger filter variants.
#[derive(Debug, PartialEq, Clone)]
pub enum TriggerTimeFilter {
    PreCommit,
    Schedule {
        start_ms: u64,
        period_ms: Option<u64>,
    },
}

/// Trigger repeat policy.
#[derive(Debug, PartialEq, Clone)]
pub enum TriggerRepeats {
    Indefinitely,
    Exactly(u32),
}

/// Trigger metadata entry (literal JSON values only).
#[derive(Debug, PartialEq, Clone)]
pub struct TriggerMetadataEntry {
    pub key: String,
    pub value: Expr,
}

/// Contract-level localization table.
#[derive(Debug, PartialEq, Clone)]
pub struct MessageBlock {
    pub entries: Vec<MessageEntry>,
}

/// Localized message entry keyed by a stable message id.
#[derive(Debug, PartialEq, Clone)]
pub struct MessageEntry {
    pub msg_id: String,
    pub translations: Vec<MessageTranslation>,
}

/// Localization entry for a specific language tag.
#[derive(Debug, PartialEq, Clone)]
pub struct MessageTranslation {
    pub lang: String,
    pub text: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Pattern {
    Name(String),
    Tuple(Vec<String>),
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AssignOp {
    /// Simple assignment: `=`.
    Set,
    /// Compound assignment: `+=`.
    Add,
    /// Compound assignment: `-=`.
    Sub,
    /// Compound assignment: `*=`.
    Mul,
    /// Compound assignment: `/=`.
    Div,
    /// Compound assignment: `%=`.
    Mod,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    /// Local binding declared with `let` (immutable) or `var` (mutable).
    Let {
        /// Whether the source declaration used `var`.
        mutable: bool,
        pat: Pattern,
        ty: Option<TypeExpr>,
        value: Expr,
    },
    /// Assignment to a mutable local, parameter/state handle, or state binding.
    Assign {
        name: String,
        value: Expr,
    },
    /// Assignment to a general lvalue (field/indexed), e.g. `a[i].f = v` or `obj.x += 1`.
    AssignExpr {
        target: Expr,
        op: AssignOp,
        value: Expr,
    },
    Expr(Expr),
    Return(Option<Expr>),
    Break,
    Continue,
    If {
        cond: Expr,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    For {
        line: usize,
        init: Option<Box<Statement>>,
        cond: Option<Expr>,
        step: Option<Box<Statement>>,
        body: Block,
    },
    /// Canonically bounded map iteration: `for (k, v) in map.take(64) { ... }`.
    ForEachMap {
        key: String,
        value: Option<String>,
        map: Expr,
        body: Block,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    /// Ternary conditional expression: `cond ? then : else`.
    Conditional {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    /// Call to a builtin function like `crypto::poseidon2(a, b)`.
    Call {
        name: String,
        args: Vec<Expr>,
    },
    /// Field access: `expr.field`
    Member {
        object: Box<Expr>,
        field: String,
    },
    /// Indexing: `expr[index]`
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    /// Tuple literal: `(a, b, c)`
    Tuple(Vec<Expr>),
    Bool(bool),
    Number(i64),
    /// Canonical text of an explicitly `u128`-suffixed integer literal.
    ///
    /// The historical variant name is internal; V1 source does not support
    /// decimal-fraction literals.
    Decimal(String),
    String(String),
    Bytes(Vec<u8>),
    Ident(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    /// Integer modulo (remainder) operator: `a % b`.
    Mod,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

impl Program {
    /// Derive the runtime half of an explicitly selected local test suite.
    ///
    /// Production compiler paths must never call this helper: they reject test
    /// syntax instead of silently changing the source program. The `koto test`
    /// driver uses it only after discovering and compiling the complete suite
    /// in explicit test mode.
    #[must_use]
    pub fn without_local_tests_for_runner(&self) -> Self {
        Self {
            unit: self.unit.clone(),
            items: self
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Function(func) if func.modifiers.is_test => None,
                    _ => Some(item.clone()),
                })
                .collect(),
            test_target: None,
            fixtures: Vec::new(),
        }
    }
}

/// Destroy a parsed program without recursively dropping adversarially deep
/// expression, statement, or type trees.
///
/// V1 accepts nesting up to the fixed frontend limit. Rust's derived drop glue
/// walks recursive enums on the caller's stack, which can overflow the smaller
/// stacks used by editor workers and test executors even though parsing itself
/// stayed within that limit. Tooling that consumes an AST only for validation
/// uses this explicit work list instead.
pub(crate) fn drop_program_iterative(program: Program) {
    enum Pending {
        Statement(Statement),
        Expr(Expr),
        Type(TypeExpr),
    }

    fn push_block(block: Block, pending: &mut Vec<Pending>) {
        pending.extend(block.statements.into_iter().map(Pending::Statement));
    }

    let Program {
        unit: _,
        items,
        test_target: _,
        fixtures,
    } = program;
    let mut pending = Vec::new();

    for item in items {
        match item {
            Item::Function(function) => {
                pending.extend(
                    function
                        .params
                        .into_iter()
                        .filter_map(|parameter| parameter.ty)
                        .map(Pending::Type),
                );
                pending.extend(function.ret_ty.into_iter().map(Pending::Type));
                push_block(function.body, &mut pending);
            }
            Item::Struct(definition) => pending.extend(
                definition
                    .fields
                    .into_iter()
                    .map(|(_, ty)| Pending::Type(ty)),
            ),
            Item::Const(declaration) => {
                pending.extend(declaration.ty.into_iter().map(Pending::Type));
                pending.push(Pending::Expr(declaration.value));
            }
            Item::State(declaration) => pending.push(Pending::Type(declaration.ty)),
            Item::Trigger(declaration) => pending.extend(
                declaration
                    .metadata
                    .into_iter()
                    .map(|entry| Pending::Expr(entry.value)),
            ),
            Item::ErrorEnum(_) => {}
        }
    }
    for fixture in fixtures {
        for action in fixture.actions {
            pending.extend(action.args.into_iter().map(Pending::Expr));
        }
    }

    while let Some(node) = pending.pop() {
        match node {
            Pending::Type(TypeExpr::Generic { args, .. })
            | Pending::Type(TypeExpr::Tuple(args)) => {
                pending.extend(args.into_iter().map(Pending::Type));
            }
            Pending::Type(TypeExpr::Path(_)) => {}
            Pending::Statement(statement) => match statement {
                Statement::Let { ty, value, .. } => {
                    pending.extend(ty.into_iter().map(Pending::Type));
                    pending.push(Pending::Expr(value));
                }
                Statement::Assign { value, .. } | Statement::Expr(value) => {
                    pending.push(Pending::Expr(value));
                }
                Statement::AssignExpr { target, value, .. } => {
                    pending.push(Pending::Expr(target));
                    pending.push(Pending::Expr(value));
                }
                Statement::Return(value) => {
                    pending.extend(value.into_iter().map(Pending::Expr));
                }
                Statement::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(Pending::Expr(cond));
                    push_block(then_branch, &mut pending);
                    if let Some(block) = else_branch {
                        push_block(block, &mut pending);
                    }
                }
                Statement::While { cond, body } => {
                    pending.push(Pending::Expr(cond));
                    push_block(body, &mut pending);
                }
                Statement::For {
                    init,
                    cond,
                    step,
                    body,
                    ..
                } => {
                    pending.extend(
                        init.into_iter()
                            .map(|statement| Pending::Statement(*statement)),
                    );
                    pending.extend(cond.into_iter().map(Pending::Expr));
                    pending.extend(
                        step.into_iter()
                            .map(|statement| Pending::Statement(*statement)),
                    );
                    push_block(body, &mut pending);
                }
                Statement::ForEachMap { map, body, .. } => {
                    pending.push(Pending::Expr(map));
                    push_block(body, &mut pending);
                }
                Statement::Break | Statement::Continue => {}
            },
            Pending::Expr(expression) => match expression {
                Expr::Binary { left, right, .. }
                | Expr::Index {
                    target: left,
                    index: right,
                } => {
                    pending.push(Pending::Expr(*left));
                    pending.push(Pending::Expr(*right));
                }
                Expr::Unary { expr, .. } | Expr::Member { object: expr, .. } => {
                    pending.push(Pending::Expr(*expr));
                }
                Expr::Conditional {
                    cond,
                    then_expr,
                    else_expr,
                } => {
                    pending.push(Pending::Expr(*cond));
                    pending.push(Pending::Expr(*then_expr));
                    pending.push(Pending::Expr(*else_expr));
                }
                Expr::Call { args, .. } | Expr::Tuple(args) => {
                    pending.extend(args.into_iter().map(Pending::Expr));
                }
                Expr::Bool(_)
                | Expr::Number(_)
                | Expr::Decimal(_)
                | Expr::String(_)
                | Expr::Bytes(_)
                | Expr::Ident(_) => {}
            },
        }
    }
}
