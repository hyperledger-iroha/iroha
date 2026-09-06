/// Exact leader-wire authority consulted by one fair-ingress selector cut.
#[derive(Clone, Debug)]
struct FairV2IngressLeaderWireSelectorProjection {
    required: bool,
    gate: Option<Arc<serviced_candidate_store::LeaderWireLifecycleStoreGate>>,
    durable_ingress_ordinals: BTreeSet<u128>,
    active_carriers: Vec<(FairV2IngressLeaderWireRecord, u64)>,
    obsolete_tokens: BTreeSet<FairV2IngressLeaderWireToken>,
    selected_barrier: Option<FairV2IngressLeaderWireRecord>,
    selected_carrier_ordinal: Option<u64>,
    body_dependency: Option<(ConsensusRound, BlockSubject)>,
    control_barrier: bool,
}
impl PartialEq for FairV2IngressLeaderWireSelectorProjection {
    fn eq(&self, other: &Self) -> bool {
        let same_gate = match (&self.gate, &other.gate) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        self.required == other.required
            && same_gate
            && self.durable_ingress_ordinals == other.durable_ingress_ordinals
            && self.active_carriers == other.active_carriers
            && self.obsolete_tokens == other.obsolete_tokens
            && self.selected_barrier == other.selected_barrier
            && self.selected_carrier_ordinal == other.selected_carrier_ordinal
            && self.body_dependency == other.body_dependency
            && self.control_barrier == other.control_barrier
    }
}
impl Eq for FairV2IngressLeaderWireSelectorProjection {}
fn fair_v2_ingress_leader_wire_selector_projection(
    state: &FairV2IngressState,
    observe_obsolete: bool,
    physical_cut: Option<u128>,
) -> Result<FairV2IngressLeaderWireSelectorProjection, String> {
    let gate = state.leader_wire_lifecycle_gate.clone();
    let active_leader_wire_owners = state
        .leader_wire_lifecycles
        .values()
        .filter(|record| record.status == FairV2IngressLeaderWireStatus::Ingress)
        .cloned()
        .collect::<Vec<_>>();
    if state.requires_leader_wire_lifecycle_gate {
        let gate = gate
            .as_ref()
            .ok_or_else(|| "leader-wire selector crossed an unbound durable gate".to_owned())?;
        let durable_ingress_ordinals = gate.ingress_scheduler_ordinals()?;
        let active_ordinals = active_leader_wire_owners
            .iter()
            .map(|record| record.token.scheduler_ordinal)
            .collect::<BTreeSet<_>>();
        if durable_ingress_ordinals != active_ordinals {
            return Err("leader-wire selector changed its durable Ingress owner set".to_owned());
        }
    }
    let mut carrier_ordinals = BTreeMap::new();
    for entry in state.lanes.values().flat_map(|lane| lane.entries.iter()) {
        let Some(token) = entry.leader_wire_token.as_ref() else {
            continue;
        };
        if carrier_ordinals
            .insert(token.clone(), entry.admission_ordinal)
            .is_some()
        {
            return Err(
                "leader-wire selector duplicated its exact fair-ingress carrier".to_owned(),
            );
        }
    }
    let mut active_carriers = Vec::with_capacity(active_leader_wire_owners.len());
    for owner in active_leader_wire_owners {
        let carrier_ordinal = carrier_ordinals
            .remove(&owner.token)
            .ok_or_else(|| "leader-wire selector lost its exact fair-ingress carrier".to_owned())?;
        // A restored token keeps its logical admission ordinal, so derive the
        // immutable prefix from its fresh physical carrier. Every dequeue must
        // reduce this exact per-source prefix before the carrier may cross.
        for (source, lane) in &state.lanes {
            let actual_predecessors = lane
                .entries
                .iter()
                .filter(|entry| entry.admission_ordinal < carrier_ordinal)
                .count();
            let retained_predecessors =
                owner.ingress_predecessors.get(source).copied().unwrap_or(0);
            if retained_predecessors != actual_predecessors {
                return Err(format!(
                    "leader-wire selector changed its exact ingress predecessor geometry for \
                     source class {:?}: retained {retained_predecessors}, actual \
                     {actual_predecessors}",
                    source.class(),
                ));
            }
        }
        // Authenticated non-validator lanes disappear after their final
        // dequeue. Their retained zero is equivalent to an absent map entry;
        // a positive count would name physical ownership that no longer exists.
        if let Some((source, retained_predecessors)) = owner
            .ingress_predecessors
            .iter()
            .find(|(source, count)| **count != 0 && !state.lanes.contains_key(*source))
        {
            return Err(format!(
                "leader-wire selector changed its exact ingress predecessor geometry for \
                 removed source class {:?}: retained {retained_predecessors}, actual 0",
                source.class(),
            ));
        }
        active_carriers.push((owner, carrier_ordinal));
    }
    if !carrier_ordinals.is_empty() {
        return Err("leader-wire carrier has no matching active lifecycle owner".to_owned());
    }
    active_carriers
        .retain(|(_, ordinal)| physical_cut.is_none_or(|cut| u128::from(*ordinal) < cut));
    active_carriers.sort_by_key(|(_, ordinal)| *ordinal);
    if active_carriers
        .windows(2)
        .any(|pair| pair[0].1 == pair[1].1)
    {
        return Err("leader-wire selector reused a physical carrier ordinal".to_owned());
    }
    let durable_ingress_ordinals = active_carriers
        .iter()
        .map(|(record, _)| record.token.scheduler_ordinal)
        .collect::<BTreeSet<_>>();
    let mut obsolete_tokens = BTreeSet::new();
    if observe_obsolete && let Some(gate) = gate.as_ref() {
        for (record, _) in &active_carriers {
            if gate.identity_is_obsolete(&record.token.identity)? {
                obsolete_tokens.insert(record.token.clone());
            }
        }
    }
    let (selected_barrier, selected_carrier_ordinal) = match active_carriers.first() {
        Some((owner, carrier_ordinal)) => (Some(owner.clone()), Some(*carrier_ordinal)),
        None => (None, None),
    };
    let body_dependency = selected_barrier.as_ref().and_then(|owner| {
        state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| entry.leader_wire_token.as_ref() == Some(&owner.token))
            .and_then(|entry| {
                let BlockMessage::V2(message) = entry.inbound.message() else {
                    return None;
                };
                match &message.payload {
                    ConsensusMessageV2Payload::Proposal(proposal) => {
                        Some((proposal.round, proposal.subject))
                    }
                    ConsensusMessageV2Payload::Vote(vote) => {
                        Some((vote.proposal_round, vote.subject))
                    }
                    ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                        Some((certificate.proposal_round, certificate.subject))
                    }
                    ConsensusMessageV2Payload::TimeoutVote(_)
                    | ConsensusMessageV2Payload::TimeoutCertificate(_)
                    | ConsensusMessageV2Payload::PayloadChunk(_)
                    | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                    | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
                    | ConsensusMessageV2Payload::CommitCertificateRequest(_)
                    | ConsensusMessageV2Payload::CommitCertificateResponse(_)
                    | ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => None,
                }
            })
    });
    let control_barrier = selected_barrier.as_ref().is_some_and(|owner| {
        owner.token.source_class == FairV2IngressLeaderWireSourceClass::Control
    });
    Ok(FairV2IngressLeaderWireSelectorProjection {
        required: state.requires_leader_wire_lifecycle_gate,
        gate,
        durable_ingress_ordinals,
        active_carriers,
        obsolete_tokens,
        selected_barrier,
        selected_carrier_ordinal,
        body_dependency,
        control_barrier,
    })
}
/// Queue-local eligibility of one exact fair-ingress occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FairV2IngressQueueGateVerdict {
    Blocked,
    Strict,
    Dependency,
}
fn fair_v2_ingress_queue_gate_verdict(
    source: &FairV2IngressSource,
    lane: &FairV2IngressLane,
    index: usize,
    leader: &FairV2IngressLeaderWireSelectorProjection,
) -> FairV2IngressQueueGateVerdict {
    let entry = &lane.entries[index];
    let leader_wire_barrier = leader.selected_barrier.as_ref();
    let leader_wire_body_dependency = leader.body_dependency;
    let leader_wire_control_barrier = leader.control_barrier;
    let leader_wire_chunk_barrier = leader_wire_barrier
        .is_some_and(|owner| owner.token.source_class == FairV2IngressLeaderWireSourceClass::Chunk);
    // A control occurrence may wait for downstream capacity, but a later view
    // or conflicting carrier in the same semantic slot cannot replace it.
    let has_live_control_predecessor = lane
        .entries
        .iter()
        .take(index)
        .any(|prior| fair_v2_ingress_same_control_slot(&prior.inbound, &entry.inbound));
    let ingress_barrier_allows = leader_wire_barrier.is_none_or(|owner| {
        // A physically selected leader turn exclusively drains its immutable
        // ingress-prefix episode.
        index < owner.ingress_predecessors.get(source).copied().unwrap_or(0)
            || (owner.ingress_predecessors.values().all(|count| *count == 0)
                && entry.leader_wire_token.as_ref() == Some(&owner.token))
    });
    let earlier_dependency = entry.class == FairV2IngressClass::TransportCompletion
        || leader_wire_body_dependency.is_some_and(|(round, subject)| {
            leader_wire_barrier
                .is_some_and(|owner| entry.leader_wire_token.as_ref() != Some(&owner.token))
                && matches!(
                    entry.inbound.message(),
                    BlockMessage::V2(ConsensusMessageV2 {
                        payload: ConsensusMessageV2Payload::Proposal(proposal),
                        ..
                    }) if proposal.round == round && proposal.subject == subject
                )
        });
    let timeout_control_dependency = leader_wire_barrier.is_some_and(|owner| {
        fair_v2_ingress_timeout_control_advances_owner(
            &owner.token,
            entry.leader_wire_token.as_ref(),
            &entry.inbound,
        )
    });
    let authenticated_certified_fence_escape =
        fair_v2_ingress_is_certified_fence_escape(&entry.inbound);
    let certified_fence_escape_dependency = authenticated_certified_fence_escape
        && leader_wire_barrier.is_some_and(|owner| {
            fair_v2_ingress_certified_fence_escape_advances_owner(&owner.token, &entry.inbound)
        });
    // A blocked local owner must not hide the authenticated historical request
    // which lets another replica recover that owner's predecessor. The strict
    // pass still wins whenever the owner itself is drainable; this dependency
    // neither retires the owner nor spends certified-fence capacity.
    let historical_replica_release_dependency = leader_wire_barrier.is_some_and(|owner| {
        entry.history_serve_request.is_some_and(|request| {
            let height = request.height();
            height != 0 && height < owner.token.identity.height
        }) && entry.inbound.reply_routes().is_some_and(|routes| {
            !routes.is_empty()
                && routes.semantic_target() == entry.inbound.sender()
                && routes.iter().any(NetworkReplyRoute::is_reply_writable)
        })
    });
    let dependency_bypass = !ingress_barrier_allows
        && ((leader_wire_control_barrier && earlier_dependency)
            || timeout_control_dependency
            || ((leader_wire_control_barrier || leader_wire_chunk_barrier)
                && (certified_fence_escape_dependency || historical_replica_release_dependency)));
    if has_live_control_predecessor || (!ingress_barrier_allows && !dependency_bypass) {
        FairV2IngressQueueGateVerdict::Blocked
    } else if dependency_bypass {
        FairV2IngressQueueGateVerdict::Dependency
    } else {
        FairV2IngressQueueGateVerdict::Strict
    }
}

