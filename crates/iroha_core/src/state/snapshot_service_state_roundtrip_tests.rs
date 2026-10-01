// The mandatory service envelopes participate in the complete State snapshot.

state_test! { sync service_snapshot_roundtrip_preserves_state_hash_and_actual_replacement
    let service = super::snapshot_service_state::tests::service_world();
    let mut chain = crate::sumeragi::test_chain::CertifiedTestChain::start(
        crate::sumeragi::test_chain::TestChainConfig::new(service, 1_000),
    ).expect("native genesis retains original service state");
    chain.commit(Vec::new());
    let state = chain.state();
    let snapshot = norito::json::to_value(state.as_ref()).unwrap();
    assert!(matches!(
        deserialize_state_snapshot_value_with_kura(snapshot.clone(), Arc::clone(&state.kura)),
        Err(super::deserialize::StateRestoreError::NativeExecutionReplayRequired)
    ));
    let mut original_service = String::new();
    super::snapshot_service_state::serialize(&state.world, &mut original_service);
    // These fourteen service envelopes belong to the outer State serializer,
    // not to World's ordinary JSON field set. Use their canonical DTO owner.
    let component: super::snapshot_service_state::SnapshotServiceState =
        norito::json::from_json(&format!("{{{}}}", &original_service[1..]))
            .expect("decode complete service current and predecessor envelopes");
    let mut decoded_world = World::default();
    component.restore(&mut decoded_world).expect("validate exact service component");
    let mut decoded_service = String::new();
    super::snapshot_service_state::serialize(&decoded_world, &mut decoded_service);
    assert_eq!(decoded_service, original_service);
    let mut replay = crate::sumeragi::test_chain::CertifiedTestChain::start(
        crate::sumeragi::test_chain::TestChainConfig::new(
            super::snapshot_service_state::tests::service_world(), 1_000,
        ),
    ).expect("same original signed service genesis");
    replay.replay_from(&chain).expect("replay original certified service history");
    let restored = Arc::clone(replay.state());
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap());
    let mut before = String::new();
    super::snapshot_service_state::serialize(&restored.world, &mut before);
    let carrier = restored.kura.get_block(NonZeroUsize::new(restored.committed_height()).unwrap()).unwrap();
    {
        let replacement = restored.block_and_revert(carrier.header());
        assert_eq!(*replacement.world.soradns_directory_latest.get(), Some([1; 32]));
        assert_eq!(replacement.world.capacity_disputes.len(), 1);
        assert_eq!(replacement.world.soradns_release_signers.len(), 1);
        assert!(replacement.world.soradns_directory_pending.is_empty());
    }
    let mut after = String::new();
    super::snapshot_service_state::serialize(&restored.world, &mut after);
    assert_eq!(before, after);
    let mut missing = snapshot;
    missing.as_object_mut().unwrap().remove("provider_credit_ledger");
    assert!(deserialize_state_snapshot_value_with_kura(missing, Arc::clone(&state.kura)).is_err());
}
