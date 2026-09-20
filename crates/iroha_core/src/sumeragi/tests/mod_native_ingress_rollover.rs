// Physical ingress tests: authenticated hops and exact canonical bytes do not
// themselves grant Native voting authority. The live Native entrypoint stays closed.

fn native_rollover_gate(
    ingress: &Arc<super::FairV2Ingress>,
    validators: &[PeerId],
    round: wire::ConsensusRound,
) -> TempDir {
    ingress.close();
    ingress
        .configure_roster(validators.iter().cloned())
        .unwrap();
    if !ingress.state.lock().requires_leader_wire_lifecycle_gate {
        ingress.require_leader_wire_lifecycle_gate();
    }
    ingress.state.lock().leader_wire_max_chunk_count = 2;
    let directory = TempDir::new().unwrap();
    let owner = [0xA7; 32];
    let capacity = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
        validators.len(),
        2,
    )
    .unwrap();
    let authority =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            0,
            false,
        );
    let (gate, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &directory.path().join("safety.wal"),
        round.context_id,
        round.height,
        owner,
        validators.iter().cloned().collect(),
        capacity,
        2,
        authority,
        &[],
        &[],
    )
    .unwrap();
    ingress
        .bind_leader_wire_lifecycle_gate(
            gate,
            restore,
            super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            round.context_id,
            round.height,
        )
        .unwrap();
    ingress.open().unwrap();
    directory
}

