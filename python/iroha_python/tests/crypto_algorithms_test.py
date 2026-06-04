from __future__ import annotations

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
    PRIVACY_FFI_ERROR_INVALID_REQUEST,
    PRIVACY_FFI_ERROR_MALFORMED_NORITO,
    PRIVACY_FFI_ERROR_NULL_POINTER,
    PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
    PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM,
    PRIVACY_FFI_STATUS_ERROR,
    PRIVACY_FFI_VERSION_V1,
    PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
    PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
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
    is_privacy_native_available,
    privacy_bridge_abi_version,
    privacy_build_proof_v1,
    privacy_capabilities_v1,
    privacy_verify_proof_v1,
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


def test_privacy_native_archive_cap_is_stable() -> None:
    assert PRIVACY_NATIVE_ARCHIVE_MAX_BYTES == 64 * 1024 * 1024


def test_privacy_native_archive_cap_is_reexported_from_package_root() -> None:
    import iroha_python

    assert iroha_python.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES == PRIVACY_NATIVE_ARCHIVE_MAX_BYTES
    assert "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES" in iroha_python.__all__
    assert iroha_python.PRIVACY_FFI_STATUS_ERROR == PRIVACY_FFI_STATUS_ERROR
    assert "PRIVACY_FFI_STATUS_ERROR" in iroha_python.__all__


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


def _privacy_norito_frame(schema_byte: int) -> bytes:
    frame = bytearray(40)
    frame[0:4] = b"NRT0"
    frame[6:22] = bytes([schema_byte]) * 16
    return bytes(frame)


def _privacy_norito_frame_with_payload(schema_byte: int) -> bytes:
    frame = bytearray(_privacy_norito_frame(schema_byte) + b"\x00\x00\xa5\x5a\x11")
    frame[23:31] = (3).to_bytes(8, "little")
    frame[31:39] = bytes.fromhex("b9d3a80ccd5d1324")
    return bytes(frame)


def _privacy_norito_frame_with_padding(schema_byte: int, padding_length: int) -> bytes:
    frame = bytearray(
        _privacy_norito_frame(schema_byte)
        + (b"\x00" * padding_length)
        + b"\xa5\x5a\x11"
    )
    frame[23:31] = (3).to_bytes(8, "little")
    frame[31:39] = bytes.fromhex("b9d3a80ccd5d1324")
    return bytes(frame)


def _privacy_norito_frame_with_schema_override(
    schema_byte: int,
    offset: int,
    value: int,
) -> bytes:
    frame = bytearray(_privacy_norito_frame_with_payload(schema_byte))
    frame[offset] = value
    return bytes(frame)


def _privacy_norito_frame_with_declared_payload_length(
    schema_byte: int,
    payload_length: int,
) -> bytes:
    frame = bytearray(_privacy_norito_frame_with_payload(schema_byte))
    frame[23:31] = payload_length.to_bytes(8, "little")
    return bytes(frame)


def _privacy_norito_frame_with_flags(schema_byte: int, flags: int) -> bytes:
    frame = bytearray(_privacy_norito_frame_with_payload(schema_byte))
    frame[39] = flags
    return bytes(frame)


_PRIVACY_CAPABILITIES_ARCHIVE = _privacy_norito_frame_with_payload(0x50)
_PRIVACY_BUILD_ARCHIVE = _privacy_norito_frame_with_payload(0x42)
_PRIVACY_VERIFY_ARCHIVE = _privacy_norito_frame_with_payload(0x56)
_PRIVACY_REQUEST_ARCHIVE = _privacy_norito_frame_with_payload(0x52)


def _malformed_privacy_native_output_archives(schema_byte: int) -> tuple[bytes, ...]:
    archive = _privacy_norito_frame_with_payload(schema_byte)
    bad_magic = bytearray(archive)
    bad_magic[0] = 0x00
    bad_version = bytearray(archive)
    bad_version[4] = 1
    bad_minor_version = bytearray(archive)
    bad_minor_version[5] = 1
    bad_compression = bytearray(archive)
    bad_compression[22] = 1
    bad_declared_payload_length = _privacy_norito_frame_with_declared_payload_length(
        schema_byte,
        6,
    )
    bad_oversized_declared_payload_length = _privacy_norito_frame_with_declared_payload_length(
        schema_byte,
        0x8000000000000000,
    )
    bad_padding = archive + b"\x7f"
    bad_excessive_padding = _privacy_norito_frame_with_padding(schema_byte, 65)
    bad_flags = bytearray(archive)
    bad_flags[39] = 0x08
    bad_field_bitset_flags = bytearray(archive)
    bad_field_bitset_flags[39] = 0x20
    bad_checksum = bytearray(archive)
    bad_checksum[31] ^= 0x01
    bad_payload = bytearray(archive)
    bad_payload[44] ^= 0x7F
    return (
        b"not norito",
        bytes(bad_magic),
        bytes(bad_version),
        bytes(bad_minor_version),
        bytes(bad_compression),
        bad_declared_payload_length,
        bad_oversized_declared_payload_length,
        bad_padding,
        bad_excessive_padding,
        bytes(bad_flags),
        bytes(bad_field_bitset_flags),
        bytes(bad_checksum),
        bytes(bad_payload),
    )


