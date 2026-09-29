//! Canonical envelope, scalar, and facade controls for borrowed GOST verification.

use super::*;
use crate::signature::gost;
use crate::{KeyPair, PublicKey, PublicKeyCompact, PublicKeyMaterial, Signature};
use num_bigint::BigUint;
use num_traits::{One, Zero};

const ALGORITHMS: [Algorithm; 5] = [
    Algorithm::Gost3410_2012_256ParamSetA,
    Algorithm::Gost3410_2012_256ParamSetB,
    Algorithm::Gost3410_2012_256ParamSetC,
    Algorithm::Gost3410_2012_512ParamSetA,
    Algorithm::Gost3410_2012_512ParamSetB,
];

#[test]
fn borrowed_key_boundaries_preserve_diagnostics_and_key_before_signature_order() {
    for algorithm in ALGORITHMS {
        let params = gost::params_for_algorithm(algorithm).unwrap().curve();
        let expected = params.scalar_len * 2;
        for actual in [0, 1, expected - 1, expected + 1, 65_536] {
            let bytes = vec![0; actual];
            let rejection = KeyRejection::Length {
                name: params.name,
                expected,
                actual,
            };
            assert_eq!(validate_public_key(algorithm, &bytes), Err(rejection));
            let exact = format!(
                "public key for {} must be {expected} bytes, got {actual}",
                params.name
            );
            assert_eq!(rejection.into_parse_error().to_string(), exact);
            let malformed = PublicKey(PublicKeyCompact::new(algorithm, &bytes));
            let signature = Signature::from_bytes(&[]);
            for result in [
                signature.verify(&malformed, b""),
                crate::verify_signature_for_admission(&signature, &malformed, b""),
            ] {
                let error = result.unwrap_err();
                assert!(matches!(error, Error::Parse(ref error) if error.to_string() == exact));
            }
        }
        let zero = vec![0; expected];
        let error = KeyRejection::AllZero(params.name);
        assert_eq!(validate_public_key(algorithm, &zero), Err(error));
        assert_eq!(
            error.into_parse_error().to_string(),
            format!("public key for {} must not be all zero", params.name)
        );
        let mut generator = gost::point_to_le_bytes(&params.generator(), params.scalar_len);
        validate_public_key(algorithm, &generator).unwrap();
        for coordinate in [0, params.scalar_len] {
            for value in [
                &params.p,
                &((BigUint::one() << (params.scalar_len * 8)) - BigUint::one()),
            ] {
                let invalid = gost::scalar_to_le_bytes(value, params.scalar_len);
                let original = generator[coordinate..coordinate + params.scalar_len].to_vec();
                generator[coordinate..coordinate + params.scalar_len].copy_from_slice(&invalid);
                let error = KeyRejection::NotOnCurve(params.name);
                assert_eq!(validate_public_key(algorithm, &generator), Err(error));
                assert_eq!(
                    error.into_parse_error().to_string(),
                    format!("public key is not on the curve for {}", params.name)
                );
                generator[coordinate..coordinate + params.scalar_len].copy_from_slice(&original);
            }
        }
    }
    assert_eq!(
        validate_public_key(Algorithm::Ed25519, &[]),
        Err(KeyRejection::Unsupported(Algorithm::Ed25519))
    );
    assert_eq!(
        KeyRejection::Unsupported(Algorithm::Ed25519)
            .into_parse_error()
            .to_string(),
        "algorithm Ed25519 is not a supported GOST parameter set"
    );
}

fn compare_scalar<const LIMBS: usize>(curve: &CurveParameters<LIMBS>, params: &gost::CurveParams) {
    for message in [
        &b""[..],
        &b"borrowed scalar relation"[..],
        &[0xa5; 63],
        &[0x5a; 64],
        &[0xc3; 65],
    ] {
        let actual = super::super::uint_to_biguint(&message_scalar(curve, message).as_uint());
        assert_eq!(
            actual,
            gost::hash_to_scalar(params, message),
            "{} digest byte order",
            params.name
        );
    }
    for value in [
        BigUint::zero(),
        BigUint::one(),
        &params.q - BigUint::one(),
        params.q.clone(),
    ] {
        let fixed = super::super::biguint_to_uint(&value).unwrap();
        let actual = reduce_digest(curve, &fixed).as_uint();
        let mut expected = &value % &params.q;
        if expected.is_zero() {
            expected = BigUint::one();
        }
        assert_eq!(super::super::uint_to_biguint(&actual), expected);
        let inverse = FieldElement::from_uint(actual, curve.scalar_params)
            .invert()
            .unwrap()
            .as_uint();
        assert_eq!(
            super::super::uint_to_biguint(&inverse),
            gost::mod_inv(&expected, &params.q).unwrap()
        );
    }
}

