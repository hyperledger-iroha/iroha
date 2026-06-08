"""Shared privacy backend label validation helpers."""

from __future__ import annotations

import re
from typing import Any, Optional

_PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS = frozenset(
    {
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/kaigi-usage-v1",
        "halo2/pasta/ivm-overlay-bind",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/offline-note-recursive",
        "halo2/pasta/kagemusha-folded-v1",
        "halo2/pasta/kagemusha-recursive-aggregation-v1",
        "halo2/pasta/kagemusha-recursive-compact-v1",
        "halo2/pasta/kagemusha-recursive-spend-lineage-v1",
        "halo2/pasta/kagemusha-recursive-spend-lineage-onehop-v1",
        "halo2/pasta/kagemusha-recursive-spend-lineage-append-v1",
        "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
        "halo2/pasta/anon-unshield-merkle16-poseidon-diversified",
        "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified",
    }
)
_TRUSTED_SETUP_BACKEND_SEGMENTS = frozenset(
    {
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "powersoftau",
    }
)
_TRUSTED_SETUP_COMPACT_TOKENS = frozenset(
    {
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12381",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "trustedsetup",
        "structuredreferencestring",
        "universalsrs",
        "powersoftau",
    }
)

_PRODUCTION_CLAIM_BACKEND_FRAGMENTS = (
    "productionready",
    "productionhardened",
    "productionenabled",
    "productionapproved",
    "productioncertified",
    "productionclaim",
    "claimedproduction",
    "mainnetready",
    "mainnetcomplete",
    "mainnetclaim",
    "claimedmainnet",
    "mainnetcertified",
    "mainnetapproved",
    "mainnetrelease",
    "auditedproduction",
    "externallyaudited",
    "thirdpartyaudited",
    "boiaudited",
    "auditedmainnet",
    "externalaudit",
    "auditpassed",
    "auditapproved",
    "auditsignoff",
    "auditclaim",
    "claimedaudit",
    "securityreviewpassed",
    "securityauditpassed",
    "securityaudited",
    "externalsecurityreview",
    "certifiedproduction",
    "certifiedmainnet",
    "releaseready",
    "releaseapproved",
    "releasecertified",
)
_PENDING_PRODUCTION_BACKEND_ALIASES = frozenset(
    {
        "halo2ipaorchard",
        "orchard",
        "zcashorchard",
        "groth16bls12377",
        "groth16bls12377decaf377",
        "bls12377",
        "decaf377",
        "masp",
        "penumbra",
        "penumbramasp",
        "halo2ipapenumbra",
        "halo2ipamasp",
        "fcmppluspluscurvetree",
        "fcmp",
        "monero",
        "monerofcmp",
        "monerofcmpplusplus",
        "curvetree",
        "halo2ipamonero",
        "halo2ipacurvetree",
        "latticepcssis",
        "latticepcszk",
        "jindo",
        "jindolatticepcszk",
        "jindolatticepcszkv0",
        "jindolatticepcssis",
        "starkfrimiden",
        "midenstark",
        "aztecplonkishprivatekernel",
        "aztecprivatekernel",
        "pqmaspstarkfri",
        "pqmaspstark",
        "starkfripqmaspstarkfri",
        "postquantummasp",
        "anonymouspgc",
        "anonymouspgckoutofn",
        "anonymouspgckoutofnv1",
        "verange",
        "verangetransparentrange",
        "verangetransparentrangev1",
        "zkat",
        "zkatpolicyprivateauthenticator",
        "zkatpolicyprivateauthv1",
        "recursiveanonymousadmission",
        "recursiveanonymousadmissionv0",
        "zkamsrecursiveadmission",
        "zkamsrecursiveadmissionv0",
        "vegaexistingcredentialzk",
        "vegaexistingcredentialzkv0",
        "silentthresholdanoncred",
        "silentthresholdanoncredv0",
        "silentthresholdanonymouscredential",
        "thresholdanonymouscredentials",
        "zkx509",
        "zkvmx509identity",
        "zkx509onchainidentity",
        "zkx509onchainidentityv0",
        "siswithhints",
        "sishints",
        "sishintsanoncredpqv0",
        "latticeanonymouscredentials",
    }
)


def _compact_privacy_backend_label(value: str) -> str:
    return re.sub(r"[^a-z0-9]", "", value.strip().lower())


def _is_pending_production_backend_label(value: str) -> bool:
    compact = _compact_privacy_backend_label(value)
    return compact in _PENDING_PRODUCTION_BACKEND_ALIASES


