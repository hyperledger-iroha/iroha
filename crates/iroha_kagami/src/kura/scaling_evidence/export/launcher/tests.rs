//! Real signed launch requests, exact first-release frames and admission failures.

use super::*;
use norito::codec::Encode as _;

#[path = "../../fixture.rs"]
mod fixture;

fn bindings(f: &fixture::Fixture) -> Vec<HeightInputBinding> {
    vec![
        HeightInputBinding {
            height: 1,
            finality_hash: Hash::new(norito::encode_canonical(&f.first).unwrap()),
            query_hashes: vec![],
        },
        HeightInputBinding {
            height: 2,
            finality_hash: Hash::new(norito::encode_canonical(&f.second).unwrap()),
            query_hashes: f.queries().iter().map(Hash::new).collect(),
        },
    ]
}

fn request(f: &fixture::Fixture) -> RequestV1 {
    RequestV1::from_parts(f.plan(), fixture::limits(), bindings(f)).unwrap()
}

fn frame(f: &fixture::Fixture) -> Vec<u8> {
    encode(f.plan(), fixture::limits(), bindings(f), MAX_PROOF_BYTES).unwrap()
}

fn decode_frame(bytes: &[u8]) -> Result<BoundLauncherRequest> {
    decode(bytes, iroha_crypto::sha256(bytes), MAX_PROOF_BYTES)
}

#[test]
fn roundtrip_preserves_every_independent_fact_and_actual_signed_request() {
    for lanes in [1, 4] {
        let f = fixture::Fixture::new(lanes);
        let original = f.plan();
        let bytes = frame(&f);
        let (decoded, limits, actual) = decode_frame(&bytes).unwrap().into_parts();
        assert_eq!(decoded.network_id, original.network_id);
        assert_eq!(decoded.first_context, original.first_context);
        assert_eq!(decoded.first_height, original.first_height);
        assert_eq!(decoded.last_height, original.last_height);
        assert_eq!(decoded.lane_catalog_hash, original.lane_catalog_hash);
        assert_eq!(
            norito::encode_canonical(&decoded.active_lanes).unwrap(),
            norito::encode_canonical(&original.active_lanes).unwrap()
        );
        assert_eq!(
            norito::encode_canonical(&decoded.lane_authorities).unwrap(),
            norito::encode_canonical(&original.lane_authorities).unwrap()
        );
        assert_eq!(decoded.scheduled.len(), original.scheduled.len());
        assert!(!decoded.scheduled.is_empty());
        for (got, expected) in decoded.scheduled.iter().zip(&original.scheduled) {
            assert_eq!(got.logical_id, expected.logical_id);
            assert_eq!(got.phase, expected.phase);
            assert_eq!(got.route, expected.route);
            assert_eq!(got.signed_transaction, expected.signed_transaction);
        }
        let expected = bindings(&f);
        assert_eq!(actual.len(), expected.len());
        for (got, expected) in actual.iter().zip(expected) {
            assert_eq!(got.height, expected.height);
            assert_eq!(got.finality_hash, expected.finality_hash);
            assert_eq!(got.query_hashes, expected.query_hashes);
        }
        assert_eq!(
            limits.admitted_proof_bytes,
            fixture::limits().admitted_proof_bytes
        );
        assert_eq!(limits.input_bytes, fixture::limits().input_bytes);
        assert_eq!(limits.output_bytes, fixture::limits().output_bytes);
        assert_eq!(limits.heights, fixture::limits().heights);
        assert_eq!(limits.requests, fixture::limits().requests);
        assert_eq!(
            limits.leaves_per_carrier,
            fixture::limits().leaves_per_carrier
        );
        assert_eq!(
            encode(decoded, limits, actual, bytes.len() as u64).unwrap(),
            bytes
        );
    }
}

#[test]
fn count_preflight_obeys_exact_cap_and_rejects_one_byte_short() {
    let f = fixture::Fixture::new(1);
    let bytes = frame(&f);
    assert_eq!(
        norito::canonical_frame_len(&request(&f)).unwrap(),
        bytes.len()
    );
    assert_eq!(
        encode(
            f.plan(),
            fixture::limits(),
            bindings(&f),
            bytes.len() as u64
        )
        .unwrap(),
        bytes
    );
    assert!(
        encode(
            f.plan(),
            fixture::limits(),
            bindings(&f),
            bytes.len() as u64 - 1
        )
        .is_err()
    );
    assert!(decode(&bytes, iroha_crypto::sha256(&bytes), bytes.len() as u64).is_ok());
    for cap in [0, bytes.len() as u64 - 1, MAX_PROOF_BYTES + 1, u64::MAX] {
        assert!(decode(&bytes, iroha_crypto::sha256(&bytes), cap).is_err());
    }
    assert!(decode_frame(&[]).is_err());
}

#[test]
fn wrong_external_raw_digest_rejects_before_malformed_norito_decode() {
    let invalid = b"not a Norito frame";
    let error = decode(invalid, [0; 32], 1024).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("launcher request digest mismatch")
    );
    let f = fixture::Fixture::new(1);
    assert!(decode(&frame(&f), [0; 32], MAX_PROOF_BYTES).is_err());
}

