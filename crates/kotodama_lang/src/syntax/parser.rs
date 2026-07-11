//! Canonical single-scan source parser and lossless CST construction.

use crate::{
    ast::Program,
    diagnostic::DiagnosticBundle,
    source::{FrontendBudget, SourceFile},
};

use super::{
    cst::{SyntaxOutline, SyntaxOutlineBuilder, SyntaxTree, build_tree_from_outline},
    kind::SyntaxKind,
    lexer::lex,
};

/// Result of parsing for lossless editor and formatter consumers.
#[derive(Clone, Debug)]
pub struct ParseOutput {
    /// Concrete syntax tree, available even when diagnostics were emitted.
    pub tree: SyntaxTree,
    /// Deterministically ordered, bounded syntax diagnostics.
    pub diagnostics: DiagnosticBundle,
}

impl ParseOutput {
    /// Return whether parsing completed without diagnostics.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.diagnostics.diagnostics.is_empty()
    }
}

/// Canonical compiler parse result.
///
/// `tree` and `program` are derived from the same lossless token stream. A
/// failed parse retains the complete tree and omits the AST, preventing tools
/// from accidentally compiling a source that editor validation rejected.
#[derive(Clone, Debug)]
pub struct ProgramParseOutput {
    /// Complete lossless source tree.
    pub tree: SyntaxTree,
    /// Plain tooling AST parsed from the spanned tokens, present only after
    /// success. Hidden source/resolution provenance is stripped iteratively.
    pub program: Option<Program>,
    /// Wrapper-bearing parser AST consumed only by compiler-internal resolved
    /// HIR construction. Public tooling receives `program`, whose hidden
    /// provenance wrappers have been removed iteratively.
    pub(crate) sourced_program: Option<Program>,
    /// Deterministically ordered, bounded frontend diagnostics.
    pub diagnostics: DiagnosticBundle,
    /// Significant spanned tokens lowered from the same lossless scan.
    ///
    /// Compiler phases use this private stream for exact semantic source
    /// ranges; exposing it only within the crate prevents a second scanner
    /// from drifting from the CST.
    pub(crate) tokens: Vec<crate::lexer::Token>,
    /// Stable CST-derived AST source facts for compiler resolution.
    pub(crate) ast_facts: Option<crate::spanned_ast::AstFacts>,
}

impl ProgramParseOutput {
    /// Return whether a valid AST was produced without diagnostics.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.program.is_some() && self.diagnostics.diagnostics.is_empty()
    }
}

/// Parse one source file once, producing both its lossless CST and compiler AST.
#[must_use]
pub fn parse_program(source: &SourceFile, budget: FrontendBudget) -> ProgramParseOutput {
    parse_program_internal(source, budget, true)
}

pub(crate) fn parse_spanned_program(
    source: &SourceFile,
    budget: FrontendBudget,
) -> Result<(crate::spanned_ast::SpannedProgram, Vec<crate::lexer::Token>), DiagnosticBundle> {
    let output = parse_program_internal(source, budget, false);
    match (output.sourced_program, output.ast_facts) {
        (Some(program), Some(facts)) => Ok((
            crate::spanned_ast::SpannedProgram { program, facts },
            output.tokens,
        )),
        _ => Err(output.diagnostics),
    }
}

