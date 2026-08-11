#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Grouped integration harness for consensus, DA/RBC, and localnet scenarios.

#[path = "da.rs"]
mod da;
#[path = "sumeragi_adversarial.rs"]
mod sumeragi_adversarial;
#[path = "sumeragi_commit_certificates.rs"]
mod sumeragi_commit_certificates;
#[path = "sumeragi_da.rs"]
mod sumeragi_da;
#[path = "sumeragi_kagami_localnet.rs"]
mod sumeragi_kagami_localnet;
#[path = "sumeragi_localnet_smoke.rs"]
mod sumeragi_localnet_smoke;
#[path = "sumeragi_lock_convergence.rs"]
mod sumeragi_lock_convergence;
#[path = "sumeragi_npos_happy_path.rs"]
mod sumeragi_npos_happy_path;
#[path = "sumeragi_npos_liveness.rs"]
mod sumeragi_npos_liveness;
#[path = "sumeragi_npos_pacemaker_latency.rs"]
mod sumeragi_npos_pacemaker_latency;
#[path = "sumeragi_npos_performance.rs"]
mod sumeragi_npos_performance;
#[path = "sumeragi_npos_stake_activation.rs"]
mod sumeragi_npos_stake_activation;
#[path = "sumeragi_rotation.rs"]
mod sumeragi_rotation;
#[path = "sumeragi_telemetry.rs"]
mod sumeragi_telemetry;
#[path = "sumeragi_vote_qc_commit.rs"]
mod sumeragi_vote_qc_commit;
#[path = "taikai_da.rs"]
mod taikai_da;
#[path = "taira_public_localnet.rs"]
mod taira_public_localnet;
#[path = "zk_confidential_localnet.rs"]
mod zk_confidential_localnet;
#[path = "zk_stark_network.rs"]
mod zk_stark_network;
