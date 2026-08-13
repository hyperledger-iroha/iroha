#[test]
fn recovered_wal_sign_status_publication_is_exact_last_and_unwired() {
    let source = include_str!("v2.rs");
    let body_store_source = include_str!("v2_body_store.rs");
    let (production, _) = source
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("locate unconditional production/test boundary");
    let publication = production
        .split_once("// RECOVERED_WAL_SIGN_STATUS_PUBLICATION_BEGIN")
        .expect("recovered Sign publication begins")
        .1
        .split_once("// RECOVERED_WAL_SIGN_STATUS_PUBLICATION_END")
        .expect("recovered Sign publication ends")
        .0;
    assert!(
        publication.contains("#[cfg(test)]"),
        "the superseded parts-based publication remains test-only"
    );
    for required in [
        "struct PublishedRecoveredWalLifecycleStartup<'registry>",
        "struct RecoveredWalLifecycleOpenPublicationError<'registry>",
        "OpenedRecoveredWalSignLifecycleCut<'registry>",
        "RecoveredWalSignLifecycleOpenError<'registry>",
        "fn publish_open_result(",
        "let opened = match opened",
        "if let Err(error) = adapter.publish_status()",
        "RecoveredWalLifecycleOpenPublicationFailure::Status",
        "fn open_coordinator_and_publish(",
        "installed.open_coordinator_from_verified(",
        "fn open_coordinator_and_publish_for_test(",
        "installed.open_coordinator_for_test(",
    ] {
        assert!(
            publication.contains(required),
            "status-last publication omitted {required}"
        );
    }
    assert_eq!(publication.matches("adapter.publish_status()").count(), 1);
    let opened = publication
        .find("let opened = match opened")
        .expect("inner exact open is classified");
    let status = publication
        .find("adapter.publish_status()")
        .expect("adapter status is published");
    assert!(opened < status, "status must follow the exact open result");
    let owner_factory = production
        .split_once("pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(")
        .expect("locate the sole production owner factory")
        .1
        .split_once("/// Open an empty-marker test body store")
        .expect("locate the end of the production owner factory")
        .0;
    let canonical_factory = owner_factory
        .split_once("fn open_production_lifecycle_owner_v1_at_authenticated_roots(")
        .expect("locate the private authenticated-root implementation")
        .0;
    let factory_inputs = canonical_factory
        .find("factory_inputs: RecoveredLifecycleOwnerFactoryInputsV1")
        .expect("factory consumes the adapter-bound execution/storage seal");
    assert!(
        canonical_factory.contains("body_store: super::v2_body_store::QuarantinedV2BodyStore")
    );
    assert!(!canonical_factory.contains("body_store: super::v2_body_store::V2BodyStore"));
    assert!(
        !canonical_factory.contains("body_store: super::v2_body_store::RevalidatedV2BodyStore")
    );
    let residual = canonical_factory
        .find("if !self.effects.is_empty()")
        .expect("factory rejects residual effects before marker replay");
    let startup_binding = canonical_factory
        .find("Arc::ptr_eq(&adapter_owner, &self.factory_owner)")
        .expect("factory input remains bound to this exact authenticated startup");
    let context_binding = canonical_factory
        .find("storage.context_id != context.id()")
        .expect("factory binds the storage authority to the recovered context");
    let body_root = canonical_factory
        .find("body_store.matches_lifecycle_storage_root(")
        .expect("factory joins the body store to the sealed root and policy");
    let wal_path = canonical_factory
        .find("self.adapter.wal.matches_path(&storage.wal_path)")
        .expect("factory joins the adapter to the recovery-sealed WAL path");
    let apply_service = canonical_factory
        .find("let apply_service = super::v2_apply::V2ApplyService::new(")
        .expect("factory constructs one exact marker/live Apply service");
    let replay = canonical_factory
        .find(".into_revalidated_lifecycle_startup(")
        .expect("factory consumes the fixed marker replay cut");
    let sealed_parts = canonical_factory
        .find("let RecoveredLifecycleStorageAuthorityV1 {")
        .expect("factory opens the storage authority only after validation");
    let authenticated_roots = canonical_factory
        .find("self.open_production_lifecycle_owner_v1_at_authenticated_roots(")
        .expect("factory enters the private implementation after target checks");
    let kura_binding = canonical_factory
        .find("owner.with_recovered_kura_binding_and_apply_service(")
        .expect("factory retains the Kura and replay service in one owner transition");
    assert!(factory_inputs < residual);
    assert!(residual < startup_binding);
    assert!(startup_binding < context_binding);
    assert!(context_binding < body_root);
    assert!(body_root < wal_path);
    assert!(wal_path < apply_service);
    assert!(apply_service < replay);
    assert!(replay < sealed_parts);
    assert!(sealed_parts < authenticated_roots);
    assert!(authenticated_roots < kura_binding);
    for forbidden in [
        "kura: &Kura",
        "ledger_root: &std::path::Path",
        "serve_payload_root: &std::path::Path",
        "body_root: &std::path::Path",
        "body_signature_policy:",
    ] {
        assert!(
            !canonical_factory.contains(forbidden),
            "production owner factory accepts forbidden raw target {forbidden}"
        );
    }
    let control_projection = owner_factory
        .find("project_recovered_wal_control_sign")
        .expect("factory projects the sealed control authority");
    let decision_projection = owner_factory
        .find("project_recovered_wal_decision_fetch")
        .expect("factory projects the sealed Decision authority");
    let decision_body_preflight = owner_factory
        .find("detach_recovered_decision_apply_body")
        .expect("factory preflights an opaque same-store Decision body");
    let body_handoff = owner_factory
        .find("into_lifecycle_owner_store")
        .expect("factory consumes the revalidated same-store handoff");
    let serve_open = owner_factory
        .find("CertifiedServePayloadStoreV1::open(")
        .expect("factory opens the Serve store");
    let owner_open = owner_factory
        .find(".into_owner(registry, payload_store, body_store)")
        .expect("factory constructs the recovered owner");
    assert!(residual < control_projection);
    assert!(control_projection < body_handoff);
    assert!(decision_projection < decision_body_preflight);
    assert!(decision_body_preflight < body_handoff);
    assert!(body_handoff < serve_open);
    assert!(serve_open < owner_open);
    assert!(!owner_factory.contains("publish_recovered_adapter_status"));
    assert!(!owner_factory.contains("recovery: AuthenticatedLifecycleRecoveryCut"));
    assert!(
        owner_factory
            .contains("ProductionLifecycleOwnerV1::open_recovered_decision_apply_startup")
    );
    assert!(
        !owner_factory.contains("restart-closed Decision Apply publication is not implemented")
    );
    assert!(!owner_factory.contains("V2BodyStore::open_with_policy("));
    assert!(!owner_factory.contains("body_root:"));
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
        .expect("fixed replay derives the recovered-finality marker subject");
    let subject_filter = quarantine
        .find(".retain_recovered_markers_for_subject(subject)")
        .expect("fixed replay filters markers to recovered finality first");
    let authority_filter = quarantine
        .find(".retain_recovered_markers_for_authority(validation_authority)")
        .expect("fixed replay then filters markers to authenticated WAL authority");
    let semantic_replay = quarantine
        .find(".revalidate_recovered_markers(|body|")
        .expect("fixed replay semantically validates retained markers");
    let seal = quarantine
        .find("self.0.into_revalidated_startup()")
        .expect("fixed replay seals only replayed marker state");
    assert!(finality < subject_filter);
    assert!(subject_filter < authority_filter);
    assert!(authority_filter < semantic_replay);
    assert!(semantic_replay < seal);
    for forbidden in [
        "CandidateAdmission",
        "PendingRuntimeEffectBinding",
        "RuntimeEffectOwnership",
        "into_parts",
        "pub(crate) fn coordinator(",
        "pub(crate) fn effect(",
        "pub(crate) fn receipt(",
    ] {
        assert!(
            !publication.contains(forbidden),
            "status publication exposes forbidden surface {forbidden}"
        );
    }
    for runner_source in [
        include_str!("v2_runner.rs"),
        include_str!("v2_worker.rs"),
        include_str!("v2_effects.rs"),
    ] {
        assert!(!runner_source.contains("open_coordinator_and_publish("));
        assert!(!runner_source.contains("PublishedRecoveredWalLifecycleStartup"));
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
    let (durable, validated) =
        validated_receipts_for_manifest(&adapter.wire_context, &manifest);
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
    let wire::ConsensusMessageV2Payload::Proposal(canonical_proposal) = &canonical.payload
    else {
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
    let conflicting = AuthenticatedConsensusMessage::for_test(conflicting);
    assert!(matches!(
        adapter.ensure_authenticated_manifest_compatible(&conflicting),
        Err(AdapterError::ConflictingManifest)
    ));
    assert!(!adapter.fail_closed);
}
