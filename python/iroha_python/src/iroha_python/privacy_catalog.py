"""Retired JSON capability-payload inspection; never live Exact12 admission."""

from __future__ import annotations

import json
from typing import Any, Literal, TypedDict, cast

LEGACY_PRIVACY_CAPABILITY_INSPECTION_VERSION_V1 = 1
LEGACY_PRIVACY_CAPABILITY_INSPECTION_MAX_JSON_BYTES_V1 = 256 * 1024

PrivacyProtocolIdV1 = Literal[
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
]

PRIVACY_PROTOCOL_IDS_V1: tuple[PrivacyProtocolIdV1, ...] = (
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
)


PrivacyProofSystemIdV1 = Literal[
    "stark-fri-sha256-goldilocks",
    "anonymous-pgc-p256",
    "iroha-verange-p256",
    "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
    "vega-neutron-nova-spartan-hyrax-t256",
    "jindo-polynomial-commitment",
    "lantern-lnp22-module-linear-norm",
    "halo2-ipa-pasta",
    "fcmp-plus-plus-curve-tree-bulletproofs",
]

PrivacyEngineIdV1 = Literal[
    "native-goldilocks-stark-fri",
    "native-anonymous-pgc-p256",
    "native-verange-p256",
    "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
    "native-vega",
    "native-jindo",
    "native-lantern-lnp22",
    "native-halo2-orchard",
    "native-fcmp-plus-plus",
]


class PrivacyProtocolTagV1(TypedDict):
    """Canonical unit-valued protocol discriminant."""

    protocol: PrivacyProtocolIdV1
    value: None


class PrivacyProofSystemTagV1(TypedDict):
    """Canonical unit-valued proof-system discriminant."""

    proof_system: PrivacyProofSystemIdV1
    value: None


class PrivacyEngineTagV1(TypedDict):
    """Canonical unit-valued native-engine discriminant."""

    engine: PrivacyEngineIdV1
    value: None


class PrivacyProtocolLimitsV1(TypedDict):
    """Protocol-specific limits tagged by their exact protocol."""

    protocol: PrivacyProtocolIdV1
    limits: dict[str, int] | None


class PrivacyCompiledProfileValueV1(TypedDict):
    """One native compiled-profile binding and its hard ceilings."""

    protocol_id: PrivacyProtocolTagV1
    proof_system_id: PrivacyProofSystemTagV1
    engine_id: PrivacyEngineTagV1
    parameter_id: list[int]
    parameter_digest: list[int]
    verifier_digest: list[int]
    statement_schema_digest: list[int]
    engine_manifest_digest: list[int]
    protocol_limits: PrivacyProtocolLimitsV1


class PrivacyStatementSchemaErrorV1(TypedDict):
    """Closed statement-schema initialization failure detail."""

    schema_error: Literal[
        "conflicting-stable-type-id",
        "missing-type-reference",
    ]
    detail: None


class PrivacyCompiledProfileUnavailableReasonV1(TypedDict):
    """Fail-closed reason why a native compiled profile is unavailable."""

    reason: Literal[
        "engine-unavailable",
        "profile-initialization-failed",
        "statement-schema-invalid",
    ]
    detail: PrivacyStatementSchemaErrorV1 | None


class PrivacyCompiledProfileAvailableV1(TypedDict):
    """Available native profile variant."""

    status: Literal["available"]
    value: PrivacyCompiledProfileValueV1


class PrivacyCompiledProfileUnavailableV1(TypedDict):
    """Unavailable native profile variant."""

    status: Literal["unavailable"]
    value: PrivacyCompiledProfileUnavailableReasonV1


PrivacyCompiledProfileV1 = PrivacyCompiledProfileAvailableV1 | PrivacyCompiledProfileUnavailableV1


class PrivacyLifecycleProposedRecordV1(TypedDict):
    """Historical JSON representation of a proposed activation schedule."""

    proposed_at_height: int
    activate_at_height: int


class PrivacyLifecycleEstablishedRecordV1(TypedDict):
    """Historical JSON representation of established lifecycle heights."""

    proposed_at_height: int
    activated_at_height: int | None
    state_since_height: int


