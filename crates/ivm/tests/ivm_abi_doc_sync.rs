//! Verify every localized IVM-header ABI hash table is generated and up to date.

#[test]
fn generated_abi_hashes_sections_in_all_ivm_headers_are_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED ABI HASHES -->";
    const END: &str = "<!-- END GENERATED ABI HASHES -->";
    // docs/source/ivm_header.md at workspace root
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("workspace root");
    let source_dir = repo_root.join("docs/source");
    let mut paths = std::fs::read_dir(&source_dir)
        .expect("read docs/source")
        .map(|entry| entry.expect("read docs/source entry").path())
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            name == "ivm_header.md" || (name.starts_with("ivm_header.") && name.ends_with(".md"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty(), "no localized IVM header documents");
    let table = ivm::syscalls::render_abi_hashes_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let beg = text
            .find(BEGIN)
            .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
        let end = text
            .find(END)
            .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
        assert_eq!(
            &text[beg..end + END.len()],
            expected,
            "ABI hashes section out of date in {}",
            path.display()
        );
    }
}
