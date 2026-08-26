//! Fixed V1 frontend budget and non-recursive recovery coverage.
use kotodama_lang::{
    source::{
        FrontendBudget, MAX_NESTING_DEPTH, MAX_SOURCE_BYTES, MAX_TOKENS, SourceFile, SourceId,
    },
    syntax::{lex, parse, parse_program},
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
    let boundary_text = format!("seiyaku Demo {{ state {ty} value; }}");
    let boundary = SourceFile::new(SourceId(8), "type-boundary.ko", boundary_text);
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert!(
        boundary_output.diagnostics.diagnostics.is_empty(),
        "accepted depth boundary emitted diagnostics: {:?}",
        boundary_output.diagnostics.diagnostics
    );
    ty = format!("Option<{ty}>");
    let text = format!("seiyaku Demo {{ state {ty} value; }}");
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

fn function_source(body: &str) -> String {
    format!("seiyaku Demo {{ fn f() {{ {body} }} }}")
}

fn mixed_postfix_expression(links: usize) -> String {
    let mut expression = String::from("root");
    for index in 0..links {
        expression.push_str(match index % 3 {
            0 => ".field",
            1 => "[0]",
            _ => ".step()",
        });
    }
    expression
}

fn assert_expression_depth_boundary(
    label: &str,
    boundary_body: String,
    excessive_body: String,
    source_id: u32,
) {
    let boundary_text = function_source(&boundary_body);
    let boundary = SourceFile::new(
        SourceId(source_id),
        format!("{label}-boundary.ko"),
        boundary_text.clone(),
    );
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert_eq!(boundary_output.tree.text(&boundary), boundary_text);
    assert!(
        boundary_output.diagnostics.diagnostics.is_empty(),
        "{label} boundary emitted diagnostics: {:?}",
        boundary_output.diagnostics.diagnostics
    );

    let excessive_text = function_source(&excessive_body);
    let excessive = SourceFile::new(
        SourceId(source_id + 1),
        format!("{label}-excessive.ko"),
        excessive_text.clone(),
    );
    let excessive_output = parse(&excessive, FrontendBudget::v1());
    assert_eq!(excessive_output.tree.text(&excessive), excessive_text);
    assert!(
        has_code(&excessive_output.diagnostics.diagnostics, "K0003"),
        "{label} chain above the boundary must be rejected"
    );
    let excessive_program = parse_program(&excessive, FrontendBudget::v1());
    assert!(
        excessive_program.program.is_none(),
        "{label} chain above the boundary must not return a public AST"
    );
}

fn assert_depth_rejected_losslessly(label: &str, body: String, source_id: u32) {
    let text = function_source(&body);
    let source = SourceFile::new(SourceId(source_id), format!("{label}.ko"), text.clone());
    let output = parse(&source, FrontendBudget::v1());
    assert_eq!(output.tree.text(&source), text);
    assert!(
        has_code(&output.diagnostics.diagnostics, "K0003"),
        "{label} must retain the nesting diagnostic: {:?}",
        output.diagnostics.diagnostics
    );
    let program = parse_program(&source, FrontendBudget::v1());
    assert!(
        program.program.is_none(),
        "{label} must not return an over-depth AST"
    );
}

#[test]
fn flat_expression_shapes_share_the_depth_budget_on_a_small_stack() {
    std::thread::Builder::new()
        .name("kotodama-expression-depth".into())
        .stack_size(128 * 1024)
        .spawn(|| {
            let boundary = MAX_NESTING_DEPTH - 2;
            let excessive = boundary + 1;
            assert_expression_depth_boundary(
                "binary",
                format!("let value = 0{};", " + 1".repeat(boundary)),
                format!("let value = 0{};", " + 1".repeat(excessive)),
                14,
            );
            assert_expression_depth_boundary(
                "member",
                format!("let value = root{};", ".field".repeat(boundary)),
                format!("let value = root{};", ".field".repeat(excessive)),
                16,
            );
            assert_expression_depth_boundary(
                "method",
                format!("let value = root{};", ".step()".repeat(boundary)),
                format!("let value = root{};", ".step()".repeat(excessive)),
                18,
            );
            assert_expression_depth_boundary(
                "index",
                format!("let value = root{};", "[0]".repeat(boundary)),
                format!("let value = root{};", "[0]".repeat(excessive)),
                20,
            );
            assert_expression_depth_boundary(
                "assignment-target",
                format!("root{} = 1;", ".field".repeat(boundary)),
                format!("root{} = 1;", ".field".repeat(excessive)),
                22,
            );
            assert_expression_depth_boundary(
                "mixed-postfix",
                format!("let value = {};", mixed_postfix_expression(boundary)),
                format!("let value = {};", mixed_postfix_expression(excessive)),
                24,
            );
            let grouping = 100;
            let grouped_boundary = boundary - grouping;
            assert_expression_depth_boundary(
                "grouped-binary",
                format!(
                    "let value = {}0{}{};",
                    "(".repeat(grouping),
                    " + 1".repeat(grouped_boundary),
                    ")".repeat(grouping),
                ),
                format!(
                    "let value = {}0{}{};",
                    "(".repeat(grouping),
                    " + 1".repeat(grouped_boundary + 1),
                    ")".repeat(grouping),
                ),
                26,
            );
            assert_depth_rejected_losslessly(
                "missing-semicolon",
                format!("let value = 0{}", " + 1".repeat(excessive)),
                28,
            );
            assert_depth_rejected_losslessly(
                "malformed-postfix",
                format!("let value = root{}.", ".field".repeat(excessive)),
                29,
            );
            assert_depth_rejected_losslessly(
                "speculative-lvalue",
                format!("root{};", ".field".repeat(excessive)),
                30,
            );
        })
        .expect("spawn small-stack parser worker")
        .join()
        .expect("small-stack parser worker");
}

fn assert_source_depth_boundary(
    label: &str,
    boundary_text: String,
    excessive_text: String,
    source_id: u32,
) {
    let boundary = SourceFile::new(
        SourceId(source_id),
        format!("{label}-boundary.ko"),
        boundary_text,
    );
    let boundary_output = parse(&boundary, FrontendBudget::v1());
    assert!(
        boundary_output.diagnostics.diagnostics.is_empty(),
        "{label} boundary emitted diagnostics: {:?}",
        boundary_output.diagnostics.diagnostics
    );
    let excessive = SourceFile::new(
        SourceId(source_id + 1),
        format!("{label}-excessive.ko"),
        excessive_text,
    );
    let excessive_output = parse_program(&excessive, FrontendBudget::v1());
    assert!(
        has_code(&excessive_output.diagnostics.diagnostics, "K0003"),
        "{label} must reject one level above its context boundary"
    );
    assert!(excessive_output.program.is_none());
}

#[test]
fn expression_depth_accounts_for_const_fixture_and_trigger_contexts() {
    let const_links = MAX_NESTING_DEPTH - 1;
    assert_source_depth_boundary(
        "const-expression",
        format!(
            "module Demo {{ const VALUE = 0{}; }}",
            " + 1".repeat(const_links)
        ),
        format!(
            "module Demo {{ const VALUE = 0{}; }}",
            " + 1".repeat(const_links + 1)
        ),
        31,
    );

    let nested_links = MAX_NESTING_DEPTH - 3;
    assert_source_depth_boundary(
        "fixture-expression",
        format!(
            "module Tests {{ fixture actors {{ caller(0{}); }} }}",
            " + 1".repeat(nested_links)
        ),
        format!(
            "module Tests {{ fixture actors {{ caller(0{}); }} }}",
            " + 1".repeat(nested_links + 1)
        ),
        33,
    );
    assert_source_depth_boundary(
        "trigger-metadata-expression",
        format!(
            "seiyaku Demo {{ kotoage fn run() authorize(\"Run\") {{}} trigger wake -> run {{ on time pre_commit; metadata {{ payload: 0{}; }} }} }}",
            " + 1".repeat(nested_links)
        ),
        format!(
            "seiyaku Demo {{ kotoage fn run() authorize(\"Run\") {{}} trigger wake -> run {{ on time pre_commit; metadata {{ payload: 0{}; }} }} }}",
            " + 1".repeat(nested_links + 1)
        ),
        35,
    );
}

fn else_if_source(branches: usize) -> String {
    format!(
        "seiyaku Demo {{ fn f() {{ {}{{}} }} }}",
        "if true {} else ".repeat(branches)
    )
}

#[test]
fn delimiter_free_else_if_recursion_shares_the_depth_budget() {
    let boundary_branches = MAX_NESTING_DEPTH - 1;
    assert_source_depth_boundary(
        "else-if",
        else_if_source(boundary_branches),
        else_if_source(boundary_branches + 1),
        37,
    );
}

#[test]
fn nesting_diagnostic_survives_the_diagnostic_cap() {
    let mut items = String::new();
    for index in 0..63 {
        items.push_str(&format!("const VALUE_{index} = Amount;"));
    }
    let excessive = MAX_NESTING_DEPTH - 1;
    let text = format!(
        "module Demo {{ {items} fn f() {{ let value = 0{}; }} }}",
        " + 1".repeat(excessive)
    );
    let source = SourceFile::new(SourceId(39), "diagnostic-cap.ko", text);
    let output = parse_program(&source, FrontendBudget::v1());
    assert!(output.program.is_none());
    assert!(has_code(&output.diagnostics.diagnostics, "K0003"));
    assert!(has_code(&output.diagnostics.diagnostics, "K0004"));
}
