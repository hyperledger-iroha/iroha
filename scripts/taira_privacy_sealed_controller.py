#!/usr/bin/env python3
"""Fail-closed Exact12 construction and diagnostic mechanics.

This module is deliberately not a receipt issuer.  It owns the narrow native
action-driver IPC, a generic VeRange finality diagnostic, and a typed planned
query for the canonical committed VeRange capability row.  The diagnostic does
not invoke that query before and after restart and VeRange has no action-owned
committed state, so it is not a release controller case.  The release barrier
is keyed to the separate ``CONTROLLER_CASE_RUNNERS`` table, which remains empty
until complete protocol submission, state agreement, replay, restart, and
post-restart validation exist.

The native driver receives no endpoint, credential, restart handle, or expected
network outcome.  Its response contains one proof-bearing signed transaction
plus immutable identifiers; all semantic conclusions are derived here from
direct peer responses.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import re
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import date, timedelta
from pathlib import Path
from types import MappingProxyType
from typing import Iterator, Mapping, NoReturn, Sequence

try:
    from . import taira_privacy_action_driver_ipc as action_ipc
    from . import taira_privacy_verange_case_plan as verange_case_plan
except ImportError:
    import taira_privacy_action_driver_ipc as action_ipc
    import taira_privacy_verange_case_plan as verange_case_plan

VeRangePublicAdmissionArtifactsV1 = (
    verange_case_plan.VeRangePublicAdmissionArtifactsV1
)
VeRangeQualificationCasePlanV1 = verange_case_plan.VeRangeQualificationCasePlanV1
VeRangeQualificationPlanError = verange_case_plan.VeRangeQualificationPlanError
build_verange_qualification_case_plan_v1 = (
    verange_case_plan.build_verange_qualification_case_plan_v1
)

DRIVER_REQUEST_SCHEMA = action_ipc.REQUEST_SCHEMA
DRIVER_RESPONSE_SCHEMA = action_ipc.RESPONSE_SCHEMA
DRIVER_SCHEMA_VERSION = action_ipc.SCHEMA_VERSION
VERANGE_OPERATION = action_ipc.VERANGE_OPERATION
VERANGE_PROTOCOL = action_ipc.VERANGE_PROTOCOL
CONSTRUCTIBLE_OPERATION_BY_PROTOCOL = action_ipc.CONSTRUCTIBLE_OPERATION_BY_PROTOCOL
PROTOCOL_BY_CONSTRUCTIBLE_OPERATION = action_ipc.PROTOCOL_BY_CONSTRUCTIBLE_OPERATION
REQUEST_ID_DOMAIN = action_ipc.REQUEST_ID_DOMAIN
DRIVER_SEED_DOMAIN = b"iroha.taira.privacy_action_driver_seed.v1\0"

TRANSCRIPT_SCHEMA = "iroha.taira.privacy_sealed_case_transcript"
RESULT_SCHEMA = "iroha.taira.privacy_sealed_case_result"
CONTROLLER_SCHEMA_VERSION = 1
TRANSCRIPT_ID_DOMAIN = b"iroha.taira.privacy_sealed_case_transcript.v1\0"
RESULT_ID_DOMAIN = b"iroha.taira.privacy_sealed_case_result.v1\0"

MAX_DRIVER_REQUEST_BYTES = action_ipc.MAX_REQUEST_BYTES
MAX_ACTION_BYTES = action_ipc.MAX_TRANSACTION_BYTES
MAX_DRIVER_RESPONSE_BYTES = action_ipc.MAX_RESPONSE_BYTES
MAX_DRIVER_ERROR_BYTES = 64 * 1024
MAX_ACTION_DRIVER_BYTES = 1024 * 1024 * 1024
MAX_HTTP_RESPONSE_BYTES = 256 * 1024
MAX_TRANSCRIPT_BYTES = 48 * 1024 * 1024
MAX_RESULT_BYTES = 128 * 1024
MAX_TRANSCRIPT_EVENTS = 512
PEER_COUNT = 4
PRIVACY_CAPABILITIES_PATH = "/v1/privacy/capabilities"
PRIVACY_CAPABILITIES_MEDIA_TYPE = "application/x-norito"

SHA256_RE = re.compile(r"[0-9a-f]{64}")
BLOCK_HASH_RE = re.compile(
    r"(?:hash:)?([0-9A-Fa-f]{64})(?:#[0-9A-Fa-f]{4})?"
)

# Exact PrivacyProtocolIdV1::ALL order.  This tuple is an issuance requirement,
# not a claim that the local controller can build the corresponding actions.
RETAINED_PROTOCOLS = (
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

# Every row below has only a native, proof-bearing, signed transaction
# constructor.  It is not evidence of network submission or protocol state.
CONSTRUCTIBLE_OPERATIONS: Mapping[str, str] = MappingProxyType(
    dict(CONSTRUCTIBLE_OPERATION_BY_PROTOCOL)
)

# A row may enter this table only when the sealed controller itself implements
# submission, four-peer protocol-specific state queries, replay rejection,
# restart convergence, and post-restart state validation.  No such complete
# runner exists yet; constructor additions cannot open receipt issuance.
CONTROLLER_CASE_RUNNERS: Mapping[str, str] = MappingProxyType({})

# These are code-owned completion residuals, not caller- or environment-owned
# assertions.  VeRange has a concrete planned state query, but cannot enter the
# runner table until the controller has an authority admitted by the exact
# genesis/source closure and the native driver exposes the complete public-only
# policy/activation transaction bundle.
CONTROLLER_CASE_BLOCKERS: Mapping[str, tuple[str, ...]] = MappingProxyType(
    {
        protocol: (
            action_ipc.VERANGE_CONTROLLER_BLOCKERS
            if protocol == VERANGE_PROTOCOL
            else (action_ipc.MISSING_CONTROLLER_CASE_EVIDENCE,)
        )
        for protocol in RETAINED_PROTOCOLS
    }
)


@dataclass(frozen=True)
class PlannedControllerCaseV1:
    protocol: str
    operation: str
    state_query_path: str
    state_query_media_type: str
    state_query_phases: tuple[str, ...]
    blockers: tuple[str, ...]


VERANGE_PLANNED_CONTROLLER_CASE = PlannedControllerCaseV1(
    protocol=VERANGE_PROTOCOL,
    operation=VERANGE_OPERATION,
    state_query_path=PRIVACY_CAPABILITIES_PATH,
    state_query_media_type=PRIVACY_CAPABILITIES_MEDIA_TYPE,
    state_query_phases=("before-action", "after-rejection", "after-restart"),
    blockers=CONTROLLER_CASE_BLOCKERS[VERANGE_PROTOCOL],
)


class SealedPrivacyControllerError(RuntimeError):
    """The controller could not independently establish a required outcome."""


class TransientPeerError(SealedPrivacyControllerError):
    """A direct peer was temporarily unavailable while a bounded wait was active."""


def _fail(message: str) -> NoReturn:
    raise SealedPrivacyControllerError(message)


def missing_constructible_operations() -> tuple[str, ...]:
    """Return retained protocols that still lack a native constructor."""

    return tuple(
        protocol
        for protocol in RETAINED_PROTOCOLS
        if protocol not in CONSTRUCTIBLE_OPERATIONS
    )


def missing_release_operations() -> tuple[str, ...]:
    """Return retained protocols without a complete sealed controller case."""

    return tuple(
        protocol
        for protocol in RETAINED_PROTOCOLS
        if protocol not in CONTROLLER_CASE_RUNNERS
        or CONTROLLER_CASE_BLOCKERS.get(protocol)
    )


def controller_case_blockers(protocol: str) -> tuple[str, ...]:
    """Return immutable source-owned residuals for one retained case."""

    if protocol not in RETAINED_PROTOCOLS:
        _fail("controller-case blocker query selected an unknown protocol")
    return CONTROLLER_CASE_BLOCKERS.get(protocol, ())


def require_complete_release_operation_surface() -> None:
    """Keep issuance closed until all retained controller cases really exist."""

    unexpected_constructors = sorted(
        set(CONSTRUCTIBLE_OPERATIONS) - set(RETAINED_PROTOCOLS)
    )
    unexpected_cases = sorted(set(CONTROLLER_CASE_RUNNERS) - set(RETAINED_PROTOCOLS))
    cases_without_constructors = sorted(
        set(CONTROLLER_CASE_RUNNERS) - set(CONSTRUCTIBLE_OPERATIONS)
    )
    cases_with_blockers = sorted(
        protocol
        for protocol in CONTROLLER_CASE_RUNNERS
        if CONTROLLER_CASE_BLOCKERS.get(protocol)
    )
    invalid_blocker_rows = sorted(
        protocol
        for protocol, blockers in CONTROLLER_CASE_BLOCKERS.items()
        if protocol not in RETAINED_PROTOCOLS
        or not isinstance(blockers, tuple)
        or not blockers
        or any(not isinstance(blocker, str) or not blocker for blocker in blockers)
    )
    missing_blocker_rows = sorted(
        set(RETAINED_PROTOCOLS)
        - set(CONTROLLER_CASE_RUNNERS)
        - set(CONTROLLER_CASE_BLOCKERS)
    )
    missing = missing_release_operations()
    if (
        missing
        or unexpected_constructors
        or unexpected_cases
        or cases_without_constructors
        or cases_with_blockers
        or invalid_blocker_rows
        or missing_blocker_rows
    ):
        blocking_residuals = {
            protocol: list(CONTROLLER_CASE_BLOCKERS.get(protocol, ()))
            for protocol in missing
        }
        _fail(
            "privacy release issuance is closed; complete sealed controller cases "
            f"are missing={list(missing)!r}, blocking_residuals={blocking_residuals!r}, "
            f"unexpected_cases={unexpected_cases!r}, "
            f"cases_without_constructors={cases_without_constructors!r}, "
            f"cases_with_blockers={cases_with_blockers!r}, "
            f"invalid_blocker_rows={invalid_blocker_rows!r}, "
            f"missing_blocker_rows={missing_blocker_rows!r}, "
            f"unexpected_constructors={unexpected_constructors!r}"
        )


def _canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(
            value,
            indent=2,
            sort_keys=True,
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def _driver_json_bytes(value: object) -> bytes:
    """Render the driver's one compact, field-order-preserving JSON form."""

    return (
        json.dumps(
            value,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def _duplicate_free_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON field {key!r}")
        value[key] = item
    return value


def _reject_constant(value: str) -> NoReturn:
    raise ValueError(f"non-finite JSON number {value!r} is forbidden")


def _json_object(payload: bytes, label: str, maximum: int) -> dict[str, object]:
    if not payload or len(payload) > maximum:
        _fail(f"{label} is empty or exceeds {maximum} bytes")
    try:
        text = payload.decode("ascii")
        value = json.loads(
            text,
            object_pairs_hook=_duplicate_free_object,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as exc:
        raise SealedPrivacyControllerError(f"{label} is not strict JSON: {exc}") from exc
    if not isinstance(value, dict):
        _fail(f"{label} must be one JSON object")
    return value


def _exact_fields(value: Mapping[str, object], expected: set[str], label: str) -> None:
    if set(value) != expected:
        _fail(
            f"{label} fields are not exact: "
            f"missing={sorted(expected - set(value))!r}, "
            f"extra={sorted(set(value) - expected)!r}"
        )


def _sha256(value: object, label: str, *, nonzero: bool = False) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    if nonzero and value == "0" * 64:
        _fail(f"{label} must be nonzero")
    return value


def _hex_bytes(value: object, label: str, maximum: int) -> bytes:
    if (
        not isinstance(value, str)
        or len(value) % 2 != 0
        or not value
        or len(value) > maximum * 2
        or re.fullmatch(r"[0-9a-f]+", value) is None
    ):
        _fail(f"{label} is not bounded canonical lowercase hexadecimal")
    return bytes.fromhex(value)


@dataclass(frozen=True)
class Exact12ActionRequest:
    """Public inputs for one admitted native Exact12 action operation."""

    asset_definition_id: str
    candidate_binding_sha256: str
    creation_time_millis: int
    network_id_hex: str
    nonce: int
    protocol: str
    ttl_millis: int

    def canonical_bytes(self) -> bytes:
        try:
            return action_ipc.build_action_request(
                asset_definition_id=self.asset_definition_id,
                candidate_binding_sha256=self.candidate_binding_sha256,
                creation_time_millis=self.creation_time_millis,
                network_id_hex=self.network_id_hex,
                nonce=self.nonce,
                protocol=self.protocol,
                ttl_millis=self.ttl_millis,
            )
        except action_ipc.PrivacyActionDriverIpcError as exc:
            raise SealedPrivacyControllerError(str(exc)) from exc


@dataclass(frozen=True)
class VeRangeActionRequest:
    """Public inputs for one native proof-bearing VeRange action."""

    asset_definition_id: str
    candidate_binding_sha256: str
    creation_time_millis: int
    network_id_hex: str
    nonce: int
    ttl_millis: int

    def canonical_bytes(self) -> bytes:
        try:
            return action_ipc.build_verange_request(
                asset_definition_id=self.asset_definition_id,
                candidate_binding_sha256=self.candidate_binding_sha256,
                creation_time_millis=self.creation_time_millis,
                network_id_hex=self.network_id_hex,
                nonce=self.nonce,
                ttl_millis=self.ttl_millis,
            )
        except action_ipc.PrivacyActionDriverIpcError as exc:
            raise SealedPrivacyControllerError(str(exc)) from exc


@dataclass(frozen=True)
class ActionArtifact:
    availability: str
    limitations: tuple[str, ...]
    network_outcome_authoritative: bool
    qualification_scope: str
    request_id: str
    request_bytes: bytes
    response_bytes: bytes
    transaction: bytes
    transaction_hash_hex: str
    transaction_sha256: str
    action_driver_sha256: str = ""
    operation: str = ""
    protocol: str = ""
    public_admission_artifacts: VeRangePublicAdmissionArtifactsV1 | None = None


@dataclass(frozen=True)
class NativeVeRangeInspection:
    transaction_hash_hex: str
    transaction_intent_digest: str
    statement_digest: str
    proof_envelope_hash: str
    policy_id: str
    proof_bytes: int
    statement_bytes: int


@dataclass(frozen=True)
class NativeActionInspection:
    """Authenticated common and operation-specific identity for one action."""

    operation: str
    protocol: str
    transaction_hash_hex: str
    transaction_intent_digest: str
    statement_digest: str
    proof_envelope_hash: str
    proof_bytes: int
    statement_bytes: int
    identity_sha256: str
    availability: str
    limitations: tuple[str, ...]
    network_outcome_authoritative: bool
    qualification_scope: str


_NATIVE_COMMON_FIELDS = frozenset(
    {
        "adaptive_signed_transaction_bytes",
        "encoded_proof_envelope_bytes",
        "execution_classification",
        "ledger_effect",
        "proof_bytes",
        "proof_envelope_hash",
        "protocol_id",
        "statement_bytes",
        "statement_digest",
        "submitted_versioned_transaction_bytes",
        "transaction_hash",
        "transaction_intent_digest",
    }
)
_NATIVE_JINDO_FIELDS = frozenset(
    {
        "adaptive_signed_transaction_bytes",
        "availability",
        "encoded_proof_envelope_bytes",
        "limitations",
        "polynomial_count",
        "proof_bytes",
        "proof_envelope_hash",
        "statement_bytes",
        "statement_digest",
        "submitted_versioned_transaction_bytes",
        "transaction_hash",
        "transaction_intent_digest",
    }
)
_NATIVE_EXTRA_FIELDS: Mapping[str, frozenset[str]] = MappingProxyType(
    {
        "zk-ace-pq-authorization-v0": frozenset(
            {
                "action_kind",
                "amount",
                "asset_definition_id",
                "authorization_epoch",
                "destination_account_id",
                "identity_commitment",
                "policy_digest",
                "policy_id",
                "public_balance_scope",
                "replay_nullifier",
                "source_account_id",
            }
        ),
        "anonymous-pgc-k-out-of-n-v1": frozenset(
            {
                "account_state_root",
                "account_state_root_epoch",
                "action_kind",
                "anonymity_set_size",
                "asset_definition_id",
                "next_account_state_root",
                "next_account_state_root_epoch",
                "pool_id",
                "recipient_count",
            }
        ),
        VERANGE_PROTOCOL: frozenset(
            {
                "aggregation_count",
                "asset_definition_id",
                "bit_length",
                "policy_id",
                "value_commitments",
            }
        ),
        "vega-existing-credential-zk-v0": frozenset(
            {
                "device_authentication_algorithm",
                "device_authentication_digest",
                "digest_algorithm",
                "document_type",
                "issuer_authentication_algorithm",
                "issuer_id",
                "issuer_public_key",
                "issuer_record_digest",
                "issuer_record_epoch",
                "minimum_age_years",
                "namespace",
                "presentation_day",
                "presentation_month",
                "presentation_year",
                "reader_challenge",
                "session_transcript_digest",
            }
        ),
        "iroha-jindo-polynomial-commitment-v0": frozenset({"polynomial_count"}),
        "iroha-bootle-lantern-anoncred-v1": frozenset(
            {
                "action_kind",
                "disclosed_attribute_values",
                "disclosure_indices",
                "issuer_id",
                "issuer_parameter_digest",
                "issuer_parameter_id",
                "issuer_policy_epoch",
                "issuer_policy_record_digest",
                "policy_id",
            }
        ),
        "orchard-halo2-actions-v1": frozenset(
            {
                "action_count",
                "action_kind",
                "anchor",
                "anchor_epoch",
                "asset_definition_id",
                "expiry_height",
                "pool_id",
                "public_balance_scope",
            }
        ),
        "monero-fcmp-plus-plus-v1": frozenset(
            {
                "action_kind",
                "asset_definition_id",
                "input_count",
                "output_count",
                "pool_id",
                "root_epoch",
            }
        ),
        "iroha-ivm-private-note-stark-v1": frozenset(
            {
                "action_digest",
                "action_kind",
                "asset_definition_id",
                "execution_epoch",
                "nullifier_count",
                "output_count",
                "pool_id",
                "program_id",
                "public_balance_scope",
                "root_epoch",
                "state_root",
            }
        ),
        "pq-masp-stark-v0": frozenset(
            {
                "action_kind",
                "anchor",
                "anchor_epoch",
                "asset_definition_id",
                "authorization_epoch",
                "authorization_key_digest",
                "note_encryption_key_digest",
                "nullifier_count",
                "output_count",
                "pool_id",
            }
        ),
    }
)
_NATIVE_INSPECTOR: Mapping[str, str] = MappingProxyType(
    {
        "zk-ace-pq-authorization-v0": "inspect_signed_privacy_zk_ace_transfer_action_v1",
        "anonymous-pgc-k-out-of-n-v1": "inspect_signed_privacy_anonymous_pgc_payment_action_v1",
        VERANGE_PROTOCOL: "inspect_signed_privacy_verange_action_v1",
        "vega-existing-credential-zk-v0": "inspect_signed_privacy_vega_action_v1",
        "iroha-jindo-polynomial-commitment-v0": "inspect_signed_privacy_jindo_action_v1",
        "iroha-bootle-lantern-anoncred-v1": "inspect_signed_privacy_bootle_lantern_presentation_action_v1",
        "orchard-halo2-actions-v1": "inspect_signed_privacy_orchard_note_action_v1",
        "monero-fcmp-plus-plus-v1": "inspect_signed_privacy_fcmp_membership_payment_action_v1",
        "iroha-ivm-private-note-stark-v1": "inspect_signed_privacy_ivm_private_note_action_v1",
        "pq-masp-stark-v0": "inspect_signed_privacy_pq_masp_note_action_v1",
    }
)
# These constants describe the constructor/inspector schema only.  They are
# never recorded as observed ledger effects and never contribute to the
# authenticated action identity below.
_NATIVE_DECLARED_ACTION_SHAPES: Mapping[
    str, tuple[str | None, str | None]
] = MappingProxyType(
    {
        "zk-ace-pq-authorization-v0": (
            "authorization_action",
            "zk_ace_transparent_transfer",
        ),
        "anonymous-pgc-k-out-of-n-v1": (
            "payment_action",
            "anonymous_pgc_account_state_transition",
        ),
        VERANGE_PROTOCOL: ("action_verification_and_finality_only", None),
        "vega-existing-credential-zk-v0": (
            "action_verification_and_finality_only",
            None,
        ),
        "iroha-jindo-polynomial-commitment-v0": (None, None),
        "iroha-bootle-lantern-anoncred-v1": ("presentation_action", None),
        "orchard-halo2-actions-v1": (
            "note_action",
            "orchard_note_state_transition",
        ),
        "monero-fcmp-plus-plus-v1": ("payment_action", "fcmp_membership_payment"),
        "iroha-ivm-private-note-stark-v1": (
            "note_action",
            "ivm_private_note_state_transition",
        ),
        "pq-masp-stark-v0": ("note_action", "pq_masp_note_state_transition"),
    }
)
_NATIVE_CONTEXT_FIELDS = frozenset(
    {
        "authority",
        "authority_public_key",
        "creation_time_millis",
        "fee_payment",
        "metadata",
        "network_id",
        "nonce",
        "statement_action_index",
        "statement_network_id",
        "transaction_hash",
        "ttl_millis",
    }
)


def _native_digest(value: object, label: str) -> str:
    if not isinstance(value, bytes) or len(value) != 32 or value == bytes(32):
        _fail(f"native action inspection returned an invalid {label}")
    return value.hex()


def _native_uint(value: object, label: str, *, positive: bool = False) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < (1 if positive else 0)
        or value > 0xFFFF_FFFF
    ):
        _fail(f"native action inspection returned an invalid {label}")
    return value


def _native_u64(value: object, label: str, *, positive: bool = False) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < (1 if positive else 0)
        or value > 0xFFFF_FFFF_FFFF_FFFF
    ):
        _fail(f"native action inspection returned an invalid {label}")
    return value


def _driver_seed_v1(request: Exact12ActionRequest | VeRangeActionRequest, purpose: int) -> bytes:
    request_id = bytes.fromhex(
        str(action_ipc.validate_request(request.canonical_bytes())["request_id"])
    )
    candidate = bytes.fromhex(request.candidate_binding_sha256)
    seed = hashlib.sha256(
        DRIVER_SEED_DOMAIN + candidate + request_id + bytes([purpose])
    ).digest()
    return (b"\x01" + seed[1:]) if seed == bytes(32) else seed


def _network_seed_v1(master: bytes, purpose: bytes, index: int = 0) -> bytes:
    seed = hashlib.sha256(
        b"iroha.privacy.release.network-action.seed.v1"
        + master
        + len(purpose).to_bytes(8, "big")
        + purpose
        + bytes([index])
    ).digest()
    return (b"\x01" + seed[1:]) if seed == bytes(32) else seed


def _utc_date_from_timestamp_millis_v1(timestamp_millis: int) -> tuple[int, int, int]:
    try:
        value = date(1970, 1, 1) + timedelta(days=timestamp_millis // 86_400_000)
    except (OverflowError, ValueError) as exc:
        raise SealedPrivacyControllerError(
            "native Vega request timestamp is outside the admitted UTC date range"
        ) from exc
    return value.year, value.month, value.day


def _identity_json_v1(value: object) -> object:
    if isinstance(value, bytes):
        return value.hex()
    if isinstance(value, list):
        return [_identity_json_v1(item) for item in value]
    if value is None or isinstance(value, (str, int, bool)):
        return value
    _fail("native action inspection returned a noncanonical identity value")


def _inspect_native_action(
    action: ActionArtifact,
    request: Exact12ActionRequest | VeRangeActionRequest,
) -> NativeActionInspection:
    """Decode, authenticate, and exact-check one operation-selected action."""

    protocol = (
        request.protocol
        if isinstance(request, Exact12ActionRequest)
        else VERANGE_PROTOCOL
    )
    operation = CONSTRUCTIBLE_OPERATIONS.get(protocol)
    inspector_name = _NATIVE_INSPECTOR.get(protocol)
    if operation is None or inspector_name is None:
        _fail("native action inspection selected a nonconstructible protocol operation")
    if action.operation != operation or action.protocol != protocol:
        _fail("native action artifact differs from its operation-selected request")
    expected_availability = action_ipc.protocol_status(protocol)
    expected_limitations = action_ipc.protocol_limitations(protocol)
    if (
        action.availability != expected_availability
        or action.limitations != expected_limitations
        or action.network_outcome_authoritative is not False
        or action.qualification_scope != action_ipc.QUALIFICATION_SCOPE
    ):
        _fail("native action artifact overstates its construction-only qualification scope")
    try:
        from iroha_python import crypto as native_crypto

        inspector = getattr(native_crypto, inspector_name)
        network_id = native_crypto.NetworkId.from_bytes(
            bytes.fromhex(request.network_id_hex)
        )
        context_value = (
            native_crypto.inspect_privacy_exact12_action_driver_transaction_context_v1(
                action.transaction,
                bytes.fromhex(request.candidate_binding_sha256),
                bytes.fromhex(action.request_id),
                network_id,
                request.creation_time_millis,
                request.ttl_millis,
                request.nonce,
            )
        )
        value = inspector(action.transaction)
    except (ImportError, AttributeError) as exc:
        raise SealedPrivacyControllerError(
            f"native {protocol} transaction inspection is unavailable"
        ) from exc
    except (RuntimeError, TypeError, ValueError) as exc:
        raise SealedPrivacyControllerError(
            f"native {protocol} transaction inspection rejected the action bytes"
        ) from exc
    if type(context_value) is not dict:
        _fail("native action-driver context inspection did not return one typed record")
    _exact_fields(
        context_value,
        set(_NATIVE_CONTEXT_FIELDS),
        "native action-driver context inspection",
    )
    if type(value) is not dict:
        _fail(f"native {protocol} inspection did not return one typed record")
    expected_fields = (
        _NATIVE_JINDO_FIELDS
        if protocol == "iroha-jindo-polynomial-commitment-v0"
        else _NATIVE_COMMON_FIELDS | _NATIVE_EXTRA_FIELDS[protocol]
    )
    _exact_fields(value, set(expected_fields), f"native {protocol} action inspection")

    transaction_hash = _native_digest(value["transaction_hash"], "transaction hash")
    transaction_intent = _native_digest(
        value["transaction_intent_digest"], "transaction intent digest"
    )
    statement_digest = _native_digest(value["statement_digest"], "statement digest")
    proof_envelope_hash = _native_digest(
        value["proof_envelope_hash"], "proof envelope hash"
    )
    statement_bytes = _native_uint(
        value["statement_bytes"], "statement bytes", positive=True
    )
    proof_bytes = _native_uint(value["proof_bytes"], "proof bytes", positive=True)
    _native_uint(
        value["encoded_proof_envelope_bytes"],
        "encoded proof envelope bytes",
        positive=True,
    )
    _native_uint(
        value["adaptive_signed_transaction_bytes"],
        "adaptive signed transaction bytes",
        positive=True,
    )
    submitted_bytes = _native_uint(
        value["submitted_versioned_transaction_bytes"],
        "submitted transaction bytes",
        positive=True,
    )
    context_transaction_hash = _native_digest(
        context_value["transaction_hash"], "context transaction hash"
    )
    network_id_hex = _native_digest(context_value["network_id"], "NetworkId")
    statement_network_id_hex = _native_digest(
        context_value["statement_network_id"], "statement NetworkId"
    )
    statement_action_index = _native_uint(
        context_value["statement_action_index"], "statement action index"
    )
    context_creation_time = _native_u64(
        context_value["creation_time_millis"], "creation time", positive=True
    )
    context_ttl = _native_u64(context_value["ttl_millis"], "TTL", positive=True)
    context_nonce = _native_uint(context_value["nonce"], "nonce", positive=True)
    authority = context_value["authority"]
    authority_public_key = context_value["authority_public_key"]
    if (
        not isinstance(authority, str)
        or not authority
        or not authority.isascii()
        or len(authority.encode("ascii")) > 1024
        or not isinstance(authority_public_key, bytes)
        or len(authority_public_key) != 32
        or authority_public_key == bytes(32)
    ):
        _fail("native action-driver authority identity is invalid")
    public_artifacts = action.public_admission_artifacts
    if protocol == VERANGE_PROTOCOL:
        if public_artifacts is None:
            _fail("native VeRange action omitted its public admission artifacts")
        if (
            public_artifacts.action_authority_account_id != authority
            or public_artifacts.action_authority_public_key_hex
            != authority_public_key.hex()
            or public_artifacts.protocol_id != VERANGE_PROTOCOL
            or public_artifacts.proof_system_id != "iroha-verange-p256"
            or public_artifacts.engine_id != "native-verange-p256"
            or public_artifacts.max_aggregation_count != 8
            or public_artifacts.schema
            != action_ipc.VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA
            or public_artifacts.schema_version
            != action_ipc.VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION
            or not public_artifacts.compiled_profile_norito
            or len(public_artifacts.compiled_profile_norito)
            > action_ipc.MAX_COMPILED_PROFILE_BYTES
            or hashlib.sha256(public_artifacts.compiled_profile_norito).hexdigest()
            != public_artifacts.compiled_profile_sha256
        ):
            _fail("native VeRange public artifacts differ from the signed action identity")
        for label, digest in (
            ("engine manifest", public_artifacts.engine_manifest_digest_hex),
            ("parameter digest", public_artifacts.parameter_digest_hex),
            ("parameter ID", public_artifacts.parameter_id_hex),
            ("policy ID", public_artifacts.policy_id_hex),
            ("statement schema", public_artifacts.statement_schema_digest_hex),
            ("verifier", public_artifacts.verifier_digest_hex),
        ):
            _sha256(digest, f"VeRange public {label}", nonzero=True)
    elif public_artifacts is not None:
        _fail("native non-VeRange action carried unrelated public admission artifacts")
    declared_execution, declared_effect = _NATIVE_DECLARED_ACTION_SHAPES[protocol]
    if (
        submitted_bytes != len(action.transaction)
        or transaction_hash != action.transaction_hash_hex
        or hashlib.sha256(action.transaction).hexdigest() != action.transaction_sha256
        or context_transaction_hash != transaction_hash
        or network_id_hex != request.network_id_hex
        or statement_network_id_hex != request.network_id_hex
        or statement_action_index != 0
        or context_creation_time != request.creation_time_millis
        or context_ttl != request.ttl_millis
        or context_nonce != request.nonce
        or context_value["fee_payment"] != "authority-empty-v1"
        or context_value["metadata"] != "empty-v1"
        or (
            protocol != "iroha-jindo-polynomial-commitment-v0"
            and value["protocol_id"] != protocol
        )
        or (
            declared_execution is not None
            and value["execution_classification"] != declared_execution
        )
        or (
            protocol != "iroha-jindo-polynomial-commitment-v0"
            and value["ledger_effect"] != declared_effect
        )
    ):
        _fail("native action inspection differs from its signed request context")

    if "asset_definition_id" in value and value["asset_definition_id"] != request.asset_definition_id:
        _fail("native action asset differs from its exact request")
    fixture_seed = _driver_seed_v1(request, 1)
    policy_seed = _driver_seed_v1(request, 2)
    pool_seed = _driver_seed_v1(request, 5)
    if protocol in {
        "anonymous-pgc-k-out-of-n-v1",
        "orchard-halo2-actions-v1",
        "monero-fcmp-plus-plus-v1",
        "iroha-ivm-private-note-stark-v1",
    } and _native_digest(value["pool_id"], "pool ID") != pool_seed.hex():
        _fail("native action pool ID differs from its derived request binding")
    if protocol == "pq-masp-stark-v0" and _native_digest(
        value["pool_id"], "pool ID"
    ) != bytes([7] * 32).hex():
        _fail("native PQ-MASP pool differs from its release fixture")
    expected_policy_id = (
        _network_seed_v1(fixture_seed, b"zk-ace-policy-id")
        if protocol == "zk-ace-pq-authorization-v0"
        else policy_seed
    )
    if protocol in {"zk-ace-pq-authorization-v0", VERANGE_PROTOCOL} and _native_digest(
        value["policy_id"], "policy ID"
    ) != expected_policy_id.hex():
        _fail("native action policy ID differs from its derived request binding")
    if (
        protocol == VERANGE_PROTOCOL
        and public_artifacts is not None
        and public_artifacts.policy_id_hex != expected_policy_id.hex()
    ):
        _fail("native VeRange public policy ID differs from its derived request binding")

    if protocol == "zk-ace-pq-authorization-v0":
        destination = value["destination_account_id"]
        if (
            value["action_kind"] != "transparent_transfer"
            or value["amount"] != "19"
            or value["source_account_id"] != authority
            or not isinstance(destination, str)
            or not destination
            or not destination.isascii()
            or len(destination.encode("ascii")) > 1024
            or destination == authority
            or _native_uint(value["authorization_epoch"], "authorization epoch", positive=True)
            != 1
        ):
            _fail("native ZK-ACE statement differs from the selected operation")
    elif protocol == "anonymous-pgc-k-out-of-n-v1":
        if (
            value["action_kind"] != "payment"
            or _native_uint(value["recipient_count"], "recipient count", positive=True) != 2
            or _native_uint(value["anonymity_set_size"], "anonymity set size", positive=True)
            != 16
            or _native_uint(value["account_state_root_epoch"], "account root epoch", positive=True)
            != 1
            or _native_uint(
                value["next_account_state_root_epoch"],
                "next account root epoch",
                positive=True,
            )
            != 2
        ):
            _fail("native Anonymous-PGC statement differs from the selected operation")
    elif protocol == VERANGE_PROTOCOL:
        commitments = value["value_commitments"]
        if (
            value["bit_length"] != 32
            or value["aggregation_count"] != 4
            or not isinstance(commitments, list)
            or len(commitments) != 4
            or any(not isinstance(commitment, bytes) or not commitment for commitment in commitments)
        ):
            _fail("native VeRange statement differs from the selected operation")
    elif protocol == "vega-existing-credential-zk-v0":
        expected_date = _utc_date_from_timestamp_millis_v1(
            request.creation_time_millis
        )
        if (
            value["document_type"] != "org.iso.18013.5.1.mDL"
            or value["namespace"] != "org.iso.18013.5.1"
            or value["digest_algorithm"] != "SHA-256"
            or value["issuer_authentication_algorithm"] != "COSE_Sign1/ES256"
            or value["device_authentication_algorithm"] != "COSE_Sign1/ES256"
            or _native_digest(value["issuer_id"], "issuer ID") != "40" * 32
            or _native_uint(value["issuer_record_epoch"], "issuer epoch", positive=True) != 1
            or (
                value["presentation_year"],
                value["presentation_month"],
                value["presentation_day"],
            )
            != expected_date
            or value["minimum_age_years"] != 18
            or _native_digest(value["reader_challenge"], "reader challenge") != "31" * 32
            or _native_digest(value["session_transcript_digest"], "session transcript")
            != "32" * 32
        ):
            _fail("native Vega statement differs from the release fixture")
    elif protocol == "iroha-jindo-polynomial-commitment-v0":
        if (
            _native_uint(value["polynomial_count"], "polynomial count", positive=True)
            != 4
            or value["availability"] != action_ipc.JINDO_EXPERIMENTAL_STATUS
            or value["limitations"]
            != [action_ipc.MISSING_JINDO_KNOWLEDGE_SOUNDNESS]
        ):
            _fail("native Jindo statement differs from the release fixture")
    elif protocol == "iroha-bootle-lantern-anoncred-v1":
        expected_policy = _network_seed_v1(fixture_seed, b"bootle-policy")
        expected_issuer = _network_seed_v1(fixture_seed, b"bootle-issuer")
        if (
            value["action_kind"] != "presentation"
            or _native_digest(value["policy_id"], "policy ID") != expected_policy.hex()
            or _native_digest(value["issuer_id"], "issuer ID") != expected_issuer.hex()
            or _native_uint(value["issuer_policy_epoch"], "issuer policy epoch", positive=True)
            != 1
            or value["disclosure_indices"] != [1]
            or value["disclosed_attribute_values"] != [bytes([1] * 8)]
        ):
            _fail("native Bootle/Lantern statement differs from the release fixture")
    elif protocol == "orchard-halo2-actions-v1":
        if (
            value["action_kind"] != "note_action"
            or value["action_count"] != 1
            or value["anchor_epoch"] != 1
            or value["expiry_height"] != 1_000_000_000
        ):
            _fail("native Orchard statement differs from the release fixture")
    elif protocol == "monero-fcmp-plus-plus-v1":
        if (
            value["action_kind"] != "membership_payment"
            or _native_uint(value["input_count"], "FCMP input count", positive=True) == 0
            or _native_uint(value["output_count"], "FCMP output count", positive=True) == 0
        ):
            _fail("native FCMP++ statement differs from the selected operation")
    elif protocol == "iroha-ivm-private-note-stark-v1":
        if (
            value["action_kind"] != "note_action"
            or _native_uint(value["nullifier_count"], "private-IVM nullifier count", positive=True)
            == 0
            or _native_uint(value["output_count"], "private-IVM output count", positive=True)
            == 0
        ):
            _fail("native private-IVM statement differs from the selected operation")
    elif protocol == "pq-masp-stark-v0":
        if (
            value["action_kind"] != "note_action"
            or _native_uint(value["nullifier_count"], "PQ-MASP nullifier count", positive=True)
            == 0
            or _native_uint(value["output_count"], "PQ-MASP output count", positive=True) == 0
        ):
            _fail("native PQ-MASP statement differs from the release fixture")

    unauthenticated_schema_labels = {
        "availability",
        "execution_classification",
        "ledger_effect",
        "limitations",
    }
    identity = {
        "action": {
            key: _identity_json_v1(value[key])
            for key in sorted(value)
            if key not in unauthenticated_schema_labels
        },
        "transaction_context": {
            key: _identity_json_v1(context_value[key]) for key in sorted(context_value)
        },
    }
    identity_sha256 = hashlib.sha256(_canonical_json_bytes(identity)).hexdigest()
    return NativeActionInspection(
        operation,
        protocol,
        transaction_hash,
        transaction_intent,
        statement_digest,
        proof_envelope_hash,
        proof_bytes,
        statement_bytes,
        identity_sha256,
        expected_availability,
        expected_limitations,
        False,
        action_ipc.QUALIFICATION_SCOPE,
    )


def _inspect_native_verange_action(
    action: ActionArtifact,
    request: VeRangeActionRequest,
) -> NativeVeRangeInspection:
    """Use the shared exact operation/context validator for the legacy case API."""

    inspected = _inspect_native_action(action, request)
    return NativeVeRangeInspection(
        inspected.transaction_hash_hex,
        inspected.transaction_intent_digest,
        inspected.statement_digest,
        inspected.proof_envelope_hash,
        _driver_seed_v1(request, 2).hex(),
        inspected.proof_bytes,
        inspected.statement_bytes,
    )


def _parse_action_response(
    response_bytes: bytes,
    request_bytes: bytes,
) -> ActionArtifact:
    try:
        request = action_ipc.validate_request(request_bytes)
        response = action_ipc.validate_response(
            response_bytes,
            expected_request=request,
        )
    except action_ipc.PrivacyActionDriverIpcError as exc:
        raise SealedPrivacyControllerError(str(exc)) from exc
    public_artifacts_value = response["public_admission_artifacts"]
    public_artifacts = (
        VeRangePublicAdmissionArtifactsV1.from_ipc(public_artifacts_value)
        if isinstance(public_artifacts_value, Mapping)
        else None
    )
    return ActionArtifact(
        availability=str(response["availability"]),
        limitations=tuple(response["limitations"]),
        network_outcome_authoritative=bool(response["network_outcome_authoritative"]),
        qualification_scope=str(response["qualification_scope"]),
        request_id=str(response["request_id"]),
        request_bytes=request_bytes,
        response_bytes=response_bytes,
        transaction=bytes(response["transaction_norito"]),
        transaction_hash_hex=str(response["transaction_hash_hex"]),
        transaction_sha256=str(response["transaction_sha256"]),
        operation=str(response["operation"]),
        protocol=str(response["protocol"]),
        public_admission_artifacts=public_artifacts,
    )


def _driver_stat_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_gid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _validate_driver_metadata(metadata: os.stat_result, label: str) -> None:
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or not 1 <= metadata.st_size <= MAX_ACTION_DRIVER_BYTES
        or metadata.st_mode & 0o111 == 0
        or metadata.st_mode & 0o222 != 0
    ):
        _fail(
            f"{label} must be one non-writable, singly linked executable regular file"
        )


def _hash_driver_descriptor(descriptor: int, label: str) -> tuple[os.stat_result, str]:
    before = os.fstat(descriptor)
    _validate_driver_metadata(before, label)
    digest = hashlib.sha256()
    offset = 0
    while offset < before.st_size:
        try:
            chunk = os.pread(
                descriptor,
                min(1024 * 1024, before.st_size - offset),
                offset,
            )
        except OSError as exc:
            raise SealedPrivacyControllerError(
                f"cannot hash {label} through its pinned descriptor"
            ) from exc
        if not chunk:
            _fail(f"{label} ended while its pinned bytes were being hashed")
        digest.update(chunk)
        offset += len(chunk)
    after = os.fstat(descriptor)
    if _driver_stat_identity(before) != _driver_stat_identity(after):
        _fail(f"{label} changed while its pinned bytes were being hashed")
    return after, digest.hexdigest()


def _canonical_work_directory(path: Path) -> Path:
    if not path.is_absolute():
        _fail("action-driver work directory must be absolute")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as exc:
        raise SealedPrivacyControllerError(
            f"cannot inspect action-driver work directory: {exc}"
        ) from exc
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077 != 0
    ):
        _fail("action-driver work directory must be canonical and owner-private")
    return path


