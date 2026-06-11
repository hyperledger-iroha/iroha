from __future__ import annotations

import base64
import hashlib
import inspect
import json
from dataclasses import FrozenInstanceError
from pathlib import Path

import pytest

import iroha_python
from iroha_python import kagemusha

RECURSIVE_AGGREGATION_METHOD = (
    "kagemusha_prove_verified_recursive_aggregation_proof_bundle"
    "_with_records_and_pallas_open_envelopes"
)
RECURSIVE_COMPACT_METHOD = (
    "kagemusha_prove_verified_recursive_compact_payment_token"
    "_with_records_and_pallas_open_envelopes"
)
RECURSIVE_COMPACT_VERIFY_METHOD = "kagemusha_verify_recursive_compact_payment_token"
RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD = (
    "kagemusha_recursive_spend_compact_payment_token_from_bundle"
)
RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD = (
    "kagemusha_verify_recursive_spend_compact_payment_token_projection"
)
RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD = (
    "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
)
RECURSIVE_SPEND_METHODS = (
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_transition_profile_init",
    "kagemusha_recursive_spend_transition_profile_append",
    "kagemusha_recursive_spend_lineage_append_boundary",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
    "kagemusha_recursive_spend_verify",
    "kagemusha_recursive_spend_redeem",
)
MALFORMED_PROBE_ARCHIVE = b"\x00"


def _shared_recursive_spend_manifest() -> dict[str, object]:
    return _shared_recursive_spend_fixture("manifest.json")


def _shared_recursive_spend_archives() -> dict[str, object]:
    return _shared_recursive_spend_fixture("archives.json")


def _shared_recursive_spend_fixture(file_name: str) -> dict[str, object]:
    path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "kagemusha_recursive_spend_abi6"
        / file_name
    )
    return json.loads(path.read_text(encoding="utf-8"))


def _shared_recursive_spend_abi7_fixture(file_name: str) -> dict[str, object]:
    path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "kagemusha_recursive_spend_abi7"
        / file_name
    )
    return json.loads(path.read_text(encoding="utf-8"))


def _shared_recursive_spend_archive(name: str) -> bytes:
    archives = _shared_recursive_spend_archives()["archives"]
    assert isinstance(archives, list)
    for entry in archives:
        assert isinstance(entry, dict)
        if entry.get("name") == name:
            encoded = entry.get("bytes_base64")
            assert isinstance(encoded, str)
            return base64.b64decode(encoded)
    raise AssertionError(f"missing shared recursive spend archive: {name}")


def _shared_recursive_spend_abi7_archive(name: str) -> bytes:
    archives = _shared_recursive_spend_abi7_fixture("archives.json")["archives"]
    assert isinstance(archives, list)
    for entry in archives:
        assert isinstance(entry, dict)
        if entry.get("name") == name:
            encoded = entry.get("bytes_base64")
            assert isinstance(encoded, str)
            return base64.b64decode(encoded)
    raise AssertionError(f"missing shared recursive spend ABI-7 archive: {name}")


def _instruction_archive_bytes(instruction: object) -> bytes:
    to_json = getattr(instruction, "to_json")
    encoded = json.loads(to_json())
    assert isinstance(encoded, str)
    archive = base64.b64decode(encoded)
    assert archive.startswith(b"NRT0")
    return archive


def _is_malformed_probe_archive(value: bytes) -> bool:
    return bytes(value) == MALFORMED_PROBE_ARCHIVE


def _kagemusha_norito_frame(schema_byte: int) -> bytes:
    frame = bytearray(40)
    frame[:4] = b"NRT0"
    frame[6:22] = bytes([schema_byte]) * 16
    return bytes(frame)


def _kagemusha_norito_frame_with_payload(schema_byte: int) -> bytes:
    frame = bytearray(_kagemusha_norito_frame(schema_byte) + b"\x00\x00\xa5\x5a\x11")
    frame[23:31] = (3).to_bytes(8, "little")
    frame[31:39] = bytes([0xB9, 0xD3, 0xA8, 0x0C, 0xCD, 0x5D, 0x13, 0x24])
    return bytes(frame)


def _kagemusha_norito_frame_with_header_padding(
    archive: bytes, padding: bytes
) -> bytes:
    return bytes(archive[:40] + padding + archive[40:])


_TEST_CRC64_MASK = 0xFFFF_FFFF_FFFF_FFFF
_TEST_CRC64_REFLECTED_POLY = 0xC96C_5795_D787_0F42


def _build_test_crc64_table() -> tuple[int, ...]:
    table: list[int] = []
    for index in range(256):
        crc = index
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ _TEST_CRC64_REFLECTED_POLY
            else:
                crc >>= 1
        table.append(crc)
    return tuple(table)


_TEST_CRC64_TABLE = _build_test_crc64_table()
_TEST_NORITO_COMPACT_LEN_FLAG = 0x02
_TEST_NORITO_PACKED_STRUCT_FLAG = 0x04
_TEST_NORITO_FIELD_BITSET_FLAG = 0x20
_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = bytes.fromhex(
    "c88489618a012c283ff3bb2ebabc7775"
)
_OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = bytes.fromhex(
    "119f4df38a98ef5848ad0aadb9715779"
)


def _test_crc64(payload: bytes) -> int:
    crc = _TEST_CRC64_MASK
    for byte in payload:
        index = (crc ^ byte) & 0xFF
        crc = _TEST_CRC64_TABLE[index] ^ (crc >> 8)
    return (crc ^ _TEST_CRC64_MASK) & _TEST_CRC64_MASK


def _kagemusha_norito_frame_from_payload(schema_byte: int, payload: bytes) -> bytes:
    frame = bytearray(_kagemusha_norito_frame(schema_byte) + bytes(payload))
    frame[23:31] = len(payload).to_bytes(8, "little")
    frame[31:39] = _test_crc64(payload).to_bytes(8, "little")
    return bytes(frame)


def _kagemusha_norito_frame_from_schema_hash(
    schema_hash: bytes,
    payload: bytes,
    flags: int = 0,
) -> bytes:
    frame = bytearray(40 + len(payload))
    frame[0:4] = b"NRT0"
    frame[6:22] = schema_hash
    frame[23:31] = len(payload).to_bytes(8, "little")
    frame[31:39] = _test_crc64(payload).to_bytes(8, "little")
    frame[39] = flags
    frame[40:] = payload
    return bytes(frame)


def _kagemusha_norito_length(value: int, flags: int = 0) -> bytes:
    if not flags & _TEST_NORITO_COMPACT_LEN_FLAG:
        return value.to_bytes(8, "little")
    remaining = value
    output = bytearray()
    while remaining >= 0x80:
        output.append((remaining & 0x7F) | 0x80)
        remaining >>= 7
    output.append(remaining)
    return bytes(output)


def _kagemusha_overlong_compact_length(value: int) -> bytes:
    if value < 0 or value >= 0x80:
        raise ValueError("test helper only encodes small overlong lengths")
    return bytes([value | 0x80, 0x00])


def _kagemusha_oversized_terminal_compact_length() -> bytes:
    return (b"\x80" * 9) + b"\x02"


def _kagemusha_huge_canonical_compact_length() -> bytes:
    return (b"\x80" * 9) + b"\x01"


def _kagemusha_norito_field(
    payload: bytes,
    flags: int = _TEST_NORITO_COMPACT_LEN_FLAG,
) -> bytes:
    return _kagemusha_norito_length(len(payload), flags) + payload


def _kagemusha_norito_string(
    value: str,
    flags: int = _TEST_NORITO_COMPACT_LEN_FLAG,
) -> bytes:
    payload = value.encode("utf-8")
    return _kagemusha_norito_length(len(payload), flags) + payload


def _kagemusha_norito_byte_vec(value: bytes) -> bytes:
    return len(value).to_bytes(8, "little") + value


def _kagemusha_zk1_tlv(tag: bytes, payload: bytes) -> bytes:
    return tag + len(payload).to_bytes(4, "little") + bytes(payload)


def _kagemusha_lineage_verifier_key(circuit_id: str, seed: int) -> bytes:
    return (
        b"ZK1\x00"
        + _kagemusha_zk1_tlv(b"IPAK", bytes([8, 0, 0, 0]))
        + _kagemusha_zk1_tlv(b"CID1", circuit_id.encode("utf-8"))
        + _kagemusha_zk1_tlv(b"H2VK", bytes([seed]) * 32)
    )


def _kagemusha_verifier_key_commitment(verifier_key: bytes) -> bytes:
    backend = kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND.encode("utf-8")
    digest = hashlib.sha256()
    digest.update(b"iroha:zk:v1:vk")
    digest.update(len(backend).to_bytes(8, "big"))
    digest.update(backend)
    digest.update(len(verifier_key).to_bytes(8, "big"))
    digest.update(verifier_key)
    return digest.digest()


def _kagemusha_lineage_proving_key_archive(
    circuit_id: str,
    verifier_key: bytes,
    seed: int,
) -> bytes:
    return _kagemusha_lineage_proving_key_archive_raw(
        1,
        circuit_id,
        _kagemusha_verifier_key_commitment(verifier_key),
        bytes([seed]) * 64,
    )


def _kagemusha_lineage_proving_key_archive_raw(
    version: int,
    circuit_id: str,
    verifier_key_commitment: bytes,
    proving_key: bytes,
    flags: int = _TEST_NORITO_COMPACT_LEN_FLAG,
    schema_hash: bytes = _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    trailing_payload: bytes = b"",
) -> bytes:
    payload = (
        _kagemusha_norito_field(version.to_bytes(2, "little"), flags)
        + _kagemusha_norito_field(_kagemusha_norito_string(circuit_id, flags), flags)
        + _kagemusha_norito_field(verifier_key_commitment, flags)
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(proving_key), flags)
        + trailing_payload
    )
    return _kagemusha_norito_frame_from_schema_hash(
        schema_hash,
        payload,
        flags,
    )


def _kagemusha_input_archive(schema_byte: int = 0x50) -> bytes:
    return _kagemusha_norito_frame_with_payload(schema_byte)


RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE = _kagemusha_input_archive(0xE1)
RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE = _kagemusha_input_archive(0xE2)


def _kagemusha_test_keypair() -> iroha_python.Ed25519KeyPair:
    return iroha_python.Ed25519KeyPair.from_private_key(bytes([0x42] * 32))


def test_kagemusha_instruction_archive_transaction_helpers_wrap_redeem_archive() -> None:
    archive = _shared_recursive_spend_abi7_archive("redeem_instruction")
    instruction = kagemusha.kagemusha_instruction_archive_instruction(
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        archive,
    )
    canonical_archive = _instruction_archive_bytes(instruction)
    assert canonical_archive.startswith(b"NRT0")
    assert len(canonical_archive) > 0

    keypair = _kagemusha_test_keypair()
    authority = keypair.default_account_id("wonderland")
    envelope = kagemusha.build_kagemusha_instruction_transaction(
        "chain",
        authority,
        keypair.private_key,
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        archive,
        creation_time_ms=1,
        ttl_ms=10_000,
        nonce=1,
        metadata={"kagemusha": "redeem"},
    )
    assert envelope.chain_id == "chain"
    assert envelope.authority == authority
    assert bytes(envelope.signed_transaction)
    assert bytes(envelope.signed_transaction_versioned)
    assert envelope.hash_hex()

    draft = iroha_python.TransactionDraft(
        iroha_python.TransactionConfig(chain_id="chain", authority=authority)
    )
    draft.kagemusha_instruction_archive(
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        archive,
    )
    assert len(draft) == 1


def test_kagemusha_recursive_redeem_transaction_helper_derives_instruction_before_signing() -> None:
    request_archive = _shared_recursive_spend_abi7_archive("redeem_request")
    redeem_instruction_archive = _shared_recursive_spend_abi7_archive("redeem_instruction")
    instruction = kagemusha.kagemusha_recursive_redeem_instruction(request_archive)
    committed_instruction = kagemusha.kagemusha_instruction_archive_instruction(
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        redeem_instruction_archive,
    )
    assert _instruction_archive_bytes(instruction) == _instruction_archive_bytes(
        committed_instruction
    )

    keypair = _kagemusha_test_keypair()
    authority = keypair.default_account_id("wonderland")
    envelope = kagemusha.build_kagemusha_recursive_redeem_transaction(
        "chain",
        authority,
        keypair.private_key,
        request_archive,
        creation_time_ms=2,
        ttl_ms=10_000,
        nonce=2,
        metadata={"kagemusha": "recursive-redeem"},
    )
    assert envelope.chain_id == "chain"
    assert envelope.authority == authority
    assert bytes(envelope.signed_transaction)
    assert envelope.hash_hex()

    draft = iroha_python.TransactionDraft(
        iroha_python.TransactionConfig(chain_id="chain", authority=authority)
    )
    draft.kagemusha_recursive_redeem(request_archive)
    assert len(draft) == 1


def test_kagemusha_instruction_archive_transaction_helpers_reject_adversarial_inputs() -> None:
    archive = _shared_recursive_spend_abi7_archive("redeem_instruction")

    assert (
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES[
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE
        ]
        == "iroha_data_model::isi::offline::RedeemKagemushaRecursive"
    )
    assert (
        iroha_python.KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME
        == "iroha_data_model::isi::offline::KagemushaTransfer"
    )

    with pytest.raises(ValueError, match="instruction_type must be KagemushaTransfer"):
        kagemusha.kagemusha_instruction_archive_instruction("RedeemRecursive", archive)

    with pytest.raises(ValueError, match="instruction_archive must not be empty"):
        kagemusha.kagemusha_instruction_archive_instruction(
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
            b"",
        )

    with pytest.raises(ValueError, match="schema must match RedeemKagemushaRecursive"):
        kagemusha.kagemusha_instruction_archive_instruction(
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
            _shared_recursive_spend_abi7_archive("redeem_request"),
        )

    tampered = bytearray(archive)
    tampered[-1] ^= 0x01
    with pytest.raises(ValueError, match="instruction_archive must be a valid Norito archive"):
        kagemusha.kagemusha_instruction_archive_instruction(
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
            tampered,
        )

    def assert_rejects_instruction_archive(mutated: bytearray) -> None:
        with pytest.raises(
            ValueError,
            match="instruction_archive must be a valid Norito archive",
        ):
            kagemusha.kagemusha_instruction_archive_instruction(
                kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
                mutated,
            )

    compressed = bytearray(archive)
    compressed[22] = 1
    assert_rejects_instruction_archive(compressed)

    unsupported_flags = bytearray(archive)
    unsupported_flags[39] = 0x08
    assert_rejects_instruction_archive(unsupported_flags)

    invalid_field_bitset = bytearray(archive)
    invalid_field_bitset[39] = 0x20
    assert_rejects_instruction_archive(invalid_field_bitset)

    non_zero_padding = bytearray(archive)
    non_zero_padding.insert(40, 0x7F)
    assert_rejects_instruction_archive(non_zero_padding)

    excessive_padding = bytearray(archive)
    excessive_padding[40:40] = b"\x00" * 65
    assert_rejects_instruction_archive(excessive_padding)

    keypair = _kagemusha_test_keypair()
    authority = keypair.default_account_id("wonderland")
    with pytest.raises(ValueError, match="redeem_request_archive must be a valid Norito archive"):
        kagemusha.build_kagemusha_recursive_redeem_transaction(
            "chain",
            authority,
            keypair.private_key,
            b"\x00",
        )

    bad_request_flags = bytearray(_shared_recursive_spend_abi7_archive("redeem_request"))
    bad_request_flags[39] = 0x20
    with pytest.raises(ValueError, match="redeem_request_archive must be a valid Norito archive"):
        kagemusha.build_kagemusha_recursive_redeem_transaction(
            "chain",
            authority,
            keypair.private_key,
            bad_request_flags,
        )


class _Native:
    def __init__(self) -> None:
        self.calls: list[tuple[str, bytes]] = []
        setattr(self, RECURSIVE_AGGREGATION_METHOD, self._recursive_aggregation)

    def _reject_probe(self, context: str, *archives: bytes) -> None:
        if archives and all(_is_malformed_probe_archive(archive) for archive in archives):
            raise ValueError(f"invalid Kagemusha {context} probe archive")

    def kagemusha_recursive_spend_native_bridge_abi_version(self) -> int:
        return kagemusha.KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION + 1

    def kagemusha_prove_verified_compact_payment_token_with_records(
        self,
        record_bundle: bytes,
    ) -> bytes:
        self._reject_probe("compact", record_bundle)
        self.calls.append(("compact", record_bundle))
        return _kagemusha_norito_frame_with_payload(0x31)

    def _recursive_aggregation(
        self,
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
    ) -> bytes:
        self._reject_probe("recursive aggregation", record_bundle, pallas_open_envelopes)
        self.calls.append(
            ("recursive_aggregation", record_bundle + b"|" + pallas_open_envelopes)
        )
        return _kagemusha_norito_frame_with_payload(0x32)

    def kagemusha_recursive_spend_init(self, request: bytes) -> bytes:
        self._reject_probe("init", request)
        self.calls.append(("init", request))
        return _kagemusha_norito_frame_with_payload(0x33)

    def kagemusha_recursive_spend_append(self, request: bytes) -> bytes:
        self._reject_probe("append", request)
        self.calls.append(("append", request))
        return _kagemusha_norito_frame_with_payload(0x34)

    def kagemusha_recursive_spend_transition_profile_init(self, request: bytes) -> bytes:
        self._reject_probe("transition profile init", request)
        self.calls.append(("transition-profile-init", request))
        return _kagemusha_norito_frame_with_payload(0x35)

    def kagemusha_recursive_spend_transition_profile_append(self, request: bytes) -> bytes:
        self._reject_probe("transition profile append", request)
        self.calls.append(("transition-profile-append", request))
        return _kagemusha_norito_frame_with_payload(0x36)

    def kagemusha_recursive_spend_lineage_append_boundary(self, profile: bytes) -> bytes:
        self._reject_probe("lineage append boundary", profile)
        self.calls.append(("lineage-append-boundary", profile))
        return _kagemusha_norito_frame_with_payload(0x37)

    def kagemusha_recursive_spend_lineage_witness_from_init_result(
        self,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        self._reject_probe("lineage init", request, bundle)
        self.calls.append(("lineage-init", request + b"|" + bundle))
        return _kagemusha_norito_frame_with_payload(0x38)

    def kagemusha_recursive_spend_lineage_witness_append_result(
        self,
        previous_witness: bytes,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        self._reject_probe("lineage append", previous_witness, request, bundle)
        self.calls.append(("lineage-append", previous_witness + b"|" + request + b"|" + bundle))
        return _kagemusha_norito_frame_with_payload(0x39)

    def kagemusha_recursive_spend_verify(self, request: bytes) -> bytes:
        self._reject_probe("verify", request)
        self.calls.append(("verify", request))
        return _kagemusha_norito_frame_with_payload(0x3A)

    def kagemusha_recursive_spend_redeem(self, request: bytes) -> bytes:
        self._reject_probe("redeem", request)
        self.calls.append(("redeem", request))
        return _kagemusha_norito_frame_with_payload(0x3B)


def test_recursive_kagemusha_helpers_reject_empty_requests(monkeypatch: pytest.MonkeyPatch) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(ValueError, match="request_archive must not be empty"):
            helper(b"")
    with pytest.raises(ValueError, match="profile_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(b"")
    with pytest.raises(ValueError, match="request_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            b"",
            _kagemusha_input_archive(0x51),
        )
    with pytest.raises(ValueError, match="bundle_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x52),
            b"",
        )
    with pytest.raises(ValueError, match="previous_witness_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            b"",
            _kagemusha_input_archive(0x53),
            _kagemusha_input_archive(0x54),
        )
    with pytest.raises(ValueError, match="request_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x55),
            b"",
            _kagemusha_input_archive(0x56),
        )
    with pytest.raises(ValueError, match="bundle_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x57),
            _kagemusha_input_archive(0x58),
            b"",
        )

    assert native.calls == []


def test_recursive_kagemusha_helpers_reject_malformed_norito_requests() -> None:
    with pytest.raises(ValueError, match="request_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_init(b"\x01")
    with pytest.raises(ValueError, match="profile_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(b"\x01")
    with pytest.raises(ValueError, match="bundle_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x59),
            b"\x01",
        )
    with pytest.raises(ValueError, match="request_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x5A),
            b"\x01",
            _kagemusha_input_archive(0x5B),
        )


def test_recursive_kagemusha_helpers_reject_empty_payload_norito_requests() -> None:
    with pytest.raises(
        ValueError,
        match="request_archive must contain a non-empty Norito payload",
    ):
        kagemusha.kagemusha_recursive_spend_verify(_kagemusha_norito_frame(0x5C))
    with pytest.raises(
        ValueError,
        match="previous_witness_archive must contain a non-empty Norito payload",
    ):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_norito_frame(0x5D),
            _kagemusha_input_archive(0x5E),
            _kagemusha_input_archive(0x5F),
        )


def test_kagemusha_native_prover_helpers_reject_empty_requests(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"")

    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            b"",
            b"pallas",
        )

    with pytest.raises(ValueError, match="pallas_open_envelopes_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xA1),
            b"",
        )
    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            b"",
            b"pallas",
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(ValueError, match="pallas_open_envelopes_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xA2),
            b"",
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_key_artifacts_archive must not be empty",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xA3),
            _kagemusha_input_archive(0xA4),
            b"",
        )
    with pytest.raises(ValueError, match="compact_token_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            b"",
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_verifier_keys_archive must not be empty",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_input_archive(0x4B),
            b"",
        )
    with pytest.raises(ValueError, match="compact_token_archive must be a valid Norito archive"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            b"\x01",
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="compact_token_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )

    assert native.calls == []


def test_kagemusha_native_prover_helpers_reject_malformed_norito_requests() -> None:
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must be a valid Norito archive",
    ):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"\x01")
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            b"\x01",
            _kagemusha_input_archive(0xB1),
        )
    with pytest.raises(
        ValueError,
        match="pallas_open_envelopes_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xB2),
            b"\x01",
        )
    with pytest.raises(
        ValueError,
        match="pallas_open_envelopes_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB3),
            b"\x01",
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_key_artifacts_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB3),
            _kagemusha_input_archive(0xB4),
            b"\x01",
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_verifier_keys_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_input_archive(0x4B),
            b"\x01",
        )


