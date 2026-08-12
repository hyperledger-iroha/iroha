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

    fn join_recovered_prepare_startup<'registry>(
        startup: RecoveredAdapterStartup,
        proposal: wire::Proposal,
        manifest: wire::PayloadManifest,
        validated: ValidatedBodyReceipt,
        holder: &'registry mut super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder,
    ) -> AuthenticatedRecoveredWalLifecycleStartup<'registry> {
        let authenticated =
            startup
                .authenticate_final_wal_vote()
                .unwrap_or_else(|(error, _startup)| {
                    panic!("authenticate recovered Prepare WAL vote: {error}")
                });
        let authority = authenticated
            .recovered_vote
            .as_ref()
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

    fn expect_recovered_open_error<'registry>(
        result: Result<
            PublishedRecoveredWalLifecycleStartup<'registry>,
            RecoveredWalLifecycleOpenPublicationError<'registry>,
        >,
        message: &str,
    ) -> RecoveredWalLifecycleOpenPublicationError<'registry> {
        match result {
            Ok(_published) => panic!("{message}"),
            Err(error) => error,
        }
    }

    #[test]
    fn recovered_prepare_wal_vote_fsyncs_repair_and_installs_exact_sign() {
        let directory = TempDir::new().expect("temporary Prepare recovery directory");
        let (startup, expected_vote, proposal, manifest, validated) =
            reopen_with_prepare_intent(&directory, 0xD1);
        let authenticated = match startup.authenticate_final_wal_vote() {
            Ok(authenticated) => authenticated,
            Err((error, _startup)) => {
                panic!("authenticate the final recovered PrepareIntent: {error}")
            }
        };
        let authority = authenticated
            .recovered_vote
            .as_ref()
            .expect("PrepareIntent carries one restart vote");
        assert!(
            authenticated.effects.is_empty(),
            "the raw vote-sign effect is consumed"
        );
        assert!(authority.wal_identity().is_exact());
        assert!(authority.replay_evidence_is_exact());
        let wal_frame = authenticated
            .adapter
            .wal
            .recovered_records()
            .last()
            .expect("PrepareIntent WAL frame remains retained");
        assert!(authority.exactly_matches_wal_record(wal_frame));
        let mut foreign_hash = wal_frame.frame_hash();
        foreign_hash[0] ^= 1;
        assert!(
            !authority
                .wal_identity()
                .exactly_matches(RecoveredWalFrameIdentity::for_test(
                    wal_frame.sequence(),
                    wal_frame
                        .sequence()
                        .checked_add(1)
                        .expect("fixture sequence"),
                    foreign_hash,
                ))
        );
        assert!(
            RecoveredWalFrameIdentity::for_test(0, 1, [0; 32]).is_exact(),
            "cryptographic hash bytes have no reserved sentinel value"
        );
        assert_eq!(authority.tag(), authenticated.adapter.current_tag());
        assert_eq!(authority.vote(), &expected_vote);
        assert_eq!(authority.vote().round, authority.vote().proposal_round);
        assert_eq!(authority.vote().phase, wire::GlobalPhase::Prepare);
        assert_eq!(
            authority.vote().execution_commitment,
            expected_vote.execution_commitment
        );
        assert!(authority.prepare_certificate().is_none());
        let verified = VerifiedHeightContext {
            context: authenticated.adapter.wire_context.clone(),
            proofs_of_possession: authenticated.adapter.proofs_of_possession.clone(),
            parent_verification: authenticated.adapter.parent_verification.clone(),
        };
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let validate = holder.recovered_wal_validate_registry_cut_for_test(
            &verified, authority, proposal, manifest, validated,
        );
        let joined = authenticated
            .authenticate_recovered_validate(validate)
            .unwrap_or_else(|error| panic!("join recovered Prepare WAL vote: {}", error.reason()));
        assert!(joined.repair.concrete_pair_and_validation_are_exact());
        assert!(
            joined
                .repair
                .rejects_wrong_ledger_parent_bindings_for_test()
        );
        assert!(
            joined.repair.rejects_foreign_replay_authorities_for_test(),
            "structurally valid foreign replay origins must fail for both repaired rows"
        );

        let ledger_directory = TempDir::new().expect("temporary recovered Prepare ledger");
        let (summary, durable_startup) = joined
            .persist_repair_for_test(ledger_directory.path())
            .unwrap_or_else(|error| {
                panic!(
                    "fsync recovered Prepare lifecycle repair: {}",
                    error.reason()
                )
            });
        assert!(summary.first_changed());
        assert!(!summary.repeat_changed());
        assert!(summary.parent_advanced());
        assert!(summary.child_live());
        assert_eq!(summary.child_ordinal(), 2);
        assert!(summary.is_prepare_edge());
        assert!(!summary.is_commit_edge());
        assert_eq!(summary.high_water(), 2);
        assert!(summary.durable_frame_bound());
        assert!(summary.reopened_exact());
        assert!(
            durable_startup.remains_sealed_and_exact_for_test(ledger_directory.path()),
            "post-fsync startup must retain the adapter, empty unpublished batch, and vacant registry pair"
        );
        let installed = durable_startup
            .install_recovered_sign_for_test(ledger_directory.path())
            .unwrap_or_else(|error| {
                panic!(
                    "install exact recovered Prepare Sign child: {}",
                    error.reason()
                )
            });
        assert!(
            installed.exact_installed_shape_for_test(ledger_directory.path()),
            "the parent must stay absent while one same-owner child occupies the sole Effect slot at the durable ordinal and digest"
        );
        drop(installed);
        assert_eq!(
            holder.recovered_wal_sign_entry_count_for_test(),
            1,
            "dropping the exclusive installed cut releases only its borrow"
        );
    }

    #[test]
    fn recovered_prepare_outer_fsync_rejects_a_stale_opened_ledger_snapshot() {
        let directory = TempDir::new().expect("temporary stale Prepare recovery directory");
        let (startup, _expected_vote, proposal, manifest, validated) =
            reopen_with_prepare_intent(&directory, 0xD3);
        let authenticated = match startup.authenticate_final_wal_vote() {
            Ok(authenticated) => authenticated,
            Err((error, _startup)) => {
                panic!("authenticate stale recovered PrepareIntent: {error}")
            }
        };
        let authority = authenticated
            .recovered_vote
            .as_ref()
            .expect("PrepareIntent carries one stale-snapshot restart vote");
        let verified = VerifiedHeightContext {
            context: authenticated.adapter.wire_context.clone(),
            proofs_of_possession: authenticated.adapter.proofs_of_possession.clone(),
            parent_verification: authenticated.adapter.parent_verification.clone(),
        };
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let validate = holder.recovered_wal_validate_registry_cut_for_test(
            &verified, authority, proposal, manifest, validated,
        );
        let joined = authenticated
            .authenticate_recovered_validate(validate)
            .unwrap_or_else(|error| {
                panic!("join stale recovered Prepare WAL vote: {}", error.reason())
            });
        let ledger_directory = TempDir::new().expect("temporary stale Prepare ledger");
        let error = match joined.persist_stale_snapshot_for_test(ledger_directory.path()) {
            Ok(_durable) => panic!("a stale opened ledger snapshot must not fsync"),
            Err(error) => error,
        };
        assert_eq!(
            error.reason(),
            "recovered WAL ledger fsync did not complete authoritatively"
        );
        drop(error);
    }

    #[test]
    fn recovered_prepare_sign_install_rejects_wrong_store_before_registry_mutation() {
        let directory = TempDir::new().expect("temporary wrong-store Prepare recovery directory");
        let (startup, _expected_vote, proposal, manifest, validated) =
            reopen_with_prepare_intent(&directory, 0xD4);
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let joined =
            join_recovered_prepare_startup(startup, proposal, manifest, validated, &mut holder);
        let ledger_directory = TempDir::new().expect("exact recovered Prepare ledger");
        let (_summary, durable) = joined
            .persist_repair_for_test(ledger_directory.path())
            .unwrap_or_else(|error| panic!("fsync recovered Prepare repair: {}", error.reason()));
        let wrong_ledger_directory = TempDir::new().expect("foreign recovered Prepare ledger root");
        let error = match durable.install_recovered_sign_for_test(wrong_ledger_directory.path()) {
            Ok(_installed) => panic!("a foreign store frame must not install recovered Sign work"),
            Err(error) => error,
        };
        assert_eq!(
            error.reason(),
            "fsynced recovered WAL Sign child failed exact registry preflight"
        );
        assert!(
            error.remains_sealed_with_exact_vacancies_for_test(ledger_directory.path()),
            "the opaque error must retain the adapter, empty batch, exact receipt, and both vacant registry addresses"
        );
        drop(error);
        assert_eq!(
            holder.recovered_wal_sign_entry_count_for_test(),
            0,
            "preflight failure must not insert a recovered Sign row"
        );
    }

    #[test]
    fn recovered_prepare_restart_reenters_repaired_frame_and_installs_sign() {
        let directory = TempDir::new().expect("temporary re-entry Prepare recovery directory");
        let (startup, _expected_vote, proposal, manifest, validated) =
            reopen_with_prepare_intent(&directory, 0xD5);
        let replay_proposal = proposal.clone();
        let replay_manifest = manifest.clone();
        let replay_validated = validated.clone();
        let ledger_directory = TempDir::new().expect("re-entry recovered Prepare ledger");

        let mut first_holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let first_joined = join_recovered_prepare_startup(
            startup,
            proposal,
            manifest,
            validated,
            &mut first_holder,
        );
        let (first_summary, durable_before_crash) = first_joined
            .persist_repair_for_test(ledger_directory.path())
            .unwrap_or_else(|error| {
                panic!("fsync first recovered Prepare repair: {}", error.reason())
            });
        assert!(first_summary.first_changed());
        drop(durable_before_crash);
        assert_eq!(first_holder.recovered_wal_sign_entry_count_for_test(), 0);

        let restarted = open_recovered_startup_test(&directory)
            .expect("fresh startup replays the unchanged Prepare WAL frame");
        let mut restarted_holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let restarted_joined = join_recovered_prepare_startup(
            restarted,
            replay_proposal,
            replay_manifest,
            replay_validated,
            &mut restarted_holder,
        );
        let (changed, durable) = restarted_joined
            .persist_reopened_repair_for_test(ledger_directory.path())
            .unwrap_or_else(|error| {
                panic!(
                    "idempotently fsync reopened Prepare repair: {}",
                    error.reason()
                )
            });
        assert!(
            !changed,
            "the exact Advanced-parent/live-child pair must stutter on fresh startup"
        );
        let installed = durable
            .install_recovered_sign_for_test(ledger_directory.path())
            .unwrap_or_else(|error| {
                panic!(
                    "install recovered Prepare Sign after re-entry: {}",
                    error.reason()
                )
            });
        assert!(installed.exact_installed_shape_for_test(ledger_directory.path()));
        drop(installed);
        assert_eq!(
            restarted_holder.recovered_wal_sign_entry_count_for_test(),
            1,
            "fresh startup leaves one exact closed Sign child after releasing the borrow"
        );
    }

    #[test]
    fn recovered_prepare_opens_exact_coordinator_before_status_publication() {
        let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
        crate::sumeragi::status::clear_v2_status();
        let safety = TempDir::new().expect("temporary published Prepare recovery directory");
        let ledger = TempDir::new().expect("temporary published Prepare ledger");
        let payload = TempDir::new().expect("temporary published payload store");
        let body = TempDir::new().expect("temporary published body store");
        let mut holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let installed =
            install_recovered_prepare_startup(&safety, ledger.path(), 0xD6, &mut holder);
        let verified = verified_from_installed_startup(&installed);
        let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
            &verified,
            ledger.path(),
            payload.path(),
            body.path(),
        );
        assert!(
            installed
                .installed
                .seed_parent_recovery_for_test(&mut recovery)
        );
        crate::sumeragi::status::clear_v2_status();
        assert!(crate::sumeragi::status::v2_status().is_none());

        let published = installed
            .open_coordinator_and_publish_for_test(ledger.path(), &mut payload_store, recovery)
            .unwrap_or_else(|error| {
                panic!(
                    "open exact recovered coordinator before status: {}",
                    error.reason()
                )
            });
        assert!(published.exact_published_join_for_test());
        assert_eq!(
            crate::sumeragi::status::v2_status()
                .expect("status is published only after the exact join")
                .height,
            verified.context().height
        );
        drop(published);
        crate::sumeragi::status::clear_v2_status();
    }

    #[test]
    fn recovered_prepare_open_failures_retain_authority_and_publish_no_status() {
        let _status_guard = crate::sumeragi::status::rbc_status_test_guard();

        // A same-context cut with no exact parent or child is rejected before
        // coordinator preparation.
        crate::sumeragi::status::clear_v2_status();
        {
            let safety = TempDir::new().expect("missing-recovery safety directory");
            let ledger = TempDir::new().expect("missing-recovery ledger");
            let payload = TempDir::new().expect("missing-recovery payload store");
            let body = TempDir::new().expect("missing-recovery body store");
            let mut holder =
                super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
            let installed =
                install_recovered_prepare_startup(&safety, ledger.path(), 0xD7, &mut holder);
            let verified = verified_from_installed_startup(&installed);
            let (mut payload_store, recovery) = empty_authenticated_lifecycle_recovery(
                &verified,
                ledger.path(),
                payload.path(),
                body.path(),
            );
            crate::sumeragi::status::clear_v2_status();
            let error = expect_recovered_open_error(
                installed.open_coordinator_and_publish_for_test(
                    ledger.path(),
                    &mut payload_store,
                    recovery,
                ),
                "missing recovered parent/child must fail closed",
            );
            assert_eq!(
                error.reason(),
                "authenticated recovery lacks the exact recovered WAL handoff"
            );
            assert!(error.retains_exact_installed_for_test(ledger.path()));
            assert!(crate::sumeragi::status::v2_status().is_none());
        }

        // A cut from another authenticated height context cannot be spliced.
        crate::sumeragi::status::clear_v2_status();
        {
            let safety = TempDir::new().expect("foreign-recovery safety directory");
            let ledger = TempDir::new().expect("foreign-recovery ledger");
            let payload = TempDir::new().expect("foreign-recovery payload store");
            let body = TempDir::new().expect("foreign-recovery body store");
            let mut holder =
                super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
            let installed =
                install_recovered_prepare_startup(&safety, ledger.path(), 0xD8, &mut holder);
            let mut foreign_context = installed.adapter.wire_context.clone();
            foreign_context.leader_seed[0] ^= 0x5A;
            let foreign_verified = verified_genesis(foreign_context);
            let foreign_ledger = TempDir::new().expect("foreign-recovery authenticated ledger");
            let (mut payload_store, recovery) = empty_authenticated_lifecycle_recovery(
                &foreign_verified,
                foreign_ledger.path(),
                payload.path(),
                body.path(),
            );
            crate::sumeragi::status::clear_v2_status();
            let error = expect_recovered_open_error(
                installed.open_coordinator_and_publish_for_test(
                    ledger.path(),
                    &mut payload_store,
                    recovery,
                ),
                "foreign recovery context must fail closed",
            );
            assert_eq!(
                error.reason(),
                "authenticated recovery lacks the exact recovered WAL handoff"
            );
            assert!(error.retains_exact_installed_for_test(ledger.path()));
            assert!(crate::sumeragi::status::v2_status().is_none());
        }

        // Both exact sides are an ambiguous recovery shape and must be
        // preserved rather than normalized by overwriting either key.
        crate::sumeragi::status::clear_v2_status();
        {
            let safety = TempDir::new().expect("wrong-recovery safety directory");
            let ledger = TempDir::new().expect("wrong-recovery ledger");
            let payload = TempDir::new().expect("wrong-recovery payload store");
            let body = TempDir::new().expect("wrong-recovery body store");
            let mut holder =
                super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
            let installed =
                install_recovered_prepare_startup(&safety, ledger.path(), 0xD9, &mut holder);
            let verified = verified_from_installed_startup(&installed);
            let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
                &verified,
                ledger.path(),
                payload.path(),
                body.path(),
            );
            assert!(
                installed
                    .installed
                    .seed_both_recovery_for_test(&mut recovery)
            );
            crate::sumeragi::status::clear_v2_status();
            let error = expect_recovered_open_error(
                installed.open_coordinator_and_publish_for_test(
                    ledger.path(),
                    &mut payload_store,
                    recovery,
                ),
                "ambiguous exact parent/child recovery must fail closed",
            );
            assert_eq!(
                error.reason(),
                "authenticated recovery lacks the exact recovered WAL handoff"
            );
            assert!(error.retains_exact_installed_for_test(ledger.path()));
            assert!(crate::sumeragi::status::v2_status().is_none());
        }

        // A foreign ledger root fails during non-publishing preparation while
        // the exact receipt-bound installed row remains sealed.
        crate::sumeragi::status::clear_v2_status();
        {
            let safety = TempDir::new().expect("wrong-ledger safety directory");
            let ledger = TempDir::new().expect("wrong-ledger exact ledger");
            let wrong_ledger = TempDir::new().expect("wrong-ledger foreign root");
            let payload = TempDir::new().expect("wrong-ledger payload store");
            let body = TempDir::new().expect("wrong-ledger body store");
            let mut holder =
                super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
            let installed =
                install_recovered_prepare_startup(&safety, ledger.path(), 0xDA, &mut holder);
            let verified = verified_from_installed_startup(&installed);
            let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
                &verified,
                ledger.path(),
                payload.path(),
                body.path(),
            );
            assert!(
                installed
                    .installed
                    .seed_parent_recovery_for_test(&mut recovery)
            );
            crate::sumeragi::status::clear_v2_status();
            let error = expect_recovered_open_error(
                installed.open_coordinator_and_publish_for_test(
                    wrong_ledger.path(),
                    &mut payload_store,
                    recovery,
                ),
                "foreign lifecycle ledger must fail before publication",
            );
            assert_eq!(
                error.reason(),
                "repaired lifecycle ledger could not prepare an exact coordinator open"
            );
            assert!(error.retains_exact_installed_for_test(ledger.path()));
            assert!(crate::sumeragi::status::v2_status().is_none());
        }

        // A corrupt opaque registry seal cannot mint the logical projection;
        // its closed row remains owned by the fail-stop error.
        crate::sumeragi::status::clear_v2_status();
        {
            let safety = TempDir::new().expect("wrong-registry safety directory");
            let ledger = TempDir::new().expect("wrong-registry ledger");
            let payload = TempDir::new().expect("wrong-registry payload store");
            let body = TempDir::new().expect("wrong-registry body store");
            let mut holder =
                super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
            let mut installed =
                install_recovered_prepare_startup(&safety, ledger.path(), 0xDB, &mut holder);
            let verified = verified_from_installed_startup(&installed);
            let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
                &verified,
                ledger.path(),
                payload.path(),
                body.path(),
            );
            assert!(
                installed
                    .installed
                    .seed_parent_recovery_for_test(&mut recovery)
            );
            installed.installed.corrupt_registry_seal_for_test();
            crate::sumeragi::status::clear_v2_status();
            let error = expect_recovered_open_error(
                installed.open_coordinator_and_publish_for_test(
                    ledger.path(),
                    &mut payload_store,
                    recovery,
                ),
                "corrupt installed registry seal must fail closed",
            );
            assert_eq!(
                error.reason(),
                "installed recovered Sign registry seal is inconsistent"
            );
            assert!(error.retains_closed_registry_row_for_test());
            assert!(crate::sumeragi::status::v2_status().is_none());
        }

        // Even after the exact coordinator and both stores are committed, a
        // status construction error retains that whole opened authority.
        crate::sumeragi::status::clear_v2_status();
        {
            let safety = TempDir::new().expect("status-failure safety directory");
            let ledger = TempDir::new().expect("status-failure ledger");
            let payload = TempDir::new().expect("status-failure payload store");
            let body = TempDir::new().expect("status-failure body store");
            let mut holder =
                super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
            let mut installed =
                install_recovered_prepare_startup(&safety, ledger.path(), 0xDC, &mut holder);
            let verified = verified_from_installed_startup(&installed);
            let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
                &verified,
                ledger.path(),
                payload.path(),
                body.path(),
            );
            assert!(
                installed
                    .installed
                    .seed_parent_recovery_for_test(&mut recovery)
            );
            installed.adapter.registry.validators.clear();
            crate::sumeragi::status::clear_v2_status();
            let error = expect_recovered_open_error(
                installed.open_coordinator_and_publish_for_test(
                    ledger.path(),
                    &mut payload_store,
                    recovery,
                ),
                "invalid adapter status must fail after exact open",
            );
            assert_eq!(
                error.reason(),
                "adapter status publication failed after exact lifecycle open"
            );
            assert!(error.retains_exact_installed_for_test(ledger.path()));
            assert!(crate::sumeragi::status::v2_status().is_none());
        }
        crate::sumeragi::status::clear_v2_status();
    }

    #[test]
    fn recovered_prepare_already_repaired_child_reopens_and_publishes() {
        let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
        crate::sumeragi::status::clear_v2_status();
        let safety = TempDir::new().expect("repaired-child safety directory");
        let ledger = TempDir::new().expect("repaired-child ledger");
        let payload = TempDir::new().expect("repaired-child payload store");
        let body = TempDir::new().expect("repaired-child body store");
        let (startup, _vote, proposal, manifest, validated) =
            reopen_with_prepare_intent(&safety, 0xDD);
        let replay_proposal = proposal.clone();
        let replay_manifest = manifest.clone();
        let replay_validated = validated.clone();

        let mut first_holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let first = join_recovered_prepare_startup(
            startup,
            proposal,
            manifest,
            validated,
            &mut first_holder,
        );
        let (_summary, durable_before_crash) = first
            .persist_repair_for_test(ledger.path())
            .unwrap_or_else(|error| panic!("fsync the first repaired frame: {}", error.reason()));
        drop(durable_before_crash);

        let restarted = open_recovered_startup_test(&safety)
            .expect("fresh startup replays the unchanged repaired WAL frame");
        let mut restarted_holder =
            super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let restarted = join_recovered_prepare_startup(
            restarted,
            replay_proposal,
            replay_manifest,
            replay_validated,
            &mut restarted_holder,
        );
        let (changed, durable) = restarted
            .persist_reopened_repair_for_test(ledger.path())
            .unwrap_or_else(|error| {
                panic!(
                    "stutter on the already repaired ledger frame: {}",
                    error.reason()
                )
            });
        assert!(!changed);
        let installed = durable
            .install_recovered_sign_for_test(ledger.path())
            .unwrap_or_else(|error| {
                panic!("install the repaired-frame Sign child: {}", error.reason())
            });
        let verified = verified_from_installed_startup(&installed);
        let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
            &verified,
            ledger.path(),
            payload.path(),
            body.path(),
        );
        assert!(
            installed
                .installed
                .seed_child_recovery_for_test(&mut recovery)
        );
        crate::sumeragi::status::clear_v2_status();
        let published = installed
            .open_coordinator_and_publish_for_test(ledger.path(), &mut payload_store, recovery)
            .unwrap_or_else(|error| {
                panic!(
                    "already-repaired child must reopen idempotently: {}",
                    error.reason()
                )
            });
        assert!(published.exact_published_join_for_test());
        assert!(crate::sumeragi::status::v2_status().is_some());
        drop(published);
        crate::sumeragi::status::clear_v2_status();
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_commit_vote_sign_retains_the_exact_authenticated_prepare_qc() {
        let directory = TempDir::new().expect("temporary Commit recovery directory");
        let (mut adapter, startup) = open_test(&directory).expect("open Commit replay fixture");
        assert!(startup.is_empty());
        let locked_subject = subject(0xD2);
        let round = wire::ConsensusRound {
            context_id: adapter.wire_context.id(),
            height: adapter.wire_context.height,
            view: 0,
        };
        let (_, keys, _) = authenticated_context();
        let mut wire_prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: locked_subject,
            execution_commitment: execution_commitment(0xD2),
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        authenticate_qc(&mut wire_prepare, &keys);
        let prepare = adapter
            .registry
            .qc_to_core(&wire_prepare, &adapter.wire_context)
            .expect("register the authenticated PrepareQC");
        let core_context = adapter.reducer.context().id();
        let core_round = reducer::Round::new(round.height, round.view);
        let core_subject = prepare.subject();
        let local_validator = adapter
            .registry
            .validator_id(0)
            .expect("fixture local validator");
        let entry = reducer::WalEntry::new(
            reducer::PersistenceId::new(1),
            reducer::WalRecord::LockAndCommit {
                prepare,
                vote: reducer::Vote::new_with_proposal_round(
                    core_context,
                    core_round,
                    core_round,
                    reducer::Phase::Commit,
                    core_subject,
                    local_validator,
                ),
            },
        );
        let encoded = adapter
            .registry
            .encode_wal_entry(&entry, &TestAggregator)
            .expect("encode the exact LockAndCommit frame");
        assert_eq!(
            adapter
                .wal
                .append(&encoded)
                .expect("append lock frame")
                .sequence(),
            0
        );
        drop(adapter);

        let startup = open_recovered_startup_test(&directory)
            .expect("replay authenticated LockAndCommit behind the sealed startup cut");
        let authenticated = match startup.authenticate_final_wal_vote() {
            Ok(authenticated) => authenticated,
            Err((error, _startup)) => {
                panic!("authenticate the final recovered LockAndCommit: {error}")
            }
        };
        let authority = authenticated
            .recovered_vote
            .as_ref()
            .expect("LockAndCommit carries one restart vote");
        assert!(authenticated.effects.is_empty());
        assert!(authority.wal_identity().is_exact());
        assert!(authority.replay_evidence_is_exact());
        assert!(
            authority.exactly_matches_wal_record(
                authenticated
                    .adapter
                    .wal
                    .recovered_records()
                    .last()
                    .expect("LockAndCommit WAL frame remains retained")
            )
        );
        assert_eq!(authority.vote().phase, wire::GlobalPhase::Commit);
        assert_eq!(authority.vote().round, round);
        assert_eq!(authority.vote().proposal_round, round);
        assert_eq!(authority.vote().subject, locked_subject);
        let retained_prepare = authority
            .prepare_certificate()
            .expect("Commit recovery retains the exact PrepareQC");
        assert_eq!(retained_prepare, &wire_prepare);
        assert_eq!(
            retained_prepare.execution_commitment,
            authority.vote().execution_commitment
        );
        drop(authenticated);
    }

    #[test]
    fn recovered_vote_sign_startup_cut_is_one_shot_and_drop_inert() {
        let directory = TempDir::new().expect("temporary recovery seal directory");
        let (startup, _expected_vote, _proposal, _manifest, _validated) =
            reopen_with_prepare_intent(&directory, 0xD3);
        let wal_path = directory.path().join("safety.wal");
        let durable_before = std::fs::read(&wal_path).expect("read sealed WAL before drop");
        let authenticated = match startup.authenticate_final_wal_vote() {
            Ok(authenticated) => authenticated,
            Err((error, _startup)) => panic!("authenticate exact replay vote: {error}"),
        };
        assert!(authenticated.recovered_vote.is_some());
        assert!(
            authenticated.effects.iter().all(|effect| !matches!(
                effect,
                AdapterEffect::Sign {
                    request: SignRequest::Vote(_),
                    ..
                }
            )),
            "the retained batch no longer contains the sealed vote"
        );
        assert!(
            authenticated.finish_without_wal_vote().is_err(),
            "a phase-vote startup cannot escape through the no-vote path"
        );
        assert_eq!(
            std::fs::read(&wal_path).expect("read sealed WAL after drop"),
            durable_before,
            "dropping the sealed startup cannot rewrite its WAL"
        );

        let repeated = open_recovered_startup_test(&directory)
            .expect("the unchanged WAL can be authenticated by a new sealed startup instance");
        let repeated = repeated
            .authenticate_final_wal_vote()
            .unwrap_or_else(|(error, _startup)| panic!("reauthenticate unchanged WAL: {error}"));
        assert!(repeated.recovered_vote.is_some());
        assert!(repeated.effects.is_empty());
        drop(repeated);
    }

    #[test]
    fn recovered_startup_exposes_authenticated_non_vote_wal_records() {
        let directory = TempDir::new().expect("temporary non-vote recovery directory");
        let (mut adapter, startup) = open_test(&directory).expect("open timeout replay fixture");
        assert!(startup.is_empty());
        let timeout = adapter
            .timeout_elapsed(adapter.current_tag())
            .expect("persist the exact TimeoutIntent")
            .into_effects();
        assert!(matches!(
            timeout.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));
        drop(adapter);

        let startup = open_recovered_startup_test(&directory)
            .expect("replay the durable TimeoutIntent behind the sealed startup cut");
        let authenticated = startup
            .authenticate_final_wal_vote()
            .unwrap_or_else(|(error, _startup)| panic!("authenticate TimeoutIntent: {error}"));
        assert!(
            authenticated.recovered_vote.is_none(),
            "TimeoutIntent owns no phase-vote continuation"
        );
        let Ok((adapter, startup)) = authenticated.finish_without_wal_vote() else {
            panic!("authenticated non-vote startup must use the ordinary ready path")
        };
        assert!(matches!(
            startup.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));
        assert_eq!(adapter.wal.recovered_records().len(), 1);
    }
