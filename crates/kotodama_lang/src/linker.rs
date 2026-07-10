//! Typed-HIR linker for Kotodama V1 modules.
//!
//! Source units are parsed and type checked independently. The linker resolves
//! only explicit `alias::symbol` imports backed by a locked export table,
//! rewrites final symbol identities in typed HIR, and then reruns whole-program
//! recursion and effect analysis before handing the result to the canonical
//! compiler session.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    error::Error,
    fmt,
    sync::Mutex,
};

use iroha_crypto::Hash;

use crate::{
    ast::{Block, Expr, Item, Program, SourceUnitKind, Statement},
    builtins::{Builtin, BuiltinSurface},
    diagnostic::DiagnosticBundle,
    semantic::{
        self, ExprKind, FunctionSignature, Type, TypedBlock, TypedExpr, TypedItem, TypedProgram,
        TypedStatement,
    },
    source::{FrontendBudget, SourceFile, SourceId},
};

const LINKED_SYMBOL_PREFIX: &str = "__kotodama_link_";
const MAX_PARSED_CACHE_ENTRIES: usize = 64;
const MAX_PARSED_CACHE_SOURCE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum number of source units in one typed module graph.
pub const MAX_MODULE_GRAPH_SOURCES: usize = 512;
/// Maximum aggregate UTF-8 bytes in one typed module graph.
pub const MAX_MODULE_GRAPH_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// One explicit import alias resolved by a lockfile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportBinding {
    /// Source-level alias used before `::`.
    pub alias: String,
    /// Stable package identity referenced by the lockfile.
    pub package: String,
}

/// One parsed reusable module and its diagnostic source name.
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleUnit {
    /// Logical source path used in linker errors.
    pub source_name: String,
    /// Parsed `module Name { ... }` source unit.
    pub program: Program,
}

/// One locked package presented to the typed linker.
#[derive(Clone, Debug, PartialEq)]
pub struct PackageUnit {
    /// Stable canonical package reference.
    pub identity: String,
    /// Every reusable source unit in the package.
    pub modules: Vec<ModuleUnit>,
    /// Explicit function exports from package metadata.
    pub exports: BTreeSet<String>,
    /// Dependency aliases locked for this package.
    pub imports: Vec<ImportBinding>,
}

/// Complete request for linking one deployable contract.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkRequest {
    /// The only deployable `seiyaku Name { ... }` source unit.
    pub root: ModuleUnit,
    /// Direct dependency aliases visible to the root contract.
    pub imports: Vec<ImportBinding>,
    /// Locked transitive package graph.
    pub packages: Vec<PackageUnit>,
}

/// One reusable source unit before parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceModuleUnit {
    /// Logical path retained in diagnostics and source-map sidecars.
    pub source_name: String,
    /// Complete Kotodama source text.
    pub source: String,
}

/// One locked package before its source modules are parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePackageUnit {
    /// Stable canonical package reference.
    pub identity: String,
    /// Every reusable source unit in the package.
    pub modules: Vec<SourceModuleUnit>,
    /// Explicit function exports from package metadata.
    pub exports: BTreeSet<String>,
    /// Dependency aliases locked for this package.
    pub imports: Vec<ImportBinding>,
}

/// Complete source-level request for one typed-HIR module build graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLinkRequest {
    /// The single deployable contract source.
    pub root: SourceModuleUnit,
    /// Direct dependency aliases visible to the root contract.
    pub imports: Vec<ImportBinding>,
    /// Locked transitive package graph.
    pub packages: Vec<SourcePackageUnit>,
}

/// Linked typed-HIR plus the canonical identity of every graph input.
#[derive(Debug)]
pub struct LinkedSourceGraph {
    /// Fully resolved typed-HIR program accepted by the canonical compiler session.
    pub program: TypedProgram,
    /// Domain-separated digest of source contents, logical paths, imports, and exports.
    pub fingerprint: Hash,
}

/// Failure while parsing or linking a source module graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceGraphError {
    /// A source graph exceeded a fixed compiler-service resource budget.
    Budget {
        /// Number of source units supplied.
        sources: usize,
        /// Aggregate UTF-8 source bytes supplied.
        source_bytes: usize,
        /// Maximum accepted source-unit count.
        max_sources: usize,
        /// Maximum accepted aggregate source bytes.
        max_source_bytes: usize,
    },
    /// A source unit failed the canonical parser.
    Parse {
        /// Logical source path associated with the diagnostics.
        source: String,
        /// Structured parser diagnostics.
        diagnostics: DiagnosticBundle,
    },
    /// Typed-HIR linking failed.
    Link(LinkError),
}

impl fmt::Display for SourceGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget {
                sources,
                source_bytes,
                max_sources,
                max_source_bytes,
            } => write!(
                formatter,
                "Kotodama module graph contains {sources} sources/{source_bytes} bytes; V1 permits at most {max_sources} sources/{max_source_bytes} bytes"
            ),
            Self::Parse {
                source,
                diagnostics,
            } => write!(
                formatter,
                "Kotodama parse error in `{source}`: {}",
                diagnostics.render_human()
            ),
            Self::Link(error) => error.fmt(formatter),
        }
    }
}

impl Error for SourceGraphError {}

impl From<LinkError> for SourceGraphError {
    fn from(error: LinkError) -> Self {
        Self::Link(error)
    }
}

struct CachedParsedSource {
    digest: String,
    // Retaining the exact source prevents a digest collision from substituting
    // one parsed module for another.
    source: String,
    program: Program,
}

#[derive(Default)]
struct ParsedSourceCache {
    entries: VecDeque<CachedParsedSource>,
    source_bytes: usize,
}

impl ParsedSourceCache {
    fn get(&mut self, digest: &str, source: &str) -> Option<Program> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.digest == digest && entry.source == source)?;
        let entry = self
            .entries
            .remove(index)
            .expect("cache index came from the same deque");
        let program = entry.program.clone();
        self.entries.push_back(entry);
        Some(program)
    }

    fn insert(&mut self, digest: String, source: String, program: Program) {
        if source.len() > MAX_PARSED_CACHE_SOURCE_BYTES {
            return;
        }
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.digest == digest && entry.source == source)
        {
            let replaced = self
                .entries
                .remove(index)
                .expect("cache index came from the same deque");
            self.source_bytes = self.source_bytes.saturating_sub(replaced.source.len());
        }
        while self.entries.len() >= MAX_PARSED_CACHE_ENTRIES
            || self.source_bytes.saturating_add(source.len()) > MAX_PARSED_CACHE_SOURCE_BYTES
        {
            let Some(evicted) = self.entries.pop_front() else {
                break;
            };
            self.source_bytes = self.source_bytes.saturating_sub(evicted.source.len());
        }
        self.source_bytes = self.source_bytes.saturating_add(source.len());
        self.entries.push_back(CachedParsedSource {
            digest,
            source,
            program,
        });
    }
}

/// Reusable, content-addressed parser and typed-HIR linker.
///
/// A call parses independent changed modules in parallel. Equal source contents
/// are parsed once, while a bounded LRU retains exact source text to defend
/// against digest collisions without permitting unbounded service memory. The
/// final linker still receives a deterministic request and performs whole-graph
/// type/effect analysis exactly once.
#[derive(Default)]
pub struct ModuleBuildGraph {
    parsed: Mutex<ParsedSourceCache>,
    #[cfg(test)]
    parse_attempts: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    link_attempts: std::sync::atomic::AtomicUsize,
}

impl ModuleBuildGraph {
    /// Return the canonical identity of a complete locked source graph.
    ///
    /// This preflight performs the same aggregate resource-budget check as
    /// [`Self::link`] but does not parse, resolve, or type-check any source. A
    /// build driver can therefore authenticate a previously published result
    /// before doing compiler work on an unchanged graph.
    pub fn fingerprint(request: &SourceLinkRequest) -> Result<Hash, SourceGraphError> {
        validate_source_graph_budget(request)?;
        Ok(source_graph_fingerprint(request))
    }