def _sliced_privacy_memoryview(
    archive: bytes,
    prefix: bytes = b"\xff\x7f\x42",
    suffix: bytes = b"\x24\x13",
) -> memoryview:
    backing = bytearray(prefix + archive + suffix)
    return memoryview(backing)[len(prefix) : len(prefix) + len(archive)]


def _malformed_privacy_request_archives() -> tuple[bytes, ...]:
    bad_magic = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_magic[0] = 0x00
    bad_version = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_version[4] = 1
    bad_minor_version = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_minor_version[5] = 1
    bad_compression = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_compression[22] = 1
    bad_declared_payload_length = _privacy_norito_frame_with_declared_payload_length(0x52, 6)
    bad_oversized_declared_payload_length = _privacy_norito_frame_with_declared_payload_length(
        0x52,
        0x8000000000000000,
    )
    bad_padding = _PRIVACY_REQUEST_ARCHIVE + b"\x7f"
    bad_excessive_padding = _privacy_norito_frame_with_padding(0x52, 65)
    bad_flags = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_flags[39] = 0x08
    bad_field_bitset_flags = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_field_bitset_flags[39] = 0x20
    bad_checksum = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_checksum[31] ^= 0x01
    bad_payload = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    bad_payload[44] ^= 0x7F
    return (
        b"not norito",
        bytes(bad_magic),
        bytes(bad_version),
        bytes(bad_minor_version),
        bytes(bad_compression),
        bad_declared_payload_length,
        bad_oversized_declared_payload_length,
        bad_padding,
        bad_excessive_padding,
        bytes(bad_flags),
        bytes(bad_field_bitset_flags),
        bytes(bad_checksum),
        bytes(bad_payload),
    )


class _FakePrivacyNative:
    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> bytes:
        return _PRIVACY_CAPABILITIES_ARCHIVE

    def privacy_build_proof_v1(self, request_archive: bytes) -> bytes:
        assert request_archive
        return _PRIVACY_BUILD_ARCHIVE

    def privacy_verify_proof_v1(self, request_archive: bytes) -> bytes:
        assert request_archive
        return _PRIVACY_VERIFY_ARCHIVE


class _FakePrivacyNativeMustNotDispatch:
    def privacy_bridge_abi_version(self) -> int:
        pytest.fail("invalid privacy request must not probe native ABI")

    def privacy_capabilities_v1(self) -> bytes:
        pytest.fail("invalid privacy request must not call native capabilities")

    def privacy_build_proof_v1(self, request_archive: bytes) -> bytes:
        pytest.fail("invalid privacy request must not call native build")

    def privacy_verify_proof_v1(self, request_archive: bytes) -> bytes:
        pytest.fail("invalid privacy request must not call native verify")


class _FakeTextPrivacyNative:
    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> str:
        return "json is not a Norito archive"

    def privacy_build_proof_v1(self, request_archive: bytes) -> str:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return "not bytes"

    def privacy_verify_proof_v1(self, request_archive: bytes) -> str:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return "not bytes"


class _FakeNoOutputPrivacyNative:
    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> None:
        return None

    def privacy_build_proof_v1(self, request_archive: bytes) -> bytes:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return b""

    def privacy_verify_proof_v1(self, request_archive: bytes) -> None:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return None


class _FakeOversizedOutputPrivacyNative:
    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> bytes:
        return bytes([0x7F]) * (len(_PRIVACY_REQUEST_ARCHIVE) + 1)

    def privacy_build_proof_v1(self, request_archive: bytes) -> bytes:
        assert request_archive
        return bytes([0x7F]) * (len(_PRIVACY_REQUEST_ARCHIVE) + 1)

    def privacy_verify_proof_v1(self, request_archive: bytes) -> bytes:
        assert request_archive
        return bytes([0x7F]) * (len(_PRIVACY_REQUEST_ARCHIVE) + 1)


class _FakeListOutputPrivacyNative:
    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> list[int]:
        return [0x50, 0x01]

    def privacy_build_proof_v1(self, request_archive: bytes) -> list[int]:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return [0x42]

    def privacy_verify_proof_v1(self, request_archive: bytes) -> list[int]:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return [0x56]


