//! End-to-end multihash roundtrip tests for `MlDsa` (Dilithium3) keys.

use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PublicKey};
use pqcrypto_dilithium::dilithium3 as dilithium;
use pqcrypto_traits::sign::{PublicKey as _, SecretKey as _};

#[test]
fn mldsa_public_key_multihash_roundtrip() {
    let kp = KeyPair::from_seed(b"iroha:ml-dsa:multihash:pk".to_vec(), Algorithm::MlDsa);
    let pk = dilithium::PublicKey::from_bytes(kp.public_key().to_bytes().1)
        .expect("seeded ML-DSA public key");
    let pubkey = PublicKey::from_bytes(iroha_crypto::Algorithm::MlDsa, pk.as_bytes())
        .expect("mldsa pk from bytes");
    // Serialize to multihash string
    let s = format!("{pubkey}");
    // Parse back from string
    let parsed: PublicKey = s.parse().expect("parse public key multihash");
    // Compare algorithm and payload bytes
    assert_eq!(pubkey.algorithm(), parsed.algorithm());
    assert_eq!(pubkey.to_bytes().1, parsed.to_bytes().1);
}

#[test]
fn mldsa_private_key_multihash_roundtrip_exposed() {
    let kp = KeyPair::from_seed(b"iroha:ml-dsa:multihash:sk".to_vec(), Algorithm::MlDsa);
    let sk = dilithium::SecretKey::from_bytes(&kp.private_key().to_bytes().1)
        .expect("seeded ML-DSA secret key");
    let prikey =
        iroha_crypto::PrivateKey::from_bytes(iroha_crypto::Algorithm::MlDsa, sk.as_bytes())
            .expect("mldsa sk from bytes");
    let exposed = ExposedPrivateKey(prikey.clone());
    // Serialize to multihash string via ExposedPrivateKey
    let s = format!("{exposed}");
    // Parse back via ExposedPrivateKey::from_str
    let parsed: ExposedPrivateKey = s.parse().expect("parse private key multihash");
    // Compare payload bytes recovered via to_bytes
    assert_eq!(prikey.algorithm(), parsed.0.algorithm());
    assert_eq!(prikey.to_bytes().1, parsed.0.to_bytes().1);
}
