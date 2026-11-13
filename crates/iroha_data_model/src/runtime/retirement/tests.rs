//! Canonical retirement transport and inclusion checks; no producer-authority shortcut.

use super::*;
use iroha_crypto::HashOf;

fn proof() -> RuntimeRetirementProofV1 {
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"root")));
    RuntimeRetirementProofV1 {
        receipt: RuntimeRetirementReceiptV1 {
            root: RuntimeRetirementRootV1 {
                network,
                scope: SumeragiRootScope::Global,
                instance: [3; 32],
            },
            definition: DefinitionId([4; 32]),
            history_started: 2,
            last_required: Some(3),
            excluded_at: 4,
            startup_floor: StartupFloorV1 {
                network,
                height: 4,
                block_hash: Hash::new(b"floor").into(),
                committed_ms: 4_000,
            },
            receipt_height: 5,
        },
        block: AmxCertifiedBlockV1 {
            consensus_header: vec![1],
            commit_qc: vec![2],
            result_preimage: vec![3],
        },
        path: AmxWriteProofV1 {
            present: [0; 32],
            siblings: Vec::new(),
        },
    }
}

#[test]
fn retirement_receipt_and_proof_roundtrip_canonical_binary_and_json() {
    let proof = proof();
    proof.validate_bounds().unwrap();
    let bytes = norito::encode_canonical(&proof).unwrap();
    assert_eq!(
        norito::decode_canonical::<RuntimeRetirementProofV1>(&bytes).unwrap(),
        proof
    );
    let json = norito::json::to_json(&proof).unwrap();
    assert_eq!(
        norito::json::from_str::<RuntimeRetirementProofV1>(&json).unwrap(),
        proof
    );
    let mut never = proof.receipt.clone();
    never.last_required = None;
    never.validate().unwrap();
    let bytes = norito::encode_canonical(&never).unwrap();
    assert_eq!(
        norito::decode_canonical::<RuntimeRetirementReceiptV1>(&bytes).unwrap(),
        never
    );
    let row = RuntimeRetirementStateV1 {
        history_started: proof.receipt.history_started,
        last_required: proof.receipt.last_required,
        governed_excluded_at: Some(proof.receipt.excluded_at),
        receipt: Some(proof.receipt.clone()),
    };
    row.validate(proof.receipt.definition).unwrap();
    assert_eq!(
        norito::decode_canonical::<RuntimeRetirementStateV1>(
            &norito::encode_canonical(&row).unwrap()
        )
        .unwrap(),
        row
    );
    let mut changed = row.clone();
    changed.last_required = Some(2);
    assert!(changed.validate(proof.receipt.definition).is_err());
    let mut changed = row.clone();
    changed.governed_excluded_at = None;
    assert!(changed.validate(proof.receipt.definition).is_err());
    let json = norito::json::to_json(&row).unwrap();
    assert_eq!(
        norito::json::from_str::<RuntimeRetirementStateV1>(&json).unwrap(),
        row
    );
    let mut value: norito::json::Value = norito::json::from_str(&json).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("governed_excluded_at");
    assert!(norito::json::from_value::<RuntimeRetirementStateV1>(value).is_err());
    assert!(row.validate(DefinitionId([9; 32])).is_err());
    let instruction: crate::isi::InstructionBox =
        crate::isi::runtime_retirement::RetireRuntimeDefinitionV1 {
            definition: proof.receipt.definition,
        }
        .into();
    let wire = norito::encode_canonical(&instruction).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<crate::isi::InstructionBox>(&wire).unwrap(),
        instruction
    );
    let json = norito::json::to_json(&instruction).unwrap();
    assert_eq!(
        norito::json::from_str::<crate::isi::InstructionBox>(&json).unwrap(),
        instruction
    );
}

#[test]
fn retirement_inclusion_binds_every_root_target_carrier_and_floor_field() {
    let original = proof();
    let receipt = &original.receipt;
    let value = norito::encode_canonical(receipt).unwrap();
    let root = original.path.root(&receipt.witness_key(), &value).unwrap();
    let verify = |proof: &RuntimeRetirementProofV1| {
        proof.verify_inclusion(
            receipt.root,
            receipt.definition,
            receipt.receipt_height,
            root,
        )
    };
    verify(&original).unwrap();
    let mut changed = original.clone();
    changed.receipt.root.instance[0] ^= 1;
    assert!(verify(&changed).is_err());
    changed = original.clone();
    changed.receipt.definition.0[0] ^= 1;
    assert!(verify(&changed).is_err());
    assert_ne!(changed.receipt.witness_key(), receipt.witness_key());
    changed = original.clone();
    changed.receipt.receipt_height += 1;
    assert!(verify(&changed).is_err());
    changed = original.clone();
    changed.receipt.startup_floor.block_hash[0] ^= 1;
    assert!(verify(&changed).is_err());
    changed = original.clone();
    changed.receipt.last_required = None;
    assert!(verify(&changed).is_err());
    changed = original.clone();
    changed.path.siblings.push([7; 32]);
    assert!(verify(&changed).is_err());
}

#[test]
fn retirement_floor_must_exceed_last_code_needed_use_and_follow_history() {
    let original = proof();
    for last in [0, 1, 4, 5, u64::MAX] {
        let mut receipt = original.receipt.clone();
        receipt.last_required = Some(last);
        assert!(receipt.validate().is_err());
    }
    let mut receipt = original.receipt.clone();
    receipt.startup_floor.height = 3;
    assert!(receipt.validate().is_err());
    receipt = original.receipt.clone();
    receipt.excluded_at = 6;
    assert!(receipt.validate().is_err());
    receipt = original.receipt.clone();
    receipt.history_started = 0;
    assert!(receipt.validate().is_err());
}

#[test]
fn retirement_proof_limits_preserve_original_codec_refusal() {
    let mut original = proof();
    original.block.consensus_header.resize(8193, 0);
    assert!(matches!(
        original.validate_bounds(),
        Err(AmxError::Proof(_))
    ));
    let original = proof();
    let value = norito::encode_canonical(&original.receipt).unwrap();
    let root = original
        .path
        .root(&original.receipt.witness_key(), &value)
        .unwrap();
    let wire = norito::encode_canonical(&original).unwrap();
    let refusal = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || {
            norito::core::classify_decode_attempt(|| {
                norito::decode_canonical::<RuntimeRetirementProofV1>(&wire)
            })
            .map_err(super::encoding)
        },
    );
    assert!(matches!(refusal, Err(AmxError::Resource(_))));
    original
        .verify_inclusion(original.receipt.root, original.receipt.definition, 5, root)
        .unwrap();
}
