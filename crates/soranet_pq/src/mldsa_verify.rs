//! Byte-oriented ML-DSA verification with an `AArch64` SHA3 capability guard.

use pqcrypto_mldsa::{ffi, mldsa44, mldsa65, mldsa87};
use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _, VerificationError};

#[cfg(any(target_arch = "aarch64", test))]
const fn aarch64_backend_available(neon: bool, sha3: bool) -> bool {
    neon && sha3
}

pub(super) fn upstream_verifier_available() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        // pqcrypto 0.1.x selects its NEON implementation unconditionally, but
        // its shared Keccak backend also requires the optional SHA3 ISA.
        // std's detector uses OS capabilities, including the Exynos 9810
        // heterogeneous-core workaround. Unknown capability selects CLEAN.
        aarch64_backend_available(
            std::arch::is_aarch64_feature_detected!("neon"),
            std::arch::is_aarch64_feature_detected!("sha3"),
        )
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        // Preserve the dependency's existing dispatch on other architectures.
        true
    }
}

fn verification_status(status: core::ffi::c_int) -> Result<(), VerificationError> {
    // Match pqcrypto's typed verification API exactly.
    match status {
        0 => Ok(()),
        -1 => Err(VerificationError::InvalidSignature),
        _ => Err(VerificationError::UnknownVerificationError),
    }
}

macro_rules! verifier {
    ($verify:ident, $with_upstream:ident, $suite:ident, $clean:ident) => {
        pub(super) fn $verify(
            signature: &$suite::DetachedSignature,
            message: &[u8],
            context: &[u8],
            public_key: &$suite::PublicKey,
        ) -> Result<(), VerificationError> {
            $with_upstream(signature, message, context, public_key, true)
        }

        fn $with_upstream(
            signature: &$suite::DetachedSignature,
            message: &[u8],
            context: &[u8],
            public_key: &$suite::PublicKey,
            allow_upstream: bool,
        ) -> Result<(), VerificationError> {
            if allow_upstream && upstream_verifier_available() {
                return $suite::verify_detached_signature_ctx(
                    signature, message, context, public_key,
                );
            }
            let signature = signature.as_bytes();
            let public_key = public_key.as_bytes();
            // SAFETY: typed keys have the exact suite length; all other
            // buffers carry their actual lengths and stay alive for the call.
            // The CLEAN verifier checks context and signature lengths before
            // decoding. No private PQClean structures cross this boundary.
            let status = unsafe {
                ffi::$clean(
                    signature.as_ptr(),
                    signature.len(),
                    message.as_ptr(),
                    message.len(),
                    context.as_ptr(),
                    context.len(),
                    public_key.as_ptr(),
                )
            };
            verification_status(status)
        }
    };
}

verifier!(
    verify44_ctx,
    verify44_ctx_with_upstream,
    mldsa44,
    PQCLEAN_MLDSA44_CLEAN_crypto_sign_verify_ctx
);
verifier!(
    verify65_ctx,
    verify65_ctx_with_upstream,
    mldsa65,
    PQCLEAN_MLDSA65_CLEAN_crypto_sign_verify_ctx
);
verifier!(
    verify87_ctx,
    verify87_ctx_with_upstream,
    mldsa87,
    PQCLEAN_MLDSA87_CLEAN_crypto_sign_verify_ctx
);

