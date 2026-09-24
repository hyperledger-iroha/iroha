//! Replicated lane-drain frontier checks for live and replayed carriers.

use super::*;

impl State {
    /// Check an exact historical drain carrier before its WSV height is replayed.
    /// Recovery already authenticated this carrier against retained global
    /// finality. The ordered replay path separately enforces its WSV intent and
    /// frontier; later node-local evidence cannot rewrite historical admission.
    pub(super) fn validate_historical_merge_lane_drain_certificate(
        &self,
        authority: HistoricalMergeDrainAuthority<'_>,
    ) -> Result<(), MergeLedgerCommitError> {
        let entry = authority.entry;
        if authority.carrier.version != 1
            || authority.carrier.epoch_id != entry.epoch_id
            || authority.carrier.entry_hash != entry.canonical_hash()
            || authority.carrier.block_height != entry.merge_qc.carrier_height
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "historical drain entry differs from its exact authenticated carrier".to_owned(),
            ));
        }
        if entry.lane_drain_certificates.is_empty() {
            return Ok(());
        }
        if entry.execution_batch.is_some() || !entry.lane_snapshots.is_empty() {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "historical drain carrier must contain only its exact certificate".to_owned(),
            ));
        }
        self.validate_merge_lane_drain_certificate_structure(
            &entry.lane_drain_certificates,
            entry.merge_qc.carrier_height,
            &entry.active_lanes,
        )
    }

    pub(super) fn validate_merge_lane_drain_certificate_structure(
        &self,
        certificates: &[LaneDrainCertificateV1],
        carrier_height: u64,
        active_lanes: &[MergeLaneBinding],
    ) -> Result<(), MergeLedgerCommitError> {
        if certificates.len() != 1 {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "a merge entry may carry exactly one lane drain certificate".to_owned(),
            ));
        }
        let certificate = &certificates[0];
        crate::lane_consensus::validate_lane_drain_certificate(certificate).map_err(|err| {
            MergeLedgerCommitError::ExecutionBatchInvalid(format!(
                "lane drain certificate is invalid: {err}"
            ))
        })?;
        let intent = &certificate.body.intent;
        if intent.network_id != self.network_id || carrier_height <= intent.close_global_height {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "lane drain certificate has the wrong network or an invalid carrier height"
                    .to_owned(),
            ));
        }
        if !active_lanes.iter().any(|binding| {
            binding.lane_id == intent.lane_id
                && binding.dataspace_id == intent.dataspace_id
                && binding.incarnation == intent.lane_incarnation
        }) {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "lane drain certificate does not name an exact active lane binding".to_owned(),
            ));
        }
        Ok(())
    }

    /// Reconstruct the drain frontier from the committed WSV prefix. Callers
    /// first authenticate `certified` against the incoming vote or carrier.
    /// Evidence hashes absent from WSV remain bound by that signed certificate;
    /// current node-local receipts and pending work are never voting authority.
    pub(super) fn lane_drain_frontier_from_committed_state(
        state: &impl StateReadOnly,
        certified: LaneDrainFrontierV1,
    ) -> Result<LaneDrainFrontierV1, MergeLedgerCommitError> {
        let invalid =
            |message: &str| MergeLedgerCommitError::ExecutionMarkerConflict(message.to_owned());
        crate::lane_consensus::validate_lane_drain_frontier(&certified)
            .map_err(|error| invalid(&format!("invalid replay drain frontier: {error}")))?;
        let (height, hash) = Self::canonical_merged_lane_frontier_from_world(
            state.world(),
            certified.lane_id,
            certified.dataspace_id,
            certified.lane_incarnation,
        )?;
        let mut frontier = LaneDrainFrontierV1::ordinary(
            certified.lane_id,
            certified.dataspace_id,
            certified.lane_incarnation,
            height,
            hash,
        );
        if let Some(marker) = Self::canonical_native_amx_participant_frontier_from_world(
            state.world(),
            certified.lane_id,
            certified.dataspace_id,
            certified.lane_incarnation,
        )? {
            Self::validate_native_amx_participant_shared_frontier(state.world(), &marker)?;
            let index = marker
                .application_block_height
                .checked_sub(1)
                .and_then(|height| usize::try_from(height).ok());
            if index.is_none_or(|index| {
                state.block_hashes().get(index) != Some(&marker.application_block_hash)
            }) {
                return Err(invalid(
                    "replay drain Native application is outside the exact State prefix",
                ));
            }
            if marker.lane_block_height == height {
                let evidence = certified.native_application.ok_or_else(|| {
                    invalid(
                        "replay drain Native frontier lacks its authenticated certificate evidence",
                    )
                })?;
                if evidence.participant_view != marker.participant_view
                    || evidence.predecessor_height != marker.previous_lane_block_height
                    || evidence.predecessor_descriptor_hash
                        != marker.previous_lane_block_descriptor_hash
                    || evidence.participant_proposal_hash != marker.participant_proposal_hash
                    || evidence.participant_settlement_hash != marker.participant_settlement_hash
                    || u64::from(evidence.source_count) != marker.source_count
                    || evidence.application_block_height != marker.application_block_height
                    || evidence.application_block_hash != marker.application_block_hash
                {
                    return Err(invalid(
                        "replay drain Native evidence differs from the replicated marker",
                    ));
                }
                frontier.native_application = Some(evidence);
            }
        }
        if frontier != certified {
            return Err(invalid(
                "replay drain certificate differs from the exact replicated frontier",
            ));
        }
        Ok(frontier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::store::LiveQueryStore;

    #[test]
    fn replay_native_drain_frontier_binds_marker_prefix_and_certificate_evidence() {
        let marker = AppliedNativeAmxParticipantFrontierMarker {
            version: 2,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(9),
            lane_incarnation: Hash::new(b"replay-drain-incarnation"),
            lane_block_height: 1,
            participant_view: 3,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Hash::new(b"replay-drain-descriptor"),
            participant_proposal_hash: Hash::new(b"replay-drain-proposal"),
            participant_settlement_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"replay-drain-settlement",
            )),
            application_block_height: 1,
            application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"replay-drain-application",
            )),
            source_count: 2,
        };
        let shared = AppliedMergeLaneFrontierMarker {
            version: 1,
            lane_id: marker.lane_id,
            dataspace_id: marker.dataspace_id,
            lane_incarnation: marker.lane_incarnation,
            lane_block_height: marker.lane_block_height,
            lane_block_descriptor_hash: marker.lane_block_descriptor_hash,
            applied_global_height: 1,
        };
        let mut world = World::default();
        for (key, payload) in [
            State::encode_merge_lane_frontier_marker(shared).unwrap(),
            State::encode_native_amx_participant_frontier_marker(marker).unwrap(),
        ] {
            world.smart_contract_state.insert(key, payload);
        }
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        {
            let mut hashes = state.block_hashes.block();
            hashes.push(marker.application_block_hash);
            hashes.commit_for_tests();
        }
        let mut frontier = LaneDrainFrontierV1::ordinary(
            marker.lane_id,
            marker.dataspace_id,
            marker.lane_incarnation,
            1,
            Some(marker.lane_block_descriptor_hash),
        );
        frontier.native_application =
            Some(iroha_data_model::merge::LaneDrainNativeFrontierEvidenceV1 {
                version: 1,
                participant_view: marker.participant_view,
                predecessor_height: marker.previous_lane_block_height,
                predecessor_descriptor_hash: marker.previous_lane_block_descriptor_hash,
                participant_proposal_hash: marker.participant_proposal_hash,
                participant_settlement_hash: marker.participant_settlement_hash,
                source_count: 2,
                application_block_height: marker.application_block_height,
                application_block_hash: marker.application_block_hash,
                executed_block_wire_hash: Hash::new(b"replay-drain-wire"),
                finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"replay-drain-finality",
                )),
                application_manifest_root: Hash::new(b"replay-drain-manifest-root"),
                application_manifest_leaf_count: 1,
                application_manifest_leaf_index: 0,
                manifest_artifact_hash: Hash::new(b"replay-drain-manifest-artifact"),
                receipt_artifact_hash: Hash::new(b"replay-drain-receipt"),
                latest_index_artifact_hash: Hash::new(b"replay-drain-index"),
            });
        assert_eq!(
            State::lane_drain_frontier_from_committed_state(&state.view(), frontier).unwrap(),
            frontier
        );
        for attack in ["missing-native", "settlement", "source-count", "descriptor"] {
            let mut changed = frontier;
            match attack {
                "missing-native" => changed.native_application = None,
                "settlement" => {
                    changed
                        .native_application
                        .as_mut()
                        .unwrap()
                        .participant_settlement_hash =
                        HashOf::from_untyped_unchecked(Hash::new(b"wrong-settlement"))
                }
                "source-count" => changed.native_application.as_mut().unwrap().source_count += 1,
                "descriptor" => {
                    changed.lane_block_descriptor_hash = Some(Hash::new(b"wrong-descriptor"))
                }
                _ => unreachable!(),
            }
            assert!(
                State::lane_drain_frontier_from_committed_state(&state.view(), changed).is_err(),
                "{attack}"
            );
        }
        {
            let mut hashes = state.block_hashes.block_and_revert();
            hashes.push(HashOf::from_untyped_unchecked(Hash::new(b"foreign-prefix")));
            hashes.commit_for_tests();
        }
        assert!(State::lane_drain_frontier_from_committed_state(&state.view(), frontier).is_err());
    }
}
