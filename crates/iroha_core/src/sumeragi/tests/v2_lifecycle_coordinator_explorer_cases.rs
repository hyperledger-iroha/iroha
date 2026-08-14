#[test]
fn dependency_free_explorer_covers_capacities_peers_stages_and_eight_events() {
    let mut covered_stages = BTreeSet::new();
    let mut covered_phases = BTreeSet::new();
    let mut covered_work_classes = BTreeSet::new();
    let mut explored_states = 0_usize;
    for capacity in 0..=3 {
        let geometry = capacities(capacity);
        for template_start in (0..EXPLORER_TEMPLATES.len()).step_by(4) {
            let candidates: Vec<_> = (0_u8..4)
                .map(|peer| {
                    let seed = 100_u8
                        .checked_add(peer * 20)
                        .and_then(|seed| {
                            seed.checked_add(
                                u8::try_from(template_start)
                                    .expect("explorer template index fits u8"),
                            )
                        })
                        .expect("explorer seed remains representable");
                    let template = EXPLORER_TEMPLATES
                        [(template_start + usize::from(peer)) % EXPLORER_TEMPLATES.len()];
                    capacity_matched(explorer_candidate(seed, template), &geometry)
                })
                .collect();
            let initial = LifecycleCoordinator::new(context(), 0, geometry.clone());
            let mut frontier = vec![initial.clone()];
            let mut seen = BTreeSet::from([format!("{initial:?}")]);
            for depth in 0_u64..=8 {
                let mut next = Vec::new();
                for state in frontier {
                    explored_states += 1;
                    assert_coordinator_invariants(&state);
                    for record in state.records.values() {
                        covered_stages.insert(record.stage.kind);
                        covered_phases.insert(record.key.phase);
                        covered_work_classes.insert(record.work_class);
                    }
                    if depth == 8 || state.fault.is_some() {
                        continue;
                    }
                    let mut successors = Vec::new();
                    for candidate in &candidates {
                        let mut admitted = state.clone();
                        admitted.admit(AdmissionRequest::Candidate(candidate.clone()));
                        successors.push(admitted);
                    }
                    if state.active_lease.is_none() {
                        let mut planned = state.clone();
                        plan_turn(
                            &mut planned,
                            [(WaitSource::External(digest(240)), depth + 1)],
                        );
                        successors.push(planned);
                    } else {
                        let lease = state
                            .active_lease
                            .clone()
                            .expect("active explorer lease is present");
                        let lease_capacity_class = lease.work_class.capacity_class();
                        for outcome in [
                            TurnOutcome::Advanced,
                            TurnOutcome::Terminal(TerminalOutcome::Completed(None)),
                            TurnOutcome::Terminal(TerminalOutcome::Completed(Some(digest(241)))),
                            TurnOutcome::Terminal(TerminalOutcome::Cancelled),
                            TurnOutcome::Blocked(WaitToken::new(
                                WaitSource::External(digest(240)),
                                depth,
                            )),
                            TurnOutcome::Replenished(PhysicalSlot::new(
                                PhysicalSlotId::for_capacity(lease_capacity_class, 1),
                                digest(u8::try_from(depth + 1).expect("depth is at most eight")),
                            )),
                        ] {
                            let mut settled = state.clone();
                            settle_with_test_serve_receipt(&mut settled, lease.clone(), outcome);
                            successors.push(settled);
                        }
                    }
                    for record in state.records.values().filter(|record| {
                        matches!(record.state, LifecycleState::Waiting(_))
                            && !matches!(
                                record.state,
                                LifecycleState::Waiting(WaitToken {
                                    source: WaitSource::ProducerTurn(_),
                                    ..
                                })
                            )
                    }) {
                        let LifecycleState::Waiting(wait) = record.state else {
                            unreachable!("filtered waiting record")
                        };
                        let mut published = state.clone();
                        published.publish_ready(ReadyEvent::new(
                            record.ordinal,
                            record.owner,
                            wait,
                            None,
                        ));
                        successors.push(published);
                    }
                    let mut restarted = LifecycleCoordinator::new_with_authority(
                        state.episode_authority.clone(),
                        state.high_water,
                    );
                    restarted.reconcile_restart(recovery_snapshot(&state));
                    successors.push(restarted);
                    if state.active_lease.is_none()
                        && let Some(successor_height) = state.active_context.height.checked_add(1)
                    {
                        let successor = LifecycleContext::new(
                            digest(
                                220_u8
                                    .checked_add(
                                        u8::try_from(depth).expect("depth is at most eight"),
                                    )
                                    .expect("rollover explorer digest fits"),
                            ),
                            successor_height,
                        );
                        let mut rolled = state.clone();
                        rolled.rollover(RolloverSnapshot {
                            retired_context: state.active_context,
                            successor_context: successor,
                            successor_predecessor: state.active_context.id,
                            successor_authority: authority(
                                successor,
                                state.capacity_geometry.clone(),
                            ),
                            successor_ledger_root: None,
                            serve_cancellations: Vec::new(),
                            retained_high_water: state.high_water,
                            retire_ordinals: state
                                .records
                                .iter()
                                .filter_map(|(ordinal, record)| {
                                    (!matches!(record.state, LifecycleState::Terminal(_)))
                                        .then_some(*ordinal)
                                })
                                .collect(),
                            retire_admission_keys: state.admission_waits.keys().copied().collect(),
                        });
                        successors.push(rolled);
                    }
                    for successor in successors {
                        assert_terminal_irreversibility(&state, &successor);
                        assert_coordinator_invariants(&successor);
                        let signature = format!("{successor:?}");
                        if seen.insert(signature) {
                            next.push(successor);
                        }
                    }
                }
                frontier = next;
            }
        }
    }
    assert_eq!(covered_stages, BTreeSet::from(LifecycleStageKind::ALL));
    assert_eq!(covered_phases, BTreeSet::from(LifecyclePhase::ALL));
    assert_eq!(
        covered_work_classes,
        BTreeSet::from(LifecycleWorkClass::ALL)
    );
    assert!(explored_states > 10_000);
}
#[test]
fn restart_seeds_high_water_and_rollover_preserves_it() {
    let mut coordinator = LifecycleCoordinator::new(context(), 5, capacities(8));
    let owner = OwnerId {
        causal_root: CausalRoot::new(digest(77)),
        first_admission_ordinal: 5,
    };
    let replay = super::replay_authority::exact_record_fixture(
        context(),
        LifecycleStageKind::ApplyDecision,
        30,
    );
    let recovered = RecoveredLifecycleRecord {
        key: replay.key,
        owner,
        ordinal: 5,
        work_class: LifecycleWorkClass::Apply,
        stage: stage(
            LifecycleStageKind::ApplyDecision,
            1,
            PredecessorScope::Independent,
        ),
        terminal: None,
        reconstruction_source: digest(88),
        payload: replay.payload,
        replay_authority: replay.authority,
        continuation: DurableContinuation::None,
        physical_slot_universe: BTreeSet::from([PhysicalSlotId::for_capacity(
            CapacityClass::Effect,
            0,
        )]),
    };
    coordinator.reconcile_restart(RecoverySnapshot {
        context: context(),
        high_water: 5,
        records: vec![recovered],
        producer_debts: BTreeMap::new(),
    });
    assert_eq!(coordinator.high_water, 5);
    assert_eq!(
        coordinator.records[&5].state,
        LifecycleState::Waiting(WaitToken::new(WaitSource::Recovery(digest(88)), 0))
    );
    let mut premature = coordinator.clone();
    premature.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: LifecycleContext::new(digest(44), 8),
        successor_predecessor: context().id,
        successor_authority: authority(LifecycleContext::new(digest(44), 8), capacities(8)),
        successor_ledger_root: None,
        serve_cancellations: Vec::new(),
        retained_high_water: 5,
        retire_ordinals: BTreeSet::new(),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(premature.fault, Some(CoordinatorFault::InvalidRollover));
    coordinator.publish_ready(ReadyEvent::new(
        5,
        owner,
        WaitToken::new(WaitSource::Recovery(digest(88)), 0),
        None,
    ));
    let lease = execute(plan_turn(&mut coordinator, []));
    coordinator.settle_turn(lease, TurnOutcome::Advanced);
    coordinator.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: LifecycleContext::new(digest(44), 8),
        successor_predecessor: context().id,
        successor_authority: authority(LifecycleContext::new(digest(44), 8), capacities(8)),
        successor_ledger_root: None,
        serve_cancellations: Vec::new(),
        retained_high_water: 5,
        retire_ordinals: BTreeSet::new(),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(
        (
            coordinator.active_context,
            coordinator.high_water,
            coordinator.records.len()
        ),
        (LifecycleContext::new(digest(44), 8), 5, 0)
    );
    let mut successor = candidate(
        40,
        LifecycleWorkClass::Fetch,
        LifecyclePhase::Fetch,
        InitialLifecycleState::Ready,
        PredecessorScope::Independent,
    );
    successor.key.context = digest(44);
    successor.key.round.height = 8;
    successor
        .key
        .proposal_round
        .as_mut()
        .expect("proposal round")
        .height = 8;
    assert_eq!(
        admitted(coordinator.admit(AdmissionRequest::Candidate(successor))).1,
        6
    );
}
#[test]
fn restart_rejects_a_no_successor_validate_without_its_body_frame() {
    let geometry = capacities(8);
    let mut request = capacity_matched(
        candidate(
            32,
            LifecycleWorkClass::Validate,
            LifecyclePhase::Validate,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ),
        &geometry,
    );
    request.reconstruction_source = request.causal_root.digest();
    let key = request.key;
    request.payload = DurablePayloadReference::BodyFrame(schema::DurableBodyFrameReference::new(
        key.context(),
        key.proposal_round().expect("Validate proposal round"),
        key.subject().expect("Validate body subject"),
        digest(91),
        digest(92),
    ));
    let mut live = LifecycleCoordinator::new(context(), 0, geometry.clone());
    admitted(live.admit(AdmissionRequest::Candidate(request)));
    let live_snapshot = recovery_snapshot(&live);
    let mut live_payload_free = live_snapshot.clone();
    live_payload_free.records[0].payload = DurablePayloadReference::None;
    let mut rejected_live = LifecycleCoordinator::new(context(), 1, geometry.clone());
    rejected_live.reconcile_restart(live_payload_free);
    assert_eq!(
        rejected_live.fault,
        Some(CoordinatorFault::RecoveryRejected)
    );
    assert!(rejected_live.records.is_empty());
    let mut exact = live_snapshot;
    exact.records[0].terminal = Some(TerminalOutcome::Advanced);
    exact.records[0].continuation = DurableContinuation::AdvancedNoSuccessor;
    let mut recovered = LifecycleCoordinator::new(context(), 1, geometry.clone());
    recovered.reconcile_restart(exact.clone());
    assert_eq!(recovered.fault, None);
    assert_eq!(
        recovered.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    let mut payload_free = exact;
    payload_free.records[0].payload = DurablePayloadReference::None;
    let mut rejected = LifecycleCoordinator::new(context(), 1, geometry);
    rejected.reconcile_restart(payload_free);
    assert_eq!(rejected.fault, Some(CoordinatorFault::RecoveryRejected));
    assert!(rejected.records.is_empty());
}
#[test]
fn exact_retry_rebinds_recovered_work_without_allocating() {
    let request = candidate(
        33,
        LifecycleWorkClass::Fetch,
        LifecyclePhase::Fetch,
        InitialLifecycleState::Ready,
        PredecessorScope::Independent,
    );
    let mut live = LifecycleCoordinator::new(context(), 0, capacities(8));
    let (owner, ordinal, _) = admitted(live.admit(AdmissionRequest::Candidate(request.clone())));
    let snapshot = recovery_snapshot(&live);
    let mut recovered = LifecycleCoordinator::new(context(), 1, capacities(8));
    recovered.reconcile_restart(snapshot);
    assert_eq!(recovered.high_water, 1);
    assert!(recovered.records[&ordinal].physical_slots.is_empty());
    assert!(matches!(
        recovered.records[&ordinal].state,
        LifecycleState::Waiting(WaitToken {
            source: WaitSource::Recovery(_),
            ..
        })
    ));
    assert_eq!(
        recovered.admit(AdmissionRequest::Candidate(request)),
        AdmissionDecision::Retry {
            owner,
            ordinal,
            action: RetryAction::ReenqueueIncumbent,
        }
    );
    assert_eq!(recovered.high_water, 1);
    assert_eq!(recovered.records[&ordinal].state, LifecycleState::Ready);
    assert!(!recovered.records[&ordinal].physical_slots.is_empty());
}
#[test]
fn restart_reconciliation_is_pristine_and_high_water_fenced() {
    let mut live = LifecycleCoordinator::new(context(), 0, capacities(8));
    admitted(live.admit(AdmissionRequest::Candidate(candidate(
        31,
        LifecycleWorkClass::Fetch,
        LifecyclePhase::Fetch,
        InitialLifecycleState::Ready,
        PredecessorScope::Independent,
    ))));
    let records_before = live.records.clone();
    let high_water = live.high_water;
    live.reconcile_restart(RecoverySnapshot {
        context: context(),
        high_water,
        records: Vec::new(),
        producer_debts: BTreeMap::new(),
    });
    assert_eq!(live.fault, Some(CoordinatorFault::RecoveryRejected));
    assert_eq!(live.records, records_before);
    assert_eq!(live.high_water, high_water);
    let mut wrong_seed = LifecycleCoordinator::new(context(), 1, capacities(8));
    wrong_seed.reconcile_restart(RecoverySnapshot {
        context: context(),
        high_water: 2,
        records: Vec::new(),
        producer_debts: BTreeMap::new(),
    });
    assert_eq!(wrong_seed.fault, Some(CoordinatorFault::RecoveryRejected));
    assert_eq!(wrong_seed.high_water, 1);
}
#[test]
fn rollover_requires_exact_retirement_and_an_immediate_bound_successor() {
    let geometry = capacities(1);
    let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
    let request = capacity_matched(
        candidate(
            41,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ),
        &geometry,
    );
    admitted(coordinator.admit(AdmissionRequest::Candidate(request)));
    let successor = LifecycleContext::new(digest(45), 8);
    let mut wrong_parent = coordinator.clone();
    wrong_parent.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: successor,
        successor_predecessor: digest(99),
        successor_authority: authority(successor, geometry.clone()),
        successor_ledger_root: None,
        serve_cancellations: Vec::new(),
        retained_high_water: 1,
        retire_ordinals: BTreeSet::from([1]),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(wrong_parent.fault, Some(CoordinatorFault::InvalidRollover));
    let mut skipped = coordinator.clone();
    skipped.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: LifecycleContext::new(digest(45), 9),
        successor_predecessor: context().id,
        successor_authority: authority(LifecycleContext::new(digest(45), 9), geometry.clone()),
        successor_ledger_root: None,
        serve_cancellations: Vec::new(),
        retained_high_water: 1,
        retire_ordinals: BTreeSet::from([1]),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(skipped.fault, Some(CoordinatorFault::InvalidRollover));
    coordinator.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: successor,
        successor_predecessor: context().id,
        successor_authority: authority(successor, geometry.clone()),
        successor_ledger_root: None,
        serve_cancellations: Vec::new(),
        retained_high_water: 1,
        retire_ordinals: BTreeSet::from([1]),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(coordinator.active_context, successor);
    assert!(coordinator.records.is_empty());
    let zero_geometry = capacities(0);
    let mut fenced = LifecycleCoordinator::new(context(), 0, zero_geometry.clone());
    let pending = capacity_matched(
        candidate(
            42,
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            InitialLifecycleState::Ready,
            PredecessorScope::Independent,
        ),
        &zero_geometry,
    );
    assert!(matches!(
        fenced.admit(AdmissionRequest::Candidate(pending.clone())),
        AdmissionDecision::WaitForCapacity(_)
    ));
    fenced.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: successor,
        successor_predecessor: context().id,
        successor_authority: authority(successor, zero_geometry.clone()),
        successor_ledger_root: None,
        serve_cancellations: Vec::new(),
        retained_high_water: 0,
        retire_ordinals: BTreeSet::new(),
        retire_admission_keys: BTreeSet::from([pending.key]),
    });
    assert_eq!(fenced.active_context, successor);
    assert!(fenced.admission_waits.is_empty());
    let max_context = LifecycleContext::new(digest(46), u64::MAX);
    let mut overflow = LifecycleCoordinator::new(max_context, 0, capacities(1));
    overflow.rollover(RolloverSnapshot {
        retired_context: max_context,
        successor_context: LifecycleContext::new(digest(47), 0),
        successor_predecessor: max_context.id,
        successor_authority: authority(LifecycleContext::new(digest(47), 0), capacities(1)),
        successor_ledger_root: None,
        serve_cancellations: Vec::new(),
        retained_high_water: 0,
        retire_ordinals: BTreeSet::new(),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(overflow.fault, Some(CoordinatorFault::InvalidRollover));
}
#[test]
fn durable_rollover_persists_retirement_then_opens_high_water_successor() {
    let root = tempfile::tempdir().expect("temporary rollover root");
    let retired_root = root.path().join("retired");
    let successor_root = root.path().join("successor");
    let geometry = capacities(2);
    let retired_authority = authority(context(), geometry.clone());
    let mut coordinator = LifecycleCoordinator::new_with_authority(retired_authority, 0);
    coordinator
        .attach_empty_test_ledger(&retired_root)
        .expect("attach predecessor ledger");
    admitted(
        coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
            candidate(
                43,
                LifecycleWorkClass::Fetch,
                LifecyclePhase::Fetch,
                InitialLifecycleState::Ready,
                PredecessorScope::Independent,
            ),
            &geometry,
        ))),
    );
    let successor = LifecycleContext::new(digest(48), 8);
    coordinator.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: successor,
        successor_predecessor: context().id,
        successor_authority: authority(successor, geometry.clone()),
        successor_ledger_root: Some(successor_root.clone()),
        serve_cancellations: Vec::new(),
        retained_high_water: 1,
        retire_ordinals: BTreeSet::from([1]),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(coordinator.fault, None);
    assert_eq!(coordinator.active_context, successor);
    assert_eq!(coordinator.high_water, 1);
    assert!(coordinator.records.is_empty());
    assert!(coordinator.ledger_store.is_some());
    let (_, retired) = ledger::LifecycleLedgerStoreV1::open(&retired_root, context())
        .expect("retired ledger reloads");
    assert_eq!(
        retired.records()[0].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
    let (_, opened_successor) = ledger::LifecycleLedgerStoreV1::open(&successor_root, successor)
        .expect("successor ledger reloads");
    assert_eq!(opened_successor.high_water(), 1);
    assert!(opened_successor.records().is_empty());
    // Authenticated durable-open coverage lives with the real signed
    // Certified-Serve fixtures in the sealed projection module. This
    // rollover fixture intentionally has no forgeable wire authority from
    // which to mint an authenticated empty payload cut.
}
#[test]
fn durable_rollover_rejects_live_serve_without_payload_cancellation_receipt() {
    let root = tempfile::tempdir().expect("temporary rollover root");
    let retired_root = root.path().join("retired");
    let successor_root = root.path().join("successor");
    let geometry = capacities(2);
    let mut coordinator = LifecycleCoordinator::new(context(), 0, geometry.clone());
    coordinator
        .attach_empty_test_ledger(&retired_root)
        .expect("attach predecessor ledger");
    admitted(
        coordinator.admit(AdmissionRequest::Candidate(capacity_matched(
            serve_candidate(44, InitialLifecycleState::Ready),
            &geometry,
        ))),
    );
    let successor = LifecycleContext::new(digest(49), 8);
    coordinator.rollover(RolloverSnapshot {
        retired_context: context(),
        successor_context: successor,
        successor_predecessor: context().id,
        successor_authority: authority(successor, geometry),
        successor_ledger_root: Some(successor_root),
        serve_cancellations: Vec::new(),
        retained_high_water: 2,
        retire_ordinals: BTreeSet::from([1, 2]),
        retire_admission_keys: BTreeSet::new(),
    });
    assert_eq!(coordinator.fault, Some(CoordinatorFault::InvalidRollover));
    assert!(matches!(
        coordinator.records[&1].state,
        LifecycleState::Ready
    ));
    let (_, persisted) = ledger::LifecycleLedgerStoreV1::open(&retired_root, context())
        .expect("pre-rollover ledger remains readable");
    assert_eq!(persisted.records()[0].terminal(), Some(None));
}
#[test]
fn restart_derives_ready_producer_debt_from_terminal_serve() {
    let owner = OwnerId {
        causal_root: CausalRoot::new(digest(78)),
        first_admission_ordinal: 1,
    };
    let serve = recovered_pair_record(
        31,
        owner,
        1,
        LifecycleWorkClass::CertifiedServe,
        Some(TerminalOutcome::Completed(Some(digest(84)))),
    );
    let producer = recovered_pair_record(31, owner, 2, LifecycleWorkClass::ProducerTurn, None);
    let geometry = capacities(2);
    let serve = recovery_capacity_matched(serve, &geometry);
    let producer = recovery_capacity_matched(producer, &geometry);
    let mut coordinator = LifecycleCoordinator::new(context(), 2, geometry);
    coordinator.reconcile_restart(RecoverySnapshot {
        context: context(),
        high_water: 2,
        records: vec![serve, producer],
        producer_debts: BTreeMap::from([(1, 2)]),
    });
    assert_eq!(coordinator.records[&2].state, LifecycleState::Ready);
    assert_eq!(coordinator.producer_debts, BTreeMap::from([(1, 2)]));
    let lease = execute(plan_turn(&mut coordinator, []));
    assert_eq!(lease.ordinal, 2);
    coordinator.settle_turn(
        lease,
        TurnOutcome::Terminal(TerminalOutcome::Completed(None)),
    );
    assert!(coordinator.producer_debts.is_empty());
}
#[test]
fn recovery_requires_a_bijective_atomic_serve_producer_pair() {
    let owner = OwnerId {
        causal_root: CausalRoot::new(digest(79)),
        first_admission_ordinal: 1,
    };
    let serve = |ordinal, terminal| {
        recovered_pair_record(
            60,
            owner,
            ordinal,
            LifecycleWorkClass::CertifiedServe,
            terminal,
        )
    };
    let producer = |record_owner, ordinal, terminal| {
        recovered_pair_record(
            60,
            record_owner,
            ordinal,
            LifecycleWorkClass::ProducerTurn,
            terminal,
        )
    };
    let rejected = |records: Vec<RecoveredLifecycleRecord>,
                    producer_debts: BTreeMap<u128, u128>| {
        let high_water = records
            .iter()
            .map(|record: &RecoveredLifecycleRecord| record.ordinal)
            .max()
            .unwrap_or(0);
        let mut coordinator = LifecycleCoordinator::new(context(), high_water, capacities(8));
        coordinator.reconcile_restart(RecoverySnapshot {
            context: context(),
            high_water,
            records,
            producer_debts,
        });
        assert_eq!(coordinator.fault, Some(CoordinatorFault::RecoveryRejected));
    };
    rejected(
        vec![serve(1, Some(TerminalOutcome::Completed(Some(digest(82)))))],
        BTreeMap::new(),
    );
    rejected(
        vec![producer(owner, 2, Some(TerminalOutcome::Completed(None)))],
        BTreeMap::new(),
    );
    rejected(
        vec![serve(1, None), producer(owner, 3, None)],
        BTreeMap::from([(1, 3)]),
    );
    let owner_two = OwnerId {
        causal_root: CausalRoot::new(digest(81)),
        first_admission_ordinal: 3,
    };
    rejected(
        vec![
            serve(1, None),
            producer(owner, 2, None),
            recovered_pair_record(62, owner_two, 3, LifecycleWorkClass::CertifiedServe, None),
            recovered_pair_record(62, owner_two, 4, LifecycleWorkClass::ProducerTurn, None),
        ],
        BTreeMap::from([(1, 2), (3, 2)]),
    );
    let foreign_owner = OwnerId {
        causal_root: CausalRoot::new(digest(80)),
        first_admission_ordinal: 2,
    };
    rejected(
        vec![serve(1, None), producer(foreign_owner, 2, None)],
        BTreeMap::from([(1, 2)]),
    );
    let mut mismatched_key = producer(owner, 2, None);
    mismatched_key.key.subject = Some(digest(199));
    rejected(
        vec![serve(1, None), mismatched_key],
        BTreeMap::from([(1, 2)]),
    );
    let mut mismatched_source = producer(owner, 2, None);
    mismatched_source.reconstruction_source = digest(198);
    rejected(
        vec![serve(1, None), mismatched_source],
        BTreeMap::from([(1, 2)]),
    );
    let mut foreign_replay_family = producer(owner, 2, None);
    foreign_replay_family.replay_authority =
        super::replay_authority::foreign_certified_serve_family_authority_fixture(
            context(),
            LifecycleStageKind::ProducerTurn,
            60,
        );
    rejected(
        vec![serve(1, None), foreign_replay_family],
        BTreeMap::from([(1, 2)]),
    );
    let mut mixed_height = serve(1, None);
    mixed_height.key.round.height = 8;
    mixed_height
        .key
        .proposal_round
        .as_mut()
        .expect("proposal round")
        .height = 8;
    rejected(
        vec![mixed_height, producer(owner, 2, None)],
        BTreeMap::from([(1, 2)]),
    );
    rejected(
        vec![
            serve(1, Some(TerminalOutcome::Cancelled)),
            producer(owner, 2, Some(TerminalOutcome::Completed(None))),
        ],
        BTreeMap::new(),
    );
    for (serve_outcome, producer_outcome) in [
        (
            TerminalOutcome::Completed(Some(digest(82))),
            TerminalOutcome::Completed(None),
        ),
        (TerminalOutcome::Cancelled, TerminalOutcome::Cancelled),
    ] {
        let mut coordinator = LifecycleCoordinator::new(context(), 2, capacities(8));
        coordinator.reconcile_restart(RecoverySnapshot {
            context: context(),
            high_water: 2,
            records: vec![
                serve(1, Some(serve_outcome)),
                producer(owner, 2, Some(producer_outcome)),
            ],
            producer_debts: BTreeMap::new(),
        });
        assert_eq!(coordinator.fault, None);
        assert_eq!(coordinator.records.len(), 2);
    }
}
