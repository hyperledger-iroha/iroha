"""Canonical Kagemusha V1 codecs and orchestration bindings.

This module deliberately contains no monetary prover, signer, encryptor,
decryptor, or software-device fallback.  Those operations belong to the
release-pinned native implementation and qualified hardware.
"""

from __future__ import annotations

import base64
import hashlib
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
_COMPACT_LENGTHS: Final = 0x02
_HEADER_BYTES: Final = 40
_MAX_U16: Final = (1 << 16) - 1
_MAX_U32: Final = (1 << 32) - 1
_MAX_U64: Final = (1 << 64) - 1
_MAX_U128: Final = (1 << 128) - 1
_CREDIT_OPENING_BYTES: Final = 200
_ENCRYPTED_CREDIT_BYTES: Final = _CREDIT_OPENING_BYTES + 16


class KagemushaV1Error(ValueError):
    """Stable canonical-codec or public-binding failure."""


def _fail(message: str) -> NoReturn:
    raise KagemushaV1Error(message)


def _bytes(value: object, context: str) -> bytes:
    if type(value) is bytes:
        return value
    if type(value) is bytearray:
        return bytes(value)
    if type(value) is memoryview:
        return value.tobytes()
    raise TypeError(f"{context} must be bytes-like")


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
        raise KagemushaV1Error("unsupported account public-key algorithm") from error


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
            literal = require_canonical_asset_definition_id(value, "Kagemusha V1 asset")
            index = {character: position for position, character in enumerate(BASE58_ALPHABET)}
            decoded = decode_base_n([index[character] for character in literal], len(BASE58_ALPHABET))
            payload = _fixed_array_archive(decoded[1:17])
        else:
            payload = _bytes(value, "Kagemusha V1 asset payload")
        _require_fixed_archive(payload, 16, "Kagemusha V1 asset payload")
        object.__setattr__(self, "canonical_payload", payload)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("Kagemusha V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.canonical_payload == other.canonical_payload

    def __hash__(self) -> int:
        return hash(self.canonical_payload)


def _validate_account_payload(payload: bytes) -> None:
    if not payload or len(payload) > 512:
        _fail("Kagemusha V1 account payload is empty or oversized")
    if len(payload) < 4 or int.from_bytes(payload[:4], "little") != 0:
        _fail("Kagemusha V1 account must use a canonical single-key controller")
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
                _fail("Kagemusha V1 account must use exact canonical I105 form")
            address = AccountAddress.parse_encoded(value)
            if address.to_i105() != value:
                _fail("Kagemusha V1 account must use exact canonical I105 form")
            controller = address.controller
            key = bytes((_curve_algorithm_tag(controller.curve),)) + controller.public_key
            payload = (0).to_bytes(4, "little") + _field(_const_vec(key))
        else:
            payload = _bytes(value, "Kagemusha V1 account payload")
        _validate_account_payload(payload)
        object.__setattr__(self, "canonical_payload", payload)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("Kagemusha V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.canonical_payload == other.canonical_payload

    def __hash__(self) -> int:
        return hash(self.canonical_payload)


class KagemushaAssetIncarnationV1:
    """Marked 32-byte registered asset-incarnation token."""

    __slots__ = ("hash_bytes",)

    def __init__(self, value: bytes | bytearray | memoryview) -> None:
        raw = _raw32(value, "Kagemusha V1 asset incarnation")
        if raw[-1] & 1 != 1:
            _fail("Kagemusha V1 asset incarnation must be a marked Iroha hash")
        object.__setattr__(self, "hash_bytes", raw)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("Kagemusha V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.hash_bytes == other.hash_bytes

    def __hash__(self) -> int:
        return hash(self.hash_bytes)


class KagemushaDevicePublicKeyV1:
    """Canonical fixed-width public key bytes; native code authenticates the point."""

    __slots__ = ("sec1_bytes",)

    def __init__(self, value: bytes | bytearray | memoryview) -> None:
        raw = _bytes(value, "Kagemusha V1 device public key")
        if len(raw) != 65 or raw[0] != 4 or not any(raw[1:]):
            _fail("device public key must be nonzero 65-byte uncompressed SEC1")
        object.__setattr__(self, "sec1_bytes", raw)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("Kagemusha V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.sec1_bytes == other.sec1_bytes

    def __hash__(self) -> int:
        return hash(self.sec1_bytes)


class KagemushaDeviceSignatureV1:
    """Canonical fixed-width signature bytes; native code verifies the signature."""

    __slots__ = ("raw_bytes",)

    def __init__(self, value: bytes | bytearray | memoryview) -> None:
        raw = _bytes(value, "Kagemusha V1 device signature")
        if len(raw) != 64 or not any(raw[:32]) or not any(raw[32:]):
            _fail("device signature must be nonzero fixed-width r || s")
        object.__setattr__(self, "raw_bytes", raw)

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("Kagemusha V1 values are immutable")

    def __eq__(self, other: object) -> bool:
        return isinstance(other, type(self)) and self.raw_bytes == other.raw_bytes

    def __hash__(self) -> int:
        return hash(self.raw_bytes)


class _Kind:
    __slots__ = ("name",)

    def __init__(self, name: str) -> None:
        self.name = name


_U16, _U32, _U64, _U128 = (_Kind(name) for name in ("u16", "u32", "u64", "u128"))
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
_CREDIT_PURPOSE = _Kind("credit_purpose")
_OPTIONAL_MINT_AUTHORIZATION = _Kind("optional_mint_authorization")

_DEFINITIONS: dict[type[Any], tuple[tuple[str, object], ...]] = {}


class _Model:
    __slots__ = ()

    def __setattr__(self, _name: str, _value: object) -> None:
        raise AttributeError("Kagemusha V1 values are immutable")

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
        _fail("Kagemusha V1 wire version must be 1")


def _header(values: Mapping[str, object], *, positive_amount: bool = False) -> None:
    _require_version(values["version"])
    if values["scale"] > 28:
        _fail("Kagemusha V1 asset scale exceeds 28")
    if positive_amount and values["amount"] == 0:
        _fail("Kagemusha V1 amount must be positive")


def _same_bytes(left: bytes, right: bytes, context: str) -> None:
    if left != right:
        _fail(f"{context} does not match")


def _same_state_commitment(
    left: KagemushaPastaStateCommitmentV1,
    right: KagemushaPastaStateCommitmentV1,
    context: str,
) -> None:
    if left != right:
        _fail(f"{context} does not match")


def _validate_live_state_commitments(values: Mapping[str, object], context: str) -> None:
    before = values["sender_before_commitment"]
    after = values["sender_after_commitment"]
    if not any(before.eq) or not any(before.ep) or not any(after.eq) or not any(after.ep):
        _fail(f"{context} state commitments must be nonzero")
    if before == after:
        _fail(f"{context} predecessor and successor commitments must differ")


def _x25519(value: bytes, context: str) -> None:
    if len(value) != 32 or not any(value):
        _fail(f"{context} must be a nonzero 32-byte X25519 key")


def _validate_proof_vectors(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["eq_protocol_digest"] == values["ep_protocol_digest"]:
        _fail("Kagemusha V1 proof protocol digests are aliased")
    if values["eq_deferred_audit"] == values["ep_deferred_audit"]:
        _fail("Kagemusha V1 proof deferred audits are aliased")
    eq_proof, ep_proof = values["eq_proof"], values["ep_proof"]
    if not eq_proof or not ep_proof or len(eq_proof) > 2495 or len(ep_proof) > 2495:
        _fail("Kagemusha V1 current proof bytes are out of bounds")
    if len(eq_proof) + len(ep_proof) > 4990:
        _fail("Kagemusha V1 combined proof bytes are out of bounds")
    eq_history, ep_history = values["eq_history"], values["ep_history"]
    if (
        len(eq_history) != 544
        or len(ep_history) != 544
        or not any(eq_history)
        or not any(ep_history)
        or eq_history == ep_history
    ):
        _fail("Kagemusha V1 history accumulators are invalid")


def _validate_paired_proof(values: Mapping[str, object]) -> None:
    if values["guard_eq_credential_audit"] == values["guard_ep_credential_audit"]:
        _fail("Kagemusha V1 proof credential audits are aliased")
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
                   _fail("Kagemusha V1 credit opening amount must be positive") if value["amount"] == 0 else None),
)
KagemushaEncryptedCreditAadV1 = _define_model(
    "KagemushaEncryptedCreditAadV1",
    (
        ("version", _U16), ("purpose", _CREDIT_PURPOSE), ("context_digest", _FIXED32),
        ("issuance_or_transition_commitment", _FIXED32), ("credit_id", _FIXED32),
        ("amount", _U128),
    ),
    lambda value: (_require_version(value["version"]),
                   _fail("Kagemusha V1 encrypted-credit AAD amount must be positive") if value["amount"] == 0 else None),
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
    "bootstrap", "mint_fold", "send_split", "receive_fold", "redeem_split",
    "rotate",
)
_CREDIT_PURPOSES: Final = ("mint", "peer")


KagemushaOutboxReservationV1 = _define_model(
    "KagemushaOutboxReservationV1",
    (
        ("reservation_id", _FIXED32),
        ("operation_kind", _OPERATION_KIND),
        ("reserved_outbox_bytes", _U32),
        ("issued_at_ms", _U64),
        ("expires_at_ms", _U64),
    ),
    lambda value: _fail("Kagemusha V1 outbox reservation is invalid")
    if (
        value["operation_kind"] not in ("send_split", "redeem_split")
        or value["reserved_outbox_bytes"] < 26_112
        or value["issued_at_ms"] >= value["expires_at_ms"]
    )
    else None,
)


KagemushaLifecycleBindingV1 = _define_model(
    "KagemushaLifecycleBindingV1",
    (
        ("version", _U16), ("network_id", _NETWORK), ("protocol_version", _U16),
        ("suite_id", _FIXED32), ("vk_digest", _FIXED32), ("release_id", _FIXED32),
        ("asset", _ASSET), ("asset_incarnation", _INCARNATION), ("scale", _U32),
        ("liability_pool_id", _FIXED32), ("hardware_profile_id", _FIXED32),
        ("policy_epoch", _U64), ("operation_kind", _OPERATION_KIND),
        ("request_id", _RAW32), ("credit_id", _RAW32),
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
        ("recipient_lane_id", _FIXED32), ("recipient_encryption_key", _FIXED32),
        ("amount", _U128), ("hardware_credential", KagemushaHardwareCredentialV1),
        ("request_id", _FIXED32), ("issued_at_ms", _U64), ("expires_at_ms", _U64),
        ("signature", _SIGNATURE),
    ),
    lambda value: _validate_request_values(value),
)
KagemushaPeerCreditContextV1 = _define_model(
    "KagemushaPeerCreditContextV1",
    (
        ("version", _U16), ("request_digest", _FIXED32),
        ("sender_before_commitment", KagemushaPastaStateCommitmentV1),
        ("sender_after_commitment", KagemushaPastaStateCommitmentV1),
        ("lifecycle_context_digest", _FIXED32), ("recipient_lane_id", _FIXED32),
        ("recipient_encryption_key", _FIXED32), ("committed_at_ms", _U64),
        ("hardware_transition_commitment", _FIXED32),
    ),
    lambda value: _validate_peer_context_values(value),
)
KagemushaTransferStatementV1 = _define_model(
    "KagemushaTransferStatementV1",
    (
        ("version", _U16), ("lifecycle", KagemushaLifecycleBindingV1),
        ("amount", _U128), ("transition_nullifier", _FIXED32),
        ("sender_before_commitment", KagemushaPastaStateCommitmentV1),
        ("sender_after_commitment", KagemushaPastaStateCommitmentV1),
        ("request_digest", _FIXED32), ("recipient_lane_id", _FIXED32),
        ("recipient_encryption_key", _FIXED32), ("ciphertext_commitment", _FIXED32),
        ("committed_at_ms", _U64), ("hardware_transition_commitment", _FIXED32),
    ),
    lambda value: _validate_transfer_values(value),
)
KagemushaPaymentV1 = _define_model(
    "KagemushaPaymentV1",
    (
        ("version", _U16), ("statement", KagemushaTransferStatementV1),
        ("proof", KagemushaPairedProofV1), ("encrypted_credit", _VECTOR),
    ),
    lambda value: _validate_nested_versions(value, ("statement", "proof")),
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
KagemushaRedemptionStatementV1 = _define_model(
    "KagemushaRedemptionStatementV1",
    (
        ("version", _U16), ("lifecycle", KagemushaLifecycleBindingV1),
        ("amount", _U128), ("beneficiary", _ACCOUNT), ("terminal_nullifier", _FIXED32),
        ("sender_before_commitment", KagemushaPastaStateCommitmentV1),
        ("sender_after_commitment", KagemushaPastaStateCommitmentV1),
        ("redemption_commitment", _FIXED32), ("redemption_id", _FIXED32),
        ("committed_at_ms", _U64), ("hardware_transition_commitment", _FIXED32),
    ),
    lambda value: _validate_redemption_statement_values(value),
)
KagemushaRedemptionVoucherV1 = _define_model(
    "KagemushaRedemptionVoucherV1",
    (
        ("version", _U16), ("statement", KagemushaRedemptionStatementV1),
        ("proof", KagemushaPairedProofV1),
    ),
    lambda value: _validate_nested_versions(value, ("statement", "proof")),
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
        _fail(f"Kagemusha V1 {field} version mismatch")


def _validate_nested_versions(values: Mapping[str, object], fields: Sequence[str]) -> None:
    _require_version(values["version"])
    if any(values[field].version != values["version"] for field in fields):
        _fail("Kagemusha V1 nested version mismatch")


def _validate_hardware_credential(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["policy_epoch"] == 0 or values["issued_at_ms"] >= values["expires_at_ms"]:
        _fail("Kagemusha V1 hardware credential header is invalid")
    _same_bytes(
        values["device_key_reference"],
        device_key_reference(values["device_public_key"]),
        "hardware credential device key reference",
    )


def _validate_envelope_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    _x25519(values["ephemeral_x25519_public_key"], "encrypted-credit ephemeral key")
    if len(values["ciphertext_and_tag"]) != _ENCRYPTED_CREDIT_BYTES:
        _fail(f"Kagemusha V1 ciphertext and tag must be exactly {_ENCRYPTED_CREDIT_BYTES} bytes")


def _validate_lifecycle_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if values["protocol_version"] != 1 or values["policy_epoch"] == 0:
        _fail("Kagemusha V1 lifecycle header is invalid")
    _same_bytes(
        values["liability_pool_id"],
        liability_pool_id(values["network_id"], values["asset"], values["asset_incarnation"]),
        "lifecycle liability pool",
    )
    request_set = any(values["request_id"])
    credit_set = any(values["credit_id"]) and any(values["ciphertext_digest"])
    all_zero = not any(
        any(values[name])
        for name in ("request_id", "credit_id", "ciphertext_digest")
    )
    operation = values["operation_kind"]
    if (
        (operation == "send_split" and not (request_set and credit_set))
        or (
            operation == "mint_fold"
            and (any(values["request_id"]) or not credit_set)
        )
        or (operation not in ("send_split", "mint_fold") and not all_zero)
    ):
        _fail("Kagemusha V1 lifecycle operation identities are invalid")


def _validate_request_values(values: Mapping[str, object]) -> None:
    _header(values, positive_amount=True)
    _x25519(values["recipient_encryption_key"], "request recipient encryption key")
    if (
        values["expires_at_ms"] <= values["issued_at_ms"]
        or values["expires_at_ms"] - values["issued_at_ms"] > 300_000
    ):
        _fail("Kagemusha V1 request validity window is invalid")
    credential = values["hardware_credential"]
    if (
        _network_bytes(values["network_id"]) != _network_bytes(credential.network_id)
        or values["recipient_lane_id"] != credential.lane_commitment
        or values["issued_at_ms"] < credential.issued_at_ms
        or values["expires_at_ms"] > credential.expires_at_ms
    ):
        _fail("Kagemusha V1 request credential binding is invalid")


def _validate_peer_context_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    _validate_live_state_commitments(values, "peer credit context")
    _x25519(values["recipient_encryption_key"], "peer context recipient encryption key")
    if values["committed_at_ms"] == 0:
        _fail("Kagemusha V1 peer credit context commit time must be positive")


def _validate_transfer_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if (
        values["lifecycle"].version != values["version"]
        or values["lifecycle"].operation_kind != "send_split"
        or values["amount"] == 0
        or values["committed_at_ms"] == 0
    ):
        _fail("Kagemusha V1 transfer statement is invalid")
    _validate_live_state_commitments(values, "transfer")
    _x25519(values["recipient_encryption_key"], "transfer recipient encryption key")


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
        _fail("Kagemusha V1 mint statement is invalid")


def _validate_redemption_statement_values(values: Mapping[str, object]) -> None:
    _require_version(values["version"])
    if (
        values["lifecycle"].version != values["version"]
        or values["lifecycle"].operation_kind != "redeem_split"
        or values["amount"] == 0
        or values["committed_at_ms"] == 0
    ):
        _fail("Kagemusha V1 redemption statement is invalid")
    _validate_live_state_commitments(values, "redemption")


def _validate_top_up_values(values: Mapping[str, object]) -> None:
    _header(values, positive_amount=True)
    _x25519(values["recipient_one_time_key"], "top-up recipient key")


def _normalize_type(kind: object, value: object, context: str) -> object:
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
    if kind is _OPERATION_KIND:
        if value not in _OPERATION_KINDS:
            raise TypeError(f"{context} is not an Kagemusha V1 operation kind")
        return value
    if kind is _CREDIT_PURPOSE:
        if value not in _CREDIT_PURPOSES:
            raise TypeError(f"{context} is not an Kagemusha V1 credit purpose")
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
    if kind in (_U16, _U32, _U64, _U128):
        width = {_U16: 2, _U32: 4, _U64: 8, _U128: 16}[kind]
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
    if kind is _VECTOR:
        return _vector(value)
    if kind is _OPERATION_KIND:
        return _OPERATION_KINDS.index(value).to_bytes(4, "little")
    if kind is _CREDIT_PURPOSE:
        return _CREDIT_PURPOSES.index(value).to_bytes(4, "little")
    if kind is _OPTIONAL_MINT_AUTHORIZATION:
        return b"\0" if value is None else b"\x01" + _field(_encode_model(value))
    return _encode_model(value)


def _encode_model(value: object) -> bytes:
    try:
        definition = _DEFINITIONS[type(value)]
    except KeyError as error:
        raise TypeError("value is not an Kagemusha V1 model") from error
    return b"".join(
        _field(_encode_type(kind, getattr(value, name))) for name, kind in definition
    )


def _decode_unsigned(payload: bytes, width: int, context: str) -> int:
    if len(payload) != width:
        _fail(f"{context} must contain exactly {width} bytes")
    return int.from_bytes(payload, "little")


def _decode_type(kind: object, payload: bytes, context: str) -> object:
    if kind in (_U16, _U32, _U64, _U128):
        return _decode_unsigned(payload, {_U16: 2, _U32: 4, _U64: 8, _U128: 16}[kind], context)
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
    if kind is _VECTOR:
        if len(payload) < 8 or int.from_bytes(payload[:8], "little") != len(payload) - 8:
            _fail(f"{context} vector length is invalid")
        return payload[8:]
    if kind is _OPERATION_KIND:
        tag = _decode_unsigned(payload, 4, context)
        if tag >= len(_OPERATION_KINDS):
            _fail(f"{context} has an unknown operation tag")
        return _OPERATION_KINDS[tag]
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
    KagemushaPaymentV1: f"{_MODEL}KagemushaPaymentV1",
    KagemushaAcknowledgementV1: f"{_MODEL}KagemushaAcknowledgementV1",
    KagemushaMintAuthorizationV1: f"{_MODEL}KagemushaMintAuthorizationV1",
    KagemushaMintCreditV1: f"{_MODEL}KagemushaMintCreditV1",
    KagemushaRedemptionVoucherV1: f"{_MODEL}KagemushaRedemptionVoucherV1",
    KagemushaEncryptedCreditEnvelopeV1: f"{_MODEL}KagemushaEncryptedCreditEnvelopeV1",
    KagemushaEncryptedCreditAadV1: f"{_MODEL}KagemushaEncryptedCreditAadV1",
    KagemushaCreditOpeningV1: f"{_MODEL}KagemushaCreditOpeningV1",
    KagemushaTopUpRequestV1: "iroha.torii.v1.kagemusha.top_up.request",
    KagemushaRedemptionRequestV1: "iroha.torii.v1.kagemusha.redeem.request",
}

_SCHEMA_LIFECYCLE: Final = f"{_MODEL}KagemushaLifecycleBindingV1"
_SCHEMA_TRANSFER_STATEMENT: Final = f"{_MODEL}KagemushaTransferStatementV1"
_SCHEMA_MINT_CONTEXT: Final = f"{_MODEL}KagemushaMintAuthorizationContextV1"
_SCHEMA_MINT_AUTH_STATEMENT: Final = f"{_MODEL}KagemushaMintAuthorizationStatementV1"
_SCHEMA_MINT_STATEMENT: Final = f"{_MODEL}KagemushaMintCreditStatementV1"
_SCHEMA_REDEMPTION_STATEMENT: Final = f"{_MODEL}KagemushaRedemptionStatementV1"


def _model_alignment(model: type[Any]) -> int:
    if model in (KagemushaEncryptedCreditEnvelopeV1, KagemushaPeerCreditContextV1):
        return 8
    if model is KagemushaAcknowledgementV1:
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
        _fail(f"Kagemusha V1 {model.__name__} exceeds {maximum} bytes")
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
    "credit_id": b"iroha:kagemusha:v1:credit-id",
    "peer_credit_context": b"iroha:kagemusha:v1:peer-credit-context",
    "peer_credit_lifecycle_context": b"iroha:kagemusha:v1:peer-credit-lifecycle-context",
    "lifecycle": b"iroha:kagemusha:v1:lifecycle-binding",
    "outbox_reservation": b"iroha:kagemusha:v1:outbox-reservation",
    "statement": b"iroha:kagemusha:v1:send-split-statement",
    "payment": b"iroha:kagemusha:v1:payment",
    "ciphertext": b"iroha:kagemusha:v1:ciphertext",
    "mint_context": b"iroha:kagemusha:v1:mint-authorization-context",
    "mint_auth_statement": b"iroha:kagemusha:v1:mint-authorization-statement",
    "mint_auth": b"iroha:kagemusha:v1:mint-authorization",
    "mint_statement": b"iroha:kagemusha:v1:mint-statement",
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


def payment_request_signing_bytes(value: KagemushaPaymentRequestV1) -> bytes:
    if not isinstance(value, KagemushaPaymentRequestV1):
        raise TypeError("value must be KagemushaPaymentRequestV1")
    payload = b"".join(
        (
            _field(_vector(_DOMAIN["request_signing"])), _field(value.version.to_bytes(2, "little")),
            _field(value.release_id), _field(_network_bytes(value.network_id)),
            _field(value.asset.canonical_payload), _field(_encode_type(_INCARNATION, value.asset_incarnation)),
            _field(value.scale.to_bytes(4, "little")), _field(value.liability_pool_id),
            _field(value.recipient.canonical_payload), _field(value.recipient_lane_id),
            _field(value.recipient_encryption_key),
            _field(value.amount.to_bytes(16, "little")),
            _field(value.hardware_credential.credential_id), _field(value.request_id),
            _field(value.issued_at_ms.to_bytes(8, "little")),
            _field(value.expires_at_ms.to_bytes(8, "little")),
        )
    )
    return _frame("iroha.kagemusha.v1.payment-request-signing-preimage", payload)


def payment_request_digest(value: KagemushaPaymentRequestV1) -> bytes:
    _validate_request(value)
    return _digest_model(_DOMAIN["request_digest"], _SCHEMAS[KagemushaPaymentRequestV1], value)


def ciphertext_digest(value: object) -> bytes:
    return _digest_encoded(_DOMAIN["ciphertext"], _bytes(value, "encrypted credit"))


def credit_id(
    transition_nullifier: object,
    request_digest: object,
    sender_before_commitment: KagemushaPastaStateCommitmentV1,
    sender_after_commitment: KagemushaPastaStateCommitmentV1,
    recipient_lane_id: object,
    recipient_encryption_key: object,
    amount: int,
    ciphertext_commitment: object,
) -> bytes:
    payload = b"".join(
        (
            _field(_fixed32(transition_nullifier, "transition_nullifier")),
            _field(_fixed32(request_digest, "request_digest")),
            _field(_encode_model(sender_before_commitment)),
            _field(_encode_model(sender_after_commitment)),
            _field(_fixed32(recipient_lane_id, "recipient_lane_id")),
            _field(_fixed32(recipient_encryption_key, "recipient_encryption_key")),
            _field(_unsigned(amount, _MAX_U128, "amount").to_bytes(16, "little")),
            _field(_fixed32(ciphertext_commitment, "ciphertext_commitment")),
        )
    )
    return _digest_encoded(_DOMAIN["credit_id"], _frame("iroha.kagemusha.v1.credit-id-preimage", payload))


def mint_authorization_context_digest(value: KagemushaMintAuthorizationContextV1) -> bytes:
    return _digest_model(_DOMAIN["mint_context"], _SCHEMA_MINT_CONTEXT, value)


def mint_authorization_statement_digest(value: KagemushaMintAuthorizationStatementV1) -> bytes:
    return _digest_model(_DOMAIN["mint_auth_statement"], _SCHEMA_MINT_AUTH_STATEMENT, value)


def mint_authorization_digest(value: KagemushaMintAuthorizationV1) -> bytes:
    return _digest_model(_DOMAIN["mint_auth"], _SCHEMAS[KagemushaMintAuthorizationV1], value)


def _validate_envelope_recipient(recipient_key: object | None) -> None:
    if recipient_key is not None:
        _x25519(_raw32(recipient_key, "recipient X25519 key"), "recipient X25519 key")


def _lifecycle_digest(lifecycle: KagemushaLifecycleBindingV1) -> bytes:
    if not isinstance(lifecycle, KagemushaLifecycleBindingV1):
        raise TypeError("lifecycle must be KagemushaLifecycleBindingV1")
    return _digest_model(_DOMAIN["lifecycle"], _SCHEMA_LIFECYCLE, lifecycle, 1)


def _peer_lifecycle_context_digest(lifecycle: KagemushaLifecycleBindingV1) -> bytes:
    if not isinstance(lifecycle, KagemushaLifecycleBindingV1):
        raise TypeError("lifecycle must be KagemushaLifecycleBindingV1")
    payload = b"".join(
        (
            _field(_encode_type(_U16, lifecycle.version)),
            _field(_encode_type(_NETWORK, lifecycle.network_id)),
            _field(_encode_type(_U16, lifecycle.protocol_version)),
            _field(_encode_type(_FIXED32, lifecycle.suite_id)),
            _field(_encode_type(_FIXED32, lifecycle.vk_digest)),
            _field(_encode_type(_FIXED32, lifecycle.release_id)),
            _field(_encode_type(_ASSET, lifecycle.asset)),
            _field(_encode_type(_INCARNATION, lifecycle.asset_incarnation)),
            _field(_encode_type(_U32, lifecycle.scale)),
            _field(_encode_type(_FIXED32, lifecycle.liability_pool_id)),
            _field(_encode_type(_FIXED32, lifecycle.hardware_profile_id)),
            _field(_encode_type(_U64, lifecycle.policy_epoch)),
            _field(_encode_type(_OPERATION_KIND, lifecycle.operation_kind)),
            _field(_encode_type(_RAW32, lifecycle.request_id)),
        )
    )
    return _digest_encoded(
        _DOMAIN["peer_credit_lifecycle_context"],
        _frame("iroha.kagemusha.v1.peer-credit-lifecycle-context-preimage", payload, 8),
    )


def peer_credit_context(
    statement: KagemushaTransferStatementV1,
    request: KagemushaPaymentRequestV1,
) -> KagemushaPeerCreditContextV1:
    if not isinstance(statement, KagemushaTransferStatementV1):
        raise TypeError("statement must be KagemushaTransferStatementV1")
    if not isinstance(request, KagemushaPaymentRequestV1):
        raise TypeError("request must be KagemushaPaymentRequestV1")
    _same_bytes(statement.request_digest, payment_request_digest(request), "peer context request digest")
    _same_bytes(statement.recipient_lane_id, request.recipient_lane_id, "peer context recipient lane")
    _same_bytes(
        statement.recipient_encryption_key,
        request.recipient_encryption_key,
        "peer context recipient encryption key",
    )
    if (
        statement.amount != request.amount
        or statement.committed_at_ms < request.issued_at_ms
        or statement.committed_at_ms >= request.expires_at_ms
    ):
        _fail("Kagemusha V1 peer context does not match the request")
    return KagemushaPeerCreditContextV1(
        version=1,
        request_digest=statement.request_digest,
        sender_before_commitment=statement.sender_before_commitment,
        sender_after_commitment=statement.sender_after_commitment,
        lifecycle_context_digest=_peer_lifecycle_context_digest(statement.lifecycle),
        recipient_lane_id=statement.recipient_lane_id,
        recipient_encryption_key=statement.recipient_encryption_key,
        committed_at_ms=statement.committed_at_ms,
        hardware_transition_commitment=statement.hardware_transition_commitment,
    )


def outbox_reservation_commitment(value: KagemushaOutboxReservationV1) -> bytes:
    """Return the fixed-width commitment to one valid private outbox reservation."""
    if not isinstance(value, KagemushaOutboxReservationV1):
        raise TypeError("value must be KagemushaOutboxReservationV1")
    transcript = b"".join(
        (
            value.reservation_id,
            _OPERATION_KINDS.index(value.operation_kind).to_bytes(4, "little"),
            value.reserved_outbox_bytes.to_bytes(4, "little"),
            value.issued_at_ms.to_bytes(8, "little"),
            value.expires_at_ms.to_bytes(8, "little"),
        )
    )
    return _digest_encoded(_DOMAIN["outbox_reservation"], transcript)


def _validate_payment(payment: KagemushaPaymentV1, request: KagemushaPaymentRequestV1) -> None:
    statement = payment.statement
    for actual, expected, context in (
        (statement.request_digest, payment_request_digest(request), "payment request digest"),
        (statement.recipient_lane_id, request.recipient_lane_id, "payment recipient lane"),
        (statement.recipient_encryption_key, request.recipient_encryption_key, "payment recipient encryption key"),
        (statement.lifecycle.request_id, request.request_id, "payment lifecycle request ID"),
    ):
        _same_bytes(actual, expected, context)
    if (
        statement.amount != request.amount
        or statement.lifecycle.release_id != request.release_id
        or _network_bytes(statement.lifecycle.network_id) != _network_bytes(request.network_id)
        or statement.lifecycle.asset != request.asset
        or statement.lifecycle.asset_incarnation != request.asset_incarnation
        or statement.lifecycle.scale != request.scale
        or statement.lifecycle.liability_pool_id != request.liability_pool_id
        or statement.lifecycle.suite_id != request.hardware_credential.suite_id
        or statement.lifecycle.hardware_profile_id
        != request.hardware_credential.hardware_profile_id
        or statement.lifecycle.policy_epoch != request.hardware_credential.policy_epoch
        or statement.committed_at_ms < request.issued_at_ms
        or statement.committed_at_ms >= request.expires_at_ms
    ):
        _fail("payment does not match the request")
    decode_encrypted_credit_envelope(payment.encrypted_credit, statement.recipient_encryption_key)
    _same_bytes(statement.lifecycle.ciphertext_digest, ciphertext_digest(payment.encrypted_credit), "payment ciphertext digest")
    _same_bytes(
        statement.lifecycle.credit_id,
        credit_id(
            statement.transition_nullifier, statement.request_digest,
            statement.sender_before_commitment, statement.sender_after_commitment,
            statement.recipient_lane_id, statement.recipient_encryption_key,
            statement.amount, statement.ciphertext_commitment,
        ),
        "payment credit ID",
    )
    _same_bytes(
        payment.proof.semantic_digest,
        transfer_statement_digest(statement),
        "payment proof semantic digest",
    )


def transfer_statement_digest(statement: KagemushaTransferStatementV1) -> bytes:
    if not isinstance(statement, KagemushaTransferStatementV1):
        raise TypeError("statement must be KagemushaTransferStatementV1")
    return _digest_model(_DOMAIN["statement"], _SCHEMA_TRANSFER_STATEMENT, statement)


def payment_digest(payment: KagemushaPaymentV1, request: KagemushaPaymentRequestV1) -> bytes:
    _validate_payment(payment, request)
    return _digest_model(_DOMAIN["payment"], _SCHEMAS[KagemushaPaymentV1], payment)


def _validate_acknowledgement(
    acknowledgement: KagemushaAcknowledgementV1,
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
) -> None:
    _same_bytes(acknowledgement.request_digest, payment_request_digest(request), "acknowledgement request digest")
    _same_bytes(acknowledgement.payment_digest, payment_digest(payment, request), "acknowledgement payment digest")
    _same_bytes(acknowledgement.inbox_receipt.credit_id, payment.statement.lifecycle.credit_id, "acknowledgement credit ID")


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
        _digest_model(_DOMAIN["mint_statement"], _SCHEMA_MINT_STATEMENT, credit.statement),
        "mint proof semantic digest",
    )
    if authorization is not None:
        validate_mint_credit_against_authorization(credit, authorization)


def validate_mint_credit_against_authorization(
    credit: KagemushaMintCreditV1,
    authorization: KagemushaMintAuthorizationV1,
) -> bool:
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
    _same_bytes(
        voucher.proof.semantic_digest,
        redemption_statement_digest(statement),
        "redemption proof semantic digest",
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
    statement: KagemushaTransferStatementV1,
    request: KagemushaPaymentRequestV1,
) -> KagemushaEncryptedCreditAadV1:
    context = peer_credit_context(statement, request)
    return KagemushaEncryptedCreditAadV1(
        version=1,
        purpose="peer",
        context_digest=_digest_model(
            _DOMAIN["peer_credit_context"],
            _SCHEMAS[KagemushaPeerCreditContextV1],
            context,
            8,
        ),
        issuance_or_transition_commitment=statement.ciphertext_commitment,
        credit_id=statement.lifecycle.credit_id,
        amount=statement.amount,
    )


def encode_payment_request(value: KagemushaPaymentRequestV1) -> bytes:
    return _encode_top_level(value, KagemushaPaymentRequestV1, 1024, _validate_request)


def decode_payment_request(raw: object) -> KagemushaPaymentRequestV1:
    return _decode_top_level(raw, KagemushaPaymentRequestV1, 1024, _validate_request)


def encode_peer_credit_context(value: KagemushaPeerCreditContextV1) -> bytes:
    return _encode_top_level(value, KagemushaPeerCreditContextV1, 512)


def decode_peer_credit_context(raw: object) -> KagemushaPeerCreditContextV1:
    return _decode_top_level(raw, KagemushaPeerCreditContextV1, 512)


def encode_payment(value: KagemushaPaymentV1, request: KagemushaPaymentRequestV1) -> bytes:
    return _encode_top_level(value, KagemushaPaymentV1, 7936, lambda item: _validate_payment(item, request))


def decode_payment(raw: object, request: KagemushaPaymentRequestV1) -> KagemushaPaymentV1:
    return _decode_top_level(raw, KagemushaPaymentV1, 7936, lambda item: _validate_payment(item, request))


def encode_acknowledgement(
    value: KagemushaAcknowledgementV1,
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
) -> bytes:
    return _encode_top_level(
        value, KagemushaAcknowledgementV1, 512,
        lambda item: _validate_acknowledgement(item, request, payment),
    )


def decode_acknowledgement(
    raw: object, request: KagemushaPaymentRequestV1, payment: KagemushaPaymentV1
) -> KagemushaAcknowledgementV1:
    return _decode_top_level(
        raw, KagemushaAcknowledgementV1, 512,
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
        _fail("Kagemusha V1 credit opening has a noncanonical fixed size")
    return raw


def decode_credit_opening(
    raw: object, expected_credit_id: object | None = None, expected_amount: int | None = None
) -> KagemushaCreditOpeningV1:
    value = _decode_top_level(raw, KagemushaCreditOpeningV1, 256)
    if len(_bytes(raw, "credit opening")) != _CREDIT_OPENING_BYTES:
        _fail("Kagemusha V1 credit opening has a noncanonical fixed size")
    if expected_credit_id is not None:
        _same_bytes(value.credit_id, _fixed32(expected_credit_id, "credit_id"), "credit opening credit ID")
    if expected_amount is not None and value.amount != _unsigned(expected_amount, _MAX_U128, "amount"):
        _fail("credit opening amount does not match")
    return value


def _validate_top_up_request(value: KagemushaTopUpRequestV1) -> None:
    if value.mint_authorization is None:
        _fail("canonical Kagemusha V1 top-up requires mint authorization")
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
    return _encode_top_level(value, KagemushaTopUpRequestV1, 4096)


def decode_top_up_request(raw: object) -> KagemushaTopUpRequestV1:
    value = _decode_top_level(raw, KagemushaTopUpRequestV1, 4096)
    _validate_top_up_request(value)
    return value


def encode_redemption_request(value: KagemushaRedemptionRequestV1) -> bytes:
    return _encode_top_level(value, KagemushaRedemptionRequestV1, 8192)


def decode_redemption_request(raw: object) -> KagemushaRedemptionRequestV1:
    return _decode_top_level(raw, KagemushaRedemptionRequestV1, 8192)


_KIND_LIMITS: Final[Mapping[str, tuple[int, int]]] = {
    "payment_request": (1024, 1370),
    "payment": (7936, 10586),
    "acknowledgement": (512, 687),
    "mint_authorization": (7936, 10586),
    "mint_credit": (7936, 10586),
    "redemption_voucher": (7936, 10586),
    "encrypted_credit_envelope": (384, 516),
    "encrypted_credit_aad": (256, 346),
    "credit_opening": (256, 346),
}


def _kind_limits(kind: str) -> tuple[int, int]:
    try:
        return _KIND_LIMITS[kind]
    except (KeyError, TypeError) as error:
        raise KagemushaV1Error("unknown Kagemusha V1 payload kind") from error


def encode_text(kind: str, raw: object) -> str:
    maximum_raw, maximum_text = _kind_limits(kind)
    payload = _bytes(raw, "Kagemusha V1 payload")
    if not payload or len(payload) > maximum_raw:
        _fail("Kagemusha V1 payload is empty or oversized")
    text = "kgm1:" + base64.urlsafe_b64encode(payload).decode("ascii").rstrip("=")
    if len(text.encode("ascii")) > maximum_text:
        _fail("Kagemusha V1 text is oversized")
    return text


def decode_text(kind: str, text: str) -> bytes:
    maximum_raw, maximum_text = _kind_limits(kind)
    if type(text) is not str or not text.startswith("kgm1:") or len(text.encode("utf-8")) > maximum_text:
        _fail("Kagemusha V1 text prefix or size is invalid")
    body = text[len("kgm1:") :]
    if not body or len(body) % 4 == 1 or any(
        character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        for character in body
    ):
        _fail("Kagemusha V1 text is not canonical unpadded base64url")
    try:
        raw = base64.urlsafe_b64decode(body + "=" * (-len(body) % 4))
    except Exception as error:
        raise KagemushaV1Error("Kagemusha V1 text is not base64url") from error
    if len(raw) > maximum_raw or encode_text(kind, raw) != text:
        _fail("Kagemusha V1 text is noncanonical or oversized")
    return raw


_ENCODERS: Final = {
    "payment_request": encode_payment_request,
    "payment": encode_payment,
    "acknowledgement": encode_acknowledgement,
    "mint_authorization": encode_mint_authorization,
    "mint_credit": encode_mint_credit,
    "redemption_voucher": encode_redemption_voucher,
    "encrypted_credit_envelope": encode_encrypted_credit_envelope,
    "encrypted_credit_aad": encode_encrypted_credit_aad,
    "credit_opening": encode_credit_opening,
}
_DECODERS: Final = {
    "payment_request": decode_payment_request,
    "payment": decode_payment,
    "acknowledgement": decode_acknowledgement,
    "mint_authorization": decode_mint_authorization,
    "mint_credit": decode_mint_credit,
    "redemption_voucher": decode_redemption_voucher,
    "encrypted_credit_envelope": decode_encrypted_credit_envelope,
    "encrypted_credit_aad": decode_encrypted_credit_aad,
    "credit_opening": decode_credit_opening,
}


def encode_typed_text(kind: str, value: object, *bindings: object) -> str:
    try:
        raw = _ENCODERS[kind](value, *bindings)
    except KeyError as error:
        raise KagemushaV1Error("unknown Kagemusha V1 payload kind") from error
    return encode_text(kind, raw)


def decode_typed_text(kind: str, text: str, *bindings: object) -> object:
    try:
        decoder = _DECODERS[kind]
    except KeyError as error:
        raise KagemushaV1Error("unknown Kagemusha V1 payload kind") from error
    return decoder(decode_text(kind, text), *bindings)


def _text_length(raw_length: int) -> int:
    return 4 + (raw_length * 4 + 2) // 3


def validate_session(
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
    if raw_bytes > 9211 or text_bytes > 12288:
        _fail("Kagemusha V1 terminal trio is oversized")
    return raw_bytes


class KagemushaV1:
    """Sole public Kagemusha V1 codec/orchestration namespace."""

    wire_version: ClassVar[int] = 1
    device_lifecycle_version: ClassVar[int] = 1
    handoff_capability: ClassVar[str] = "kagemusha_handoff_v1"
    text_prefix: ClassVar[str] = "kgm1:"
    maximum_request_raw_bytes: ClassVar[int] = 1024
    maximum_request_text_bytes: ClassVar[int] = 1370
    maximum_session_raw_bytes: ClassVar[int] = 9211
    maximum_session_text_bytes: ClassVar[int] = 12288
    maximum_paired_proof_bytes: ClassVar[int] = 6528
    maximum_current_proofs_bytes: ClassVar[int] = 4990
    maximum_parity_proof_bytes: ClassVar[int] = 2495
    history_accumulator_bytes: ClassVar[int] = 544
    maximum_encrypted_credit_bytes: ClassVar[int] = 384
    maximum_credit_opening_bytes: ClassVar[int] = 256
    payment_outbox_minimum_bytes: ClassVar[int] = 26_112
    redemption_outbox_minimum_bytes: ClassVar[int] = 26_112
    maximum_top_up_request_bytes: ClassVar[int] = 4096
    maximum_redemption_request_bytes: ClassVar[int] = 8192
    maximum_operation_status_bytes: ClassVar[int] = 4 * 1024 * 1024
    maximum_operation_status_json_bytes: ClassVar[int] = 16 * 1024 * 1024
    payload_kinds: ClassVar[Mapping[str, tuple[int, int]]] = _KIND_LIMITS

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
    OutboxReservation = KagemushaOutboxReservationV1
    LifecycleBinding = KagemushaLifecycleBindingV1
    PaymentRequest = KagemushaPaymentRequestV1
    PeerCreditContext = KagemushaPeerCreditContextV1
    TransferStatement = KagemushaTransferStatementV1
    Payment = KagemushaPaymentV1
    InboxReceipt = KagemushaInboxReceiptV1
    Acknowledgement = KagemushaAcknowledgementV1
    MintAuthorizationContext = KagemushaMintAuthorizationContextV1
    MintAuthorizationStatement = KagemushaMintAuthorizationStatementV1
    MintAuthorization = KagemushaMintAuthorizationV1
    MintCreditStatement = KagemushaMintCreditStatementV1
    MintCredit = KagemushaMintCreditV1
    RedemptionStatement = KagemushaRedemptionStatementV1
    RedemptionVoucher = KagemushaRedemptionVoucherV1
    TopUpRequest = KagemushaTopUpRequestV1
    RedemptionRequest = KagemushaRedemptionRequestV1
    Error = KagemushaV1Error

    encode_payment_request = staticmethod(encode_payment_request)
    decode_payment_request = staticmethod(decode_payment_request)
    encode_peer_credit_context = staticmethod(encode_peer_credit_context)
    decode_peer_credit_context = staticmethod(decode_peer_credit_context)
    encode_payment = staticmethod(encode_payment)
    decode_payment = staticmethod(decode_payment)
    encode_acknowledgement = staticmethod(encode_acknowledgement)
    decode_acknowledgement = staticmethod(decode_acknowledgement)
    encode_mint_authorization = staticmethod(encode_mint_authorization)
    decode_mint_authorization = staticmethod(decode_mint_authorization)
    encode_mint_credit = staticmethod(encode_mint_credit)
    decode_mint_credit = staticmethod(decode_mint_credit)
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
    encode_redemption_request = staticmethod(encode_redemption_request)
    decode_redemption_request = staticmethod(decode_redemption_request)
    encode_text = staticmethod(encode_text)
    decode_text = staticmethod(decode_text)
    encode_typed_text = staticmethod(encode_typed_text)
    decode_typed_text = staticmethod(decode_typed_text)
    validate_session = staticmethod(validate_session)
    validate_mint_credit_against_authorization = staticmethod(validate_mint_credit_against_authorization)
    encrypted_credit_aad_for_mint = staticmethod(encrypted_credit_aad_for_mint)
    encrypted_credit_aad_for_peer = staticmethod(encrypted_credit_aad_for_peer)
    peer_credit_context = staticmethod(peer_credit_context)
    device_key_reference = staticmethod(device_key_reference)
    pasta_state_commitment = staticmethod(pasta_state_commitment)
    liability_pool_id = staticmethod(liability_pool_id)
    payment_request_signing_bytes = staticmethod(payment_request_signing_bytes)
    payment_request_digest = staticmethod(payment_request_digest)
    outbox_reservation_commitment = staticmethod(outbox_reservation_commitment)
    transfer_statement_digest = staticmethod(transfer_statement_digest)
    payment_digest = staticmethod(payment_digest)
    ciphertext_digest = staticmethod(ciphertext_digest)
    credit_id = staticmethod(credit_id)
    mint_authorization_context_digest = staticmethod(mint_authorization_context_digest)
    mint_authorization_statement_digest = staticmethod(mint_authorization_statement_digest)
    mint_authorization_digest = staticmethod(mint_authorization_digest)
    redemption_statement_digest = staticmethod(redemption_statement_digest)


__all__ = ["KagemushaV1"]