def test_kagemusha_native_prover_helpers_reject_empty_payload_norito_requests() -> None:
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must contain a non-empty Norito payload",
    ):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_norito_frame(0xB4)
        )
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_norito_frame(0xB5),
            _kagemusha_input_archive(0xB6),
        )
    with pytest.raises(
        ValueError,
        match="pallas_open_envelopes_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB7),
            _kagemusha_norito_frame(0xB8),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_key_artifacts_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB7),
            _kagemusha_input_archive(0xB8),
            _kagemusha_norito_frame(0xB9),
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_verifier_keys_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_input_archive(0x4B),
            _kagemusha_norito_frame(0x4C),
        )


def test_recursive_compact_unavailable_classifier_matches_reserved_fragments() -> None:
    payment_token_message = (
        kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT
    )
    multi_hop_message = (
        kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT
    )

    assert kagemusha.is_kagemusha_recursive_compact_unavailable(
        RuntimeError(payment_token_message)
    )
    assert kagemusha.is_kagemusha_recursive_compact_unavailable(
        f"bridge: {multi_hop_message}"
    )
    assert not kagemusha.is_kagemusha_recursive_compact_unavailable(
        RuntimeError("recursive compact proof composition unavailable")
    )
    assert not kagemusha.is_kagemusha_recursive_compact_unavailable(None)
    assert (
        iroha_python.is_kagemusha_recursive_compact_unavailable(multi_hop_message)
        is True
    )


def test_recursive_kagemusha_helpers_probe_and_delegate(monkeypatch: pytest.MonkeyPatch) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    record_bundle = _kagemusha_input_archive(0xB9)
    pallas_open_envelopes = _kagemusha_input_archive(0xBA)
    init_request = _kagemusha_input_archive(0x61)
    append_request = _kagemusha_input_archive(0x62)
    transition_init_request = _kagemusha_input_archive(0x63)
    transition_append_request = _kagemusha_input_archive(0x64)
    boundary_profile = _kagemusha_input_archive(0x65)
    lineage_init_request = _kagemusha_input_archive(0x66)
    lineage_init_bundle = _kagemusha_input_archive(0x67)
    lineage_append_previous_witness = _kagemusha_input_archive(0x68)
    lineage_append_request = _kagemusha_input_archive(0x69)
    lineage_append_bundle = _kagemusha_input_archive(0x6A)
    verify_request = _kagemusha_input_archive(0x6B)
    redeem_request = _kagemusha_input_archive(0x6C)

    assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode_for_capabilities(True, True)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode_for_capabilities(False, True)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode(True)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode(False)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            record_bundle
        )
        == _kagemusha_norito_frame_with_payload(0x31)
    )
    recursive_aggregation = getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)
    assert (
        recursive_aggregation(record_bundle, pallas_open_envelopes)
        == _kagemusha_norito_frame_with_payload(0x32)
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token prover"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token verifier"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )

    def permissive_recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native.calls.append(
            (
                "permissive_recursive_compact",
                record_bundle + b"|" + pallas_open_envelopes + b"|" + key_artifacts,
            )
        )
        return b"permissive_recursive_compact"

    setattr(native, RECURSIVE_COMPACT_METHOD, permissive_recursive_compact)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token prover"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    assert native.calls[-1] == (
        "permissive_recursive_compact",
        MALFORMED_PROBE_ARCHIVE
        + b"|"
        + MALFORMED_PROBE_ARCHIVE
        + b"|"
        + MALFORMED_PROBE_ARCHIVE,
    )

    def recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native._reject_probe("recursive compact", record_bundle, pallas_open_envelopes, key_artifacts)
        native.calls.append(
            ("recursive_compact", record_bundle + b"|" + pallas_open_envelopes + b"|" + key_artifacts)
        )
        return _kagemusha_norito_frame_with_payload(0x4D)

    setattr(native, RECURSIVE_COMPACT_METHOD, recursive_compact)
    setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, lambda compact_token, verifier_keys: True)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token verifier"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )

    def recursive_compact_verify(compact_token: bytes, verifier_keys: bytes) -> bool:
        native._reject_probe("recursive compact verify", compact_token, verifier_keys)
        native.calls.append(("recursive_compact_verify", compact_token + b"|" + verifier_keys))
        return compact_token[6] == 0x4B

    setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, recursive_compact_verify)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is True
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )

    def unavailable_recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native._reject_probe("recursive compact", record_bundle, pallas_open_envelopes, key_artifacts)
        raise RuntimeError("recursive compact proof composition unavailable")

    setattr(native, RECURSIVE_COMPACT_METHOD, unavailable_recursive_compact)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is True
    with pytest.raises(RuntimeError, match="proof composition unavailable"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )

    setattr(native, RECURSIVE_COMPACT_METHOD, recursive_compact)
    assert (
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
        == _kagemusha_norito_frame_with_payload(0x4D)
    )
    valid_recursive_compact_token = _kagemusha_norito_frame_with_payload(0x4B)
    forged_recursive_compact_token = _kagemusha_norito_frame_with_payload(0x4C)
    assert (
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            valid_recursive_compact_token,
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
        is True
    )
    assert (
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            forged_recursive_compact_token,
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
        is False
    )
    assert (
        kagemusha.kagemusha_recursive_spend_init(init_request)
        == _kagemusha_norito_frame_with_payload(0x33)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_append(bytearray(append_request))
        == _kagemusha_norito_frame_with_payload(0x34)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_transition_profile_init(transition_init_request)
        == _kagemusha_norito_frame_with_payload(0x35)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_transition_profile_append(
            transition_append_request
        )
        == _kagemusha_norito_frame_with_payload(0x36)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(boundary_profile)
        == _kagemusha_norito_frame_with_payload(0x37)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            lineage_init_request,
            lineage_init_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x38)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            lineage_append_previous_witness,
            lineage_append_request,
            lineage_append_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x39)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_verify(memoryview(verify_request))
        == _kagemusha_norito_frame_with_payload(0x3A)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_redeem(redeem_request)
        == _kagemusha_norito_frame_with_payload(0x3B)
    )
    assert native.calls == [
        ("compact", record_bundle),
        ("recursive_aggregation", record_bundle + b"|" + pallas_open_envelopes),
        ("permissive_recursive_compact", b"\x00|\x00|\x00"),
        ("permissive_recursive_compact", b"\x00|\x00|\x00"),
        ("permissive_recursive_compact", b"\x00|\x00|\x00"),
        (
            "recursive_compact",
            record_bundle
            + b"|"
            + pallas_open_envelopes
            + b"|"
            + RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
        (
            "recursive_compact_verify",
            valid_recursive_compact_token
            + b"|"
            + RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        (
            "recursive_compact_verify",
            forged_recursive_compact_token
            + b"|"
            + RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        ("init", init_request),
        ("append", append_request),
        ("transition-profile-init", transition_init_request),
        ("transition-profile-append", transition_append_request),
        ("lineage-append-boundary", boundary_profile),
        ("lineage-init", lineage_init_request + b"|" + lineage_init_bundle),
        (
            "lineage-append",
            lineage_append_previous_witness
            + b"|"
            + lineage_append_request
            + b"|"
            + lineage_append_bundle,
        ),
        ("verify", verify_request),
        ("redeem", redeem_request),
    ]


def test_recursive_spend_compact_projection_probes_and_delegates(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    bundle_archive = _kagemusha_input_archive(0xE1)
    projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD)

    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is False
    )
    with pytest.raises(
        RuntimeError,
        match=(
            "recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7"
            ".*compact projection symbol"
        ),
    ):
        projection(bundle_archive)

    def project_bundle(bundle: bytes) -> bytes:
        native._reject_probe("recursive spend compact projection", bundle)
        native.calls.append(("recursive_spend_compact_projection", bundle))
        return _kagemusha_norito_frame_with_payload(0x4F)

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD, project_bundle)
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is True
    )
    assert projection(bundle_archive) == _kagemusha_norito_frame_with_payload(0x4F)
    assert native.calls[-1] == ("recursive_spend_compact_projection", bundle_archive)

    with pytest.raises(ValueError, match="bundle_archive must not be empty"):
        projection(b"")
    with pytest.raises(ValueError, match="bundle_archive must be a valid Norito archive"):
        projection(b"\x01")
    with pytest.raises(ValueError, match="bundle_archive must contain a non-empty Norito payload"):
        projection(_kagemusha_norito_frame(0x4C))

    def invalid_projection(bundle: bytes) -> bytes:
        native._reject_probe("recursive spend compact projection", bundle)
        return b"\x01"

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD, invalid_projection)
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is True
    )
    with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
        projection(bundle_archive)


