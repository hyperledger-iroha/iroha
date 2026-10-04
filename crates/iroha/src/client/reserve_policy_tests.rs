// Reserve proof transport binds the signed manager request to independently selected finality.

fn reserve_proof_policy() -> iroha_data_model::sorafs::reserve::ReserveAuthorityPolicyV1 {
    use iroha_data_model::sorafs::reserve::{
        RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReservePolicyV1,
    };
    let account = |seed| {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    };
    ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
        custody_account: account(41),
        treasury_account: account(42),
        operations_authority: account(43),
        decision_authority: account(44),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: sorafs_manifest::deal::XorQuantity::try_from_micro(1_000_000_000)
            .unwrap(),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    }
}

#[test]
fn independent_reserve_policy_proof_uses_signed_bounded_route_and_http_absence_is_not_state() {
    use iroha_data_model::sorafs::reserve::proof::MAX_RESERVE_POLICY_PROOF_BYTES_V1;
    let (client, _, _, block) = stream_token_custody_control_fixture();
    let policy = reserve_proof_policy();
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
            vec![0; MAX_RESERVE_POLICY_PROOF_BYTES_V1 + 1],
            Some(APPLICATION_NORITO),
        ),
    ] {
        let (result, request) = capture_request(response, |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_reserve_policy_state(
                &client.account,
                &policy,
                Hash::new(b"qualified reserve schema"),
                &block,
            )
        });
        assert!(result.is_err());
        assert_eq!(request.url.path(), "/v1/sorafs/reserve/policy/2");
        assert_eq!(
            request.max_response_bytes,
            MAX_RESERVE_POLICY_PROOF_BYTES_V1
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
fn independent_reserve_policy_proof_verifies_selected_global_absence() {
    // Synthetic World value hashes with genuine finality signatures exercise SDK transport
    // and key membership; this fixture does not qualify native execution or publication.
    use iroha_data_model::{
        sorafs::reserve::{history::reserve_policy_permission, proof::ReservePolicyProofV1},
        sumeragi_finality::{
            WorldStateElementKindV1, WorldStateSnapshotEntryV1, WorldStateSnapshotV1,
            world_state_value_hash_v1,
        },
        testing::native_finality::NativeFinalityFixture,
    };
    let (mut client, _, _, _) = stream_token_custody_control_fixture();
    let mut native = NativeFinalityFixture::start("sdk-reserve-policy-absence");
    client.chain = native.chain_id().parse().unwrap();
    client.network_id = native.network_id();
    let policy = reserve_proof_policy();
    let permissions = [reserve_policy_permission()].into_iter().collect();
    let schema = Hash::new(b"independently qualified reserve schema");
    let mut entries = Vec::new();
    for account in [
        &client.account,
        &policy.custody_account,
        &policy.treasury_account,
        &policy.operations_authority,
        &policy.decision_authority,
    ] {
        entries.push(WorldStateSnapshotEntryV1 {
            field_id: "world.accounts".into(),
            kind: WorldStateElementKindV1::Table,
            key_hash: Some(world_state_value_hash_v1(account).unwrap()),
            value_hash: Hash::new(b"independently certified account value"),
        });
    }
    entries.push(WorldStateSnapshotEntryV1 {
        field_id: "world.asset_definitions".into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(world_state_value_hash_v1(&policy.asset_definition).unwrap()),
        value_hash: Hash::new(b"independently certified asset definition value"),
    });
    entries.push(WorldStateSnapshotEntryV1 {
        field_id: "world.account_permissions".into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(world_state_value_hash_v1(&client.account).unwrap()),
        value_hash: world_state_value_hash_v1(&permissions).unwrap(),
    });
    entries
        .sort_by(|a, b| (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash)));
    let proof = ReservePolicyProofV1 {
        world: WorldStateSnapshotV1 {
            schema_hash: schema,
            entries,
        },
        manager_permissions: permissions,
        current: None,
    };
    let block = native.block_with_submitted_work(native.next_header());
    let certificate = native.certify_with_world_root(block, proof.world.root().unwrap());
    let verified = native
        .verifier()
        .verify_retained_decision(&certificate)
        .unwrap();
    let (result, _) = capture_request(
        mk_response(
            StatusCode::OK,
            norito::encode_canonical(&proof).unwrap(),
            Some(APPLICATION_NORITO),
        ),
        |transport| {
            let client = client.with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_reserve_policy_state(&client.account, &policy, schema, &verified)
        },
    );
    assert!(result.unwrap().current().is_none());
}

#[test]
fn independent_reserve_policy_proof_refuses_wrong_manager_scope_and_deadline_before_dispatch() {
    use iroha_data_model::{
        block::consensus::SumeragiRootScope, testing::native_finality::NativeFinalityFixture,
    };
    let (client, _, _, block) = stream_token_custody_control_fixture();
    let policy = reserve_proof_policy();
    let mut private = NativeFinalityFixture::start_with_scope(
        "private-reserve-policy",
        SumeragiRootScope::Dataspace {
            parent_network_id: client.network_id,
            dataspace_id: iroha_model_base::topology::DataSpaceId::new(9),
        },
    );
    let private_block = private.block_with_submitted_work(private.next_header());
    let certificate = private.certify(private_block);
    let private_block = private
        .verifier()
        .verify_retained_decision(&certificate)
        .unwrap();
    with_mock_http(
        |_| panic!("invalid selection must not dispatch"),
        |transport| {
            let client = client.with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            let schema = Hash::new(b"qualified schema");
            assert!(
                client
                    .get_reserve_policy_state(&policy.custody_account, &policy, schema, &block)
                    .is_err()
            );
            let mut foreign = client.clone();
            foreign.chain = "other-reserve-chain".parse().unwrap();
            assert!(
                foreign
                    .get_reserve_policy_state(&foreign.account, &policy, schema, &block)
                    .is_err()
            );
            let mut private_client = client.clone();
            private_client.chain = private.chain_id().parse().unwrap();
            private_client.network_id = private.network_id();
            assert!(
                private_client
                    .get_reserve_policy_state(
                        &private_client.account,
                        &policy,
                        schema,
                        &private_block
                    )
                    .is_err()
            );
            let expired = client.with_request_deadline(std::time::Instant::now());
            assert!(
                expired
                    .get_reserve_policy_state(&expired.account, &policy, schema, &block)
                    .is_err()
            );
        },
    );
}
