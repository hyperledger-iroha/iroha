//! Carrier-owned opening, closure and witness sealing of lane consensus instances.

use super::lane_consensus_commitment::LaneConsensusContextsCommitmentV1;
use super::*;
use iroha_data_model::block::consensus_v2 as wire;

/// Fixed synthetic write authenticating the complete set, including absence.
pub(crate) const LANE_CONSENSUS_CONTEXTS_WITNESS_KEY: &[u8] =
    b"iroha:sumeragi:open-lane-contexts:v1";

/// Check the final metadata against canonical pending work and applied frontiers.
/// This is also used when restoring the authenticated full snapshot.
pub(super) fn validate_committed_lane_consensus_contexts(
    contexts: &LaneConsensusContextsV1,
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    incarnations: &BTreeMap<LaneId, Hash>,
    network_id: iroha_data_model::NetworkId,
    height: u64,
) -> Result<(), String> {
    contexts.validate().map_err(|error| error.to_string())?;
    let mut observed = 0;
    for lane in nexus.lane_catalog.lanes() {
        let Some(incarnation) = incarnations.get(&lane.id).copied() else {
            continue;
        };
        let route = QueuePlanPendingObligationRouteV1 {
            version: QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1,
            lane_id: lane.id,
            dataspace_id: lane.dataspace_id,
            lane_incarnation: incarnation,
        };
        let pending = State::queue_plan_pending_route_obligation_count_from_world(world, route)
            .map_err(|error| error.to_string())?;
        let context = contexts.contexts.iter().find(|context| {
            (
                context.lane_id,
                context.dataspace_id,
                context.lane_incarnation,
            ) == (lane.id, lane.dataspace_id, incarnation)
        });
        match (pending > 0, context) {
            (false, None) => {}
            (true, Some(context)) => {
                let head = State::queue_plan_pending_route_at_admission_cut_from_storage(
                    world.smart_contract_state(),
                    &network_id,
                    route,
                    height,
                    height,
                )?
                .into_iter()
                .next()
                .ok_or_else(|| "open lane context has no exact admitted head".to_owned())?;
                let frontier = State::canonical_merged_lane_frontier_with_anchor_from_world(
                    world,
                    lane.id,
                    lane.dataspace_id,
                    incarnation,
                )
                .map_err(|error| error.to_string())?;
                if context.network_id != network_id
                    || context.opening_global_height > height
                    || context.admitted_binding_hash != head.binding.canonical_hash()
                    || context.admission_priority != head.priority
                    || frontier
                        != (
                            context.predecessor_height,
                            context.predecessor_hash,
                            context.predecessor_applied_global_height,
                        )
                {
                    return Err(
                        "lane context does not bind its committed application frontier".to_owned(),
                    );
                }
                observed += 1;
            }
            _ => {
                return Err(
                    "open lane context set differs from canonical pending obligations".to_owned(),
                );
            }
        }
    }
    if observed != contexts.contexts.len() {
        return Err("open lane context belongs to an absent or replaced route".to_owned());
    }
    Ok(())
}

