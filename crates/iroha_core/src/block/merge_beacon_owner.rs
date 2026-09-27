// One move-only transfer from the pre-State NPoS plan into certified merge composition.

impl PreparedPristineConsensusEffects<'_> {
    /// Transfer the one parent-validated plan into certified merge composition
    /// without cloning its nested effects or vectors.
    fn into_merge_beacon(
        self,
        network_id: NetworkId,
        parent_surface: Hash,
    ) -> VerifiedMergeBeaconPulse {
        // Execution-bearing merge composition admits beacon-only effects, so
        // the validated plan must contain no slash index or penalty action.
        debug_assert!(self.penalty_index.index.is_none());
        VerifiedMergeBeaconPulse {
            header: self.header,
            network_id,
            effects: self.effects,
            prune_keys: self.prune_keys,
            roster: self.roster,
            parent_surface,
        }
    }
}
