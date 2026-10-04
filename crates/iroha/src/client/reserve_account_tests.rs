// Negative transport and independent request-admission tests; no successful native state is fabricated.

#[test]
fn independent_reserve_account_uses_signed_bounded_route_and_http_errors_are_not_absence() {
    use iroha_data_model::sorafs::{
        capacity::ProviderId, reserve::account_proof::MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1,
    };
    let (client, _, _, block) = stream_token_custody_control_fixture();
    let mut policy = reserve_proof_policy();
    policy.operations_authority = client.account.clone();
    let owner = policy.decision_authority.clone();
    let provider = ProviderId::new([0xab; 32]);
    for response in [
        mk_response(StatusCode::OK, vec![1, 2, 3], Some(APPLICATION_NORITO)),
        mk_response(StatusCode::NOT_FOUND, Vec::new(), Some(APPLICATION_NORITO)),
        mk_response(
            StatusCode::SERVICE_UNAVAILABLE,
            Vec::new(),
            Some(APPLICATION_NORITO),
        ),
        mk_response(StatusCode::OK, b"{}".to_vec(), Some(APPLICATION_JSON)),
        mk_response(
            StatusCode::OK,
            vec![0; MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1 + 1],
            Some(APPLICATION_NORITO),
        ),
    ] {
        let (result, request) = capture_request(response, |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_reserve_account_state(
                &client.account,
                provider,
                &owner,
                &policy,
                Hash::new(b"qualified reserve schema"),
                &block,
            )
        });
        assert!(result.is_err());
        assert_eq!(
            request.url.path(),
            format!("/v1/sorafs/reserve/providers/{}/proof/2", "ab".repeat(32))
        );
        assert_eq!(
            request.max_response_bytes,
            MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1
        );
        for name in [
            "x-iroha-account",
            "x-iroha-signature",
            "x-iroha-timestamp-ms",
            "x-iroha-nonce",
        ] {
            assert!(
                request
                    .headers
                    .iter()
                    .any(|(header, value)| header.eq_ignore_ascii_case(name) && !value.is_empty()),
                "missing {name}"
            );
        }
    }
}

#[test]
fn independent_reserve_account_refuses_mismatched_selections_and_expiry_before_dispatch() {
    use iroha_data_model::{
        block::consensus::SumeragiRootScope, sorafs::capacity::ProviderId,
        testing::native_finality::NativeFinalityFixture,
    };
    let (client, _, _, block) = stream_token_custody_control_fixture();
    let mut policy = reserve_proof_policy();
    policy.operations_authority = client.account.clone();
    let owner = policy.decision_authority.clone();
    let provider = ProviderId::new([0xab; 32]);
    let schema = Hash::new(b"qualified reserve schema");
    // This fixture establishes only a genuine signed private consensus scope for a
    // before-HTTP refusal. It supplies no successful reserve state or execution outcome.
    let mut private = NativeFinalityFixture::start_with_scope(
        "private-reserve-account",
        SumeragiRootScope::Dataspace {
            parent_network_id: client.network_id,
            dataspace_id: iroha_model_base::topology::DataSpaceId::new(9),
        },
    );
    let body = private.block_with_submitted_work(private.next_header());
    let certificate = private.certify(body);
    let private_block = private
        .verifier()
        .verify_retained_decision(&certificate)
        .unwrap();
    with_mock_http(
        |_| panic!("invalid selection must not dispatch"),
        |transport| {
            let client = client.with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            assert!(
                client
                    .get_reserve_account_state(&owner, provider, &owner, &policy, schema, &block)
                    .is_err()
            );
            assert!(
                client
                    .get_reserve_account_state(
                        &client.account,
                        ProviderId::new([0; 32]),
                        &owner,
                        &policy,
                        schema,
                        &block
                    )
                    .is_err()
            );
            let mut wrong_policy = policy.clone();
            wrong_policy.operations_authority = owner.clone();
            assert!(
                client
                    .get_reserve_account_state(
                        &client.account,
                        provider,
                        &owner,
                        &wrong_policy,
                        schema,
                        &block
                    )
                    .is_err()
            );
            let mut wrong_chain = client.clone();
            wrong_chain.chain = "another-chain".parse().unwrap();
            assert!(
                wrong_chain
                    .get_reserve_account_state(
                        &wrong_chain.account,
                        provider,
                        &owner,
                        &policy,
                        schema,
                        &block
                    )
                    .is_err()
            );
            let mut private_client = client.clone();
            private_client.chain = private.chain_id().parse().unwrap();
            private_client.network_id = private.network_id();
            assert!(
                private_client
                    .get_reserve_account_state(
                        &private_client.account,
                        provider,
                        &owner,
                        &policy,
                        schema,
                        &private_block
                    )
                    .is_err()
            );
            let expired = client.with_request_deadline(std::time::Instant::now());
            assert!(
                expired
                    .get_reserve_account_state(
                        &expired.account,
                        provider,
                        &owner,
                        &policy,
                        schema,
                        &block
                    )
                    .is_err()
            );
        },
    );
}
