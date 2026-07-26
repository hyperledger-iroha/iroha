#[test]
fn abi_hash_markdown_matches_computed_values() {
    let table = ivm::syscalls::render_abi_hashes_markdown_table();
    // Expect header + single row (ABI v1) in the first release
    let mut rows = Vec::new();
    for line in table.lines() {
        if line.starts_with("|---") {
            continue;
        }
        if line.starts_with("| Policy ") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        rows.push(line.to_string());
    }
    assert_eq!(rows.len(), 1, "expected one policy row (ABI v1)");

    let mut parsed = Vec::new();
    for row in rows {
        let parts: Vec<_> = row.split('|').map(|s| s.trim()).collect();
        assert!(parts.len() >= 3);
        let name = parts[1];
        let hex = parts[2];
        assert_eq!(hex.len(), 64, "abi hash must be 32 bytes hex");
        parsed.push((name.to_string(), hex.to_string()));
    }
    // Check ABI v1 value matches compute_abi_hash(AbiV1)
    let v1 = parsed
        .iter()
        .find(|(n, _)| n == "ABI v1")
        .expect("ABI v1 row present");
    let expected = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(v1.1, hex::encode(expected), "V1 hash mismatch");
}