class PrivacyLifecycleV1(TypedDict):
    """Inspection-only lifecycle state from a retired JSON payload."""

    state: Literal["proposed", "active", "suspended", "retired"]
    record: PrivacyLifecycleProposedRecordV1 | PrivacyLifecycleEstablishedRecordV1


class PrivacyProtocolLimitsTighteningV1(TypedDict):
    """Inspection-only JSON representation of protocol-limit tightening."""

    scheduled_at_height: int
    effective_at_height: int
    next_limits: PrivacyProtocolLimitsV1


class PrivacyAssuranceTagV1(TypedDict):
    """Closed first-release assurance classification."""

    assurance: Literal["experimental"]
    value: None


class PrivacyActivationV1(PrivacyCompiledProfileValueV1):
    """Inspection-only historical activation bound to its compiled profile."""

    lifecycle: PrivacyLifecycleV1
    pending_protocol_limits_tightening: PrivacyProtocolLimitsTighteningV1 | None
    assurance: PrivacyAssuranceTagV1


class PrivacyConsensusLimitsV1(TypedDict):
    """Historical JSON representation of first-release resource limits."""

    max_actions_per_transaction: int
    max_actions_per_block: int
    max_proof_bytes_per_action: int
    max_action_bytes: int
    max_privacy_bytes_per_transaction: int
    max_privacy_bytes_per_block: int
    max_statement_and_encrypted_output_bytes_per_transaction: int
    max_nullifiers_per_action: int
    max_commitments_per_action: int
    retained_root_count: int


class PrivacyConsensusLimitsTighteningV1(TypedDict):
    """Inspection-only JSON representation of consensus-limit tightening."""

    scheduled_at_height: int
    effective_at_height: int
    next_limits: PrivacyConsensusLimitsV1


class PrivacyConsensusPolicyV1(TypedDict):
    """Historical JSON policy fields with no live admission authority."""

    current_limits: PrivacyConsensusLimitsV1
    pending_tightening: PrivacyConsensusLimitsTighteningV1 | None


class LegacyPrivacyCapabilityInspectionRowV1(TypedDict):
    """One historical JSON row retained only for explicit inspection."""

    protocol_id: PrivacyProtocolTagV1
    compiled_profile: PrivacyCompiledProfileV1
    activation: PrivacyActivationV1 | None


class LegacyPrivacyCapabilityInspectionSnapshotV1(TypedDict):
    """Validated historical JSON payload with no admission authority."""

    version: Literal[1]
    committed_height: int
    consensus_policy: PrivacyConsensusPolicyV1
    protocols: list[LegacyPrivacyCapabilityInspectionRowV1]


class LegacyPrivacyCapabilityInspectionError(ValueError):
    """Raised when a retired JSON inspection payload violates its contract."""

    def __init__(
        self,
        message: str,
        path: str = "legacy privacy capability inspection",
    ) -> None:
        super().__init__(f"{path}: {message}")
        self.path = path


_MAX_U32 = (1 << 32) - 1
_MAX_U64 = (1 << 64) - 1
_POLICY_DELAY_BLOCKS_V1 = 300
_PROTOCOL_SET = frozenset(PRIVACY_PROTOCOL_IDS_V1)

_PROTOCOL_BINDINGS: dict[PrivacyProtocolIdV1, tuple[str, str]] = {
    "zk-ace-pq-authorization-v0": (
        "stark-fri-sha256-goldilocks",
        "native-goldilocks-stark-fri",
    ),
    "anonymous-pgc-k-out-of-n-v1": (
        "anonymous-pgc-p256",
        "native-anonymous-pgc-p256",
    ),
    "verange-transparent-range-v1": (
        "iroha-verange-p256",
        "native-verange-p256",
    ),
    "iroha-zk-ams-v1": (
        "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
        "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
    ),
    "vega-existing-credential-zk-v0": (
        "vega-neutron-nova-spartan-hyrax-t256",
        "native-vega",
    ),
    "iroha-zk-x509-stark-p256-v0": (
        "stark-fri-sha256-goldilocks",
        "native-goldilocks-stark-fri",
    ),
    "iroha-jindo-polynomial-commitment-v0": (
        "jindo-polynomial-commitment",
        "native-jindo",
    ),
    "iroha-bootle-lantern-anoncred-v1": (
        "lantern-lnp22-module-linear-norm",
        "native-lantern-lnp22",
    ),
    "orchard-halo2-actions-v1": ("halo2-ipa-pasta", "native-halo2-orchard"),
    "monero-fcmp-plus-plus-v1": (
        "fcmp-plus-plus-curve-tree-bulletproofs",
        "native-fcmp-plus-plus",
    ),
    "iroha-ivm-private-note-stark-v1": (
        "stark-fri-sha256-goldilocks",
        "native-goldilocks-stark-fri",
    ),
    "pq-masp-stark-v0": (
        "stark-fri-sha256-goldilocks",
        "native-goldilocks-stark-fri",
    ),
}

