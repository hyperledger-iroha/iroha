//! Ensure `docs/source/ivm_header.md` generated header policy section matches the implementation.

#[test]
fn generated_header_policy_section_is_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED HEADER POLICY -->";
    const END: &str = "<!-- END GENERATED HEADER POLICY -->";
    // Resolve workspace root from this crate
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
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

    // Render expected table using the generator's logic inline
    let zk = ivm::ivm_mode::ZK;
    let vec = ivm::ivm_mode::VECTOR;
    let htm = ivm::ivm_mode::HTM;
    let known_bits = zk | vec | htm;
    let table = {
        let mut md = String::new();
        md.push_str("| Field | Policy |\n");
        md.push_str("|---|---|\n");
        md.push_str(&format!("| version_major | {} |\n", 1));
        md.push_str("| version_minor | 0 |\n");
        md.push_str(&format!(
            "| mode (known bits) | 0x{known_bits:02x} (ZK=0x{zk:02x}, VECTOR=0x{vec:02x}, HTM=0x{htm:02x}) |\n"
        ));
        md.push_str("| abi_version | 1 |\n");
        md.push_str("| vector_length | 0 or 1..=64 (advisory; independent of VECTOR bit) |\n");
        md
    };
    let expected = format!("{BEGIN}\n{table}{END}");

    assert_eq!(
        section, expected,
        "docs/source/ivm_header.md out of date; run: cargo run -p ivm --bin gen_header_doc -- --write"
    );
}
