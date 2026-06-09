//! Grouped Iroha Core integration tests.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

#[path = "../gov_min_duration.rs"]
mod gov_min_duration;
#[path = "../gov_mode_mismatch.rs"]
mod gov_mode_mismatch;
#[path = "../gov_mode_mismatch_zk.rs"]
mod gov_mode_mismatch_zk;
#[path = "../gov_parliament_adversarial_sybil.rs"]
mod gov_parliament_adversarial_sybil;
#[path = "../gov_parliament_bodies.rs"]
mod gov_parliament_bodies;
#[path = "../gov_parliament_lifecycle_plain.rs"]
mod gov_parliament_lifecycle_plain;
#[path = "../gov_parliament_lifecycle_zk.rs"]
mod gov_parliament_lifecycle_zk;
#[path = "../gov_parliament_term_state.rs"]
mod gov_parliament_term_state;
#[path = "../gov_pipeline_sla.rs"]
mod gov_pipeline_sla;
#[path = "../gov_plain_ballot.rs"]
mod gov_plain_ballot;
#[path = "../gov_plain_conviction.rs"]
mod gov_plain_conviction;
#[path = "../gov_plain_disabled.rs"]
mod gov_plain_disabled;
#[path = "../gov_plain_missing_ref.rs"]
mod gov_plain_missing_ref;
#[path = "../gov_plain_referendum_open_event.rs"]
mod gov_plain_referendum_open_event;
#[path = "../gov_plain_revote_monotonic.rs"]
mod gov_plain_revote_monotonic;
#[path = "../gov_propose_validation.rs"]
mod gov_propose_validation;
#[path = "../gov_protected_gate.rs"]
mod gov_protected_gate;
#[path = "../gov_referendum_open_close.rs"]
mod gov_referendum_open_close;
#[path = "../gov_referendum_window_guard.rs"]
mod gov_referendum_window_guard;
#[path = "../gov_slash_and_restitute.rs"]
mod gov_slash_and_restitute;
#[path = "../gov_slash_restitution.rs"]
mod gov_slash_restitution;
#[path = "../gov_sortition_seed.rs"]
mod gov_sortition_seed;
#[path = "../gov_thresholds.rs"]
mod gov_thresholds;
#[path = "../gov_thresholds_positive.rs"]
mod gov_thresholds_positive;
#[path = "../gov_unlock_sweep.rs"]
mod gov_unlock_sweep;
#[path = "../gov_zk_ballot.rs"]
mod gov_zk_ballot;
#[path = "../gov_zk_ballot_lock_verified.rs"]
mod gov_zk_ballot_lock_verified;
#[path = "../gov_zk_ballot_real_vk.rs"]
mod gov_zk_ballot_real_vk;
#[path = "../gov_zk_ballot_vk_status.rs"]
mod gov_zk_ballot_vk_status;
#[path = "../gov_zk_create_inserts_referendum.rs"]
mod gov_zk_create_inserts_referendum;
#[path = "../gov_zk_create_rejects_plain_conflict.rs"]
mod gov_zk_create_rejects_plain_conflict;
#[path = "../gov_zk_nullifier_owner_salt.rs"]
mod gov_zk_nullifier_owner_salt;
