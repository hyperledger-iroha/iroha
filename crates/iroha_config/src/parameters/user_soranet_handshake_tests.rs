use super::*;

#[test]
fn default_ticket_ttl_uses_full_future_skew_window() {
    assert_eq!(
        SoranetHandshakePow::default_ticket_ttl(),
        SoranetHandshakePow::default_max_future_skew()
    );
    assert!(
        SoranetHandshakePow::default_ticket_ttl() > SoranetHandshakePow::default_min_ticket_ttl()
    );
}

#[test]
fn puzzle_work_capacity_is_nonzero_and_bounded() {
    assert_eq!(
        SoranetHandshakePow::default_puzzle_work_capacity(),
        actual::SoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION
    );
    assert_eq!(
        SoranetHandshakePow::bound_puzzle_work_capacity(nonzero!(usize::MAX)).get(),
        actual::SoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION
    );
}

#[test]
fn parse_bounds_argon2_resource_costs() {
    let upper = SoranetHandshakePuzzle {
        memory_kib: u32::MAX,
        time_cost: u32::MAX,
        lanes: u32::MAX,
    }
    .parse();
    assert_eq!(
        upper.memory_kib.get(),
        iroha_crypto::soranet::puzzle::MAX_MEMORY_KIB
    );
    assert_eq!(
        upper.time_cost.get(),
        iroha_crypto::soranet::puzzle::MAX_TIME_COST
    );
    assert_eq!(upper.lanes.get(), iroha_crypto::soranet::puzzle::MAX_LANES);
    let lower = SoranetHandshakePuzzle {
        memory_kib: 0,
        time_cost: 0,
        lanes: 0,
    }
    .parse();
    assert_eq!(
        lower.memory_kib.get(),
        iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB
    );
    assert_eq!(lower.time_cost.get(), 1);
    assert_eq!(lower.lanes.get(), 1);
}
