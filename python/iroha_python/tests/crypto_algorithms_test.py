from __future__ import annotations

from array import array

import pytest

import iroha_python.crypto as crypto_module
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
    PRIVACY_CAPABILITY_VALIDATION_STATUS_V1,
    PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
    SECP256K1_ALGORITHM,
    SM2_ALGORITHM,
    SUPPORTED_CRYPTO_ALGORITHMS,
    CryptoKeyPair,
    derive_keypair_from_seed,
    generate_keypair,
    load_keypair,
    load_keypair_from_multihash,
    normalize_crypto_algorithm,
    parse_private_key_multihash,
    parse_public_key_multihash,
    is_privacy_native_available,
    privacy_bridge_abi_version,
    privacy_capabilities_v1,
    private_key_multihash,
    public_key_multihash,
    sign,
    supported_crypto_algorithms,
    verify,
    verify_ed25519,
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


def _signed_byte_array(data: bytes) -> array[int]:
    return array("b", (byte if byte < 128 else byte - 256 for byte in data))


def test_lane_privacy_attachment_rejects_padded_verifier_selectors() -> None:
    base = {
        "commitment_id": 7,
        "leaf": b"l" * 32,
        "leaf_index": 0,
        "audit_path": [None, b"a" * 32],
        "proof_backend": "halo2/ipa",
        "proof_bytes": b"proof",
        "verifying_key_name": "vk_lane_privacy",
    }

    normalized = crypto_module._normalize_lane_privacy_attachment(base)
    assert normalized["proof_backend"] == "halo2/ipa"
    assert normalized["verifying_key_name"] == "vk_lane_privacy"

    for override, message in [
        (
            {"proof_backend": " halo2/ipa"},
            r"proof_backend must not contain surrounding whitespace",
        ),
        (
            {"proof_backend": "halo2/ipa "},
            r"proof_backend must not contain surrounding whitespace",
        ),
        (
            {"verifying_key_name": " vk_lane_privacy"},
            r"verifying_key_name must not contain surrounding whitespace",
        ),
        (
            {"verifying_key_name": "vk_lane_privacy "},
            r"verifying_key_name must not contain surrounding whitespace",
        ),
    ]:
        with pytest.raises(ValueError, match=message):
            crypto_module._normalize_lane_privacy_attachment({**base, **override})


def test_supported_crypto_algorithms_include_all_rust_signature_suites() -> None:
    assert supported_crypto_algorithms() == SUPPORTED_CRYPTO_ALGORITHMS
    assert tuple(SUPPORTED_CRYPTO_ALGORITHMS) == EXPECTED_ALGORITHMS


def test_privacy_native_archive_cap_is_stable() -> None:
    assert PRIVACY_NATIVE_ARCHIVE_MAX_BYTES == 256 * 1024


def test_privacy_native_archive_cap_is_reexported_from_package_root() -> None:
    import iroha_python

    assert iroha_python.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES == PRIVACY_NATIVE_ARCHIVE_MAX_BYTES
    assert (
        iroha_python.PRIVACY_CAPABILITY_VALIDATION_STATUS_V1
        == PRIVACY_CAPABILITY_VALIDATION_STATUS_V1
    )
    assert "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES" in iroha_python.__all__
    assert "PRIVACY_CAPABILITY_VALIDATION_STATUS_V1" in iroha_python.__all__
    assert dict(PRIVACY_CAPABILITY_VALIDATION_STATUS_V1) == {
        "VALID": 0,
        "NULL_POINTER": 1,
        "EMPTY": 2,
        "ARCHIVE_TOO_LARGE": 3,
        "DECODE_RESOURCE_LIMIT": 4,
        "SCHEMA_MISMATCH": 5,
        "NON_CANONICAL": 6,
        "MALFORMED_ARCHIVE": 7,
        "INVALID_SNAPSHOT": 8,
    }
    for retired in (
        "PRIVACY_FFI_STATUS_ERROR",
        "privacy_proof_request_v1",
        "privacy_build_proof_v1",
        "privacy_verify_proof_v1",
    ):
        assert not hasattr(iroha_python, retired)
        assert retired not in iroha_python.__all__


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


def test_algorithm_labels_reject_empty_strings_before_native_normalization() -> None:
    with pytest.raises(ValueError, match="algorithm must be a non-empty string"):
        normalize_crypto_algorithm("")


def test_algorithm_labels_reject_empty_strings_across_public_api() -> None:
    keypair = derive_keypair_from_seed(b"strict empty algorithm label boundary", ED25519_ALGORITHM)
    message = b"strict empty algorithm label boundary message"
    signature = sign(ED25519_ALGORITHM, keypair.private_key, message)

    calls = (
        lambda: normalize_crypto_algorithm(""),
        lambda: generate_keypair(""),
        lambda: derive_keypair_from_seed(b"strict empty algorithm label boundary", ""),
        lambda: load_keypair(keypair.private_key, ""),
        lambda: public_key_multihash("", keypair.public_key),
        lambda: private_key_multihash("", keypair.private_key),
        lambda: sign("", keypair.private_key, message),
        lambda: verify("", keypair.public_key, message, signature),
        lambda: CryptoKeyPair("", keypair.private_key, keypair.public_key),
    )
    for call in calls:
        with pytest.raises(ValueError, match="algorithm must be a non-empty string"):
            call()


