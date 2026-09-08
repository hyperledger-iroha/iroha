//! Immutable compiler-owned editor analysis. Navigation uses resolved identities, never spelling scans.
//!
//! Recovery supplies completion candidates only. An incomplete buffer cannot produce a compilable
//! recovered AST, a rename edit, or a claimed cross-file reference. Locked imports are the sole
//! authority for cross-file symbols; this module never reads the filesystem.
use crate::{
    ast::ParameterCallMode,
    builtins::{Builtin, BuiltinCallPolicy, BuiltinMode, BuiltinSurface},
    lexer::{Token, TokenKind},
    linker::{
        ImportBinding, LinkRequest, LinkerOptions, ModuleBuildGraph, ModuleUnit, PackageUnit,
        SourceLinkRequest, TypedLinker,
    },
    resolved::{
        BindingId, ResolvedCallTarget, ResolvedProgram, ResolvedSymbolKind, ResolvedTarget,
        ResolvedTypeTarget, ResolvedValueTarget, SymbolId,
    },
    semantic::{FunctionSignature, SemanticContext, Type, TypedHirNode, render_type_name},
    source::{FrontendBudget, SourceFile, SourceId, SourceRange, TextRange},
    spanned_ast::{AstFacts, DeclarationKind},
};
use std::collections::{BTreeMap, BTreeSet};

