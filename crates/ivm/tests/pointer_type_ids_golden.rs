//! Golden tests for pointer‑ABI type IDs and policy allow lists.
#[test]
fn pointer_type_ids_match_golden() {
    use ivm::PointerType as P;
    // BEGIN GENERATED ABI V1 POINTER TYPE IDS
    let expected: &[(P, u16)] = &[
        (P::AccountId, 0x0001),
        (P::AssetDefinitionId, 0x0002),
        (P::Name, 0x0003),
        (P::Json, 0x0004),
        (P::NftId, 0x0005),
        (P::Blob, 0x0006),
        (P::AssetId, 0x0007),
        (P::DomainId, 0x0008),
        (P::NoritoBytes, 0x0009),
        (P::DataSpaceId, 0x000A),
        (P::AxtDescriptor, 0x000B),
        (P::AssetHandle, 0x000C),
        (P::ProofBlob, 0x000D),
        (P::SoracloudRequest, 0x000E),
        (P::SoracloudResponse, 0x000F),
        (P::Quantity, 0x0010),
        (P::Int, 0x0011),
        (P::Decimal, 0x0012),
    ];
    // END GENERATED ABI V1 POINTER TYPE IDS
    let expected_types = expected
        .iter()
        .map(|(pointer_type, _)| *pointer_type)
        .collect::<Vec<_>>();
    assert_eq!(
        P::all(),
        expected_types.as_slice(),
        "the ABI-v1 pointer type surface changed; update numeric ID and policy goldens together"
    );
    for &(pointer_type, id) in expected {
        assert_eq!(
            pointer_type as u16, id,
            "the generated ABI-v1 pointer type ID changed for {pointer_type:?}"
        );
        assert_eq!(
            P::from_u16(id),
            Some(pointer_type),
            "the generated ABI-v1 pointer type decoder changed for 0x{id:04X}"
        );
    }
    assert_eq!(P::from_u16(0x0013), None);
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
        P::Int,
        P::Decimal,
        P::Quantity,
    ] {
        assert!(is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty));
    }
}
#[test]
fn unassigned_numeric_pointer_id_is_unknown_and_never_allowed() {
    use ivm::{PointerType as P, SyscallPolicy, is_type_allowed_for_policy};
    assert_eq!(P::from_u16(0x0013), None);
    assert!(
        !ivm::pointer_abi::policy_pointer_types(SyscallPolicy::AbiV1)
            .iter()
            .any(|ty| *ty as u16 == 0x0013)
    );
    for ty in P::all() {
        assert!(
            is_type_allowed_for_policy(SyscallPolicy::AbiV1, *ty),
            "all known first-release pointer types are in the sole ABI-v1 policy: {ty:?}"
        );
    }
}
