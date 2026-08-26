fn advance_direct_validation_fixture_to_next_view(
    adapter: &mut SumeragiV2Adapter,
    manifest: &wire::PayloadManifest,
    durable: &DurableBodyReceipt,
    execution_commitment: wire::ExecutionCommitment,
    signature_marker: u8,
) -> reducer::EventTag {
    let previous = adapter.current_tag();
    let prepare = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![signature_marker; 96],
    };
    let timeout = wire::TimeoutCertificate {
        round: manifest.round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare.clone()),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![signature_marker; 96],
        }],
    };
    let entered = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                timeout,
            )),
        ))
        .expect("install the exact timeout certificate");
    let current = adapter.current_tag();
    assert_eq!(current.height(), previous.height());
    assert_eq!(current.view(), previous.view().checked_add(1).unwrap());
    assert!(current.strictly_advances(previous));
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::EnterView { tag, .. } if *tag == current
    )));
    assert!(entered.effects().iter().any(|effect| matches!(
        effect,
        AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if *tag == current
            && *round == manifest.round
            && *subject == manifest.subject
            && certificate == &prepare
    )));
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(current, manifest)
        .expect("prepare the protected body after EnterView")
    else {
        panic!("the protected missing body must stage one Store successor")
    };
    assert!(matches!(
        available.commit(),
        AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } if tag == current && round == manifest.round && subject == manifest.subject
    ));
    let DirectBodyStoredPreparation::Applied(stored) = adapter
        .prepare_direct_body_stored(current, manifest.round, manifest.subject, durable)
        .expect("prepare the protected durable body after EnterView")
    else {
        panic!("the protected available body must stage one Validate successor")
    };
    assert!(matches!(
        stored.commit(),
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } if tag == current && round == manifest.round && subject == manifest.subject
    ));
    current
}

