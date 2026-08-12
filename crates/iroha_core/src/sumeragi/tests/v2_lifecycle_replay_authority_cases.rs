#[test]
fn every_stage_has_one_canonical_round_trip_and_exact_record_mapping() {
    let fixture = Fixture::new();
    assert_eq!(fixture.tag.generation(), 3);
    let cases = fixture.cases();
    assert_eq!(cases.len(), LifecycleStageKind::ALL.len());
    let stages = cases
        .iter()
        .map(|case| case.stage.kind())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        stages,
        LifecycleStageKind::ALL.into_iter().collect::<BTreeSet<_>>()
    );

    for case in cases {
        let encoded = case.authority.encode();
        assert!(encoded.len() <= MAX_REPLAY_AUTHORITY_BYTES);
        let decoded = LifecycleReplayAuthorityV1::decode_canonical(&encoded)
            .expect("canonical replay authority decodes");
        assert_eq!(decoded, case.authority);
        decoded
            .validate_record(
                fixture.context,
                case.key,
                case.work_class,
                case.stage,
                case.payload,
            )
            .expect("exact lifecycle row matches its replay envelope");
    }
}

#[test]
fn canonical_decoder_enforces_version_size_and_complete_input() {
    let fixture = Fixture::new();
    let mut authority = fixture
        .cases()
        .into_iter()
        .next()
        .expect("fixture has cases")
        .authority;
    assert_eq!(
        LifecycleReplayAuthorityV1::decode_canonical(&[]),
        Err(ReplayAuthorityCodecError::FrameBounds)
    );
    assert_eq!(
        LifecycleReplayAuthorityV1::decode_canonical(&vec![0; MAX_REPLAY_AUTHORITY_BYTES + 1]),
        Err(ReplayAuthorityCodecError::FrameBounds)
    );

    authority.format_version = REPLAY_AUTHORITY_FORMAT_VERSION + 1;
    assert_eq!(
        LifecycleReplayAuthorityV1::decode_canonical(&authority.encode()),
        Err(ReplayAuthorityCodecError::UnsupportedVersion)
    );

    authority.format_version = REPLAY_AUTHORITY_FORMAT_VERSION;
    let mut trailing = authority.encode();
    trailing.push(0);
    assert!(matches!(
        LifecycleReplayAuthorityV1::decode_canonical(&trailing),
        Err(ReplayAuthorityCodecError::InvalidEncoding
            | ReplayAuthorityCodecError::NonCanonicalEncoding)
    ));
}

#[test]
fn decoded_decision_fetch_rejects_duplicate_certified_sources() {
    let fixture = Fixture::new();
    let first_key = KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519)
        .expect("deterministic first Decision Fetch source");
    let second_key = KeyPair::try_from_seed(vec![0xD2; 32], Algorithm::Ed25519)
        .expect("deterministic second Decision Fetch source");
    let first = PeerId::new(first_key.public_key().clone());
    let second = PeerId::new(second_key.public_key().clone());
    let payload = ReplayPayloadBindingV1::None;
    let source = WalReplaySourceV1 {
        locator: RecoveredWalFrameIdentity::for_test(8, 9, [0xD3; 32]).persisted_locator(),
        role: ReplayWalRoleV1::DECISION,
        tag: fixture.tag,
        action: WalReplayActionV1::FetchDecision {
            certificate: fixture.commit_qc.clone(),
            certified_sources: vec![first.clone(), second],
        },
    };
    let shape = source
        .project(fixture.context, LifecycleStageKind::FetchBody, &payload)
        .expect("unique Decision Fetch source roster is structurally valid");
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload: payload.clone(),
        source: LifecycleReplaySourceV1::Wal(source),
    };
    let mut decoded = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode())
        .expect("unique Decision Fetch source decodes canonically");
    decoded
        .validate_record(
            fixture.context,
            shape.key,
            LifecycleWorkClass::Fetch,
            LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
            DurablePayloadReference::None,
        )
        .expect("unique Decision Fetch source projects exactly");

    let LifecycleReplaySourceV1::Wal(source) = &mut decoded.source else {
        unreachable!("Decision Fetch retains its WAL replay source")
    };
    let WalReplayActionV1::FetchDecision {
        certified_sources, ..
    } = &mut source.action
    else {
        unreachable!("Decision Fetch retains its exact replay action")
    };
    *certified_sources = vec![first.clone(), first];
    let duplicate = LifecycleReplayAuthorityV1::decode_canonical(&decoded.encode())
        .expect("duplicate source bytes remain canonically decodable");
    assert_eq!(
        duplicate.validate_record(
            fixture.context,
            shape.key,
            LifecycleWorkClass::Fetch,
            LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent,),
            DurablePayloadReference::None,
        ),
        Err(ReplayAuthorityValidationError::InvalidSource)
    );
}

#[test]
fn recovered_decision_body_lineage_is_stage_closed_and_predecessor_bound() {
    let fixture = Fixture::new();
    let source_key = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519)
        .expect("deterministic recovered Decision source");
    let locator = RecoveredWalFrameIdentity::for_test(21, 22, [0xD5; 32]).persisted_locator();
    let fetch = replay_case(
        fixture.context,
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role: ReplayWalRoleV1::DECISION,
            tag: fixture.tag,
            action: WalReplayActionV1::FetchDecision {
                certificate: fixture.commit_qc.clone(),
                certified_sources: vec![PeerId::new(source_key.public_key().clone())],
            },
        }),
        LifecycleStageKind::FetchBody,
        DurablePayloadReference::None,
    );
    let body_source = LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
        tag: fixture.tag,
        origin: BodyPipelineOriginV1::RecoveredDecision {
            locator,
            certificate: fixture.commit_qc.clone(),
            manifest: fixture.proposal.manifest.clone(),
        },
    });
    assert!(matches!(
        body_source.project(
            fixture.context,
            LifecycleStageKind::FetchBody,
            &ReplayPayloadBindingV1::from_payload(fixture.body_payload),
        ),
        Err(ReplayAuthorityValidationError::RecordMismatch)
    ));
    assert!(matches!(
        body_source.project(
            fixture.context,
            LifecycleStageKind::ApplyDecision,
            &ReplayPayloadBindingV1::from_payload(fixture.body_payload),
        ),
        Err(ReplayAuthorityValidationError::RecordMismatch)
    ));
    let store = replay_case(
        fixture.context,
        body_source.clone(),
        LifecycleStageKind::StoreBody,
        fixture.body_payload,
    );
    let validate = replay_case(
        fixture.context,
        body_source,
        LifecycleStageKind::ValidateBody,
        fixture.body_payload,
    );
    let apply = replay_case(
        fixture.context,
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role: ReplayWalRoleV1::DECISION,
            tag: fixture.tag,
            action: WalReplayActionV1::ApplyDecision(fixture.commit_qc.clone()),
        }),
        LifecycleStageKind::ApplyDecision,
        fixture.body_payload,
    );
    assert_eq!(
        recovered_decision_body_continuation_is_exact(
            DurableContinuationEdge::FetchToStore,
            &fetch.authority,
            fetch.payload,
            &store.authority,
            store.payload,
        ),
        Some(true)
    );
    assert_eq!(
        recovered_decision_body_continuation_is_exact(
            DurableContinuationEdge::StoreToValidate,
            &store.authority,
            store.payload,
            &validate.authority,
            validate.payload,
        ),
        Some(true)
    );
    assert_eq!(
        recovered_decision_body_continuation_is_exact(
            DurableContinuationEdge::ValidateToApply,
            &validate.authority,
            validate.payload,
            &apply.authority,
            apply.payload,
        ),
        Some(true)
    );
    assert_eq!(
        recovered_decision_body_continuation_is_exact(
            DurableContinuationEdge::FetchToStore,
            &fetch.authority,
            fetch.payload,
            &validate.authority,
            validate.payload,
        ),
        Some(false),
        "the recovered lineage cannot skip Store"
    );

    let causal_root = CausalRoot::new(digest_from_hash(&Hash::new(
        b"recovered Decision Apply test root",
    )));
    let candidate = |case: &ReplayCase| {
        CandidateAdmission::new(
            case.key,
            causal_root,
            case.work_class,
            case.stage,
            InitialLifecycleState::Ready,
            causal_root.digest(),
            case.payload,
            case.authority.clone(),
            PhysicalGeometry::new([], []),
            None,
        )
    };
    let lineage = RecoveredDecisionApplyCandidateLineageV1 {
        fetch: fetch.authority.clone(),
        store: candidate(&store),
        validate: candidate(&validate),
        apply: candidate(&apply),
    };
    let validated = ValidatedBodyReceipt::for_test(fixture.body_receipt.clone());
    assert!(lineage.exactly_matches_validated_receipt(fixture.context, &validated));
    let owner = OwnerId::new(causal_root, 1);
    let [store_record, validate_record, live_apply_record] = lineage
        .successor_records(owner, 2, 3, 4)
        .expect("exact recovered Decision lineage projects adjacent records");
    assert!(lineage.exactly_matches_successor_records(
        owner,
        &store_record,
        &validate_record,
        &live_apply_record,
    ));
    assert!(!lineage.exactly_matches_terminal_successor_records(
        owner,
        &store_record,
        &validate_record,
        &live_apply_record,
    ));
    let terminal_apply_record = super::super::ledger::LifecycleLedgerRecordV1::new(
        lineage.apply.key,
        owner,
        4,
        lineage.apply.work_class,
        lineage.apply.stage,
        Some(TerminalOutcome::Advanced),
        lineage.apply.reconstruction_source,
        lineage.apply.payload,
        lineage.apply.replay_authority.clone(),
        super::super::schema::DurableContinuation::None,
    )
    .expect("terminal recovered Decision Apply record remains canonical");
    assert!(lineage.exactly_matches_terminal_successor_records(
        owner,
        &store_record,
        &validate_record,
        &terminal_apply_record,
    ));
    assert!(!lineage.exactly_matches_successor_records(
        owner,
        &store_record,
        &validate_record,
        &terminal_apply_record,
    ));

    let live_store_record = super::super::ledger::LifecycleLedgerRecordV1::new(
        lineage.store.key,
        owner,
        2,
        lineage.store.work_class,
        lineage.store.stage,
        None,
        lineage.store.reconstruction_source,
        lineage.store.payload,
        lineage.store.replay_authority.clone(),
        super::super::schema::DurableContinuation::None,
    )
    .expect("construct recovered Decision Store crash cut");
    assert!(lineage.exactly_matches_live_store_record(owner, &live_store_record));
    let [resumed_store, resumed_validate, resumed_apply] = lineage
        .successor_records_after_live_store(owner, &live_store_record, 7, 8)
        .expect("restart appends the exact Validate/Apply tail after unrelated ordinals");
    assert_eq!(resumed_store.ordinal(), 2);
    assert_eq!(resumed_validate.ordinal(), 7);
    assert_eq!(resumed_apply.ordinal(), 8);
    assert!(lineage.exactly_matches_successor_records(
        owner,
        &resumed_store,
        &resumed_validate,
        &resumed_apply,
    ));
    assert!(
        lineage
            .successor_records_after_live_store(owner, &resumed_store, 9, 10)
            .is_none(),
        "an already-advanced Store collision cannot be rewritten as a fresh crash cut"
    );
    let foreign_owner = OwnerId::new(
        CausalRoot::new(digest_from_hash(&Hash::new(
            b"foreign recovered Decision Store owner",
        ))),
        2,
    );
    assert!(
        lineage
            .successor_records_after_live_store(foreign_owner, &live_store_record, 7, 8)
            .is_none(),
        "a same-record collision under another owner must fail exact projection"
    );
    let foreign_durable = DurableBodyReceipt::for_test(
        fixture.body_receipt.context_id(),
        fixture.body_receipt.round(),
        fixture.body_receipt.subject(),
        HashOf::from_untyped_unchecked(Hash::new(b"foreign Decision Apply manifest")),
    );
    let foreign_validated = ValidatedBodyReceipt::for_test(foreign_durable);
    assert!(
        !lineage.exactly_matches_validated_receipt(fixture.context, &foreign_validated),
        "a valid receipt for another durable body cannot enter the Apply carrier"
    );

    let mut foreign_store = store.authority.clone();
    let LifecycleReplaySourceV1::BodyPipeline(source) = &mut foreign_store.source else {
        unreachable!("recovered Store retains one body-pipeline source")
    };
    let BodyPipelineOriginV1::RecoveredDecision { locator, .. } = &mut source.origin else {
        unreachable!("recovered Store retains one Decision origin")
    };
    *locator = RecoveredWalFrameIdentity::for_test(22, 23, [0xD6; 32]).persisted_locator();
    assert_eq!(
        recovered_decision_body_continuation_is_exact(
            DurableContinuationEdge::FetchToStore,
            &fetch.authority,
            fetch.payload,
            &foreign_store,
            store.payload,
        ),
        Some(false),
        "a foreign exact locator cannot enter the body lineage"
    );
}