class _FakeMutableOutputPrivacyNative:
    def __init__(self) -> None:
        self.capabilities_output = bytearray(_PRIVACY_CAPABILITIES_ARCHIVE)
        self.build_output = bytearray(_PRIVACY_BUILD_ARCHIVE)
        self.verify_backing = bytearray(b"\x00" + _PRIVACY_VERIFY_ARCHIVE + b"\x00")
        self.verify_output = memoryview(self.verify_backing)[1:-1]

    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> bytearray:
        return self.capabilities_output

    def privacy_build_proof_v1(self, request_archive: bytes) -> bytearray:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return self.build_output

    def privacy_verify_proof_v1(self, request_archive: bytes) -> memoryview:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return self.verify_output


class _FakeSlicedOutputPrivacyNative:
    def __init__(self) -> None:
        self.capabilities_backing = bytearray(b"\xff\x7f\x50" + _PRIVACY_CAPABILITIES_ARCHIVE + b"\x24")
        self.build_backing = bytearray(b"\xff\x7f\x42" + _PRIVACY_BUILD_ARCHIVE + b"\x13")
        self.verify_backing = bytearray(b"\xff\x7f\x56" + _PRIVACY_VERIFY_ARCHIVE + b"\x37")
        self.prefix_len = 3

    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> memoryview:
        return memoryview(self.capabilities_backing)[
            self.prefix_len : self.prefix_len + len(_PRIVACY_CAPABILITIES_ARCHIVE)
        ]

    def privacy_build_proof_v1(self, request_archive: bytes) -> memoryview:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return memoryview(self.build_backing)[
            self.prefix_len : self.prefix_len + len(_PRIVACY_BUILD_ARCHIVE)
        ]

    def privacy_verify_proof_v1(self, request_archive: bytes) -> memoryview:
        assert request_archive == _PRIVACY_REQUEST_ARCHIVE
        return memoryview(self.verify_backing)[
            self.prefix_len : self.prefix_len + len(_PRIVACY_VERIFY_ARCHIVE)
        ]


class _FakeLeakingExceptionPrivacyNative:
    def __init__(self, witness: bytes) -> None:
        self.witness = witness
        self.requests: list[bytearray] = []

    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def _raise(self) -> None:
        raise RuntimeError(f"native panic included {self.witness.decode()}")

    def privacy_capabilities_v1(self) -> bytes:
        self._raise()

    def privacy_build_proof_v1(self, request_archive: bytes) -> bytes:
        self.requests.append(request_archive)  # type: ignore[arg-type]
        assert bytes(request_archive) == _PRIVACY_REQUEST_ARCHIVE
        self._raise()

    def privacy_verify_proof_v1(self, request_archive: bytes) -> bytes:
        self.requests.append(request_archive)  # type: ignore[arg-type]
        assert bytes(request_archive) == _PRIVACY_REQUEST_ARCHIVE
        self._raise()


class _FakeCapturingPrivacyNative:
    def __init__(self, expected_archive: bytes, caller_archive: object) -> None:
        self.expected_archive = expected_archive
        self.caller_archive = caller_archive
        self.requests: list[bytearray] = []

    def privacy_bridge_abi_version(self) -> int:
        return PRIVACY_REQUIRED_BRIDGE_ABI_VERSION

    def privacy_capabilities_v1(self) -> bytes:
        return _PRIVACY_CAPABILITIES_ARCHIVE

    def privacy_build_proof_v1(self, request_archive: bytes) -> bytes:
        self._capture(request_archive)
        return _PRIVACY_BUILD_ARCHIVE

    def privacy_verify_proof_v1(self, request_archive: bytes) -> bytes:
        self._capture(request_archive)
        return _PRIVACY_VERIFY_ARCHIVE

    def _capture(self, request_archive: bytes) -> None:
        self.requests.append(request_archive)  # type: ignore[arg-type]
        assert request_archive is not self.caller_archive
        assert bytes(request_archive) == self.expected_archive


def test_privacy_native_capabilities_are_opaque_norito_bytes(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakePrivacyNative())

    archive = privacy_capabilities_v1()

    assert PRIVACY_FFI_VERSION_V1 == 1
    assert PRIVACY_REQUIRED_BRIDGE_ABI_VERSION == 6
    assert PRIVACY_FFI_STATUS_ERROR == 1
    assert PRIVACY_FFI_ERROR_NULL_POINTER == 1
    assert PRIVACY_FFI_ERROR_MALFORMED_NORITO == 2
    assert PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM == 3
    assert PRIVACY_FFI_ERROR_PRODUCTION_DISABLED == 4
    assert PRIVACY_FFI_ERROR_INVALID_REQUEST == 5
    assert privacy_bridge_abi_version() == 6
    assert is_privacy_native_available() is True
    assert isinstance(archive, bytes)
    assert len(archive) > 0
    assert not archive.startswith(b"{")


