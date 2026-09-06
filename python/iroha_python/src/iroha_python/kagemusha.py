"""Canonical KAGEMUSHA wire-version-1 codecs and orchestration bindings.

This module deliberately contains no monetary prover, signer, encryptor,
decryptor, or software-device fallback.  Those operations belong to the
release-pinned native implementation and qualified hardware.
"""

from __future__ import annotations

import base64
import hashlib
import json
from dataclasses import dataclass
from typing import Any, Callable, ClassVar, Final, Mapping, NoReturn, Sequence

from norito.crc64 import crc64 as _crc64_xz

from .address import (
    BASE58_ALPHABET,
    AccountAddress,
    CurveId,
    decode_base_n,
    require_canonical_asset_definition_id,
)
from .crypto import NetworkId, _require_network_id

_MODEL: Final = "iroha_data_model::kagemusha::kagemusha_v1::"
_DEVICE_MODEL: Final = "iroha_data_model::kagemusha::kagemusha_device_v1::"
_COMPACT_LENGTHS: Final = 0x02
_HEADER_BYTES: Final = 40
_MAX_U16: Final = (1 << 16) - 1
_MAX_U32: Final = (1 << 32) - 1
_MAX_U64: Final = (1 << 64) - 1
_MAX_U128: Final = (1 << 128) - 1
_CREDIT_OPENING_BYTES: Final = 200
_ENCRYPTED_CREDIT_BYTES: Final = _CREDIT_OPENING_BYTES + 16
_TOP_UP_REQUEST_MAX_BYTES: Final = 16 * 1024
_DEVICE_MINT_STAGE_COMMAND_MAX_BYTES: Final = 64 * 1024
_DEVICE_MINT_STAGE_RESULT_MAX_BYTES: Final = 128
_TOP_UP_INSTRUCTION_MAX_BYTES: Final = _TOP_UP_REQUEST_MAX_BYTES + 1024
_TOP_UP_INSTRUCTION_WIRE_ID: Final = "iroha.kagemusha.v1.top_up"
_TOP_UP_INSTRUCTION_TYPE_NAME: Final = (
    "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1"
)
_INSTRUCTION_BOX_TYPE_NAME: Final = "(alloc::string::String, alloc::vec::Vec<u8>)"
_MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES: Final = 9_211
_MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES: Final = 12_288
KAGEMUSHA_REDEMPTION_PROOF_MAX_BYTES_V1: Final = 6_528


class KagemushaError(ValueError):
    """Stable canonical-codec or public-binding failure."""


def _fail(message: str) -> NoReturn:
    raise KagemushaError(message)


def _bytes(value: object, context: str) -> bytes:
    if type(value) is bytes:
        return value
    if type(value) is bytearray:
        return bytes(value)
    if type(value) is memoryview:
        return value.tobytes()
    raise TypeError(f"{context} must be bytes-like")


def _bounded_bytes(value: object, maximum: int, context: str) -> bytes:
    """Reject an oversized buffer before materializing a mutable/view-backed copy."""

    if type(value) in (bytes, bytearray):
        size = len(value)
    elif type(value) is memoryview:
        size = value.nbytes
    else:
        raise TypeError(f"{context} must be bytes-like")
    if size > maximum:
        _fail(f"KAGEMUSHA V1 {context} exceeds {maximum} bytes")
    return _bytes(value, context)


def _unsigned(value: object, maximum: int, context: str) -> int:
    if type(value) is not int or value < 0 or value > maximum:
        _fail(f"{context} is outside its unsigned integer domain")
    return value


def _fixed(value: object, width: int, context: str, *, nonzero: bool = True) -> bytes:
    raw = _bytes(value, context)
    if len(raw) != width or (nonzero and not any(raw)):
        qualifier = "nonzero " if nonzero else ""
        _fail(f"{context} must be exactly one {qualifier}{width}-byte value")
    return raw


def _fixed32(value: object, context: str) -> bytes:
    return _fixed(value, 32, context)


def _raw32(value: object, context: str) -> bytes:
    return _fixed(value, 32, context, nonzero=False)


def _network_bytes(value: object, context: str = "network_id") -> bytes:
    network = _require_network_id(value, context)
    raw = _bytes(network.to_bytes(), context)
    if len(raw) != 32:
        _fail(f"{context} must encode to exactly 32 bytes")
    return raw


def _curve_algorithm_tag(curve: CurveId) -> int:
    mapping = {
        CurveId.ED25519: 0,
        CurveId.MLDSA: 4,
        CurveId.GOST_256_A: 5,
        CurveId.GOST_256_B: 6,
        CurveId.GOST_256_C: 7,
        CurveId.GOST_512_A: 8,
        CurveId.GOST_512_B: 9,
    }
    try:
        return mapping[curve]
    except KeyError as error:
        raise KagemushaError("unsupported account public-key algorithm") from error


def _compact(value: int) -> bytes:
    _unsigned(value, _MAX_U64, "compact length")
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def _field(payload: bytes) -> bytes:
    return _compact(len(payload)) + payload


def _vector(payload: bytes) -> bytes:
    return len(payload).to_bytes(8, "little") + payload


def _const_vec(payload: bytes) -> bytes:
    return len(payload).to_bytes(8, "little") + b"".join(
        _field(bytes((item,))) for item in payload
    )


def _fixed_array_archive(payload: bytes) -> bytes:
    return b"".join(b"\x01" + bytes((item,)) for item in payload)


def _require_fixed_archive(payload: bytes, width: int, context: str) -> None:
    if len(payload) != width * 2 or any(payload[index * 2] != 1 for index in range(width)):
        _fail(f"{context} is not a canonical fixed-byte-array payload")


