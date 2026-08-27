#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn assert_ready_validate_vote_sign_live_transaction(
    attach_ledger: bool,
    sign_phase: wire::GlobalPhase,
    supersede_prepare: bool,
    exercise_later_terminal_validate_retry: bool,
) {
    assert!(matches!(
        sign_phase,
        wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit
    ));
    assert!(!supersede_prepare || (attach_ledger && sign_phase == wire::GlobalPhase::Prepare));
    assert!(
        !exercise_later_terminal_validate_retry
            || (attach_ledger && sign_phase == wire::GlobalPhase::Prepare && !supersede_prepare)
    );
    let marker = match sign_phase {
        wire::GlobalPhase::Prepare => 0xDF,
        wire::GlobalPhase::Commit => 0xE0,
    };
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        holder: _,
        lease: _,
        durable,
    } = ready_durable_validate_fixture_at_view(
        marker,
        0,
        ReadyDurableValidateFixtureOutcome::Validated,
    );
    let (tag, round, subject) = match &fixture.effect {
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => (*tag, *round, *subject),
        _ => unreachable!("Ready fixture retains one Validate effect"),
    };
    let adapter_directory = TempDir::new().expect("temporary Ready Validate adapter");
    let wal_path = adapter_directory.path().join("safety.wal");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        &wal_path,
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [0xE0; 32],
        AdapterFingerprints {
            node: Hash::new(b"Ready Validate registry join node"),
            build: Hash::new(b"Ready Validate registry join build"),
            config: Hash::new(b"Ready Validate registry join config"),
        },
        DeferredAdmissionOrdinalSource::new(1),
    )
    .expect("open exact Ready Validate adapter");
    assert!(startup.is_empty());

    let proposal = wire::Proposal {
        round,
        proposer: fixture.verified.context().leader(round.view),
        subject,
        manifest: fixture.manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![marker],
    };
    let fetch = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            )),
        ))
        .expect("admit exact Ready Validate proposal")
        .into_effects();
    assert!(matches!(
        fetch.as_slice(),
        [AdapterEffect::FetchBody {
            tag: effect_tag,
            manifest: Some(effect_manifest),
            ..
        }] if *effect_tag == tag && effect_manifest == &fixture.manifest
    ));
    let stored = adapter
        .body_available(tag, fixture.manifest.clone())
        .expect("advance exact Ready Validate body to Store")
        .into_effects();
    assert!(matches!(
        stored.as_slice(),
        [AdapterEffect::StoreBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));
    let validate = adapter
        .body_stored(tag, round, subject, &durable)
        .expect("advance exact Ready Validate body to Validate")
        .into_effects();
    assert!(matches!(
        validate.as_slice(),
        [AdapterEffect::ValidateBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));

    let validated_receipt = ValidatedBodyReceipt::for_test(durable.clone());
    if sign_phase == wire::GlobalPhase::Commit {
        let prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: validated_receipt.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        };
        let observed = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    prepare,
                )),
            ))
            .expect("register exact concurrent PrepareQC");
        assert!(observed.effects().is_empty());
    }
    let mut holder = LifecycleWorkRegistryHolder::empty();
    let (lease, slot, coordinator_candidate, _retry_census) = holder
        .install_remote_proposal_validate_completion_for_test(
            &fixture.verified,
            tag,
            proposal,
            fixture.manifest.clone(),
            validated_receipt,
            None,
        );
    let registry_before = format!("{:?}", holder.registry_for_test());
    let prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, slot, &fixture.verified)
        .expect("prepare exact Ready Validate registry carrier");
    let preview = prepared
        .prepare_adapter_preview(&mut adapter)
        .unwrap_or_else(|_| panic!("join exact registry carrier to adapter preview"));
    let wal_before = std::fs::read(&wal_path).expect("read empty Ready Validate WAL");
    let persisted = preview
        .seal_live_wal_validate_sign()
        .unwrap_or_else(|_| panic!("seal exact Ready Validate Sign to real WAL"));
    let wal_after = std::fs::read(&wal_path).expect("read persisted Ready Validate WAL");
    assert!(wal_after.len() > wal_before.len());

    let active_context = LifecycleContext::new(
        coordinator_candidate.key.context(),
        coordinator_candidate.key.round().height(),
    );
    let mut coordinator = LifecycleCoordinator::new(
        active_context,
        0,
        CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
    );
    assert!(matches!(
        coordinator.reduce_admit(AdmissionRequest::Candidate(coordinator_candidate)),
        AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } if owner == lease.owner() && ordinal == lease.ordinal()
    ));
    coordinator.ready_index.remove(&lease.ordinal());
    let parent = coordinator
        .records
        .get_mut(&lease.ordinal())
        .expect("admitted Validate parent");
    parent.physical_slots = lease.physical_slots().clone();
    parent.state = LifecycleState::Claimed(lease.id());
    coordinator.active_lease = Some(lease.clone());
    if !attach_ledger {
        let result = coordinator.prepare_sealed_validate_sign_transition(
            &lease,
            &fixture.verified,
            persisted,
        );
        assert!(result.is_err());
        drop(result);
        assert!(coordinator.ledger_store.is_none());
        assert_eq!(
            coordinator.records[&lease.ordinal()].state,
            LifecycleState::Claimed(lease.id())
        );
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(
            std::fs::read(&wal_path).expect("read WAL after missing-store rejection"),
            wal_after
        );
        assert!(matches!(
            adapter.body_available(tag, fixture.manifest.clone()),
            Err(AdapterError::FailClosed)
        ));
        return;
    }
    let ledger_directory = TempDir::new().expect("temporary live publication ledger");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach exact current LedgerV1");
    let (_runtime_ordinal_authority, coordinator_ordinal_authority) =
        authority::lifecycle_ordinal_authorities_after_high_watermark(coordinator.high_water());
    coordinator
        .bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)
        .expect("bind the coordinator half of the live Validate ordinal authority");

    coordinator
        .prepare_sealed_validate_sign_transition(&lease, &fixture.verified, persisted)
        .unwrap_or_else(|_| panic!("stage exact sealed Validate-to-Vote transaction"))
        .persist_and_publish()
        .unwrap_or_else(|_| panic!("fsync and publish exact live Validate-to-Vote cut"));

    let child_ordinal = lease.ordinal() + 1;
    let child_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
    let child_address = ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot)
        .expect("exact live Sign child address");
    assert_ne!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(holder.registry_for_test().entries.len(), 1);
    let child_work = holder
        .registry_for_test()
        .entries
        .get(&child_address)
        .expect("reserved Sign child is installed");
    assert!(child_work.validate_exact());
    assert_eq!(child_work.causal_root(), lease.owner().causal_root());
    assert!(matches!(
        &child_work.kind,
        ConcreteLifecycleWorkKind::DurableLiveWalSign(sign)
            if matches!(
                &sign.admission.bound.effect,
                AdapterEffect::Sign {
                    request: SignRequest::Vote(vote),
                    ..
                } if vote.phase == sign_phase
            )
                && sign.dispatch_key.is_none()
    ));
    let child_sign_effect = match &child_work.kind {
        ConcreteLifecycleWorkKind::DurableLiveWalSign(sign) => sign.admission.bound.effect.clone(),
        _ => unreachable!("reserved Validate successor remains one live-WAL Sign"),
    };
    assert_eq!(
        coordinator.records[&lease.ordinal()].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    let expected_edge = match sign_phase {
        wire::GlobalPhase::Prepare => {
            super::super::schema::DurableContinuationEdge::ValidateToSignPrepare
        }
        wire::GlobalPhase::Commit => {
            super::super::schema::DurableContinuationEdge::ValidateToSignCommit
        }
    };
    assert_eq!(
        coordinator.durable_records[&lease.ordinal()].continuation,
        super::super::schema::DurableContinuation::successor(expected_edge, child_ordinal)
    );
    assert_eq!(
        coordinator.records[&child_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(
        coordinator.records[&child_ordinal].stage.kind(),
        match sign_phase {
            wire::GlobalPhase::Prepare => LifecycleStageKind::SignPrepareVote,
            wire::GlobalPhase::Commit => LifecycleStageKind::SignCommitVote,
        }
    );
    assert!(
        holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &coordinator)
    );
    let exact_replay = coordinator.durable_records[&child_ordinal]
        .replay_authority
        .clone();
    let foreign_replay = exact_replay
        .with_foreign_origin_generation_for_test()
        .expect("live WAL Sign replay supports a foreign-generation negative fixture");
    let child = &coordinator.records[&child_ordinal];
    let child_metadata = &coordinator.durable_records[&child_ordinal];
    assert!(foreign_replay.structurally_matches_record(
        coordinator.active_context,
        child.key,
        child.work_class,
        child.stage,
        child_metadata.payload,
    ));
    coordinator
        .durable_records
        .get_mut(&child_ordinal)
        .expect("live Sign metadata")
        .replay_authority = foreign_replay;
    assert!(
        !holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &coordinator)
    );
    coordinator
        .durable_records
        .get_mut(&child_ordinal)
        .expect("live Sign metadata")
        .replay_authority = exact_replay;
    assert!(coordinator.active_lease.is_none());
    assert!(adapter.signature_fence_is_active());
    assert!(matches!(
        adapter.signature_fence_identity(),
        Some((identity_tag, reducer::SignableMessage::Vote(vote)))
            if identity_tag == tag
                && vote.phase()
                    == match sign_phase {
                        wire::GlobalPhase::Prepare => reducer::Phase::Prepare,
                        wire::GlobalPhase::Commit => reducer::Phase::Commit,
                    }
    ));
    let (_, reopened) =
        super::super::ledger::LifecycleLedgerStoreV1::open(ledger_directory.path(), active_context)
            .expect("reopen exact committed LedgerV1");
    assert_eq!(reopened.high_water(), child_ordinal);
    assert_eq!(reopened.records().len(), 2);
    assert_eq!(
        reopened.records()[0].terminal(),
        Some(Some(TerminalOutcome::Advanced))
    );
    assert_eq!(
        reopened.records()[0].continuation(),
        Some(super::super::schema::DurableContinuation::successor(
            expected_edge,
            child_ordinal,
        ))
    );

    let attestation = holder
        .registry_for_test()
        .attest_ready_recovered_lifecycle_sign(&coordinator, child_ordinal)
        .expect("typed live Commit Sign is the sole Ready bounded-I/O carrier");
    assert_eq!(
        attestation.demand(),
        ReadyRecoveredLifecycleSignDemandV1::BoundedIo
    );
    let child_record = coordinator.records[&child_ordinal].clone();
    let sign_lease = TurnLease {
        id: LeaseId(lease.id().0 + 1),
        ordinal: child_ordinal,
        owner: child_record.owner,
        key: child_record.key,
        work_class: child_record.work_class,
        stage: child_record.stage,
        rank: super::super::SchedulerRank::new(0, 0, 0, 0, 0, 0, 0, 0),
        physical_slots: child_record.physical_slots.clone(),
        output_reservation: Some(super::super::schema::LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            0,
        )),
    };
    coordinator.ready_index.remove(&child_ordinal);
    coordinator
        .records
        .get_mut(&child_ordinal)
        .expect("live Commit Sign row remains installed")
        .state = LifecycleState::Claimed(sign_lease.id());
    coordinator.active_lease = Some(sign_lease.clone());

    let continuation = coordinator.durable_records[&lease.ordinal()].continuation;
    coordinator
        .durable_records
        .get_mut(&lease.ordinal())
        .expect("live Validate predecessor metadata remains installed")
        .continuation = super::super::schema::DurableContinuation::None;
    assert!(matches!(
        holder
            .registry_for_test_mut()
            .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease),
        Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier)
    ));
    coordinator
        .durable_records
        .get_mut(&lease.ordinal())
        .expect("live Validate predecessor metadata remains installed")
        .continuation = continuation;

    let prepared = holder
        .registry_for_test_mut()
        .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease)
        .expect("project the exact claimed live Commit Sign once");
    let dispatch_key = prepared.dispatch_key();
    let task = prepared.commit_for_worker();
    assert_eq!(task.dispatch_key(), dispatch_key);
    assert!(matches!(
        holder
            .registry_for_test_mut()
            .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease),
        Err(RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched)
    ));

    if sign_phase == wire::GlobalPhase::Prepare {
        let AdapterEffect::Sign {
            tag: sign_tag,
            request,
        } = child_sign_effect
        else {
            unreachable!("validated receiver successor remains one Vote Sign")
        };
        let SignRequest::Vote(local_vote) = &request else {
            unreachable!("validated receiver successor remains one Prepare Vote Sign")
        };
        let roster_len = u32::try_from(fixture.verified.context().roster.len())
            .expect("fixture roster length fits a validator index");
        let mut peer_vote = local_vote.clone();
        peer_vote.signer = local_vote
            .signer
            .checked_add(1)
            .expect("fixture signer advances")
            % roster_len;
        peer_vote.signature = vec![marker.wrapping_add(1); 96];
        let peer_message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(peer_vote));
        let deferred = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                peer_message.clone(),
            ))
            .expect("admit one authenticated peer Prepare behind the receiver Sign fence");
        assert_eq!(
            deferred.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );

        let queue_projection = |adapter: &mut SumeragiV2Adapter| {
            adapter
                .status()
                .expect("snapshot receiver deferred queues")
                .liveness
                .queues
                .into_iter()
                .filter(|status| {
                    matches!(
                        status.queue,
                        wire::SumeragiV2QueueKind::DeferredCompletion
                            | wire::SumeragiV2QueueKind::DeferredProgress
                            | wire::SumeragiV2QueueKind::DeferredNormal
                    )
                })
                .map(|status| {
                    (
                        status.queue,
                        status.depth,
                        status.capacity,
                        status.service_debt,
                    )
                })
                .collect::<Vec<_>>()
        };
        let queue_projection_before = queue_projection(&mut adapter);
        assert_eq!(
            queue_projection_before
                .iter()
                .find(|(queue, ..)| *queue == wire::SumeragiV2QueueKind::DeferredCompletion)
                .map(|(_, depth, ..)| *depth),
            Some(0)
        );
        assert_eq!(
            queue_projection_before
                .iter()
                .find(|(queue, ..)| *queue == wire::SumeragiV2QueueKind::DeferredProgress)
                .map(|(_, depth, ..)| *depth),
            Some(0)
        );
        assert_eq!(
            queue_projection_before
                .iter()
                .find(|(queue, ..)| *queue == wire::SumeragiV2QueueKind::DeferredNormal)
                .map(|(_, depth, ..)| *depth),
            Some(1)
        );
        let ordinals_before = adapter.all_deferred_admission_ordinals();
        let authenticated_before = adapter.authenticated_deferred_admission_ordinals();
        let (owned_tag, deferred_ordinal) = adapter
            .deferred_authenticated_message_owner(&peer_message)
            .expect("the exact authenticated peer Prepare retains one deferred owner");
        assert_eq!(owned_tag, sign_tag);
        let ownership_before = adapter
            .deferred_occurrence_ownership(deferred_ordinal)
            .expect("the deferred peer Prepare retains its opaque occurrence authority");
        assert!(ownership_before.is_authenticated_ingress());
        assert!(ownership_before.still_retained());

        let keys = durable_store_keys(marker);
        let signer =
            usize::try_from(local_vote.signer).expect("fixture local Vote signer is representable");
        let signature = iroha_crypto::Signature::try_new(
            keys[signer].private_key(),
            &request.signature_preimage(),
        )
        .expect("sign exact receiver Prepare Vote task");
        let signature = signature.payload().to_vec();
        let authority = crate::sumeragi::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::from_registry_task_for_test(
            task,
            signature.clone(),
            None,
        );
        let preview = adapter
            .prepare_recovered_lifecycle_sign_completion(authority)
            .expect("preview receiver Prepare signature ahead of Busy-deferred peer ingress");
        assert_eq!(
            preview.settlement_family(),
            Some(crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast)
        );
        if exercise_later_terminal_validate_retry {
            let successor = holder
                .registry_for_test_mut()
                .prepare_recovered_lifecycle_sign_broadcast_successor(
                    &coordinator,
                    &sign_lease,
                    &fixture.verified,
                    dispatch_key,
                    preview,
                )
                .expect("bind the exact live Prepare Sign to its signed Broadcast child");
            let transition = coordinator
                .prepare_recovered_lifecycle_sign_broadcast_transition(
                    &sign_lease,
                    &fixture.verified,
                    successor,
                )
                .expect("stage the exact live Prepare Sign-to-Broadcast successor");
            transition
                .persist_exact_successor()
                .expect("persist the live Prepare Sign-to-Broadcast successor");
            transition.commit_after_publication();

            let broadcast_ordinal = coordinator.high_water();
            let broadcast = coordinator
                .records
                .get(&broadcast_ordinal)
                .expect("published signed Broadcast remains in the coordinator");
            assert_eq!(broadcast.owner, lease.owner());
            assert_eq!(broadcast.work_class, LifecycleWorkClass::Broadcast);
            assert_eq!(broadcast.state, LifecycleState::Ready);
            let (&broadcast_slot, &broadcast_digest) = broadcast
                .physical_slots
                .first_key_value()
                .expect("published signed Broadcast retains one physical slot");
            let broadcast_address =
                ConcreteWorkAddress::new(broadcast.owner, broadcast_ordinal, broadcast_slot)
                    .expect("published signed Broadcast has one exact address");
            assert!(matches!(
                &holder.registry_for_test().entries[&broadcast_address].kind,
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_)
            ));

            // Model the assertion-only tail of the production refanout: the
            // durable Broadcast remains nonterminal but parks on its exact
            // Recovery(digest) generation after output handoff.
            let wait_source = WaitSource::Recovery(broadcast_digest);
            let observed_generation = coordinator
                .observed_generation
                .get(&wait_source)
                .copied()
                .unwrap_or(0);
            coordinator
                .observed_generation
                .insert(wait_source, observed_generation);
            assert!(coordinator.ready_index.remove(&broadcast_ordinal));
            coordinator
                .records
                .get_mut(&broadcast_ordinal)
                .expect("refanned signed Broadcast remains installed")
                .state = LifecycleState::Waiting(WaitToken::new(wait_source, observed_generation));
            assert!(
                holder
                    .registry_for_test()
                    .exactly_covers_finalization_work(&coordinator),
                "the original live Validate-to-Sign-to-Broadcast lineage is finalization-exact"
            );

            // Reproduce the durable-retry history observed in the network:
            // the same causal owner later acquires a fresh Validate ordinal,
            // which terminalizes without a successor. It is not part of the
            // older closed Sign-to-Broadcast interval.
            let retry_case = super::super::replay_authority::exact_record_fixture(
                coordinator.active_context,
                LifecycleStageKind::ValidateBody,
                0xA7,
            );
            assert_eq!(retry_case.work_class, LifecycleWorkClass::Validate);
            assert_ne!(
                retry_case.key,
                coordinator.records[&lease.ordinal()].key,
                "the later Validate occurrence must retain a distinct lifecycle key"
            );
            let retry_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
            let retry_candidate = CandidateAdmission::new(
                retry_case.key,
                lease.owner().causal_root(),
                retry_case.work_class,
                retry_case.stage,
                InitialLifecycleState::Ready,
                lease.owner().causal_root().digest(),
                retry_case.payload,
                retry_case.authority,
                super::super::PhysicalGeometry::new(
                    [PhysicalSlot::new(
                        retry_slot,
                        LifecycleDigest::new([0xA7; 32]),
                    )],
                    [retry_slot],
                ),
                None,
            );
            let mut staged = coordinator.stage_durable_transaction();
            let retry_ordinal = match staged
                .reduce_admit(AdmissionRequest::Candidate(retry_candidate))
            {
                AdmissionDecision::Admitted {
                    owner,
                    ordinal,
                    producer_turn_ordinal: None,
                } if owner == lease.owner() => ordinal,
                decision => panic!("admit later same-owner Validate retry: {decision:?}"),
            };
            assert_eq!(
                retry_ordinal,
                broadcast_ordinal
                    .checked_add(1)
                    .expect("later Validate retry ordinal remains representable")
            );
            staged
                .finish_terminal(retry_ordinal, TerminalOutcome::Advanced)
                .expect("terminalize the later same-owner Validate retry");
            staged
                .durable_records
                .get_mut(&retry_ordinal)
                .expect("later terminal Validate retry retains durable metadata")
                .continuation = super::super::schema::DurableContinuation::AdvancedNoSuccessor;
            assert!(
                super::super::ledger::LifecycleLedgerV1::from_coordinator(&staged).is_ok(),
                "the later same-owner terminal Validate retry must be a valid LedgerV1 row"
            );
            coordinator
                .persist_exact_staged_successor(&staged)
                .expect("persist the later same-owner terminal Validate retry");
            coordinator = staged;

            assert_eq!(
                coordinator
                    .records
                    .values()
                    .filter(|record| record.owner == lease.owner())
                    .map(|record| record.ordinal)
                    .collect::<Vec<_>>(),
                vec![
                    lease.ordinal(),
                    child_ordinal,
                    broadcast_ordinal,
                    retry_ordinal,
                ]
            );
            assert!(
                holder
                    .registry_for_test()
                    .exactly_covers_finalization_work(&coordinator),
                "a later terminal same-owner Validate retry cannot invalidate the older refanned Broadcast"
            );
            return;
        }
        drop(preview);

        assert_eq!(queue_projection(&mut adapter), queue_projection_before);
        assert_eq!(adapter.all_deferred_admission_ordinals(), ordinals_before);
        assert_eq!(
            adapter.authenticated_deferred_admission_ordinals(),
            authenticated_before
        );
        assert_eq!(
            adapter.deferred_authenticated_message_owner(&peer_message),
            Some((owned_tag, deferred_ordinal))
        );
        let ownership_after = adapter
            .deferred_occurrence_ownership(deferred_ordinal)
            .expect("drop retains the exact authenticated peer Prepare owner");
        assert_eq!(ownership_after, ownership_before);
        assert!(ownership_after.still_retained());
        assert!(adapter.signature_fence_is_active());

        if supersede_prepare {
            let decision = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: local_vote.execution_commitment.clone(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![marker; 96],
            };
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::QuorumCertificate(decision),
                    ),
                ))
                .expect("certified CommitQC bypasses the exact Prepare Sign fence");
            let mut forged_signature = signature.clone();
            forged_signature[0] ^= 1;
            let forged = crate::sumeragi::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
                child_ordinal,
                sign_tag,
                request.clone(),
                forged_signature,
                None,
                RecoveredLifecycleSignClassV1::PhaseVote,
            );
            assert!(matches!(
                adapter.prepare_recovered_lifecycle_sign_completion(forged),
                Err(AdapterError::RecoveredLifecycleSignCompletionMismatch)
            ));
            let superseded = crate::sumeragi::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
                child_ordinal,
                sign_tag,
                request,
                signature,
                None,
                RecoveredLifecycleSignClassV1::PhaseVote,
            );
            assert!(matches!(
                adapter.prepare_recovered_lifecycle_sign_completion(superseded),
                Err(AdapterError::RecoveredLifecycleSignCompletionSuperseded)
            ));
            return;
        }

        let signed = adapter
            .signature_completed(sign_tag, signature)
            .expect("apply the same exact receiver Prepare signature after inert preview drop");
        assert!(matches!(
            signed.effects(),
            [AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(_),
                ..
            })]
        ));
        let (_effects, evidence) = adapter
            .drain_deferred_with_evidence()
            .expect("service receiver peer Prepare after its Sign fence opens")
            .expect("the exact Busy-deferred peer Prepare remains selectable");
        assert!(evidence.validate_exact());
        assert_eq!(evidence.admission_ordinal, deferred_ordinal);
        assert_eq!(
            evidence.priority,
            crate::sumeragi::v2::DeferredPriority::Normal
        );
        assert_eq!(
            evidence.service_cursor_before,
            crate::sumeragi::v2::DeferredPriority::Completion
        );
        assert_eq!(
            evidence.service_cursor_after,
            crate::sumeragi::v2::DeferredPriority::Completion
        );
        assert_eq!(evidence.queue_lengths_before.completion, 0);
        assert_eq!(evidence.queue_lengths_before.progress, 0);
        assert_eq!(evidence.queue_lengths_before.normal, 1);
    } else {
        let cancellation = holder
            .registry_for_test()
            .prepare_recovered_lifecycle_sign_cancellation(&coordinator, &sign_lease, dispatch_key)
            .expect("bind the exact dispatched Commit Sign for cancellation");
        let mut staged = coordinator.stage_durable_transaction();
        staged.reduce_cancel_superseded_sign(sign_lease.clone());
        let mut wrong_staged = staged.clone();
        wrong_staged.high_water = wrong_staged
            .high_water
            .checked_add(1)
            .expect("negative staged high-water mutation fits");
        let cancellation = match holder
            .registry_for_test_mut()
            .publish_recovered_lifecycle_sign_cancellation(
                cancellation,
                &coordinator,
                &wrong_staged,
                &sign_lease,
                || -> Result<(), ()> { panic!("invalid staged cancellation must not publish") },
            ) {
            Err(RecoveredLifecycleSignCancellationPublicationError::Preflight(cancellation)) => {
                cancellation
            }
            Ok(()) => panic!("invalid staged cancellation cannot publish"),
            Err(RecoveredLifecycleSignCancellationPublicationError::Publication(_, _)) => {
                panic!("invalid staged cancellation cannot reach publication")
            }
        };
        assert!(
            holder
                .registry_for_test()
                .entries
                .contains_key(&child_address)
        );
        match holder
            .registry_for_test_mut()
            .publish_recovered_lifecycle_sign_cancellation(
                cancellation,
                &coordinator,
                &staged,
                &sign_lease,
                || coordinator.persist_exact_staged_successor(&staged),
            ) {
            Ok(()) => {}
            Err(RecoveredLifecycleSignCancellationPublicationError::Preflight(_)) => {
                panic!("exact superseded Commit Sign cancellation must pass preflight")
            }
            Err(RecoveredLifecycleSignCancellationPublicationError::Publication(_, _)) => {
                panic!("exact superseded Commit Sign cancellation must publish")
            }
        }
        coordinator = staged;
        assert!(holder.registry_for_test().entries.is_empty());
        assert!(coordinator.active_lease.is_none());
        assert_eq!(
            coordinator.records[&child_ordinal].state,
            LifecycleState::Terminal(TerminalOutcome::Cancelled)
        );
        let (_, reopened_cancelled) = super::super::ledger::LifecycleLedgerStoreV1::open(
            ledger_directory.path(),
            active_context,
        )
        .expect("reopen LedgerV1 after Sign cancellation");
        assert_eq!(
            reopened_cancelled.records()[1].terminal(),
            Some(Some(TerminalOutcome::Cancelled))
        );
    }
}

