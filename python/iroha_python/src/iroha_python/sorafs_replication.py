"""Canonical SoraFS replication-order instruction builders."""

from __future__ import annotations

import base64
import binascii
import hashlib
from dataclasses import dataclass
from typing import Any, Mapping, Union

from .address import AccountAddress, AccountAddressError, i105_discriminant_from_sentinel

SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1 = 1024 * 1024
_MAX_PAYLOAD_BASE64_CHARS_V1 = (
    4 * ((SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1 + 2) // 3)
)
_U64_MAX = (1 << 64) - 1
_HEX_DIGITS = frozenset("0123456789abcdef")
_REPLICATION_ORDER_SCHEMA = hashlib.sha256(
    b"norito:v1:type-name\0sorafs_manifest::capacity::ReplicationOrderV1"
).digest()[:16]
_CRC64_POLY = 0xC96C5795D7870F42
_U64_MASK = (1 << 64) - 1


def _crc64(payload: bytes) -> int:
    crc = _U64_MASK
    for byte in payload:
        crc ^= byte
        for _ in range(8):
            crc = ((crc >> 1) ^ _CRC64_POLY) if crc & 1 else crc >> 1
    return (crc ^ _U64_MASK) & _U64_MASK


class _NoritoReader:
    def __init__(self, payload: bytes, context: str):
        self.payload = payload
        self.context = context
        self.offset = 0

    def _read(self, length: int, field: str) -> bytes:
        if length < 0 or self.offset + length > len(self.payload):
            raise ValueError(f"{self.context}.{field} overruns the Norito payload")
        value = self.payload[self.offset : self.offset + length]
        self.offset += length
        return value

    def _compact_length(self, field: str) -> int:
        start = self.offset
        value = 0
        shift = 0
        while True:
            byte = self._read(1, f"{field}.length")[0]
            part = byte & 0x7F
            if shift == 63 and part > 1:
                raise ValueError(f"{self.context}.{field} length exceeds u64")
            value |= part << shift
            if byte & 0x80 == 0:
                break
            shift += 7
            if shift > 63:
                raise ValueError(f"{self.context}.{field} length exceeds u64")
        encoded = bytearray()
        remaining = value
        while True:
            chunk = remaining & 0x7F
            remaining >>= 7
            encoded.append(chunk | (0x80 if remaining else 0))
            if not remaining:
                break
        if self.payload[start : self.offset] != bytes(encoded):
            raise ValueError(f"{self.context}.{field} uses a noncanonical length")
        return value

    def field(self, name: str) -> bytes:
        return self._read(self._compact_length(name), name)

    def assert_eof(self) -> None:
        if self.offset != len(self.payload):
            raise ValueError(f"{self.context} contains trailing bytes")


def _u64(payload: bytes, field: str) -> int:
    if len(payload) != 8:
        raise ValueError(f"{field} must contain exactly eight bytes")
    return int.from_bytes(payload, "little")


def _validate_replication_order_payload(payload: bytes, expected_order_id: str) -> None:
    if len(payload) < 40:
        raise ValueError("order_payload is shorter than a Norito header")
    if payload[:6] != b"NRT0\0\0":
        raise ValueError("order_payload has an unsupported Norito magic or version")
    if payload[6:22] != _REPLICATION_ORDER_SCHEMA:
        raise ValueError("order_payload has the wrong ReplicationOrderV1 schema")
    if payload[22] != 0 or payload[39] != 0x02:
        raise ValueError("order_payload must use canonical uncompressed compact-length Norito")
    declared_length = int.from_bytes(payload[23:31], "little")
    if declared_length != len(payload) - 40:
        raise ValueError("order_payload must use canonical unpadded Norito framing")
    body = payload[40:]
    if int.from_bytes(payload[31:39], "little") != _crc64(body):
        raise ValueError("order_payload has an invalid Norito checksum")

    reader = _NoritoReader(body, "ReplicationOrderV1")
    fields = {
        name: reader.field(name)
        for name in (
            "version",
            "order_id",
            "manifest_cid",
            "manifest_digest",
            "chunking_profile",
            "target_replicas",
            "assignments",
            "issued_at",
            "deadline_at",
            "sla",
            "metadata",
        )
    }
    reader.assert_eof()
    if fields["version"] != b"\x01":
        raise ValueError("ReplicationOrderV1.version must be 1")
    order_id = fields["order_id"]
    if len(order_id) != 32 or not any(order_id):
        raise ValueError("ReplicationOrderV1.order_id must be a non-zero 32-byte value")
    if order_id.hex() != expected_order_id:
        raise ValueError(
            "IssueReplicationOrder.order_id must match ReplicationOrderV1.order_id"
        )
    if len(fields["target_replicas"]) != 2:
        raise ValueError("ReplicationOrderV1.target_replicas must be a u16")
    target_replicas = int.from_bytes(fields["target_replicas"], "little")
    if target_replicas == 0:
        raise ValueError("ReplicationOrderV1.target_replicas must be greater than zero")

    assignments = _NoritoReader(
        fields["assignments"],
        "ReplicationOrderV1.assignments",
    )
    count = int.from_bytes(assignments._read(8, "count"), "little")
    if count == 0 or count > 1024:
        raise ValueError("ReplicationOrderV1.assignments must contain 1..1024 entries")
    providers: list[bytes] = []
    for index in range(count):
        assignment = _NoritoReader(
            assignments.field(f"item[{index}]"),
            f"ReplicationOrderV1.assignments[{index}]",
        )
        provider = assignment.field("provider_id")
        slice_gib = assignment.field("slice_gib")
        assignment.field("lane")
        assignment.assert_eof()
        if len(provider) != 32 or not any(provider):
            raise ValueError(
                f"ReplicationOrderV1.assignments[{index}].provider_id must be non-zero"
            )
        if _u64(slice_gib, f"assignments[{index}].slice_gib") == 0:
            raise ValueError(
                f"ReplicationOrderV1.assignments[{index}].slice_gib must be positive"
            )
        providers.append(provider)
    assignments.assert_eof()
    if target_replicas > len(providers):
        raise ValueError(
            "ReplicationOrderV1.target_replicas must not exceed assignment count"
        )
    if any(previous >= current for previous, current in zip(providers, providers[1:])):
        raise ValueError(
            "ReplicationOrderV1 assignments must use unique, strictly increasing providers"
        )

    issued_at = _u64(fields["issued_at"], "ReplicationOrderV1.issued_at")
    deadline_at = _u64(fields["deadline_at"], "ReplicationOrderV1.deadline_at")
    if deadline_at <= issued_at:
        raise ValueError(
            "ReplicationOrderV1.deadline_at must be greater than issued_at"
        )


def _canonical_identifier(value: Any, field: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in _HEX_DIGITS for character in value)
    ):
        raise ValueError(
            f"{field} must contain exactly 64 lowercase hexadecimal characters"
        )
    if value == "0" * 64:
        raise ValueError(f"{field} must not be the zero identifier")
    return value


