"""Client helpers for interacting with Iroha Torii endpoints."""

from __future__ import annotations

import base64
import binascii
import copy
import hashlib
import json
import logging
import math
import os
import re
import secrets
import time
from dataclasses import asdict, dataclass, field, is_dataclass
from decimal import Decimal
from enum import Enum
from pathlib import Path
from types import ModuleType
from typing import (
    TYPE_CHECKING,
    Any,
    Callable,
    Dict,
    Iterable,
    Iterator,
    List,
    Mapping,
    MutableMapping,
    NewType,
    Optional,
    Sequence,
    Tuple,
    Union,
)
from urllib.parse import quote, urlencode, urlparse, urlunparse

import requests
from iroha_torii_client.client import (
    ConfidentialGasSchedule,
    ConfigurationSnapshot,
    OfflineActiveRecursiveStepEpVerifier,
    OfflineActiveRecursiveStepEqVerifier,
    OfflineActiveTopUpShieldVerifier,
    OfflineActiveTransferVerifier,
    OfflineActiveUnshieldVerifier,
    StreamingSoranetConfig,
    StreamingTransportConfig,
    TransportConfig,
    TransportNoritoRpcConfig,
    NetworkTimeRttBucket,
    NetworkTimeSample,
    NetworkTimeSnapshot,
    NetworkTimeStatus,
    MultisigResponse,
    OfflineAppliedOperation,
    OfflineAppliedResult,
    OfflineAxtErrorDetails,
    OfflineAuthorizationJson,
    OfflineAssetScale,
    OfflineBranchClaimJson,
    OfflineBranchPathJson,
    OfflineLanePrivacyMerkleVariantJson,
    OfflineLanePrivacyMerkleWitnessJson,
    OfflineLanePrivacyProofJson,
    OfflineLanePrivacySnarkVariantJson,
    OfflineLanePrivacySnarkWitnessJson,
    OfflineLanePrivacyWitnessJson,
    OfflineMerkleProofJson,
    OfflineOperationKind,
    OfflineOperationReference,
    OfflineOperationStatus,
    OfflinePendingState,
    OfflinePendingOperation,
    OfflineProofAttachmentJson,
    OfflineProofBoxJson,
    OfflineProofBackend,
    OfflinePeerSplitTransitionJson,
    OfflinePeerSplitTransitionVariantJson,
    OfflineActiveTopUpShieldVerifier,
    OfflineActiveTransferVerifier,
    OfflineReadiness,
    SumeragiV2CommitQcStatus as _CanonicalSumeragiV2CommitQcStatus,
    SumeragiV2HeightContextStatus as _CanonicalSumeragiV2HeightContextStatus,
    SumeragiV2LivenessStatus,
    SumeragiV2Round,
    SumeragiV2Status as _CanonicalSumeragiV2Status,
    OfflineReadinessBlocker,
    OfflineVerifierId,
    OfflineRedeemChangeJson,
    OfflineRedeemOperationResult,
    OfflineRedeemResult,
    OfflineRecursiveSpendBundleJson,
    OfflineRecursiveSpendProofJson,
    OfflineRecursiveSpendStatementJson,
    OfflineRecursiveSpendTransitionJson,
    OfflineRedemptionChangeTransitionJson,
    OfflineRedemptionChangeTransitionVariantJson,
    OfflineRedemptionIntentJson,
    OfflineRejectedOperation,
    OfflineQueueErrorDetails,
    OfflineErrorDetails,
    OfflineErrorEnvelope,
    OfflineScaledAmount,
    OfflineScaledAmountJson,
    OfflineSpendableNote,
    OfflineSpendableNoteJson,
    OfflineSpendBranchJson,
    OfflineTopUpAnchor,
    OfflineTopUpAnchorReferenceJson,
    OfflineTopUpFinalityProof,
    OfflineTopUpFinalityProofAnchor,
    OfflineTopUpOperationResult,
    OfflineTopUpResult,
    OfflineVerifierKeyId,
    OfflineVerifierKeyIdJson,
    OfflineVerifierStatus,
    OfflineUnshieldPublicInputsJson,
    OfflineVerifyingKeyJson,
    OfflineVerifyingKeyRecordJson,
    OfflineVerifiedFoldBundleJson,
    OfflineVerifiedFoldRecordBundleJson,
    OfflineVerifiedFoldStepJson,
    OfflineVerifiedFoldVerifierRecordJson,
    SumeragiAutonomousLaneExecution as _CanonicalSumeragiAutonomousLaneExecution,
    SumeragiDataspaceCommitmentStatus as _CanonicalSumeragiDataspaceCommitmentStatus,
    SumeragiDiagnosticsStatus as _CanonicalSumeragiDiagnosticsStatus,
    SumeragiLaneCommitmentStatus as _CanonicalSumeragiLaneCommitmentStatus,
    SumeragiLaneGovernanceStatus as _CanonicalSumeragiLaneGovernanceStatus,
    SumeragiNativeAmxParticipantApplication as _CanonicalSumeragiNativeAmxParticipantApplication,
    SumeragiNposDiagnostics as _CanonicalSumeragiNposDiagnostics,
    SumeragiPipelineExecutionStatus as _CanonicalSumeragiPipelineExecutionStatus,
    SubscriptionActionResult,
    SubscriptionCreateResult,
    SubscriptionListItem,
    SubscriptionListPage,
    SubscriptionPlanCreateResult,
    SubscriptionPlanListItem,
    SubscriptionPlanListPage,
    ToriiCanonicalRequestAuth,
    ToriiClient as _BaseToriiClient,
    VpnProfile,
    VpnQuote,
    VpnQuoteCreateRequest,
    VpnReceipt,
    VpnReceiptListResponse,
    VpnReceiptSubmitRequest,
    VpnSession,
    VpnSessionCreateRequest,
    build_canonical_request_headers,
    canonical_query_string,
    canonical_request_message,
    canonical_request_signature_message,
)
from iroha_torii_client.native_amx import (
    compute_native_amx_descriptor_hash,
    compute_native_amx_participant_settlement_hash,
    compute_native_amx_proposal_hash,
    compute_native_amx_validator_set_hash,
    validate_bls_normal_validator_set,
)

from .address import AccountAddress, AccountAddressError, normalize_i105_discriminant
from .connect import ConnectSessionInfo
from .event_filter import DataEventFilter, ensure_event_filter
from ._privacy_backends import _require_production_verify_backend_label
from .privacy_catalog import (
    get_privacy_algorithm_descriptors,
    privacy_capabilities as _privacy_capabilities,
)
from .dataspaces import (
    DataspacePlan,
    DataspaceSpec,
    DataspaceStatus,
    plan_dataspace as _plan_dataspace,
    write_dataspace_plan as _write_dataspace_plan,
)
from .query import (
    account_query_envelope,
    asset_definitions_query_envelope,
    asset_holders_query_envelope,
    domain_query_envelope,
    rwa_query_envelope,
)
from .numeric_v1 import NumericV1Codec
from .repo import RepoAgreementListPage
from .sorafs import (
    SorafsAliasError,
    SorafsAliasEvaluation,
    SorafsAliasPolicy,
    SorafsAliasWarning,
)
from .sorafs import (
    enforce_alias_policy as enforce_sorafs_alias_policy,
)

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .connect import _ConnectControlBase as ConnectControlBase  # noqa: F401
    from .crypto import Instruction, SignedTransactionEnvelope  # noqa: F401
    from .tx import QuantityLike, TransactionDraft
else:  # pragma: no cover - runtime type aliases
    Instruction = Any  # type: ignore[assignment]
    SignedTransactionEnvelope = Any  # type: ignore[assignment]
    ConnectControlBase = Any  # type: ignore[assignment]
    QuantityLike = Any  # type: ignore[assignment]
    TransactionDraft = Any  # type: ignore[assignment]


def _json_safe_value(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray, memoryview)):
        return base64.b64encode(bytes(value)).decode("ascii")
    if isinstance(value, Mapping):
        return {key: _json_safe_value(val) for key, val in value.items()}
    if isinstance(value, list):
        return [_json_safe_value(item) for item in value]
    if isinstance(value, tuple):
        return [_json_safe_value(item) for item in value]
    return value


def _encode_filter_arg(filter_value: Optional[Any]) -> Optional[str]:
    if filter_value is None:
        return None
    if isinstance(filter_value, str):
        return filter_value
    return json.dumps(filter_value)


def _encode_sort_arg(sort_value: Optional[Any]) -> Optional[str]:
    if sort_value is None:
        return None
    if isinstance(sort_value, str):
        return sort_value
    if isinstance(sort_value, Sequence):
        return ",".join(str(entry).strip() for entry in sort_value if entry is not None)
    return str(sort_value)

DEFAULT_I105_DISCRIMINANT = 0x02F1
# Must match `iroha_data_model::DATA_MODEL_VERSION` on the node.
DATA_MODEL_VERSION = 3
ACCOUNT_FAUCET_POW_DOMAIN_SEPARATOR = b"iroha:accounts:faucet:pow:v2"
ACCOUNT_ONBOARDING_TOKEN_HEADER = "X-Iroha-Onboarding-Token"


def _require_account_onboarding_token(value: Any) -> str:
    if not isinstance(value, str):
        raise TypeError("onboarding_token must be a string")
    encoded = value.encode("utf-8")
    if not 32 <= len(encoded) <= 256 or any(byte < 0x21 or byte > 0x7E for byte in encoded):
        raise ValueError(
            "onboarding_token must contain 32..256 printable ASCII bytes "
            "without spaces or normalization"
        )
    return value


def _set_exact_header(headers: MutableMapping[str, str], name: str, value: str) -> None:
    lower_name = name.lower()
    for existing in list(headers):
        if existing.lower() == lower_name:
            del headers[existing]
    headers[name] = value


def _reject_default_onboarding_header(headers: Mapping[str, Any], context: str) -> None:
    if any(str(name).lower() == ACCOUNT_ONBOARDING_TOKEN_HEADER.lower() for name in headers):
        raise ValueError(
            f"{context} must not contain {ACCOUNT_ONBOARDING_TOKEN_HEADER}; "
            "pass onboarding_token explicitly to onboard_account"
        )


def _reject_alias_keys(source: Mapping[str, Any],
                       aliases: Mapping[str, str],
                       *,
                       context: str) -> None:
    for alias_key, canonical_key in aliases.items():
        if alias_key in source:
            raise TypeError(
                f"{context} does not accept {alias_key}; use {canonical_key}"
            )


_ISO_STATUS_VALUES = {
    "pending": "Pending",
    "accepted": "Accepted",
    "committed": "Committed",
    "rejected": "Rejected",
}
_ISO_NON_TERMINAL_STATUSES = frozenset({"Pending", "Accepted"})
_PACS002_STATUS_CODES = frozenset({"ACTC", "ACSP", "ACSC", "ACWC", "PDNG", "RJCT"})
_DEFAULT_ISO_POLL_INTERVAL_SECONDS = 2.0
_DEFAULT_ISO_WAIT_ATTEMPTS = 12
_MIN_ISO_POLL_INTERVAL_SECONDS = 0.01


def _require_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed:
        raise ValueError(f"{context} must be a non-empty string")
    return trimmed


def _require_mapping(value: Any, context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be a JSON object")
    return value


def _require_exact_non_empty_string(value: Any, context: str) -> str:
    trimmed = _require_non_empty_string(value, context)
    if trimmed != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    return value


def _normalize_optional_exact_string(value: Any, context: str) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed:
        return None
    if trimmed != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    return value


def _normalize_zk_verifying_key_registration_payload(payload: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(payload, Mapping):
        raise TypeError("ZK verifying-key registration payload must be a mapping")
    body = dict(_json_safe_value(dict(payload)))
    _normalize_zk_verifying_key_submission_payload(
        body,
        "register_zk_verifying_key",
        require_gas_schedule=True,
    )
    return body


def _normalize_zk_verifying_key_update_payload(payload: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(payload, Mapping):
        raise TypeError("ZK verifying-key update payload must be a mapping")
    body = dict(_json_safe_value(dict(payload)))
    _normalize_zk_verifying_key_submission_payload(
        body,
        "update_zk_verifying_key",
        require_gas_schedule=False,
    )
    return body


def _normalize_zk_verifying_key_submission_payload(
    body: MutableMapping[str, Any],
    context: str,
    *,
    require_gas_schedule: bool,
) -> None:
    body["backend"] = _require_production_verify_backend_label(
        body.get("backend"),
        f"{context}.backend",
    )
    body["name"] = _require_exact_non_empty_string(body.get("name"), f"{context}.name")
    if ":" in body["name"]:
        raise ValueError(f"{context}.name must not contain ':'")
    body["authority"] = _require_non_empty_string(
        body.get("authority"),
        f"{context}.authority",
    )
    body["private_key"] = _require_non_empty_string(
        body.get("private_key"),
        f"{context}.private_key",
    )
    version = _coerce_int(body.get("version"), f"{context}.version")
    if version is None:
        raise ValueError(f"{context}.version must be provided")
    if version > 0xFFFF_FFFF:
        raise ValueError(f"{context}.version must fit in a u32")
    body["version"] = version
    body["circuit_id"] = _require_exact_non_empty_string(
        body.get("circuit_id"),
        f"{context}.circuit_id",
    )
    body["public_inputs_schema_hash_hex"] = _normalize_32_byte_hex(
        body.get("public_inputs_schema_hash_hex"),
        f"{context}.public_inputs_schema_hash_hex",
    )
    if require_gas_schedule:
        body["gas_schedule_id"] = _require_exact_non_empty_string(
            body.get("gas_schedule_id"),
            f"{context}.gas_schedule_id",
        )
    elif "gas_schedule_id" in body:
        gas_schedule_id = _normalize_optional_exact_string(
            body.get("gas_schedule_id"),
            f"{context}.gas_schedule_id",
        )
        if gas_schedule_id is None:
            body.pop("gas_schedule_id", None)
        else:
            body["gas_schedule_id"] = gas_schedule_id

    for field in ("curve", "metadata_uri_cid", "vk_bytes_cid"):
        if field in body:
            normalized = _normalize_optional_string(body.get(field), f"{context}.{field}")
            if normalized is None:
                body.pop(field, None)
            else:
                body[field] = normalized

    if "max_proof_bytes" in body:
        max_proof_bytes = _normalize_optional_u32_field(
            body.get("max_proof_bytes"),
            f"{context}.max_proof_bytes",
            allow_zero=True,
        )
        if max_proof_bytes is None:
            body.pop("max_proof_bytes", None)
        else:
            body["max_proof_bytes"] = max_proof_bytes
    if "status" in body:
        status = _normalize_optional_zk_verifying_key_status(body.get("status"), f"{context}.status")
        if status is None:
            body.pop("status", None)
        else:
            body["status"] = status
    _validate_zk_verifying_key_height_range(body, context)
    _validate_zk_verifying_key_material_and_commitment(body, context)


def _validate_zk_verifying_key_height_range(
    body: MutableMapping[str, Any],
    context: str,
) -> None:
    activation_height = None
    withdraw_height = None
    if "activation_height" in body:
        activation_height = _normalize_optional_int_field(
            body.get("activation_height"),
            f"{context}.activation_height",
        )
        body["activation_height"] = activation_height
    if "withdraw_height" in body:
        withdraw_height = _normalize_optional_int_field(
            body.get("withdraw_height"),
            f"{context}.withdraw_height",
        )
        body["withdraw_height"] = withdraw_height
    if (
        activation_height is not None
        and withdraw_height is not None
        and withdraw_height < activation_height
    ):
        raise ValueError(
            f"{context}.withdraw_height must be greater than or equal to activation_height"
        )


def _validate_zk_verifying_key_material_and_commitment(
    body: MutableMapping[str, Any],
    context: str,
) -> None:
    commitment_hex: Optional[str] = None
    if "commitment_hex" in body:
        commitment_value = body.get("commitment_hex")
        if commitment_value is not None:
            commitment_hex = _normalize_32_byte_hex(
                commitment_value,
                f"{context}.commitment_hex",
            )
            body["commitment_hex"] = commitment_hex
        else:
            body.pop("commitment_hex", None)

    vk_len: Optional[int] = None
    if "vk_len" in body:
        vk_len = _normalize_optional_u32_field(
            body.get("vk_len"),
            f"{context}.vk_len",
            allow_zero=False,
        )
        if vk_len is None:
            body.pop("vk_len", None)
        else:
            body["vk_len"] = vk_len

    vk_bytes_value = body.get("vk_bytes")
    vk_bytes: Optional[bytes] = None
    if vk_bytes_value is None:
        body.pop("vk_bytes", None)
    else:
        if not isinstance(vk_bytes_value, str):
            raise TypeError(f"{context}.vk_bytes must be a base64 string")
        try:
            vk_bytes = base64.b64decode(vk_bytes_value, validate=True)
        except binascii.Error as exc:
            raise ValueError(f"{context}.vk_bytes must be valid base64") from exc
        if not vk_bytes:
            raise ValueError(f"{context}.vk_bytes must be non-empty")
        if len(vk_bytes) > 0xFFFF_FFFF:
            raise ValueError(f"{context}.vk_bytes length must fit in a u32")
        body["vk_bytes"] = base64.b64encode(vk_bytes).decode("ascii")
        if vk_len is not None and vk_len != len(vk_bytes):
            raise ValueError(f"{context}.vk_len must match vk_bytes length")
        body["vk_len"] = len(vk_bytes)

    if vk_bytes is None:
        if commitment_hex is None:
            raise ValueError(f"{context}.commitment_hex is required when vk_bytes is omitted")
        if vk_len is None:
            raise ValueError(f"{context}.vk_len is required when vk_bytes is omitted")

    if vk_bytes is not None and commitment_hex is not None:
        expected = _zk_verifying_key_commitment_hex(body["backend"], vk_bytes)
        if commitment_hex != expected:
            raise ValueError(
                f"{context}.commitment_hex must match domain-separated SHA-256 of backend and vk_bytes"
            )


def _zk_verifying_key_commitment_hex(backend: str, vk_bytes: bytes) -> str:
    backend_bytes = backend.encode("utf-8")
    preimage = (
        b"iroha:zk:v1:vk"
        + len(backend_bytes).to_bytes(8, "big")
        + backend_bytes
        + len(vk_bytes).to_bytes(8, "big")
        + vk_bytes
    )
    return hashlib.sha256(preimage).hexdigest()


def _normalize_optional_zk_verifying_key_status(value: Any, context: str) -> Optional[str]:
    normalized = _normalize_optional_string(value, context)
    if normalized is None:
        return None
    lowered = normalized.lower()
    if lowered == "proposed":
        return "Proposed"
    if lowered == "active":
        return "Active"
    if lowered == "withdrawn":
        return "Withdrawn"
    raise ValueError(f"{context} must be Proposed, Active, or Withdrawn")


def _normalize_optional_u32_field(
    value: Any,
    context: str,
    *,
    allow_zero: bool,
) -> Optional[int]:
    parsed = _coerce_int(value, context, allow_zero=allow_zero)
    if parsed is None:
        return None
    if parsed > 0xFFFF_FFFF:
        raise ValueError(f"{context} must fit in a u32")
    return parsed


def _normalize_32_byte_hex(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a 32-byte hex string")
    trimmed = value.strip().lower()
    if trimmed.startswith("0x"):
        trimmed = trimmed[2:].strip()
    return _normalize_hex_string(trimmed, context, expected_length=64)


def _normalize_zk_ace_hex32(value: Any, context: str) -> str:
    if isinstance(value, (bytes, bytearray, memoryview)):
        raw = bytes(value)
        if len(raw) != 32:
            raise ValueError(f"{context} must contain 32 bytes")
        return raw.hex()
    return _normalize_32_byte_hex(value, context)


def _zk_ace_plain_mapping(value: Any) -> Optional[Mapping[str, Any]]:
    if isinstance(value, Mapping):
        return value
    if not isinstance(value, str) or len(value) > 4096:
        return None
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError:
        return None
    return parsed if isinstance(parsed, Mapping) else None


def _zk_ace_asset_metadata_entry(
    asset_definition: Optional[Mapping[str, Any]],
    key: str,
) -> Optional[Mapping[str, Any]]:
    if asset_definition is None:
        return None
    metadata = _zk_ace_plain_mapping(asset_definition.get("metadata"))
    if metadata is None:
        return None
    return _zk_ace_plain_mapping(metadata.get(key))


def _zk_ace_verifier_key_state(verifier_key: Union[str, Mapping[str, Any]]) -> Dict[str, str]:
    if isinstance(verifier_key, Mapping):
        backend = _require_production_verify_backend_label(
            verifier_key.get("backend"),
            "verifier_key.backend",
        )
        name = _require_exact_non_empty_string(verifier_key.get("name"), "verifier_key.name")
    else:
        literal = _require_exact_non_empty_string(verifier_key, "verifier_key")
        if ":" not in literal:
            raise ValueError("verifier_key must include a backend prefix")
        backend, name = literal.rsplit(":", 1)
        backend = _require_production_verify_backend_label(backend, "verifier_key.backend")
        name = _require_exact_non_empty_string(name, "verifier_key.name")
    return {"backend": backend, "name": name}


def _zk_ace_identity_commitment_state(
    *,
    identity_commitment: str,
    policy_hash: str,
    allowed_accounts: Sequence[str],
    chain_id: str,
    verifier_key: Union[str, Mapping[str, Any]],
    action_class: Optional[str],
    domain_tag: Optional[str],
) -> Dict[str, Any]:
    return {
        "identity_commitment": identity_commitment,
        "policy_hash": policy_hash,
        "chain_id": chain_id,
        "domain_tag": domain_tag,
        "action_class": action_class,
        "verifier_key_id": _zk_ace_verifier_key_state(verifier_key),
        "allowed_accounts": [
            _require_non_empty_string(account_id, f"allowed_accounts[{index}]")
            for index, account_id in enumerate(allowed_accounts)
        ],
        "commitment_status": "active",
        "revoked": False,
        "revocation_status": "not_revoked",
        "rotation_state": "current",
    }


def _zk_ace_transfer_replay_state(
    *,
    identity_commitment: str,
    tx_digest: str,
    replay_nullifier: str,
    policy_hash: str,
    source_account: str,
    chain_id: str,
    verifier_key: Union[str, Mapping[str, Any]],
    action_class: Optional[str],
    domain_tag: Optional[str],
) -> Dict[str, Any]:
    return {
        "identity_commitment": identity_commitment,
        "tx_digest": tx_digest,
        "replay_nullifier": replay_nullifier,
        "policy_hash": policy_hash,
        "chain_id": chain_id,
        "domain_tag": domain_tag,
        "action_class": action_class,
        "verifier_key_id": _zk_ace_verifier_key_state(verifier_key),
        "source_account": source_account,
        "source_account_allowed": True,
        "replay_status": "fresh",
        "duplicate": False,
        "already_seen": False,
    }


def _zk_ace_committed_identity_state(
    asset_definition: Optional[Mapping[str, Any]],
    *,
    identity_commitment: str,
    policy_hash: str,
    allowed_accounts: Sequence[str],
    chain_id: str,
    verifier_key: Union[str, Mapping[str, Any]],
    action_class: Optional[str],
    domain_tag: Optional[str],
) -> Optional[Dict[str, Any]]:
    last_identity = _zk_ace_asset_metadata_entry(
        asset_definition,
        "zk.ace.identity.last",
    )
    if last_identity is None:
        return None
    try:
        if (
            _normalize_zk_ace_hex32(
                last_identity.get("identity_commitment"),
                "zk.ace.identity.last.identity_commitment",
            )
            != identity_commitment
            or _normalize_zk_ace_hex32(
                last_identity.get("policy_hash"),
                "zk.ace.identity.last.policy_hash",
            )
            != policy_hash
        ):
            return None
    except (TypeError, ValueError):
        return None
    return _zk_ace_identity_commitment_state(
        identity_commitment=identity_commitment,
        policy_hash=policy_hash,
        allowed_accounts=allowed_accounts,
        chain_id=chain_id,
        verifier_key=verifier_key,
        action_class=action_class,
        domain_tag=domain_tag,
    )


def _zk_ace_committed_transfer_state(
    asset_definition: Optional[Mapping[str, Any]],
    *,
    identity_commitment: str,
    tx_digest: str,
    replay_nullifier: str,
    policy_hash: str,
    source_account: str,
    chain_id: str,
    verifier_key: Union[str, Mapping[str, Any]],
    action_class: Optional[str],
    domain_tag: Optional[str],
) -> Optional[Dict[str, Any]]:
    last_transfer = _zk_ace_asset_metadata_entry(
        asset_definition,
        "zk.ace.transfer.last",
    )
    if last_transfer is None:
        return None
    expected = {
        "identity_commitment": identity_commitment,
        "tx_digest": tx_digest,
        "replay_nullifier": replay_nullifier,
        "policy_hash": policy_hash,
    }
    try:
        for field_name, expected_value in expected.items():
            if (
                _normalize_zk_ace_hex32(
                    last_transfer.get(field_name),
                    f"zk.ace.transfer.last.{field_name}",
                )
                != expected_value
            ):
                return None
    except (TypeError, ValueError):
        return None
    return _zk_ace_transfer_replay_state(
        identity_commitment=identity_commitment,
        tx_digest=tx_digest,
        replay_nullifier=replay_nullifier,
        policy_hash=policy_hash,
        source_account=source_account,
        chain_id=chain_id,
        verifier_key=verifier_key,
        action_class=action_class,
        domain_tag=domain_tag,
    )


def _zk_ace_enrich_result(
    result: Mapping[str, Any],
    *,
    key: str,
    state: Optional[Mapping[str, Any]],
) -> Mapping[str, Any]:
    if state is None or key in result:
        return result
    enriched: Dict[str, Any] = dict(result)
    enriched[key] = dict(state)
    return enriched


def _normalize_optional_int_field(value: Any, context: str) -> Optional[int]:
    parsed = _coerce_int(value, context, allow_zero=True)
    return parsed if parsed is not None else None


def _normalize_optional_string(value: Any, context: str) -> Optional[str]:
    if value is None:
        return None
    return _require_non_empty_string(value, context)


def _dedupe_strings(values: Iterable[str]) -> List[str]:
    deduped: List[str] = []
    for value in values:
        if value not in deduped:
            deduped.append(value)
    return deduped


def _response_text(response: requests.Response) -> str:
    return response.text.strip() if response.text else ""


def _response_has_network_prefix_error(response: requests.Response) -> bool:
    return (
        response.status_code == 400
        and "ERR_UNEXPECTED_NETWORK_PREFIX" in _response_text(response)
    )


def _extract_page_items(payload: Any) -> List[Mapping[str, Any]]:
    raw_items = payload.get("items") if isinstance(payload, Mapping) else payload
    if raw_items is None:
        return []
    if not isinstance(raw_items, list):
        raise RuntimeError("Torii list response `items` must be a list")
    return [item for item in raw_items if isinstance(item, Mapping)]


def _page_total(payload: Any) -> Optional[int]:
    if not isinstance(payload, Mapping):
        return None
    total = payload.get("total")
    if isinstance(total, int):
        return total
    try:
        return int(total) if total is not None else None
    except (TypeError, ValueError):
        return None


def _page_metadata(payload: Mapping[str, Any], item_count: int, context: str) -> Dict[str, Any]:
    has_total = payload.get("total") is not None
    if has_total:
        try:
            total: Optional[int] = int(payload["total"])
        except (TypeError, ValueError) as exc:
            raise TypeError(f"{context} `total` must be numeric") from exc
        if total < 0:
            raise TypeError(f"{context} `total` must be non-negative")
    elif "has_more" in payload or "count_mode" in payload:
        total = None
    else:
        total = item_count

    has_more_raw = payload.get("has_more", False)
    if not isinstance(has_more_raw, bool):
        raise TypeError(f"{context} `has_more` must be a boolean")

    count_mode_raw = payload.get("count_mode")
    if count_mode_raw is None:
        count_mode = "exact" if total is not None else "bounded"
    elif isinstance(count_mode_raw, str) and count_mode_raw in {"bounded", "exact"}:
        count_mode = count_mode_raw
    else:
        raise TypeError(f"{context} `count_mode` must be 'bounded' or 'exact'")

    indexed_height_raw = payload.get("indexed_height")
    if indexed_height_raw is None:
        indexed_height: Optional[int] = None
    else:
        try:
            indexed_height = int(indexed_height_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError(f"{context} `indexed_height` must be numeric") from exc
        if indexed_height < 0:
            raise TypeError(f"{context} `indexed_height` must be non-negative")

    indexed_block_hash = payload.get("indexed_block_hash")
    if indexed_block_hash is not None and not isinstance(indexed_block_hash, str):
        raise TypeError(f"{context} `indexed_block_hash` must be a string or null")
    query_source = payload.get("query_source")
    if query_source is not None and not isinstance(query_source, str):
        raise TypeError(f"{context} `query_source` must be a string")

    return {
        "total": total,
        "has_more": has_more_raw,
        "count_mode": count_mode,
        "indexed_height": indexed_height,
        "indexed_block_hash": indexed_block_hash,
        "query_source": query_source,
    }


def _normalize_count_mode_arg(count_mode: Optional[str]) -> Optional[str]:
    if count_mode is None:
        return None
    value = str(count_mode).strip().lower()
    if value not in {"bounded", "exact"}:
        raise ValueError("count_mode must be 'bounded' or 'exact'")
    return value


def _asset_entry_matches_definition(
    item: Mapping[str, Any],
    asset_definition_id: str,
    account_id: str,
) -> bool:
    asset_id = str(item.get("asset_id") or item.get("asset") or "").strip()
    asset_alias = str(item.get("asset_alias") or "").strip()
    candidates = {
        asset_definition_id,
        f"{asset_definition_id}#{account_id}",
        f"{asset_definition_id}##{account_id}",
    }
    return (
        asset_alias == asset_definition_id
        or asset_id in candidates
        or asset_id.startswith(f"{asset_definition_id}#")
    )


def _canonical_quantity_text(value: Any, context: str) -> str:
    if type(value) is not str:
        raise TypeError(f"{context} must be a canonical JSON string")
    if len(value) > 155:
        raise ValueError(f"{context} exceeds the canonical V1 text bound")
    return str(NumericV1Codec.decode_quantity_json(value))


def _quantity_decimal(value: Any) -> Decimal:
    return Decimal(_canonical_quantity_text(value, "asset quantity"))


def _leading_zero_bits(payload: bytes) -> int:
    count = 0
    for byte in payload:
        if byte == 0:
            count += 8
            continue
        count += 8 - byte.bit_length()
        break
    return count


def _normalize_canonical_account_id(
    value: Any,
    context: str,
    *,
    expected_discriminant: int = DEFAULT_I105_DISCRIMINANT,
) -> str:
    literal = _require_non_empty_string(value, context)
    if any(ch.isspace() for ch in literal):
        raise ValueError(
            f"{context} must be a canonical I105 account id or on-chain account alias"
        )
    if "@" in literal:
        label, separator, scope = literal.partition("@")
        scope_parts = scope.split(".") if separator else []
        if (
            not label
            or not separator
            or not scope
            or len(scope_parts) not in (1, 2)
            or any(not part for part in scope_parts)
        ):
            raise ValueError(
                f"{context} must use canonical I105 account id or account alias `name@dataspace` / `name@domain.dataspace`"
            )
        return literal
    try:
        address = AccountAddress.parse_encoded(
            literal, expected_discriminant=expected_discriminant
        )
    except AccountAddressError as exc:
        raise ValueError(
            f"{context} must be a canonical I105 account id or on-chain account alias"
        ) from exc
    canonical = address.to_i105(expected_discriminant)
    if canonical != literal:
        raise ValueError(
            f"{context} must use canonical I105 account id form when not using an alias"
        )
    return canonical


def _normalize_string_list(value: Any, context: str) -> List[str]:
    if not isinstance(value, (list, tuple)):
        raise TypeError(f"{context} must be an array of strings")
    normalized: List[str] = []
    for index, item in enumerate(value):
        if not isinstance(item, str):
            raise TypeError(f"{context}[{index}] must be a string")
        normalized.append(item)
    return normalized


def _normalize_iso_optional_string(
    value: Any,
    context: str,
    *,
    allow_empty: bool = False,
) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed and not allow_empty:
        return None
    return trimmed


def _normalize_iso_string_array(value: Any, context: str) -> Tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, Sequence):
        raise TypeError(f"{context} must be an array of strings")
    entries: List[str] = []
    for index, entry in enumerate(value):
        if not isinstance(entry, str):
            raise TypeError(f"{context}[{index}] must be a string")
        trimmed = entry.strip()
        if not trimmed:
            raise ValueError(f"{context}[{index}] must be a non-empty string")
        entries.append(trimmed)
    return tuple(entries)


def _normalize_iso_status(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip().lower()
    if not trimmed:
        raise ValueError(f"{context} must be non-empty")
    normalized = _ISO_STATUS_VALUES.get(trimmed)
    if normalized is None:
        allowed = ", ".join(sorted(_ISO_STATUS_VALUES.values()))
        raise ValueError(f"{context} must be one of {allowed}")
    return normalized


def _normalize_pacs002_code(value: Any, context: str) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string or null")
    trimmed = value.strip().upper()
    if not trimmed:
        return None
    if trimmed not in _PACS002_STATUS_CODES:
        allowed = ", ".join(sorted(_PACS002_STATUS_CODES))
        raise ValueError(f"{context} must be one of {allowed}")
    return trimmed


def _normalize_iso_payload(message: Any, context: str) -> bytes:
    if isinstance(message, (bytes, bytearray, memoryview)):
        payload = bytes(message)
        if not payload:
            raise ValueError(f"{context} must be non-empty")
        return payload
    if isinstance(message, str):
        trimmed = message.strip()
        if not trimmed:
            raise ValueError(f"{context} must be a non-empty string")
        return trimmed.encode("utf-8")
    raise TypeError(f"{context} must be bytes or a UTF-8 string")


def _is_iso_status_terminal(
    status: Optional["IsoSubmissionRecord"],
    resolve_on_accepted: bool,
) -> bool:
    if status is None:
        return False
    if status.status in _ISO_NON_TERMINAL_STATUSES:
        return status.status == "Accepted" and resolve_on_accepted
    return True


def _normalize_iso_wait_kwargs(
    options: Optional[Mapping[str, Any]],
    *,
    context: str,
) -> Dict[str, Any]:
    if options is None:
        return {}
    if not isinstance(options, Mapping):
        raise TypeError(f"{context} must be a mapping when provided")
    allowed = {"poll_interval", "max_attempts", "resolve_on_accepted", "timeout", "on_poll"}
    extras = [key for key in options.keys() if key not in allowed]
    if extras:
        extras_str = ", ".join(sorted(extras))
        raise ValueError(f"{context} contains unsupported fields: {extras_str}")
    normalized: Dict[str, Any] = {}
    if "poll_interval" in options:
        normalized["poll_interval"] = options["poll_interval"]
    if "max_attempts" in options:
        normalized["max_attempts"] = options["max_attempts"]
    if "resolve_on_accepted" in options:
        normalized["resolve_on_accepted"] = options["resolve_on_accepted"]
    if "timeout" in options:
        normalized["timeout"] = options["timeout"]
    if "on_poll" in options:
        normalized["on_poll"] = options["on_poll"]
    return normalized


def _bytes_like_to_hex(value: Any, context: str) -> str:
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value).hex()
    if isinstance(value, (list, tuple)):
        try:
            return bytes(value).hex()
        except (TypeError, ValueError) as exc:
            raise TypeError(f"{context} must contain byte values") from exc
    raise TypeError(f"{context} must be bytes-like")


def _normalize_hex_string(
    value: Any,
    context: str,
    *,
    expected_length: Optional[int] = None,
) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a hex string")
    trimmed = value.strip().lower()
    if not trimmed:
        raise ValueError(f"{context} must be a non-empty hex string")
    if expected_length is not None and len(trimmed) != expected_length:
        raise ValueError(f"{context} must contain {expected_length} hex characters")
    try:
        bytes.fromhex(trimmed)
    except ValueError as exc:
        raise ValueError(f"{context} must contain valid hexadecimal characters") from exc
    return trimmed


def _normalize_hash_hex(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a hex string")
    trimmed = value.strip().lower()
    if trimmed.startswith("0x"):
        trimmed = trimmed[2:].strip()
    if ":" in trimmed:
        scheme, rest = trimmed.split(":", 1)
        if scheme and scheme != "blake2b32":
            raise ValueError(f"{context} must use blake2b32 hex encoding")
        trimmed = rest.strip()
    return _normalize_hex_string(trimmed, context, expected_length=64)


def _normalize_uaid_literal(value: Any, *, context: str = "uaid") -> str:
    """Normalise raw UAID inputs to the canonical ``uaid:<hex>`` form (LSB=1)."""

    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    trimmed = value.strip()
    if not trimmed:
        raise ValueError(f"{context} must be a non-empty string")
    if trimmed.lower().startswith("uaid:"):
        _, hex_value = trimmed.split(":", 1)
    else:
        hex_value = trimmed
    normalized = _normalize_hex_string(
        hex_value,
        context,
        expected_length=64,
    )
    if int(normalized[-1], 16) % 2 == 0:
        raise ValueError(f"{context} must have least significant bit set to 1")
    return f"uaid:{normalized}"


def _normalize_positive_int(value: Any, context: str, *, allow_zero: bool) -> int:
    try:
        integer = int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{context} must be numeric") from exc
    if integer < 0 or (integer == 0 and not allow_zero):
        comparator = "non-negative" if allow_zero else "greater than zero"
        raise ValueError(f"{context} must be {comparator}")
    return integer


def _normalize_space_directory_manifest_payload(
    manifest: Any,
    *,
    context: str,
) -> Dict[str, Any]:
    if not isinstance(manifest, Mapping):
        raise TypeError(f"{context} must be an object")
    _reject_alias_keys(
        manifest,
        {
            "Version": "version",
            "uaid_literal": "uaid",
            "uaidLiteral": "uaid",
            "dataspace_id": "dataspace",
            "dataspaceId": "dataspace",
            "issuedMs": "issued_ms",
            "activationEpoch": "activation_epoch",
            "expiryEpoch": "expiry_epoch",
            "Entries": "entries",
        },
        context=context,
    )
    result: Dict[str, Any] = {}
    version_raw = manifest.get("version")
    result["version"] = _require_non_empty_string(version_raw, f"{context}.version")
    uaid_literal = manifest.get("uaid")
    result["uaid"] = _normalize_uaid_literal(uaid_literal, context=f"{context}.uaid")
    dataspace_raw = manifest.get("dataspace")
    if dataspace_raw is None:
        raise TypeError(f"{context}.dataspace is required")
    result["dataspace"] = _normalize_positive_int(
        dataspace_raw,
        f"{context}.dataspace",
        allow_zero=True,
    )
    issued_ms = _normalize_optional_int_field(
        manifest.get("issued_ms"),
        f"{context}.issued_ms",
    )
    if issued_ms is not None:
        result["issued_ms"] = issued_ms
    activation_epoch = _normalize_optional_int_field(
        manifest.get("activation_epoch"),
        f"{context}.activation_epoch",
    )
    if activation_epoch is not None:
        result["activation_epoch"] = activation_epoch
    expiry_epoch = _normalize_optional_int_field(
        manifest.get("expiry_epoch"),
        f"{context}.expiry_epoch",
    )
    if expiry_epoch is not None:
        result["expiry_epoch"] = expiry_epoch
    accounts = manifest.get("accounts")
    if accounts is not None:
        result["accounts"] = _normalize_string_list(accounts, f"{context}.accounts")
    entries_raw = manifest.get("entries")
    if not isinstance(entries_raw, Sequence) or not entries_raw:
        raise TypeError(f"{context}.entries must be a non-empty array")
    normalized_entries: List[Dict[str, Any]] = []
    for index, entry in enumerate(entries_raw):
        if not isinstance(entry, Mapping):
            raise TypeError(f"{context}.entries[{index}] must be an object")
        effect = entry.get("effect")
        if not isinstance(effect, Mapping):
            raise TypeError(f"{context}.entries[{index}].effect must be an object")
        normalized_entry: Dict[str, Any] = {"effect": _json_safe_value(effect)}
        scope = entry.get("scope")
        if scope is not None:
            if not isinstance(scope, Mapping):
                raise TypeError(f"{context}.entries[{index}].scope must be an object")
            normalized_entry["scope"] = _json_safe_value(scope)
        notes = entry.get("notes")
        if notes is not None:
            if not isinstance(notes, str):
                raise TypeError(f"{context}.entries[{index}].notes must be a string")
            normalized_entry["notes"] = notes
        normalized_entries.append(normalized_entry)
    result["entries"] = normalized_entries
    return result


def _normalize_authority_credentials(
    payload: Mapping[str, Any],
    *,
    context: str,
) -> Dict[str, str]:
    if not isinstance(payload, Mapping):
        raise TypeError(f"{context} must be an object")
    _reject_alias_keys(
        payload,
        {
            "account": "authority",
            "privateKey": "private_key",
            "privateKeyMultihash": "private_key_multihash",
            "privateKeyHex": "private_key_hex",
            "privateKeyBytes": "private_key_bytes",
            "privateKeySeed": "private_key_seed",
            "privateKeyAlgorithm": "private_key_algorithm",
        },
        context=context,
    )
    authority_raw = payload.get("authority")
    authority = _require_non_empty_string(authority_raw, f"{context}.authority")
    private_key_literal = payload.get("private_key")
    if private_key_literal is not None:
        private_key = _require_non_empty_string(private_key_literal, f"{context}.private_key")
    else:
        multihash = payload.get("private_key_multihash")
        if multihash is not None:
            private_key = _require_non_empty_string(multihash, f"{context}.private_key_multihash")
        else:
            hex_literal = payload.get("private_key_hex")
            bytes_literal = payload.get("private_key_bytes") or payload.get("private_key_seed")
            if hex_literal is None and bytes_literal is None:
                raise TypeError(f"{context}.private_key is required")
            if hex_literal is not None:
                hex_value = _normalize_hex_string(
                    hex_literal,
                    f"{context}.private_key_hex",
                    expected_length=64,
                )
            else:
                hex_value = _bytes_like_to_hex(
                    bytes_literal,
                    f"{context}.private_key_bytes",
                )
                if len(hex_value) != 64:
                    raise ValueError(f"{context}.private_key_bytes must contain 32 bytes")
            algorithm = payload.get("private_key_algorithm") or "ed25519"
            algorithm_literal = _require_non_empty_string(
                algorithm,
                f"{context}.private_key_algorithm",
            )
            private_key = f"{algorithm_literal}:{hex_value.lower()}"
    return {"authority": authority, "private_key": private_key}


def _normalize_publish_space_directory_manifest_request(
    request: Mapping[str, Any],
) -> Dict[str, Any]:
    credentials = _normalize_authority_credentials(
        request,
        context="publish_space_directory_manifest",
    )
    manifest_payload = request.get("manifest")
    if manifest_payload is None:
        raise TypeError("publish_space_directory_manifest.manifest is required")
    manifest = _normalize_space_directory_manifest_payload(
        manifest_payload,
        context="publish_space_directory_manifest.manifest",
    )
    payload: Dict[str, Any] = {**credentials, "manifest": manifest}
    reason = request.get("reason")
    if reason is not None:
        if not isinstance(reason, str):
            raise TypeError("publish_space_directory_manifest.reason must be a string")
        payload["reason"] = reason
    return payload


def _normalize_revoke_space_directory_manifest_request(
    request: Mapping[str, Any],
) -> Dict[str, Any]:
    credentials = _normalize_authority_credentials(
        request,
        context="revoke_space_directory_manifest",
    )
    _reject_alias_keys(
        request,
        {
            "uaid_literal": "uaid",
            "uaidLiteral": "uaid",
            "dataspace_id": "dataspace",
            "dataspaceId": "dataspace",
            "revokedEpoch": "revoked_epoch",
        },
        context="revoke_space_directory_manifest",
    )
    uaid_literal = request.get("uaid")
    uaid = _normalize_uaid_literal(
        uaid_literal,
        context="revoke_space_directory_manifest.uaid",
    )
    dataspace_raw = request.get("dataspace")
    if dataspace_raw is None:
        raise TypeError("revoke_space_directory_manifest.dataspace is required")
    dataspace = _normalize_positive_int(
        dataspace_raw,
        "revoke_space_directory_manifest.dataspace",
        allow_zero=True,
    )
    revoked_epoch_raw = request.get("revoked_epoch")
    if revoked_epoch_raw is None:
        raise TypeError("revoke_space_directory_manifest.revoked_epoch is required")
    revoked_epoch = _normalize_positive_int(
        revoked_epoch_raw,
        "revoke_space_directory_manifest.revoked_epoch",
        allow_zero=True,
    )
    payload: Dict[str, Any] = {
        **credentials,
        "uaid": uaid,
        "dataspace": dataspace,
        "revoked_epoch": revoked_epoch,
    }
    reason = request.get("reason")
    if reason is not None:
        if not isinstance(reason, str):
            raise TypeError("revoke_space_directory_manifest.reason must be a string")
        payload["reason"] = reason
    return payload


def _normalize_iso_week_label(value: Any, context: str) -> str:
    if isinstance(value, str):
        label = value.strip().upper()
        if not _ISO_WEEK_RE.match(label):
            raise ValueError(f"{context} must match YYYY-Www (e.g., 2026-W05)")
        return label
    if isinstance(value, (tuple, list)):
        if len(value) != 2:
            raise ValueError(f"{context} tuple must contain (year, week)")
        year = _normalize_positive_int(value[0], f"{context}.year", allow_zero=False)
        week = _normalize_positive_int(value[1], f"{context}.week", allow_zero=False)
        if week > 53:
            raise ValueError(f"{context}.week must be between 1 and 53")
        return f"{year:04d}-W{week:02d}"
    raise TypeError(f"{context} must be a string or (year, week) tuple")


def _coerce_bool_flag(value: Any, context: str) -> bool:
    if isinstance(value, bool):
        return value
    raise TypeError(f"{context} must be a boolean")


def _coerce_finite_float(value: Any, context: str) -> float:
    try:
        number = float(value)
    except (TypeError, ValueError):
        raise TypeError(f"{context} must be a finite number") from None
    if not math.isfinite(number):
        raise ValueError(f"{context} must be finite")
    return number


def _parse_optional_duration_ms_field(value: Any, context: str) -> Optional[int]:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be an object")
    return _coerce_int(value.get("ms"), f"{context}.ms", allow_zero=True)


def _normalize_base64_payload(
    explicit_b64: Optional[Any],
    default_payload: Optional[Any],
    context: str,
) -> str:
    source = explicit_b64 if explicit_b64 is not None else default_payload
    if source is None:
        raise ValueError(f"{context} must be provided")
    if isinstance(source, str):
        trimmed = source.strip()
        if not trimmed:
            raise ValueError(f"{context} must be a non-empty base64 string")
        try:
            base64.b64decode(trimmed, validate=True)
        except binascii.Error as exc:
            raise ValueError(f"{context} must be base64 encoded") from exc
        return trimmed
    if isinstance(source, (bytes, bytearray, memoryview)):
        return base64.b64encode(bytes(source)).decode("ascii")
    raise TypeError(f"{context} must be bytes or a base64 string")


_MISSING = object()


def _first_present(source: Mapping[str, Any], *keys: str) -> Any:
    present = [key for key in keys if key in source and source[key] is not None]
    if len(present) > 1:
        raise TypeError(f"ambiguous aliases: {', '.join(present)}")
    if not present:
        return _MISSING
    return source[present[0]]


def _normalize_sorafs_digest_hex(value: Any, context: str) -> str:
    if isinstance(value, (bytes, bytearray, memoryview, list, tuple)):
        literal = _bytes_like_to_hex(value, context)
    elif isinstance(value, str):
        literal = value.strip()
        if literal.startswith(("0x", "0X")):
            literal = literal[2:].strip()
    else:
        raise TypeError(f"{context} must be a 32-byte hex string")
    if not re.fullmatch(r"[0-9a-fA-F]{64}", literal):
        raise ValueError(f"{context} must be a 32-byte hex string")
    return literal.lower()


def _normalize_sorafs_reputation_snapshot_id_hex(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a 16-byte hex string")
    literal = value.strip()
    if literal.startswith(("0x", "0X")):
        literal = literal[2:].strip()
    if not re.fullmatch(r"[0-9a-fA-F]{32}", literal):
        raise ValueError(f"{context} must be a 16-byte hex string")
    return literal.lower()


def _normalize_sorafs_reputation_provider_id(value: Any, context: str) -> str:
    provider_id = _require_non_empty_string(value, context)
    if len(provider_id) > 256:
        raise ValueError(f"{context} must be at most 256 characters")
    if not re.fullmatch(r"[0-9A-Za-z_.:-]+", provider_id):
        raise ValueError(f"{context} contains unsupported characters")
    return provider_id


def _sorafs_reputation_headers(
    *,
    if_none_match: Optional[str] = None,
    etag: Optional[str] = None,
    headers: Optional[Mapping[str, str]] = None,
    context: str,
) -> Dict[str, str]:
    if if_none_match is not None and etag is not None:
        raise ValueError(f"{context} accepts only one of if_none_match or etag")
    final_headers: Dict[str, str] = {"Accept": "application/json"}
    if headers is not None:
        if not isinstance(headers, Mapping):
            raise TypeError(f"{context}.headers must be a mapping")
        final_headers.update({str(key): str(value) for key, value in headers.items()})
    validator = if_none_match if if_none_match is not None else etag
    if validator is not None:
        final_headers["If-None-Match"] = _require_non_empty_string(
            validator,
            f"{context}.if_none_match",
        )
    return final_headers


def _sorafs_reputation_event_params(
    *,
    since: Optional[Any] = None,
    limit: Optional[Any] = None,
    context: str,
) -> Optional[Dict[str, int]]:
    params: Dict[str, int] = {}
    if since is not None:
        params["since"] = _normalize_sorafs_unsigned_integer(
            since,
            f"{context}.since",
            allow_zero=True,
        )
    if limit is not None:
        params["limit"] = _normalize_sorafs_unsigned_integer(
            limit,
            f"{context}.limit",
            allow_zero=False,
        )
    return params or None


def _sorafs_orderbook_events_websocket_url(
    base_url: str,
    *,
    params: Optional[Mapping[str, Any]],
    endpoint_path: str = "/v1/sorafs/orderbook/events/ws",
    context: str,
) -> str:
    if not isinstance(endpoint_path, str) or not endpoint_path:
        raise TypeError(f"{context}.endpoint_path must be a non-empty string")
    if "?" in endpoint_path or "#" in endpoint_path:
        raise ValueError(f"{context}.endpoint_path must not include query or fragment")
    if not endpoint_path.startswith("/"):
        raise ValueError(f"{context}.endpoint_path must start with '/'")
    parsed = urlparse(base_url)
    scheme_map = {"http": "ws", "https": "wss", "ws": "ws", "wss": "wss"}
    if parsed.scheme not in scheme_map:
        raise ValueError(f"{context}.base_url uses unsupported scheme {parsed.scheme!r}")
    if not parsed.netloc:
        raise ValueError(f"{context}.base_url must include a host")
    query = urlencode(params or {})
    return urlunparse((scheme_map[parsed.scheme], parsed.netloc, endpoint_path, "", query, ""))


def _websocket_text_frame(raw: Any, context: str) -> str:
    if isinstance(raw, str):
        return raw
    if isinstance(raw, (bytes, bytearray, memoryview)):
        return bytes(raw).decode("utf-8")
    raise TypeError(f"{context} expected WebSocket text or bytes frame")


def _parse_websocket_json_event(raw: Any, context: str) -> "WebSocketEvent":
    text = _websocket_text_frame(raw, context)
    try:
        payload = json.loads(text)
    except json.JSONDecodeError as exc:
        raise ValueError(f"{context} received non-JSON WebSocket frame") from exc
    record = _require_mapping(payload, f"{context} frame")
    event = record.get("event")
    if event is not None:
        event = _require_non_empty_string(event, f"{context}.event")
    return WebSocketEvent(event=event, data=record.get("data", ""), raw=text)


def _normalize_sorafs_unsigned_integer(
    value: Any,
    context: str,
    *,
    allow_zero: bool,
) -> int:
    if value is None or value == "":
        raise TypeError(f"{context} is required")
    if isinstance(value, bool):
        raise TypeError(f"{context} must be an integer")
    if isinstance(value, int):
        number = value
    elif isinstance(value, str):
        literal = value.strip()
        if not re.fullmatch(r"[+-]?\d+", literal):
            raise TypeError(f"{context} must be an integer")
        number = int(literal)
    else:
        raise TypeError(f"{context} must be an integer")
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{context} must be {'non-negative' if allow_zero else 'positive'}")
    return number


def _normalize_required_base64_payload(value: Any, context: str) -> str:
    normalized = _normalize_base64_payload(None, value, context)
    decoded = base64.b64decode(normalized, validate=True)
    if not decoded:
        raise ValueError(f"{context} must be a non-empty base64 string")
    return base64.b64encode(decoded).decode("ascii")




def _reject_governance_public_input_key(
    record: Dict[str, Any],
    key: str,
    canonical_key: str,
    *,
    context: str,
) -> None:
    if key not in record:
        return
    raise ValueError(f"{context} must use {canonical_key} (unsupported key {key})")


def _normalize_governance_public_hex_hint(
    record: Dict[str, Any],
    key: str,
    *,
    context: str,
) -> None:
    if key not in record:
        return
    value = record[key]
    if value is None:
        return
    if isinstance(value, (bytes, bytearray, memoryview, list, tuple)):
        raw = _bytes_like_to_hex(value, f"{context}.{key}")
        if len(raw) != 64:
            raise ValueError(f"{context}.{key} must be a 32-byte hex string")
        record[key] = raw.lower()
        return
    if not isinstance(value, str):
        raise ValueError(f"{context}.{key} must be a 32-byte hex string")
    raw = value.strip()
    if ":" in raw:
        scheme, rest = raw.split(":", 1)
        if scheme and scheme.lower() != "blake2b32":
            raise ValueError(f"{context}.{key} must be a 32-byte hex string")
        raw = rest.strip()
    if raw.startswith(("0x", "0X")):
        raw = raw[2:]
    if not re.fullmatch(r"[0-9a-fA-F]{64}", raw):
        raise ValueError(f"{context}.{key} must be a 32-byte hex string")
    record[key] = raw.lower()


def _ensure_governance_lock_hints_complete(
    owner: Any,
    amount: Any,
    duration_blocks: Any,
    *,
    context: str,
) -> None:
    has_owner = owner is not None
    has_amount = amount is not None
    has_duration = duration_blocks is not None
    has_any = has_owner or has_amount or has_duration
    if has_any and not (has_owner and has_amount and has_duration):
        raise ValueError(
            f"{context} must include owner, amount, duration_blocks when providing lock hints"
        )


def _ensure_governance_owner_canonical(owner: Any, *, context: str) -> None:
    if owner is None:
        return
    if not isinstance(owner, str):
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    trimmed = owner.strip()
    if not trimmed or trimmed != owner:
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    if any(ch.isspace() for ch in trimmed):
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    if "@" in trimmed:
        raise ValueError(f"{context}.owner must be a canonical I105 account id")
    try:
        address = AccountAddress.parse_encoded(trimmed, expected_discriminant=DEFAULT_I105_DISCRIMINANT)
    except AccountAddressError as exc:
        raise ValueError(f"{context}.owner must be a canonical I105 account id") from exc
    canonical = address.to_i105(DEFAULT_I105_DISCRIMINANT)
    if canonical != owner:
        raise ValueError(f"{context}.owner must be a canonical I105 account id")


def _normalize_governance_zk_public_inputs(value: Any, *, context: str) -> Dict[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be an object")
    normalized = dict(value)
    _reject_governance_public_input_key(
        normalized,
        "durationBlocks",
        "duration_blocks",
        context=context,
    )
    _reject_governance_public_input_key(
        normalized,
        "root_hint_hex",
        "root_hint",
        context=context,
    )
    _reject_governance_public_input_key(
        normalized,
        "rootHintHex",
        "root_hint",
        context=context,
    )
    _reject_governance_public_input_key(
        normalized,
        "rootHint",
        "root_hint",
        context=context,
    )
    _reject_governance_public_input_key(
        normalized,
        "nullifier_hex",
        "nullifier",
        context=context,
    )
    _reject_governance_public_input_key(
        normalized,
        "nullifierHex",
        "nullifier",
        context=context,
    )
    _normalize_governance_public_hex_hint(
        normalized,
        "root_hint",
        context=context,
    )
    _normalize_governance_public_hex_hint(
        normalized,
        "nullifier",
        context=context,
    )
    _ensure_governance_lock_hints_complete(
        normalized.get("owner"),
        normalized.get("amount"),
        normalized.get("duration_blocks"),
        context=context,
    )
    if normalized.get("amount") is not None:
        normalized["amount"] = _canonical_quantity_text(
            normalized["amount"],
            f"{context}.amount",
        )
    _ensure_governance_owner_canonical(normalized.get("owner"), context=context)
    return normalized


def _normalize_governance_zk_ballot_payload(
    payload: Mapping[str, Any],
    *,
    context: str,
) -> Dict[str, Any]:
    record = dict(payload)
    if "public" in record:
        public_inputs = record.get("public")
        if public_inputs is None:
            record.pop("public", None)
        else:
            record["public"] = _normalize_governance_zk_public_inputs(
                public_inputs,
                context=f"{context}.public",
            )
    return record


def _normalize_governance_zk_ballot_v1_payload(
    payload: Mapping[str, Any],
    *,
    context: str,
) -> Dict[str, Any]:
    record = dict(payload)
    _reject_governance_public_input_key(
        record,
        "durationBlocks",
        "duration_blocks",
        context=context,
    )
    _reject_governance_public_input_key(
        record,
        "root_hint_hex",
        "root_hint",
        context=context,
    )
    _reject_governance_public_input_key(
        record,
        "rootHintHex",
        "root_hint",
        context=context,
    )
    _reject_governance_public_input_key(
        record,
        "rootHint",
        "root_hint",
        context=context,
    )
    _reject_governance_public_input_key(
        record,
        "nullifier_hex",
        "nullifier",
        context=context,
    )
    _reject_governance_public_input_key(
        record,
        "nullifierHex",
        "nullifier",
        context=context,
    )
    _normalize_governance_public_hex_hint(
        record,
        "root_hint",
        context=context,
    )
    _normalize_governance_public_hex_hint(
        record,
        "nullifier",
        context=context,
    )
    _ensure_governance_lock_hints_complete(
        record.get("owner"),
        record.get("amount"),
        record.get("duration_blocks"),
        context=context,
    )
    if record.get("amount") is not None:
        record["amount"] = _canonical_quantity_text(
            record["amount"],
            f"{context}.amount",
        )
    _ensure_governance_owner_canonical(record.get("owner"), context=context)
    return record


def _normalize_governance_zk_ballot_proof_payload(
    payload: Mapping[str, Any],
    *,
    context: str,
) -> Dict[str, Any]:
    record = dict(payload)
    ballot = record.get("ballot")
    if ballot is None:
        raise ValueError(f"{context}.ballot must be provided")
    if not isinstance(ballot, Mapping):
        raise TypeError(f"{context}.ballot must be an object")
    ballot_record = dict(ballot)
    ballot_context = f"{context}.ballot"
    _reject_governance_public_input_key(
        ballot_record,
        "rootHintHex",
        "root_hint",
        context=ballot_context,
    )
    _reject_governance_public_input_key(
        ballot_record,
        "root_hint_hex",
        "root_hint",
        context=ballot_context,
    )
    _reject_governance_public_input_key(
        ballot_record,
        "rootHint",
        "root_hint",
        context=ballot_context,
    )
    _reject_governance_public_input_key(
        ballot_record,
        "nullifierHex",
        "nullifier",
        context=ballot_context,
    )
    _reject_governance_public_input_key(
        ballot_record,
        "nullifier_hex",
        "nullifier",
        context=ballot_context,
    )
    _normalize_governance_public_hex_hint(
        ballot_record,
        "root_hint",
        context=ballot_context,
    )
    _normalize_governance_public_hex_hint(
        ballot_record,
        "nullifier",
        context=ballot_context,
    )
    _ensure_governance_lock_hints_complete(
        ballot_record.get("owner"),
        ballot_record.get("amount"),
        ballot_record.get("duration_blocks"),
        context=ballot_context,
    )
    if ballot_record.get("amount") is not None:
        ballot_record["amount"] = _canonical_quantity_text(
            ballot_record["amount"],
            f"{ballot_context}.amount",
        )
    _ensure_governance_owner_canonical(ballot_record.get("owner"), context=ballot_context)
    record["ballot"] = ballot_record
    return record


def _build_sorafs_por_status_params(
    manifest_hex: Optional[str],
    provider_hex: Optional[str],
    epoch: Optional[int],
    status: Optional[str],
    limit: Optional[int],
    page_token_hex: Optional[str],
) -> Optional[Dict[str, Any]]:
    params: Dict[str, Any] = {}
    if manifest_hex is not None:
        params["manifest"] = _normalize_hex_string(
            manifest_hex, "sorafs_por_status.manifest_hex", expected_length=64
        )
    if provider_hex is not None:
        params["provider"] = _normalize_hex_string(
            provider_hex, "sorafs_por_status.provider_hex", expected_length=64
        )
    if epoch is not None:
        params["epoch"] = _normalize_positive_int(epoch, "sorafs_por_status.epoch", allow_zero=False)
    if status is not None:
        trimmed = status.strip()
        if not trimmed:
            raise ValueError("sorafs_por_status.status must be non-empty")
        params["status"] = trimmed
    if limit is not None:
        params["limit"] = _normalize_positive_int(limit, "sorafs_por_status.limit", allow_zero=False)
    if page_token_hex is not None:
        params["page_token"] = _normalize_hex_string(
            page_token_hex, "sorafs_por_status.page_token_hex", expected_length=64
        )
    return params or None


def _build_sorafs_por_export_params(
    start_epoch: Optional[int],
    end_epoch: Optional[int],
) -> Optional[Dict[str, Any]]:
    params: Dict[str, Any] = {}
    if start_epoch is not None:
        params["start_epoch"] = _normalize_positive_int(
            start_epoch, "sorafs_por_export.start_epoch", allow_zero=False
        )
    if end_epoch is not None:
        params["end_epoch"] = _normalize_positive_int(
            end_epoch, "sorafs_por_export.end_epoch", allow_zero=False
        )
    return params or None

_CRYPTO_MODULE: Optional[ModuleType] = None
_ISO_WEEK_RE = re.compile(r"^\d{4}-W(0[1-9]|[1-4][0-9]|5[0-3])$")


@dataclass(frozen=True)
class ResolvedToriiClientConfig:
    """Fully merged Torii client configuration."""

    timeout: float
    max_retries: int
    backoff_initial: float
    backoff_multiplier: float
    max_backoff: float
    retry_statuses: frozenset[int]
    retry_methods: frozenset[str]
    default_headers: Dict[str, str]
    auth_token: Optional[str]
    api_token: Optional[str]
    sorafs_alias_policy: SorafsAliasPolicy


@dataclass(frozen=True)
class _ToriiClientConfigDefaults:
    """Non-SoraFS defaults that are safe to materialize without native policy code."""

    timeout: float
    max_retries: int
    backoff_initial: float
    backoff_multiplier: float
    max_backoff: float
    retry_statuses: frozenset[int]
    retry_methods: frozenset[str]
    default_headers: Dict[str, str]
    auth_token: Optional[str]
    api_token: Optional[str]


_DEFAULT_RESOLVED_CONFIG = _ToriiClientConfigDefaults(
    timeout=30.0,
    max_retries=3,
    backoff_initial=0.5,
    backoff_multiplier=2.0,
    max_backoff=5.0,
    retry_statuses=frozenset({429, 502, 503, 504}),
    retry_methods=frozenset({"GET", "HEAD", "OPTIONS"}),
    default_headers={"Accept": "application/json"},
    auth_token=None,
    api_token=None,
)


@dataclass(frozen=True)
class SorafsPorSubmissionResponse:
    """Response wrapper for PoR proof submissions."""

    status: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any], context: str) -> "SorafsPorSubmissionResponse":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be a JSON object")
        status = payload.get("status")
        if not isinstance(status, str) or not status.strip():
            raise TypeError(f"{context} missing string `status` field")
        return cls(status=status.strip())


@dataclass(frozen=True)
class SorafsPorVerdictResponse:
    """Response wrapper for PoR verdict submissions."""

    status: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any], context: str) -> "SorafsPorVerdictResponse":
        base = SorafsPorSubmissionResponse.from_payload(payload, context)
        return cls(status=base.status)


@dataclass(frozen=True)
class SorafsPinRegisterResponse:
    """Queue-admission identity returned by `/v1/sorafs/pin/register`."""

    status: str
    tx_hash_hex: str
    manifest_digest_hex: str

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
        context: str,
    ) -> "SorafsPinRegisterResponse":
        required = {"status", "tx_hash_hex", "manifest_digest_hex"}
        if not isinstance(payload, Mapping) or set(payload) != required:
            raise TypeError(
                f"{context} must contain only status, tx_hash_hex, and manifest_digest_hex"
            )
        if payload["status"] != "submitted":
            raise ValueError(f"{context}.status must be submitted")

        def canonical_digest(field: str) -> str:
            value = payload[field]
            if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
                raise ValueError(
                    f"{context}.{field} must be exactly 64 lowercase hexadecimal characters"
                )
            return value

        return cls(
            status="submitted",
            tx_hash_hex=canonical_digest("tx_hash_hex"),
            manifest_digest_hex=canonical_digest("manifest_digest_hex"),
        )


@dataclass(frozen=True)
class SorafsPorIngestionProviderStatus:
    """Provider-level PoR ingestion snapshot returned by `/v1/sorafs/por/ingestion/{manifest}`."""

    provider_id_hex: str
    pending_challenges: int
    oldest_epoch_id: Optional[int]
    oldest_response_deadline_unix: Optional[int]
    last_success_unix: Optional[int]
    last_failure_unix: Optional[int]
    failures_total: int
    consecutive_failures: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SorafsPorIngestionProviderStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("por ingestion provider entry must be an object")
        provider_literal = payload.get("provider_id_hex")
        if not isinstance(provider_literal, str) or not provider_literal:
            raise TypeError("por ingestion provider entry missing string `provider_id_hex` field")
        provider_id_hex = _normalize_hex_string(
            provider_literal,
            "por_ingestion.provider_id_hex",
            expected_length=64,
        )
        pending = _coerce_int(
            payload.get("pending_challenges"),
            "por ingestion provider pending_challenges",
            allow_zero=True,
        )
        if pending is None:
            raise TypeError("por ingestion provider entry missing numeric `pending_challenges` field")
        oldest_epoch = _coerce_int(
            payload.get("oldest_epoch_id"),
            "por ingestion provider oldest_epoch_id",
            allow_zero=True,
        )
        oldest_deadline = _coerce_int(
            payload.get("oldest_response_deadline_unix"),
            "por ingestion provider oldest_response_deadline_unix",
            allow_zero=True,
        )
        last_success = _coerce_int(
            payload.get("last_success_unix"),
            "por ingestion provider last_success_unix",
            allow_zero=True,
        )
        last_failure = _coerce_int(
            payload.get("last_failure_unix"),
            "por ingestion provider last_failure_unix",
            allow_zero=True,
        )
        failures_total = _coerce_int(
            payload.get("failures_total"),
            "por ingestion provider failures_total",
            allow_zero=True,
        )
        if failures_total is None:
            raise TypeError("por ingestion provider entry missing numeric `failures_total` field")
        consecutive_failures = _coerce_int(
            payload.get("consecutive_failures"),
            "por ingestion provider consecutive_failures",
            allow_zero=True,
        )
        if consecutive_failures is None:
            raise TypeError("por ingestion provider entry missing numeric `consecutive_failures` field")
        return cls(
            provider_id_hex=provider_id_hex,
            pending_challenges=pending,
            oldest_epoch_id=oldest_epoch,
            oldest_response_deadline_unix=oldest_deadline,
            last_success_unix=last_success,
            last_failure_unix=last_failure,
            failures_total=failures_total,
            consecutive_failures=consecutive_failures,
        )


@dataclass(frozen=True)
class SorafsPorIngestionStatus:
    """Manifest-level PoR ingestion snapshot."""

    manifest_digest_hex: str
    providers: List[SorafsPorIngestionProviderStatus]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SorafsPorIngestionStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("por ingestion response must be an object")
        manifest_literal = payload.get("manifest_digest_hex")
        if not isinstance(manifest_literal, str) or not manifest_literal:
            raise TypeError("por ingestion response missing string `manifest_digest_hex` field")
        manifest_digest_hex = _normalize_hex_string(
            manifest_literal,
            "por_ingestion.manifest_digest_hex",
            expected_length=64,
        )
        providers_payload = payload.get("providers")
        if not isinstance(providers_payload, list):
            raise TypeError("por ingestion response `providers` must be a list")
        providers: List[SorafsPorIngestionProviderStatus] = []
        for index, entry in enumerate(providers_payload):
            if not isinstance(entry, Mapping):
                raise TypeError(f"por ingestion providers[{index}] must be an object")
            providers.append(SorafsPorIngestionProviderStatus.from_payload(entry))
        return cls(manifest_digest_hex=manifest_digest_hex, providers=providers)


@dataclass(frozen=True)
class ExplorerMetricsSnapshot:
    """Network metrics exposed via `/v1/explorer/metrics`."""

    peers: int
    domains: int
    accounts: int
    assets: int
    transactions_accepted: int
    transactions_rejected: int
    block_height: int
    block_created_at: Optional[str]
    finalized_block_height: int
    average_commit_time_ms: Optional[int]
    average_block_time_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ExplorerMetricsSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("explorer metrics payload must be an object")

        def _resolve_int(key: str, label: str) -> int:
            raw = payload.get(key)
            parsed = _coerce_int(raw, label, allow_zero=True)
            return 0 if parsed is None else parsed

        def _resolve_optional_string(key: str, label: str) -> Optional[str]:
            raw = payload.get(key)
            if raw is None:
                return None
            if not isinstance(raw, str):
                raise TypeError(f"{label} must be a string")
            trimmed = raw.strip()
            return trimmed or None

        def _resolve_optional_duration(
            key: str,
            label: str,
        ) -> Optional[int]:
            raw = payload.get(key)
            return _parse_optional_duration_ms_field(raw, label)

        return cls(
            peers=_resolve_int("peers", "explorer_metrics.peers"),
            domains=_resolve_int("domains", "explorer_metrics.domains"),
            accounts=_resolve_int("accounts", "explorer_metrics.accounts"),
            assets=_resolve_int("assets", "explorer_metrics.assets"),
            transactions_accepted=_resolve_int(
                "transactions_accepted",
                "explorer_metrics.transactions_accepted",
            ),
            transactions_rejected=_resolve_int(
                "transactions_rejected",
                "explorer_metrics.transactions_rejected",
            ),
            block_height=_resolve_int(
                "block",
                "explorer_metrics.block",
            ),
            block_created_at=_resolve_optional_string(
                "block_created_at",
                "explorer_metrics.block_created_at",
            ),
            finalized_block_height=_resolve_int(
                "finalized_block",
                "explorer_metrics.finalized_block",
            ),
            average_commit_time_ms=_resolve_optional_duration(
                "avg_commit_time",
                "explorer_metrics.avg_commit_time",
            ),
            average_block_time_ms=_resolve_optional_duration(
                "avg_block_time",
                "explorer_metrics.avg_block_time",
            ),
        )


@dataclass(frozen=True)
class ExplorerAccountQrSnapshot:
    """Account QR metadata exposed via `/v1/explorer/accounts/{account_id}/qr`."""

    canonical_id: str
    literal: str
    network_prefix: int
    error_correction: str
    modules: int
    qr_version: int
    svg: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ExplorerAccountQrSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("explorer account qr payload must be an object")

        def _require_string(key: str, label: str) -> str:
            raw = payload.get(key)
            if not isinstance(raw, str) or not raw.strip():
                raise TypeError(f"{label} must be a non-empty string")
            return raw.strip()

        def _require_positive_int(key: str, label: str) -> int:
            value = _coerce_int(payload.get(key), label, allow_zero=False)
            if value is None:
                raise TypeError(f"{label} must be provided")
            return value

        canonical_id = _require_string(
            "canonical_id",
            "explorer_account_qr.canonical_id",
        )
        literal = _require_string("literal", "explorer_account_qr.literal")
        error_correction = _require_string(
            "error_correction",
            "explorer_account_qr.error_correction",
        )
        svg = _require_string("svg", "explorer_account_qr.svg")

        network_prefix = _require_positive_int(
            "network_prefix",
            "explorer_account_qr.network_prefix",
        )
        modules = _require_positive_int("modules", "explorer_account_qr.modules")
        qr_version = _require_positive_int(
            "qr_version",
            "explorer_account_qr.qr_version",
        )

        return cls(
            canonical_id=canonical_id,
            literal=literal,
            network_prefix=network_prefix,
            error_correction=error_correction,
            modules=modules,
            qr_version=qr_version,
            svg=svg,
        )


@dataclass(frozen=True)
class ExplorerPaginationMeta:
    """Pagination metadata returned by explorer list endpoints."""

    page: int
    per_page: int
    total_pages: int
    total_items: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ExplorerPaginationMeta":
        if not isinstance(payload, Mapping):
            raise TypeError("explorer pagination payload must be an object")
        page = _coerce_int(payload.get("page"), "explorer_pagination.page")
        per_page = _coerce_int(payload.get("per_page"), "explorer_pagination.per_page")
        total_pages = _coerce_int(
            payload.get("total_pages"),
            "explorer_pagination.total_pages",
            allow_zero=True,
        )
        total_items = _coerce_int(
            payload.get("total_items"),
            "explorer_pagination.total_items",
            allow_zero=True,
        )
        if page is None:
            raise TypeError("explorer pagination missing numeric `page` field")
        if per_page is None:
            raise TypeError("explorer pagination missing numeric `per_page` field")
        if total_pages is None:
            raise TypeError("explorer pagination missing numeric `total_pages` field")
        if total_items is None:
            raise TypeError("explorer pagination missing numeric `total_items` field")
        return cls(
            page=page,
            per_page=per_page,
            total_pages=total_pages,
            total_items=total_items,
        )


@dataclass(frozen=True)
class ExplorerRwaRecord:
    """Explorer RWA lot projection returned by `/v1/explorer/rwas`."""

    id: str
    owned_by: str
    quantity: str
    held_quantity: str
    primary_reference: str
    status: Optional[str]
    is_frozen: bool
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ExplorerRwaRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("explorer RWA record must be an object")

        def _require_string(key: str, label: str) -> str:
            raw = payload.get(key)
            if not isinstance(raw, str) or not raw.strip():
                raise TypeError(f"{label} must be a non-empty string")
            return raw.strip()

        identifier = _require_string("id", "explorer_rwa.id")
        owned_by = _require_string("owned_by", "explorer_rwa.owned_by")
        quantity = _canonical_quantity_text(
            payload.get("quantity"),
            "explorer_rwa.quantity",
        )
        held_quantity = _canonical_quantity_text(
            payload.get("held_quantity"),
            "explorer_rwa.held_quantity",
        )
        primary_reference = _require_string(
            "primary_reference",
            "explorer_rwa.primary_reference",
        )

        is_frozen = payload.get("is_frozen")
        if not isinstance(is_frozen, bool):
            raise TypeError("explorer_rwa.is_frozen must be a boolean")

        status_raw = payload.get("status")
        if status_raw is None:
            status = None
        elif isinstance(status_raw, str) and status_raw.strip():
            status = status_raw.strip()
        else:
            raise TypeError("explorer_rwa.status must be a string when present")

        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata = dict(metadata_payload)
        else:
            raise TypeError("explorer_rwa.metadata must be an object when present")

        return cls(
            id=identifier,
            owned_by=owned_by,
            quantity=quantity,
            held_quantity=held_quantity,
            primary_reference=primary_reference,
            status=status,
            is_frozen=is_frozen,
            metadata=metadata,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class ExplorerRwasPage:
    """Paginated explorer RWA list returned by `/v1/explorer/rwas`."""

    pagination: ExplorerPaginationMeta
    items: List[ExplorerRwaRecord]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ExplorerRwasPage":
        if not isinstance(payload, Mapping):
            raise TypeError("explorer RWA page payload must be an object")
        pagination_payload = payload.get("pagination")
        if not isinstance(pagination_payload, Mapping):
            raise TypeError("explorer RWA page missing object `pagination` field")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("explorer RWA page `items` must be a list")
        items = [ExplorerRwaRecord.from_payload(entry) for entry in items_payload]
        return cls(
            pagination=ExplorerPaginationMeta.from_payload(pagination_payload),
            items=items,
        )


@dataclass(frozen=True)
class IsoSubmissionRecord:
    """Normalized ISO 20022 bridge status payload."""

    message_id: str
    status: str
    pacs002_code: Optional[str]
    transaction_hash: Optional[str]
    hold_reason_code: Optional[str]
    change_reason_codes: Tuple[str, ...]
    rejection_reason_code: Optional[str]
    ledger_id: Optional[str]
    source_account_id: Optional[str]
    source_account_address: Optional[str]
    target_account_id: Optional[str]
    target_account_address: Optional[str]
    asset_definition_id: Optional[str]
    asset_id: Optional[str]
    detail: Optional[str]
    updated_at_ms: Optional[int]

    @classmethod
    def from_payload(
        cls,
        payload: Mapping[str, Any],
        *,
        context: str,
    ) -> "IsoSubmissionRecord":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be a JSON object")
        record = dict(payload)
        message_id = _require_non_empty_string(record.get("message_id"), f"{context}.message_id")
        status = _normalize_iso_status(record.get("status"), f"{context}.status")
        pacs002_code = _normalize_pacs002_code(record.get("pacs002_code"), f"{context}.pacs002_code")
        transaction_hash = _normalize_iso_optional_string(
            record.get("transaction_hash"),
            f"{context}.transaction_hash",
        )
        hold_reason_code = _normalize_iso_optional_string(
            record.get("hold_reason_code"),
            f"{context}.hold_reason_code",
        )
        change_reason_codes = _normalize_iso_string_array(
            record.get("change_reason_codes"),
            f"{context}.change_reason_codes",
        )
        rejection_reason_code = _normalize_iso_optional_string(
            record.get("rejection_reason_code"),
            f"{context}.rejection_reason_code",
        )
        ledger_id = _normalize_iso_optional_string(record.get("ledger_id"), f"{context}.ledger_id")
        source_account_id = _normalize_iso_optional_string(
            record.get("source_account_id"),
            f"{context}.source_account_id",
        )
        source_account_address = _normalize_iso_optional_string(
            record.get("source_account_address"),
            f"{context}.source_account_address",
        )
        target_account_id = _normalize_iso_optional_string(
            record.get("target_account_id"),
            f"{context}.target_account_id",
        )
        target_account_address = _normalize_iso_optional_string(
            record.get("target_account_address"),
            f"{context}.target_account_address",
        )
        asset_definition_id = _normalize_iso_optional_string(
            record.get("asset_definition_id"),
            f"{context}.asset_definition_id",
        )
        asset_id = _normalize_iso_optional_string(record.get("asset_id"), f"{context}.asset_id")
        detail = _normalize_iso_optional_string(
            record.get("detail"),
            f"{context}.detail",
            allow_empty=True,
        )
        updated_at_field = record.get("updated_at_ms")
        if updated_at_field is None:
            updated_at_ms = None
        else:
            updated_at_ms = _normalize_positive_int(
                updated_at_field,
                f"{context}.updated_at_ms",
                allow_zero=True,
            )
        return cls(
            message_id=message_id,
            status=status,
            pacs002_code=pacs002_code,
            transaction_hash=transaction_hash,
            hold_reason_code=hold_reason_code,
            change_reason_codes=change_reason_codes,
            rejection_reason_code=rejection_reason_code,
            ledger_id=ledger_id,
            source_account_id=source_account_id,
            source_account_address=source_account_address,
            target_account_id=target_account_id,
            target_account_address=target_account_address,
            asset_definition_id=asset_definition_id,
            asset_id=asset_id,
            detail=detail,
            updated_at_ms=updated_at_ms,
        )


class IsoMessageTimeoutError(RuntimeError):
    """Raised when ISO bridge messages fail to reach a terminal state."""

    def __init__(
        self,
        message_id: str,
        attempts: int,
        last_status: Optional[IsoSubmissionRecord],
    ) -> None:
        detail = f"Timed out waiting for ISO message {message_id} after {attempts} attempts"
        if last_status is not None:
            detail = f"{detail} (last status: {last_status.status})"
        super().__init__(detail)
        self.message_id = message_id
        self.attempts = attempts
        self.last_status = last_status


_KAIGI_HEALTH_STATUSES = frozenset({"healthy", "degraded", "unavailable"})


@dataclass(frozen=True)
class KaigiRelaySummary:
    """Summary entry returned by `/v1/kaigi/relays`."""

    relay_id: str
    domain: str
    bandwidth_class: int
    hpke_fingerprint_hex: str
    status: Optional[str]
    reported_at_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelaySummary":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay summary payload must be an object")
        relay_literal = payload.get("relay_id")
        domain_literal = payload.get("domain")
        bandwidth_literal = payload.get("bandwidth_class")
        fingerprint_literal = payload.get("hpke_fingerprint_hex")
        status_literal = payload.get("status")
        reported_literal = payload.get("reported_at_ms")

        relay_id = _require_non_empty_string(relay_literal, "kaigi_relay_summary.relay_id")
        domain = _require_non_empty_string(domain_literal, "kaigi_relay_summary.domain")
        bandwidth_value = _coerce_int(
            bandwidth_literal,
            "kaigi_relay_summary.bandwidth_class",
            allow_zero=True,
        )
        if bandwidth_value is None:
            bandwidth_value = 0
        fingerprint = _require_non_empty_string(
            fingerprint_literal,
            "kaigi_relay_summary.hpke_fingerprint_hex",
        )
        status: Optional[str] = None
        if status_literal is not None:
            status_value = _require_non_empty_string(
                status_literal,
                "kaigi_relay_summary.status",
            ).lower()
            if status_value not in _KAIGI_HEALTH_STATUSES:
                raise ValueError(
                    f"kaigi_relay_summary.status must be one of {sorted(_KAIGI_HEALTH_STATUSES)}"
                )
            status = status_value
        reported_at_ms = (
            _coerce_int(
                reported_literal,
                "kaigi_relay_summary.reported_at_ms",
                allow_zero=True,
            )
            if reported_literal is not None
            else None
        )

        return cls(
            relay_id=relay_id,
            domain=domain,
            bandwidth_class=bandwidth_value,
            hpke_fingerprint_hex=_normalize_hex_string(
                fingerprint,
                "kaigi_relay_summary.hpke_fingerprint_hex",
            ),
            status=status,
            reported_at_ms=reported_at_ms,
        )


@dataclass(frozen=True)
class KaigiRelaySummaryList:
    """Payload envelope returned by `/v1/kaigi/relays`."""

    items: List[KaigiRelaySummary]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelaySummaryList":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay summary response must be an object")
        raw_items = payload.get("items") or []
        if not isinstance(raw_items, Sequence) or isinstance(raw_items, (bytes, bytearray, str)):
            raise TypeError("kaigi relay summary response `items` must be an array")
        items = []
        for index, entry in enumerate(raw_items):
            if not isinstance(entry, Mapping):
                raise TypeError(f"kaigi relay summary response items[{index}] must be an object")
            items.append(KaigiRelaySummary.from_payload(entry))
        total_literal = payload.get("total")
        total_value = _coerce_int(
            total_literal,
            "kaigi_relay_summary.total",
            allow_zero=True,
        )
        if total_value is None:
            total_value = len(items)
        return cls(items=items, total=total_value)


@dataclass(frozen=True)
class KaigiRelayReportedCall:
    """Call metadata referenced by `/v1/kaigi/relays/{relay_id}`."""

    domain_id: str
    call_name: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayReportedCall":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay call payload must be an object")
        domain_literal = payload.get("domain_id")
        name_literal = payload.get("call_name")
        return cls(
            domain_id=_require_non_empty_string(
                domain_literal,
                "kaigi_relay_reported_call.domain_id",
            ),
            call_name=_require_non_empty_string(
                name_literal,
                "kaigi_relay_reported_call.call_name",
            ),
        )


@dataclass(frozen=True)
class KaigiRelayDomainMetrics:
    """Per-domain metrics included in Kaigi relay responses."""

    domain: str
    registrations_total: int
    manifest_updates_total: int
    failovers_total: int
    health_reports_total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayDomainMetrics":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay domain metrics payload must be an object")
        domain_literal = payload.get("domain")

        def _resolve_counter(name: str) -> int:
            value = _coerce_int(payload.get(name), f"kaigi_relay_domain_metrics.{name}", allow_zero=True)
            return value or 0

        return cls(
            domain=_require_non_empty_string(domain_literal, "kaigi_relay_domain_metrics.domain"),
            registrations_total=_resolve_counter("registrations_total"),
            manifest_updates_total=_resolve_counter("manifest_updates_total"),
            failovers_total=_resolve_counter("failovers_total"),
            health_reports_total=_resolve_counter("health_reports_total"),
        )


@dataclass(frozen=True)
class KaigiRelayDetail:
    """Detailed relay metadata returned by `/v1/kaigi/relays/{relay_id}`."""

    relay: KaigiRelaySummary
    hpke_public_key_b64: str
    reported_call: Optional[KaigiRelayReportedCall]
    reported_by: Optional[str]
    notes: Optional[str]
    metrics: Optional[KaigiRelayDomainMetrics]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayDetail":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay detail payload must be an object")
        relay_payload = payload.get("relay")
        if not isinstance(relay_payload, Mapping):
            raise TypeError("kaigi relay detail payload missing object `relay` field")
        hpke_literal = payload.get("hpke_public_key_b64")
        reported_call_payload = payload.get("reported_call")
        metrics_payload = payload.get("metrics")
        reported_by_literal = payload.get("reported_by")
        notes_literal = payload.get("notes")

        reported_by: Optional[str] = None
        if reported_by_literal is not None:
            reported_by = _require_non_empty_string(
                reported_by_literal,
                "kaigi_relay_detail.reported_by",
            )
        notes: Optional[str] = None
        if notes_literal is not None:
            if not isinstance(notes_literal, str):
                notes = str(notes_literal)
            else:
                trimmed = notes_literal.strip()
                notes = trimmed or None

        return cls(
            relay=KaigiRelaySummary.from_payload(relay_payload),
            hpke_public_key_b64=_require_non_empty_string(
                hpke_literal,
                "kaigi_relay_detail.hpke_public_key_b64",
            ),
            reported_call=KaigiRelayReportedCall.from_payload(reported_call_payload)
            if isinstance(reported_call_payload, Mapping)
            else None,
            reported_by=reported_by,
            notes=notes,
            metrics=KaigiRelayDomainMetrics.from_payload(metrics_payload)
            if isinstance(metrics_payload, Mapping)
            else None,
        )


@dataclass(frozen=True)
class KaigiRelayHealthSnapshot:
    """Aggregated relay health counters returned by `/v1/kaigi/relays/health`."""

    healthy_total: int
    degraded_total: int
    unavailable_total: int
    reports_total: int
    registrations_total: int
    failovers_total: int
    domains: List[KaigiRelayDomainMetrics]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "KaigiRelayHealthSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay health payload must be an object")
        domains_value = payload.get("domains") or []
        if not isinstance(domains_value, Sequence) or isinstance(domains_value, (bytes, bytearray, str)):
            raise TypeError("kaigi relay health payload `domains` must be an array")
        domains: List[KaigiRelayDomainMetrics] = []
        for index, entry in enumerate(domains_value):
            if not isinstance(entry, Mapping):
                raise TypeError(f"kaigi relay health payload domains[{index}] must be an object")
            domains.append(KaigiRelayDomainMetrics.from_payload(entry))

        def _resolve_counter(name: str) -> int:
            value = _coerce_int(payload.get(name), f"kaigi_relay_health.{name}", allow_zero=True)
            return value or 0

        return cls(
            healthy_total=_resolve_counter("healthy_total"),
            degraded_total=_resolve_counter("degraded_total"),
            unavailable_total=_resolve_counter("unavailable_total"),
            reports_total=_resolve_counter("reports_total"),
            registrations_total=_resolve_counter("registrations_total"),
            failovers_total=_resolve_counter("failovers_total"),
            domains=domains,
        )

def _configuration_snapshot_to_dict(snapshot: ConfigurationSnapshot) -> Dict[str, Any]:
    result: Dict[str, Any] = {
        "public_key": snapshot.public_key_hex,
        "logger": snapshot.logger.to_payload(),
        "network": {
            "block_gossip_size": snapshot.network.block_gossip_size,
            "block_gossip_period_ms": snapshot.network.block_gossip_period_ms,
            "transaction_gossip_size": snapshot.network.transaction_gossip_size,
            "transaction_gossip_period_ms": snapshot.network.transaction_gossip_period_ms,
        },
    }
    if snapshot.queue is not None:
        result["queue"] = {"capacity": snapshot.queue.capacity}
    if snapshot.confidential_gas is not None:
        result["confidential_gas"] = snapshot.confidential_gas.to_payload()
    if snapshot.transport is not None:
        transport_payload: Dict[str, Any] = {}
        if snapshot.transport.norito_rpc is not None:
            norito_rpc = snapshot.transport.norito_rpc
            transport_payload["norito_rpc"] = {
                "enabled": norito_rpc.enabled,
                "stage": norito_rpc.stage,
                "require_mtls": norito_rpc.require_mtls,
                "canary_allowlist_size": norito_rpc.canary_allowlist_size,
            }
        if snapshot.transport.streaming is not None:
            streaming_payload: Dict[str, Any] = {}
            soranet = snapshot.transport.streaming.soranet
            if soranet is not None:
                streaming_payload["soranet"] = {
                    "enabled": soranet.enabled,
                    "stream_tag": soranet.stream_tag,
                    "exit_multiaddr": soranet.exit_multiaddr,
                    "padding_budget_ms": soranet.padding_budget_ms,
                    "access_kind": soranet.access_kind,
                    "gar_category": soranet.gar_category,
                    "channel_salt": soranet.channel_salt,
                    "provision_spool_dir": soranet.provision_spool_dir,
                    "provision_window_segments": soranet.provision_window_segments,
                    "provision_queue_capacity": soranet.provision_queue_capacity,
                }
            if streaming_payload:
                transport_payload["streaming"] = streaming_payload
        if transport_payload:
            result["transport"] = transport_payload
    return result


def _configuration_update_payload(snapshot: ConfigurationSnapshot) -> Dict[str, Any]:
    payload = _configuration_snapshot_to_dict(snapshot)
    payload.pop("public_key", None)
    payload.pop("transport", None)
    return payload


def _network_time_snapshot_to_dict(snapshot: NetworkTimeSnapshot) -> Dict[str, int]:
    return {
        "now": snapshot.now_ms,
        "offset_ms": snapshot.offset_ms,
        "confidence_ms": snapshot.confidence_ms,
    }


def _network_time_status_to_dict(status: NetworkTimeStatus) -> Dict[str, Any]:
    samples = [
        {
            "peer": sample.peer,
            "last_offset_ms": sample.last_offset_ms,
            "last_rtt_ms": sample.last_rtt_ms,
            "count": sample.count,
        }
        for sample in status.samples
    ]
    rtt: Dict[str, Any] = {
        "buckets": [
            {"le": bucket.upper_bound_ms, "count": bucket.count}
            for bucket in status.rtt_buckets
        ],
        "sum_ms": status.rtt_sum_ms,
        "count": status.rtt_count,
    }
    payload: Dict[str, Any] = {
        "peers": status.peers,
        "samples": samples,
        "rtt": rtt,
    }
    if status.note is not None:
        payload["note"] = status.note
    return payload

@dataclass(frozen=True)
class GovernanceReferendumResult:
    """Wrapper for `/v1/gov/referenda/{id}` responses."""

    found: bool
    referendum: Optional[Dict[str, Any]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceReferendumResult":
        if not isinstance(payload, Mapping):
            raise TypeError("referendum payload must be a mapping")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("referendum payload missing bool `found` field")
        referendum = payload.get("referendum")
        if referendum is not None and not isinstance(referendum, Mapping):
            raise TypeError("referendum payload `referendum` must be an object when present")
        copied = dict(referendum) if isinstance(referendum, Mapping) else None
        return cls(found=found, referendum=copied)


@dataclass(frozen=True)
class GovernanceTally:
    """Referendum tally summary returned by `/v1/gov/tally/{id}`."""

    referendum_id: str
    approve: int
    reject: int
    abstain: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceTally":
        if not isinstance(payload, Mapping):
            raise TypeError("tally payload must be a mapping")
        referendum_id = payload.get("referendum_id")
        if not isinstance(referendum_id, str):
            raise TypeError("tally payload missing string `referendum_id`")
        try:
            approve = int(payload.get("approve", 0))
            reject = int(payload.get("reject", 0))
            abstain = int(payload.get("abstain", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("tally payload must contain numeric approve/reject/abstain values") from exc
        return cls(
            referendum_id=referendum_id,
            approve=approve,
            reject=reject,
            abstain=abstain,
        )


class GovernanceProposalStatus(str, Enum):
    """Governance proposal lifecycle status."""

    PROPOSED = "Proposed"
    APPROVED = "Approved"
    REJECTED = "Rejected"
    ENACTED = "Enacted"

    @classmethod
    def from_value(cls, value: str) -> "GovernanceProposalStatus":
        try:
            return cls(value)
        except ValueError as exc:
            raise TypeError("proposal status must be one of Proposed, Approved, Rejected, Enacted") from exc


@dataclass(frozen=True)
class GovernanceProposalDeployContract:
    """`DeployContract` payload embedded in governance proposals."""

    contract_address: str
    code_hash_hex: str
    abi_hash_hex: str
    abi_version: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalDeployContract":
        if not isinstance(payload, Mapping):
            raise TypeError("DeployContract payload must be an object")
        def _require_str(field_name: str) -> str:
            value = payload.get(field_name)
            if not isinstance(value, str):
                raise TypeError(f"DeployContract payload missing string `{field_name}` field")
            return value

        contract_address = _require_str("contract_address")
        code_hash_hex = _require_str("code_hash_hex")
        abi_hash_hex = _require_str("abi_hash_hex")
        abi_version = _require_str("abi_version")
        return cls(
            contract_address=contract_address,
            code_hash_hex=code_hash_hex,
            abi_hash_hex=abi_hash_hex,
            abi_version=abi_version,
        )


@dataclass(frozen=True)
class GovernanceContractRecord:
    """Governance binding returned by `GET /v1/gov/contracts/{contract_address}`."""

    found: bool
    contract_address: str
    dataspace: Optional[str]
    code_hash_hex: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceContractRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("governance contract payload must be an object")
        found = bool(payload.get("found", False))
        contract_address = payload.get("contract_address")
        if not isinstance(contract_address, str):
            raise TypeError("governance contract payload missing string `contract_address` field")
        dataspace = payload.get("dataspace")
        if dataspace is not None and not isinstance(dataspace, str):
            raise TypeError("governance contract payload `dataspace` must be a string or null")
        code_hash_hex = payload.get("code_hash_hex")
        if code_hash_hex is not None and not isinstance(code_hash_hex, str):
            raise TypeError("governance contract payload `code_hash_hex` must be a string or null")
        return cls(
            found=found,
            contract_address=contract_address,
            dataspace=dataspace,
            code_hash_hex=code_hash_hex,
        )


@dataclass(frozen=True)
class GovernanceProposalKind:
    """Normalized governance proposal kind."""

    variant: str
    deploy_contract: Optional[GovernanceProposalDeployContract]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalKind":
        if not isinstance(payload, Mapping):
            raise TypeError("proposal kind must be an object")
        if len(payload) != 1:
            raise TypeError("proposal kind must contain exactly one variant entry")
        (variant, details), = payload.items()
        if not isinstance(variant, str):
            raise TypeError("proposal kind variant key must be a string")
        raw: Dict[str, Any]
        deploy_contract: Optional[GovernanceProposalDeployContract] = None
        if isinstance(details, Mapping):
            raw = dict(details)
        else:
            raw = {"value": details}
        if variant == "DeployContract":
            if not isinstance(details, Mapping):
                raise TypeError("DeployContract proposal kind expects an object payload")
            deploy_contract = GovernanceProposalDeployContract.from_payload(details)
        return cls(variant=variant, deploy_contract=deploy_contract, raw=raw)


@dataclass(frozen=True)
class GovernanceProposalRecord:
    """Structured governance proposal record."""

    proposer: str
    kind: GovernanceProposalKind
    created_height: int
    status: GovernanceProposalStatus

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("proposal record must be an object")
        proposer = payload.get("proposer")
        if not isinstance(proposer, str):
            raise TypeError("proposal record missing string `proposer` field")
        kind_payload = payload.get("kind")
        if not isinstance(kind_payload, Mapping):
            raise TypeError("proposal record missing object `kind` field")
        created_height_raw = payload.get("created_height")
        status_raw = payload.get("status")
        if not isinstance(status_raw, str):
            raise TypeError("proposal record missing string `status` field")
        if created_height_raw is None:
            raise TypeError("proposal record missing numeric `created_height` field")
        try:
            created_height = int(created_height_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("proposal record `created_height` must be numeric") from exc
        status = GovernanceProposalStatus.from_value(status_raw)
        kind = GovernanceProposalKind.from_payload(kind_payload)
        return cls(
            proposer=proposer,
            kind=kind,
            created_height=created_height,
            status=status,
        )


@dataclass(frozen=True)
class GovernanceProposalResult:
    """Wrapper for `/v1/gov/proposals/{id}` responses."""

    found: bool
    proposal: Optional[GovernanceProposalRecord]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceProposalResult":
        if not isinstance(payload, Mapping):
            raise TypeError("proposal response must be an object")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("proposal response missing bool `found` field")
        proposal_payload = payload.get("proposal")
        if proposal_payload is None:
            return cls(found=found, proposal=None)
        if not isinstance(proposal_payload, Mapping):
            raise TypeError("proposal response `proposal` must be an object when present")
        proposal = GovernanceProposalRecord.from_payload(proposal_payload)
        return cls(found=found, proposal=proposal)


@dataclass(frozen=True)
class GovernanceLockRecord:
    """Governance lock record stored for a referendum."""

    owner: str
    amount: str
    slashed: str
    expiry_height: int
    direction: int
    duration_blocks: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceLockRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("governance lock record must be an object")
        owner = payload.get("owner")
        if not isinstance(owner, str):
            raise TypeError("governance lock record missing string `owner` field")
        amount_raw = payload.get("amount")
        if amount_raw is None:
            raise TypeError("governance lock record missing Quantity `amount` field")
        amount = _canonical_quantity_text(
            amount_raw,
            "governance lock record `amount`",
        )
        slashed = _canonical_quantity_text(
            payload.get("slashed"),
            "governance lock record `slashed`",
        )
        expiry_raw = payload.get("expiry_height")
        if expiry_raw is None:
            raise TypeError("governance lock record missing numeric `expiry_height` field")
        try:
            expiry_height = int(expiry_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("governance lock record `expiry_height` must be numeric") from exc
        direction_raw = payload.get("direction")
        if direction_raw is None:
            raise TypeError("governance lock record missing numeric `direction` field")
        try:
            direction = int(direction_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("governance lock record `direction` must be numeric") from exc
        if direction < 0 or direction > 255:
            raise ValueError("governance lock record `direction` must be within 0-255")
        duration_raw = payload.get("duration_blocks", 0)
        try:
            duration_blocks = int(duration_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("governance lock record `duration_blocks` must be numeric") from exc
        return cls(
            owner=owner,
            amount=amount,
            slashed=slashed,
            expiry_height=expiry_height,
            direction=direction,
            duration_blocks=duration_blocks,
        )


@dataclass(frozen=True)
class GovernanceLocksResult:
    """Wrapper for `/v1/gov/locks/{id}` responses."""

    found: bool
    referendum_id: str
    locks: Dict[str, GovernanceLockRecord]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceLocksResult":
        if not isinstance(payload, Mapping):
            raise TypeError("locks response must be an object")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("locks response missing bool `found` field")
        referendum_id = payload.get("referendum_id")
        if not isinstance(referendum_id, str):
            raise TypeError("locks response missing string `referendum_id` field")
        locks_payload = payload.get("locks")
        parsed: Dict[str, GovernanceLockRecord] = {}
        if locks_payload is not None:
            if not isinstance(locks_payload, Mapping):
                raise TypeError("locks response `locks` must be an object when present")
            for account, record_payload in locks_payload.items():
                if not isinstance(account, str):
                    raise TypeError("locks response keys must be account-id strings")
                if not isinstance(record_payload, Mapping):
                    raise TypeError("locks response values must be objects")
                parsed[account] = GovernanceLockRecord.from_payload(record_payload)
        return cls(found=found, referendum_id=referendum_id, locks=parsed)


@dataclass(frozen=True)
class GovernanceUnlockStats:
    """Aggregate unlock sweep statistics returned by `/v1/gov/unlocks/stats`."""

    height_current: int
    expired_locks_now: int
    referenda_with_expired: int
    last_sweep_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "GovernanceUnlockStats":
        if not isinstance(payload, Mapping):
            raise TypeError("unlock stats payload must be an object")
        def _require_int(name: str) -> int:
            value = payload.get(name)
            if value is None:
                raise TypeError(f"unlock stats missing `{name}` field")
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"unlock stats `{name}` must be numeric") from exc

        height_current = _require_int("height_current")
        expired_locks_now = _require_int("expired_locks_now")
        referenda_with_expired = _require_int("referenda_with_expired")
        last_sweep_height = _require_int("last_sweep_height")
        return cls(
            height_current=height_current,
            expired_locks_now=expired_locks_now,
            referenda_with_expired=referenda_with_expired,
            last_sweep_height=last_sweep_height,
        )


# BEGIN GENERATED: kotodama-v1-validator-policy
_KOTODAMA_RESERVED_IDENTIFIERS = frozenset(
    {
        "authorize",
        "break",
        "const",
        "continue",
        "else",
        "enum",
        "error",
        "false",
        "fn",
        "for",
        "hajimari",
        "始まり",
        "if",
        "in",
        "kaizen",
        "改善",
        "kotoage",
        "言挙げ",
        "let",
        "match",
        "module",
        "return",
        "seiyaku",
        "誓約",
        "state",
        "struct",
        "trigger",
        "true",
        "var",
        "view",
    }
)

_KOTODAMA_RESERVED_DECLARATION_IDENTIFIERS = frozenset(
    {
        "int",
        "decimal",
        "quantity",
        "bool",
        "string",
        "bytes",
        "Json",
        "AccountId",
        "AssetDefinitionId",
        "AssetId",
        "DomainId",
        "Name",
        "NftId",
        "DataSpaceId",
        "Option",
        "Result",
        "List",
        "StateMap",
        "Secret",
        "AccountView",
        "AssetView",
        "AssetDefinitionView",
        "DomainView",
        "NftView",
        "QueryPage",
        "AxtDescriptor",
        "AssetHandle",
        "ProofBlob",
        "SoracloudRequest",
        "SoracloudResponse",
        "state_map_get",
        "__kotodama_list_len",
        "__kotodama_list_get",
        "__kotodama_list_try_set",
        "__kotodama_list_try_push",
        "__kotodama_list_pop",
        "__kotodama_list_contains",
        "__kotodama_list_take",
        "__kotodama_list_enumerate",
        "__kotodama_decimal_div_round",
        "__kotodama_quantity_div_round",
        "__kotodama_quantity_ratio_round",
        "__kotodama_decimal_to_int_trunc",
        "__kotodama_decimal_to_int_round",
    }
)

_KOTODAMA_RETIRED_NUMERIC_TYPE_NAMES = frozenset(
    {
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "isize",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "num",
        "Int",
        "Integer",
        "float",
        "f32",
        "f64",
        "Decimal",
        "Fixed",
        "FixedPoint",
        "Amount",
        "amount",
        "money",
        "Quantity",
        "number",
    }
)
# END GENERATED: kotodama-v1-validator-policy
_KOTODAMA_IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
_KOTODAMA_RETIRED_NUMERIC_TYPE_RE = re.compile(
    r"(?<![A-Za-z0-9_])(?:"
    + "|".join(re.escape(name) for name in _KOTODAMA_RETIRED_NUMERIC_TYPE_NAMES)
    + r")(?![A-Za-z0-9_])"
)


def _contract_object(value: Any, path: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise TypeError(f"{path} must be an object")
    return value


def _contract_array(value: Any, path: str) -> Sequence[Any]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise TypeError(f"{path} must be an array")
    return value


def _contract_required_string(value: Any, path: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or not value.strip()
        or value.strip() != value
    ):
        raise TypeError(f"{path} must be an exact non-empty string")
    return value


def _contract_optional_string(value: Any, path: str) -> Optional[str]:
    if value is None:
        return None
    return _contract_required_string(value, path)


def _contract_type_name(value: Any, path: str) -> str:
    type_name = _contract_required_string(value, path)
    if _KOTODAMA_RETIRED_NUMERIC_TYPE_RE.search(type_name) is not None:
        raise TypeError(f"{path} contains a retired Kotodama numeric type")
    return type_name


def _contract_string_tuple(value: Any, path: str) -> Tuple[str, ...]:
    return tuple(
        _contract_required_string(item, f"{path}[{index}]")
        for index, item in enumerate(_contract_array(value, path))
    )


def _canonical_kotodama_identifier(
    value: str, *, declaration: bool = False, type_declaration: bool = False
) -> bool:
    return (
        _KOTODAMA_IDENTIFIER_RE.fullmatch(value) is not None
        and value not in _KOTODAMA_RESERVED_IDENTIFIERS
        and not value.startswith("__kotodama_link_")
        and (
            not declaration
            or value not in _KOTODAMA_RESERVED_DECLARATION_IDENTIFIERS
        )
        and (
            not type_declaration
            or (
                value not in _KOTODAMA_RESERVED_DECLARATION_IDENTIFIERS
                and value not in _KOTODAMA_RETIRED_NUMERIC_TYPE_NAMES
            )
        )
    )


def _canonical_kotodama_entrypoint(value: str) -> bool:
    return value in {"hajimari", "始まり", "kaizen", "改善"} or (
        _canonical_kotodama_identifier(value, declaration=True)
    )


def _contract_hash_crc16(body: str) -> int:
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return crc


def _contract_canonical_hash_hex(value: Any, path: str) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{path} must be a canonical checksummed Norito Hash literal")
    matched = re.fullmatch(r"hash:([0-9A-F]{64})#([0-9A-F]{4})", value)
    if matched is None:
        raise TypeError(f"{path} must be a canonical checksummed Norito Hash literal")
    body, checksum = matched.groups()
    expected = _contract_hash_crc16(body)
    if int(checksum, 16) != expected:
        raise TypeError(f"{path} has an invalid Norito literal checksum")
    raw = bytes.fromhex(body)
    if raw[-1] & 1 != 1:
        raise TypeError(f"{path} must set the Iroha Hash marker bit")
    return body.lower()


def _contract_hash_convenience_hex(value: Any, path: str) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise TypeError(f"{path} must be canonical lowercase 64-hex")
    if bytes.fromhex(value)[-1] & 1 != 1:
        raise TypeError(f"{path} must set the Iroha Hash marker bit")
    return value


class ContractEntrypointKind(str, Enum):
    """Canonical V1 category encoded in an entrypoint descriptor."""

    KOTOAGE = "Kotoage"
    VIEW = "View"
    HAJIMARI = "Hajimari"
    KAIZEN = "Kaizen"

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractEntrypointKind":
        tagged = _contract_object(payload, "entrypoint kind")
        if "value" not in tagged or tagged["value"] is not None:
            raise TypeError("entrypoint kind `value` must be null")
        label = _contract_required_string(tagged.get("kind"), "entrypoint kind.kind")
        try:
            return cls(label)
        except ValueError as exc:
            raise TypeError(f"unsupported Kotodama entrypoint kind `{label}`") from exc


class EntrypointValueKindV1(str, Enum):
    """Leaf representation used by the exact V1 public boundary schema."""

    INT = "Int"
    DECIMAL = "Decimal"
    QUANTITY = "Quantity"
    BOOL = "Bool"
    STRING = "String"
    JSON = "Json"
    NAME = "Name"
    ACCOUNT_ID = "AccountId"
    ASSET_DEFINITION_ID = "AssetDefinitionId"
    ASSET_ID = "AssetId"
    DOMAIN_ID = "DomainId"
    NFT_ID = "NftId"
    DATA_SPACE_ID = "DataSpaceId"
    BLOB = "Blob"

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "EntrypointValueKindV1":
        tagged = _contract_object(payload, "entrypoint value kind")
        if "value" not in tagged or tagged["value"] is not None:
            raise TypeError("entrypoint value kind `value` must be null")
        label = _contract_required_string(tagged.get("kind"), "entrypoint value kind.kind")
        try:
            return cls(label)
        except ValueError as exc:
            raise TypeError(f"unsupported Kotodama boundary value kind `{label}`") from exc


class EntrypointValueTypeNodeKindV1(str, Enum):
    """One exact V1 recursive boundary-schema node category."""

    STRUCT = "Struct"
    TUPLE = "Tuple"
    OPTION = "Option"
    RESULT = "Result"
    LIST = "List"
    LEAF = "Leaf"


_RESERVED_ENTRYPOINT_STRUCT_NAMES = frozenset(
    {
        "AccountView",
        "AssetView",
        "AssetDefinitionView",
        "DomainView",
        "NftView",
        "QueryPage",
    }
)


@dataclass(frozen=True)
class EntrypointStructTypeNodeV1:
    """Named product metadata in an exact V1 boundary schema."""

    name: str
    fields: Tuple[str, ...]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "EntrypointStructTypeNodeV1":
        value = _contract_object(payload, "entrypoint struct node")
        return cls(
            name=_contract_required_string(value.get("name"), "entrypoint struct node.name"),
            fields=_contract_string_tuple(
                value.get("fields"), "entrypoint struct node.fields"
            ),
        )


@dataclass(frozen=True)
class EntrypointListTypeNodeV1:
    """Bounded-list metadata in the flat V1 boundary-schema tape."""

    capacity: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "EntrypointListTypeNodeV1":
        value = _contract_object(payload, "entrypoint list node")
        if set(value) != {"capacity"}:
            raise TypeError(
                "entrypoint list node must contain only `capacity`; "
                "its element subtree follows in the enclosing node tape"
            )
        capacity = value.get("capacity")
        if isinstance(capacity, bool) or not isinstance(capacity, int):
            raise TypeError("entrypoint list node.capacity must be an integer")
        if not 1 <= capacity <= 64:
            raise TypeError("entrypoint list node.capacity must be in 1..64")
        return cls(capacity=capacity)


@dataclass(frozen=True)
class EntrypointValueTypeNodeV1:
    """One typed preorder node in an exact V1 public boundary schema."""

    kind: EntrypointValueTypeNodeKindV1
    value: Any

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "EntrypointValueTypeNodeV1":
        tagged = _contract_object(payload, "entrypoint value type node")
        label = _contract_required_string(
            tagged.get("kind"), "entrypoint value type node.kind"
        )
        try:
            kind = EntrypointValueTypeNodeKindV1(label)
        except ValueError as exc:
            raise TypeError(f"unsupported Kotodama boundary type node `{label}`") from exc
        if "value" not in tagged:
            raise TypeError("entrypoint value type node is missing `value`")
        raw_value = tagged["value"]
        if kind is EntrypointValueTypeNodeKindV1.STRUCT:
            value: Any = EntrypointStructTypeNodeV1.from_payload(raw_value)
        elif kind is EntrypointValueTypeNodeKindV1.TUPLE:
            if isinstance(raw_value, bool) or not isinstance(raw_value, int):
                raise TypeError("entrypoint tuple arity must be an integer")
            if not 2 <= raw_value <= 0xFFFF:
                raise TypeError("entrypoint tuple arity must be in 2..65535")
            value = raw_value
        elif kind in (
            EntrypointValueTypeNodeKindV1.OPTION,
            EntrypointValueTypeNodeKindV1.RESULT,
        ):
            if raw_value is not None:
                raise TypeError(f"entrypoint {kind.value} node `value` must be null")
            value = None
        elif kind is EntrypointValueTypeNodeKindV1.LIST:
            value = EntrypointListTypeNodeV1.from_payload(raw_value)
        else:
            value = EntrypointValueKindV1.from_payload(raw_value)
        return cls(kind=kind, value=value)


@dataclass(frozen=True)
class EntrypointValueTypeV1:
    """Validated preorder representation of one exact V1 boundary type."""

    nodes: Tuple[EntrypointValueTypeNodeV1, ...]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "EntrypointValueTypeV1":
        value = _contract_object(payload, "entrypoint value type")
        nodes = tuple(
            EntrypointValueTypeNodeV1.from_payload(node)
            for node in _contract_array(value.get("nodes"), "entrypoint value type.nodes")
        )
        result = cls(nodes=nodes)
        analysis = result._analyze(1)
        if analysis is None:
            raise TypeError("entrypoint value type is not a valid canonical V1 schema")
        try:
            result.canonical_type_name
        except ValueError as exc:
            raise TypeError("entrypoint value type contains a forged reserved V1 schema") from exc
        return result

    def _analyze(self, root_depth: int) -> Optional[Tuple[int, int, int]]:
        """Return `(node_count, word_count, max_depth)` for a canonical schema."""

        if not self.nodes or len(self.nodes) > 256 or root_depth != 1:
            return None

        def child_count(node: EntrypointValueTypeNodeV1) -> Optional[int]:
            if node.kind is EntrypointValueTypeNodeKindV1.STRUCT:
                if not isinstance(node.value, EntrypointStructTypeNodeV1):
                    return None
                return len(node.value.fields)
            if node.kind is EntrypointValueTypeNodeKindV1.TUPLE:
                return node.value if isinstance(node.value, int) else None
            if node.kind in (
                EntrypointValueTypeNodeKindV1.OPTION,
                EntrypointValueTypeNodeKindV1.LIST,
            ):
                return 1
            if node.kind is EntrypointValueTypeNodeKindV1.RESULT:
                return 2
            if node.kind is EntrypointValueTypeNodeKindV1.LEAF:
                return 0
            return None

        frames: list[dict[str, Any]] = []
        word_count = 0
        max_depth = 0
        for index, node in enumerate(self.nodes):
            while frames and frames[-1]["remaining"] == 0:
                frames.pop()
            suppress_words = False
            if index != 0:
                if not frames or frames[-1]["remaining"] == 0:
                    return None
                frames[-1]["remaining"] -= 1
                suppress_words = bool(frames[-1]["suppress_words"])
            depth = len(frames) + 1
            if depth > 256:
                return None
            max_depth = max(max_depth, depth)

            if node.kind is EntrypointValueTypeNodeKindV1.STRUCT:
                descriptor = node.value
                reserved_schema_name = (
                    isinstance(descriptor, EntrypointStructTypeNodeV1)
                    and descriptor.name in _RESERVED_ENTRYPOINT_STRUCT_NAMES
                )
                if (
                    not isinstance(descriptor, EntrypointStructTypeNodeV1)
                    or not descriptor.fields
                    or (
                        not reserved_schema_name
                        and not _canonical_kotodama_identifier(
                            descriptor.name, type_declaration=True
                        )
                    )
                    or any(
                        not _canonical_kotodama_identifier(field)
                        for field in descriptor.fields
                    )
                    or len(set(descriptor.fields)) != len(descriptor.fields)
                ):
                    return None
            elif node.kind is EntrypointValueTypeNodeKindV1.TUPLE:
                if not isinstance(node.value, int) or not 2 <= node.value <= 0xFFFF:
                    return None
            elif node.kind is EntrypointValueTypeNodeKindV1.LIST:
                if not isinstance(node.value, EntrypointListTypeNodeV1):
                    return None
            elif node.kind is EntrypointValueTypeNodeKindV1.LEAF:
                if not isinstance(node.value, EntrypointValueKindV1):
                    return None

            handle = node.kind in (
                EntrypointValueTypeNodeKindV1.OPTION,
                EntrypointValueTypeNodeKindV1.RESULT,
                EntrypointValueTypeNodeKindV1.LIST,
            )
            if not suppress_words and (
                handle or node.kind is EntrypointValueTypeNodeKindV1.LEAF
            ):
                word_count += 1
            children = child_count(node)
            if children is None:
                return None
            if children:
                frames.append(
                    {
                        "remaining": children,
                        "suppress_words": suppress_words or handle,
                    }
                )
        while frames and frames[-1]["remaining"] == 0:
            frames.pop()
        if frames:
            return None
        return len(self.nodes), word_count, max_depth

    @property
    def word_count(self) -> int:
        """Return the fixed V1 ABI word count after schema validation."""

        analysis = self._analyze(1)
        if analysis is None:
            raise ValueError("invalid entrypoint value type")
        return analysis[1]

    @property
    def canonical_type_name(self) -> str:
        """Render the exact canonical Kotodama V1 type name."""

        leaf_names = {
            EntrypointValueKindV1.INT: "int",
            EntrypointValueKindV1.DECIMAL: "decimal",
            EntrypointValueKindV1.QUANTITY: "quantity",
            EntrypointValueKindV1.BOOL: "bool",
            EntrypointValueKindV1.STRING: "string",
            EntrypointValueKindV1.JSON: "Json",
            EntrypointValueKindV1.NAME: "Name",
            EntrypointValueKindV1.ACCOUNT_ID: "AccountId",
            EntrypointValueKindV1.ASSET_DEFINITION_ID: "AssetDefinitionId",
            EntrypointValueKindV1.ASSET_ID: "AssetId",
            EntrypointValueKindV1.DOMAIN_ID: "DomainId",
            EntrypointValueKindV1.NFT_ID: "NftId",
            EntrypointValueKindV1.DATA_SPACE_ID: "DataSpaceId",
            EntrypointValueKindV1.BLOB: "bytes",
        }

        core_views = {
            "AccountView": (["id", "metadata"], ["AccountId", "Json"]),
            "AssetView": (["id", "amount"], ["AssetId", "quantity"]),
            "AssetDefinitionView": (
                [
                    "id",
                    "name",
                    "description",
                    "owned_by",
                    "total_quantity",
                    "metadata",
                ],
                [
                    "AssetDefinitionId",
                    "string",
                    "Option<string>",
                    "AccountId",
                    "quantity",
                    "Json",
                ],
            ),
            "DomainView": (
                ["id", "owned_by", "metadata"],
                ["DomainId", "AccountId", "Json"],
            ),
            "NftView": (
                ["id", "owned_by", "content"],
                ["NftId", "AccountId", "Json"],
            ),
        }

        def child_count(node: EntrypointValueTypeNodeV1) -> int:
            if node.kind is EntrypointValueTypeNodeKindV1.STRUCT:
                return len(node.value.fields)
            if node.kind is EntrypointValueTypeNodeKindV1.TUPLE:
                return int(node.value)
            if node.kind in (
                EntrypointValueTypeNodeKindV1.OPTION,
                EntrypointValueTypeNodeKindV1.LIST,
            ):
                return 1
            if node.kind is EntrypointValueTypeNodeKindV1.RESULT:
                return 2
            return 0

        rendered: list[Dict[str, Any]] = []
        for node in reversed(self.nodes):
            count = child_count(node)
            if len(rendered) < count:
                raise ValueError("invalid entrypoint value type")
            children = rendered[len(rendered) - count :] if count else []
            if count:
                del rendered[len(rendered) - count :]
                children.reverse()

            if node.kind is EntrypointValueTypeNodeKindV1.STRUCT:
                descriptor = node.value
                if not isinstance(descriptor, EntrypointStructTypeNodeV1):
                    raise ValueError("invalid struct node")
                child_names = [child["text"] for child in children]
                if descriptor.name in core_views:
                    expected_fields, expected_children = core_views[descriptor.name]
                    if list(descriptor.fields) != expected_fields or child_names != expected_children:
                        raise ValueError("forged reserved query view")
                    result = {"text": descriptor.name, "core_view": descriptor.name}
                elif descriptor.name == "QueryPage":
                    if (
                        list(descriptor.fields) != ["items", "next_offset"]
                        or len(children) != 2
                        or children[0].get("kind") != "List"
                        or children[0].get("capacity") != 64
                        or children[0].get("list_element_core_view") is None
                        or children[1]["text"] != "Option<int>"
                    ):
                        raise ValueError("forged QueryPage schema")
                    result = {
                        "text": f"QueryPage<{children[0]['list_element_core_view']}>"
                    }
                else:
                    result = {"text": f"struct {descriptor.name}"}
            elif node.kind is EntrypointValueTypeNodeKindV1.TUPLE:
                result = {"text": f"({', '.join(child['text'] for child in children)})"}
            elif node.kind is EntrypointValueTypeNodeKindV1.OPTION:
                result = {"text": f"Option<{children[0]['text']}>"}
            elif node.kind is EntrypointValueTypeNodeKindV1.RESULT:
                result = {"text": f"Result<{children[0]['text']}, {children[1]['text']}>"}
            elif node.kind is EntrypointValueTypeNodeKindV1.LIST:
                descriptor = node.value
                if not isinstance(descriptor, EntrypointListTypeNodeV1):
                    raise ValueError("invalid list node")
                result = {
                    "text": f"List<{children[0]['text']}, {descriptor.capacity}>",
                    "kind": "List",
                    "capacity": descriptor.capacity,
                    "list_element_core_view": children[0].get("core_view"),
                }
            elif node.kind is EntrypointValueTypeNodeKindV1.LEAF:
                if not isinstance(node.value, EntrypointValueKindV1):
                    raise ValueError("invalid leaf node")
                result = {"text": leaf_names[node.value]}
            else:
                raise ValueError("invalid entrypoint value type")
            rendered.append(result)

        if len(rendered) != 1:
            raise ValueError("invalid entrypoint value type")
        return str(rendered[0]["text"])


@dataclass(frozen=True)
class EntrypointArgumentFieldV1:
    """One named field in a canonical V1 argument record."""

    name: str
    type: EntrypointValueTypeV1

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "EntrypointArgumentFieldV1":
        value = _contract_object(payload, "entrypoint argument field")
        return cls(
            name=_contract_required_string(
                value.get("name"), "entrypoint argument field.name"
            ),
            type=EntrypointValueTypeV1.from_payload(
                _contract_object(value.get("ty"), "entrypoint argument field.ty")
            ),
        )


@dataclass(frozen=True)
class EntrypointArgumentSchemaV1:
    """Exact canonical V1 schema for one public argument record."""

    fields: Tuple[EntrypointArgumentFieldV1, ...]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "EntrypointArgumentSchemaV1":
        value = _contract_object(payload, "entrypoint argument schema")
        fields = tuple(
            EntrypointArgumentFieldV1.from_payload(field)
            for field in _contract_array(
                value.get("fields"), "entrypoint argument schema.fields"
            )
        )
        names = [field.name for field in fields]
        if (
            not 1 <= len(fields) <= 13
            or any(not _canonical_kotodama_identifier(name) for name in names)
            or len(set(names)) != len(names)
            or sum(field.type.word_count for field in fields) > 13
        ):
            raise TypeError("entrypoint argument schema violates canonical V1 bounds")
        return cls(fields=fields)


@dataclass(frozen=True)
class ContractEntrypointParameter:
    """One declared public Kotodama parameter."""

    name: str
    type_name: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractEntrypointParameter":
        value = _contract_object(payload, "entrypoint parameter")
        return cls(
            name=_contract_required_string(value.get("name"), "entrypoint parameter.name"),
            type_name=_contract_type_name(
                value.get("type_name"), "entrypoint parameter.type_name"
            ),
        )


def _contract_trigger_descriptor(
    payload: Mapping[str, Any], path: str
) -> Mapping[str, Any]:
    value = _contract_object(payload, path)
    trigger_id = _contract_required_string(value.get("id"), f"{path}.id")
    repeats = _contract_object(value.get("repeats"), f"{path}.repeats")
    if len(repeats) != 1 or next(iter(repeats)) not in {"Indefinitely", "Exactly"}:
        raise TypeError(f"{path}.repeats must contain exactly one canonical variant")
    repeat_kind, repeat_value = next(iter(repeats.items()))
    if repeat_kind == "Indefinitely":
        if repeat_value is not None:
            raise TypeError(f"{path}.repeats.Indefinitely must be null")
    elif (
        isinstance(repeat_value, bool)
        or not isinstance(repeat_value, int)
        or not 0 <= repeat_value <= 0xFFFFFFFF
    ):
        raise TypeError(f"{path}.repeats.Exactly must be a u32")

    encoded_filter = value.get("filter")
    if not isinstance(encoded_filter, str) or not encoded_filter:
        raise TypeError(f"{path}.filter must be non-empty exact standard-base64")
    try:
        decoded_filter = base64.b64decode(encoded_filter, validate=True)
    except (binascii.Error, ValueError) as exc:
        raise TypeError(f"{path}.filter must be exact standard-base64") from exc
    if not decoded_filter or base64.b64encode(decoded_filter).decode("ascii") != encoded_filter:
        raise TypeError(f"{path}.filter must be non-empty exact standard-base64")

    authority = value.get("authority")
    if authority is not None:
        _contract_required_string(authority, f"{path}.authority")
    metadata = _contract_object(value.get("metadata", {}), f"{path}.metadata")
    callback = _contract_object(value.get("callback"), f"{path}.callback")
    namespace = callback.get("namespace")
    if namespace is not None:
        _contract_required_string(namespace, f"{path}.callback.namespace")
    callback_entrypoint = _contract_required_string(
        callback.get("entrypoint"), f"{path}.callback.entrypoint"
    )
    if not _canonical_kotodama_entrypoint(callback_entrypoint):
        raise TypeError(f"{path}.callback.entrypoint is not canonical")
    return copy.deepcopy(
        {
            "id": trigger_id,
            "repeats": dict(repeats),
            "filter": encoded_filter,
            "authority": authority,
            "metadata": dict(metadata),
            "callback": {
                "namespace": namespace,
                "entrypoint": callback_entrypoint,
            },
        }
    )


@dataclass(frozen=True)
class ContractEntrypointDescriptor:
    """Exact public interface metadata for one Kotodama entrypoint."""

    name: str
    kind: ContractEntrypointKind
    params: Tuple[ContractEntrypointParameter, ...]
    argument_schema: Optional[EntrypointArgumentSchemaV1]
    return_type: Optional[str]
    return_schema: Optional[EntrypointValueTypeV1]
    permission: Optional[str]
    read_keys: Tuple[str, ...]
    write_keys: Tuple[str, ...]
    access_hints_complete: Optional[bool]
    access_hints_skipped: Tuple[str, ...]
    triggers: Tuple[Mapping[str, Any], ...]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractEntrypointDescriptor":
        value = _contract_object(payload, "entrypoint descriptor")
        params_raw = value.get("params", ())
        params = tuple(
            ContractEntrypointParameter.from_payload(param)
            for param in _contract_array(params_raw, "entrypoint descriptor.params")
        )
        argument_schema_raw = value.get("argument_schema")
        argument_schema = (
            None
            if argument_schema_raw is None
            else EntrypointArgumentSchemaV1.from_payload(argument_schema_raw)
        )
        return_schema_raw = value.get("return_schema")
        return_schema = (
            None
            if return_schema_raw is None
            else EntrypointValueTypeV1.from_payload(return_schema_raw)
        )
        access_hints_complete = value.get("access_hints_complete")
        if access_hints_complete is not None and not isinstance(access_hints_complete, bool):
            raise TypeError("entrypoint descriptor.access_hints_complete must be a boolean")
        trigger_values = []
        for index, trigger in enumerate(
            _contract_array(value.get("triggers", ()), "entrypoint descriptor.triggers")
        ):
            trigger_values.append(
                _contract_trigger_descriptor(
                    _contract_object(
                        trigger, f"entrypoint descriptor.triggers[{index}]"
                    ),
                    f"entrypoint descriptor.triggers[{index}]",
                )
            )
        name = _contract_required_string(value.get("name"), "entrypoint descriptor.name")
        if not _canonical_kotodama_entrypoint(name):
            raise TypeError("entrypoint descriptor.name is not canonical")
        descriptor = cls(
            name=name,
            kind=ContractEntrypointKind.from_payload(
                _contract_object(value.get("kind"), "entrypoint descriptor.kind")
            ),
            params=params,
            argument_schema=argument_schema,
            return_type=(
                None
                if value.get("return_type") is None
                else _contract_type_name(
                    value.get("return_type"), "entrypoint descriptor.return_type"
                )
            ),
            return_schema=return_schema,
            permission=_contract_optional_string(
                value.get("permission"), "entrypoint descriptor.permission"
            ),
            read_keys=_contract_string_tuple(
                value.get("read_keys", ()), "entrypoint descriptor.read_keys"
            ),
            write_keys=_contract_string_tuple(
                value.get("write_keys", ()), "entrypoint descriptor.write_keys"
            ),
            access_hints_complete=access_hints_complete,
            access_hints_skipped=_contract_string_tuple(
                value.get("access_hints_skipped", ()),
                "entrypoint descriptor.access_hints_skipped",
            ),
            triggers=tuple(trigger_values),
        )
        parameter_names = [parameter.name for parameter in descriptor.params]
        schema_names = (
            None
            if descriptor.argument_schema is None
            else [field.name for field in descriptor.argument_schema.fields]
        )
        exact_arguments = (
            descriptor.argument_schema is None
            if not descriptor.params
            else schema_names == parameter_names
            and all(
                field.type.canonical_type_name == parameter.type_name
                for field, parameter in zip(
                    descriptor.argument_schema.fields, descriptor.params
                )
            )
        )
        exact_return = (descriptor.return_type is None) == (
            descriptor.return_schema is None
        ) and (
            descriptor.return_schema is None
            or (
                descriptor.return_schema.word_count <= 13
                and descriptor.return_schema.canonical_type_name
                == descriptor.return_type
            )
        )
        lifecycle_kind = (
            ContractEntrypointKind.HAJIMARI
            if descriptor.name in {"hajimari", "始まり"}
            else ContractEntrypointKind.KAIZEN
            if descriptor.name in {"kaizen", "改善"}
            else None
        )
        exact_lifecycle = (
            descriptor.kind is lifecycle_kind
            if lifecycle_kind is not None
            else descriptor.kind
            not in {ContractEntrypointKind.HAJIMARI, ContractEntrypointKind.KAIZEN}
        )
        exact_authorization = (
            descriptor.permission is not None
            if descriptor.kind is ContractEntrypointKind.KOTOAGE
            else descriptor.permission is None
            if descriptor.kind
            in {ContractEntrypointKind.HAJIMARI, ContractEntrypointKind.KAIZEN}
            else True
        )
        exact_access_hints = not (
            descriptor.access_hints_complete is True
            and descriptor.access_hints_skipped
        ) and not (
            descriptor.access_hints_complete is False
            and not descriptor.access_hints_skipped
        )
        if (
            len(descriptor.params) > 13
            or len(set(parameter_names)) != len(parameter_names)
            or any(
                not _canonical_kotodama_identifier(parameter.name)
                for parameter in descriptor.params
            )
            or not exact_arguments
            or not exact_return
            or not exact_lifecycle
            or not exact_authorization
            or not exact_access_hints
        ):
            raise TypeError("entrypoint descriptor is not a canonical exact V1 interface")
        return descriptor


@dataclass(frozen=True)
class ContractStateDescriptor:
    """One durable state slot advertised by a Kotodama seiyaku."""

    name: str
    type_name: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractStateDescriptor":
        value = _contract_object(payload, "state descriptor")
        name = _contract_required_string(value.get("name"), "state descriptor.name")
        if not _canonical_kotodama_identifier(name, declaration=True):
            raise TypeError("state descriptor.name must be a canonical Kotodama identifier")
        return cls(
            name=name,
            type_name=_contract_type_name(
                value.get("type_name"), "state descriptor.type_name"
            ),
        )


@dataclass(frozen=True)
class ContractErrorCodeDescriptor:
    """One stable declared Kotodama application error code."""

    namespace: str
    name: str
    code: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractErrorCodeDescriptor":
        value = _contract_object(payload, "error code descriptor")
        code = value.get("code")
        if isinstance(code, bool) or not isinstance(code, int) or not 1 <= code <= 0xFFFFFFFF:
            raise TypeError("error code descriptor.code must be a non-zero u32")
        namespace = _contract_required_string(
            value.get("namespace"), "error code descriptor.namespace"
        )
        name = _contract_required_string(value.get("name"), "error code descriptor.name")
        if not _canonical_kotodama_identifier(
            namespace, type_declaration=True
        ) or not _canonical_kotodama_identifier(name):
            raise TypeError("error code names must be canonical Kotodama identifiers")
        return cls(namespace=namespace, name=name, code=code)


@dataclass(frozen=True)
class ContractDynamicAccessHint:
    """One bounded dynamic access-set hint from the compiler."""

    base_key: str
    key_type: str
    bound_kind: str
    max_keys: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractDynamicAccessHint":
        value = _contract_object(payload, "dynamic access hint")
        max_keys = value.get("max_keys")
        if isinstance(max_keys, bool) or not isinstance(max_keys, int):
            raise TypeError("dynamic access hint.max_keys must be an integer")
        if not 0 <= max_keys <= 0xFFFFFFFF:
            raise TypeError("dynamic access hint.max_keys must be a u32")
        return cls(
            base_key=_contract_required_string(
                value.get("base_key"), "dynamic access hint.base_key"
            ),
            key_type=_contract_type_name(
                value.get("key_type"), "dynamic access hint.key_type"
            ),
            bound_kind=_contract_required_string(
                value.get("bound_kind"), "dynamic access hint.bound_kind"
            ),
            max_keys=max_keys,
        )


@dataclass(frozen=True)
class ContractAccessSetHints:
    """Exact static and bounded-dynamic scheduler hints in a manifest."""

    read_keys: Tuple[str, ...]
    write_keys: Tuple[str, ...]
    dynamic_reads: Tuple[ContractDynamicAccessHint, ...]
    dynamic_writes: Tuple[ContractDynamicAccessHint, ...]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractAccessSetHints":
        value = _contract_object(payload, "access set hints")

        def dynamic(name: str) -> Tuple[ContractDynamicAccessHint, ...]:
            return tuple(
                ContractDynamicAccessHint.from_payload(item)
                for item in _contract_array(value.get(name, ()), f"access set hints.{name}")
            )

        return cls(
            read_keys=_contract_string_tuple(value.get("read_keys"), "access set hints.read_keys"),
            write_keys=_contract_string_tuple(
                value.get("write_keys"), "access set hints.write_keys"
            ),
            dynamic_reads=dynamic("dynamic_reads"),
            dynamic_writes=dynamic("dynamic_writes"),
        )


@dataclass(frozen=True)
class ContractKotobaTranslation:
    """One localized message text in a Kotodama manifest."""

    language: str
    text: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractKotobaTranslation":
        value = _contract_object(payload, "kotoba translation")
        text = value.get("text")
        if not isinstance(text, str):
            raise TypeError("kotoba translation.text must be a string")
        return cls(
            language=_contract_required_string(value.get("lang"), "kotoba translation.lang"),
            text=text,
        )


@dataclass(frozen=True)
class ContractKotobaTranslationEntry:
    """One stable message id and its localized Kotodama texts."""

    message_id: str
    translations: Tuple[ContractKotobaTranslation, ...]

    @classmethod
    def from_payload(
        cls, payload: Mapping[str, Any]
    ) -> "ContractKotobaTranslationEntry":
        value = _contract_object(payload, "kotoba translation entry")
        translations = tuple(
            ContractKotobaTranslation.from_payload(item)
            for item in _contract_array(
                value.get("translations"), "kotoba translation entry.translations"
            )
        )
        return cls(
            message_id=_contract_required_string(
                value.get("msg_id"), "kotoba translation entry.msg_id"
            ),
            translations=translations,
        )


@dataclass(frozen=True)
class ContractManifest:
    """On-chain contract manifest metadata with its exact V1 public interface."""

    seiyaku_name: Optional[str]
    code_hash: Optional[str]
    abi_hash: Optional[str]
    compiler_fingerprint: Optional[str]
    features_bitmap: Optional[int]
    access_set_hints: Optional[ContractAccessSetHints]
    entrypoints: Optional[Tuple[ContractEntrypointDescriptor, ...]]
    states: Optional[Tuple[ContractStateDescriptor, ...]]
    error_codes: Optional[Tuple[ContractErrorCodeDescriptor, ...]]
    kotoba: Optional[Tuple[ContractKotobaTranslationEntry, ...]]
    provenance: Optional[Mapping[str, Any]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractManifest":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest payload must be an object")
        allowed_fields = {
            "seiyaku_name",
            "code_hash",
            "abi_hash",
            "compiler_fingerprint",
            "features_bitmap",
            "access_set_hints",
            "entrypoints",
            "states",
            "error_codes",
            "kotoba",
            "provenance",
        }
        unknown_fields = sorted(set(payload) - allowed_fields)
        if unknown_fields:
            raise TypeError(
                "manifest payload contains unsupported fields: "
                + ", ".join(unknown_fields)
            )
        seiyaku_name = payload.get("seiyaku_name")
        if seiyaku_name is not None and (
            not isinstance(seiyaku_name, str) or not seiyaku_name
        ):
            raise TypeError("manifest `seiyaku_name` must be a non-empty string when provided")
        if seiyaku_name is not None and not _canonical_kotodama_identifier(
            seiyaku_name, type_declaration=True
        ):
            raise TypeError("manifest `seiyaku_name` must be a canonical Kotodama identifier")
        code_hash = _contract_canonical_hash_hex(
            payload.get("code_hash"), "manifest `code_hash`"
        )
        abi_hash = _contract_canonical_hash_hex(
            payload.get("abi_hash"), "manifest `abi_hash`"
        )
        compiler_fingerprint = payload.get("compiler_fingerprint")
        if compiler_fingerprint is not None and (
            not isinstance(compiler_fingerprint, str)
            or not compiler_fingerprint.strip()
            or compiler_fingerprint.strip() != compiler_fingerprint
        ):
            raise TypeError(
                "manifest `compiler_fingerprint` must be a non-empty string when provided"
            )
        features_raw = payload.get("features_bitmap")
        if features_raw is None:
            features_bitmap: Optional[int] = None
        elif isinstance(features_raw, bool) or not isinstance(features_raw, int):
            raise TypeError("manifest `features_bitmap` must be an unsigned integer")
        elif not 0 <= features_raw <= 0xFFFFFFFFFFFFFFFF:
            raise TypeError("manifest `features_bitmap` must be a u64")
        else:
            features_bitmap = features_raw

        access_set_hints_raw = payload.get("access_set_hints")
        access_set_hints = (
            None
            if access_set_hints_raw is None
            else ContractAccessSetHints.from_payload(access_set_hints_raw)
        )

        def optional_descriptors(
            name: str, parser: Callable[[Mapping[str, Any]], Any]
        ) -> Optional[Tuple[Any, ...]]:
            raw = payload.get(name)
            if raw is None:
                return None
            return tuple(
                parser(_contract_object(item, f"manifest.{name}[{index}]"))
                for index, item in enumerate(_contract_array(raw, f"manifest.{name}"))
            )

        provenance_raw = payload.get("provenance")
        provenance = (
            None
            if provenance_raw is None
            else copy.deepcopy(
                dict(_contract_object(provenance_raw, "manifest.provenance"))
            )
        )

        entrypoints = optional_descriptors(
            "entrypoints", ContractEntrypointDescriptor.from_payload
        )
        states = optional_descriptors("states", ContractStateDescriptor.from_payload)
        error_codes = optional_descriptors(
            "error_codes", ContractErrorCodeDescriptor.from_payload
        )
        kotoba = optional_descriptors(
            "kotoba", ContractKotobaTranslationEntry.from_payload
        )

        if entrypoints is not None:
            entrypoint_names = [entrypoint.name for entrypoint in entrypoints]
            lifecycle_kinds = [
                entrypoint.kind
                for entrypoint in entrypoints
                if entrypoint.kind
                in {ContractEntrypointKind.HAJIMARI, ContractEntrypointKind.KAIZEN}
            ]
            if len(set(entrypoint_names)) != len(entrypoint_names) or len(
                set(lifecycle_kinds)
            ) != len(lifecycle_kinds):
                raise TypeError("manifest contains duplicate entrypoint declarations")
            entrypoint_kinds = {
                entrypoint.name: entrypoint.kind for entrypoint in entrypoints
            }
            trigger_ids = set()
            for entrypoint in entrypoints:
                for trigger in entrypoint.triggers:
                    trigger_id = trigger["id"]
                    if trigger_id in trigger_ids:
                        raise TypeError("manifest contains duplicate trigger ids")
                    trigger_ids.add(trigger_id)
                    callback = trigger["callback"]
                    if callback["namespace"] is None:
                        target_kind = entrypoint_kinds.get(callback["entrypoint"])
                        if target_kind is None:
                            raise TypeError(
                                "manifest trigger targets an undeclared local entrypoint"
                            )
                        if target_kind is not ContractEntrypointKind.KOTOAGE:
                            raise TypeError(
                                "manifest local trigger callback must target kotoage/言挙げ"
                            )

        if states is not None and len({state.name for state in states}) != len(states):
            raise TypeError("manifest contains duplicate state descriptors")
        if error_codes is not None:
            paths = {(error.namespace, error.name) for error in error_codes}
            codes = {error.code for error in error_codes}
            if len(paths) != len(error_codes) or len(codes) != len(error_codes):
                raise TypeError("manifest contains duplicate error paths or numeric codes")
        if kotoba is not None:
            message_ids = [entry.message_id for entry in kotoba]
            if len(set(message_ids)) != len(message_ids):
                raise TypeError("manifest contains duplicate kotoba message ids")
            for entry in kotoba:
                languages = [translation.language for translation in entry.translations]
                if len(set(languages)) != len(languages):
                    raise TypeError("manifest contains duplicate kotoba languages")

        return cls(
            seiyaku_name=seiyaku_name,
            code_hash=code_hash,
            abi_hash=abi_hash,
            compiler_fingerprint=compiler_fingerprint,
            features_bitmap=features_bitmap,
            access_set_hints=access_set_hints,
            entrypoints=entrypoints,
            states=states,
            error_codes=error_codes,
            kotoba=kotoba,
            provenance=provenance,
        )


@dataclass(frozen=True)
class ContractManifestRecord:
    """Contract manifest record returned by Torii (`/v1/contracts/code/{hash}`)."""

    manifest: ContractManifest
    code_hash: Optional[str]
    abi_hash: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ContractManifestRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest response must be an object")
        manifest_payload = payload.get("manifest")
        if not isinstance(manifest_payload, Mapping):
            raise TypeError("manifest response missing object `manifest` field")
        if "code_bytes" in payload:
            raise TypeError("manifest response must not inline `code_bytes`")
        manifest = ContractManifest.from_payload(manifest_payload)
        code_hash = _contract_hash_convenience_hex(
            payload.get("code_hash"), "manifest response `code_hash`"
        )
        abi_hash = _contract_hash_convenience_hex(
            payload.get("abi_hash"), "manifest response `abi_hash`"
        )
        if code_hash != manifest.code_hash or abi_hash != manifest.abi_hash:
            raise TypeError(
                "top-level contract hash conveniences must exactly match "
                "the canonical manifest hashes"
            )
        return cls(manifest=manifest, code_hash=code_hash, abi_hash=abi_hash)


@dataclass(frozen=True)
class PeerInfo:
    """Metadata describing an online peer returned by `GET /v1/peers`."""

    address: str
    public_key_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerInfo":
        if not isinstance(payload, Mapping):
            raise TypeError("peer payload must be a mapping")
        address = payload.get("address")
        if not isinstance(address, str):
            raise TypeError("peer payload missing string `address` field")
        id_section = payload.get("id")
        if not isinstance(id_section, Mapping):
            raise TypeError("peer payload missing `id` object")
        public_key = id_section.get("public_key")
        if not isinstance(public_key, str):
            raise TypeError("peer id missing string `public_key` field")
        return cls(address=address, public_key_hex=public_key)


@dataclass(frozen=True)
class PeerTelemetryConfig:
    """Configuration snapshot returned by `/v1/telemetry/peers-info`."""

    public_key_hex: str
    queue_capacity: Optional[int]
    network_block_gossip_size: Optional[int]
    network_block_gossip_period_ms: Optional[int]
    network_tx_gossip_size: Optional[int]
    network_tx_gossip_period_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerTelemetryConfig":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry peer config must be an object")
        public_key = payload.get("public_key")
        if not isinstance(public_key, str) or not public_key:
            raise TypeError("telemetry peer config missing string `public_key` field")
        queue_capacity = _coerce_int(
            payload.get("queue_capacity"),
            "telemetry peer config queue_capacity",
            allow_zero=True,
        )
        block_size = _coerce_int(
            payload.get("network_block_gossip_size"),
            "telemetry peer config network_block_gossip_size",
            allow_zero=True,
        )
        tx_size = _coerce_int(
            payload.get("network_tx_gossip_size"),
            "telemetry peer config network_tx_gossip_size",
            allow_zero=True,
        )
        block_period = _parse_optional_duration_ms_field(
            payload.get("network_block_gossip_period"),
            "telemetry peer config network_block_gossip_period",
        )
        tx_period = _parse_optional_duration_ms_field(
            payload.get("network_tx_gossip_period"),
            "telemetry peer config network_tx_gossip_period",
        )
        return cls(
            public_key_hex=public_key,
            queue_capacity=queue_capacity,
            network_block_gossip_size=block_size,
            network_block_gossip_period_ms=block_period,
            network_tx_gossip_size=tx_size,
            network_tx_gossip_period_ms=tx_period,
        )


@dataclass(frozen=True)
class PeerTelemetryLocation:
    """Geolocation metadata for `/v1/telemetry/peers-info` entries."""

    lat: float
    lon: float
    country: str
    city: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerTelemetryLocation":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry peer location must be an object")
        lat = _coerce_finite_float(payload.get("lat"), "telemetry peer location lat")
        lon = _coerce_finite_float(payload.get("lon"), "telemetry peer location lon")
        country = payload.get("country")
        city = payload.get("city")
        if not isinstance(country, str) or not country:
            raise TypeError("telemetry peer location missing string `country` field")
        if not isinstance(city, str) or not city:
            raise TypeError("telemetry peer location missing string `city` field")
        return cls(lat=lat, lon=lon, country=country, city=city)


@dataclass(frozen=True)
class PeerTelemetryInfo:
    """Entry returned by `GET /v1/telemetry/peers-info`."""

    url: str
    connected: bool
    telemetry_unsupported: bool
    config: Optional[PeerTelemetryConfig]
    location: Optional[PeerTelemetryLocation]
    connected_peers: Optional[List[str]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PeerTelemetryInfo":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry peer payload must be an object")
        url = payload.get("url")
        if not isinstance(url, str) or not url:
            raise TypeError("telemetry peer payload missing string `url` field")
        connected = _coerce_bool_flag(payload.get("connected"), "telemetry peer connected")
        telemetry_flag = payload.get("telemetry_unsupported")
        telemetry_unsupported = _coerce_bool_flag(
            telemetry_flag if telemetry_flag is not None else False,
            "telemetry peer telemetry_unsupported",
        )
        config_payload = payload.get("config")
        if config_payload is not None and not isinstance(config_payload, Mapping):
            raise TypeError("telemetry peer config must be an object when provided")
        location_payload = payload.get("location")
        if location_payload is not None and not isinstance(location_payload, Mapping):
            raise TypeError("telemetry peer location must be an object when provided")
        peers_value = payload.get("connected_peers")
        connected_peers = None
        if peers_value is not None:
            if not isinstance(peers_value, list):
                raise TypeError("telemetry peer `connected_peers` must be a list when provided")
            peer_list: List[str] = []
            for index, peer in enumerate(peers_value):
                if not isinstance(peer, str) or not peer:
                    raise TypeError(f"telemetry peer connected_peers[{index}] must be a non-empty string")
                peer_list.append(peer)
            connected_peers = peer_list
        return cls(
            url=url,
            connected=connected,
            telemetry_unsupported=telemetry_unsupported,
            config=PeerTelemetryConfig.from_payload(config_payload) if config_payload else None,
            location=PeerTelemetryLocation.from_payload(location_payload) if location_payload else None,
            connected_peers=connected_peers,
        )


@dataclass(frozen=True)
class SseEvent:
    """Structured Server-Sent Event returned by Torii SSE endpoints."""

    event: Optional[str]
    data: Any
    id: Optional[str]
    retry: Optional[int]
    raw: str


@dataclass(frozen=True)
class WebSocketEvent:
    """Structured JSON event returned by Torii WebSocket event streams."""

    event: Optional[str]
    data: Any
    raw: str


class SseStreamError(RuntimeError):
    """Terminal error reported after an SSE response has been established.

    Canonical Torii live streams cannot change their HTTP status after sending
    the response headers, so they report a terminal ``event: stream_error``
    frame instead.  The exception keeps the stable server error code and the
    loss/replay metadata available to callers.
    """

    MALFORMED_CODE = "malformed_stream_error"

    def __init__(
        self,
        code: str,
        message: str,
        *,
        dropped_messages: Optional[int],
        replay_available: Optional[bool],
        payload: Any,
        raw: str,
        malformed_reason: Optional[str] = None,
    ) -> None:
        self.code = code
        self.message = message
        self.dropped_messages = dropped_messages
        self.replay_available = replay_available
        self.payload = payload
        self.raw = raw
        self.malformed_reason = malformed_reason
        detail = f"{code}: {message}"
        if dropped_messages is not None:
            detail = f"{detail} (dropped_messages={dropped_messages})"
        super().__init__(detail)

    @classmethod
    def from_event(cls, event: SseEvent) -> "SseStreamError":
        """Validate and convert a terminal ``stream_error`` SSE frame."""

        payload = event.data
        if isinstance(payload, str):
            try:
                payload = json.loads(payload)
            except json.JSONDecodeError:
                return cls._malformed(event, "data must be a JSON object")
        if not isinstance(payload, Mapping):
            return cls._malformed(event, "data must be a JSON object")

        code = payload.get("code")
        if not isinstance(code, str) or not code.strip():
            return cls._malformed(event, "code must be a non-empty string")
        message = payload.get("message")
        if not isinstance(message, str) or not message.strip():
            return cls._malformed(event, "message must be a non-empty string")
        if "dropped_messages" not in payload:
            return cls._malformed(event, "dropped_messages is required")
        dropped_messages = payload["dropped_messages"]
        if dropped_messages is not None and (
            isinstance(dropped_messages, bool)
            or not isinstance(dropped_messages, int)
            or dropped_messages < 0
        ):
            return cls._malformed(
                event,
                "dropped_messages must be a non-negative integer or null",
            )
        if "replay_available" not in payload:
            return cls._malformed(event, "replay_available is required")
        replay_available = payload["replay_available"]
        if not isinstance(replay_available, bool):
            return cls._malformed(event, "replay_available must be a boolean")
        return cls(
            code,
            message,
            dropped_messages=dropped_messages,
            replay_available=replay_available,
            payload=dict(payload),
            raw=event.raw,
        )

    @classmethod
    def _malformed(cls, event: SseEvent, reason: str) -> "SseStreamError":
        return cls(
            cls.MALFORMED_CODE,
            f"Torii emitted a malformed stream_error event: {reason}",
            dropped_messages=None,
            replay_available=None,
            payload=event.data,
            raw=event.raw,
            malformed_reason=reason,
        )


@dataclass
class EventCursor:
    """Track the last event id for an SSE endpoint with a replay log."""

    last_event_id: Optional[str] = None

    def advance(self, event: SseEvent) -> None:
        """Record the latest event id if present."""

        if event.id is not None:
            self.last_event_id = event.id


@dataclass(frozen=True)
class NodeSmAcceleration:
    """Acceleration advert nested within :class:`NodeCapabilities`."""

    scalar: bool
    neon_sm3: bool
    neon_sm4: bool
    policy: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeSmAcceleration":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities acceleration payload must be an object")
        try:
            scalar = bool(payload.get("scalar", False))
            neon_sm3 = bool(payload.get("neon_sm3", False))
            neon_sm4 = bool(payload.get("neon_sm4", False))
        except (TypeError, ValueError) as exc:
            raise TypeError("node capabilities acceleration booleans must be bool") from exc
        policy_value = payload.get("policy", "unknown")
        if policy_value is None:
            policy = "unknown"
        else:
            policy = str(policy_value)
        return cls(scalar=scalar, neon_sm3=neon_sm3, neon_sm4=neon_sm4, policy=policy)


@dataclass(frozen=True)
class NodeSmCapabilities:
    """SM manifest nested within :class:`NodeCapabilities`."""

    enabled: bool
    default_hash: str
    allowed_signing: List[str]
    sm2_distid_default: str
    openssl_preview: bool
    acceleration: NodeSmAcceleration

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeSmCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities `crypto.sm` payload must be an object")
        enabled = bool(payload.get("enabled", False))
        default_hash_value = payload.get("default_hash", "")
        default_hash = str(default_hash_value)
        allowed_payload = payload.get("allowed_signing", [])
        if not isinstance(allowed_payload, list):
            raise TypeError("node capabilities `allowed_signing` must be a list")
        allowed_signing = [str(item) for item in allowed_payload]
        sm2_distid_default = str(payload.get("sm2_distid_default", ""))
        openssl_preview = bool(payload.get("openssl_preview", False))
        accel_payload = payload.get("acceleration", {})
        acceleration = NodeSmAcceleration.from_payload(accel_payload)
        return cls(
            enabled=enabled,
            default_hash=default_hash,
            allowed_signing=allowed_signing,
            sm2_distid_default=sm2_distid_default,
            openssl_preview=openssl_preview,
            acceleration=acceleration,
        )


@dataclass(frozen=True)
class NodeCurveCapabilities:
    """Curve manifest nested within :class:`NodeCapabilities`."""

    registry_version: int
    allowed_curve_ids: List[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeCurveCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities `crypto.curves` payload must be an object")
        raw_version = payload.get("registry_version", 1)
        try:
            registry_version = int(raw_version)
        except (TypeError, ValueError) as exc:
            raise TypeError("node curve capability `registry_version` must be numeric") from exc
        if registry_version <= 0:
            raise TypeError("node curve capability `registry_version` must be positive")
        allowed_payload = payload.get("allowed_curve_ids", [])
        if not isinstance(allowed_payload, list):
            raise TypeError("node curve capability `allowed_curve_ids` must be a list")
        allowed_curve_ids: List[int] = []
        for entry in allowed_payload:
            try:
                allowed_curve_ids.append(int(entry))
            except (TypeError, ValueError) as exc:
                raise TypeError("node curve capability `allowed_curve_ids` entries must be numeric") from exc
        return cls(registry_version=registry_version, allowed_curve_ids=allowed_curve_ids)


@dataclass(frozen=True)
class NodeCryptoCapabilities:
    """Crypto manifest nested within :class:`NodeCapabilities`."""

    sm: NodeSmCapabilities
    curves: NodeCurveCapabilities

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeCryptoCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities `crypto` payload must be an object")
        sm_payload = payload.get("sm")
        if not isinstance(sm_payload, Mapping):
            raise TypeError("node capabilities `crypto.sm` payload must be an object")
        sm_caps = NodeSmCapabilities.from_payload(sm_payload)
        curves_payload = payload.get("curves", {})
        if not isinstance(curves_payload, Mapping):
            raise TypeError("node capabilities `crypto.curves` payload must be an object when present")
        curves_caps = NodeCurveCapabilities.from_payload(curves_payload)
        return cls(sm=sm_caps, curves=curves_caps)


@dataclass(frozen=True)
class NodeCapabilities:
    """Typed advert covering `/v1/node/capabilities`."""

    abi_version: int
    data_model_version: int
    crypto: Optional[NodeCryptoCapabilities]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "NodeCapabilities":
        if not isinstance(payload, Mapping):
            raise TypeError("node capabilities payload must be an object")
        try:
            abi_version = int(payload["abi_version"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("node capabilities missing numeric `abi_version` field") from exc
        if abi_version <= 0:
            raise TypeError("node capabilities `abi_version` must be positive")
        try:
            data_model_version = int(payload["data_model_version"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("node capabilities missing numeric `data_model_version` field") from exc
        if data_model_version <= 0:
            raise TypeError("node capabilities `data_model_version` must be positive")
        crypto_payload = payload.get("crypto")
        crypto_caps: Optional[NodeCryptoCapabilities]
        if crypto_payload is None:
            crypto_caps = None
        else:
            if not isinstance(crypto_payload, Mapping):
                raise TypeError("node capabilities `crypto` field must be an object when present")
            crypto_caps = NodeCryptoCapabilities.from_payload(crypto_payload)
        return cls(
            abi_version=abi_version,
            data_model_version=data_model_version,
            crypto=crypto_caps,
        )


@dataclass(frozen=True)
class NodeAdminSnapshot:
    """Aggregated evidence captured from `/v1/configuration`, `/v1/peers`, `/v1/time/*`, `/v1/telemetry/peers-info`, and `/v1/node/capabilities`."""

    configuration: ConfigurationSnapshot
    peers: List[PeerInfo]
    time_now: NetworkTimeSnapshot
    time_status: NetworkTimeStatus
    node_capabilities: NodeCapabilities
    telemetry_peers: Optional[List[PeerTelemetryInfo]] = None


@dataclass(frozen=True)
class PipelineDagSnapshot:
    """Deterministic DAG fingerprint snapshot in pipeline recovery payloads."""

    fingerprint_hex: str
    key_count: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PipelineDagSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline DAG snapshot must be an object")
        fingerprint = payload.get("fingerprint")
        if not isinstance(fingerprint, str):
            raise TypeError("pipeline DAG snapshot missing string `fingerprint`")
        try:
            key_count = int(payload.get("key_count", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("pipeline DAG snapshot `key_count` must be numeric") from exc
        return cls(fingerprint_hex=fingerprint, key_count=key_count)


@dataclass(frozen=True)
class PipelineTxSnapshot:
    """Access summary for a transaction in a pipeline recovery sidecar."""

    hash_hex: str
    reads: List[str]
    writes: List[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PipelineTxSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline transaction snapshot must be an object")
        hash_hex = payload.get("hash")
        if not isinstance(hash_hex, str):
            raise TypeError("pipeline transaction snapshot missing string `hash`")
        reads_value = payload.get("reads", [])
        writes_value = payload.get("writes", [])
        if not isinstance(reads_value, list) or not all(isinstance(item, str) for item in reads_value):
            raise TypeError("pipeline transaction snapshot `reads` must be a list of strings")
        if not isinstance(writes_value, list) or not all(isinstance(item, str) for item in writes_value):
            raise TypeError("pipeline transaction snapshot `writes` must be a list of strings")
        return cls(hash_hex=hash_hex, reads=list(reads_value), writes=list(writes_value))


@dataclass(frozen=True)
class PipelineRecoverySidecar:
    """Typed representation of `/v1/pipeline/recovery/{height}` responses."""

    format: str
    height: int
    dag: PipelineDagSnapshot
    txs: List[PipelineTxSnapshot]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "PipelineRecoverySidecar":
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline recovery payload must be an object")
        format_label = payload.get("format")
        if not isinstance(format_label, str):
            raise TypeError("pipeline recovery payload missing string `format`")
        try:
            height = int(payload.get("height", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("pipeline recovery `height` must be numeric") from exc
        dag_payload = payload.get("dag")
        if not isinstance(dag_payload, Mapping):
            raise TypeError("pipeline recovery payload missing object `dag`")
        txs_payload = payload.get("txs", [])
        if not isinstance(txs_payload, list):
            raise TypeError("pipeline recovery payload `txs` must be a list")
        dag = PipelineDagSnapshot.from_payload(dag_payload)
        txs = [PipelineTxSnapshot.from_payload(item) for item in txs_payload]
        return cls(format=format_label, height=height, dag=dag, txs=txs)


@dataclass(frozen=True)
class AccountAsset:
    """Account asset entry returned by account asset listings."""

    asset_id: str
    quantity: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountAsset":
        if not isinstance(payload, Mapping):
            raise TypeError("account asset entry must be an object")
        asset_id = payload.get("asset_id") or payload.get("asset")
        quantity = payload.get("quantity")
        if not isinstance(asset_id, str):
            raise TypeError("account asset entry missing string `asset_id` field")
        return cls(
            asset_id=asset_id,
            quantity=_canonical_quantity_text(quantity, "account asset quantity"),
        )


@dataclass(frozen=True)
class AccountAssetsPage:
    """Paginated account asset list."""

    items: List[AccountAsset]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountAssetsPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account assets response must be an object")
        items_raw = payload.get("items", [])
        if not isinstance(items_raw, list):
            raise TypeError("account assets response `items` must be a list")
        items = [AccountAsset.from_payload(entry) for entry in items_raw]
        return cls(
            items=items,
            **_page_metadata(payload, len(items), "account assets response"),
        )


@dataclass(frozen=True)
class AccountTransaction:
    """Projection of a transaction returned by account transaction listings."""

    entrypoint_hash: str
    result_ok: bool
    authority: Optional[str]
    timestamp_ms: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountTransaction":
        if not isinstance(payload, Mapping):
            raise TypeError("account transaction entry must be an object")
        entrypoint_hash = payload.get("entrypoint_hash")
        if not isinstance(entrypoint_hash, str):
            raise TypeError("account transaction entry missing string `entrypoint_hash` field")
        result_ok = payload.get("result_ok")
        if not isinstance(result_ok, bool):
            raise TypeError("account transaction entry missing bool `result_ok` field")
        authority = payload.get("authority")
        if authority is not None and not isinstance(authority, str):
            raise TypeError("account transaction `authority` must be a string when provided")
        timestamp_value = payload.get("timestamp_ms")
        if timestamp_value is None:
            timestamp_ms: Optional[int] = None
        else:
            try:
                timestamp_ms = int(timestamp_value)
            except (TypeError, ValueError) as exc:
                raise TypeError("account transaction `timestamp_ms` must be numeric") from exc
        return cls(
            entrypoint_hash=entrypoint_hash,
            result_ok=result_ok,
            authority=authority,
            timestamp_ms=timestamp_ms,
        )


@dataclass(frozen=True)
class AccountTransactionsPage:
    """Paginated account transaction list."""

    items: List[AccountTransaction]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountTransactionsPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account transactions response must be an object")
        items_raw = payload.get("items", [])
        if not isinstance(items_raw, list):
            raise TypeError("account transactions response `items` must be a list")
        items = [AccountTransaction.from_payload(entry) for entry in items_raw]
        return cls(
            items=items,
            **_page_metadata(payload, len(items), "account transactions response"),
        )


@dataclass(frozen=True)
class AccountRecord:
    """Account entry returned by account queries."""

    id: str
    signatories: List[str]
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("account record must be an object")
        account_id = payload.get("id")
        if not isinstance(account_id, str):
            raise TypeError("account record missing string `id` field")
        signatories_payload = payload.get("signatories", [])
        if signatories_payload is None:
            signatories_list: List[str] = []
        elif isinstance(signatories_payload, list):
            if not all(isinstance(item, str) for item in signatories_payload):
                raise TypeError("account record `signatories` must be a list of strings")
            signatories_list = list(signatories_payload)
        else:
            raise TypeError("account record `signatories` must be a list")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata_dict: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata_dict = dict(metadata_payload)
        else:
            raise TypeError("account record `metadata` must be an object when present")
        return cls(
            id=account_id,
            signatories=signatories_list,
            metadata=metadata_dict,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class AccountListPage:
    """Paginated account query result."""

    items: List[AccountRecord]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("account query `items` must be a list")
        items = [AccountRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, **_page_metadata(payload, len(items), "account query"))


@dataclass(frozen=True)
class DomainRecord:
    """Domain projection returned by domain listings and queries."""

    id: str
    owned_by: Optional[str]
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "DomainRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("domain record must be an object")
        domain_id = payload.get("id")
        if not isinstance(domain_id, str):
            raise TypeError("domain record missing string `id` field")
        owned_by = payload.get("owned_by")
        if owned_by is not None and not isinstance(owned_by, str):
            raise TypeError("domain record `owned_by` must be a string when provided")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata = dict(metadata_payload)
        else:
            raise TypeError("domain record `metadata` must be an object when present")
        return cls(id=domain_id, owned_by=owned_by, metadata=metadata, raw=dict(payload))


@dataclass(frozen=True)
class DomainListPage:
    """Paginated domain query result."""

    items: List[DomainRecord]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "DomainListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("domain query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("domain query `items` must be a list")
        items = [DomainRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, **_page_metadata(payload, len(items), "domain query"))


@dataclass(frozen=True)
class AssetDefinitionRecord:
    """Asset definition projection returned by asset definition queries."""

    id: str
    metadata: Dict[str, Any]
    owned_by: Optional[str]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetDefinitionRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("asset definition record must be an object")
        definition_id = payload.get("id")
        if not isinstance(definition_id, str):
            raise TypeError("asset definition record missing string `id` field")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata = dict(metadata_payload)
        else:
            raise TypeError("asset definition record `metadata` must be an object when present")
        owned_by = payload.get("owned_by")
        if owned_by is not None and not isinstance(owned_by, str):
            raise TypeError("asset definition record `owned_by` must be a string when provided")
        return cls(id=definition_id, metadata=metadata, owned_by=owned_by, raw=dict(payload))


@dataclass(frozen=True)
class AssetDefinitionListPage:
    """Paginated asset definition query result."""

    items: List[AssetDefinitionRecord]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetDefinitionListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("asset definition query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("asset definition query `items` must be a list")
        items = [AssetDefinitionRecord.from_payload(entry) for entry in items_payload]
        return cls(
            items=items,
            **_page_metadata(payload, len(items), "asset definition query"),
        )


@dataclass(frozen=True)
class AssetHolderRecord:
    """Asset holder projection returned by asset holder queries."""

    account_id: str
    quantity: str
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetHolderRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("asset holder record must be an object")
        account_id = payload.get("account_id")
        quantity = payload.get("quantity")
        if not isinstance(account_id, str):
            raise TypeError("asset holder record missing string `account_id` field")
        canonical_quantity = _canonical_quantity_text(
            quantity,
            "asset holder quantity",
        )
        raw = dict(payload)
        raw["quantity"] = canonical_quantity
        return cls(account_id=account_id, quantity=canonical_quantity, raw=raw)


@dataclass(frozen=True)
class AssetHolderListPage:
    """Paginated asset holder query result."""

    items: List[AssetHolderRecord]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AssetHolderListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("asset holder query payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("asset holder query `items` must be a list")
        items = [AssetHolderRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, **_page_metadata(payload, len(items), "asset holder query"))


@dataclass(frozen=True)
class RwaListItem:
    """Chain-state RWA lot entry returned by `/v1/rwas` and `/v1/rwas/query`."""

    id: str
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RwaListItem":
        if not isinstance(payload, Mapping):
            raise TypeError("RWA list item must be an object")
        identifier = payload.get("id")
        if not isinstance(identifier, str) or not identifier.strip():
            raise TypeError("RWA list item missing string `id` field")
        raw = dict(payload)
        for quantity_field in ("quantity", "held_quantity"):
            if quantity_field in raw:
                raw[quantity_field] = _canonical_quantity_text(
                    raw[quantity_field],
                    f"RWA list item {quantity_field}",
                )
        parents = raw.get("parents")
        if parents is not None:
            if not isinstance(parents, list):
                raise TypeError("RWA list item parents must be a list")
            canonical_parents: List[Any] = []
            for index, parent in enumerate(parents):
                if not isinstance(parent, Mapping):
                    raise TypeError(f"RWA list item parents[{index}] must be an object")
                canonical_parent = dict(parent)
                if "quantity" in canonical_parent:
                    canonical_parent["quantity"] = _canonical_quantity_text(
                        canonical_parent["quantity"],
                        f"RWA list item parents[{index}].quantity",
                    )
                canonical_parents.append(canonical_parent)
            raw["parents"] = canonical_parents
        return cls(id=identifier.strip(), raw=raw)


@dataclass(frozen=True)
class RwaListPage:
    """Paginated chain-state RWA lot list returned by `/v1/rwas` and `/v1/rwas/query`."""

    items: List[RwaListItem]
    total: Optional[int]
    has_more: bool = False
    count_mode: str = "exact"
    indexed_height: Optional[int] = None
    indexed_block_hash: Optional[str] = None
    query_source: Optional[str] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RwaListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("RWA list payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("RWA list `items` must be a list")
        items = [RwaListItem.from_payload(entry) for entry in items_payload]
        return cls(items=items, **_page_metadata(payload, len(items), "RWA list"))


@dataclass(frozen=True)
class AccountPermissionRecord:
    """Account permission entry returned by `GET /v1/accounts/{account_id}/permissions`."""

    name: str
    payload: Any
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountPermissionRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("account permission record must be an object")
        name = payload.get("name")
        if not isinstance(name, str):
            raise TypeError("account permission record missing string `name` field")
        permission_payload = _json_safe_value(payload.get("payload"))
        return cls(name=name, payload=permission_payload, raw=dict(payload))


@dataclass(frozen=True)
class AccountPermissionListPage:
    """Paginated account permission result."""

    items: List[AccountPermissionRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "AccountPermissionListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("account permission payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("account permission `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("account permission `total` must be numeric") from exc
        items = [AccountPermissionRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


# ---------------------------------------------------------------------------
# UAID portfolio & Space Directory helpers
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class UaidPortfolioAsset:
    asset_id: str
    asset_definition_id: str
    quantity: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioAsset":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio asset must be an object")
        asset_id = payload.get("asset_id")
        definition_id = payload.get("asset_definition_id")
        quantity = payload.get("quantity")
        if not isinstance(asset_id, str) or not asset_id:
            raise TypeError("portfolio asset missing `asset_id` string")
        if not isinstance(definition_id, str) or not definition_id:
            raise TypeError("portfolio asset missing `asset_definition_id` string")
        canonical_quantity = _canonical_quantity_text(
            quantity,
            "portfolio asset quantity",
        )
        return cls(
            asset_id=asset_id,
            asset_definition_id=definition_id,
            quantity=canonical_quantity,
        )


@dataclass(frozen=True)
class UaidPortfolioAccount:
    account_id: str
    label: Optional[str]
    assets: List[UaidPortfolioAsset]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioAccount":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio account must be an object")
        account_id = payload.get("account_id")
        label = payload.get("label")
        if not isinstance(account_id, str) or not account_id:
            raise TypeError("portfolio account missing `account_id` string")
        if label is not None and not isinstance(label, str):
            raise TypeError("portfolio account `label` must be a string when present")
        assets_payload = payload.get("assets", [])
        if not isinstance(assets_payload, list):
            raise TypeError("portfolio account `assets` must be a list")
        assets = [UaidPortfolioAsset.from_payload(item) for item in assets_payload]
        return cls(account_id=account_id, label=label, assets=assets)


@dataclass(frozen=True)
class UaidPortfolioDataspace:
    dataspace_id: int
    dataspace_alias: Optional[str]
    accounts: List[UaidPortfolioAccount]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioDataspace":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio dataspace must be an object")
        dataspace_id = _coerce_int(payload.get("dataspace_id"), "dataspace_id", allow_zero=True)
        if dataspace_id is None:
            raise TypeError("portfolio dataspace missing `dataspace_id` integer")
        alias = payload.get("dataspace_alias")
        if alias is not None and not isinstance(alias, str):
            raise TypeError("portfolio dataspace `dataspace_alias` must be a string when present")
        accounts_payload = payload.get("accounts", [])
        if not isinstance(accounts_payload, list):
            raise TypeError("portfolio dataspace `accounts` must be a list")
        accounts = [UaidPortfolioAccount.from_payload(item) for item in accounts_payload]
        return cls(dataspace_id=dataspace_id, dataspace_alias=alias, accounts=accounts)


@dataclass(frozen=True)
class UaidPortfolioSnapshot:
    uaid: str
    total_accounts: int
    total_positions: int
    dataspaces: List[UaidPortfolioDataspace]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidPortfolioSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("portfolio payload must be an object")
        uaid = payload.get("uaid")
        if not isinstance(uaid, str) or not uaid:
            raise TypeError("portfolio payload missing `uaid` string")
        totals = payload.get("totals", {})
        if not isinstance(totals, Mapping):
            raise TypeError("portfolio payload `totals` must be an object")
        accounts = _coerce_int(totals.get("accounts"), "totals.accounts", allow_zero=True)
        positions = _coerce_int(totals.get("positions"), "totals.positions", allow_zero=True)
        if accounts is None or positions is None:
            raise TypeError("portfolio payload missing totals")
        dataspaces_payload = payload.get("dataspaces", [])
        if not isinstance(dataspaces_payload, list):
            raise TypeError("portfolio payload `dataspaces` must be a list")
        dataspaces = [UaidPortfolioDataspace.from_payload(item) for item in dataspaces_payload]
        return cls(
            uaid=uaid,
            total_accounts=accounts,
            total_positions=positions,
            dataspaces=dataspaces,
        )


@dataclass(frozen=True)
class UaidBindingsSlice:
    dataspace_id: int
    dataspace_alias: Optional[str]
    accounts: List[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidBindingsSlice":
        if not isinstance(payload, Mapping):
            raise TypeError("bindings slice must be an object")
        dataspace_id = _coerce_int(payload.get("dataspace_id"), "dataspace_id", allow_zero=True)
        if dataspace_id is None:
            raise TypeError("bindings slice missing `dataspace_id` integer")
        alias = payload.get("dataspace_alias")
        if alias is not None and not isinstance(alias, str):
            raise TypeError("bindings slice `dataspace_alias` must be a string when present")
        accounts_value = payload.get("accounts", [])
        if not isinstance(accounts_value, list):
            raise TypeError("bindings slice `accounts` must be a list")
        accounts: List[str] = []
        for index, literal in enumerate(accounts_value):
            if not isinstance(literal, str) or not literal:
                raise TypeError(f"bindings slice account[{index}] must be a non-empty string")
            accounts.append(literal)
        return cls(dataspace_id=dataspace_id, dataspace_alias=alias, accounts=accounts)


@dataclass(frozen=True)
class UaidBindingsSnapshot:
    uaid: str
    dataspaces: List[UaidBindingsSlice]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "UaidBindingsSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("bindings payload must be an object")
        uaid = payload.get("uaid")
        if not isinstance(uaid, str) or not uaid:
            raise TypeError("bindings payload missing `uaid` string")
        dataspaces_payload = payload.get("dataspaces", [])
        if not isinstance(dataspaces_payload, list):
            raise TypeError("bindings payload `dataspaces` must be a list")
        dataspaces = [UaidBindingsSlice.from_payload(item) for item in dataspaces_payload]
        return cls(uaid=uaid, dataspaces=dataspaces)


@dataclass(frozen=True)
class SpaceDirectoryManifestLifecycle:
    activated_epoch: Optional[int]
    expired_epoch: Optional[int]
    revocation_epoch: Optional[int]
    revocation_reason: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SpaceDirectoryManifestLifecycle":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest lifecycle must be an object")
        activated = _coerce_int(payload.get("activated_epoch"), "lifecycle.activated_epoch", allow_zero=True)
        expired = _coerce_int(payload.get("expired_epoch"), "lifecycle.expired_epoch", allow_zero=True)
        revocation = payload.get("revocation")
        revocation_epoch: Optional[int] = None
        revocation_reason: Optional[str] = None
        if revocation is not None:
            if not isinstance(revocation, Mapping):
                raise TypeError("lifecycle.revocation must be an object when present")
            revocation_epoch = _coerce_int(revocation.get("epoch"), "lifecycle.revocation.epoch", allow_zero=True)
            reason_value = revocation.get("reason")
            if reason_value is not None and not isinstance(reason_value, str):
                raise TypeError("lifecycle.revocation.reason must be a string when present")
            revocation_reason = reason_value
        return cls(
            activated_epoch=activated,
            expired_epoch=expired,
            revocation_epoch=revocation_epoch,
            revocation_reason=revocation_reason,
        )


@dataclass(frozen=True)
class SpaceDirectoryManifestRecord:
    dataspace_id: int
    dataspace_alias: Optional[str]
    manifest_hash: str
    status: str
    lifecycle: SpaceDirectoryManifestLifecycle
    accounts: List[str]
    manifest: Mapping[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SpaceDirectoryManifestRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest record must be an object")
        dataspace_id = _coerce_int(payload.get("dataspace_id"), "dataspace_id", allow_zero=True)
        if dataspace_id is None:
            raise TypeError("manifest record missing `dataspace_id` integer")
        alias = payload.get("dataspace_alias")
        if alias is not None and not isinstance(alias, str):
            raise TypeError("manifest record `dataspace_alias` must be a string when present")
        manifest_hash = payload.get("manifest_hash")
        if not isinstance(manifest_hash, str) or not manifest_hash:
            raise TypeError("manifest record missing `manifest_hash` string")
        status = payload.get("status")
        if not isinstance(status, str) or not status:
            raise TypeError("manifest record missing `status` string")
        lifecycle_payload = payload.get("lifecycle", {})
        lifecycle = SpaceDirectoryManifestLifecycle.from_payload(lifecycle_payload)
        accounts_value = payload.get("accounts", [])
        if not isinstance(accounts_value, list):
            raise TypeError("manifest record `accounts` must be a list")
        accounts: List[str] = []
        for index, literal in enumerate(accounts_value):
            if not isinstance(literal, str) or not literal:
                raise TypeError(f"manifest record account[{index}] must be a non-empty string")
            accounts.append(literal)
        manifest_payload = payload.get("manifest")
        if not isinstance(manifest_payload, Mapping):
            raise TypeError("manifest record `manifest` must be an object")
        return cls(
            dataspace_id=dataspace_id,
            dataspace_alias=alias,
            manifest_hash=manifest_hash,
            status=status,
            lifecycle=lifecycle,
            accounts=accounts,
            manifest=dict(manifest_payload),
        )


@dataclass(frozen=True)
class SpaceDirectoryManifestList:
    uaid: str
    total: int
    manifests: List[SpaceDirectoryManifestRecord]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SpaceDirectoryManifestList":
        if not isinstance(payload, Mapping):
            raise TypeError("manifest list payload must be an object")
        uaid = payload.get("uaid")
        if not isinstance(uaid, str) or not uaid:
            raise TypeError("manifest list payload missing `uaid` string")
        total = payload.get("total")
        if total is None:
            raise TypeError("manifest list missing numeric `total` field")
        try:
            total_int = int(total)
        except (TypeError, ValueError) as exc:
            raise TypeError("manifest list `total` must be numeric") from exc
        manifests_payload = payload.get("manifests", [])
        if not isinstance(manifests_payload, list):
            raise TypeError("manifest list `manifests` must be a list")
        manifests = [SpaceDirectoryManifestRecord.from_payload(item) for item in manifests_payload]
        return cls(uaid=uaid, total=total_int, manifests=manifests)


@dataclass(frozen=True)
class TriggerRecord:
    """Trigger definition returned by trigger listing/query endpoints."""

    id: str
    action: Dict[str, Any]
    metadata: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("trigger record must be an object")
        trigger_id = payload.get("id")
        if not isinstance(trigger_id, str):
            raise TypeError("trigger record missing string `id` field")
        action_payload = payload.get("action", {})
        if not isinstance(action_payload, Mapping):
            raise TypeError("trigger record `action` must be an object")
        metadata_payload = payload.get("metadata", {})
        if metadata_payload is None:
            metadata: Dict[str, Any] = {}
        elif isinstance(metadata_payload, Mapping):
            metadata = dict(metadata_payload)
        else:
            raise TypeError("trigger record `metadata` must be an object when present")
        return cls(
            id=trigger_id,
            action=dict(action_payload),
            metadata=metadata,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class TriggerListPage:
    """Paginated trigger listing."""

    items: List[TriggerRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("trigger list payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("trigger list `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("trigger list `total` must be numeric") from exc
        items = [TriggerRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class TriggerMutationResponse:
    """Governance draft emitted when triggers are registered or deleted."""

    ok: bool
    tx_instructions: List[RuntimeInstruction]
    trigger_id: Optional[str]
    proposal_id: Optional[str]
    accepted: Optional[bool]
    message: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "TriggerMutationResponse":
        if not isinstance(payload, Mapping):
            raise TypeError("trigger mutation response must be an object")
        ok_value = payload.get("ok")
        if not isinstance(ok_value, bool):
            raise TypeError("trigger mutation response missing boolean `ok` field")
        trigger_id_field = payload.get("trigger_id")
        if trigger_id_field is None:
            trigger_id: Optional[str] = None
        else:
            trigger_id = _require_non_empty_string(
                trigger_id_field, "trigger mutation response `trigger_id`"
            )
        proposal_field = payload.get("proposal_id")
        if proposal_field is None:
            proposal_id: Optional[str] = None
        else:
            proposal_id = _require_non_empty_string(
                proposal_field, "trigger mutation response `proposal_id`"
            )
        instructions_payload = payload.get("tx_instructions")
        if instructions_payload is None:
            instructions_payload = []
        if not isinstance(instructions_payload, list):
            raise TypeError("trigger mutation response `tx_instructions` must be a list")
        instructions = [RuntimeInstruction.from_payload(entry) for entry in instructions_payload]
        accepted_field = payload.get("accepted")
        accepted = None
        if accepted_field is not None:
            if not isinstance(accepted_field, bool):
                raise TypeError("trigger mutation response `accepted` must be boolean when present")
            accepted = accepted_field
        message_field = payload.get("message")
        if message_field is None:
            message_field = payload.get("error")
        if message_field is None:
            message_field = payload.get("reason")
        message = None
        if message_field is not None:
            message = str(message_field).strip()
            if not message:
                message = None
        return cls(
            ok=ok_value,
            tx_instructions=instructions,
            trigger_id=trigger_id,
            proposal_id=proposal_id,
            accepted=accepted,
            message=message,
        )


@dataclass(frozen=True)
class SumeragiAvailabilityCollector:
    """Collector vote ingestion snapshot exposed via `/v1/sumeragi/telemetry`."""

    collector_idx: int
    peer_id: str
    votes_ingested: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiAvailabilityCollector":
        if not isinstance(payload, Mapping):
            raise TypeError("availability collector entry must be an object")
        try:
            collector_idx = int(payload["collector_idx"])
            votes_ingested = int(payload["votes_ingested"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("availability collector entry must expose numeric counters") from exc
        peer_id = payload.get("peer_id")
        if not isinstance(peer_id, str):
            raise TypeError("availability collector entry missing string `peer_id` field")
        return cls(
            collector_idx=collector_idx,
            peer_id=peer_id,
            votes_ingested=votes_ingested,
        )


@dataclass(frozen=True)
class SumeragiAvailabilitySnapshot:
    """Aggregated availability vote ingestion statistics."""

    total_votes_ingested: int
    collectors: List[SumeragiAvailabilityCollector]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiAvailabilitySnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("availability snapshot must be an object")
        try:
            total_votes_ingested = int(payload["total_votes_ingested"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("availability snapshot missing numeric `total_votes_ingested`") from exc
        raw_collectors = payload.get("collectors", [])
        if raw_collectors is None:
            collectors_payload: List[Any] = []
        elif isinstance(raw_collectors, list):
            collectors_payload = raw_collectors
        else:
            raise TypeError("availability snapshot `collectors` must be a list when present")
        collectors = [
            SumeragiAvailabilityCollector.from_payload(entry) for entry in collectors_payload
        ]
        return cls(total_votes_ingested=total_votes_ingested, collectors=collectors)


@dataclass(frozen=True)
class SumeragiQcLatencyEntry:
    """Latency moving average for a consensus stage."""

    kind: str
    last_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiQcLatencyEntry":
        if not isinstance(payload, Mapping):
            raise TypeError("QC latency entry must be an object")
        kind = payload.get("kind")
        if not isinstance(kind, str):
            raise TypeError("QC latency entry missing string `kind` field")
        try:
            last_ms = int(payload["last_ms"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("QC latency entry missing numeric `last_ms` field") from exc
        return cls(kind=kind, last_ms=last_ms)


@dataclass(frozen=True)
class SumeragiRbcBacklog:
    """Backpressure snapshot for RBC chunk ingestion."""

    pending_sessions: int
    total_missing_chunks: int
    max_missing_chunks: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcBacklog":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC backlog snapshot must be an object")
        try:
            pending_sessions = int(payload.get("pending_sessions", 0))
            total_missing_chunks = int(payload.get("total_missing_chunks", 0))
            max_missing_chunks = int(payload.get("max_missing_chunks", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC backlog counters must be numeric") from exc
        return cls(
            pending_sessions=pending_sessions,
            total_missing_chunks=total_missing_chunks,
            max_missing_chunks=max_missing_chunks,
        )


@dataclass(frozen=True)
class SumeragiRbcEviction:
    """Evicted RBC payload metadata retained for status telemetry consumers."""

    block_hash: Optional[str]
    height: Optional[int]
    view: Optional[int]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcEviction":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC eviction entry must be an object")
        block_hash = payload.get("block_hash")
        if block_hash is not None and not isinstance(block_hash, str):
            raise TypeError("RBC eviction `block_hash` must be a string when present")
        height_val = payload.get("height")
        view_val = payload.get("view")
        try:
            height = None if height_val is None else int(height_val)
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC eviction `height` must be numeric when present") from exc
        try:
            view = None if view_val is None else int(view_val)
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC eviction `view` must be numeric when present") from exc
        return cls(block_hash=block_hash, height=height, view=view)


@dataclass(frozen=True)
class SumeragiRbcStoreStatus:
    """RBC on-disk store health retained as typed status telemetry."""

    sessions: int
    bytes: int
    pressure_level: int
    backpressure_deferrals_total: int
    persist_drops_total: int
    evictions_total: int
    recent_evictions: List[SumeragiRbcEviction]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiRbcStoreStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("RBC store payload must be an object")
        try:
            sessions = int(payload.get("sessions", 0))
            bytes_used = int(payload.get("bytes", 0))
            pressure_level = int(payload.get("pressure_level", 0))
            backpressure_deferrals_total = int(payload.get("backpressure_deferrals_total", 0))
            persist_drops_total = int(payload.get("persist_drops_total", 0))
            evictions_total = int(payload.get("evictions_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("RBC store counters must be numeric") from exc
        raw_evictions = payload.get("recent_evictions", [])
        if raw_evictions is None:
            evictions_payload: List[Any] = []
        elif isinstance(raw_evictions, list):
            evictions_payload = raw_evictions
        else:
            raise TypeError("RBC store `recent_evictions` must be a list when present")
        recent_evictions = [
            SumeragiRbcEviction.from_payload(entry) for entry in evictions_payload
        ]
        return cls(
            sessions=sessions,
            bytes=bytes_used,
            pressure_level=pressure_level,
            backpressure_deferrals_total=backpressure_deferrals_total,
            persist_drops_total=persist_drops_total,
            evictions_total=evictions_total,
            recent_evictions=recent_evictions,
        )


@dataclass(frozen=True)
class SumeragiVrfLateReveal:
    """Late reveal entry surfaced in the telemetry VRF summary."""

    signer: str
    noted_at_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiVrfLateReveal":
        if not isinstance(payload, Mapping):
            raise TypeError("VRF late reveal entry must be an object")
        signer = payload.get("signer")
        if not isinstance(signer, str):
            raise TypeError("VRF late reveal entry missing string `signer` field")
        try:
            noted_at_height = int(payload["noted_at_height"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("VRF late reveal entry missing numeric `noted_at_height`") from exc
        return cls(signer=signer, noted_at_height=noted_at_height)


@dataclass(frozen=True)
class SumeragiVrfSummary:
    """VRF epoch snapshot embedded in `/v1/sumeragi/telemetry`."""

    found: bool
    epoch: int
    finalized: bool
    seed_hex: Optional[str]
    epoch_length: int
    commit_deadline_offset: int
    reveal_deadline_offset: int
    roster_len: int
    updated_at_height: int
    participants_total: int
    commitments_total: int
    reveals_total: int
    late_reveals_total: int
    committed_no_reveal: List[int]
    no_participation: List[int]
    late_reveals: List[SumeragiVrfLateReveal]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiVrfSummary":
        if not isinstance(payload, Mapping):
            raise TypeError("VRF summary must be an object")
        found = payload.get("found")
        if not isinstance(found, bool):
            raise TypeError("VRF summary missing bool `found` field")
        try:
            epoch = int(payload.get("epoch", 0))
            epoch_length = int(payload.get("epoch_length", 0))
            commit_deadline_offset = int(payload.get("commit_deadline_offset", 0))
            reveal_deadline_offset = int(payload.get("reveal_deadline_offset", 0))
            roster_len = int(payload.get("roster_len", 0))
            updated_at_height = int(payload.get("updated_at_height", 0))
            participants_total = int(payload.get("participants_total", 0))
            commitments_total = int(payload.get("commitments_total", 0))
            reveals_total = int(payload.get("reveals_total", 0))
            late_reveals_total = int(payload.get("late_reveals_total", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("VRF summary numeric fields must contain integers") from exc
        finalized = payload.get("finalized")
        if not isinstance(finalized, bool):
            raise TypeError("VRF summary missing bool `finalized` field")
        seed_value = payload.get("seed_hex")
        if seed_value is not None and seed_value is not False and not isinstance(seed_value, str):
            raise TypeError("VRF summary `seed_hex` must be a string when provided")
        seed_hex: Optional[str]
        if isinstance(seed_value, str):
            seed_hex = seed_value
        else:
            seed_hex = None
        committed_payload = payload.get("committed_no_reveal", [])
        no_participation_payload = payload.get("no_participation", [])
        if committed_payload is None:
            committed_raw: List[Any] = []
        elif isinstance(committed_payload, list):
            committed_raw = committed_payload
        else:
            raise TypeError("VRF summary `committed_no_reveal` must be a list when present")
        if no_participation_payload is None:
            no_participation_raw: List[Any] = []
        elif isinstance(no_participation_payload, list):
            no_participation_raw = no_participation_payload
        else:
            raise TypeError("VRF summary `no_participation` must be a list when present")
        try:
            committed_no_reveal = [int(item) for item in committed_raw]
            no_participation = [int(item) for item in no_participation_raw]
        except (TypeError, ValueError) as exc:
            raise TypeError("VRF summary participant arrays must be numeric") from exc
        late_reveals_payload = payload.get("late_reveals", [])
        if late_reveals_payload is None:
            late_reveals_raw: List[Any] = []
        elif isinstance(late_reveals_payload, list):
            late_reveals_raw = late_reveals_payload
        else:
            raise TypeError("VRF summary `late_reveals` must be a list when present")
        late_reveals = [
            SumeragiVrfLateReveal.from_payload(entry) for entry in late_reveals_raw
        ]
        return cls(
            found=found,
            epoch=epoch,
            finalized=finalized,
            seed_hex=seed_hex,
            epoch_length=epoch_length,
            commit_deadline_offset=commit_deadline_offset,
            reveal_deadline_offset=reveal_deadline_offset,
            roster_len=roster_len,
            updated_at_height=updated_at_height,
            participants_total=participants_total,
            commitments_total=commitments_total,
            reveals_total=reveals_total,
            late_reveals_total=late_reveals_total,
            committed_no_reveal=committed_no_reveal,
            no_participation=no_participation,
            late_reveals=late_reveals,
        )


@dataclass(frozen=True)
class SumeragiTelemetrySnapshot:
    """Typed payload for `/v1/sumeragi/telemetry`."""

    availability: SumeragiAvailabilitySnapshot
    qc_latency_ms: List[SumeragiQcLatencyEntry]
    rbc_backlog: SumeragiRbcBacklog
    vrf: SumeragiVrfSummary

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiTelemetrySnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry payload must be an object")
        availability_payload = payload.get("availability")
        if not isinstance(availability_payload, Mapping):
            raise TypeError("telemetry payload missing object `availability` field")
        rbc_payload = payload.get("rbc_backlog")
        if not isinstance(rbc_payload, Mapping):
            raise TypeError("telemetry payload missing object `rbc_backlog` field")
        qc_payload = payload.get("qc_latency_ms", [])
        if qc_payload is None:
            qc_entries_raw: List[Any] = []
        elif isinstance(qc_payload, list):
            qc_entries_raw = qc_payload
        else:
            raise TypeError("telemetry payload `qc_latency_ms` must be a list when present")
        vrf_payload = payload.get("vrf")
        if not isinstance(vrf_payload, Mapping):
            raise TypeError("telemetry payload missing object `vrf` field")
        availability = SumeragiAvailabilitySnapshot.from_payload(availability_payload)
        qc_entries = [SumeragiQcLatencyEntry.from_payload(entry) for entry in qc_entries_raw]
        rbc_backlog = SumeragiRbcBacklog.from_payload(rbc_payload)
        vrf = SumeragiVrfSummary.from_payload(vrf_payload)
        return cls(
            availability=availability,
            qc_latency_ms=qc_entries,
            rbc_backlog=rbc_backlog,
            vrf=vrf,
        )


@dataclass(frozen=True)
class SumeragiEvidenceRecord:
    """Evidence record returned by `/v1/sumeragi/evidence`."""

    kind: str
    recorded_height: int
    recorded_view: int
    recorded_ms: int
    data: Dict[str, Any]
    raw: Dict[str, Any]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiEvidenceRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi evidence record must be an object")
        kind = payload.get("kind")
        if not isinstance(kind, str):
            raise TypeError("sumeragi evidence record missing string `kind` field")
        height_raw = payload.get("recorded_height")
        view_raw = payload.get("recorded_view")
        recorded_ms_raw = payload.get("recorded_ms")
        if height_raw is None or view_raw is None or recorded_ms_raw is None:
            raise TypeError("sumeragi evidence record missing timing fields")
        try:
            recorded_height = int(height_raw)
            recorded_view = int(view_raw)
            recorded_ms = int(recorded_ms_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi evidence timing fields must be numeric") from exc
        extras = {
            key: value
            for key, value in payload.items()
            if key not in {"kind", "recorded_height", "recorded_view", "recorded_ms"}
        }
        return cls(
            kind=kind,
            recorded_height=recorded_height,
            recorded_view=recorded_view,
            recorded_ms=recorded_ms,
            data=extras,
            raw=dict(payload),
        )


@dataclass(frozen=True)
class SumeragiEvidenceListPage:
    """Paginated evidence snapshot from `/v1/sumeragi/evidence`."""

    items: List[SumeragiEvidenceRecord]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiEvidenceListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi evidence payload must be an object")
        items_payload = payload.get("items", [])
        if items_payload is None:
            items_payload = []
        if not isinstance(items_payload, list):
            raise TypeError("sumeragi evidence `items` must be a list")
        try:
            total = int(payload.get("total", len(items_payload)))
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi evidence `total` must be numeric") from exc
        items = [SumeragiEvidenceRecord.from_payload(entry) for entry in items_payload]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class SumeragiQcSummary:
    """Quorum certificate tuple {height, view, subject_block_hash}."""

    height: int
    view: int
    subject_block_hash: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiQcSummary":
        if not isinstance(payload, Mapping):
            raise TypeError("QC summary must be an object")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("QC summary `height` and `view` must be numeric") from exc
        hash_value = payload.get("subject_block_hash")
        if hash_value is None:
            subject_hash: Optional[str] = None
        elif isinstance(hash_value, str):
            subject_hash = hash_value
        else:
            raise TypeError("QC summary `subject_block_hash` must be a string when present")
        return cls(height=height, view=view, subject_block_hash=subject_hash)


@dataclass(frozen=True)
class SumeragiCommitQc:
    """Commit QC record returned by `/v1/sumeragi/commit-qcs/{block_hash}`."""

    phase: str
    parent_state_root: str
    post_state_root: str
    height: int
    view: int
    epoch: int
    mode_tag: str
    validator_set_hash: str
    validator_set_hash_version: int
    validator_set: List[str]
    signers_bitmap: str
    bls_aggregate_signature: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiCommitQc":
        if not isinstance(payload, Mapping):
            raise TypeError("commit_qc payload must be an object")
        phase = payload.get("phase")
        mode_tag = payload.get("mode_tag")
        if not isinstance(phase, str):
            raise TypeError("commit_qc `phase` must be a string")
        if not isinstance(mode_tag, str):
            raise TypeError("commit_qc `mode_tag` must be a string")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
            epoch = int(payload.get("epoch", 0))
            validator_set_hash_version = int(payload.get("validator_set_hash_version", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("commit_qc numeric fields must be numeric") from exc
        parent_state_root = _normalize_hex_string(
            payload.get("parent_state_root"),
            "commit_qc.parent_state_root",
            expected_length=64,
        )
        post_state_root = _normalize_hex_string(
            payload.get("post_state_root"),
            "commit_qc.post_state_root",
            expected_length=64,
        )
        validator_set_hash = _normalize_hex_string(
            payload.get("validator_set_hash"),
            "commit_qc.validator_set_hash",
            expected_length=64,
        )
        validator_set_value = payload.get("validator_set")
        if not isinstance(validator_set_value, list):
            raise TypeError("commit_qc `validator_set` must be a list")
        validator_set: List[str] = []
        for index, entry in enumerate(validator_set_value):
            if not isinstance(entry, str):
                raise TypeError(f"commit_qc validator_set entry {index} must be a string")
            validator_set.append(entry)
        signers_bitmap = _normalize_hex_string(
            payload.get("signers_bitmap"),
            "commit_qc.signers_bitmap",
        )
        bls_aggregate_signature = _normalize_hex_string(
            payload.get("bls_aggregate_signature"),
            "commit_qc.bls_aggregate_signature",
        )
        return cls(
            phase=phase,
            parent_state_root=parent_state_root,
            post_state_root=post_state_root,
            height=height,
            view=view,
            epoch=epoch,
            mode_tag=mode_tag,
            validator_set_hash=validator_set_hash,
            validator_set_hash_version=validator_set_hash_version,
            validator_set=validator_set,
            signers_bitmap=signers_bitmap,
            bls_aggregate_signature=bls_aggregate_signature,
        )


@dataclass(frozen=True)
class SumeragiCommitQcRecord:
    """Commit QC response wrapper returned by `/v1/sumeragi/commit-qcs/{block_hash}`."""

    subject_block_hash: str
    commit_qc: Optional[SumeragiCommitQc]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiCommitQcRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("commit_qc response must be an object")
        subject_block_hash = _normalize_hex_string(
            payload.get("subject_block_hash"),
            "commit_qc.subject_block_hash",
            expected_length=64,
        )
        commit_qc_payload = payload.get("commit_qc")
        if commit_qc_payload is None:
            commit_qc = None
        elif isinstance(commit_qc_payload, Mapping):
            commit_qc = SumeragiCommitQc.from_payload(commit_qc_payload)
        else:
            raise TypeError("commit_qc response `commit_qc` must be an object or null")
        return cls(subject_block_hash=subject_block_hash, commit_qc=commit_qc)


@dataclass(frozen=True)
class SumeragiPrfStatus:
    """Pending PRF (pseudo-random function) window state."""

    height: int
    view: int
    epoch_seed: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPrfStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("PRF status must be an object")
        try:
            height = int(payload.get("height", 0))
            view = int(payload.get("view", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("PRF status `height` and `view` must be numeric") from exc
        seed_value = payload.get("epoch_seed")
        if seed_value is None:
            epoch_seed: Optional[str] = None
        elif isinstance(seed_value, str):
            epoch_seed = seed_value
        else:
            raise TypeError("PRF status `epoch_seed` must be a string when present")
        return cls(height=height, view=view, epoch_seed=epoch_seed)


@dataclass(frozen=True)
class SumeragiLaneSettlementReceipt:
    """Receipt entry bundled in a lane settlement commitment."""

    source_id: str
    local_amount: str
    xor_due: str
    xor_after_haircut: str
    xor_variance: str
    timestamp_ms: int


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


_MAX_NATIVE_AMX_GROUP_SOURCES = 4096


def _required_field(payload: Mapping[str, Any], field_name: str, context: str) -> Any:
    if field_name not in payload:
        raise TypeError(f"{context} is missing required `{field_name}` field")
    return payload[field_name]


def _strict_exact_fields(
    payload: Mapping[str, Any], fields: Iterable[str], context: str
) -> None:
    expected = set(fields)
    unknown = sorted(set(payload).difference(expected))
    if unknown:
        raise ValueError(f"{context} contains unknown field `{unknown[0]}`")
    missing = sorted(expected.difference(payload))
    if missing:
        raise TypeError(f"{context} is missing required `{missing[0]}` field")


def _strict_uint(
    payload: Mapping[str, Any], field_name: str, bits: int, context: str
) -> int:
    value = _required_field(payload, field_name, context)
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{context} `{field_name}` must be an unsigned integer")
    maximum = (1 << bits) - 1
    if value < 0 or value > maximum:
        raise ValueError(
            f"{context} `{field_name}` must be between 0 and {maximum}"
        )
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
        raise ValueError(
            f"{context} `{field_name}` must contain exactly `{tag}` and `{content}`"
        )
    variant = value[tag]
    if not isinstance(variant, str) or variant not in variants:
        raise ValueError(f"{context} `{field_name}` contains an unsupported variant")
    if value[content] is not None:
        raise ValueError(f"{context} `{field_name}.{content}` must be null")
    return variant


def _strict_quantity_string(
    payload: Mapping[str, Any], field_name: str, context: str
) -> str:
    """Decode one canonical bounded non-negative Kotodama quantity."""

    value = _required_field(payload, field_name, context)
    if not isinstance(value, str):
        raise TypeError(f"{context} `{field_name}` must be a quantity string")
    if len(value) > 155:
        raise ValueError(
            f"{context} `{field_name}` exceeds the quantity text length bound"
        )
    matched = re.fullmatch(r"(0|[1-9][0-9]*)(?:\.([0-9]{0,27}[1-9]))?", value)
    if matched is None:
        raise TypeError(
            f"{context} `{field_name}` must be a canonical non-negative quantity"
        )
    fraction = matched.group(2) or ""
    mantissa = int(matched.group(1) + fraction)
    if mantissa > (1 << 511) - 1:
        raise ValueError(f"{context} `{field_name}` exceeds the signed 512-bit domain")
    return value


def _strict_nonempty_string(
    payload: Mapping[str, Any], field_name: str, context: str
) -> str:
    value = _required_field(payload, field_name, context)
    if not isinstance(value, str) or value.strip() == "":
        raise TypeError(f"{context} `{field_name}` must be a non-empty string")
    return value


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


def _require_strictly_ordered_source_ids(
    source_ids: Sequence[str], context: str
) -> None:
    if any(left >= right for left, right in zip(source_ids, source_ids[1:])):
        raise ValueError(f"{context} source IDs must be strictly ordered and unique")


def _crc16_ccitt_false(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def _strict_hash_literal(
    payload: Mapping[str, Any], field_name: str, context: str
) -> str:
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


def _strict_byte_vector(value: Any, length: int, context: str) -> Tuple[int, ...]:
    if not isinstance(value, list) or len(value) != length:
        raise TypeError(f"{context} must contain exactly {length} byte values")
    result: List[int] = []
    for index, byte in enumerate(value):
        if isinstance(byte, bool) or not isinstance(byte, int) or not 0 <= byte <= 255:
            raise TypeError(f"{context}[{index}] must be an integer byte")
        result.append(byte)
    return tuple(result)


@dataclass(frozen=True)
class SumeragiNativeAmxAttestationBody:
    """Context-bound v2 identity signed by a native AMX participant committee."""

    round: SumeragiV2Round
    epoch: int
    chain_id_hash: str
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
    def from_payload(
        cls, payload: Mapping[str, Any]
    ) -> "SumeragiNativeAmxAttestationBody":
        context = "native AMX v2 attestation body"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        expected_fields = {
            "round",
            "epoch",
            "chain_id_hash",
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
        validator_count = _strict_uint(
            payload, "participant_validator_count", 32, context
        )
        min_quorum = _strict_uint(payload, "participant_min_quorum", 32, context)
        expected_quorum = validator_count - (validator_count - 1) // 3 if validator_count else 0
        authority_context_height = _strict_uint(
            payload, "authority_context_height", 64, context
        )
        planned_height = _strict_uint(
            payload, "planned_coordinator_block_height", 64, context
        )
        coordinator_view = _strict_uint(
            payload, "coordinator_lane_block_view", 64, context
        )
        participant_previous_height = _strict_uint(
            payload, "participant_previous_block_height", 64, context
        )
        participant_height = _strict_uint(
            payload, "participant_lane_block_height", 64, context
        )
        participant_view = _strict_uint(
            payload, "participant_lane_block_view", 64, context
        )
        previous_descriptor_value = _required_field(
            payload, "participant_previous_block_descriptor_hash", context
        )
        if previous_descriptor_value is None:
            previous_descriptor_hash: Optional[str] = None
        else:
            previous_descriptor_hash = _strict_hash_literal(
                {
                    "participant_previous_block_descriptor_hash": previous_descriptor_value
                },
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
            or (participant_previous_height == 0)
            != (previous_descriptor_hash is None)
            or validator_count == 0
            or validator_count > 128
            or min_quorum != expected_quorum
        ):
            raise ValueError(f"{context} contains inconsistent round or quorum fields")
        return cls(
            round=round_value,
            epoch=_strict_uint(payload, "epoch", 64, context),
            chain_id_hash=_strict_hash_literal(payload, "chain_id_hash", context),
            source_id=source_id,
            tx_entrypoint_hash=entrypoint_hash,
            plan_digest=_strict_hash_literal(payload, "plan_digest", context),
            phase=phase,
            coordinator_lane_id=_strict_uint(
                payload, "coordinator_lane_id", 32, context
            ),
            coordinator_dataspace_id=_strict_uint(
                payload, "coordinator_dataspace_id", 64, context
            ),
            coordinator_lane_incarnation=_strict_hash_literal(
                payload, "coordinator_lane_incarnation", context
            ),
            participant_lane_id=_strict_uint(
                payload, "participant_lane_id", 32, context
            ),
            participant_dataspace_id=_strict_uint(
                payload, "participant_dataspace_id", 64, context
            ),
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
            self.chain_id_hash,
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


@dataclass(frozen=True)
class SumeragiNativeAmxAttestationQc:
    """Participant validator-set certificate for one native AMX v2 phase."""

    body: SumeragiNativeAmxAttestationBody
    validator_set_hash_version: int
    validator_set_hash: str
    validator_set: Tuple[str, ...]
    validator_set_pops: Tuple[Tuple[int, ...], ...]
    signers_bitmap: Tuple[int, ...]
    bls_aggregate_signature: Tuple[int, ...]

    @classmethod
    def from_payload(
        cls, payload: Mapping[str, Any]
    ) -> "SumeragiNativeAmxAttestationQc":
        context = "native AMX v2 attestation QC"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _strict_exact_fields(
            payload,
            {
                "body",
                "validator_set_hash_version",
                "validator_set_hash",
                "validator_set",
                "validator_set_pops",
                "signers_bitmap",
                "bls_aggregate_signature",
            },
            context,
        )
        body_payload = _required_field(payload, "body", context)
        if not isinstance(body_payload, Mapping):
            raise TypeError(f"{context} `body` must be an object")
        body = SumeragiNativeAmxAttestationBody.from_payload(body_payload)
        version = _strict_uint(payload, "validator_set_hash_version", 16, context)
        if version != 1:
            raise ValueError(f"{context} uses unsupported validator-set hash version {version}")
        validator_set_raw = _required_field(payload, "validator_set", context)
        if (
            not isinstance(validator_set_raw, list)
            or not validator_set_raw
            or len(validator_set_raw) > 128
        ):
            raise TypeError(f"{context} `validator_set` must be a bounded non-empty list")
        validator_set = validate_bls_normal_validator_set(
            validator_set_raw, f"{context} `validator_set`"
        )
        expected_quorum = len(validator_set) - (len(validator_set) - 1) // 3
        validator_set_hash = _strict_hash_literal(
            payload, "validator_set_hash", context
        )
        computed_validator_set_hash = compute_native_amx_validator_set_hash(
            validator_set
        )
        if (
            body.participant_validator_count != len(validator_set)
            or body.participant_min_quorum != expected_quorum
            or body.participant_validator_set_hash != validator_set_hash
            or validator_set_hash != computed_validator_set_hash
        ):
            raise ValueError(f"{context} committee fields differ from the signed body")

        pops_raw = _required_field(payload, "validator_set_pops", context)
        if not isinstance(pops_raw, list) or len(pops_raw) != len(validator_set):
            raise TypeError(f"{context} must carry one proof of possession per validator")
        pops = tuple(
            _strict_byte_vector(pop, 96, f"{context} validator_set_pops[{index}]")
            for index, pop in enumerate(pops_raw)
        )
        if any(not any(pop) for pop in pops):
            raise ValueError(f"{context} proofs of possession must not be all zeroes")

        bitmap_raw = _required_field(payload, "signers_bitmap", context)
        expected_bitmap_len = (len(validator_set) + 7) // 8
        bitmap = _strict_byte_vector(
            bitmap_raw, expected_bitmap_len, f"{context} signers_bitmap"
        )
        trailing_bits = len(validator_set) % 8
        if trailing_bits and bitmap[-1] & ~((1 << trailing_bits) - 1):
            raise ValueError(f"{context} signer bitmap addresses an out-of-range validator")
        signer_count = sum(bin(byte).count("1") for byte in bitmap)
        if signer_count < expected_quorum:
            raise ValueError(
                f"{context} signer bitmap has {signer_count} signers; "
                f"{expected_quorum} required"
            )

        signature = _strict_byte_vector(
            _required_field(payload, "bls_aggregate_signature", context),
            96,
            f"{context} bls_aggregate_signature",
        )
        if not any(signature):
            raise ValueError(f"{context} aggregate signature must not be all zeroes")
        return cls(
            body=body,
            validator_set_hash_version=version,
            validator_set_hash=validator_set_hash,
            validator_set=validator_set,
            validator_set_pops=pops,
            signers_bitmap=bitmap,
            bls_aggregate_signature=signature,
        )


@dataclass(frozen=True)
class SumeragiNativeAmxParticipantLaneBlockDescriptor:
    """Strict control-only participant lane-block descriptor."""

    lane_id: int
    dataspace_id: int
    lane_incarnation: str
    proposal_height: int
    previous_lane_block_height: int
    previous_lane_block_descriptor_hash: Optional[str]
    lane_block_height: int
    lane_block_view: int
    subject_hash: str
    payload_ownership_hash: str
    rbc_instance_hash: str
    accepted_candidate_indices: Tuple[int, ...]
    accepted_transaction_hashes: Tuple[str, ...]
    validator_set_hash_version: int
    validator_set_hash: str
    validator_set: Tuple[str, ...]
    validator_count: int
    min_quorum: int
    qc_mode_tag: str
    descriptor_hash: str

    @classmethod
    def from_payload(
        cls, payload: Mapping[str, Any]
    ) -> "SumeragiNativeAmxParticipantLaneBlockDescriptor":
        context = "native AMX participant lane-block descriptor"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        required_fields = {
            "lane_id",
            "dataspace_id",
            "lane_incarnation",
            "proposal_height",
            "previous_lane_block_height",
            "lane_block_height",
            "lane_block_view",
            "subject_hash",
            "payload_ownership_hash",
            "rbc_instance_hash",
            "accepted_candidate_indices",
            "accepted_transaction_hashes",
            "validator_set_hash_version",
            "validator_set_hash",
            "validator_set",
            "validator_count",
            "min_quorum",
            "qc_mode_tag",
            "descriptor_hash",
        }
        allowed_fields = required_fields | {"previous_lane_block_descriptor_hash"}
        unknown = sorted(set(payload).difference(allowed_fields))
        if unknown:
            raise ValueError(f"{context} contains unknown field `{unknown[0]}`")
        missing = sorted(required_fields.difference(payload))
        if missing:
            raise TypeError(f"{context} is missing required `{missing[0]}` field")

        previous_height = _strict_uint(
            payload, "previous_lane_block_height", 64, context
        )
        previous_value = payload.get("previous_lane_block_descriptor_hash")
        if previous_height == 0:
            if "previous_lane_block_descriptor_hash" in payload:
                raise ValueError(
                    f"{context} must omit the predecessor hash at genesis"
                )
            previous_hash: Optional[str] = None
        else:
            if previous_value is None:
                raise TypeError(f"{context} must carry a predecessor descriptor hash")
            previous_hash = _strict_hash_literal(
                {"previous_lane_block_descriptor_hash": previous_value},
                "previous_lane_block_descriptor_hash",
                context,
            )

        lane_height = _strict_uint(payload, "lane_block_height", 64, context)
        if lane_height == 0 or previous_height + 1 != lane_height:
            raise ValueError(f"{context} lane-block heights must be contiguous")

        indices_value = _required_field(payload, "accepted_candidate_indices", context)
        hashes_value = _required_field(payload, "accepted_transaction_hashes", context)
        if (
            not isinstance(indices_value, list)
            or not isinstance(hashes_value, list)
            or not indices_value
            or len(indices_value) > 4096
            or len(indices_value) != len(hashes_value)
        ):
            raise TypeError(
                f"{context} accepted work must be matching bounded non-empty lists"
            )
        indices = tuple(
            _strict_uint({"index": value}, "index", 64, f"{context} accepted work")
            for value in indices_value
        )
        hashes = tuple(
            _strict_hash_literal(
                {"hash": value}, "hash", f"{context} accepted transaction"
            )
            for value in hashes_value
        )
        if len(set(indices)) != len(indices) or len(set(hashes)) != len(hashes):
            raise ValueError(f"{context} accepted work contains duplicates")

        validators_value = _required_field(payload, "validator_set", context)
        if (
            not isinstance(validators_value, list)
            or not validators_value
            or len(validators_value) > 128
        ):
            raise TypeError(f"{context} validator set must be a bounded non-empty list")
        validators = validate_bls_normal_validator_set(
            validators_value, f"{context} validator set"
        )
        validator_count = _strict_uint(payload, "validator_count", 32, context)
        min_quorum = _strict_uint(payload, "min_quorum", 32, context)
        expected_quorum = len(validators) - (len(validators) - 1) // 3
        version = _strict_uint(payload, "validator_set_hash_version", 16, context)
        if (
            version != 1
            or validator_count != len(validators)
            or min_quorum != expected_quorum
        ):
            raise ValueError(f"{context} contains inconsistent committee fields")

        validator_set_hash = _strict_hash_literal(
            payload, "validator_set_hash", context
        )
        if validator_set_hash != compute_native_amx_validator_set_hash(validators):
            raise ValueError(
                f"{context} validator-set hash does not match the canonical committee"
            )
        parsed = cls(
            lane_id=_strict_uint(payload, "lane_id", 32, context),
            dataspace_id=_strict_uint(payload, "dataspace_id", 64, context),
            lane_incarnation=_strict_hash_literal(
                payload, "lane_incarnation", context
            ),
            proposal_height=_strict_uint(payload, "proposal_height", 64, context),
            previous_lane_block_height=previous_height,
            previous_lane_block_descriptor_hash=previous_hash,
            lane_block_height=lane_height,
            lane_block_view=_strict_uint(payload, "lane_block_view", 64, context),
            subject_hash=_strict_hash_literal(payload, "subject_hash", context),
            payload_ownership_hash=_strict_hash_literal(
                payload, "payload_ownership_hash", context
            ),
            rbc_instance_hash=_strict_hash_literal(
                payload, "rbc_instance_hash", context
            ),
            accepted_candidate_indices=indices,
            accepted_transaction_hashes=hashes,
            validator_set_hash_version=version,
            validator_set_hash=validator_set_hash,
            validator_set=validators,
            validator_count=validator_count,
            min_quorum=min_quorum,
            qc_mode_tag=_require_exact_non_empty_string(
                _required_field(payload, "qc_mode_tag", context),
                f"{context} `qc_mode_tag`",
            ),
            descriptor_hash=_strict_hash_literal(payload, "descriptor_hash", context),
        )
        if parsed.descriptor_hash != compute_native_amx_descriptor_hash(
            asdict(parsed)
        ):
            raise ValueError(
                f"{context} descriptor hash does not match its canonical preimage"
            )
        return parsed


@dataclass(frozen=True)
class SumeragiNativeAmxParticipantLaneBlockProposal:
    """Exact participant proposal, with recovery payload hints forbidden."""

    descriptor: SumeragiNativeAmxParticipantLaneBlockDescriptor
    proposal_hash: str

    @classmethod
    def from_payload(
        cls, payload: Mapping[str, Any]
    ) -> "SumeragiNativeAmxParticipantLaneBlockProposal":
        context = "native AMX participant lane-block proposal"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _strict_exact_fields(payload, {"descriptor", "proposal_hash"}, context)
        descriptor = _required_field(payload, "descriptor", context)
        if not isinstance(descriptor, Mapping):
            raise TypeError(f"{context} `descriptor` must be an object")
        parsed_descriptor = (
            SumeragiNativeAmxParticipantLaneBlockDescriptor.from_payload(descriptor)
        )
        proposal_hash = _strict_hash_literal(payload, "proposal_hash", context)
        if proposal_hash != compute_native_amx_proposal_hash(
            asdict(parsed_descriptor)
        ):
            raise ValueError(
                f"{context} proposal hash does not match its canonical preimage"
            )
        return cls(descriptor=parsed_descriptor, proposal_hash=proposal_hash)


@dataclass(frozen=True)
class SumeragiNativeAmxLeg:
    """Prepare and commit v2 certificates for one participant lane/dataspace."""

    lane_id: int
    dataspace_id: int
    lane_incarnation: str
    participant_proposal: SumeragiNativeAmxParticipantLaneBlockProposal
    participant_settlement: SumeragiLaneSettlementCommitment
    participant_settlement_hash: str
    requires_mixed_role_anchor_validation: bool
    prepare_qc: SumeragiNativeAmxAttestationQc
    commit_qc: SumeragiNativeAmxAttestationQc

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiNativeAmxLeg":
        context = "native AMX v2 leg"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _strict_exact_fields(
            payload,
            {
                "lane_id",
                "dataspace_id",
                "participant_proposal",
                "participant_settlement",
                "participant_settlement_hash",
                "prepare_qc",
                "commit_qc",
            },
            context,
        )
        proposal_payload = _required_field(payload, "participant_proposal", context)
        settlement_payload = _required_field(payload, "participant_settlement", context)
        prepare_payload = _required_field(payload, "prepare_qc", context)
        commit_payload = _required_field(payload, "commit_qc", context)
        if (
            not isinstance(proposal_payload, Mapping)
            or not isinstance(settlement_payload, Mapping)
            or not isinstance(prepare_payload, Mapping)
            or not isinstance(commit_payload, Mapping)
        ):
            raise TypeError(f"{context} participant artifacts and QCs must be objects")
        # The protocol-defined participant settlement is terminal and cannot
        # recursively embed another native AMX receipt.
        if settlement_payload.get("native_amx_receipts") != []:
            raise ValueError(f"{context} participant settlement must be terminal")
        if settlement_payload.get("nexus_fee_receipts") != []:
            raise ValueError(f"{context} participant settlement cannot charge a fee")
        settlement_receipts_payload = settlement_payload.get("receipts")
        if (
            not isinstance(settlement_receipts_payload, list)
            or not settlement_receipts_payload
            or len(settlement_receipts_payload)
            > _MAX_NATIVE_AMX_GROUP_SOURCES
        ):
            raise ValueError(
                f"{context} participant settlement receipts must be bounded and non-empty"
            )
        proposal = SumeragiNativeAmxParticipantLaneBlockProposal.from_payload(
            proposal_payload
        )
        settlement = SumeragiLaneSettlementCommitment.from_payload(settlement_payload)
        prepare = SumeragiNativeAmxAttestationQc.from_payload(prepare_payload)
        commit = SumeragiNativeAmxAttestationQc.from_payload(commit_payload)
        if prepare.body.phase is not SumeragiNativeAmxPhase.PREPARE:
            raise ValueError(f"{context} prepare QC carries the wrong phase")
        if commit.body.phase is not SumeragiNativeAmxPhase.COMMIT:
            raise ValueError(f"{context} commit QC carries the wrong phase")
        if prepare.body.identity() != commit.body.identity():
            raise ValueError(f"{context} prepare and commit QC identities do not match")
        if (
            prepare.validator_set_hash_version != commit.validator_set_hash_version
            or prepare.validator_set_hash != commit.validator_set_hash
            or prepare.validator_set != commit.validator_set
            or prepare.validator_set_pops != commit.validator_set_pops
        ):
            raise ValueError(f"{context} prepare and commit validator sets do not match")
        lane_id = _strict_uint(payload, "lane_id", 32, context)
        dataspace_id = _strict_uint(payload, "dataspace_id", 64, context)
        settlement_hash = _strict_hash_literal(
            payload, "participant_settlement_hash", context
        )
        if settlement_hash != compute_native_amx_participant_settlement_hash(
            asdict(settlement)
        ):
            raise ValueError(
                f"{context} participant settlement hash does not match its canonical commitment"
            )
        body = prepare.body
        descriptor = proposal.descriptor
        if (
            body.participant_lane_id != lane_id
            or body.participant_dataspace_id != dataspace_id
            or descriptor.lane_id != lane_id
            or descriptor.dataspace_id != dataspace_id
            or descriptor.lane_incarnation != body.participant_lane_incarnation
            or descriptor.proposal_height != body.authority_context_height
            or descriptor.previous_lane_block_height
            != body.participant_previous_block_height
            or descriptor.previous_lane_block_descriptor_hash
            != body.participant_previous_block_descriptor_hash
            or descriptor.lane_block_height != body.participant_lane_block_height
            or descriptor.lane_block_view != body.participant_lane_block_view
            or proposal.proposal_hash != body.participant_proposal_hash
            or descriptor.validator_set_hash_version
            != prepare.validator_set_hash_version
            or descriptor.validator_set_hash != prepare.validator_set_hash
            or descriptor.validator_set != prepare.validator_set
            or descriptor.validator_count != body.participant_validator_count
            or descriptor.min_quorum != body.participant_min_quorum
        ):
            raise ValueError(f"{context} participant proposal differs from its QC bodies")
        settlement_sources = [receipt.source_id for receipt in settlement.receipts]
        _require_strictly_ordered_source_ids(
            settlement_sources, f"{context} participant settlement"
        )
        matching_entrypoint_positions = tuple(
            index
            for index, entrypoint_hash in enumerate(
                descriptor.accepted_transaction_hashes
            )
            if entrypoint_hash == body.tx_entrypoint_hash
        )
        if len(matching_entrypoint_positions) > 1:
            raise ValueError(
                f"{context} participant descriptor repeats the current transaction entrypoint"
            )
        requires_mixed_role_anchor_validation = not matching_entrypoint_positions
        if not requires_mixed_role_anchor_validation:
            position = matching_entrypoint_positions[0]
            if (
                len(descriptor.accepted_candidate_indices) != len(settlement.receipts)
                or len(descriptor.accepted_transaction_hashes)
                != len(settlement.receipts)
                or settlement.receipts[position].source_id != body.source_id
            ):
                raise ValueError(
                    f"{context} participant descriptor and grouped settlement are not aligned"
                )
        if (
            settlement_hash != body.participant_settlement_commitment
            or settlement.block_height != body.participant_lane_block_height
            or settlement.lane_id != lane_id
            or settlement.dataspace_id != dataspace_id
            or settlement.lane_incarnation != body.participant_lane_incarnation
            or settlement.tx_count != len(settlement.receipts)
            or settlement.total_local_amount != "0"
            or settlement.total_xor_due != "0"
            or settlement.total_xor_after_haircut != "0"
            or settlement.total_xor_variance != "0"
            or settlement.swap_metadata is not None
            or len(set(settlement_sources)) != len(settlement_sources)
            or settlement_sources.count(body.source_id) != 1
            or any(
                receipt.local_amount != "0"
                or receipt.xor_due != "0"
                or receipt.xor_after_haircut != "0"
                or receipt.xor_variance != "0"
                or receipt.timestamp_ms != body.authority_context_height
                for receipt in settlement.receipts
            )
            or settlement.nexus_fee_receipts
            or settlement.native_amx_receipts
        ):
            raise ValueError(f"{context} participant settlement differs from its QC body")
        return cls(
            lane_id=lane_id,
            dataspace_id=dataspace_id,
            lane_incarnation=body.participant_lane_incarnation,
            participant_proposal=proposal,
            participant_settlement=settlement,
            participant_settlement_hash=settlement_hash,
            requires_mixed_role_anchor_validation=requires_mixed_role_anchor_validation,
            prepare_qc=prepare,
            commit_qc=commit,
        )


@dataclass(frozen=True)
class SumeragiNativeAmxReceipt:
    """Validated context-bound native AMX v2 coordinator receipt."""

    version: int
    source_id: SumeragiNativeAmxSourceId
    chain_id_hash: str
    plan_digest: str
    lane_id: int
    dataspace_id: int
    lane_incarnation: str
    authority_context_height: int
    lane_block_height: int
    lane_block_view: int
    coordinator_proposal_hash: str
    legs: Tuple[SumeragiNativeAmxLeg, ...]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiNativeAmxReceipt":
        context = "native AMX v2 receipt"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _strict_exact_fields(
            payload,
            {
                "version",
                "source_id",
                "chain_id_hash",
                "plan_digest",
                "lane_id",
                "dataspace_id",
                "lane_incarnation",
                "authority_context_height",
                "lane_block_height",
                "lane_block_view",
                "coordinator_proposal_hash",
                "legs",
            },
            context,
        )
        version = _strict_uint(payload, "version", 16, context)
        if version != 2:
            raise ValueError(f"{context} uses unsupported version {version}")
        source_id = SumeragiNativeAmxSourceId(
            _strict_hex_string(payload, "source_id", 32, context)
        )
        chain_id_hash = _strict_hash_literal(payload, "chain_id_hash", context)
        plan_digest = _strict_hash_literal(payload, "plan_digest", context)
        lane_id = _strict_uint(payload, "lane_id", 32, context)
        dataspace_id = _strict_uint(payload, "dataspace_id", 64, context)
        lane_incarnation = _strict_hash_literal(payload, "lane_incarnation", context)
        authority_context_height = _strict_uint(
            payload, "authority_context_height", 64, context
        )
        lane_block_height = _strict_uint(payload, "lane_block_height", 64, context)
        lane_block_view = _strict_uint(payload, "lane_block_view", 64, context)
        coordinator_proposal_hash = _strict_hash_literal(
            payload, "coordinator_proposal_hash", context
        )
        if authority_context_height == 0 or lane_block_height == 0:
            raise ValueError(f"{context} authority and lane-block heights must be non-zero")
        legs_raw = _required_field(payload, "legs", context)
        if not isinstance(legs_raw, list) or not 0 < len(legs_raw) < 256:
            raise TypeError(f"{context} `legs` must be a bounded non-empty list")
        legs = tuple(SumeragiNativeAmxLeg.from_payload(leg) for leg in legs_raw)
        identities = {(leg.lane_id, leg.dataspace_id) for leg in legs}
        if len(identities) != len(legs):
            raise ValueError(f"{context} contains duplicate participant legs")
        expected_round = legs[0].prepare_qc.body.round
        expected_epoch = legs[0].prepare_qc.body.epoch
        entrypoint_hash: Optional[SumeragiNativeAmxTransactionEntrypointHash] = None
        for leg in legs:
            body = leg.prepare_qc.body
            if leg.lane_id == lane_id and leg.dataspace_id == dataspace_id:
                descriptor = leg.participant_proposal.descriptor
                if (
                    leg.requires_mixed_role_anchor_validation
                    or leg.lane_incarnation != lane_incarnation
                    or descriptor.lane_incarnation != lane_incarnation
                    or descriptor.lane_block_height != lane_block_height
                    or descriptor.lane_block_view != lane_block_view
                    or leg.participant_proposal.proposal_hash
                    != coordinator_proposal_hash
                ):
                    raise ValueError(
                        f"{context} same-route proposal is not the coordinator identity"
                    )
            if body.round != expected_round or body.epoch != expected_epoch:
                raise ValueError(f"{context} legs carry mismatched frozen round context")
            if body.chain_id_hash != chain_id_hash:
                raise ValueError(f"{context} chain identity differs from a QC body")
            if body.source_id != source_id:
                raise ValueError(f"{context} source identity differs from a QC body")
            if body.plan_digest != plan_digest:
                raise ValueError(f"{context} plan digest differs from a QC body")
            if (
                body.coordinator_lane_id != lane_id
                or body.coordinator_dataspace_id != dataspace_id
                or body.coordinator_lane_incarnation != lane_incarnation
            ):
                raise ValueError(f"{context} coordinator identity differs from a QC body")
            if (
                body.authority_context_height != authority_context_height
                or body.planned_coordinator_block_height != lane_block_height
                or body.coordinator_lane_block_view != lane_block_view
                or body.coordinator_proposal_hash != coordinator_proposal_hash
            ):
                raise ValueError(f"{context} coordinator session differs from a QC body")
            if entrypoint_hash is None:
                entrypoint_hash = body.tx_entrypoint_hash
            elif body.tx_entrypoint_hash != entrypoint_hash:
                raise ValueError(f"{context} legs carry mismatched entrypoint hashes")
        return cls(
            version=version,
            source_id=source_id,
            chain_id_hash=chain_id_hash,
            plan_digest=plan_digest,
            lane_id=lane_id,
            dataspace_id=dataspace_id,
            lane_incarnation=lane_incarnation,
            authority_context_height=authority_context_height,
            lane_block_height=lane_block_height,
            lane_block_view=lane_block_view,
            coordinator_proposal_hash=coordinator_proposal_hash,
            legs=legs,
        )


@dataclass(frozen=True)
class SumeragiNexusFeeScheduleInputs:
    """Exact fee inputs needed to recompute a Nexus fee receipt."""

    tx_bytes_len: int
    instruction_count: int
    gas_used: int
    base_fee: str
    per_byte_fee: str
    per_instruction_fee: str
    per_gas_unit_fee: str

    @classmethod
    def from_payload(
        cls, payload: Mapping[str, Any]
    ) -> "SumeragiNexusFeeScheduleInputs":
        context = "Nexus fee schedule"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _strict_exact_fields(
            payload,
            {
                "tx_bytes_len",
                "instruction_count",
                "gas_used",
                "base_fee",
                "per_byte_fee",
                "per_instruction_fee",
                "per_gas_unit_fee",
            },
            context,
        )
        return cls(
            tx_bytes_len=_strict_uint(payload, "tx_bytes_len", 64, context),
            instruction_count=_strict_uint(payload, "instruction_count", 64, context),
            gas_used=_strict_uint(payload, "gas_used", 64, context),
            base_fee=_strict_quantity_string(payload, "base_fee", context),
            per_byte_fee=_strict_quantity_string(payload, "per_byte_fee", context),
            per_instruction_fee=_strict_quantity_string(
                payload, "per_instruction_fee", context
            ),
            per_gas_unit_fee=_strict_quantity_string(
                payload, "per_gas_unit_fee", context
            ),
        )


@dataclass(frozen=True)
class SumeragiNexusFeeReceipt:
    """Versioned public Nexus fee charge committed by a lane block."""

    version: int
    source_id: str
    dataspace_id: int
    lane_id: int
    block_height: int
    payer_account_id: str
    fee_asset_id: str
    fee_amount: str
    schedule: SumeragiNexusFeeScheduleInputs

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiNexusFeeReceipt":
        context = "Nexus fee receipt"
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _strict_exact_fields(
            payload,
            {
                "version",
                "source_id",
                "dataspace_id",
                "lane_id",
                "block_height",
                "payer_account_id",
                "fee_asset_id",
                "fee_amount",
                "schedule",
            },
            context,
        )
        version = _strict_uint(payload, "version", 16, context)
        if version != 1:
            raise ValueError(f"{context} uses unsupported version {version}")
        schedule_payload = _required_field(payload, "schedule", context)
        if not isinstance(schedule_payload, Mapping):
            raise TypeError(f"{context} `schedule` must be an object")
        return cls(
            version=version,
            source_id=_strict_hex_string(payload, "source_id", 32, context),
            dataspace_id=_strict_uint(payload, "dataspace_id", 64, context),
            lane_id=_strict_uint(payload, "lane_id", 32, context),
            block_height=_strict_uint(payload, "block_height", 64, context),
            payer_account_id=_strict_nonempty_string(
                payload, "payer_account_id", context
            ),
            fee_asset_id=_strict_nonempty_string(payload, "fee_asset_id", context),
            fee_amount=_strict_quantity_string(payload, "fee_amount", context),
            schedule=SumeragiNexusFeeScheduleInputs.from_payload(schedule_payload),
        )


@dataclass(frozen=True)
class SumeragiLaneSwapMetadata:
    """Swap metadata attached to a lane settlement commitment."""

    epsilon_bps: int
    twap_window_seconds: int
    liquidity_profile: str
    twap_local_per_xor: str
    volatility_class: str


@dataclass(frozen=True)
class SumeragiLaneSettlementCommitment:
    """Lane settlement totals and receipts bundled into sumeragi status."""

    block_height: int
    lane_id: int
    lane_incarnation: str
    dataspace_id: int
    tx_count: int
    total_local_amount: str
    total_xor_due: str
    total_xor_after_haircut: str
    total_xor_variance: str
    receipts: List[SumeragiLaneSettlementReceipt]
    nexus_fee_receipts: Tuple[SumeragiNexusFeeReceipt, ...]
    native_amx_receipts: Tuple[SumeragiNativeAmxReceipt, ...]
    swap_metadata: Optional[SumeragiLaneSwapMetadata]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLaneSettlementCommitment":
        if not isinstance(payload, Mapping):
            raise TypeError("lane settlement commitment must be an object")
        context = "lane settlement commitment"
        _strict_exact_fields(
            payload,
            {
                "block_height",
                "lane_id",
                "lane_incarnation",
                "dataspace_id",
                "tx_count",
                "total_local_amount",
                "total_xor_due",
                "total_xor_after_haircut",
                "total_xor_variance",
                "swap_metadata",
                "receipts",
                "nexus_fee_receipts",
                "native_amx_receipts",
            },
            context,
        )
        block_height = _strict_uint(payload, "block_height", 64, context)
        lane_id = _strict_uint(payload, "lane_id", 32, context)
        lane_incarnation = _strict_hash_literal(payload, "lane_incarnation", context)
        dataspace_id = _strict_uint(payload, "dataspace_id", 64, context)
        tx_count = _strict_uint(payload, "tx_count", 64, context)
        total_local_amount = _strict_quantity_string(
            payload, "total_local_amount", context
        )
        total_xor_due = _strict_quantity_string(payload, "total_xor_due", context)
        total_xor_after_haircut = _strict_quantity_string(
            payload, "total_xor_after_haircut", context
        )
        total_xor_variance = _strict_quantity_string(
            payload, "total_xor_variance", context
        )
        receipts_payload = _required_field(payload, "receipts", context)
        if not isinstance(receipts_payload, list):
            raise TypeError("lane settlement `receipts` must be a list")
        receipts: List[SumeragiLaneSettlementReceipt] = []
        for index, receipt in enumerate(receipts_payload):
            if not isinstance(receipt, Mapping):
                raise TypeError("lane settlement receipts must be objects")
            receipt_context = f"lane settlement receipt at index {index}"
            _strict_exact_fields(
                receipt,
                {
                    "source_id",
                    "local_amount",
                    "xor_due",
                    "xor_after_haircut",
                    "xor_variance",
                    "timestamp_ms",
                },
                receipt_context,
            )
            source_id = _strict_hex_string(receipt, "source_id", 32, receipt_context)
            receipt_local = _strict_quantity_string(
                receipt, "local_amount", receipt_context
            )
            receipt_due = _strict_quantity_string(receipt, "xor_due", receipt_context)
            receipt_after = _strict_quantity_string(
                receipt, "xor_after_haircut", receipt_context
            )
            receipt_variance = _strict_quantity_string(
                receipt, "xor_variance", receipt_context
            )
            receipt_timestamp = _strict_uint(receipt, "timestamp_ms", 64, receipt_context)
            receipts.append(
                SumeragiLaneSettlementReceipt(
                    source_id=source_id,
                    local_amount=receipt_local,
                    xor_due=receipt_due,
                    xor_after_haircut=receipt_after,
                    xor_variance=receipt_variance,
                    timestamp_ms=receipt_timestamp,
                )
            )
        nexus_fee_payload = _required_field(payload, "nexus_fee_receipts", context)
        if not isinstance(nexus_fee_payload, list):
            raise TypeError("lane settlement `nexus_fee_receipts` must be a list")
        nexus_fee_receipts = tuple(
            SumeragiNexusFeeReceipt.from_payload(receipt) for receipt in nexus_fee_payload
        )
        native_amx_payload = _required_field(payload, "native_amx_receipts", context)
        if not isinstance(native_amx_payload, list):
            raise TypeError("lane settlement `native_amx_receipts` must be a list")
        if len(native_amx_payload) > _MAX_NATIVE_AMX_GROUP_SOURCES:
            raise ValueError(
                "lane settlement `native_amx_receipts` exceeds the grouped source bound"
            )
        native_amx_receipts = tuple(
            SumeragiNativeAmxReceipt.from_payload(receipt)
            for receipt in native_amx_payload
        )
        native_amx_sources = tuple(
            receipt.source_id for receipt in native_amx_receipts
        )
        _require_strictly_ordered_source_ids(
            native_amx_sources, "lane settlement native AMX receipt group"
        )
        for receipt in nexus_fee_receipts:
            if (
                receipt.lane_id != lane_id
                or receipt.dataspace_id != dataspace_id
                or receipt.block_height != block_height
            ):
                raise ValueError(
                    "lane settlement receipt coordinates differ from the containing commitment"
                )
        for receipt in native_amx_receipts:
            if (
                receipt.lane_id != lane_id
                or receipt.dataspace_id != dataspace_id
                or receipt.lane_incarnation != lane_incarnation
                or receipt.lane_block_height != block_height
            ):
                raise ValueError(
                    "lane settlement receipt coordinates differ from the containing commitment"
                )
        if len({receipt.source_id for receipt in nexus_fee_receipts}) != len(
            nexus_fee_receipts
        ):
            raise ValueError("lane settlement contains duplicate Nexus fee receipt sources")
        for receipt in native_amx_receipts:
            for leg in receipt.legs:
                participant_sources = tuple(
                    settlement_receipt.source_id
                    for settlement_receipt in leg.participant_settlement.receipts
                )
                if participant_sources != native_amx_sources:
                    raise ValueError(
                        "lane settlement native AMX receipt does not bind the exact "
                        "ordered source group"
                    )

        swap_metadata_payload = _required_field(payload, "swap_metadata", context)
        swap_metadata: Optional[SumeragiLaneSwapMetadata]
        if swap_metadata_payload is None:
            swap_metadata = None
        elif isinstance(swap_metadata_payload, Mapping):
            _strict_exact_fields(
                swap_metadata_payload,
                {
                    "epsilon_bps",
                    "twap_window_seconds",
                    "liquidity_profile",
                    "twap_local_per_xor",
                    "volatility_class",
                },
                "lane swap metadata",
            )
            swap_metadata = SumeragiLaneSwapMetadata(
                epsilon_bps=_strict_uint(
                    swap_metadata_payload, "epsilon_bps", 16, "lane swap metadata"
                ),
                twap_window_seconds=_strict_uint(
                    swap_metadata_payload,
                    "twap_window_seconds",
                    32,
                    "lane swap metadata",
                ),
                liquidity_profile=_strict_tagged_unit_enum(
                    swap_metadata_payload,
                    "liquidity_profile",
                    tag="profile",
                    content="state",
                    variants=("Tier1", "Tier2", "Tier3"),
                    context="lane swap metadata",
                ),
                twap_local_per_xor=_strict_nonempty_string(
                    swap_metadata_payload, "twap_local_per_xor", "lane swap metadata"
                ),
                volatility_class=_strict_tagged_unit_enum(
                    swap_metadata_payload,
                    "volatility_class",
                    tag="bucket",
                    content="state",
                    variants=("Stable", "Elevated", "Dislocated"),
                    context="lane swap metadata",
                ),
            )
        else:
            raise TypeError("lane settlement `swap_metadata` must be an object when present")
        return cls(
            block_height=block_height,
            lane_id=lane_id,
            lane_incarnation=lane_incarnation,
            dataspace_id=dataspace_id,
            tx_count=tx_count,
            total_local_amount=total_local_amount,
            total_xor_due=total_xor_due,
            total_xor_after_haircut=total_xor_after_haircut,
            total_xor_variance=total_xor_variance,
            receipts=receipts,
            nexus_fee_receipts=nexus_fee_receipts,
            native_amx_receipts=native_amx_receipts,
            swap_metadata=swap_metadata,
        )


@dataclass(frozen=True)
class SumeragiLaneRelayEnvelope:
    """Canonical relay status envelope and its exact settlement commitment."""

    lane_id: int
    lane_incarnation: str
    dataspace_id: int
    block_height: int
    block_header: Mapping[str, Any]
    qc: Optional[Mapping[str, Any]]
    da_commitment_hash: Optional[str]
    lane_block_descriptor_hash: Optional[str]
    settlement_commitment: SumeragiLaneSettlementCommitment
    settlement_hash: str
    rbc_bytes_total: int
    manifest_root: Optional[str]
    fastpq_proof: Optional[Mapping[str, Any]]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLaneRelayEnvelope":
        if not isinstance(payload, Mapping):
            raise TypeError("lane relay envelope must be an object")
        context = "lane relay envelope"
        _strict_exact_fields(
            payload,
            {
                "lane_id",
                "lane_incarnation",
                "dataspace_id",
                "block_height",
                "block_header",
                "qc",
                "da_commitment_hash",
                "lane_block_descriptor_hash",
                "settlement_commitment",
                "settlement_hash",
                "rbc_bytes_total",
                "manifest_root",
                "fastpq_proof",
            },
            context,
        )
        lane_id = _strict_uint(payload, "lane_id", 32, context)
        dataspace_id = _strict_uint(payload, "dataspace_id", 64, context)
        block_height = _strict_uint(payload, "block_height", 64, context)
        rbc_bytes_total = _strict_uint(payload, "rbc_bytes_total", 64, context)
        lane_incarnation = _strict_hash_literal(payload, "lane_incarnation", context)
        block_header = _required_field(payload, "block_header", context)
        if not isinstance(block_header, Mapping):
            raise TypeError("lane relay `block_header` must be an object")
        qc = _required_field(payload, "qc", context)
        if qc is not None and not isinstance(qc, Mapping):
            raise TypeError("lane relay `qc` must be an object when present")
        da_commitment_hash = _required_field(payload, "da_commitment_hash", context)
        if da_commitment_hash is not None:
            da_commitment_hash = _strict_hash_literal(
                {"da_commitment_hash": da_commitment_hash},
                "da_commitment_hash",
                context,
            )
        descriptor_hash = payload.get("lane_block_descriptor_hash")
        lane_block_descriptor_hash = (
            None
            if descriptor_hash is None
            else _strict_hash_literal(
                {"lane_block_descriptor_hash": descriptor_hash},
                "lane_block_descriptor_hash",
                context,
            )
        )
        settlement_hash = _strict_hash_literal(payload, "settlement_hash", context)
        settlement_payload = _required_field(payload, "settlement_commitment", context)
        if not isinstance(settlement_payload, Mapping):
            raise TypeError("lane relay `settlement_commitment` must be an object")
        settlement_commitment = SumeragiLaneSettlementCommitment.from_payload(settlement_payload)
        if (
            settlement_commitment.lane_id != lane_id
            or settlement_commitment.dataspace_id != dataspace_id
            or settlement_commitment.block_height != block_height
            or settlement_commitment.lane_incarnation != lane_incarnation
        ):
            raise ValueError(
                "lane relay coordinates differ from the embedded settlement commitment"
            )
        manifest_root_value = payload.get("manifest_root")
        manifest_root = (
            None
            if manifest_root_value is None
            else _strict_hex_string(
                {"manifest_root": manifest_root_value},
                "manifest_root",
                32,
                context,
            )
        )
        fastpq_value = payload.get("fastpq_proof")
        if fastpq_value is None:
            fastpq_proof: Optional[Mapping[str, Any]] = None
        elif isinstance(fastpq_value, Mapping):
            if set(fastpq_value) != {"proof_digest", "verified_at_height"}:
                raise ValueError(
                    "lane relay `fastpq_proof` contains unexpected fields"
                )
            fastpq_proof = {
                "proof_digest": _strict_hash_literal(
                    fastpq_value, "proof_digest", "lane relay FastPQ proof"
                ),
                "verified_at_height": _strict_uint(
                    fastpq_value,
                    "verified_at_height",
                    64,
                    "lane relay FastPQ proof",
                ),
            }
        else:
            raise TypeError("lane relay `fastpq_proof` must be an object when present")
        return cls(
            lane_id=lane_id,
            lane_incarnation=lane_incarnation,
            dataspace_id=dataspace_id,
            block_height=block_height,
            block_header=dict(block_header),
            qc=None if qc is None else dict(qc),
            da_commitment_hash=da_commitment_hash,
            lane_block_descriptor_hash=lane_block_descriptor_hash,
            settlement_commitment=settlement_commitment,
            settlement_hash=settlement_hash,
            rbc_bytes_total=rbc_bytes_total,
            manifest_root=manifest_root,
            fastpq_proof=fastpq_proof,
        )


class SumeragiV2StatusPhase(str, Enum):
    """High-level state of the authoritative Sumeragi v2 reducer."""

    AWAITING_PROPOSAL = "awaiting_proposal"
    RECONSTRUCTING_PAYLOAD = "reconstructing_payload"
    VALIDATING_PAYLOAD = "validating_payload"
    PREPARE = "prepare"
    COMMIT = "commit"
    PENDING_APPLY = "pending_apply"


class SumeragiV2BodyState(str, Enum):
    """Local state of the proposal body reported by Sumeragi v2."""

    MISSING = "missing"
    RECONSTRUCTING = "reconstructing"
    STORED = "stored"
    VALIDATED = "validated"
    PENDING_APPLY = "pending_apply"
    APPLIED = "applied"


class SumeragiV2GlobalPhase(str, Enum):
    """Global two-phase consensus phase."""

    PREPARE = "prepare"
    COMMIT = "commit"


_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION = 1
_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES = 1024
_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT = (
    "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
)


def _sumeragi_v2_exact_fields(
    payload: Mapping[str, Any], allowed: Sequence[str], context: str
) -> None:
    unknown = sorted(set(payload) - set(allowed))
    if unknown:
        raise TypeError(f"{context} contains unsupported fields: {', '.join(unknown)}")


def _sumeragi_v2_uint(value: Any, context: str, maximum: int = (1 << 64) - 1) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{context} must be an unsigned integer")
    if value < 0 or value > maximum:
        raise ValueError(f"{context} is outside its unsigned integer range")
    return value


def _sumeragi_v2_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value.strip() or value != value.strip():
        raise TypeError(f"{context} must be a non-empty string without surrounding whitespace")
    return value


def _sumeragi_v2_tagged_unit(
    payload: Any, tag: str, admitted: Sequence[str], context: str
) -> str:
    if not isinstance(payload, Mapping):
        raise TypeError(f"{context} must be an object")
    _sumeragi_v2_exact_fields(payload, (tag, "details"), context)
    if set(payload) != {tag, "details"} or payload.get("details") is not None:
        raise TypeError(f"{context} must contain `{tag}` and null `details`")
    variant = _sumeragi_v2_string(payload.get(tag), f"{context}.{tag}")
    if variant not in admitted:
        raise ValueError(f"{context}.{tag} has unknown variant {variant!r}")
    return variant


@dataclass(frozen=True)
class SumeragiV2HeightContextId:
    """Hash identifying one immutable height context."""

    hash: str

    @classmethod
    def from_payload(cls, payload: Any, context: str) -> "SumeragiV2HeightContextId":
        if (
            not isinstance(payload, list)
            or len(payload) != 1
        ):
            raise TypeError(f"{context} must be a one-element tuple")
        return cls(hash=_sumeragi_v2_string(payload[0], f"{context}[0]"))


@dataclass(frozen=True)
class SumeragiV2ConsensusRound:
    """Context-bound Sumeragi height and view."""

    context_id: SumeragiV2HeightContextId
    height: int
    view: int

    @classmethod
    def from_payload(cls, payload: Any, context: str) -> "SumeragiV2ConsensusRound":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _sumeragi_v2_exact_fields(payload, ("context_id", "height", "view"), context)
        return cls(
            context_id=SumeragiV2HeightContextId.from_payload(
                payload.get("context_id"), f"{context}.context_id"
            ),
            height=_sumeragi_v2_uint(payload.get("height"), f"{context}.height"),
            view=_sumeragi_v2_uint(payload.get("view"), f"{context}.view"),
        )


@dataclass(frozen=True)
class SumeragiV2BlockSubject:
    """Exact block and payload hashes certified by consensus."""

    parent_block_hash: Optional[str]
    block_hash: str
    payload_hash: str

    @classmethod
    def from_payload(cls, payload: Any, context: str) -> "SumeragiV2BlockSubject":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _sumeragi_v2_exact_fields(
            payload, ("parent_block_hash", "block_hash", "payload_hash"), context
        )
        parent_block_hash_value = payload.get("parent_block_hash")
        return cls(
            parent_block_hash=(
                None
                if parent_block_hash_value is None
                else _sumeragi_v2_string(
                    parent_block_hash_value, f"{context}.parent_block_hash"
                )
            ),
            block_hash=_sumeragi_v2_string(
                payload.get("block_hash"), f"{context}.block_hash"
            ),
            payload_hash=_sumeragi_v2_string(
                payload.get("payload_hash"), f"{context}.payload_hash"
            ),
        )


@dataclass(frozen=True)
class SumeragiV2ExecutionCommitment:
    """Exact deterministic execution result authenticated by a v2 QC."""

    parent_state_root: str
    post_state_root: str
    ordinary_writes_root: str
    topup_anchor_root: Optional[str]
    topup_anchor_count: int
    native_amx_application_manifest_version: int
    native_amx_application_manifest_root: str
    native_amx_application_manifest_count: int
    executed_block_wire_hash: str

    @classmethod
    def from_payload(
        cls, payload: Any, context: str
    ) -> "SumeragiV2ExecutionCommitment":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _sumeragi_v2_exact_fields(
            payload,
            (
                "parent_state_root",
                "post_state_root",
                "ordinary_writes_root",
                "topup_anchor_root",
                "topup_anchor_count",
                "native_amx_application_manifest_version",
                "native_amx_application_manifest_root",
                "native_amx_application_manifest_count",
                "executed_block_wire_hash",
            ),
            context,
        )
        topup_anchor_count = _sumeragi_v2_uint(
            payload.get("topup_anchor_count"),
            f"{context}.topup_anchor_count",
            maximum=16,
        )
        topup_anchor_root_value = payload.get("topup_anchor_root")
        topup_anchor_root = (
            None
            if topup_anchor_root_value is None
            else _sumeragi_v2_string(
                topup_anchor_root_value, f"{context}.topup_anchor_root"
            )
        )
        if (topup_anchor_count == 0) != (topup_anchor_root is None):
            raise ValueError(
                f"{context}.topup_anchor_root must be present exactly when "
                "topup_anchor_count is positive"
            )
        native_manifest_version = _sumeragi_v2_uint(
            payload.get("native_amx_application_manifest_version"),
            f"{context}.native_amx_application_manifest_version",
            maximum=(1 << 16) - 1,
        )
        if native_manifest_version != _SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION:
            raise ValueError(
                f"{context}.native_amx_application_manifest_version must equal "
                f"{_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION}"
            )
        native_manifest_root = _strict_hash_literal(
            payload,
            "native_amx_application_manifest_root",
            context,
        )
        native_manifest_count = _sumeragi_v2_uint(
            payload.get("native_amx_application_manifest_count"),
            f"{context}.native_amx_application_manifest_count",
            maximum=_SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES,
        )
        if (native_manifest_count == 0) != (
            native_manifest_root
            == _SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT
        ):
            raise ValueError(
                f"{context}.native_amx_application_manifest_count must be zero "
                "exactly for the canonical empty root"
            )
        return cls(
            parent_state_root=_sumeragi_v2_string(
                payload.get("parent_state_root"), f"{context}.parent_state_root"
            ),
            post_state_root=_sumeragi_v2_string(
                payload.get("post_state_root"), f"{context}.post_state_root"
            ),
            ordinary_writes_root=_sumeragi_v2_string(
                payload.get("ordinary_writes_root"),
                f"{context}.ordinary_writes_root",
            ),
            topup_anchor_root=topup_anchor_root,
            topup_anchor_count=topup_anchor_count,
            native_amx_application_manifest_version=native_manifest_version,
            native_amx_application_manifest_root=native_manifest_root,
            native_amx_application_manifest_count=native_manifest_count,
            executed_block_wire_hash=_sumeragi_v2_string(
                payload.get("executed_block_wire_hash"),
                f"{context}.executed_block_wire_hash",
            ),
        )


@dataclass(frozen=True)
class SumeragiV2QuorumCertificateRef:
    """Stable reference to a PrepareQC or CommitQC."""

    round: SumeragiV2ConsensusRound
    proposal_round: SumeragiV2ConsensusRound
    phase: SumeragiV2GlobalPhase
    subject: SumeragiV2BlockSubject
    execution_commitment: SumeragiV2ExecutionCommitment

    @classmethod
    def from_payload(cls, payload: Any, context: str) -> "SumeragiV2QuorumCertificateRef":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _sumeragi_v2_exact_fields(
            payload,
            (
                "round",
                "proposal_round",
                "phase",
                "subject",
                "execution_commitment",
            ),
            context,
        )
        phase = _sumeragi_v2_tagged_unit(
            payload.get("phase"),
            "phase",
            tuple(item.value for item in SumeragiV2GlobalPhase),
            f"{context}.phase",
        )
        return cls(
            round=SumeragiV2ConsensusRound.from_payload(
                payload.get("round"), f"{context}.round"
            ),
            proposal_round=SumeragiV2ConsensusRound.from_payload(
                payload.get("proposal_round"), f"{context}.proposal_round"
            ),
            phase=SumeragiV2GlobalPhase(phase),
            subject=SumeragiV2BlockSubject.from_payload(
                payload.get("subject"), f"{context}.subject"
            ),
            execution_commitment=SumeragiV2ExecutionCommitment.from_payload(
                payload.get("execution_commitment"),
                f"{context}.execution_commitment",
            ),
        )


@dataclass(frozen=True)
class SumeragiV2TimeoutCertificateRef:
    """Stable reference to the most recently installed timeout certificate."""

    round: SumeragiV2ConsensusRound
    highest_prepare_qc: Optional[SumeragiV2QuorumCertificateRef]
    certificate_hash: str

    @classmethod
    def from_payload(
        cls, payload: Any, context: str
    ) -> "SumeragiV2TimeoutCertificateRef":
        if not isinstance(payload, Mapping):
            raise TypeError(f"{context} must be an object")
        _sumeragi_v2_exact_fields(
            payload, ("round", "highest_prepare_qc", "certificate_hash"), context
        )
        highest_payload = payload.get("highest_prepare_qc")
        highest = (
            None
            if highest_payload is None
            else SumeragiV2QuorumCertificateRef.from_payload(
                highest_payload, f"{context}.highest_prepare_qc"
            )
        )
        if highest is not None and highest.phase is not SumeragiV2GlobalPhase.PREPARE:
            raise ValueError(f"{context}.highest_prepare_qc must reference a PrepareQC")
        return cls(
            round=SumeragiV2ConsensusRound.from_payload(
                payload.get("round"), f"{context}.round"
            ),
            highest_prepare_qc=highest,
            certificate_hash=_sumeragi_v2_string(
                payload.get("certificate_hash"), f"{context}.certificate_hash"
            ),
        )


@dataclass(frozen=True)
class SumeragiStatusSnapshot:
    """Authoritative protocol-v2 reducer snapshot from `/v1/sumeragi/status`."""

    protocol_version: int
    node_fingerprint: str
    build_fingerprint: str
    config_fingerprint: str
    restart_required: bool
    height_context_id: SumeragiV2HeightContextId
    height: int
    view: int
    phase: SumeragiV2StatusPhase
    leader: int
    locked_prepare_qc: Optional[SumeragiV2QuorumCertificateRef]
    highest_prepare_qc: Optional[SumeragiV2QuorumCertificateRef]
    last_timeout_certificate: Optional[SumeragiV2TimeoutCertificateRef]
    body_state: SumeragiV2BodyState
    pending_persistence_id: Optional[int]
    last_committed_height: int
    last_committed_subject: Optional[SumeragiV2BlockSubject]
    height_context: _CanonicalSumeragiV2HeightContextStatus
    last_commit_qc: Optional[_CanonicalSumeragiV2CommitQcStatus]
    liveness: SumeragiV2LivenessStatus

    @staticmethod
    def _subject_from_canonical(subject: Any) -> SumeragiV2BlockSubject:
        return SumeragiV2BlockSubject(
            parent_block_hash=subject.parent_block_hash,
            block_hash=subject.block_hash,
            payload_hash=subject.payload_hash,
        )

    @staticmethod
    def _execution_commitment_from_canonical(
        execution_commitment: Any,
    ) -> SumeragiV2ExecutionCommitment:
        return SumeragiV2ExecutionCommitment(
            parent_state_root=execution_commitment.parent_state_root,
            post_state_root=execution_commitment.post_state_root,
            ordinary_writes_root=execution_commitment.ordinary_writes_root,
            topup_anchor_root=execution_commitment.topup_anchor_root,
            topup_anchor_count=execution_commitment.topup_anchor_count,
            native_amx_application_manifest_version=(
                execution_commitment.native_amx_application_manifest_version
            ),
            native_amx_application_manifest_root=(
                execution_commitment.native_amx_application_manifest_root
            ),
            native_amx_application_manifest_count=(
                execution_commitment.native_amx_application_manifest_count
            ),
            executed_block_wire_hash=execution_commitment.executed_block_wire_hash,
        )

    @classmethod
    def _qc_from_canonical(cls, qc: Any) -> SumeragiV2QuorumCertificateRef:
        return SumeragiV2QuorumCertificateRef(
            round=SumeragiV2ConsensusRound(
                context_id=SumeragiV2HeightContextId(hash=qc.round.context_id[0]),
                height=qc.round.height,
                view=qc.round.view,
            ),
            proposal_round=SumeragiV2ConsensusRound(
                context_id=SumeragiV2HeightContextId(
                    hash=qc.proposal_round.context_id[0]
                ),
                height=qc.proposal_round.height,
                view=qc.proposal_round.view,
            ),
            phase=SumeragiV2GlobalPhase(qc.phase),
            subject=cls._subject_from_canonical(qc.subject),
            execution_commitment=cls._execution_commitment_from_canonical(
                qc.execution_commitment
            ),
        )

    @classmethod
    def _timeout_from_canonical(
        cls, timeout: Any
    ) -> SumeragiV2TimeoutCertificateRef:
        return SumeragiV2TimeoutCertificateRef(
            round=SumeragiV2ConsensusRound(
                context_id=SumeragiV2HeightContextId(
                    hash=timeout.round.context_id[0]
                ),
                height=timeout.round.height,
                view=timeout.round.view,
            ),
            highest_prepare_qc=(
                None
                if timeout.highest_prepare_qc is None
                else cls._qc_from_canonical(timeout.highest_prepare_qc)
            ),
            certificate_hash=timeout.certificate_hash,
        )

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiStatusSnapshot":
        canonical = _CanonicalSumeragiV2Status.from_payload(payload)
        return cls(
            protocol_version=canonical.protocol_version,
            node_fingerprint=canonical.node_fingerprint,
            build_fingerprint=canonical.build_fingerprint,
            config_fingerprint=canonical.config_fingerprint,
            restart_required=canonical.restart_required,
            height_context_id=SumeragiV2HeightContextId(
                hash=canonical.height_context_id[0]
            ),
            height=canonical.height,
            view=canonical.view,
            phase=SumeragiV2StatusPhase(canonical.phase),
            leader=canonical.leader,
            locked_prepare_qc=(
                None
                if canonical.locked_prepare_qc is None
                else cls._qc_from_canonical(canonical.locked_prepare_qc)
            ),
            highest_prepare_qc=(
                None
                if canonical.highest_prepare_qc is None
                else cls._qc_from_canonical(canonical.highest_prepare_qc)
            ),
            last_timeout_certificate=(
                None
                if canonical.last_timeout_certificate is None
                else cls._timeout_from_canonical(canonical.last_timeout_certificate)
            ),
            body_state=SumeragiV2BodyState(canonical.body_state),
            pending_persistence_id=canonical.pending_persistence_id,
            last_committed_height=canonical.last_committed_height,
            last_committed_subject=(
                None
                if canonical.last_committed_subject is None
                else cls._subject_from_canonical(canonical.last_committed_subject)
            ),
            height_context=canonical.height_context,
            last_commit_qc=canonical.last_commit_qc,
            liveness=canonical.liveness,
        )


@dataclass(frozen=True)
class SumeragiDiagnosticsSnapshot:
    """Bounded operator and lane evidence from `/v1/sumeragi/diagnostics`."""

    pipeline_execution: _CanonicalSumeragiPipelineExecutionStatus
    tx_queue_depth: int
    tx_queue_capacity: int
    tx_queue_retained_bytes: int
    tx_queue_max_retained_bytes: int
    tx_queue_saturated: bool
    tx_queue_saturated_by_count: bool
    tx_queue_saturated_by_bytes: bool
    tx_queue_saturated_by_age: bool
    tx_queue_oldest_queued_age_ms: int
    npos: Optional[_CanonicalSumeragiNposDiagnostics]
    lane_commitments: List[_CanonicalSumeragiLaneCommitmentStatus]
    dataspace_commitments: List[_CanonicalSumeragiDataspaceCommitmentStatus]
    lane_settlement_commitments: List[SumeragiLaneSettlementCommitment]
    lane_relay_envelopes: List[SumeragiLaneRelayEnvelope]
    lane_payload_ownerships: List[Dict[str, Any]]
    committed_lane_blocks: List[Dict[str, Any]]
    lane_block_sessions: List[Dict[str, Any]]
    lane_governance_sealed_total: int
    lane_governance_sealed_aliases: List[str]
    lane_governance: List[_CanonicalSumeragiLaneGovernanceStatus]
    native_amx_participant_applications: List[
        _CanonicalSumeragiNativeAmxParticipantApplication
    ]
    autonomous_lane_executions: List[_CanonicalSumeragiAutonomousLaneExecution]

    @classmethod
    def from_payload(
        cls, payload: Mapping[str, Any]
    ) -> "SumeragiDiagnosticsSnapshot":
        """Validate one diagnostics payload through the canonical parser."""

        canonical = _CanonicalSumeragiDiagnosticsStatus.from_payload(payload)
        return cls(
            pipeline_execution=canonical.pipeline_execution,
            tx_queue_depth=canonical.tx_queue_depth,
            tx_queue_capacity=canonical.tx_queue_capacity,
            tx_queue_retained_bytes=canonical.tx_queue_retained_bytes,
            tx_queue_max_retained_bytes=canonical.tx_queue_max_retained_bytes,
            tx_queue_saturated=canonical.tx_queue_saturated,
            tx_queue_saturated_by_count=canonical.tx_queue_saturated_by_count,
            tx_queue_saturated_by_bytes=canonical.tx_queue_saturated_by_bytes,
            tx_queue_saturated_by_age=canonical.tx_queue_saturated_by_age,
            tx_queue_oldest_queued_age_ms=canonical.tx_queue_oldest_queued_age_ms,
            npos=canonical.npos,
            lane_commitments=list(canonical.lane_commitments),
            dataspace_commitments=list(canonical.dataspace_commitments),
            lane_settlement_commitments=[
                SumeragiLaneSettlementCommitment.from_payload(entry)
                for entry in payload["lane_settlement_commitments"]
            ],
            lane_relay_envelopes=[
                SumeragiLaneRelayEnvelope.from_payload(entry)
                for entry in payload["lane_relay_envelopes"]
            ],
            lane_payload_ownerships=copy.deepcopy(
                canonical.lane_payload_ownerships
            ),
            committed_lane_blocks=copy.deepcopy(canonical.committed_lane_blocks),
            lane_block_sessions=copy.deepcopy(canonical.lane_block_sessions),
            lane_governance_sealed_total=canonical.lane_governance_sealed_total,
            lane_governance_sealed_aliases=list(
                canonical.lane_governance_sealed_aliases
            ),
            lane_governance=list(canonical.lane_governance),
            native_amx_participant_applications=list(
                canonical.native_amx_participant_applications
            ),
            autonomous_lane_executions=list(canonical.autonomous_lane_executions),
        )


@dataclass(frozen=True)
class SumeragiParamsSnapshot:
    """On-chain Sumeragi parameter snapshot from `/v1/sumeragi/params`."""

    block_time_ms: int
    commit_time_ms: int
    max_clock_drift_ms: int
    collectors_k: int
    redundant_send_r: int
    da_enabled: bool
    next_mode: Optional[str]
    mode_activation_height: Optional[int]
    chain_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiParamsSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi params payload must be an object")
        next_mode_value = payload.get("next_mode")
        if next_mode_value is not None and not isinstance(next_mode_value, str):
            raise TypeError("sumeragi params `next_mode` must be a string when present")
        mode_activation_value = payload.get("mode_activation_height")
        try:
            block_time_ms = int(payload.get("block_time_ms", 0))
            commit_time_ms = int(payload.get("commit_time_ms", 0))
            max_clock_drift_ms = int(payload.get("max_clock_drift_ms", 0))
            collectors_k = int(payload.get("collectors_k", 0))
            redundant_send_r = int(payload.get("redundant_send_r", 0))
            chain_height = int(payload.get("chain_height", 0))
            mode_activation_height = (
                None if mode_activation_value is None else int(mode_activation_value)
            )
        except (TypeError, ValueError) as exc:
            raise TypeError("sumeragi params numeric fields must be integers") from exc
        da_enabled = payload.get("da_enabled")
        if not isinstance(da_enabled, bool):
            raise TypeError("sumeragi params `da_enabled` must be a boolean")
        return cls(
            block_time_ms=block_time_ms,
            commit_time_ms=commit_time_ms,
            max_clock_drift_ms=max_clock_drift_ms,
            collectors_k=collectors_k,
            redundant_send_r=redundant_send_r,
            da_enabled=da_enabled,
            next_mode=next_mode_value,
            mode_activation_height=mode_activation_height,
            chain_height=chain_height,
        )


@dataclass(frozen=True)
class SumeragiPacemakerSnapshot:
    """Pacemaker runtime metrics from `/v1/sumeragi/pacemaker`."""

    backoff_ms: int
    rtt_floor_ms: int
    jitter_ms: int
    backoff_multiplier: int
    rtt_floor_multiplier: int
    max_backoff_ms: int
    jitter_frac_permille: int
    round_elapsed_ms: int
    view_timeout_target_ms: int
    view_timeout_remaining_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPacemakerSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("pacemaker payload must be an object")
        try:
            backoff_ms = int(payload.get("backoff_ms", 0))
            rtt_floor_ms = int(payload.get("rtt_floor_ms", 0))
            jitter_ms = int(payload.get("jitter_ms", 0))
            backoff_multiplier = int(payload.get("backoff_multiplier", 0))
            rtt_floor_multiplier = int(payload.get("rtt_floor_multiplier", 0))
            max_backoff_ms = int(payload.get("max_backoff_ms", 0))
            jitter_frac_permille = int(payload.get("jitter_frac_permille", 0))
            round_elapsed_ms = int(payload.get("round_elapsed_ms", 0))
            view_timeout_target_ms = int(payload.get("view_timeout_target_ms", 0))
            view_timeout_remaining_ms = int(payload.get("view_timeout_remaining_ms", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("pacemaker metrics must be numeric") from exc
        return cls(
            backoff_ms=backoff_ms,
            rtt_floor_ms=rtt_floor_ms,
            jitter_ms=jitter_ms,
            backoff_multiplier=backoff_multiplier,
            rtt_floor_multiplier=rtt_floor_multiplier,
            max_backoff_ms=max_backoff_ms,
            jitter_frac_permille=jitter_frac_permille,
            round_elapsed_ms=round_elapsed_ms,
            view_timeout_target_ms=view_timeout_target_ms,
            view_timeout_remaining_ms=view_timeout_remaining_ms,
        )


@dataclass(frozen=True)
class SumeragiPhasesEmaSnapshot:
    """Exponential moving average latency per phase."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    commit_ms: int
    pipeline_total_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPhasesEmaSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("phases EMA payload must be an object")
        try:
            return cls(
                propose_ms=int(payload.get("propose_ms", 0)),
                collect_da_ms=int(payload.get("collect_da_ms", 0)),
                collect_prevote_ms=int(payload.get("collect_prevote_ms", 0)),
                collect_precommit_ms=int(payload.get("collect_precommit_ms", 0)),
                collect_aggregator_ms=int(payload.get("collect_aggregator_ms", 0)),
                commit_ms=int(payload.get("commit_ms", 0)),
                pipeline_total_ms=int(payload.get("pipeline_total_ms", 0)),
            )
        except (TypeError, ValueError) as exc:
            raise TypeError("phases EMA metrics must be numeric") from exc


@dataclass(frozen=True)
class SumeragiPhasesSnapshot:
    """Per-phase latency summary from `/v1/sumeragi/phases`."""

    propose_ms: int
    collect_da_ms: int
    collect_prevote_ms: int
    collect_precommit_ms: int
    collect_aggregator_ms: int
    commit_ms: int
    pipeline_total_ms: int
    collect_aggregator_gossip_total: int
    block_created_dropped_by_lock_total: int
    block_created_hint_mismatch_total: int
    block_created_proposal_mismatch_total: int
    ema_ms: SumeragiPhasesEmaSnapshot

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiPhasesSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("phases payload must be an object")
        ema_payload = payload.get("ema_ms")
        if not isinstance(ema_payload, Mapping):
            raise TypeError("phases payload missing object `ema_ms` field")
        try:
            return cls(
                propose_ms=int(payload.get("propose_ms", 0)),
                collect_da_ms=int(payload.get("collect_da_ms", 0)),
                collect_prevote_ms=int(payload.get("collect_prevote_ms", 0)),
                collect_precommit_ms=int(payload.get("collect_precommit_ms", 0)),
                collect_aggregator_ms=int(payload.get("collect_aggregator_ms", 0)),
                commit_ms=int(payload.get("commit_ms", 0)),
                pipeline_total_ms=int(payload.get("pipeline_total_ms", 0)),
                collect_aggregator_gossip_total=int(
                    payload.get("collect_aggregator_gossip_total", 0)
                ),
                block_created_dropped_by_lock_total=int(
                    payload.get("block_created_dropped_by_lock_total", 0)
                ),
                block_created_hint_mismatch_total=int(
                    payload.get("block_created_hint_mismatch_total", 0)
                ),
                block_created_proposal_mismatch_total=int(
                    payload.get("block_created_proposal_mismatch_total", 0)
                ),
                ema_ms=SumeragiPhasesEmaSnapshot.from_payload(ema_payload),
            )
        except (TypeError, ValueError) as exc:
            raise TypeError("phases metrics must be numeric") from exc


@dataclass(frozen=True)
class SumeragiLeaderSnapshot:
    """Leader index snapshot from `/v1/sumeragi/leader`."""

    leader_index: int
    prf: SumeragiPrfStatus

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiLeaderSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("leader payload must be an object")
        prf_payload = payload.get("prf")
        if not isinstance(prf_payload, Mapping):
            raise TypeError("leader payload missing object `prf` field")
        try:
            leader_index = int(payload.get("leader_index", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("leader index must be numeric") from exc
        return cls(leader_index=leader_index, prf=SumeragiPrfStatus.from_payload(prf_payload))


@dataclass(frozen=True)
class SumeragiQcSnapshot:
    """Highest/Locked QC snapshot from `/v1/sumeragi/qc`."""

    highest_qc: SumeragiQcSummary
    locked_qc: SumeragiQcSummary

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiQcSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("qc payload must be an object")
        highest_payload = payload.get("highest_qc")
        locked_payload = payload.get("locked_qc")
        if not isinstance(highest_payload, Mapping) or not isinstance(locked_payload, Mapping):
            raise TypeError("qc payload must contain object `highest_qc` and `locked_qc` fields")
        return cls(
            highest_qc=SumeragiQcSummary.from_payload(highest_payload),
            locked_qc=SumeragiQcSummary.from_payload(locked_payload),
        )


@dataclass(frozen=True)
class SumeragiEvidenceCount:
    """Evidence store size from `/v1/sumeragi/evidence/count`."""

    count: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "SumeragiEvidenceCount":
        if not isinstance(payload, Mapping):
            raise TypeError("evidence count payload must be an object")
        try:
            count = int(payload.get("count", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("evidence count must be numeric") from exc
        return cls(count=count)


@dataclass(frozen=True)
class RuntimeUpgradeCounters:
    """Lifecycle counters returned by `/v1/runtime/metrics`."""

    proposed: int
    activated: int
    canceled: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeCounters":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade counters payload must be an object")
        try:
            proposed = int(payload.get("proposed", 0))
            activated = int(payload.get("activated", 0))
            canceled = int(payload.get("canceled", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrade counter values must be numeric") from exc
        return cls(proposed=proposed, activated=activated, canceled=canceled)


@dataclass(frozen=True)
class RuntimeMetrics:
    """Summary metrics for runtime upgrades."""

    abi_version: int
    upgrade_events_total: RuntimeUpgradeCounters

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeMetrics":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime metrics payload must be an object")
        try:
            abi_version = int(payload["abi_version"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("runtime metrics `abi_version` must be numeric") from exc
        counters_payload = payload.get("upgrade_events_total", {})
        counters = RuntimeUpgradeCounters.from_payload(
            counters_payload if isinstance(counters_payload, Mapping) else {}
        )
        return cls(abi_version=abi_version, upgrade_events_total=counters)


@dataclass(frozen=True)
class RuntimeAbiActive:
    """Active ABI version advertised by `/v1/runtime/abi/active`."""

    abi_version: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeAbiActive":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime ABI active payload must be an object")
        try:
            abi_version = int(payload["abi_version"])
        except (KeyError, TypeError, ValueError) as exc:
            raise TypeError("runtime ABI active missing numeric `abi_version` field") from exc
        return cls(abi_version=abi_version)


@dataclass(frozen=True)
class RuntimeAbiHash:
    """Canonical ABI hash summary from `/v1/runtime/abi/hash`."""

    policy: str
    abi_hash_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeAbiHash":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime ABI hash payload must be an object")
        policy = payload.get("policy")
        abi_hash_hex = payload.get("abi_hash_hex")
        if not isinstance(policy, str) or not isinstance(abi_hash_hex, str):
            raise TypeError("runtime ABI hash payload missing string `policy`/`abi_hash_hex` fields")
        return cls(policy=policy, abi_hash_hex=abi_hash_hex)


@dataclass(frozen=True)
class RuntimeUpgradeStatus:
    """Lifecycle status for a runtime upgrade record."""

    kind: str
    activated_height: Optional[int] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeStatus":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade status payload must be an object")
        if len(payload) != 1:
            raise TypeError("runtime upgrade status payload must contain exactly one variant")
        variant, value = next(iter(payload.items()))
        if variant == "Proposed":
            return cls(kind="Proposed")
        if variant == "Canceled":
            return cls(kind="Canceled")
        if variant == "ActivatedAt":
            if value is None:
                raise TypeError("runtime upgrade status `ActivatedAt` requires a height value")
            try:
                height = int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError("runtime upgrade status `ActivatedAt` height must be numeric") from exc
            if height < 0:
                raise ValueError("runtime upgrade status `ActivatedAt` height must be non-negative")
            return cls(kind="ActivatedAt", activated_height=height)
        raise TypeError(f"unknown runtime upgrade status variant `{variant}`")


def _coerce_int_list(values: Any, label: str) -> List[int]:
    if values is None:
        return []
    if not isinstance(values, list):
        raise TypeError(f"{label} must be a list")
    result: List[int] = []
    for entry in values:
        try:
            number = int(entry)
        except (TypeError, ValueError) as exc:
            raise TypeError(f"{label} entries must be integers") from exc
        if number < 0:
            raise ValueError(f"{label} entries must be non-negative")
        result.append(number)
    return result


def _validate_runtime_upgrade_manifest_fields(
    *,
    abi_version: int,
    added_syscalls: List[int],
    added_pointer_types: List[int],
    start_height: int,
    end_height: int,
) -> None:
    if abi_version != 1:
        raise ValueError("runtime upgrade manifest `abi_version` must be 1 in the first release")
    if added_syscalls:
        raise ValueError("runtime upgrade manifest `added_syscalls` must be empty in the first release")
    if added_pointer_types:
        raise ValueError(
            "runtime upgrade manifest `added_pointer_types` must be empty in the first release"
        )
    if end_height <= start_height:
        raise ValueError("runtime upgrade manifest `end_height` must be greater than `start_height`")


@dataclass(frozen=True)
class RuntimeUpgradeManifest:
    """Runtime upgrade manifest advertised by `/v1/runtime/upgrades`."""

    name: str
    description: str
    abi_version: int
    abi_hash_hex: str
    added_syscalls: List[int]
    added_pointer_types: List[int]
    start_height: int
    end_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeManifest":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade manifest payload must be an object")
        name = payload.get("name")
        description = payload.get("description")
        if not isinstance(name, str) or not isinstance(description, str):
            raise TypeError("runtime upgrade manifest requires string `name` and `description`")
        abi_hash_hex = payload.get("abi_hash")
        if not isinstance(abi_hash_hex, str):
            raise TypeError("runtime upgrade manifest missing string `abi_hash` field")
        abi_version_raw = payload.get("abi_version")
        start_height_raw = payload.get("start_height")
        end_height_raw = payload.get("end_height")
        if abi_version_raw is None or start_height_raw is None or end_height_raw is None:
            raise TypeError("runtime upgrade manifest missing numeric fields")
        try:
            abi_version = int(abi_version_raw)
            start_height = int(start_height_raw)
            end_height = int(end_height_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrade manifest numeric fields must be integers") from exc
        added_syscalls = _coerce_int_list(
            payload.get("added_syscalls", []), "runtime upgrade manifest `added_syscalls`"
        )
        added_pointer_types = _coerce_int_list(
            payload.get("added_pointer_types", []), "runtime upgrade manifest `added_pointer_types`"
        )
        _validate_runtime_upgrade_manifest_fields(
            abi_version=abi_version,
            added_syscalls=added_syscalls,
            added_pointer_types=added_pointer_types,
            start_height=start_height,
            end_height=end_height,
        )
        return cls(
            name=name,
            description=description,
            abi_version=abi_version,
            abi_hash_hex=abi_hash_hex,
            added_syscalls=added_syscalls,
            added_pointer_types=added_pointer_types,
            start_height=start_height,
            end_height=end_height,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Return a JSON-serialisable payload suitable for Torii POST requests."""

        _validate_runtime_upgrade_manifest_fields(
            abi_version=self.abi_version,
            added_syscalls=self.added_syscalls,
            added_pointer_types=self.added_pointer_types,
            start_height=self.start_height,
            end_height=self.end_height,
        )
        return {
            "name": self.name,
            "description": self.description,
            "abi_version": self.abi_version,
            "abi_hash": self.abi_hash_hex,
            "added_syscalls": list(self.added_syscalls),
            "added_pointer_types": list(self.added_pointer_types),
            "start_height": self.start_height,
            "end_height": self.end_height,
        }


@dataclass(frozen=True)
class RuntimeUpgradeRecord:
    """Individual runtime upgrade record maintained by the node."""

    manifest: RuntimeUpgradeManifest
    status: RuntimeUpgradeStatus
    proposer: str
    created_height: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade record payload must be an object")
        manifest_payload = payload.get("manifest")
        status_payload = payload.get("status")
        if not isinstance(manifest_payload, Mapping):
            raise TypeError("runtime upgrade record missing object `manifest` field")
        if not isinstance(status_payload, Mapping):
            raise TypeError("runtime upgrade record missing object `status` field")
        proposer = payload.get("proposer")
        if not isinstance(proposer, str):
            raise TypeError("runtime upgrade record missing string `proposer` field")
        created_height_raw = payload.get("created_height")
        if created_height_raw is None:
            raise TypeError("runtime upgrade record missing numeric `created_height` field")
        try:
            created_height = int(created_height_raw)
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrade record `created_height` must be numeric") from exc
        manifest = RuntimeUpgradeManifest.from_payload(manifest_payload)
        status = RuntimeUpgradeStatus.from_payload(status_payload)
        return cls(manifest=manifest, status=status, proposer=proposer, created_height=created_height)


@dataclass(frozen=True)
class RuntimeUpgradeListItem:
    """Entry returned from `/v1/runtime/upgrades`."""

    id_hex: str
    record: RuntimeUpgradeRecord

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeListItem":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade list entry must be an object")
        id_hex = payload.get("id_hex")
        if not isinstance(id_hex, str):
            raise TypeError("runtime upgrade list entry missing string `id_hex` field")
        record_payload = payload.get("record")
        if not isinstance(record_payload, Mapping):
            raise TypeError("runtime upgrade list entry missing object `record` field")
        record = RuntimeUpgradeRecord.from_payload(record_payload)
        return cls(id_hex=id_hex, record=record)


@dataclass(frozen=True)
class RuntimeUpgradeListPage:
    """Paginated runtime upgrade listing."""

    items: List[RuntimeUpgradeListItem]
    total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeListPage":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrades response must be an object")
        items_raw = payload.get("items", [])
        if items_raw is None:
            items_raw = []
        if not isinstance(items_raw, list):
            raise TypeError("runtime upgrades response `items` must be a list")
        try:
            total = int(payload.get("total", len(items_raw)))
        except (TypeError, ValueError) as exc:
            raise TypeError("runtime upgrades response `total` must be numeric") from exc
        items = [RuntimeUpgradeListItem.from_payload(entry) for entry in items_raw]
        return cls(items=items, total=total)


@dataclass(frozen=True)
class RuntimeInstruction:
    """Instruction emitted by runtime upgrade helpers."""

    wire_id: str
    payload_hex: str

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeInstruction":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime instruction payload must be an object")
        wire_id = payload.get("wire_id")
        payload_hex = payload.get("payload_hex")
        if not isinstance(wire_id, str) or not isinstance(payload_hex, str):
            raise TypeError("runtime instruction requires string `wire_id` and `payload_hex` fields")
        return cls(wire_id=wire_id, payload_hex=payload_hex)


@dataclass(frozen=True)
class RuntimeUpgradeActionResponse:
    """Response returned by runtime upgrade proposal/activation helpers."""

    ok: bool
    tx_instructions: List[RuntimeInstruction]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "RuntimeUpgradeActionResponse":
        if not isinstance(payload, Mapping):
            raise TypeError("runtime upgrade action response must be an object")
        ok_value = payload.get("ok")
        if not isinstance(ok_value, bool):
            raise TypeError("runtime upgrade action response missing boolean `ok` field")
        instructions_payload = payload.get("tx_instructions", [])
        if instructions_payload is None:
            instructions_payload = []
        if not isinstance(instructions_payload, list):
            raise TypeError("runtime upgrade action response `tx_instructions` must be a list")
        instructions = [RuntimeInstruction.from_payload(entry) for entry in instructions_payload]
        return cls(ok=ok_value, tx_instructions=instructions)


@dataclass(frozen=True)
class ConnectPerIpSessions:
    """Active session count for a single IP address."""

    ip: str
    sessions: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectPerIpSessions":
        if not isinstance(payload, Mapping):
            raise TypeError("per-ip sessions entry must be an object")
        ip = payload.get("ip")
        if not isinstance(ip, str):
            raise TypeError("per-ip sessions entry missing string `ip` field")
        try:
            sessions = int(payload.get("sessions", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("per-ip sessions entry `sessions` must be numeric") from exc
        return cls(ip=ip, sessions=sessions)


@dataclass(frozen=True)
class ConnectPolicyStatusSnapshot:
    """Policy limits surfaced by `/v1/connect/status`."""

    ws_max_sessions: int
    ws_per_ip_max_sessions: int
    ws_rate_per_ip_per_min: int
    session_ttl_ms: int
    frame_max_bytes: int
    session_buffer_max_bytes: int
    relay_enabled: bool
    relay_strategy: str
    relay_effective_strategy: str
    relay_p2p_attached: bool
    p2p_ttl_hops: int
    heartbeat_interval_ms: int
    heartbeat_miss_tolerance: int
    heartbeat_min_interval_ms: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectPolicyStatusSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("connect policy payload must be an object")
        try:
            ws_max_sessions = int(payload.get("ws_max_sessions", 0))
            ws_per_ip_max_sessions = int(payload.get("ws_per_ip_max_sessions", 0))
            ws_rate = int(payload.get("ws_rate_per_ip_per_min", 0))
            session_ttl_ms = int(payload.get("session_ttl_ms", 0))
            frame_max_bytes = int(payload.get("frame_max_bytes", 0))
            session_buffer_max_bytes = int(payload.get("session_buffer_max_bytes", 0))
            relay_enabled = bool(payload.get("relay_enabled", False))
            relay_strategy = str(payload.get("relay_strategy", ""))
            relay_effective_strategy = str(payload.get("relay_effective_strategy", ""))
            relay_p2p_attached = bool(payload.get("relay_p2p_attached", False))
            p2p_ttl_hops = int(payload.get("p2p_ttl_hops", 0))
            heartbeat_interval_ms = int(payload.get("heartbeat_interval_ms", 0))
            heartbeat_miss_tolerance = int(payload.get("heartbeat_miss_tolerance", 0))
            heartbeat_min_interval_ms = int(payload.get("heartbeat_min_interval_ms", 0))
        except (TypeError, ValueError) as exc:
            raise TypeError("connect policy fields have invalid types") from exc
        return cls(
            ws_max_sessions=ws_max_sessions,
            ws_per_ip_max_sessions=ws_per_ip_max_sessions,
            ws_rate_per_ip_per_min=ws_rate,
            session_ttl_ms=session_ttl_ms,
            frame_max_bytes=frame_max_bytes,
            session_buffer_max_bytes=session_buffer_max_bytes,
            relay_enabled=relay_enabled,
            relay_strategy=relay_strategy,
            relay_effective_strategy=relay_effective_strategy,
            relay_p2p_attached=relay_p2p_attached,
            p2p_ttl_hops=p2p_ttl_hops,
            heartbeat_interval_ms=heartbeat_interval_ms,
            heartbeat_miss_tolerance=heartbeat_miss_tolerance,
            heartbeat_min_interval_ms=heartbeat_min_interval_ms,
        )


@dataclass(frozen=True)
class ConnectStatusSnapshot:
    """Runtime status snapshot returned by `/v1/connect/status`."""

    enabled: bool
    sessions_total: int
    sessions_active: int
    per_ip_sessions: List[ConnectPerIpSessions]
    buffered_sessions: int
    total_buffer_bytes: int
    dedupe_size: int
    policy: Optional[ConnectPolicyStatusSnapshot]
    frames_in_total: int
    frames_out_total: int
    ciphertext_total: int
    dedupe_drops_total: int
    buffer_drops_total: int
    plaintext_control_drops_total: int
    monotonic_drops_total: int
    sequence_violation_closes_total: int
    role_direction_mismatch_total: int
    ping_miss_total: int
    p2p_rebroadcasts_total: int
    p2p_rebroadcast_skipped_total: int
    p2p_auth_failures_total: int
    p2p_ttl_drops_total: int
    p2p_unknown_session_drops_total: int
    p2p_session_claims_in_total: int
    p2p_session_claims_installed_total: int
    p2p_session_claim_conflicts_total: int
    p2p_role_consumed_total: int
    p2p_session_terminated_total: int

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectStatusSnapshot":
        if not isinstance(payload, Mapping):
            raise TypeError("connect status payload must be an object")
        enabled = bool(payload.get("enabled", False))
        def _coerce_int_field(name: str, default: int = 0) -> int:
            try:
                return int(payload.get(name, default))
            except (TypeError, ValueError) as exc:
                raise TypeError(f"connect status field `{name}` must be numeric") from exc

        per_ip_raw = payload.get("per_ip_sessions", [])
        if per_ip_raw is None:
            per_ip_raw = []
        if not isinstance(per_ip_raw, list):
            raise TypeError("connect status `per_ip_sessions` must be a list")
        per_ip = [ConnectPerIpSessions.from_payload(item) for item in per_ip_raw]
        policy_payload = payload.get("policy")
        policy = (
            ConnectPolicyStatusSnapshot.from_payload(policy_payload)
            if isinstance(policy_payload, Mapping)
            else None
        )
        return cls(
            enabled=enabled,
            sessions_total=_coerce_int_field("sessions_total"),
            sessions_active=_coerce_int_field("sessions_active"),
            per_ip_sessions=per_ip,
            buffered_sessions=_coerce_int_field("buffered_sessions"),
            total_buffer_bytes=_coerce_int_field("total_buffer_bytes"),
            dedupe_size=_coerce_int_field("dedupe_size"),
            policy=policy,
            frames_in_total=_coerce_int_field("frames_in_total"),
            frames_out_total=_coerce_int_field("frames_out_total"),
            ciphertext_total=_coerce_int_field("ciphertext_total"),
            dedupe_drops_total=_coerce_int_field("dedupe_drops_total"),
            buffer_drops_total=_coerce_int_field("buffer_drops_total"),
            plaintext_control_drops_total=_coerce_int_field("plaintext_control_drops_total"),
            monotonic_drops_total=_coerce_int_field("monotonic_drops_total"),
            sequence_violation_closes_total=_coerce_int_field("sequence_violation_closes_total"),
            role_direction_mismatch_total=_coerce_int_field("role_direction_mismatch_total"),
            ping_miss_total=_coerce_int_field("ping_miss_total"),
            p2p_rebroadcasts_total=_coerce_int_field("p2p_rebroadcasts_total"),
            p2p_rebroadcast_skipped_total=_coerce_int_field("p2p_rebroadcast_skipped_total"),
            p2p_auth_failures_total=_coerce_int_field("p2p_auth_failures_total"),
            p2p_ttl_drops_total=_coerce_int_field("p2p_ttl_drops_total"),
            p2p_unknown_session_drops_total=_coerce_int_field("p2p_unknown_session_drops_total"),
            p2p_session_claims_in_total=_coerce_int_field("p2p_session_claims_in_total"),
            p2p_session_claims_installed_total=_coerce_int_field("p2p_session_claims_installed_total"),
            p2p_session_claim_conflicts_total=_coerce_int_field("p2p_session_claim_conflicts_total"),
            p2p_role_consumed_total=_coerce_int_field("p2p_role_consumed_total"),
            p2p_session_terminated_total=_coerce_int_field("p2p_session_terminated_total"),
        )


@dataclass(frozen=True)
class ToriiStatusMetrics:
    """Derived metrics computed from consecutive `/v1/status` samples."""

    commit_latency_ms: int
    queue_size: int
    queue_queued: int
    queue_inflight: int
    queue_delta: int
    time_since_last_block_ms: int
    time_since_last_non_empty_block_ms: int
    da_reschedule_delta: int
    tx_approved_delta: int
    tx_rejected_delta: int
    view_change_delta: int

    @classmethod
    def from_samples(
        cls,
        previous: Optional["ToriiStatusPayload"],
        current: "ToriiStatusPayload",
    ) -> "ToriiStatusMetrics":
        if previous is None:
            return cls(
                commit_latency_ms=current.commit_time_ms,
                queue_size=current.queue_size,
                queue_queued=current.queue_queued,
                queue_inflight=current.queue_inflight,
                queue_delta=0,
                time_since_last_block_ms=current.time_since_last_block_ms,
                time_since_last_non_empty_block_ms=current.time_since_last_non_empty_block_ms,
                da_reschedule_delta=0,
                tx_approved_delta=0,
                tx_rejected_delta=0,
                view_change_delta=0,
            )
        return cls(
            commit_latency_ms=current.commit_time_ms,
            queue_size=current.queue_size,
            queue_queued=current.queue_queued,
            queue_inflight=current.queue_inflight,
            queue_delta=current.queue_size - previous.queue_size,
            time_since_last_block_ms=current.time_since_last_block_ms,
            time_since_last_non_empty_block_ms=current.time_since_last_non_empty_block_ms,
            da_reschedule_delta=max(
                0, current.da_reschedule_total - previous.da_reschedule_total
            ),
            tx_approved_delta=max(0, current.txs_approved - previous.txs_approved),
            tx_rejected_delta=max(0, current.txs_rejected - previous.txs_rejected),
            view_change_delta=max(0, current.view_changes - previous.view_changes),
        )

    @property
    def has_activity(self) -> bool:
        """Return ``True`` if the snapshot reflects any queue or transaction movement."""

        return any(
            value
            for value in (
                self.queue_delta,
                self.da_reschedule_delta,
                self.tx_approved_delta,
                self.tx_rejected_delta,
                self.view_change_delta,
            )
        )


@dataclass(frozen=True)
class GovernanceProposalSnapshot:
    proposed: int
    approved: int
    rejected: int
    enacted: int


@dataclass(frozen=True)
class GovernanceProtectedNamespaceSnapshot:
    total_checks: int
    allowed: int
    rejected: int


@dataclass(frozen=True)
class GovernanceManifestAdmissionSnapshot:
    total_checks: int
    allowed: int
    missing_manifest: int
    non_validator_authority: int
    quorum_rejected: int
    protected_namespace_rejected: int
    runtime_hook_rejected: int


@dataclass(frozen=True)
class GovernanceManifestQuorumSnapshot:
    total_checks: int
    satisfied: int
    rejected: int


@dataclass(frozen=True)
class GovernanceManifestActivationSnapshot:
    contract_address: str
    code_hash_hex: str
    abi_hash_hex: Optional[str]
    height: int
    activated_at_ms: int


@dataclass(frozen=True)
class GovernanceStatusSnapshot:
    proposals: GovernanceProposalSnapshot
    protected_namespace: GovernanceProtectedNamespaceSnapshot
    manifest_admission: GovernanceManifestAdmissionSnapshot
    manifest_quorum: GovernanceManifestQuorumSnapshot
    recent_manifest_activations: List[GovernanceManifestActivationSnapshot]


@dataclass(frozen=True)
class ToriiLaneCommitmentSnapshot:
    block_height: int
    lane_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash_hex: str


@dataclass(frozen=True)
class ToriiDataspaceCommitmentSnapshot:
    block_height: int
    lane_id: int
    dataspace_id: int
    tx_count: int
    total_chunks: int
    rbc_bytes_total: int
    teu_total: int
    block_hash_hex: str


@dataclass(frozen=True)
class ToriiLaneRuntimeUpgradeHookSnapshot:
    allow: bool
    require_metadata: bool
    metadata_key: Optional[str]
    allowed_ids: List[str]


@dataclass(frozen=True)
class ToriiLaneMerkleCommitmentSnapshot:
    root: str
    max_depth: int


@dataclass(frozen=True)
class ToriiLaneSnarkCommitmentSnapshot:
    circuit_id: int
    verifying_key_digest: str
    statement_hash: str
    proof_hash: str


@dataclass(frozen=True)
class ToriiLanePrivacyCommitmentSnapshot:
    id: int
    scheme: str
    merkle: Optional[ToriiLaneMerkleCommitmentSnapshot]
    snark: Optional[ToriiLaneSnarkCommitmentSnapshot]


@dataclass(frozen=True)
class ToriiLaneGovernanceSnapshot:
    lane_id: int
    alias: str
    dataspace_id: int
    visibility: str
    storage_profile: str
    governance: Optional[str]
    manifest_required: bool
    manifest_ready: bool
    manifest_path: Optional[str]
    validator_ids: List[str]
    quorum: Optional[int]
    protected_namespaces: List[str]
    runtime_upgrade: Optional[ToriiLaneRuntimeUpgradeHookSnapshot]
    privacy_commitments: List[ToriiLanePrivacyCommitmentSnapshot]


@dataclass(frozen=True)
class ToriiStatusPayload:
    """Decoded `/v1/status` payload with convenient integer accessors and lane summaries."""

    observed_at_ms: int
    peers: int
    queue_size: int
    queue_queued: int
    queue_inflight: int
    last_block_committed_at_ms: int
    last_non_empty_block_committed_at_ms: int
    time_since_last_block_ms: int
    time_since_last_non_empty_block_ms: int
    commit_time_ms: int
    da_reschedule_total: int
    txs_approved: int
    txs_rejected: int
    view_changes: int
    governance: Optional[GovernanceStatusSnapshot]
    lane_commitments: List[ToriiLaneCommitmentSnapshot]
    dataspace_commitments: List[ToriiDataspaceCommitmentSnapshot]
    lane_governance: List[ToriiLaneGovernanceSnapshot]
    lane_governance_sealed_total: int
    lane_governance_sealed_aliases: List[str]
    raw: Mapping[str, Any] = field(default_factory=dict)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ToriiStatusPayload":
        if not isinstance(payload, Mapping):
            raise TypeError("status payload must be an object")

        def _coerce_int(name: str) -> int:
            value = payload.get(name, 0)
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"status payload field `{name}` must be numeric") from exc

        def _coerce_nested_int(mapping: Mapping[str, Any], key: str, context: str) -> int:
            value = mapping.get(key, 0)
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"{context} `{key}` must be numeric") from exc

        def _coerce_string(value: Any, context: str) -> str:
            if isinstance(value, str):
                return value
            raise TypeError(f"{context} must be a string")

        def _coerce_optional_string(value: Any, context: str) -> Optional[str]:
            if value is None:
                return None
            if isinstance(value, str):
                return value
            raise TypeError(f"{context} must be a string when present")

        def _coerce_string_list(value: Any, context: str) -> List[str]:
            if value is None:
                return []
            if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
                raise TypeError(f"{context} must be an array of strings")
            result: List[str] = []
            for idx, item in enumerate(value):
                if not isinstance(item, str):
                    raise TypeError(f"{context}[{idx}] must be a string")
                result.append(item)
            return result

        def _parse_privacy_commitments(
            value: Any, context: str
        ) -> List[ToriiLanePrivacyCommitmentSnapshot]:
            if value is None:
                return []
            if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
                raise TypeError(f"{context} must be an array")
            commitments: List[ToriiLanePrivacyCommitmentSnapshot] = []
            for idx, item in enumerate(value):
                if not isinstance(item, Mapping):
                    raise TypeError(f"{context}[{idx}] must be an object")
                entry_context = f"{context}[{idx}]"
                commitment_id = _coerce_nested_int(item, "id", entry_context)
                scheme = _coerce_string(item.get("scheme"), f"{entry_context}.scheme")
                merkle_payload = item.get("merkle")
                snark_payload = item.get("snark")
                merkle: Optional[ToriiLaneMerkleCommitmentSnapshot] = None
                snark: Optional[ToriiLaneSnarkCommitmentSnapshot] = None
                if scheme == "merkle":
                    if not isinstance(merkle_payload, Mapping):
                        raise TypeError(f"{entry_context}.merkle must be an object")
                    merkle = ToriiLaneMerkleCommitmentSnapshot(
                        root=_coerce_string(
                            merkle_payload.get("root"),
                            f"{entry_context}.merkle.root",
                        ),
                        max_depth=_coerce_nested_int(
                            merkle_payload,
                            "max_depth",
                            f"{entry_context}.merkle",
                        ),
                    )
                elif scheme == "snark":
                    if not isinstance(snark_payload, Mapping):
                        raise TypeError(f"{entry_context}.snark must be an object")
                    snark = ToriiLaneSnarkCommitmentSnapshot(
                        circuit_id=_coerce_nested_int(
                            snark_payload,
                            "circuit_id",
                            f"{entry_context}.snark",
                        ),
                        verifying_key_digest=_coerce_string(
                            snark_payload.get("verifying_key_digest"),
                            f"{entry_context}.snark.verifying_key_digest",
                        ),
                        statement_hash=_coerce_string(
                            snark_payload.get("statement_hash"),
                            f"{entry_context}.snark.statement_hash",
                        ),
                        proof_hash=_coerce_string(
                            snark_payload.get("proof_hash"),
                            f"{entry_context}.snark.proof_hash",
                        ),
                    )
                else:
                    raise ValueError(f"{entry_context}.scheme must be 'merkle' or 'snark'")
                commitments.append(
                    ToriiLanePrivacyCommitmentSnapshot(
                        id=commitment_id,
                        scheme=scheme,
                        merkle=merkle,
                        snark=snark,
                    )
                )
            return commitments

        def _coerce_bool(value: Any, context: str) -> bool:
            if isinstance(value, bool):
                return value
            raise TypeError(f"{context} must be a boolean")

        def _coerce_optional_int(value: Any, context: str) -> Optional[int]:
            if value is None:
                return None
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"{context} must be numeric when present") from exc

        governance_snapshot: Optional[GovernanceStatusSnapshot] = None
        governance_payload = payload.get("governance")
        if isinstance(governance_payload, Mapping):
            proposals_payload = governance_payload.get("proposals")
            protected_payload = governance_payload.get("protected_namespace")
            admission_payload = governance_payload.get("manifest_admission")
            quorum_payload = governance_payload.get("manifest_quorum")
            activations_payload = governance_payload.get("recent_manifest_activations")

            if not isinstance(proposals_payload, Mapping):
                raise TypeError("governance payload missing object `proposals` field")
            if not isinstance(protected_payload, Mapping):
                raise TypeError("governance payload missing object `protected_namespace` field")
            if not isinstance(admission_payload, Mapping):
                raise TypeError("governance payload missing object `manifest_admission` field")
            if not isinstance(quorum_payload, Mapping):
                raise TypeError("governance payload missing object `manifest_quorum` field")
            if activations_payload is None:
                activations_payload = []
            if not isinstance(activations_payload, Sequence):
                raise TypeError("governance payload `recent_manifest_activations` must be an array")

            proposals = GovernanceProposalSnapshot(
                proposed=_coerce_nested_int(proposals_payload, "proposed", "governance.proposals"),
                approved=_coerce_nested_int(proposals_payload, "approved", "governance.proposals"),
                rejected=_coerce_nested_int(proposals_payload, "rejected", "governance.proposals"),
                enacted=_coerce_nested_int(proposals_payload, "enacted", "governance.proposals"),
            )
            protected = GovernanceProtectedNamespaceSnapshot(
                total_checks=_coerce_nested_int(
                    protected_payload, "total_checks", "governance.protected_namespace"
                ),
                allowed=_coerce_nested_int(
                    protected_payload, "allowed", "governance.protected_namespace"
                ),
                rejected=_coerce_nested_int(
                    protected_payload, "rejected", "governance.protected_namespace"
                ),
            )
            admission = GovernanceManifestAdmissionSnapshot(
                total_checks=_coerce_nested_int(
                    admission_payload, "total_checks", "governance.manifest_admission"
                ),
                allowed=_coerce_nested_int(
                    admission_payload, "allowed", "governance.manifest_admission"
                ),
                missing_manifest=_coerce_nested_int(
                    admission_payload, "missing_manifest", "governance.manifest_admission"
                ),
                non_validator_authority=_coerce_nested_int(
                    admission_payload,
                    "non_validator_authority",
                    "governance.manifest_admission",
                ),
                quorum_rejected=_coerce_nested_int(
                    admission_payload, "quorum_rejected", "governance.manifest_admission"
                ),
                protected_namespace_rejected=_coerce_nested_int(
                    admission_payload,
                    "protected_namespace_rejected",
                    "governance.manifest_admission",
                ),
                runtime_hook_rejected=_coerce_nested_int(
                    admission_payload, "runtime_hook_rejected", "governance.manifest_admission"
                ),
            )
            quorum = GovernanceManifestQuorumSnapshot(
                total_checks=_coerce_nested_int(
                    quorum_payload, "total_checks", "governance.manifest_quorum"
                ),
                satisfied=_coerce_nested_int(
                    quorum_payload, "satisfied", "governance.manifest_quorum"
                ),
                rejected=_coerce_nested_int(
                    quorum_payload, "rejected", "governance.manifest_quorum"
                ),
            )

            recent_activations: List[GovernanceManifestActivationSnapshot] = []
            for idx, item in enumerate(activations_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(
                        f"governance manifest activation at index {idx} must be an object"
                    )
                contract_address = item.get("contract_address", "")
                code_hash = item.get("code_hash_hex", "")
                abi_hash = item.get("abi_hash_hex")
                try:
                    height = int(item.get("height", 0))
                    activated_at_ms = int(item.get("activated_at_ms", 0))
                except (TypeError, ValueError) as exc:
                    raise TypeError(
                        "governance manifest activation height/activated_at_ms must be numeric"
                    ) from exc
                recent_activations.append(
                    GovernanceManifestActivationSnapshot(
                        contract_address=str(contract_address),
                        code_hash_hex=str(code_hash),
                        abi_hash_hex=str(abi_hash) if abi_hash is not None else None,
                        height=height,
                        activated_at_ms=activated_at_ms,
                    )
                )

            governance_snapshot = GovernanceStatusSnapshot(
                proposals=proposals,
                protected_namespace=protected,
                manifest_admission=admission,
                manifest_quorum=quorum,
                recent_manifest_activations=recent_activations,
            )

        lane_commitments_payload = payload.get("lane_commitments")
        lane_commitments: List[ToriiLaneCommitmentSnapshot] = []
        if lane_commitments_payload:
            if not isinstance(lane_commitments_payload, Sequence):
                raise TypeError("lane_commitments must be an array")
            for idx, item in enumerate(lane_commitments_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(f"lane_commitments[{idx}] must be an object")
                lane_commitments.append(
                    ToriiLaneCommitmentSnapshot(
                        block_height=_coerce_nested_int(item, "block_height", f"lane_commitments[{idx}]"),
                        lane_id=_coerce_nested_int(item, "lane_id", f"lane_commitments[{idx}]"),
                        tx_count=_coerce_nested_int(item, "tx_count", f"lane_commitments[{idx}]"),
                        total_chunks=_coerce_nested_int(item, "total_chunks", f"lane_commitments[{idx}]"),
                        rbc_bytes_total=_coerce_nested_int(item, "rbc_bytes_total", f"lane_commitments[{idx}]"),
                        teu_total=_coerce_nested_int(item, "teu_total", f"lane_commitments[{idx}]"),
                        block_hash_hex=_coerce_string(item.get("block_hash"), f"lane_commitments[{idx}].block_hash"),
                    )
                )

        dataspace_commitments_payload = payload.get("dataspace_commitments")
        dataspace_commitments: List[ToriiDataspaceCommitmentSnapshot] = []
        if dataspace_commitments_payload:
            if not isinstance(dataspace_commitments_payload, Sequence):
                raise TypeError("dataspace_commitments must be an array")
            for idx, item in enumerate(dataspace_commitments_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(f"dataspace_commitments[{idx}] must be an object")
                dataspace_commitments.append(
                    ToriiDataspaceCommitmentSnapshot(
                        block_height=_coerce_nested_int(item, "block_height", f"dataspace_commitments[{idx}]"),
                        lane_id=_coerce_nested_int(item, "lane_id", f"dataspace_commitments[{idx}]"),
                        dataspace_id=_coerce_nested_int(item, "dataspace_id", f"dataspace_commitments[{idx}]"),
                        tx_count=_coerce_nested_int(item, "tx_count", f"dataspace_commitments[{idx}]"),
                        total_chunks=_coerce_nested_int(item, "total_chunks", f"dataspace_commitments[{idx}]"),
                        rbc_bytes_total=_coerce_nested_int(item, "rbc_bytes_total", f"dataspace_commitments[{idx}]"),
                        teu_total=_coerce_nested_int(item, "teu_total", f"dataspace_commitments[{idx}]"),
                        block_hash_hex=_coerce_string(item.get("block_hash"), f"dataspace_commitments[{idx}].block_hash"),
                    )
                )

        lane_governance_payload = payload.get("lane_governance")
        lane_governance: List[ToriiLaneGovernanceSnapshot] = []
        if lane_governance_payload:
            if not isinstance(lane_governance_payload, Sequence):
                raise TypeError("lane_governance must be an array")
            for idx, item in enumerate(lane_governance_payload):
                if not isinstance(item, Mapping):
                    raise TypeError(f"lane_governance[{idx}] must be an object")
                validator_ids = _coerce_string_list(
                    item.get("validator_ids"),
                    f"lane_governance[{idx}].validator_ids",
                )
                namespaces = _coerce_string_list(
                    item.get("protected_namespaces"),
                    f"lane_governance[{idx}].protected_namespaces",
                )
                runtime_payload = item.get("runtime_upgrade")
                runtime_upgrade = None
                if runtime_payload is not None:
                    if not isinstance(runtime_payload, Mapping):
                        raise TypeError(f"lane_governance[{idx}].runtime_upgrade must be an object")
                    runtime_upgrade = ToriiLaneRuntimeUpgradeHookSnapshot(
                        allow=_coerce_bool(
                            runtime_payload.get("allow"),
                            f"lane_governance[{idx}].runtime_upgrade.allow",
                        ),
                        require_metadata=_coerce_bool(
                            runtime_payload.get("require_metadata"),
                            f"lane_governance[{idx}].runtime_upgrade.require_metadata",
                        ),
                        metadata_key=_coerce_optional_string(
                            runtime_payload.get("metadata_key"),
                            f"lane_governance[{idx}].runtime_upgrade.metadata_key",
                        ),
                        allowed_ids=_coerce_string_list(
                            runtime_payload.get("allowed_ids"),
                            f"lane_governance[{idx}].runtime_upgrade.allowed_ids",
                        ),
                    )
                privacy_commitments = _parse_privacy_commitments(
                    item.get("privacy_commitments"),
                    f"lane_governance[{idx}].privacy_commitments",
                )
                lane_governance.append(
                    ToriiLaneGovernanceSnapshot(
                        lane_id=_coerce_nested_int(item, "lane_id", f"lane_governance[{idx}]"),
                        alias=_coerce_string(item.get("alias"), f"lane_governance[{idx}].alias"),
                        dataspace_id=_coerce_nested_int(
                            item,
                            "dataspace_id",
                            f"lane_governance[{idx}]",
                        ),
                        visibility=_coerce_string(
                            item.get("visibility"),
                            f"lane_governance[{idx}].visibility",
                        ),
                        storage_profile=_coerce_string(
                            item.get("storage_profile"),
                            f"lane_governance[{idx}].storage_profile",
                        ),
                        governance=_coerce_optional_string(
                            item.get("governance"),
                            f"lane_governance[{idx}].governance",
                        ),
                        manifest_required=_coerce_bool(
                            item.get("manifest_required"),
                            f"lane_governance[{idx}].manifest_required",
                        ),
                        manifest_ready=_coerce_bool(
                            item.get("manifest_ready"),
                            f"lane_governance[{idx}].manifest_ready",
                        ),
                        manifest_path=_coerce_optional_string(
                            item.get("manifest_path"),
                            f"lane_governance[{idx}].manifest_path",
                        ),
                        validator_ids=validator_ids,
                        quorum=_coerce_optional_int(
                            item.get("quorum"),
                            f"lane_governance[{idx}].quorum",
                        ),
                        protected_namespaces=namespaces,
                        runtime_upgrade=runtime_upgrade,
                        privacy_commitments=privacy_commitments,
                    )
                )

        lane_governance_sealed_total = _coerce_int("lane_governance_sealed_total")
        lane_governance_sealed_aliases = _coerce_string_list(
            payload.get("lane_governance_sealed_aliases"),
            "lane_governance_sealed_aliases",
        )

        return cls(
            observed_at_ms=_coerce_int("observed_at_ms"),
            peers=_coerce_int("peers"),
            queue_size=_coerce_int("queue_size"),
            queue_queued=_coerce_int("queue_queued"),
            queue_inflight=_coerce_int("queue_inflight"),
            last_block_committed_at_ms=_coerce_int("last_block_committed_at_ms"),
            last_non_empty_block_committed_at_ms=_coerce_int(
                "last_non_empty_block_committed_at_ms"
            ),
            time_since_last_block_ms=_coerce_int("time_since_last_block_ms"),
            time_since_last_non_empty_block_ms=_coerce_int(
                "time_since_last_non_empty_block_ms"
            ),
            commit_time_ms=_coerce_int("commit_time_ms"),
            da_reschedule_total=_coerce_int("da_reschedule_total"),
            txs_approved=_coerce_int("txs_approved"),
            txs_rejected=_coerce_int("txs_rejected"),
            view_changes=_coerce_int("view_changes"),
            governance=governance_snapshot,
            lane_commitments=lane_commitments,
            dataspace_commitments=dataspace_commitments,
            lane_governance=lane_governance,
            lane_governance_sealed_total=lane_governance_sealed_total,
            lane_governance_sealed_aliases=lane_governance_sealed_aliases,
            raw=dict(payload),
        )

    @property
    def liveness_elapsed_ms(self) -> int:
        """Return elapsed block time used for queue-aware stall checks."""

        if self.time_since_last_non_empty_block_ms > 0:
            return self.time_since_last_non_empty_block_ms
        return self.time_since_last_block_ms

    def is_queue_stalled(self, stall_threshold_ms: int) -> bool:
        """Classify stalls only when queued work exists and elapsed block time exceeds the threshold."""

        return self.queue_size > 0 and self.liveness_elapsed_ms > int(stall_threshold_ms)


@dataclass(frozen=True)
class ToriiStatusSnapshot:
    """Snapshot captured from `/v1/status` together with derived metrics."""

    timestamp: float
    status: ToriiStatusPayload
    metrics: ToriiStatusMetrics

    @property
    def has_activity(self) -> bool:
        """Return ``True`` when the underlying metrics observed any movement."""

        return self.metrics.has_activity


@dataclass(frozen=True)
class ToriiPipelinePreflight:
    """Typed response from `GET /v1/pipeline/preflight`."""

    schema_version: int
    chain_height: int
    sumeragi: Mapping[str, Any]
    admission: Mapping[str, Any]
    block: Mapping[str, Any]
    pipeline: Mapping[str, Any]
    queue: Mapping[str, Any]
    fees: Mapping[str, Any]
    raw: Mapping[str, Any] = field(default_factory=dict)

    @property
    def stall_threshold_ms(self) -> int:
        return int(self.sumeragi.get("stall_threshold_ms", 0))

    def is_status_stalled(self, status: ToriiStatusPayload) -> bool:
        return status.is_queue_stalled(self.stall_threshold_ms)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ToriiPipelinePreflight":
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline preflight response must be a JSON object")

        def _mapping(name: str) -> Mapping[str, Any]:
            value = payload.get(name)
            if not isinstance(value, Mapping):
                raise TypeError(f"pipeline preflight `{name}` must be a JSON object")
            return dict(value)

        return cls(
            schema_version=int(payload.get("schema_version", 0)),
            chain_height=int(payload.get("chain_height", 0)),
            sumeragi=_mapping("sumeragi"),
            admission=_mapping("admission"),
            block=_mapping("block"),
            pipeline=_mapping("pipeline"),
            queue=_mapping("queue"),
            fees=_mapping("fees"),
            raw=dict(payload),
        )


class _ToriiStatusState:
    """Internal helper tracking the previous status sample per client."""

    def __init__(self) -> None:
        self._previous: Optional[ToriiStatusPayload] = None

    def record(self, payload: ToriiStatusPayload) -> ToriiStatusMetrics:
        metrics = ToriiStatusMetrics.from_samples(self._previous, payload)
        self._previous = payload
        return metrics


_TORII_ENV_KEYS = {
    "timeout_ms": "IROHA_TORII_TIMEOUT_MS",
    "max_retries": "IROHA_TORII_MAX_RETRIES",
    "backoff_initial_ms": "IROHA_TORII_BACKOFF_INITIAL_MS",
    "backoff_multiplier": "IROHA_TORII_BACKOFF_MULTIPLIER",
    "max_backoff_ms": "IROHA_TORII_MAX_BACKOFF_MS",
    "retry_statuses": "IROHA_TORII_RETRY_STATUSES",
    "retry_methods": "IROHA_TORII_RETRY_METHODS",
    "api_token": "IROHA_TORII_API_TOKEN",
    "auth_token": "IROHA_TORII_AUTH_TOKEN",
}


def _coerce_sorafs_policy_value(value: Any, context: str) -> SorafsAliasPolicy:
    if isinstance(value, SorafsAliasPolicy):
        return value
    if isinstance(value, Mapping):
        return SorafsAliasPolicy.from_mapping(value)
    raise TypeError(f"{context} must be provided as a mapping or SorafsAliasPolicy instance")


def _normalize_sorafs_policy_config(
    policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]]
) -> SorafsAliasPolicy:
    if policy is None:
        return SorafsAliasPolicy.defaults()
    return _coerce_sorafs_policy_value(policy, "sorafs_alias_policy")


def resolve_torii_client_config(
    *,
    config: Optional[Mapping[str, Any]] = None,
    env: Optional[Mapping[str, str]] = None,
    overrides: Optional[Mapping[str, Any]] = None,
) -> ResolvedToriiClientConfig:
    """Merge Torii client settings from config files, environment variables, and overrides."""

    state: Dict[str, Any] = {
        "timeout": _DEFAULT_RESOLVED_CONFIG.timeout,
        "max_retries": _DEFAULT_RESOLVED_CONFIG.max_retries,
        "backoff_initial": _DEFAULT_RESOLVED_CONFIG.backoff_initial,
        "backoff_multiplier": _DEFAULT_RESOLVED_CONFIG.backoff_multiplier,
        "max_backoff": _DEFAULT_RESOLVED_CONFIG.max_backoff,
        "retry_statuses": set(_DEFAULT_RESOLVED_CONFIG.retry_statuses),
        "retry_methods": set(_DEFAULT_RESOLVED_CONFIG.retry_methods),
        "default_headers": dict(_DEFAULT_RESOLVED_CONFIG.default_headers),
        "auth_token": _DEFAULT_RESOLVED_CONFIG.auth_token,
        "api_token": _DEFAULT_RESOLVED_CONFIG.api_token,
        "sorafs_alias_policy": None,
    }

    def apply_source(source: Optional[Mapping[str, Any]]) -> None:
        if not source:
            return
        _reject_alias_keys(
            source,
            {
                "timeoutMs": "timeout_ms",
                "timeoutSeconds": "timeout",
                "maxRetries": "max_retries",
                "backoffInitialMs": "backoff_initial_ms",
                "backoffInitial": "backoff_initial",
                "backoffMultiplier": "backoff_multiplier",
                "maxBackoffMs": "max_backoff_ms",
                "maxBackoff": "max_backoff",
                "retryStatuses": "retry_statuses",
                "retryMethods": "retry_methods",
                "defaultHeaders": "default_headers",
                "authToken": "auth_token",
                "apiToken": "api_token",
                "sorafsAliasPolicy": "sorafs_alias_policy",
            },
            context="torii_client config",
        )
        timeout = _coerce_timeout_seconds(
            source.get("timeout_ms"),
            default_value=source.get("timeout"),
        )
        if timeout is not None:
            state["timeout"] = timeout
        max_retries = _coerce_int(
            source.get("max_retries"),
            "max_retries",
            allow_zero=True,
        )
        if max_retries is not None:
            state["max_retries"] = max_retries
        backoff_initial = _coerce_duration_seconds(
            source.get("backoff_initial_ms"),
            default_value=source.get("backoff_initial"),
        )
        if backoff_initial is not None:
            state["backoff_initial"] = backoff_initial
        backoff_multiplier = _coerce_float(
            source.get("backoff_multiplier"),
            "backoff_multiplier",
            allow_zero=False,
        )
        if backoff_multiplier is not None:
            state["backoff_multiplier"] = max(backoff_multiplier, 1.0)
        max_backoff = _coerce_duration_seconds(
            source.get("max_backoff_ms"),
            default_value=source.get("max_backoff"),
        )
        if max_backoff is not None:
            state["max_backoff"] = max_backoff
        statuses = _parse_retry_statuses(
            source.get("retry_statuses")
        )
        if statuses is not None:
            state["retry_statuses"] = statuses
        methods = _parse_retry_methods(
            source.get("retry_methods")
        )
        if methods is not None:
            state["retry_methods"] = methods
        headers = _normalize_headers(source.get("default_headers"))
        if headers:
            state["default_headers"].update(headers)
        auth_token = source.get("auth_token")
        if auth_token is not None:
            state["auth_token"] = str(auth_token)
        api_token = source.get("api_token")
        if api_token is not None:
            state["api_token"] = str(api_token)
        policy_override = source.get("sorafs_alias_policy")
        if policy_override is not None:
            state["sorafs_alias_policy"] = _coerce_sorafs_policy_value(
                policy_override, "sorafs_alias_policy"
            )

    apply_source(_extract_torii_client_section(config))

    if isinstance(config, Mapping):
        if "toriiConfig" in config:
            raise TypeError("toriiConfig is not supported; use torii")
        torii_section = config.get("torii")
        token = _pick_api_token(torii_section)
        if token and not state["api_token"]:
            state["api_token"] = token

    env_vars = os.environ if env is None else env
    apply_source(
        {
            "timeout_ms": env_vars.get(_TORII_ENV_KEYS["timeout_ms"]),
            "max_retries": env_vars.get(_TORII_ENV_KEYS["max_retries"]),
            "backoff_initial_ms": env_vars.get(_TORII_ENV_KEYS["backoff_initial_ms"]),
            "backoff_multiplier": env_vars.get(_TORII_ENV_KEYS["backoff_multiplier"]),
            "max_backoff_ms": env_vars.get(_TORII_ENV_KEYS["max_backoff_ms"]),
            "retry_statuses": env_vars.get(_TORII_ENV_KEYS["retry_statuses"]),
            "retry_methods": env_vars.get(_TORII_ENV_KEYS["retry_methods"]),
            "api_token": env_vars.get(_TORII_ENV_KEYS["api_token"]),
            "auth_token": env_vars.get(_TORII_ENV_KEYS["auth_token"]),
        }
    )

    apply_source(overrides)

    headers = dict(state["default_headers"])
    if not any(key.lower() == "accept" for key in headers):
        headers["Accept"] = "application/json"

    max_backoff = state["max_backoff"]
    if max_backoff <= 0:
        max_backoff = math.inf

    return ResolvedToriiClientConfig(
        timeout=max(state["timeout"], 0.0),
        max_retries=max(0, int(state["max_retries"])),
        backoff_initial=max(state["backoff_initial"], 0.0),
        backoff_multiplier=max(state["backoff_multiplier"], 1.0),
        max_backoff=max_backoff,
        retry_statuses=frozenset(int(code) for code in state["retry_statuses"]),
        retry_methods=frozenset(method.upper() for method in state["retry_methods"]),
        default_headers=headers,
        auth_token=state["auth_token"],
        api_token=state["api_token"],
        sorafs_alias_policy=_normalize_sorafs_policy_config(
            state["sorafs_alias_policy"]
        ),
    )


def _extract_torii_client_section(config: Optional[Mapping[str, Any]]) -> Mapping[str, Any]:
    if not config:
        return {}
    if isinstance(config, Mapping):
        if "toriiClient" in config:
            raise TypeError("toriiClient is not supported; use torii_client")
        nested = config.get("torii_client")
        if isinstance(nested, Mapping):
            return nested
    return config


def _extract_pipeline_status_kind(payload: Any) -> Optional[str]:
    """Return the pipeline status `kind` from a Torii response, if present."""

    if not isinstance(payload, Mapping):
        return None

    def coerce_status(status_obj: Any) -> Optional[str]:
        if isinstance(status_obj, Mapping):
            kind = status_obj.get("kind")
            if kind is not None:
                return str(kind)
        elif status_obj is not None:
            return str(status_obj)
        return None

    status = coerce_status(payload.get("status"))
    if status is not None:
        return status

    content = payload.get("content")
    if isinstance(content, Mapping):
        return coerce_status(content.get("status"))

    return None


def _pick_api_token(torii_section: Optional[Mapping[str, Any]]) -> Optional[str]:
    if not isinstance(torii_section, Mapping):
        return None
    if "apiTokens" in torii_section:
        raise TypeError("apiTokens is not supported; use api_tokens")
    tokens = torii_section.get("api_tokens")
    if isinstance(tokens, (list, tuple)) and tokens:
        return str(tokens[0])
    if isinstance(tokens, str):
        return tokens
    return None


def _coerce_int(value: Any, name: str, *, allow_zero: bool = False) -> Optional[int]:
    if value is None or value == "":
        return None
    try:
        number = int(value)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be an integer") from None
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be {'non-negative' if allow_zero else 'positive'}")
    return number


def _coerce_float(value: Any, name: str, *, allow_zero: bool = False) -> Optional[float]:
    if value is None or value == "":
        return None
    try:
        number = float(value)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be numeric") from None
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be greater than 0")
    return number


def _coerce_duration_seconds(value: Any, *, default_value: Any = None) -> Optional[float]:
    millis = _coerce_float(value, "duration_ms", allow_zero=True)
    if millis is not None:
        return millis / 1000.0
    seconds = _coerce_float(default_value, "duration", allow_zero=True)
    return seconds


def _coerce_timeout_seconds(value: Any, *, default_value: Any = None) -> Optional[float]:
    result = _coerce_duration_seconds(value)
    if result is not None:
        return result
    seconds = _coerce_float(default_value, "timeout", allow_zero=True)
    return seconds


def _parse_retry_statuses(value: Any) -> Optional[set[int]]:
    if value is None or value == "":
        return None
    statuses: set[int] = set()
    parts: Iterable[Any]
    if isinstance(value, str):
        parts = [part.strip() for part in value.split(",") if part.strip()]
    elif isinstance(value, (list, tuple, set)):
        parts = value
    else:
        parts = [value]
    for entry in parts:
        statuses.add(int(entry))
    return statuses


def _parse_retry_methods(value: Any) -> Optional[set[str]]:
    if value is None or value == "":
        return None
    methods: set[str] = set()
    entries: Iterable[Any]
    if isinstance(value, str):
        entries = [part.strip() for part in value.split(",") if part.strip()]
    elif isinstance(value, (list, tuple, set)):
        entries = value
    else:
        entries = [value]
    for entry in entries:
        methods.add(str(entry).upper())
    return methods


def _normalize_headers(headers: Any) -> Dict[str, str]:
    normalized: Dict[str, str] = {}
    if isinstance(headers, Mapping):
        for key, value in headers.items():
            if value is not None:
                normalized[str(key)] = str(value)
    return normalized


def _require_crypto() -> ModuleType:
    """Return the compiled crypto bindings, raising a helpful error when missing."""

    global _CRYPTO_MODULE
    if _CRYPTO_MODULE is not None:
        return _CRYPTO_MODULE
    try:
        from . import crypto as _crypto
    except RuntimeError as exc:  # pragma: no cover - optional runtime dependency
        raise RuntimeError(
            "iroha_python._crypto extension module is required for transaction helpers. "
            "Run `maturin develop --release` inside `python/iroha_python` (or install the wheel) "
            "before using these APIs."
        ) from exc
    _CRYPTO_MODULE = _crypto
    return _crypto


def signed_transaction_envelope_from_json(envelope_json: str) -> "SignedTransactionEnvelope":
    """Parse a signed transaction envelope from a JSON payload."""

    return _require_crypto().signed_transaction_envelope_from_json(envelope_json)


__all__ = [
    "ToriiClient",
    "create_torii_client",
    "TransactionStatusError",
    "DataModelMismatchError",
    "signed_transaction_envelope_from_json",
    "resolve_torii_client_config",
    "ResolvedToriiClientConfig",
    "SseEvent",
    "SseStreamError",
    "EventCursor",
    "NetworkTimeSnapshot",
    "NetworkTimeStatus",
    "NetworkTimeSample",
    "NetworkTimeRttBucket",
    "OfflineActiveTransferVerifier",
    "OfflineActiveTopUpShieldVerifier",
    "OfflineActiveUnshieldVerifier",
    "OfflineActiveRecursiveStepEqVerifier",
    "OfflineActiveRecursiveStepEpVerifier",
    "NodeCapabilities",
    "NodeAdminSnapshot",
    "TransportConfig",
    "TransportNoritoRpcConfig",
    "StreamingTransportConfig",
    "StreamingSoranetConfig",
    "SorafsPorSubmissionResponse",
    "SorafsPorVerdictResponse",
    "SorafsPinRegisterResponse",
    "SorafsPorIngestionProviderStatus",
    "SorafsPorIngestionStatus",
    "ExplorerMetricsSnapshot",
    "ExplorerAccountQrSnapshot",
    "IsoSubmissionRecord",
    "IsoMessageTimeoutError",
    "AccountAsset",
    "AccountAssetsPage",
    "AccountTransaction",
    "AccountTransactionsPage",
    "AccountRecord",
    "AccountListPage",
    "DomainRecord",
    "DomainListPage",
    "AssetDefinitionRecord",
    "AssetDefinitionListPage",
    "AssetHolderRecord",
    "AssetHolderListPage",
    "AccountPermissionRecord",
    "AccountPermissionListPage",
    "SubscriptionPlanCreateResult",
    "SubscriptionPlanListItem",
    "SubscriptionPlanListPage",
    "SubscriptionCreateResult",
    "SubscriptionListItem",
    "SubscriptionListPage",
    "SubscriptionActionResult",
    "SumeragiEvidenceRecord",
    "SumeragiEvidenceListPage",
    "SumeragiQcSummary",
    "SumeragiRbcEviction",
    "SumeragiRbcStoreStatus",
    "SumeragiPrfStatus",
    "SumeragiStatusSnapshot",
    "SumeragiDiagnosticsSnapshot",
    "SumeragiV2LivenessStatus",
    "SumeragiV2StatusPhase",
    "SumeragiV2BodyState",
    "SumeragiV2GlobalPhase",
    "SumeragiV2HeightContextId",
    "SumeragiV2ConsensusRound",
    "SumeragiV2BlockSubject",
    "SumeragiV2ExecutionCommitment",
    "SumeragiV2QuorumCertificateRef",
    "SumeragiV2TimeoutCertificateRef",
    "SumeragiLaneSettlementReceipt",
    "SumeragiLaneSwapMetadata",
    "SumeragiLaneSettlementCommitment",
    "SumeragiLaneRelayEnvelope",
    "SumeragiNexusFeeScheduleInputs",
    "SumeragiNexusFeeReceipt",
    "SumeragiNativeAmxPhase",
    "SumeragiNativeAmxAttestationBody",
    "SumeragiNativeAmxAttestationQc",
    "SumeragiNativeAmxParticipantLaneBlockDescriptor",
    "SumeragiNativeAmxParticipantLaneBlockProposal",
    "SumeragiNativeAmxLeg",
    "SumeragiNativeAmxReceipt",
    "SumeragiParamsSnapshot",
    "SumeragiPacemakerSnapshot",
    "SumeragiPhasesEmaSnapshot",
    "SumeragiPhasesSnapshot",
    "SumeragiLeaderSnapshot",
    "SumeragiQcSnapshot",
    "SumeragiEvidenceCount",
    "TriggerRecord",
    "TriggerListPage",
    "PipelineDagSnapshot",
    "PipelineTxSnapshot",
    "PipelineRecoverySidecar",
    "RuntimeUpgradeCounters",
    "RuntimeMetrics",
    "RuntimeAbiActive",
    "RuntimeAbiHash",
    "RuntimeUpgradeStatus",
    "RuntimeUpgradeManifest",
    "RuntimeUpgradeRecord",
    "RuntimeUpgradeListItem",
    "RuntimeUpgradeListPage",
    "RuntimeInstruction",
    "RuntimeUpgradeActionResponse",
    "MultisigResponse",
    "ToriiCanonicalRequestAuth",
    "canonical_query_string",
    "canonical_request_message",
    "canonical_request_signature_message",
    "build_canonical_request_headers",
    "VpnQuoteCreateRequest",
    "VpnSessionCreateRequest",
    "VpnReceiptSubmitRequest",
    "VpnProfile",
    "VpnQuote",
    "VpnSession",
    "VpnReceipt",
    "VpnReceiptListResponse",
]


_DEFAULT_SUCCESS_STATUSES = frozenset({"Approved", "Committed", "Applied"})
_DEFAULT_FAILURE_STATUSES = frozenset({"Rejected", "Expired"})
_DEFAULT_RETRY_STATUSES = frozenset({502, 503, 504})
_DEFAULT_RETRY_METHODS = frozenset({"GET", "HEAD", "OPTIONS"})
_TRANSACTION_STATUS_SCOPES = frozenset({"local", "global"})


def _normalize_transaction_status_scope(value: str, context: str) -> str:
    normalized = _require_non_empty_string(value, context).lower()
    if normalized not in _TRANSACTION_STATUS_SCOPES:
        raise ValueError(f"{context} must be one of: local, global")
    return normalized

try:  # pragma: no cover - optional dependency
    import websocket
except ImportError:  # pragma: no cover - optional dependency
    websocket = None


class TransactionStatusError(RuntimeError):
    """Raised when a transaction reaches a terminal failure status."""

    def __init__(self, hash_hex: str, status: Optional[str], payload: Any) -> None:
        self.hash_hex = hash_hex
        self.status = status
        self.payload = payload
        status_repr = repr(status) if status is not None else "unknown"
        super().__init__(f"transaction {hash_hex} reported failure status {status_repr}")


class DataModelMismatchError(RuntimeError):
    """Raised when the node data model version does not match the SDK."""

    def __init__(self, expected: int, actual: Optional[int]) -> None:
        actual_label = "missing" if actual is None else str(actual)
        super().__init__(
            f"Torii data model version mismatch (expected {expected}, got {actual_label})"
        )
        self.expected = expected
        self.actual = actual


class ToriiClient(_BaseToriiClient):
    """Convenience wrapper that exposes Torii attachment/prover APIs under `iroha_python`.

    The implementation delegates to :class:`iroha_torii_client.client.ToriiClient`
    so existing behaviour stays intact while we grow a richer, higher-level SDK.
    """

    # NOTE: `iroha_torii_client.client.ToriiClient` already implements the full
    # API surface. This subclass only exists to document the export location and
    # to leave room for SDK-specific conveniences (e.g., auth interceptors).
    def __init__(
        self,
        base_url: str,
        session: Optional[requests.Session] = None,
        *,
        auth_token: Optional[str] = None,
        api_token: Optional[str] = None,
        default_headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = 30.0,
        max_retries: int = 3,
        backoff_factor: float = 0.5,
        backoff_initial_ms: Optional[int] = None,
        max_backoff_ms: Optional[int] = None,
        backoff_multiplier: Optional[float] = None,
        retry_on_status: Optional[Sequence[int]] = None,
        retry_on_methods: Optional[Sequence[str]] = None,
        chain_discriminant: Optional[int] = None,
        sorafs_alias_policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]] = None,
        sorafs_alias_warning: Optional[Callable[[SorafsAliasWarning], None]] = None,
        sorafs_alias_logger: Optional[logging.Logger] = None,
    ) -> None:
        super().__init__(base_url, session=session)
        self._chain_discriminant = normalize_i105_discriminant(
            DEFAULT_I105_DISCRIMINANT
            if chain_discriminant is None
            else chain_discriminant,
            "chain_discriminant",
        )
        self._timeout = timeout
        self._max_retries = max(0, int(max_retries))
        self._retry_statuses = (
            set(retry_on_status) if retry_on_status is not None else set(_DEFAULT_RETRY_STATUSES)
        )
        self._retry_methods = {
            method.upper()
            for method in (
                retry_on_methods if retry_on_methods is not None else _DEFAULT_RETRY_METHODS
            )
        }
        self._default_headers: Dict[str, str] = {"Accept": "application/json"}
        if default_headers:
            _reject_default_onboarding_header(default_headers, "default_headers")
            self._default_headers.update(default_headers)
        self._auth_token: Optional[str] = None
        self._api_token: Optional[str] = None
        self._status_state = _ToriiStatusState()
        if auth_token:
            self.set_auth_token(auth_token)
        if api_token:
            self.set_api_token(api_token)
        if backoff_initial_ms is not None or max_backoff_ms is not None or backoff_multiplier is not None:
            self._backoff_initial = max(0.0, (backoff_initial_ms or 0) / 1000.0)

            self._backoff_multiplier = max(1.0, backoff_multiplier if backoff_multiplier is not None else 2.0)
            if max_backoff_ms is None or max_backoff_ms <= 0:
                self._backoff_cap = math.inf
            else:
                self._backoff_cap = max(0.0, max_backoff_ms / 1000.0)
        else:
            self._backoff_initial = max(0.0, float(backoff_factor))
            self._backoff_multiplier = 2.0
            self._backoff_cap = math.inf
        self._sorafs_alias_policy = _normalize_sorafs_policy_config(sorafs_alias_policy)
        self._sorafs_alias_warning_hook = sorafs_alias_warning
        self._sorafs_alias_logger = sorafs_alias_logger or logging.getLogger(
            "iroha_python.sorafs.client"
        )
        self._sorafs_alias_metrics: Dict[str, int] = {}
        self._last_sorafs_alias_evaluation: Optional[SorafsAliasEvaluation] = None
        self._data_model_validation = "unknown"
        self._data_model_actual: Optional[int] = None

    def _normalize_canonical_account_id(self, value: Any, context: str) -> str:
        return _normalize_canonical_account_id(
            value,
            context,
            expected_discriminant=self._chain_discriminant,
        )

    def _native_transaction_account_id(self, value: Any, context: str) -> str:
        literal = _require_non_empty_string(value, context)
        if "@" in literal:
            return literal
        candidate_discriminants = [DEFAULT_I105_DISCRIMINANT]
        if self._chain_discriminant != DEFAULT_I105_DISCRIMINANT:
            candidate_discriminants.append(self._chain_discriminant)
        for discriminant in candidate_discriminants:
            try:
                address = AccountAddress.parse_encoded(
                    literal,
                    expected_discriminant=discriminant,
                )
            except AccountAddressError:
                continue
            return address.to_i105(DEFAULT_I105_DISCRIMINANT)
        return literal

    def _native_transaction_asset_id(self, value: Any, context: str) -> str:
        literal = _require_non_empty_string(value, context)
        parts = literal.split("#")
        if len(parts) not in {2, 3} or not all(parts):
            return literal
        definition, account_id = parts[0], parts[1]
        scope = parts[2] if len(parts) == 3 else None
        native_account_id = self._native_transaction_account_id(
            account_id,
            f"{context}.account_id",
        )
        if native_account_id == account_id:
            return literal
        result = f"{definition}#{native_account_id}"
        if scope is not None:
            result = f"{result}#{scope}"
        return result

    def privacy_capabilities(
        self,
        production_evidence: Any | None = None,
        *,
        chain_id: str | None = None,
    ) -> Dict[str, Any]:
        """Return SDK privacy catalog and implementation capability metadata."""

        return _privacy_capabilities(
            self,
            production_evidence,
            chain_id=chain_id,
        )

    def privacy_algorithm_descriptors(
        self,
        production_evidence: Any | None = None,
        *,
        chain_id: str | None = None,
    ) -> List[Dict[str, Any]]:
        """Return defensive-copy privacy algorithm descriptors."""

        return get_privacy_algorithm_descriptors(
            production_evidence,
            chain_id=chain_id,
        )

    @property
    def sorafs_alias_policy(self) -> SorafsAliasPolicy:
        """Return the resolved SoraFS alias cache policy."""

        return self._sorafs_alias_policy

    def set_sorafs_alias_policy(
        self,
        policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]],
    ) -> None:
        """Override the SoraFS alias cache policy used for validation."""

        self._sorafs_alias_policy = _normalize_sorafs_policy_config(policy)
        self._sorafs_alias_metrics.clear()
        self._last_sorafs_alias_evaluation = None

    def set_sorafs_alias_warning(
        self, callback: Optional[Callable[[SorafsAliasWarning], None]]
    ) -> None:
        """Install a callback invoked when proofs enter the refresh window or require rotation."""

        self._sorafs_alias_warning_hook = callback

    def get_sorafs_alias_metrics(self) -> Dict[str, int]:
        """Return aggregate counters for alias proof evaluations."""

        return dict(self._sorafs_alias_metrics)

    def get_last_sorafs_alias_evaluation(self) -> Optional[SorafsAliasEvaluation]:
        """Return the most recent alias proof evaluation observed by the client, if any."""

        return self._last_sorafs_alias_evaluation

    def _ensure_data_model_validation(self) -> None:
        if self._data_model_validation == "matched":
            return
        if self._data_model_validation == "mismatched":
            raise DataModelMismatchError(DATA_MODEL_VERSION, self._data_model_actual)

        try:
            capabilities = self.get_node_capabilities_typed()
        except RuntimeError as error:
            if "data_model_version" in str(error):
                self._data_model_validation = "mismatched"
                self._data_model_actual = None
                raise DataModelMismatchError(DATA_MODEL_VERSION, None) from error
            raise
        actual = capabilities.data_model_version
        if actual != DATA_MODEL_VERSION:
            self._data_model_validation = "mismatched"
            self._data_model_actual = actual
            raise DataModelMismatchError(DATA_MODEL_VERSION, actual)
        self._data_model_validation = "matched"
        self._data_model_actual = actual

    def submit_transaction(self, payload: bytes) -> Optional[Any]:
        """Submit a Norito-encoded transaction payload to `/v1/pipeline/transactions`.

        Raises :class:`DataModelMismatchError` when the node data model version mismatches.
        """

        self._ensure_data_model_validation()
        response = self._request(
            "POST",
            "/v1/pipeline/transactions",
            data=payload,
            headers={
                "Content-Type": "application/x-norito",
                "Accept": "application/x-norito, application/json",
            },
        )
        self._expect_status(response, {200, 201, 202, 204})
        receipt = type(self)._maybe_transaction_receipt(response)
        if receipt is not None:
            return receipt
        return type(self)._maybe_json(response)

    def submit_transaction_envelope(
        self, envelope: "SignedTransactionEnvelope"
    ) -> Optional[Any]:
        """Submit a transaction using a :class:`SignedTransactionEnvelope`."""

        payload = envelope.signed_transaction_versioned
        return self.submit_transaction(bytes(payload))

    def submit_transaction_draft(
        self,
        draft: "TransactionDraft",
        *,
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        instructions: Optional[Iterable["Instruction"]] = None,
        **sign_overrides: Any,
    ) -> tuple["SignedTransactionEnvelope", Optional[Any]]:
        """Sign a :class:`TransactionDraft` and submit it to Torii.

        Exactly one of ``private_key`` or ``private_key_hex`` must be provided. Additional
        keyword arguments are forwarded to :meth:`TransactionDraft.sign`, allowing callers to
        override fields such as ``creation_time_ms`` or ``ttl_ms``.
        """

        if (private_key is None) and (private_key_hex is None):
            raise ValueError("provide either `private_key` or `private_key_hex`")
        if private_key is not None and private_key_hex is not None:
            raise ValueError("provide only one of `private_key` or `private_key_hex`")

        envelope = self._sign_transaction_draft(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            instructions=instructions,
            **sign_overrides,
        )
        status = self.submit_transaction_envelope(envelope)
        return envelope, status

    def submit_transaction_json(self, envelope_json: str) -> Optional[Any]:
        """Submit a transaction described by the JSON produced via `to_json`."""

        envelope = signed_transaction_envelope_from_json(envelope_json)
        return self.submit_transaction_envelope(envelope)

    def submit_transaction_draft_and_wait(
        self,
        draft: "TransactionDraft",
        *,
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        instructions: Optional[Iterable["Instruction"]] = None,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        scope: str = "global",
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
        **sign_overrides: Any,
    ) -> Any:
        """Sign a draft, submit it, and wait for the transaction to reach a terminal status."""

        envelope = self._sign_transaction_draft(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            instructions=instructions,
            **sign_overrides,
        )
        self.submit_transaction_envelope(envelope)
        return self.submit_transaction_envelope_and_wait(
            envelope,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            scope=scope,
            success_statuses=success_statuses,
            failure_statuses=failure_statuses,
            on_status=on_status,
        )

    @staticmethod
    def _sign_transaction_draft(
        draft: "TransactionDraft",
        *,
        private_key: Optional[bytes],
        private_key_hex: Optional[str],
        instructions: Optional[Iterable["Instruction"]],
        **sign_overrides: Any,
    ) -> "SignedTransactionEnvelope":
        if private_key is None and private_key_hex is None:
            raise ValueError("provide either `private_key` or `private_key_hex`")
        if private_key is not None and private_key_hex is not None:
            raise ValueError("provide only one of `private_key` or `private_key_hex`")

        if private_key_hex is not None:
            return draft.sign_hex_private_key(
                private_key_hex,
                instructions=instructions,
                **sign_overrides,
            )
        assert private_key is not None
        return draft.sign(
            private_key,
            instructions=instructions,
            **sign_overrides,
        )

    def submit_transaction_json_and_wait(
        self,
        envelope_json: str,
        *,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        scope: str = "global",
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
    ) -> Any:
        """Submit a transaction JSON blob and wait for final status."""

        envelope = signed_transaction_envelope_from_json(envelope_json)
        return self.submit_transaction_envelope_and_wait(
            envelope,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            scope=scope,
            success_statuses=success_statuses,
            failure_statuses=failure_statuses,
            on_status=on_status,
        )

    def submit_transaction_envelope_and_wait(
        self,
        envelope: "SignedTransactionEnvelope",
        *,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        scope: str = "global",
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
    ) -> Any:
        """Submit a signed transaction and wait for its terminal status."""

        self.submit_transaction_envelope(envelope)

        hash_field = getattr(envelope, "hash", None)
        if hash_field is None:
            raise ValueError("SignedTransactionEnvelope.hash is required to poll status")
        if isinstance(hash_field, memoryview):
            hash_field = hash_field.tobytes()
        if isinstance(hash_field, (bytes, bytearray)):
            hash_hex = bytes(hash_field).hex()
        elif isinstance(hash_field, str):
            hash_hex = hash_field
        else:
            raise TypeError(
                "SignedTransactionEnvelope.hash must be bytes or hex string, "
                f"got {type(hash_field)!r}"
            )

        return self.wait_for_transaction_status(
            hash_hex,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            scope=scope,
            success_statuses=success_statuses,
            failure_statuses=failure_statuses,
            on_status=on_status,
        )

    # ------------------------------------------------------------------
    # HTTP helper utilities
    # ------------------------------------------------------------------
    def set_auth_token(self, token: Optional[str]) -> None:
        """Configure (or clear) the Authorization bearer token."""

        if token:
            self._auth_token = token
            self._default_headers["Authorization"] = f"Bearer {token}"
        else:
            self._auth_token = None
            self._default_headers.pop("Authorization", None)

    def set_api_token(self, token: Optional[str]) -> None:
        """Configure (or clear) the Torii `X-API-Token` header."""

        if token:
            self._api_token = token
            self._default_headers["X-API-Token"] = token
        else:
            self._api_token = None
            self._default_headers.pop("X-API-Token", None)

    def update_default_headers(self, headers: Mapping[str, str]) -> None:
        """Merge `headers` into the default header set applied to every request."""

        _reject_default_onboarding_header(headers, "headers")
        self._default_headers.update(headers)

    def request_json(
        self,
        method: str,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[Mapping[str, str]] = None,
        json_body: Optional[Mapping[str, Any]] = None,
        data: Optional[bytes] = None,
        expected_status: Sequence[int] = (200,),
        timeout: Optional[float] = None,
        allow_retry: bool = True,
    ) -> Optional[Any]:
        """Issue an HTTP request and decode the JSON payload when present."""

        response = self._request(
            method,
            path,
            params=params,
            headers=headers,
            json_body=json_body,
            data=data,
            timeout=timeout,
            allow_retry=allow_retry,
        )
        self._expect_status(response, expected_status)
        return self._maybe_json(response)

    @staticmethod
    def _operator_key_pair(
        *,
        key_pair: Optional[Any] = None,
        private_key: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        private_key_hex: Optional[str] = None,
    ) -> Any:
        provided = sum(value is not None for value in (key_pair, private_key, private_key_hex))
        if provided != 1:
            raise ValueError("provide exactly one of key_pair, private_key, or private_key_hex")
        if key_pair is not None:
            signer = getattr(key_pair, "sign", None)
            public_key = getattr(key_pair, "public_key_multihash", None)
            if not callable(signer) or public_key is None:
                raise TypeError("key_pair must expose sign(message) and public_key_multihash")
            return key_pair

        from .crypto import CryptoKeyPair, Ed25519KeyPair

        if private_key_hex is not None:
            raw_hex = ToriiClient._require_non_empty_string(private_key_hex, "private_key_hex")
            return Ed25519KeyPair.from_private_key_hex(raw_hex)

        assert private_key is not None
        if isinstance(private_key, (bytes, bytearray, memoryview)):
            return Ed25519KeyPair.from_private_key(bytes(private_key))
        secret = ToriiClient._require_non_empty_string(private_key, "private_key")
        try:
            return CryptoKeyPair.from_private_key_multihash(secret)
        except Exception as multihash_error:
            try:
                raw = bytes.fromhex(secret)
            except ValueError as exc:
                raise ValueError(
                    "private_key must be a private-key multihash or raw Ed25519 hex"
                ) from exc
            if len(raw) != 32:
                raise ValueError("raw Ed25519 private_key must be 32 bytes") from multihash_error
            return Ed25519KeyPair.from_private_key(raw)

    @staticmethod
    def build_operator_signature_headers(
        *,
        method: str,
        path: str,
        body: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        key_pair: Optional[Any] = None,
        private_key: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        private_key_hex: Optional[str] = None,
        timestamp_ms: Optional[int] = None,
        nonce: Optional[str] = None,
    ) -> Dict[str, str]:
        """Build `x-iroha-operator-*` headers for operator-only Torii endpoints."""

        key = ToriiClient._operator_key_pair(
            key_pair=key_pair,
            private_key=private_key,
            private_key_hex=private_key_hex,
        )
        public_key = getattr(key, "public_key_multihash", None)
        public_key_text = str(public_key or "").strip()
        if not public_key_text:
            raise TypeError("operator key pair must expose a non-empty public_key_multihash")
        effective_timestamp = int(timestamp_ms if timestamp_ms is not None else time.time() * 1000)
        effective_nonce = nonce if nonce is not None else secrets.token_urlsafe(12)
        if effective_timestamp < 0:
            raise ValueError("timestamp_ms must be non-negative")
        if not isinstance(effective_nonce, str) or not effective_nonce.strip():
            raise ValueError("nonce must be a non-empty string")
        message = canonical_request_signature_message(
            method,
            path,
            body,
            timestamp_ms=effective_timestamp,
            nonce=effective_nonce,
        )
        signature = key.sign(message)
        if not isinstance(signature, (bytes, bytearray, memoryview)):
            raise TypeError("operator signer must return bytes")
        return {
            "x-iroha-operator-public-key": public_key_text,
            "x-iroha-operator-timestamp-ms": str(effective_timestamp),
            "x-iroha-operator-nonce": effective_nonce,
            "x-iroha-operator-signature": base64.b64encode(bytes(signature)).decode("ascii"),
        }

    # -------------------------
    # Explorer APIs
    # -------------------------

    def get_explorer_metrics(self) -> Optional[Any]:
        """Fetch `/v1/explorer/metrics`. Returns `None` when telemetry is gated."""

        response = self._request(
            "GET",
            "/v1/explorer/metrics",
            headers={"Accept": "application/json"},
            allow_retry=True,
        )
        if response.status_code in {403, 404, 503}:
            return None
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def get_explorer_metrics_typed(self) -> Optional[ExplorerMetricsSnapshot]:
        """Typed wrapper for :meth:`get_explorer_metrics`."""

        payload = self.get_explorer_metrics()
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("explorer metrics endpoint returned non-object payload")
        return ExplorerMetricsSnapshot.from_payload(payload)

    def get_explorer_account_qr(
        self,
        account_id: str,
    ) -> Mapping[str, Any]:
        """Fetch explorer QR metadata via `GET /v1/explorer/accounts/{account_id}/qr`."""

        canonical_account_id = self._normalize_canonical_account_id(
            account_id, "account_id"
        )
        payload = self.request_json(
            "GET",
            f"/v1/explorer/accounts/{quote(canonical_account_id, safe='')}/qr",
            headers={"Accept": "application/json"},
            expected_status=(200,),
        )
        if payload is None:
            raise RuntimeError("explorer account qr endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("explorer account qr response must be a JSON object")
        return payload

    def get_explorer_account_qr_typed(
        self,
        account_id: str,
    ) -> ExplorerAccountQrSnapshot:
        """Typed QR wrapper for :meth:`get_explorer_account_qr`."""

        payload = self.get_explorer_account_qr(account_id)
        return ExplorerAccountQrSnapshot.from_payload(payload)

    def list_explorer_rwas(
        self,
        *,
        page: Optional[int] = None,
        per_page: Optional[int] = None,
        owned_by: Optional[str] = None,
        domain: Optional[str] = None,
    ) -> Mapping[str, Any]:
        """List explorer RWAs via `GET /v1/explorer/rwas`."""

        params: Dict[str, Any] = {}
        page_value = _coerce_int(page, "list_explorer_rwas.page")
        if page_value is not None:
            params["page"] = page_value
        per_page_value = _coerce_int(per_page, "list_explorer_rwas.per_page")
        if per_page_value is not None:
            params["per_page"] = per_page_value
        owned_by_value = _normalize_optional_string(owned_by, "list_explorer_rwas.owned_by")
        if owned_by_value is not None:
            params["owned_by"] = owned_by_value
        domain_value = _normalize_optional_string(domain, "list_explorer_rwas.domain")
        if domain_value is not None:
            params["domain"] = domain_value
        payload = self.request_json(
            "GET",
            "/v1/explorer/rwas",
            params=params or None,
            headers={"Accept": "application/json"},
            expected_status=(200,),
        )
        if payload is None:
            raise RuntimeError("explorer RWA endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("explorer RWA endpoint returned malformed payload")
        return payload

    def list_explorer_rwas_typed(
        self,
        *,
        page: Optional[int] = None,
        per_page: Optional[int] = None,
        owned_by: Optional[str] = None,
        domain: Optional[str] = None,
    ) -> ExplorerRwasPage:
        """Typed wrapper for :meth:`list_explorer_rwas`."""

        payload = self.list_explorer_rwas(
            page=page,
            per_page=per_page,
            owned_by=owned_by,
            domain=domain,
        )
        return ExplorerRwasPage.from_payload(payload)

    def get_explorer_rwa_detail(self, rwa_id: str) -> Mapping[str, Any]:
        """Fetch a single explorer RWA detail via `GET /v1/explorer/rwas/{rwa_id}`."""

        rwa_id_value = _normalize_optional_string(rwa_id, "get_explorer_rwa_detail.rwa_id")
        if rwa_id_value is None:
            raise ValueError("get_explorer_rwa_detail.rwa_id must be a non-empty string")
        payload = self.request_json(
            "GET",
            f"/v1/explorer/rwas/{quote(rwa_id_value, safe='')}",
            headers={"Accept": "application/json"},
            expected_status=(200,),
        )
        if payload is None:
            raise RuntimeError("explorer RWA detail endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("explorer RWA detail endpoint returned malformed payload")
        return payload

    def get_explorer_rwa_detail_typed(self, rwa_id: str) -> ExplorerRwaRecord:
        """Typed wrapper for :meth:`get_explorer_rwa_detail`."""

        payload = self.get_explorer_rwa_detail(rwa_id)
        return ExplorerRwaRecord.from_payload(payload)

    # -------------------------
    # ISO 20022 bridge APIs
    # -------------------------

    def _submit_iso_message(
        self,
        path: str,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str],
        timeout: Optional[float],
        context: str,
    ) -> Optional[Any]:
        payload = _normalize_iso_payload(message, f"{context}.message")
        headers = {
            "Content-Type": content_type.strip() if isinstance(content_type, str) and content_type.strip() else "application/xml",
            "Accept": "application/json",
        }
        response = self._request(
            "POST",
            path,
            data=payload,
            headers=headers,
            timeout=timeout,
        )
        self._expect_status(response, (202,))
        return self._maybe_json(response)

    def submit_iso_pacs008(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Submit a pacs.008 payload (`POST /v1/iso20022/pacs008`)."""

        return self._submit_iso_message(
            "/v1/iso20022/pacs008",
            message,
            content_type=content_type,
            timeout=timeout,
            context="submit_iso_pacs008",
        )

    def submit_iso_pacs008_typed(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[IsoSubmissionRecord]:
        """Typed wrapper for :meth:`submit_iso_pacs008`."""

        payload = self.submit_iso_pacs008(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("ISO pacs.008 submission returned a non-object payload")
        return IsoSubmissionRecord.from_payload(payload, context="iso pacs.008 submission")

    def submit_iso_pacs009(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Submit a pacs.009 payload (`POST /v1/iso20022/pacs009`)."""

        return self._submit_iso_message(
            "/v1/iso20022/pacs009",
            message,
            content_type=content_type,
            timeout=timeout,
            context="submit_iso_pacs009",
        )

    def submit_iso_pacs009_typed(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Optional[IsoSubmissionRecord]:
        """Typed wrapper for :meth:`submit_iso_pacs009`."""

        payload = self.submit_iso_pacs009(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("ISO pacs.009 submission returned a non-object payload")
        return IsoSubmissionRecord.from_payload(payload, context="iso pacs.009 submission")

    def get_iso_message_status(
        self,
        message_id: str,
        *,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Fetch ISO bridge status via `GET /v1/iso20022/messages/{message_id}`."""

        normalized_id = _require_non_empty_string(message_id, "message_id")
        encoded_id = quote(normalized_id, safe="")
        response = self._request(
            "GET",
            f"/v1/iso20022/messages/{encoded_id}",
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def get_iso_message_status_typed(
        self,
        message_id: str,
        *,
        timeout: Optional[float] = None,
    ) -> Optional[IsoSubmissionRecord]:
        """Typed wrapper for :meth:`get_iso_message_status`."""

        payload = self.get_iso_message_status(message_id, timeout=timeout)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("ISO status endpoint returned a non-object payload")
        return IsoSubmissionRecord.from_payload(payload, context="iso status response")

    def wait_for_iso_message_status(
        self,
        message_id: str,
        *,
        poll_interval: float = _DEFAULT_ISO_POLL_INTERVAL_SECONDS,
        max_attempts: int = _DEFAULT_ISO_WAIT_ATTEMPTS,
        resolve_on_accepted: bool = False,
        timeout: Optional[float] = None,
        on_poll: Optional[Callable[[Optional[IsoSubmissionRecord], int], None]] = None,
    ) -> IsoSubmissionRecord:
        """Poll the ISO bridge until the message reaches a terminal state."""

        normalized_id = _require_non_empty_string(message_id, "message_id")
        if poll_interval < 0.0:
            raise ValueError("poll_interval must be non-negative")
        if 0.0 < poll_interval < _MIN_ISO_POLL_INTERVAL_SECONDS:
            poll_interval = _MIN_ISO_POLL_INTERVAL_SECONDS
        if max_attempts <= 0:
            raise ValueError("max_attempts must be positive")
        if on_poll is not None and not callable(on_poll):
            raise TypeError("wait.on_poll must be callable when provided")

        attempts = 0
        last_status: Optional[IsoSubmissionRecord] = None
        while True:
            attempts += 1
            status_payload = self.get_iso_message_status_typed(normalized_id, timeout=timeout)
            last_status = status_payload
            if on_poll is not None:
                on_poll(status_payload, attempts)
            if status_payload and _is_iso_status_terminal(status_payload, resolve_on_accepted):
                return status_payload
            if attempts >= max_attempts:
                raise IsoMessageTimeoutError(normalized_id, attempts, last_status)
            if poll_interval > 0.0:
                time.sleep(poll_interval)

    def submit_iso_pacs008_and_wait(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
        wait: Optional[Mapping[str, Any]] = None,
    ) -> IsoSubmissionRecord:
        """Submit a pacs.008 payload and wait for a terminal status."""

        submission = self.submit_iso_pacs008_typed(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if submission is None:
            raise RuntimeError("ISO pacs.008 submission did not return a message_id")
        wait_kwargs = _normalize_iso_wait_kwargs(wait, context="submit_iso_pacs008_and_wait.wait")
        return self.wait_for_iso_message_status(submission.message_id, **wait_kwargs)

    def submit_iso_pacs009_and_wait(
        self,
        message: Union[str, bytes, bytearray, memoryview],
        *,
        content_type: Optional[str] = None,
        timeout: Optional[float] = None,
        wait: Optional[Mapping[str, Any]] = None,
    ) -> IsoSubmissionRecord:
        """Submit a pacs.009 payload and wait for a terminal status."""

        submission = self.submit_iso_pacs009_typed(
            message,
            content_type=content_type,
            timeout=timeout,
        )
        if submission is None:
            raise RuntimeError("ISO pacs.009 submission did not return a message_id")
        wait_kwargs = _normalize_iso_wait_kwargs(wait, context="submit_iso_pacs009_and_wait.wait")
        return self.wait_for_iso_message_status(submission.message_id, **wait_kwargs)

    # ------------------------------------------------------------------
    # Repo agreements
    # ------------------------------------------------------------------

    def list_repo_agreements(self, **params: Any) -> RepoAgreementListPage:
        """List repo agreements (`GET /v1/repo/agreements`)."""

        cleaned_params = self._clean_params(params)
        if "count_mode" in cleaned_params:
            cleaned_params["count_mode"] = _normalize_count_mode_arg(
                cleaned_params["count_mode"]
            )
        response = self._request(
            "GET",
            "/v1/repo/agreements",
            params=cleaned_params,
        )
        self._expect_status(response, (200,))
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("repo agreements endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("repo agreements endpoint returned malformed payload")
        return RepoAgreementListPage.from_payload(payload)

    def query_repo_agreements(self, envelope: Mapping[str, Any]) -> RepoAgreementListPage:
        """Query repo agreements (`POST /v1/repo/agreements/query`)."""

        body = dict(envelope)
        if body.get("count_mode") is not None:
            body["count_mode"] = _normalize_count_mode_arg(body["count_mode"])
        payload = self._post_json(
            "/v1/repo/agreements/query",
            body,
            context="repo agreements query",
        )
        return RepoAgreementListPage.from_payload(payload)

    def get_sorafs_pin_manifest(
        self,
        digest_hex: str,
        *,
        headers: Optional[Mapping[str, str]] = None,
    ) -> Optional[Any]:
        """Fetch a SoraFS pin manifest (`GET /v1/sorafs/pin/{digest}`) enforcing alias policy."""

        if not isinstance(digest_hex, str) or not digest_hex.strip():
            raise ValueError("digest_hex must be a non-empty string")
        response = self._request(
            "GET",
            f"/v1/sorafs/pin/{digest_hex}",
            headers=headers,
        )
        self._expect_status(response, (200,))
        return type(self)._maybe_json(response)

    def register_sorafs_pin_manifest(
        self,
        transaction: "SignedTransactionEnvelope",
        *,
        timeout: Optional[float] = None,
    ) -> SorafsPinRegisterResponse:
        """Submit one already-signed native pin-registration transaction."""

        payload = bytes(transaction.signed_transaction_versioned)
        if not payload:
            raise ValueError("transaction must contain non-empty versioned signed bytes")
        response = self._request(
            "POST",
            "/v1/sorafs/pin/register",
            data=payload,
            headers={
                "Content-Type": "application/x-norito",
                "Accept": "application/json",
            },
            timeout=timeout,
        )
        self._expect_status(response, (202,))
        body = type(self)._maybe_json(response)
        if not isinstance(body, Mapping):
            raise RuntimeError("sorafs pin register endpoint returned malformed payload")
        return SorafsPinRegisterResponse.from_payload(body, "sorafs_pin_register")

    def get_sorafs_orderbook(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """Fetch one finalized native order page and authoritative ledger status."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/book",
            params=type(self)._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="get_sorafs_orderbook",
            ),
            headers=type(self)._sorafs_orderbook_headers(
                headers=headers,
                context="get_sorafs_orderbook",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook book endpoint returned no payload")
        return type(self)._parse_sorafs_orderbook_book(
            payload,
            context="sorafs orderbook book response",
        )

    def list_sorafs_orderbook_trades(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """List finalized native SoraFS orderbook trades."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/trades",
            params=type(self)._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="list_sorafs_orderbook_trades",
            ),
            headers=type(self)._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_trades",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook trades endpoint returned no payload")
        return type(self)._parse_sorafs_orderbook_trade_page_response(
            payload,
            context="sorafs orderbook trades response",
        )

    def list_sorafs_orderbook_channels(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """List finalized native SoraFS settlement channels."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/channels",
            params=type(self)._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="list_sorafs_orderbook_channels",
            ),
            headers=type(self)._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_channels",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook channels endpoint returned no payload")
        return type(self)._parse_sorafs_orderbook_channel_page_response(
            payload,
            context="sorafs orderbook channels response",
        )

    def list_sorafs_orderbook_receipts(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_id_hex: Optional[Any] = None,
        limit: Optional[Any] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """List finalized native SoraFS settlement receipts."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/receipts",
            params=type(self)._sorafs_orderbook_read_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_id_hex=after_id_hex,
                limit=limit,
                context="list_sorafs_orderbook_receipts",
            ),
            headers=type(self)._sorafs_orderbook_headers(
                headers=headers,
                context="list_sorafs_orderbook_receipts",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook receipts endpoint returned no payload")
        return type(self)._parse_sorafs_orderbook_receipt_page_response(
            payload,
            context="sorafs orderbook receipts response",
        )

    def submit_sorafs_orderbook_order(
        self,
        signed_transaction: Any,
        *,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """Submit a caller-signed native transaction containing one order ISI."""

        return self._submit_sorafs_orderbook_transaction(
            "/v1/sorafs/orderbook/orders",
            signed_transaction,
            headers=headers,
            timeout=timeout,
            context="submit_sorafs_orderbook_order",
        )

    def submit_sorafs_orderbook_cancel(
        self,
        signed_transaction: Any,
        *,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """Submit a caller-signed native transaction containing one cancel ISI."""

        return self._submit_sorafs_orderbook_transaction(
            "/v1/sorafs/orderbook/cancel",
            signed_transaction,
            headers=headers,
            timeout=timeout,
            context="submit_sorafs_orderbook_cancel",
        )

    def submit_sorafs_orderbook_receipt(
        self,
        signed_transaction: Any,
        *,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """Submit a caller-signed native transaction containing one receipt ISI."""

        return self._submit_sorafs_orderbook_transaction(
            "/v1/sorafs/orderbook/receipts",
            signed_transaction,
            headers=headers,
            timeout=timeout,
            context="submit_sorafs_orderbook_receipt",
        )

    def _submit_sorafs_orderbook_transaction(
        self,
        path: str,
        signed_transaction: Any,
        *,
        headers: Optional[Mapping[str, str]],
        timeout: Optional[float],
        context: str,
    ) -> Dict[str, Any]:
        body = type(self)._sorafs_orderbook_transaction_bytes(
            signed_transaction,
            f"{context}.signed_transaction",
        )
        response = self._request(
            "POST",
            path,
            headers=type(self)._sorafs_orderbook_submit_headers(
                headers=headers,
                context=context,
            ),
            data=body,
            timeout=timeout,
            allow_retry=False,
        )
        self._expect_status(response, (202,))
        response_payload = type(self)._maybe_json(response)
        if response_payload is None:
            raise RuntimeError(f"{context} endpoint returned no payload")
        return type(self)._parse_sorafs_orderbook_submission_receipt(
            response_payload,
            context=f"{context} response",
        )

    def list_sorafs_orderbook_events(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_sequence: Optional[Any] = None,
        after_block_height: Optional[Any] = None,
        after_block_hash_hex: Optional[Any] = None,
        after_event_index: Optional[Any] = None,
        limit: Optional[Any] = None,
        if_none_match: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Dict[str, Any]]:
        """List replayable finalized native SoraFS orderbook events."""

        response = self._request(
            "GET",
            "/v1/sorafs/orderbook/events",
            params=type(self)._sorafs_orderbook_event_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_sequence=after_sequence,
                after_block_height=after_block_height,
                after_block_hash_hex=after_block_hash_hex,
                after_event_index=after_event_index,
                limit=limit,
                context="list_sorafs_orderbook_events",
            ),
            headers=type(self)._sorafs_orderbook_headers(
                if_none_match=if_none_match,
                headers=headers,
                context="list_sorafs_orderbook_events",
                cache=True,
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200, 304))
        if response.status_code == 304:
            return None
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs orderbook events endpoint returned no payload")
        return type(self)._parse_sorafs_orderbook_event_page_response(
            payload,
            context="sorafs orderbook events response",
        )

    def stream_sorafs_orderbook_events(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_sequence: Optional[Any] = None,
        after_block_height: Optional[Any] = None,
        after_block_hash_hex: Optional[Any] = None,
        after_event_index: Optional[Any] = None,
        limit: Optional[Any] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream finalized orderbook events from the native ledger journal."""

        params = type(self)._sorafs_orderbook_event_params(
            expected_finalized_height=expected_finalized_height,
            expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
            after_sequence=after_sequence,
            after_block_height=after_block_height,
            after_block_hash_hex=after_block_hash_hex,
            after_event_index=after_event_index,
            limit=limit,
            context="stream_sorafs_orderbook_events",
        )
        initial_event_id = last_event_id if last_event_id is not None else (
            cursor.last_event_id if cursor is not None else None
        )
        should_resume = resume or cursor is not None or last_event_id is not None

        def _normalize_event(event: SseEvent) -> SseEvent:
            if event.event == "lagged" or not isinstance(event.data, Mapping):
                return event
            return SseEvent(
                event=event.event,
                data=type(self)._parse_sorafs_orderbook_finalized_event(
                    event.data,
                    context=f"sorafs orderbook stream event {event.id or ''}".strip(),
                ),
                id=event.id,
                retry=event.retry,
                raw=event.raw,
            )

        def _handle(event: SseEvent) -> None:
            if on_event is None:
                return
            normalized = _normalize_event(event)
            if with_metadata:
                on_event(normalized)
            else:
                on_event(normalized.data, normalized.id)

        iterator = self._stream_sse(
            "/v1/sorafs/orderbook/events/stream",
            params=params,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=initial_event_id,
            resume=should_resume,
            decode_json=decode_json,
            cursor=cursor,
            allow_resume=True,
            on_event=_handle if on_event is not None else None,
        )

        def _events():
            for event in iterator:
                normalized = _normalize_event(event)
                yield normalized if with_metadata else normalized.data

        return _events()

    def build_sorafs_orderbook_events_websocket_url(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_sequence: Optional[Any] = None,
        after_block_height: Optional[Any] = None,
        after_block_hash_hex: Optional[Any] = None,
        after_event_index: Optional[Any] = None,
        limit: Optional[Any] = None,
        endpoint_path: str = "/v1/sorafs/orderbook/events/ws",
    ) -> str:
        """Build the finalized native orderbook event WebSocket URL."""

        return _sorafs_orderbook_events_websocket_url(
            self._base_url,
            params=type(self)._sorafs_orderbook_event_params(
                expected_finalized_height=expected_finalized_height,
                expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
                after_sequence=after_sequence,
                after_block_height=after_block_height,
                after_block_hash_hex=after_block_hash_hex,
                after_event_index=after_event_index,
                limit=limit,
                context="build_sorafs_orderbook_events_websocket_url",
            ),
            endpoint_path=endpoint_path,
            context="build_sorafs_orderbook_events_websocket_url",
        )

    def connect_sorafs_orderbook_events_websocket(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_sequence: Optional[Any] = None,
        after_block_height: Optional[Any] = None,
        after_block_hash_hex: Optional[Any] = None,
        after_event_index: Optional[Any] = None,
        limit: Optional[Any] = None,
        endpoint_path: str = "/v1/sorafs/orderbook/events/ws",
        timeout: Optional[float] = None,
        headers: Optional[Mapping[str, str]] = None,
        subprotocols: Optional[Sequence[str]] = None,
        websocket_factory: Optional[Callable[..., Any]] = None,
    ) -> Any:
        """Open the finalized native SoraFS orderbook event WebSocket."""

        factory = websocket_factory
        if factory is None:
            if websocket is None:  # pragma: no cover - dependency optional
                raise RuntimeError(
                    "websocket-client is not installed. Install iroha-python with the `ws` extra "
                    "(`pip install iroha-python[ws]`) or add `websocket-client` to your environment."
                )
            factory = websocket.create_connection
        ws_url = self.build_sorafs_orderbook_events_websocket_url(
            expected_finalized_height=expected_finalized_height,
            expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
            after_sequence=after_sequence,
            after_block_height=after_block_height,
            after_block_hash_hex=after_block_hash_hex,
            after_event_index=after_event_index,
            limit=limit,
            endpoint_path=endpoint_path,
        )

        header_list: List[str] = []
        combined_headers: Dict[str, str] = dict(self._default_headers)
        combined_headers.pop("Accept", None)
        if headers:
            combined_headers.update(headers)
        for key, value in combined_headers.items():
            header_list.append(f"{key}: {value}")

        return factory(
            ws_url,
            timeout=timeout,
            header=header_list or None,
            subprotocols=list(subprotocols) if subprotocols else None,
        )

    def stream_sorafs_orderbook_events_websocket(
        self,
        *,
        expected_finalized_height: Optional[Any] = None,
        expected_finalized_block_hash_hex: Optional[Any] = None,
        after_sequence: Optional[Any] = None,
        after_block_height: Optional[Any] = None,
        after_block_hash_hex: Optional[Any] = None,
        after_event_index: Optional[Any] = None,
        limit: Optional[Any] = None,
        endpoint_path: str = "/v1/sorafs/orderbook/events/ws",
        timeout: Optional[float] = None,
        headers: Optional[Mapping[str, str]] = None,
        subprotocols: Optional[Sequence[str]] = None,
        websocket_factory: Optional[Callable[..., Any]] = None,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        close_on_return: bool = True,
    ):
        """Stream finalized native orderbook events from WebSocket JSON frames."""

        socket = self.connect_sorafs_orderbook_events_websocket(
            expected_finalized_height=expected_finalized_height,
            expected_finalized_block_hash_hex=expected_finalized_block_hash_hex,
            after_sequence=after_sequence,
            after_block_height=after_block_height,
            after_block_hash_hex=after_block_hash_hex,
            after_event_index=after_event_index,
            limit=limit,
            endpoint_path=endpoint_path,
            timeout=timeout,
            headers=headers,
            subprotocols=subprotocols,
            websocket_factory=websocket_factory,
        )

        def _normalize_event(event: WebSocketEvent) -> WebSocketEvent:
            if event.event == "lagged" or not isinstance(event.data, Mapping):
                return event
            return WebSocketEvent(
                event=event.event,
                data=type(self)._parse_sorafs_orderbook_finalized_event(
                    event.data,
                    context=(
                        f"sorafs orderbook websocket event {event.event or ''}".strip()
                    ),
                ),
                raw=event.raw,
            )

        def _events():
            try:
                while True:
                    event = _normalize_event(
                        _parse_websocket_json_event(
                            socket.recv(),
                            "stream_sorafs_orderbook_events_websocket",
                        )
                    )
                    if on_event is not None:
                        if with_metadata:
                            on_event(event)
                        else:
                            on_event(event.data)
                    yield event if with_metadata else event.data
            finally:
                if close_on_return and hasattr(socket, "close"):
                    socket.close()

        return _events()

    def get_sorafs_reputation_latest(
        self,
        *,
        if_none_match: Optional[str] = None,
        etag: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Fetch the latest SoraFS reputation snapshot summary."""

        response = self._request(
            "GET",
            "/v1/sorafs/reputation/latest",
            headers=_sorafs_reputation_headers(
                if_none_match=if_none_match,
                etag=etag,
                headers=headers,
                context="get_sorafs_reputation_latest",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200, 304, 404))
        if response.status_code in {304, 404}:
            return None
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs reputation latest endpoint returned no payload")
        return payload

    def get_sorafs_reputation_provider(
        self,
        provider_id: str,
        *,
        if_none_match: Optional[str] = None,
        etag: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Fetch a provider reputation record and Merkle proof from the latest snapshot."""

        normalized_provider = _normalize_sorafs_reputation_provider_id(
            provider_id,
            "get_sorafs_reputation_provider.provider_id",
        )
        response = self._request(
            "GET",
            f"/v1/sorafs/reputation/providers/{quote(normalized_provider, safe='')}",
            headers=_sorafs_reputation_headers(
                if_none_match=if_none_match,
                etag=etag,
                headers=headers,
                context="get_sorafs_reputation_provider",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200, 304, 404))
        if response.status_code in {304, 404}:
            return None
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs reputation provider endpoint returned no payload")
        return payload

    def get_sorafs_reputation_snapshot(
        self,
        snapshot_id_hex: str,
        *,
        if_none_match: Optional[str] = None,
        etag: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Fetch a historical SoraFS reputation snapshot by its 16-byte id."""

        normalized_snapshot_id = _normalize_sorafs_reputation_snapshot_id_hex(
            snapshot_id_hex,
            "get_sorafs_reputation_snapshot.snapshot_id_hex",
        )
        response = self._request(
            "GET",
            f"/v1/sorafs/reputation/snapshots/{normalized_snapshot_id}",
            headers=_sorafs_reputation_headers(
                if_none_match=if_none_match,
                etag=etag,
                headers=headers,
                context="get_sorafs_reputation_snapshot",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200, 304, 404))
        if response.status_code in {304, 404}:
            return None
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs reputation snapshot endpoint returned no payload")
        return payload

    def get_sorafs_reputation_weights(
        self,
        *,
        if_none_match: Optional[str] = None,
        etag: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Fetch active SoraFS reputation scoring weights."""

        response = self._request(
            "GET",
            "/v1/sorafs/reputation/weights",
            headers=_sorafs_reputation_headers(
                if_none_match=if_none_match,
                etag=etag,
                headers=headers,
                context="get_sorafs_reputation_weights",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200, 304, 404))
        if response.status_code in {304, 404}:
            return None
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs reputation weights endpoint returned no payload")
        return payload

    def list_sorafs_reputation_events(
        self,
        *,
        since: Optional[Any] = None,
        limit: Optional[Any] = None,
        if_none_match: Optional[str] = None,
        etag: Optional[str] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """List SoraFS reputation snapshot events."""

        response = self._request(
            "GET",
            "/v1/sorafs/reputation/events",
            params=_sorafs_reputation_event_params(
                since=since,
                limit=limit,
                context="list_sorafs_reputation_events",
            ),
            headers=_sorafs_reputation_headers(
                if_none_match=if_none_match,
                etag=etag,
                headers=headers,
                context="list_sorafs_reputation_events",
            ),
            timeout=timeout,
        )
        self._expect_status(response, (200, 304))
        if response.status_code == 304:
            return None
        payload = type(self)._maybe_json(response)
        if payload is None:
            raise RuntimeError("sorafs reputation events endpoint returned no payload")
        return payload

    def stream_sorafs_reputation_events(
        self,
        *,
        since: Optional[Any] = None,
        limit: Optional[Any] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        on_event: Optional[Callable[..., None]] = None,
        cursor: Optional[EventCursor] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream SoraFS reputation snapshot events via `/v1/sorafs/reputation/events/stream`."""

        params = _sorafs_reputation_event_params(
            since=since,
            limit=limit,
            context="stream_sorafs_reputation_events",
        )
        initial_event_id = last_event_id if last_event_id is not None else (
            cursor.last_event_id if cursor is not None else None
        )
        should_resume = resume or cursor is not None or last_event_id is not None

        def _handle(event: SseEvent) -> None:
            if on_event is None:
                return
            if with_metadata:
                on_event(event)
            else:
                on_event(event.data, event.id)

        iterator = self._stream_sse(
            "/v1/sorafs/reputation/events/stream",
            params=params,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            last_event_id=initial_event_id,
            resume=should_resume,
            decode_json=decode_json,
            cursor=cursor,
            allow_resume=True,
            on_event=_handle if on_event is not None else None,
        )
        if with_metadata:
            return iterator
        return (event.data for event in iterator)

    # -------------------------
    # SoraFS Proof-of-Retrievability APIs
    # -------------------------

    def record_sorafs_por_proof(
        self,
        *,
        proof: Optional[Union[bytes, bytearray, memoryview]] = None,
        proof_b64: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> SorafsPorSubmissionResponse:
        """Submit a `PorProofV1` record for a provider."""

        payload = {
            "proof_b64": _normalize_base64_payload(
                proof_b64, proof, "record_sorafs_por_proof.proof"
            )
        }
        response = self._request(
            "POST",
            "/v1/sorafs/capacity/por-proof",
            json_body=payload,
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("por-proof endpoint returned an empty payload")
        return SorafsPorSubmissionResponse.from_payload(body, "sorafs_por_proof")

    def record_sorafs_por_verdict(
        self,
        *,
        verdict: Optional[Union[bytes, bytearray, memoryview]] = None,
        verdict_b64: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> SorafsPorVerdictResponse:
        """Submit an audit verdict for a PoR challenge."""

        payload = {
            "verdict_b64": _normalize_base64_payload(
                verdict_b64, verdict, "record_sorafs_por_verdict.verdict"
            )
        }
        response = self._request(
            "POST",
            "/v1/sorafs/capacity/por-verdict",
            json_body=payload,
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        body = self._maybe_json(response)
        if body is None:
            raise RuntimeError("por-verdict endpoint returned an empty payload")
        return SorafsPorVerdictResponse.from_payload(body, "sorafs_por_verdict")

    def get_sorafs_por_status(
        self,
        *,
        manifest_hex: Optional[str] = None,
        provider_hex: Optional[str] = None,
        epoch: Optional[int] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        page_token_hex: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> bytes:
        """Return Norito-encoded `PorChallengeStatusV1` records for the given filters."""

        params = _build_sorafs_por_status_params(
            manifest_hex, provider_hex, epoch, status, limit, page_token_hex
        )
        response = self._request(
            "GET",
            "/v1/sorafs/por/status",
            params=params,
            headers={"Accept": "application/x-norito"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return response.content

    def export_sorafs_por_status(
        self,
        *,
        start_epoch: Optional[int] = None,
        end_epoch: Optional[int] = None,
        timeout: Optional[float] = None,
    ) -> bytes:
        """Return a Norito-exported history for the supplied epoch range."""

        params = _build_sorafs_por_export_params(start_epoch, end_epoch)
        response = self._request(
            "GET",
            "/v1/sorafs/por/export",
            params=params,
            headers={"Accept": "application/x-norito"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return response.content

    def get_sorafs_por_weekly_report(
        self,
        iso_week: Union[str, Tuple[int, int]],
        *,
        timeout: Optional[float] = None,
    ) -> bytes:
        """Fetch the Norito-encoded weekly PoR report for the provided ISO week."""

        label = _normalize_iso_week_label(iso_week, "get_sorafs_por_weekly_report.iso_week")
        response = self._request(
            "GET",
            f"/v1/sorafs/por/report/{label}",
            headers={"Accept": "application/x-norito"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        return response.content

    def get_sorafs_por_ingestion_status(
        self,
        manifest_hex: str,
        *,
        timeout: Optional[float] = None,
    ) -> SorafsPorIngestionStatus:
        """Return the JSON PoR ingestion snapshot for the provided manifest digest."""

        digest = _normalize_hex_string(
            manifest_hex,
            "get_sorafs_por_ingestion_status.manifest_hex",
            expected_length=64,
        )
        response = self._request(
            "GET",
            f"/v1/sorafs/por/ingestion/{digest}",
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, (200,))
        payload = self._maybe_json(response)
        if payload is None or not isinstance(payload, Mapping):
            raise RuntimeError("por ingestion endpoint returned an invalid payload")
        return SorafsPorIngestionStatus.from_payload(payload)

    @staticmethod
    def _pagination_params(
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Dict[str, Any]:
        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        return params

    def get_status(self) -> Optional[Any]:
        """Return Torii node status (`GET /v1/status`, falling back to `/status`)."""

        headers = {"Accept": "application/json"}
        response = self._request("GET", "/v1/status", headers=headers)
        if response.status_code == 404:
            response = self._request("GET", "/status", headers=headers)
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    @staticmethod
    def _status_mapping(status: Optional[Any]) -> Mapping[str, Any]:
        if status is None:
            return {}
        raw = getattr(status, "raw", None)
        if isinstance(raw, Mapping):
            return raw
        nested_status = getattr(status, "status", None)
        nested_raw = getattr(nested_status, "raw", None)
        if isinstance(nested_raw, Mapping):
            return nested_raw
        if isinstance(status, Mapping):
            return status
        if is_dataclass(status):
            return asdict(status)
        raise TypeError("status must be a mapping or typed Torii status object")

    @staticmethod
    def _dataspace_entry_from_lane(entry: Mapping[str, Any]) -> Dict[str, Any]:
        alias = str(
            entry.get("dataspace_alias") or entry.get("dataspace") or entry.get("alias") or ""
        ).strip()
        dataspace_id = entry.get("dataspace_id", entry.get("id"))
        result: Dict[str, Any] = dict(entry)
        if alias:
            result.setdefault("alias", alias)
            result.setdefault("dataspace_alias", alias)
        if dataspace_id is not None:
            try:
                result["id"] = int(dataspace_id)
                result["dataspace_id"] = int(dataspace_id)
            except (TypeError, ValueError):
                pass
        return result

    def list_dataspaces(self, status: Optional[Any] = None) -> List[Mapping[str, Any]]:
        """Return dataspace catalog entries from Torii status.

        Nodes expose dataspace information through slightly different status
        shapes across releases. This helper accepts all supported shapes and
        normalizes entries enough for readiness checks.
        """

        payload = self._status_mapping(self.get_status() if status is None else status)
        indexed: Dict[Tuple[Optional[str], Optional[int]], Dict[str, Any]] = {}
        for key in (
            "teu_dataspace_backlog",
            "dataspaces",
            "dataspace_catalog",
            "teu_lane_commit",
        ):
            entries = payload.get(key)
            if not isinstance(entries, list):
                continue
            for item in entries:
                if not isinstance(item, Mapping):
                    continue
                entry = self._dataspace_entry_from_lane(item)
                alias = str(entry.get("alias") or entry.get("dataspace_alias") or "").strip()
                dataspace_id = entry.get("id", entry.get("dataspace_id"))
                try:
                    normalized_id: Optional[int] = int(dataspace_id) if dataspace_id is not None else None
                except (TypeError, ValueError):
                    normalized_id = None
                key = (alias or None, normalized_id)
                existing = indexed.get(key, {})
                indexed[key] = {**existing, **entry}

        lane_entries = payload.get("lane_governance")
        if isinstance(lane_entries, list):
            sealed_aliases = {
                str(alias)
                for alias in payload.get("lane_governance_sealed_aliases", [])
                if isinstance(alias, str)
            }
            for item in lane_entries:
                if not isinstance(item, Mapping):
                    continue
                entry = self._dataspace_entry_from_lane(item)
                alias = str(entry.get("alias") or entry.get("dataspace_alias") or "").strip()
                if alias:
                    entry["sealed"] = bool(entry.get("sealed", alias in sealed_aliases))
                dataspace_id = entry.get("id", entry.get("dataspace_id"))
                try:
                    normalized_id = int(dataspace_id) if dataspace_id is not None else None
                except (TypeError, ValueError):
                    normalized_id = None
                key = (alias or None, normalized_id)
                existing = indexed.get(key, {})
                indexed[key] = {**existing, **entry}

        return list(indexed.values())

    def get_dataspace(
        self,
        alias_or_id: Union[str, int],
        status: Optional[Any] = None,
    ) -> Optional[Mapping[str, Any]]:
        """Return a dataspace by alias or numeric id, if present in Torii status."""

        needle = str(alias_or_id).strip()
        if not needle:
            raise ValueError("alias_or_id must be non-empty")
        numeric_needle: Optional[int]
        try:
            numeric_needle = int(needle)
        except ValueError:
            numeric_needle = None
        for entry in self.list_dataspaces(status=status):
            aliases = {
                str(entry.get("alias") or "").strip(),
                str(entry.get("dataspace_alias") or "").strip(),
                str(entry.get("dataspace") or "").strip(),
            }
            if needle in aliases:
                return entry
            if numeric_needle is not None:
                raw_id = entry.get("id", entry.get("dataspace_id"))
                try:
                    if int(raw_id) == numeric_needle:
                        return entry
                except (TypeError, ValueError):
                    pass
        return None

    def require_dataspace(
        self,
        alias_or_id: Union[str, int],
        status: Optional[Any] = None,
    ) -> Mapping[str, Any]:
        """Return a dataspace or raise ``KeyError`` with an actionable message."""

        entry = self.get_dataspace(alias_or_id, status=status)
        if entry is None:
            raise KeyError(f"dataspace {alias_or_id!r} is not present in Torii status")
        return entry

    def dataspace_status(
        self,
        alias_or_id: Union[str, int],
        status: Optional[Any] = None,
    ) -> DataspaceStatus:
        """Return a compact readiness status for a dataspace."""

        entry = self.get_dataspace(alias_or_id, status=status)
        if entry is None:
            return DataspaceStatus(
                alias=str(alias_or_id),
                dataspace_id=0,
                lane_id=0,
                found=False,
                ready=False,
                manifest_required=False,
                manifest_ready=False,
                sealed=False,
            )
        alias = str(
            entry.get("dataspace_alias") or entry.get("dataspace") or entry.get("alias") or alias_or_id
        )
        try:
            dataspace_id = int(entry.get("dataspace_id", entry.get("id", 0)))
        except (TypeError, ValueError):
            dataspace_id = 0
        try:
            lane_id = int(entry.get("lane_id", entry.get("index", 0)))
        except (TypeError, ValueError):
            lane_id = 0
        manifest_required = bool(entry.get("manifest_required", False))
        manifest_ready = bool(entry.get("manifest_ready", not manifest_required))
        sealed = bool(entry.get("sealed", False))
        return DataspaceStatus(
            alias=alias,
            dataspace_id=dataspace_id,
            lane_id=lane_id,
            found=True,
            ready=(not sealed and (not manifest_required or manifest_ready)),
            manifest_required=manifest_required,
            manifest_ready=manifest_ready,
            sealed=sealed,
            lane=dict(entry),
        )

    def dataspace_ready(
        self,
        alias_or_id: Union[str, int],
        status: Optional[Any] = None,
    ) -> bool:
        """Return ``True`` when a dataspace exists and is ready for routing."""

        return self.dataspace_status(alias_or_id, status=status).ready

    def smoke_dataspace(
        self,
        alias_or_id: Union[str, int],
        *,
        expected_lane_count: Optional[int] = None,
        expected_dataspace_id: Optional[int] = None,
        status: Optional[Any] = None,
    ) -> DataspaceStatus:
        """Validate dataspace readiness and optionally the configured lane count."""

        payload = self._status_mapping(self.get_status() if status is None else status)
        if expected_lane_count is not None:
            lane_count = payload.get("lane_count")
            if lane_count is None:
                lane_entries = payload.get("lane_governance")
                if not lane_entries:
                    lane_entries = payload.get("dataspace_catalog")
                lane_count = len(lane_entries or [])
            if int(lane_count) < int(expected_lane_count):
                raise AssertionError(
                    f"Torii reports lane_count={lane_count}, expected at least {expected_lane_count}"
                )
        result = self.dataspace_status(alias_or_id, status=payload)
        if not result.found:
            raise AssertionError(f"dataspace {alias_or_id!r} is not present in Torii status")
        if expected_dataspace_id is not None and result.dataspace_id != int(expected_dataspace_id):
            raise AssertionError(
                f"dataspace {alias_or_id!r} has id {result.dataspace_id}, "
                f"expected {int(expected_dataspace_id)}"
            )
        if not result.ready:
            raise AssertionError(
                f"dataspace {alias_or_id!r} is not ready "
                f"(sealed={result.sealed}, manifest_required={result.manifest_required}, "
                f"manifest_ready={result.manifest_ready})"
            )
        return result

    def plan_dataspace(self, spec: DataspaceSpec) -> DataspacePlan:
        """Build manifest/config artifacts for a dataspace without writing files."""

        return _plan_dataspace(spec)

    def write_dataspace_plan(
        self,
        plan: DataspacePlan,
        output_dir: Union[str, Path],
        *,
        force: bool = False,
    ) -> Dict[str, Path]:
        """Write a dataspace plan to ``output_dir``."""

        return _write_dataspace_plan(plan, output_dir, force=force)

    def nexus_lane_lifecycle_status(
        self,
        *,
        timeout: Optional[float] = None,
    ) -> Dict[str, Any]:
        """Fetch the exact current lane catalog and optimistic lifecycle hash."""

        response = self._request(
            "GET",
            "/v1/nexus/lifecycle",
            headers={"Accept": "application/json"},
            timeout=timeout,
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, Mapping):
            raise TypeError("Nexus lane lifecycle status must be a JSON object")
        status = dict(payload)
        if status.get("version") != 1:
            raise ValueError("Nexus lane lifecycle status version must be 1")
        if not isinstance(status.get("nexus_enabled"), bool):
            raise TypeError("Nexus lane lifecycle status `nexus_enabled` must be boolean")
        lane_count = status.get("lane_count")
        if (
            isinstance(lane_count, bool)
            or not isinstance(lane_count, int)
            or lane_count <= 0
            or lane_count > 0xFFFFFFFF
        ):
            raise ValueError(
                "Nexus lane lifecycle status `lane_count` must be in 1..=4294967295"
            )
        lanes = status.get("lanes")
        if not isinstance(lanes, list) or not lanes:
            raise TypeError("Nexus lane lifecycle status `lanes` must be a non-empty list")
        if len(lanes) > 1024:
            raise ValueError("Nexus lane lifecycle status contains more than 1024 lanes")
        if any(not isinstance(lane, Mapping) for lane in lanes):
            raise TypeError("Nexus lane lifecycle status lanes must be JSON objects")
        lane_ids: list[int] = []
        for index, lane in enumerate(lanes):
            lane_id = lane.get("id")
            if (
                isinstance(lane_id, bool)
                or not isinstance(lane_id, int)
                or lane_id < 0
                or lane_id >= lane_count
            ):
                raise ValueError(f"Nexus lane lifecycle status lanes[{index}].id is invalid")
            lane_ids.append(lane_id)
        if lane_ids != sorted(set(lane_ids)):
            raise ValueError("Nexus lane lifecycle status lane ids must be unique and sorted")
        catalog_hash = status.get("catalog_hash")
        if not isinstance(catalog_hash, str) or not catalog_hash.strip():
            raise ValueError("Nexus lane lifecycle status `catalog_hash` must be non-empty")
        incarnation_entries = status.get("incarnations")
        if not isinstance(incarnation_entries, list):
            raise TypeError("Nexus lane lifecycle status `incarnations` must be a list")
        incarnation_ids: list[int] = []
        incarnation_values: set[str] = set()
        for index, entry in enumerate(incarnation_entries):
            if not isinstance(entry, Mapping):
                raise TypeError(
                    f"Nexus lane lifecycle status incarnations[{index}] must be an object"
                )
            lane_id = entry.get("lane_id")
            if isinstance(lane_id, bool) or not isinstance(lane_id, int):
                raise ValueError(
                    f"Nexus lane lifecycle status incarnations[{index}].lane_id is invalid"
                )
            incarnation = entry.get("incarnation")
            if not isinstance(incarnation, str) or not incarnation.strip():
                raise ValueError(
                    f"Nexus lane lifecycle status incarnations[{index}].incarnation is invalid"
                )
            if incarnation in incarnation_values:
                raise ValueError("Nexus lane lifecycle status incarnations must be unique")
            incarnation_ids.append(lane_id)
            incarnation_values.add(incarnation)
        if incarnation_ids != lane_ids:
            raise ValueError(
                "Nexus lane lifecycle status incarnation lane ids must exactly match the catalog"
            )
        incarnation_root = status.get("incarnation_root")
        if not isinstance(incarnation_root, str) or not incarnation_root.strip():
            raise ValueError("Nexus lane lifecycle status `incarnation_root` must be non-empty")
        return status

    def nexus_lane_lifecycle(
        self,
        additions: Sequence[Mapping[str, Any]],
        *,
        fee_payment: Mapping[str, Any],
        retire: Optional[Sequence[int]] = None,
        chain_id: Optional[str] = None,
        authority: Optional[str] = None,
        key_pair: Optional[Any] = None,
        private_key: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        private_key_hex: Optional[str] = None,
        wait: bool = True,
        interval: float = 1.0,
        timeout: Optional[float] = None,
        max_attempts: Optional[int] = None,
    ) -> tuple["SignedTransactionEnvelope", Optional[Any]]:
        """Submit a signed consensus-replayed Nexus lane lifecycle transaction.

        The former operator-only POST shape is deliberately unsupported. Callers
        must provide ``chain_id``, ``authority``, and raw private-key bytes for an
        account holding ``CanSetParameters``. The status commitment is fetched
        once and is never silently refreshed after a stale/concurrent rejection.
        """

        if key_pair is not None or private_key_hex is not None or isinstance(private_key, str):
            raise RuntimeError(
                "operator-only Nexus lifecycle calls are deprecated; provide chain_id, "
                "authority, and private_key bytes to submit SetParameter(nexus_lane_lifecycle_v1)"
            )
        if chain_id is None or not isinstance(chain_id, str) or not chain_id.strip():
            raise ValueError("chain_id is required for signed Nexus lane lifecycle submission")
        if authority is None or not isinstance(authority, str) or not authority.strip():
            raise ValueError("authority is required for signed Nexus lane lifecycle submission")
        if not isinstance(private_key, (bytes, bytearray, memoryview)):
            raise ValueError(
                "private_key bytes are required for signed Nexus lane lifecycle submission"
            )
        private_key_bytes = bytes(private_key)
        if not private_key_bytes:
            raise ValueError("private_key must not be empty")

        if isinstance(additions, (str, bytes, bytearray, memoryview)) or not isinstance(
            additions, Sequence
        ):
            raise TypeError("additions must be a sequence of lane config mappings")
        if len(additions) > 1024:
            raise ValueError("additions must contain at most 1024 lane configs")
        normalized_additions: List[Dict[str, Any]] = []
        addition_ids: set[int] = set()
        addition_aliases: set[str] = set()
        for index, addition in enumerate(additions):
            if not isinstance(addition, Mapping):
                raise TypeError(f"additions[{index}] must be a mapping")
            normalized = dict(addition)
            lane_id = normalized.get("id")
            if (
                isinstance(lane_id, bool)
                or not isinstance(lane_id, int)
                or lane_id < 0
                or lane_id > 0xFFFFFFFF
            ):
                raise ValueError(f"additions[{index}].id must be a u32 integer")
            if lane_id in addition_ids:
                raise ValueError(f"additions[{index}].id duplicates lane {lane_id}")
            addition_ids.add(lane_id)
            alias = normalized.get("alias")
            if not isinstance(alias, str) or not alias.strip():
                raise ValueError(f"additions[{index}].alias must be a non-empty string")
            if alias in addition_aliases:
                raise ValueError(f"additions[{index}].alias duplicates `{alias}`")
            addition_aliases.add(alias)
            normalized_additions.append(normalized)

        if retire is None:
            retire_items: Sequence[int] = ()
        elif isinstance(retire, (str, bytes, bytearray, memoryview)) or not isinstance(
            retire, Sequence
        ):
            raise TypeError("retire must be a sequence of lane ids")
        else:
            retire_items = retire
        if len(retire_items) > 1024:
            raise ValueError("retire must contain at most 1024 lane ids")

        normalized_retire: List[int] = []
        retired_ids: set[int] = set()
        for index, lane_id in enumerate(retire_items):
            if (
                isinstance(lane_id, bool)
                or not isinstance(lane_id, int)
                or lane_id < 0
                or lane_id > 0xFFFFFFFF
            ):
                raise ValueError(f"retire[{index}] must be a u32 integer lane id")
            if lane_id in retired_ids:
                raise ValueError(f"retire[{index}] duplicates lane {lane_id}")
            retired_ids.add(lane_id)
            normalized_retire.append(lane_id)
        if not normalized_additions and not normalized_retire:
            raise ValueError("lane lifecycle plan must add or retire at least one lane")

        status = self.nexus_lane_lifecycle_status(timeout=timeout)
        if not status["nexus_enabled"]:
            raise RuntimeError("Nexus lane lifecycle is disabled on the serving node")
        plan = _json_safe_value(
            {"additions": normalized_additions, "retire": normalized_retire}
        )
        instruction = _require_crypto().Instruction.nexus_lane_lifecycle(
            json.dumps(_json_safe_value(status), sort_keys=True, separators=(",", ":")),
            json.dumps(plan, sort_keys=True, separators=(",", ":")),
        )
        return self.build_and_submit_transaction(
            chain_id.strip(),
            authority.strip(),
            private_key_bytes,
            fee_payment=fee_payment,
            instructions=[instruction],
            wait=wait,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            success_statuses=("Applied",),
            expect_json=True,
        )

    def publish_dataspace_manifest(
        self,
        *,
        authority: str,
        private_key: str,
        uaid: str,
        dataspace: int,
        manifest: Mapping[str, Any],
        reason: Optional[str] = None,
    ) -> Optional[Any]:
        """Publish a Space Directory manifest using dataspace-oriented keywords."""

        request: Dict[str, Any] = {
            "authority": authority,
            "private_key": private_key,
            "manifest": {
                "uaid": uaid,
                "dataspace": dataspace,
                **dict(manifest),
            }
        }
        if reason:
            request["reason"] = reason
        return self.publish_space_directory_manifest(request)

    def revoke_dataspace_manifest(
        self,
        *,
        authority: str,
        private_key: str,
        uaid: str,
        dataspace: int,
        revoked_epoch: int,
        reason: Optional[str] = None,
    ) -> Optional[Any]:
        """Revoke a Space Directory manifest using dataspace-oriented keywords."""

        request: Dict[str, Any] = {
            "authority": authority,
            "private_key": private_key,
            "uaid": uaid,
            "dataspace": dataspace,
            "revoked_epoch": revoked_epoch,
        }
        if reason:
            request["reason"] = reason
        return self.revoke_space_directory_manifest(request)

    def get_status_snapshot_typed(self) -> ToriiStatusSnapshot:
        """Return a typed status snapshot together with derived metrics."""

        payload = self.request_json("GET", "/v1/status", expected_status=(200,))
        if payload is None:
            raise TypeError("status response body was empty")
        if not isinstance(payload, Mapping):
            raise TypeError("status response must be a JSON object")
        status_payload = ToriiStatusPayload.from_payload(payload)
        metrics = self._status_state.record(status_payload)
        return ToriiStatusSnapshot(
            timestamp=time.monotonic(),
            status=status_payload,
            metrics=metrics,
        )

    def get_pipeline_preflight(self) -> ToriiPipelinePreflight:
        """Return pipeline preflight diagnostics (`GET /v1/pipeline/preflight`)."""

        payload = self.request_json(
            "GET", "/v1/pipeline/preflight", expected_status=(200,)
        )
        if payload is None:
            raise TypeError("pipeline preflight response body was empty")
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline preflight response must be a JSON object")
        return ToriiPipelinePreflight.from_payload(payload)

    def get_health(self) -> Optional[Any]:
        """Return Torii health information (`GET /v1/health`)."""

        return self.request_json("GET", "/v1/health", expected_status=(200,))

    def get_configuration(self) -> Mapping[str, Any]:
        """Return the current node configuration as a JSON mapping."""

        snapshot = self.get_configuration_typed()
        return _configuration_snapshot_to_dict(snapshot)

    def get_configuration_typed(self) -> ConfigurationSnapshot:
        """Typed configuration snapshot (`GET /v1/configuration`)."""

        return super().get_configuration()

    def get_confidential_gas_schedule(self) -> Optional[Mapping[str, int]]:
        """Return the confidential verification gas schedule as a mapping, when available."""

        schedule = self.get_confidential_gas_schedule_typed()
        if schedule is None:
            return None
        return schedule.to_payload()

    def get_confidential_gas_schedule_typed(self) -> Optional[ConfidentialGasSchedule]:
        """Typed confidential verification gas schedule."""

        snapshot = self.get_configuration_typed()
        return snapshot.confidential_gas

    def set_confidential_gas_schedule(
        self,
        *,
        proof_base: int,
        per_public_input: int,
        per_proof_byte: int,
        per_nullifier: int,
        per_commitment: int,
    ) -> Optional[Any]:
        """Update the node's confidential verification gas schedule.

        Torii requires the current logger configuration to be supplied alongside updates.
        This helper fetches the latest configuration, reuses the existing ``logger`` section,
        and posts the new ``confidential_gas`` payload.
        """

        schedule = ConfidentialGasSchedule(
            proof_base=int(proof_base),
            per_public_input=int(per_public_input),
            per_proof_byte=int(per_proof_byte),
            per_nullifier=int(per_nullifier),
            per_commitment=int(per_commitment),
        )
        snapshot = self.get_configuration_typed()
        payload = {
            "logger": snapshot.logger.to_payload(),
            "confidential_gas": schedule.to_payload(),
        }
        return self.update_configuration(payload)

    def set_network_gossip_config(
        self,
        *,
        block_gossip_size: int,
        block_gossip_period_ms: int,
        transaction_gossip_size: int,
        transaction_gossip_period_ms: int,
    ) -> Mapping[str, Any]:
        """Update Torii gossip fan-out and interval parameters.

        The helper fetches the latest configuration, preserves the existing logger/queue/gas sections,
        and posts the updated `network` payload so PY6 admin-surface evidence can remain deterministic.
        """

        snapshot = self.get_configuration_typed()
        payload = _configuration_update_payload(snapshot)
        payload["network"] = {
            "block_gossip_size": _normalize_positive_int(
                block_gossip_size, "network.block_gossip_size", allow_zero=False
            ),
            "block_gossip_period_ms": _normalize_positive_int(
                block_gossip_period_ms, "network.block_gossip_period_ms", allow_zero=False
            ),
            "transaction_gossip_size": _normalize_positive_int(
                transaction_gossip_size, "network.transaction_gossip_size", allow_zero=False
            ),
            "transaction_gossip_period_ms": _normalize_positive_int(
                transaction_gossip_period_ms, "network.transaction_gossip_period_ms", allow_zero=False
            ),
        }
        return self.update_configuration(payload)

    def set_queue_capacity(self, *, capacity: int) -> Mapping[str, Any]:
        """Update the transaction queue capacity exposed by `/v1/configuration`.

        The payload reuses the current logger/network/confidential gas configuration so the queue
        change mirrors the node's existing state.
        """

        snapshot = self.get_configuration_typed()
        payload = _configuration_update_payload(snapshot)
        payload["queue"] = {
            "capacity": _normalize_positive_int(capacity, "queue.capacity", allow_zero=False)
        }
        return self.update_configuration(payload)

    def update_configuration(self, payload: Mapping[str, Any]) -> Mapping[str, Any]:
        """Update node configuration (`POST /v1/configuration`)."""

        return super().update_configuration(payload)

    def get_metrics(self, *, as_text: bool = False) -> Optional[Any]:
        """Fetch Torii metrics (`GET /v1/metrics`)."""

        if as_text:
            response = self._request(
                "GET",
                "/v1/metrics",
                headers={"Accept": "text/plain"},
                allow_retry=False,
            )
            self._expect_status(response, {200})
            return response.text
        return self.request_json("GET", "/v1/metrics", expected_status=(200,))

    def get_block(self, height: int) -> Optional[Any]:
        """Fetch a block by height (`GET /v1/blocks/{height}`)."""

        return self.request_json("GET", f"/v1/blocks/{height}", expected_status=(200, 404))

    def list_blocks(
        self,
        *,
        offset_height: Optional[int] = None,
        limit: Optional[int] = None,
    ) -> Optional[Any]:
        """List blocks via `GET /v1/blocks` with optional pagination."""

        params: Dict[str, Any] = {}
        if offset_height is not None:
            params["offset_height"] = int(offset_height)
        if limit is not None:
            params["limit"] = int(limit)
        return self.request_json("GET", "/v1/blocks", params=params or None, expected_status=(200,))

    def get_pipeline_recovery(self, height: int) -> Optional[Any]:
        """Fetch pipeline recovery sidecar for `height` (`GET /v1/pipeline/recovery/{height}`)."""

        response = self._request(
            "GET",
            f"/v1/pipeline/recovery/{int(height)}",
        )
        self._expect_status(response, {200, 404})
        return self._maybe_json(response)

    def get_pipeline_recovery_typed(self, height: int) -> Optional[PipelineRecoverySidecar]:
        """Typed wrapper for :meth:`get_pipeline_recovery`."""

        payload = self.get_pipeline_recovery(height)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise TypeError("pipeline recovery response must be a JSON object")
        return PipelineRecoverySidecar.from_payload(payload)

    def list_peers(self) -> Optional[Any]:
        """List currently online peers (`GET /v1/peers`)."""

        return self.request_json("GET", "/v1/peers", expected_status=(200,))

    def list_peers_typed(self) -> List[PeerInfo]:
        """Return the online peer set as `PeerInfo` structures (`GET /v1/peers`)."""

        payload = self.request_json("GET", "/v1/peers", expected_status=(200,))
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise TypeError("expected list payload from /v1/peers")
        peers: List[PeerInfo] = []
        for entry in payload:
            peers.append(PeerInfo.from_payload(entry))
        return peers

    def list_telemetry_peers_info(self) -> Optional[Any]:
        """Return telemetry metadata from `GET /v1/telemetry/peers-info`."""

        return self.request_json(
            "GET",
            "/v1/telemetry/peers-info",
            expected_status=(200,),
        )

    def list_telemetry_peers_info_typed(self) -> List[PeerTelemetryInfo]:
        """Typed wrapper for :meth:`list_telemetry_peers_info`."""

        payload = self.list_telemetry_peers_info()
        if payload is None:
            return []
        if not isinstance(payload, list):
            raise TypeError("/v1/telemetry/peers-info response must be a list")
        entries: List[PeerTelemetryInfo] = []
        for index, entry in enumerate(payload):
            if not isinstance(entry, Mapping):
                raise TypeError(f"telemetry peers[{index}] must be an object")
            entries.append(PeerTelemetryInfo.from_payload(entry))
        return entries

    def list_kaigi_relays(self) -> Optional[Any]:
        """List registered Kaigi relays (`GET /v1/kaigi/relays`)."""

        response = self._request(
            "GET",
            "/v1/kaigi/relays",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def list_kaigi_relays_typed(self) -> KaigiRelaySummaryList:
        """Typed wrapper for :meth:`list_kaigi_relays`."""

        payload = self.list_kaigi_relays()
        if payload is None:
            raise RuntimeError("kaigi relays endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relays response must be a JSON object")
        return KaigiRelaySummaryList.from_payload(payload)

    def get_kaigi_relay(self, relay_id: str) -> Optional[Any]:
        """Fetch metadata for a specific Kaigi relay (`GET /v1/kaigi/relays/{relay_id}`)."""

        relay_literal = self._normalize_canonical_account_id(relay_id, "relay_id")
        response = self._request(
            "GET",
            f"/v1/kaigi/relays/{quote(relay_literal, safe='')}",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, (200, 404))
        if response.status_code == 404:
            return None
        payload = self._maybe_json(response)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relay detail response must be an object")
        return payload

    def get_kaigi_relay_typed(self, relay_id: str) -> Optional[KaigiRelayDetail]:
        """Typed wrapper for :meth:`get_kaigi_relay`."""

        payload = self.get_kaigi_relay(relay_id)
        if payload is None:
            return None
        return KaigiRelayDetail.from_payload(payload)

    def get_kaigi_relays_health(self) -> Optional[Any]:
        """Fetch aggregated Kaigi relay health metrics (`GET /v1/kaigi/relays/health`)."""

        response = self._request(
            "GET",
            "/v1/kaigi/relays/health",
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, (200,))
        return self._maybe_json(response)

    def get_kaigi_relays_health_typed(self) -> KaigiRelayHealthSnapshot:
        """Typed wrapper for :meth:`get_kaigi_relays_health`."""

        payload = self.get_kaigi_relays_health()
        if payload is None:
            raise RuntimeError("kaigi relays health endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("kaigi relays health response must be an object")
        return KaigiRelayHealthSnapshot.from_payload(payload)

    def get_time_now(self) -> Mapping[str, int]:
        """Return the Network Time Service snapshot as a mapping."""

        snapshot = self.get_time_now_typed()
        return _network_time_snapshot_to_dict(snapshot)

    def get_time_now_typed(self) -> NetworkTimeSnapshot:
        """Typed Network Time Service snapshot."""

        return super().get_time_now()

    def get_time_status(self) -> Mapping[str, Any]:
        """Return Network Time Service diagnostics as a mapping."""

        status = self.get_time_status_typed()
        return _network_time_status_to_dict(status)

    def get_time_status_typed(self) -> NetworkTimeStatus:
        """Typed Network Time Service diagnostics."""

        return super().get_time_status()

    def capture_node_admin_snapshot(
        self,
        *,
        include_peer_telemetry: bool = True,
    ) -> NodeAdminSnapshot:
        """Collect `/v1/configuration`, `/v1/peers`, `/v1/time/*`, and `/v1/node/capabilities` evidence.

        When ``include_peer_telemetry`` is true (default) the helper also fetches
        `/v1/telemetry/peers-info` so roadmap item PY6-P5 can record peer
        instrumentation alongside the core admin surfaces.
        """

        configuration = self.get_configuration_typed()
        peers = self.list_peers_typed()
        time_status = self.get_time_status_typed()
        time_now = self.get_time_now_typed()
        node_capabilities = self.get_node_capabilities_typed()
        telemetry_peers: Optional[List[PeerTelemetryInfo]]
        if include_peer_telemetry:
            telemetry_peers = self.list_telemetry_peers_info_typed()
        else:
            telemetry_peers = None
        return NodeAdminSnapshot(
            configuration=configuration,
            peers=peers,
            time_now=time_now,
            time_status=time_status,
            node_capabilities=node_capabilities,
            telemetry_peers=telemetry_peers,
        )

    # ------------------------------------------------------------------
    # Runtime & admission helpers
    # ------------------------------------------------------------------
    def get_node_capabilities(self) -> Optional[Any]:
        """Fetch node capability advert (`GET /v1/node/capabilities`)."""

        return self.request_json(
            "GET",
            "/v1/node/capabilities",
            expected_status=(200,),
        )

    def get_node_capabilities_typed(self) -> NodeCapabilities:
        """Typed wrapper for :meth:`get_node_capabilities`."""

        payload = self.get_node_capabilities()
        if payload is None:
            raise RuntimeError("node capabilities endpoint returned no payload")
        return NodeCapabilities.from_payload(payload)

    def get_runtime_metrics(self) -> Optional[Any]:
        """Fetch runtime upgrade metrics summary (`GET /v1/runtime/metrics`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/metrics",
            expected_status=(200,),
        )

    def get_runtime_metrics_typed(self) -> RuntimeMetrics:
        """Typed wrapper for :meth:`get_runtime_metrics`."""

        payload = self.get_runtime_metrics()
        if payload is None:
            raise RuntimeError("runtime metrics endpoint returned no payload")
        return RuntimeMetrics.from_payload(payload)

    def get_runtime_abi_active(self) -> Optional[Any]:
        """Fetch the active ABI version (`GET /v1/runtime/abi/active`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/abi/active",
            expected_status=(200,),
        )

    def get_runtime_abi_active_typed(self) -> RuntimeAbiActive:
        """Typed wrapper for :meth:`get_runtime_abi_active`."""

        payload = self.get_runtime_abi_active()
        if payload is None:
            raise RuntimeError("runtime ABI active endpoint returned no payload")
        return RuntimeAbiActive.from_payload(payload)

    def get_runtime_abi_hash(self) -> Optional[Any]:
        """Fetch the canonical ABI hash for the node's active policy (`GET /v1/runtime/abi/hash`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/abi/hash",
            expected_status=(200,),
        )

    def get_runtime_abi_hash_typed(self) -> RuntimeAbiHash:
        """Typed wrapper for :meth:`get_runtime_abi_hash`."""

        payload = self.get_runtime_abi_hash()
        if payload is None:
            raise RuntimeError("runtime ABI hash endpoint returned no payload")
        return RuntimeAbiHash.from_payload(payload)

    def list_runtime_upgrades(self) -> Optional[Any]:
        """List runtime upgrade records (`GET /v1/runtime/upgrades`)."""

        return self.request_json(
            "GET",
            "/v1/runtime/upgrades",
            expected_status=(200,),
        )

    def list_runtime_upgrades_typed(self) -> RuntimeUpgradeListPage:
        """Typed wrapper for :meth:`list_runtime_upgrades`."""

        payload = self.list_runtime_upgrades()
        if payload is None:
            return RuntimeUpgradeListPage(items=[], total=0)
        return RuntimeUpgradeListPage.from_payload(payload)

    def propose_runtime_upgrade(self, manifest: Mapping[str, Any]) -> Dict[str, Any]:
        """Wrap a runtime upgrade manifest into instructions (`POST /v1/runtime/upgrades/propose`)."""

        response = self._request(
            "POST",
            "/v1/runtime/upgrades/propose",
            json_body=dict(manifest),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected runtime upgrade proposal response")
        return payload

    def propose_runtime_upgrade_typed(
        self, manifest: Union[RuntimeUpgradeManifest, Mapping[str, Any]]
    ) -> RuntimeUpgradeActionResponse:
        """Typed wrapper for :meth:`propose_runtime_upgrade`."""

        manifest_payload: Mapping[str, Any]
        if isinstance(manifest, RuntimeUpgradeManifest):
            manifest_payload = manifest.to_payload()
        else:
            manifest_payload = manifest
        payload = self.propose_runtime_upgrade(manifest_payload)
        return RuntimeUpgradeActionResponse.from_payload(payload)

    def activate_runtime_upgrade(self, upgrade_id_hex: str) -> Dict[str, Any]:
        """Generate activation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/activate/{id}`)."""

        path = f"/v1/runtime/upgrades/activate/{upgrade_id_hex.strip()}"
        response = self._request(
            "POST",
            path,
            headers={"Content-Type": "application/json"},
            data=b"",
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected runtime upgrade activation response")
        return payload

    def activate_runtime_upgrade_typed(self, upgrade_id_hex: str) -> RuntimeUpgradeActionResponse:
        """Typed wrapper for :meth:`activate_runtime_upgrade`."""

        payload = self.activate_runtime_upgrade(upgrade_id_hex)
        return RuntimeUpgradeActionResponse.from_payload(payload)

    def cancel_runtime_upgrade(self, upgrade_id_hex: str) -> Dict[str, Any]:
        """Generate cancellation instructions for a runtime upgrade (`POST /v1/runtime/upgrades/cancel/{id}`)."""

        path = f"/v1/runtime/upgrades/cancel/{upgrade_id_hex.strip()}"
        response = self._request(
            "POST",
            path,
            headers={"Content-Type": "application/json"},
            data=b"",
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected runtime upgrade cancellation response")
        return payload

    def cancel_runtime_upgrade_typed(self, upgrade_id_hex: str) -> RuntimeUpgradeActionResponse:
        """Typed wrapper for :meth:`cancel_runtime_upgrade`."""

        payload = self.cancel_runtime_upgrade(upgrade_id_hex)
        return RuntimeUpgradeActionResponse.from_payload(payload)

    def list_account_assets(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        asset_id: Optional[str] = None,
        count_mode: Optional[str] = None,
    ) -> Optional[Any]:
        """List account assets via `GET /v1/accounts/{account_id}/assets` (optional `asset_id`)."""

        canonical_account_id = self._normalize_canonical_account_id(
            account_id, "account_id"
        )
        params = self._pagination_params(limit=limit, offset=offset)
        if count_mode is not None:
            params["count_mode"] = _normalize_count_mode_arg(count_mode)
        asset_id_value = _normalize_optional_string(asset_id, "list_account_assets.asset_id")
        if asset_id_value is not None:
            params["asset_id"] = asset_id_value
        return self.request_json(
            "GET",
            f"/v1/accounts/{quote(canonical_account_id, safe='')}/assets",
            params=params or None,
            expected_status=(200,),
        )

    def list_account_assets_typed(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        asset_id: Optional[str] = None,
        count_mode: Optional[str] = None,
    ) -> AccountAssetsPage:
        """Typed wrapper for :meth:`list_account_assets`."""

        payload = self.list_account_assets(
            account_id,
            limit=limit,
            offset=offset,
            asset_id=asset_id,
            count_mode=count_mode,
        )
        if payload is None:
            raise RuntimeError("account assets endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("account assets response must be a JSON object")
        return AccountAssetsPage.from_payload(payload)

    def list_account_transactions(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        asset_id: Optional[str] = None,
        count_mode: Optional[str] = None,
    ) -> Optional[Any]:
        """List account transactions via `GET /v1/accounts/{account_id}/transactions` (optional `asset_id`)."""

        canonical_account_id = self._normalize_canonical_account_id(
            account_id, "account_id"
        )
        params = self._pagination_params(limit=limit, offset=offset)
        asset_id_value = _normalize_optional_string(
            asset_id,
            "list_account_transactions.asset_id",
        )
        if asset_id_value is not None:
            params["asset_id"] = asset_id_value
        return self.request_json(
            "GET",
            f"/v1/accounts/{quote(canonical_account_id, safe='')}/transactions",
            params=params or None,
            expected_status=(200,),
        )

    def list_account_transactions_typed(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        asset_id: Optional[str] = None,
    ) -> AccountTransactionsPage:
        """Typed wrapper for :meth:`list_account_transactions`."""

        payload = self.list_account_transactions(
            account_id,
            limit=limit,
            offset=offset,
            asset_id=asset_id,
        )
        if payload is None:
            raise RuntimeError("account transactions endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise TypeError("account transactions response must be a JSON object")
        return AccountTransactionsPage.from_payload(payload)

    def query_account_assets(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/accounts/{account_id}/assets/query` with a Norito-style envelope."""

        canonical_account_id = self._normalize_canonical_account_id(
            account_id, "account_id"
        )
        if envelope is not None:
            self._ensure_no_query_args(
                envelope=envelope,
                filter=filter,
                select=select,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                count_mode=count_mode,
                query_name=query_name,
            )
            body = dict(envelope)
        else:
            body = self._build_query_envelope(
                filter=filter,
                select=select,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                count_mode=count_mode,
                query_name=query_name,
            )
        response = self._request(
            "POST",
            f"/v1/accounts/{quote(canonical_account_id, safe='')}/assets/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected account assets query response")
        return payload

    def query_account_assets_typed(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> AccountAssetsPage:
        """Typed wrapper for :meth:`query_account_assets`."""

        payload = self.query_account_assets(
            account_id,
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            query_name=query_name,
            envelope=envelope,
            count_mode=count_mode,
        )
        return AccountAssetsPage.from_payload(payload)

    def query_account_transactions(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/accounts/{account_id}/transactions/query` with a Norito-style envelope."""

        canonical_account_id = self._normalize_canonical_account_id(
            account_id, "account_id"
        )
        if envelope is not None:
            self._ensure_no_query_args(
                envelope=envelope,
                filter=filter,
                select=select,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                count_mode=count_mode,
                query_name=query_name,
            )
            body = dict(envelope)
        else:
            body = self._build_query_envelope(
                filter=filter,
                select=select,
                sort=sort,
                limit=limit,
                offset=offset,
                fetch_size=fetch_size,
                count_mode=count_mode,
                query_name=query_name,
            )
        response = self._request(
            "POST",
            f"/v1/accounts/{quote(canonical_account_id, safe='')}/transactions/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected account transactions query response")
        return payload

    def query_account_transactions_typed(
        self,
        account_id: str,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
        envelope: Optional[Mapping[str, Any]] = None,
    ) -> AccountTransactionsPage:
        """Typed wrapper for :meth:`query_account_transactions`."""

        payload = self.query_account_transactions(
            account_id,
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
            envelope=envelope,
        )
        return AccountTransactionsPage.from_payload(payload)

    # ------------------------------------------------------------------
    # UAID portfolio & Space Directory surfaces
    # ------------------------------------------------------------------

    def get_uaid_portfolio(
        self,
        uaid: str,
        *,
        asset_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Fetch the aggregated UAID portfolio (`GET /v1/accounts/{uaid}/portfolio`).

        Use ``asset_id`` to filter the response to a specific asset identifier.
        """

        literal = _normalize_uaid_literal(uaid)
        params: Dict[str, Any] = {}
        asset_id_value = _normalize_optional_string(
            asset_id, "get_uaid_portfolio.asset_id"
        )
        if asset_id_value is not None:
            params["asset_id"] = asset_id_value
        response = self._request(
            "GET",
            f"/v1/accounts/{literal}/portfolio",
            params=self._clean_params(params),
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected UAID portfolio response")
        return payload

    def get_uaid_portfolio_typed(
        self,
        uaid: str,
        *,
        asset_id: Optional[str] = None,
    ) -> UaidPortfolioSnapshot:
        """Typed wrapper for :meth:`get_uaid_portfolio`."""

        payload = self.get_uaid_portfolio(uaid, asset_id=asset_id)
        return UaidPortfolioSnapshot.from_payload(payload)

    def get_uaid_bindings(
        self,
        uaid: str,
    ) -> Dict[str, Any]:
        """Fetch UAID dataspace bindings (`GET /v1/space-directory/uaids/{uaid}`)."""

        literal = _normalize_uaid_literal(uaid)
        response = self._request(
            "GET",
            f"/v1/space-directory/uaids/{literal}",
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected UAID bindings response")
        return payload

    def get_uaid_bindings_typed(
        self,
        uaid: str,
    ) -> UaidBindingsSnapshot:
        """Typed wrapper for :meth:`get_uaid_bindings`."""

        payload = self.get_uaid_bindings(uaid)
        return UaidBindingsSnapshot.from_payload(payload)

    def list_space_directory_manifests(
        self,
        uaid: str,
        *,
        dataspace: Optional[int] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Dict[str, Any]:
        """List Space Directory manifests bound to a UAID (`GET /v1/space-directory/uaids/{uaid}/manifests`)."""

        literal = _normalize_uaid_literal(uaid)
        params: Dict[str, Any] = {}
        if dataspace is not None:
            params["dataspace"] = _normalize_positive_int(
                dataspace,
                "dataspace",
                allow_zero=True,
            )
        if status is not None:
            if not isinstance(status, str):
                raise TypeError("status must be a string when provided")
            normalized_status = status.strip().lower()
            if normalized_status not in {"active", "inactive", "all"}:
                raise ValueError("status must be one of {'active', 'inactive', 'all'}")
            params["status"] = normalized_status
        if limit is not None:
            params["limit"] = _normalize_positive_int(limit, "limit", allow_zero=False)
        if offset is not None:
            params["offset"] = _normalize_positive_int(offset, "offset", allow_zero=True)
        response = self._request(
            "GET",
            f"/v1/space-directory/uaids/{literal}/manifests",
            params=params or None,
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected Space Directory manifests response")
        return payload

    def list_space_directory_manifests_typed(
        self,
        uaid: str,
        *,
        dataspace: Optional[int] = None,
        status: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> SpaceDirectoryManifestList:
        """Typed wrapper for :meth:`list_space_directory_manifests`."""

        payload = self.list_space_directory_manifests(
            uaid,
            dataspace=dataspace,
            status=status,
            limit=limit,
            offset=offset,
        )
        return SpaceDirectoryManifestList.from_payload(payload)

    def publish_space_directory_manifest(
        self,
        request: Mapping[str, Any],
    ) -> Optional[Any]:
        """Publish or rotate a Space Directory manifest (`POST /v1/space-directory/manifests`)."""

        payload = _normalize_publish_space_directory_manifest_request(request)
        response = self._request(
            "POST",
            "/v1/space-directory/manifests",
            json_body=payload,
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {202})
        return self._maybe_json(response)

    def revoke_space_directory_manifest(
        self,
        request: Mapping[str, Any],
    ) -> Optional[Any]:
        """Revoke a Space Directory manifest (`POST /v1/space-directory/manifests/revoke`)."""

        payload = _normalize_revoke_space_directory_manifest_request(request)
        response = self._request(
            "POST",
            "/v1/space-directory/manifests/revoke",
            json_body=payload,
            headers={"Accept": "application/json"},
        )
        self._expect_status(response, {202})
        return self._maybe_json(response)

    def _handle_sorafs_alias_warning(self, warning: SorafsAliasWarning) -> None:
        """Internal hook for alias-proof warnings."""

        self._sorafs_alias_metrics["warnings"] = self._sorafs_alias_metrics.get("warnings", 0) + 1
        if self._sorafs_alias_warning_hook:
            self._sorafs_alias_warning_hook(warning)

    def _enforce_sorafs_alias_policy(
        self,
        response: requests.Response,
    ) -> Optional[SorafsAliasEvaluation]:
        """Validate SoraFS alias proofs stapled on HTTP responses."""

        try:
            evaluation = enforce_sorafs_alias_policy(
                response,
                policy=self._sorafs_alias_policy,
                warning_hook=self._handle_sorafs_alias_warning,
                logger=self._sorafs_alias_logger,
            )
        except SorafsAliasError as exc:
            raise RuntimeError(f"failed to validate SoraFS alias proof: {exc}") from exc
        if evaluation is None:
            self._last_sorafs_alias_evaluation = None
            return None
        self._last_sorafs_alias_evaluation = evaluation
        self._sorafs_alias_metrics["total"] = self._sorafs_alias_metrics.get("total", 0) + 1
        label = evaluation.status_label or evaluation.state
        self._sorafs_alias_metrics[label] = self._sorafs_alias_metrics.get(label, 0) + 1
        return evaluation

    def _request(
        self,
        method: str,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[Mapping[str, str]] = None,
        data: Optional[bytes] = None,
        json_body: Optional[Mapping[str, Any]] = None,
        timeout: Optional[float] = None,
        allow_retry: bool = True,
        allow_redirects: bool = True,
    ) -> requests.Response:
        if json_body is not None and data is not None:
            raise ValueError("provide either `json_body` or `data`, not both")

        final_headers: Dict[str, str] = dict(self._default_headers)
        if headers:
            for name, value in headers.items():
                _set_exact_header(final_headers, str(name), str(value))

        payload: Optional[bytes]
        if json_body is not None:
            payload = json.dumps(json_body).encode("utf-8")
            final_headers.setdefault("Content-Type", "application/json")
        else:
            payload = data

        method_upper = method.upper()
        retry_enabled = allow_retry and method_upper in self._retry_methods
        max_attempts = 1 + (self._max_retries if retry_enabled else 0)
        request_timeout = timeout if timeout is not None else self._timeout
        url = f"{self._base_url}{path}"

        delay = self._backoff_initial
        for attempt in range(max_attempts):
            try:
                response = self._session.request(
                    method_upper,
                    url,
                    params=params,
                    headers=final_headers or None,
                    data=payload,
                    timeout=request_timeout,
                    allow_redirects=allow_redirects,
                )
            except requests.RequestException:
                if attempt == max_attempts - 1:
                    raise
                delay = self._apply_backoff(delay)
                continue

            if (
                retry_enabled
                and response.status_code in self._retry_statuses
                and attempt < max_attempts - 1
            ):
                delay = self._apply_backoff(delay)
                continue

            self._enforce_sorafs_alias_policy(response)
            return response

        raise RuntimeError("exhausted retries without receiving a response")

    def _apply_backoff(self, current_delay: float) -> float:
        delay = current_delay
        if delay <= 0.0 and self._backoff_initial > 0.0:
            delay = self._backoff_initial
        if delay > 0.0:
            time.sleep(delay)
        if delay <= 0.0:
            return 0.0
        next_delay = delay * self._backoff_multiplier
        if self._backoff_cap != math.inf:
            next_delay = min(self._backoff_cap, next_delay)
        return next_delay

    def build_and_submit_transaction(
        self,
        chain_id: str,
        authority: str,
        private_key: bytes,
        *,
        fee_payment: Mapping[str, Any],
        instructions: Iterable["Instruction"] = (),
        creation_time_ms: Optional[int] = None,
        ttl_ms: Optional[int] = None,
        nonce: Optional[int] = None,
        metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        scope: str = "global",
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
        expect_json: bool = True,
        envelope_format: str = "object",
    ) -> tuple["SignedTransactionEnvelope", Optional[Any]]:
        """Build, submit, and optionally wait for a transaction to finalize.

        When `expect_json` is true, an empty or non-dict response from the
        submission endpoint is normalised to `{}` so callers can assume a mapping.
        """

        crypto = _require_crypto()
        envelope = crypto.build_signed_transaction(
            chain_id,
            authority,
            private_key,
            fee_payment=fee_payment,
            instructions=instructions,
            creation_time_ms=creation_time_ms,
            ttl_ms=ttl_ms,
            nonce=nonce,
            metadata=metadata,
        )
        if envelope_format == "object":
            envelope_out: Union[SignedTransactionEnvelope, Dict[str, Any], str] = envelope
        elif envelope_format == "dict":
            envelope_out = envelope.as_dict()
        elif envelope_format == "json":
            envelope_out = envelope.to_json()
        else:
            raise ValueError(
                "envelope_format must be one of {'object', 'dict', 'json'}"
            )
        if wait:
            result = self.submit_transaction_envelope_and_wait(
                envelope,
                interval=interval,
                timeout=timeout,
                max_attempts=max_attempts,
                scope=scope,
                success_statuses=success_statuses,
                failure_statuses=failure_statuses,
                on_status=on_status,
            )
            return envelope_out, result
        response = self.submit_transaction_envelope(envelope)
        if expect_json and response is None:
            response = {}
        return envelope_out, response

    def get_transaction_status(
        self,
        hash_hex: str,
        *,
        scope: str = "global",
        timeout: Optional[float] = None,
    ) -> Optional[Any]:
        """Fetch transaction pipeline status for the given hash (hex encoded)."""

        scope = _normalize_transaction_status_scope(scope, "get_transaction_status.scope")
        response = self._request(
            "GET",
            "/v1/pipeline/transactions/status",
            params={"hash": hash_hex, "scope": scope},
            timeout=timeout,
        )
        if response.status_code == 404:
            return None
        self._expect_status(response, {200, 202, 204})
        return self._maybe_json(response)

    def wait_for_transaction_status(
        self,
        hash_hex: str,
        *,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        scope: str = "global",
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
    ) -> Any:
        """Poll pipeline status until the transaction reaches a terminal state.

        Returns the final payload when the transaction reports one of the
        `success_statuses`. Raises :class:`TransactionStatusError` if a failure
        status is encountered, or :class:`TimeoutError` if neither a success nor
        a failure status is observed within the configured bounds.
        """

        scope = _normalize_transaction_status_scope(scope, "wait_for_transaction_status.scope")
        success_set = (
            frozenset(str(s) for s in success_statuses)
            if success_statuses is not None
            else _DEFAULT_SUCCESS_STATUSES
        )
        failure_set = (
            frozenset(str(s) for s in failure_statuses)
            if failure_statuses is not None
            else _DEFAULT_FAILURE_STATUSES
        )

        attempts = 0
        deadline = None if timeout is None else (time.monotonic() + max(timeout, 0.0))

        while True:
            request_timeout = None
            if deadline is not None:
                remaining = deadline - time.monotonic()
                if remaining <= 0.0:
                    raise TimeoutError(
                        f"transaction {hash_hex} did not reach a terminal status "
                        f"within {timeout} seconds"
                    )
                request_timeout = (
                    remaining if self._timeout is None else min(self._timeout, remaining)
                )
            attempts += 1
            payload = self.get_transaction_status(
                hash_hex,
                scope=scope,
                timeout=request_timeout,
            )
            status = _extract_pipeline_status_kind(payload)

            if on_status is not None:
                on_status(status, payload, attempts)

            if status is not None:
                if status in success_set:
                    return payload
                if status in failure_set:
                    raise TransactionStatusError(hash_hex, status, payload)

            if max_attempts is not None and attempts >= max_attempts:
                raise TimeoutError(
                    f"transaction {hash_hex} did not reach a terminal status "
                    f"after {attempts} attempts"
                )

            if deadline is not None and time.monotonic() >= deadline:
                raise TimeoutError(
                    f"transaction {hash_hex} did not reach a terminal status "
                    f"within {timeout} seconds"
                )

            if interval > 0.0:
                if deadline is None:
                    time.sleep(interval)
                else:
                    sleep_for = min(interval, max(deadline - time.monotonic(), 0.0))
                    if sleep_for > 0.0:
                        time.sleep(sleep_for)

    # ------------------------------------------------------------------
    # Transaction construction convenience helpers
    # ------------------------------------------------------------------

    @staticmethod
    def compose_asset_id(
        asset_definition_id: str,
        account_id: str,
        *,
        scope: Optional[str] = None,
    ) -> str:
        """Build a canonical asset balance bucket literal."""

        definition = _require_non_empty_string(
            asset_definition_id,
            "compose_asset_id.asset_definition_id",
        )
        account = _require_non_empty_string(account_id, "compose_asset_id.account_id")
        literal = f"{definition}#{account}"
        if scope:
            scope_text = _require_non_empty_string(scope, "compose_asset_id.scope")
            if scope_text.isdigit():
                scope_text = f"dataspace:{scope_text}"
            literal = f"{literal}#{scope_text}"
        return literal

    @staticmethod
    def _envelope_hash_hex(envelope: "SignedTransactionEnvelope") -> str:
        hash_field = getattr(envelope, "hash", None)
        if hash_field is None:
            raise ValueError("SignedTransactionEnvelope.hash is required to poll status")
        if isinstance(hash_field, memoryview):
            hash_field = hash_field.tobytes()
        if isinstance(hash_field, (bytes, bytearray)):
            return bytes(hash_field).hex()
        if isinstance(hash_field, str):
            return hash_field
        raise TypeError(
            "SignedTransactionEnvelope.hash must be bytes or hex string, "
            f"got {type(hash_field)!r}"
        )

    def _transaction_draft(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        ttl_ms: Optional[int] = 900_000,
        nonce: Optional[int] = None,
        metadata: Optional[Mapping[str, Any]] = None,
    ) -> "TransactionDraft":
        from .tx import TransactionConfig, TransactionDraft

        effective_chain_id = _require_exact_non_empty_string(chain_id, "chain_id")
        effective_authority = self._native_transaction_account_id(
            _require_exact_non_empty_string(authority, "authority"),
            "authority",
        )
        return TransactionDraft(
            TransactionConfig(
                chain_id=effective_chain_id,
                authority=effective_authority,
                fee_payment=fee_payment,
                ttl_ms=ttl_ms,
                nonce=nonce,
                metadata=metadata,
            )
        )

    def _submit_transaction_draft_result(
        self,
        draft: "TransactionDraft",
        *,
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        wait: bool = True,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        scope: str = "global",
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
        on_status: Optional[Callable[[Optional[str], Any, int], None]] = None,
        **sign_overrides: Any,
    ) -> Mapping[str, Any]:
        envelope = self._sign_transaction_draft(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            instructions=None,
            **sign_overrides,
        )
        hash_hex = self._envelope_hash_hex(envelope)
        try:
            status = self.submit_transaction_envelope(envelope)
        except (requests.RequestException, TimeoutError) as exc:
            if wait:
                raise
            status = {
                "ok": False,
                "status": "submission_timeout_pending_status",
                "error": str(exc),
            }
        result: Dict[str, Any] = {
            "envelope": envelope,
            "hash": hash_hex,
            "submission": status,
        }
        if wait:
            result["terminal"] = self.wait_for_transaction_status(
                hash_hex,
                interval=interval,
                timeout=timeout,
                max_attempts=max_attempts,
                scope=scope,
                success_statuses=success_statuses,
                failure_statuses=failure_statuses,
                on_status=on_status,
            )
        return result

    def submit_instructions_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        instructions: Iterable["Instruction"],
        wait: bool = True,
        ttl_ms: Optional[int] = 900_000,
        nonce: Optional[int] = None,
        metadata: Optional[Mapping[str, Any]] = None,
        interval: float = 1.0,
        timeout: Optional[float] = 30.0,
        max_attempts: Optional[int] = None,
        scope: str = "global",
    ) -> Mapping[str, Any]:
        """Submit arbitrary instructions in one signed transaction."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            ttl_ms=ttl_ms,
            nonce=nonce,
            metadata=metadata,
        )
        draft.extend_instructions(instructions)
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            interval=interval,
            timeout=timeout,
            max_attempts=max_attempts,
            scope=scope,
        )

    def register_domain_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        domain_id: str,
        domain_metadata: Optional[Mapping[str, Any]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Register a domain and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.register_domain(domain_id, metadata=domain_metadata)
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def register_account_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        account_id: str,
        account_metadata: Optional[Mapping[str, Any]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Register an account and optionally wait for commit."""

        return self.register_accounts_and_wait(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            private_key=private_key,
            private_key_hex=private_key_hex,
            accounts=[account_id],
            account_metadata={account_id: account_metadata or {}},
            transaction_metadata=transaction_metadata,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def register_accounts_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        accounts: Iterable[str],
        account_metadata: Optional[Mapping[str, Mapping[str, Any]]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Register multiple accounts in one transaction."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        metadata_by_account = dict(account_metadata or {})
        registered = 0
        for account_id in accounts:
            native_account_id = self._native_transaction_account_id(
                account_id,
                f"accounts[{registered}]",
            )
            draft.register_account(
                native_account_id,
                metadata=(
                    metadata_by_account.get(account_id)
                    or metadata_by_account.get(native_account_id)
                ),
            )
            registered += 1
        if registered == 0:
            raise ValueError("accounts must contain at least one account id")
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def grant_account_permission_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        account_id: str,
        permission_name: str,
        permission_payload: Any = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Grant one account permission and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.grant_account_permission(
            self._native_transaction_account_id(account_id, "account_id"),
            permission_name,
            payload=permission_payload,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def revoke_account_permission_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        account_id: str,
        permission_name: str,
        permission_payload: Any = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Revoke one account permission and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.revoke_account_permission(
            self._native_transaction_account_id(account_id, "account_id"),
            permission_name,
            payload=permission_payload,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def register_asset_definition_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        definition_id: str,
        owner: str,
        name: Optional[str] = None,
        description: Optional[str] = None,
        alias: Optional[str] = None,
        scale: Optional[Union[int, str]] = None,
        mintable: Optional[str] = "Infinitely",
        balance_scope_policy: Optional[str] = None,
        confidential_policy: Optional[str] = None,
        asset_metadata: Optional[Mapping[str, Any]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Register a quantity asset definition and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.register_asset_definition(
            definition_id,
            self._native_transaction_account_id(owner, "owner"),
            name=name,
            description=description,
            alias=alias,
            scale=scale,
            mintable=mintable,
            balance_scope_policy=balance_scope_policy,
            confidential_policy=confidential_policy,
            metadata=asset_metadata,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def mint_asset_quantity_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_id: str,
        quantity: QuantityLike,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Mint an exact nominal asset quantity and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.mint_asset_quantity(
            self._native_transaction_asset_id(asset_id, "asset_id"),
            quantity,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def mint_asset_and_wait(self, **kwargs: Any) -> Mapping[str, Any]:
        """Alias for :meth:`mint_asset_quantity_and_wait`."""

        return self.mint_asset_quantity_and_wait(**kwargs)

    def mint_assets_quantity_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        mints: Iterable[Mapping[str, Any]],
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Mint multiple exact nominal asset quantities in one transaction."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        count = 0
        for index, record in enumerate(mints):
            if not isinstance(record, Mapping):
                raise TypeError(f"mints[{index}] must be a mapping")
            asset_id = _require_non_empty_string(
                record.get("asset_id"),
                f"mints[{index}].asset_id",
            )
            if "quantity" not in record:
                raise TypeError(f"mints[{index}].quantity is required")
            draft.mint_asset_quantity(
                self._native_transaction_asset_id(
                    asset_id,
                    f"mints[{index}].asset_id",
                ),
                record["quantity"],
            )
            count += 1
        if count == 0:
            raise ValueError("mints must contain at least one mint record")
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def mint_assets_and_wait(self, **kwargs: Any) -> Mapping[str, Any]:
        """Alias for :meth:`mint_assets_quantity_and_wait`."""

        return self.mint_assets_quantity_and_wait(**kwargs)

    def burn_asset_quantity_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_id: str,
        quantity: QuantityLike,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Burn an exact nominal asset quantity and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.burn_asset_quantity(
            self._native_transaction_asset_id(asset_id, "asset_id"),
            quantity,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def burn_asset_and_wait(self, **kwargs: Any) -> Mapping[str, Any]:
        """Alias for :meth:`burn_asset_quantity_and_wait`."""

        return self.burn_asset_quantity_and_wait(**kwargs)

    def transfer_asset_quantity_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_id: str,
        quantity: QuantityLike,
        destination: str,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Transfer an exact nominal asset quantity and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.transfer_asset_quantity(
            self._native_transaction_asset_id(asset_id, "asset_id"),
            quantity,
            self._native_transaction_account_id(destination, "destination"),
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def transfer_asset_and_wait(self, **kwargs: Any) -> Mapping[str, Any]:
        """Alias for :meth:`transfer_asset_quantity_and_wait`."""

        return self.transfer_asset_quantity_and_wait(**kwargs)

    def open_asset_lock_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        escrow_id: str,
        asset_definition_id: str,
        destination: str,
        amount: QuantityLike,
        release_authority: Optional[str] = None,
        expires_at_ms: Optional[int] = None,
        evidence_hashes: Optional[Sequence[Any]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Open a native asset lock and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.open_asset_lock(
            escrow_id,
            asset_definition_id,
            self._native_transaction_account_id(destination, "destination"),
            amount,
            release_authority=(
                self._native_transaction_account_id(
                    release_authority,
                    "release_authority",
                )
                if release_authority is not None
                else None
            ),
            expires_at_ms=expires_at_ms,
            evidence_hashes=evidence_hashes,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def drawdown_asset_lock_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        escrow_id: str,
        amount: QuantityLike,
        expected_remaining_amount: QuantityLike,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Draw down a native asset lock and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.drawdown_asset_lock(escrow_id, amount, expected_remaining_amount)
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def cancel_asset_lock_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        escrow_id: str,
        expected_remaining_amount: QuantityLike,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Cancel from an exact bounded lock-ID preimage and optionally await commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.cancel_asset_lock(escrow_id, expected_remaining_amount)
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def expire_asset_lock_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        escrow_id: str,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Expire a native asset lock and optionally wait for commit."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.expire_asset_lock(escrow_id)
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def transfer_assets_quantity_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        transfers: Iterable[Mapping[str, Any]],
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Transfer multiple exact nominal asset quantities in one transaction."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        count = 0
        for index, record in enumerate(transfers):
            if not isinstance(record, Mapping):
                raise TypeError(f"transfers[{index}] must be a mapping")
            asset_id = _require_non_empty_string(
                record.get("asset_id"),
                f"transfers[{index}].asset_id",
            )
            destination = _require_non_empty_string(
                record.get("destination"),
                f"transfers[{index}].destination",
            )
            if "quantity" not in record:
                raise TypeError(f"transfers[{index}].quantity is required")
            draft.transfer_asset_quantity(
                self._native_transaction_asset_id(
                    asset_id,
                    f"transfers[{index}].asset_id",
                ),
                record["quantity"],
                self._native_transaction_account_id(
                    destination,
                    f"transfers[{index}].destination",
                ),
            )
            count += 1
        if count == 0:
            raise ValueError("transfers must contain at least one transfer record")
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def transfer_assets_and_wait(self, **kwargs: Any) -> Mapping[str, Any]:
        """Alias for :meth:`transfer_assets_quantity_and_wait`."""

        return self.transfer_assets_quantity_and_wait(**kwargs)

    def register_zk_asset_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_definition_id: str,
        mode: str = "Hybrid",
        allow_shield: bool = True,
        allow_unshield: bool = True,
        vk_transfer: Optional[Union[str, Mapping[str, Any]]] = None,
        vk_unshield: Optional[Union[str, Mapping[str, Any]]] = None,
        vk_shield: Optional[Union[str, Mapping[str, Any]]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Register ZK policy metadata for an asset definition."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.register_zk_asset(
            asset_definition_id,
            mode=mode,
            allow_shield=allow_shield,
            allow_unshield=allow_unshield,
            vk_transfer=vk_transfer,
            vk_unshield=vk_unshield,
            vk_shield=vk_shield,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def verify_proof_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        proof: Mapping[str, Any],
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Submit a generic `zk::VerifyProof` instruction."""

        if not isinstance(proof, Mapping):
            raise TypeError("proof must be a mapping")
        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.verify_proof(dict(proof))
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def register_asset_hidden_zk_pool_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        pool_id: str,
        storage_asset: str,
        asset_set_root: Union[str, bytes, bytearray, memoryview],
        vk_transfer: Union[str, Mapping[str, Any]],
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Register asset-hidden shielded pool verifier state."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.register_asset_hidden_zk_pool(
            pool_id,
            storage_asset,
            asset_set_root=asset_set_root,
            vk_transfer=vk_transfer,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def register_zk_ace_identity_commitment_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_definition_id: str,
        identity_commitment: Union[str, bytes, bytearray, memoryview],
        policy_hash: Union[str, bytes, bytearray, memoryview],
        allowed_accounts: Sequence[str],
        verifier_key: Union[str, Mapping[str, Any]],
        action_class: Optional[str] = None,
        domain_tag: Optional[str] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Register a ZK-ACE identity commitment for transparent-transfer authorization."""

        normalized_identity_commitment = _normalize_zk_ace_hex32(
            identity_commitment,
            "identity_commitment",
        )
        normalized_policy_hash = _normalize_zk_ace_hex32(policy_hash, "policy_hash")
        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.register_zk_ace_identity_commitment(
            asset_definition_id,
            identity_commitment=normalized_identity_commitment,
            policy_hash=normalized_policy_hash,
            allowed_accounts=[
                self._native_transaction_account_id(
                    account_id,
                    f"allowed_accounts[{index}]",
                )
                for index, account_id in enumerate(allowed_accounts)
            ],
            verifier_key=verifier_key,
            action_class=action_class,
            domain_tag=domain_tag,
        )
        result = self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )
        if not wait:
            return result
        try:
            state = _zk_ace_committed_identity_state(
                self.get_asset_definition(asset_definition_id),
                identity_commitment=normalized_identity_commitment,
                policy_hash=normalized_policy_hash,
                allowed_accounts=allowed_accounts,
                chain_id=chain_id,
                verifier_key=verifier_key,
                action_class=action_class,
                domain_tag=domain_tag,
            )
        except Exception:
            state = None
        return _zk_ace_enrich_result(
            result,
            key="identity_commitment_state",
            state=state,
        )

    def rotate_zk_ace_identity_commitment_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_definition_id: str,
        old_identity_commitment: Union[str, bytes, bytearray, memoryview],
        new_identity_commitment: Union[str, bytes, bytearray, memoryview],
        policy_hash: Union[str, bytes, bytearray, memoryview],
        allowed_accounts: Sequence[str],
        verifier_key: Union[str, Mapping[str, Any]],
        action_class: Optional[str] = None,
        domain_tag: Optional[str] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Rotate an active ZK-ACE identity commitment to a replacement commitment."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.rotate_zk_ace_identity_commitment(
            asset_definition_id,
            old_identity_commitment=old_identity_commitment,
            new_identity_commitment=new_identity_commitment,
            policy_hash=policy_hash,
            allowed_accounts=[
                self._native_transaction_account_id(
                    account_id,
                    f"allowed_accounts[{index}]",
                )
                for index, account_id in enumerate(allowed_accounts)
            ],
            verifier_key=verifier_key,
            action_class=action_class,
            domain_tag=domain_tag,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def revoke_zk_ace_identity_commitment_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_definition_id: str,
        identity_commitment: Union[str, bytes, bytearray, memoryview],
        reason_hash: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Revoke an active ZK-ACE identity commitment."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.revoke_zk_ace_identity_commitment(
            asset_definition_id,
            identity_commitment=identity_commitment,
            reason_hash=reason_hash,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def shield_asset_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_definition_id: str,
        from_account_id: str,
        amount: QuantityLike,
        note_commitment: Union[str, bytes, bytearray, memoryview],
        ephemeral_public_key: Union[str, bytes, bytearray, memoryview],
        nonce: Union[str, bytes, bytearray, memoryview],
        ciphertext: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        ciphertext_b64: Optional[str] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Shield public funds into an asset's ZK ledger."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.shield_asset(
            asset_definition_id,
            self._native_transaction_account_id(from_account_id, "from_account_id"),
            amount,
            note_commitment=note_commitment,
            ephemeral_public_key=ephemeral_public_key,
            nonce=nonce,
            ciphertext=ciphertext,
            ciphertext_b64=ciphertext_b64,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def zk_transfer_prepared_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_definition_id: str,
        inputs: Iterable[Union[str, bytes, bytearray, memoryview]],
        outputs: Iterable[Union[str, bytes, bytearray, memoryview]],
        proof: Mapping[str, Any],
        root_hint: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Submit a prepared private-to-private ZK transfer."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.zk_transfer_prepared(
            asset_definition_id,
            inputs=inputs,
            outputs=outputs,
            proof=proof,
            root_hint=root_hint,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def unshield_prepared_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        asset_definition_id: str,
        to_account_id: str,
        public_amount: QuantityLike,
        inputs: Iterable[Union[str, bytes, bytearray, memoryview]],
        proof: Mapping[str, Any],
        outputs: Optional[Iterable[Union[str, bytes, bytearray, memoryview]]] = None,
        root_hint: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Submit a prepared ZK unshield transaction."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.unshield_prepared(
            asset_definition_id,
            self._native_transaction_account_id(to_account_id, "to_account_id"),
            public_amount,
            inputs=inputs,
            proof=proof,
            outputs=outputs,
            root_hint=root_hint,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def asset_hidden_zk_transfer_prepared_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        pool_id: str,
        inputs: Iterable[Union[str, bytes, bytearray, memoryview]],
        outputs: Iterable[Union[str, bytes, bytearray, memoryview]],
        proof: Mapping[str, Any],
        root_hint: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Submit a prepared asset-hidden ZK transfer."""

        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.asset_hidden_zk_transfer_prepared(
            pool_id,
            inputs=inputs,
            outputs=outputs,
            proof=proof,
            root_hint=root_hint,
        )
        return self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )

    def zk_ace_authorized_transfer_and_wait(
        self,
        *,
        chain_id: str,
        authority: str,
        fee_payment: Mapping[str, Any],
        private_key: Optional[bytes] = None,
        private_key_hex: Optional[str] = None,
        from_account_id: str,
        to_account_id: str,
        asset_definition_id: str,
        amount: Union[str, int],
        identity_commitment: Union[str, bytes, bytearray, memoryview],
        tx_digest: Union[str, bytes, bytearray, memoryview],
        domain_tag: str,
        action_class: str,
        replay_nullifier: Union[str, bytes, bytearray, memoryview],
        policy_hash: Union[str, bytes, bytearray, memoryview],
        proof: Mapping[str, Any],
        transaction_metadata: Optional[Mapping[str, Any]] = None,
        wait: bool = True,
        timeout: Optional[float] = 30.0,
        interval: float = 1.0,
    ) -> Mapping[str, Any]:
        """Submit a prepared ZK-ACE-authorized transparent transfer."""

        normalized_identity_commitment = _normalize_zk_ace_hex32(
            identity_commitment,
            "identity_commitment",
        )
        normalized_tx_digest = _normalize_zk_ace_hex32(tx_digest, "tx_digest")
        normalized_replay_nullifier = _normalize_zk_ace_hex32(
            replay_nullifier,
            "replay_nullifier",
        )
        normalized_policy_hash = _normalize_zk_ace_hex32(policy_hash, "policy_hash")
        draft = self._transaction_draft(
            chain_id=chain_id,
            authority=authority,
            fee_payment=fee_payment,
            metadata=transaction_metadata,
        )
        draft.zk_ace_authorized_transfer(
            from_account_id=self._native_transaction_account_id(
                from_account_id,
                "from_account_id",
            ),
            to_account_id=self._native_transaction_account_id(
                to_account_id,
                "to_account_id",
            ),
            asset_definition_id=asset_definition_id,
            amount=amount,
            identity_commitment=normalized_identity_commitment,
            tx_digest=normalized_tx_digest,
            chain_id=chain_id,
            domain_tag=domain_tag,
            action_class=action_class,
            replay_nullifier=normalized_replay_nullifier,
            policy_hash=normalized_policy_hash,
            proof=proof,
        )
        result = self._submit_transaction_draft_result(
            draft,
            private_key=private_key,
            private_key_hex=private_key_hex,
            wait=wait,
            timeout=timeout,
            interval=interval,
        )
        if not wait:
            return result
        try:
            state = _zk_ace_committed_transfer_state(
                self.get_asset_definition(asset_definition_id),
                identity_commitment=normalized_identity_commitment,
                tx_digest=normalized_tx_digest,
                replay_nullifier=normalized_replay_nullifier,
                policy_hash=normalized_policy_hash,
                source_account=from_account_id,
                chain_id=chain_id,
                verifier_key=proof.get("verifying_key_ref"),
                action_class=action_class,
                domain_tag=domain_tag,
            )
        except Exception:
            state = None
        return _zk_ace_enrich_result(result, key="replay_state", state=state)

    # ------------------------------------------------------------------
    # Ledger account and asset convenience helpers
    # ------------------------------------------------------------------

    @staticmethod
    def account_id_variants(
        account_id: str,
        *,
        include_taira_prefix_variant: bool = False,
    ) -> List[str]:
        """Return account-id literals to try for REST reads.

        Taira public ingress has historically exposed both ``testu`` and
        ``sorau`` I105 sentinels during rollout windows. SDK callers can opt in
        to trying the alternate sentinel without copying that compatibility
        rule into application code.
        """

        literal = _require_non_empty_string(account_id, "account_id")
        variants = [literal]
        if include_taira_prefix_variant:
            if literal.startswith("testu"):
                variants.append(f"sorau{literal[5:]}")
            elif literal.startswith("sorau"):
                variants.append(f"testu{literal[5:]}")
        return _dedupe_strings(variants)

    @staticmethod
    def _normalize_account_id_variants(
        account_id: str,
        account_id_variants: Optional[Iterable[str]],
        *,
        include_taira_prefix_variant: bool,
    ) -> List[str]:
        if account_id_variants is None:
            return ToriiClient.account_id_variants(
                account_id,
                include_taira_prefix_variant=include_taira_prefix_variant,
            )
        variants = [
            _require_non_empty_string(item, "account_id_variants[]")
            for item in account_id_variants
        ]
        if account_id not in variants:
            variants.insert(0, account_id)
        return _dedupe_strings(variants)

    def _account_record_from_listing(
        self,
        account_id: str,
        *,
        account_id_variants: Optional[Iterable[str]] = None,
        include_taira_prefix_variant: bool = False,
        limit: int = 200,
    ) -> Optional[Mapping[str, Any]]:
        candidates = set(
            self._normalize_account_id_variants(
                account_id,
                account_id_variants,
                include_taira_prefix_variant=include_taira_prefix_variant,
            )
        )
        offset = 0
        while True:
            payload = self.list_accounts(limit=limit, offset=offset)
            items = _extract_page_items(payload)
            for item in items:
                candidate = str(item.get("id") or item.get("account_id") or "")
                if candidate in candidates:
                    return item
            batch_size = len(items)
            total = _page_total(payload)
            if batch_size == 0:
                return None
            offset += batch_size
            if total is not None and offset >= total:
                return None

    def find_account(
        self,
        account_id: str,
        *,
        account_id_variants: Optional[Iterable[str]] = None,
        include_taira_prefix_variant: bool = False,
    ) -> Optional[Mapping[str, Any]]:
        """Fetch an account by id, returning ``None`` when it is absent.

        The helper retries supplied account-id variants and falls back to the
        paginated account list when Torii reports route or network-prefix
        compatibility errors.
        """

        variants = self._normalize_account_id_variants(
            account_id,
            account_id_variants,
            include_taira_prefix_variant=include_taira_prefix_variant,
        )
        saw_network_prefix_error = False
        for candidate in variants:
            response = self._request(
                "GET",
                f"/v1/accounts/{quote(candidate, safe='')}",
            )
            if response.status_code == 404:
                continue
            if response.status_code == 200:
                payload = self._maybe_json(response)
                if not isinstance(payload, Mapping):
                    raise RuntimeError("account endpoint returned non-object payload")
                return payload
            if (
                response.status_code == 503
                and response.headers.get("x-iroha-reject-code") == "route_unavailable"
            ):
                return self._account_record_from_listing(
                    account_id,
                    account_id_variants=variants,
                )
            if _response_has_network_prefix_error(response):
                saw_network_prefix_error = True
                continue
            self._expect_status(response, {200, 404})
        if saw_network_prefix_error:
            return self._account_record_from_listing(
                account_id,
                account_id_variants=variants,
            )
        return None

    def account_exists(
        self,
        account_id: str,
        *,
        account_id_variants: Optional[Iterable[str]] = None,
        include_taira_prefix_variant: bool = False,
    ) -> bool:
        """Return whether Torii can see the account."""

        return self.find_account(
            account_id,
            account_id_variants=account_id_variants,
            include_taira_prefix_variant=include_taira_prefix_variant,
        ) is not None

    def find_account_assets(
        self,
        account_id: str,
        *,
        account_id_variants: Optional[Iterable[str]] = None,
        include_taira_prefix_variant: bool = False,
        asset_id: Optional[str] = None,
    ) -> Optional[List[Mapping[str, Any]]]:
        """Return raw account asset entries, or ``None`` when the account is absent."""

        result = self._find_account_assets_for_variants(
            account_id,
            account_id_variants=account_id_variants,
            include_taira_prefix_variant=include_taira_prefix_variant,
            asset_id=asset_id,
        )
        if result is None:
            return None
        _resolved_account_id, items = result
        return items

    def _find_account_assets_for_variants(
        self,
        account_id: str,
        *,
        account_id_variants: Optional[Iterable[str]] = None,
        include_taira_prefix_variant: bool = False,
        asset_id: Optional[str] = None,
    ) -> Optional[Tuple[str, List[Mapping[str, Any]]]]:
        variants = self._normalize_account_id_variants(
            account_id,
            account_id_variants,
            include_taira_prefix_variant=include_taira_prefix_variant,
        )
        params: Dict[str, Any] = {}
        asset_id_value = _normalize_optional_string(asset_id, "find_account_assets.asset_id")
        if asset_id_value is not None:
            params["asset_id"] = asset_id_value
        last_network_prefix_error: Optional[requests.Response] = None
        for candidate in variants:
            response = self._request(
                "GET",
                f"/v1/accounts/{quote(candidate, safe='')}/assets",
                params=params or None,
            )
            if response.status_code == 404:
                continue
            if response.status_code == 200:
                return candidate, _extract_page_items(self._maybe_json(response))
            if _response_has_network_prefix_error(response):
                last_network_prefix_error = response
                continue
            self._expect_status(response, {200, 404})
        if last_network_prefix_error is not None:
            self._expect_status(last_network_prefix_error, {200, 404})
        return None

    def find_account_asset_items(
        self,
        account_id: str,
        asset_definition_id: str,
        *,
        account_id_variants: Optional[Iterable[str]] = None,
        include_taira_prefix_variant: bool = False,
    ) -> List[Mapping[str, Any]]:
        """Return raw asset entries matching an asset definition for an account."""

        definition = _require_non_empty_string(
            asset_definition_id,
            "asset_definition_id",
        )
        result = self._find_account_assets_for_variants(
            account_id,
            account_id_variants=account_id_variants,
            include_taira_prefix_variant=include_taira_prefix_variant,
        )
        if result is None:
            return []
        resolved_account_id, items = result
        return [
            item
            for item in items
            if _asset_entry_matches_definition(item, definition, resolved_account_id)
        ]

    def asset_balance(
        self,
        account_id: str,
        asset_definition_id: str,
        *,
        account_id_variants: Optional[Iterable[str]] = None,
        include_taira_prefix_variant: bool = False,
    ) -> Decimal:
        """Return an exact canonical quantity parsed from account asset listings."""

        definition = _require_non_empty_string(
            asset_definition_id,
            "asset_definition_id",
        )
        result = self._find_account_assets_for_variants(
            account_id,
            account_id_variants=account_id_variants,
            include_taira_prefix_variant=include_taira_prefix_variant,
        )
        if result is None:
            raise RuntimeError(f"account {account_id} not found via Torii REST")
        resolved_account_id, items = result
        for item in items:
            if _asset_entry_matches_definition(item, definition, resolved_account_id):
                return _quantity_decimal(item.get("quantity"))
        return Decimal("0")

    def get_asset_definition(
        self,
        asset_definition_id: str,
    ) -> Optional[Mapping[str, Any]]:
        """Fetch an asset definition by id, returning ``None`` for 404."""

        definition = _require_non_empty_string(
            asset_definition_id,
            "asset_definition_id",
        )
        response = self._request(
            "GET",
            f"/v1/assets/definitions/{quote(definition, safe='')}",
        )
        if response.status_code == 404:
            return None
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, Mapping):
            raise RuntimeError("asset definition endpoint returned non-object payload")
        return payload

    def asset_definition_exists(self, asset_definition_id: str) -> bool:
        """Return whether Torii can resolve the asset definition."""

        return self.get_asset_definition(asset_definition_id) is not None

    def get_account_faucet_puzzle(self) -> Mapping[str, Any]:
        """Return the account-faucet proof-of-work puzzle."""

        payload = self.request_json(
            "GET",
            "/v1/accounts/faucet/puzzle",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise RuntimeError("account faucet puzzle endpoint returned non-object payload")
        return payload

    @staticmethod
    def solve_account_faucet_pow(
        account_id: str,
        puzzle: Mapping[str, Any],
        *,
        max_nonce: int = 1_000_000,
    ) -> Tuple[int, str]:
        """Solve a Torii account-faucet proof-of-work puzzle."""

        account = _require_non_empty_string(account_id, "account_id")
        difficulty_bits = int(puzzle["difficulty_bits"])
        anchor_height = int(puzzle["anchor_height"])
        anchor_hash = bytes.fromhex(str(puzzle["anchor_block_hash_hex"]))
        challenge_salt_hex = puzzle.get("challenge_salt_hex")
        challenge_salt = bytes.fromhex(str(challenge_salt_hex)) if challenge_salt_hex else None
        scrypt_n = 1 << int(puzzle["scrypt_log_n"])
        scrypt_r = int(puzzle["scrypt_r"])
        scrypt_p = int(puzzle["scrypt_p"])
        challenge = hashlib.sha256(
            b"".join(
                (
                    ACCOUNT_FAUCET_POW_DOMAIN_SEPARATOR,
                    account.encode("utf-8"),
                    anchor_height.to_bytes(8, "big"),
                    anchor_hash,
                    challenge_salt or b"",
                )
            )
        ).digest()
        for nonce in range(max_nonce):
            nonce_bytes = nonce.to_bytes(8, "big")
            digest = hashlib.scrypt(
                nonce_bytes,
                salt=challenge,
                n=scrypt_n,
                r=scrypt_r,
                p=scrypt_p,
                dklen=32,
            )
            if _leading_zero_bits(digest) >= difficulty_bits:
                return anchor_height, nonce_bytes.hex()
        raise RuntimeError(
            f"could not solve account faucet proof-of-work after {max_nonce} attempts"
        )

    def submit_account_faucet_registration(
        self,
        account_id: str,
        *,
        puzzle: Optional[Mapping[str, Any]] = None,
        max_nonce: int = 1_000_000,
    ) -> requests.Response:
        """Submit an account-faucet registration and return the raw response."""

        puzzle_payload = puzzle or self.get_account_faucet_puzzle()
        anchor_height, nonce_hex = self.solve_account_faucet_pow(
            account_id,
            puzzle_payload,
            max_nonce=max_nonce,
        )
        return self.submit_account_faucet_claim(
            account_id,
            pow_anchor_height=anchor_height,
            pow_nonce_hex=nonce_hex,
        )

    def submit_account_faucet_claim(
        self,
        account_id: str,
        *,
        pow_anchor_height: int,
        pow_nonce_hex: str,
    ) -> requests.Response:
        """Submit a pre-solved account-faucet proof-of-work claim."""

        return self._request(
            "POST",
            "/v1/accounts/faucet",
            json_body={
                "account_id": _require_non_empty_string(account_id, "account_id"),
                "pow_anchor_height": int(pow_anchor_height),
                "pow_nonce_hex": _require_non_empty_string(
                    pow_nonce_hex,
                    "pow_nonce_hex",
                ),
            },
        )

    def onboard_account(
        self,
        *,
        onboarding_token: str,
        alias: str,
        uaid: str,
        account_id: Optional[str] = None,
        public_key_hex: Optional[str] = None,
        identity_commitment_hex: Optional[str] = None,
        permissions: Optional[Sequence[str]] = None,
    ) -> requests.Response:
        """Submit a JSON-only account onboarding request with an explicit route credential."""

        exact_onboarding_token = _require_account_onboarding_token(onboarding_token)
        if (account_id is None) == (public_key_hex is None):
            raise ValueError(
                "onboard_account requires exactly one of account_id or public_key_hex"
            )
        payload: Dict[str, Any] = {
            "alias": _require_non_empty_string(alias, "onboard_account.alias"),
            "uaid": _normalize_uaid_literal(uaid, context="onboard_account.uaid"),
        }
        if account_id is not None:
            canonical_account_id = self._normalize_canonical_account_id(
                account_id,
                "onboard_account.account_id",
            )
            if "@" in canonical_account_id:
                raise ValueError("onboard_account.account_id must be a canonical I105 account id")
            payload["account_id"] = canonical_account_id
        else:
            assert public_key_hex is not None
            payload["public_key_hex"] = _normalize_32_byte_hex(
                public_key_hex,
                "onboard_account.public_key_hex",
            )
        if identity_commitment_hex is not None:
            payload["identity_commitment_hex"] = _normalize_32_byte_hex(
                identity_commitment_hex,
                "onboard_account.identity_commitment_hex",
            )
        if permissions is not None:
            if isinstance(permissions, (str, bytes, bytearray)):
                raise TypeError("onboard_account.permissions must be a sequence of strings")
            normalized_permissions: List[str] = []
            for index, permission in enumerate(permissions):
                normalized = _require_non_empty_string(
                    permission,
                    f"onboard_account.permissions[{index}]",
                )
                if normalized not in normalized_permissions:
                    normalized_permissions.append(normalized)
            if normalized_permissions:
                payload["permissions"] = normalized_permissions
        return self._request(
            "POST",
            "/v1/accounts/onboard",
            headers={
                "Accept": "application/json",
                "Content-Type": "application/json",
                ACCOUNT_ONBOARDING_TOKEN_HEADER: exact_onboarding_token,
            },
            json_body=payload,
            allow_retry=False,
            allow_redirects=False,
        )

    def find_domain(self, domain_id: str, *, limit: int = 200) -> Optional[Mapping[str, Any]]:
        """Fetch a domain by id, falling back to paginated listing on route gaps."""

        resolved_domain_id = _require_non_empty_string(domain_id, "domain_id")
        response = self._request(
            "GET",
            f"/v1/domains/{quote(resolved_domain_id, safe='')}",
        )
        if response.status_code == 200:
            payload = self._maybe_json(response)
            if not isinstance(payload, Mapping):
                raise RuntimeError("domain endpoint returned non-object payload")
            return payload
        if response.status_code not in {404, 502, 503, 504}:
            self._expect_status(response, {200, 404})

        offset = 0
        while True:
            payload = self.list_domains(limit=limit, offset=offset)
            items = _extract_page_items(payload)
            for item in items:
                candidate = str(item.get("id") or item.get("domain_id") or "")
                if candidate == resolved_domain_id:
                    return item
            batch_size = len(items)
            total = _page_total(payload)
            if batch_size == 0:
                return None
            offset += batch_size
            if total is not None and offset >= total:
                return None

    def domain_exists(self, domain_id: str) -> bool:
        """Return whether Torii can resolve a domain."""

        return self.find_domain(domain_id) is not None

    def request_sns_name(self, namespace: str, literal: str) -> requests.Response:
        """Fetch an SNS name registration and return the raw response."""

        return self._request(
            "GET",
            "/v1/sns/names/"
            f"{quote(_require_non_empty_string(namespace, 'namespace'), safe='')}/"
            f"{quote(_require_non_empty_string(literal, 'literal'), safe='')}",
        )

    def get_sns_name(self, namespace: str, literal: str) -> Optional[Mapping[str, Any]]:
        """Fetch an SNS name registration, returning ``None`` on 404."""

        response = self.request_sns_name(namespace, literal)
        if response.status_code == 404:
            return None
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, Mapping):
            raise RuntimeError("SNS name endpoint returned non-object payload")
        return payload

    def get_sns_policy(self, suffix_id: int) -> Mapping[str, Any]:
        """Fetch the SNS suffix policy."""

        payload = self.request_json(
            "GET",
            f"/v1/sns/policies/{int(suffix_id)}",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise RuntimeError("SNS policy endpoint returned non-object payload")
        return payload

    def request_zk_verifying_key(self, backend: str, name: str) -> requests.Response:
        """Fetch a ZK verifying-key registry entry and return the raw response."""

        return self._request(
            "GET",
            "/v1/zk/vk/"
            f"{quote(_require_production_verify_backend_label(backend, 'backend'), safe='')}/"
            f"{quote(_require_exact_non_empty_string(name, 'name'), safe='')}",
        )

    def get_zk_verifying_key(self, backend: str, name: str) -> Optional[Mapping[str, Any]]:
        """Fetch a ZK verifying-key registry entry, returning ``None`` on 404."""

        response = self.request_zk_verifying_key(backend, name)
        if response.status_code == 404:
            return None
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, Mapping):
            raise RuntimeError("ZK verifying-key endpoint returned non-object payload")
        return payload

    def zk_verifying_key_active(self, backend: str, name: str) -> bool:
        """Return whether a ZK verifying key exists and is active."""

        payload = self.get_zk_verifying_key(backend, name)
        if payload is None:
            return False
        record = payload.get("record") if isinstance(payload, Mapping) else None
        return isinstance(record, Mapping) and record.get("status") == "Active"

    def submit_zk_verifying_key_registration(
        self,
        payload: Mapping[str, Any],
    ) -> requests.Response:
        """Submit a ZK verifying-key registration request and return the raw response."""

        return self._request(
            "POST",
            "/v1/zk/vk/register",
            json_body=_normalize_zk_verifying_key_registration_payload(payload),
            timeout=60.0,
        )

    def register_zk_verifying_key(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """Submit a ZK verifying-key registration request and decode the response."""

        response = self.submit_zk_verifying_key_registration(payload)
        self._expect_status(response, {200, 201, 202, 409})
        return self._maybe_json(response)

    def submit_zk_verifying_key_update(
        self,
        payload: Mapping[str, Any],
    ) -> requests.Response:
        """Submit a ZK verifying-key update request and return the raw response."""

        return self._request(
            "POST",
            "/v1/zk/vk/update",
            json_body=_normalize_zk_verifying_key_update_payload(payload),
            timeout=60.0,
        )

    def update_zk_verifying_key(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """Submit a ZK verifying-key update request and decode the response."""

        response = self.submit_zk_verifying_key_update(payload)
        self._expect_status(response, {200, 201, 202, 409})
        return self._maybe_json(response)

    def account_has_permission(
        self,
        account_id: str,
        permission_name: str,
        *,
        expected_payload: Optional[Mapping[str, Any]] = None,
    ) -> bool:
        """Return whether an account has a direct permission token."""

        expected_payload_value = (
            _json_safe_value(dict(expected_payload)) if expected_payload is not None else None
        )
        permissions = self.list_account_permissions_typed(account_id)
        for permission in permissions.items:
            if permission.name != permission_name:
                continue
            if expected_payload is None or permission.payload == expected_payload_value:
                return True
        return False

    # ------------------------------------------------------------------
    # RWA queries
    # ------------------------------------------------------------------

    def query_rwas(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute POST `/v1/rwas/query` with a structured envelope."""

        body = rwa_query_envelope(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            "/v1/rwas/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected RWA query response")
        return payload

    def query_rwas_typed(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> RwaListPage:
        """Typed wrapper for :meth:`query_rwas`."""

        payload = self.query_rwas(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        return RwaListPage.from_payload(payload)

    def list_rwas(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> Optional[Any]:
        """List chain-state RWAs via `GET /v1/rwas`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        if count_mode is not None:
            params["count_mode"] = _normalize_count_mode_arg(count_mode)
        filter_arg = _encode_filter_arg(filter)
        if filter_arg is not None:
            params["filter"] = filter_arg
        sort_arg = _encode_sort_arg(sort)
        if sort_arg is not None:
            params["sort"] = sort_arg
        return self.request_json(
            "GET",
            "/v1/rwas",
            params=params or None,
            expected_status=(200,),
        )

    def list_rwas_typed(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> RwaListPage:
        """Typed wrapper for :meth:`list_rwas`."""

        payload = self.list_rwas(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            count_mode=count_mode,
        )
        if payload is None:
            return RwaListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("RWA list endpoint returned non-object payload")
        return RwaListPage.from_payload(payload)

    # ------------------------------------------------------------------
    # Account queries
    # ------------------------------------------------------------------
    def query_accounts(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute POST `/v1/accounts/query` with a structured envelope."""

        body = account_query_envelope(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            "/v1/accounts/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected accounts query response")
        return payload

    def query_accounts_typed(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> AccountListPage:
        """Typed wrapper for :meth:`query_accounts`."""

        payload = self.query_accounts(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        return AccountListPage.from_payload(payload)

    def list_accounts(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> Optional[Any]:
        """List accounts via `GET /v1/accounts`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        if count_mode is not None:
            params["count_mode"] = _normalize_count_mode_arg(count_mode)
        filter_arg = _encode_filter_arg(filter)
        if filter_arg is not None:
            params["filter"] = filter_arg
        sort_arg = _encode_sort_arg(sort)
        if sort_arg is not None:
            params["sort"] = sort_arg
        return self.request_json(
            "GET",
            "/v1/accounts",
            params=params or None,
            expected_status=(200,),
        )

    def list_accounts_typed(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> AccountListPage:
        """Typed wrapper for :meth:`list_accounts`."""

        payload = self.list_accounts(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            count_mode=count_mode,
        )
        if payload is None:
            return AccountListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("accounts endpoint returned non-object payload")
        return AccountListPage.from_payload(payload)

    def list_domains(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> Optional[Any]:
        """List domains via `GET /v1/domains`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        if count_mode is not None:
            params["count_mode"] = _normalize_count_mode_arg(count_mode)
        filter_arg = _encode_filter_arg(filter)
        if filter_arg is not None:
            params["filter"] = filter_arg
        sort_arg = _encode_sort_arg(sort)
        if sort_arg is not None:
            params["sort"] = sort_arg
        return self.request_json(
            "GET",
            "/v1/domains",
            params=params or None,
            expected_status=(200,),
        )

    def list_domains_typed(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> DomainListPage:
        """Typed wrapper for :meth:`list_domains`."""

        payload = self.list_domains(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            count_mode=count_mode,
        )
        if payload is None:
            return DomainListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("domains endpoint returned non-object payload")
        return DomainListPage.from_payload(payload)

    def query_asset_definitions(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/assets/definitions/query` with a structured envelope."""

        body = asset_definitions_query_envelope(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            "/v1/assets/definitions/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected assets definitions query response")
        return payload

    def query_asset_definitions_typed(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> AssetDefinitionListPage:
        """Typed wrapper for :meth:`query_asset_definitions`."""

        payload = self.query_asset_definitions(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        return AssetDefinitionListPage.from_payload(payload)

    def list_asset_definitions(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> Optional[Any]:
        """List asset definitions via `GET /v1/assets/definitions`."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        if count_mode is not None:
            params["count_mode"] = _normalize_count_mode_arg(count_mode)
        filter_arg = _encode_filter_arg(filter)
        if filter_arg is not None:
            params["filter"] = filter_arg
        sort_arg = _encode_sort_arg(sort)
        if sort_arg is not None:
            params["sort"] = sort_arg
        return self.request_json(
            "GET",
            "/v1/assets/definitions",
            params=params or None,
            expected_status=(200,),
        )

    def list_asset_definitions_typed(
        self,
        *,
        filter: Optional[Any] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        count_mode: Optional[str] = None,
    ) -> AssetDefinitionListPage:
        """Typed wrapper for :meth:`list_asset_definitions`."""

        payload = self.list_asset_definitions(
            filter=filter,
            sort=sort,
            limit=limit,
            offset=offset,
            count_mode=count_mode,
        )
        if payload is None:
            return AssetDefinitionListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("asset definitions endpoint returned non-object payload")
        return AssetDefinitionListPage.from_payload(payload)

    def query_domains(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/domains/query` with a structured envelope."""

        body = domain_query_envelope(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            "/v1/domains/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected domains query response")
        return payload

    def query_domains_typed(
        self,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> DomainListPage:
        """Typed wrapper for :meth:`query_domains`."""

        payload = self.query_domains(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        return DomainListPage.from_payload(payload)

    def query_asset_holders(
        self,
        asset_definition_id: str,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/assets/{definition}/holders/query` with a structured envelope."""

        body = asset_holders_query_envelope(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            f"/v1/assets/{asset_definition_id}/holders/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected asset holders query response")
        return payload

    def query_asset_holders_typed(
        self,
        asset_definition_id: str,
        *,
        filter: Optional[Dict[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: int = 0,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> AssetHolderListPage:
        """Typed wrapper for :meth:`query_asset_holders`."""

        payload = self.query_asset_holders(
            asset_definition_id,
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        return AssetHolderListPage.from_payload(payload)

    def list_asset_holders(
        self,
        asset_definition_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        asset_id: Optional[str] = None,
        count_mode: Optional[str] = None,
    ) -> Optional[Any]:
        """List asset holders via `GET /v1/assets/{definition}/holders` (optional `asset_id`)."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        if count_mode is not None:
            params["count_mode"] = _normalize_count_mode_arg(count_mode)
        asset_id_value = _normalize_optional_string(
            asset_id,
            "list_asset_holders.asset_id",
        )
        if asset_id_value is not None:
            params["asset_id"] = asset_id_value
        return self.request_json(
            "GET",
            f"/v1/assets/{asset_definition_id}/holders",
            params=params or None,
            expected_status=(200,),
        )

    def list_asset_holders_typed(
        self,
        asset_definition_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        asset_id: Optional[str] = None,
        count_mode: Optional[str] = None,
    ) -> AssetHolderListPage:
        """Typed wrapper for :meth:`list_asset_holders`."""

        payload = self.list_asset_holders(
            asset_definition_id,
            limit=limit,
            offset=offset,
            asset_id=asset_id,
            count_mode=count_mode,
        )
        if payload is None:
            return AssetHolderListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("asset holders endpoint returned non-object payload")
        return AssetHolderListPage.from_payload(payload)

    def list_account_permissions(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """List account permissions via `GET /v1/accounts/{account_id}/permissions`."""

        canonical_account_id = self._normalize_canonical_account_id(
            account_id, "account_id"
        )
        params = self._pagination_params(limit=limit, offset=offset)
        return self.request_json(
            "GET",
            f"/v1/accounts/{quote(canonical_account_id, safe='')}/permissions",
            params=params or None,
            expected_status=(200,),
        )

    def list_account_permissions_typed(
        self,
        account_id: str,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> AccountPermissionListPage:
        """Typed wrapper for :meth:`list_account_permissions`."""

        payload = self.list_account_permissions(account_id, limit=limit, offset=offset)
        if payload is None:
            return AccountPermissionListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("account permissions endpoint returned non-object payload")
        return AccountPermissionListPage.from_payload(payload)

    # ------------------------------------------------------------------
    # Contracts API
    # ------------------------------------------------------------------
    @staticmethod
    def _contract_response_payload(response: Any) -> Any:
        if is_dataclass(response):
            return asdict(response)
        return _json_safe_value(response)

    @staticmethod
    def _contract_response_tx_hashes(response: Any) -> List[str]:
        payload = ToriiClient._contract_response_payload(response)
        hashes: List[str] = []

        def visit(value: Any) -> None:
            if isinstance(value, Mapping):
                for key in ("tx_hash_hex", "hash", "tx_hash", "transaction_hash"):
                    candidate = value.get(key)
                    if isinstance(candidate, str) and candidate.strip():
                        hashes.append(candidate.strip())
                for child in value.values():
                    visit(child)
            elif isinstance(value, list):
                for child in value:
                    visit(child)

        visit(payload)
        return _dedupe_strings(hashes)

    @staticmethod
    def _contract_response_pipeline_statuses(response: Any) -> List[Mapping[str, Any]]:
        payload = ToriiClient._contract_response_payload(response)
        statuses: List[Mapping[str, Any]] = []
        seen: set[tuple[str, str, str]] = set()

        def add(candidate: Any) -> None:
            if not isinstance(candidate, Mapping):
                return
            # Operation receipts also carry a string-valued ``status`` and a
            # transaction hash. Treating those as pipeline snapshots lets a
            # nested ``status: submitted`` shadow an authoritative embedded
            # ``status: {kind: Committed}`` for the same hash and triggers an
            # unnecessary (or impossible) poll. Only accept the typed pipeline
            # response shapes emitted by Torii.
            direct_status = candidate.get("status")
            content = candidate.get("content")
            content_status = content.get("status") if isinstance(content, Mapping) else None
            if not (
                isinstance(direct_status, Mapping)
                and direct_status.get("kind") is not None
            ) and not (
                isinstance(content_status, Mapping)
                and content_status.get("kind") is not None
            ):
                return
            kind = _extract_pipeline_status_kind(candidate)
            if kind is None:
                return
            status_hash = candidate.get("hash") or candidate.get("tx_hash_hex")
            key = (
                str(status_hash or ""),
                kind,
                json.dumps(_json_safe_value(candidate), sort_keys=True, default=str),
            )
            if key in seen:
                return
            seen.add(key)
            statuses.append(candidate)

        def visit(value: Any) -> None:
            if isinstance(value, Mapping):
                add(value.get("pipeline_status"))
                add(value)
                for child in value.values():
                    visit(child)
            elif isinstance(value, list):
                for child in value:
                    visit(child)

        visit(payload)
        return statuses

    def _wait_for_contract_response(
        self,
        response: Any,
        *,
        timeout_ms: Optional[int],
        interval: float,
        scope: str,
        success_statuses: Optional[Iterable[str]],
        failure_statuses: Optional[Iterable[str]],
    ) -> Dict[str, Any]:
        submit_payload = self._contract_response_payload(response)
        tx_hashes = self._contract_response_tx_hashes(response)
        embedded_statuses = self._contract_response_pipeline_statuses(response)
        embedded_by_hash: Dict[str, Mapping[str, Any]] = {}
        for status_payload in embedded_statuses:
            status_hash = status_payload.get("hash") or status_payload.get("tx_hash_hex")
            if isinstance(status_hash, str) and status_hash.strip():
                embedded_by_hash[status_hash.strip().lower()] = status_payload
        success_set = (
            frozenset(str(s) for s in success_statuses)
            if success_statuses is not None
            else _DEFAULT_SUCCESS_STATUSES
        )
        failure_set = (
            frozenset(str(s) for s in failure_statuses)
            if failure_statuses is not None
            else _DEFAULT_FAILURE_STATUSES
        )
        final_payloads: List[Any] = []
        for tx_hash in tx_hashes:
            embedded_status = embedded_by_hash.get(tx_hash.lower())
            if embedded_status is None and len(tx_hashes) == 1 and len(embedded_statuses) == 1:
                embedded_status = embedded_statuses[0]
            embedded_kind = _extract_pipeline_status_kind(embedded_status)
            if embedded_kind is not None:
                if embedded_kind in success_set:
                    final_payloads.append(embedded_status)
                    continue
                if embedded_kind in failure_set:
                    raise TransactionStatusError(tx_hash, embedded_kind, embedded_status)
            final_payloads.append(
                self.wait_for_transaction_status(
                    tx_hash,
                    interval=interval,
                    timeout=None if timeout_ms is None else timeout_ms / 1000.0,
                    scope=scope,
                    success_statuses=success_statuses,
                    failure_statuses=failure_statuses,
                )
            )
        final_payload: Any
        if not final_payloads:
            final_payload = None
        elif len(final_payloads) == 1:
            final_payload = final_payloads[0]
        else:
            final_payload = {"items": final_payloads}
        return {
            "submit": submit_payload,
            "tx_hashes": tx_hashes,
            "terminal_kind": _extract_pipeline_status_kind(final_payload),
            "r#final": final_payload,
        }

    def register_contract_code(self, manifest: Mapping[str, Any]) -> Optional[Any]:
        response = self._request(
            "POST",
            "/v1/contracts/code",
            data=json.dumps(manifest).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200, 202})
        return self._maybe_json(response)


    def call_contract_and_wait(
        self,
        *,
        authority: str,
        private_key: str,
        fee_payment: Mapping[str, Any],
        entrypoint: str,
        contract_address: Optional[str] = None,
        contract_alias: Optional[str] = None,
        payload: Any = None,
        wait: bool = True,
        timeout_ms: Optional[int] = 120_000,
        interval: float = 1.0,
        scope: str = "global",
        success_statuses: Optional[Iterable[str]] = None,
        failure_statuses: Optional[Iterable[str]] = None,
    ) -> Any:
        """Call a contract and optionally wait for its submitted transaction."""

        typed_response = self.call_contract(
            authority=authority,
            private_key=private_key,
            contract_address=contract_address,
            contract_alias=contract_alias,
            entrypoint=entrypoint,
            payload=payload,
            fee_payment=fee_payment,
        )
        if not wait:
            return self._contract_response_payload(typed_response)
        return self._wait_for_contract_response(
            typed_response,
            timeout_ms=timeout_ms,
            interval=interval,
            scope=scope,
            success_statuses=success_statuses,
            failure_statuses=failure_statuses,
        )

    def get_contract_manifest(self, code_hash_hex: str) -> Optional[Any]:
        response = self._request(
            "GET",
            f"/v1/contracts/code/{code_hash_hex}",
        )
        self._expect_status(response, {200, 404})
        return self._maybe_json(response)

    def get_contract_manifest_typed(self, code_hash_hex: str) -> ContractManifestRecord:
        """Typed wrapper for :meth:`get_contract_manifest`."""

        payload = self.get_contract_manifest(code_hash_hex)
        if payload is None:
            raise RuntimeError("contract manifest endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("contract manifest endpoint returned non-object payload")
        return ContractManifestRecord.from_payload(payload)

    def get_contract_code_bytes(self, code_hash_hex: str) -> Optional[Any]:
        response = self._request(
            "GET",
            f"/v1/contracts/code-bytes/{code_hash_hex}",
        )
        self._expect_status(response, {200, 404})
        return self._maybe_json(response)

    # ------------------------------------------------------------------
    # Connect API
    # ------------------------------------------------------------------
    def create_connect_session(
        self,
        payload: Optional[Mapping[str, Any]] = None,
    ) -> Optional[Any]:
        """POST `/v1/connect/session` and return the session payload."""

        body = dict(payload) if payload is not None else {}
        return self.request_json(
            "POST",
            "/v1/connect/session",
            json_body=body,
            expected_status=(200, 201),
        )

    def create_connect_session_info(
        self,
        payload: Optional[Mapping[str, Any]] = None,
        *,
        include_expiry: bool = True,
    ) -> ConnectSessionInfo:
        """Create a session and parse the response into `ConnectSessionInfo`."""

        response = self.create_connect_session(payload)
        if not isinstance(response, Mapping):
            raise ValueError("connect session response is missing or malformed")
        ttl_ms: Optional[int] = None
        if include_expiry:
            status_snapshot = self.get_connect_status_typed()
            if status_snapshot is not None and status_snapshot.policy is not None:
                ttl_ms = status_snapshot.policy.session_ttl_ms
        return ConnectSessionInfo.from_mapping(response, session_ttl_ms=ttl_ms)

    def send_connect_control(
        self,
        sid: str,
        *,
        kind: str,
        payload: Mapping[str, Any],
    ) -> Optional[Any]:
        """Convenience helper for posting Connect control frames via `/v1/connect/control/{kind}`."""

        return self.request_json(
            "POST",
            f"/v1/connect/control/{kind}",
            params={"sid": sid},
            json_body=dict(payload),
            expected_status=(200, 202),
        )

    def send_connect_control_frame(
        self,
        sid: str,
        control: "ConnectControlBase",
    ) -> Optional[Any]:
        """Send a typed Connect control by inferring the REST endpoint from the variant."""

        payload = _json_safe_value(control.to_dict())
        return self.send_connect_control(
            sid,
            kind=control.endpoint_kind,
            payload=payload,
        )

    def delete_connect_session(self, sid: str, token_management: str) -> bool:
        """DELETE `/v1/connect/session/{sid}` and return True when the session existed."""

        if not isinstance(sid, str) or not sid:
            raise TypeError("sid must be a non-empty string")
        if not isinstance(token_management, str) or not token_management:
            raise TypeError("token_management must be a non-empty string")
        response = self._request(
            "DELETE",
            f"/v1/connect/session/{sid}",
            headers={"Authorization": f"Bearer {token_management}"},
        )
        self._expect_status(response, (204, 404))
        return response.status_code == 204

    def connect_websocket(
        self,
        sid: str,
        role: str,
        token: str,
        *,
        timeout: Optional[float] = None,
        headers: Optional[Mapping[str, str]] = None,
        subprotocols: Optional[Sequence[str]] = None,
    ):
        """Open a Connect WebSocket (`/v1/connect/ws`). Requires `websocket-client`."""

        if websocket is None:  # pragma: no cover - dependency optional
            raise RuntimeError(
                "websocket-client is not installed. Install iroha-python with the `ws` extra "
                "(`pip install iroha-python[ws]`) or add `websocket-client` to your environment."
            )

        parsed = urlparse(self._base_url)
        scheme = "wss" if parsed.scheme == "https" else "ws"
        ws_path = "/v1/connect/ws"
        query = urlencode({"sid": sid, "role": role, "token": token})
        ws_url = urlunparse((scheme, parsed.netloc, ws_path, "", query, ""))

        header_list: List[str] = []
        combined_headers: Dict[str, str] = dict(self._default_headers)
        combined_headers.pop("Accept", None)
        if headers:
            combined_headers.update(headers)
        for key, value in combined_headers.items():
            header_list.append(f"{key}: {value}")

        return websocket.create_connection(
            ws_url,
            timeout=timeout,
            header=header_list or None,
            subprotocols=list(subprotocols) if subprotocols else None,
        )

    def get_connect_status(self) -> Optional[Any]:
        """Fetch Connect runtime status (`GET /v1/connect/status`)."""

        return self.request_json(
            "GET",
            "/v1/connect/status",
            expected_status=(200,),
        )

    def get_connect_status_typed(self) -> Optional[ConnectStatusSnapshot]:
        """Typed wrapper for :meth:`get_connect_status`. Returns `None` when Connect is disabled."""

        payload = self.get_connect_status()
        if payload is None:
            return None
        return ConnectStatusSnapshot.from_payload(payload)

    def list_connect_apps(
        self,
        *,
        limit: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> "ConnectAppRegistryPage":
        """List registered Connect applications (`GET /v1/connect/app/apps`)."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if cursor is not None:
            if not isinstance(cursor, str):
                raise TypeError("connect app cursor must be a string")
            params["cursor"] = cursor
        payload = self.request_json(
            "GET",
            "/v1/connect/app/apps",
            params=params or None,
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app registry response must be a JSON object")
        return ConnectAppRegistryPage.from_payload(payload)

    def iter_connect_apps(
        self,
        *,
        page_size: Optional[int] = None,
        cursor: Optional[str] = None,
    ) -> Iterator["ConnectAppRecord"]:
        """Iterate over all Connect applications by following pagination cursors.

        Args:
            page_size: Optional limit applied to each request. Must be positive when set.
            cursor: Optional starting cursor returned by a previous listing.

        Yields:
            :class:`ConnectAppRecord` entries for every registry item.
        """

        if page_size is not None:
            page_limit = int(page_size)
            if page_limit <= 0:
                raise ValueError("page_size must be positive when provided")
        else:
            page_limit = None

        if cursor is not None and not isinstance(cursor, str):
            raise TypeError("cursor must be a string when provided")

        seen_cursors: set[str] = set()
        next_cursor = cursor
        if next_cursor is not None:
            seen_cursors.add(next_cursor)

        while True:
            page = self.list_connect_apps(limit=page_limit, cursor=next_cursor)
            for record in page.items:
                yield record
            next_cursor = page.next_cursor
            if next_cursor is None:
                break
            if next_cursor in seen_cursors:
                raise RuntimeError(
                    f"connect app registry returned duplicate cursor {next_cursor!r}"
                )
            seen_cursors.add(next_cursor)

    def get_connect_app(self, app_id: str) -> "ConnectAppRecord":
        """Fetch a single Connect application (`GET /v1/connect/app/apps/{app_id}`)."""

        if not isinstance(app_id, str) or not app_id:
            raise TypeError("app_id must be a non-empty string")
        payload = self.request_json(
            "GET",
            f"/v1/connect/app/apps/{app_id}",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app response must be a JSON object")
        return ConnectAppRecord.from_payload(payload)

    def register_connect_app(
        self,
        registration: Union["ConnectAppRecord", Mapping[str, Any]],
    ) -> Optional[Any]:
        """Register or update a Connect application (`POST /v1/connect/app/apps`)."""

        if isinstance(registration, ConnectAppRecord):
            body = registration.to_payload()
        else:
            body = dict(registration)
        payload = self.request_json(
            "POST",
            "/v1/connect/app/apps",
            json_body=_json_safe_value(body),
            expected_status=(200, 201, 202),
        )
        if isinstance(payload, Mapping):
            try:
                return ConnectAppRecord.from_payload(payload)
            except TypeError:
                return payload
        return payload

    def delete_connect_app(self, app_id: str) -> Optional[Any]:
        """Delete a Connect application (`DELETE /v1/connect/app/apps/{app_id}`)."""

        if not isinstance(app_id, str) or not app_id:
            raise TypeError("app_id must be a non-empty string")
        response = self._request("DELETE", f"/v1/connect/app/apps/{app_id}")
        self._expect_status(response, {200, 202, 204, 404})
        return self._maybe_json(response)

    def get_connect_app_policy_controls(self) -> "ConnectAppPolicyControls":
        """Fetch mutable Connect policy toggles (`GET /v1/connect/app/policy`)."""

        payload = self.request_json(
            "GET",
            "/v1/connect/app/policy",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app policy response must be a JSON object")
        policy_payload = payload.get("policy")
        if isinstance(policy_payload, Mapping):
            return ConnectAppPolicyControls.from_payload(policy_payload)
        return ConnectAppPolicyControls.from_payload(payload)

    def update_connect_app_policy_controls(
        self,
        updates: Union["ConnectAppPolicyControls", Mapping[str, Any]],
    ) -> "ConnectAppPolicyControls":
        """Update Connect policy toggles (`POST /v1/connect/app/policy`)."""

        if isinstance(updates, ConnectAppPolicyControls):
            body = updates.to_payload()
        else:
            body = dict(updates)
        payload = self.request_json(
            "POST",
            "/v1/connect/app/policy",
            json_body=_json_safe_value(body),
            expected_status=(200, 202),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect app policy response must be a JSON object")
        policy_payload = payload.get("policy")
        if isinstance(policy_payload, Mapping):
            return ConnectAppPolicyControls.from_payload(policy_payload)
        return ConnectAppPolicyControls.from_payload(payload)

    def get_connect_admission_manifest(self) -> "ConnectAdmissionManifest":
        """Fetch the Connect admission manifest (`GET /v1/connect/app/manifest`)."""

        payload = self.request_json(
            "GET",
            "/v1/connect/app/manifest",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission manifest response must be a JSON object")
        return ConnectAdmissionManifest.from_payload(payload)

    def set_connect_admission_manifest(
        self,
        manifest: Union["ConnectAdmissionManifest", Mapping[str, Any]],
    ) -> "ConnectAdmissionManifest":
        """Replace the Connect admission manifest (`PUT /v1/connect/app/manifest`)."""

        if isinstance(manifest, ConnectAdmissionManifest):
            body = manifest.to_payload()
        else:
            body = dict(manifest)
        payload = self.request_json(
            "PUT",
            "/v1/connect/app/manifest",
            json_body=_json_safe_value(body),
            expected_status=(200, 202),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission manifest response must be a JSON object")
        return ConnectAdmissionManifest.from_payload(payload)


    # Telemetry & Sumeragi helpers
    # ------------------------------------------------------------------
    def get_sumeragi_telemetry(self) -> Optional[Any]:
        """Fetch aggregated consensus telemetry (`GET /v1/sumeragi/telemetry`)."""

        return self.request_json("GET", "/v1/sumeragi/telemetry", expected_status=(200,))

    def get_sumeragi_telemetry_typed(self) -> SumeragiTelemetrySnapshot:
        """Return `/v1/sumeragi/telemetry` as a structured snapshot."""

        payload = self.request_json("GET", "/v1/sumeragi/telemetry", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("telemetry response must be a JSON object")
        return SumeragiTelemetrySnapshot.from_payload(payload)

    def get_sumeragi_status(self) -> Optional[Any]:
        """Fetch the raw authoritative v2 consensus status JSON."""

        return self.request_json("GET", "/v1/sumeragi/status", expected_status=(200,))

    def get_sumeragi_status_typed(self) -> SumeragiStatusSnapshot:
        """Validate the fail-closed authoritative v2 reducer snapshot."""

        payload = self.request_json("GET", "/v1/sumeragi/status", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi status response must be a JSON object")
        return SumeragiStatusSnapshot.from_payload(payload)

    def get_sumeragi_diagnostics(self) -> Optional[Any]:
        """Fetch raw bounded Sumeragi operator and lane diagnostics."""

        return self.request_json(
            "GET", "/v1/sumeragi/diagnostics", expected_status=(200,)
        )

    def get_sumeragi_diagnostics_typed(self) -> SumeragiDiagnosticsSnapshot:
        """Validate `/v1/sumeragi/diagnostics` as a separate typed payload."""

        payload = self.request_json(
            "GET", "/v1/sumeragi/diagnostics", expected_status=(200,)
        )
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi diagnostics response must be a JSON object")
        return SumeragiDiagnosticsSnapshot.from_payload(payload)

    def get_sumeragi_pacemaker(self) -> Optional[Any]:
        """Fetch pacemaker configuration (`GET /v1/sumeragi/pacemaker`)."""

        return self.request_json("GET", "/v1/sumeragi/pacemaker", expected_status=(200,))

    def get_sumeragi_pacemaker_typed(self) -> SumeragiPacemakerSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_pacemaker`."""

        payload = self.request_json("GET", "/v1/sumeragi/pacemaker", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("pacemaker response must be a JSON object")
        return SumeragiPacemakerSnapshot.from_payload(payload)

    def get_sumeragi_qc(self) -> Optional[Any]:
        """Fetch HighestQC/LockedQC snapshot (`GET /v1/sumeragi/qc`)."""

        return self.request_json("GET", "/v1/sumeragi/qc", expected_status=(200,))

    def get_sumeragi_qc_typed(self) -> SumeragiQcSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_qc`."""

        payload = self.request_json("GET", "/v1/sumeragi/qc", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("qc response must be a JSON object")
        return SumeragiQcSnapshot.from_payload(payload)

    def get_sumeragi_commit_qc(self, block_hash_hex: str) -> Optional[Any]:
        """Fetch commit QC details for a block hash (`GET /v1/sumeragi/commit-qcs/{block_hash}`)."""

        normalized = _normalize_hash_hex(block_hash_hex, "block_hash_hex")
        return self.request_json(
            "GET",
            f"/v1/sumeragi/commit-qcs/{normalized}",
            expected_status=(200,),
        )

    def get_sumeragi_commit_qc_typed(self, block_hash_hex: str) -> SumeragiCommitQcRecord:
        """Typed wrapper for :meth:`get_sumeragi_commit_qc`."""

        payload = self.get_sumeragi_commit_qc(block_hash_hex)
        if not isinstance(payload, Mapping):
            raise TypeError("commit_qc response must be a JSON object")
        return SumeragiCommitQcRecord.from_payload(payload)

    def get_sumeragi_leader(self) -> Optional[Any]:
        """Fetch leader index snapshot (`GET /v1/sumeragi/leader`)."""

        return self.request_json("GET", "/v1/sumeragi/leader", expected_status=(200,))

    def get_sumeragi_leader_typed(self) -> SumeragiLeaderSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_leader`."""

        payload = self.request_json("GET", "/v1/sumeragi/leader", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("leader response must be a JSON object")
        return SumeragiLeaderSnapshot.from_payload(payload)

    def get_sumeragi_evidence_count(self) -> Optional[Any]:
        """Return total persisted evidence records (`GET /v1/sumeragi/evidence/count`)."""

        return self.request_json(
            "GET",
            "/v1/sumeragi/evidence/count",
            expected_status=(200,),
        )

    def get_sumeragi_evidence_count_typed(self) -> SumeragiEvidenceCount:
        """Typed wrapper for :meth:`get_sumeragi_evidence_count`."""

        payload = self.request_json(
            "GET",
            "/v1/sumeragi/evidence/count",
            expected_status=(200,),
        )
        if not isinstance(payload, Mapping):
            raise TypeError("evidence count response must be a JSON object")
        return SumeragiEvidenceCount.from_payload(payload)

    def list_sumeragi_evidence(
        self,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        kind: Optional[str] = None,
    ) -> Optional[Any]:
        """List evidence records with optional filters (`GET /v1/sumeragi/evidence`)."""

        params: Dict[str, Any] = {}
        if limit is not None:
            params["limit"] = int(limit)
        if offset is not None:
            params["offset"] = int(offset)
        if kind is not None:
            params["kind"] = str(kind)
        return self.request_json(
            "GET",
            "/v1/sumeragi/evidence",
            params=params or None,
            expected_status=(200,),
        )

    def list_sumeragi_evidence_typed(
        self,
        *,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        kind: Optional[str] = None,
    ) -> SumeragiEvidenceListPage:
        """Typed wrapper for :meth:`list_sumeragi_evidence`."""

        payload = self.list_sumeragi_evidence(limit=limit, offset=offset, kind=kind)
        if payload is None:
            return SumeragiEvidenceListPage(items=[], total=0)
        if not isinstance(payload, Mapping):
            raise RuntimeError("sumeragi evidence endpoint returned non-object payload")
        return SumeragiEvidenceListPage.from_payload(payload)

    def submit_sumeragi_evidence(self, evidence_hex: str) -> Optional[Any]:
        """Submit a Norito-encoded evidence payload (`POST /v1/sumeragi/evidence`)."""

        return self.request_json(
            "POST",
            "/v1/sumeragi/evidence",
            json_body={"evidence_hex": str(evidence_hex)},
            expected_status=(200, 202),
            allow_retry=False,
        )

    def get_sumeragi_phases(self) -> Optional[Any]:
        """Fetch consensus phase durations (`GET /v1/sumeragi/phases`)."""

        return self.request_json("GET", "/v1/sumeragi/phases", expected_status=(200,))

    def get_sumeragi_phases_typed(self) -> SumeragiPhasesSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_phases`."""

        payload = self.request_json("GET", "/v1/sumeragi/phases", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("phases response must be a JSON object")
        return SumeragiPhasesSnapshot.from_payload(payload)

    def get_sumeragi_params(self) -> Optional[Any]:
        """Fetch on-chain Sumeragi parameters (`GET /v1/sumeragi/params`)."""

        return self.request_json("GET", "/v1/sumeragi/params", expected_status=(200,))

    def get_sumeragi_params_typed(self) -> SumeragiParamsSnapshot:
        """Typed wrapper for :meth:`get_sumeragi_params`."""

        payload = self.request_json("GET", "/v1/sumeragi/params", expected_status=(200,))
        if not isinstance(payload, Mapping):
            raise TypeError("sumeragi params response must be a JSON object")
        return SumeragiParamsSnapshot.from_payload(payload)

    # ------------------------------------------------------------------
    # Governance API
    # ------------------------------------------------------------------
    def set_protected_namespaces(self, namespaces: Sequence[str]) -> Optional[Any]:
        """Apply the `gov_protected_namespaces` parameter via `POST /v1/gov/protected-namespaces`."""

        if isinstance(namespaces, str):
            values = [str(namespaces)]
        else:
            values = [str(value) for value in namespaces]
        return self.request_json(
            "POST",
            "/v1/gov/protected-namespaces",
            json_body={"namespaces": values},
            expected_status=(200,),
        )

    def get_protected_namespaces(self) -> Optional[Any]:
        """Fetch the current `gov_protected_namespaces` setting (`GET /v1/gov/protected-namespaces`)."""

        return self.request_json(
            "GET",
            "/v1/gov/protected-namespaces",
            expected_status=(200,),
        )

    def get_governance_council_current(self) -> Optional[Any]:
        """GET `/v1/gov/council/current`."""

        return self.request_json(
            "GET",
            "/v1/gov/council/current",
            expected_status=(200,),
        )

    def get_governance_council_audit(self, *, epoch: Optional[int] = None) -> Optional[Any]:
        """GET `/v1/gov/council/audit` (optionally override the audited epoch)."""

        params: Dict[str, Any] = {}
        if epoch is not None:
            params["epoch"] = int(epoch)
        return self.request_json(
            "GET",
            "/v1/gov/council/audit",
            params=params or None,
            expected_status=(200,),
        )

    def derive_governance_council_vrf(
        self,
        payload: Mapping[str, Any],
    ) -> Optional[Any]:
        """POST `/v1/gov/council/derive-vrf` (requires Torii built with `gov_vrf`)."""

        return self.request_json(
            "POST",
            "/v1/gov/council/derive-vrf",
            json_body=dict(payload),
            expected_status=(200,),
        )

    def persist_governance_council(
        self,
        payload: Mapping[str, Any],
    ) -> Optional[Any]:
        """POST `/v1/gov/council/persist` (requires `gov_vrf`)."""

        return self.request_json(
            "POST",
            "/v1/gov/council/persist",
            json_body=dict(payload),
            expected_status=(200,),
        )

    def get_governance_contract(
        self,
        contract_address: str,
    ) -> Optional[Any]:
        """Fetch one governance-managed contract binding (`GET /v1/gov/contracts/{contract_address}`)."""

        return self.request_json(
            "GET",
            f"/v1/gov/contracts/{contract_address}",
            expected_status=(200,),
        )

    def get_governance_contract_typed(
        self,
        contract_address: str,
    ) -> GovernanceContractRecord:
        """Typed wrapper for :meth:`get_governance_contract`."""

        payload = self.get_governance_contract(contract_address)
        if payload is None:
            raise RuntimeError("governance contract endpoint returned no payload")
        if not isinstance(payload, Mapping):
            raise RuntimeError("governance contract endpoint returned non-object payload")
        return GovernanceContractRecord.from_payload(payload)

    def governance_deploy_contract_proposal(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/proposals/deploy-contract`."""

        return self.request_json(
            "POST",
            "/v1/gov/proposals/deploy-contract",
            json_body=dict(payload),
        )

    def governance_submit_plain_ballot(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/plain`."""

        normalized = dict(payload)
        normalized["amount"] = _canonical_quantity_text(
            normalized.get("amount"),
            "governance plain ballot amount",
        )
        return self.request_json(
            "POST",
            "/v1/gov/ballots/plain",
            json_body=normalized,
        )

    def governance_submit_zk_ballot(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/zk`."""

        return self.request_json(
            "POST",
            "/v1/gov/ballots/zk",
            json_body=_normalize_governance_zk_ballot_payload(
                payload,
                context="governance zk ballot",
            ),
        )

    def governance_submit_zk_ballot_v1(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/zk-v1`."""

        return self.request_json(
            "POST",
            "/v1/gov/ballots/zk-v1",
            json_body=_normalize_governance_zk_ballot_v1_payload(
                payload,
                context="governance zk ballot v1",
            ),
        )

    def governance_submit_zk_ballot_proof_v1(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/ballots/zk-v1/ballot-proof`."""

        return self.request_json(
            "POST",
            "/v1/gov/ballots/zk-v1/ballot-proof",
            json_body=_normalize_governance_zk_ballot_proof_payload(
                payload,
                context="governance zk ballot proof v1",
            ),
        )

    def governance_finalize_referendum(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/finalize`."""

        return self.request_json(
            "POST",
            "/v1/gov/finalize",
            json_body=dict(payload),
        )

    def governance_enact_proposal(self, payload: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/gov/enact`."""

        return self.request_json(
            "POST",
            "/v1/gov/enact",
            json_body=dict(payload),
        )

    def get_governance_proposal(self, proposal_id: str) -> Optional[Any]:
        """GET `/v1/gov/proposals/{proposal_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/proposals/{proposal_id}",
            expected_status=(200, 404),
        )

    def get_governance_proposal_typed(self, proposal_id: str) -> GovernanceProposalResult:
        """Typed wrapper for :meth:`get_governance_proposal`."""

        payload = self.get_governance_proposal(proposal_id)
        if payload is None:
            return GovernanceProposalResult(found=False, proposal=None)
        return GovernanceProposalResult.from_payload(payload)

    def get_governance_referendum(self, referendum_id: str) -> Optional[Any]:
        """GET `/v1/gov/referenda/{referendum_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/referenda/{referendum_id}",
            expected_status=(200, 404),
        )

    def get_governance_referendum_typed(
        self,
        referendum_id: str,
    ) -> GovernanceReferendumResult:
        """Typed wrapper for :meth:`get_governance_referendum`."""

        payload = self.get_governance_referendum(referendum_id)
        if payload is None:
            return GovernanceReferendumResult(found=False, referendum=None)
        return GovernanceReferendumResult.from_payload(payload)

    def get_governance_tally(self, referendum_id: str) -> Optional[Any]:
        """GET `/v1/gov/tally/{referendum_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/tally/{referendum_id}",
            expected_status=(200, 404),
        )

    def get_governance_tally_typed(self, referendum_id: str) -> GovernanceTally:
        """Typed wrapper for :meth:`get_governance_tally`."""

        payload = self.get_governance_tally(referendum_id)
        if payload is None:
            return GovernanceTally(
                referendum_id=referendum_id,
                approve=0,
                reject=0,
                abstain=0,
            )
        return GovernanceTally.from_payload(payload)

    def get_governance_locks(self, referendum_id: str) -> Optional[Any]:
        """GET `/v1/gov/locks/{referendum_id}`."""

        return self.request_json(
            "GET",
            f"/v1/gov/locks/{referendum_id}",
            expected_status=(200, 404),
        )

    def get_governance_locks_typed(self, referendum_id: str) -> GovernanceLocksResult:
        """Typed wrapper for :meth:`get_governance_locks`."""

        payload = self.get_governance_locks(referendum_id)
        if payload is None:
            return GovernanceLocksResult(found=False, referendum_id=referendum_id, locks={})
        return GovernanceLocksResult.from_payload(payload)

    def get_governance_unlock_stats(self) -> Optional[Any]:
        """GET `/v1/gov/unlocks/stats`."""

        return self.request_json(
            "GET",
            "/v1/gov/unlocks/stats",
            expected_status=(200,),
        )

    def get_governance_unlock_stats_typed(self) -> GovernanceUnlockStats:
        """Typed wrapper for :meth:`get_governance_unlock_stats`."""

        payload = self.get_governance_unlock_stats()
        if payload is None:
            raise RuntimeError("governance unlock stats endpoint returned no payload")
        return GovernanceUnlockStats.from_payload(payload)

    def stream_events(
        self,
        *,
        filter: Optional[Any] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream live JSON events from `/v1/events/sse`.

        The `filter` parameter accepts a JSON string, mapping, or an
        :class:`iroha_python.event_filter.EventFilter` instance.

        Torii does not retain a replay log for this route. A reconnect starts a
        new live subscription and can have a gap; use the committed block
        stream when complete ledger history is required.
        """

        filter_payload = ensure_event_filter(filter)
        params = {"filter": filter_payload} if filter_payload else None

        def _handle(event: SseEvent) -> None:
            if on_event is None:
                return
            if with_metadata:
                on_event(event)
            else:
                on_event(event.data, event.id)

        iterator = self._stream_sse(
            "/v1/events/sse",
            params=params,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            decode_json=decode_json,
            on_event=_handle if on_event is not None else None,
        )
        if with_metadata:
            return iterator
        return (event.data for event in iterator)

    def stream_sumeragi_status(
        self,
        *,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream `/v1/sumeragi/status/sse` live consensus metrics."""

        def _handle(event: SseEvent) -> None:
            if on_event is None:
                return
            if with_metadata:
                on_event(event)
            else:
                on_event(event.data, event.id)

        iterator = self._stream_sse(
            "/v1/sumeragi/status/sse",
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            decode_json=decode_json,
            on_event=_handle if on_event is not None else None,
        )
        if with_metadata:
            return iterator
        return (event.data for event in iterator)

    def stream_verifying_key_events(
        self,
        *,
        backend: Optional[str] = None,
        name: Optional[str] = None,
        registered: bool = True,
        updated: bool = True,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream verifying-key lifecycle events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.verifying_key(
            backend=backend,
            name=name,
            registered=registered,
            updated=updated,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            on_event=on_event,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_proof_events(
        self,
        *,
        backend: Optional[str] = None,
        proof_hash_hex: Optional[str] = None,
        verified: bool = True,
        rejected: bool = True,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream proof verification events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.proof(
            backend=backend,
            proof_hash_hex=proof_hash_hex,
            verified=verified,
            rejected=rejected,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            on_event=on_event,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_trigger_events(
        self,
        *,
        trigger_id: Optional[str] = None,
        created: bool = True,
        deleted: bool = True,
        extended: bool = True,
        shortened: bool = True,
        metadata_inserted: bool = True,
        metadata_removed: bool = True,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream trigger lifecycle events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.trigger(
            trigger_id=trigger_id,
            created=created,
            deleted=deleted,
            extended=extended,
            shortened=shortened,
            metadata_inserted=metadata_inserted,
            metadata_removed=metadata_removed,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            on_event=on_event,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_transactions(
        self,
        *,
        hash_hex: Optional[str] = None,
        block_height: Optional[int] = None,
        status: Optional[str] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream pipeline transaction events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_transaction(
            hash_hex=hash_hex,
            block_height=block_height,
            status=status,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            on_event=on_event,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_blocks(
        self,
        *,
        height: Optional[int] = None,
        status: Optional[str] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream pipeline block events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_block(
            height=height,
            status=status,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            on_event=on_event,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_witnesses(
        self,
        *,
        block_hash_hex: Optional[str] = None,
        height: Optional[int] = None,
        view: Optional[int] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream execution witness events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_witness(
            block_hash_hex=block_hash_hex,
            height=height,
            view=view,
        )
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            on_event=on_event,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    def stream_pipeline_merges(
        self,
        *,
        epoch_id: Optional[int] = None,
        timeout: Optional[float] = None,
        max_retries: int = 3,
        backoff_base: float = 0.5,
        on_event: Optional[Callable[..., None]] = None,
        with_metadata: bool = False,
        decode_json: bool = True,
    ):
        """Stream merge-ledger events via `/v1/events/sse`."""

        filter_obj = DataEventFilter.pipeline_merge(epoch_id=epoch_id)
        return self.stream_events(
            filter=filter_obj,
            timeout=timeout,
            max_retries=max_retries,
            backoff_base=backoff_base,
            on_event=on_event,
            with_metadata=with_metadata,
            decode_json=decode_json,
        )

    # ------------------------------------------------------------------
    # Triggers API
    # ------------------------------------------------------------------
    def list_triggers(
        self,
        *,
        namespace: Optional[str] = None,
        authority: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> Optional[Any]:
        """GET `/v1/triggers` with optional filtering."""

        params: Dict[str, Any] = {}
        namespace_value = _normalize_optional_string(namespace, "list_triggers.namespace")
        if namespace_value is not None:
            params["namespace"] = namespace_value
        authority_value = _normalize_optional_string(authority, "list_triggers.authority")
        if authority_value is not None:
            params["authority"] = authority_value
        limit_value = _coerce_int(limit, "list_triggers.limit") if limit is not None else None
        if limit_value is not None:
            params["limit"] = limit_value
        offset_value = (
            _coerce_int(offset, "list_triggers.offset", allow_zero=True)
            if offset is not None
            else None
        )
        if offset_value is not None:
            params["offset"] = offset_value
        return self.request_json(
            "GET",
            "/v1/triggers",
            params=params or None,
            expected_status=(200,),
        )

    def list_triggers_typed(
        self,
        *,
        namespace: Optional[str] = None,
        authority: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> TriggerListPage:
        """Typed wrapper for :meth:`list_triggers`."""

        payload = self.list_triggers(
            namespace=namespace,
            authority=authority,
            limit=limit,
            offset=offset,
        )
        if payload is None:
            return TriggerListPage(items=[], total=0)
        return TriggerListPage.from_payload(payload)

    def get_trigger(self, trigger_id: str) -> Optional[Any]:
        """GET `/v1/triggers/{trigger_id}` and return the stored trigger or `None` when missing."""

        normalized_id = _require_non_empty_string(trigger_id, "trigger_id")
        return self.request_json(
            "GET",
            f"/v1/triggers/{normalized_id}",
            expected_status=(200, 404),
        )

    def get_trigger_typed(self, trigger_id: str) -> Optional[TriggerRecord]:
        """Typed wrapper for :meth:`get_trigger`."""

        payload = self.get_trigger(trigger_id)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger endpoint returned non-object payload")
        return TriggerRecord.from_payload(payload)

    def register_trigger(self, trigger: Mapping[str, Any]) -> Optional[Any]:
        """POST `/v1/triggers` with a trigger registration payload."""

        if not isinstance(trigger, Mapping):
            raise TypeError("trigger must be a mapping")
        return self.request_json(
            "POST",
            "/v1/triggers",
            json_body=dict(trigger),
            expected_status=(200, 201, 202),
        )

    def register_trigger_typed(self, trigger: Mapping[str, Any]) -> Optional[TriggerMutationResponse]:
        """Typed wrapper for :meth:`register_trigger`."""

        payload = self.register_trigger(trigger)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger registration returned malformed payload")
        return TriggerMutationResponse.from_payload(payload)

    def delete_trigger(self, trigger_id: str) -> Optional[Any]:
        """DELETE `/v1/triggers/{trigger_id}`."""

        response = self._request("DELETE", f"/v1/triggers/{trigger_id}")
        self._expect_status(response, {200, 202, 204, 404})
        return self._maybe_json(response)

    def delete_trigger_typed(self, trigger_id: str) -> Optional[TriggerMutationResponse]:
        """Typed wrapper for :meth:`delete_trigger`."""

        payload = self.delete_trigger(trigger_id)
        if payload is None:
            return None
        if not isinstance(payload, Mapping):
            raise RuntimeError("trigger deletion returned malformed payload")
        return TriggerMutationResponse.from_payload(payload)

    def query_triggers(
        self,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        """POST `/v1/triggers/query` with a structured envelope."""

        if filter is not None:
            if not isinstance(filter, Mapping):
                raise TypeError("query_triggers.filter must be a mapping")
        body = self._build_query_envelope(
            filter=filter,
            select=select,
            sort=sort,
            limit=_coerce_int(limit, "query_triggers.limit") if limit is not None else None,
            offset=(
                _coerce_int(offset, "query_triggers.offset", allow_zero=True)
                if offset is not None
                else None
            ),
            fetch_size=(
                _coerce_int(fetch_size, "query_triggers.fetch_size")
                if fetch_size is not None
                else None
            ),
            count_mode=count_mode,
            query_name=query_name,
        )
        response = self._request(
            "POST",
            "/v1/triggers/query",
            data=json.dumps(body).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        self._expect_status(response, {200})
        payload = self._maybe_json(response)
        if not isinstance(payload, dict):
            raise RuntimeError("unexpected triggers query response")
        return payload

    def query_triggers_typed(
        self,
        *,
        filter: Optional[Mapping[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> TriggerListPage:
        """Typed wrapper for :meth:`query_triggers`."""

        payload = self.query_triggers(
            filter=filter,
            select=select,
            sort=sort,
            limit=limit,
            offset=offset,
            fetch_size=fetch_size,
            count_mode=count_mode,
            query_name=query_name,
        )
        return TriggerListPage.from_payload(payload)

    @staticmethod
    def _maybe_json(response: requests.Response) -> Optional[Any]:
        if not hasattr(response, "content"):
            try:
                return response.json()
            except ValueError:
                return getattr(response, "text", "") or None
        if not response.content:
            return None
        try:
            return response.json()
        except ValueError:
            return response.text or None

    @staticmethod
    def _maybe_transaction_receipt(response: requests.Response) -> Optional[Any]:
        content_type = response.headers.get("Content-Type", "")
        if "application/x-norito" not in content_type.lower():
            return None
        if not response.content:
            return None
        try:
            crypto = _require_crypto()
        except RuntimeError:
            return None
        if not hasattr(crypto, "decode_transaction_receipt_json"):
            return None
        try:
            receipt_json = crypto.decode_transaction_receipt_json(response.content)
        except Exception:
            return None
        try:
            return json.loads(receipt_json)
        except json.JSONDecodeError:
            return None

    @staticmethod
    def _parse_sse_event(
        lines: Iterable[str],
        *,
        decode_json: bool = True,
    ) -> Optional[SseEvent]:
        raw_lines = list(lines)
        data_chunks: List[str] = []
        event_name: Optional[str] = None
        event_id: Optional[str] = None
        retry_value: Optional[int] = None
        for entry in raw_lines:
            if entry.startswith(":"):
                continue
            field, sep, value = entry.partition(":")
            value = value.lstrip() if sep else ""
            if field == "data":
                data_chunks.append(value)
            elif field == "id":
                event_id = value or None
            elif field == "event":
                event_name = value or None
            elif field == "retry":
                try:
                    retry_value = int(value)
                except ValueError:
                    retry_value = None
        if not data_chunks and event_name is None and event_id is None and retry_value is None:
            return None
        payload: Any
        if data_chunks:
            joined = "\n".join(data_chunks)
            if decode_json:
                try:
                    payload = json.loads(joined)
                except json.JSONDecodeError:
                    payload = joined
            else:
                payload = joined
        else:
            payload = None
        return SseEvent(
            event=event_name,
            data=payload,
            id=event_id,
            retry=retry_value,
            raw="\n".join(raw_lines),
        )

    def _stream_sse(
        self,
        path: str,
        *,
        params: Optional[Mapping[str, Any]] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: Optional[float] = None,
        max_retries: Optional[int] = 3,
        backoff_base: float = 0.5,
        last_event_id: Optional[str] = None,
        resume: bool = False,
        decode_json: bool = True,
        cursor: Optional[EventCursor] = None,
        allow_resume: bool = False,
        on_event: Optional[Callable[[SseEvent], None]] = None,
    ):
        url = f"{self._base_url}{path}"
        if not allow_resume and (last_event_id is not None or resume or cursor is not None):
            raise ValueError(f"{path} does not support SSE replay")
        active_last_id = (
            last_event_id
            if last_event_id is not None
            else (cursor.last_event_id if cursor is not None else None)
        )
        should_resume = allow_resume and (
            resume or last_event_id is not None or cursor is not None
        )

        def iterator():
            nonlocal active_last_id

            def process_event(event: SseEvent) -> SseEvent:
                nonlocal active_last_id
                if event.event == "stream_error":
                    raise SseStreamError.from_event(event)
                if event.id is not None and allow_resume:
                    active_last_id = event.id
                    if cursor is not None:
                        cursor.advance(event)
                if on_event is not None:
                    on_event(event)
                return event

            attempt = 0
            backoff = max(backoff_base, 0.0)
            while True:
                try:
                    final_headers: Dict[str, str] = dict(self._default_headers)
                    final_headers.pop("Accept", None)
                    if headers:
                        final_headers.update(headers)
                    if not allow_resume:
                        for name in tuple(final_headers):
                            if name.lower() == "last-event-id":
                                final_headers.pop(name)
                    final_headers.setdefault("Accept", "text/event-stream")
                    if should_resume and active_last_id:
                        final_headers["Last-Event-ID"] = active_last_id
                    with self._session.get(
                        url,
                        params=params,
                        headers=final_headers or None,
                        stream=True,
                        timeout=timeout,
                    ) as response:
                        self._expect_status(response, {200})
                        attempt = 0
                        backoff = max(backoff_base, 0.0)
                        buffer: list[str] = []
                        for raw_line in response.iter_lines(decode_unicode=True):
                            if raw_line is None:
                                continue
                            line = raw_line.strip()
                            if not line:
                                if buffer:
                                    event = self._parse_sse_event(buffer, decode_json=decode_json)
                                    buffer.clear()
                                    if event is None:
                                        continue
                                    yield process_event(event)
                                continue
                            buffer.append(line)
                        if buffer:
                            event = self._parse_sse_event(buffer, decode_json=decode_json)
                            buffer.clear()
                            if event is not None:
                                yield process_event(event)
                        break
                except requests.RequestException:
                    attempt += 1
                    if max_retries is not None and attempt > max_retries:
                        raise
                    if backoff > 0.0:
                        time.sleep(backoff)
                        backoff *= 2
                    continue

        return iterator()

    @staticmethod
    def _build_query_envelope(
        *,
        filter: Optional[Mapping[str, Any]] = None,
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]] = None,
        sort: Optional[Any] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        fetch_size: Optional[int] = None,
        count_mode: Optional[str] = None,
        query_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {}
        if filter is not None:
            body["filter"] = dict(filter)
        normalized_select = ToriiClient._normalize_query_select(select)
        if normalized_select is not None:
            body["select"] = normalized_select
        if sort is not None:
            body["sort"] = sort
        pagination: Dict[str, int] = {}
        if limit is not None:
            pagination["limit"] = int(limit)
        if offset is not None:
            pagination["offset"] = int(offset)
        if pagination:
            body["pagination"] = pagination
        if fetch_size is not None:
            body["fetch_size"] = int(fetch_size)
        if count_mode is not None:
            body["count_mode"] = _normalize_count_mode_arg(count_mode)
        query_name_value = _normalize_optional_string(query_name, "query_name")
        if query_name_value is not None:
            body["query"] = query_name_value
        return body

    @staticmethod
    def _normalize_query_select(
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]],
    ) -> Optional[List[Union[str, Dict[str, Any]]]]:
        if select is None:
            return None
        if isinstance(select, (str, bytes, bytearray)):
            raise TypeError("select must be a sequence of field paths or objects")
        normalized: List[Union[str, Dict[str, Any]]] = []
        for index, entry in enumerate(select):
            if isinstance(entry, str):
                field_path = entry.strip()
                if not field_path:
                    raise ValueError(f"select[{index}] must be a non-empty field path")
                normalized.append(field_path)
            elif isinstance(entry, Mapping):
                normalized.append(dict(entry))
            else:
                raise TypeError(f"select[{index}] must be a field-path string or mapping")
        return normalized

    @staticmethod
    def _ensure_no_query_args(
        *,
        envelope: Mapping[str, Any],
        filter: Optional[Mapping[str, Any]],
        select: Optional[Iterable[Union[str, Mapping[str, Any]]]],
        sort: Optional[Any],
        limit: Optional[int],
        offset: Optional[int],
        fetch_size: Optional[int],
        count_mode: Optional[str],
        query_name: Optional[str],
    ) -> None:
        if any(
            value is not None
            for value in (filter, select, sort, limit, offset, fetch_size, count_mode, query_name)
        ):
            raise ValueError(
                "provide either `envelope` or builder arguments (filter/select/sort/limit/offset/fetch_size/count_mode/query_name), not both"
            )


def create_torii_client(
    base_url: str,
    *,
    session: Optional[requests.Session] = None,
    auth_token: Optional[str] = None,
    api_token: Optional[str] = None,
    default_headers: Optional[Mapping[str, str]] = None,
    timeout: Optional[float] = 30.0,
    max_retries: int = 3,
    backoff_factor: float = 0.5,
    retry_on_status: Optional[Sequence[int]] = None,
    retry_on_methods: Optional[Sequence[str]] = None,
    config: Optional[Mapping[str, Any]] = None,
    env: Optional[Mapping[str, str]] = None,
    overrides: Optional[Mapping[str, Any]] = None,
    resolved_config: Optional[ResolvedToriiClientConfig] = None,
    sorafs_alias_policy: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]] = None,
    sorafs_alias_warning: Optional[Callable[[SorafsAliasWarning], None]] = None,
    sorafs_alias_logger: Optional[logging.Logger] = None,
) -> ToriiClient:
    """Return a :class:`ToriiClient` instance with the given base URL."""
    resolved = resolved_config
    if resolved is None and (config is not None or overrides is not None or env is not None):
        resolved = resolve_torii_client_config(config=config, env=env, overrides=overrides)

    header_merge: Dict[str, str] = (
        dict(resolved.default_headers) if resolved is not None else {"Accept": "application/json"}
    )
    if default_headers:
        header_merge.update(default_headers)

    auth_value = auth_token if auth_token is not None else (resolved.auth_token if resolved else None)
    api_value = api_token if api_token is not None else (resolved.api_token if resolved else None)
    timeout_value = timeout if timeout is not None else (resolved.timeout if resolved else 30.0)
    max_retries_value = max_retries if max_retries is not None else (
        resolved.max_retries if resolved else 3
    )
    retry_statuses = retry_on_status if retry_on_status is not None else (
        list(resolved.retry_statuses) if resolved else None
    )
    retry_methods = retry_on_methods if retry_on_methods is not None else (
        list(resolved.retry_methods) if resolved else None
    )
    if resolved is not None:
        backoff_initial_ms = int(resolved.backoff_initial * 1000)
        max_backoff_ms = None if math.isinf(resolved.max_backoff) else int(resolved.max_backoff * 1000)
        backoff_mult = resolved.backoff_multiplier
    else:
        backoff_initial_ms = None
        max_backoff_ms = None
        backoff_mult = None

    policy_value: Optional[Union[SorafsAliasPolicy, Mapping[str, Any]]] = sorafs_alias_policy
    if policy_value is None and resolved is not None:
        policy_value = resolved.sorafs_alias_policy

    return ToriiClient(
        base_url,
        session=session,
        auth_token=auth_value,
        api_token=api_value,
        default_headers=header_merge,
        timeout=timeout_value,
        max_retries=max_retries_value,
        backoff_factor=backoff_factor,
        backoff_initial_ms=backoff_initial_ms,
        max_backoff_ms=max_backoff_ms,
        backoff_multiplier=backoff_mult,
        retry_on_status=retry_statuses,
        retry_on_methods=retry_methods,
        sorafs_alias_policy=policy_value,
        sorafs_alias_warning=sorafs_alias_warning,
        sorafs_alias_logger=sorafs_alias_logger,
    )


@dataclass(frozen=True)
class ConnectAppRecord:
    """Registered Connect application metadata."""

    app_id: str
    display_name: Optional[str]
    description: Optional[str]
    icon_url: Optional[str]
    namespaces: Sequence[str]
    metadata: Mapping[str, Any]
    policy: Mapping[str, Any]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app entry must be an object")
        data = dict(payload)
        app_id = data.get("app_id")
        if not isinstance(app_id, str) or not app_id:
            raise TypeError("connect app entry requires string `app_id` field")

        def _coerce_optional_str(name: str) -> Optional[str]:
            value = data.get(name)
            if value is None:
                return None
            if isinstance(value, str):
                return value
            raise TypeError(f"connect app entry `{name}` must be a string when present")

        namespaces_raw = data.get("namespaces") or []
        if namespaces_raw is None:
            namespaces_raw = []
        if not isinstance(namespaces_raw, list):
            raise TypeError("connect app entry `namespaces` must be a list")
        namespaces: List[str] = []
        for item in namespaces_raw:
            if not isinstance(item, str):
                raise TypeError("connect app entry `namespaces` must contain strings")
            namespaces.append(item)

        metadata_raw = data.get("metadata") or {}
        if metadata_raw is None:
            metadata_raw = {}
        if not isinstance(metadata_raw, Mapping):
            raise TypeError("connect app entry `metadata` must be an object")

        policy_raw = data.get("policy") or {}
        if policy_raw is None:
            policy_raw = {}
        if not isinstance(policy_raw, Mapping):
            raise TypeError("connect app entry `policy` must be an object")

        recognized = {
            "app_id",
            "display_name",
            "description",
            "icon_url",
            "namespaces",
            "metadata",
            "policy",
        }

        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            app_id=app_id,
            display_name=_coerce_optional_str("display_name"),
            description=_coerce_optional_str("description"),
            icon_url=_coerce_optional_str("icon_url"),
            namespaces=tuple(namespaces),
            metadata=dict(metadata_raw),
            policy=dict(policy_raw),
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the record back into a JSON-friendly mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        payload["app_id"] = self.app_id
        if self.display_name is not None:
            payload["display_name"] = self.display_name
        if self.description is not None:
            payload["description"] = self.description
        if self.icon_url is not None:
            payload["icon_url"] = self.icon_url
        payload["namespaces"] = list(self.namespaces)
        payload["metadata"] = dict(self.metadata)
        payload["policy"] = dict(self.policy)
        return payload


@dataclass(frozen=True)
class ConnectAppRegistryPage:
    """Paginated Connect application registry results."""

    items: Sequence[ConnectAppRecord]
    total: Optional[int]
    next_cursor: Optional[str]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppRegistryPage":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app registry payload must be an object")
        data = dict(payload)
        items_raw = data.get("items") or []
        if items_raw is None:
            items_raw = []
        if not isinstance(items_raw, list):
            raise TypeError("connect app registry `items` must be a list")
        items = [ConnectAppRecord.from_payload(entry) for entry in items_raw]

        total_raw = data.get("total")
        total: Optional[int]
        if total_raw is None:
            total = None
        else:
            try:
                total = int(total_raw)
            except (TypeError, ValueError) as exc:
                raise TypeError("connect app registry `total` must be numeric when present") from exc

        cursor_raw = data.get("next_cursor")
        if cursor_raw is not None and not isinstance(cursor_raw, str):
            raise TypeError("connect app registry cursor must be a string when present")

        recognized = {"items", "total", "next_cursor"}
        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(items=tuple(items), total=total, next_cursor=cursor_raw, extra=extra)


@dataclass(frozen=True)
class ConnectAdmissionManifestEntry:
    """Admission control record for a Connect application."""

    app_id: str
    namespaces: Sequence[str]
    metadata: Mapping[str, Any]
    policy: Mapping[str, Any]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAdmissionManifestEntry":
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission entry must be an object")
        data = dict(payload)
        app_id = data.get("app_id")
        if not isinstance(app_id, str) or not app_id:
            raise TypeError("connect admission entry requires string `app_id` field")

        namespaces_raw = data.get("namespaces") or []
        if namespaces_raw is None:
            namespaces_raw = []
        if not isinstance(namespaces_raw, list):
            raise TypeError("connect admission entry `namespaces` must be a list")
        namespaces: List[str] = []
        for item in namespaces_raw:
            if not isinstance(item, str):
                raise TypeError("connect admission entry `namespaces` values must be strings")
            namespaces.append(item)

        metadata_raw = data.get("metadata") or {}
        if metadata_raw is None:
            metadata_raw = {}
        if not isinstance(metadata_raw, Mapping):
            raise TypeError("connect admission entry `metadata` must be an object")

        policy_raw = data.get("policy") or {}
        if policy_raw is None:
            policy_raw = {}
        if not isinstance(policy_raw, Mapping):
            raise TypeError("connect admission entry `policy` must be an object")

        recognized = {"app_id", "namespaces", "metadata", "policy"}
        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            app_id=app_id,
            namespaces=tuple(namespaces),
            metadata=dict(metadata_raw),
            policy=dict(policy_raw),
            extra=extra,
        )


@dataclass(frozen=True)
class ConnectAdmissionManifest:
    """Connect admission manifest describing allowed applications."""

    version: Optional[int]
    entries: Sequence[ConnectAdmissionManifestEntry]
    manifest_hash: Optional[str]
    updated_at: Optional[str]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAdmissionManifest":
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission manifest payload must be an object")
        data = dict(payload)
        entries_raw = data.get("entries") or []
        if entries_raw is None:
            entries_raw = []
        if not isinstance(entries_raw, list):
            raise TypeError("connect admission manifest `entries` must be a list")
        entries = [ConnectAdmissionManifestEntry.from_payload(item) for item in entries_raw]

        version_raw = data.get("version")
        if version_raw is None:
            version: Optional[int] = None
        else:
            try:
                version = int(version_raw)
            except (TypeError, ValueError) as exc:
                raise TypeError("connect admission manifest `version` must be numeric") from exc

        manifest_hash = data.get("manifest_hash")
        if manifest_hash is not None and not isinstance(manifest_hash, str):
            raise TypeError("connect admission manifest `manifest_hash` must be a string when present")
        updated_at = data.get("updated_at")
        if updated_at is not None and not isinstance(updated_at, str):
            raise TypeError("connect admission manifest `updated_at` must be a string when present")

        recognized = {"entries", "version", "manifest_hash", "updated_at"}
        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            version=version,
            entries=tuple(entries),
            manifest_hash=manifest_hash,
            updated_at=updated_at,
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the manifest to a JSON-serializable mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        payload["entries"] = [
            {
                "app_id": entry.app_id,
                "namespaces": list(entry.namespaces),
                "metadata": dict(entry.metadata),
                "policy": dict(entry.policy),
                **dict(entry.extra),
            }
            for entry in self.entries
        ]
        if self.version is not None:
            payload["version"] = self.version
        if self.manifest_hash is not None:
            payload["manifest_hash"] = self.manifest_hash
        if self.updated_at is not None:
            payload["updated_at"] = self.updated_at
        return payload

    # ------------------------------------------------------------------
@dataclass(frozen=True)
class ConnectAppPolicyControls:
    """Runtime-configurable Connect policy toggles."""

    relay_enabled: Optional[bool]
    ws_max_sessions: Optional[int]
    ws_per_ip_max_sessions: Optional[int]
    ws_rate_per_ip_per_min: Optional[int]
    session_ttl_ms: Optional[int]
    frame_max_bytes: Optional[int]
    session_buffer_max_bytes: Optional[int]
    ping_interval_ms: Optional[int]
    ping_miss_tolerance: Optional[int]
    ping_min_interval_ms: Optional[int]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppPolicyControls":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app policy payload must be an object")
        data = dict(payload)

        def _coerce_optional_int(name: str) -> Optional[int]:
            value = data.get(name)
            if value is None:
                return None
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"connect app policy field `{name}` must be numeric") from exc

        relay_enabled_raw = data.get("relay_enabled")
        relay_enabled: Optional[bool]
        if relay_enabled_raw is None:
            relay_enabled = None
        elif isinstance(relay_enabled_raw, bool):
            relay_enabled = relay_enabled_raw
        else:
            raise TypeError("connect app policy `relay_enabled` must be boolean when present")

        recognized = {
            "relay_enabled",
            "ws_max_sessions",
            "ws_per_ip_max_sessions",
            "ws_rate_per_ip_per_min",
            "session_ttl_ms",
            "frame_max_bytes",
            "session_buffer_max_bytes",
            "ping_interval_ms",
            "ping_miss_tolerance",
            "ping_min_interval_ms",
        }

        extra = {k: v for k, v in data.items() if k not in recognized}
        return cls(
            relay_enabled=relay_enabled,
            ws_max_sessions=_coerce_optional_int("ws_max_sessions"),
            ws_per_ip_max_sessions=_coerce_optional_int("ws_per_ip_max_sessions"),
            ws_rate_per_ip_per_min=_coerce_optional_int("ws_rate_per_ip_per_min"),
            session_ttl_ms=_coerce_optional_int("session_ttl_ms"),
            frame_max_bytes=_coerce_optional_int("frame_max_bytes"),
            session_buffer_max_bytes=_coerce_optional_int("session_buffer_max_bytes"),
            ping_interval_ms=_coerce_optional_int("ping_interval_ms"),
            ping_miss_tolerance=_coerce_optional_int("ping_miss_tolerance"),
            ping_min_interval_ms=_coerce_optional_int("ping_min_interval_ms"),
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the policy controls back to a JSON-serializable mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        if self.relay_enabled is not None:
            payload["relay_enabled"] = self.relay_enabled
        if self.ws_max_sessions is not None:
            payload["ws_max_sessions"] = self.ws_max_sessions
        if self.ws_per_ip_max_sessions is not None:
            payload["ws_per_ip_max_sessions"] = self.ws_per_ip_max_sessions
        if self.ws_rate_per_ip_per_min is not None:
            payload["ws_rate_per_ip_per_min"] = self.ws_rate_per_ip_per_min
        if self.session_ttl_ms is not None:
            payload["session_ttl_ms"] = self.session_ttl_ms
        if self.frame_max_bytes is not None:
            payload["frame_max_bytes"] = self.frame_max_bytes
        if self.session_buffer_max_bytes is not None:
            payload["session_buffer_max_bytes"] = self.session_buffer_max_bytes
        if self.ping_interval_ms is not None:
            payload["ping_interval_ms"] = self.ping_interval_ms
        if self.ping_miss_tolerance is not None:
            payload["ping_miss_tolerance"] = self.ping_miss_tolerance
        if self.ping_min_interval_ms is not None:
            payload["ping_min_interval_ms"] = self.ping_min_interval_ms
        return payload
