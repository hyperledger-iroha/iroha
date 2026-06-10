//! Grouped Torii Sumeragi and telemetry integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[cfg(feature = "telemetry")]
static RBC_STATUS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(feature = "telemetry")]
fn rbc_status_test_guard() -> std::sync::MutexGuard<'static, ()> {
    RBC_STATUS_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[path = "../metrics_registry.rs"]
mod metrics_registry;
#[path = "../metrics_registry_reset.rs"]
mod metrics_registry_reset;
#[path = "../new_view_json.rs"]
mod new_view_json;
#[path = "../new_view_sse.rs"]
mod new_view_sse;
#[path = "../pipeline_recovery_endpoint.rs"]
mod pipeline_recovery_endpoint;
#[path = "../sse_proof_callhash.rs"]
mod sse_proof_callhash;
#[path = "../sse_proof_envelope_hash.rs"]
mod sse_proof_envelope_hash;
#[path = "../sse_proof_rejected_fields.rs"]
mod sse_proof_rejected_fields;
#[path = "../sse_proof_verified_fields.rs"]
mod sse_proof_verified_fields;
#[path = "../sumeragi_collectors_endpoint.rs"]
mod sumeragi_collectors_endpoint;
#[path = "../sumeragi_commit_qc_endpoints.rs"]
mod sumeragi_commit_qc_endpoints;
#[path = "../sumeragi_evidence_count_endpoint.rs"]
mod sumeragi_evidence_count_endpoint;
#[path = "../sumeragi_evidence_list_endpoint.rs"]
mod sumeragi_evidence_list_endpoint;
#[path = "../sumeragi_leader_endpoint.rs"]
mod sumeragi_leader_endpoint;
#[path = "../sumeragi_pacemaker_endpoint.rs"]
mod sumeragi_pacemaker_endpoint;
#[path = "../sumeragi_params_endpoint.rs"]
mod sumeragi_params_endpoint;
#[path = "../sumeragi_phases_endpoint.rs"]
mod sumeragi_phases_endpoint;
#[path = "../sumeragi_qc_endpoint.rs"]
mod sumeragi_qc_endpoint;
#[path = "../sumeragi_rbc_delivered_endpoint.rs"]
mod sumeragi_rbc_delivered_endpoint;
#[path = "../sumeragi_rbc_endpoint.rs"]
mod sumeragi_rbc_endpoint;
#[path = "../sumeragi_rbc_sessions_endpoint.rs"]
mod sumeragi_rbc_sessions_endpoint;
#[path = "../sumeragi_status_endpoint.rs"]
mod sumeragi_status_endpoint;
#[path = "../sumeragi_status_sse.rs"]
mod sumeragi_status_sse;
#[path = "../sumeragi_status_sse_profile_gate.rs"]
mod sumeragi_status_sse_profile_gate;
#[path = "../sumeragi_tel_subrouter_smoke.rs"]
mod sumeragi_tel_subrouter_smoke;
#[path = "../sumeragi_telemetry_endpoints.rs"]
mod sumeragi_telemetry_endpoints;
#[path = "../sumeragi_vrf_penalties_endpoint.rs"]
mod sumeragi_vrf_penalties_endpoint;
#[path = "../telemetry_gating.rs"]
mod telemetry_gating;
#[path = "../torii_start.rs"]
mod torii_start;