def _epoch(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{field} must be an integer")
    if value < 0 or value > _U64_MAX:
        raise ValueError(f"{field} must be a non-negative u64")
    return value


def _positive_u64(value: Any, field: str) -> int:
    value = _epoch(value, field)
    if value == 0:
        raise ValueError(f"{field} must be greater than zero")
    return value


def _canonical_account_id(value: Any, field: str) -> str:
    if not isinstance(value, str) or value != value.strip():
        raise ValueError(f"{field} must be an exact canonical I105 account id")
    if any(character.isspace() for character in value):
        raise ValueError(f"{field} must be an exact canonical I105 account id")
    try:
        discriminant = i105_discriminant_from_sentinel(value)
        if discriminant is None:
            raise AccountAddressError("missing canonical I105 sentinel")
        address = AccountAddress.parse_encoded(value, expected_discriminant=discriminant)
        if address.to_i105(discriminant) != value:
            raise AccountAddressError("noncanonical I105 spelling")
    except AccountAddressError as error:
        raise ValueError(
            f"{field} must be an exact canonical I105 account id"
        ) from error
    return value


def _canonical_order_payload(value: Any, expected_order_id: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError("order_payload must be non-empty canonical standard base64")
    if len(value) > _MAX_PAYLOAD_BASE64_CHARS_V1:
        raise ValueError(
            "order_payload encoded form exceeds the "
            f"{SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1}-byte decoded limit"
        )
    if value != value.strip() or any(character.isspace() for character in value):
        raise ValueError("order_payload must be non-empty canonical standard base64")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (binascii.Error, ValueError) as error:
        raise ValueError("order_payload must be canonical standard base64") from error
    if not decoded or base64.b64encode(decoded).decode("ascii") != value:
        raise ValueError("order_payload must be non-empty canonical standard base64")
    if len(decoded) > SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1:
        raise ValueError(
            "order_payload exceeds the "
            f"{SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1}-byte decoded limit"
        )

    _validate_replication_order_payload(decoded, expected_order_id)
    return value


def _exact_mapping(
    value: Any,
    expected_fields: frozenset[str],
    context: str,
) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be a mapping")
    fields = frozenset(value)
    if fields != expected_fields:
        raise ValueError(
            f"{context} must contain exactly {sorted(expected_fields)}; "
            f"found {sorted(str(field) for field in fields)}"
        )
    return value


@dataclass(frozen=True)
class IssueReplicationOrderInstruction:
    """Typed canonical `IssueReplicationOrder` instruction payload."""

    order_id: str
    order_payload: str
    issued_epoch: int
    deadline_epoch: int
    musubi_archive: str | None = None

    def __post_init__(self) -> None:
        order_id = _canonical_identifier(self.order_id, "order_id")
        issued_epoch = _epoch(self.issued_epoch, "issued_epoch")
        deadline_epoch = _epoch(self.deadline_epoch, "deadline_epoch")
        if deadline_epoch <= issued_epoch:
            raise ValueError("deadline_epoch must be greater than issued_epoch")
        if self.musubi_archive is not None:
            object.__setattr__(
                self,
                "musubi_archive",
                _canonical_identifier(self.musubi_archive, "musubi_archive"),
            )
        object.__setattr__(
            self,
            "order_payload",
            _canonical_order_payload(self.order_payload, order_id),
        )

    def to_payload(self) -> dict[str, dict[str, Any]]:
        """Return the schema-closed SDK JSON instruction representation."""

        return {
            "IssueReplicationOrder": {
                "order_id": self.order_id,
                "order_payload": self.order_payload,
                "issued_epoch": self.issued_epoch,
                "deadline_epoch": self.deadline_epoch,
                "musubi_archive": self.musubi_archive,
            }
        }

    def to_instruction(self) -> Any:
        """Create the SDK's native `Instruction` value."""

        from .crypto import _native_issue_replication_order

        return _native_issue_replication_order(
            self.order_id,
            self.order_payload,
            self.issued_epoch,
            self.deadline_epoch,
            self.musubi_archive,
        )

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> IssueReplicationOrderInstruction:
        """Decode a schema-closed canonical JSON instruction mapping."""

        outer = _exact_mapping(
            payload,
            frozenset({"IssueReplicationOrder"}),
            "instruction",
        )
        body = _exact_mapping(
            outer["IssueReplicationOrder"],
            frozenset(
                {
                    "order_id",
                    "order_payload",
                    "issued_epoch",
                    "deadline_epoch",
                    "musubi_archive",
                }
            ),
            "IssueReplicationOrder",
        )
        return cls(
            order_id=body["order_id"],
            order_payload=body["order_payload"],
            issued_epoch=body["issued_epoch"],
            deadline_epoch=body["deadline_epoch"],
            musubi_archive=body["musubi_archive"],
        )


@dataclass(frozen=True)
class ProviderIngestCompletionSignerPolicyV1:
    """Exact governed signer-policy identity expected at completion commit."""

    policy_id: str
    revision: int
    predecessor_digest: str | None
    policy_digest: str

    def __post_init__(self) -> None:
        object.__setattr__(
            self,
            "policy_id",
            _canonical_identifier(self.policy_id, "policy_id"),
        )
        revision = _positive_u64(self.revision, "revision")
        object.__setattr__(self, "revision", revision)
        if revision == 1:
            if self.predecessor_digest is not None:
                raise ValueError("predecessor_digest must be absent at revision one")
        elif self.predecessor_digest is None:
            raise ValueError("predecessor_digest is required after revision one")
        else:
            object.__setattr__(
                self,
                "predecessor_digest",
                _canonical_identifier(
                    self.predecessor_digest,
                    "predecessor_digest",
                ),
            )
        object.__setattr__(
            self,
            "policy_digest",
            _canonical_identifier(self.policy_digest, "policy_digest"),
        )

    def to_payload(self) -> dict[str, Any]:
        """Return the exact signer-policy mapping."""

        return {
            "policy_id": self.policy_id,
            "revision": self.revision,
            "predecessor_digest": self.predecessor_digest,
            "policy_digest": self.policy_digest,
        }

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
    ) -> ProviderIngestCompletionSignerPolicyV1:
        """Decode one schema-closed signer-policy mapping."""

        body = _exact_mapping(
            payload,
            frozenset(
                {
                    "policy_id",
                    "revision",
                    "predecessor_digest",
                    "policy_digest",
                }
            ),
            "ProviderIngestCompletionSignerPolicyV1",
        )
        return cls(
            policy_id=body["policy_id"],
            revision=body["revision"],
            predecessor_digest=body["predecessor_digest"],
            policy_digest=body["policy_digest"],
        )


@dataclass(frozen=True)
class ProviderIngestCompletionAuthorityV1:
    """Exact provider owner and governed signer policy expected at commit."""

    provider_owner: str
    signer_policy: ProviderIngestCompletionSignerPolicyV1

    def __post_init__(self) -> None:
        object.__setattr__(
            self,
            "provider_owner",
            _canonical_account_id(self.provider_owner, "provider_owner"),
        )
        if not isinstance(
            self.signer_policy,
            ProviderIngestCompletionSignerPolicyV1,
        ):
            raise TypeError(
                "signer_policy must be ProviderIngestCompletionSignerPolicyV1"
            )

    def to_payload(self) -> dict[str, Any]:
        """Return the exact completion-authority mapping."""

        return {
            "provider_owner": self.provider_owner,
            "signer_policy": self.signer_policy.to_payload(),
        }

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
    ) -> ProviderIngestCompletionAuthorityV1:
        """Decode one schema-closed completion-authority mapping."""

        body = _exact_mapping(
            payload,
            frozenset({"provider_owner", "signer_policy"}),
            "ProviderIngestCompletionAuthorityV1",
        )
        return cls(
            provider_owner=body["provider_owner"],
            signer_policy=ProviderIngestCompletionSignerPolicyV1.from_payload(
                body["signer_policy"]
            ),
        )


