/// Closed internal policy for crossing a durable physical ingress barrier.
///
/// The ordinary selector preserves every barrier. The timeout-vote episode
/// variant exposes only a directly authenticated validator's exact productive
/// TimeoutVote to the downstream episode predicate while a selected Serve
/// occurrence or one bounded certified-response carrier owns the shared
/// physical turn. Response authority is acquired only after dequeue, so the
/// phase check deliberately does not assume a claim which cannot exist yet.
/// It neither borrows certified capacity nor admits the vote by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FairV2IngressBarrierBypass {
    /// Preserve every durable ingress barrier.
    None,
    /// Let the finite current-view TimeoutVote episode reach its predicate.
    TimeoutVoteEpisode,
}

/// Exact Certified-Serve authority consulted by one fair-ingress selector cut.
///
/// The handle is retained only to compare actor identity. Mutable queue state
/// is projected into the selected barrier, request cutoff, and predecessor
/// predicate while the fair-ingress state lock is held.
#[derive(Clone, Debug)]
struct FairV2IngressServeSelectorProjection {
    required: bool,
    gate: Option<v2_worker::CertifiedServeIngressGate>,
    selected_barrier: Option<v2_worker::CertifiedServeBarrier>,
    certified_body_request_cutoff: Option<u64>,
    selected_predecessors_cleared: bool,
}

