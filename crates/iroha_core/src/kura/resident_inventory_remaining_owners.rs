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
flat_resident_owner!(BTreeMap<LaneId,LaneStorageEntry>, ResidentFrontier);
flat_resident_owner!(BTreeMap<LaneId,CertifiedPairDurabilityAttestation>, ResidentFrontier);
flat_resident_owner!(BTreeMap<LaneId,CertifiedFrontierArtifactValidationAttestation>, ResidentFrontier);

