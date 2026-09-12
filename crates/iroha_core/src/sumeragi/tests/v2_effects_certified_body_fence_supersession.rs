mod certified_body_fence_supersession {
    //! Exercise certified body completion after a real timeout changes its reducer owner.

    use super::*;
    use crate::sumeragi::{
        v2_lifecycle_coordinator::{
            LifecycleWorkClass, ProductionCompletionDispatchErrorV1,
            ProductionCompletionDispatchV1, ProductionCompletionReadyWorkV1,
            ProductionLifecycleOwnerV1,
        },
        v2_worker::{
            LifecycleCompletionTakeV1, ProductionV2Services, tests::LifecyclePlannerIoFixture,
        },
    };

    struct ReadyBodyFixture {
        transport: ProductionTransportFixture,
        owner: ProductionLifecycleOwnerV1,
        planner_io: LifecyclePlannerIoFixture,
        services: ProductionV2Services,
        _owner_directory: TempDir,
        certificate: wire::QuorumCertificate,
        ordinal: u128,
    }

    fn ready_body_fixture() -> ReadyBodyFixture {
        // A fixed roster index can be a dormant Set-B validator. The current
        // leader is always eligible to fetch a certified candidate body.
        let mut transport = ProductionTransportFixture::new_with_local_role_and_queue_config(
            Some(crate::sumeragi::v2_core::CommitteeRole::Leader),
            RuntimeQueueConfig::default(),
        );
        let leader = transport.context.leader(0);
        assert_eq!(transport.executor.local_validator, Some(leader));
        let proofs = transport
            .validator_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("frozen validator PoP")
            })
            .collect();
        let verified = VerifiedHeightContext::genesis(transport.context.clone(), proofs)
            .expect("authenticate the four-validator owner context");
        let owner_directory = TempDir::new().expect("temporary certified completion owner");
        let mut owner = ProductionLifecycleOwnerV1::empty_owner_for_ingress_test(
            verified,
            &transport.validator_keys[usize::try_from(leader).expect("leader index fits")],
            owner_directory.path(),
        );
        let ordinals = RuntimeLifecycleOrdinalSource::from_authority(
            owner.bind_empty_ingress_ordinal_authority_for_test(),
        );
        let adapter = transport.executor.runtime.into_driver();
        let (runtime, startup) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            Vec::new(),
            Instant::now(),
            Duration::from_secs(10),
            RuntimeQueueConfig::default(),
            ordinals.clone(),
        )
        .expect("runtime and coordinator share the same live ordinal authority");
        assert!(startup.is_empty());
        transport.executor = V2EffectExecutor::with_runtime(
            runtime,
            BTreeMap::new(),
            transport.context.clone(),
            PeerId::new(transport.requester_key.public_key().clone()),
            Some(leader),
            EffectQueueConfig::default(),
        )
        .expect("the empty body executor retains the paired runtime authority");
        transport._lifecycle_ordinals = ordinals;
        let certificate = transport
            .quorum_certificate(wire::GlobalPhase::Prepare, transport.canonical_commitment);
        let driver = transport.executor.runtime.driver_mut_for_test();
        let authenticated = driver
            .authenticate(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
            ))
            .expect("the missing-body PrepareQC is signed by three frozen validators");
        let effects = driver
            .receive_authenticated(authenticated)
            .expect("the authenticated PrepareQC creates real missing-body reducer work")
            .into_effects();
        assert!(
            matches!(
                effects.as_slice(),
            [AdapterEffect::FetchBody { manifest: None, certificate: Some(observed), certified_sources, .. }]
                if observed == &certificate
                    && certified_sources.iter().eq(transport.context.roster.iter().map(|entry| &entry.validator))
            ),
            "the eligible validator must emit one exact Fetch: {effects:?}"
        );
        transport
            .executor
            .runtime
            .retain_retransmit_effect_ownership_for_test(&effects)
            .expect("bind the exact reducer-emitted Fetch owner");
        let mut fetch_services = FakeServices {
            requester_key: Some(transport.requester_key.clone()),
            ..FakeServices::default()
        };
        assert_eq!(
            transport
                .executor
                .consume_effects(effects, &mut fetch_services)
                .expect("publish one real certified Fetch request"),
            1,
        );
        let task = fetch_services
            .fetch_tasks
            .pop()
            .expect("one exact Fetch task");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(task.certified_request().expect("signed request")),
            manifest: transport.manifest.clone(),
            body: transport.body.clone(),
            responder: transport.context.roster[0].validator.clone(),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            transport.validator_keys[0].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        let (_ingress_directory, ingress, _gate) = transport.bound_certified_response_ingress();
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
                )),
                transport.context.roster[0].validator.clone(),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        let response_ordinal = ingress.state.lock().last_admission_ordinal;
        let selected = transport
            .executor
            .prepare_lifecycle_ingress_selector(&ingress, response_ordinal)
            .expect("authenticate the exact signed response selector");
        let (_, _, _, _, _, _, source) = selected
            .certified_fetch_ready_authority_for_test()
            .expect("derive the exact authenticated response's Fetch wake authority");
        // The production ingress admission obtains the missing manifest from
        // the authenticated response while retaining the original Fetch shape.
        let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
        services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
        let mut planner_io = owner.bind_body_store_to_planner_io_for_test(
            &mut services,
            leader,
            Arc::clone(&transport.executor.output_guard),
            1,
        );
        planner_io.install_output_guard_for_test(
            &mut services,
            Arc::clone(&transport.executor.output_guard),
        );
        services
            .enqueue_body_fetch(task)
            .expect("install the exact Fetch service owner");
        let planned = owner.plan_ingress_turn_for_test(
            &services,
            &transport.executor,
            transport.executor.lifecycle_mode_rank_snapshot(),
            selected,
            crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(
                &transport.context,
            ),
        );
        let queued = match planned {
            Ok(ProductionIngressTurnPreparation::Queued(queued)) => queued,
            Ok(ProductionIngressTurnPreparation::CapacityWait(_)) => {
                panic!("exact capacity is available")
            }
            Err(error) => panic!(
                "the authenticated response must enter Phase A: {}",
                error.reason()
            ),
        };
        let ordinal = queued.ordinal();
        planner_io.execute_one_certified_fetch(Arc::clone(&transport.executor.output_guard));
        let completion = match services
            .take_next_lifecycle_completion()
            .expect("fsynced body completion")
        {
            LifecycleCompletionTakeV1::CertifiedFetch(completion) => completion,
            _ => panic!("the worker must return the exact certified body carrier"),
        };
        assert!(
            owner
                .complete_certified_fetch_for_test(
                    &mut transport.executor,
                    &mut services,
                    &ingress,
                    completion,
                )
                .is_ok(),
            "the real Phase B must publish a durable Ready Fetch"
        );
        assert!(matches!(
            owner.fetch_wait_projection_for_test(ordinal, source),
            (Some(LifecycleState::Ready), Some(2), None, false)
        ));
        assert!(transport.executor.pending_fetches.is_empty());
        assert!(transport.executor.certified_work.is_empty());
        assert!(transport.executor.outstanding_requests.is_empty());
        ReadyBodyFixture {
            transport,
            owner,
            services,
            planner_io,
            _owner_directory: owner_directory,
            certificate,
            ordinal,
        }
    }

    fn signed_timeout_certificate(
        fixture: &ReadyBodyFixture,
        protect_body: bool,
    ) -> wire::TimeoutCertificate {
        let highest_prepare_qc = protect_body.then(|| fixture.certificate.clone());
        let round = wire::ConsensusRound {
            view: fixture.transport.executor.current_tag().view(),
            ..fixture.transport.round
        };
        let preimage = wire::TimeoutVote {
            round,
            highest_prepare_qc: highest_prepare_qc.clone(),
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = fixture.transport.validator_keys[..3]
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
                .expect("authenticate the exact timeout quorum"),
            }],
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RetirementOrder {
        BeforeCurrentEffects,
        AfterCurrentFetch,
        AfterCoalescedCurrentStore,
    }

    // Split one ordinary runtime step at its production effect-dispatch seam,
    // so retirement can race the actual owned effects in either order.
    fn install_timeout(
        fixture: &mut ReadyBodyFixture,
        protect_body: bool,
        services: &mut FakeServices,
        now: Instant,
    ) -> Vec<AdapterEffect> {
        let timeout = signed_timeout_certificate(fixture, protect_body);
        install_timeout_certificate(fixture, timeout, services, now)
    }

    fn install_timeout_certificate(
        fixture: &mut ReadyBodyFixture,
        timeout: wire::TimeoutCertificate,
        services: &mut FakeServices,
        now: Instant,
    ) -> Vec<AdapterEffect> {
        let executor = &mut fixture.transport.executor;
        if executor.lifecycle_live_clocks_are_unarmed() {
            executor
                .arm_live_clocks(
                    ProductionLifecycleLiveClockActivationPermitV1::for_test(),
                    now,
                )
                .expect("arm the ordinary runtime clocks after body startup");
        }
        executor
            .enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
            ))
            .expect("enqueue the real authenticated timeout quorum");
        executor
            .publish_external_lifecycle_owners()
            .expect("publish the exact runtime owner census");
        assert!(
            executor
                .runtime
                .decided_body()
                .expect("read the exact decision frontier")
                .is_none()
        );
        let wal_step = executor
            .output_guard
            .begin_fail_stop_operation()
            .expect("the ordinary runtime WAL boundary is open");
        let step = executor
            .runtime
            .step_effects(now)
            .expect("normal TC scheduling installs and persists the new reducer generation");
        executor
            .runtime
            .take_scheduler_ownership()
            .expect("consume the real scheduler proof");
        wal_step.complete();
        executor
            .finish_runtime_step_reconciliation(services)
            .expect("retain the runtime's actual completion terminals");
        assert!(
            executor
                .runtime
                .decided_body()
                .expect("read the resulting decision frontier")
                .is_none()
        );
        let RuntimeStep::Advanced(effects) = step else {
            panic!("the queued TC must advance the ordinary runtime");
        };
        let observed = effects.clone();
        let frontier = executor
            .runtime
            .reconciliation_frontier()
            .expect("derive the actual post-TC reconciliation frontier");
        executor
            .preflight_effect_batch_frontier(&effects, frontier)
            .expect("the real EnterView leads the exact current effect batch");
        let ownership = EffectRuntime::take_effect_ownership(&mut executor.runtime, &effects)
            .expect("transfer the real runtime effect owners into the executor");
        assert!(
            executor
                .plan_local_proposal_replay_consumptions(&effects, &ownership)
                .expect("the TC carries no local ProposalIntent replay")
                .is_empty()
        );
        assert!(
            executor
                .runtime
                .take_live_proposal_intent_wal_sign(&effects)
                .expect("the TC has no live ProposalIntent WAL sidecar")
                .is_none()
        );
        executor
            .retain_effect_batch_at_frontier(effects, ownership, frontier)
            .expect("retain all genuine current effects before any service dispatch");
        executor
            .commit_reconciliation_frontier(frontier, services)
            .expect("commit the ordinary post-retention lock reconciliation");
        executor
            .consume_leader_wire_runtime_terminals(services)
            .expect("consume the actual TC ingress retirement before lifecycle completion");
        observed
    }

    fn drive_current_body_to_validate(
        fixture: &mut ReadyBodyFixture,
        services: &mut FakeServices,
        started: Instant,
        new_tag: EventTag,
        require_periodic_store: bool,
    ) -> u128 {
        let key = (fixture.transport.round, fixture.transport.subject);
        let receipt = fixture.transport.executor.durable_bodies[&key].clone();
        let periodic = started + fixture.transport.executor.runtime.retransmit_interval();
        assert!(periodic < started + Duration::from_secs(10));
        let mut saw_periodic_store = false;
        let mut saw_fifo_validate = false;
        for _ in 0..12 {
            let executor = &mut fixture.transport.executor;
            if executor
                .pending_durable_validate_admissions
                .contains_key(&key)
            {
                break;
            }
            executor
                .step(periodic, services)
                .expect("normal periodic/FIFO execution must progress the current protected body");
            let observed = executor
                .last_runtime_step_observation_for_test()
                .expect("the ordinary runtime exposes its selected test observation");
            if observed.selected() == Some(RuntimeSelectedOwnerKind::PeriodicTimer)
                && observed.store_count() == 1
            {
                assert_eq!(observed.broadcast_count(), 2, "{observed:?}");
                assert_eq!(observed.effect_count(), 3, "{observed:?}");
                assert!(
                    observed.batch_has_exact_prepare_qc(&fixture.certificate),
                    "the periodic Store accompanies the exact protected PrepareQC and its TC"
                );
                saw_periodic_store = true;
            }
            saw_fifo_validate |= observed.selected() == Some(RuntimeSelectedOwnerKind::Fifo)
                && observed.validate_count() == 1;
        }
        let executor = &mut fixture.transport.executor;
        assert!(
            !require_periodic_store || saw_periodic_store,
            "a previously coalesced Store must be re-emitted by the real periodic timer"
        );
        assert!(
            saw_fifo_validate,
            "the genuine BodyStored FIFO completion must emit Validate"
        );
        assert_eq!(
            executor
                .runtime
                .replayed_body_authority_certificate()
                .expect("rejoin the protected body's full authenticated QC"),
            Some(fixture.certificate.clone())
        );
        let pending = executor
            .pending_durable_validate_admissions
            .get(&key)
            .expect("ordinary execution must create one pending current Validate");
        assert!(pending.exactly_retains_for_test(
            &AdapterEffect::ValidateBody {
                tag: new_tag,
                round: key.0,
                subject: key.1,
            },
            false
        ));
        assert!(!pending.projects_local_proposal_handoff_for_test());
        assert_eq!(
            executor
                .settle_pending_durable_validate_admissions(&mut fixture.owner, services)
                .expect("the production owner admits the protected current Validate"),
            1
        );
        assert!(executor.pending_durable_validate_admissions.is_empty());
        let validate_ordinal = fixture
            .owner
            .assert_ready_body_validate_for_test(key.0, key.1, &receipt);
        assert!(validate_ordinal > fixture.ordinal);
        assert_eq!(executor.durable_bodies.get(&key), Some(&receipt));
        assert_eq!(executor.body_pipeline_owners[&key].tag, new_tag);
        assert!(
            services.store_tasks.is_empty(),
            "the fsynced body uses the ordinary durable fast path"
        );
        assert!(
            services.fetch_tasks.is_empty(),
            "the immutable protected body is already local"
        );
        assert!(!executor.status().fail_closed);
        validate_ordinal
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RetirementFailure {
        ClosedOutput,
        LedgerPublication,
    }

    fn assert_reopened_body(transport: &ProductionTransportFixture, root: &std::path::Path) {
        let key = (transport.round, transport.subject);
        let receipt = &transport.executor.durable_bodies[&key];
        let store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.join("body"),
            transport.context.clone(),
        )
        .expect("the same immutable body frame reopens after completion retirement");
        assert_eq!(store.receipt(key.0, key.1).as_ref(), Some(receipt));
        assert_eq!(
            store
                .load_canonical_wire(receipt)
                .expect("revalidate the exact fsynced body"),
            transport.body,
        );
    }

    fn assert_cancelled_body_cold_rehydration(fixture: ReadyBodyFixture, validate_ordinal: u128) {
        let ReadyBodyFixture {
            mut transport,
            owner,
            planner_io,
            mut services,
            _owner_directory: directory,
            certificate,
            ordinal,
        } = fixture;
        let expected = owner.body_recovery_snapshot_for_test(ordinal, validate_ordinal);
        let key = (transport.round, transport.subject);
        let receipt = transport.executor.durable_bodies[&key].clone();
        let validator = transport.context.leader(0);
        let verified = VerifiedHeightContext::genesis(
            transport.context.clone(),
            transport
                .validator_keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("the frozen validator PoP remains valid")
                })
                .collect(),
        )
        .expect("reauthenticate the same immutable restart context");
        let wal_path = transport
            ._directory
            .path()
            .join("transport-regression-safety.wal");
        planner_io.detach(&mut services);
        drop(services);
        drop(owner);
        drop(transport.executor);
        let mut recovered = SumeragiV2Adapter::reopen_body_owner_for_test(
            &wal_path,
            directory.path(),
            verified,
            validator,
            &transport.validator_keys
                [usize::try_from(validator).expect("the frozen local index fits")],
            AdapterFingerprints {
                node: Hash::new(b"production transport node"),
                build: Hash::new(b"production transport build"),
                config: Hash::new(b"production transport config"),
            },
            [0x63; 32],
            None,
        );
        recovered.assert_body_recovery_snapshot_for_test(&expected);
        assert!(recovered.exact_recovered_body_pipeline_join_for_test());
        let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
        services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
        let (executor, mut planner_io, _leader_wire_gate, ordinals) = recovered
            .bind_recovered_cancelled_body_executor_for_test(
                &wal_path,
                &mut services,
                ConsensusOutputGuard::isolated(),
                validator,
            );
        crate::sumeragi::v2_worker::tests::install_active_tag_for_test(
            &mut services,
            executor.current_tag(),
        );
        crate::sumeragi::v2_worker::tests::install_local_signer_for_test(
            &mut services,
            &transport.validator_keys
                [usize::try_from(validator).expect("the recovered local index fits")],
        );
        transport.executor = executor;
        transport._lifecycle_ordinals = ordinals;
        recovered.assert_body_recovery_snapshot_for_test(&expected);
        assert!(
            transport
                .executor
                .published_lifecycle_store_retry_markers
                .is_empty(),
            "cold cancellation must never recreate the obsolete Store publication marker"
        );
        assert!(
            transport
                .executor
                .published_lifecycle_validate_retry_markers
                .is_empty()
        );
        assert_eq!(transport.executor.durable_validate_retry_seals.len(), 1);
        assert_eq!(
            transport.executor.durable_validate_retry_seals[&key].lifecycle_ordinal(),
            Some(validate_ordinal)
        );
        assert_eq!(transport.executor.durable_bodies.get(&key), Some(&receipt));
        assert_eq!(
            transport.executor.recovered_bodies.get(&key),
            Some(&(transport.manifest.clone(), receipt.clone()))
        );
        assert_eq!(
            transport
                .executor
                .runtime
                .replayed_body_authority_certificate()
                .expect("cold startup retains the full protected PrepareQC"),
            Some(certificate)
        );
        assert_eq!(
            recovered
                .dispatch_completion_for_test(&mut services, &mut transport.executor, 0)
                .expect(
                    "the exact recovered Validate remains executable through normal Completion"
                ),
            ProductionCompletionDispatchV1::ValidateQueued {
                ordinal: validate_ordinal
            }
        );
        assert!(!transport.executor.status().fail_closed);
        let queued = planner_io.lifecycle_validate_io_snapshot();
        assert_eq!(queued.command_depth(), 1);
        assert_eq!(queued.physical_admissions(), 1);
        assert_eq!(queued.queued(), 1);
        assert_eq!(queued.active(), 0);
        assert_eq!(queued.completion_pending(), 0);
        assert_eq!(queued.completion_owners(), 0);
        planner_io.activate_one_lifecycle_validate();
        assert_eq!(
            planner_io.execute_held_lifecycle_validate_fixture(
                transport.canonical_commitment,
                Arc::clone(&transport.executor.output_guard),
            ),
            1,
            "the recovered stored body must execute its first validation exactly once"
        );
        let completion = match services
            .take_next_lifecycle_completion()
            .expect("take the recovered Validate worker's guarded completion")
        {
            LifecycleCompletionTakeV1::Validate(completion) => completion,
            _ => panic!("the recovered Validate worker returned a foreign completion class"),
        };
        recovered.publish_validated_body_completion_for_test(completion, validate_ordinal);
        let settled = planner_io.lifecycle_validate_io_snapshot();
        assert_eq!(settled.command_depth(), 0);
        assert_eq!(settled.physical_admissions(), 0);
        assert_eq!(settled.queued(), 0);
        assert_eq!(settled.active(), 0);
        assert_eq!(settled.completion_pending(), 0);
        assert_eq!(settled.completion_owners(), 0);
        assert!(!transport.executor.output_guard.restart_required());
        assert!(!transport.executor.status().fail_closed);
        planner_io.detach(&mut services);
        assert_reopened_body(&transport, directory.path());
    }

    fn retry_body_after_timeout(
        stage: LifecycleWorkClass,
        protect_body: bool,
        order: RetirementOrder,
        failure: Option<RetirementFailure>,
        cold_recovery: bool,
    ) {
        let mut fixture = ready_body_fixture();
        if stage == LifecycleWorkClass::Store {
            let advanced = fixture
                .owner
                .dispatch_completion_for_test(
                    &mut fixture.services,
                    &mut fixture.transport.executor,
                    0,
                )
                .expect("the exact Ready Fetch must publish one Store successor");
            let ProductionCompletionDispatchV1::BodyStageAdvanced {
                parent_ordinal,
                child_ordinal,
                child: LifecycleWorkClass::Store,
            } = advanced
            else {
                panic!("Fetch must advance to Store");
            };
            assert_eq!(parent_ordinal, fixture.ordinal);
            fixture.ordinal = child_ordinal;
        }
        assert!(fixture.transport.executor.pending_stores.is_empty());
        assert!(fixture.transport.executor.pending_applications.is_empty());
        let old_tag = fixture.transport.executor.current_tag();
        let sign = fixture
            .transport
            .executor
            .runtime
            .driver_mut_for_test()
            .timeout_elapsed(old_tag)
            .expect("persist the ordinary timeout signature fence");
        assert!(matches!(
            sign.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));
        let before_fence = fixture
            .transport
            .executor
            .lifecycle_reducer_fence_observation();
        let blocked = fixture
            .owner
            .dispatch_completion_for_test(&mut fixture.services, &mut fixture.transport.executor, 0)
            .expect("an active signer must park the exact body carrier without a fault");
        let ProductionCompletionDispatchV1::ReducerFenceWait { ordinal, wait } = blocked else {
            panic!("the body carrier must wait on the reducer fence");
        };
        assert_eq!(ordinal, fixture.ordinal);
        assert_eq!(wait.source(), before_fence.source());
        assert_eq!(wait.observed_generation(), before_fence.generation());

        let started = Instant::now();
        let mut current_services = FakeServices {
            requester_key: Some(fixture.transport.requester_key.clone()),
            ..FakeServices::default()
        };
        let entered = install_timeout(&mut fixture, protect_body, &mut current_services, started);
        let new_tag = fixture.transport.executor.current_tag();
        assert!(new_tag.strictly_advances(old_tag));
        assert_eq!(new_tag.view(), old_tag.view() + 1);
        assert!(entered.iter().any(
            |effect| matches!(effect, AdapterEffect::EnterView { tag, .. } if *tag == new_tag)
        ));
        assert_eq!(entered.iter().any(|effect| matches!(effect,
            AdapterEffect::FetchBody { tag, round, subject, certificate: Some(certificate), .. }
                if *tag == new_tag && *round == fixture.transport.round
                    && *subject == fixture.transport.subject && certificate == &fixture.certificate
        )), protect_body, "only the protected immutable body is reseeded after view installation");
        let after_fence = fixture
            .transport
            .executor
            .lifecycle_reducer_fence_observation();
        assert_eq!(after_fence.source(), before_fence.source());
        assert!(after_fence.generation() > wait.observed_generation());

        let key = (fixture.transport.round, fixture.transport.subject);
        if order != RetirementOrder::BeforeCurrentEffects {
            assert!(protect_body);
            fixture
                .transport
                .executor
                .step(started, &mut current_services)
                .expect("drain the actual EnterView and protected Fetch with their runtime owners");
            assert!(fixture.transport.executor.retained_effect_batch.is_none());
            assert_eq!(
                fixture.transport.executor.body_pipeline_owners[&key].tag,
                new_tag
            );
        }
        if order == RetirementOrder::AfterCoalescedCurrentStore {
            assert_eq!(stage, LifecycleWorkClass::Store);
            let published = fixture
                .transport
                .executor
                .published_lifecycle_store_retry_markers[&key]
                .publication_census_entry()
                .expect("the old Store retains exact publication identity");
            fixture
                .transport
                .executor
                .step(started, &mut current_services)
                .expect("the genuine BodyAvailable FIFO emits a current Store retry");
            let observed = fixture
                .transport
                .executor
                .last_runtime_step_observation_for_test()
                .unwrap();
            assert_eq!(observed.selected(), Some(RuntimeSelectedOwnerKind::Fifo));
            assert_eq!(
                observed.non_validate_class(),
                Some(RuntimeEffectClassV1::StoreBody)
            );
            assert_eq!(
                fixture
                    .transport
                    .executor
                    .published_lifecycle_store_retry_markers[&key]
                    .publication_census_entry(),
                Some(published)
            );
            assert!(fixture.transport.executor.retained_effect_batch.is_none());
            assert!(
                fixture
                    .transport
                    .executor
                    .pending_durable_validate_admissions
                    .is_empty()
            );
            assert!(
                current_services.store_tasks.is_empty(),
                "the published marker really coalesced current Store"
            );
        }

        let before = fixture.owner.body_owner_snapshot_for_test(fixture.ordinal);
        let bodies = fixture.transport.executor.durable_bodies.clone();
        let recovered = fixture.transport.executor.recovered_bodies.clone();
        let current_owners = fixture.transport.executor.body_pipeline_owners.clone();
        let retained_effects = format!("{:?}", fixture.transport.executor.retained_effect_batch);
        let mut expected_markers = fixture
            .transport
            .executor
            .published_lifecycle_store_retry_markers
            .clone();
        if let Some(failure) = failure {
            match failure {
                RetirementFailure::ClosedOutput => {
                    fixture
                        .transport
                        .executor
                        .output_guard
                        .activate_restart_required();
                }
                RetirementFailure::LedgerPublication => {
                    fixture
                        .owner
                        .fail_body_retirement_publication_for_test(fixture._owner_directory.path());
                }
            }
            let failed = fixture.owner.dispatch_completion_for_test(
                &mut fixture.services,
                &mut fixture.transport.executor,
                0,
            );
            match (failure, failed) {
                (
                    RetirementFailure::ClosedOutput,
                    Err(ProductionCompletionDispatchErrorV1::Service(reason)),
                ) => {
                    assert_eq!(reason, "obsolete body cancellation output is closed");
                }
                (
                    RetirementFailure::LedgerPublication,
                    Err(ProductionCompletionDispatchErrorV1::DispatchProjection),
                ) => {}
                (_, observed) => {
                    panic!("cancellation must fail at its exact publication boundary: {observed:?}")
                }
            }
            fixture
                .owner
                .assert_body_owner_retained_after_failure_for_test(
                    &before,
                    fixture._owner_directory.path(),
                    failure == RetirementFailure::LedgerPublication,
                );
            assert_eq!(fixture.transport.executor.durable_bodies, bodies);
            assert_eq!(fixture.transport.executor.recovered_bodies, recovered);
            assert_eq!(
                fixture.transport.executor.body_pipeline_owners,
                current_owners
            );
            assert_eq!(
                format!("{:?}", fixture.transport.executor.retained_effect_batch),
                retained_effects
            );
            assert_eq!(
                fixture
                    .transport
                    .executor
                    .published_lifecycle_store_retry_markers,
                expected_markers
            );
            assert!(fixture.transport.executor.output_guard.restart_required());
            assert!(fixture.transport.executor.status().fail_closed);
            fixture.planner_io.detach(&mut fixture.services);
            assert_reopened_body(&fixture.transport, fixture._owner_directory.path());
            return;
        }
        if stage == LifecycleWorkClass::Store {
            let removed = expected_markers
                .remove(&key)
                .expect("the old Store owns its exact retry marker");
            assert!(
                matches!(removed.effect, AdapterEffect::StoreBody { tag, .. } if tag == old_tag)
            );
        }

        let retry = fixture.owner.dispatch_completion_for_test(
            &mut fixture.services,
            &mut fixture.transport.executor,
            0,
        );
        assert_eq!(
            retry.expect("the obsolete authenticated carrier must cancel without restart"),
            ProductionCompletionDispatchV1::BodyStageCancelled {
                ordinal: fixture.ordinal,
                stage
            }
        );
        fixture
            .owner
            .assert_body_owner_cancelled_for_test(&before, fixture._owner_directory.path());
        assert_eq!(fixture.transport.executor.durable_bodies, bodies);
        assert_eq!(fixture.transport.executor.recovered_bodies, recovered);
        assert_eq!(
            fixture.transport.executor.body_pipeline_owners, current_owners,
            "retiring an old carrier must retain the exact current executor owner"
        );
        assert_eq!(
            format!("{:?}", fixture.transport.executor.retained_effect_batch),
            retained_effects,
            "cancellation preserves the exact owned current effect suffix"
        );
        assert_eq!(
            fixture
                .transport
                .executor
                .published_lifecycle_store_retry_markers,
            expected_markers
        );
        assert!(!fixture.transport.executor.output_guard.restart_required());
        assert_eq!(
            fixture.owner.classify_completion_ready_work(
                fixture
                    .transport
                    .executor
                    .lifecycle_reducer_fence_observation(),
            ),
            ProductionCompletionReadyWorkV1::None,
            "the ordinary rank classifier cannot select the cancelled owner a second time"
        );
        fixture
            .owner
            .assert_body_owner_cancelled_for_test(&before, fixture._owner_directory.path());

        if order == RetirementOrder::BeforeCurrentEffects {
            fixture
                .transport
                .executor
                .step(started, &mut current_services)
                .expect("drain the exact retained current effects after obsolete cancellation");
            assert!(fixture.transport.executor.retained_effect_batch.is_none());
        }
        if protect_body {
            let validate_ordinal = drive_current_body_to_validate(
                &mut fixture,
                &mut current_services,
                started,
                new_tag,
                order == RetirementOrder::AfterCoalescedCurrentStore,
            );
            if cold_recovery {
                assert_cancelled_body_cold_rehydration(fixture, validate_ordinal);
                return;
            }
        } else {
            assert!(!cold_recovery);
            assert!(
                fixture
                    .transport
                    .executor
                    .pending_durable_validate_admissions
                    .is_empty()
            );
            assert!(
                fixture
                    .transport
                    .executor
                    .body_pipeline_owners
                    .get(&key)
                    .is_none()
            );
        }
        assert_eq!(fixture.transport.executor.durable_bodies, bodies);
        assert_eq!(fixture.transport.executor.recovered_bodies, recovered);
        assert!(!fixture.transport.executor.status().fail_closed);
        assert!(current_services.closed.is_empty());
        fixture.planner_io.detach(&mut fixture.services);
        assert_reopened_body(&fixture.transport, fixture._owner_directory.path());
    }

    macro_rules! body_fence_test {
        ($name:ident, $stage:expr, $protected:expr, $order:expr) => {
            body_fence_test!($name, $stage, $protected, $order, None);
        };
        ($name:ident, $stage:expr, $protected:expr, $order:expr, $failure:expr) => {
            body_fence_test!($name, $stage, $protected, $order, $failure, false);
        };
        ($name:ident, $stage:expr, $protected:expr, $order:expr, $failure:expr, $recovery:expr) => {
            #[test]
            fn $name() {
                let result = crate::sumeragi::sumeragi_thread_builder(stringify!($name))
                    .spawn(|| {
                        retry_body_after_timeout($stage, $protected, $order, $failure, $recovery)
                    })
                    .expect("spawn on the production consensus stack")
                    .join();
                if let Err(payload) = result {
                    std::panic::resume_unwind(payload);
                }
            }
        };
    }
    body_fence_test!(
        certified_fetch_fence_timeout_retires_superseded_owner,
        LifecycleWorkClass::Fetch,
        false,
        RetirementOrder::BeforeCurrentEffects
    );
    body_fence_test!(
        certified_fetch_fence_timeout_preserves_protected_body,
        LifecycleWorkClass::Fetch,
        true,
        RetirementOrder::BeforeCurrentEffects
    );
    body_fence_test!(
        durable_store_fence_timeout_retires_superseded_owner,
        LifecycleWorkClass::Store,
        false,
        RetirementOrder::BeforeCurrentEffects
    );
    body_fence_test!(
        durable_store_fence_timeout_preserves_protected_body,
        LifecycleWorkClass::Store,
        true,
        RetirementOrder::BeforeCurrentEffects
    );
    body_fence_test!(
        certified_fetch_fence_timeout_retains_current_owner,
        LifecycleWorkClass::Fetch,
        true,
        RetirementOrder::AfterCurrentFetch
    );
    body_fence_test!(
        durable_store_fence_timeout_retains_current_owner,
        LifecycleWorkClass::Store,
        true,
        RetirementOrder::AfterCurrentFetch
    );
    body_fence_test!(
        durable_store_fence_timeout_replays_coalesced_current_store,
        LifecycleWorkClass::Store,
        true,
        RetirementOrder::AfterCoalescedCurrentStore
    );
    body_fence_test!(
        certified_fetch_fence_timeout_closed_output_preserves_owner,
        LifecycleWorkClass::Fetch,
        true,
        RetirementOrder::AfterCurrentFetch,
        Some(RetirementFailure::ClosedOutput)
    );
    body_fence_test!(
        durable_store_fence_timeout_closed_output_preserves_owner,
        LifecycleWorkClass::Store,
        true,
        RetirementOrder::AfterCurrentFetch,
        Some(RetirementFailure::ClosedOutput)
    );
    body_fence_test!(
        certified_fetch_fence_timeout_failed_publication_preserves_owner,
        LifecycleWorkClass::Fetch,
        true,
        RetirementOrder::AfterCurrentFetch,
        Some(RetirementFailure::LedgerPublication)
    );
    body_fence_test!(
        durable_store_fence_timeout_failed_publication_preserves_owner,
        LifecycleWorkClass::Store,
        true,
        RetirementOrder::AfterCoalescedCurrentStore,
        Some(RetirementFailure::LedgerPublication)
    );
    body_fence_test!(
        certified_fetch_fence_timeout_recovers_current_validate,
        LifecycleWorkClass::Fetch,
        true,
        RetirementOrder::AfterCurrentFetch,
        None,
        true
    );
    body_fence_test!(
        durable_store_fence_timeout_recovers_current_validate,
        LifecycleWorkClass::Store,
        true,
        RetirementOrder::AfterCoalescedCurrentStore,
        None,
        true
    );
    include!("v2_effects_resolved_validate_owner_cases.rs");
    include!("v2_effects_terminal_sign_cold_owner_cases.rs");
}
