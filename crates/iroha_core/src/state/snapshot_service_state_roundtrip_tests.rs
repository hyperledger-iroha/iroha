// The mandatory service envelopes participate in the complete State snapshot.

state_test! { sync service_snapshot_roundtrip_preserves_state_hash_and_actual_replacement
    let service = super::snapshot_service_state::tests::service_world();
    let mut chain = crate::sumeragi::test_chain::CertifiedTestChain::start(
        crate::sumeragi::test_chain::TestChainConfig::new(service, 1_000),
    ).expect("native genesis retains original service state");
    chain.commit(Vec::new());
    let state = chain.state();
    let snapshot = norito::json::to_value(state.as_ref()).unwrap();
    let restored = deserialize_state_snapshot_value_with_kura(snapshot.clone(), Arc::clone(&state.kura)).unwrap();
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