#[cfg(feature = "bls")]
#[test]
fn ready_validate_apply_publishes_at_actor_global_child_coordinates() {
    let handle = std::thread::Builder::new()
        .name("ready-validate-apply-actor-global-child".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| ready_validate_apply_actor_global_child_fixture(false, false))
        .expect("spawn Ready Validate Apply actor-global child fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[test]
fn ready_validate_apply_rejects_a_tampered_body_frame_before_publication() {
    let handle = std::thread::Builder::new()
        .name("ready-validate-apply-tampered-body-frame".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| ready_validate_apply_actor_global_child_fixture(true, false))
        .expect("spawn tampered Ready Validate Apply body-frame fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[test]
fn lifecycle_decision_apply_live_recovered_substitution_matrix_is_inert() {
    let handle = std::thread::Builder::new()
        .name("lifecycle-apply-live-recovered-substitution".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| ready_validate_apply_actor_global_child_fixture(false, true))
        .expect("spawn lifecycle Decision Apply lineage substitution fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[test]
fn recovered_decision_apply_finality_retires_authenticated_validate_retry_seal() {
    let handle = std::thread::Builder::new()
        .name("recovered-apply-validate-retry-retirement".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(recovered_decision_apply_validate_retry_retirement_fixture)
        .expect("spawn recovered Apply Validate retry retirement fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn ready_validate_apply_actor_global_child_fixture(
    tamper_apply_frame: bool,
    exercise_lineage_matrix: bool,
) {
    let marker = 0xE0;
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        holder: _,
        lease: _,
        durable,
    } = ready_durable_validate_fixture_at_view(
        marker,
        0,
        ReadyDurableValidateFixtureOutcome::Validated,
    );
    let (tag, round, subject) = match &fixture.effect {
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => (*tag, *round, *subject),
        _ => unreachable!("Ready fixture retains one Validate effect"),
    };
    let adapter_directory = TempDir::new().expect("temporary Ready Validate Apply adapter");
    let wal_path = adapter_directory.path().join("safety.wal");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        &wal_path,
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [marker; 32],
        AdapterFingerprints {
            node: Hash::new(b"Ready Validate Apply registry join node"),
            build: Hash::new(b"Ready Validate Apply registry join build"),
            config: Hash::new(b"Ready Validate Apply registry join config"),
        },
        DeferredAdmissionOrdinalSource::new(1),
    )
    .expect("open exact Ready Validate Apply adapter");
    assert!(startup.is_empty());

    let proposal = wire::Proposal {
        round,
        proposer: fixture.verified.context().leader(round.view),
        subject,
        manifest: fixture.manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![marker],
    };
    let fetch = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            )),
        ))
        .expect("admit exact Ready Validate Apply proposal")
        .into_effects();
    assert!(
        matches!(
            fetch.as_slice(),
            [AdapterEffect::FetchBody {
                tag: effect_tag,
                manifest: Some(effect_manifest),
                ..
            }] if *effect_tag == tag && effect_manifest == &fixture.manifest
        ),
        "unexpected proposal ingress effects: {fetch:?}"
    );
    let stored = adapter
        .body_available(tag, fixture.manifest.clone())
        .expect("advance exact decided body to Store")
        .into_effects();
    assert!(matches!(
        stored.as_slice(),
        [AdapterEffect::StoreBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));
    let validate = adapter
        .body_stored(tag, round, subject, &durable)
        .expect("advance exact decided body to Validate")
        .into_effects();
    assert!(matches!(
        validate.as_slice(),
        [AdapterEffect::ValidateBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));
    let validated_receipt = ValidatedBodyReceipt::for_test(durable.clone());
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: validated_receipt.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                prepare,
            )),
        ))
        .expect("register exact concurrent PrepareQC");
    assert!(observed.effects().is_empty());
    let decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: validated_receipt.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("register exact concurrent CommitQC");
    assert!(observed.effects().is_empty());

    let mut holder = LifecycleWorkRegistryHolder::empty();
    let (lease, slot, coordinator_candidate, recovered_validate_retry_census) = holder
        .install_remote_proposal_validate_completion_for_test(
            &fixture.verified,
            tag,
            proposal,
            fixture.manifest.clone(),
            validated_receipt,
            Some((
                decision.round,
                decision.proposal_round,
                decision.subject,
                decision.execution_commitment,
            )),
        );
    let active_context = LifecycleContext::new(
        coordinator_candidate.key.context(),
        coordinator_candidate.key.round().height(),
    );
    let mut coordinator = LifecycleCoordinator::new(
        active_context,
        0,
        CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
    );
    assert!(matches!(
        coordinator.reduce_admit(AdmissionRequest::Candidate(coordinator_candidate)),
        AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } if owner == lease.owner() && ordinal == lease.ordinal()
    ));
    coordinator
        .records
        .get_mut(&lease.ordinal())
        .expect("admitted Validate parent")
        .physical_slots = lease.physical_slots().clone();
    let live_validate_attestation = coordinator
        .attest_ready_validate_demand(&holder, lease.ordinal())
        .expect("attest exact Ready Validate predecessor before publication");
    assert!(!live_validate_attestation.requires_io_dispatch());
    let live_validate_dispatch_key = live_validate_attestation.dispatch_key();
    assert!(live_validate_dispatch_key.matches_consensus_round(&round));
    assert_eq!(
        recovered_validate_retry_census.owner_class_counts_for_test(),
        (1, 0),
        "the remote-Proposal Validate parent owns one admission retry seal"
    );
    coordinator.ready_index.remove(&lease.ordinal());
    coordinator
        .records
        .get_mut(&lease.ordinal())
        .expect("claim admitted Validate parent")
        .state = LifecycleState::Claimed(lease.id());
    coordinator.active_lease = Some(lease.clone());
    let prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, slot, &fixture.verified)
        .expect("prepare exact Ready Validate Apply registry carrier");
    let preview = prepared
        .prepare_adapter_preview(&mut adapter)
        .unwrap_or_else(|_| panic!("join exact Validate carrier to decided adapter preview"));
    let publication = preview
        .seal_live_wal_validate_apply()
        .unwrap_or_else(|_| panic!("seal exact Ready Validate Apply publication"));

    let local_prediction = coordinator
        .high_water
        .checked_add(1)
        .expect("local child prediction remains bounded");
    assert_eq!(local_prediction, 2);
    let (runtime_ordinals, coordinator_ordinals) =
        authority::lifecycle_ordinal_authorities_after_high_watermark(coordinator.high_water);
    coordinator.lifecycle_ordinal_authority = Some(coordinator_ordinals);
    let runtime_ordinals =
        crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
            runtime_ordinals,
        );
    runtime_ordinals
        .advance_past(7)
        .expect("advance actor-global ordinals past the local prediction");
    let ledger_directory = TempDir::new().expect("temporary Validate Apply lifecycle ledger");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach exact current LedgerV1");

    let mut transition = coordinator
        .prepare_sealed_validate_apply_transition(&lease, &fixture.verified, publication)
        .unwrap_or_else(|_| panic!("stage exact sealed Validate-to-Apply transaction"));
    if tamper_apply_frame {
        assert!(transition.tamper_apply_body_frame_for_test());
        let publication_error = transition
            .persist_and_publish()
            .expect_err("reject a candidate body frame foreign to the retained receipt");
        assert_eq!(
            publication_error.registry_failure_reason(),
            Some(LiveValidateApplyRegistryPublicationFailureReason::AdapterWork)
        );
        drop(publication_error);
        assert_eq!(
            coordinator.fault,
            Some(crate::sumeragi::v2_lifecycle_coordinator::CoordinatorFault::DurabilityFailure)
        );
        assert_eq!(holder.registry_for_test().entries.len(), 1);
        let parent_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .expect("retained Validate parent address");
        assert!(
            holder
                .registry_for_test()
                .entries
                .contains_key(&parent_address)
        );
        return;
    }
    let publication_result = transition.persist_and_publish();
    if let Err(error) = publication_result {
        panic!(
            "fsync and publish exact live Validate-to-Apply cut: {:?}",
            error.registry_failure_reason()
        );
    }

    let child_ordinal = 8;
    assert_ne!(child_ordinal, local_prediction);
    let child_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
    let child_address = ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot)
        .expect("actor-global Apply child address");
    let predicted_address = ConcreteWorkAddress::new(lease.owner(), local_prediction, child_slot)
        .expect("obsolete local Apply prediction");
    assert!(
        !holder
            .registry_for_test()
            .entries
            .contains_key(&predicted_address)
    );
    let child_work = holder
        .registry_for_test()
        .entries
        .get(&child_address)
        .expect("exact actor-global Apply child is installed");
    assert!(child_work.validate_exact());
    assert!(matches!(
        &child_work.kind,
        ConcreteLifecycleWorkKind::DurableLiveWalApply(_)
    ));
    let cleanup = holder
        .prepare_ready_live_decision_apply_reconciliation(&coordinator, child_ordinal)
        .expect("attest the exact dedicated live Apply carrier")
        .expect("live Apply projects queue-inert cleanup authority");
    assert_eq!(
        cleanup.dispatch_key().lineage(),
        LifecycleDecisionApplyLineageV1::Live
    );
    assert_eq!(cleanup.validate_predecessor_ordinal(), lease.ordinal());
    assert_eq!(cleanup.subject(), subject);
    assert_eq!(cleanup.certificate(), &decision);
    assert_eq!(holder.registry_for_test().entries.len(), 1);
    assert_eq!(coordinator.high_water, child_ordinal);
    assert_eq!(
        coordinator.records[&lease.ordinal()].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    assert_eq!(
        coordinator.durable_records[&lease.ordinal()].continuation,
        super::super::schema::DurableContinuation::successor(
            super::super::schema::DurableContinuationEdge::ValidateToApply,
            child_ordinal,
        )
    );
    assert_eq!(
        coordinator.records[&child_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(
        coordinator.records[&child_ordinal].stage.kind(),
        LifecycleStageKind::ApplyDecision
    );
    if exercise_lineage_matrix {
        assert_lifecycle_decision_apply_live_recovered_substitution_matrix(
            &fixture.verified,
            &mut holder,
            &mut coordinator,
            child_ordinal,
            child_address,
            tag,
            ledger_directory.path(),
            adapter,
            startup,
            cleanup,
            live_validate_dispatch_key,
            recovered_validate_retry_census,
            _directory.path(),
        );
        return;
    }
    assert!(
        holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &coordinator)
    );
    let mut tampered_coordinator = coordinator.clone();
    let DurablePayloadReference::BodyFrame(mut tampered_frame) = tampered_coordinator
        .durable_records
        .get(&child_ordinal)
        .expect("actor-global Apply child retains durable metadata")
        .payload
    else {
        panic!("actor-global Apply child retains one body-frame payload")
    };
    let first_tamper = LifecycleDigest::new([0xA6; 32]);
    tampered_frame.frame = if tampered_frame.frame == first_tamper {
        LifecycleDigest::new([0x6A; 32])
    } else {
        first_tamper
    };
    tampered_coordinator
        .durable_records
        .get_mut(&child_ordinal)
        .expect("tamper only the copied actor-global Apply metadata")
        .payload = DurablePayloadReference::BodyFrame(tampered_frame);
    assert!(
        !holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &tampered_coordinator)
    );
    assert_eq!(
        runtime_ordinals
            .next_ordinal_for_test()
            .expect("inspect committed actor-global cursor"),
        Some(9)
    );
    let (_, reopened) =
        super::super::ledger::LifecycleLedgerStoreV1::open(ledger_directory.path(), active_context)
            .expect("reopen exact committed Validate-to-Apply LedgerV1");
    assert_eq!(reopened.high_water(), child_ordinal);
    assert_eq!(reopened.records().len(), 2);

    // Continue the same actor-global successor through the production
    // cleanup, capacity, queue, guarded completion, LedgerV1, and registry
    // terminal seams. The worker fixture preserves the real bounded queue and
    // tracker transitions while supplying only the structurally authenticated
    // Kura terminal; State/Kura execution is covered by the apply-service and
    // four-peer acceptance lanes.
    let payload_directory = TempDir::new().expect("temporary live Apply payload store");
    let (payload_store, serve_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            payload_directory.path(),
            fixture.verified.context(),
        )
        .expect("open empty live Apply Serve payload owner");
    let mut body_store = V2BodyStore::open(_directory.path(), fixture.verified.context().clone())
        .expect("reopen exact live Apply body store");
    body_store
        .revalidate_recovered_markers(|_| {
            Ok::<_, String>(cleanup.validated_receipt().execution_commitment())
        })
        .expect("semantically revalidate exact live Apply body marker");
    let mut owner = super::super::ProductionLifecycleOwnerV1 {
        verified: fixture.verified.clone(),
        coordinator,
        registry: holder,
        recovered_lifecycle_outputs: None,
        payload_store,
        serve_payloads,
        body_store: Some(body_store),
        body_store_identity: None,
        kura_binding: None,
        apply_service: None,
        adapter_startup: None,
        timeout_supersession_successor: None,
    };
    let runtime = crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
        adapter,
        startup,
        std::time::Instant::now(),
        std::time::Duration::from_secs(10),
        crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap exact live Apply adapter")
    .0;
    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let (mut executor, mut planner_io) = owner
        .bind_body_store_to_lifecycle_completion_io_with_validate_retry_census_for_test(
            &mut services,
            runtime,
            std::sync::Arc::clone(&output_guard),
            0,
            2,
            recovered_validate_retry_census,
        );
    let live_started_at = std::time::Instant::now();
    executor
        .arm_live_clocks(
            super::super::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            live_started_at,
        )
        .expect("arm exact live Apply clocks after service construction");
    assert_eq!(
        executor
            .reconcile_reopened_decision_for_lifecycle_apply_lineage_test(&mut services, false)
            .expect("reconcile exact live Apply Decision into the executor"),
        (
            decision.round,
            decision.proposal_round,
            subject,
            decision.execution_commitment,
        )
    );
    assert!(
        executor
            .ready_to_finish_blockers()
            .contains(&"durable-validate-retry-seal"),
        "the synchronous Apply cleanup fixture must begin with its live Validate retry ordinal"
    );
    executor
        .reconcile_live_lifecycle_decision_apply(cleanup, &mut services)
        .expect("install exact live Apply owner before the normal Ready scheduler gate");
    assert!(
        !executor
            .ready_to_finish_blockers()
            .contains(&"durable-validate-retry-seal"),
        "synchronous Apply reconciliation must release its authenticated Validate predecessor"
    );
    let repeated_cleanup = owner
        .registry
        .prepare_ready_live_decision_apply_reconciliation(&owner.coordinator, child_ordinal)
        .expect("reattest the unchanged Ready live Apply carrier")
        .expect("unchanged live Apply projects repeatable cleanup authority");
    executor
        .reconcile_live_lifecycle_decision_apply(repeated_cleanup, &mut services)
        .expect("repeat exact live Apply reconciliation after retry release");
    assert!(
        !executor
            .ready_to_finish_blockers()
            .contains(&"durable-validate-retry-seal")
    );

    planner_io.saturate_consensus_prefix(&services);
    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("clean live Apply before the frozen queue census"),
        super::super::ProductionCompletionDispatchV1::CapacityUnavailable {
            protected_live_apply_ordinal: Some(child_ordinal),
        }
    );
    let barrier_key = executor
        .live_lifecycle_decision_apply_key_for_test()
        .expect("capacity retry retains the exact live Apply retransmit owner");
    assert_eq!(barrier_key.lineage(), LifecycleDecisionApplyLineageV1::Live);
    assert_eq!(barrier_key.lifecycle_ordinal(), child_ordinal);
    let live_work = owner
        .registry
        .registry_for_test()
        .entries
        .get(&child_address)
        .expect("capacity retry retains the exact live Apply carrier");
    let ConcreteLifecycleWorkKind::DurableLiveWalApply(live_apply) = &live_work.kind else {
        panic!("capacity retry changed the dedicated live Apply carrier")
    };
    assert!(live_apply.dispatch_key.is_none());
    assert_eq!(planner_io.queued_lifecycle_decision_apply_count(), 0);

    planner_io.release_all_predecessors();
    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("claim and queue the exact live Apply carrier"),
        super::super::ProductionCompletionDispatchV1::ApplyQueued {
            ordinal: child_ordinal,
        }
    );
    assert_eq!(planner_io.queued_lifecycle_decision_apply_count(), 1);
    assert_eq!(
        executor.live_lifecycle_decision_apply_key_for_test(),
        Some(barrier_key)
    );
    executor
        .coalesce_live_lifecycle_apply_retransmit_for_test(
            tag,
            subject,
            decision.clone(),
            &mut services,
        )
        .expect("exact due Apply retransmit coalesces on the live lifecycle owner");
    assert_eq!(planner_io.queued_lifecycle_decision_apply_count(), 1);
    assert_eq!(executor.status().pending_applications, 0);

    planner_io.execute_one_lifecycle_decision_apply_fixture(std::sync::Arc::clone(&output_guard));
    let completion = match services
        .take_next_lifecycle_completion()
        .expect("take exact guarded live Apply completion")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::Apply(completion) => completion,
        _ => panic!("live Apply queue produced a foreign completion class"),
    };
    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(applied) =
        completion.result()
    else {
        panic!("live Apply fixture must produce an applied terminal")
    };
    let lease = owner
        .coordinator
        .active_lease
        .clone()
        .expect("queued live Apply retains its exact active lease");
    let (transition, authority) = owner
        .registry
        .prepare_lifecycle_decision_apply_terminal_transition(&owner.coordinator, &lease, applied)
        .expect("join exact live worker result to its installed carrier");
    let adapter = executor
        .prepare_lifecycle_decision_apply_completion(authority)
        .expect("preview exact live Apply completion on the serialized adapter");
    let mut staged = owner.coordinator.stage_durable_transaction();
    staged.reduce_settle_turn(lease.clone(), super::super::TurnOutcome::Advanced, None);
    assert!(staged.fault.is_none());
    owner
        .registry
        .publish_lifecycle_decision_apply_terminal_transition(
            transition,
            &owner.coordinator,
            &staged,
            &lease,
            || owner.coordinator.persist_exact_staged_successor(&staged),
        )
        .unwrap_or_else(|_| panic!("publish exact live Apply terminal through LedgerV1"));
    owner.coordinator = staged;
    let finality = adapter.commit_after_durable_settlement();
    let status = executor.commit_lifecycle_decision_apply_finality(finality);
    let settled = completion.acknowledge_after_owner_settlement();
    assert!(matches!(
        settled,
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(_)
    ));
    assert_eq!(status.height, fixture.verified.context().height);
    assert!(owner.registry.registry_for_test().entries.is_empty());
    assert_eq!(
        owner.coordinator.records[&child_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    assert!(owner.coordinator.active_lease.is_none());
    assert!(
        executor
            .live_lifecycle_decision_apply_key_for_test()
            .is_none()
    );
    assert!(executor.durable_finality().is_some());
    assert!(
        executor.ready_to_finish(),
        "terminal live Apply left rollover blockers: {:?}",
        executor.ready_to_finish_blockers()
    );
    let (_, terminal_ledger) =
        super::super::ledger::LifecycleLedgerStoreV1::open(ledger_directory.path(), active_context)
            .expect("reopen exact terminal live Apply LedgerV1");
    assert_eq!(terminal_ledger.high_water(), child_ordinal);
    assert_eq!(
        terminal_ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == child_ordinal)
            .and_then(super::super::ledger::LifecycleLedgerRecordV1::terminal),
        Some(Some(TerminalOutcome::Advanced))
    );
    assert!(!output_guard.restart_required());
    planner_io.detach(&mut services);
}