    /// Parse, resolve, and type-check one complete locked source graph.
    pub fn link(
        &self,
        request: SourceLinkRequest,
        options: LinkerOptions,
    ) -> Result<LinkedSourceGraph, SourceGraphError> {
        let fingerprint = Self::fingerprint(&request)?;
        #[cfg(test)]
        self.link_attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut sources = Vec::new();
        sources.push(request.root.clone());
        for package in &request.packages {
            sources.extend(package.modules.iter().cloned());
        }
        let mut programs = self.parse_sources(&sources)?.into_iter();
        let root = ModuleUnit {
            source_name: request.root.source_name,
            program: programs
                .next()
                .expect("the root source is always included in the parse graph"),
        };
        let mut packages = Vec::with_capacity(request.packages.len());
        for package in request.packages {
            let modules = package
                .modules
                .into_iter()
                .map(|module| ModuleUnit {
                    source_name: module.source_name,
                    program: programs
                        .next()
                        .expect("every source module has one parsed program"),
                })
                .collect();
            packages.push(PackageUnit {
                identity: package.identity,
                modules,
                exports: package.exports,
                imports: package.imports,
            });
        }
        debug_assert!(programs.next().is_none());
        let program = TypedLinker::new(options).link(LinkRequest {
            root,
            imports: request.imports,
            packages,
        })?;
        Ok(LinkedSourceGraph {
            program,
            fingerprint,
        })
    }

    #[cfg(test)]
    pub(crate) fn parse_attempt_count(&self) -> usize {
        self.parse_attempts
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn link_attempt_count(&self) -> usize {
        self.link_attempts
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn parse_sources(
        &self,
        sources: &[SourceModuleUnit],
    ) -> Result<Vec<Program>, SourceGraphError> {
        self.parse_sources_with_digest(sources, |source| {
            Hash::new_from_chunks(&[b"kotodama-module-source-v1\0", source.as_bytes()]).to_string()
        })
    }

    fn parse_sources_with_digest(
        &self,
        sources: &[SourceModuleUnit],
        digest: impl Fn(&str) -> String,
    ) -> Result<Vec<Program>, SourceGraphError> {
        struct UniqueSource {
            digest: String,
            source: String,
            source_name: String,
            members: Vec<usize>,
            program: Option<Program>,
        }

        let mut unique = Vec::<UniqueSource>::new();
        let mut digest_indexes = HashMap::<String, Vec<usize>>::new();
        for (source_index, unit) in sources.iter().enumerate() {
            let source_digest = digest(&unit.source);
            let existing = digest_indexes.get(&source_digest).and_then(|indexes| {
                indexes
                    .iter()
                    .copied()
                    .find(|index| unique[*index].source == unit.source)
            });
            if let Some(index) = existing {
                unique[index].members.push(source_index);
            } else {
                let index = unique.len();
                unique.push(UniqueSource {
                    digest: source_digest.clone(),
                    source: unit.source.clone(),
                    source_name: unit.source_name.clone(),
                    members: vec![source_index],
                    program: None,
                });
                digest_indexes.entry(source_digest).or_default().push(index);
            }
        }

        {
            let mut cache = self
                .parsed
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            for item in &mut unique {
                item.program = cache.get(&item.digest, &item.source);
            }
        }

        let pending = unique
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.program.is_none().then_some(index))
            .collect::<Vec<_>>();
        let jobs = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .max(1);
        for chunk in pending.chunks(jobs) {
            let parsed = std::thread::scope(|scope| {
                let handles = chunk
                    .iter()
                    .map(|index| {
                        let item = &unique[*index];
                        scope.spawn(move || {
                            #[cfg(test)]
                            self.parse_attempts
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let file = SourceFile::new(
                                SourceId(0),
                                item.source_name.as_str(),
                                item.source.as_str(),
                            );
                            let result = crate::parser::parse_source(&file, FrontendBudget::v1())
                                .map_err(|diagnostics| SourceGraphError::Parse {
                                    source: item.source_name.clone(),
                                    diagnostics,
                                });
                            (*index, result)
                        })
                    })
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .map(|handle| {
                        handle
                            .join()
                            .expect("Kotodama module parser workers must not panic")
                    })
                    .collect::<Vec<_>>()
            });
            // Join order follows deterministic source order, so multiple parser
            // failures always report the same first source.
            for (index, result) in parsed {
                unique[index].program = Some(result?);
            }
        }

        {
            let mut cache = self
                .parsed
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            for item in &unique {
                cache.insert(
                    item.digest.clone(),
                    item.source.clone(),
                    item.program
                        .as_ref()
                        .expect("every unique source was parsed")
                        .clone(),
                );
            }
        }

        let mut programs = vec![None; sources.len()];
        for item in unique {
            let program = item.program.expect("every unique source was parsed");
            for member in item.members {
                programs[member] = Some(program.clone());
            }
        }
        Ok(programs
            .into_iter()
            .map(|program| program.expect("every source belongs to a unique group"))
            .collect())
    }
}

fn validate_source_graph_budget(request: &SourceLinkRequest) -> Result<(), SourceGraphError> {
    let mut sources = 1_usize;
    let mut source_bytes = request.root.source.len();
    for package in &request.packages {
        sources = sources.saturating_add(package.modules.len());
        for module in &package.modules {
            source_bytes = source_bytes.saturating_add(module.source.len());
        }
    }
    if sources > MAX_MODULE_GRAPH_SOURCES || source_bytes > MAX_MODULE_GRAPH_SOURCE_BYTES {
        return Err(SourceGraphError::Budget {
            sources,
            source_bytes,
            max_sources: MAX_MODULE_GRAPH_SOURCES,
            max_source_bytes: MAX_MODULE_GRAPH_SOURCE_BYTES,
        });
    }
    Ok(())
}

fn source_graph_fingerprint(request: &SourceLinkRequest) -> Hash {
    fn field(transcript: &mut Vec<u8>, value: impl AsRef<[u8]>) {
        let value = value.as_ref();
        transcript.extend_from_slice(&(value.len() as u64).to_le_bytes());
        transcript.extend_from_slice(value);
    }

    fn imports(transcript: &mut Vec<u8>, values: &[ImportBinding]) {
        let mut values = values.to_vec();
        values.sort_by(|left, right| {
            left.alias
                .cmp(&right.alias)
                .then_with(|| left.package.cmp(&right.package))
        });
        field(transcript, (values.len() as u64).to_le_bytes());
        for value in values {
            field(transcript, value.alias);
            field(transcript, value.package);
        }
    }

    let mut transcript = b"kotodama-source-graph-v1\0".to_vec();
    field(&mut transcript, &request.root.source_name);
    field(&mut transcript, &request.root.source);
    imports(&mut transcript, &request.imports);
    let mut packages = request.packages.iter().collect::<Vec<_>>();
    packages.sort_by(|left, right| left.identity.cmp(&right.identity));
    field(&mut transcript, (packages.len() as u64).to_le_bytes());
    for package in packages {
        field(&mut transcript, &package.identity);
        imports(&mut transcript, &package.imports);
        field(
            &mut transcript,
            (package.exports.len() as u64).to_le_bytes(),
        );
        for export in &package.exports {
            field(&mut transcript, export);
        }
        let mut modules = package.modules.iter().collect::<Vec<_>>();
        modules.sort_by(|left, right| left.source_name.cmp(&right.source_name));
        field(&mut transcript, (modules.len() as u64).to_le_bytes());
        for module in modules {
            field(&mut transcript, &module.source_name);
            field(&mut transcript, &module.source);
        }
    }
    Hash::new(transcript)
}

/// Compiler capabilities applied consistently to every linked source unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkerOptions {
    /// Permit ZK-only types and builtins.
    pub zk_enabled: bool,
    /// Permit compiler-owned test builtins.
    pub test_builtins_enabled: bool,
    /// Accept local test declarations. This must match `test_builtins_enabled`;
    /// production linking rejects test syntax and never strips it implicitly.
    pub include_tests: bool,
}

