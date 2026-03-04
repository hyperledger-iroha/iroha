//! Doc-sync tests for runtime upgrade documentation.

#[test]
fn runtime_upgrade_types_section_matches_documentation() {
    const BEGIN: &str = "<!-- BEGIN RUNTIME UPGRADE TYPES -->";
    const END: &str = "<!-- END RUNTIME UPGRADE TYPES -->";

    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let path = repo_root.join("docs/source/runtime_upgrades.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

    let begin = text
        .find(BEGIN)
        .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
    let end = text
        .find(END)
        .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
    let section = &text[begin..end + END.len()];

    let expected = iroha_data_model::runtime::render_runtime_upgrade_types_markdown_section();
    assert_eq!(
        section, expected,
        "runtime upgrade types section out of date"
    );
}
