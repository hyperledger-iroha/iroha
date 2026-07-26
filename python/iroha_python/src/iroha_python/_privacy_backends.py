"""Exact native verifier-registry labels shared by Python SDK surfaces."""

from __future__ import annotations

from typing import Any, Final, Literal

VerifierBackendTag = Literal["halo2-ipa-pasta", "stark"]

_HALO2_IPA_PASTA_REGISTRY_LABELS_V1: Final[frozenset[str]] = frozenset(
    {
        "halo2/ipa",
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/kaigi-usage-v1",
        "halo2/pasta/ivm-overlay-bind",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
        (
            "halo2/pasta/kagemusha-recursive-spend-step-eq-"
            "two-parent-operation-protocol-v2"
        ),
        (
            "halo2/pasta/kagemusha-recursive-spend-step-ep-"
            "two-parent-operation-protocol-v2"
        ),
        "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
    }
)
_STARK_REGISTRY_LABELS_V1: Final[frozenset[str]] = frozenset(
    {
        "stark/fri",
        "stark/fri/sha256-goldilocks",
        "stark/fri/poseidon2-goldilocks",
        "stark/fri/sha256_goldilocks.v1",
    }
)

# Public within the package so parity tests can compare this exact closed set.
_VERIFIER_BACKEND_REGISTRY_LABELS_V1: Final[frozenset[str]] = (
    _HALO2_IPA_PASTA_REGISTRY_LABELS_V1 | _STARK_REGISTRY_LABELS_V1
)


def _verifier_backend_registry_tag_v1(value: Any) -> VerifierBackendTag | None:
    """Resolve one exact registry label to its low-level proof engine."""

    if not isinstance(value, str):
        return None
    if value in _HALO2_IPA_PASTA_REGISTRY_LABELS_V1:
        return "halo2-ipa-pasta"
    if value in _STARK_REGISTRY_LABELS_V1:
        return "stark"
    return None


def _is_verifier_backend_registry_label_v1(value: Any) -> bool:
    """Return whether ``value`` is one byte-exact registry-v1 label."""

    return _verifier_backend_registry_tag_v1(value) is not None


def _require_verifier_backend_registry_label_v1(value: Any, context: str) -> str:
    """Require one byte-exact registry-v1 label and return it unchanged."""

    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    if not _is_verifier_backend_registry_label_v1(value):
        raise ValueError(
            f"{context} uses unsupported verifier-registry label {value}"
        )
    return value
