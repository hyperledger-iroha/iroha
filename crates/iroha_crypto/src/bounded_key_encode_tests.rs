//! Authored source controls only: this detached candidate is uncompiled/unrun.
use super::*;
use norito::core::{
    BoundedEncodeWorkError, EncodeWorkLimits, EncodeWorkRefusal, Encoder, SerializePayload,
    write_canonical_payload_bounded,
};

fn limits() -> EncodeWorkLimits {
    EncodeWorkLimits {
        max_frame_bytes: 32_768,
        max_byte_visits: 1_000_000,
        max_operations: 100_000,
        max_depth: 64,
    }
}
fn compact(bytes: Vec<u8>) -> PublicKeyCompact {
    PublicKeyCompact {
        algorithm_and_payload: ConstVec::new(bytes),
    }
}
fn encode<P: SerializePayload + ?Sized>(
    value: &P,
    bounds: EncodeWorkLimits,
) -> (Result<(), BoundedEncodeWorkError>, Vec<u8>, bool) {
    let mut output = Vec::new();
    let mut prepared = false;
    let result = write_canonical_payload_bounded::<PublicKeyCompact, _, _, _>(
        value,
        &mut output,
        bounds,
        |_, _| {
            prepared = true;
            Ok(())
        },
    );
    (result, output, prepared)
}
fn assert_source_refusal(key: &PublicKeyCompact) {
    let (result, output, prepared) = encode(key, limits());
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(EncodeWorkRefusal::Source))
    ));
    assert!(output.is_empty());
    assert!(!prepared);
}
fn structural_shapes() -> Vec<(Algorithm, Vec<u8>)> {
    // These are private structural fixtures, not cryptographically validated
    // public constructors or live key evidence.
    let mut shapes = vec![
        (Algorithm::Ed25519, vec![7; 32]),
        (Algorithm::Secp256k1, [vec![2], vec![7; 32]].concat()),
        (Algorithm::MlDsa, vec![7; ML_DSA_65_PUBLIC_KEY_BYTES]),
    ];
    shapes.extend([
        (Algorithm::BlsNormal, vec![7; 48]),
        (Algorithm::BlsSmall, vec![7; 96]),
    ]);
    shapes.extend([
        (Algorithm::Gost3410_2012_256ParamSetA, vec![7; 64]),
        (Algorithm::Gost3410_2012_256ParamSetB, vec![7; 64]),
        (Algorithm::Gost3410_2012_256ParamSetC, vec![7; 64]),
        (Algorithm::Gost3410_2012_512ParamSetA, vec![7; 128]),
        (Algorithm::Gost3410_2012_512ParamSetB, vec![7; 128]),
    ]);
    shapes.push((Algorithm::Sm2, [vec![0, 1, b'a', 4], vec![7; 64]].concat()));
    shapes
}
#[test]
fn bounded_compact_key_bytes_match_every_canonical_structural_shape() {
    for (algorithm, payload) in structural_shapes() {
        let key = PublicKeyCompact::new(algorithm, &payload);
        let expected = norito::encode_canonical(&key).unwrap();
        let (result, output, prepared) = encode(&key, limits());
        result.unwrap();
        assert!(prepared);
        assert_eq!(output, expected);
        assert_eq!(
            key.algorithm_and_payload.as_ref()[0],
            PublicKeyCompact::algorithm_tag(algorithm)
        );
    }
}
#[test]
fn bounded_key_work_refusal_precedes_malformed_tag_validation() {
    let key = compact(vec![255]);
    let (result, output, prepared) = encode(
        &key,
        EncodeWorkLimits {
            max_byte_visits: <PublicKeyCompact as norito::NoritoSchema>::static_frame_name()
                .unwrap()
                .len()
                + 128,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert!(output.is_empty());
    assert!(!prepared);
}
#[test]
fn malformed_key_envelopes_refuse_before_frame_owner_preparation() {
    for bytes in [
        vec![],
        vec![255],
        vec![0],
        vec![0; 33],
        [vec![0], vec![7; 31]].concat(),
        [vec![1, 4], vec![7; 32]].concat(),
        [vec![4], vec![7; ML_DSA_65_PUBLIC_KEY_BYTES - 1]].concat(),
    ] {
        assert_source_refusal(&compact(bytes));
    }
}
struct IgnoredSizing<'a> {
    key: &'a PublicKeyCompact,
    exact: bool,
    repeats: usize,
}
impl SerializePayload for IgnoredSizing<'_> {
    fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), norito::core::Error> {
        for _ in 0..self.repeats {
            let _ignored = if self.exact {
                self.key.encoded_len_exact()
            } else {
                self.key.encoded_len_hint()
            };
        }
        Ok(())
    }
}
#[test]
fn erased_hint_error_remains_sticky_source_refusal() {
    let key = compact(vec![0]);
    let (result, output, prepared) = encode(
        &IgnoredSizing {
            key: &key,
            exact: false,
            repeats: 2,
        },
        limits(),
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(EncodeWorkRefusal::Source))
    ));
    assert!(output.is_empty());
    assert!(!prepared);
}
#[test]
fn erased_exact_length_error_remains_sticky_source_refusal() {
    let key = compact(vec![255]);
    let (result, output, prepared) = encode(
        &IgnoredSizing {
            key: &key,
            exact: true,
            repeats: 2,
        },
        limits(),
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(EncodeWorkRefusal::Source))
    ));
    assert!(output.is_empty());
    assert!(!prepared);
}
#[test]
fn repeated_sizing_uses_one_finite_prewrite_allowance() {
    let key = PublicKeyCompact::new(Algorithm::Ed25519, &[7; 32]);
    for exact in [false, true] {
        let (result, output, prepared) = encode(
            &IgnoredSizing {
                key: &key,
                exact,
                repeats: 100,
            },
            EncodeWorkLimits {
                max_byte_visits: 512,
                ..limits()
            },
        );
        assert!(matches!(
            result,
            Err(BoundedEncodeWorkError::Refused(
                EncodeWorkRefusal::ByteVisits
            ))
        ));
        assert!(output.is_empty());
        assert!(!prepared);
    }
}
#[test]
fn unknown_tags_and_malformed_known_envelopes_are_fixed_source_refusals() {
    for tag in [11, 255] {
        assert_source_refusal(&compact(vec![tag, 1]));
    }
    // Canonical tags remain recognized independently of backend features;
    // a one-byte payload is structurally malformed for all of them.
    for tag in 0..=10 {
        assert_source_refusal(&compact(vec![tag, 1]));
    }
}
#[test]
fn legacy_public_key_errors_and_sizing_remain_outside_bounded_scope() {
    let malformed = compact(vec![0]);
    let mut bytes = Vec::new();
    let error = malformed
        .serialize(&mut Encoder::new(&mut bytes))
        .unwrap_err();
    assert!(
        matches!(error, norito::core::Error::Message(message) if message == "invalid public key")
    );
    assert!(bytes.is_empty());
    assert_eq!(malformed.encoded_len_hint(), None);
    assert_eq!(malformed.encoded_len_exact(), None);
    assert!(matches!(
        validate_public_key_structural_envelope(Algorithm::Ed25519, &[]).unwrap_err(),
        ParseError::Invalid(message) if message == "invalid public key structural envelope"
    ));
    assert!(matches!(
        validate_public_key_structural_envelope(Algorithm::Secp256k1, &[4; 33]).unwrap_err(),
        ParseError::Invalid(message) if message == "invalid secp256k1 public key envelope"
    ));
}
#[test]
fn bounded_sm2_invalid_utf8_and_bit_length_overflow_refuse() {
    for payload in [
        [vec![0, 1, 255, 4], vec![7; 64]].concat(),
        [vec![0x20, 0], vec![b'a'; 8192], vec![4], vec![7; 64]].concat(),
    ] {
        assert_source_refusal(&PublicKeyCompact::new(Algorithm::Sm2, &payload));
    }
}
#[cfg(feature = "pqc")]
#[test]
fn bounded_key_serialization_does_not_invoke_cryptographic_validation() {
    let key = PublicKeyCompact::new(Algorithm::MlDsa, &[7; ML_DSA_65_PUBLIC_KEY_BYTES]);
    reset_public_key_validation_call_count();
    encode(&key, limits()).0.unwrap();
    assert_eq!(public_key_validation_call_count(), 0);
}

