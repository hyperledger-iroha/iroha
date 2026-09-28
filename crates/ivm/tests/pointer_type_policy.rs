use ivm::{self, PointerType, SyscallPolicy};
#[test]
fn abi_v1_policy_allows_full_pointer_surface() {
    use PointerType::*;
    for ty in [
        AccountId,
        AssetDefinitionId,
        Name,
        Json,
        NftId,
        Blob,
        AssetId,
        DomainId,
        NoritoBytes,
        DataSpaceId,
        AxtDescriptor,
        ProofBlob,
        SoracloudRequest,
        SoracloudResponse,
        Int,
        Decimal,
        Quantity,
        AxtAnchoredSpendV1,
    ] {
        assert!(ivm::is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty))
    }
}
#[test]
fn abi_v1_uses_anchored_spends_and_rejects_retired_handle_pointers() {
    assert_eq!(PointerType::from_u16(0x0010), Some(PointerType::Quantity));
    assert_eq!(PointerType::from_u16(0x000C), None);
    assert_eq!(
        PointerType::from_u16(0x0013),
        Some(PointerType::AxtAnchoredSpendV1)
    );
    assert_eq!(PointerType::from_u16(0x0014), None);
    assert!(ivm::is_type_allowed_for_policy(
        SyscallPolicy::AbiV1,
        PointerType::Quantity
    ));
    assert!(
        !ivm::pointer_abi::policy_pointer_types(SyscallPolicy::AbiV1)
            .iter()
            .any(|pointer_type| *pointer_type as u16 == 0x000C)
    );
}
