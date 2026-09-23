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
    let mut proposal = v2_maximum_structural_proposal_wire(minimal_rs16_layout(), 4);
    let BlockMessage::V2(envelope) = &mut proposal else {
        unreachable!()
    };
    let wire::ConsensusMessageV2Payload::Proposal(value) = &mut envelope.payload else {
        unreachable!()
    };
    // This fixture otherwise uses the maximum wire-size height. Rollover needs
    // a reachable successor, with every embedded round at the same height.
    let height = 41;
    value.round.height = height;
    value.manifest.round.height = height;
    let wire::ProposalJustification::Timeout(justification) = &mut value.justification else {
        unreachable!()
    };
    justification.timeout_certificate.round.height = height;
    for certificate in justification
        .timeout_certificate
        .groups
        .iter_mut()
        .filter_map(|group| group.highest_prepare_qc.as_mut())
        .chain(justification.highest_prepare_qc.as_mut())
    {
        certificate.round.height = height;
        certificate.proposal_round.height = height;
    }
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
    ingress.close();
    assert!(
        ingress.ensure_closed_global_drained_cut().is_err(),
        "a queued global proposal still prevents the global cut"
    );
    ingress.retire_leader_wire_lifecycle_gate(&gate).unwrap();
    ingress.ensure_closed_global_drained_cut().unwrap();
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
    ingress.ensure_closed_global_drained_cut().unwrap();
}

#[test]
fn native_ingress_global_cut_preserves_multiple_original_occurrences() {
    let (_handle, ingress, _relay) = test_sumeragi_handle_with_source_geometry(40, Some(2));
    let validators = validator_peers(4);
    ingress.close();
    ingress.configure_roster(validators.clone()).unwrap();
    ingress.open().unwrap();
    for message in native_wire_classification_fixtures() {
        for _ in 0..2 {
            ingress.try_push_owned_at(
                InboundBlockMessage::from_authenticated_peer(message.clone(), validators[0].clone()),
                Instant::now(),
            ).unwrap();
        }
    }
    assert!(ingress.ensure_closed_global_drained_cut().is_err());
    ingress.close();
    let original = {
        let state = ingress.state.lock();
        assert_eq!(state.len, 2, "duplicates retain the two original occurrences");
        let entries = &state.lanes[&super::FairV2IngressSource::Native(validators[0].clone())].entries;
        entries.iter().map(|entry| (
            Arc::as_ptr(&entry.inbound), entry.encoded_bytes.as_ptr(), entry.admission_ordinal,
            entry.ownership_snapshot.process_local_projection_hash(), entry.encoded_len,
        )).collect::<Vec<_>>()
    };
    ingress.ensure_closed_global_drained_cut().expect("global finality cannot require Native delivery");
    assert!(ingress.ensure_closed_drained_cut().is_err(), "process-wide drain still sees both owners");
    for (allocation, bytes, ordinal, projection, length) in original {
        {
            let state = ingress.state.lock();
            let entry = &state.lanes[&super::FairV2IngressSource::Native(validators[0].clone())].entries[0];
            assert_eq!(Arc::as_ptr(&entry.inbound), allocation);
            assert_eq!(entry.encoded_bytes.as_ptr(), bytes);
            assert_eq!(entry.admission_ordinal, ordinal);
            assert_eq!(entry.ownership_snapshot.process_local_projection_hash(), projection);
            assert_eq!(entry.encoded_len, length);
        }
        let inbound = ingress.try_recv_if_checked(|message| message.message().is_native_lane()).unwrap().unwrap();
        assert_eq!(inbound.ingress_ownership().unwrap().physical_admission_ordinal(), Some(ordinal));
        ingress.ensure_closed_global_drained_cut().unwrap();
    }
    ingress.ensure_closed_drained_cut().unwrap();
}

#[test]
fn native_ingress_global_cut_rejects_corrupted_retained_accounting_and_custody() {
    for corruption in 0..13 {
        let (_handle, ingress, _relay) = test_sumeragi_handle_with_source_geometry(40, Some(2));
        let validators = validator_peers(4);
        ingress.close();
        ingress.configure_roster(validators.clone()).unwrap();
        ingress.open().unwrap();
        let [message, _] = native_wire_classification_fixtures();
        ingress.try_push_owned_at(
            InboundBlockMessage::from_authenticated_peer(message, validators[0].clone()),
            Instant::now(),
        ).unwrap();
        ingress.close();
        ingress.ensure_closed_global_drained_cut().unwrap();
        {
            let mut state = ingress.state.lock();
            if corruption == 0 { state.bytes += 1; }
            else if corruption == 1 { state.len += 1; }
            else if corruption == 2 { state.ready.clear(); }
            else if corruption == 3 { state.pending_wire_owners.clear(); }
            else if corruption == 4 { state.nonempty_since = None; }
            else {
                let lane = state.lanes.get_mut(&super::FairV2IngressSource::Native(validators[0].clone())).unwrap();
                match corruption {
                    5 => lane.bytes += 1,
                    6 => lane.pending_wire.clear(),
                    7 => lane.progress_len += 1,
                    8 => lane.timeout_vote_bytes += 1,
                    9 => lane.transport_completion_len += 1,
                    10 => lane.entries[0].admission_ordinal += 1,
                    11 => Arc::make_mut(&mut lane.entries[0].inbound).sender = validators[1].clone(),
                    12 => Arc::make_mut(&mut lane.entries[0].ownership_snapshot).admission_count += 1,
                    _ => unreachable!(),
                }
            }
        }
        assert!(ingress.ensure_closed_global_drained_cut().is_err(), "corruption {corruption}");
    }
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
