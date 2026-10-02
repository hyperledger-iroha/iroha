//! Mathematical ordinary projection and exact data-framing regressions; no Native authority.
use super::*;
use crate::kagemusha_v1_poseidon::encode;
fn projection(operation: u8) -> OrdinaryCashTerminalPublicV1 {
    let send = operation == 2;
    OrdinaryCashTerminalPublicV1 {
        operation,
        suite_id: [1; 32],
        vk_set_digest: [2; 32],
        release_id: [3; 32],
        network_id: [4; 32],
        asset_id: [5; 32],
        asset_incarnation: [6; 32],
        asset_scale: 7,
        liability_pool_id: [8; 32],
        app_credential_profile_id: [9; 32],
        policy_epoch: 10,
        lifecycle_digest: [11; 32],
        body_digest: [12; 32],
        candidate_digest: [13; 32],
        terminal_record_digest: [14; 32],
        transition_nullifier: [15; 32],
        request_digest: if send { [16; 32] } else { [0; 32] },
        receiver_credential_digest: if send { [17; 32] } else { [0; 32] },
        ciphertext_commitment: if send { [18; 32] } else { [0; 32] },
        amount: (1u128 << 100) + 19,
        output_binding_digest: [20; 32],
        redemption_manifest_digest: if send { [0; 32] } else { [21; 32] },
        eq_deferred_audit: [22; 32],
        ep_deferred_audit: [23; 32],
        eq_protocol_digest: encode(Fp::from(24)),
        ep_protocol_digest: encode(Fq::from(25)),
    }
}
fn prefix<F: KagemushaPoseidonFieldV1>() {
    for operation in [2, 4] {
        let p = projection(operation);
        let out = p.public_prefix::<F>().unwrap();
        assert_eq!(out.len(), 49);
        assert_eq!(out[0], F::from(u64::from(operation)));
        assert_eq!(out[1], F::ONE);
        // Exact ordinary slots: body22, candidate24, logical terminal record26, amount36.
        for (position, value) in [
            (17, p.app_credential_profile_id),
            (22, p.body_digest),
            (24, p.candidate_digest),
            (26, p.terminal_record_digest),
            (28, p.transition_nullifier),
            (37, p.output_binding_digest),
        ] {
            assert_eq!(out[position..position + 2], digest_limbs::<F>(value));
        }
        assert_eq!(out[36], from_u128::<F>((1u128 << 100) + 19));
        let mut history = [0u8; 544];
        for (i, chunk) in history.chunks_exact_mut(16).enumerate() {
            chunk.copy_from_slice(&(1u128 << 99 | i as u128).to_le_bytes());
        }
        let column = p.public_column::<F>(&history).unwrap();
        assert_eq!(column.len(), 83);
        assert_eq!(&column[..49], &out);
        for i in 0..34 {
            assert_eq!(column[49 + i], from_u128::<F>(1u128 << 99 | i as u128));
        }
        let mut changed = p.clone();
        changed.terminal_record_digest[31] ^= 1;
        let next = changed.public_prefix::<F>().unwrap();
        assert_eq!(&out[..26], &next[..26]);
        assert_ne!(&out[26..28], &next[26..28]);
        assert_eq!(&out[28..], &next[28..]);
        let mut changed = p.clone();
        changed.body_digest[0] ^= 1;
        assert_ne!(out, changed.public_prefix::<F>().unwrap());
        let mut changed = p.clone();
        changed.amount ^= 1u128 << 127;
        assert_ne!(out, changed.public_prefix::<F>().unwrap());
    }
}
#[test]
fn ordinary_terminal_prefix_exact_named_slots_and_full128_history_both_fields() {
    prefix::<Fp>();
    prefix::<Fq>();
}
#[test]
fn ordinary_terminal_projection_rejects_inactive_or_noncanonical_parity_roles() {
    for op in [0, 1, 3, 5, 6, 255] {
        assert!(projection(op).validate().is_err());
    }
    for op in [2, 4] {
        let p = projection(op);
        for mutation in 0..9 {
            let mut q = p.clone();
            match mutation {
                0 => q.amount = 0,
                1 => q.policy_epoch = 0,
                2 => q.body_digest = [0; 32],
                3 => q.terminal_record_digest = [0; 32],
                4 => q.eq_deferred_audit = q.ep_deferred_audit,
                5 => q.eq_protocol_digest = q.ep_protocol_digest,
                6 => q.eq_protocol_digest = [255; 32],
                7 => q.request_digest = if op == 2 { [0; 32] } else { [2; 32] },
                _ => q.redemption_manifest_digest = if op == 2 { [2; 32] } else { [0; 32] },
            }
            assert!(q.validate().is_err(), "operation {op} mutation {mutation}");
        }
    }
}
fn wire(relation: u8) -> OrdinaryCashProofPairWireV1 {
    OrdinaryCashProofPairWireV1 {
        version: 1,
        relation,
        release_id: [1; 32],
        artifact_manifest_digest: [2; 32],
        eq_protocol_digest: encode(Fp::from(3)),
        ep_protocol_digest: encode(Fq::from(4)),
        eq_deferred_audit: [5; 32],
        ep_deferred_audit: [6; 32],
        eq_proof: vec![7; 64],
        ep_proof: vec![8; 96],
        eq_history: [9; 544],
        ep_history: [10; 544],
    }
}
fn decode(
    w: &OrdinaryCashProofPairWireV1,
    bytes: &[u8],
    relation: u8,
) -> Result<OrdinaryCashProofPairWireV1> {
    decode_profile_exact(
        bytes,
        relation,
        [1; 32],
        [2; 32],
        [encode(Fp::from(3)), encode(Fq::from(4))],
        [w.eq_proof.len(), w.ep_proof.len()],
    )
}
#[test]
fn ordinary_pair_data_codec_exact_roundtrip_and_phase_release_protocol_length_mutations() {
    for relation in [1, 2] {
        let w = wire(relation);
        let bytes = norito::encode_canonical(&w).unwrap();
        assert_eq!(decode(&w, &bytes, relation).unwrap(), w);
        assert!(decode(&w, &bytes, 3 - relation).is_err());
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode(&w, &extra, relation).is_err());
        assert!(decode(&w, &bytes[..bytes.len() - 1], relation).is_err());
        for mutation in 0..9 {
            let mut bad = w.clone();
            match mutation {
                0 => bad.version = 2,
                1 => bad.relation = 3,
                2 => bad.release_id[0] ^= 1,
                3 => bad.artifact_manifest_digest[31] ^= 1,
                4 => bad.eq_protocol_digest[0] ^= 1,
                5 => bad.ep_protocol_digest[31] ^= 1,
                6 => bad.eq_proof.pop().map(|_| ()).unwrap(),
                7 => bad.ep_proof.push(0),
                _ => bad.eq_deferred_audit = bad.ep_deferred_audit,
            }
            let raw = norito::encode_canonical(&bad).unwrap();
            assert!(
                decode(&w, &raw, relation).is_err(),
                "phase {relation} mutation {mutation}"
            );
        }
    }
}
#[test]
fn ordinary_pair_compact_phase_has_distinct_full_original_and_bound() {
    let private = wire(1);
    let mut compact = wire(2);
    assert_ne!(
        norito::encode_canonical(&private).unwrap(),
        norito::encode_canonical(&compact).unwrap()
    );
    compact.eq_proof = vec![7; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 + 1];
    let original = norito::encode_canonical(&compact).unwrap();
    assert!(decode(&compact, &original, 2).is_err());
    let mut retained = compact.clone();
    retained.relation = 1;
    let original = norito::encode_canonical(&retained).unwrap();
    assert!(decode(&retained, &original, 1).is_ok()); // Data framing only; no real proof/authority is claimed.
}