def test_privacy_native_availability_requires_abi_6(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for abi_version in (None, 5, True, "6"):
        native = _FakePrivacyNative()
        if abi_version is None:
            native.privacy_bridge_abi_version = None  # type: ignore[method-assign]
        else:
            native.privacy_bridge_abi_version = lambda abi_version=abi_version: abi_version
        monkeypatch.setattr(crypto_module, "_crypto", native)

        assert is_privacy_native_available() is False
        with pytest.raises(RuntimeError, match="privacy FFI requires native bridge ABI 6"):
            privacy_capabilities_v1()


def test_privacy_native_availability_rejects_broken_abi_probe(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _FakePrivacyNative()

    def broken_abi_probe() -> int:
        raise OSError("bridge denied")

    native.privacy_bridge_abi_version = broken_abi_probe
    monkeypatch.setattr(crypto_module, "_crypto", native)

    assert is_privacy_native_available() is False
    with pytest.raises(RuntimeError, match="privacy FFI requires native bridge ABI 6"):
        privacy_bridge_abi_version()
    with pytest.raises(RuntimeError, match="privacy FFI requires native bridge ABI 6"):
        privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)


def test_privacy_native_availability_requires_complete_method_surface(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _FakePrivacyNative()
    native.privacy_verify_proof_v1 = None  # type: ignore[method-assign]
    monkeypatch.setattr(crypto_module, "_crypto", native)

    assert is_privacy_native_available() is False
    with pytest.raises(
        RuntimeError,
        match="iroha_python._crypto is missing privacy_verify_proof_v1",
    ):
        privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)
    with pytest.raises(
        RuntimeError,
        match="privacy FFI requires complete native method surface; missing privacy_verify_proof_v1",
    ):
        privacy_capabilities_v1()
    with pytest.raises(
        RuntimeError,
        match="privacy FFI requires complete native method surface; missing privacy_verify_proof_v1",
    ):
        privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)


