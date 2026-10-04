//! Runtime lifecycle rows preserve canonical wire bytes and one authenticated graph.

use super::*;
use crate::{beacon::fixtures::*, test_allocations::allocations_during};

#[test]
fn retained_lifecycle_encoding_and_transitions_preserve_the_original_session_owner() {
    let fixture = adaptive_beacon_fixture();
    let pool = fixture_budget();
    let finalized = fixture.session.adaptive_dkg.finalized_at_height;
    let mut raw = FinalizedGlobalThresholdBeaconKeySessionRecordV1 {
        session: fixture.session.record().clone(),
        activated_at_height: None,
        retired_at_height: None,
    };
    let mut live = RetainedFinalizedGlobalThresholdBeaconSessionV1::admit(&raw, &pool).unwrap();
    let witness = live.clone();
    let held = pool.reserved_bytes();
    assert_eq!(held, live.session.retained_allocation_bytes());
    let check = |live: &RetainedFinalizedGlobalThresholdBeaconSessionV1,
                 raw: &FinalizedGlobalThresholdBeaconKeySessionRecordV1| {
        assert_eq!(
            norito::to_bytes(live).unwrap(),
            norito::to_bytes(raw).unwrap()
        );
        assert_eq!(
            norito::json::to_json(live).unwrap(),
            norito::json::to_json(raw).unwrap()
        );
        assert!(live.session.ptr_eq(&witness.session));
        assert!(live.session.belongs_to(&pool));
        assert_eq!(pool.reserved_bytes(), held);
    };
    check(&live, &raw);
    assert_eq!(
        live.activate(finalized - 1),
        Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
    );
    assert_eq!(
        live.retire(finalized + 1),
        Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
    );
    assert!(live.activated_at_height.is_none());
    assert!(live.retired_at_height.is_none());
    assert_eq!(allocations_during(|| live.activate(finalized).unwrap()), 0);
    raw.activated_at_height = Some(finalized);
    check(&live, &raw);
    assert!(live.is_active_at(finalized));
    assert_eq!(
        live.activate(finalized + 1),
        Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
    );
    assert_eq!(
        live.retire(finalized),
        Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
    );
    assert_eq!(
        allocations_during(|| live.retire(finalized + 1).unwrap()),
        0
    );
    raw.retired_at_height = Some(finalized + 1);
    check(&live, &raw);
    assert!(!live.is_active_at(finalized + 1));
    assert_eq!(
        live.activate(finalized),
        Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
    );
    drop(live);
    assert_eq!(pool.reserved_bytes(), held);
    drop(witness);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn invalid_lifecycle_never_installs_or_leaks_a_new_authenticated_graph() {
    let fixture = adaptive_beacon_fixture();
    let pool = fixture_budget();
    let finalized = fixture.session.adaptive_dkg.finalized_at_height;
    for (activated_at_height, retired_at_height) in [
        (None, Some(finalized + 1)),
        (Some(finalized - 1), None),
        (Some(finalized), Some(finalized)),
    ] {
        let raw = FinalizedGlobalThresholdBeaconKeySessionRecordV1 {
            session: fixture.session.record().clone(),
            activated_at_height,
            retired_at_height,
        };
        assert!(matches!(
            RetainedFinalizedGlobalThresholdBeaconSessionV1::admit(&raw, &pool),
            Err(GlobalThresholdBeaconSessionError::Invalid(
                GlobalThresholdBeaconError::InvalidKeyLifecycle
            ))
        ));
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(raw.session, *fixture.session.record());
    }
}
