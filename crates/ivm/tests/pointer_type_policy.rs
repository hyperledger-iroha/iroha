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
    ] {
        assert!(ivm::is_type_allowed_for_policy(SyscallPolicy::AbiV1, ty))
    }
}

#[test]
fn experimental_policy_rejects_pointer_types() {
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
    ] {
        assert!(
            !ivm::is_type_allowed_for_policy(SyscallPolicy::Experimental(1), ty),
            "experimental policy must reject {ty:?}"
        )
    }
}