@dataclass
class PinnedActionDriver:
    """One admitted executable identity retained through a single invocation."""

    source_path: Path
    source_descriptor: int
    source_identity: tuple[int, ...]
    sha256: str
    execution_path: str
    inherited_descriptors: tuple[int, ...]
    execution_method: str
    private_directory: Path | None = None
    private_directory_descriptor: int = -1
    private_file_descriptor: int = -1
    private_file_identity: tuple[int, ...] = ()
    private_directory_identity: tuple[int, int] = ()

    def validate(self) -> None:
        metadata, digest = _hash_driver_descriptor(
            self.source_descriptor, "action driver"
        )
        try:
            path_metadata = os.stat(self.source_path, follow_symlinks=False)
        except OSError as exc:
            raise SealedPrivacyControllerError(
                "action-driver source path disappeared after admission"
            ) from exc
        if (
            _driver_stat_identity(metadata) != self.source_identity
            or _driver_stat_identity(path_metadata) != self.source_identity
            or digest != self.sha256
        ):
            _fail("action-driver source identity changed after admission")
        if self.private_directory is None:
            try:
                execution_metadata = os.stat(self.execution_path)
            except OSError as exc:
                raise SealedPrivacyControllerError(
                    "pinned Linux action-driver descriptor path is unavailable"
                ) from exc
            if _driver_stat_identity(execution_metadata) != self.source_identity:
                _fail("pinned Linux action-driver descriptor identifies different bytes")
            return
        copied_metadata, copied_digest = _hash_driver_descriptor(
            self.private_file_descriptor, "private action-driver copy"
        )
        try:
            copied_path_metadata = os.stat(
                self.execution_path, follow_symlinks=False
            )
            directory_metadata = os.fstat(self.private_directory_descriptor)
            directory_path_metadata = os.stat(
                self.private_directory, follow_symlinks=False
            )
        except OSError as exc:
            raise SealedPrivacyControllerError(
                "private Darwin action-driver execution identity is unavailable"
            ) from exc
        if (
            _driver_stat_identity(copied_metadata) != self.private_file_identity
            or _driver_stat_identity(copied_path_metadata) != self.private_file_identity
            or copied_digest != self.sha256
            or (directory_metadata.st_dev, directory_metadata.st_ino)
            != self.private_directory_identity
            or (directory_path_metadata.st_dev, directory_path_metadata.st_ino)
            != self.private_directory_identity
            or directory_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(directory_metadata.st_mode) != 0o500
        ):
            _fail("private Darwin action-driver execution copy changed after admission")

    def close(self) -> None:
        cleanup_error: BaseException | None = None
        if self.private_directory is not None:
            try:
                file_path_metadata = os.stat(
                    "privacy_exact12_action_driver",
                    dir_fd=self.private_directory_descriptor,
                    follow_symlinks=False,
                )
                directory_path_metadata = os.stat(
                    self.private_directory, follow_symlinks=False
                )
                if (
                    _driver_stat_identity(file_path_metadata)
                    != self.private_file_identity
                    or (
                        directory_path_metadata.st_dev,
                        directory_path_metadata.st_ino,
                    )
                    != self.private_directory_identity
                ):
                    _fail("private Darwin action-driver cleanup identity changed")
                os.fchmod(self.private_directory_descriptor, 0o700)
                os.unlink(
                    "privacy_exact12_action_driver",
                    dir_fd=self.private_directory_descriptor,
                )
            except BaseException as exc:
                cleanup_error = exc
            for descriptor_name in (
                "private_file_descriptor",
                "private_directory_descriptor",
            ):
                descriptor = getattr(self, descriptor_name)
                if descriptor >= 0:
                    os.close(descriptor)
                    setattr(self, descriptor_name, -1)
            if cleanup_error is None:
                try:
                    os.rmdir(self.private_directory)
                except BaseException as exc:
                    cleanup_error = exc
        if self.source_descriptor >= 0:
            os.close(self.source_descriptor)
            self.source_descriptor = -1
        if cleanup_error is not None:
            raise SealedPrivacyControllerError(
                "cannot remove the private Darwin action-driver execution copy"
            ) from cleanup_error


