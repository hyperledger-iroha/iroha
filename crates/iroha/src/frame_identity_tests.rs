//! Typed frame identities captured before the canonical codec cutover.

#[path = "../../../fixtures/sdk/frame_identity_assertions.rs"]
mod captured;
pub(crate) use captured::assert_bidirectional;

#[test]
fn original_package_observations_are_complete() {
    captured::assert_package_complete("iroha", 28, 56);
}

#[test]
fn captured_public_frame_identities() {
    assert_bidirectional::<crate::client::AccountOnboardingPlanRequestV1>(
        "iroha::client::AccountOnboardingPlanRequestV1",
    );
    assert_bidirectional::<crate::client::AccountOnboardingPlanBodyV1>(
        "iroha::client::AccountOnboardingPlanBodyV1",
    );
    assert_bidirectional::<crate::client::AccountOnboardingPlanReceiptV1>(
        "iroha::client::AccountOnboardingPlanReceiptV1",
    );
    assert_bidirectional::<crate::client::PreparedOperationBindingV1>(
        "iroha::client::PreparedOperationBindingV1",
    );
    assert_bidirectional::<crate::client::AccountOnboardingPrepareRequestV1>(
        "iroha::client::AccountOnboardingPrepareRequestV1",
    );
    assert_bidirectional::<crate::client::AccountOnboardingPreparedTransactionV1>(
        "iroha::client::AccountOnboardingPreparedTransactionV1",
    );
    assert_bidirectional::<crate::client::AccountOnboardingProofRequiredPrepareResponseV1>(
        "iroha::client::AccountOnboardingProofRequiredPrepareResponseV1",
    );
    assert_bidirectional::<crate::client::AccountFaucetClaimV1>(
        "iroha::client::AccountFaucetClaimV1",
    );
    assert_bidirectional::<crate::client::AccountFaucetPrepareRequestV1>(
        "iroha::client::AccountFaucetPrepareRequestV1",
    );
    assert_bidirectional::<crate::client::AccountFaucetPreparedTransactionV1>(
        "iroha::client::AccountFaucetPreparedTransactionV1",
    );
    assert_bidirectional::<crate::client::MultisigSpecRequest>(
        "iroha::client::MultisigSpecRequest",
    );
    assert_bidirectional::<crate::client::MultisigSpecResponse>(
        "iroha::client::MultisigSpecResponse",
    );
    assert_bidirectional::<crate::client::SccpBridgeSubmitResponse>(
        "iroha::client::SccpBridgeSubmitResponse",
    );
    assert_bidirectional::<crate::client::MultisigProposalsQueryRequest>(
        "iroha::client::MultisigProposalsQueryRequest",
    );
    assert_bidirectional::<crate::client::MultisigProposalEntry>(
        "iroha::client::MultisigProposalEntry",
    );
    assert_bidirectional::<crate::client::MultisigProposalsQueryResponse>(
        "iroha::client::MultisigProposalsQueryResponse",
    );
    assert_bidirectional::<crate::client::MultisigProposalsResolveRequest>(
        "iroha::client::MultisigProposalsResolveRequest",
    );
    assert_bidirectional::<crate::client::MultisigProposalResolveResponse>(
        "iroha::client::MultisigProposalResolveResponse",
    );
    assert_bidirectional::<crate::client::MultisigProposeRequest>(
        "iroha::client::MultisigProposeRequest",
    );
    assert_bidirectional::<crate::client::MultisigResponse>("iroha::client::MultisigResponse");
}

// SCCP capability and discovery records have one shared owner. The former SDK
// declarations remain only in the immutable historical observation fixture;
// their separate frame names are not accepted wire variants.
#[test]
fn shared_sccp_frames_have_one_canonical_producer_identity() {
    fn assert_frame<T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>>(
        expected: &str,
    ) {
        assert_eq!(<T as norito::NoritoSchema>::nominal_name(), expected);
        assert_eq!(<T as norito::NoritoSchema>::frame_name(), expected);
        assert_eq!(
            norito::schema::identity::frame_hash::<T>(),
            norito::core::schema_hash_for_name(expected),
        );
    }
    assert_frame::<iroha_sccp::api::SccpRegistryLimits>(
        "iroha_torii::routing::SccpRegistryLimitsDto",
    );
    assert_frame::<iroha_sccp::api::SccpResourceLimits>(
        "iroha_torii::routing::SccpResourceLimitsDto",
    );
    assert_frame::<iroha_sccp::api::SccpCapabilities>("iroha_torii::routing::SccpCapabilitiesDto");
    assert_frame::<iroha_sccp::api::SccpRecentMessageLinks>(
        "iroha_torii::routing::SccpRecentMessageLinksDto",
    );
    assert_frame::<iroha_sccp::api::SccpRecentMessage>(
        "iroha_torii::routing::SccpRecentMessageDto",
    );
    assert_frame::<iroha_sccp::api::SccpRecentCursor>("iroha_torii::routing::SccpRecentCursorDto");
    assert_frame::<iroha_sccp::api::SccpRecentMessages>(
        "iroha_torii::routing::SccpRecentMessagesDto",
    );
}
