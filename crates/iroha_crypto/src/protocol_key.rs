//! Deterministic public points for protocol-owned, non-signing identities.
use curve25519_dalek::edwards::CompressedEdwardsY;
use sha2::{Digest as _, Sha512};
use crate::{Algorithm, PublicKey};
const NON_SIGNING_ED25519_TRANSCRIPT_V1: &[u8] = b"iroha:non-signing-ed25519-public-point:v1";
fn update_framed(hasher: &mut Sha512, bytes: &[u8]) {
    let len = u64::try_from(bytes.len())
        .expect("in-memory protocol-key derivation input length must fit u64");
    hasher.update(len.to_be_bytes());
    hasher.update(bytes);
}
/// Derive a deterministic Ed25519 public point for a protocol-owned identity.
///
/// This is deliberately not seeded key generation. It rejection-samples a
/// canonical point in the prime-order subgroup from a domain-separated SHA-512
/// transcript, so deriving the account identifier does not reveal a signing
/// scalar. Under the discrete-log assumption, no private key for the returned
/// point is known.
///
/// Use this only for identities whose state is exclusively controlled by
/// protocol logic, such as custody accounts. It must not be used where any
/// participant is expected to sign as the derived identity. `domain` must be a
/// unique, versioned protocol constant. Fields are length-delimited, so their
/// boundaries are part of the derivation.
#[must_use]
pub fn derive_non_signing_ed25519_public_key(domain: &[u8], fields: &[&[u8]]) -> PublicKey {
    for counter in 0_u64..=u64::MAX {
        let mut hasher = Sha512::new();
        update_framed(&mut hasher, NON_SIGNING_ED25519_TRANSCRIPT_V1);
        update_framed(&mut hasher, domain);
        hasher.update(
            u64::try_from(fields.len())
                .expect("protocol-key derivation field count must fit u64")
                .to_be_bytes(),
        );
        for field in fields {
            update_framed(&mut hasher, field);
        }
        hasher.update(counter.to_be_bytes());
        let digest = hasher.finalize();
        let mut candidate = [0_u8; 32];
        let candidate_len = candidate.len();
        candidate.copy_from_slice(&digest[..candidate_len]);
        let compressed = CompressedEdwardsY(candidate);
        let Some(point) = compressed.decompress() else {
            continue;
        };
        if point.compress().as_bytes() != &candidate
            || point.is_small_order()
            || !point.is_torsion_free()
        {
            continue;
        }
        return PublicKey::from_bytes(Algorithm::Ed25519, &candidate)
            .expect("candidate was validated as a canonical prime-order Ed25519 point");
    }
    unreachable!("a SHA-512 transcript must eventually yield a valid Ed25519 subgroup point")
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::KeyPair;
    #[test]
    fn non_signing_protocol_key_is_stable_and_framed() {
        let derived = derive_non_signing_ed25519_public_key(
            b"iroha:test:protocol-account:v1",
            &[b"alpha", b"beta"],
        );
        let repeated = derive_non_signing_ed25519_public_key(
            b"iroha:test:protocol-account:v1",
            &[b"alpha", b"beta"],
        );
        assert_eq!(derived, repeated);
        let (algorithm, payload) = derived.to_bytes();
        assert_eq!(algorithm, Algorithm::Ed25519);
        assert_eq!(
            payload,
            &[
                0xd6, 0xb6, 0xad, 0xe8, 0x95, 0x59, 0xb7, 0xd0, 0x84, 0x39, 0x88, 0xf5, 0xe0, 0x75,
                0x12, 0x78, 0x5d, 0x2d, 0x36, 0x37, 0xba, 0x7a, 0xf9, 0x9e, 0xba, 0x93, 0xf8, 0xc3,
                0x6c, 0x7a, 0x36, 0xa1,
            ]
        );
        assert_ne!(
            derived,
            derive_non_signing_ed25519_public_key(
                b"iroha:test:protocol-account:v1",
                &[b"alph", b"abeta"],
            )
        );
        assert_ne!(
            derived,
            derive_non_signing_ed25519_public_key(
                b"iroha:test:other-protocol-account:v1",
                &[b"alpha", b"beta"],
            )
        );
    }
    #[test]
    fn non_signing_protocol_key_is_not_seeded_key_generation() {
        let fields: &[&[u8]] = &[b"public", b"inputs"];
        let derived =
            derive_non_signing_ed25519_public_key(b"iroha:test:protocol-account:v1", fields);
        let legacy_seed = fields.concat();
        let legacy = KeyPair::try_from_seed(legacy_seed, Algorithm::Ed25519)
            .expect("non-zero fixture seed derives");
        assert_ne!(&derived, legacy.public_key());
    }
}
