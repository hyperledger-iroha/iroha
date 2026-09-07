#[test]
fn direct_certified_body_busy_wait_observes_monotone_reducer_fence() {
    let directory = TempDir::new().expect("temporary direct-fence directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0xA2);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, subject))
        .expect("accept proposal")
        .into_effects();
    let (fetch_tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let timeout_tag = adapter.current_tag();
    let sign = adapter
        .timeout_elapsed(timeout_tag)
        .expect("persist timeout intent")
        .into_effects();
    assert!(matches!(
        sign.as_slice(),
        [AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(_),
        }] if *tag == timeout_tag
    ));
    let blocked_generation = adapter.reducer_fence_generation();
    let DirectCertifiedBodyAvailablePreparation::Blocked(wait) = adapter
        .prepare_direct_certified_body_available(fetch_tag, &manifest)
        .expect("classify persistence/signature-fenced body completion")
    else {
        panic!("active signature work must return an explicit reducer-fence wait")
    };
    assert_eq!(wait.context_id(), manifest.round.context_id);
    assert_eq!(wait.generation(), blocked_generation);
    drop(wait);
    adapter
        .signature_completed(timeout_tag, vec![0xA2; 96])
        .expect("complete exact timeout signature");
    assert!(adapter.reducer_fence_generation() > blocked_generation);
    assert!(matches!(
        adapter
            .prepare_direct_certified_body_available(fetch_tag, &manifest)
            .expect("retry after the observed fence advances"),
        DirectCertifiedBodyAvailablePreparation::Applied(_)
    ));
}
#[test]
fn reducer_fence_generation_reserves_max_for_coordinator_overflow_detection() {
    let directory = TempDir::new().expect("temporary reducer-fence-overflow directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    adapter.reducer_fence_generation = u64::MAX - 1;
    assert!(matches!(
        adapter.timeout_elapsed(adapter.current_tag()),
        Err(AdapterError::ReducerFenceGenerationExhausted)
    ));
    assert_eq!(adapter.reducer_fence_generation, u64::MAX - 1);
    assert!(adapter.fail_closed);
}
#[test]
fn pacemaker_certificate_stays_queued_until_exact_wal_acknowledgement() {
    use super::super::v2_runtime::{
        RuntimeQueueConfig, RuntimeSelectedCandidateOwnership, RuntimeSelectedOwnerKind,
        RuntimeStep, SerializedV2Runtime,
    };
    let directory = TempDir::new().expect("temporary pending-WAL directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let pending = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        })
        .expect("stage one real TimeoutIntent persistence fence");
    assert!(matches!(
        pending.effects(),
        [reducer::Effect::Persist { .. }]
    ));
    assert!(adapter.pacemaker_escape_is_parked());
    assert!(!adapter.signature_fence_is_active());
    let wire_context = adapter.wire_context.clone();
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic pending-WAL key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert!(
        keys.iter()
            .zip(&wire_context.roster)
            .all(|(key, validator)| key.public_key() == validator.validator.public_key())
    );
    let round = wire::ConsensusRound {
        context_id: wire_context.id(),
        height: wire_context.height,
        view: 0,
    };
    let signers = vec![0, 1, 2];
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let certificate = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                    .expect("aggregate pending-WAL timeout certificate"),
            }],
        }),
    );
    let now = Instant::now();
    let (mut runtime, startup) = SerializedV2Runtime::new(
        adapter,
        startup,
        now,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(4, 1, 1),
    )
    .expect("construct runtime across the pending persistence cut");
    assert!(startup.is_empty());
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime while persistence owns dispatch");
    runtime
        .enqueue_network(certificate)
        .expect("admit the authenticated TC behind the WAL fence");
    assert_eq!(runtime.queued_commands(), 1);
    assert!(
        runtime
            .try_step_pacemaker_escape(now)
            .expect("parked pacemaker observation remains valid")
            .is_none(),
        "certified progress cannot cross an unacknowledged safety write"
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(runtime.last_scheduler_ownership().is_none());
    let post_persist = runtime
        .driver_mut_for_test()
        .drive_effects(pending.into_effects())
        .expect("append, fsync, and acknowledge the exact TimeoutIntent");
    assert!(matches!(
        post_persist.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    runtime
        .observe_effects_with_test_ownership(now, &post_persist)
        .expect("retain the signer effect's runtime owner");
    assert!(!runtime.driver().pacemaker_escape_is_parked());
    assert!(runtime.driver().signature_fence_is_active());
    let escaped = runtime
        .try_step_pacemaker_escape(now)
        .expect("post-ack pacemaker selection remains exact")
        .expect("the queued TC advances after its WAL predecessor");
    let RuntimeStep::Advanced(effects) = escaped else {
        panic!("the post-ack TC unexpectedly idled")
    };
    assert!(matches!(
        effects.as_slice(),
        [AdapterEffect::EnterView { tag, .. }] if tag.view() == 1
    ));
    let evidence = runtime
        .take_last_scheduler_ownership()
        .expect("post-ack TC retains one exact scheduler owner");
    assert_eq!(
        evidence.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    assert!(matches!(
        evidence.candidate,
        RuntimeSelectedCandidateOwnership::Exact(_)
    ));
    assert_eq!(evidence.validate_exact(), Ok(()));
    runtime
        .take_effect_ownership(effects.len())
        .expect("consume the post-ack EnterView ownership");
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.driver().fail_closed);
}
#[test]
fn tc_promoted_lock_requires_same_subject_reproposal_before_commit() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let subject = subject(0x97);
    let payload = [0x97, 2];
    let manifest = encode_payload(&adapter.wire_context, round, subject, &payload)
        .expect("encode certified-body payload")
        .manifest()
        .clone();
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let execution_commitment = validated.execution_commitment();
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signers: vec![1, 2, 3],
        aggregate_signature: vec![0x97; 96],
    };
    let timeout_tag = adapter.current_tag();
    let timeout_sign = adapter
        .timeout_elapsed(timeout_tag)
        .expect("persist a local timeout without the remote PrepareQC")
        .into_effects();
    assert!(matches!(
        timeout_sign.as_slice(),
        [AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(vote),
        }] if *tag == timeout_tag && vote.highest_prepare_qc.is_none()
    ));
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    adapter
        .signature_completed(timeout_tag, vec![0xA7; 96])
        .expect("complete the timeout vote before installing the remote TC");
    let timeout = wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare.clone()),
            signers: vec![1, 2, 3],
            aggregate_signature: vec![0xB7; 96],
        }],
    };
    let installed = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("install the TC carrying a PrepareQC missed by this validator")
        .into_effects();
    assert_eq!(adapter.wal.recovered_records().len(), 2);
    assert!(
        installed
            .iter()
            .all(|effect| !matches!(effect, AdapterEffect::Sign { .. })),
        "the TC cannot expose Commit signing before local body validation"
    );
    let fetch_tag = match installed.as_slice() {
        [
            AdapterEffect::EnterView {
                tag: enter_tag,
                protected_lock: Some(protected_lock),
                ..
            },
            AdapterEffect::FetchBody {
                tag,
                round: fetched_round,
                subject: fetched_subject,
                certificate: Some(certificate),
                ..
            },
        ] if enter_tag == tag
            && protected_lock == &prepare
            && *fetched_round == round
            && *fetched_subject == subject
            && certificate.as_ref() == prepare.as_ref() =>
        {
            *tag
        }
        effects => panic!(
            "TC acknowledgement must expose EnterView before its exact body fetch: {effects:?}"
        ),
    };
    assert!(matches!(
        adapter
            .body_available(fetch_tag, manifest)
            .expect("recover the TC-protected body")
            .effects(),
        [AdapterEffect::StoreBody {
            tag,
            round: stored_round,
            subject: stored_subject,
        }] if *tag == fetch_tag
            && *stored_round == round
            && *stored_subject == subject
    ));
    assert!(matches!(
        adapter
            .body_stored(fetch_tag, round, subject, &durable)
            .expect("store the TC-protected body")
            .effects(),
        [AdapterEffect::ValidateBody {
            tag,
            round: validated_round,
            subject: validated_subject,
        }] if *tag == fetch_tag
            && *validated_round == round
            && *validated_subject == subject
    ));
    let validation = settle_ready_validate_succeeded_for_test(
        &mut adapter,
        fetch_tag,
        round,
        subject,
        &validated,
    );
    let current_round = wire::ConsensusRound {
        view: fetch_tag.view(),
        ..round
    };
    assert_eq!(
        current_round.view,
        round.view + 1,
        "the TC installs the successor proposal view"
    );
    assert!(
        validation.is_empty(),
        "validating an old-round lock cannot mint a split-round Commit vote: {validation:?}"
    );
    assert_eq!(
        adapter.wal.recovered_records().len(),
        2,
        "validation must not append LockAndCommit until the immutable body is re-proposed"
    );
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 2);
    let core_current_round = reducer::Round::new(current_round.height, current_round.view);
    assert_eq!(
        adapter
            .reducer
            .durable_state()
            .commit_intent(core_current_round),
        None,
        "only a new same-round PrepareQC may authorize Commit in the successor view"
    );
    let status = adapter.status().expect("protected reproposal status");
    assert!(status.liveness.outbound_intents.iter().all(|intent| {
        !matches!(
            intent.kind,
            wire::SumeragiV2OutboundIntentKind::CommitVote
                | wire::SumeragiV2OutboundIntentKind::CommitQc
        )
    }));
}
#[test]
fn leader_without_owned_candidate_work_reports_missing_proposal_state() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader adapter");
    assert!(startup.is_empty());
    let status = adapter.status().expect("fresh leader status");
    let local = adapter
        .registry
        .validator_index(
            adapter
                .reducer
                .local_validator()
                .expect("fixture has a local validator"),
        )
        .expect("map local validator");
    assert_eq!(status.leader, local, "fixture local validator is leader");
    assert_eq!(
        status.liveness.work.candidate,
        wire::SumeragiV2LocalWorkStage::Idle,
        "leadership alone is not ownership of candidate construction"
    );
    assert_eq!(status.phase, wire::SumeragiV2StatusPhase::AwaitingProposal);
}
#[test]
fn one_round_and_subject_cannot_change_its_registered_manifest() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0x3D);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, subject))
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
    adapter
        .body_available(tag, manifest.clone())
        .expect("register exact manifest");
    let alternate_body = b"other";
    let alternate_chunks =
        wire::encode_payload_chunks(adapter.wire_context.da_layout, alternate_body)
            .expect("encode complete canonical alternate-body chunks");
    // Deliberately bind the complete canonical alternate body to the
    // original subject so this remains a manifest-conflict negative.
    let conflicting = wire::PayloadManifest::derive(
        &adapter.wire_context,
        manifest.round,
        manifest.subject,
        u64::try_from(alternate_body.len()).expect("alternate body length fits u64"),
        &alternate_chunks,
    )
    .expect("structurally valid conflicting manifest");
    assert!(matches!(
        adapter.body_available(tag, conflicting),
        Err(AdapterError::ConflictingManifest)
    ));
}
#[test]
fn authenticated_proposal_cannot_conflict_with_registered_canonical_manifest() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, _) = open_test(&directory).expect("open adapter");
    let context = adapter.wire_context.clone();
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(0x3E);
    let canonical = proposal(&context, proposer, subject);
    let wire::ConsensusMessageV2Payload::Proposal(canonical_proposal) = &canonical.payload else {
        panic!("fixture is a proposal")
    };
    adapter
        .registry
        .manifest_to_core(&canonical_proposal.manifest, &context)
        .expect("register canonical body manifest before proposal arrival");
    let canonical = AuthenticatedConsensusMessage::for_test(canonical);
    adapter
        .ensure_authenticated_manifest_compatible(&canonical)
        .expect("the exact registered manifest remains admissible");
    let mut conflicting = proposal(&context, proposer, subject);
    let wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal) = &mut conflicting.payload
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
    let conflicting = AuthenticatedConsensusMessage::for_test(conflicting);
    assert!(matches!(
        adapter.ensure_authenticated_manifest_compatible(&conflicting),
        Err(AdapterError::ConflictingManifest)
    ));
    assert!(!adapter.fail_closed);
}
include!("v2_adapter_01_replay_and_registry.rs");
include!("v2_adapter_02_view_and_lock_progress.rs");
include!("v2_adapter_03_tc_and_terminal_ingress.rs");
#[test]
fn full_normal_deferred_lane_cannot_drop_absolute_timeout() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    // Leave the reducer waiting for a Prepare signature, then model a
    // saturated untrusted deferred lane. The absolute timeout is delivered
    // while that signature fence is active, exactly where it used to be
    // classified as normal traffic and silently discarded.
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(0xD2);
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
    let receipt = durable_body_receipt(&adapter, round, proposed_subject);
    adapter
        .body_stored(tag, round, proposed_subject, &receipt)
        .expect("body stored");
    let validated = ValidatedBodyReceipt::for_test(receipt);
    let sign = settle_ready_validate_succeeded_for_test(
        &mut adapter,
        tag,
        round,
        proposed_subject,
        &validated,
    );
    let sign_tag = match sign.as_slice() {
        [AdapterEffect::Sign { tag, .. }] => *tag,
        effects => panic!("unexpected validation effects: {effects:?}"),
    };
    let normal_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0xD3),
        execution_commitment: execution_commitment(0xD3),
        signer: 1,
        signature: vec![0xD3],
    };
    let deferred_vote = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(normal_vote.clone()),
        ))
        .expect("defer normal authenticated vote");
    assert_eq!(
        deferred_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    let filler = adapter
        .deferred_inputs
        .front()
        .expect("normal vote is queued")
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
    let mut backpressured_vote = normal_vote;
    backpressured_vote.signer = 2;
    backpressured_vote.signature = vec![0xD4];
    let backpressured_key = IngressSemanticKey::Vote {
        round,
        phase: wire::GlobalPhase::Prepare,
        signer: 2,
    };
    let backpressured = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(backpressured_vote.clone()),
        ))
        .expect("apply normal-lane backpressure");
    assert_eq!(
        backpressured.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(
        adapter
            .ingress_equivocations
            .contains_key(&backpressured_key)
    );
    assert!(
        !adapter.ingress_deliveries.contains_key(&backpressured_key),
        "admission without queue ownership must remain retryable"
    );
    adapter.deferred_inputs.pop_back();
    let retried = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(backpressured_vote),
        ))
        .expect("retry after reserved ownership becomes available");
    assert_eq!(
        retried.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert!(adapter.ingress_deliveries.contains_key(&backpressured_key));
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    // Saturate the ordinary semantic table as well. TimeoutVote owns an
    // independent signer-bounded semantic slot, so it must still reach the
    // protected Busy-deferred partition instead of being rejected before
    // the reducer boundary.
    saturate_ordinary_semantic_history(&mut adapter, round);
    let timeout_vote = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 1,
        signature: vec![0xD5],
    };
    let timeout_key = IngressSemanticKey::TimeoutVote { round, signer: 1 };
    let deferred_timeout_vote = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(timeout_vote),
        ))
        .expect("defer TimeoutVote through its protected class");
    assert_eq!(
        deferred_timeout_vote.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert!(
        adapter
            .ingress_equivocations
            .get(&timeout_key)
            .is_some_and(|record| record.capacity_bypass),
        "current-view TimeoutVote must bypass saturated ordinary semantic capacity"
    );
    assert!(adapter.ingress_deliveries.contains_key(&timeout_key));
    assert!(matches!(
        adapter.deferred_progress_inputs.back(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutVoteReceived { .. },
            priority: DeferredPriority::Progress,
            protected_progress: false,
            ..
        })
    ));
    assert_eq!(
        deferred_progress_class(
            adapter
                .deferred_progress_inputs
                .back()
                .expect("deferred TimeoutVote owns the progress lane")
        ),
        Some(DeferredProgressClass::TimeoutVote)
    );
    let timeout = adapter
        .timeout_elapsed(sign_tag)
        .expect("defer trusted absolute timeout");
    assert_eq!(
        timeout.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
    );
    assert_eq!(adapter.deferred_inputs.len(), MAX_DEFERRED_INPUTS);
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::TimeoutElapsed { .. },
            priority: DeferredPriority::Completion,
            ..
        })
    ));
    let completed = adapter
        .signature_completed(sign_tag, vec![0xD2; 96])
        .expect("complete outstanding Prepare signature")
        .into_effects();
    assert!(completed.iter().all(|effect| !matches!(
        effect,
        AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }
    )));
    let timeout_effects = adapter
        .drain_deferred()
        .expect("service the absolute timeout as one deferred macro-step");
    let timeout_sign_tag = timeout_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            } => Some(*tag),
            _ => None,
        })
        .expect("absolute timeout starts the durable local TimeoutVote signature");
    assert!(adapter.deferred_completions.is_empty());
    assert_eq!(
        adapter.deferred_progress_inputs.len(),
        1,
        "the remote TimeoutVote remains owned while the local TimeoutVote signature fences the reducer"
    );
    adapter
        .signature_completed(timeout_sign_tag, vec![0xD6; 96])
        .expect("complete the local TimeoutVote signature");
    adapter
        .drain_deferred()
        .expect("service protected progress in its own macro-step");
    assert!(adapter.deferred_progress_inputs.is_empty());
}
#[test]
fn failed_ingress_conversion_rolls_back_registry_and_admission() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let proposed_subject = subject(0xE0);
    let valid = proposal(&adapter.wire_context, proposer, proposed_subject);
    let mut malformed = valid.clone();
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut malformed.payload else {
        unreachable!("proposal helper returns a proposal")
    };
    proposal.justification = wire::ProposalJustification::Timeout(wire::TimeoutJustification {
        timeout_certificate: wire::TimeoutCertificate {
            round: proposal.round,
            groups: Vec::new(),
        },
        highest_prepare_qc: None,
    });
    let subject_count = adapter.registry.subjects.len();
    let manifest_count = adapter.registry.manifests.len();
    assert!(adapter.receive_verified(malformed).is_err());
    assert_eq!(adapter.registry.subjects.len(), subject_count);
    assert_eq!(adapter.registry.manifests.len(), manifest_count);
    assert!(adapter.ingress_equivocations.is_empty());
    assert!(adapter.ingress_deliveries.is_empty());
    assert!(adapter.active_subject.is_none());
    // The failed conversion did not poison the semantic key; the valid
    // proposal for the same leader and round is still admitted.
    assert!(matches!(
        adapter
            .receive_verified(valid)
            .expect("valid retry")
            .effects(),
        [AdapterEffect::FetchBody { .. }]
    ));
}
#[cfg(feature = "bls")]
#[test]
fn authentication_rejects_valid_commitment_conflicts_without_mutating_adapter() {
    let directory = TempDir::new().expect("temporary directory");
    let (context, keys, pops) = authenticated_context();
    let verified = VerifiedHeightContext::genesis(context.clone(), pops).expect("verified context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("commitment-auth-safety.wal"),
        verified,
        None,
        reducer::Generation::new(1),
        [0x83; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let locally_validated_subject = subject(0x87);
    let locally_validated_payload = [0x87, 2];
    let locally_validated_manifest = encode_payload(
        &context,
        round,
        locally_validated_subject,
        &locally_validated_payload,
    )
    .expect("encode locally validated payload")
    .manifest()
    .clone();
    let (_, locally_validated_receipt) =
        validated_receipts_for_manifest(&context, &locally_validated_manifest);
    let locally_validated_commitment = locally_validated_receipt.execution_commitment();
    let wrong_unbound_commitment = execution_commitment(0x87);
    assert_ne!(wrong_unbound_commitment, locally_validated_commitment);
    let signed_vote = |execution_commitment| {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: locally_validated_subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(vote.signer).expect("small signer")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        vote
    };
    let wrong_unbound_vote = signed_vote(wrong_unbound_commitment);
    let canonical_unbound_vote = signed_vote(locally_validated_commitment);
    let registry_before_unbound_votes = adapter.registry.clone();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote.clone()),
        )),
        Err(AdapterError::MissingExecutionCommitment)
    ));
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote.clone()),
        )),
        Err(AdapterError::MissingExecutionCommitment)
    ));
    assert_registry_eq(&adapter.registry, &registry_before_unbound_votes);
    assert!(adapter.ingress_equivocations.is_empty());
    assert!(adapter.ingress_deliveries.is_empty());
    assert!(adapter.deferred_completions.is_empty());
    assert!(adapter.deferred_progress_inputs.is_empty());
    assert!(adapter.deferred_inputs.is_empty());
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);
    adapter
        .recover_validated_body(&locally_validated_manifest, &locally_validated_receipt)
        .expect("local deterministic validation establishes canonical commitment authority");
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(wrong_unbound_vote),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_unbound_vote),
        ))
        .expect("the same signed canonical vote is admissible after local validation");
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);
    let bound_subject = subject(0x83);
    let canonical_commitment = execution_commitment(0x83);
    let conflicting_commitment = execution_commitment(0x84);
    let core_subject = adapter
        .registry
        .register_subject(bound_subject)
        .expect("register canonical subject");
    adapter
        .registry
        .register_execution_commitment(
            reducer::Round::new(round.height, round.view),
            core_subject,
            canonical_commitment,
        )
        .expect("bind canonical validated execution result");
    let retained_registry = adapter.registry.clone();
    let retained_equivocations = adapter.ingress_equivocations.clone();
    let retained_deliveries = adapter.ingress_deliveries.clone();
    let retained_queue_lengths = (
        adapter.deferred_completions.len(),
        adapter.deferred_progress_inputs.len(),
        adapter.deferred_inputs.len(),
    );
    let mut conflicting_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: conflicting_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    conflicting_vote.signature = Signature::new(
        keys[usize::try_from(conflicting_vote.signer).expect("small signer")].private_key(),
        &conflicting_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(conflicting_vote.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let mut conflicting_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: conflicting_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut conflicting_qc, &keys);
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(conflicting_qc.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let later_round = wire::ConsensusRound { view: 1, ..round };
    let mut cross_round_conflicting_vote = wire::Vote {
        round: later_round,
        proposal_round: later_round,
        signature: Vec::new(),
        ..conflicting_vote
    };
    cross_round_conflicting_vote.signature = Signature::new(
        keys[usize::try_from(cross_round_conflicting_vote.signer).expect("small signer index")]
            .private_key(),
        &cross_round_conflicting_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    let cross_round_conflicting_payload =
        wire::ConsensusMessageV2Payload::Vote(cross_round_conflicting_vote.clone());
    assert_eq!(
        adapter.wire_ingress_missing_execution_commitment(&cross_round_conflicting_payload),
        None,
        "a same-subject cross-round conflict must drain instead of retaining fair-ingress ownership"
    );
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            cross_round_conflicting_payload,
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let mut cross_round_canonical_vote = wire::Vote {
        execution_commitment: canonical_commitment,
        signature: Vec::new(),
        ..cross_round_conflicting_vote
    };
    cross_round_canonical_vote.signature = Signature::new(
        keys[usize::try_from(cross_round_canonical_vote.signer).expect("small signer index")]
            .private_key(),
        &cross_round_canonical_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    let cross_round_canonical_payload =
        wire::ConsensusMessageV2Payload::Vote(cross_round_canonical_vote);
    assert_eq!(
        adapter.wire_ingress_missing_execution_commitment(&cross_round_canonical_payload),
        Some((later_round, bound_subject)),
        "the same commitment on another round remains unbound until exact-round validation"
    );
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(cross_round_canonical_payload,)),
        Err(AdapterError::MissingExecutionCommitment)
    ));
    let mut cross_round_conflict = wire::QuorumCertificate {
        round: later_round,
        proposal_round: later_round,
        ..conflicting_qc.clone()
    };
    authenticate_qc(&mut cross_round_conflict, &keys);
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_conflict),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let mut cross_round_canonical = wire::QuorumCertificate {
        round: later_round,
        proposal_round: later_round,
        execution_commitment: canonical_commitment,
        ..conflicting_qc.clone()
    };
    authenticate_qc(&mut cross_round_canonical, &keys);
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(cross_round_canonical),
        ))
        .expect("an unchanged re-proposal authenticates the same deterministic execution");
    let timeout_round = wire::ConsensusRound { view: 1, ..round };
    let timeout_preimage = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: Some(conflicting_qc.clone()),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let timeout_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &timeout_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let timeout_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate valid timeout signatures");
    let mut conflicting_timeout_vote = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: Some(conflicting_qc.clone()),
        signer: 0,
        signature: Vec::new(),
    };
    conflicting_timeout_vote.signature = Signature::new(
        keys[usize::try_from(conflicting_timeout_vote.signer).expect("small signer")].private_key(),
        &conflicting_timeout_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(conflicting_timeout_vote),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let conflicting_tc = wire::TimeoutCertificate {
        round: timeout_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(conflicting_qc.clone()),
            signers: vec![0, 1, 2],
            aggregate_signature: timeout_signature,
        }],
    };
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(conflicting_tc.clone()),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let proposal_round = wire::ConsensusRound { view: 2, ..round };
    let proposal_subject = bound_subject;
    let proposal_body = vec![0x83, 2];
    let proposal_manifest =
        encode_payload(&context, proposal_round, proposal_subject, &proposal_body)
            .expect("encode later-view proposal payload")
            .manifest()
            .clone();
    let proposer = context.leader(proposal_round.view);
    let mut conflicting_proposal = wire::Proposal {
        round: proposal_round,
        proposer,
        subject: proposal_subject,
        manifest: proposal_manifest,
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: conflicting_tc,
            highest_prepare_qc: Some(conflicting_qc.clone()),
        }),
        signature: Vec::new(),
    };
    conflicting_proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
        &conflicting_proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let conflicting_proposal_message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(conflicting_proposal),
    );
    // Exercise the read-only embedded-certificate compatibility walk
    // directly, then confirm ordinary ingress rejects the same
    // structurally valid proposal for its conflicting deterministic
    // execution result.
    let authenticated_conflicting_proposal =
        AuthenticatedConsensusMessage::for_test(conflicting_proposal_message.clone());
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(
            &authenticated_conflicting_proposal,
        ),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert!(matches!(
        adapter.authenticate(conflicting_proposal_message),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let unbound_subject = subject(0x85);
    let mut unbound_qc_a = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: unbound_subject,
        execution_commitment: execution_commitment(0x85),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut unbound_qc_a, &keys);
    let mut unbound_qc_b = wire::QuorumCertificate {
        round: timeout_round,
        proposal_round: timeout_round,
        execution_commitment: execution_commitment(0x86),
        ..unbound_qc_a.clone()
    };
    authenticate_qc(&mut unbound_qc_b, &keys);
    let timeout_group = |highest_prepare_qc: wire::QuorumCertificate,
                         signers: Vec<wire::ValidatorIndex>| {
        let preimage = wire::TimeoutVote {
            round: timeout_round,
            highest_prepare_qc: Some(highest_prepare_qc.clone()),
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(highest_prepare_qc),
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate valid disjoint timeout group"),
        }
    };
    let mut conflicting_groups = vec![
        timeout_group(unbound_qc_a, vec![0, 1]),
        timeout_group(unbound_qc_b, vec![2]),
    ];
    conflicting_groups.sort_by_key(|group| {
        group
            .highest_prepare_qc
            .as_ref()
            .map(wire::QuorumCertificate::as_ref)
    });
    let within_envelope_conflict = wire::TimeoutCertificate {
        round: timeout_round,
        groups: conflicting_groups,
    };
    assert!(matches!(
        adapter.authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(within_envelope_conflict),
        )),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert!(
        !adapter
            .registry
            .execution_commitments
            .keys()
            .any(|(_, registered_subject)| *registered_subject
                == reducer::Subject::new(Hash::new(unbound_subject.encode()).into())),
        "within-envelope checking cannot bind either attacker commitment"
    );
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);
    // Transport adapters authenticate their outer request/response
    // identities separately. The same read-only compatibility walk still
    // covers every embedded certificate before a transport payload is
    // unwrapped into reducer ingress.
    let certified_request = AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
            round,
            subject: bound_subject,
            certificate: conflicting_qc.clone(),
            requester: context.roster[0].validator.clone(),
            signature: vec![0x83; 96],
        }),
    ));
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(&certified_request),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    let commit_response = AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"commitment-conflict-request",
                )),
                certificate: wire::QuorumCertificate {
                    phase: wire::GlobalPhase::Commit,
                    ..conflicting_qc
                },
                responder: context.roster[1].validator.clone(),
                signature: vec![0x84; 96],
            },
        ),
    ));
    assert!(matches!(
        adapter.ensure_authenticated_execution_commitments_compatible(&commit_response),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert_registry_eq(&adapter.registry, &retained_registry);
    assert_eq!(adapter.ingress_equivocations, retained_equivocations);
    assert_eq!(adapter.ingress_deliveries, retained_deliveries);
    assert_eq!(
        (
            adapter.deferred_completions.len(),
            adapter.deferred_progress_inputs.len(),
            adapter.deferred_inputs.len(),
        ),
        retained_queue_lengths
    );
    assert!(adapter.ingress_ready());
    assert!(!adapter.fail_closed);
    let mut canonical_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: bound_subject,
        execution_commitment: canonical_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    canonical_vote.signature = Signature::new(
        keys[usize::try_from(canonical_vote.signer).expect("small signer")].private_key(),
        &canonical_vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Vote(canonical_vote),
        ))
        .expect("the exact canonical commitment remains authentically admissible");
    assert!(adapter.ingress_ready());
}
#[cfg(feature = "bls")]
#[test]
fn authenticated_ingress_verifies_individual_and_aggregate_bls() {
    let (context, keys, pops) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = subject(12);
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signer: 0,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(keys[0].private_key(), &vote.signature_preimage())
        .payload()
        .to_vec();
    verify_authenticated_message(
        &context,
        None,
        &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
        &pops,
    )
    .expect("verify individual vote");
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
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
    let refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(12),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&refs)
            .expect("aggregate BLS votes"),
    };
    verify_authenticated_message(
        &context,
        None,
        &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            certificate,
        )),
        &pops,
    )
    .expect("verify aggregate QC");
}
#[cfg(feature = "bls")]
mod kagemusha_finality_boundary {
    //! Verify the canonical paired seal and its enclosing BLS-authenticated message together.