def _copy_driver_for_darwin(
    snapshot: PinnedActionDriver,
    work_directory: Path,
) -> None:
    private_directory = Path(
        tempfile.mkdtemp(prefix=".privacy-action-driver-", dir=work_directory)
    )
    directory_descriptor = -1
    file_descriptor = -1
    created_file = False
    try:
        directory_flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        directory_descriptor = os.open(private_directory, directory_flags)
        directory_metadata = os.fstat(directory_descriptor)
        if (
            not stat.S_ISDIR(directory_metadata.st_mode)
            or directory_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(directory_metadata.st_mode) != 0o700
        ):
            _fail("private Darwin action-driver directory is unsafe")
        write_flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        writer = os.open(
            "privacy_exact12_action_driver",
            write_flags,
            0o400,
            dir_fd=directory_descriptor,
        )
        created_file = True
        try:
            offset = 0
            while offset < snapshot.source_identity[6]:
                chunk = os.pread(
                    snapshot.source_descriptor,
                    min(1024 * 1024, snapshot.source_identity[6] - offset),
                    offset,
                )
                if not chunk:
                    _fail("action driver ended during its private Darwin copy")
                view = memoryview(chunk)
                while view:
                    written = os.write(writer, view)
                    if written <= 0:
                        _fail("private Darwin action-driver copy made no progress")
                    view = view[written:]
                offset += len(chunk)
            os.fsync(writer)
            os.fchmod(writer, 0o500)
            os.fsync(writer)
        finally:
            os.close(writer)
        read_flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        file_descriptor = os.open(
            "privacy_exact12_action_driver",
            read_flags,
            dir_fd=directory_descriptor,
        )
        copied_metadata, copied_digest = _hash_driver_descriptor(
            file_descriptor, "private action-driver copy"
        )
        copied_path_metadata = os.stat(
            "privacy_exact12_action_driver",
            dir_fd=directory_descriptor,
            follow_symlinks=False,
        )
        if (
            _driver_stat_identity(copied_metadata)
            != _driver_stat_identity(copied_path_metadata)
            or copied_digest != snapshot.sha256
        ):
            _fail("private Darwin action-driver copy differs from admitted bytes")
        os.fchmod(directory_descriptor, 0o500)
        os.fsync(directory_descriptor)
        snapshot.execution_path = str(
            private_directory / "privacy_exact12_action_driver"
        )
        snapshot.execution_method = "darwin-private-fd-copy"
        snapshot.private_directory = private_directory
        snapshot.private_directory_descriptor = directory_descriptor
        snapshot.private_file_descriptor = file_descriptor
        snapshot.private_file_identity = _driver_stat_identity(copied_metadata)
        snapshot.private_directory_identity = (
            directory_metadata.st_dev,
            directory_metadata.st_ino,
        )
        directory_descriptor = -1
        file_descriptor = -1
    except BaseException:
        if file_descriptor >= 0:
            os.close(file_descriptor)
        if directory_descriptor >= 0:
            try:
                os.fchmod(directory_descriptor, 0o700)
                if created_file:
                    os.unlink(
                        "privacy_exact12_action_driver",
                        dir_fd=directory_descriptor,
                    )
            finally:
                os.close(directory_descriptor)
        try:
            os.rmdir(private_directory)
        except OSError:
            pass
        raise


