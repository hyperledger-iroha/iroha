//! Golden tests for pointer‑ABI type IDs and policy allow lists.

#[test]
fn pointer_type_ids_match_golden() {
    use ivm::PointerType as P;
    // Keep explicit numeric IDs to catch accidental renumbering.
    assert_eq!(P::AccountId as u16, 0x0001);
    assert_eq!(P::AssetDefinitionId as u16, 0x0002);
    assert_eq!(P::Name as u16, 0x0003);
    assert_eq!(P::Json as u16, 0x0004);
    assert_eq!(P::NftId as u16, 0x0005);
    assert_eq!(P::Blob as u16, 0x0006);
    assert_eq!(P::AssetId as u16, 0x0007);
    assert_eq!(P::DomainId as u16, 0x0008);
    assert_eq!(P::NoritoBytes as u16, 0x0009);
    assert_eq!(P::DataSpaceId as u16, 0x000A);
    assert_eq!(P::AxtDescriptor as u16, 0x000B);
    assert_eq!(P::AssetHandle as u16, 0x000C);
    assert_eq!(P::ProofBlob as u16, 0x000D);
    assert_eq!(P::SoracloudRequest as u16, 0x000E);
    assert_eq!(P::SoracloudResponse as u16, 0x000F);
}

#[test]
fn pointer_policy_allows_expected_types_for_v1() {
    use ivm::{PointerType as P, SyscallPolicy, is_type_allowed_for_policy};
    for ty in [
        P::AccountId,
        P::AssetDefinitionId,
        P::Name,
        P::Json,
        P::NftId,
        P::Blob,
        P::AssetId,
        P::DomainId,
        P::NoritoBytes,
        P::DataSpaceId,
        P::AxtDescriptor,
        P::AssetHandle,
        P::ProofBlob,
        P::SoracloudRequest,
        P::SoracloudResponse,
    ] {
        assert!(is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty));
    }
}
