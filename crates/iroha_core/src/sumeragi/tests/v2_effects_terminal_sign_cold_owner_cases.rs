// Exercise the reachable same-view generation cut with an actual ordinary body.
fn terminal_sign_timeout_certificate(
    transport: &ProductionTransportFixture,
    highest_prepare_qc: Option<wire::QuorumCertificate>,
) -> wire::TimeoutCertificate {
    let round = round(&transport.context, 0);
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: highest_prepare_qc.clone(),
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = transport.validator_keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .expect("the timeout owns three signatures from the exact four-validator roster"),
        }],
    }
}

fn ordinary_view_one_validate_fixture() -> (ReadyBodyFixture, u128, FakeServices, Instant) {
    let mut transport = ProductionTransportFixture::new_with_local_role_at_view(
        Some(crate::sumeragi::v2_core::CommitteeRole::SetAValidator),
        RuntimeQueueConfig::default(),
        1,
        false,
    );
    let local = transport
        .executor
        .local_validator
        .expect("the actual view-one Set-A validator");
    assert_ne!(local, transport.context.leader(1));
    let verified = VerifiedHeightContext::genesis(
        transport.context.clone(),
        transport
            .validator_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP")
            })
            .collect(),
    )
    .expect("authenticate the real view-one committee");
    let directory = TempDir::new().expect("ordinary body lifecycle owner");
    let mut owner = ProductionLifecycleOwnerV1::empty_owner_for_ingress_test(
        verified,
        &transport.validator_keys[usize::try_from(local).expect("local index")],
        directory.path(),
    );
    let ordinals = RuntimeLifecycleOrdinalSource::from_authority(
        owner.bind_empty_ingress_ordinal_authority_for_test(),
    );
    let adapter = transport.executor.runtime.into_driver();
    let now = Instant::now();
    let (runtime, startup) = SerializedV2Runtime::new_with_lifecycle_ordinals(
        adapter,
        Vec::new(),
        now,
        Duration::from_secs(10),
        RuntimeQueueConfig::default(),
        ordinals.clone(),
    )
    .expect("pair the ordinary runtime and durable owner");
    assert!(startup.is_empty());
    transport.executor = V2EffectExecutor::with_runtime(
        runtime,
        BTreeMap::new(),
        transport.context.clone(),
        PeerId::new(transport.requester_key.public_key().clone()),
        Some(local),
        EffectQueueConfig::default(),
    )
    .expect("open without seeded validation or durable body receipts");
    transport._lifecycle_ordinals = ordinals;
    assert!(transport.executor.validated_bodies.is_empty());
    assert!(transport.executor.durable_bodies.is_empty());
    let certificate =
        transport.quorum_certificate(wire::GlobalPhase::Prepare, transport.canonical_commitment);
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    let planner_io = owner.bind_body_store_to_planner_io_for_test(
        &mut services,
        local,
        Arc::clone(&transport.executor.output_guard),
        1,
    );
    crate::sumeragi::v2_worker::tests::install_local_signer_for_test(
        &mut services,
        &transport.validator_keys[usize::try_from(local).expect("local validator index")],
    );
    planner_io
        .install_output_guard_for_test(&mut services, Arc::clone(&transport.executor.output_guard));
    let mut fixture = ReadyBodyFixture {
        transport,
        owner,
        planner_io,
        services,
        _owner_directory: directory,
        certificate,
        ordinal: 0,
    };
    let mut current_services = FakeServices {
        requester_key: Some(fixture.transport.requester_key.clone()),
        ..FakeServices::default()
    };
    let timeout = terminal_sign_timeout_certificate(&fixture.transport, None);
    install_timeout_certificate(&mut fixture, timeout.clone(), &mut current_services, now);
    fixture
        .transport
        .executor
        .step(now, &mut current_services)
        .expect("consume the real initial EnterView");
    assert_eq!(fixture.transport.executor.current_tag().view(), 1);
    let proposer = fixture.transport.context.leader(1);
    let mut proposal = wire::Proposal {
        round: fixture.transport.round,
        proposer,
        subject: fixture.transport.subject,
        manifest: fixture.transport.manifest.clone(),
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: timeout,
            highest_prepare_qc: None,
        }),
        signature: Vec::new(),
    };
    proposal.signature = Signature::new(
        fixture.transport.validator_keys[usize::try_from(proposer).expect("local index")]
            .private_key(),
        &proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    fixture
        .transport
        .executor
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal),
        ))
        .expect("admit the real signed view-one Proposal");
    for turn in 1..12 {
        fixture
            .transport
            .executor
            .step(now + Duration::from_millis(turn), &mut current_services)
            .expect("receive the Proposal through the actual runtime");
        if !current_services.fetch_tasks.is_empty() {
            break;
        }
    }
    let fetch = current_services
        .fetch_tasks
        .last()
        .expect("ordinary Proposal Fetch")
        .clone();
    assert_eq!(fetch.round, fixture.transport.round);
    assert!(fetch.certified_request().is_none());
    let (manifest, chunks) = crate::sumeragi::v2_chunks::encode_payload(
        &fixture.transport.context,
        fixture.transport.round,
        fixture.transport.subject,
        &fixture.transport.body,
    )
    .expect("encode the mandatory canonical RS16 layout")
    .into_parts();
    assert_eq!(manifest, fixture.transport.manifest);
    let mut session = crate::sumeragi::v2_chunks::V2ChunkSession::open(
        &fixture.transport.context,
        manifest.clone(),
    )
    .expect("open the normal bounded chunk session");
    for (index, bytes) in chunks.into_iter().enumerate() {
        let mut chunk = wire::PayloadChunk {
            manifest_hash: HashOf::new(&manifest),
            index: u32::try_from(index).expect("bounded shard count"),
            bytes,
            sender: local,
            signature: Vec::new(),
        };
        chunk.signature = Signature::new(
            fixture.transport.validator_keys[usize::try_from(local).expect("local index")]
                .private_key(),
            &chunk
                .signature_payload(session.validated_manifest())
                .expect("canonical chunk preimage")
                .signature_preimage(),
        )
        .payload()
        .to_vec();
        let authenticated = crate::sumeragi::v2_transport::authenticate_payload_chunk(
            session.validated_manifest(),
            chunk,
            &fixture.transport.context.roster[usize::try_from(local).expect("local index")]
                .validator,
        )
        .expect("authenticate the shard against its actual sender");
        session
            .admit(authenticated)
            .expect("admit the canonical signed shard");
    }
    let reconstructed = session
        .reconstruct()
        .expect("reconstruct the exact RS16 body")
        .expect("all canonical shards are present");
    assert_eq!(reconstructed, fixture.transport.body);
    assert_eq!(
        fixture
            .transport
            .executor
            .complete_body_reconstruction(&fetch, manifest, reconstructed, &mut current_services,)
            .expect("complete the exact ordinary Fetch"),
        CompletionDisposition::Accepted
    );
    for turn in 12..24 {
        fixture
            .transport
            .executor
            .step(now + Duration::from_millis(turn), &mut current_services)
            .expect("deliver real BodyAvailable to ordinary Store");
        if !current_services.store_tasks.is_empty() {
            break;
        }
    }
    let store = current_services
        .store_tasks
        .last()
        .expect("actual ordinary Store task")
        .clone();
    let stored = fixture
        .planner_io
        .execute_ordinary_body_store_for_test(&store);
    assert_eq!(
        fixture
            .transport
            .executor
            .complete_body_store(stored, &mut current_services)
            .expect("publish the genuine ordinary durable-body receipt"),
        CompletionDisposition::Accepted
    );
    let key = (fixture.transport.round, fixture.transport.subject);
    for turn in 24..36 {
        fixture
            .transport
            .executor
            .step(now + Duration::from_millis(turn), &mut current_services)
            .expect("deliver real BodyStored to lifecycle Validate admission");
        if fixture
            .transport
            .executor
            .pending_durable_validate_admissions
            .contains_key(&key)
        {
            break;
        }
    }
    assert_eq!(
        fixture
            .transport
            .executor
            .settle_pending_durable_validate_admissions(&mut fixture.owner, &mut current_services,)
            .expect("admit the actual ordinary Validate owner"),
        1
    );
    let receipt = fixture.transport.executor.durable_bodies[&key].clone();
    let ordinal = fixture
        .owner
        .assert_ready_body_validate_for_test(key.0, key.1, &receipt);
    fixture.ordinal = ordinal;
    (
        fixture,
        ordinal,
        current_services,
        now + Duration::from_millis(40),
    )
}

