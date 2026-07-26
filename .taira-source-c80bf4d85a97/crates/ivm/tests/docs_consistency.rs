#[test]
fn syscalls_doc_has_abi_policy_section() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");
    let text = std::fs::read_to_string(path).expect("read syscalls.md");
    assert!(
        text.contains("ABI policy"),
        "expected 'ABI policy' section in syscalls.md"
    );
    assert!(
        text.contains("is_syscall_allowed"),
        "expected mention of is_syscall_allowed in syscalls.md"
    );
}