def _is_trusted_setup_backend_label(value: str) -> bool:
    backend = value.strip().lower()
    compact = _compact_privacy_backend_label(value)
    return (
        any(
            segment in _TRUSTED_SETUP_BACKEND_SEGMENTS
            for segment in re.split(r"[^a-z0-9]+", backend)
        )
        or any(token in compact for token in _TRUSTED_SETUP_COMPACT_TOKENS)
        or backend == "groth16"
        or backend.startswith("groth16/")
        or backend == "kzg"
        or backend.startswith("kzg/")
        or backend == "bn254"
        or backend == "bn256"
        or backend == "bls12_381"
        or backend == "bls12-381"
        or backend == "halo2/bn254"
        or backend.startswith("halo2/bn254/")
        or "/bn254" in backend
        or ":bn254" in backend
        or "/bn256" in backend
        or ":bn256" in backend
        or "/bls12" in backend
        or ":bls12" in backend
        or backend == "halo2/kzg"
        or backend.startswith("halo2/kzg/")
        or "/kzg" in backend
        or ":kzg" in backend
    )


_DEVELOPER_ONLY_EMBEDDED_BACKEND_TOKENS = ("debug", "mock", "fixture", "dev")
_DEVELOPER_ONLY_EXACT_BACKEND_TOKENS = {
    "test",
    "dummy",
    "fake",
    "stub",
    "sample",
    "placeholder",
}

_STARK_FRI_PRODUCTION_BACKEND_LABELS = {
    "stark/fri",
    "stark/fri/sha256-goldilocks",
    "stark/fri/poseidon2-goldilocks",
    "stark/fri/sha256_goldilocks.v1",
}


def _is_developer_only_direct_backend_token(token: str) -> bool:
    return any(
        reserved in token for reserved in _DEVELOPER_ONLY_EMBEDDED_BACKEND_TOKENS
    ) or token in _DEVELOPER_ONLY_EXACT_BACKEND_TOKENS


def _is_developer_only_compact_backend_run(run: str) -> bool:
    return any(
        reserved in run for reserved in _DEVELOPER_ONLY_EMBEDDED_BACKEND_TOKENS
    ) or run in _DEVELOPER_ONLY_EXACT_BACKEND_TOKENS


def _is_developer_only_backend_label(value: str) -> bool:
    tokens = re.findall(r"[a-z0-9]+", value.strip().lower())
    letter_run = []
    for token in tokens:
        if _is_developer_only_direct_backend_token(token):
            return True
        if len(token) == 1:
            letter_run.append(token)
            continue
        if _is_developer_only_compact_backend_run("".join(letter_run)):
            return True
        letter_run = []
    return _is_developer_only_compact_backend_run("".join(letter_run))


def _is_production_claim_backend_label(value: str) -> bool:
    compact = _compact_privacy_backend_label(value)
    return any(fragment in compact for fragment in _PRODUCTION_CLAIM_BACKEND_FRAGMENTS)


def _is_portable_verify_backend_label(value: str) -> bool:
    return bool(
        re.fullmatch(r"[a-z0-9/_.:-]+", value)
        and re.match(r"[a-z0-9]", value)
        and re.search(r"[a-z0-9]$", value)
        and not any(
            separator in value
            for separator in ("//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:")
        )
    )


def _is_stark_fri_production_backend_label(backend: str) -> bool:
    return backend in _STARK_FRI_PRODUCTION_BACKEND_LABELS


def _normalize_native_halo2_pasta_backend_label(value: str) -> Optional[str]:
    backend = value
    if not backend or backend.strip() != backend:
        return None
    for prefix, target_prefix in (
        ("halo2/pasta/ipa/", "halo2/pasta/"),
        ("halo2/pasta/", "halo2/pasta/"),
        ("halo2/ipa::", "halo2/pasta/"),
        ("halo2/ipa:", "halo2/pasta/"),
        ("halo2/ipa/", "halo2/pasta/"),
    ):
        if backend.startswith(prefix):
            rest = backend[len(prefix):]
            return f"{target_prefix}{rest}" if rest else None
    return None


def _is_native_halo2_pasta_production_backend_label(backend: str) -> bool:
    normalized = _normalize_native_halo2_pasta_backend_label(backend)
    return normalized in _PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS


def _is_production_verify_backend_label(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    backend = value
    if (
        not backend
        or backend.strip() != backend
        or not _is_portable_verify_backend_label(backend)
        or _is_pending_production_backend_label(backend)
        or _is_production_claim_backend_label(backend)
        or _is_trusted_setup_backend_label(backend)
        or _is_developer_only_backend_label(backend)
    ):
        return False
    return (
        backend == "halo2/ipa"
        or _is_stark_fri_production_backend_label(backend)
        or _is_native_halo2_pasta_production_backend_label(backend)
    )


def _require_production_verify_backend_label(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    backend = value
    if not backend.strip():
        raise ValueError(f"{context} must be a non-empty string")
    if not _is_production_verify_backend_label(backend):
        raise ValueError(f"{context} uses unsupported production verifier backend {backend}")
    return backend