def _admit_action_driver(
    executable: Path,
    expected_sha256: str,
    work_directory: Path,
) -> PinnedActionDriver:
    expected_sha256 = _sha256(
        expected_sha256, "expected action-driver digest", nonzero=True
    )
    work_directory = _canonical_work_directory(work_directory)
    if sys.platform not in {"darwin", "linux"}:
        _fail("pinned action-driver execution supports only Darwin and Linux")
    if not executable.is_absolute():
        _fail("action driver must be an absolute path")
    try:
        resolved = executable.resolve(strict=True)
        path_before = executable.lstat()
    except OSError as exc:
        raise SealedPrivacyControllerError(f"cannot inspect action driver: {exc}") from exc
    if resolved != executable or stat.S_ISLNK(path_before.st_mode):
        _fail("action driver must have one canonical non-symlink path")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(executable, flags)
    except OSError as exc:
        raise SealedPrivacyControllerError(
            f"cannot open action driver without following links: {exc}"
        ) from exc
    snapshot: PinnedActionDriver | None = None
    try:
        metadata, digest = _hash_driver_descriptor(descriptor, "action driver")
        path_after = os.stat(executable, follow_symlinks=False)
        if (
            _driver_stat_identity(path_before) != _driver_stat_identity(metadata)
            or _driver_stat_identity(path_after) != _driver_stat_identity(metadata)
        ):
            _fail("action-driver path changed while it was admitted")
        if digest != expected_sha256:
            _fail("action-driver bytes differ from the expected SHA-256")
        snapshot = PinnedActionDriver(
            source_path=executable,
            source_descriptor=descriptor,
            source_identity=_driver_stat_identity(metadata),
            sha256=digest,
            execution_path=f"/proc/self/fd/{descriptor}",
            inherited_descriptors=(descriptor,),
            execution_method="linux-pinned-fd",
        )
        if sys.platform == "linux":
            execution_metadata = os.stat(snapshot.execution_path)
            if _driver_stat_identity(execution_metadata) != snapshot.source_identity:
                _fail("Linux descriptor execution path identifies different bytes")
        else:
            snapshot.execution_path = ""
            snapshot.inherited_descriptors = ()
            _copy_driver_for_darwin(snapshot, work_directory)
        snapshot.validate()
        return snapshot
    except BaseException:
        if snapshot is None:
            os.close(descriptor)
        else:
            snapshot.close()
        raise


