//! ABI hash tests ensure the hash is stable for the same policy.
use ivm::syscalls::compute_abi_hash;
#[path = "../../iroha_data_model/tests/fixtures/abi_v1_hash.rs"]
mod abi_v1_fixture;
use abi_v1_fixture::ABI_V1_HASH_GOLDEN;
#[test]
fn abi_hash_is_stable() {
    let h1 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    let h2 = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(h1, h2, "ABI hash must be stable for the same policy");
}
#[test]
fn abi_hash_matches_v1_golden() {
    let hash = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(hex::encode(hash), ABI_V1_HASH_GOLDEN);
    assert_eq!(hash, abi_v1_fixture::abi_v1_hash());
}
#[test]
fn abi_hash_has_valid_iroha_hash_marker() {
    let hash = compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    assert_eq!(
        hash[hash.len() - 1] & 1,
        1,
        "ABI hash must not be an invalid-surface diagnostic sentinel"
    );
}
#[test]
fn abi_v1_execute_instruction_binds_typed_x509_queue_semantics() {
    // Both typed operations are hash-bound; neither tag exposes SCCP records or
    // an arbitrary instruction escape. X509 returns queue identity only.
    let table = ivm::syscalls::render_syscalls_markdown_table();
    let rows: Vec<&str> = table
        .lines()
        .filter(|line| line.starts_with("| 0xA0 |"))
        .collect();
    assert_eq!(rows.len(), 1, "exactly one 0xA0 row: {rows:?}");
    assert!(
        rows[0].contains("r11=operation_tag(1=SubmitBallot;2=SubmitPrivacyZkX509CertificateV1) |"),
        "{}",
        rows[0]
    );
    assert!(rows[0].contains("tag1:u64=0;tag2:r10=PendingAction(original signed action ordinal;queued only;no proof/effect authority)"));
    assert_eq!(
        ivm::syscalls::SMARTCONTRACT_INSTRUCTION_TAG_SUBMIT_BALLOT,
        1
    );
    assert_eq!(ivm::syscalls::SMARTCONTRACT_INSTRUCTION_TAG_PRESENT_X509, 2);
    assert!(!table.contains("RecordSccpMessage"));
}

#[test]
fn abi_v1_register_asset_requires_owning_domain() {
    // The execution host and compiler require the owning-domain pointer in r14.
    // Keep that argument in the hashed signature; a matching stale golden alone
    // cannot detect a missing runtime requirement.
    let table = ivm::syscalls::render_syscalls_markdown_table();
    let rows: Vec<&str> = table
        .lines()
        .filter(|line| line.starts_with("| 0x20 |"))
        .collect();
    assert_eq!(rows.len(), 1, "exactly one REGISTER_ASSET row: {rows:?}");
    let columns: Vec<&str> = rows[0].split('|').map(str::trim).collect();
    assert_eq!(
        columns.len(),
        7,
        "canonical syscall table shape: {}",
        rows[0]
    );
    assert_eq!(columns[2], "REGISTER_ASSET");
    assert_eq!(
        columns[3],
        "r10=&AssetDefinitionId, r11=&Blob(name), r12=numeric_spec:u64, r13=mintability:u64, r14=&DomainId(owning domain)"
    );
    assert_eq!(columns[4], "u64=0");
}
