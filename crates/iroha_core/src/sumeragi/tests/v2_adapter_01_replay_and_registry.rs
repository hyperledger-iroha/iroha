#[test]
fn proposal_registry_preserves_the_first_exact_semantic_envelope() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let wire::ConsensusMessageV2Payload::Proposal(first) =
        proposal(&context, context.leader(0), subject(0x40)).payload
    else {
        unreachable!("proposal fixture")
    };
    let mut later = first.clone();
    later.signature = vec![0x40; 96];
    registry
        .proposal_to_core(&first, &context)
        .expect("register first exact proposal envelope");
    registry
        .proposal_to_core(&later, &context)
        .expect("the same semantic proposal remains convertible");
    let key = (
        reducer::Round::new(first.round.height, first.round.view),
        reducer::Subject::new(Hash::new(first.subject.encode()).into()),
    );
    assert_eq!(
        registry.proposals.get(&key),
        Some(&first),
        "a later exact-envelope alias cannot retarget durable re-signing"
    );
}
#[test]
fn canonical_body_rolls_back_exact_busy_deferred_conflicting_proposal() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let context = adapter.wire_context.clone();
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0x3F);
    let canonical = proposal(&context, proposer, subject);
    let wire::ConsensusMessageV2Payload::Proposal(canonical_proposal) = &canonical.payload else {
        panic!("fixture is a proposal")
    };
    let canonical_manifest = canonical_proposal.manifest.clone();
    let round = canonical_manifest.round;
    let mut conflicting = proposal(&context, proposer, subject);
    let conflicting_proposal = {
        let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) =
            &mut conflicting.payload
        else {
            panic!("fixture is a proposal")
        };
        let alternate_body = b"other";
        let alternate_chunks = wire::encode_payload_chunks(context.da_layout, alternate_body)
            .expect("encode complete canonical alternate-body chunks");
        // Deliberately bind the complete canonical alternate body to the
        // original subject so this remains a manifest-conflict negative.
        conflicting_proposal.manifest = wire::PayloadManifest::derive(
            &context,
            conflicting_proposal.round,
            conflicting_proposal.subject,
            u64::try_from(alternate_body.len()).expect("alternate body length fits u64"),
            &alternate_chunks,
        )
        .expect("structurally valid alternate manifest");
        conflicting_proposal.clone()
    };
    let conflicting_wire_identity = Arc::<[u8]>::from(conflicting.encode());
    let deferred = adapter
        .registry
        .proposal_to_core(&conflicting_proposal, &context)
        .expect("convert authenticated proposal before reducer reports Busy");
    let deferred_tag = adapter.current_tag();
    adapter.deferred_inputs.push_back(DeferredInput {
        admission_ordinal: 1,
        admission_capability: DeferredAdmissionCapability::for_authenticated_test(1),
        event: reducer::Event::ProposalReceived {
            tag: deferred_tag,
            proposal: deferred,
        },
        completion_evidence: None,
        retag_authenticated_ingress: true,
        priority: DeferredPriority::Normal,
        protected_progress: false,
        admission: None,
        authenticated_wire_identity: Some(conflicting_wire_identity),
        admitted_at: Instant::now(),
        eligible_skips: 0,
    });
    let admission_key = IngressSemanticKey::Proposal { round, proposer };
    adapter.ingress_equivocations.insert(
        admission_key,
        IngressEquivocationRecord {
            fingerprint: IngressFingerprint::Proposal(Hash::new(
                conflicting_proposal.signature_preimage(),
            )),
            artifact: IngressEquivocationArtifact::Proposal(Arc::new(conflicting_proposal.clone())),
            equivocation_reported: true,
            capacity_bypass: false,
            admitted_at: Instant::now(),
        },
    );
    adapter.ingress_deliveries.insert(
        admission_key,
        IngressDeliveryRecord {
            fingerprint: IngressFingerprint::Proposal(Hash::new(
                conflicting_proposal.signature_preimage(),
            )),
            consumer_tag: deferred_tag,
            locked_commit_progress: false,
            locked_reproposal_prepare_progress: false,
        },
    );
    let body_command = super::super::v2_runtime::AdapterCommand::BodyAvailable {
        manifest: canonical_manifest.clone(),
    };
    adapter
        .deferred_inputs
        .front_mut()
        .expect("the conflicting proposal remains deferred")
        .retag_authenticated_ingress = false;
    assert_eq!(
        adapter.preflight_runtime_command_admission(deferred_tag, &body_command),
        super::super::v2_runtime::RuntimeCommandAdmissionPreflight::Reject,
        "a generic deferred item cannot authorize proposal-registry rollback"
    );
    adapter
        .deferred_inputs
        .front_mut()
        .expect("the conflicting proposal remains deferred")
        .retag_authenticated_ingress = true;
    assert_eq!(
        adapter.preflight_runtime_command_admission(deferred_tag, &body_command),
        super::super::v2_runtime::RuntimeCommandAdmissionPreflight::Admit,
        "preflight must project the exact rollback supported by dispatch"
    );
    let retained_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(0x3F),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x3F; 96],
    };
    adapter
        .registry
        .qc_to_core(&retained_qc, &context)
        .expect("register independently authenticated QC material");
    let retained_certificates = adapter.registry.certificates.clone();
    let retained_execution_commitments = adapter.registry.execution_commitments.clone();
    assert!(adapter.registry.manifest_conflicts(&canonical_manifest));
    assert!(
        adapter
            .prepare_direct_certified_body_available(deferred_tag, &canonical_manifest)
            .is_err(),
        "the direct lifecycle preview rejects a conflicting deferred proposal"
    );
    assert_eq!(adapter.deferred_inputs.len(), 1);
    assert!(adapter.registry.manifest_conflicts(&canonical_manifest));
    let outcome = adapter
        .body_available(deferred_tag, canonical_manifest.clone())
        .expect("canonical body supersedes only its Busy-deferred proposal authority");
    assert_eq!(
        outcome.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
    assert!(adapter.deferred_inputs.is_empty());
    assert!(!adapter.ingress_equivocations.contains_key(&admission_key));
    assert!(!adapter.ingress_deliveries.contains_key(&admission_key));
    assert!(adapter.registry.proposals.is_empty());
    assert_eq!(
        adapter.registry.manifests.values().next(),
        Some(&canonical_manifest)
    );
    assert_eq!(adapter.registry.certificates, retained_certificates);
    assert_eq!(
        adapter.registry.execution_commitments,
        retained_execution_commitments
    );
    assert!(!adapter.fail_closed);
}
#[test]
fn forged_body_receipt_cannot_cross_the_prepare_durability_boundary() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(31);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, proposed_subject))
        .expect("accept proposal")
        .into_effects();
    let (tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let round = manifest.round;
    adapter
        .body_available(tag, manifest)
        .expect("body available");
    let correct = durable_body_receipt(&adapter, round, proposed_subject);
    let forged = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        round,
        subject(32),
        correct.manifest_hash(),
    );
    assert!(matches!(
        adapter.body_stored(tag, round, proposed_subject, &forged),
        Err(AdapterError::DurableBodyMismatch)
    ));
    assert!(matches!(
        adapter
            .body_stored(tag, round, proposed_subject, &correct)
            .expect("the real durable receipt remains usable")
            .effects(),
        [AdapterEffect::ValidateBody { .. }]
    ));
}
#[test]
fn local_proposal_and_prepare_are_each_persisted_before_signing() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let subject = subject(8);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let (durable, validated) =
        validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
    let proposal_tag = adapter.current_tag();
    let sign = adapter
        .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
        .expect("submit local proposal")
        .into_effects();
    let tag = match sign.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(proposal),
            },
        ] => {
            assert!(proposal.signature.is_empty());
            *tag
        }
        effects => panic!("unexpected local proposal effects: {effects:?}"),
    };
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    let effects = adapter
        .signature_completed(tag, vec![0xD1; 96])
        .expect("sign local proposal")
        .into_effects();
    assert!(matches!(
        effects.as_slice(),
        [
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            }),
            AdapterEffect::Sign {
                request: SignRequest::Vote(_),
                ..
            }
        ]
    ));
    assert_eq!(adapter.wal.recovered_records().len(), 2);
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 2);
}
#[test]
fn local_proposal_commitment_conflict_is_transactional() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let proposed_subject = subject(0x7b);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let conflicting = execution_commitment(0x7c);
    assert_ne!(conflicting, validated.execution_commitment());
    adapter
        .registry
        .register_execution_commitment(round, core_subject, conflicting)
        .expect("pre-bind a conflicting authenticated commitment");
    let subjects_before = adapter.registry.subjects.clone();
    let manifests_before = adapter.registry.manifests.clone();
    let commitments_before = adapter.registry.execution_commitments.clone();
    let active_before = adapter.active_subject;
    let reducer_before = adapter.reducer.clone();
    let wal_len_before = adapter.wal.recovered_records().len();
    assert!(matches!(
        adapter.local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated,),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert_eq!(adapter.registry.subjects, subjects_before);
    assert_eq!(adapter.registry.manifests, manifests_before);
    assert_eq!(adapter.registry.execution_commitments, commitments_before);
    assert_eq!(adapter.active_subject, active_before);
    assert_eq!(adapter.reducer, reducer_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_len_before);
}
#[test]
fn wrong_view_local_proposal_completion_preserves_registry_without_becoming_active() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let proposed_subject = subject(0x7d);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let commitment = validated.execution_commitment();
    let current = adapter.current_tag();
    let wrong_view =
        reducer::EventTag::new(current.height(), current.view() + 1, current.generation());
    assert!(adapter.active_subject.is_none());
    let publications_before = adapter.status_publication_attempts;

    let outcome = adapter
        .local_proposal_ready(wrong_view, manifest.clone(), &durable, &validated)
        .expect("wrong-view local completion stutters");

    assert_eq!(
        outcome.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::WrongView)
    );
    assert_eq!(
        adapter.registry.manifests.get(&(round, core_subject)),
        Some(&manifest),
        "the independently durable manifest remains trusted"
    );
    assert_eq!(
        adapter
            .registry
            .execution_commitments
            .get(&(round, core_subject))
            .copied(),
        Some(commitment),
        "the independently fsynced validation commitment remains trusted"
    );
    assert!(
        adapter.active_subject.is_none(),
        "an ignored obsolete completion cannot replace the current active subject"
    );
    assert_eq!(
        adapter.status_publication_attempts,
        publications_before + 1,
        "the stale subject must be rolled back before the only status publication"
    );
}
#[test]
fn rejected_nonleader_local_proposal_completion_restores_the_active_subject() {
    let directory = TempDir::new().expect("temporary directory");
    let context = context();
    let leader = context.leader(0);
    let nonleader = (leader + 1) % u32::try_from(context.roster.len()).expect("small roster");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("nonleader-safety.wal"),
        verified_genesis(context.clone()),
        Some(nonleader),
        reducer::Generation::new(1),
        [0x23; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open nonleader");
    assert!(startup.is_empty());
    let proposal = proposal(&context, leader, subject(0x7e));
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&context, &manifest);
    assert!(adapter.active_subject.is_none());

    assert!(matches!(
        adapter.local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated,),
        Err(AdapterError::Reducer(
            reducer::ReducerError::NotCurrentLeader
        ))
    ));
    assert!(
        adapter.active_subject.is_none(),
        "a transactional reducer rejection cannot install the speculative subject"
    );
}
#[test]
fn post_decision_selected_lifecycles_cannot_reopen_the_reclaimed_owner_epoch() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0x7c);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7c; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the exact durable Decision");
    assert!(matches!(
        decided.effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    let reclaimed_snapshot = std::fs::read(adapter.serviced_candidate_store_path_for_test())
        .expect("read reclaimed owner snapshot");
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision validated body"), 1)
        .expect("bind post-Decision validation lifecycle");
    let applied = adapter
        .local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated)
        .expect("service selected post-Decision validation without a producer owner");
    adapter.clear_selected_producer_lifecycle();
    let apply_tag = match applied.effects() {
        [
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            },
        ] if *subject == decided_subject && certificate == &decision => *tag,
        effects => panic!("unexpected exact Decision application effects: {effects:?}"),
    };
    assert!(applied.producer_handoff().is_none());
    adapter
        .bind_selected_producer_lifecycle(Hash::new(b"post-Decision application"), 2)
        .expect("bind post-Decision application lifecycle");
    let completed = adapter
        .application_completed(apply_tag, decided_subject)
        .expect("service selected post-Decision application completion");
    adapter.clear_selected_producer_lifecycle();
    assert_eq!(completed.disposition(), reducer::StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    assert!(completed.producer_handoff().is_none());
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert!(adapter.serviced_candidates.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert!(adapter.producer_continuations.is_empty());
    assert!(adapter.durable_producer_continuations.is_empty());
    assert!(adapter.restored_dormant_producer_continuations.is_empty());
    assert!(adapter.deferred_producer_continuations.is_empty());
    assert!(adapter.pending_producer_handoffs.is_empty());
    assert_eq!(
        std::fs::read(adapter.serviced_candidate_store_path_for_test())
            .expect("reread reclaimed owner snapshot"),
        reclaimed_snapshot,
        "post-Decision service cannot republish or mutate the reclaimed owner epoch"
    );
}
#[test]
fn exact_local_completion_after_decision_reports_body_validated_progress() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let predecision_a = unowned_body_event(&adapter, 0x79);
    adapter
        .step(predecision_a)
        .expect("service pre-Decision candidate A");
    let predecision_b = unowned_body_event(&adapter, 0x7A);
    adapter
        .step(predecision_b)
        .expect("service pre-Decision candidate B");
    assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
    assert_eq!(adapter.durable_serviced_candidates.len(), 2);
    let decided_subject = subject(0x7d);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7d; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the exact durable Decision");
    assert!(matches!(
        decided.effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
    assert!(adapter.serviced_candidates_decision_reclaimed);
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "durable Decision reclaims the complete candidate-service epoch, including its triggering occurrence"
    );
    let applied = adapter
        .local_proposal_ready(adapter.current_tag(), manifest, &durable, &validated)
        .expect("transfer trusted local validation to the Decision");
    let apply_tag = match applied.effects() {
        [
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            },
        ] if *subject == decided_subject && certificate == &decision => *tag,
        effects => panic!("unexpected exact Decision application effects: {effects:?}"),
    };
    assert!(matches!(
        adapter.status().expect("liveness snapshot").liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            round,
            transition: wire::SumeragiV2ProgressTransition::BodyValidated,
            ..
        }) if round == decision.round
    ));
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "post-Decision application progress cannot resurrect candidate tombstones"
    );
    let completed = adapter
        .application_completed(apply_tag, decided_subject)
        .expect("retire the exact Decision application lifecycle");
    assert_eq!(completed.disposition(), reducer::StepDisposition::Applied);
    assert!(completed.effects().is_empty());
    let expected_retransmit = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
    ));
    for attempt in 0..3 {
        let retransmit = adapter
            .retransmit_elapsed(adapter.current_tag())
            .unwrap_or_else(|error| panic!("post-drain retransmission {attempt}: {error}"));
        assert_eq!(
            retransmit.effects(),
            std::slice::from_ref(&expected_retransmit),
            "a drained exact Decision may retransmit only its exact durable CommitQC control"
        );
    }
    assert!(adapter.deferred_completions.is_empty());
    assert!(adapter.durable_serviced_candidates.is_empty());
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "monotone applied state, not a recycled dormant ordinal or tombstone, suppresses resurrection"
    );
}
#[test]
fn busy_local_completion_during_decision_wal_reaches_apply_once() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0x7e);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7e; 96],
    };
    let context = adapter.wire_context.clone();
    let certificate = adapter
        .registry
        .qc_to_core(&decision, &context)
        .expect("convert exact Decision certificate");
    let decision_tag = adapter.current_tag();
    let pending_decision = adapter
        .reducer
        .step(reducer::Event::QuorumCertificateReceived {
            tag: decision_tag,
            certificate,
        })
        .expect("stage Decision WAL persistence");
    assert!(matches!(
        pending_decision.effects(),
        [reducer::Effect::Persist { .. }]
    ));
    let busy = adapter
        .local_proposal_ready(decision_tag, manifest, &durable, &validated)
        .expect("Busy boundary retains the trusted local completion");
    assert_eq!(
        busy.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(busy.effects().is_empty());
    assert_eq!(adapter.deferred_completions.len(), 1);
    let decision_effects = adapter
        .drive_effects(pending_decision.into_effects())
        .expect("fsync and acknowledge the Decision WAL record");
    assert!(matches!(
        decision_effects.as_slice(),
        [AdapterEffect::FetchBody {
            subject,
            certificate: Some(certificate),
            ..
        }] if *subject == decided_subject && certificate == &decision
    ));
    let completion_effects = adapter
        .drain_deferred()
        .expect("fairly service the Busy-deferred completion");
    assert!(matches!(
        completion_effects.as_slice(),
        [AdapterEffect::Apply {
            subject,
            certificate,
            ..
        }] if *subject == decided_subject && certificate == &decision
    ));
    assert!(adapter.deferred_completions.is_empty());
    assert!(
        adapter
            .drain_deferred()
            .expect("completion cannot be applied twice")
            .is_empty()
    );
}
#[test]
fn busy_deferred_input_blocks_terminal_readiness_until_serviced() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0x7f);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, decided_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x7f; 96],
    };
    let context = adapter.wire_context.clone();
    let certificate = adapter
        .registry
        .qc_to_core(&decision, &context)
        .expect("convert exact Decision certificate");
    let decision_tag = adapter.current_tag();
    let pending_decision = adapter
        .reducer
        .step(reducer::Event::QuorumCertificateReceived {
            tag: decision_tag,
            certificate,
        })
        .expect("stage Decision WAL persistence");
    assert!(matches!(
        pending_decision.effects(),
        [reducer::Effect::Persist { .. }]
    ));
    let busy_completion = adapter
        .local_proposal_ready(decision_tag, manifest.clone(), &durable, &validated)
        .expect("retain the trusted completion across the Busy fence");
    assert_eq!(
        busy_completion.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let terminal_vote = wire::Vote {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: decided_subject,
        execution_commitment: validated.execution_commitment(),
        signer: 3,
        signature: vec![0x80; 96],
    };
    let busy_vote = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(terminal_vote)),
        ))
        .expect("retain authenticated ingress across the Busy fence");
    assert_eq!(
        busy_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_completions.len(), 1);
    assert_eq!(adapter.deferred_inputs.len(), 1);
    let decision_effects = adapter
        .drive_effects(pending_decision.into_effects())
        .expect("fsync and acknowledge the Decision WAL record");
    assert!(matches!(
        decision_effects.as_slice(),
        [AdapterEffect::FetchBody { subject, .. }] if *subject == decided_subject
    ));
    let completion_effects = adapter
        .drain_deferred()
        .expect("service the retained completion first");
    assert!(matches!(
        completion_effects.as_slice(),
        [AdapterEffect::Apply { subject, .. }] if *subject == decided_subject
    ));
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(adapter.deferred_inputs.len(), 1);
    let applied = adapter
        .application_completed(decision_tag, decided_subject)
        .expect("acknowledge exact decision application");
    assert_eq!(applied.disposition(), reducer::StepDisposition::Applied);
    assert!(applied.effects().is_empty());
    assert!(adapter.reducer.ready_to_finish());
    assert!(adapter.deferred_work_is_serviceable());
    assert!(
        !adapter.ready_to_finish(),
        "adapter-owned Busy debt must block terminal height rollover"
    );
    assert!(
        adapter
            .drain_deferred()
            .expect("retire the authenticated terminal vote")
            .is_empty()
    );
    assert!(adapter.deferred_inputs.is_empty());
    assert!(adapter.ready_to_finish());
}
#[test]
fn saturated_normal_lane_suppresses_exact_serviced_local_proposal_retry() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let proposed_subject = subject(0x81);
    let leader = adapter.wire_context.leader(0);
    let proposal = proposal(&adapter.wire_context, leader, proposed_subject);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
        manifest: manifest.clone(),
        durable_receipt: durable.clone(),
        validated_receipt: validated.clone(),
    };
    let proposal_tag = adapter.current_tag();
    let sign = adapter
        .local_proposal_ready(proposal_tag, manifest.clone(), &durable, &validated)
        .expect("persist the local proposal before signing")
        .into_effects();
    assert!(matches!(
        sign.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        },]
    ));
    let deferred_vote = wire::Vote {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0x82),
        execution_commitment: execution_commitment(0x82),
        signer: 0,
        signature: vec![0x82; 96],
    };
    let busy = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(deferred_vote),
        ))
        .expect("defer normal ingress behind the proposal signature");
    assert_eq!(
        busy.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let filler = adapter
        .deferred_inputs
        .front()
        .expect("normal ingress owns one deferred slot")
        .clone();
    assert_eq!(filler.priority, DeferredPriority::Normal);
    let mut saturated_inputs = VecDeque::from([filler.clone()]);
    for _ in 1..MAX_DEFERRED_INPUTS {
        let admission_capability = adapter
            .deferred_admission_ordinals
            .mint(filler.admission_capability.origin)
            .expect("each saturated fixture owns a distinct adapter admission");
        let mut distinct_filler = filler.clone();
        distinct_filler.admission_ordinal = admission_capability.ordinal;
        distinct_filler.admission_capability = admission_capability;
        saturated_inputs.push_back(distinct_filler);
    }
    adapter.deferred_inputs = saturated_inputs;
    let next_ordinal_before_retry = adapter.deferred_admission_ordinals.next_for_test();
    let first_retry = adapter
        .local_proposal_ready(proposal_tag, manifest.clone(), &durable, &validated)
        .expect("exact serviced retry is suppressed before the Busy fence");
    assert_eq!(
        first_retry.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert_eq!(
        adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
        (0, 0),
        "a serviced retry must not mint a deferred completion owner"
    );
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(
        adapter.deferred_admission_ordinals.next_for_test(),
        next_ordinal_before_retry,
        "serviced suppression must not consume an actor ordinal"
    );
    let exact_retry = adapter
        .local_proposal_ready(proposal_tag, manifest, &durable, &validated)
        .expect("a repeated serviced retry remains suppressed");
    assert_eq!(
        exact_retry.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert_eq!(
        adapter.deferred_body_pipeline_completion_ownership(proposal_tag, &evidence),
        (0, 0),
        "a repeated serviced retry cannot create completion ownership"
    );
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(
        adapter.deferred_admission_ordinals.next_for_test(),
        next_ordinal_before_retry,
        "repeated serviced suppression must not consume an actor ordinal"
    );
    assert!(!adapter.fail_closed);
}
#[test]
fn replay_resigns_a_durable_proposal_before_prepare() {
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, _) = open_test_as_leader(&directory).expect("open leader");
        let subject = subject(10);
        let leader = adapter.wire_context.leader(0);
        let proposal = proposal(&adapter.wire_context, leader, subject);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let proposal_tag = adapter.current_tag();
        let sign = adapter
            .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
            .expect("persist proposal intent");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
    }
    let (adapter, startup) = open_test_as_leader(&directory).expect("replay leader");
    assert!(adapter.ingress_ready());
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        }]
    ));
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
}
#[test]
fn proposal_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let proposal_signature = vec![0xD1; 96];
    {
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let proposed_subject = subject(0xA8);
        let proposal = proposal(
            &adapter.wire_context,
            adapter.wire_context.leader(0),
            proposed_subject,
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal fixture")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let sign = adapter
            .local_proposal_ready(
                adapter.current_tag(),
                proposal.manifest,
                &durable,
                &validated,
            )
            .expect("persist proposal intent before signing");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Proposal(_),
                },
            ] => *tag,
            effects => panic!("unexpected proposal sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, proposal_signature.clone())
            .expect("complete proposal signature before simulated control loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Proposal(_),
                ..
            })
        )));
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            } if vote.phase == wire::GlobalPhase::Prepare
        )));
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            retained,
            "a Signed callback is not a durable candidate tombstone"
        );
        // Drop both returned controls: the WAL contains ProposalIntent and
        // PrepareIntent, while neither broadcast reached transport.
    }
    let context = context();
    let leader = context.leader(0);
    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(2),
        [0x22; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover proposal and Prepare intents");
    let proposal_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(_),
            },
        ] => *tag,
        effects => panic!("unexpected recovered proposal frontier: {effects:?}"),
    };
    let retained = recovered.serviced_candidate_count_for_test();
    let replayed = recovered
        .signature_completed(proposal_tag, proposal_signature)
        .expect("new generation accepts the replay-issued proposal callback");
    assert!(replayed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(_),
            ..
        })
    )));
    let prepare_tag = replayed
        .effects()
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.phase == wire::GlobalPhase::Prepare => Some(*tag),
            _ => None,
        })
        .expect("recovered proposal releases its durable Prepare signature");
    assert_eq!(recovered.serviced_candidate_count_for_test(), retained);
    let prepare_signature = vec![0xD2; 96];
    let prepared = recovered
        .signature_completed(prepare_tag, prepare_signature.clone())
        .expect("complete replayed Prepare signature");
    assert!(prepared.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) if vote.phase == wire::GlobalPhase::Prepare
    )));
    assert_eq!(
        recovered
            .signature_completed(prepare_tag, prepare_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}
#[test]
fn vote_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let vote_signature = vec![0xE1; 96];
    let prepared_subject = subject(0xA9);
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let proposer = adapter.status().expect("status").leader;
        let fetch = adapter
            .receive_verified(proposal(&adapter.wire_context, proposer, prepared_subject))
            .expect("accept remote proposal");
        let (tag, manifest) = match fetch.effects() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };
        let round = manifest.round;
        adapter
            .body_available(tag, manifest)
            .expect("make remote body available");
        let receipt = durable_body_receipt(&adapter, round, prepared_subject);
        adapter
            .body_stored(tag, round, prepared_subject, &receipt)
            .expect("acknowledge durable body");
        let validated = ValidatedBodyReceipt::for_test(receipt);
        let sign = settle_ready_validate_succeeded_for_test(
            &mut adapter,
            tag,
            round,
            prepared_subject,
            &validated,
        );
        let sign_tag = match sign.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::Vote(vote),
                },
            ] if vote.phase == wire::GlobalPhase::Prepare => *tag,
            effects => panic!("unexpected Prepare sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, vote_signature.clone())
            .expect("complete Prepare signature before simulated transport loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(vote),
                ..
            }) if vote.phase == wire::GlobalPhase::Prepare
        )));
        assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
    }
    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover durable Prepare intent");
    let sign_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] if vote.phase == wire::GlobalPhase::Prepare && vote.subject == prepared_subject => *tag,
        effects => panic!("unexpected recovered Prepare frontier: {effects:?}"),
    };
    let validation_authority = recovered
        .recovered_validation_authority(&startup)
        .expect("WAL replay mints the exact bounded validation frontier");
    assert_eq!(validation_authority.len(), 1);
    assert!(validation_authority.authorizes(
        wire::ConsensusRound {
            context_id: context().id(),
            height: context().height,
            view: 0,
        },
        prepared_subject,
    ));
    let signed = recovered
        .signature_completed(sign_tag, vote_signature.clone())
        .expect("new generation accepts the replay-issued Prepare callback");
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        }) if vote.phase == wire::GlobalPhase::Prepare
            && vote.subject == prepared_subject
    )));
    assert_eq!(
        recovered
            .signature_completed(sign_tag, vote_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}
#[test]
fn recovered_validation_authority_uses_locked_certificate_round() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposal_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let certificate_round = wire::ConsensusRound {
        view: 2,
        ..proposal_round
    };
    let timeout = |view, marker| wire::TimeoutCertificate {
        round: wire::ConsensusRound {
            view,
            ..proposal_round
        },
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        }],
    };
    let timeout_zero = adapter
        .registry
        .tc_to_core(&timeout(0, 0xA8), &adapter.wire_context)
        .expect("register the view-zero timeout certificate");
    let timeout_one = adapter
        .registry
        .tc_to_core(&timeout(1, 0xA9), &adapter.wire_context)
        .expect("register the view-one timeout certificate");
    let locked_subject = subject(0xAA);
    let wire_prepare = wire::QuorumCertificate {
        round: certificate_round,
        proposal_round: certificate_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xAA),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAA; 96],
    };
    let core_context = adapter.reducer.context().clone();
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the carried durable PrepareQC");
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(3),
        reducer::WalRecord::LockAndCommit {
            vote: reducer::Vote::new_with_proposal_round(
                core_context.id(),
                prepare.round(),
                prepare.proposal_round(),
                reducer::Phase::Commit,
                prepare.subject(),
                local_validator,
            ),
            prepare,
        },
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(2),
        [
            reducer::WalEntry::new(
                reducer::PersistenceId::new(1),
                reducer::WalRecord::InstallTimeout(timeout_zero),
            ),
            reducer::WalEntry::new(
                reducer::PersistenceId::new(2),
                reducer::WalRecord::InstallTimeout(timeout_one),
            ),
            lock_entry,
        ],
    )
    .expect("recover the carried durable lock");
    let authority = adapter
        .recovered_validation_authority(&[])
        .expect("mint the recovered lock frontier");
    assert_eq!(authority.len(), 1);
    assert!(authority.authorizes(certificate_round, locked_subject));
    assert!(!authority.authorizes(proposal_round, locked_subject));
}
#[test]
fn recovered_lockless_highest_prepare_retains_exact_validation_authority() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let prepared_subject = subject(0xAB);
    let wire_prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: prepared_subject,
        execution_commitment: execution_commitment(0xAB),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAB; 96],
    };
    let core_context = adapter.reducer.context().clone();
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the durable highest PrepareQC");
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(2),
        [reducer::WalEntry::new(
            reducer::PersistenceId::new(1),
            reducer::WalRecord::ObservePrepare(prepare),
        )],
    )
    .expect("recover the lockless durable highest PrepareQC");
    assert!(adapter.reducer.durable_state().locked().is_none());
    assert_eq!(
        adapter
            .replayed_highest_prepare_certificate_ref()
            .expect("project the replayed highest Prepare reference"),
        Some(wire_prepare.as_ref())
    );
    let authority = adapter
        .recovered_validation_authority(&[])
        .expect("mint the recovered highest-Prepare frontier");
    assert_eq!(authority.len(), 1);
    assert!(authority.authorizes(round, prepared_subject));

    let (runtime, startup) = super::super::v2_runtime::SerializedV2Runtime::new(
        adapter,
        Vec::new(),
        Instant::now(),
        Duration::from_secs(10),
        super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap the recovered adapter in the serialized runtime");
    assert!(startup.is_empty());
    assert_eq!(
        runtime
            .replayed_highest_prepare_certificate_ref()
            .expect("project the replayed highest Prepare through the runtime"),
        Some(wire_prepare.as_ref())
    );
}
#[test]
fn timeout_signed_callback_is_restart_scoped_before_control_delivery() {
    let directory = TempDir::new().expect("temporary directory");
    let timeout_signature = vec![0xF1; 96];
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let sign = adapter
            .timeout_elapsed(adapter.current_tag())
            .expect("persist Timeout intent");
        let sign_tag = match sign.effects() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(_),
                },
            ] => *tag,
            effects => panic!("unexpected Timeout sign effects: {effects:?}"),
        };
        let retained = adapter.serviced_candidate_count_for_test();
        let signed = adapter
            .signature_completed(sign_tag, timeout_signature.clone())
            .expect("complete Timeout signature before simulated transport loss");
        assert!(signed.effects().iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
                ..
            })
        )));
        assert_eq!(adapter.serviced_candidate_count_for_test(), retained);
    }
    let (mut recovered, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("recover durable Timeout intent");
    let sign_tag = match startup.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected recovered Timeout frontier: {effects:?}"),
    };
    let signed = recovered
        .signature_completed(sign_tag, timeout_signature.clone())
        .expect("new generation accepts the replay-issued Timeout callback");
    assert!(signed.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
            ..
        })
    )));
    assert_eq!(
        recovered
            .signature_completed(sign_tag, timeout_signature)
            .expect("same-episode duplicate is reducer-idempotent")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork)
    );
}
#[test]
fn locally_signed_timeout_quorum_leads_with_enter_view_and_subsumes_vote() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };
    for signer in [1, 2] {
        let retained = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                    round,
                    highest_prepare_qc: None,
                    signer,
                    signature: vec![signer as u8; 96],
                }),
            ))
            .expect("retain a remote TimeoutVote before the local timeout");
        assert_eq!(retained.disposition(), reducer::StepDisposition::Applied);
        assert!(retained.effects().is_empty());
    }
    let timeout = adapter
        .timeout_elapsed(tag)
        .expect("persist the local timeout intent");
    let sign_tag = match timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected local TimeoutVote effects: {effects:?}"),
    };
    let entered = adapter
        .signature_completed(sign_tag, vec![0xF2; 96])
        .expect("the local signature completes the retained timeout quorum")
        .into_effects();
    assert!(
        matches!(
            entered.as_slice(),
            [
                AdapterEffect::EnterView {
                    tag: entered_tag,
                    protected_lock: None,
                    ..
                },
                AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
                    ..
                }),
            ] if entered_tag.view() == round.view + 1
                && certificate.round == round
                && certificate.groups.iter().any(|group| group.signers.contains(&0))
        ),
        "the advancing WAL continuation must lead and its durable TC must subsume the old-view vote: {entered:?}"
    );
}
#[test]
fn locally_signed_timeout_without_quorum_broadcasts_only_the_vote() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: tag.view(),
    };
    let timeout = adapter
        .timeout_elapsed(tag)
        .expect("persist the local timeout intent");
    let sign_tag = match timeout.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected local TimeoutVote effects: {effects:?}"),
    };
    let signed = adapter
        .signature_completed(sign_tag, vec![0xF3; 96])
        .expect("complete the non-quorum local TimeoutVote")
        .into_effects();
    assert!(
        matches!(
            signed.as_slice(),
            [AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::TimeoutVote(vote),
                ..
            })] if vote.round == round && vote.signer == 0
        ),
        "a non-quorum local timeout must emit only its vote broadcast: {signed:?}"
    );
}
#[test]
fn deferred_adapter_replay_with_startup_effects_publishes_no_status() {
    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
        assert!(startup.is_empty());
        let proposal = proposal(
            &adapter.wire_context,
            adapter.wire_context.leader(0),
            subject(10),
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
            unreachable!("proposal helper returns a proposal")
        };
        let (durable, validated) =
            validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
        let proposal_tag = adapter.current_tag();
        let sign = adapter
            .local_proposal_ready(proposal_tag, proposal.manifest, &durable, &validated)
            .expect("persist proposal intent");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            }]
        ));
    }
    crate::sumeragi::status::clear_v2_status();
    let context = context();
    let leader = context.leader(0);
    let (mut adapter, startup) = SumeragiV2Adapter::open_deferred_status(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(1),
        [0x22; 32],
        fingerprints(),
        deferred_admission_ordinals(),
    )
    .expect("replay leader without publishing status");
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        }]
    ));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "nonempty startup work must not publish the prepared successor"
    );
    let prepared = adapter
        .successor_activation_status()
        .expect("prepare reducer-owned activation snapshot");
    assert_eq!(prepared.height, 1);
    assert!(matches!(
        prepared.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            ..
        })
    ));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "snapshot construction must remain separate from publication"
    );
    crate::sumeragi::status::clear_v2_status();
}
#[test]
fn replay_resigns_only_an_acknowledged_intent() {
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, _) = open_test(&directory).expect("open adapter");
        let proposer = adapter.status().expect("status").leader;
        let subject = subject(9);
        let proposal = proposal(&adapter.wire_context, proposer, subject);
        let effects = adapter
            .receive_verified(proposal)
            .expect("accept proposal")
            .into_effects();
        let (tag, manifest) = match effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };
        let round = manifest.round;
        adapter
            .body_available(tag, manifest)
            .expect("body available");
        let receipt = durable_body_receipt(&adapter, round, subject);
        adapter
            .body_stored(tag, round, subject, &receipt)
            .expect("body stored");
        let validated = ValidatedBodyReceipt::for_test(receipt);
        let sign =
            settle_ready_validate_succeeded_for_test(&mut adapter, tag, round, subject, &validated);
        assert!(matches!(sign.as_slice(), [AdapterEffect::Sign { .. }]));
    }
    let (adapter, startup) = open_test(&directory).expect("replay adapter");
    assert!(adapter.ingress_ready());
    assert!(matches!(startup.as_slice(), [AdapterEffect::Sign { .. }]));
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
}
#[cfg(feature = "bls")]
#[test]
fn replayed_decision_key_survives_incomplete_tail_and_rejects_key_drift() {
    let directory = TempDir::new().expect("temporary directory");
    let expected;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"pending Kura block")),
            payload_hash: Hash::new(b"pending exact body"),
        };
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let commitment = execution_commitment(0xD4);
        let mut decision = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS-normal key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        decision.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate fixture CommitQC");
        let record = WalEnvelopeV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            persistence_id: 1,
            record: WalRecordV2::Decision(decision),
        }
        .encode();
        let _append_receipt = adapter
            .wal
            .append(&record)
            .expect("append acknowledged Decision record");
        expected = (round, round, subject, commitment);
    }
    OpenOptions::new()
        .append(true)
        .open(directory.path().join("safety.wal"))
        .expect("open WAL tail")
        .write_all(b"S2FR\x01\x00")
        .expect("model incomplete next frame");
    let (mut adapter, startup) = open_test(&directory).expect("replay durable Decision");
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::FetchBody {
            certificate: Some(_),
            ..
        }]
    ));
    assert_eq!(
        adapter
            .replayed_decision_key()
            .expect("map replayed Decision"),
        Some(expected)
    );
    let (active_round, active_subject) = adapter
        .active_subject
        .expect("durable Decision owns the recovery body pipeline");
    assert_eq!(adapter.registry.round_to_wire(active_round), expected.1);
    assert_eq!(
        adapter
            .registry
            .subject(active_subject)
            .expect("map active decision subject"),
        expected.2
    );
    let status = adapter.status().expect("first decision recovery snapshot");
    assert_eq!(
        status.liveness.work.candidate,
        wire::SumeragiV2LocalWorkStage::Complete
    );
    assert_eq!(
        status.liveness.work.body_recovery,
        wire::SumeragiV2LocalWorkStage::Queued
    );
    assert_eq!(
        status.liveness.work.application,
        wire::SumeragiV2LocalWorkStage::Queued
    );
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
            ..
        })
    ));
    drop(adapter);
    assert!(matches!(
        SumeragiV2Adapter::open_with_aggregator(
            directory.path().join("safety.wal"),
            verified_genesis(context()),
            Some(0),
            reducer::Generation::new(1),
            [0x99; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        ),
        Err(AdapterError::SafetyWal(SafetyWalError::IdentityMismatch {
            field: "consensus key hash",
            ..
        }))
    ));
}
#[cfg(feature = "bls")]
#[test]
fn replay_rejects_checksummed_wal_decision_without_quorum_authority() {
    let directory = TempDir::new().expect("temporary directory");
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let decision = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: subject(0xD5),
            execution_commitment: execution_commitment(0xD5),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD5; 48],
        };
        let record = WalEnvelopeV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            persistence_id: 1,
            record: WalRecordV2::Decision(decision),
        }
        .encode();
        let _append_receipt = adapter
            .wal
            .append(&record)
            .expect("append a fully checksummed but unauthenticated Decision");
    }
    assert!(matches!(
        open_test(&directory),
        Err(AdapterError::Cryptography(_))
    ));
}
#[cfg(feature = "bls")]
#[test]
fn replay_rejects_forged_lock_before_resigning_the_commit_intent() {
    let directory = TempDir::new().expect("temporary directory");
    let wal_path = directory.path().join("forged-lock-safety.wal");
    let (context, _keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let locked_subject = subject(0xDB);
    let commitment = execution_commitment(0xDB);
    let forged_prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xDB; 48],
    };
    let commit_intent = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: locked_subject,
        execution_commitment: commitment,
        signer: 0,
        signature: Vec::new(),
    };
    {
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs.clone())
            .expect("verified genesis context");
        let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            wal_path.clone(),
            verified,
            Some(0),
            reducer::Generation::new(1),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("open adapter");
        assert!(startup.is_empty());
        let record = WalEnvelopeV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            persistence_id: 1,
            record: WalRecordV2::LockAndCommit {
                prepare: forged_prepare,
                vote: commit_intent,
            },
        }
        .encode();
        let _append_receipt = adapter
            .wal
            .append(&record)
            .expect("append checksummed forged lock");
    }
    let verified =
        VerifiedHeightContext::genesis(context, proofs).expect("verified genesis context");
    assert!(matches!(
        SumeragiV2Adapter::open_with_aggregator(
            wal_path,
            verified,
            Some(0),
            reducer::Generation::new(1),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        ),
        Err(AdapterError::Cryptography(_))
    ));
}
#[cfg(feature = "bls")]
#[test]
fn wal_record_authority_rejects_forged_certificates_in_every_record_variant() {
    let (context, _keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let certified_subject = subject(0xD7);
    let commitment = execution_commitment(0xD7);
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: certified_subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xD7; 48],
    };
    let commit = wire::QuorumCertificate {
        phase: wire::GlobalPhase::Commit,
        ..prepare.clone()
    };
    let timeout = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD7; 48],
        }],
    };
    let proposal_round = wire::ConsensusRound { view: 1, ..round };
    let proposal_payload = b"chunk";
    let mut proposal_subject = subject(0xD8);
    proposal_subject.payload_hash = Hash::new(proposal_payload);
    let proposal_chunks = wire::encode_payload_chunks(context.da_layout, proposal_payload)
        .expect("encode canonical fixture chunks");
    let proposal_manifest = wire::PayloadManifest::derive(
        &context,
        proposal_round,
        proposal_subject,
        u64::try_from(proposal_payload.len()).expect("fixture payload length fits u64"),
        &proposal_chunks,
    )
    .expect("valid fixture manifest");
    let proposal = wire::Proposal {
        round: proposal_round,
        proposer: context.leader(proposal_round.view),
        subject: proposal_subject,
        manifest: proposal_manifest,
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: timeout.clone(),
            highest_prepare_qc: None,
        }),
        signature: Vec::new(),
    };
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: certified_subject,
        execution_commitment: commitment,
        signer: 0,
        signature: Vec::new(),
    };
    let timeout_vote = wire::TimeoutVote {
        round,
        highest_prepare_qc: Some(prepare.clone()),
        signer: 0,
        signature: Vec::new(),
    };
    let records = [
        (
            "ProposalIntent timeout",
            WalRecordV2::ProposalIntent(proposal),
        ),
        (
            "ObservePrepare",
            WalRecordV2::ObservePrepare(prepare.clone()),
        ),
        (
            "LockAndCommit",
            WalRecordV2::LockAndCommit {
                prepare: prepare.clone(),
                vote,
            },
        ),
        ("TimeoutIntent", WalRecordV2::TimeoutIntent(timeout_vote)),
        ("InstallTimeout", WalRecordV2::InstallTimeout(timeout)),
        ("Decision", WalRecordV2::Decision(commit)),
    ];
    for (kind, record) in records {
        assert!(
            matches!(
                verify_wal_record_authority(&context, None, &record, &proofs),
                Err(AdapterError::Cryptography(_))
            ),
            "{kind} must reauthenticate every embedded certificate"
        );
    }
    let forged_parent = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: certified_subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xD9; 48],
    };
    let mut successor = context.clone();
    successor.height = context.height + 1;
    successor.parent_commit_qc = Some(forged_parent.clone());
    let successor_round = wire::ConsensusRound {
        context_id: successor.id(),
        height: successor.height,
        view: 0,
    };
    let successor_payload = b"chunk";
    let mut successor_subject = subject(0xD9);
    successor_subject.payload_hash = Hash::new(successor_payload);
    successor_subject.parent_block_hash = Some(certified_subject.block_hash);
    let successor_chunks = wire::encode_payload_chunks(successor.da_layout, successor_payload)
        .expect("encode canonical successor fixture chunks");
    let successor_manifest = wire::PayloadManifest::derive(
        &successor,
        successor_round,
        successor_subject,
        u64::try_from(successor_payload.len()).expect("fixture payload length fits u64"),
        &successor_chunks,
    )
    .expect("valid successor fixture manifest");
    let parent_proposal = wire::Proposal {
        round: successor_round,
        proposer: successor.leader(0),
        subject: successor_subject,
        manifest: successor_manifest,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: Some(forged_parent),
        }),
        signature: Vec::new(),
    };
    let parent_verification = ParentVerificationContext {
        context,
        proofs_of_possession: proofs.clone(),
    };
    assert!(matches!(
        verify_wal_record_authority(
            &successor,
            Some(&parent_verification),
            &WalRecordV2::ProposalIntent(parent_proposal),
            &proofs,
        ),
        Err(AdapterError::Cryptography(_))
    ));
}
#[test]
fn wal_unsigned_intents_reject_ignored_signature_bytes() {
    let context = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let wire::ConsensusMessageV2Payload::Proposal(mut proposal) =
        proposal(&context, context.leader(0), subject(0xDA)).payload
    else {
        unreachable!("proposal fixture")
    };
    proposal.signature = vec![0xDA];
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xDA),
        execution_commitment: execution_commitment(0xDA),
        signer: 0,
        signature: vec![0xDA],
    };
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: vote.subject,
        execution_commitment: vote.execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xDA; 48],
    };
    let timeout_vote = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 0,
        signature: vec![0xDA],
    };
    let records = [
        WalRecordV2::ProposalIntent(proposal),
        WalRecordV2::PrepareIntent(vote.clone()),
        WalRecordV2::LockAndCommit {
            prepare,
            vote: wire::Vote {
                phase: wire::GlobalPhase::Commit,
                ..vote
            },
        },
        WalRecordV2::TimeoutIntent(timeout_vote),
    ];
    for record in records {
        assert!(matches!(
            verify_wal_record_authority(&context, None, &record, &[]),
            Err(AdapterError::WalDecode(_))
        ));
    }
}
#[cfg(feature = "bls")]
#[test]
fn replay_authenticates_the_exact_decision_not_a_same_reference_cache_alias() {
    let directory = TempDir::new().expect("temporary directory");
    let wal_path = directory.path().join("exact-decision-safety.wal");
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let decision_subject = subject(0xD6);
    let commitment = execution_commitment(0xD6);
    let forged = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: decision_subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xD6; 48],
    };
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: decision_subject,
        execution_commitment: commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let valid_signers = [0_usize, 1, 3];
    let valid_shares = valid_signers
        .iter()
        .map(|index| {
            Signature::new(keys[*index].private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let valid = wire::QuorumCertificate {
        signers: valid_signers
            .into_iter()
            .map(|index| u32::try_from(index).expect("small fixture signer index"))
            .collect(),
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
            &valid_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate valid same-reference CommitQC"),
        ..forged.clone()
    };
    verify_quorum_certificate(&context, &valid, &proofs)
        .expect("cache-alias fixture must be cryptographically valid");
    {
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs.clone())
            .expect("verified genesis context");
        let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
            wal_path.clone(),
            verified,
            Some(0),
            reducer::Generation::new(1),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        )
        .expect("open adapter");
        assert!(startup.is_empty());
        for (persistence_id, certificate) in [(1, forged), (2, valid)] {
            let record = WalEnvelopeV2 {
                protocol_version: wire::PROTOCOL_VERSION,
                persistence_id,
                record: WalRecordV2::Decision(certificate),
            }
            .encode();
            let _append_receipt = adapter
                .wal
                .append(&record)
                .expect("append checksummed Decision record");
        }
    }
    let verified =
        VerifiedHeightContext::genesis(context, proofs).expect("verified genesis context");
    assert!(matches!(
        SumeragiV2Adapter::open_with_aggregator(
            wal_path,
            verified,
            Some(0),
            reducer::Generation::new(1),
            [0x22; 32],
            fingerprints(),
            Box::new(TestAggregator),
            deferred_admission_ordinals(),
        ),
        Err(AdapterError::Cryptography(_))
    ));
}
#[test]
fn verified_aggregate_qc_roundtrips_without_reaggregation() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let subject = subject(3);
    let certificate = wire::QuorumCertificate {
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        },
        proposal_round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        },
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(3),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAA; 96],
    };
    let core = registry
        .qc_to_core(&certificate, &context)
        .expect("convert verified QC");
    let roundtrip = registry
        .qc_to_wire(&core, &TestAggregator)
        .expect("convert QC to wire");
    assert_eq!(roundtrip, certificate);
}
#[test]
fn registry_preserves_exact_qc_when_one_reference_has_distinct_signer_quorums() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 2,
    };
    let first = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0x31),
        execution_commitment: execution_commitment(0x31),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xA1; 96],
    };
    let second = wire::QuorumCertificate {
        signers: vec![0, 1, 3],
        aggregate_signature: vec![0xB2; 96],
        ..first.clone()
    };
    let first_core = registry
        .qc_to_core(&first, &context)
        .expect("register first signer quorum");
    let second_core = registry
        .qc_to_core(&second, &context)
        .expect("register second signer quorum for the same reference");
    assert_eq!(
        registry
            .qc_to_wire(&first_core, &TestAggregator)
            .expect("recover first exact certificate"),
        first
    );
    assert_eq!(
        registry
            .qc_to_wire(&second_core, &TestAggregator)
            .expect("recover second exact certificate"),
        second
    );
}
#[test]
fn aggregate_reconstruction_rejects_mixed_or_disagreeing_verified_tokens() {
    let mixed = vec![
        reducer::SignatureShare::new(
            validator_token(0),
            reducer::OpaqueSignature::new(vec![0xA0; 96]),
        ),
        reducer::SignatureShare::new(validator_token(1), aggregate_token(&[0xA1; 96])),
    ];
    assert!(matches!(
        aggregate_core_shares(&mixed, &TestAggregator),
        Err(AdapterError::SignatureAggregation(_))
    ));
    let disagreeing = vec![
        reducer::SignatureShare::new(validator_token(0), aggregate_token(&[0xA2; 96])),
        reducer::SignatureShare::new(validator_token(1), aggregate_token(&[0xA3; 96])),
    ];
    assert!(matches!(
        aggregate_core_shares(&disagreeing, &TestAggregator),
        Err(AdapterError::SignatureAggregation(_))
    ));
}
#[test]
fn registry_rejects_vote_or_qc_execution_commitment_drift_for_one_body() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let subject = subject(0xEC);
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let canonical_commitment = execution_commitment(0xEC);
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: canonical_commitment,
        signer: 0,
        signature: vec![1],
    };
    registry
        .vote_to_core(&vote, &context)
        .expect("first commitment binds body");
    vote.signer = 1;
    vote.execution_commitment = execution_commitment(0xED);
    assert!(matches!(
        registry.vote_to_core(&vote, &context),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(0xED),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![2],
    };
    assert!(matches!(
        registry.qc_to_core(&certificate, &context),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let reproposal_round = wire::ConsensusRound { view: 1, ..round };
    let mut reproposal_certificate = wire::QuorumCertificate {
        round: reproposal_round,
        proposal_round: reproposal_round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: execution_commitment(0xEF),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![3],
    };
    assert!(matches!(
        registry.qc_to_core(&reproposal_certificate, &context),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    reproposal_certificate.execution_commitment = canonical_commitment;
    registry
        .qc_to_core(&reproposal_certificate, &context)
        .expect("an unchanged re-proposal retains the deterministic execution result");
}
#[test]
fn registry_rejects_split_round_vote_and_qc_reference() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let proposal_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let certified_round = wire::ConsensusRound {
        view: 1,
        ..proposal_round
    };
    let subject = subject(0xEE);
    let commitment = execution_commitment(0xEE);
    let vote = wire::Vote {
        round: certified_round,
        proposal_round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: commitment,
        signer: 0,
        signature: vec![1],
    };
    assert!(matches!(
        registry.vote_to_core(&vote, &context),
        Err(AdapterError::WireValidation(
            wire::ValidationError::InvalidProposalRound
        ))
    ));
    let reference = wire::QuorumCertificateRef {
        round: certified_round,
        proposal_round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: commitment,
    };
    assert!(matches!(
        registry.qc_reference_to_core(&reference),
        Err(AdapterError::WireValidation(
            wire::ValidationError::InvalidProposalRound
        ))
    ));
}
#[test]
fn self_contained_grouped_timeout_certificate_roundtrips() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let subject = subject(5);
    let prepare = wire::QuorumCertificate {
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        },
        proposal_round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        },
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(5),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xAB; 96],
    };
    let certificate = wire::TimeoutCertificate {
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 3,
        },
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xBC; 96],
        }],
    };
    let core = registry
        .tc_to_core(&certificate, &context)
        .expect("convert verified TC");
    let roundtrip = registry
        .tc_to_wire(&core, &TestAggregator)
        .expect("convert TC to wire");
    assert_eq!(roundtrip, certificate);
}
#[test]
fn registry_preserves_distinct_timeout_certificates_for_one_round() {
    let context = context();
    let mut registry = WireRegistry::new(&context).expect("registry");
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 3,
    };
    let first = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC1; 96],
        }],
    };
    let second = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 3],
            aggregate_signature: vec![0xC2; 96],
        }],
    };
    let first_core = registry
        .tc_to_core(&first, &context)
        .expect("register first timeout quorum");
    let second_core = registry
        .tc_to_core(&second, &context)
        .expect("register second timeout quorum for the same round");
    assert_eq!(
        registry
            .tc_to_wire(&first_core, &TestAggregator)
            .expect("recover first exact timeout certificate"),
        first
    );
    assert_eq!(
        registry
            .tc_to_wire(&second_core, &TestAggregator)
            .expect("recover second exact timeout certificate"),
        second
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn equivocation_flood_is_bounded_and_cannot_starve_commit_qc() {
    fn flood_subject(counter: u64) -> wire::BlockSubject {
        let mut bytes = [0_u8; 9];
        bytes[..8].copy_from_slice(&counter.to_le_bytes());
        bytes[8] = 0;
        let parent_block_hash = HashOf::from_untyped_unchecked(Hash::new(bytes));
        bytes[8] = 1;
        let block_hash = HashOf::from_untyped_unchecked(Hash::new(bytes));
        bytes[8] = 2;
        wire::BlockSubject {
            parent_block_hash: Some(parent_block_hash),
            block_hash,
            payload_hash: Hash::new(bytes),
        }
    }
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    // Drive the local node to an outstanding Prepare signature. Authenticated
    // network inputs now exercise the adapter's deferred queues.
    let proposer = adapter.status().expect("status").leader;
    let decided_subject = subject(0xD0);
    let proposal = proposal(&adapter.wire_context, proposer, decided_subject);
    let fetch = adapter
        .receive_verified(proposal)
        .expect("accept proposal")
        .into_effects();
    let (tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let round = manifest.round;
    adapter
        .body_available(tag, manifest)
        .expect("body available");
    let receipt = durable_body_receipt(&adapter, round, decided_subject);
    adapter
        .body_stored(tag, round, decided_subject, &receipt)
        .expect("body stored");
    let validated = ValidatedBodyReceipt::for_test(receipt);
    let decided_execution_commitment = validated.execution_commitment();
    let sign = settle_ready_validate_succeeded_for_test(
        &mut adapter,
        tag,
        round,
        decided_subject,
        &validated,
    );
    let _sign_tag = match sign.as_slice() {
        [AdapterEffect::Sign { tag, .. }] => *tag,
        effects => panic!("unexpected validation effects: {effects:?}"),
    };
    let first_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: flood_subject(0),
        execution_commitment: execution_commitment(0x41),
        signer: 1,
        signature: vec![0x41],
    };
    let first = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(first_vote),
        ))
        .expect("defer first vote");
    assert_eq!(
        first.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), 1);
    let mut evidence_reports = 0_usize;
    let flood_size = u64::try_from(MAX_DEFERRED_INPUTS).expect("queue bound fits u64") + 128;
    for counter in 1..=flood_size {
        let vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: flood_subject(counter),
            execution_commitment: execution_commitment(0x42),
            signer: 1,
            signature: vec![0x42],
        };
        let outcome = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(vote),
            ))
            .expect("equivocation admission stays live");
        evidence_reports += outcome
            .effects()
            .iter()
            .filter(|effect| matches!(effect, AdapterEffect::ReportEquivocation { .. }))
            .count();
    }
    assert_eq!(evidence_reports, 1, "evidence is capped per semantic key");
    assert_eq!(adapter.deferred_inputs.len(), 1);
    assert_eq!(adapter.ingress_equivocations.len(), 2);
    assert_eq!(adapter.ingress_deliveries.len(), 2);
    assert!(adapter.registry.subjects.len() <= 2);
    // A valid CommitQC supersedes the outstanding local signer immediately;
    // it must not join ordinary or PrepareQC Busy-deferred ownership.
    let commit_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: decided_subject,
        execution_commitment: decided_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xC0; 96],
    };
    let commit = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(commit_qc),
        ))
        .expect("apply CommitQC through the signature fence");
    assert_eq!(commit.disposition(), reducer::StepDisposition::Applied);
    let decided = commit.into_effects();
    assert!(decided.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Apply { subject, .. } if *subject == decided_subject
    )));
    let decided_subject = adapter
        .registry
        .register_subject(decided_subject)
        .expect("subject");
    assert_eq!(
        adapter
            .reducer
            .durable_state()
            .decision()
            .map(reducer::QuorumCertificate::subject),
        Some(decided_subject)
    );
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert_eq!(adapter.deferred_inputs.len(), 1);
    assert!(
        adapter
            .drain_deferred()
            .expect("service the remaining normal deferred input")
            .is_empty()
    );
    assert!(adapter.deferred_inputs.is_empty());
}
#[test]
#[allow(clippy::too_many_lines)]
fn unsafe_proposal_admission_preserves_duplicate_and_equivocation_semantics() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let locked_subject = subject(0xC4);
    let locked_execution_commitment = execution_commitment(0xC4);
    let core_context = adapter.reducer.context().clone();
    let core_round = reducer::Round::new(core_context.height(), 0);
    let core_subject = adapter
        .registry
        .register_subject(locked_subject)
        .expect("register locked subject");
    adapter
        .registry
        .register_execution_commitment(core_round, core_subject, locked_execution_commitment)
        .expect("register locked execution commitment");
    let shares = (0_u32..3)
        .map(|index| {
            reducer::SignatureShare::new(
                adapter
                    .registry
                    .validator_id(index)
                    .expect("fixture validator"),
                reducer::OpaqueSignature::new(vec![
                    0xC4,
                    u8::try_from(index).expect("small validator index"),
                ]),
            )
        })
        .collect::<Vec<_>>();
    let prepare = reducer::QuorumCertificate::new(
        reducer::CertificateRef::new(
            core_context.id(),
            core_round,
            reducer::Phase::Prepare,
            core_subject,
        ),
        shares,
    );
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    adapter.reducer = reducer::Reducer::recover(
        core_context.clone(),
        Some(local_validator),
        reducer::Generation::new(2),
        [reducer::WalEntry::new(
            reducer::PersistenceId::new(1),
            reducer::WalRecord::LockAndCommit {
                prepare,
                vote: reducer::Vote::new(
                    core_context.id(),
                    core_round,
                    reducer::Phase::Commit,
                    core_subject,
                    local_validator,
                ),
            },
        )],
    )
    .expect("recover durable lock without resuming reducer delivery");
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let proposer = adapter.wire_context.leader(wire_round.view);
    let unsafe_proposal = proposal(&adapter.wire_context, proposer, subject(0xC5));
    let conflicting_proposal = proposal(&adapter.wire_context, proposer, subject(0xC6));
    let safe_subject = adapter
        .registry
        .register_subject(subject(0xC5))
        .expect("register the proposal subject for the upgraded lock");
    adapter
        .registry
        .register_execution_commitment(core_round, safe_subject, execution_commitment(0xC5))
        .expect("register the upgraded lock execution commitment");
    let safe_shares = (0_u32..3)
        .map(|index| {
            reducer::SignatureShare::new(
                adapter
                    .registry
                    .validator_id(index)
                    .expect("fixture validator"),
                reducer::OpaqueSignature::new(vec![
                    0xC5,
                    u8::try_from(index).expect("small validator index"),
                ]),
            )
        })
        .collect::<Vec<_>>();
    let safe_prepare = reducer::QuorumCertificate::new(
        reducer::CertificateRef::new(
            core_context.id(),
            core_round,
            reducer::Phase::Prepare,
            safe_subject,
        ),
        safe_shares,
    );
    let upgraded_reducer = reducer::Reducer::recover(
        core_context.clone(),
        Some(local_validator),
        reducer::Generation::new(3),
        [reducer::WalEntry::new(
            reducer::PersistenceId::new(1),
            reducer::WalRecord::LockAndCommit {
                prepare: safe_prepare,
                vote: reducer::Vote::new(
                    core_context.id(),
                    core_round,
                    reducer::Phase::Commit,
                    safe_subject,
                    local_validator,
                ),
            },
        )],
    )
    .expect("recover the same-view upgraded lock");
    let reducer_before = adapter.reducer.clone();
    let registry_before = (
        adapter.registry.subjects.len(),
        adapter.registry.manifests.len(),
        adapter.registry.execution_commitments.len(),
        adapter.registry.proposals.len(),
    );
    let active_subject_before = adapter.active_subject;
    let first = adapter
        .receive_verified(unsafe_proposal.clone())
        .expect("reject the first unsafe proposal at admission");
    assert_eq!(
        first.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::UnsafeProposal)
    );
    assert!(first.effects().is_empty());
    let retransmit = adapter
        .receive_verified(unsafe_proposal.clone())
        .expect("coalesce the exact unsafe proposal retransmission");
    assert_eq!(
        retransmit.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert!(retransmit.effects().is_empty());
    adapter.reducer = upgraded_reducer;
    let (retry_outcome, retry_admission) = adapter
        .admit_authenticated_payload(&unsafe_proposal.payload)
        .expect("re-evaluate the exact proposal after the lock generation changes");
    assert!(
        retry_outcome.is_none(),
        "a proposal made safe by the upgraded lock must not remain tombstoned"
    );
    assert_eq!(
        retry_admission
            .expect("the proposal owns the upgraded consumer epoch")
            .consumer_tag,
        adapter.current_tag()
    );
    assert_eq!(
        adapter.current_tag().generation(),
        reducer::Generation::new(3)
    );
    adapter.reducer = reducer_before.clone();
    let conflict = adapter
        .receive_verified(conflicting_proposal.clone())
        .expect("report the conflicting proposal fingerprint");
    assert_eq!(conflict.disposition(), reducer::StepDisposition::Applied);
    let wire::ConsensusMessageV2Payload::Proposal(first_proposal) = unsafe_proposal.payload else {
        unreachable!("proposal fixture contains a proposal")
    };
    let wire::ConsensusMessageV2Payload::Proposal(second_proposal) = conflicting_proposal.payload
    else {
        unreachable!("proposal fixture contains a proposal")
    };
    assert_eq!(
        conflict.effects(),
        &[AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::proposal(first_proposal, second_proposal),
        }]
    );
    assert_eq!(
        adapter.reducer, reducer_before,
        "unsafe proposal admission must not reach reducer delivery"
    );
    assert_eq!(
        (
            adapter.registry.subjects.len(),
            adapter.registry.manifests.len(),
            adapter.registry.execution_commitments.len(),
            adapter.registry.proposals.len(),
        ),
        registry_before,
        "unsafe proposal admission must not stage registry conversion"
    );
    assert_eq!(adapter.active_subject, active_subject_before);
    assert!(!adapter.fail_closed);
}
#[test]
fn current_locked_reproposal_prepare_uses_progress_only_after_local_binding() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let locked_subject = subject(0xD3);
    let locked_execution_commitment = execution_commitment(0xD3);
    let core_subject = reducer::Subject::new(Hash::new(locked_subject.encode()).into());
    let core_context = adapter.reducer.context().clone();
    let locked_round = reducer::Round::new(core_context.height(), 0);
    assert_eq!(
        adapter
            .registry
            .register_subject(locked_subject)
            .expect("register locked reproposal subject"),
        core_subject
    );
    adapter
        .registry
        .register_execution_commitment(locked_round, core_subject, locked_execution_commitment)
        .expect("register the historical lock commitment");
    let shares = (0_u32..3)
        .map(|index| {
            reducer::SignatureShare::new(
                adapter
                    .registry
                    .validator_id(index)
                    .expect("fixture validator"),
                reducer::OpaqueSignature::new(vec![
                    0xD3,
                    u8::try_from(index).expect("small fixture validator index"),
                ]),
            )
        })
        .collect::<Vec<_>>();
    let prepare = reducer::QuorumCertificate::new(
        reducer::CertificateRef::new(
            core_context.id(),
            locked_round,
            reducer::Phase::Prepare,
            core_subject,
        ),
        shares.clone(),
    );
    let timeout = reducer::TimeoutCertificate::new(
        core_context.id(),
        locked_round,
        vec![reducer::TimeoutSignatureGroup::new(Some(prepare), shares)],
    );
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    adapter.reducer = reducer::Reducer::recover(
        core_context,
        Some(local_validator),
        reducer::Generation::new(2),
        [reducer::WalEntry::new(
            reducer::PersistenceId::new(1),
            reducer::WalRecord::InstallTimeout(timeout),
        )],
    )
    .expect("recover a TC-promoted lock without a local Commit intent");
    let current_tag = adapter.current_tag();
    assert!(current_tag.view() > locked_round.view());
    let locked = adapter
        .reducer
        .durable_state()
        .locked()
        .expect("the highest PrepareQC becomes the durable lock");
    assert_eq!(locked.round(), locked_round);
    assert!(
        adapter
            .reducer
            .durable_state()
            .commit_intent_for_lock(locked)
            .is_none(),
        "TC promotion alone cannot manufacture a closed-view Commit intent"
    );
    let current_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: current_tag.height(),
        view: current_tag.view(),
    };
    let exact_prepare = wire::Vote {
        round: current_round,
        proposal_round: current_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: locked_execution_commitment,
        signer: 1,
        signature: vec![0xD4],
    };
    assert!(
        !adapter.is_exact_locked_reproposal_prepare_vote(&exact_prepare),
        "remote wire data cannot bootstrap its own local execution binding"
    );
    adapter
        .registry
        .register_execution_commitment(
            reducer::Round::new(current_tag.height(), current_tag.view()),
            core_subject,
            locked_execution_commitment,
        )
        .expect("bind the locally validated current-round reproposal");
    assert!(adapter.is_exact_locked_reproposal_prepare_vote(&exact_prepare));
    let exact_message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(exact_prepare.clone()));
    assert!(adapter.wire_ingress_may_use_progress(&exact_message.payload));
    assert!(
        adapter.authenticated_ingress_is_progress(&AuthenticatedConsensusMessage::for_test(
            exact_message
        ))
    );

    let mut stale = exact_prepare.clone();
    stale.round.view = locked_round.view();
    stale.proposal_round = stale.round;
    assert!(!adapter.is_exact_locked_reproposal_prepare_vote(&stale));
    let mut future = exact_prepare.clone();
    future.round.view = current_tag.view() + 1;
    future.proposal_round = future.round;
    assert!(!adapter.is_exact_locked_reproposal_prepare_vote(&future));
    let mut wrong_subject = exact_prepare.clone();
    wrong_subject.subject = subject(0xD5);
    assert!(!adapter.is_exact_locked_reproposal_prepare_vote(&wrong_subject));
    let mut wrong_commitment = exact_prepare.clone();
    wrong_commitment.execution_commitment = execution_commitment(0xD5);
    assert!(!adapter.is_exact_locked_reproposal_prepare_vote(&wrong_commitment));
    let mut commit = exact_prepare;
    commit.phase = wire::GlobalPhase::Commit;
    assert!(!adapter.is_exact_locked_reproposal_prepare_vote(&commit));
}
#[test]
fn admission_keeps_only_the_exact_locked_commit_vote_beyond_one_rotation() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let locked_subject = subject(0xD4);
    let core_subject = reducer::Subject::new(Hash::new(locked_subject.encode()).into());
    let core_context = adapter.reducer.context().clone();
    let round = reducer::Round::new(core_context.height(), 0);
    assert_eq!(
        adapter
            .registry
            .register_subject(locked_subject)
            .expect("register locked subject"),
        core_subject
    );
    adapter
        .registry
        .register_execution_commitment(round, core_subject, execution_commitment(0xD4))
        .expect("register locked execution commitment");
    let shares = |marker| {
        (0_u32..3)
            .map(|index| {
                reducer::SignatureShare::new(
                    adapter
                        .registry
                        .validator_id(index)
                        .expect("fixture validator"),
                    reducer::OpaqueSignature::new(vec![
                        marker,
                        u8::try_from(index).expect("small fixture validator index"),
                    ]),
                )
            })
            .collect::<Vec<_>>()
    };
    let prepare = reducer::QuorumCertificate::new(
        reducer::CertificateRef::new(
            core_context.id(),
            round,
            reducer::Phase::Prepare,
            core_subject,
        ),
        shares(0xA1),
    );
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let timeout_round = reducer::Round::new(
        core_context.height(),
        u64::try_from(adapter.wire_context.roster.len()).expect("small roster") + 1,
    );
    let timeout = reducer::TimeoutCertificate::new(
        core_context.id(),
        timeout_round,
        vec![reducer::TimeoutSignatureGroup::new(
            Some(prepare.clone()),
            shares(0xA2),
        )],
    );
    adapter.reducer = reducer::Reducer::recover(
        core_context.clone(),
        Some(local_validator),
        reducer::Generation::new(2),
        [
            reducer::WalEntry::new(
                reducer::PersistenceId::new(1),
                reducer::WalRecord::LockAndCommit {
                    prepare,
                    vote: reducer::Vote::new(
                        core_context.id(),
                        round,
                        reducer::Phase::Commit,
                        core_subject,
                        local_validator,
                    ),
                },
            ),
            reducer::WalEntry::new(
                reducer::PersistenceId::new(2),
                reducer::WalRecord::InstallTimeout(timeout),
            ),
        ],
    )
    .expect("recover a lock older than one complete leader rotation");
    let replay_tag = adapter.reducer.current_tag();
    let replay = adapter
        .reducer
        .step(reducer::Event::ResumeAfterReplay { tag: replay_tag })
        .expect("resume the durable Commit intent");
    assert!(matches!(
        replay.effects(),
        [reducer::Effect::Sign {
            message: reducer::SignableMessage::Vote(vote),
            ..
        }] if vote.phase() == reducer::Phase::Commit
    ));
    adapter
        .reducer
        .step(reducer::Event::Signed {
            tag: replay_tag,
            signature: reducer::OpaqueSignature::new(vec![0xB0]),
        })
        .expect("restore the local locked CommitVote");
    assert_eq!(adapter.reducer.volatile_evidence_counts().0, 1);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let locked_commit = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Commit,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xD4),
        signer: 1,
        signature: vec![0xB1],
    });
    let (outcome, admission) = adapter
        .admit_authenticated_payload(&locked_commit)
        .expect("exact locked CommitVote remains admissible");
    assert!(outcome.is_none());
    assert!(admission.is_some());
    let (outcome, admission) = adapter
        .admit_authenticated_payload(&locked_commit)
        .expect("pre-delivery admission does not consume the generation");
    assert!(outcome.is_none());
    assert!(admission.is_some());
    let received = adapter
        .receive_verified(wire::ConsensusMessageV2::new(locked_commit.clone()))
        .expect("locked CommitVote reaches the freshly cleared reducer pool");
    assert_eq!(received.disposition(), reducer::StepDisposition::Applied);
    assert!(received.effects().is_empty());
    assert_eq!(adapter.reducer.volatile_evidence_counts().0, 1);
    let duplicate = adapter
        .receive_verified(wire::ConsensusMessageV2::new(locked_commit.clone()))
        .expect("same-generation duplicate is harmless");
    assert_eq!(
        duplicate.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    let mut quorum_vote = match locked_commit.clone() {
        wire::ConsensusMessageV2Payload::Vote(vote) => vote,
        _ => unreachable!("fixture is a CommitVote"),
    };
    quorum_vote.signer = 2;
    quorum_vote.signature = vec![0xB2];
    let quorum_vote = adapter
        .registry
        .vote_to_core(&quorum_vote, &adapter.wire_context)
        .expect("convert the final locked-round CommitVote");
    let quorum = adapter
        .reducer
        .step(reducer::Event::VoteReceived {
            tag: adapter.reducer.current_tag(),
            vote: quorum_vote,
        })
        .expect("a third locked-round CommitVote rebuilds the cleared quorum");
    assert!(matches!(
        quorum.effects(),
        [reducer::Effect::Persist { entry, .. }]
            if matches!(
                entry.record(),
                reducer::WalRecord::Decision(certificate)
                    if certificate.round() == round
                        && certificate.phase() == reducer::Phase::Commit
                        && certificate.subject() == core_subject
            )
    ));
    for rejected in [
        wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Prepare,
            subject: locked_subject,
            execution_commitment: execution_commitment(0xD4),
            signer: 1,
            signature: vec![0xB2],
        }),
        wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round: wire_round,
            proposal_round: wire_round,
            phase: wire::GlobalPhase::Commit,
            subject: subject(0xD5),
            execution_commitment: execution_commitment(0xD5),
            signer: 1,
            signature: vec![0xB3],
        }),
    ] {
        let (outcome, admission) = adapter
            .admit_authenticated_payload(&rejected)
            .expect("irrelevant historical vote is harmless");
        assert!(matches!(
            outcome.map(|outcome| outcome.disposition()),
            Some(reducer::StepDisposition::Ignored(
                reducer::IgnoreReason::IrrelevantView
            ))
        ));
        assert!(admission.is_none());
    }
}