@contextmanager
def _pinned_action_driver(
    executable: Path,
    expected_sha256: str,
    work_directory: Path,
) -> Iterator[PinnedActionDriver]:
    snapshot = _admit_action_driver(executable, expected_sha256, work_directory)
    try:
        yield snapshot
    finally:
        try:
            snapshot.validate()
        finally:
            snapshot.close()


def _kill_process_group(process: subprocess.Popen[bytes]) -> None:
    process_group = process.pid
    if process_group <= 1 or process_group == os.getpgrp():
        _fail("action-driver process group is unsafe to terminate")
    try:
        os.killpg(process_group, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except PermissionError as exc:
        raise SealedPrivacyControllerError(
            "cannot terminate the action-driver process group"
        ) from exc
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired as exc:
        raise SealedPrivacyControllerError(
            "action-driver process group did not terminate"
        ) from exc
    deadline = time.monotonic() + 1.0
    while time.monotonic() < deadline:
        try:
            os.killpg(process_group, 0)
        except ProcessLookupError:
            return
        except PermissionError as exc:
            raise SealedPrivacyControllerError(
                "cannot verify action-driver descendant termination"
            ) from exc
        try:
            os.killpg(process_group, signal.SIGKILL)
        except ProcessLookupError:
            return
        time.sleep(0.01)
    _fail("action-driver descendants survived process-group termination")


def invoke_action_driver(
    executable: Path,
    request: Exact12ActionRequest | VeRangeActionRequest,
    *,
    expected_sha256: str,
    work_directory: Path,
    timeout_seconds: float,
) -> ActionArtifact:
    """Invoke exactly the admitted native action-builder bytes once."""

    if timeout_seconds <= 0:
        _fail("action-driver timeout must be positive")
    work_directory = _canonical_work_directory(work_directory)
    expected_sha256 = _sha256(
        expected_sha256, "expected action-driver digest", nonzero=True
    )
    with _pinned_action_driver(
        executable, expected_sha256, work_directory
    ) as pinned_driver:
        request_bytes = bytearray(request.canonical_bytes())
        environment = {
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": "/usr/bin:/bin",
            "TMPDIR": str(work_directory),
        }
        process: subprocess.Popen[bytes] | None = None
        stdout = bytearray()
        stderr = bytearray()
        selector = selectors.DefaultSelector()
        try:
            process = subprocess.Popen(
                [pinned_driver.execution_path],
                cwd=work_directory,
                env=environment,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                close_fds=True,
                pass_fds=pinned_driver.inherited_descriptors,
                start_new_session=True,
            )
            assert process.stdin is not None
            assert process.stdout is not None
            assert process.stderr is not None
            try:
                process_group = os.getpgid(process.pid)
            except ProcessLookupError as exc:
                raise SealedPrivacyControllerError(
                    "native action driver exited before process-group containment"
                ) from exc
            if process_group != process.pid or process_group == os.getpgrp():
                _fail("native action driver did not enter its own process group")
            process.stdin.write(request_bytes)
            process.stdin.close()
            request_bytes[:] = b"\0" * len(request_bytes)
            selector.register(
                process.stdout,
                selectors.EVENT_READ,
                (stdout, MAX_DRIVER_RESPONSE_BYTES),
            )
            selector.register(
                process.stderr,
                selectors.EVENT_READ,
                (stderr, MAX_DRIVER_ERROR_BYTES),
            )
            deadline = time.monotonic() + timeout_seconds
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    _fail("native action driver exceeded its timeout")
                events = selector.select(min(remaining, 0.5))
                if not events and process.poll() is not None:
                    events = selector.select(0)
                for key, _mask in events:
                    target, maximum = key.data
                    chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    target.extend(chunk)
                    if len(target) > maximum:
                        _fail("native action-driver output exceeds its bound")
            remaining = max(0.01, deadline - time.monotonic())
            try:
                status = process.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                _fail("native action driver did not exit after closing its output")
        finally:
            request_bytes[:] = b"\0" * len(request_bytes)
            selector.close()
            if process is not None:
                for stream in (process.stdin, process.stdout, process.stderr):
                    if stream is not None and not stream.closed:
                        stream.close()
                _kill_process_group(process)
        if status != 0:
            message = bytes(stderr).decode("utf-8", "replace").strip()
            _fail(f"native action driver exited with status {status}: {message}")
        if stderr:
            _fail("successful native action driver emitted unexpected stderr")
        artifact = _parse_action_response(bytes(stdout), request.canonical_bytes())
        return ActionArtifact(
            availability=artifact.availability,
            limitations=artifact.limitations,
            network_outcome_authoritative=artifact.network_outcome_authoritative,
            qualification_scope=artifact.qualification_scope,
            request_id=artifact.request_id,
            request_bytes=artifact.request_bytes,
            response_bytes=artifact.response_bytes,
            transaction=artifact.transaction,
            transaction_hash_hex=artifact.transaction_hash_hex,
            transaction_sha256=artifact.transaction_sha256,
            action_driver_sha256=pinned_driver.sha256,
            operation=artifact.operation,
            protocol=artifact.protocol,
            public_admission_artifacts=artifact.public_admission_artifacts,
        )


@dataclass(frozen=True)
class PeerEndpoint:
    label: str
    root: str

    def __post_init__(self) -> None:
        parsed = urllib.parse.urlsplit(self.root)
        try:
            port = parsed.port
        except ValueError as exc:
            raise SealedPrivacyControllerError("peer endpoint port is invalid") from exc
        if (
            not re.fullmatch(r"peer-[1-4]", self.label)
            or parsed.scheme != "http"
            or parsed.hostname != "127.0.0.1"
            or port is None
            or not 1 <= port <= 65535
            or parsed.username is not None
            or parsed.password is not None
            or parsed.path not in ("", "/")
            or parsed.query
            or parsed.fragment
            or self.root != f"http://127.0.0.1:{port}"
        ):
            _fail("peer endpoint must be one canonical credential-free loopback Torii root")


def require_exact_peer_set(peers: Sequence[PeerEndpoint]) -> tuple[PeerEndpoint, ...]:
    values = tuple(peers)
    if len(values) != PEER_COUNT:
        _fail("sealed privacy controller requires exactly four direct peers")
    if tuple(peer.label for peer in values) != tuple(
        f"peer-{index}" for index in range(1, PEER_COUNT + 1)
    ):
        _fail("direct peer rows are missing, duplicated, or reordered")
    if len({peer.root for peer in values}) != PEER_COUNT:
        _fail("direct peer Torii roots must be distinct")
    return values


@dataclass(frozen=True)
class HttpObservation:
    status: int
    headers: Mapping[str, str]
    body: bytes


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):  # noqa: ANN001
        return None