#[test]
fn direct_certified_body_preview_is_inert_and_commits_one_store_successor() {
    let directory = TempDir::new().expect("temporary direct-completion directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let body_subject = subject(0xA1);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, body_subject))
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
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Missing
    );
    let DirectCertifiedBodyAvailablePreparation::Applied(preview) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("preview exact body completion")
    else {
        panic!("missing body work must preview one StoreBody successor")
    };
    assert!(matches!(
        preview.store_effect(),
        AdapterEffect::StoreBody {
            tag: effect_tag,
            round,
            subject: effect_subject,
        } if *effect_tag == tag && *round == manifest.round && *effect_subject == manifest.subject
    ));
    drop(preview);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Missing,
        "dropping the preview must not publish cloned reducer state"
    );
    let DirectCertifiedBodyAvailablePreparation::Applied(prepared) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare exact body completion again")
    else {
        panic!("unchanged body work must remain directly executable")
    };
    let store = prepared.commit();
    assert!(matches!(
        store,
        AdapterEffect::StoreBody {
            tag: effect_tag,
            round,
            subject: effect_subject,
        } if effect_tag == tag && round == manifest.round && effect_subject == manifest.subject
    ));
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Available
    );
    let DirectCertifiedBodyAvailablePreparation::Inactive(repeated) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("classify exact repeated completion")
    else {
        panic!("the exact repeated completion must stutter")
    };
    assert_eq!(
        repeated.disposition(),
        DirectCertifiedBodyAvailableInactive::Stutter(
            DirectCertifiedBodyAvailableStutter::Duplicate
        )
    );
    drop(repeated);
    let wire::ConsensusMessageV2Payload::Proposal(unowned) =
        proposal(&adapter.wire_context, proposer, subject(0xA3)).payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    let DirectCertifiedBodyAvailablePreparation::Inactive(unowned) = adapter
        .prepare_direct_certified_body_available(tag, &unowned.manifest)
        .expect("classify a body with no reducer work")
    else {
        panic!("unowned body completion must stutter")
    };
    assert_eq!(
        unowned.disposition(),
        DirectCertifiedBodyAvailableInactive::Stutter(
            DirectCertifiedBodyAvailableStutter::NoMatchingWork
        )
    );
    drop(unowned);
    let stale_tag = reducer::EventTag::new(
        tag.height(),
        tag.view(),
        reducer::Generation::new(
            tag.generation()
                .get()
                .checked_add(1)
                .expect("fixture generation remains bounded"),
        ),
    );
    let DirectCertifiedBodyAvailablePreparation::Inactive(superseded) = adapter
        .prepare_direct_certified_body_available(stale_tag, &manifest)
        .expect("classify a foreign reducer generation")
    else {
        panic!("foreign generation must be superseded")
    };
    assert_eq!(
        superseded.disposition(),
        DirectCertifiedBodyAvailableInactive::Superseded(reducer::IgnoreReason::StaleGeneration)
    );
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    recovered_broadcast_and_sign_projection_remains_affine_and_phase_body_bound
);
#[test]
fn direct_body_stored_preview_is_inert_and_commits_one_validate_successor() {
    let directory = TempDir::new().expect("temporary direct-body-stored directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let body_subject = subject(0xA4);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, body_subject))
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
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare exact BodyAvailable transition")
    else {
        panic!("missing body work must prepare one Store successor")
    };
    assert!(matches!(
        available.commit(),
        AdapterEffect::StoreBody {
            tag: effect_tag,
            round,
            subject: effect_subject,
        } if effect_tag == tag && round == manifest.round && effect_subject == body_subject
    ));
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(body_subject.encode()).into());
    let receipt = durable_body_receipt(&adapter, manifest.round, body_subject);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Available
    );
    let DirectBodyStoredPreparation::Applied(preview) = adapter
        .prepare_direct_body_stored(tag, manifest.round, body_subject, &receipt)
        .expect("preview exact durable-body completion")
    else {
        panic!("available body work must preview one ValidateBody successor")
    };
    assert!(matches!(
        preview.validate_effect(),
        AdapterEffect::ValidateBody {
            tag: effect_tag,
            round,
            subject: effect_subject,
        } if *effect_tag == tag && *round == manifest.round && *effect_subject == body_subject
    ));
    drop(preview);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Available,
        "dropping the preview must not publish cloned reducer state"
    );
    let stale_tag = reducer::EventTag::new(
        tag.height(),
        tag.view(),
        reducer::Generation::new(
            tag.generation()
                .get()
                .checked_add(1)
                .expect("fixture generation remains bounded"),
        ),
    );
    let DirectBodyStoredPreparation::Inactive(stale) = adapter
        .prepare_direct_body_stored(stale_tag, manifest.round, body_subject, &receipt)
        .expect("classify a foreign reducer generation")
    else {
        panic!("foreign generation must be superseded")
    };
    assert_eq!(
        stale.disposition(),
        DirectBodyStoredInactive::Superseded(reducer::IgnoreReason::StaleGeneration)
    );
    drop(stale);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Available
    );
    let DirectBodyStoredPreparation::Applied(prepared) = adapter
        .prepare_direct_body_stored(tag, manifest.round, body_subject, &receipt)
        .expect("prepare exact durable-body completion again")
    else {
        panic!("unchanged durable body work must remain directly executable")
    };
    assert!(matches!(
        prepared.commit(),
        AdapterEffect::ValidateBody {
            tag: effect_tag,
            round,
            subject: effect_subject,
        } if effect_tag == tag && round == manifest.round && effect_subject == body_subject
    ));
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable
    );
    let DirectBodyStoredPreparation::Inactive(repeated) = adapter
        .prepare_direct_body_stored(tag, manifest.round, body_subject, &receipt)
        .expect("classify exact repeated durable-body completion")
    else {
        panic!("the exact repeated completion must stutter")
    };
    assert_eq!(
        repeated.disposition(),
        DirectBodyStoredInactive::Stutter(DirectBodyStoredStutter::Duplicate)
    );
    drop(repeated);
    let wire::ConsensusMessageV2Payload::Proposal(unowned) =
        proposal(&adapter.wire_context, proposer, subject(0xA8)).payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    let context = adapter.wire_context.clone();
    adapter
        .registry
        .manifest_to_core(&unowned.manifest, &context)
        .expect("register a valid manifest without reducer body work");
    let unowned_receipt = DurableBodyReceipt::for_test(
        context.id(),
        unowned.manifest.round,
        unowned.manifest.subject,
        HashOf::new(&unowned.manifest),
    );
    let reducer_before = adapter.reducer.clone();
    let DirectBodyStoredPreparation::Inactive(unowned) = adapter
        .prepare_direct_body_stored(
            tag,
            unowned.manifest.round,
            unowned.manifest.subject,
            &unowned_receipt,
        )
        .expect("classify durable body with no reducer work")
    else {
        panic!("unowned durable body must stutter")
    };
    assert_eq!(
        unowned.disposition(),
        DirectBodyStoredInactive::Stutter(DirectBodyStoredStutter::NoMatchingWork)
    );
    drop(unowned);
    assert_eq!(adapter.reducer, reducer_before);
}
#[test]
fn direct_body_stored_rejects_mismatched_receipts_without_mutation() {
    let directory = TempDir::new().expect("temporary direct-body-receipt directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let body_subject = subject(0xA5);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, body_subject))
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
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare exact BodyAvailable transition")
    else {
        panic!("missing body work must prepare one Store successor")
    };
    let _store = available.commit();
    let reducer_before = adapter.reducer.clone();
    let subjects_before = adapter.registry.subjects.clone();
    let manifests_before = adapter.registry.manifests.clone();
    let fence_before = adapter.reducer_fence_generation;
    let manifest_hash = HashOf::new(&manifest);
    let wrong_round = wire::ConsensusRound {
        view: manifest
            .round
            .view
            .checked_add(1)
            .expect("fixture view remains bounded"),
        ..manifest.round
    };
    let wrong_round_receipt = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        wrong_round,
        body_subject,
        manifest_hash,
    );
    assert!(matches!(
        adapter
            .prepare_direct_body_stored(tag, manifest.round, body_subject, &wrong_round_receipt,),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_subject_receipt = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        manifest.round,
        subject(0xA6),
        manifest_hash,
    );
    assert!(matches!(
        adapter.prepare_direct_body_stored(
            tag,
            manifest.round,
            body_subject,
            &wrong_subject_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let mut foreign_context = adapter.wire_context.clone();
    foreign_context.leader_seed[0] ^= 0x80;
    let foreign_context_receipt = DurableBodyReceipt::for_test(
        foreign_context.id(),
        manifest.round,
        body_subject,
        manifest_hash,
    );
    assert!(matches!(
        adapter.prepare_direct_body_stored(
            tag,
            manifest.round,
            body_subject,
            &foreign_context_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_manifest_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong durable-body manifest"));
    let wrong_manifest_receipt = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        manifest.round,
        body_subject,
        wrong_manifest_hash,
    );
    assert!(matches!(
        adapter.prepare_direct_body_stored(
            tag,
            manifest.round,
            body_subject,
            &wrong_manifest_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    assert_eq!(adapter.reducer, reducer_before);
    assert_eq!(adapter.registry.subjects, subjects_before);
    assert_eq!(adapter.registry.manifests, manifests_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
}
#[test]
fn direct_body_stored_busy_wait_and_max_fence_are_inert() {
    let directory = TempDir::new().expect("temporary direct-body-fence directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("status").leader;
    let body_subject = subject(0xA7);
    let fetch = adapter
        .receive_verified(proposal(&adapter.wire_context, proposer, body_subject))
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
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare exact BodyAvailable transition")
    else {
        panic!("missing body work must prepare one Store successor")
    };
    let _store = available.commit();
    let receipt = durable_body_receipt(&adapter, manifest.round, body_subject);
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(body_subject.encode()).into());
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
    let DirectBodyStoredPreparation::Blocked(wait) = adapter
        .prepare_direct_body_stored(tag, manifest.round, body_subject, &receipt)
        .expect("classify signature-fenced durable-body completion")
    else {
        panic!("active signature work must return an explicit reducer-fence wait")
    };
    assert_eq!(wait.context_id(), manifest.round.context_id);
    assert_eq!(wait.generation(), blocked_generation);
    drop(wait);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Available
    );
    adapter.reducer_fence_generation = u64::MAX;
    assert!(matches!(
        adapter.prepare_direct_body_stored(tag, manifest.round, body_subject, &receipt),
        Err(AdapterError::ReducerFenceGenerationExhausted)
    ));
    assert_eq!(adapter.reducer_fence_generation, u64::MAX);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Available
    );
}
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
#[test]
fn direct_validation_apply_preview_preserves_complete_decision_authority() {
    let directory = TempDir::new().expect("temporary direct-validation Apply directory");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0xB2);
    let leader = adapter.wire_context.leader(0);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) =
        proposal(&adapter.wire_context, leader, decided_subject).payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xB2; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install exact durable Decision");
    let tag = match decided.effects() {
        [
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                certificate: Some(certificate),
                ..
            },
        ] if *round == manifest.round
            && *subject == manifest.subject
            && certificate == &decision =>
        {
            *tag
        }
        effects => panic!("unexpected Decision recovery effects: {effects:?}"),
    };
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare decided BodyAvailable transition")
    else {
        panic!("missing decided body must stage StoreBody")
    };
    let _store = available.commit();
    let DirectBodyStoredPreparation::Applied(stored) = adapter
        .prepare_direct_body_stored(tag, manifest.round, manifest.subject, &durable)
        .expect("prepare decided BodyStored transition")
    else {
        panic!("available decided body must stage ValidateBody")
    };
    let _validate = stored.commit();
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let DirectValidationSucceededPreparation::Apply(preview) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("preview decided successful validation")
    else {
        panic!("the exact durable Decision must stage one Apply effect")
    };
    assert!(matches!(
        preview.apply_effect(),
        AdapterEffect::Apply {
            tag: effect_tag,
            subject,
            certificate,
        } if *effect_tag == tag && *subject == manifest.subject && certificate == &decision
    ));
    assert!(matches!(
        &preview.core_effect,
        reducer::Effect::Apply {
            tag: effect_tag,
            subject,
            certificate,
        } if *effect_tag == tag
            && *subject == core_subject
            && preview.next_reducer.durable_state().decision() == Some(certificate)
    ));
    assert_eq!(
        preview.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Validated
    );
    assert_eq!(
        preview
            .next_registry
            .execution_commitments
            .get(&(core_round, core_subject)),
        Some(&validated.execution_commitment())
    );
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable
    );
}

#[test]
fn stale_validation_completion_marks_the_exact_body_valid_before_a_later_decision() {
    let directory = TempDir::new().expect("temporary stale-validation directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (stale_tag, manifest, durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB3);
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let current_tag = advance_direct_validation_fixture_to_next_view(
        &mut adapter,
        &manifest,
        &durable,
        validated.execution_commitment(),
        0xB3,
    );
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable
    );

    let DirectValidationSucceededPreparation::Inactive(preview) = adapter
        .prepare_direct_validation_succeeded(
            stale_tag,
            manifest.round,
            manifest.subject,
            &validated,
        )
        .expect("rebind the exact stale validation completion")
    else {
        panic!("an old-round valid body must settle without current-view voting")
    };
    assert_eq!(
        preview.disposition(),
        &DirectValidationSucceededInactive::Superseded(reducer::IgnoreReason::IrrelevantView)
    );
    assert!(matches!(
        &preview.event,
        reducer::Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: true,
        } if *tag == current_tag && *round == core_round && *subject == core_subject
    ));
    assert_eq!(
        preview.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Validated
    );
    let publication = SealedReadyDurableValidateAdapterPreview(
        ReadyDurableValidateAdapterPreviewKind::ValidatedInactive(preview),
    )
    .preflight_publication()
    .expect("preflight the rebound no-successor publication");
    publication.commit_no_successor_after_durable_ledger();
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Validated
    );

    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xB3; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the later exact Decision");
    assert!(matches!(
        decided.effects(),
        [AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        }] if *tag == current_tag
            && *subject == manifest.subject
            && certificate == &decision
    ));
    assert!(
        adapter.pending_live_decision_apply.is_none(),
        "a persisted Decision which directly emits its exact Apply cannot leave a deferred WAL seal"
    );
}

#[test]
fn stale_validation_completion_uses_the_current_tag_when_decision_arrives_first() {
    let directory = TempDir::new().expect("temporary decided stale-validation directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (stale_tag, manifest, durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB4);
    let current_tag = advance_direct_validation_fixture_to_next_view(
        &mut adapter,
        &manifest,
        &durable,
        validated.execution_commitment(),
        0xB4,
    );
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xB4; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install the exact Decision before validation publication");
    assert!(
        decided.effects().is_empty(),
        "a Decision over an already Durable body must not race its retained validation work"
    );
    assert!(
        adapter.pending_live_decision_apply.is_some(),
        "a decided Durable body must retain its WAL seal for the Ready Validate join"
    );

    let DirectValidationSucceededPreparation::Apply(preview) = adapter
        .prepare_direct_validation_succeeded(
            stale_tag,
            manifest.round,
            manifest.subject,
            &validated,
        )
        .expect("rebind the decided stale validation completion")
    else {
        panic!("the rebound decided body must emit its sole Apply child")
    };
    assert!(matches!(
        &preview.event,
        reducer::Event::ValidationCompleted { tag, valid: true, .. } if *tag == current_tag
    ));
    assert!(matches!(
        preview.apply_effect(),
        AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } if *tag == current_tag && *subject == manifest.subject && certificate == &decision
    ));
}

#[test]
fn pending_kura_validated_apply_preview_rejects_foreign_authority_and_fence_exhaustion_inertly() {
    let directory = TempDir::new().expect("temporary pending-Kura preview directory");
    let wal_path = directory.path().join("leader-safety.wal");
    let (mut adapter, startup) = open_test_as_leader(&directory).expect("open leader");
    assert!(startup.is_empty());
    let decided_subject = subject(0xD2);
    let leader = adapter.wire_context.leader(0);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) =
        proposal(&adapter.wire_context, leader, decided_subject).payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    let manifest = proposal.manifest;
    let (durable, validated) = validated_receipts_for_manifest(&adapter.wire_context, &manifest);
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xD2; 96],
    };
    let decided = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("install exact durable Decision");
    let tag = match decided.effects() {
        [
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                certificate: Some(certificate),
                ..
            },
        ] if *round == manifest.round
            && *subject == manifest.subject
            && certificate == &decision =>
        {
            *tag
        }
        effects => panic!("unexpected Decision recovery effects: {effects:?}"),
    };
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare decided BodyAvailable transition")
    else {
        panic!("missing decided body must stage StoreBody")
    };
    let _store = available.commit();
    let DirectBodyStoredPreparation::Applied(stored) = adapter
        .prepare_direct_body_stored(tag, manifest.round, manifest.subject, &durable)
        .expect("prepare decided BodyStored transition")
    else {
        panic!("available decided body must stage ValidateBody")
    };
    let _validate = stored.commit();
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round: decision.proposal_round,
        subject: decision.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(decision.clone()),
    };
    let exact_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified_fetch),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                0xD2_01,
                b"pending Kura exact Commit Fetch",
            ),
        ],
    )
    .expect("bind exact Commit Fetch owner")
    .pop()
    .expect("one exact Commit Fetch owner")
    .rebind_as_inherited_adapter_effect(&validate_effect)
    .expect("carry exact Commit authority into Validate");
    assert!(exact_ownership.binds_durable_decision_authority(
        decision.round,
        decision.proposal_round,
        decision.subject,
        decision.execution_commitment,
    ));

    let mut foreign_decision = decision.clone();
    foreign_decision.execution_commitment = execution_commitment(0xD3);
    assert_ne!(
        foreign_decision.execution_commitment,
        decision.execution_commitment
    );
    let foreign_fetch = AdapterEffect::FetchBody {
        tag,
        round: decision.proposal_round,
        subject: decision.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(foreign_decision),
    };
    let foreign_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&foreign_fetch),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                0xD2_02,
                b"pending Kura foreign Commit Fetch",
            ),
        ],
    )
    .expect("bind foreign Commit Fetch owner")
    .pop()
    .expect("one foreign Commit Fetch owner")
    .rebind_as_inherited_adapter_effect(&validate_effect)
    .expect("carry foreign Commit authority into Validate");
    assert!(!foreign_ownership.binds_durable_decision_authority(
        decision.round,
        decision.proposal_round,
        decision.subject,
        decision.execution_commitment,
    ));

    adapter.reducer_fence_generation = u64::MAX;
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let last_progress_before = adapter.last_progress;
    let wal_before = std::fs::read(&wal_path).expect("snapshot pending-Kura preview WAL");
    let marker = DeferredPendingKuraValidatedMarkerV1::for_test(
        tag, &manifest, &durable, &validated, &decision,
    );
    let expected_marker = marker.clone();
    let Err((returned_marker, error)) =
        marker.prepare_apply(&mut adapter, &validate_effect, &foreign_ownership)
    else {
        panic!("foreign Commit authority must not prepare pending-Kura Apply")
    };
    assert!(matches!(
        error,
        AdapterError::RecoveredPendingKuraApplyMismatch
    ));
    assert_eq!(returned_marker, expected_marker);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, u64::MAX);
    assert_eq!(adapter.last_progress, last_progress_before);
    assert!(!adapter.fail_closed);
    assert_eq!(
        std::fs::read(&wal_path).expect("reread pending-Kura preview WAL"),
        wal_before
    );

    let marker = expected_marker.clone();
    let Err((returned_marker, error)) =
        marker.prepare_apply(&mut adapter, &validate_effect, &exact_ownership)
    else {
        panic!("exhausted reducer fence must not commit pending-Kura validation")
    };
    assert!(matches!(
        error,
        AdapterError::ReducerFenceGenerationExhausted
    ));
    assert_eq!(returned_marker, expected_marker);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, u64::MAX);
    assert_eq!(adapter.last_progress, last_progress_before);
    assert!(!adapter.fail_closed);
    assert_eq!(
        std::fs::read(&wal_path).expect("reread inert pending-Kura preview WAL"),
        wal_before
    );
}

