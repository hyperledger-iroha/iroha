//! Native application proof checkpoints must be pinned before any HTTP request.
use super::{
    evidence_http_tests::{base_url, capture_requests, client_with_base_url, empty_response},
    *,
};
use iroha_data_model::{
    governance::types::BallotAttemptId, testing::native_finality::NativeFinalityFixture,
};

#[test]
fn proof_page_and_catch_up_reject_foreign_native_checkpoint_before_http() {
    let fixture = NativeFinalityFixture::start("sdk-independent-checkpoint");
    let checkpoint = fixture.checkpoint();
    let client = client_with_base_url(base_url());
    assert_ne!(client.network_id, checkpoint.network_id());
    let ballot = BallotAttemptId::new([0x42; 32]);
    let (errors, requests) = capture_requests(empty_response(StatusCode::OK), |transport| {
        let client = client.with_test_http_transport(transport);
        [
            client
                .get_validation_fee_current_policy_proof_page(&checkpoint)
                .unwrap_err(),
            client
                .catch_up_validation_fee_current_policy_proof(&checkpoint)
                .unwrap_err(),
            client
                .get_parliament_timed_ovn_casting_proof_page(ballot, &checkpoint)
                .unwrap_err(),
            client
                .catch_up_parliament_timed_ovn_casting_proof(ballot, &checkpoint)
                .unwrap_err(),
        ]
    });
    for error in errors {
        assert!(error.to_string().contains("different network"), "{error:#}");
    }
    assert!(
        requests.is_empty(),
        "a foreign checkpoint cannot trigger HTTP"
    );
}

#[test]
fn casting_page_rejects_zero_ballot_with_current_native_checkpoint_before_http() {
    let fixture = NativeFinalityFixture::start("sdk-independent-checkpoint");
    let checkpoint = fixture.checkpoint();
    let mut client = client_with_base_url(base_url());
    client.network_id = checkpoint.network_id();
    let (error, requests) = capture_requests(empty_response(StatusCode::OK), |transport| {
        client
            .with_test_http_transport(transport)
            .get_parliament_timed_ovn_casting_proof_page(BallotAttemptId::new([0; 32]), &checkpoint)
            .unwrap_err()
    });
    assert!(
        error
            .to_string()
            .contains("ballot attempt id must be non-zero")
    );
    assert!(requests.is_empty(), "an invalid ballot cannot trigger HTTP");
}