def test_recursive_spend_compact_projection_verifier_probes_and_delegates(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    compact_token = _kagemusha_input_archive(0xE2)
    verifier_record = _kagemusha_input_archive(0xE3)
    verify_projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD)
    verify_projection_at_height = getattr(
        kagemusha,
        RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
    )

    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
        is False
    )
    with pytest.raises(
        RuntimeError,
        match=(
            "recursive spend compact Kagemusha payment-token projection verifier "
            "requires native bridge ABI 7.*compact projection verifier symbols"
        ),
    ):
        verify_projection(compact_token, verifier_record)

    def verify_without_height(token: bytes, record: bytes) -> bool:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        native.calls.append(("recursive_spend_compact_projection_verify", token + b"|" + record))
        return False

    def verify_at_height(token: bytes, record: bytes, block_height: int) -> bool:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        native.calls.append(
            (
                "recursive_spend_compact_projection_verify_at_height",
                token + b"|" + record + b"|" + str(block_height).encode("ascii"),
            )
        )
        return True

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD, verify_without_height)
    setattr(
        native,
        RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
        verify_at_height,
    )
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
        is True
    )
    assert verify_projection(compact_token, verifier_record) is False
    assert native.calls[-1] == (
        "recursive_spend_compact_projection_verify",
        compact_token + b"|" + verifier_record,
    )
    assert verify_projection(compact_token, verifier_record, block_height=2) is True
    assert native.calls[-1] == (
        "recursive_spend_compact_projection_verify_at_height",
        compact_token + b"|" + verifier_record + b"|2",
    )
    assert verify_projection_at_height(compact_token, verifier_record, 3) is True
    assert native.calls[-1] == (
        "recursive_spend_compact_projection_verify_at_height",
        compact_token + b"|" + verifier_record + b"|3",
    )

    with pytest.raises(ValueError, match="compact_token_archive must not be empty"):
        verify_projection(b"", verifier_record)
    with pytest.raises(ValueError, match="verifier_record_archive must be a valid Norito archive"):
        verify_projection(compact_token, b"\x01")
    with pytest.raises(ValueError, match="block_height must be non-negative"):
        verify_projection(compact_token, verifier_record, block_height=-1)
    with pytest.raises(ValueError, match="block_height must be non-negative"):
        verify_projection_at_height(compact_token, verifier_record, -1)
    for bad_height in (True, False, 1.5, "1"):
        with pytest.raises(TypeError, match="block_height must be an integer"):
            verify_projection(
                compact_token,
                verifier_record,
                block_height=bad_height,  # type: ignore[arg-type]
            )
        with pytest.raises(TypeError, match="block_height must be an integer"):
            verify_projection_at_height(
                compact_token,
                verifier_record,
                bad_height,  # type: ignore[arg-type]
            )
    with pytest.raises(ValueError, match="block_height must fit in u64"):
        verify_projection(compact_token, verifier_record, block_height=1 << 64)
    with pytest.raises(ValueError, match="block_height must fit in u64"):
        verify_projection_at_height(compact_token, verifier_record, 1 << 64)

    def invalid_boolean(token: bytes, record: bytes) -> str:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        return "false"

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD, invalid_boolean)
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
        is True
    )
    with pytest.raises(RuntimeError, match="returned non-boolean result"):
        verify_projection(compact_token, verifier_record)


def test_recursive_kagemusha_lineage_helpers_copy_mutable_archives_before_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[tuple[str, bytes, ...]] = []

    def lineage_init(request: bytes, bundle: bytes) -> bytes:
        native._reject_probe("lineage init", request, bundle)
        calls.append(("lineage-init", request, bundle))
        return _kagemusha_norito_frame_with_payload(0x58)

    def lineage_append(
        previous_witness: bytes,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        native._reject_probe("lineage append", previous_witness, request, bundle)
        calls.append(("lineage-append", previous_witness, request, bundle))
        return _kagemusha_norito_frame_with_payload(0x59)

    native.kagemusha_recursive_spend_lineage_witness_from_init_result = lineage_init
    native.kagemusha_recursive_spend_lineage_witness_append_result = lineage_append
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    init_request = bytearray(_kagemusha_input_archive(0xA1))
    init_bundle = bytearray(_kagemusha_input_archive(0xA2))
    previous_witness_storage = bytearray(_kagemusha_input_archive(0xA3))
    append_request = bytearray(_kagemusha_input_archive(0xA4))
    append_bundle = bytearray(_kagemusha_input_archive(0xA5))
    expected_init_request = bytes(init_request)
    expected_init_bundle = bytes(init_bundle)
    expected_previous_witness = bytes(previous_witness_storage)
    expected_append_request = bytes(append_request)
    expected_append_bundle = bytes(append_bundle)

    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            init_request,
            init_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x58)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            memoryview(previous_witness_storage),
            append_request,
            append_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x59)
    )

    init_request[6] = 0x7F
    init_bundle[6] = 0x7F
    previous_witness_storage[6] = 0x7F
    append_request[6] = 0x7F
    append_bundle[6] = 0x7F

    assert calls == [
        ("lineage-init", expected_init_request, expected_init_bundle),
        (
            "lineage-append",
            expected_previous_witness,
            expected_append_request,
            expected_append_bundle,
        ),
    ]


def test_recursive_compact_payment_token_verifier_rejects_non_boolean_native_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native._reject_probe("recursive compact", record_bundle, pallas_open_envelopes, key_artifacts)
        return b"recursive_compact"

    def non_boolean_verify(compact_token: bytes, verifier_keys: bytes) -> bytes:
        native._reject_probe("recursive compact verify", compact_token, verifier_keys)
        return b"not-a-boolean"

    setattr(native, RECURSIVE_COMPACT_METHOD, recursive_compact)
    setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, non_boolean_verify)
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is True
    with pytest.raises(RuntimeError, match="returned non-boolean result"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )


