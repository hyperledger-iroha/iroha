//! Fixed V1 frontend budget and non-recursive recovery coverage.

use kotodama_lang::{
    source::{
        FrontendBudget, MAX_NESTING_DEPTH, MAX_SOURCE_BYTES, MAX_TOKENS, SourceFile, SourceId,
    },
    syntax::{lex, parse},
};

fn has_code(diagnostics: &[kotodama_lang::diagnostic::Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

#[test]
fn oversized_source_is_one_lossless_budget_error_region() {
    let text = " ".repeat(MAX_SOURCE_BYTES + 1);
    let source = SourceFile::new(SourceId(1), "oversized.ko", text.clone());
    let output = parse(&source, FrontendBudget::v1());

    assert_eq!(output.tree.text(&source), text);
    assert!(has_code(&output.diagnostics.diagnostics, "K0001"));
}

#[test]
fn token_budget_accepts_the_boundary_and_rejects_one_more() {
    let boundary_text = "a ".repeat(MAX_TOKENS - 1);
    let boundary = SourceFile::new(SourceId(2), "token-boundary.ko", boundary_text);
    let boundary_lexed = lex(&boundary, FrontendBudget::v1());
    assert!(!has_code(&boundary_lexed.diagnostics, "K0002"));

    let excessive_text = "a ".repeat(MAX_TOKENS);
    let excessive = SourceFile::new(SourceId(3), "too-many-tokens.ko", excessive_text);
    let excessive_lexed = lex(&excessive, FrontendBudget::v1());
    assert!(has_code(&excessive_lexed.diagnostics, "K0002"));
}

#[test]
fn delimiter_depth_boundary_is_deterministic() {
    let available = MAX_NESTING_DEPTH - 2;
    let boundary_text = format!(
        "seiyaku Demo {{ fn f() {{ let value = {}true{}; }} }}",
        "(".repeat(available),
        ")".repeat(available)
    );
    let boundary = SourceFile::new(SourceId(4), "depth-boundary.ko", boundary_text);
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert!(!has_code(&boundary_output.diagnostics.diagnostics, "K0003"));

    let excessive_text = format!(
        "seiyaku Demo {{ fn f() {{ let value = {}true{}; }} }}",
        "(".repeat(available + 1),
        ")".repeat(available + 1)
    );
    let excessive = SourceFile::new(SourceId(5), "too-deep.ko", excessive_text);
    let excessive_output = parse(&excessive, FrontendBudget::v1());
    assert!(has_code(&excessive_output.diagnostics.diagnostics, "K0003"));
}

#[test]
fn long_unary_chain_hits_depth_budget_without_recursive_parsing() {
    let available = MAX_NESTING_DEPTH - 2;
    let boundary_text = format!(
        "seiyaku Demo {{ fn f() {{ let value = {}true; }} }}",
        "!".repeat(available)
    );
    let boundary = SourceFile::new(SourceId(6), "unary-boundary.ko", boundary_text);
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert!(!has_code(&boundary_output.diagnostics.diagnostics, "K0003"));

    let text = format!(
        "seiyaku Demo {{ fn f() {{ let value = {}true; }} }}",
        "!".repeat(available + 1)
    );
    let source = SourceFile::new(SourceId(7), "unary-depth.ko", text);
    let output = parse(&source, FrontendBudget::v1());

    assert!(has_code(&output.diagnostics.diagnostics, "K0003"));
}

#[test]
fn deeply_nested_type_hits_depth_budget_without_recursive_parsing() {
    let available = MAX_NESTING_DEPTH - 1;
    let mut ty = String::from("int");
    for _ in 0..available {
        ty = format!("Option<{ty}>");
    }
    let boundary_text = format!("seiyaku Demo {{ state value: {ty}; }}");
    let boundary = SourceFile::new(SourceId(8), "type-boundary.ko", boundary_text);
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert!(!has_code(&boundary_output.diagnostics.diagnostics, "K0003"));

    ty = format!("Option<{ty}>");
    let text = format!("seiyaku Demo {{ state value: {ty}; }}");
    let source = SourceFile::new(SourceId(9), "type-depth.ko", text);
    let output = parse(&source, FrontendBudget::v1());

    assert!(has_code(&output.diagnostics.diagnostics, "K0003"));
}

#[test]
fn mixed_grouping_and_prefixes_share_one_depth_budget() {
    let grouping = 100;
    let prefixes = MAX_NESTING_DEPTH - 2 - grouping;
    let boundary_text = format!(
        "seiyaku Demo {{ fn f() {{ let value = {}{}true{}; }} }}",
        "(".repeat(grouping),
        "!".repeat(prefixes),
        ")".repeat(grouping),
    );
    let boundary = SourceFile::new(SourceId(10), "mixed-boundary.ko", boundary_text);
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert!(!has_code(&boundary_output.diagnostics.diagnostics, "K0003"));

    let excessive_text = format!(
        "seiyaku Demo {{ fn f() {{ let value = {}{}true{}; }} }}",
        "(".repeat(grouping),
        "!".repeat(prefixes + 1),
        ")".repeat(grouping),
    );
    let excessive = SourceFile::new(SourceId(11), "mixed-depth.ko", excessive_text);
    let excessive_output = parse(&excessive, FrontendBudget::v1());
    assert!(has_code(&excessive_output.diagnostics.diagnostics, "K0003"));
}

#[test]
fn conditional_chains_are_iterative_and_share_the_depth_budget() {
    let available = MAX_NESTING_DEPTH - 2;
    let boundary_expression = format!("true{}", " ? true : true".repeat(available));
    let boundary_text =
        format!("seiyaku Demo {{ fn f() {{ let value = {boundary_expression}; }} }}");
    let boundary = SourceFile::new(SourceId(12), "conditional-boundary.ko", boundary_text);
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert!(!has_code(&boundary_output.diagnostics.diagnostics, "K0003"));

    let excessive_expression = format!("true{}", " ? true : true".repeat(available + 1));
    let excessive_text =
        format!("seiyaku Demo {{ fn f() {{ let value = {excessive_expression}; }} }}");
    let excessive = SourceFile::new(SourceId(13), "conditional-depth.ko", excessive_text);
    let excessive_output = parse(&excessive, FrontendBudget::v1());
    assert!(has_code(&excessive_output.diagnostics.diagnostics, "K0003"));
}
