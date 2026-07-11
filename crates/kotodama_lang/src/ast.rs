//! Abstract syntax tree definitions for KOTODAMA.
//!
//! These structures represent the parsed Kotodama source surface accepted by
//! the compiler.

use crate::source::{SourceRange, TextRange};

/// Stable identity assigned when a spanned AST node enters resolved HIR.
///
/// The identity is local to one source unit. It is embedded in resolver-only
/// provenance wrappers, so moving or cloning a resolved program cannot detach
/// semantic analysis from the name-resolution result it is required to use.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct HirId(pub(crate) u32);

/// Stable identity assigned by the CST/AST parser to one source-backed node.
///
/// Unlike an address-derived lookup key, this identity is stored directly in
/// source provenance and therefore survives moves, clones, and parser caches.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    pub(crate) fn index(self) -> usize {
        usize::try_from(self.0).expect("u32 source-node identity fits usize")
    }
}

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

/// Whether a source file declares a deployable `seiyaku` or a library module.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SourceUnitKind {
    /// A deployable `seiyaku`/`誓約` unit.
    Seiyaku,
    /// A non-deployable library unit.
    Module,
}

/// Identity of the single top-level source unit.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceUnit {
    /// Unit category.
    pub kind: SourceUnitKind,
    /// Source-level seiyaku or module name.
    pub name: String,
}

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

/// Exact declaration role of a function inside a seiyaku or module.
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum FunctionKind {
    /// Private `fn`, including module-private and compiler-created helpers.
    #[default]
    Private,
    /// Mutating public `kotoage fn`/`言挙げ fn` declaration.
    Kotoage,
    /// Seiyaku lifecycle declaration (`hajimari` / `始まり`).
    Hajimari,
    /// Seiyaku lifecycle declaration (`kaizen` / `改善`).
    Kaizen,
    /// Read-only public query entrypoint (`view fn`).
    View,
}

/// Parsed modifiers associated with a function.
#[derive(Debug, PartialEq, Clone, Default)]
pub struct FunctionModifiers {
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
    /// Stable seiyaku error codes used by `require`.
    ErrorEnum(ErrorEnumDef),
    /// Seiyaku-level constant declaration.
    Const(ConstDecl),
    /// Seiyaku-level durable state declaration lowered to host-backed state
    /// paths, including flattened singleton struct/tuple children.
    State(StateDecl),
    /// Seiyaku-level trigger declaration (manifest-only metadata).
    Trigger(TriggerDecl),
}

/// A syntactic type expression as written by the user.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    /// Parser-internal provenance carrier removed before any canonical public
    /// parse/session output is returned. This is not a Kotodama type variant.
    #[doc(hidden)]
    Source {
        node: NodeId,
        source: SourceRange,
        ty: Box<TypeExpr>,
    },
    /// Resolver-owned provenance carrier. This variant never appears in parser
    /// output and is removed before an AST is returned to source tooling.
    #[doc(hidden)]
    Resolved {
        id: HirId,
        source: Option<SourceRange>,
        ty: Box<TypeExpr>,
    },
    /// A path or simple identifier, e.g. `i64`, `AccountId`.
    Path(String),
    /// A generic type, such as `StateMap<K, V>`.
    Generic { base: String, args: Vec<TypeExpr> },
    /// A tuple type, e.g. `(i64, bool)`.
    Tuple(Vec<TypeExpr>),
    /// A non-negative compile-time integer argument, used by `List<T, N>`.
    Const(u64),
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
    /// Final expression without a semicolon, evaluated as the block value.
    pub tail: Option<Box<Expr>>,
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

/// One explicitly numbered seiyaku error.
#[derive(Debug, PartialEq, Clone)]
pub struct ErrorVariant {
    pub name: String,
    pub code: u32,
}

/// A seiyaku-level `const` declaration: `const NAME: Type = expr;`.
#[derive(Debug, PartialEq, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub value: Expr,
}

/// A seiyaku-level `state` declaration: `state name: Type;`.
#[derive(Debug, PartialEq, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: TypeExpr,
}

/// Standalone Kotodama test-file declaration identifying the seiyaku under test.
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

