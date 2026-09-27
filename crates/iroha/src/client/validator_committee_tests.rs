// Request binding and bounded canonical transport for committee preparation reads.

#[test]
fn validator_committee_status_client_binds_route_target_network_and_deadline() {
    use iroha_data_model::nexus::ValidatorCommitteeStatusV1;
    let (first, _, _) = bridge_finality_chain_fixture();
    let status = ValidatorCommitteeStatusV1 {
        network_id: first.finality_artifact.height_context.network_id,
        target_epoch: 7,
        latest_finality: first.finality_artifact,
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
    for mutation in 0..4 {
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
            _ => empty_response(StatusCode::NOT_FOUND),
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
    next.target_epoch = next.latest_finality.height_context.epoch + 1;
    let (result, request) = capture_request(norito_response(StatusCode::OK, &next), |transport| {
        client
            .with_test_http_transport(transport)
            .get_validator_committee_status(None)
    });
    assert_eq!(result.unwrap(), next);
    assert_eq!(request.url.query(), None);
}
