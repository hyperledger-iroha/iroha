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
    let sign =
        settle_ready_validate_succeeded_for_test(&mut adapter, tag, round, subject, &validated);
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
        .exact_pending_adapter_effect_binding(&store)
        .expect("bind ordinary Store predecessor");
    let validate_pending = store_pending
        .project_store_validate_successor(&store, &validate)
        .expect("project ordinary Validate predecessor");
    (validate, validate_pending)
}
fn settle_ready_validate_succeeded_for_test(
    adapter: &mut SumeragiV2Adapter,
    tag: reducer::EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    validated: &ValidatedBodyReceipt,
) -> Vec<AdapterEffect> {
    let preview = adapter
        .prepare_direct_validation_succeeded(tag, round, subject, validated)
        .expect("prepare the exact sealed successful-validation transition");
    let sealed = SealedReadyDurableValidateAdapterPreview(match preview {
        DirectValidationSucceededPreparation::Busy(prepared) => {
            ReadyDurableValidateAdapterPreviewKind::ValidatedBusy(prepared)
        }
        DirectValidationSucceededPreparation::Inactive(prepared) => {
            ReadyDurableValidateAdapterPreviewKind::ValidatedInactive(prepared)
        }
        DirectValidationSucceededPreparation::NoEffect(prepared) => {
            ReadyDurableValidateAdapterPreviewKind::ValidatedNoEffect(prepared)
        }
        DirectValidationSucceededPreparation::Apply(prepared) => {
            ReadyDurableValidateAdapterPreviewKind::ValidatedApply(prepared)
        }
        DirectValidationSucceededPreparation::Persist(prepared) => {
            ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(prepared)
        }
    });
    let publication = sealed
        .preflight_publication()
        .expect("preflight the sealed successful-validation publication");
    match publication.kind() {
        ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
        | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect => {
            publication.commit_no_successor_after_durable_ledger();
            Vec::new()
        }
        ReadyDurableValidateAdapterPublicationKind::ValidatedPersist => {
            let ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) =
                &publication.0
            else {
                unreachable!("Persist discriminator retains one exact publication")
            };
            let sign = prepared.sign_effect.clone();
            let (validate, validate_pending) = ordinary_validate_predecessor_for_test(
                tag,
                round,
                subject,
                u128::from(round.view).saturating_add(90_001),
            );
            let bound = publication
                .bind_validate_sign_predecessor(ReadyValidateSignPredecessorAuthority::for_test(
                    &validate,
                    &validate_pending,
                ))
                .unwrap_or_else(|_| panic!("bind the exact sealed Validate predecessor"));
            let persisted = bound
                .append_live_wal()
                .unwrap_or_else(|_| panic!("append and fsync the exact Validate WAL frame"));
            Box::new(persisted).commit_after_test_durable_ledger_settlement();
            vec![sign]
        }
        ReadyDurableValidateAdapterPublicationKind::ValidatedBusy => {
            panic!("adapter test settlement cannot bypass a live reducer fence")
        }
        ReadyDurableValidateAdapterPublicationKind::ValidatedApply => {
            panic!("adapter test settlement requires the dedicated Apply lifecycle fixture")
        }
        ReadyDurableValidateAdapterPublicationKind::RejectedBusy
        | ReadyDurableValidateAdapterPublicationKind::RejectedInactive
        | ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect
        | ReadyDurableValidateAdapterPublicationKind::RejectedReport => {
            unreachable!("successful validation cannot produce a rejected publication")
        }
    }
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
    let sign = settle_ready_validate_succeeded_for_test(
        &mut adapter,
        tag,
        manifest.round,
        manifest.subject,
        &validated,
    );
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
    let sign =
        settle_ready_validate_succeeded_for_test(&mut adapter, tag, round, subject, &validated);
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
    assert!(owner.exact_recovered_body_pipeline_join_for_test());
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
fn persist_timeout_broadcasts_and_successor_timeout_intent(
    directory: &TempDir,
) -> Vec<(wire::TimeoutVote, wire::TimeoutVote)> {
    persist_timeout_broadcast_count_and_successor_timeout_intent(directory, 1)
}