_CONSENSUS_LIMIT_MAXIMA = {
    "max_actions_per_transaction": 1,
    "max_actions_per_block": 2,
    "max_proof_bytes_per_action": 9 * 1024 * 1024,
    "max_action_bytes": 9 * 1024 * 1024,
    "max_privacy_bytes_per_transaction": 9 * 1024 * 1024,
    "max_privacy_bytes_per_block": 18 * 1024 * 1024,
    "max_statement_and_encrypted_output_bytes_per_transaction": 256 * 1024,
    "max_nullifiers_per_action": 8,
    "max_commitments_per_action": 8,
    "retained_root_count": 2048,
}

_PROTOCOL_LIMIT_FIELDS: dict[
    PrivacyProtocolIdV1, tuple[tuple[str, int, frozenset[int] | None], ...]
] = {
    "anonymous-pgc-k-out-of-n-v1": (
        ("max_anonymity_set_size", 64, frozenset((16, 32, 64))),
        ("max_recipient_count", 8, None),
    ),
    "verange-transparent-range-v1": (("max_aggregation_count", 8, None),),
    "iroha-zk-ams-v1": (
        ("max_batch_size", 8, None),
        ("max_ring_size", 64, frozenset((16, 32, 64))),
    ),
    "iroha-jindo-polynomial-commitment-v0": (("max_polynomial_count", 4, None),),
    "orchard-halo2-actions-v1": (("max_action_count", 2, None),),
    "monero-fcmp-plus-plus-v1": (
        ("max_input_count", 2, None),
        ("max_output_count", 4, None),
    ),
    "iroha-ivm-private-note-stark-v1": (
        ("max_input_count", 2, None),
        ("max_output_count", 2, None),
    ),
    "pq-masp-stark-v0": (
        ("max_input_count", 2, None),
        ("max_output_count", 2, None),
    ),
}


def parse_legacy_privacy_capability_inspection_v1(
    payload: object,
) -> LegacyPrivacyCapabilityInspectionSnapshotV1:
    """Inspect and defensively copy one retired V1 JSON payload.

    This function is deliberately disconnected from ``Client.privacy_capabilities_v1``
    and cannot produce an Exact12 admission token.
    """

    snapshot = _exact_object(
        payload,
        ("version", "committed_height", "consensus_policy", "protocols"),
        "legacy privacy capability inspection",
    )
    if _u32(snapshot["version"], "legacy privacy capability inspection.version") != 1:
        _fail("version must be exactly 1", "legacy privacy capability inspection.version")
    committed_height = _u64(
        snapshot["committed_height"], "legacy privacy capability inspection.committed_height"
    )
    consensus_policy = _parse_consensus_policy(snapshot["consensus_policy"], committed_height)
    rows = snapshot["protocols"]
    if type(rows) is not list or len(rows) != len(PRIVACY_PROTOCOL_IDS_V1):
        _fail(
            "protocols must contain exactly the 12 canonical protocol rows",
            "legacy privacy capability inspection.protocols",
        )
    protocols = [
        _parse_capability_row(
            row,
            expected,
            committed_height,
            f"legacy privacy capability inspection.protocols[{index}]",
        )
        for index, (row, expected) in enumerate(zip(rows, PRIVACY_PROTOCOL_IDS_V1, strict=True))
    ]
    return cast(
        LegacyPrivacyCapabilityInspectionSnapshotV1,
        {
            "version": 1,
            "committed_height": committed_height,
            "consensus_policy": consensus_policy,
            "protocols": protocols,
        },
    )


