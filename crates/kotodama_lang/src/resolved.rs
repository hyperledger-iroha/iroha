//! Fail-closed declaration, type, and call resolution for spanned Kotodama AST.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iroha_primitives::bigint::BigInt;

use crate::{
    ast::{
        Block, Expr, FunctionKind, HirId, Item, Pattern, PatternBinding, Program, Statement,
        SumPattern, TypeExpr,
    },
    builtins::Builtin,
    diagnostic::{Diagnostic, DiagnosticBundle, DiagnosticLabel, DiagnosticPhase, SourceSpan},
    source::{SourceFile, SourceRange, TextRange},
    spanned_ast::{
        AstFacts, AstNodeKind, AstSourceMap, BindingFact, BindingFactKind, DeclarationFact,
        DeclarationKind, NodeId, SpannedProgram, TypeUseFact,
    },
};

/// Stable identity of one resolved source declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(u32);

/// Stable identity of a lexical scope in resolved HIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(u32);

/// Stable identity of a parameter or local binding in resolved HIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(u32);

/// Declaration role retained by the resolved symbol arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedSymbolKind {
    /// The single source unit.
    SourceUnit,
    /// A function or lifecycle declaration.
    Function,
    /// A user-defined struct.
    Struct,
    /// A declared error-code namespace.
    ErrorEnum,
    /// A durable state declaration.
    State,
    /// A typed constant declaration.
    Const,
    /// A trigger declaration.
    Trigger,
}

/// Resolved declaration retained before semantic typing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSymbol {
    /// Stable symbol identity.
    pub id: SymbolId,
    /// Source node declaring the symbol.
    pub node: NodeId,
    /// Exact source range of the declared name.
    pub source: crate::source::SourceRange,
    /// Declared spelling.
    pub name: String,
    /// Declaration role.
    pub kind: ResolvedSymbolKind,
}

/// Lexical binding role retained before type checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedBindingKind {
    /// Function parameter.
    Parameter,
    /// `let` or `var` declaration.
    Local,
    /// Sum-pattern payload declaration.
    Pattern,
    /// Bounded-loop iterator declaration.
    Iterator,
    /// List-comprehension item declaration.
    Comprehension,
}

/// One parameter or local declaration with a stable lexical identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBinding {
    /// Stable binding identity.
    pub id: BindingId,
    /// Scope that owns the binding.
    pub scope: ScopeId,
    /// Source spelling.
    pub name: String,
    /// Binding role.
    pub kind: ResolvedBindingKind,
    /// Exact declaration range when source-backed.
    pub source: Option<SourceRange>,
    /// Exact parser-owned name token when source-backed.
    pub source_node: Option<NodeId>,
    /// Whether assignment is permitted.
    pub mutable: bool,
}

/// Target selected for one value-name use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedValueTarget {
    /// Parameter or local binding.
    Binding(BindingId),
    /// Durable state declaration.
    State(SymbolId),
    /// Source constant declaration.
    Const(SymbolId),
    /// Stable declared error code.
    ErrorCode(u32),
    /// Compiler-owned value such as a rounding mode or JSON null.
    Intrinsic,
    /// State supplied by an explicitly typed standalone-test target.
    ExternalState,
    /// Constant supplied by an explicitly typed standalone-test target.
    ExternalConst,
}

/// Target selected for one source type reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedTypeTarget {
    /// Compiler-owned scalar, aggregate, or Iroha boundary type.
    Builtin,
    /// User-defined struct declaration.
    Struct(SymbolId),
    /// Struct supplied by an explicitly typed standalone-test target.
    ExternalStruct,
}

/// One resolved named type use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTypeUse {
    /// Exact source node of the type name.
    pub node: NodeId,
    /// Exact source range of the named type use.
    pub source: crate::source::SourceRange,
    /// Owning function declaration, when the use occurs in a function.
    pub owner: Option<NodeId>,
    /// Source spelling.
    pub name: String,
    /// Bound type declaration.
    pub target: ResolvedTypeTarget,
}

/// Target selected for one source call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedCallTarget {
    /// User-defined function declaration.
    Function(SymbolId),
    /// Canonical builtin registry entry.
    Builtin(Builtin),
    /// Receiver-typed method resolved during semantic typing.
    Method,
    /// Explicit compiler-owned numeric/sum intrinsic.
    Intrinsic,
    /// User-defined struct referenced with retired positional syntax.
    Struct(SymbolId),
    /// Explicit import-alias call whose export is bound by the typed linker.
    External,
}

/// Authoritative target attached to one resolved-HIR node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedTarget {
    /// Named type reference.
    Type(ResolvedTypeTarget),
    /// Function, builtin, intrinsic, or method call.
    Call(ResolvedCallTarget),
    /// Value-name use.
    Value(ResolvedValueTarget),
    /// Named struct literal.
    StructLiteral(SymbolId),
    /// Named struct literal supplied by an explicitly typed standalone-test target.
    ExternalStructLiteral,
    /// Simple named assignment target.
    Assignment(ResolvedValueTarget),
}

/// Coarse resolved-HIR node category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedNodeKind {
    /// Type expression.
    Type,
    /// Statement.
    Statement,
    /// Expression.
    Expression,
}

/// One stable node in the native resolved-HIR arena.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedNode {
    /// Stable identity embedded in the resolved tree.
    pub id: HirId,
    /// Lexical scope containing the node.
    pub scope: ScopeId,
    /// Exact source range when source-backed.
    pub source: Option<SourceRange>,
    /// Stable CST/AST source identity when source-backed.
    pub source_node: Option<NodeId>,
    /// Node category.
    pub kind: ResolvedNodeKind,
    /// Named target, if this node performs name resolution.
    pub target: Option<ResolvedTarget>,
    /// Bindings declared by this node, in source order.
    pub bindings: Vec<BindingId>,
}

/// One lexical scope in resolved HIR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedScope {
    /// Stable scope identity.
    pub id: ScopeId,
    /// Enclosing scope, absent for the source-unit root.
    pub parent: Option<ScopeId>,
}

/// Immutable resolver output consulted by semantic typing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedArena {
    source: crate::source::SourceId,
    nodes: Vec<ResolvedNode>,
    scopes: Vec<ResolvedScope>,
    bindings: Vec<ResolvedBinding>,
    symbols: Vec<ResolvedSymbol>,
}

impl ResolvedArena {
    pub(crate) const fn source(&self) -> crate::source::SourceId {
        self.source
    }

    pub(crate) fn node(&self, id: HirId) -> Option<&ResolvedNode> {
        self.nodes
            .get(usize::try_from(id.0).ok()?)
            .filter(|node| node.id == id)
    }

    pub(crate) fn binding(&self, id: BindingId) -> Option<&ResolvedBinding> {
        self.bindings
            .get(usize::try_from(id.0).ok()?)
            .filter(|binding| binding.id == id)
    }

    pub(crate) fn nodes(&self) -> impl ExactSizeIterator<Item = &ResolvedNode> {
        self.nodes.iter()
    }

    pub(crate) fn bindings(&self) -> impl ExactSizeIterator<Item = &ResolvedBinding> {
        self.bindings.iter()
    }

    pub(crate) fn symbol(&self, id: SymbolId) -> Option<&ResolvedSymbol> {
        self.symbols
            .get(usize::try_from(id.0).ok()?)
            .filter(|symbol| symbol.id == id)
    }

    pub(crate) fn binding_visible_at(&self, binding: BindingId, node: HirId) -> bool {
        let Some(binding) = self.binding(binding) else {
            return false;
        };
        let Some(node) = self.node(node) else {
            return false;
        };
        let mut scope = Some(node.scope);
        while let Some(current) = scope {
            if current == binding.scope {
                return true;
            }
            let Some(index) = usize::try_from(current.0).ok() else {
                return false;
            };
            scope = self
                .scopes
                .get(index)
                .filter(|entry| entry.id == current)
                .and_then(|entry| entry.parent);
        }
        false
    }
}

/// One resolved source call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedCall {
    /// Exact complete call node.
    pub node: NodeId,
    /// Exact call-name node.
    pub name_node: NodeId,
    /// Exact source range of the complete call expression.
    pub source: crate::source::SourceRange,
    /// Exact source range of the called name.
    pub name_source: crate::source::SourceRange,
    /// Owning function declaration, when the call occurs in a function.
    pub owner: Option<NodeId>,
    /// Source spelling.
    pub name: String,
    /// Bound call target.
    pub target: ResolvedCallTarget,
}

/// Distinct resolved HIR consumed by canonical semantic typing.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedProgram {
    program: Program,
    facts: AstFacts,
    source_file: SourceFile,
    symbols: Vec<ResolvedSymbol>,
    types: Vec<ResolvedTypeUse>,
    calls: Vec<ResolvedCall>,
    arena: Arc<ResolvedArena>,
}

impl ResolvedProgram {
    /// Return the source AST after fail-closed resolution.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Return the stable source-node arena.
    #[must_use]
    pub const fn source_map(&self) -> &AstSourceMap {
        &self.facts.source_map
    }

    /// Return the immutable target/scope/binding arena required by typing.
    pub(crate) fn arena(&self) -> Arc<ResolvedArena> {
        Arc::clone(&self.arena)
    }

    /// Return resolved declarations.
    pub fn symbols(&self) -> impl ExactSizeIterator<Item = &ResolvedSymbol> {
        self.symbols.iter()
    }

    /// Return resolved named type uses.
    pub fn types(&self) -> impl ExactSizeIterator<Item = &ResolvedTypeUse> {
        self.types.iter()
    }

    /// Return resolved source calls.
    pub fn calls(&self) -> impl ExactSizeIterator<Item = &ResolvedCall> {
        self.calls.iter()
    }

    /// Return stable parameter/local bindings in resolver allocation order.
    pub fn bindings(&self) -> impl ExactSizeIterator<Item = &ResolvedBinding> {
        self.arena.bindings()
    }

    /// Return the exact declared parameter-name range for one source function.
    pub(crate) fn parameter_name_source(
        &self,
        function_name: &str,
        parameter_name: &str,
    ) -> Option<SourceRange> {
        let owner = self
            .facts
            .declarations
            .iter()
            .find(|fact| fact.kind == DeclarationKind::Function && fact.name == function_name)?
            .node;
        self.facts
            .declarations
            .iter()
            .find(|fact| {
                fact.kind == DeclarationKind::Parameter
                    && fact.owner == Some(owner)
                    && fact.name == parameter_name
            })
            .and_then(|fact| self.facts.source_map.source_range(fact.name_node))
    }

    /// Return the exact lifecycle-name range for the source `hajimari` declaration.
    pub(crate) fn hajimari_name_source(&self) -> Option<SourceRange> {
        let name = self.program.items.iter().find_map(|item| {
            let Item::Function(function) = item else {
                return None;
            };
            (function.modifiers.kind == FunctionKind::Hajimari).then_some(&function.name)
        })?;
        self.facts
            .declarations
            .iter()
            .find(|fact| fact.kind == DeclarationKind::Function && &fact.name == name)
            .and_then(|fact| self.facts.source_map.source_range(fact.name_node))
    }