#[test]
fn direct_validation_inactive_retains_ignored_state_change_and_commitment() {
    let directory = TempDir::new().expect("temporary direct-validation observer directory");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("observer-safety.wal"),
        verified_genesis(context()),
        None,
        reducer::Generation::new(1),
        [0xB3; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("open observer adapter");
    assert!(startup.is_empty());
    let proposer = adapter.status().expect("observer status").leader;
    let proposal_message = proposal(&adapter.wire_context, proposer, subject(0xB3));
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &proposal_message.payload else {
        unreachable!("observer fixture starts from one Proposal")
    };
    let manifest = proposal.manifest.clone();
    assert!(
        adapter
            .receive_verified(proposal_message)
            .expect("accept observer proposal")
            .effects()
            .is_empty(),
        "an observer does not fetch an uncertified proposal body"
    );
    let commitment = execution_commitment(0xB3);
    let prepare = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xB3; 96],
    };
    let fetch = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                prepare,
            )),
        ))
        .expect("accept observer PrepareQC")
        .into_effects();
    let tag = match fetch.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(fetched),
                ..
            },
        ] if fetched == &manifest => *tag,
        effects => panic!("unexpected observer certified-Fetch effects: {effects:?}"),
    };
    let DirectCertifiedBodyAvailablePreparation::Applied(available) = adapter
        .prepare_direct_certified_body_available(tag, &manifest)
        .expect("prepare observer BodyAvailable transition")
    else {
        panic!("certified observer body must prepare one Store successor")
    };
    assert!(matches!(
        available.commit(),
        AdapterEffect::StoreBody { .. }
    ));
    let durable = durable_body_receipt(&adapter, manifest.round, manifest.subject);
    let DirectBodyStoredPreparation::Applied(stored) = adapter
        .prepare_direct_body_stored(tag, manifest.round, manifest.subject, &durable)
        .expect("prepare observer BodyStored transition")
    else {
        panic!("stored observer body must prepare one Validate successor")
    };
    assert!(matches!(
        stored.commit(),
        AdapterEffect::ValidateBody { .. }
    ));
    let validated = ValidatedBodyReceipt::for_test_with_commitment(durable, commitment);
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let DirectValidationSucceededPreparation::Inactive(preview) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("classify observer validation")
    else {
        panic!("an observer must ignore successful validation without a child effect")
    };
    assert_eq!(
        preview.disposition(),
        &DirectValidationSucceededInactive::Superseded(reducer::IgnoreReason::Observer)
    );
    assert_eq!(
        preview.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Validated,
        "Ignored(Observer) still advances the staged reducer body state"
    );
    assert_eq!(
        preview
            .next_registry
            .execution_commitments
            .get(&(core_round, core_subject)),
        Some(&validated.execution_commitment())
    );
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable
    );
}
#[test]
fn direct_validation_busy_retains_commitment_and_rejects_reserved_fence() {
    let directory = TempDir::new().expect("temporary direct-validation Busy directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, _durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB4);
    let timeout_tag = adapter.current_tag();
    let sign = adapter
        .timeout_elapsed(timeout_tag)
        .expect("open exact signature fence")
        .into_effects();
    assert!(matches!(
        sign.as_slice(),
        [AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(_),
        }] if *tag == timeout_tag
    ));
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let generation = adapter.reducer_fence_generation;
    let DirectValidationSucceededPreparation::Busy(preview) = adapter
        .prepare_direct_validation_succeeded(tag, manifest.round, manifest.subject, &validated)
        .expect("classify signature-fenced validation")
    else {
        panic!("the active signature task must return Busy")
    };
    assert_eq!(preview.context_id(), manifest.round.context_id);
    assert_eq!(preview.generation(), generation);
    assert_ne!(preview.generation(), u64::MAX);
    assert_eq!(
        preview
            .next_registry
            .execution_commitments
            .get(&(core_round, core_subject)),
        Some(&validated.execution_commitment())
    );
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    adapter.reducer_fence_generation = u64::MAX;
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            &validated,
        ),
        Err(AdapterError::ReducerFenceGenerationExhausted)
    ));
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
}
#[test]
fn direct_validation_rejects_foreign_receipts_and_commitments_without_mutation() {
    let directory = TempDir::new().expect("temporary direct-validation receipt directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB5);
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let fence_before = adapter.reducer_fence_generation;
    let wal_records_before = adapter.wal.recovered_records().len();
    let manifest_hash = HashOf::new(&manifest);
    let mut foreign_context = adapter.wire_context.clone();
    foreign_context.leader_seed[0] ^= 0x40;
    let wrong_context = ValidatedBodyReceipt::for_test(DurableBodyReceipt::for_test(
        foreign_context.id(),
        manifest.round,
        manifest.subject,
        manifest_hash,
    ));
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_context,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_round = wire::ConsensusRound {
        view: manifest
            .round
            .view
            .checked_add(1)
            .expect("fixture view remains bounded"),
        ..manifest.round
    };
    let wrong_round_receipt = ValidatedBodyReceipt::for_test(DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        wrong_round,
        manifest.subject,
        manifest_hash,
    ));
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_round_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_subject_receipt = ValidatedBodyReceipt::for_test(DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        manifest.round,
        subject(0xB6),
        manifest_hash,
    ));
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_subject_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_manifest_receipt = ValidatedBodyReceipt::for_test(DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        manifest.round,
        manifest.subject,
        HashOf::from_untyped_unchecked(Hash::new(b"wrong validation manifest")),
    ));
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_manifest_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let unowned_subject = subject(0xB7);
    let unowned_round = manifest.round;
    let wire::ConsensusMessageV2Payload::Proposal(unowned_proposal) = proposal(
        &adapter.wire_context,
        adapter.wire_context.leader(0),
        unowned_subject,
    )
    .payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    let unowned = ValidatedBodyReceipt::for_test(DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        unowned_round,
        unowned_subject,
        HashOf::new(&unowned_proposal.manifest),
    ));
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(tag, unowned_round, unowned_subject, &unowned,),
        Err(AdapterError::MissingManifest)
    ));
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(adapter.wal.recovered_records().len(), wal_records_before);
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    adapter
        .registry
        .register_execution_commitment(core_round, core_subject, validated.execution_commitment())
        .expect("bind exact independent validation authority");
    let registry_with_commitment = adapter.registry.clone();
    let conflicting_commitment = execution_commitment(0xB8);
    assert_ne!(
        conflicting_commitment,
        validated.execution_commitment(),
        "fixture must exercise a genuine commitment conflict"
    );
    let conflicting =
        ValidatedBodyReceipt::for_test_with_commitment(durable, conflicting_commitment);
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            &conflicting,
        ),
        Err(AdapterError::ConflictingExecutionCommitment)
    ));
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_with_commitment);
    assert_eq!(adapter.wal.recovered_records().len(), wal_records_before);
    adapter.reducer_fence_generation = u64::MAX;
    assert!(matches!(
        adapter.prepare_direct_validation_succeeded(
            tag,
            manifest.round,
            manifest.subject,
            &validated,
        ),
        Err(AdapterError::ReducerFenceGenerationExhausted)
    ));
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_with_commitment);
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    direct_validation_preview_surface_is_closed_move_only_and_lifecycle_wired
);
#[test]
fn stale_failed_validation_rebinds_only_the_exact_durable_body() {
    let directory = TempDir::new().expect("temporary stale-rejection directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (stale_tag, manifest, durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xC0);
    let current_tag = advance_direct_validation_fixture_to_next_view(
        &mut adapter,
        &manifest,
        &durable,
        validated.execution_commitment(),
        0xC0,
    );
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());

    let DirectValidationFailedPreparation::NoEffect(preview) = adapter
        .prepare_direct_validation_failed(stale_tag, manifest.round, manifest.subject, &durable)
        .expect("rebind the exact stale deterministic rejection")
    else {
        panic!("an uncertified stale rejection must settle without a child effect")
    };
    assert!(matches!(
        &preview.event,
        reducer::Event::ValidationCompleted {
            tag,
            round,
            subject,
            valid: false,
        } if *tag == current_tag && *round == core_round && *subject == core_subject
    ));
    assert_eq!(
        preview.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Invalid
    );
    drop(preview);
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable,
        "dropping the preview cannot consume the exact body work"
    );

    let foreign_generation = reducer::EventTag::new(
        current_tag.height(),
        current_tag.view(),
        reducer::Generation::new(
            current_tag
                .generation()
                .get()
                .checked_add(1)
                .expect("fixture generation remains bounded"),
        ),
    );
    let DirectValidationFailedPreparation::Inactive(foreign) = adapter
        .prepare_direct_validation_failed(
            foreign_generation,
            manifest.round,
            manifest.subject,
            &durable,
        )
        .expect("classify a foreign validation generation")
    else {
        panic!("foreign generation cannot borrow the current reducer tag")
    };
    assert_eq!(
        foreign.disposition(),
        &DirectValidationFailedInactive::Superseded(reducer::IgnoreReason::StaleGeneration)
    );
    assert_eq!(
        foreign.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable
    );
}

