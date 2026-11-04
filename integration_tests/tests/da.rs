//! Aggregated DA integration tests.
#[path = "da/admission.rs"]
pub(super) mod admission;
#[path = "da/pdp_potr_simulation.rs"]
mod pdp_potr_simulation;
#[path = "da/replication_policy.rs"]
mod replication_policy;