#[test]
fn digest_order_zero_reduction_and_inverse_match_existing_independent_arithmetic() {
    for algorithm in ALGORITHMS {
        let params = gost::params_for_algorithm(algorithm).unwrap().curve();
        match curve_for_algorithm(algorithm).unwrap() {
            CurveSelection::Bits256(curve) => compare_scalar(curve, params),
            CurveSelection::Bits512(curve) => compare_scalar(curve, params),
        }
    }
}

#[test]
fn signature_ranges_messages_and_every_signature_byte_are_bound_for_all_sets() {
    for algorithm in ALGORITHMS {
        let params = gost::params_for_algorithm(algorithm).unwrap().curve();
        let (public, private) =
            gost::generate_seeded_keypair(algorithm, b"fixed GOST verification controls").unwrap();
        let message = b"all five canonical GOST relations";
        let mut nonce = gost::StreebogNonceGenerator::new();
        let signature = gost::sign_impl(params, message, &private, &mut nonce, None).unwrap();
        verify_bytes(algorithm, message, &signature, public.as_bytes()).unwrap();
        gost::verify(algorithm, message, &signature, &public).unwrap();
        for length in [0, 1, signature.len() - 1] {
            assert!(matches!(
                verify_bytes(algorithm, message, &signature[..length], public.as_bytes()),
                Err(Error::BadSignature)
            ));
        }
        let mut oversized = signature.clone();
        oversized.push(0);
        assert!(matches!(
            verify_bytes(algorithm, message, &oversized, public.as_bytes()),
            Err(Error::BadSignature)
        ));
        for offset in [0, params.scalar_len] {
            for scalar in [
                BigUint::zero(),
                params.q.clone(),
                (BigUint::one() << (params.scalar_len * 8)) - BigUint::one(),
            ] {
                let mut invalid = signature.clone();
                invalid[offset..offset + params.scalar_len]
                    .copy_from_slice(&gost::scalar_to_le_bytes(&scalar, params.scalar_len));
                assert!(matches!(
                    verify_bytes(algorithm, message, &invalid, public.as_bytes()),
                    Err(Error::BadSignature)
                ));
            }
        }
        for index in 0..signature.len() {
            let mut invalid = signature.clone();
            invalid[index] ^= 1;
            assert!(
                verify_bytes(algorithm, message, &invalid, public.as_bytes()).is_err(),
                "{algorithm:?} signature byte {index}"
            );
        }
        for index in 0..message.len() {
            let mut invalid = message.to_vec();
            invalid[index] ^= 1;
            assert!(
                verify_bytes(algorithm, &invalid, &signature, public.as_bytes()).is_err(),
                "{algorithm:?} message byte {index}"
            );
        }
        for index in 0..public.as_bytes().len() {
            let mut invalid = public.as_bytes().to_vec();
            invalid[index] ^= 1;
            assert!(
                verify_bytes(algorithm, message, &signature, &invalid).is_err(),
                "{algorithm:?} public byte {index}"
            );
        }
        for other in ALGORITHMS {
            if other != algorithm {
                assert!(
                    verify_bytes(other, message, &signature, public.as_bytes()).is_err(),
                    "cross-parameter {algorithm:?}/{other:?}"
                );
            }
        }
    }
}

#[test]
fn compact_gost_material_borrows_original_bytes_and_never_enters_decoded_cache() {
    for algorithm in ALGORITHMS {
        let pair = KeyPair::from_seed(vec![0x4a; 32], algorithm);
        let key = pair.public_key();
        let (_, bytes) = key.try_to_bytes().unwrap();
        let material = crate::parse_public_key_material(algorithm, bytes).unwrap();
        let PublicKeyMaterial::Gost {
            algorithm: parsed,
            bytes: borrowed,
        } = material
        else {
            panic!("GOST must remain borrowed");
        };
        assert_eq!(parsed, algorithm);
        assert!(core::ptr::eq(bytes.as_ptr(), borrowed.as_ptr()));
        crate::signature::PUBLIC_KEY_FULL_CACHE.with(|cache| cache.borrow_mut().clear());
        for _ in 0..3 {
            let material = crate::signature::public_key_material_cached(key).unwrap();
            let PublicKeyMaterial::Gost {
                algorithm: parsed,
                bytes: borrowed,
            } = material
            else {
                panic!("GOST must remain borrowed");
            };
            assert_eq!(parsed, algorithm);
            assert!(core::ptr::eq(bytes.as_ptr(), borrowed.as_ptr()));
            assert!(matches!(
                Signature::from_bytes(&[]).verify(key, b""),
                Err(Error::BadSignature)
            ));
            assert_eq!(
                crate::signature::PUBLIC_KEY_FULL_CACHE.with(|cache| cache.borrow().len()),
                0
            );
        }
        let compact = PublicKeyMaterial::Gost { algorithm, bytes }.into_public_key();
        assert_eq!(&compact, key);
    }
}

