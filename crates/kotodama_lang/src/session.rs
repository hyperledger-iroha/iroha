//! Reusable canonical compiler session API.

use iroha_data_model::smart_contract::manifest::ContractManifest;

use crate::{
    ast::{Program, SourceUnitKind},
    compiler::{CompileReport, Compiler, CompilerMode, CompilerOptions},
    diagnostic::{Diagnostic, DiagnosticBundle, DiagnosticPhase, SourcePosition, SourceSpan},
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
    /// Deployable `.to` bytes.
    pub artifact: Vec<u8>,
    /// Manifest derived from the embedded contract interface.
    pub manifest: ContractManifest,
    /// Source-map, budget, and access-hint sidecar data.
    pub report: CompileReport,
}

/// Explicit reusable compiler context used by CLIs, SDK bindings, and language tools.
#[derive(Clone, Debug)]
pub struct CompilerSession {
    options: CompilerOptions,
}

struct IterativeProgramGuard {
    program: Option<Program>,
}

impl IterativeProgramGuard {
    fn new(program: Program) -> Self {
        Self {
            program: Some(program),
        }
    }

    fn get(&self) -> &Program {
        self.program.as_ref().expect("program guard is populated")
    }

    fn take(mut self) -> Program {
        self.program.take().expect("program guard is populated")
    }
}

impl Drop for IterativeProgramGuard {
    fn drop(&mut self) {
        if let Some(program) = self.program.take() {
            crate::ast::drop_program_iterative(program);
        }
    }
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

    /// Parse and type/effect-check one contract or reusable module without
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