#[test]
fn native_ingress_rollover_preserves_original_physical_owner_through_both_cuts() {
    let (_handle, ingress, _relay) = test_sumeragi_handle_with_source_geometry(40, Some(2));
    let validators = validator_peers(4);
    let proposal = v2_maximum_structural_proposal_wire(minimal_rs16_layout(), 4);
    let BlockMessage::V2(envelope) = &proposal else {
        unreachable!()
    };
    let wire::ConsensusMessageV2Payload::Proposal(value) = &envelope.payload else {
        unreachable!()
    };
    let round = value.round;
    let _first_directory = native_rollover_gate(&ingress, &validators, round);
    let [native, _] = native_wire_classification_fixtures();
    let hop = validators[0].clone();
    let inbound = || InboundBlockMessage::from_authenticated_peer(native.clone(), hop.clone());
    // Public ingress cannot activate the new protocol before its consumer cutover.
    assert!(matches!(
        ingress.try_push(inbound()),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
    ingress
        .try_push_owned_at(inbound(), Instant::now())
        .unwrap();
    ingress
        .try_push(InboundBlockMessage::from_authenticated_peer(
            proposal,
            hop.clone(),
        ))
        .unwrap();
    let source = super::FairV2IngressSource::Native(hop.clone());
    let (allocation, encoded, projection, ordinal, byte_count, gate) = {
        let state = ingress.state.lock();
        let entry = &state.lanes[&source].entries[0];
        let evidence = entry.ownership_snapshot.as_ref();
        assert!(evidence.validate_exact());
        assert_eq!(evidence.runtime_lifecycle_ordinal(), None);
        assert!(!evidence.first.authenticated_via_is_validator);
        (
            Arc::as_ptr(&entry.inbound),
            entry.encoded_bytes.as_ptr(),
            evidence.process_local_projection_hash(),
            entry.admission_ordinal,
            entry.encoded_len,
            state.leader_wire_lifecycle_gate.as_ref().unwrap().clone(),
        )
    };
    ingress.retire_leader_wire_lifecycle_gate(&gate).unwrap();
    assert!(
        ingress.ensure_closed_drained_cut().is_err(),
        "Native bytes still prevent a full shutdown drain"
    );
    let mut successor = round;
    successor.height += 1;
    successor.context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(CryptoHash::new(
        b"next global context",
    )));
    let next_validators = validator_peers(8).into_iter().skip(4).collect::<Vec<_>>();
    assert!(!next_validators.contains(&hop));
    let _next_directory = native_rollover_gate(&ingress, &next_validators, successor);
    {
        let state = ingress.state.lock();
        let entry = &state.lanes[&source].entries[0];
        assert_eq!(Arc::as_ptr(&entry.inbound), allocation);
        assert_eq!(entry.encoded_bytes.as_ptr(), encoded);
        assert_eq!(
            entry.ownership_snapshot.process_local_projection_hash(),
            projection
        );
        assert_eq!(entry.admission_ordinal, ordinal);
        assert_eq!(state.len, 1);
        assert_eq!(state.bytes, byte_count);
        assert_eq!(state.pending_wire_owners.len(), 1);
        assert_eq!(
            state.ready.iter().cloned().collect::<Vec<_>>(),
            vec![source.clone()]
        );
        assert!(!state.has_global_ingress());
    }
    // Duplicate delivery after a roster change retains the original physical owner.
    assert!(matches!(
        ingress.try_push_owned_at(inbound(), Instant::now()),
        Ok(super::FairV2IngressPushDisposition::Coalesced)
    ));
    let mut delivered = ingress
        .try_recv_if(|message| message.message().is_native_lane())
        .unwrap();
    let evidence = delivered.take_ingress_ownership().unwrap();
    assert!(evidence.validate_exact());
    assert_eq!(evidence.physical_admission_ordinal(), Some(ordinal));
    assert_eq!(evidence.runtime_lifecycle_ordinal(), None);
    assert_eq!(delivered.message().encode(), native.encode());
    assert!(!ingress.state.lock().lanes.contains_key(&source));
    ingress.close();
    ingress.ensure_closed_drained_cut().unwrap();
}

#[test]
fn native_ingress_shares_bounded_authenticated_source_capacity_with_global_traffic() {
    let (_handle, ingress, _relay) = test_sumeragi_handle_with_source_geometry(40, Some(1));
    let validators = validator_peers(4);
    ingress.close();
    ingress.configure_roster(validators).unwrap();
    ingress.open().unwrap();
    let hop = authenticated_peer_for_test();
    let [native, _] = native_wire_classification_fixtures();
    let inbound = || InboundBlockMessage::from_authenticated_peer(native.clone(), hop.clone());
    ingress
        .try_push_owned_at(inbound(), Instant::now())
        .unwrap();
    assert!(
        matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                v2_auxiliary_prepare(0),
                hop.clone()
            )),
            Err(super::FairV2IngressPushError::Full(_))
        ),
        "same peer's second protocol namespace still costs one source slot"
    );
    assert!(ingress.try_recv_if(|_| true).is_some());
    ingress
        .try_push(InboundBlockMessage::from_authenticated_peer(
            v2_auxiliary_prepare(0),
            hop.clone(),
        ))
        .unwrap();
    assert!(matches!(
        ingress.try_push_owned_at(inbound(), Instant::now()),
        Err(super::FairV2IngressPushError::Full(_))
    ));
    assert!(ingress.try_recv_if(|_| true).is_some());
    ingress
        .try_push_owned_at(inbound(), Instant::now())
        .unwrap();
    ingress.debug_assert_consistent(&ingress.state.lock());
}

#[test]
fn native_ingress_capacity_refusal_keeps_retained_bytes_available_to_drain() {
    let (_handle, ingress, _relay) = test_sumeragi_handle_with_source_geometry(24, Some(1));
    ingress.close();
    ingress.configure_roster(validator_peers(4)).unwrap();
    ingress.open().unwrap();
    let [native, _] = native_wire_classification_fixtures();
    let hop = authenticated_peer_for_test();
    ingress
        .try_push_owned_at(
            InboundBlockMessage::from_authenticated_peer(native.clone(), hop),
            Instant::now(),
        )
        .unwrap();
    let before = ingress.state.lock().bytes;
    assert!(ingress.configure_roster(validator_peers(7)).is_err());
    assert!(ingress.open().is_err());
    assert_eq!(ingress.len(), 1);
    assert_eq!(ingress.state.lock().bytes, before);
    let delivered = ingress.try_recv_if(|_| true).unwrap();
    assert_eq!(delivered.message().encode(), native.encode());
    ingress.ensure_closed_drained_cut().unwrap();
}
