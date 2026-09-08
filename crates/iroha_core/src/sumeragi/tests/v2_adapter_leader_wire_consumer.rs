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
            crate::sumeragi::fair_v2_ingress_required_certified_fence_escape_bytes(roster.len()),
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
        adapter
            .registry
            .register_execution_commitment(
                reducer::Round::new(vote.proposal_round.height, vote.proposal_round.view),
                reducer::Subject::new(Hash::new(vote.subject.encode()).into()),
                vote.execution_commitment,
            )
            .expect("bind the locally checked body before admitting its direct remote vote");
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
        let authority = adapter
            .leader_wire_recovery_authority()
            .expect("actual upgraded WAL authority");
        let exact_lock = Some((round, subject(0x72)));
        assert!(authority.matches_entered_view(adapter.current_tag(), exact_lock));
        assert!(!authority.matches_entered_view(consumed_tag, exact_lock));
        assert!(!authority.matches_entered_view(adapter.current_tag(), None));
        assert!(
            !authority.matches_entered_view(adapter.current_tag(), Some((round, subject(0x71))),)
        );
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

#[cfg(feature = "bls")]
#[test]
fn actual_wal_fence_prediction_rejects_terminal_votes_before_reducer_busy() {
    use crate::sumeragi::v2_runtime::AdapterCommand;

    for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
        let directory = TempDir::new().expect("real reducer-fence WAL");
        let (context, keys, proofs) = authenticated_context();
        let (mut adapter, _startup) = SumeragiV2Adapter::open(
            directory.path().join("fence.wal"),
            VerifiedHeightContext::genesis(context.clone(), proofs)
                .expect("authenticate frozen BLS roster"),
            Some(0),
            reducer::Generation::INITIAL,
            [0x63; 32],
            fingerprints(),
            deferred_admission_ordinals(),
        )
        .expect("open actual voting adapter");
        let old_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let timeout = authenticated_timeout_certificate(old_round, None, vec![0, 1, 2], &keys);
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
            ))
            .expect("verify actual TC");
        let entered = adapter
            .receive_authenticated(authenticated)
            .expect("persist TC and enter view one");
        assert!(entered.effects().iter().any(
            |effect| matches!(effect, AdapterEffect::EnterView { tag, .. } if tag.view() == 1)
        ));
        let mut observed_prepare = wire::QuorumCertificate {
            round: old_round,
            proposal_round: old_round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0x79),
            execution_commitment: execution_commitment(0x79),
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut observed_prepare, &keys);
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(observed_prepare.clone()),
            ))
            .expect("verify the historical PrepareQC");
        adapter
            .receive_authenticated(authenticated)
            .expect("persist actual ObservePrepare and bind the exact execution statement");
        assert!(
            adapter.reducer.durable_state().locked().is_none(),
            "ObservePrepare does not mint a CommitIntent"
        );
        let tag = adapter.current_tag();
        let timeout = adapter
            .timeout_elapsed(tag)
            .expect("persist local timeout intent and open the real signer fence");
        assert!(matches!(
            timeout.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));
        assert!(adapter.reducer.pending_persistence_record().is_none());
        let signer_before = adapter.reducer.awaiting_signature().cloned();
        assert!(signer_before.is_some());
        let wal_before = adapter.reducer.durable_state().last_id();
        let mut vote = wire::Vote {
            round: old_round,
            proposal_round: old_round,
            phase,
            subject: observed_prepare.subject,
            execution_commitment: observed_prepare.execution_commitment,
            signer: 1,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(keys[1].private_key(), &vote.signature_preimage())
            .payload()
            .to_vec();
        let payload = wire::ConsensusMessageV2Payload::Vote(vote);
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(payload.clone()))
            .expect(
                "authenticate the valid old-view vote against the QC-bound execution statement",
            );
        assert!(
            !adapter
                .leader_wire_recovery_authority()
                .expect("actual WAL eligibility")
                .admits_payload(&payload)
        );
        assert!(
            !adapter.authenticated_command_reaches_fenced_reducer(&authenticated),
            "a retired Prepare or unowned Commit cannot justify skipping its FIFO occurrence for a signing completion"
        );
        assert!(!adapter.command_is_blocked_by_deferred_fence(
            tag,
            &AdapterCommand::Authenticated(authenticated.clone())
        ));
        let rejected = adapter
            .receive_authenticated(authenticated)
            .expect("terminal admission remains healthy");
        assert_eq!(
            rejected.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::IrrelevantView)
        );
        assert!(rejected.effects().is_empty());
        assert!(rejected.deferred_admission_ordinal().is_none());
        assert_eq!(adapter.reducer.awaiting_signature().cloned(), signer_before);
        assert_eq!(adapter.reducer.durable_state().last_id(), wal_before);

        let mut current = wire::TimeoutVote {
            round: wire::ConsensusRound {
                view: tag.view(),
                ..old_round
            },
            highest_prepare_qc: None,
            signer: 2,
            signature: Vec::new(),
        };
        current.signature = Signature::new(keys[2].private_key(), &current.signature_preimage())
            .payload()
            .to_vec();
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(current),
            ))
            .expect("authenticate the fresh current-view Busy control");
        assert!(adapter.authenticated_command_reaches_fenced_reducer(&authenticated));
        assert!(adapter.command_is_blocked_by_deferred_fence(
            tag,
            &AdapterCommand::Authenticated(authenticated.clone())
        ));
        let blocked = adapter
            .receive_authenticated(authenticated)
            .expect("retain the exact current-view vote behind the signer");
        assert_eq!(
            blocked.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert!(blocked.deferred_admission_ordinal().is_some());
        assert_eq!(adapter.reducer.awaiting_signature().cloned(), signer_before);
        assert_eq!(adapter.reducer.durable_state().last_id(), wal_before);
    }
}
