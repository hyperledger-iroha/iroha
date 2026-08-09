// Same-scope regression coverage extracted to keep the parent source budget bounded.

#[test]
fn anchor_drawdown_is_canonical_bounded_and_cross_anchor() {
    let balances = [
        KagemushaV4AnchorDrawdownBalance {
            operation_id: [0x11; 32],
            capacity_atomic_units: 100,
            redeemed_atomic_units: 20,
        },
        KagemushaV4AnchorDrawdownBalance {
            operation_id: [0x22; 32],
            capacity_atomic_units: 50,
            redeemed_atomic_units: 10,
        },
    ];
    assert_eq!(
        allocate_kagemusha_v4_anchor_drawdown(&balances, 110),
        Some(vec![([0x11; 32], 100), ([0x22; 32], 40)])
    );
    assert_eq!(
        allocate_kagemusha_v4_anchor_drawdown(&balances, 120),
        Some(vec![([0x11; 32], 100), ([0x22; 32], 50)])
    );
    assert!(
        allocate_kagemusha_v4_anchor_drawdown(&balances, 121).is_none(),
        "redemption must not exceed aggregate unredeemed provenance"
    );
    assert!(
        allocate_kagemusha_v4_anchor_drawdown(&balances, 0).is_none(),
        "zero-value drawdown must not produce a state update"
    );

    let corrupt = [KagemushaV4AnchorDrawdownBalance {
        operation_id: [0x33; 32],
        capacity_atomic_units: 1,
        redeemed_atomic_units: 2,
    }];
    assert!(
        allocate_kagemusha_v4_anchor_drawdown(&corrupt, 1).is_none(),
        "a persisted drawdown above its anchor must fail closed"
    );

    let duplicate = [
        KagemushaV4AnchorDrawdownBalance {
            operation_id: [0x44; 32],
            capacity_atomic_units: 100,
            redeemed_atomic_units: 0,
        },
        KagemushaV4AnchorDrawdownBalance {
            operation_id: [0x44; 32],
            capacity_atomic_units: 100,
            redeemed_atomic_units: 0,
        },
    ];
    assert!(
        allocate_kagemusha_v4_anchor_drawdown(&duplicate, 101).is_none(),
        "duplicate anchor identities must fail closed before allocation"
    );
}