impl FairV2Ingress {
    /// Render the private selector geometry for a rate-limited starvation
    /// warning. The snapshot is read-only and never advances the service
    /// clock, rotates a source, or observes durable obsolescence.
    pub(crate) fn scheduler_stall_diagnostic(&self) -> Result<String, String> {
        let state = self.state.lock();
        let projection = fair_v2_ingress_leader_wire_selector_projection(&state, false, None)?;
        let mut message_counts = BTreeMap::<u8, (FairV2IngressMessageKind, usize)>::new();
        let mut blocked = 0usize;
        let mut strict = 0usize;
        let mut dependency = 0usize;
        for (source, lane) in &state.lanes {
            for (index, entry) in lane.entries.iter().enumerate() {
                if let Some(kind) = FairV2IngressMessageKind::classify(entry.inbound.message()) {
                    message_counts
                        .entry(kind.projection_code())
                        .and_modify(|(_, count)| *count = count.saturating_add(1))
                        .or_insert((kind, 1));
                }
                match fair_v2_ingress_queue_gate_verdict(source, lane, index, &projection) {
                    FairV2IngressQueueGateVerdict::Blocked => {
                        blocked = blocked.saturating_add(1);
                    }
                    FairV2IngressQueueGateVerdict::Strict => {
                        strict = strict.saturating_add(1);
                    }
                    FairV2IngressQueueGateVerdict::Dependency => {
                        dependency = dependency.saturating_add(1);
                    }
                }
            }
        }
        let message_counts = message_counts
            .into_values()
            .collect::<Vec<(FairV2IngressMessageKind, usize)>>();
        let selected_barrier = projection.selected_barrier.as_ref().map(|record| {
            let predecessor_total = record.ingress_predecessors.values().sum::<usize>();
            let predecessor_source_count = record
                .ingress_predecessors
                .values()
                .filter(|count| **count != 0)
                .count();
            format!(
                "source_class={:?}, phase={:?}, view={}, scheduler_ordinal={}, carrier_ordinal={:?}, predecessor_total={}, predecessor_source_count={}",
                record.token.source_class,
                record.token.identity.phase,
                record.token.identity.view,
                record.token.scheduler_ordinal,
                projection.selected_carrier_ordinal,
                predecessor_total,
                predecessor_source_count,
            )
        });
        let current_physical_cut = u128::from(state.last_admission_ordinal).saturating_add(1);
        let current_depth = state.len;
        let current = format!(
            "depth={}, ready_sources={}, active_leader_carriers={}, selected_barrier={selected_barrier:?}, gate_blocked={blocked}, gate_strict={strict}, gate_dependency={dependency}, message_counts={message_counts:?}",
            state.len,
            state.ready.len(),
            projection.active_carriers.len(),
        );
        drop(state);
        let now = Instant::now();
        let last_attempt = *self.last_selector_attempt.lock();
        let last_attempt = last_attempt.map(|attempt| {
            let fresh = attempt.physical_cut == current_physical_cut
                && attempt.depth == current_depth;
            format!(
                "age={:?}, fresh={fresh}, physical_cut={}, depth={}, gate_blocked={}, gate_strict={}, gate_dependency={}, predicate_tested={}, predicate_rejected={}, selected={}",
                now.saturating_duration_since(attempt.observed_at),
                attempt.physical_cut,
                attempt.depth,
                attempt.gate_blocked,
                attempt.gate_strict,
                attempt.gate_dependency,
                attempt.predicate_tested,
                attempt.predicate_rejected,
                attempt.selected,
            )
        });
        Ok(format!("{current}, last_selector_attempt={last_attempt:?}"))
    }
}
