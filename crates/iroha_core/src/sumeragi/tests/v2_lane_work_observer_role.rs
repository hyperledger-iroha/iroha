#[test]
fn observer_role_cannot_sign_lane_merge_or_native_amx_votes() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    adapter.voting_enabled = false;
    let request = native_request(&adapter, &keys);
    assert_eq!(adapter.local_validator_index(), None);
    assert!(adapter.sign_native_request_once(&request, 0).is_none());
    assert!(adapter.local_native_claims.is_empty());
}