def parse_legacy_privacy_capability_inspection_json_v1(
    payload: bytes | bytearray | memoryview | str,
) -> LegacyPrivacyCapabilityInspectionSnapshotV1:
    """Decode retired JSON strictly for diagnostics, never live admission."""

    if isinstance(payload, str):
        encoded = payload.encode("utf-8")
    elif isinstance(payload, (bytes, bytearray, memoryview)):
        encoded = bytes(payload)
    else:
        raise TypeError("privacy capability JSON must be bytes or str")
    if len(encoded) > LEGACY_PRIVACY_CAPABILITY_INSPECTION_MAX_JSON_BYTES_V1:
        raise LegacyPrivacyCapabilityInspectionError(
            "JSON exceeds the 262144-byte first-release limit"
        )
    try:
        text = encoded.decode("utf-8", "strict")
    except UnicodeDecodeError as exc:
        raise LegacyPrivacyCapabilityInspectionError("JSON must be strict UTF-8") from exc

    def reject_constant(value: str) -> None:
        raise LegacyPrivacyCapabilityInspectionError(f"JSON number {value} is not permitted")

    def reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise LegacyPrivacyCapabilityInspectionError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result

    try:
        decoded = json.loads(
            text,
            object_pairs_hook=reject_duplicate_pairs,
            parse_constant=reject_constant,
        )
    except LegacyPrivacyCapabilityInspectionError:
        raise
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise LegacyPrivacyCapabilityInspectionError("malformed JSON") from exc
    return parse_legacy_privacy_capability_inspection_v1(decoded)


def _parse_consensus_policy(value: object, committed_height: int) -> dict[str, Any]:
    path = "legacy privacy capability inspection.consensus_policy"
    policy = _exact_object(value, ("current_limits", "pending_tightening"), path)
    current = _parse_consensus_limits(policy["current_limits"], f"{path}.current_limits")
    pending_value = policy["pending_tightening"]
    pending: dict[str, Any] | None = None
    if pending_value is not None:
        pending_path = f"{path}.pending_tightening"
        tightening = _exact_object(
            pending_value,
            ("scheduled_at_height", "effective_at_height", "next_limits"),
            pending_path,
        )
        scheduled = _positive_u64(
            tightening["scheduled_at_height"], f"{pending_path}.scheduled_at_height"
        )
        effective = _positive_u64(
            tightening["effective_at_height"], f"{pending_path}.effective_at_height"
        )
        if (
            scheduled > _MAX_U64 - _POLICY_DELAY_BLOCKS_V1
            or effective <= scheduled
            or effective < scheduled + _POLICY_DELAY_BLOCKS_V1
            or scheduled > committed_height
            or effective <= committed_height
        ):
            _fail("has invalid committed-height schedule", pending_path)
        next_limits = _parse_consensus_limits(
            tightening["next_limits"], f"{pending_path}.next_limits"
        )
        _assert_strict_consensus_tightening(current, next_limits, pending_path)
        pending = {
            "scheduled_at_height": scheduled,
            "effective_at_height": effective,
            "next_limits": next_limits,
        }
    return {"current_limits": current, "pending_tightening": pending}


def _parse_consensus_limits(value: object, path: str) -> dict[str, int]:
    limits = _exact_object(value, tuple(_CONSENSUS_LIMIT_MAXIMA), path)
    result: dict[str, int] = {}
    for key, maximum in _CONSENSUS_LIMIT_MAXIMA.items():
        number = _positive_u32(limits[key], f"{path}.{key}")
        if number > maximum:
            _fail("exceeds the first-release hard maximum", f"{path}.{key}")
        result[key] = number
    if (
        result["max_actions_per_transaction"] > result["max_actions_per_block"]
        or result["max_proof_bytes_per_action"] > result["max_action_bytes"]
        or result["max_action_bytes"] > result["max_privacy_bytes_per_transaction"]
        or result["max_privacy_bytes_per_transaction"] > result["max_privacy_bytes_per_block"]
        or result["max_statement_and_encrypted_output_bytes_per_transaction"]
        > result["max_action_bytes"]
    ):
        _fail("violates consensus resource-limit ordering", path)
    return result