def test_recursive_kagemusha_shared_abi6_fixture_matches_sdk_surface() -> None:
    manifest = _shared_recursive_spend_manifest()
    assert manifest["schema"] == "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1"
    assert (
        kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1
        == "recursive_compact_v1"
    )
    assert kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 7
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1
        == "kagemusha-recursive-compact-v1"
    )
    assert (
        manifest["native_bridge_abi_version"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
    )
    assert manifest["operation_count"] == 9

    operations = manifest["operations"]
    assert isinstance(operations, list)
    assert len(operations) == manifest["operation_count"]
    assert {operation["symbol"] for operation in operations} == {
        "connect_norito_kagemusha_recursive_spend_init",
        "connect_norito_kagemusha_recursive_spend_append",
        "connect_norito_kagemusha_recursive_spend_transition_profile_init",
        "connect_norito_kagemusha_recursive_spend_transition_profile_append",
        "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
        "connect_norito_kagemusha_recursive_spend_verify",
        "connect_norito_kagemusha_recursive_spend_redeem",
    }
    append_witness = next(
        operation
        for operation in operations
        if operation["name"] == "lineage_witness_append_result"
    )
    assert append_witness["input_archives"] == [
        "KagemushaRecursiveSpendLineageWitnessV1",
        "KagemushaRecursiveSpendAppendRequestV1",
        "KagemushaRecursiveSpendBundleV1",
    ]
    assert append_witness["output_archive"] == "KagemushaRecursiveSpendLineageWitnessV1"

    circuit_ids = manifest["proof_circuit_ids"]
    assert isinstance(circuit_ids, dict)
    assert (
        circuit_ids["recursive_aggregation"]
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        circuit_ids["reserved_lineage"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
    )
    assert (
        circuit_ids["reserved_lineage_one_hop"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
    )
    assert (
        circuit_ids["reserved_lineage_append"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )

    limits = manifest["limits"]
    assert isinstance(limits, dict)
    assert limits["compact_token_max_hops"] == kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS
    assert (
        limits["reserved_lineage_witnessless_max_hops"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
    )
    assert (
        limits["previous_proof_open_envelopes_required_count"]
        == kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1
    )
    assert (
        limits["previous_proof_open_envelopes_max_bytes"]
        == kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
    )
    assert (
        limits["pallas_open_envelope_max_transcript_label_bytes"]
        == kagemusha.KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
    )
    assert limits["native_archive_max_bytes"] == kagemusha.KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES

    domains = manifest["domains"]
    assert isinstance(domains, dict)
    assert (
        domains["transition_profile"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN
    )
    assert (
        domains["lineage_append_boundary_final_note_binding"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1
    )

    benchmarks = manifest["payload_benchmarks"]
    assert isinstance(benchmarks, dict)
    assert benchmarks["semantic_payload_bytes"] == 1751
    assert benchmarks["reserved_lineage_payload_bytes"] == 3847
    assert benchmarks["reserved_lineage_transition_profile_bytes"] == 2817

    archive_fixture = _shared_recursive_spend_archives()
    assert (
        archive_fixture["schema"]
        == "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1"
    )
    archives = archive_fixture["archives"]
    assert isinstance(archives, list)
    assert {archive["name"] for archive in archives} == {
        "init_request",
        "init_bundle",
        "transition_profile_init",
        "append_request",
        "append_bundle",
        "transition_profile_append",
        "lineage_append_boundary",
        "lineage_witness_from_init_result",
        "lineage_witness_append_result",
        "verify_request",
        "verify_result",
        "redeem_request",
        "redeem_instruction",
    }
    request_archive_fields = archive_fixture["request_archive_fields"]
    assert isinstance(request_archive_fields, list)
    request_fields_by_type = {
        entry["norito_type"]: entry["fields"] for entry in request_archive_fields
    }
    expected_request_fields = {
        "KagemushaRecursiveSpendInitRequestV1": [
            "record_bundle",
            "pallas_open_envelopes_archive",
            "current_note",
            "lineage_verifier_key",
            "lineage_proving_key_archive",
            "block_height",
        ],
        "KagemushaRecursiveSpendAppendRequestV1": [
            "previous_bundle",
            "record_bundle",
            "pallas_open_envelopes_archive",
            "current_note",
            "output_proof_circuit_id",
            "previous_lineage_verifier_record",
            "previous_recursive_proof_open_envelopes_archive",
            "lineage_verifier_key",
            "lineage_proving_key_archive",
            "block_height",
        ],
        "KagemushaRecursiveSpendVerifyRequestV1": [
            "bundle",
            "lineage_verifier_record",
            "block_height",
        ],
        "KagemushaRecursiveSpendRedeemRequestV1": [
            "bundle",
            "recipient",
            "public_amount",
            "redeem_proof",
            "lineage_witness",
            "lineage_verifier_record",
            "block_height",
        ],
    }
    assert set(request_fields_by_type) == set(expected_request_fields)
    for request_type, expected_fields in expected_request_fields.items():
        fields = request_fields_by_type[request_type]
        assert [field["name"] for field in fields] == expected_fields
        block_height = next(field for field in fields if field["name"] == "block_height")
        assert block_height["type"] == "Option<u64>"
        assert block_height["norito_default"] is True
        assert block_height["semantics"] == "verifier_record_activation_height"

    redeem_archive = next(
        archive for archive in archives if archive["name"] == "redeem_request"
    )
    assert redeem_archive["operation"] == "redeem"
    assert redeem_archive["norito_type"] == "KagemushaRecursiveSpendRedeemRequestV1"
    assert (
        redeem_archive["sha256_hex"]
        == "f5a4a6a25fd9bfd8a121893ddb0c977753c16d8b9dfd835477d2965957c7c03e"
    )
    assert redeem_archive["byte_len"] > 0
    assert len(base64.b64decode(redeem_archive["bytes_base64"])) > 0
    redeem_instruction_archive = next(
        archive for archive in archives if archive["name"] == "redeem_instruction"
    )
    assert redeem_instruction_archive["norito_type"] == "RedeemKagemushaRecursive"
    assert (
        redeem_instruction_archive["sha256_hex"]
        == "88f293dccb455b6fbcd85d7c06426ce45f02a42fc330e68afda490d504903c03"
    )

    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(1)
        == circuit_ids["reserved_lineage_append"]
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(63)
        == circuit_ids["reserved_lineage_append"]
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(64)
        == circuit_ids["recursive_aggregation"]
    )
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(0)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(63)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(64)
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        2,
    )
    assert not kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        65,
    )


def test_recursive_kagemusha_availability_rejects_permissive_native_probes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for method_name in (
        "kagemusha_prove_verified_compact_payment_token_with_records",
        RECURSIVE_AGGREGATION_METHOD,
        RECURSIVE_COMPACT_METHOD,
        RECURSIVE_COMPACT_VERIFY_METHOD,
        *RECURSIVE_SPEND_METHODS,
    ):
        native = _Native()
        if method_name == RECURSIVE_AGGREGATION_METHOD:
            setattr(native, method_name, lambda record, pallas: b"accepted")
        elif method_name == RECURSIVE_COMPACT_METHOD:
            setattr(native, method_name, lambda record, pallas, key_artifacts: b"accepted")

            def rejecting_verify(archive: bytes, verifier_keys: bytes) -> bool:
                native._reject_probe("recursive compact verify", archive, verifier_keys)
                return False

            setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, rejecting_verify)
        elif method_name == RECURSIVE_COMPACT_VERIFY_METHOD:
            def rejecting_recursive_compact(
                record: bytes,
                pallas: bytes,
                key_artifacts: bytes,
            ) -> bytes:
                native._reject_probe("recursive compact", record, pallas, key_artifacts)
                return b"accepted"

            setattr(native, RECURSIVE_COMPACT_METHOD, rejecting_recursive_compact)
            setattr(native, method_name, lambda archive, verifier_keys: True)
        elif method_name == "kagemusha_recursive_spend_lineage_witness_from_init_result":
            setattr(native, method_name, lambda request, bundle: b"accepted")
        elif method_name == "kagemusha_recursive_spend_lineage_witness_append_result":
            setattr(native, method_name, lambda witness, request, bundle: b"accepted")
        else:
            setattr(native, method_name, lambda archive: b"accepted")
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda native=native: native)

        if method_name == "kagemusha_prove_verified_compact_payment_token_with_records":
            assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is False
        elif method_name == RECURSIVE_AGGREGATION_METHOD:
            assert (
                kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available()
                is False
            )
        elif method_name in (RECURSIVE_COMPACT_METHOD, RECURSIVE_COMPACT_VERIFY_METHOD):
            assert (
                kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
                is False
            )
            if method_name == RECURSIVE_COMPACT_METHOD:
                assert (
                    kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
                    is True
                )
                with pytest.raises(
                    RuntimeError,
                    match="recursive compact Kagemusha payment-token prover",
                ):
                    getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
                        _kagemusha_input_archive(0xBB),
                        _kagemusha_input_archive(0xBC),
                        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
                    )
            else:
                assert (
                    kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
                    is False
                )
                with pytest.raises(
                    RuntimeError,
                    match="recursive compact Kagemusha payment-token verifier",
                ):
                    getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
                        _kagemusha_norito_frame_with_payload(0x4B),
                        RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
                    )
        else:
            assert kagemusha.is_kagemusha_recursive_spend_available() is False
            with pytest.raises(RuntimeError, match="reject malformed probe archives"):
                kagemusha.kagemusha_recursive_spend_verify(_kagemusha_input_archive(0x74))

    vague_prover_native = _Native()

    def vague_recursive_compact_prover(record: bytes, pallas: bytes, key_artifacts: bytes) -> bytes:
        raise RuntimeError("Kagemusha recursive compact proof unavailable")

    def rejecting_recursive_compact_verify(archive: bytes, verifier_keys: bytes) -> bool:
        vague_prover_native._reject_probe("recursive compact verify", archive, verifier_keys)
        return True

    setattr(
        vague_prover_native,
        RECURSIVE_COMPACT_METHOD,
        vague_recursive_compact_prover,
    )
    setattr(
        vague_prover_native,
        RECURSIVE_COMPACT_VERIFY_METHOD,
        rejecting_recursive_compact_verify,
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: vague_prover_native)
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
        is False
    )
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
        is True
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token prover"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xBD),
            _kagemusha_input_archive(0xBE),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )

    vague_verifier_native = _Native()

    def rejecting_recursive_compact_prover(record: bytes, pallas: bytes, key_artifacts: bytes) -> bytes:
        vague_verifier_native._reject_probe("recursive compact", record, pallas, key_artifacts)
        return _kagemusha_input_archive(0xBF)

    def vague_recursive_compact_verify(archive: bytes, verifier_keys: bytes) -> bool:
        raise RuntimeError("Kagemusha recursive compact verifier unavailable")

    setattr(
        vague_verifier_native,
        RECURSIVE_COMPACT_METHOD,
        rejecting_recursive_compact_prover,
    )
    setattr(
        vague_verifier_native,
        RECURSIVE_COMPACT_VERIFY_METHOD,
        vague_recursive_compact_verify,
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: vague_verifier_native)
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
        is False
    )
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
        is False
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token verifier"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )


