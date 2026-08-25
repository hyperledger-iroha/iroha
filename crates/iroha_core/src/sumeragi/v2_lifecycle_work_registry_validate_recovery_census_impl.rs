impl ConcreteLifecycleWorkRegistry {
#[allow(clippy::too_many_lines)]
    fn exactly_covers_all_live_work_with_optional_active_producer(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &LifecycleCoordinator,
        active_producer: Option<&TurnLease>,
    ) -> bool {
        let active_producer_is_exact = match (&coordinator.active_lease, active_producer) {
            (None, None) => true,
            (Some(active), Some(expected)) => {
                active == expected && active.work_class == LifecycleWorkClass::ProducerTurn
            }
            (None, Some(_)) | (Some(_), None) => false,
        };
        if coordinator.fault.is_some()
            || !active_producer_is_exact
            || coordinator.active_context != projection::lifecycle_context(verified.context())
            || coordinator.episode_authority.context() != coordinator.active_context
            || coordinator.episode_authority.capacity_geometry() != &coordinator.capacity_geometry
            || coordinator.records.len() != coordinator.durable_records.len()
            || coordinator.records.len() != coordinator.key_index.len()
        {
            return false;
        }
        let exact_capacity_classes = CapacityClass::ALL
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        if coordinator
            .capacity_generation
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            != exact_capacity_classes
            || coordinator
                .capacity_geometry
                .limits
                .keys()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                != exact_capacity_classes
        {
            return false;
        }
        if coordinator.admission_waits.len() > super::MAX_PENDING_ADMISSION_WAITS
            || coordinator.admission_waits.iter().any(|(key, waiting)| {
                let candidate = &waiting.candidate;
                let mut canonical = candidate.clone();
                let WaitSource::Capacity(class) = waiting.wait_token.source else {
                    return true;
                };
                let candidate_slots_are_exact =
                    candidate
                        .physical_geometry
                        .normalized()
                        .is_ok_and(|(_, slots, _)| {
                            coordinator
                                .episode_authority
                                .universe_for(candidate.key)
                                .is_some()
                                && coordinator
                                    .episode_authority
                                    .admits_slots(candidate.work_class.capacity_class(), &slots)
                        });
                let producer_slots_are_exact =
                    candidate.producer_turn.as_ref().is_none_or(|producer| {
                        producer
                            .physical_geometry
                            .normalized()
                            .is_ok_and(|(_, slots, _)| {
                                coordinator
                                    .episode_authority
                                    .universe_for(producer.key)
                                    .is_some()
                                    && coordinator
                                        .episode_authority
                                        .admits_slots(CapacityClass::Producer, &slots)
                            })
                    });
                let producer_shape_is_invalid =
                    match (candidate.work_class, candidate.producer_turn.as_ref()) {
                        (LifecycleWorkClass::CertifiedServe, Some(producer)) => {
                            !super::schema::serve_and_producer_keys_match(
                                candidate.key,
                                producer.key,
                            ) || producer.stage.kind != LifecycleStageKind::ProducerTurn
                                || producer.reconstruction_source != candidate.reconstruction_source
                        }
                        (LifecycleWorkClass::CertifiedServe, None) | (_, Some(_)) => true,
                        (_, None) => false,
                    };
                key != &candidate.key
                    || coordinator.key_index.contains_key(key)
                    || candidate.work_class == LifecycleWorkClass::ProducerTurn
                    || !candidate
                        .work_class
                        .accepts_stage(candidate.key.phase, candidate.stage)
                    || !candidate
                        .payload
                        .matches_terminal(candidate.work_class, None)
                    || (candidate.work_class == LifecycleWorkClass::Validate
                        && !super::body_pipeline_transition::durable_validate_payload_is_exact(
                            candidate.key,
                            candidate.payload,
                        ))
                    || matches!(
                        candidate.initial_state,
                        InitialLifecycleState::Waiting(WaitToken {
                            source: WaitSource::Capacity(_)
                                | WaitSource::Recovery(_)
                                | WaitSource::ProducerTurn(_),
                            ..
                        })
                    )
                    || matches!(
                        candidate.initial_state,
                        InitialLifecycleState::Waiting(WaitToken {
                            observed_generation: u64::MAX,
                            ..
                        })
                    )
                    || canonical.canonicalize_geometry().is_err()
                    || canonical != *candidate
                    || !candidate.replay_authority_is_exact(coordinator.active_context)
                    || !candidate_slots_are_exact
                    || !producer_slots_are_exact
                    || producer_shape_is_invalid
                    || (class != candidate.work_class.capacity_class()
                        && !(class == CapacityClass::Producer && candidate.producer_turn.is_some()))
                    || waiting.wait_token.observed_generation
                        > coordinator.capacity_generation[&class]
                    || waiting.serve_payload_receipt.is_some()
                        && candidate.work_class != LifecycleWorkClass::CertifiedServe
            })
        {
            return false;
        }
        let Ok(exact_ledger) = super::ledger::LifecycleLedgerV1::from_coordinator(coordinator)
        else {
            return false;
        };
        let mut exact_owners = BTreeMap::new();
        if coordinator.records.iter().any(|(&ordinal, record)| {
            let frozen_predecessors_are_invalid =
                record
                    .episode
                    .frozen_predecessors
                    .iter()
                    .any(|predecessor| {
                        *predecessor >= ordinal || !coordinator.records.contains_key(predecessor)
                    })
                    || (matches!(
                        record.stage.predecessor_scope,
                        PredecessorScope::Independent
                    ) && !record.episode.frozen_predecessors.is_empty())
                    || (!matches!(
                        record.stage.predecessor_scope,
                        PredecessorScope::Independent
                    ) && coordinator
                        .records
                        .range(..ordinal)
                        .any(|(predecessor, prior)| {
                            !matches!(prior.state, super::LifecycleState::Terminal(_))
                                && !record.episode.frozen_predecessors.contains(predecessor)
                        }));
            let wait_state_is_invalid = match record.state {
                super::LifecycleState::Waiting(wait) => match wait.source {
                    WaitSource::Capacity(_) => true,
                    WaitSource::External(_) | WaitSource::Recovery(_) => {
                        wait.observed_generation == u64::MAX
                            || coordinator
                                .observed_generation
                                .get(&wait.source)
                                .copied()
                                .unwrap_or(0)
                                != wait.observed_generation
                    }
                    WaitSource::ProducerTurn(serve_ordinal) => {
                        record.work_class != LifecycleWorkClass::ProducerTurn
                            || wait.observed_generation != 0
                            || coordinator.producer_debts.get(&serve_ordinal) != Some(&ordinal)
                    }
                },
                super::LifecycleState::Ready | super::LifecycleState::Terminal(_) => false,
                super::LifecycleState::Claimed(lease_id) => active_producer.is_none_or(|lease| {
                    record.ordinal != lease.ordinal
                        || record.work_class != LifecycleWorkClass::ProducerTurn
                        || lease_id != lease.id
                }),
            };
            let unique_digests = record
                .physical_slots
                .values()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            record.ordinal != ordinal
                || coordinator.key_index.get(&record.key) != Some(&ordinal)
                || record.owner.first_admission_ordinal() == 0
                || record.owner.first_admission_ordinal() > ordinal
                || coordinator
                    .episode_authority
                    .universe_for(record.key)
                    .as_ref()
                    != Some(&record.episode.universe)
                || !coordinator.episode_authority.admits_slots(
                    record.work_class.capacity_class(),
                    &record.episode.slot_universe,
                )
                || !record
                    .physical_slots
                    .keys()
                    .all(|slot| record.episode.slot_universe.contains(slot))
                || !record
                    .episode
                    .consumed_slots
                    .is_subset(&record.episode.slot_universe)
                || unique_digests.len() != record.physical_slots.len()
                || frozen_predecessors_are_invalid
                || wait_state_is_invalid
                || exact_owners
                    .insert(record.owner.causal_root(), record.owner)
                    .is_some_and(|known| known != record.owner)
                || coordinator
                    .durable_records
                    .get(&ordinal)
                    .is_none_or(|metadata| {
                        !metadata.replay_authority.structurally_matches_record(
                            coordinator.active_context,
                            record.key,
                            record.work_class,
                            record.stage,
                            metadata.payload,
                        )
                    })
        }) || coordinator.owner_index != exact_owners
        {
            return false;
        }
        let exact_ready = coordinator
            .records
            .values()
            .filter_map(|record| {
                (record.state == super::LifecycleState::Ready).then_some(record.ordinal)
            })
            .collect::<std::collections::BTreeSet<_>>();
        if coordinator.ready_index != exact_ready {
            return false;
        }
        let exact_capacity_used = CapacityClass::ALL
            .into_iter()
            .map(|class| {
                (
                    class,
                    coordinator
                        .records
                        .values()
                        .filter(|record| {
                            record.work_class.capacity_class() == class
                                && !matches!(record.state, super::LifecycleState::Terminal(_))
                        })
                        .count(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if coordinator.capacity_used != exact_capacity_used
            || CapacityClass::ALL.into_iter().any(|class| {
                exact_capacity_used[&class] > coordinator.capacity_geometry.limit(class)
            })
        {
            return false;
        }
        let live = coordinator
            .records
            .iter()
            .filter(|(_, record)| !matches!(record.state, super::LifecycleState::Terminal(_)))
            .collect::<Vec<_>>();
        if self.entries.len() != live.len() {
            return false;
        }
        if !coordinator
            .producer_debts
            .iter()
            .all(|(&serve_ordinal, &producer_ordinal)| {
                let (Some(serve), Some(producer)) = (
                    coordinator.records.get(&serve_ordinal),
                    coordinator.records.get(&producer_ordinal),
                ) else {
                    return false;
                };
                let (Some(serve_metadata), Some(producer_metadata)) = (
                    coordinator.durable_records.get(&serve_ordinal),
                    coordinator.durable_records.get(&producer_ordinal),
                ) else {
                    return false;
                };
                if !serve_ordinal_pair_is_exact(serve, producer)
                    || !serve_metadata
                        .replay_authority
                        .same_persisted_family(&producer_metadata.replay_authority)
                {
                    return false;
                }
                if matches!(serve.state, super::LifecycleState::Terminal(_)) {
                    return true;
                }
                let (Some((serve_slot, _)), Some((producer_slot, _))) = (
                    exact_single_record_slot(
                        serve,
                        LifecycleWorkClass::CertifiedServe.capacity_class(),
                    ),
                    exact_single_record_slot(
                        producer,
                        LifecycleWorkClass::ProducerTurn.capacity_class(),
                    ),
                ) else {
                    return false;
                };
                let (Some(serve_address), Some(producer_address)) = (
                    ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot),
                    ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot),
                ) else {
                    return false;
                };
                matches!(
                    (
                        self.entries.get(&serve_address).map(|work| &work.kind),
                        self.entries.get(&producer_address).map(|work| &work.kind),
                    ),
                    (
                        Some(ConcreteLifecycleWorkKind::DurableCertifiedServe(serve)),
                        Some(ConcreteLifecycleWorkKind::DurableProducerTurn(producer)),
                    ) if Arc::ptr_eq(&serve.replay_evidence, &producer.replay_evidence)
                )
            })
        {
            return false;
        }
        let exact_next_vote_addresses = self
            .entries
            .iter()
            .filter_map(|(&address, work)| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
                )
                .then_some(address)
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut paired_next_vote_addresses = std::collections::BTreeSet::new();
        if self.entries.values().any(|work| {
            let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
                &work.kind
            else {
                return false;
            };
            let Some((next_address, next_digest)) = broadcast.paired_next_sign else {
                return !broadcast.is_unpaired();
            };
            !broadcast.pairs_exact_next_sign(next_address, next_digest)
                || !paired_next_vote_addresses.insert(next_address)
                || self.entries.get(&next_address).is_none_or(|next_work| {
                    next_work.digest != next_digest
                        || !matches!(
                            &next_work.kind,
                            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
                        )
                })
        }) || !paired_next_vote_addresses.is_subset(&exact_next_vote_addresses)
        {
            return false;
        }

        live.into_iter().all(|(&ordinal, record)| {
            if record.ordinal != ordinal
                || matches!(record.state, super::LifecycleState::Claimed(_))
                    && active_producer.is_none_or(|lease| {
                        record.ordinal != lease.ordinal
                            || record.state != super::LifecycleState::Claimed(lease.id)
                    })
                || coordinator.key_index.get(&record.key) != Some(&ordinal)
                || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
                || coordinator.high_water < ordinal
            {
                return false;
            }
            let Some(metadata) = coordinator.durable_records.get(&ordinal) else {
                return false;
            };
            if !metadata.replay_authority.structurally_matches_record(
                coordinator.active_context,
                record.key,
                record.work_class,
                record.stage,
                metadata.payload,
            ) {
                return false;
            }
            let Some((slot, digest)) =
                exact_single_record_slot(record, record.work_class.capacity_class())
            else {
                return false;
            };
            let Some(address) = ConcreteWorkAddress::new(record.owner, ordinal, slot) else {
                return false;
            };
            let Some(work) = self.entries.get(&address) else {
                return false;
            };
            if work.digest != digest || !work.validates_at(address) {
                return false;
            }

            let candidate_core_matches = |candidate: &CandidateAdmission| {
                let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized()
                else {
                    return false;
                };
                candidate.key == record.key
                    && candidate.causal_root == record.owner.causal_root()
                    && candidate.work_class == record.work_class
                    && candidate.stage == record.stage
                    && candidate.reconstruction_source == metadata.reconstruction_source
                    && candidate.producer_turn.is_none()
                    && physical.len() == 1
                    && physical.contains_key(&slot)
                    && record.episode.slot_universe == universe
                    && record.episode.consumed_slots == consumed
                    && metadata.matches_admission(candidate)
                    && metadata.continuation == super::schema::DurableContinuation::None
            };

            match &work.kind {
                ConcreteLifecycleWorkKind::PendingAdapter {
                    effect,
                    pending,
                    replay_authority,
                } => {
                    let Ok(projected) = projection::authority_free_admission_projection(
                        coordinator.active_context,
                        verified,
                        effect,
                        pending,
                    ) else {
                        return false;
                    };
                    let Ok((physical, universe, consumed)) =
                        projected.physical_geometry.normalized()
                    else {
                        return false;
                    };
                    let payload_is_exact = match (
                        projected.work_class,
                        projected.stage.kind(),
                        metadata.payload,
                    ) {
                        (
                            LifecycleWorkClass::Apply,
                            LifecycleStageKind::ApplyDecision,
                            DurablePayloadReference::BodyFrame(frame),
                        ) => frame.matches_key(record.key),
                        (LifecycleWorkClass::Apply, _, _) => false,
                        (_, _, DurablePayloadReference::None) => true,
                        _ => false,
                    };
                    let candidate = CandidateAdmission::new(
                        projected.key,
                        projected.causal_root,
                        projected.work_class,
                        projected.stage,
                        projected.initial_state,
                        projected.reconstruction_source,
                        metadata.payload,
                        metadata.replay_authority.clone(),
                        projected.physical_geometry,
                        None,
                    );
                    candidate.initial_state == InitialLifecycleState::Ready
                        && candidate_core_matches(&candidate)
                        && physical == record.physical_slots
                        && universe == record.episode.slot_universe
                        && consumed == record.episode.consumed_slots
                        && payload_is_exact
                        && replay_authority == &metadata.replay_authority
                }
                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                    let candidate = CandidateAdmission::new(
                        record.key,
                        record.owner.causal_root(),
                        record.work_class,
                        record.stage,
                        InitialLifecycleState::Ready,
                        metadata.reconstruction_source,
                        metadata.payload,
                        metadata.replay_authority.clone(),
                        super::PhysicalGeometry::new([PhysicalSlot::new(slot, digest)], [slot]),
                        None,
                    );
                    record.state == super::LifecycleState::Ready
                        && candidate_core_matches(&candidate)
                        && completion.matches_recovered_candidate(&candidate)
                }
                ConcreteLifecycleWorkKind::DurableStoreBody(store) => {
                    store.project_candidate(verified).is_ok_and(|candidate| {
                        candidate_core_matches(&candidate)
                            && candidate
                                .physical_geometry
                                .normalized()
                                .is_ok_and(|(physical, _, _)| physical == record.physical_slots)
                    })
                }
                ConcreteLifecycleWorkKind::DurableValidateBody(validate) => {
                    validate.project_candidate(verified).is_ok_and(|candidate| {
                        candidate_core_matches(&candidate)
                            && candidate
                                .physical_geometry
                                .normalized()
                                .is_ok_and(|(physical, _, _)| physical == record.physical_slots)
                    })
                }
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                    record.state == super::LifecycleState::Ready
                        && completion
                            .incumbent
                            .project_candidate(verified)
                            .is_ok_and(|candidate| candidate_core_matches(&candidate))
                }
                ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) => {
                    apply.dispatch_key.is_none()
                        && apply.validates_in_ledger(&exact_ledger)
                        && apply.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableLiveWalSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.validates_in_ledger(&exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.repair.validates_in_ledger(&exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.validates_in_ledger(&exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.carrier.validates_in_ledger(verified, &exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) => {
                    broadcast.validates_in_ledger(&exact_ledger)
                        && broadcast.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                    fetch.carrier.validates_in_ledger(verified, &exact_ledger)
                        && match (fetch.dispatch_key, fetch.wait_source) {
                            (None, None) => {
                                fetch.matches_current_ready_record(address, digest, coordinator)
                            }
                            (Some(key), Some(source)) => {
                                key.matches(coordinator.active_context, address, digest)
                                    && fetch.matches_waiting_record(
                                        address,
                                        digest,
                                        coordinator,
                                        source,
                                    )
                            }
                            (None, Some(_)) | (Some(_), None) => false,
                        }
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store) => {
                    store.store.is_exact(verified)
                        && store.fetch.validates(verified)
                        && store
                            .fetch
                            .validates_recovered_store_in_ledger(&store.store, &exact_ledger)
                        && store.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => {
                    apply.dispatch_key.is_none()
                        && apply.carrier.validates_in_ledger(
                            verified,
                            &exact_ledger,
                            address.ordinal,
                        )
                        && apply.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => {
                    serve.matches_record(record, metadata, digest)
                }
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => active_producer
                    .map_or_else(
                        || producer.matches_record(record, metadata, digest),
                        |lease| {
                            if record.ordinal == lease.ordinal {
                                producer.matches_claimed_record(record, metadata, digest, lease)
                            } else {
                                producer.matches_record(record, metadata, digest)
                            }
                        },
                    ),
            }
        })
    }

}