def test_privacy_native_availability_probes_use_norito_request_archives(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class _ProbeCapturePrivacyNative(_FakePrivacyNative):
        def __init__(self) -> None:
            self.build_request: bytearray | None = None
            self.verify_request: bytearray | None = None
            self.build_request_bytes: bytes | None = None
            self.verify_request_bytes: bytes | None = None

        def privacy_build_proof_v1(self, request_archive: bytearray) -> bytes:
            self.build_request = request_archive
            self.build_request_bytes = bytes(request_archive)
            return _PRIVACY_BUILD_ARCHIVE

        def privacy_verify_proof_v1(self, request_archive: bytearray) -> bytes:
            self.verify_request = request_archive
            self.verify_request_bytes = bytes(request_archive)
            return _PRIVACY_VERIFY_ARCHIVE

    native = _ProbeCapturePrivacyNative()
    monkeypatch.setattr(crypto_module, "_crypto", native)

    assert crypto_module._PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE == (
        _privacy_norito_frame(0x52)
    )
    assert crypto_module._PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE != (
        b"iroha-privacy-native-availability-probe-v1"
    )
    assert is_privacy_native_available() is True

    assert native.build_request_bytes == _privacy_norito_frame(0x52)
    assert native.verify_request_bytes == _privacy_norito_frame(0x52)
    assert native.build_request is not None
    assert native.verify_request is not None
    assert all(value == 0 for value in native.build_request)
    assert all(value == 0 for value in native.verify_request)


def test_privacy_native_availability_probes_clear_request_copies_after_failures(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class _ThrowingBuildProbePrivacyNative(_FakePrivacyNative):
        def __init__(self) -> None:
            self.build_request: bytearray | None = None

        def privacy_build_proof_v1(self, request_archive: bytearray) -> bytes:
            self.build_request = request_archive
            raise RuntimeError("probe failure after request copy")

    class _BadVerifyOutputProbePrivacyNative(_FakePrivacyNative):
        def __init__(self) -> None:
            self.verify_request: bytearray | None = None

        def privacy_verify_proof_v1(self, request_archive: bytearray) -> bytes:
            self.verify_request = request_archive
            return b"\x56"

    throwing_native = _ThrowingBuildProbePrivacyNative()
    monkeypatch.setattr(crypto_module, "_crypto", throwing_native)
    assert is_privacy_native_available() is False
    assert throwing_native.build_request is not None
    assert all(value == 0 for value in throwing_native.build_request)

    bad_output_native = _BadVerifyOutputProbePrivacyNative()
    monkeypatch.setattr(crypto_module, "_crypto", bad_output_native)
    assert is_privacy_native_available() is False
    assert bad_output_native.verify_request is not None
    assert all(value == 0 for value in bad_output_native.verify_request)


def test_privacy_native_availability_probes_reject_unsafe_raw_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def build_cases() -> tuple[tuple[str, object], ...]:
        return (
            ("privacy_capabilities_v1", lambda: "json is not Norito"),
            ("privacy_build_proof_v1", lambda _request: b""),
            ("privacy_verify_proof_v1", lambda _request: None),
            ("privacy_build_proof_v1", lambda _request: [0x42]),
            ("privacy_build_proof_v1", lambda _request: b"\x42"),
            (
                "privacy_verify_proof_v1",
                lambda _request: (_ for _ in ()).throw(RuntimeError("probe failed")),
            ),
        )

    cases: list[tuple[str, object]] = list(build_cases())
    for archive in _malformed_privacy_native_output_archives(0x50):
        cases.append(("privacy_capabilities_v1", lambda archive=archive: archive))
    for archive in _malformed_privacy_native_output_archives(0x42):
        cases.append(("privacy_build_proof_v1", lambda _request, archive=archive: archive))
    for archive in _malformed_privacy_native_output_archives(0x56):
        cases.append(("privacy_verify_proof_v1", lambda _request, archive=archive: archive))

    for operation, replacement in cases:
        native = _FakePrivacyNative()
        setattr(native, operation, replacement)
        monkeypatch.setattr(crypto_module, "_crypto", native)

        assert is_privacy_native_available() is False

    monkeypatch.setattr(crypto_module, "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES", 2)
    for operation, replacement in (
        ("privacy_capabilities_v1", lambda: b"\x50\x01\x7f"),
        ("privacy_build_proof_v1", lambda _request: b"\x42\x7f\x7f"),
        ("privacy_verify_proof_v1", lambda _request: b"\x56\x7f\x7f"),
    ):
        native = _FakePrivacyNative()
        setattr(native, operation, replacement)
        monkeypatch.setattr(crypto_module, "_crypto", native)

        assert is_privacy_native_available() is False


def test_privacy_native_wrappers_reject_wrong_operation_result_schemas(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    cases = (
        (
            "privacy_capabilities_v1",
            lambda: _privacy_norito_frame_with_schema_override(0x50, 21, 0x42),
            privacy_capabilities_v1,
        ),
        (
            "privacy_build_proof_v1",
            lambda _request: _privacy_norito_frame_with_schema_override(0x42, 6, 0x56),
            lambda: privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE),
        ),
        (
            "privacy_verify_proof_v1",
            lambda _request: _privacy_norito_frame_with_schema_override(0x56, 21, 0x50),
            lambda: privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE),
        ),
    )

    for operation, replacement, invoke in cases:
        native = _FakePrivacyNative()
        setattr(native, operation, replacement)
        monkeypatch.setattr(crypto_module, "_crypto", native)

        assert is_privacy_native_available() is False
        with pytest.raises(
            RuntimeError,
            match=f"native {operation} returned unexpected privacy result schema",
        ):
            invoke()


def test_privacy_native_build_and_verify_reject_empty_request_archive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakePrivacyNative())

    with pytest.raises(ValueError, match="request_archive must not be empty"):
        privacy_build_proof_v1(b"")

    with pytest.raises(ValueError, match="request_archive must not be empty"):
        privacy_verify_proof_v1(memoryview(b""))


def test_privacy_native_build_and_verify_reject_oversized_request_archive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES", 2)
    monkeypatch.setattr(crypto_module, "_crypto", _FakePrivacyNativeMustNotDispatch())

    with pytest.raises(ValueError, match="request_archive must not exceed 2 bytes"):
        crypto_module.privacy_build_proof_v1(b"\x01\x02\x03")
    with pytest.raises(ValueError, match="request_archive must not exceed 2 bytes"):
        crypto_module.privacy_verify_proof_v1(memoryview(b"\x01\x02\x03"))


def test_privacy_native_build_and_verify_reject_text_request_archive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakePrivacyNative())

    with pytest.raises(
        TypeError,
        match="request_archive must be Norito V1 bytes, not a string",
    ):
        privacy_build_proof_v1("not norito")  # type: ignore[arg-type]

    with pytest.raises(
        TypeError,
        match="request_archive must be Norito V1 bytes, not a string",
    ):
        privacy_verify_proof_v1("not norito")  # type: ignore[arg-type]


def test_privacy_native_build_and_verify_reject_integer_list_request_archive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakePrivacyNative())

    with pytest.raises(TypeError, match="request_archive must be bytes-like"):
        privacy_build_proof_v1([1, 2, 3])  # type: ignore[arg-type]

    with pytest.raises(TypeError, match="request_archive must be bytes-like"):
        privacy_verify_proof_v1([1, 2, 3])  # type: ignore[arg-type]


