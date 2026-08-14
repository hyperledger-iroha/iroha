//! Verify the root `ivm.md` pointer type table is up to date.
const BEGIN: &str = "<!-- BEGIN GENERATED POINTER TYPES -->";
const END: &str = "<!-- END GENERATED POINTER TYPES -->";
fn repo_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("workspace root")
}
fn assert_generated_pointer_types(path: &std::path::Path, expected: &str) {
    let text = std::fs::read_to_string(path).expect("read ivm.md");
    let beg = text
        .find(BEGIN)
        .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
    let end = text
        .find(END)
        .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
    let section = &text[beg..end + END.len()];
    assert_eq!(section, expected, "ivm.md pointer type list out of date");
}
#[test]
fn generated_pointer_types_section_in_ivm_md_is_up_to_date() {
    let table = ivm::render_pointer_types_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");
    assert_generated_pointer_types(&repo_root().join("ivm.md"), &expected);
}