#[test]
fn sizing_loop_has_a_separate_precharge_after_structural_validation() {
    let key = PublicKeyCompact::new(Algorithm::Ed25519, &[7; 32]);
    let schema_cost = <PublicKeyCompact as norito::NoritoSchema>::static_frame_name()
        .unwrap()
        .len()
        + 128;
    for exact in [false, true] {
        let (result, output, prepared) = encode(
            &IgnoredSizing {
                key: &key,
                exact,
                repeats: 1,
            },
            EncodeWorkLimits {
                max_byte_visits: schema_cost + 33,
                ..limits()
            },
        );
        assert!(matches!(
            result,
            Err(BoundedEncodeWorkError::Refused(
                EncodeWorkRefusal::ByteVisits
            ))
        ));
        assert!(output.is_empty());
        assert!(!prepared);
    }
}

#[test]
fn secp256k1_prefix_revisit_is_precharged() {
    // The nonzero final byte makes the all-zero check visit the whole payload;
    // the invalid first byte then has to be inspected a second time.
    let key = compact([vec![1], vec![0; 32], vec![1]].concat());
    let schema_cost = <PublicKeyCompact as norito::NoritoSchema>::static_frame_name()
        .unwrap()
        .len()
        + 128;
    let (result, output, prepared) = encode(
        &key,
        EncodeWorkLimits {
            max_byte_visits: schema_cost + key.algorithm_and_payload.len(),
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert!(output.is_empty());
    assert!(!prepared);
    // With enough prepaid work the original predicate still rejects its shape.
    assert_source_refusal(&key);
}
