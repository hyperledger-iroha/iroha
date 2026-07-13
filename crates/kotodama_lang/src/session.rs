//! Reusable canonical compiler session API.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

use indexmap::IndexMap;
use iroha_data_model::smart_contract::manifest::ContractManifest;
use ivm_abi::metadata::EmbeddedContractInterfaceV1;

use crate::{
    ast::{FunctionKind, Item, Program, SourceUnitKind},
    compiler::{CompileReport, Compiler, CompilerMode, CompilerOptions},
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticLabel, DiagnosticPhase, SourcePosition, SourceSpan,
    },
    lexer::{Token, TokenKind},
    semantic::TypedProgram,
    source::{FrontendBudget, MAX_SOURCE_BYTES, SourceFile, SourceId, TextRange},
};

/// One compilation request.
#[derive(Clone, Copy, Debug)]
pub struct CompileRequest<'source> {
    /// Kotodama source text.
    pub source: &'source str,
    /// Logical source path used in diagnostics and sidecars.
    pub source_name: Option<&'source str>,
}

/// Successful canonical compiler output.
#[derive(Clone, Debug)]
pub struct CompileOutput {
    /// Compiled `.to` bytes.
    ///
    /// Production output is deployable; test-mode output is a local-only
    /// generic IVM harness.
    pub artifact: Vec<u8>,
    /// Exact compiler-owned contract interface for this artifact.
    ///
    /// Production artifacts embed the same descriptor in their `CNTR` section.
    /// Local test artifacts carry it beside the generic IVM image so the test
    /// runner can validate entrypoint and durable-state metadata without making
    /// the harness deployable.
    pub contract_interface: EmbeddedContractInterfaceV1,
    /// Manifest derived from the compiler-owned contract interface.
    pub manifest: ContractManifest,
    /// Source-map, budget, and access-hint sidecar data.
    pub report: CompileReport,
}

/// Paired artifacts produced for one explicitly selected local test suite.
#[derive(Clone, Debug)]
pub struct TestCompileOutput {
    /// Test-mode artifact containing the local test functions.
    pub suite: CompileOutput,
    /// Production-mode artifact derived after removing local-only test declarations.
    ///
    /// Pure unit-test targets with no public runtime entrypoint do not need a
    /// deployable projection and therefore return `None`.
    pub runtime: Option<CompileOutput>,
}

/// One source-identified input to the typed-HIR local test linker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestSourceUnit {
    /// Stable logical path retained in diagnostics and debug sidecars.
    pub source_name: String,
    /// Complete bounded Kotodama source text.
    pub source: String,
}

/// Explicit reusable compiler context used by CLIs, SDK bindings, and language tools.
#[derive(Clone, Debug)]
pub struct CompilerSession {
    options: CompilerOptions,
}

struct IterativeResolvedGuard {
    program: Option<crate::resolved::ResolvedProgram>,
}

impl Clone for IterativeResolvedGuard {
    fn clone(&self) -> Self {
        Self::new(self.get().clone())
    }
}

impl IterativeResolvedGuard {
    fn new(program: crate::resolved::ResolvedProgram) -> Self {
        Self {
            program: Some(program),
        }
    }

    fn get(&self) -> &crate::resolved::ResolvedProgram {
        self.program.as_ref().expect("resolved guard is populated")
    }

    fn take_program(mut self) -> Program {
        self.program
            .take()
            .expect("resolved guard is populated")
            .into_program()
    }
}

impl Drop for IterativeResolvedGuard {
    fn drop(&mut self) {
        if let Some(program) = self.program.take() {
            crate::ast::drop_program_iterative(program.into_program());
        }
    }
}

/// Canonically parsed source retained between compiler phases.
///
/// The fields stay crate-private so instrumentation can time phase boundaries
/// without exposing forgeable frontend artifacts to compiler clients.
#[derive(Clone)]
pub(crate) struct ParsedCompilationUnit {
    source: SourceFile,
    program: crate::spanned_ast::SpannedProgram,
    source_name: Option<String>,
}

/// Canonically resolved HIR retained between compiler phases.
///
/// The iterative guard preserves the normal compiler's bounded-drop behavior
/// when a benchmark discards a cloned phase input.
#[derive(Clone)]
pub(crate) struct ResolvedCompilationUnit {
    source: SourceFile,
    program: IterativeResolvedGuard,
    source_name: Option<String>,
}

impl CompilerSession {
    /// Create a session with deterministic compiler options.
    pub fn new(options: CompilerOptions) -> Self {
        Self { options }
    }

    /// Return a deterministic cache identity for every caller-controlled
    /// compiler policy that can change deployable output.
    pub fn policy_fingerprint(&self) -> iroha_crypto::Hash {
        let mode = match self.options.mode {
            crate::compiler::CompilerMode::Production => 0_u8,
            crate::compiler::CompilerMode::Test => 1_u8,
        };
        iroha_crypto::Hash::new_from_chunks(&[
            b"kotodama-compiler-policy-v1\0",
            &[u8::from(self.options.force_zk)],
            &self.options.max_cycles.to_le_bytes(),
            &[mode],
        ])
    }

    /// Derive typed-module linker capabilities from this compiler session.
    ///
    /// Keeping this mapping owned by the session prevents package frontends
    /// from linking with ZK or test capabilities that differ from the policy
    /// later used to generate the deployable artifact.
    pub(crate) fn linker_options(&self) -> crate::linker::LinkerOptions {
        let test_mode = self.options.mode == crate::compiler::CompilerMode::Test;
        crate::linker::LinkerOptions {
            zk_enabled: self.options.force_zk,
            test_builtins_enabled: test_mode,
            include_tests: test_mode,
        }
    }

    /// Validate a reusable package and its authenticated locked dependencies.
    ///
    /// Package frontends use this entry point so typed linking receives the
    /// exact production, ZK, and test capabilities owned by this compiler
    /// session. Validation does not synthesize a deployable seiyaku and does
    /// not emit bytecode.
    pub fn validate_package_graph(
        &self,
        graph: &crate::linker::ModuleBuildGraph,
        request: crate::linker::SourcePackageGraphRequest,
    ) -> Result<crate::linker::ValidatedSourcePackageGraph, crate::linker::SourceGraphError> {
        graph.validate_package(request, self.linker_options())
    }

    /// Parse and type/effect-check one seiyaku or reusable module without
    /// publishing deployable output.
    pub fn check(&self, request: CompileRequest<'_>) -> Result<(), DiagnosticBundle> {
        let program = self.checked_program(request)?;
        crate::ast::drop_program_iterative(program);
        Ok(())
    }

    /// Run the canonical check pipeline and return non-fatal lint findings.
    ///
    /// Parsing and semantic analysis occur exactly once. Frontends use this
    /// method to replace the retired standalone linter without maintaining a
    /// second parser path.
    pub fn check_with_lints(
        &self,
        request: CompileRequest<'_>,
    ) -> Result<Vec<crate::lint::LintWarning>, DiagnosticBundle> {
        let program = self.checked_program(request)?;
        let warnings = crate::lint::lint_program(&program);
        crate::ast::drop_program_iterative(program);
        Ok(warnings)
    }

