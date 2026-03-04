//! Tests covering `KeyPair::from_private_key` helper.

use iroha_crypto::{Algorithm, KeyPair};

fn assert_roundtrip(algorithm: Algorithm) {
    let original = KeyPair::random_with_algorithm(algorithm);
    let private = original.private_key().clone();

    let reconstructed =
        KeyPair::from_private_key(private.clone()).expect("derived key pair should be valid");

    assert_eq!(
        original.public_key(),
        reconstructed.public_key(),
        "public key should match for {algorithm:?}"
    );
    assert_eq!(
        &private,
        reconstructed.private_key(),
        "private key should be preserved for {algorithm:?}"
    );

    let via_from: KeyPair = private.into();
    assert_eq!(
        reconstructed.public_key(),
        via_from.public_key(),
        "`From<PrivateKey>` should delegate to `from_private_key` for {algorithm:?}"
    );
}

#[test]
fn keypair_from_private_key_ed25519() {
    assert_roundtrip(Algorithm::Ed25519);
}

#[test]
fn keypair_from_private_key_secp256k1() {
    assert_roundtrip(Algorithm::Secp256k1);
}

#[cfg(feature = "bls")]
#[test]
fn keypair_from_private_key_bls_normal() {
    assert_roundtrip(Algorithm::BlsNormal);
}

#[cfg(feature = "bls")]
#[test]
fn keypair_from_private_key_bls_small() {
    assert_roundtrip(Algorithm::BlsSmall);
}

#[test]
fn keypair_from_private_key_mldsa() {
    assert_roundtrip(Algorithm::MlDsa);
}

#[cfg(feature = "gost")]
#[test]
fn keypair_from_private_key_gost_roundtrip() {
    for algorithm in [
        Algorithm::Gost3410_2012_256ParamSetA,
        Algorithm::Gost3410_2012_256ParamSetB,
        Algorithm::Gost3410_2012_256ParamSetC,
        Algorithm::Gost3410_2012_512ParamSetA,
        Algorithm::Gost3410_2012_512ParamSetB,
    ] {
        assert_roundtrip(algorithm);
    }
}