fn parse_program_internal(
    source: &SourceFile,
    budget: FrontendBudget,
    produce_plain_program: bool,
) -> ProgramParseOutput {
    let lexed = lex(source, budget);
    let lossless_tokens = lexed.tokens.clone();
    let (lowered_tokens, lexical_diagnostics) =
        crate::lexer::lower_lexed_recovering(source, budget, lexed);
    let lexical_failure = !lexical_diagnostics.is_empty();
    let resource_failure = lexical_diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.code.as_str(), "K0001" | "K0002" | "K0003"));
    let (
        mut program,
        mut sourced_program,
        mut ast_facts,
        syntax_diagnostics,
        tokens,
        outline,
        missing,
    ) = if resource_failure {
        (
            None,
            None,
            None,
            DiagnosticBundle::new(Vec::new()),
            Vec::new(),
            error_outline(source),
            Vec::new(),
        )
    } else {
        match crate::parser::validate_nesting(source, budget, &lowered_tokens) {
            Ok(()) => {
                let parsed = crate::parser::parse_with_syntax(source, &lowered_tokens);
                let (program, sourced_program, ast_facts) =
                    parsed.spanned.map_or((None, None, None), |spanned| {
                        if produce_plain_program {
                            let mut plain = spanned.program;
                            crate::ast::strip_program_provenance(&mut plain);
                            (Some(plain), None, None)
                        } else {
                            (None, Some(spanned.program), Some(spanned.facts))
                        }
                    });
                let tokens = if produce_plain_program {
                    Vec::new()
                } else {
                    lowered_tokens
                };
                (
                    program,
                    sourced_program,
                    ast_facts,
                    parsed.diagnostics,
                    tokens,
                    parsed.outline,
                    parsed.missing,
                )
            }
            Err(diagnostics) => (
                None,
                None,
                None,
                diagnostics,
                Vec::new(),
                error_outline(source),
                Vec::new(),
            ),
        }
    };
    if lexical_failure {
        // Recovery trees are tooling output only. No AST containing or
        // surrounding a malformed token can cross into semantic analysis.
        program = None;
        sourced_program = None;
        ast_facts = None;
    }
    // Parser recovery after a malformed token exists to preserve CST shape,
    // not to manufacture cascaded user diagnostics from a token that the
    // semantic stream deliberately omitted. Report the authoritative lexical
    // failures first; syntax diagnostics become authoritative only when the
    // token stream itself was valid.
    let diagnostics = if lexical_failure {
        DiagnosticBundle::new(lexical_diagnostics)
    } else {
        syntax_diagnostics
    };
    let tree = build_tree_from_outline(source.id(), &lossless_tokens, &outline, &missing);
    ProgramParseOutput {
        tree,
        program,
        sourced_program,
        diagnostics,
        tokens,
        ast_facts,
    }
}

/// Parse one source file into the canonical lossless CST.
#[must_use]
pub fn parse(source: &SourceFile, budget: FrontendBudget) -> ParseOutput {
    let output = parse_program(source, budget);
    ParseOutput {
        tree: output.tree,
        diagnostics: output.diagnostics,
    }
}