#[test]
fn direct_validation_failed_no_effect_preview_is_drop_inert() {
    let directory = TempDir::new().expect("temporary direct-rejection directory");
    let wal_path = directory.path().join("safety.wal");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, durable, _validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xC1);
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let fence_before = adapter.reducer_fence_generation;
    let wal_before = std::fs::read(&wal_path).expect("snapshot rejection WAL");
    let DirectValidationFailedPreparation::NoEffect(preview) = adapter
        .prepare_direct_validation_failed(tag, manifest.round, manifest.subject, &durable)
        .expect("preview exact deterministic rejection")
    else {
        panic!("an uncertified local candidate must reject without a child effect")
    };
    assert_eq!(
        preview.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Invalid
    );
    assert_registry_eq(&preview.next_registry, &registry_before);
    assert!(matches!(
        &preview.event,
        reducer::Event::ValidationCompleted {
            tag: event_tag,
            round,
            subject,
            valid: false,
        } if *event_tag == tag && *round == core_round && *subject == core_subject
    ));
    assert_eq!(preview.next_fence_generation, fence_before);
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(
        std::fs::read(&wal_path).expect("read rejection WAL after drop"),
        wal_before
    );
    let stale_tag = reducer::EventTag::new(
        tag.height(),
        tag.view(),
        reducer::Generation::new(
            tag.generation()
                .get()
                .checked_add(1)
                .expect("fixture generation remains bounded"),
        ),
    );
    let DirectValidationFailedPreparation::Inactive(stale) = adapter
        .prepare_direct_validation_failed(stale_tag, manifest.round, manifest.subject, &durable)
        .expect("classify a foreign reducer generation")
    else {
        panic!("foreign generation must be retained as inactive")
    };
    assert_eq!(
        stale.disposition(),
        &DirectValidationFailedInactive::Superseded(reducer::IgnoreReason::StaleGeneration)
    );
    assert_eq!(stale.next_reducer, reducer_before);
    assert_registry_eq(&stale.next_registry, &registry_before);
    drop(stale);
    let DirectValidationFailedPreparation::NoEffect(repeated) = adapter
        .prepare_direct_validation_failed(tag, manifest.round, manifest.subject, &durable)
        .expect("dropped rejection remains exactly executable")
    else {
        panic!("a dropped rejection cannot consume durable body work")
    };
    drop(repeated);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(
        std::fs::read(&wal_path).expect("read rejection WAL after repeat"),
        wal_before
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn direct_validation_failed_report_carries_exact_registered_prepare_qc() {
    let directory = TempDir::new().expect("temporary certified-rejection directory");
    let wal_path = directory.path().join("safety.wal");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, durable, _validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xC2);
    let prepare = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: execution_commitment(0xC2),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xC2; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                prepare.clone(),
            )),
        ))
        .expect("durably observe exact PrepareQC");
    assert!(
        observed.effects().is_empty(),
        "a PrepareQC observed while its body is Durable has no external successor"
    );
    let core_round = reducer::Round::new(manifest.round.height, manifest.round.view);
    let core_subject = reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
    let reference = reducer::CertificateRef::new(
        adapter.reducer.context().id(),
        core_round,
        reducer::Phase::Prepare,
        core_subject,
    );
    assert_eq!(
        adapter.registry.certificates.get(&reference),
        Some(&prepare)
    );
    assert_eq!(
        adapter.reducer.body_state(core_round, core_subject),
        reducer::BodyState::Durable
    );
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let fence_before = adapter.reducer_fence_generation;
    let wal_before = std::fs::read(&wal_path).expect("snapshot certified-rejection WAL");
    let DirectValidationFailedPreparation::Report(preview) = adapter
        .prepare_direct_validation_failed(tag, manifest.round, manifest.subject, &durable)
        .expect("preview exact certified rejection")
    else {
        panic!("a pending PrepareQC must produce one exact rejection report")
    };
    assert!(matches!(
        preview.report_effect(),
        AdapterEffect::ReportInvalidCertifiedBody {
            subject,
            certificate,
        } if *subject == manifest.subject && certificate == &prepare
    ));
    assert!(matches!(
        &preview.core_effect,
        reducer::Effect::ReportInvalidCertifiedBody {
            subject,
            certificate,
        } if *subject == core_subject && certificate.reference() == reference
    ));
    assert!(matches!(
        &preview.event,
        reducer::Event::ValidationCompleted {
            tag: event_tag,
            round,
            subject,
            valid: false,
        } if *event_tag == tag && *round == core_round && *subject == core_subject
    ));
    assert_eq!(
        preview.next_reducer.body_state(core_round, core_subject),
        reducer::BodyState::Invalid
    );
    assert_eq!(
        preview.next_registry.certificates.get(&reference),
        Some(&prepare)
    );
    assert_eq!(preview.next_fence_generation, fence_before);
    let capability = preview
        .registered_prepare_report_capability()
        .expect("the exact post-rejection registry mints one Prepare capability");
    assert!(capability.exactly_matches_report(preview.report_effect()));
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let ordinary_store_pending = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&store_effect),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                0xC2_01,
                b"ordinary validation report",
            ),
        ],
    )
    .expect("bind one ordinary Store owner")
    .pop()
    .expect("one ordinary Store owner")
    .exact_pending_adapter_effect_binding(&store_effect)
    .expect("ordinary Store retains one pending binding");
    let ordinary_validate_pending = ordinary_store_pending
        .project_store_validate_successor(&store_effect, &validate_effect)
        .expect("ordinary Store projects its exact Validate owner");
    assert!(
        ordinary_validate_pending
            .project_validate_report_invalid_certified_body_successor(
                &validate_effect,
                preview.report_effect(),
            )
            .is_none(),
        "ordinary Validate cannot inherit a Prepare statement it never carried"
    );
    let ordinary_report_pending = ordinary_validate_pending
        .project_validate_report_invalid_certified_body_with_registered_prepare(
            &validate_effect,
            preview.report_effect(),
            &capability,
        )
        .expect("adapter-registered Prepare refines the exact ordinary Validate owner");
    assert!(ordinary_report_pending.exactly_binds_adapter_effect(preview.report_effect()));
    assert_eq!(
        ordinary_report_pending.causal_lifecycle_key(),
        ordinary_validate_pending.causal_lifecycle_key(),
        "the registered Prepare refinement must retain the remote/ordinary causal root"
    );
    let certified_fetch_effect = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(prepare.clone()),
    };
    let certified_fetch_pending = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified_fetch_effect),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                0xC2_02,
                b"certified validation report",
            ),
        ],
    )
    .expect("bind one Prepare-certified Fetch owner")
    .pop()
    .expect("one Prepare-certified Fetch owner")
    .exact_pending_adapter_effect_binding(&certified_fetch_effect)
    .expect("certified Fetch retains one pending binding");
    let certified_store_pending = certified_fetch_pending
        .project_certified_fetch_store_successor(&certified_fetch_effect, &store_effect)
        .expect("certified Fetch projects its exact Store owner");
    let certified_validate_pending = certified_store_pending
        .project_store_validate_successor(&store_effect, &validate_effect)
        .expect("certified Store projects its exact Validate owner");
    let inherited_report_pending = certified_validate_pending
        .project_validate_report_invalid_certified_body_successor(
            &validate_effect,
            preview.report_effect(),
        )
        .expect("Prepare-certified Validate inherits exact report authority");
    assert!(inherited_report_pending.exactly_binds_adapter_effect(preview.report_effect()));
    assert_eq!(
        inherited_report_pending.causal_lifecycle_key(),
        certified_validate_pending.causal_lifecycle_key()
    );
    assert_ne!(
        inherited_report_pending.causal_lifecycle_key(),
        ordinary_report_pending.causal_lifecycle_key(),
        "matching reports cannot splice distinct Validate causal roots"
    );
    let mut changed_commitment = prepare.clone();
    changed_commitment.execution_commitment = execution_commitment(0xC3);
    let changed_report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: manifest.subject,
        certificate: changed_commitment,
    };
    assert!(
        ordinary_validate_pending
            .project_validate_report_invalid_certified_body_with_registered_prepare(
                &validate_effect,
                &changed_report,
                &capability,
            )
            .is_none(),
        "the registry capability rejects a substituted QC commitment"
    );
    drop(capability);
    let revalidation_capability = preview
        .registered_prepare_report_capability()
        .expect("the retained preview may reproduce its private revalidation proof");
    assert!(revalidation_capability.exactly_matches_report(preview.report_effect()));
    drop(revalidation_capability);
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(
        std::fs::read(&wal_path).expect("read certified-rejection WAL after drop"),
        wal_before
    );
}
#[test]
fn direct_validation_failed_busy_retains_registry_and_rejects_reserved_fence() {
    let directory = TempDir::new().expect("temporary Busy rejection directory");
    let wal_path = directory.path().join("safety.wal");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, durable, _validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xC3);
    let timeout_tag = adapter.current_tag();
    let sign = adapter
        .timeout_elapsed(timeout_tag)
        .expect("open exact signature fence")
        .into_effects();
    assert!(matches!(
        sign.as_slice(),
        [AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(_),
        }] if *tag == timeout_tag
    ));
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let generation = adapter.reducer_fence_generation;
    let wal_before = std::fs::read(&wal_path).expect("snapshot Busy rejection WAL");
    let DirectValidationFailedPreparation::Busy(preview) = adapter
        .prepare_direct_validation_failed(tag, manifest.round, manifest.subject, &durable)
        .expect("classify signature-fenced rejection")
    else {
        panic!("the active signature task must return Busy")
    };
    assert_eq!(preview.context_id(), manifest.round.context_id);
    assert_eq!(preview.generation(), generation);
    assert_ne!(preview.generation(), u64::MAX);
    assert_registry_eq(&preview.next_registry, &registry_before);
    drop(preview);
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(
        std::fs::read(&wal_path).expect("read Busy rejection WAL after drop"),
        wal_before
    );
    adapter.reducer_fence_generation = u64::MAX;
    assert!(matches!(
        adapter.prepare_direct_validation_failed(tag, manifest.round, manifest.subject, &durable,),
        Err(AdapterError::ReducerFenceGenerationExhausted)
    ));
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, u64::MAX);
    assert_eq!(
        std::fs::read(&wal_path).expect("read reserved-fence rejection WAL"),
        wal_before
    );
}
#[test]
fn direct_validation_failed_rejects_foreign_receipts_without_mutation() {
    let directory = TempDir::new().expect("temporary rejection-receipt directory");
    let wal_path = directory.path().join("safety.wal");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    let (tag, manifest, _durable, _validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xC4);
    let reducer_before = adapter.reducer.clone();
    let registry_before = adapter.registry.clone();
    let fence_before = adapter.reducer_fence_generation;
    let wal_before = std::fs::read(&wal_path).expect("snapshot receipt-rejection WAL");
    let manifest_hash = HashOf::new(&manifest);
    let mut foreign_context = adapter.wire_context.clone();
    foreign_context.leader_seed[0] ^= 0x40;
    let wrong_context = DurableBodyReceipt::for_test(
        foreign_context.id(),
        manifest.round,
        manifest.subject,
        manifest_hash,
    );
    assert!(matches!(
        adapter.prepare_direct_validation_failed(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_context,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_round = wire::ConsensusRound {
        view: manifest
            .round
            .view
            .checked_add(1)
            .expect("fixture view remains bounded"),
        ..manifest.round
    };
    let wrong_round_receipt = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        wrong_round,
        manifest.subject,
        manifest_hash,
    );
    assert!(matches!(
        adapter.prepare_direct_validation_failed(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_round_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_subject_receipt = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        manifest.round,
        subject(0xC5),
        manifest_hash,
    );
    assert!(matches!(
        adapter.prepare_direct_validation_failed(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_subject_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let wrong_manifest_receipt = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        manifest.round,
        manifest.subject,
        HashOf::from_untyped_unchecked(Hash::new(b"wrong rejection manifest")),
    );
    assert!(matches!(
        adapter.prepare_direct_validation_failed(
            tag,
            manifest.round,
            manifest.subject,
            &wrong_manifest_receipt,
        ),
        Err(AdapterError::DurableBodyMismatch)
    ));
    let unowned_subject = subject(0xC6);
    let wire::ConsensusMessageV2Payload::Proposal(unowned_proposal) = proposal(
        &adapter.wire_context,
        adapter.wire_context.leader(0),
        unowned_subject,
    )
    .payload
    else {
        unreachable!("proposal helper returns a proposal")
    };
    let unowned = DurableBodyReceipt::for_test(
        adapter.wire_context.id(),
        unowned_proposal.manifest.round,
        unowned_subject,
        HashOf::new(&unowned_proposal.manifest),
    );
    assert!(matches!(
        adapter.prepare_direct_validation_failed(
            tag,
            unowned_proposal.manifest.round,
            unowned_subject,
            &unowned,
        ),
        Err(AdapterError::MissingManifest)
    ));
    assert_eq!(adapter.reducer, reducer_before);
    assert_registry_eq(&adapter.registry, &registry_before);
    assert_eq!(adapter.reducer_fence_generation, fence_before);
    assert_eq!(
        std::fs::read(&wal_path).expect("read receipt-rejection WAL after errors"),
        wal_before
    );
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    direct_validation_failed_surface_is_closed_move_only_and_lifecycle_wired
);
#[test]
#[allow(clippy::too_many_lines)]
fn ready_validate_adapter_bridge_is_sealed_and_live_sign_has_one_real_append() {
    let source = crate::sumeragi::v2_lifecycle_coordinator::reviewed_v2_adapter_source_for_test();
    let (production, _) = source
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("locate unconditional production/test boundary");
    assert_eq!(
        production
            .matches("fn prepare_sealed_ready_durable_validate_succeeded")
            .count(),
        1,
        "successful Ready Validate authority has one sealed adapter entry"
    );
    assert_eq!(
        production
            .matches("fn prepare_sealed_ready_durable_validate_failed")
            .count(),
        1,
        "rejected Ready Validate authority has one sealed adapter entry"
    );
    let token_start = production
        .find("// READY_DURABLE_VALIDATE_ADAPTER_PREVIEW_BEGIN")
        .expect("locate sealed Ready Validate adapter inventory");
    let token_end = production[token_start..]
        .find("// READY_DURABLE_VALIDATE_ADAPTER_PREVIEW_END")
        .map(|offset| token_start + offset)
        .expect("locate end of sealed Ready Validate adapter inventory");
    let tokens = &production[token_start..token_end];
    let live_start = tokens
        .find("// READY_DURABLE_VALIDATE_LIVE_SIGN_BEGIN")
        .expect("locate sealed live-Sign cut");
    let live_end = tokens
        .find("// READY_DURABLE_VALIDATE_LIVE_SIGN_END")
        .expect("locate end of sealed live-Sign cut");
    let live_sign = &tokens[live_start..live_end];
    let preflight_tokens = [&tokens[..live_start], &tokens[live_end..]].concat();
    assert!(tokens.contains("pub(crate) struct SealedReadyDurableValidateAdapterPreview"));
    assert!(tokens.contains("pub(crate) struct PreparedReadyDurableValidateAdapterPublication"));
    assert!(tokens.contains("pub(crate) enum ReadyDurableValidateAdapterPublicationKind"));
    assert!(tokens.contains("fn preflight_publication("));
    assert!(tokens.contains("project_invalid_body_report_candidate("));
    assert!(tokens.contains("permit: &SealedInvalidBodyReportProjectionPermit"));
    assert!(tokens.contains(".project_sealed_invalid_body_report_candidate("));
    assert!(!tokens.contains("projection::admission_request"));
    for outcome in [
        "ValidatedBusy",
        "ValidatedInactive",
        "ValidatedNoEffect",
        "ValidatedApply",
        "ValidatedPersist",
        "RejectedBusy",
        "RejectedInactive",
        "RejectedNoEffect",
        "RejectedReport",
    ] {
        assert!(tokens.contains(outcome), "missing closed outcome {outcome}");
    }
    for required in [
        "PreparedReadyDurableValidatePersistPublication",
        "reducer::WalRecord::PrepareIntent(vote)",
        "reducer::WalRecord::LockAndCommit { prepare, vote }",
        "vote.phase() == reducer::Phase::Commit",
        "next_registry.encode_wal_entry",
        "reducer::Event::Persisted",
        "continuation_effects.len() != 1",
        "reducer::SignableMessage::Vote(vote)",
        "expected_wal_sequence.checked_add(1)",
        "next_reducer.pending_persistence_record().is_some()",
    ] {
        assert!(
            tokens.contains(required),
            "Ready Validate publication preflight omitted {required}"
        );
    }
    assert!(!tokens.contains(
        "#[derive(Clone)]\npub(crate) struct PreparedReadyDurableValidateAdapterPublication"
    ));
    for forbidden in [
        "fn commit(",
        "fn install(",
        "into_parts",
        "PreparedReadyDurableValidateExecution",
        "ReadyValidatedAdapterAuthority",
        "ReadyRejectedAdapterAuthority",
        "ValidatedBodyReceipt::",
        "DurableBodyReceipt::",
        "rejection_reason",
        "wal.append(",
        "drive_effects(",
        "publish_status(",
        "fn encoded_wal_payload(",
        "fn sign_effect(",
        "fn persisted_event(",
    ] {
        assert!(
            !preflight_tokens.contains(forbidden),
            "sealed Ready Validate adapter token exposes forbidden surface {forbidden}"
        );
    }
    for required in [
        "PreparedReadyDurableValidateBoundSignPublication",
        "PreparedReadyDurableValidatePersistedSign",
        "ReadyValidateSignPredecessorAuthority",
        ".project_successor(&self.sign_effect, self.registered_prepare.as_ref())",
        "adapter.wal.append(&encoded_wal_payload)",
        "LiveWalFrameIdentity::from_append_receipt(frame, receipt, persistence_id)",
        "bind_exact_validate_sign_pending(child_pending)",
        "impl Drop for PreparedReadyDurableValidatePersistedSign",
        "self.adapter.fail_closed = true",
        "registry_work: Option<PreparedLiveValidateSignRegistryWork>",
        "committed_status: Option<wire::SumeragiV2Status>",
        "let committed_status = adapter.status()",
        "work.install_into(reservation)",
        "self.adapter.pending_persistence_id = None",
        "self.adapter.record_reducer_outcome(",
        ".log_body_progress(&validation_event, reducer::StepDisposition::Applied, 1)",
        "self.armed = false",
        "if self.adapter.status_publication_enabled",
        "super::status::set_v2_status(committed_status)",
    ] {
        assert!(
            live_sign.contains(required),
            "sealed live-Sign cut omitted {required}"
        );
    }
    assert_eq!(live_sign.matches(".wal.append(").count(), 1);
    let post_fsync = live_sign
        .split("fn install_registry_and_commit_adapter(")
        .nth(1)
        .and_then(|suffix| suffix.split("\n    }\n}").next())
        .expect("live Sign adapter publication has one bounded body");
    let registry_install = post_fsync
        .find("work.install_into(reservation)")
        .expect("reserved registry child is installed");
    let reducer_swap = post_fsync
        .find("self.adapter.reducer = next_reducer")
        .expect("adapter reducer is swapped");
    let marker_clear = post_fsync
        .find("self.adapter.pending_persistence_id = None")
        .expect("post-WAL marker is cleared");
    let disarm = post_fsync
        .find("self.armed = false")
        .expect("fail-stop Drop is disarmed before status publication");
    let publication_guard = post_fsync
        .find("if self.adapter.status_publication_enabled")
        .expect("preactivation keeps direct Ready Validate publication closed");
    let status_publish = post_fsync
        .find("super::status::set_v2_status(committed_status)")
        .expect("precomputed committed status is published last");
    assert!(
        registry_install < reducer_swap
            && reducer_swap < marker_clear
            && marker_clear < disarm
            && disarm < publication_guard
            && publication_guard < status_publish
    );
    for forbidden in ["?", "return Err", "publish_status(", ".wal.append("] {
        assert!(
            !post_fsync.contains(forbidden),
            "post-fsync adapter publication acquired fallible work through {forbidden}"
        );
    }
    for forbidden in [
        "LiveWalFrameIdentity::for_test",
        "fn into_parts(",
        "fn effect(",
        "fn pending(",
        "fn receipt(",
        "fn locator(",
        "fn commit(",
        "fn install(",
        "LifecycleCoordinator",
        "persist_durable_projection",
    ] {
        assert!(
            !live_sign.contains(forbidden),
            "sealed live-Sign cut exposed forbidden surface {forbidden}"
        );
    }
    let bridge_start = production
        .find("// READY_DURABLE_VALIDATE_ADAPTER_BRIDGE_BEGIN")
        .expect("locate Ready Validate adapter bridge");
    let bridge_end = production[bridge_start..]
        .find("// READY_DURABLE_VALIDATE_ADAPTER_BRIDGE_END")
        .map(|offset| bridge_start + offset)
        .expect("locate end of Ready Validate adapter bridge");
    let bridge = &production[bridge_start..bridge_end];
    assert!(bridge.contains("authority: ReadyValidatedAdapterAuthority<'_>"));
    assert!(bridge.contains("authority: ReadyRejectedAdapterAuthority<'_>"));
    assert_eq!(bridge.matches("authority.into_parts()").count(), 2);
    assert!(
        bridge.contains("self.prepare_direct_validation_succeeded_with_local_origin_manifest(")
    );
    assert!(bridge.contains("self.prepare_direct_validation_failed_with_local_origin_manifest("));
    assert_eq!(bridge.matches("local_origin_manifest.as_ref()").count(), 2);
    assert!(bridge.contains("SealedReadyDurableValidateAdapterPreview(match preview"));
    for forbidden in [
        "PreparedReadyDurableValidateExecution",
        "ReadyDurableValidateOutcomeKind",
        "with_validated_preview",
        "with_rejected_preview",
        "FnOnce",
        ".commit(",
        "wal.append(",
        "drive_effects(",
        "step_with_completion_evidence(",
        "EffectWorkId",
        "BodyValidationTask",
        "deferred_completions",
        "serviced_candidates",
        "producer_continuations",
        "LifecycleCoordinator",
        "v2_runtime",
        "rejection_reason",
        "reducer::Event::",
    ] {
        assert!(
            !bridge.contains(forbidden),
            "Ready Validate adapter bridge invokes forbidden machinery {forbidden}"
        );
    }
}
include!("v2_adapter_05_direct_lifecycle_recovered_wal_seal_case.rs");
