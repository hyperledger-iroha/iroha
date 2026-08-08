#[test]
fn global_lifecycle_minimum_blocks_later_fifo_until_its_completion_arrives() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );
    let older = runtime
        .mint_fresh_lifecycle_owner(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"older external exact request",
        )
        .expect("mint the older externally retained lifecycle");
    runtime
        .configure_external_lifecycle_owner_capacity(4)
        .expect("install the independent asynchronous bound");
    runtime
        .set_external_lifecycle_owners(vec![older.clone()])
        .expect("publish the older external owner");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(9),
    )
    .expect("enqueue later unrelated work");

    assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));
    let idle = runtime
        .take_last_scheduler_ownership()
        .expect("blocked scheduling still publishes exact Idle evidence");
    assert_eq!(idle.selected, RuntimeSelectedOwnerKind::Idle);
    assert!(!idle.fifo_ready);
    assert_eq!(runtime.queued_commands(), 1);

    let due = start + Duration::from_secs(10);
    assert!(matches!(runtime.step(due), Ok(RuntimeStep::Idle)));
    runtime
        .take_last_scheduler_ownership()
        .expect("blocked due clocks publish exact Idle evidence");
    assert!(runtime.timeout_owner.is_some());
    assert!(
        runtime.retransmit_owner.is_none(),
        "an absolute timeout suppresses replenishing the periodic owner until the timeout drains"
    );
    assert!(runtime.driver.timeouts.is_empty());
    assert!(runtime.driver.retransmits.is_empty());

    let older_effect = RuntimeEffectOwnership::fresh(
        older.clone(),
        RuntimeFreshRootKind::HistoricalLockedRetransmit,
    );
    runtime
        .enqueue_with_lifecycle_owner(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(1),
            &older_effect,
        )
        .expect("enqueue the exact older completion");
    assert!(matches!(
        runtime.step(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    let selected = runtime
        .take_last_scheduler_ownership()
        .expect("completion selection publishes exact ownership");
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = selected.candidate else {
        panic!("older completion must be the exact FIFO candidate");
    };
    assert_eq!(candidate.fifo_position, 1);
    assert_eq!(candidate.lifecycle_ordinal, older.lifecycle_ordinal());
    runtime
        .take_effect_ownership(1)
        .expect("test executor consumes the completion effect owner");
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 1)]);
    assert_eq!(runtime.queued_commands(), 1);

    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("the asynchronous owner retires after its exact completion handoff");
    runtime
        .step_and_take_scheduler_ownership_for_test(due)
        .expect("the older queued FIFO command now drains");
    assert_eq!(
        runtime.driver.delivered,
        vec![(owner_tag, 1), (owner_tag, 9)]
    );
    runtime
        .step_and_take_scheduler_ownership_for_test(due)
        .expect("the frozen timeout drains after all older lifecycles");
    assert_eq!(runtime.driver.timeouts, vec![owner_tag]);
    assert!(runtime.timeout_owner.is_none());
    runtime
        .step_and_take_scheduler_ownership_for_test(due)
        .expect("the later frozen retransmission drains next");
    assert_eq!(runtime.driver.retransmits, vec![owner_tag]);
    assert!(runtime.retransmit_owner.is_none());
}

#[test]
fn external_owner_bound_uses_effect_capacity_not_small_ingress_capacity() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );
    let pending_bound = 1_024usize;
    runtime
        .configure_external_lifecycle_owner_capacity(pending_bound)
        .expect("configure the executor's independent pending-work bound");
    let exact_capacity = pending_bound + MAX_EFFECTS_PER_STEP;
    let owners = (0..exact_capacity)
        .map(|ordinal| {
            let ordinal = u128::try_from(ordinal).expect("small test owner ordinal");
            let semantic = ordinal.to_le_bytes();
            RuntimeLifecycleOwner::new(
                RuntimeCandidateCausalOrigin::mint_fresh_root(
                    owner_tag,
                    CommandClass::Progress,
                    RuntimeFreshRootKind::HistoricalLockedRetransmit,
                    &semantic,
                ),
                ordinal,
            )
            .expect("synthetic external owner binds its first ordinal")
        })
        .collect::<Vec<_>>();
    runtime
        .set_external_lifecycle_owners(owners)
        .expect("1024 pending owners plus one retained batch fit despite ingress capacity 8");
    assert_eq!(runtime.external_lifecycle_owners.len(), exact_capacity);
    assert!(!runtime.fail_closed);
}