@dataclass(frozen=True)
class ProviderIngestFinalizedAnchorV1:
    """One finalized block prefix used to prepare a completion."""

    height: int
    block_hash: str

    def __post_init__(self) -> None:
        object.__setattr__(self, "height", _positive_u64(self.height, "height"))
        object.__setattr__(
            self,
            "block_hash",
            _canonical_identifier(self.block_hash, "block_hash"),
        )

    def to_payload(self) -> dict[str, Any]:
        """Return the exact finalized-anchor mapping."""

        return {"height": self.height, "block_hash": self.block_hash}

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
    ) -> ProviderIngestFinalizedAnchorV1:
        """Decode one schema-closed finalized-anchor mapping."""

        body = _exact_mapping(
            payload,
            frozenset({"height", "block_hash"}),
            "ProviderIngestFinalizedAnchorV1",
        )
        return cls(height=body["height"], block_hash=body["block_hash"])


@dataclass(frozen=True)
class CompleteReplicationOrderInstruction:
    """Typed provider-specific `CompleteReplicationOrder` instruction payload."""

    order_id: str
    provider_id: str
    completion_epoch: int
    expected_authority: ProviderIngestCompletionAuthorityV1
    expected_assignment_revision: int
    finalized_anchor: ProviderIngestFinalizedAnchorV1

    def __post_init__(self) -> None:
        object.__setattr__(
            self,
            "order_id",
            _canonical_identifier(self.order_id, "order_id"),
        )
        object.__setattr__(
            self,
            "provider_id",
            _canonical_identifier(self.provider_id, "provider_id"),
        )
        object.__setattr__(
            self,
            "completion_epoch",
            _epoch(self.completion_epoch, "completion_epoch"),
        )
        if not isinstance(
            self.expected_authority,
            ProviderIngestCompletionAuthorityV1,
        ):
            raise TypeError(
                "expected_authority must be ProviderIngestCompletionAuthorityV1"
            )
        object.__setattr__(
            self,
            "expected_assignment_revision",
            _positive_u64(
                self.expected_assignment_revision,
                "expected_assignment_revision",
            ),
        )
        if not isinstance(self.finalized_anchor, ProviderIngestFinalizedAnchorV1):
            raise TypeError(
                "finalized_anchor must be ProviderIngestFinalizedAnchorV1"
            )

    def to_payload(self) -> dict[str, dict[str, Any]]:
        """Return the exact six-field Rust/Norito JSON representation."""

        return {
            "CompleteReplicationOrder": {
                "order_id": self.order_id,
                "provider_id": self.provider_id,
                "completion_epoch": self.completion_epoch,
                "expected_authority": self.expected_authority.to_payload(),
                "expected_assignment_revision": self.expected_assignment_revision,
                "finalized_anchor": self.finalized_anchor.to_payload(),
            }
        }

    def to_instruction(self) -> Any:
        """Create the SDK's native `Instruction` value."""

        from .crypto import _native_complete_replication_order

        return _native_complete_replication_order(
            self.order_id,
            self.provider_id,
            self.completion_epoch,
            self.expected_authority.to_payload(),
            self.expected_assignment_revision,
            self.finalized_anchor.to_payload(),
        )

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
    ) -> CompleteReplicationOrderInstruction:
        """Decode a schema-closed provider-specific completion mapping."""

        outer = _exact_mapping(
            payload,
            frozenset({"CompleteReplicationOrder"}),
            "instruction",
        )
        body = _exact_mapping(
            outer["CompleteReplicationOrder"],
            frozenset(
                {
                    "order_id",
                    "provider_id",
                    "completion_epoch",
                    "expected_authority",
                    "expected_assignment_revision",
                    "finalized_anchor",
                }
            ),
            "CompleteReplicationOrder",
        )
        return cls(
            order_id=body["order_id"],
            provider_id=body["provider_id"],
            completion_epoch=body["completion_epoch"],
            expected_authority=ProviderIngestCompletionAuthorityV1.from_payload(
                body["expected_authority"]
            ),
            expected_assignment_revision=body["expected_assignment_revision"],
            finalized_anchor=ProviderIngestFinalizedAnchorV1.from_payload(
                body["finalized_anchor"]
            ),
        )


