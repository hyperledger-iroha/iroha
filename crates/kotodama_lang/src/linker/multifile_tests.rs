use super::*;

fn file(path: &str, source: &str) -> SourceModuleUnit {
    SourceModuleUnit {
        source_name: path.into(),
        source: source.into(),
    }
}
fn project(root: &str, sources: &[(&str, &str)]) -> SourceLinkRequest {
    SourceLinkRequest {
        root: file("app.ko", root),
        sources: sources
            .iter()
            .map(|(path, source)| file(path, source))
            .collect(),
        imports: Vec::new(),
        packages: Vec::new(),
    }
}
fn link(request: SourceLinkRequest) -> TypedProgram {
    ModuleBuildGraph::default()
        .link(request, LinkerOptions::default())
        .unwrap_or_else(|error| panic!("{}", error.into_diagnostics().render_human()))
        .program
}
fn codes(request: SourceLinkRequest) -> Vec<String> {
    ModuleBuildGraph::default()
        .link(request, LinkerOptions::default())
        .unwrap_err()
        .into_diagnostics()
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}
#[test]
fn includes_share_state_types_functions_and_original_source_ranges() {
    let request = project(
        r#"seiyaku Wallet { include "./types.ko"; include "./ops.ko"; include "./types.ko"; view fn balance() -> int { read_balance() } }"#,
        &[
            (
                "types.ko",
                "error enum Failure { #[message(\"Balance is too low\")] Low = 1001; } struct Record { int amount; Failure failure; } state int funds; hajimari() { funds = 0; }",
            ),
            (
                "ops.ko",
                "fn read_balance() -> int { funds } fn failure() -> Failure { Failure::Low }",
            ),
        ],
    );
    let typed = link(request);
    assert_eq!(typed.items.len(), 4);
    assert_eq!(typed.states.len(), 1);
    assert!(
        typed
            .error_types
            .iter()
            .any(|error| error.identity.ends_with("Wallet::Failure"))
    );
    assert_eq!(typed.error_messages[0].message, "Balance is too low");
    assert_eq!(typed.source_files.len(), 3);
    let state_source = typed.states[0].source.expect("state source");
    assert_eq!(typed.source_files[&state_source.source].name(), "types.ko");
    let function = typed
        .items
        .iter()
        .find_map(|item| {
            let TypedItem::Function(function) = item;
            (function.name == "read_balance").then_some(function)
        })
        .unwrap();
    assert_eq!(
        typed.source_files[&function.source.unwrap().source].name(),
        "ops.ko"
    );
}
#[test]
fn included_constants_follow_depth_first_directive_order() {
    let valid = project(
        r#"seiyaku App { const int BASE = 2; include "sub/a.ko"; const int LAST = NEXT + 1; view fn value() -> int { LAST } }"#,
        &[("sub/a.ko", "const int NEXT = BASE + 1;")],
    );
    link(valid);
    let invalid = project(
        r#"seiyaku App { include "sub/a.ko"; const int BASE = 2; view fn value() -> int { NEXT } }"#,
        &[("sub/a.ko", "const int NEXT = BASE + 1;")],
    );
    assert!(
        !codes(invalid).is_empty(),
        "includes must not make forward constants legal"
    );
}
#[test]
fn nested_local_imports_export_functions_constants_and_nominal_types() {
    let request = project(
        r#"seiyaku App { import "lib/math.ko" as arithmetic; view fn value(arithmetic::Quote quote) -> int { arithmetic::twice(quote.amount) + arithmetic::SCALE } }"#,
        &[
            (
                "lib/math.ko",
                r#"module Math { import "../base.ko" as base; include "quote.ko"; export const int SCALE = base::FACTOR; export fn twice(int _ value) -> int { base::double(value) } }"#,
            ),
            (
                "lib/quote.ko",
                "export struct Quote { int amount; } export error enum Failure { #[message(\"Invalid quote\")] Invalid = 7; }",
            ),
            (
                "base.ko",
                "module Base { export const int FACTOR = 2; export fn double(int _ value) -> int { value * FACTOR } }",
            ),
        ],
    );
    let typed = link(request.clone());
    assert_eq!(typed.source_files.len(), 4);
    let error = typed
        .error_types
        .iter()
        .find(|error| error.identity.ends_with("::Math::Failure"))
        .expect("local error");
    assert!(error.identity.starts_with("local::"));
    assert!(error.identity.ends_with("::Math::Failure"));
    let mut alias = request.clone();
    alias.root.source = alias
        .root
        .source
        .replace("as arithmetic", "as other")
        .replace("arithmetic::", "other::");
    alias.sources.reverse();
    assert_eq!(typed.error_types, link(alias).error_types);
    let mut wording = request.clone();
    wording.sources[1].source = wording.sources[1]
        .source
        .replace("Invalid quote", "Quote invalid");
    assert_eq!(typed.error_types, link(wording).error_types);
    let mut moved = request;
    moved.root.source = moved.root.source.replace("lib/math.ko", "lib/renamed.ko");
    moved.sources[0].source_name = "lib/renamed.ko".into();
    assert_ne!(typed.error_types, link(moved).error_types);
}
#[test]
fn local_imports_require_explicit_exports_and_cannot_read_contract_state() {
    assert!(codes(project(r#"seiyaku App { import "math.ko" as arithmetic; view fn value() -> int { arithmetic::hidden() } }"#,
        &[("math.ko", "module Math { fn hidden() -> int { 1 } }")])).contains(&"E_UNEXPORTED_SYMBOL".into()));
    assert!(!codes(project(r#"seiyaku App { state int value; import "math.ko" as arithmetic; view fn read() -> int { arithmetic::read() } }"#,
        &[("math.ko", "module Math { export fn read() -> int { value } }")])).is_empty());
}
#[test]
fn graph_rejects_cycles_missing_files_and_shared_fragment_ownership() {
    let fixtures = [
        (
            project(
                r#"seiyaku App { include "a.ko"; }"#,
                &[
                    ("a.ko", r#"include "b.ko";"#),
                    ("b.ko", r#"include "a.ko";"#),
                ],
            ),
            "E_INCLUDE_CYCLE",
        ),
        (
            project(
                r#"seiyaku App { import "a.ko" as a; }"#,
                &[
                    ("a.ko", r#"module A { import "b.ko" as b; }"#),
                    ("b.ko", r#"module B { import "a.ko" as a; }"#),
                ],
            ),
            "E_LOCAL_IMPORT_CYCLE",
        ),
        (
            project(r#"seiyaku App { include "missing.ko"; }"#, &[]),
            "E_SOURCE_NOT_FOUND",
        ),
        (
            project(
                r#"seiyaku App { include "shared.ko"; import "math.ko" as arithmetic; }"#,
                &[
                    ("shared.ko", "const int ONE = 1;"),
                    ("math.ko", r#"module Math { include "shared.ko"; }"#),
                ],
            ),
            "E_SOURCE_OWNERSHIP",
        ),
        (
            project(
                r#"seiyaku App { include "module.ko"; }"#,
                &[("module.ko", "module Helper {}")],
            ),
            "E_SOURCE_UNIT_KIND",
        ),
        (
            project(r#"seiyaku App { include "../escape.ko"; }"#, &[]),
            "E_INVALID_SOURCE_PATH",
        ),
    ];
    for (request, expected) in fixtures {
        assert!(codes(request).contains(&expected.into()), "{expected}");
    }
}
#[test]
fn semantic_errors_in_fragments_retain_native_file_text() {
    let request = project(
        r#"seiyaku App { include "broken.ko"; }"#,
        &[("broken.ko", "view fn value() -> int { true }")],
    );
    let diagnostics = ModuleBuildGraph::default()
        .link(request, LinkerOptions::default())
        .unwrap_err()
        .into_diagnostics();
    assert!(diagnostics.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .primary_span
            .as_ref()
            .and_then(|span| span.source.as_deref())
            == Some("broken.ko")
    }));
    assert!(diagnostics.render_human().contains("view fn value()"));
}
#[test]
fn package_constants_require_both_source_and_manifest_exports() {
    let mut request = project(
        "seiyaku App { view fn value() -> int { arithmetic::SCALE } }",
        &[],
    );
    request.imports.push(ImportBinding {
        alias: "arithmetic".into(),
        package: "demo/math@1".into(),
    });
    request.packages.push(SourcePackageUnit {
        identity: "demo/math@1".into(),
        modules: vec![file(
            "lib.ko",
            "module Math { export const int SCALE = 10; }",
        )],
        sources: Vec::new(),
        exports: BTreeSet::from(["SCALE".into()]),
        imports: Vec::new(),
    });
    link(request.clone());
    request.packages[0].modules[0].source = "module Math { const int SCALE = 10; }".into();
    assert!(codes(request).contains(&"E_UNEXPORTED_SYMBOL".into()));
}
#[test]
fn standalone_tests_include_helpers_and_import_local_modules() {
    let request = project(
        "seiyaku App { state int count; hajimari() { count = 0; } fn value() -> int { count } }",
        &[
            (
                "tests/checks.ko",
                "#[test] fn value_matches() { test::assert(value() == calc::ZERO); test::assert(helper() == 0); } fn helper() -> int { count }",
            ),
            (
                "tests/math.ko",
                "module Math { export const int ZERO = 0; }",
            ),
        ],
    );
    let test = file(
        "tests/unit.ko",
        r#"module Tests { koto_test { target: "../app.ko" } include "checks.ko"; import "math.ko" as calc; }"#,
    );
    let typed = ModuleBuildGraph::default()
        .link_sources_inner(
            request,
            &[test],
            LinkerOptions {
                test_builtins_enabled: true,
                include_tests: true,
                ..LinkerOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("{}", error.into_diagnostics().render_human()))
        .program;
    assert_eq!(typed.source_files.len(), 4);
    assert!(typed.items.iter().any(|item| {
        let TypedItem::Function(function) = item;
        function.name == "value_matches"
    }));
}
#[test]
fn fingerprints_cover_reachable_sources_only_but_bound_the_entire_inventory() {
    let graph = ModuleBuildGraph::default();
    let request = project(
        r#"seiyaku App { include "value.ko"; }"#,
        &[("value.ko", "view fn value() -> int { 1 }")],
    );
    let before = ModuleBuildGraph::fingerprint(&request).unwrap();
    let mut extra = request.clone();
    extra
        .sources
        .push(file("unused.ko", "malformed unused content"));
    assert_eq!(before, ModuleBuildGraph::fingerprint(&extra).unwrap());
    assert_eq!(
        graph
            .link(extra.clone(), LinkerOptions::default())
            .unwrap()
            .program
            .source_files
            .len(),
        2
    );
    extra.sources[0].source = "view fn value() -> int { 2 }".into();
    assert_ne!(before, ModuleBuildGraph::fingerprint(&extra).unwrap());
    extra.sources[1].source = " ".repeat(crate::source::MAX_SOURCE_BYTES + 1);
    assert_eq!(
        ModuleBuildGraph::fingerprint(&extra)
            .unwrap_err()
            .diagnostic_code(),
        "K0001"
    );
}
#[test]
fn included_contract_fragment_cannot_export_declarations() {
    assert!(
        codes(project(
            r#"seiyaku App { include "value.ko"; }"#,
            &[("value.ko", "export const int VALUE = 1;")]
        ))
        .contains(&"E_SOURCE_UNIT_KIND".into())
    );
}
#[test]
fn splitting_declarations_preserves_artifact_and_durable_interface() {
    let declarations = "error enum Failure { #[message(\"Balance too low\")] Low = 1001; } state int funds; hajimari() { funds = 0; }";
    let functions = "fn read_balance() -> int { funds } view fn balance() -> int { read_balance() } view fn failure() -> Failure { Failure::Low }";
    let session = crate::session::CompilerSession::new(crate::compiler::CompilerOptions::default());
    let single = session
        .build_source_bundle(project(
            &format!("seiyaku Wallet {{ {declarations} {functions} }}"),
            &[],
        ))
        .unwrap();
    let split = session
        .build_source_bundle(project(
            r#"seiyaku Wallet { include "state.ko"; include "functions.ko"; }"#,
            &[("state.ko", declarations), ("functions.ko", functions)],
        ))
        .unwrap();
    assert_eq!(single.contract_interface, split.contract_interface);
    assert_eq!(single.artifact, split.artifact);
}
#[test]
fn test_fingerprint_includes_test_only_companions() {
    let graph = ModuleBuildGraph::default();
    let mut request = project(
        "seiyaku App { fn value() -> int { 1 } }",
        &[(
            "checks.ko",
            "#[test] fn check() { test::assert(value() == 1); }",
        )],
    );
    let test = file(
        "tests.ko",
        r#"module Tests { koto_test { target: "app.ko" } include "checks.ko"; }"#,
    );
    let options = LinkerOptions {
        test_builtins_enabled: true,
        include_tests: true,
        ..LinkerOptions::default()
    };
    let before = graph
        .link_sources_inner(request.clone(), std::slice::from_ref(&test), options)
        .unwrap();
    request.sources[0].source = request.sources[0].source.replace("== 1", "== 2");
    let after = graph.link_sources_inner(request, &[test], options).unwrap();
    assert_ne!(before.fingerprint, after.fingerprint);
}
