//! Exact State-derived committee methods for the lane-work adapter.

use super::*;

impl V2LaneWorkAdapter {
    pub(super) fn expected_lane_validators(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        proposal_height: u64,
    ) -> Option<Vec<PeerId>> {
        if proposal_height != self.context.height {
            return None;
        }
        self.state
            .resolve_lane_committee_at_height(
                crate::state::LaneAuthorityRoute::new(lane_id, dataspace_id),
                proposal_height,
            )
            .ok()
            .map(crate::state::LaneAuthorityCommittee::into_validators)
    }

    pub(super) fn native_committee_shape_for_route(
        &self,
        participant_lane: LaneId,
        participant_dataspace: DataSpaceId,
        authority_height: u64,
    ) -> Option<(Vec<PeerId>, usize)> {
        let validators = self.expected_lane_validators(
            participant_lane,
            participant_dataspace,
            authority_height,
        )?;
        if validators.is_empty()
            || validators.len() > crate::native_amx::MAX_NATIVE_AMX_VALIDATORS
            || validators.windows(2).any(|pair| pair[0] >= pair[1])
            || validators
                .iter()
                .any(|peer| peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal))
        {
            return None;
        }
        // The typed State resolver is the sole geometry authority and has already
        // enforced the exact dataspace `3f+1` size in the same immutable view.
        let min_signers =
            crate::sumeragi::network_topology::commit_quorum_from_len(validators.len()).max(1);
        Some((validators, min_signers))
    }

    pub(super) fn native_committee_for_route(
        &self,
        participant_lane: LaneId,
        participant_dataspace: DataSpaceId,
        authority_height: u64,
    ) -> Option<(
        Vec<PeerId>,
        usize,
        BTreeMap<PublicKey, Vec<u8>>,
        Vec<Vec<u8>>,
    )> {
        let (validators, min_signers) = self.native_committee_shape_for_route(
            participant_lane,
            participant_dataspace,
            authority_height,
        )?;
        let pinned =
            pinned_autoscale_validator_pops_for_set(&self.state, participant_lane, &validators)?;
        let aligned_pops = if let Some(pops) = pinned {
            pops
        } else {
            let world = self.state.world_view();
            validators
                .iter()
                .map(|peer| {
                    let pop = self.consensus_pop_for_peer_at_height(
                        &world,
                        participant_lane,
                        peer,
                        authority_height,
                    )?;
                    iroha_crypto::bls_normal_pop_verify(peer.public_key(), &pop).ok()?;
                    Some(pop)
                })
                .collect::<Option<Vec<_>>>()?
        };
        let pops = verified_native_committee_pops(&validators, &aligned_pops)?;
        Some((validators, min_signers, pops, aligned_pops))
    }
}
