//! Verify that a qualified backend can link the enrollment kernel in a production library build.

use connect_norito_bridge::{
    AcceptedIssuerChallengeV1, FreshIssuerAdmissionV1, PendingIssuerEnrollmentV1,
    PreparedIssuerProofV1, SignedAppPreparationPinsV1, verify_signed_app_preparation_v1,
};

#[test]
fn qualified_backend_enrollment_api_is_not_test_only() {
    // Integration tests compile the library as a dependency without cfg(test). Imports and
    // method references therefore fail if the consuming kernel is accidentally gated again.
    let _begin = PendingIssuerEnrollmentV1::begin_selected;
    let _accept = PendingIssuerEnrollmentV1::accept_challenge;
    let _prepare = AcceptedIssuerChallengeV1::prepare_proof;
    let _complete = PreparedIssuerProofV1::complete;
    let _evidence = FreshIssuerAdmissionV1::evidence;
    let _verify_preparation = verify_signed_app_preparation_v1;
    assert!(std::mem::size_of::<SignedAppPreparationPinsV1<'static>>() > 0);
}
