#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Grouped integration harness for network churn and functional network scenarios.

#[path = "concurrency.rs"]
mod concurrency;
#[path = "extra_functional/mod.rs"]
mod extra_functional;
#[path = "observer_sync.rs"]
mod observer_sync;
#[path = "privacy_exact12_jindo_network.rs"]
mod privacy_exact12_jindo_network;
#[path = "privacy_exact12_zk_ams_vega_network.rs"]
mod privacy_exact12_zk_ams_vega_network;
#[path = "sccp_route_governance.rs"]
mod sccp_route_governance;
