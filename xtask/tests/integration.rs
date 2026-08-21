//! Aggregated integration tests for `xtask`.

#[path = "address_vectors.rs"]
mod address_vectors;
#[path = "android_dashboard_parity_cli.rs"]
mod android_dashboard_parity_cli;
#[path = "codec_rans_tables.rs"]
mod codec_rans_tables;
#[path = "da_proof_bench.rs"]
mod da_proof_bench;
#[path = "iso_bridge_lint.rs"]
mod iso_bridge_lint;
#[path = "ministry_agenda.rs"]
mod ministry_agenda;
#[path = "mochi_bundle.rs"]
mod mochi_bundle;
#[path = "sm_wycheproof_sync.rs"]
mod sm_wycheproof_sync;
#[path = "soradns_cli.rs"]
mod soradns_cli;
#[path = "sorafs_fetch_fixture.rs"]
mod sorafs_fetch_fixture;
#[path = "soranet_bug_bounty.rs"]
mod soranet_bug_bounty;
#[path = "soranet_chaos.rs"]
mod soranet_chaos;
#[path = "soranet_gateway_billing.rs"]
mod soranet_gateway_billing;
#[path = "soranet_gateway_m0.rs"]
mod soranet_gateway_m0;
#[path = "soranet_gateway_m1.rs"]
mod soranet_gateway_m1;
#[path = "soranet_gateway_m2.rs"]
mod soranet_gateway_m2;
#[path = "soranet_gateway_ops_m0.rs"]
mod soranet_gateway_ops_m0;
#[path = "soranet_pop_template.rs"]
mod soranet_pop_template;
#[path = "streaming_bundle_check.rs"]
mod streaming_bundle_check;
#[path = "streaming_entropy_bench.rs"]
mod streaming_entropy_bench;
