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
        AssetHandle,
        ProofBlob,
        SoracloudRequest,
        SoracloudResponse,
        Int,
        Decimal,
        Quantity,
    ] {
        assert!(ivm::is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty))
    }
}

#[test]
fn abi_v1_rejects_the_retired_amount_pointer_id() {
    assert_eq!(
        PointerType::from_u16(0x0010),
        Some(PointerType::RetiredAmount)
    );
    assert_eq!(PointerType::from_u16(0x0013), Some(PointerType::Quantity));
    assert!(
        ivm::pointer_abi::policy_pointer_types(SyscallPolicy::AbiV1)
            .iter()
            .all(|pointer_type| *pointer_type != PointerType::RetiredAmount)
    );
    assert!(!ivm::is_type_allowed_for_policy(
        SyscallPolicy::AbiV1,
        PointerType::RetiredAmount
    ));
}
