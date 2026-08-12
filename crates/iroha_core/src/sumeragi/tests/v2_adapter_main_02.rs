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
    let (tag, manifest, _durable, validated) =
        advance_direct_validation_fixture_to_durable(&mut adapter, 0xB3);
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

#[test]
fn direct_validation_preview_surface_is_closed_move_only_and_unwired() {
    let source = include_str!("../v2.rs");
    let (production, _) = source
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("locate unconditional production/test boundary");
    assert_eq!(
        production
            .matches("prepare_direct_validation_succeeded(")
            .count(),
        2,
        "only the private definition and sealed Ready-carrier bridge may name the preview"
    );

    let token_start = production
        .find("enum DirectValidationSucceededStutter")
        .expect("locate direct-validation token inventory");
    let token_end = production[token_start..]
        .find("// READY_DURABLE_VALIDATE_ADAPTER_PREVIEW_BEGIN")
        .map(|offset| token_start + offset)
        .expect("locate end of direct-validation token inventory");
    let tokens = &production[token_start..token_end];
    assert_eq!(
        tokens.matches("next_registry: WireRegistry").count(),
        5,
        "Busy, inactive, no-effect, Apply, and Persist must each retain the staged registry"
    );
    for outcome in [
        "Busy(PreparedDirectValidationSucceededBusy<'a>)",
        "Inactive(PreparedDirectValidationSucceededInactive<'a>)",
        "NoEffect(PreparedDirectValidationSucceededNoEffect<'a>)",
        "Apply(PreparedDirectValidationSucceededApply<'a>)",
        "Persist(PreparedDirectValidationSucceededPersist<'a>)",
    ] {
        assert!(tokens.contains(outcome), "missing closed outcome {outcome}");
    }
    assert!(tokens.contains("next_reducer: reducer::Reducer"));
    assert!(tokens.contains("persist_effect: reducer::Effect"));
    assert!(tokens.contains("apply_effect: AdapterEffect"));
    for forbidden in [
        "#[derive(Clone",
        "fn new(",
        "fn commit(",
        "fn install(",
        "into_parts",
        "Vec<u8>",
        "WalRecord",
        "BodyPipelineCompletionEvidence",
        "AdapterOutcome",
        "encode_wal_entry",
    ] {
        assert!(
            !tokens.contains(forbidden),
            "sealed direct-validation tokens expose forbidden surface {forbidden}"
        );
    }

    let method_start = production
        .find("fn prepare_direct_validation_succeeded(")
        .expect("locate direct-validation preview");
    let method_end = production[method_start..]
        .find("\n    /// Preview one exact deterministic rejection")
        .map(|offset| method_start + offset)
        .expect("locate end of direct-validation preview");
    let method = &production[method_start..method_end];
    for forbidden in [
        "drive_effects(",
        "wal.append(",
        "step_with_completion_evidence(",
        "deferred_completions",
        "serviced_candidates",
        "producer_continuations",
        "LifecycleCoordinator",
        "v2_runtime",
        ".commit(",
    ] {
        assert!(
            !method.contains(forbidden),
            "inert direct-validation preview invokes forbidden machinery {forbidden}"
        );
    }
    // The reducer's Applied-without-effect branch requires a production
    // body-work owner which is neither a candidate, pending PrepareQC,
    // observer, stale view, nor Decision. Existing adapter fixtures expose
    // no safe mint for that otherwise-closed internal shape; its enum arm
    // remains statically pinned without adding a test-only production bypass.
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

#[test]
fn direct_validation_failed_surface_is_closed_move_only_and_unwired() {
    let source = include_str!("../v2.rs");
    let (production, _) = source
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("locate unconditional production/test boundary");
    assert_eq!(
        production
            .matches("prepare_direct_validation_failed(")
            .count(),
        2,
        "only the private definition and sealed Ready-carrier bridge may name the preview"
    );

    let token_start = production
        .find("enum DirectValidationFailedStutter")
        .expect("locate direct-rejection token inventory");
    let token_end = production[token_start..]
        .find("/// Exact idempotent disposition of one direct successful-validation preview")
        .map(|offset| token_start + offset)
        .expect("locate end of direct-rejection token inventory");
    let tokens = &production[token_start..token_end];
    assert_eq!(
        tokens.matches("next_registry: WireRegistry").count(),
        4,
        "Busy, inactive, no-effect, and report must each retain the staged registry"
    );
    assert_eq!(
        tokens.matches("next_reducer: reducer::Reducer").count(),
        3,
        "every non-Busy rejection must retain the staged reducer"
    );
    for outcome in [
        "Busy(PreparedDirectValidationFailedBusy<'a>)",
        "Inactive(PreparedDirectValidationFailedInactive<'a>)",
        "NoEffect(PreparedDirectValidationFailedNoEffect<'a>)",
        "Report(PreparedDirectValidationFailedReport<'a>)",
    ] {
        assert!(tokens.contains(outcome), "missing closed outcome {outcome}");
    }
    assert!(tokens.contains("core_effect: reducer::Effect"));
    assert!(tokens.contains("report_effect: AdapterEffect"));
    for forbidden in [
        "#[derive(Clone",
        "fn new(",
        "fn commit(",
        "fn install(",
        "into_parts",
        "Vec<u8>",
        "encode_wal_entry",
    ] {
        assert!(
            !tokens.contains(forbidden),
            "sealed direct-rejection tokens expose forbidden surface {forbidden}"
        );
    }

    let method_start = production
        .find("fn prepare_direct_validation_failed(")
        .expect("locate direct-rejection preview");
    let method_end = production[method_start..]
        .find("\n    // READY_DURABLE_VALIDATE_ADAPTER_BRIDGE_BEGIN")
        .map(|offset| method_start + offset)
        .expect("locate end of direct-rejection preview");
    let method = &production[method_start..method_end];
    assert!(method.contains("durable_receipt: &DurableBodyReceipt"));
    assert!(method.contains("valid: false"));
    assert!(method.contains("wire::GlobalPhase::Prepare"));
    for forbidden in [
        "ValidatedBodyReceipt",
        "self.validation_failed(",
        "BodyPipelineCompletionEvidence",
        "AdapterOutcome",
        "WalRecord",
        "drive_effects(",
        "wal.append(",
        "step_with_completion_evidence(",
        "deferred_completions",
        "serviced_candidates",
        "producer_continuations",
        "LifecycleCoordinator",
        "v2_runtime",
        ".commit(",
    ] {
        assert!(
            !method.contains(forbidden),
            "inert direct-rejection preview invokes forbidden machinery {forbidden}"
        );
    }
}

#[test]
fn ready_validate_adapter_bridge_is_sealed_fixed_output_and_unwired() {
    let source = include_str!("../v2.rs");
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
    assert!(tokens.contains("pub(crate) struct SealedReadyDurableValidateAdapterPreview"));
    assert!(tokens.contains("pub(crate) struct PreparedReadyDurableValidateAdapterPublication"));
    assert!(tokens.contains("pub(crate) enum ReadyDurableValidateAdapterPublicationKind"));
    assert!(tokens.contains("fn preflight_publication("));
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
            !tokens.contains(forbidden),
            "sealed Ready Validate adapter token exposes forbidden surface {forbidden}"
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
    assert!(bridge.contains("self.prepare_direct_validation_succeeded("));
    assert!(bridge.contains("self.prepare_direct_validation_failed("));
    assert!(bridge.contains("SealedReadyDurableValidateAdapterPreview(match preview"));
    for forbidden in [
        "PreparedReadyDurableValidateExecution",
        "ReadyDurableValidateOutcomeKind",
        "with_validated_preview",
        "with_rejected_preview",
        "FnOnce",
        "-> R",
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
    let validation = adapter
        .validation_succeeded(fetch_tag, round, subject, &validated)
        .expect("validate the TC-protected body without relabelling its origin")
        .into_effects();
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
            generation: deferred_tag.generation(),
            locked_commit_progress: false,
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
    let DirectCertifiedBodyAvailablePreparation::Inactive(conflict) = adapter
        .prepare_direct_certified_body_available(deferred_tag, &canonical_manifest)
        .expect("classify the legacy deferred conflict without mutating it")
    else {
        panic!("legacy conflict must remain a non-applied classification")
    };
    assert_eq!(
        conflict.disposition(),
        DirectCertifiedBodyAvailableInactive::LegacyDeferredConflict
    );
    drop(conflict);
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
