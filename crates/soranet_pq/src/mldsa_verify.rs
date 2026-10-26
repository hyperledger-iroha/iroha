//! Fixed-storage ML-DSA verification using the maintained RustCrypto implementation.
//!
//! Default allocation features remain disabled: decoding and the verification
//! relation use inline arrays on every target, preserving borrowed admission.
use ml_dsa::{EncodedVerifyingKey, Error, MlDsa44, MlDsa65, MlDsa87, Signature, VerifyingKey};

macro_rules! verifier {
    ($verify:ident, $parameter:ident) => {
        pub(super) fn $verify(
            signature: &[u8],
            message: &[u8],
            context: &[u8],
            public_key: &[u8],
        ) -> Result<(), Error> {
            let encoded = EncodedVerifyingKey::<$parameter>::try_from(public_key)
                .map_err(|_| Error::new())?;
            let key = VerifyingKey::<$parameter>::decode(&encoded);
            let signature = Signature::<$parameter>::try_from(signature)?;
            if key.verify_with_context(message, context, &signature) {
                Ok(())
            } else {
                Err(Error::new())
            }
        }
    };
}
verifier!(verify44_ctx, MlDsa44);
verifier!(verify65_ctx, MlDsa65);
verifier!(verify87_ctx, MlDsa87);

/// Verify a borrowed FIPS 204 ML-DSA-65 signature with an empty context.
///
/// # Errors
/// Returns a fixed verification error for malformed encodings or an invalid signature.
pub fn verify_mldsa65_detached(
    signature: &[u8],
    message: &[u8],
    public_key: &[u8],
) -> Result<(), Error> {
    verify65_ctx(signature, message, &[], public_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HedgedRngSeed, MlDsaSuite, deterministic_chacha20_rng,
        generate_mldsa_keypair_from_fips_seed, sign_mldsa,
    };
    macro_rules! parity_test {
        ($name:ident, $suite:ident, $verify:ident) => {
            #[test]
            fn $name() {
                let suite = MlDsaSuite::$suite;
                let keypair = generate_mldsa_keypair_from_fips_seed(suite, &[0x51; 32]).unwrap();
                let other = generate_mldsa_keypair_from_fips_seed(suite, &[0x91; 32]).unwrap();
                assert_ne!(keypair.public_key(), other.public_key());
                let message = b"ML-DSA CPU dispatch parity";
                for context in [vec![], vec![0x43], vec![0x44; 255]] {
                    let mut rng = deterministic_chacha20_rng(
                        HedgedRngSeed::from_entropy([0x72; 32]),
                        b"soranet-pq:verification-dispatch-parity",
                    );
                    let signed =
                        sign_mldsa(suite, keypair.secret_key(), &context, message, &mut rng)
                            .unwrap();
                    assert!(
                        $verify(signed.as_bytes(), message, &context, keypair.public_key()).is_ok()
                    );
                    let altered_context = if context.is_empty() { &[0x43][..] } else { &[] };
                    for (input, ctx) in [
                        (&b"changed message"[..], context.as_slice()),
                        (&message[..], altered_context),
                        (&message[..], &[0x45; 256][..]),
                    ] {
                        assert!(
                            $verify(signed.as_bytes(), input, ctx, keypair.public_key()).is_err()
                        );
                    }
                    let mut corrupted = signed.as_bytes().to_vec();
                    corrupted[0] ^= 1;
                    let mut malformed_hint = signed.as_bytes().to_vec();
                    *malformed_hint.last_mut().unwrap() = u8::MAX;
                    for invalid in [
                        corrupted,
                        malformed_hint,
                        signed.as_bytes()[..signed.as_bytes().len() - 1].to_vec(),
                        vec![],
                        vec![0; suite.signature_len()],
                    ] {
                        assert!(
                            $verify(&invalid, message, &context, keypair.public_key()).is_err()
                        );
                    }
                    assert!(
                        $verify(signed.as_bytes(), message, &context, other.public_key()).is_err()
                    );
                    assert!(
                        $verify(
                            signed.as_bytes(),
                            message,
                            &context,
                            &vec![0; suite.public_key_len()]
                        )
                        .is_err()
                    );
                }
            }
        };
    }
    parity_test!(
        native_signatures_and_rustcrypto_verification_agree_44,
        MlDsa44,
        verify44_ctx
    );
    parity_test!(
        native_signatures_and_rustcrypto_verification_agree_65,
        MlDsa65,
        verify65_ctx
    );
    parity_test!(
        native_signatures_and_rustcrypto_verification_agree_87,
        MlDsa87,
        verify87_ctx
    );
    #[test]
    fn empty_context_api_preserves_verdicts() {
        let suite = MlDsaSuite::MlDsa65;
        let keypair = generate_mldsa_keypair_from_fips_seed(suite, &[0x52; 32]).unwrap();
        let mut rng = deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([0x73; 32]),
            b"soranet-pq:typed-verifier",
        );
        let signed = sign_mldsa(suite, keypair.secret_key(), &[], b"", &mut rng).unwrap();
        assert!(verify_mldsa65_detached(signed.as_bytes(), b"", keypair.public_key()).is_ok());
        assert!(
            verify_mldsa65_detached(signed.as_bytes(), b"changed", keypair.public_key()).is_err()
        );
        assert!(verify_mldsa65_detached(&[], b"", keypair.public_key()).is_err());
    }
}