#[test]
fn nested_record_validation_rejects_oversized_canonical_authority() {
    let fixture = Fixture::new();
    let case = fixture.cases().remove(8);
    assert_eq!(case.stage.kind(), LifecycleStageKind::BroadcastProposal);
    let mut authority = case.authority;
    let LifecycleReplaySourceV1::ConsensusBroadcast(message) = &mut authority.source else {
        panic!("BroadcastProposal fixture retains one consensus message")
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut message.payload else {
        panic!("BroadcastProposal fixture retains one proposal")
    };
    proposal.signature = vec![0xA5; MAX_REPLAY_AUTHORITY_BYTES + 1];
    assert!(authority.encode().len() > MAX_REPLAY_AUTHORITY_BYTES);
    assert_eq!(
        authority.validate_record(
            fixture.context,
            case.key,
            case.work_class,
            case.stage,
            case.payload,
        ),
        Err(ReplayAuthorityValidationError::InvalidEncoding)
    );
    assert!(!authority.structurally_matches_record(
        fixture.context,
        case.key,
        case.work_class,
        case.stage,
        case.payload,
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn certified_serve_pending_replay_pair_binds_exact_fsync_origin_and_records() {
    let temporary = TempDir::new().expect("temporary Certified-Serve replay directory");
    let fixture = CertifiedServeReplayFixture::new();
    let (mut store, recovery) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open Certified-Serve replay payload store");
    assert!(recovery.is_empty());
    let receipt = store
        .persist_pending(&fixture.authenticated)
        .expect("persist exact Pending Certified-Serve request");
    let pair = CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
        fixture.active_context,
        &fixture.authenticated,
        receipt,
    )
    .expect("seal exact post-fsync Serve/Producer replay pair");
    assert!(pair.shares_exact_storage_origin());

    let serve_shape = pair
        .serve
        .family
        .source
        .project(
            fixture.active_context,
            LifecycleStageKind::CertifiedServe,
            &pair.serve.payload,
        )
        .expect("derive fixed Certified-Serve record");
    let producer_shape = pair
        .producer
        .family
        .source
        .project(
            fixture.active_context,
            LifecycleStageKind::ProducerTurn,
            &ReplayPayloadBindingV1::None,
        )
        .expect("derive fixed ProducerTurn record");
    let serve_stage = LifecycleStage::new(
        LifecycleStageKind::CertifiedServe,
        PredecessorScope::ReadyOrdinalPrefix,
    );
    let producer_stage = LifecycleStage::new(
        LifecycleStageKind::ProducerTurn,
        PredecessorScope::ProducerHandoffBarrier,
    );
    assert!(pair.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        serve_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));
    assert!(pair.exactly_matches_producer_record(
        fixture.active_context,
        producer_shape.key,
        producer_stage,
        DurablePayloadReference::None,
        receipt.payload_hash(),
    ));

    let shared = Arc::new(pair);
    let adjacent = Arc::clone(&shared);
    assert!(Arc::ptr_eq(&shared, &adjacent));
    assert!(shared.shares_exact_storage_origin());
    assert!(shared.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        serve_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));
    assert!(adjacent.exactly_matches_producer_record(
        fixture.active_context,
        producer_shape.key,
        producer_stage,
        DurablePayloadReference::None,
        receipt.payload_hash(),
    ));

    let foreign_request_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign Certified-Serve replay request"));
    assert!(
        CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            fixture.active_context,
            &fixture.authenticated,
            receipt.with_request_hash_for_test(foreign_request_hash),
        )
        .is_none()
    );
    let foreign_certificate_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign Certified-Serve replay certificate"));
    assert!(
        CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            fixture.active_context,
            &fixture.authenticated,
            receipt.with_certificate_hash_for_test(foreign_certificate_hash),
        )
        .is_none()
    );
    assert!(
        CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            fixture.active_context,
            &fixture.authenticated,
            receipt
                .with_payload_hash_for_test(Hash::new(b"foreign Certified-Serve replay payload",)),
        )
        .is_none()
    );
    let out_of_range = wire::ValidatorIndex::try_from(wire::MAX_VALIDATORS_PER_HEIGHT)
        .expect("validator hard bound fits its wire index");
    assert!(
        CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            fixture.active_context,
            &fixture.authenticated,
            receipt.with_local_retainer_for_test(out_of_range),
        )
        .is_none()
    );
    assert!(
        CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            fixture.active_context,
            &fixture.authenticated,
            receipt.with_local_retainer_for_test(1),
        )
        .is_none(),
        "a different QC signer cannot replace the receipt's exact local retainer"
    );
    assert!(
        CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            fixture.active_context,
            &fixture.authenticated,
            receipt.with_local_retainer_for_test(3),
        )
        .is_none(),
        "a roster member absent from the QC signer set cannot retain replay authority"
    );

    let foreign_context = LifecycleContext::new(
        LifecycleDigest::new([0xD1; 32]),
        fixture.active_context.height(),
    );
    assert!(
        CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(
            foreign_context,
            &fixture.authenticated,
            receipt,
        )
        .is_none()
    );
    assert!(!pair.exactly_matches_serve_record(
        fixture.active_context,
        producer_shape.key,
        serve_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));
    assert!(!pair.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        producer_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));
    assert!(!pair.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        serve_stage,
        DurablePayloadReference::None,
        receipt.payload_hash(),
    ));
    assert!(!pair.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        serve_stage,
        fixture.pending_payload(),
        Hash::new(b"wrong retained payload hash"),
    ));
    assert!(!pair.exactly_matches_producer_record(
        fixture.active_context,
        producer_shape.key,
        producer_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));

    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload: pair.serve.payload.clone(),
        source: LifecycleReplaySourceV1::CertifiedServeStorage(pair.serve.family.source.clone()),
    };
    let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode())
        .expect("exact Certified-Serve replay source canonical-roundtrips");
    assert!(pair.serve.exactly_matches_authority(&canonical));

    let mut wrong_payload_source = canonical.clone();
    let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut wrong_payload_source.source
    else {
        unreachable!("Serve replay authority retains its storage source")
    };
    source.payload_hash[0] ^= 1;
    assert!(!pair.serve.exactly_matches_authority(&wrong_payload_source));

    let mut wrong_qc_source = canonical.clone();
    let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut wrong_qc_source.source else {
        unreachable!("Serve replay authority retains its storage source")
    };
    source.request.certificate.aggregate_signature[0] ^= 1;
    let wrong_qc_source = LifecycleReplayAuthorityV1::decode_canonical(&wrong_qc_source.encode())
        .expect("mutated QC source remains canonical codec data");
    assert!(!pair.serve.exactly_matches_authority(&wrong_qc_source));

    let mut absent_retainer = canonical;
    let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut absent_retainer.source else {
        unreachable!("Serve replay authority retains its storage source")
    };
    source.local_retainer = 3;
    assert!(
        absent_retainer
            .validate_record(
                fixture.active_context,
                serve_shape.key,
                LifecycleWorkClass::CertifiedServe,
                serve_stage,
                fixture.pending_payload(),
            )
            .is_err()
    );
}

#[cfg(feature = "bls")]
#[test]
fn recovered_serve_states_reconstruct_one_common_source_per_replay_pair() {
    let fixture = CertifiedServeRecoveredReplayFixture::new();
    let pending = fixture.replay_pair(RecoveredServeState::Pending);
    let completed = fixture.replay_pair(RecoveredServeState::Completed);
    let negative = fixture.replay_pair(RecoveredServeState::Negative);

    for pair in [&pending, &completed, &negative] {
        assert!(pair.shares_exact_storage_origin());
        assert!(Arc::ptr_eq(&pair.serve.family, &pair.producer.family));
    }
    assert!(matches!(
        pending.serve.payload,
        ReplayPayloadBindingV1::CertifiedServePending { .. }
    ));
    assert!(matches!(
        completed.serve.payload,
        ReplayPayloadBindingV1::CertifiedServeCompleted { .. }
    ));
    assert!(matches!(
        negative.serve.payload,
        ReplayPayloadBindingV1::CertifiedServeNegative {
            outcome: DurableServeNegativeOutcome::Rejected(17),
            ..
        }
    ));
    assert_eq!(
        pending.serve.family.source.request,
        completed.serve.family.source.request
    );
    assert_eq!(
        pending.serve.family.source.request,
        negative.serve.family.source.request
    );
    assert_eq!(
        pending.serve.family.source.local_retainer,
        completed.serve.family.source.local_retainer
    );
    assert_eq!(
        pending.serve.family.source.local_retainer,
        negative.serve.family.source.local_retainer
    );
    assert_ne!(
        pending.serve.family.source.payload_hash, completed.serve.family.source.payload_hash,
        "the exact canonical frame hash binds its completed state"
    );
    assert_ne!(
        pending.serve.family.source.payload_hash, negative.serve.family.source.payload_hash,
        "the exact canonical frame hash binds its negative state"
    );
}

#[test]
fn recovered_prepare_and_commit_votes_build_canonical_attached_evidence() {
    let fixture = Fixture::new();
    let locator = RecoveredWalFrameIdentity::for_test(8, 9, [0xB1; 32]);
    let tag = fixture.recovered_tag();
    for mut vote in [fixture.prepare_vote.clone(), fixture.commit_vote.clone()] {
        vote.signature.clear();
        let evidence =
            RecoveredWalVoteReplayEvidenceV1::from_sealed_recovered_vote(locator, tag, &vote)
                .expect("production-shaped recovered vote builds canonical evidence");
        assert!(evidence.exactly_matches_recovered_vote(locator, tag, &vote));
        assert_eq!(evidence, evidence.clone());
        let encoded = evidence.authority.encode();
        assert_eq!(
            LifecycleReplayAuthorityV1::decode_canonical(&encoded)
                .expect("attached evidence remains canonical"),
            evidence.authority
        );
        let LifecycleReplaySourceV1::Wal(source) = &evidence.authority.source else {
            panic!("recovered vote evidence is WAL-backed")
        };
        let expected_role = match vote.phase {
            wire::GlobalPhase::Prepare => ReplayWalRoleV1::PREPARE_INTENT,
            wire::GlobalPhase::Commit => ReplayWalRoleV1::LOCK_AND_COMMIT,
        };
        assert!(source.role.matches(expected_role));
        assert!(source.locator.exactly_matches_runtime(locator));
    }
}