#[cfg(feature = "bls")]
fn persist_timeout_broadcast_count_and_successor_timeout_intent(
    directory: &TempDir,
    obsolete_count: usize,
) -> Vec<(wire::TimeoutVote, wire::TimeoutVote)> {
    assert!(obsolete_count > 0);
    let (mut adapter, startup) = open_test(directory).expect("open two-view timeout WAL");
    assert!(startup.is_empty());
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic timeout signer")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let mut obsolete = Vec::with_capacity(obsolete_count);
    for expected_view in 0..obsolete_count {
        let old_tag = adapter.current_tag();
        assert_eq!(
            old_tag.view(),
            u64::try_from(expected_view).expect("small requested timeout view")
        );
        let mut old_sign = adapter
            .timeout_elapsed(old_tag)
            .expect("persist the old-view TimeoutIntent")
            .into_effects();
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(unsigned),
        } = old_sign.remove(0)
        else {
            panic!("old TimeoutIntent owns one exact Sign")
        };
        assert!(old_sign.is_empty());
        let signature = Signature::new(keys[0].private_key(), &unsigned.signature_preimage())
            .payload()
            .to_vec();
        let completed = adapter
            .signature_completed(tag, signature)
            .expect("complete the old-view timeout signature")
            .into_effects();
        let signed = completed
            .iter()
            .find_map(|effect| match effect {
                AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::TimeoutVote(vote),
                    ..
                }) => Some(vote.clone()),
                _ => None,
            })
            .expect("old timeout completion emits its signed Broadcast");
        let timeout_certificate =
            authenticated_timeout_certificate(unsigned.round, None, vec![0, 1, 2], &keys);
        let _entered = adapter
            .receive_verified(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate),
            ))
            .expect("install the authenticated successor view");
        obsolete.push((unsigned, signed));
    }
    let current_tag = adapter.current_tag();
    assert_eq!(
        current_tag.view(),
        u64::try_from(obsolete_count).expect("small requested timeout count")
    );
    let current = adapter
        .timeout_elapsed(current_tag)
        .expect("persist the successor-view TimeoutIntent");
    assert!(matches!(
        current.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(vote),
            ..
        }] if vote.round.view == current_tag.view()
    ));
    obsolete
}

#[cfg(feature = "bls")]
fn lifecycle_context_for_control_test() -> LifecycleContext {
    let wire_context = context();
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(wire_context.id().0.as_ref());
    LifecycleContext::new(LifecycleDigest::new(context_id), wire_context.height)
}

#[cfg(feature = "bls")]
fn signed_timeout_pair(round: wire::ConsensusRound) -> (wire::TimeoutVote, wire::TimeoutVote) {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic timeout signer")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let unsigned = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: 0,
        signature: Vec::new(),
    };
    let mut signed = unsigned.clone();
    signed.signature = Signature::new(keys[0].private_key(), &unsigned.signature_preimage())
        .payload()
        .to_vec();
    (unsigned, signed)
}

#[cfg(feature = "bls")]
fn assert_control_owner_rejected_without_rewrite(
    safety: &TempDir,
    storage: &TempDir,
    proposal: bool,
    expected_frame: &[u8],
) -> String {
    crate::sumeragi::status::clear_v2_status();
    let startup = if proposal {
        open_recovered_leader_startup_test(safety)
    } else {
        open_recovered_startup_test(safety)
    }
    .expect("reopen rejected recovered control startup");
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("WAL control authority remains exact: {error}"));
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic rejected control signer");
    let error = match authenticated.open_production_lifecycle_owner_v1_from_roots_for_test(
        &lifecycle_owner_config(),
        4,
        &storage.path().join("ledger"),
        &storage.path().join("serve"),
        &storage.path().join("body"),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
        &local_signer,
    ) {
        Ok(_owner) => panic!("unsupported control-neighbor row must fail closed"),
        Err(error) => error,
    };
    assert_eq!(
        std::fs::read(storage.path().join("ledger/lifecycle-ledger-v1.norito"))
            .expect("reread rejected control frame"),
        expected_frame,
        "rejected supersession cannot rewrite the lifecycle frame"
    );
    assert!(crate::sumeragi::status::v2_status().is_none());
    error.to_string()
}

