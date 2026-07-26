#!/usr/bin/env python3
"""Validate Ministry transparency release artefacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Dict, Iterable

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RELEASE_ROOT = REPO_ROOT / "artifacts" / "ministry" / "transparency"
EPSILON_TOLERANCE = 1e-9
MAX_EPSILON_COUNTS = 0.75
MAX_EPSILON_ACCURACY = 0.5
MAX_DELTA = 1e-6
MIN_SUPPRESS_THRESHOLD = 5
MIN_ACCURACY_SAMPLES = 50


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def blake2b_256(path: Path) -> str:
    digest = hashlib.blake2b(digest_size=32)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as err:  # pragma: no cover - defensive
        raise SystemExit(f"missing required file: {path}") from err
    except json.JSONDecodeError as err:
        raise SystemExit(f"invalid JSON in {path}: {err}") from err


def load_checksums(path: Path) -> Dict[str, str]:
    mapping: Dict[str, str] = {}
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            raise SystemExit(f"malformed checksum line in {path}: {raw_line}")
        digest, rel_path = parts
        mapping[rel_path.strip()] = digest
    if not mapping:
        raise SystemExit(f"no checksum entries found in {path}")
    return mapping


def resolve_under(base: Path, rel: Path) -> Path:
    if rel.is_absolute():
        raise SystemExit(f"expected relative path inside release bundle, got {rel}")
    return (base / rel).resolve()


def _require_numeric(obj: dict, key: str, *, context: str) -> float:
    value = obj.get(key)
    if not isinstance(value, (int, float)):
        raise SystemExit(f"{context} is missing numeric field '{key}'")
    return float(value)


def _require_str(obj: dict, key: str, *, context: str) -> str:
    value = obj.get(key)
    if not isinstance(value, str) or not value:
        raise SystemExit(f"{context} is missing string field '{key}'")
    return value


def _ensure_close(label: str, actual: float, expected: float) -> None:
    if abs(actual - expected) > EPSILON_TOLERANCE:
        raise SystemExit(
            f"{label} mismatch: expected {expected:.6f}, found {actual:.6f}"
        )


def _validate_budget_limits(metadata: dict, dp_report: dict) -> None:
    context_dp = "dp_report"
    epsilon_counts = _require_numeric(dp_report, "epsilon_counts", context=context_dp)
    epsilon_accuracy = _require_numeric(
        dp_report, "epsilon_accuracy", context=context_dp
    )
    delta = _require_numeric(dp_report, "delta", context=context_dp)
    suppress_threshold = _require_numeric(
        dp_report, "suppress_threshold", context=context_dp
    )
    min_accuracy_samples = _require_numeric(
        dp_report, "min_accuracy_samples", context=context_dp
    )

    context_meta = "sanitized metadata"
    meta_epsilon_counts = _require_numeric(
        metadata, "epsilon_counts", context=context_meta
    )
    meta_epsilon_accuracy = _require_numeric(
        metadata, "epsilon_accuracy", context=context_meta
    )
    meta_delta = _require_numeric(metadata, "delta", context=context_meta)
    meta_suppress_threshold = _require_numeric(
        metadata, "suppress_threshold", context=context_meta
    )
    meta_min_accuracy_samples = _require_numeric(
        metadata, "min_accuracy_samples", context=context_meta
    )

    _ensure_close("epsilon_counts", epsilon_counts, meta_epsilon_counts)
    _ensure_close("epsilon_accuracy", epsilon_accuracy, meta_epsilon_accuracy)
    _ensure_close("delta", delta, meta_delta)
    _ensure_close(
        "suppress_threshold", suppress_threshold, meta_suppress_threshold
    )
    _ensure_close(
        "min_accuracy_samples",
        min_accuracy_samples,
        meta_min_accuracy_samples,
    )

    if epsilon_counts - MAX_EPSILON_COUNTS > EPSILON_TOLERANCE:
        raise SystemExit(
            f"epsilon_counts {epsilon_counts:.6f} exceeds {MAX_EPSILON_COUNTS:.2f}"
        )
    if epsilon_accuracy - MAX_EPSILON_ACCURACY > EPSILON_TOLERANCE:
        raise SystemExit(
            f"epsilon_accuracy {epsilon_accuracy:.6f} exceeds {MAX_EPSILON_ACCURACY:.1f}"
        )
    if delta - MAX_DELTA > EPSILON_TOLERANCE:
        raise SystemExit(f"delta {delta:.2e} exceeds {MAX_DELTA:.1e}")
    if suppress_threshold + EPSILON_TOLERANCE < MIN_SUPPRESS_THRESHOLD:
        raise SystemExit(
            f"suppress_threshold {suppress_threshold} below minimum {MIN_SUPPRESS_THRESHOLD}"
        )
    if min_accuracy_samples + EPSILON_TOLERANCE < MIN_ACCURACY_SAMPLES:
        raise SystemExit(
            f"min_accuracy_samples {min_accuracy_samples} below minimum {MIN_ACCURACY_SAMPLES}"
        )

    _validate_bucket_suppression(dp_report, suppress_threshold, min_accuracy_samples)


def _validate_bucket_suppression(
    dp_report: dict, suppress_threshold: float, min_accuracy_samples: float
) -> None:
    count_buckets = dp_report.get("count_buckets")
    if not isinstance(count_buckets, list):
        raise SystemExit("dp_report must include count_buckets[]")
    for entry in count_buckets:
        if not isinstance(entry, dict):
            raise SystemExit(f"count_buckets entry must be an object: {entry}")
        bucket = _require_str(entry, "bucket", context="count_bucket")
        sanitized_value = _require_numeric(
            entry, "sanitized", context=f"count_bucket[{bucket}]"
        )
        suppressed = bool(entry.get("suppressed"))
        if (
            not suppressed
            and sanitized_value + EPSILON_TOLERANCE < suppress_threshold
        ):
            raise SystemExit(
                f"count bucket '{bucket}' has sanitized value {sanitized_value:.3f} "
                f"below threshold {suppress_threshold} but suppression flag is false"
            )

    accuracy_buckets = dp_report.get("accuracy_buckets")
    if not isinstance(accuracy_buckets, list):
        raise SystemExit("dp_report must include accuracy_buckets[]")
    for entry in accuracy_buckets:
        if not isinstance(entry, dict):
            raise SystemExit(f"accuracy_buckets entry must be an object: {entry}")
        policy = _require_str(entry, "policy", context="accuracy_bucket")
        sanitized_samples = _require_numeric(
            entry, "sanitized_samples", context=f"accuracy_bucket[{policy}]"
        )
        suppressed = bool(entry.get("suppressed"))
        if (
            not suppressed
            and sanitized_samples + EPSILON_TOLERANCE < min_accuracy_samples
        ):
            raise SystemExit(
                f"accuracy bucket '{policy}' exposes {sanitized_samples:.3f} samples "
                f"below minimum {min_accuracy_samples}"
            )


def validate_release(release_dir: Path) -> None:
    manifest_path = release_dir / "transparency_manifest.json"
    manifest = load_json(manifest_path)

    quarter = manifest.get("quarter")
    if not isinstance(quarter, str):
        raise SystemExit(f"manifest {manifest_path} is missing a string quarter field")
    dir_quarter = release_dir.name
    if dir_quarter != quarter:
        raise SystemExit(
            f"release directory {release_dir} does not match manifest quarter {quarter}"
        )

    checksums_rel = Path(manifest.get("checksums_file", "checksums.sha256"))
    checksums_path = resolve_under(release_dir, checksums_rel)
    if not checksums_path.exists():
        raise SystemExit(f"checksums file missing: {checksums_path}")
    checksums = load_checksums(checksums_path)

    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise SystemExit(f"manifest {manifest_path} must declare at least one artifact")
    artifact_by_label = {}
    for entry in artifacts:
        if not isinstance(entry, dict):
            raise SystemExit(f"manifest entry is not an object: {entry}")
        label = entry.get("label")
        path_str = entry.get("path")
        digest = entry.get("sha256")
        if not all(isinstance(val, str) and val for val in (label, path_str, digest)):
            raise SystemExit(f"manifest entry missing label/path/digest: {entry}")
        artifact_path = resolve_under(release_dir, Path(path_str))
        if not artifact_path.exists():
            raise SystemExit(f"artifact '{label}' missing at {artifact_path}")
        actual_digest = sha256(artifact_path)
        if actual_digest != digest:
            raise SystemExit(
                f"sha256 mismatch for {artifact_path}: manifest={digest} actual={actual_digest}"
            )
        checksum_digest = checksums.get(path_str)
        if checksum_digest != digest:
            raise SystemExit(
                f"checksums entry mismatch for {path_str}: manifest={digest} checksum={checksum_digest}"
            )
        artifact_by_label[label] = artifact_path

    for required in ("sanitized_metrics", "dp_report"):
        if required not in artifact_by_label:
            raise SystemExit(
                f"manifest {manifest_path} is missing required '{required}' artifact entry"
            )

    sanitized_path = artifact_by_label["sanitized_metrics"]
    sanitized = load_json(sanitized_path)
    if sanitized.get("quarter") != quarter:
        raise SystemExit(
            f"sanitized metrics quarter mismatch: {sanitized_path} vs {quarter}"
        )
    metadata = sanitized.get("metadata") or {}
    seed_commitment = metadata.get("seed_commitment")

    dp_path = artifact_by_label["dp_report"]
    dp_report = load_json(dp_path)
    if dp_report.get("quarter") != quarter:
        raise SystemExit(f"DP report quarter mismatch: {dp_path} vs {quarter}")
    if seed_commitment and dp_report.get("seed_commitment") != seed_commitment:
        raise SystemExit(
            "seed_commitment mismatch between sanitized metrics and DP report"
        )
    _validate_budget_limits(metadata, dp_report)

    manifest_cid = manifest.get("sorafs_cid")
    if not isinstance(manifest_cid, str) or not manifest_cid:
        raise SystemExit(f"manifest {manifest_path} missing sorafs_cid")

    action_path = release_dir / "transparency_release_action.json"
    action = load_json(action_path)
    if action.get("action") != "TransparencyReleaseV1":
        raise SystemExit(f"unexpected action type in {action_path}")
    if action.get("quarter") != quarter:
        raise SystemExit(f"action quarter mismatch in {action_path}")

    manifest_digest = blake2b_256(manifest_path)
    if action.get("manifest_digest_blake2b_256") != manifest_digest:
        raise SystemExit(
            f"manifest digest mismatch in {action_path}: expected {manifest_digest}"
        )

    expected_manifest_rel = str(manifest_path.relative_to(REPO_ROOT))
    if action.get("manifest_path") != expected_manifest_rel:
        raise SystemExit(
            f"action manifest_path mismatch: {action.get('manifest_path')} vs {expected_manifest_rel}"
        )

    expected_checksums_rel = str(checksums_path.relative_to(REPO_ROOT))
    if action.get("checksums_path") != expected_checksums_rel:
        raise SystemExit(
            f"action checksums_path mismatch: {action.get('checksums_path')} vs {expected_checksums_rel}"
        )

    action_cid = action.get("sorafs_cid_hex")
    if action_cid != manifest_cid:
        raise SystemExit(
            f"action sorafs_cid_hex {action_cid} does not match manifest {manifest_cid}"
        )

    dashboards_sha = action.get("dashboards_git_sha")
    if not isinstance(dashboards_sha, str) or len(dashboards_sha) != 40:
        raise SystemExit(
            f"dashboards_git_sha must be a 40-character hex digest: {dashboards_sha}"
        )
    try:
        int(dashboards_sha, 16)
    except (TypeError, ValueError):  # pragma: no cover - defensive
        raise SystemExit(f"dashboards_git_sha must be hex: {dashboards_sha}")

    print(f"[ministry] validated transparency bundle {release_dir.relative_to(REPO_ROOT)}")


def discover_release_dirs(user_dirs: Iterable[str]) -> list[Path]:
    if user_dirs:
        dirs = [Path(path).resolve() for path in user_dirs]
    else:
        if not DEFAULT_RELEASE_ROOT.exists():
            raise SystemExit(
                f"default release root {DEFAULT_RELEASE_ROOT} does not exist; provide --release"
            )
        dirs = sorted(p.resolve() for p in DEFAULT_RELEASE_ROOT.iterdir() if p.is_dir())
    if not dirs:
        raise SystemExit("no release directories to validate")
    return dirs


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Validate Ministry transparency release evidence"
    )
    parser.add_argument(
        "--release",
        action="append",
        help="Path to a transparency release directory (may be repeated)",
    )
    args = parser.parse_args()

    release_dirs = discover_release_dirs(args.release or [])
    for release_dir in release_dirs:
        validate_release(release_dir)


if __name__ == "__main__":
    main()
