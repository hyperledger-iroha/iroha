// Actual compiler observations for existing owners in sorafs_manifest::provider_admission.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::ProviderAdmissionProposalV1>(
        "sorafs_manifest::provider_admission::ProviderAdmissionProposalV1",
        "sorafs_manifest::provider_admission::ProviderAdmissionProposalV1",
        "22628e6300d0a1b7aef0ad8da2b7ae5c",
        "22628e6300d0a1b7aef0ad8da2b7ae5c",
    );
    crate::captured_owner_identity_support::check_both::<self::ProviderVrfPublicKeyV1>(
        "sorafs_manifest::provider_admission::ProviderVrfPublicKeyV1",
        "sorafs_manifest::provider_admission::ProviderVrfPublicKeyV1",
        "9158b513598fcd0ee6e518322f32580f",
        "9158b513598fcd0ee6e518322f32580f",
    );
    crate::captured_owner_identity_support::check_both::<self::EndpointAdmissionV1>(
        "sorafs_manifest::provider_admission::EndpointAdmissionV1",
        "sorafs_manifest::provider_admission::EndpointAdmissionV1",
        "08758b21b8ec66afbdcc251bb1126afa",
        "08758b21b8ec66afbdcc251bb1126afa",
    );
    crate::captured_owner_identity_support::check_both::<self::EndpointAttestationKind>(
        "sorafs_manifest::provider_admission::EndpointAttestationKind",
        "sorafs_manifest::provider_admission::EndpointAttestationKind",
        "e86608ce936b7cc5d15c606d61102243",
        "e86608ce936b7cc5d15c606d61102243",
    );
    crate::captured_owner_identity_support::check_both::<self::EndpointAttestationV1>(
        "sorafs_manifest::provider_admission::EndpointAttestationV1",
        "sorafs_manifest::provider_admission::EndpointAttestationV1",
        "50bc400eb3b6f76b734057bb8ec1d962",
        "50bc400eb3b6f76b734057bb8ec1d962",
    );
    crate::captured_owner_identity_support::check_both::<self::ProviderAdmissionEnvelopeV1>(
        "sorafs_manifest::provider_admission::ProviderAdmissionEnvelopeV1",
        "sorafs_manifest::provider_admission::ProviderAdmissionEnvelopeV1",
        "8b18cc3f9af99a00615dbdcb39e2e842",
        "8b18cc3f9af99a00615dbdcb39e2e842",
    );
    crate::captured_owner_identity_support::check_both::<self::ProviderAdmissionRenewalV1>(
        "sorafs_manifest::provider_admission::ProviderAdmissionRenewalV1",
        "sorafs_manifest::provider_admission::ProviderAdmissionRenewalV1",
        "e5ce0f9ac4011243124988841af7e0c2",
        "e5ce0f9ac4011243124988841af7e0c2",
    );
    crate::captured_owner_identity_support::check_both::<self::ProviderAdmissionRevocationV1>(
        "sorafs_manifest::provider_admission::ProviderAdmissionRevocationV1",
        "sorafs_manifest::provider_admission::ProviderAdmissionRevocationV1",
        "6c68cd68df57756883ee1d0884a8ec38",
        "6c68cd68df57756883ee1d0884a8ec38",
    );
}
