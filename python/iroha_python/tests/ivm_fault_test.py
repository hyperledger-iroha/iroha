"""Closed deterministic IVM fault payload validation."""

from copy import deepcopy

import pytest

from iroha_python.client import _normalize_ivm_fault


def fault():
    return {
        "kind": {"kind": "Numeric", "value": {"kind": "DivisionByZero", "value": None}},
        "site": {
            "code_hash": "11" * 32,
            "selector": {"kind": "Entrypoint", "value": (1 << 32) - 1},
            "position": {"kind": "Execute", "value": {"pc_offset": (1 << 64) - 1}},
        },
    }


# `IvmFaultKindV1` in codec-index order (0..=27).
FAULT_KINDS = (
    "OutOfGas", "MemoryLimitExceeded", "MemoryAccessViolation", "MisalignedAccess", "MemoryOutOfBounds",
    "DecodeError", "InvalidOpcode", "UnknownSyscall", "UnsupportedSyscall", "GasCostOverflow", "Numeric",
    "PointerAbi", "AssertionFailed", "ExceededMaxCycles", "InvalidMetadata", "MissingHalt",
    "NullifierAlreadyUsed", "PermissionDenied", "PrivacyViolation", "RegisterOutOfBounds", "NoritoInvalid",
    "AbiTypeNotAllowed", "HostOutputItemsExceeded", "HostOutputBytesExceeded", "AmxBudgetExceeded",
    "ReentrantCall", "CallDepthExceeded", "ControlFlowViolation",
)
# `NumericFaultV1` in ABI tag order (1..=14).
NUMERIC_FAULTS = (
    "MantissaOverflow", "ScaleOverflow", "DivisionByZero", "RepeatingDecimal", "ExactDivisionScaleOverflow",
    "InvalidScale", "InexactConversion", "NegativeQuantity", "QuantityUnderflow", "InvalidRoundingMode",
    "InvalidFailureMode", "ReservedRegisterNonZero", "NegativeSquareRoot", "RegisterIntegerOverflow",
)
# `PointerAbiFaultV1` in ABI tag order (1..=11).
POINTER_ABI_FAULTS = (
    "InvalidAddress", "UnknownType", "TypeNotAllowed", "WrongType", "InvalidEnvelopeVersion", "OversizedLength",
    "TruncatedEnvelope", "PayloadHashMismatch", "MalformedFrame", "SchemaMismatch", "NonCanonical",
)


def test_closed_fault_catalog_has_the_current_codec_cardinality():
    assert len(FAULT_KINDS) == 28
    assert len(NUMERIC_FAULTS) == 14
    assert len(POINTER_ABI_FAULTS) == 11


@pytest.mark.parametrize(
    "kind",
    [{"kind": name, "value": None} for name in FAULT_KINDS if name not in ("Numeric", "PointerAbi")]
    + [{"kind": "Numeric", "value": {"kind": code, "value": None}} for code in NUMERIC_FAULTS]
    + [{"kind": "PointerAbi", "value": {"kind": code, "value": None}} for code in POINTER_ABI_FAULTS],
    ids=lambda kind: kind["kind"] if kind["value"] is None else f'{kind["kind"]}({kind["value"]["kind"]})',
)
def test_every_current_fault_category_and_subtype_is_accepted(kind):
    value = fault()
    value["kind"] = kind
    assert _normalize_ivm_fault(value)["kind"] == kind


@pytest.mark.parametrize(
    "kind",
    [
        {"kind": "InvalidVectorLength", "value": None},
        {"kind": "VectorExtensionDisabled", "value": None},
        {"kind": "ZkExtensionDisabled", "value": None},
        {"kind": "Numeric", "value": {"kind": "ControlFlowViolation", "value": None}},
        {"kind": "PointerAbi", "value": {"kind": "RegisterIntegerOverflow", "value": None}},
        {"kind": "ControlFlowViolation", "value": {"kind": "DivisionByZero", "value": None}},
    ],
    ids=["InvalidVectorLength", "VectorExtensionDisabled", "ZkExtensionDisabled", "numeric-cross", "pointer-cross", "unit-payload"],
)
def test_retired_or_cross_category_fault_tags_are_rejected(kind):
    value = fault()
    value["kind"] = kind
    with pytest.raises(ValueError):
        _normalize_ivm_fault(value)


def test_typed_fault_retains_exact_unsigned_origin_and_snapshots_input():
    value = fault()
    normalized = _normalize_ivm_fault(value)
    assert normalized == value
    value["site"]["position"]["value"]["pc_offset"] = 0
    assert normalized["site"]["position"]["value"]["pc_offset"] == (1 << 64) - 1


@pytest.mark.parametrize("mutation", ["unknown", "subtype", "ordinal", "pc", "bool", "extra", "missing", "stage"])
def test_typed_fault_rejects_invalid_closed_shape(mutation):
    value = deepcopy(fault())
    if mutation == "unknown":
        value["kind"]["kind"] = "DebugString"
    elif mutation == "subtype":
        value["kind"]["value"]["kind"] = "WrongType"
    elif mutation == "ordinal":
        value["site"]["selector"]["value"] = 1 << 32
    elif mutation == "pc":
        value["site"]["position"]["value"]["pc_offset"] = 1 << 64
    elif mutation == "bool":
        value["site"]["position"]["value"]["pc_offset"] = True
    elif mutation == "extra":
        value["site"]["local_pointer"] = 7
    elif mutation == "missing":
        del value["site"]["code_hash"]
    elif mutation == "stage":
        value["site"]["position"]["kind"] = "ReturnValidation"
    with pytest.raises(ValueError):
        _normalize_ivm_fault(value)