impl PartialEq for FairV2IngressServeSelectorProjection {
    fn eq(&self, other: &Self) -> bool {
        let same_gate = match (&self.gate, &other.gate) {
            (Some(left), Some(right)) => left.ptr_eq(right),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        self.required == other.required
            && same_gate
            && self.selected_barrier == other.selected_barrier
            && self.certified_body_request_cutoff == other.certified_body_request_cutoff
            && self.selected_predecessors_cleared == other.selected_predecessors_cleared
    }
}

impl Eq for FairV2IngressServeSelectorProjection {}

fn fair_v2_ingress_serve_selector_projection(
    state: &FairV2IngressState,
    physical_cut: Option<u128>,
) -> Result<FairV2IngressServeSelectorProjection, String> {
    let gate = state.certified_serve_gate.clone();
    let live_selected_barrier = match gate.as_ref() {
        Some(gate) => gate.selected_barrier()?,
        None if state.requires_certified_serve_gate => {
            return Err("Serve selector crossed an unbound durable gate".to_owned());
        }
        None => None,
    };
    if let Some(barrier) = live_selected_barrier {
        let matching_carriers = state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .filter(|entry| {
                entry.admission_ordinal == barrier.carrier_ordinal()
                    && entry
                        .certified_serve_reservation
                        .as_ref()
                        .is_some_and(|reservation| reservation.matches_barrier(barrier))
                    // The reservation lifecycle id already binds the exact
                    // request hash. Retain only the payload-shape check here
                    // so final state-locked selector CAS stays hash-free.
                    && matches!(
                        entry.inbound.message(),
                        BlockMessage::V2(ConsensusMessageV2 {
                            payload: ConsensusMessageV2Payload::CertifiedBodyRequest(_),
                            ..
                        })
                    )
            })
            .count();
        if matching_carriers != 1 {
            return Err(
                "Serve selector changed its exact fair-ingress carrier identity".to_owned(),
            );
        }
    }
    let selected_barrier = live_selected_barrier.filter(|barrier| {
        physical_cut.is_none_or(|cut| u128::from(barrier.carrier_ordinal()) < cut)
    });
    let certified_body_request_cutoff = selected_barrier
        .is_none()
        .then(|| {
            state
                .lanes
                .values()
                .flat_map(|lane| lane.entries.iter())
                .filter(|entry| {
                    physical_cut.is_none_or(|cut| u128::from(entry.admission_ordinal) < cut)
                        && fair_v2_ingress_is_certified_body_request(&entry.inbound)
                        && (!state.requires_certified_serve_gate
                            || entry.certified_serve_reservation.is_some())
                })
                .map(|entry| entry.admission_ordinal)
                .min()
        })
        .flatten();
    let selected_predecessors_cleared = selected_barrier.is_none_or(|serve| {
        state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .all(|entry| entry.admission_ordinal >= serve.carrier_ordinal())
    });
    Ok(FairV2IngressServeSelectorProjection {
        required: state.requires_certified_serve_gate,
        gate,
        selected_barrier,
        certified_body_request_cutoff,
        selected_predecessors_cleared,
    })
}

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
    selected_serve_barrier: Option<v2_worker::CertifiedServeBarrier>,
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
    let (mut selected_barrier, selected_carrier_ordinal) = match active_carriers.first() {
        Some((owner, carrier_ordinal)) => (Some(owner.clone()), Some(*carrier_ordinal)),
        None => (None, None),
    };
    if selected_serve_barrier.is_some_and(|serve| {
        selected_carrier_ordinal
            .is_some_and(|leader_ordinal| serve.carrier_ordinal() <= leader_ordinal)
    }) {
        selected_barrier = None;
    }

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
                    | ConsensusMessageV2Payload::PayloadManifest(_)
                    | ConsensusMessageV2Payload::PayloadChunk(_)
                    | ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                    | ConsensusMessageV2Payload::CertifiedBodyResponse(_)
                    | ConsensusMessageV2Payload::CommitCertificateRequest(_)
                    | ConsensusMessageV2Payload::CommitCertificateResponse(_)
                    | ConsensusMessageV2Payload::VrfCommit(_)
                    | ConsensusMessageV2Payload::VrfReveal(_) => None,
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
    serve: &FairV2IngressServeSelectorProjection,
    leader: &FairV2IngressLeaderWireSelectorProjection,
    barrier_bypass: FairV2IngressBarrierBypass,
) -> FairV2IngressQueueGateVerdict {
    let entry = &lane.entries[index];
    let selected_serve_barrier = serve.selected_barrier;
    let certified_body_request_cutoff = serve.certified_body_request_cutoff;
    let selected_serve_predecessors_cleared = serve.selected_predecessors_cleared;
    let leader_wire_barrier = leader.selected_barrier.as_ref();
    let leader_wire_body_dependency = leader.body_dependency;
    let leader_wire_control_barrier = leader.control_barrier;

    // A control occurrence may wait for downstream capacity, but a later view
    // or conflicting carrier in the same semantic slot cannot replace it.
    let has_live_control_predecessor = lane
        .entries
        .iter()
        .take(index)
        .any(|prior| fair_v2_ingress_same_control_slot(&prior.inbound, &entry.inbound));
    let ingress_barrier_allows = if let Some(owner) = leader_wire_barrier {
        // A physically selected leader turn exclusively drains its immutable
        // ingress-prefix episode.
        index < owner.ingress_predecessors.get(source).copied().unwrap_or(0)
            || (owner.ingress_predecessors.values().all(|count| *count == 0)
                && entry.leader_wire_token.as_ref() == Some(&owner.token))
    } else if let Some(selected) = selected_serve_barrier {
        // The exact Serve target and its immutable earlier physical prefix
        // form one finite rank goal.
        entry.admission_ordinal < selected.carrier_ordinal()
            || (selected_serve_predecessors_cleared
                && entry.admission_ordinal == selected.carrier_ordinal()
                && entry
                    .certified_serve_reservation
                    .as_ref()
                    .is_some_and(|reservation| reservation.matches_barrier(selected))
                // `matches_barrier` binds the request through its lifecycle
                // id; do not re-hash a carrier under the queue-state lock.
                && matches!(
                    entry.inbound.message(),
                    BlockMessage::V2(ConsensusMessageV2 {
                        payload: ConsensusMessageV2Payload::CertifiedBodyRequest(_),
                        ..
                    })
                ))
    } else {
        certified_body_request_cutoff.is_none_or(|cutoff| entry.admission_ordinal <= cutoff)
    };
    let selected_serve_control_dependency =
        leader_wire_body_dependency.is_some_and(|(round, subject)| {
            selected_serve_barrier.is_some_and(|selected| {
                entry.admission_ordinal == selected.carrier_ordinal()
                    && entry
                        .certified_serve_reservation
                        .as_ref()
                        .is_some_and(|reservation| reservation.matches_barrier(selected))
                    // The reservation already binds the request hash. Only
                    // the dependency round/subject remains to compare here.
                    && matches!(
                        entry.inbound.message(),
                        BlockMessage::V2(ConsensusMessageV2 {
                            payload: ConsensusMessageV2Payload::CertifiedBodyRequest(request),
                            ..
                        }) if request.round == round
                            && request.subject == subject
                    )
            })
        });
    let earlier_dependency = selected_serve_barrier
        .is_none_or(|selected| entry.admission_ordinal < selected.carrier_ordinal())
        && (entry.class == FairV2IngressClass::TransportCompletion
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
            }));
    let timeout_control_dependency = leader_wire_barrier.is_some_and(|owner| {
        fair_v2_ingress_timeout_control_advances_owner(&owner.token, &entry.inbound)
    });
    let authenticated_certified_fence_escape = !matches!(source, FairV2IngressSource::Anonymous)
        && fair_v2_ingress_is_certified_fence_escape(&entry.inbound);
    let certified_fence_escape_dependency = authenticated_certified_fence_escape
        && leader_wire_barrier.is_some_and(|owner| {
            fair_v2_ingress_certified_fence_escape_advances_owner(&owner.token, &entry.inbound)
        });
    let serve_fence_escape_dependency = authenticated_certified_fence_escape
        && (selected_serve_barrier.is_some() || certified_body_request_cutoff.is_some());
    let timeout_vote_episode_dependency = barrier_bypass
        == FairV2IngressBarrierBypass::TimeoutVoteEpisode
        && fair_v2_ingress_is_direct_validator_timeout_vote_owner(source, entry)
        && (leader_wire_barrier.is_some_and(|owner| {
            owner.token.identity.phase == FairV2IngressLeaderWirePhase::CertifiedResponse
        }) || (leader_wire_barrier.is_none()
            && (selected_serve_barrier.is_some() || certified_body_request_cutoff.is_some())));
    let dependency_bypass = !ingress_barrier_allows
        && (serve_fence_escape_dependency
            || timeout_vote_episode_dependency
            || (leader_wire_control_barrier
                && (earlier_dependency
                    || selected_serve_control_dependency
                    || timeout_control_dependency
                    || certified_fence_escape_dependency)));
    if has_live_control_predecessor || (!ingress_barrier_allows && !dependency_bypass) {
        FairV2IngressQueueGateVerdict::Blocked
    } else if dependency_bypass {
        FairV2IngressQueueGateVerdict::Dependency
    } else {
        FairV2IngressQueueGateVerdict::Strict
    }
}
