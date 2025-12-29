//! Verify the pointer type table in `ivm.md` is generated and up to date.

#[test]
fn generated_pointer_types_section_in_ivm_md_is_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED POINTER TYPES -->";
    const END: &str = "<!-- END GENERATED POINTER TYPES -->";
    // Root ivm.md (not the crate docs)
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("workspace root");
    let path = repo_root.join("ivm.md");
    let text = std::fs::read_to_string(&path).expect("read ivm.md");
    let beg = text
        .find(BEGIN)
        .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
    let end = text
        .find(END)
        .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
    let section = &text[beg..end + END.len()];
    let table = ivm::render_pointer_types_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");
    assert_eq!(section, expected, "ivm.md pointer type list out of date");
}