#[test]
fn recovered_vote_evidence_rejects_role_vote_and_frame_hash_substitution() {
    let fixture = Fixture::new();
    let locator = RecoveredWalFrameIdentity::for_test(8, 9, [0xB2; 32]);
    let tag = fixture.recovered_tag();
    let mut vote = fixture.prepare_vote.clone();
    vote.signature.clear();
    let evidence =
        RecoveredWalVoteReplayEvidenceV1::from_sealed_recovered_vote(locator, tag, &vote)
            .expect("Prepare replay evidence fixture");

    let mut wrong_role = evidence.clone();
    let LifecycleReplaySourceV1::Wal(source) = &mut wrong_role.authority.source else {
        panic!("recovered vote evidence is WAL-backed")
    };
    source.role = ReplayWalRoleV1::LOCK_AND_COMMIT;
    assert!(!wrong_role.exactly_matches_recovered_vote(locator, tag, &vote));

    let mut wrong_vote = vote.clone();
    wrong_vote.subject = fixture.conflicting_vote.subject;
    assert!(!evidence.exactly_matches_recovered_vote(locator, tag, &wrong_vote));

    let wrong_hash = RecoveredWalFrameIdentity::for_test(8, 9, [0xB3; 32]);
    assert!(!evidence.exactly_matches_recovered_vote(wrong_hash, tag, &vote));
}

#[test]
fn certified_fetch_store_validate_evidence_retains_one_canonical_origin_and_frame() {
    let fixture = Fixture::new();
    let tag = fixture.recovered_tag();
    let certificate = fixture.prepare_qc.clone();
    let manifest = fixture.proposal.manifest.clone();
    let fetch_effect = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(certificate),
    };
    let responder = KeyPair::random();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(&fixture.serve_request),
        manifest: manifest.clone(),
        body: vec![0xA1, 0xA2],
        responder: 0,
        signature: Vec::new(),
    };
    response.signature = Signature::new(responder.private_key(), &response.signature_preimage())
        .payload()
        .to_vec();
    let receipt = DurableBodyReceipt::for_test(
        manifest.round.context_id,
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let fetch = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
        &fetch_effect,
        &response,
        &receipt,
    )
    .expect("signed certified response builds canonical Fetch evidence");
    assert!(fetch.family.is_exact_all_stages());
    assert!(fetch.exactly_matches_signed_response_for_test(&fetch_effect, &response, &receipt,));
    let mut zero_frame = fetch.family.clone();
    zero_frame.body_frame.frame = [0; 32];
    assert!(
        zero_frame.is_exact_all_stages(),
        "body-frame digests have no reserved zero sentinel"
    );

    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store = fetch
        .project_store_for_test(&store_effect, &receipt)
        .expect("Fetch evidence projects only its exact Store stage");
    assert!(store.exactly_matches_store(&store_effect, &receipt));

    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store_pending = pending_binding(&store_effect, tag, 81);
    let validate_pending = store_pending
        .project_store_validate_successor(&store_effect, &validate_effect)
        .expect("Store pending projects one exact Validate root");
    let validate = store
        .project_validate(&store_effect, &receipt, &validate_effect, &validate_pending)
        .expect("Store evidence projects only its exact Validate stage");
    assert!(validate.exactly_matches_validate_pending(
        &validate_effect,
        &receipt,
        &validate_pending,
    ));
    let foreign_pending = pending_binding(&validate_effect, tag, 82);
    assert!(!validate.exactly_matches_validate_pending(
        &validate_effect,
        &receipt,
        &foreign_pending,
    ));
    assert!(validate.exactly_matches_durable_body(&receipt));
    assert_eq!(validate, validate.clone());
    assert_eq!(fetch.family, store.family);
    assert_eq!(store.family, validate.family);
}

#[test]
fn durable_ready_fetch_digest_ignores_transport_retransmission_but_binds_replay_identity() {
    fn projection(
        effect: &AdapterEffect,
        response: &wire::CertifiedBodyResponse,
        receipt: &DurableBodyReceipt,
        causal_root: CausalRoot,
    ) -> DurableCertifiedFetchReplayProjectionV1 {
        let evidence = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
            effect, response, receipt,
        )
        .expect("structurally signed response projects one exact durable family");
        let pending = PendingRuntimeEffectBinding::from_durable_certified_fetch(
            DurableCertifiedFetchPendingMintPermit::new(),
            Hash::prehashed(*causal_root.digest().as_bytes()),
            effect,
        )
        .expect("exact certified Fetch effect mints one frame-bound pending binding");
        evidence
            .project_durable_ready_fetch(effect, &pending, receipt)
            .expect("exact family, pending binding, and receipt project Ready Fetch")
    }

    let fixture = Fixture::new();
    let tag = fixture.recovered_tag();
    let manifest = fixture.proposal.manifest.clone();
    let effect = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(fixture.prepare_qc.clone()),
    };
    let receipt = DurableBodyReceipt::for_test(
        manifest.round.context_id,
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let first_response = wire::CertifiedBodyResponse {
        request_hash: HashOf::from_untyped_unchecked(Hash::new(b"first request occurrence")),
        manifest: manifest.clone(),
        body: vec![0xD1, 0xD2],
        responder: 0,
        signature: vec![0xD3],
    };
    let retransmitted_response = wire::CertifiedBodyResponse {
        request_hash: HashOf::from_untyped_unchecked(Hash::new(b"different request occurrence")),
        responder: 3,
        signature: vec![0xD4, 0xD5],
        ..first_response.clone()
    };
    assert_ne!(
        HashOf::new(&first_response),
        HashOf::new(&retransmitted_response)
    );
    let causal_root = CausalRoot::new(digest_from_hash(&Hash::new(b"ready Fetch causal root")));
    let first = projection(&effect, &first_response, &receipt, causal_root);
    let retransmitted = projection(&effect, &retransmitted_response, &receipt, causal_root);
    let first_queue_identity = super::super::ingress_position::PendingFairIngressIdentity::for_test(
        fixture.context,
        digest_from_hash(&Hash::new(b"first queue occurrence")),
        11,
    );
    let retransmitted_queue_identity =
        super::super::ingress_position::PendingFairIngressIdentity::for_test(
            fixture.context,
            digest_from_hash(&Hash::new(b"second queue occurrence")),
            12,
        );
    assert_ne!(first_queue_identity, retransmitted_queue_identity);
    assert_eq!(
        first.completion_digest(),
        retransmitted.completion_digest(),
        "request, response, responder, signature, and physical queue occurrence are not restart identity",
    );

    let foreign_causal = projection(
        &effect,
        &first_response,
        &receipt,
        CausalRoot::new(digest_from_hash(&Hash::new(b"foreign Fetch causal root"))),
    );
    assert_ne!(
        first.completion_digest(),
        foreign_causal.completion_digest()
    );

    let foreign_effect_identity = Hash::new(b"foreign exact Fetch effect identity");
    assert_ne!(
        first.completion_digest(),
        canonical_durable_certified_fetch_completion_digest(
            first.causal_key,
            foreign_effect_identity,
            &first.authority,
        )
    );

    let mut foreign_qc_authority = first.authority.clone();
    let LifecycleReplaySourceV1::BodyPipeline(source) = &mut foreign_qc_authority.source else {
        panic!("durable Ready Fetch authority is body-pipeline backed")
    };
    let BodyPipelineOriginV1::Certified { certificate, .. } = &mut source.origin else {
        panic!("durable Ready Fetch authority is certified")
    };
    certificate.aggregate_signature[0] ^= 1;
    assert_ne!(
        first.completion_digest(),
        canonical_durable_certified_fetch_completion_digest(
            first.causal_key,
            first.effect_identity,
            &foreign_qc_authority,
        )
    );

    let manifest_absent_effect = AdapterEffect::FetchBody {
        manifest: None,
        ..effect.clone()
    };
    let manifest_absent = projection(
        &manifest_absent_effect,
        &first_response,
        &receipt,
        causal_root,
    );
    assert_ne!(
        first.completion_digest(),
        manifest_absent.completion_digest()
    );

    let source_key = KeyPair::try_from_seed(vec![0xD6; 32], Algorithm::Ed25519)
        .expect("deterministic certified-source identity");
    let foreign_sources_effect = AdapterEffect::FetchBody {
        certified_sources: vec![PeerId::new(source_key.public_key().clone())],
        ..effect.clone()
    };
    let foreign_sources = projection(
        &foreign_sources_effect,
        &first_response,
        &receipt,
        causal_root,
    );
    assert_ne!(
        first.completion_digest(),
        foreign_sources.completion_digest()
    );

    let mut foreign_frame_authority = first.authority.clone();
    let ReplayPayloadBindingV1::BodyFrame(frame) = &mut foreign_frame_authority.payload else {
        panic!("durable Ready Fetch authority is frame-bound")
    };
    frame.frame[0] ^= 1;
    assert_ne!(
        first.completion_digest(),
        canonical_durable_certified_fetch_completion_digest(
            first.causal_key,
            first.effect_identity,
            &foreign_frame_authority,
        )
    );
}