/// Stable source declaration or lexical binding in one snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EditorIdentity {
    /// A resolver-owned top-level declaration.
    Symbol(SourceId, SymbolId),
    /// A resolver-owned lexical binding.
    Binding(SourceId, BindingId),
}
/// One source parameter, including its explicit call mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorParameter {
    /// Source parameter name.
    pub name: String,
    /// Canonical source type.
    pub ty: String,
    /// Whether the argument requires its name.
    pub named: bool,
}
/// A source or builtin callable interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorSignature {
    /// Source-call spelling, including any explicitly imported alias.
    pub name: String,
    /// Ordered source parameters.
    pub parameters: Vec<EditorParameter>,
    /// Canonical source return type.
    pub return_type: String,
    /// Effects, permission, and mode requirements.
    pub documentation: String,
}
impl EditorSignature {
    /// Render the declaration using type-first parameter syntax.
    pub fn label(&self) -> String {
        format!(
            "{}({}) -> {}",
            self.name,
            self.parameters
                .iter()
                .map(|parameter| {
                    format!(
                        "{} {}{}",
                        parameter.ty,
                        if parameter.named { "" } else { "_ " },
                        parameter.name
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
            self.return_type
        )
    }
    /// Build an ordered LSP snippet with the declaration's required labels.
    pub fn snippet(&self) -> String {
        format!(
            "{}({})",
            self.name,
            self.parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    format!(
                        "{}${{{}:{}}}",
                        if parameter.named {
                            format!("{}: ", parameter.name)
                        } else {
                            String::new()
                        },
                        index + 1,
                        parameter.name
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
/// One completion candidate selected for the current source and lexical context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorCompletion {
    /// Source spelling shown by the editor.
    pub label: String,
    /// LSP completion kind.
    pub kind: u64,
    /// Canonical type or full callable signature.
    pub detail: String,
    /// Snippet or plain spelling inserted by the editor.
    pub insert_text: String,
    /// Whether insert_text is an LSP snippet.
    pub snippet: bool,
    /// Effects and declaration documentation.
    pub documentation: String,
}
/// One declaration, shared by navigation, hover and completion.
#[derive(Clone, Debug)]
pub struct EditorDefinition {
    /// Resolver identity.
    pub identity: EditorIdentity,
    /// Exact declaration-name range.
    pub source: SourceRange,
    /// Source spelling.
    pub name: String,
    /// LSP completion kind.
    pub kind: u64,
    /// Canonical type or signature.
    pub detail: String,
    /// Callable interface, when applicable.
    pub signature: Option<EditorSignature>,
}
/// A package export that must change atomically with its resolved source declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorExportRename {
    /// Exact package identity; aliases and unrelated JSON strings are not export references.
    pub package: String,
    /// Existing exported declaration name.
    pub old_name: String,
    /// Replacement exported declaration name.
    pub new_name: String,
}
/// Semantically checked rename across source identities and explicit package metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorRename {
    /// Exact identifier ranges, including source call labels and imported references.
    pub sources: Vec<SourceRange>,
    /// Required metadata changes. A frontend must own these manifests before offering any edit.
    pub exports: Vec<EditorExportRename>,
}
#[derive(Clone, Debug)]
struct Occurrence {
    source: SourceRange,
    identity: EditorIdentity,
    declaration: bool,
}
struct EditorUnit {
    file: SourceFile,
    tokens: Vec<Token>,
    facts: AstFacts,
    resolved: Option<ResolvedProgram>,
    imports: Vec<ImportBinding>,
    package: Option<String>,
    exports: BTreeSet<String>,
    binding_types: BTreeMap<BindingId, Type>,
    typed_nodes: Vec<TypedHirNode>,
    signatures: BTreeMap<String, FunctionSignature>,
    error_types:
        BTreeMap<String, iroha_data_model::smart_contract::manifest::ContractErrorTypeDescriptor>,
}
/// A bounded, immutable source-graph snapshot used by every semantic editor operation.
#[derive(Default)]
pub struct EditorSnapshot {
    units: BTreeMap<SourceId, EditorUnit>,
    definitions: BTreeMap<EditorIdentity, EditorDefinition>,
    occurrences: Vec<Occurrence>,
    complete: bool,
    project_request: Option<SourceLinkRequest>,
    zk_enabled: bool,
}
impl EditorSnapshot {
    /// Analyze a loose document without any ambient import authority.
    pub fn single(name: &str, text: &str, zk_enabled: bool) -> Self {
        crate::session::run_with_compiler_stack(|| {
            let mut snapshot = Self {
                complete: true,
                zk_enabled,
                ..Self::default()
            };
            snapshot.add_unit(
                SourceFile::new(SourceId(0), name, text),
                vec![],
                None,
                BTreeSet::new(),
                zk_enabled,
            );
            snapshot.index();
            snapshot
        })
        .unwrap_or_default()
    }
    /// Analyze the exact locked project graph supplied by a frontend, including its source overlays.
    pub fn project(request: &SourceLinkRequest, zk_enabled: bool) -> Self {
        crate::session::run_with_compiler_stack(|| {
            let Ok(request) = ModuleBuildGraph::editor_request(request) else {
                return Self::default();
            };
            let keys = std::iter::once(format!("root\0{}", request.root.source_name))
                .chain(request.packages.iter().flat_map(|package| {
                    package.modules.iter().map(|module| {
                        format!("package\0{}\0{}", package.identity, module.source_name)
                    })
                }))
                .collect::<Vec<_>>();
            let ids = crate::linker::stable_source_ids(&keys);
            let mut snapshot = Self {
                complete: true,
                zk_enabled,
                ..Self::default()
            };
            snapshot.project_request = Some(request.clone());
            snapshot.add_unit(
                SourceFile::new(
                    ids[0],
                    request.root.source_name.as_str(),
                    &request.root.source,
                ),
                request.imports.clone(),
                None,
                BTreeSet::new(),
                zk_enabled,
            );
            let mut index = 1;
            for package in &request.packages {
                for module in &package.modules {
                    snapshot.add_unit(
                        SourceFile::new_in_package(
                            ids[index],
                            package.identity.as_str(),
                            module.source_name.as_str(),
                            &module.source,
                        ),
                        package.imports.clone(),
                        Some(package.identity.clone()),
                        package.exports.clone(),
                        zk_enabled,
                    );
                    index += 1;
                }
            }
            // Successful linking supplies graph-authenticated receiver types, preserving original HIR ids.
            match ModuleBuildGraph::default().link(
                request.clone(),
                LinkerOptions {
                    zk_enabled,
                    ..LinkerOptions::default()
                },
            ) {
                Ok(linked) => {
                    snapshot.complete = snapshot.units.values().all(|unit| unit.resolved.is_some());
                    for crate::semantic::TypedItem::Function(function) in &linked.program.items {
                        if let Some(name_source) = function.name_source
                            && let Some(unit) = snapshot.units.get_mut(&name_source.source)
                            && let Some(resolved) = &unit.resolved
                            && let Some(symbol) = resolved
                                .symbols()
                                .find(|symbol| symbol.source == name_source)
                        {
                            unit.signatures.insert(
                                symbol.name.clone(),
                                FunctionSignature {
                                    params: function.param_types.clone(),
                                    return_type: function.ret_ty.clone().unwrap_or(Type::Unit),
                                    modifiers: function.modifiers.clone(),
                                },
                            );
                        }
                    }
                    for (_, node) in linked.program.hir_nodes {
                        if let Some(unit) = snapshot.units.get_mut(&node.id.source) {
                            if let Some(ResolvedTarget::Value(ResolvedValueTarget::Binding(id))) =
                                node.target
                            {
                                unit.binding_types.insert(id, node.ty.clone());
                            }
                            unit.typed_nodes.push(node);
                        }
                    }
                }
                Err(_) => {
                    snapshot.complete = false;
                    // A body error must not discard receiver types from locked dependencies.
                    // The editor projection exposes facts only; strict linking still failed.
                    if let Some(request) = snapshot.resolved_project_request()
                        && let Ok(facts) = TypedLinker::new(LinkerOptions {
                            zk_enabled,
                            ..LinkerOptions::default()
                        })
                        .analyze_editor_graph(request)
                    {
                        for (source, facts) in facts {
                            if let Some(unit) = snapshot.units.get_mut(&source) {
                                unit.signatures = facts.signatures;
                                unit.binding_types = facts.bindings;
                                unit.typed_nodes = facts.nodes;
                            }
                        }
                    }
                }
            }
            snapshot.index();
            snapshot
        })
        .unwrap_or_default()
    }
    fn resolved_project_request(&self) -> Option<LinkRequest> {
        let request = self.project_request.as_ref()?;
        let module = |package: Option<&str>, name: &str| {
            let unit = self
                .units
                .values()
                .find(|unit| unit.package.as_deref() == package && unit.file.name() == name)?;
            Some(ModuleUnit {
                source_name: name.to_owned(),
                program: unit.resolved.clone()?,
            })
        };
        Some(LinkRequest {
            root: module(None, &request.root.source_name)?,
            imports: request.imports.clone(),
            packages: request
                .packages
                .iter()
                .map(|package| {
                    Some(PackageUnit {
                        identity: package.identity.clone(),
                        modules: package
                            .modules
                            .iter()
                            .map(|source| module(Some(&package.identity), &source.source_name))
                            .collect::<Option<Vec<_>>>()?,
                        exports: package.exports.clone(),
                        imports: package.imports.clone(),
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        })
    }
    fn add_unit(
        &mut self,
        file: SourceFile,
        imports: Vec<ImportBinding>,
        package: Option<String>,
        exports: BTreeSet<String>,
        zk_enabled: bool,
    ) {
        let budget = FrontendBudget::v1();
        let (tokens, _) =
            crate::lexer::lower_lexed_recovering(&file, budget, crate::syntax::lex(&file, budget));
        let parsed = crate::parser::parse_source_spanned(&file, budget)
            .ok()
            .map(|(parsed, _)| parsed);
        let facts = parsed
            .as_ref()
            .map(|parsed| parsed.facts.clone())
            .unwrap_or_else(|| crate::parser::editor_source_facts(&file, &tokens));
        let aliases = imports
            .iter()
            .map(|import| (import.alias.clone(), ()))
            .collect();
        let resolved = parsed
            .and_then(|parsed| crate::resolved::resolve_with_imports(parsed, &file, &aliases).ok());
        let signatures = resolved
            .as_ref()
            .and_then(|resolved| {
                SemanticContext::with_capabilities(zk_enabled, true)
                    .resolve_resolved_function_signatures(resolved)
                    .ok()
            })
            .unwrap_or_default();
        let mut binding_types = BTreeMap::new();
        let mut typed_nodes = Vec::new();
        let mut error_types = BTreeMap::new();
        if let Some(resolved) = &resolved {
            let (typed, bindings, nodes) = SemanticContext::with_capabilities(zk_enabled, true)
                .analyze_editor(resolved, BTreeMap::new(), BTreeMap::new());
            binding_types = bindings;
            typed_nodes = nodes;
            if let Ok(program) = &typed {
                for error in &program.error_types {
                    if let Some(name) = error.identity.rsplit("::").next() {
                        error_types.insert(name.to_owned(), error.clone());
                    }
                }
            }
            if typed.is_err() {
                self.complete = false;
            }
        } else {
            self.complete = false;
        }
        self.units.insert(
            file.id(),
            EditorUnit {
                file,
                tokens,
                facts,
                resolved,
                imports,
                package,
                exports,
                binding_types,
                typed_nodes,
                signatures,
                error_types,
            },
        );
    }
    fn index(&mut self) {
        for (source, unit) in &self.units {
            let Some(resolved) = &unit.resolved else {
                continue;
            };
            let signatures = &unit.signatures;
            for symbol in resolved.symbols() {
                let identity = EditorIdentity::Symbol(*source, symbol.id);
                let signature = signatures
                    .get(&symbol.name)
                    .map(|signature| source_signature(&symbol.name, signature));
                let detail = signature
                    .as_ref()
                    .map(EditorSignature::label)
                    .unwrap_or_else(|| format!("{:?} {}", symbol.kind, symbol.name));
                let kind = match symbol.kind {
                    ResolvedSymbolKind::Function => 3,
                    ResolvedSymbolKind::Struct => 22,
                    ResolvedSymbolKind::ErrorEnum => 13,
                    ResolvedSymbolKind::State => 6,
                    ResolvedSymbolKind::Const => 21,
                    _ => 9,
                };
                self.definitions.insert(
                    identity,
                    EditorDefinition {
                        identity,
                        source: symbol.source,
                        name: symbol.name.clone(),
                        kind,
                        detail,
                        signature,
                    },
                );
                self.occurrences.push(Occurrence {
                    source: symbol.source,
                    identity,
                    declaration: true,
                });
            }
            for binding in resolved.bindings() {
                let Some(range) = binding.source else {
                    continue;
                };
                let identity = EditorIdentity::Binding(*source, binding.id);
                let detail = unit
                    .binding_types
                    .get(&binding.id)
                    .map(render_type_name)
                    .unwrap_or_else(|| "binding".to_owned());
                self.definitions.insert(
                    identity,
                    EditorDefinition {
                        identity,
                        source: range,
                        name: binding.name.clone(),
                        kind: 6,
                        detail,
                        signature: None,
                    },
                );
                self.occurrences.push(Occurrence {
                    source: range,
                    identity,
                    declaration: true,
                });
            }
            for node in resolved.arena().nodes() {
                if let (Some(range), Some(identity)) = (
                    node.source,
                    node.target.and_then(|target| local_target(*source, target)),
                ) {
                    // Calls carry full expression ranges in HIR; their exact names are indexed below.
                    if !matches!(node.target, Some(ResolvedTarget::Call(_))) {
                        let name = self
                            .definitions
                            .get(&identity)
                            .map(|definition| definition.name.as_str());
                        if let Some(token) = unit
                            .tokens
                            .iter()
                            .find(|token| token.range.start == range.range.start)
                            && matches!(&token.kind, TokenKind::Ident(value) if Some(value.as_str()) == name)
                        {
                            self.occurrences.push(Occurrence {
                                source: SourceRange::new(*source, token.range),
                                identity,
                                declaration: false,
                            });
                        }
                    }
                }
            }
        }
        for (source, unit) in &self.units {
            let Some(resolved) = &unit.resolved else {
                continue;
            };
            for node in resolved.arena().nodes() {
                if let Some(ResolvedTarget::Value(
                    target @ (ResolvedValueTarget::ErrorCode(_)
                    | ResolvedValueTarget::ImportedErrorVariant),
                )) = node.target
                    && let Some(source) = node.source
                    && let Some((namespace, range)) = error_namespace_source(unit, source)
                {
                    let identity = match target {
                        ResolvedValueTarget::ErrorCode(_) => resolved
                            .symbols()
                            .find(|symbol| {
                                symbol.kind == ResolvedSymbolKind::ErrorEnum
                                    && symbol.name == namespace
                            })
                            .map(|symbol| EditorIdentity::Symbol(unit.file.id(), symbol.id)),
                        ResolvedValueTarget::ImportedErrorVariant => {
                            self.imported_identity(unit, &namespace)
                        }
                        _ => None,
                    };
                    if let Some(identity) = identity {
                        self.occurrences.push(Occurrence {
                            source: range,
                            identity,
                            declaration: false,
                        });
                    }
                }
            }
            for call in resolved.calls() {
                let identity = match call.target {
                    ResolvedCallTarget::Function(id) | ResolvedCallTarget::Struct(id) => {
                        Some(EditorIdentity::Symbol(*source, id))
                    }
                    ResolvedCallTarget::External => self.imported_identity(unit, &call.name),
                    _ => None,
                };
                if let Some(identity) = identity {
                    let range = terminal_name_range(&unit.file, call.name_source);
                    self.occurrences.push(Occurrence {
                        source: range,
                        identity,
                        declaration: false,
                    });
                    if let Some(definition) = self.definitions.get(&identity)
                        && let Some(signature) = &definition.signature
                        && let Some(target_unit) = self.units.get(&definition.source.source)
                        && let Some(target) = &target_unit.resolved
                    {
                        for label in call.argument_name_sources.iter().flatten() {
                            let Some(name) = unit.file.slice(label.range) else {
                                continue;
                            };
                            if !signature
                                .parameters
                                .iter()
                                .any(|parameter| parameter.name == name && parameter.named)
                            {
                                continue;
                            }
                            if let Some(range) =
                                target.parameter_name_source(&definition.name, name)
                                && let Some(binding) = target
                                    .bindings()
                                    .find(|binding| binding.source == Some(range))
                            {
                                self.occurrences.push(Occurrence {
                                    source: *label,
                                    identity: EditorIdentity::Binding(
                                        definition.source.source,
                                        binding.id,
                                    ),
                                    declaration: false,
                                });
                            }
                        }
                    }
                }
            }
            for ty in resolved.types() {
                let identity = match ty.target {
                    ResolvedTypeTarget::ExternalType => self.imported_identity(unit, &ty.name),
                    ResolvedTypeTarget::Struct(id) | ResolvedTypeTarget::ErrorEnum(id) => {
                        Some(EditorIdentity::Symbol(*source, id))
                    }
                    _ => None,
                };
                if let Some(identity) = identity {
                    self.occurrences.push(Occurrence {
                        source: terminal_name_range(&unit.file, ty.source),
                        identity,
                        declaration: false,
                    });
                }
            }
        }
        self.occurrences
            .sort_by_key(|occurrence| (occurrence.source, occurrence.identity));
        self.occurrences
            .dedup_by_key(|occurrence| (occurrence.source, occurrence.identity));
    }
    fn imported_identity(&self, unit: &EditorUnit, path: &str) -> Option<EditorIdentity> {
        let (alias, name) = path.split_once("::")?;
        if name.contains("::") {
            return None;
        }
        let import = unit.imports.iter().find(|import| import.alias == alias)?;
        let mut found = self
            .units
            .values()
            .filter(|candidate| {
                candidate.package.as_deref() == Some(&import.package)
                    && candidate.exports.contains(name)
            })
            .filter_map(|candidate| {
                candidate
                    .resolved
                    .as_ref()?
                    .symbols()
                    .find(|symbol| symbol.name == name)
                    .map(|symbol| EditorIdentity::Symbol(candidate.file.id(), symbol.id))
            });
        let identity = found.next()?;
        found.next().is_none().then_some(identity)
    }
    /// All immutable source files in this snapshot.
    pub fn sources(&self) -> impl Iterator<Item = &SourceFile> {
        self.units.values().map(|unit| &unit.file)
    }
    /// Exact file by source identity.
    pub fn source(&self, id: SourceId) -> Option<&SourceFile> {
        self.units.get(&id).map(|unit| &unit.file)
    }
    /// Return declaration-site callable signatures for source documentation.
    /// Modes come from resolved compiler interfaces, including locked imports;
    /// public JSON record schemas intentionally do not encode those modes.
    pub fn declaration_signatures(&self, source: SourceId) -> Vec<EditorSignature> {
        let mut signatures = self
            .units
            .get(&source)
            .map(|unit| {
                unit.signatures
                    .iter()
                    .map(|(name, signature)| source_signature(name, signature))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        signatures.sort_by(|left, right| left.name.cmp(&right.name));
        signatures
    }
    /// Whether every source has complete, successful semantic analysis.
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
    /// Resolve the smallest exact identifier under a cursor.
    pub fn definition(&self, source: SourceId, offset: u32) -> Option<&EditorDefinition> {
        let occurrence = self
            .occurrences
            .iter()
            .filter(|occurrence| {
                occurrence.source.source == source && contains(occurrence.source.range, offset)
            })
            .min_by_key(|occurrence| occurrence.source.range.end - occurrence.source.range.start)?;
        self.definitions.get(&occurrence.identity)
    }
    /// Canonical hover text from a resolved declaration, call, or typed expression.
    pub fn hover(&self, source: SourceId, offset: u32) -> Option<(String, String)> {
        let unit = self.units.get(&source)?;
        if let Some(definition) = self.definition(source, offset) {
            let ty = unit
                .typed_nodes
                .iter()
                .filter(|node| {
                    node.source
                        .is_some_and(|source| contains(source.range, offset))
                })
                .min_by_key(|node| {
                    node.source
                        .map_or(u32::MAX, |source| source.range.end - source.range.start)
                });
            let detail = if definition.signature.is_some() {
                definition.detail.clone()
            } else {
                ty.map_or_else(
                    || definition.detail.clone(),
                    |node| format!("{}: {}", definition.name, render_type_name(&node.ty)),
                )
            };
            return Some((
                detail,
                definition
                    .signature
                    .as_ref()
                    .map_or_else(String::new, |signature| signature.documentation.clone()),
            ));
        }
        if let Some(resolved) = &unit.resolved
            && let Some(call) = resolved
                .calls()
                .find(|call| contains(call.name_source.range, offset))
            && let Some(signature) =
                self.signature_for_name(unit, &call.name, call.name_source.range.end)
        {
            return Some((signature.label(), signature.documentation));
        }
        unit.typed_nodes
            .iter()
            .filter(|node| {
                node.source
                    .is_some_and(|source| contains(source.range, offset))
            })
            .min_by_key(|node| {
                node.source
                    .map_or(u32::MAX, |source| source.range.end - source.range.start)
            })
            .map(|node| (render_type_name(&node.ty), String::new()))
    }
    /// Return every identity-bound source use, optionally including its declaration.
    pub fn references(
        &self,
        source: SourceId,
        offset: u32,
        declarations: bool,
    ) -> Vec<SourceRange> {
        let Some(definition) = self.definition(source, offset) else {
            return vec![];
        };
        self.occurrences
            .iter()
            .filter(|occurrence| {
                occurrence.identity == definition.identity
                    && (declarations || !occurrence.declaration)
            })
            .map(|occurrence| occurrence.source)
            .collect()
    }
    /// Plan an atomic source/export rename for a complete graph and collision-free identifier.
    /// Frontends must authorize and bind every export edit to its exact owned manifest snapshot.
    pub fn rename(
        &self,
        source: SourceId,
        offset: u32,
        name: &str,
    ) -> Result<EditorRename, String> {
        if !self.complete {
            return Err("Rename requires a complete, successfully checked source graph.".into());
        }
        let tokens =
            crate::lexer::lex(name).map_err(|_| "Rename requires one Kotodama identifier.")?;
        if !matches!(tokens.as_slice(), [Token { kind: TokenKind::Ident(value), .. }, Token { kind: TokenKind::EOF, .. }] if value == name)
            || crate::semantic::is_reserved_source_declaration(name, false)
        {
            return Err("Rename requires an available Kotodama identifier.".into());
        }
        let definition = self
            .definition(source, offset)
            .ok_or("No resolved declaration at this position.")?;
        if definition.kind == 9 {
            return Err("Source-unit names are part of package and contract identity; rename their manifest explicitly.".into());
        }
        let exports = self
            .units
            .get(&definition.source.source)
            .filter(|unit| {
                matches!(definition.identity, EditorIdentity::Symbol(..))
                    && unit.exports.contains(&definition.name)
            })
            .and_then(|unit| unit.package.as_ref())
            .map(|package| EditorExportRename {
                package: package.clone(),
                old_name: definition.name.clone(),
                new_name: name.into(),
            })
            .into_iter()
            .collect::<Vec<_>>();
        let ranges = self.references(source, offset, true);
        let mut rewritten = BTreeMap::new();
        for (id, unit) in &self.units {
            let mut text = unit.file.text().to_owned();
            let mut edits = ranges
                .iter()
                .filter(|range| range.source == *id)
                .collect::<Vec<_>>();
            edits.sort_by_key(|range| std::cmp::Reverse(range.range.start));
            for range in edits {
                text.replace_range(range.range.start as usize..range.range.end as usize, name);
            }
            rewritten.insert(*id, text);
        }
        let checked = if let Some(request) = &self.project_request {
            let mut request = request.clone();
            for export in &exports {
                let package = request
                    .packages
                    .iter_mut()
                    .find(|package| package.identity == export.package)
                    .ok_or("Rename export package is unavailable.")?;
                if export.old_name != export.new_name && package.exports.contains(&export.new_name)
                {
                    return Err(format!(
                        "`{name}` is already exported by `{}`.",
                        export.package
                    ));
                }
                if !package.exports.remove(&export.old_name) {
                    return Err("Rename export no longer matches its checked graph.".into());
                }
                package.exports.insert(export.new_name.clone());
            }
            for (id, unit) in &self.units {
                let replacement = rewritten.get(id).expect("rewritten source");
                if let Some(package) = &unit.package {
                    for module in request
                        .packages
                        .iter_mut()
                        .filter(|candidate| &candidate.identity == package)
                        .flat_map(|package| &mut package.modules)
                    {
                        if module.source_name == unit.file.name() {
                            module.source.clone_from(replacement);
                        }
                    }
                } else {
                    request.root.source.clone_from(replacement);
                }
            }
            Self::project(&request, self.zk_enabled)
        } else {
            let (id, unit) = self
                .units
                .first_key_value()
                .ok_or("Rename source is unavailable.")?;
            Self::single(unit.file.name(), &rewritten[id], self.zk_enabled)
        };
        if !checked.is_complete() {
            return Err("The proposed source rename does not pass semantic checks; update the associated syntax or metadata explicitly.".into());
        }
        // A graph can still type-check after accidental capture or shadowing. Preserve every
        // resolver identity after translating offsets through exactly the proposed edits.
        // This permits unrelated declarations in disjoint scopes without accepting rebinding.
        let mut deltas = BTreeMap::<SourceId, Vec<(u32, i64)>>::new();
        for range in &ranges {
            deltas.entry(range.source).or_default().push((
                range.range.end,
                name.len() as i64 - i64::from(range.range.end - range.range.start),
            ));
        }
        for edits in deltas.values_mut() {
            edits.sort_unstable_by_key(|(end, _)| *end);
            let mut cumulative = 0_i64;
            for (_, delta) in edits {
                cumulative = cumulative
                    .checked_add(*delta)
                    .ok_or("Rename offset overflow.")?;
                *delta = cumulative;
            }
        }
        let checked_occurrences = checked
            .occurrences
            .iter()
            .map(|occurrence| {
                (
                    occurrence.source.source,
                    occurrence.source.range.start,
                    occurrence.source.range.end,
                    occurrence.identity,
                    occurrence.declaration,
                )
            })
            .collect::<BTreeSet<_>>();
        for occurrence in &self.occurrences {
            let shift = |offset: u32| -> Option<u32> {
                let delta = deltas.get(&occurrence.source.source).map_or(0, |edits| {
                    let count = edits.partition_point(|(end, _)| *end <= offset);
                    count.checked_sub(1).map_or(0, |index| edits[index].1)
                });
                u32::try_from(i64::from(offset).checked_add(delta)?).ok()
            };
            let start = shift(occurrence.source.range.start).ok_or("Rename offset overflow.")?;
            let end = shift(occurrence.source.range.end).ok_or("Rename offset overflow.")?;
            if !checked_occurrences.contains(&(
                occurrence.source.source,
                start,
                end,
                occurrence.identity,
                occurrence.declaration,
            )) {
                return Err(
                    "Rename would capture, shadow, or redirect a resolved reference.".into(),
                );
            }
        }
        Ok(EditorRename {
            sources: ranges,
            exports,
        })
    }
    /// Determine the enclosing callable and active source-order argument.
    pub fn signature_help(
        &self,
        source: SourceId,
        offset: u32,
    ) -> Option<(EditorSignature, usize)> {
        let unit = self.units.get(&source)?;
        let (name, opening, active) = call_context(&unit.tokens, offset)?;
        let signature = self.signature_for_name(unit, &name, opening)?;
        let (_, label, _) = argument_context(&unit.tokens, opening, offset);
        let active = label
            .and_then(|label| {
                signature
                    .parameters
                    .iter()
                    .position(|parameter| parameter.name == label)
            })
            .unwrap_or(active);
        Some((signature, active))
    }
    fn signature_for_name(
        &self,
        unit: &EditorUnit,
        name: &str,
        offset: u32,
    ) -> Option<EditorSignature> {
        if let Some(signature) = intrinsic_signatures()
            .into_iter()
            .find(|signature| signature.name == name)
        {
            return Some(signature);
        }
        if let Some(builtin) = Builtin::from_source_name(name) {
            return Some(builtin_signature(builtin, false));
        }
        if let Some(identity) = self.imported_identity(unit, name) {
            let mut signature = self.definitions.get(&identity)?.signature.clone()?;
            signature.name = name.to_owned();
            return Some(signature);
        }
        self.definitions
            .values()
            .find(|definition| {
                definition.source.source == unit.file.id()
                    && definition.name == name
                    && definition.signature.is_some()
            })
            .and_then(|definition| definition.signature.clone())
            .or_else(|| {
                let receiver = receiver_before(&unit.tokens, offset)?;
                let ty = self.type_at(unit, receiver)?;
                member_signatures(ty)
                    .into_iter()
                    .find(|signature| signature.name == name)
            })
    }
    fn type_at<'a>(&'a self, unit: &'a EditorUnit, range: TextRange) -> Option<&'a Type> {
        unit.typed_nodes
            .iter()
            .filter(|node| node.source.is_some_and(|source| source.range == range))
            .map(|node| &node.ty)
            .next()
            .or_else(|| {
                let definition = self.definition(unit.file.id(), range.start)?;
                let EditorIdentity::Binding(_, binding) = definition.identity else {
                    return None;
                };
                unit.binding_types.get(&binding)
            })
    }
    /// Contextual completion using lexical bindings, locked exports, receiver types, and call labels.
    pub fn completions(&self, source: SourceId, offset: u32) -> Vec<EditorCompletion> {
        self.completions_inner(source, offset, true)
    }
    fn completions_inner(
        &self,
        source: SourceId,
        offset: u32,
        recover: bool,
    ) -> Vec<EditorCompletion> {
        let Some(unit) = self.units.get(&source) else {
            return vec![];
        };
        if recover
            && unit.resolved.is_none()
            && let Some(repaired) = completion_repair(&unit.file, &unit.tokens, offset)
        {
            let recovery = if let Some(request) = &self.project_request {
                let mut request = request.clone();
                if let Some(package) = &unit.package {
                    if let Some(module) = request
                        .packages
                        .iter_mut()
                        .filter(|candidate| &candidate.identity == package)
                        .flat_map(|package| &mut package.modules)
                        .find(|module| module.source_name == unit.file.name())
                    {
                        module.source = repaired;
                    }
                } else {
                    request.root.source = repaired;
                }
                Self::project(&request, self.zk_enabled)
            } else {
                Self::single(unit.file.name(), &repaired, self.zk_enabled)
            };
            let candidates = recovery.completions_inner(source, offset, false);
            if !candidates.is_empty() {
                return candidates;
            }
        }
        if let Some(receiver) = receiver_before(&unit.tokens, offset) {
            return self
                .type_at(unit, receiver)
                .map(|ty| {
                    let mut candidates = member_signatures(ty)
                        .into_iter()
                        .map(signature_completion)
                        .collect::<Vec<_>>();
                    if let Type::Struct { fields, .. } = ty {
                        candidates.extend(
                            fields
                                .iter()
                                .map(|(name, ty)| plain_completion(name, 5, &render_type_name(ty))),
                        );
                    }
                    candidates
                })
                .unwrap_or_default();
        }
        let prefix = path_prefix(&unit.tokens, offset);
        if let Some((namespace, _)) = prefix.rsplit_once("::") {
            let alias = namespace.split("::").next().unwrap_or(namespace);
            let mut candidates = Vec::new();
            if let Some(import) = unit.imports.iter().find(|import| import.alias == alias) {
                if namespace != alias {
                    if let Some(identity) = self.imported_identity(unit, namespace)
                        && let Some(definition) = self.definitions.get(&identity)
                        && definition.kind == 13
                        && let Some(descriptor) = self
                            .units
                            .get(&definition.source.source)
                            .and_then(|owner| owner.error_types.get(&definition.name))
                    {
                        for variant in &descriptor.variants {
                            candidates.push(plain_completion(
                                &variant.name,
                                20,
                                &format!("{namespace}::{} = {}", variant.name, variant.code),
                            ));
                        }
                    }
                    return candidates;
                }
                for definition in self.definitions.values().filter(|definition| {
                    self.units
                        .get(&definition.source.source)
                        .is_some_and(|candidate| {
                            candidate.package.as_deref() == Some(&import.package)
                                && candidate.exports.contains(&definition.name)
                        })
                }) {
                    candidates.push(definition_completion(definition));
                }
                return candidates;
            }
            for path in crate::semantic::V1_SUM_PATHS
                .iter()
                .chain(crate::semantic::V1_ROUNDING_PATHS)
            {
                if let Some(suffix) = path.strip_prefix(&format!("{namespace}::")) {
                    candidates.push(plain_completion(suffix, 20, path));
                }
            }
            for descriptor in [
                ivm_abi::error_types::list_error_type(),
                ivm_abi::error_types::numeric_error_type(),
            ] {
                if descriptor.identity.rsplit("::").next() == Some(namespace) {
                    for variant in descriptor.variants {
                        candidates.push(plain_completion(
                            &variant.name,
                            20,
                            &format!("{namespace}::{} = {}", variant.name, variant.code),
                        ));
                    }
                }
            }
            for declaration in unit.facts.declarations.iter().filter(|declaration| {
                declaration.kind == DeclarationKind::ErrorEnum && declaration.name == namespace
            }) {
                if let Some(node) = unit.facts.source_map.node(declaration.node)
                    && let Some(source) = unit.file.slice(node.range)
                    && let Ok(program) =
                        crate::parser::parse(&format!("module Editor {{ {source} }}"))
                {
                    for item in program.items {
                        if let crate::ast::Item::ErrorEnum(error) = item {
                            for variant in error.variants {
                                candidates.push(plain_completion(
                                    &variant.name,
                                    20,
                                    &format!("{namespace}::{} = {}", variant.name, variant.code),
                                ));
                            }
                        }
                    }
                }
            }
            for mut signature in intrinsic_signatures() {
                if let Some(suffix) = signature.name.strip_prefix(&format!("{namespace}::")) {
                    signature.name = suffix.to_owned();
                    candidates.push(signature_completion(signature));
                }
            }
            for (builtin, spec) in Builtin::registry() {
                if builtin_visible(unit, builtin, offset, self.zk_enabled)
                    && spec.surface != BuiltinSurface::CompilerInternal
                    && spec.name.starts_with(&format!("{namespace}::"))
                {
                    let mut signature = builtin_signature(builtin, false);
                    signature.name = spec.name[namespace.len() + 2..].to_owned();
                    candidates.push(signature_completion(signature));
                }
            }
            return candidates;
        }
        if let Some((signature, _)) = self.signature_help(source, offset)
            && let Some((_, opening, _)) = call_context(&unit.tokens, offset)
            && argument_context(&unit.tokens, opening, offset).1.is_none()
        {
            let (_, opening, _) = call_context(&unit.tokens, offset).expect("signature context");
            let (_, _, existing) = argument_context(&unit.tokens, opening, offset);
            let labels = signature
                .parameters
                .iter()
                .filter(|parameter| parameter.named && !existing.contains(parameter.name.as_str()))
                .map(|parameter| EditorCompletion {
                    label: format!("{}:", parameter.name),
                    kind: 5,
                    detail: parameter.ty.clone(),
                    insert_text: format!("{}: ${{1:{}}}", parameter.name, parameter.name),
                    snippet: true,
                    documentation: String::new(),
                })
                .collect::<Vec<_>>();
            if !labels.is_empty() {
                return labels;
            }
        }
        let mut candidates = BTreeMap::new();
        for definition in self
            .definitions
            .values()
            .filter(|definition| definition.source.source == source)
        {
            let visible = match definition.identity {
                EditorIdentity::Symbol(..) => true,
                EditorIdentity::Binding(_, binding) => visible_binding(unit, binding, offset),
            };
            if visible {
                candidates.insert(definition.name.clone(), definition_completion(definition));
            }
        }
        // Only parser-owned declarations are available after syntax failure. They are candidates,
        // never promoted into resolved identities or used by rename/navigation.
        if unit.resolved.is_none() {
            for declaration in &unit.facts.declarations {
                if declaration.kind != DeclarationKind::Parameter
                    || declaration.owner.is_some_and(|owner| {
                        unit.facts
                            .source_map
                            .node(owner)
                            .is_some_and(|node| contains_open(node.range, offset))
                    })
                {
                    candidates
                        .entry(declaration.name.clone())
                        .or_insert_with(|| {
                            plain_completion(
                                &declaration.name,
                                if declaration.kind == DeclarationKind::Function {
                                    3
                                } else {
                                    6
                                },
                                "declaration (incomplete source)",
                            )
                        });
                }
            }
        }
        for &keyword in crate::lexer::V1_KEYWORDS {
            candidates
                .entry(keyword.to_owned())
                .or_insert_with(|| plain_completion(keyword, 14, "Kotodama keyword"));
        }
        for &name in crate::semantic::V1_SOURCE_TYPE_NAMES {
            candidates
                .entry(name.to_owned())
                .or_insert_with(|| plain_completion(name, 7, "Kotodama type"));
        }
        for signature in intrinsic_signatures() {
            candidates.insert(signature.name.clone(), signature_completion(signature));
        }
        for (builtin, spec) in Builtin::registry() {
            if builtin_visible(unit, builtin, offset, self.zk_enabled)
                && matches!(
                    spec.surface,
                    BuiltinSurface::Function | BuiltinSurface::FunctionOrMethod
                )
            {
                candidates
                    .entry(spec.name.to_owned())
                    .or_insert_with(|| signature_completion(builtin_signature(builtin, false)));
            }
        }
        candidates.into_values().collect()
    }
}
fn local_target(source: SourceId, target: ResolvedTarget) -> Option<EditorIdentity> {
    Some(match target {
        ResolvedTarget::Value(ResolvedValueTarget::Binding(id))
        | ResolvedTarget::Assignment(ResolvedValueTarget::Binding(id)) => {
            EditorIdentity::Binding(source, id)
        }
        ResolvedTarget::Value(ResolvedValueTarget::State(id) | ResolvedValueTarget::Const(id))
        | ResolvedTarget::Assignment(
            ResolvedValueTarget::State(id) | ResolvedValueTarget::Const(id),
        )
        | ResolvedTarget::Type(
            ResolvedTypeTarget::Struct(id) | ResolvedTypeTarget::ErrorEnum(id),
        )
        | ResolvedTarget::StructLiteral(id) => EditorIdentity::Symbol(source, id),
        _ => return None,
    })
}
// The resolver authenticated this value as a nominal variant. Read only its exact
// source-backed path tokens; numeric codes alone cannot identify the declaring enum.
fn error_namespace_source(unit: &EditorUnit, source: SourceRange) -> Option<(String, SourceRange)> {
    let start = unit
        .tokens
        .partition_point(|token| token.range.start < source.range.start);
    let end = unit
        .tokens
        .partition_point(|token| token.range.end <= source.range.end);
    let mut tokens = unit.tokens.get(start..end)?;
    while matches!(tokens.first()?.kind, TokenKind::LParen)
        && matches!(tokens.last()?.kind, TokenKind::RParen)
    {
        tokens = &tokens[1..tokens.len() - 1];
    }
    let (namespace, name) = match tokens {
        [first, separator, variant]
            if matches!(separator.kind, TokenKind::ColonColon)
                && matches!(variant.kind, TokenKind::Ident(_)) =>
        {
            let TokenKind::Ident(namespace) = &first.kind else {
                return None;
            };
            (namespace.clone(), first)
        }
        [alias, first_separator, name, second_separator, variant]
            if matches!(first_separator.kind, TokenKind::ColonColon)
                && matches!(second_separator.kind, TokenKind::ColonColon)
                && matches!(variant.kind, TokenKind::Ident(_)) =>
        {
            let (TokenKind::Ident(alias), TokenKind::Ident(namespace)) = (&alias.kind, &name.kind)
            else {
                return None;
            };
            (format!("{alias}::{namespace}"), name)
        }
        _ => return None,
    };
    Some((namespace, SourceRange::new(source.source, name.range)))
}
fn contains(range: TextRange, offset: u32) -> bool {
    range.start <= offset && offset < range.end
}
fn contains_open(range: TextRange, offset: u32) -> bool {
    range.start <= offset && (offset <= range.end || range.is_empty())
}
fn terminal_name_range(file: &SourceFile, source: SourceRange) -> SourceRange {
    let offset = file
        .slice(source.range)
        .and_then(|text| text.rfind("::"))
        .map_or(0, |offset| offset + 2);
    SourceRange::new(
        source.source,
        TextRange::new(source.range.start + offset as u32, source.range.end),
    )
}
fn visible_binding(unit: &EditorUnit, id: BindingId, offset: u32) -> bool {
    let Some(resolved) = &unit.resolved else {
        return false;
    };
    let arena = resolved.arena();
    let Some(binding) = arena.binding(id) else {
        return false;
    };
    if binding
        .source
        .is_some_and(|source| source.range.start > offset)
    {
        return false;
    }
    if let Some(owner) = binding
        .source_node
        .and_then(|node| unit.facts.source_map.node(node))
        .and_then(|node| node.owner)
        && unit
            .facts
            .source_map
            .node(owner)
            .is_some_and(|node| !contains_open(node.range, offset))
    {
        return false;
    }
    let scope_range = binding
        .source
        .and_then(|source| enclosing_brace(&unit.tokens, source.range.start));
    if let Some(range) = scope_range
        && !contains_open(range, offset)
    {
        return false;
    }
    let node = arena
        .nodes()
        .filter(|node| {
            node.source
                .is_some_and(|source| contains_open(source.range, offset))
        })
        .min_by_key(|node| {
            node.source
                .map_or(u32::MAX, |source| source.range.end - source.range.start)
        });
    if let Some(node) = node {
        return arena.binding_visible_at(id, node.id);
    }
    // Whitespace has no expression node. Bindings are still restricted to their parser-token
    // block and declaring function; exact-name use resolution remains solely the resolver's job.
    scope_range.is_some()
}
fn enclosing_brace(tokens: &[Token], offset: u32) -> Option<TextRange> {
    let mut stack = Vec::new();
    let mut ranges = Vec::new();
    for token in tokens {
        match token.kind {
            TokenKind::LBrace => stack.push(token.range.start),
            TokenKind::RBrace => {
                if let Some(start) = stack.pop() {
                    ranges.push(TextRange::new(start, token.range.end));
                }
            }
            _ => {}
        }
    }
    let end = tokens.last().map_or(offset, |token| token.range.end);
    ranges.extend(stack.into_iter().map(|start| TextRange::new(start, end)));
    ranges
        .into_iter()
        .filter(|range| contains_open(*range, offset))
        .min_by_key(|range| range.end - range.start)
}
fn source_signature(name: &str, signature: &FunctionSignature) -> EditorSignature {
    EditorSignature {
        name: name.into(),
        parameters: signature
            .params
            .iter()
            .map(|parameter| EditorParameter {
                name: parameter.name.clone(),
                ty: render_type_name(&parameter.ty),
                named: parameter.call_mode == ParameterCallMode::Named,
            })
            .collect(),
        return_type: render_type_name(&signature.return_type),
        documentation: format!(
            "{:?}; authorization: {:?}",
            signature.modifiers.kind, signature.modifiers.permission
        ),
    }
}
fn builtin_signature(builtin: Builtin, receiver: bool) -> EditorSignature {
    let signature = builtin.signature();
    let positional = match builtin.call_policy() {
        BuiltinCallPolicy::Named => 0,
        BuiltinCallPolicy::PositionalPrefix(count) => count,
    };
    EditorSignature {
        name: if receiver {
            builtin.name()
        } else {
            builtin.source_name()
        }
        .into(),
        parameters: signature
            .parameter_names
            .iter()
            .zip(signature.parameters)
            .enumerate()
            .skip(usize::from(receiver))
            .map(|(index, (name, ty))| EditorParameter {
                name: (*name).into(),
                ty: (*ty).into(),
                named: index >= positional,
            })
            .collect(),
        return_type: signature.return_type.into(),
        documentation: format!(
            "Effects: {:?}; access: {:?}; mode: {:?}",
            builtin.effects(),
            builtin.access(),
            builtin.mode()
        ),
    }
}
fn builtin_visible(unit: &EditorUnit, builtin: Builtin, offset: u32, zk: bool) -> bool {
    match builtin.mode() {
        BuiltinMode::Any => true,
        BuiltinMode::ZkOnly => zk,
        BuiltinMode::CompilerInternal => false,
        BuiltinMode::TestOnly | BuiltinMode::TestFunctionOnly => {
            unit.facts.declarations.iter().any(|declaration| {
                declaration.kind == DeclarationKind::Function
                    && unit
                        .signatures
                        .get(&declaration.name)
                        .is_some_and(|signature| signature.modifiers.is_test)
                    && unit
                        .facts
                        .source_map
                        .node(declaration.node)
                        .is_some_and(|node| contains_open(node.range, offset))
            })
        }
    }
}
fn editor_signature(
    name: &str,
    parameters: Vec<(&str, String, bool)>,
    return_type: String,
    documentation: &str,
) -> EditorSignature {
    EditorSignature {
        name: name.into(),
        parameters: parameters
            .into_iter()
            .map(|(name, ty, named)| EditorParameter {
                name: name.into(),
                ty,
                named,
            })
            .collect(),
        return_type,
        documentation: documentation.into(),
    }
}
fn intrinsic_signatures() -> Vec<EditorSignature> {
    let mut signatures = [
        ("decimal::from_int", "int", "decimal"),
        ("decimal::from_quantity", "quantity", "decimal"),
        ("decimal::to_int_exact", "decimal", "int"),
        ("decimal::to_int_trunc", "decimal", "int"),
        (
            "quantity::try_from_int",
            "int",
            "Result<quantity, NumericError>",
        ),
        (
            "quantity::try_from_decimal",
            "decimal",
            "Result<quantity, NumericError>",
        ),
    ]
    .into_iter()
    .map(|(name, input, output)| {
        editor_signature(
            name,
            vec![("value", input.into(), false)],
            output.into(),
            "Canonical numeric conversion; recoverable conversion returns NumericError.",
        )
    })
    .collect::<Vec<_>>();
    signatures.push(editor_signature(
        "decimal::to_int_round",
        vec![
            ("value", "decimal".into(), false),
            ("mode", "Rounding".into(), true),
        ],
        "int".into(),
        "Round once using the explicit Rounding mode.",
    ));
    signatures
}
fn member_signatures(ty: &Type) -> Vec<EditorSignature> {
    if let Type::List(element, _) = ty {
        let element = render_type_name(element);
        return [
            ("len", vec![], "int".into()),
            ("pop", vec![], format!("Option<{element}>")),
            (
                "contains",
                vec![("value", element.clone(), false)],
                "bool".into(),
            ),
            (
                "take",
                vec![("limit", "int".into(), false)],
                render_type_name(ty),
            ),
            (
                "enumerate",
                vec![],
                format!(
                    "List<(int, {element}), {}>",
                    if let Type::List(_, capacity) = ty {
                        *capacity
                    } else {
                        0
                    }
                ),
            ),
            (
                "get",
                vec![("index", "int".into(), false)],
                format!("Option<{element}>"),
            ),
            (
                "set",
                vec![
                    ("index", "int".into(), true),
                    ("value", element.clone(), true),
                ],
                "()".into(),
            ),
            (
                "try_set",
                vec![
                    ("index", "int".into(), true),
                    ("value", element.clone(), true),
                ],
                "Result<(), ListError>".into(),
            ),
            ("push", vec![("value", element.clone(), false)], "()".into()),
            (
                "try_push",
                vec![("value", element, false)],
                "Result<(), ListError>".into(),
            ),
        ]
        .into_iter()
        .map(|(name, parameters, return_type)| EditorSignature {
            name: name.into(),
            parameters: parameters
                .into_iter()
                .map(|(name, ty, named)| EditorParameter {
                    name: name.into(),
                    ty,
                    named,
                })
                .collect(),
            return_type,
            documentation: "Bounded List operation; checked mutation rejects with ListError."
                .into(),
        })
        .collect();
    }
    let name = render_type_name(ty);
    let mut signatures = Vec::new();
    if matches!(ty, Type::Decimal | Type::Quantity) {
        let rounding = "Round once at the requested scale with an explicit Rounding mode.";
        signatures.push(editor_signature(
            "div_round",
            vec![
                ("divisor", "decimal".into(), true),
                ("scale", "int".into(), true),
                ("mode", "Rounding".into(), true),
            ],
            name.clone(),
            rounding,
        ));
        signatures.push(editor_signature(
            "mul_div_round",
            vec![
                ("multiplier", "decimal".into(), true),
                ("divisor", "decimal".into(), true),
                ("scale", "int".into(), true),
                ("mode", "Rounding".into(), true),
            ],
            name.clone(),
            "Fused multiply/divide with one final rounding step; intermediate arithmetic is exact.",
        ));
        if matches!(ty, Type::Quantity) {
            signatures.push(editor_signature(
                "ratio_round",
                vec![
                    ("divisor", "quantity".into(), true),
                    ("scale", "int".into(), true),
                    ("mode", "Rounding".into(), true),
                ],
                "decimal".into(),
                rounding,
            ));
        }
    }
    if let Type::StateMap(key, value) = ty {
        let (key, value) = (render_type_name(key), render_type_name(value));
        signatures.push(editor_signature(
            "get",
            vec![("key", key.clone(), false)],
            format!("Option<{value}>"),
            "Read one durable key; absence is Option::none.",
        ));
        signatures.push(editor_signature("page", vec![("after", format!("Option<StateCursor<{key}>>"), true), ("limit", "int".into(), true)], format!("StatePage<{key}, {value}, N>"), "Live keyset page; limit is a compile-time int constant expression N in 1..=64. At most 64 candidate positions are examined, including tombstones, and at most N values are returned. Read items and next; a final empty page is possible."));
        signatures.push(editor_signature("take", vec![("limit", "int".into(), false)], format!("List<({key}, {value}), N>"), "Return the first page's items; limit is a compile-time int constant expression N in 1..=64."));
    }
    signatures.extend(Builtin::registry().filter_map(|(builtin, spec)| {
        let first = spec.signature.parameters.first()?;
        (matches!(
            spec.surface,
            BuiltinSurface::MethodOnly | BuiltinSurface::FunctionOrMethod
        ) && (*first == name
            || (*first == "StateMap<K,V>" && matches!(ty, Type::StateMap(..)))
            || (*first == "decimal|quantity" && matches!(ty, Type::Decimal | Type::Quantity))))
        .then(|| {
            let mut signature = builtin_signature(builtin, true);
            if let Type::StateMap(key, value) = ty {
                for parameter in &mut signature.parameters {
                    parameter.ty = parameter
                        .ty
                        .replace('K', &render_type_name(key))
                        .replace('V', &render_type_name(value));
                }
                signature.return_type = signature
                    .return_type
                    .replace('K', &render_type_name(key))
                    .replace('V', &render_type_name(value));
            }
            signature
        })
    }));
    signatures
}
fn plain_completion(name: &str, kind: u64, detail: &str) -> EditorCompletion {
    EditorCompletion {
        label: name.into(),
        kind,
        detail: detail.into(),
        insert_text: name.into(),
        snippet: false,
        documentation: String::new(),
    }
}
fn signature_completion(signature: EditorSignature) -> EditorCompletion {
    EditorCompletion {
        label: signature.name.clone(),
        kind: 3,
        detail: signature.label(),
        insert_text: signature.snippet(),
        snippet: true,
        documentation: signature.documentation,
    }
}
fn definition_completion(definition: &EditorDefinition) -> EditorCompletion {
    definition
        .signature
        .clone()
        .map(signature_completion)
        .unwrap_or_else(|| plain_completion(&definition.name, definition.kind, &definition.detail))
}
fn path_prefix(tokens: &[Token], offset: u32) -> String {
    let mut pieces = Vec::new();
    for token in tokens
        .iter()
        .rev()
        .filter(|token| token.range.start < offset && token.kind != TokenKind::EOF)
    {
        match &token.kind {
            TokenKind::Ident(name) => pieces.push(name.clone()),
            TokenKind::ColonColon => pieces.push("::".into()),
            _ => break,
        }
    }
    pieces.reverse();
    pieces.concat()
}
fn receiver_before(tokens: &[Token], offset: u32) -> Option<TextRange> {
    let before = tokens
        .iter()
        .take_while(|token| token.range.start < offset)
        .collect::<Vec<_>>();
    let dot = before
        .iter()
        .rposition(|token| token.kind == TokenKind::Dot)?;
    if before[dot + 1..]
        .iter()
        .any(|token| !matches!(token.kind, TokenKind::Ident(_) | TokenKind::LParen))
    {
        return None;
    }
    let receiver = before.get(dot.checked_sub(1)?)?;
    matches!(receiver.kind, TokenKind::Ident(_)).then_some(receiver.range)
}
fn call_context(tokens: &[Token], offset: u32) -> Option<(String, u32, usize)> {
    let mut stack: Vec<(TokenKind, usize)> = Vec::new();
    for (index, token) in tokens
        .iter()
        .enumerate()
        .take_while(|(_, token)| token.range.start < offset)
    {
        match token.kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                stack.push((token.kind.clone(), index))
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                stack.pop();
            }
            _ => {}
        }
    }
    let (_, index) = stack
        .iter()
        .rev()
        .find(|(kind, _)| *kind == TokenKind::LParen)?;
    let opening = tokens[*index].range.start;
    let name = path_prefix(&tokens[..*index], opening);
    let (active, _, _) = argument_context(tokens, opening, offset);
    (!name.is_empty()).then_some((name, opening, active))
}
fn argument_context(
    tokens: &[Token],
    opening: u32,
    offset: u32,
) -> (usize, Option<String>, BTreeSet<String>) {
    let mut depth = 0_usize;
    let mut active = 0;
    let mut label = None;
    let mut labels = BTreeSet::new();
    let mut previous: Option<&TokenKind> = None;
    for token in tokens
        .iter()
        .filter(|token| token.range.start > opening && token.range.start < offset)
    {
        match token.kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                depth = depth.saturating_sub(1)
            }
            TokenKind::Comma if depth == 0 => {
                active += 1;
                label = None;
            }
            TokenKind::Colon if depth == 0 => {
                if let Some(TokenKind::Ident(name)) = previous {
                    label = Some(name.clone());
                    labels.insert(name.clone());
                }
            }
            _ => {}
        }
        previous = Some(&token.kind);
    }
    (active, label, labels)
}
/// Repair only the active completion token and missing closing delimiters in a temporary buffer.
/// This buffer is never returned to a build API; callers consume completion candidates only.
fn completion_repair(file: &SourceFile, tokens: &[Token], offset: u32) -> Option<String> {
    let mut text = file.text().to_owned();
    let before = tokens
        .iter()
        .filter(|token| token.range.start < offset && token.kind != TokenKind::EOF)
        .collect::<Vec<_>>();
    let last = before.last()?;
    let mut changed = false;
    if last.kind == TokenKind::Dot {
        if tokens
            .iter()
            .any(|token| token.range.start == offset && matches!(token.kind, TokenKind::Ident(_)))
        {
            return None;
        }
        text.insert_str(offset as usize, "len()");
        changed = true;
    } else if let TokenKind::Ident(_) = &last.kind {
        if last.range.end != offset {
            return None;
        }
        let member = before
            .get(before.len().checked_sub(2)?)
            .is_some_and(|token| token.kind == TokenKind::Dot);
        let replacement = if member {
            "len()".to_owned()
        } else {
            format!(
                "0{}",
                " ".repeat(last.range.end.saturating_sub(last.range.start + 1) as usize)
            )
        };
        text.replace_range(
            last.range.start as usize..last.range.end as usize,
            &replacement,
        );
        changed = true;
    }
    let repaired_tokens = crate::lexer::lex(&text).ok()?;
    let mut closing = Vec::new();
    for token in repaired_tokens {
        match token.kind {
            TokenKind::LParen => closing.push(')'),
            TokenKind::LBrace => closing.push('}'),
            TokenKind::LBracket => closing.push(']'),
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                closing.pop();
            }
            _ => {}
        }
    }
    changed |= !closing.is_empty();
    text.extend(closing.into_iter().rev());
    changed.then_some(text)
}

#[cfg(test)]
mod locked_recovery_tests;
#[cfg(test)]
mod nominal_references_tests;
#[cfg(test)]
mod tests {
    use super::*;
    fn cursor(source: &str, fragment: &str) -> u32 {
        source.find(fragment).expect("cursor fragment") as u32
    }
    fn labels(snapshot: &EditorSnapshot, offset: u32) -> BTreeSet<String> {
        snapshot
            .completions(SourceId(0), offset)
            .into_iter()
            .map(|candidate| candidate.label)
            .collect()
    }
    #[test]
    fn rename_tracks_binding_identity_and_named_argument_labels() {
        let source = "module Names { fn target(int _ first, int second) -> int { first + second } fn caller() -> int { target(1, second: 2) } }";
        let snapshot = EditorSnapshot::single("names.ko", source, false);
        assert!(snapshot.is_complete());
        let ranges = snapshot
            .rename(SourceId(0), cursor(source, "second)"), "amount")
            .expect("rename named parameter");
        assert!(ranges.exports.is_empty());
        assert_eq!(ranges.sources.len(), 3);
        for range in ranges.sources {
            assert_eq!(
                &source[range.range.start as usize..range.range.end as usize],
                "second"
            );
        }
        let call = cursor(source, "second: 2") + 9;
        let (signature, active) = snapshot
            .signature_help(SourceId(0), call)
            .expect("signature");
        assert_eq!(active, 1);
        assert_eq!(signature.label(), "target(int _ first, int second) -> int");
        assert_eq!(
            signature.snippet(),
            "target(${1:first}, second: ${2:second})"
        );
        assert!(
            snapshot
                .rename(SourceId(0), cursor(source, "second)"), "first")
                .is_err()
        );
    }
    #[test]
    fn rename_allows_disjoint_scopes_and_rejects_reference_capture() {
        let source = "module Scopes { fn first(int _ value) -> int { value } fn second(int _ amount) -> int { amount } }";
        let snapshot = EditorSnapshot::single("scopes.ko", source, false);
        let rename = snapshot
            .rename(SourceId(0), cursor(source, "value)"), "amount")
            .expect("independent function scopes may use the same parameter name");
        assert_eq!(rename.sources.len(), 2);
        let captured = "module Capture { fn run(int _ value) -> int { if true { let amount = 1; value + amount } else { value } } }";
        let snapshot = EditorSnapshot::single("capture.ko", captured, false);
        assert!(snapshot.is_complete());
        assert!(
            snapshot
                .rename(SourceId(0), cursor(captured, "value)"), "amount")
                .is_err(),
            "a type-correct rename must not capture an outer parameter inside a nested scope"
        );
    }
    #[test]
    fn scopes_do_not_leak_sibling_parameters_or_finished_block_locals() {
        let source = "module Scope { fn first(int _ input) -> int { if true { let hidden = input; } input } fn second(int _ other) -> int { other } }";
        let snapshot = EditorSnapshot::single("scope.ko", source, false);
        assert!(snapshot.is_complete());
        let visible = labels(&snapshot, cursor(source, "} input }") + 3);
        assert!(visible.contains("input"));
        assert!(!visible.contains("hidden"));
        assert!(!visible.contains("other"));
        let visible = labels(&snapshot, cursor(source, "{ other }") + 3);
        assert!(visible.contains("other"));
        assert!(!visible.contains("input"));
    }
    #[test]
    fn receivers_offer_only_their_actual_members_including_incomplete_buffers() {
        let source =
            "module Lists { fn read(List<int, 4> values) -> Option<int> { values.get(0) } }";
        let snapshot = EditorSnapshot::single("lists.ko", source, false);
        let members = labels(&snapshot, cursor(source, ".get") + 1);
        assert_eq!(
            members,
            crate::semantic::V1_LIST_MEMBER_NAMES
                .iter()
                .map(|name| (*name).to_owned())
                .collect()
        );
        assert!(!members.contains("ledger::asset::mint"));
        let incomplete = "module Lists { fn read(List<int, 4> values) { values. } }";
        let snapshot = EditorSnapshot::single("lists.ko", incomplete, false);
        assert!(!snapshot.is_complete());
        assert!(labels(&snapshot, cursor(incomplete, ". }") + 1).contains("try_set"));
        assert!(
            snapshot
                .rename(SourceId(0), cursor(incomplete, "values)"), "items")
                .is_err()
        );
        assert!(
            crate::parser::parse(incomplete).is_err(),
            "editor recovery must never broaden parser acceptance"
        );
    }
    #[test]
    fn canonical_builtin_hover_and_nested_argument_context_share_signatures() {
        let source = "module Calls { fn target(Json payload, int count) -> int { count } fn run() -> int { target(payload: json { nested: [1, 2], other: 3 }, count: 4) } }";
        let snapshot = EditorSnapshot::single("calls.ko", source, false);
        let offset = cursor(source, "count: 4") + 8;
        let (_, active) = snapshot
            .signature_help(SourceId(0), offset)
            .expect("nested signature");
        assert_eq!(active, 1);
        let source = "module Builtins { fn check() { let x = Json::parse(\"{}\"); } }";
        let snapshot = EditorSnapshot::single("builtins.ko", source, false);
        let (hover, _) = snapshot
            .hover(SourceId(0), cursor(source, "Json::parse") + 7)
            .expect("builtin hover");
        assert!(hover.contains("Json::parse"));
        assert!(hover.contains("string _"));
    }
    #[test]
    fn locked_import_references_keep_source_and_package_identity() {
        use crate::linker::{SourceModuleUnit, SourcePackageUnit};
        let request = SourceLinkRequest {
            root: SourceModuleUnit {
                source_name: "app.ko".into(),
                source: "seiyaku App { view fn run() -> int { arithmetic::value() } }".into(),
            },
            imports: vec![ImportBinding {
                alias: "arithmetic".into(),
                package: "std/math@1.0.0".into(),
            }],
            packages: vec![SourcePackageUnit {
                identity: "std/math@1.0.0".into(),
                modules: vec![SourceModuleUnit {
                    source_name: "math.ko".into(),
                    source: "module Math { fn value() -> int { 7 } fn hidden() -> int { 9 } }"
                        .into(),
                }],
                exports: BTreeSet::from(["value".into()]),
                imports: vec![],
            }],
        };
        let snapshot = EditorSnapshot::project(&request, false);
        assert!(
            snapshot.is_complete(),
            "{:#?}",
            ModuleBuildGraph::default().link(request.clone(), LinkerOptions::default())
        );
        let root = snapshot
            .sources()
            .find(|source| source.name() == "app.ko")
            .expect("root");
        let call = cursor(root.text(), "value()");
        let definition = snapshot
            .definition(root.id(), call)
            .expect("import definition");
        assert_eq!(
            snapshot
                .source(definition.source.source)
                .expect("definition source")
                .package_identity(),
            Some("std/math@1.0.0")
        );
        assert_eq!(snapshot.references(root.id(), call, true).len(), 2);
        let candidates = snapshot.completions(root.id(), cursor(root.text(), "arithmetic::") + 12);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.label.as_str())
                .collect::<Vec<_>>(),
            vec!["value"]
        );
        let rename = snapshot
            .rename(root.id(), call, "next")
            .expect("checked source and export plan");
        assert_eq!(rename.sources.len(), 2);
        assert_eq!(
            rename.exports,
            vec![EditorExportRename {
                package: "std/math@1.0.0".into(),
                old_name: "value".into(),
                new_name: "next".into(),
            }]
        );
        assert!(snapshot.rename(root.id(), call, "hidden").is_err());
    }
    #[test]
    fn imported_error_namespace_completion_uses_the_exported_nominal_type() {
        use crate::linker::{SourceModuleUnit, SourcePackageUnit};
        let request = SourceLinkRequest {
            root: SourceModuleUnit {
                source_name: "app.ko".into(),
                source: "seiyaku App { view fn run() -> errors::Failure { errors::Failure::Missing } }".into(),
            },
            imports: vec![ImportBinding { alias: "errors".into(), package: "local/errors@1".into() }],
            packages: vec![SourcePackageUnit {
                identity: "local/errors@1".into(),
                modules: vec![SourceModuleUnit {
                    source_name: "errors.ko".into(),
                    source: "module Errors { error enum Failure { Missing = 1, Invalid = 2 } fn value() -> int { 7 } }".into(),
                }],
                exports: BTreeSet::from(["Failure".into(), "value".into()]), imports: vec![],
            }],
        };
        let snapshot = EditorSnapshot::project(&request, false);
        assert!(snapshot.is_complete());
        let root = snapshot
            .sources()
            .find(|file| file.name() == "app.ko")
            .unwrap();
        let offset =
            cursor(root.text(), "errors::Failure::Missing") + "errors::Failure::".len() as u32;
        let candidates = snapshot.completions(root.id(), offset);
        assert_eq!(
            candidates
                .iter()
                .map(|item| item.label.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["Missing", "Invalid"])
        );
        assert!(
            candidates
                .iter()
                .all(|item| item.detail.starts_with("errors::Failure::"))
        );
    }
    #[test]
    fn namespace_completion_preserves_nested_prefixes_and_nominal_errors() {
        let source = "module Partial { fn run() { ledger::asset:: } }";
        let snapshot = EditorSnapshot::single("partial.ko", source, false);
        let candidates = snapshot.completions(SourceId(0), cursor(source, "ledger::asset::") + 15);
        assert!(candidates.iter().any(|candidate| candidate.label == "mint"));
        assert!(
            candidates
                .iter()
                .all(|candidate| !candidate.insert_text.starts_with("asset::"))
        );
        let source = "module Partial { fn run() { ListError:: } }";
        let snapshot = EditorSnapshot::single("partial.ko", source, false);
        let candidates = snapshot.completions(SourceId(0), cursor(source, "ListError::") + 11);
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.label == "IndexOutOfBounds")
        );
    }
    #[test]
    fn numeric_and_cursor_signatures_offer_exact_labels_and_types() {
        let numeric = member_signatures(&Type::Quantity);
        let fused = numeric
            .iter()
            .find(|signature| signature.name == "mul_div_round")
            .unwrap();
        assert_eq!(fused.return_type, "quantity");
        assert_eq!(
            fused.snippet(),
            "mul_div_round(multiplier: ${1:multiplier}, divisor: ${2:divisor}, scale: ${3:scale}, mode: ${4:mode})"
        );
        let map = member_signatures(&Type::StateMap(Box::new(Type::Int), Box::new(Type::Bool)));
        let page = map
            .iter()
            .find(|signature| signature.name == "page")
            .unwrap();
        assert_eq!(page.parameters[0].ty, "Option<StateCursor<int>>");
        assert_eq!(page.return_type, "StatePage<int, bool, N>");
        assert!(map.iter().all(|signature| signature.name != "range"));
        assert_eq!(
            map.iter()
                .find(|signature| signature.name == "take")
                .unwrap()
                .snippet(),
            "take(${1:limit})"
        );
        assert!(
            intrinsic_signatures()
                .iter()
                .any(|signature| signature.name == "quantity::try_from_int"
                    && signature.return_type == "Result<quantity, NumericError>"
                    && !signature.parameters[0].named)
        );
    }
    #[test]
    fn completion_filters_test_and_zk_modes_by_source_context() {
        let source = "module Modes { fn ordinary() {} #[test] fn test_case() {} }";
        let snapshot = EditorSnapshot::single("modes.ko", source, false);
        let ordinary = snapshot.completions(SourceId(0), cursor(source, "ordinary() {") + 12);
        assert!(
            ordinary
                .iter()
                .all(|candidate| !candidate.label.starts_with("test::"))
        );
        let test = snapshot.completions(SourceId(0), cursor(source, "test_case() {") + 13);
        assert!(
            test.iter()
                .any(|candidate| candidate.label == "test::expect_reject_as")
        );
    }
}
