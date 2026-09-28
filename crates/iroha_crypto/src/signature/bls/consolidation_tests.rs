//! Specified BLS diagnostics, facade ownership and checked typed-key controls.

use super::uncached::tests::without_allocations;
use super::uncached::{Orientation, Rejection};
use super::{canonical, implementation, normal::NormalConfiguration, small::SmallConfiguration};
use crate::{Error, KeyGenOption};
use implementation::{BlsConfiguration, BlsImpl, VerifyOkCacheAccess};
use w3f_bls::{EngineBLS, PublicKey, SerializableToBytes};

#[derive(Clone, Copy, Debug)]
enum Expected {
    Valid,
    BadSignature,
    Parse(&'static str),
}
const INVALID_KEY: Expected = Expected::Parse("the input buffer contained invalid data");
const KEY_FLAGS: Expected = Expected::Parse("the call expects empty flags");
const KEY_NONCANONICAL: Expected = Expected::Parse("non-canonical BLS public key encoding");
const INVALID_SIGNATURE: Expected = Expected::Parse("Failed to parse signature.");
const SIGNATURE_NONCANONICAL: Expected = Expected::Parse("non-canonical BLS signature encoding");

fn assert_expected(actual: Result<(), Error>, expected: Expected) {
    match (actual, expected) {
        (Ok(()), Expected::Valid) | (Err(Error::BadSignature), Expected::BadSignature) => {}
        (Err(Error::Parse(error)), Expected::Parse(message)) => {
            assert_eq!(error.to_string(), message)
        }
        other => panic!("specified BLS diagnostic differs: {other:?}"),
    }
}

fn fixture<C: BlsConfiguration>(seed: u8, message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let (key, private) = BlsImpl::<C>::try_keypair(KeyGenOption::UseSeed(vec![seed; 32])).unwrap();
    (
        key.to_bytes(),
        BlsImpl::<C>::try_sign(message, &private).unwrap(),
    )
}

struct PointCase {
    bytes: Vec<u8>,
    key: Expected,
    signature: Expected,
}
fn specified_point_cases(valid: &[u8]) -> Vec<PointCase> {
    let mut cases = Vec::new();
    let mut add = |bytes: Vec<u8>, key, signature| {
        cases.push(PointCase {
            bytes,
            key,
            signature,
        })
    };
    add(valid.to_vec(), Expected::Valid, Expected::Valid);
    for length in [0, 1, valid.len() / 2, valid.len() - 1] {
        add(valid[..length].to_vec(), INVALID_KEY, INVALID_SIGNATURE);
    }
    for length in [1, valid.len(), valid.len() + 1] {
        add(
            vec![0; length],
            Expected::Parse("BLS public key material must not be all zero"),
            Expected::Parse("BLS signature material must not be all zero"),
        );
    }
    let flag_key = [
        KEY_FLAGS,
        KEY_FLAGS,
        KEY_FLAGS,
        KEY_FLAGS,
        Expected::Valid,
        Expected::Valid,
        KEY_NONCANONICAL,
        KEY_NONCANONICAL,
    ];
    let flag_signature = [
        INVALID_SIGNATURE,
        INVALID_SIGNATURE,
        INVALID_SIGNATURE,
        INVALID_SIGNATURE,
        Expected::Valid,
        Expected::Valid,
        SIGNATURE_NONCANONICAL,
        SIGNATURE_NONCANONICAL,
    ];
    assert!(
        valid[1..].iter().any(|byte| *byte != 0),
        "flag cases use a nonidentity coordinate"
    );
    for flags in 0..8 {
        let mut bytes = valid.to_vec();
        bytes[0] = (bytes[0] & 0x1f) | ((u8::try_from(flags).expect("three-bit flag")) << 5);
        add(bytes, flag_key[flags], flag_signature[flags]);
    }
    let mut identity = vec![0; valid.len()];
    identity[0] = 0xc0;
    add(
        identity.clone(),
        Expected::Parse("BLS public key is identity"),
        Expected::Parse("BLS signature is identity"),
    );
    identity[0] = 0xe0;
    add(identity, KEY_NONCANONICAL, SIGNATURE_NONCANONICAL);
    let modulus = hex_literal::hex!(
        "1a0111ea397fe69a4b1ba7b6434bacd764774b84f38512bf6730d2a0f6b0f6241eabfffeb153ffffb9feffffffffaaab"
    );
    for coordinate in 0..valid.len() / 48 {
        let mut dirty = vec![0; valid.len()];
        dirty[0] = 0xc0;
        dirty[(coordinate + 1) * 48 - 1] = 1;
        add(dirty, KEY_NONCANONICAL, SIGNATURE_NONCANONICAL);
        for last in [0xab, 0xac] {
            let mut bytes = valid.to_vec();
            bytes[coordinate * 48..(coordinate + 1) * 48].copy_from_slice(&modulus);
            bytes[(coordinate + 1) * 48 - 1] = last;
            bytes[0] |= 0x80;
            add(bytes.clone(), INVALID_KEY, INVALID_SIGNATURE);
            bytes[0] &= 0x7f;
            add(bytes, KEY_FLAGS, INVALID_SIGNATURE);
        }
        let mut invalid = valid.to_vec();
        invalid[coordinate * 48..(coordinate + 1) * 48].fill(0xff);
        invalid[0] = (invalid[0] & 0x1f) | 0x80;
        add(invalid.clone(), INVALID_KEY, INVALID_SIGNATURE);
        invalid.push(0x7e);
        add(invalid, INVALID_KEY, INVALID_SIGNATURE);
    }
    for suffix in [0, 0x7e] {
        let mut bytes = valid.to_vec();
        bytes.push(suffix);
        add(bytes, KEY_NONCANONICAL, SIGNATURE_NONCANONICAL);
    }
    cases
}

fn parser_controls<C: BlsConfiguration>(seed: u8) {
    let (key, proof) = fixture::<C>(seed, b"specified point diagnostics");
    for case in specified_point_cases(&key) {
        let actual =
            without_allocations(|| canonical::public_key::<C::Engine>(&case.bytes).map(drop));
        assert_expected(
            actual.map_err(|error| error.into_parse_error().into()),
            case.key,
        );
    }
    for case in specified_point_cases(&proof) {
        let actual =
            without_allocations(|| canonical::signature::<C::Engine>(&case.bytes).map(drop));
        assert_expected(
            actual.map_err(|error| error.into_parse_error().into()),
            case.signature,
        );
    }
}
#[test]
fn shared_parser_preserves_specified_boundary_diagnostics_without_allocating() {
    parser_controls::<NormalConfiguration>(0xb1);
    parser_controls::<SmallConfiguration>(0xb2);
}

fn facade_case<C: BlsConfiguration + VerifyOkCacheAccess>(
    orientation: Orientation,
    key: &[u8],
    proof: &[u8],
    message: &[u8],
    expected: Expected,
) {
    let actual =
        without_allocations(|| super::uncached::verify_facade(orientation, key, proof, message));
    assert_expected(actual.map_err(Rejection::into_error), expected);
    let key = crate::PublicKey(crate::PublicKeyCompact::new(C::ALGORITHM, key));
    let proof = crate::Signature::from_bytes(proof);
    assert_expected(proof.verify(&key, message), expected);
    assert_expected(
        crate::verify_signature_for_admission(&proof, &key, message),
        expected,
    );
}
fn facade_controls<C: BlsConfiguration + VerifyOkCacheAccess>(orientation: Orientation, seed: u8) {
    let message = [0xc1; 32];
    let (key, proof) = fixture::<C>(seed, &message);
    facade_case::<C>(orientation, &key, &proof, &message, Expected::Valid);
    facade_case::<C>(
        orientation,
        &key,
        &proof,
        b"wrong message",
        Expected::BadSignature,
    );
    for case in specified_point_cases(&proof) {
        let expected = if case.bytes.len() != C::Engine::SIGNATURE_SERIALIZED_SIZE
            || (!case.bytes.is_empty() && case.bytes.iter().all(|byte| *byte == 0))
        {
            Expected::BadSignature
        } else if matches!(case.signature, Expected::Valid) && case.bytes != proof {
            // The other sign flag is the negation of this signature, not a
            // second valid signature over the same key and message.
            Expected::BadSignature
        } else {
            case.signature
        };
        facade_case::<C>(orientation, &key, &case.bytes, &message, expected);
    }
    // Explicit malformed keys beat both generic signature geometry failures
    // and signature parse failures, including after positive-cache warming.
    for case in specified_point_cases(&key) {
        if matches!(case.key, Expected::Valid) {
            continue;
        }
        for invalid in [Vec::new(), vec![0; proof.len()], vec![0xff; proof.len()]] {
            facade_case::<C>(orientation, &case.bytes, &invalid, &message, case.key);
        }
    }
}
#[test]
fn ordinary_and_admission_facades_preserve_specified_errors_and_precedence() {
    facade_controls::<NormalConfiguration>(Orientation::Normal, 0xc2);
    facade_controls::<SmallConfiguration>(Orientation::Small, 0xc3);
}

fn cache_bypass<C: BlsConfiguration + VerifyOkCacheAccess>(seed: u8) {
    let message = [0xd1; 32];
    let (key, proof) = fixture::<C>(seed, &message);
    let public_key = crate::PublicKey::from_bytes(C::ALGORITHM, &key).unwrap();
    let proof = crate::Signature::from_bytes(&proof);
    let full_before = crate::signature::bls_full_key_cache_accesses_for_tests();
    let positive_before = implementation::verify_ok_cache_accesses_for_tests();
    without_allocations(|| crate::verify_signature_for_admission(&proof, &public_key, &message))
        .unwrap();
    assert_eq!(
        implementation::verify_ok_cache_accesses_for_tests(),
        positive_before
    );
    assert_eq!(
        crate::signature::bls_full_key_cache_accesses_for_tests(),
        full_before
    );
    proof.verify(&public_key, &message).unwrap();
    let ordinary = implementation::verify_ok_cache_accesses_for_tests();
    assert!(
        ordinary > positive_before,
        "ordinary facade retains its explicit positive-cache policy"
    );
    assert_eq!(
        crate::signature::bls_full_key_cache_accesses_for_tests(),
        full_before
    );
    without_allocations(|| crate::verify_signature_for_admission(&proof, &public_key, &message))
        .unwrap();
    assert_eq!(
        implementation::verify_ok_cache_accesses_for_tests(),
        ordinary
    );
    assert_eq!(
        crate::signature::bls_full_key_cache_accesses_for_tests(),
        full_before
    );
}

#[test]
fn admission_bypasses_all_retained_caches_before_and_after_ordinary_verification() {
    cache_bypass::<NormalConfiguration>(0xd2);
    cache_bypass::<SmallConfiguration>(0xd3);
}

fn unchecked_point<T: SerializableToBytes>(bytes: &[u8]) -> T {
    T::deserialize_compressed_unchecked(bytes).unwrap()
}
fn typed_invalid<C: BlsConfiguration + VerifyOkCacheAccess>(non_subgroup: &[u8], seed: u8) {
    let message = [0xe1; 32];
    let (_, proof) = fixture::<C>(seed, &message);
    let key: PublicKey<C::Engine> = unchecked_point(non_subgroup);
    assert_eq!(
        key.to_bytes(),
        non_subgroup,
        "typed point is the exact canonical subgroup tripwire"
    );
    assert!(BlsImpl::<C>::parse_public_key(non_subgroup).is_err());
    assert!(BlsImpl::<C>::verify(&message, &proof, &key).is_err());
    let identity = PublicKey::<C::Engine>(Default::default());
    assert_expected(
        BlsImpl::<C>::verify(&message, &vec![0; proof.len()], &identity),
        Expected::Parse("BLS signature material must not be all zero"),
    );
    for bytes in [Vec::new(), vec![0xff; proof.len()], proof] {
        assert_expected(
            BlsImpl::<C>::verify(&message, &bytes, &identity),
            Expected::Parse("BLS public key is identity"),
        );
    }
}
#[test]
fn typed_keys_require_subgroups_and_keep_signature_zero_before_identity() {
    let g1 = hex_literal::hex!(
        "800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004"
    );
    let g2 = hex_literal::hex!(
        "8158b0083c00046272a9b63583963fff07e147f3f9e6e24174328ad8bc2aa150298f3189a9cf6ed626f461e944bbd3d117762a3b9108c4a74a151b732a6075bf2199bc19c48c393d4ceb92d0a76057be02f08540770fabd60262cea73ea1906c"
    );
    typed_invalid::<NormalConfiguration>(&g1, 0xe2);
    typed_invalid::<SmallConfiguration>(&g2, 0xe3);
}