def _parse_capability_row(
    value: object,
    expected_protocol: PrivacyProtocolIdV1,
    committed_height: int,
    path: str,
) -> LegacyPrivacyCapabilityInspectionRowV1:
    row = _exact_object(value, ("protocol_id", "compiled_profile", "activation"), path)
    protocol = _protocol_tag(row["protocol_id"], f"{path}.protocol_id")
    if protocol != expected_protocol:
        _fail(f"must be canonical protocol {expected_protocol}", f"{path}.protocol_id")
    compiled = _parse_compiled_profile(
        row["compiled_profile"], protocol, f"{path}.compiled_profile"
    )
    activation = (
        None
        if row["activation"] is None
        else _parse_activation(
            row["activation"],
            protocol,
            compiled,
            committed_height,
            f"{path}.activation",
        )
    )
    if activation is not None and compiled["status"] != "available":
        _fail("cannot activate an unavailable compiled profile", f"{path}.activation")
    return cast(
        LegacyPrivacyCapabilityInspectionRowV1,
        {
            "protocol_id": _tagged("protocol", protocol),
            "compiled_profile": compiled,
            "activation": activation,
        },
    )


def _parse_compiled_profile(
    value: object, protocol: PrivacyProtocolIdV1, path: str
) -> dict[str, Any]:
    result = _exact_object(value, ("status", "value"), path)
    if result["status"] == "available":
        return {
            "status": "available",
            "value": _parse_profile(result["value"], protocol, f"{path}.value"),
        }
    if result["status"] != "unavailable":
        _fail("status must be available or unavailable", f"{path}.status")
    reason_path = f"{path}.value"
    reason = _exact_object(result["value"], ("reason", "detail"), reason_path)
    if reason["reason"] in ("engine-unavailable", "profile-initialization-failed"):
        if reason["detail"] is not None:
            _fail("unit unavailable reason must have null detail", f"{reason_path}.detail")
        normalized_reason: dict[str, Any] = {
            "reason": reason["reason"],
            "detail": None,
        }
    elif reason["reason"] == "statement-schema-invalid":
        detail = _tagged_unit(
            reason["detail"],
            "schema_error",
            "detail",
            ("conflicting-stable-type-id", "missing-type-reference"),
            f"{reason_path}.detail",
        )
        normalized_reason = {
            "reason": "statement-schema-invalid",
            "detail": _tagged("schema_error", detail, content_key="detail"),
        }
    else:
        _fail("unknown unavailable reason", f"{reason_path}.reason")
    return {"status": "unavailable", "value": normalized_reason}


def _parse_profile(value: object, protocol: PrivacyProtocolIdV1, path: str) -> dict[str, Any]:
    profile = _exact_object(
        value,
        (
            "protocol_id",
            "proof_system_id",
            "engine_id",
            "parameter_id",
            "parameter_digest",
            "verifier_digest",
            "statement_schema_digest",
            "engine_manifest_digest",
            "protocol_limits",
        ),
        path,
    )
    result = _parse_bindings(profile, protocol, path)
    result["protocol_limits"] = _parse_protocol_limits(
        profile["protocol_limits"], protocol, f"{path}.protocol_limits"
    )
    return result


