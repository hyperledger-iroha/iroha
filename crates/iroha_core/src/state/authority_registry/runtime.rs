//! Exact runtime ownership. The retained owner policy has a typed Norito codec;
//! effective lanes use the alias-inclusive V1 semantic projection.

use super::{Canonical, Field, Role, Schema, V1_LAYOUT, schema};
use crate::state::{
    AutoscaleSampleRecord, SnapshotLaneIncarnationLineage, SnapshotNexusOwnerPolicy,
    SnapshotNexusRuntime,
};

classified_owner!(SnapshotNexusRuntime, check_runtime_fields, RUNTIME_FIELDS, {
    version: u8 => ("runtime.version",
        Role::Canonical(Canonical::Cell(schema::<u8>())));
    owner_policy: SnapshotNexusOwnerPolicy => ("runtime.owner_policy",
        Role::Canonical(Canonical::Cell(schema::<SnapshotNexusOwnerPolicy>())));
    lane_count: u32 => ("runtime.lane_count",
        Role::Canonical(Canonical::Cell(schema::<u32>())));
    lanes: Vec<iroha_data_model::nexus::LaneConfig> => ("runtime.lanes",
        Role::Canonical(Canonical::Cell(Schema::Semantic {
            identity: "iroha:state:runtime-lane-catalog:v1",
            encoder: "state::authority_registry::runtime_lane_policy::RuntimeLaneCatalogAuthorityV1::from_lanes; alias-inclusive LaneConfig::consensus_projection",
            layout: V1_LAYOUT,
        })));
    lane_incarnation_lineage: Vec<SnapshotLaneIncarnationLineage> => ("runtime.lane_incarnation_lineage",
        Role::Canonical(Canonical::Cell(schema::<Vec<SnapshotLaneIncarnationLineage>>())));
    autoscale_last_transition_height: u64 => ("runtime.autoscale_last_transition_height",
        Role::Canonical(Canonical::Cell(schema::<u64>())));
    autoscale_scale_out_window_blocks: u16 => ("runtime.autoscale_scale_out_window_blocks",
        Role::Canonical(Canonical::Cell(schema::<u16>())));
    autoscale_scale_in_window_blocks: u16 => ("runtime.autoscale_scale_in_window_blocks",
        Role::Canonical(Canonical::Cell(schema::<u16>())));
    autoscale_sample_history_cap: u32 => ("runtime.autoscale_sample_history_cap",
        Role::Canonical(Canonical::Cell(schema::<u32>())));
    autoscale_sample_history: Vec<AutoscaleSampleRecord> => ("runtime.autoscale_sample_history",
        Role::Canonical(Canonical::Cell(schema::<Vec<AutoscaleSampleRecord>>())));
});