#[test]
fn certified_pipeline_evidence_rejects_certificate_manifest_frame_and_stage_substitution() {
    let fixture = Fixture::new();
    let tag = fixture.recovered_tag();
    let manifest = fixture.proposal.manifest.clone();
    let fetch_effect = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(fixture.prepare_qc.clone()),
    };
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(&fixture.serve_request),
        manifest: manifest.clone(),
        body: vec![0xB1],
        responder: 0,
        signature: vec![0xB2],
    };
    let receipt = DurableBodyReceipt::for_test(
        manifest.round.context_id,
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let fetch = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
        &fetch_effect,
        &response,
        &receipt,
    )
    .expect("certified substitution fixture");

    let mut wrong_certificate = fetch.clone();
    let BodyPipelineOriginV1::Certified { certificate, .. } =
        &mut wrong_certificate.family.source.origin
    else {
        panic!("certified fixture retains its QC")
    };
    certificate.aggregate_signature[0] ^= 1;
    assert!(!wrong_certificate.exactly_matches_signed_response_for_test(
        &fetch_effect,
        &response,
        &receipt,
    ));

    response.manifest.chunk_root = Hash::new(b"substituted response manifest");
    assert!(!fetch.exactly_matches_signed_response_for_test(&fetch_effect, &response, &receipt,));

    let mut wrong_frame = fetch.clone();
    wrong_frame.family.body_frame.frame[0] ^= 1;
    assert!(!wrong_frame.exactly_matches_signed_response_for_test(
        &fetch_effect,
        &wire::CertifiedBodyResponse {
            manifest,
            ..response.clone()
        },
        &receipt,
    ));

    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: receipt.round(),
        subject: receipt.subject(),
    };
    let store = fetch
        .project_store_for_test(&store_effect, &receipt)
        .expect("exact Store stage fixture");
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: receipt.round(),
        subject: receipt.subject(),
    };
    assert!(!store.exactly_matches_store(&validate_effect, &receipt));
    let store_pending = pending_binding(&store_effect, tag, 83);
    let validate_pending = store_pending
        .project_store_validate_successor(&store_effect, &validate_effect)
        .expect("Store pending projects one exact Validate root");
    let validate = store
        .project_validate(&store_effect, &receipt, &validate_effect, &validate_pending)
        .expect("exact Validate stage fixture");
    assert!(
        !validate.exactly_matches_validate_pending(&store_effect, &receipt, &validate_pending,)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn local_body_pre_intent_seal_rejects_owner_manifest_frame_and_stage_substitution() {
    let fixture = Fixture::new();
    let tag = fixture.recovered_tag();
    let manifest = fixture.proposal.manifest.clone();
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 70)],
    )
    .expect("bind exact local Store owner")
    .pop()
    .expect("one local Store owner");
    let store_pending = store_ownership
        .pending_adapter_effect_binding(&store_effect)
        .expect("local Store owner projects one pending seal");
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let validate_pending = store_pending
        .project_store_validate_successor(&store_effect, &validate_effect)
        .expect("local Store owner projects one Validate successor");
    let receipt = DurableBodyReceipt::for_test(
        manifest.round.context_id,
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let seal = LocalBodyPreIntentReplaySealV1::for_test(&store_effect, store_pending, &manifest)
        .expect("mint test-only local pre-intent seal");
    assert!(seal.exactly_projects_validate(
        &store_effect,
        &manifest,
        &receipt,
        &validate_effect,
        &validate_pending,
    ));

    let foreign_pending = pending_binding(&validate_effect, tag, 71);
    assert!(!seal.exactly_projects_validate(
        &store_effect,
        &manifest,
        &receipt,
        &validate_effect,
        &foreign_pending,
    ));
    let seal = seal
        .bind_and_project_validate(
            &store_effect,
            &manifest,
            &receipt,
            &validate_effect,
            &foreign_pending,
        )
        .expect_err("foreign owner returns the original move-only seal");
    let mut foreign_manifest = manifest.clone();
    foreign_manifest.chunk_root = Hash::new(b"foreign local replay manifest");
    let foreign_receipt = DurableBodyReceipt::for_test(
        manifest.round.context_id,
        manifest.round,
        manifest.subject,
        HashOf::new(&foreign_manifest),
    );
    assert!(!seal.exactly_projects_validate(
        &store_effect,
        &manifest,
        &foreign_receipt,
        &validate_effect,
        &validate_pending,
    ));

    let mut validate = seal
        .bind_and_project_validate(
            &store_effect,
            &manifest,
            &receipt,
            &validate_effect,
            &validate_pending,
        )
        .expect("exact local durability joins Validate replay evidence");
    assert!(validate.exactly_matches_validate(&validate_effect, &receipt));
    validate.family.body_frame.frame[0] ^= 1;
    assert!(!validate.exactly_matches_validate(&validate_effect, &receipt));
    validate.family.body_frame.frame = [0; 32];
    assert!(
        validate
            .family
            .is_exact_for_stage(LifecycleStageKind::ValidateBody),
        "zero-valued digest bytes remain structurally valid rather than sentinel values"
    );
    assert!(!validate.exactly_matches_validate(&store_effect, &receipt));

    let validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("local Store root rebinds to its exact Validate effect");
    let second_store_pending = store_ownership
        .pending_adapter_effect_binding(&store_effect)
        .expect("local Store root retains its exact pending projection");
    let second_validate_pending = validate_ownership
        .pending_adapter_effect_binding(&validate_effect)
        .expect("local Validate root retains its exact pending projection");
    let exact_validate =
        LocalBodyPreIntentReplaySealV1::for_test(&store_effect, second_store_pending, &manifest)
            .expect("remint an independent test-only local seal")
            .bind_and_project_validate(
                &store_effect,
                &manifest,
                &receipt,
                &validate_effect,
                &second_validate_pending,
            )
            .expect("exact local Store evidence advances to Validate");
    let validated_receipt = ValidatedBodyReceipt::for_test(receipt.clone());
    let command_identity = LocalProposalReadyCommandIdentity::from_exact_handoff(
        tag,
        &manifest,
        &receipt,
        &validated_receipt,
        &validate_ownership,
    )
    .expect("exact Validate completion has one inert command identity");
    let ready = exact_validate
        .complete_local_proposal(
            &validate_effect,
            &manifest,
            validated_receipt,
            command_identity,
        )
        .expect("exact Validate completion retains local replay evidence");
    let mut unsigned_proposal = fixture.proposal.clone();
    unsigned_proposal.signature.clear();
    let proposal_intent = AdapterEffect::Sign {
        tag,
        request: SignRequest::Proposal(unsigned_proposal),
    };
    let proposal_ownership = validate_ownership
        .rebind_as_inherited_adapter_effect(&proposal_intent)
        .expect("local Validate root rebinds to exact ProposalIntent");
    let foreign_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&proposal_intent),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 72)],
    )
    .expect("bind foreign ProposalIntent owner")
    .pop()
    .expect("one foreign ProposalIntent owner");
    assert!(!ready.exactly_matches_proposal_intent(
        command_identity,
        &proposal_intent,
        &foreign_ownership,
    ));
    let intent = ready
        .bind_proposal_intent(command_identity, &proposal_intent, &proposal_ownership)
        .expect("exact command consumes into one inseparable ProposalIntent composite");
    assert!(intent.exactly_matches_proposal_intent(
        command_identity,
        &proposal_intent,
        &proposal_ownership,
    ));
    drop(intent);
}

