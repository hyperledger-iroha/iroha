//! Parser recovery, progress, and diagnostic-fanout coverage.
use kotodama_lang::{
    diagnostic::DiagnosticPhase,
    source::{FrontendBudget, MAX_DIAGNOSTICS, SourceFile, SourceId},
    syntax::{SyntaxKind, parse},
};
use std::fmt::Write as _;
#[test]
fn recovers_multiple_errors_inside_one_block_and_across_items() {
    let text = r#"seiyaku Broken {
        fn first() {
            let int first = ;
            let bool second = ;
            return;
        }
        fn second() {
            let int third = ;
        }
    }"#;
    let source = SourceFile::new(SourceId(1), "recovery.ko", text);
    let output = parse(&source, FrontendBudget::v1());
    assert_eq!(output.tree.text(&source), text);
    let parse_errors = output
        .diagnostics
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.phase == DiagnosticPhase::Parse)
        .count();
    assert!(parse_errors >= 3, "{:?}", output.diagnostics.diagnostics);
    assert!(
        output
            .tree
            .tokens()
            .into_iter()
            .filter(|token| token.kind == SyntaxKind::Missing)
            .count()
            >= 3
    );
}
#[test]
fn malformed_delimiters_keep_the_complete_tree_and_make_progress() {
    let text = "seiyaku Broken { fn f() { let value = (1 + 2; let next = ; } }";
    let source = SourceFile::new(SourceId(2), "delimiters.ko", text);
    let output = parse(&source, FrontendBudget::v1());
    assert_eq!(output.tree.text(&source), text);
    assert!(!output.diagnostics.diagnostics.is_empty());
    assert_eq!(
        output
            .tree
            .tokens()
            .into_iter()
            .filter(|token| token.kind == SyntaxKind::Eof)
            .count(),
        1
    );
}
#[test]
fn adversarial_error_fanout_is_bounded_and_reported() {
    let mut text = String::from("seiyaku Many { fn f() {\n");
    for index in 0..80 {
        writeln!(text, "let value_{index}: int = ;").expect("write fixture");
    }
    text.push_str("} }");
    let source = SourceFile::new(SourceId(3), "fanout.ko", text);
    let output = parse(&source, FrontendBudget::v1());
    assert_eq!(output.diagnostics.diagnostics.len(), MAX_DIAGNOSTICS);
    let limit = output
        .diagnostics
        .diagnostics
        .last()
        .expect("limit diagnostic");
    assert_eq!(limit.code, "K0004");
    assert!(limit.message.contains("additional syntax error"));
}
#[test]
fn lexical_error_fanout_counts_every_omitted_diagnostic() {
    let text = format!("seiyaku Many {{ fn f() {{ {} }} }}", "@".repeat(80));
    let source = SourceFile::new(SourceId(4), "lex-fanout.ko", text);
    let output = parse(&source, FrontendBudget::v1());
    assert_eq!(output.diagnostics.diagnostics.len(), MAX_DIAGNOSTICS);
    let limit = output
        .diagnostics
        .diagnostics
        .last()
        .expect("limit diagnostic");
    assert_eq!(limit.code, "K0004");
    assert!(
        limit.message.contains("17 additional syntax error(s)"),
        "{}",
        limit.message
    );
}