#[test]
fn bare_payload_trailer_truncation_and_other_versions_have_no_fallback() {
    let f = fixture::Fixture::new(1);
    let bytes = frame(&f);
    assert!(decode_frame(&request(&f).encode()).is_err());
    assert!(decode_frame(&bytes[..bytes.len() - 1]).is_err());
    let mut trailer = bytes.clone();
    trailer.push(0);
    assert!(decode_frame(&trailer).is_err());
    for version in [0, 2, u16::MAX] {
        let mut changed = request(&f);
        changed.version = version;
        let changed = norito::encode_canonical(&changed).unwrap();
        assert!(
            decode_frame(&changed)
                .err()
                .unwrap()
                .to_string()
                .contains("unsupported launcher request version")
        );
    }
}

#[test]
fn strict_wire_work_bounds_reject_before_host_integer_narrowing() {
    let f = fixture::Fixture::new(1);
    for bound in [0, MAX_REQUESTS as u64 + 1, u64::MAX] {
        for leaves in [false, true] {
            let mut changed = request(&f);
            if leaves {
                changed.limits.leaves_per_carrier = bound;
            } else {
                changed.limits.requests = bound;
            }
            assert!(changed.into_parts().is_err());
        }
    }
}

#[test]
fn normal_admission_rejects_tampered_signed_bytes_and_route() {
    let f = fixture::Fixture::new(4);
    let mut changed = request(&f);
    changed.plan.scheduled[0].signed_transaction = b"headerless transaction".to_vec();
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
    let mut changed = request(&f);
    changed.plan.scheduled[0].route.lane_id = LaneId::new(900);
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
    let mut plan = f.plan();
    plan.scheduled[0].route.lane_id = LaneId::new(900);
    assert!(encode(plan, fixture::limits(), bindings(&f), MAX_PROOF_BYTES).is_err());
}

#[test]
fn actual_schedule_admission_rejects_missing_duplicate_and_phase_reentry() {
    let f = fixture::Fixture::new(1);
    let mut changed = request(&f);
    changed.plan.scheduled.clear();
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
    let mut changed = request(&f);
    changed.plan.scheduled[1].logical_id = changed.plan.scheduled[0].logical_id.clone();
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
    let mut changed = request(&f);
    changed.plan.scheduled.last_mut().unwrap().phase = WorkloadPhase::Warmup;
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
}

#[test]
fn binding_interval_and_count_use_the_actual_adapter_admission() {
    let f = fixture::Fixture::new(1);
    let mut changed = request(&f);
    changed.bindings.swap(0, 1);
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
    let mut changed = request(&f);
    changed.bindings.pop();
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
    let mut changed = request(&f);
    changed.bindings[1].query_hashes = vec![Hash::new(b"query"); fixture::limits().requests + 1];
    assert!(decode_frame(&norito::encode_canonical(&changed).unwrap()).is_err());
}

#[test]
fn readmitted_plan_still_requires_every_real_signed_finality_and_effect() {
    let f = fixture::Fixture::new(4);
    let (plan, limits, _) = decode_frame(&frame(&f)).unwrap().into_parts();
    let mut verifier = f.start(plan, limits);
    f.push(&mut verifier).unwrap();
    let complete = verifier.finish().unwrap();
    assert_eq!(complete.rows().len(), f.plan().scheduled.len());
    assert!(
        complete
            .rows()
            .iter()
            .any(|row| row.lane_id == LaneId::new(3))
    );
    let (plan, limits, _) = decode_frame(&frame(&f)).unwrap().into_parts();
    assert!(
        ScalingProofVerifier::new(plan, limits)
            .unwrap()
            .finish()
            .is_err()
    );
}

#[test]
fn misaligned_canonical_request_obeys_exact_raw_cap_without_changing_semantics() {
    let f = fixture::Fixture::new(4);
    let bytes = frame(&f);
    let digest = iroha_crypto::sha256(&bytes);
    let mut misaligned = 0;
    for offset in 0..16 {
        let mut backing = vec![0u8; offset + bytes.len()];
        backing[offset..].copy_from_slice(&bytes);
        let slice = &backing[offset..];
        if slice.as_ptr() as usize % std::mem::align_of::<u64>() != 0 {
            misaligned += 1;
        }
        let (plan, limits, bindings) = decode(slice, digest, bytes.len() as u64)
            .unwrap()
            .into_parts();
        assert_eq!(plan.scheduled.len(), f.requests.len());
        assert_eq!(
            encode(plan, limits, bindings, bytes.len() as u64).unwrap(),
            bytes
        );
    }
    assert!(misaligned > 0);
}

#[test]
fn launcher_request_declares_v1_canonical_identity_before_plan_admission() {
    let fixture = fixture::Fixture::new(1);
    let original = request(&fixture);
    let decoded = crate::kura::scaling_evidence::tests::assert_declared_scaling_frame::<
        RequestV1,
        AuthenticatedRequest,
    >(
        &original,
        "iroha_kagami::scaling_evidence::LauncherRequestV1",
        [
            116, 39, 208, 10, 84, 9, 3, 43, 128, 167, 143, 203, 8, 240, 222, 205,
        ],
    );
    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.plan.scheduled.len(), original.plan.scheduled.len());
    let bytes = norito::encode_canonical(&decoded).unwrap();
    assert_eq!(bytes, frame(&fixture));
    let (plan, _, bindings) = decode_frame(&bytes).unwrap().into_parts();
    assert_eq!(plan.scheduled.len(), 8);
    assert_eq!(bindings.len(), 2);
}