def test_recursive_kagemusha_key_artifact_helpers_are_package_root_exports() -> None:
    import iroha_python

    if "is_kagemusha_recursive_spend_available" not in iroha_python.__all__:
        pytest.skip("package root crypto exports are unavailable")

    from iroha_python import (
        KagemushaRecursiveSpendLineageKeyArtifacts
        as RootLineageKeyArtifacts,
        kagemusha_recursive_spend_lineage_key_artifacts_for_append
        as root_lineage_key_artifacts_for_append,
        kagemusha_recursive_spend_lineage_key_artifacts_for_init
        as root_lineage_key_artifacts_for_init,
        kagemusha_recursive_spend_compact_payment_token_from_bundle
        as root_recursive_spend_compact_projection,
        kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes
        as root_recursive_compact_prover,
        kagemusha_verify_recursive_compact_payment_token
        as root_recursive_compact_verify,
        kagemusha_verify_recursive_spend_compact_payment_token_projection
        as root_recursive_spend_compact_projection_verify,
        kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height
        as root_recursive_spend_compact_projection_verify_at_height,
        is_kagemusha_recursive_compact_payment_token_prover_available
        as root_is_recursive_compact_prover_available,
        is_kagemusha_recursive_compact_payment_token_verifier_available
        as root_is_recursive_compact_verifier_available,
        is_kagemusha_recursive_spend_compact_payment_token_projection_available
        as root_is_recursive_spend_compact_projection_available,
        is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available
        as root_is_recursive_spend_compact_projection_verifier_available,
        requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output
        as root_requires_key_artifacts_for_append_output,
        requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init
        as root_requires_key_artifacts_for_init,
    )

    assert (
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init"
        in iroha_python.__all__
    )
    assert "KagemushaRecursiveSpendLineageKeyArtifacts" in iroha_python.__all__
    assert (
        "kagemusha_recursive_spend_lineage_key_artifacts_for_init"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_recursive_spend_lineage_key_artifacts_for_append"
        in iroha_python.__all__
    )
    assert (
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output"
        in iroha_python.__all__
    )
    assert (
        "is_kagemusha_recursive_compact_payment_token_verifier_available"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes"
        in iroha_python.__all__
    )
    assert "kagemusha_verify_recursive_compact_payment_token" in iroha_python.__all__
    assert (
        "is_kagemusha_recursive_spend_compact_payment_token_projection_available"
        in iroha_python.__all__
    )
    assert (
        "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_recursive_spend_compact_payment_token_from_bundle"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_verify_recursive_spend_compact_payment_token_projection"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
        in iroha_python.__all__
    )
    assert (
        root_requires_key_artifacts_for_init
        is kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init
    )
    assert (
        root_requires_key_artifacts_for_append_output
        is kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output
    )
    assert (
        RootLineageKeyArtifacts
        is kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts
    )
    assert (
        root_lineage_key_artifacts_for_init
        is kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init
    )
    assert (
        root_lineage_key_artifacts_for_append
        is kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_append
    )
    assert (
        root_is_recursive_compact_prover_available
        is kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available
    )
    assert (
        root_is_recursive_compact_verifier_available
        is kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available
    )
    assert (
        root_recursive_compact_prover
        is getattr(kagemusha, RECURSIVE_COMPACT_METHOD)
    )
    assert (
        root_recursive_compact_verify
        is getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)
    )
    assert (
        root_is_recursive_spend_compact_projection_available
        is kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available
    )
    assert (
        root_is_recursive_spend_compact_projection_verifier_available
        is kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available
    )
    assert (
        root_recursive_spend_compact_projection
        is kagemusha.kagemusha_recursive_spend_compact_payment_token_from_bundle
    )
    assert (
        root_recursive_spend_compact_projection_verify
        is kagemusha.kagemusha_verify_recursive_spend_compact_payment_token_projection
    )
    assert (
        root_recursive_spend_compact_projection_verify_at_height
        is kagemusha.kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height
    )
    prover_signature = inspect.signature(root_recursive_compact_prover)
    assert list(prover_signature.parameters) == [
        "record_bundle_archive",
        "pallas_open_envelopes_archive",
        "recursive_compact_key_artifacts_archive",
    ]
    assert all(
        parameter.default is inspect.Parameter.empty
        for parameter in prover_signature.parameters.values()
    )
    verifier_signature = inspect.signature(root_recursive_compact_verify)
    assert list(verifier_signature.parameters) == [
        "compact_token_archive",
        "recursive_compact_verifier_keys_archive",
    ]
    assert all(
        parameter.default is inspect.Parameter.empty
        for parameter in verifier_signature.parameters.values()
    )
    projection_at_height_signature = inspect.signature(
        root_recursive_spend_compact_projection_verify_at_height
    )
    assert list(projection_at_height_signature.parameters) == [
        "compact_token_archive",
        "verifier_record_archive",
        "block_height",
    ]
    assert all(
        parameter.default is inspect.Parameter.empty
        for parameter in projection_at_height_signature.parameters.values()
    )


def test_recursive_kagemusha_lineage_key_artifacts_validate_inputs() -> None:
    assert kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND == "halo2/ipa"
    for opening_len in (2, 4, 8, 16, 32, 64, 128):
        assert (
            kagemusha.is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len(
                opening_len,
            )
            is True
        )
    for opening_len in (0, 1, 3, 65, 129, -2, 2.5, "2", True):
        assert (
            kagemusha.is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len(
                opening_len,  # type: ignore[arg-type]
            )
            is False
        )

    init_verifier_key = _kagemusha_lineage_verifier_key(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        0xA1,
    )
    init_proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        init_verifier_key,
        0xA2,
    )
    append_verifier_key = _kagemusha_lineage_verifier_key(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        0xA3,
    )
    append_proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        append_verifier_key,
        0xA4,
    )

    verifier_key = bytearray(init_verifier_key)
    proving_key = bytearray(init_proving_key_archive)
    init_artifacts = kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
        128,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        verifier_key,
        memoryview(proving_key),
    )
    verifier_key[:] = b"\x00" * len(verifier_key)
    proving_key[:] = b"\x00" * len(proving_key)
    assert init_artifacts.proof_circuit_id == (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
    )
    assert init_artifacts.verifier_opening_len == 128
    assert init_artifacts.lineage_verifier_key_backend == "halo2/ipa"
    assert init_artifacts.lineage_verifier_key == init_verifier_key
    assert init_artifacts.lineage_proving_key_archive == init_proving_key_archive
    assert init_artifacts.is_init_artifact is True
    assert init_artifacts.is_append_artifact is False
    with pytest.raises(FrozenInstanceError):
        init_artifacts.lineage_proving_key_archive = b"mutated"  # type: ignore[misc]

    append_artifacts = kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_append(
        64,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        append_verifier_key,
        append_proving_key_archive,
    )
    assert append_artifacts.proof_circuit_id == (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert append_artifacts.is_init_artifact is False
    assert append_artifacts.is_append_artifact is True

    generic_artifacts = kagemusha.kagemusha_recursive_spend_lineage_key_artifacts(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        2,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        append_verifier_key,
        append_proving_key_archive,
    )
    assert (
        kagemusha.validate_kagemusha_recursive_spend_lineage_key_artifacts(
            generic_artifacts,
        )
        == generic_artifacts
    )

    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            append_verifier_key,
            append_proving_key_archive,
        )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            append_proving_key_archive,
        )
    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            b"not-zk1",
            init_proving_key_archive,
        )
    duplicate_cid_verifier_key = (
        init_verifier_key
        + _kagemusha_zk1_tlv(
            b"CID1",
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.encode(
                "utf-8",
            ),
        )
    )
    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            duplicate_cid_verifier_key,
            init_proving_key_archive,
        )
    whitespace_cid_verifier_key = _kagemusha_lineage_verifier_key(
        f" {kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1} ",
        0xA5,
    )
    whitespace_cid_proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        whitespace_cid_verifier_key,
        0xA6,
    )
    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            whitespace_cid_verifier_key,
            whitespace_cid_proving_key_archive,
        )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            b"not-norito",
        )
    missing_circuit_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xA5]) * 64,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            missing_circuit_archive,
        )
    smuggled_circuit_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.encode(
                "utf-8",
            )
            + bytes([0xA6]) * 64
        ),
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            smuggled_circuit_archive,
        )
    wrong_commitment_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        append_verifier_key,
        0xA6,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            wrong_commitment_archive,
        )
    smuggled_commitment_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(append_verifier_key),
        _kagemusha_verifier_key_commitment(init_verifier_key) + bytes([0xA7]) * 64,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            smuggled_commitment_archive,
        )
    wrong_version_archive = _kagemusha_lineage_proving_key_archive_raw(
        2,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xA8]) * 64,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            wrong_version_archive,
        )
    empty_proving_key_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        b"",
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            empty_proving_key_archive,
        )
    trailing_payload_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xA9]) * 64,
        trailing_payload=b"\x7f",
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            trailing_payload_archive,
        )
    old_schema_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xAA]) * 64,
        schema_hash=_OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            old_schema_archive,
        )
    packed_struct_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xAB]) * 64,
        flags=_TEST_NORITO_COMPACT_LEN_FLAG | _TEST_NORITO_PACKED_STRUCT_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            packed_struct_archive,
        )
    field_bitset_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xAC]) * 64,
        flags=_TEST_NORITO_COMPACT_LEN_FLAG | _TEST_NORITO_FIELD_BITSET_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            field_bitset_archive,
        )
    overlong_version_length_payload = (
        _kagemusha_overlong_compact_length(2)
        + (1).to_bytes(2, "little")
        + _kagemusha_norito_field(
            _kagemusha_norito_string(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xAD]) * 64))
    )
    overlong_version_length_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        overlong_version_length_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            overlong_version_length_archive,
        )
    oversized_terminal_compact_length_payload = (
        _kagemusha_oversized_terminal_compact_length()
        + (1).to_bytes(2, "little")
        + _kagemusha_norito_field(
            _kagemusha_norito_string(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xB0]) * 64))
    )
    oversized_terminal_compact_length_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        oversized_terminal_compact_length_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            oversized_terminal_compact_length_archive,
        )
    huge_canonical_compact_length_payload = (
        _kagemusha_huge_canonical_compact_length()
        + (1).to_bytes(2, "little")
        + _kagemusha_norito_field(
            _kagemusha_norito_string(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xB1]) * 64))
    )
    huge_canonical_compact_length_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        huge_canonical_compact_length_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            huge_canonical_compact_length_archive,
        )
    circuit_id_bytes = (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.encode(
            "utf-8",
        )
    )
    overlong_circuit_string_payload = (
        _kagemusha_norito_field((1).to_bytes(2, "little"))
        + _kagemusha_norito_field(
            _kagemusha_overlong_compact_length(len(circuit_id_bytes)) + circuit_id_bytes,
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xAE]) * 64))
    )
    overlong_circuit_string_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        overlong_circuit_string_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            overlong_circuit_string_archive,
        )
    invalid_utf8_circuit_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        (
            _kagemusha_norito_field((1).to_bytes(2, "little"))
            + _kagemusha_norito_field(_kagemusha_norito_length(1) + b"\xff")
            + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
            + _kagemusha_norito_field(
                _kagemusha_norito_byte_vec(circuit_id_bytes + bytes([0xAF]) * 64),
            )
        ),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            invalid_utf8_circuit_archive,
        )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            _kagemusha_norito_frame(0x9A),
        )

    invalid_dataclasses = [
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"pk",
            ),
            "proof_circuit_id",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                3,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"pk",
            ),
            "verifier_opening_len",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                128,
                "halo2/kzg",
                b"vk",
                b"pk",
            ),
            "lineage_verifier_key",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"",
                b"pk",
            ),
            "lineage_verifier_key",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                "not-bytes",  # type: ignore[arg-type]
                b"pk",
            ),
            "lineage_verifier_key",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"",
            ),
            "lineage_proving_key_archive",
        ),
    ]
    for artifacts, message in invalid_dataclasses:
        with pytest.raises(ValueError, match=message):
            kagemusha.validate_kagemusha_recursive_spend_lineage_key_artifacts(
                artifacts,
            )
    for malformed, message in (
        (None, "lineage_key_artifacts"),
        ("not-artifacts", "lineage_key_artifacts"),
    ):
        with pytest.raises(ValueError, match=message):
            kagemusha.validate_kagemusha_recursive_spend_lineage_key_artifacts(
                malformed,
            )
    for builder_args, message in (
        (
            (
                3,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"pk",
            ),
            "verifier_opening_len",
        ),
        ((128, "halo2/kzg", b"vk", b"pk"), "lineage_verifier_key"),
        (
            (
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"",
                b"pk",
            ),
            "lineage_verifier_key",
        ),
        (
            (
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"",
            ),
            "lineage_proving_key_archive",
        ),
        (
            (
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                "not-bytes",
                b"pk",
            ),
            "lineage_verifier_key",
        ),
    ):
        with pytest.raises(ValueError, match=message):
            kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
                *builder_args,  # type: ignore[arg-type]
            )


