/// Default (G1 pubkey, G2 signature) BLS suite.
pub use normal::NormalBls as BlsNormal;
/// Default BLS private key (G2 signature, G1 public key).
pub use normal::NormalPrivateKey as BlsNormalPrivateKey;
/// Default BLS public key (G2 signature, G1 public key).
pub use normal::NormalPublicKey as BlsNormalPublicKey;
/// Compact BLS suite (smaller signatures, slower ops).
pub use small::SmallBls as BlsSmall;
/// Compact BLS private key (smaller signatures).
pub use small::SmallPrivateKey as BlsSmallPrivateKey;
/// Compact BLS public key (smaller signatures).
pub use small::SmallPublicKey as BlsSmallPublicKey;

// Select backend implementation module
// - Default: compat w3f-bls (arkworks-based) when `bls-backend-blstrs` is NOT set
// - New: pure blstrs backend when `bls-backend-blstrs` is set
#[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
mod implementation;
#[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
#[path = "implementation_blstrs.rs"]
mod implementation;

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

// Public helpers to access aggregate-style verification from outside without
// exposing the internal configuration types.
pub fn verify_aggregate_same_message_normal(
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

pub fn verify_aggregate_same_message_small(
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

/// Multi-message aggregate verification (distinct messages), normal variant.
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

/// Multi-message aggregate verification (distinct messages), small variant.
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
pub fn verify_preaggregated_same_message_normal(
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