fn error_outline(source: &SourceFile) -> SyntaxOutline {
    let mut builder = SyntaxOutlineBuilder::default();
    let root = builder.start(SyntaxKind::Root, 0);
    let error = builder.start(SyntaxKind::ErrorNode, 0);
    builder.finish(error, source.full_range().end);
    builder.finish(root, source.full_range().end);
    builder.into_outline()
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::source::SourceId;

    fn count_nodes(node: &crate::syntax::GreenNode, kind: SyntaxKind) -> usize {
        let mut count = usize::from(node.kind == kind);
        for child in &node.children {
            if let crate::syntax::GreenElement::Node(child) = child {
                count = count.saturating_add(count_nodes(child, kind));
            }
        }
        count
    }

    fn cst_snapshot(
        node: &crate::syntax::GreenNode,
        source: &SourceFile,
        depth: usize,
        output: &mut String,
    ) {
        let indent = "  ".repeat(depth);
        writeln!(
            output,
            "{indent}{:?}@{}..{}",
            node.kind, node.range.start, node.range.end
        )
        .expect("write CST node snapshot");
        for child in &node.children {
            match child {
                crate::syntax::GreenElement::Node(child) => {
                    cst_snapshot(child, source, depth + 1, output);
                }
                crate::syntax::GreenElement::Token(token)
                    if !token.kind.is_trivia() && token.kind != SyntaxKind::Eof =>
                {
                    if token.is_missing() {
                        writeln!(
                            output,
                            "{indent}  Missing({:?})@{}",
                            token.expected, token.range.start
                        )
                        .expect("write missing-token snapshot");
                    } else {
                        writeln!(
                            output,
                            "{indent}  {:?}={:?}@{}..{}",
                            token.kind,
                            source.slice(token.range).unwrap_or_default(),
                            token.range.start,
                            token.range.end
                        )
                        .expect("write CST token snapshot");
                    }
                }
                crate::syntax::GreenElement::Token(_) => {}
            }
        }
    }

    #[test]
    fn cst_preserves_amount_literal_text() {
        let text = "seiyaku Demo { view fn amount() -> Amount { return 1.250_0amt; } }";
        let source = SourceFile::new(SourceId(0), "amount.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        assert!(
            output
                .tree
                .tokens()
                .iter()
                .any(|token| token.kind == SyntaxKind::Decimal)
        );
    }

    #[test]
    fn compiler_uses_one_direct_cst_lowering_without_a_token_only_reparse() {
        crate::parser::reset_direct_cst_lowering_count();
        let text = r#"seiyaku Direct {
            struct Pair { left: i64, right: i64 }
            const limit: i64 = 2;
            state value: i64;
            hajimari() { value = 0; }
            kotoage fn set(next: i64) authorize("Set") { value = next; }
            view fn read() -> i64 { value }
        }"#;
        let source = SourceFile::new(SourceId(41), "direct.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(crate::parser::direct_cst_lowering_count(), 1);
        assert_eq!(output.tree.text(&source), text);
    }

    #[test]
    fn public_ast_is_plain_but_internal_ast_retains_direct_node_ids() {
        use crate::ast::{Item, Statement};

        let text = "誓約 Direct { 始まり() { let value: i64 = 1; } }";
        let source = SourceFile::new(SourceId(45), "direct-node-ids.ko", text);
        let public = parse_program(&source, FrontendBudget::v1());
        assert!(public.sourced_program.is_none());
        assert!(public.ast_facts.is_none());
        assert!(public.tokens.is_empty());
        let public_program = public.program.expect("plain public AST");
        let Item::Function(public_function) = &public_program.items[0] else {
            panic!("expected lifecycle function")
        };
        assert!(matches!(
            public_function.body.statements[0],
            Statement::Let { .. }
        ));

        let (spanned, _) = parse_spanned_program(&source, FrontendBudget::v1())
            .expect("wrapper-bearing compiler AST");
        let Item::Function(function) = &spanned.program.items[0] else {
            panic!("expected lifecycle function")
        };
        let Statement::Source {
            node,
            source: statement_source,
            ..
        } = &function.body.statements[0]
        else {
            panic!("compiler AST statement must retain direct provenance")
        };
        assert_eq!(
            spanned
                .facts
                .source_map
                .node(*node)
                .map(|entry| entry.range),
            Some(statement_source.range)
        );
    }

    #[test]
    fn structured_missing_tokens_are_independent_of_diagnostic_wording() {
        let text = "seiyaku Broken { fn bad(value i64) { return; } }";
        let source = SourceFile::new(SourceId(46), "structured-missing.ko", text);
        let lexed = lex(&source, FrontendBudget::v1());
        let lossless = lexed.tokens.clone();
        let tokens = crate::lexer::lower_lexed(&source, FrontendBudget::v1(), lexed)
            .expect("lower significant token view");
        let mut parsed = crate::parser::parse_with_syntax(&source, &tokens);
        let missing_offset =
            u32::try_from(text.find("i64").expect("unexpected type")).expect("source budget");
        assert!(parsed.missing.iter().any(|missing| {
            missing.offset == missing_offset && missing.expected == SyntaxKind::Colon
        }));
        for diagnostic in &mut parsed.diagnostics.diagnostics {
            diagnostic.message = "localized parser message without token spelling".into();
        }
        let tree =
            build_tree_from_outline(source.id(), &lossless, &parsed.outline, &parsed.missing);
        assert!(
            tree.tokens()
                .iter()
                .any(|token| { token.is_missing() && token.expected == Some(SyntaxKind::Colon) })
        );
    }

    #[test]
    fn direct_cst_lowering_preserves_every_branded_declaration_form() {
        use crate::ast::{FunctionKind, Item, SourceUnitKind};

        let text = r#"誓約 Branded {
            struct Pair { left: i64, right: i64 }
            error enum Failure { Bad = 1 }
            const limit: i64 = 2;
            state value: i64;
            trigger tick -> apply { on time pre_commit; }
            始まり() { value = 0; }
            改善() { value = value + 1; }
            言挙げ fn apply(next: i64) authorize("Apply") { value = next; }
            view fn read() -> i64 { value }
            fn helper(value: i64) -> i64 { value }
        }"#;
        let source = SourceFile::new(SourceId(42), "branded-direct.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        let program = output.program.expect("direct CST AST");
        assert_eq!(program.unit.kind, SourceUnitKind::Seiyaku);
        let functions = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) => Some((function.name.as_str(), function.modifiers.kind)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            functions,
            [
                ("hajimari", FunctionKind::Hajimari),
                ("kaizen", FunctionKind::Kaizen),
                ("apply", FunctionKind::Kotoage),
                ("read", FunctionKind::View),
                ("helper", FunctionKind::Private),
            ]
        );
    }

    #[test]
    fn direct_cst_lowering_recovers_multiple_malformed_items() {
        let text = r#"seiyaku Broken {
            fn first(value i64) { return; }
            fn second() { let missing: i64 = ; return; }
            fn third(flag: bool) { if flag { return } }
        }"#;
        let source = SourceFile::new(SourceId(43), "multi-error.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert_eq!(output.tree.text(&source), text);
        assert!(!output.is_ok());
        assert!(
            output.diagnostics.diagnostics.len() >= 2,
            "{:?}",
            output.diagnostics
        );
        assert!(output.diagnostics.diagnostics.iter().all(|diagnostic| {
            diagnostic
                .primary_span
                .as_ref()
                .and_then(|span| span.byte_range)
                .is_some_and(|range| source.slice(range).is_some())
        }));
        assert!(output.tree.tokens().iter().any(|token| token.is_missing()));
    }

    #[test]
    fn japanese_keyword_name_facts_keep_exact_utf8_byte_ranges() {
        let text = "誓約 Demo { 始まり() { let message: string = \"雪\"; } }";
        let source = SourceFile::new(SourceId(44), "unicode-spans.ko", text);
        let (spanned, _) =
            parse_spanned_program(&source, FrontendBudget::v1()).expect("direct-ID compiler AST");
        let facts = spanned.facts;
        let lifecycle = facts
            .declarations
            .iter()
            .find(|declaration| declaration.name == "hajimari")
            .expect("hajimari declaration");
        let name = facts
            .source_map
            .node(lifecycle.name_node)
            .expect("lifecycle name node");
        let declaration = facts
            .source_map
            .node(lifecycle.node)
            .expect("lifecycle declaration node");
        assert_eq!(source.slice(name.range), Some("始まり"));
        assert_eq!(
            source.slice(declaration.range),
            Some("始まり() { let message: string = \"雪\"; }")
        );
        assert_eq!(
            name.range.start,
            u32::try_from(text.find("始まり").expect("keyword offset"))
                .expect("source budget fits u32")
        );
        assert!(text.is_char_boundary(
            usize::try_from(name.range.start).expect("source budget fits usize")
        ));
        assert!(
            text.is_char_boundary(
                usize::try_from(name.range.end).expect("source budget fits usize")
            )
        );
    }

    #[test]
    fn cst_recovers_without_losing_unsuffixed_fraction_text() {
        let text = "seiyaku Demo { fn invalid() { let value = 1.25; } }";
        let source = SourceFile::new(SourceId(0), "invalid-amount.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(!output.is_ok());
        assert_eq!(output.diagnostics.diagnostics[0].code, "E_AMOUNT_SUFFIX");
        assert_eq!(output.tree.text(&source), text);
        assert!(
            output
                .tree
                .tokens()
                .iter()
                .any(|token| token.kind == SyntaxKind::ErrorToken)
        );
    }

    #[test]
    fn cst_preserves_named_call_and_struct_literal_tokens() {
        let text = "seiyaku Demo { struct Pair { first: i64, second: string } fn build(first: i64) -> Pair { return Pair { second: \"two\", first, }; } fn call() { build(first: 1,); } }";
        let source = SourceFile::new(SourceId(0), "named.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        let kinds = output
            .tree
            .tokens()
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>();
        assert!(
            kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::Colon)
                .count()
                >= 5
        );
        assert!(
            kinds
                .iter()
                .filter(|kind| **kind == SyntaxKind::Comma)
                .count()
                >= 4
        );
    }

    #[test]
    fn named_source_units_do_not_synthesize_missing_tokens() {
        for text in ["seiyaku Demo {}", "誓約 Demo {}", "module Demo {}"] {
            let source = SourceFile::new(SourceId(0), "named-unit.ko", text);
            let output = parse_program(&source, FrontendBudget::v1());
            assert!(output.is_ok(), "{text}: {:?}", output.diagnostics);
            assert_eq!(output.tree.text(&source), text);
            assert!(
                output.tree.tokens().iter().all(|token| !token.is_missing()),
                "valid named source unit synthesized a recovery token: {text}"
            );
        }
    }

    #[test]
    fn statement_boundaries_lower_only_source_tokens() {
        let text = r#"seiyaku Statements {
            state value: i64;
            hajimari() {
                value = 0;
            }
            kotoage fn update(limit: i64) authorize("Update") {
                var total: i64 = 0;
                for item in range(4) {
                    if item < limit { total += item; } else { continue; }
                }
                value = total;
            }
            view fn read() -> i64 { return value; }
        }"#;
        let source = SourceFile::new(SourceId(0), "statements.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert!(output.tree.tokens().iter().all(|token| !token.is_missing()));
        assert_eq!(output.tree.text(&source), text);
    }

    #[test]
    fn cst_terminated_items_do_not_leak_recovery_tokens_into_the_ast_stream() {
        let text = r#"seiyaku Demo {
    state values: StateMap<i64, i64>;
    const limit: i64 = 2;
    view fn read() -> i64 { values.get(limit).unwrap_or(0) }
}"#;
        let source = SourceFile::new(SourceId(0), "terminated-items.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::StateItem), 1);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::ConstItem), 1);
        assert!(output.tree.tokens().iter().all(|token| !token.is_missing()));
    }

    #[test]
    fn cst_terminated_items_track_braced_initializers() {
        let text = r#"seiyaku Demo {
    struct Entry { value: i64 }
    const payload: Json = json { value: 1, };
    const entry: Entry = Entry { value: 2, };
    view fn read() -> i64 { entry.value }
}"#;
        let source = SourceFile::new(SourceId(0), "braced-consts.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::ConstItem), 2);
        assert!(output.tree.tokens().iter().all(|token| !token.is_missing()));
    }

    #[test]
    fn cst_missing_semicolon_recovers_before_an_attributed_item() {
        let text = r#"seiyaku Demo {
    const limit: i64 = 2
    #[test]
    fn check() {}
}"#;
        let source = SourceFile::new(SourceId(0), "attributed-recovery.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(!output.is_ok());
        assert_eq!(output.tree.text(&source), text);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::ConstItem), 1);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::FunctionItem), 1);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::Attribute), 1);
        assert!(output.tree.tokens().iter().any(|token| {
            token.kind == SyntaxKind::Missing && token.expected == Some(SyntaxKind::Semicolon)
        }));
    }

    #[test]
    fn cst_missing_semicolon_recovers_before_a_plain_item() {
        let text = r#"seiyaku Demo {
    const limit: i64 = 2
    fn check() {}
}"#;
        let source = SourceFile::new(SourceId(0), "plain-recovery.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(!output.is_ok());
        assert_eq!(output.tree.text(&source), text);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::ConstItem), 1);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::FunctionItem), 1);
        assert!(output.tree.tokens().iter().any(|token| {
            token.kind == SyntaxKind::Missing && token.expected == Some(SyntaxKind::Semicolon)
        }));
    }

    #[test]
    fn pseudo_item_spellings_remain_ordinary_expression_identifiers() {
        let text = r#"seiyaku Demo {
    const fixture: i64 = 1;
    const selected: i64 = fixture;
    view fn read() -> i64 { selected }
}"#;
        let source = SourceFile::new(SourceId(0), "pseudo-item-identifiers.ko", text);
        let output = parse_program(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::ConstItem), 2);
        assert!(output.tree.tokens().iter().all(|token| !token.is_missing()));
    }

    #[test]
    fn cst_recovery_preserves_mixed_call_arguments() {
        let text = "seiyaku Demo { fn invalid() { target(1, second: 2); } }";
        let source = SourceFile::new(SourceId(0), "mixed.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(!output.is_ok());
        assert_eq!(
            output.diagnostics.diagnostics[0].code,
            "E_MIXED_CALL_ARGUMENTS"
        );
        assert_eq!(output.tree.text(&source), text);
    }

    #[test]
    fn cst_structures_tail_match_arms_and_sum_patterns_losslessly() {
        let text = "seiyaku Demo { fn unwrap(value: Option<i64>) -> i64 { match value { Option::some(item) => item, Option::none => 0, } } }";
        let source = SourceFile::new(SourceId(0), "match.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        let root = output.tree.root();
        assert_eq!(count_nodes(root, SyntaxKind::TailExpr), 1);
        assert_eq!(count_nodes(root, SyntaxKind::MatchExpr), 1);
        assert_eq!(count_nodes(root, SyntaxKind::MatchArm), 2);
        assert_eq!(count_nodes(root, SyntaxKind::SumPattern), 2);
        assert!(output.tree.tokens().iter().all(|token| !token.is_missing()));
    }

    #[test]
    fn cst_structures_lists_and_comprehensions_losslessly() {
        let text = "seiyaku Demo { fn lists() -> List<i64, 4> { let source: List<i64, 4> = [1, [2].get(0).unwrap_or(0),]; [value * 2 for value in source if value > 0] } }";
        let source = SourceFile::new(SourceId(0), "lists.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        let root = output.tree.root();
        assert_eq!(count_nodes(root, SyntaxKind::ListExpr), 2);
        assert_eq!(count_nodes(root, SyntaxKind::ListComprehension), 1);
        assert!(output.tree.tokens().iter().all(|token| !token.is_missing()));
    }

    #[test]
    fn cst_structures_recursive_native_json_losslessly() {
        let text = r#"seiyaku Demo { fn build(label: string) -> Json { json { owner: "alice", labels: json ["primary", label], nested: json { "owner": 1, }, } } }"#;
        let source = SourceFile::new(SourceId(0), "json.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(output.is_ok(), "{:?}", output.diagnostics);
        assert_eq!(output.tree.text(&source), text);
        let root = output.tree.root();
        assert_eq!(count_nodes(root, SyntaxKind::JsonObjectExpr), 2);
        assert_eq!(count_nodes(root, SyntaxKind::JsonObjectEntry), 4);
        assert_eq!(count_nodes(root, SyntaxKind::JsonArrayExpr), 1);
        assert!(output.tree.tokens().iter().all(|token| !token.is_missing()));
    }

    #[test]
    fn cst_native_json_recovery_inserts_the_specific_closing_delimiter() {
        let text = "seiyaku Demo { fn invalid() -> Json { json { labels: json [1, 2; } } }";
        let source = SourceFile::new(SourceId(0), "invalid-json.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(!output.is_ok());
        assert_eq!(output.tree.text(&source), text);
        assert_eq!(
            count_nodes(output.tree.root(), SyntaxKind::JsonObjectExpr),
            1
        );
        assert_eq!(
            count_nodes(output.tree.root(), SyntaxKind::JsonArrayExpr),
            1
        );
        assert!(output.tree.tokens().iter().any(|token| {
            token.kind == SyntaxKind::Missing && token.expected == Some(SyntaxKind::RBracket)
        }));
    }

    #[test]
    fn cst_list_recovery_inserts_a_specific_closing_bracket() {
        let text = "seiyaku Demo { fn invalid() { let values = [1, 2; } }";
        let source = SourceFile::new(SourceId(0), "invalid-list.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(!output.is_ok());
        assert_eq!(output.tree.text(&source), text);
        assert_eq!(count_nodes(output.tree.root(), SyntaxKind::ListExpr), 1);
        assert!(output.tree.tokens().iter().any(|token| {
            token.kind == SyntaxKind::Missing && token.expected == Some(SyntaxKind::RBracket)
        }));
    }

    #[test]
    fn cst_snapshot_locks_expression_json_list_and_recovery_structure() {
        let text = "seiyaku C { fn f(v: Option<i64>) { let choice = v? ? 1 : 2; let payload = json { values: json [1, [x for x in [1, 2] if x > 0]; }; } }";
        let source = SourceFile::new(SourceId(0), "snapshot.ko", text);
        let output = parse(&source, FrontendBudget::v1());
        assert!(!output.is_ok(), "fixture must retain its recovery token");
        assert_eq!(output.tree.text(&source), text);
        let mut snapshot = String::new();
        cst_snapshot(output.tree.root(), &source, 0, &mut snapshot);
        assert_eq!(
            snapshot,
            r#"Root@0..134
  SourceUnit@0..132
    KwSeiyaku="seiyaku"@0..7
    Ident="C"@8..9
    LBrace="{"@10..11
    ItemList@11..131
      FunctionItem@12..129
        KwFn="fn"@12..14
        Ident="f"@15..16
        ParamList@16..32
          LParen="("@16..17
          Ident="v"@17..18
          Colon=":"@18..19
          Ident="Option"@20..26
          Less="<"@26..27
          Ident="i64"@27..30
          Greater=">"@30..31
          RParen=")"@31..32
        Block@33..129
          LBrace="{"@33..34
          StatementList@34..128
            LetStmt@35..59
              KwLet="let"@35..38
              Ident="choice"@39..45
              Equal="="@46..47
              Ident="v"@48..49
              Question="?"@49..50
              Question="?"@51..52
              Number="1"@53..54
              Colon=":"@55..56
              Number="2"@57..58
              Semicolon=";"@58..59
            LetStmt@60..127
              KwLet="let"@60..63
              Ident="payload"@64..71
              Equal="="@72..73
              JsonObjectExpr@74..126
                Ident="json"@74..78
                LBrace="{"@79..80
                JsonObjectEntry@81..126
                  Ident="values"@81..87
                  Colon=":"@87..88
                  JsonArrayExpr@89..126
                    Ident="json"@89..93
                    LBracket="["@94..95
                    Number="1"@95..96
                    Comma=","@96..97
                    ListComprehension@98..126
                      LBracket="["@98..99
                      Ident="x"@99..100
                      KwFor="for"@101..104
                      Ident="x"@105..106
                      KwIn="in"@107..109
                      ListExpr@110..116
                        LBracket="["@110..111
                        Number="1"@111..112
                        Comma=","@112..113
                        Number="2"@114..115
                        RBracket="]"@115..116
                      KwIf="if"@117..119
                      Ident="x"@120..121
                      Greater=">"@122..123
                      Number="0"@124..125
                      RBracket="]"@125..126
                    Missing(Some(RBracket))@126
              ErrorNode@126..127
                Semicolon=";"@126..127
          RBrace="}"@128..129
      ErrorNode@129..130
        Semicolon=";"@129..130
    RBrace="}"@131..132
  RBrace="}"@133..134
"#,
        );
    }
}