    /// Return the exact `state` keyword range of the first scalar state declaration.
    pub(crate) fn first_scalar_state_keyword_source(&self) -> Option<SourceRange> {
        let name = self.program.items.iter().find_map(|item| {
            let Item::State(state) = item else {
                return None;
            };
            (!matches!(
                state.ty.kind(),
                TypeExpr::Generic { base, .. } if base == "StateMap"
            ))
            .then_some(&state.name)
        })?;
        let declaration = self
            .facts
            .declarations
            .iter()
            .find(|fact| fact.kind == DeclarationKind::State && &fact.name == name)?;
        let declaration = self.facts.source_map.source_range(declaration.node)?;
        let keyword_end = declaration.range.start.checked_add(5)?;
        let keyword = TextRange::new(declaration.range.start, keyword_end);
        (keyword.end <= declaration.range.end && self.source_file.slice(keyword) == Some("state"))
            .then_some(SourceRange::new(declaration.source, keyword))
    }

    pub(crate) fn into_program(self) -> Program {
        let mut program = self.program;
        crate::ast::strip_program_provenance(&mut program);
        program
    }

    pub(crate) fn attach_sources(&self, typed: &mut crate::semantic::TypedProgram) {
        typed
            .source_files
            .insert(self.source_map().source(), self.source_file.clone());
        for item in &mut typed.items {
            let crate::semantic::TypedItem::Function(function) = item;
            let Some(declaration) =
                self.facts.declarations.iter().find(|fact| {
                    fact.kind == DeclarationKind::Function && fact.name == function.name
                })
            else {
                continue;
            };
            let source_map = &self.facts.source_map;
            let Some(declaration_range) = source_map.source_range(declaration.node) else {
                continue;
            };
            let Some(name_range) = source_map.source_range(declaration.name_node) else {
                continue;
            };
            function.source = Some(declaration_range);
            function.name_source = Some(name_range);
        }
        for state in &mut typed.states {
            state.source = self
                .facts
                .declarations
                .iter()
                .find(|fact| fact.kind == DeclarationKind::State && fact.name == state.name)
                .and_then(|fact| self.facts.source_map.source_range(fact.node));
        }
    }

    pub(crate) fn span_for_location(
        &self,
        source: &SourceFile,
        line: usize,
        column: usize,
    ) -> Option<SourceSpan> {
        self.facts
            .declarations
            .iter()
            .filter(|fact| fact.kind == DeclarationKind::Function)
            .find_map(|fact| {
                let span = self.facts.source_map.source_span(source, fact.name_node)?;
                (span.start.line == line && span.start.column == column).then_some(span)
            })
    }
}

fn symbol_id(index: usize) -> SymbolId {
    SymbolId(u32::try_from(index).expect("symbol budget fits u32"))
}

fn symbol_kind(kind: DeclarationKind) -> Option<ResolvedSymbolKind> {
    Some(match kind {
        DeclarationKind::SourceUnit => ResolvedSymbolKind::SourceUnit,
        DeclarationKind::Function => ResolvedSymbolKind::Function,
        DeclarationKind::Struct => ResolvedSymbolKind::Struct,
        DeclarationKind::ErrorEnum => ResolvedSymbolKind::ErrorEnum,
        DeclarationKind::State => ResolvedSymbolKind::State,
        DeclarationKind::Const => ResolvedSymbolKind::Const,
        DeclarationKind::Trigger => ResolvedSymbolKind::Trigger,
        DeclarationKind::Parameter => return None,
    })
}

fn declaration_span(
    ast: &SpannedProgram,
    source: &SourceFile,
    fact: &DeclarationFact,
) -> Option<SourceSpan> {
    ast.facts.source_map.source_span(source, fact.name_node)
}

fn duplicate_diagnostic(
    ast: &SpannedProgram,
    source: &SourceFile,
    current: &DeclarationFact,
    previous: &DeclarationFact,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E_DUPLICATE_DECLARATION",
        DiagnosticPhase::Resolve,
        format!(
            "declaration name `{}` is already used by a {}",
            current.name,
            previous.kind.description()
        ),
        declaration_span(ast, source, current),
    );
    if let Some(span) = declaration_span(ast, source, previous) {
        diagnostic.labels.push(DiagnosticLabel {
            span,
            message: "first declaration is here".to_owned(),
        });
    }
    diagnostic
}