/// Verify a typed ML-DSA-65 signature with an empty FIPS 204 context.
///
/// Preserves the `pqcrypto` typed verifier's return values and uses its CLEAN
/// implementation when `AArch64` NEON or SHA3 support is unavailable. Callers
/// retain their existing application-level encoding and all-zero checks.
///
/// # Errors
/// Returns [`VerificationError::InvalidSignature`] for a rejected signature,
/// or [`VerificationError::UnknownVerificationError`] for any other backend error.
pub fn verify_mldsa65_detached(
    signature: &mldsa65::DetachedSignature,
    message: &[u8],
    public_key: &mldsa65::PublicKey,
) -> Result<(), VerificationError> {
    verify65_ctx(signature, message, &[], public_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HedgedRngSeed, MlDsaSuite, deterministic_chacha20_rng,
        generate_mldsa_keypair_from_fips_seed, sign_mldsa,
    };

    #[test]
    fn aarch64_selection_requires_both_neon_and_sha3() {
        assert!(!aarch64_backend_available(false, false));
        assert!(!aarch64_backend_available(true, false));
        assert!(!aarch64_backend_available(false, true));
        assert!(aarch64_backend_available(true, true));
    }

    #[test]
    fn runtime_selection_matches_platform_capabilities() {
        #[cfg(target_arch = "aarch64")]
        assert_eq!(
            upstream_verifier_available(),
            aarch64_backend_available(
                std::arch::is_aarch64_feature_detected!("neon"),
                std::arch::is_aarch64_feature_detected!("sha3"),
            )
        );
        #[cfg(not(target_arch = "aarch64"))]
        assert!(upstream_verifier_available());
    }

    #[test]
    fn backend_status_preserves_typed_error_categories() {
        assert!(verification_status(0).is_ok());
        assert!(matches!(
            verification_status(-1),
            Err(VerificationError::InvalidSignature)
        ));
        for status in [1, -2, i32::MIN, i32::MAX] {
            assert!(matches!(
                verification_status(status),
                Err(VerificationError::UnknownVerificationError)
            ));
        }
    }

    macro_rules! parity_test {
        ($name:ident, $suite:ident, $typed:ident, $verify:ident, $with_upstream:ident) => {
            #[test]
            fn $name() {
                let suite = MlDsaSuite::$suite;
                let keypair = generate_mldsa_keypair_from_fips_seed(suite, &[0x51; 32])
                    .expect("fixed ML-DSA keypair");
                let public_key =
                    $typed::PublicKey::from_bytes(keypair.public_key()).expect("typed public key");
                let other_keypair = generate_mldsa_keypair_from_fips_seed(suite, &[0x91; 32])
                    .expect("different fixed ML-DSA keypair");
                let other_public_key = $typed::PublicKey::from_bytes(other_keypair.public_key())
                    .expect("different typed public key");
                assert_ne!(public_key.as_bytes(), other_public_key.as_bytes());
                let message = b"ML-DSA CPU dispatch parity";
                for context in [vec![], vec![0x43], vec![0x44; 255]] {
                    let mut rng = deterministic_chacha20_rng(
                        HedgedRngSeed::from_entropy([0x72; 32]),
                        b"soranet-pq:verification-dispatch-parity",
                    );
                    let signed =
                        sign_mldsa(suite, keypair.secret_key(), &context, message, &mut rng)
                            .expect("portable deterministic signing");
                    let typed = $typed::DetachedSignature::from_bytes(signed.as_bytes())
                        .expect("typed signature");
                    assert!($verify(&typed, message, &context, &public_key).is_ok());
                    for allow_upstream in [false, true] {
                        // false forces CLEAN even on this host's accelerated
                        // CPU. true still checks actual capabilities, so this
                        // suite also runs safely on devices without SHA3.
                        assert!(
                            $with_upstream(&typed, message, &context, &public_key, allow_upstream,)
                                .is_ok()
                        );
                        let altered_context = if context.is_empty() { &[0x43][..] } else { &[] };
                        for (input, ctx) in [
                            (&b"changed message"[..], context.as_slice()),
                            (&message[..], altered_context),
                            (&message[..], &[0x45; 256][..]),
                        ] {
                            assert!(matches!(
                                $with_upstream(&typed, input, ctx, &public_key, allow_upstream),
                                Err(VerificationError::InvalidSignature),
                            ));
                        }
                        let mut corrupted = signed.as_bytes().to_vec();
                        corrupted[0] ^= 1;
                        let mut malformed_hint = signed.as_bytes().to_vec();
                        // The last byte is the final cumulative hint count.
                        // 255 exceeds OMEGA for all three suites (80/55/75).
                        *malformed_hint.last_mut().unwrap() = u8::MAX;
                        for signature in [
                            corrupted,
                            malformed_hint,
                            signed.as_bytes()[..signed.as_bytes().len() - 1].to_vec(),
                            vec![],
                            vec![0; suite.signature_len()],
                        ] {
                            let invalid = $typed::DetachedSignature::from_bytes(&signature)
                                .expect("typed signature retains actual short length");
                            assert!(matches!(
                                $with_upstream(
                                    &invalid,
                                    message,
                                    &context,
                                    &public_key,
                                    allow_upstream
                                ),
                                Err(VerificationError::InvalidSignature),
                            ));
                        }
                        assert!(matches!(
                            $with_upstream(
                                &typed,
                                message,
                                &context,
                                &other_public_key,
                                allow_upstream,
                            ),
                            Err(VerificationError::InvalidSignature),
                        ));
                        let zero_key =
                            $typed::PublicKey::from_bytes(&vec![0; suite.public_key_len()])
                                .expect("typed key is structurally the right length");
                        assert!(matches!(
                            $with_upstream(&typed, message, &context, &zero_key, allow_upstream),
                            Err(VerificationError::InvalidSignature),
                        ));
                    }
                }
            }
        };
    }

    parity_test!(
        clean_and_automatic_44_verification_agree,
        MlDsa44,
        mldsa44,
        verify44_ctx,
        verify44_ctx_with_upstream
    );
    parity_test!(
        clean_and_automatic_65_verification_agree,
        MlDsa65,
        mldsa65,
        verify65_ctx,
        verify65_ctx_with_upstream
    );
    parity_test!(
        clean_and_automatic_87_verification_agree,
        MlDsa87,
        mldsa87,
        verify87_ctx,
        verify87_ctx_with_upstream
    );

    #[test]
    fn typed_65_empty_context_api_preserves_verdicts() {
        let suite = MlDsaSuite::MlDsa65;
        let keypair =
            generate_mldsa_keypair_from_fips_seed(suite, &[0x52; 32]).expect("fixed keypair");
        let public_key = mldsa65::PublicKey::from_bytes(keypair.public_key()).unwrap();
        let mut rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0x73; 32]),
            b"soranet-pq:typed-verifier",
        );
        let signature = sign_mldsa(suite, keypair.secret_key(), &[], b"", &mut rng).unwrap();
        let typed = mldsa65::DetachedSignature::from_bytes(signature.as_bytes()).unwrap();
        assert!(verify_mldsa65_detached(&typed, b"", &public_key).is_ok());
        assert!(matches!(
            verify_mldsa65_detached(&typed, b"changed", &public_key),
            Err(VerificationError::InvalidSignature),
        ));
        let short = mldsa65::DetachedSignature::from_bytes(&[]).unwrap();
        assert!(matches!(
            verify_mldsa65_detached(&short, b"", &public_key),
            Err(VerificationError::InvalidSignature),
        ));
    }
}
