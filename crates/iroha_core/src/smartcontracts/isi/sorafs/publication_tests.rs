// Native assertions bind paid registration, exact assignment, completion, publisher and floor.

#[test]
fn publication_assertion_requires_exact_live_assignment_and_all_completions() {
    use iroha_data_model::isi::sorafs::AssertSorafsPublicationV1;
    let state = make_state_with_completion_anchor();
    let mut block = state.block(block_header());
    let mut stx = block.transaction_for_fastpq_testing(Hash::prehashed([0x51; Hash::LENGTH]));
    seed_automatic_replication_capacity(&mut stx, default_policy().min_replicas);
    RegisterPinManifest {
        manifest_payload: default_manifest_payload(),
        alias: None,
        successor_of: None,
    }
    .execute(&alice(), &mut stx)
    .unwrap();
    let order_id = derive_sorafs_auto_replication_order_id_v1(&default_digest());
    let order = stx.world.replication_orders.get(&order_id).unwrap().clone();
    let assertion = AssertSorafsPublicationV1 {
        manifest_digest: default_digest(),
        order_id,
        assignment_revision: order.assignment_revision,
        canonical_order_digest: *blake3_hash(&order.canonical_order).as_bytes(),
        require_complete: false,
        challenge: [0x71; 32],
        minimum_height: 1,
        minimum_block_hash: completion_anchor().block_hash,
    };
    assertion.clone().execute(&alice(), &mut stx).unwrap();
    for changed in [
        AssertSorafsPublicationV1 {
            assignment_revision: 2,
            ..assertion.clone()
        },
        AssertSorafsPublicationV1 {
            canonical_order_digest: [3; 32],
            ..assertion.clone()
        },
        AssertSorafsPublicationV1 {
            minimum_block_hash: [4; 32],
            ..assertion.clone()
        },
        AssertSorafsPublicationV1 {
            challenge: [0; 32],
            ..assertion.clone()
        },
        AssertSorafsPublicationV1 {
            require_complete: true,
            ..assertion.clone()
        },
    ] {
        assert!(changed.execute(&alice(), &mut stx).is_err());
    }
    let other = AccountId::new(
        KeyPair::try_from_seed(vec![93; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    assert!(assertion.clone().execute(&other, &mut stx).is_err());
    let canonical = validate_stored_replication_order(&order, "publication fixture").unwrap();
    for assignment in canonical.assignments {
        completion_instruction(
            order_id,
            ProviderId::new(assignment.provider_id),
            5,
            &alice(),
        )
        .execute(&alice(), &mut stx)
        .unwrap();
    }
    AssertSorafsPublicationV1 {
        require_complete: true,
        ..assertion.clone()
    }
    .execute(&alice(), &mut stx)
    .unwrap();
    let mut pin = stx
        .world
        .pin_manifests
        .get(&default_digest())
        .unwrap()
        .clone();
    pin.pin_fee_payment = None;
    stx.world.pin_manifests.insert(pin.digest, pin);
    assert!(assertion.execute(&alice(), &mut stx).is_err());
}