/// A deterministic typed-link failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkError {
    /// The deployable root was not a contract source unit.
    RootMustBeContract {
        /// Root diagnostic source name.
        source: String,
    },
    /// A dependency source was not a module source unit.
    DependencyMustBeModule {
        /// Dependency diagnostic source name.
        source: String,
    },
    /// Two package records used the same stable identity.
    DuplicatePackage {
        /// Repeated canonical package identity.
        package: String,
    },
    /// Two module files in one package declared the same module name.
    DuplicateModule {
        /// Canonical package identity.
        package: String,
        /// Repeated module name.
        module: String,
    },
    /// An import alias was repeated in the same scope.
    DuplicateImport {
        /// Root or package import scope.
        scope: String,
        /// Repeated source alias.
        alias: String,
    },
    /// An import alias occupied a compiler-owned capability namespace.
    ReservedImport {
        /// Root or package import scope.
        scope: String,
        /// Compiler-owned source namespace.
        alias: String,
    },
    /// Two declarations occupied the same source-unit namespace.
    DuplicateSymbol {
        /// Diagnostic source name.
        source: String,
        /// Repeated declaration name.
        symbol: String,
    },
    /// An import referenced a package absent from the locked graph.
    UnknownPackage {
        /// Root or package import scope.
        scope: String,
        /// Missing canonical package identity.
        package: String,
    },
    /// A call used an alias not explicitly imported by its source package.
    UnknownAlias {
        /// Diagnostic source name.
        source: String,
        /// Unresolved source alias.
        alias: String,
    },
    /// A call targeted a function absent from the package export table.
    UnexportedSymbol {
        /// Diagnostic source name.
        source: String,
        /// Resolved package alias.
        alias: String,
        /// Function absent from the export table.
        symbol: String,
    },
    /// A declared export was not defined by any module.
    MissingExport {
        /// Canonical package identity.
        package: String,
        /// Missing function name.
        symbol: String,
    },
    /// Multiple modules defined the same declared export.
    AmbiguousExport {
        /// Canonical package identity.
        package: String,
        /// Ambiguous function name.
        symbol: String,
    },
    /// A wildcard import/call was requested.
    WildcardImport {
        /// Diagnostic source or import scope.
        source: String,
    },
    /// A source or lockfile name is not a strict V1 identifier.
    InvalidIdentifier {
        /// Kind of source name being checked.
        context: String,
        /// Invalid spelling.
        name: String,
    },
    /// A source declaration collides with compiler-owned or builtin names.
    ReservedSymbol {
        /// Diagnostic source name.
        source: String,
        /// Reserved declaration spelling.
        symbol: String,
    },
    /// A dependency module contained deployable-only state or triggers.
    InvalidModuleItem {
        /// Diagnostic source name.
        source: String,
        /// Rejected declaration category.
        item: String,
    },
    /// A module error code collided with another linked error code.
    DuplicateErrorCode {
        /// Repeated stable error code.
        code: u32,
    },
    /// A module localization key collided with another linked key.
    DuplicateMessage {
        /// Repeated localization key.
        key: String,
    },
    /// Parsing succeeded but type/effect analysis rejected one unit or the link.
    Semantic {
        /// Diagnostic source or linked-program label.
        source: String,
        /// Semantic failure message.
        message: String,
    },
}

impl fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootMustBeContract { source } => {
                write!(
                    formatter,
                    "deployable root `{source}` must declare exactly one contract"
                )
            }
            Self::DependencyMustBeModule { source } => {
                write!(
                    formatter,
                    "dependency `{source}` must declare exactly one module"
                )
            }
            Self::DuplicatePackage { package } => {
                write!(formatter, "duplicate locked package `{package}`")
            }
            Self::DuplicateModule { package, module } => {
                write!(
                    formatter,
                    "package `{package}` declares module `{module}` more than once"
                )
            }
            Self::DuplicateImport { scope, alias } => {
                write!(
                    formatter,
                    "scope `{scope}` imports alias `{alias}` more than once"
                )
            }
            Self::ReservedImport { scope, alias } => write!(
                formatter,
                "scope `{scope}` cannot import compiler-owned capability namespace `{alias}`"
            ),
            Self::DuplicateSymbol { source, symbol } => {
                write!(
                    formatter,
                    "source `{source}` declares symbol `{symbol}` more than once"
                )
            }
            Self::UnknownPackage { scope, package } => {
                write!(
                    formatter,
                    "scope `{scope}` imports unknown locked package `{package}`"
                )
            }
            Self::UnknownAlias { source, alias } => {
                write!(
                    formatter,
                    "source `{source}` uses unknown import alias `{alias}`"
                )
            }
            Self::UnexportedSymbol {
                source,
                alias,
                symbol,
            } => write!(
                formatter,
                "source `{source}` cannot call unexported symbol `{alias}::{symbol}`"
            ),
            Self::MissingExport { package, symbol } => {
                write!(
                    formatter,
                    "package `{package}` exports missing function `{symbol}`"
                )
            }
            Self::AmbiguousExport { package, symbol } => write!(
                formatter,
                "package `{package}` exports ambiguous function `{symbol}` from multiple modules"
            ),
            Self::WildcardImport { source } => {
                write!(
                    formatter,
                    "source `{source}` uses a wildcard import; Kotodama V1 requires explicit symbols"
                )
            }
            Self::InvalidIdentifier { context, name } => {
                write!(
                    formatter,
                    "invalid Kotodama V1 identifier `{name}` in {context}"
                )
            }
            Self::ReservedSymbol { source, symbol } => {
                write!(
                    formatter,
                    "source `{source}` declares reserved symbol `{symbol}`"
                )
            }
            Self::InvalidModuleItem { source, item } => {
                write!(
                    formatter,
                    "module `{source}` contains deployable-only {item}"
                )
            }
            Self::DuplicateErrorCode { code } => {
                write!(
                    formatter,
                    "linked modules assign duplicate contract error code {code}"
                )
            }
            Self::DuplicateMessage { key } => {
                write!(
                    formatter,
                    "linked modules define duplicate messages key `{key}`"
                )
            }
            Self::Semantic { source, message } => {
                write!(
                    formatter,
                    "Kotodama analysis failed for `{source}`: {message}"
                )
            }
        }
    }
}

impl Error for LinkError {}

/// Stateless deterministic typed-HIR linker.
#[derive(Clone, Copy, Debug, Default)]
pub struct TypedLinker {
    options: LinkerOptions,
}

impl TypedLinker {
    /// Create a linker with explicit compiler capabilities.
    pub const fn new(options: LinkerOptions) -> Self {
        Self { options }
    }

