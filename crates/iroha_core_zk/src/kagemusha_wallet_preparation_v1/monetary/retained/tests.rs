//! DATA carrier and full-slot restart tests; these do not admit monetary proofs.
use super::*;

fn frozen() -> FrozenTransition {
    let all: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../../fixtures/kagemusha/wallet_v1_vectors.json"
    ))
    .unwrap();
    let row = all["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["type"].as_str() == Some("KagemushaWalletRecoveryCapsuleV1")
                && row["variant"].as_str() == Some("Receive")
        })
        .unwrap();
    let capsule: KagemushaWalletRecoveryCapsuleV1 =
        norito::decode_from_bytes(&hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap())
            .unwrap();
    let request: KagemushaWalletRequestV1 = norito::decode_from_bytes(
        exact_retained(&capsule, KagemushaWalletRetainedInputRoleV1::Request).unwrap(),
    )
    .unwrap();
    FrozenTransition {
        credential: request.receiver_credential,
        capsule,
    }
}
fn carrier(frozen: &FrozenTransition) -> Payload {
    Payload {
        version: 1,
        predecessor_capsule_digest: frozen.capsule.predecessor_capsule_digest,
        manifest_digest: [7; 32],
        statement_digest: frozen.capsule.statement.statement_digest().unwrap(),
        enrollment: vec![7; 16],
        send: None,
        receive: Some(Receive {
            recorded_history: None,
            recorded_gap: None,
            fold_history: KagemushaWalletIndexedTreeV1::new()
                .opening(0)
                .leaf_transcript(&KagemushaWalletIndexedLeafV1::SENTINEL),
        }),
    }
}
fn attach(frozen: &mut FrozenTransition, carrier: Payload) {
    let retained = MonetaryRetentionV1::encode(carrier).unwrap();
    frozen
        .capsule
        .retained_inputs
        .push(KagemushaWalletRetainedInputV1 {
            role: KagemushaWalletRetainedInputRoleV1::MonetaryWitness,
            bytes: retained.original,
        });
}
#[test]
fn exact_source_carrier_rejects_missing_duplicate_corrupted_and_cross_operation_originals() {
    let original = frozen();
    assert!(payload(&original).is_err());
    let mut valid = original.clone();
    attach(&mut valid, carrier(&original));
    let decoded = payload(&valid).unwrap();
    assert_eq!(
        decoded.predecessor_capsule_digest,
        original.capsule.predecessor_capsule_digest
    );
    assert_eq!(
        decoded.statement_digest,
        original.capsule.statement.statement_digest().unwrap()
    );
    let mut duplicate = valid.clone();
    duplicate
        .capsule
        .retained_inputs
        .push(valid.capsule.retained_inputs.last().unwrap().clone());
    assert!(payload(&duplicate).is_err());
    for field in 0..4 {
        let mut data = carrier(&original);
        match field {
            0 => data.version = 2,
            1 => data.predecessor_capsule_digest[0] ^= 1,
            2 => data.statement_digest[0] ^= 1,
            _ => {
                data.send = Some(Send {
                    observed: None,
                    anchored_original: None,
                    blacklist: None,
                    quota_share_original: None,
                    quota_usage_original: vec![0],
                })
            }
        };
        let mut changed = original.clone();
        attach(&mut changed, data);
        assert!(payload(&changed).is_err());
    }
    for malformed in [vec![0], vec![0; MAX_BYTES + 1]] {
        let mut changed = original.clone();
        changed
            .capsule
            .retained_inputs
            .push(KagemushaWalletRetainedInputV1 {
                role: KagemushaWalletRetainedInputRoleV1::MonetaryWitness,
                bytes: malformed,
            });
        assert!(payload(&changed).is_err());
    }
}
#[test]
fn quota_restart_carrier_preserves_all64_original_slots_and_detects_changed_root() {
    let slots = core::array::from_fn(|i| {
        Some(KagemushaWalletQuotaUsageLeafV1 {
            window_kind: KagemushaWalletQuotaWindowKindV1::Daily,
            window_start_ms: i as u64 * 100,
            window_end_ms: i as u64 * 100 + 99,
            used: i as u128 * 19,
        })
    });
    let usage = KagemushaWalletQuotaUsageArrayV1::from_slots(slots).unwrap();
    let original = norito::encode_canonical(&usage.slots().to_vec()).unwrap();
    let decoded: Vec<Option<KagemushaWalletQuotaUsageLeafV1>> =
        norito::decode_canonical_with_limits(&original, norito::canonical_decode_limits(MAX_BYTES))
            .unwrap();
    assert_eq!(decoded.len(), 64);
    let restored =
        KagemushaWalletQuotaUsageArrayV1::from_slots(decoded.try_into().unwrap()).unwrap();
    assert_eq!(restored, usage);
    assert_eq!(restored.root(), usage.root());
    let mut changed = *restored.slots();
    changed[63].as_mut().unwrap().used += 1;
    assert_ne!(
        KagemushaWalletQuotaUsageArrayV1::from_slots(changed)
            .unwrap()
            .root(),
        usage.root()
    );
}