#[cfg(feature = "bls")]
fn claim_ready_apply_for_lineage_test(
    coordinator: &mut LifecycleCoordinator,
    ordinal: u128,
) -> TurnLease {
    assert!(coordinator.fault.is_none());
    assert!(coordinator.active_lease.is_none());
    assert!(coordinator.ready_index.remove(&ordinal));
    let id_value = coordinator
        .next_lease
        .expect("lineage fixture retains one next lease id");
    coordinator.next_lease = Some(
        id_value
            .checked_add(1)
            .expect("lineage fixture lease id remains bounded"),
    );
    let id = LeaseId(id_value);
    let record = coordinator
        .records
        .get_mut(&ordinal)
        .expect("lineage fixture retains its exact Ready Apply row");
    assert_eq!(record.work_class, LifecycleWorkClass::Apply);
    assert_eq!(record.state, LifecycleState::Ready);
    record.state = LifecycleState::Claimed(id);
    let lease = TurnLease {
        id,
        ordinal: record.ordinal,
        owner: record.owner,
        key: record.key,
        work_class: record.work_class,
        stage: record.stage,
        rank: super::super::schema::SchedulerRank::new(0, 0, 0, 0, 0, 0, 0, 0),
        physical_slots: record.physical_slots.clone(),
        output_reservation: None,
    };
    coordinator.active_lease = Some(lease.clone());
    lease
}