    /// Resolve and link one contract plus its locked module graph.
    pub fn link(&self, mut request: LinkRequest) -> Result<TypedProgram, LinkError> {
        if self.options.include_tests != self.options.test_builtins_enabled {
            return Err(LinkError::Semantic {
                source: "linker options".to_owned(),
                message: "E_TEST_ONLY_PRODUCTION: include_tests and test_builtins_enabled must select one explicit compiler test mode together"
                    .to_owned(),
            });
        }
        if request.root.program.unit.kind != SourceUnitKind::Contract {
            return Err(LinkError::RootMustBeContract {
                source: request.root.source_name,
            });
        }
        validate_program_symbols(&request.root)?;

        request
            .packages
            .sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut package_identities = HashSet::new();
        for package in &mut request.packages {
            if !package_identities.insert(package.identity.clone()) {
                return Err(LinkError::DuplicatePackage {
                    package: package.identity.clone(),
                });
            }
            package.modules.sort_by(|left, right| {
                left.program
                    .unit
                    .name
                    .cmp(&right.program.unit.name)
                    .then_with(|| left.source_name.cmp(&right.source_name))
            });
            let mut module_names = HashSet::new();
            for module in &mut package.modules {
                if module.program.unit.kind != SourceUnitKind::Module {
                    return Err(LinkError::DependencyMustBeModule {
                        source: module.source_name.clone(),
                    });
                }
                if !module_names.insert(module.program.unit.name.clone()) {
                    return Err(LinkError::DuplicateModule {
                        package: package.identity.clone(),
                        module: module.program.unit.name.clone(),
                    });
                }
                validate_program_symbols(module)?;
                validate_module_items(module)?;
            }
        }

        let package_indexes = request
            .packages
            .iter()
            .enumerate()
            .map(|(index, package)| (package.identity.clone(), index))
            .collect::<HashMap<_, _>>();
        let root_imports = resolve_imports("root", &request.imports, &package_indexes)?;

        let mut resolved_packages = Vec::with_capacity(request.packages.len());
        for (package_index, package) in request.packages.iter().enumerate() {
            let imports = resolve_imports(&package.identity, &package.imports, &package_indexes)?;
            let mut modules = Vec::with_capacity(package.modules.len());
            for (module_index, module) in package.modules.iter().enumerate() {
                let semantic = semantic::SemanticContext::with_capabilities(
                    self.options.zk_enabled,
                    self.options.test_builtins_enabled,
                );
                let mut signatures = semantic
                    .resolve_function_signatures(&module.program)
                    .map_err(|error| LinkError::Semantic {
                        source: module.source_name.clone(),
                        message: error.message,
                    })?;
                let local_structs = module
                    .program
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        Item::Struct(definition) => Some(definition.name.clone()),
                        _ => None,
                    })
                    .collect::<HashSet<_>>();
                let type_prefix =
                    format!("{LINKED_SYMBOL_PREFIX}p{package_index}_m{module_index}_t");
                for signature in signatures.values_mut() {
                    qualify_signature(signature, &local_structs, &type_prefix);
                }
                let linked_names = signatures
                    .keys()
                    .enumerate()
                    .map(|(function_index, name)| {
                        (
                            name.clone(),
                            format!(
                                "{LINKED_SYMBOL_PREFIX}p{package_index}_m{module_index}_f{function_index}"
                            ),
                        )
                    })
                    .collect();
                modules.push(ResolvedModule {
                    source: module,
                    signatures,
                    linked_names,
                    local_structs,
                    type_prefix,
                });
            }

            let mut exports = BTreeMap::new();
            for export in &package.exports {
                validate_identifier("package export", export)?;
                let candidates = modules
                    .iter()
                    .filter_map(|module| {
                        module
                            .signatures
                            .get(export)
                            .map(|signature| ResolvedExport {
                                linked_name: module
                                    .linked_names
                                    .get(export)
                                    .expect("every signature receives a linked name")
                                    .clone(),
                                signature: signature.clone(),
                            })
                    })
                    .collect::<Vec<_>>();
                let resolved = match candidates.as_slice() {
                    [] => {
                        return Err(LinkError::MissingExport {
                            package: package.identity.clone(),
                            symbol: export.clone(),
                        });
                    }
                    [resolved] => resolved.clone(),
                    _ => {
                        return Err(LinkError::AmbiguousExport {
                            package: package.identity.clone(),
                            symbol: export.clone(),
                        });
                    }
                };
                exports.insert(export.clone(), resolved);
            }
            resolved_packages.push(ResolvedPackage {
                imports,
                modules,
                exports,
            });
        }

        validate_imported_calls(&request.root, &root_imports, &resolved_packages)?;
        let root_external = external_signatures(&root_imports, &resolved_packages);
        let semantic = semantic::SemanticContext::with_capabilities(
            self.options.zk_enabled,
            self.options.test_builtins_enabled,
        );
        let mut root = semantic
            .analyze_with_external_functions(&request.root.program, &root_external)
            .map_err(|error| LinkError::Semantic {
                source: request.root.source_name.clone(),
                message: error.message,
            })?;
        let root_external_names = external_linked_names(&root_imports, &resolved_packages);
        rename_program_calls(&mut root, &BTreeMap::new(), &root_external_names);

        let mut seen_error_codes = root
            .error_codes
            .iter()
            .map(|error| error.code)
            .collect::<HashSet<_>>();
        let mut seen_messages = root
            .message_entries
            .iter()
            .map(|entry| entry.msg_id.clone())
            .collect::<HashSet<_>>();

        for package_index in 0..resolved_packages.len() {
            let external = external_signatures(
                &resolved_packages[package_index].imports,
                &resolved_packages,
            );
            let external_names = external_linked_names(
                &resolved_packages[package_index].imports,
                &resolved_packages,
            );
            for module_index in 0..resolved_packages[package_index].modules.len() {
                let module = &resolved_packages[package_index].modules[module_index];
                validate_imported_calls(
                    module.source,
                    &resolved_packages[package_index].imports,
                    &resolved_packages,
                )?;
                let semantic = semantic::SemanticContext::with_capabilities(
                    self.options.zk_enabled,
                    self.options.test_builtins_enabled,
                );
                let mut typed = semantic
                    .analyze_with_external_functions(&module.source.program, &external)
                    .map_err(|error| LinkError::Semantic {
                        source: module.source.source_name.clone(),
                        message: error.message,
                    })?;
                qualify_typed_program(&mut typed, &module.local_structs, &module.type_prefix);
                rename_program_calls(&mut typed, &module.linked_names, &external_names);

                for mut error in typed.error_codes {
                    if !seen_error_codes.insert(error.code) {
                        return Err(LinkError::DuplicateErrorCode { code: error.code });
                    }
                    error.namespace = format!(
                        "{LINKED_SYMBOL_PREFIX}p{package_index}_m{module_index}_{}",
                        error.namespace
                    );
                    root.error_codes.push(error);
                }
                for message in typed.message_entries {
                    if !seen_messages.insert(message.msg_id.clone()) {
                        return Err(LinkError::DuplicateMessage {
                            key: message.msg_id,
                        });
                    }
                    root.message_entries.push(message);
                }
                root.items.extend(typed.items);
            }
        }

        semantic::validate_linked_program(&root, self.options.zk_enabled).map_err(|error| {
            LinkError::Semantic {
                source: "linked program".to_owned(),
                message: error.message,
            }
        })?;
        Ok(root)
    }
}

#[derive(Clone)]
struct ResolvedExport {
    linked_name: String,
    signature: FunctionSignature,
}

struct ResolvedModule<'request> {
    source: &'request ModuleUnit,
    signatures: BTreeMap<String, FunctionSignature>,
    linked_names: BTreeMap<String, String>,
    local_structs: HashSet<String>,
    type_prefix: String,
}

struct ResolvedPackage<'request> {
    imports: BTreeMap<String, usize>,
    modules: Vec<ResolvedModule<'request>>,
    exports: BTreeMap<String, ResolvedExport>,
}

