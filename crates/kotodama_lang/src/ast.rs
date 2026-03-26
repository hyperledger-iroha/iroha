//! Abstract syntax tree definitions for KOTODAMA.
//!
//! These structures represent a small subset of the language used for
//! demonstration purposes.

#[derive(Debug, PartialEq, Clone)]
pub struct Program {
    pub items: Vec<Item>,
    /// Optional contract-level metadata captured from a `seiyaku` container.
    pub contract_meta: Option<ContractMeta>,
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
    /// Exposed as a public entrypoint (e.g., `kotoage fn`).
    Public,
}

/// Logical role of a function inside the contract.
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum FunctionKind {
    /// Free-standing function (top-level outside any `seiyaku` block).
    #[default]
    Free,
    /// Function defined inside a `seiyaku` contract body.
    Contract,
    /// Contract initializer (`hajimari`).
    Hajimari,
    /// Contract upgrade hook (`kaizen`).
    Kaizen,
    /// Read-only public query entrypoint (`view fn`).
    View,
}

/// Parsed modifiers associated with a function.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct FunctionModifiers {
    pub visibility: FunctionVisibility,
    pub kind: FunctionKind,
    pub permission: Option<String>,
    /// Optional explicit read access hints for this function.
    pub access_reads: Vec<String>,
    /// Optional explicit write access hints for this function.
    pub access_writes: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Item {
    Function(Function),
    /// User-defined product type with named fields.
    Struct(StructDef),
    /// Contract-level durable state declaration. For now, the compiler lowers
    /// these to ephemeral allocations per run; durable host-backed storage is
    /// pending and tracked in the roadmap/docs.
    State(StateDecl),
    /// Contract-level trigger declaration (manifest-only metadata).
    Trigger(TriggerDecl),
    /// Contract-level localization table (`kotoba { ... }`).
    Kotoba(KotobaBlock),
}

/// Metadata declared at the `seiyaku` contract level.
#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub enum ContractFeature {
    /// Request zero-knowledge tracing (sets the ZK mode bit).
    Zk,
    /// Request vector/SIMD tracing (sets the VECTOR mode bit).
    Vector,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ContractMeta {
    /// ABI version to encode into the IVM header.
    pub abi_version: Option<u8>,
    /// Vector length hint to encode in the IVM header (0 = max).
    pub vector_length: Option<u8>,
    /// Maximum cycles to encode in the IVM header (0 = none).
    pub max_cycles: Option<u64>,
    /// Force ZK mode bit in the header.
    pub force_zk: Option<bool>,
    /// Force VECTOR mode bit in the header.
    pub force_vector: Option<bool>,
    /// Requested feature toggles (e.g., `"zk"`, `"simd"`).
    pub features: Vec<ContractFeature>,
}

/// A syntactic type expression as written by the user.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// A path or simple identifier, e.g. `int`, `AccountId`.
    Path(String),
    /// A generic type: `Map<K,V>`
    Generic { base: String, args: Vec<TypeExpr> },
    /// A tuple type, e.g. `(int, bool)`
    Tuple(Vec<TypeExpr>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Param {
    pub ty: Option<TypeExpr>,
    pub name: String,
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

/// A contract-level `state` declaration: `state Type name;`.
#[derive(Debug, PartialEq, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: TypeExpr,
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
    Transaction,
    Block,
    Merge,
    Witness,
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
pub struct KotobaBlock {
    pub entries: Vec<KotobaEntry>,
}

/// Localized message entry keyed by a stable message id.
#[derive(Debug, PartialEq, Clone)]
pub struct KotobaEntry {
    pub msg_id: String,
    pub translations: Vec<KotobaTranslation>,
}

/// Localization entry for a specific language tag.
#[derive(Debug, PartialEq, Clone)]
pub struct KotobaTranslation {
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
    /// Variable declaration with optional type annotation.
    Let {
        pat: Pattern,
        ty: Option<TypeExpr>,
        value: Expr,
    },
    /// Simple assignment to a local variable (SSA-style rebinding).
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
    /// For‑each iteration over maps: `for (k, v) in map { ... }`
    ForEachMap {
        key: String,
        value: Option<String>,
        map: Expr,
        bound: Option<usize>,
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
    /// Call to a builtin function like `poseidon2(a,b)`.
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