#[cfg(feature = "bls")]
fn assert_lifecycle_decision_apply_key_coordinates_are_closed(
    key: LifecycleDecisionApplyDispatchKeyV1,
    context: LifecycleContext,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    lineage: LifecycleDecisionApplyLineageV1,
) {
    assert!(key.matches_carrier(context, address, digest, lineage));

    let mut foreign_context = key;
    let first_context = LifecycleDigest::new([0x8F; 32]);
    foreign_context.context = if context.id() == first_context {
        LifecycleDigest::new([0x90; 32])
    } else {
        first_context
    };
    assert!(!foreign_context.matches_carrier(context, address, digest, lineage));
    foreign_context.context = context.id();
    assert_eq!(foreign_context, key);

    let mut foreign_height = key;
    foreign_height.height = context
        .height()
        .checked_add(1)
        .expect("lineage fixture height remains bounded");
    assert!(!foreign_height.matches_carrier(context, address, digest, lineage));
    foreign_height.height = context.height();
    assert_eq!(foreign_height, key);

    let mut foreign_owner = key;
    let mut root = CausalRoot::new(LifecycleDigest::new([0x91; 32]));
    if root == address.owner.causal_root() {
        root = CausalRoot::new(LifecycleDigest::new([0x92; 32]));
    }
    foreign_owner.owner = OwnerId::new(root, address.owner.first_admission_ordinal());
    assert!(!foreign_owner.matches_carrier(context, address, digest, lineage));

    let mut foreign_ordinal = key;
    foreign_ordinal.ordinal = address
        .ordinal
        .checked_add(1)
        .expect("lineage fixture ordinal remains bounded");
    assert!(!foreign_ordinal.matches_carrier(context, address, digest, lineage));

    let mut foreign_slot = key;
    foreign_slot.slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 1);
    assert!(!foreign_slot.matches_carrier(context, address, digest, lineage));

    let mut foreign_digest = key;
    let first_digest = LifecycleDigest::new([0x93; 32]);
    foreign_digest.digest = if digest == first_digest {
        LifecycleDigest::new([0x94; 32])
    } else {
        first_digest
    };
    assert!(!foreign_digest.matches_carrier(context, address, digest, lineage));

    let opposite = match lineage {
        LifecycleDecisionApplyLineageV1::Live => LifecycleDecisionApplyLineageV1::Recovered,
        LifecycleDecisionApplyLineageV1::Recovered => LifecycleDecisionApplyLineageV1::Live,
    };
    let foreign_lineage = key.with_lineage_for_test(opposite);
    assert_eq!(foreign_lineage.with_lineage_for_test(lineage), key);
    assert!(!foreign_lineage.matches_carrier(context, address, digest, lineage));
}

