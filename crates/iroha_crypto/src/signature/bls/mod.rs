/// Default (G1 pubkey, G2 signature) BLS suite.
pub use normal::NormalBls as BlsNormal;
/// Default BLS private key (G2 signature, G1 public key).
pub use normal::NormalPrivateKey as BlsNormalPrivateKey;
/// Default BLS public key (G2 signature, G1 public key).
pub use normal::NormalPublicKey as BlsNormalPublicKey;
/// Compact BLS suite (smaller signatures, slower ops).
///
/// Raw-key same-message aggregate verification is intentionally not part of
/// the public API because it is unsafe without verified proofs of possession.
/// Use [`crate::bls_small_verify_aggregate_same_message`] instead.
///
/// ```compile_fail
/// use iroha_crypto::BlsSmall;
///
/// let signature = [0_u8; 48];
/// let public_key = [0_u8; 96];
/// let signatures: [&[u8]; 1] = [&signature];
/// let public_keys: [&[u8]; 1] = [&public_key];
/// let _ = BlsSmall::verify_aggregate_same_message(
///     b"same message",
///     &signatures,
///     &public_keys,
/// );
/// ```
///
/// Pre-aggregated same-message verification has the same requirement and is
/// deliberately not exposed for BLS-small. Pass the individual signatures and
/// `PoPs` to [`crate::bls_small_verify_aggregate_same_message`] instead.
///
/// ```compile_fail
/// use iroha_crypto::BlsSmall;
///
/// let aggregate_signature = [0_u8; 48];
/// let public_key = [0_u8; 96];
/// let public_keys: [&[u8]; 1] = [&public_key];
/// let _ = BlsSmall::verify_preaggregated_same_message(
///     b"same message",
///     &aggregate_signature,
///     &public_keys,
/// );
/// ```
pub use small::SmallBls as BlsSmall;
/// Compact BLS private key (smaller signatures).
pub use small::SmallPrivateKey as BlsSmallPrivateKey;
/// Compact BLS public key (smaller signatures).
pub use small::SmallPublicKey as BlsSmallPublicKey;
// Select backend implementation module
// - Default: compat w3f-bls (arkworks-based) when `bls-backend-blstrs` is NOT set
// - New: pure blstrs backend when `bls-backend-blstrs` is set
mod ethereum;
#[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
mod implementation;
#[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
#[path = "implementation_blstrs.rs"]
mod implementation;
pub use ethereum::{
    ETHEREUM_BLS_POP_DST, ethereum_bls_pop_fast_aggregate_verify,
    ethereum_bls_pop_validate_public_key,
};
/// This version is the "normal" BLS signature scheme
/// with the public key group in G1 and signature group in G2.
/// 192 byte signatures and 97 byte public keys
mod normal {
    use super::{implementation, implementation::BlsConfiguration};
    use crate::Algorithm;
    #[derive(Debug, Clone, Copy)]
    pub struct NormalConfiguration;
    impl BlsConfiguration for NormalConfiguration {
        const ALGORITHM: Algorithm = Algorithm::BlsNormal;
        #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
        type Engine = w3f_bls::ZBLS;
        #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
        const NORMAL: bool = true;
    }
    /// Default (non-compact) BLS signature suite.
    pub type NormalBls = implementation::BlsImpl<NormalConfiguration>;
    /// Public key type for the default BLS suite.
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    pub type NormalPublicKey =
        w3f_bls::PublicKey<<NormalConfiguration as BlsConfiguration>::Engine>;
    /// Private key type for the default BLS suite.
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    pub type NormalPrivateKey = implementation::ManagedSecretKey<NormalConfiguration>;
    /// Public key type for the default BLS suite (blstrs backend).
    #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
    pub type NormalPublicKey = implementation::PublicKey<NormalConfiguration>;
    /// Private key type for the default BLS suite (blstrs backend).
    #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
    pub type NormalPrivateKey = implementation::SecretKey<NormalConfiguration>;
}
/// Small BLS signature scheme results in smaller signatures but slower
/// operations and bigger public key.
///
/// This is good for situations where space is a consideration and verification is infrequent.
mod small {
    use super::implementation::{self, BlsConfiguration};
    use crate::Algorithm;
    #[derive(Debug, Clone, Copy)]
    pub struct SmallConfiguration;
    impl BlsConfiguration for SmallConfiguration {
        const ALGORITHM: Algorithm = Algorithm::BlsSmall;
        #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
        type Engine = w3f_bls::TinyBLS381;
        #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
        const NORMAL: bool = false;
    }
    /// Compact BLS signature suite with smaller signatures.
    pub type SmallBls = implementation::BlsImpl<SmallConfiguration>;
    /// Public key type for the compact BLS suite.
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    pub type SmallPublicKey = w3f_bls::PublicKey<<SmallConfiguration as BlsConfiguration>::Engine>;
    /// Private key type for the compact BLS suite.
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    pub type SmallPrivateKey = implementation::ManagedSecretKey<SmallConfiguration>;
    /// Public key type for the compact BLS suite (blstrs backend).
    #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
    pub type SmallPublicKey = implementation::PublicKey<SmallConfiguration>;
    /// Private key type for the compact BLS suite (blstrs backend).
    #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
    pub type SmallPrivateKey = implementation::SecretKey<SmallConfiguration>;
}
#[cfg(test)]
mod tests;
// Crate-local helpers let the PoP-enforcing public wrappers share the
// aggregate implementations without exposing raw-key same-message checks.
pub(crate) fn verify_aggregate_same_message_normal(
    message: &[u8],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), crate::Error> {
    implementation::BlsImpl::<normal::NormalConfiguration>::verify_aggregate_same_message(
        message,
        signatures,
        public_keys,
    )
}
pub(crate) fn verify_aggregate_same_message_small(
    message: &[u8],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), crate::Error> {
    implementation::BlsImpl::<small::SmallConfiguration>::verify_aggregate_same_message(
        message,
        signatures,
        public_keys,
    )
}
/// Exact per-signature verification across distinct messages, normal variant.
///
/// Success proves every signature is valid for the public key and message at
/// the same index.
#[allow(dead_code)]
pub fn verify_aggregate_multi_message_normal(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), crate::Error> {
    implementation::BlsImpl::<normal::NormalConfiguration>::verify_aggregate_multi_message(
        messages,
        signatures,
        public_keys,
    )
}
/// Exact per-signature verification across distinct messages, small variant.
///
/// Success proves every signature is valid for the public key and message at
/// the same index.
#[allow(dead_code)]
pub fn verify_aggregate_multi_message_small(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), crate::Error> {
    implementation::BlsImpl::<small::SmallConfiguration>::verify_aggregate_multi_message(
        messages,
        signatures,
        public_keys,
    )
}
/// Aggregate (sum) signatures for the same-message case (normal variant: pk in G1, sig in G2).
/// Returns aggregated signature bytes.
#[cfg(feature = "bls")]
pub fn aggregate_same_message_normal(signatures: &[&[u8]]) -> Result<Vec<u8>, crate::Error> {
    implementation::BlsImpl::<normal::NormalConfiguration>::aggregate_signatures(signatures)
}
/// Verify a pre-aggregated signature for the same-message case (normal variant).
#[cfg(feature = "bls")]
pub(crate) fn verify_preaggregated_same_message_normal(
    message: &[u8],
    aggregated_signature: &[u8],
    public_keys: &[&[u8]],
) -> Result<(), crate::Error> {
    implementation::BlsImpl::<normal::NormalConfiguration>::verify_preaggregated_same_message(
        message,
        aggregated_signature,
        public_keys,
    )
}
