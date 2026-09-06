//! Source graph fixtures, including independent rustc module-resolution checks.

use super::*;

fn put(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn graph(root: &Path) -> ContextGraph {
    inventory_contexts(root, &["src/entry.rs".into()]).unwrap()
}

fn compile(root: &Path) {
    let output =
        std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args([
                "--edition=2024",
                "--crate-type=lib",
                "--emit=metadata",
                "--crate-name=graph_fixture",
            ])
            .arg(root.join("src/entry.rs"))
            .arg("--out-dir")
            .arg(root)
            .output()
            .expect("run the installed rustc for independent source-resolution verification");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn module_directories_and_include_fragments_match_real_rustc_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        r#"
mod plain;
#[path="relocated/odd.rs"] mod named;
#[path="custom"] mod inline_attr { pub mod child; #[path="other.rs"] pub mod branch; }
include!("fragments/items.inc");
mod r#type;
const _: [(); 2] = [(); plain::nested::VALUE];
const _: [(); 3] = [(); plain::inlined::leaf::VALUE];
const _: [(); 5] = [(); named::nested::VALUE];
const _: [(); 7] = [(); inline_attr::child::VALUE];
const _: [(); 11] = [(); inline_attr::branch::VALUE];
const _: [(); 13] = [(); included::VALUE];
const _: [(); 17] = [(); r#type::VALUE];
"#,
    );
    put(
        root,
        "src/plain.rs",
        "pub mod nested; pub mod inlined { #[path=\"leaf.rs\"] pub mod leaf; }",
    );
    put(root, "src/plain/nested.rs", "pub const VALUE: usize = 2;");
    put(
        root,
        "src/plain/inlined/leaf.rs",
        "pub const VALUE: usize = 3;",
    );
    put(root, "src/relocated/odd.rs", "pub mod nested;");
    put(
        root,
        "src/relocated/nested.rs",
        "pub const VALUE: usize = 5;",
    );
    put(root, "src/custom/child.rs", "pub const VALUE: usize = 7;");
    put(root, "src/custom/other.rs", "pub const VALUE: usize = 11;");
    put(root, "src/fragments/items.inc", "mod included;");
    put(
        root,
        "src/fragments/included.rs",
        "pub const VALUE: usize = 13;",
    );
    put(root, "src/type.rs", "pub const VALUE: usize = 17;");
    // Wrong directory interpretations would silently select different bytes.
    put(
        root,
        "src/relocated/odd/nested.rs",
        "pub const VALUE: usize = 99;",
    );
    put(
        root,
        "src/fragments/items/included.rs",
        "pub const VALUE: usize = 99;",
    );
    compile(root);
    let report = graph(root);
    assert!(!report.invalid_sources);
    let files: Vec<_> = report.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(files.len(), 11);
    assert!(files.contains(&"src/relocated/nested.rs"));
    assert!(files.contains(&"src/fragments/included.rs"));
    assert!(files.contains(&"src/type.rs"));
    assert!(!files.contains(&"src/relocated/odd/nested.rs"));
    assert!(!files.contains(&"src/fragments/items/included.rs"));
    assert!(report.edges.iter().all(|edge| edge.state == "resolved"));
}

#[test]
fn inline_path_overrides_in_non_mod_files_match_real_rustc() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "mod plain; const _: [(); 23] = [(); plain::thread::leaf::VALUE];",
    );
    put(
        root,
        "src/plain.rs",
        "#[path=\"threads\"] pub mod thread { #[path=\"leaf.rs\"] pub mod leaf; }",
    );
    put(root, "src/threads/leaf.rs", "pub const VALUE: usize = 23;");
    put(
        root,
        "src/plain/threads/leaf.rs",
        "pub const VALUE: usize = 99;",
    );
    compile(root);
    let report = graph(root);
    assert!(
        report
            .files
            .iter()
            .any(|file| file.path == "src/threads/leaf.rs")
    );
    assert!(
        !report
            .files
            .iter()
            .any(|file| file.path == "src/plain/threads/leaf.rs")
    );
}

