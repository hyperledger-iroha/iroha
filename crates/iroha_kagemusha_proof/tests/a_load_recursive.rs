//! Genuine rooted Bootstrap predecessor and Load recursive partition inventory.
//! A passing diagnostic is not a final Load lineage acceptance or size gate.

// Both included suites also run independently; each keeps its own deterministic
// helper/cache module when composed into this complete-chain test.
#![allow(clippy::duplicate_mod)]

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
/// Shared genuine outer-proof fixture, including its component regressions.
#[path = "bootstrap_omega.rs"]
pub mod bootstrap_outer;
mod common;
#[path = "common/consuming_proof.rs"]
mod consuming_proof;
/// Shared genuine Load sigma, map and signed-object fixtures.
#[path = "a_load.rs"]
pub mod load_components;
#[path = "common/load_objects.rs"]
#[allow(dead_code)]
mod load_objects;

include!("common/proof_fixtures/a_load_recursive_body.rs");

#[test]
#[ignore = "real rooted Bootstrap chain, Load Qs and A predecessor partition; run optimized"]
fn rooted_load_first_partition_preserves_real_predecessor_and_q() {
    let _ = rooted_load_profiles(&[0]);
}

#[test]
#[ignore = "real rooted Bootstrap, Load Qs and shared-range four-stage chain; run optimized"]
fn rooted_load_shared_range_four_stage_inventory() {
    assert!(rooted_load_profiles(&[4, 5]).is_some());
}

#[test]
#[ignore = "canonical selector/task schema and genuine five-bus Load closure; run optimized"]
fn canonical_five_bus_load_tasks_retain_all_openings() {
    assert!(rooted_load_profiles(&[5]).is_some());
}

#[test]
#[ignore = "canonical selector/task schema and genuine four-bus Load closure; run optimized"]
fn canonical_four_bus_load_tasks_retain_all_openings() {
    assert!(rooted_load_profiles(&[4]).is_some());
}

#[test]
#[ignore = "genuine predecessor proof and both hard signature Q leaves; run optimized"]
fn current_authorization_and_transport_reject_cross_tape_substitution() {
    let rooted = bootstrap_outer::rooted_bootstrap_omega(false);
    let _ = load_profiles(&rooted, &[4], true, true);
}

#[test]
#[ignore = "genuine candidate Q2/A3 Bootstrap predecessor and complete Load stages"]
fn reduced_q_two_bus_three_bus_load_capacity() {
    let rooted = bootstrap_outer::rooted_bootstrap_omega_with_q_layout(false, 3, Some(2));
    assert!(load_profiles_with_q(&rooted, &[3], Some(2), false, false).is_some());
}

#[test]
#[ignore = "genuine explicit tagged-A3 candidate; no production cap relaxation"]
fn reduced_q_two_bus_tagged_three_bus_load_capacity() {
    let profile = SourceProfile::Tagged { buses: 3 };
    let rooted = bootstrap_outer::rooted_bootstrap_omega_with_profile(false, profile, Some(2));
    assert!(load_source_profiles(&rooted, &[profile], Some(2), false, false, false).is_some());
}

#[test]
#[ignore = "genuine fixed-key production Load A1/W0/A2/W1/A3/W2/A4 proof and restoration differential"]
fn installed_native_load_proves_and_restores_all_four_stages() {
    let rooted = bootstrap_outer::rooted_bootstrap_omega(false);
    let profile = SourceProfile::Serialized {
        buses: iroha_kagemusha_proof::a_relation::native::load::SOURCE_RANGE_BUSES,
    };
    assert!(load_source_profiles(&rooted, &[profile], None, false, false, true).is_some());
}