def test_privacy_native_build_and_verify_accept_max_header_padding(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    padded_request = _privacy_norito_frame_with_padding(0x52, 64)
    native = _FakeCapturingPrivacyNative(padded_request, padded_request)
    monkeypatch.setattr(crypto_module, "_crypto", native)

    assert privacy_build_proof_v1(padded_request) == _PRIVACY_BUILD_ARCHIVE
    assert privacy_verify_proof_v1(bytearray(padded_request)) == _PRIVACY_VERIFY_ARCHIVE
    assert len(native.requests) == 2
    for request in native.requests:
        assert isinstance(request, bytearray)
        assert all(value == 0 for value in request)


def test_privacy_native_build_and_verify_accept_complete_field_bitset_flags(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    request_archive = _privacy_norito_frame_with_flags(0x52, 0x26)
    build_archive = _privacy_norito_frame_with_flags(0x42, 0x26)
    verify_archive = _privacy_norito_frame_with_flags(0x56, 0x26)

    class _NativeWithFieldBitsetOutput(_FakeCapturingPrivacyNative):
        def privacy_build_proof_v1(self, request_archive: bytes) -> bytes:
            super().privacy_build_proof_v1(request_archive)
            return build_archive

        def privacy_verify_proof_v1(self, request_archive: bytes) -> bytes:
            super().privacy_verify_proof_v1(request_archive)
            return verify_archive

    native = _NativeWithFieldBitsetOutput(request_archive, request_archive)
    monkeypatch.setattr(crypto_module, "_crypto", native)

    assert privacy_build_proof_v1(request_archive) == build_archive
    assert privacy_verify_proof_v1(bytearray(request_archive)) == verify_archive
    assert len(native.requests) == 2
    for request in native.requests:
        assert isinstance(request, bytearray)
        assert all(value == 0 for value in request)


def test_privacy_native_build_and_verify_reject_invalid_request_archive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakePrivacyNativeMustNotDispatch())

    for wrong_schema_archive in (
        _PRIVACY_CAPABILITIES_ARCHIVE,
        _PRIVACY_BUILD_ARCHIVE,
        _PRIVACY_VERIFY_ARCHIVE,
        _privacy_norito_frame_with_schema_override(0x52, 6, 0x42),
        _privacy_norito_frame_with_schema_override(0x52, 21, 0x56),
    ):
        with pytest.raises(
            ValueError,
            match="request_archive must use the privacy request schema",
        ):
            privacy_build_proof_v1(wrong_schema_archive)
        with pytest.raises(
            ValueError,
            match="request_archive must use the privacy request schema",
        ):
            privacy_verify_proof_v1(bytearray(wrong_schema_archive))

    for malformed_archive in _malformed_privacy_request_archives():
        with pytest.raises(
            ValueError,
            match="request_archive must be a valid Norito V1 archive",
        ):
            privacy_build_proof_v1(malformed_archive)
        with pytest.raises(
            ValueError,
            match="request_archive must be a valid Norito V1 archive",
        ):
            privacy_verify_proof_v1(bytearray(malformed_archive))


def test_privacy_native_build_and_verify_clear_temporary_request_copy(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    request_archive = bytearray(_PRIVACY_REQUEST_ARCHIVE)
    original_archive = bytes(request_archive)
    native = _FakeCapturingPrivacyNative(original_archive, request_archive)
    monkeypatch.setattr(crypto_module, "_crypto", native)

    assert privacy_build_proof_v1(request_archive) == _PRIVACY_BUILD_ARCHIVE
    assert privacy_verify_proof_v1(memoryview(request_archive)) == _PRIVACY_VERIFY_ARCHIVE

    assert bytes(request_archive) == original_archive
    assert len(native.requests) == 2
    for request in native.requests:
        assert isinstance(request, bytearray)
        assert all(value == 0 for value in request)


def test_privacy_native_build_and_verify_respect_sliced_request_views(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    build_view = _sliced_privacy_memoryview(_PRIVACY_REQUEST_ARCHIVE)
    verify_view = _sliced_privacy_memoryview(
        _PRIVACY_REQUEST_ARCHIVE,
        prefix=b"\x99\x88",
        suffix=b"\x77",
    )
    native = _FakeCapturingPrivacyNative(_PRIVACY_REQUEST_ARCHIVE, build_view)
    monkeypatch.setattr(crypto_module, "_crypto", native)

    assert privacy_build_proof_v1(build_view) == _PRIVACY_BUILD_ARCHIVE
    assert privacy_verify_proof_v1(verify_view) == _PRIVACY_VERIFY_ARCHIVE

    assert build_view.tobytes() == _PRIVACY_REQUEST_ARCHIVE
    assert verify_view.tobytes() == _PRIVACY_REQUEST_ARCHIVE
    assert len(native.requests) == 2
    for request in native.requests:
        assert isinstance(request, bytearray)
        assert all(value == 0 for value in request)


def test_privacy_native_wrappers_reject_textual_native_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakeTextPrivacyNative())

    with pytest.raises(
        RuntimeError,
        match="native privacy_capabilities_v1 returned text instead of Norito V1 bytes",
    ):
        privacy_capabilities_v1()

    with pytest.raises(
        RuntimeError,
        match="native privacy_build_proof_v1 returned text instead of Norito V1 bytes",
    ):
        privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)

    with pytest.raises(
        RuntimeError,
        match="native privacy_verify_proof_v1 returned text instead of Norito V1 bytes",
    ):
        privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)