def _direct_exchange(
    peer: PeerEndpoint,
    method: str,
    path: str,
    *,
    body: bytes | None,
    timeout_seconds: float,
    accept: str = "application/json",
) -> HttpObservation:
    """Perform one direct, non-redirecting, credential-free Torii request."""

    if (
        method not in {"GET", "POST"}
        or not path.startswith("/")
        or "#" in path
        or accept not in {"application/json", PRIVACY_CAPABILITIES_MEDIA_TYPE}
    ):
        _fail("controller attempted an unsupported direct Torii request")
    headers = {"Accept": accept}
    if body is not None:
        headers["Content-Type"] = "application/x-norito"
    request = urllib.request.Request(
        peer.root + path,
        method=method,
        data=body,
        headers=headers,
    )
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({}),
        _NoRedirect(),
    )
    try:
        response = opener.open(request, timeout=timeout_seconds)
    except urllib.error.HTTPError as exc:
        response = exc
    except (OSError, TimeoutError, urllib.error.URLError) as exc:
        raise TransientPeerError(f"{peer.label} direct Torii request failed") from exc
    try:
        length = response.headers.get("Content-Length")
        if length is not None:
            try:
                announced = int(length)
            except ValueError as exc:
                raise SealedPrivacyControllerError(
                    f"{peer.label} returned a noncanonical Content-Length"
                ) from exc
            if announced < 0 or announced > MAX_HTTP_RESPONSE_BYTES:
                _fail(f"{peer.label} response exceeds its announced bound")
        response_body = response.read(MAX_HTTP_RESPONSE_BYTES + 1)
        if len(response_body) > MAX_HTTP_RESPONSE_BYTES:
            _fail(f"{peer.label} direct Torii response exceeds its bound")
        response_headers = {
            str(key).lower(): str(value) for key, value in response.headers.items()
        }
        return HttpObservation(int(response.status), response_headers, response_body)
    finally:
        response.close()


@dataclass(frozen=True)
class FleetSample:
    height: int
    block_hash: str


class TranscriptBuilder:
    def __init__(
        self,
        case: str,
        candidate_binding_sha256: str,
        action_driver_sha256: str,
    ) -> None:
        self.case = case
        self.candidate_binding_sha256 = _sha256(
            candidate_binding_sha256, "candidate binding", nonzero=True
        )
        self.action_driver_sha256 = _sha256(
            action_driver_sha256, "action-driver digest", nonzero=True
        )
        self.events: list[dict[str, object]] = []
        self._encoded_event_bytes = 0

    def event(self, kind: str, **fields: object) -> None:
        if len(self.events) >= MAX_TRANSCRIPT_EVENTS:
            _fail("sealed case transcript exceeds its event bound")
        row = {"index": len(self.events), "kind": kind, **fields}
        encoded_size = len(_canonical_json_bytes(row))
        if self._encoded_event_bytes + encoded_size > MAX_TRANSCRIPT_BYTES:
            _fail("sealed case transcript exceeds its incremental byte bound")
        self._encoded_event_bytes += encoded_size
        self.events.append(row)

    def http(
        self,
        peer: PeerEndpoint,
        method: str,
        path: str,
        request_body: bytes | None,
        response: HttpObservation,
    ) -> None:
        self.event(
            "direct-peer-http",
            method=method,
            path=path,
            peer=peer.label,
            request_sha256=(
                hashlib.sha256(request_body).hexdigest()
                if request_body is not None
                else hashlib.sha256(b"").hexdigest()
            ),
            request_size=len(request_body or b""),
            response_base64=base64.b64encode(response.body).decode("ascii"),
            response_content_type=response.headers.get("content-type", ""),
            response_sha256=hashlib.sha256(response.body).hexdigest(),
            response_size=len(response.body),
            status=response.status,
        )

    def finish(self) -> tuple[bytes, str]:
        body: dict[str, object] = {
            "action_driver_sha256": self.action_driver_sha256,
            "candidate_binding_sha256": self.candidate_binding_sha256,
            "case": self.case,
            "events": self.events,
            "schema": TRANSCRIPT_SCHEMA,
            "schema_version": CONTROLLER_SCHEMA_VERSION,
        }
        identifier = hashlib.sha256(
            TRANSCRIPT_ID_DOMAIN + _canonical_json_bytes(body)
        ).hexdigest()
        payload = _canonical_json_bytes({**body, "transcript_id": identifier})
        if len(payload) > MAX_TRANSCRIPT_BYTES:
            _fail("sealed case transcript exceeds its canonical byte bound")
        return payload, identifier


def bind_verange_qualification_case_plan_v1(
    transcript: TranscriptBuilder,
    plan: VeRangeQualificationCasePlanV1,
) -> None:
    """Bind one non-executing source/reset/supervisor plan to a transcript."""

    if transcript.candidate_binding_sha256 != plan.candidate_binding_sha256:
        _fail("VeRange qualification plan differs from the transcript candidate")
    transcript.event("verange-qualification-case-plan", **plan.transcript_fields())


def _controller_exchange(
    transcript: TranscriptBuilder,
    peer: PeerEndpoint,
    method: str,
    path: str,
    *,
    body: bytes | None = None,
    timeout_seconds: float = 2.0,
    accept: str = "application/json",
) -> HttpObservation:
    if accept == "application/json":
        # Preserve the original narrow call shape for existing diagnostic test
        # doubles; only the capability query opens the exact Norito accept path.
        response = _direct_exchange(
            peer,
            method,
            path,
            body=body,
            timeout_seconds=timeout_seconds,
        )
    else:
        response = _direct_exchange(
            peer,
            method,
            path,
            body=body,
            timeout_seconds=timeout_seconds,
            accept=accept,
        )
    transcript.http(peer, method, path, body, response)
    return response


def _json_http(response: HttpObservation, label: str) -> dict[str, object]:
    media_type = response.headers.get("content-type", "").split(";", 1)[0].strip().lower()
    if media_type != "application/json":
        _fail(f"{label} must use Content-Type application/json")
    return _json_object(response.body, label, MAX_HTTP_RESPONSE_BYTES)


def _normalized_block_hash(value: object, label: str) -> str:
    if not isinstance(value, str):
        _fail(f"{label} is not a block hash")
    match = BLOCK_HASH_RE.fullmatch(value)
    if match is None:
        _fail(f"{label} is not a canonical block hash")
    return match.group(1).lower()


def _uint(value: object, label: str, *, positive: bool = False) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < (1 if positive else 0)
    ):
        _fail(f"{label} is not a canonical unsigned integer")
    return value


_NATIVE_CAPABILITY_TUPLE_FIELDS = {
    "activation_state",
    "committed_height",
    "compiled_profile_status",
    "engine_id",
    "engine_manifest_digest",
    "limitation",
    "manifest_digest",
    "network_available",
    "operation_schema",
    "parameter_digest",
    "parameter_id",
    "privacy_feature_mask",
    "proof_system_id",
    "protocol_id",
    "readiness",
    "statement_schema_digest",
    "unavailable_reason",
    "verifier_digest",
    "execution_mode",
}


@dataclass(frozen=True)
class VeRangeCapabilityStateV1:
    """Typed facts from one native-validated committed capability manifest."""

    committed_height: int
    manifest_digest_hex: str
    manifest_archive_sha256: str
    capability_binding_sha256: str
    engine_id: str
    engine_manifest_digest_hex: str
    parameter_digest_hex: str
    parameter_id_hex: str
    proof_system_id: str
    statement_schema_digest_hex: str
    verifier_digest_hex: str


def _native_capability_digest(value: object, label: str) -> str:
    if not isinstance(value, bytes) or len(value) != 32 or value == bytes(32):
        _fail(f"native VeRange capability returned an invalid {label}")
    return value.hex()


def _parse_verange_capability_state(
    response: HttpObservation,
    peer: PeerEndpoint,
    public_artifacts: VeRangePublicAdmissionArtifactsV1,
) -> VeRangeCapabilityStateV1:
    """Validate one bounded canonical manifest and extract committed VeRange admission state."""

    if response.status != 200:
        raise TransientPeerError(f"{peer.label} privacy capabilities are unavailable")
    media_type = response.headers.get("content-type", "").split(";", 1)[0].strip().lower()
    if media_type != PRIVACY_CAPABILITIES_MEDIA_TYPE:
        _fail(
            f"{peer.label} privacy capabilities must use Content-Type "
            f"{PRIVACY_CAPABILITIES_MEDIA_TYPE}"
        )
    if not response.body or len(response.body) > MAX_HTTP_RESPONSE_BYTES:
        _fail(f"{peer.label} privacy capability archive violates its byte bound")
    try:
        from iroha_python import crypto as native_crypto

        manifest = native_crypto.privacy_exact12_capability_manifest_v1(response.body)
        row = manifest.require_network_capability(VERANGE_PROTOCOL)
        canonical_archive = bytes(manifest.canonical_archive)
        manifest_digest = bytes(manifest.manifest_digest)
        version = manifest.version
        committed_height = manifest.committed_height
    except (ImportError, AttributeError) as exc:
        raise SealedPrivacyControllerError(
            "native canonical Exact12 capability-manifest validation is unavailable"
        ) from exc
    except (RuntimeError, TypeError, ValueError) as exc:
        raise SealedPrivacyControllerError(
            f"{peer.label} canonical Exact12 capability manifest was rejected"
        ) from exc
    if canonical_archive != response.body or version != 1:
        _fail(f"{peer.label} privacy capability archive is not canonical v1")
    if not isinstance(row, dict):
        _fail(f"{peer.label} native VeRange capability is not one typed row")
    _exact_fields(row, _NATIVE_CAPABILITY_TUPLE_FIELDS, f"{peer.label} VeRange capability")
    manifest_digest_hex = _native_capability_digest(
        manifest_digest, "manifest digest"
    )
    row_manifest_digest_hex = _native_capability_digest(
        row["manifest_digest"], "row manifest digest"
    )
    height = _uint(committed_height, f"{peer.label} capability height", positive=True)
    if (
        row_manifest_digest_hex != manifest_digest_hex
        or _uint(row["committed_height"], f"{peer.label} row capability height", positive=True)
        != height
        or row["protocol_id"] != VERANGE_PROTOCOL
        or row["operation_schema"] != "verange_range_proof_v1"
        or row["execution_mode"] != "component"
        or row["privacy_feature_mask"] != 1
        or row["readiness"] != "available"
        or row["unavailable_reason"] is not None
        or row["activation_state"] != "active"
        or row["network_available"] is not True
        or row["limitation"] is not None
        or row["compiled_profile_status"] != "available"
        or row["proof_system_id"] != "iroha-verange-p256"
        or row["engine_id"] != "native-verange-p256"
    ):
        _fail(f"{peer.label} VeRange capability is not actively admitted")
    bindings = {
        label: _native_capability_digest(row[field], label)
        for field, label in (
            ("parameter_id", "parameter ID"),
            ("parameter_digest", "parameter digest"),
            ("verifier_digest", "verifier digest"),
            ("statement_schema_digest", "statement schema digest"),
            ("engine_manifest_digest", "engine manifest digest"),
        )
    }
    if (
        bindings["parameter ID"] != public_artifacts.parameter_id_hex
        or bindings["parameter digest"] != public_artifacts.parameter_digest_hex
        or bindings["verifier digest"] != public_artifacts.verifier_digest_hex
        or bindings["statement schema digest"]
        != public_artifacts.statement_schema_digest_hex
        or bindings["engine manifest digest"]
        != public_artifacts.engine_manifest_digest_hex
        or row["proof_system_id"] != public_artifacts.proof_system_id
        or row["engine_id"] != public_artifacts.engine_id
    ):
        _fail(f"{peer.label} committed VeRange profile differs from the driver artifact")
    observed_binding = {
        "activation_state": row["activation_state"],
        "compiled_profile_status": row["compiled_profile_status"],
        "engine_id": row["engine_id"],
        "engine_manifest_digest_hex": bindings["engine manifest digest"],
        "execution_mode": row["execution_mode"],
        "limitation": row["limitation"],
        "network_available": row["network_available"],
        "operation_schema": row["operation_schema"],
        "parameter_digest_hex": bindings["parameter digest"],
        "parameter_id_hex": bindings["parameter ID"],
        "privacy_feature_mask": row["privacy_feature_mask"],
        "proof_system_id": row["proof_system_id"],
        "protocol_id": row["protocol_id"],
        "readiness": row["readiness"],
        "statement_schema_digest_hex": bindings["statement schema digest"],
        "unavailable_reason": row["unavailable_reason"],
        "verifier_digest_hex": bindings["verifier digest"],
    }
    return VeRangeCapabilityStateV1(
        committed_height=height,
        manifest_digest_hex=manifest_digest_hex,
        manifest_archive_sha256=hashlib.sha256(response.body).hexdigest(),
        capability_binding_sha256=hashlib.sha256(
            _canonical_json_bytes(observed_binding)
        ).hexdigest(),
        engine_id=str(row["engine_id"]),
        engine_manifest_digest_hex=bindings["engine manifest digest"],
        parameter_digest_hex=bindings["parameter digest"],
        parameter_id_hex=bindings["parameter ID"],
        proof_system_id=str(row["proof_system_id"]),
        statement_schema_digest_hex=bindings["statement schema digest"],
        verifier_digest_hex=bindings["verifier digest"],
    )