@dataclass(frozen=True)
class ExpireReplicationOrderInstruction:
    """Typed canonical `ExpireReplicationOrder` instruction payload."""

    order_id: str
    expiration_epoch: int

    def __post_init__(self) -> None:
        object.__setattr__(
            self,
            "order_id",
            _canonical_identifier(self.order_id, "order_id"),
        )
        object.__setattr__(
            self,
            "expiration_epoch",
            _epoch(self.expiration_epoch, "expiration_epoch"),
        )

    def to_payload(self) -> dict[str, dict[str, Any]]:
        """Return the exact Rust/Norito JSON instruction representation."""

        return {
            "ExpireReplicationOrder": {
                "order_id": self.order_id,
                "expiration_epoch": self.expiration_epoch,
            }
        }

    def to_instruction(self) -> Any:
        """Create the SDK's native `Instruction` value."""

        from .crypto import _native_expire_replication_order

        return _native_expire_replication_order(
            self.order_id,
            self.expiration_epoch,
        )

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
    ) -> ExpireReplicationOrderInstruction:
        """Decode a schema-closed expiration mapping."""

        outer = _exact_mapping(
            payload,
            frozenset({"ExpireReplicationOrder"}),
            "instruction",
        )
        body = _exact_mapping(
            outer["ExpireReplicationOrder"],
            frozenset({"order_id", "expiration_epoch"}),
            "ExpireReplicationOrder",
        )
        return cls(
            order_id=body["order_id"],
            expiration_epoch=body["expiration_epoch"],
        )


