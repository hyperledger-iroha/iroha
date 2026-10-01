// Bounded SDK transport over a genuine independent native decision; model tests own proof forgery.

#[test]
fn sns_lease_reader_uses_selected_cut_and_bounded_norito_without_trusting_response() {
    use iroha_data_model::{
        sns::lease::MAX_SNS_LEASE_PROOF_BYTES_V1, testing::native_finality::NativeFinalityFixture,
    };
    let mut native = NativeFinalityFixture::start("sdk-sns-lease");
    let block = native.block_with_submitted_work(native.next_header());
    let proof = native.certify(block);
    let verified = native.verifier().verify_retained_decision(&proof).unwrap();
    let mut client = client_with_base_url(base_url());
    client.network_id = native.network_id();
    let owner = client.account.clone();
    let (result, request) = capture_request(
        mk_response(StatusCode::OK, vec![1, 2, 3], Some(APPLICATION_NORITO)),
        |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_dataspace_lease(
                "acme",
                &owner,
                Hash::new(b"independently-qualified-native-schema"),
                &verified,
                20_000,
            )
        },
    );
    assert!(result.is_err());
    assert_eq!(request.url.path(), "/v1/sns/dataspaces/acme/lease/2");
    assert_eq!(request.max_response_bytes, MAX_SNS_LEASE_PROOF_BYTES_V1);
    assert_eq!(
        request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
            .map(|(_, value)| value.as_str()),
        Some(APPLICATION_NORITO)
    );
    for alias in ["Acme", "acme/else", "acme.other", "universal", " acme", ""] {
        assert!(
            client
                .get_dataspace_lease(alias, &owner, Hash::new(b"schema"), &verified, 20_000)
                .is_err()
        );
    }
    client.network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"foreign network"),
    ));
    assert!(
        client
            .get_dataspace_lease("acme", &owner, Hash::new(b"schema"), &verified, 20_000)
            .is_err()
    );
}