#[test]
fn mod_rs_child_layout_matches_real_rustc() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "mod directory; const _: [(); 29] = [(); directory::child::VALUE];",
    );
    put(root, "src/directory/mod.rs", "pub mod child;");
    put(
        root,
        "src/directory/child.rs",
        "pub const VALUE: usize = 29;",
    );
    compile(root);
    let report = graph(root);
    assert_eq!(report.files.len(), 3);
    assert!(report.edges.iter().all(|edge| edge.state == "resolved"));
    let directory = report
        .contexts
        .iter()
        .find(|context| context.file == "src/directory/mod.rs")
        .unwrap();
    assert_eq!(directory.module_directory, "src/directory");
}

#[test]
fn multiple_inclusion_retains_each_valid_context_and_one_physical_record() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "mod left { include!(\"shared.inc\"); } mod right { include!(\"shared.inc\"); } const _: [(); left::VALUE] = [(); right::VALUE];",
    );
    put(
        root,
        "src/shared.inc",
        "pub mod child; pub const VALUE: usize = child::VALUE;",
    );
    put(root, "src/child.rs", "pub const VALUE: usize = 7;");
    compile(root);
    let report = graph(root);
    assert_eq!(report.files.len(), 3);
    let includes: Vec<_> = report
        .contexts
        .iter()
        .filter(|context| context.kind == "include")
        .collect();
    assert_eq!(includes.len(), 2);
    assert_eq!(includes[0].lexical_modules, ["left"]);
    assert_eq!(includes[1].lexical_modules, ["right"]);
    assert!(includes.iter().all(|context| context.state == "visited"));
    assert_eq!(
        report
            .contexts
            .iter()
            .filter(|context| context.file == "src/child.rs")
            .count(),
        2
    );
    let repeated = report
        .review
        .iter()
        .find(|entry| entry.kind == "multiple_inclusion")
        .unwrap();
    assert_eq!(repeated.related_contexts, [includes[0].id]);
    assert!(!report.invalid_sources);
}

#[test]
fn ordinary_cfg_branches_are_traversed_without_selecting_a_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "#[cfg(feature=\"a\")] #[path=\"a.rs\"] mod value; #[cfg(not(feature=\"a\"))] #[path=\"b.rs\"] mod value;",
    );
    put(root, "src/a.rs", "#![cfg(unix)] struct A;");
    put(root, "src/b.rs", "struct B;");
    let report = graph(root);
    let a = report
        .contexts
        .iter()
        .find(|context| context.file == "src/a.rs")
        .unwrap();
    assert_eq!(a.conditions, ["#[cfg(feature=\"a\")]", "#![cfg(unix)]"]);
    assert!(
        report
            .contexts
            .iter()
            .any(|context| context.file == "src/b.rs")
    );
}

#[test]
fn nested_method_expression_and_statement_cfgs_reach_fragment_contexts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "struct Owner; impl Owner { #[cfg(a)] fn f() { #[cfg(b)] let _ = { #[cfg(c)] include!(\"value.inc\"); }; } } trait Trait { #[cfg(d)] fn f() { #[cfg(e)] { let _ = include!(\"value.inc\"); } } }",
    );
    put(root, "src/value.inc", "7");
    let report = graph(root);
    let includes: Vec<_> = report
        .contexts
        .iter()
        .filter(|context| context.kind == "include")
        .collect();
    assert_eq!(
        includes[0].conditions,
        ["#[cfg(a)]", "#[cfg(b)]", "#[cfg(c)]"]
    );
    assert_eq!(includes[1].conditions, ["#[cfg(d)]", "#[cfg(e)]"]);
    assert!(
        includes
            .iter()
            .all(|context| context.local_block && context.state == "parse_review")
    );
}

