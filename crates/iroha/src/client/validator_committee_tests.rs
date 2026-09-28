// Request binding and bounded canonical transport for committee preparation reads.

#[test]
fn validator_committee_status_client_binds_route_target_network_and_deadline() {
    use crate::crypto::{Algorithm, KeyPair};
    use iroha_data_model::nexus::ValidatorCommitteeStatusV1;
    use iroha_data_model::{
        block::builder::BlockBuilder,
        sumeragi::finality::{NativeFinalityArtifact, NativeFinalityLimits},
    };
    // This is an HTTP carrier fixture, not a finality fixture: no QC is fabricated.
    // Authorization tests use the complete native journal consumer in Core.
    let key = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
    let transaction = TransactionBuilder::new(
        test_network_id(),
        AccountId::new(key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .try_sign(key.private_key())
    .unwrap();
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(HashOf::from_untyped_unchecked(Hash::new(
            b"transport predecessor",
        ))),
        None,
        10,
        0,
    ));
    builder.push_transaction(transaction);
    let block = builder
        .try_build_with_signature(0, key.private_key())
        .unwrap();
    let artifact = NativeFinalityArtifact::from_block(
        &block,
        NativeFinalityLimits {
            block_bytes: 16 * 1024 * 1024,
            journal_bytes: 16 * 1024 * 1024,
            block_count: 256,
            allocated_bytes: 64 * 1024 * 1024,
        },
    )
    .unwrap();
    let status = ValidatorCommitteeStatusV1 {
        network_id: test_network_id(),
        target_epoch: 7,
        latest_finality: artifact,
        selected: None,
        candidate_keys: vec![],
        pending_beacon_session: None,
    };
    let mut client = client_with_base_url(base_url());
    client.network_id = status.network_id;
    let response = norito_response(StatusCode::OK, &status);
    let (result, request) = capture_request(response, |transport| {
        client
            .clone()
            .with_test_http_transport(transport)
            .get_validator_committee_status(Some(7))
    });
    assert_eq!(result.unwrap(), status);
    assert_eq!(request.method, HttpMethod::GET);
    assert_eq!(request.url.path(), "/v1/nexus/validator-committee");
    assert_eq!(request.url.query(), Some("target_epoch=7"));
    assert_eq!(request.max_response_bytes, 16 * 1024 * 1024);
    assert_single_accept_header(&request, APPLICATION_NORITO);
    assert_eq!(request.timeout, Some(client.torii_request_timeout));
    for mutation in 0..5 {
        let mut changed = status.clone();
        let response = match mutation {
            0 => {
                changed.target_epoch = 8;
                norito_response(StatusCode::OK, &changed)
            }
            1 => {
                changed.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"foreign committee network"),
                ));
                norito_response(StatusCode::OK, &changed)
            }
            2 => json_response(StatusCode::OK, &norito::json::to_json(&changed).unwrap()),
            3 => empty_response(StatusCode::NOT_FOUND),
            _ => {
                changed.latest_finality.block_wire.push(0);
                norito_response(StatusCode::OK, &changed)
            }
        };
        let (result, _) = capture_request(response, |transport| {
            client
                .clone()
                .with_test_http_transport(transport)
                .get_validator_committee_status(Some(7))
        });
        assert!(result.is_err(), "unbound committee response {mutation}");
    }
    let mut next = status;
    next.target_epoch = 1; // server-selected observation, no authorization claim
    let (result, request) = capture_request(norito_response(StatusCode::OK, &next), |transport| {
        client
            .with_test_http_transport(transport)
            .get_validator_committee_status(None)
    });
    assert_eq!(result.unwrap(), next);
    assert_eq!(request.url.query(), None);
}