#[test]
fn local_body_replay_authority_is_linear_nondecode_and_closed_to_fixed_joins() {
    let source = include_str!("../v2_lifecycle_replay_authority.rs");
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("replay authority has one production prefix");
    let local = production
        .split("pub(in crate::sumeragi) struct LocalBodyPreIntentReplaySealV1")
        .nth(1)
        .expect("local replay seal has one declaration")
        .split("/// Selector-authenticated origin awaiting one exact durable body-frame binding.")
        .next()
        .expect("certified replay evidence follows local replay authority");
    for required in [
        "store_pending: PendingRuntimeEffectBinding",
        "pub(in crate::sumeragi) struct LocalValidateReplayEvidenceV1",
        "pub(in crate::sumeragi) struct LocalProposalReadyReplayEvidenceV1",
        "pub(in crate::sumeragi) struct LocalProposalIntentReplayEvidenceV1",
        "fn bind_and_project_validate(",
        "project_store_validate_successor(store_effect, validate_effect)",
        "fn complete_local_proposal(",
        "command_identity: LocalProposalReadyCommandIdentity",
        "exactly_matches_proposal_intent(",
        "exactly_matches_proposal_intent_effect(",
        "fn bind_proposal_intent(",
        "BodyPipelineOriginV1::LocalBody(manifest.clone())",
    ] {
        assert!(
            local.contains(required),
            "local replay authority omitted {required}"
        );
    }
    for forbidden in [
        "#[derive(Clone",
        "#[derive(Copy",
        "Decode",
        "pub(in crate::sumeragi) fn source(",
        "pub(in crate::sumeragi) fn receipt(",
        "pub(in crate::sumeragi) fn pending(",
        "pub(in crate::sumeragi) fn manifest(",
        "pub(in crate::sumeragi) fn into_parts(",
        "Arc<LocalBodyPreIntentReplaySealV1>",
        "Arc<LocalValidateReplayEvidenceV1>",
        "Arc<LocalProposalIntentReplayEvidenceV1>",
        "!= [0; 32]",
        "== [0; 32]",
        "is_zero()",
    ] {
        assert!(
            !local.contains(forbidden),
            "local replay authority exposed or reserved {forbidden}"
        );
    }

    let runtime = include_str!("../v2_runtime.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("runtime has one production prefix");
    let executor = include_str!("../v2_effects.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("effect executor has one production prefix");
    assert_eq!(
        runtime.matches("LocalBodyReplayMintPermit::new()").count(),
        1
    );
    assert_eq!(
        runtime
            .matches("LocalProposalEffectOwnership::from_exact_assemble_body(")
            .count(),
        2,
        "only the active-view and fresh local producer branches mint the composite"
    );
    for required in [
        "local_store_replay: BTreeMap<EffectWorkId, LocalProposalEffectOwnership>",
        "local_validate_replay: BTreeMap<EffectWorkId, LocalValidateReplayEvidenceV1>",
        "BTreeMap<LocalProposalReadyCommandIdentity, LocalProposalReadyReplayEvidenceV1>",
        "BTreeMap<LocalProposalReadyCommandIdentity, LocalProposalIntentReplayEvidenceV1>",
        ".project_exact_validate(",
        ".complete_local_proposal(",
        ".bind_proposal_intent(",
        "plan_local_proposal_replay_consumptions(",
        "retire_local_proposal_ready_replay(",
    ] {
        assert!(
            executor.contains(required),
            "executor omitted local replay cut {required}"
        );
    }
    assert!(!production.contains("BodyPipelineOriginV1::ProposalIntent"));
}

#[test]
#[allow(clippy::too_many_lines)]
fn certified_serve_replay_pair_is_opaque_exact_and_fixed_admission_only() {
    let source = include_str!("../v2_lifecycle_replay_authority.rs");
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("replay authority has one production prefix");
    let evidence = production
        .split("struct CertifiedServeStorageReplayFamilyV1 {")
        .nth(1)
        .expect("Certified-Serve replay family has one declaration")
        .split("struct CertifiedBodyPipelineReplayFamilyV1 {")
        .next()
        .expect("certified body replay family follows Serve evidence");
    for required in [
        "source: CertifiedServeStorageSourceV1",
        "struct CertifiedServeReplayEvidenceV1",
        "struct CertifiedServeProducerTurnReplayEvidenceV1",
        "pub(super) struct CertifiedServeReplayEvidencePairV1",
        "family: Arc<CertifiedServeStorageReplayFamilyV1>",
        "pub(super) fn from_post_fsync_pending(",
        "receipt.exactly_matches_pending(authenticated)",
        "pub(super) fn from_authenticated_recovery(",
        "recovered.exactly_matches_persisted_payload()",
        "recovered.local_retainer()",
        "binary_search(&local_retainer)",
        "pub(super) fn exactly_matches_serve_record(",
        "pub(super) fn exactly_matches_terminal_serve_record(",
        "pub(super) fn exactly_matches_producer_record(",
        "pub(super) fn admission_candidate(",
        "pub(super) fn exactly_matches_serve_carrier(",
        "pub(super) fn exactly_matches_producer_carrier(",
        "Some(CandidateAdmission::new(",
        "Arc::ptr_eq(&self.serve.family, &self.producer.family)",
        "LifecycleStageKind::CertifiedServe",
        "LifecycleStageKind::ProducerTurn",
        "producer_turn_key_for_serve(serve.key)",
    ] {
        assert!(
            evidence.contains(required),
            "Certified-Serve replay evidence omitted {required}"
        );
    }
    for runtime_seal in [
        "CertifiedServeStorageReplayFamilyV1",
        "CertifiedServeReplayEvidenceV1",
        "CertifiedServeProducerTurnReplayEvidenceV1",
        "CertifiedServeReplayEvidencePairV1",
    ] {
        let derive = production
            .split(runtime_seal)
            .next()
            .expect("Serve runtime seal has a declaration prefix")
            .rsplit("#[derive(")
            .next()
            .expect("Serve runtime seal derive is inspectable")
            .split(")]")
            .next()
            .expect("Serve runtime seal derive is bounded");
        assert!(
            !derive.contains("Decode") && !derive.contains("Encode"),
            "Serve runtime seal {runtime_seal} became codec-constructible"
        );
    }
    for forbidden in [
        "pub(super) fn from_parts(",
        "pub(super) fn into_parts(",
        "pub(super) fn source(",
        "pub(super) fn request(",
        "pub(super) fn certificate(",
        "pub(super) fn payload_hash(",
        "pub(super) fn local_retainer(",
        "pub(super) fn encoded(",
        "pub(super) fn authority(",
        "pub(super) fn serve(",
        "pub(super) fn producer(",
        "into_concrete_evidence",
        "pub(super) fn into_admission(",
        "impl Drop for CertifiedServe",
    ] {
        assert!(
            !evidence.contains(forbidden),
            "Certified-Serve replay evidence exposed {forbidden}"
        );
    }

    let storage = production
        .split("struct CertifiedServeStorageSourceV1 {")
        .nth(1)
        .expect("Certified-Serve storage source has one declaration")
        .split("enum ReplayPayloadBindingV1 {")
        .next()
        .expect("replay payload binding follows Serve storage source");
    for required in [
        "local_retainer >= wire::MAX_VALIDATORS_PER_HEIGHT",
        ".binary_search(&self.local_retainer)",
        "LifecycleStageKind::CertifiedServe => LifecyclePhase::Serve",
        "LifecycleStageKind::ProducerTurn => LifecyclePhase::ProducerTurn",
    ] {
        assert!(
            storage.contains(required),
            "canonical Certified-Serve source omitted {required}"
        );
    }

    let payload_store = include_str!("../v2_certified_serve_payload_store.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("payload store has one production prefix");
    for required in [
        "local_retainer: wire::ValidatorIndex",
        "pub(crate) const fn local_retainer(&self)",
        "pub(crate) fn exactly_matches_persisted_payload(&self)",
        "local_retainer,\n                state",
    ] {
        assert!(
            payload_store.contains(required),
            "authenticated payload recovery omitted {required}"
        );
    }

    for outside in [
        include_str!("../v2_lifecycle_coordinator.rs"),
        include_str!("../v2_lifecycle_ledger.rs"),
        include_str!("../v2.rs"),
        include_str!("../v2_runtime.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        let outside = outside
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("outside production prefix is bounded");
        assert!(!outside.contains("CertifiedServeReplayEvidencePairV1"));
        assert!(!outside.contains("CertifiedServeReplayEvidenceV1"));
        assert!(!outside.contains("CertifiedServeProducerTurnReplayEvidenceV1"));
    }
    let registry = include_str!("../v2_lifecycle_work_registry.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("registry production prefix is bounded");
    for required in [
        "CertifiedServeReplayEvidencePairV1",
        "Arc<CertifiedServeReplayEvidencePairV1>",
        "Arc::clone(&replay_evidence)",
        "exactly_matches_serve_carrier(",
        "exactly_matches_terminal_serve_record(",
        "exactly_matches_producer_carrier(",
    ] {
        assert!(
            registry.contains(required),
            "whole-pair registry ownership omitted {required}"
        );
    }
    for forbidden in [
        "CertifiedServeReplayEvidenceV1,",
        "CertifiedServeProducerTurnReplayEvidenceV1,",
        "into_concrete_evidence",
    ] {
        assert!(
            !registry.contains(forbidden),
            "registry decomposed Certified-Serve replay authority through {forbidden}"
        );
    }
    let projection = include_str!("../v2_lifecycle_projection.rs")
        .split("\n#[cfg(test)]\nmod wait_source_tests {")
        .next()
        .expect("projection production prefix is bounded");
    for required in [
        "CertifiedServeReplayEvidencePairV1::from_post_fsync_pending(",
        "CertifiedServeReplayEvidencePairV1::from_authenticated_recovery(",
        "replay\n        .admission_candidate(active_context)",
        "impl super::ProductionLifecycleOwnerV1",
        "fn admit_selected_certified_serve(",
        "target: super::LifecycleIngressIoTargetSeal",
        "target.matches_certified_serve_request(authenticated.request_hash())",
        "prepare_certified_serve_admission(",
        ".install_certified_serve_fresh_batch_before_publication(",
        "|| self.coordinator.persist_exact_staged_successor(&staged)",
        "fn certified_serve_terminal_replay_decision(",
        ".exactly_matches_certified_serve_publication(authenticated, publication.receipt())",
        "CertifiedServeConcreteAdmissionV1::restart_required(",
        "fn into_safe_continuation(",
        "fn settle_certified_serve_completed(",
        "fn settle_certified_serve_negative(",
        ".persist_completed_with_exact_body(",
        ".persist_negative_for_authenticated_request(authenticated, outcome)",
        "fn preflight_certified_serve_terminal(",
        "fn publish_certified_serve_terminal(",
        "CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(",
        "CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(",
        ".prepare_certified_serve_terminal_transition(",
        ".publish_certified_serve_terminal_transition(",
    ] {
        assert!(
            projection.contains(required),
            "fixed Certified-Serve admission omitted {required}"
        );
    }
    for cfg_test_only in [
        "#[cfg(test)]\n    pub(crate) fn persist_and_admit_certified_serve(",
        "#[cfg(test)]\npub(super) fn certified_serve_admission_request(",
    ] {
        assert!(
            projection.contains(cfg_test_only),
            "raw Certified-Serve surface escaped its fixture gate: {cfg_test_only}"
        );
    }
    let settlement = include_str!("../v2_lifecycle_settlement.rs");
    assert!(
        settlement
            .contains("#[cfg(test)]\n    pub(super) fn settle_turn_with_durable_serve_terminal(")
    );
    assert_eq!(
        settlement
            .matches("fn settle_turn_with_durable_serve_terminal(")
            .count(),
        1,
        "the raw terminal reducer wrapper must remain a single test fixture"
    );
    for outside in [
        include_str!("../v2.rs"),
        include_str!("../v2_lifecycle_coordinator.rs"),
        include_str!("../v2_lifecycle_ledger.rs"),
        include_str!("../v2_lifecycle_open.rs"),
        include_str!("../v2_lifecycle_settlement.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        let outside = outside
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("outside terminal production prefix is bounded");
        assert!(!outside.contains(".persist_completed_with_exact_body("));
        assert!(!outside.contains(".publish_certified_serve_terminal_transition("));
    }
    let coordinator = include_str!("../v2_lifecycle_coordinator.rs");
    assert!(coordinator.contains("#[cfg(test)]\n    pub(super) fn admit_certified_serve("));
    assert!(coordinator.contains(
        "CertifiedServeTerminalSettlementErrorV1, CertifiedServeTerminalSettlementFailureV1,"
    ));
    assert!(!projection.contains("CertifiedServeSettlementError"));
    assert_eq!(
        projection
            .matches("fn settle_certified_serve_completed(")
            .count(),
        1,
        "only the production owner may expose completed Serve settlement"
    );
    assert_eq!(
        projection
            .matches("fn settle_certified_serve_negative(")
            .count(),
        1,
        "only the production owner may expose negative Serve settlement"
    );
    for terminal_method in [
        "settle_certified_serve_completed",
        "settle_certified_serve_negative",
    ] {
        let signature_marker = format!("fn {terminal_method}(");
        let signature = projection
            .split(signature_marker.as_str())
            .nth(1)
            .expect("terminal owner transaction has one signature")
            .split(") -> Result<(), CertifiedServeTerminalSettlementErrorV1>")
            .next()
            .expect("terminal owner signature is bounded");
        for forbidden in [
            "receipt:",
            "payload_id",
            "CandidateAdmission",
            "ordinal:",
            "digest:",
            "ReplayEvidence",
            "parts",
            "route",
            "effect",
            "pending",
        ] {
            assert!(
                !signature.contains(forbidden),
                "terminal owner transaction accepted raw {forbidden}"
            );
        }
    }
    let fresh_signature = projection
        .split("fn admit_selected_certified_serve(")
        .nth(1)
        .expect("fresh Serve owner transaction has one signature")
        .split(") -> CertifiedServeConcreteAdmissionV1")
        .next()
        .expect("fresh Serve owner signature is bounded");
    for forbidden in [
        "CandidateAdmission",
        "AdapterEffect",
        "PendingRuntimeEffectBinding",
        "ReplayEvidence",
        "ordinal:",
        "digest:",
        "route",
        "queue",
    ] {
        assert!(
            !fresh_signature.contains(forbidden),
            "fresh Serve owner transaction accepted raw {forbidden}"
        );
    }
    assert!(projection.contains("struct CertifiedServeConcreteAdmissionV1 {"));
    assert!(!projection.contains("pub(crate) enum CertifiedServeConcreteAdmissionV1"));
    let opaque_result = projection
        .split("impl CertifiedServeConcreteAdmissionV1 {")
        .nth(1)
        .expect("opaque Serve outcome has one implementation")
        .split("impl CertifiedServeConcreteAdmissionContinuationV1 {")
        .next()
        .expect("safe continuation follows opaque Serve outcome");
    assert!(!opaque_result.contains("fn into_target("));

    for required in [
        "impl Drop for DurableCertifiedServeAdmissionPublication",
        "fresh_pending: bool",
        "fn can_abort_fresh_pending(&self)",
        "fn exactly_matches_authenticated_request(",
        "enum CertifiedServePayloadRetentionError",
        "PublicationAmbiguous(CertifiedServePayloadStoreError)",
        "PublishedButUnsynchronized(CertifiedServePayloadStoreError)",
        "fn persist_completed_with_exact_body(",
        "body_store.owns_receipt(durable_body)",
        "fn persist_negative_for_authenticated_request(",
        "#[cfg(test)]\n    pub(crate) fn persist_negative(",
    ] {
        assert!(
            payload_store.contains(required),
            "payload-first Serve ownership omitted {required}"
        );
    }
    for required in [
        "fn exactly_matches_fresh_staged_append(",
        "exactly_covers_recovered_ready_work(current)",
        "exactly_covers_recovered_ready_work_and_wal_authority(current)",
        "carrier.matches_record(record, metadata, work.digest)",
        "fn matches_current_ready_record(",
        "fn exact_optional_recovered_wal_authority(",
        "fn preflight_certified_serve_terminal_owner_state(",
        "fn preflights_exact_staged_successor(",
        "staged.high_water == current.high_water",
        "staged.producer_debts == expected_debts",
        "staged.capacity_used == expected_capacity_used",
        "fn publish_certified_serve_terminal_transition",
    ] {
        assert!(
            registry.contains(required),
            "fresh Serve whole-census preflight omitted {required}"
        );
    }

    let authority = include_str!("../v2_lifecycle_authority.rs");
    assert!(authority.contains(
        "#[cfg(test)]\n#[derive(Clone, Debug, PartialEq, Eq)]\npub(crate) struct RolloverSnapshot"
    ));
    assert!(!authority.contains("fn verified_successor("));
    assert!(coordinator.contains("#[cfg(test)]\npub(crate) use authority::RolloverSnapshot;"));
    let lifecycle_open = include_str!("../v2_lifecycle_open.rs");
    for test_only_rollover in [
        "#[cfg(test)]\n    pub(crate) fn rollover(",
        "#[cfg(test)]\n    pub(crate) fn rollover_with_payload_store(",
        "#[cfg(test)]\n    fn rollover_inner(",
        "#[cfg(test)]\n    fn serve_cancellation_receipts_are_exact(",
        "#[cfg(test)]\n    fn retire_for_rollover(",
    ] {
        assert!(
            lifecycle_open.contains(test_only_rollover),
            "raw terminal-receipt rollover escaped its test gate: {test_only_rollover}"
        );
    }
}

#[test]
fn certified_pipeline_replay_evidence_is_normalized_inert_and_stage_fixed() {
    let source = include_str!("../v2_lifecycle_replay_authority.rs");
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("replay authority has one production prefix");
    let evidence = production
        .split("pub(super) struct AuthenticatedCertifiedFetchReplayOriginV1")
        .nth(1)
        .expect("certified Fetch replay origin has one declaration")
        .split("fn exact_recovered_wal_vote_authority(")
        .next()
        .expect("recovered WAL authority follows certified body evidence");
    for required in [
        "from_completion_authority(\n        authority: &CertifiedFetchCompletionAuthority<'_>",
        "candidate_pending()\n                .exactly_binds_adapter_effect(effect)",
        "pub(super) fn bind_durable_body(",
        "pub(super) struct CertifiedFetchReplayEvidenceV1",
        "pub(super) struct CertifiedStoreReplayEvidenceV1",
        "pub(in crate::sumeragi) struct CertifiedValidateReplayEvidenceV1",
        "validate_pending: DirectSignedPendingBindingV1",
        "pub(super) fn project_store(",
        "pub(super) fn project_validate(",
        "validate_pending: &PendingRuntimeEffectBinding",
        "fn exactly_matches_validate_pending(",
        "fn is_exact_for_stage(&self, stage: LifecycleStageKind)",
        "LifecycleStageKind::FetchBody\n            | LifecycleStageKind::StoreBody",
        "ReplayPayloadBindingV1::BodyFrame(self.body_frame)",
        "family.is_exact_for_stage(stage)",
    ] {
        assert!(
            evidence.contains(required),
            "certified body replay evidence omitted {required}"
        );
    }

    let family = evidence
        .split("struct CertifiedBodyPipelineReplayFamilyV1 {")
        .nth(1)
        .expect("certified body replay family has one declaration")
        .split('}')
        .next()
        .expect("certified body replay family declaration is bounded");
    assert!(family.contains("source: BodyPipelineReplaySourceV1"));
    assert!(family.contains("body_frame: BodyFrameBindingV1"));
    assert_eq!(family.lines().filter(|line| line.contains(':')).count(), 2);
    assert!(!family.contains("LifecycleReplayAuthorityV1"));

    for runtime_seal in [
        "AuthenticatedCertifiedFetchReplayOriginV1",
        "CertifiedFetchReplayEvidenceV1",
        "CertifiedStoreReplayEvidenceV1",
        "CertifiedValidateReplayEvidenceV1",
        "CertifiedBodyPipelineReplayFamilyV1",
    ] {
        let derive = production
            .split(runtime_seal)
            .next()
            .expect("runtime seal has a declaration prefix")
            .rsplit("#[derive(")
            .next()
            .expect("runtime seal derive is inspectable")
            .split(")]")
            .next()
            .expect("runtime seal derive is bounded");
        assert!(
            !derive.contains("Decode") && !derive.contains("Encode"),
            "runtime seal {runtime_seal} became codec-constructible"
        );
    }
    for forbidden in [
        "pub(crate) struct CertifiedFetchReplayEvidenceV1",
        "pub(crate) struct CertifiedStoreReplayEvidenceV1",
        "pub(crate) struct CertifiedValidateReplayEvidenceV1",
        "pub(super) fn encoded(",
        "pub(super) fn into_parts(",
        "pub(super) fn from_parts(",
        "pub(super) fn certificate(",
        "pub(super) fn manifest(",
        "pub(super) fn receipt(",
        "pub(super) fn body_frame(",
        "[0_u8; 32]",
        "!= [0; 32]",
        "== [0; 32]",
    ] {
        assert!(
            !evidence.contains(forbidden),
            "certified body replay evidence exposed or reserved {forbidden}"
        );
    }
    assert!(evidence.contains("#[cfg(test)]\n    pub(super) fn from_signed_response_for_test("));
    assert_eq!(
        production
            .matches("#[cfg(test)]\n    pub(super) fn project_candidate_for_test(")
            .count(),
        3,
        "body-transition candidate helpers must remain test-only"
    );
    for helper in [
        "exact_live_wal_body_successor_candidate_for_test",
        "exact_invalid_body_report_candidate_for_test",
    ] {
        assert!(
            production.contains(&format!("#[cfg(test)]\npub(super) fn {helper}(")),
            "transition fixture helper {helper} lost its test-only gate"
        );
    }

    for caller in [
        include_str!("../v2_lifecycle_coordinator.rs"),
        include_str!("../v2_lifecycle_ledger.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        assert!(!caller.contains("CertifiedFetchReplayEvidenceV1"));
        assert!(!caller.contains("CertifiedStoreReplayEvidenceV1"));
        assert!(!caller.contains("CertifiedValidateReplayEvidenceV1"));
    }
}

#[test]
fn direct_signed_broadcast_evidence_covers_all_seven_fixed_stages() {
    let fixture = Fixture::new();
    let effects = signed_broadcast_effects(&fixture);
    assert_eq!(effects.len(), 7);
    for (ordinal, effect) in (1_u128..).zip(effects) {
        let pending = pending_binding(&effect, fixture.recovered_tag(), ordinal);
        let evidence = SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)
            .expect("signed broadcast has one canonical replay envelope");
        assert!(evidence.exactly_matches_effect(&effect, &pending));
    }

    let zero_digest_binding = DirectSignedPendingBindingV1 {
        causal_lifecycle_key: [0; 32],
        effect_identity: [0; 32],
    };
    assert_eq!(zero_digest_binding.causal_lifecycle_key, [0; 32]);
    assert_eq!(zero_digest_binding.effect_identity, [0; 32]);
}

#[test]
fn direct_signed_broadcast_evidence_rejects_signature_message_and_pending_substitution() {
    let fixture = Fixture::new();
    let mut effects = signed_broadcast_effects(&fixture);
    let effect = effects.remove(0);
    let pending = pending_binding(&effect, fixture.recovered_tag(), 11);
    let evidence = SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)
        .expect("signed proposal broadcast replay evidence");

    let AdapterEffect::Broadcast(message) = &effect else {
        unreachable!("first signed broadcast fixture is a Proposal")
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
        unreachable!("first signed broadcast fixture is a Proposal")
    };
    let mut re_signed = proposal.clone();
    re_signed.signature = vec![0xD1];
    let re_signed = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(re_signed),
    ));
    let re_signed_pending = pending_binding(&re_signed, fixture.recovered_tag(), 12);
    assert!(!evidence.exactly_matches_effect(&re_signed, &re_signed_pending));

    let substituted = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(fixture.conflicting_proposal.clone()),
    ));
    let substituted_pending = pending_binding(&substituted, fixture.recovered_tag(), 13);
    assert!(!evidence.exactly_matches_effect(&substituted, &substituted_pending));
    assert!(
        SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &substituted_pending).is_none()
    );

    let foreign_tag = EventTag::new(
        fixture.recovered_tag().height(),
        fixture.recovered_tag().view() + 1,
        Generation::new(9),
    );
    let foreign_pending = pending_binding(&effect, foreign_tag, 14);
    assert!(!evidence.exactly_matches_effect(&effect, &foreign_pending));
}