ReplicationOrderInstruction = Union[
    IssueReplicationOrderInstruction,
    CompleteReplicationOrderInstruction,
    ExpireReplicationOrderInstruction,
]


def decode_replication_order_instruction(
    payload: Mapping[str, Any],
) -> ReplicationOrderInstruction:
    """Decode one schema-closed replication-order instruction mapping."""

    if not isinstance(payload, Mapping):
        raise TypeError("instruction must be a mapping")
    if frozenset(payload) == {"IssueReplicationOrder"}:
        return IssueReplicationOrderInstruction.from_payload(payload)
    if frozenset(payload) == {"CompleteReplicationOrder"}:
        return CompleteReplicationOrderInstruction.from_payload(payload)
    if frozenset(payload) == {"ExpireReplicationOrder"}:
        return ExpireReplicationOrderInstruction.from_payload(payload)
    raise ValueError(
        "instruction must contain exactly one IssueReplicationOrder, "
        "CompleteReplicationOrder, or ExpireReplicationOrder field"
    )


def build_issue_replication_order_instruction(
    order_id: str,
    order_payload: str,
    issued_epoch: int,
    deadline_epoch: int,
    musubi_archive: str | None = None,
) -> Any:
    """Build a native `IssueReplicationOrder` instruction."""

    return IssueReplicationOrderInstruction(
        order_id,
        order_payload,
        issued_epoch,
        deadline_epoch,
        musubi_archive,
    ).to_instruction()


