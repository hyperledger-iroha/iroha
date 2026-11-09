//! Deadline closure preserves custody while releasing derived hidden-ballot work.

use super::*;
use iroha_config::parameters::actual::LaneConfig as RuntimeLaneConfig;

#[test]
fn sorafs_authority_closure_releases_casting_capacity_without_rewriting_custody() {
    let attempt =
        crate::governance::parliament::tests::active_timed_ovn_reservation_attempt_fixture_v1(
            0x51, 0x61, 27,
        );
    let id = attempt.attempt().id;
    let original = norito::to_bytes(&attempt).unwrap();
    let world = World::new();
    let mut block = world.block();
    let mut tx = block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
    tx.put_parliament_attempt(attempt).unwrap();
    let reservations = tx
        .parliament_timed_ovn_resource_reservations
        .iter()
        .map(|(id, row)| (*id, *row))
        .collect::<BTreeMap<_, _>>();
    let custody = tx
        .parliament_tle_key_session_retention_deadlines
        .iter()
        .map(|(id, row)| (*id, row.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        reservations.values().filter(|row| row.cast_capable).count(),
        1
    );
    assert!(!custody.is_empty());
    tx.close_sorafs_authority_attempt_work(id).unwrap();
    assert!(
        tx.parliament_timed_ovn_casting_candidates
            .iter()
            .next()
            .is_none()
    );
    assert!(
        tx.parliament_timed_ovn_resource_reservations
            .iter()
            .all(|(_, row)| !row.cast_capable)
    );
    for (ballot, before) in &reservations {
        let after = tx
            .parliament_timed_ovn_resource_reservations
            .get(ballot)
            .unwrap();
        assert_eq!(after.resource_windows, before.resource_windows);
        assert_eq!(after.governance_attempt_id, before.governance_attempt_id);
    }
    assert_eq!(
        tx.parliament_tle_key_session_retention_deadlines
            .iter()
            .map(|(id, row)| (*id, row.clone()))
            .collect::<BTreeMap<_, _>>(),
        custody
    );
    assert_eq!(
        norito::to_bytes(tx.parliament_attempts.get(&id).unwrap()).unwrap(),
        original
    );
    let rebuilt =
        parliament_derived_read_indexes_v1(tx.parliament_attempts.iter(), |_| true).unwrap();
    assert!(rebuilt.timed_ovn_casting_candidates.is_empty());
    assert_eq!(
        rebuilt.timed_ovn_resource_reservations,
        tx.parliament_timed_ovn_resource_reservations
            .iter()
            .map(|(id, row)| (*id, *row))
            .collect()
    );
    assert_eq!(rebuilt.tle_key_session_retention_deadlines, custody);
    let capacity =
        iroha_data_model::parliament_casting::MAX_PARLIAMENT_CONCURRENT_CASTING_CONTEXTS_V1;
    assert!(
        !iroha_data_model::parliament_casting::parliament_timed_ovn_casting_capacity_allows_new_v1(
            capacity
        )
    );
    assert!(
        iroha_data_model::parliament_casting::parliament_timed_ovn_casting_capacity_allows_new_v1(
            capacity - 1
        )
    );
}
