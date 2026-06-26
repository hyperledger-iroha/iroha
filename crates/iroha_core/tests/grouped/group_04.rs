//! Grouped Iroha Core integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../parallel_apply.rs"]
mod parallel_apply;
#[path = "../parallel_apply_knob.rs"]
mod parallel_apply_knob;
#[path = "../pin_registry.rs"]
mod pin_registry;
#[path = "../pipeline_warning_event.rs"]
mod pipeline_warning_event;
#[path = "../quarantine_lane.rs"]
mod quarantine_lane;
#[path = "../query_active_abi.rs"]
mod query_active_abi;
#[path = "../queue_regressions.rs"]
mod queue_regressions;
#[path = "../queue_stress.rs"]
mod queue_stress;
#[path = "../runtime_upgrade_admission.rs"]
mod runtime_upgrade_admission;
#[path = "../sccp_route_manifest_isi.rs"]
mod sccp_route_manifest_isi;
#[path = "../scheduler_gpu_key_bucket_parity.rs"]
mod scheduler_gpu_key_bucket_parity;
#[path = "../scheduler_ready_queue_heap_parity.rs"]
mod scheduler_ready_queue_heap_parity;
#[path = "../scheduler_telemetry.rs"]
mod scheduler_telemetry;
#[path = "../settlement_overlay.rs"]
mod settlement_overlay;
#[path = "../signature_batch_determinism.rs"]
mod signature_batch_determinism;
#[path = "../social_viral_incentives.rs"]
mod social_viral_incentives;
#[path = "../sparse_block_bytes.rs"]
mod sparse_block_bytes;
#[path = "../sumeragi_collectors.rs"]
mod sumeragi_collectors;
#[path = "../sumeragi_doc_sync.rs"]
mod sumeragi_doc_sync;
#[path = "../validation_fee_admission.rs"]
mod validation_fee_admission;
#[path = "../zk_asset_stark_envelope.rs"]
mod zk_asset_stark_envelope;
#[path = "../zk_asset_vk_enforcement.rs"]
mod zk_asset_vk_enforcement;
#[path = "../zk_backend_tags.rs"]
mod zk_backend_tags;
#[path = "../zk_confidential_events.rs"]
mod zk_confidential_events;
#[path = "../zk_dedup.rs"]
mod zk_dedup;
#[path = "../zk_ipa_native.rs"]
mod zk_ipa_native;
#[path = "../zk_lane_warning.rs"]
mod zk_lane_warning;
#[path = "../zk_ledger_scaffold.rs"]
mod zk_ledger_scaffold;
#[path = "../zk_preverify_budget.rs"]
mod zk_preverify_budget;
#[path = "../zk_proof_event_callhash.rs"]
mod zk_proof_event_callhash;
#[path = "../zk_proof_hash_events.rs"]
mod zk_proof_hash_events;
#[path = "../zk_proof_hash_metadata.rs"]
mod zk_proof_hash_metadata;
#[path = "../zk_proof_retention.rs"]
mod zk_proof_retention;
#[path = "../zk_root_hint_enforced.rs"]
mod zk_root_hint_enforced;
