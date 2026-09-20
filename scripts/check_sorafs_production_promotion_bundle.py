#!/usr/bin/env python3
"""Verify the final payload-free SoraFS production-promotion bundle.

Prerequisites are existing positive replay outputs, one locally qualified
six-case negative archive, an externally signed promotion-provenance receipt,
and the exact cosign bundle named by that receipt.  The checker is read-only:
it emits a schema-closed digest summary on stdout and never creates evidence.
It requires no environment variables and accepts reviewed ``@ARGFILE`` input.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import run_sorafs_production_readiness as promotion_runner  # noqa: E402
import run_sorafs_production_readiness_negative_archive as negative_runner  # noqa: E402
import sorafs_final_promotion_cosign as final_promotion_cosign  # noqa: E402
import sorafs_final_promotion_evidence as final_promotion_evidence  # noqa: E402
from check_sorafs_production_readiness import (  # noqa: E402
    MAX_SUMMARY_BYTES,
    canonical_lower_hex,
    canonical_public_provenance_url,
    canonical_string,
)
from sccp_release_common import verify_ed25519  # noqa: E402
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_lines,
    emit_checker_exception,
    render_checker_summary,
)
from sorafs_evidence_json import decode_evidence_json, read_evidence_bytes  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


PROMOTION_PROVENANCE_SCHEMA = (
    "sorafs.production_readiness.production_promotion_provenance.v1"
)
PROMOTION_SUMMARY_SCHEMA = (
    "sorafs.production_readiness.production_promotion_summary.v1"
)
PROMOTION_ATTESTATION_SCOPE = "production-promotion-bundle"
PROMOTION_PROVENANCE_SIGNATURE_DOMAIN = (
    b"iroha:sorafs:production-readiness:production-promotion-provenance:v1\x00"
)
PROMOTION_COSIGN_SUBJECT_DOMAIN = (
    b"iroha:sorafs:production-readiness:production-promotion-cosign-subject:v1\x00"
)
REQUIRED_SIGNING_PROVIDER = "authenticated_external_signer"
DEFAULT_MAX_PROVENANCE_AGE_SECS = 14 * 24 * 60 * 60
MAX_TIMESTAMP = (1 << 63) - 1
MAX_PROMOTION_PROVENANCE_BYTES = 256 * 1024
MAX_COSIGN_BUNDLE_BYTES = 16 * 1024 * 1024

PROMOTION_PROVENANCE_FIELDS = frozenset(
    {
        "schema",
        "status",
        "attestation_scope",
        "generated_at_unix",
        "chain_id",
        "network_id_hex",
        "deployment_id",
        "signing_provider",
        "baseline_input_count",
        "baseline_input_set_sha256",
        "negative_archive_manifest_sha256",
        "negative_receipts",
        "aggregate_runner_sha256",
        "aggregate_checker_sha256",
        "aggregate_toolchain_sha256",
        "python_runtime",
        "positive_output_sha256",
        "cosign_bundle_sha256",
        "provenance_certificate_identity",
        "provenance_oidc_issuer",
        "oidc_identity_status",
        "cosign_provenance_status",
        "authentication",
        "errors",
    }
)
AUTHENTICATION_FIELDS = frozenset(
    {
        "kind", "algorithm", "service_id", "administrator_id",
        "key_revision", "policy_revision", "policy_digest_sha256",
        "public_key_fingerprint_sha256", "signature_hex",
    }
)
PROMOTION_COSIGN_SUBJECT_FIELDS = PROMOTION_PROVENANCE_FIELDS - {"cosign_bundle_sha256"}

PROMOTION_SUMMARY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "attestation_scope",
        "externally_authenticated",
        "promotion_eligible",
        "baseline_input_count",
        "baseline_input_set_sha256",
        "positive_output_sha256",
        "negative_archive_manifest_sha256",
        "negative_receipt_count",
        "negative_receipts",
        "aggregate_runner_sha256",
        "aggregate_checker_sha256",
        "aggregate_toolchain_sha256",
        "python_runtime",
        "cosign_bundle_sha256",
        "provenance_certificate_identity",
        "provenance_oidc_issuer",
        "errors",
    }
)


@dataclass(frozen=True)
class CosignBundleEvidence:
    """Exact captured bundle bytes and their digest; this is not a verification result."""

    raw: bytes
    sha256: str


@dataclass(frozen=True)
class PositiveReplayEvidence:
    """Validated digest-only view of the two positive aggregate executions."""

    input_count: int
    input_set_sha256: str
    output_sha256: dict[str, str]


@dataclass(frozen=True)
class NegativeArchiveEvidence:
    """Validated digest-only view of the fixed negative archive."""

    baseline_input_count: int
    baseline_input_set_sha256: str
    manifest_sha256: str
    receipts: tuple[dict[str, str], ...]
    runner_sha256: str
    checker_sha256: str
    toolchain_sha256: str
    python_runtime: dict[str, str]
    baseline_output_sha256: dict[str, str]


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _canonical_nonzero_sha256(value: Any) -> str | None:
    digest = canonical_lower_hex(value, 64)
    return digest if digest is not None and any(bytes.fromhex(digest)) else None


def _canonical_signature(value: Any) -> bytes | None:
    signature = canonical_lower_hex(value, 128)
    if signature is None:
        return None
    decoded = bytes.fromhex(signature)
    return decoded if any(decoded) else None


def _load_json_bytes(
    path: Path,
    maximum: int,
    *,
    label: str,
    errors: list[str],
) -> tuple[dict[str, Any], bytes] | None:
    try:
        raw = read_evidence_bytes(path, maximum)
        return decode_evidence_json(raw), raw
    except (OSError, RuntimeError, UnicodeDecodeError, ValueError):
        errors.append(f"{label} must be a bounded strict JSON object")
        return None


def _snapshot_from_replay_manifest(
    manifest: Mapping[str, Any],
) -> promotion_runner.InputDigestSnapshot | None:
    rows = manifest.get("input_sha256")
    if not isinstance(rows, list):
        return None
    snapshot: list[tuple[str, str]] = []
    for row in rows:
        if not isinstance(row, Mapping):
            return None
        slot = row.get("slot")
        digest = row.get("sha256")
        if not isinstance(slot, str) or not isinstance(digest, str):
            return None
        snapshot.append((slot, digest))
    return tuple(snapshot)


def load_positive_replay(
    first_aggregate_path: Path,
    second_aggregate_path: Path,
    replay_manifest_path: Path,
) -> tuple[PositiveReplayEvidence | None, list[str]]:
    """Revalidate the exact positive aggregate pair and its 22-input manifest."""

    errors: list[str] = []
    replay, replay_errors = promotion_runner.load_and_validate_replayed_aggregates(
        first_aggregate_path,
        second_aggregate_path,
    )
    errors.extend(replay_errors)
    loaded_manifest = _load_json_bytes(
        replay_manifest_path,
        MAX_SUMMARY_BYTES,
        label="deterministic replay manifest",
        errors=errors,
    )
    if replay is None or loaded_manifest is None:
        return None, errors
    manifest, manifest_raw = loaded_manifest
    snapshot = _snapshot_from_replay_manifest(manifest)
    if snapshot is None:
        errors.append(
            "deterministic replay manifest input inventory must be an ordered digest array"
        )
        return None, errors
    errors.extend(
        promotion_runner.validate_replay_manifest(manifest, snapshot, replay)
    )
    if errors:
        return None, errors
    return (
        PositiveReplayEvidence(
            input_count=len(snapshot),
            input_set_sha256=promotion_runner.input_set_sha256(snapshot),
            output_sha256={
                "first_aggregate_sha256": replay.first_sha256,
                "second_aggregate_sha256": replay.second_sha256,
                "aggregate_semantic_sha256": replay.semantic_sha256,
                "replay_manifest_sha256": _sha256(manifest_raw),
            },
        ),
        [],
    )


def _archive_directory_names(
    archive_dir: Path,
    errors: list[str],
) -> tuple[str, ...] | None:
    try:
        if archive_dir.is_symlink() or not archive_dir.is_dir():
            errors.append("negative-promotion archive must be an existing directory")
            return None
        return tuple(sorted(path.name for path in archive_dir.iterdir()))
    except (OSError, RuntimeError):
        errors.append("negative-promotion archive directory could not be inspected")
        return None


def load_negative_archive(
    archive_dir: Path,
) -> tuple[NegativeArchiveEvidence | None, list[str]]:
    """Revalidate the local manifest and all six exact ordered receipts."""

    errors: list[str] = []
    expected_files = tuple(
        f"{index:02d}-{case.mutation_id}.json"
        for index, case in enumerate(negative_runner.MUTATION_CASES, start=1)
    )
    expected_inventory = tuple(
        sorted((*expected_files, negative_runner.ARCHIVE_MANIFEST_FILENAME))
    )
    before_inventory = _archive_directory_names(archive_dir, errors)
    if before_inventory is not None and before_inventory != expected_inventory:
        errors.append(
            "negative-promotion archive must contain exactly the manifest and six matrix receipts"
        )

    loaded_manifest = _load_json_bytes(
        archive_dir / negative_runner.ARCHIVE_MANIFEST_FILENAME,
        MAX_SUMMARY_BYTES,
        label="negative-promotion archive manifest",
        errors=errors,
    )
    if loaded_manifest is None:
        return None, errors
    manifest, manifest_raw = loaded_manifest
    runtime_value = manifest.get("python_runtime")
    if isinstance(runtime_value, Mapping):
        python_runtime = negative_runner.PythonRuntime(
            executable=Path("."),
            implementation=runtime_value.get("implementation"),
            version=runtime_value.get("version"),
            executable_sha256=runtime_value.get("executable_sha256"),
        )
    else:
        python_runtime = negative_runner.PythonRuntime(
            executable=Path("."),
            implementation="",
            version="",
            executable_sha256="",
        )
    errors.extend(
        negative_runner.validate_archive_manifest(
            manifest,
            baseline_input_set_sha256=manifest.get("baseline_input_set_sha256"),
            runner_sha256=manifest.get("aggregate_runner_sha256"),
            checker_sha256=manifest.get("aggregate_checker_sha256"),
            toolchain_sha256=manifest.get("aggregate_toolchain_sha256"),
            python_runtime=python_runtime,
        )
    )
    if isinstance(runtime_value, Mapping):
        for field in ("implementation", "version"):
            if canonical_string(runtime_value.get(field)) is None:
                errors.append(
                    f"negative-promotion archive Python runtime {field} must be canonical text"
                )
    for field in (
        "baseline_input_set_sha256",
        "aggregate_runner_sha256",
        "aggregate_checker_sha256",
        "aggregate_toolchain_sha256",
    ):
        if _canonical_nonzero_sha256(manifest.get(field)) is None:
            errors.append(
                f"negative-promotion archive {field} must be non-zero SHA-256"
            )

    receipt_rows = manifest.get("receipts")
    rows_by_index = receipt_rows if isinstance(receipt_rows, list) else []
    validated_rows: list[dict[str, str]] = []
    for index, (case, filename) in enumerate(
        zip(negative_runner.MUTATION_CASES, expected_files),
    ):
        loaded_receipt = _load_json_bytes(
            archive_dir / filename,
            MAX_SUMMARY_BYTES,
            label=f"negative-promotion receipt {index + 1}",
            errors=errors,
        )
        if loaded_receipt is None:
            continue
        receipt, receipt_raw = loaded_receipt
        row = rows_by_index[index] if index < len(rows_by_index) else None
        receipt_sha256 = _sha256(receipt_raw)
        if (
            not isinstance(row, Mapping)
            or row.get("mutation_id") != case.mutation_id
            or row.get("receipt_file") != filename
            or row.get("sha256") != receipt_sha256
        ):
            errors.append(
                f"negative-promotion receipt {index + 1} must match its manifest binding"
            )
        errors.extend(
            negative_runner.validate_receipt(
                receipt,
                case=case,
                baseline_input_set_sha256=manifest.get(
                    "baseline_input_set_sha256"
                ),
                checker_sha256=manifest.get("aggregate_checker_sha256"),
                toolchain_sha256=manifest.get("aggregate_toolchain_sha256"),
            )
        )
        validated_rows.append(
            {
                "mutation_id": case.mutation_id,
                "receipt_file": filename,
                "sha256": receipt_sha256,
            }
        )

    after_inventory = _archive_directory_names(archive_dir, errors)
    if before_inventory is not None and after_inventory != before_inventory:
        errors.append("negative-promotion archive changed while it was verified")
    if errors:
        return None, errors
    baseline_hashes = manifest["baseline_output_sha256"]
    return (
        NegativeArchiveEvidence(
            baseline_input_count=manifest["baseline_input_count"],
            baseline_input_set_sha256=manifest["baseline_input_set_sha256"],
            manifest_sha256=_sha256(manifest_raw),
            receipts=tuple(validated_rows),
            runner_sha256=manifest["aggregate_runner_sha256"],
            checker_sha256=manifest["aggregate_checker_sha256"],
            toolchain_sha256=manifest["aggregate_toolchain_sha256"],
            python_runtime=dict(manifest["python_runtime"]),
            baseline_output_sha256=dict(baseline_hashes),
        ),
        [],
    )


def load_cosign_bundle(
    path: Path | None,
) -> tuple[CosignBundleEvidence | None, list[str]]:
    """Capture the exact bounded Sigstore v0.3 bundle once for binding and verification."""

    if path is None:
        return None, ["production promotion requires an exact cosign bundle"]
    try:
        raw = read_evidence_bytes(path, MAX_COSIGN_BUNDLE_BYTES)
        final_promotion_cosign.validate_cosign_bundle(raw)
    except (OSError, RuntimeError, UnicodeDecodeError, ValueError):
        return None, ["cosign provenance bundle must be a bounded strict JSON object in Sigstore v0.3 format"]
    return CosignBundleEvidence(raw, _sha256(raw)), []


def _unsigned_promotion_body(payload: Mapping[str, Any]) -> dict[str, Any]:
    if set(payload) != PROMOTION_PROVENANCE_FIELDS:
        raise ValueError("production promotion provenance has the wrong exact schema")
    unsigned = dict(payload)
    authentication = unsigned.get("authentication")
    if not isinstance(authentication, Mapping) or set(authentication) != AUTHENTICATION_FIELDS:
        raise ValueError("production promotion authentication has the wrong exact schema")
    unsigned_authentication = dict(authentication)
    unsigned_authentication.pop("signature_hex", None)
    unsigned["authentication"] = unsigned_authentication
    return unsigned


def _encode_promotion_subject(unsigned: Mapping[str, Any], domain: bytes) -> bytes:
    try:
        encoded = json.dumps(
            unsigned,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ValueError(
            "production promotion provenance is not canonically encodable"
        ) from error
    message = domain + encoded
    if len(message) > MAX_PROMOTION_PROVENANCE_BYTES:
        raise ValueError("production promotion signing statement exceeds the byte limit")
    return message


def promotion_provenance_signing_payload(payload: Mapping[str, Any]) -> bytes:
    """Return the exact domain-separated bytes the configured signer authenticates."""

    return _encode_promotion_subject(_unsigned_promotion_body(payload), PROMOTION_PROVENANCE_SIGNATURE_DOMAIN)


def promotion_cosign_subject_bytes(unsigned: Mapping[str, Any]) -> bytes:
    """Encode the closed 23-field subject before any bundle or signer signature exists.

    This codec checks the exact unsigned shape; the caller owns evidence and trust validation.
    Every final-provenance field except its bundle hash and detached signature remains bound.
    """

    if not isinstance(unsigned, Mapping) or set(unsigned) != PROMOTION_COSIGN_SUBJECT_FIELDS:
        raise ValueError("promotion cosign subject has the wrong exact schema")
    authentication = unsigned.get("authentication")
    if not isinstance(authentication, Mapping) or set(authentication) != AUTHENTICATION_FIELDS - {"signature_hex"}:
        raise ValueError("promotion cosign authentication has the wrong exact schema")
    return _encode_promotion_subject(unsigned, PROMOTION_COSIGN_SUBJECT_DOMAIN)


def _expected_provenance_binding(
    positive: PositiveReplayEvidence,
    negative: NegativeArchiveEvidence,
    cosign_bundle_sha256: str,
) -> dict[str, Any]:
    return {
        "baseline_input_count": positive.input_count,
        "baseline_input_set_sha256": positive.input_set_sha256,
        "negative_archive_manifest_sha256": negative.manifest_sha256,
        "negative_receipts": [dict(row) for row in negative.receipts],
        "aggregate_runner_sha256": negative.runner_sha256,
        "aggregate_checker_sha256": negative.checker_sha256,
        "aggregate_toolchain_sha256": negative.toolchain_sha256,
        "python_runtime": dict(negative.python_runtime),
        "positive_output_sha256": dict(positive.output_sha256),
        "cosign_bundle_sha256": cosign_bundle_sha256,
    }


def _canonical_identity(value: Any) -> str | None:
    """Match the Manifest custody identity grammar without normalizing supplied text."""

    return (
        value
        if isinstance(value, str)
        and re.fullmatch(r"[A-Za-z0-9._:-]{1,128}", value) is not None
        and not {
            "null", "mock", "test", "dev", "demo", "fake", "dummy", "placeholder",
        }.intersection(re.split(r"[^A-Za-z0-9]+", value.lower()))
        else None
    )


def _canonical_chain_id(value: Any) -> str | None:
    """Match the shared exact ASCII chain-label grammar."""

    return (
        value
        if isinstance(value, str)
        and re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9._:-]{0,126}[A-Za-z0-9])?", value)
        is not None
        else None
    )


def _validate_signer(
    row: Mapping[str, Any], errors: list[str],
) -> dict[str, Any]:
    """Validate public coordinates; only the native receipt establishes signer custody."""

    result = {f"signer_{field}": row.get(field) for field in (
        "service_id", "administrator_id", "key_revision",
        "policy_revision", "policy_digest_sha256",
    )}
    for field in ("service_id", "administrator_id"):
        if _canonical_identity(row.get(field)) is None:
            errors.append(f"final promotion signer {field} must be a canonical custody identity")
    if row.get("service_id") == row.get("administrator_id"):
        errors.append("final promotion signer service_id and administrator_id must differ")
    for field in ("key_revision", "policy_revision"):
        value = row.get(field)
        if type(value) is not int or not 0 < value <= (1 << 64) - 1:
            errors.append(f"final promotion signer {field} must be in 1..2^64-1")
    if _canonical_nonzero_sha256(row.get("policy_digest_sha256")) is None:
        errors.append("final promotion signer policy_digest_sha256 must be non-zero canonical SHA-256")
    return result


def _validate_operator_signer_tuple(
    *,
    service_id: str | None,
    administrator_id: str | None,
    key_revision: int | None,
    policy_revision: int | None,
    policy_digest_sha256: str | None,
) -> tuple[dict[str, Any], list[str]]:
    expected = {
        "signer_service_id": service_id,
        "signer_administrator_id": administrator_id,
        "signer_key_revision": key_revision,
        "signer_policy_revision": policy_revision,
        "signer_policy_digest_sha256": policy_digest_sha256,
    }
    errors: list[str] = []
    _validate_signer({field.removeprefix("signer_"): value for field, value in expected.items()}, errors)
    return expected, errors


def validate_promotion_provenance(
    payload: object,
    *,
    positive: PositiveReplayEvidence,
    negative: NegativeArchiveEvidence,
    cosign_bundle_sha256: str,
    trusted_public_key: bytes | None,
    trusted_service_id: str | None,
    trusted_administrator_id: str | None,
    trusted_key_revision: int | None,
    trusted_policy_revision: int | None,
    trusted_policy_digest_sha256: str | None,
    trusted_certificate_identity: str | None,
    trusted_oidc_issuer: str | None,
    trusted_chain_id: str | None,
    trusted_network_id_hex: str | None,
    trusted_deployment_id: str | None,
    now_unix: int,
    max_provenance_age_secs: int,
) -> list[str]:
    """Validate exact final-provenance structure and signature before native receipt verification."""

    if not isinstance(payload, Mapping):
        return ["production promotion provenance must be an object"]
    errors: list[str] = []
    if set(payload) != PROMOTION_PROVENANCE_FIELDS:
        errors.append(
            "production promotion provenance fields must match the schema-closed contract"
        )
    exact_values = {
        "schema": PROMOTION_PROVENANCE_SCHEMA,
        "status": "verified",
        "attestation_scope": PROMOTION_ATTESTATION_SCOPE,
        "signing_provider": REQUIRED_SIGNING_PROVIDER,
        "oidc_identity_status": "verified",
        "cosign_provenance_status": "verified",
    }
    for field, expected in exact_values.items():
        if payload.get(field) != expected:
            errors.append(
                f"production promotion provenance {field} must be `{expected}`"
            )
    if payload.get("errors") != []:
        errors.append("production promotion provenance errors must be empty")

    for field, expected, canonical in (
        ("chain_id", trusted_chain_id, _canonical_chain_id),
        ("network_id_hex", trusted_network_id_hex, _canonical_nonzero_sha256),
        ("deployment_id", trusted_deployment_id, _canonical_identity),
    ):
        if canonical(expected) is None or canonical(payload.get(field)) is None:
            errors.append(f"production promotion {field} must have canonical independent trust")
        elif payload.get(field) != expected:
            errors.append(f"production promotion {field} must match operator trust")

    generated_at = payload.get("generated_at_unix")
    if (
        not isinstance(generated_at, int)
        or isinstance(generated_at, bool)
        or not 0 < generated_at <= MAX_TIMESTAMP
    ):
        errors.append(
            "production promotion provenance generated_at_unix must be positive and bounded"
        )
    elif generated_at > now_unix:
        errors.append("production promotion provenance must not be future-dated")
    elif now_unix - generated_at > max_provenance_age_secs:
        errors.append("production promotion provenance exceeds the reviewed age bound")

    expected_binding = _expected_provenance_binding(
        positive,
        negative,
        cosign_bundle_sha256,
    )
    for field, expected in expected_binding.items():
        if not promotion_runner.exact_json_equal(payload.get(field), expected):
            errors.append(
                f"production promotion provenance {field} must match the verified bundle"
            )

    certificate_identity = payload.get("provenance_certificate_identity")
    oidc_issuer = payload.get("provenance_oidc_issuer")
    if canonical_public_provenance_url(certificate_identity) is None:
        errors.append(
            "production promotion certificate identity must be a canonical public HTTPS URL"
        )
    if canonical_public_provenance_url(oidc_issuer) is None:
        errors.append(
            "production promotion OIDC issuer must be a canonical public HTTPS URL"
        )
    if (
        trusted_certificate_identity is None
        or certificate_identity != trusted_certificate_identity
    ):
        errors.append(
            "production promotion certificate identity must match operator trust"
        )
    if trusted_oidc_issuer is None or oidc_issuer != trusted_oidc_issuer:
        errors.append("production promotion OIDC issuer must match operator trust")

    expected_signer, signer_errors = _validate_operator_signer_tuple(
        service_id=trusted_service_id,
        administrator_id=trusted_administrator_id,
        key_revision=trusted_key_revision,
        policy_revision=trusted_policy_revision,
        policy_digest_sha256=trusted_policy_digest_sha256,
    )
    errors.extend(f"operator trust: {error}" for error in signer_errors)
    authentication = payload.get("authentication")
    authenticated = False
    if not isinstance(authentication, Mapping):
        errors.append("production promotion authentication must be an object")
    else:
        if set(authentication) != AUTHENTICATION_FIELDS:
            errors.append(
                "production promotion authentication fields must match the schema-closed contract"
            )
        if authentication.get("kind") != "external-ed25519":
            errors.append(
                "production promotion authentication.kind must be `external-ed25519`"
            )
        if authentication.get("algorithm") != "ed25519":
            errors.append(
                "production promotion authentication.algorithm must be `ed25519`"
            )
        signer_row = {
            "service_id": authentication.get("service_id"),
            "administrator_id": authentication.get("administrator_id"),
            "key_revision": authentication.get("key_revision"),
            "policy_revision": authentication.get("policy_revision"),
            "policy_digest_sha256": authentication.get("policy_digest_sha256"),
        }
        observed_signer = _validate_signer(
            signer_row,
            errors,
        )
        for field in expected_signer:
            if observed_signer.get(field) != expected_signer[field]:
                errors.append(
                    f"production promotion authentication {field} must match operator trust"
                )
        fingerprint = _canonical_nonzero_sha256(
            authentication.get("public_key_fingerprint_sha256")
        )
        if fingerprint is None:
            errors.append(
                "production promotion authentication public-key fingerprint must be non-zero SHA-256"
            )
        expected_fingerprint = (
            _sha256(trusted_public_key)
            if isinstance(trusted_public_key, bytes)
            and any(trusted_public_key)
            else None
        )
        if expected_fingerprint is None:
            errors.append(
                "production promotion requires an operator-trusted Ed25519 public key"
            )
        elif fingerprint != expected_fingerprint:
            errors.append(
                "production promotion authentication key must match operator trust"
            )
        signature = _canonical_signature(authentication.get("signature_hex"))
        if signature is None:
            errors.append(
                "production promotion authentication signature must be canonical Ed25519"
            )
        if (
            signature is not None
            and expected_fingerprint is not None
            and fingerprint == expected_fingerprint
            and not signer_errors
        ):
            try:
                signing_payload = promotion_provenance_signing_payload(payload)
            except ValueError:
                errors.append(
                    "production promotion provenance could not be encoded for authentication"
                )
            else:
                authenticated = verify_ed25519(
                    trusted_public_key,
                    signature,
                    signing_payload,
                )
                if not authenticated:
                    errors.append(
                        "production promotion provenance signature verification failed"
                    )
    if not authenticated and not any(
        "signature verification failed" in error for error in errors
    ):
        errors.append("production promotion provenance is not externally authenticated")
    return errors


def _cross_validate_positive_and_negative(
    positive: PositiveReplayEvidence,
    negative: NegativeArchiveEvidence,
) -> list[str]:
    errors: list[str] = []
    if positive.input_count != negative.baseline_input_count:
        errors.append(
            "positive replay and negative archive input counts must match"
        )
    if positive.input_set_sha256 != negative.baseline_input_set_sha256:
        errors.append(
            "positive replay and negative archive input-set digests must match"
        )
    expected_baseline = {
        "aggregate_summary_sha256": positive.output_sha256[
            "first_aggregate_sha256"
        ],
        "replay_summary_sha256": positive.output_sha256[
            "second_aggregate_sha256"
        ],
        "replay_manifest_sha256": positive.output_sha256[
            "replay_manifest_sha256"
        ],
    }
    for field, expected in expected_baseline.items():
        if negative.baseline_output_sha256.get(field) != expected:
            errors.append(
                f"negative archive {field} must match the verified positive replay"
            )
    return errors


def validate_inner_approval_chain() -> list[str]:
    """Keep production closed until every prerequisite's custody proof is independently verified."""

    # TODO: Migrate foundational, topology, resilience and lane-inventory authorities together.
    # Their aggregate/replay schemas carry signatures and digest summaries,
    # not independently verified purpose-owned signer custody and completed-operation proofs.
    # Replacing those labels, or signing their hashes with the outer signing key, cannot supply
    # the missing authority. Replace this gate only with verification of all four actual contracts.
    return [
        "production promotion remains blocked: foundational, topology, resilience and lane-inventory "
        "signer custody proofs are not integrated; complete and verify the inner approval chain"
    ]


