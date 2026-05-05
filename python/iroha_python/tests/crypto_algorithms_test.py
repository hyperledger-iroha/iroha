from __future__ import annotations

from iroha_python.crypto import (
    BLS_NORMAL_ALGORITHM,
    BLS_SMALL_ALGORITHM,
    ED25519_ALGORITHM,
    GOST_3410_2012_256_PARAMSET_A_ALGORITHM,
    GOST_3410_2012_256_PARAMSET_B_ALGORITHM,
    GOST_3410_2012_256_PARAMSET_C_ALGORITHM,
    GOST_3410_2012_512_PARAMSET_A_ALGORITHM,
    GOST_3410_2012_512_PARAMSET_B_ALGORITHM,
    ML_DSA_ALGORITHM,
    SECP256K1_ALGORITHM,
    SM2_ALGORITHM,
    SUPPORTED_CRYPTO_ALGORITHMS,
    CryptoKeyPair,
    derive_keypair_from_seed,
    load_keypair,
    load_keypair_from_multihash,
    normalize_crypto_algorithm,
    parse_private_key_multihash,
    parse_public_key_multihash,
    private_key_multihash,
    public_key_multihash,
    sign,
    supported_crypto_algorithms,
    verify,
)


EXPECTED_ALGORITHMS = (
    ED25519_ALGORITHM,
    SECP256K1_ALGORITHM,
    ML_DSA_ALGORITHM,
    GOST_3410_2012_256_PARAMSET_A_ALGORITHM,
    GOST_3410_2012_256_PARAMSET_B_ALGORITHM,
    GOST_3410_2012_256_PARAMSET_C_ALGORITHM,
    GOST_3410_2012_512_PARAMSET_A_ALGORITHM,
    GOST_3410_2012_512_PARAMSET_B_ALGORITHM,
    BLS_NORMAL_ALGORITHM,
    BLS_SMALL_ALGORITHM,
    SM2_ALGORITHM,
)


def test_supported_crypto_algorithms_include_all_rust_signature_suites() -> None:
    assert supported_crypto_algorithms() == SUPPORTED_CRYPTO_ALGORITHMS
    assert tuple(SUPPORTED_CRYPTO_ALGORITHMS) == EXPECTED_ALGORITHMS


def test_algorithm_aliases_normalize_to_canonical_labels() -> None:
    aliases = {
        "ed-25519": ED25519_ALGORITHM,
        "ECDSA-SECP256K1-SHA256": SECP256K1_ALGORITHM,
        "mldsa65": ML_DSA_ALGORITHM,
        "dilithium3": ML_DSA_ALGORITHM,
        "gost-3410-2012-256-paramset-a": GOST_3410_2012_256_PARAMSET_A_ALGORITHM,
        "gost3410_2012_512_paramset_b": GOST_3410_2012_512_PARAMSET_B_ALGORITHM,
        "bls-normal": BLS_NORMAL_ALGORITHM,
        "bls-small": BLS_SMALL_ALGORITHM,
        "SM2": SM2_ALGORITHM,
    }

    for alias, canonical in aliases.items():
        assert normalize_crypto_algorithm(alias) == canonical


def test_all_supported_algorithms_sign_verify_and_roundtrip_keys() -> None:
    message = b"python sdk all-algorithm signing smoke"

    for algorithm in SUPPORTED_CRYPTO_ALGORITHMS:
        keypair = derive_keypair_from_seed(f"iroha-python:{algorithm}".encode(), algorithm)
        signature = keypair.sign(message)

        assert keypair.algorithm == algorithm
        assert signature
        assert keypair.verify(message, signature)
        assert verify(algorithm, keypair.public_key, message, signature)
        assert not verify(algorithm, keypair.public_key, b"tampered", signature)

        loaded = load_keypair(keypair.private_key, algorithm)
        assert loaded.algorithm == algorithm
        assert loaded.private_key == keypair.private_key
        assert loaded.public_key == keypair.public_key
        assert loaded.verify(message, sign(algorithm, loaded.private_key, message))

        public_multihash = public_key_multihash(algorithm, keypair.public_key, prefixed=True)
        private_multihash = private_key_multihash(algorithm, keypair.private_key, prefixed=True)
        parsed_public_algorithm, parsed_public = parse_public_key_multihash(public_multihash)
        parsed_private_algorithm, parsed_private = parse_private_key_multihash(private_multihash)
        from_multihash = load_keypair_from_multihash(private_multihash)

        assert parsed_public_algorithm == algorithm
        assert parsed_public == keypair.public_key
        assert parsed_private_algorithm == algorithm
        assert parsed_private == keypair.private_key
        assert from_multihash == keypair
        assert CryptoKeyPair.from_private_key_multihash(private_multihash) == keypair
