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
fn abi_v1_assigns_quantity_and_rejects_the_unassigned_numeric_pointer_id() {
    assert_eq!(PointerType::from_u16(0x0010), Some(PointerType::Quantity));
    assert_eq!(PointerType::from_u16(0x0013), None);
    assert!(ivm::is_type_allowed_for_policy(
        SyscallPolicy::AbiV1,
        PointerType::Quantity
    ));
    assert!(
        !ivm::pointer_abi::policy_pointer_types(SyscallPolicy::AbiV1)
            .iter()
            .any(|pointer_type| *pointer_type as u16 == 0x0013)
    );
}
