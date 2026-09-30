//! Select one original lane incarnation only from an already authenticated chronological prefix.
use iroha_data_model::sumeragi_lanes::{SumeragiLaneRecord, SumeragiLaneState};
use iroha_model_base::topology::LaneId;
use std::io;

pub(super) struct LaneSelection {
    lane: LaneId,
    incarnation: [u8; 32],
    original: Option<SumeragiLaneRecord>,
    disappeared: bool,
}
impl LaneSelection {
    pub(super) fn new(lane: LaneId, incarnation: [u8; 32]) -> Self {
        Self {
            lane,
            incarnation,
            original: None,
            disappeared: false,
        }
    }
    // The enclosing provider calls this only with a VerifiedNativeExecutionCarrier receipt.
    // No result leaves that provider until the full interval matches the original State tip.
    pub(super) fn observe(&mut self, height: u64, state: &SumeragiLaneState) -> io::Result<()> {
        let record = state
            .lane(self.lane)
            .filter(|record| record.incarnation == self.incarnation);
        let Some(record) = record else {
            self.disappeared |= self.original.is_some();
            return Ok(());
        };
        if self.disappeared
            || record.created_at > height
            || record.created_at.checked_add(2) != Some(record.active_from)
        {
            return Err(super::invalid(
                "historical incarnation reappears or has invalid activation",
            ));
        }
        if let Some(original) = &self.original {
            if original.lane != record.lane
                || original.dataspace != record.dataspace
                || original.incarnation != record.incarnation
                || original.params != record.params
                || original.da_layout != record.da_layout
                || original.committee != record.committee
                || original.created_at != record.created_at
                || original.active_from != record.active_from
                || original.anchor_freshness != record.anchor_freshness
            {
                return Err(super::invalid(
                    "historical incarnation changes its immutable authority",
                ));
            }
        } else {
            self.original = Some(record.clone());
        }
        Ok(())
    }
    pub(super) fn finish(self, tip: u64) -> Option<SumeragiLaneRecord> {
        self.original.filter(|record| record.active_from <= tip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        parameter::system::SumeragiParameters, sumeragi_lanes::SumeragiLaneFrontier,
    };
    use iroha_model_base::topology::DataSpaceId;

    // Predicate-only fixtures. These do not purport to authenticate a carrier or its committee;
    // the production caller obtains those facts solely from NativeExecutionEvidenceVerifier.
    fn record() -> SumeragiLaneRecord {
        SumeragiLaneRecord {
            da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
            lane: LaneId::new(7),
            dataspace: DataSpaceId::new(3),
            incarnation: [0x21; 32],
            params: SumeragiParameters::default(),
            committee: vec![],
            created_at: 10,
            active_from: 12,
            closing: None,
            anchor_freshness: 16,
            merged: SumeragiLaneFrontier::default(),
            merged_at: 12,
            rescued: 0,
        }
    }
    fn state(record: SumeragiLaneRecord) -> SumeragiLaneState {
        SumeragiLaneState {
            lanes: vec![record],
            ..SumeragiLaneState::default()
        }
    }
    #[test]
    fn activation_is_required_and_retirement_keeps_the_exact_original_authority() {
        let original = record();
        let mut selection = LaneSelection::new(original.lane, original.incarnation);
        selection.observe(10, &state(original.clone())).unwrap();
        assert!(selection.finish(11).is_none());
        let mut selection = LaneSelection::new(original.lane, original.incarnation);
        selection.observe(10, &state(original.clone())).unwrap();
        let mut progressed = original.clone();
        progressed.closing = Some(20);
        progressed.rescued = 999;
        progressed.merged.height = 6;
        progressed.merged_at = 19;
        selection.observe(20, &state(progressed)).unwrap();
        selection
            .observe(40, &SumeragiLaneState::default())
            .unwrap();
        let mut replacement = original.clone();
        replacement.incarnation = [0x22; 32];
        selection.observe(41, &state(replacement)).unwrap();
        assert_eq!(selection.finish(41), Some(original));
    }
    #[test]
    fn another_incarnation_or_lane_is_never_substituted_for_the_request() {
        let original = record();
        for (lane, incarnation) in [
            (LaneId::new(8), original.incarnation),
            (original.lane, [0x22; 32]),
        ] {
            let mut selected = LaneSelection::new(lane, incarnation);
            selected.observe(12, &state(original.clone())).unwrap();
            assert!(selected.finish(12).is_none());
        }
    }
    #[test]
    fn immutable_authority_changes_and_reappearance_are_rejected() {
        let original = record();
        let mut changes = vec![];
        let mut changed = original.clone();
        changed.da_layout.chunk_size_bytes *= 2;
        changes.push(changed);
        let mut changed = original.clone();
        changed.anchor_freshness += 1;
        changes.push(changed);
        let mut changed = original.clone();
        changed.dataspace = DataSpaceId::new(8);
        changes.push(changed);
        let mut changed = original.clone();
        changed.created_at += 1;
        changed.active_from += 1;
        changes.push(changed);
        for changed in changes {
            let mut selected = LaneSelection::new(original.lane, original.incarnation);
            selected.observe(10, &state(original.clone())).unwrap();
            assert_eq!(
                selected.observe(15, &state(changed)).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        let mut selected = LaneSelection::new(original.lane, original.incarnation);
        selected.observe(10, &state(original.clone())).unwrap();
        selected.observe(40, &SumeragiLaneState::default()).unwrap();
        assert!(selected.observe(41, &state(original)).is_err());
    }
    #[test]
    fn malformed_future_activation_and_overflow_never_grant_authority() {
        for (created, active, height) in [(10, 11, 10), (10, 12, 9), (u64::MAX, 1, u64::MAX)] {
            let mut record = record();
            record.created_at = created;
            record.active_from = active;
            let mut selected = LaneSelection::new(record.lane, record.incarnation);
            assert!(selected.observe(height, &state(record)).is_err());
            assert!(selected.finish(height).is_none());
        }
    }
}