/// Seiyaku-level trigger declaration.
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

/// Seiyaku-level localization table.
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

/// Canonical namespaced variant admitted in `match` and `if let` patterns.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum SumVariant {
    OptionSome,
    OptionNone,
    ResultOk,
    ResultErr,
}

/// Payload handling for an active sum variant pattern.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PatternBinding {
    /// Bind the active payload to a new local.
    Name(String),
    /// Explicitly ignore the active payload.
    Wildcard,
}

/// One exhaustive namespaced `Option` or `Result` pattern.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SumPattern {
    pub variant: SumVariant,
    /// `None` is valid only for the inactive `Option::none` pattern.
    pub binding: Option<PatternBinding>,
}

/// One source `match` arm.
#[derive(Debug, PartialEq, Clone)]
pub struct MatchArm {
    pub pattern: SumPattern,
    pub body: Block,
}

/// One native JSON object entry with its decoded key and exact source spelling.
#[derive(Debug, PartialEq, Clone)]
pub struct JsonObjectEntry {
    /// Decoded object key used for duplicate detection and canonical encoding.
    pub key: String,
    /// Exact identifier or quoted-string token spelling retained for tooling.
    pub key_spelling: String,
    /// Exact source range of the key token.
    pub key_range: TextRange,
    /// Dynamically converted JSON value expression.
    pub value: Expr,
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
    /// Parser-internal provenance carrier removed before any canonical public
    /// parse/session output is returned. This is not a Kotodama statement.
    #[doc(hidden)]
    Source {
        node: NodeId,
        source: SourceRange,
        statement: Box<Statement>,
    },
    /// Resolver-owned provenance carrier. This variant never appears in parser
    /// output and is removed before an AST is returned to source tooling.
    #[doc(hidden)]
    Resolved {
        id: HirId,
        source: Option<SourceRange>,
        statement: Box<Statement>,
    },
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
    /// Pattern-guarded statement form. Unlike the expression form, `else` may be absent.
    IfLet {
        pattern: SumPattern,
        value: Expr,
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
    /// Parser-internal provenance carrier removed before any canonical public
    /// parse/session output is returned. This is not a Kotodama expression.
    #[doc(hidden)]
    Source {
        node: NodeId,
        source: SourceRange,
        expression: Box<Expr>,
    },
    /// Resolver-owned provenance carrier. This variant never appears in parser
    /// output and is removed before an AST is returned to source tooling.
    #[doc(hidden)]
    Resolved {
        id: HirId,
        source: Option<SourceRange>,
        expression: Box<Expr>,
    },
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
    /// Expression-valued conditional whose branches are blocks.
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    /// Expression-valued sum pattern test.
    IfLet {
        pattern: SumPattern,
        value: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    /// Exhaustive expression-valued `Option` or `Result` match.
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// Active `Option` constructor; no inactive payload is materialized.
    OptionSome(Box<Expr>),
    /// Contextual inactive `Option` constructor.
    OptionNone,
    /// Active success constructor; the error type is supplied by context.
    ResultOk(Box<Expr>),
    /// Active error constructor; the success type is supplied by context.
    ResultErr(Box<Expr>),
    /// Postfix propagation expression.
    Propagate(Box<Expr>),
    /// Call to a builtin function like `crypto::poseidon2(left: a, right: b)`.
    Call {
        name: String,
        args: Vec<Expr>,
        /// Source argument names in source order. `None` denotes an all-positional call.
        ///
        /// Method receivers are stored as `args[0]` and are deliberately excluded
        /// from this list because they are compiler-inserted rather than source
        /// arguments.
        argument_names: Option<Vec<String>>,
        /// Whether `args[0]` is the implicit receiver from source method syntax.
        implicit_receiver: bool,
    },
    /// Named source construction for a declared struct.
    StructLiteral {
        name: String,
        fields: Vec<StructLiteralField>,
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
    /// Bounded list literal. Its capacity is inferred from context or the
    /// exact number of elements.
    List(Vec<Expr>),
    /// Capacity-proven list comprehension:
    /// `[expression for item in source if condition]`.
    ListComprehension {
        expression: Box<Expr>,
        item: String,
        source: Box<Expr>,
        condition: Option<Box<Expr>>,
    },
    /// Native canonical JSON object construction.
    JsonObject(Vec<JsonObjectEntry>),
    /// Native canonical JSON array construction.
    JsonArray(Vec<Expr>),
    Bool(bool),
    Number(i64),
    /// Canonical text of an explicitly `u128`-suffixed integer literal.
    ///
    /// The historical variant name is internal; V1 source does not support
    /// decimal-fraction literals.
    Decimal(String),
    /// Exact source spelling of a non-negative decimal Amount literal.
    AmountLiteral(String),
    String(String),
    Bytes(Vec<u8>),
    Ident(String),
}

/// One source field in a named struct literal.
#[derive(Debug, PartialEq, Clone)]
pub struct StructLiteralField {
    /// Declared field name supplied by the source.
    pub name: String,
    /// Field value; shorthand fields contain `Expr::Ident(name)`.
    pub value: Expr,
    /// Whether the source used shorthand spelling without `: value`.
    pub shorthand: bool,
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

impl TypeExpr {
    /// Return this type's exact source range when it came from source text.
    #[must_use]
    pub const fn source(&self) -> Option<SourceRange> {
        match self {
            Self::Source { source, .. } => Some(*source),
            Self::Resolved { source, ty, .. } => match source {
                Some(source) => Some(*source),
                None => ty.source(),
            },
            _ => None,
        }
    }

    /// Return the stable resolved-HIR identity, when name resolution has run.
    #[must_use]
    pub const fn hir_id(&self) -> Option<HirId> {
        match self {
            Self::Resolved { id, .. } => Some(*id),
            Self::Source { ty, .. } => ty.hir_id(),
            _ => None,
        }
    }

    /// Return the parser-owned source identity before resolved-HIR lowering.
    #[must_use]
    pub const fn source_node(&self) -> Option<NodeId> {
        match self {
            Self::Source { node, .. } => Some(*node),
            Self::Resolved { ty, .. } => ty.source_node(),
            _ => None,
        }
    }

    /// View the semantic type form without its source wrapper.
    #[must_use]
    pub fn kind(&self) -> &Self {
        match self {
            Self::Source { ty, .. } => ty.kind(),
            Self::Resolved { ty, .. } => ty.kind(),
            _ => self,
        }
    }

    /// Consume the source wrapper and return the semantic type form.
    #[must_use]
    pub fn into_kind(self) -> Self {
        match self {
            Self::Source { ty, .. } => ty.into_kind(),
            Self::Resolved { ty, .. } => ty.into_kind(),
            _ => self,
        }
    }
}

impl Statement {
    /// Return this statement's exact source range when it came from source text.
    #[must_use]
    pub const fn source(&self) -> Option<SourceRange> {
        match self {
            Self::Source { source, .. } => Some(*source),
            Self::Resolved {
                source, statement, ..
            } => match source {
                Some(source) => Some(*source),
                None => statement.source(),
            },
            _ => None,
        }
    }

    /// Return the stable resolved-HIR identity, when name resolution has run.
    #[must_use]
    pub const fn hir_id(&self) -> Option<HirId> {
        match self {
            Self::Resolved { id, .. } => Some(*id),
            Self::Source { statement, .. } => statement.hir_id(),
            _ => None,
        }
    }

    /// Return the parser-owned source identity before resolved-HIR lowering.
    #[must_use]
    pub const fn source_node(&self) -> Option<NodeId> {
        match self {
            Self::Source { node, .. } => Some(*node),
            Self::Resolved { statement, .. } => statement.source_node(),
            _ => None,
        }
    }

    /// View the semantic statement form without its source wrapper.
    #[must_use]
    pub fn kind(&self) -> &Self {
        match self {
            Self::Source { statement, .. } => statement.kind(),
            Self::Resolved { statement, .. } => statement.kind(),
            _ => self,
        }
    }

    /// Consume the source wrapper and return the semantic statement form.
    #[must_use]
    pub fn into_kind(self) -> Self {
        match self {
            Self::Source { statement, .. } => statement.into_kind(),
            Self::Resolved { statement, .. } => statement.into_kind(),
            _ => self,
        }
    }
}

impl Expr {
    /// Return this expression's exact source range when it came from source text.
    #[must_use]
    pub const fn source(&self) -> Option<SourceRange> {
        match self {
            Self::Source { source, .. } => Some(*source),
            Self::Resolved {
                source, expression, ..
            } => match source {
                Some(source) => Some(*source),
                None => expression.source(),
            },
            _ => None,
        }
    }

    /// Return the stable resolved-HIR identity, when name resolution has run.
    #[must_use]
    pub const fn hir_id(&self) -> Option<HirId> {
        match self {
            Self::Resolved { id, .. } => Some(*id),
            Self::Source { expression, .. } => expression.hir_id(),
            _ => None,
        }
    }

    /// Return the parser-owned source identity before resolved-HIR lowering.
    #[must_use]
    pub const fn source_node(&self) -> Option<NodeId> {
        match self {
            Self::Source { node, .. } => Some(*node),
            Self::Resolved { expression, .. } => expression.source_node(),
            _ => None,
        }
    }

    /// View the semantic expression form without its source wrapper.
    #[must_use]
    pub fn kind(&self) -> &Self {
        match self {
            Self::Source { expression, .. } => expression.kind(),
            Self::Resolved { expression, .. } => expression.kind(),
            _ => self,
        }
    }

    /// Consume the source wrapper and return the semantic expression form.
    #[must_use]
    pub fn into_kind(self) -> Self {
        match self {
            Self::Source { expression, .. } => expression.into_kind(),
            Self::Resolved { expression, .. } => expression.into_kind(),
            _ => self,
        }
    }
}

#[derive(Clone, Copy)]
enum ProvenanceAction {
    Rebase(crate::source::SourceId),
    Strip,
}

fn normalize_type_provenance(ty: &mut TypeExpr, action: ProvenanceAction) -> &mut TypeExpr {
    match action {
        ProvenanceAction::Rebase(source_id) => {
            let mut current = ty;
            loop {
                match current {
                    TypeExpr::Source { source, ty, .. } => {
                        source.source = source_id;
                        current = ty;
                    }
                    TypeExpr::Resolved { source, ty, .. } => {
                        if let Some(source) = source {
                            source.source = source_id;
                        }
                        current = ty;
                    }
                    _ => return current,
                }
            }
        }
        ProvenanceAction::Strip => loop {
            let current = std::mem::replace(ty, TypeExpr::Const(0));
            match current {
                TypeExpr::Source { ty: inner, .. } | TypeExpr::Resolved { ty: inner, .. } => {
                    *ty = *inner;
                }
                current => {
                    *ty = current;
                    return ty;
                }
            }
        },
    }
}

fn normalize_statement_provenance(
    statement: &mut Statement,
    action: ProvenanceAction,
) -> &mut Statement {
    match action {
        ProvenanceAction::Rebase(source_id) => {
            let mut current = statement;
            loop {
                match current {
                    Statement::Source {
                        source, statement, ..
                    } => {
                        source.source = source_id;
                        current = statement;
                    }
                    Statement::Resolved {
                        source, statement, ..
                    } => {
                        if let Some(source) = source {
                            source.source = source_id;
                        }
                        current = statement;
                    }
                    _ => return current,
                }
            }
        }
        ProvenanceAction::Strip => loop {
            let current = std::mem::replace(statement, Statement::Break);
            match current {
                Statement::Source {
                    statement: inner, ..
                }
                | Statement::Resolved {
                    statement: inner, ..
                } => {
                    *statement = *inner;
                }
                current => {
                    *statement = current;
                    return statement;
                }
            }
        },
    }
}

fn normalize_expr_provenance(expression: &mut Expr, action: ProvenanceAction) -> &mut Expr {
    match action {
        ProvenanceAction::Rebase(source_id) => {
            let mut current = expression;
            loop {
                match current {
                    Expr::Source {
                        source, expression, ..
                    } => {
                        source.source = source_id;
                        current = expression;
                    }
                    Expr::Resolved {
                        source, expression, ..
                    } => {
                        if let Some(source) = source {
                            source.source = source_id;
                        }
                        current = expression;
                    }
                    _ => return current,
                }
            }
        }
        ProvenanceAction::Strip => loop {
            let current = std::mem::replace(expression, Expr::Number(0));
            match current {
                Expr::Source {
                    expression: inner, ..
                }
                | Expr::Resolved {
                    expression: inner, ..
                } => {
                    *expression = *inner;
                }
                current => {
                    *expression = current;
                    return expression;
                }
            }
        },
    }
}

fn transform_program_provenance(program: &mut Program, action: ProvenanceAction) {
    enum Pending<'a> {
        Type(&'a mut TypeExpr),
        Statement(&'a mut Statement),
        Expr(&'a mut Expr),
    }

    fn push_block<'a>(block: &'a mut Block, pending: &mut Vec<Pending<'a>>) {
        pending.extend(block.statements.iter_mut().map(Pending::Statement));
        if let Some(tail) = &mut block.tail {
            pending.push(Pending::Expr(tail));
        }
    }

    let mut pending = Vec::new();
    for item in &mut program.items {
        match item {
            Item::Function(function) => {
                pending.extend(
                    function
                        .params
                        .iter_mut()
                        .filter_map(|parameter| parameter.ty.as_mut())
                        .map(Pending::Type),
                );
                if let Some(ret_ty) = &mut function.ret_ty {
                    pending.push(Pending::Type(ret_ty));
                }
                push_block(&mut function.body, &mut pending);
            }
            Item::Struct(definition) => {
                pending.extend(
                    definition
                        .fields
                        .iter_mut()
                        .map(|(_, ty)| Pending::Type(ty)),
                );
            }
            Item::Const(declaration) => {
                if let Some(ty) = &mut declaration.ty {
                    pending.push(Pending::Type(ty));
                }
                pending.push(Pending::Expr(&mut declaration.value));
            }
            Item::State(declaration) => pending.push(Pending::Type(&mut declaration.ty)),
            Item::Trigger(declaration) => pending.extend(
                declaration
                    .metadata
                    .iter_mut()
                    .map(|entry| Pending::Expr(&mut entry.value)),
            ),
            Item::ErrorEnum(_) => {}
        }
    }
    for fixture in &mut program.fixtures {
        for fixture_action in &mut fixture.actions {
            pending.extend(fixture_action.args.iter_mut().map(Pending::Expr));
        }
    }

    while let Some(node) = pending.pop() {
        match node {
            Pending::Type(ty) => match normalize_type_provenance(ty, action) {
                TypeExpr::Generic { args, .. } | TypeExpr::Tuple(args) => {
                    pending.extend(args.iter_mut().map(Pending::Type));
                }
                TypeExpr::Path(_) | TypeExpr::Const(_) => {}
                TypeExpr::Source { .. } | TypeExpr::Resolved { .. } => {
                    unreachable!("provenance normalization returns the semantic node")
                }
            },
            Pending::Statement(statement) => {
                match normalize_statement_provenance(statement, action) {
                    Statement::Let { ty, value, .. } => {
                        if let Some(ty) = ty {
                            pending.push(Pending::Type(ty));
                        }
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
                        if let Some(value) = value {
                            pending.push(Pending::Expr(value));
                        }
                    }
                    Statement::If {
                        cond,
                        then_branch,
                        else_branch,
                    }
                    | Statement::IfLet {
                        value: cond,
                        then_branch,
                        else_branch,
                        ..
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
                        if let Some(init) = init {
                            pending.push(Pending::Statement(init));
                        }
                        if let Some(cond) = cond {
                            pending.push(Pending::Expr(cond));
                        }
                        if let Some(step) = step {
                            pending.push(Pending::Statement(step));
                        }
                        push_block(body, &mut pending);
                    }
                    Statement::ForEachMap { map, body, .. } => {
                        pending.push(Pending::Expr(map));
                        push_block(body, &mut pending);
                    }
                    Statement::Break | Statement::Continue => {}
                    Statement::Source { .. } | Statement::Resolved { .. } => {
                        unreachable!("provenance normalization returns the semantic node")
                    }
                }
            }
            Pending::Expr(expression) => match normalize_expr_provenance(expression, action) {
                Expr::Binary { left, right, .. }
                | Expr::Index {
                    target: left,
                    index: right,
                } => {
                    pending.push(Pending::Expr(left));
                    pending.push(Pending::Expr(right));
                }
                Expr::Unary { expr, .. }
                | Expr::Member { object: expr, .. }
                | Expr::OptionSome(expr)
                | Expr::ResultOk(expr)
                | Expr::ResultErr(expr)
                | Expr::Propagate(expr) => pending.push(Pending::Expr(expr)),
                Expr::Conditional {
                    cond,
                    then_expr,
                    else_expr,
                } => {
                    pending.push(Pending::Expr(cond));
                    pending.push(Pending::Expr(then_expr));
                    pending.push(Pending::Expr(else_expr));
                }
                Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                }
                | Expr::IfLet {
                    value: condition,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    pending.push(Pending::Expr(condition));
                    push_block(then_branch, &mut pending);
                    if let Some(block) = else_branch {
                        push_block(block, &mut pending);
                    }
                }
                Expr::Match { value, arms } => {
                    pending.push(Pending::Expr(value));
                    for arm in arms {
                        push_block(&mut arm.body, &mut pending);
                    }
                }
                Expr::Call { args, .. }
                | Expr::Tuple(args)
                | Expr::List(args)
                | Expr::JsonArray(args) => {
                    pending.extend(args.iter_mut().map(Pending::Expr));
                }
                Expr::ListComprehension {
                    expression,
                    source,
                    condition,
                    ..
                } => {
                    pending.push(Pending::Expr(expression));
                    pending.push(Pending::Expr(source));
                    if let Some(condition) = condition {
                        pending.push(Pending::Expr(condition));
                    }
                }
                Expr::StructLiteral { fields, .. } => pending.extend(
                    fields
                        .iter_mut()
                        .map(|field| Pending::Expr(&mut field.value)),
                ),
                Expr::JsonObject(entries) => pending.extend(
                    entries
                        .iter_mut()
                        .map(|entry| Pending::Expr(&mut entry.value)),
                ),
                Expr::Number(_)
                | Expr::Decimal(_)
                | Expr::AmountLiteral(_)
                | Expr::OptionNone
                | Expr::Bool(_)
                | Expr::String(_)
                | Expr::Bytes(_)
                | Expr::Ident(_) => {}
                Expr::Source { .. } | Expr::Resolved { .. } => {
                    unreachable!("provenance normalization returns the semantic node")
                }
            },
        }
    }
}

/// Rebase every embedded source range while preserving local NodeId/HirId identities.
pub(crate) fn rebase_program_source(program: &mut Program, source: crate::source::SourceId) {
    transform_program_provenance(program, ProvenanceAction::Rebase(source));
}

/// Remove compiler provenance wrappers for public syntax/tooling AST consumers.
pub(crate) fn strip_program_provenance(program: &mut Program) {
    transform_program_provenance(program, ProvenanceAction::Strip);
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
        pending.extend(
            block
                .tail
                .into_iter()
                .map(|expression| Pending::Expr(*expression)),
        );
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
            Pending::Type(TypeExpr::Source { ty, .. } | TypeExpr::Resolved { ty, .. }) => {
                pending.push(Pending::Type(*ty));
            }
            Pending::Type(TypeExpr::Generic { args, .. })
            | Pending::Type(TypeExpr::Tuple(args)) => {
                pending.extend(args.into_iter().map(Pending::Type));
            }
            Pending::Type(TypeExpr::Path(_) | TypeExpr::Const(_)) => {}
            Pending::Statement(statement) => match statement {
                Statement::Source { statement, .. } | Statement::Resolved { statement, .. } => {
                    pending.push(Pending::Statement(*statement));
                }
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
                Statement::IfLet {
                    value,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    pending.push(Pending::Expr(value));
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
                Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
                    pending.push(Pending::Expr(*expression));
                }
                Expr::Binary { left, right, .. }
                | Expr::Index {
                    target: left,
                    index: right,
                } => {
                    pending.push(Pending::Expr(*left));
                    pending.push(Pending::Expr(*right));
                }
                Expr::Unary { expr, .. }
                | Expr::Member { object: expr, .. }
                | Expr::OptionSome(expr)
                | Expr::ResultOk(expr)
                | Expr::ResultErr(expr)
                | Expr::Propagate(expr) => {
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
                Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(Pending::Expr(*condition));
                    push_block(then_branch, &mut pending);
                    if let Some(block) = else_branch {
                        push_block(block, &mut pending);
                    }
                }
                Expr::IfLet {
                    value,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    pending.push(Pending::Expr(*value));
                    push_block(then_branch, &mut pending);
                    if let Some(block) = else_branch {
                        push_block(block, &mut pending);
                    }
                }
                Expr::Match { value, arms } => {
                    pending.push(Pending::Expr(*value));
                    for arm in arms {
                        push_block(arm.body, &mut pending);
                    }
                }
                Expr::Call { args, .. } | Expr::Tuple(args) | Expr::List(args) => {
                    pending.extend(args.into_iter().map(Pending::Expr));
                }
                Expr::JsonObject(entries) => {
                    pending.extend(entries.into_iter().map(|entry| Pending::Expr(entry.value)))
                }
                Expr::JsonArray(elements) => {
                    pending.extend(elements.into_iter().map(Pending::Expr));
                }
                Expr::ListComprehension {
                    expression,
                    source,
                    condition,
                    ..
                } => {
                    pending.push(Pending::Expr(*expression));
                    pending.push(Pending::Expr(*source));
                    pending.extend(condition.map(|condition| Pending::Expr(*condition)));
                }
                Expr::StructLiteral { fields, .. } => {
                    pending.extend(fields.into_iter().map(|field| Pending::Expr(field.value)))
                }
                Expr::Bool(_)
                | Expr::Number(_)
                | Expr::Decimal(_)
                | Expr::AmountLiteral(_)
                | Expr::OptionNone
                | Expr::String(_)
                | Expr::Bytes(_)
                | Expr::Ident(_) => {}
            },
        }
    }
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    use crate::source::{SourceId, TextRange};

    fn deeply_sourced_program(depth: u32) -> Program {
        let old_source = SourceId(11);
        let mut expression = Expr::Number(1);
        for index in (0..depth).rev() {
            expression = Expr::Source {
                node: NodeId(index),
                source: SourceRange::new(old_source, TextRange::new(index, index + 1)),
                expression: Box::new(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expression),
                }),
            };
        }
        Program {
            unit: SourceUnit {
                kind: SourceUnitKind::Module,
                name: "Provenance".to_owned(),
            },
            items: vec![Item::Const(ConstDecl {
                name: "VALUE".to_owned(),
                ty: None,
                value: expression,
            })],
            test_target: None,
            fixtures: Vec::new(),
        }
    }

    #[test]
    fn full_depth_provenance_rebases_and_strips_iteratively() {
        let mut program = deeply_sourced_program(256);
        let new_source = SourceId(99);
        rebase_program_source(&mut program, new_source);

        let Item::Const(declaration) = &program.items[0] else {
            panic!("const declaration")
        };
        let mut current = &declaration.value;
        let mut wrappers = 0_u32;
        loop {
            match current {
                Expr::Source {
                    source, expression, ..
                } => {
                    assert_eq!(source.source, new_source);
                    wrappers += 1;
                    let Expr::Unary { expr, .. } = expression.as_ref() else {
                        panic!("nested unary")
                    };
                    current = expr;
                }
                Expr::Number(1) => break,
                other => panic!("unexpected nested expression: {other:?}"),
            }
        }
        assert_eq!(wrappers, 256);

        strip_program_provenance(&mut program);
        let Item::Const(declaration) = &program.items[0] else {
            panic!("const declaration")
        };
        let mut current = &declaration.value;
        for _ in 0..256 {
            let Expr::Unary { expr, .. } = current else {
                panic!("stripped unary")
            };
            current = expr;
        }
        assert!(matches!(current, Expr::Number(1)));
        drop_program_iterative(program);
    }
}
