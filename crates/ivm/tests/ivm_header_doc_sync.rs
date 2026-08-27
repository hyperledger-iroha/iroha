//! Ensure the canonical generated IVM header policy matches the implementation.
const BEGIN: &str = "<!-- BEGIN GENERATED HEADER POLICY -->";
const END: &str = "<!-- END GENERATED HEADER POLICY -->";
fn expected_header_policy() -> String {
    let zk = ivm::ivm_mode::ZK;
    let vec = ivm::ivm_mode::VECTOR;
    let known_bits = zk | vec;
    let mut table = String::new();
    table.push_str("| Field | Policy |\n");
    table.push_str("|---|---|\n");
    table.push_str("| version_major | 1 |\n");
    table.push_str("| version_minor | 0 or 1 (deployable CNTR contracts require 1) |\n");
    table.push_str(&format!(
        "| mode (known bits) | 0x{known_bits:02x} (ZK=0x{zk:02x}, VECTOR=0x{vec:02x}) |\n"
    ));
    table.push_str("| abi_version | 1 |\n");
    table.push_str(
        "| vector_length | 0 or 1..=64 (0 selects runtime default; independent of VECTOR bit) |\n",
    );
    format!("{BEGIN}\n{table}{END}")
}
fn assert_generated_header_policy(path: &std::path::Path, expected: &str) {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let beg = text
        .find(BEGIN)
        .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
    let end = text
        .find(END)
        .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
    let section = &text[beg..end + END.len()];
    assert_eq!(
        section,
        expected,
        "{} out of date; run: cargo run -p ivm --features dev-tools --bin gen_header_doc -- --write",
        path.display()
    );
}
#[test]
fn generated_header_policy_sections_are_up_to_date() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .join("specs");
    let expected = expected_header_policy();
    assert_generated_header_policy(&source_dir.join("ivm_header.md"), &expected);
}