#[cfg(feature = "bls")]
fn project_live_apply_task_for_lineage_test(
    holder: &LifecycleWorkRegistryHolder,
    address: ConcreteWorkAddress,
    key: LifecycleDecisionApplyDispatchKeyV1,
) -> crate::sumeragi::v2_apply::LifecycleDecisionApplyTaskV1 {
    let work = &holder.registry_for_test().entries[&address];
    let ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) = &work.kind else {
        panic!("lineage fixture expected one genuine live Apply carrier")
    };
    apply
        .project_task(LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
            key,
        ))
        .expect("project exact live Apply task for lineage test")
}

#[cfg(feature = "bls")]
fn project_recovered_apply_task_for_lineage_test(
    holder: &LifecycleWorkRegistryHolder,
    address: ConcreteWorkAddress,
    key: LifecycleDecisionApplyDispatchKeyV1,
) -> crate::sumeragi::v2_apply::LifecycleDecisionApplyTaskV1 {
    let work = &holder.registry_for_test().entries[&address];
    let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
        panic!("lineage fixture expected one genuine recovered Apply carrier")
    };
    apply
        .carrier
        .project_recovered_apply_task(
            LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(key),
            address,
        )
        .expect("project exact recovered Apply task for lineage test")
}

#[cfg(feature = "bls")]
fn recovered_apply_validate_retry_census_for_test(
    holder: &LifecycleWorkRegistryHolder,
    address: ConcreteWorkAddress,
    key: LifecycleDecisionApplyDispatchKeyV1,
    validate_predecessor_ordinal: u128,
) -> RecoveredDurableValidateRetryCensusV1 {
    let task = project_recovered_apply_task_for_lineage_test(holder, address, key);
    let tag = task.exact_tag();
    let subject = task.subject();
    let certificate = task.certificate().clone();
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: certificate.proposal_round,
        subject,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: Some(certificate.clone()),
    };
    let validate = AdapterEffect::ValidateBody {
        tag,
        round: certificate.proposal_round,
        subject,
    };
    let ownership = crate::sumeragi::v2_runtime::bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&fetch),
        vec![
            crate::sumeragi::v2_runtime::RuntimeEffectOwnership::fresh_for_test(
                tag,
                validate_predecessor_ordinal,
            ),
        ],
    )
    .expect("bind the recovered Apply fixture's certified Fetch authority")
    .pop()
    .expect("one certified Fetch has one exact owner")
    .rebind_as_inherited_adapter_effect(&validate)
    .expect("carry certified Fetch authority into the recovered Validate predecessor");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&validate)
        .expect("seal the recovered Validate predecessor's exact pending binding");
    let retry_owner = RecoveredDurableValidateRetryOwnerV1::for_test(
        validate,
        task.validated_receipt().durable().clone(),
        &pending,
        validate_predecessor_ordinal,
        Some((
            certificate.round,
            certificate.proposal_round,
            subject,
            certificate.execution_commitment,
        )),
    )
    .expect("seal the recovered Apply fixture's exact Validate retry owner");
    RecoveredDurableValidateRetryCensusV1::from_admission_owner_for_test(retry_owner)
}