fn resolve_imports(
    scope: &str,
    imports: &[ImportBinding],
    package_indexes: &HashMap<String, usize>,
) -> Result<BTreeMap<String, usize>, LinkError> {
    let mut resolved = BTreeMap::new();
    for import in imports {
        if import.alias == "*" {
            return Err(LinkError::WildcardImport {
                source: scope.to_owned(),
            });
        }
        validate_identifier("import alias", &import.alias)?;
        if is_reserved_import_alias(&import.alias) {
            return Err(LinkError::ReservedImport {
                scope: scope.to_owned(),
                alias: import.alias.clone(),
            });
        }
        let package = package_indexes
            .get(&import.package)
            .copied()
            .ok_or_else(|| LinkError::UnknownPackage {
                scope: scope.to_owned(),
                package: import.package.clone(),
            })?;
        if resolved.insert(import.alias.clone(), package).is_some() {
            return Err(LinkError::DuplicateImport {
                scope: scope.to_owned(),
                alias: import.alias.clone(),
            });
        }
    }
    Ok(resolved)
}

/// Return whether an import alias collides with a V1 builtin or compiler name.
///
/// Package frontends use the same predicate as the typed linker so a manifest
/// cannot accept an alias that will only fail later during contract linking.
pub fn is_reserved_import_alias(alias: &str) -> bool {
    semantic::is_reserved_source_declaration(alias, false)
        || Builtin::ALL.iter().any(|builtin| {
            matches!(
                builtin.spec().surface,
                BuiltinSurface::Function | BuiltinSurface::FunctionOrMethod
            ) && builtin
                .source_name()
                .split_once("::")
                .is_some_and(|(root, _)| root == alias)
        })
}