#[test]
fn anchor_drawdown_state_is_paired_sequential_and_rollback_safe() {
    let operation_id = [0x45; 32];
    let state = offline_test_state();
    let mut block = state.block(offline_test_header());
    let mut transaction = block.transaction();
    let anchor_archive = b"validated-anchor-archive".to_vec();
    persist_kagemusha_v4_topup_anchor_archive(
        operation_id,
        [0x46; 32],
        anchor_archive.clone(),
        &mut transaction,
    )
    .expect("paired anchor/drawdown initialization");

    let anchor_key = kagemusha_v4_topup_anchor_state_key(operation_id).expect("anchor state key");
    let drawdown_key =
        kagemusha_v4_topup_drawdown_state_key(operation_id).expect("drawdown state key");
    assert_eq!(
        transaction.world.smart_contract_state.get(&anchor_key),
        Some(&anchor_archive),
    );
    assert_eq!(
        transaction.world.smart_contract_state.get(&drawdown_key),
        Some(&0_u128.to_le_bytes().to_vec()),
        "paired initialization must persist an exact zero u128",
    );

    let first =
        plan_kagemusha_v4_anchor_drawdown_capacities(&[(operation_id, 100)], 40, &transaction)
            .expect("first drawdown plan");
    commit_kagemusha_v4_anchor_drawdown(first, &mut transaction);
    assert_eq!(
        load_kagemusha_v4_topup_drawdown(operation_id, &transaction)
            .expect("first persisted drawdown"),
        40,
    );

    let second =
        plan_kagemusha_v4_anchor_drawdown_capacities(&[(operation_id, 100)], 60, &transaction)
            .expect("second drawdown plan");
    commit_kagemusha_v4_anchor_drawdown(second, &mut transaction);
    assert_eq!(
        load_kagemusha_v4_topup_drawdown(operation_id, &transaction)
            .expect("cumulative persisted drawdown"),
        100,
    );

    let assets_before = offline_asset_entries(&transaction);
    let confidential_before = transaction
        .world
        .zk_assets
        .iter()
        .map(|(id, state)| {
            (
                id.clone(),
                state.tree_profile,
                state.commitments.clone(),
                state.root_history.clone(),
                state.nullifiers.clone(),
            )
        })
        .collect::<Vec<_>>();
    let branch_and_replay_before = transaction
        .world
        .kagemusha_replay_keys
        .iter()
        .map(|(key, ())| *key)
        .collect::<Vec<_>>();
    let receipt_key = kagemusha_v4_redemption_receipt_state_key([0x47; 32]).expect("receipt key");
    let receipt_before = transaction
        .world
        .smart_contract_state
        .get(&receipt_key)
        .cloned();
    let events_before = transaction.world.internal_event_buf.len();

    let overdraw =
        plan_kagemusha_v4_anchor_drawdown_capacities(&[(operation_id, 100)], 1, &transaction)
            .expect_err("one unit beyond cumulative capacity must fail");
    assert!(overdraw.to_string().contains("topup_drawdown_exhausted"));
    assert_eq!(offline_asset_entries(&transaction), assets_before);
    assert_eq!(
        transaction
            .world
            .zk_assets
            .iter()
            .map(|(id, state)| {
                (
                    id.clone(),
                    state.tree_profile,
                    state.commitments.clone(),
                    state.root_history.clone(),
                    state.nullifiers.clone(),
                )
            })
            .collect::<Vec<_>>(),
        confidential_before,
        "rejected drawdown must not change a nullifier or tree",
    );
    assert_eq!(
        transaction
            .world
            .kagemusha_replay_keys
            .iter()
            .map(|(key, ())| *key)
            .collect::<Vec<_>>(),
        branch_and_replay_before,
        "rejected drawdown must not consume branch/replay markers",
    );
    assert_eq!(
        transaction
            .world
            .smart_contract_state
            .get(&receipt_key)
            .cloned(),
        receipt_before,
        "rejected drawdown must not create a receipt",
    );
    assert_eq!(
        load_kagemusha_v4_topup_drawdown(operation_id, &transaction)
            .expect("unchanged exhausted drawdown"),
        100,
    );
    assert_eq!(transaction.world.internal_event_buf.len(), events_before);

    transaction
        .world
        .smart_contract_state
        .remove(drawdown_key.clone());
    assert!(
        plan_kagemusha_v4_anchor_drawdown_capacities(&[(operation_id, 100)], 1, &transaction,)
            .expect_err("orphan anchor must fail closed")
            .to_string()
            .contains("topup_drawdown_missing"),
    );
    transaction
        .world
        .smart_contract_state
        .insert(drawdown_key, vec![0; 15]);
    assert!(
        plan_kagemusha_v4_anchor_drawdown_capacities(&[(operation_id, 100)], 1, &transaction,)
            .expect_err("malformed drawdown must fail closed")
            .to_string()
            .contains("topup_drawdown_invalid"),
    );
}

#[test]
fn offline_proof_boundary_rejects_alternate_norito_layout() {
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: "halo2/pasta/ipa/offline-boundary-v1".to_owned(),
        vk_hash: [7_u8; 32],
        public_inputs: b"offline-boundary-v1".to_vec(),
        proof_bytes: vec![1, 2, 3],
        aux: Vec::new(),
    };
    let canonical = norito::encode_canonical(&envelope).expect("canonical offline proof envelope");
    assert_eq!(
        decode_canonical_offline_proof_envelope(&canonical, "fixture")
            .expect("canonical envelope must decode"),
        envelope
    );

    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&envelope).expect("alternate-layout offline proof envelope")
    };
    assert_ne!(alternate, canonical);
    let error =
        decode_canonical_offline_proof_envelope(&alternate, "alternate offline envelope rejected")
            .expect_err("alternate-layout envelope must fail closed");
    assert!(
        error
            .to_string()
            .contains("alternate offline envelope rejected"),
        "unexpected boundary error: {error}"
    );
}

