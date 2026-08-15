use crate::sumeragi::v2_lifecycle_coordinator::{
    reviewed_lifecycle_ledger_source_for_test, reviewed_lifecycle_work_registry_source_for_test,
};
#[test]
fn exact_live_wal_cut_rejects_pre_persist_effect_without_appending() {
    let directory = TempDir::new().expect("temporary missing-cause WAL directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let effect = reducer::Effect::FetchBody {
        tag: adapter.current_tag(),
        round: reducer::Round::new(adapter.wire_context.height, adapter.current_tag().view()),
        subject: reducer::Subject::default(),
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    assert!(matches!(
        adapter.drive_exact_persisted_continuation(effect),
        Err(AdapterError::LiveWalReplayCauseMismatch)
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}
#[test]
fn quorum_forming_local_timeout_flattens_to_certificate_only() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let round = reducer::Round::new(tag.height(), tag.view());
    let context_id = adapter.reducer.context().id();
    for signer_index in [1_u32, 2] {
        let signer = adapter
            .registry
            .validator_id(signer_index)
            .expect("remote timeout signer belongs to the frozen roster");
        let retained = adapter
            .reducer
            .step(reducer::Event::TimeoutVoteReceived {
                tag,
                vote: reducer::SignedTimeoutVote::new(
                    reducer::TimeoutVote::new(context_id, round, signer, None),
                    reducer::OpaqueSignature::new(vec![
                        u8::try_from(signer_index)
                            .expect("small signer index");
                        96
                    ]),
                ),
            })
            .expect("retain the remote timeout share before local signing");
        assert!(retained.effects().is_empty());
    }
    let sign = adapter
        .timeout_elapsed(tag)
        .expect("persist the local timeout intent");
    let sign_tag = match sign.effects() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(_),
            },
        ] => *tag,
        effects => panic!("unexpected timeout signing frontier: {effects:?}"),
    };
    let formed = adapter
        .signature_completed(sign_tag, vec![0xA1; 96])
        .expect("flatten the quorum-forming timeout persistence boundary");
    assert!(matches!(
        formed.effects(),
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
        ] if entered_tag.view() == tag.view() + 1
            && certificate.round.view == tag.view()
            && certificate.groups.iter().any(|group| group.signers.contains(&0))
    ));
    assert_eq!(adapter.current_tag().view(), tag.view() + 1);
    assert!(!adapter.fail_closed);
}
#[test]
fn drive_effects_rejects_oversized_non_persisting_batch() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let effect = reducer::Effect::FetchBody {
        tag,
        round: reducer::Round::new(tag.height(), tag.view()),
        subject: reducer::Subject::default(),
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    let oversized = vec![effect; MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1];
    assert!(matches!(
        adapter.drive_effects(oversized),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects,
            maximum_initial_effects,
            persist_effects: 0,
            continuation_effects: 0,
            continuation_contains_persist: false,
            ..
        }) if initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP + 1
            && maximum_initial_effects == MAX_ADAPTER_EFFECTS_PER_MACRO_STEP
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}
#[test]
fn drive_effects_rejects_record_specific_overbudget_before_wal_append() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed { tag })
        .expect("stage the sole TimeoutIntent Persist")
        .into_effects();
    assert!(matches!(
        timeout.as_slice(),
        [reducer::Effect::Persist { .. }]
    ));
    let unrelated = reducer::Effect::FetchBody {
        tag,
        round: reducer::Round::new(tag.height(), tag.view()),
        subject: reducer::Subject::default(),
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    let mut overbudget = vec![unrelated];
    overbudget.extend(timeout);
    assert!(matches!(
        adapter.drive_effects(overbudget),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects: 2,
            maximum_initial_effects: 1,
            persist_effects: 1,
            continuation_effects: 0,
            maximum_continuation_effects: 1,
            maximum_flattened_effects: 1,
            continuation_contains_persist: false,
        })
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}
#[test]
fn drive_effects_rejects_multiple_persist_owners_before_wal_append() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let tag = adapter.current_tag();
    let mut timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutElapsed { tag })
        .expect("stage the sole TimeoutIntent Persist")
        .into_effects();
    let persist = timeout.pop().expect("one Persist effect");
    assert!(matches!(&persist, reducer::Effect::Persist { .. }));
    assert!(matches!(
        adapter.drive_effects(vec![persist.clone(), persist]),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            persist_effects: 2,
            continuation_effects: 0,
            continuation_contains_persist: false,
            ..
        })
    ));
    assert!(adapter.fail_closed);
    assert!(adapter.wal.recovered_records().is_empty());
}
#[test]
fn post_wal_oversized_continuation_fails_closed_and_replays_exact_record() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let protected_subject = subject(0x6d);
    let prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: protected_subject,
        execution_commitment: execution_commitment(0x6d),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x6d; 96],
    };
    let timeout = wire::TimeoutCertificate {
        round: wire_round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x6e; 96],
        }],
    };
    let wire_context = adapter.wire_context.clone();
    let timeout = adapter
        .registry
        .tc_to_core(&timeout, &wire_context)
        .expect("convert the lock-promoting timeout certificate");
    let timeout_tag = adapter.current_tag();
    let pending_timeout = adapter
        .reducer
        .step(reducer::Event::TimeoutCertificateReceived {
            tag: timeout_tag,
            certificate: timeout,
        })
        .expect("stage the real InstallTimeout persistence");
    let mut pending_effects = pending_timeout.into_effects();
    let reducer::Effect::Persist { tag, entry } = pending_effects
        .pop()
        .expect("InstallTimeout has one Persist effect")
    else {
        panic!("InstallTimeout must stage persistence");
    };
    assert!(pending_effects.is_empty());
    // Keep the reducer's real lock-promoting continuation, but classify
    // and encode this adversarial boundary call as the smaller
    // TimeoutIntent class. The substitute is itself a valid first WAL
    // record with the exact pending persistence ID, so the continuation
    // guard is reached only after the append succeeds.
    let timeout_round = reducer::Round::new(wire_round.height, wire_round.view);
    let local_validator = adapter
        .reducer
        .local_validator()
        .expect("test adapter is a validator");
    let forged_entry = reducer::WalEntry::new(
        entry.id(),
        reducer::WalRecord::TimeoutIntent(reducer::TimeoutVote::new(
            adapter.reducer.context().id(),
            timeout_round,
            local_validator,
            None,
        )),
    );
    assert!(matches!(
        adapter.drive_effects(vec![reducer::Effect::Persist {
            tag,
            entry: forged_entry,
        }]),
        Err(AdapterError::AdapterMacroStepBoundExceeded {
            initial_effects: 1,
            maximum_initial_effects: 1,
            persist_effects: 1,
            continuation_effects: 2,
            maximum_continuation_effects: 1,
            maximum_flattened_effects: 1,
            continuation_contains_persist: false,
        })
    ));
    assert!(adapter.fail_closed);
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    assert_eq!(adapter.wal.recovered_records()[0].sequence(), 0);
    drop(adapter);
    let (recovered, first_startup) =
        open_test(&directory).expect("replay the one valid timeout intent");
    assert!(recovered.ingress_ready());
    assert!(!recovered.fail_closed);
    assert_eq!(recovered.wal.recovered_records().len(), 1);
    assert_eq!(recovered.reducer.durable_state().last_id().get(), 1);
    assert!(
        recovered
            .reducer
            .durable_state()
            .timeout_intent(timeout_round)
            .is_some()
    );
    assert!(first_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    assert!(matches!(
        first_startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(vote),
            ..
        }] if vote.round == wire_round
            && vote.highest_prepare_qc.is_none()
            && vote.signer == 0
            && vote.signature.is_empty()
    ));
    drop(recovered);
    let (recovered_again, second_startup) =
        open_test(&directory).expect("repeat deterministic timeout-intent replay");
    assert_eq!(second_startup, first_startup);
    assert!(second_startup.len() <= MAX_ADAPTER_EFFECTS_PER_MACRO_STEP);
    assert_eq!(recovered_again.wal.recovered_records().len(), 1);
    assert!(recovered_again.ingress_ready());
    assert!(!recovered_again.fail_closed);
}
#[test]
fn open_records_exactly_one_recovery_progress_transition() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    assert!(matches!(
        adapter.last_progress,
        Some((
            generation,
            round,
            wire::SumeragiV2ProgressTransition::RecoveryReplayed
        )) if generation == adapter.current_tag().generation()
            && round == reducer::Round::new(adapter.wire_context.height, 0)
    ));
    assert_eq!(
        adapter
            .ignore_counts
            .get(&reducer::IgnoreReason::Duplicate)
            .copied()
            .unwrap_or_default(),
        0,
        "opening must step ResumeAfterReplay once, not record a duplicate replay"
    );
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "the replay control trigger cannot consume candidate-tombstone capacity"
    );
    for attempt in 0..3 {
        adapter
            .retransmit_elapsed(adapter.current_tag())
            .unwrap_or_else(|error| panic!("retransmit control attempt {attempt}: {error}"));
    }
    assert_eq!(
        adapter.serviced_candidate_count_for_test(),
        0,
        "periodic retransmission triggers remain executable without becoming tombstones"
    );
    let status = adapter.status().expect("status after replay");
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
            ..
        })
    ));
}
#[cfg(feature = "bls")]
#[test]
fn first_recovery_snapshot_tracks_the_durable_locked_body() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let locked_subject = subject(0xCE);
    let wire_round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let (_, keys, _) = authenticated_context();
    let mut wire_prepare = wire::QuorumCertificate {
        round: wire_round,
        proposal_round: wire_round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xCE),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut wire_prepare, &keys);
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register durable PrepareQC");
    let round = prepare.round();
    let core_subject = prepare.subject();
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("local fixture validator");
    let lock_entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::LockAndCommit {
            prepare,
            vote: reducer::Vote::new(
                adapter.reducer.context().id(),
                round,
                reducer::Phase::Commit,
                core_subject,
                local_validator,
            ),
        },
    );
    let encoded = adapter
        .registry
        .encode_wal_entry(&lock_entry, &TestAggregator)
        .expect("encode durable lock");
    assert_eq!(
        adapter
            .wal
            .append(&encoded)
            .expect("append durable lock")
            .sequence(),
        0
    );
    drop(adapter);
    let (mut recovered, startup) = open_test(&directory).expect("recover durable lock");
    assert!(matches!(
        startup.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        }] if vote.phase == wire::GlobalPhase::Commit
            && vote.subject == locked_subject
    ));
    assert_eq!(recovered.active_subject, Some((round, core_subject)));
    let status = recovered.status().expect("first locked recovery snapshot");
    assert_eq!(
        status.liveness.work.candidate,
        wire::SumeragiV2LocalWorkStage::Complete
    );
    assert_eq!(
        status.liveness.work.body_recovery,
        wire::SumeragiV2LocalWorkStage::Queued
    );
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::RecoveryReplayed,
            ..
        })
    ));
}
#[test]
fn persistence_is_fsynced_before_sign_is_exposed() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    assert!(adapter.ingress_ready());
    let proposer = adapter.status().expect("status").leader;
    let subject = subject(7);
    let proposal = proposal(&adapter.wire_context, proposer, subject);
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
    let store = adapter
        .body_available(tag, manifest)
        .expect("body available")
        .into_effects();
    assert!(matches!(
        store.as_slice(),
        [AdapterEffect::StoreBody { .. }]
    ));
    let receipt = durable_body_receipt(&adapter, round, subject);
    let validate = adapter
        .body_stored(tag, round, subject, &receipt)
        .expect("body stored")
        .into_effects();
    assert!(matches!(
        validate.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let validated = ValidatedBodyReceipt::for_test(receipt.clone());
    let sign = adapter
        .validation_succeeded(tag, round, subject, &validated)
        .expect("valid body")
        .into_effects();
    assert!(matches!(sign.as_slice(), [AdapterEffect::Sign { .. }]));
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    assert_eq!(adapter.reducer.durable_state().last_id().get(), 1);
}
fn advance_direct_validation_fixture_to_durable(
    adapter: &mut SumeragiV2Adapter,
    marker: u8,
) -> (
    reducer::EventTag,
    wire::PayloadManifest,
    DurableBodyReceipt,
    ValidatedBodyReceipt,
) {
    let (tag, _proposal, manifest, durable, validated) =
        advance_direct_validation_fixture_to_durable_with_proposal(adapter, marker);
    (tag, manifest, durable, validated)
}
fn ordinary_validate_predecessor_for_test(
    tag: reducer::EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    owner_ordinal: u128,
) -> (AdapterEffect, PendingRuntimeEffectBinding) {
    let store = AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let owner = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&store),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, owner_ordinal)],
    )
    .expect("bind one ordinary Store owner")
    .pop()
    .expect("one ordinary Store owner");
    let store_pending = owner
        .pending_adapter_effect_binding(&store)
        .expect("bind ordinary Store predecessor");
    let validate_pending = store_pending
        .project_store_validate_successor(&store, &validate)
        .expect("project ordinary Validate predecessor");
    (validate, validate_pending)
}
fn advance_direct_validation_fixture_to_durable_with_proposal(
    adapter: &mut SumeragiV2Adapter,
    marker: u8,
) -> (
    reducer::EventTag,
    wire::Proposal,
    wire::PayloadManifest,
    DurableBodyReceipt,
    ValidatedBodyReceipt,
) {
    let proposer = adapter.status().expect("status").leader;
    let body_subject = subject(marker);
    let proposal_message = proposal(&adapter.wire_context, proposer, body_subject);
    let wire::ConsensusMessageV2Payload::Proposal(exact_proposal) = &proposal_message.payload
    else {
        unreachable!("direct-validation fixture starts from one Proposal")
    };
    let exact_proposal = exact_proposal.clone();
    let fetch = adapter
        .receive_verified(proposal_message)
        .expect("accept direct-validation proposal")
        .into_effects();
    let (tag, manifest) = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected direct-validation fetch effects: {effects:?}"),
    };
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare direct BodyAvailable transition")
    else {
        panic!("missing body must prepare one Store successor")
    };
    assert!(matches!(
        available.commit(),
        AdapterEffect::StoreBody {
            tag: effect_tag,
            round,
            subject,
        } if effect_tag == tag && round == manifest.round && subject == manifest.subject
    ));
    let durable = durable_body_receipt(adapter, manifest.round, manifest.subject);
    let DirectBodyStoredPreparation::Applied(stored) = adapter
        .prepare_direct_body_stored(tag, manifest.round, manifest.subject, &durable)
        .expect("prepare direct BodyStored transition")
    else {
        panic!("available body must prepare one Validate successor")
    };
    assert!(matches!(
        stored.commit(),
        AdapterEffect::ValidateBody {
            tag: effect_tag,
            round,
            subject,
        } if effect_tag == tag && round == manifest.round && subject == manifest.subject
    ));
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    assert_eq!(exact_proposal.manifest, manifest);
    (tag, exact_proposal, manifest, durable, validated)
}
fn reopen_with_prepare_intent(
    directory: &TempDir,
    marker: u8,
) -> (
    RecoveredAdapterStartup,
    wire::Vote,
    wire::Proposal,
    wire::PayloadManifest,
    ValidatedBodyReceipt,
) {
    let (mut adapter, startup) = open_test(directory).expect("open Prepare replay fixture");
    assert!(startup.is_empty());
    let (tag, proposal, manifest, _durable, validated) =
        advance_direct_validation_fixture_to_durable_with_proposal(&mut adapter, marker);
    let sign = adapter
        .validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("persist the exact PrepareIntent")
        .into_effects();
    let [
        AdapterEffect::Sign {
            request: SignRequest::Vote(expected_vote),
            ..
        },
    ] = sign.as_slice()
    else {
        panic!("durable PrepareIntent must expose one vote sign")
    };
    assert_eq!(expected_vote.phase, wire::GlobalPhase::Prepare);
    let expected_vote = expected_vote.clone();
    drop(adapter);
    let recovered = open_recovered_startup_test(directory)
        .expect("replay the durable PrepareIntent behind the sealed startup cut");
    (recovered, expected_vote, proposal, manifest, validated)
}
#[allow(clippy::too_many_lines)]
fn reopen_with_persisted_prepare_intent(
    safety_directory: &TempDir,
    body_root: &std::path::Path,
    marker: u8,
) -> (
    RecoveredAdapterStartup,
    wire::Proposal,
    wire::PayloadManifest,
    ValidatedBodyReceipt,
) {
    let (mut adapter, startup) =
        open_test(safety_directory).expect("open persisted Prepare replay fixture");
    assert!(startup.is_empty());
    let context = adapter.wire_context.clone();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let leader = context.leader(round.view);
    let leader_index = usize::try_from(leader).expect("fixture leader index fits usize");
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic persisted-body signer")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert_eq!(
        keys[leader_index].public_key(),
        context.roster[leader_index].validator.public_key(),
    );
    let header = BlockHeader::new(
        NonZeroU64::new(round.height).expect("fixture height is non-zero"),
        None,
        None,
        None,
        4_000 + u64::from(marker),
        round.view,
    );
    let signature = SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
        .expect("sign persisted Prepare body");
    let block = SignedBlock::presigned(
        BlockSignature::new(u64::from(leader), signature),
        header,
        Vec::new(),
    );
    let canonical_wire = block
        .encode_wire()
        .expect("encode persisted Prepare SignedBlockWire");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let chunks = wire::encode_payload_chunks(context.da_layout, &canonical_wire)
        .expect("encode persisted Prepare body chunks");
    let manifest = wire::PayloadManifest::derive(
        &context,
        round,
        subject,
        u64::try_from(canonical_wire.len()).expect("fixture body length fits u64"),
        &chunks,
    )
    .expect("derive persisted Prepare manifest");
    let proposal = wire::Proposal {
        round,
        proposer: leader,
        subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![0x91],
    };
    let fetch = adapter
        .receive_verified(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
        ))
        .expect("accept persisted Prepare proposal")
        .into_effects();
    let [
        AdapterEffect::FetchBody {
            tag,
            manifest: Some(projected_manifest),
            ..
        },
    ] = fetch.as_slice()
    else {
        panic!("persisted Prepare proposal must expose one exact Fetch")
    };
    assert_eq!(projected_manifest, &manifest);
    let tag = *tag;
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare persisted Prepare Store transition")
    else {
        panic!("persisted Prepare body must project one Store")
    };
    assert!(matches!(
        available.commit(),
        AdapterEffect::StoreBody { .. }
    ));
    let mut body_store = super::super::v2_body_store::V2BodyStore::open(body_root, context)
        .expect("open persisted Prepare body store");
    let durable = body_store
        .store(manifest.clone(), canonical_wire)
        .expect("fsync persisted Prepare body");
    let DirectBodyStoredPreparation::Applied(stored) = adapter
        .prepare_direct_body_stored(tag, round, subject, &durable)
        .expect("prepare persisted Prepare Validate transition")
    else {
        panic!("persisted Prepare Store must project one Validate")
    };
    assert!(matches!(
        stored.commit(),
        AdapterEffect::ValidateBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        } if effect_tag == tag && effect_round == round && effect_subject == subject
    ));
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let validated = body_store
        .validate(&durable, |_| Ok::<_, String>(commitment))
        .expect("fsync persisted Prepare validation marker");
    let sign = adapter
        .validation_succeeded(tag, round, subject, &validated)
        .expect("persist exact PrepareIntent for the durable body")
        .into_effects();
    assert!(matches!(
        sign.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        }] if vote.phase == wire::GlobalPhase::Prepare
            && vote.execution_commitment == commitment
    ));
    drop(adapter);
    drop(body_store);
    let recovered = open_recovered_startup_test(safety_directory)
        .expect("replay persisted PrepareIntent behind the startup cut");
    (recovered, proposal, manifest, validated)
}
fn join_recovered_prepare_startup<'registry>(
    startup: RecoveredAdapterStartup,
    proposal: wire::Proposal,
    manifest: wire::PayloadManifest,
    validated: ValidatedBodyReceipt,
    holder: &'registry mut super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder,
) -> AuthenticatedRecoveredWalLifecycleStartup<'registry> {
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate recovered Prepare WAL vote: {error}")
        });
    let authority = authenticated
        .recovered_phase_vote_for_test()
        .expect("PrepareIntent carries one restart vote");
    let verified = VerifiedHeightContext {
        context: authenticated.adapter.wire_context.clone(),
        proofs_of_possession: authenticated.adapter.proofs_of_possession.clone(),
        parent_verification: authenticated.adapter.parent_verification.clone(),
    };
    let validate = holder.recovered_wal_validate_registry_cut_for_test(
        &verified, authority, proposal, manifest, validated,
    );
    authenticated
        .authenticate_recovered_validate(validate)
        .unwrap_or_else(|error| panic!("join recovered Prepare WAL vote: {}", error.reason()))
}
fn install_recovered_prepare_startup<'registry>(
    safety_directory: &TempDir,
    ledger_root: &std::path::Path,
    marker: u8,
    holder: &'registry mut super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder,
) -> InstalledRecoveredWalLifecycleStartup<'registry> {
    let (startup, _expected_vote, proposal, manifest, validated) =
        reopen_with_prepare_intent(safety_directory, marker);
    let joined = join_recovered_prepare_startup(startup, proposal, manifest, validated, holder);
    let (_summary, durable) = joined
        .persist_repair_for_test(ledger_root)
        .unwrap_or_else(|error| panic!("fsync recovered Prepare repair: {}", error.reason()));
    durable
        .install_recovered_sign_for_test(ledger_root)
        .unwrap_or_else(|error| panic!("install recovered Prepare Sign: {}", error.reason()))
}
fn verified_from_installed_startup(
    startup: &InstalledRecoveredWalLifecycleStartup<'_>,
) -> VerifiedHeightContext {
    VerifiedHeightContext {
        context: startup.adapter.wire_context.clone(),
        proofs_of_possession: startup.adapter.proofs_of_possession.clone(),
        parent_verification: startup.adapter.parent_verification.clone(),
    }
}
fn empty_authenticated_lifecycle_recovery(
    verified: &VerifiedHeightContext,
    ledger_root: &std::path::Path,
    payload_root: &std::path::Path,
    body_root: &std::path::Path,
) -> (
    super::super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
    AuthenticatedLifecycleRecoveryCut,
) {
    let body_store =
        super::super::v2_body_store::V2BodyStore::open(body_root, verified.context().clone())
            .expect("open empty same-context body store");
    let (payload_store, recovered) =
        super::super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
            payload_root,
            verified.context(),
        )
        .expect("open empty same-context Certified-Serve payload store");
    let signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic empty-payload signer");
    let payloads = recovered
        .authenticate(verified, &signer, &body_store)
        .expect("authenticate empty Certified-Serve payload recovery");
    let recovery = AuthenticatedLifecycleRecoveryCut::open_empty_for_recovered_wal_test(
        verified,
        ledger_root,
        payloads,
    )
    .expect("open exact ledger and assemble empty same-context lifecycle recovery cut");
    (payload_store, recovery)
}
#[test]
fn production_lifecycle_owner_factory_opens_the_private_no_vote_branch() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("temporary no-vote safety store");
    let storage = TempDir::new().expect("temporary no-vote lifecycle stores");
    let authenticated = open_recovered_startup_test(&safety)
        .expect("open sealed no-vote adapter startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate empty recovered WAL: {error}"));
    assert!(authenticated.has_no_recovered_wal_authority_for_test());
    assert!(authenticated.effects.is_empty());
    assert!(
        authenticated
            .recovered_validation_authority()
            .authorizes_context(&context()),
        "the sealed startup retains its exact marker-recovery frontier"
    );
    assert_eq!(
        authenticated.restored_producer_continuation_ordinal_high_watermark(),
        None
    );
    assert!(authenticated.durable_producer_terminal_tokens().is_empty());
    authenticated
        .leader_wire_recovery_authority()
        .expect("the sealed startup projects its leader-wire recovery boundary");
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic local Serve retainer");
    let mut owner = authenticated
        .open_production_lifecycle_owner_v1_from_roots_for_test(
            &lifecycle_owner_config(),
            4,
            &storage.path().join("ledger"),
            &storage.path().join("serve"),
            &storage.path().join("body"),
            super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("open complete no-vote lifecycle owner: {error}"));
    assert!(owner.exact_recovered_fetch_join_for_test());
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "owner construction must keep status sealed until runner activation"
    );
    crate::sumeragi::status::clear_v2_status();
}
fn assert_control_repair_and_coalesce(proposal_intent: bool, marker: u8) {
    let safety = TempDir::new().expect("temporary control safety store");
    let storage = TempDir::new().expect("temporary control lifecycle stores");
    if proposal_intent {
        persist_proposal_intent_for_control_recovery(&safety, marker);
    } else {
        persist_timeout_intent_for_control_recovery(&safety);
    }
    crate::sumeragi::status::clear_v2_status();
    assert!(crate::sumeragi::status::v2_status().is_none());
    let mut first = open_control_owner_for_test(&safety, &storage, proposal_intent);
    let first_summary = first
        .recovered_control_row_summary_for_test()
        .expect("missing-row repair installs one exact control row and carrier");
    assert_eq!(first_summary.0, first_summary.1);
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "control owner construction keeps status sealed until runner activation"
    );
    let ledger_path = storage.path().join("ledger/lifecycle-ledger-v1.norito");
    let first_frame = std::fs::read(&ledger_path).expect("read repaired control LedgerV1");
    #[cfg(unix)]
    let first_inode = {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(&ledger_path)
            .expect("inspect repaired control LedgerV1")
            .ino()
    };
    drop(first);
    crate::sumeragi::status::clear_v2_status();
    let mut reopened = open_control_owner_for_test(&safety, &storage, proposal_intent);
    let reopened_summary = reopened
        .recovered_control_row_summary_for_test()
        .expect("coalesced reopen reconstructs the exact volatile carrier");
    assert_eq!(reopened_summary, first_summary);
    assert_eq!(
        std::fs::read(&ledger_path).expect("read coalesced control LedgerV1"),
        first_frame,
        "exact coalesce preserves the full frame, high-water, and control ordinal"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        assert_eq!(
            std::fs::metadata(&ledger_path)
                .expect("inspect coalesced control LedgerV1")
                .ino(),
            first_inode,
            "exact coalesce validates the durable frame without replacing it"
        );
    }
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "coalesced control owner remains unpublished until runner activation"
    );
    crate::sumeragi::status::clear_v2_status();
}
#[test]
fn bls_proposal_intent_control_sign_repairs_and_coalesces_exactly() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    assert_control_repair_and_coalesce(true, 0xC1);
}
#[test]
fn bls_timeout_intent_control_sign_repairs_and_coalesces_exactly() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    assert_control_repair_and_coalesce(false, 0xC2);
}
#[cfg(feature = "bls")]
#[test]
fn bls_decision_fetch_repairs_and_coalesces_without_rewrite() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("temporary Decision Fetch safety store");
    let storage = TempDir::new().expect("temporary Decision Fetch lifecycle stores");
    let (wire_context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: wire_context.id(),
        height: wire_context.height,
        view: 0,
    };
    let mut decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: subject(0xCA),
        execution_commitment: execution_commitment(0xCA),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut decision, &keys);
    let first_startup = write_and_reopen_authenticated_wal_startup(
        &safety,
        &wire_context,
        &proofs,
        0,
        [0xCA; 32],
        vec![WalRecordV2::Decision(decision)],
    );
    let first_authenticated = first_startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate Decision Fetch startup: {error}"));
    assert!(matches!(
        &first_authenticated.authority,
        RecoveredWalStartupAuthorityV1::DecisionFetch(_)
    ));
    assert!(first_authenticated.effects.is_empty());
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic Decision Fetch Serve retainer");
    let ledger_root = storage.path().join("ledger");
    let serve_root = storage.path().join("serve");
    let body_root = storage.path().join("body");
    let mut first = first_authenticated
        .open_production_lifecycle_owner_v1_from_roots_for_test(
            &lifecycle_owner_config(),
            4,
            &ledger_root,
            &serve_root,
            &body_root,
            super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("open Decision Fetch lifecycle owner: {error}"));
    let first_summary = first
        .recovered_decision_fetch_row_summary_for_test()
        .expect("missing Decision appends one exact Fetch row");
    assert_eq!(first_summary.0, first_summary.1);
    let mixed_sign_ordinal = first.add_recovered_next_vote_completion_for_test(0xCD);
    assert!(
        mixed_sign_ordinal > first_summary.0,
        "the later Sign must win by remaining-stage rank rather than ordinal"
    );
    {
        let runtime_verified = VerifiedHeightContext::genesis(wire_context.clone(), proofs.clone())
            .expect("verify the exact recovered Fetch executor context");
        let (adapter, startup) = SumeragiV2Adapter::open(
            storage
                .path()
                .join("decision-fetch-composite-sign-runtime.wal"),
            runtime_verified,
            Some(0),
            reducer::Generation::new(1),
            [0xCB; 32],
            fingerprints(),
            deferred_admission_ordinals(),
        )
        .expect("open an exact recovered Fetch executor adapter");
        assert!(startup.is_empty());
        let runtime = super::super::v2_runtime::SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("wrap the recovered Fetch executor adapter")
        .0;
        let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
        let (mut services, _) = super::super::v2_worker::tests::fixture();
        let (mut executor, planner_io) = first.bind_body_store_to_recovered_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            2,
        );
        super::super::v2_worker::tests::install_local_signer_for_test(&mut services, &keys[0]);
        assert_eq!(
            first
                .dispatch_recovered_completion_for_test(&services, &mut executor, 0)
                .expect("rank the genuine WAL-backed Sign beside recovered Fetch"),
            super::super::v2_lifecycle_coordinator::ProductionRecoveredCompletionDispatchV1::SignQueued {
                ordinal: mixed_sign_ordinal,
            }
        );
        assert!(
            first.recovered_completion_selection_is_exact_for_test(
                mixed_sign_ordinal,
                first_summary.0,
            ),
            "the lower-rank later Sign is claimed while the higher-rank Fetch remains Ready"
        );
        assert!(!output_guard.restart_required());
        // The extra Sign exists only in this closed scheduler fixture. Model
        // the cold boundary explicitly before discarding its claimed volatile
        // owner; the unchanged Ledger then reopens the durable Fetch alone.
        output_guard.close_admission_for_restart();
        assert!(output_guard.restart_required());
        planner_io.detach(&mut services);
    }
    let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
    let first_frame = std::fs::read(&ledger_path).expect("read Decision Fetch LedgerV1");
    #[cfg(unix)]
    let first_inode = {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(&ledger_path)
            .expect("inspect Decision Fetch LedgerV1")
            .ino()
    };
    drop(first);
    crate::sumeragi::status::clear_v2_status();
    let verified = VerifiedHeightContext::genesis(wire_context.clone(), proofs.clone())
        .expect("reverify Decision Fetch context");
    let reopened = SumeragiV2Adapter::open_recovered_startup_with_aggregator(
        safety.path().join("authenticated-fifo-safety.wal"),
        verified,
        Some(0),
        reducer::Generation::new(50),
        [0xCA; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen unchanged Decision WAL")
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _)| panic!("reauthenticate Decision Fetch startup: {error}"));
    let mut reopened = reopened
        .open_production_lifecycle_owner_v1_from_roots_for_test(
            &lifecycle_owner_config(),
            4,
            &ledger_root,
            &serve_root,
            &body_root,
            super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("coalesce Decision Fetch lifecycle owner: {error}"));
    assert_eq!(
        reopened
            .recovered_decision_fetch_row_summary_for_test()
            .expect("coalesced Decision Fetch retains its exact row"),
        first_summary
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("reread coalesced Decision Fetch LedgerV1"),
        first_frame,
        "exact Decision Fetch coalesce preserves the complete frame"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        assert_eq!(
            std::fs::metadata(&ledger_path)
                .expect("inspect coalesced Decision Fetch LedgerV1")
                .ino(),
            first_inode,
            "exact Decision Fetch coalesce validates without replacing the inode"
        );
    }
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "Decision Fetch owner construction must remain unpublished"
    );
    {
        let runtime_verified = VerifiedHeightContext::genesis(wire_context.clone(), proofs)
            .expect("verify the clean recovered Fetch executor context");
        let (adapter, startup) = SumeragiV2Adapter::open(
            storage
                .path()
                .join("decision-fetch-composite-fetch-runtime.wal"),
            runtime_verified,
            Some(0),
            reducer::Generation::new(2),
            [0xCC; 32],
            fingerprints(),
            deferred_admission_ordinals(),
        )
        .expect("open a clean recovered Fetch executor adapter");
        assert!(startup.is_empty());
        let runtime = super::super::v2_runtime::SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("wrap the clean recovered Fetch executor adapter")
        .0;
        let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
        let (mut services, _) = super::super::v2_worker::tests::fixture();
        let (mut executor, planner_io) = reopened
            .bind_body_store_to_recovered_completion_io_for_test(
                &mut services,
                runtime,
                Arc::clone(&output_guard),
                2,
            );
        super::super::v2_worker::tests::install_local_signer_for_test(&mut services, &keys[0]);
        assert_eq!(
            reopened
                .dispatch_recovered_completion_for_test(&services, &mut executor, 0)
                .expect("dispatch the genuine WAL-backed recovered Fetch"),
            super::super::v2_lifecycle_coordinator::ProductionRecoveredCompletionDispatchV1::FetchDispatched {
                ordinal: first_summary.0,
            }
        );
        assert!(
            services
                .has_pending_exact_output()
                .expect("inspect the recovered Fetch exact-output owner")
        );
        assert!(!output_guard.restart_required());
        planner_io.detach(&mut services);
    }
    assert_eq!(
        crate::sumeragi::status::v2_status()
            .expect("the explicitly published executor adapter remains visible")
            .height,
        wire_context.height
    );
    crate::sumeragi::status::clear_v2_status();
}
#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn bls_decision_fetch_body_markers_fail_before_ledger_mutation() {
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic Decision body-marker retainer");
    for (outcome, promoted_marker, quarantined_marker) in [
        (DecisionBodyMarkerFixture::Validated, 0xD1, 0xD2),
        (DecisionBodyMarkerFixture::Rejected, 0xD3, 0xD4),
    ] {
        let promoted_safety = TempDir::new().expect("temporary promoted Decision marker WAL");
        let promoted_storage = TempDir::new().expect("temporary promoted Decision marker stores");
        let promoted_body_root = promoted_storage.path().join("body");
        let promoted_ledger_root = promoted_storage.path().join("ledger");
        let (startup, body_store) = write_decision_startup_with_body_marker(
            &promoted_safety,
            &promoted_body_root,
            promoted_marker,
            outcome,
        );
        let authenticated = startup
            .authenticate_final_wal_startup_authority()
            .unwrap_or_else(|(error, _)| {
                panic!("authenticate promoted Decision marker startup: {error}")
            });
        let AuthenticatedRecoveredAdapterStartup {
            adapter,
            effects,
            authority,
            validation_authority: _,
            factory_owner: _,
        } = authenticated;
        assert!(effects.is_empty());
        let verified = VerifiedHeightContext {
            context: adapter.wire_context.clone(),
            proofs_of_possession: adapter.proofs_of_possession.clone(),
            parent_verification: adapter.parent_verification.clone(),
        };
        let RecoveredWalStartupAuthorityV1::DecisionFetch(fetch) = authority else {
            panic!("Decision body marker must retain one Fetch authority")
        };
        let Ok(projection) =
            crate::sumeragi::v2_runtime::project_recovered_wal_decision_fetch(&verified, fetch)
        else {
            panic!("project exact promoted Decision Fetch")
        };
        let (payload_store, recovered_payloads) =
            super::super::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &promoted_storage.path().join("serve"),
                verified.context(),
            )
            .expect("open promoted Decision Serve store");
        let serve_payloads = recovered_payloads
            .authenticate(&verified, &local_signer, &body_store)
            .expect("authenticate empty promoted Decision Serve recovery");
        let Err(error) = ProductionLifecycleOwnerV1::open_recovered_decision_fetch_startup(
            verified,
            projection,
            &promoted_ledger_root,
            body_store,
            &lifecycle_owner_config(),
            4,
            payload_store,
            serve_payloads,
            ProductionLifecycleAdapterStartupV1::recovered(adapter, effects),
        ) else {
            panic!("a promoted Decision body marker must block duplicate Fetch startup")
        };
        match outcome {
            DecisionBodyMarkerFixture::Validated => {
                assert!(error.reason().contains("exact validated marker"))
            }
            DecisionBodyMarkerFixture::Rejected => {
                assert!(
                    error
                        .reason()
                        .contains("deterministic local body rejection")
                )
            }
        }
        assert!(
            !promoted_ledger_root.exists(),
            "promoted body conflicts must stop before LedgerV1 opens"
        );
        let quarantined_safety = TempDir::new().expect("temporary quarantined Decision marker WAL");
        let quarantined_storage =
            TempDir::new().expect("temporary quarantined Decision marker stores");
        let quarantined_body_root = quarantined_storage.path().join("body");
        let quarantined_ledger_root = quarantined_storage.path().join("ledger");
        let (startup, body_store) = write_decision_startup_with_body_marker(
            &quarantined_safety,
            &quarantined_body_root,
            quarantined_marker,
            outcome,
        );
        let context = startup.adapter.wire_context.clone();
        drop(body_store);
        drop(startup);
        let reopened =
            super::super::v2_body_store::V2BodyStore::open(&quarantined_body_root, context)
                .expect("reopen quarantined Decision body marker");
        let Err(error) = reopened.into_revalidated_startup() else {
            panic!("a quarantined Decision marker cannot cross the startup seal")
        };
        assert!(matches!(
            error,
            super::super::v2_body_store::V2BodyStoreError::UnrevalidatedValidationMarkers
        ));
        assert!(
            !quarantined_ledger_root.exists(),
            "quarantined body markers stop before LedgerV1 opens"
        );
        assert!(!quarantined_storage.path().join("serve").exists());
    }
}
#[cfg(feature = "bls")]
#[test]
fn bls_revalidated_decision_body_cut_is_same_store_and_drop_restores_exactly() {
    let safety = TempDir::new().expect("temporary Decision body-cut WAL");
    let storage = TempDir::new().expect("temporary Decision body-cut store");
    let body_root = storage.path().join("body");
    let (startup, body_store) = write_decision_startup_with_body_marker(
        &safety,
        &body_root,
        0xD5,
        DecisionBodyMarkerFixture::Validated,
    );
    let context_directory = std::fs::read_dir(&body_root)
        .expect("read Decision body-store root")
        .next()
        .expect("body-store root contains one context")
        .expect("read body-store context entry")
        .path();
    let frames_before = std::fs::read_dir(&context_directory)
        .expect("read Decision body-store context")
        .map(|entry| {
            let path = entry.expect("read Decision body-store frame entry").path();
            let bytes = std::fs::read(&path).expect("read Decision body-store frame");
            (
                path.file_name().expect("frame has a file name").to_owned(),
                bytes,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate Decision body cut: {error}"));
    let AuthenticatedRecoveredAdapterStartup {
        adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    let verified = VerifiedHeightContext {
        context: adapter.wire_context.clone(),
        proofs_of_possession: adapter.proofs_of_possession.clone(),
        parent_verification: adapter.parent_verification.clone(),
    };
    let RecoveredWalStartupAuthorityV1::DecisionFetch(fetch) = authority else {
        panic!("Decision body cut requires exact Fetch authority")
    };
    let projection =
        crate::sumeragi::v2_runtime::project_recovered_wal_decision_fetch(&verified, fetch)
            .unwrap_or_else(|_| panic!("project exact Decision body cut"));
    let mut body_store = body_store
        .into_revalidated_startup()
        .expect("seal semantically revalidated Decision body store");
    assert!(body_store.matches_context(verified.context()));
    let mut foreign_context = verified.context().clone();
    foreign_context.height = foreign_context
        .height
        .checked_add(1)
        .expect("fixture height advances");
    assert!(!body_store.matches_context(&foreign_context));
    let identity = body_store.instance_identity();
    for attempt in 0..2 {
        let body = body_store
            .detach_recovered_decision_apply_body(&projection)
            .unwrap_or_else(|error| panic!("detach Decision body attempt {attempt}: {error}"));
        assert!(body.exactly_matches_decision(&projection));
        assert!(body.exactly_matches_store_for_test(&identity, verified.context()));
        assert!(
            body.prepare_replay_lineage(&verified, &projection)
                .is_some(),
            "the same-store cut derives one closed Decision body replay lineage"
        );
        drop(body);
    }
    let body = body_store
        .detach_recovered_decision_apply_body(&projection)
        .expect("detach Decision body for the closed adapter preview");
    let replay = body
        .prepare_replay_lineage(&verified, &projection)
        .expect("derive the exact Decision body replay lineage");
    let preview = body
        .into_adapter_preview(adapter, &verified, projection, replay)
        .unwrap_or_else(|error| panic!("stage exact Decision body preview: {}", error.reason()));
    assert!(
        preview.validates_for_test(),
        "the closed composite retains exact Fetch, body, replay, effects, and pending lineage"
    );
    drop(preview);
    let reopened = reopen_authenticated_decision_startup(
        &safety,
        verified.context(),
        verified.proofs_of_possession.clone(),
        0xD5,
    )
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _)| panic!("reauthenticate restored Decision body: {error}"));
    let AuthenticatedRecoveredAdapterStartup {
        adapter: restored_adapter,
        effects: restored_effects,
        authority: restored_authority,
        validation_authority: _,
        factory_owner: _,
    } = reopened;
    assert!(restored_effects.is_empty());
    let RecoveredWalStartupAuthorityV1::DecisionFetch(restored_fetch) = restored_authority else {
        panic!("restored Decision body retains its exact Fetch authority")
    };
    let restored_projection = crate::sumeragi::v2_runtime::project_recovered_wal_decision_fetch(
        &verified,
        restored_fetch,
    )
    .unwrap_or_else(|_| panic!("project restored Decision body Fetch"));
    let restored_body = body_store
        .detach_recovered_decision_apply_body(&restored_projection)
        .expect("the dropped composite restores its exact in-memory body cut");
    assert!(restored_body.exactly_matches_decision(&restored_projection));
    drop(restored_body);
    drop(restored_adapter);
    let frames_after = std::fs::read_dir(&context_directory)
        .expect("reread Decision body-store context")
        .map(|entry| {
            let path = entry
                .expect("reread Decision body-store frame entry")
                .path();
            let bytes = std::fs::read(&path).expect("reread Decision body-store frame");
            (
                path.file_name().expect("frame has a file name").to_owned(),
                bytes,
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(frames_after, frames_before);
}
#[cfg(feature = "bls")]
#[test]
fn bls_unified_decision_body_publishes_apply_or_rejects_before_storage_open() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic Decision body-preflight retainer");
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("temporary validated Decision WAL");
    let storage = TempDir::new().expect("temporary validated Decision stores");
    let (startup, body_store) = write_decision_startup_with_body_marker(
        &safety,
        &storage.path().join("body"),
        0xD6,
        DecisionBodyMarkerFixture::Validated,
    );
    let context = startup.adapter.wire_context.clone();
    let proofs = startup.adapter.proofs_of_possession.clone();
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate Decision Apply startup: {error}"));
    let body_store = body_store
        .into_revalidated_startup()
        .expect("seal validated Decision body outcome");
    let ledger_root = storage.path().join("ledger");
    let serve_root = storage.path().join("serve");
    let mut owner = authenticated
        .open_production_lifecycle_owner_v1_with_store_for_test(
            &lifecycle_owner_config(),
            4,
            &ledger_root,
            &serve_root,
            body_store,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("publish recovered Decision Apply: {error}"));
    let (row_count, apply_ordinal) = owner
        .recovered_decision_apply_summary_for_test()
        .expect("owner retains the exact four-row Decision Apply chain");
    assert_eq!(row_count, 4);
    assert!(apply_ordinal > 0);
    assert_eq!(
                owner.plan_direct_registry_turn(),
                Err(
                    super::super::v2_lifecycle_coordinator::ProductionSchedulerInputsError::IoCapacityObservationRequired {
                        ordinal: apply_ordinal,
                    },
                ),
                "the exact recovered Apply is classified before a lease can be claimed"
            );
    assert_eq!(
        owner.recovered_decision_apply_summary_for_test(),
        Some((4, apply_ordinal)),
        "capacity classification leaves the coordinator and registry unchanged"
    );
    assert!(ledger_root.join("lifecycle-ledger-v1.norito").exists());
    assert!(serve_root.exists());
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "durable owner recovery must not publish status before live launch and ingress activation"
    );
    drop(owner);
    let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
    let first_frame = std::fs::read(&ledger_path).expect("read Decision Apply ledger");
    #[cfg(unix)]
    let first_inode = {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(&ledger_path)
            .expect("inspect Decision Apply ledger")
            .ino()
    };
    crate::sumeragi::status::clear_v2_status();
    let mut body_store = super::super::v2_body_store::V2BodyStore::open(
        storage.path().join("body"),
        context.clone(),
    )
    .expect("reopen Decision Apply body store");
    body_store
        .revalidate_recovered_markers(|_| Ok::<_, String>(execution_commitment(0xD6)))
        .expect("semantically revalidate Decision Apply marker");
    let body_store = body_store
        .into_revalidated_startup()
        .expect("reseal Decision Apply body store");
    let reopened = reopen_authenticated_decision_startup(&safety, &context, proofs, 0xD6)
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("reauthenticate Decision Apply: {error}"));
    let mut owner = reopened
        .open_production_lifecycle_owner_v1_with_store_for_test(
            &lifecycle_owner_config(),
            4,
            &ledger_root,
            &serve_root,
            body_store,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("coalesce recovered Decision Apply: {error}"));
    assert_eq!(
        owner.recovered_decision_apply_summary_for_test(),
        Some((4, apply_ordinal))
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("reread Decision Apply ledger"),
        first_frame,
        "exact Decision Apply coalesce preserves the complete frame"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        assert_eq!(
            std::fs::metadata(&ledger_path)
                .expect("inspect coalesced Decision Apply ledger")
                .ino(),
            first_inode,
            "exact Decision Apply coalesce validates without replacing the inode"
        );
    }
    drop(owner);
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("temporary rejected Decision WAL");
    let storage = TempDir::new().expect("temporary rejected Decision stores");
    let (startup, body_store) = write_decision_startup_with_body_marker(
        &safety,
        &storage.path().join("body"),
        0xD7,
        DecisionBodyMarkerFixture::Rejected,
    );
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate rejected Decision: {error}"));
    let body_store = body_store
        .into_revalidated_startup()
        .expect("seal rejected Decision body outcome");
    let ledger_root = storage.path().join("ledger");
    let serve_root = storage.path().join("serve");
    let Err(error) = authenticated.open_production_lifecycle_owner_v1_with_store_for_test(
        &lifecycle_owner_config(),
        4,
        &ledger_root,
        &serve_root,
        body_store,
        &local_signer,
    ) else {
        panic!("a deterministically rejected Decision body cannot publish Apply")
    };
    assert!(
        error
            .to_string()
            .contains("recovered Decision body was deterministically rejected")
    );
    assert!(!ledger_root.exists());
    assert!(!serve_root.exists());
    assert!(crate::sumeragi::status::v2_status().is_none());
}
#[cfg(feature = "bls")]
#[test]
fn bls_decision_fetch_same_key_drift_fails_without_rewrite() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let mutations: [(&str, fn(&std::path::Path, LifecycleContext) -> bool); 2] = [
        ("owner", substitute_recovered_decision_fetch_owner_for_test),
        (
            "replay authority",
            substitute_recovered_decision_fetch_replay_authority_for_test,
        ),
    ];
    for (index, (label, mutate)) in mutations.into_iter().enumerate() {
        crate::sumeragi::status::clear_v2_status();
        let safety = TempDir::new().expect("temporary Decision drift WAL");
        let storage = TempDir::new().expect("temporary Decision drift stores");
        let marker = 0xD8_u8
            .checked_add(u8::try_from(index).expect("bounded drift fixture index"))
            .expect("bounded drift fixture marker");
        let (startup, context, proofs) = write_authenticated_decision_startup(&safety, marker);
        let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
            .expect("deterministic Decision drift retainer");
        let ledger_root = storage.path().join("ledger");
        let serve_root = storage.path().join("serve");
        let body_root = storage.path().join("body");
        let owner = startup
            .authenticate_final_wal_startup_authority()
            .unwrap_or_else(|(error, _)| {
                panic!("authenticate initial Decision drift startup: {error}")
            })
            .open_production_lifecycle_owner_v1_from_roots_for_test(
                &lifecycle_owner_config(),
                4,
                &ledger_root,
                &serve_root,
                &body_root,
                super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
                &local_signer,
            )
            .unwrap_or_else(|error| panic!("open initial Decision drift owner: {error}"));
        drop(owner);
        assert!(
            mutate(
                &ledger_root,
                super::super::v2_lifecycle_coordinator::lifecycle_context(&context),
            ),
            "construct structurally valid {label} drift"
        );
        let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
        let drifted = std::fs::read(&ledger_path).expect("read drifted Decision ledger");
        crate::sumeragi::status::clear_v2_status();
        let reopened = reopen_authenticated_decision_startup(&safety, &context, proofs, marker)
            .authenticate_final_wal_startup_authority()
            .unwrap_or_else(|(error, _)| {
                panic!("reauthenticate Decision {label} drift startup: {error}")
            });
        assert!(
            reopened
                .open_production_lifecycle_owner_v1_from_roots_for_test(
                    &lifecycle_owner_config(),
                    4,
                    &ledger_root,
                    &serve_root,
                    &body_root,
                    super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
                    &local_signer,
                )
                .is_err(),
            "same-key Decision {label} drift must fail closed"
        );
        assert_eq!(
            std::fs::read(&ledger_path).expect("reread drifted Decision ledger"),
            drifted,
            "failed Decision {label} recovery must not rewrite the incumbent"
        );
    }
    crate::sumeragi::status::clear_v2_status();
}
#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn recovered_signature_fifo_uses_latest_exact_owner_before_terminal_wal_frame() {
    let directory = TempDir::new().expect("temporary Proposal FIFO WAL");
    let (context, keys, proofs) = authenticated_context();
    let round_zero = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let round_one = wire::ConsensusRound {
        view: 1,
        ..round_zero
    };
    let local = context.leader(round_one.view);
    let subject = subject(0xC6);
    let commitment = execution_commitment(0xC6);
    let mut old_prepare = wire::QuorumCertificate {
        round: round_zero,
        proposal_round: round_zero,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut old_prepare, &keys);
    let timeout = authenticated_timeout_certificate(
        round_zero,
        Some(old_prepare.clone()),
        vec![0, 1, 2],
        &keys,
    );
    let chunks = wire::encode_payload_chunks(context.da_layout, b"recovered signature fifo")
        .expect("encode FIFO proposal payload");
    let manifest = wire::PayloadManifest::derive(
        &context,
        round_one,
        subject,
        u64::try_from(b"recovered signature fifo".len()).expect("small FIFO payload"),
        &chunks,
    )
    .expect("derive FIFO proposal manifest");
    let proposal = wire::Proposal {
        round: round_one,
        proposer: local,
        subject,
        manifest,
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: timeout.clone(),
            highest_prepare_qc: Some(old_prepare.clone()),
        }),
        signature: Vec::new(),
    };
    let prepare_vote = wire::Vote {
        round: round_one,
        proposal_round: round_one,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signer: local,
        signature: Vec::new(),
    };
    let mut current_prepare = wire::QuorumCertificate {
        round: round_one,
        proposal_round: round_one,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut current_prepare, &keys);
    let current_commit = wire::Vote {
        phase: wire::GlobalPhase::Commit,
        ..prepare_vote.clone()
    };
    let old_commit = wire::Vote {
        round: round_zero,
        proposal_round: round_zero,
        phase: wire::GlobalPhase::Commit,
        ..prepare_vote.clone()
    };
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xC6; 32],
        vec![
            WalRecordV2::LockAndCommit {
                prepare: old_prepare,
                vote: old_commit,
            },
            WalRecordV2::InstallTimeout(timeout),
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::PrepareIntent(prepare_vote.clone()),
            WalRecordV2::PrepareIntent(prepare_vote.clone()),
            WalRecordV2::LockAndCommit {
                prepare: current_prepare.clone(),
                vote: current_commit.clone(),
            },
            WalRecordV2::LockAndCommit {
                prepare: current_prepare.clone(),
                vote: current_commit.clone(),
            },
        ],
    );
    assert!(matches!(
        startup.effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(observed),
            ..
        }] if observed == &proposal
    ));
    assert_eq!(startup.adapter.reducer.queued_signatures().count(), 2);
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate FIFO Proposal owner: {error}"));
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated.authority else {
        panic!("the current FIFO head must be the Proposal control Sign")
    };
    let frames = authenticated.adapter.wal.recovered_records();
    assert!(control.wal_identity.exactly_matches_record(&frames[3]));
    assert!(
        !control
            .wal_identity
            .exactly_matches_record(frames.last().expect("terminal FIFO frame"))
    );
    let AuthenticatedRecoveredAdapterStartup {
        mut adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    drop(authority);
    let tag = adapter.current_tag();
    let mut after_proposal = adapter
        .signature_completed(tag, vec![0xA1; 96])
        .expect("complete recovered Proposal")
        .into_effects();
    let prepare_sign = take_current_sign(&mut after_proposal);
    assert!(matches!(
        prepare_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(ref vote),
            ..
        } if vote == &prepare_vote
    ));
    let mut current = vec![prepare_sign];
    let prepare_owner = adapter
        .authenticate_recovered_wal_vote_sign(&mut current)
        .expect("authenticate current recovered Prepare")
        .expect("Prepare has one WAL owner");
    assert!(current.is_empty());
    assert!(prepare_owner.exactly_matches_wal_record(&adapter.wal.recovered_records()[5]));
    let prepare_tag = prepare_owner.tag();
    drop(prepare_owner);
    let mut after_prepare = adapter
        .signature_completed(prepare_tag, vec![0xA2; 96])
        .expect("complete recovered Prepare")
        .into_effects();
    let commit_sign = take_current_sign(&mut after_prepare);
    assert!(matches!(
        commit_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(ref vote),
            ..
        } if vote == &current_commit
    ));
    let mut current = vec![commit_sign];
    let commit_owner = adapter
        .authenticate_recovered_wal_vote_sign(&mut current)
        .expect("authenticate current recovered Commit")
        .expect("Commit has one WAL owner");
    assert!(current.is_empty());
    assert!(commit_owner.exactly_matches_wal_record(&adapter.wal.recovered_records()[7]));
    assert_eq!(commit_owner.prepare_certificate(), Some(&current_prepare));
}
#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn recovered_current_timeout_then_historical_commit_keeps_intrinsic_vote_round() {
    let directory = TempDir::new().expect("temporary Timeout/Commit FIFO WAL");
    let (context, keys, proofs) = authenticated_context();
    let locked_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let current_round = wire::ConsensusRound {
        view: 1,
        ..locked_round
    };
    let local = 0;
    let subject = subject(0xC7);
    let commitment = execution_commitment(0xC7);
    let mut locked_prepare = wire::QuorumCertificate {
        round: locked_round,
        proposal_round: locked_round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut locked_prepare, &keys);
    let historical_commit = wire::Vote {
        round: locked_round,
        proposal_round: locked_round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: commitment,
        signer: local,
        signature: Vec::new(),
    };
    let installed_timeout =
        authenticated_timeout_certificate(locked_round, None, vec![0, 1, 2], &keys);
    let current_timeout = wire::TimeoutVote {
        round: current_round,
        highest_prepare_qc: Some(locked_prepare.clone()),
        signer: local,
        signature: Vec::new(),
    };
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xC7; 32],
        vec![
            WalRecordV2::LockAndCommit {
                prepare: locked_prepare.clone(),
                vote: historical_commit.clone(),
            },
            WalRecordV2::InstallTimeout(installed_timeout),
            WalRecordV2::TimeoutIntent(current_timeout.clone()),
        ],
    );
    assert!(matches!(
        startup.effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(observed),
            ..
        }] if observed == &current_timeout
    ));
    assert_eq!(startup.adapter.reducer.queued_signatures().count(), 1);
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate current Timeout owner: {error}"));
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated.authority else {
        panic!("the current FIFO head must be the Timeout control Sign")
    };
    assert!(
        control.wal_identity.exactly_matches_record(
            authenticated
                .adapter
                .wal
                .recovered_records()
                .last()
                .expect("final TimeoutIntent frame")
        )
    );
    let AuthenticatedRecoveredAdapterStartup {
        mut adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    drop(authority);
    let tag = adapter.current_tag();
    let mut after_timeout = adapter
        .signature_completed(tag, vec![0xB1; 96])
        .expect("complete current recovered Timeout")
        .into_effects();
    let commit_sign = take_current_sign(&mut after_timeout);
    assert!(matches!(
        commit_sign,
        AdapterEffect::Sign {
            tag: commit_tag,
            request: SignRequest::Vote(ref vote),
        } if commit_tag.view() == current_round.view
            && vote == &historical_commit
            && vote.round == locked_round
    ));
    let mut current = vec![commit_sign];
    let commit_owner = adapter
        .authenticate_recovered_wal_vote_sign(&mut current)
        .expect("authenticate historical recovered Commit")
        .expect("historical Commit has one WAL owner");
    assert!(current.is_empty());
    assert!(commit_owner.exactly_matches_wal_record(&adapter.wal.recovered_records()[0]));
    assert_eq!(commit_owner.tag().view(), current_round.view);
    assert_eq!(commit_owner.vote().round, locked_round);
    assert_eq!(commit_owner.prepare_certificate(), Some(&locked_prepare));
}
#[cfg(feature = "bls")]
#[test]
fn recovered_decision_fetch_classifier_authenticates_exact_absent_manifest_and_sources() {
    let exact_directory = TempDir::new().expect("temporary exact Decision Fetch WAL");
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let decision_subject = subject(0xC8);
    let mut decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: decision_subject,
        execution_commitment: execution_commitment(0xC8),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut decision, &keys);
    let startup = write_and_reopen_authenticated_wal_startup(
        &exact_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let exact_effect = startup
        .effects
        .first()
        .expect("Decision replays one exact Fetch")
        .clone();
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate exact Decision Fetch: {error}"));
    assert!(authenticated.effects.is_empty());
    assert!(matches!(
        authenticated.authority,
        RecoveredWalStartupAuthorityV1::DecisionFetch(_)
    ));
    let pending_directory = TempDir::new().expect("temporary pending Kura Decision Fetch WAL");
    let expected_pending = crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
        context.id(),
        context.height,
        decision.subject.block_hash,
    );
    let pending = write_and_reopen_authenticated_wal_startup(
        &pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    )
    .bind_pending_kura_apply(expected_pending)
    .unwrap_or_else(|(error, _)| panic!("bind exact pending Kura tip: {error}"))
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|error| panic!("authenticate pending Kura Decision Fetch: {error}"));
    assert!(pending.is_storage_only_for_test());
    assert_eq!(pending.expected_for_test(), expected_pending);

    let mismatched_pending_directory =
        TempDir::new().expect("temporary mismatched pending Kura Decision Fetch WAL");
    let mismatched_pending = crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
        context.id(),
        context.height,
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign pending Kura block")),
    );
    let Err(mismatched) = write_and_reopen_authenticated_wal_startup(
        &mismatched_pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    )
    .bind_pending_kura_apply(mismatched_pending)
    .unwrap_or_else(|(error, _)| panic!("bind same-height pending Kura tip: {error}"))
    .authenticate_final_wal_startup_authority() else {
        panic!("a same-height foreign Kura block must fail before owner launch")
    };
    assert!(matches!(
        mismatched,
        AdapterError::RecoveredPendingKuraApplyMismatch
    ));
    let mut runtime_startup = pending.into_runtime_startup_for_test();
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        pending_kura_apply,
        leader_wire_launch_prepared,
        ..
    } = &mut runtime_startup.state
    else {
        panic!("pending Kura startup must remain a recovered adapter")
    };
    assert!(pending_kura_apply.is_some());
    *leader_wire_launch_prepared = true;
    let (mut runtime, prepared, local_proposal_attempt) = runtime_startup
        .into_serialized_runtime(
            Instant::now(),
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("move pending Kura Fetch and its lifecycle sidecar into runtime");
    assert!(local_proposal_attempt.is_none());
    let prepared = prepared.expect("runtime returns one opaque pending Kura replay seal");
    assert_eq!(prepared.expected_for_test(), expected_pending);
    assert!(prepared.is_exact_for_test());
    assert_eq!(
        runtime
            .take_effect_ownership(1)
            .expect("runtime retains the exact pending Kura lifecycle sidecar")
            .len(),
        1
    );
    drop(prepared);
    drop(runtime);

    let install_pending_directory =
        TempDir::new().expect("temporary pending Kura install-failure WAL");
    let install_pending = write_and_reopen_authenticated_wal_startup(
        &install_pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    )
    .bind_pending_kura_apply(expected_pending)
    .unwrap_or_else(|(error, _)| panic!("bind pending Kura install fixture: {error}"))
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|error| panic!("authenticate pending Kura install fixture: {error}"));
    assert!(install_pending.is_storage_only_for_test());
    let mut install_runtime_startup = install_pending.into_runtime_startup_for_test();
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        leader_wire_launch_prepared,
        ..
    } = &mut install_runtime_startup.state
    else {
        panic!("pending Kura install fixture must remain recovered")
    };
    *leader_wire_launch_prepared = true;
    let (runtime, prepared, local_proposal_attempt) = install_runtime_startup
        .into_serialized_runtime(
            Instant::now(),
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("move the install fixture into runtime");
    assert!(local_proposal_attempt.is_none());
    let prepared = prepared.expect("install fixture returns its pending replay seal");
    let mut executor = super::super::v2_effects::V2EffectExecutor::with_runtime(
        runtime,
        BTreeMap::new(),
        context.clone(),
        context.roster[0].validator.clone(),
        Some(0),
        super::super::v2_effects::EffectQueueConfig::default(),
    )
    .expect("open a pending Kura executor without the required recovered body");
    let (mut services, _planner_io) = super::super::v2_worker::tests::fixture();
    let install_result = prepared.install(&mut executor, &mut services);
    assert!(matches!(
        install_result,
        Err(super::super::v2_effects::EffectExecutorError::PendingApplyRecoveryMismatch(_))
    ));

    let empty_pending_directory =
        TempDir::new().expect("temporary pending Kura startup without a Decision");
    let empty_pending = write_and_reopen_authenticated_wal_startup(
        &empty_pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        Vec::new(),
    )
    .bind_pending_kura_apply(expected_pending)
    .unwrap_or_else(|(error, _)| panic!("bind empty pending Kura startup: {error}"))
    .authenticate_final_wal_startup_authority();
    let Err(empty_error) = empty_pending else {
        panic!("pending Kura startup without a Decision Fetch must fail closed")
    };
    assert!(matches!(
        empty_error,
        AdapterError::RecoveredPendingKuraApplyMismatch
    ));

    let foreign_pending_directory =
        TempDir::new().expect("temporary foreign pending Kura Decision Fetch WAL");
    let foreign_pending = crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
        context.id(),
        context.height + 1,
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign pending Kura block")),
    );
    let foreign_startup = write_and_reopen_authenticated_wal_startup(
        &foreign_pending_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let Err((error, retained)) = foreign_startup.bind_pending_kura_apply(foreign_pending) else {
        panic!("a foreign pending Kura height must not bind recovered startup")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredPendingKuraApplyMismatch
    ));
    assert_eq!(retained.effects.len(), 1);

    let manifest_directory = TempDir::new().expect("temporary mutated-manifest WAL");
    let mut retained = write_and_reopen_authenticated_wal_startup(
        &manifest_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let AdapterEffect::FetchBody { manifest, .. } = &mut retained.effects[0] else {
        panic!("Decision replay effect must remain FetchBody")
    };
    let chunks = wire::encode_payload_chunks(context.da_layout, b"forbidden guessed manifest")
        .expect("encode mutation payload");
    let guessed = wire::PayloadManifest::derive(
        &context,
        round,
        decision_subject,
        u64::try_from(b"forbidden guessed manifest".len()).expect("small mutation payload"),
        &chunks,
    )
    .expect("derive mutation manifest");
    *manifest = Some(guessed);
    let Err((error, _retained)) = retained.authenticate_final_wal_startup_authority() else {
        panic!("a guessed Decision manifest must fail exact classification")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredDecisionFetchMismatch
    ));
    let certificate_directory = TempDir::new().expect("temporary foreign-certificate WAL");
    let mut retained = write_and_reopen_authenticated_wal_startup(
        &certificate_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    let mut foreign_certificate = decision.clone();
    foreign_certificate.subject = subject(0xC9);
    foreign_certificate.execution_commitment = execution_commitment(0xC9);
    authenticate_qc(&mut foreign_certificate, &keys);
    let AdapterEffect::FetchBody {
        round: effect_round,
        subject: effect_subject,
        certificate,
        ..
    } = &mut retained.effects[0]
    else {
        panic!("Decision replay effect must remain FetchBody")
    };
    *effect_round = foreign_certificate.proposal_round;
    *effect_subject = foreign_certificate.subject;
    *certificate = Some(foreign_certificate);
    let Err((error, _retained)) = retained.authenticate_final_wal_startup_authority() else {
        panic!("a foreign Decision certificate must fail exact classification")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredDecisionFetchMismatch
    ));
    let sources_directory = TempDir::new().expect("temporary mutated-sources WAL");
    let mut retained = write_and_reopen_authenticated_wal_startup(
        &sources_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision.clone())],
    );
    retained.effects[0] = exact_effect;
    let AdapterEffect::FetchBody {
        certified_sources, ..
    } = &mut retained.effects[0]
    else {
        panic!("Decision replay effect must remain FetchBody")
    };
    certified_sources.pop();
    let Err((error, _retained)) = retained.authenticate_final_wal_startup_authority() else {
        panic!("a truncated Decision source roster must fail exact classification")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredDecisionFetchMismatch
    ));
    let locator_directory = TempDir::new().expect("temporary foreign-locator WAL");
    let authenticated = write_and_reopen_authenticated_wal_startup(
        &locator_directory,
        &context,
        &proofs,
        0,
        [0xC8; 32],
        vec![WalRecordV2::Decision(decision)],
    )
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _)| panic!("authenticate locator fixture: {error}"));
    let AuthenticatedRecoveredAdapterStartup {
        adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    let RecoveredWalStartupAuthorityV1::DecisionFetch(mut fetch) = authority else {
        panic!("locator fixture must retain Decision Fetch")
    };
    fetch.wal_identity = RecoveredWalFrameIdentity::for_test(0, 1, [0xE8; 32]);
    let verified = VerifiedHeightContext {
        context: adapter.wire_context.clone(),
        proofs_of_possession: adapter.proofs_of_possession.clone(),
        parent_verification: adapter.parent_verification.clone(),
    };
    assert!(
        crate::sumeragi::v2_runtime::project_recovered_wal_decision_fetch(&verified, fetch)
            .is_err(),
        "a substituted exact-shaped WAL locator must not project Decision Fetch authority"
    );
}
#[test]
fn bls_control_classifier_rejects_action_tag_extra_and_dual_residuals_pre_store() {
    let proposal_safety = TempDir::new().expect("temporary ProposalIntent classifier WAL");
    let timeout_safety = TempDir::new().expect("temporary TimeoutIntent classifier WAL");
    persist_proposal_intent_for_control_recovery(&proposal_safety, 0xC3);
    persist_timeout_intent_for_control_recovery(&timeout_safety);
    let authenticated_proposal = open_recovered_leader_startup_test(&proposal_safety)
        .expect("open exact ProposalIntent startup for local-attempt ownership")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate exact ProposalIntent startup: {error}")
        });
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated_proposal.authority
    else {
        panic!("ProposalIntent must retain one exact control Sign")
    };
    let recovered_attempt = RecoveredLifecycleLocalProposalAttemptV1::from_control(control)
        .expect("ProposalIntent control Sign mints one opaque local-attempt owner");
    let directive = authenticated_proposal
        .adapter
        .local_proposal_directive()
        .expect("read exact recovered Proposal directive");
    assert!(recovered_attempt.exactly_matches_directive(directive));
    drop(authenticated_proposal);

    let authenticated_timeout = open_recovered_startup_test(&timeout_safety)
        .expect("open exact TimeoutIntent startup for local-attempt exclusion")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate exact TimeoutIntent startup: {error}")
        });
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &authenticated_timeout.authority
    else {
        panic!("TimeoutIntent must retain one exact control Sign")
    };
    assert!(RecoveredLifecycleLocalProposalAttemptV1::from_control(control).is_none());
    drop(authenticated_timeout);

    let mut proposal =
        open_recovered_leader_startup_test(&proposal_safety).expect("open ProposalIntent startup");
    let mut timeout =
        open_recovered_startup_test(&timeout_safety).expect("open TimeoutIntent startup");
    let proposal_effect = proposal.effects.pop().expect("one Proposal control effect");
    let timeout_effect = timeout.effects.pop().expect("one Timeout control effect");
    proposal.effects.push(timeout_effect);
    timeout.effects.push(proposal_effect);
    for swapped in [proposal, timeout] {
        let Err((error, retained)) = swapped.authenticate_final_wal_startup_authority() else {
            panic!("Proposal/Timeout owner-frame action swaps must fail")
        };
        assert!(matches!(error, AdapterError::RecoveredControlSignMismatch));
        assert_eq!(retained.effects.len(), 1);
    }
    let mut wrong_tag = open_recovered_leader_startup_test(&proposal_safety)
        .expect("reopen ProposalIntent for tag mutation");
    let AdapterEffect::Sign { tag, .. } = &mut wrong_tag.effects[0] else {
        unreachable!("ProposalIntent replays one Sign")
    };
    *tag = reducer::EventTag::new(
        tag.height(),
        tag.view(),
        reducer::Generation::new(tag.generation().get().saturating_add(1)),
    );
    let Err((error, retained)) = wrong_tag.authenticate_final_wal_startup_authority() else {
        panic!("mutated recovered control tag must fail")
    };
    assert!(matches!(error, AdapterError::RecoveredControlSignMismatch));
    assert_eq!(retained.effects.len(), 1);
    let mut extra = open_recovered_leader_startup_test(&proposal_safety)
        .expect("reopen ProposalIntent for residual mutation");
    let duplicate = extra.effects[0].clone();
    extra.effects.push(duplicate);
    let Err((error, retained)) = extra.authenticate_final_wal_startup_authority() else {
        panic!("an extra residual effect must fail before authority removal")
    };
    assert!(matches!(error, AdapterError::RecoveredControlSignMismatch));
    assert_eq!(retained.effects.len(), 2);
    let phase_safety = TempDir::new().expect("temporary phase/control exclusivity WAL");
    let (mut phase, _vote, _proposal, _manifest, _validated) =
        reopen_with_prepare_intent(&phase_safety, 0xC4);
    let control = open_recovered_startup_test(&timeout_safety)
        .expect("reopen TimeoutIntent control for dual residual")
        .effects
        .into_iter()
        .next()
        .expect("one Timeout control effect");
    phase.effects.push(control);
    let Err((error, retained)) = phase.authenticate_final_wal_startup_authority() else {
        panic!("phase-vote and control authority must be mutually exclusive")
    };
    assert!(matches!(error, AdapterError::RecoveredVoteSignAmbiguous));
    assert_eq!(retained.effects.len(), 2);
    let unopened = TempDir::new().expect("unopened classification store root");
    assert!(!unopened.path().join("ledger").exists());
    assert!(!unopened.path().join("serve").exists());
}
#[test]
fn bls_foreign_control_replay_row_is_never_repaired_or_published() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary foreign control replay WAL");
    let storage = TempDir::new().expect("temporary foreign control lifecycle stores");
    persist_timeout_intent_for_control_recovery(&safety);
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, false));
    crate::sumeragi::status::clear_v2_status();
    let wire_context = context();
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(wire_context.id().0.as_ref());
    assert!(substitute_recovered_control_replay_authority_for_test(
        &storage.path().join("ledger"),
        LifecycleContext::new(LifecycleDigest::new(context_id), wire_context.height),
    ));
    let ledger_path = storage.path().join("ledger/lifecycle-ledger-v1.norito");
    let foreign_frame = std::fs::read(&ledger_path).expect("read foreign replay frame");
    let authenticated = open_recovered_startup_test(&safety)
        .expect("reopen foreign replay control startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("WAL authority itself remains exact: {error}"));
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic BLS control-startup signer");
    assert!(
        authenticated
            .open_production_lifecycle_owner_v1_from_roots_for_test(
                &lifecycle_owner_config(),
                4,
                &storage.path().join("ledger"),
                &storage.path().join("serve"),
                &storage.path().join("body"),
                super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
                &local_signer,
            )
            .is_err(),
        "a structurally valid foreign replay row must never be repaired in place"
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("reread rejected foreign replay frame"),
        foreign_frame,
        "rejection performs no LedgerV1 fsync"
    );
    assert!(crate::sumeragi::status::v2_status().is_none());
}
#[test]
fn bls_same_owner_foreign_terminal_control_row_is_rejected_without_rewrite() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary same-owner control WAL");
    let storage = TempDir::new().expect("temporary same-owner lifecycle stores");
    persist_timeout_intent_for_control_recovery(&safety);
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, false));
    crate::sumeragi::status::clear_v2_status();
    let wire_context = context();
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(wire_context.id().0.as_ref());
    assert!(append_same_owner_foreign_terminal_for_test(
        &storage.path().join("ledger"),
        LifecycleContext::new(LifecycleDigest::new(context_id), wire_context.height),
    ));
    let ledger_path = storage.path().join("ledger/lifecycle-ledger-v1.norito");
    let foreign_frame = std::fs::read(&ledger_path).expect("read same-owner foreign frame");
    let authenticated = open_recovered_startup_test(&safety)
        .expect("reopen same-owner control startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("WAL authority itself remains exact: {error}"));
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic same-owner control signer");
    assert!(
        authenticated
            .open_production_lifecycle_owner_v1_from_roots_for_test(
                &lifecycle_owner_config(),
                4,
                &storage.path().join("ledger"),
                &storage.path().join("serve"),
                &storage.path().join("body"),
                super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
                &local_signer,
            )
            .is_err(),
        "a foreign terminal row cannot share the standalone control owner"
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("reread rejected same-owner frame"),
        foreign_frame,
        "same-owner rejection performs no LedgerV1 rewrite"
    );
    assert!(crate::sumeragi::status::v2_status().is_none());
}
#[test]
fn bls_mutated_control_frame_identity_fails_before_serve_or_ledger_open() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary mutated control frame WAL");
    let storage = TempDir::new().expect("temporary unopened control stores");
    persist_timeout_intent_for_control_recovery(&safety);
    crate::sumeragi::status::clear_v2_status();
    let mut authenticated = open_recovered_startup_test(&safety)
        .expect("open exact TimeoutIntent before identity mutation")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate exact frame first: {error}"));
    let (frame_sequence, mut foreign_hash) = {
        let frame = authenticated
            .adapter
            .wal
            .recovered_records()
            .last()
            .expect("retained TimeoutIntent frame");
        (frame.sequence(), frame.frame_hash())
    };
    foreign_hash[0] ^= 1;
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = &mut authenticated.authority else {
        panic!("TimeoutIntent owns one control Sign")
    };
    control.wal_identity = RecoveredWalFrameIdentity::for_test(
        frame_sequence,
        frame_sequence
            .checked_add(1)
            .expect("fixture persistence id"),
        foreign_hash,
    );
    let body_root = storage.path().join("body");
    let body_store = super::super::v2_body_store::V2BodyStore::open(
        &body_root,
        authenticated.adapter.wire_context.clone(),
    )
    .expect("open the runner-owned empty body store");
    let body_entries_before = std::fs::read_dir(&body_root)
        .expect("read preflight body directory")
        .count();
    let body_store = body_store
        .into_revalidated_startup()
        .expect("seal the empty runner-owned body store");
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic BLS control-startup signer");
    assert!(
        authenticated
            .open_production_lifecycle_owner_v1_with_store_for_test(
                &lifecycle_owner_config(),
                4,
                &storage.path().join("ledger"),
                &storage.path().join("serve"),
                body_store,
                &local_signer,
            )
            .is_err()
    );
    assert!(body_root.exists());
    assert_eq!(
        std::fs::read_dir(&body_root)
            .expect("reread unchanged preflight body directory")
            .count(),
        body_entries_before
    );
    assert!(!storage.path().join("ledger").exists());
    assert!(!storage.path().join("serve").exists());
    assert!(crate::sumeragi::status::v2_status().is_none());
}
#[test]
fn recovered_wal_first_release_source_is_closed_and_store_ordered() {
    let adapter = crate::sumeragi::v2_lifecycle_coordinator::reviewed_v2_adapter_source_for_test();
    let body_store_source = include_str!("../v2_body_store.rs");
    let runtime = crate::sumeragi::v2_lifecycle_coordinator::reviewed_v2_runtime_source_for_test();
    let replay = concat!(
        include_str!("../v2_lifecycle_replay_authority.rs"),
        include_str!("../v2_lifecycle_replay_authority_certified_body.rs"),
    );
    let wal_recovery = include_str!("../v2_lifecycle_wal_recovery.rs");
    let ledger = reviewed_lifecycle_ledger_source_for_test();
    let registry = reviewed_lifecycle_work_registry_source_for_test();
    let factory_start = adapter
        .find("pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(")
        .expect("locate unified lifecycle owner factory");
    let factory_tail = &adapter[factory_start..];
    let factory_end = factory_tail
        .find("fn open_production_lifecycle_owner_v1_from_roots_for_test(")
        .expect("locate test-only root adapter after the production factory");
    let factory = &factory_tail[..factory_end];
    let canonical_factory_end = factory
        .find("fn open_production_lifecycle_owner_v1_at_authenticated_roots(")
        .expect("locate the private authenticated-root implementation");
    let canonical_factory = &factory[..canonical_factory_end];
    let factory_inputs = canonical_factory
        .find("factory_inputs: RecoveredLifecycleOwnerFactoryInputsV1")
        .expect("consume the adapter-bound execution/storage seal");
    assert!(canonical_factory.contains("body_store: super::v2_body_store::QuarantinedV2BodyStore"));
    assert!(!canonical_factory.contains("body_store: super::v2_body_store::V2BodyStore"));
    assert!(
        !canonical_factory.contains("body_store: super::v2_body_store::RevalidatedV2BodyStore")
    );
    let residual = canonical_factory
        .find("if !self.effects.is_empty()")
        .expect("reject residual effects before marker replay");
    let startup_binding = canonical_factory
        .find("Arc::ptr_eq(&adapter_owner, &self.factory_owner)")
        .expect("bind inputs to the exact authenticated startup");
    let context_binding = canonical_factory
        .find("storage.context_id != context.id()")
        .expect("bind the storage authority to the recovered context");
    let body_root = canonical_factory
        .find("body_store.matches_lifecycle_storage_root(")
        .expect("bind the body store to the sealed root and policy");
    let wal_path = canonical_factory
        .find("self.adapter.wal.matches_path(&storage.wal_path)")
        .expect("bind the adapter to the recovery-sealed WAL path");
    let apply_service = canonical_factory
        .find("let apply_service = super::v2_apply::V2ApplyService::new(")
        .expect("construct one exact replay/live Apply service");
    let replay_markers = canonical_factory
        .find(".into_revalidated_lifecycle_startup(")
        .expect("consume the fixed marker replay cut");
    let sealed_parts = canonical_factory
        .find("let RecoveredLifecycleStorageAuthorityV1 {")
        .expect("open the storage seal only after exact validation");
    let authenticated_roots = canonical_factory
        .find("self.open_production_lifecycle_owner_v1_at_authenticated_roots(")
        .expect("enter the private implementation after exact target checks");
    let kura_binding = canonical_factory
        .find("owner.with_recovered_kura_binding_and_apply_service(")
        .expect("retain exact Kura and replay service together");
    assert!(factory_inputs < residual);
    assert!(residual < startup_binding);
    assert!(startup_binding < context_binding);
    assert!(context_binding < body_root);
    assert!(body_root < wal_path);
    assert!(wal_path < apply_service);
    assert!(apply_service < replay_markers);
    assert!(replay_markers < sealed_parts);
    assert!(sealed_parts < authenticated_roots);
    assert!(authenticated_roots < kura_binding);
    let quarantine = body_store_source
        .split_once("impl QuarantinedV2BodyStore {")
        .expect("locate quarantined recovered-startup cut")
        .1
        .split_once("impl RevalidatedV2BodyStore {")
        .expect("locate end of quarantined recovered-startup cut")
        .0;
    assert!(quarantine.contains("fn into_revalidated_lifecycle_startup("));
    assert!(!quarantine.contains("fn retain_recovered_markers_for_subject("));
    assert!(!quarantine.contains("fn retain_recovered_markers_for_authority("));
    assert!(!quarantine.contains("fn revalidate_recovered_markers<"));
    assert!(!quarantine.contains("fn into_revalidated_startup("));
    let finality = quarantine
        .find("apply_service.recovered_finality_subject(context)")
        .expect("derive recovered-finality authority inside fixed replay");
    let subject_filter = quarantine
        .find(".retain_recovered_markers_for_subject(subject)")
        .expect("filter markers to recovered finality first");
    let authority_filter = quarantine
        .find(".retain_recovered_markers_for_authority(validation_authority)")
        .expect("then filter markers to WAL authority");
    let semantic_replay = quarantine
        .find(".revalidate_recovered_markers(|body|")
        .expect("semantically replay retained markers");
    let seal_markers = quarantine
        .find("self.0.into_revalidated_startup()")
        .expect("seal only replayed marker state");
    assert!(finality < subject_filter);
    assert!(subject_filter < authority_filter);
    assert!(authority_filter < semantic_replay);
    assert!(semantic_replay < seal_markers);
    for forbidden in [
        "kura: &Kura",
        "ledger_root: &std::path::Path",
        "serve_payload_root: &std::path::Path",
        "body_root: &std::path::Path",
        "body_signature_policy:",
    ] {
        assert!(!canonical_factory.contains(forbidden));
    }
    let projection = factory
        .find("project_recovered_wal_control_sign")
        .expect("control projection is in the unified factory");
    let decision_projection = factory
        .find("project_recovered_wal_decision_fetch")
        .expect("Decision Fetch projection is in the unified factory");
    let decision_body_preflight = factory
        .find("detach_recovered_decision_apply_body")
        .expect("locate the opaque Decision body preflight");
    let decision_adapter_preview = factory
        .find(".into_adapter_preview(adapter, verified, fetch, lineage)")
        .expect("locate the consuming recovered Decision adapter preview");
    let body_handoff = factory
        .find("into_lifecycle_owner_store")
        .expect("locate the revalidated same-store handoff");
    let serve_open = factory
        .find("CertifiedServePayloadStoreV1::open")
        .expect("locate Serve store open");
    let control_open = factory
        .find("ProductionLifecycleOwnerV1::open_recovered_control_startup")
        .expect("locate control owner open");
    let decision_open = factory
        .find("ProductionLifecycleOwnerV1::open_recovered_decision_fetch_startup")
        .expect("locate Decision Fetch owner open");
    assert!(projection < body_handoff);
    assert!(decision_projection < decision_body_preflight);
    assert!(decision_body_preflight < decision_adapter_preview);
    assert!(decision_adapter_preview < body_handoff);
    assert!(body_handoff < serve_open);
    assert!(serve_open < control_open);
    assert!(serve_open < decision_open);
    assert!(!factory.contains("publish_recovered_adapter_status"));
    assert!(factory[..projection].contains("if !self.effects.is_empty()"));
    let decision_apply_open = factory
        .find("ProductionLifecycleOwnerV1::open_recovered_decision_apply_startup")
        .expect("validated Decision body enters the exact Apply owner transaction");
    assert!(decision_adapter_preview < decision_apply_open);
    assert!(!factory.contains("restart-closed Decision Apply publication is not implemented"));
    for forbidden in ["V2BodyStore::open_with_policy", "body_root:"] {
        assert!(
            !factory.contains(forbidden),
            "production owner factory retained a second root-open surface {forbidden}"
        );
    }
    let control_token = adapter
        .split_once("pub(crate) struct RecoveredWalControlSign")
        .expect("locate opaque control token")
        .1
        .split_once("impl RecoveredWalVoteSign")
        .expect("locate end of control token surface")
        .0;
    for forbidden in [
        "#[derive(Clone)]",
        "fn candidate(",
        "fn effect(",
        "fn installed_effect(",
        "fn pending(",
        "fn locator(",
        "fn into_parts(",
        "fn bytes(",
        "fn ordinal(",
        "RuntimeLifecycleOrdinalSource",
    ] {
        assert!(
            !control_token.contains(forbidden),
            "forbidden control surface: {forbidden}"
        );
    }
    let control_classifier = adapter
        .split_once("fn authenticate_recovered_wal_control_sign(")
        .expect("locate exact control classifier")
        .1
        .split_once("fn authenticate_recovered_wal_decision_fetch(")
        .expect("locate end of exact control classifier")
        .0;
    for required in [
        "WalRecordV2::ProposalIntent(candidate) if candidate == proposal",
        "reducer::SignableMessage::Proposal(awaiting)",
        "SignRequest::Proposal(proposal)",
        "WalRecordV2::TimeoutIntent(candidate) if candidate == vote",
        "reducer::SignableMessage::TimeoutVote(awaiting)",
        "SignRequest::TimeoutVote(vote)",
        "self.wal.recovered_records().iter().rev()",
        "self.authenticate_recovered_wal_frame(frame)",
        "body_store.has_exact_recovered_decision_fetch_parent(&fetch)",
    ] {
        assert!(
            control_classifier.contains(required),
            "missing exact residual mapping: {required}"
        );
    }
    let decision_classifier = adapter
        .split_once("fn authenticate_recovered_wal_decision_fetch(")
        .expect("locate exact Decision Fetch classifier")
        .1
        .split_once("pub(crate) fn replayed_decision_key(")
        .expect("locate end of exact Decision Fetch classifier")
        .0;
    for required in [
        "manifest: None",
        "certified_sources: expected_sources",
        "certificate: Some(certificate.clone())",
        "self.wal.recovered_records().iter().rev()",
        "self.authenticate_recovered_wal_frame(frame)",
        "WalRecordV2::Decision(candidate) if candidate == certificate",
        "startup_effects.as_slice() != [expected.clone()]",
    ] {
        assert!(
            decision_classifier.contains(required),
            "missing exact Decision Fetch mapping: {required}"
        );
    }
    let exhaustive_classifier = adapter
        .split_once("pub(crate) fn authenticate_final_wal_startup_authority(")
        .expect("locate exhaustive startup-authority classifier")
        .1
        .split_once("impl AuthenticatedRecoveredAdapterStartup")
        .expect("locate end of exhaustive startup-authority classifier")
        .0;
    let validation_frontier = exhaustive_classifier
        .find("recovered_validation_authority(&self.effects)")
        .expect("the marker frontier is sealed before the effect is removed");
    let frontier = exhaustive_classifier
        .find("authenticate_recovered_wal_frontier")
        .expect("terminal WAL frontier is authenticated before classification");
    let phase = exhaustive_classifier
        .find("authenticate_recovered_wal_vote_sign")
        .expect("phase vote is classified first");
    let control = exhaustive_classifier
        .find("authenticate_recovered_wal_control_sign")
        .expect("control Sign is classified only without a phase vote");
    let decision_fetch = exhaustive_classifier
        .find("authenticate_recovered_wal_decision_fetch")
        .expect("Decision Fetch is classified only without a Sign");
    assert!(
        frontier < validation_frontier
            && validation_frontier < phase
            && phase < control
            && control < decision_fetch
    );
    for required in [
        "validation_authority,",
        "RecoveredWalStartupAuthorityV1::PhaseVote(recovered_vote)",
        "RecoveredWalStartupAuthorityV1::ControlSign(control)",
        "RecoveredWalStartupAuthorityV1::DecisionFetch(fetch)",
        "debug_assert!(self.effects.is_empty())",
    ] {
        assert!(
            exhaustive_classifier.contains(required),
            "missing exclusive startup-authority branch: {required}"
        );
    }
    let runtime_control = runtime
        .split_once("pub(in crate::sumeragi) fn project_recovered_wal_control_sign(")
        .expect("locate runtime control projection")
        .1
        .split_once("/// Ownership-preserving failure")
        .expect("locate end of runtime control projection")
        .0;
    assert!(!runtime_control.contains("RuntimeLifecycleOrdinalSource"));
    assert!(!runtime_control.contains("CandidateAdmission::new"));
    let authority = replay
        .split_once("fn exact_recovered_wal_control_authority(")
        .expect("locate exact control replay authority")
        .1
        .split_once("fn exact_recovered_wal_vote_authority(")
        .expect("locate end of exact control replay authority")
        .0;
    for required in [
        "SignRequest::Proposal(proposal)",
        "ReplayWalRoleV1::PROPOSAL_INTENT",
        "LifecycleStageKind::SignProposal",
        "SignRequest::TimeoutVote(vote)",
        "ReplayWalRoleV1::TIMEOUT_INTENT",
        "LifecycleStageKind::SignTimeoutVote",
    ] {
        assert!(
            authority.contains(required),
            "missing exact control mapping: {required}"
        );
    }
    let durable = ledger
        .split_once("pub(in crate::sumeragi) fn open_recovered_control_startup(")
        .expect("locate recovered control storage transaction")
        .1
        .split_once("/// Bind one paired recovered-WAL open")
        .expect("locate end of recovered control storage transaction")
        .0;
    let control_stage = durable
        .find("stage_authenticated_wal_control_sign")
        .expect("locate recovered control durable staging");
    let control_persist = durable[control_stage..]
        .find("persist_exact_successor")
        .map(|offset| control_stage + offset)
        .expect("locate recovered control durable publication");
    let control_registry = durable[control_persist..]
        .find("LifecycleWorkRegistryHolder::empty")
        .map(|offset| control_persist + offset)
        .expect("locate recovered control registry construction");
    assert!(durable.contains("if changed"));
    assert!(
        control_stage < control_persist && control_persist < control_registry,
        "control staging and durable publication precede volatile registry construction"
    );
    assert!(!durable.contains("RuntimeLifecycleOrdinalSource"));
    assert!(!durable.contains("publish_recovered_adapter_status"));
    let decision_durable = durable
        .split_once("pub(in crate::sumeragi) fn open_recovered_decision_fetch_startup(")
        .expect("locate recovered Decision Fetch storage transaction")
        .1;
    let body_parent_check = decision_durable
        .find("has_exact_recovered_decision_fetch_parent")
        .expect("validated body is checked before Decision Fetch repair");
    let quarantined_body_parent_check = decision_durable
        .find("has_quarantined_recovered_decision_fetch_parent")
        .expect("quarantined body is checked before Decision Fetch repair");
    let rejected_body_check = decision_durable
        .find("has_rejected_recovered_decision_body")
        .expect("rejected body is checked before Decision Fetch repair");
    let decision_ledger_open = decision_durable
        .find("LifecycleLedgerStoreV1::open")
        .expect("locate Decision Fetch LedgerV1 open");
    let decision_stage = decision_durable
        .find("stage_authenticated_wal_decision_fetch")
        .expect("locate Decision Fetch durable staging");
    let decision_registry = decision_durable
        .find("install_recovered_wal_decision_fetch")
        .expect("locate Decision Fetch carrier installation");
    assert!(
        body_parent_check < decision_ledger_open
            && quarantined_body_parent_check < decision_ledger_open
            && rejected_body_check < decision_ledger_open
    );
    assert!(decision_ledger_open < decision_stage && decision_stage < decision_registry);
    assert!(decision_durable.contains("if changed"));
    assert!(decision_durable.contains("persist_exact_successor"));
    for required in [
        "enum RecoveredWalRegistrySlotV1",
        "PhaseVote(ConcreteWorkAddress)",
        "ControlSign(ConcreteWorkAddress)",
        "DecisionFetch(ConcreteWorkAddress)",
        "DurableRecoveredWalControlSign",
        "DurableRecoveredWalDecisionFetch",
        "exactly_covers_recovered_ready_work_and_wal_authority",
    ] {
        assert!(
            registry.contains(required),
            "missing complete registry cut: {required}"
        );
    }
    for required in [
        "struct AuthenticatedRecoveredWalControlProjection",
        "struct DurableRecoveredWalControlSignCarrierV1",
        "struct AuthenticatedRecoveredWalDecisionFetchProjection",
        "struct DurableRecoveredWalDecisionFetchCarrierV1",
        "there is no parts API",
    ] {
        assert!(
            wal_recovery.contains(required),
            "missing sealed WAL cut: {required}"
        );
    }
    for required in [
        "RecoveredWalDecisionFetch",
        "fn authenticate_recovered_wal_decision_fetch(",
        "startup_effects.as_slice() != [expected.clone()]",
        "return Err((AdapterError::RecoveredStartupEffectMismatch, self))",
    ] {
        assert!(
            adapter.contains(required),
            "missing closed Decision startup rule: {required}"
        );
    }
    assert!(!adapter.contains("classify_unsupported_recovered_startup"));
    assert!(!adapter.contains("RecoveredDecisionApplyRequiresDurablePredecessor"));
    for required in [
        "WalReplayActionV1::FetchDecision",
        "certified_sources[..index].contains(source)",
    ] {
        assert!(
            replay.contains(required),
            "missing exact Decision Fetch replay source: {required}"
        );
    }
}