#[cfg(feature = "bls")]
#[test]
fn direct_signed_equivocation_evidence_covers_all_three_fixed_pairs() {
    let fixture = Fixture::new();
    let effects = vec![
        AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::proposal_for_test(
                fixture.proposal.clone(),
                fixture.conflicting_proposal.clone(),
            ),
        },
        AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::vote_for_test(
                fixture.prepare_vote.clone(),
                fixture.conflicting_vote.clone(),
            ),
        },
        AdapterEffect::ReportEquivocation {
            evidence: AdapterEquivocationEvidence::timeout_vote_for_test(
                fixture.timeout_vote.clone(),
                fixture.conflicting_timeout_vote.clone(),
            ),
        },
    ];
    assert_eq!(effects.len(), 3);
    for (ordinal, effect) in (21_u128..).zip(effects) {
        let pending = pending_binding(&effect, fixture.recovered_tag(), ordinal);
        let evidence = SignedEquivocationReplayEvidenceV1::from_exact_effect(&effect, &pending)
            .expect("authenticated conflict has one canonical replay envelope");
        assert!(evidence.exactly_matches_effect(&effect, &pending));
    }
}

#[test]
fn direct_signed_equivocation_evidence_rejects_pair_order_signature_and_pending_drift() {
    let fixture = Fixture::new();
    let forward = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(
            fixture.prepare_vote.clone(),
            fixture.conflicting_vote.clone(),
        ),
    };
    let pending = pending_binding(&forward, fixture.recovered_tag(), 31);
    let evidence = SignedEquivocationReplayEvidenceV1::from_exact_effect(&forward, &pending)
        .expect("authenticated vote conflict replay evidence");

    let reversed = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(
            fixture.conflicting_vote.clone(),
            fixture.prepare_vote.clone(),
        ),
    };
    let reversed_pending = pending_binding(&reversed, fixture.recovered_tag(), 32);
    assert!(!evidence.exactly_matches_effect(&reversed, &reversed_pending));

    let mut re_signed = fixture.prepare_vote.clone();
    re_signed.signature = vec![0xD2];
    let re_signed = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(
            re_signed,
            fixture.conflicting_vote.clone(),
        ),
    };
    let re_signed_pending = pending_binding(&re_signed, fixture.recovered_tag(), 33);
    assert!(!evidence.exactly_matches_effect(&re_signed, &re_signed_pending));
    assert!(
        SignedEquivocationReplayEvidenceV1::from_exact_effect(&forward, &re_signed_pending)
            .is_none()
    );

    let foreign_tag = EventTag::new(
        fixture.recovered_tag().height(),
        fixture.recovered_tag().view() + 1,
        Generation::new(10),
    );
    let foreign_pending = pending_binding(&forward, foreign_tag, 34);
    assert!(!evidence.exactly_matches_effect(&forward, &foreign_pending));
}

