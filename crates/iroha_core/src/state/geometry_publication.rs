// One process-owned lane geometry operation. Local I/O errors return to the
// caller without discarding either storage plan or recapturing moved paths.

struct LaneGeometryPublication {
    raw: Option<crate::kura::RawGeometryAttempt>,
    tiered: Option<tiered::TieredGeometryAttempt>,
    previous: LaneConfig,
    current: LaneConfig,
    previous_incarnations: BTreeMap<LaneId, Hash>,
    current_incarnations: BTreeMap<LaneId, Hash>,
    previous_activation_heights: BTreeMap<LaneId, u64>,
    current_activation_heights: BTreeMap<LaneId, u64>,
    previous_lineage_root: Hash,
    current_lineage_root: Hash,
    replaced: BTreeSet<LaneId>,
    certified: BTreeMap<(LaneId, DataSpaceId, Hash), LaneDrainFrontierV1>,
    startup_owner: Option<Arc<()>>,
    startup_attached: bool,
    transition_height: u64,
    cursors_updated: bool,
    publish_cursors: bool,
}

fn geometry_lease_error(error: crate::kura::KuraPublicationPreparationError) -> LaneLifecycleError {
    match error {
        crate::kura::KuraPublicationPreparationError::Busy { field, wait } => {
            LaneLifecycleError::PublicationBusy { field, wait }
        }
        crate::kura::KuraPublicationPreparationError::Storage(error) => {
            LaneLifecycleError::GeometryStorage(error)
        }
    }
}

impl State {


}

struct TieredStartupGeometry {
    configuration: iroha_config::parameters::actual::TieredState,
    lanes: LaneConfig,
    attempt: tiered::TieredGeometryAttempt,
}
impl TieredStartupGeometry {
    fn matches(
        &self,
        cfg: &iroha_config::parameters::actual::TieredState,
        lanes: &LaneConfig,
    ) -> bool {
        let original = &self.configuration;
        original.enabled == cfg.enabled
            && original.hot_retained_keys == cfg.hot_retained_keys
            && original.hot_retained_bytes.get() == cfg.hot_retained_bytes.get()
            && original.hot_retained_grace_snapshots == cfg.hot_retained_grace_snapshots
            && original.cold_store_root == cfg.cold_store_root
            && original.da_store_root == cfg.da_store_root
            && original.max_snapshots == cfg.max_snapshots
            && original.max_cold_bytes.get() == cfg.max_cold_bytes.get()
            && lane_config_entries_match(&self.lanes, lanes)
    }
}
