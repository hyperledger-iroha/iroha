//! Canonical hash rows distinguish named host profiles within ABI V1.
#[test]
fn abi_hash_markdown_matches_computed_values() {
    let table = ivm::syscalls::render_abi_hashes_markdown_table();
    // Both immutable host profiles use ABI version 1 and have separate exact identities.
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
    assert_eq!(
        rows.len(),
        2,
        "expected baseline and named lifecycle profile rows"
    );
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
    let named = parsed
        .iter()
        .find(|(name, _)| name == ivm::syscalls::RUNTIME_LIFECYCLE_HOST_PROFILE)
        .expect("named runtime lifecycle profile row present");
    let named_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1RuntimeLifecycle);
    assert_eq!(named.1, hex::encode(named_hash));
    assert_ne!(named.1, v1.1);
}
