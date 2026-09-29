//! Canonical secret commitments with clearing owned hash state.

use super::{RamLfeBackend, RamLfeError, validate_secret};
use norito::codec::Encode;
use zeroize::Zeroizing;

const CONTEXT: &str = "iroha.ram_lfe.policy_secret.v1";
pub(super) const PROGRAM_CONTEXT: &str = "iroha.ram_lfe.bfv_program.secret_tape.v1";

// Both fields are length-framed by the canonical Norito encoder. Borrowing the
// secret avoids constructing a second, uncleared preimage allocation.
#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_crypto::ram_lfe::PolicySecretInputV1",
    frame = "iroha_crypto::ram_lfe::PolicySecretInputV1"
)]
struct PolicySecretInputV1<'a> {
    backend: RamLfeBackend,
    secret: &'a [u8],
}

/// Derive the public 32-byte commitment used by the outer policy transcript.
///
/// The result is a raw BLAKE3 commitment, not an Iroha Blake2b `Hash`. Both
/// secret-bearing library owners are cleared on success, errors and unwinding.
/// This does not claim erasure of compiler-created copies inside the primitive.
pub(super) fn commit(backend: RamLfeBackend, secret: &[u8]) -> Result<[u8; 32], RamLfeError> {
    validate_secret(secret)?;
    let input = PolicySecretInputV1 { backend, secret };
    commit_canonical(CONTEXT, &input)
        .map_err(|error| RamLfeError::TranscriptEncoding(error.to_string()))
}

/// Stream one canonical private transcript into a clearing commitment owner.
pub(super) fn commit_canonical<T: norito::NoritoSerialize>(
    context: &'static str,
    input: &T,
) -> Result<[u8; 32], norito::core::Error> {
    let mut hasher = Zeroizing::new(blake3::Hasher::new_derive_key(context));
    norito::core::write_canonical_to_writer(input, &mut *hasher)?;
    let mut reader = Zeroizing::new(hasher.finalize_xof());
    let mut commitment = [0_u8; 32];
    reader.fill(&mut commitment);
    Ok(commitment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ram_lfe::RAM_LFE_SECRET_MAX_BYTES;

    #[test]
    fn commitment_binds_backend_secret_and_length() {
        let backends = [
            RamLfeBackend::HkdfSha3_512PrfV1,
            RamLfeBackend::BfvAffineV1,
            RamLfeBackend::BfvProgrammedV1,
        ];
        let mut commitments = std::collections::BTreeSet::new();
        for backend in backends {
            for secret in [b"secret".as_slice(), b"secreu", b"secret\0"] {
                let digest = commit(backend, secret).expect("valid secret");
                assert_eq!(digest, commit(backend, secret).expect("same input"));
                assert!(commitments.insert(digest), "distinct framed inputs");
            }
        }
    }

    #[test]
    fn commitment_enforces_secret_boundaries() {
        let backend = RamLfeBackend::BfvProgrammedV1;
        assert_eq!(commit(backend, b""), Err(RamLfeError::EmptySecret));
        assert!(commit(backend, b"x").is_ok());
        assert!(commit(backend, &vec![0x5a; RAM_LFE_SECRET_MAX_BYTES]).is_ok());
        assert_eq!(
            commit(backend, &vec![0x5a; RAM_LFE_SECRET_MAX_BYTES + 1]),
            Err(RamLfeError::SecretTooLarge)
        );
    }

    #[test]
    fn canonical_commitment_binds_context_and_ignores_ambient_layout() {
        let input = PolicySecretInputV1 {
            backend: RamLfeBackend::BfvProgrammedV1,
            secret: b"canonical-secret",
        };
        let expected = commit_canonical(CONTEXT, &input).expect("canonical commitment");
        assert_ne!(
            expected,
            commit_canonical(PROGRAM_CONTEXT, &input).expect("different context")
        );
        for flags in [0, norito::core::default_encode_flags()] {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(
                expected,
                commit_canonical(CONTEXT, &input).expect("fixed canonical encoding")
            );
        }
    }
}