#[cfg(feature = "bls")]
fn recovered_decision_apply_validate_retry_retirement_fixture() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let (mut owner, _safety, storage) =
        crate::sumeragi::v2::recovered_decision_apply_owner_for_lineage_test(0xEA);
    let active_context = owner.coordinator.active_context;
    let (_, apply_ordinal) = owner
        .recovered_decision_apply_summary_for_test()
        .expect("genuine recovered owner retains one Ready Decision Apply");
    let apply_record = &owner.coordinator.records[&apply_ordinal];
    let (&apply_slot, _) = apply_record
        .physical_slots
        .first_key_value()
        .expect("recovered Apply retains its physical slot");
    let apply_address = ConcreteWorkAddress::new(apply_record.owner, apply_ordinal, apply_slot)
        .expect("recovered Apply address is exact");
    let predecessor_ordinals = owner
        .coordinator
        .records
        .iter()
        .filter_map(|(&ordinal, record)| {
            let continuation = owner
                .coordinator
                .durable_records
                .get(&ordinal)?
                .continuation;
            (record.owner == apply_address.owner
                && record.work_class == LifecycleWorkClass::Validate
                && continuation
                    == super::super::schema::DurableContinuation::successor(
                        super::super::schema::DurableContinuationEdge::ValidateToApply,
                        apply_ordinal,
                    ))
            .then_some(ordinal)
        })
        .collect::<Vec<_>>();
    let [validate_predecessor_ordinal] = predecessor_ordinals.as_slice() else {
        panic!("recovered Apply fixture lost its sole ValidateToApply predecessor")
    };
    let validate_predecessor_ordinal = *validate_predecessor_ordinal;
    let apply_key = owner
        .registry
        .attest_ready_lifecycle_decision_apply(&owner.coordinator, apply_ordinal)
        .expect("attest the genuine recovered Apply")
        .dispatch_key();
    let task =
        project_recovered_apply_task_for_lineage_test(&owner.registry, apply_address, apply_key);
    let decision = (
        task.certificate().round,
        task.certificate().proposal_round,
        task.subject(),
        task.certificate().execution_commitment,
    );
    let retry_key = (task.certificate().proposal_round, task.subject());
    let retry_census = recovered_apply_validate_retry_census_for_test(
        &owner.registry,
        apply_address,
        apply_key,
        validate_predecessor_ordinal,
    );

    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let (mut executor, mut planner_io) = owner.bind_recovered_apply_executor_for_lineage_test(
        &mut services,
        std::sync::Arc::clone(&output_guard),
        retry_census,
        2,
    );
    executor
        .reconcile_recovered_validate_retry_decision_for_test(decision, false, &mut services)
        .expect("reconcile the decided-body frontier without bypassing lifecycle-owned Apply");
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        vec![retry_key]
    );
    assert!(
        executor
            .ready_to_finish_blockers()
            .contains(&"durable-validate-retry-seal")
    );

    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("queue the genuine recovered Decision Apply"),
        super::super::ProductionCompletionDispatchV1::ApplyQueued {
            ordinal: apply_ordinal,
        }
    );
    planner_io.execute_one_lifecycle_decision_apply_fixture(std::sync::Arc::clone(&output_guard));
    let completion = match services
        .take_next_lifecycle_completion()
        .expect("take the guarded recovered Apply completion")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::Apply(completion) => completion,
        other => {
            drop(other);
            panic!("recovered Apply completion lost its dedicated queue class")
        }
    };
    assert!(matches!(
        super::super::settle_applied_live_lifecycle_decision_apply_completion_for_test(
            &mut owner,
            &mut executor,
            completion,
        ),
        Ok(super::super::ProductionLifecycleDecisionApplyCompletionV1::Applied)
    ));
    assert!(matches!(
        owner.coordinator.records[&apply_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    ));
    let (_, terminal_ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
        &storage.path().join("ledger"),
        active_context,
    )
    .expect("reopen the durably published recovered Apply terminal");
    assert_eq!(terminal_ledger.high_water(), apply_ordinal);
    assert_eq!(
        terminal_ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == apply_ordinal)
            .and_then(super::super::ledger::LifecycleLedgerRecordV1::terminal),
        Some(Some(TerminalOutcome::Advanced))
    );
    assert!(executor.durable_finality().is_some());
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        vec![retry_key],
        "the recovered retry owner remains only as the decided-body inert tombstone"
    );
    assert!(
        !executor
            .ready_to_finish_blockers()
            .contains(&"durable-validate-retry-seal"),
        "recovered Apply finality must retire its exact durable Validate predecessor"
    );
    assert!(
        executor.ready_to_finish(),
        "recovered Apply finality left rollover blockers: {:?}",
        executor.ready_to_finish_blockers()
    );
    assert!(!output_guard.restart_required());
    planner_io.detach(&mut services);
}

