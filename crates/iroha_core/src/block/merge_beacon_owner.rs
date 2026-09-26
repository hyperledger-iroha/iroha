// One move-only transfer from the pre-State NPoS plan into certified merge composition.

impl PreparedPristineConsensusEffects {
    /// Transfer the one parent-validated plan into certified merge composition
    /// without cloning its nested effects or vectors.
    fn into_merge_beacon(
        self,
        network_id: NetworkId,
        parent_surface: Hash,
    ) -> VerifiedMergeBeaconPulse {
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