#[test]
fn malformed_gost_key_precedes_keypair_algorithm_mismatch() {
    let pair = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
    for algorithm in ALGORITHMS {
        let malformed = PublicKey(PublicKeyCompact::new(algorithm, &[]));
        let error = KeyPair::new(malformed, pair.private_key().clone()).unwrap_err();
        assert!(matches!(error, Error::Parse(_)));
        let other = KeyPair::from_seed(vec![0x41; 32], algorithm);
        let error =
            KeyPair::new(other.public_key().clone(), pair.private_key().clone()).unwrap_err();
        assert!(matches!(error, Error::KeyGen(ref text) if text == "Mismatch of key algorithms"));
    }
}

fn check_infinite_sum<const LIMBS: usize>(
    curve: &CurveParameters<LIMBS>,
    algorithm: Algorithm,
    params: &gost::CurveParams,
) {
    use crate::{Hash, HashOf, SignatureOf, verify_signature_for_admission};
    let hash = HashOf::<()>::from_untyped_unchecked(Hash::prehashed([0x61; 32]));
    let public = gost::point_to_le_bytes(&params.generator(), params.scalar_len);
    let point = parse_point(curve, &public).unwrap();
    // Q=G and r=s=1 are individually canonical nonzero scalars. The verifier's
    // actual joint relation must still reject (1/e)G + (-1/e)Q = infinity.
    let inverse = message_scalar(curve, hash.as_ref()).invert().unwrap();
    let z1 = inverse.as_uint();
    let z2 = inverse.negate().as_uint();
    assert_ne!(z1, Uint::ZERO);
    assert_ne!(z2, Uint::ZERO);
    assert!(z1 < curve.scalar_modulus);
    assert!(z2 < curve.scalar_modulus);
    assert!(mul_add_impl(curve, &z1, &z2, &point).is_none());
    let mut signature = vec![0; params.scalar_len * 2];
    signature[0] = 1;
    signature[params.scalar_len] = 1;
    let key = PublicKey::from_bytes(algorithm, &public).unwrap();
    let owned = gost::parse_public_key(algorithm, &public).unwrap();
    let proof = Signature::from_bytes(&signature);
    let typed = SignatureOf::<()>::from_signature(proof.clone());
    for result in [
        verify_bytes(algorithm, hash.as_ref(), &signature, &public),
        gost::verify(algorithm, hash.as_ref(), &signature, &owned),
        proof.verify(&key, hash.as_ref()),
        typed.verify_hash(&key, hash),
        verify_signature_for_admission(&proof, &key, hash.as_ref()),
    ] {
        assert!(matches!(result, Err(Error::BadSignature)));
    }
}

#[test]
fn infinite_joint_relation_is_rejected_by_all_public_paths_for_every_parameter_set() {
    for algorithm in ALGORITHMS {
        let params = gost::params_for_algorithm(algorithm).unwrap().curve();
        match curve_for_algorithm(algorithm).unwrap() {
            CurveSelection::Bits256(curve) => check_infinite_sum(curve, algorithm, params),
            CurveSelection::Bits512(curve) => check_infinite_sum(curve, algorithm, params),
        }
    }
}

#[test]
fn fixed_native_widths_cover_full_field_and_scalar_boundaries_without_narrowing() {
    fn check<const LIMBS: usize>(curve: &CurveParameters<LIMBS>, params: &gost::CurveParams) {
        assert_eq!(Uint::<LIMBS>::BYTES, params.scalar_len);
        assert_eq!(core::mem::size_of::<Uint<LIMBS>>(), params.scalar_len);
        let modulus = curve.field_params.modulus().as_ref();
        for value in [
            BigUint::zero(),
            BigUint::one(),
            &params.p - BigUint::one(),
            params.p.clone(),
            &params.q - BigUint::one(),
            params.q.clone(),
            (BigUint::one() << (params.scalar_len * 8)) - BigUint::one(),
        ] {
            let encoded = gost::scalar_to_le_bytes(&value, params.scalar_len);
            let fixed = Uint::<LIMBS>::from_le_slice(&encoded);
            assert_eq!(super::super::uint_to_biguint(&fixed), value);
            assert_eq!(fixed < *modulus, value < params.p);
            assert_eq!(fixed < curve.scalar_modulus, value < params.q);
            assert_eq!(
                super::super::uint_to_biguint(
                    &FieldElement::from_uint(fixed, curve.field_params).as_uint()
                ),
                &value % &params.p
            );
            assert_eq!(
                super::super::uint_to_biguint(
                    &FieldElement::from_uint(fixed, curve.scalar_params).as_uint()
                ),
                &value % &params.q
            );
        }
    }
    for algorithm in ALGORITHMS {
        let params = gost::params_for_algorithm(algorithm).unwrap().curve();
        match curve_for_algorithm(algorithm).unwrap() {
            CurveSelection::Bits256(curve) => check(curve, params),
            CurveSelection::Bits512(curve) => check(curve, params),
        }
    }
}
