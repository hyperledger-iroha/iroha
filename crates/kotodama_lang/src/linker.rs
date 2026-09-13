//! Typed-HIR linker for Kotodama V1 modules.
//!
//! Source units are parsed and type checked independently. The linker resolves only explicit
//! `alias::symbol` imports backed by a locked export table, rewrites final symbol identities in
//! typed HIR, and then reruns whole-program recursion and effect analysis before handing the result
//! to the canonical compiler session.
use crate::{
    ast::{FunctionKind, Item, Program, SourceUnitKind},
    builtins::{Builtin, BuiltinSurface},
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticLabel, DiagnosticPhase, SourceSpan,
        phase_for_semantic_failure,
    },
    semantic::{
        self, ExprKind, FunctionSignature, Type, TypedBlock, TypedExpr, TypedItem, TypedProgram,
        TypedStatement,
    },
    source::{FrontendBudget, SourceFile, SourceId},
    spanned_ast::SpannedProgram,
};
use iroha_crypto::Hash;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};
mod editor;
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
/// Unlike [`SourceLinkRequest`], this graph has no deployable seiyaku root. The package being
/// published and every locked dependency must consist only of production `module` units. All
/// declared exports, imported calls, types, and transitive effects are checked together after
/// independent module analysis.
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
    /// Domain-separated digest of the exact typed exports exposed by the local package.
    pub interface_fingerprint: Hash,
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
        formatter.write_str(&self.clone().into_diagnostics().render_human())
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
    /// Convert parsing, resolution, metadata, and typed-link failures into one
    /// canonical bundle suitable for human, JSON, or SARIF rendering.
    pub fn into_diagnostics(self) -> DiagnosticBundle {
        match self {
            Self::Budget {
                sources,
                source_bytes,
                max_sources,
                max_source_bytes,
            } => DiagnosticBundle::single(Diagnostic::error(
                "E_PACKAGE_BUDGET",
                DiagnosticPhase::Parse,
                format!(
                    "Kotodama module graph contains {sources} sources/{source_bytes} bytes; V1 permits at most {max_sources} sources/{max_source_bytes} bytes"
                ),
                None,
            )),
            Self::Parse { diagnostics, .. } | Self::Resolve { diagnostics, .. } => diagnostics,
            Self::InvalidSourcePath {
                scope,
                source,
                reason,
            } => DiagnosticBundle::single(Diagnostic::error(
                "E_INVALID_SOURCE_PATH",
                DiagnosticPhase::Resolve,
                format!(
                    "Kotodama source path `{}` in `{scope}` is invalid: {reason}",
                    source.escape_debug()
                ),
                None,
            )),
            Self::DuplicateSource { scope, source } => DiagnosticBundle::single(Diagnostic::error(
                "E_DUPLICATE_SOURCE",
                DiagnosticPhase::Resolve,
                format!("Kotodama package `{scope}` contains duplicate logical source `{source}`"),
                None,
            )),
            Self::Link(error) => error.into_diagnostics(),
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
/// A call parses independent changed modules in parallel. Equal source contents are parsed once,
/// while a bounded LRU retains exact source text to defend against digest collisions without
/// permitting unbounded service memory. The final linker still receives a deterministic request and
/// performs whole-graph type/effect analysis exactly once.
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
    /// This preflight performs the same aggregate resource-budget check as [`Self::link`] but does
    /// not parse, resolve, or type-check any source. A build driver can therefore authenticate a
    /// previously published result before doing compiler work on an unchanged graph.
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
    /// Parse editor/project sources through the same content-addressed cache
    /// later consumed by typed graph linking.
    pub(crate) fn parse_project_sources(
        &self,
        sources: &[SourceModuleUnit],
    ) -> Result<Vec<SpannedProgram>, SourceGraphError> {
        let keys = sources
            .iter()
            .map(|source| format!("project\0{}", source.source_name))
            .collect::<Vec<_>>();
        let source_ids = stable_source_ids(&keys);
        self.parse_sources_with_ids(sources, &source_ids)
    }
    /// Parse, resolve, and type-check one complete locked source graph.
    pub fn link(
        &self,
        request: SourceLinkRequest,
        options: LinkerOptions,
    ) -> Result<LinkedSourceGraph, SourceGraphError> {
        crate::session::run_with_compiler_stack(move || {
            self.link_sources_inner(request, &[], options)
        })
        .map_err(|_| SourceGraphError::Parse {
            source: "<project>".to_owned(),
            diagnostics: crate::session::compiler_worker_unavailable_diagnostic(Some("<project>")),
        })?
    }
    fn link_sources_inner(
        &self,
        mut request: SourceLinkRequest,
        test_sources: &[SourceModuleUnit],
        options: LinkerOptions,
    ) -> Result<LinkedSourceGraph, SourceGraphError> {
        let names = validate_source_link_request(&request)?;
        let fingerprint = source_graph_fingerprint(&request, &names);
        canonicalize_source_link_request(&mut request, names);
        let source_count = 1
            + test_sources.len()
            + request
                .packages
                .iter()
                .map(|package| package.modules.len())
                .sum::<usize>();
        let source_bytes = test_sources
            .iter()
            .chain(request.packages.iter().flat_map(|package| &package.modules))
            .fold(request.root.source.len(), |total, source| {
                total.saturating_add(source.source.len())
            });
        if source_count > MAX_MODULE_GRAPH_SOURCES || source_bytes > MAX_MODULE_GRAPH_SOURCE_BYTES {
            return Err(SourceGraphError::Budget {
                sources: source_count,
                source_bytes,
                max_sources: MAX_MODULE_GRAPH_SOURCES,
                max_source_bytes: MAX_MODULE_GRAPH_SOURCE_BYTES,
            });
        }
        let mut test_sources = test_sources.to_vec();
        let mut local_names = BTreeSet::from([request.root.source_name.clone()]);
        for test in &mut test_sources {
            test.source_name = canonical_logical_source_name("test", &test.source_name)?;
            if !local_names.insert(test.source_name.clone()) {
                return Err(SourceGraphError::DuplicateSource {
                    scope: "test".to_owned(),
                    source: test.source_name.clone(),
                });
            }
        }
        test_sources.sort_by(|left, right| left.source_name.cmp(&right.source_name));
        let fingerprint = if test_sources.is_empty() {
            fingerprint
        } else {
            let mut transcript = b"kotodama-test-source-graph-v1\0".to_vec();
            transcript.extend_from_slice(fingerprint.as_ref());
            for source in &test_sources {
                for value in [source.source_name.as_bytes(), source.source.as_bytes()] {
                    transcript.extend_from_slice(&(value.len() as u64).to_le_bytes());
                    transcript.extend_from_slice(value);
                }
            }
            Hash::new(transcript)
        };
        #[cfg(test)]
        self.link_attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut sources = Vec::new();
        sources.push(request.root.clone());
        let mut package_identities = vec![None];
        for package in &request.packages {
            sources.extend(package.modules.iter().cloned());
            package_identities.extend(std::iter::repeat_n(
                Some(package.identity.clone()),
                package.modules.len(),
            ));
        }
        let ordinary_source_count = sources.len();
        sources.extend(test_sources.iter().cloned());
        package_identities.extend(std::iter::repeat_n(None, test_sources.len()));
        let source_keys = std::iter::once(format!("root\0{}", request.root.source_name))
            .chain(request.packages.iter().flat_map(|package| {
                package
                    .modules
                    .iter()
                    .map(|module| format!("package\0{}\0{}", package.identity, module.source_name))
            }))
            .chain(
                test_sources
                    .iter()
                    .map(|source| format!("test\0{}", source.source_name)),
            )
            .collect::<Vec<_>>();
        let source_ids = stable_source_ids(&source_keys);
        let mut parsed =
            self.parse_sources_with_ids_scoped(&sources, &source_ids, &package_identities)?;
        let parsed_tests = parsed
            .split_off(ordinary_source_count)
            .into_iter()
            .zip(&sources[ordinary_source_count..])
            .zip(&source_ids[ordinary_source_count..])
            .map(|((program, source), id)| {
                (
                    program,
                    SourceFile::new(*id, source.source_name.as_str(), source.source.as_str()),
                )
            })
            .collect::<Vec<_>>();
        let mut programs = Vec::with_capacity(parsed.len());
        let mut resolve_diagnostics = Vec::new();
        for (index, ((program, source), source_id)) in parsed
            .into_iter()
            .zip(&sources)
            .zip(source_ids.iter().copied())
            .enumerate()
        {
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
            let file = package_identities[index].as_ref().map_or_else(
                || SourceFile::new(source_id, source.source_name.as_str(), &source.source),
                |package| {
                    SourceFile::new_in_package(
                        source_id,
                        package.as_str(),
                        source.source_name.as_str(),
                        &source.source,
                    )
                },
            );
            match crate::resolved::resolve_with_imports(program, &file, &imports) {
                Ok(program) => programs.push(Some(program)),
                Err(diagnostics) => {
                    resolve_diagnostics.extend(diagnostics.diagnostics);
                    programs.push(None);
                }
            }
        }
        if !resolve_diagnostics.is_empty() {
            return Err(SourceGraphError::Resolve {
                source: "<project>".to_owned(),
                diagnostics: DiagnosticBundle::new(resolve_diagnostics),
            });
        }
        let programs = programs
            .into_iter()
            .map(|program| program.expect("resolution failures returned before typed linking"))
            .collect::<Vec<_>>();
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
        let program = TypedLinker::new(options).link_with_tests(
            LinkRequest {
                root,
                imports: request.imports,
                packages,
            },
            parsed_tests,
        )?;
        Ok(LinkedSourceGraph {
            program,
            fingerprint,
        })
    }
    /// Parse, resolve, type/effect-check, and validate one publishable package.
    ///
    /// No synthetic deployable root is created. Every source is parsed once through this graph's
    /// content-addressed parser, every package is resolved against its explicit locked imports, and
    /// whole-graph call/effect checks run over the resulting typed HIR.
    pub fn validate_package(
        &self,
        request: SourcePackageGraphRequest,
        options: LinkerOptions,
    ) -> Result<ValidatedSourcePackageGraph, SourceGraphError> {
        crate::session::run_with_compiler_stack(move || {
            self.validate_package_inner(request, options)
        })
        .map_err(|_| SourceGraphError::Parse {
            source: "<project>".to_owned(),
            diagnostics: crate::session::compiler_worker_unavailable_diagnostic(Some("<project>")),
        })?
    }
    fn validate_package_inner(
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
        let package_identities = packages
            .iter()
            .flat_map(|package| {
                std::iter::repeat_n(Some(package.identity.clone()), package.modules.len())
            })
            .collect::<Vec<_>>();
        let mut parsed = self
            .parse_sources_with_ids_scoped(&sources, &source_ids, &package_identities)?
            .into_iter();
        let mut source_ids = source_ids.into_iter();
        let mut package_identities = package_identities.into_iter();
        let mut resolved_packages = Vec::with_capacity(packages.len());
        let mut resolve_diagnostics = Vec::new();
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
                let package_identity = package_identities
                    .next()
                    .flatten()
                    .expect("every reusable module has one package identity");
                let file = SourceFile::new_in_package(
                    source_id,
                    package_identity,
                    module.source_name.as_str(),
                    module.source.as_str(),
                );
                match crate::resolved::resolve_with_imports(program, &file, &imports) {
                    Ok(program) => modules.push(ModuleUnit {
                        source_name: module.source_name,
                        program,
                    }),
                    Err(diagnostics) => resolve_diagnostics.extend(diagnostics.diagnostics),
                }
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
        debug_assert!(package_identities.next().is_none());
        if !resolve_diagnostics.is_empty() {
            return Err(SourceGraphError::Resolve {
                source: "<project>".to_owned(),
                diagnostics: DiagnosticBundle::new(resolve_diagnostics),
            });
        }
        let interface_fingerprint =
            TypedLinker::new(options).validate_package_graph(resolved_packages, &local_identity)?;
        Ok(ValidatedSourcePackageGraph {
            fingerprint,
            interface_fingerprint,
            exports: local_exports,
        })
    }
    /// Link and compile one explicit local test root with its exact package graph.
    ///
    /// The suite and deployable runtime projection are derived from the same linked typed HIR. This
    /// keeps external module calls bound to the supplied package identities while avoiding source
    /// rewriting between test and production compilation.
    pub fn build_test_project(
        &self,
        request: SourceLinkRequest,
        options: crate::compiler::CompilerOptions,
        source_name: &str,
    ) -> Result<crate::session::TestCompileOutput, DiagnosticBundle> {
        self.build_test_project_with_sources(request, &[], options, source_name)
    }
    /// Compile a target and explicitly supplied standalone test modules with one exact graph.
    ///
    /// Every source has its own stable identity and is parsed without filesystem discovery.
    /// Standalone modules inherit the target interface and only the supplied import bindings.
    pub fn build_test_project_with_sources(
        &self,
        request: SourceLinkRequest,
        test_sources: &[SourceModuleUnit],
        options: crate::compiler::CompilerOptions,
        source_name: &str,
    ) -> Result<crate::session::TestCompileOutput, DiagnosticBundle> {
        crate::session::run_with_compiler_stack(move || {
            self.build_test_project_inner(request, test_sources, options, source_name)
        })
        .map_err(|_| crate::session::compiler_worker_unavailable_diagnostic(Some(source_name)))?
    }
    fn build_test_project_inner(
        &self,
        request: SourceLinkRequest,
        test_sources: &[SourceModuleUnit],
        options: crate::compiler::CompilerOptions,
        source_name: &str,
    ) -> Result<crate::session::TestCompileOutput, DiagnosticBundle> {
        if options.mode != crate::compiler::CompilerMode::Test {
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "E_TEST_ONLY_PRODUCTION",
                DiagnosticPhase::Semantic,
                "the linked test compiler requires an explicit test-mode compiler policy",
                None,
            )));
        }
        let session = crate::session::CompilerSession::new(options.clone());
        let _chain_discriminant = session.enter_chain_discriminant();
        let linked = self
            .link_sources_inner(
                request,
                test_sources,
                LinkerOptions {
                    zk_enabled: options.force_zk,
                    test_builtins_enabled: true,
                    include_tests: true,
                },
            )
            .map_err(SourceGraphError::into_diagnostics)?;
        let suite_typed = linked.program;
        let runtime_typed =
            semantic::project_test_target_to_production(suite_typed.clone(), options.force_zk)
                .map_err(|error| {
                    DiagnosticBundle::single(Diagnostic::error(
                        error.code(),
                        phase_for_semantic_failure(error.code()),
                        error.message(),
                        None,
                    ))
                })?;
        let suite = session.build_typed_program(suite_typed, Some(source_name))?;
        let has_runtime_entrypoint = runtime_typed.items.iter().any(|item| {
            let TypedItem::Function(function) = item;
            function.modifiers.kind != FunctionKind::Private
        });
        let runtime = if has_runtime_entrypoint {
            let mut runtime_options = options;
            runtime_options.mode = crate::compiler::CompilerMode::Production;
            Some(
                crate::session::CompilerSession::new(runtime_options)
                    .build_typed_program(runtime_typed, Some(source_name))?,
            )
        } else {
            None
        };
        Ok(crate::session::TestCompileOutput { suite, runtime })
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
        let package_identities = vec![None; sources.len()];
        self.parse_sources_with_ids_scoped(sources, source_ids, &package_identities)
    }
    fn parse_sources_with_ids_scoped(
        &self,
        sources: &[SourceModuleUnit],
        source_ids: &[SourceId],
        package_identities: &[Option<String>],
    ) -> Result<Vec<SpannedProgram>, SourceGraphError> {
        self.parse_sources_with_digest_scoped(sources, source_ids, package_identities, |source| {
            Hash::new_from_chunks(&[b"kotodama-module-source-v1\0", source.as_bytes()]).to_string()
        })
    }
    #[cfg(test)]
    fn parse_sources_with_digest(
        &self,
        sources: &[SourceModuleUnit],
        source_ids: &[SourceId],
        digest: impl Fn(&str) -> String,
    ) -> Result<Vec<SpannedProgram>, SourceGraphError> {
        let package_identities = vec![None; sources.len()];
        self.parse_sources_with_digest_scoped(sources, source_ids, &package_identities, digest)
    }
    fn parse_sources_with_digest_scoped(
        &self,
        sources: &[SourceModuleUnit],
        source_ids: &[SourceId],
        package_identities: &[Option<String>],
        digest: impl Fn(&str) -> String,
    ) -> Result<Vec<SpannedProgram>, SourceGraphError> {
        debug_assert_eq!(sources.len(), source_ids.len());
        debug_assert_eq!(sources.len(), package_identities.len());
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
            .clamp(1, crate::syntax::parser::MAX_PARSER_WORKERS);
        let mut parse_diagnostics = Vec::new();
        for chunk in pending.chunks(jobs) {
            let parsed = std::thread::scope(|scope| {
                let mut handles = Vec::with_capacity(chunk.len());
                for index in chunk {
                    let item = &unique[*index];
                    let spawn_source = item.source_name.clone();
                    let handle = std::thread::Builder::new()
                        .name("kotodama-module-parser".to_owned())
                        .spawn_scoped(scope, move || {
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
                                    .map(|(program, _)| program);
                            (*index, result)
                        })
                        .map_err(|_| SourceGraphError::Parse {
                            diagnostics: crate::session::compiler_worker_unavailable_diagnostic(
                                Some(&spawn_source),
                            ),
                            source: spawn_source,
                        })?;
                    handles.push(handle);
                }
                Ok::<_, SourceGraphError>(
                    handles
                        .into_iter()
                        .map(|handle| {
                            handle
                                .join()
                                .expect("Kotodama module parser workers must not panic")
                        })
                        .collect::<Vec<_>>(),
                )
            })?;
            // Join order follows deterministic source order. Keep every
            // independent file failure rather than making thread timing or the
            // first malformed module hide the rest of the project diagnostics.
            for (index, result) in parsed {
                match result {
                    Ok(program) => unique[index].program = Some(program),
                    Err(bundle) => {
                        for member in unique[index].members.iter().copied() {
                            let mut bundle = bundle.clone();
                            remap_diagnostic_bundle_owner(
                                &mut bundle,
                                package_identities[member].as_deref(),
                                &sources[member].source_name,
                            );
                            parse_diagnostics.extend(bundle.diagnostics);
                        }
                    }
                }
            }
        }
        if !parse_diagnostics.is_empty() {
            return Err(SourceGraphError::Parse {
                source: "<project>".to_owned(),
                diagnostics: DiagnosticBundle::new(parse_diagnostics),
            });
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
fn remap_diagnostic_bundle_owner(
    bundle: &mut DiagnosticBundle,
    package_identity: Option<&str>,
    source_name: &str,
) {
    let remap = |span: &mut SourceSpan| {
        span.package_identity = package_identity.map(str::to_owned);
        span.source = Some(source_name.to_owned());
    };
    for diagnostic in &mut bundle.diagnostics {
        if let Some(primary) = &mut diagnostic.primary_span {
            remap(primary);
        }
        for label in &mut diagnostic.labels {
            remap(&mut label.span);
        }
        if let Some(fix) = &mut diagnostic.fix {
            remap(&mut fix.span);
        }
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
        validate_package_identity(&package.identity)?;
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
    /// One nominal error identity was assigned conflicting finite schemas.
    ConflictingErrorType {
        /// Stable nominal error identity.
        identity: String,
    },
    /// A module localization key collided with another linked key.
    DuplicateMessage {
        /// Repeated localization key.
        key: String,
    },
    /// Parsing succeeded but type/effect analysis rejected one unit or the link.
    Semantic {
        /// Complete canonical diagnostics, including all semantic failures and spans.
        diagnostics: DiagnosticBundle,
    },
    /// Name/export validation produced one or more structured link diagnostics.
    Diagnostics(DiagnosticBundle),
}
impl LinkError {
    /// Return the stable code for this typed-link failure.
    pub fn diagnostic_code(&self) -> &str {
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
            Self::ConflictingErrorType { .. } => "E_CONFLICTING_ERROR_TYPE",
            Self::DuplicateMessage { .. } => "E_DUPLICATE_MESSAGE",
            Self::Semantic { diagnostics } | Self::Diagnostics(diagnostics) => diagnostics
                .diagnostics
                .first()
                .map_or("K2099", |diagnostic| diagnostic.code.as_str()),
        }
    }
    /// Convert every typed-link failure into the canonical diagnostic schema.
    pub fn into_diagnostics(self) -> DiagnosticBundle {
        let diagnostic = |code, message| {
            DiagnosticBundle::single(Diagnostic::error(
                code,
                DiagnosticPhase::Resolve,
                message,
                None,
            ))
        };
        match self {
            Self::RootMustBeSeiyaku { source } => diagnostic(
                "E_ROOT_MUST_BE_SEIYAKU",
                format!("deployable root `{source}` must declare exactly one `seiyaku`/`誓約`"),
            ),
            Self::DependencyMustBeModule { source } => diagnostic(
                "E_DEPENDENCY_MUST_BE_MODULE",
                format!("dependency `{source}` must declare exactly one module"),
            ),
            Self::DuplicatePackage { package } => diagnostic(
                "E_DUPLICATE_PACKAGE",
                format!("duplicate locked package `{package}`"),
            ),
            Self::EmptyPackage { package } => diagnostic(
                "E_EMPTY_PACKAGE",
                format!("package `{package}` contains no Kotodama modules"),
            ),
            Self::DuplicateModule { package, module } => diagnostic(
                "E_DUPLICATE_MODULE",
                format!("package `{package}` declares module `{module}` more than once"),
            ),
            Self::DuplicateImport { scope, alias } => diagnostic(
                "E_DUPLICATE_IMPORT",
                format!("scope `{scope}` imports alias `{alias}` more than once"),
            ),
            Self::ReservedImport { scope, alias } => diagnostic(
                "E_RESERVED_IMPORT",
                format!(
                    "scope `{scope}` cannot import compiler-owned capability namespace `{alias}`"
                ),
            ),
            Self::DuplicateSymbol { source, symbol } => diagnostic(
                "E_DUPLICATE_DECLARATION",
                format!("source `{source}` declares symbol `{symbol}` more than once"),
            ),
            Self::UnknownPackage { scope, package } => diagnostic(
                "E_UNKNOWN_PACKAGE",
                format!("scope `{scope}` imports unknown locked package `{package}`"),
            ),
            Self::PackageImportCycle { cycle } => diagnostic(
                "E_PACKAGE_IMPORT_CYCLE",
                format!(
                    "locked package import cycle is not permitted: {}",
                    cycle.join(" -> ")
                ),
            ),
            Self::UnknownAlias { source, alias } => diagnostic(
                "E_UNKNOWN_IMPORT_ALIAS",
                format!("source `{source}` uses unknown import alias `{alias}`"),
            ),
            Self::UnexportedSymbol {
                source,
                alias,
                symbol,
            } => diagnostic(
                "E_UNEXPORTED_SYMBOL",
                format!("source `{source}` cannot call unexported symbol `{alias}::{symbol}`"),
            ),
            Self::MissingExport { package, symbol } => diagnostic(
                "E_MISSING_EXPORT",
                format!("package `{package}` exports missing function `{symbol}`"),
            ),
            Self::AmbiguousExport { package, symbol } => diagnostic(
                "E_AMBIGUOUS_EXPORT",
                format!(
                    "package `{package}` exports ambiguous function `{symbol}` from multiple modules"
                ),
            ),
            Self::WildcardImport { source } => diagnostic(
                "E_WILDCARD_IMPORT",
                format!(
                    "source `{source}` uses a wildcard import; Kotodama V1 requires explicit symbols"
                ),
            ),
            Self::InvalidIdentifier { context, name } => diagnostic(
                "E_INVALID_IDENTIFIER",
                format!("invalid Kotodama V1 identifier `{name}` in {context}"),
            ),
            Self::ReservedSymbol { source, symbol } => diagnostic(
                "E_RESERVED_DECLARATION",
                format!("source `{source}` declares reserved symbol `{symbol}`"),
            ),
            Self::InvalidModuleItem { source, item } => diagnostic(
                "E_INVALID_MODULE_ITEM",
                format!("module `{source}` contains deployable-only {item}"),
            ),
            Self::ConflictingErrorType { identity } => diagnostic(
                "E_CONFLICTING_ERROR_TYPE",
                format!("linked modules assign conflicting schemas to error type `{identity}`"),
            ),
            Self::DuplicateMessage { key } => diagnostic(
                "E_DUPLICATE_MESSAGE",
                format!("linked modules define duplicate messages key `{key}`"),
            ),
            Self::Semantic { diagnostics } | Self::Diagnostics(diagnostics) => diagnostics,
        }
    }
}
impl fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.clone().into_diagnostics().render_human())
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
    pub fn link(&self, request: LinkRequest) -> Result<TypedProgram, LinkError> {
        self.link_with_tests(request, Vec::new())
    }
    fn link_with_tests(
        &self,
        request: LinkRequest,
        test_sources: Vec<(SpannedProgram, SourceFile)>,
    ) -> Result<TypedProgram, LinkError> {
        let sources = std::iter::once(request.root.program.source_file().clone())
            .chain(request.packages.iter().flat_map(|package| {
                package
                    .modules
                    .iter()
                    .map(|module| module.program.source_file().clone())
            }))
            .chain(test_sources.iter().map(|(_, file)| file.clone()))
            .collect::<Vec<_>>();
        crate::session::run_with_compiler_stack(move || self.link_inner(request, test_sources))
            .map_err(|_| LinkError::Semantic {
                diagnostics: crate::session::compiler_worker_unavailable_diagnostic(None),
            })?
            .map_err(|mut error| {
                if let LinkError::Semantic { diagnostics } | LinkError::Diagnostics(diagnostics) =
                    &mut error
                {
                    for source in &sources {
                        diagnostics.capture_source(source);
                    }
                }
                error
            })
    }
    fn link_inner(
        &self,
        mut request: LinkRequest,
        test_sources: Vec<(SpannedProgram, SourceFile)>,
    ) -> Result<TypedProgram, LinkError> {
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
        let mut import_diagnostics =
            imported_call_diagnostics(&request.root, &root_imports, &resolved_packages);
        for package in &resolved_packages {
            for module in &package.modules {
                import_diagnostics.extend(imported_call_diagnostics(
                    module.source,
                    &package.imports,
                    &resolved_packages,
                ));
            }
        }
        if !import_diagnostics.is_empty() {
            return Err(LinkError::Diagnostics(DiagnosticBundle::new(
                import_diagnostics,
            )));
        }
        let root_external = external_signatures(&root_imports, &resolved_packages);
        let root_types = external_types(&root_imports, &resolved_packages);
        let semantic = semantic::SemanticContext::with_capabilities(
            self.options.zk_enabled,
            self.options.test_builtins_enabled,
        );
        let mut root = semantic
            .analyze_resolved_with_external_types(
                &request.root.program,
                &root_external,
                &root_types,
            )
            .map_err(|failures| semantic_link_error(&request.root, failures))?;
        let root_external_names = external_linked_names(&root_imports, &resolved_packages);
        if !test_sources.is_empty() {
            let signatures = root
                .items
                .iter()
                .map(|item| {
                    let TypedItem::Function(function) = item;
                    (
                        function.name.clone(),
                        FunctionSignature {
                            params: function.param_types.clone(),
                            return_type: function.ret_ty.clone().unwrap_or(Type::Unit),
                            modifiers: function.modifiers.clone(),
                        },
                    )
                })
                .collect();
            let states = root
                .states
                .iter()
                .map(|state| (state.name.clone(), state.ty.clone()))
                .collect();
            let mut environment = semantic.test_target_environment(signatures, states);
            environment.functions.extend(root_external);
            environment.types.extend(root_types);
            let resolution_environment = crate::resolved::ExternalResolutionEnvironment {
                functions: environment.functions.keys().cloned().collect(),
                states: environment.states.keys().cloned().collect(),
                structs: environment.structs.keys().cloned().collect(),
                consts: environment.consts.keys().cloned().collect(),
                error_codes: environment
                    .error_codes
                    .iter()
                    .map(|(name, code)| (name.clone(), *code))
                    .collect(),
            };
            let mut tests = Vec::with_capacity(test_sources.len());
            for (program, file) in test_sources {
                crate::session::validate_test_module_source(
                    &program.program,
                    Some(file.name()),
                    &request.root.source_name,
                )
                .map_err(LinkError::Diagnostics)?;
                let resolved = crate::resolved::resolve_with_imports_and_external_environment(
                    program,
                    &file,
                    &resolution_environment,
                )
                .map_err(LinkError::Diagnostics)?;
                let module = ModuleUnit {
                    source_name: file.name().to_owned(),
                    program: resolved,
                };
                let diagnostics =
                    imported_call_diagnostics(&module, &root_imports, &resolved_packages);
                if !diagnostics.is_empty() {
                    return Err(LinkError::Diagnostics(DiagnosticBundle::new(diagnostics)));
                }
                validate_program_symbols(&module)?;
                tests.push(module);
            }
            crate::session::reject_duplicate_test_graph_symbols(
                std::iter::once((&request.root.program, request.root.program.source_file())).chain(
                    tests
                        .iter()
                        .map(|module| (&module.program, module.program.source_file())),
                ),
            )
            .map_err(LinkError::Diagnostics)?;
            for module in tests {
                let context =
                    semantic::SemanticContext::with_capabilities(self.options.zk_enabled, true);
                let mut typed = context
                    .analyze_resolved_with_test_target(&module.program, &environment)
                    .map_err(|failures| semantic_link_error(&module, failures))?;
                crate::session::merge_source_files(&mut root, &mut typed, &module.source_name)
                    .map_err(LinkError::Diagnostics)?;
                root.items.append(&mut typed.items);
                root.error_types.append(&mut typed.error_types);
                root.message_entries.append(&mut typed.message_entries);
                root.test_support_enabled |= typed.test_support_enabled;
            }
        }
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
        packages: Vec<PackageUnit>,
        local_identity: &str,
    ) -> Result<Hash, LinkError> {
        crate::session::run_with_compiler_stack(move || {
            self.validate_package_graph_inner(packages, local_identity)
        })
        .map_err(|_| LinkError::Semantic {
            diagnostics: crate::session::compiler_worker_unavailable_diagnostic(None),
        })?
    }
    fn validate_package_graph_inner(
        &self,
        mut packages: Vec<PackageUnit>,
        local_identity: &str,
    ) -> Result<Hash, LinkError> {
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
        let interface_fingerprint = resolved_packages
            .iter()
            .find(|package| package.identity == local_identity)
            .map(package_interface_fingerprint)
            .expect("validated local package remains in the resolved graph");
        let import_diagnostics = resolved_packages
            .iter()
            .flat_map(|package| {
                package.modules.iter().flat_map(|module| {
                    imported_call_diagnostics(module.source, &package.imports, &resolved_packages)
                })
            })
            .collect::<Vec<_>>();
        if !import_diagnostics.is_empty() {
            return Err(LinkError::Diagnostics(DiagnosticBundle::new(
                import_diagnostics,
            )));
        }
        link_resolved_packages(self.options, &resolved_packages, None)?;
        Ok(interface_fingerprint)
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
    types: BTreeMap<String, Type>,
    linked_names: BTreeMap<String, String>,
    local_structs: HashSet<String>,
    type_prefix: String,
}
struct ResolvedPackage<'request> {
    identity: String,
    imports: BTreeMap<String, usize>,
    modules: Vec<ResolvedModule<'request>>,
    exports: BTreeMap<String, ResolvedExport>,
    type_exports: BTreeMap<String, Type>,
}
fn package_interface_fingerprint(package: &ResolvedPackage<'_>) -> Hash {
    let mut transcript = b"kotodama-package-interface-v1\0".to_vec();
    interface_count(&mut transcript, package.exports.len());
    for (name, export) in &package.exports {
        interface_field(&mut transcript, name.as_bytes());
        let signature = &export.signature;
        interface_count(&mut transcript, signature.params.len());
        for parameter in &signature.params {
            interface_field(&mut transcript, parameter.name.as_bytes());
            transcript.push(u8::from(parameter.is_state));
            transcript.push(match parameter.call_mode {
                crate::ast::ParameterCallMode::Named => 0,
                crate::ast::ParameterCallMode::Positional => 1,
            });
            interface_type(&mut transcript, &parameter.ty);
        }
        interface_type(&mut transcript, &signature.return_type);
        interface_modifiers(&mut transcript, &signature.modifiers);
    }
    interface_count(&mut transcript, package.type_exports.len());
    for (name, ty) in &package.type_exports {
        interface_field(&mut transcript, name.as_bytes());
        interface_type(&mut transcript, ty);
    }
    Hash::new(transcript)
}
fn interface_field(transcript: &mut Vec<u8>, value: &[u8]) {
    transcript.extend_from_slice(
        &u64::try_from(value.len())
            .expect("bounded Kotodama interface field length fits u64")
            .to_le_bytes(),
    );
    transcript.extend_from_slice(value);
}
fn interface_count(transcript: &mut Vec<u8>, count: usize) {
    transcript.extend_from_slice(
        &u64::try_from(count)
            .expect("bounded Kotodama interface collection length fits u64")
            .to_le_bytes(),
    );
}
fn interface_optional_field(transcript: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            transcript.push(1);
            interface_field(transcript, value.as_bytes());
        }
        None => transcript.push(0),
    }
}
fn interface_modifiers(transcript: &mut Vec<u8>, modifiers: &crate::ast::FunctionModifiers) {
    transcript.push(match modifiers.kind {
        FunctionKind::Private => 0,
        FunctionKind::Kotoage => 1,
        FunctionKind::Hajimari => 2,
        FunctionKind::Kaizen => 3,
        FunctionKind::View => 4,
    });
    interface_optional_field(transcript, modifiers.permission.as_deref());
    transcript.push(u8::from(modifiers.is_test));
    interface_optional_field(transcript, modifiers.test_fixture.as_deref());
}
fn interface_type(transcript: &mut Vec<u8>, ty: &Type) {
    match ty {
        Type::Int => transcript.push(0),
        Type::Decimal => transcript.push(1),
        Type::Quantity => transcript.push(2),
        Type::Bool => transcript.push(3),
        Type::String => transcript.push(4),
        Type::Bytes => transcript.push(5),
        Type::DataSpaceId => transcript.push(6),
        Type::AxtDescriptor => transcript.push(7),
        Type::AssetHandle => transcript.push(8),
        Type::ProofBlob => transcript.push(9),
        Type::SoracloudRequest => transcript.push(10),
        Type::SoracloudResponse => transcript.push(11),
        Type::AccountId => transcript.push(12),
        Type::AssetDefinitionId => transcript.push(13),
        Type::AssetId => transcript.push(14),
        Type::NftId => transcript.push(15),
        Type::DomainId => transcript.push(16),
        Type::Name => transcript.push(17),
        Type::Json => transcript.push(18),
        Type::Unit => transcript.push(19),
        Type::ErrorEnum(descriptor) => {
            transcript.push(28);
            interface_field(transcript, descriptor.identity.as_bytes());
            interface_field(transcript, &descriptor.schema_hash());
        }
        Type::StateCursor(key) => {
            transcript.push(29);
            interface_type(transcript, key);
        }
        Type::Secret(inner) => {
            transcript.push(20);
            interface_type(transcript, inner);
        }
        Type::StateMap(key, value) => {
            transcript.push(21);
            interface_type(transcript, key);
            interface_type(transcript, value);
        }
        Type::Option(inner) => {
            transcript.push(22);
            interface_type(transcript, inner);
        }
        Type::Result(ok, error) => {
            transcript.push(23);
            interface_type(transcript, ok);
            interface_type(transcript, error);
        }
        Type::List(element, capacity) => {
            transcript.push(24);
            interface_type(transcript, element);
            transcript.push(*capacity);
        }
        Type::Tuple(items) => {
            transcript.push(25);
            interface_count(transcript, items.len());
            for item in items {
                interface_type(transcript, item);
            }
        }
        Type::Struct { name, fields } => {
            transcript.push(26);
            interface_field(transcript, name.as_bytes());
            interface_count(transcript, fields.len());
            for (field, ty) in fields.iter() {
                interface_field(transcript, field.as_bytes());
                interface_type(transcript, ty);
            }
        }
        Type::NamedStruct(name) => {
            transcript.push(27);
            interface_field(transcript, name.as_bytes());
        }
    }
}
fn semantic_link_error(module: &ModuleUnit, failures: semantic::SemanticFailures) -> LinkError {
    LinkError::Semantic {
        diagnostics: crate::semantic_diagnostics::from_semantic_failures(
            failures,
            Some(&module.source_name),
            Some(module.program.source_file()),
            Some(&module.program),
        ),
    }
}
fn validate_linker_options(options: LinkerOptions) -> Result<(), LinkError> {
    if options.include_tests != options.test_builtins_enabled {
        return Err(LinkError::Semantic {
            diagnostics: DiagnosticBundle::single(Diagnostic::error(
                "E_TEST_ONLY_PRODUCTION",
                DiagnosticPhase::Semantic,
                "include_tests and test_builtins_enabled must select one explicit compiler test mode together",
                None,
            )),
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
        validate_package_identity(&package.identity)?;
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
    let mut resolved_packages: Vec<Option<ResolvedPackage<'request>>> =
        (0..packages.len()).map(|_| None).collect();
    let mut remaining = (0..packages.len()).collect::<BTreeSet<_>>();
    let mut export_diagnostics = Vec::new();
    while !remaining.is_empty() {
        let package_index = remaining
            .iter()
            .copied()
            .find(|index| {
                resolved_imports[*index]
                    .values()
                    .all(|dependency| resolved_packages[*dependency].is_some())
            })
            .expect("validated acyclic package imports have a ready dependency");
        remaining.remove(&package_index);
        let package = &packages[package_index];
        let imports = resolved_imports[package_index].clone();
        let mut imported_types = BTreeMap::new();
        for (alias, dependency) in &imports {
            for (name, ty) in &resolved_packages[*dependency]
                .as_ref()
                .expect("dependency resolved")
                .type_exports
            {
                imported_types.insert(format!("{alias}::{name}"), ty.clone());
            }
        }
        let mut modules = Vec::with_capacity(package.modules.len());
        for (module_index, module) in package.modules.iter().enumerate() {
            let semantic = semantic::SemanticContext::with_capabilities(
                options.zk_enabled,
                options.test_builtins_enabled,
            );
            semantic.set_package_identity(package.identity.clone());
            let mut signatures = semantic
                .resolve_resolved_function_signatures_with_types(&module.program, &imported_types)
                .map_err(|failures| semantic_link_error(module, failures))?;
            let mut types = semantic
                .declared_nominal_types(module.ast())
                .map_err(|error| {
                    semantic_link_error(module, semantic::SemanticFailures::from(error))
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
            let type_prefix = format!("{}::{}", package.identity, module.ast().unit.name);
            for signature in signatures.values_mut() {
                qualify_signature(signature, &local_structs, &type_prefix);
            }
            for ty in types.values_mut() {
                qualify_type(ty, &local_structs, &type_prefix);
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
                types,
                linked_names,
                local_structs,
                type_prefix,
            });
        }
        let mut exports = BTreeMap::new();
        let mut type_exports = BTreeMap::new();
        for export in &package.exports {
            validate_identifier("package export", export)?;
            let candidates = modules
                .iter()
                .filter(|module| {
                    module.signatures.contains_key(export) || module.types.contains_key(export)
                })
                .collect::<Vec<_>>();
            let module = match candidates.as_slice() {
                [] => {
                    export_diagnostics.push(Diagnostic::error(
                        "E_MISSING_EXPORT",
                        DiagnosticPhase::Resolve,
                        format!(
                            "package `{}` exports missing declaration `{export}`",
                            package.identity
                        ),
                        None,
                    ));
                    continue;
                }
                [module] => *module,
                _ => {
                    let mut spans = candidates.iter().filter_map(|module| {
                        module
                            .source
                            .program
                            .symbols()
                            .find(|symbol| symbol.name == *export)
                            .and_then(|symbol| module.source.program.source_span(symbol.source))
                    });
                    let primary_span = spans.next();
                    let mut diagnostic = Diagnostic::error(
                        "E_AMBIGUOUS_EXPORT",
                        DiagnosticPhase::Resolve,
                        format!(
                            "package `{}` exports ambiguous declaration `{export}` from multiple modules",
                            package.identity
                        ),
                        primary_span,
                    );
                    diagnostic.labels.extend(spans.map(|span| {
                        DiagnosticLabel {
                            span,
                            message: "another exported declaration with this name is declared here"
                                .to_owned(),
                        }
                    }));
                    export_diagnostics.push(diagnostic);
                    continue;
                }
            };
            if let Some(signature) = module.signatures.get(export) {
                exports.insert(
                    export.clone(),
                    ResolvedExport {
                        linked_name: module
                            .linked_names
                            .get(export)
                            .expect("every signature receives a linked name")
                            .clone(),
                        signature: signature.clone(),
                    },
                );
            } else if let Some(ty) = module.types.get(export) {
                type_exports.insert(export.clone(), ty.clone());
            }
        }
        resolved_packages[package_index] = Some(ResolvedPackage {
            identity: package.identity.clone(),
            imports,
            modules,
            exports,
            type_exports,
        });
    }
    if export_diagnostics.is_empty() {
        Ok(resolved_packages
            .into_iter()
            .map(|package| package.expect("all packages resolved"))
            .collect())
    } else {
        Err(LinkError::Diagnostics(DiagnosticBundle::new(
            export_diagnostics,
        )))
    }
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
    let mut seen_error_types = BTreeMap::new();
    if let Some(root) = &mut linked {
        for error in std::mem::take(&mut root.error_types) {
            let hash = error.schema_hash();
            if let Some(previous) = seen_error_types.get(&error.identity) {
                if previous != &hash {
                    return Err(LinkError::ConflictingErrorType {
                        identity: error.identity,
                    });
                }
            } else {
                seen_error_types.insert(error.identity.clone(), hash);
                root.error_types.push(error);
            }
        }
    }
    let mut seen_messages = linked
        .iter()
        .flat_map(|program| program.message_entries.iter())
        .map(|entry| entry.msg_id.clone())
        .collect::<HashSet<_>>();
    for package in packages {
        let external = external_signatures(&package.imports, packages);
        let types = external_types(&package.imports, packages);
        let external_names = external_linked_names(&package.imports, packages);
        for module in &package.modules {
            let semantic = semantic::SemanticContext::with_capabilities(
                options.zk_enabled,
                options.test_builtins_enabled,
            );
            semantic.set_package_identity(package.identity.clone());
            let mut typed = semantic
                .analyze_resolved_with_external_types(&module.source.program, &external, &types)
                .map_err(|failures| semantic_link_error(module.source, failures))?;
            qualify_typed_program(&mut typed, &module.local_structs, &module.type_prefix);
            rename_program_calls(&mut typed, &module.linked_names, &external_names);
            let mut new_error_types = Vec::new();
            for error in std::mem::take(&mut typed.error_types) {
                let hash = error.schema_hash();
                if let Some(previous) = seen_error_types.get(&error.identity) {
                    if previous != &hash {
                        return Err(LinkError::ConflictingErrorType {
                            identity: error.identity,
                        });
                    }
                } else {
                    seen_error_types.insert(error.identity.clone(), hash);
                    new_error_types.push(error);
                }
            }
            typed.error_types = new_error_types;
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
                            diagnostics: DiagnosticBundle::single(Diagnostic::error(
                                "E_INTERNAL_RESOLUTION",
                                DiagnosticPhase::Resolve,
                                format!(
                                    "typed module graph reused HIR identity {}:{}",
                                    id.source.0, id.local.0
                                ),
                                None,
                            )),
                        });
                    }
                }
                for (source_id, source_file) in std::mem::take(&mut typed.source_files) {
                    if let Some(previous) =
                        program.source_files.insert(source_id, source_file.clone())
                        && previous != source_file
                    {
                        return Err(LinkError::Semantic {
                            diagnostics: DiagnosticBundle::single(Diagnostic::error(
                                "E_INTERNAL_RESOLUTION",
                                DiagnosticPhase::Resolve,
                                format!(
                                    "compiler assigned SourceId {} to both `{}` and `{}`",
                                    source_id.0,
                                    previous.name(),
                                    source_file.name()
                                ),
                                None,
                            )),
                        });
                    }
                }
                program.items.extend(typed.items);
                program.states.extend(typed.states);
                program.error_types.extend(typed.error_types);
                program.triggers.extend(typed.triggers);
                program.message_entries.extend(typed.message_entries);
                program.test_support_enabled |= typed.test_support_enabled;
            } else {
                linked = Some(typed);
            }
        }
    }
    let linked = linked.ok_or_else(|| LinkError::Semantic {
        diagnostics: DiagnosticBundle::single(Diagnostic::error(
            "E_EMPTY_PACKAGE_GRAPH",
            DiagnosticPhase::Resolve,
            "package graph contains no typed modules",
            None,
        )),
    })?;
    semantic::validate_linked_program(&linked, options.zk_enabled).map_err(|error| {
        LinkError::Semantic {
            diagnostics: DiagnosticBundle::single(Diagnostic::error(
                error.code,
                phase_for_semantic_failure(error.code),
                error.message,
                None,
            )),
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
        || Builtin::registry().any(|(builtin, spec)| {
            matches!(
                spec.surface,
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
fn external_types(
    imports: &BTreeMap<String, usize>,
    packages: &[ResolvedPackage<'_>],
) -> BTreeMap<String, Type> {
    let mut external = BTreeMap::new();
    for (alias, package_index) in imports {
        for (symbol, ty) in &packages[*package_index].type_exports {
            external.insert(format!("{alias}::{symbol}"), ty.clone());
        }
    }
    external
}
fn validate_package_identity(identity: &str) -> Result<(), LinkError> {
    if !ivm_abi::entrypoint::is_canonical_kotodama_package_identity(identity) {
        return Err(LinkError::InvalidIdentifier {
            context: "locked package identity".to_owned(),
            name: identity.to_owned(),
        });
    }
    Ok(())
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
            Item::Function(function) => match function.modifiers.kind {
                crate::ast::FunctionKind::Private => None,
                crate::ast::FunctionKind::Kotoage => Some("kotoage declaration"),
                crate::ast::FunctionKind::View => Some("view fn declaration"),
                crate::ast::FunctionKind::Hajimari => Some("hajimari declaration"),
                crate::ast::FunctionKind::Kaizen => Some("kaizen declaration"),
            },
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
fn imported_call_diagnostics(
    module: &ModuleUnit,
    imports: &BTreeMap<String, usize>,
    packages: &[ResolvedPackage<'_>],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for ty in module.program.types() {
        if ty.target != crate::resolved::ResolvedTypeTarget::ExternalType {
            continue;
        }
        let Some((alias, symbol)) = ty.name.split_once("::") else {
            continue;
        };
        let error = match imports.get(alias) {
            None => Some((
                "E_UNKNOWN_IMPORT_ALIAS",
                format!(
                    "source `{}` uses unknown import alias `{alias}`",
                    module.source_name
                ),
            )),
            Some(index) if !packages[*index].type_exports.contains_key(symbol) => Some((
                "E_UNEXPORTED_TYPE",
                format!(
                    "source `{}` cannot use unexported type `{}`",
                    module.source_name, ty.name
                ),
            )),
            Some(_) => None,
        };
        if let Some((code, message)) = error {
            diagnostics.push(Diagnostic::error(
                code,
                DiagnosticPhase::Resolve,
                message,
                module.program.source_span(ty.source),
            ));
        }
    }
    for call in module.program.calls() {
        if call.target != crate::resolved::ResolvedCallTarget::External {
            continue;
        }
        // Unqualified external calls are already bound to the authenticated standalone
        // target environment by the resolver. Only explicit package calls use aliases.
        if !call.name.contains("::") {
            continue;
        }
        let mut parts = call.name.split("::");
        let alias = parts.next().expect("split always has a first item");
        let symbol = parts.next();
        if alias == "*" || symbol == Some("*") || parts.next().is_some() {
            diagnostics.push(Diagnostic::error(
                "E_WILDCARD_IMPORT",
                DiagnosticPhase::Resolve,
                format!(
                    "source `{}` uses a wildcard import; Kotodama V1 requires explicit symbols",
                    module.source_name
                ),
                module.program.source_span(call.name_source),
            ));
            continue;
        }
        let symbol = symbol.unwrap_or_default();
        let Some(package_index) = imports.get(alias).copied() else {
            diagnostics.push(Diagnostic::error(
                "E_UNKNOWN_IMPORT_ALIAS",
                DiagnosticPhase::Resolve,
                format!(
                    "source `{}` uses unknown import alias `{alias}`",
                    module.source_name
                ),
                module.program.source_span(call.name_source),
            ));
            continue;
        };
        if !packages[package_index].exports.contains_key(symbol) {
            diagnostics.push(Diagnostic::error(
                "E_UNEXPORTED_SYMBOL",
                DiagnosticPhase::Resolve,
                format!(
                    "source `{}` cannot call unexported symbol `{alias}::{symbol}`",
                    module.source_name
                ),
                module.program.source_span(call.name_source),
            ));
        }
    }
    diagnostics
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
                *name = format!("{prefix}::{name}");
            }
            for (_, field) in Arc::make_mut(fields) {
                qualify_type(field, local_structs, prefix);
            }
        }
        Type::NamedStruct(name) if local_structs.contains(name) => {
            *name = format!("{prefix}::{name}");
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
        | Type::ErrorEnum(_)
        | Type::StateCursor(_)
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
        | ExprKind::ErrorValue(_)
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
        | ExprKind::ErrorValue(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => {}
    }
}
#[cfg(test)]
#[path = "linker_tests.rs"]
mod tests;