#[test]
fn restart_and_periodic_historical_retries_reuse_one_lifecycle_owner() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let historical = FakeEffect::historical(0xA5);
    let (mut runtime, startup) = SerializedV2Runtime::with_driver(
        FakeDriver::new(owner_tag),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        vec![historical],
    )
    .expect("construct deterministic restart ownership");
    assert_eq!(startup, vec![historical]);
    let startup_owner = runtime
        .take_effect_ownership(1)
        .expect("consume startup ownership")
        .pop()
        .expect("one startup owner");
    assert_eq!(
        startup_owner.causality(),
        RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::StartupRecovery)
    );
    runtime
        .arm_live_clocks(start)
        .expect("startup dispatch completes before clocks arm");
    runtime.driver.timer_effects.push_back(vec![historical]);
    runtime.driver.timer_effects.push_back(vec![historical]);

    let mut retry_owners = Vec::new();
    for elapsed in [2, 4] {
        let RuntimeStep::Advanced(effects) = runtime
            .step(start + Duration::from_secs(elapsed))
            .expect("periodic historical retry dispatches")
        else {
            panic!("periodic historical retry must advance");
        };
        assert_eq!(effects, vec![historical]);
        runtime
            .take_last_scheduler_ownership()
            .expect("periodic retry publishes scheduler ownership");
        retry_owners.push(
            runtime
                .take_effect_ownership(1)
                .expect("consume retry ownership")
                .pop()
                .expect("one retry owner"),
        );
    }
    assert!(retry_owners.iter().all(|ownership| {
        ownership.causality()
            == RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::HistoricalLockedRetransmit)
            && ownership.owner() == startup_owner.owner()
    }));
    let cache_after_owned_retries = runtime.dormant_fresh_lifecycle_owners.len();
    assert_ne!(cache_after_owned_retries, 0);
    for elapsed in [6, 8] {
        let RuntimeStep::Advanced(effects) = runtime
            .step(start + Duration::from_secs(elapsed))
            .expect("drained historical lifecycle still services its periodic clock")
        else {
            panic!("the periodic clock must advance even after exact work drains")
        };
        assert!(
            effects.is_empty(),
            "a drained exact historical request cannot recreate physical work"
        );
        runtime
            .take_last_scheduler_ownership()
            .expect("proofless periodic stutter retains scheduler ownership");
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime.dormant_fresh_lifecycle_owners.len(),
            cache_after_owned_retries,
            "fresh periodic episodes replace one bounded cache slot rather than growing it"
        );
    }
    assert_eq!(runtime.driver.retransmits, vec![owner_tag; 4]);

    let next_tag = tag(1);
    runtime
        .observe_effects_with_test_ownership(
            start + Duration::from_secs(9),
            &[FakeEffect::enter_view(next_tag)],
        )
        .expect("test EnterView retains positional producer ownership");
    assert!(
        runtime.dormant_fresh_lifecycle_owners.is_empty(),
        "certified view transition purges every prior-view dormant alias"
    );
}

#[test]
fn dormant_fresh_owner_cache_is_derived_bounded_and_purged_by_view() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let queue = RuntimeQueueConfig::new(8, 2, 2);
    let exact_capacity = queue.capacity + MAX_EFFECTS_PER_STEP;
    let mut runtime = runtime(FakeDriver::new(owner_tag), start, queue);
    let mut last_ordinal = None;
    for identity in 0..exact_capacity {
        let identity = u128::try_from(identity)
            .expect("small dormant-cache fixture")
            .to_le_bytes();
        let owner = runtime
            .mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                &identity,
            )
            .expect("derived dormant-cache capacity admits every configured owner");
        last_ordinal = Some(owner.lifecycle_ordinal());
    }
    assert_eq!(runtime.dormant_fresh_lifecycle_owners.len(), exact_capacity);
    assert_eq!(
        runtime.mint_fresh_lifecycle_owner(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"one owner beyond the derived bound",
        ),
        Err(EnqueueError::Full)
    );

    let next_tag = tag(1);
    runtime
        .observe_effects_with_test_ownership(start, &[FakeEffect::enter_view(next_tag)])
        .expect("test EnterView retains positional producer ownership");
    assert!(runtime.dormant_fresh_lifecycle_owners.is_empty());
    let successor = runtime
        .mint_fresh_lifecycle_owner(
            next_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"successor-view exact request",
        )
        .expect("view reclamation reopens the same derived cache geometry");
    assert!(
        successor.lifecycle_ordinal() > last_ordinal.expect("cache was filled"),
        "cache reclamation cannot reuse an old admission ordinal"
    );
}
