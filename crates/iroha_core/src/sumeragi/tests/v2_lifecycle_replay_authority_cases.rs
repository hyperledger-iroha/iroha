fn pending_binding_with_distinct_root(
    effect: &AdapterEffect,
    tag: EventTag,
    ordinal: u128,
    semantic_identity: &[u8],
) -> PendingRuntimeEffectBinding {
    bind_adapter_effect_batch_ownership(
        core::slice::from_ref(effect),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                ordinal,
                semantic_identity,
            ),
        ],
    )
    .expect("bind replay fixture with a distinct semantic root")
    .pop()
    .expect("one distinct-root replay fixture owner")
    .exact_pending_adapter_effect_binding(effect)
    .expect("mint exact distinct-root pending binding")
}
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
fn timeout_certificate_retransmit_replay_rejects_pre_envelope_subject_key() {
    let fixture = Fixture::new();
    let case = fixture
        .cases()
        .into_iter()
        .find(|case| case.stage.kind() == LifecycleStageKind::BroadcastTc)
        .expect("fixture retains one timeout-certificate Broadcast");
    let LifecycleReplaySourceV1::ConsensusBroadcast(message) = &case.authority.source else {
        panic!("timeout-certificate row retains a direct Broadcast authority")
    };
    let wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) = &message.payload else {
        panic!("BroadcastTc authority retains a timeout certificate")
    };
    let highest = certificate.highest_prepare_qc();
    let pre_envelope_key = lifecycle_key(
        fixture.context,
        certificate.round,
        highest.map(|qc| qc.proposal_round),
        highest.map(|qc| block_subject(qc.subject)),
        LifecyclePhase::BroadcastTc,
        highest.map(|qc| execution_commitment(qc.execution_commitment)),
    );

    assert_ne!(case.key, pre_envelope_key);
    assert_eq!(
        case.key.subject(),
        Some(timeout_certificate_envelope_subject(certificate))
    );
    case.authority
        .validate_record(
            fixture.context,
            case.key,
            case.work_class,
            case.stage,
            case.payload,
        )
        .expect("envelope-subject BroadcastTc key is canonical");
    assert_eq!(
        case.authority.validate_record(
            fixture.context,
            pre_envelope_key,
            case.work_class,
            case.stage,
            case.payload,
        ),
        Err(ReplayAuthorityValidationError::RecordMismatch)
    );
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
fn decision_replay_accepts_future_view_commit_qc_without_relaxing_other_sources() {
    let base = Fixture::new();
    let future = Fixture::for_record(base.context, 1);
    let lagging_tag = ReplayEventTagV1::new(base.context.height(), 0, 9);
    let locator = RecoveredWalFrameIdentity::for_test(31, 32, [0xD6; 32]);
    let payload = ReplayPayloadBindingV1::from_payload(future.body_payload);
    let source_key = KeyPair::try_from_seed(vec![0xD7; 32], Algorithm::Ed25519)
        .expect("deterministic future-view Decision source");
    let certified_sources = vec![PeerId::new(source_key.public_key().clone())];

    assert!(!lagging_tag.matches_round(base.context, future.commit_qc.round));
    assert!(lagging_tag.matches_decision_round(base.context, future.commit_qc.round));

    let apply = WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role: ReplayWalRoleV1::DECISION,
        tag: lagging_tag,
        action: WalReplayActionV1::ApplyDecision(future.commit_qc.clone()),
    };
    assert!(
        apply
            .project(base.context, LifecycleStageKind::ApplyDecision, &payload,)
            .is_ok()
    );
    let fetch = WalReplaySourceV1 {
        locator: locator.persisted_locator(),
        role: ReplayWalRoleV1::DECISION,
        tag: lagging_tag,
        action: WalReplayActionV1::FetchDecision {
            certificate: future.commit_qc.clone(),
            certified_sources,
        },
    };
    assert!(
        fetch
            .project(
                base.context,
                LifecycleStageKind::FetchBody,
                &ReplayPayloadBindingV1::None,
            )
            .is_ok()
    );

    for origin in [
        BodyPipelineOriginV1::Certified {
            certificate: future.commit_qc.clone(),
            manifest: future.proposal.manifest.clone(),
            fetch_manifest_present: true,
            certified_sources: Vec::new(),
        },
        BodyPipelineOriginV1::RecoveredDecision {
            locator: locator.persisted_locator(),
            certificate: future.commit_qc.clone(),
            manifest: future.proposal.manifest.clone(),
        },
    ] {
        let source = BodyPipelineReplaySourceV1 {
            tag: lagging_tag,
            origin,
        };
        for stage in [
            LifecycleStageKind::StoreBody,
            LifecycleStageKind::ValidateBody,
        ] {
            assert!(source.project(base.context, stage, &payload).is_ok());
        }
    }

    let event_tag = EventTag::new(base.context.height(), 0, Generation::new(9));
    let fetch_effect = AdapterEffect::FetchBody {
        tag: event_tag,
        round: future.commit_qc.proposal_round,
        subject: future.commit_qc.subject,
        manifest: Some(future.proposal.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(future.commit_qc.clone()),
    };
    let coordinates =
        exact_certified_fetch_coordinates_from_manifest(&fetch_effect, &future.proposal.manifest)
            .expect("future-view CommitQC Fetch retains the current reducer owner");
    assert_eq!(coordinates.tag, lagging_tag);

    let apply_effect = AdapterEffect::Apply {
        tag: event_tag,
        subject: future.commit_qc.subject,
        certificate: future.commit_qc.clone(),
    };
    let live = exact_live_wal_replay_projection(
        &LiveWalFrameIdentity::for_test(41, 42, [0xD8; 32]),
        &apply_effect,
    )
    .expect("future-view CommitQC seals its exact live Decision continuation");
    assert_eq!(live.stage, LifecycleStageKind::ApplyDecision);

    let prepare_fetch = AdapterEffect::FetchBody {
        tag: event_tag,
        round: future.prepare_qc.proposal_round,
        subject: future.prepare_qc.subject,
        manifest: Some(future.proposal.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(future.prepare_qc.clone()),
    };
    assert!(
        exact_certified_fetch_coordinates_from_manifest(&prepare_fetch, &future.proposal.manifest,)
            .is_none()
    );
    let prepare_body = BodyPipelineReplaySourceV1 {
        tag: lagging_tag,
        origin: BodyPipelineOriginV1::Certified {
            certificate: future.prepare_qc,
            manifest: future.proposal.manifest,
            fetch_manifest_present: true,
            certified_sources: Vec::new(),
        },
    };
    assert!(matches!(
        prepare_body.project(
            base.context,
            LifecycleStageKind::FetchBody,
            &ReplayPayloadBindingV1::None,
        ),
        Err(ReplayAuthorityValidationError::InvalidSource)
    ));
}
#[cfg(feature = "bls")]
#[test]
fn recovered_decision_fetch_accepts_authenticated_future_view_commit_qc() {
    let fixture = CertifiedServeRecoveredReplayFixture::new();
    let context = fixture.verified.context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 1,
    };
    let subject = fixture.authenticated.request().subject;
    let execution_commitment = fixture
        .authenticated
        .request()
        .certificate
        .execution_commitment;
    let signers = vec![0, 1, 2];
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                fixture.keys[usize::try_from(*signer).expect("small fixture signer")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate future-view CommitQC");
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers,
        aggregate_signature,
    };
    let effect = AdapterEffect::FetchBody {
        tag: EventTag::new(round.height, 0, Generation::new(7)),
        round,
        subject,
        manifest: None,
        certified_sources: context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    assert!(
        exact_recovered_wal_decision_fetch_authority(
            &fixture.verified,
            RecoveredWalFrameIdentity::for_test(51, 52, [0xD9; 32]),
            &effect,
        )
        .is_some()
    );
}
#[cfg(feature = "bls")]
#[test]
fn refined_proposal_validate_joins_complete_qc_replay_authority() {
    let fixture = CertifiedServeRecoveredReplayFixture::new();
    let context = fixture.verified.context();
    let certificate = &fixture.authenticated.request().certificate;
    let round = certificate.proposal_round;
    let subject = certificate.subject;
    let tag =
        crate::sumeragi::v2_core::EventTag::new(round.height, round.view, Generation::new(11));
    let proposal = wire::Proposal {
        round,
        proposer: context.leader(round.view),
        subject,
        manifest: fixture.manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![0xDA],
    };
    let proposal_source = BodyPipelineReplaySourceV1 {
        tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
        origin: BodyPipelineOriginV1::Proposal(proposal),
    };
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate.clone()),
    };
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 0xDA)],
    )
    .expect("bind certified Fetch authority")
    .pop()
    .expect("one certified Fetch owner");
    let validate_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("inherit Prepare authority at Validate");
    let validate_pending = validate_ownership
        .exact_pending_adapter_effect_binding(&validate_effect)
        .expect("seal refined Validate pending binding");
    let validate_source = exact_remote_proposal_validate_source(
        &proposal_source,
        &validate_pending,
        Some(certificate),
    )
    .expect("complete PrepareQC refines the Proposal replay source");
    assert!(matches!(
        validate_source.origin,
        BodyPipelineOriginV1::Certified { .. }
    ));

    let active_context = replay_context(round);
    let receipt =
        DurableBodyReceipt::for_test(context.id(), round, subject, HashOf::new(&fixture.manifest));
    let payload = DurablePayloadReference::BodyFrame(
        durable_body_frame_reference(active_context, &receipt)
            .expect("fixture receipt has one durable body frame"),
    );
    let payload_binding = ReplayPayloadBindingV1::from_payload(payload);
    let projected = super::super::projection::authority_free_admission_projection(
        active_context,
        &fixture.verified,
        &validate_effect,
        &validate_pending,
    )
    .expect("project refined Validate coordinates");
    let stale_proposal_authority = canonical_replay_authority(
        active_context,
        LifecycleReplaySourceV1::BodyPipeline(proposal_source),
        LifecycleStageKind::ValidateBody,
        payload_binding.clone(),
    )
    .expect("ordinary Proposal remains canonical for an ordinary Validate");
    assert!(
        candidate_from_authorized_projection(
            active_context,
            projected,
            payload,
            stale_proposal_authority,
        )
        .is_none(),
        "the ordinary Proposal cannot authorize a Prepare-refined Validate key",
    );

    let projected = super::super::projection::authority_free_admission_projection(
        active_context,
        &fixture.verified,
        &validate_effect,
        &validate_pending,
    )
    .expect("reproject refined Validate coordinates");
    let refined_authority = canonical_replay_authority(
        active_context,
        LifecycleReplaySourceV1::BodyPipeline(validate_source),
        LifecycleStageKind::ValidateBody,
        payload_binding,
    )
    .expect("the complete QC is canonical Validate replay authority");
    let candidate =
        candidate_from_authorized_projection(active_context, projected, payload, refined_authority)
            .expect("the QC replay source joins the refined Validate candidate");
    assert_eq!(
        candidate.key.execution_commitment(),
        Some(execution_commitment(certificate.execution_commitment))
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
        store.authority, validate.authority,
        "the fixed Store/Validate pair intentionally shares one body replay envelope"
    );
    assert!(
        !super::super::body_pipeline_transition::durable_continuation_successor_is_exact(
            DurableContinuationEdge::FetchToStore,
            fetch.work_class,
            fetch.key,
            fetch.stage,
            validate.work_class,
            validate.key,
            validate.stage,
        ),
        "the typed recovered lineage cannot skip Store"
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
    let gapped_apply_ordinal = 10;
    let gapped_validate = super::super::ledger::LifecycleLedgerRecordV1::new(
        lineage.validate.key,
        owner,
        resumed_validate.ordinal(),
        lineage.validate.work_class,
        lineage.validate.stage,
        Some(TerminalOutcome::Advanced),
        lineage.validate.reconstruction_source,
        lineage.validate.payload,
        lineage.validate.replay_authority.clone(),
        super::super::schema::DurableContinuation::successor(
            DurableContinuationEdge::ValidateToApply,
            gapped_apply_ordinal,
        ),
    )
    .expect("recovered Validate can point across unrelated shared ordinals");
    let gapped_live_apply = super::super::ledger::LifecycleLedgerRecordV1::new(
        lineage.apply.key,
        owner,
        gapped_apply_ordinal,
        lineage.apply.work_class,
        lineage.apply.stage,
        None,
        lineage.apply.reconstruction_source,
        lineage.apply.payload,
        lineage.apply.replay_authority.clone(),
        super::super::schema::DurableContinuation::None,
    )
    .expect("live recovered Apply can follow an unrelated shared ordinal");
    assert!(lineage.exactly_matches_successor_records(
        owner,
        &resumed_store,
        &gapped_validate,
        &gapped_live_apply,
    ));
    let gapped_terminal_apply = super::super::ledger::LifecycleLedgerRecordV1::new(
        lineage.apply.key,
        owner,
        gapped_apply_ordinal,
        lineage.apply.work_class,
        lineage.apply.stage,
        Some(TerminalOutcome::Advanced),
        lineage.apply.reconstruction_source,
        lineage.apply.payload,
        lineage.apply.replay_authority.clone(),
        super::super::schema::DurableContinuation::None,
    )
    .expect("terminal recovered Apply can follow an unrelated shared ordinal");
    assert!(lineage.exactly_matches_terminal_successor_records(
        owner,
        &resumed_store,
        &gapped_validate,
        &gapped_terminal_apply,
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
    assert!(!shared.exactly_matches_serve_record(
        fixture.active_context,
        producer_shape.key,
        serve_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));
    assert!(!shared.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        producer_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));
    assert!(!shared.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        serve_stage,
        DurablePayloadReference::None,
        receipt.payload_hash(),
    ));
    assert!(!shared.exactly_matches_serve_record(
        fixture.active_context,
        serve_shape.key,
        serve_stage,
        fixture.pending_payload(),
        Hash::new(b"wrong retained payload hash"),
    ));
    assert!(!shared.exactly_matches_producer_record(
        fixture.active_context,
        producer_shape.key,
        producer_stage,
        fixture.pending_payload(),
        receipt.payload_hash(),
    ));
    let authority = LifecycleReplayAuthorityV1 {
        format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
        payload: shared.serve.payload.clone(),
        source: LifecycleReplaySourceV1::CertifiedServeStorage(shared.serve.family.source.clone()),
    };
    let canonical = LifecycleReplayAuthorityV1::decode_canonical(&authority.encode())
        .expect("exact Certified-Serve replay source canonical-roundtrips");
    assert!(shared.serve.exactly_matches_authority(&canonical));
    let mut wrong_payload_source = canonical.clone();
    let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut wrong_payload_source.source
    else {
        unreachable!("Serve replay authority retains its storage source")
    };
    source.payload_hash[0] ^= 1;
    assert!(
        !shared
            .serve
            .exactly_matches_authority(&wrong_payload_source)
    );
    let mut wrong_qc_source = canonical.clone();
    let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut wrong_qc_source.source else {
        unreachable!("Serve replay authority retains its storage source")
    };
    source.request.certificate.aggregate_signature[0] ^= 1;
    let wrong_qc_source = LifecycleReplayAuthorityV1::decode_canonical(&wrong_qc_source.encode())
        .expect("mutated QC source remains canonical codec data");
    assert!(!shared.serve.exactly_matches_authority(&wrong_qc_source));
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
        negative.serve.payload.durable_payload(),
        Some(DurablePayloadReference::CertifiedServeNegative {
            outcome: DurableServeNegativeOutcome::Rejected(17),
            ..
        })
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
    let foreign_pending = pending_binding_with_distinct_root(
        &validate_effect,
        tag,
        82,
        b"foreign certified Validate root",
    );
    assert!(foreign_pending.exactly_binds_adapter_effect(&validate_effect));
    assert_ne!(
        foreign_pending.causal_lifecycle_key(),
        validate_pending.causal_lifecycle_key()
    );
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
    let mut manifest_absent_effect = effect.clone();
    let AdapterEffect::FetchBody {
        manifest: candidate_manifest,
        ..
    } = &mut manifest_absent_effect
    else {
        unreachable!("fixture effect is one certified Fetch")
    };
    *candidate_manifest = None;
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
    let mut foreign_sources_effect = effect.clone();
    let AdapterEffect::FetchBody {
        certified_sources: candidate_sources,
        ..
    } = &mut foreign_sources_effect
    else {
        unreachable!("fixture effect is one certified Fetch")
    };
    *candidate_sources = vec![PeerId::new(source_key.public_key().clone())];
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
        .exact_pending_adapter_effect_binding(&store_effect)
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
    let foreign_pending = pending_binding_with_distinct_root(
        &validate_effect,
        tag,
        71,
        b"foreign local Validate root",
    );
    assert!(foreign_pending.exactly_binds_adapter_effect(&validate_effect));
    assert_ne!(
        foreign_pending.causal_lifecycle_key(),
        validate_pending.causal_lifecycle_key()
    );
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
    validate.family.assembled_body_frame_mut_for_test().frame[0] ^= 1;
    assert!(!validate.exactly_matches_validate(&validate_effect, &receipt));
    validate.family.assembled_body_frame_mut_for_test().frame = [0; 32];
    assert!(
        validate
            .family
            .is_exact_for_stage_for_test(LifecycleStageKind::ValidateBody),
        "zero-valued digest bytes remain structurally valid rather than sentinel values"
    );
    assert!(!validate.exactly_matches_validate(&store_effect, &receipt));
    let validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("local Store root rebinds to its exact Validate effect");
    let second_store_pending = store_ownership
        .exact_pending_adapter_effect_binding(&store_effect)
        .expect("local Store root retains its exact pending projection");
    let second_validate_pending = validate_ownership
        .exact_pending_adapter_effect_binding(&validate_effect)
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
    let command_identity = LocalProposalReadyCommandIdentity::from_exact_pending_handoff(
        tag,
        &manifest,
        &receipt,
        &validated_receipt,
        &second_validate_pending,
    )
    .expect("exact Validate completion has one inert command identity");
    let ready = exact_validate
        .complete_local_proposal(
            &validate_effect,
            &manifest,
            validated_receipt,
            command_identity,
            validate_ownership.owner().lifecycle_ordinal(),
        )
        .expect("exact Validate completion retains local replay evidence");
    assert!(ready.exactly_matches_retry(command_identity, tag, &manifest));
    let mut unsigned_proposal = fixture.proposal.clone();
    unsigned_proposal.signature.clear();
    let proposal_intent = AdapterEffect::Sign {
        tag,
        request: SignRequest::Proposal(unsigned_proposal),
    };
    let proposal_ownership = validate_ownership
        .rebind_as_inherited_adapter_effect(&proposal_intent)
        .expect("local Validate root rebinds to exact ProposalIntent");
    let wrong_ordinal_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&proposal_intent),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 71)],
    )
    .expect("bind the same causal root at a foreign ordinal")
    .pop()
    .expect("one wrong-ordinal ProposalIntent owner");
    assert_eq!(
        wrong_ordinal_ownership
            .owner()
            .causal_origin()
            .lifecycle_key,
        proposal_ownership.owner().causal_origin().lifecycle_key,
    );
    assert_ne!(
        wrong_ordinal_ownership.owner().lifecycle_ordinal(),
        proposal_ownership.owner().lifecycle_ordinal(),
    );
    assert!(!ready.exactly_matches_proposal_intent(
        command_identity,
        &proposal_intent,
        &wrong_ordinal_ownership,
    ));
    let foreign_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&proposal_intent),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                tag,
                72,
                b"foreign local proposal intent owner",
            ),
        ],
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
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    local_body_replay_authority_is_linear_nondecode_and_closed_to_fixed_joins
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    #[allow(clippy::too_many_lines)]
    certified_serve_replay_pair_is_opaque_exact_and_fixed_admission_only
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    certified_pipeline_replay_evidence_is_normalized_inert_and_stage_fixed
);
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
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    direct_signed_replay_wrappers_are_opaque_nondecodable_and_fixed_class
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    remote_proposal_replay_wrappers_are_opaque_exact_and_have_one_runtime_mint
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    invalid_body_runtime_evidence_is_nondecodable_exact_and_fixed_join_only
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    #[allow(clippy::too_many_lines)]
    live_wal_replay_seal_is_linear_nondecodable_and_has_only_specialized_production_mints
);
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
    let local_source = BodyPipelineReplaySourceV1 {
        tag: fixture.tag,
        origin: BodyPipelineOriginV1::LocalBody(fixture.proposal.manifest.clone()),
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
