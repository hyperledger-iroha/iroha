// Actual compiler observations for existing owners in sorafs_manifest::signer::custody.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::SignerCustodyBindingV1>(
        "sorafs_manifest::signer::custody::SignerCustodyBindingV1",
        "sorafs_manifest::signer::custody::SignerCustodyBindingV1",
        "ef1479a7d18032051f46f33d6eed1736",
        "ef1479a7d18032051f46f33d6eed1736",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerCustodyAuthorityV1>(
        "sorafs_manifest::signer::custody::SignerCustodyAuthorityV1",
        "sorafs_manifest::signer::custody::SignerCustodyAuthorityV1",
        "fc23f9f8da55aa221531e050ed51d0f5",
        "fc23f9f8da55aa221531e050ed51d0f5",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerCustodyAnchorV1>(
        "sorafs_manifest::signer::custody::SignerCustodyAnchorV1",
        "sorafs_manifest::signer::custody::SignerCustodyAnchorV1",
        "91780dc525bb32dcbc517b5dcc841f1a",
        "91780dc525bb32dcbc517b5dcc841f1a",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerCustodyStatementV1>(
        "sorafs_manifest::signer::custody::SignerCustodyStatementV1",
        "sorafs_manifest::signer::custody::SignerCustodyStatementV1",
        "ebb9ee3caab19063aec5677b06ccb7da",
        "ebb9ee3caab19063aec5677b06ccb7da",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerCustodyRecordV1>(
        "sorafs_manifest::signer::custody::SignerCustodyRecordV1",
        "sorafs_manifest::signer::custody::SignerCustodyRecordV1",
        "bd30ad634cf8d153a2c45372faa86889",
        "bd30ad634cf8d153a2c45372faa86889",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerCustodyActiveHeadV1>(
        "sorafs_manifest::signer::custody::SignerCustodyActiveHeadV1",
        "sorafs_manifest::signer::custody::SignerCustodyActiveHeadV1",
        "1f7647a6f16832f0e5daa926ad36b690",
        "1f7647a6f16832f0e5daa926ad36b690",
    );
}