#[test]
fn same_view_resolved_validation_publishes_commit_sign_and_cold_reopens_exact_owner() {
    let result = crate::sumeragi::sumeragi_thread_builder("terminal-validate-sign-cold-reopen")
        .spawn(|| {
            let (mut fixture, validate_ordinal, mut current_services, now) =
                ordinary_view_one_validate_fixture();
            let old_tag = fixture.transport.executor.current_tag();
            let key = (fixture.transport.round, fixture.transport.subject);
            let pending_digest = fixture.owner.validate_slot_digest_for_retry_test(validate_ordinal);
            let local = usize::try_from(fixture.transport.executor.local_validator.expect("local validator"))
                .expect("local index");
            crate::sumeragi::v2_worker::tests::install_active_tag_for_test(&mut fixture.services, old_tag);
            crate::sumeragi::v2_worker::tests::install_local_signer_for_test(
                &mut fixture.services, &fixture.transport.validator_keys[local],
            );
            assert_eq!(fixture.owner.dispatch_completion_for_test(
                &mut fixture.services, &mut fixture.transport.executor, 0,
            ).expect("queue actual ordinary Validate"), ProductionCompletionDispatchV1::ValidateQueued { ordinal: validate_ordinal });
            fixture.planner_io.activate_one_lifecycle_validate();
            let earlier = ProductionTransportFixture::new();
            assert_eq!(earlier.context, fixture.transport.context);
            let high = earlier.quorum_certificate(wire::GlobalPhase::Prepare, earlier.canonical_commitment);
            assert_ne!(high.subject, fixture.transport.subject);
            let upgrade = terminal_sign_timeout_certificate(&fixture.transport, Some(high));
            install_timeout_certificate(&mut fixture, upgrade, &mut current_services, now);
            let current = fixture.transport.executor.current_tag();
            assert_eq!(current.view(), old_tag.view());
            assert!(current.strictly_advances(old_tag));
            assert_eq!(fixture.planner_io.execute_held_lifecycle_validate_fixture(
                fixture.transport.canonical_commitment,
                Arc::clone(&fixture.transport.executor.output_guard),
            ), 1);
            let completion = match fixture.services.take_next_lifecycle_completion().expect("physical Validate completion") {
                LifecycleCompletionTakeV1::Validate(completion) => completion,
                _ => panic!("one actual Validate result"),
            };
            let successor = fixture.owner.publish_validate_successor_for_retry_test(completion, validate_ordinal, false);
            assert!(matches!(fixture.owner.dispatch_ready_validate_successor_for_test(
                &mut fixture.services, &mut fixture.transport.executor, successor, 0,
            ).expect("the obsolete generation becomes a real inert terminal"),
                crate::sumeragi::v2_lifecycle_coordinator::ReadyValidateSuccessorDispatchV1::Resolved(
                    ProductionCompletionDispatchV1::ValidateNoSuccessor { ordinal }
                ) if ordinal == validate_ordinal));
            let ledger_root = fixture._owner_directory.path().join("ledger");
            let terminal = fixture.owner.resolved_validate_owner_snapshot_for_test(
                validate_ordinal, pending_digest, &ledger_root,
            );
            let io_before = fixture.planner_io.lifecycle_validate_io_snapshot();
            fixture.transport.executor.enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.certificate.clone()),
            )).expect("current PrepareQC revives exact body validation authority");
            let mut settled = 0;
            for turn in 1..32 {
                let executor = &mut fixture.transport.executor;
                executor.step(now + Duration::from_millis(turn), &mut current_services)
                    .expect("service the actual new-generation Prepare/body FIFO");
                let output_summary = executor.settle_pending_lifecycle_output_admissions(&mut fixture.owner, &mut current_services)
                    .expect("persist exact current control output owners");
                // Fresh terminal publication ends this fixture turn; duplicates do not.
                if output_summary.requires_outer_executor_yield() {
                    continue;
                }
                executor.settle_pending_durable_validate_admissions(&mut fixture.owner, &mut current_services)
                    .expect("rejoin the current Validate owner to its immutable result");
                settled += executor.settle_pending_released_validate_apply_publication(
                    &mut fixture.owner, &mut current_services,
                ).expect("reuse actual terminal success for current Commit Sign");
                if settled != 0 { break; }
            }
            assert_eq!(
                settled, 1,
                "terminal Sign did not settle: tag={:?}, body={:?}, replay={}, deferred_apply={}, status={:?}",
                fixture.transport.executor.current_tag(),
                fixture.transport.executor.runtime.driver().body_state_for_test(key.0, key.1),
                fixture.transport.executor.pending_resolved_validate_replay.is_some(),
                fixture.transport.executor.pending_released_lifecycle_validate_apply.is_some(),
                fixture.transport.executor.status(),
            );
            assert_eq!(fixture.transport.executor.current_tag(), current);
            assert_eq!(fixture.planner_io.lifecycle_validate_io_snapshot(), io_before);
            assert!(current_services.apply_tasks.is_empty());
            assert!(current_services.sign_tasks.is_empty(), "the durable Sign must have one lifecycle owner before dispatch");
            fixture.owner.assert_resolved_validate_owner_retained_for_test(&terminal, &ledger_root, 0);
            let sign = fixture.owner.resolved_commit_sign_snapshot_for_test(
                &fixture.certificate, &ledger_root,
            );
            let (mut reopened, _leader_wire_gate) = reopen_body_owner_fixture(fixture, FixtureValidationReplay::Validated);
            reopened.owner.resolved_validate_cold_snapshot_for_test(&terminal, &ledger_root);
            reopened.owner.assert_resolved_commit_sign_cold_for_test(&sign, &ledger_root);
            assert!(reopened.owner.apply_ordinals_for_retry_test().is_empty());
            let cold_io = reopened.planner_io.lifecycle_validate_io_snapshot();
            assert_eq!(cold_io.command_depth(), 0);
            assert_eq!(cold_io.physical_admissions(), 0);
            assert_eq!(cold_io.active(), 0);
            let mut services = FakeServices::default();
            assert_eq!(reopened.transport.executor.settle_pending_released_validate_apply_publication(
                &mut reopened.owner, &mut services,
            ).expect("an already-published Sign cannot replay Validate again"), 0);
            assert!(services.sign_tasks.is_empty());
            assert!(services.apply_tasks.is_empty());
            assert_eq!(reopened.planner_io.lifecycle_validate_io_snapshot(), cold_io);
            reopened.owner.assert_resolved_commit_sign_cold_for_test(&sign, &ledger_root);
            assert!(!reopened.transport.executor.output_guard.restart_required());
            assert!(!reopened.transport.executor.status().fail_closed);
            reopened.planner_io.detach(&mut reopened.services);
            assert_eq!(key.0.view, 1);
        })
        .expect("production consensus stack")
        .join();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}