    /// Parse one source through the canonical, budgeted lossless frontend.
    pub(crate) fn parse_compilation_unit(
        &self,
        request: CompileRequest<'_>,
    ) -> Result<ParsedCompilationUnit, DiagnosticBundle> {
        enforce_source_budget(request)?;
        let source = SourceFile::new(
            SourceId(0),
            request.source_name.unwrap_or("<source>"),
            request.source,
        );
        let (program, tokens) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())?;
        reject_production_test_surface(
            self.options.mode,
            &program.program,
            request.source_name,
            Some((&source, &tokens)),
        )?;
        Ok(ParsedCompilationUnit {
            source,
            program,
            source_name: request.source_name.map(ToOwned::to_owned),
        })
    }

    /// Resolve one canonical parsed source into fail-closed named HIR.
    pub(crate) fn resolve_compilation_unit(
        &self,
        parsed: ParsedCompilationUnit,
    ) -> Result<ResolvedCompilationUnit, DiagnosticBundle> {
        let ParsedCompilationUnit {
            source,
            program,
            source_name,
        } = parsed;
        let program = crate::resolved::resolve(program, &source)?;
        Ok(ResolvedCompilationUnit {
            source,
            program: IterativeResolvedGuard::new(program),
            source_name,
        })
    }

    /// Type- and effect-check one resolved-HIR source with this session's policy.
    pub(crate) fn type_effect_compilation_unit(
        &self,
        resolved: ResolvedCompilationUnit,
    ) -> Result<TypedProgram, DiagnosticBundle> {
        self.type_effect_compilation_unit_ref(&resolved)
    }

    fn type_effect_compilation_unit_ref(
        &self,
        resolved: &ResolvedCompilationUnit,
    ) -> Result<TypedProgram, DiagnosticBundle> {
        let semantic = crate::semantic::SemanticContext::with_capabilities(
            self.options.force_zk,
            self.options.mode == crate::compiler::CompilerMode::Test,
        );
        let typed = semantic
            .analyze_resolved(resolved.program.get())
            .map_err(|failures| {
                crate::semantic_diagnostics::from_semantic_failures(
                    failures,
                    resolved.source_name.as_deref(),
                    Some(&resolved.source),
                    Some(resolved.program.get()),
                )
            })?;
        enforce_argument_register_window(&typed, &resolved.source, resolved.program.get())?;
        Ok(typed)
    }

    fn checked_program(&self, request: CompileRequest<'_>) -> Result<Program, DiagnosticBundle> {
        let parsed = self.parse_compilation_unit(request)?;
        let resolved = self.resolve_compilation_unit(parsed)?;
        let typed = self.type_effect_compilation_unit_ref(&resolved)?;
        crate::semantic::validate_linked_program(&typed, self.options.force_zk).map_err(
            |error| {
                crate::semantic_diagnostics::from_semantic_failures(
                    error.into(),
                    request.source_name,
                    Some(&resolved.source),
                    Some(resolved.program.get()),
                )
            },
        )?;
        crate::policy::enforce_on_chain_profile(&typed).map_err(|errors| {
            DiagnosticBundle::new(
                errors
                    .into_iter()
                    .map(|error| {
                        Diagnostic::error(
                            "K2100",
                            DiagnosticPhase::Semantic,
                            error.message,
                            source_start_span(request.source_name),
                        )
                    })
                    .collect(),
            )
        })?;
        Ok(resolved.program.take_program())
    }

    /// Compile one named source unit into a deployable artifact and sidecar report.
    pub fn build(&self, request: CompileRequest<'_>) -> Result<CompileOutput, DiagnosticBundle> {
        let parsed = self.parse_compilation_unit(request)?;
        let resolved = self.resolve_compilation_unit(parsed)?;
        let typed = self.type_effect_compilation_unit(resolved)?;
        self.build_typed_program(typed, request.source_name)
    }

    /// Compile a source-identified local test graph and its verified runtime projection.
    ///
    /// Every file is parsed and resolved independently with a stable `SourceId`.
    /// Standalone modules receive only the target's typed function/state
    /// interface; no AST items are flattened, reordered, or rewritten.
    pub fn build_test_sources(
        &self,
        target: &TestSourceUnit,
        test_modules: &[TestSourceUnit],
    ) -> Result<TestCompileOutput, DiagnosticBundle> {
        if self.options.mode != CompilerMode::Test {
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "E_TEST_ONLY_PRODUCTION",
                DiagnosticPhase::Semantic,
                "the local test compiler requires an explicit test-mode CompilerSession",
                source_start_span(Some(&target.source_name)),
            )));
        }
        let source_count = 1_usize.saturating_add(test_modules.len());
        let source_bytes = test_modules
            .iter()
            .fold(target.source.len(), |total, source| {
                total.saturating_add(source.source.len())
            });
        if source_count > crate::linker::MAX_MODULE_GRAPH_SOURCES
            || source_bytes > crate::linker::MAX_MODULE_GRAPH_SOURCE_BYTES
        {
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "K1004",
                DiagnosticPhase::Parse,
                format!(
                    "Kotodama test graph contains {source_count} sources/{source_bytes} bytes; V1 permits at most {} sources/{} bytes",
                    crate::linker::MAX_MODULE_GRAPH_SOURCES,
                    crate::linker::MAX_MODULE_GRAPH_SOURCE_BYTES
                ),
                source_start_span(Some(&target.source_name)),
            )));
        }

        let mut ordered_tests = test_modules.iter().collect::<Vec<_>>();
        ordered_tests.sort_by(|left, right| {
            normalize_logical_path(Path::new(&left.source_name))
                .cmp(&normalize_logical_path(Path::new(&right.source_name)))
                .then_with(|| left.source_name.cmp(&right.source_name))
        });
        let mut names = BTreeSet::new();
        if !names.insert(normalize_logical_path(Path::new(&target.source_name)))
            || ordered_tests
                .iter()
                .any(|source| !names.insert(normalize_logical_path(Path::new(&source.source_name))))
        {
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "E_DUPLICATE_SOURCE",
                DiagnosticPhase::Resolve,
                "the Kotodama test graph contains duplicate logical source paths",
                source_start_span(Some(&target.source_name)),
            )));
        }

        let units = std::iter::once(target)
            .chain(ordered_tests.iter().copied())
            .collect::<Vec<_>>();
        let keys = std::iter::once(format!(
            "target\0{}",
            normalize_logical_path(Path::new(&target.source_name)).display()
        ))
        .chain(ordered_tests.iter().map(|source| {
            format!(
                "test\0{}",
                normalize_logical_path(Path::new(&source.source_name)).display()
            )
        }))
        .collect::<Vec<_>>();
        let source_ids = crate::linker::stable_source_ids(&keys);
        let mut files = Vec::with_capacity(units.len());
        let mut parsed = Vec::with_capacity(units.len());
        for (unit, source_id) in units.iter().zip(source_ids) {
            enforce_source_budget(CompileRequest {
                source: &unit.source,
                source_name: Some(&unit.source_name),
            })?;
            let file = SourceFile::new(source_id, unit.source_name.clone(), &unit.source);
            let (program, _) = crate::parser::parse_source_spanned(&file, FrontendBudget::v1())?;
            files.push(file);
            parsed.push(program);
        }

        require_deployable_contract(&parsed[0].program, Some(&target.source_name))?;
        for (index, program) in parsed.iter().enumerate().skip(1) {
            validate_test_module_source(
                &program.program,
                Some(files[index].name()),
                &target.source_name,
            )?;
        }

        let target_resolved = crate::resolved::resolve(parsed.remove(0), &files[0])?;
        let target_semantic =
            crate::semantic::SemanticContext::with_capabilities(self.options.force_zk, true);
        let target_signatures = target_semantic
            .resolve_resolved_function_signatures(&target_resolved)
            .map_err(|error| semantic_error_diagnostic(error, Some(files[0].name())))?;
        let target_typed =
            target_semantic
                .analyze_resolved(&target_resolved)
                .map_err(|failures| {
                    crate::semantic_diagnostics::from_semantic_failures(
                        failures,
                        Some(files[0].name()),
                        Some(&files[0]),
                        Some(&target_resolved),
                    )
                })?;
        let external_states = target_typed
            .states
            .iter()
            .map(|state| (state.name.clone(), state.ty.clone()))
            .collect::<IndexMap<_, _>>();
        let target_environment =
            target_semantic.test_target_environment(target_signatures, external_states);
        let resolution_environment = crate::resolved::ExternalResolutionEnvironment {
            functions: target_environment.functions.keys().cloned().collect(),
            states: target_environment.states.keys().cloned().collect(),
            structs: target_environment.structs.keys().cloned().collect(),
            consts: target_environment.consts.keys().cloned().collect(),
            error_codes: target_environment
                .error_codes
                .iter()
                .map(|(name, code)| (name.clone(), *code))
                .collect(),
        };

        let mut resolved_modules = Vec::with_capacity(parsed.len());
        for (index, program) in parsed.into_iter().enumerate() {
            let file = &files[index + 1];
            resolved_modules.push(crate::resolved::resolve_with_external_environment(
                program,
                file,
                &resolution_environment,
            )?);
        }
        reject_duplicate_test_graph_symbols(
            std::iter::once((&target_resolved, &files[0]))
                .chain(resolved_modules.iter().zip(files.iter().skip(1))),
        )?;

        let runtime_typed = crate::semantic::project_test_target_to_production(
            target_typed.clone(),
            self.options.force_zk,
        )
        .map_err(|error| semantic_error_diagnostic(error, Some(&target.source_name)))?;
        let mut suite_typed = target_typed;
        let mut error_codes = suite_typed
            .error_codes
            .iter()
            .map(|error| error.code)
            .collect::<BTreeSet<_>>();
        for (index, resolved) in resolved_modules.iter().enumerate() {
            let file = &files[index + 1];
            let semantic =
                crate::semantic::SemanticContext::with_capabilities(self.options.force_zk, true);
            let mut typed = semantic
                .analyze_resolved_with_test_target(resolved, &target_environment)
                .map_err(|failures| {
                    crate::semantic_diagnostics::from_semantic_failures(
                        failures,
                        Some(file.name()),
                        Some(file),
                        Some(resolved),
                    )
                })?;
            for error in &typed.error_codes {
                if !error_codes.insert(error.code) {
                    return Err(DiagnosticBundle::single(Diagnostic::error(
                        "E_DUPLICATE_ERROR_CODE",
                        DiagnosticPhase::Semantic,
                        format!(
                            "test graph assigns duplicate seiyaku error code {}",
                            error.code
                        ),
                        source_start_span(Some(file.name())),
                    )));
                }
            }
            merge_source_files(&mut suite_typed, &mut typed, file.name())?;
            suite_typed.items.append(&mut typed.items);
            suite_typed.error_codes.append(&mut typed.error_codes);
            suite_typed
                .message_entries
                .append(&mut typed.message_entries);
        }
        crate::semantic::validate_linked_program(&suite_typed, self.options.force_zk)
            .map_err(|error| semantic_error_diagnostic(error, Some(&target.source_name)))?;

        let suite = self.build_typed_program(suite_typed, Some(&target.source_name))?;
        let has_runtime_entrypoint = runtime_typed.items.iter().any(|item| {
            let crate::semantic::TypedItem::Function(function) = item;
            function.modifiers.kind != FunctionKind::Private
        });
        let runtime = if has_runtime_entrypoint {
            let mut runtime_options = self.options.clone();
            runtime_options.mode = CompilerMode::Production;
            Some(
                CompilerSession::new(runtime_options)
                    .build_typed_program(runtime_typed, Some(&target.source_name))?,
            )
        } else {
            None
        };
        Ok(TestCompileOutput { suite, runtime })
    }

    /// Compile a fully resolved and linked typed-HIR program inside the trusted driver.
    ///
    /// This is the only post-link code-generation entry point. It deliberately
    /// accepts no AST, so package managers cannot reintroduce source rewriting
    /// after module type/effect analysis.
    pub(crate) fn build_typed_program(
        &self,
        program: TypedProgram,
        source_name: Option<&str>,
    ) -> Result<CompileOutput, DiagnosticBundle> {
        if program.unit.kind != SourceUnitKind::Seiyaku {
            return Err(non_deployable_module_diagnostic(source_name));
        }
        let compiler = Compiler::new_with_options(self.options.clone());
        compiler.compile_typed_program_with_manifest_and_report_diagnostics(program, source_name)
    }
}

