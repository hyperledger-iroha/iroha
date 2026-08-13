//! Verify the canonical ABI hash table and runtime sample are up to date.
fn docs_source_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(|path| path.parent()) // workspace root
        .expect("workspace root")
        .join("specs")
}
#[test]
fn generated_abi_hashes_section_in_ivm_header_is_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED ABI HASHES -->";
    const END: &str = "<!-- END GENERATED ABI HASHES -->";
    let source_dir = docs_source_dir();
    let table = ivm::syscalls::render_abi_hashes_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");
    let path = source_dir.join("ivm_header.md");
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
#[test]
fn runtime_abi_hash_samples_match_the_descriptor() {
    let sample_dir = docs_source_dir().join("samples");
    let hash = hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1));
    let expected = format!("\"abi_hash_hex\": \"{hash}\"");
    let path = sample_dir.join("runtime_abi_hash.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_eq!(
        text.matches("\"abi_hash_hex\"").count(),
        1,
        "runtime ABI sample must contain exactly one hash field in {}",
        path.display()
    );
    assert!(
        text.contains(&expected),
        "runtime ABI hash out of date in {}",
        path.display()
    );
}
