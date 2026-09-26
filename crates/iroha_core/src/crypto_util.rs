//! Small, consensus-neutral key predicates shared by validation paths.
use iroha_crypto::{Algorithm, PublicKey};
/// Return whether `public_key` is a BLS-normal key.
///
/// Uses checked algorithm access, so a key whose algorithm cannot be decoded
/// is treated as not BLS-normal instead of panicking.
pub(crate) fn is_bls_normal_public_key(public_key: &PublicKey) -> bool {
    public_key
        .try_algorithm()
        .is_ok_and(|algorithm| algorithm == Algorithm::BlsNormal)
}
#[cfg(test)]
mod tests {
    use super::is_bls_normal_public_key;
    use iroha_crypto::{Algorithm, KeyPair};
    #[test]
    fn bls_normal_public_key_check_uses_checked_algorithm_access() {
        let bls_key = KeyPair::try_from_seed(b"checked-bls-key".to_vec(), Algorithm::BlsNormal)
            .expect("derive BLS fixture key");
        let ed25519_key =
            KeyPair::try_from_seed(b"checked-ed25519-key".to_vec(), Algorithm::Ed25519)
                .expect("derive Ed25519 fixture key");
        assert!(is_bls_normal_public_key(bls_key.public_key()));
        assert!(!is_bls_normal_public_key(ed25519_key.public_key()));
    }
}