#[cfg(feature = "bls")]
#[test]
fn obsolete_timeout_broadcast_is_atomically_cancelled_before_current_control_recovery() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary superseded timeout WAL");
    let storage = TempDir::new().expect("temporary superseded timeout lifecycle stores");
    let obsolete = persist_timeout_broadcasts_and_successor_timeout_intent(&safety);
    crate::sumeragi::status::clear_v2_status();
    let first_owner = open_control_owner_for_test(&safety, &storage, false);
    assert!(
        !first_owner.has_timeout_supersession_successor_for_test(),
        "ordinary pre-incident owner-open cannot mint a timeout-supersession witness"
    );
    drop(first_owner);
    crate::sumeragi::status::clear_v2_status();
    let lifecycle_context = lifecycle_context_for_control_test();
    let ledger_root = storage.path().join("ledger");
    assert!(install_timeout_broadcasts_before_current_control_for_test(
        &ledger_root,
        lifecycle_context,
        obsolete,
        true,
    ));
    let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
    let incident = std::fs::read(&ledger_path).expect("read exact three-row incident frame");

    let authenticated = open_recovered_startup_test(&safety)
        .expect("reopen exact current control startup")
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _)| panic!("authenticate current control WAL: {error}"));
    let verified = VerifiedHeightContext {
        context: authenticated.adapter.wire_context.clone(),
        proofs_of_possession: authenticated.adapter.proofs_of_possession.clone(),
        parent_verification: authenticated.adapter.parent_verification.clone(),
    };
    let AuthenticatedRecoveredAdapterStartup {
        adapter,
        effects,
        authority,
        validation_authority: _,
        factory_owner: _,
    } = authenticated;
    assert!(effects.is_empty());
    let RecoveredWalStartupAuthorityV1::ControlSign(control) = authority else {
        panic!("successor TimeoutIntent retains current control authority")
    };
    let projection =
        crate::sumeragi::v2_runtime::project_recovered_wal_control_sign(&verified, control)
            .unwrap_or_else(|_| panic!("project exact current control Sign"));
    assert!(control_timeout_supersession_persistence_failure_for_test(
        &ledger_root,
        lifecycle_context,
        &verified,
        &projection,
    ));
    assert_eq!(
        std::fs::read(&ledger_path).expect("reread publication-failure frame"),
        incident,
        "a failed exact publication cannot terminalize or stage either row"
    );
    drop(projection);
    drop(adapter);

    let repeated_owner = open_control_owner_for_test(&safety, &storage, false);
    assert!(
        repeated_owner.has_timeout_supersession_successor_for_test(),
        "the exact first cancellation CAS retains one move-only CompleteTip join witness"
    );
    drop(repeated_owner);
    crate::sumeragi::status::clear_v2_status();
    assert_eq!(
        control_timeout_supersession_summary_for_test(&ledger_root, lifecycle_context),
        Some((3, 1, 1)),
        "only the old timeout Broadcast is cancelled beside the incumbent current Sign"
    );
    let repaired = std::fs::read(&ledger_path).expect("read atomic supersession frame");
    assert_ne!(repaired, incident);
    #[cfg(unix)]
    let repaired_inode = {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(&ledger_path)
            .expect("inspect atomic supersession frame")
            .ino()
    };
    let owner = open_control_owner_for_test(&safety, &storage, false);
    assert!(
        !owner.has_timeout_supersession_successor_for_test(),
        "the byte-identical cold stutter cannot mint another successor witness"
    );
    drop(owner);
    crate::sumeragi::status::clear_v2_status();
    assert_eq!(
        std::fs::read(&ledger_path).expect("read repeated exact supersession frame"),
        repaired,
        "repeat recovery stutters without another cancellation or rewrite"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        assert_eq!(
            std::fs::metadata(&ledger_path)
                .expect("inspect repeated supersession frame")
                .ino(),
            repaired_inode,
            "repeat recovery skips a second lifecycle publication"
        );
    }
}

#[cfg(feature = "bls")]
#[test]
fn obsolete_timeout_broadcast_and_missing_current_sign_publish_one_successor() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary missing-current timeout WAL");
    let storage = TempDir::new().expect("temporary missing-current lifecycle stores");
    let obsolete = persist_timeout_broadcasts_and_successor_timeout_intent(&safety);
    crate::sumeragi::status::clear_v2_status();
    let initial_owner = open_control_owner_for_test(&safety, &storage, false);
    assert!(
        !initial_owner.has_timeout_supersession_successor_for_test(),
        "ordinary pre-incident owner-open cannot mint a timeout-supersession witness"
    );
    drop(initial_owner);
    crate::sumeragi::status::clear_v2_status();
    let lifecycle_context = lifecycle_context_for_control_test();
    let ledger_root = storage.path().join("ledger");
    assert!(install_timeout_broadcasts_before_current_control_for_test(
        &ledger_root,
        lifecycle_context,
        obsolete,
        false,
    ));
    let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
    let incident = std::fs::read(&ledger_path).expect("read timeout-only incident frame");
    let owner = open_control_owner_for_test(&safety, &storage, false);
    assert!(
        owner.has_timeout_supersession_successor_for_test(),
        "the atomic cancellation-plus-missing-Sign successor retains one exact join witness"
    );
    drop(owner);
    crate::sumeragi::status::clear_v2_status();
    assert_eq!(
        control_timeout_supersession_summary_for_test(&ledger_root, lifecycle_context),
        Some((3, 1, 1)),
        "one fsynced successor must both cancel the old Broadcast and stage current Sign"
    );
    assert_ne!(
        std::fs::read(&ledger_path).expect("read repaired missing-current frame"),
        incident
    );
}

#[cfg(feature = "bls")]
#[test]
fn same_view_timeout_broadcast_is_not_superseded_or_rewritten() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary same-view control WAL");
    let storage = TempDir::new().expect("temporary same-view lifecycle stores");
    persist_proposal_intent_for_control_recovery(&safety, 0xDA);
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, true));
    crate::sumeragi::status::clear_v2_status();
    let wire_context = context();
    let round = wire::ConsensusRound {
        context_id: wire_context.id(),
        height: wire_context.height,
        view: 0,
    };
    let lifecycle_context = lifecycle_context_for_control_test();
    assert!(install_timeout_broadcasts_before_current_control_for_test(
        &storage.path().join("ledger"),
        lifecycle_context,
        vec![signed_timeout_pair(round)],
        true,
    ));
    let frame = std::fs::read(storage.path().join("ledger/lifecycle-ledger-v1.norito"))
        .expect("read same-view timeout frame");
    let error = assert_control_owner_rejected_without_rewrite(&safety, &storage, true, &frame);
    assert!(error.contains("recovered control storage census assembly failed"));
}

