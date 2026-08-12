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
                                TurnOutcome::Terminal(TerminalOutcome::Completed(Some(digest(
                                    241,
                                )))),
                                TurnOutcome::Terminal(TerminalOutcome::Cancelled),
                                TurnOutcome::Blocked(WaitToken::new(
                                    WaitSource::External(digest(240)),
                                    depth,
                                )),
                                TurnOutcome::Replenished(PhysicalSlot::new(
                                    PhysicalSlotId::for_capacity(lease_capacity_class, 1),
                                    digest(
                                        u8::try_from(depth + 1).expect("depth is at most eight"),
                                    ),
                                )),
                            ] {
                                let mut settled = state.clone();
                                settle_with_test_serve_receipt(
                                    &mut settled,
                                    lease.clone(),
                                    outcome,
                                );
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
                            && let Some(successor_height) =
                                state.active_context.height.checked_add(1)
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
                                retire_admission_keys: state
                                    .admission_waits
                                    .keys()
                                    .copied()
                                    .collect(),
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
