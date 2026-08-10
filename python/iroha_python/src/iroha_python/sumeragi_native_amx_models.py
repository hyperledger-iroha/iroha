"""Strict public models for signed Native AMX v2 attestation identities."""

from __future__ import annotations

import re
from dataclasses import dataclass
from enum import Enum
from typing import Any, Iterable, Mapping, NewType, Optional, Sequence, Tuple

from iroha_torii_client.client import SumeragiV2Round


class SumeragiNativeAmxPhase(str, Enum):
    """Native AMX participant phase carried by an attestation QC."""

    PREPARE = "prepare"
    COMMIT = "commit"


# These domains deliberately remain separate in the public type surface even
# though both are represented by JSON strings. Their constructors are used
# only after the incompatible wire grammars have been validated below.
SumeragiNativeAmxSourceId = NewType("SumeragiNativeAmxSourceId", str)
SumeragiNativeAmxTransactionEntrypointHash = NewType(
    "SumeragiNativeAmxTransactionEntrypointHash", str
)


def _required_field(payload: Mapping[str, Any], field_name: str, context: str) -> Any:
    if field_name not in payload:
        raise TypeError(f"{context} is missing required `{field_name}` field")
    return payload[field_name]


def _strict_exact_fields(payload: Mapping[str, Any], fields: Iterable[str], context: str) -> None:
    expected = set(fields)
    unknown = sorted(set(payload).difference(expected))
    if unknown:
        raise ValueError(f"{context} contains unknown field `{unknown[0]}`")
    missing = sorted(expected.difference(payload))
    if missing:
        raise TypeError(f"{context} is missing required `{missing[0]}` field")


def _strict_uint(payload: Mapping[str, Any], field_name: str, bits: int, context: str) -> int:
    value = _required_field(payload, field_name, context)
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{context} `{field_name}` must be an unsigned integer")
    maximum = (1 << bits) - 1
    if value < 0 or value > maximum:
        raise ValueError(f"{context} `{field_name}` must be between 0 and {maximum}")
    return value


def _strict_tagged_unit_enum(
    payload: Mapping[str, Any],
    field_name: str,
    *,
    tag: str,
    content: str,
    variants: Sequence[str],
    context: str,
) -> str:
    value = _required_field(payload, field_name, context)
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} `{field_name}` must be a tagged enum object")
    if set(value) != {tag, content}:
        raise ValueError(f"{context} `{field_name}` must contain exactly `{tag}` and `{content}`")
    variant = value[tag]
    if not isinstance(variant, str) or variant not in variants:
        raise ValueError(f"{context} `{field_name}` contains an unsupported variant")
    if value[content] is not None:
        raise ValueError(f"{context} `{field_name}.{content}` must be null")
    return variant


def _strict_hex_string(
    payload: Mapping[str, Any],
    field_name: str,
    byte_length: int,
    context: str,
) -> str:
    value = _required_field(payload, field_name, context)
    if (
        not isinstance(value, str)
        or len(value) != byte_length * 2
        or re.fullmatch(r"[0-9A-F]+", value) is None
    ):
        raise TypeError(
            f"{context} `{field_name}` must be exactly {byte_length} bytes of uppercase hex"
        )
    return value