def test_algorithm_labels_reject_surrounding_whitespace_across_public_api() -> None:
    keypair = derive_keypair_from_seed(b"strict algorithm label boundary", ED25519_ALGORITHM)
    message = b"strict algorithm label boundary message"
    signature = sign(ED25519_ALGORITHM, keypair.private_key, message)

    labels = (
        " ed25519",
        "ed25519 ",
        "\ted25519",
        "ed25519\n",
        " eD-25519 ",
    )

    for label in labels:
        calls = (
            lambda: normalize_crypto_algorithm(label),
            lambda: generate_keypair(label),
            lambda: derive_keypair_from_seed(b"strict algorithm label boundary", label),
            lambda: load_keypair(keypair.private_key, label),
            lambda: public_key_multihash(label, keypair.public_key),
            lambda: private_key_multihash(label, keypair.private_key),
            lambda: sign(label, keypair.private_key, message),
            lambda: verify(label, keypair.public_key, message, signature),
            lambda: CryptoKeyPair(label, keypair.private_key, keypair.public_key),
        )
        for call in calls:
            with pytest.raises(
                ValueError,
                match="algorithm must not contain surrounding whitespace",
            ):
                call()


@pytest.mark.parametrize(
    ("label", "message"),
    [
        ("", "algorithm must be a non-empty string"),
        (" ed25519", "algorithm must not contain surrounding whitespace"),
        ("ed25519 ", "algorithm must not contain surrounding whitespace"),
        ("\ted25519", "algorithm must not contain surrounding whitespace"),
        ("ed25519\n", "algorithm must not contain surrounding whitespace"),
        (" eD-25519 ", "algorithm must not contain surrounding whitespace"),
    ],
)
def test_algorithm_labels_reject_empty_and_padded_native_inputs(
    label: str,
    message: str,
) -> None:
    keypair = derive_keypair_from_seed(b"strict native algorithm label boundary", ED25519_ALGORITHM)
    payload = b"strict native algorithm label boundary message"
    signature = sign(ED25519_ALGORITHM, keypair.private_key, payload)

    calls = (
        lambda: crypto_module._crypto.normalize_crypto_algorithm(label),
        lambda: crypto_module._crypto.generate_keypair(label),
        lambda: crypto_module._crypto.derive_keypair_from_seed(
            b"strict native algorithm label boundary",
            label,
        ),
        lambda: crypto_module._crypto.load_keypair(keypair.private_key, label),
        lambda: crypto_module._crypto.public_key_multihash(label, keypair.public_key, False),
        lambda: crypto_module._crypto.private_key_multihash(label, keypair.private_key, False),
        lambda: crypto_module._crypto.sign(label, keypair.private_key, payload),
        lambda: crypto_module._crypto.verify(label, keypair.public_key, payload, signature),
    )
    for call in calls:
        with pytest.raises(ValueError, match=message):
            call()


@pytest.mark.parametrize(
    "label",
    [
        "ed\00025519",
        "ed\u001f25519",
        "ed\u007f25519",
        "ed\u200b25519",
        "\u0435d25519",
        "ed\uff0d25519",
    ],
)
def test_algorithm_labels_reject_control_and_confusable_native_inputs(label: str) -> None:
    with pytest.raises(ValueError, match="unsupported crypto algorithm"):
        normalize_crypto_algorithm(label)


def test_asset_definition_id_builds_canonical_address_from_domain_and_name() -> None:
    asset = crypto_module.AssetDefinitionId.from_domain_and_name("boi.is", "ds")

    assert asset.canonical_address() == "56HTweMpySR2JErjpkisQ2FBTGnN"
    assert asset.value == "56HTweMpySR2JErjpkisQ2FBTGnN"
    assert asset.domain.value == "boi.is"


@pytest.mark.parametrize(
    ("domain_id", "name", "message"),
    [
        ("is", "ds", "domain id"),
        ("boi.is", "", "asset name"),
        ("boi.is", "not valid", "asset name"),
    ],
)
def test_asset_definition_id_rejects_invalid_domain_or_name(
    domain_id: str,
    name: str,
    message: str,
) -> None:
    with pytest.raises(ValueError, match=message):
        crypto_module.AssetDefinitionId.from_domain_and_name(domain_id, name)


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


def test_native_verify_rejects_empty_and_all_zero_signatures_before_backend() -> None:
    keypair = derive_keypair_from_seed(
        b"python native verify checked signature admission",
        ED25519_ALGORITHM,
    )
    message = b"python native verify checked signature admission message"

    for signature in (b"", bytes(64)):
        assert not verify(ED25519_ALGORITHM, keypair.public_key, message, signature)
        assert not verify_ed25519(keypair.public_key, message, signature)

    signature = bytearray(keypair.sign(message))
    malformed_r_cases = (
        (
            "small-order",
            bytes(
                [
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]
            ),
        ),
        ("noncanonical", bytes.fromhex("ee" + ("ff" * 30) + "7f")),
    )
    for _label, replacement_r in malformed_r_cases:
        malformed = bytearray(signature)
        malformed[:32] = replacement_r
        assert not verify(ED25519_ALGORITHM, keypair.public_key, message, bytes(malformed))
        assert not verify_ed25519(keypair.public_key, message, bytes(malformed))


