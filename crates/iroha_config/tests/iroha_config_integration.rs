//! Consolidated integration-test harness for `iroha_config`.

#[path = "autoscale_config.rs"]
mod autoscale_config;
#[path = "compute_economics.rs"]
mod compute_economics;
#[path = "da_ingest_compute_limit.rs"]
mod da_ingest_compute_limit;
#[path = "fastpq_queue_overrides.rs"]
mod fastpq_queue_overrides;
#[path = "fixtures.rs"]
mod fixtures;
#[path = "governance_alternates_parse.rs"]
mod governance_alternates_parse;
#[path = "governance_citizen_service_parse.rs"]
mod governance_citizen_service_parse;
#[path = "pipeline_cycle_ceiling.rs"]
mod pipeline_cycle_ceiling;
#[path = "sccp_route_manifest_aliases.rs"]
mod sccp_route_manifest_aliases;
#[path = "sorafs_gateway_runtime_providers.rs"]
mod sorafs_gateway_runtime_providers;
#[path = "sorafs_governance_dag_runtime_signer.rs"]
mod sorafs_governance_dag_runtime_signer;
#[path = "sorafs_native_transaction_signers.rs"]
mod sorafs_native_transaction_signers;
#[path = "sorafs_por_replay_archive.rs"]
mod sorafs_por_replay_archive;
#[path = "sorafs_provider_ingest_finalized_archive.rs"]
mod sorafs_provider_ingest_finalized_archive;
#[path = "sorafs_reputation_finalized_archive.rs"]
mod sorafs_reputation_finalized_archive;
#[path = "sorafs_storage_pin_aliases.rs"]
mod sorafs_storage_pin_aliases;
#[path = "sorafs_stream_token_runtime_signer.rs"]
mod sorafs_stream_token_runtime_signer;
#[path = "sumeragi_v2_merge_runtime_config.rs"]
mod sumeragi_v2_merge_runtime_config;
#[path = "transaction_ingress_limits.rs"]
mod transaction_ingress_limits;
#[path = "trusted_peers_pop_validation.rs"]
mod trusted_peers_pop_validation;