def _crc16_ccitt_false(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def _strict_hash_literal(payload: Mapping[str, Any], field_name: str, context: str) -> str:
    value = _required_field(payload, field_name, context)
    if not isinstance(value, str):
        raise TypeError(f"{context} `{field_name}` must be a canonical hash literal")
    match = re.fullmatch(r"hash:([0-9A-F]{64})#([0-9A-F]{4})", value)
    if match is None:
        raise ValueError(
            f"{context} `{field_name}` must use canonical `hash:<uppercase hex>#<CRC16>` syntax"
        )
    body, checksum = match.groups()
    expected = _crc16_ccitt_false(f"hash:{body}".encode("ascii"))
    if int(checksum, 16) != expected:
        raise ValueError(f"{context} `{field_name}` hash checksum mismatch")
    if int(body[-2:], 16) & 1 == 0:
        raise ValueError(f"{context} `{field_name}` has an invalid Iroha hash marker bit")
    return value


@dataclass(frozen=True)
class SumeragiNativeAmxAttestationBody:
    """Context-bound v2 identity signed by a native AMX participant committee."""

    round: SumeragiV2Round
    epoch: int
    network_id: str
    source_id: SumeragiNativeAmxSourceId
    tx_entrypoint_hash: SumeragiNativeAmxTransactionEntrypointHash
    plan_digest: str
    phase: SumeragiNativeAmxPhase
    coordinator_lane_id: int
    coordinator_dataspace_id: int
    coordinator_lane_incarnation: str
    participant_lane_id: int
    participant_dataspace_id: int
    participant_lane_incarnation: str
    participant_previous_block_height: int
    participant_previous_block_descriptor_hash: Optional[str]
    participant_lane_block_height: int
    participant_lane_block_view: int
    participant_proposal_hash: str
    participant_settlement_commitment: str
    participant_validator_set_hash: str
    participant_validator_count: int
    participant_min_quorum: int
    authority_context_height: int
    planned_coordinator_block_height: int
    coordinator_lane_block_view: int
    coordinator_proposal_hash: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiNativeAmxAttestationBody":
        context = "native AMX v2 attestation body"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        expected_fields = {
            "round",
            "epoch",
            "network_id",
            "source_id",
            "tx_entrypoint_hash",
            "plan_digest",
            "phase",
            "coordinator_lane_id",
            "coordinator_dataspace_id",
            "coordinator_lane_incarnation",
            "participant_lane_id",
            "participant_dataspace_id",
            "participant_lane_incarnation",
            "participant_previous_block_height",
            "participant_previous_block_descriptor_hash",
            "participant_lane_block_height",
            "participant_lane_block_view",
            "participant_proposal_hash",
            "participant_settlement_commitment",
            "participant_validator_set_hash",
            "participant_validator_count",
            "participant_min_quorum",
            "authority_context_height",
            "planned_coordinator_block_height",
            "coordinator_lane_block_view",
            "coordinator_proposal_hash",
        }
        _strict_exact_fields(payload, expected_fields, context)

        round_payload = _required_field(payload, "round", context)
        if not isinstance(round_payload, Mapping) or set(round_payload) != {
            "context_id",
            "height",
            "view",
        }:
            raise TypeError(f"{context} `round` must be an exact v2 round object")
        context_id_payload = _required_field(round_payload, "context_id", f"{context} round")
        if not isinstance(context_id_payload, list) or len(context_id_payload) != 1:
            raise TypeError(f"{context} round context id must be a one-element hash tuple")
        round_value = SumeragiV2Round(
            context_id=(
                _strict_hash_literal(
                    {"context_id": context_id_payload[0]},
                    "context_id",
                    f"{context} round",
                ),
            ),
            height=_strict_uint(round_payload, "height", 64, f"{context} round"),
            view=_strict_uint(round_payload, "view", 64, f"{context} round"),
        )
        phase_value = _strict_tagged_unit_enum(
            payload,
            "phase",
            tag="phase",
            content="detail",
            variants=("prepare", "commit"),
            context=context,
        )
        phase = SumeragiNativeAmxPhase(phase_value)
        validator_count = _strict_uint(payload, "participant_validator_count", 32, context)
        min_quorum = _strict_uint(payload, "participant_min_quorum", 32, context)
        expected_quorum = validator_count - (validator_count - 1) // 3 if validator_count else 0
        authority_context_height = _strict_uint(payload, "authority_context_height", 64, context)
        planned_height = _strict_uint(payload, "planned_coordinator_block_height", 64, context)
        coordinator_view = _strict_uint(payload, "coordinator_lane_block_view", 64, context)
        participant_previous_height = _strict_uint(
            payload, "participant_previous_block_height", 64, context
        )
        participant_height = _strict_uint(payload, "participant_lane_block_height", 64, context)
        participant_view = _strict_uint(payload, "participant_lane_block_view", 64, context)
        previous_descriptor_value = _required_field(
            payload, "participant_previous_block_descriptor_hash", context
        )
        if previous_descriptor_value is None:
            previous_descriptor_hash: Optional[str] = None
        else:
            previous_descriptor_hash = _strict_hash_literal(
                {"participant_previous_block_descriptor_hash": previous_descriptor_value},
                "participant_previous_block_descriptor_hash",
                context,
            )
        source_id = SumeragiNativeAmxSourceId(
            _strict_hex_string(payload, "source_id", 32, context)
        )
        entrypoint_hash = SumeragiNativeAmxTransactionEntrypointHash(
            _strict_hash_literal(payload, "tx_entrypoint_hash", context)
        )
        if (
            round_value.height == 0
            or authority_context_height != round_value.height
            or planned_height == 0
            or participant_height == 0
            or participant_previous_height + 1 != participant_height
            or (participant_previous_height == 0) != (previous_descriptor_hash is None)
            or validator_count == 0
            or validator_count > 128
            or min_quorum != expected_quorum
        ):
            raise ValueError(f"{context} contains inconsistent round or quorum fields")
        return cls(
            round=round_value,
            epoch=_strict_uint(payload, "epoch", 64, context),
            network_id=_strict_hash_literal(payload, "network_id", context),
            source_id=source_id,
            tx_entrypoint_hash=entrypoint_hash,
            plan_digest=_strict_hash_literal(payload, "plan_digest", context),
            phase=phase,
            coordinator_lane_id=_strict_uint(payload, "coordinator_lane_id", 32, context),
            coordinator_dataspace_id=_strict_uint(payload, "coordinator_dataspace_id", 64, context),
            coordinator_lane_incarnation=_strict_hash_literal(
                payload, "coordinator_lane_incarnation", context
            ),
            participant_lane_id=_strict_uint(payload, "participant_lane_id", 32, context),
            participant_dataspace_id=_strict_uint(payload, "participant_dataspace_id", 64, context),
            participant_lane_incarnation=_strict_hash_literal(
                payload, "participant_lane_incarnation", context
            ),
            participant_previous_block_height=participant_previous_height,
            participant_previous_block_descriptor_hash=previous_descriptor_hash,
            participant_lane_block_height=participant_height,
            participant_lane_block_view=participant_view,
            participant_proposal_hash=_strict_hash_literal(
                payload, "participant_proposal_hash", context
            ),
            participant_settlement_commitment=_strict_hash_literal(
                payload, "participant_settlement_commitment", context
            ),
            participant_validator_set_hash=_strict_hash_literal(
                payload, "participant_validator_set_hash", context
            ),
            participant_validator_count=validator_count,
            participant_min_quorum=min_quorum,
            authority_context_height=authority_context_height,
            planned_coordinator_block_height=planned_height,
            coordinator_lane_block_view=coordinator_view,
            coordinator_proposal_hash=_strict_hash_literal(
                payload, "coordinator_proposal_hash", context
            ),
        )

    def identity(self) -> Tuple[Any, ...]:
        """Return all signed identity fields except the prepare/commit phase."""

        return (
            self.round,
            self.epoch,
            self.network_id,
            self.source_id,
            self.tx_entrypoint_hash,
            self.plan_digest,
            self.coordinator_lane_id,
            self.coordinator_dataspace_id,
            self.coordinator_lane_incarnation,
            self.participant_lane_id,
            self.participant_dataspace_id,
            self.participant_lane_incarnation,
            self.participant_previous_block_height,
            self.participant_previous_block_descriptor_hash,
            self.participant_lane_block_height,
            self.participant_lane_block_view,
            self.participant_proposal_hash,
            self.participant_settlement_commitment,
            self.participant_validator_set_hash,
            self.participant_validator_count,
            self.participant_min_quorum,
            self.authority_context_height,
            self.planned_coordinator_block_height,
            self.coordinator_lane_block_view,
            self.coordinator_proposal_hash,
        )


__all__ = [
    "SumeragiNativeAmxAttestationBody",
    "SumeragiNativeAmxPhase",
    "SumeragiNativeAmxSourceId",
    "SumeragiNativeAmxTransactionEntrypointHash",
]
