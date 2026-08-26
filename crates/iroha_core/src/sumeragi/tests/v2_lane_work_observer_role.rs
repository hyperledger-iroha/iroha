#[test]
fn observer_role_cannot_sign_lane_merge_or_native_amx_votes() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    adapter.voting_enabled = false;
    let request = native_request(&adapter, &keys);
    assert_eq!(adapter.local_validator_index(), None);
    assert!(adapter.sign_native_request_once(&request, 0).is_none());
    assert!(adapter.local_native_claims.is_empty());
}

#[test]
fn observer_does_not_open_or_mutate_the_merge_signing_guard() {
    let (validator, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    assert!(validator.merge_signing_guard.is_some());
    let candidate = merge_candidate_for_persistence_retry(&validator, 0);
    let signature =
        signed_merge_share_for_test(&validator, &keys, &candidate, validator.context.leader(0));
    let signer = signature.signer;
    let message_digest = signature.message_digest;
    let context = validator.context.clone();
    let local_peer = validator.local_peer.clone();
    let key_pair = validator.key_pair.clone();
    let state = Arc::clone(&validator.state);
    let kura = Arc::clone(&validator.kura);
    let limits = validator.limits;
    drop(validator);

    // A voting adapter must reject this unknown artifact while opening the
    // durable guard. A passive observer must not inspect that namespace.
    let guard_directory = kura.store_root().join("merge-signing-guard-v2");
    let guard_artifact = guard_directory.join("observer-must-ignore");
    std::fs::write(&guard_artifact, b"passive")
        .expect("plant an artifact which a voting guard would reject");

    let mut observer = V2LaneWorkAdapter::new_with_output_guard(
        context,
        local_peer,
        key_pair,
        false,
        state,
        kura,
        limits,
        None,
        None,
        ConsensusOutputGuard::isolated(),
    )
    .expect("non-voting construction must leave the signing namespace untouched");
    assert!(observer.merge_signing_guard.is_none());
    assert_eq!(
        observer.accept_relay_message(LaneRelayMessage::MergeSignature(signature), 0),
        V2LaneIngressOutcome::Rejected,
    );
    assert!(matches!(
        observer.authorize_local_merge_claim(&candidate, 0, signer, message_digest),
        Err(MergeSidecarError::SigningGuard(message)) if message.contains("non-voting")
    ));
    assert!(observer.merge_signing_guard.is_none());
    assert!(observer.merge_claims.is_empty());
    assert!(observer.merge_entries.is_empty());
    assert_eq!(
        std::fs::read(guard_artifact).expect("read passive guard artifact"),
        b"passive",
    );
}
