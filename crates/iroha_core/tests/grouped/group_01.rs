//! Grouped Iroha Core integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../admission_batching.rs"]
mod admission_batching;
#[path = "../adversarial_block_rejections.rs"]
mod adversarial_block_rejections;
#[path = "../asset_total_amount.rs"]
mod asset_total_amount;
#[path = "../bench_repro.rs"]
mod bench_repro;
#[path = "../bls_batch_pop.rs"]
mod bls_batch_pop;
#[path = "../bridge_finality_proof.rs"]
mod bridge_finality_proof;
#[path = "../bridge_proofs.rs"]
mod bridge_proofs;
#[path = "../cache_policy.rs"]
mod cache_policy;
#[path = "../check_genesis_sig.rs"]
mod check_genesis_sig;
#[path = "../confidential_params_registry.rs"]
mod confidential_params_registry;
#[path = "../confidential_policy_gates.rs"]
mod confidential_policy_gates;
#[path = "../contract_code_bytes.rs"]
mod contract_code_bytes;
#[path = "../contract_manifest_triggers.rs"]
mod contract_manifest_triggers;
#[path = "../deterministic_tie_break.rs"]
mod deterministic_tie_break;
#[path = "../event_ordering.rs"]
mod event_ordering;
#[path = "../execute_trigger_events.rs"]
mod execute_trigger_events;
#[path = "../executor_migration_introspect.rs"]
mod executor_migration_introspect;
#[path = "../fastpq_transfer_batch.rs"]
mod fastpq_transfer_batch;
#[path = "../find_accounts_with_asset.rs"]
mod find_accounts_with_asset;
#[path = "../fraud_monitoring.rs"]
mod fraud_monitoring;
#[path = "../gov_auto_close_approve.rs"]
mod gov_auto_close_approve;
#[path = "../gov_auto_close_zk_requires_tally.rs"]
mod gov_auto_close_zk_requires_tally;
#[path = "../gov_bond_escrow.rs"]
mod gov_bond_escrow;
#[path = "../gov_citizen_service.rs"]
mod gov_citizen_service;
#[path = "../gov_citizenship.rs"]
mod gov_citizenship;
#[path = "../gov_council_gate.rs"]
mod gov_council_gate;
#[path = "../gov_draw.rs"]
mod gov_draw;
#[path = "../gov_draw_replace.rs"]
mod gov_draw_replace;
#[path = "../gov_draw_selector.rs"]
mod gov_draw_selector;
#[path = "../gov_enact_deploy.rs"]
mod gov_enact_deploy;
#[path = "../gov_finalize_real_vk.rs"]
mod gov_finalize_real_vk;
#[path = "../gov_finalize_requires_zk_tally.rs"]
mod gov_finalize_requires_zk_tally;