def query_four_peer_verange_capability_state(
    transcript: TranscriptBuilder,
    peers: Sequence[PeerEndpoint],
    public_artifacts: VeRangePublicAdmissionArtifactsV1,
) -> VeRangeCapabilityStateV1:
    """Query exact committed VeRange admission state from four direct peers.

    This is a planned-case primitive only.  It does not turn stateless action
    finality, pipeline status, or block height into protocol action state and
    does not remove any controller-case blocker.
    """

    peer_set = require_exact_peer_set(peers)
    states: list[VeRangeCapabilityStateV1] = []
    for peer in peer_set:
        response = _controller_exchange(
            transcript,
            peer,
            "GET",
            PRIVACY_CAPABILITIES_PATH,
            accept=PRIVACY_CAPABILITIES_MEDIA_TYPE,
        )
        states.append(_parse_verange_capability_state(response, peer, public_artifacts))
    baseline = states[0]
    if any(state != baseline for state in states[1:]):
        _fail("four direct peers disagree on canonical committed VeRange capability state")
    transcript.event(
        "four-peer-verange-capability-state",
        capability_binding_sha256=baseline.capability_binding_sha256,
        committed_height=baseline.committed_height,
        manifest_archive_sha256=baseline.manifest_archive_sha256,
        manifest_digest_hex=baseline.manifest_digest_hex,
        media_type=PRIVACY_CAPABILITIES_MEDIA_TYPE,
        peers=[peer.label for peer in peer_set],
        protocol=VERANGE_PROTOCOL,
        query_path=PRIVACY_CAPABILITIES_PATH,
    )
    return baseline


def require_verange_capability_state_preserved(
    before: VeRangeCapabilityStateV1,
    after: VeRangeCapabilityStateV1,
) -> None:
    """Require stable protocol admission bindings across a later/restart query."""

    if after.committed_height < before.committed_height:
        _fail("post-restart VeRange capability state regressed in committed height")
    if (
        after.capability_binding_sha256 != before.capability_binding_sha256
        or after.engine_id != before.engine_id
        or after.engine_manifest_digest_hex != before.engine_manifest_digest_hex
        or after.parameter_digest_hex != before.parameter_digest_hex
        or after.parameter_id_hex != before.parameter_id_hex
        or after.proof_system_id != before.proof_system_id
        or after.statement_schema_digest_hex != before.statement_schema_digest_hex
        or after.verifier_digest_hex != before.verifier_digest_hex
    ):
        _fail("post-restart VeRange committed capability binding changed")


def _peer_sample(
    transcript: TranscriptBuilder,
    peer: PeerEndpoint,
) -> FleetSample:
    status_response = _controller_exchange(transcript, peer, "GET", "/status")
    if status_response.status != 200:
        raise TransientPeerError(f"{peer.label} status is unavailable")
    status = _json_http(status_response, f"{peer.label} status")
    height = _uint(status.get("blocks"), f"{peer.label} blocks", positive=True)
    consensus_response = _controller_exchange(
        transcript, peer, "GET", "/v1/sumeragi/status"
    )
    if consensus_response.status != 200:
        raise TransientPeerError(f"{peer.label} consensus status is unavailable")
    consensus = _json_http(consensus_response, f"{peer.label} consensus status")
    committed = _uint(
        consensus.get("last_committed_height"),
        f"{peer.label} committed height",
        positive=True,
    )
    subject = consensus.get("last_committed_subject")
    if not isinstance(subject, dict):
        _fail(f"{peer.label} omitted its committed subject")
    block_hash = _normalized_block_hash(
        subject.get("block_hash"), f"{peer.label} committed block"
    )
    if committed != height:
        _fail(f"{peer.label} public and durable heights differ")
    return FleetSample(height, block_hash)


def _fleet_sample(
    transcript: TranscriptBuilder,
    peers: Sequence[PeerEndpoint],
) -> FleetSample:
    samples = [_peer_sample(transcript, peer) for peer in peers]
    baseline = samples[0]
    if any(sample != baseline for sample in samples[1:]):
        _fail("four direct peers disagree on exact committed height/hash")
    transcript.event(
        "four-peer-convergence",
        block_hash=baseline.block_hash,
        height=baseline.height,
        peers=[peer.label for peer in peers],
    )
    return baseline


def _pipeline_status_kind(
    response: HttpObservation,
    expected_transaction_hash: str,
    peer: PeerEndpoint,
) -> str:
    if response.status == 404:
        return "NotFound"
    if response.status != 200:
        raise TransientPeerError(f"{peer.label} pipeline status is unavailable")
    payload = _json_http(response, f"{peer.label} pipeline status")
    if (
        payload.get("hash") != expected_transaction_hash
        or payload.get("scope") != "global"
        or not isinstance(payload.get("resolved_from"), str)
        or not payload["resolved_from"]
    ):
        _fail(f"{peer.label} pipeline status is not bound to the transaction")
    status = payload.get("status")
    if not isinstance(status, dict) or not isinstance(status.get("kind"), str):
        _fail(f"{peer.label} pipeline status omitted its typed result")
    kind = status["kind"]
    if kind not in {
        "Queued",
        "Approved",
        "Committed",
        "Applied",
        "Rejected",
        "Expired",
    }:
        _fail(f"{peer.label} pipeline status kind is unknown")
    return kind


def _wait_for_four_peer_finality(
    transcript: TranscriptBuilder,
    peers: Sequence[PeerEndpoint],
    transaction_hash: str,
    deadline: float,
) -> None:
    pending = {peer.label: peer for peer in peers}
    path = (
        "/v1/pipeline/transactions/status?"
        + urllib.parse.urlencode({"hash": transaction_hash, "scope": "global"})
    )
    while pending and time.monotonic() < deadline:
        for label, peer in tuple(pending.items()):
            try:
                response = _controller_exchange(transcript, peer, "GET", path)
                kind = _pipeline_status_kind(response, transaction_hash, peer)
            except TransientPeerError:
                continue
            if kind in {"Applied", "Committed"}:
                del pending[label]
            elif kind in {"Rejected", "Expired"}:
                _fail(f"{peer.label} reports terminal privacy rejection {kind}")
        if pending:
            time.sleep(0.1)
    if pending:
        _fail(f"privacy action did not finalize on direct peers {sorted(pending)!r}")


def _submit(
    transcript: TranscriptBuilder,
    peer: PeerEndpoint,
    transaction: bytes,
    *,
    expected: str,
) -> None:
    response = _controller_exchange(
        transcript,
        peer,
        "POST",
        "/v1/pipeline/transactions",
        body=transaction,
    )
    if expected == "accepted":
        if response.status not in {200, 202}:
            _fail(f"{peer.label} rejected a canonical privacy action")
        return
    if expected == "rejected":
        if response.status not in {400, 409, 422}:
            _fail(f"{peer.label} accepted or ambiguously handled forbidden privacy input")
        return
    _fail("controller selected an unknown submission expectation")