def test_privacy_native_wrappers_reject_list_native_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakeListOutputPrivacyNative())

    with pytest.raises(
        TypeError,
        match="native privacy_capabilities_v1 returned non-byte output",
    ):
        privacy_capabilities_v1()

    with pytest.raises(
        TypeError,
        match="native privacy_build_proof_v1 returned non-byte output",
    ):
        privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)

    with pytest.raises(
        TypeError,
        match="native privacy_verify_proof_v1 returned non-byte output",
    ):
        privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)


def test_privacy_native_wrappers_reject_missing_and_empty_native_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(crypto_module, "_crypto", _FakeNoOutputPrivacyNative())

    with pytest.raises(
        RuntimeError,
        match="native privacy_capabilities_v1 returned no output",
    ):
        privacy_capabilities_v1()

    with pytest.raises(
        RuntimeError,
        match="native privacy_build_proof_v1 returned empty output",
    ):
        privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)

    with pytest.raises(
        RuntimeError,
        match="native privacy_verify_proof_v1 returned no output",
    ):
        privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)


def test_privacy_native_wrappers_reject_oversized_native_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        crypto_module,
        "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        len(_PRIVACY_REQUEST_ARCHIVE),
    )
    monkeypatch.setattr(crypto_module, "_crypto", _FakeOversizedOutputPrivacyNative())

    assert crypto_module.is_privacy_native_available() is False

    with pytest.raises(
        RuntimeError,
        match="native privacy_capabilities_v1 returned oversized output",
    ):
        privacy_capabilities_v1()

    with pytest.raises(
        RuntimeError,
        match="native privacy_build_proof_v1 returned oversized output",
    ):
        privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)

    with pytest.raises(
        RuntimeError,
        match="native privacy_verify_proof_v1 returned oversized output",
    ):
        privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)


def test_privacy_native_wrappers_reject_invalid_norito_native_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    bad_magic = bytearray(_PRIVACY_CAPABILITIES_ARCHIVE)
    bad_magic[0] = 0x00
    bad_version = bytearray(_PRIVACY_BUILD_ARCHIVE)
    bad_version[4] = 1
    bad_minor_version = bytearray(_PRIVACY_BUILD_ARCHIVE)
    bad_minor_version[5] = 1
    bad_compression = bytearray(_PRIVACY_BUILD_ARCHIVE)
    bad_compression[22] = 1
    bad_declared_payload_length = _privacy_norito_frame_with_declared_payload_length(0x42, 6)
    bad_oversized_declared_payload_length = _privacy_norito_frame_with_declared_payload_length(
        0x42,
        0x8000000000000000,
    )
    bad_padding = _PRIVACY_VERIFY_ARCHIVE + b"\x7f"
    bad_excessive_padding = _privacy_norito_frame_with_padding(0x42, 65)
    bad_flags = bytearray(_PRIVACY_BUILD_ARCHIVE)
    bad_flags[39] = 0x08
    bad_field_bitset_flags = bytearray(_PRIVACY_BUILD_ARCHIVE)
    bad_field_bitset_flags[39] = 0x20
    bad_checksum = bytearray(_PRIVACY_VERIFY_ARCHIVE + b"\x00")
    bad_checksum[31] = 0x01
    bad_payload = bytearray(_privacy_norito_frame_with_payload(0x57))
    bad_payload[44] ^= 0x7F

    native = _FakePrivacyNative()
    native.privacy_capabilities_v1 = lambda: bytes(bad_magic)  # type: ignore[method-assign]
    native.privacy_build_proof_v1 = lambda _request: bytes(bad_version)  # type: ignore[method-assign]
    native.privacy_verify_proof_v1 = lambda _request: bad_padding  # type: ignore[method-assign]
    monkeypatch.setattr(crypto_module, "_crypto", native)

    with pytest.raises(
        RuntimeError,
        match="native privacy_capabilities_v1 returned invalid Norito V1 archive",
    ):
        privacy_capabilities_v1()
    with pytest.raises(
        RuntimeError,
        match="native privacy_build_proof_v1 returned invalid Norito V1 archive",
    ):
        privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)
    with pytest.raises(
        RuntimeError,
        match="native privacy_verify_proof_v1 returned invalid Norito V1 archive",
    ):
        privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)

    for operation, replacement in (
        ("privacy_capabilities_v1", lambda: bytes(bad_payload)),
        ("privacy_build_proof_v1", lambda _request: bytes(bad_minor_version)),
        ("privacy_build_proof_v1", lambda _request: bytes(bad_compression)),
        ("privacy_build_proof_v1", lambda _request: bad_declared_payload_length),
        ("privacy_build_proof_v1", lambda _request: bad_oversized_declared_payload_length),
        ("privacy_build_proof_v1", lambda _request: bytes(bad_flags)),
        ("privacy_build_proof_v1", lambda _request: bytes(bad_field_bitset_flags)),
        ("privacy_build_proof_v1", lambda _request: bad_excessive_padding),
        ("privacy_verify_proof_v1", lambda _request: bytes(bad_checksum)),
    ):
        native = _FakePrivacyNative()
        setattr(native, operation, replacement)
        monkeypatch.setattr(crypto_module, "_crypto", native)
        if operation == "privacy_capabilities_v1":
            invoke = privacy_capabilities_v1
        elif operation == "privacy_build_proof_v1":
            invoke = lambda: privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)
        else:
            invoke = lambda: privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)
        with pytest.raises(
            RuntimeError,
            match=f"native {operation} returned invalid Norito V1 archive",
        ):
            invoke()


