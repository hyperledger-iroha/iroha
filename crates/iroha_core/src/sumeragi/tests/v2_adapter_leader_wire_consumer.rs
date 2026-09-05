#[cfg(feature = "bls")]
fn actual_wal_consumer_fixture(
    directory: &TempDir,
) -> (
    SumeragiV2Adapter,
    Vec<KeyPair>,
    Arc<crate::sumeragi::FairV2Ingress>,
) {
    use crate::sumeragi::{
        FairV2Ingress, serviced_candidate_store::LeaderWireLifecycleStoreGate,
        v2_runtime::RuntimeLifecycleOrdinalSource,
    };
    let (context, keys, proofs) = authenticated_context();
    let wal_path = directory.path().join("consumer.wal");
    let (adapter, effects) = SumeragiV2Adapter::open(
        &wal_path,
        VerifiedHeightContext::genesis(context.clone(), proofs).expect("authenticate roster"),
        None,
        reducer::Generation::INITIAL,
        [0x61; 32],
        fingerprints(),
        deferred_admission_ordinals(),
    )
    .expect("open real safety WAL");
    assert!(effects.is_empty());
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<BTreeSet<_>>();
    let (gate, restore) = LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(
        adapter
            .mint_leader_wire_store_authority(&wal_path)
            .expect("mint exact sibling store"),
        context.id(),
        context.height,
        adapter.fingerprints.node.into(),
        roster.clone(),
        LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            context.da_layout.max_chunk_count,
        )
        .expect("bounded slots"),
        context.da_layout.max_chunk_count,
        adapter
            .leader_wire_recovery_authority()
            .expect("actual WAL consumer"),
        &[],
        &[],
    )
    .expect("open actual WAL-owned ingress store");
    let ingress = Arc::new(
        FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            crate::sumeragi::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            8 * 1024 * 1024,
            crate::sumeragi::fair_v2_ingress_required_transport_completion_bytes(context.da_layout)
                .max(crate::sumeragi::MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES),
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    ingress
        .configure_roster_for_context(roster, &context.network_id, context.da_layout)
        .expect("configure exact ingress roster");
    ingress.require_leader_wire_lifecycle_gate();
    ingress
        .bind_leader_wire_lifecycle_gate(
            gate,
            restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            context.id(),
            context.height,
        )
        .expect("bind WAL gate");
    ingress.open().expect("open ingress");
    (adapter, keys, ingress)
}

#[cfg(feature = "bls")]
fn deliver_actual_wal_consumer_wire(
    adapter: &mut SumeragiV2Adapter,
    ingress: &crate::sumeragi::FairV2Ingress,
    payload: wire::ConsensusMessageV2Payload,
    sender: wire::ValidatorIndex,
) -> (
    AdapterOutcome,
    crate::sumeragi::serviced_candidate_store::LeaderWireLifecycleRuntimeReceipt,
) {
    use crate::sumeragi::{
        FairV2IngressPushDisposition, InboundBlockMessage, message::BlockMessage,
    };
    let message = wire::ConsensusMessageV2::new(payload);
    let authenticated = adapter
        .authenticate(message.clone())
        .expect("verify actual BLS wire");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message),
            adapter.wire_context.roster[sender as usize]
                .validator
                .clone()
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let mut inbound = ingress.try_recv().expect("dequeue actual owned wire");
    let ownership = inbound
        .take_ingress_ownership()
        .expect("physical ingress ownership");
    let receipt = ownership
        .leader_wire_runtime_receipt()
        .expect("bound actual runtime receipt")
        .clone();
    let outcome = adapter
        .receive_authenticated(authenticated)
        .expect("consume authenticated input and persist WAL");
    ingress
        .advance_leader_wire_recovery_cut(
            adapter
                .leader_wire_recovery_authority()
                .expect("post-WAL authority"),
        )
        .expect("publish actual WAL frontier");
    (outcome, receipt)
}