#[cfg(feature = "bls")]
fn lifecycle_ledger_frame_for_lineage_test(root: &std::path::Path) -> Vec<u8> {
    std::fs::read(root.join("lifecycle-ledger-v1.norito"))
        .expect("read lifecycle Decision Apply lineage fixture ledger")
}

#[cfg(feature = "bls")]
fn assert_executor_completion_lineage_substitution_is_inert(
    executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<
        crate::sumeragi::v2_runtime::SerializedV2Runtime,
    >,
    holder: &LifecycleWorkRegistryHolder,
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    exact: &crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1,
    opposite_lineage: LifecycleDecisionApplyLineageV1,
    validate_predecessor_ordinal: u128,
) {
    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(completion) =
        exact
    else {
        panic!("executor lineage substitution requires one exact Applied result")
    };
    let registry_before = format!("{:?}", holder.registry_for_test());
    let coordinator_before = format!("{coordinator:?}");
    let (transition, authority) = holder
        .prepare_lifecycle_decision_apply_terminal_transition(coordinator, lease, completion)
        .expect("project exact completion authority before lineage substitution");
    assert_eq!(
        authority.validate_predecessor_ordinal(),
        validate_predecessor_ordinal,
        "completion authority must retain its registry-authenticated Validate predecessor"
    );
    drop(transition);
    executor.assert_lifecycle_apply_completion_lineage_substitution_is_inert_for_test(
        authority,
        opposite_lineage,
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);

    let (transition, exact_authority) = holder
        .prepare_lifecycle_decision_apply_terminal_transition(coordinator, lease, completion)
        .expect("reproject fresh exact completion authority after inert substitution");
    drop(transition);
    let exact_preview = executor
        .prepare_lifecycle_decision_apply_completion(exact_authority)
        .unwrap_or_else(|error| {
            panic!("fresh exact completion authority must still preview: {error}")
        });
    drop(exact_preview);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
}