    fn checked_program(&self, request: CompileRequest<'_>) -> Result<Program, DiagnosticBundle> {
        enforce_source_budget(request)?;
        let source = SourceFile::new(
            SourceId(0),
            request.source_name.unwrap_or("<source>"),
            request.source,
        );
        let (program, tokens) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())?;
        let program = IterativeProgramGuard::new(program);
        reject_production_test_surface(
            self.options.mode,
            program.get(),
            request.source_name,
            Some((&source, &tokens)),
        )?;
        crate::semantic_diagnostics::audit(program.get(), &source, &tokens)?;
        let semantic = crate::semantic::SemanticContext::with_capabilities(
            self.options.force_zk,
            self.options.mode == crate::compiler::CompilerMode::Test,
        );
        let typed = semantic.analyze_all(program.get()).map_err(|failures| {
            crate::semantic_diagnostics::from_semantic_failures(
                failures,
                request.source_name,
                Some(&source),
                Some(&tokens),
            )
        })?;
        crate::semantic::validate_linked_program(&typed, self.options.force_zk).map_err(
            |error| {
                crate::semantic_diagnostics::from_semantic_failures(
                    error.into(),
                    request.source_name,
                    Some(&source),
                    Some(&tokens),
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
        Ok(program.take())
    }

    /// Compile one named source unit into a deployable artifact and sidecar report.
    pub fn build(&self, request: CompileRequest<'_>) -> Result<CompileOutput, DiagnosticBundle> {
        enforce_source_budget(request)?;
        let source = SourceFile::new(
            SourceId(0),
            request.source_name.unwrap_or("<source>"),
            request.source,
        );
        let (program, tokens) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())?;
        let program = IterativeProgramGuard::new(program);
        reject_production_test_surface(
            self.options.mode,
            program.get(),
            request.source_name,
            Some((&source, &tokens)),
        )?;
        crate::semantic_diagnostics::audit(program.get(), &source, &tokens)?;
        require_deployable_contract(program.get(), request.source_name)?;
        let semantic = crate::semantic::SemanticContext::with_capabilities(
            self.options.force_zk,
            self.options.mode == crate::compiler::CompilerMode::Test,
        );
        let typed = semantic.analyze_all(program.get()).map_err(|failures| {
            crate::semantic_diagnostics::from_semantic_failures(
                failures,
                request.source_name,
                Some(&source),
                Some(&tokens),
            )
        })?;
        drop(program);
        self.build_typed_program(typed, request.source_name)
    }

    /// Compile a resolved source program through the same canonical driver.
    ///
    /// Typed-HIR module linkers use this entry point after assembling a single
    /// deployable program; all artifact construction and diagnostics remain
    /// shared with ordinary source builds.
    pub fn build_program(
        &self,
        program: &Program,
        source_name: Option<&str>,
    ) -> Result<CompileOutput, DiagnosticBundle> {
        reject_production_test_surface(self.options.mode, program, source_name, None)?;
        require_deployable_contract(program, source_name)?;
        let compiler = Compiler::new_with_options(self.options.clone());
        let semantic_context = crate::semantic::SemanticContext::with_capabilities(
            self.options.force_zk,
            self.options.mode == crate::compiler::CompilerMode::Test,
        );
        let (artifact, manifest, report) = compiler
            .compile_program_with_manifest_and_report_diagnostics(
                program,
                &semantic_context,
                source_name,
            )?;
        Ok(CompileOutput {
            artifact,
            manifest,
            report,
        })
    }

    /// Compile a fully resolved and linked typed-HIR program.
    ///
    /// This is the only post-link code-generation entry point. It deliberately
    /// accepts no AST, so package managers cannot reintroduce source rewriting
    /// after module type/effect analysis.
    pub fn build_typed_program(
        &self,
        program: TypedProgram,
        source_name: Option<&str>,
    ) -> Result<CompileOutput, DiagnosticBundle> {
        if program.unit.kind != SourceUnitKind::Contract {
            return Err(non_deployable_module_diagnostic(source_name));
        }
        let compiler = Compiler::new_with_options(self.options.clone());
        let (artifact, manifest, report) = compiler
            .compile_typed_program_with_manifest_and_report_diagnostics(program, source_name)?;
        Ok(CompileOutput {
            artifact,
            manifest,
            report,
        })
    }
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
    if program.unit.kind == SourceUnitKind::Contract {
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
        Some("link the module into exactly one contract root, then build that contract".to_owned());
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
                source: "seiyaku Demo { view fn ping() -> i64 { return 1; } }",
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
                source: "not a contract",
                source_name: Some("broken.ko"),
            })
            .expect_err("invalid source must return diagnostics");
        assert_eq!(error.diagnostics.len(), 1);
    }

    #[test]
    fn check_drops_a_boundary_depth_program_iteratively() {
        let mut ty = String::from("i64");
        for _ in 0..crate::source::MAX_NESTING_DEPTH - 2 {
            ty = format!("Option<{ty}>");
        }
        let source = format!("module Deep {{ struct Wrapper {{ value: {ty} }} }}");
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

        let invalid = source.replace("i64", "UnknownType");
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
    f00: i64, f01: i64, f02: i64, f03: i64, f04: i64, f05: i64, f06: i64,
    f07: i64, f08: i64, f09: i64, f10: i64, f11: i64, f12: i64, f13: i64
  }
  view fn inspect(value: Wide) -> i64 { return value.f00; }
}"#;
        let diagnostics = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("wide-call.ko"),
            })
            .expect_err("oversized flattened argument ABI must fail before lowering");
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
    }

    #[test]
    fn modules_can_be_checked_but_never_emitted_directly() {
        let session = CompilerSession::default();
        let request = CompileRequest {
            source: "module Math { fn add(left: i64, right: i64) -> i64 { return left + right; } }",
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
    fn check_and_build_return_identical_semantic_records_for_contracts_and_modules() {
        let session = CompilerSession::default();
        for source in [
            r#"seiyaku Broken {
                fn first(value: Missing) -> i64 { return unknown; }
                fn second(flag: bool) { flag = false; missing_call(); }
            }"#,
            r#"module Broken {
                fn first(value: Missing) -> i64 { return unknown; }
                fn second(flag: bool) { flag = false; missing_call(); }
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
            assert!(checked.diagnostics.len() >= 4, "{checked:?}");
            assert!(checked.diagnostics.iter().all(|diagnostic| {
                diagnostic
                    .primary_span
                    .as_ref()
                    .is_some_and(|span| span.byte_range.is_some())
            }));
        }
    }

    #[test]
    fn semantic_failures_are_collected_with_stable_source_spans() {
        let source = "seiyaku Broken {\nfn first() -> i64 { return missing_first; }\nfn second() -> i64 { return missing_second; }\n}";
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
            assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
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
    fn parser_recovers_independent_items_into_one_spanned_bundle() {
        let source = "seiyaku Broken {\nfn first() { let value: i64 = ; }\nfn second() { let value: bool = ; }\n}";
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
        let source = "seiyaku Broken {\nfn first() { let value: i64 = ; }\nfn second() { let value: bool = ; }\n}";
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
        let source = r#"module Escape { fn invalid() { let value: string = "\q"; } }"#;
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
                "seiyaku Missing { state value: i64; view fn read() -> i64 { return value; } }",
                "E_STATE_INIT_REQUIRED",
                "state",
            ),
            (
                "seiyaku Partial { state left: i64; state right: i64; hajimari() { left = 0; } }",
                "E_STATE_INIT_INCOMPLETE",
                "hajimari",
            ),
        ] {
            let error = session
                .build(CompileRequest {
                    source,
                    source_name: Some("state.ko"),
                })
                .expect_err("incomplete scalar initialization must fail");
            let diagnostic = error
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == code)
                .unwrap_or_else(|| panic!("missing {code}: {error:?}"));
            let span = diagnostic.primary_span.as_ref().expect("primary span");
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
                "fn failure_{index}() -> i64 {{ return missing_{index}; }}"
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
        assert!(
            limit.message.contains("17 additional"),
            "unexpected limit diagnostic: {}",
            limit.message
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
                "seiyaku Demo { fn helper() {} #[test] fn smoke() {} }",
                "#[test]",
            ),
            (
                "seiyaku Demo { fn helper() {} fixture seeded { caller(\"alice\"); } }",
                "fixture",
            ),
            (
                "seiyaku Demo { fn helper() {} koto_test { target: \"demo.ko\" } }",
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
    fn session_enforces_presence_aware_state_map_reads() {
        let session = CompilerSession::default();
        for (source, expected_code) in [
            (
                "seiyaku InvalidIndex { state values: StateMap<i64, i64>; view fn read() -> i64 { return values[1]; } }",
                "E_STATE_MAP_OPTIONAL_READ",
            ),
            (
                "seiyaku InvalidFlatGet { state values: StateMap<i64, i64>; view fn read() -> Option<i64> { return get(values, 1); } }",
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
                state values: StateMap<i64, i64>;

                fn get(value: i64) -> i64 { return value; }

                kotoage fn set(key: i64, value: i64) authorize("Write") {
                    values[key] = value;
                }

                view fn read(key: i64) -> Option<i64> {
                    return values.get(key);
                }

                view fn echo(value: i64) -> i64 {
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
        assert_eq!(interface.return_type.as_deref(), Some("Option<i64>"));
    }

    #[test]
    fn session_rejects_unit_values_degenerate_tuple_types_and_uninitialized_vars() {
        let session = CompilerSession::default();
        for (source, expected_span) in [
            ("seiyaku Invalid { fn run() { let value = (); } }", "()"),
            ("seiyaku Invalid { fn run(value: (i64)) {} }", "(i64)"),
            ("seiyaku Invalid { fn run() -> () { return; } }", "()"),
            ("seiyaku Invalid { fn run() { var value: i64; } }", ";"),
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
                view fn classify(value: i64) -> i64 {
                    if value < 0 {
                        return -1;
                    } else if value == 0 {
                        return 0;
                    } else {
                        return 1;
                    }
                }

                view fn grouped(value: i64) -> i64 {
                    return (value);
                }

                view fn pair() -> (i64, bool) {
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
        assert_eq!(output.manifest.contract_name.as_deref(), Some("Branches"));
    }

    #[test]
    fn concurrent_builds_do_not_share_semantic_declarations() {
        let session = CompilerSession::default();
        let sources = [
            (
                r#"
                seiyaku IntegerContract {
                    struct Shared { value: i64; }
                    fn make() -> Shared { return Shared(7); }
                    view fn read() -> i64 {
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
                    struct Shared { value: bool; }
                    fn make() -> Shared { return Shared(true); }
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
                                output.manifest.contract_name.as_deref(),
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