def _parse_activation(
    value: object,
    protocol: PrivacyProtocolIdV1,
    compiled: dict[str, Any],
    committed_height: int,
    path: str,
) -> dict[str, Any]:
    record = _exact_object(
        value,
        (
            "protocol_id",
            "proof_system_id",
            "engine_id",
            "parameter_id",
            "parameter_digest",
            "verifier_digest",
            "statement_schema_digest",
            "engine_manifest_digest",
            "lifecycle",
            "protocol_limits",
            "pending_protocol_limits_tightening",
            "assurance",
        ),
        path,
    )
    result = _parse_bindings(record, protocol, path)
    if compiled["status"] == "available":
        _assert_bindings_equal(result, compiled["value"], path)
    limits = _parse_protocol_limits(record["protocol_limits"], protocol, f"{path}.protocol_limits")
    if compiled["status"] == "available":
        _assert_limits_at_most(
            limits, compiled["value"]["protocol_limits"], f"{path}.protocol_limits"
        )
    result.update(
        {
            "lifecycle": _parse_lifecycle(
                record["lifecycle"], committed_height, f"{path}.lifecycle"
            ),
            "protocol_limits": limits,
            "pending_protocol_limits_tightening": _parse_protocol_tightening(
                record["pending_protocol_limits_tightening"],
                limits,
                committed_height,
                f"{path}.pending_protocol_limits_tightening",
            ),
            "assurance": _tagged(
                "assurance",
                _tagged_unit(
                    record["assurance"],
                    "assurance",
                    "value",
                    ("experimental",),
                    f"{path}.assurance",
                ),
            ),
        }
    )
    return result


def _parse_bindings(
    value: dict[str, Any], protocol: PrivacyProtocolIdV1, path: str
) -> dict[str, Any]:
    protocol_id = _protocol_tag(value["protocol_id"], f"{path}.protocol_id")
    if protocol_id != protocol:
        _fail("does not match its row protocol", f"{path}.protocol_id")
    proof_system, engine = _PROTOCOL_BINDINGS[protocol]
    parsed_proof = _tagged_unit(
        value["proof_system_id"],
        "proof_system",
        "value",
        (proof_system,),
        f"{path}.proof_system_id",
    )
    parsed_engine = _tagged_unit(
        value["engine_id"],
        "engine",
        "value",
        (engine,),
        f"{path}.engine_id",
    )
    return {
        "protocol_id": _tagged("protocol", protocol),
        "proof_system_id": _tagged("proof_system", parsed_proof),
        "engine_id": _tagged("engine", parsed_engine),
        "parameter_id": _fixed_bytes(value["parameter_id"], f"{path}.parameter_id"),
        "parameter_digest": _fixed_bytes(value["parameter_digest"], f"{path}.parameter_digest"),
        "verifier_digest": _fixed_bytes(value["verifier_digest"], f"{path}.verifier_digest"),
        "statement_schema_digest": _fixed_bytes(
            value["statement_schema_digest"], f"{path}.statement_schema_digest"
        ),
        "engine_manifest_digest": _fixed_bytes(
            value["engine_manifest_digest"], f"{path}.engine_manifest_digest"
        ),
    }


def _parse_protocol_limits(
    value: object, protocol: PrivacyProtocolIdV1, path: str
) -> dict[str, Any]:
    tagged = _exact_object(value, ("protocol", "limits"), path)
    protocol_value = tagged["protocol"]
    if type(protocol_value) is not str or protocol_value not in _PROTOCOL_SET:
        _fail("has an unknown or non-canonical protocol tag", f"{path}.protocol")
    if protocol_value != protocol:
        _fail("does not match the protocol binding", f"{path}.protocol")
    fields = _PROTOCOL_LIMIT_FIELDS.get(protocol, ())
    if not fields:
        if tagged["limits"] is not None:
            _fail("fixed protocol limits must be null", f"{path}.limits")
        return {"protocol": protocol, "limits": None}
    limits = _exact_object(tagged["limits"], tuple(field[0] for field in fields), f"{path}.limits")
    normalized: dict[str, int] = {}
    for key, maximum, permitted in fields:
        number = _positive_u32(limits[key], f"{path}.limits.{key}")
        if number > maximum or (permitted is not None and number not in permitted):
            _fail("is outside the closed first-release limit set", f"{path}.limits.{key}")
        normalized[key] = number
    return {"protocol": protocol, "limits": normalized}


