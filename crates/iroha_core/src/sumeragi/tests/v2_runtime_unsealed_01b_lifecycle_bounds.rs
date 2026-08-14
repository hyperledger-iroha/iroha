#[test]
fn selected_owner_without_a_runtime_minted_ordinal_fails_closed() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    runtime.ingress.commands.push_back(TaggedCommand::new(
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(1),
        start,
    ));
    assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
    assert!(runtime.fail_closed);
    assert!(runtime.last_scheduler_ownership().is_none());
}
#[test]
fn corrupt_cached_identity_and_rebound_origin_are_rejected_before_service() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
    let mut corrupt = TaggedCommand::new(
        owner_tag,
        CommandClass::Normal,
        FakeCommand::record(1),
        admitted_at,
    );
    corrupt.identity.canonical_hash = iroha_crypto::Hash::new(b"corrupt cached identity");
    assert_eq!(ingress.enqueue(corrupt), Err(EnqueueError::FailClosed));
    assert!(ingress.commands.is_empty());
    let root = FakeCommand::record(2);
    let mut origin =
        RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &root, None);
    assert!(origin.bind_lifecycle_ordinal(7));
    assert!(matches!(
        TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(3),
            admitted_at,
            origin,
            8,
        ),
        Err(EnqueueError::FailClosed)
    ));
}
#[test]
fn lifecycle_owner_constructor_rejects_a_conflicting_prebound_ordinal() {
    let owner_tag = tag(0);
    let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
        owner_tag,
        CommandClass::Progress,
        RuntimeFreshRootKind::HistoricalLockedRetransmit,
        b"prebound-owner",
    );
    assert!(origin.bind_lifecycle_ordinal(7));
    assert!(matches!(
        RuntimeLifecycleOwner::new(origin.clone(), 8),
        Err(EnqueueError::FailClosed)
    ));
    let exact = RuntimeLifecycleOwner::new(origin, 7)
        .expect("the already-bound exact ordinal remains admissible");
    assert!(exact.validate_exact());
    assert_eq!(exact.lifecycle_ordinal(), 7);
}
#[test]
fn runtime_physical_cut_is_monotone_and_regression_fails_closed() {
    let start = Instant::now();
    let owner_tag = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(owner_tag),
        start,
        RuntimeQueueConfig::new(6, 2, 1),
    );
    assert_eq!(runtime.ingress_physical_cut, 1);
    runtime
        .set_ingress_physical_cut(4)
        .expect("receiver high-watermark advances");
    runtime
        .set_ingress_physical_cut(4)
        .expect("publishing the same high-watermark is idempotent");
    assert_eq!(runtime.ingress_physical_cut, 4);
    assert!(runtime.set_ingress_physical_cut(3).is_err());
    assert!(runtime.fail_closed);
    assert_eq!(runtime.ingress_physical_cut, 4);
}
#[test]
fn deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences() {
    let directory = TempDir::new().expect("temporary physical-cut runtime directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    let message = signed_runtime_proposal(&context, &keys, 0x5A);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
        unreachable!("signed runtime proposal fixture carries Proposal")
    };
    let semantic_origin = context.roster
        [usize::try_from(proposal.proposer).expect("small fixture proposer")]
    .validator
    .clone();
    let (_owner_directory, _owner_ingress, mut ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), semantic_origin)],
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let pre_cut_fair = ownerships
        .pop()
        .expect("one productive leader-wire ownership carrier");
    let predecessor_ordinal = pre_cut_fair
        .runtime_lifecycle_ordinal()
        .expect("leader-wire carrier has an immutable logical ordinal");
    let target_cut = pre_cut_fair
        .runtime_physical_cut()
        .expect("checked dequeue freezes the target predecessor cut");
    assert!(
        u128::from(
            pre_cut_fair
                .physical_admission_ordinal()
                .expect("leader-wire carrier has a physical occurrence")
        ) < target_cut
    );
    let target_owner = runtime
        .mint_fresh_lifecycle_owner(
            runtime.round_tag(),
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"already-admitted deferred continuation",
        )
        .expect("mint target lifecycle after the leader-wire predecessor");
    assert!(predecessor_ordinal < target_owner.lifecycle_ordinal());
    let target = deferred_lifecycle_ownership_for_test(
        target_owner.clone(),
        7,
        RuntimeDispatchIngress::LocalOrCausal,
        None,
        target_cut,
    )
    .expect("freeze the target physical cut exactly once");
    assert!(matches!(
        deferred_lifecycle_ownership_for_test(
            target_owner.clone(),
            7,
            RuntimeDispatchIngress::LocalOrCausal,
            Some(u64::try_from(target_cut).expect("small target cut")),
            target_cut,
        ),
        Err(EnqueueError::FailClosed)
    ));
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(7, target.clone())
            .is_none()
    );
    let foreign_source = DeferredAdmissionOrdinalSource::new(7);
    let mut foreign_target = target.clone();
    foreign_target.runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
        &foreign_source,
        foreign_target.owner.causal_origin().lifecycle_key.clone(),
        foreign_target.owner.lifecycle_ordinal(),
        false,
        None,
        foreign_target.physical_cut,
    );
    assert!(
        foreign_target.validate_exact(),
        "the foreign capability can be internally self-consistent"
    );
    assert!(
        !foreign_target.validate_active_against_ingress(
            None,
            runtime.driver.deferred_admission_ordinal_source(),
        ),
        "a same-number capability minted by another source cannot own this runtime"
    );
    let make_command = |runtime: &SerializedV2Runtime<SumeragiV2Adapter>,
                        fair: FairV2IngressOwnershipEvidence| {
        let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, fair)
            .expect("project exact leader-wire ownership into runtime");
        let authenticated = runtime
            .driver
            .authenticate(message.clone())
            .expect("authenticate the exact leader-wire proposal");
        TaggedCommand::with_ingress_ownership(
            runtime.round_tag(),
            CommandClass::Normal,
            AdapterCommand::Authenticated(authenticated),
            Instant::now(),
            ownership,
        )
    };
    let pre_cut_command = make_command(&runtime, pre_cut_fair.clone());
    runtime
        .ingress
        .enqueue(pre_cut_command)
        .expect("enqueue the real pre-cut predecessor");
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("pre-cut minimum is exact"),
        Some(predecessor_ordinal),
        "a physical predecessor with an older logical identity still blocks"
    );
    runtime.ingress.commands.clear();
    let mut post_cut_fair = pre_cut_fair;
    let post_cut_ordinal = u64::try_from(target_cut).expect("small receiver-local physical cut");
    post_cut_fair.first.physical_admission_ordinal = post_cut_ordinal;
    post_cut_fair.latest.physical_admission_ordinal = post_cut_ordinal;
    post_cut_fair.runtime_physical_cut = target_cut.checked_add(1);
    assert!(
        post_cut_fair.validate_exact(),
        "the replay retains its exact logical identity at a fresh physical occurrence"
    );
    let periodic_replay_fair = post_cut_fair.clone();
    let post_cut_command = make_command(&runtime, post_cut_fair);
    runtime
        .ingress
        .enqueue(post_cut_command)
        .expect("enqueue the exact post-cut replay");
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("post-cut minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "a post-cut replay cannot resurrect its obsolete logical queue position"
    );
    let replay_owner = runtime
        .ingress
        .commands
        .front()
        .expect("post-cut replay remains physically queued")
        .lifecycle_owner()
        .expect("post-cut replay retains its old logical owner");
    let replay_ingress = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.clone())
        .expect("post-cut replay retains its exact ingress carrier");
    runtime.ingress.commands.clear();
    let causal_completion = TaggedCommand::with_causal_origin(
        runtime.round_tag(),
        CommandClass::Completion,
        AdapterCommand::ApplicationCompleted(proposal.subject),
        Instant::now(),
        replay_owner.causal_origin().clone(),
        replay_owner.lifecycle_ordinal(),
    )
    .expect("construct a local completion inheriting the replay root");
    runtime
        .ingress
        .enqueue(causal_completion)
        .expect("enqueue the post-cut causal completion");
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("post-cut causal FIFO minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "dropping the current envelope cannot drop the causal root's physical position"
    );
    runtime.ingress.commands.clear();
    runtime.pending_effect_ownership = Some(vec![RuntimeEffectOwnership::inherited(
        replay_owner.clone(),
    )]);
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("post-cut effect minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "post-cut effect and external work cannot reclaim the root's old logical rank"
    );
    runtime.pending_effect_ownership = None;
    let replay = deferred_lifecycle_ownership_for_test(
        replay_owner.clone(),
        8,
        RuntimeDispatchIngress::DirectAuthenticated,
        Some(post_cut_ordinal),
        target_cut
            .checked_add(1)
            .expect("small target cut has a successor"),
    )
    .expect("post-cut replay can cross into a distinct Busy-deferred owner");
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(8, replay)
            .is_none()
    );
    assert!(
        runtime
            .deferred_ingress_ownership
            .insert(8, replay_ingress)
            .is_none()
    );
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal_for_deferred(&target)
            .expect("deferred post-cut minimum is exact"),
        Some(target_owner.lifecycle_ordinal()),
        "crossing Busy cannot turn the post-cut replay into a predecessor"
    );
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("pairwise deferred cut relation is exact"),
        BTreeSet::from([7]),
        "the earlier target remains the sole runner-eligible continuation"
    );
    // Retire the earlier deferred target, leaving only the replay whose
    // physical occurrence began at that target's old cut. Its inherited
    // logical ordinal is older than the timeout which is frozen next, but
    // the new physical occurrence is not: the timeout cut must win.
    assert!(runtime.deferred_lifecycle_ownership.remove(&7).is_some());
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("the replay is otherwise the logical minimum"),
        BTreeSet::from([8])
    );
    runtime
        .set_ingress_physical_cut(target_cut)
        .expect("publish the timeout's receiver-local cut");
    let clock_start = Instant::now();
    runtime
        .arm_live_clocks(clock_start)
        .expect("arm timeout for the post-cut replay regression");
    let timeout_owner = runtime
        .frozen_timeout_owner_for_test(clock_start + runtime.base_round_timeout)
        .expect("freeze one exact timeout owner");
    assert!(replay_owner.lifecycle_ordinal() < timeout_owner.lifecycle_ordinal());
    assert_eq!(runtime.timeout_owner_physical_cut, Some(target_cut));
    assert!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("the timeout cut rejects obsolete logical resurrection")
            .is_empty(),
        "a post-cut replay cannot overtake the already-admitted timeout"
    );
    let frozen_timeout_owner = runtime
        .timeout_owner
        .clone()
        .expect("the timeout owner remains frozen until transfer");
    runtime.timeout_owner_physical_cut = None;
    assert!(matches!(
        runtime.eligible_deferred_admission_ordinals(),
        Err(EnqueueError::FailClosed)
    ));
    runtime.timeout_owner_physical_cut = Some(target_cut);
    runtime.timeout_owner = None;
    assert!(matches!(
        runtime.eligible_deferred_admission_ordinals(),
        Err(EnqueueError::FailClosed)
    ));
    runtime.timeout_owner = Some(frozen_timeout_owner);
    runtime
        .set_ingress_physical_cut(
            target_cut
                .checked_add(1)
                .expect("small timeout cut has a successor"),
        )
        .expect("later ingress advances only the live high-watermark");
    assert_eq!(
        runtime.timeout_owner_physical_cut,
        Some(target_cut),
        "later ingress cannot refresh the frozen timeout cut"
    );
    let arbitration = runtime
        .scheduler_arbitration_inputs(clock_start + runtime.base_round_timeout)
        .expect("the frozen timeout compares against its original physical cut");
    assert!(
        arbitration.timeout_due,
        "post-cut deferred replay cannot suppress an already-admitted timeout"
    );
    runtime.timeout_owner = None;
    runtime.timeout_owner_physical_cut = None;
    // Retire the old physical occurrence, freeze a periodic owner at the
    // advanced receiver cut, then admit another physical replay which
    // retains the same obsolete logical lifecycle. The periodic selector
    // must compare only with its immutable pre-cut prefix.
    assert!(runtime.deferred_lifecycle_ownership.remove(&8).is_some());
    assert!(runtime.deferred_ingress_ownership.remove(&8).is_some());
    runtime.timeout_emitted = true;
    runtime.retransmit_started_at = clock_start;
    let periodic_due_at = clock_start + runtime.retransmit_interval;
    runtime
        .freeze_due_clock_owners(periodic_due_at)
        .expect("freeze one exact periodic lifecycle and physical cut");
    let frozen_periodic_owner = runtime
        .retransmit_owner
        .clone()
        .expect("the due periodic episode owns one lifecycle position");
    let periodic_cut = runtime
        .retransmit_owner_physical_cut
        .expect("the due periodic episode freezes receiver ingress");
    assert_eq!(periodic_cut, runtime.ingress_physical_cut);
    let mut later_replay_fair = periodic_replay_fair;
    let later_physical_ordinal = u64::try_from(periodic_cut).expect("small periodic cut fits u64");
    later_replay_fair.first.physical_admission_ordinal = later_physical_ordinal;
    later_replay_fair.latest.physical_admission_ordinal = later_physical_ordinal;
    later_replay_fair.runtime_physical_cut = periodic_cut.checked_add(1);
    assert!(later_replay_fair.validate_exact());
    let later_replay_command = make_command(&runtime, later_replay_fair.clone());
    let later_replay_owner = later_replay_command
        .lifecycle_owner()
        .expect("later replay retains its old logical lifecycle");
    assert!(
        later_replay_owner.lifecycle_ordinal() < frozen_periodic_owner.lifecycle_ordinal(),
        "the regression requires physically later but logically older replay"
    );
    runtime
        .set_ingress_physical_cut(
            periodic_cut
                .checked_add(1)
                .expect("small periodic cut has a successor"),
        )
        .expect("publish the later physical admission without refreshing the clock cut");
    let mut pre_runtime_replay = later_replay_fair.clone();
    pre_runtime_replay.runtime_physical_cut = None;
    pre_runtime_replay.leader_wire_runtime_receipt = None;
    assert!(pre_runtime_replay.validate_exact());
    assert!(
        !runtime.can_admit_network_message_with_ingress_ownership(&message, &pre_runtime_replay,),
        "checked dequeue must retain a post-cut productive replay behind the periodic owner",
    );
    assert!(!runtime.fail_closed);
    let queue_len_before_replay = runtime.ingress.commands.len();
    assert_eq!(
        runtime.enqueue_after_clock_reservation(later_replay_command),
        Err(EnqueueError::Full),
        "the physically later replay remains on its existing ingress carrier"
    );
    assert_eq!(
        runtime.ingress.commands.len(),
        queue_len_before_replay,
        "backpressure cannot publish a FIFO position ahead of the periodic owner"
    );
    assert_eq!(runtime.retransmit_owner_physical_cut, Some(periodic_cut));
    let arbitration = runtime
        .scheduler_arbitration_inputs(periodic_due_at)
        .expect("periodic arbitration uses the frozen physical prefix");
    assert!(
        arbitration.periodic_timer_due,
        "post-cut replay cannot suppress an already-admitted periodic episode"
    );
    let (selected, _) = ScheduleState { fifo_owed: true }.select(
        arbitration.timeout_due,
        arbitration.periodic_timer_due,
        arbitration.fifo_ready,
    );
    assert_eq!(
        selected,
        ScheduledWork::PeriodicTimer,
        "a later replay cannot inherit stale FIFO debt ahead of the frozen target"
    );
    runtime.retransmit_owner_physical_cut = None;
    assert!(matches!(
        runtime.scheduler_arbitration_inputs(periodic_due_at),
        Err(EnqueueError::FailClosed)
    ));
    runtime.retransmit_owner_physical_cut = Some(periodic_cut);
    runtime.retransmit_owner = None;
    assert!(matches!(
        runtime.eligible_deferred_admission_ordinals(),
        Err(EnqueueError::FailClosed)
    ));
    runtime.retransmit_owner = Some(frozen_periodic_owner);
    runtime.retransmit_owner = None;
    runtime.retransmit_owner_physical_cut = None;
    assert!(
        runtime.can_admit_network_message_with_ingress_ownership(&message, &pre_runtime_replay,),
        "the retained productive replay becomes admissible after clock transfer",
    );
    let later_replay_command = make_command(&runtime, later_replay_fair);
    runtime
        .enqueue_after_clock_reservation(later_replay_command)
        .expect("the same retained replay becomes admissible after target transfer");
    // Pairwise target-relative precedence can form a cycle even though
    // every source/cut pair is individually exact: B logically precedes
    // A, C logically precedes B, and A physically precedes C.  The global
    // selector must first exclude C as post-A-cut, then choose B by
    // logical rank.  Retiring each selected owner yields B, A, C without
    // a lasso or an empty eligible set.
    runtime.ingress.commands.clear();
    runtime.deferred_ingress_ownership.clear();
    runtime.deferred_lifecycle_ownership.clear();
    let (a, b, c) = {
        let source = runtime.driver.deferred_admission_ordinal_source();
        let make_owner = |semantic_identity: &[u8],
                          source_physical_ordinal: Option<u64>,
                          physical_cut: u128,
                          lifecycle_ordinal: u128| {
            let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
                runtime.round_tag(),
                CommandClass::Progress,
                RuntimeFreshRootKind::StartupRecovery,
                semantic_identity,
            );
            if let Some(source_physical_ordinal) = source_physical_ordinal {
                origin.root_ingress_identity = Some(Hash::new(semantic_identity));
                origin.root_ingress_physical_ownership = Some(RuntimeIngressPhysicalOwnership {
                    source_ordinal: source_physical_ordinal,
                    physical_cut,
                });
                origin.lifecycle_key = runtime_candidate_causal_origin_lifecycle_key(&origin);
            }
            let owner = RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
                .expect("cycle fixture owns an exact logical lifecycle");
            let runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
                source,
                owner.causal_origin().lifecycle_key.clone(),
                owner.lifecycle_ordinal(),
                false,
                source_physical_ordinal,
                physical_cut,
            );
            let admission_ordinal = runtime_seal.admission_ordinal();
            let ownership = RuntimeDeferredLifecycleOwnership::new(
                owner,
                admission_ordinal,
                RuntimeDispatchIngress::LocalOrCausal,
                source_physical_ordinal,
                physical_cut,
                runtime_seal,
            )
            .expect("cycle fixture retains an exact source-bound runtime seal");
            assert!(ownership.validate_active_against_ingress(None, source));
            (admission_ordinal, ownership)
        };
        (
            make_owner(b"cycle-a", None, 5, 3),
            make_owner(b"cycle-b", Some(4), 9, 2),
            make_owner(b"cycle-c", Some(8), 12, 1),
        )
    };
    for (ordinal, ownership) in [a.clone(), b.clone(), c.clone()] {
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(ordinal, ownership)
                .is_none()
        );
    }
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("two-stage selector breaks the physical/logical cycle"),
        BTreeSet::from([b.0])
    );
    assert!(runtime.deferred_lifecycle_ownership.remove(&b.0).is_some());
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("A becomes eligible after B retires"),
        BTreeSet::from([a.0])
    );
    assert!(runtime.deferred_lifecycle_ownership.remove(&a.0).is_some());
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("C becomes eligible only after its physical predecessor retires"),
        BTreeSet::from([c.0])
    );
}
#[test]
fn passive_external_owner_cannot_fence_fifo_or_absolute_timeout() {
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
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.delivered, vec![(owner_tag, 9)]);
    assert_eq!(runtime.queued_commands(), 0);
    let due = start + Duration::from_secs(10);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert!(runtime.timeout_owner.is_none());
    assert!(
        runtime.retransmit_owner.is_none(),
        "an absolute timeout suppresses replenishing the periodic owner during its turn"
    );
    assert_eq!(runtime.driver.timeouts, vec![owner_tag]);
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
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert_eq!(runtime.driver.retransmits, vec![owner_tag]);
    assert_eq!(runtime.queued_commands(), 1);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(
        runtime.driver.delivered,
        vec![(owner_tag, 9), (owner_tag, 1)]
    );
    assert_eq!(runtime.queued_commands(), 0);
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("the asynchronous owner retires after its exact completion handoff");
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
    let exact_capacity = pending_bound + 2 * MAX_EFFECTS_PER_STEP;
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
        .expect("pending owners plus two retained batches fit despite ingress capacity 8");
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
#[test]
fn causal_successors_inherit_root_and_lifecycle_ordinal() {
    let admitted_at = Instant::now();
    let root_tag = tag(0);
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 2, 2));
    ingress
        .enqueue(TaggedCommand::new(
            root_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            admitted_at,
        ))
        .expect("root candidate is admitted");
    let (root, root_owner) = ingress
        .pop_next_with_ownership()
        .expect("root selection is exact")
        .expect("root candidate is ready");
    assert_eq!(root.lifecycle_ordinal, Some(root_owner.lifecycle_ordinal));
    let successor_tag = EventTag::new(
        root_tag.height(),
        root_tag.view() + 1,
        Generation::new(root_tag.generation().get() + 1),
    );
    for value in [2, 3, 4] {
        ingress
            .enqueue(
                TaggedCommand::with_causal_origin(
                    successor_tag,
                    CommandClass::Completion,
                    FakeCommand::record(value),
                    admitted_at,
                    root_owner.causal_origin.clone(),
                    root_owner.lifecycle_ordinal,
                )
                .expect("causal owner is internally consistent"),
            )
            .expect("causal child is admitted with a unique physical owner");
    }
    let physical_ordinals = ingress
        .commands
        .iter()
        .map(|candidate| {
            assert_eq!(
                candidate.causal_origin, root_owner.causal_origin,
                "evidence/view rewriting cannot replace the first-admission root"
            );
            assert_eq!(
                candidate.lifecycle_ordinal,
                Some(root_owner.lifecycle_ordinal),
                "every child inherits one logical lifecycle ordinal"
            );
            candidate
                .admission_ordinal
                .expect("every physical child has its own FIFO ordinal")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(physical_ordinals.len(), 3);
    let unrelated = TaggedCommand::new(
        successor_tag,
        CommandClass::Completion,
        FakeCommand::record(2),
        admitted_at,
    );
    assert!(
        !unrelated
            .causal_origin
            .same_lifecycle(&root_owner.causal_origin),
        "a physically similar command with a different causal root cannot coalesce"
    );
}
#[test]
fn equal_lifecycle_fence_siblings_follow_exact_physical_rank() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"fence-sibling-block")),
        payload_hash: Hash::new(b"fence-sibling-payload"),
    };
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 2, 0));
    let lifecycle_ordinal = ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve one shared fence lifecycle");
    let predecessor = AdapterCommand::ApplicationCompleted(subject);
    let mut causal_origin =
        RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &predecessor, None);
    assert!(causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal));
    let sibling = |class, command| {
        TaggedCommand::with_causal_origin(
            owner_tag,
            class,
            command,
            admitted_at,
            causal_origin.clone(),
            lifecycle_ordinal,
        )
        .expect("construct one exact same-lifecycle fence sibling")
    };
    ingress
        .enqueue(sibling(CommandClass::Normal, predecessor))
        .expect("enqueue the physical predecessor");
    ingress
        .enqueue(sibling(
            CommandClass::Completion,
            AdapterCommand::SignatureCompleted(vec![0xA5]),
        ))
        .expect("enqueue the later causal completion");
    let (first, first_owner, first_is_completion) = ingress
        .pop_fence_dependency_with_ownership(
            lifecycle_ordinal,
            u128::MAX,
            |queued| matches!(queued.command, AdapterCommand::SignatureCompleted(_)),
            |_| true,
        )
        .expect("the exact dependency rank is valid")
        .expect("one equal-lifecycle owner is ready");
    assert!(matches!(
        first.command,
        AdapterCommand::ApplicationCompleted(_)
    ));
    assert!(!first_is_completion);
    assert_eq!(first_owner.fifo_position, 0);
    let (second, second_owner, second_is_completion) = ingress
        .pop_fence_dependency_with_ownership(
            lifecycle_ordinal,
            u128::MAX,
            |queued| matches!(queued.command, AdapterCommand::SignatureCompleted(_)),
            |_| true,
        )
        .expect("the remaining completion rank is valid")
        .expect("the equal-lifecycle completion is ready after its predecessor");
    assert!(matches!(
        second.command,
        AdapterCommand::SignatureCompleted(_)
    ));
    assert!(second_is_completion);
    assert_eq!(second_owner.fifo_position, 0);
    assert!(ingress.commands.is_empty());
}
#[test]
fn preassigned_batch_lifecycles_require_shared_mint_and_exact_root() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let unminted_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let mut unminted_ingress = BoundedIngress::with_lifecycle_ordinals(
        RuntimeQueueConfig::new(4, 1, 1),
        unminted_source.clone(),
    );
    let unminted_command = FakeCommand::record(1);
    let mut unminted_origin = RuntimeCandidateCausalOrigin::mint(
        owner_tag,
        CommandClass::Completion,
        &unminted_command,
        None,
    );
    assert!(unminted_origin.bind_lifecycle_ordinal(1));
    let unminted = TaggedCommand::with_causal_origin(
        owner_tag,
        CommandClass::Completion,
        unminted_command,
        admitted_at,
        unminted_origin,
        1,
    )
    .expect("construct internally exact but unminted lifecycle");
    assert_eq!(
        unminted_ingress.enqueue_completion_batch(vec![unminted]),
        Err(EnqueueError::FailClosed)
    );
    assert!(unminted_ingress.commands.is_empty());
    assert_eq!(
        unminted_source
            .next_ordinal_for_test()
            .expect("unminted batch rejection preserves the source"),
        Some(1)
    );
    let collision_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
    let mut collision_ingress = BoundedIngress::with_lifecycle_ordinals(
        RuntimeQueueConfig::new(4, 1, 1),
        collision_source.clone(),
    );
    collision_ingress
        .enqueue(TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(2),
            admitted_at,
        ))
        .expect("mint one exact lifecycle root");
    let (_, root_owner) = collision_ingress
        .pop_next_with_ownership()
        .expect("select the minted root exactly")
        .expect("root is ready");
    let sibling = TaggedCommand::with_causal_origin(
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(3),
        admitted_at,
        root_owner.causal_origin.clone(),
        root_owner.lifecycle_ordinal,
    )
    .expect("construct one legitimate causal sibling");
    let conflicting_command = FakeCommand::record(4);
    let mut conflicting_origin = RuntimeCandidateCausalOrigin::mint(
        owner_tag,
        CommandClass::Completion,
        &conflicting_command,
        None,
    );
    assert!(conflicting_origin.bind_lifecycle_ordinal(root_owner.lifecycle_ordinal));
    let conflicting = TaggedCommand::with_causal_origin(
        owner_tag,
        CommandClass::Completion,
        conflicting_command,
        admitted_at,
        conflicting_origin,
        root_owner.lifecycle_ordinal,
    )
    .expect("construct a distinct root at the colliding ordinal");
    let next_before_collision = collision_source
        .next_ordinal_for_test()
        .expect("inspect source before batch collision");
    assert_eq!(
        collision_ingress.enqueue_completion_batch(vec![sibling, conflicting]),
        Err(EnqueueError::FailClosed)
    );
    assert!(
        collision_ingress.commands.is_empty(),
        "batch collision must reject atomically"
    );
    assert_eq!(
        collision_source
            .next_ordinal_for_test()
            .expect("batch collision preserves the source"),
        next_before_collision,
        "collision validation must run before reserving physical positions"
    );
}
#[test]
fn restart_dormant_local_fifo_reservation_survives_full_class_churn() {
    let started_at = Instant::now();
    let owner_tag = tag(0);
    let lifecycle_key = Hash::new(b"restart dormant Local FIFO lifecycle");
    let mut driver = FakeDriver::new(owner_tag);
    driver.dormant_local_fifo_reservations = vec![RuntimeDormantLocalFifoReservation::completion(
        lifecycle_key,
        1,
        8,
    )];
    let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
    let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
        driver,
        started_at,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(7, 1, 2),
        Vec::new(),
        lifecycle_ordinals,
    )
    .expect("restart installs exact latent FIFO ownership")
    .0;
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime without advancing its latent owner");
    assert_eq!(
        runtime.remaining_completion_capacity(),
        5,
        "the dormant Local stage consumes one physical completion slot"
    );
    let later_serve = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("mint a later exact Serve ticket");
    assert_eq!(
        runtime.minimum_active_lifecycle_ordinal(),
        Ok(Some(1)),
        "the complete active inventory retains restart-dormant lifecycle debt"
    );
    assert!(
        !runtime
            .older_lifecycle_predates_exact_serve(started_at, later_serve)
            .expect("inspect passive dormant ownership at the Serve cut"),
        "passive dormant debt cannot open an executable Serve predecessor episode"
    );
    for value in [1, 2, 3] {
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(value),
        )
        .expect("ordinary churn fills only the remaining normal prefix");
    }
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(4),
        ),
        Err(EnqueueError::ReservedCapacity),
        "normal churn cannot acquire the dormant target's slot"
    );
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Progress,
        FakeCommand::record(5),
    )
    .expect("progress fills its existing prefix");
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(6),
    )
    .expect("a trusted completion fills the last unreserved position");
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert!(
        runtime.driver.delivered.is_empty(),
        "the full-capacity cut is retained before exact replacement"
    );
    runtime.driver.admission_preflight_override =
        Some(RuntimeCommandAdmissionPreflight::ReuseDormant {
            causal_lifecycle_key: lifecycle_key,
            admission_ordinal: 1,
            producer_stage: 8,
        });
    let next_before_replay = runtime.ingress.next_admission_ordinal;
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(9),
    )
    .expect("exact retry atomically replaces its latent slot at full capacity");
    assert!(runtime.ingress.dormant_local_fifo_reservations.is_empty());
    assert_eq!(runtime.queued_commands(), 6);
    assert_eq!(runtime.remaining_completion_capacity(), 0);
    assert_eq!(
        runtime.minimum_active_lifecycle_ordinal(),
        Ok(Some(1)),
        "the restored FIFO owner retains the pre-restart lifecycle age"
    );
    let next_after_replay = runtime.ingress.next_admission_ordinal;
    assert_ne!(
        next_after_replay, next_before_replay,
        "the first physical replay receives one fresh FIFO position"
    );
    enqueue_fake(
        &mut runtime,
        owner_tag,
        CommandClass::Completion,
        FakeCommand::record(9),
    )
    .expect("duplicate exact retry coalesces with the one physical owner");
    assert_eq!(runtime.queued_commands(), 6);
    assert_eq!(
        runtime.ingress.next_admission_ordinal, next_after_replay,
        "coalescing cannot mint another physical admission ordinal"
    );
    let RuntimeStep::Advanced(effects) = runtime
        .step(started_at)
        .expect("the exact replacement becomes the global ready owner")
    else {
        panic!("the exact replacement must dispatch before younger queued work");
    };
    assert_eq!(effects, vec![FakeEffect::other()]);
    let selected = runtime
        .take_last_scheduler_ownership()
        .expect("the replacement dispatch retains exact FIFO ownership");
    runtime
        .take_effect_ownership(effects.len())
        .expect("the executor consumes the restored target's effect owner");
    assert_eq!(selected.selected, RuntimeSelectedOwnerKind::Fifo);
    assert_eq!(
        runtime.driver.delivered,
        vec![(owner_tag, 9)],
        "the restored target dispatches before every younger physical command"
    );
    assert_eq!(runtime.queued_commands(), 5);
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(9),
        ),
        Err(EnqueueError::FailClosed),
        "ReuseDormant after latent-slot removal cannot recreate the drained stage"
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.queued_commands(),
        5,
        "rejected resurrection cannot install another physical owner"
    );
}
#[test]
fn restart_dormant_completion_batch_atomically_replaces_latent_slots() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let first_key = Hash::new(b"first dormant validation lifecycle");
    let second_key = Hash::new(b"second dormant validation lifecycle");
    let mut ingress = BoundedIngress::with_lifecycle_ordinals(
        RuntimeQueueConfig::new(5, 1, 1),
        RuntimeLifecycleOrdinalSource::after_high_watermark(2),
    );
    ingress
        .install_dormant_local_fifo_reservations(vec![
            RuntimeDormantLocalFifoReservation::completion(first_key, 1, 9),
            RuntimeDormantLocalFifoReservation::completion(second_key, 2, 9),
        ])
        .expect("restart installs two exact completion reservations");
    for value in [1, 2] {
        ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(value),
                admitted_at,
            ))
            .expect("ordinary completions fill the unreserved positions");
    }
    assert_eq!(ingress.remaining_capacity(), 0);
    let batch = vec![
        restored_fake_command(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(3),
            first_key,
            1,
            9,
        ),
        restored_fake_command(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(4),
            second_key,
            2,
            9,
        ),
    ];
    ingress
        .enqueue_completion_batch(batch.clone())
        .expect("one atomic batch replaces both latent reservations");
    assert!(ingress.dormant_local_fifo_reservations.is_empty());
    assert_eq!(ingress.len(), 4);
    let next_after_first_batch = ingress.next_admission_ordinal;
    ingress
        .enqueue_completion_batch(batch)
        .expect("repeated exact batch coalesces with physical owners");
    assert_eq!(ingress.len(), 4);
    assert_eq!(
        ingress.next_admission_ordinal, next_after_first_batch,
        "duplicate batch cannot allocate another physical range"
    );
}
#[test]
fn dormant_local_fifo_metadata_rejects_wrong_stage_ordinal_and_capacity() {
    let owner_tag = tag(0);
    let lifecycle_key = Hash::new(b"immutable dormant completion lifecycle");
    let new_ingress = || {
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(5, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(2),
        );
        ingress
            .install_dormant_local_fifo_reservations(vec![
                RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8),
            ])
            .expect("install exact dormant metadata");
        ingress
    };
    let mut wrong_stage = new_ingress();
    assert_eq!(
        wrong_stage.enqueue(restored_fake_command(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(1),
            lifecycle_key,
            1,
            9,
        )),
        Err(EnqueueError::FailClosed),
        "a retry cannot change its persisted reducer stage"
    );
    assert_eq!(wrong_stage.remaining_capacity(), 3);
    let mut wrong_ordinal = new_ingress();
    assert_eq!(
        wrong_ordinal.enqueue(restored_fake_command(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(1),
            lifecycle_key,
            2,
            8,
        )),
        Err(EnqueueError::FailClosed),
        "a retry cannot change its immutable lifecycle ordinal"
    );
    assert_eq!(wrong_ordinal.remaining_capacity(), 3);
    let mut over_capacity = BoundedIngress::<FakeCommand>::with_lifecycle_ordinals(
        RuntimeQueueConfig::new(5, 1, 1),
        RuntimeLifecycleOrdinalSource::after_high_watermark(5),
    );
    let forged = (1_u128..=5)
        .map(|ordinal| {
            RuntimeDormantLocalFifoReservation::completion(
                Hash::new(ordinal.to_le_bytes()),
                ordinal,
                8,
            )
        })
        .collect();
    assert_eq!(
        over_capacity.install_dormant_local_fifo_reservations(forged),
        Err(EnqueueError::FailClosed),
        "an over-capacity snapshot must fail before live admission"
    );
    assert!(over_capacity.dormant_local_fifo_reservations.is_empty());
    for producer_stage in 0_u8..=u8::MAX {
        if RuntimeDormantLocalFifoReservation::is_local_fifo_stage(producer_stage) {
            continue;
        }
        let mut malformed = BoundedIngress::<FakeCommand>::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(1),
        );
        assert_eq!(
            malformed.install_dormant_local_fifo_reservations(vec![
                RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, producer_stage,),
            ]),
            Err(EnqueueError::FailClosed),
            "nonlocal or unknown stage {producer_stage} cannot forge reserved FIFO capacity"
        );
        assert!(malformed.dormant_local_fifo_reservations.is_empty());
    }
}
#[test]
fn restored_exact_stage_coalesces_at_full_capacity_without_aliasing_successors() {
    let admitted_at = Instant::now();
    let owner_tag = tag(0);
    let lifecycle_key = Hash::new(b"persisted producer lifecycle");
    let mut ingress = BoundedIngress::with_lifecycle_ordinals(
        RuntimeQueueConfig::new(5, 1, 1),
        RuntimeLifecycleOrdinalSource::after_high_watermark(1),
    );
    let restored_with_ordinal = |value, producer_stage, tag, class, lifecycle_ordinal| {
        let command = FakeCommand::record(value);
        let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            tag,
            class,
            &command,
            None,
            lifecycle_key,
            lifecycle_ordinal,
        )
        .expect("validated dormant metadata reconstructs one exact owner");
        let mut tagged = TaggedCommand::with_causal_origin(
            tag,
            class,
            command,
            admitted_at,
            owner.causal_origin().clone(),
            owner.lifecycle_ordinal(),
        )
        .expect("restored command binds its persisted ordinal");
        tagged.restored_producer_stage = Some(producer_stage);
        tagged
    };
    let restored_with = |value, producer_stage, tag, class| {
        restored_with_ordinal(value, producer_stage, tag, class, 1)
    };
    let restored = |value, producer_stage| {
        restored_with(value, producer_stage, owner_tag, CommandClass::Completion)
    };
    ingress
        .install_dormant_local_fifo_reservations(vec![
            RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8),
            RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 9),
        ])
        .expect("restart installs both latent Local FIFO reservations");
    ingress
        .enqueue(restored(1, 8))
        .expect("first restored stage owns one physical position");
    ingress
        .enqueue(restored(2, 9))
        .expect("a distinct causal successor stage shares the lifecycle");
    for value in [3, 4] {
        ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(value),
                admitted_at,
            ))
            .expect("fill the remaining physical capacity");
    }
    assert_eq!(ingress.remaining_capacity(), 0);
    let next_before_duplicate = ingress.next_admission_ordinal;
    ingress
        .enqueue(restored(1, 8))
        .expect("the exact restored retry coalesces at full capacity");
    assert_eq!(ingress.len(), 4);
    assert_eq!(
        ingress.next_admission_ordinal, next_before_duplicate,
        "coalescing cannot mint another physical admission ordinal"
    );
    assert_eq!(
        ingress.enqueue(restored_with_ordinal(
            1,
            8,
            owner_tag,
            CommandClass::Completion,
            2,
        )),
        Err(EnqueueError::FailClosed),
        "one restored lifecycle key cannot change its immutable ordinal at the same stage"
    );
    assert_eq!(
        ingress.enqueue(restored_with_ordinal(
            2,
            9,
            owner_tag,
            CommandClass::Completion,
            2,
        )),
        Err(EnqueueError::FailClosed),
        "a restored successor stage cannot change its lifecycle ordinal"
    );
    assert_eq!(
        ingress.enqueue(restored(9, 8)),
        Err(EnqueueError::FailClosed),
        "one persisted lifecycle stage cannot carry conflicting command identity"
    );
    assert_eq!(
        ingress.enqueue(restored_with(1, 8, owner_tag, CommandClass::Progress,)),
        Err(EnqueueError::FailClosed),
        "one persisted lifecycle stage cannot change its service class"
    );
    assert_eq!(
        ingress.enqueue(restored_with(
            1,
            8,
            EventTag::new(
                owner_tag.height(),
                owner_tag.view(),
                Generation::new(owner_tag.generation().get() + 1),
            ),
            CommandClass::Completion,
        )),
        Err(EnqueueError::FailClosed),
        "one queued restart stage cannot change its exact reducer tag"
    );
    let mut changed_origin = restored(1, 8);
    changed_origin.causal_origin.root_ingress_identity =
        Some(Hash::new(b"foreign restored ingress origin"));
    changed_origin.causal_origin.projection_hash =
        runtime_candidate_causal_origin_projection_hash(&changed_origin.causal_origin);
    assert!(changed_origin.validate_admission_identity());
    assert_eq!(
        ingress.enqueue(changed_origin),
        Err(EnqueueError::FailClosed),
        "one persisted lifecycle stage cannot change causal-origin metadata"
    );
    assert_eq!(ingress.len(), 4);
}
#[test]
fn restored_producer_preflight_cannot_change_completion_service_class() {
    let owner_tag = tag(0);
    let mut driver = FakeDriver::new(owner_tag);
    driver.admission_preflight_override = Some(RuntimeCommandAdmissionPreflight::ReuseDormant {
        causal_lifecycle_key: Hash::new(b"persisted completion lifecycle"),
        admission_ordinal: 1,
        producer_stage: 5,
    });
    let started_at = Instant::now();
    let mut runtime = runtime(driver, started_at, RuntimeQueueConfig::new(4, 1, 1));
    assert_eq!(
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(1),
        ),
        Err(EnqueueError::FailClosed)
    );
    assert!(runtime.fail_closed);
    assert_eq!(
        runtime.ingress.len(),
        0,
        "a caller-class mutation cannot acquire a priority position"
    );
}
