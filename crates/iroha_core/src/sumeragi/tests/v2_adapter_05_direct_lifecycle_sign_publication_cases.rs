// Exact direct-validation Sign publication, WAL persistence and crash replay cases.

#[test]
fn direct_validation_persist_preview_binds_receipt_and_is_drop_inert() {
    let directory = TempDir::new().expect("temporary direct-validation directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, _durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB1);
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let fence_before = adapter.reducer_fence_generation;
    let wal_records_before = adapter.wal.recovered_records().len();
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable
    );
    assert!(
        adapter
            .registry
            .execution_commitments
            .get(&(core_round, core_subject))
            .is_none()
    );
    let DirectValidationSucceededPreparation::Persist(preview) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("preview exact successful validation")
    else {
        panic!("a current local candidate must stage one PrepareIntent persistence")
    };
    assert_eq!(
        preview
            .next_registry
            .execution_commitments
            .get(&(core_round, core_subject)),
        Some(&validated.execution_commitment())
    );
    assert_eq!(
        preview.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Validated
    );
    assert!(matches!(
        &preview.event,
        reducer::Event::ValidationCompleted {
            tag: event_tag,
            round,
            subject,
            valid: true,
        } if *event_tag == tag && *round == core_round && *subject == core_subject
    ));
    let reducer::Effect::Persist {
        tag: persist_tag,
        entry,
    } = &preview.persist_effect
    else {
        panic!("Persist classification must seal the exact core WAL effect")
    };
    assert_eq!(*persist_tag, tag);
    assert_eq!(
        preview.next_reducer.pending_persistence_record(),
        Some(entry.record())
    );
    assert_eq!(
        preview.next_fence_generation,
        fence_before
            .checked_add(1)
            .expect("fixture fence remains bounded")
    );
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_records_before);
    let DirectValidationSucceededPreparation::Persist(repeated) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("dropped preview leaves the exact validation executable")
    else {
        panic!("dropped preview must not consume the reducer transition")
    };
    drop(repeated);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_records_before);
}
#[test]
fn ready_validate_persist_publication_preflights_one_sign_and_is_drop_inert() {
    let directory = TempDir::new().expect("temporary Ready Validate publication directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, _durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB8);
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let fence_before = adapter.reducer_fence_generation;
    let wal_records_before = adapter.wal.recovered_records().len();
    let DirectValidationSucceededPreparation::Persist(preview) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("preview exact successful validation")
    else {
        panic!("a current local candidate must stage one PrepareIntent persistence")
    };
    let sealed = SealedReadyDurableValidateAdapterPreview(
        ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(preview),
    );
    let publication = sealed
        .preflight_publication()
        .expect("preflight exact persistence acknowledgement");
    assert_eq!(
        publication.kind(),
        ReadyDurableValidateAdapterPublicationKind::ValidatedPersist
    );
    let ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) = &publication.0
    else {
        panic!("validated Persist discriminator must retain its exact prepared state")
    };
    assert_eq!(prepared.expected_wal_sequence, 0);
    assert!(!prepared.encoded_wal_payload.is_empty());
    let mut encoded = prepared.encoded_wal_payload.as_slice();
    let envelope = WalEnvelopeV2::decode(&mut encoded).expect("decode preflighted WAL payload");
    assert!(encoded.is_empty());
    assert_eq!(envelope.protocol_version, wire::PROTOCOL_VERSION);
    assert_eq!(envelope.persistence_id, 1);
    assert!(matches!(envelope.record, WalRecordV2::PrepareIntent(_)));
    assert!(matches!(
        &prepared.validation_event,
        reducer::Event::ValidationCompleted {
            tag: event_tag,
            round,
            subject,
            valid: true,
        } if *event_tag == tag && *round == core_round && *subject == core_subject
    ));
    assert!(matches!(
        &prepared.persist_effect,
        reducer::Effect::Persist {
            tag: persist_tag,
            entry,
        } if *persist_tag == tag
            && matches!(entry.record(), reducer::WalRecord::PrepareIntent(_))
    ));
    assert!(matches!(
        &prepared.persisted_event,
        reducer::Event::Persisted { tag: event_tag, id }
            if *event_tag == tag && id.get() == 1
    ));
    assert!(matches!(
        &prepared.sign_core_effect,
        reducer::Effect::Sign {
            tag: sign_tag,
            message: reducer::SignableMessage::Vote(vote),
        } if *sign_tag == tag && vote.phase() == reducer::Phase::Prepare
    ));
    assert!(matches!(
        &prepared.sign_effect,
        AdapterEffect::Sign {
            tag: sign_tag,
            request: SignRequest::Vote(vote),
        } if *sign_tag == tag
            && vote.phase == wire::GlobalPhase::Prepare
            && vote.subject == manifest.subject
            && vote.execution_commitment == validated.execution_commitment()
            && vote.signature.is_empty()
    ));
    let exact_sign = prepared.sign_effect.clone();
    let mut foreign_sign = exact_sign.clone();
    let AdapterEffect::Sign {
        request: SignRequest::Vote(foreign_vote),
        ..
    } = &mut foreign_sign
    else {
        unreachable!("Persist publication retains one vote-sign effect")
    };
    foreign_vote.signature.push(0xFF);
    assert!(publication.matches_exact_successor_effect(&exact_sign));
    assert!(!publication.matches_exact_successor_effect(&foreign_sign));
    assert_eq!(
        prepared.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Validated
    );
    assert!(prepared.next_reducer.pending_persistence_record().is_none());
    assert!(matches!(
        prepared.next_reducer.awaiting_signature(),
        Some(reducer::SignableMessage::Vote(vote))
            if vote.phase() == reducer::Phase::Prepare
    ));
    assert_eq!(
        prepared
            .next_registry
            .execution_commitments
            .get(&(core_round, core_subject)),
        Some(&validated.execution_commitment())
    );
    assert_eq!(
        prepared.next_fence_generation,
        fence_before
            .checked_add(2)
            .expect("fixture fence remains bounded across preview and acknowledgement")
    );
    drop(publication);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_records_before);
}
#[test]
fn ready_validate_prepare_sign_uses_real_wal_and_retains_pre_wal_retry() {
    let directory = TempDir::new().expect("temporary Ready Validate Sign directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, _durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB9);
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let last_progress_before = adapter.last_progress;
    let fence_before = adapter.reducer_fence_generation;
    let (validate, validate_pending) =
        ordinary_validate_predecessor_for_test(tag, manifest.round, manifest.subject, 61_001);
    let DirectValidationSucceededPreparation::Persist(preview) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("preview exact successful validation")
    else {
        panic!("ordinary validation must stage PrepareIntent")
    };
    let publication = SealedReadyDurableValidateAdapterPreview(
        ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(preview),
    )
    .preflight_publication()
    .expect("preflight PrepareIntent publication");
    let exact_sign = match &publication.0 {
        ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) => {
            assert!(prepared.registered_prepare.is_none());
            prepared.sign_effect.clone()
        }
        _ => unreachable!("fixture retains one Persist publication"),
    };
    let (foreign_validate, foreign_pending) =
        ordinary_validate_predecessor_for_test(tag, manifest.round, subject(0xBA), 61_002);
    let publication = match publication.bind_validate_sign_predecessor(
        ReadyValidateSignPredecessorAuthority::for_test(&foreign_validate, &foreign_pending),
    ) {
        Ok(_) => panic!("foreign predecessor cannot bind the WAL Sign intent"),
        Err(publication) => publication,
    };
    let ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) = &publication.0
    else {
        unreachable!("failed pre-WAL join returns the exact publication")
    };
    assert!(prepared._adapter.wal.recovered_records().is_empty());
    assert!(prepared._adapter.pending_persistence_id.is_none());
    assert!(!prepared._adapter.fail_closed);
    let bound = publication
        .bind_validate_sign_predecessor(ReadyValidateSignPredecessorAuthority::for_test(
            &validate,
            &validate_pending,
        ))
        .unwrap_or_else(|_| panic!("returned publication remains exactly retryable"));
    assert!(bound.pre_wal_is_exact());
    let persisted = bound
        .append_live_wal()
        .unwrap_or_else(|_| panic!("append and fsync exact PrepareIntent"));
    assert_eq!(persisted.adapter.wal.recovered_records().len(), 1);
    assert_eq!(persisted.adapter.pending_persistence_id, Some(1));
    assert!(!persisted.adapter.fail_closed);
    assert!(
        persisted
            .persisted_sign
            .as_ref()
            .expect("post-WAL fixture retains its nested Sign seal")
            .exactly_matches_validate_sign_for_test(
                &exact_sign,
                validate_pending.causal_lifecycle_key(),
            )
    );
    assert!(matches!(
        persisted
            .next_reducer
            .as_ref()
            .expect("post-WAL fixture retains staged reducer state")
            .awaiting_signature(),
        Some(reducer::SignableMessage::Vote(vote))
            if vote.phase() == reducer::Phase::Prepare
    ));
    let committed_status = persisted
        .committed_status
        .as_ref()
        .expect("post-WAL fixture precomputes exact committed status");
    assert_eq!(committed_status.pending_persistence_id, None);
    assert_eq!(committed_status.phase, wire::SumeragiV2StatusPhase::Prepare);
    assert_eq!(
        committed_status.body_state,
        wire::SumeragiV2BodyState::Validated
    );
    assert!(matches!(
        committed_status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::BodyValidated,
            ..
        })
    ));
    drop(persisted);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.last_progress, last_progress_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(adapter.pending_persistence_id, Some(1));
    assert_eq!(adapter.wal.recovered_records().len(), 1);
    assert!(adapter.fail_closed);
}
#[test]
fn ready_validate_crash_after_wal_append_replays_exact_prepare_and_commit() {
    for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
        let directory = TempDir::new().expect("temporary post-append crash directory");
        let wal_path = directory.path().join("safety.wal");
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let (tag, manifest, _durable, validated) =
            advance_direct_validation_fixture_to_durable(&mut adapter, 0xBD);
        if phase == wire::GlobalPhase::Commit {
            let mut keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic crash-replay validator key")
                })
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            assert!(
                keys.iter()
                    .zip(&adapter.wire_context.roster)
                    .all(|(key, row)| { key.public_key() == row.validator.public_key() })
            );
            let preimage = wire::Vote {
                round: manifest.round,
                proposal_round: manifest.round,
                phase: wire::GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: validated.execution_commitment(),
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
            let prepare = wire::QuorumCertificate {
                round: manifest.round,
                proposal_round: manifest.round,
                phase: wire::GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: validated.execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                    &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                )
                .expect("authenticate the PrepareQC retained by the Commit WAL intent"),
            };
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP")
                })
                .collect::<Vec<_>>();
            verify_quorum_certificate(&adapter.wire_context, &prepare, &proofs)
                .expect("the crash fixture must retain a cryptographically valid PrepareQC");
            let observed = adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::QuorumCertificate(prepare),
                    ),
                ))
                .expect("register the exact PrepareQC before Commit validation");
            assert!(observed.effects().is_empty());
        }
        let reducer_before = adapter.reducer.clone();
        let registry_before = adapter.registry.clone();
        let progress_before = adapter.last_progress;
        let fence_before = adapter.reducer_fence_generation;
        let records_before = adapter.wal.recovered_records().len();
        let wal_before = std::fs::read(&wal_path).expect("read pre-append WAL");
        let (validate, pending) =
            ordinary_validate_predecessor_for_test(tag, manifest.round, manifest.subject, 61_004);
        let DirectValidationSucceededPreparation::Persist(preview) = adapter
            .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
            .expect("prepare exact validation intent")
        else {
            panic!("validation must stage the requested vote intent");
        };
        let publication = SealedReadyDurableValidateAdapterPreview(
            ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(preview),
        )
        .preflight_publication()
        .expect("preflight exact vote publication");
        let ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) =
            &publication.0
        else {
            unreachable!("Persist preview retains the vote publication");
        };
        let AdapterEffect::Sign {
            request: SignRequest::Vote(expected_vote),
            ..
        } = &prepared.sign_effect
        else {
            panic!("validation must prepare one vote Sign continuation");
        };
        let expected_vote = expected_vote.clone();
        assert_eq!(expected_vote.phase, phase);
        let Some(reducer::SignableMessage::Vote(expected_core_vote)) =
            prepared.next_reducer.awaiting_signature()
        else {
            panic!("preflight must retain the exact staged vote");
        };
        let expected_core_vote = *expected_core_vote;
        let expected_id = prepared.expected_wal_sequence + 1;
        let mut bound = publication
            .bind_validate_sign_predecessor(ReadyValidateSignPredecessorAuthority::for_test(
                &validate, &pending,
            ))
            .unwrap_or_else(|_| panic!("bind the exact Validate predecessor"));
        bound.crash_after_wal_append = true;
        let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _unpublished = bound.append_live_wal();
        }))
        .expect_err("crash before the append receipt can mint a live Sign seal");
        assert_eq!(
            crash.downcast_ref::<&str>().copied(),
            Some("injected Ready Validate crash after WAL append")
        );
        assert!(adapter.fail_closed);
        assert!(!adapter.ingress_ready());
        assert_eq!(adapter.pending_persistence_id, Some(expected_id));
        assert_eq!(adapter.reducer, reducer_before);
        assert_registry_eq(&adapter.registry, &registry_before);
        assert_eq!(adapter.last_progress, progress_before);
        assert_eq!(adapter.reducer_fence_generation, fence_before);
        assert_eq!(adapter.wal.recovered_records().len(), records_before + 1);
        let durable_wal = std::fs::read(&wal_path).expect("read fsynced crash boundary");
        assert_ne!(durable_wal, wal_before);
        drop(adapter);

        // Discard every live adapter and publication object. Two independent
        // opens must recover the same one unsigned vote without another append,
        // so neither a missing seal nor a repeated restart loses or duplicates
        // the durable intent.
        for _ in 0..2 {
            let (mut recovered, startup) =
                open_test(&directory).expect("replay the exact post-append crash boundary");
            assert!(recovered.ingress_ready());
            assert!(!recovered.fail_closed);
            assert!(recovered.pending_persistence_id.is_none());
            assert_eq!(recovered.wal.recovered_records().len(), records_before + 1);
            assert_eq!(
                recovered.reducer.durable_state().last_id().get(),
                expected_id
            );
            let durable = recovered.reducer.durable_state();
            let recovered_intent = if phase == wire::GlobalPhase::Prepare {
                durable.prepare_intent(expected_core_vote.round())
            } else {
                durable.commit_intent(expected_core_vote.round())
            };
            assert_eq!(recovered_intent, Some(expected_core_vote));
            let [
                AdapterEffect::Sign {
                    tag: recovered_tag,
                    request: SignRequest::Vote(vote),
                },
            ] = startup.as_slice()
            else {
                panic!("restart must recover exactly one vote Sign: {startup:?}");
            };
            assert_eq!(vote, &expected_vote);
            assert!(vote.signature.is_empty());
            let signed = recovered
                .signature_completed(*recovered_tag, vec![0xBE; 96])
                .expect("complete only the recovered vote intent");
            assert!(matches!(
                signed.effects(),
                [AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                    payload: wire::ConsensusMessageV2Payload::Vote(vote),
                    ..
                })] if vote.round == expected_vote.round
                    && vote.proposal_round == expected_vote.proposal_round
                    && vote.phase == phase
                    && vote.subject == expected_vote.subject
                    && vote.execution_commitment == expected_vote.execution_commitment
                    && vote.signer == expected_vote.signer
                    && vote.signature == vec![0xBE; 96]
            ));
            assert_eq!(recovered.wal.recovered_records().len(), records_before + 1);
            drop(recovered);
            assert_eq!(
                std::fs::read(&wal_path).expect("read WAL after fresh replay and signing"),
                durable_wal
            );
        }
    }
}
#[test]
fn ready_validate_commit_sign_uses_only_registered_prepare_capability() {
    let directory = TempDir::new().expect("temporary Ready Validate Commit directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, _durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xBB);
    let prepare = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xBB; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                prepare.clone(),
            )),
        ))
        .expect("register concurrent PrepareQC");
    assert!(observed.effects().is_empty());
    let wal_records_before = adapter.wal.recovered_records().len();
    let (validate, ordinary_pending) =
        ordinary_validate_predecessor_for_test(tag, manifest.round, manifest.subject, 61_003);
    let wal_before = std::fs::read(directory.path().join("safety.wal"))
        .expect("snapshot WAL before substituted registered-QC preflight");
    let DirectValidationSucceededPreparation::Persist(mut substituted) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("preview exact LockAndCommit for substitution check")
    else {
        panic!("concurrent PrepareQC must stage LockAndCommit")
    };
    let reducer::Effect::Persist { entry, .. } = &substituted.persist_effect else {
        unreachable!("successful validation stages one Persist effect")
    };
    let reducer::WalRecord::LockAndCommit {
        prepare: registered_prepare,
        ..
    } = entry.record()
    else {
        unreachable!("concurrent PrepareQC stages LockAndCommit")
    };
    let substituted_commitment = execution_commitment(0xBC);
    assert_ne!(substituted_commitment, prepare.execution_commitment);
    substituted
        .next_registry
        .certificates
        .get_mut(&registered_prepare.reference())
        .expect("staged registry retains its registered PrepareQC")
        .execution_commitment = substituted_commitment;
    let canonicalized = substituted
        .preflight_publication()
        .expect("preflight reconstructs the registered PrepareQC from reducer authority");
    assert_eq!(
        canonicalized
            .registered_prepare
            .as_ref()
            .map(|capability| &capability.prepare),
        Some(&prepare)
    );
    drop(canonicalized);
    assert_eq!(adapter.wal.recovered_records().len(), wal_records_before);
    assert!(adapter.pending_persistence_id.is_none());
    assert_eq!(
        std::fs::read(directory.path().join("safety.wal"))
            .expect("read WAL after substituted registered-QC preflight"),
        wal_before
    );
    let DirectValidationSucceededPreparation::Persist(preview) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("preview exact LockAndCommit validation")
    else {
        panic!("concurrent PrepareQC must stage LockAndCommit")
    };
    let publication = SealedReadyDurableValidateAdapterPreview(
        ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(preview),
    )
    .preflight_publication()
    .expect("preflight LockAndCommit publication");
    let exact_sign = match &publication.0 {
        ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) => {
            assert!(prepared.registered_prepare.is_some());
            assert!(matches!(
                &prepared.persist_effect,
                reducer::Effect::Persist { entry, .. }
                    if matches!(entry.record(), reducer::WalRecord::LockAndCommit { .. })
            ));
            prepared.sign_effect.clone()
        }
        _ => unreachable!("fixture retains one LockAndCommit publication"),
    };
    assert!(
        ordinary_pending
            .project_validate_sign_commit_successor(&validate, &exact_sign)
            .is_none(),
        "ordinary Validate cannot mint Commit without the opaque carrier"
    );
    let bound = publication
        .bind_validate_sign_predecessor(ReadyValidateSignPredecessorAuthority::for_test(
            &validate,
            &ordinary_pending,
        ))
        .unwrap_or_else(|_| panic!("registered carrier refines ordinary Validate"));
    let persisted = bound
        .append_live_wal()
        .unwrap_or_else(|_| panic!("append and fsync exact LockAndCommit"));
    let frame = persisted
        .adapter
        .wal
        .recovered_records()
        .last()
        .expect("one real LockAndCommit frame");
    let mut encoded = frame.payload();
    let envelope = WalEnvelopeV2::decode(&mut encoded).expect("decode persisted envelope");
    assert!(encoded.is_empty());
    assert!(matches!(
        envelope.record,
        WalRecordV2::LockAndCommit {
            prepare: persisted_prepare,
            vote,
        } if persisted_prepare == prepare
            && vote.phase == wire::GlobalPhase::Commit
            && vote.execution_commitment == validated.execution_commitment()
    ));
    assert!(
        persisted
            .persisted_sign
            .as_ref()
            .expect("post-WAL fixture retains its nested Sign seal")
            .exactly_matches_validate_sign_for_test(
                &exact_sign,
                ordinary_pending.causal_lifecycle_key(),
            )
    );
    drop(persisted);
    assert_eq!(
        adapter.wal.recovered_records().len(),
        wal_records_before + 1
    );
    assert!(adapter.fail_closed);
}
