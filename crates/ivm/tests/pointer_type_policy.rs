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
    ] {
        assert!(ivm::is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty))
    }
}
