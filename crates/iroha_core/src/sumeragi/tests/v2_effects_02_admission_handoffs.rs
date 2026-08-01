    #[test]
    fn restart_required_guard_stops_serialized_runtime_before_any_effect_work() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(Vec::new())));
        let queued_steps = executor.runtime.steps.len();
        executor.output_guard.activate_restart_required();

        assert!(matches!(
            executor.step(Instant::now(), &mut services),
            Err(EffectExecutorError::FailClosed(_))
        ));
        assert_eq!(
            executor.runtime.steps.len(),
            queued_steps,
            "post-latch runtime work must remain completely unconsumed"
        );
        assert!(services.statuses.is_empty());
        assert!(services.sign_tasks.is_empty());
        assert!(services.fetch_tasks.is_empty());
        assert!(services.store_tasks.is_empty());
        assert!(services.validation_tasks.is_empty());
        assert!(services.apply_tasks.is_empty());
    }

    #[test]
    fn failed_initial_local_store_admission_does_not_publish_pipeline_owner() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("store");

        assert!(
            executor
                .admit_local_proposal(
                    tag(0),
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.store_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_new_uncertified_fetch_admission_preserves_exact_projection() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.fetch_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_new_certified_fetch_admission_preserves_request_indexes() {
        let fixture = Fixture::new();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: None,
                        certified_sources: sources,
                        certificate: Some(prepare),
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.fetch_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_newer_certified_fetch_retransmission_preserves_exact_projection() {
        let fixture = Fixture::new();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        let effect = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: None,
            certified_sources: sources,
            certificate: Some(prepare),
        };
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(vec![effect.clone()], &mut services)
            .expect("admit initial certified fetch");
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(vec![effect], &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed by the exact service retry"
        );
    }

    #[test]
    fn failed_existing_fetch_certificate_upgrade_preserves_request_indexes() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("admit ordinary fetch");
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: sources,
                        certificate: Some(prepare),
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_staged_exact_body_runtime_admission_preserves_ready_owner() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive ready body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        executor.ready_bodies.insert(key, ready);
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn failed_staged_conflict_replacement_preserves_ready_bytes() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive staged body");
        let mut conflicting = fixture.manifest.clone();
        conflicting.payload_size_bytes = conflicting
            .payload_size_bytes
            .checked_add(1)
            .expect("small body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        executor.ready_bodies.insert(key, ready);
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        services.fail_on = Some("fetch");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(conflicting),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.fetch_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_retained_locked_body_runtime_admission_preserves_exact_projection() {
        let fixture = Fixture::new();
        let retained: Arc<[u8]> = fixture.body.clone().into();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.protected_lock = Some((fixture.manifest.round, fixture.manifest.subject));
        executor.retained_locked_body = Some((fixture.manifest.subject, retained));
        executor.ready_body_bytes =
            u64::try_from(fixture.body.len()).expect("retained body length");
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn late_fetch_conflict_does_not_fill_pipeline_owner_manifest() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive retained body");
        let mut conflicting = fixture.manifest.clone();
        conflicting.payload_size_bytes = conflicting
            .payload_size_bytes
            .checked_add(1)
            .expect("small body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        executor.ready_bodies.insert(key, ready);
        executor.body_pipeline_owners.insert(
            key,
            BodyPipelineOwner {
                tag: tag(0),
                manifest_hash: None,
            },
        );
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(conflicting),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
    }

    #[test]
    fn failed_detached_store_runtime_admission_preserves_exact_projection() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let id = executor.allocate_work_id().expect("allocate store work");
        let task = BodyStoreTask::for_test(
            id.get(),
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
        );
        executor.pending_store_bytes =
            u64::try_from(task.canonical_wire.len()).expect("body length");
        executor.pending_stores.insert(
            id,
            PendingStore {
                task,
                consumer: None,
            },
        );
        let mut services = fixture.services();
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn failed_recovered_body_runtime_admission_preserves_durable_catalogue() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let receipt = services
            .body_store
            .as_mut()
            .expect("body store")
            .store(fixture.manifest.clone(), fixture.body.clone())
            .expect("persist recovery body");
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), receipt));
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: key.0,
                        subject: key.1,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn successful_new_certified_fetch_commits_exact_task_and_request_once() {
        let fixture = Fixture::new();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: None,
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("admit certified fetch");
        let task = services.fetch_tasks.first().expect("fetch task");
        let request_hash = HashOf::new(task.certified_request().expect("certified request"));
        assert_eq!(executor.pending_fetches[&task.id()].task, *task);
        assert_eq!(
            executor.outstanding_requests.hashes(),
            BTreeSet::from([request_hash])
        );
        assert_eq!(
            executor.certified_work,
            BTreeMap::from([(request_hash, task.id())])
        );
    }

    #[test]
    fn successful_fetch_certificate_upgrade_commits_exact_delta_once() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("admit ordinary fetch");
        let id = services.fetch_tasks[0].id();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("upgrade fetch authority");
        let upgraded = services.fetch_tasks.last().expect("upgraded task");
        let request_hash = HashOf::new(upgraded.certified_request().expect("certified request"));
        assert_eq!(upgraded.id(), id);
        assert_eq!(executor.pending_fetches[&id].task, *upgraded);
        assert_eq!(
            executor.outstanding_requests.hashes(),
            BTreeSet::from([request_hash])
        );
        assert_eq!(
            executor.certified_work,
            BTreeMap::from([(request_hash, id)])
        );
    }

    #[test]
    fn successful_staged_conflict_retires_old_ready_only_after_fetch_admission() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let ready = ReadyBody::derive(&fixture.context, key.0, key.1, fixture.body.clone())
            .expect("derive staged body");
        let old_ready_bytes = u64::try_from(ready.bytes.len()).expect("body length");
        let mut incoming = fixture.manifest.clone();
        incoming.payload_size_bytes = incoming
            .payload_size_bytes
            .checked_add(1)
            .expect("small body");
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.ready_body_bytes = old_ready_bytes;
        executor.ready_bodies.insert(key, ready);
        let mut services = fixture.services();

        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                    manifest: Some(incoming.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("admit replacement fetch");
        assert!(!executor.ready_bodies.contains_key(&key));
        assert_eq!(executor.ready_body_bytes, 0);
        let task = services.fetch_tasks.first().expect("replacement fetch");
        assert_eq!(task.manifest(), Some(&incoming));
        assert_eq!(executor.pending_fetches[&task.id()].task, *task);
        assert_eq!(
            executor.body_pipeline_owners[&key].manifest_hash,
            Some(HashOf::new(&incoming))
        );
    }

    #[test]
    fn successful_ready_store_handoff_shares_exact_bytes_without_copy() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        let ready_bytes = Arc::clone(&executor.ready_bodies[&key].bytes);

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                }],
                &mut services,
            )
            .expect("admit store");
        let queued = services.store_tasks.first().expect("queued store");
        let pending = &executor.pending_stores[&queued.id()].task;
        assert!(Arc::ptr_eq(&ready_bytes, &queued.canonical_wire));
        assert!(Arc::ptr_eq(&ready_bytes, &pending.canonical_wire));
        assert_eq!(executor.ready_body_bytes, 0);
        assert_eq!(
            executor.pending_store_bytes,
            u64::try_from(ready_bytes.len()).expect("body length")
        );
    }

    #[test]
    fn runtime_wal_step_panic_latches_restart_required_before_callbacks() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        executor.runtime.panic_step = true;
        let output_guard = Arc::clone(&executor.output_guard);
        let mut services = fixture.services();

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = executor.step(Instant::now(), &mut services);
        }));

        assert!(unwind.is_err());
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
        assert!(services.statuses.is_empty());
        assert!(services.sign_tasks.is_empty());
        assert!(services.fetch_tasks.is_empty());
        assert!(services.store_tasks.is_empty());
    }

    #[test]
    fn failed_body_available_admission_preserves_exact_body_owners() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (work_id, ready) = executor
            .stage_body_fetch_for_test(&fixture)
            .expect("stage exact fetch");
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert_eq!(
            executor.finish_fetch(work_id, ready, &mut services),
            Err(EffectTransportError::Backpressure)
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
        assert!(executor.fatal_reason.is_none());
    }

    #[test]
    fn failed_store_admission_preserves_ready_owner_and_accounting() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        let before = executor.body_ownership_projection();
        services.fail_on = Some("store");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.store_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_body_stored_runtime_admission_preserves_pending_store() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("admit store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let completion = services.execute_store(store_id);
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .complete_body_store(completion, &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn failed_local_validation_admission_preserves_pending_store() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("admit local store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let completion = services.execute_store(store_id);
        let before = executor.body_ownership_projection();
        services.fail_on = Some("validation");

        assert!(
            executor
                .complete_body_store(completion, &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.validation_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_validation_admission_preserves_durable_owner() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("admit store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let completion = services.execute_store(store_id);
        executor
            .complete_body_store(completion, &mut services)
            .expect("record durable body");
        let before = executor.body_ownership_projection();
        services.fail_on = Some("validation");

        assert!(
            executor
                .consume_effects(
                    vec![AdapterEffect::ValidateBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    }],
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.validation_tasks.is_empty());
        assert!(
            services.fail_on.is_none(),
            "failure injection was not consumed"
        );
    }

    #[test]
    fn failed_validation_completion_admission_preserves_pending_validation() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("admit store");
        let store_id = services.store_tasks.last().expect("store task").id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("record durable body");
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("admit validation");
        let validation_id = services
            .validation_tasks
            .last()
            .expect("validation task")
            .id();
        let completion = services.execute_validation(validation_id);
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;

        assert!(
            executor
                .complete_body_validation(completion, &mut services)
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
    }

    #[test]
    fn successful_ready_store_validation_handoff_has_one_exact_owner() {
        let fixture = Fixture::new();
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        let ready_bytes = executor.ready_body_bytes;
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                }],
                &mut services,
            )
            .expect("admit store");
        assert!(!executor.ready_bodies.contains_key(&key));
        assert_eq!(executor.ready_body_bytes, 0);
        assert_eq!(executor.pending_stores.len(), 1);
        assert_eq!(executor.pending_store_bytes, ready_bytes);
        let store_id = services.store_tasks.last().expect("store task").id();
        assert_eq!(
            executor.pending_stores[&store_id].task,
            services.store_tasks[0]
        );

        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("complete store");
        assert!(executor.pending_stores.is_empty());
        assert_eq!(executor.pending_store_bytes, 0);
        assert!(executor.durable_bodies.contains_key(&key));
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyStored(completion_tag, round, subject, _))
                if *completion_tag == tag(0) && (*round, *subject) == key
        ));

        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: key.0,
                    subject: key.1,
                }],
                &mut services,
            )
            .expect("admit validation");
        assert_eq!(executor.pending_validations.len(), 1);
        let validation = services.validation_tasks.last().expect("validation task");
        assert_eq!(
            executor.pending_validations[&validation.id()].task,
            *validation
        );

        // Missing-sidecar completion is still a validation completion: it
        // cannot mutate deferred ownership or call recovery services before
        // the immutable consumer owner is checked. Build the state through
        // the production validation admission path, then corrupt only that
        // owner projection.
        for corruption in ["missing", "mismatched", "work-id", "orphan"] {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let (pending, reference, _) = pending_merge_validation(&fixture);
            let round = pending.task.round();
            let subject = pending.task.subject();
            let task = begin_reachable_merge_validation(
                &fixture,
                &mut executor,
                &mut services,
                round,
                subject,
            );
            let key = (round, subject);
            match corruption {
                "missing" => {
                    executor.body_pipeline_owners.remove(&key);
                }
                "mismatched" => {
                    executor
                        .body_pipeline_owners
                        .get_mut(&key)
                        .expect("reachable validation owner")
                        .tag = EventTag::new(1, round.view, Generation::new(8));
                }
                "work-id" => {
                    executor
                        .pending_validations
                        .get_mut(&task.id())
                        .expect("reachable pending validation")
                        .task
                        .id = EffectWorkId(999);
                }
                "orphan" => {
                    executor.durable_bodies.remove(&key);
                }
                _ => unreachable!("the test enumerates exact owner corruptions"),
            }
            let before = executor.body_ownership_projection();

            let error = executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id: task.id(),
                        reference,
                    },
                    &mut services,
                )
                .expect_err("corrupt validation owner must fail closed");
            assert!(matches!(
                error,
                EffectExecutorError::Contract(_) | EffectExecutorError::BodyStore(_)
            ));
            assert_eq!(executor.body_ownership_projection(), before);
            assert!(executor.deferred_merge_work.is_empty());
            assert!(services.deferred_merge_sidecars.is_empty());
        }
    }

    impl V2EffectExecutor<FakeRuntime> {
        fn stage_body_fetch_for_test(
            &mut self,
            fixture: &Fixture,
        ) -> Result<(EffectWorkId, ReadyBody), EffectExecutorError> {
            let id = self.allocate_work_id()?;
            self.bind_body_pipeline_owner(tag(0), &fixture.manifest)?;
            self.pending_fetches.insert(
                id,
                PendingFetch {
                    task: BodyFetchTask::for_test(
                        id.get(),
                        tag(0),
                        fixture.manifest.clone(),
                    ),
                    request_hash: None,
                },
            );
            let ready_body = ReadyBody::derive(
                &self.context,
                fixture.manifest.round,
                fixture.manifest.subject,
                fixture.body.clone(),
            )
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
            Ok((id, ready_body))
        }

        fn admit_ready_body_for_test(
            &mut self,
            fixture: &Fixture,
            services: &mut FakeServices,
        ) -> Result<(), EffectExecutorError> {
            let (id, ready_body) = self.stage_body_fetch_for_test(fixture)?;
            self.finish_fetch(id, ready_body, services)
                .map(|_| ())
                .map_err(|error| EffectExecutorError::Contract(error.to_string()))
        }
    }
