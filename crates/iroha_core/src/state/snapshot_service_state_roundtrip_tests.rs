// The mandatory service envelopes participate in the complete State snapshot.

state_test! { sync service_snapshot_roundtrip_preserves_state_hash_and_actual_replacement
    let mut fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let service = super::snapshot_service_state::tests::service_world();
    let state = &mut fixture.native.state;
    macro_rules! move_stores {
        ($($field:ident),+ $(,)?) => {$(state.world.$field = service.0.$field;)+};
    }
    move_stores!(capacity_fee_ledger, capacity_disputes, provider_credit_ledger, sorafs_pricing,
        soradns_directory_records, soradns_directory_pending, soradns_directory_history,
        soradns_directory_prev_of, soradns_directory_revocations, soradns_release_signers,
        soradns_directory_latest, soradns_rotation_policy, soradns_last_publish_ms, soradns_history_len);
    let snapshot = norito::json::to_value(&*state).unwrap();
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