#[cfg(feature = "bls")]
fn assert_terminal_lineage_substitution_is_inert(
    holder: &mut LifecycleWorkRegistryHolder,
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    exact: &crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1,
    opposite: &crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1,
    opposite_lineage: LifecycleDecisionApplyLineageV1,
    ledger_root: &std::path::Path,
) {
    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(
        opposite_completion,
    ) = opposite
    else {
        panic!("terminal lineage substitution requires one applied result")
    };
    let registry_before = format!("{:?}", holder.registry_for_test());
    let coordinator_before = format!("{coordinator:?}");
    let ledger_before = lifecycle_ledger_frame_for_lineage_test(ledger_root);
    assert!(
        holder
            .prepare_lifecycle_decision_apply_terminal_transition(
                coordinator,
                lease,
                opposite_completion,
            )
            .is_none(),
        "an opposite-lineage completion cannot rejoin the claimed carrier"
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(
        lifecycle_ledger_frame_for_lineage_test(ledger_root),
        ledger_before
    );

    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(exact_completion) =
        exact
    else {
        panic!("terminal lineage fixture requires one exact applied result")
    };
    let (mut prepared, authority) = holder
        .prepare_lifecycle_decision_apply_terminal_transition(coordinator, lease, exact_completion)
        .expect("exact completion prepares one terminal transition");
    assert_eq!(
        authority.lineage(),
        exact_completion.dispatch_key().lineage()
    );
    drop(authority);
    prepared.substitute_lineage_for_test(opposite_lineage);
    let mut staged = coordinator.stage_durable_transaction();
    staged.reduce_settle_turn(lease.clone(), super::super::TurnOutcome::Advanced, None);
    assert!(staged.fault.is_none());
    let staged_before = format!("{staged:?}");
    let callback_called = Cell::new(false);
    let result = holder.publish_lifecycle_decision_apply_terminal_transition(
        prepared,
        coordinator,
        &staged,
        lease,
        || {
            callback_called.set(true);
            Ok::<(), &'static str>(())
        },
    );
    assert!(matches!(
        result,
        Err(LifecycleDecisionApplyTerminalPublicationErrorV1::Preflight(
            _
        ))
    ));
    assert!(!callback_called.get());
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(format!("{staged:?}"), staged_before);
    assert_eq!(
        lifecycle_ledger_frame_for_lineage_test(ledger_root),
        ledger_before
    );
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn assert_lifecycle_decision_apply_live_recovered_substitution_matrix(
    verified: &crate::sumeragi::v2::VerifiedHeightContext,
    live_holder: &mut LifecycleWorkRegistryHolder,
    live_coordinator: &mut LifecycleCoordinator,
    live_ordinal: u128,
    live_address: ConcreteWorkAddress,
    live_tag: crate::sumeragi::v2_core::EventTag,
    live_ledger_root: &std::path::Path,
    live_adapter: crate::sumeragi::v2::SumeragiV2Adapter,
    live_startup: Vec<AdapterEffect>,
    live_cleanup: LiveLifecycleDecisionApplyReconciliationAuthorityV1,
    _live_validate_dispatch_key: LifecycleValidateDispatchKeyV1,
    recovered_validate_retry_census: RecoveredDurableValidateRetryCensusV1,
    live_body_root: &std::path::Path,
) {
    let live_validate_predecessor_ordinal = live_cleanup.validate_predecessor_ordinal();
    let live_runtime = crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
        live_adapter,
        live_startup,
        std::time::Instant::now(),
        std::time::Duration::from_secs(10),
        crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap exact live Apply lineage adapter")
    .0;
    let mut live_body_store = V2BodyStore::open(live_body_root, verified.context().clone())
        .expect("reopen exact live Apply lineage body store");
    live_body_store
        .revalidate_recovered_markers(|_| {
            Ok::<_, String>(live_cleanup.validated_receipt().execution_commitment())
        })
        .expect("semantically revalidate exact live Apply lineage body marker");
    let live_body_store_identity = live_body_store.instance_identity();
    let live_output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut live_executor, live_body_store) =
        crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
            live_runtime,
            live_body_store,
            recovered_validate_retry_census,
            None,
            verified.context().clone(),
            verified.context().roster[0].validator.clone(),
            Some(0),
            std::sync::Arc::clone(&live_output_guard),
            crate::sumeragi::v2_effects::EffectQueueConfig::default(),
        )
        .expect("open exact live Apply lineage executor");
    let (mut live_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let live_planner_io = crate::sumeragi::v2_worker::tests::install_lifecycle_planner_io_for_test(
        &mut live_services,
        verified.context().clone(),
        std::sync::Arc::clone(&live_output_guard),
        live_body_store,
        live_body_store_identity,
        2,
    );
    let live_started_at = std::time::Instant::now();
    live_executor
        .arm_live_clocks(
            super::super::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            live_started_at,
        )
        .expect("arm exact live Apply lineage clocks after service construction");
    let live_certificate = live_cleanup.certificate();
    assert_eq!(
        live_executor.validate_retry_lifecycle_ordinal_for_test((
            live_certificate.proposal_round,
            live_cleanup.subject(),
        )),
        None,
        "cold lineage executor must not reconstruct a terminal Validate parent"
    );
    assert_eq!(
        live_executor
            .reconcile_reopened_decision_for_lifecycle_apply_lineage_test(&mut live_services, true,)
            .expect("reconcile exact live Apply Decision into the lineage executor"),
        (
            live_certificate.round,
            live_certificate.proposal_round,
            live_cleanup.subject(),
            live_certificate.execution_commitment,
        )
    );
    live_executor
        .reconcile_live_lifecycle_decision_apply(live_cleanup, &mut live_services)
        .expect("install exact live Apply executor owner before lineage substitution");

    let (mut recovered, _recovered_safety, recovered_storage) =
        crate::sumeragi::v2::recovered_decision_apply_owner_for_lineage_test(0xE8);
    let (_, recovered_ordinal) = recovered
        .recovered_decision_apply_summary_for_test()
        .expect("genuine recovered owner retains one Ready Decision Apply");
    let recovered_record = &recovered.coordinator.records[&recovered_ordinal];
    let recovered_view = recovered_record.key.round().view();
    let (&recovered_slot, &recovered_digest) = recovered_record
        .physical_slots
        .first_key_value()
        .expect("recovered Apply row retains one physical slot");
    let recovered_address =
        ConcreteWorkAddress::new(recovered_record.owner, recovered_ordinal, recovered_slot)
            .expect("recovered Apply address is exact");
    let recovered_validate_predecessors = recovered
        .coordinator
        .records
        .iter()
        .filter_map(|(&ordinal, record)| {
            let continuation = recovered
                .coordinator
                .durable_records
                .get(&ordinal)?
                .continuation;
            (record.owner == recovered_address.owner
                && record.work_class == LifecycleWorkClass::Validate
                && continuation
                    == super::super::schema::DurableContinuation::successor(
                        super::super::schema::DurableContinuationEdge::ValidateToApply,
                        recovered_ordinal,
                    ))
            .then_some(ordinal)
        })
        .collect::<Vec<_>>();
    let [recovered_validate_predecessor_ordinal] = recovered_validate_predecessors.as_slice()
    else {
        panic!("recovered lineage fixture lost its sole ValidateToApply predecessor")
    };
    let recovered_validate_predecessor_ordinal = *recovered_validate_predecessor_ordinal;
    let recovered_retry_key = recovered
        .registry
        .attest_ready_lifecycle_decision_apply(&recovered.coordinator, recovered_ordinal)
        .expect("attest the recovered Apply before binding its retry census")
        .dispatch_key();
    let recovered_validate_retry_census = recovered_apply_validate_retry_census_for_test(
        &recovered.registry,
        recovered_address,
        recovered_retry_key,
        recovered_validate_predecessor_ordinal,
    );
    let recovered_output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut recovered_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let (mut recovered_executor, recovered_planner_io) = recovered
        .bind_recovered_apply_executor_for_lineage_test(
            &mut recovered_services,
            std::sync::Arc::clone(&recovered_output_guard),
            recovered_validate_retry_census,
            2,
        );
    assert!(
        recovered_executor
            .live_lifecycle_decision_apply_key_for_test()
            .is_none(),
        "genuine recovered executor must not inherit the live Apply owner"
    );

    let live_ready_before = (
        format!("{live_coordinator:?}"),
        format!("{:?}", live_holder.registry_for_test()),
    );
    let live_record = &live_coordinator.records[&live_ordinal];
    let mut live_attestation = live_holder
        .attest_ready_lifecycle_decision_apply(live_coordinator, live_ordinal)
        .expect("attest genuine Ready live Apply carrier");
    let live_key = live_attestation.dispatch_key();
    assert_eq!(live_key.lineage(), LifecycleDecisionApplyLineageV1::Live);
    assert_eq!(
        live_executor.live_lifecycle_decision_apply_key_for_test(),
        Some(live_key),
        "live reconciliation and Ready attestation must retain the same complete key"
    );
    assert!(live_attestation.matches_ready_record(live_record));
    live_attestation
        .substitute_dispatch_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered);
    assert!(!live_attestation.matches_ready_record(live_record));
    assert_eq!(
        (
            format!("{live_coordinator:?}"),
            format!("{:?}", live_holder.registry_for_test()),
        ),
        live_ready_before,
        "Ready live carrier lineage substitution must be read-only"
    );

    let recovered_ready_before = (
        format!("{:?}", recovered.coordinator),
        format!("{:?}", recovered.registry.registry_for_test()),
    );
    let recovered_record = &recovered.coordinator.records[&recovered_ordinal];
    let mut recovered_attestation = recovered
        .registry
        .attest_ready_lifecycle_decision_apply(&recovered.coordinator, recovered_ordinal)
        .expect("attest genuine Ready recovered Apply carrier");
    let recovered_key = recovered_attestation.dispatch_key();
    assert_eq!(
        recovered_key.lineage(),
        LifecycleDecisionApplyLineageV1::Recovered
    );
    assert!(recovered_attestation.matches_ready_record(recovered_record));
    recovered_attestation
        .substitute_dispatch_lineage_for_test(LifecycleDecisionApplyLineageV1::Live);
    assert!(!recovered_attestation.matches_ready_record(recovered_record));
    assert_eq!(
        (
            format!("{:?}", recovered.coordinator),
            format!("{:?}", recovered.registry.registry_for_test()),
        ),
        recovered_ready_before,
        "Ready recovered carrier lineage substitution must be read-only"
    );

    let live_digest = live_holder.registry_for_test().entries[&live_address].digest;
    assert_lifecycle_decision_apply_key_coordinates_are_closed(
        live_key,
        live_coordinator.active_context,
        live_address,
        live_digest,
        LifecycleDecisionApplyLineageV1::Live,
    );
    assert_lifecycle_decision_apply_key_coordinates_are_closed(
        recovered_key,
        recovered.coordinator.active_context,
        recovered_address,
        recovered_digest,
        LifecycleDecisionApplyLineageV1::Recovered,
    );

    {
        let work = &live_holder.registry_for_test().entries[&live_address];
        let ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) = &work.kind else {
            panic!("live lineage fixture changed carrier kind")
        };
        assert!(
            apply
                .project_task(LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
                    live_key.with_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered),
                ))
                .is_none(),
            "live carrier must reject the opposite task constructor identity"
        );
    }
    {
        let work = &recovered.registry.registry_for_test().entries[&recovered_address];
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
            panic!("recovered lineage fixture changed carrier kind")
        };
        assert!(
            apply
                .carrier
                .project_recovered_apply_task(
                    LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
                        recovered_key.with_lineage_for_test(LifecycleDecisionApplyLineageV1::Live,),
                    ),
                    recovered_address,
                )
                .is_none(),
            "recovered carrier must reject the opposite task constructor identity"
        );
    }

    let live_lease = claim_ready_apply_for_lineage_test(live_coordinator, live_ordinal);
    let recovered_lease =
        claim_ready_apply_for_lineage_test(&mut recovered.coordinator, recovered_ordinal);
    let live_task = live_holder
        .prepare_lifecycle_decision_apply_dispatch(live_coordinator, &live_lease)
        .expect("prepare exact claimed live Apply")
        .commit_for_worker();
    let recovered_task = recovered
        .registry
        .prepare_lifecycle_decision_apply_dispatch(&recovered.coordinator, &recovered_lease)
        .expect("prepare exact claimed recovered Apply")
        .commit_for_worker();
    assert_eq!(live_task.dispatch_key(), live_key);
    assert_eq!(recovered_task.dispatch_key(), recovered_key);
    drop(live_task);
    drop(recovered_task);

    let live_exact =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            verified.context(),
            project_live_apply_task_for_lineage_test(live_holder, live_address, live_key),
        )
        .expect("build exact live Applied result");
    let live_opposite =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            verified.context(),
            project_live_apply_task_for_lineage_test(live_holder, live_address, live_key)
                .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered, live_tag),
        )
        .expect("build same-coordinate recovered-lineage result from live material");
    let live_deferred_task =
        project_live_apply_task_for_lineage_test(live_holder, live_address, live_key)
            .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered, live_tag);
    let live_deferred = crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Deferred {
        reference: detached_validation_merge_reference(
            live_deferred_task.validated_receipt().durable(),
        ),
        task: live_deferred_task,
    };
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        live_key,
        &live_opposite,
    );
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        live_key,
        &live_deferred,
    );

    let recovered_context = recovered.verified.context();
    let recovered_live_tag = crate::sumeragi::v2_core::EventTag::new(
        recovered_context.height,
        recovered_view,
        live_tag.generation(),
    );
    let recovered_exact =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            recovered_context,
            project_recovered_apply_task_for_lineage_test(
                &recovered.registry,
                recovered_address,
                recovered_key,
            ),
        )
        .expect("build exact recovered Applied result");
    let recovered_opposite =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            recovered_context,
            project_recovered_apply_task_for_lineage_test(
                &recovered.registry,
                recovered_address,
                recovered_key,
            )
            .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Live, recovered_live_tag),
        )
        .expect("build same-coordinate live-lineage result from recovered material");
    let recovered_deferred_task = project_recovered_apply_task_for_lineage_test(
        &recovered.registry,
        recovered_address,
        recovered_key,
    )
    .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Live, recovered_live_tag);
    let recovered_deferred =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Deferred {
            reference: detached_validation_merge_reference(
                recovered_deferred_task.validated_receipt().durable(),
            ),
            task: recovered_deferred_task,
        };
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        recovered_key,
        &recovered_opposite,
    );
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        recovered_key,
        &recovered_deferred,
    );

    assert_executor_completion_lineage_substitution_is_inert(
        &mut live_executor,
        live_holder,
        live_coordinator,
        &live_lease,
        &live_exact,
        LifecycleDecisionApplyLineageV1::Recovered,
        live_validate_predecessor_ordinal,
    );
    assert_eq!(
        live_executor.live_lifecycle_decision_apply_key_for_test(),
        Some(live_key),
        "live executor keeps its exact owner after lineage rejection and exact reprojection"
    );
    assert_executor_completion_lineage_substitution_is_inert(
        &mut recovered_executor,
        &recovered.registry,
        &recovered.coordinator,
        &recovered_lease,
        &recovered_exact,
        LifecycleDecisionApplyLineageV1::Live,
        recovered_validate_predecessor_ordinal,
    );
    assert!(
        recovered_executor
            .live_lifecycle_decision_apply_key_for_test()
            .is_none(),
        "recovered executor cannot acquire a live owner through authority substitution"
    );

    assert_terminal_lineage_substitution_is_inert(
        live_holder,
        live_coordinator,
        &live_lease,
        &live_exact,
        &live_opposite,
        LifecycleDecisionApplyLineageV1::Recovered,
        live_ledger_root,
    );
    assert_terminal_lineage_substitution_is_inert(
        &mut recovered.registry,
        &recovered.coordinator,
        &recovered_lease,
        &recovered_exact,
        &recovered_opposite,
        LifecycleDecisionApplyLineageV1::Live,
        &recovered_storage.path().join("ledger"),
    );
    live_planner_io.detach(&mut live_services);
    recovered_planner_io.detach(&mut recovered_services);
}
