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
    leader: &FairV2IngressLeaderWireSelectorProjection,
) -> FairV2IngressQueueGateVerdict {
    let entry = &lane.entries[index];
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
        fair_v2_ingress_timeout_control_advances_owner(&owner.token, &entry.inbound)
    });
    let authenticated_certified_fence_escape =
        fair_v2_ingress_is_certified_fence_escape(&entry.inbound);
    let certified_fence_escape_dependency = authenticated_certified_fence_escape
        && leader_wire_barrier.is_some_and(|owner| {
            fair_v2_ingress_certified_fence_escape_advances_owner(&owner.token, &entry.inbound)
        });
    let dependency_bypass = !ingress_barrier_allows
        && leader_wire_control_barrier
        && (earlier_dependency
            || timeout_control_dependency
            || certified_fence_escape_dependency);
    if has_live_control_predecessor || (!ingress_barrier_allows && !dependency_bypass) {
        FairV2IngressQueueGateVerdict::Blocked
    } else if dependency_bypass {
        FairV2IngressQueueGateVerdict::Dependency
    } else {
        FairV2IngressQueueGateVerdict::Strict
    }
}