#[cfg(feature = "bls")]
#[test]
fn foreign_timeout_signature_is_not_superseded_or_rewritten() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary foreign timeout WAL");
    let storage = TempDir::new().expect("temporary foreign timeout lifecycle stores");
    let mut obsolete = persist_timeout_broadcasts_and_successor_timeout_intent(&safety);
    obsolete[0].1.signature[0] ^= 0x01;
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, false));
    crate::sumeragi::status::clear_v2_status();
    let lifecycle_context = lifecycle_context_for_control_test();
    assert!(install_timeout_broadcasts_before_current_control_for_test(
        &storage.path().join("ledger"),
        lifecycle_context,
        obsolete,
        true,
    ));
    let frame = std::fs::read(storage.path().join("ledger/lifecycle-ledger-v1.norito"))
        .expect("read foreign timeout frame");
    let error = assert_control_owner_rejected_without_rewrite(&safety, &storage, false, &frame);
    assert!(error.contains("recovered control storage census assembly failed"));
}

#[cfg(feature = "bls")]
#[test]
fn multiple_obsolete_timeout_broadcasts_fail_before_publication() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary ambiguous timeout WAL");
    let storage = TempDir::new().expect("temporary ambiguous timeout lifecycle stores");
    let obsolete = persist_timeout_broadcast_count_and_successor_timeout_intent(&safety, 2);
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, false));
    crate::sumeragi::status::clear_v2_status();
    let lifecycle_context = lifecycle_context_for_control_test();
    assert!(install_timeout_broadcasts_before_current_control_for_test(
        &storage.path().join("ledger"),
        lifecycle_context,
        obsolete,
        true,
    ));
    let frame = std::fs::read(storage.path().join("ledger/lifecycle-ledger-v1.norito"))
        .expect("read ambiguous timeout frame");
    let error = assert_control_owner_rejected_without_rewrite(&safety, &storage, false, &frame);
    assert!(error.contains("recovered control timeout supersession invariant failed"));
}

#[cfg(feature = "bls")]
#[test]
fn non_timeout_broadcast_remains_owned_by_the_closed_census() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    let safety = TempDir::new().expect("temporary non-timeout control WAL");
    let storage = TempDir::new().expect("temporary non-timeout lifecycle stores");
    persist_timeout_intent_for_control_recovery(&safety);
    crate::sumeragi::status::clear_v2_status();
    drop(open_control_owner_for_test(&safety, &storage, false));
    crate::sumeragi::status::clear_v2_status();
    let lifecycle_context = lifecycle_context_for_control_test();
    assert!(
        install_non_timeout_broadcast_before_current_control_for_test(
            &storage.path().join("ledger"),
            lifecycle_context,
        )
    );
    let frame = std::fs::read(storage.path().join("ledger/lifecycle-ledger-v1.norito"))
        .expect("read non-timeout Broadcast frame");
    let error = assert_control_owner_rejected_without_rewrite(&safety, &storage, false, &frame);
    assert!(error.contains("recovered control storage census assembly failed"));
}