def _parse_lifecycle(value: object, committed_height: int, path: str) -> dict[str, Any]:
    lifecycle = _exact_object(value, ("state", "record"), path)
    state = lifecycle["state"]
    if state not in ("proposed", "active", "suspended", "retired"):
        _fail("unknown lifecycle state", f"{path}.state")
    keys = (
        ("proposed_at_height", "activate_at_height")
        if state == "proposed"
        else ("proposed_at_height", "activated_at_height", "state_since_height")
    )
    record = _exact_object(lifecycle["record"], keys, f"{path}.record")
    proposed = _positive_u64(record["proposed_at_height"], f"{path}.record.proposed_at_height")
    normalized: dict[str, Any]
    if state == "proposed":
        activate = _positive_u64(record["activate_at_height"], f"{path}.record.activate_at_height")
        if activate <= proposed or proposed > committed_height or activate <= committed_height:
            _fail("has invalid proposed lifecycle heights", path)
        normalized = {
            "proposed_at_height": proposed,
            "activate_at_height": activate,
        }
    else:
        activated_value = record["activated_at_height"]
        activated = (
            None
            if state == "retired" and activated_value is None
            else _positive_u64(activated_value, f"{path}.record.activated_at_height")
        )
        since = _positive_u64(record["state_since_height"], f"{path}.record.state_since_height")
        if (
            proposed > committed_height
            or since > committed_height
            or (activated is not None and activated > committed_height)
        ):
            _fail("claims a state after committed height", path)
        if (
            activated is None
            and (state != "retired" or since <= proposed)
            or activated is not None
            and (
                activated <= proposed
                or (since < activated if state == "active" else since <= activated)
            )
        ):
            _fail("has invalid lifecycle ordering", path)
        normalized = {
            "proposed_at_height": proposed,
            "activated_at_height": activated,
            "state_since_height": since,
        }
    return {"state": state, "record": normalized}


def _parse_protocol_tightening(
    value: object,
    current: dict[str, Any],
    committed_height: int,
    path: str,
) -> dict[str, Any] | None:
    if value is None:
        return None
    tightening = _exact_object(
        value, ("scheduled_at_height", "effective_at_height", "next_limits"), path
    )
    scheduled = _positive_u64(tightening["scheduled_at_height"], f"{path}.scheduled_at_height")
    effective = _positive_u64(tightening["effective_at_height"], f"{path}.effective_at_height")
    if (
        scheduled > _MAX_U64 - _POLICY_DELAY_BLOCKS_V1
        or effective <= scheduled
        or effective < scheduled + _POLICY_DELAY_BLOCKS_V1
        or scheduled > committed_height
        or effective <= committed_height
    ):
        _fail("has invalid committed-height schedule", path)
    next_limits = _parse_protocol_limits(
        tightening["next_limits"], current["protocol"], f"{path}.next_limits"
    )
    _assert_limits_at_most(next_limits, current, f"{path}.next_limits")
    if next_limits == current:
        _fail("must be a strict tightening", path)
    return {
        "scheduled_at_height": scheduled,
        "effective_at_height": effective,
        "next_limits": next_limits,
    }


def _protocol_tag(value: object, path: str) -> PrivacyProtocolIdV1:
    return cast(
        PrivacyProtocolIdV1,
        _tagged_unit(value, "protocol", "value", PRIVACY_PROTOCOL_IDS_V1, path),
    )


def _tagged_unit(
    value: object,
    tag_key: str,
    content_key: str,
    permitted: tuple[str, ...],
    path: str,
) -> str:
    tagged = _exact_object(value, (tag_key, content_key), path)
    tag = tagged[tag_key]
    if type(tag) is not str or tag not in permitted:
        _fail("has an unknown or non-canonical tag", f"{path}.{tag_key}")
    if tagged[content_key] is not None:
        _fail("unit enum content must be null", f"{path}.{content_key}")
    return tag


def _tagged(tag_key: str, value: str, *, content_key: str = "value") -> dict[str, Any]:
    return {tag_key: value, content_key: None}


def _fixed_bytes(value: object, path: str) -> list[int]:
    if type(value) is not list or len(value) != 32:
        _fail("must be exactly 32 bytes", path)
    result: list[int] = []
    for index, byte in enumerate(cast(list[object], value)):
        if type(byte) is not int or not 0 <= byte <= 255:
            _fail("must contain only uint8 values", f"{path}[{index}]")
        result.append(cast(int, byte))
    if not any(result):
        _fail("must not be all zero", path)
    return result