def test_recursive_kagemusha_exports_stable_circuit_ids() -> None:
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 6
    assert kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND == "halo2/ipa"
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-aggregation-v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-spend-lineage-v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-spend-lineage-onehop-v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-spend-lineage-append-v1"
    )
    assert kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS == 64
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 == 64
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1
        == 1
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
        == 8 * 1024 * 1024
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
        == 128
    )
    assert kagemusha.KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES == 64 * 1024 * 1024
    assert "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES" in kagemusha.__all__
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            None,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "",
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "unknown-kagemusha-recursive-spend-circuit",
        )
        == "unknown-kagemusha-recursive-spend-circuit"
    )
    for circuit_id in (
        None,
        "",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ):
        assert (
            kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                circuit_id,
            )
        )
    assert not (
        kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert not (
        kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "unknown-kagemusha-recursive-spend-circuit",
        )
    )
    for lineage_circuit_id in (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ):
        assert kagemusha.is_kagemusha_recursive_spend_lineage_proof_circuit_id(
            lineage_circuit_id,
        )
    assert not kagemusha.is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init()
    for output_circuit_id in (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ):
        assert (
            kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output(
                output_circuit_id,
            )
        )
    for output_circuit_id in (
        None,
        "",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        "unknown-kagemusha-recursive-spend-circuit",
        True,
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output(
                output_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    )
    for previous_circuit_id in (
        "unknown-kagemusha-recursive-spend-circuit",
        None,
        True,
    ):
        assert not (
            kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                previous_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        "",
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        "unknown-kagemusha-recursive-spend-circuit",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        "unknown-kagemusha-recursive-spend-circuit",
    )
    assert not (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        )
    )
    for previous_circuit_id in (
        "unknown-kagemusha-recursive-spend-circuit",
        None,
        True,
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
                previous_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        2,
    )
    assert not kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        64,
    )
    assert not kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        64,
    )
    for circuit_id, hop_count in (
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, -1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 65),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 2**63),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"),
        (None, 1),
        ("", 1),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
    ):
        assert not kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
            circuit_id,
            hop_count,  # type: ignore[arg-type]
        )
        assert kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
            circuit_id,
            hop_count,  # type: ignore[arg-type]
        )
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(0)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(1)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(63)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(64)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(-1)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(2**63)
    for previous_hop_count in (
        1.5,
        float("nan"),
        float("inf"),
        float("-inf"),
        True,
        "1",
    ):
        assert not (
            kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            1,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            63,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            64,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    ), "preferred append selector falls back at the witnessless hop cap"
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            0,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        None,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert not kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        63,
    )
    for circuit_id, previous_hop_count in (
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 0),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
        ),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 64),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, "1"),
    ):
        assert not (
            kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert not (
        kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        )
    ), "semantic previous proofs cannot select Reserved-lineage output"
    for previous_circuit_id, output_circuit_id, previous_hop_count in (
        (
            "unknown-kagemusha-recursive-spend-circuit",
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            "unknown-kagemusha-recursive-spend-circuit",
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            0,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1.5,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("nan"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("inf"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("-inf"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            True,
        ),
    ):
        assert not (
            kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                previous_circuit_id,
                output_circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            64,
        )
    )
    for circuit_id, previous_hop_count in (
        ("", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"),
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )


def test_recursive_kagemusha_availability_requires_bridge_abi_6(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for abi_version in (
        5,
        True,
        "6",
        -1,
        6.5,
        0x1_0000_0000,
        10**100,
    ):
        native = _Native()
        native.kagemusha_recursive_spend_native_bridge_abi_version = (
            lambda abi_version=abi_version: abi_version
        )
        setattr(native, RECURSIVE_COMPACT_METHOD, lambda record, pallas, key_artifacts: b"compact")
        setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, lambda token, verifier_keys: True)
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

        assert kagemusha.is_kagemusha_recursive_spend_available() is False
        assert (
            kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
            is False
        )
        assert (
            kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
            is False
        )
        assert (
            kagemusha.preferred_kagemusha_offline_spend_mode()
            == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
        )
        with pytest.raises(RuntimeError, match="native bridge ABI 6"):
            kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0x70))


def test_recursive_kagemusha_availability_rejects_broken_abi_probe(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def broken_abi_probe() -> int:
        raise OSError("bridge denied")

    native.kagemusha_recursive_spend_native_bridge_abi_version = broken_abi_probe
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="native bridge ABI 6"):
        kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0x71))


def test_recursive_kagemusha_helpers_require_complete_abi_surface(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class PartialNative:
        def kagemusha_recursive_spend_native_bridge_abi_version(self) -> int:
            return 6

        def kagemusha_recursive_spend_init(self, request: bytes) -> bytes:
            return b"init"

        def kagemusha_recursive_spend_append(self, request: bytes) -> bytes:
            return b"append"

        def kagemusha_recursive_spend_transition_profile_init(self, request: bytes) -> bytes:
            return b"transition-profile-init"

        def kagemusha_recursive_spend_transition_profile_append(self, request: bytes) -> bytes:
            return b"transition-profile-append"

        def kagemusha_recursive_spend_lineage_witness_from_init_result(
            self,
            request: bytes,
            bundle: bytes,
        ) -> bytes:
            return b"lineage-init"

        def kagemusha_recursive_spend_verify(self, request: bytes) -> bytes:
            return b"verify"

        def kagemusha_recursive_spend_redeem(self, request: bytes) -> bytes:
            return b"redeem"

    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: PartialNative())

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="complete native bridge ABI 6 surface"):
        kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0x72))


@pytest.mark.parametrize("missing_method", RECURSIVE_SPEND_METHODS)
def test_recursive_kagemusha_helpers_reject_each_missing_abi_method(
    monkeypatch: pytest.MonkeyPatch,
    missing_method: str,
) -> None:
    native = _Native()
    setattr(native, missing_method, None)
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    with pytest.raises(RuntimeError, match="complete native bridge ABI 6 surface"):
        kagemusha.kagemusha_recursive_spend_verify(_kagemusha_input_archive(0x73))


def test_recursive_kagemusha_helpers_reject_empty_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def empty_one(archive: bytes) -> bytes:
        native._reject_probe("empty one", archive)
        return b""

    def empty_two(first: bytes, second: bytes) -> bytes:
        native._reject_probe("empty two", first, second)
        return b""

    def empty_three(first: bytes, second: bytes, third: bytes) -> bytes:
        native._reject_probe("empty three", first, second, third)
        return b""

    native.kagemusha_prove_verified_compact_payment_token_with_records = empty_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, empty_two)
    native.kagemusha_recursive_spend_init = empty_one
    native.kagemusha_recursive_spend_append = empty_one
    native.kagemusha_recursive_spend_transition_profile_init = empty_one
    native.kagemusha_recursive_spend_transition_profile_append = empty_one
    native.kagemusha_recursive_spend_lineage_append_boundary = empty_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = empty_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = empty_three
    native.kagemusha_recursive_spend_verify = empty_one
    native.kagemusha_recursive_spend_redeem = empty_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xC0)
        )
    with pytest.raises(RuntimeError, match="returned empty output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xC1),
            _kagemusha_input_archive(0xC2),
        )

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned empty output"):
            helper(_kagemusha_input_archive(0x80))
    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x81),
            _kagemusha_input_archive(0x82),
        )
    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x83),
            _kagemusha_input_archive(0x84),
            _kagemusha_input_archive(0x85),
        )


def test_recursive_kagemusha_helpers_reject_oversized_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def oversized_one(archive: bytes) -> bytes:
        native._reject_probe("oversized one", archive)
        return b"x" * 49

    def oversized_two(first: bytes, second: bytes) -> bytes:
        native._reject_probe("oversized two", first, second)
        return b"x" * 49

    monkeypatch.setattr(kagemusha, "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES", 48)
    native.kagemusha_prove_verified_compact_payment_token_with_records = oversized_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, oversized_two)
    native.kagemusha_recursive_spend_redeem = oversized_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned oversized output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xC3)
        )
    with pytest.raises(RuntimeError, match="returned oversized output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xC4),
            _kagemusha_input_archive(0xC5),
        )
    with pytest.raises(RuntimeError, match="returned oversized output"):
        kagemusha.kagemusha_recursive_spend_redeem(_kagemusha_input_archive(0x86))


