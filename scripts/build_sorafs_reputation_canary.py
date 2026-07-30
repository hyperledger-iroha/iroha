#!/usr/bin/env python3
"""Build payload-free SoraFS reputation rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_reputation_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_INGEST_LAG_SECS,
    DEFAULT_MAX_SNAPSHOT_AGE_SECS,
    FORBIDDEN_PROVIDER_ID_MARKERS,
    FORBIDDEN_TRANSPORT_EVENT_LABEL_MARKERS,
    KIND_BY_NAME,
    LoadedEvidence,
    MAX_EVIDENCE_BYTES,
    PROVIDER_ID_ERROR,
    PROVIDER_ID_PATTERN,
    REQUIRED_METRICS,
    SSE_EVENT_LABEL_ERROR,
    SSE_EVENT_LABEL_PATTERN,
    SNAPSHOT_ANCHOR_KINDS,
    SNAPSHOT_BOUND_KINDS,
    WEBSOCKET_EVENT_LABEL_ERROR,
    WEBSOCKET_EVENT_LABEL_PATTERN,
    validate_common_rollout_context,
    validate_evidence_set,
    validate_provider,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    fsync_checker_output_parent,
    write_all_checker_summary_bytes,
    validate_checker_output_parent,
)
from sorafs_path_identity import (  # noqa: E402
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    path_diagnostic_label,
    resolve_path_identity,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_validation import (  # noqa: E402
    forbidden_non_production_markers,
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)
SOURCE_BOUND_KINDS = frozenset({"latest", "provider", "events", "verify"})
HEX32_LEN = 32
HEX64_LEN = 64
DEFAULT_PROOF_SIBLING_HEX = "33" * 32
MIN_REPUTATION_SCORE_BPS = 500
MAX_REPUTATION_SCORE_BPS = 9_900
MAX_REPUTATION_DEGRADATION_FLAGS = 5
REPUTATION_DEGRADATION_FLAGS = (
    "reserve_warning",
    "reserve_grace",
    "reserve_delinquent",
    "reserve_default",
    "proof_success_below90",
    "proof_success_below80",
    "active_dispute",
    "slashing_event",
    "low_score",
)
SOURCE_BOUND_MANUAL_OPTIONS = frozenset(
    {
        "--deployment-id",
        "--environment",
        "--generated-at-unix",
        "--snapshot-id-hex",
        "--merkle-root-hex",
        "--weights-digest-hex",
        "--provider-id",
        "--provider-count",
        "--provider-name",
        "--provider-score-bps",
        "--leaf-index",
        "--sibling-hex",
        "--since",
        "--next-since",
        "--event-count",
        "--snapshot-age-seconds",
        "--ingest-lag-seconds",
        "--metric",
        "--sse-event-count",
        "--sse-event",
        "--websocket-event-count",
        "--websocket-event",
    }
)
SOURCE_BOUND_OPTIONS = frozenset(
    {
        "--source-cli-json",
        "--publish-evidence",
        "--latest-evidence",
        "--provider-evidence",
        "--expected-provider-id",
        "--expected-since",
        "--expected-limit",
        "--expected-snapshot-path",
        "--expected-proof-path",
    }
)
REPEATABLE_OPTIONS = frozenset(
    {
        "--provider-name",
        "--sibling-hex",
        "--metric",
        "--sse-event",
        "--websocket-event",
    }
)
LATEST_SOURCE_FIELDS = frozenset(
    {
        "snapshot_id_hex",
        "generated_at_unix",
        "previous_snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "returned_provider_count",
        "limit",
        "truncated_providers",
        "alpha_bps",
        "current_score_weight_bps",
        "weights",
        "providers",
    }
)
PROVIDER_SOURCE_FIELDS = frozenset(
    {
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider",
        "proof",
    }
)
EVENTS_SOURCE_FIELDS = frozenset({"since", "limit", "count", "next_since", "events"})
EVENT_SOURCE_FIELDS = frozenset(
    {
        "version",
        "sequence",
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider_count",
        "previous_snapshot_id_hex",
    }
)
VERIFY_SOURCE_FIELDS = frozenset(
    {
        "snapshot_path",
        "snapshot_id_hex",
        "generated_at_unix",
        "provider_count",
        "merkle_root_hex",
        "alpha_bps",
        "current_score_weight_bps",
        "valid",
        "provider_id",
        "provider_score_bps",
        "proof_path",
        "proof_leaf_index",
        "proof_sibling_count",
        "proof_verified",
    }
)
VERIFY_SOURCE_OPTIONAL_FIELDS = frozenset({"previous_snapshot_id_hex"})
RAW_PROVIDER_FIELDS = frozenset(
    {
        "provider_id",
        "score_bps",
        "degradation_flags",
        "raw_metrics",
        "raw_metrics_hash_hex",
    }
)
RAW_PROOF_FIELDS = frozenset(
    {"provider_id", "leaf_index", "leaf_count", "siblings_hex"}
)
RAW_METRICS_FIELDS = frozenset(
    {
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_health_bps",
        "dispute_rate_bps",
        "token_violation_rate_bps",
        "repair_breach_rate_bps",
    }
)
RAW_WEIGHTS_FIELDS = frozenset(
    {
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_bps",
        "dispute_bps",
        "token_violation_bps",
        "repair_breach_bps",
    }
)
CANONICAL_ANCHOR_FIELDS = frozenset(
    {
        "schema",
        "generated_at_unix",
        "deployment_id",
        "environment",
        "deployment_context_reviewed",
        "status",
        "snapshot_id_hex",
        "merkle_root_hex",
        "weights_digest_hex",
        "provider_count",
        "providers",
    }
)
CANONICAL_PROVIDER_FIELDS = frozenset(
    {
        "schema",
        "generated_at_unix",
        "deployment_id",
        "environment",
        "deployment_context_reviewed",
        "snapshot_id_hex",
        "merkle_root_hex",
        "provider",
        "proof",
    }
)


@dataclass(frozen=True)
class SourceArtifact:
    """One bounded JSON source artifact and its digest."""

    path: Path
    payload: dict[str, Any]
    digest: str


@dataclass(frozen=True)
class SourceBoundInputs:
    """Source artifacts used to build one canonical live-read canary."""

    source: SourceArtifact
    publish: SourceArtifact
    latest: SourceArtifact | None = None
    provider: SourceArtifact | None = None


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing a canary artifact."""

    if not isinstance(path, Path):
        errors.append(f"--out `{path_diagnostic_label(path)}` must be a path")
        return
    try:
        if path.is_symlink():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a symlink")
            return
        if path.exists() and path.is_dir():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a directory")
            return
    except (OSError, RuntimeError) as error:
        del error
        errors.append(f"--out `{path_diagnostic_label(path)}` cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label="--out")


def require_exact_fields(
    payload: Any,
    expected: frozenset[str],
    *,
    label: str,
    errors: list[str],
    optional: frozenset[str] = frozenset(),
) -> bool:
    """Require one schema-closed object without reflecting payload values."""

    if not isinstance(payload, dict):
        errors.append(f"{label} must be an object")
        return False
    fields = set(payload)
    if not expected <= fields or not fields <= expected | optional:
        errors.append(f"{label} fields must match the schema-closed profile")
        return False
    return True


def require_source_int(
    payload: dict[str, Any],
    field: str,
    *,
    label: str,
    errors: list[str],
    minimum: int = 0,
    maximum: int | None = None,
) -> int | None:
    """Return one bounded source integer or record a payload-free diagnostic."""

    value = payload.get(field)
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < minimum
        or (maximum is not None and value > maximum)
    ):
        errors.append(f"{label}.{field} must be a canonical bounded integer")
        return None
    return value


