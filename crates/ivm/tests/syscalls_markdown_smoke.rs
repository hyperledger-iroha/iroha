//! Smoke tests for syscall docs generators (table and minimal list).

#[test]
fn render_syscalls_markdown_table_includes_headers_and_known_rows() {
    let md = ivm::syscalls::render_syscalls_markdown_table();
    // Headers present
    assert!(md.contains("| Number | Name | Args | Return | Gas |"));
    // A couple of representative rows present
    assert!(md.contains("| 0x64 | ZK_ROOTS_GET"));
    assert!(md.contains("| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY"));
    assert!(md.contains("| 0xF0 | ALLOC"));
}

#[test]
fn render_syscalls_min_list_contains_expected_items() {
    let md = ivm::syscalls::render_syscalls_min_list();
    // Ensure formatting and a few entries are present
    assert!(md.contains("- 0x60 ZK_VERIFY_TRANSFER"));
    assert!(md.contains("- 0x01 EXIT"));
    assert!(md.contains("- 0xFF GET_REGISTER_MERKLE_COMPACT"));
}