def _exact_object(value: object, expected_keys: tuple[str, ...], path: str) -> dict[str, Any]:
    if type(value) is not dict:
        _fail("must be a plain JSON object", path)
    result = cast(dict[str, Any], value)
    if len(result) != len(expected_keys) or set(result) != set(expected_keys):
        _fail(f"must contain exactly: {', '.join(sorted(expected_keys))}", path)
    return result


def _u32(value: object, path: str) -> int:
    if type(value) is not int or not 0 <= value <= _MAX_U32:
        _fail("must be a uint32 integer", path)
    return cast(int, value)


def _positive_u32(value: object, path: str) -> int:
    result = _u32(value, path)
    if result == 0:
        _fail("must be non-zero", path)
    return result


def _u64(value: object, path: str) -> int:
    if type(value) is not int or not 0 <= value <= _MAX_U64:
        _fail("must be a uint64 integer", path)
    return cast(int, value)


def _positive_u64(value: object, path: str) -> int:
    result = _u64(value, path)
    if result == 0:
        _fail("must be non-zero", path)
    return result


def _assert_bindings_equal(actual: dict[str, Any], expected: dict[str, Any], path: str) -> None:
    for key in (
        "protocol_id",
        "proof_system_id",
        "engine_id",
        "parameter_id",
        "parameter_digest",
        "verifier_digest",
        "statement_schema_digest",
        "engine_manifest_digest",
    ):
        if actual[key] != expected[key]:
            _fail(f"does not match compiled profile {key}", f"{path}.{key}")


def _assert_limits_at_most(actual: dict[str, Any], ceiling: dict[str, Any], path: str) -> None:
    if actual["protocol"] != ceiling["protocol"] or (
        (actual["limits"] is None) != (ceiling["limits"] is None)
    ):
        _fail("protocol-limit tag differs from compiled ceiling", path)
    if actual["limits"] is not None:
        for key, value in actual["limits"].items():
            if value > ceiling["limits"][key]:
                _fail("exceeds the compiled profile ceiling", f"{path}.{key}")


def _assert_strict_consensus_tightening(
    current: dict[str, int], next_limits: dict[str, int], path: str
) -> None:
    changed = False
    for key in _CONSENSUS_LIMIT_MAXIMA:
        if next_limits[key] > current[key]:
            _fail("cannot increase a consensus limit", f"{path}.next_limits.{key}")
        changed = changed or next_limits[key] != current[key]
    if not changed:
        _fail("must be a strict tightening", path)


def _fail(message: str, path: str) -> None:
    raise LegacyPrivacyCapabilityInspectionError(message, path)


__all__ = [
    "LEGACY_PRIVACY_CAPABILITY_INSPECTION_MAX_JSON_BYTES_V1",
    "LEGACY_PRIVACY_CAPABILITY_INSPECTION_VERSION_V1",
    "PRIVACY_PROTOCOL_IDS_V1",
    "PrivacyActivationV1",
    "PrivacyAssuranceTagV1",
    "LegacyPrivacyCapabilityInspectionRowV1",
    "LegacyPrivacyCapabilityInspectionError",
    "LegacyPrivacyCapabilityInspectionSnapshotV1",
    "PrivacyCompiledProfileAvailableV1",
    "PrivacyCompiledProfileUnavailableReasonV1",
    "PrivacyCompiledProfileUnavailableV1",
    "PrivacyCompiledProfileV1",
    "PrivacyCompiledProfileValueV1",
    "PrivacyConsensusLimitsTighteningV1",
    "PrivacyConsensusLimitsV1",
    "PrivacyConsensusPolicyV1",
    "PrivacyEngineIdV1",
    "PrivacyEngineTagV1",
    "PrivacyLifecycleEstablishedRecordV1",
    "PrivacyLifecycleProposedRecordV1",
    "PrivacyLifecycleV1",
    "PrivacyProofSystemIdV1",
    "PrivacyProofSystemTagV1",
    "PrivacyProtocolIdV1",
    "PrivacyProtocolLimitsTighteningV1",
    "PrivacyProtocolLimitsV1",
    "PrivacyProtocolTagV1",
    "PrivacyStatementSchemaErrorV1",
    "parse_legacy_privacy_capability_inspection_json_v1",
    "parse_legacy_privacy_capability_inspection_v1",
]