def build_complete_replication_order_instruction(
    order_id: str,
    provider_id: str,
    completion_epoch: int,
    expected_authority: ProviderIngestCompletionAuthorityV1,
    expected_assignment_revision: int,
    finalized_anchor: ProviderIngestFinalizedAnchorV1,
) -> Any:
    """Build the native six-field provider-specific completion instruction."""

    return CompleteReplicationOrderInstruction(
        order_id,
        provider_id,
        completion_epoch,
        expected_authority,
        expected_assignment_revision,
        finalized_anchor,
    ).to_instruction()


def build_expire_replication_order_instruction(
    order_id: str,
    expiration_epoch: int,
) -> Any:
    """Build a native `ExpireReplicationOrder` instruction."""

    return ExpireReplicationOrderInstruction(
        order_id,
        expiration_epoch,
    ).to_instruction()


__all__ = [
    "CompleteReplicationOrderInstruction",
    "ExpireReplicationOrderInstruction",
    "IssueReplicationOrderInstruction",
    "ProviderIngestCompletionAuthorityV1",
    "ProviderIngestCompletionSignerPolicyV1",
    "ProviderIngestFinalizedAnchorV1",
    "ReplicationOrderInstruction",
    "SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1",
    "build_complete_replication_order_instruction",
    "build_expire_replication_order_instruction",
    "build_issue_replication_order_instruction",
    "decode_replication_order_instruction",
]