def validate_bundle(args: argparse.Namespace) -> tuple[dict[str, Any], list[str]]:
    """Validate every conjunct and return one payload-free promotion summary."""

    errors: list[str] = []
    positive, positive_errors = load_positive_replay(
        args.first_aggregate,
        args.second_aggregate,
        args.replay_manifest,
    )
    errors.extend(f"positive replay: {error}" for error in positive_errors)
    negative, negative_errors = load_negative_archive(args.negative_archive_dir)
    errors.extend(f"negative archive: {error}" for error in negative_errors)
    cosign_bundle, cosign_errors = load_cosign_bundle(args.cosign_bundle)
    cosign_sha256 = cosign_bundle.sha256 if cosign_bundle is not None else None
    errors.extend(cosign_errors)

    trusted_key_hex = _canonical_nonzero_sha256(args.provenance_verification_public_key_hex)
    trusted_public_key = bytes.fromhex(trusted_key_hex) if trusted_key_hex else None
    if trusted_public_key is None:
        errors.append("--provenance-verification-public-key-hex must be non-zero canonical 32-byte hex")
    certificate_identity = (
        args.provenance_certificate_identity
        if canonical_public_provenance_url(args.provenance_certificate_identity)
        is not None
        else None
    )
    if certificate_identity is None:
        errors.append(
            "--provenance-certificate-identity must be a canonical public HTTPS URL"
        )
    oidc_issuer = (
        args.provenance_oidc_issuer
        if canonical_public_provenance_url(args.provenance_oidc_issuer) is not None
        else None
    )
    if oidc_issuer is None:
        errors.append(
            "--provenance-oidc-issuer must be a canonical public HTTPS URL"
        )

    provenance_payload: dict[str, Any] | None = None
    if args.promotion_provenance is None:
        errors.append(
            "production promotion requires externally authenticated provenance"
        )
    else:
        loaded_provenance = _load_json_bytes(
            args.promotion_provenance,
            MAX_PROMOTION_PROVENANCE_BYTES,
            label="production promotion provenance",
            errors=errors,
        )
        if loaded_provenance is not None:
            provenance_payload = loaded_provenance[0]

    if positive is not None and negative is not None:
        errors.extend(_cross_validate_positive_and_negative(positive, negative))
    if (
        positive is not None
        and negative is not None
        and cosign_sha256 is not None
        and provenance_payload is not None
    ):
        errors.extend(
            validate_promotion_provenance(
                provenance_payload,
                positive=positive,
                negative=negative,
                cosign_bundle_sha256=cosign_sha256,
                trusted_public_key=trusted_public_key,
                trusted_service_id=args.provenance_signer_service_id,
                trusted_administrator_id=(
                    args.provenance_signer_administrator_id
                ),
                trusted_key_revision=args.provenance_signer_key_revision,
                trusted_policy_revision=args.provenance_signer_policy_revision,
                trusted_policy_digest_sha256=(
                    args.provenance_signer_policy_digest_hex
                ),
                trusted_certificate_identity=certificate_identity,
                trusted_oidc_issuer=oidc_issuer,
                trusted_chain_id=args.provenance_chain_id,
                trusted_network_id_hex=args.provenance_network_id_hex,
                trusted_deployment_id=args.provenance_deployment_id,
                now_unix=args.now_unix,
                max_provenance_age_secs=args.max_provenance_age_secs,
            )
        )

    # The native adapter receives only the structurally valid, authenticated canonical subject.
    # It independently pins the executable, policy, trust, current observation and complete receipt.
    if not errors and provenance_payload is not None and trusted_public_key is not None:
        statement = promotion_provenance_signing_payload(provenance_payload)
        signature = bytes.fromhex(provenance_payload["authentication"]["signature_hex"])
        errors.extend(final_promotion_evidence.verify_final_promotion_receipt(
            args, statement, signature, trusted_public_key,
        ))
        if not errors and cosign_bundle is not None:
            unsigned_subject = _unsigned_promotion_body(provenance_payload)
            del unsigned_subject["cosign_bundle_sha256"]
            errors.extend(final_promotion_cosign.verify_final_promotion_cosign(
                args, promotion_cosign_subject_bytes(unsigned_subject), cosign_bundle.raw,
            ))
    errors.extend(validate_inner_approval_chain())
    qualified = not errors
    summary: dict[str, Any] = {
        "schema": PROMOTION_SUMMARY_SCHEMA,
        "status": "ready" if qualified else "blocked",
        "attestation_scope": PROMOTION_ATTESTATION_SCOPE,
        "externally_authenticated": qualified,
        "promotion_eligible": qualified,
        "baseline_input_count": positive.input_count if positive else 0,
        "baseline_input_set_sha256": (
            positive.input_set_sha256 if positive else None
        ),
        "positive_output_sha256": (
            dict(positive.output_sha256) if positive else None
        ),
        "negative_archive_manifest_sha256": (
            negative.manifest_sha256 if negative else None
        ),
        "negative_receipt_count": len(negative.receipts) if negative else 0,
        "negative_receipts": (
            [dict(row) for row in negative.receipts] if negative else []
        ),
        "aggregate_runner_sha256": negative.runner_sha256 if negative else None,
        "aggregate_checker_sha256": negative.checker_sha256 if negative else None,
        "aggregate_toolchain_sha256": (
            negative.toolchain_sha256 if negative else None
        ),
        "python_runtime": dict(negative.python_runtime) if negative else None,
        "cosign_bundle_sha256": cosign_sha256,
        "provenance_certificate_identity": (
            certificate_identity if qualified else None
        ),
        "provenance_oidc_issuer": oidc_issuer if qualified else None,
        "errors": errors,
    }
    assert set(summary) == PROMOTION_SUMMARY_FIELDS
    return summary, errors


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse read-only production-promotion verification arguments."""

    parser = EvidenceArgumentParser(
        description=(
            "Conjunctively verify the final SoraFS positive replay, fixed "
            "negative archive, and signer receipt/cosign provenance."
        ),
    )
    parser.add_argument("--first-aggregate", required=True, type=Path)
    parser.add_argument("--second-aggregate", required=True, type=Path)
    parser.add_argument("--replay-manifest", required=True, type=Path)
    parser.add_argument("--negative-archive-dir", required=True, type=Path)
    parser.add_argument(
        "--promotion-provenance",
        type=Path,
        help="Externally signed schema-closed final promotion provenance receipt.",
    )
    parser.add_argument(
        "--cosign-bundle",
        type=Path,
        help="Exact non-empty cosign JSON bundle bound by the signed receipt.",
    )
    parser.add_argument(
        "--provenance-verification-public-key-hex",
        help="Operator-trusted non-zero raw Ed25519 provenance verification key.",
    )
    parser.add_argument(
        "--provenance-signer-service-id",
        help="Operator-trusted signer service identity.",
    )
    parser.add_argument(
        "--provenance-signer-administrator-id",
        help="Independently administered promotion signer identity.",
    )
    parser.add_argument(
        "--provenance-signer-key-revision",
        type=positive_int_arg,
    )
    parser.add_argument(
        "--provenance-signer-policy-revision",
        type=positive_int_arg,
    )
    parser.add_argument("--provenance-signer-policy-digest-hex")
    parser.add_argument(
        "--provenance-certificate-identity",
        help="Operator-trusted public HTTPS cosign certificate identity.",
    )
    parser.add_argument(
        "--provenance-oidc-issuer",
        help="Operator-trusted public HTTPS OIDC issuer.",
    )
    for option in (
        "--provenance-receipt-verifier", "--provenance-signer-policy",
        "--provenance-custody-trust", "--provenance-completed-operation-state",
        "--provenance-operation-receipt",
        "--provenance-cosign-verifier", "--provenance-cosign-trusted-root",
    ):
        parser.add_argument(option, required=True, type=Path)
    for option in (
        "--provenance-receipt-verifier-sha256", "--provenance-signer-policy-sha256",
        "--provenance-custody-trust-sha256", "--provenance-chain-id",
        "--provenance-network-id-hex", "--provenance-deployment-id",
        "--provenance-cosign-verifier-sha256", "--provenance-cosign-trusted-root-sha256",
    ):
        parser.add_argument(option, required=True)
    parser.add_argument("--now-unix", required=True, type=positive_int_arg)
    parser.add_argument(
        "--max-provenance-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_PROVENANCE_AGE_SECS,
    )
    raw_args = list(sys.argv[1:] if argv is None else argv)
    try:
        expanded = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error
    return parser.parse_args(expanded)


def main(argv: Sequence[str] | None = None) -> int:
    """Run the read-only final promotion checker."""

    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1
    if not 0 < args.now_unix <= MAX_TIMESTAMP:
        emit_checker_error_lines(
            ["--now-unix must be a positive bounded integer timestamp"]
        )
        return 2
    if not 0 <= args.max_provenance_age_secs <= MAX_TIMESTAMP:
        emit_checker_error_lines(
            ["--max-provenance-age-secs must be a non-negative bounded integer"]
        )
        return 2
    try:
        summary, errors = validate_bundle(args)
        sys.stdout.write(render_checker_summary(summary))
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        emit_checker_exception(error)
        return 2
    if errors:
        emit_checker_error_lines(errors)
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover - exercised through main tests
    raise SystemExit(main())