#[cfg(feature = "bls")]
#[test]
fn actual_wal_same_round_timeout_upgrade_rearms_consumed_prepare_without_reminting() {
    for retire_before_upgrade in [false, true] {
        let directory = TempDir::new().expect("real consumer stores");
        let (mut adapter, keys, ingress) = actual_wal_consumer_fixture(&directory);
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let thin = authenticated_timeout_certificate(round, None, vec![0, 1, 2], &keys);
        let (_, thin_receipt) = deliver_actual_wal_consumer_wire(
            &mut adapter,
            &ingress,
            wire::ConsensusMessageV2Payload::TimeoutCertificate(thin),
            0,
        );
        ingress
            .mark_leader_wire_volatile_terminal(&thin_receipt)
            .expect("retire installed TC carrier");
        let mut vote = wire::Vote {
            round: wire::ConsensusRound { view: 1, ..round },
            proposal_round: wire::ConsensusRound { view: 1, ..round },
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0x71),
            execution_commitment: execution_commitment(0x71),
            signer: 1,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(keys[1].private_key(), &vote.signature_preimage())
            .payload()
            .to_vec();
        let (first, first_receipt) = deliver_actual_wal_consumer_wire(
            &mut adapter,
            &ingress,
            wire::ConsensusMessageV2Payload::Vote(vote.clone()),
            1,
        );
        assert_eq!(first.disposition(), reducer::StepDisposition::Applied);
        let consumed_tag = adapter.current_tag();
        if retire_before_upgrade {
            ingress
                .mark_leader_wire_volatile_terminal(&first_receipt)
                .expect("release first consumer");
        }
        let mut prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0x72),
            execution_commitment: execution_commitment(0x72),
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut prepare, &keys);
        let upgrade = authenticated_timeout_certificate(round, Some(prepare), vec![0, 1, 2], &keys);
        let (installed, upgrade_receipt) = deliver_actual_wal_consumer_wire(
            &mut adapter,
            &ingress,
            wire::ConsensusMessageV2Payload::TimeoutCertificate(upgrade),
            0,
        );
        assert!(
            installed
                .effects()
                .iter()
                .any(|effect| matches!(effect, AdapterEffect::EnterView { .. }))
        );
        assert_eq!(adapter.current_tag().view(), consumed_tag.view());
        assert!(adapter.current_tag().strictly_advances(consumed_tag));
        ingress
            .mark_leader_wire_volatile_terminal(&upgrade_receipt)
            .expect("release stronger TC carrier");
        if !retire_before_upgrade {
            ingress
                .mark_leader_wire_volatile_terminal(&first_receipt)
                .expect("late old-epoch departure reopens exact token");
        }
        let (retried, retry_receipt) = deliver_actual_wal_consumer_wire(
            &mut adapter,
            &ingress,
            wire::ConsensusMessageV2Payload::Vote(vote),
            1,
        );
        assert_eq!(
            retried.disposition(),
            reducer::StepDisposition::Applied,
            "TC cleared the old volatile Prepare pool"
        );
        assert_eq!(
            retry_receipt.token(),
            first_receipt.token(),
            "logical identity and both ordinals survive consumer replacement"
        );
        assert_ne!(
            retry_receipt, first_receipt,
            "runtime consumer epoch prevents stale receipt ABA"
        );
        assert!(
            ingress
                .mark_leader_wire_volatile_terminal(&first_receipt)
                .is_err(),
            "old receipt cannot retire the replacement consumer"
        );
        ingress
            .mark_leader_wire_volatile_terminal(&retry_receipt)
            .expect("release exact replacement consumer");
    }
}

#[cfg(feature = "bls")]
#[test]
fn actual_wal_view_cut_admits_old_prepare_observation_without_inventing_commit_intent() {
    let directory = TempDir::new().expect("real observation stores");
    let (mut adapter, keys, ingress) = actual_wal_consumer_fixture(&directory);
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let thin = authenticated_timeout_certificate(round, None, vec![0, 1, 2], &keys);
    let (_, receipt) = deliver_actual_wal_consumer_wire(
        &mut adapter,
        &ingress,
        wire::ConsensusMessageV2Payload::TimeoutCertificate(thin),
        0,
    );
    ingress
        .mark_leader_wire_volatile_terminal(&receipt)
        .expect("release TC");
    let mut prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0x73),
        execution_commitment: execution_commitment(0x73),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut prepare, &keys);
    let old_id = adapter.reducer.durable_state().last_id();
    let (observed, receipt) = deliver_actual_wal_consumer_wire(
        &mut adapter,
        &ingress,
        wire::ConsensusMessageV2Payload::QuorumCertificate(prepare.clone()),
        0,
    );
    assert_eq!(observed.disposition(), reducer::StepDisposition::Applied);
    assert!(adapter.reducer.durable_state().last_id() > old_id);
    assert_eq!(
        adapter
            .reducer
            .durable_state()
            .highest_prepare()
            .expect("persisted historical Prepare")
            .round()
            .view(),
        0
    );
    assert!(
        adapter.reducer.durable_state().locked().is_none(),
        "ObservePrepare does not invent a lock or local CommitIntent"
    );
    ingress
        .mark_leader_wire_volatile_terminal(&receipt)
        .expect("release observed QC");
    let mut commit = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: prepare.subject,
        execution_commitment: prepare.execution_commitment,
        signer: 1,
        signature: Vec::new(),
    };
    commit.signature = Signature::new(keys[1].private_key(), &commit.signature_preimage())
        .payload()
        .to_vec();
    let payload = wire::ConsensusMessageV2Payload::Vote(commit);
    assert!(
        !adapter
            .leader_wire_recovery_authority()
            .expect("actual authority")
            .admits_payload(&payload)
    );
}
#[cfg(feature = "bls")]
#[test]
fn commit_vote_statement_hash_binds_round_subject_and_execution() {
    let (context, _, _) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = subject(0x71);
    let commitment = execution_commitment(0x71);
    let expected = super::leader_wire_vote_statement_hash(round, subject, &commitment);
    assert_ne!(
        expected,
        super::leader_wire_vote_statement_hash(
            wire::ConsensusRound { view: 1, ..round },
            subject,
            &commitment,
        )
    );
    let mut changed_subject = subject;
    changed_subject.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"other vote block"));
    assert_ne!(
        expected,
        super::leader_wire_vote_statement_hash(round, changed_subject, &commitment,)
    );
    assert_ne!(
        expected,
        super::leader_wire_vote_statement_hash(round, subject, &execution_commitment(0x72),)
    );
}
