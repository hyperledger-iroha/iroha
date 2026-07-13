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
    sync::{Arc, Mutex},
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
    spanned_ast::SpannedProgram,
};

const LINKED_SYMBOL_PREFIX: &str = "__kotodama_link_";
const MAX_PARSED_CACHE_ENTRIES: usize = 64;
const MAX_PARSED_CACHE_SOURCE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum number of source units in one typed module graph.
pub const MAX_MODULE_GRAPH_SOURCES: usize = 512;
/// Maximum aggregate UTF-8 bytes in one typed module graph.
pub const MAX_MODULE_GRAPH_SOURCE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum UTF-8 bytes in one portable logical source path.
pub const MAX_LOGICAL_SOURCE_PATH_BYTES: usize = 4096;

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
    /// CST-derived, fail-closed resolved source unit.
    pub program: crate::resolved::ResolvedProgram,
}

impl ModuleUnit {
    fn ast(&self) -> &Program {
        self.program.program()
    }
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

/// Complete request for linking one deployable seiyaku.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkRequest {
    /// The only deployable `seiyaku Name { ... }` source unit.
    pub root: ModuleUnit,
    /// Direct dependency aliases visible to the root seiyaku.
    pub imports: Vec<ImportBinding>,
    /// Locked transitive package graph.
    pub packages: Vec<PackageUnit>,
}

/// One reusable source unit before parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceModuleUnit {
    /// Portable relative logical path retained in diagnostics and source-map sidecars.
    ///
    /// Both slash spellings and lexical `.` components are accepted at the API
    /// boundary, then canonicalized before parsing. Absolute paths and paths
    /// that escape their package root are rejected.
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
    /// The single deployable `seiyaku`/`誓約` source.
    pub root: SourceModuleUnit,
    /// Direct dependency aliases visible to the root seiyaku.
    pub imports: Vec<ImportBinding>,
    /// Locked transitive package graph.
    pub packages: Vec<SourcePackageUnit>,
}

/// Complete source graph for validating one reusable package before publish.
///
/// Unlike [`SourceLinkRequest`], this graph has no deployable seiyaku root.
/// The package being published and every locked dependency must consist only
/// of production `module` units. All declared exports, imported calls, types,
/// and transitive effects are checked together after independent module
/// analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePackageGraphRequest {
    /// The local package being validated for publication.
    pub package: SourcePackageUnit,
    /// Authenticated, locked transitive dependency packages.
    pub dependencies: Vec<SourcePackageUnit>,
}

/// Linked typed-HIR plus the canonical identity of every graph input.
#[derive(Debug)]
pub struct LinkedSourceGraph {
    /// Fully resolved typed-HIR program accepted by the canonical compiler session.
    pub program: TypedProgram,
    /// Domain-separated digest of source contents, logical paths, imports, and exports.
    pub fingerprint: Hash,
}

/// Successful canonical validation of one reusable package graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedSourcePackageGraph {
    /// Domain-separated identity of the local and locked source graph.
    pub fingerprint: Hash,
    /// Unique production functions exposed by the local package manifest.
    pub exports: BTreeSet<String>,
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
    /// A parsed source unit failed declaration, type, or call resolution.
    Resolve {
        /// Logical source path associated with the diagnostics.
        source: String,
        /// Structured resolver diagnostics.
        diagnostics: DiagnosticBundle,
    },
    /// A source used a non-portable or non-relative logical path.
    InvalidSourcePath {
        /// Package identity containing the source, or `root` for the deployable source.
        scope: String,
        /// Rejected source-path spelling as supplied by the caller.
        source: String,
        /// Structured reason why the spelling cannot identify a V1 source.
        reason: InvalidSourcePathReason,
    },
    /// Two sources in the same package used the same normalized logical path.
    DuplicateSource {
        /// Package identity containing the collision.
        scope: String,
        /// Normalized logical source path.
        source: String,
    },
    /// Typed-HIR linking failed.
    Link(LinkError),
}

impl fmt::Display for SourceGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::Link(error) = self {
            return error.fmt(formatter);
        }
        write!(formatter, "[{}] ", self.diagnostic_code())?;
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
            Self::Resolve {
                source,
                diagnostics,
            } => write!(
                formatter,
                "Kotodama resolution error in `{source}`: {}",
                diagnostics.render_human()
            ),
            Self::InvalidSourcePath {
                scope,
                source,
                reason,
            } => write!(
                formatter,
                "Kotodama source path `{}` in `{scope}` is invalid: {reason}",
                source.escape_debug()
            ),
            Self::DuplicateSource { scope, source } => write!(
                formatter,
                "Kotodama package `{scope}` contains duplicate logical source `{source}`"
            ),
            Self::Link(_) => unreachable!("linked failures return before source-graph formatting"),
        }
    }
}

impl Error for SourceGraphError {}

impl SourceGraphError {
    /// Return the stable code for the primary source-graph failure.
    pub fn diagnostic_code(&self) -> &str {
        match self {
            Self::Budget { .. } => "E_PACKAGE_BUDGET",
            Self::Parse { diagnostics, .. } => diagnostics
                .diagnostics
                .first()
                .map_or("K1001", |diagnostic| diagnostic.code.as_str()),
            Self::Resolve { diagnostics, .. } => diagnostics
                .diagnostics
                .first()
                .map_or("K2002", |diagnostic| diagnostic.code.as_str()),
            Self::InvalidSourcePath { .. } => "E_INVALID_SOURCE_PATH",
            Self::DuplicateSource { .. } => "E_DUPLICATE_SOURCE",
            Self::Link(error) => error.diagnostic_code(),
        }
    }
}

/// Stable reason for rejecting a logical source path before parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidSourcePathReason {
    /// The spelling is empty or normalizes to no path components.
    Empty,
    /// The spelling is a POSIX, UNC, or backslash-rooted absolute path.
    Absolute,
    /// The spelling uses a Windows drive prefix, including drive-relative forms.
    WindowsDrive,
    /// A parent component would escape above the package source root.
    EscapesRoot,
    /// A non-special path component consists only of dots.
    DotOnlyComponent,
    /// A character is not portable in a logical source identity.
    NonPortableCharacter {
        /// UTF-8 byte offset of the rejected character.
        byte_offset: usize,
        /// Rejected character.
        character: char,
    },
    /// The path exceeds the fixed V1 metadata budget.
    TooLong {
        /// Supplied UTF-8 byte length.
        bytes: usize,
        /// Maximum accepted UTF-8 byte length.
        max_bytes: usize,
    },
}

impl fmt::Display for InvalidSourcePathReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("the logical path is empty after normalization"),
            Self::Absolute => formatter.write_str("logical paths must be relative"),
            Self::WindowsDrive => {
                formatter.write_str("Windows drive-prefixed logical paths are not allowed")
            }
            Self::EscapesRoot => {
                formatter.write_str("a parent component escapes the package source root")
            }
            Self::DotOnlyComponent => {
                formatter.write_str("dot-only file-name components are not portable")
            }
            Self::NonPortableCharacter {
                byte_offset,
                character,
            } => write!(
                formatter,
                "character `{}` at byte {byte_offset} is not portable",
                character.escape_debug()
            ),
            Self::TooLong { bytes, max_bytes } => write!(
                formatter,
                "the logical path contains {bytes} bytes; V1 permits at most {max_bytes}"
            ),
        }
    }
}

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
    program: SpannedProgram,
}

#[derive(Default)]
struct ParsedSourceCache {
    entries: VecDeque<CachedParsedSource>,
    source_bytes: usize,
}

