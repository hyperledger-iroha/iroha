//! Verify the ABI hashes table in `docs/source/ivm_header.md` is generated and up to date.

#[test]
fn generated_abi_hashes_section_in_ivm_header_is_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED ABI HASHES -->";
    const END: &str = "<!-- END GENERATED ABI HASHES -->";
    // docs/source/ivm_header.md at workspace root
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("workspace root");
    let path = repo_root.join("docs/source/ivm_header.md");
    let text = std::fs::read_to_string(&path).expect("read ivm_header.md");
    let beg = text
        .find(BEGIN)
        .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
    let end = text
        .find(END)
        .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
    let section = &text[beg..end + END.len()];
    let table = ivm::syscalls::render_abi_hashes_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");
    assert_eq!(section, expected, "ABI hashes section out of date");
}