#[test]
fn conditional_dynamic_and_ambiguous_paths_remain_visible_without_guesses() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "#[cfg_attr(a,path=\"other.rs\")] mod conditional; mod both; include!(concat!(\"generated\", \".rs\")); make_modules!();",
    );
    put(root, "src/conditional.rs", "struct ShouldNotSelect;");
    put(root, "src/other.rs", "struct NorThis;");
    put(root, "src/both.rs", "struct Flat;");
    put(root, "src/both/mod.rs", "struct Nested;");
    let report = graph(root);
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.edges.len(), 3);
    assert!(report.edges.iter().all(|edge| edge.to.is_none()));
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.kind == "macro_expansion")
    );
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.reason.contains("both name.rs"))
    );
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.reason.contains("conditional module"))
    );
}

#[test]
fn cycles_are_visible_and_do_not_erase_other_paths() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "include!(\"cycle.inc\"); mod regular;",
    );
    put(root, "src/cycle.inc", "include!(\"entry.rs\");");
    put(root, "src/regular.rs", "struct Regular;");
    let report = graph(root);
    assert_eq!(report.files.len(), 3);
    let cycle = report
        .contexts
        .iter()
        .find(|context| context.state == "cycle")
        .unwrap();
    assert_eq!(cycle.file, "src/entry.rs");
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.kind == "cycle" && entry.related_contexts == [0])
    );
}

#[test]
fn literal_expression_fragments_are_hashed_without_claiming_an_item_parse() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "const VALUE: u32 = include!(\"value.inc\");",
    );
    put(root, "src/value.inc", "7 + 2");
    compile(root);
    let report = graph(root);
    assert_eq!(report.files.len(), 2);
    assert!(
        report
            .files
            .iter()
            .find(|file| file.path == "src/value.inc")
            .unwrap()
            .parse_error
            .is_some()
    );
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.kind == "parse_fragment")
    );
    assert!(!report.invalid_sources);
}

#[test]
fn source_parse_errors_and_missing_module_files_are_not_silent_successes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(root, "src/entry.rs", "mod bad; mod missing;");
    put(root, "src/bad.rs", "struct Broken {");
    let report = graph(root);
    assert!(report.invalid_sources);
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.reason.contains("neither name.rs"))
    );
}

#[test]
fn every_context_and_edge_span_uses_its_own_physical_utf8_source() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(
        root,
        "src/entry.rs",
        "//! café\r\nmod café { include!(\"fragment.inc\"); }",
    );
    put(root, "src/fragment.inc", "struct Café;");
    let report = graph(root);
    for site in report.edges.iter().map(|edge| &edge.site).chain(
        report
            .contexts
            .iter()
            .filter_map(|context| context.origin.as_ref()),
    ) {
        let source = fs::read(root.join(&site.file)).unwrap();
        assert_eq!(
            &source[site.span.start_byte..site.span.end_byte],
            site.span.source.as_bytes()
        );
    }
}

#[test]
fn roots_are_explicit_occurrences_and_root_relative_escapes_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    put(root, "src/entry.rs", "include!(\"../../outside.rs\");");
    let report = inventory_contexts(root, &["src/entry.rs".into(), "src/entry.rs".into()]).unwrap();
    assert_eq!(report.roots.len(), 2);
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.contexts[1].root, 1);
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.reason.contains("escapes"))
    );
    assert!(inventory_contexts(root, &["../outside.rs".into()]).is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_are_rejected_before_parent_path_reduction() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let other = tempfile::tempdir().unwrap();
    put(root, "src/entry.rs", "include!(\"link/../safe.inc\");");
    put(root, "src/safe.inc", "struct Safe;");
    symlink(other.path(), root.join("src/link")).unwrap();
    let report = graph(root);
    assert_eq!(report.files.len(), 1);
    assert!(
        report
            .review
            .iter()
            .any(|entry| entry.reason.contains("symlink"))
    );
}