def _privacy_capability_archive() -> bytes:
    frame = bytearray(43)
    frame[0:4] = b"NRT0"
    frame[6:22] = bytes([0x50]) * 16
    frame[23:31] = (3).to_bytes(8, "little")
    frame[31:39] = bytes.fromhex("b9d3a80ccd5d1324")
    frame[40:43] = bytes([0xA5, 0x5A, 0x11])
    return bytes(frame)


_DEFAULT_CAPABILITY_ARCHIVE = object()


class _CapabilityNative:
    def __init__(
        self,
        archive: object = _DEFAULT_CAPABILITY_ARCHIVE,
        *,
        abi: object = PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
    ) -> None:
        self.archive = (
            _privacy_capability_archive()
            if archive is _DEFAULT_CAPABILITY_ARCHIVE
            else archive
        )
        self.abi = abi

    def privacy_bridge_abi_version(self) -> object:
        return self.abi

    def privacy_capabilities_v1(self) -> object:
        return self.archive

    def privacy_validate_capabilities_v1(self, archive: bytes) -> int:
        return (
            PRIVACY_CAPABILITY_VALIDATION_STATUS_V1["VALID"]
            if archive == _privacy_capability_archive()
            else PRIVACY_CAPABILITY_VALIDATION_STATUS_V1["MALFORMED_ARCHIVE"]
        )


def test_privacy_native_capability_surface_is_minimal(monkeypatch: pytest.MonkeyPatch) -> None:
    import iroha_python

    monkeypatch.setattr(crypto_module, "_crypto", _CapabilityNative())
    assert is_privacy_native_available()
    assert privacy_bridge_abi_version() == PRIVACY_REQUIRED_BRIDGE_ABI_VERSION
    assert privacy_capabilities_v1() == _privacy_capability_archive()

    for surface in (crypto_module, iroha_python):
        for retired in (
            "privacy_proof_request_v1",
            "privacy_build_proof_v1",
            "privacy_verify_proof_v1",
            "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        ):
            assert not hasattr(surface, retired)


def test_privacy_native_availability_rejects_missing_stale_and_malformed_bridges(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for native in (
        object(),
        _CapabilityNative(abi=PRIVACY_REQUIRED_BRIDGE_ABI_VERSION - 1),
        _CapabilityNative(abi=PRIVACY_REQUIRED_BRIDGE_ABI_VERSION + 1),
        _CapabilityNative(abi=True),
        _CapabilityNative(abi="21"),
        _CapabilityNative(archive=b""),
        _CapabilityNative(archive=b"not-norito"),
    ):
        monkeypatch.setattr(crypto_module, "_crypto", native)
        assert not is_privacy_native_available()

    missing_validator = _CapabilityNative()
    missing_validator.privacy_validate_capabilities_v1 = None  # type: ignore[method-assign]
    monkeypatch.setattr(crypto_module, "_crypto", missing_validator)
    assert not is_privacy_native_available()


def test_privacy_capability_archive_rejects_adversarial_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    bad_magic = bytearray(_privacy_capability_archive())
    bad_magic[0] ^= 0xFF
    wrong_schema = bytearray(_privacy_capability_archive())
    wrong_schema[6:22] = bytes([0x42]) * 16
    bad_crc = bytearray(_privacy_capability_archive())
    bad_crc[31] ^= 0x01

    for output in (
        None,
        "json is not Norito",
        b"\x50",
        [0x50, 0x00],
        memoryview(array("H", [0x5050] * 24)),
        bytes(bad_magic),
        bytes(wrong_schema),
        bytes(bad_crc),
    ):
        monkeypatch.setattr(crypto_module, "_crypto", _CapabilityNative(output))
        with pytest.raises((RuntimeError, TypeError)):
            privacy_capabilities_v1()
        assert not is_privacy_native_available()


def test_privacy_capability_archive_is_defensively_copied(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native_archive = bytearray(_privacy_capability_archive())
    monkeypatch.setattr(crypto_module, "_crypto", _CapabilityNative(native_archive))
    returned = privacy_capabilities_v1()
    native_archive[0] ^= 0xFF
    assert returned == _privacy_capability_archive()


def test_privacy_capability_native_exceptions_are_sanitized(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class LeakingNative(_CapabilityNative):
        def privacy_capabilities_v1(self) -> object:
            raise RuntimeError("secret native implementation detail")

    monkeypatch.setattr(crypto_module, "_crypto", LeakingNative())
    with pytest.raises(RuntimeError, match="native privacy_capabilities_v1 failed") as error:
        privacy_capabilities_v1()
    assert "secret native implementation detail" not in str(error.value)
    assert error.value.__cause__ is None