fn external_signatures(
    imports: &BTreeMap<String, usize>,
    packages: &[ResolvedPackage<'_>],
) -> BTreeMap<String, FunctionSignature> {
    let mut external = BTreeMap::new();
    for (alias, package_index) in imports {
        for (symbol, export) in &packages[*package_index].exports {
            external.insert(format!("{alias}::{symbol}"), export.signature.clone());
        }
    }
    external
}

fn external_linked_names(
    imports: &BTreeMap<String, usize>,
    packages: &[ResolvedPackage<'_>],
) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    for (alias, package_index) in imports {
        for (symbol, export) in &packages[*package_index].exports {
            names.insert(format!("{alias}::{symbol}"), export.linked_name.clone());
        }
    }
    names
}

fn validate_identifier(context: &str, name: &str) -> Result<(), LinkError> {
    let mut chars = name.chars();
    let first = chars.next();
    let valid = first.is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if !valid || name == "*" {
        return Err(LinkError::InvalidIdentifier {
            context: context.to_owned(),
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn validate_program_symbols(module: &ModuleUnit) -> Result<(), LinkError> {
    validate_identifier("source-unit name", &module.program.unit.name)?;
    let mut declarations = HashSet::new();
    for item in &module.program.items {
        let (name, is_function) = match item {
            Item::Function(function) => (Some(function.name.as_str()), true),
            Item::Struct(definition) => (Some(definition.name.as_str()), false),
            Item::ErrorEnum(definition) => (Some(definition.name.as_str()), false),
            Item::Const(constant) => (Some(constant.name.as_str()), false),
            Item::State(state) => (Some(state.name.as_str()), false),
            Item::Trigger(trigger) => (Some(trigger.name.as_str()), false),
        };
        let Some(name) = name else { continue };
        validate_identifier("source declaration", name)?;
        if !declarations.insert(name) {
            return Err(LinkError::DuplicateSymbol {
                source: module.source_name.clone(),
                symbol: name.to_owned(),
            });
        }
        if semantic::is_reserved_source_declaration(name, is_function) {
            return Err(LinkError::ReservedSymbol {
                source: module.source_name.clone(),
                symbol: name.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_module_items(module: &ModuleUnit) -> Result<(), LinkError> {
    for item in &module.program.items {
        let invalid = match item {
            Item::State(_) => Some("state declaration"),
            Item::Trigger(_) => Some("trigger declaration"),
            Item::Function(function)
                if function.modifiers.visibility != crate::ast::FunctionVisibility::Internal
                    || !matches!(
                        function.modifiers.kind,
                        crate::ast::FunctionKind::Free | crate::ast::FunctionKind::Contract
                    ) =>
            {
                Some("entrypoint declaration")
            }
            _ => None,
        };
        if let Some(item) = invalid {
            return Err(LinkError::InvalidModuleItem {
                source: module.source_name.clone(),
                item: item.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_imported_calls(
    module: &ModuleUnit,
    imports: &BTreeMap<String, usize>,
    packages: &[ResolvedPackage<'_>],
) -> Result<(), LinkError> {
    let mut calls = Vec::new();
    for item in &module.program.items {
        match item {
            Item::Function(function) => collect_block_calls(&function.body, &mut calls),
            Item::Const(constant) => collect_expr_calls(&constant.value, &mut calls),
            Item::Trigger(trigger) => {
                for metadata in &trigger.metadata {
                    collect_expr_calls(&metadata.value, &mut calls);
                }
            }
            Item::Struct(_) | Item::ErrorEnum(_) | Item::State(_) => {}
        }
    }
    for call in calls {
        if Builtin::from_source_name(call).is_some() || !call.contains("::") {
            continue;
        }
        let mut parts = call.split("::");
        let alias = parts.next().expect("split always has a first item");
        let symbol = parts.next();
        if alias == "*" || symbol == Some("*") || parts.next().is_some() {
            return Err(LinkError::WildcardImport {
                source: module.source_name.clone(),
            });
        }
        let symbol = symbol.unwrap_or_default();
        let package_index = imports
            .get(alias)
            .copied()
            .ok_or_else(|| LinkError::UnknownAlias {
                source: module.source_name.clone(),
                alias: alias.to_owned(),
            })?;
        if !packages[package_index].exports.contains_key(symbol) {
            return Err(LinkError::UnexportedSymbol {
                source: module.source_name.clone(),
                alias: alias.to_owned(),
                symbol: symbol.to_owned(),
            });
        }
    }
    Ok(())
}

fn collect_block_calls<'source>(block: &'source Block, calls: &mut Vec<&'source str>) {
    for statement in &block.statements {
        collect_statement_calls(statement, calls);
    }
}

fn collect_statement_calls<'source>(statement: &'source Statement, calls: &mut Vec<&'source str>) {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } | Statement::Expr(value) => {
            collect_expr_calls(value, calls);
        }
        Statement::AssignExpr { target, value, .. } => {
            collect_expr_calls(target, calls);
            collect_expr_calls(value, calls);
        }
        Statement::Return(Some(value)) => collect_expr_calls(value, calls),
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_calls(cond, calls);
            collect_block_calls(then_branch, calls);
            if let Some(branch) = else_branch {
                collect_block_calls(branch, calls);
            }
        }
        Statement::While { cond, body } => {
            collect_expr_calls(cond, calls);
            collect_block_calls(body, calls);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                collect_statement_calls(init, calls);
            }
            if let Some(cond) = cond {
                collect_expr_calls(cond, calls);
            }
            if let Some(step) = step {
                collect_statement_calls(step, calls);
            }
            collect_block_calls(body, calls);
        }
        Statement::ForEachMap { map, body, .. } => {
            collect_expr_calls(map, calls);
            collect_block_calls(body, calls);
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn collect_expr_calls<'source>(expr: &'source Expr, calls: &mut Vec<&'source str>) {
    match expr {
        Expr::Call { name, args } => {
            calls.push(name);
            for arg in args {
                collect_expr_calls(arg, calls);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_calls(left, calls);
            collect_expr_calls(right, calls);
        }
        Expr::Unary { expr, .. } => collect_expr_calls(expr, calls),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_expr_calls(cond, calls);
            collect_expr_calls(then_expr, calls);
            collect_expr_calls(else_expr, calls);
        }
        Expr::Member { object, .. } => collect_expr_calls(object, calls),
        Expr::Index { target, index } => {
            collect_expr_calls(target, calls);
            collect_expr_calls(index, calls);
        }
        Expr::Tuple(items) => {
            for item in items {
                collect_expr_calls(item, calls);
            }
        }
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Decimal(_)
        | Expr::String(_)
        | Expr::Bytes(_)
        | Expr::Ident(_) => {}
    }
}

fn qualify_signature(
    signature: &mut FunctionSignature,
    local_structs: &HashSet<String>,
    prefix: &str,
) {
    for param in &mut signature.params {
        qualify_type(&mut param.ty, local_structs, prefix);
    }
    qualify_type(&mut signature.return_type, local_structs, prefix);
}

fn qualify_type(ty: &mut Type, local_structs: &HashSet<String>, prefix: &str) {
    match ty {
        Type::Secret(inner) | Type::Option(inner) => qualify_type(inner, local_structs, prefix),
        Type::StateMap(key, value) | Type::Result(key, value) => {
            qualify_type(key, local_structs, prefix);
            qualify_type(value, local_structs, prefix);
        }
        Type::Tuple(items) => {
            for item in items {
                qualify_type(item, local_structs, prefix);
            }
        }
        Type::Struct { name, fields } => {
            if local_structs.contains(name) {
                *name = format!("{prefix}_{name}");
            }
            for (_, field) in fields {
                qualify_type(field, local_structs, prefix);
            }
        }
        Type::NamedStruct(name) if local_structs.contains(name) => {
            *name = format!("{prefix}_{name}");
        }
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
        | Type::Unit
        | Type::NamedStruct(_) => {}
    }
}

fn qualify_typed_program(
    program: &mut TypedProgram,
    local_structs: &HashSet<String>,
    prefix: &str,
) {
    for item in &mut program.items {
        let TypedItem::Function(function) = item;
        for param in &mut function.param_types {
            qualify_type(&mut param.ty, local_structs, prefix);
        }
        if let Some(return_type) = &mut function.ret_ty {
            qualify_type(return_type, local_structs, prefix);
        }
        qualify_block(&mut function.body, local_structs, prefix);
    }
}

fn qualify_block(block: &mut TypedBlock, local_structs: &HashSet<String>, prefix: &str) {
    for statement in &mut block.statements {
        qualify_statement(statement, local_structs, prefix);
    }
}

fn qualify_statement(
    statement: &mut TypedStatement,
    local_structs: &HashSet<String>,
    prefix: &str,
) {
    match statement {
        TypedStatement::Let { value, .. } | TypedStatement::Expr(value) => {
            qualify_expr(value, local_structs, prefix)
        }
        TypedStatement::Return(Some(value)) => qualify_expr(value, local_structs, prefix),
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            qualify_expr(cond, local_structs, prefix);
            qualify_block(then_branch, local_structs, prefix);
            if let Some(branch) = else_branch {
                qualify_block(branch, local_structs, prefix);
            }
        }
        TypedStatement::While { cond, body } => {
            qualify_expr(cond, local_structs, prefix);
            qualify_block(body, local_structs, prefix);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                qualify_statement(init, local_structs, prefix);
            }
            if let Some(cond) = cond {
                qualify_expr(cond, local_structs, prefix);
            }
            if let Some(step) = step {
                qualify_statement(step, local_structs, prefix);
            }
            qualify_block(body, local_structs, prefix);
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            qualify_expr(map, local_structs, prefix);
            qualify_block(body, local_structs, prefix);
        }
        TypedStatement::MapSet { map, key, value } => {
            qualify_expr(map, local_structs, prefix);
            qualify_expr(key, local_structs, prefix);
            qualify_expr(value, local_structs, prefix);
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
    }
}

fn qualify_expr(expr: &mut TypedExpr, local_structs: &HashSet<String>, prefix: &str) {
    qualify_type(&mut expr.ty, local_structs, prefix);
    match &mut expr.expr {
        ExprKind::Binary { left, right, .. } => {
            qualify_expr(left, local_structs, prefix);
            qualify_expr(right, local_structs, prefix);
        }
        ExprKind::Unary { expr, .. } | ExprKind::NumericCast { expr } => {
            qualify_expr(expr, local_structs, prefix)
        }
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            qualify_expr(cond, local_structs, prefix);
            qualify_expr(then_expr, local_structs, prefix);
            qualify_expr(else_expr, local_structs, prefix);
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) => {
            for arg in args {
                qualify_expr(arg, local_structs, prefix);
            }
        }
        ExprKind::Member { object, .. } => qualify_expr(object, local_structs, prefix),
        ExprKind::Index { target, index } => {
            qualify_expr(target, local_structs, prefix);
            qualify_expr(index, local_structs, prefix);
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => {}
    }
}

fn rename_program_calls(
    program: &mut TypedProgram,
    local_names: &BTreeMap<String, String>,
    external_names: &BTreeMap<String, String>,
) {
    for item in &mut program.items {
        let TypedItem::Function(function) = item;
        let original = function.name.clone();
        rename_block_calls(&mut function.body, local_names, external_names);
        if let Some(linked) = local_names.get(&original) {
            function.name = linked.clone();
        }
    }
}

fn rename_block_calls(
    block: &mut TypedBlock,
    local_names: &BTreeMap<String, String>,
    external_names: &BTreeMap<String, String>,
) {
    for statement in &mut block.statements {
        rename_statement_calls(statement, local_names, external_names);
    }
}

fn rename_statement_calls(
    statement: &mut TypedStatement,
    local_names: &BTreeMap<String, String>,
    external_names: &BTreeMap<String, String>,
) {
    match statement {
        TypedStatement::Let { value, .. } | TypedStatement::Expr(value) => {
            rename_expr_calls(value, local_names, external_names)
        }
        TypedStatement::Return(Some(value)) => {
            rename_expr_calls(value, local_names, external_names)
        }
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            rename_expr_calls(cond, local_names, external_names);
            rename_block_calls(then_branch, local_names, external_names);
            if let Some(branch) = else_branch {
                rename_block_calls(branch, local_names, external_names);
            }
        }
        TypedStatement::While { cond, body } => {
            rename_expr_calls(cond, local_names, external_names);
            rename_block_calls(body, local_names, external_names);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                rename_statement_calls(init, local_names, external_names);
            }
            if let Some(cond) = cond {
                rename_expr_calls(cond, local_names, external_names);
            }
            if let Some(step) = step {
                rename_statement_calls(step, local_names, external_names);
            }
            rename_block_calls(body, local_names, external_names);
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            rename_expr_calls(map, local_names, external_names);
            rename_block_calls(body, local_names, external_names);
        }
        TypedStatement::MapSet { map, key, value } => {
            rename_expr_calls(map, local_names, external_names);
            rename_expr_calls(key, local_names, external_names);
            rename_expr_calls(value, local_names, external_names);
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
    }
}

fn rename_expr_calls(
    expr: &mut TypedExpr,
    local_names: &BTreeMap<String, String>,
    external_names: &BTreeMap<String, String>,
) {
    match &mut expr.expr {
        ExprKind::Call { name, args } => {
            if let Some(linked) = local_names.get(name).or_else(|| external_names.get(name)) {
                *name = linked.clone();
            }
            for arg in args {
                rename_expr_calls(arg, local_names, external_names);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            rename_expr_calls(left, local_names, external_names);
            rename_expr_calls(right, local_names, external_names);
        }
        ExprKind::Unary { expr, .. } | ExprKind::NumericCast { expr } => {
            rename_expr_calls(expr, local_names, external_names)
        }
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            rename_expr_calls(cond, local_names, external_names);
            rename_expr_calls(then_expr, local_names, external_names);
            rename_expr_calls(else_expr, local_names, external_names);
        }
        ExprKind::Tuple(items) => {
            for item in items {
                rename_expr_calls(item, local_names, external_names);
            }
        }
        ExprKind::Member { object, .. } => rename_expr_calls(object, local_names, external_names),
        ExprKind::Index { target, index } => {
            rename_expr_calls(target, local_names, external_names);
            rename_expr_calls(index, local_names, external_names);
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn source(name: &str, source: &str) -> ModuleUnit {
        ModuleUnit {
            source_name: name.to_owned(),
            program: parse(source).expect("parse linker fixture"),
        }
    }

    fn package(modules: Vec<ModuleUnit>, exports: &[&str]) -> PackageUnit {
        PackageUnit {
            identity: "std/math@1.0.0".to_owned(),
            modules,
            exports: exports.iter().map(|name| (*name).to_owned()).collect(),
            imports: Vec::new(),
        }
    }

    fn request(root: ModuleUnit, package: PackageUnit) -> LinkRequest {
        LinkRequest {
            root,
            imports: vec![ImportBinding {
                alias: "arith".to_owned(),
                package: package.identity.clone(),
            }],
            packages: vec![package],
        }
    }

    fn transitive_source_request(base_source: &str) -> SourceLinkRequest {
        let base_identity = "std/base@1.0.0".to_owned();
        let derived_identity = "std/derived@1.0.0".to_owned();
        SourceLinkRequest {
            root: SourceModuleUnit {
                source_name: "app.ko".to_owned(),
                source: "seiyaku App { view fn run() -> i64 { return derived::value(); } }"
                    .to_owned(),
            },
            imports: vec![ImportBinding {
                alias: "derived".to_owned(),
                package: derived_identity.clone(),
            }],
            packages: vec![
                SourcePackageUnit {
                    identity: base_identity.clone(),
                    modules: vec![SourceModuleUnit {
                        source_name: "base.ko".to_owned(),
                        source: base_source.to_owned(),
                    }],
                    exports: BTreeSet::from(["value".to_owned()]),
                    imports: Vec::new(),
                },
                SourcePackageUnit {
                    identity: derived_identity,
                    modules: vec![SourceModuleUnit {
                        source_name: "derived.ko".to_owned(),
                        source:
                            "module Derived { fn value() -> i64 { return base::value() + 1; } }"
                                .to_owned(),
                    }],
                    exports: BTreeSet::from(["value".to_owned()]),
                    imports: vec![ImportBinding {
                        alias: "base".to_owned(),
                        package: base_identity,
                    }],
                },
            ],
        }
    }

    #[test]
    fn links_explicit_export_after_independent_type_analysis() {
        let linked = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> i64 { return arith::add(2, 3); } }",
                ),
                package(
                    vec![source(
                        "math.ko",
                        "module Math { fn add(left: i64, right: i64) -> i64 { return left + right; } }",
                    )],
                    &["add"],
                ),
            ))
            .expect("link typed HIR");

        assert_eq!(linked.unit.name, "App");
        assert_eq!(linked.items.len(), 2);
        let TypedItem::Function(root) = &linked.items[0];
        let TypedStatement::Return(Some(TypedExpr {
            expr: ExprKind::Call { name, .. },
            ..
        })) = &root.body.statements[0]
        else {
            panic!("expected linked call")
        };
        assert!(name.starts_with(LINKED_SYMBOL_PREFIX));
        let TypedItem::Function(module) = &linked.items[1];
        assert_eq!(name, &module.name);
    }

    #[test]
    fn rejects_unexported_and_unknown_calls() {
        let dependency = package(
            vec![source(
                "math.ko",
                "module Math { fn hidden() -> i64 { return 1; } }",
            )],
            &[],
        );
        let unexported = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> i64 { return arith::hidden(); } }",
                ),
                dependency,
            ))
            .expect_err("unexported function must fail");
        assert!(matches!(unexported, LinkError::UnexportedSymbol { .. }));

        let unknown = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> i64 { return other::add(); } }",
                ),
                package(
                    vec![source(
                        "math.ko",
                        "module Math { fn add() -> i64 { return 1; } }",
                    )],
                    &["add"],
                ),
            ))
            .expect_err("unknown alias must fail");
        assert!(matches!(unknown, LinkError::UnknownAlias { .. }));
    }

    #[test]
    fn rejects_import_aliases_that_collide_with_builtin_namespaces() {
        let dependency = package(
            vec![source(
                "hash.ko",
                "module Hash { fn sha256(value: bytes) -> bytes { return value; } }",
            )],
            &["sha256"],
        );
        for alias in ["crypto", "Amount"] {
            let mut request = request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run(value: bytes) -> bytes { return crypto::sha256(value); } }",
                ),
                dependency.clone(),
            );
            request.imports[0].alias = alias.to_owned();

            let error = TypedLinker::default()
                .link(request)
                .expect_err("compiler namespace import must be rejected as ambiguous");
            assert!(
                matches!(
                    error,
                    LinkError::ReservedImport {
                        alias: ref rejected,
                        ..
                    } if rejected == alias
                ),
                "{error:?}"
            );
        }
    }

    #[test]
    fn rejects_ambiguous_duplicate_export() {
        let error = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> i64 { return arith::value(); } }",
                ),
                package(
                    vec![
                        source("a.ko", "module A { fn value() -> i64 { return 1; } }"),
                        source("b.ko", "module B { fn value() -> i64 { return 2; } }"),
                    ],
                    &["value"],
                ),
            ))
            .expect_err("ambiguous export must fail");
        assert!(matches!(error, LinkError::AmbiguousExport { .. }));
    }

    #[test]
    fn same_private_function_name_in_two_modules_remains_module_local() {
        let linked = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> i64 { return arith::left() + arith::right(); } }",
                ),
                package(
                    vec![
                        source(
                            "left.ko",
                            "module Left { fn helper() -> i64 { return 1; } fn left() -> i64 { return helper(); } }",
                        ),
                        source(
                            "right.ko",
                            "module Right { fn helper() -> i64 { return 2; } fn right() -> i64 { return helper(); } }",
                        ),
                    ],
                    &["left", "right"],
                ),
            ))
            .expect("private names are scoped per module");

        let names = linked
            .items
            .iter()
            .map(|item| match item {
                TypedItem::Function(function) => function.name.clone(),
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), linked.items.len());
        assert_eq!(
            names
                .iter()
                .filter(|name| name.starts_with(LINKED_SYMBOL_PREFIX))
                .count(),
            4
        );
    }

    #[test]
    fn rejects_compiler_reserved_declaration() {
        let error = TypedLinker::default()
            .link(request(
                source("app.ko", "seiyaku App { view fn run() -> i64 { return math::ok(); } }"),
                package(
                    vec![source(
                        "reserved.ko",
                        "module Reserved { fn __kotodama_link_private() -> i64 { return 1; } fn ok() -> i64 { return 1; } }",
                    )],
                    &["ok"],
                ),
            ))
            .expect_err("reserved linker prefix must fail");
        assert!(matches!(error, LinkError::ReservedSymbol { .. }));
    }

    #[test]
    fn source_graph_parses_equal_contents_once_and_reuses_cache() {
        let graph = ModuleBuildGraph::default();
        let modules = vec![
            SourceModuleUnit {
                source_name: "first.ko".to_owned(),
                source: "module Shared { fn value() -> i64 { return 1; } }".to_owned(),
            },
            SourceModuleUnit {
                source_name: "second.ko".to_owned(),
                source: "module Shared { fn value() -> i64 { return 1; } }".to_owned(),
            },
        ];
        let first = graph
            .parse_sources(&modules)
            .expect("parse shared contents");
        assert_eq!(first[0], first[1]);
        assert_eq!(
            graph
                .parse_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        graph
            .parse_sources(&modules)
            .expect("reuse parsed source cache");
        assert_eq!(
            graph
                .parse_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            1,
            "an unchanged source must not be reparsed"
        );
    }

    #[test]
    fn reused_graph_parses_only_changes_and_rechecks_dependents() {
        let graph = ModuleBuildGraph::default();
        let first = graph
            .link(
                transitive_source_request("module Base { fn value() -> i64 { return 1; } }"),
                LinkerOptions::default(),
            )
            .expect("link initial transitive graph");
        assert_eq!(graph.parse_attempt_count(), 3);
        assert_eq!(graph.link_attempt_count(), 1);

        let implementation_changed = graph
            .link(
                transitive_source_request("module Base { fn value() -> i64 { return 2; } }"),
                LinkerOptions::default(),
            )
            .expect("implementation-only dependency change remains valid");
        assert_eq!(
            graph.parse_attempt_count(),
            4,
            "only the changed base module should be reparsed",
        );
        assert_eq!(
            graph.link_attempt_count(),
            2,
            "every changed graph must rerun whole-graph typed linking",
        );
        assert_ne!(first.fingerprint, implementation_changed.fingerprint);
        assert_ne!(
            first.program, implementation_changed.program,
            "the reused dependent must link against the changed implementation",
        );

        let error = graph
            .link(
                transitive_source_request(
                    "module Base { fn value(input: i64) -> i64 { return input; } }",
                ),
                LinkerOptions::default(),
            )
            .expect_err("a changed export signature must invalidate its dependent");
        assert!(
            matches!(
                error,
                SourceGraphError::Link(LinkError::Semantic { ref source, .. })
                    if source == "derived.ko"
            ),
            "{error:?}",
        );
        assert_eq!(
            graph.parse_attempt_count(),
            5,
            "the cached root and dependent ASTs must not be reparsed",
        );
        assert_eq!(
            graph.link_attempt_count(),
            3,
            "cached parsing must never suppress dependent semantic validation",
        );
    }

    #[test]
    fn source_cache_defends_against_adversarial_digest_collision() {
        let graph = ModuleBuildGraph::default();
        let modules = vec![
            SourceModuleUnit {
                source_name: "left.ko".to_owned(),
                source: "module Left { fn value() -> i64 { return 1; } }".to_owned(),
            },
            SourceModuleUnit {
                source_name: "right.ko".to_owned(),
                source: "module Right { fn value() -> i64 { return 2; } }".to_owned(),
            },
        ];
        let parsed = graph
            .parse_sources_with_digest(&modules, |_| "forced-collision".to_owned())
            .expect("exact source comparison must disambiguate a digest collision");
        assert_ne!(parsed[0].unit.name, parsed[1].unit.name);
        assert_eq!(
            graph
                .parse_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }

    #[test]
    fn parsed_source_cache_is_bounded_and_uses_lru_eviction() {
        let program = parse("module Cached { fn value() -> i64 { return 1; } }")
            .expect("parse cache fixture");
        let mut cache = ParsedSourceCache::default();
        for index in 0..MAX_PARSED_CACHE_ENTRIES {
            cache.insert(
                format!("digest-{index}"),
                format!("source-{index}"),
                program.clone(),
            );
        }
        assert_eq!(cache.entries.len(), MAX_PARSED_CACHE_ENTRIES);
        assert!(cache.get("digest-0", "source-0").is_some());

        cache.insert("digest-new".to_owned(), "source-new".to_owned(), program);
        assert_eq!(cache.entries.len(), MAX_PARSED_CACHE_ENTRIES);
        assert!(cache.get("digest-0", "source-0").is_some());
        assert!(cache.get("digest-1", "source-1").is_none());
        assert!(cache.source_bytes <= MAX_PARSED_CACHE_SOURCE_BYTES);
    }

    #[test]
    fn parsed_source_cache_enforces_aggregate_source_budget() {
        let program = parse("module Cached { fn value() -> i64 { return 1; } }")
            .expect("parse cache fixture");
        let mut cache = ParsedSourceCache::default();
        for index in 0..5 {
            let suffix = u8::try_from(index).expect("test index fits in u8");
            cache.insert(
                format!("large-{index}"),
                char::from(b'a' + suffix).to_string().repeat(1024 * 1024),
                program.clone(),
            );
        }
        assert!(cache.source_bytes <= MAX_PARSED_CACHE_SOURCE_BYTES);
        assert_eq!(cache.entries.len(), 4);
        assert!(cache.get("large-0", &"a".repeat(1024 * 1024)).is_none());
        assert!(cache.get("large-4", &"e".repeat(1024 * 1024)).is_some());
    }

    #[test]
    fn source_graph_rejects_excessive_module_count_before_parsing() {
        let modules = (0..MAX_MODULE_GRAPH_SOURCES)
            .map(|index| SourceModuleUnit {
                source_name: format!("module-{index}.ko"),
                source: "not parsed".to_owned(),
            })
            .collect();
        let request = SourceLinkRequest {
            root: SourceModuleUnit {
                source_name: "root.ko".to_owned(),
                source: "also not parsed".to_owned(),
            },
            imports: Vec::new(),
            packages: vec![SourcePackageUnit {
                identity: "oversized@1".to_owned(),
                modules,
                exports: BTreeSet::new(),
                imports: Vec::new(),
            }],
        };
        let error = validate_source_graph_budget(&request)
            .expect_err("root plus maximum modules exceeds the graph count");
        assert!(matches!(error, SourceGraphError::Budget { .. }));
    }

    #[test]
    fn source_graph_rejects_excessive_aggregate_bytes_before_parsing() {
        let request = SourceLinkRequest {
            root: SourceModuleUnit {
                source_name: "root.ko".to_owned(),
                source: "x".repeat(MAX_MODULE_GRAPH_SOURCE_BYTES + 1),
            },
            imports: Vec::new(),
            packages: Vec::new(),
        };
        let error = validate_source_graph_budget(&request)
            .expect_err("oversized aggregate source must fail before parsing");
        assert!(matches!(
            error,
            SourceGraphError::Budget {
                source_bytes,
                max_source_bytes: MAX_MODULE_GRAPH_SOURCE_BYTES,
                ..
            } if source_bytes == MAX_MODULE_GRAPH_SOURCE_BYTES + 1
        ));
    }

    #[test]
    fn graph_fingerprint_is_order_stable_and_binds_exports() {
        let root = SourceModuleUnit {
            source_name: "app.ko".to_owned(),
            source: "seiyaku App { view fn run() -> i64 { return arith::value(); } }".to_owned(),
        };
        let package = SourcePackageUnit {
            identity: "std/math@1.0.0".to_owned(),
            modules: vec![SourceModuleUnit {
                source_name: "math.ko".to_owned(),
                source: "module Math { fn value() -> i64 { return 1; } }".to_owned(),
            }],
            exports: ["value".to_owned()].into_iter().collect(),
            imports: Vec::new(),
        };
        let left = SourceLinkRequest {
            root: root.clone(),
            imports: vec![ImportBinding {
                alias: "arith".to_owned(),
                package: package.identity.clone(),
            }],
            packages: vec![package.clone()],
        };
        let mut changed = left.clone();
        changed.packages[0].exports.insert("other".to_owned());
        assert_ne!(
            source_graph_fingerprint(&left),
            source_graph_fingerprint(&changed),
            "export metadata participates in the graph identity"
        );

        let mut two_imports = left;
        two_imports.imports.push(ImportBinding {
            alias: "another".to_owned(),
            package: package.identity,
        });
        let mut reordered = two_imports.clone();
        reordered.imports.reverse();
        assert_eq!(
            source_graph_fingerprint(&two_imports),
            source_graph_fingerprint(&reordered),
            "incidental lockfile ordering must not invalidate the graph"
        );
    }
}