class KagemushaAssetDefinitionIdV1:
    """Exact canonical bare-Norito asset-definition identity."""

    __slots__ = ("canonical_payload",)

    def __init__(self, value: str | bytes | bytearray | memoryview) -> None:
        if isinstance(value, str):
            literal = require_canonical_asset_definition_id(value, "KAGEMUSHA V1 asset")
            index = {character: position for position, character in enumerate(BASE58_ALPHABET)}
            decoded = decode_base_n([index[character] for character in literal], len(BASE58_ALPHABET))
            payload = _fixed_array_archive(decoded[1:17])
        else:
            payload = _bytes(value, "KAGEMUSHA V1 asset payload")
        _require_fixed_archive(payload, 16, "KAGEMUSHA V1 asset payload")
        object.__setattr__(self, "canonical_payload", payload)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("KAGEMUSHA V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.canonical_payload == other.canonical_payload

    def __hash__(self) -> int:
        return hash(self.canonical_payload)


def _validate_account_payload(payload: bytes) -> None:
    if not payload or len(payload) > 512:
        _fail("KAGEMUSHA V1 account payload is empty or oversized")
    if len(payload) < 4 or int.from_bytes(payload[:4], "little") != 0:
        _fail("KAGEMUSHA V1 account must use a canonical single-key controller")
    reader = _Reader(payload[4:], "account")
    public_key = reader.field("public_key")
    reader.eof()
    if len(public_key) < 8:
        _fail("account public key is truncated")
    count = int.from_bytes(public_key[:8], "little")
    items = _Reader(public_key[8:], "account.public_key")
    raw = bytearray()
    for index in range(count):
        item = items.field(str(index))
        if len(item) != 1:
            _fail("account public key byte field is invalid")
        raw.extend(item)
    items.eof()
    if not raw or raw[0] > 10:
        _fail("account public key algorithm is invalid")


class KagemushaAccountIdV1:
    """Exact canonical bare-Norito universal account identity."""

    __slots__ = ("canonical_payload",)

    def __init__(self, value: str | bytes | bytearray | memoryview) -> None:
        if isinstance(value, str):
            if not value or value.strip() != value:
                _fail("KAGEMUSHA V1 account must use exact canonical I105 form")
            address = AccountAddress.parse_encoded(value)
            if address.to_i105() != value:
                _fail("KAGEMUSHA V1 account must use exact canonical I105 form")
            controller = address.controller
            key = bytes((_curve_algorithm_tag(controller.curve),)) + controller.public_key
            payload = (0).to_bytes(4, "little") + _field(_const_vec(key))
        else:
            payload = _bytes(value, "KAGEMUSHA V1 account payload")
        _validate_account_payload(payload)
        object.__setattr__(self, "canonical_payload", payload)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("KAGEMUSHA V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.canonical_payload == other.canonical_payload

    def __hash__(self) -> int:
        return hash(self.canonical_payload)


class KagemushaAssetIncarnationV1:
    """Marked 32-byte registered asset-incarnation token."""

    __slots__ = ("hash_bytes",)

    def __init__(self, value: bytes | bytearray | memoryview) -> None:
        raw = _raw32(value, "KAGEMUSHA V1 asset incarnation")
        if raw[-1] & 1 != 1:
            _fail("KAGEMUSHA V1 asset incarnation must be a marked Iroha hash")
        object.__setattr__(self, "hash_bytes", raw)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("KAGEMUSHA V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.hash_bytes == other.hash_bytes

    def __hash__(self) -> int:
        return hash(self.hash_bytes)


class KagemushaDevicePublicKeyV1:
    """Canonical fixed-width public key bytes; native code authenticates the point."""

    __slots__ = ("sec1_bytes",)

    def __init__(self, value: bytes | bytearray | memoryview) -> None:
        raw = _bytes(value, "KAGEMUSHA V1 device public key")
        if len(raw) != 65 or raw[0] != 4 or not any(raw[1:]):
            _fail("device public key must be nonzero 65-byte uncompressed SEC1")
        object.__setattr__(self, "sec1_bytes", raw)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("KAGEMUSHA V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.sec1_bytes == other.sec1_bytes

    def __hash__(self) -> int:
        return hash(self.sec1_bytes)


class KagemushaDeviceSignatureV1:
    """Canonical fixed-width signature bytes; native code verifies the signature."""

    __slots__ = ("raw_bytes",)

    def __init__(self, value: bytes | bytearray | memoryview) -> None:
        raw = _bytes(value, "KAGEMUSHA V1 device signature")
        if len(raw) != 64 or not any(raw[:32]) or not any(raw[32:]):
            _fail("device signature must be nonzero fixed-width r || s")
        object.__setattr__(self, "raw_bytes", raw)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("KAGEMUSHA V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.raw_bytes == other.raw_bytes

    def __hash__(self) -> int:
        return hash(self.raw_bytes)


class _Kind:
    __slots__ = ("name",)

    def __init__(self, name: str) -> None:
        self.name = name


_U8, _U16, _U32, _U64, _U128 = (
    _Kind(name) for name in ("u8", "u16", "u32", "u64", "u128")
)
_FIXED32 = _Kind("fixed32")
_RAW32 = _Kind("raw32")
_FIXED24 = _Kind("fixed24")
_NETWORK = _Kind("network")
_ASSET = _Kind("asset")
_INCARNATION = _Kind("incarnation")
_ACCOUNT = _Kind("account")
_PUBLIC_KEY = _Kind("public_key")
_SIGNATURE = _Kind("signature")
_VECTOR = _Kind("vector")
_OPERATION_KIND = _Kind("operation_kind")
_COMMIT_EVIDENCE = _Kind("commit_evidence")
_CREDIT_PURPOSE = _Kind("credit_purpose")
_OPTIONAL_MINT_AUTHORIZATION = _Kind("optional_mint_authorization")
_MINT_FRAME = _Kind("mint_frame")

_DEFINITIONS: dict[type[Any], tuple[tuple[str, object], ...]] = {}


class _Model:
    __slots__ = ()

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("KAGEMUSHA V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        fields = _DEFINITIONS[type(self)]
        return type(other) is type(self) and all(
            getattr(self, name) == getattr(other, name) for name, _kind in fields
        )

    def __hash__(self) -> int:
        return hash(tuple(getattr(self, name) for name, _kind in _DEFINITIONS[type(self)]))

    def __repr__(self) -> str:
        values = ", ".join(
            f"{name}={getattr(self, name)!r}" for name, _kind in _DEFINITIONS[type(self)]
        )
        return f"{type(self).__name__}({values})"


def _define_model(
    name: str,
    fields: Sequence[tuple[str, object]],
    validator: Callable[[Mapping[str, object]], None] | None = None,
) -> type[_Model]:
    field_tuple = tuple(fields)
    field_names = tuple(field for field, _kind in field_tuple)

    def __init__(self: _Model, *args: object, **kwargs: object) -> None:
        if args and kwargs:
            raise TypeError(f"{name} accepts positional or keyword fields, not both")
        if args:
            if len(args) != len(field_names):
                raise TypeError(f"{name} contains missing or unknown fields")
            values = dict(zip(field_names, args, strict=True))
        else:
            if len(kwargs) != len(field_names) or set(kwargs) != set(field_names):
                raise TypeError(f"{name} contains missing or unknown fields")
            values = dict(kwargs)
        normalized = {
            field: _normalize_type(kind, values[field], f"{name}.{field}")
            for field, kind in field_tuple
        }
        if validator is not None:
            validator(normalized)
        for field in field_names:
            object.__setattr__(self, field, normalized[field])

    model = type(
        name,
        (_Model,),
        {"__slots__": field_names, "__init__": __init__, "__doc__": f"Canonical {name} value."},
    )
    _DEFINITIONS[model] = field_tuple
    return model


def _require_version(value: object) -> None:
    if value != 1:
        _fail("KAGEMUSHA V1 wire version must be 1")


def _header(values: Mapping[str, object], *, positive_amount: bool = False) -> None:
    _require_version(values["version"])
    if values["scale"] > 28:
        _fail("KAGEMUSHA V1 asset scale exceeds 28")
    if positive_amount and values["amount"] == 0:
        _fail("KAGEMUSHA V1 amount must be positive")


def _same_bytes(left: bytes, right: bytes, context: str) -> None:
    if left != right:
        _fail(f"{context} does not match")


def _x25519(value: bytes, context: str) -> None:
    if len(value) != 32 or not any(value):
        _fail(f"{context} must be a nonzero 32-byte X25519 key")


def _validate_proof_vectors(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["eq_protocol_digest"] == values["ep_protocol_digest"]:
        _fail("KAGEMUSHA V1 proof protocol digests are aliased")
    if values["eq_deferred_audit"] == values["ep_deferred_audit"]:
        _fail("KAGEMUSHA V1 proof deferred audits are aliased")
    eq_proof, ep_proof = values["eq_proof"], values["ep_proof"]
    if not eq_proof or not ep_proof or len(eq_proof) > 2495 or len(ep_proof) > 2495:
        _fail("KAGEMUSHA V1 current proof bytes are out of bounds")
    if len(eq_proof) + len(ep_proof) > 4990:
        _fail("KAGEMUSHA V1 combined proof bytes are out of bounds")
    eq_history, ep_history = values["eq_history"], values["ep_history"]
    if (
        len(eq_history) != 544
        or len(ep_history) != 544
        or not any(eq_history)
        or not any(ep_history)
        or eq_history == ep_history
    ):
        _fail("KAGEMUSHA V1 history accumulators are invalid")


def _validate_paired_proof(values: Mapping[str, object]) -> None:
    if values["guard_eq_credential_audit"] == values["guard_ep_credential_audit"]:
        _fail("KAGEMUSHA V1 proof credential audits are aliased")
    _validate_proof_vectors(values)


KagemushaHardwareCredentialV1 = _define_model(
    "KagemushaHardwareCredentialV1",
    (
        ("version", _U16), ("credential_id", _FIXED32), ("network_id", _NETWORK),
        ("hardware_profile_id", _FIXED32), ("suite_id", _FIXED32),
        ("firmware_policy_digest", _FIXED32), ("policy_epoch", _U64),
        ("lane_commitment", _FIXED32), ("hardware_epoch_id", _FIXED32),
        ("hardware_epoch_generation", _U64), ("device_public_key", _PUBLIC_KEY),
        ("device_key_reference", _FIXED32), ("issued_at_ms", _U64),
        ("expires_at_ms", _U64), ("governance_signature", _SIGNATURE),
    ),
    lambda value: _validate_hardware_credential(value),
)
KagemushaPastaStateCommitmentV1 = _define_model(
    "KagemushaPastaStateCommitmentV1",
    (("eq", _RAW32), ("ep", _RAW32)),
    lambda value: _fail("Pasta state commitment must be fully zero or fully present")
    if bool(any(value["eq"])) != bool(any(value["ep"]))
    else None,
)
KagemushaPairedProofV1 = _define_model(
    "KagemushaPairedProofV1",
    (
        ("version", _U16), ("eq_protocol_digest", _FIXED32),
        ("ep_protocol_digest", _FIXED32), ("semantic_digest", _FIXED32),
        ("guard_eq_credential_audit", _FIXED32),
        ("guard_ep_credential_audit", _FIXED32), ("eq_deferred_audit", _FIXED32),
        ("ep_deferred_audit", _FIXED32), ("eq_proof", _VECTOR),
        ("ep_proof", _VECTOR), ("eq_history", _VECTOR), ("ep_history", _VECTOR),
    ),
    _validate_paired_proof,
)
KagemushaCreditOpeningV1 = _define_model(
    "KagemushaCreditOpeningV1",
    (
        ("version", _U16), ("credit_id", _FIXED32), ("amount", _U128),
        ("credit_commitment_opening", _FIXED32),
        ("recipient_binding_opening", _FIXED32), ("recovery_nonce", _FIXED32),
    ),
    lambda value: (_require_version(value["version"]),
                   _fail("KAGEMUSHA V1 credit opening amount must be positive") if value["amount"] == 0 else None),
)
KagemushaEncryptedCreditAadV1 = _define_model(
    "KagemushaEncryptedCreditAadV1",
    (
        ("version", _U16), ("purpose", _CREDIT_PURPOSE), ("context_digest", _FIXED32),
        ("issuance_or_transition_commitment", _FIXED32), ("credit_id", _FIXED32),
        ("amount", _U128),
    ),
    lambda value: (_require_version(value["version"]),
                   _fail("KAGEMUSHA V1 encrypted-credit AAD amount must be positive") if value["amount"] == 0 else None),
)
KagemushaEncryptedCreditEnvelopeV1 = _define_model(
    "KagemushaEncryptedCreditEnvelopeV1",
    (
        ("version", _U16), ("ephemeral_x25519_public_key", _RAW32),
        ("nonce", _FIXED24), ("ciphertext_and_tag", _VECTOR),
    ),
    lambda value: _validate_envelope_values(value),
)

_OPERATION_KINDS: Final = (
    "bootstrap", "mint_fold", "send_split", "receive_fold", "redeem_split", "rotate",
)
_CREDIT_PURPOSES: Final = ("mint", "peer")
_IPM1_PAYLOAD_KIND_TAGS: Final[Mapping[str, int]] = {
    "request": 1,
    "payment": 2,
    "acknowledgement": 3,
}


KagemushaLifecycleBindingV1 = _define_model(
    "KagemushaLifecycleBindingV1",
    (
        ("version", _U16), ("network_id", _NETWORK), ("protocol_version", _U16),
        ("suite_id", _FIXED32), ("vk_digest", _FIXED32), ("release_id", _FIXED32),
        ("asset", _ASSET), ("asset_incarnation", _INCARNATION), ("scale", _U32),
        ("liability_pool_id", _FIXED32), ("hardware_profile_id", _FIXED32),
        ("policy_epoch", _U64), ("operation_kind", _OPERATION_KIND),
        ("request_id", _RAW32), ("receiver_lane_commitment", _RAW32),
        ("credit_id", _RAW32),
        ("ciphertext_digest", _RAW32),
    ),
    lambda value: _validate_lifecycle_values(value),
)
KagemushaPaymentRequestV1 = _define_model(
    "KagemushaPaymentRequestV1",
    (
        ("version", _U16), ("release_id", _FIXED32), ("network_id", _NETWORK),
        ("asset", _ASSET), ("asset_incarnation", _INCARNATION), ("scale", _U32),
        ("liability_pool_id", _FIXED32), ("recipient", _ACCOUNT),
        ("amount", _U128), ("recipient_encryption_key", _FIXED32),
        ("hardware_credential", KagemushaHardwareCredentialV1),
        ("request_id", _FIXED32), ("issued_at_ms", _U64), ("expires_at_ms", _U64),
        ("signature", _SIGNATURE),
    ),
    lambda value: _validate_request_values(value),
)
KagemushaPeerCreditContextV1 = _define_model(
    "KagemushaPeerCreditContextV1",
    (
        ("version", _U16), ("request_digest", _FIXED32),
        ("amount", _U128), ("sender_before_commitment", _FIXED32),
        ("sender_after_commitment", _FIXED32),
        ("prepared_transfer_digest", _FIXED32), ("recipient_encryption_key", _FIXED32),
    ),
    lambda value: _validate_peer_context_values(value),
)
KagemushaPaymentOutputV1 = _define_model(
    "KagemushaPaymentOutputV1",
    (
        ("version", _U16), ("request_digest", _FIXED32), ("amount", _U128),
        ("sender_before_commitment", _FIXED32),
        ("sender_after_commitment", _FIXED32), ("transition_nullifier", _FIXED32),
        ("credit_id", _FIXED32), ("ciphertext_commitment", _FIXED32),
        ("commit_evidence", _COMMIT_EVIDENCE), ("committed_at_ms", _U64),
    ),
    lambda value: _validate_payment_output_values(value),
)
KagemushaTrustedCommitTimeV1 = _define_model(
    "KagemushaTrustedCommitTimeV1",
    (("time_evidence_commitment", _FIXED32),),
)
KagemushaMonotonicLeaseV1 = _define_model(
    "KagemushaMonotonicLeaseV1",
    (("lease_evidence_commitment", _FIXED32),),
)
_COMMIT_EVIDENCE_TYPES: Final = (
    KagemushaTrustedCommitTimeV1,
    KagemushaMonotonicLeaseV1,
)
KagemushaOutboxReservationV1 = _define_model(
    "KagemushaOutboxReservationV1",
    (
        ("reservation_id", _FIXED32), ("operation_kind", _OPERATION_KIND),
        ("reserved_outbox_bytes", _U32), ("issued_at_ms", _U64), ("expires_at_ms", _U64),
    ),
    lambda value: _validate_outbox_reservation_values(value),
)
KagemushaHardwareTerminalBodyV1 = _define_model(
    "KagemushaHardwareTerminalBodyV1",
    (
        ("version", _U16), ("candidate_envelope_digest", _FIXED32),
        ("lifecycle_binding_digest", _FIXED32), ("transition_nullifier", _FIXED32),
        ("outbox_reservation_commitment", _FIXED32),
        ("commit_evidence", _COMMIT_EVIDENCE), ("hardware_profile_id", _FIXED32),
        ("policy_epoch", _U64), ("private_successor_commitment", _FIXED32),
        ("private_journal_commitment", _FIXED32), ("private_recovery_commitment", _FIXED32),
    ),
    lambda value: _validate_terminal_body_values(value),
)
KagemushaCommitCertificateV1 = _define_model(
    "KagemushaCommitCertificateV1",
    (
        ("version", _U16), ("certificate_id", _FIXED32),
        ("candidate_envelope_digest", _FIXED32), ("lifecycle_binding_digest", _FIXED32),
        ("transition_nullifier", _FIXED32), ("outbox_reservation_commitment", _FIXED32),
        ("commit_evidence", _COMMIT_EVIDENCE), ("hardware_profile_id", _FIXED32),
        ("policy_epoch", _U64), ("hardware_terminal_commitment", _FIXED32),
    ),
    lambda value: _validate_commit_certificate_values(value),
)
KagemushaRedemptionProofV1 = _define_model(
    "KagemushaRedemptionProofV1",
    (
        ("version", _U16), ("eq_protocol_digest", _FIXED32),
        ("ep_protocol_digest", _FIXED32), ("semantic_digest", _FIXED32),
        ("candidate_envelope_digest", _FIXED32), ("commit_certificate_digest", _FIXED32),
        ("eq_deferred_audit", _FIXED32), ("ep_deferred_audit", _FIXED32),
        ("eq_proof", _VECTOR), ("ep_proof", _VECTOR),
        ("eq_history", _VECTOR), ("ep_history", _VECTOR),
    ),
    lambda value: _validate_proof_vectors(value),
)
KagemushaPaymentProofV1 = _define_model(
    "KagemushaPaymentProofV1",
    (
        ("version", _U16), ("eq_protocol_digest", _FIXED32),
        ("ep_protocol_digest", _FIXED32), ("semantic_digest", _FIXED32),
        ("candidate_envelope_digest", _FIXED32), ("commit_certificate_digest", _FIXED32),
        ("eq_deferred_audit", _FIXED32), ("ep_deferred_audit", _FIXED32),
        ("eq_proof", _VECTOR), ("ep_proof", _VECTOR),
        ("eq_history", _VECTOR), ("ep_history", _VECTOR),
    ),
    lambda value: _validate_proof_vectors(value),
)
KagemushaPaymentV1 = _define_model(
    "KagemushaPaymentV1",
    (
        ("version", _U16), ("output", KagemushaPaymentOutputV1),
        ("encrypted_credit", _VECTOR), ("commit_certificate", KagemushaCommitCertificateV1),
        ("proof", KagemushaPaymentProofV1),
    ),
    lambda value: _validate_nested_versions(value, ("output", "commit_certificate", "proof")),
)
KagemushaInboxReceiptV1 = _define_model(
    "KagemushaInboxReceiptV1",
    (("version", _U16), ("credit_id", _FIXED32), ("receipt_commitment", _FIXED32)),
    lambda value: _require_version(value["version"]),
)
KagemushaAcknowledgementV1 = _define_model(
    "KagemushaAcknowledgementV1",
    (
        ("version", _U16), ("request_digest", _FIXED32), ("payment_digest", _FIXED32),
        ("inbox_receipt", KagemushaInboxReceiptV1), ("signature", _SIGNATURE),
    ),
    lambda value: _validate_nested_version(value, "inbox_receipt"),
)
KagemushaMintAuthorizationContextV1 = _define_model(
    "KagemushaMintAuthorizationContextV1",
    (
        ("version", _U16), ("operation_id", _FIXED32), ("release_id", _FIXED32),
        ("suite_id", _FIXED32), ("vk_digest", _FIXED32),
        ("artifact_manifest_digest", _FIXED32), ("network_id", _NETWORK),
        ("asset", _ASSET), ("asset_incarnation", _INCARNATION), ("scale", _U32),
        ("liability_pool_id", _FIXED32), ("amount", _U128), ("payer", _ACCOUNT),
        ("recipient", _ACCOUNT), ("hardware_credential_id", _FIXED32),
        ("hardware_profile_id", _FIXED32), ("policy_epoch", _U64),
        ("recipient_credential_commitment", _FIXED32), ("credit_commitment", _FIXED32),
        ("recipient_one_time_key", _FIXED32),
    ),
    lambda value: _validate_mint_context_values(value),
)
KagemushaMintAuthorizationStatementV1 = _define_model(
    "KagemushaMintAuthorizationStatementV1",
    (
        ("version", _U16), ("context", KagemushaMintAuthorizationContextV1),
        ("issuance_commitment", _FIXED32), ("credit_id", _FIXED32),
        ("ciphertext_digest", _FIXED32),
    ),
    lambda value: _validate_nested_version(value, "context"),
)
KagemushaMintAuthorizationV1 = _define_model(
    "KagemushaMintAuthorizationV1",
    (
        ("version", _U16), ("statement", KagemushaMintAuthorizationStatementV1),
        ("proof", KagemushaPairedProofV1),
    ),
    lambda value: _validate_nested_versions(value, ("statement", "proof")),
)
KagemushaMintCreditStatementV1 = _define_model(
    "KagemushaMintCreditStatementV1",
    (
        ("version", _U16), ("lifecycle", KagemushaLifecycleBindingV1),
        ("recipient_credential_commitment", _FIXED32),
        ("authorization_context_digest", _FIXED32), ("mint_authorization_digest", _FIXED32),
        ("amount", _U128), ("issuance_commitment", _FIXED32), ("recipient", _ACCOUNT),
        ("credit_commitment", _FIXED32), ("minted_at_ms", _U64),
    ),
    lambda value: _validate_mint_statement_values(value),
)
KagemushaMintCreditV1 = _define_model(
    "KagemushaMintCreditV1",
    (
        ("version", _U16), ("statement", KagemushaMintCreditStatementV1),
        ("proof", KagemushaPairedProofV1), ("finality_certificate_binding", _FIXED32),
        ("finality_authority_head", _FIXED32), ("finality_genesis_roster_id", _FIXED32),
        ("finality_proof_binding_digest", _FIXED32), ("encrypted_credit", _VECTOR),
        ("artifact_manifest_digest", _FIXED32),
    ),
    lambda value: _validate_nested_versions(value, ("statement", "proof")),
)
KagemushaDeviceMintStageCommandV1 = _define_model(
    "KagemushaDeviceMintStageCommandV1",
    (
        ("version", _U16), ("canonical_authorization", _MINT_FRAME),
        ("canonical_mint_credit", _MINT_FRAME),
    ),
    lambda value: _require_version(value["version"]),
)
KagemushaDeviceMintStageResultV1 = _define_model(
    "KagemushaDeviceMintStageResultV1",
    (("version", _U16), ("disposition", _U8), ("credit_id", _FIXED32)),
    lambda value: _validate_device_mint_stage_result_values(value),
)
KagemushaRedemptionStatementV1 = _define_model(
    "KagemushaRedemptionStatementV1",
    (
        ("version", _U16), ("lifecycle", KagemushaLifecycleBindingV1),
        ("amount", _U128), ("beneficiary", _ACCOUNT), ("terminal_nullifier", _FIXED32),
        ("redemption_commitment", _FIXED32), ("redemption_id", _FIXED32),
        ("commit_evidence", _COMMIT_EVIDENCE),
    ),
    lambda value: _validate_redemption_statement_values(value),
)
KagemushaRedemptionVoucherV1 = _define_model(
    "KagemushaRedemptionVoucherV1",
    (
        ("version", _U16), ("statement", KagemushaRedemptionStatementV1),
        ("commit_certificate", KagemushaCommitCertificateV1),
        ("proof", KagemushaRedemptionProofV1),
        ("artifact_manifest_digest", _FIXED32),
    ),
    lambda value: _validate_nested_versions(value, ("statement", "commit_certificate", "proof")),
)
KagemushaTopUpRequestV1 = _define_model(
    "KagemushaTopUpRequestV1",
    (
        ("version", _U16), ("operation_id", _FIXED32),
        ("issuance_commitment", _FIXED32), ("credit_id", _FIXED32),
        ("release_id", _FIXED32), ("suite_id", _FIXED32), ("vk_digest", _FIXED32),
        ("network_id", _NETWORK), ("asset", _ASSET), ("asset_incarnation", _INCARNATION),
        ("scale", _U32), ("amount", _U128), ("liability_pool_id", _FIXED32),
        ("payer", _ACCOUNT), ("recipient", _ACCOUNT),
        ("hardware_credential", KagemushaHardwareCredentialV1),
        ("recipient_credential_commitment", _FIXED32), ("credit_commitment", _FIXED32),
        ("recipient_one_time_key", _FIXED32), ("encrypted_credit", _VECTOR),
        ("artifact_manifest_digest", _FIXED32),
        ("mint_authorization", _OPTIONAL_MINT_AUTHORIZATION),
    ),
    lambda value: _validate_top_up_values(value),
)
KagemushaRedemptionRequestV1 = _define_model(
    "KagemushaRedemptionRequestV1",
    (("version", _U16), ("operation_id", _FIXED32), ("voucher", KagemushaRedemptionVoucherV1)),
    lambda value: _validate_nested_version(value, "voucher"),
)


def _validate_nested_version(values: Mapping[str, object], field: str) -> None:
    _require_version(values["version"])
    if values[field].version != values["version"]:
        _fail(f"KAGEMUSHA V1 {field} version mismatch")


def _validate_device_mint_stage_result_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["disposition"] not in (0, 1):
        _fail("KAGEMUSHA V1 mint-stage disposition must be 0 or 1")
    _fixed32(values["credit_id"], "mint-stage result credit ID")


def _validate_nested_versions(values: Mapping[str, object], fields: Sequence[str]) -> None:
    _require_version(values["version"])
    if any(values[field].version != values["version"] for field in fields):
        _fail("KAGEMUSHA V1 nested version mismatch")


def _validate_hardware_credential(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["policy_epoch"] == 0 or values["issued_at_ms"] >= values["expires_at_ms"]:
        _fail("KAGEMUSHA V1 hardware credential header is invalid")
    _same_bytes(
        values["device_key_reference"],
        device_key_reference(values["device_public_key"]),
        "hardware credential device key reference",
    )


def _validate_envelope_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    _x25519(values["ephemeral_x25519_public_key"], "encrypted-credit ephemeral key")
    if len(values["ciphertext_and_tag"]) != _ENCRYPTED_CREDIT_BYTES:
        _fail(f"KAGEMUSHA V1 ciphertext and tag must be exactly {_ENCRYPTED_CREDIT_BYTES} bytes")


def _validate_lifecycle_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["protocol_version"] != 1 or values["policy_epoch"] == 0:
        _fail("KAGEMUSHA V1 lifecycle header is invalid")
    _same_bytes(
        values["liability_pool_id"],
        liability_pool_id(values["network_id"], values["asset"], values["asset_incarnation"]),
        "lifecycle liability pool",
    )
    request_set = any(values["request_id"]) and any(values["receiver_lane_commitment"])
    credit_set = any(values["credit_id"]) and any(values["ciphertext_digest"])
    all_zero = not any(
        any(values[name])
        for name in ("request_id", "receiver_lane_commitment", "credit_id", "ciphertext_digest")
    )
    operation = values["operation_kind"]
    if (
        (operation == "send_split" and not (request_set and credit_set))
        or (
            operation == "mint_fold"
            and (
                any(values["request_id"])
                or any(values["receiver_lane_commitment"])
                or not credit_set
            )
        )
        or (operation not in ("send_split", "mint_fold") and not all_zero)
    ):
        _fail("KAGEMUSHA V1 lifecycle operation identities are invalid")


def _validate_request_values(values: Mapping[str, object]) -> None:
    _header(values)
    if values["amount"] == 0:
        _fail("KAGEMUSHA V1 request amount must be positive")
    _x25519(values["recipient_encryption_key"], "request recipient encryption key")
    if (
        values["expires_at_ms"] <= values["issued_at_ms"]
        or values["expires_at_ms"] - values["issued_at_ms"] > 300_000
    ):
        _fail("KAGEMUSHA V1 request validity window is invalid")
    credential = values["hardware_credential"]
    if (
        _network_bytes(values["network_id"]) != _network_bytes(credential.network_id)
        or values["issued_at_ms"] < credential.issued_at_ms
        or values["expires_at_ms"] > credential.expires_at_ms
    ):
        _fail("KAGEMUSHA V1 request credential binding is invalid")


def _validate_payment_output_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if (
        values["amount"] == 0
        or values["committed_at_ms"] == 0
        or values["sender_before_commitment"] == values["sender_after_commitment"]
    ):
        _fail("KAGEMUSHA V1 payment output is invalid")

def _validate_peer_context_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if (
        values["amount"] == 0
        or values["sender_before_commitment"] == values["sender_after_commitment"]
    ):
        _fail("KAGEMUSHA V1 peer credit context is invalid")
    _x25519(values["recipient_encryption_key"], "peer context recipient encryption key")


def _validate_mint_context_values(values: Mapping[str, object]) -> None:
    _header(values, positive_amount=True)
    if values["policy_epoch"] == 0:
        _fail("mint authorization policy epoch must be positive")
    _x25519(values["recipient_one_time_key"], "mint recipient key")
    _same_bytes(
        values["liability_pool_id"],
        liability_pool_id(values["network_id"], values["asset"], values["asset_incarnation"]),
        "mint authorization liability pool",
    )


def _validate_mint_statement_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if (
        values["lifecycle"].version != values["version"]
        or values["lifecycle"].operation_kind != "mint_fold"
        or values["amount"] == 0
        or values["minted_at_ms"] == 0
    ):
        _fail("KAGEMUSHA V1 mint statement is invalid")


def _validate_redemption_statement_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if (
        values["lifecycle"].version != values["version"]
        or values["lifecycle"].operation_kind != "redeem_split"
        or values["amount"] == 0
    ):
        _fail("KAGEMUSHA V1 redemption statement is invalid")


def _validate_outbox_reservation_values(values: Mapping[str, object]) -> None:
    minimum = {
        "send_split": 25_728,
        "redeem_split": 26_112,
    }.get(values["operation_kind"])
    if (
        minimum is None
        or values["reserved_outbox_bytes"] < minimum
        or values["issued_at_ms"] >= values["expires_at_ms"]
    ):
        _fail("KAGEMUSHA V1 outbox reservation is invalid")


def _validate_terminal_body_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["policy_epoch"] == 0:
        _fail("KAGEMUSHA V1 terminal body policy epoch must be positive")


def _validate_top_up_values(values: Mapping[str, object]) -> None:
    _header(values, positive_amount=True)
    _x25519(values["recipient_one_time_key"], "top-up recipient key")


def _normalize_type(kind: object, value: object, context: str) -> object:
    if kind is _U8:
        return _unsigned(value, 0xFF, context)
    if kind is _U16:
        return _unsigned(value, _MAX_U16, context)
    if kind is _U32:
        return _unsigned(value, _MAX_U32, context)
    if kind is _U64:
        return _unsigned(value, _MAX_U64, context)
    if kind is _U128:
        return _unsigned(value, _MAX_U128, context)
    if kind is _FIXED32:
        return _fixed32(value, context)
    if kind is _RAW32:
        return _raw32(value, context)
    if kind is _FIXED24:
        return _fixed(value, 24, context, nonzero=False)
    if kind is _NETWORK:
        return _require_network_id(value, context)
    if kind is _ASSET:
        return value if isinstance(value, KagemushaAssetDefinitionIdV1) else KagemushaAssetDefinitionIdV1(value)
    if kind is _INCARNATION:
        return value if isinstance(value, KagemushaAssetIncarnationV1) else KagemushaAssetIncarnationV1(value)
    if kind is _ACCOUNT:
        return value if isinstance(value, KagemushaAccountIdV1) else KagemushaAccountIdV1(value)
    if kind is _PUBLIC_KEY:
        return value if isinstance(value, KagemushaDevicePublicKeyV1) else KagemushaDevicePublicKeyV1(value)
    if kind is _SIGNATURE:
        return value if isinstance(value, KagemushaDeviceSignatureV1) else KagemushaDeviceSignatureV1(value)
    if kind is _VECTOR:
        return _bytes(value, context)
    if kind is _MINT_FRAME:
        return _bounded_bytes(value, 7936, context)
    if kind is _OPERATION_KIND:
        if value not in _OPERATION_KINDS:
            raise TypeError(f"{context} is not a KAGEMUSHA V1 operation kind")
        return value
    if kind is _COMMIT_EVIDENCE:
        if not isinstance(value, _COMMIT_EVIDENCE_TYPES):
            raise TypeError(f"{context} is not KAGEMUSHA V1 commit evidence")
        return value
    if kind is _CREDIT_PURPOSE:
        if value not in _CREDIT_PURPOSES:
            raise TypeError(f"{context} is not a KAGEMUSHA V1 credit purpose")
        return value
    if kind is _OPTIONAL_MINT_AUTHORIZATION:
        if value is not None and not isinstance(value, KagemushaMintAuthorizationV1):
            raise TypeError(f"{context} must be KagemushaMintAuthorizationV1 or None")
        return value
    if isinstance(kind, type) and isinstance(value, kind):
        return value
    raise TypeError(f"{context} must be {getattr(kind, '__name__', kind)!s}")


class _Reader:
    def __init__(self, payload: bytes, context: str) -> None:
        self.payload, self.context, self.offset = payload, context, 0

    def field(self, name: str) -> bytes:
        value, shift, used = 0, 0, 0
        while used < 10:
            if self.offset >= len(self.payload):
                _fail(f"{self.context}.{name} is truncated")
            byte = self.payload[self.offset]
            self.offset += 1
            if used == 9 and byte & 0xFE:
                _fail(f"{self.context}.{name} length exceeds u64")
            value |= (byte & 0x7F) << shift
            used += 1
            if not byte & 0x80:
                if used > 1 and byte == 0:
                    _fail(f"{self.context}.{name} length is not minimal")
                break
            shift += 7
        else:
            _fail(f"{self.context}.{name} length exceeds u64")
        end = self.offset + value
        if end > len(self.payload):
            _fail(f"{self.context}.{name} length is invalid")
        result = self.payload[self.offset:end]
        self.offset = end
        return result

    def eof(self) -> None:
        if self.offset != len(self.payload):
            _fail(f"{self.context} contains trailing bytes")


def _encode_type(kind: object, value: object) -> bytes:
    if kind in (_U8, _U16, _U32, _U64, _U128):
        width = {_U8: 1, _U16: 2, _U32: 4, _U64: 8, _U128: 16}[kind]
        return int(value).to_bytes(width, "little")
    if kind in (_FIXED32, _RAW32, _FIXED24):
        return value
    if kind is _NETWORK:
        return _network_bytes(value)
    if kind in (_ASSET, _ACCOUNT):
        return value.canonical_payload
    if kind is _INCARNATION:
        return _field(value.hash_bytes)
    if kind is _PUBLIC_KEY:
        return value.sec1_bytes
    if kind is _SIGNATURE:
        return value.raw_bytes
    if kind in (_VECTOR, _MINT_FRAME):
        return _vector(value)
    if kind is _OPERATION_KIND:
        return _OPERATION_KINDS.index(value).to_bytes(4, "little")
    if kind is _COMMIT_EVIDENCE:
        return _COMMIT_EVIDENCE_TYPES.index(type(value)).to_bytes(4, "little") + _field(
            _encode_model(value)
        )
    if kind is _CREDIT_PURPOSE:
        return _CREDIT_PURPOSES.index(value).to_bytes(4, "little")
    if kind is _OPTIONAL_MINT_AUTHORIZATION:
        return b"\0" if value is None else b"\x01" + _field(_encode_model(value))
    return _encode_model(value)


def _encode_model(value: object) -> bytes:
    try:
        definition = _DEFINITIONS[type(value)]
    except KeyError as error:
        raise TypeError("value is not a KAGEMUSHA V1 model") from error
    return b"".join(
        _field(_encode_type(kind, getattr(value, name))) for name, kind in definition
    )


def _decode_unsigned(payload: bytes, width: int, context: str) -> int:
    if len(payload) != width:
        _fail(f"{context} must contain exactly {width} bytes")
    return int.from_bytes(payload, "little")


def _decode_type(kind: object, payload: bytes, context: str) -> object:
    if kind in (_U8, _U16, _U32, _U64, _U128):
        return _decode_unsigned(
            payload, {_U8: 1, _U16: 2, _U32: 4, _U64: 8, _U128: 16}[kind], context
        )
    if kind is _FIXED32:
        return _fixed32(payload, context)
    if kind is _RAW32:
        return _raw32(payload, context)
    if kind is _FIXED24:
        return _fixed(payload, 24, context, nonzero=False)
    if kind is _NETWORK:
        if len(payload) != 32:
            _fail(f"{context} must contain exactly 32 bytes")
        return NetworkId.from_bytes(payload)
    if kind is _ASSET:
        return KagemushaAssetDefinitionIdV1(payload)
    if kind is _INCARNATION:
        reader = _Reader(payload, context)
        result = KagemushaAssetIncarnationV1(reader.field("hash"))
        reader.eof()
        return result
    if kind is _ACCOUNT:
        return KagemushaAccountIdV1(payload)
    if kind is _PUBLIC_KEY:
        return KagemushaDevicePublicKeyV1(payload)
    if kind is _SIGNATURE:
        return KagemushaDeviceSignatureV1(payload)
    if kind in (_VECTOR, _MINT_FRAME):
        if len(payload) < 8 or int.from_bytes(payload[:8], "little") != len(payload) - 8:
            _fail(f"{context} vector length is invalid")
        value = payload[8:]
        return _bounded_bytes(value, 7936, context) if kind is _MINT_FRAME else value
    if kind is _OPERATION_KIND:
        tag = _decode_unsigned(payload, 4, context)
        if tag >= len(_OPERATION_KINDS):
            _fail(f"{context} has an unknown operation tag")
        return _OPERATION_KINDS[tag]
    if kind is _COMMIT_EVIDENCE:
        variants = _COMMIT_EVIDENCE_TYPES
        if len(payload) < 5:
            _fail(f"{context} enum payload is truncated")
        tag = _decode_unsigned(payload[:4], 4, context)
        if tag >= len(variants):
            _fail(f"{context} has an unknown enum tag")
        reader = _Reader(payload[4:], context)
        result = _decode_model(variants[tag], reader.field("value"))
        reader.eof()
        return result
    if kind is _CREDIT_PURPOSE:
        tag = _decode_unsigned(payload, 4, context)
        if tag >= len(_CREDIT_PURPOSES):
            _fail(f"{context} has an unknown purpose tag")
        return _CREDIT_PURPOSES[tag]
    if kind is _OPTIONAL_MINT_AUTHORIZATION:
        if payload == b"\0":
            return None
        if len(payload) < 3 or payload[0] != 1:
            _fail(f"{context} has an invalid option tag")
        reader = _Reader(payload[1:], context)
        result = _decode_model(KagemushaMintAuthorizationV1, reader.field("value"))
        reader.eof()
        return result
    return _decode_model(kind, payload)


def _decode_model(model: type[Any], payload: bytes) -> Any:
    reader = _Reader(payload, model.__name__)
    values = {
        name: _decode_type(kind, reader.field(name), f"{model.__name__}.{name}")
        for name, kind in _DEFINITIONS[model]
    }
    reader.eof()
    return model(**values)


_SCHEMAS: Final = {
    KagemushaPaymentRequestV1: f"{_MODEL}KagemushaPaymentRequestV1",
    KagemushaPeerCreditContextV1: f"{_MODEL}KagemushaPeerCreditContextV1",
    KagemushaCommitCertificateV1: f"{_MODEL}KagemushaCommitCertificateV1",
    KagemushaRedemptionProofV1: f"{_MODEL}KagemushaRedemptionProofV1",
    KagemushaPaymentProofV1: f"{_MODEL}KagemushaPaymentProofV1",
    KagemushaPaymentV1: f"{_MODEL}KagemushaPaymentV1",
    KagemushaAcknowledgementV1: f"{_MODEL}KagemushaAcknowledgementV1",
    KagemushaMintAuthorizationV1: f"{_MODEL}KagemushaMintAuthorizationV1",
    KagemushaMintCreditV1: f"{_MODEL}KagemushaMintCreditV1",
    KagemushaDeviceMintStageCommandV1: f"{_DEVICE_MODEL}KagemushaDeviceMintStageCommandV1",
    KagemushaDeviceMintStageResultV1: f"{_DEVICE_MODEL}KagemushaDeviceMintStageResultV1",
    KagemushaRedemptionVoucherV1: f"{_MODEL}KagemushaRedemptionVoucherV1",
    KagemushaEncryptedCreditEnvelopeV1: f"{_MODEL}KagemushaEncryptedCreditEnvelopeV1",
    KagemushaEncryptedCreditAadV1: f"{_MODEL}KagemushaEncryptedCreditAadV1",
    KagemushaCreditOpeningV1: f"{_MODEL}KagemushaCreditOpeningV1",
    KagemushaTopUpRequestV1: "iroha.torii.v1.kagemusha.top_up.request",
    KagemushaRedemptionRequestV1: "iroha.torii.v1.kagemusha.redeem.request",
}

_SCHEMA_LIFECYCLE: Final = f"{_MODEL}KagemushaLifecycleBindingV1"
_SCHEMA_PAYMENT_OUTPUT: Final = f"{_MODEL}KagemushaPaymentOutputV1"
_SCHEMA_COMMIT_CERTIFICATE: Final = f"{_MODEL}KagemushaCommitCertificateV1"
_SCHEMA_MINT_CONTEXT: Final = f"{_MODEL}KagemushaMintAuthorizationContextV1"
_SCHEMA_MINT_AUTH_STATEMENT: Final = f"{_MODEL}KagemushaMintAuthorizationStatementV1"
_SCHEMA_MINT_STATEMENT: Final = f"{_MODEL}KagemushaMintCreditStatementV1"
_SCHEMA_REDEMPTION_STATEMENT: Final = f"{_MODEL}KagemushaRedemptionStatementV1"


def _model_alignment(model: type[Any]) -> int:
    if model in (
        KagemushaEncryptedCreditEnvelopeV1,
        KagemushaCommitCertificateV1,
        KagemushaRedemptionProofV1,
        KagemushaPaymentProofV1,
        KagemushaDeviceMintStageCommandV1,
    ):
        return 8
    if model in (
        KagemushaAcknowledgementV1,
        KagemushaDeviceMintStageResultV1,
    ):
        return 2
    return 16


def _header_padding(alignment: int) -> int:
    return 0 if alignment <= 1 else (alignment - (_HEADER_BYTES % alignment)) % alignment


def _frame(type_name: str, payload: bytes, alignment: int = 16) -> bytes:
    padding = _header_padding(alignment)
    type_hash = hashlib.sha256(b"norito:v1:type-name\0" + type_name.encode("ascii")).digest()[:16]
    header = (
        b"NRT0\0\0" + type_hash + b"\0" + len(payload).to_bytes(8, "little")
        + _crc64_xz(payload).to_bytes(8, "little") + bytes((_COMPACT_LENGTHS,))
    )
    return header + bytes(padding) + payload


def _unframe(raw: object, maximum: int, type_name: str, alignment: int) -> bytes:
    archive = _bytes(raw, type_name)
    padding = _header_padding(alignment)
    payload_offset = _HEADER_BYTES + padding
    if not archive or len(archive) > maximum or len(archive) < payload_offset:
        _fail(f"{type_name} archive length is invalid")
    expected_hash = hashlib.sha256(
        b"norito:v1:type-name\0" + type_name.encode("ascii")
    ).digest()[:16]
    if (
        archive[:6] != b"NRT0\0\0"
        or archive[6:22] != expected_hash
        or archive[22] != 0
        or archive[39] != _COMPACT_LENGTHS
        or archive[_HEADER_BYTES:payload_offset] != bytes(padding)
    ):
        _fail(f"{type_name} Norito header is invalid")
    length = int.from_bytes(archive[23:31], "little")
    payload = archive[payload_offset:]
    if length != len(payload) or int.from_bytes(archive[31:39], "little") != _crc64_xz(payload):
        _fail(f"{type_name} Norito payload is invalid")
    return payload


def _encode_top_level(
    value: object,
    model: type[Any],
    maximum: int,
    validate: Callable[[Any], None] | None = None,
) -> bytes:
    if not isinstance(value, model):
        raise TypeError(f"value must be {model.__name__}")
    if validate is not None:
        validate(value)
    encoded = _frame(_SCHEMAS[model], _encode_model(value), _model_alignment(model))
    if len(encoded) > maximum:
        _fail(f"KAGEMUSHA V1 {model.__name__} exceeds {maximum} bytes")
    return encoded


def _decode_top_level(
    raw: object,
    model: type[Any],
    maximum: int,
    validate: Callable[[Any], None] | None = None,
) -> Any:
    archive = _bytes(raw, model.__name__)
    value = _decode_model(
        model,
        _unframe(archive, maximum, _SCHEMAS[model], _model_alignment(model)),
    )
    if validate is not None:
        validate(value)
    if archive != _encode_top_level(value, model, maximum, validate):
        _fail(f"{model.__name__} archive is noncanonical")
    return value


def _digest_encoded(domain: bytes, canonical: bytes) -> bytes:
    return hashlib.sha256(domain + b"\0" + len(canonical).to_bytes(8, "little") + canonical).digest()


def _digest_model(
    domain: bytes, schema: str, value: _Model, alignment: int = 16
) -> bytes:
    return _digest_encoded(domain, _frame(schema, _encode_model(value), alignment))


_DOMAIN: Final = {
    "device_key_reference": b"iroha:kagemusha:v1:device-key-reference",
    "pasta_state_commitment": b"iroha:kagemusha:v1:pasta-state-commitment",
    "liability_pool": b"iroha:kagemusha:v1:liability-pool",
    "request_signing": b"iroha:kagemusha:v1:payment-request-signing",
    "request_digest": b"iroha:kagemusha:v1:payment-request",
    "prepared_transfer": b"iroha:kagemusha:v1:prepared-transfer",
    "payment_body": b"iroha:kagemusha:v1:payment-body",
    "asset_identity": b"iroha:kagemusha:v1:asset-identity",
    "account_identity": b"iroha:kagemusha:v1:account-identity",
    "credit_id": b"iroha:kagemusha:v1:credit-id",
    "peer_credit_opening_commitment": (
        b"iroha:kagemusha:v1:peer-credit-opening-commitment"
    ),
    "peer_credit_context": b"iroha:kagemusha:v1:peer-credit-context",
    "lifecycle": b"iroha:kagemusha:v1:lifecycle-binding",
    "statement": b"iroha:kagemusha:v1:send-split-statement",
    "payment": b"iroha:kagemusha:v1:payment",
    "acknowledgement_signing": b"iroha:kagemusha:v1:acknowledgement-signing",
    "ciphertext": b"iroha:kagemusha:v1:ciphertext",
    "outbox_reservation": b"iroha:kagemusha:v1:outbox-reservation",
    "hardware_terminal_body": b"iroha:kagemusha:v1:hardware-terminal-body",
    "commit_certificate_id": b"iroha:kagemusha:v1:commit-certificate-id",
    "commit_certificate": b"iroha:kagemusha:v1:commit-certificate",
    "mint_context": b"iroha:kagemusha:v1:mint-authorization-context",
    "mint_auth_statement": b"iroha:kagemusha:v1:mint-authorization-statement",
    "mint_auth": b"iroha:kagemusha:v1:mint-authorization",
    "mint_statement": b"iroha:kagemusha:v1:mint-statement",
    "mint_lifecycle_context": b"iroha:kagemusha:v1:mint-lifecycle-context",
    "mint_credit_id": b"iroha:kagemusha:v1:mint-credit-id",
    "redemption_statement": b"iroha:kagemusha:v1:redemption-statement",
    "redemption_id": b"iroha:kagemusha:v1:redemption-id",
}


def device_key_reference(public_key: KagemushaDevicePublicKeyV1) -> bytes:
    if not isinstance(public_key, KagemushaDevicePublicKeyV1):
        raise TypeError("public_key must be KagemushaDevicePublicKeyV1")
    return hashlib.sha256(_DOMAIN["device_key_reference"] + b"\0" + public_key.sec1_bytes).digest()


def pasta_state_commitment(value: KagemushaPastaStateCommitmentV1) -> bytes:
    if not isinstance(value, KagemushaPastaStateCommitmentV1):
        raise TypeError("value must be KagemushaPastaStateCommitmentV1")
    return hashlib.sha256(_DOMAIN["pasta_state_commitment"] + b"\0" + value.eq + value.ep).digest()


def liability_pool_id(
    network_id: NetworkId,
    asset: KagemushaAssetDefinitionIdV1,
    asset_incarnation: KagemushaAssetIncarnationV1,
) -> bytes:
    network = _normalize_type(_NETWORK, network_id, "network_id")
    definition = _normalize_type(_ASSET, asset, "asset")
    incarnation = _normalize_type(_INCARNATION, asset_incarnation, "asset_incarnation")
    payload = _field(_network_bytes(network)) + _field(definition.canonical_payload) + _field(
        _encode_type(_INCARNATION, incarnation)
    )
    return _digest_encoded(
        _DOMAIN["liability_pool"],
        _frame("iroha.kagemusha.v1.liability-pool-preimage", payload, 1),
    )


def _validate_request(request: KagemushaPaymentRequestV1) -> None:
    _same_bytes(
        request.liability_pool_id,
        liability_pool_id(request.network_id, request.asset, request.asset_incarnation),
        "request liability pool",
    )


def asset_identity_digest(value: KagemushaAssetDefinitionIdV1) -> bytes:
    return _digest_encoded(_DOMAIN["asset_identity"], _frame("iroha_data_model::asset::id::model::AssetDefinitionId", value.canonical_payload, 1))


def account_identity_digest(value: KagemushaAccountIdV1) -> bytes:
    return _digest_encoded(_DOMAIN["account_identity"], _frame("iroha_data_model::account::model::AccountId", value.canonical_payload, 8))


def _payment_request_unsigned_transcript(value: KagemushaPaymentRequestV1) -> bytes:
    if not isinstance(value, KagemushaPaymentRequestV1):
        raise TypeError("value must be KagemushaPaymentRequestV1")
    return (value.version.to_bytes(2, "little") + value.release_id + _network_bytes(value.network_id)
        + asset_identity_digest(value.asset) + value.asset_incarnation.hash_bytes + value.scale.to_bytes(4, "little")
        + value.liability_pool_id + account_identity_digest(value.recipient)
        + value.amount.to_bytes(16, "little") + value.recipient_encryption_key
        + value.hardware_credential.credential_id
        + value.request_id + value.issued_at_ms.to_bytes(8, "little") + value.expires_at_ms.to_bytes(8, "little"))


def payment_request_transcript(value: KagemushaPaymentRequestV1) -> bytes:
    return _payment_request_unsigned_transcript(value) + value.signature.raw_bytes


def payment_request_signing_bytes(value: KagemushaPaymentRequestV1) -> bytes:
    return _DOMAIN["request_signing"] + b"\0" + _payment_request_unsigned_transcript(value)

def payment_request_digest(value: KagemushaPaymentRequestV1) -> bytes:
    _validate_request(value)
    return _digest_encoded(_DOMAIN["request_digest"], payment_request_transcript(value))

def ciphertext_digest(value: object) -> bytes:
    return _digest_encoded(_DOMAIN["ciphertext"], _bytes(value, "encrypted credit"))


def prepared_transfer_digest(
    request: KagemushaPaymentRequestV1,
    sender_before_commitment: object,
    sender_after_commitment: object,
    transition_nullifier: object,
    ciphertext_commitment: object,
) -> bytes:
    _validate_request(request)
    before = _fixed32(sender_before_commitment, "sender_before_commitment")
    after = _fixed32(sender_after_commitment, "sender_after_commitment")
    if before == after:
        _fail("sender state commitments must differ")
    transcript = (b"\x01\x00" + payment_request_digest(request)
        + request.amount.to_bytes(16, "little") + before + after
        + _fixed32(transition_nullifier, "transition_nullifier") + request.recipient_encryption_key
        + _fixed32(ciphertext_commitment, "ciphertext_commitment"))
    return _digest_encoded(_DOMAIN["prepared_transfer"], transcript)

def credit_id(transition_nullifier: object, request_digest_value: object) -> bytes:
    return hashlib.sha256(_DOMAIN["credit_id"] + b"\0"
        + _fixed32(transition_nullifier, "transition_nullifier")
        + _fixed32(request_digest_value, "request_digest")).digest()

def peer_credit_opening_commitment(
    request_digest: object,
    recipient_encryption_key: object,
    amount: int,
    credit_commitment_opening: object,
    recipient_binding_opening: object,
    recovery_nonce: object,
) -> bytes:
    """Commit one private peer-credit opening before deriving its credit ID."""

    exact_amount = _unsigned(amount, _MAX_U128, "amount")
    if exact_amount == 0:
        _fail("amount must be positive")
    return hashlib.sha256(
        _DOMAIN["peer_credit_opening_commitment"]
        + b"\x00"
        + (1).to_bytes(2, "little")
        + _fixed32(request_digest, "request_digest")
        + _fixed32(recipient_encryption_key, "recipient_encryption_key")
        + exact_amount.to_bytes(16, "little")
        + _fixed32(credit_commitment_opening, "credit_commitment_opening")
        + _fixed32(recipient_binding_opening, "recipient_binding_opening")
        + _fixed32(recovery_nonce, "recovery_nonce")
    ).digest()


def mint_authorization_context_digest(value: KagemushaMintAuthorizationContextV1) -> bytes:
    return _digest_model(_DOMAIN["mint_context"], _SCHEMA_MINT_CONTEXT, value)


def mint_authorization_statement_digest(value: KagemushaMintAuthorizationStatementV1) -> bytes:
    return _digest_model(_DOMAIN["mint_auth_statement"], _SCHEMA_MINT_AUTH_STATEMENT, value)


def mint_authorization_digest(value: KagemushaMintAuthorizationV1) -> bytes:
    return _digest_model(_DOMAIN["mint_auth"], _SCHEMAS[KagemushaMintAuthorizationV1], value)


def mint_credit_id(value: KagemushaMintCreditStatementV1) -> bytes:
    """Derive the unique mint identity without consuming its current ID field."""

    if not isinstance(value, KagemushaMintCreditStatementV1):
        raise TypeError("value must be KagemushaMintCreditStatementV1")
    lifecycle = value.lifecycle
    # This frozen prefix ends at operation_kind and excludes the current credit ID,
    # ciphertext, and authorization proof bytes from the issuance preimage.
    context = b"".join(
        _field(_encode_type(kind, getattr(lifecycle, name)))
        for name, kind in _DEFINITIONS[KagemushaLifecycleBindingV1][:13]
    )
    lifecycle_digest = _digest_encoded(
        _DOMAIN["mint_lifecycle_context"],
        _frame("iroha.kagemusha.v1.mint-lifecycle-context-preimage", context, 8),
    )
    preimage = b"".join(
        (
            _field(lifecycle_digest),
            _field(value.recipient_credential_commitment),
            _field(value.authorization_context_digest),
            _field(value.amount.to_bytes(16, "little")),
            _field(value.issuance_commitment),
            _field(value.recipient.canonical_payload),
            _field(value.credit_commitment),
        )
    )
    return _digest_encoded(
        _DOMAIN["mint_credit_id"],
        _frame("iroha.kagemusha.v1.mint-credit-id-preimage", preimage, 16),
    )


def mint_credit_statement_digest(value: KagemushaMintCreditStatementV1) -> bytes:
    """Validate the derived identity and return the proof semantic digest."""

    _same_bytes(value.lifecycle.credit_id, mint_credit_id(value), "mint credit ID")
    return _digest_model(_DOMAIN["mint_statement"], _SCHEMA_MINT_STATEMENT, value)


def _validate_envelope_recipient(recipient_key: object | None) -> None:
    if recipient_key is not None:
        _x25519(_raw32(recipient_key, "recipient X25519 key"), "recipient X25519 key")


def _lifecycle_digest(lifecycle: KagemushaLifecycleBindingV1) -> bytes:
    if not isinstance(lifecycle, KagemushaLifecycleBindingV1):
        raise TypeError("lifecycle must be KagemushaLifecycleBindingV1")
    return _digest_model(_DOMAIN["lifecycle"], _SCHEMA_LIFECYCLE, lifecycle, 8)


def lifecycle_binding_digest(lifecycle: KagemushaLifecycleBindingV1) -> bytes:
    """Return the canonical digest of a validated released lifecycle binding."""

    return _lifecycle_digest(lifecycle)


def peer_credit_context(
    output: KagemushaPaymentOutputV1,
    request: KagemushaPaymentRequestV1,
) -> KagemushaPeerCreditContextV1:
    _validate_payment_output(output, request)
    return KagemushaPeerCreditContextV1(
        version=1,
        request_digest=output.request_digest,
        amount=output.amount,
        sender_before_commitment=output.sender_before_commitment,
        sender_after_commitment=output.sender_after_commitment,
        prepared_transfer_digest=prepared_transfer_digest(
            request,
            output.sender_before_commitment,
            output.sender_after_commitment,
            output.transition_nullifier,
            output.ciphertext_commitment,
        ),
        recipient_encryption_key=request.recipient_encryption_key,
    )

def _commit_evidence_transcript(value: object) -> bytes:
    evidence = _normalize_type(_COMMIT_EVIDENCE, value, "commit_evidence")
    if isinstance(evidence, KagemushaTrustedCommitTimeV1):
        tag, commitment = 0, evidence.time_evidence_commitment
    else:
        tag, commitment = 1, evidence.lease_evidence_commitment
    return tag.to_bytes(4, "little") + commitment


def outbox_reservation_commitment(value: KagemushaOutboxReservationV1) -> bytes:
    if not isinstance(value, KagemushaOutboxReservationV1):
        raise TypeError("value must be KagemushaOutboxReservationV1")
    transcript = (
        value.reservation_id
        + _OPERATION_KINDS.index(value.operation_kind).to_bytes(4, "little")
        + value.reserved_outbox_bytes.to_bytes(4, "little")
        + value.issued_at_ms.to_bytes(8, "little")
        + value.expires_at_ms.to_bytes(8, "little")
    )
    return _digest_encoded(_DOMAIN["outbox_reservation"], transcript)


def hardware_terminal_body_commitment(value: KagemushaHardwareTerminalBodyV1) -> bytes:
    if not isinstance(value, KagemushaHardwareTerminalBodyV1):
        raise TypeError("value must be KagemushaHardwareTerminalBodyV1")
    return _digest_model(
        _DOMAIN["hardware_terminal_body"],
        f"{_MODEL}KagemushaHardwareTerminalBodyV1",
        value,
        8,
    )


def _commit_certificate_transcript(
    value: KagemushaCommitCertificateV1, *, include_id: bool
) -> bytes:
    return b"".join(
        (
            value.version.to_bytes(2, "little"),
            value.certificate_id if include_id else b"",
            value.candidate_envelope_digest,
            value.lifecycle_binding_digest,
            value.transition_nullifier,
            value.outbox_reservation_commitment,
            _commit_evidence_transcript(value.commit_evidence),
            value.hardware_profile_id,
            value.policy_epoch.to_bytes(8, "little"),
            value.hardware_terminal_commitment,
        )
    )


def commit_certificate_id(value: KagemushaCommitCertificateV1) -> bytes:
    if not isinstance(value, KagemushaCommitCertificateV1):
        raise TypeError("value must be KagemushaCommitCertificateV1")
    return _digest_encoded(
        _DOMAIN["commit_certificate_id"],
        _commit_certificate_transcript(value, include_id=False),
    )


def _validate_commit_certificate_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["policy_epoch"] == 0:
        _fail("commit certificate policy epoch must be positive")


def _validate_commit_certificate(
    value: KagemushaCommitCertificateV1,
    lifecycle: KagemushaLifecycleBindingV1,
    evidence: object,
    nullifier: object,
) -> None:
    expected_nullifier = _fixed32(nullifier, "transition nullifier")
    if (
        value.lifecycle_binding_digest != _lifecycle_digest(lifecycle)
        or value.transition_nullifier != expected_nullifier
        or value.commit_evidence != evidence
        or value.hardware_profile_id != lifecycle.hardware_profile_id
        or value.policy_epoch != lifecycle.policy_epoch
        or value.certificate_id != commit_certificate_id(value)
    ):
        _fail("KAGEMUSHA V1 commit certificate binding is invalid")
    canonical = _frame(
        _SCHEMAS[KagemushaCommitCertificateV1],
        _encode_model(value),
        _model_alignment(KagemushaCommitCertificateV1),
    )
    if len(canonical) > 1_024:
        _fail("KAGEMUSHA V1 commit certificate exceeds 1024 bytes")


def commit_certificate_digest(value: KagemushaCommitCertificateV1, lifecycle: KagemushaLifecycleBindingV1 | None = None, evidence: object = None, nullifier: object = None) -> bytes:
    if lifecycle is not None:
        _validate_commit_certificate(value, lifecycle, evidence, nullifier)
    else:
        _same_bytes(value.certificate_id, commit_certificate_id(value), "certificate ID")
    return _digest_encoded(_DOMAIN["commit_certificate"], _commit_certificate_transcript(value, include_id=True))

def _validate_redemption_proof(
    value: KagemushaRedemptionProofV1,
    semantic_digest: object,
    candidate_envelope_digest: object,
    certificate_digest: object,
) -> None:
    if (
        value.semantic_digest != _fixed32(semantic_digest, "semantic digest")
        or value.candidate_envelope_digest
        != _fixed32(candidate_envelope_digest, "candidate envelope digest")
        or value.commit_certificate_digest
        != _fixed32(certificate_digest, "commit certificate digest")
    ):
        _fail("KAGEMUSHA V1 redemption proof binding is invalid")
    canonical = _frame(
        _SCHEMAS[KagemushaRedemptionProofV1],
        _encode_model(value),
        _model_alignment(KagemushaRedemptionProofV1),
    )
    if len(canonical) > KAGEMUSHA_REDEMPTION_PROOF_MAX_BYTES_V1:
        _fail("KAGEMUSHA V1 redemption proof exceeds 6528 bytes")


def _validate_payment_output(
    output: KagemushaPaymentOutputV1,
    request: KagemushaPaymentRequestV1,
) -> None:
    request_digest = payment_request_digest(request)
    _same_bytes(output.request_digest, request_digest, "payment output request digest")
    if output.amount != request.amount:
        _fail("payment output amount does not match request")
    _same_bytes(
        output.credit_id,
        credit_id(output.transition_nullifier, request_digest),
        "payment output credit ID",
    )
    if output.committed_at_ms < request.issued_at_ms or output.committed_at_ms >= request.expires_at_ms:
        _fail("payment commit time is outside the request window")

def payment_output_transcript(output: KagemushaPaymentOutputV1) -> bytes:
    if not isinstance(output, KagemushaPaymentOutputV1):
        raise TypeError("output must be KagemushaPaymentOutputV1")
    return (output.version.to_bytes(2, "little") + output.request_digest
        + output.amount.to_bytes(16, "little") + output.sender_before_commitment
        + output.sender_after_commitment + output.transition_nullifier + output.credit_id
        + output.ciphertext_commitment + _commit_evidence_transcript(output.commit_evidence)
        + output.committed_at_ms.to_bytes(8, "little"))


def payment_output_digest(
    output: KagemushaPaymentOutputV1,
    request: KagemushaPaymentRequestV1 | None = None,
) -> bytes:
    if request is not None:
        _validate_payment_output(output, request)
    return _digest_encoded(_DOMAIN["statement"], payment_output_transcript(output))


def payment_body_digest(output: KagemushaPaymentOutputV1, encrypted_credit: object) -> bytes:
    decode_encrypted_credit_envelope(encrypted_credit)
    return _digest_encoded(_DOMAIN["payment_body"], payment_output_digest(output) + ciphertext_digest(encrypted_credit))

def _validate_payment(
    payment: KagemushaPaymentV1,
    request: KagemushaPaymentRequestV1,
) -> None:
    _validate_payment_output(payment.output, request)
    decode_encrypted_credit_envelope(payment.encrypted_credit, request.recipient_encryption_key)
    encrypted_credit_aad_for_peer(payment.output, request)
    certificate = payment.commit_certificate
    _same_bytes(certificate.certificate_id, commit_certificate_id(certificate), "payment certificate ID")
    _same_bytes(certificate.transition_nullifier, payment.output.transition_nullifier, "payment certificate nullifier")
    _same_bytes(_commit_evidence_transcript(certificate.commit_evidence), _commit_evidence_transcript(payment.output.commit_evidence), "payment evidence")
    _same_bytes(payment.proof.candidate_envelope_digest, certificate.candidate_envelope_digest, "payment candidate digest")
    _same_bytes(payment.proof.commit_certificate_digest, commit_certificate_digest(certificate), "payment certificate digest")
    _same_bytes(payment.proof.semantic_digest, payment_body_digest(payment.output, payment.encrypted_credit), "payment semantic digest")
def payment_digest(
    payment: KagemushaPaymentV1,
    request: KagemushaPaymentRequestV1,
) -> bytes:
    _validate_payment(payment, request)
    return _digest_model(
        _DOMAIN["payment"], _SCHEMAS[KagemushaPaymentV1], payment
    )


def _validate_acknowledgement(
    acknowledgement: KagemushaAcknowledgementV1,
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
) -> None:
    _same_bytes(acknowledgement.request_digest, payment_request_digest(request), "acknowledgement request digest")
    _same_bytes(
        acknowledgement.payment_digest,
        payment_digest(payment, request),
        "acknowledgement payment digest",
    )
    _same_bytes(
        acknowledgement.inbox_receipt.credit_id,
        payment.output.credit_id,
        "acknowledgement credit ID",
    )


def acknowledgement_signing_bytes(
    acknowledgement: KagemushaAcknowledgementV1,
) -> bytes:
    if not isinstance(acknowledgement, KagemushaAcknowledgementV1):
        raise TypeError("acknowledgement must be KagemushaAcknowledgementV1")
    payload = b"".join(
        (
            _field(_vector(_DOMAIN["acknowledgement_signing"])),
            _field(_encode_type(_U16, acknowledgement.version)),
            _field(acknowledgement.request_digest),
            _field(acknowledgement.payment_digest),
            _field(_encode_model(acknowledgement.inbox_receipt)),
        )
    )
    return _frame(
        "iroha.kagemusha.v1.acknowledgement-signing-preimage", payload, 8
    )


def _validate_mint_authorization(value: KagemushaMintAuthorizationV1) -> None:
    _same_bytes(
        value.proof.semantic_digest,
        mint_authorization_statement_digest(value.statement),
        "mint authorization proof semantic digest",
    )


def _validate_mint_credit(
    credit: KagemushaMintCreditV1,
    authorization: KagemushaMintAuthorizationV1 | None = None,
) -> None:
    decode_encrypted_credit_envelope(credit.encrypted_credit)
    _same_bytes(
        credit.statement.lifecycle.ciphertext_digest,
        ciphertext_digest(credit.encrypted_credit),
        "mint ciphertext digest",
    )
    _same_bytes(
        credit.proof.semantic_digest,
        mint_credit_statement_digest(credit.statement),
        "mint proof semantic digest",
    )
    if authorization is not None:
        validate_mint_credit_against_authorization(credit, authorization)


def validate_mint_credit_against_authorization(
    credit: KagemushaMintCreditV1,
    authorization: KagemushaMintAuthorizationV1,
) -> bool:
    _validate_mint_credit(credit)
    _validate_mint_authorization(authorization)
    context, statement = authorization.statement.context, credit.statement
    for actual, expected, label in (
        (statement.authorization_context_digest, mint_authorization_context_digest(context), "mint authorization context digest"),
        (statement.mint_authorization_digest, mint_authorization_digest(authorization), "mint authorization digest"),
        (statement.issuance_commitment, authorization.statement.issuance_commitment, "mint issuance commitment"),
        (statement.lifecycle.credit_id, authorization.statement.credit_id, "mint credit ID"),
        (statement.lifecycle.ciphertext_digest, authorization.statement.ciphertext_digest, "mint ciphertext binding"),
        (statement.recipient_credential_commitment, context.recipient_credential_commitment, "mint recipient credential commitment"),
        (statement.credit_commitment, context.credit_commitment, "mint credit commitment"),
        (authorization.statement.ciphertext_digest, ciphertext_digest(credit.encrypted_credit), "mint authorization ciphertext digest"),
    ):
        _same_bytes(actual, expected, label)
    if (
        statement.amount != context.amount
        or statement.recipient != context.recipient
        or statement.lifecycle.release_id != context.release_id
        or statement.lifecycle.suite_id != context.suite_id
        or statement.lifecycle.vk_digest != context.vk_digest
        or _network_bytes(statement.lifecycle.network_id) != _network_bytes(context.network_id)
        or statement.lifecycle.asset != context.asset
        or statement.lifecycle.asset_incarnation != context.asset_incarnation
        or statement.lifecycle.scale != context.scale
        or statement.lifecycle.liability_pool_id != context.liability_pool_id
        or statement.lifecycle.hardware_profile_id != context.hardware_profile_id
        or statement.lifecycle.policy_epoch != context.policy_epoch
        or credit.artifact_manifest_digest != context.artifact_manifest_digest
    ):
        _fail("mint authorization context binding is invalid")
    decode_encrypted_credit_envelope(credit.encrypted_credit, context.recipient_one_time_key)
    return True


def _expected_redemption_id(statement: KagemushaRedemptionStatementV1) -> bytes:
    if not isinstance(statement, KagemushaRedemptionStatementV1):
        raise TypeError("statement must be KagemushaRedemptionStatementV1")
    preimage = b"".join(
        (
            _field(_encode_type(_FIXED32, _lifecycle_digest(statement.lifecycle))),
            _field(_encode_type(_FIXED32, statement.terminal_nullifier)),
            _field(_encode_type(_U128, statement.amount)),
            _field(_encode_type(_ACCOUNT, statement.beneficiary)),
            _field(_encode_type(_FIXED32, statement.redemption_commitment)),
        )
    )
    return _digest_encoded(
        _DOMAIN["redemption_id"],
        _frame("iroha.kagemusha.v1.redemption-id-preimage", preimage),
    )


def redemption_id(statement: KagemushaRedemptionStatementV1) -> bytes:
    """Return the canonical redemption ID preimage digest, ignoring its current ID field."""

    return _expected_redemption_id(statement)


def _validate_redemption_statement(statement: KagemushaRedemptionStatementV1) -> None:
    identities = (
        statement.terminal_nullifier,
        statement.redemption_commitment,
        statement.redemption_id,
    )
    if len(set(identities)) != len(identities):
        _fail("redemption statement identities must be distinct")
    _same_bytes(
        statement.redemption_id,
        _expected_redemption_id(statement),
        "redemption ID",
    )


def redemption_statement_digest(statement: KagemushaRedemptionStatementV1) -> bytes:
    _validate_redemption_statement(statement)
    return _digest_model(
        _DOMAIN["redemption_statement"], _SCHEMA_REDEMPTION_STATEMENT, statement
    )


def _validate_redemption_voucher(voucher: KagemushaRedemptionVoucherV1) -> None:
    statement = voucher.statement
    _validate_redemption_statement(statement)
    _validate_commit_certificate(
        voucher.commit_certificate,
        statement.lifecycle,
        statement.commit_evidence,
        statement.terminal_nullifier,
    )
    _validate_redemption_proof(
        voucher.proof,
        redemption_statement_digest(statement),
        voucher.commit_certificate.candidate_envelope_digest,
        commit_certificate_digest(
            voucher.commit_certificate,
            statement.lifecycle,
            statement.commit_evidence,
            statement.terminal_nullifier,
        ),
    )


def encrypted_credit_aad_for_mint(
    statement: KagemushaMintAuthorizationStatementV1,
) -> KagemushaEncryptedCreditAadV1:
    if not isinstance(statement, KagemushaMintAuthorizationStatementV1):
        raise TypeError("statement must be KagemushaMintAuthorizationStatementV1")
    return KagemushaEncryptedCreditAadV1(
        version=1,
        purpose="mint",
        context_digest=mint_authorization_context_digest(statement.context),
        issuance_or_transition_commitment=statement.issuance_commitment,
        credit_id=statement.credit_id,
        amount=statement.context.amount,
    )


def encrypted_credit_aad_for_peer(
    output: KagemushaPaymentOutputV1,
    request: KagemushaPaymentRequestV1,
) -> KagemushaEncryptedCreditAadV1:
    _validate_payment_output(output, request)
    context = peer_credit_context(output, request)
    return KagemushaEncryptedCreditAadV1(
        version=1,
        purpose="peer",
        context_digest=_digest_model(
            _DOMAIN["peer_credit_context"],
            _SCHEMAS[KagemushaPeerCreditContextV1],
            context,
        ),
        issuance_or_transition_commitment=output.ciphertext_commitment,
        credit_id=output.credit_id,
        amount=output.amount,
    )


def encode_payment_request(value: KagemushaPaymentRequestV1) -> bytes:
    return _encode_top_level(value, KagemushaPaymentRequestV1, 928, _validate_request)


def decode_payment_request(raw: object) -> KagemushaPaymentRequestV1:
    return _decode_top_level(raw, KagemushaPaymentRequestV1, 928, _validate_request)


def encode_payment_proof(value: KagemushaPaymentProofV1) -> bytes:
    return _encode_top_level(value, KagemushaPaymentProofV1, 6528)


def decode_payment_proof(raw: object) -> KagemushaPaymentProofV1:
    return _decode_top_level(raw, KagemushaPaymentProofV1, 6528)


def encode_peer_credit_context(value: KagemushaPeerCreditContextV1) -> bytes:
    return _encode_top_level(value, KagemushaPeerCreditContextV1, 512)


def decode_peer_credit_context(raw: object) -> KagemushaPeerCreditContextV1:
    return _decode_top_level(raw, KagemushaPeerCreditContextV1, 512)


def encode_commit_certificate(
    value: KagemushaCommitCertificateV1,
    lifecycle: KagemushaLifecycleBindingV1,
    evidence: object,
    nullifier: object,
) -> bytes:
    return _encode_top_level(
        value,
        KagemushaCommitCertificateV1,
        1_024,
        lambda item: _validate_commit_certificate(item, lifecycle, evidence, nullifier),
    )


def decode_commit_certificate(
    raw: object,
    lifecycle: KagemushaLifecycleBindingV1,
    evidence: object,
    nullifier: object,
) -> KagemushaCommitCertificateV1:
    return _decode_top_level(
        raw,
        KagemushaCommitCertificateV1,
        1_024,
        lambda item: _validate_commit_certificate(item, lifecycle, evidence, nullifier),
    )


def encode_payment(
    value: KagemushaPaymentV1,
    request: KagemushaPaymentRequestV1,
) -> bytes:
    return _encode_top_level(
        value,
        KagemushaPaymentV1,
        7552,
        lambda item: _validate_payment(item, request),
    )


def decode_payment(
    raw: object,
    request: KagemushaPaymentRequestV1,
) -> KagemushaPaymentV1:
    return _decode_top_level(
        raw,
        KagemushaPaymentV1,
        7552,
        lambda item: _validate_payment(item, request),
    )


def encode_acknowledgement(
    value: KagemushaAcknowledgementV1,
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
) -> bytes:
    return _encode_top_level(
        value, KagemushaAcknowledgementV1, 256,
        lambda item: _validate_acknowledgement(item, request, payment),
    )


def decode_acknowledgement(
    raw: object,
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
) -> KagemushaAcknowledgementV1:
    return _decode_top_level(
        raw, KagemushaAcknowledgementV1, 256,
        lambda item: _validate_acknowledgement(item, request, payment),
    )


def encode_mint_authorization(value: KagemushaMintAuthorizationV1) -> bytes:
    return _encode_top_level(value, KagemushaMintAuthorizationV1, 7936, _validate_mint_authorization)


def decode_mint_authorization(raw: object) -> KagemushaMintAuthorizationV1:
    return _decode_top_level(raw, KagemushaMintAuthorizationV1, 7936, _validate_mint_authorization)


def encode_mint_credit(
    value: KagemushaMintCreditV1,
    authorization: KagemushaMintAuthorizationV1 | None = None,
) -> bytes:
    return _encode_top_level(
        value, KagemushaMintCreditV1, 7936,
        lambda item: _validate_mint_credit(item, authorization),
    )


def decode_mint_credit(
    raw: object, authorization: KagemushaMintAuthorizationV1 | None = None
) -> KagemushaMintCreditV1:
    return _decode_top_level(
        raw, KagemushaMintCreditV1, 7936,
        lambda item: _validate_mint_credit(item, authorization),
    )


def _validate_device_mint_stage_command(
    value: KagemushaDeviceMintStageCommandV1,
) -> KagemushaMintCreditV1:
    _require_version(value.version)
    authorization = decode_mint_authorization(
        _bounded_bytes(value.canonical_authorization, 7936, "mint authorization")
    )
    return decode_mint_credit(
        _bounded_bytes(value.canonical_mint_credit, 7936, "mint credit"), authorization
    )


def encode_device_mint_stage_command_shape(
    value: object,
    canonical_mint_credit: object | None = None,
) -> bytes:
    """Encode a structural operation-16 body without granting hardware authority."""

    if canonical_mint_credit is not None:
        value = KagemushaDeviceMintStageCommandV1(
            version=1,
            canonical_authorization=value,
            canonical_mint_credit=canonical_mint_credit,
        )
    return _encode_top_level(
        value,
        KagemushaDeviceMintStageCommandV1,
        _DEVICE_MINT_STAGE_COMMAND_MAX_BYTES,
        _validate_device_mint_stage_command,
    )


def decode_device_mint_stage_command_shape_exact(
    raw: object,
) -> KagemushaDeviceMintStageCommandV1:
    """Decode one exact bounded operation-16 body and both nested archives."""

    return _decode_top_level(
        _bounded_bytes(raw, _DEVICE_MINT_STAGE_COMMAND_MAX_BYTES, "mint-stage command"),
        KagemushaDeviceMintStageCommandV1,
        _DEVICE_MINT_STAGE_COMMAND_MAX_BYTES,
        _validate_device_mint_stage_command,
    )


def _validate_device_mint_stage_result(
    value: KagemushaDeviceMintStageResultV1,
) -> None:
    _validate_device_mint_stage_result_values(
        {"version": value.version, "disposition": value.disposition, "credit_id": value.credit_id}
    )


def validate_device_mint_stage_result_against_command(
    result: KagemushaDeviceMintStageResultV1,
    command: KagemushaDeviceMintStageCommandV1,
) -> bool:
    """Check only public credit binding; native response authentication remains mandatory."""

    if not isinstance(result, KagemushaDeviceMintStageResultV1):
        raise TypeError("result must be KagemushaDeviceMintStageResultV1")
    if not isinstance(command, KagemushaDeviceMintStageCommandV1):
        raise TypeError("command must be KagemushaDeviceMintStageCommandV1")
    _validate_device_mint_stage_result(result)
    credit = _validate_device_mint_stage_command(command)
    _same_bytes(result.credit_id, credit.statement.lifecycle.credit_id, "mint-stage result credit ID")
    return True


def encode_device_mint_stage_result_shape(
    value: KagemushaDeviceMintStageResultV1,
    command: KagemushaDeviceMintStageCommandV1 | None = None,
) -> bytes:
    """Encode an unauthenticated public result shape for a qualified native adapter."""

    validator = (
        _validate_device_mint_stage_result
        if command is None
        else lambda result: validate_device_mint_stage_result_against_command(result, command)
    )
    return _encode_top_level(
        value, KagemushaDeviceMintStageResultV1, _DEVICE_MINT_STAGE_RESULT_MAX_BYTES, validator
    )


def decode_device_mint_stage_result_shape_exact(
    raw: object,
    command: KagemushaDeviceMintStageCommandV1 | None = None,
) -> KagemushaDeviceMintStageResultV1:
    """Decode a bounded public result and optionally bind it to the command credit."""

    validator = (
        _validate_device_mint_stage_result
        if command is None
        else lambda result: validate_device_mint_stage_result_against_command(result, command)
    )
    return _decode_top_level(
        _bounded_bytes(raw, _DEVICE_MINT_STAGE_RESULT_MAX_BYTES, "mint-stage result"),
        KagemushaDeviceMintStageResultV1,
        _DEVICE_MINT_STAGE_RESULT_MAX_BYTES,
        validator,
    )


def encode_redemption_voucher(value: KagemushaRedemptionVoucherV1) -> bytes:
    return _encode_top_level(
        value, KagemushaRedemptionVoucherV1, 7936, _validate_redemption_voucher
    )


def decode_redemption_voucher(raw: object) -> KagemushaRedemptionVoucherV1:
    return _decode_top_level(
        raw, KagemushaRedemptionVoucherV1, 7936, _validate_redemption_voucher
    )


def encode_encrypted_credit_aad(value: KagemushaEncryptedCreditAadV1) -> bytes:
    return _encode_top_level(value, KagemushaEncryptedCreditAadV1, 256)


def decode_encrypted_credit_aad(raw: object) -> KagemushaEncryptedCreditAadV1:
    return _decode_top_level(raw, KagemushaEncryptedCreditAadV1, 256)


def encode_encrypted_credit_envelope(
    value: KagemushaEncryptedCreditEnvelopeV1, recipient_key: object | None = None
) -> bytes:
    _validate_envelope_recipient(recipient_key)
    return _encode_top_level(value, KagemushaEncryptedCreditEnvelopeV1, 384)


def decode_encrypted_credit_envelope(
    raw: object, recipient_key: object | None = None
) -> KagemushaEncryptedCreditEnvelopeV1:
    _validate_envelope_recipient(recipient_key)
    return _decode_top_level(raw, KagemushaEncryptedCreditEnvelopeV1, 384)


def encode_credit_opening(value: KagemushaCreditOpeningV1) -> bytes:
    raw = _encode_top_level(value, KagemushaCreditOpeningV1, 256)
    if len(raw) != _CREDIT_OPENING_BYTES:
        _fail("KAGEMUSHA V1 credit opening has a noncanonical fixed size")
    return raw


def decode_credit_opening(
    raw: object, expected_credit_id: object | None = None, expected_amount: int | None = None
) -> KagemushaCreditOpeningV1:
    value = _decode_top_level(raw, KagemushaCreditOpeningV1, 256)
    if len(_bytes(raw, "credit opening")) != _CREDIT_OPENING_BYTES:
        _fail("KAGEMUSHA V1 credit opening has a noncanonical fixed size")
    if expected_credit_id is not None:
        _same_bytes(value.credit_id, _fixed32(expected_credit_id, "credit_id"), "credit opening credit ID")
    if expected_amount is not None and value.amount != _unsigned(expected_amount, _MAX_U128, "amount"):
        _fail("credit opening amount does not match")
    return value


def _validate_top_up_request(value: KagemushaTopUpRequestV1) -> None:
    if value.mint_authorization is None:
        _fail("canonical KAGEMUSHA V1 top-up requires mint authorization")
    _validate_mint_authorization(value.mint_authorization)
    context = value.mint_authorization.statement.context
    _same_bytes(
        value.liability_pool_id,
        liability_pool_id(value.network_id, value.asset, value.asset_incarnation),
        "top-up liability pool",
    )
    _same_bytes(
        ciphertext_digest(value.encrypted_credit),
        value.mint_authorization.statement.ciphertext_digest,
        "top-up ciphertext digest",
    )
    _same_bytes(value.issuance_commitment, value.mint_authorization.statement.issuance_commitment, "top-up issuance commitment")
    _same_bytes(value.credit_id, value.mint_authorization.statement.credit_id, "top-up credit ID")
    if (
        value.operation_id != context.operation_id
        or value.release_id != context.release_id
        or value.suite_id != context.suite_id
        or value.vk_digest != context.vk_digest
        or _network_bytes(value.network_id) != _network_bytes(context.network_id)
        or value.asset != context.asset
        or value.asset_incarnation != context.asset_incarnation
        or value.scale != context.scale
        or value.amount != context.amount
        or value.liability_pool_id != context.liability_pool_id
        or value.payer != context.payer
        or value.recipient != context.recipient
        or value.hardware_credential.credential_id != context.hardware_credential_id
        or value.hardware_credential.hardware_profile_id != context.hardware_profile_id
        or value.hardware_credential.policy_epoch != context.policy_epoch
        or value.recipient_credential_commitment != context.recipient_credential_commitment
        or value.credit_commitment != context.credit_commitment
        or value.recipient_one_time_key != context.recipient_one_time_key
        or value.artifact_manifest_digest != context.artifact_manifest_digest
    ):
        _fail("top-up mint authorization context binding is invalid")


def encode_top_up_request(value: KagemushaTopUpRequestV1) -> bytes:
    _validate_top_up_request(value)
    return _encode_top_level(value, KagemushaTopUpRequestV1, _TOP_UP_REQUEST_MAX_BYTES)


def decode_top_up_request(raw: object) -> KagemushaTopUpRequestV1:
    value = _decode_top_level(raw, KagemushaTopUpRequestV1, _TOP_UP_REQUEST_MAX_BYTES)
    _validate_top_up_request(value)
    return value


def encode_redemption_request(value: KagemushaRedemptionRequestV1) -> bytes:
    return _encode_top_level(value, KagemushaRedemptionRequestV1, 8192)


def decode_redemption_request(raw: object) -> KagemushaRedemptionRequestV1:
    return _decode_top_level(raw, KagemushaRedemptionRequestV1, 8192)


@dataclass(frozen=True)
class KagemushaTopUpInstructionV1:
    """Canonical ``TopUpKagemushaV1`` instruction ready for transaction framing."""

    request: KagemushaTopUpRequestV1

    def __post_init__(self) -> None:
        encode_top_up_request(self.request)

    def to_norito_bytes(self) -> bytes:
        """Return the exact framed ``InstructionBox`` accepted by transaction builders."""

        request_archive = encode_top_up_request(self.request)
        request_payload = _unframe(
            request_archive,
            _TOP_UP_REQUEST_MAX_BYTES,
            _SCHEMAS[KagemushaTopUpRequestV1],
            _model_alignment(KagemushaTopUpRequestV1),
        )
        inner_frame = _frame(
            _TOP_UP_INSTRUCTION_TYPE_NAME,
            _field(request_payload),
            16,
        )
        outer_payload = b"".join(
            (
                _field(_field(_TOP_UP_INSTRUCTION_WIRE_ID.encode("ascii"))),
                _field(_vector(inner_frame)),
            )
        )
        archive = _frame(_INSTRUCTION_BOX_TYPE_NAME, outer_payload, 1)
        if len(archive) > _TOP_UP_INSTRUCTION_MAX_BYTES:
            _fail("KAGEMUSHA V1 top-up InstructionBox is oversized")
        return archive

    def to_json(self) -> str:
        """Return the exact native ``Instruction.from_json`` base64 literal."""

        encoded = base64.b64encode(self.to_norito_bytes()).decode("ascii")
        return json.dumps(encoded, separators=(",", ":"))

    def to_instruction(self) -> Any:
        """Decode this archive through the Rust-backed standard transaction boundary."""

        from .crypto import Instruction

        instruction = Instruction.from_json(self.to_json())
        if instruction.wire_id() != _TOP_UP_INSTRUCTION_WIRE_ID:
            raise RuntimeError("native KAGEMUSHA instruction decoder changed the wire ID")
        if bytes(instruction.to_norito_bytes()) != self.to_norito_bytes():
            raise RuntimeError("native KAGEMUSHA instruction decoder changed canonical bytes")
        return instruction


def build_top_up_instruction(
    request: KagemushaTopUpRequestV1,
) -> KagemushaTopUpInstructionV1:
    """Build the sole first-release payer-authorized top-up instruction."""

    return KagemushaTopUpInstructionV1(request)


def encode_top_up_instruction(value: KagemushaTopUpInstructionV1) -> bytes:
    """Encode one exact canonical top-up ``InstructionBox``."""

    if not isinstance(value, KagemushaTopUpInstructionV1):
        raise TypeError("value must be KagemushaTopUpInstructionV1")
    return value.to_norito_bytes()


def decode_top_up_instruction(raw: object) -> KagemushaTopUpInstructionV1:
    """Decode and canonicalize one exact top-up ``InstructionBox``."""

    archive = _bytes(raw, "KAGEMUSHA V1 top-up InstructionBox")
    outer_payload = _unframe(
        archive,
        _TOP_UP_INSTRUCTION_MAX_BYTES,
        _INSTRUCTION_BOX_TYPE_NAME,
        1,
    )
    outer = _Reader(outer_payload, "KAGEMUSHA V1 top-up InstructionBox")
    wire_container = _Reader(outer.field("wire_id"), "KAGEMUSHA V1 top-up wire ID")
    try:
        wire_id = wire_container.field("value").decode("ascii")
    except UnicodeDecodeError as error:
        raise KagemushaError("KAGEMUSHA V1 top-up wire ID is not ASCII") from error
    wire_container.eof()
    if wire_id != _TOP_UP_INSTRUCTION_WIRE_ID:
        _fail("KAGEMUSHA V1 top-up instruction uses the wrong wire ID")
    inner_container = outer.field("inner")
    outer.eof()
    if len(inner_container) < 8:
        _fail("KAGEMUSHA V1 top-up instruction inner frame is truncated")
    inner_length = int.from_bytes(inner_container[:8], "little")
    inner_frame = inner_container[8:]
    if inner_length != len(inner_frame):
        _fail("KAGEMUSHA V1 top-up instruction inner frame length is invalid")
    inner_payload = _unframe(
        inner_frame,
        _TOP_UP_INSTRUCTION_MAX_BYTES,
        _TOP_UP_INSTRUCTION_TYPE_NAME,
        16,
    )
    inner = _Reader(inner_payload, "TopUpKagemushaV1")
    request_payload = inner.field("request")
    inner.eof()
    request_archive = _frame(
        _SCHEMAS[KagemushaTopUpRequestV1],
        request_payload,
        _model_alignment(KagemushaTopUpRequestV1),
    )
    result = KagemushaTopUpInstructionV1(decode_top_up_request(request_archive))
    if result.to_norito_bytes() != archive:
        _fail("KAGEMUSHA V1 top-up InstructionBox is not canonical")
    return result


_KIND_LIMITS: Final[Mapping[str, tuple[int, int]]] = {
    "request": (928, 1243),
    "payment": (7552, 10075),
    "acknowledgement": (256, 347),
    "mint_authorization": (7936, 10587),
    "mint_credit": (7936, 10587),
    "redemption_voucher": (7936, 10587),
}


def _kind_limits(kind: str) -> tuple[int, int]:
    try:
        return _KIND_LIMITS[kind]
    except (KeyError, TypeError) as error:
        raise KagemushaError("unknown KAGEMUSHA V1 payload kind") from error


def encode_text(kind: str, raw: object) -> str:
    maximum_raw, maximum_text = _kind_limits(kind)
    payload = _bytes(raw, "KAGEMUSHA V1 payload")
    if not payload or len(payload) > maximum_raw:
        _fail("KAGEMUSHA V1 payload is empty or oversized")
    text = "kgm1:" + base64.urlsafe_b64encode(payload).decode("ascii").rstrip("=")
    if len(text.encode("ascii")) > maximum_text:
        _fail("KAGEMUSHA V1 text is oversized")
    return text


def decode_text(kind: str, text: str) -> bytes:
    maximum_raw, maximum_text = _kind_limits(kind)
    if type(text) is not str or not text.startswith("kgm1:") or len(text.encode("utf-8")) > maximum_text:
        _fail("KAGEMUSHA V1 text prefix or size is invalid")
    body = text[len("kgm1:") :]
    if not body or len(body) % 4 == 1 or any(
        character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        for character in body
    ):
        _fail("KAGEMUSHA V1 text is not canonical unpadded base64url")
    try:
        raw = base64.urlsafe_b64decode(body + "=" * (-len(body) % 4))
    except Exception as error:
        raise KagemushaError("KAGEMUSHA V1 text is not base64url") from error
    if len(raw) > maximum_raw or encode_text(kind, raw) != text:
        _fail("KAGEMUSHA V1 text is noncanonical or oversized")
    return raw


_ENCODERS: Final = {
    "request": encode_payment_request,
    "payment": encode_payment,
    "acknowledgement": encode_acknowledgement,
    "mint_authorization": encode_mint_authorization,
    "mint_credit": encode_mint_credit,
    "redemption_voucher": encode_redemption_voucher,
}
_DECODERS: Final = {
    "request": decode_payment_request,
    "payment": decode_payment,
    "acknowledgement": decode_acknowledgement,
    "mint_authorization": decode_mint_authorization,
    "mint_credit": decode_mint_credit,
    "redemption_voucher": decode_redemption_voucher,
}


def encode_typed_text(kind: str, value: object, *bindings: object) -> str:
    try:
        raw = _ENCODERS[kind](value, *bindings)
    except KeyError as error:
        raise KagemushaError("unknown KAGEMUSHA V1 payload kind") from error
    return encode_text(kind, raw)


def decode_typed_text(kind: str, text: str, *bindings: object) -> object:
    try:
        decoder = _DECODERS[kind]
    except KeyError as error:
        raise KagemushaError("unknown KAGEMUSHA V1 payload kind") from error
    return decoder(decode_text(kind, text), *bindings)


def _text_length(raw_length: int) -> int:
    return len("kgm1:") + (raw_length * 4 + 2) // 3


def encode_ipm1_payload_kind(kind: str) -> bytes:
    try:
        return bytes((_IPM1_PAYLOAD_KIND_TAGS[kind],))
    except (KeyError, TypeError) as error:
        raise KagemushaError("unknown KAGEMUSHA V1 IPM1 payload kind") from error


def decode_ipm1_payload_kind(raw: object) -> str:
    tag = _bytes(raw, "IPM1 payload kind")
    if len(tag) != 1:
        _fail("KAGEMUSHA V1 IPM1 payload kind must be exactly one byte")
    for kind, expected in _IPM1_PAYLOAD_KIND_TAGS.items():
        if tag[0] == expected:
            return kind
    _fail("unknown KAGEMUSHA V1 IPM1 payload kind")


def encode_ipm1_payload(kind: str, value: object, *bindings: object) -> bytes:
    if kind not in _IPM1_PAYLOAD_KIND_TAGS:
        _fail("unknown KAGEMUSHA V1 IPM1 payload kind")
    return _ENCODERS[kind](value, *bindings)


def decode_ipm1_payload(
    kind_tag: object, raw: object, *bindings: object
) -> object:
    kind = decode_ipm1_payload_kind(kind_tag)
    return _DECODERS[kind](raw, *bindings)


def validate_complete_exchange(
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
    acknowledgement: KagemushaAcknowledgementV1,
) -> int:
    parts = (
        encode_payment_request(request),
        encode_payment(payment, request),
        encode_acknowledgement(acknowledgement, request, payment),
    )
    raw_bytes = sum(map(len, parts))
    text_bytes = sum(_text_length(len(part)) for part in parts)
    if (
        raw_bytes > _MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES
        or text_bytes > _MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES
    ):
        _fail("KAGEMUSHA V1 complete three-message exchange is oversized")
    return raw_bytes


class Kagemusha:
    """Sole public KAGEMUSHA codec/orchestration namespace for wire version 1."""

    wire_version: ClassVar[int] = 1
    device_lifecycle_version: ClassVar[int] = 1
    handoff_capability: ClassVar[str] = "kagemusha_handoff_v1"
    text_prefix: ClassVar[str] = "kgm1:"
    maximum_request_raw_bytes: ClassVar[int] = 928
    maximum_request_text_bytes: ClassVar[int] = 1243
    target_complete_exchange_raw_bytes: ClassVar[int] = 8_960
    maximum_complete_exchange_raw_bytes: ClassVar[int] = _MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES
    maximum_complete_exchange_text_bytes: ClassVar[int] = _MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES
    maximum_paired_proof_bytes: ClassVar[int] = 6528
    maximum_payment_proof_bytes: ClassVar[int] = 6528
    maximum_redemption_proof_bytes: ClassVar[int] = (
        KAGEMUSHA_REDEMPTION_PROOF_MAX_BYTES_V1
    )
    maximum_current_proofs_bytes: ClassVar[int] = 4990
    maximum_parity_proof_bytes: ClassVar[int] = 2495
    history_accumulator_bytes: ClassVar[int] = 544
    maximum_encrypted_credit_bytes: ClassVar[int] = 384
    maximum_credit_opening_bytes: ClassVar[int] = 256
    payment_outbox_minimum_bytes: ClassVar[int] = 25_728
    redemption_outbox_minimum_bytes: ClassVar[int] = 26_112
    maximum_top_up_request_bytes: ClassVar[int] = _TOP_UP_REQUEST_MAX_BYTES
    maximum_device_mint_stage_command_bytes: ClassVar[int] = (
        _DEVICE_MINT_STAGE_COMMAND_MAX_BYTES
    )
    maximum_device_mint_stage_result_bytes: ClassVar[int] = (
        _DEVICE_MINT_STAGE_RESULT_MAX_BYTES
    )
    device_mint_stage_disposition_staged: ClassVar[int] = 0
    device_mint_stage_disposition_exact_duplicate: ClassVar[int] = 1
    top_up_instruction_wire_id: ClassVar[str] = _TOP_UP_INSTRUCTION_WIRE_ID
    maximum_redemption_request_bytes: ClassVar[int] = 8192
    maximum_operation_status_bytes: ClassVar[int] = 4 * 1024 * 1024
    maximum_operation_status_json_bytes: ClassVar[int] = 16 * 1024 * 1024
    payload_kinds: ClassVar[Mapping[str, tuple[int, int]]] = _KIND_LIMITS
    ipm1_payload_kinds: ClassVar[Mapping[str, int]] = _IPM1_PAYLOAD_KIND_TAGS
    operation_kinds: ClassVar[tuple[str, ...]] = _OPERATION_KINDS

    AssetDefinitionId = KagemushaAssetDefinitionIdV1
    AssetIncarnation = KagemushaAssetIncarnationV1
    AccountId = KagemushaAccountIdV1
    DevicePublicKey = KagemushaDevicePublicKeyV1
    DeviceSignature = KagemushaDeviceSignatureV1
    HardwareCredential = KagemushaHardwareCredentialV1
    PastaStateCommitment = KagemushaPastaStateCommitmentV1
    PairedProof = KagemushaPairedProofV1
    CreditOpening = KagemushaCreditOpeningV1
    EncryptedCreditAad = KagemushaEncryptedCreditAadV1
    EncryptedCreditEnvelope = KagemushaEncryptedCreditEnvelopeV1
    LifecycleBinding = KagemushaLifecycleBindingV1
    PaymentRequest = KagemushaPaymentRequestV1
    PeerCreditContext = KagemushaPeerCreditContextV1
    PaymentOutput = KagemushaPaymentOutputV1
    TrustedCommitTime = KagemushaTrustedCommitTimeV1
    MonotonicLease = KagemushaMonotonicLeaseV1
    OutboxReservation = KagemushaOutboxReservationV1
    HardwareTerminalBody = KagemushaHardwareTerminalBodyV1
    CommitCertificate = KagemushaCommitCertificateV1
    RedemptionProof = KagemushaRedemptionProofV1
    PaymentProof = KagemushaPaymentProofV1
    encode_payment_proof = staticmethod(encode_payment_proof)
    decode_payment_proof = staticmethod(decode_payment_proof)
    Payment = KagemushaPaymentV1
    InboxReceipt = KagemushaInboxReceiptV1
    Acknowledgement = KagemushaAcknowledgementV1
    MintAuthorizationContext = KagemushaMintAuthorizationContextV1
    MintAuthorizationStatement = KagemushaMintAuthorizationStatementV1
    MintAuthorization = KagemushaMintAuthorizationV1
    MintCreditStatement = KagemushaMintCreditStatementV1
    MintCredit = KagemushaMintCreditV1
    DeviceMintStageCommand = KagemushaDeviceMintStageCommandV1
    DeviceMintStageResult = KagemushaDeviceMintStageResultV1
    RedemptionStatement = KagemushaRedemptionStatementV1
    RedemptionVoucher = KagemushaRedemptionVoucherV1
    TopUpRequest = KagemushaTopUpRequestV1
    TopUpInstruction = KagemushaTopUpInstructionV1
    RedemptionRequest = KagemushaRedemptionRequestV1
    Error = KagemushaError

    encode_payment_request = staticmethod(encode_payment_request)
    decode_payment_request = staticmethod(decode_payment_request)
    encode_peer_credit_context = staticmethod(encode_peer_credit_context)
    decode_peer_credit_context = staticmethod(decode_peer_credit_context)
    encode_commit_certificate = staticmethod(encode_commit_certificate)
    decode_commit_certificate = staticmethod(decode_commit_certificate)
    encode_payment = staticmethod(encode_payment)
    decode_payment = staticmethod(decode_payment)
    encode_acknowledgement = staticmethod(encode_acknowledgement)
    decode_acknowledgement = staticmethod(decode_acknowledgement)
    encode_mint_authorization = staticmethod(encode_mint_authorization)
    decode_mint_authorization = staticmethod(decode_mint_authorization)
    encode_mint_credit = staticmethod(encode_mint_credit)
    decode_mint_credit = staticmethod(decode_mint_credit)
    encode_device_mint_stage_command_shape = staticmethod(
        encode_device_mint_stage_command_shape
    )
    decode_device_mint_stage_command_shape_exact = staticmethod(
        decode_device_mint_stage_command_shape_exact
    )
    encode_device_mint_stage_result_shape = staticmethod(
        encode_device_mint_stage_result_shape
    )
    decode_device_mint_stage_result_shape_exact = staticmethod(
        decode_device_mint_stage_result_shape_exact
    )
    validate_device_mint_stage_result_against_command = staticmethod(
        validate_device_mint_stage_result_against_command
    )
    encode_redemption_voucher = staticmethod(encode_redemption_voucher)
    decode_redemption_voucher = staticmethod(decode_redemption_voucher)
    encode_credit_opening = staticmethod(encode_credit_opening)
    decode_credit_opening = staticmethod(decode_credit_opening)
    encode_encrypted_credit_aad = staticmethod(encode_encrypted_credit_aad)
    decode_encrypted_credit_aad = staticmethod(decode_encrypted_credit_aad)
    encode_encrypted_credit_envelope = staticmethod(encode_encrypted_credit_envelope)
    decode_encrypted_credit_envelope = staticmethod(decode_encrypted_credit_envelope)
    encode_top_up_request = staticmethod(encode_top_up_request)
    decode_top_up_request = staticmethod(decode_top_up_request)
    build_top_up_instruction = staticmethod(build_top_up_instruction)
    encode_top_up_instruction = staticmethod(encode_top_up_instruction)
    decode_top_up_instruction = staticmethod(decode_top_up_instruction)
    encode_redemption_request = staticmethod(encode_redemption_request)
    decode_redemption_request = staticmethod(decode_redemption_request)
    encode_text = staticmethod(encode_text)
    decode_text = staticmethod(decode_text)
    encode_typed_text = staticmethod(encode_typed_text)
    decode_typed_text = staticmethod(decode_typed_text)
    encode_ipm1_payload_kind = staticmethod(encode_ipm1_payload_kind)
    decode_ipm1_payload_kind = staticmethod(decode_ipm1_payload_kind)
    encode_ipm1_payload = staticmethod(encode_ipm1_payload)
    decode_ipm1_payload = staticmethod(decode_ipm1_payload)
    validate_complete_exchange = staticmethod(validate_complete_exchange)
    validate_mint_credit_against_authorization = staticmethod(validate_mint_credit_against_authorization)
    encrypted_credit_aad_for_mint = staticmethod(encrypted_credit_aad_for_mint)
    encrypted_credit_aad_for_peer = staticmethod(encrypted_credit_aad_for_peer)
    peer_credit_context = staticmethod(peer_credit_context)
    device_key_reference = staticmethod(device_key_reference)
    pasta_state_commitment = staticmethod(pasta_state_commitment)
    liability_pool_id = staticmethod(liability_pool_id)
    payment_request_signing_bytes = staticmethod(payment_request_signing_bytes)
    payment_request_digest = staticmethod(payment_request_digest)
    payment_request_transcript = staticmethod(payment_request_transcript)
    asset_identity_digest = staticmethod(asset_identity_digest)
    account_identity_digest = staticmethod(account_identity_digest)
    lifecycle_binding_digest = staticmethod(lifecycle_binding_digest)
    prepared_transfer_digest = staticmethod(prepared_transfer_digest)
    payment_output_digest = staticmethod(payment_output_digest)
    payment_output_transcript = staticmethod(payment_output_transcript)
    payment_body_digest = staticmethod(payment_body_digest)
    payment_digest = staticmethod(payment_digest)
    acknowledgement_signing_bytes = staticmethod(acknowledgement_signing_bytes)
    ciphertext_digest = staticmethod(ciphertext_digest)
    credit_id = staticmethod(credit_id)
    peer_credit_opening_commitment = staticmethod(peer_credit_opening_commitment)
    outbox_reservation_commitment = staticmethod(outbox_reservation_commitment)
    hardware_terminal_body_commitment = staticmethod(hardware_terminal_body_commitment)
    commit_certificate_id = staticmethod(commit_certificate_id)
    commit_certificate_digest = staticmethod(commit_certificate_digest)
    mint_authorization_context_digest = staticmethod(mint_authorization_context_digest)
    mint_authorization_statement_digest = staticmethod(mint_authorization_statement_digest)
    mint_authorization_digest = staticmethod(mint_authorization_digest)
    mint_credit_id = staticmethod(mint_credit_id)
    mint_credit_statement_digest = staticmethod(mint_credit_statement_digest)
    redemption_statement_digest = staticmethod(redemption_statement_digest)
    redemption_id = staticmethod(redemption_id)


__all__ = ["Kagemusha"]
