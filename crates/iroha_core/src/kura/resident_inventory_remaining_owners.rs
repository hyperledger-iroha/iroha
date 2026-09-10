// Each association below is a stored metadata lookup/queue record. Payload
// internals and allocator capacities remain the separate process RSS measure.

macro_rules! flat_resident_owner {
    ($owner:ty, $family:ident) => {
        impl resident_inventory::ResidentOwner for $owner {
            const FAMILY: resource_inventory::Family = resource_inventory::Family::$family;
            fn resident_associations(
                &self,
            ) -> std::result::Result<u64, resource_inventory::Unavailable> {
                resident_inventory::lengths([self.len()])
            }
            fn resident_complete(&self) -> bool {
                true
            }
        }
    };
}
flat_resident_owner!(VecDeque<VerifiedV2FinalityCacheEntry>, ResidentVerification);
flat_resident_owner!(VecDeque<PipelineRecoverySidecar>, ResidentQueue);
flat_resident_owner!(VecDeque<QueuedFastpqProofSnapshot>, ResidentQueue);
flat_resident_owner!(BTreeMap<LaneId,LaneConfigEntry>, ResidentFrontier);
flat_resident_owner!(BTreeMap<LaneId,CertifiedFrontierPairDurabilityAttestation>, ResidentFrontier);
flat_resident_owner!(BTreeMap<LaneId,CertifiedFrontierArtifactValidationAttestation>, ResidentFrontier);

impl resident_nested_map::AssociationValue for BTreeMap<PeerId, BlockReplicaAdvert> {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentReplica;
    fn association_weight(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        resident_inventory::lengths([1, self.len()])
    }
}
impl resident_nested_map::AssociationValue for PostWsvLaneArtifactBudgetReservation {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentFrontier;
    fn association_weight(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        resident_inventory::lengths([
            1,
            self.plan.stable_components.len(),
            self.plan.executions.len(),
            self.outstanding_components.len(),
            self.incomplete_terminal_outcomes.len(),
        ])
    }
}
impl resident_nested_map::AssociationValue for CertifiedBundleCapacityReservation {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentFrontier;
    fn association_weight(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        resident_inventory::lengths([
            1,
            self.plan.component_bytes.len(),
            self.plan.component_transient_bytes.len(),
            self.outstanding_components.len(),
        ])
    }
}
impl resident_nested_map::AssociationValue for StableSidecarDirectoryInventory {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentVerification;
    fn association_weight(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        resident_inventory::lengths([1, self.files.len()])
    }
}
impl resident_inventory::ResidentOwner for V2StartupFinalityVerificationInventoryData {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentVerification;
    fn resident_associations(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        let direct = resident_inventory::lengths([
            self.lane_auxiliary_directories.len(),
            self.hash_only_heights.len(),
            self.entries.len(),
            usize::from(self.durable_tip_artifact.is_some()),
            usize::from(self.highest_verified_finality_artifact.is_some()),
        ])?;
        direct
            .checked_add(self.replay_associations.get()?)
            .and_then(|count| {
                count.checked_add(self.auxiliary_sidecars.resident_associations().ok()?)
            })
            .ok_or(resource_inventory::Unavailable::Arithmetic)
    }
    fn resident_complete(&self) -> bool {
        self.resident_associations().is_ok()
    }
}
/// Count the newly built replay vector once, within the authenticated audit.
fn startup_replay_associations(sidecars: &[V2StartupReplaySidecarsAtHeight]) -> AssociationCount {
    let mut count = AssociationCount::default();
    for at_height in sidecars {
        count.replace(
            Some(0),
            resident_inventory::lengths([
                1,
                usize::from(at_height.checkpoint.is_some()),
                usize::from(at_height.manifest.is_some()),
            ])
            .ok(),
        );
    }
    count
}