#[test]
fn direct_signed_replay_wrappers_are_opaque_nondecodable_and_fixed_class() {
    let source = include_str!("../v2_lifecycle_replay_authority.rs");
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("replay authority has one production prefix");
    let direct = production
        .split("pub(super) struct SignedBroadcastReplayEvidenceV1")
        .nth(1)
        .expect("signed Broadcast wrapper has one declaration")
        .split("/// Selector-authenticated origin awaiting one exact durable body-frame binding.")
        .next()
        .expect("certified body replay follows direct signed evidence");
    for required in [
        "pub(super) struct SignedEquivocationReplayEvidenceV1",
        "pending: DirectSignedPendingBindingV1",
        "causal_lifecycle_key: [u8; 32]",
        "effect_identity: [u8; 32]",
        "pub(super) fn from_exact_effect(\n        effect: &AdapterEffect,\n        pending: &PendingRuntimeEffectBinding",
        "pub(super) fn exactly_matches_effect(",
        "pending.exactly_binds_adapter_effect(effect)",
        "exact_signed_broadcast_authority(effect)",
        "exact_signed_equivocation_authority(effect)",
        "LifecycleReplaySourceV1::ConsensusBroadcast(message.clone())",
        "LifecycleReplaySourceV1::Equivocation(evidence)",
        "canonical_replay_authority(",
    ] {
        assert!(
            direct.contains(required),
            "direct signed replay wrapper omitted {required}"
        );
    }

    for runtime_seal in [
        "SignedBroadcastReplayEvidenceV1",
        "SignedEquivocationReplayEvidenceV1",
        "DirectSignedPendingBindingV1",
    ] {
        let derive = production
            .split(runtime_seal)
            .next()
            .expect("direct signed seal has a declaration prefix")
            .rsplit("#[derive(")
            .next()
            .expect("direct signed seal derive is inspectable")
            .split(")]")
            .next()
            .expect("direct signed seal derive is bounded");
        assert!(
            !derive.contains("Decode") && !derive.contains("Encode"),
            "runtime seal {runtime_seal} became codec-constructible"
        );
    }
    for forbidden in [
        "pub(crate) struct SignedBroadcastReplayEvidenceV1",
        "pub(crate) struct SignedEquivocationReplayEvidenceV1",
        "pub(super) fn source(",
        "pub(super) fn message(",
        "pub(super) fn evidence(",
        "pub(super) fn encoded(",
        "pub(super) fn into_parts(",
        "pub(super) fn pending(",
        "pub(super) fn effect_identity(",
        "!= [0; 32]",
        "== [0; 32]",
        "is_zero()",
    ] {
        assert!(
            !direct.contains(forbidden),
            "direct signed replay wrapper exposed or reserved {forbidden}"
        );
    }

    for caller in [
        include_str!("../v2_lifecycle_coordinator.rs"),
        include_str!("../v2_lifecycle_ledger.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        assert!(!caller.contains("SignedBroadcastReplayEvidenceV1"));
        assert!(!caller.contains("SignedEquivocationReplayEvidenceV1"));
    }
}

#[test]
fn remote_proposal_replay_wrappers_are_opaque_exact_and_have_one_runtime_mint() {
    let source = include_str!("../v2_lifecycle_replay_authority.rs");
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("replay authority has one production prefix");
    let remote = production
        .split("pub(in crate::sumeragi) struct RemoteProposalFetchReplayEvidenceV1")
        .nth(1)
        .expect("remote Proposal Fetch wrapper has one declaration")
        .split("/// Move-only pre-intent replay seal for one exact local")
        .next()
        .expect("local body replay follows remote Proposal replay");
    for required in [
        "RemoteProposalStoreReplayEvidenceV1",
        "RemoteProposalStoredReplayEvidenceV1",
        "RemoteProposalValidateReplayEvidenceV1",
        "from_exact_authenticated_proposal(",
        "RemoteProposalReplayMintPermit",
        "ingress.exactly_matches_authenticated(authenticated)",
        "certificate: None",
        "certified_sources.is_empty()",
        "pending.exactly_binds_adapter_effect(effect)",
        "project_proposal_fetch_store_successor",
        "project_store_validate_successor",
        "bind_durable_body(",
        "durable_body_frame_reference",
        "ReplayPayloadBindingV1::BodyFrame",
        "LifecycleStageKind::FetchBody",
        "LifecycleStageKind::StoreBody",
        "LifecycleStageKind::ValidateBody",
        "canonical_replay_authority(",
    ] {
        assert!(
            remote.contains(required),
            "remote Proposal replay wrapper omitted {required}"
        );
    }
    for wrapper in [
        "RemoteProposalFetchReplayEvidenceV1",
        "RemoteProposalStoreReplayEvidenceV1",
        "RemoteProposalStoredReplayEvidenceV1",
        "RemoteProposalValidateReplayEvidenceV1",
    ] {
        let derive = production
            .split(wrapper)
            .next()
            .expect("remote Proposal wrapper has a declaration prefix")
            .rsplit("#[derive(")
            .next()
            .expect("remote Proposal wrapper derive is inspectable")
            .split(")]")
            .next()
            .expect("remote Proposal wrapper derive is bounded");
        assert!(
            !derive.contains("Decode") && !derive.contains("Encode"),
            "runtime replay wrapper {wrapper} became codec-constructible"
        );
    }
    for forbidden in [
        "pub(crate) struct RemoteProposal",
        "pub(in crate::sumeragi) fn authenticated(",
        "pub(in crate::sumeragi) fn ingress(",
        "pub(in crate::sumeragi) fn source(",
        "pub(in crate::sumeragi) fn proposal(",
        "pub(in crate::sumeragi) fn pending(",
        "pub(in crate::sumeragi) fn receipt(",
        "pub(in crate::sumeragi) fn into_parts(",
        "!= [0; 32]",
        "== [0; 32]",
        "is_zero()",
    ] {
        assert!(
            !remote.contains(forbidden),
            "remote Proposal replay wrapper exposed or reserved {forbidden}"
        );
    }

    let runtime = include_str!("../v2_runtime.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("runtime has one production prefix");
    assert_eq!(
        runtime
            .matches("RemoteProposalFetchReplayEvidenceV1::from_exact_authenticated_proposal(")
            .count(),
        1,
        "only authenticated runtime dispatch mints remote Proposal evidence"
    );
    for required in [
        "remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>",
        "deferred_remote_proposal_replay",
        "DeferredEventKind::ProposalReceived",
        "bind_remote_proposal_fetch_replay(",
        "certificate: None",
        "exact_remote_proposal_fetch_replay(",
    ] {
        assert!(
            runtime.contains(required),
            "runtime remote Proposal transport omitted {required}"
        );
    }
    for outside in [
        include_str!("../v2_lifecycle_ledger.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        let outside = outside
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("outside production prefix is bounded");
        assert!(!outside.contains("RemoteProposalFetchReplayEvidenceV1"));
        assert!(!outside.contains("PreparedRemoteProposalFetchReplayPreAdmission"));
    }
}

#[test]
fn invalid_body_runtime_evidence_is_nondecodable_exact_and_fixed_join_only() {
    let source = include_str!("../v2_lifecycle_replay_authority.rs");
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("replay authority has one production prefix");
    let invalid = production
        .split("pub(in crate::sumeragi) enum DurableValidateReplayEvidenceV1")
        .nth(1)
        .expect("durable Validate replay enum has one declaration")
        .split("fn exact_certified_fetch_coordinates(")
        .next()
        .expect("certified Fetch projection follows invalid-body evidence");
    for required in [
        "Certified(CertifiedValidateReplayEvidenceV1)",
        "RemoteProposal(RemoteProposalValidateReplayEvidenceV1)",
        "pub(in crate::sumeragi) struct InvalidBodyReportReplayEvidenceV1",
        "authority: LifecycleReplayAuthorityV1",
        "validate_origin: DurableValidateReplayEvidenceV1",
        "report_pending: DirectSignedPendingBindingV1",
        "pub(in crate::sumeragi) fn seal_invalid_body_report(",
        "capability: RegisteredPrepareInvalidBodyReportCapability",
        "capability.exactly_matches_report(report_effect)",
        "validate_origin.exactly_matches_validate_pending(",
        "validate_pending: &PendingRuntimeEffectBinding",
        ".project_validate_report_invalid_certified_body_successor(",
        ".project_validate_report_invalid_certified_body_with_registered_prepare(",
        "DirectSignedPendingBindingV1::from_exact_effect(report_effect, report_pending)",
        "const CANONICAL_REJECTION_CODE: u8 = 0",
        "LifecycleReplaySourceV1::InvalidCertifiedBody",
        "body_frame_hash: *receipt.frame_hash().as_ref()",
        "LifecycleStageKind::ReportInvalidBody",
        "ReplayPayloadBindingV1::None",
        "project_sealed_invalid_body_report_candidate(",
        "_permit: &SealedInvalidBodyReportProjectionPermit",
        "authority_free_admission_projection(",
        "self.authority.clone()",
    ] {
        assert!(
            invalid.contains(required),
            "invalid-body runtime evidence omitted {required}"
        );
    }
    let persisted_invalid = production
        .split("struct InvalidBodyReplaySourceV1 {")
        .nth(1)
        .expect("persisted invalid-body source has one declaration")
        .split("struct CertifiedServeStorageSourceV1 {")
        .next()
        .expect("Certified Serve source follows invalid-body source");
    for required in [
        "validation_origin: BodyPipelineReplaySourceV1",
        "self.validation_origin.project(",
        "LifecycleStageKind::ValidateBody",
        "self.certificate.round != self.certificate.proposal_round",
        "BodyPipelineOriginV1::Proposal(proposal)",
        "certificate == &self.certificate && manifest == &self.outcome.manifest",
        "BodyPipelineOriginV1::LocalBody(_)",
        "origin_shape.key.context() != context.id()",
    ] {
        assert!(
            persisted_invalid.contains(required),
            "persisted invalid-body source omitted {required}"
        );
    }
    for runtime_seal in [
        "DurableValidateReplayEvidenceV1",
        "InvalidBodyReportReplayEvidenceV1",
    ] {
        let derive = production
            .split(runtime_seal)
            .next()
            .expect("runtime seal has a declaration prefix")
            .rsplit("#[derive(")
            .next()
            .expect("runtime seal derive is inspectable")
            .split(")]")
            .next()
            .expect("runtime seal derive is bounded");
        assert!(
            !derive.contains("Decode") && !derive.contains("Encode"),
            "runtime seal {runtime_seal} became codec-constructible"
        );
    }
    for forbidden in [
        "fn from_parts(",
        "fn into_parts(",
        "fn certificate(",
        "fn manifest(",
        "fn receipt(",
        "fn pending(",
        "fn source(",
        "fn encoded(",
        "fn candidate(",
        "!= [0; 32]",
        "== [0; 32]",
        "is_zero()",
    ] {
        assert!(
            !invalid.contains(forbidden),
            "invalid-body evidence exposed or reserved {forbidden}"
        );
    }

    let adapter = include_str!("../v2.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("adapter production prefix is bounded");
    assert_eq!(
        adapter
            .matches("DurableValidateReplayEvidenceV1::seal_invalid_body_report(")
            .count(),
        1,
        "only the fixed adapter preview mints invalid-body evidence"
    );
    for required in [
        "struct RegisteredPrepareInvalidBodyReportCapability",
        "report_effect: AdapterEffect",
        "fn registered_prepare_report_capability(",
        ".project_validate_report_invalid_certified_body_with_registered_prepare(",
        "PreparedInvalidBodyReportAdapterReplay",
        "projected.as_ref() == Some(&self.child_pending)",
        "project_invalid_body_report_candidate(",
        "permit: &SealedInvalidBodyReportProjectionPermit",
        ".project_sealed_invalid_body_report_candidate(",
    ] {
        assert!(
            adapter.contains(required),
            "adapter invalid-body seal omitted {required}"
        );
    }
    let capability = adapter
        .split("pub(in crate::sumeragi) struct RegisteredPrepareInvalidBodyReportCapability")
        .nth(1)
        .expect("registered Prepare capability has one declaration")
        .split("/// Closed classification of one direct deterministic validation rejection.")
        .next()
        .expect("direct rejection classification follows its capability");
    for forbidden in [
        "derive(Clone",
        "fn into_parts(",
        "fn certificate(",
        "fn statement(",
        "RegisteredPrepareInvalidBodyReportLinearity",
        "impl Drop for RegisteredPrepareInvalidBodyReportCapability",
    ] {
        assert!(
            !capability.contains(forbidden),
            "registered Prepare capability exposed {forbidden}"
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn live_wal_replay_seal_is_linear_nondecodable_and_has_two_closed_production_mints() {
    let source = include_str!("../v2_lifecycle_replay_authority.rs");
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("replay authority has one production prefix");
    let live = production
        .split("struct LiveWalPersistedReplaySealV1")
        .nth(1)
        .expect("live WAL replay seal has one declaration")
        .split("/// Canonical inert replay evidence for one exact signed broadcast effect.")
        .next()
        .expect("direct signed evidence follows live WAL seal");
    for required in [
        "LiveWalPersistedReplayStateV1::ApplyPending",
        "LiveWalPersistedPendingV1::PayloadFree",
        "LiveWalPersistedPendingV1::ValidateSignBound",
        "LiveWalPersistedPendingV1::ApplyPending",
        "LiveWalPersistedPendingV1::ApplyBound",
        "from_exact_live_append(\n        cause: ExactLiveWalPersistedContinuationCause",
        "exactly_binds_payload_free_pending(&self)",
        "bind_exact_validate_sign_pending(",
        "exactly_binds_validate_sign_pending(&self)",
        "project_validate_apply_successor(predecessor_effect, &self.effect)",
        "exactly_matches_apply_effect(&self.effect, receipt)",
        "ReplayWalRoleV1::PROPOSAL_INTENT",
        "ReplayWalRoleV1::PREPARE_INTENT",
        "ReplayWalRoleV1::LOCK_AND_COMMIT",
        "ReplayWalRoleV1::TIMEOUT_INTENT",
        "ReplayWalRoleV1::DECISION",
        "ReplayWalRoleV1::INSTALL_TIMEOUT",
    ] {
        assert!(live.contains(required), "live WAL seal omitted {required}");
    }
    for forbidden in [
        "#[derive(Clone",
        "#[derive(Copy",
        "Decode",
        "pub(super) fn locator(",
        "pub(super) fn action(",
        "pub(super) fn source(",
        "pub(super) fn effect(",
        "pub(super) fn pending(",
        "into_parts",
        "RecoveredWalFrameIdentity",
        "!= [0; 32]",
        "== [0; 32]",
        "is_zero()",
    ] {
        assert!(
            !live.contains(forbidden),
            "live WAL seal exposed or reserved forbidden surface {forbidden}"
        );
    }

    let adapter = include_str!("../v2.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("adapter has one production prefix");
    let runtime = include_str!("../v2_runtime.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("runtime has one production prefix");
    let work_registry = include_str!("../v2_lifecycle_work_registry.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("work registry has one production prefix");
    assert_eq!(
        adapter
            .matches("SealedLiveWalPersistedEffectV1::from_exact_live_append(")
            .count(),
        2,
        "only the generic persisted cut and sealed Ready-Sign cut mint live replay authority"
    );
    assert_eq!(
        adapter
            .matches("PendingRuntimeEffectBinding::from_exact_live_wal_append(")
            .count(),
        2,
        "the same two closed post-fsync cuts derive frame-bound placeholder owners"
    );
    let ready_sign = adapter
        .split("// READY_DURABLE_VALIDATE_LIVE_SIGN_BEGIN")
        .nth(1)
        .expect("sealed Ready-Sign segment exists")
        .split("// READY_DURABLE_VALIDATE_LIVE_SIGN_END")
        .next()
        .expect("sealed Ready-Sign segment is bounded");
    assert_eq!(
        ready_sign
            .matches("SealedLiveWalPersistedEffectV1::from_exact_live_append(")
            .count(),
        1
    );
    assert_eq!(
        ready_sign
            .matches("PendingRuntimeEffectBinding::from_exact_live_wal_append(")
            .count(),
        1
    );
    assert!(ready_sign.contains("LiveWalFrameIdentity::from_append_receipt("));
    assert!(ready_sign.contains("bind_exact_validate_sign_pending(child_pending)"));
    let generic = adapter
        .split("fn drive_exact_persisted_continuation(")
        .nth(1)
        .expect("generic exact persisted cut exists")
        .split("fn live_wal_record_exactly_owns_effect(")
        .next()
        .expect("generic exact persisted cut is bounded");
    assert_eq!(
        generic
            .matches("SealedLiveWalPersistedEffectV1::from_exact_live_append(")
            .count(),
        1
    );
    assert_eq!(
        generic
            .matches("PendingRuntimeEffectBinding::from_exact_live_wal_append(")
            .count(),
        1
    );
    assert_eq!(
        adapter
            .matches("drive_exact_persisted_continuation(")
            .count(),
        1,
        "the inert live cut has no production caller yet"
    );
    assert_eq!(runtime.matches("fn from_exact_live_wal_append(").count(), 1);
    assert_eq!(
        work_registry.matches(".complete_exact_apply(").count(),
        1,
        "only the retained Validate completion supplies an Apply receipt"
    );
    assert!(!adapter.contains("RecoveredWalFrameIdentity::for_test"));
    for outside in [
        include_str!("../v2_lifecycle_ledger.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        assert!(!outside.contains("SealedLiveWalPersistedEffectV1"));
        assert!(!outside.contains("drive_exact_persisted_continuation"));
    }
}

#[test]
fn record_matching_rejects_substitution_of_every_external_coordinate() {
    let fixture = Fixture::new();
    let case = fixture
        .cases()
        .into_iter()
        .next()
        .expect("fixture has cases");
    let foreign_context =
        LifecycleContext::new(LifecycleDigest::new([0xFF; 32]), fixture.context.height());
    assert!(
        case.authority
            .validate_record(
                foreign_context,
                case.key,
                case.work_class,
                case.stage,
                case.payload,
            )
            .is_err()
    );
    let wrong_key = LifecycleKey::new(
        case.key.context(),
        case.key.round(),
        case.key.proposal_round(),
        case.key.subject(),
        LifecyclePhase::BroadcastProposal,
        case.key.execution_commitment(),
    );
    assert_eq!(
        case.authority.validate_record(
            fixture.context,
            wrong_key,
            case.work_class,
            case.stage,
            case.payload,
        ),
        Err(ReplayAuthorityValidationError::RecordMismatch)
    );
    assert!(
        case.authority
            .validate_record(
                fixture.context,
                case.key,
                LifecycleWorkClass::Broadcast,
                case.stage,
                case.payload,
            )
            .is_err()
    );
    assert!(
        case.authority
            .validate_record(
                fixture.context,
                case.key,
                case.work_class,
                LifecycleStage::new(
                    LifecycleStageKind::SignPrepareVote,
                    PredecessorScope::Independent,
                ),
                case.payload,
            )
            .is_err()
    );
    assert_eq!(
        case.authority.validate_record(
            fixture.context,
            case.key,
            case.work_class,
            case.stage,
            fixture.body_payload,
        ),
        Err(ReplayAuthorityValidationError::PayloadMismatch)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn typed_sources_reject_locator_role_signature_and_outcome_drift() {
    let fixture = Fixture::new();
    let wal_case = fixture.cases().remove(0);
    let mut wrong_locator = wal_case.authority.clone();
    let LifecycleReplaySourceV1::Wal(source) = &mut wrong_locator.source else {
        panic!("first fixture authority is WAL-backed")
    };
    source.locator = RecoveredWalFrameIdentity::for_test(8, 10, [0x21; 32]).persisted_locator();
    assert!(
        wrong_locator
            .validate_record(
                fixture.context,
                wal_case.key,
                wal_case.work_class,
                wal_case.stage,
                wal_case.payload,
            )
            .is_err()
    );

    let mut wrong_role = wal_case.authority;
    let LifecycleReplaySourceV1::Wal(source) = &mut wrong_role.source else {
        panic!("first fixture authority is WAL-backed")
    };
    source.role = ReplayWalRoleV1::DECISION;
    assert!(
        wrong_role
            .validate_record(
                fixture.context,
                wal_case.key,
                wal_case.work_class,
                wal_case.stage,
                wal_case.payload,
            )
            .is_err()
    );

    let mut broadcast = fixture.cases().remove(8).authority;
    let LifecycleReplaySourceV1::ConsensusBroadcast(message) = &mut broadcast.source else {
        panic!("ninth fixture authority is a broadcast")
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut message.payload else {
        panic!("ninth fixture authority broadcasts a proposal")
    };
    proposal.signature.clear();
    let broadcast_case = fixture.cases().remove(8);
    assert!(
        broadcast
            .validate_record(
                fixture.context,
                broadcast_case.key,
                broadcast_case.work_class,
                broadcast_case.stage,
                broadcast_case.payload,
            )
            .is_err()
    );

    let invalid_case = fixture.cases().remove(19);
    let mut invalid = invalid_case.authority.clone();
    let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut invalid.source else {
        panic!("twentieth fixture authority is an invalid-body report")
    };
    source.outcome.rejection_code = 1;
    assert!(
        invalid
            .validate_record(
                fixture.context,
                invalid_case.key,
                invalid_case.work_class,
                invalid_case.stage,
                invalid_case.payload,
            )
            .is_err()
    );

    let mut wrong_report_round = invalid_case.authority.clone();
    let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut wrong_report_round.source
    else {
        panic!("invalid-body fixture retains one report certificate")
    };
    source.certificate.round.view = source.certificate.round.view.saturating_add(1);
    assert!(
        wrong_report_round
            .validate_record(
                fixture.context,
                invalid_case.key,
                invalid_case.work_class,
                invalid_case.stage,
                invalid_case.payload,
            )
            .is_err(),
        "the report QC round cannot diverge from its validation origin"
    );

    let mut wrong_remote_origin = invalid_case.authority.clone();
    let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut wrong_remote_origin.source
    else {
        panic!("invalid-body fixture retains one validation origin")
    };
    source.validation_origin.origin =
        BodyPipelineOriginV1::Proposal(fixture.conflicting_proposal.clone());
    assert!(
        wrong_remote_origin
            .validate_record(
                fixture.context,
                invalid_case.key,
                invalid_case.work_class,
                invalid_case.stage,
                invalid_case.payload,
            )
            .is_err(),
        "a report cannot splice a different signed Proposal origin"
    );

    let mut local_origin = invalid_case.authority.clone();
    let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut local_origin.source else {
        panic!("invalid-body fixture retains one validation origin")
    };
    source.validation_origin.origin =
        BodyPipelineOriginV1::LocalBody(source.outcome.manifest.clone());
    assert!(
        local_origin
            .validate_record(
                fixture.context,
                invalid_case.key,
                invalid_case.work_class,
                invalid_case.stage,
                invalid_case.payload,
            )
            .is_err(),
        "local body authority cannot stand in for a reported remote/certified origin"
    );

    let mut certified_origin = invalid_case.authority.clone();
    let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut certified_origin.source else {
        panic!("invalid-body fixture retains one validation origin")
    };
    source.validation_origin.origin = BodyPipelineOriginV1::Certified {
        certificate: fixture.prepare_qc.clone(),
        manifest: source.outcome.manifest.clone(),
        fetch_manifest_present: true,
        certified_sources: Vec::new(),
    };
    assert!(
        certified_origin
            .validate_record(
                fixture.context,
                invalid_case.key,
                invalid_case.work_class,
                invalid_case.stage,
                invalid_case.payload,
            )
            .is_ok(),
        "the exact certified Validate origin remains canonical"
    );
    let LifecycleReplaySourceV1::InvalidCertifiedBody(source) = &mut certified_origin.source else {
        unreachable!("certified invalid-body fixture retains its source")
    };
    let BodyPipelineOriginV1::Certified { certificate, .. } = &mut source.validation_origin.origin
    else {
        unreachable!("certified invalid-body fixture retains its QC")
    };
    *certificate = fixture.commit_qc.clone();
    assert!(
        certified_origin
            .validate_record(
                fixture.context,
                invalid_case.key,
                invalid_case.work_class,
                invalid_case.stage,
                invalid_case.payload,
            )
            .is_err(),
        "a certified origin must retain the report's exact PrepareQC"
    );

    let serve_case = fixture.cases().remove(20);
    let mut invalid_retainer = serve_case.authority.clone();
    let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut invalid_retainer.source
    else {
        panic!("twenty-first fixture authority is Certified-Serve storage")
    };
    source.local_retainer =
        u32::try_from(wire::MAX_VALIDATORS_PER_HEIGHT).expect("validator bound fits u32");
    assert!(
        invalid_retainer
            .validate_record(
                fixture.context,
                serve_case.key,
                serve_case.work_class,
                serve_case.stage,
                serve_case.payload,
            )
            .is_err()
    );

    let local_store = fixture.cases().remove(5);
    let LifecycleReplaySourceV1::BodyPipeline(local_source) = local_store.authority.source else {
        panic!("sixth fixture authority is a local body source")
    };
    assert!(matches!(
        local_source.project(
            fixture.context,
            LifecycleStageKind::FetchBody,
            &ReplayPayloadBindingV1::None,
        ),
        Err(ReplayAuthorityValidationError::RecordMismatch)
    ));
}