impl StateBlock<'_> {
    /// Reconcile lane instances after execution and before capturing its witness.
    ///
    /// Only globally admitted pending work opens an instance. A later global
    /// height does not change an existing instance's committee, policy or anchor.
    /// Native participant frontiers are projected from the already validated
    /// carrier; their eventual metadata publication must match this projection.
    pub(crate) fn finalize_lane_consensus_contexts(
        &mut self,
        block: &SignedBlock,
        opening: Option<&wire::HeightContext>,
    ) -> Result<(), String> {
        if self.lane_consensus_contexts_seal.is_some() {
            return Err("lane consensus contexts were already sealed".to_owned());
        }
        let height = self._curr_block.height().get();
        if block.header().height().get() != height {
            return Err("lane context carrier height differs from its execution".to_owned());
        }
        if let Some(opening) = opening {
            opening.validate().map_err(|error| error.to_string())?;
            if opening.height != height || opening.network_id != self.network_id {
                return Err("lane opening authority belongs to another carrier".to_owned());
            }
        }
        self.lane_consensus_contexts
            .get()
            .validate()
            .map_err(|error| error.to_string())?;
        let native = State::native_amx_participant_frontier_markers_and_merge_entry(
            block,
            self.staged_merge_entry(),
        )
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|marker| {
            (
                (marker.lane_id, marker.dataspace_id, marker.lane_incarnation),
                (
                    marker.lane_block_height,
                    Some(marker.lane_block_descriptor_hash),
                    height,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
        let previous = self
            .lane_consensus_contexts
            .get()
            .contexts
            .iter()
            .map(|context| {
                (
                    (
                        context.lane_id,
                        context.dataspace_id,
                        context.lane_incarnation,
                    ),
                    context,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut next = BTreeMap::new();
        for lane in self.nexus.lane_catalog.lanes() {
            // Closing ingress does not revoke an already opened instance. Its
            // admitted work remains an obligation until the carrier settles it.
            let Some(incarnation) = self.lane_incarnations.get(&lane.id).copied() else {
                continue;
            };
            let route = (lane.id, lane.dataspace_id, incarnation);
            let obligation_route = QueuePlanPendingObligationRouteV1 {
                version: QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1,
                lane_id: lane.id,
                dataspace_id: lane.dataspace_id,
                lane_incarnation: incarnation,
            };
            let pending = State::queue_plan_pending_route_obligation_count_from_world(
                &self.world,
                obligation_route,
            )
            .map_err(|error| error.to_string())?;
            // A globally decided terminal resolution closes an empty instance.
            // Its absence is authenticated by the complete-set witness below.
            if pending == 0 {
                continue;
            }
            let head = State::queue_plan_pending_route_head_at_admission_cut(
                self,
                lane.id,
                lane.dataspace_id,
                incarnation,
                height,
                height,
            )?
            .ok_or_else(|| "pending lane work has no exact admitted head".to_owned())?;
            let staged = State::canonical_merged_lane_frontier_with_anchor_from_world(
                &self.world,
                lane.id,
                lane.dataspace_id,
                incarnation,
            )
            .map_err(|error| error.to_string())?;
            let frontier = native.get(&route).copied().unwrap_or(staged);
            if frontier.0 < staged.0 || (frontier.0 == staged.0 && frontier != staged) {
                return Err("Native lane frontier conflicts with staged application".to_owned());
            }
            if let Some(current) = previous.get(&route) {
                if current.network_id != self.network_id {
                    return Err("retained lane context belongs to another network".to_owned());
                }
                if frontier
                    == (
                        current.predecessor_height,
                        current.predecessor_hash,
                        current.predecessor_applied_global_height,
                    )
                    && current.admitted_binding_hash == head.binding.canonical_hash()
                    && current.admission_priority == head.priority
                {
                    next.insert(route, (*current).clone());
                    continue;
                }
                if frontier.0 < current.predecessor_height
                    || (frontier.0 == current.predecessor_height
                        && frontier
                            != (
                                current.predecessor_height,
                                current.predecessor_hash,
                                current.predecessor_applied_global_height,
                            ))
                {
                    return Err(
                        "retained lane context predecessor changed without application".to_owned(),
                    );
                }
            }
            let opening = opening.ok_or_else(|| {
                "pending lane work requires authenticated carrier context before opening".to_owned()
            })?;
            let (committee, validator_set_pops) =
                lane_consensus_authority::resolve_open_lane_authority(
                    self,
                    lane,
                    incarnation,
                    height,
                )?;
            let context = FrozenLaneConsensusContextV1 {
                network_id: self.network_id,
                protocol_version: opening.protocol_version,
                opening_global_height: height,
                opening_global_context_id: opening.id(),
                admitted_binding_hash: head.binding.canonical_hash(),
                admission_priority: head.priority,
                epoch: opening.epoch,
                mode: opening.mode,
                lane_id: lane.id,
                dataspace_id: lane.dataspace_id,
                lane_incarnation: incarnation,
                next_lane_height: frontier
                    .0
                    .checked_add(1)
                    .ok_or_else(|| "lane height exhausted".to_owned())?,
                predecessor_height: frontier.0,
                predecessor_hash: frontier.1,
                predecessor_applied_global_height: frontier.2,
                committee,
                validator_set_pops,
                nexus_amx_context_hash: opening.nexus_amx_context_hash,
                execution_policy_hash: opening.execution_policy_hash,
                da_layout: opening.da_layout,
                leader_seed: opening.leader_seed,
            };
            context.validate().map_err(|error| error.to_string())?;
            next.insert(route, context);
        }
        let next = LaneConsensusContextsV1::new(next.into_values().collect())
            .map_err(|error| error.to_string())?;
        *self.lane_consensus_contexts.get_mut() = next;
        Ok(())
    }

    /// Bind one canonical complete-set value and retain its immutable seal.
    pub(super) fn capture_lane_consensus_contexts(
        &mut self,
        witness: &mut ExecWitness,
    ) -> Result<(), String> {
        let snapshot = self.lane_consensus_contexts.get();
        let hash = snapshot
            .canonical_hash()
            .map_err(|error| error.to_string())?;
        self.verify_lane_consensus_contexts_seal()?;
        let commitment = LaneConsensusContextsCommitmentV1::from_contexts(
            self.network_id,
            self._curr_block.height().get(),
            snapshot,
        )?;
        let value = norito::to_bytes(&commitment).map_err(|error| error.to_string())?;
        witness
            .writes
            .retain(|entry| entry.key != LANE_CONSENSUS_CONTEXTS_WITNESS_KEY);
        witness.writes.push(ExecKv {
            key: LANE_CONSENSUS_CONTEXTS_WITNESS_KEY.to_vec(),
            value,
        });
        self.lane_consensus_contexts_seal = Some(hash);
        Ok(())
    }

    /// Output extraction and publication must retain the exact captured set.
    pub(super) fn verify_lane_consensus_contexts_seal(&self) -> Result<(), String> {
        let Some(seal) = self.lane_consensus_contexts_seal else {
            return Ok(());
        };
        if self
            .lane_consensus_contexts
            .get()
            .canonical_hash()
            .map_err(|error| error.to_string())?
            != seal
        {
            return Err("lane consensus context set changed after witness capture".to_owned());
        }
        Ok(())
    }

    /// Reject mutation, removal or duplication of the synthetic witness write.
    pub(super) fn verify_lane_consensus_contexts_witness(
        &self,
        witness: &ExecWitness,
    ) -> Result<(), String> {
        self.verify_lane_consensus_contexts_seal()?;
        let commitment = LaneConsensusContextsCommitmentV1::from_contexts(
            self.network_id,
            self._curr_block.height().get(),
            self.lane_consensus_contexts.get(),
        )?;
        let expected = norito::to_bytes(&commitment).map_err(|error| error.to_string())?;
        let mut entries = witness
            .writes
            .iter()
            .filter(|entry| entry.key == LANE_CONSENSUS_CONTEXTS_WITNESS_KEY);
        if !entries.next().is_some_and(|entry| entry.value == expected) || entries.next().is_some()
        {
            return Err(
                "execution witness does not contain the exact lane context snapshot".to_owned(),
            );
        }
        Ok(())
    }

    /// Finality metadata publication must realize the frontiers sealed before it.
    pub(super) fn verify_lane_consensus_contexts_publication(&self) -> Result<(), String> {
        self.verify_lane_consensus_contexts_seal()?;
        if self.lane_consensus_contexts_seal.is_some() {
            validate_committed_lane_consensus_contexts(
                self.lane_consensus_contexts.get(),
                &self.world,
                &self.nexus,
                &self.lane_incarnations,
                self.network_id,
                self._curr_block.height().get(),
            )?;
        }
        Ok(())
    }
}
