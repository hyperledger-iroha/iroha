"""Typed native-Norito Hijiri validation-fee quote helpers."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping

from ._native import load_crypto_extension

VALIDATION_FEE_HIJIRI_QUOTE_PATH = "/v1/validation-fee/hijiri/quote"
VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA = "iroha.torii.v1.validation_fee.hijiri_quote.response"
VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE = "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED"
VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES = 4 * 1024
VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES = 64 * 1024
VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS = 100_000
VALIDATION_FEE_HIJIRI_QUOTE_REQUIRED_BRIDGE_ABI_VERSION = 23

_PROJECTION_FIELDS = frozenset(
    {
        "accountId",
        "accountRiskDigest",
        "accountRiskRevision",
        "activePolicyHash",
        "activePolicyVersion",
        "adjustedPerTransferFeeMinorUnits",
        "aggregateAdjustedFeeMinorUnits",
        "aggregateBaseFeeMinorUnits",
        "assurance",
        "basePerTransferFeeMinorUnits",
        "defaultAccountRiskQ16",
        "effectiveAccountRiskQ16",
        "evaluatedStateHeight",
        "feeAssetDefinitionId",
        "feeMultiplierQ16",
        "feeScale",
        "hijiriFeeQuoteHash",
        "hijiriParametersDigest",
        "hijiriParametersRevision",
        "hijiriParametersVersion",
        "qualifyingTransferCount",
        "quotedExecutionHeight",
        "schema",
        "treasuryAccountId",
        "version",
    }
)


@dataclass(frozen=True)
class ValidationFeeHijiriQuoteV1:
    """One native-verified, evaluated-only Hijiri validation-fee quote."""

    schema: str
    version: int
    assurance: str
    evaluated_state_height: str
    quoted_execution_height: str
    account_id: str
    active_policy_version: str
    active_policy_hash: str
    fee_asset_definition_id: str
    treasury_account_id: str
    fee_scale: int
    hijiri_parameters_version: int
    hijiri_parameters_revision: str
    hijiri_parameters_digest: str
    default_account_risk_q16: int
    effective_account_risk_q16: int
    account_risk_revision: str | None
    account_risk_digest: str | None
    fee_multiplier_q16: int
    hijiri_fee_quote_hash: str
    base_per_transfer_fee_minor_units: str
    adjusted_per_transfer_fee_minor_units: str
    qualifying_transfer_count: int
    aggregate_base_fee_minor_units: str
    aggregate_adjusted_fee_minor_units: str

    @classmethod
    def from_native_projection(cls, value: Mapping[str, Any]) -> "ValidationFeeHijiriQuoteV1":
        """Construct from the closed projection emitted by the native verifier."""

        if set(value) != _PROJECTION_FIELDS:
            raise ValueError("native Hijiri quote projection has an unexpected field set")
        if (
            value["schema"] != VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA
            or value["version"] != 1
            or value["assurance"] != VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE
            or value["hijiriParametersVersion"] != 1
        ):
            raise ValueError("native Hijiri quote projection markers are invalid")
        if (value["accountRiskRevision"] is None) != (value["accountRiskDigest"] is None):
            raise ValueError(
                "native Hijiri quote risk revision and digest must be present together"
            )
        return cls(
            schema=_exact_string(value["schema"], "schema"),
            version=_exact_u32(value["version"], "version"),
            assurance=_exact_string(value["assurance"], "assurance"),
            evaluated_state_height=_positive_decimal(
                value["evaluatedStateHeight"], "evaluatedStateHeight"
            ),
            quoted_execution_height=_positive_decimal(
                value["quotedExecutionHeight"], "quotedExecutionHeight"
            ),
            account_id=_exact_string(value["accountId"], "accountId"),
            active_policy_version=_positive_decimal(
                value["activePolicyVersion"], "activePolicyVersion"
            ),
            active_policy_hash=_lower_hex_32(value["activePolicyHash"], "activePolicyHash"),
            fee_asset_definition_id=_exact_string(
                value["feeAssetDefinitionId"], "feeAssetDefinitionId"
            ),
            treasury_account_id=_exact_string(value["treasuryAccountId"], "treasuryAccountId"),
            fee_scale=_exact_u32(value["feeScale"], "feeScale"),
            hijiri_parameters_version=_exact_u32(
                value["hijiriParametersVersion"], "hijiriParametersVersion"
            ),
            hijiri_parameters_revision=_positive_decimal(
                value["hijiriParametersRevision"], "hijiriParametersRevision"
            ),
            hijiri_parameters_digest=_lower_hex_32(
                value["hijiriParametersDigest"], "hijiriParametersDigest"
            ),
            default_account_risk_q16=_exact_u32(
                value["defaultAccountRiskQ16"], "defaultAccountRiskQ16"
            ),
            effective_account_risk_q16=_exact_u32(
                value["effectiveAccountRiskQ16"], "effectiveAccountRiskQ16"
            ),
            account_risk_revision=_optional_positive_decimal(
                value["accountRiskRevision"], "accountRiskRevision"
            ),
            account_risk_digest=_optional_lower_hex_32(
                value["accountRiskDigest"], "accountRiskDigest"
            ),
            fee_multiplier_q16=_exact_u32(value["feeMultiplierQ16"], "feeMultiplierQ16"),
            hijiri_fee_quote_hash=_lower_hex_32(value["hijiriFeeQuoteHash"], "hijiriFeeQuoteHash"),
            base_per_transfer_fee_minor_units=_positive_decimal(
                value["basePerTransferFeeMinorUnits"],
                "basePerTransferFeeMinorUnits",
            ),
            adjusted_per_transfer_fee_minor_units=_positive_decimal(
                value["adjustedPerTransferFeeMinorUnits"],
                "adjustedPerTransferFeeMinorUnits",
            ),
            qualifying_transfer_count=_exact_u32(
                value["qualifyingTransferCount"], "qualifyingTransferCount"
            ),
            aggregate_base_fee_minor_units=_positive_decimal(
                value["aggregateBaseFeeMinorUnits"], "aggregateBaseFeeMinorUnits"
            ),
            aggregate_adjusted_fee_minor_units=_positive_decimal(
                value["aggregateAdjustedFeeMinorUnits"],
                "aggregateAdjustedFeeMinorUnits",
            ),
        )


def _native_binding() -> Any:
    native = load_crypto_extension()
    if (
        not callable(getattr(native, "connect_norito_bridge_abi_version", None))
        or native.connect_norito_bridge_abi_version()
        != VALIDATION_FEE_HIJIRI_QUOTE_REQUIRED_BRIDGE_ABI_VERSION
        or not callable(getattr(native, "validation_fee_hijiri_quote_request_v1", None))
        or not callable(getattr(native, "validation_fee_verify_hijiri_quote_response_v1", None))
    ):
        raise RuntimeError(
            "iroha_python._crypto lacks the ABI 23 Hijiri validation-fee quote codec"
        )
    return native


def encode_validation_fee_hijiri_quote_request_v1(
    account_id: str,
    qualifying_transfer_count: int,
) -> bytes:
    """Encode one exact bounded V1 request through the native Rust layout."""

    if isinstance(qualifying_transfer_count, bool) or not isinstance(
        qualifying_transfer_count, int
    ):
        raise TypeError("qualifying_transfer_count must be an integer")
    if not 1 <= qualifying_transfer_count <= VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS:
        raise ValueError(
            "qualifying_transfer_count must be between 1 and "
            f"{VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS}"
        )
    if not isinstance(account_id, str) or not account_id:
        raise TypeError("account_id must be a non-empty canonical I105 account id")
    encoded = _native_binding().validation_fee_hijiri_quote_request_v1(
        account_id, qualifying_transfer_count
    )
    if not isinstance(encoded, (bytes, bytearray, memoryview)):
        raise RuntimeError("native Hijiri quote request encoder returned non-bytes")
    request = bytes(encoded)
    if not 1 <= len(request) <= VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES:
        raise RuntimeError("native Hijiri quote request encoder returned invalid bytes")
    return request


def verify_validation_fee_hijiri_quote_response_v1(
    response_norito: bytes | bytearray | memoryview,
    request_norito: bytes | bytearray | memoryview,
) -> ValidationFeeHijiriQuoteV1:
    """Verify one canonical response against the exact native request archive."""

    _require_bounded_bytes(
        response_norito,
        VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES,
        "response_norito",
    )
    _require_bounded_bytes(
        request_norito,
        VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES,
        "request_norito",
    )
    response = memoryview(response_norito).tobytes()
    request = memoryview(request_norito).tobytes()
    projection_json = _native_binding().validation_fee_verify_hijiri_quote_response_v1(
        response, request
    )
    if not isinstance(projection_json, str) or not projection_json:
        raise RuntimeError("native Hijiri quote verifier returned no projection")
    if len(projection_json.encode("utf-8")) > VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES:
        raise RuntimeError("native Hijiri quote projection exceeds its byte bound")
    projection = json.loads(projection_json, object_pairs_hook=_closed_json_object)
    if not isinstance(projection, Mapping):
        raise RuntimeError("native Hijiri quote verifier returned a non-object projection")
    return ValidationFeeHijiriQuoteV1.from_native_projection(projection)


def _closed_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"native Hijiri quote projection duplicated field {key!r}")
        result[key] = value
    return result


def _require_bounded_bytes(value: Any, maximum: int, field: str) -> None:
    if not isinstance(value, (bytes, bytearray, memoryview)):
        raise TypeError(f"{field} must be bytes-like")
    if isinstance(value, bytes):
        length = bytes.__len__(value)
    elif isinstance(value, bytearray):
        length = bytearray.__len__(value)
    else:
        length = value.nbytes
    if not 1 <= length <= maximum:
        raise ValueError(f"{field} must contain 1..{maximum} bytes")


def _exact_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"native Hijiri quote {field} must be a non-empty string")
    return value


def _exact_u32(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 0xFFFF_FFFF:
        raise ValueError(f"native Hijiri quote {field} must be an unsigned 32-bit integer")
    return value


def _positive_decimal(value: Any, field: str) -> str:
    result = _exact_string(value, field)
    if not result.isascii() or not result.isdecimal() or result.startswith("0"):
        raise ValueError(f"native Hijiri quote {field} must be a canonical positive decimal")
    return result


def _optional_positive_decimal(value: Any, field: str) -> str | None:
    return None if value is None else _positive_decimal(value, field)


def _lower_hex_32(value: Any, field: str) -> str:
    result = _exact_string(value, field)
    if len(result) != 64 or any(character not in "0123456789abcdef" for character in result):
        raise ValueError(f"native Hijiri quote {field} must be one lowercase 32-byte hash")
    return result


def _optional_lower_hex_32(value: Any, field: str) -> str | None:
    return None if value is None else _lower_hex_32(value, field)


__all__ = [
    "VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE",
    "VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES",
    "VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES",
    "VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS",
    "VALIDATION_FEE_HIJIRI_QUOTE_PATH",
    "VALIDATION_FEE_HIJIRI_QUOTE_REQUIRED_BRIDGE_ABI_VERSION",
    "VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA",
    "ValidationFeeHijiriQuoteV1",
    "encode_validation_fee_hijiri_quote_request_v1",
    "verify_validation_fee_hijiri_quote_response_v1",
]