fn builtin_type(name: &str) -> bool {
    crate::semantic::V1_SOURCE_TYPE_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast::{Expr, Item, Statement},
        source::{FrontendBudget, SourceId},
        spanned_ast::AstNodeKind,
    };

    fn primary_spellings(source: &SourceFile, diagnostics: &DiagnosticBundle) -> Vec<String> {
        diagnostics
            .diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.primary_span.as_ref())
            .filter_map(|span| span.byte_range)
            .map(|range| {
                assert!(
                    !range.is_empty(),
                    "resolver spans must never be fabricated empties"
                );
                source
                    .slice(range)
                    .expect("diagnostic range belongs to its source")
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn identical_spellings_keep_distinct_cst_ranges() {
        let text = r#"
seiyaku Same {
    struct Packet { Missing left; Missing right; }
    fn repeated(Missing first, int first) {
        absent();
        absent();
    }
    fn repeated() {}
}
"#;
        let source = SourceFile::new(SourceId(37), "same_tokens.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("the adversarial source is syntactically valid");
        assert_eq!(ast.facts.source_map.source(), SourceId(37));

        let diagnostics = resolve(ast, &source).expect_err("resolution must fail closed");
        assert_eq!(
            primary_spellings(&source, &diagnostics),
            [
                "Missing", "Missing", "Missing", "first", "absent", "absent", "repeated"
            ]
        );

        let missing_ranges = diagnostics
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message == "unknown type `Missing`")
            .map(|diagnostic| {
                diagnostic
                    .primary_span
                    .as_ref()
                    .and_then(|span| span.byte_range)
                    .expect("unknown type has an exact range")
            })
            .collect::<Vec<_>>();
        assert_eq!(missing_ranges.len(), 3);
        assert!(
            missing_ranges
                .windows(2)
                .all(|ranges| ranges[0] < ranges[1])
        );

        let absent_ranges = diagnostics
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("`absent`"))
            .map(|diagnostic| {
                diagnostic
                    .primary_span
                    .as_ref()
                    .and_then(|span| span.byte_range)
                    .expect("unknown call has an exact range")
            })
            .collect::<Vec<_>>();
        assert_eq!(absent_ranges.len(), 2);
        assert_ne!(absent_ranges[0], absent_ranges[1]);
    }

    #[test]
    fn diagnostic_targets_bind_to_exact_nested_source_nodes() {
        let text = r#"
module Origins {
    fn mutate(List<int, 4> values, int outer, int inner) {
        values[outer][inner] = 1;
        let selected = values[inner];
        let quantity price = 1.250_0;
        let copy = [item for item in values if true];
    }
}
"#;
        let source = SourceFile::new(SourceId(41), "origins.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("diagnostic-target source parses");
        let Item::Function(function) = &ast.program.items[0] else {
            panic!("function item")
        };

        let Statement::AssignExpr { target, .. } = function.body.statements[0].kind() else {
            panic!("indexed assignment")
        };
        let outer_id = target.source_node().expect("outer index source identity");
        let outer_node = ast
            .facts
            .source_map
            .node(outer_id)
            .expect("outer index node");
        assert_eq!(outer_node.kind, AstNodeKind::IndexExpression);
        assert_eq!(source.slice(outer_node.range), Some("values[outer][inner]"));
        let Expr::Index {
            target: inner_target,
            ..
        } = target.kind()
        else {
            panic!("outer index expression")
        };
        let inner_id = inner_target
            .source_node()
            .expect("inner lvalue index source identity");
        let inner_node = ast
            .facts
            .source_map
            .node(inner_id)
            .expect("inner index node");
        assert_eq!(inner_node.kind, AstNodeKind::IndexExpression);
        assert_eq!(source.slice(inner_node.range), Some("values[outer]"));
        let Expr::Index {
            target: receiver, ..
        } = inner_target.kind()
        else {
            panic!("inner index expression")
        };
        let receiver_id = receiver
            .source_node()
            .expect("exact lvalue receiver source identity");
        let receiver_node = ast
            .facts
            .source_map
            .node(receiver_id)
            .expect("lvalue receiver node");
        assert_eq!(source.slice(receiver_node.range), Some("values"));

        let Statement::Let {
            value: read_index, ..
        } = function.body.statements[1].kind()
        else {
            panic!("read index binding")
        };
        let Expr::Index {
            target: read_receiver,
            ..
        } = read_index.kind()
        else {
            panic!("read index expression")
        };
        let read_index_node = ast
            .facts
            .source_map
            .node(
                read_index
                    .source_node()
                    .expect("read index source identity"),
            )
            .expect("read index node");
        assert_eq!(source.slice(read_index_node.range), Some("values[inner]"));
        let read_receiver_node = ast
            .facts
            .source_map
            .node(
                read_receiver
                    .source_node()
                    .expect("read receiver source identity"),
            )
            .expect("read receiver node");
        assert_eq!(source.slice(read_receiver_node.range), Some("values"));

        let Statement::Let { value: amount, .. } = function.body.statements[2].kind() else {
            panic!("quantity binding")
        };
        let amount_id = amount
            .source_node()
            .expect("quantity literal source identity");
        let amount_node = ast.facts.source_map.node(amount_id).expect("quantity node");
        assert_eq!(amount_node.kind, AstNodeKind::DecimalLiteral);
        assert_eq!(source.slice(amount_node.range), Some("1.250_0"));

        let Statement::Let {
            value: comprehension,
            ..
        } = function.body.statements[3].kind()
        else {
            panic!("comprehension binding")
        };
        let comprehension_id = comprehension
            .source_node()
            .expect("comprehension source identity");
        let comprehension_node = ast
            .facts
            .source_map
            .node(comprehension_id)
            .expect("comprehension node");
        assert_eq!(comprehension_node.kind, AstNodeKind::ListComprehension);
        assert_eq!(
            source.slice(comprehension_node.range),
            Some("[item for item in values if true]")
        );
    }

    #[test]
    fn successful_resolution_retains_declarations_types_and_calls() {
        let text = r#"
誓約 Sample {
    struct Pair { int left; int right; }
    fn helper(int value) -> int { value }
    言挙げ fn run(int value) -> int authorize("CanRun") {
        helper(value)
    }
}
"#;
        let source = SourceFile::new(SourceId(9), "japanese.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("Japanese declaration spellings parse");
        let resolved = resolve(ast, &source).expect("all named references resolve");

        assert_eq!(resolved.source_map().source(), SourceId(9));
        assert!(resolved.symbols().any(|symbol| symbol.name == "helper"));
        assert!(resolved.types().all(|ty| ty.name == "int"));
        assert!(resolved.calls().any(|call| {
            call.name == "helper" && matches!(call.target, ResolvedCallTarget::Function(_))
        }));
        let parameter = resolved
            .parameter_name_source("helper", "value")
            .expect("parameter name source");
        assert_eq!(source.slice(parameter.range), Some("value"));
    }

    #[test]
    fn parser_binding_facts_have_direct_owners_and_exact_utf8_ranges() {
        let text = r#"
誓約 ExactFacts {
    fn inspect(Option<int> input, List<int, 4> values) {
        // 雪 before every repeated ASCII binding makes character and byte offsets differ.
        let int repeated = 1;
        if let Option::some(repeated) = input {
            let int repeated = 2;
        }
        let List<int, 4> mapped = [repeated for repeated in values if true];
        match input {
            Option::some(repeated) => { repeated },
            Option::none => { 0 },
        };
    }
}
"#;
        let source = SourceFile::new(SourceId(75), "utf8-binding-facts.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("adversarial repeated-name source parses");

        let mut owner_ordinals = BTreeSet::new();
        let mut name_nodes = BTreeSet::new();
        let mut repeated_ranges = Vec::new();
        for fact in &ast.facts.bindings {
            assert!(
                owner_ordinals.insert((fact.owner, fact.ordinal)),
                "binding owner/ordinal pairs must be unique"
            );
            assert!(
                name_nodes.insert(fact.name_node),
                "each binding must own a distinct exact name token"
            );
            let owner = ast
                .facts
                .source_map
                .node(fact.owner)
                .expect("direct binding owner");
            let name = ast
                .facts
                .source_map
                .node(fact.name_node)
                .expect("binding name token");
            assert_eq!(name.kind, AstNodeKind::Name);
            assert!(owner.range.contains(name.range));
            assert_eq!(source.slice(name.range), Some(fact.name.as_str()));
            if fact.name == "repeated" {
                repeated_ranges.push(name.range);
            }
            match fact.kind {
                BindingFactKind::Local | BindingFactKind::Iterator => {
                    assert_eq!(owner.kind, AstNodeKind::Statement)
                }
                BindingFactKind::Pattern => assert!(matches!(
                    owner.kind,
                    AstNodeKind::Statement | AstNodeKind::Expression
                )),
                BindingFactKind::Comprehension => {
                    assert_eq!(owner.kind, AstNodeKind::ListComprehension)
                }
            }
        }

        assert_eq!(repeated_ranges.len(), 5);
        repeated_ranges.sort_unstable();
        assert!(
            repeated_ranges.windows(2).all(|pair| pair[0] < pair[1]),
            "identical spellings must retain distinct source-token ranges"
        );
        let first = repeated_ranges[0];
        assert_eq!(source.slice(first), Some("repeated"));
        assert_ne!(
            usize::try_from(first.start).expect("source budget"),
            text[..usize::try_from(first.start).expect("source budget")]
                .chars()
                .count(),
            "the fixture must exercise UTF-8 byte offsets, not ASCII-only offsets"
        );
    }

    #[test]
    fn parenthesized_if_let_keeps_its_direct_binding_owner_when_becoming_a_statement() {
        let text = r#"
module Parenthesized {
    fn inspect(Option<int> input) {
        (if let Option::some(payload) = input { return; })
    }
}
"#;
        let source = SourceFile::new(SourceId(81), "parenthesized-if-let.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("parenthesized if-let statement parses");
        let fact = ast
            .facts
            .bindings
            .iter()
            .find(|fact| fact.name == "payload")
            .expect("payload binding fact");
        assert_eq!(
            ast.facts
                .source_map
                .node(fact.owner)
                .map(|owner| owner.kind),
            Some(AstNodeKind::Statement)
        );
        let name = ast
            .facts
            .source_map
            .node(fact.name_node)
            .expect("payload name token");
        assert_eq!(source.slice(name.range), Some("payload"));
        resolve(ast, &source).expect("direct binding owner survives statement conversion");
    }

    fn binding_fact_fixture() -> (SourceFile, SpannedProgram) {
        let text = r#"
誓約 BindingIntegrity {
    fn inspect(Option<int> input, List<int, 4> values) -> int {
        let int base = 1;
        if let Option::some(payload) = input { return payload; }
        if let Option::some(payload) = input { return payload; }
        let List<int, 4> mapped = [item for item in values if true];
        return base;
    }
}
"#;
        let source = SourceFile::new(SourceId(76), "binding-integrity.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("binding-integrity fixture parses");
        (source, ast)
    }

    fn assert_binding_fact_corruption_fails(source: &SourceFile, ast: SpannedProgram) {
        let diagnostics = resolve(ast, source).expect_err("corrupt binding facts must fail closed");
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "K2099"),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn resolver_rejects_missing_duplicate_and_mismatched_binding_facts() {
        let (source, original) = binding_fact_fixture();
        resolve(original.clone(), &source).expect("uncorrupted binding facts resolve");
        assert!(original.facts.bindings.len() >= 5);

        let mut missing = original.clone();
        missing.facts.bindings.remove(0);
        assert_binding_fact_corruption_fails(&source, missing);

        let mut duplicate = original.clone();
        duplicate
            .facts
            .bindings
            .push(duplicate.facts.bindings[0].clone());
        assert_binding_fact_corruption_fails(&source, duplicate);

        let mut wrong_owner = original.clone();
        wrong_owner.facts.bindings[0].owner = wrong_owner.facts.declarations[0].node;
        assert_binding_fact_corruption_fails(&source, wrong_owner);

        let mut wrong_ordinal = original.clone();
        wrong_ordinal.facts.bindings[0].ordinal = u16::MAX;
        assert_binding_fact_corruption_fails(&source, wrong_ordinal);

        let mut wrong_role = original.clone();
        wrong_role.facts.bindings[0].kind = BindingFactKind::Iterator;
        assert_binding_fact_corruption_fails(&source, wrong_role);

        let mut wrong_spelling = original.clone();
        wrong_spelling.facts.bindings[0].name = "forged".to_owned();
        assert_binding_fact_corruption_fails(&source, wrong_spelling);

        let mut wrong_name_node = original.clone();
        wrong_name_node.facts.bindings[0].name_node = wrong_name_node.facts.bindings[0].owner;
        assert_binding_fact_corruption_fails(&source, wrong_name_node);

        let mut reused_name_node = original;
        let payload_indices = reused_name_node
            .facts
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(index, fact)| (fact.name == "payload").then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(payload_indices.len(), 2);
        reused_name_node.facts.bindings[payload_indices[1]].name_node =
            reused_name_node.facts.bindings[payload_indices[0]].name_node;
        assert_binding_fact_corruption_fails(&source, reused_name_node);
    }

    #[test]
    fn resolver_rejects_mismatched_source_identity_even_for_an_empty_module() {
        let parsed_source = SourceFile::new(SourceId(78), "empty.ko", "module Empty {}");
        let (ast, _) = crate::parser::parse_source_spanned(&parsed_source, FrontendBudget::v1())
            .expect("empty module parses");
        let different_identity = SourceFile::new(SourceId(79), "empty.ko", "module Empty {}");
        let diagnostics =
            resolve(ast, &different_identity).expect_err("source identity mismatch must fail");
        assert_eq!(diagnostics.diagnostics.len(), 1);
        assert_eq!(diagnostics.diagnostics[0].code, "K2099");
    }

    #[test]
    fn shadowing_diagnostic_labels_both_exact_names_after_utf8_prefix() {
        let text = "誓約 Shadow { fn run(int value) { /* 雪 */ let int value = 1; } }";
        let source = SourceFile::new(SourceId(77), "utf8-shadow.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("shadowing source parses");
        let diagnostics = resolve(ast, &source).expect_err("shadowing must be rejected");
        let diagnostic = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E_LOCAL_SHADOWING")
            .expect("shadowing diagnostic");
        let primary = diagnostic
            .primary_span
            .as_ref()
            .and_then(|span| span.byte_range)
            .expect("exact shadowing name range");
        let previous = diagnostic.labels[0]
            .span
            .byte_range
            .expect("exact previous name range");
        assert_eq!(source.slice(primary), Some("value"));
        assert_eq!(source.slice(previous), Some("value"));
        assert_ne!(primary, previous);
        assert_eq!(
            usize::try_from(primary.start).expect("source budget"),
            text.rfind("value").expect("local name byte offset")
        );
        assert_eq!(
            usize::try_from(previous.start).expect("source budget"),
            text.find("value").expect("parameter name byte offset")
        );
    }

    #[test]
    fn state_and_lifecycle_diagnostic_ranges_come_from_resolved_nodes() {
        let text = r#"
seiyaku Init {
    state StateMap<AccountId, int> entries;
    state int count;
    始まり() { count = 0; }
}
"#;
        let source = SourceFile::new(SourceId(11), "state.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("state source parses");
        let resolved = resolve(ast, &source).expect("state source resolves");

        let state = resolved
            .first_scalar_state_keyword_source()
            .expect("scalar state keyword source");
        assert_eq!(source.slice(state.range), Some("state"));
        let lifecycle = resolved
            .hajimari_name_source()
            .expect("hajimari name source");
        assert_eq!(source.slice(lifecycle.range), Some("始まり"));
    }

    fn resolved_local_program() -> (SourceFile, ResolvedProgram) {
        let text = r#"
seiyaku Stable {
    fn helper(int value) -> int {
        let int copy = value;
        return copy;
    }
    view fn read(int value) -> int {
        return helper(value: value);
    }
}
"#;
        let source = SourceFile::new(SourceId(73), "stable.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("stable local source parses");
        let resolved = resolve(ast, &source).expect("stable local source resolves");
        (source, resolved)
    }

    fn helper_return(program: &mut ResolvedProgram) -> &mut Expr {
        let Item::Function(function) = &mut program.program.items[0] else {
            panic!("helper function")
        };
        let Statement::Resolved { statement, .. } = &mut function.body.statements[1] else {
            panic!("resolved return statement")
        };
        let Statement::Return(Some(expression)) = statement.as_mut() else {
            panic!("return expression")
        };
        expression
    }

    fn helper_statement(program: &mut ResolvedProgram, index: usize) -> &mut Statement {
        let Item::Function(function) = &mut program.program.items[0] else {
            panic!("helper function")
        };
        function
            .body
            .statements
            .get_mut(index)
            .expect("helper statement index")
    }

    fn assert_internal_resolution_failure(program: &ResolvedProgram) {
        let failures = crate::semantic::SemanticContext::new()
            .analyze_resolved(program)
            .expect_err("corrupted resolved HIR must fail closed");
        assert!(
            failures
                .failures
                .iter()
                .any(|failure| failure.error.code == "E_INTERNAL_RESOLUTION"),
            "unexpected failures: {failures:?}"
        );
    }

    #[test]
    fn resolved_bindings_and_targets_survive_clone_and_move() {
        let (_source, resolved) = resolved_local_program();
        let cloned = resolved.clone();
        let moved = std::hint::black_box(cloned);
        assert_eq!(resolved.arena, moved.arena);
        assert!(moved.bindings().count() >= 3);
        assert!(moved.arena.nodes().any(|node| {
            matches!(
                node.target,
                Some(ResolvedTarget::Value(ResolvedValueTarget::Binding(_)))
            )
        }));
        let typed = crate::semantic::SemanticContext::new()
            .analyze_resolved(&moved)
            .expect("moved resolved HIR types without pointer rebinding");
        assert!(!typed.hir_nodes.is_empty());
    }

    #[test]
    fn semantic_rejects_corrupted_hir_id_kind_source_and_target() {
        let (_source, resolved) = resolved_local_program();
        let original_id = helper_return(&mut resolved.clone())
            .hir_id()
            .expect("return value HIR id");

        let mut missing_id = resolved.clone();
        let Expr::Resolved { id, .. } = helper_return(&mut missing_id) else {
            panic!("resolved expression")
        };
        *id = HirId(u32::MAX);
        assert_internal_resolution_failure(&missing_id);

        let mut wrong_kind = resolved.clone();
        Arc::make_mut(&mut wrong_kind.arena)
            .nodes
            .get_mut(original_id.0 as usize)
            .expect("return node")
            .kind = ResolvedNodeKind::Statement;
        assert_internal_resolution_failure(&wrong_kind);

        let mut wrong_source = resolved.clone();
        let Expr::Resolved { source, .. } = helper_return(&mut wrong_source) else {
            panic!("resolved expression")
        };
        let range = source.as_mut().expect("source-backed return value");
        range.range.end = range.range.end.saturating_sub(1);
        assert_internal_resolution_failure(&wrong_source);

        let mut missing_target = resolved.clone();
        Arc::make_mut(&mut missing_target.arena)
            .nodes
            .get_mut(original_id.0 as usize)
            .expect("return node")
            .target = None;
        assert_internal_resolution_failure(&missing_target);

        let mut wrong_target = resolved;
        Arc::make_mut(&mut wrong_target.arena)
            .nodes
            .get_mut(original_id.0 as usize)
            .expect("return node")
            .target = Some(ResolvedTarget::Call(ResolvedCallTarget::Intrinsic));
        assert_internal_resolution_failure(&wrong_target);
    }

    #[test]
    fn semantic_rejects_missing_or_corrupted_statement_hir_wrappers() {
        let (_source, resolved) = resolved_local_program();

        let mut missing_return = resolved.clone();
        let current = std::mem::replace(helper_statement(&mut missing_return, 1), Statement::Break);
        let Statement::Resolved { statement, .. } = current else {
            panic!("resolved return statement")
        };
        *helper_statement(&mut missing_return, 1) = *statement;
        assert_internal_resolution_failure(&missing_return);

        let mut missing_let = resolved.clone();
        let current = std::mem::replace(helper_statement(&mut missing_let, 0), Statement::Break);
        let Statement::Resolved { statement, .. } = current else {
            panic!("resolved let statement")
        };
        *helper_statement(&mut missing_let, 0) = *statement;
        assert_internal_resolution_failure(&missing_let);

        let mut wrong_id = resolved.clone();
        let Statement::Resolved { id, .. } = helper_statement(&mut wrong_id, 0) else {
            panic!("resolved let statement")
        };
        *id = HirId(u32::MAX);
        assert_internal_resolution_failure(&wrong_id);

        let let_id = helper_statement(&mut resolved.clone(), 0)
            .hir_id()
            .expect("let HIR id");
        let mut wrong_kind = resolved.clone();
        Arc::make_mut(&mut wrong_kind.arena)
            .nodes
            .get_mut(let_id.0 as usize)
            .expect("let arena node")
            .kind = ResolvedNodeKind::Expression;
        assert_internal_resolution_failure(&wrong_kind);

        let mut wrong_source = resolved;
        let Statement::Resolved { source, .. } = helper_statement(&mut wrong_source, 0) else {
            panic!("resolved let statement")
        };
        let range = source.as_mut().expect("source-backed let statement");
        range.range.end = range.range.end.saturating_sub(1);
        assert_internal_resolution_failure(&wrong_source);
    }

    fn resolved_type_program() -> (SourceFile, ResolvedProgram) {
        let text = r#"
module ResolvedTypes {
    fn inspect((int, bool) pair, List<int, 4> values) { return; }
}
"#;
        let source = SourceFile::new(SourceId(80), "resolved-types.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("resolved type fixture parses");
        let resolved = resolve(ast, &source).expect("resolved type fixture resolves");
        (source, resolved)
    }

    fn parameter_type(program: &mut ResolvedProgram, index: usize) -> &mut TypeExpr {
        let Item::Function(function) = &mut program.program.items[0] else {
            panic!("type fixture function")
        };
        function.params[index]
            .ty
            .as_mut()
            .expect("typed parameter annotation")
    }

    fn list_capacity_type(program: &mut ResolvedProgram) -> &mut TypeExpr {
        let TypeExpr::Resolved { ty, .. } = parameter_type(program, 1) else {
            panic!("resolved List type")
        };
        let TypeExpr::Generic { args, .. } = ty.as_mut() else {
            panic!("List generic type")
        };
        &mut args[1]
    }

    #[test]
    fn semantic_rejects_missing_or_corrupted_tuple_and_const_type_wrappers() {
        let (_source, resolved) = resolved_type_program();

        let mut missing_tuple = resolved.clone();
        let current = std::mem::replace(parameter_type(&mut missing_tuple, 0), TypeExpr::Const(0));
        let TypeExpr::Resolved { ty, .. } = current else {
            panic!("resolved tuple type")
        };
        *parameter_type(&mut missing_tuple, 0) = *ty;
        assert_internal_resolution_failure(&missing_tuple);

        let mut wrong_tuple_id = resolved.clone();
        let TypeExpr::Resolved { id, .. } = parameter_type(&mut wrong_tuple_id, 0) else {
            panic!("resolved tuple type")
        };
        *id = HirId(u32::MAX);
        assert_internal_resolution_failure(&wrong_tuple_id);

        let tuple_id = parameter_type(&mut resolved.clone(), 0)
            .hir_id()
            .expect("tuple HIR id");
        let mut wrong_tuple_kind = resolved.clone();
        Arc::make_mut(&mut wrong_tuple_kind.arena)
            .nodes
            .get_mut(tuple_id.0 as usize)
            .expect("tuple arena node")
            .kind = ResolvedNodeKind::Expression;
        assert_internal_resolution_failure(&wrong_tuple_kind);

        let mut wrong_tuple_source = resolved.clone();
        let TypeExpr::Resolved { source, .. } = parameter_type(&mut wrong_tuple_source, 0) else {
            panic!("resolved tuple type")
        };
        let range = source.as_mut().expect("source-backed tuple type");
        range.range.end = range.range.end.saturating_sub(1);
        assert_internal_resolution_failure(&wrong_tuple_source);

        let mut missing_const = resolved.clone();
        let current = std::mem::replace(list_capacity_type(&mut missing_const), TypeExpr::Const(0));
        let TypeExpr::Resolved { ty, .. } = current else {
            panic!("resolved List capacity")
        };
        *list_capacity_type(&mut missing_const) = *ty;
        assert_internal_resolution_failure(&missing_const);

        let mut wrong_const_id = resolved.clone();
        let TypeExpr::Resolved { id, .. } = list_capacity_type(&mut wrong_const_id) else {
            panic!("resolved List capacity")
        };
        *id = HirId(u32::MAX);
        assert_internal_resolution_failure(&wrong_const_id);

        let const_id = list_capacity_type(&mut resolved.clone())
            .hir_id()
            .expect("capacity HIR id");
        let mut wrong_const_kind = resolved.clone();
        Arc::make_mut(&mut wrong_const_kind.arena)
            .nodes
            .get_mut(const_id.0 as usize)
            .expect("capacity arena node")
            .kind = ResolvedNodeKind::Statement;
        assert_internal_resolution_failure(&wrong_const_kind);

        let mut wrong_const_source = resolved;
        let TypeExpr::Resolved { source, .. } = list_capacity_type(&mut wrong_const_source) else {
            panic!("resolved List capacity")
        };
        let range = source.as_mut().expect("source-backed List capacity");
        range.range.end = range.range.end.saturating_sub(1);
        assert_internal_resolution_failure(&wrong_const_source);
    }

    #[test]
    fn resolver_reports_shadowing_and_multiple_unknown_values_with_locations() {
        let text = r#"
module BadLocals {
    fn broken(int value) {
        let int value = 1;
        let int first = missing_one;
        let int second = missing_two;
    }
}
"#;
        let source = SourceFile::new(SourceId(74), "bad-locals.ko", text);
        let (ast, _) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("adversarial local source parses");
        let diagnostics = resolve(ast, &source).expect_err("resolution must reject every error");
        assert!(diagnostics.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E_LOCAL_SHADOWING" && diagnostic.primary_span.is_some()
        }));
        let unknowns = diagnostics
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.starts_with("unknown value"))
            .collect::<Vec<_>>();
        assert_eq!(unknowns.len(), 2);
        assert!(unknowns.iter().all(|diagnostic| {
            diagnostic
                .primary_span
                .as_ref()
                .and_then(|span| span.byte_range)
                .is_some()
        }));
    }

    #[test]
    fn only_canonical_numeric_conversions_are_resolver_intrinsics() {
        for canonical in [
            "decimal::from_int",
            "decimal::to_int_exact",
            "decimal::to_int_trunc",
            "decimal::to_int_round",
            "quantity::try_from_int",
            "quantity::try_from_decimal",
            "decimal::from_quantity",
        ] {
            assert!(intrinsic_call(canonical), "missing intrinsic `{canonical}`");
        }
        for retired in ["int::from_i64", "quantity::from_i64", "quantity::from_u128"] {
            assert!(
                !intrinsic_call(retired),
                "retired intrinsic `{retired}` leaked into V1"
            );
        }
    }
}

fn intrinsic_call(name: &str) -> bool {
    matches!(
        name,
        "decimal::from_int"
            | "decimal::to_int_exact"
            | "decimal::to_int_trunc"
            | "decimal::to_int_round"
            | "quantity::try_from_int"
            | "quantity::try_from_decimal"
            | "decimal::from_quantity"
    )
}

fn resolve_type(
    ast: &SpannedProgram,
    source: &SourceFile,
    fact: &TypeUseFact,
    structs: &BTreeMap<String, SymbolId>,
    external_structs: &BTreeSet<String>,
) -> Result<ResolvedTypeUse, Diagnostic> {
    let target = if builtin_type(&fact.name) {
        ResolvedTypeTarget::Builtin
    } else if let Some(symbol) = structs.get(&fact.name) {
        ResolvedTypeTarget::Struct(*symbol)
    } else if external_structs.contains(&fact.name) {
        ResolvedTypeTarget::ExternalStruct
    } else {
        return Err(Diagnostic::error(
            "K2002",
            DiagnosticPhase::Resolve,
            format!("unknown type `{}`", fact.name),
            ast.facts.source_map.source_span(source, fact.node),
        ));
    };
    Ok(ResolvedTypeUse {
        node: fact.node,
        source: ast
            .facts
            .source_map
            .source_range(fact.node)
            .expect("resolver facts always reference their source arena"),
        owner: fact.owner,
        name: fact.name.clone(),
        target,
    })
}

#[derive(Clone)]
struct GlobalTargets {
    all: BTreeMap<String, SymbolId>,
    structs: BTreeMap<String, SymbolId>,
    functions: BTreeMap<String, SymbolId>,
    states: BTreeMap<String, SymbolId>,
    consts: BTreeMap<String, SymbolId>,
    error_codes: BTreeMap<String, u32>,
    imports: BTreeSet<String>,
    external_functions: BTreeSet<String>,
    external_states: BTreeSet<String>,
    external_structs: BTreeSet<String>,
    external_consts: BTreeSet<String>,
    external_error_codes: BTreeMap<String, u32>,
}

struct HirLowerer<'a> {
    source: &'a SourceFile,
    source_map: &'a AstSourceMap,
    globals: GlobalTargets,
    parameter_sources: BTreeMap<(String, usize), (NodeId, SourceRange)>,
    binding_facts: BTreeMap<NodeId, Vec<BindingFact>>,
    consumed_binding_facts: BTreeSet<(NodeId, u16)>,
    consumed_binding_name_nodes: BTreeSet<NodeId>,
    arena: ResolvedArena,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> HirLowerer<'a> {
    fn new(
        source: &'a SourceFile,
        source_map: &'a AstSourceMap,
        globals: GlobalTargets,
        parameter_sources: BTreeMap<(String, usize), (NodeId, SourceRange)>,
        binding_facts: &[BindingFact],
    ) -> Self {
        let mut facts_by_owner = BTreeMap::<NodeId, Vec<BindingFact>>::new();
        for fact in binding_facts {
            facts_by_owner
                .entry(fact.owner)
                .or_default()
                .push(fact.clone());
        }
        for facts in facts_by_owner.values_mut() {
            facts.sort_by_key(|fact| fact.ordinal);
        }
        Self {
            source,
            source_map,
            globals,
            parameter_sources,
            binding_facts: facts_by_owner,
            consumed_binding_facts: BTreeSet::new(),
            consumed_binding_name_nodes: BTreeSet::new(),
            arena: ResolvedArena {
                source: source.id(),
                nodes: Vec::new(),
                scopes: vec![ResolvedScope {
                    id: ScopeId(0),
                    parent: None,
                }],
                bindings: Vec::new(),
                symbols: Vec::new(),
            },
            diagnostics: Vec::new(),
        }
    }

    fn source_span(&self, source: Option<SourceRange>) -> Option<SourceSpan> {
        source
            .filter(|range| range.source == self.source.id())
            .map(|range| SourceSpan::from_range(self.source, range.range))
    }

    fn validate_source_node(
        &mut self,
        node: NodeId,
        source: SourceRange,
        kinds: &[AstNodeKind],
    ) -> Option<SourceRange> {
        let valid = source.source == self.source.id()
            && self
                .source_map
                .node(node)
                .is_some_and(|mapped| mapped.range == source.range && kinds.contains(&mapped.kind));
        if valid {
            Some(source)
        } else {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                "source provenance NodeId/range/kind does not match the stable source arena",
                self.source_span(Some(source)),
            ));
            None
        }
    }

    fn new_scope(&mut self, parent: ScopeId) -> ScopeId {
        let id = ScopeId(u32::try_from(self.arena.scopes.len()).expect("scope budget fits u32"));
        self.arena.scopes.push(ResolvedScope {
            id,
            parent: Some(parent),
        });
        id
    }

    fn alloc_node(
        &mut self,
        kind: ResolvedNodeKind,
        scope: ScopeId,
        source_node: Option<NodeId>,
        source: Option<SourceRange>,
    ) -> HirId {
        let id = HirId(u32::try_from(self.arena.nodes.len()).expect("HIR node budget fits u32"));
        self.arena.nodes.push(ResolvedNode {
            id,
            scope,
            source,
            source_node,
            kind,
            target: None,
            bindings: Vec::new(),
        });
        id
    }

    fn node_mut(&mut self, id: HirId) -> &mut ResolvedNode {
        self.arena
            .nodes
            .get_mut(usize::try_from(id.0).expect("HIR id fits usize"))
            .expect("newly allocated HIR node exists")
    }

    fn binding_source_label(&self, binding: BindingId) -> Option<DiagnosticLabel> {
        self.arena
            .binding(binding)
            .and_then(|binding| binding.source)
            .and_then(|source| self.source_span(Some(source)))
            .map(|span| DiagnosticLabel {
                span,
                message: "previous binding is declared here".to_owned(),
            })
    }

    fn binding_fact_kind(kind: ResolvedBindingKind) -> Option<BindingFactKind> {
        match kind {
            ResolvedBindingKind::Parameter => None,
            ResolvedBindingKind::Local => Some(BindingFactKind::Local),
            ResolvedBindingKind::Pattern => Some(BindingFactKind::Pattern),
            ResolvedBindingKind::Iterator => Some(BindingFactKind::Iterator),
            ResolvedBindingKind::Comprehension => Some(BindingFactKind::Comprehension),
        }
    }

    fn consume_binding_fact(
        &mut self,
        owner: Option<NodeId>,
        ordinal: usize,
        name: &str,
        kind: ResolvedBindingKind,
    ) -> (Option<NodeId>, Option<SourceRange>) {
        let Some(owner) = owner else {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                format!("binding `{name}` has no direct parser-owned source node"),
                None,
            ));
            return (None, None);
        };
        let ordinal = u16::try_from(ordinal).expect("one node's binding budget fits u16");
        let expected_kind = Self::binding_fact_kind(kind)
            .expect("only non-parameter bindings use parser binding facts");
        let matches = self
            .binding_facts
            .get(&owner)
            .into_iter()
            .flatten()
            .filter(|fact| fact.ordinal == ordinal)
            .cloned()
            .collect::<Vec<_>>();
        let owner_source = self.source_map.source_range(owner);
        let [fact] = matches.as_slice() else {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                format!(
                    "binding `{name}` does not have exactly one parser fact for owner {:?} ordinal {ordinal}",
                    owner
                ),
                self.source_span(owner_source),
            ));
            return (None, None);
        };
        let owner_node = self.source_map.node(owner);
        let name_source = self.source_map.source_range(fact.name_node);
        let name_node = self.source_map.node(fact.name_node);
        let valid_owner_kind = owner_node.is_some_and(|node| match expected_kind {
            BindingFactKind::Local | BindingFactKind::Iterator => {
                node.kind == AstNodeKind::Statement
            }
            BindingFactKind::Pattern => {
                matches!(node.kind, AstNodeKind::Statement | AstNodeKind::Expression)
            }
            BindingFactKind::Comprehension => node.kind == AstNodeKind::ListComprehension,
        });
        let valid_name_node = name_node.is_some_and(|node| {
            node.kind == AstNodeKind::Name
                && !node.range.is_empty()
                && owner_node.is_some_and(|owner| owner.range.contains(node.range))
                && self.source.slice(node.range) == Some(fact.name.as_str())
        });
        if fact.owner != owner
            || fact.name != name
            || fact.kind != expected_kind
            || !valid_owner_kind
            || !valid_name_node
            || name_source.is_none()
        {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                format!(
                    "binding fact for `{name}` has mismatched owner, ordinal, role, spelling, or name token"
                ),
                self.source_span(name_source.or(owner_source)),
            ));
            return (None, None);
        }
        if !self.consumed_binding_name_nodes.insert(fact.name_node) {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                format!("binding fact for `{name}` reuses another binding's name token"),
                self.source_span(name_source),
            ));
            return (None, None);
        }
        if !self.consumed_binding_facts.insert((owner, ordinal)) {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                format!("binding fact for `{name}` was consumed more than once"),
                self.source_span(name_source),
            ));
            return (None, None);
        }
        (Some(fact.name_node), name_source)
    }

    fn diagnose_unconsumed_binding_facts(&mut self) {
        for facts in self.binding_facts.values() {
            for fact in facts {
                if !self
                    .consumed_binding_facts
                    .contains(&(fact.owner, fact.ordinal))
                {
                    self.diagnostics.push(Diagnostic::error(
                        "K2099",
                        DiagnosticPhase::Resolve,
                        format!(
                            "parser binding fact for `{}` was not consumed by its direct HIR owner",
                            fact.name
                        ),
                        self.source_map.source_span(self.source, fact.name_node),
                    ));
                }
            }
        }
    }

    fn declare_binding(
        &mut self,
        scope: ScopeId,
        visible: &mut BTreeMap<String, BindingId>,
        name: &str,
        kind: ResolvedBindingKind,
        source_node: Option<NodeId>,
        source: Option<SourceRange>,
        mutable: bool,
    ) -> BindingId {
        let id =
            BindingId(u32::try_from(self.arena.bindings.len()).expect("binding budget fits u32"));
        let reserved = crate::semantic::is_reserved_source_declaration(name, false);
        let previous = visible.get(name).copied();
        let global = self.globals.all.contains_key(name);
        if reserved || previous.is_some() || global {
            let message = if reserved {
                format!("local binding `{name}` uses a compiler-reserved name")
            } else if previous.is_some() {
                format!("local binding `{name}` duplicates or shadows an existing binding")
            } else if self.globals.consts.contains_key(name) {
                format!("local binding `{name}` shadows a const declaration")
            } else if self.globals.states.contains_key(name) {
                format!("local binding `{name}` shadows a state declaration")
            } else if self.globals.functions.contains_key(name) {
                format!("local binding `{name}` shadows a function declaration")
            } else if self.globals.structs.contains_key(name) {
                format!("local binding `{name}` shadows a struct declaration")
            } else {
                format!("local binding `{name}` shadows a source declaration")
            };
            let mut diagnostic = Diagnostic::error(
                if reserved {
                    "E_RESERVED_DECLARATION"
                } else {
                    "E_LOCAL_SHADOWING"
                },
                DiagnosticPhase::Resolve,
                message,
                self.source_span(source),
            );
            if let Some(previous) = previous
                && let Some(label) = self.binding_source_label(previous)
            {
                diagnostic.labels.push(label);
            }
            self.diagnostics.push(diagnostic);
        } else {
            visible.insert(name.to_owned(), id);
        }
        self.arena.bindings.push(ResolvedBinding {
            id,
            scope,
            name: name.to_owned(),
            kind,
            source,
            source_node,
            mutable,
        });
        id
    }

    fn value_target(
        &mut self,
        name: &str,
        visible: &BTreeMap<String, BindingId>,
        source: Option<SourceRange>,
    ) -> Option<ResolvedValueTarget> {
        let target = if let Some(binding) = visible.get(name) {
            Some(ResolvedValueTarget::Binding(*binding))
        } else if let Some(symbol) = self.globals.states.get(name) {
            Some(ResolvedValueTarget::State(*symbol))
        } else if let Some(symbol) = self.globals.consts.get(name) {
            Some(ResolvedValueTarget::Const(*symbol))
        } else if let Some(code) = self.globals.error_codes.get(name) {
            Some(ResolvedValueTarget::ErrorCode(*code))
        } else if crate::semantic::V1_ROUNDING_PATHS.contains(&name) || name == "null" {
            Some(ResolvedValueTarget::Intrinsic)
        } else if self.globals.external_states.contains(name) {
            Some(ResolvedValueTarget::ExternalState)
        } else if self.globals.external_consts.contains(name) {
            Some(ResolvedValueTarget::ExternalConst)
        } else if let Some(code) = self.globals.external_error_codes.get(name) {
            Some(ResolvedValueTarget::ErrorCode(*code))
        } else {
            None
        };
        if target.is_none() {
            self.diagnostics.push(Diagnostic::error(
                "K2002",
                DiagnosticPhase::Resolve,
                format!("unknown value `{name}`"),
                self.source_span(source),
            ));
        }
        target
    }

    fn type_target(
        &mut self,
        name: &str,
        _source: Option<SourceRange>,
    ) -> Option<ResolvedTypeTarget> {
        if builtin_type(name) {
            Some(ResolvedTypeTarget::Builtin)
        } else if self.globals.external_structs.contains(name) {
            Some(ResolvedTypeTarget::ExternalStruct)
        } else {
            self.globals
                .structs
                .get(name)
                .copied()
                .map(ResolvedTypeTarget::Struct)
        }
    }

    fn call_target(
        &mut self,
        name: &str,
        implicit_receiver: bool,
        _source: Option<SourceRange>,
    ) -> Option<ResolvedCallTarget> {
        if implicit_receiver {
            Some(ResolvedCallTarget::Method)
        } else if let Some(symbol) = self.globals.functions.get(name) {
            Some(ResolvedCallTarget::Function(*symbol))
        } else if let Some(builtin) = Builtin::from_source_name(name) {
            Some(ResolvedCallTarget::Builtin(builtin))
        } else if let Some(symbol) = self.globals.structs.get(name) {
            Some(ResolvedCallTarget::Struct(*symbol))
        } else if intrinsic_call(name) {
            Some(ResolvedCallTarget::Intrinsic)
        } else if self.globals.external_functions.contains(name)
            || name
                .split_once("::")
                .is_some_and(|(alias, _)| self.globals.imports.contains(alias))
        {
            Some(ResolvedCallTarget::External)
        } else {
            None
        }
    }

    fn wrap_type(&mut self, ty: TypeExpr, scope: ScopeId) -> TypeExpr {
        let mut ty = ty;
        let mut source_node = None;
        let mut source = None;
        while let TypeExpr::Source {
            node,
            source: range,
            ty: inner,
        } = ty
        {
            source = self.validate_source_node(node, range, &[AstNodeKind::Type]);
            source_node = Some(node);
            ty = *inner;
        }
        if let TypeExpr::Resolved { source, .. } = ty {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                "resolved-HIR type wrapper was supplied as spanned AST input",
                self.source_span(source),
            ));
            return TypeExpr::Const(0);
        }
        if source_node.is_none() {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                "spanned AST contains a type node without explicit Source(NodeId, range) provenance",
                None,
            ));
        }
        let id = self.alloc_node(ResolvedNodeKind::Type, scope, source_node, source);
        match &mut ty {
            TypeExpr::Path(name) => {
                self.node_mut(id).target = self.type_target(name, source).map(ResolvedTarget::Type);
            }
            TypeExpr::Generic { base, args } => {
                self.node_mut(id).target = self.type_target(base, source).map(ResolvedTarget::Type);
                for argument in args {
                    let current = std::mem::replace(argument, TypeExpr::Const(0));
                    *argument = self.wrap_type(current, scope);
                }
            }
            TypeExpr::Tuple(elements) => {
                for element in elements {
                    let current = std::mem::replace(element, TypeExpr::Const(0));
                    *element = self.wrap_type(current, scope);
                }
            }
            TypeExpr::Const(_) => {}
            TypeExpr::Source { .. } | TypeExpr::Resolved { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    "K2099",
                    DiagnosticPhase::Resolve,
                    "source or resolved wrapper escaped the AST/HIR stage boundary",
                    self.source_span(source),
                ));
            }
        }
        TypeExpr::Resolved {
            id,
            source,
            ty: Box::new(ty),
        }
    }

    fn declare_pattern(
        &mut self,
        pattern: &Pattern,
        scope: ScopeId,
        visible: &mut BTreeMap<String, BindingId>,
        kind: ResolvedBindingKind,
        owner: Option<NodeId>,
        source: Option<SourceRange>,
        mutable: bool,
    ) -> Vec<BindingId> {
        let names = match pattern {
            Pattern::Name(name) => std::slice::from_ref(name),
            Pattern::Tuple(names) => names.as_slice(),
        };
        names
            .iter()
            .enumerate()
            .map(|(ordinal, name)| {
                let (name_node, name_source) =
                    self.consume_binding_fact(owner, ordinal, name, kind);
                self.declare_binding(
                    scope,
                    visible,
                    name,
                    kind,
                    name_node,
                    name_source.or(source),
                    mutable,
                )
            })
            .collect()
    }

    fn declare_sum_pattern(
        &mut self,
        pattern: &SumPattern,
        scope: ScopeId,
        visible: &mut BTreeMap<String, BindingId>,
        owner: Option<NodeId>,
        ordinal: usize,
        source: Option<SourceRange>,
    ) -> Vec<BindingId> {
        match &pattern.binding {
            Some(PatternBinding::Name(name)) => {
                let (name_node, name_source) =
                    self.consume_binding_fact(owner, ordinal, name, ResolvedBindingKind::Pattern);
                vec![self.declare_binding(
                    scope,
                    visible,
                    name,
                    ResolvedBindingKind::Pattern,
                    name_node,
                    name_source.or(source),
                    false,
                )]
            }
            Some(PatternBinding::Wildcard) | None => Vec::new(),
        }
    }

    fn wrap_block(
        &mut self,
        block: &mut Block,
        scope: ScopeId,
        visible: &mut BTreeMap<String, BindingId>,
    ) {
        for statement in &mut block.statements {
            let current = std::mem::replace(statement, Statement::Break);
            *statement = self.wrap_statement(current, scope, visible);
        }
        if let Some(tail) = block.tail.take() {
            block.tail = Some(Box::new(self.wrap_expr(*tail, scope, visible)));
        }
    }

    fn wrap_child_block(
        &mut self,
        block: &mut Block,
        parent: ScopeId,
        visible: &BTreeMap<String, BindingId>,
    ) {
        let scope = self.new_scope(parent);
        let mut child_visible = visible.clone();
        self.wrap_block(block, scope, &mut child_visible);
    }

    fn wrap_statement(
        &mut self,
        statement: Statement,
        scope: ScopeId,
        visible: &mut BTreeMap<String, BindingId>,
    ) -> Statement {
        let mut statement = statement;
        let mut source_node = None;
        let mut source = None;
        while let Statement::Source {
            node,
            source: range,
            statement: inner,
        } = statement
        {
            source = self.validate_source_node(node, range, &[AstNodeKind::Statement]);
            source_node = Some(node);
            statement = *inner;
        }
        if let Statement::Resolved { source, .. } = statement {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                "resolved-HIR statement wrapper was supplied as spanned AST input",
                self.source_span(source),
            ));
            return Statement::Break;
        }
        if source_node.is_none() {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                "spanned AST contains a statement without explicit Source(NodeId, range) provenance",
                None,
            ));
        }
        let id = self.alloc_node(ResolvedNodeKind::Statement, scope, source_node, source);
        match &mut statement {
            Statement::Let {
                mutable,
                pat,
                ty,
                value,
            } => {
                if let Some(current) = ty.take() {
                    *ty = Some(self.wrap_type(current, scope));
                }
                let current = std::mem::replace(value, Expr::IntLiteral(BigInt::zero()));
                *value = self.wrap_expr(current, scope, visible);
                self.node_mut(id).bindings = self.declare_pattern(
                    pat,
                    scope,
                    visible,
                    ResolvedBindingKind::Local,
                    source_node,
                    source,
                    *mutable,
                );
            }
            Statement::Assign { name, value } => {
                let current = std::mem::replace(value, Expr::IntLiteral(BigInt::zero()));
                *value = self.wrap_expr(current, scope, visible);
                self.node_mut(id).target = self
                    .value_target(name, visible, source)
                    .map(ResolvedTarget::Assignment);
            }
            Statement::AssignExpr { target, value, .. } => {
                let current = std::mem::replace(target, Expr::IntLiteral(BigInt::zero()));
                *target = self.wrap_expr(current, scope, visible);
                let current = std::mem::replace(value, Expr::IntLiteral(BigInt::zero()));
                *value = self.wrap_expr(current, scope, visible);
            }
            Statement::Expr(expression) => {
                let current = std::mem::replace(expression, Expr::IntLiteral(BigInt::zero()));
                *expression = self.wrap_expr(current, scope, visible);
            }
            Statement::Return(expression) => {
                if let Some(current) = expression.take() {
                    *expression = Some(self.wrap_expr(current, scope, visible));
                }
            }
            Statement::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let current = std::mem::replace(cond, Expr::Bool(false));
                *cond = self.wrap_expr(current, scope, visible);
                self.wrap_child_block(then_branch, scope, visible);
                if let Some(block) = else_branch {
                    self.wrap_child_block(block, scope, visible);
                }
            }
            Statement::IfLet {
                pattern,
                value,
                then_branch,
                else_branch,
            } => {
                let current = std::mem::replace(value, Expr::Bool(false));
                *value = self.wrap_expr(current, scope, visible);
                let then_scope = self.new_scope(scope);
                let mut then_visible = visible.clone();
                self.node_mut(id).bindings = self.declare_sum_pattern(
                    pattern,
                    then_scope,
                    &mut then_visible,
                    source_node,
                    0,
                    source,
                );
                self.wrap_block(then_branch, then_scope, &mut then_visible);
                if let Some(block) = else_branch {
                    self.wrap_child_block(block, scope, visible);
                }
            }
            Statement::While { cond, body } => {
                let current = std::mem::replace(cond, Expr::Bool(false));
                *cond = self.wrap_expr(current, scope, visible);
                self.wrap_child_block(body, scope, visible);
            }
            Statement::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                let loop_scope = self.new_scope(scope);
                let mut loop_visible = visible.clone();
                if let Some(current) = init.take() {
                    *init = Some(Box::new(self.wrap_statement(
                        *current,
                        loop_scope,
                        &mut loop_visible,
                    )));
                }
                if let Some(current) = cond.take() {
                    *cond = Some(self.wrap_expr(current, loop_scope, &loop_visible));
                }
                if let Some(current) = step.take() {
                    *step = Some(Box::new(self.wrap_statement(
                        *current,
                        loop_scope,
                        &mut loop_visible,
                    )));
                }
                self.wrap_block(body, loop_scope, &mut loop_visible);
            }
            Statement::ForEachMap {
                key,
                value,
                map,
                body,
            } => {
                let current = std::mem::replace(map, Expr::Bool(false));
                *map = self.wrap_expr(current, scope, visible);
                let loop_scope = self.new_scope(scope);
                let mut loop_visible = visible.clone();
                let (key_node, key_source) =
                    self.consume_binding_fact(source_node, 0, key, ResolvedBindingKind::Iterator);
                let mut bindings = vec![self.declare_binding(
                    loop_scope,
                    &mut loop_visible,
                    key,
                    ResolvedBindingKind::Iterator,
                    key_node,
                    key_source.or(source),
                    false,
                )];
                if let Some(value) = value {
                    let (value_node, value_source) = self.consume_binding_fact(
                        source_node,
                        1,
                        value,
                        ResolvedBindingKind::Iterator,
                    );
                    bindings.push(self.declare_binding(
                        loop_scope,
                        &mut loop_visible,
                        value,
                        ResolvedBindingKind::Iterator,
                        value_node,
                        value_source.or(source),
                        false,
                    ));
                }
                self.node_mut(id).bindings = bindings;
                self.wrap_block(body, loop_scope, &mut loop_visible);
            }
            Statement::Break | Statement::Continue => {}
            Statement::Source { .. } | Statement::Resolved { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    "K2099",
                    DiagnosticPhase::Resolve,
                    "source or resolved wrapper escaped the AST/HIR stage boundary",
                    self.source_span(source),
                ));
            }
        }
        Statement::Resolved {
            id,
            source,
            statement: Box::new(statement),
        }
    }

    fn wrap_expr(
        &mut self,
        expression: Expr,
        scope: ScopeId,
        visible: &BTreeMap<String, BindingId>,
    ) -> Expr {
        let mut expression = expression;
        let mut source_node = None;
        let mut source = None;
        while let Expr::Source {
            node,
            source: range,
            expression: inner,
        } = expression
        {
            source = self.validate_source_node(
                node,
                range,
                &[
                    AstNodeKind::Expression,
                    AstNodeKind::Call,
                    AstNodeKind::IndexExpression,
                    AstNodeKind::ListComprehension,
                    AstNodeKind::DecimalLiteral,
                ],
            );
            source_node = Some(node);
            expression = *inner;
        }
        if let Expr::Resolved { source, .. } = expression {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                "resolved-HIR expression wrapper was supplied as spanned AST input",
                self.source_span(source),
            ));
            return Expr::IntLiteral(BigInt::zero());
        }
        if source_node.is_none() {
            self.diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                "spanned AST contains an expression without explicit Source(NodeId, range) provenance",
                None,
            ));
        }
        let id = self.alloc_node(ResolvedNodeKind::Expression, scope, source_node, source);
        match &mut expression {
            Expr::Ident(name) => {
                self.node_mut(id).target = self
                    .value_target(name, visible, source)
                    .map(ResolvedTarget::Value);
            }
            Expr::Call {
                name,
                args,
                implicit_receiver,
                ..
            } => {
                self.node_mut(id).target = self
                    .call_target(name, *implicit_receiver, source)
                    .map(ResolvedTarget::Call);
                for argument in args {
                    let current = std::mem::replace(argument, Expr::IntLiteral(BigInt::zero()));
                    *argument = self.wrap_expr(current, scope, visible);
                }
            }
            Expr::StructLiteral { name, fields } => {
                self.node_mut(id).target = if let Some(symbol) = self.globals.structs.get(name) {
                    Some(ResolvedTarget::StructLiteral(*symbol))
                } else if self.globals.external_structs.contains(name) {
                    Some(ResolvedTarget::ExternalStructLiteral)
                } else {
                    None
                };
                if self.node_mut(id).target.is_none() {
                    self.diagnostics.push(Diagnostic::error(
                        "K2002",
                        DiagnosticPhase::Resolve,
                        format!("unknown struct `{name}`"),
                        self.source_span(source),
                    ));
                }
                for field in fields {
                    let current =
                        std::mem::replace(&mut field.value, Expr::IntLiteral(BigInt::zero()));
                    field.value = self.wrap_expr(current, scope, visible);
                }
            }
            Expr::Binary { left, right, .. }
            | Expr::Index {
                target: left,
                index: right,
            } => {
                let current = std::mem::replace(left, Box::new(Expr::IntLiteral(BigInt::zero())));
                *left = Box::new(self.wrap_expr(*current, scope, visible));
                let current = std::mem::replace(right, Box::new(Expr::IntLiteral(BigInt::zero())));
                *right = Box::new(self.wrap_expr(*current, scope, visible));
            }
            Expr::Unary { expr, .. }
            | Expr::Member { object: expr, .. }
            | Expr::OptionSome(expr)
            | Expr::ResultOk(expr)
            | Expr::ResultErr(expr)
            | Expr::Propagate(expr) => {
                let current = std::mem::replace(expr, Box::new(Expr::IntLiteral(BigInt::zero())));
                *expr = Box::new(self.wrap_expr(*current, scope, visible));
            }
            Expr::Conditional {
                cond,
                then_expr,
                else_expr,
            } => {
                for child in [cond, then_expr, else_expr] {
                    let current =
                        std::mem::replace(child, Box::new(Expr::IntLiteral(BigInt::zero())));
                    *child = Box::new(self.wrap_expr(*current, scope, visible));
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let current = std::mem::replace(condition, Box::new(Expr::Bool(false)));
                *condition = Box::new(self.wrap_expr(*current, scope, visible));
                self.wrap_child_block(then_branch, scope, visible);
                if let Some(block) = else_branch {
                    self.wrap_child_block(block, scope, visible);
                }
            }
            Expr::IfLet {
                pattern,
                value,
                then_branch,
                else_branch,
            } => {
                let current = std::mem::replace(value, Box::new(Expr::Bool(false)));
                *value = Box::new(self.wrap_expr(*current, scope, visible));
                let then_scope = self.new_scope(scope);
                let mut then_visible = visible.clone();
                self.node_mut(id).bindings = self.declare_sum_pattern(
                    pattern,
                    then_scope,
                    &mut then_visible,
                    source_node,
                    0,
                    source,
                );
                self.wrap_block(then_branch, then_scope, &mut then_visible);
                if let Some(block) = else_branch {
                    self.wrap_child_block(block, scope, visible);
                }
            }
            Expr::Match { value, arms } => {
                let current = std::mem::replace(value, Box::new(Expr::Bool(false)));
                *value = Box::new(self.wrap_expr(*current, scope, visible));
                let mut bindings = Vec::new();
                let mut binding_ordinal = 0;
                for arm in arms {
                    let arm_scope = self.new_scope(scope);
                    let mut arm_visible = visible.clone();
                    let ordinal = binding_ordinal;
                    if matches!(&arm.pattern.binding, Some(PatternBinding::Name(_))) {
                        binding_ordinal += 1;
                    }
                    bindings.extend(self.declare_sum_pattern(
                        &arm.pattern,
                        arm_scope,
                        &mut arm_visible,
                        source_node,
                        ordinal,
                        source,
                    ));
                    self.wrap_block(&mut arm.body, arm_scope, &mut arm_visible);
                }
                self.node_mut(id).bindings = bindings;
            }
            Expr::Tuple(elements) | Expr::List(elements) | Expr::JsonArray(elements) => {
                for element in elements {
                    let current = std::mem::replace(element, Expr::IntLiteral(BigInt::zero()));
                    *element = self.wrap_expr(current, scope, visible);
                }
            }
            Expr::ListComprehension {
                expression: item_expression,
                item,
                source: list_source,
                condition,
            } => {
                let comprehension_scope = self.new_scope(scope);
                let mut comprehension_visible = visible.clone();
                let (item_node, item_source) = self.consume_binding_fact(
                    source_node,
                    0,
                    item,
                    ResolvedBindingKind::Comprehension,
                );
                self.node_mut(id).bindings = vec![self.declare_binding(
                    comprehension_scope,
                    &mut comprehension_visible,
                    item,
                    ResolvedBindingKind::Comprehension,
                    item_node,
                    item_source.or(source),
                    false,
                )];
                let current =
                    std::mem::replace(item_expression, Box::new(Expr::IntLiteral(BigInt::zero())));
                *item_expression =
                    Box::new(self.wrap_expr(*current, comprehension_scope, &comprehension_visible));
                let current =
                    std::mem::replace(list_source, Box::new(Expr::IntLiteral(BigInt::zero())));
                *list_source = Box::new(self.wrap_expr(*current, scope, visible));
                if let Some(current) = condition.take() {
                    *condition = Some(Box::new(self.wrap_expr(
                        *current,
                        comprehension_scope,
                        &comprehension_visible,
                    )));
                }
            }
            Expr::JsonObject(entries) => {
                for entry in entries {
                    let current =
                        std::mem::replace(&mut entry.value, Expr::IntLiteral(BigInt::zero()));
                    entry.value = self.wrap_expr(current, scope, visible);
                }
            }
            Expr::IntLiteral(_)
            | Expr::DecimalLiteral(_)
            | Expr::OptionNone
            | Expr::Bool(_)
            | Expr::String(_)
            | Expr::Bytes(_) => {}
            Expr::Source { .. } | Expr::Resolved { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    "K2099",
                    DiagnosticPhase::Resolve,
                    "source or resolved wrapper escaped the AST/HIR stage boundary",
                    self.source_span(source),
                ));
            }
        }
        Expr::Resolved {
            id,
            source,
            expression: Box::new(expression),
        }
    }

    fn lower_program(mut self, mut program: Program) -> (Program, ResolvedArena, Vec<Diagnostic>) {
        let root = ScopeId(0);
        let root_visible = BTreeMap::new();
        for item in &mut program.items {
            match item {
                Item::Function(function) => {
                    let scope = self.new_scope(root);
                    let mut visible = BTreeMap::new();
                    for (index, parameter) in function.params.iter().enumerate() {
                        let source = self
                            .parameter_sources
                            .get(&(function.name.clone(), index))
                            .copied();
                        let (source_node, source) = source
                            .map(|(node, range)| (Some(node), Some(range)))
                            .unwrap_or((None, None));
                        self.declare_binding(
                            scope,
                            &mut visible,
                            &parameter.name,
                            ResolvedBindingKind::Parameter,
                            source_node,
                            source,
                            false,
                        );
                    }
                    for parameter in &mut function.params {
                        if let Some(current) = parameter.ty.take() {
                            parameter.ty = Some(self.wrap_type(current, scope));
                        }
                    }
                    if let Some(current) = function.ret_ty.take() {
                        function.ret_ty = Some(self.wrap_type(current, scope));
                    }
                    self.wrap_block(&mut function.body, scope, &mut visible);
                }
                Item::Struct(definition) => {
                    for (_, ty) in &mut definition.fields {
                        let current = std::mem::replace(ty, TypeExpr::Const(0));
                        *ty = self.wrap_type(current, root);
                    }
                }
                Item::Const(declaration) => {
                    if let Some(current) = declaration.ty.take() {
                        declaration.ty = Some(self.wrap_type(current, root));
                    }
                    let current =
                        std::mem::replace(&mut declaration.value, Expr::IntLiteral(BigInt::zero()));
                    declaration.value = self.wrap_expr(current, root, &root_visible);
                }
                Item::State(declaration) => {
                    let current = std::mem::replace(&mut declaration.ty, TypeExpr::Const(0));
                    declaration.ty = self.wrap_type(current, root);
                }
                Item::Trigger(declaration) => {
                    for entry in &mut declaration.metadata {
                        let current =
                            std::mem::replace(&mut entry.value, Expr::IntLiteral(BigInt::zero()));
                        entry.value = self.wrap_expr(current, root, &root_visible);
                    }
                }
                Item::ErrorEnum(_) => {}
            }
        }
        for fixture in &mut program.fixtures {
            for action in &mut fixture.actions {
                for argument in &mut action.args {
                    let current = std::mem::replace(argument, Expr::IntLiteral(BigInt::zero()));
                    *argument = self.wrap_expr(current, root, &root_visible);
                }
            }
        }

        self.diagnose_unconsumed_binding_facts();
        (program, self.arena, self.diagnostics)
    }
}

