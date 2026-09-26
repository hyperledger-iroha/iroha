//! Deterministic carrier metadata preparation within the State owner.
//!
//! This private continuation stages candidate-derived values only. Its caller
//! retains source/context admission, finality, event delivery and publication
//! authority; the method neither creates nor consumes any publication guard.
//! TODO: compose this tail with complete prepared State and predecessor ownership
//! before exposing a candidate commitment or allowing native publication.

use super::*;

impl StateBlock<'_> {
    /// Stage the deterministic metadata selected by the caller's checked context.
    ///
    /// Authenticated candidate preparation and admitted application share this
    /// continuation. Raw topology values are not authentication; the caller keeps
    /// its validated frozen-context owner throughout preparation and publication.
    #[allow(clippy::too_many_lines)]
    pub(super) fn prepare_deterministic_carrier_metadata(
        &mut self,
        signed_block: &SignedBlock,
        topology: Vec<PeerId>,
        topology_authority: ApplyTopologyAuthority,
    ) -> Result<(), MergeLedgerCommitError> {
        let invalid = |reason: &str| {
            MergeLedgerCommitError::ExecutionBatchInvalid(format!(
                "carrier metadata preparation: {reason}"
            ))
        };
        let block_hash = signed_block.hash();
        if signed_block.header() != self._curr_block {
            return Err(invalid("proposal differs from the executed State scope"));
        }
        let block_height: NonZeroUsize = signed_block
            .header()
            .height()
            .try_into()
            .map_err(|_| invalid("block height exceeds usize"))?;
        if self.block_hashes.len().checked_add(1) != Some(block_height.get())
            || signed_block.header().prev_block_hash() != self.block_hashes.last().copied()
        {
            return Err(invalid(
                "proposal does not extend its exact State predecessor",
            ));
        }
        validate_applied_npos_consensus_effects(
            signed_block.npos_consensus_effects(),
            self.applied_npos_consensus_effects_hash.as_ref(),
        )?;
        crate::bridge::validate_sccp_commitment_root_for_signed_block(signed_block)
            .map_err(|error| invalid(&format!("SCCP commitment: {error:?}")))?;
        let snapshot = signed_block
            .axt_policy_snapshot()
            .ok_or_else(|| invalid("missing AXT policy snapshot"))?;
        snapshot
            .validate()
            .map_err(|error| invalid(&format!("AXT policy snapshot: {error}")))?;
        let committed_fragment_count = signed_block
            .committed_fragment_count()
            .ok_or_else(|| invalid("missing committed fragment count"))?;
        let transitions = signed_block
            .axt_transitioned_dataspaces()
            .ok_or_else(|| invalid("missing AXT transition set"))?;
        self.stage_prepaid_ordinary_carrier_membership(signed_block, block_height)?;
        if self
            .canonical_wsv_merge_commit_authorization
            .as_ref()
            .is_some_and(|authorization| authorization.beacon_composition.is_some())
        {
            self.validate_staged_merge_execution_authorization()?;
        }
        if let Some(bundle) = signed_block.da_commitments() {
            let height = signed_block.header().height().get();
            self.pending_da_commitments = Some(PendingDaCommitmentBundle {
                block_height: height,
                bundle: bundle.clone(),
            });
        }
        if let Some(bundle) = signed_block.da_pin_intents() {
            let height = signed_block.header().height().get();
            self.stage_da_pin_intent_bundle(height, bundle.intents.clone())?;
        }
        let current_slot =
            current_axt_slot_from_block(&signed_block.header(), self.nexus.axt.slot_length_ms);
        if let Some(envelopes) = signed_block.axt_envelopes() {
            if !envelopes.is_empty() {
                iroha_logger::trace!(
                    count = envelopes.len(),
                    current_slot,
                    "persisting AXT envelopes from committed block"
                );
                self.apply_replayed_axt_envelopes(envelopes, current_slot);
            }
        }
        self.block_hashes.push(block_hash);
        self.stage_musubi_resolver_index_checkpoint(
            signed_block.header().height().get(),
            block_hash,
        )?;
        // Merge metadata belongs to the captured World version. Certified
        // merge and native stages already update that overlay through their
        // owners; an ordinary carrier retains its actual MV predecessor.
        // The rolling merge cache may be cold, stale, or ahead during recovery
        // and cannot replace these values at the metadata boundary.
        let prev_topology = self.commit_topology.take_vec();
        let prev_topology_for_derivation = prev_topology.clone();
        self.prev_commit_topology
            .mutate_vec(|vec| *vec = prev_topology);
        let checkpoint_topology = topology;
        #[cfg(not(any(test, feature = "iroha-core-tests")))]
        let _ = topology_authority;
        #[cfg(any(test, feature = "iroha-core-tests"))]
        let checkpoint_block_height = signed_block.header().height().get();
        #[cfg(any(test, feature = "iroha-core-tests"))]
        let topology_source = match &topology_authority {
            ApplyTopologyAuthority::V2Finality => checkpoint_topology.clone(),
            ApplyTopologyAuthority::Fixture => {
                let mut world_peers: Vec<PeerId> = self.world.peers().iter().cloned().collect();
                let active_lane_ids = nexus_active_lane_ids(&self.nexus);
                let checkpoint_lane_ids = if checkpoint_topology.is_empty() {
                    BTreeSet::new()
                } else {
                    validator_lane_ids_for_peers(
                        &self.world,
                        checkpoint_topology.iter(),
                        checkpoint_block_height,
                    )
                    .into_iter()
                    .filter(|lane_id| active_lane_ids.contains(lane_id))
                    .collect()
                };
                if !checkpoint_lane_ids.is_empty() {
                    let before = world_peers.len();
                    world_peers.retain(|peer| {
                        checkpoint_topology.contains(peer)
                            || !validator_lane_ids_for_peer(
                                &self.world,
                                peer,
                                checkpoint_block_height,
                            )
                            .is_disjoint(&checkpoint_lane_ids)
                    });
                    let filtered = before.saturating_sub(world_peers.len());
                    if filtered > 0 {
                        warn!(
                            height = block_height,
                            block = %block_hash,
                            filtered,
                            lanes = checkpoint_lane_ids.len(),
                            "ignoring world peers outside checkpoint topology lanes during fixture reconciliation"
                        );
                    }
                }
                let npos_mode_active = self.world.sumeragi_npos_parameters().is_some();
                if checkpoint_topology.is_empty() {
                    if world_peers.is_empty() {
                        Vec::new()
                    } else {
                        world_peers.sort();
                        world_peers
                    }
                } else if world_peers.is_empty() {
                    checkpoint_topology.clone()
                } else {
                    let checkpoint_set: BTreeSet<_> = checkpoint_topology.iter().cloned().collect();
                    let mut missing: Vec<_> = world_peers
                        .into_iter()
                        .filter(|peer| !checkpoint_set.contains(peer))
                        .collect();
                    if !missing.is_empty() && npos_mode_active {
                        missing.sort();
                        let missing_active_validators =
                            active_stake_elected_validator_peers_for_checkpoint_lanes(
                                &self.world,
                                missing.iter(),
                                &checkpoint_lane_ids,
                                checkpoint_block_height,
                                &self.nexus,
                            );
                        if missing_active_validators.is_empty() {
                            checkpoint_topology.clone()
                        } else {
                            let mut combined = checkpoint_topology.clone();
                            combined.extend(missing_active_validators);
                            combined
                        }
                    } else {
                        missing.sort();
                        let mut combined = checkpoint_topology.clone();
                        combined.extend(missing);
                        combined
                    }
                }
            }
        };
        #[cfg(not(any(test, feature = "iroha-core-tests")))]
        let topology_source = checkpoint_topology.clone();
        let next_topology = if topology_source.is_empty() {
            Vec::new()
        } else {
            // Always derive commit topology from a deterministic source.
            let rotation_base = if !prev_topology_for_derivation.is_empty() {
                prev_topology_for_derivation
            } else if !checkpoint_topology.is_empty() {
                checkpoint_topology.clone()
            } else {
                topology_source.clone()
            };
            let mut topo = crate::sumeragi::network_topology::Topology::new(rotation_base);
            topo.block_committed(topology_source, block_hash);
            topo.as_ref().to_vec()
        };
        self.commit_topology.mutate_vec(|vec| *vec = next_topology);
        self.stage_native_amx_participant_frontiers(signed_block)?;
        self.evaluate_nexus_autoscale(signed_block, committed_fragment_count)
            .map_err(|error| match error {
                LaneLifecycleError::DrainObservation(error) => {
                    MergeLedgerCommitError::LocalDrainObservation(Box::new(error))
                }
                LaneLifecycleError::GeometryStorage(error) => {
                    MergeLedgerCommitError::Persistence(error)
                }
                local @ (LaneLifecycleError::Storage(_)
                | LaneLifecycleError::PublicationBusy { .. }) => {
                    MergeLedgerCommitError::LocalDrainObservation(Box::new(
                        MergeLedgerCommitError::ExecutionMarkerConflict(local.to_string()),
                    ))
                }
                other => invalid(&format!("autoscale inputs: {other}")),
            })?;
        self.axt_authorization_transitioned = transitions.clone();
        self.replace_axt_policy_projection(snapshot);
        self.finalize_axt_policy_transition_ratchets()
            .map_err(|error| invalid(&format!("AXT counter ratchets: {error}")))?;
        self.install_axt_policy_snapshot(snapshot)
            .map_err(|error| invalid(&format!("AXT policy projection: {error}")))?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "carrier_metadata_preparation_tests.rs"]
mod tests;