fn source_range_span(source: &SourceFile, range: crate::source::SourceRange) -> Option<SourceSpan> {
    (source.id() == range.source).then(|| SourceSpan::from_range(source, range.range))
}

fn enforce_argument_register_window(
    program: &TypedProgram,
    source: &SourceFile,
    resolved: &crate::resolved::ResolvedProgram,
) -> Result<(), DiagnosticBundle> {
    let limit = crate::regalloc::MAX_ARGUMENT_VALUES;
    let mut diagnostics = Vec::new();
    for item in &program.items {
        let crate::semantic::TypedItem::Function(function) = item;
        let counts = function
            .param_types
            .iter()
            .map(|parameter| {
                crate::semantic::runtime_value_word_count(&parameter.ty).ok_or_else(|| {
                    let primary_span = resolved
                        .parameter_name_source(&function.name, &parameter.name)
                        .and_then(|range| source_range_span(source, range));
                    DiagnosticBundle::single(Diagnostic::error(
                        "K2099",
                        DiagnosticPhase::Semantic,
                        format!(
                            "parameter `{}` of function `{}` retained an unresolved ABI type",
                            parameter.name, function.name
                        ),
                        primary_span,
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let total_words = counts
            .iter()
            .try_fold(0_usize, |total, words| total.checked_add(*words))
            .unwrap_or(usize::MAX);
        if function.param_types.len() <= limit && total_words <= limit {
            continue;
        }

        let mut cumulative_words = 0_usize;
        let crossing_index = counts
            .iter()
            .enumerate()
            .find_map(|(index, words)| {
                cumulative_words = cumulative_words.saturating_add(*words);
                (index >= limit || cumulative_words > limit).then_some(index)
            })
            .unwrap_or_else(|| function.param_types.len().saturating_sub(1));
        let parameter = &function.param_types[crossing_index];
        let primary_span = resolved
            .parameter_name_source(&function.name, &parameter.name)
            .and_then(|range| source_range_span(source, range));
        let mut diagnostic = Diagnostic::error(
            "K2007",
            DiagnosticPhase::Semantic,
            format!(
                "function `{}` declares {} source parameter(s) requiring {total_words} flattened argument words; V1 permits at most {limit}",
                function.name,
                function.param_types.len(),
            ),
            primary_span,
        );
        if let Some(range) = function.name_source
            && let Some(span) = source_range_span(source, range)
        {
            diagnostic.labels.push(DiagnosticLabel {
                span,
                message: "function exceeds the V1 argument-register window".to_owned(),
            });
        }
        diagnostics.push(diagnostic);
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(DiagnosticBundle::new(diagnostics))
    }
}

fn semantic_error_diagnostic(
    error: crate::semantic::SemanticError,
    source_name: Option<&str>,
) -> DiagnosticBundle {
    DiagnosticBundle::single(Diagnostic::error(
        error.code,
        DiagnosticPhase::Semantic,
        error.message,
        source_start_span(source_name),
    ))
}

fn validate_test_module_source(
    program: &Program,
    source_name: Option<&str>,
    target_source_name: &str,
) -> Result<(), DiagnosticBundle> {
    if program.unit.kind != SourceUnitKind::Module {
        return Err(DiagnosticBundle::single(Diagnostic::error(
            "E_TEST_MODULE_KIND",
            DiagnosticPhase::Semantic,
            "standalone Kotodama tests must declare a `module` source unit",
            source_start_span(source_name),
        )));
    }
    let Some(test_target) = program.test_target.as_ref() else {
        return Err(DiagnosticBundle::single(Diagnostic::error(
            "E_TEST_TARGET_REQUIRED",
            DiagnosticPhase::Semantic,
            "standalone Kotodama tests require a `koto_test { target: \"...\" }` declaration",
            source_start_span(source_name),
        )));
    };
    let test_source_name = source_name.unwrap_or("<test>");
    let declared_target = Path::new(test_source_name)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(&test_target.target);
    if normalize_logical_path(&declared_target)
        != normalize_logical_path(Path::new(target_source_name))
    {
        return Err(DiagnosticBundle::single(Diagnostic::error(
            "E_TEST_TARGET_MISMATCH",
            DiagnosticPhase::Resolve,
            format!(
                "standalone test target `{}` does not resolve to graph target `{target_source_name}`",
                test_target.target
            ),
            source_start_span(source_name),
        )));
    }
    for item in &program.items {
        let invalid = match item {
            Item::State(_) => Some("durable state declaration"),
            Item::Trigger(_) => Some("trigger declaration"),
            Item::Function(function)
                if function.modifiers.kind != crate::ast::FunctionKind::Private =>
            {
                Some("public or lifecycle function")
            }
            Item::Function(_) | Item::Struct(_) | Item::ErrorEnum(_) | Item::Const(_) => None,
        };
        if let Some(invalid) = invalid {
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "E_TEST_MODULE_ITEM",
                DiagnosticPhase::Semantic,
                format!("standalone Kotodama test module contains {invalid}"),
                source_start_span(source_name),
            )));
        }
    }
    Ok(())
}

fn normalize_logical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

fn reject_duplicate_test_graph_symbols<'program>(
    units: impl IntoIterator<
        Item = (
            &'program crate::resolved::ResolvedProgram,
            &'program SourceFile,
        ),
    >,
) -> Result<(), DiagnosticBundle> {
    let units = units.into_iter().collect::<Vec<_>>();
    let files = units
        .iter()
        .map(|(_, file)| (file.id(), *file))
        .collect::<BTreeMap<_, _>>();
    let mut declarations = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for (program, file) in units {
        for symbol in program.symbols() {
            if let Some(previous) = declarations.insert(symbol.name.clone(), symbol.source) {
                let mut diagnostic = Diagnostic::error(
                    "E_DUPLICATE_DECLARATION",
                    DiagnosticPhase::Resolve,
                    format!(
                        "declaration name `{}` is already used by another test-graph source",
                        symbol.name
                    ),
                    Some(SourceSpan::from_range(file, symbol.source.range)),
                );
                if let Some(previous_file) = files.get(&previous.source) {
                    diagnostic.labels.push(DiagnosticLabel {
                        span: SourceSpan::from_range(previous_file, previous.range),
                        message: "first graph declaration is here".to_owned(),
                    });
                }
                diagnostics.push(diagnostic);
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(DiagnosticBundle::new(diagnostics))
    }
}

fn merge_source_files(
    target: &mut TypedProgram,
    source: &mut TypedProgram,
    source_name: &str,
) -> Result<(), DiagnosticBundle> {
    for (id, node) in std::mem::take(&mut source.hir_nodes) {
        if target.hir_nodes.insert(id, node).is_some() {
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "E_DUPLICATE_HIR_ID",
                DiagnosticPhase::Resolve,
                format!(
                    "typed module graph reused HIR identity {}:{}",
                    id.source.0, id.local.0
                ),
                source_start_span(Some(source_name)),
            )));
        }
    }
    for (source_id, file) in std::mem::take(&mut source.source_files) {
        if let Some(previous) = target.source_files.insert(source_id, file.clone())
            && previous != file
        {
            return Err(DiagnosticBundle::single(Diagnostic::error(
                "E_DUPLICATE_SOURCE_ID",
                DiagnosticPhase::Resolve,
                format!(
                    "compiler assigned SourceId {} to both `{}` and `{}`",
                    source_id.0,
                    previous.name(),
                    file.name()
                ),
                source_start_span(Some(source_name)),
            )));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ProductionTestSurface {
    TestTarget,
    Fixture,
    TestFunction,
}

impl ProductionTestSurface {
    fn message(self) -> &'static str {
        match self {
            Self::TestTarget => {
                "`koto_test` target declarations require explicit compiler test mode"
            }
            Self::Fixture => "fixture declarations require explicit compiler test mode",
            Self::TestFunction => "`#[test]` functions require explicit compiler test mode",
        }
    }
}

fn reject_production_test_surface(
    mode: CompilerMode,
    program: &Program,
    source_name: Option<&str>,
    source_tokens: Option<(&SourceFile, &[Token])>,
) -> Result<(), DiagnosticBundle> {
    if mode == CompilerMode::Test {
        return Ok(());
    }

    let has_test_target = program.test_target.is_some();
    let has_fixture = !program.fixtures.is_empty();
    let first_test_function = program.items.iter().find_map(|item| match item {
        crate::ast::Item::Function(function)
            if function.modifiers.is_test || function.modifiers.test_fixture.is_some() =>
        {
            Some(function)
        }
        _ => None,
    });
    if !has_test_target && !has_fixture && first_test_function.is_none() {
        return Ok(());
    }

    let exact = source_tokens.and_then(|(source, tokens)| {
        first_test_surface_token(
            tokens,
            has_test_target,
            has_fixture,
            first_test_function.is_some(),
        )
        .map(|(surface, range)| (surface, Some(SourceSpan::from_range(source, range))))
    });
    let (surface, span) = exact.unwrap_or_else(|| {
        if has_test_target {
            (
                ProductionTestSurface::TestTarget,
                source_start_span(source_name),
            )
        } else if has_fixture {
            (
                ProductionTestSurface::Fixture,
                source_start_span(source_name),
            )
        } else {
            let function = first_test_function.expect("a test surface was found");
            (
                ProductionTestSurface::TestFunction,
                Some(SourceSpan {
                    source: source_name.map(ToOwned::to_owned),
                    start: SourcePosition {
                        line: function.location.line,
                        column: function.location.column,
                    },
                    end: SourcePosition {
                        line: function.location.line,
                        column: function.location.column.saturating_add(function.name.len()),
                    },
                    byte_range: None,
                }),
            )
        }
    });
    Err(DiagnosticBundle::single(Diagnostic::error(
        "E_TEST_ONLY_PRODUCTION",
        DiagnosticPhase::Semantic,
        surface.message(),
        span,
    )))
}

fn first_test_surface_token(
    tokens: &[Token],
    has_test_target: bool,
    has_fixture: bool,
    has_test_function: bool,
) -> Option<(ProductionTestSurface, TextRange)> {
    let mut brace_depth = 0_usize;
    for (index, token) in tokens.iter().enumerate() {
        if brace_depth == 1 {
            if has_test_function
                && matches!(token.kind, TokenKind::Hash)
                && matches!(
                    tokens.get(index + 1).map(|token| &token.kind),
                    Some(TokenKind::LBracket)
                )
                && matches!(
                    tokens.get(index + 2).map(|token| &token.kind),
                    Some(TokenKind::Ident(name)) if name == "test"
                )
            {
                let end = tokens[index + 2..]
                    .iter()
                    .find(|token| matches!(token.kind, TokenKind::RBracket))
                    .map_or(tokens[index + 2].range.end, |token| token.range.end);
                return Some((
                    ProductionTestSurface::TestFunction,
                    TextRange::new(token.range.start, end),
                ));
            }
            if has_test_target
                && matches!(&token.kind, TokenKind::Ident(name) if name == "koto_test")
                && matches!(
                    tokens.get(index + 1).map(|token| &token.kind),
                    Some(TokenKind::LBrace)
                )
            {
                return Some((ProductionTestSurface::TestTarget, token.range));
            }
            if has_fixture
                && matches!(&token.kind, TokenKind::Ident(name) if name == "fixture")
                && matches!(
                    tokens.get(index + 1).map(|token| &token.kind),
                    Some(TokenKind::Ident(_))
                )
                && matches!(
                    tokens.get(index + 2).map(|token| &token.kind),
                    Some(TokenKind::LBrace)
                )
            {
                return Some((ProductionTestSurface::Fixture, token.range));
            }
        }
        match token.kind {
            TokenKind::LBrace => brace_depth = brace_depth.saturating_add(1),
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn require_deployable_contract(
    program: &Program,
    source_name: Option<&str>,
) -> Result<(), DiagnosticBundle> {
    if program.unit.kind == SourceUnitKind::Seiyaku {
        Ok(())
    } else {
        Err(non_deployable_module_diagnostic(source_name))
    }
}

fn non_deployable_module_diagnostic(source_name: Option<&str>) -> DiagnosticBundle {
    let mut diagnostic = Diagnostic::error(
        "K4003",
        DiagnosticPhase::Artifact,
        "a reusable module cannot be emitted as a deployable .to artifact",
        source_start_span(source_name),
    );
    diagnostic.help =
        Some("link the module into exactly one seiyaku root, then build that seiyaku".to_owned());
    DiagnosticBundle::single(diagnostic)
}

fn source_start_span(source_name: Option<&str>) -> Option<SourceSpan> {
    Some(SourceSpan {
        source: source_name.map(ToOwned::to_owned),
        start: SourcePosition { line: 1, column: 1 },
        end: SourcePosition { line: 1, column: 2 },
        byte_range: None,
    })
}

fn enforce_source_budget(request: CompileRequest<'_>) -> Result<(), DiagnosticBundle> {
    if request.source.len() <= MAX_SOURCE_BYTES {
        return Ok(());
    }
    let mut diagnostic = Diagnostic::error(
        "K0001",
        DiagnosticPhase::Lex,
        format!(
            "source contains {} bytes and exceeds the {MAX_SOURCE_BYTES}-byte Kotodama V1 limit",
            request.source.len()
        ),
        source_start_span(request.source_name),
    );
    diagnostic.help = Some("split reusable code into typed modules and import it".to_owned());
    Err(DiagnosticBundle::single(diagnostic))
}

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new(CompilerOptions::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_returns_structured_success_and_failure() {
        let session = CompilerSession::default();
        let output = session
            .build(CompileRequest {
                source: "seiyaku Demo { view fn ping() -> int { return 1; } }",
                source_name: Some("demo.ko"),
            })
            .expect("compile canonical source");
        assert!(!output.artifact.is_empty());
        assert!(output.manifest.code_hash.is_some());
        assert!(
            output
                .report
                .source_map
                .iter()
                .all(|entry| entry.source.source_path.as_deref() == Some("demo.ko"))
        );

        let error = session
            .build(CompileRequest {
                source: "not a seiyaku",
                source_name: Some("broken.ko"),
            })
            .expect_err("invalid source must return diagnostics");
        assert_eq!(error.diagnostics.len(), 1);
    }

    #[test]
    fn check_drops_a_boundary_depth_program_iteratively() {
        let mut ty = String::from("int");
        for _ in 0..crate::source::MAX_NESTING_DEPTH - 2 {
            ty = format!("Option<{ty}>");
        }
        let source = format!("module Deep {{ struct Wrapper {{ {ty} value }} }}");
        let request = CompileRequest {
            source: &source,
            source_name: Some("deep.ko"),
        };
        let session = CompilerSession::default();
        session
            .check(request)
            .expect("the inclusive frontend nesting boundary must be stack-safe");
        session
            .check_with_lints(CompileRequest {
                source: &source,
                source_name: Some("deep.ko"),
            })
            .expect("lint cleanup at the nesting boundary must be stack-safe");

        let invalid = source.replace("int", "UnknownType");
        session
            .check(CompileRequest {
                source: &invalid,
                source_name: Some("deep-invalid.ko"),
            })
            .expect_err("semantic-error cleanup at the nesting boundary must be stack-safe");
    }

    #[test]
    fn session_rejects_oversized_input_before_constructing_a_source_database() {
        let source = " ".repeat(MAX_SOURCE_BYTES + 1);
        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source: &source,
                source_name: Some("oversized.ko"),
            })
            .expect_err("oversized source must fail before parsing");
        assert_eq!(diagnostics.diagnostics.len(), 1);
        let diagnostic = &diagnostics.diagnostics[0];
        assert_eq!(diagnostic.code, "K0001");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Lex);
        assert_eq!(
            diagnostic
                .primary_span
                .as_ref()
                .and_then(|span| span.source.as_deref()),
            Some("oversized.ko")
        );
    }

    #[test]
    fn session_rejects_aggregate_arguments_over_the_register_word_limit() {
        let source = r#"seiyaku WideCall {
  struct Wide {
    int f00, int f01, int f02, int f03, int f04, int f05, int f06,
    int f07, int f08, int f09, int f10, int f11, int f12, int f13
  }
  view fn inspect(Wide value) -> int { return value.f00; }
}"#;
        let session = CompilerSession::default();
        let request = CompileRequest {
            source,
            source_name: Some("wide-call.ko"),
        };
        let checked = session
            .check(request)
            .expect_err("oversized flattened argument ABI must fail semantic checking");
        let diagnostics = session
            .build(CompileRequest {
                source,
                source_name: Some("wide-call.ko"),
            })
            .expect_err("oversized flattened argument ABI must fail before lowering");
        assert_eq!(checked, diagnostics);
        let diagnostic = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K2007")
            .expect("stable argument-limit diagnostic");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
        let span = diagnostic.primary_span.as_ref().expect("parameter span");
        assert_eq!(span.source.as_deref(), Some("wide-call.ko"));
        let range = span.byte_range.expect("parameter byte range");
        assert_eq!(
            &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
            "value"
        );

        let nested = r#"seiyaku NestedWideCall {
  struct Inner { int a, int b, int c, int d, int e, int f, int g }
  struct Outer { Inner left, Inner right }
  view fn inspect_nested(Outer payload) -> int { return payload.left.a; }
}"#;
        let nested_error = session
            .check(CompileRequest {
                source: nested,
                source_name: Some("nested-wide-call.ko"),
            })
            .expect_err("nested aggregate must use the same recursive ABI word accounting");
        let nested_diagnostic = nested_error
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K2007")
            .expect("nested aggregate argument-limit diagnostic");
        let nested_range = nested_diagnostic
            .primary_span
            .as_ref()
            .and_then(|span| span.byte_range)
            .expect("nested parameter range");
        assert_eq!(
            &nested[nested_range.start as usize..nested_range.end as usize],
            "payload"
        );

        let at_limit = source.replace("int f13", "");
        session
            .build(CompileRequest {
                source: &at_limit,
                source_name: Some("wide-call-at-limit.ko"),
            })
            .expect("session preflight and lowering must admit exactly thirteen words");
    }

    #[test]
    fn modules_can_be_checked_but_never_emitted_directly() {
        let session = CompilerSession::default();
        let request = CompileRequest {
            source: "module Math { fn add(int left, int right) -> int { return left + right; } }",
            source_name: Some("math.ko"),
        };
        session.check(request).expect("standalone module check");
        let diagnostics = session
            .build(request)
            .expect_err("a module must not produce deployable bytecode");
        assert_eq!(diagnostics.diagnostics.len(), 1);
        assert_eq!(diagnostics.diagnostics[0].code, "K4003");
        assert_eq!(diagnostics.diagnostics[0].phase, DiagnosticPhase::Artifact);
    }

    #[test]
    fn check_and_build_return_identical_resolution_records_for_contracts_and_modules() {
        let session = CompilerSession::default();
        for source in [
            r#"seiyaku Broken {
                fn first(Missing value) -> int { return unknown; }
                fn second(bool flag) { flag = false; missing_call(); }
            }"#,
            r#"module Broken {
                fn first(Missing value) -> int { return unknown; }
                fn second(bool flag) { flag = false; missing_call(); }
            }"#,
        ] {
            let request = CompileRequest {
                source,
                source_name: Some("broken.ko"),
            };
            let checked = session
                .check(request)
                .expect_err("check must reject source");
            let built = session
                .build(request)
                .expect_err("build must reject source");
            assert_eq!(checked, built);
            assert_eq!(checked.diagnostics.len(), 3, "{checked:?}");
            assert_eq!(
                checked
                    .diagnostics
                    .iter()
                    .map(|diagnostic| (diagnostic.code.as_str(), diagnostic.phase))
                    .collect::<Vec<_>>(),
                vec![
                    ("K2002", DiagnosticPhase::Resolve),
                    ("K2002", DiagnosticPhase::Resolve),
                    ("K2002", DiagnosticPhase::Resolve),
                ]
            );
            assert_eq!(
                checked
                    .diagnostics
                    .iter()
                    .map(|diagnostic| {
                        let range = diagnostic
                            .primary_span
                            .as_ref()
                            .and_then(|span| span.byte_range)
                            .expect("resolved diagnostic range");
                        &source[range.start as usize..range.end as usize]
                    })
                    .collect::<Vec<_>>(),
                vec!["Missing", "unknown", "missing_call"]
            );
            assert!(checked.diagnostics.iter().all(|diagnostic| {
                diagnostic
                    .primary_span
                    .as_ref()
                    .is_some_and(|span| span.byte_range.is_some())
            }));
        }
    }

    #[test]
    fn resolution_failures_are_collected_with_stable_source_spans() {
        let source = "seiyaku Broken {\nfn first() -> int { return missing_first; }\nfn second() -> int { return missing_second; }\n}";
        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("multi-error.ko"),
            })
            .expect_err("both invalid function bodies must be diagnosed");

        assert_eq!(diagnostics.diagnostics.len(), 2);
        for ((diagnostic, expected_line), expected_name) in diagnostics
            .diagnostics
            .iter()
            .zip([2, 3])
            .zip(["missing_first", "missing_second"])
        {
            assert_eq!(diagnostic.phase, DiagnosticPhase::Resolve);
            assert_eq!(diagnostic.code, "K2002");
            let span = diagnostic.primary_span.as_ref().expect("function span");
            assert_eq!(span.source.as_deref(), Some("multi-error.ko"));
            assert_eq!(span.start.line, expected_line);
            let range = span.byte_range.expect("exact UTF-8 byte range");
            assert_eq!(
                &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
                expected_name
            );
        }

        let repeated = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("multi-error.ko"),
            })
            .expect_err("repeated invalid compilation must fail identically");
        assert_eq!(diagnostics, repeated);
    }

    #[test]
    fn compiler_session_preserves_explicit_semantic_code_and_plain_message() {
        let source = "seiyaku Broken { fn missing_context() { let values = []; } }";
        let diagnostics = CompilerSession::default()
            .check(CompileRequest {
                source,
                source_name: Some("explicit-semantic-code.ko"),
            })
            .expect_err("an empty List without a type context must fail");
        let [diagnostic] = diagnostics.diagnostics.as_slice() else {
            panic!("expected one semantic diagnostic: {diagnostics:?}");
        };
        assert_eq!(diagnostic.code, "E_LIST_EMPTY_CONTEXT");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
        assert_eq!(
            diagnostic.message,
            "an empty list requires an exact `List<T, N>` type context"
        );
        assert!(!diagnostic.message.contains("E_LIST_EMPTY_CONTEXT"));

        let human = diagnostics.render_human();
        let json = diagnostics.render_json().expect("JSON diagnostics");
        let sarif = diagnostics.render_sarif().expect("SARIF diagnostics");
        for rendered in [&human, &json, &sarif] {
            assert!(rendered.contains("E_LIST_EMPTY_CONTEXT"));
            assert!(rendered.contains(&diagnostic.message));
        }
    }

    #[test]
    fn secret_signature_and_state_rejections_have_exact_type_spans() {
        let session = CompilerSession::new(CompilerOptions {
            force_zk: true,
            ..CompilerOptions::default()
        });
        for (source, code) in [
            (
                "seiyaku Privacy { state Secret<int> hidden; }",
                "E_SECRET_STATE_TYPE",
            ),
            (
                "seiyaku Privacy { kotoage fn leak(Secret<int> value) authorize(\"Leak\") {} }",
                "E_SECRET_PUBLIC_PARAMETER",
            ),
            (
                "seiyaku Privacy { kotoage fn leak() -> Secret<int> authorize(\"Leak\") { return crypto::private_input(0); } }",
                "E_SECRET_PUBLIC_RETURN",
            ),
        ] {
            let diagnostics = session
                .check(CompileRequest {
                    source,
                    source_name: Some("secret-span.ko"),
                })
                .expect_err("public or durable Secret<T> use must fail");
            let diagnostic = diagnostics
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == code)
                .unwrap_or_else(|| panic!("missing {code}: {diagnostics:?}"));
            assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
            assert!(!diagnostic.message.contains(code));
            let range = diagnostic
                .primary_span
                .as_ref()
                .and_then(|span| span.byte_range)
                .expect("security diagnostics retain an exact type span");
            assert_eq!(
                &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
                "Secret<int>"
            );
        }
    }

    #[test]
    fn v1_retired_and_unsafe_diagnostics_have_exact_ranges_and_safe_fixes() {
        fn reject(source: &str) -> DiagnosticBundle {
            CompilerSession::default()
                .check(CompileRequest {
                    source,
                    source_name: Some("v1-diagnostic.ko"),
                })
                .expect_err("fixture must be rejected")
        }

        fn diagnostic<'a>(bundle: &'a DiagnosticBundle, code: &str) -> &'a Diagnostic {
            bundle
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == code)
                .unwrap_or_else(|| panic!("missing {code}: {bundle:#?}"))
        }

        fn primary_text<'a>(source: &'a str, diagnostic: &Diagnostic) -> &'a str {
            let range = diagnostic
                .primary_span
                .as_ref()
                .and_then(|span| span.byte_range)
                .expect("exact primary byte range");
            &source[range.start as usize..range.end as usize]
        }

        fn fix_text<'source, 'diagnostic>(
            source: &'source str,
            diagnostic: &'diagnostic Diagnostic,
        ) -> (&'source str, &'diagnostic str) {
            let fix = diagnostic.fix.as_ref().expect("machine-applicable fix");
            let range = fix.span.byte_range.expect("exact fix byte range");
            (
                &source[range.start as usize..range.end as usize],
                &fix.replacement,
            )
        }

        let positional =
            "seiyaku C { struct Pair { int left, int right } fn f() { let pair = Pair(1, 2); } }";
        let positional_error = reject(positional);
        let positional_diagnostic = diagnostic(&positional_error, "E_POSITIONAL_STRUCT");
        assert_eq!(
            primary_text(positional, positional_diagnostic),
            "Pair(1, 2)"
        );
        assert_eq!(
            fix_text(positional, positional_diagnostic),
            ("Pair(1, 2)", "Pair { left: 1, right: 2, }")
        );

        let mixed =
            "seiyaku C { fn target(int first, int second) {} fn f() { target(1, second: 2); } }";
        let mixed_error = reject(mixed);
        let mixed_diagnostic = diagnostic(&mixed_error, "E_MIXED_CALL_ARGUMENTS");
        assert_eq!(primary_text(mixed, mixed_diagnostic), "1");
        assert_eq!(fix_text(mixed, mixed_diagnostic), ("1", "first: 1"));

        let unresolved_mixed = "seiyaku C { fn f() { target(1, second: 2); } }";
        let unresolved_error = reject(unresolved_mixed);
        let unresolved_diagnostic = diagnostic(&unresolved_error, "E_MIXED_CALL_ARGUMENTS");
        assert_eq!(
            primary_text(unresolved_mixed, unresolved_diagnostic),
            "second"
        );
        assert!(unresolved_diagnostic.fix.is_none());

        let unsafe_read =
            "seiyaku C { fn read(List<int, 2> values) -> Option<int> { return values[0]; } }";
        let read_error = reject(unsafe_read);
        let read_diagnostic = diagnostic(&read_error, "E_LIST_UNSAFE_INDEX");
        assert_eq!(primary_text(unsafe_read, read_diagnostic), "values[0]");
        assert_eq!(
            fix_text(unsafe_read, read_diagnostic),
            ("values[0]", "values.get(0)")
        );

        let mistyped_read = "seiyaku C { fn read(List<int, 2> values) -> Option<int> { return values[\"zero\"]; } }";
        let mistyped_read_error = reject(mistyped_read);
        assert!(
            diagnostic(&mistyped_read_error, "E_LIST_UNSAFE_INDEX")
                .fix
                .is_none(),
            "a List.get recipe with a non-int index would not type-check"
        );

        let unsafe_write =
            "seiyaku C { fn write() { var List<int, 2> values = [1]; values[0] = 2; } }";
        let write_error = reject(unsafe_write);
        let write_diagnostic = diagnostic(&write_error, "E_LIST_UNSAFE_INDEX");
        assert_eq!(
            primary_text(unsafe_write, write_diagnostic),
            "values[0] = 2;"
        );
        assert_eq!(
            fix_text(unsafe_write, write_diagnostic),
            ("values[0] = 2;", "values.try_set(index: 0, value: 2);",)
        );

        for unsafe_without_recipe in [
            "seiyaku C { fn write() { let List<int, 2> values = [1]; values[0] = 2; } }",
            "seiyaku C { fn write() { var List<int, 2> values = [1]; values[\"zero\"] = 2; } }",
            "seiyaku C { fn write() { var List<int, 2> values = [1]; values[0] = \"two\"; } }",
            "seiyaku C { fn write() { var List<int, 2> values = [1]; values[0] += 2; } }",
            "seiyaku C { fn write() { var List<int, 2> values = [1]; values[0] /* keep */ = 2; } }",
        ] {
            let error = reject(unsafe_without_recipe);
            let diagnostic = diagnostic(&error, "E_LIST_UNSAFE_INDEX");
            assert!(
                diagnostic.fix.is_none(),
                "unsafe, non-compiling, compound, or trivia-moving rewrite must fail closed: {diagnostic:#?}"
            );
        }

        let legacy_sum = "seiyaku C { fn f() -> Option<int> { option::none(0) } }";
        let legacy_error = reject(legacy_sum);
        let legacy_diagnostic = diagnostic(&legacy_error, "E_LEGACY_SUM_CONSTRUCTOR");
        assert_eq!(
            primary_text(legacy_sum, legacy_diagnostic),
            "option::none(0)"
        );
        assert_eq!(
            fix_text(legacy_sum, legacy_diagnostic),
            ("option::none(0)", "Option::none")
        );

        let query_key = "seiyaku C { view fn account(bytes raw) { let account_view = ledger::query::account(raw); } }";
        let query_key_error = reject(query_key);
        let query_key_diagnostic = diagnostic(&query_key_error, "E_QUERY_KEY_TYPE");
        assert_eq!(
            primary_text(query_key, query_key_diagnostic),
            "ledger::query::account(raw)"
        );
        assert!(query_key_diagnostic.fix.is_none());

        let query_result = "seiyaku C { view fn account(AccountId id) { let bytes raw = ledger::query::account(id); } }";
        let query_result_error = reject(query_result);
        let query_result_diagnostic = diagnostic(&query_result_error, "E_QUERY_RESULT_TYPE");
        assert_eq!(primary_text(query_result, query_result_diagnostic), "bytes");
        assert_eq!(
            fix_text(query_result, query_result_diagnostic),
            ("bytes", "Option<AccountView>")
        );

        let comprehension = "seiyaku C { fn copy() { let List<int, 8> source = [1]; let List<int, 4> result = [item for item in source]; } }";
        let comprehension_error = reject(comprehension);
        let comprehension_diagnostic =
            diagnostic(&comprehension_error, "E_LIST_COMPREHENSION_CAPACITY");
        assert_eq!(
            primary_text(comprehension, comprehension_diagnostic),
            "List<int, 4>"
        );
        assert_eq!(
            fix_text(comprehension, comprehension_diagnostic),
            ("List<int, 4>", "List<int, 8>")
        );

        let retired_suffix = "seiyaku C { fn quantity_value() -> quantity { 1.25amt } }";
        let retired_error = reject(retired_suffix);
        let retired_diagnostic = diagnostic(&retired_error, "E_RETIRED_NUMERIC_SUFFIX");
        assert_eq!(primary_text(retired_suffix, retired_diagnostic), "1.25amt");
        assert!(
            retired_diagnostic.fix.is_none(),
            "first-release V1 diagnostics must reject retired suffixes without compatibility rewrites"
        );

        let scale_29 = format!("0.{}1", "0".repeat(28));
        let invalid_quantity =
            format!("seiyaku C {{ fn quantity_value() -> quantity {{ return {scale_29}; }} }}");
        let quantity_error = reject(&invalid_quantity);
        let quantity_diagnostic = diagnostic(&quantity_error, "E_DECIMAL_SCALE_OVERFLOW");
        assert_eq!(
            primary_text(&invalid_quantity, quantity_diagnostic),
            scale_29
        );
        assert!(quantity_diagnostic.fix.is_none());
    }

    #[test]
    fn parser_recovers_independent_items_into_one_spanned_bundle() {
        let source = "seiyaku Broken {\nfn first() { let int value = ; }\nfn second() { let bool value = ; }\n}";
        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("parse-errors.ko"),
            })
            .expect_err("both malformed function bodies must be diagnosed");

        assert_eq!(diagnostics.diagnostics.len(), 2);
        for (diagnostic, expected_line) in diagnostics.diagnostics.iter().zip([2, 3]) {
            assert_eq!(diagnostic.code, "K1001");
            assert_eq!(diagnostic.phase, DiagnosticPhase::Parse);
            let span = diagnostic.primary_span.as_ref().expect("parse error span");
            assert_eq!(span.source.as_deref(), Some("parse-errors.ko"));
            assert_eq!(span.start.line, expected_line);
        }

        let human = diagnostics.render_human();
        let json = diagnostics.render_json().expect("render JSON diagnostics");
        let sarif = diagnostics
            .render_sarif()
            .expect("render SARIF diagnostics");
        for field in ["K1001", "parse", "parse-errors.ko"] {
            assert!(human.contains(field), "human renderer omitted {field}");
            assert!(json.contains(field), "JSON renderer omitted {field}");
            assert!(sarif.contains(field), "SARIF renderer omitted {field}");
        }
        assert!(
            sarif.contains("\"kotodama\""),
            "SARIF must embed the canonical diagnostic record"
        );
    }

    #[test]
    fn parser_spans_cover_the_complete_unexpected_token() {
        let source = "seiyaku Broken {\nfn first() { let int value = ; }\nfn second() { let bool value = ; }\n}";
        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("token-spans.ko"),
            })
            .expect_err("malformed expressions must fail");

        assert_eq!(diagnostics.diagnostics.len(), 2);
        for (diagnostic, line_number) in diagnostics.diagnostics.iter().zip([2, 3]) {
            let line = source.lines().nth(line_number - 1).expect("fixture line");
            let semicolon_column = line.find(';').expect("malformed semicolon") + 1;
            let span = diagnostic.primary_span.as_ref().expect("parse span");
            assert_eq!(span.source.as_deref(), Some("token-spans.ko"));
            assert_eq!((span.start.line, span.end.line), (line_number, line_number));
            assert_eq!(span.start.column, semicolon_column);
            assert_eq!(span.end.column, semicolon_column + 1);
        }
    }

    #[test]
    fn retired_english_declarations_and_unregistered_unicode_have_precise_spans() {
        for (retired, source) in [
            ("contract", "contract Demo {}"),
            (
                "entry",
                "seiyaku Demo { entry fn run() authorize(\"Run\") {} }",
            ),
            ("init", "seiyaku Demo { init() {} }"),
            ("upgrade", "seiyaku Demo { upgrade() {} }"),
        ] {
            let diagnostics = CompilerSession::default()
                .build(CompileRequest {
                    source,
                    source_name: Some("alias.ko"),
                })
                .expect_err("English declaration words are not V1 keywords");
            let diagnostic = diagnostics
                .diagnostics
                .first()
                .expect("retired declaration diagnostic");
            assert_eq!(diagnostic.phase, crate::diagnostic::DiagnosticPhase::Parse);
            let span = diagnostic.primary_span.as_ref().expect("declaration span");
            let start = source.find(retired).expect("declaration offset") + 1;
            assert_eq!(span.start.column, start, "{retired}");
            assert_eq!(span.end.column, start + retired.len(), "{retired}");
        }

        let unicode_source = "seiyaku Demo { fn 利用者() {} }";
        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source: unicode_source,
                source_name: Some("unicode.ko"),
            })
            .expect_err("general Unicode identifiers are forbidden");
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.phase == crate::diagnostic::DiagnosticPhase::Lex)
        );
        let first = diagnostics.diagnostics.first().expect("Unicode diagnostic");
        let span = first.primary_span.as_ref().expect("Unicode token span");
        assert_eq!(span.source.as_deref(), Some("unicode.ko"));
        let identifier = "利用者";
        let byte_start = unicode_source
            .find(identifier)
            .expect("Unicode identifier byte offset");
        assert_eq!(
            span.start.column,
            unicode_source[..byte_start].chars().count() + 1
        );
        assert_eq!(
            span.end.column,
            span.start.column + identifier.chars().count()
        );
        assert_eq!(
            span.byte_range,
            Some(TextRange::new(
                u32::try_from(byte_start).expect("test source offset fits u32"),
                u32::try_from(byte_start + identifier.len())
                    .expect("test source end offset fits u32"),
            ))
        );
    }

    #[test]
    fn invalid_escape_reports_the_full_literal_span() {
        let source = r#"module Escape { fn invalid() { let string value = "\q"; } }"#;
        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("escape.ko"),
            })
            .expect_err("unknown escape must fail lexing");
        assert_eq!(diagnostics.diagnostics.len(), 1);
        let diagnostic = &diagnostics.diagnostics[0];
        assert_eq!(diagnostic.code, "K0100");
        assert_eq!(diagnostic.phase, crate::diagnostic::DiagnosticPhase::Lex);
        let span = diagnostic.primary_span.as_ref().expect("literal span");
        let start = source.find(r#""\q""#).expect("literal offset") + 1;
        assert_eq!(span.start.column, start);
        assert_eq!(span.end.column, start + r#""\q""#.len());
    }

    #[test]
    fn state_initialization_diagnostics_retain_actionable_spans() {
        let session = CompilerSession::default();
        for (source, code, expected_primary) in [
            (
                "seiyaku Missing { state int value; view fn read() -> int { return value; } }",
                "E_STATE_HAJIMARI_REQUIRED",
                "state",
            ),
            (
                "seiyaku Partial { state int left; state int right; hajimari() { left = 0; } }",
                "E_STATE_HAJIMARI_INCOMPLETE",
                "hajimari",
            ),
        ] {
            let request = CompileRequest {
                source,
                source_name: Some("state.ko"),
            };
            let checked = session
                .check(request)
                .expect_err("incomplete scalar initialization must fail checking");
            let error = session
                .build(CompileRequest {
                    source,
                    source_name: Some("state.ko"),
                })
                .expect_err("incomplete scalar initialization must fail");
            assert_eq!(checked, error);
            let diagnostic = error
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == code)
                .unwrap_or_else(|| panic!("missing {code}: {error:?}"));
            let span = diagnostic.primary_span.as_ref().expect("primary span");
            assert_eq!(span.source.as_deref(), Some("state.ko"));
            let range = span.byte_range.expect("byte range");
            assert_eq!(
                &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
                expected_primary
            );
        }
    }

    #[test]
    fn adversarial_error_fanout_is_bounded_and_reported() {
        let mut source = String::from("seiyaku ErrorFanout {\n");
        for index in 0..80 {
            use std::fmt::Write as _;
            writeln!(
                source,
                "fn failure_{index}() -> int {{ return missing_{index}; }}"
            )
            .expect("write source fixture");
        }
        source.push_str("}\n");

        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source: &source,
                source_name: Some("fanout.ko"),
            })
            .expect_err("adversarial source must fail");
        assert_eq!(
            diagnostics.diagnostics.len(),
            crate::diagnostic::MAX_DIAGNOSTICS
        );
        let limit = diagnostics.diagnostics.last().expect("limit diagnostic");
        assert_eq!(limit.code, "K0004");
        assert_eq!(limit.phase, DiagnosticPhase::Resolve);
        assert!(limit.primary_span.is_none());
        assert!(
            limit.message.contains("17 additional"),
            "unexpected limit diagnostic: {}",
            limit.message
        );
        assert_eq!(
            diagnostics
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "K2002")
                .count(),
            crate::diagnostic::MAX_DIAGNOSTICS - 1
        );
    }

    #[test]
    fn production_rejects_every_local_test_declaration_with_exact_spans() {
        let production = CompilerSession::default();
        let test_mode = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        });
        for (source, expected_span) in [
            (
                "seiyaku Demo { view fn helper() {} #[test] fn smoke() {} }",
                "#[test]",
            ),
            (
                "seiyaku Demo { view fn helper() {} fixture seeded { caller(\"alice\"); } }",
                "fixture",
            ),
            (
                "seiyaku Demo { view fn helper() {} koto_test { target: \"demo.ko\" } }",
                "koto_test",
            ),
        ] {
            let request = CompileRequest {
                source,
                source_name: Some("local-test.ko"),
            };
            let diagnostics = production
                .build(request)
                .expect_err("production artifacts must reject local test syntax");
            assert_eq!(diagnostics.diagnostics.len(), 1, "{diagnostics:?}");
            let diagnostic = &diagnostics.diagnostics[0];
            assert_eq!(diagnostic.code, "E_TEST_ONLY_PRODUCTION");
            assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
            let span = diagnostic.primary_span.as_ref().expect("test syntax span");
            assert_eq!(span.source.as_deref(), Some("local-test.ko"));
            let range = span.byte_range.expect("exact test syntax range");
            assert_eq!(
                &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
                expected_span
            );

            test_mode
                .build(request)
                .expect("explicit test mode must retain local test syntax");
        }
    }

    #[test]
    fn production_rejects_test_only_helpers_and_test_capable_typed_hir() {
        let source = r#"seiyaku Demo {
            kotoage fn run() authorize("Run") { test::assert(true); }
        }"#;
        let request = CompileRequest {
            source,
            source_name: Some("test-helper.ko"),
        };
        let production = CompilerSession::default();
        for diagnostics in [
            production
                .check(request)
                .expect_err("production checking must reject test-only helpers"),
            production
                .build(request)
                .expect_err("production building must reject test-only helpers"),
        ] {
            assert!(
                diagnostics
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "E_TEST_ONLY_PRODUCTION"),
                "{diagnostics:?}"
            );
        }

        let test_options = CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        };
        CompilerSession::new(test_options.clone())
            .build(request)
            .expect("explicit test mode enables test-only helpers");

        let program = crate::parser::parse(source).expect("parse helper program");
        let typed = crate::semantic::SemanticContext::with_capabilities(false, true)
            .analyze(&program)
            .expect("analyze explicit test HIR");
        assert!(typed.test_support_enabled);
        let diagnostics = production
            .build_typed_program(typed, Some("test-helper.ko"))
            .expect_err("test-capable HIR must not cross into production codegen");
        assert_eq!(diagnostics.diagnostics[0].code, "E_TEST_ONLY_PRODUCTION");
    }

    #[test]
    fn typed_test_graph_is_restricted_to_test_mode_and_derives_a_clean_runtime() {
        let source = r#"seiyaku Demo {
            fn current() -> int { return 1; }
            view fn value() -> int { return current(); }
            #[test]
            fn smoke() { test::assert(current() == 1); }
        }"#;
        let target = TestSourceUnit {
            source_name: "suite.ko".to_owned(),
            source: source.to_owned(),
        };

        let diagnostics = CompilerSession::default()
            .build_test_sources(&target, &[])
            .expect_err("production sessions must not expose the local test build path");
        assert_eq!(diagnostics.diagnostics[0].code, "E_TEST_ONLY_PRODUCTION");

        let outputs = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        })
        .build_test_sources(&target, &[])
        .expect("explicit test-mode suite build");
        assert!(
            outputs
                .suite
                .report
                .source_map
                .iter()
                .any(|entry| entry.function_name == "smoke")
        );
        assert!(
            outputs
                .runtime
                .as_ref()
                .expect("public view requires a runtime projection")
                .report
                .source_map
                .iter()
                .all(|entry| entry.function_name != "smoke")
        );
    }

    #[test]
    fn typed_test_graph_projects_durable_state_map_intrinsics_into_runtime() {
        let target = TestSourceUnit {
            source_name: "state-map-runtime.ko".to_owned(),
            source: r#"seiyaku StateMapRuntime {
                state StateMap<Name, int> flags;
                fn current() -> int {
                    return match flags.get(Name::parse("paused")) {
                        Option::some(value) => value,
                        Option::none => 0,
                    };
                }
                view fn paused() -> int { return current(); }
                #[test]
                fn absent_flag_is_zero() { test::assert(current() == 0); }
            }"#
            .to_owned(),
        };

        let outputs = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        })
        .build_test_sources(&target, &[])
        .expect("valid compiler intrinsics must survive the production projection");
        let runtime = outputs
            .runtime
            .expect("public view requires runtime output");
        assert!(
            runtime
                .report
                .source_map
                .iter()
                .any(|entry| entry.function_name == "current")
        );
        assert!(
            runtime
                .report
                .source_map
                .iter()
                .all(|entry| entry.function_name != "absent_flag_is_zero")
        );
    }

    #[test]
    fn typed_test_graph_without_runtime_entrypoint_skips_runtime_artifact() {
        let target = TestSourceUnit {
            source_name: "pure-suite.ko".to_owned(),
            source: r#"seiyaku PureSuite {
                fn helper() -> int { return 1; }
                #[test]
                fn smoke() { test::assert(helper() == 1); }
            }"#
            .to_owned(),
        };

        let outputs = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        })
        .build_test_sources(&target, &[])
        .expect("compile pure unit-test suite");

        assert!(outputs.runtime.is_none());
        assert!(
            outputs
                .suite
                .report
                .source_map
                .iter()
                .any(|entry| entry.function_name == "smoke")
        );
    }

    #[test]
    fn test_target_projection_retains_lowered_list_intrinsics() {
        let target = TestSourceUnit {
            source_name: "list-runtime.ko".to_owned(),
            source: r#"seiyaku ListRuntime {
                view fn count(List<int, 4> values) -> int {
                    let length = values.len();
                    let _contains = values.contains(1);
                    let _first = values.get(0);
                    var List<int, 4> copy = [];
                    let _pushed = copy.try_push(1);
                    return length;
                }
                #[test]
                fn runtime_projection_exists() { test::assert(true); }
            }"#
            .to_owned(),
        };

        let outputs = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        })
        .build_test_sources(&target, &[])
        .expect("lowered list intrinsics must survive runtime projection");

        assert!(
            outputs.runtime.is_some(),
            "public view requires runtime output"
        );
    }

    #[test]
    fn test_target_projection_retains_lowered_sum_type_intrinsics() {
        let target = TestSourceUnit {
            source_name: "sum-runtime.ko".to_owned(),
            source: r#"seiyaku SumRuntime {
                view fn exercise_sum_intrinsics(int fallback) -> int {
                    let Option<int> present = Option::some(5);
                    let Option<int> missing = Option::none;
                    let Result<int, string> success = Result::ok(7);
                    let Result<int, string> failure = Result::err("failed");
                    let _present = present.is_some();
                    let _missing = missing.is_none();
                    let _success = success.is_ok();
                    let _failure = failure.is_err();
                    let value = present.unwrap_or(fallback);
                    let error_value = failure.unwrap_err_or("fallback");
                    if error_value == "failed" {
                        return value + success.unwrap_or(fallback);
                    }
                    return fallback;
                }
                #[test]
                fn exercises_sum_intrinsics() {
                    test::assert(true);
                }
            }"#
            .to_owned(),
        };

        let outputs = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        })
        .build_test_sources(&target, &[])
        .expect("lowered sum-type intrinsics must survive runtime projection");

        assert!(
            outputs.runtime.is_some(),
            "public view requires runtime output"
        );
    }

    #[test]
    fn standalone_test_graph_keeps_stable_distinct_sources_and_typed_target_state() {
        let target = TestSourceUnit {
            source_name: "contracts/counter.ko".to_owned(),
            source: r#"seiyaku Counter {
                state int counter;
                hajimari() { counter = 0; }
                fn current() -> int { return counter; }
                view fn value() -> int { return current(); }
            }"#
            .to_owned(),
        };
        let first = TestSourceUnit {
            source_name: "tests/first.test.ko".to_owned(),
            source: r#"module FirstTests {
                koto_test { target: "../contracts/counter.ko" }
                #[test]
                fn observes_target_state() { test::assert(counter == 0); }
            }"#
            .to_owned(),
        };
        let second = TestSourceUnit {
            source_name: "tests/second.test.ko".to_owned(),
            source: r#"module SecondTests {
                koto_test { target: "../contracts/counter.ko" }
                #[test]
                fn calls_target_helper() { test::assert(current() == 0); }
            }"#
            .to_owned(),
        };
        let session = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        });
        let forward = session
            .build_test_sources(&target, &[first.clone(), second.clone()])
            .expect("link source-identified standalone tests");
        let reverse = session
            .build_test_sources(&target, &[second, first])
            .expect("test source order cannot affect linking");
        assert_eq!(forward.suite.artifact, reverse.suite.artifact);
        assert_eq!(
            forward
                .runtime
                .as_ref()
                .expect("target lifecycle requires runtime")
                .artifact,
            reverse
                .runtime
                .as_ref()
                .expect("target lifecycle requires runtime")
                .artifact
        );

        let paths = forward
            .suite
            .report
            .source_map
            .iter()
            .filter_map(|entry| {
                entry
                    .source
                    .source_path
                    .as_deref()
                    .map(|path| (path, entry.source.source_id))
            })
            .collect::<BTreeSet<_>>();
        assert!(
            paths
                .iter()
                .any(|(path, _)| *path == "contracts/counter.ko")
        );
        assert!(paths.iter().any(|(path, _)| *path == "tests/first.test.ko"));
        assert!(
            paths
                .iter()
                .any(|(path, _)| *path == "tests/second.test.ko")
        );
        assert_eq!(
            paths
                .iter()
                .map(|(_, source_id)| *source_id)
                .collect::<BTreeSet<_>>()
                .len(),
            3,
            "every logical source must retain a distinct stable SourceId"
        );
        assert!(forward.suite.report.source_map.iter().all(|entry| {
            entry.source.byte_start < entry.source.byte_end && entry.source.source_id != 0
        }));
        let source_texts = BTreeMap::from([
            ("contracts/counter.ko", target.source.as_str()),
            (
                "tests/first.test.ko",
                r#"module FirstTests {
                koto_test { target: "../contracts/counter.ko" }
                #[test]
                fn observes_target_state() { test::assert(counter == 0); }
            }"#,
            ),
            (
                "tests/second.test.ko",
                r#"module SecondTests {
                koto_test { target: "../contracts/counter.ko" }
                #[test]
                fn calls_target_helper() { test::assert(current() == 0); }
            }"#,
            ),
        ]);
        for entry in &forward.suite.report.source_map {
            let source_path = entry
                .source
                .source_path
                .as_deref()
                .expect("every source segment retains its logical path");
            let source = source_texts[source_path];
            let start = usize::try_from(entry.source.byte_start).expect("source offset fits usize");
            let end = usize::try_from(entry.source.byte_end).expect("source offset fits usize");
            assert!(start < end && end <= source.len());
            assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
        }
    }

    #[test]
    fn standalone_test_graph_rejects_normalized_logical_path_aliases() {
        let target = TestSourceUnit {
            source_name: "contracts/demo.ko".to_owned(),
            source: "seiyaku Demo {}".to_owned(),
        };
        let source = r#"module Tests {
            koto_test { target: "../contracts/demo.ko" }
            #[test]
            fn smoke() { test::assert(true); }
        }"#;
        let first = TestSourceUnit {
            source_name: "tests/./demo.test.ko".to_owned(),
            source: source.to_owned(),
        };
        let alias = TestSourceUnit {
            source_name: "tests/demo.test.ko".to_owned(),
            source: source.to_owned(),
        };
        let diagnostics = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        })
        .build_test_sources(&target, &[first, alias])
        .expect_err("normalized aliases must not receive distinct SourceIds");
        assert_eq!(diagnostics.diagnostics[0].code, "E_DUPLICATE_SOURCE");
    }

    #[test]
    fn session_enforces_presence_aware_state_map_reads() {
        let session = CompilerSession::default();
        for (source, expected_code) in [
            (
                "seiyaku InvalidIndex { state StateMap<int, int> values; view fn read() -> int { return values[1]; } }",
                "E_STATE_MAP_OPTIONAL_READ",
            ),
            (
                "seiyaku InvalidFlatGet { state StateMap<int, int> values; view fn read() -> Option<int> { return get(values, 1); } }",
                "K2002",
            ),
        ] {
            let diagnostics = session
                .check(CompileRequest {
                    source,
                    source_name: Some("state-map-read.ko"),
                })
                .expect_err("presence-erasing StateMap read must fail");
            assert!(
                diagnostics
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == expected_code),
                "missing {expected_code}: {diagnostics:?}"
            );
        }

        let source = r#"
            seiyaku PresenceAware {
                state StateMap<int, int> values;

                fn get(int value) -> int { return value; }

                kotoage fn set(int key, int value) authorize("Write") {
                    values[key] = value;
                }

                view fn read(int key) -> Option<int> {
                    return values.get(key);
                }

                view fn echo(int value) -> int {
                    return get(value);
                }
            }
        "#;
        let output = session
            .build(CompileRequest {
                source,
                source_name: Some("presence-aware.ko"),
            })
            .expect("canonical Option read, indexed write, and user get function must build");
        let interface = output
            .manifest
            .entrypoints
            .as_ref()
            .expect("entrypoint manifest")
            .iter()
            .find(|entrypoint| entrypoint.name == "read")
            .expect("read entrypoint manifest");
        assert_eq!(interface.return_type.as_deref(), Some("Option<int>"));
    }

    #[test]
    fn session_rejects_unit_values_degenerate_tuple_types_and_uninitialized_vars() {
        let session = CompilerSession::default();
        for (source, expected_span) in [
            ("seiyaku Invalid { fn run() { let value = (); } }", "()"),
            ("seiyaku Invalid { fn run((int) value) {} }", "(int)"),
            ("seiyaku Invalid { fn run() -> () { return; } }", "()"),
            ("seiyaku Invalid { fn run() { var int value; } }", ";"),
        ] {
            let diagnostics = session
                .check(CompileRequest {
                    source,
                    source_name: Some("invalid-unit.ko"),
                })
                .expect_err("non-V1 Unit/tuple/binding surface must fail");
            let diagnostic = diagnostics.diagnostics.first().expect("parse diagnostic");
            assert_eq!(diagnostic.code, "K1001", "{diagnostics:?}");
            assert_eq!(diagnostic.phase, DiagnosticPhase::Parse);
            let span = diagnostic.primary_span.as_ref().expect("parse span");
            let range = span.byte_range.expect("exact parse range");
            assert_eq!(
                &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
                expected_span
            );
        }
    }

    #[test]
    fn session_builds_grouping_real_tuples_and_else_if_chains() {
        let source = r#"
            seiyaku Branches {
                view fn classify(int value) -> int {
                    if value < 0 {
                        return -1;
                    } else if value == 0 {
                        return 0;
                    } else {
                        return 1;
                    }
                }

                view fn grouped(int value) -> int {
                    return (value);
                }

                view fn pair() -> (int, bool) {
                    return (1, true);
                }
            }
        "#;
        let output = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("branches.ko"),
            })
            .expect("documented else-if, grouping, and real tuples must build");
        assert_eq!(output.manifest.seiyaku_name.as_deref(), Some("Branches"));
    }

    #[test]
    fn concurrent_builds_do_not_share_semantic_declarations() {
        let session = CompilerSession::default();
        let sources = [
            (
                r#"
                seiyaku IntegerContract {
                    struct Shared { int value; }
                    fn make() -> Shared { return Shared { value: 7 }; }
                    view fn read() -> int {
                        let record = make();
                        return record.value;
                    }
                }
                "#,
                "IntegerContract",
            ),
            (
                r#"
                seiyaku BooleanContract {
                    struct Shared { bool value; }
                    fn make() -> Shared { return Shared { value: true }; }
                    view fn read() -> bool {
                        let record = make();
                        return record.value;
                    }
                }
                "#,
                "BooleanContract",
            ),
        ];

        std::thread::scope(|scope| {
            let handles = (0..8)
                .map(|worker| {
                    let session = &session;
                    let (source, expected_name) = sources[worker % sources.len()];
                    scope.spawn(move || {
                        for round in 0..4 {
                            let source_name = format!("worker-{worker}-round-{round}.ko");
                            let output = session
                                .build(CompileRequest {
                                    source,
                                    source_name: Some(&source_name),
                                })
                                .expect("parallel compilation must remain isolated");
                            assert_eq!(
                                output.manifest.seiyaku_name.as_deref(),
                                Some(expected_name)
                            );
                        }
                    })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                handle.join().expect("compiler worker must not panic");
            }
        });
    }
}
