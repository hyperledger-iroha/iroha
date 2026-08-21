use super::*;

fn layout() -> ZkAmsMkheRnsNativeSourceLayoutV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        [0x31; 32],
        [0x32; 32],
        [0x33; 32],
    )
    .expect("source layout")
}

#[test]
fn exact_layout_binds_every_context_and_arena() {
    let baseline = layout();
    baseline.validate().expect("valid layout");
    assert_ne!(
        baseline.arena_context_digest(ZkAmsMkheRnsNativeSourceArenaV1::Main),
        baseline.arena_context_digest(ZkAmsMkheRnsNativeSourceArenaV1::Nonce)
    );
    let changed = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        baseline.profile_digest(),
        baseline.topology_digest(),
        baseline.release_candidate_digest(),
        baseline.statement_digest(),
        [0x34; 32],
    )
    .expect("changed layout");
    assert_ne!(
        baseline.source_binding_digest(),
        changed.source_binding_digest()
    );
}

#[test]
fn rejects_zero_duplicate_and_foreign_profile_contexts() {
    let baseline = layout();
    assert_eq!(
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            baseline.profile_digest(),
            baseline.topology_digest(),
            [0; 32],
            [4; 32],
            [5; 32],
        ),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)
    );
    assert_eq!(
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            baseline.profile_digest(),
            baseline.topology_digest(),
            [4; 32],
            [4; 32],
            [5; 32],
        ),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)
    );
    assert_eq!(
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            [9; 32],
            baseline.topology_digest(),
            [3; 32],
            [4; 32],
            [5; 32],
        ),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)
    );
}

#[test]
fn structural_receipt_is_exactly_layout_bound_and_nonzero() {
    let baseline = layout();
    let receipt =
        ZkAmsMkheRnsNativeSourceReceiptV1::new(baseline, [0x41; 32], [0x42; 32]).expect("receipt");
    receipt.validate(baseline).expect("receipt validates");
    let changed = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        baseline.profile_digest(),
        baseline.topology_digest(),
        baseline.release_candidate_digest(),
        baseline.statement_digest(),
        [0x35; 32],
    )
    .expect("changed layout");
    assert_eq!(
        receipt.validate(changed),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication)
    );
}