def test_privacy_native_wrappers_defensively_copy_native_output_archives(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _FakeMutableOutputPrivacyNative()
    monkeypatch.setattr(crypto_module, "_crypto", native)

    capabilities = privacy_capabilities_v1()
    build = privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)
    verify = privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)

    native.capabilities_output[0] = 0x7F
    native.build_output[0] = 0x7F
    native.verify_backing[1] = 0x7F

    assert capabilities == _PRIVACY_CAPABILITIES_ARCHIVE
    assert build == _PRIVACY_BUILD_ARCHIVE
    assert verify == _PRIVACY_VERIFY_ARCHIVE


def test_privacy_native_wrappers_respect_sliced_native_output_views(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _FakeSlicedOutputPrivacyNative()
    monkeypatch.setattr(crypto_module, "_crypto", native)

    capabilities = privacy_capabilities_v1()
    build = privacy_build_proof_v1(_PRIVACY_REQUEST_ARCHIVE)
    verify = privacy_verify_proof_v1(_PRIVACY_REQUEST_ARCHIVE)

    assert capabilities == _PRIVACY_CAPABILITIES_ARCHIVE
    assert build == _PRIVACY_BUILD_ARCHIVE
    assert verify == _PRIVACY_VERIFY_ARCHIVE

    native.capabilities_backing[native.prefix_len] = 0x00
    native.build_backing[native.prefix_len] = 0x00
    native.verify_backing[native.prefix_len] = 0x00

    assert capabilities == _PRIVACY_CAPABILITIES_ARCHIVE
    assert build == _PRIVACY_BUILD_ARCHIVE
    assert verify == _PRIVACY_VERIFY_ARCHIVE


def test_privacy_native_wrappers_sanitize_native_exceptions_before_exposing_request_bytes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    witness = b"python-sdk-private-witness-never-echo-37f2"
    request_archive = _PRIVACY_REQUEST_ARCHIVE
    monkeypatch.setattr(
        crypto_module,
        "_crypto",
        _FakeLeakingExceptionPrivacyNative(witness),
    )

    for operation, invoke in (
        ("privacy_capabilities_v1", privacy_capabilities_v1),
        ("privacy_build_proof_v1", lambda: privacy_build_proof_v1(request_archive)),
        ("privacy_verify_proof_v1", lambda: privacy_verify_proof_v1(request_archive)),
    ):
        with pytest.raises(RuntimeError, match=f"native {operation} failed") as exc_info:
            invoke()
        error = exc_info.value
        assert error.__cause__ is None
        assert error.__context__ is None
        assert witness.decode() not in str(error)
        assert witness.decode() not in repr(error)

    native = crypto_module._crypto
    assert isinstance(native, _FakeLeakingExceptionPrivacyNative)
    for request in native.requests:
        assert isinstance(request, bytearray)
        assert all(value == 0 for value in request)