/// Resolve one CST-derived spanned AST into named HIR.
pub(crate) fn resolve(
    ast: SpannedProgram,
    source: &SourceFile,
) -> Result<ResolvedProgram, DiagnosticBundle> {
    let external = ExternalResolutionEnvironment::default();
    resolve_with_imports_and_externals(ast, source, &BTreeMap::new(), &external)
}

/// Resolve a module source while retaining explicit import-alias calls for the typed linker.
pub(crate) fn resolve_with_imports(
    ast: SpannedProgram,
    source: &SourceFile,
    imports: &BTreeMap<String, ()>,
) -> Result<ResolvedProgram, DiagnosticBundle> {
    let external = ExternalResolutionEnvironment::default();
    resolve_with_imports_and_externals(ast, source, imports, &external)
}

/// Names exported by one typed standalone-test target for fail-closed resolution.
#[derive(Clone, Debug, Default)]
pub(crate) struct ExternalResolutionEnvironment {
    pub(crate) functions: BTreeSet<String>,
    pub(crate) states: BTreeSet<String>,
    pub(crate) structs: BTreeSet<String>,
    pub(crate) consts: BTreeSet<String>,
    pub(crate) error_codes: BTreeMap<String, u32>,
}

/// Resolve a standalone test source against its target's typed interface.
pub(crate) fn resolve_with_external_environment(
    ast: SpannedProgram,
    source: &SourceFile,
    external: &ExternalResolutionEnvironment,
) -> Result<ResolvedProgram, DiagnosticBundle> {
    resolve_with_imports_and_externals(ast, source, &BTreeMap::new(), external)
}