#[test]
fn kagemusha_v4_chain_state_namespaces_are_version_distinct() {
    let operation_id = [0x41; 32];
    assert_ne!(
        kagemusha_v2_marker(KAGEMUSHA_V4_OPERATION_DOMAIN, &[&operation_id]),
        kagemusha_v2_marker("kagemusha-v2-operation", &[&operation_id]),
    );
    assert_ne!(
        kagemusha_v2_marker(KAGEMUSHA_V4_BRANCH_EXACT_DOMAIN, &[&operation_id]),
        kagemusha_v2_marker("kagemusha-v2-redeemed-branch", &[&operation_id]),
    );
    assert!(
        kagemusha_v4_topup_anchor_state_key(operation_id)
            .expect("valid V4 anchor key")
            .to_string()
            .starts_with("kagemusha_v4_topup_anchor_")
    );
    assert!(
        kagemusha_v4_redemption_receipt_state_key(operation_id)
            .expect("valid V4 redemption receipt key")
            .to_string()
            .starts_with("kagemusha_v4_redemption_")
    );
}

#[test]
fn kagemusha_topup_note_freshness_rejects_all_state_namespace_collisions() {
    const EXISTING_COMMITMENT: [u8; 32] = [0x51; 32];
    const SPENT_NULLIFIER: [u8; 32] = [0x52; 32];
    const FRESH_COMMITMENT: [u8; 32] = [0x53; 32];
    const FRESH_NULLIFIER: [u8; 32] = [0x54; 32];

    let mut zk_state = crate::state::ZkAssetState::default();
    zk_state.commitments.push(EXISTING_COMMITMENT);
    assert!(zk_state.nullifiers.insert(SPENT_NULLIFIER));

    ensure_kagemusha_v4_topup_note_is_fresh(&zk_state, FRESH_COMMITMENT, FRESH_NULLIFIER)
        .expect("disjoint top-up note material must remain admissible");

    for (note_commitment, spend_nullifier, expected_label) in [
        (EXISTING_COMMITMENT, FRESH_NULLIFIER, "duplicate_output"),
        (FRESH_COMMITMENT, SPENT_NULLIFIER, "duplicate_nullifier"),
        (FRESH_COMMITMENT, EXISTING_COMMITMENT, "duplicate_nullifier"),
        (SPENT_NULLIFIER, FRESH_NULLIFIER, "proof_binding"),
    ] {
        let error =
            ensure_kagemusha_v4_topup_note_is_fresh(&zk_state, note_commitment, spend_nullifier)
                .expect_err("every commitment/nullifier namespace collision must fail closed");
        assert!(
            error.to_string().contains(&format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}{expected_label}:"
            )),
            "unexpected collision rejection for {expected_label}: {error}"
        );
    }
}

#[test]
fn kagemusha_v4_admission_authenticates_exact_release_without_global_backend_flag() {
    let source = include_str!("../offline.rs");
    let topup_start = source
        .find("impl Execute for TopUpKagemushaRecursiveV4")
        .expect("V4 top-up executor");
    let redeem_start = source
        .find("impl Execute for RedeemKagemushaRecursiveV4")
        .expect("V4 redemption executor");
    let tests_start = redeem_start
        + source[redeem_start..]
            .find("#[cfg(test)]")
            .expect("offline executor test module");
    let topup = &source[topup_start..redeem_start];
    let redeem = &source[redeem_start..tests_start];

    for (name, executor) in [("top-up", topup), ("redemption", redeem)] {
        assert!(
            !executor.contains("KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE"),
            "V4 {name} must authenticate a concrete release instead of treating compile capability as runtime readiness",
        );
        assert!(
            executor.contains("resolve_kagemusha_v4_transaction_release"),
            "V4 {name} must resolve the transaction-selected authenticated release",
        );
    }
    assert!(
        redeem.contains("verify_kagemusha_v4_recursive_bundle"),
        "full and partial redemption must verify the parent recursive bundle",
    );
    assert!(
        redeem.contains("verify_bundle_operation_v4"),
        "partial redemption must separately verify its operation-bound change bundle",
    );
}