#[cfg(feature = "bls")]
#[test]
fn durable_current_round_proposal_survives_later_prepare_and_timeout_authority() {
    let directory = TempDir::new().expect("current-round ProposalIntent WAL");
    let (context, _keys, proofs) = authenticated_context();
    let local = context.leader(0);
    let wire::ConsensusMessageV2Payload::Proposal(mut proposal) =
        proposal(&context, local, subject(0xD1)).payload
    else {
        unreachable!("proposal fixture")
    };
    proposal.signature.clear();
    let prepare = wire::Vote {
        round: proposal.round,
        proposal_round: proposal.round,
        phase: wire::GlobalPhase::Prepare,
        subject: proposal.subject,
        execution_commitment: execution_commitment(0xD1),
        signer: local,
        signature: Vec::new(),
    };
    let timeout = wire::TimeoutVote {
        round: proposal.round,
        highest_prepare_qc: None,
        signer: local,
        signature: Vec::new(),
    };
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xD1; 32],
        vec![
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::PrepareIntent(prepare),
            WalRecordV2::TimeoutIntent(timeout),
        ],
    );
    let current_tag = startup.adapter.current_tag();
    let attempt =
        RecoveredLifecycleLocalProposalAttemptV1::from_authenticated_durable_current_round(
            &startup.adapter,
        )
        .expect("project authenticated ProposalIntent")
        .expect("later phase authority must not hide the ProposalIntent");
    assert!(
        attempt.exactly_matches_directive(
            startup
                .adapter
                .local_proposal_directive()
                .expect("read current proposal directive")
        )
    );
    assert!(
        startup
            .adapter
            .durable_current_round_local_proposal_is_closed()
    );
    let manifest = proposal.manifest.clone();
    let (mut runtime, _) = super::super::v2_runtime::SerializedV2Runtime::new(
        startup.adapter,
        startup.effects,
        Instant::now(),
        Duration::from_secs(10),
        super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap current-round durable authority");
    assert_eq!(
        runtime.local_proposal_admission_available(current_tag),
        Ok(false),
        "runner preflight must suppress before candidate consumption"
    );
    assert!(matches!(
        runtime.mint_local_proposal_effect_ownership(current_tag, &manifest),
        Err(reason) if reason.contains("durable local safety authority")
    ));
}
#[cfg(feature = "bls")]
#[test]
fn install_timeout_reopens_only_the_successor_view_producer() {
    let directory = TempDir::new().expect("old-view ProposalIntent WAL");
    let (context, keys, proofs) = authenticated_context();
    let local = context.leader(0);
    let wire::ConsensusMessageV2Payload::Proposal(mut proposal) =
        proposal(&context, local, subject(0xD2)).payload
    else {
        unreachable!("proposal fixture")
    };
    proposal.signature.clear();
    let timeout_vote = wire::TimeoutVote {
        round: proposal.round,
        highest_prepare_qc: None,
        signer: local,
        signature: Vec::new(),
    };
    let timeout = authenticated_timeout_certificate(proposal.round, None, vec![0, 1, 2], &keys);
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xD2; 32],
        vec![
            WalRecordV2::ProposalIntent(proposal),
            WalRecordV2::TimeoutIntent(timeout_vote),
            WalRecordV2::InstallTimeout(timeout),
        ],
    );
    let current_tag = startup.adapter.current_tag();
    assert_eq!(current_tag.view(), 1);
    assert!(
        RecoveredLifecycleLocalProposalAttemptV1::from_authenticated_durable_current_round(
            &startup.adapter,
        )
        .expect("project successor-view attempt")
        .is_none(),
        "old-view ProposalIntent must not suppress the successor view"
    );
    assert!(
        !startup
            .adapter
            .durable_current_round_local_proposal_is_closed()
    );
    let current_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: current_tag.view(),
    };
    let mut current_subject = subject(0xD3);
    current_subject.payload_hash = Hash::new(b"successor");
    let current_manifest = encode_payload(&context, current_round, current_subject, b"successor")
        .expect("encode successor-view candidate")
        .manifest()
        .clone();
    let (mut runtime, _) = super::super::v2_runtime::SerializedV2Runtime::new(
        startup.adapter,
        startup.effects,
        Instant::now(),
        Duration::from_secs(10),
        super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap successor-view adapter");
    assert_eq!(
        runtime.local_proposal_admission_available(current_tag),
        Ok(true)
    );
    let _ownership = runtime
        .mint_local_proposal_effect_ownership(current_tag, &current_manifest)
        .expect("successor view mints one fresh local Store owner");
}
#[cfg(feature = "bls")]
#[test]
fn durable_timeout_and_decision_close_direct_local_mint_without_proposal_attempt() {
    let (context, keys, proofs) = authenticated_context();
    let local = context.leader(0);
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let mut closed_subject = subject(0xD4);
    closed_subject.payload_hash = Hash::new(b"closed");
    let manifest = encode_payload(&context, round, closed_subject, b"closed")
        .expect("encode closed-round candidate")
        .manifest()
        .clone();
    let timeout_directory = TempDir::new().expect("current timeout WAL");
    let timeout_startup = write_and_reopen_authenticated_wal_startup(
        &timeout_directory,
        &context,
        &proofs,
        local,
        [0xD4; 32],
        vec![WalRecordV2::TimeoutIntent(wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: local,
            signature: Vec::new(),
        })],
    );
    assert!(
        RecoveredLifecycleLocalProposalAttemptV1::from_authenticated_durable_current_round(
            &timeout_startup.adapter,
        )
        .expect("project timeout-only attempt")
        .is_none(),
        "TimeoutIntent is closure authority, never Proposal-attempt authority"
    );
    assert!(
        timeout_startup
            .adapter
            .durable_current_round_local_proposal_is_closed()
    );
    let timeout_tag = timeout_startup.adapter.current_tag();
    let (mut timeout_runtime, _) = super::super::v2_runtime::SerializedV2Runtime::new(
        timeout_startup.adapter,
        timeout_startup.effects,
        Instant::now(),
        Duration::from_secs(10),
        super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap timeout-only adapter");
    assert_eq!(
        timeout_runtime.local_proposal_admission_available(timeout_tag),
        Ok(false)
    );
    assert!(
        timeout_runtime
            .mint_local_proposal_effect_ownership(timeout_tag, &manifest)
            .is_err()
    );

    let decision_directory = TempDir::new().expect("Decision WAL");
    let mut decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: execution_commitment(0xD4),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut decision, &keys);
    let decision_startup = write_and_reopen_authenticated_wal_startup(
        &decision_directory,
        &context,
        &proofs,
        local,
        [0xD5; 32],
        vec![WalRecordV2::Decision(decision)],
    );
    assert!(
        decision_startup
            .adapter
            .durable_current_round_local_proposal_is_closed()
    );
    let decision_tag = decision_startup.adapter.current_tag();
    let (mut decision_runtime, _) = super::super::v2_runtime::SerializedV2Runtime::new(
        decision_startup.adapter,
        decision_startup.effects,
        Instant::now(),
        Duration::from_secs(10),
        super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap decided adapter");
    assert_eq!(
        decision_runtime.local_proposal_admission_available(decision_tag),
        Ok(false)
    );
    assert!(
        decision_runtime
            .mint_local_proposal_effect_ownership(decision_tag, &manifest)
            .is_err()
    );
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
        let (mut executor, planner_io) = first.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            0,
            2,
        );
        super::super::v2_worker::tests::install_local_signer_for_test(&mut services, &keys[0]);
        assert_eq!(
            first
                .dispatch_completion_for_test(&mut services, &mut executor, 0)
                .expect("rank the genuine WAL-backed Sign beside recovered Fetch"),
            super::super::v2_lifecycle_coordinator::ProductionCompletionDispatchV1::SignQueued {
                ordinal: mixed_sign_ordinal,
            }
        );
        assert!(
            first.lifecycle_completion_selection_is_exact_for_test(
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
    let first_dispatch_projection;
    {
        let runtime_verified = VerifiedHeightContext::genesis(wire_context.clone(), proofs.clone())
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
            .bind_body_store_to_lifecycle_completion_io_for_test(
                &mut services,
                runtime,
                Arc::clone(&output_guard),
                0,
                2,
            );
        super::super::v2_worker::tests::install_local_signer_for_test(&mut services, &keys[0]);
        assert_eq!(
            reopened
                .dispatch_completion_for_test(&mut services, &mut executor, 0)
                .expect("dispatch the genuine WAL-backed recovered Fetch"),
            super::super::v2_lifecycle_coordinator::ProductionCompletionDispatchV1::FetchDispatched {
                ordinal: first_summary.0,
            }
        );
        assert!(
            services
                .has_pending_exact_output()
                .expect("inspect the recovered Fetch exact-output owner")
        );
        first_dispatch_projection = reopened
            .recovered_fetch_dispatch_projection_for_test(&executor, first_summary.0)
            .expect("published recovered Fetch parks its exact request owner externally");
        assert_eq!(first_dispatch_projection.2.observed_generation(), 0);
        let actor_admissions = Arc::new(Mutex::new(0_usize));
        let observed_admissions = Arc::clone(&actor_admissions);
        services.set_exact_output_admission_hook(move |_post, _ticket| {
            let mut admissions = observed_admissions
                .lock()
                .expect("count recovered Fetch actor admissions");
            *admissions = admissions.saturating_add(1);
            Ok(())
        });
        assert!(
            !services
                .retry_pending_exact_output()
                .expect("admit the initial recovered Fetch occurrence"),
            "actor admission releases the physical exact-output occurrence"
        );
        let initial_admissions = *actor_admissions
            .lock()
            .expect("read initial recovered Fetch actor admissions");
        assert!(initial_admissions > 0);
        let before_due = Instant::now();
        let mut next_attempt = before_due
            .checked_add(Duration::from_secs(1))
            .expect("construct a future recovered Fetch retry deadline");
        assert!(
            !super::super::v2_runner::retry_recovered_decision_fetch_if_due(
                before_due,
                &mut next_attempt,
                Duration::from_millis(25),
                &executor,
                &services,
            )
            .expect("a future recovered Fetch retry is a no-op")
        );
        assert_eq!(
            *actor_admissions
                .lock()
                .expect("read pre-deadline actor admissions"),
            initial_admissions
        );
        let due = next_attempt;
        assert!(
            super::super::v2_runner::retry_recovered_decision_fetch_if_due(
                due,
                &mut next_attempt,
                Duration::from_millis(25),
                &executor,
                &services,
            )
            .expect("recreate the actor-admitted recovered Fetch occurrence")
        );
        assert!(next_attempt > due);
        assert!(
            *actor_admissions
                .lock()
                .expect("read retransmitted recovered Fetch actor admissions")
                > initial_admissions,
            "the retained executor owner must recreate a remotely lost occurrence"
        );
        assert_eq!(
            reopened.recovered_fetch_dispatch_projection_for_test(&executor, first_summary.0,),
            Some(first_dispatch_projection),
            "retransmission must not replace the external wait or signed request owner"
        );
        let foreign_source = super::super::v2_lifecycle_coordinator::WaitSource::External(
            super::super::v2_lifecycle_coordinator::LifecycleDigest::new([0xEE; 32]),
        );
        assert_ne!(foreign_source, first_dispatch_projection.2.source());
        assert_eq!(
            reopened.replace_recovered_fetch_wait_source_for_test(first_summary.0, foreign_source,),
            Some(first_dispatch_projection.2.source())
        );
        assert!(
            reopened
                .recovered_fetch_dispatch_projection_for_test(&executor, first_summary.0)
                .is_none(),
            "a foreign registry wait source cannot authenticate the parked request owner"
        );
        assert_eq!(
            reopened.replace_recovered_fetch_wait_source_for_test(
                first_summary.0,
                first_dispatch_projection.2.source(),
            ),
            Some(foreign_source)
        );
        assert_eq!(
            reopened.recovered_fetch_dispatch_projection_for_test(&executor, first_summary.0),
            Some(first_dispatch_projection)
        );
        assert!(!output_guard.restart_required());
        output_guard.close_admission_for_restart();
        planner_io.detach(&mut services);
    }
    drop(reopened);
    crate::sumeragi::status::clear_v2_status();
    let mismatched_verified = VerifiedHeightContext::genesis(wire_context.clone(), proofs.clone())
        .expect("reverify the mismatched-generation Decision Fetch context");
    let mismatched = SumeragiV2Adapter::open_recovered_startup_with_aggregator(
        safety.path().join("authenticated-fifo-safety.wal"),
        mismatched_verified,
        Some(0),
        reducer::Generation::new(51),
        [0xCA; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("replay the Decision WAL under a mismatched process generation")
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _)| panic!("authenticate mismatched Decision Fetch: {error}"));
    let Err(error) = mismatched.open_production_lifecycle_owner_v1_from_roots_for_test(
        &lifecycle_owner_config(),
        4,
        &ledger_root,
        &serve_root,
        &body_root,
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
        &local_signer,
    ) else {
        panic!("a process-generation mismatch cannot coalesce a durable Decision Fetch")
    };
    assert!(
        error
            .to_string()
            .contains("neither an exact live Fetch nor an exact Store/Validate crash prefix"),
        "a live-Fetch mismatch must not be misclassified as a body-fsynced Store cut: {error}"
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("read generation-mismatched Decision Fetch LedgerV1"),
        first_frame,
        "a generation mismatch fails before rewriting the exact durable Fetch"
    );
    let verified = VerifiedHeightContext::genesis(wire_context.clone(), proofs.clone())
        .expect("reverify the externally parked Decision Fetch context");
    let cold = SumeragiV2Adapter::open_recovered_startup_with_aggregator(
        safety.path().join("authenticated-fifo-safety.wal"),
        verified,
        Some(0),
        reducer::Generation::new(50),
        [0xCA; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("cold-open the externally parked Decision Fetch")
    .authenticate_final_wal_startup_authority()
    .unwrap_or_else(|(error, _)| panic!("reauthenticate parked Decision Fetch: {error}"));
    let mut cold = cold
        .open_production_lifecycle_owner_v1_from_roots_for_test(
            &lifecycle_owner_config(),
            4,
            &ledger_root,
            &serve_root,
            &body_root,
            super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("reconstruct parked Decision Fetch: {error}"));
    assert_eq!(
        cold.recovered_decision_fetch_row_summary_for_test()
            .expect("cold open normalizes the volatile wait to exact Ready"),
        first_summary
    );
    assert_eq!(
        std::fs::read(&ledger_path).expect("read cold-opened Decision Fetch LedgerV1"),
        first_frame,
        "external Waiting and request ownership never rewrite the durable nonterminal row"
    );
    {
        let runtime_verified = VerifiedHeightContext::genesis(wire_context.clone(), proofs)
            .expect("verify the redispatched recovered Fetch executor context");
        let (adapter, startup) = SumeragiV2Adapter::open(
            storage
                .path()
                .join("decision-fetch-cold-redispatch-runtime.wal"),
            runtime_verified,
            Some(0),
            reducer::Generation::new(3),
            [0xCD; 32],
            fingerprints(),
            deferred_admission_ordinals(),
        )
        .expect("open the cold redispatch executor adapter");
        assert!(startup.is_empty());
        let runtime = super::super::v2_runtime::SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("wrap the cold redispatch executor adapter")
        .0;
        let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
        let (mut services, _) = super::super::v2_worker::tests::fixture();
        let (mut executor, planner_io) = cold.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            0,
            2,
        );
        super::super::v2_worker::tests::install_local_signer_for_test(&mut services, &keys[0]);
        assert_eq!(
            cold.dispatch_completion_for_test(&mut services, &mut executor, 0)
                .expect("redispatch the cold-opened recovered Fetch"),
            super::super::v2_lifecycle_coordinator::ProductionCompletionDispatchV1::FetchDispatched {
                ordinal: first_summary.0,
            }
        );
        let redispatch = cold
            .recovered_fetch_dispatch_projection_for_test(&executor, first_summary.0)
            .expect("cold redispatch reinstalls the exact external wait and owner");
        assert_eq!(redispatch, first_dispatch_projection);
        assert!(
            services
                .has_pending_exact_output()
                .expect("cold redispatch republishes the exact request fanout")
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
            DecisionBodyMarkerFixture::DurableOnly => {
                unreachable!("promoted-marker loop has no durable-only case")
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
fn run_unified_decision_body_test_on_stack() {
    let handle = std::thread::Builder::new()
        .name("sumeragi-v2-unified-decision-body".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(bls_unified_decision_body_publishes_apply_or_rejects_before_storage_open)
        .expect("spawn unified Decision-body recovery test");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}
#[cfg(feature = "bls")]
#[test]
fn bls_unified_decision_body_publishes_apply_or_rejects_before_storage_open() {
    if std::thread::current().name() != Some("sumeragi-v2-unified-decision-body") {
        return run_unified_decision_body_test_on_stack();
    }
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
fn bls_pending_kura_durable_body_without_validation_marker_fails_owner_open() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("temporary durable-only pending Decision WAL");
    let storage = TempDir::new().expect("temporary durable-only pending Decision stores");
    let (startup, body_store) = write_decision_startup_with_body_marker(
        &safety,
        &storage.path().join("body"),
        0xD8,
        DecisionBodyMarkerFixture::DurableOnly,
    );
    let context = startup.adapter.wire_context.clone();
    let (context_id, height, block_hash) = match startup.effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                ..
            },
        ] => (round.context_id, tag.height(), subject.block_hash),
        _ => panic!("durable Decision must replay one certified Fetch"),
    };
    let expected =
        crate::sumeragi::v2_recovery::PendingKuraApply::for_test(context_id, height, block_hash);
    let pending = startup
        .bind_pending_kura_apply(expected)
        .unwrap_or_else(|(error, _)| panic!("bind durable-only pending Decision: {error}"))
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|error| panic!("authenticate durable-only pending Decision: {error}"));
    assert!(pending.is_storage_only_for_test());
    assert_eq!(pending.expected_for_test(), expected);
    assert_eq!(
        body_store
            .recovery_catalog()
            .expect("read durable-only pending Decision catalog")
            .len(),
        1
    );
    assert!(body_store.validated_recovery_catalog().is_empty());
    drop(
        body_store
            .into_revalidated_startup()
            .expect("seal durable-only pending Decision store"),
    );
    let mut runtime_startup = pending.into_runtime_startup_for_test();
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        pending_kura_apply,
        leader_wire_launch_prepared,
        ..
    } = &mut runtime_startup.state
    else {
        panic!("durable-only pending Kura startup must remain recovered")
    };
    assert!(pending_kura_apply.is_some());
    *leader_wire_launch_prepared = true;
    let (runtime, prepared, local_proposal_attempt) = runtime_startup
        .into_serialized_runtime(
            Instant::now(),
            Duration::from_secs(10),
            super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
        .expect("move durable-only pending Kura startup into the runtime shell");
    assert!(local_proposal_attempt.is_none());
    let prepared = prepared.expect("runtime returns the opaque pending Kura replay seal");
    assert!(!prepared.validated_marker_was_deferred());
    let mut executor = super::super::v2_effects::V2EffectExecutor::with_runtime(
        runtime,
        std::collections::BTreeMap::new(),
        context.clone(),
        context.roster[0].validator.clone(),
        Some(0),
        super::super::v2_effects::EffectQueueConfig::default(),
    )
    .expect("open a durable-only pending Kura executor");
    let (mut services, _planner_io) = super::super::v2_worker::tests::fixture();
    let Err(super::super::v2_effects::EffectExecutorError::PendingApplyRecoveryMismatch(detail)) =
        prepared.install(&mut executor, &mut services)
    else {
        panic!("pending Kura replay without its deferred validation marker must fail closed")
    };
    assert_eq!(
        detail,
        "pending Kura replay omitted its exact deferred validation marker"
    );
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
    let recovery_authority = authenticated
        .leader_wire_recovery_authority()
        .expect("replay projects the exact durable lock into leader-wire recovery");
    let origin = context.roster[1].validator.clone();
    let phase = super::super::FairV2IngressLeaderWirePhase::CommitVote;
    let protected_commit = super::super::FairV2IngressLeaderWireToken {
        identity: super::super::FairV2IngressLeaderWireIdentity {
            context_id: context.id(),
            height: context.height,
            view: locked_round.view,
            subject_hash: Hash::new(subject.encode()),
            manifest_hash: None,
            phase,
            semantic_origin: origin.clone(),
            canonical_wire_hash: Hash::new(b"replayed historical Commit vote"),
        },
        slot: super::super::FairV2IngressLeaderWireSlot {
            semantic_origin: origin,
            phase,
            chunk_index: None,
        },
        admission_ordinal: 1,
        scheduler_ordinal: 1,
        source_class: super::super::FairV2IngressLeaderWireSourceClass::Control,
    };
    assert!(
        !recovery_authority.retires(&protected_commit),
        "startup recovery must preserve peer votes for its replayed durable lock"
    );
    let mut wrong_subject_commit = protected_commit.clone();
    wrong_subject_commit.identity.subject_hash = Hash::new(b"wrong replayed Commit subject");
    assert!(recovery_authority.retires(&wrong_subject_commit));
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
include!("v2_adapter_04_wal_recovery_decision_classifier_cases.rs");