    use super::*;
    use crate::zk::kagemusha_v1_recursion as mint;
    use iroha_data_model::isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalitySealBundleV1,
        KagemushaMintFinalitySealMessageV1, KagemushaMintFinalitySealShareV1,
        KagemushaMintFinalityValidatorSealV1, KagemushaTopUpLeafV1,
        kagemusha_mint_finality_root_v1,
    };

    struct Fixture {
        context: wire::HeightContext,
        keys: Vec<KeyPair>,
        pops: Vec<Vec<u8>>,
        vote: wire::Vote,
        share: KagemushaMintFinalitySealShareV1,
        certificate: wire::QuorumCertificate,
        bundle: KagemushaMintFinalitySealBundleV1,
    }

    impl Fixture {
        fn new() -> Self {
            let (mut context, keys, pops) = authenticated_context();
            context.epoch_end_height = context.height;
            let next_roster = crate::kagemusha_v1_test_fixtures::mint_finality_roster(
                context.network_id,
                context.epoch + 1,
                &context.roster,
            );
            context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
                epoch: context.epoch + 1,
                kagemusha_mint_finality_epoch_id: next_roster
                    .finality_epoch_id()
                    .expect("next Pasta epoch identity"),
                kagemusha_mint_finality_epoch_roster: next_roster,
                epoch_end_height: context.height + 100,
                mode: context.mode,
                roster: context.roster.clone(),
                validator_set_pops: pops.clone(),
                quorum: context.quorum,
                leader_seed: [0xCA; 32],
            });
            VerifiedHeightContext::genesis(context.clone(), pops.clone())
                .expect("real four-validator context and both epoch rosters");
            let tree = mint::KagemushaMintFinalityTreeV1::new(vec![KagemushaTopUpLeafV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                operation_id: [0x31; 32],
                reserve_receipt_digest: [0x32; 32],
                statement_digest: [0x33; 32],
                amount: 7,
            }])
            .expect("canonical non-empty top-up tree");
            let mut commitment = execution_commitment(0xC1);
            commitment.kagemusha_top_up_root = Some(kagemusha_mint_finality_root_v1(tree.root()));
            commitment.kagemusha_top_up_count = tree.leaf_count();
            rebind_post_state_root(&mut commitment);
            commitment.validate().expect("canonical top-up commitment");
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 3,
            };
            let vote = wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject: subject(0xC1),
                execution_commitment: commitment,
                signer: 0,
                signature: Vec::new(),
            };
            let message = mint::build_kagemusha_mint_finality_seal_message_v1(
                &context.kagemusha_mint_finality_epoch_roster,
                &context,
                &vote,
            )
            .expect("build exact top-up and epoch-boundary statement")
            .expect("top-up Commit requires a seal");
            let seals = (0..3)
                .map(|index| sign_seal(&context, index, &message))
                .collect::<Vec<_>>();
            let share = KagemushaMintFinalitySealShareV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1,
                message: message.clone(),
                seal: seals[0],
            };
            let bundle = KagemushaMintFinalitySealBundleV1 { message, seals };
            let certificate = certificate_for_vote(&vote);
            let mut fixture = Self {
                context,
                keys,
                pops,
                vote,
                share,
                certificate,
                bundle,
            };
            fixture.vote = fixture.signed_vote(fixture.vote.clone(), &fixture.share);
            fixture.certificate =
                fixture.signed_certificate(fixture.certificate.clone(), &fixture.bundle);
            fixture
        }

        fn signed_vote(
            &self,
            mut vote: wire::Vote,
            share: &KagemushaMintFinalitySealShareV1,
        ) -> wire::Vote {
            let index = usize::try_from(vote.signer).expect("fixture signer fits usize");
            let bls = Signature::new(self.keys[index].private_key(), &vote.signature_preimage());
            vote.signature = wire::encode_kagemusha_consensus_signature_envelope_v1(
                wire::KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1,
                bls.payload(),
                &share.encode(),
            )
            .expect("canonical share envelope");
            vote
        }

        fn signed_certificate(
            &self,
            mut certificate: wire::QuorumCertificate,
            bundle: &KagemushaMintFinalitySealBundleV1,
        ) -> wire::QuorumCertificate {
            let preimage = wire::Vote {
                round: certificate.round,
                proposal_round: certificate.proposal_round,
                phase: certificate.phase,
                subject: certificate.subject,
                execution_commitment: certificate.execution_commitment,
                signer: certificate.signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let signatures = certificate
                .signers
                .iter()
                .map(|index| {
                    Signature::new(
                        self.keys[usize::try_from(*index).expect("fixture index")].private_key(),
                        &preimage,
                    )
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            let aggregate = iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("aggregate exact fixture signer set");
            certificate.aggregate_signature =
                wire::encode_kagemusha_consensus_signature_envelope_v1(
                    wire::KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1,
                    &aggregate,
                    &bundle.encode(),
                )
                .expect("canonical bundle envelope");
            certificate
        }

        fn verify_vote(&self, vote: &wire::Vote) -> Result<(), AdapterError> {
            verify_authenticated_message(
                &self.context,
                None,
                &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote.clone())),
                &self.pops,
            )
        }

        fn verify_certificate(
            &self,
            certificate: &wire::QuorumCertificate,
        ) -> Result<(), AdapterError> {
            verify_authenticated_message(
                &self.context,
                None,
                &wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    certificate.clone(),
                )),
                &self.pops,
            )
        }

        fn verify_share(
            &self,
            vote: &wire::Vote,
            share: &KagemushaMintFinalitySealShareV1,
        ) -> Result<(), mint::KagemushaMintFinalityErrorV1> {
            mint::verify_kagemusha_mint_finality_seal_share_v1(
                &self.context.kagemusha_mint_finality_epoch_roster,
                &self.context,
                vote,
                share,
            )
        }

        fn verify_bundle(
            &self,
            certificate: &wire::QuorumCertificate,
            bundle: &KagemushaMintFinalitySealBundleV1,
        ) -> Result<(), mint::KagemushaMintFinalityErrorV1> {
            mint::verify_kagemusha_mint_finality_seal_bundle_v1(
                &self.context.kagemusha_mint_finality_epoch_roster,
                &self.context,
                certificate,
                bundle,
            )
        }
    }

    fn sign_seal(
        context: &wire::HeightContext,
        index: u32,
        message: &KagemushaMintFinalitySealMessageV1,
    ) -> KagemushaMintFinalityValidatorSealV1 {
        let signer = mint::KagemushaMintFinalitySignerV1::from_seed(
            zeroize::Zeroizing::new([0xA0 + u8::try_from(index).expect("small index"); 32]),
            index,
            &context.kagemusha_mint_finality_epoch_roster,
        )
        .expect("independently provisioned fixture Pasta signer");
        mint::sign_kagemusha_mint_finality_seal_v1(&signer, message)
            .expect("paired Pasta signatures")
    }

    fn certificate_for_vote(vote: &wire::Vote) -> wire::QuorumCertificate {
        wire::QuorumCertificate {
            round: vote.round,
            proposal_round: vote.proposal_round,
            phase: vote.phase,
            subject: vote.subject,
            execution_commitment: vote.execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        }
    }

    fn rebind_post_state_root(commitment: &mut wire::ExecutionCommitment) {
        commitment.post_state_root = wire::ExecutionCommitment::kagemusha_post_state_root_v1(
            commitment.kagemusha_top_up_count,
            commitment.ordinary_writes_root,
            commitment
                .kagemusha_top_up_root
                .expect("non-empty top-up root"),
        );
    }

    fn zero_top_up_boundary(fixture: &Fixture) -> (wire::Vote, KagemushaMintFinalitySealMessageV1) {
        let mut vote = fixture.vote.clone();
        vote.execution_commitment = execution_commitment(0xC2);
        let message = mint::build_kagemusha_mint_finality_seal_message_v1(
            &fixture.context.kagemusha_mint_finality_epoch_roster,
            &fixture.context,
            &vote,
        )
        .expect("build zero-top-up epoch rotation")
        .expect("boundary still requires a seal");
        assert_eq!(message.kagemusha_top_up_count, 0);
        assert_eq!(
            message.next_finality_epoch_id,
            fixture.share.message.next_finality_epoch_id
        );
        assert!(message.next_finality_epoch_id.is_some());
        (vote, message)
    }

    fn enclosing_vote_substitutions(vote: &wire::Vote) -> Vec<(&'static str, wire::Vote)> {
        let changes: [(&str, fn(&mut wire::Vote)); 14] = [
            ("older proposal view", |v| v.proposal_round.view -= 1),
            ("newer proposal view", |v| v.proposal_round.view += 1),
            ("foreign proposal context", |v| {
                v.proposal_round.context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(
                    Hash::new(b"foreign proposal context"),
                ))
            }),
            ("foreign proposal height", |v| v.proposal_round.height += 1),
            ("coherent foreign context", |v| {
                v.round.context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(
                    Hash::new(b"foreign context"),
                ));
                v.proposal_round = v.round;
            }),
            ("coherent foreign height", |v| {
                v.round.height += 1;
                v.proposal_round = v.round;
            }),
            ("parent subject", |v| v.subject.parent_block_hash = None),
            ("block subject", |v| {
                v.subject.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"other block"))
            }),
            ("payload subject", |v| {
                v.subject.payload_hash = Hash::new(b"other payload")
            }),
            ("execution parent root", |v| {
                v.execution_commitment.parent_state_root = Hash::new(b"other parent state")
            }),
            ("execution writes root", |v| {
                v.execution_commitment.ordinary_writes_root = Hash::new(b"other writes");
                rebind_post_state_root(&mut v.execution_commitment);
            }),
            ("execution wire hash", |v| {
                v.execution_commitment.executed_block_wire_hash = Hash::new(b"other executed bytes")
            }),
            ("top-up root", |v| {
                v.execution_commitment.kagemusha_top_up_root =
                    Some(Hash::new(b"other top-up root"));
                rebind_post_state_root(&mut v.execution_commitment);
            }),
            ("top-up count", |v| {
                v.execution_commitment.kagemusha_top_up_count += 1;
                rebind_post_state_root(&mut v.execution_commitment);
            }),
        ];
        changes
            .into_iter()
            .map(|(label, change)| {
                let mut changed = vote.clone();
                change(&mut changed);
                changed
                    .execution_commitment
                    .validate()
                    .expect("negative keeps a valid execution commitment");
                (label, changed)
            })
            .collect()
    }

    fn statement_substitutions(
        message: &KagemushaMintFinalitySealMessageV1,
    ) -> Vec<(&'static str, KagemushaMintFinalitySealMessageV1)> {
        let changes: [(&str, fn(&mut KagemushaMintFinalitySealMessageV1)); 11] = [
            ("finality epoch", |m| m.finality_epoch_id[0] ^= 1),
            ("validator count", |m| m.validator_count = 7),
            ("network", |m| m.network_id = test_network_id(0xD3)),
            ("height", |m| m.block_height += 1),
            ("context", |m| {
                m.height_context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(
                    Hash::new(b"other seal context"),
                ))
            }),
            ("subject digest", |m| m.subject_digest[0] ^= 1),
            ("execution digest", |m| {
                m.execution_commitment_digest[0] ^= 1
            }),
            ("top-up root", |m| {
                m.kagemusha_top_up_root = Hash::new(b"other seal root")
            }),
            ("top-up count", |m| m.kagemusha_top_up_count += 1),
            ("next epoch", |m| {
                m.next_finality_epoch_id = Some([0xD4; 32])
            }),
            ("missing next epoch", |m| m.next_finality_epoch_id = None),
        ];
        changes
            .into_iter()
            .map(|(label, change)| {
                let mut changed = message.clone();
                change(&mut changed);
                changed
                    .validate()
                    .expect("negative keeps a structurally valid statement");
                (label, changed)
            })
            .collect()
    }

    #[test]
    fn commit_vote_binds_round_statement_signer_and_both_signatures() {
        let fixture = Fixture::new();
        fixture
            .verify_share(&fixture.vote, &fixture.share)
            .expect("real paired share verifies");
        fixture
            .verify_vote(&fixture.vote)
            .expect("BLS and both Pasta signatures verify at ingress");
        let decoded = mint::decode_kagemusha_mint_finality_seal_share_v1(
            fixture
                .vote
                .kagemusha_finality_seal_payload()
                .expect("share envelope")
                .expect("mandatory share"),
        )
        .expect("decode exact canonical share");
        assert_eq!(decoded, fixture.share);

        for (label, vote) in enclosing_vote_substitutions(&fixture.vote) {
            let vote = fixture.signed_vote(vote, &fixture.share);
            assert!(
                matches!(
                    fixture.verify_share(&vote, &fixture.share),
                    Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
                ),
                "{label}"
            );
            assert!(
                fixture.verify_vote(&vote).is_err(),
                "fresh BLS must not authorize a substituted {label}"
            );
        }
        for (label, message) in statement_substitutions(&fixture.share.message) {
            let mut share = fixture.share.clone();
            share.message = message;
            assert!(
                matches!(
                    fixture.verify_share(&fixture.vote, &share),
                    Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
                ),
                "{label} must fail statement equality before signature verification"
            );
            let vote = fixture.signed_vote(fixture.vote.clone(), &share);
            assert!(
                fixture.verify_vote(&vote).is_err(),
                "BLS-authenticated vote rejects a substituted seal {label}"
            );
        }
        let mut signed_other_statement = fixture.share.clone();
        signed_other_statement.message.subject_digest[0] ^= 1;
        signed_other_statement.seal =
            sign_seal(&fixture.context, 0, &signed_other_statement.message);
        assert!(matches!(
            fixture.verify_share(&fixture.vote, &signed_other_statement),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(
            fixture
                .verify_vote(&fixture.signed_vote(fixture.vote.clone(), &signed_other_statement))
                .is_err(),
            "valid signatures on a different statement do not authorize this vote"
        );

        let mut other_signer = fixture.share.clone();
        other_signer.seal = sign_seal(&fixture.context, 1, &other_signer.message);
        assert!(matches!(
            fixture.verify_share(&fixture.vote, &other_signer),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(
            fixture
                .verify_vote(&fixture.signed_vote(fixture.vote.clone(), &other_signer))
                .is_err()
        );
        let mut vote_by_other_signer = fixture.vote.clone();
        vote_by_other_signer.signer = 1;
        let vote_by_other_signer = fixture.signed_vote(vote_by_other_signer, &fixture.share);
        assert!(matches!(
            fixture.verify_share(&vote_by_other_signer, &fixture.share),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(fixture.verify_vote(&vote_by_other_signer).is_err());
        fixture
            .verify_vote(&fixture.signed_vote(vote_by_other_signer, &other_signer))
            .expect(
                "coherent signer replacement with that validator's BLS and Pasta keys is valid",
            );

        for eq in [true, false] {
            let mut corrupt = fixture.share.clone();
            if eq {
                corrupt.seal.eq_proof_signature = signed_other_statement.seal.eq_proof_signature;
            } else {
                corrupt.seal.ep_proof_signature = signed_other_statement.seal.ep_proof_signature;
            }
            corrupt
                .validate()
                .expect("signature corruption preserves canonical point/scalar encoding");
            let error = fixture
                .verify_share(&fixture.vote, &corrupt)
                .expect_err("each Pasta equation is mandatory");
            assert!(matches!(
                error,
                mint::KagemushaMintFinalityErrorV1::InvalidSignature(_)
            ));
            assert!(error.to_string().contains("Schnorr equation failed"));
            let vote = fixture.signed_vote(fixture.vote.clone(), &corrupt);
            verify_individual_signature(
                &fixture.context,
                vote.signer,
                vote.bls_signature().expect("valid framing"),
                &vote.signature_preimage(),
            )
            .expect("ordinary BLS remains independently valid");
            assert!(
                fixture.verify_vote(&vote).is_err(),
                "ingress must execute both Pasta verifiers; Eq mutation = {eq}"
            );
        }
        let bls = fixture.vote.bls_signature().expect("valid BLS framing");
        let mut missing = fixture.vote.clone();
        missing.signature = bls.to_vec();
        assert!(missing.kagemusha_finality_seal_payload().is_err());
        assert!(fixture.verify_vote(&missing).is_err());
        let mut wrong_kind = fixture.vote.clone();
        wrong_kind.signature = wire::encode_kagemusha_consensus_signature_envelope_v1(
            wire::KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1,
            bls,
            &fixture.share.encode(),
        )
        .expect("well-framed wrong envelope kind");
        assert!(wrong_kind.kagemusha_finality_seal_payload().is_err());
        assert!(fixture.verify_vote(&wrong_kind).is_err());
        let mut bad_bls = fixture.vote.clone();
        let wrong_bls = Signature::new(fixture.keys[0].private_key(), b"another BLS statement");
        bad_bls.signature = wire::encode_kagemusha_consensus_signature_envelope_v1(
            wire::KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1,
            wrong_bls.payload(),
            &fixture.share.encode(),
        )
        .expect("valid envelope with wrong BLS signature");
        fixture
            .verify_share(&bad_bls, &fixture.share)
            .expect("Pasta authority remains independently valid");
        assert!(
            fixture.verify_vote(&bad_bls).is_err(),
            "Pasta signatures cannot replace ordinary BLS verification"
        );

        let mut out_of_range = fixture.vote.clone();
        out_of_range.signer = 4;
        assert!(matches!(
            fixture.verify_share(&out_of_range, &fixture.share),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(fixture.verify_vote(&out_of_range).is_err());
        let mut prepare = fixture.vote.clone();
        prepare.phase = wire::GlobalPhase::Prepare;
        let prepare = fixture.signed_vote(prepare, &fixture.share);
        assert!(matches!(
            fixture.verify_share(&prepare, &fixture.share),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(fixture.verify_vote(&prepare).is_err());

        // A zero-top-up epoch boundary requires a seal in the production verifier even
        // though its ordinary wire framing alone permits a raw BLS signature.
        let (boundary_vote, boundary_message) = zero_top_up_boundary(&fixture);
        let boundary_share = KagemushaMintFinalitySealShareV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            seal: sign_seal(&fixture.context, 0, &boundary_message),
            message: boundary_message,
        };
        let mut boundary_vote = fixture.signed_vote(boundary_vote, &boundary_share);
        fixture
            .verify_share(&boundary_vote, &boundary_share)
            .expect("zero-top-up paired share");
        fixture
            .verify_vote(&boundary_vote)
            .expect("sealed zero-top-up boundary vote");
        boundary_vote.signature = boundary_vote
            .bls_signature()
            .expect("boundary BLS")
            .to_vec();
        assert_eq!(boundary_vote.kagemusha_finality_seal_payload(), Ok(None));
        verify_individual_signature(
            &fixture.context,
            boundary_vote.signer,
            &boundary_vote.signature,
            &boundary_vote.signature_preimage(),
        )
        .expect("unsealed boundary BLS is still valid");
        assert!(
            fixture.verify_vote(&boundary_vote).is_err(),
            "epoch rotation cannot omit its Pasta share"
        );
    }

    #[test]
    fn commit_qc_binds_round_statement_exact_quorum_and_both_signatures() {
        let fixture = Fixture::new();
        fixture
            .verify_bundle(&fixture.certificate, &fixture.bundle)
            .expect("exact three-of-four paired seals verify");
        fixture
            .verify_certificate(&fixture.certificate)
            .expect("aggregate BLS and every Pasta signature verify at ingress");
        let decoded = mint::decode_kagemusha_mint_finality_seal_bundle_v1(
            fixture
                .certificate
                .kagemusha_finality_seal_payload()
                .expect("bundle envelope")
                .expect("mandatory bundle"),
        )
        .expect("decode exact canonical bundle");
        assert_eq!(decoded, fixture.bundle);

        for (label, vote) in enclosing_vote_substitutions(&fixture.vote) {
            let certificate =
                fixture.signed_certificate(certificate_for_vote(&vote), &fixture.bundle);
            assert!(
                matches!(
                    fixture.verify_bundle(&certificate, &fixture.bundle),
                    Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
                ),
                "{label}"
            );
            assert!(
                fixture.verify_certificate(&certificate).is_err(),
                "fresh aggregate BLS must not authorize a substituted {label}"
            );
        }
        for (label, message) in statement_substitutions(&fixture.bundle.message) {
            let mut bundle = fixture.bundle.clone();
            bundle.message = message;
            assert!(
                matches!(
                    fixture.verify_bundle(&fixture.certificate, &bundle),
                    Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
                ),
                "{label}"
            );
            let certificate = fixture.signed_certificate(fixture.certificate.clone(), &bundle);
            assert!(
                fixture.verify_certificate(&certificate).is_err(),
                "BLS-authenticated QC rejects a substituted seal {label}"
            );
        }
        let mut signed_other_statement = fixture.bundle.clone();
        signed_other_statement.message.subject_digest[0] ^= 1;
        signed_other_statement.seals = (0..3)
            .map(|index| sign_seal(&fixture.context, index, &signed_other_statement.message))
            .collect();
        assert!(matches!(
            fixture.verify_bundle(&fixture.certificate, &signed_other_statement),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(
            fixture
                .verify_certificate(
                    &fixture
                        .signed_certificate(fixture.certificate.clone(), &signed_other_statement)
                )
                .is_err()
        );

        let mut different_signers = fixture.bundle.clone();
        different_signers.seals[2] = sign_seal(&fixture.context, 3, &different_signers.message);
        different_signers
            .validate()
            .expect("another exact ordered quorum");
        assert!(matches!(
            fixture.verify_bundle(&fixture.certificate, &different_signers),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(
            fixture
                .verify_certificate(
                    &fixture.signed_certificate(fixture.certificate.clone(), &different_signers)
                )
                .is_err()
        );
        let mut other_certificate = fixture.certificate.clone();
        other_certificate.signers = vec![0, 1, 3];
        let other_certificate = fixture.signed_certificate(other_certificate, &fixture.bundle);
        assert!(matches!(
            fixture.verify_bundle(&other_certificate, &fixture.bundle),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(fixture.verify_certificate(&other_certificate).is_err());
        fixture
            .verify_certificate(&fixture.signed_certificate(other_certificate, &different_signers))
            .expect("coherent replacement by another exact quorum is valid");

        for signers in [vec![0, 1], vec![0, 1, 2, 3], vec![0, 0, 1], vec![0, 2, 1]] {
            let mut certificate = fixture.certificate.clone();
            certificate.signers = signers.clone();
            let mut bundle = fixture.bundle.clone();
            bundle.seals = signers
                .iter()
                .map(|index| sign_seal(&fixture.context, *index, &bundle.message))
                .collect();
            let certificate = fixture.signed_certificate(certificate, &bundle);
            assert!(
                matches!(
                    fixture.verify_bundle(&certificate, &bundle),
                    Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
                ),
                "exact ordered quorum rejects {signers:?}"
            );
            assert!(
                fixture.verify_certificate(&certificate).is_err(),
                "ingress rejects {signers:?}"
            );
        }
        for index in 0..fixture.bundle.seals.len() {
            for eq in [true, false] {
                let mut corrupt = fixture.bundle.clone();
                if eq {
                    corrupt.seals[index].eq_proof_signature =
                        signed_other_statement.seals[index].eq_proof_signature;
                } else {
                    corrupt.seals[index].ep_proof_signature =
                        signed_other_statement.seals[index].ep_proof_signature;
                }
                corrupt
                    .validate()
                    .expect("canonical signatures from the wrong statement");
                let error = fixture
                    .verify_bundle(&fixture.certificate, &corrupt)
                    .expect_err("every signer's two Pasta equations are mandatory");
                assert!(matches!(
                    error,
                    mint::KagemushaMintFinalityErrorV1::InvalidSignature(_)
                ));
                assert!(error.to_string().contains("Schnorr equation failed"));
                let certificate = fixture.signed_certificate(fixture.certificate.clone(), &corrupt);
                wire::finality::verify_quorum_certificate_with_validator_pops(
                    &fixture.context,
                    &certificate,
                    &fixture.pops,
                )
                .expect("aggregate BLS remains independently valid");
                assert!(
                    fixture.verify_certificate(&certificate).is_err(),
                    "ingress must check signer {index}, Eq mutation = {eq}"
                );
            }
        }
        let bls = fixture
            .certificate
            .bls_aggregate_signature()
            .expect("valid aggregate framing");
        let mut missing = fixture.certificate.clone();
        missing.aggregate_signature = bls.to_vec();
        assert!(missing.kagemusha_finality_seal_payload().is_err());
        assert!(fixture.verify_certificate(&missing).is_err());
        let mut wrong_kind = fixture.certificate.clone();
        wrong_kind.aggregate_signature = wire::encode_kagemusha_consensus_signature_envelope_v1(
            wire::KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1,
            bls,
            &fixture.bundle.encode(),
        )
        .expect("well-framed wrong envelope kind");
        assert!(wrong_kind.kagemusha_finality_seal_payload().is_err());
        assert!(fixture.verify_certificate(&wrong_kind).is_err());
        let mut bad_bls = fixture.certificate.clone();
        let wrong_signatures = fixture.keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), b"another BLS statement")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let wrong_bls = iroha_crypto::bls_normal_aggregate_signatures(
            &wrong_signatures
                .iter()
                .map(Vec::as_slice)
                .collect::<Vec<_>>(),
        )
        .expect("real aggregate for another statement");
        bad_bls.aggregate_signature = wire::encode_kagemusha_consensus_signature_envelope_v1(
            wire::KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1,
            &wrong_bls,
            &fixture.bundle.encode(),
        )
        .expect("valid envelope with wrong aggregate BLS signature");
        fixture
            .verify_bundle(&bad_bls, &fixture.bundle)
            .expect("paired quorum remains independently valid");
        assert!(
            fixture.verify_certificate(&bad_bls).is_err(),
            "Pasta quorum cannot replace aggregate BLS verification"
        );

        let mut out_of_range = fixture.certificate.clone();
        out_of_range.signers = vec![0, 1, 4];
        assert!(matches!(
            fixture.verify_bundle(&out_of_range, &fixture.bundle),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(fixture.verify_certificate(&out_of_range).is_err());
        let mut prepare = fixture.certificate.clone();
        prepare.phase = wire::GlobalPhase::Prepare;
        let prepare = fixture.signed_certificate(prepare, &fixture.bundle);
        assert!(matches!(
            fixture.verify_bundle(&prepare, &fixture.bundle),
            Err(mint::KagemushaMintFinalityErrorV1::InvalidStatement(_))
        ));
        assert!(fixture.verify_certificate(&prepare).is_err());

        let (boundary_vote, boundary_message) = zero_top_up_boundary(&fixture);
        let boundary_bundle = KagemushaMintFinalitySealBundleV1 {
            seals: (0..3)
                .map(|index| sign_seal(&fixture.context, index, &boundary_message))
                .collect(),
            message: boundary_message,
        };
        let mut boundary_certificate =
            fixture.signed_certificate(certificate_for_vote(&boundary_vote), &boundary_bundle);
        fixture
            .verify_bundle(&boundary_certificate, &boundary_bundle)
            .expect("zero-top-up paired quorum");
        fixture
            .verify_certificate(&boundary_certificate)
            .expect("sealed zero-top-up boundary QC");
        boundary_certificate.aggregate_signature = boundary_certificate
            .bls_aggregate_signature()
            .expect("boundary aggregate BLS")
            .to_vec();
        assert_eq!(
            boundary_certificate.kagemusha_finality_seal_payload(),
            Ok(None)
        );
        wire::finality::verify_quorum_certificate_with_validator_pops(
            &fixture.context,
            &boundary_certificate,
            &fixture.pops,
        )
        .expect("unsealed boundary aggregate BLS is still valid");
        assert!(
            fixture.verify_certificate(&boundary_certificate).is_err(),
            "epoch rotation cannot omit its Pasta bundle"
        );
    }
}

#[cfg(feature = "bls")]
#[test]
fn timeout_vote_installs_embedded_qc_before_forming_tc() {
    let directory = TempDir::new().expect("temporary directory");
    let (context, keys, pops) = authenticated_context();
    let verified_context =
        VerifiedHeightContext::genesis(context.clone(), pops).expect("verify context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("timeout-safety.wal"),
        verified_context,
        None,
        reducer::Generation::new(1),
        [0x33; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let subject = subject(13);
    let prepare_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(13),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let prepare_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &prepare_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let prepare_refs = prepare_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: execution_commitment(13),
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
            .expect("aggregate PrepareQC"),
    };
    let protected_payload = [13, 2];
    let manifest = encode_payload(&context, round, subject, &protected_payload)
        .expect("encode protected-body payload")
        .manifest()
        .clone();
    let core_manifest = adapter
        .registry
        .manifest_to_core(&manifest, &context)
        .expect("register protected-body manifest");
    let core_round = reducer::Round::new(round.height, round.view);
    let core_subject = core_manifest.subject();
    let original_tag = adapter.current_tag();
    let mut all_effects = Vec::new();
    for signer in 0_u32..3 {
        if signer == 2 {
            adapter.deferred_completions.push_back(DeferredInput {
                admission_ordinal: 1,
                admission_capability: DeferredAdmissionCapability::for_test(1),
                event: reducer::Event::BodyAvailable {
                    tag: original_tag,
                    round: core_round,
                    subject: core_subject,
                },
                completion_evidence: Some(BodyPipelineCompletionEvidence::BodyAvailable {
                    manifest: manifest.clone(),
                }),
                retag_authenticated_ingress: false,
                priority: DeferredPriority::Completion,
                protected_progress: false,
                admission: None,
                authenticated_wire_identity: None,
                admitted_at: Instant::now(),
                eligible_skips: 0,
            });
        }
        let mut timeout = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare.clone()),
            signer,
            signature: Vec::new(),
        };
        timeout.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer")].private_key(),
            &timeout.signature_preimage(),
        )
        .payload()
        .to_vec();
        let authenticated = adapter
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(timeout),
            ))
            .expect("authenticate self-contained timeout vote");
        all_effects.push(
            adapter
                .receive_authenticated(authenticated)
                .expect("ingest timeout vote")
                .into_effects(),
        );
    }
    let final_effects = all_effects.pop().expect("three timeout outcomes");
    assert_eq!(adapter.reducer.durable_state().current_view(), 1);
    assert!(adapter.reducer.durable_state().highest_prepare().is_some());
    assert!(final_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(_),
            ..
        })
    )));
    assert!(
        final_effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::EnterView { .. }))
    );
    assert!(
        !final_effects
            .iter()
            .any(|effect| matches!(effect, AdapterEffect::StoreBody { .. })),
        "old-generation BodyAvailable must not cross EnterView before executor rebinding"
    );
    let (rebound_tag, protected_lock) = final_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::EnterView {
                tag,
                protected_lock,
                ..
            } => Some((*tag, protected_lock.as_ref())),
            _ => None,
        })
        .expect("view installation effect");
    assert_eq!(protected_lock, Some(&prepare));
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::BodyAvailable { tag, round, subject },
            ..
        }) if *tag == original_tag && *round == core_round && *subject == core_subject
    ));
    assert_eq!(
        adapter.rebind_deferred_body_available(original_tag, rebound_tag, &manifest),
        1
    );
    assert!(matches!(
        adapter.deferred_completions.front(),
        Some(DeferredInput {
            event: reducer::Event::BodyAvailable { tag, .. },
            ..
        }) if *tag == rebound_tag
    ));
    assert_eq!(
        adapter
            .retire_deferred_body_available(rebound_tag, &manifest)
            .expect("persist rebound completion retirement"),
        1
    );
    assert!(adapter.deferred_completions.is_empty());
}