fn resolve_with_imports_and_externals(
    ast: SpannedProgram,
    source: &SourceFile,
    imports: &BTreeMap<String, ()>,
    external: &ExternalResolutionEnvironment,
) -> Result<ResolvedProgram, DiagnosticBundle> {
    if ast.facts.source_map.source() != source.id() {
        return Err(DiagnosticBundle::single(Diagnostic::error(
            "K2099",
            DiagnosticPhase::Resolve,
            "spanned AST and resolver source identities do not match",
            None,
        )));
    }
    let mut diagnostics = Vec::new();
    let mut symbols = Vec::new();
    let mut globals = BTreeMap::<String, (SymbolId, &DeclarationFact)>::new();
    let mut structs = BTreeMap::<String, SymbolId>::new();
    let mut functions = BTreeMap::<String, SymbolId>::new();
    let mut states = BTreeMap::<String, SymbolId>::new();
    let mut consts = BTreeMap::<String, SymbolId>::new();
    for fact in &ast.facts.declarations {
        if fact.kind == DeclarationKind::Parameter {
            if fact.owner.is_none() {
                diagnostics.push(Diagnostic::error(
                    "K2099",
                    DiagnosticPhase::Resolve,
                    format!("parameter `{}` has no owning function", fact.name),
                    declaration_span(&ast, source, fact),
                ));
            }
            // Parameter collisions are diagnosed while constructing the native
            // lexical binding arena below. Reporting them here as declarations
            // as well would emit two diagnostics for the same exact name token.
            continue;
        }

        let id = symbol_id(symbols.len());
        if crate::semantic::is_reserved_source_declaration(&fact.name, fact.kind.is_function()) {
            diagnostics.push(Diagnostic::error(
                "E_RESERVED_DECLARATION",
                DiagnosticPhase::Resolve,
                format!(
                    "{} `{}` uses a compiler-reserved name",
                    fact.kind.description(),
                    fact.name
                ),
                declaration_span(&ast, source, fact),
            ));
            continue;
        }
        if let Some((_, previous)) = globals.get(&fact.name) {
            diagnostics.push(duplicate_diagnostic(&ast, source, fact, previous));
            continue;
        }
        globals.insert(fact.name.clone(), (id, fact));
        match fact.kind {
            DeclarationKind::Function => {
                functions.insert(fact.name.clone(), id);
            }
            DeclarationKind::Struct => {
                structs.insert(fact.name.clone(), id);
            }
            DeclarationKind::State => {
                states.insert(fact.name.clone(), id);
            }
            DeclarationKind::Const => {
                consts.insert(fact.name.clone(), id);
            }
            DeclarationKind::SourceUnit | DeclarationKind::ErrorEnum | DeclarationKind::Trigger => {
            }
            DeclarationKind::Parameter => unreachable!("parameters were handled above"),
        }
        symbols.push(ResolvedSymbol {
            id,
            node: fact.node,
            source: ast
                .facts
                .source_map
                .source_range(fact.name_node)
                .expect("declaration facts always reference their source arena"),
            name: fact.name.clone(),
            kind: symbol_kind(fact.kind).expect("global declaration has a symbol kind"),
        });
    }

    let declaration_source = |name: &str, kind: DeclarationKind| {
        ast.facts
            .declarations
            .iter()
            .find(|fact| fact.kind == kind && fact.name == name)
            .and_then(|fact| ast.facts.source_map.source_range(fact.node))
    };
    let mut error_codes = BTreeMap::new();
    for item in &ast.program.items {
        match item {
            Item::Struct(definition) => {
                let mut fields = BTreeSet::new();
                for (field, _) in &definition.fields {
                    if !fields.insert(field.as_str()) {
                        diagnostics.push(Diagnostic::error(
                            "E_DUPLICATE_DECLARATION",
                            DiagnosticPhase::Resolve,
                            format!(
                                "field `{field}` is declared more than once in struct `{}`",
                                definition.name
                            ),
                            declaration_source(&definition.name, DeclarationKind::Struct)
                                .map(|range| SourceSpan::from_range(source, range.range)),
                        ));
                    }
                }
            }
            Item::ErrorEnum(definition) => {
                let mut variants = BTreeSet::new();
                for variant in &definition.variants {
                    if !variants.insert(variant.name.as_str()) {
                        diagnostics.push(Diagnostic::error(
                            "E_DUPLICATE_DECLARATION",
                            DiagnosticPhase::Resolve,
                            format!(
                                "error variant `{}` is declared more than once in `{}`",
                                variant.name, definition.name
                            ),
                            declaration_source(&definition.name, DeclarationKind::ErrorEnum)
                                .map(|range| SourceSpan::from_range(source, range.range)),
                        ));
                    }
                    error_codes.insert(
                        format!("{}::{}", definition.name, variant.name),
                        variant.code,
                    );
                }
            }
            Item::Function(_) | Item::Const(_) | Item::State(_) | Item::Trigger(_) => {}
        }
    }

    let mut types = Vec::with_capacity(ast.facts.type_uses.len());
    for fact in &ast.facts.type_uses {
        match resolve_type(&ast, source, fact, &structs, &external.structs) {
            Ok(resolved) => types.push(resolved),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    let mut calls = Vec::with_capacity(ast.facts.calls.len());
    for fact in &ast.facts.calls {
        let target = if fact.implicit_receiver {
            Some(ResolvedCallTarget::Method)
        } else if let Some(symbol) = functions.get(&fact.name) {
            Some(ResolvedCallTarget::Function(*symbol))
        } else if let Some(builtin) = Builtin::from_source_name(&fact.name) {
            Some(ResolvedCallTarget::Builtin(builtin))
        } else if let Some(symbol) = structs.get(&fact.name) {
            Some(ResolvedCallTarget::Struct(*symbol))
        } else if intrinsic_call(&fact.name) {
            Some(ResolvedCallTarget::Intrinsic)
        } else if external.functions.contains(&fact.name) {
            Some(ResolvedCallTarget::External)
        } else if fact
            .name
            .split_once("::")
            .is_some_and(|(alias, _)| imports.contains_key(alias))
        {
            Some(ResolvedCallTarget::External)
        } else {
            None
        };
        if let Some(target) = target {
            calls.push(ResolvedCall {
                node: fact.node,
                name_node: fact.name_node,
                source: ast
                    .facts
                    .source_map
                    .source_range(fact.node)
                    .expect("call facts always reference their source arena"),
                name_source: ast
                    .facts
                    .source_map
                    .source_range(fact.name_node)
                    .expect("call names always reference their source arena"),
                owner: fact.owner,
                name: fact.name.clone(),
                target,
            });
        } else {
            diagnostics.push(Diagnostic::error(
                "K2002",
                DiagnosticPhase::Resolve,
                format!("unknown function or builtin `{}`", fact.name),
                ast.facts.source_map.source_span(source, fact.name_node),
            ));
        }
    }

    let mut parameter_sources = BTreeMap::new();
    for item in &ast.program.items {
        let Item::Function(function) = item else {
            continue;
        };
        let Some(owner) = ast
            .facts
            .declarations
            .iter()
            .find(|fact| fact.kind == DeclarationKind::Function && fact.name == function.name)
            .map(|fact| fact.node)
        else {
            continue;
        };
        for (index, fact) in ast
            .facts
            .declarations
            .iter()
            .filter(|fact| fact.kind == DeclarationKind::Parameter && fact.owner == Some(owner))
            .enumerate()
        {
            if let Some(range) = ast.facts.source_map.source_range(fact.name_node) {
                parameter_sources.insert((function.name.clone(), index), (fact.name_node, range));
            }
        }
    }

    let global_targets = GlobalTargets {
        all: globals
            .iter()
            .map(|(name, (id, _))| (name.clone(), *id))
            .collect(),
        structs,
        functions,
        states,
        consts,
        error_codes,
        imports: imports.keys().cloned().collect(),
        external_functions: external.functions.clone(),
        external_states: external.states.clone(),
        external_structs: external.structs.clone(),
        external_consts: external.consts.clone(),
        external_error_codes: external.error_codes.clone(),
    };
    let SpannedProgram { program, facts } = ast;
    let (program, mut arena, lower_diagnostics) = HirLowerer::new(
        source,
        &facts.source_map,
        global_targets,
        parameter_sources,
        &facts.bindings,
    )
    .lower_program(program);
    arena.symbols = symbols.clone();
    diagnostics.extend(lower_diagnostics);

    for call in &calls {
        let matching = arena.nodes().filter(|node| {
            node.source == Some(call.source)
                && node.target == Some(ResolvedTarget::Call(call.target))
        });
        if matching.count() != 1 {
            diagnostics.push(Diagnostic::error(
                "K2099",
                DiagnosticPhase::Resolve,
                format!(
                    "resolved call `{}` did not bind exactly once into native HIR",
                    call.name
                ),
                Some(SourceSpan::from_range(source, call.name_source.range)),
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(ResolvedProgram {
            program,
            facts,
            source_file: source.clone(),
            symbols,
            types,
            calls,
            arena: Arc::new(arena),
        })
    } else {
        Err(DiagnosticBundle::new(diagnostics))
    }
}