impl ParsedSourceCache {
    fn get(&mut self, digest: &str, source: &str) -> Option<SpannedProgram> {
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

    fn insert(&mut self, digest: String, source: String, mut program: SpannedProgram) {
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
        program.rebase_source(SourceId(0));
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
        let names = validate_source_link_request(request)?;
        Ok(source_graph_fingerprint(request, &names))
    }

    /// Return the canonical identity of one reusable package source graph.
    pub fn package_fingerprint(
        request: &SourcePackageGraphRequest,
    ) -> Result<Hash, SourceGraphError> {
        let names = validate_source_package_graph_request(request)?;
        Ok(source_package_graph_fingerprint(request, &names))
    }

    /// Parse, resolve, and type-check one complete locked source graph.
    pub fn link(
        &self,
        mut request: SourceLinkRequest,
        options: LinkerOptions,
    ) -> Result<LinkedSourceGraph, SourceGraphError> {
        let names = validate_source_link_request(&request)?;
        let fingerprint = source_graph_fingerprint(&request, &names);
        canonicalize_source_link_request(&mut request, names);
        #[cfg(test)]
        self.link_attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut sources = Vec::new();
        sources.push(request.root.clone());
        for package in &request.packages {
            sources.extend(package.modules.iter().cloned());
        }
        let source_keys = std::iter::once(format!("root\0{}", request.root.source_name))
            .chain(request.packages.iter().flat_map(|package| {
                package
                    .modules
                    .iter()
                    .map(|module| format!("package\0{}\0{}", package.identity, module.source_name))
            }))
            .collect::<Vec<_>>();
        let source_ids = stable_source_ids(&source_keys);
        let programs = self
            .parse_sources_with_ids(&sources, &source_ids)?
            .into_iter()
            .zip(&sources)
            .zip(source_ids.iter().copied())
            .enumerate()
            .map(|(index, ((program, source), source_id))| {
                let imports = if index == 0 {
                    &request.imports
                } else {
                    let mut offset = 1_usize;
                    let mut selected = None;
                    for package in &request.packages {
                        let end = offset.saturating_add(package.modules.len());
                        if (offset..end).contains(&index) {
                            selected = Some(&package.imports);
                            break;
                        }
                        offset = end;
                    }
                    selected.expect("every non-root source belongs to a package")
                };
                let imports = imports
                    .iter()
                    .map(|binding| (binding.alias.clone(), ()))
                    .collect::<BTreeMap<_, _>>();
                let file = SourceFile::new(source_id, source.source_name.as_str(), &source.source);
                crate::resolved::resolve_with_imports(program, &file, &imports).map_err(
                    |diagnostics| SourceGraphError::Resolve {
                        source: source.source_name.clone(),
                        diagnostics,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut programs = programs.into_iter();
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

    /// Parse, resolve, type/effect-check, and validate one publishable package.
    ///
    /// No synthetic deployable root is created. Every source is parsed once
    /// through this graph's content-addressed parser, every package is resolved
    /// against its explicit locked imports, and whole-graph call/effect checks
    /// run over the resulting typed HIR.
    pub fn validate_package(
        &self,
        mut request: SourcePackageGraphRequest,
        options: LinkerOptions,
    ) -> Result<ValidatedSourcePackageGraph, SourceGraphError> {
        let names = validate_source_package_graph_request(&request)?;
        let fingerprint = source_package_graph_fingerprint(&request, &names);
        canonicalize_source_package_graph_request(&mut request, names);
        #[cfg(test)]
        self.link_attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let local_identity = request.package.identity.clone();
        let local_exports = request.package.exports.clone();
        let mut packages = Vec::with_capacity(1_usize.saturating_add(request.dependencies.len()));
        packages.push(request.package);
        packages.extend(request.dependencies);

        let sources = packages
            .iter()
            .flat_map(|package| package.modules.iter().cloned())
            .collect::<Vec<_>>();
        let source_keys = packages
            .iter()
            .flat_map(|package| {
                package
                    .modules
                    .iter()
                    .map(|module| format!("package\0{}\0{}", package.identity, module.source_name))
            })
            .collect::<Vec<_>>();
        let source_ids = stable_source_ids(&source_keys);
        let mut parsed = self
            .parse_sources_with_ids(&sources, &source_ids)?
            .into_iter();
        let mut source_ids = source_ids.into_iter();

        let mut resolved_packages = Vec::with_capacity(packages.len());
        for package in packages {
            let imports = package
                .imports
                .iter()
                .map(|binding| (binding.alias.clone(), ()))
                .collect::<BTreeMap<_, _>>();
            let mut modules = Vec::with_capacity(package.modules.len());
            for module in package.modules {
                let program = parsed
                    .next()
                    .expect("every package source has one parsed program");
                let source_id = source_ids
                    .next()
                    .expect("every package source has one stable source id");
                let file = SourceFile::new(
                    source_id,
                    module.source_name.as_str(),
                    module.source.as_str(),
                );
                let program = crate::resolved::resolve_with_imports(program, &file, &imports)
                    .map_err(|diagnostics| SourceGraphError::Resolve {
                        source: module.source_name.clone(),
                        diagnostics,
                    })?;
                modules.push(ModuleUnit {
                    source_name: module.source_name,
                    program,
                });
            }
            resolved_packages.push(PackageUnit {
                identity: package.identity,
                modules,
                exports: package.exports,
                imports: package.imports,
            });
        }
        debug_assert!(parsed.next().is_none());
        debug_assert!(source_ids.next().is_none());

        TypedLinker::new(options).validate_package_graph(resolved_packages, &local_identity)?;
        Ok(ValidatedSourcePackageGraph {
            fingerprint,
            exports: local_exports,
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

    fn parse_sources_with_ids(
        &self,
        sources: &[SourceModuleUnit],
        source_ids: &[SourceId],
    ) -> Result<Vec<SpannedProgram>, SourceGraphError> {
        self.parse_sources_with_digest(sources, source_ids, |source| {
            Hash::new_from_chunks(&[b"kotodama-module-source-v1\0", source.as_bytes()]).to_string()
        })
    }

    fn parse_sources_with_digest(
        &self,
        sources: &[SourceModuleUnit],
        source_ids: &[SourceId],
        digest: impl Fn(&str) -> String,
    ) -> Result<Vec<SpannedProgram>, SourceGraphError> {
        debug_assert_eq!(sources.len(), source_ids.len());
        struct UniqueSource {
            digest: String,
            source: String,
            source_name: String,
            members: Vec<usize>,
            program: Option<SpannedProgram>,
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
                            let result =
                                crate::parser::parse_source_spanned(&file, FrontendBudget::v1())
                                    .map(|(program, _)| program)
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
                programs[member] = Some(program.clone().with_source(source_ids[member]));
            }
        }
        Ok(programs
            .into_iter()
            .map(|program| program.expect("every source belongs to a unique group"))
            .collect())
    }
}

pub(crate) fn stable_source_ids(keys: &[String]) -> Vec<SourceId> {
    let mut order = keys.iter().enumerate().collect::<Vec<_>>();
    order.sort_by(|(left_index, left), (right_index, right)| {
        left.cmp(right).then_with(|| left_index.cmp(right_index))
    });
    let mut ids = vec![SourceId(0); keys.len()];
    for (ordinal, (index, _)) in order.into_iter().enumerate() {
        ids[index] =
            SourceId(u32::try_from(ordinal + 1).expect("module graph source budget fits u32"));
    }
    ids
}

struct CanonicalSourceLinkNames {
    root: String,
    packages: Vec<Vec<String>>,
}

struct CanonicalSourcePackageGraphNames {
    package: Vec<String>,
    dependencies: Vec<Vec<String>>,
}

fn validate_source_link_request(
    request: &SourceLinkRequest,
) -> Result<CanonicalSourceLinkNames, SourceGraphError> {
    validate_source_graph_budget(request)?;
    let root = canonical_logical_source_name("root", &request.root.source_name)?;
    let packages = validate_source_package_metadata(request.packages.iter())?;
    Ok(CanonicalSourceLinkNames { root, packages })
}

fn validate_source_package_graph_request(
    request: &SourcePackageGraphRequest,
) -> Result<CanonicalSourcePackageGraphNames, SourceGraphError> {
    validate_package_graph_budget(request)?;
    let mut names = validate_source_package_metadata(
        std::iter::once(&request.package).chain(request.dependencies.iter()),
    )?
    .into_iter();
    let package = names
        .next()
        .expect("package graph validation always includes the local package");
    let dependencies = names.collect();
    Ok(CanonicalSourcePackageGraphNames {
        package,
        dependencies,
    })
}

fn canonicalize_source_link_request(
    request: &mut SourceLinkRequest,
    names: CanonicalSourceLinkNames,
) {
    request.root.source_name = names.root;
    for (package, names) in request.packages.iter_mut().zip(names.packages) {
        canonicalize_source_package(package, names);
    }
    sort_imports(&mut request.imports);
    request
        .packages
        .sort_by(|left, right| left.identity.cmp(&right.identity));
}

fn canonicalize_source_package_graph_request(
    request: &mut SourcePackageGraphRequest,
    names: CanonicalSourcePackageGraphNames,
) {
    canonicalize_source_package(&mut request.package, names.package);
    for (dependency, names) in request.dependencies.iter_mut().zip(names.dependencies) {
        canonicalize_source_package(dependency, names);
    }
    request
        .dependencies
        .sort_by(|left, right| left.identity.cmp(&right.identity));
}

fn canonicalize_source_package(package: &mut SourcePackageUnit, names: Vec<String>) {
    assert_eq!(
        package.modules.len(),
        names.len(),
        "validated source names remain aligned with their package"
    );
    for (module, name) in package.modules.iter_mut().zip(names) {
        module.source_name = name;
    }
    package
        .modules
        .sort_by(|left, right| left.source_name.cmp(&right.source_name));
    sort_imports(&mut package.imports);
}

fn sort_imports(imports: &mut [ImportBinding]) {
    imports.sort_by(|left, right| {
        left.alias
            .cmp(&right.alias)
            .then_with(|| left.package.cmp(&right.package))
    });
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

fn validate_package_graph_budget(
    request: &SourcePackageGraphRequest,
) -> Result<(), SourceGraphError> {
    let sources = request
        .dependencies
        .iter()
        .fold(request.package.modules.len(), |total, package| {
            total.saturating_add(package.modules.len())
        });
    let source_bytes = std::iter::once(&request.package)
        .chain(request.dependencies.iter())
        .flat_map(|package| package.modules.iter())
        .fold(0_usize, |total, module| {
            total.saturating_add(module.source.len())
        });
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

fn validate_source_package_metadata<'a>(
    packages: impl IntoIterator<Item = &'a SourcePackageUnit>,
) -> Result<Vec<Vec<String>>, SourceGraphError> {
    let packages = packages.into_iter().collect::<Vec<_>>();
    let mut identities = BTreeSet::new();
    let mut canonical_sources = Vec::with_capacity(packages.len());
    for package in &packages {
        if !identities.insert(package.identity.as_str()) {
            return Err(LinkError::DuplicatePackage {
                package: package.identity.clone(),
            }
            .into());
        }
        if package.modules.is_empty() {
            return Err(LinkError::EmptyPackage {
                package: package.identity.clone(),
            }
            .into());
        }
        let mut sources = BTreeSet::new();
        let mut package_sources = Vec::with_capacity(package.modules.len());
        for module in &package.modules {
            let source = canonical_logical_source_name(&package.identity, &module.source_name)?;
            if !sources.insert(source.clone()) {
                return Err(SourceGraphError::DuplicateSource {
                    scope: package.identity.clone(),
                    source,
                });
            }
            package_sources.push(source);
        }
        canonical_sources.push(package_sources);
    }
    let package_indexes = packages
        .iter()
        .enumerate()
        .map(|(index, package)| (package.identity.clone(), index))
        .collect::<HashMap<_, _>>();
    let imports = packages
        .iter()
        .map(|package| resolve_imports(&package.identity, &package.imports, &package_indexes))
        .collect::<Result<Vec<_>, _>>()?;
    let identities = packages
        .iter()
        .map(|package| package.identity.clone())
        .collect::<Vec<_>>();
    validate_acyclic_package_imports(&identities, &imports)?;
    Ok(canonical_sources)
}

fn canonical_logical_source_name(scope: &str, source: &str) -> Result<String, SourceGraphError> {
    let invalid = |reason| SourceGraphError::InvalidSourcePath {
        scope: scope.to_owned(),
        source: source.to_owned(),
        reason,
    };
    if source.len() > MAX_LOGICAL_SOURCE_PATH_BYTES {
        return Err(invalid(InvalidSourcePathReason::TooLong {
            bytes: source.len(),
            max_bytes: MAX_LOGICAL_SOURCE_PATH_BYTES,
        }));
    }
    if source.is_empty() {
        return Err(invalid(InvalidSourcePathReason::Empty));
    }
    let source = source.replace('\\', "/");
    if source.starts_with('/') {
        return Err(invalid(InvalidSourcePathReason::Absolute));
    }
    let bytes = source.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(invalid(InvalidSourcePathReason::WindowsDrive));
    }
    if let Some((byte_offset, character)) = source
        .char_indices()
        .find(|(_, character)| *character == ':' || character.is_control())
    {
        return Err(invalid(InvalidSourcePathReason::NonPortableCharacter {
            byte_offset,
            character,
        }));
    }

    let mut components = Vec::new();
    for component in source.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(invalid(InvalidSourcePathReason::EscapesRoot));
                }
            }
            value if value.chars().all(|character| character == '.') => {
                return Err(invalid(InvalidSourcePathReason::DotOnlyComponent));
            }
            value => components.push(value),
        }
    }
    let normalized = components.join("/");
    if normalized.is_empty() {
        return Err(invalid(InvalidSourcePathReason::Empty));
    }
    Ok(normalized)
}

fn source_graph_fingerprint(request: &SourceLinkRequest, names: &CanonicalSourceLinkNames) -> Hash {
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
    field(&mut transcript, &names.root);
    field(&mut transcript, &request.root.source);
    imports(&mut transcript, &request.imports);
    let mut packages = request
        .packages
        .iter()
        .zip(&names.packages)
        .collect::<Vec<_>>();
    packages.sort_by(|(left, _), (right, _)| left.identity.cmp(&right.identity));
    field(&mut transcript, (packages.len() as u64).to_le_bytes());
    for (package, names) in packages {
        field(&mut transcript, &package.identity);
        imports(&mut transcript, &package.imports);
        field(
            &mut transcript,
            (package.exports.len() as u64).to_le_bytes(),
        );
        for export in &package.exports {
            field(&mut transcript, export);
        }
        let mut modules = package.modules.iter().zip(names).collect::<Vec<_>>();
        modules.sort_by(|(_, left), (_, right)| left.cmp(right));
        field(&mut transcript, (modules.len() as u64).to_le_bytes());
        for (module, name) in modules {
            field(&mut transcript, name);
            field(&mut transcript, &module.source);
        }
    }
    Hash::new(transcript)
}

fn source_package_graph_fingerprint(
    request: &SourcePackageGraphRequest,
    names: &CanonicalSourcePackageGraphNames,
) -> Hash {
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

    fn package(transcript: &mut Vec<u8>, value: &SourcePackageUnit, names: &[String]) {
        field(transcript, &value.identity);
        imports(transcript, &value.imports);
        field(transcript, (value.exports.len() as u64).to_le_bytes());
        for export in &value.exports {
            field(transcript, export);
        }
        let mut modules = value.modules.iter().zip(names).collect::<Vec<_>>();
        modules.sort_by(|(_, left), (_, right)| left.cmp(right));
        field(transcript, (modules.len() as u64).to_le_bytes());
        for (module, name) in modules {
            field(transcript, name);
            field(transcript, &module.source);
        }
    }

    let mut transcript = b"kotodama-source-package-graph-v1\0".to_vec();
    field(&mut transcript, b"local");
    package(&mut transcript, &request.package, &names.package);
    field(&mut transcript, b"dependencies");
    let mut dependencies = request
        .dependencies
        .iter()
        .zip(&names.dependencies)
        .collect::<Vec<_>>();
    dependencies.sort_by(|(left, _), (right, _)| left.identity.cmp(&right.identity));
    field(&mut transcript, (dependencies.len() as u64).to_le_bytes());
    for (dependency, names) in dependencies {
        package(&mut transcript, dependency, names);
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
    /// The deployable root was not a `seiyaku`/`誓約` source unit.
    RootMustBeSeiyaku {
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
    /// A package contained no reusable module source units.
    EmptyPackage {
        /// Canonical package identity.
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
    /// Locked package imports formed a dependency cycle.
    PackageImportCycle {
        /// Deterministic closed path whose final identity repeats the first.
        cycle: Vec<String>,
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
        /// Stable semantic diagnostic code propagated without parsing prose.
        diagnostic_code: &'static str,
        /// Semantic failure message.
        message: String,
    },
}

impl LinkError {
    /// Return the stable code for this typed-link failure.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::RootMustBeSeiyaku { .. } => "E_ROOT_MUST_BE_SEIYAKU",
            Self::DependencyMustBeModule { .. } => "E_DEPENDENCY_MUST_BE_MODULE",
            Self::DuplicatePackage { .. } => "E_DUPLICATE_PACKAGE",
            Self::EmptyPackage { .. } => "E_EMPTY_PACKAGE",
            Self::DuplicateModule { .. } => "E_DUPLICATE_MODULE",
            Self::DuplicateImport { .. } => "E_DUPLICATE_IMPORT",
            Self::ReservedImport { .. } => "E_RESERVED_IMPORT",
            Self::DuplicateSymbol { .. } => "E_DUPLICATE_DECLARATION",
            Self::UnknownPackage { .. } => "E_UNKNOWN_PACKAGE",
            Self::PackageImportCycle { .. } => "E_PACKAGE_IMPORT_CYCLE",
            Self::UnknownAlias { .. } => "E_UNKNOWN_IMPORT_ALIAS",
            Self::UnexportedSymbol { .. } => "E_UNEXPORTED_SYMBOL",
            Self::MissingExport { .. } => "E_MISSING_EXPORT",
            Self::AmbiguousExport { .. } => "E_AMBIGUOUS_EXPORT",
            Self::WildcardImport { .. } => "E_WILDCARD_IMPORT",
            Self::InvalidIdentifier { .. } => "E_INVALID_IDENTIFIER",
            Self::ReservedSymbol { .. } => "E_RESERVED_DECLARATION",
            Self::InvalidModuleItem { .. } => "E_INVALID_MODULE_ITEM",
            Self::DuplicateErrorCode { .. } => "E_DUPLICATE_ERROR_CODE",
            Self::DuplicateMessage { .. } => "E_DUPLICATE_MESSAGE",
            Self::Semantic {
                diagnostic_code, ..
            } => diagnostic_code,
        }
    }
}

impl fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "[{}] ", self.diagnostic_code())?;
        match self {
            Self::RootMustBeSeiyaku { source } => {
                write!(
                    formatter,
                    "deployable root `{source}` must declare exactly one `seiyaku`/`誓約`"
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
            Self::EmptyPackage { package } => {
                write!(
                    formatter,
                    "package `{package}` contains no Kotodama modules"
                )
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
            Self::PackageImportCycle { cycle } => write!(
                formatter,
                "locked package import cycle is not permitted: {}",
                cycle.join(" -> ")
            ),
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
                    "linked modules assign duplicate seiyaku error code {code}"
                )
            }
            Self::DuplicateMessage { key } => {
                write!(
                    formatter,
                    "linked modules define duplicate messages key `{key}`"
                )
            }
            Self::Semantic {
                source, message, ..
            } => {
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

    /// Resolve and link one seiyaku plus its locked module graph.
    pub fn link(&self, mut request: LinkRequest) -> Result<TypedProgram, LinkError> {
        validate_linker_options(self.options)?;
        if request.root.ast().unit.kind != SourceUnitKind::Seiyaku {
            return Err(LinkError::RootMustBeSeiyaku {
                source: request.root.source_name,
            });
        }
        validate_program_symbols(&request.root)?;

        let resolved_packages = resolve_packages(self.options, &mut request.packages)?;
        let package_indexes = resolved_packages
            .iter()
            .enumerate()
            .map(|(index, package)| (package.identity.clone(), index))
            .collect::<HashMap<_, _>>();
        let root_imports = resolve_imports("root", &request.imports, &package_indexes)?;

        validate_imported_calls(&request.root, &root_imports, &resolved_packages)?;
        let root_external = external_signatures(&root_imports, &resolved_packages);
        let semantic = semantic::SemanticContext::with_capabilities(
            self.options.zk_enabled,
            self.options.test_builtins_enabled,
        );
        let mut root = semantic
            .analyze_resolved_with_external_functions(&request.root.program, &root_external)
            .map_err(|error| LinkError::Semantic {
                source: request.root.source_name.clone(),
                diagnostic_code: error.code,
                message: error.message,
            })?;
        let root_external_names = external_linked_names(&root_imports, &resolved_packages);
        rename_program_calls(&mut root, &BTreeMap::new(), &root_external_names);
        link_resolved_packages(self.options, &resolved_packages, Some(root))
    }

    /// Validate a reusable package and all locked dependencies as typed HIR.
    ///
    /// Every package must contain only production module declarations. The
    /// local identity must be present exactly once; export and import checks
    /// apply uniformly to the local package and dependencies.
    pub fn validate_package_graph(
        &self,
        mut packages: Vec<PackageUnit>,
        local_identity: &str,
    ) -> Result<(), LinkError> {
        validate_linker_options(self.options)?;
        if !packages
            .iter()
            .any(|package| package.identity == local_identity)
        {
            return Err(LinkError::UnknownPackage {
                scope: "published package".to_owned(),
                package: local_identity.to_owned(),
            });
        }
        let resolved_packages = resolve_packages(self.options, &mut packages)?;
        link_resolved_packages(self.options, &resolved_packages, None).map(|_| ())
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
    identity: String,
    imports: BTreeMap<String, usize>,
    modules: Vec<ResolvedModule<'request>>,
    exports: BTreeMap<String, ResolvedExport>,
}

fn validate_linker_options(options: LinkerOptions) -> Result<(), LinkError> {
    if options.include_tests != options.test_builtins_enabled {
        return Err(LinkError::Semantic {
            source: "linker options".to_owned(),
            diagnostic_code: "E_TEST_ONLY_PRODUCTION",
            message: "include_tests and test_builtins_enabled must select one explicit compiler test mode together"
                .to_owned(),
        });
    }
    Ok(())
}

fn resolve_packages<'request>(
    options: LinkerOptions,
    packages: &'request mut [PackageUnit],
) -> Result<Vec<ResolvedPackage<'request>>, LinkError> {
    packages.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut package_identities = HashSet::new();
    for package in packages.iter_mut() {
        if !package_identities.insert(package.identity.clone()) {
            return Err(LinkError::DuplicatePackage {
                package: package.identity.clone(),
            });
        }
        if package.modules.is_empty() {
            return Err(LinkError::EmptyPackage {
                package: package.identity.clone(),
            });
        }
        package.modules.sort_by(|left, right| {
            left.ast()
                .unit
                .name
                .cmp(&right.ast().unit.name)
                .then_with(|| left.source_name.cmp(&right.source_name))
        });
        let mut module_names = HashSet::new();
        for module in &mut package.modules {
            if module.ast().unit.kind != SourceUnitKind::Module {
                return Err(LinkError::DependencyMustBeModule {
                    source: module.source_name.clone(),
                });
            }
            if !module_names.insert(module.ast().unit.name.clone()) {
                return Err(LinkError::DuplicateModule {
                    package: package.identity.clone(),
                    module: module.ast().unit.name.clone(),
                });
            }
            validate_program_symbols(module)?;
            validate_module_items(module)?;
        }
    }

    let package_indexes = packages
        .iter()
        .enumerate()
        .map(|(index, package)| (package.identity.clone(), index))
        .collect::<HashMap<_, _>>();
    let resolved_imports = packages
        .iter()
        .map(|package| resolve_imports(&package.identity, &package.imports, &package_indexes))
        .collect::<Result<Vec<_>, _>>()?;
    let package_identities = packages
        .iter()
        .map(|package| package.identity.clone())
        .collect::<Vec<_>>();
    validate_acyclic_package_imports(&package_identities, &resolved_imports)?;
    let mut resolved_packages = Vec::with_capacity(packages.len());
    for (package_index, (package, imports)) in packages.iter().zip(resolved_imports).enumerate() {
        let mut modules = Vec::with_capacity(package.modules.len());
        for (module_index, module) in package.modules.iter().enumerate() {
            let semantic = semantic::SemanticContext::with_capabilities(
                options.zk_enabled,
                options.test_builtins_enabled,
            );
            let mut signatures = semantic
                .resolve_resolved_function_signatures(&module.program)
                .map_err(|error| LinkError::Semantic {
                    source: module.source_name.clone(),
                    diagnostic_code: error.code,
                    message: error.message,
                })?;
            let local_structs = module
                .ast()
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Struct(definition) => Some(definition.name.clone()),
                    _ => None,
                })
                .collect::<HashSet<_>>();
            let type_prefix = format!("{LINKED_SYMBOL_PREFIX}p{package_index}_m{module_index}_t");
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
            identity: package.identity.clone(),
            imports,
            modules,
            exports,
        });
    }
    Ok(resolved_packages)
}

fn validate_acyclic_package_imports(
    identities: &[String],
    imports: &[BTreeMap<String, usize>],
) -> Result<(), LinkError> {
    debug_assert_eq!(identities.len(), imports.len());
    let edges = imports
        .iter()
        .map(|imports| {
            let mut edges = imports.values().copied().collect::<Vec<_>>();
            edges.sort_unstable();
            edges.dedup();
            edges
        })
        .collect::<Vec<_>>();
    let mut state = vec![0_u8; identities.len()];

    for start in 0..identities.len() {
        if state[start] != 0 {
            continue;
        }
        state[start] = 1;
        let mut path = vec![start];
        let mut stack = vec![(start, 0_usize)];
        while let Some((package, next_edge)) = stack.last_mut() {
            if let Some(&dependency) = edges[*package].get(*next_edge) {
                *next_edge = next_edge.saturating_add(1);
                match state[dependency] {
                    0 => {
                        state[dependency] = 1;
                        path.push(dependency);
                        stack.push((dependency, 0));
                    }
                    1 => {
                        let start = path
                            .iter()
                            .position(|candidate| *candidate == dependency)
                            .expect("active package is present in the DFS path");
                        let mut cycle = path[start..]
                            .iter()
                            .map(|index| identities[*index].clone())
                            .collect::<Vec<_>>();
                        cycle.push(identities[dependency].clone());
                        return Err(LinkError::PackageImportCycle { cycle });
                    }
                    _ => {}
                }
                continue;
            }

            let (finished, _) = stack.pop().expect("non-empty package DFS stack");
            path.pop().expect("package DFS path mirrors its stack");
            state[finished] = 2;
        }
    }
    Ok(())
}

fn link_resolved_packages(
    options: LinkerOptions,
    packages: &[ResolvedPackage<'_>],
    mut linked: Option<TypedProgram>,
) -> Result<TypedProgram, LinkError> {
    let mut seen_error_codes = linked
        .iter()
        .flat_map(|program| program.error_codes.iter())
        .map(|error| error.code)
        .collect::<HashSet<_>>();
    let mut seen_messages = linked
        .iter()
        .flat_map(|program| program.message_entries.iter())
        .map(|entry| entry.msg_id.clone())
        .collect::<HashSet<_>>();

    for (package_index, package) in packages.iter().enumerate() {
        let external = external_signatures(&package.imports, packages);
        let external_names = external_linked_names(&package.imports, packages);
        for (module_index, module) in package.modules.iter().enumerate() {
            validate_imported_calls(module.source, &package.imports, packages)?;
            let semantic = semantic::SemanticContext::with_capabilities(
                options.zk_enabled,
                options.test_builtins_enabled,
            );
            let mut typed = semantic
                .analyze_resolved_with_external_functions(&module.source.program, &external)
                .map_err(|error| LinkError::Semantic {
                    source: module.source.source_name.clone(),
                    diagnostic_code: error.code,
                    message: error.message,
                })?;
            qualify_typed_program(&mut typed, &module.local_structs, &module.type_prefix);
            rename_program_calls(&mut typed, &module.linked_names, &external_names);

            for error in &mut typed.error_codes {
                if !seen_error_codes.insert(error.code) {
                    return Err(LinkError::DuplicateErrorCode { code: error.code });
                }
                error.namespace = format!(
                    "{LINKED_SYMBOL_PREFIX}p{package_index}_m{module_index}_{}",
                    error.namespace
                );
            }
            for message in &typed.message_entries {
                if !seen_messages.insert(message.msg_id.clone()) {
                    return Err(LinkError::DuplicateMessage {
                        key: message.msg_id.clone(),
                    });
                }
            }

            if let Some(program) = &mut linked {
                for (id, node) in std::mem::take(&mut typed.hir_nodes) {
                    if program.hir_nodes.insert(id, node).is_some() {
                        return Err(LinkError::Semantic {
                            source: module.source.source_name.clone(),
                            diagnostic_code: "E_INTERNAL_RESOLUTION",
                            message: format!(
                                "typed module graph reused HIR identity {}:{}",
                                id.source.0, id.local.0
                            ),
                        });
                    }
                }
                for (source_id, source_file) in std::mem::take(&mut typed.source_files) {
                    if let Some(previous) =
                        program.source_files.insert(source_id, source_file.clone())
                        && previous != source_file
                    {
                        return Err(LinkError::Semantic {
                            source: source_file.name().to_owned(),
                            diagnostic_code: "E_INTERNAL_RESOLUTION",
                            message: format!(
                                "compiler assigned SourceId {} to both `{}` and `{}`",
                                source_id.0,
                                previous.name(),
                                source_file.name()
                            ),
                        });
                    }
                }
                program.items.extend(typed.items);
                program.states.extend(typed.states);
                program.error_codes.extend(typed.error_codes);
                program.triggers.extend(typed.triggers);
                program.message_entries.extend(typed.message_entries);
                program.test_support_enabled |= typed.test_support_enabled;
            } else {
                linked = Some(typed);
            }
        }
    }

    let linked = linked.ok_or_else(|| LinkError::Semantic {
        source: "package graph".to_owned(),
        diagnostic_code: "E_EMPTY_PACKAGE_GRAPH",
        message: "package graph contains no typed modules".to_owned(),
    })?;
    semantic::validate_linked_program(&linked, options.zk_enabled).map_err(|error| {
        LinkError::Semantic {
            source: "linked program".to_owned(),
            diagnostic_code: error.code,
            message: error.message,
        }
    })?;
    Ok(linked)
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
/// cannot accept an alias that will only fail later during seiyaku linking.
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
    validate_identifier("source-unit name", &module.ast().unit.name)?;
    let mut declarations = HashSet::new();
    for item in &module.ast().items {
        let (name, is_function, is_type) = match item {
            Item::Function(function) => (Some(function.name.as_str()), true, false),
            Item::Struct(definition) => (Some(definition.name.as_str()), false, true),
            Item::ErrorEnum(definition) => (Some(definition.name.as_str()), false, true),
            Item::Const(constant) => (Some(constant.name.as_str()), false, false),
            Item::State(state) => (Some(state.name.as_str()), false, false),
            Item::Trigger(trigger) => (Some(trigger.name.as_str()), false, false),
        };
        let Some(name) = name else { continue };
        validate_identifier("source declaration", name)?;
        if !declarations.insert(name) {
            return Err(LinkError::DuplicateSymbol {
                source: module.source_name.clone(),
                symbol: name.to_owned(),
            });
        }
        let reserved = if is_type {
            semantic::is_reserved_source_type_declaration(name)
        } else {
            semantic::is_reserved_source_declaration(name, is_function)
        };
        if reserved {
            return Err(LinkError::ReservedSymbol {
                source: module.source_name.clone(),
                symbol: name.to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_module_items(module: &ModuleUnit) -> Result<(), LinkError> {
    for item in &module.ast().items {
        let invalid = match item {
            Item::State(_) => Some("state declaration"),
            Item::Trigger(_) => Some("trigger declaration"),
            Item::Function(function)
                if function.modifiers.kind != crate::ast::FunctionKind::Private =>
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
    for item in &module.ast().items {
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
    if let Some(tail) = &block.tail {
        collect_expr_calls(tail, calls);
    }
}

fn collect_statement_calls<'source>(statement: &'source Statement, calls: &mut Vec<&'source str>) {
    match statement.kind() {
        Statement::Source { .. } | Statement::Resolved { .. } => {
            unreachable!("kind() strips provenance wrappers")
        }
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
        Statement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_calls(value, calls);
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
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            collect_expr_calls(expression, calls);
        }
        Expr::Call { name, args, .. } => {
            calls.push(name);
            for arg in args {
                collect_expr_calls(arg, calls);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_calls(left, calls);
            collect_expr_calls(right, calls);
        }
        Expr::Unary { expr, .. }
        | Expr::OptionSome(expr)
        | Expr::ResultOk(expr)
        | Expr::ResultErr(expr)
        | Expr::Propagate(expr) => collect_expr_calls(expr, calls),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_expr_calls(cond, calls);
            collect_expr_calls(then_expr, calls);
            collect_expr_calls(else_expr, calls);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_calls(condition, calls);
            collect_block_calls(then_branch, calls);
            if let Some(branch) = else_branch {
                collect_block_calls(branch, calls);
            }
        }
        Expr::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_calls(value, calls);
            collect_block_calls(then_branch, calls);
            if let Some(branch) = else_branch {
                collect_block_calls(branch, calls);
            }
        }
        Expr::Match { value, arms } => {
            collect_expr_calls(value, calls);
            for arm in arms {
                collect_block_calls(&arm.body, calls);
            }
        }
        Expr::Member { object, .. } => collect_expr_calls(object, calls),
        Expr::Index { target, index } => {
            collect_expr_calls(target, calls);
            collect_expr_calls(index, calls);
        }
        Expr::Tuple(items) | Expr::List(items) => {
            for item in items {
                collect_expr_calls(item, calls);
            }
        }
        Expr::JsonObject(entries) => {
            for entry in entries {
                collect_expr_calls(&entry.value, calls);
            }
        }
        Expr::JsonArray(elements) => {
            for element in elements {
                collect_expr_calls(element, calls);
            }
        }
        Expr::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            collect_expr_calls(source, calls);
            collect_expr_calls(expression, calls);
            if let Some(condition) = condition {
                collect_expr_calls(condition, calls);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for field in fields {
                collect_expr_calls(&field.value, calls);
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
        Type::Secret(inner) | Type::Option(inner) | Type::List(inner, _) => {
            qualify_type(inner, local_structs, prefix);
        }
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
            for (_, field) in Arc::make_mut(fields) {
                qualify_type(field, local_structs, prefix);
            }
        }
        Type::NamedStruct(name) if local_structs.contains(name) => {
            *name = format!("{prefix}_{name}");
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
    if let Some(tail) = &mut block.tail {
        qualify_expr(tail, local_structs, prefix);
    }
}

fn qualify_statement(
    statement: &mut TypedStatement,
    local_structs: &HashSet<String>,
    prefix: &str,
) {
    match statement.kind_mut() {
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
        TypedStatement::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => {
            if let Some(payload) = &mut pattern.payload_type {
                qualify_type(payload, local_structs, prefix);
            }
            qualify_expr(value, local_structs, prefix);
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
    match expr.kind_mut() {
        ExprKind::Binary { left, right, .. } => {
            qualify_expr(left, local_structs, prefix);
            qualify_expr(right, local_structs, prefix);
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => qualify_expr(expr, local_structs, prefix),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            qualify_expr(cond, local_structs, prefix);
            qualify_expr(then_expr, local_structs, prefix);
            qualify_expr(else_expr, local_structs, prefix);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            qualify_expr(condition, local_structs, prefix);
            qualify_block(then_branch, local_structs, prefix);
            qualify_block(else_branch, local_structs, prefix);
        }
        ExprKind::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => {
            if let Some(payload) = &mut pattern.payload_type {
                qualify_type(payload, local_structs, prefix);
            }
            qualify_expr(value, local_structs, prefix);
            qualify_block(then_branch, local_structs, prefix);
            qualify_block(else_branch, local_structs, prefix);
        }
        ExprKind::Match { value, arms } => {
            qualify_expr(value, local_structs, prefix);
            for arm in arms {
                if let Some(payload) = &mut arm.pattern.payload_type {
                    qualify_type(payload, local_structs, prefix);
                }
                qualify_block(&mut arm.body, local_structs, prefix);
            }
        }
        ExprKind::Call { args, .. }
        | ExprKind::NamedCall { args, .. }
        | ExprKind::Tuple(args)
        | ExprKind::List(args) => {
            for arg in args {
                qualify_expr(arg, local_structs, prefix);
            }
        }
        ExprKind::JsonObject(entries) => {
            for (_, value) in entries {
                qualify_expr(value, local_structs, prefix);
            }
        }
        ExprKind::JsonArray(elements) => {
            for element in elements {
                qualify_expr(element, local_structs, prefix);
            }
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            qualify_expr(source, local_structs, prefix);
            qualify_expr(expression, local_structs, prefix);
            if let Some(condition) = condition {
                qualify_expr(condition, local_structs, prefix);
            }
        }
        ExprKind::StructLiteral { name, fields } => {
            if local_structs.contains(name) {
                *name = format!("{prefix}{name}");
            }
            for (_, value) in fields {
                qualify_expr(value, local_structs, prefix);
            }
        }
        ExprKind::Member { object, .. } => qualify_expr(object, local_structs, prefix),
        ExprKind::Index { target, index } => {
            qualify_expr(target, local_structs, prefix);
            qualify_expr(index, local_structs, prefix);
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
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
    if let Some(tail) = &mut block.tail {
        rename_expr_calls(tail, local_names, external_names);
    }
}

fn rename_statement_calls(
    statement: &mut TypedStatement,
    local_names: &BTreeMap<String, String>,
    external_names: &BTreeMap<String, String>,
) {
    match statement.kind_mut() {
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
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            rename_expr_calls(value, local_names, external_names);
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
    match expr.kind_mut() {
        ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
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
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => {
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
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            rename_expr_calls(condition, local_names, external_names);
            rename_block_calls(then_branch, local_names, external_names);
            rename_block_calls(else_branch, local_names, external_names);
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            rename_expr_calls(value, local_names, external_names);
            rename_block_calls(then_branch, local_names, external_names);
            rename_block_calls(else_branch, local_names, external_names);
        }
        ExprKind::Match { value, arms } => {
            rename_expr_calls(value, local_names, external_names);
            for arm in arms {
                rename_block_calls(&mut arm.body, local_names, external_names);
            }
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            for item in items {
                rename_expr_calls(item, local_names, external_names);
            }
        }
        ExprKind::JsonObject(entries) => {
            for (_, value) in entries {
                rename_expr_calls(value, local_names, external_names);
            }
        }
        ExprKind::JsonArray(elements) => {
            for element in elements {
                rename_expr_calls(element, local_names, external_names);
            }
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            rename_expr_calls(source, local_names, external_names);
            rename_expr_calls(expression, local_names, external_names);
            if let Some(condition) = condition {
                rename_expr_calls(condition, local_names, external_names);
            }
        }
        ExprKind::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                rename_expr_calls(value, local_names, external_names);
            }
        }
        ExprKind::Member { object, .. } => rename_expr_calls(object, local_names, external_names),
        ExprKind::Index { target, index } => {
            rename_expr_calls(target, local_names, external_names);
            rename_expr_calls(index, local_names, external_names);
        }
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_graph_preserves_semantic_code_independently_of_localized_message() {
        let error = SourceGraphError::from(LinkError::Semantic {
            source: "localized.ko".to_owned(),
            diagnostic_code: "E_LIST_CAPACITY",
            message: "la capacité dépasse la limite".to_owned(),
        });
        assert_eq!(error.diagnostic_code(), "E_LIST_CAPACITY");
        assert!(error.to_string().contains("[E_LIST_CAPACITY]"));
        assert!(error.to_string().contains("la capacité dépasse la limite"));
    }

    fn spanned(source: &str) -> SpannedProgram {
        let file = SourceFile::new(SourceId(0), "cache-fixture.ko", source);
        crate::parser::parse_source_spanned(&file, FrontendBudget::v1())
            .map(|(program, _)| program)
            .expect("parse spanned linker fixture")
    }

    fn source(name: &str, source: &str) -> ModuleUnit {
        let source_id = SourceId(
            name.bytes()
                .fold(2_166_136_261_u32, |hash, byte| {
                    hash.wrapping_mul(16_777_619) ^ u32::from(byte)
                })
                .max(1),
        );
        let file = SourceFile::new(source_id, name, source);
        let (program, _) = crate::parser::parse_source_spanned(&file, FrontendBudget::v1())
            .expect("parse linker fixture");
        let imports = program
            .facts
            .calls
            .iter()
            .filter_map(|call| {
                call.name
                    .split_once("::")
                    .map(|(alias, _)| alias.to_owned())
            })
            .map(|alias| (alias, ()))
            .collect::<BTreeMap<_, _>>();
        ModuleUnit {
            source_name: name.to_owned(),
            program: crate::resolved::resolve_with_imports(program, &file, &imports)
                .expect("resolve linker fixture"),
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
                source: "seiyaku App { view fn run() -> int { return derived::value(); } }"
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
                            "module Derived { fn value() -> int { return base::value() + 1; } }"
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
                    "seiyaku App { view fn run() -> int { return arith::add(right: 3, left: 2); } }",
                ),
                package(
                    vec![source(
                        "math.ko",
                        "module Math { fn add(int left, int right) -> int { return left + right; } }",
                    )],
                    &["add"],
                ),
            ))
            .expect("link typed HIR");

        assert_eq!(linked.unit.name, "App");
        assert_eq!(linked.items.len(), 2);
        let TypedItem::Function(root) = &linked.items[0];
        let TypedStatement::Return(Some(TypedExpr {
            expr:
                ExprKind::NamedCall {
                    name,
                    evaluation_order,
                    ..
                },
            ..
        })) = &root.body.statements[0]
        else {
            panic!("expected linked named call")
        };
        assert_eq!(evaluation_order, &[1, 0]);
        assert!(name.starts_with(LINKED_SYMBOL_PREFIX));
        let TypedItem::Function(module) = &linked.items[1];
        assert_eq!(name, &module.name);
    }

    #[test]
    fn imported_repeated_parameter_types_remain_named_only() {
        let dependency = || {
            package(
                vec![source(
                    "math.ko",
                    "module Math { fn choose(int left, int right) -> int { return left; } }",
                )],
                &["choose"],
            )
        };

        let positional = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> int { return arith::choose(1, 2); } }",
                ),
                dependency(),
            ))
            .expect_err("an imported repeated-type signature must remain named-only");
        assert!(matches!(
            positional,
            LinkError::Semantic {
                ref source,
                diagnostic_code: "E_NAMED_ARGUMENTS_REQUIRED",
                ..
            } if source == "app.ko"
        ));

        let linked = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> int { return arith::choose(right: 2, left: 1); } }",
                ),
                dependency(),
            ))
            .expect("the reordered named imported call must link");
        let TypedItem::Function(root) = &linked.items[0];
        let TypedStatement::Return(Some(TypedExpr {
            expr:
                ExprKind::NamedCall {
                    name,
                    evaluation_order,
                    ..
                },
            ..
        })) = &root.body.statements[0]
        else {
            panic!("expected linked named call")
        };
        assert_eq!(evaluation_order, &[1, 0]);
        assert!(name.starts_with(LINKED_SYMBOL_PREFIX));
    }

    #[test]
    fn rejects_unexported_and_unknown_calls() {
        let dependency = package(
            vec![source(
                "math.ko",
                "module Math { fn hidden() -> int { return 1; } }",
            )],
            &[],
        );
        let unexported = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> int { return arith::hidden(); } }",
                ),
                dependency,
            ))
            .expect_err("unexported function must fail");
        assert!(matches!(unexported, LinkError::UnexportedSymbol { .. }));

        let unknown = TypedLinker::default()
            .link(request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run() -> int { return other::add(); } }",
                ),
                package(
                    vec![source(
                        "math.ko",
                        "module Math { fn add() -> int { return 1; } }",
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
                "module Hash { fn sha256(bytes value) -> bytes { return value; } }",
            )],
            &["sha256"],
        );
        for alias in ["crypto", "quantity"] {
            let mut request = request(
                source(
                    "app.ko",
                    "seiyaku App { view fn run(bytes value) -> bytes { return crypto::sha256(value); } }",
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
                    "seiyaku App { view fn run() -> int { return arith::value(); } }",
                ),
                package(
                    vec![
                        source("a.ko", "module A { fn value() -> int { return 1; } }"),
                        source("b.ko", "module B { fn value() -> int { return 2; } }"),
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
                    "seiyaku App { view fn run() -> int { return arith::left() + arith::right(); } }",
                ),
                package(
                    vec![
                        source(
                            "left.ko",
                            "module Left { fn helper() -> int { return 1; } fn left() -> int { return helper(); } }",
                        ),
                        source(
                            "right.ko",
                            "module Right { fn helper() -> int { return 2; } fn right() -> int { return helper(); } }",
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
        let package_identity = "std/math@1.0.0".to_owned();
        let error = ModuleBuildGraph::default()
            .link(
                SourceLinkRequest {
                    root: SourceModuleUnit {
                        source_name: "app.ko".to_owned(),
                        source:
                            "seiyaku App { view fn run() -> int { return math::ok(); } }"
                                .to_owned(),
                    },
                    imports: vec![ImportBinding {
                        alias: "math".to_owned(),
                        package: package_identity.clone(),
                    }],
                    packages: vec![SourcePackageUnit {
                        identity: package_identity,
                        modules: vec![SourceModuleUnit {
                            source_name: "reserved.ko".to_owned(),
                            source: "module Reserved { fn __kotodama_link_private() -> int { return 1; } fn ok() -> int { return 1; } }".to_owned(),
                        }],
                        exports: BTreeSet::from(["ok".to_owned()]),
                        imports: Vec::new(),
                    }],
                },
                LinkerOptions::default(),
            )
            .expect_err("reserved linker prefix must fail");
        assert!(matches!(error, SourceGraphError::Resolve { .. }));
    }

    #[test]
    fn source_graph_parses_equal_contents_once_and_reuses_cache() {
        let graph = ModuleBuildGraph::default();
        let modules = vec![
            SourceModuleUnit {
                source_name: "first.ko".to_owned(),
                source: "module Shared { fn value() -> int { return 1; } }".to_owned(),
            },
            SourceModuleUnit {
                source_name: "second.ko".to_owned(),
                source: "module Shared { fn value() -> int { return 1; } }".to_owned(),
            },
        ];
        let source_ids = stable_source_ids(
            &modules
                .iter()
                .map(|module| module.source_name.clone())
                .collect::<Vec<_>>(),
        );
        let first = graph
            .parse_sources_with_ids(&modules, &source_ids)
            .expect("parse shared contents");
        let mut first_plain = first[0].program.clone();
        let mut second_plain = first[1].program.clone();
        crate::ast::strip_program_provenance(&mut first_plain);
        crate::ast::strip_program_provenance(&mut second_plain);
        assert_eq!(
            first_plain, second_plain,
            "equal contents retain the same source-independent AST structure"
        );
        assert_ne!(
            first[0].facts.source_map.source(),
            first[1].facts.source_map.source(),
            "equal contents in distinct logical files retain distinct SourceIds"
        );
        assert_eq!(
            first[0]
                .facts
                .source_map
                .nodes()
                .map(|node| (node.id, node.kind, node.range))
                .collect::<Vec<_>>(),
            first[1]
                .facts
                .source_map
                .nodes()
                .map(|node| (node.id, node.kind, node.range))
                .collect::<Vec<_>>(),
            "content-identical parses retain the same structural NodeIds"
        );
        for parsed in &first {
            let source_id = parsed.facts.source_map.source();
            let Item::Function(function) = &parsed.program.items[0] else {
                panic!("cached module function")
            };
            let statement = &function.body.statements[0];
            assert_eq!(
                statement.source().map(|range| range.source),
                Some(source_id)
            );
            let Statement::Return(Some(value)) = statement.kind() else {
                panic!("cached module return")
            };
            assert_eq!(value.source().map(|range| range.source), Some(source_id));
            for node in [statement.source_node(), value.source_node()]
                .into_iter()
                .flatten()
            {
                assert!(
                    parsed
                        .facts
                        .source_map
                        .source_range(node)
                        .is_some_and(|range| range.source == source_id
                            && Some(range) == parsed.facts.source_map.source_range(node))
                );
            }
        }
        assert_eq!(
            graph
                .parse_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        let reused = graph
            .parse_sources_with_ids(&modules, &source_ids)
            .expect("reuse parsed source cache");
        for (original, cached) in first.iter().zip(&reused) {
            assert_eq!(
                original.facts.source_map.source(),
                cached.facts.source_map.source()
            );
            assert_eq!(
                original
                    .facts
                    .source_map
                    .nodes()
                    .map(|node| (node.id, node.kind, node.range))
                    .collect::<Vec<_>>(),
                cached
                    .facts
                    .source_map
                    .nodes()
                    .map(|node| (node.id, node.kind, node.range))
                    .collect::<Vec<_>>()
            );
        }
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
                transitive_source_request("module Base { fn value() -> int { return 1; } }"),
                LinkerOptions::default(),
            )
            .expect("link initial transitive graph");
        assert_eq!(graph.parse_attempt_count(), 3);
        assert_eq!(graph.link_attempt_count(), 1);

        let implementation_changed = graph
            .link(
                transitive_source_request("module Base { fn value() -> int { return 2; } }"),
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
                    "module Base { fn value(int input) -> int { return input; } }",
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
    fn linked_typed_hir_retains_path_and_order_stable_distinct_source_ids() {
        let request = transitive_source_request("module Base { fn value() -> int { return 1; } }");
        let mut reordered = request.clone();
        reordered.packages.reverse();
        reordered.root.source_name = r".\app.ko".to_owned();
        for package in &mut reordered.packages {
            for module in &mut package.modules {
                module.source_name = format!(r".\nested\..\{}", module.source_name);
            }
        }

        let left = ModuleBuildGraph::default()
            .link(request, LinkerOptions::default())
            .expect("link canonical package order");
        let right = ModuleBuildGraph::default()
            .link(reordered, LinkerOptions::default())
            .expect("link reversed package order");

        assert_eq!(left.program.source_files, right.program.source_files);
        assert_eq!(left.program.source_files.len(), 3);
        assert_eq!(
            left.program
                .source_files
                .values()
                .map(|source| source.name().to_owned())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "app.ko".to_owned(),
                "base.ko".to_owned(),
                "derived.ko".to_owned(),
            ])
        );
        for item in &left.program.items {
            let TypedItem::Function(function) = item;
            let source = function
                .source
                .expect("linked function retains its declaration source");
            let name_source = function
                .name_source
                .expect("linked function retains its name source");
            assert!(left.program.source_files.contains_key(&source.source));
            assert!(!source.range.is_empty());
            assert!(!name_source.range.is_empty());
        }
    }

    #[test]
    fn source_cache_defends_against_adversarial_digest_collision() {
        let graph = ModuleBuildGraph::default();
        let modules = vec![
            SourceModuleUnit {
                source_name: "left.ko".to_owned(),
                source: "module Left { fn value() -> int { return 1; } }".to_owned(),
            },
            SourceModuleUnit {
                source_name: "right.ko".to_owned(),
                source: "module Right { fn value() -> int { return 2; } }".to_owned(),
            },
        ];
        let source_ids = stable_source_ids(
            &modules
                .iter()
                .map(|module| module.source_name.clone())
                .collect::<Vec<_>>(),
        );
        let parsed = graph
            .parse_sources_with_digest(&modules, &source_ids, |_| "forced-collision".to_owned())
            .expect("exact source comparison must disambiguate a digest collision");
        assert_ne!(parsed[0].program.unit.name, parsed[1].program.unit.name);
        assert_eq!(
            graph
                .parse_attempts
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }

    #[test]
    fn parsed_source_cache_is_bounded_and_uses_lru_eviction() {
        let program = spanned("module Cached { fn value() -> int { return 1; } }");
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
        let program = spanned("module Cached { fn value() -> int { return 1; } }");
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
            source: "seiyaku App { view fn run() -> int { return arith::value(); } }".to_owned(),
        };
        let package = SourcePackageUnit {
            identity: "std/math@1.0.0".to_owned(),
            modules: vec![SourceModuleUnit {
                source_name: "math.ko".to_owned(),
                source: "module Math { fn value() -> int { return 1; } }".to_owned(),
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
            ModuleBuildGraph::fingerprint(&left).expect("left graph fingerprint"),
            ModuleBuildGraph::fingerprint(&changed).expect("changed graph fingerprint"),
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
            ModuleBuildGraph::fingerprint(&two_imports).expect("ordered graph fingerprint"),
            ModuleBuildGraph::fingerprint(&reordered).expect("reordered graph fingerprint"),
            "incidental lockfile ordering must not invalidate the graph"
        );
    }

    fn publish_package(modules: Vec<SourceModuleUnit>, exports: &[&str]) -> SourcePackageUnit {
        SourcePackageUnit {
            identity: "local/quotes@1.0.0".to_owned(),
            modules,
            exports: exports.iter().map(|name| (*name).to_owned()).collect(),
            imports: Vec::new(),
        }
    }

    fn source_module(name: &str, source: &str) -> SourceModuleUnit {
        SourceModuleUnit {
            source_name: name.to_owned(),
            source: source.to_owned(),
        }
    }

    fn invalid_logical_source_paths() -> Vec<(String, InvalidSourcePathReason)> {
        vec![
            (String::new(), InvalidSourcePathReason::Empty),
            (".".to_owned(), InvalidSourcePathReason::Empty),
            ("././".to_owned(), InvalidSourcePathReason::Empty),
            ("dir/..".to_owned(), InvalidSourcePathReason::Empty),
            ("...".to_owned(), InvalidSourcePathReason::DotOnlyComponent),
            (
                "/absolute/app.ko".to_owned(),
                InvalidSourcePathReason::Absolute,
            ),
            (r"\rooted.ko".to_owned(), InvalidSourcePathReason::Absolute),
            (
                r"\\server\share\app.ko".to_owned(),
                InvalidSourcePathReason::Absolute,
            ),
            (
                r"C:\source\app.ko".to_owned(),
                InvalidSourcePathReason::WindowsDrive,
            ),
            (
                "c:drive-relative.ko".to_owned(),
                InvalidSourcePathReason::WindowsDrive,
            ),
            (
                "../escape.ko".to_owned(),
                InvalidSourcePathReason::EscapesRoot,
            ),
            (
                "src/../../escape.ko".to_owned(),
                InvalidSourcePathReason::EscapesRoot,
            ),
            (
                "src/\0evil.ko".to_owned(),
                InvalidSourcePathReason::NonPortableCharacter {
                    byte_offset: 4,
                    character: '\0',
                },
            ),
            (
                "src/\nevil.ko".to_owned(),
                InvalidSourcePathReason::NonPortableCharacter {
                    byte_offset: 4,
                    character: '\n',
                },
            ),
            (
                "src/name:stream.ko".to_owned(),
                InvalidSourcePathReason::NonPortableCharacter {
                    byte_offset: 8,
                    character: ':',
                },
            ),
            (
                "a".repeat(MAX_LOGICAL_SOURCE_PATH_BYTES + 1),
                InvalidSourcePathReason::TooLong {
                    bytes: MAX_LOGICAL_SOURCE_PATH_BYTES + 1,
                    max_bytes: MAX_LOGICAL_SOURCE_PATH_BYTES,
                },
            ),
        ]
    }

    #[test]
    fn package_graph_validates_unique_typed_export_and_locked_call() {
        let dependency_identity = "std/math@1.0.0".to_owned();
        let mut local = publish_package(
            vec![source_module(
                "src/lib.ko",
                "module Quotes { fn quote(int value) -> int { return arith::add(left: value, right: 1); } }",
            )],
            &["quote"],
        );
        local.imports.push(ImportBinding {
            alias: "arith".to_owned(),
            package: dependency_identity.clone(),
        });
        let request = SourcePackageGraphRequest {
            package: local,
            dependencies: vec![SourcePackageUnit {
                identity: dependency_identity,
                modules: vec![source_module(
                    "src/lib.ko",
                    "module Math { fn add(int left, int right) -> int { return left + right; } }",
                )],
                exports: BTreeSet::from(["add".to_owned()]),
                imports: Vec::new(),
            }],
        };

        let validated = ModuleBuildGraph::default()
            .validate_package(request, LinkerOptions::default())
            .expect("typed package graph");
        assert_eq!(validated.exports, BTreeSet::from(["quote".to_owned()]));
    }

    #[test]
    fn package_graph_rejects_missing_and_ambiguous_exports() {
        let graph = ModuleBuildGraph::default();
        let missing = graph
            .validate_package(
                SourcePackageGraphRequest {
                    package: publish_package(
                        vec![source_module(
                            "quotes.ko",
                            "module Quotes { fn quote() -> int { return 1; } }",
                        )],
                        &["missing"],
                    ),
                    dependencies: Vec::new(),
                },
                LinkerOptions::default(),
            )
            .expect_err("missing export must fail");
        assert!(
            matches!(
                missing,
                SourceGraphError::Link(LinkError::MissingExport { ref symbol, .. })
                    if symbol == "missing"
            ),
            "{missing:?}"
        );

        let ambiguous = graph
            .validate_package(
                SourcePackageGraphRequest {
                    package: publish_package(
                        vec![
                            source_module("a.ko", "module A { fn quote() -> int { return 1; } }"),
                            source_module("b.ko", "module B { fn quote() -> int { return 2; } }"),
                        ],
                        &["quote"],
                    ),
                    dependencies: Vec::new(),
                },
                LinkerOptions::default(),
            )
            .expect_err("ambiguous export must fail");
        assert!(
            matches!(
                ambiguous,
                SourceGraphError::Link(LinkError::AmbiguousExport { ref symbol, .. })
                    if symbol == "quote"
            ),
            "{ambiguous:?}"
        );
    }

    #[test]
    fn package_graph_rejects_invalid_types_and_bodies() {
        for source in [
            "module Quotes { fn quote(MissingType value) -> int { return 1; } }",
            "module Quotes { fn quote() -> int { return true; } }",
        ] {
            let error = ModuleBuildGraph::default()
                .validate_package(
                    SourcePackageGraphRequest {
                        package: publish_package(
                            vec![source_module("invalid.ko", source)],
                            &["quote"],
                        ),
                        dependencies: Vec::new(),
                    },
                    LinkerOptions::default(),
                )
                .expect_err("invalid typed module must fail");
            assert!(matches!(
                error,
                SourceGraphError::Resolve { .. }
                    | SourceGraphError::Link(LinkError::Semantic { .. })
            ));
        }
    }

    #[test]
    fn package_graph_rejects_duplicate_symbols_and_module_names() {
        let duplicate_symbol = ModuleBuildGraph::default()
            .validate_package(
                SourcePackageGraphRequest {
                    package: publish_package(
                        vec![source_module(
                            "duplicate.ko",
                            "module Quotes { fn quote() -> int { return 1; } fn quote() -> int { return 2; } }",
                        )],
                        &["quote"],
                    ),
                    dependencies: Vec::new(),
                },
                LinkerOptions::default(),
            )
            .expect_err("duplicate symbol must fail closed");
        assert!(matches!(
            duplicate_symbol,
            SourceGraphError::Resolve { .. }
                | SourceGraphError::Link(LinkError::DuplicateSymbol { .. })
        ));

        let duplicate_module = ModuleBuildGraph::default()
            .validate_package(
                SourcePackageGraphRequest {
                    package: publish_package(
                        vec![
                            source_module(
                                "a.ko",
                                "module Quotes { fn quote() -> int { return 1; } }",
                            ),
                            source_module(
                                "b.ko",
                                "module Quotes { fn other() -> int { return 2; } }",
                            ),
                        ],
                        &["quote"],
                    ),
                    dependencies: Vec::new(),
                },
                LinkerOptions::default(),
            )
            .expect_err("duplicate module name must fail");
        assert!(
            matches!(
                duplicate_module,
                SourceGraphError::Link(LinkError::DuplicateModule { .. })
            ),
            "{duplicate_module:?}"
        );
    }

    #[test]
    fn package_graph_rejects_seiyaku_and_test_only_exports() {
        let seiyaku = ModuleBuildGraph::default()
            .validate_package(
                SourcePackageGraphRequest {
                    package: publish_package(
                        vec![source_module(
                            "app.ko",
                            "seiyaku Quotes { view fn quote() -> int { return 1; } }",
                        )],
                        &["quote"],
                    ),
                    dependencies: Vec::new(),
                },
                LinkerOptions::default(),
            )
            .expect_err("seiyaku cannot satisfy package export");
        assert!(
            matches!(
                seiyaku,
                SourceGraphError::Link(LinkError::DependencyMustBeModule { .. })
            ),
            "{seiyaku:?}"
        );

        let test_only = ModuleBuildGraph::default()
            .validate_package(
                SourcePackageGraphRequest {
                    package: publish_package(
                        vec![source_module(
                            "test.ko",
                            "module Quotes { #[test] fn quote() -> int { return 1; } }",
                        )],
                        &["quote"],
                    ),
                    dependencies: Vec::new(),
                },
                LinkerOptions::default(),
            )
            .expect_err("test-only function cannot satisfy production export");
        assert!(matches!(
            test_only,
            SourceGraphError::Link(LinkError::Semantic {
                diagnostic_code: "E_TEST_ONLY_PRODUCTION",
                ..
            })
        ));
    }

    #[test]
    fn package_graph_rejects_dependency_hidden_call() {
        let dependency_identity = "std/math@1.0.0".to_owned();
        let mut local = publish_package(
            vec![source_module(
                "quotes.ko",
                "module Quotes { fn quote() -> int { return arith::hidden(); } }",
            )],
            &["quote"],
        );
        local.imports.push(ImportBinding {
            alias: "arith".to_owned(),
            package: dependency_identity.clone(),
        });
        let graph = ModuleBuildGraph::default();
        let error = graph
            .validate_package(
                SourcePackageGraphRequest {
                    package: local,
                    dependencies: vec![SourcePackageUnit {
                        identity: dependency_identity,
                        modules: vec![source_module(
                            "math.ko",
                            "module Math { fn hidden() -> int { return 1; } fn visible() -> int { return 2; } }",
                        )],
                        exports: BTreeSet::from(["visible".to_owned()]),
                        imports: Vec::new(),
                    }],
                },
                LinkerOptions::default(),
            )
            .expect_err("hidden dependency call must fail");
        assert!(
            matches!(
                error,
                SourceGraphError::Link(LinkError::UnexportedSymbol { ref symbol, .. })
                    if symbol == "hidden"
            ),
            "{error:?}"
        );
    }

    #[test]
    fn package_graph_rejects_import_cycles_without_call_cycles() {
        let local_identity = "local/quotes@1.0.0".to_owned();
        let dependency_identity = "std/math@1.0.0".to_owned();
        let mut local = publish_package(
            vec![source_module(
                "quotes.ko",
                "module Quotes { fn quote() -> int { return 1; } }",
            )],
            &["quote"],
        );
        local.imports.push(ImportBinding {
            alias: "arith".to_owned(),
            package: dependency_identity.clone(),
        });
        let graph = ModuleBuildGraph::default();
        let error = graph
            .validate_package(
                SourcePackageGraphRequest {
                    package: local,
                    dependencies: vec![SourcePackageUnit {
                        identity: dependency_identity,
                        modules: vec![source_module(
                            "math.ko",
                            "module Math { fn value() -> int { return 2; } }",
                        )],
                        exports: BTreeSet::from(["value".to_owned()]),
                        imports: vec![ImportBinding {
                            alias: "quotes".to_owned(),
                            package: local_identity,
                        }],
                    }],
                },
                LinkerOptions::default(),
            )
            .expect_err("locked package cycle must fail without relying on function calls");
        assert!(
            matches!(
                error,
                SourceGraphError::Link(LinkError::PackageImportCycle { ref cycle })
                    if cycle.first() == cycle.last() && cycle.len() == 3
            ),
            "{error:?}"
        );
        assert_eq!(graph.parse_attempt_count(), 0);
    }

    #[test]
    fn package_graph_rejects_duplicate_normalized_logical_sources_before_parsing() {
        let request = SourcePackageGraphRequest {
            package: publish_package(
                vec![
                    source_module("src/../lib.ko", "not parsed"),
                    source_module("lib.ko", "also not parsed"),
                ],
                &[],
            ),
            dependencies: Vec::new(),
        };
        let graph = ModuleBuildGraph::default();
        let error = graph
            .validate_package(request, LinkerOptions::default())
            .expect_err("duplicate logical source key must fail before parsing");
        assert!(matches!(
            error,
            SourceGraphError::DuplicateSource { ref source, .. } if source == "lib.ko"
        ));
        assert_eq!(graph.parse_attempt_count(), 0);
        assert_eq!(graph.link_attempt_count(), 0);
    }

    #[test]
    fn package_fingerprint_normalizes_portable_logical_source_paths() {
        let left = SourcePackageGraphRequest {
            package: publish_package(
                vec![source_module(
                    "src/lib.ko",
                    "module Quotes { fn quote() -> int { return 1; } }",
                )],
                &["quote"],
            ),
            dependencies: Vec::new(),
        };
        let mut right = left.clone();
        right.package.modules[0].source_name = ".\\src\\lib.ko".to_owned();
        assert_eq!(
            ModuleBuildGraph::package_fingerprint(&left).expect("left fingerprint"),
            ModuleBuildGraph::package_fingerprint(&right).expect("right fingerprint")
        );
    }

    #[test]
    fn source_graph_fingerprint_normalizes_root_and_package_paths() {
        let left = SourceLinkRequest {
            root: source_module("src/app.ko", "not parsed"),
            imports: Vec::new(),
            packages: vec![SourcePackageUnit {
                identity: "std/arith@1.0.0".to_owned(),
                modules: vec![source_module("src/lib.ko", "also not parsed")],
                exports: BTreeSet::new(),
                imports: Vec::new(),
            }],
        };
        let mut right = left.clone();
        right.root.source_name = r".\src\\app.ko".to_owned();
        right.packages[0].modules[0].source_name = r"src\nested\..\lib.ko".to_owned();

        assert_eq!(
            ModuleBuildGraph::fingerprint(&left).expect("canonical fingerprint"),
            ModuleBuildGraph::fingerprint(&right).expect("portable-alias fingerprint")
        );
    }

    #[test]
    fn invalid_root_paths_fail_closed_before_any_parse_or_link_attempt() {
        for (source_name, expected_reason) in invalid_logical_source_paths() {
            let graph = ModuleBuildGraph::default();
            let error = graph
                .link(
                    SourceLinkRequest {
                        root: source_module(&source_name, "not parsed"),
                        imports: Vec::new(),
                        packages: Vec::new(),
                    },
                    LinkerOptions::default(),
                )
                .expect_err("invalid root logical paths must fail closed");
            assert_eq!(error.diagnostic_code(), "E_INVALID_SOURCE_PATH");
            assert!(
                matches!(
                    &error,
                    SourceGraphError::InvalidSourcePath {
                        scope,
                        source,
                        reason,
                    } if scope == "root" && source == &source_name && reason == &expected_reason
                ),
                "unexpected rejection for {source_name:?}: {error:?}"
            );
            assert_eq!(graph.parse_attempt_count(), 0);
            assert_eq!(graph.link_attempt_count(), 0);
            let rendered = error.to_string();
            assert!(!rendered.contains('\0'));
            assert!(!rendered.contains('\n'));
        }
    }

    #[test]
    fn invalid_package_paths_fail_closed_before_any_parse_or_link_attempt() {
        for (source_name, expected_reason) in invalid_logical_source_paths() {
            let graph = ModuleBuildGraph::default();
            let error = graph
                .validate_package(
                    SourcePackageGraphRequest {
                        package: publish_package(
                            vec![source_module(&source_name, "not parsed")],
                            &[],
                        ),
                        dependencies: Vec::new(),
                    },
                    LinkerOptions::default(),
                )
                .expect_err("invalid package logical paths must fail closed");
            assert!(matches!(
                &error,
                SourceGraphError::InvalidSourcePath {
                    scope,
                    source,
                    reason,
                } if scope == "local/quotes@1.0.0"
                    && source == &source_name
                    && reason == &expected_reason
            ));
            assert_eq!(graph.parse_attempt_count(), 0);
            assert_eq!(graph.link_attempt_count(), 0);
        }
    }

    #[test]
    fn root_path_is_canonical_in_parse_diagnostics() {
        let graph = ModuleBuildGraph::default();
        let error = graph
            .link(
                SourceLinkRequest {
                    root: source_module(r".\src\nested\..\app.ko", "@"),
                    imports: Vec::new(),
                    packages: Vec::new(),
                },
                LinkerOptions::default(),
            )
            .expect_err("invalid source text must reach the parser");
        assert!(matches!(
            error,
            SourceGraphError::Parse { ref source, .. } if source == "src/app.ko"
        ));
        assert_eq!(graph.parse_attempt_count(), 1);
        assert_eq!(graph.link_attempt_count(), 1);
    }

    #[test]
    fn canonical_module_order_makes_parallel_parse_failure_deterministic() {
        let request = SourcePackageGraphRequest {
            package: publish_package(
                vec![source_module("z.ko", "@"), source_module("a.ko", "$")],
                &[],
            ),
            dependencies: Vec::new(),
        };
        let mut reordered = request.clone();
        reordered.package.modules.reverse();

        let first = ModuleBuildGraph::default()
            .validate_package(request, LinkerOptions::default())
            .expect_err("first malformed package must fail");
        let second = ModuleBuildGraph::default()
            .validate_package(reordered, LinkerOptions::default())
            .expect_err("reordered malformed package must fail identically");
        assert_eq!(first, second);
        assert!(matches!(
            first,
            SourceGraphError::Parse { ref source, .. } if source == "a.ko"
        ));
    }
}