def test_recursive_kagemusha_helpers_reject_oversized_inputs_before_copy_and_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    valid_archive = _kagemusha_input_archive(0xB0)
    oversized_archive = memoryview(valid_archive + b"\x00")
    monkeypatch.setattr(
        kagemusha,
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        len(valid_archive),
    )
    monkeypatch.setattr(
        kagemusha,
        "load_crypto_extension",
        lambda: pytest.fail("oversized Kagemusha input reached native loading"),
    )

    def assert_oversized(call, field: str) -> None:
        with pytest.raises(
            ValueError,
            match=rf"{field} must not exceed {len(valid_archive)} bytes",
        ):
            call()

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        assert_oversized(lambda helper=helper: helper(oversized_archive), "request_archive")

    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_append_boundary(
            oversized_archive
        ),
        "profile_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            oversized_archive,
            valid_archive,
        ),
        "request_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            valid_archive,
            oversized_archive,
        ),
        "bundle_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            oversized_archive,
            valid_archive,
            valid_archive,
        ),
        "previous_witness_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            valid_archive,
            oversized_archive,
            valid_archive,
        ),
        "request_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            valid_archive,
            valid_archive,
            oversized_archive,
        ),
        "bundle_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            oversized_archive
        ),
        "record_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            oversized_archive,
            valid_archive,
        ),
        "record_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            valid_archive,
            oversized_archive,
        ),
        "pallas_open_envelopes_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            oversized_archive,
            valid_archive,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
        "record_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            valid_archive,
            oversized_archive,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
        "pallas_open_envelopes_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            oversized_archive,
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        "compact_token_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            valid_archive,
            valid_archive,
            oversized_archive,
        ),
        "recursive_compact_key_artifacts_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            valid_archive,
            oversized_archive,
        ),
        "recursive_compact_verifier_keys_archive",
    )


def test_recursive_kagemusha_helpers_reject_oversized_memoryview_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    valid_archive = _kagemusha_input_archive(0xB1)
    oversized_archive = memoryview(valid_archive + b"\x00")
    monkeypatch.setattr(
        kagemusha,
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        len(valid_archive),
    )

    def oversized_one(archive: bytes) -> memoryview:
        native._reject_probe("oversized memoryview one", archive)
        return oversized_archive

    native.kagemusha_recursive_spend_redeem = oversized_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned oversized output"):
        kagemusha.kagemusha_recursive_spend_redeem(valid_archive)


def test_recursive_kagemusha_helpers_reject_malformed_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def assert_rejects_malformed_native_outputs(output: bytes) -> None:
        native = _Native()

        def malformed_one(archive: bytes) -> bytes:
            native._reject_probe("malformed one", archive)
            return output

        def malformed_two(first: bytes, second: bytes) -> bytes:
            native._reject_probe("malformed two", first, second)
            return output

        native.kagemusha_prove_verified_compact_payment_token_with_records = malformed_one
        setattr(native, RECURSIVE_AGGREGATION_METHOD, malformed_two)
        native.kagemusha_recursive_spend_redeem = malformed_one
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
                _kagemusha_input_archive(0xC6)
            )
        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
                _kagemusha_input_archive(0xC7),
                _kagemusha_input_archive(0xC8),
            )
        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            kagemusha.kagemusha_recursive_spend_redeem(
                _kagemusha_input_archive(0x87)
            )

    assert_rejects_malformed_native_outputs(b"\x01")

    compressed = bytearray(_kagemusha_norito_frame_with_payload(0x4B))
    compressed[22] = 1
    assert_rejects_malformed_native_outputs(bytes(compressed))

    unsupported_flags = bytearray(_kagemusha_norito_frame_with_payload(0x4B))
    unsupported_flags[39] = 0x08
    assert_rejects_malformed_native_outputs(bytes(unsupported_flags))

    invalid_field_bitset = bytearray(_kagemusha_norito_frame_with_payload(0x4B))
    invalid_field_bitset[39] = 0x20
    assert_rejects_malformed_native_outputs(bytes(invalid_field_bitset))

    assert_rejects_malformed_native_outputs(
        _kagemusha_norito_frame_with_header_padding(
            _kagemusha_norito_frame_with_payload(0x4B), b"\x7f"
        )
    )
    assert_rejects_malformed_native_outputs(
        _kagemusha_norito_frame_with_header_padding(
            _kagemusha_norito_frame_with_payload(0x4B), b"\x00" * 65
        )
    )


def test_recursive_kagemusha_helpers_reject_empty_payload_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def empty_payload_one(archive: bytes) -> bytes:
        native._reject_probe("empty payload one", archive)
        return _kagemusha_norito_frame(0x4B)

    def empty_payload_two(first: bytes, second: bytes) -> bytes:
        native._reject_probe("empty payload two", first, second)
        return _kagemusha_norito_frame(0x4C)

    native.kagemusha_prove_verified_compact_payment_token_with_records = empty_payload_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, empty_payload_two)
    native.kagemusha_recursive_spend_redeem = empty_payload_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xC9)
        )
    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xCA),
            _kagemusha_input_archive(0xCB),
        )
    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        kagemusha.kagemusha_recursive_spend_redeem(_kagemusha_input_archive(0x88))


def test_recursive_kagemusha_helpers_reject_missing_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def missing_one(archive: bytes) -> None:
        native._reject_probe("missing one", archive)
        return None

    def missing_two(first: bytes, second: bytes) -> None:
        native._reject_probe("missing two", first, second)
        return None

    def missing_three(first: bytes, second: bytes, third: bytes) -> None:
        native._reject_probe("missing three", first, second, third)
        return None

    native.kagemusha_prove_verified_compact_payment_token_with_records = missing_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, missing_two)
    native.kagemusha_recursive_spend_init = missing_one
    native.kagemusha_recursive_spend_append = missing_one
    native.kagemusha_recursive_spend_transition_profile_init = missing_one
    native.kagemusha_recursive_spend_transition_profile_append = missing_one
    native.kagemusha_recursive_spend_lineage_append_boundary = missing_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = missing_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = missing_three
    native.kagemusha_recursive_spend_verify = missing_one
    native.kagemusha_recursive_spend_redeem = missing_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xCC)
        )
    with pytest.raises(RuntimeError, match="returned no output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xCD),
            _kagemusha_input_archive(0xCE),
        )

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned no output"):
            helper(_kagemusha_input_archive(0x90))
    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x91),
            _kagemusha_input_archive(0x92),
        )
    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x93),
            _kagemusha_input_archive(0x94),
            _kagemusha_input_archive(0x95),
        )


def test_recursive_kagemusha_helpers_reject_native_text_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def text_one(archive: bytes) -> str:
        native._reject_probe("text one", archive)
        return "not-norito"

    def text_two(first: bytes, second: bytes) -> str:
        native._reject_probe("text two", first, second)
        return "not-norito"

    def text_three(first: bytes, second: bytes, third: bytes) -> str:
        native._reject_probe("text three", first, second, third)
        return "not-norito"

    native.kagemusha_prove_verified_compact_payment_token_with_records = text_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, text_two)
    native.kagemusha_recursive_spend_init = text_one
    native.kagemusha_recursive_spend_append = text_one
    native.kagemusha_recursive_spend_transition_profile_init = text_one
    native.kagemusha_recursive_spend_transition_profile_append = text_one
    native.kagemusha_recursive_spend_lineage_append_boundary = text_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = text_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = text_three
    native.kagemusha_recursive_spend_verify = text_one
    native.kagemusha_recursive_spend_redeem = text_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xCF)
        )
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xD0),
            _kagemusha_input_archive(0xD1),
        )

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
            helper(_kagemusha_input_archive(0xA0))
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0xA1),
            _kagemusha_input_archive(0xA2),
        )
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0xA3),
            _kagemusha_input_archive(0xA4),
            _kagemusha_input_archive(0xA5),
        )


def test_recursive_kagemusha_redeem_propagates_native_multi_hop_lineage_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[bytes] = []
    request = _kagemusha_input_archive(0xA6)

    def rejecting_redeem(request: bytes) -> bytes:
        native._reject_probe("redeem", request)
        calls.append(request)
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: bundle.accumulator.hop_count"
        )

    native.kagemusha_recursive_spend_redeem = rejecting_redeem
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match=r"bundle\.accumulator\.hop_count"):
        kagemusha.kagemusha_recursive_spend_redeem(request)
    assert calls == [request]


def test_recursive_kagemusha_helpers_propagate_forged_lineage_record_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[tuple[str, bytes]] = []
    verify_request = _kagemusha_input_archive(0xA7)
    redeem_request = _kagemusha_input_archive(0xA8)

    def rejecting_verify(request: bytes) -> bytes:
        native._reject_probe("verify", request)
        calls.append(("verify", request))
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment"
        )

    def rejecting_redeem(request: bytes) -> bytes:
        native._reject_probe("redeem", request)
        calls.append(("redeem", request))
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment"
        )

    native.kagemusha_recursive_spend_verify = rejecting_verify
    native.kagemusha_recursive_spend_redeem = rejecting_redeem
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match=r"lineage_verifier_record\.commitment"):
        kagemusha.kagemusha_recursive_spend_verify(verify_request)
    with pytest.raises(RuntimeError, match=r"lineage_verifier_record\.commitment"):
        kagemusha.kagemusha_recursive_spend_redeem(redeem_request)
    assert calls == [
        ("verify", verify_request),
        ("redeem", redeem_request),
    ]


def test_recursive_kagemusha_transition_profile_append_propagates_forged_opening_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[bytes] = []
    request = _kagemusha_input_archive(0xA9)

    def rejecting_transition_profile_append(request: bytes) -> bytes:
        native._reject_probe("transition profile append", request)
        calls.append(request)
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: hop domain metadata mismatch"
        )

    native.kagemusha_recursive_spend_transition_profile_append = (
        rejecting_transition_profile_append
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match="hop domain metadata mismatch"):
        kagemusha.kagemusha_recursive_spend_transition_profile_append(request)
    assert calls == [request]


def test_recursive_kagemusha_availability_fails_closed(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: object())

    assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is False
    )
    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="Kagemusha support"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xD2)
        )
    with pytest.raises(RuntimeError, match="recursive Kagemusha support"):
        kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0xAA))