def _malformed_adversary(transaction: bytes) -> bytes:
    if len(transaction) < 2:
        _fail("proof-bearing transaction is too short for an adversarial mutation")
    adversary = bytearray(transaction)
    adversary[len(adversary) // 2] ^= 0x01
    if adversary == transaction:
        _fail("adversarial mutation did not change the transaction")
    return bytes(adversary)


@dataclass
class ControllerOwnedSupervisor:
    """A supervisor process started and retained by this sealed controller."""

    peer: PeerEndpoint
    process: subprocess.Popen[bytes]
    child_pid_file: Path
    owner_uid: int
    owner_gid: int
    expected_child_argv: tuple[str, ...] = ()

    def _child_pid(self) -> int:
        try:
            info_before = self.child_pid_file.lstat()
        except OSError as exc:
            raise TransientPeerError("restarted peer PID file is unavailable") from exc
        if (
            stat.S_ISLNK(info_before.st_mode)
            or not stat.S_ISREG(info_before.st_mode)
            or info_before.st_nlink != 1
            or info_before.st_uid != self.owner_uid
            or info_before.st_gid != self.owner_gid
            or stat.S_IMODE(info_before.st_mode) & 0o077
            or not 1 <= info_before.st_size <= 64
        ):
            _fail("controller-owned child PID file has unsafe identity or mode")
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        descriptor = os.open(self.child_pid_file, flags)
        try:
            info_open = os.fstat(descriptor)
            body = os.read(descriptor, 65)
            if os.read(descriptor, 1):
                _fail("controller-owned child PID file exceeds its bound")
        finally:
            os.close(descriptor)
        if (
            (info_open.st_dev, info_open.st_ino, info_open.st_size)
            != (info_before.st_dev, info_before.st_ino, info_before.st_size)
        ):
            _fail("controller-owned child PID file changed while read")
        try:
            text = body.decode("ascii")
        except UnicodeDecodeError as exc:
            raise SealedPrivacyControllerError("child PID file is not ASCII") from exc
        if re.fullmatch(r"[1-9][0-9]*\n", text) is None or int(text) <= 1:
            _fail("controller-owned child PID file is noncanonical")
        return int(text)

    @staticmethod
    def _pid_exists(pid: int) -> bool:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        except PermissionError:
            return True
        return True

    def _authenticated_child(self) -> int:
        """Bind the PID file to the exact sole child of the retained supervisor."""

        if not self.expected_child_argv or not all(
            isinstance(item, str) and item for item in self.expected_child_argv
        ):
            _fail("controller-owned supervisor lacks an exact child argv binding")
        try:
            from . import deploy_taira_v21_reset as deploy
        except ImportError:
            import deploy_taira_v21_reset as deploy
        pid = self._child_pid()
        inspector = deploy.SystemOps()
        try:
            before = inspector.inspect_process(pid)
            children = inspector.child_pids(self.process.pid)
            repeated_pid = self._child_pid()
            after = inspector.inspect_process(pid)
        except (OSError, deploy.DeploymentError) as exc:
            raise TransientPeerError(
                "controller-owned validator child identity is unavailable"
            ) from exc
        if (
            before != after
            or repeated_pid != pid
            or children != (pid,)
            or before.ppid != self.process.pid
            or before.uid != self.owner_uid
            or before.argv != self.expected_child_argv
        ):
            _fail("controller-owned supervisor does not own the exact validator child")
        return pid

    def restart(self, deadline: float) -> tuple[int, int, int]:
        if self.process.poll() is not None:
            _fail("controller-owned supervisor exited before restart")
        old_pid = self._authenticated_child()
        started = time.monotonic_ns()
        try:
            self.process.send_signal(signal.SIGUSR1)
        except ProcessLookupError as exc:
            raise SealedPrivacyControllerError(
                "controller-owned supervisor disappeared before restart"
            ) from exc
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                _fail("controller-owned supervisor exited during restart")
            try:
                new_pid = self._authenticated_child()
            except TransientPeerError:
                time.sleep(0.05)
                continue
            if new_pid != old_pid and not self._pid_exists(old_pid):
                elapsed = time.monotonic_ns() - started
                if elapsed < 0:
                    _fail("monotonic restart timer moved backwards")
                return old_pid, new_pid, elapsed // 1_000_000
            time.sleep(0.05)
        _fail("controller-owned validator child did not restart within its bound")


def _wait_for_exact_sentinel(
    transcript: TranscriptBuilder,
    peers: Sequence[PeerEndpoint],
    sentinel: FleetSample,
    deadline: float,
) -> FleetSample:
    while time.monotonic() < deadline:
        try:
            sample = _fleet_sample(transcript, peers)
        except TransientPeerError:
            time.sleep(0.1)
            continue
        if sample != sentinel:
            _fail("restarted fleet skipped or changed the exact sentinel height/hash")
        return sample
    _fail("restarted validator did not reach the exact sentinel height/hash")


def _wait_for_successor(
    transcript: TranscriptBuilder,
    peers: Sequence[PeerEndpoint],
    sentinel: FleetSample,
    deadline: float,
) -> FleetSample:
    while time.monotonic() < deadline:
        try:
            sample = _fleet_sample(transcript, peers)
        except TransientPeerError:
            time.sleep(0.1)
            continue
        if sample.height == sentinel.height and sample.block_hash == sentinel.block_hash:
            time.sleep(0.1)
            continue
        if sample.height <= sentinel.height or sample.block_hash == sentinel.block_hash:
            _fail("four-peer successor is stale or does not advance the sentinel")
        return sample
    _fail("all four validators did not finalize a successor after restart")


@dataclass(frozen=True)
class DiagnosticCaseRecords:
    transcript: bytes
    result: bytes


def run_verange_diagnostic_case(
    *,
    case: str,
    candidate_binding_sha256: str,
    action_driver: Path,
    expected_action_driver_sha256: str,
    work_directory: Path,
    peers: Sequence[PeerEndpoint],
    restarted_supervisor: ControllerOwnedSupervisor,
    primary_request: VeRangeActionRequest,
    successor_request: VeRangeActionRequest,
    timeout_seconds: float,
) -> DiagnosticCaseRecords:
    """Exercise generic VeRange finality without claiming a protocol state case."""

    if not case or len(case.encode("utf-8")) > 512 or timeout_seconds <= 0:
        _fail("diagnostic case name or timeout is outside its bound")
    peer_set = require_exact_peer_set(peers)
    expected_action_driver_sha256 = _sha256(
        expected_action_driver_sha256,
        "expected action-driver digest",
        nonzero=True,
    )
    if restarted_supervisor.peer != peer_set[-1]:
        _fail("controller restart target must be the fourth direct peer")
    transcript = TranscriptBuilder(
        case,
        candidate_binding_sha256,
        expected_action_driver_sha256,
    )
    deadline = time.monotonic() + timeout_seconds

    primary = invoke_action_driver(
        action_driver,
        primary_request,
        expected_sha256=expected_action_driver_sha256,
        work_directory=work_directory,
        timeout_seconds=max(0.01, deadline - time.monotonic()),
    )
    successor = invoke_action_driver(
        action_driver,
        successor_request,
        expected_sha256=expected_action_driver_sha256,
        work_directory=work_directory,
        timeout_seconds=max(0.01, deadline - time.monotonic()),
    )
    if (
        primary_request.candidate_binding_sha256 != candidate_binding_sha256
        or successor_request.candidate_binding_sha256 != candidate_binding_sha256
    ):
        _fail("action requests differ from the controller candidate binding")
    if (
        primary.request_id == successor.request_id
        or primary.transaction_hash_hex == successor.transaction_hash_hex
        or primary.transaction_sha256 == successor.transaction_sha256
        or primary.operation != VERANGE_OPERATION
        or successor.operation != VERANGE_OPERATION
        or primary.protocol != VERANGE_PROTOCOL
        or successor.protocol != VERANGE_PROTOCOL
        or primary.action_driver_sha256 != expected_action_driver_sha256
        or successor.action_driver_sha256 != expected_action_driver_sha256
        or primary.availability != action_ipc.CONSTRUCTION_ONLY_STATUS
        or successor.availability != action_ipc.CONSTRUCTION_ONLY_STATUS
        or primary.limitations != action_ipc.protocol_limitations(VERANGE_PROTOCOL)
        or successor.limitations != action_ipc.protocol_limitations(VERANGE_PROTOCOL)
        or primary.network_outcome_authoritative is not False
        or successor.network_outcome_authoritative is not False
        or primary.qualification_scope != action_ipc.QUALIFICATION_SCOPE
        or successor.qualification_scope != action_ipc.QUALIFICATION_SCOPE
    ):
        _fail("proof-bearing actions are non-unique or came from the wrong driver")
    inspections = (
        _inspect_native_verange_action(primary, primary_request),
        _inspect_native_verange_action(successor, successor_request),
    )
    for (role, action), inspection in zip(
        (("primary", primary), ("successor", successor)),
        inspections,
    ):
        public_artifacts = action.public_admission_artifacts
        if public_artifacts is None:
            _fail("native VeRange diagnostic action omitted public admission artifacts")
        transcript.event(
            "native-action",
            action_authority_account_id=public_artifacts.action_authority_account_id,
            action_authority_public_key_hex=public_artifacts.action_authority_public_key_hex,
            action_driver_sha256=expected_action_driver_sha256,
            availability=action.availability,
            limitations=list(action.limitations),
            network_outcome_authoritative=action.network_outcome_authoritative,
            operation=VERANGE_OPERATION,
            protocol=VERANGE_PROTOCOL,
            qualification_scope=action.qualification_scope,
            request_base64=base64.b64encode(action.request_bytes).decode("ascii"),
            request_id=action.request_id,
            request_sha256=hashlib.sha256(action.request_bytes).hexdigest(),
            request_size=len(action.request_bytes),
            response_sha256=hashlib.sha256(action.response_bytes).hexdigest(),
            response_size=len(action.response_bytes),
            role=role,
            transaction_base64=base64.b64encode(action.transaction).decode("ascii"),
            transaction_hash_hex=action.transaction_hash_hex,
            transaction_intent_digest=inspection.transaction_intent_digest,
            policy_id=inspection.policy_id,
            compiled_profile_norito_base64=base64.b64encode(
                public_artifacts.compiled_profile_norito
            ).decode("ascii"),
            compiled_profile_sha256=public_artifacts.compiled_profile_sha256,
            engine_id=public_artifacts.engine_id,
            engine_manifest_digest_hex=public_artifacts.engine_manifest_digest_hex,
            max_aggregation_count=public_artifacts.max_aggregation_count,
            parameter_digest_hex=public_artifacts.parameter_digest_hex,
            parameter_id_hex=public_artifacts.parameter_id_hex,
            proof_system_id=public_artifacts.proof_system_id,
            statement_digest=inspection.statement_digest,
            statement_schema_digest_hex=public_artifacts.statement_schema_digest_hex,
            proof_envelope_hash=inspection.proof_envelope_hash,
            proof_bytes=inspection.proof_bytes,
            statement_bytes=inspection.statement_bytes,
            transaction_sha256=action.transaction_sha256,
            transaction_size=len(action.transaction),
            verifier_digest_hex=public_artifacts.verifier_digest_hex,
        )

    before = _fleet_sample(transcript, peer_set)
    _submit(transcript, peer_set[0], primary.transaction, expected="accepted")
    _wait_for_four_peer_finality(
        transcript, peer_set, primary.transaction_hash_hex, deadline
    )
    sentinel = _fleet_sample(transcript, peer_set)
    if sentinel.height <= before.height or sentinel.block_hash == before.block_hash:
        _fail("canonical privacy action did not create a new exact sentinel")

    _submit(transcript, peer_set[1], primary.transaction, expected="rejected")
    adversary = _malformed_adversary(primary.transaction)
    _submit(transcript, peer_set[2], adversary, expected="rejected")
    transcript.event(
        "adversarial-binding",
        canonical_transaction_sha256=primary.transaction_sha256,
        mutated_transaction_sha256=hashlib.sha256(adversary).hexdigest(),
        mutated_transaction_size=len(adversary),
    )

    old_pid, new_pid, duration_ms = restarted_supervisor.restart(deadline)
    transcript.event(
        "controller-owned-restart",
        duration_ms=duration_ms,
        new_child_pid=new_pid,
        old_child_pid=old_pid,
        peer=restarted_supervisor.peer.label,
    )
    recovered = _wait_for_exact_sentinel(transcript, peer_set, sentinel, deadline)
    _wait_for_four_peer_finality(
        transcript, peer_set, primary.transaction_hash_hex, deadline
    )

    _submit(transcript, peer_set[0], successor.transaction, expected="accepted")
    _wait_for_four_peer_finality(
        transcript, peer_set, successor.transaction_hash_hex, deadline
    )
    successor_sample = _wait_for_successor(transcript, peer_set, sentinel, deadline)

    transcript_bytes, transcript_id = transcript.finish()
    result_body: dict[str, object] = {
        "candidate_binding_sha256": candidate_binding_sha256,
        "case": case,
        "diagnostic_only": True,
        "action_driver_sha256": expected_action_driver_sha256,
        "availability": primary.availability,
        "limitations": list(primary.limitations),
        "network_outcome_authoritative": False,
        "operation_surface_complete": False,
        "qualification_scope": primary.qualification_scope,
        "primary_transaction_hash_hex": primary.transaction_hash_hex,
        "recovered_hash": recovered.block_hash,
        "recovered_height": recovered.height,
        "schema": RESULT_SCHEMA,
        "schema_version": CONTROLLER_SCHEMA_VERSION,
        "sentinel_hash": sentinel.block_hash,
        "sentinel_height": sentinel.height,
        "successor_hash": successor_sample.block_hash,
        "successor_height": successor_sample.height,
        "successor_transaction_hash_hex": successor.transaction_hash_hex,
        "transcript_id": transcript_id,
        "transcript_sha256": hashlib.sha256(transcript_bytes).hexdigest(),
        "transcript_size": len(transcript_bytes),
    }
    result_id = hashlib.sha256(
        RESULT_ID_DOMAIN + _canonical_json_bytes(result_body)
    ).hexdigest()
    result = _canonical_json_bytes({**result_body, "result_id": result_id})
    if len(result) > MAX_RESULT_BYTES:
        _fail("sealed diagnostic result exceeds its canonical byte bound")
    return DiagnosticCaseRecords(transcript_bytes, result)