def require_source_bool(
    payload: dict[str, Any],
    field: str,
    *,
    label: str,
    errors: list[str],
) -> bool | None:
    """Return one exact source boolean."""

    value = payload.get(field)
    if not isinstance(value, bool):
        errors.append(f"{label}.{field} must be a boolean")
        return None
    return value


def require_source_hex(
    payload: dict[str, Any],
    field: str,
    length: int,
    *,
    label: str,
    errors: list[str],
    optional: bool = False,
) -> str:
    """Return an exact lowercase hexadecimal source field."""

    value = payload.get(field)
    if optional and value is None:
        return ""
    if (
        not isinstance(value, str)
        or len(value) != length
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(
            f"{label}.{field} must be exact lowercase hex length {length}"
        )
        return ""
    return value


def require_source_string(
    payload: dict[str, Any],
    field: str,
    *,
    label: str,
    errors: list[str],
) -> str:
    """Return one canonical source string."""

    value = payload.get(field)
    if not diagnostic_text_is_canonical(value):
        errors.append(f"{label}.{field} must be a non-empty canonical string")
        return ""
    return value


def require_equal(
    actual: Any,
    expected: Any,
    *,
    diagnostic: str,
    errors: list[str],
) -> None:
    """Record an exact binding mismatch without exposing either value."""

    if actual != expected:
        errors.append(diagnostic)


def load_source_artifact(
    path: Path | None,
    *,
    option: str,
    errors: list[str],
) -> SourceArtifact | None:
    """Load one bounded JSON source without following symlinks."""

    if not isinstance(path, Path):
        errors.append(f"{option} is required")
        return None
    loaded = load_evidence_json_with_sha256_or_record_error(
        path,
        MAX_EVIDENCE_BYTES,
        errors,
    )
    if loaded is None:
        return None
    payload, digest = loaded
    return SourceArtifact(path, payload, digest)


def validate_source_output_identity(
    args: argparse.Namespace,
    paths: Sequence[Path],
    errors: list[str],
) -> None:
    """Reject output/input aliasing and duplicate source identities."""

    identities: dict[Path, str] = {}
    for label, path in (("--out", args.out), *(
        (f"source input {index}", path) for index, path in enumerate(paths)
    )):
        identity_errors: list[str] = []
        identity = resolve_path_identity(path, identity_errors, label=label)
        errors.extend(identity_errors)
        if identity is None:
            continue
        if identity in identities:
            errors.append("source-bound input and output paths must be distinct")
            continue
        identities[identity] = label


def expanded_option_names(expanded_args: Sequence[str]) -> frozenset[str]:
    """Return the exact option names present after response-file expansion."""

    return frozenset(
        argument.split("=", 1)[0]
        for argument in expanded_args
        if argument.startswith("-")
    )


def reject_duplicate_scalar_options(expanded_args: Sequence[str]) -> None:
    """Reject ambiguous repeated scalar options in strict source-bound mode."""

    counts: dict[str, int] = {}
    for argument in expanded_args:
        if not argument.startswith("-"):
            continue
        option = argument.split("=", 1)[0]
        counts[option] = counts.get(option, 0) + 1
    if any(count > 1 and option not in REPEATABLE_OPTIONS for option, count in counts.items()):
        raise ValueError("scalar canary-builder options must not be repeated")


def parse_expanded_args(
    parser: EvidenceArgumentParser,
    expanded_args: Sequence[str],
) -> argparse.Namespace:
    """Parse one already bounded and expanded builder argument vector."""

    return parser.parse_args(expanded_args)


def validate_source_mode_options(args: argparse.Namespace, errors: list[str]) -> None:
    """Require the exact source inputs for the requested live-read kind."""

    if args.kind not in SOURCE_BOUND_KINDS:
        errors.append("--source-cli-json supports only latest, provider, events, or verify")
    if args.provided_options & SOURCE_BOUND_MANUAL_OPTIONS:
        errors.append(
            "source-bound mode must not include unbound operator-fact options"
        )
    required_by_kind = {
        "latest": frozenset({"--source-cli-json", "--publish-evidence"}),
        "provider": frozenset(
            {
                "--source-cli-json",
                "--publish-evidence",
                "--latest-evidence",
                "--expected-provider-id",
            }
        ),
        "events": frozenset(
            {
                "--source-cli-json",
                "--publish-evidence",
                "--latest-evidence",
                "--expected-since",
                "--expected-limit",
            }
        ),
        "verify": frozenset(
            {
                "--source-cli-json",
                "--publish-evidence",
                "--latest-evidence",
                "--provider-evidence",
                "--expected-provider-id",
                "--expected-snapshot-path",
                "--expected-proof-path",
            }
        ),
    }
    allowed = required_by_kind.get(args.kind, frozenset())
    present = args.provided_options & SOURCE_BOUND_OPTIONS
    if present != allowed:
        errors.append(
            "source-bound options must exactly match the requested evidence kind"
        )
    if args.expected_provider_id is not None:
        validate_provider_label_arg(
            args.expected_provider_id,
            option="--expected-provider-id",
            errors=errors,
        )


def load_source_bound_inputs(
    args: argparse.Namespace,
    errors: list[str],
) -> SourceBoundInputs | None:
    """Load every source-bound artifact required by one builder invocation."""

    source = load_source_artifact(
        args.source_cli_json,
        option="--source-cli-json",
        errors=errors,
    )
    publish = load_source_artifact(
        args.publish_evidence,
        option="--publish-evidence",
        errors=errors,
    )
    latest = (
        load_source_artifact(
            args.latest_evidence,
            option="--latest-evidence",
            errors=errors,
        )
        if args.latest_evidence is not None
        else None
    )
    provider = (
        load_source_artifact(
            args.provider_evidence,
            option="--provider-evidence",
            errors=errors,
        )
        if args.provider_evidence is not None
        else None
    )
    input_paths = [
        path
        for path in (
            args.source_cli_json,
            args.publish_evidence,
            args.latest_evidence,
            args.provider_evidence,
        )
        if isinstance(path, Path)
    ]
    validate_source_output_identity(args, input_paths, errors)
    if source is None or publish is None:
        return None
    return SourceBoundInputs(source, publish, latest, provider)


def validate_hex(value: str | None, *, option: str, length: int, errors: list[str]) -> None:
    """Validate an exact lowercase hex string."""

    if (
        not isinstance(value, str)
        or len(value) != length
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase hex length {length}")


def validate_canonical_string(value: str | None, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control/format text."""

    if not diagnostic_text_is_canonical(value):
        errors.append(f"{label} must be a non-empty canonical string")


def validate_provider_label_arg(
    value: str | None,
    *,
    option: str,
    errors: list[str],
) -> None:
    """Require a reviewed lowercase production provider label."""

    validate_canonical_string(value, label=option, errors=errors)
    if not isinstance(value, str):
        return
    if PROVIDER_ID_PATTERN.fullmatch(value) is None:
        errors.append(PROVIDER_ID_ERROR.replace("provider_id", option))
        return
    forbidden = forbidden_non_production_markers(value, FORBIDDEN_PROVIDER_ID_MARKERS)
    if forbidden:
        errors.append(f"{option} must not contain non-production markers {forbidden}")


def validate_provider_id_arg(value: str | None, *, errors: list[str]) -> None:
    """Require a reviewed lowercase production provider identifier."""

    validate_provider_label_arg(value, option="--provider-id", errors=errors)


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into exact strings."""

    items: list[str] = []
    for value in values:
        items.extend(value.split(","))
    return items


def validate_name_set(
    values: Sequence[str],
    *,
    allowed: Sequence[str],
    option: str,
    errors: list[str],
) -> list[str]:
    """Return allowed-order values, requiring complete known non-duplicate coverage."""

    allowed_set = frozenset(allowed)
    value_set = frozenset(values)
    if len(value_set) != len(values):
        errors.append(f"{option} must not contain duplicates")
    if any(name not in allowed_set for name in value_set):
        errors.append(f"{option} contains an unknown value")
    missing = [name for name in allowed if name not in value_set]
    if missing:
        errors.append(f"{option} must include every required value")
    return [name for name in allowed if name in value_set]


def validate_provider_inventory(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed provider names and bind them to provider_count."""

    provider_names = split_csv_values(args.provider_name)
    if not provider_names:
        errors.append("--provider-name is required")
    for name in provider_names:
        validate_provider_label_arg(name, option="--provider-name", errors=errors)
    if len(set(provider_names)) != len(provider_names):
        errors.append("--provider-name must not contain duplicates")
    if args.provider_count != len(set(provider_names)):
        errors.append(
            "--provider-count must match the number of unique --provider-name values"
        )
    args.providers = provider_names


def validate_reviewed_inventory(
    values: Sequence[str],
    *,
    expected_count: int,
    option: str,
    count_option: str,
    pattern: re.Pattern[str] | None = None,
    label_error: str | None = None,
    errors: list[str],
) -> list[str]:
    """Validate reviewed unique labels and bind them to a CLI count."""

    items = split_csv_values(values)
    if not items:
        errors.append(f"{option} is required")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=option, errors=errors)
        if pattern is not None and isinstance(item, str):
            if pattern.fullmatch(item) is None:
                errors.append(label_error or f"{option} uses an invalid label")
            tokens = frozenset(
                token for token in re.split(r"[^a-z0-9]+", item) if token
            )
            forbidden = forbidden_non_production_markers(tokens, FORBIDDEN_TRANSPORT_EVENT_LABEL_MARKERS)
            if forbidden:
                errors.append(
                    f"{option}[{index}] must not contain non-production "
                    f"markers {forbidden}"
                )
    if len(set(items)) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if expected_count != len(set(items)):
        errors.append(f"{option} unique values must match {count_option}")
    return items


def validate_raw_metrics(
    payload: Any,
    *,
    label: str,
    errors: list[str],
) -> None:
    """Validate the exact Torii raw-metrics JSON profile."""

    if not require_exact_fields(
        payload,
        RAW_METRICS_FIELDS,
        label=label,
        errors=errors,
    ):
        return
    assert isinstance(payload, dict)
    version = require_source_int(
        payload,
        "version",
        label=label,
        errors=errors,
        minimum=1,
        maximum=1,
    )
    del version
    for field in sorted(RAW_METRICS_FIELDS - {"version"}):
        require_source_int(
            payload,
            field,
            label=label,
            errors=errors,
            maximum=10_000,
        )


def validate_raw_weights(
    payload: Any,
    *,
    label: str,
    errors: list[str],
) -> None:
    """Validate the exact Torii reputation-weights JSON profile."""

    if not require_exact_fields(
        payload,
        RAW_WEIGHTS_FIELDS,
        label=label,
        errors=errors,
    ):
        return
    assert isinstance(payload, dict)
    require_source_int(
        payload,
        "version",
        label=label,
        errors=errors,
        minimum=1,
        maximum=1,
    )
    values = [
        require_source_int(
            payload,
            field,
            label=label,
            errors=errors,
            maximum=10_000,
        )
        for field in sorted(RAW_WEIGHTS_FIELDS - {"version"})
    ]
    if all(value is not None for value in values) and sum(values) != 10_000:
        errors.append(f"{label} basis-point weights must sum to 10000")


def validate_raw_provider_record(
    payload: Any,
    *,
    label: str,
    errors: list[str],
) -> dict[str, Any] | None:
    """Validate and reduce one exact Torii provider record."""

    if not require_exact_fields(
        payload,
        RAW_PROVIDER_FIELDS,
        label=label,
        errors=errors,
    ):
        return None
    assert isinstance(payload, dict)
    provider_id = require_source_string(
        payload,
        "provider_id",
        label=label,
        errors=errors,
    )
    if provider_id:
        validate_provider_label_arg(
            provider_id,
            option=f"{label}.provider_id",
            errors=errors,
        )
    score_bps = require_source_int(
        payload,
        "score_bps",
        label=label,
        errors=errors,
        minimum=MIN_REPUTATION_SCORE_BPS,
        maximum=MAX_REPUTATION_SCORE_BPS,
    )
    degradation_flags = payload.get("degradation_flags")
    if not isinstance(degradation_flags, list):
        errors.append(f"{label}.degradation_flags must be an array")
    else:
        if len(degradation_flags) > MAX_REPUTATION_DEGRADATION_FLAGS:
            errors.append(
                f"{label}.degradation_flags must contain at most "
                f"{MAX_REPUTATION_DEGRADATION_FLAGS} entries"
            )
        flag_indexes: list[int] = []
        for flag in degradation_flags:
            if (
                not isinstance(flag, dict)
                or set(flag) != {"flag", "value"}
                or flag.get("value") is not None
                or flag.get("flag") not in REPUTATION_DEGRADATION_FLAGS
            ):
                errors.append(
                    f"{label}.degradation_flags must contain exact V1 flag objects"
                )
                break
            flag_indexes.append(
                REPUTATION_DEGRADATION_FLAGS.index(flag["flag"])
            )
        if any(
            previous >= current
            for previous, current in zip(flag_indexes, flag_indexes[1:])
        ):
            errors.append(
                f"{label}.degradation_flags must be canonically sorted and unique"
            )
    validate_raw_metrics(
        payload.get("raw_metrics"),
        label=f"{label}.raw_metrics",
        errors=errors,
    )
    require_source_hex(
        payload,
        "raw_metrics_hash_hex",
        HEX64_LEN,
        label=label,
        errors=errors,
    )
    if not provider_id or score_bps is None:
        return None
    return {"provider_id": provider_id, "score_bps": score_bps}


def validate_raw_proof(
    payload: Any,
    *,
    label: str,
    provider_count: int,
    errors: list[str],
) -> dict[str, Any] | None:
    """Validate and reduce one exact Torii Merkle-proof record."""

    if not require_exact_fields(
        payload,
        RAW_PROOF_FIELDS,
        label=label,
        errors=errors,
    ):
        return None
    assert isinstance(payload, dict)
    provider_id = require_source_string(
        payload,
        "provider_id",
        label=label,
        errors=errors,
    )
    if provider_id:
        validate_provider_label_arg(
            provider_id,
            option=f"{label}.provider_id",
            errors=errors,
        )
    leaf_index = require_source_int(
        payload,
        "leaf_index",
        label=label,
        errors=errors,
    )
    leaf_count = require_source_int(
        payload,
        "leaf_count",
        label=label,
        errors=errors,
        minimum=1,
    )
    siblings = payload.get("siblings_hex")
    sibling_values: list[str] = []
    if not isinstance(siblings, list):
        errors.append(f"{label}.siblings_hex must be an array")
    else:
        for sibling in siblings:
            if (
                not isinstance(sibling, str)
                or len(sibling) != HEX64_LEN
                or any(character not in "0123456789abcdef" for character in sibling)
            ):
                errors.append(
                    f"{label}.siblings_hex must contain exact lowercase hex length {HEX64_LEN}"
                )
                break
            sibling_values.append(sibling)
        if len(set(sibling_values)) != len(sibling_values):
            errors.append(f"{label}.siblings_hex must not contain duplicates")
    if leaf_count is not None:
        require_equal(
            leaf_count,
            provider_count,
            diagnostic=f"{label}.leaf_count must match the latest provider_count",
            errors=errors,
        )
        expected_siblings = (leaf_count - 1).bit_length()
        require_equal(
            len(sibling_values),
            expected_siblings,
            diagnostic=f"{label}.siblings_hex length must match leaf_count",
            errors=errors,
        )
    if leaf_index is not None and leaf_count is not None and leaf_index >= leaf_count:
        errors.append(f"{label}.leaf_index must be below leaf_count")
    if not provider_id or leaf_index is None or leaf_count is None:
        return None
    return {
        "provider_id": provider_id,
        "leaf_index": leaf_index,
        "leaf_count": leaf_count,
        "siblings_hex": sibling_values,
    }


def canonical_provider_names(payload: dict[str, Any]) -> tuple[str, ...]:
    """Return canonical provider inventory names from a validated anchor."""

    providers = payload.get("providers")
    if not isinstance(providers, list):
        return ()
    names: list[str] = []
    for provider in providers:
        if not isinstance(provider, dict) or set(provider) != {"name"}:
            return ()
        name = provider.get("name")
        if not isinstance(name, str):
            return ()
        names.append(name)
    return tuple(names)


def validate_canonical_anchor(
    artifact: SourceArtifact,
    *,
    kind: str,
    now_unix: int,
    errors: list[str],
) -> None:
    """Validate one exact publish/latest canary through the release checker."""

    payload = artifact.payload
    require_exact_fields(
        payload,
        CANONICAL_ANCHOR_FIELDS,
        label=f"--{kind}-evidence",
        errors=errors,
    )
    if payload.get("schema") != KIND_BY_NAME[kind].schema:
        errors.append(f"--{kind}-evidence schema must match {kind}")
    summary = validate_evidence_set(
        [LoadedEvidence(kind, artifact.path, payload, artifact.digest)],
        required_kinds=(kind,),
        required_providers=(),
        now_unix=now_unix,
        max_snapshot_age_secs=DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        max_ingest_lag_secs=DEFAULT_MAX_INGEST_LAG_SECS,
    )
    if summary.get("status") != "ready":
        errors.append(f"--{kind}-evidence must satisfy the canonical checker profile")


def validate_canonical_provider_artifact(
    artifact: SourceArtifact,
    *,
    errors: list[str],
) -> None:
    """Validate one exact canonical provider proof without requiring an anchor."""

    payload = artifact.payload
    require_exact_fields(
        payload,
        CANONICAL_PROVIDER_FIELDS,
        label="--provider-evidence",
        errors=errors,
    )
    if payload.get("schema") != KIND_BY_NAME["provider"].schema:
        errors.append("--provider-evidence schema must match provider")
    validate_common_rollout_context(payload, errors)
    validate_provider(
        LoadedEvidence("provider", artifact.path, payload, artifact.digest),
        errors,
    )


def require_anchor_agreement(
    publish: dict[str, Any],
    latest: dict[str, Any],
    errors: list[str],
) -> None:
    """Require latest evidence to match the reviewed publication anchor exactly."""

    for field in (
        "generated_at_unix",
        "deployment_id",
        "environment",
        "deployment_context_reviewed",
        "snapshot_id_hex",
        "merkle_root_hex",
        "weights_digest_hex",
        "provider_count",
        "providers",
    ):
        require_equal(
            latest.get(field),
            publish.get(field),
            diagnostic=f"latest.{field} must match publish.{field}",
            errors=errors,
        )


def source_common_payload(publish: dict[str, Any], kind: str) -> dict[str, Any]:
    """Return canonical common fields copied from a validated publish anchor."""

    return {
        "schema": KIND_BY_NAME[kind].schema,
        "generated_at_unix": publish["generated_at_unix"],
        "deployment_id": publish["deployment_id"],
        "environment": publish["environment"],
        "deployment_context_reviewed": True,
    }


def source_snapshot_fields(publish: dict[str, Any]) -> dict[str, Any]:
    """Return the exact snapshot binding copied from reviewed publication."""

    return {
        "snapshot_id_hex": publish["snapshot_id_hex"],
        "merkle_root_hex": publish["merkle_root_hex"],
    }


def build_source_latest(
    inputs: SourceBoundInputs,
    *,
    now_unix: int,
    errors: list[str],
) -> dict[str, Any] | None:
    """Build latest evidence from an exact authenticated CLI response."""

    publish = inputs.publish.payload
    source = inputs.source.payload
    validate_canonical_anchor(
        inputs.publish,
        kind="publish",
        now_unix=now_unix,
        errors=errors,
    )
    if not require_exact_fields(
        source,
        LATEST_SOURCE_FIELDS,
        label="latest CLI source",
        errors=errors,
    ):
        return None
    snapshot_id = require_source_hex(
        source,
        "snapshot_id_hex",
        HEX32_LEN,
        label="latest CLI source",
        errors=errors,
    )
    merkle_root = require_source_hex(
        source,
        "merkle_root_hex",
        HEX64_LEN,
        label="latest CLI source",
        errors=errors,
    )
    require_source_hex(
        source,
        "previous_snapshot_id_hex",
        HEX32_LEN,
        label="latest CLI source",
        errors=errors,
        optional=True,
    )
    generated_at = require_source_int(
        source,
        "generated_at_unix",
        label="latest CLI source",
        errors=errors,
        minimum=1,
    )
    provider_count = require_source_int(
        source,
        "provider_count",
        label="latest CLI source",
        errors=errors,
        minimum=1,
    )
    returned_count = require_source_int(
        source,
        "returned_provider_count",
        label="latest CLI source",
        errors=errors,
        minimum=1,
    )
    limit = require_source_int(
        source,
        "limit",
        label="latest CLI source",
        errors=errors,
        minimum=1,
    )
    truncated = require_source_bool(
        source,
        "truncated_providers",
        label="latest CLI source",
        errors=errors,
    )
    require_source_int(
        source,
        "alpha_bps",
        label="latest CLI source",
        errors=errors,
        maximum=10_000,
    )
    require_source_int(
        source,
        "current_score_weight_bps",
        label="latest CLI source",
        errors=errors,
        maximum=10_000,
    )
    validate_raw_weights(
        source.get("weights"),
        label="latest CLI source.weights",
        errors=errors,
    )
    raw_providers = source.get("providers")
    providers: list[dict[str, str]] = []
    if not isinstance(raw_providers, list):
        errors.append("latest CLI source.providers must be an array")
    else:
        for index, raw_provider in enumerate(raw_providers):
            provider = validate_raw_provider_record(
                raw_provider,
                label=f"latest CLI source.providers[{index}]",
                errors=errors,
            )
            if provider is not None:
                providers.append({"name": provider["provider_id"]})
    provider_names = [provider["name"] for provider in providers]
    if provider_names != sorted(provider_names) or len(set(provider_names)) != len(
        provider_names
    ):
        errors.append(
            "latest CLI source.providers must be sorted and unique by provider_id"
        )
    if provider_count is not None:
        require_equal(
            len(providers),
            provider_count,
            diagnostic="latest CLI source provider_count must match complete provider inventory",
            errors=errors,
        )
        require_equal(
            provider_count,
            publish.get("provider_count"),
            diagnostic="latest CLI source provider_count must match publish.provider_count",
            errors=errors,
        )
    if returned_count is not None and provider_count is not None:
        require_equal(
            returned_count,
            provider_count,
            diagnostic="latest CLI source must contain the complete provider inventory",
            errors=errors,
        )
    if limit is not None and provider_count is not None and limit < provider_count:
        errors.append("latest CLI source limit must cover provider_count")
    if truncated is not False:
        errors.append("latest CLI source must not be truncated")
    require_equal(
        snapshot_id,
        publish.get("snapshot_id_hex"),
        diagnostic="latest CLI source snapshot_id_hex must match publish",
        errors=errors,
    )
    require_equal(
        merkle_root,
        publish.get("merkle_root_hex"),
        diagnostic="latest CLI source merkle_root_hex must match publish",
        errors=errors,
    )
    require_equal(
        generated_at,
        publish.get("generated_at_unix"),
        diagnostic="latest CLI source generated_at_unix must match publish",
        errors=errors,
    )
    require_equal(
        providers,
        publish.get("providers"),
        diagnostic="latest CLI source provider inventory must match publish",
        errors=errors,
    )
    if errors:
        return None
    payload = source_common_payload(publish, "latest")
    payload.update(
        {
            "status": "accepted",
            **source_snapshot_fields(publish),
            "weights_digest_hex": publish["weights_digest_hex"],
            "provider_count": publish["provider_count"],
            "providers": providers,
        }
    )
    return payload


def validated_publish_and_latest(
    inputs: SourceBoundInputs,
    *,
    now_unix: int,
    errors: list[str],
) -> tuple[dict[str, Any], dict[str, Any]] | None:
    """Validate and return matching canonical publish/latest anchors."""

    if inputs.latest is None:
        errors.append("--latest-evidence is required")
        return None
    validate_canonical_anchor(
        inputs.publish,
        kind="publish",
        now_unix=now_unix,
        errors=errors,
    )
    validate_canonical_anchor(
        inputs.latest,
        kind="latest",
        now_unix=now_unix,
        errors=errors,
    )
    publish = inputs.publish.payload
    latest = inputs.latest.payload
    require_anchor_agreement(publish, latest, errors)
    return publish, latest


def build_source_provider(
    args: argparse.Namespace,
    inputs: SourceBoundInputs,
    *,
    errors: list[str],
) -> dict[str, Any] | None:
    """Build provider evidence from an exact authenticated CLI response."""

    anchors = validated_publish_and_latest(
        inputs,
        now_unix=args.now_unix,
        errors=errors,
    )
    source = inputs.source.payload
    if not require_exact_fields(
        source,
        PROVIDER_SOURCE_FIELDS,
        label="provider CLI source",
        errors=errors,
    ):
        return None
    if anchors is None:
        return None
    publish, latest = anchors
    snapshot_id = require_source_hex(
        source,
        "snapshot_id_hex",
        HEX32_LEN,
        label="provider CLI source",
        errors=errors,
    )
    merkle_root = require_source_hex(
        source,
        "merkle_root_hex",
        HEX64_LEN,
        label="provider CLI source",
        errors=errors,
    )
    generated_at = require_source_int(
        source,
        "generated_at_unix",
        label="provider CLI source",
        errors=errors,
        minimum=1,
    )
    provider = validate_raw_provider_record(
        source.get("provider"),
        label="provider CLI source.provider",
        errors=errors,
    )
    provider_count = latest.get("provider_count")
    proof = (
        validate_raw_proof(
            source.get("proof"),
            label="provider CLI source.proof",
            provider_count=provider_count,
            errors=errors,
        )
        if isinstance(provider_count, int) and not isinstance(provider_count, bool)
        else None
    )
    require_equal(
        snapshot_id,
        publish.get("snapshot_id_hex"),
        diagnostic="provider CLI source snapshot_id_hex must match publish/latest",
        errors=errors,
    )
    require_equal(
        merkle_root,
        publish.get("merkle_root_hex"),
        diagnostic="provider CLI source merkle_root_hex must match publish/latest",
        errors=errors,
    )
    require_equal(
        generated_at,
        publish.get("generated_at_unix"),
        diagnostic="provider CLI source generated_at_unix must match publish/latest",
        errors=errors,
    )
    if provider is not None:
        require_equal(
            provider["provider_id"],
            args.expected_provider_id,
            diagnostic="provider CLI source provider_id must match --expected-provider-id",
            errors=errors,
        )
        if provider["provider_id"] not in canonical_provider_names(latest):
            errors.append("provider CLI source provider_id must exist in latest inventory")
    if provider is not None and proof is not None:
        require_equal(
            proof["provider_id"],
            provider["provider_id"],
            diagnostic="provider CLI source proof provider_id must match provider record",
            errors=errors,
        )
    if errors or provider is None or proof is None:
        return None
    payload = source_common_payload(publish, "provider")
    payload.update(
        {
            **source_snapshot_fields(publish),
            "provider": provider,
            "proof": {
                "provider_id": proof["provider_id"],
                "leaf_index": proof["leaf_index"],
                "siblings_hex": proof["siblings_hex"],
            },
        }
    )
    return payload


def build_source_events(
    args: argparse.Namespace,
    inputs: SourceBoundInputs,
    *,
    errors: list[str],
) -> dict[str, Any] | None:
    """Build event evidence from an exact authenticated watch response."""

    anchors = validated_publish_and_latest(
        inputs,
        now_unix=args.now_unix,
        errors=errors,
    )
    source = inputs.source.payload
    if not require_exact_fields(
        source,
        EVENTS_SOURCE_FIELDS,
        label="events CLI source",
        errors=errors,
    ):
        return None
    if anchors is None:
        return None
    publish, latest = anchors
    since = require_source_int(
        source,
        "since",
        label="events CLI source",
        errors=errors,
    )
    limit = require_source_int(
        source,
        "limit",
        label="events CLI source",
        errors=errors,
        minimum=1,
    )
    count = require_source_int(
        source,
        "count",
        label="events CLI source",
        errors=errors,
        minimum=1,
    )
    next_since = require_source_int(
        source,
        "next_since",
        label="events CLI source",
        errors=errors,
        minimum=1,
    )
    require_equal(
        since,
        args.expected_since,
        diagnostic="events CLI source since must match --expected-since",
        errors=errors,
    )
    require_equal(
        limit,
        args.expected_limit,
        diagnostic="events CLI source limit must match --expected-limit",
        errors=errors,
    )
    raw_events = source.get("events")
    events: list[dict[str, Any]] = []
    if not isinstance(raw_events, list):
        errors.append("events CLI source.events must be an array")
    else:
        previous_sequence = since if since is not None else -1
        for index, raw_event in enumerate(raw_events):
            label = f"events CLI source.events[{index}]"
            if not require_exact_fields(
                raw_event,
                EVENT_SOURCE_FIELDS,
                label=label,
                errors=errors,
            ):
                continue
            assert isinstance(raw_event, dict)
            version = require_source_int(
                raw_event,
                "version",
                label=label,
                errors=errors,
                minimum=1,
                maximum=1,
            )
            sequence = require_source_int(
                raw_event,
                "sequence",
                label=label,
                errors=errors,
                minimum=1,
            )
            snapshot_id = require_source_hex(
                raw_event,
                "snapshot_id_hex",
                HEX32_LEN,
                label=label,
                errors=errors,
            )
            generated_at = require_source_int(
                raw_event,
                "generated_at_unix",
                label=label,
                errors=errors,
                minimum=1,
            )
            merkle_root = require_source_hex(
                raw_event,
                "merkle_root_hex",
                HEX64_LEN,
                label=label,
                errors=errors,
            )
            provider_count = require_source_int(
                raw_event,
                "provider_count",
                label=label,
                errors=errors,
                minimum=1,
            )
            require_source_hex(
                raw_event,
                "previous_snapshot_id_hex",
                HEX32_LEN,
                label=label,
                errors=errors,
                optional=True,
            )
            require_equal(
                snapshot_id,
                publish.get("snapshot_id_hex"),
                diagnostic=f"{label}.snapshot_id_hex must match publish/latest",
                errors=errors,
            )
            require_equal(
                merkle_root,
                publish.get("merkle_root_hex"),
                diagnostic=f"{label}.merkle_root_hex must match publish/latest",
                errors=errors,
            )
            require_equal(
                generated_at,
                publish.get("generated_at_unix"),
                diagnostic=f"{label}.generated_at_unix must match publish/latest",
                errors=errors,
            )
            require_equal(
                provider_count,
                latest.get("provider_count"),
                diagnostic=f"{label}.provider_count must match publish/latest",
                errors=errors,
            )
            if sequence is not None and sequence <= previous_sequence:
                errors.append("events CLI source sequences must advance strictly")
            if sequence is not None:
                previous_sequence = sequence
            if (
                version is not None
                and sequence is not None
                and generated_at is not None
                and provider_count is not None
            ):
                events.append(
                    {
                        "version": version,
                        "sequence": sequence,
                        "snapshot_id_hex": snapshot_id,
                        "generated_at_unix": generated_at,
                        "merkle_root_hex": merkle_root,
                        "provider_count": provider_count,
                    }
                )
    if count is not None:
        require_equal(
            len(events),
            count,
            diagnostic="events CLI source count must match events length",
            errors=errors,
        )
    if count is not None and limit is not None and count > limit:
        errors.append("events CLI source count must not exceed limit")
    if events and next_since is not None:
        require_equal(
            events[-1]["sequence"],
            next_since,
            diagnostic="events CLI source next_since must match final sequence",
            errors=errors,
        )
    if errors:
        return None
    payload = source_common_payload(publish, "events")
    payload.update(
        {
            **source_snapshot_fields(publish),
            "since": since,
            "limit": limit,
            "count": count,
            "next_since": next_since,
            "events": events,
        }
    )
    return payload


def build_source_verify(
    args: argparse.Namespace,
    inputs: SourceBoundInputs,
    *,
    errors: list[str],
) -> dict[str, Any] | None:
    """Build proof-replay evidence from an exact offline CLI summary."""

    anchors = validated_publish_and_latest(
        inputs,
        now_unix=args.now_unix,
        errors=errors,
    )
    if inputs.provider is None:
        errors.append("--provider-evidence is required")
        return None
    validate_canonical_provider_artifact(inputs.provider, errors=errors)
    source = inputs.source.payload
    if not require_exact_fields(
        source,
        VERIFY_SOURCE_FIELDS,
        optional=VERIFY_SOURCE_OPTIONAL_FIELDS,
        label="verify CLI source",
        errors=errors,
    ):
        return None
    if anchors is None:
        return None
    publish, latest = anchors
    snapshot_id = require_source_hex(
        source,
        "snapshot_id_hex",
        HEX32_LEN,
        label="verify CLI source",
        errors=errors,
    )
    merkle_root = require_source_hex(
        source,
        "merkle_root_hex",
        HEX64_LEN,
        label="verify CLI source",
        errors=errors,
    )
    if "previous_snapshot_id_hex" in source:
        require_source_hex(
            source,
            "previous_snapshot_id_hex",
            HEX32_LEN,
            label="verify CLI source",
            errors=errors,
        )
    generated_at = require_source_int(
        source,
        "generated_at_unix",
        label="verify CLI source",
        errors=errors,
        minimum=1,
    )
    provider_count = require_source_int(
        source,
        "provider_count",
        label="verify CLI source",
        errors=errors,
        minimum=1,
    )
    require_source_int(
        source,
        "alpha_bps",
        label="verify CLI source",
        errors=errors,
        maximum=10_000,
    )
    require_source_int(
        source,
        "current_score_weight_bps",
        label="verify CLI source",
        errors=errors,
        maximum=10_000,
    )
    valid = require_source_bool(
        source,
        "valid",
        label="verify CLI source",
        errors=errors,
    )
    proof_verified = require_source_bool(
        source,
        "proof_verified",
        label="verify CLI source",
        errors=errors,
    )
    provider_id = require_source_string(
        source,
        "provider_id",
        label="verify CLI source",
        errors=errors,
    )
    if provider_id:
        validate_provider_label_arg(
            provider_id,
            option="verify CLI source.provider_id",
            errors=errors,
        )
    provider_score = require_source_int(
        source,
        "provider_score_bps",
        label="verify CLI source",
        errors=errors,
        maximum=10_000,
    )
    proof_leaf_index = require_source_int(
        source,
        "proof_leaf_index",
        label="verify CLI source",
        errors=errors,
    )
    proof_sibling_count = require_source_int(
        source,
        "proof_sibling_count",
        label="verify CLI source",
        errors=errors,
    )
    snapshot_path = require_source_string(
        source,
        "snapshot_path",
        label="verify CLI source",
        errors=errors,
    )
    proof_path = require_source_string(
        source,
        "proof_path",
        label="verify CLI source",
        errors=errors,
    )
    require_equal(
        snapshot_path,
        str(args.expected_snapshot_path),
        diagnostic="verify CLI source snapshot_path must match --expected-snapshot-path",
        errors=errors,
    )
    require_equal(
        proof_path,
        str(args.expected_proof_path),
        diagnostic="verify CLI source proof_path must match --expected-proof-path",
        errors=errors,
    )
    require_equal(
        snapshot_id,
        publish.get("snapshot_id_hex"),
        diagnostic="verify CLI source snapshot_id_hex must match publish/latest",
        errors=errors,
    )
    require_equal(
        merkle_root,
        publish.get("merkle_root_hex"),
        diagnostic="verify CLI source merkle_root_hex must match publish/latest",
        errors=errors,
    )
    require_equal(
        generated_at,
        publish.get("generated_at_unix"),
        diagnostic="verify CLI source generated_at_unix must match publish/latest",
        errors=errors,
    )
    require_equal(
        provider_count,
        latest.get("provider_count"),
        diagnostic="verify CLI source provider_count must match publish/latest",
        errors=errors,
    )
    require_equal(
        provider_id,
        args.expected_provider_id,
        diagnostic="verify CLI source provider_id must match --expected-provider-id",
        errors=errors,
    )
    if valid is not True:
        errors.append("verify CLI source valid must be true")
    if proof_verified is not True:
        errors.append("verify CLI source proof_verified must be true")
    canonical_provider = inputs.provider.payload.get("provider")
    canonical_proof = inputs.provider.payload.get("proof")
    if not isinstance(canonical_provider, dict) or not isinstance(canonical_proof, dict):
        errors.append("--provider-evidence must contain provider and proof objects")
    else:
        require_equal(
            provider_id,
            canonical_provider.get("provider_id"),
            diagnostic="verify CLI source provider_id must match provider evidence",
            errors=errors,
        )
        require_equal(
            provider_score,
            canonical_provider.get("score_bps"),
            diagnostic="verify CLI source provider score must match provider evidence",
            errors=errors,
        )
        require_equal(
            proof_leaf_index,
            canonical_proof.get("leaf_index"),
            diagnostic="verify CLI source proof leaf index must match provider evidence",
            errors=errors,
        )
        siblings = canonical_proof.get("siblings_hex")
        sibling_count = len(siblings) if isinstance(siblings, list) else None
        require_equal(
            proof_sibling_count,
            sibling_count,
            diagnostic="verify CLI source proof sibling count must match provider evidence",
            errors=errors,
        )
    if errors:
        return None
    payload = source_common_payload(publish, "verify")
    payload.update(
        {
            **source_snapshot_fields(publish),
            "provider_count": latest["provider_count"],
            "providers": latest["providers"],
            "valid": True,
            "provider_id": provider_id,
            "provider_score_bps": provider_score,
            "proof_verified": True,
        }
    )
    return payload


def build_source_bound_payload(
    args: argparse.Namespace,
    inputs: SourceBoundInputs,
    errors: list[str],
) -> dict[str, Any] | None:
    """Build one checker-ready canary from exact source-bound artifacts."""

    if args.kind == "latest":
        return build_source_latest(inputs, now_unix=args.now_unix, errors=errors)
    if args.kind == "provider":
        return build_source_provider(args, inputs, errors=errors)
    if args.kind == "events":
        return build_source_events(args, inputs, errors=errors)
    if args.kind == "verify":
        return build_source_verify(args, inputs, errors=errors)
    errors.append("unsupported source-bound evidence kind")
    return None


def validate_source_bound_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
    inputs: SourceBoundInputs,
) -> list[str]:
    """Validate the adapted payload together with its real source anchors."""

    evidence = [
        LoadedEvidence(
            "publish",
            inputs.publish.path,
            inputs.publish.payload,
            inputs.publish.digest,
        )
    ]
    required_kinds = ["publish"]
    if args.kind != "latest":
        assert inputs.latest is not None
        evidence.append(
            LoadedEvidence(
                "latest",
                inputs.latest.path,
                inputs.latest.payload,
                inputs.latest.digest,
            )
        )
        required_kinds.append("latest")
    if args.kind == "verify":
        assert inputs.provider is not None
        evidence.append(
            LoadedEvidence(
                "provider",
                inputs.provider.path,
                inputs.provider.payload,
                inputs.provider.digest,
            )
        )
        required_kinds.append("provider")
    evidence.append(loaded(args.kind, payload, args.out))
    required_kinds.append(args.kind)
    summary = validate_evidence_set(
        evidence,
        required_kinds=tuple(required_kinds),
        required_providers=(
            (args.expected_provider_id,)
            if args.kind in {"provider", "verify"}
            else ()
        ),
        now_unix=args.now_unix,
        max_snapshot_age_secs=DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        max_ingest_lag_secs=DEFAULT_MAX_INGEST_LAG_SECS,
    )
    if summary.get("status") == "ready":
        return []
    return ["source-bound canary must satisfy the canonical reputation checker"]


def common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by reputation canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def snapshot_fields(args: argparse.Namespace) -> dict[str, Any]:
    """Return the shared snapshot binding fields."""

    return {
        "snapshot_id_hex": args.snapshot_id_hex,
        "merkle_root_hex": args.merkle_root_hex,
    }


def provider_rows(args: argparse.Namespace) -> list[dict[str, str]]:
    """Return payload-free provider inventory rows for count-bearing canaries."""

    providers = getattr(args, "providers", None)
    if providers is None:
        providers = split_csv_values(args.provider_name)
    return [{"name": name} for name in providers]


def inventory_rows(names: Sequence[str]) -> list[dict[str, str]]:
    """Return payload-free reviewed inventory rows."""

    return [{"name": name} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free reputation rollout canary payload."""

    payload = common_payload(args)
    payload.update(snapshot_fields(args))
    if args.kind in SNAPSHOT_ANCHOR_KINDS:
        payload.update(
            {
                "status": "accepted",
                "weights_digest_hex": args.weights_digest_hex,
                "provider_count": args.provider_count,
                "providers": provider_rows(args),
            }
        )
    elif args.kind == "provider":
        payload.update(
            {
                "provider": {
                    "provider_id": args.provider_id,
                    "score_bps": args.provider_score_bps,
                },
                "proof": {
                    "provider_id": args.provider_id,
                    "leaf_index": args.leaf_index,
                    "siblings_hex": args.sibling_hex,
                },
            }
        )
    elif args.kind == "events":
        payload.update(
            {
                "since": args.since,
                "limit": args.event_count,
                "count": args.event_count,
                "next_since": args.next_since,
                "events": [
                    {
                        "version": 1,
                        "sequence": args.next_since,
                        "snapshot_id_hex": args.snapshot_id_hex,
                        "generated_at_unix": args.generated_at_unix,
                        "merkle_root_hex": args.merkle_root_hex,
                        "provider_count": args.provider_count,
                    }
                ],
            }
        )
    elif args.kind == "verify":
        payload.update(
            {
                "valid": True,
                "proof_verified": True,
                "provider_count": args.provider_count,
                "providers": provider_rows(args),
                "provider_id": args.provider_id,
                "provider_score_bps": args.provider_score_bps,
            }
        )
    elif args.kind == "metrics":
        payload.update(
            {
                "status": "passed",
                "metrics_scrape_success": True,
                "provider_count": args.provider_count,
                "providers": provider_rows(args),
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
                "snapshot_age_seconds": args.snapshot_age_seconds,
                "ingest_lag_seconds": args.ingest_lag_seconds,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "transport":
        payload.update(
            {
                "status": "passed",
                "sse_connected": True,
                "websocket_connected": True,
                "sse_event_count": args.sse_event_count,
                "sse_events": inventory_rows(args.sse_events),
                "websocket_event_count": args.websocket_event_count,
                "websocket_events": inventory_rows(args.websocket_events),
                "response_bodies_included": False,
            }
        )
    elif args.kind == "consumption":
        payload.update(
            {
                "status": "passed",
                "routing_score_consumed": True,
                "routing_weight_changed": True,
                "incentive_score_consumed": True,
                "provider_count": args.provider_count,
                "providers": provider_rows(args),
                "raw_provider_records_included": False,
            }
        )
    return payload


def validate_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate threshold-bound facts before payload construction."""

    if args.snapshot_age_seconds > DEFAULT_MAX_SNAPSHOT_AGE_SECS:
        errors.append(
            f"--snapshot-age-seconds must be <= {DEFAULT_MAX_SNAPSHOT_AGE_SECS}"
        )
    if args.ingest_lag_seconds > DEFAULT_MAX_INGEST_LAG_SECS:
        errors.append(f"--ingest-lag-seconds must be <= {DEFAULT_MAX_INGEST_LAG_SECS}")
    if args.next_since <= args.since:
        errors.append("--next-since must be greater than --since")


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    if args.source_cli_json is not None:
        validate_source_mode_options(args, errors)
        return errors
    if args.provided_options & SOURCE_BOUND_OPTIONS:
        errors.append(
            "source-bound options require --source-cli-json"
        )
        return errors
    required_manual_fields = (
        ("deployment_id", "--deployment-id"),
        ("environment", "--environment"),
        ("generated_at_unix", "--generated-at-unix"),
        ("snapshot_id_hex", "--snapshot-id-hex"),
        ("merkle_root_hex", "--merkle-root-hex"),
        ("weights_digest_hex", "--weights-digest-hex"),
    )
    for field, option in required_manual_fields:
        if getattr(args, field) is None:
            errors.append(f"{option} is required")
    require_rollout_deployment_id(
        {"--deployment-id": args.deployment_id},
        errors,
        field="--deployment-id",
    )
    require_rollout_environment(
        {"--environment": args.environment},
        errors,
        field="--environment",
    )
    validate_provider_id_arg(args.provider_id, errors=errors)
    validate_hex(args.snapshot_id_hex, option="--snapshot-id-hex", length=HEX32_LEN, errors=errors)
    validate_hex(args.merkle_root_hex, option="--merkle-root-hex", length=HEX64_LEN, errors=errors)
    validate_hex(
        args.weights_digest_hex,
        option="--weights-digest-hex",
        length=HEX64_LEN,
        errors=errors,
    )
    validate_thresholds(args, errors)
    validate_provider_inventory(args, errors)
    if args.kind == "metrics":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    if args.provider_score_bps > 10_000:
        errors.append("--provider-score-bps must be <= 10000")
    if args.kind == "transport":
        args.sse_events = validate_reviewed_inventory(
            args.sse_event,
            expected_count=args.sse_event_count,
            option="--sse-event",
            count_option="--sse-event-count",
            pattern=SSE_EVENT_LABEL_PATTERN,
            label_error=SSE_EVENT_LABEL_ERROR,
            errors=errors,
        )
        args.websocket_events = validate_reviewed_inventory(
            args.websocket_event,
            expected_count=args.websocket_event_count,
            option="--websocket-event",
            count_option="--websocket-event-count",
            pattern=WEBSOCKET_EVENT_LABEL_PATTERN,
            label_error=WEBSOCKET_EVENT_LABEL_ERROR,
            errors=errors,
        )
    if args.kind == "provider":
        expected_sibling_count = (args.provider_count - 1).bit_length()
        if len(args.sibling_hex) != expected_sibling_count:
            errors.append("--sibling-hex count must match --provider-count")
        if args.leaf_index >= args.provider_count:
            errors.append("--leaf-index must be below --provider-count")
        if args.provider_count > 1 and not args.sibling_hex:
            errors.append("--sibling-hex is required for provider")
        seen_siblings: set[str] = set()
        for sibling in args.sibling_hex:
            previous_error_count = len(errors)
            validate_hex(sibling, option="--sibling-hex", length=HEX64_LEN, errors=errors)
            if isinstance(sibling, str) and len(errors) == previous_error_count:
                if sibling in seen_siblings:
                    errors.append("duplicate --sibling-hex")
                seen_siblings.add(sibling)
    return errors


def loaded(kind: str, payload: dict[str, Any], path: Path) -> LoadedEvidence:
    """Wrap generated payload for reputation gate validation."""

    return LoadedEvidence(kind, path, payload, "ab" * 32)


def anchor_payload(args: argparse.Namespace, kind: str) -> dict[str, Any]:
    """Build a matching publish/latest anchor payload for bound canaries."""

    anchor_args = argparse.Namespace(**vars(args))
    anchor_args.kind = kind
    return build_payload(anchor_args)


def provider_anchor_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a matching provider proof anchor for reputation gate validation."""

    provider_args = argparse.Namespace(**vars(args))
    provider_args.kind = "provider"
    provider_args.sibling_hex = args.sibling_hex or [DEFAULT_PROOF_SIBLING_HEX]
    return build_payload(provider_args)


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the reputation gate contract."""

    payloads = {
        "publish": anchor_payload(args, "publish"),
        "latest": anchor_payload(args, "latest"),
        "provider": provider_anchor_payload(args),
        args.kind: payload,
    }
    evidence = [
        loaded(kind, item, args.out.with_name(f"{kind}.json"))
        for kind, item in payloads.items()
    ]
    required_kinds = tuple(payloads)
    summary = validate_evidence_set(
        evidence,
        required_kinds=required_kinds,
        required_providers=(args.provider_id,) if args.kind == "provider" else (),
        now_unix=args.now_unix,
        max_snapshot_age_secs=DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        max_ingest_lag_secs=DEFAULT_MAX_INGEST_LAG_SECS,
    )
    errors: list[str] = []
    if summary["status"] != "ready":
        errors.extend(summary.get("errors", []))
        for row in summary.get("required", {}).values():
            errors.extend(row.get("errors", []))
            for artifact in row.get("artifacts", []):
                errors.extend(artifact.get("errors", []))
    return sorted(set(errors))


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the canary JSON atomically without following output symlinks."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        parent_label = path_diagnostic_label(parent)
        return [
            f"--out parent `{parent_label}` cannot be created: "
            f"{error_diagnostic_label(error, path_label=parent_label)}"
        ]
    tmp_name = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_path = parent / tmp_name
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        if nofollow:
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        parent_sync_errors = fsync_checker_output_parent(path, label="--out")
        if parent_sync_errors:
            return parent_sync_errors
    except (OSError, RuntimeError) as error:
        path_label = path_diagnostic_label(path)
        try:
            if fd >= 0:
                os.close(fd)
        finally:
            try:
                tmp_path.unlink()
            except FileNotFoundError:
                pass
            except (OSError, RuntimeError):
                pass
        return [
            f"--out `{path_label}` cannot be written: "
            f"{error_diagnostic_label(error, path_label=path_label)}"
        ]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SFM-3 reputation canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id")
    parser.add_argument("--environment")
    parser.add_argument("--generated-at-unix", type=positive_int_arg)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument("--snapshot-id-hex")
    parser.add_argument("--merkle-root-hex")
    parser.add_argument("--weights-digest-hex")
    parser.add_argument("--provider-id", default="provider-a")
    parser.add_argument("--provider-count", type=positive_int_arg, default=2)
    parser.add_argument("--provider-name", action="append", default=[])
    parser.add_argument("--provider-score-bps", type=non_negative_int_arg, default=9400)
    parser.add_argument("--leaf-index", type=non_negative_int_arg, default=1)
    parser.add_argument("--sibling-hex", action="append", default=[])
    parser.add_argument("--since", type=non_negative_int_arg, default=0)
    parser.add_argument("--next-since", type=positive_int_arg, default=1)
    parser.add_argument("--event-count", type=positive_int_arg, default=1)
    parser.add_argument("--snapshot-age-seconds", type=non_negative_int_arg, default=120)
    parser.add_argument("--ingest-lag-seconds", type=non_negative_int_arg, default=60)
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--sse-event-count", type=positive_int_arg, default=1)
    parser.add_argument("--sse-event", action="append", default=[])
    parser.add_argument("--websocket-event-count", type=positive_int_arg, default=1)
    parser.add_argument("--websocket-event", action="append", default=[])
    parser.add_argument("--source-cli-json", type=Path)
    parser.add_argument("--publish-evidence", type=Path)
    parser.add_argument("--latest-evidence", type=Path)
    parser.add_argument("--provider-evidence", type=Path)
    parser.add_argument("--expected-provider-id")
    parser.add_argument("--expected-since", type=non_negative_int_arg)
    parser.add_argument("--expected-limit", type=positive_int_arg)
    parser.add_argument("--expected-snapshot-path", type=Path)
    parser.add_argument("--expected-proof-path", type=Path)
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        args = parse_expanded_args(parser, expanded_args)
        if args.source_cli_json is not None:
            reject_duplicate_scalar_options(expanded_args)
        args.provided_options = expanded_option_names(expanded_args)
        return args
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    errors = validate_inputs(args)
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS reputation canary inputs are incomplete:",
            errors,
        )
        return 2

    if args.source_cli_json is not None:
        load_errors: list[str] = []
        inputs = load_source_bound_inputs(args, load_errors)
        if load_errors or inputs is None:
            emit_checker_error_block(
                "ERROR: SoraFS reputation source artifacts could not be loaded:",
                load_errors,
            )
            return 2
        binding_errors: list[str] = []
        payload = build_source_bound_payload(args, inputs, binding_errors)
        if binding_errors or payload is None:
            emit_checker_error_block(
                "ERROR: SoraFS reputation source artifacts do not match:",
                binding_errors,
            )
            return 2
        payload_errors = validate_source_bound_payload(payload, args, inputs)
    else:
        payload = build_payload(args)
        payload_errors = validate_generated_payload(payload, args)
    if payload_errors:
        emit_checker_error_lines(payload_errors)
        return 2

    write_errors = write_payload_atomic(args.out, payload)
    if write_errors:
        emit_checker_error_lines(write_errors)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
