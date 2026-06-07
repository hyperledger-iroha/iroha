"""Roll up Kagemusha production-readiness evidence into a strict summary."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
from pathlib import Path
import shlex
import sys
from typing import Any, Iterable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402


SUMMARY_SCHEMA = "iroha.kagemusha.production_readiness.v1"
ABI6_MANIFEST_PATH = "fixtures/kagemusha_recursive_spend_abi6/manifest.json"
LINEAGE_PROOF_EVIDENCE_SCHEMA = "iroha.kagemusha.lineage_proof_evidence.v1"
LINEAGE_PROOF_EVIDENCE_FILENAME = "lineage-proof-evidence.json"
DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH = f"artifacts/kagemusha/{LINEAGE_PROOF_EVIDENCE_FILENAME}"
DEFAULT_MIN_SIGNED_AT_UTC = "2026-06-06T00:00:00Z"
DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS = 300
ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"
LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL = "<lineage-proof-evidence>"
EXPECTED_LINEAGE_PROOF_OPENING_LEN = 128
EXPECTED_LINEAGE_PROOF_IPA_K = 8
EXPECTED_LINEAGE_PROOF_BACKEND = "halo2/ipa"
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_RUNTIME_KEYGEN_ENV = (
    "IROHA_KAGEMUSHA_ALLOW_RUNTIME_LINEAGE_KEYGEN"
)
EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE = (
    "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-85x3"
)
EXPECTED_LINEAGE_CIRCUIT_IDS = {
    "one_hop": "kagemusha-recursive-spend-lineage-onehop-v1",
    "append": "kagemusha-recursive-spend-lineage-append-v1",
}
LINEAGE_PROOF_REQUIRED_ARTIFACTS = (
    "lineage-init-len128.norito",
    "lineage-init-len128.record.norito",
    "lineage-init-len128.vk",
    "lineage-init-len128.pk",
    "lineage-append-len128.norito",
    "lineage-append-len128.record.norito",
    "lineage-append-len128.vk",
    "lineage-append-len128.pk",
)
LINEAGE_PROOF_REQUIRED_TESTS = {
    "record_archive_proof": (
        "kagemusha_recursive_spend_lineage_init_append_from_record_archives_proves_reserved_lineage_output"
    ),
}
LINEAGE_PROOF_REQUIRED_TEST_LOGS = {
    "record_archive_proof": "record-archive-proof.log",
}
EXPECTED_LINEAGE_PROOF_RESULT_PREFIX = (
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;"
)


def expected_lineage_proof_command(expected_name: str) -> str:
    """Return the canonical production Reserved-lineage proof command string."""

    return (
        "cargo test -p iroha_core "
        f"{expected_name} "
        "--lib -- --ignored --test-threads=1 --nocapture"
    )
MAX_LINEAGE_PROOF_LOG_BYTES = 64 * 1024 * 1024
LINEAGE_PROOF_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        "generated_at_utc",
        "opening_len",
        "ipa_k",
        "verifier_backend",
        "verifier_witness_profile",
        "record_archive_proof_runtime_keygen_env",
        "circuit_ids",
        "artifacts",
        "tests",
    }
)
LINEAGE_PROOF_TEST_FIELDS: frozenset[str] = frozenset(
    {
        "name",
        "status",
        "ignored",
        "command",
        "elapsed_seconds",
        "log_path",
        "log_sha256",
    }
)
ABI6_OPERATION_SYMBOLS = (
    "connect_norito_kagemusha_recursive_spend_init",
    "connect_norito_kagemusha_recursive_spend_append",
    "connect_norito_kagemusha_recursive_spend_transition_profile_init",
    "connect_norito_kagemusha_recursive_spend_transition_profile_append",
    "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
    "connect_norito_kagemusha_recursive_spend_verify",
    "connect_norito_kagemusha_recursive_spend_redeem",
)
EXPECTED_ABI6_LIMITS = {
    "compact_token_max_hops": 64,
    "reserved_lineage_witnessless_max_hops": 64,
    "previous_proof_open_envelopes_required_count": 1,
    "native_archive_max_bytes": 64 * 1024 * 1024,
}
LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS = {
    "crates/iroha_cli/src/zk.rs": (
        "KagemushaCommand::LineageKeyArtifacts",
        "KagemushaCommand::LineageRecord",
        "KagemushaLineageRecordArgs",
        "record_out: Option<std::path::PathBuf>",
        "record_namespace: String",
        "record_version: u32",
        "kagemusha_lineage_vk_record_from_bytes",
        "std::fs::read(&self.vk)",
        "kagemusha_recursive_spend_lineage_vk_record_from_box(",
        "kagemusha_recursive_spend_lineage_append_vk_record_from_box(",
        "kagemusha_lineage_record_run_writes_norito_record_from_existing_vk_file",
        'record_summary = format!(", record={} bytes", record_bytes.len())',
    ),
    "crates/iroha_core/src/zk.rs": (
        "kagemusha_recursive_spend_lineage_vk_record_from_box_for_circuit",
        "pub fn kagemusha_recursive_spend_lineage_vk_record_from_box(",
        "pub fn kagemusha_recursive_spend_lineage_append_vk_record_from_box(",
        "does not generate a verifier key at runtime",
        "lineage_vk_record_from_box_canonicalizes_profiles_without_keygen",
    ),
    "docs/source/offline_kagemusha.md": (
        "--record-out artifacts/kagemusha/lineage-init-len128.record.norito",
        "--record-out artifacts/kagemusha/lineage-append-len128.record.norito",
        "iroha app zk kagemusha lineage-record",
        "--vk artifacts/kagemusha/lineage-init-len128.vk",
        "--vk artifacts/kagemusha/lineage-append-len128.vk",
        "governance/WSV `VerifyingKeyRecord` bound to `offline_kagemusha`",
        "`--record-namespace` and `--record-version`",
    ),
}
SUMMARY_OUT_PATH_INVALID_CODE = "kagemusha_summary_out_path_invalid"


def utc_now() -> str:
    """Return a canonical current UTC timestamp."""

    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00",
        "Z",
    )


def blocker(code: str, message: str, **extra: Any) -> dict[str, Any]:
    """Build a normalized readiness blocker."""

    item: dict[str, Any] = {"code": code, "message": message}
    item.update(extra)
    return item


def _secret_looking_path_blocker(
    value: str | None,
    *,
    label: str,
    code: str,
) -> dict[str, Any] | None:
    if value is not None and device_lab.SECRET_RE.search(value):
        return blocker(code, f"{label} must not contain secret-looking material")
    return None


def validate_repo_root_path(root: Path) -> list[dict[str, Any]]:
    """Reject repo roots that could alias checked-in readiness trust roots."""

    secret_blocker = _secret_looking_path_blocker(
        str(root),
        label="--repo-root",
        code="kagemusha_repo_root_path_invalid",
    )
    if secret_blocker is not None:
        return [secret_blocker]
    errors: list[str] = []
    if root.is_symlink():
        errors.append("--repo-root must not be a symlink")
    errors.extend(
        device_lab.validate_no_symlink_ancestors(
            root,
            "--repo-root ancestor directory",
        )
    )
    if root.exists() and not root.is_dir():
        errors.append("--repo-root must be a directory")
    if not root.is_dir():
        errors.append("--repo-root must be an existing directory")
    return [
        blocker("kagemusha_repo_root_path_invalid", error)
        for error in errors
    ]


def validate_cli_path_arguments(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Reject unsafe local path arguments before summaries are built."""

    blockers: list[dict[str, Any]] = []
    for value, label, code in (
        (args.repo_root, "--repo-root", "kagemusha_repo_root_path_invalid"),
        (
            args.device_lab_root,
            "--device-lab-root",
            "android_device_lab_root_path_invalid",
        ),
        (args.summary_out, "--summary-out", SUMMARY_OUT_PATH_INVALID_CODE),
        (
            args.lineage_proof_evidence,
            "--lineage-proof-evidence",
            "lineage_proof_evidence_path_invalid",
        ),
    ):
        item = _secret_looking_path_blocker(value, label=label, code=code)
        if item is not None:
            blockers.append(item)
    if not any(item["code"] == "kagemusha_repo_root_path_invalid" for item in blockers):
        repo_root_errors = validate_repo_root_path(Path(args.repo_root))
        blockers.extend(repo_root_errors)
    for index, value in enumerate(args.trusted_signer_public_keys or []):
        item = _secret_looking_path_blocker(
            value,
            label=f"--trusted-signer-public-key[{index}]",
            code="android_trusted_signer_path_invalid",
        )
        if item is not None:
            blockers.append(item)
    return blockers


def parse_utc_timestamp(value: str, label: str) -> tuple[dt.datetime | None, dict[str, Any] | None]:
    """Parse an ISO-8601 timestamp and normalize it to UTC."""

    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None, blocker(
            "timestamp_invalid",
            f"{label} must be an ISO-8601 timestamp",
        )
    if parsed.tzinfo is None:
        return None, blocker(
            "timestamp_timezone_missing",
            f"{label} must include a timezone",
        )
    return parsed.astimezone(dt.timezone.utc), None


class DuplicateJsonKeyError(ValueError):
    """Raised when a JSON object contains a duplicate key."""

    def __init__(self, key: str) -> None:
        self.key = key
        super().__init__(key)


def _display_json_key(key: str) -> str:
    return device_lab.SECRET_PATH_REDACTION if device_lab.SECRET_RE.search(key) else key


def _reject_duplicate_json_object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    item: dict[str, Any] = {}
    for key, value in pairs:
        if key in item:
            raise DuplicateJsonKeyError(key)
        item[key] = value
    return item


def _read_json_without_duplicate_keys(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=_reject_duplicate_json_object_pairs,
    )


def _duplicate_json_key_message(label: str, exc: DuplicateJsonKeyError) -> str:
    return f"{label} contains duplicate JSON object key {_display_json_key(exc.key)}"


def _load_json(path: Path) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    shape_errors = validate_release_local_json_file(path, "ABI-6 manifest")
    if shape_errors:
        missing_error = "ABI-6 manifest is missing"
        if shape_errors == [missing_error]:
            return None, [
                blocker(
                    "abi6_manifest_missing",
                    "missing ABI-6 manifest",
                )
            ]
        return None, [
            blocker(
                "abi6_manifest_file_shape",
                error,
            )
            for error in shape_errors
        ]
    try:
        data = _read_json_without_duplicate_keys(path)
    except FileNotFoundError:
        return None, [
            blocker(
                "abi6_manifest_missing",
                f"missing ABI-6 manifest at {path.as_posix()}",
            )
        ]
    except json.JSONDecodeError as exc:
        return None, [
            blocker(
                "abi6_manifest_invalid_json",
                f"ABI-6 manifest is not valid JSON: {exc}",
            )
        ]
    except DuplicateJsonKeyError as exc:
        return None, [
            blocker(
                "abi6_manifest_invalid_json",
                _duplicate_json_key_message("ABI-6 manifest", exc),
            )
        ]
    if not isinstance(data, dict):
        return None, [blocker("abi6_manifest_not_object", "ABI-6 manifest must be a JSON object")]
    return data, []


def validate_release_local_json_file(path: Path, label: str) -> list[str]:
    """Reject local release JSON files that could alias external bytes."""

    if device_lab.SECRET_RE.search(str(path)):
        return [f"{label} path must not contain secret-looking material"]
    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if release_json_ancestor_errors:
        return release_json_ancestor_errors
    if path.is_symlink():
        return [f"{label} must not be a symlink"]
    if path.exists() and not path.is_file():
        return [f"{label} must be a regular file"]
    if not path.is_file():
        return [f"{label} is missing"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    return []


def validate_repo_source_marker_file(path: Path, label: str) -> list[str]:
    """Reject checked-in marker files that could alias external bytes."""

    if device_lab.SECRET_RE.search(str(path)):
        return [f"{label} path must not contain secret-looking material"]
    errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            f"{label} ancestor directory",
        )
    ]
    if path.is_symlink():
        errors.append(f"{label} must not be a symlink")
    if path.exists() and not path.is_file():
        errors.append(f"{label} must be a regular file")
    if not path.is_file():
        errors.append(f"{label} is missing")
        return errors
    try:
        link_count = path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return errors
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")
    return errors


def _load_json_artifact(
    path: Path,
    *,
    missing_code: str,
    invalid_code: str,
    not_object_code: str,
    label: str,
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    try:
        data = _read_json_without_duplicate_keys(path)
    except FileNotFoundError:
        return None, [blocker(missing_code, f"missing {label}")]
    except json.JSONDecodeError as exc:
        return None, [blocker(invalid_code, f"{label} is not valid JSON: {exc}")]
    except DuplicateJsonKeyError as exc:
        return None, [blocker(invalid_code, _duplicate_json_key_message(label, exc))]
    if not isinstance(data, dict):
        return None, [blocker(not_object_code, f"{label} must be a JSON object")]
    return data, []


def check_abi6_reserved_lineage(repo_root: Path) -> dict[str, Any]:
    """Check the checked-in ABI-6 Reserved-lineage manifest."""

    details: dict[str, Any] = {"manifest_path": ABI6_MANIFEST_PATH}
    repo_root_blockers = validate_repo_root_path(repo_root)
    if repo_root_blockers:
        details["ok"] = False
        details["blockers"] = repo_root_blockers
        return details

    manifest_path = repo_root / ABI6_MANIFEST_PATH
    manifest, blockers = _load_json(manifest_path)
    if manifest is not None:
        details["schema"] = manifest.get("schema")
        details["bridge_abi_version"] = manifest.get("bridge_abi_version")
        details["operation_count"] = manifest.get("operation_count")
        if manifest.get("schema") != "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1":
            blockers.append(blocker("abi6_manifest_schema", "ABI-6 manifest schema mismatch"))
        if manifest.get("bridge_abi_version") != 6:
            blockers.append(
                blocker("abi6_manifest_bridge_version", "ABI-6 manifest must advertise bridge ABI 6")
            )
        operations = tuple(item.get("symbol") for item in manifest.get("operations", []))
        if manifest.get("operation_count") != len(ABI6_OPERATION_SYMBOLS):
            blockers.append(
                blocker("abi6_manifest_operation_count", "ABI-6 manifest operation count drifted")
            )
        if operations != ABI6_OPERATION_SYMBOLS:
            blockers.append(
                blocker("abi6_manifest_operations", "ABI-6 manifest operation symbols drifted")
            )
        limits = manifest.get("limits", {})
        if not isinstance(limits, dict):
            blockers.append(blocker("abi6_manifest_limits", "ABI-6 manifest limits must be an object"))
        else:
            details["limits"] = {
                key: limits.get(key) for key in sorted(EXPECTED_ABI6_LIMITS)
            }
            for key, expected in EXPECTED_ABI6_LIMITS.items():
                if limits.get(key) != expected:
                    blockers.append(
                        blocker(
                            "abi6_manifest_limit",
                            f"ABI-6 manifest limit {key} must be {expected}",
                            limit=key,
                        )
                    )
        modes = manifest.get("modes", {})
        if not isinstance(modes, dict):
            blockers.append(blocker("abi6_manifest_modes", "ABI-6 manifest modes must be an object"))
        else:
            details["modes"] = {
                "preferred_when_recursive_available": modes.get(
                    "preferred_when_recursive_available"
                ),
                "fallback_when_recursive_unavailable": modes.get(
                    "fallback_when_recursive_unavailable"
                ),
            }
            if modes.get("preferred_when_recursive_available") != "recursive_spend_v1":
                blockers.append(
                    blocker(
                        "abi6_manifest_preferred_mode",
                        "ABI-6 manifest must prefer recursive_spend_v1",
                    )
                )
            if modes.get("fallback_when_recursive_unavailable") != "checked_prefold_v1":
                blockers.append(
                    blocker(
                        "abi6_manifest_fallback_mode",
                        "ABI-6 manifest must fall back to checked_prefold_v1",
                    )
                )

    details["ok"] = not blockers
    details["blockers"] = blockers
    return details


def _is_lower_sha256_hex(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and value == value.lower()
        and value != "0" * 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_lineage_local_file(path: Path, label: str) -> list[str]:
    """Reject local lineage evidence files that could alias external bytes."""

    if device_lab.SECRET_RE.search(str(path)):
        return [f"{label} path must not contain secret-looking material"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if path.is_symlink():
        return [f"{label} must not be a symlink"]
    if path.exists() and not path.is_file():
        return [f"{label} must be a regular file"]
    if not path.is_file():
        return [f"{label} is missing"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    return []


def validate_lineage_proof_log(path: Path, expected_name: str) -> tuple[str | None, list[str]]:
    """Return the SHA-256 and content errors for a captured Reserved-lineage proof log."""

    file_errors = validate_lineage_local_file(path, "production proof log")
    if file_errors:
        if file_errors == ["production proof log is missing"]:
            return None, ["missing production proof log"]
        return None, file_errors
    try:
        if path.stat().st_size > MAX_LINEAGE_PROOF_LOG_BYTES:
            return None, [
                f"production proof log must be no more than {MAX_LINEAGE_PROOF_LOG_BYTES} bytes"
            ]
    except OSError:
        return None, ["production proof log metadata could not be read"]

    digest = _sha256_file(path)
    text = path.read_text(encoding="utf-8", errors="replace")
    errors: list[str] = []
    lines = text.splitlines()
    expected_test_line = f"test {expected_name} ... ok"
    test_lines = [
        line.rstrip()
        for line in lines
        if line.startswith("test ") and not line.startswith("test result:")
    ]
    has_expected_test_line = expected_test_line in test_lines
    if not has_expected_test_line:
        errors.append("--proof-log must contain the passing production proof test line")
    if test_lines != [expected_test_line]:
        errors.append("--proof-log must contain only the single production proof test line")

    result_lines = [line.rstrip() for line in lines if line.startswith("test result:")]
    has_expected_result_line = any(
        line.startswith(EXPECTED_LINEAGE_PROOF_RESULT_PREFIX) for line in result_lines
    )
    if not has_expected_result_line:
        errors.append("--proof-log must contain a passing cargo test result")
    if len(result_lines) != 1 or not result_lines[0].startswith(
        EXPECTED_LINEAGE_PROOF_RESULT_PREFIX
    ):
        errors.append(
            "--proof-log must contain exactly one cargo test result for one passed production test"
        )
    if any(
        marker in text
        for marker in (
            "test result: FAILED",
            "FAILED",
            "\nfailures:",
            "panicked at",
            "error: test failed",
        )
    ):
        errors.append("--proof-log must not contain cargo failure markers")
    return digest, errors


def validate_lineage_proof_command(command: Any, expected_name: str) -> list[str]:
    """Return validation errors for the production Reserved-lineage proof command."""

    if not isinstance(command, str) or not command.strip():
        return ["--command must be a non-empty string"]
    errors: list[str] = []
    expected_command = expected_lineage_proof_command(expected_name)
    expected_tokens = (
        "cargo",
        "test",
        "-p",
        "iroha_core",
        expected_name,
        "--lib",
        "--",
        "--ignored",
        "--test-threads=1",
        "--nocapture",
    )
    try:
        tokens = tuple(shlex.split(command))
    except ValueError:
        tokens = ()
        errors.append("--command must be shell-tokenizable without quoting errors")
    if tokens != expected_tokens:
        errors.append(
            "--command must exactly match the production Reserved-lineage proof command"
        )
    if command != expected_command:
        errors.append(
            "--command must exactly match the canonical production Reserved-lineage proof command string"
        )
    if KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_RUNTIME_KEYGEN_ENV in command:
        errors.append(
            "--command must not set runtime lineage keygen for the production proof run"
        )
    if device_lab.SECRET_RE.search(command):
        errors.append("--command must not contain secret-looking material")
    return errors


def _require_lineage_sha256(
    blockers: list[dict[str, Any]],
    *,
    value: Any,
    field: str,
    code: str,
) -> None:
    if not _is_lower_sha256_hex(value):
        blockers.append(
            blocker(
                code,
                f"lineage proof evidence {field} must be a non-zero lowercase sha256 hex digest",
                field=field,
            )
        )


def _display_evidence_field(field: str) -> str:
    return device_lab.SECRET_PATH_REDACTION if device_lab.SECRET_RE.search(field) else field


def check_lineage_proof_evidence(
    path: Path,
    *,
    min_generated_at: dt.datetime | None = None,
    max_generated_at: dt.datetime | None = None,
    require_canonical_filename: bool = True,
) -> dict[str, Any]:
    """Check production-width Reserved-lineage proof/keygen evidence."""

    blockers: list[dict[str, Any]] = []
    if require_canonical_filename and path.name != LINEAGE_PROOF_EVIDENCE_FILENAME:
        blockers.append(
            blocker(
                "lineage_proof_evidence_filename",
                (
                    "Reserved-lineage proof evidence file must be named "
                    f"{LINEAGE_PROOF_EVIDENCE_FILENAME}"
                ),
                expected=LINEAGE_PROOF_EVIDENCE_FILENAME,
            )
        )
    evidence_file_errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            "Reserved-lineage proof evidence ancestor directory",
        )
    ]
    for error in validate_lineage_local_file(
        path,
        "Reserved-lineage proof evidence file",
    ):
        if error != "Reserved-lineage proof evidence file is missing":
            evidence_file_errors.append(error)
    if evidence_file_errors:
        for error in evidence_file_errors:
            blockers.append(blocker("lineage_proof_evidence_file_shape", error))
        return {
            "path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "test_log_sha256": {},
            "min_generated_at_utc": (
                min_generated_at.isoformat().replace("+00:00", "Z")
                if min_generated_at is not None
                else None
            ),
            "max_generated_at_utc": (
                max_generated_at.isoformat().replace("+00:00", "Z")
                if max_generated_at is not None
                else None
            ),
            "ok": False,
            "blockers": blockers,
        }
    evidence, load_blockers = _load_json_artifact(
        path,
        missing_code="lineage_proof_evidence_missing",
        invalid_code="lineage_proof_evidence_invalid_json",
        not_object_code="lineage_proof_evidence_not_object",
        label="Reserved-lineage proof evidence",
    )
    blockers.extend(load_blockers)
    details: dict[str, Any] = {
        "path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL,
        "schema": None,
        "artifact_sha256": {},
        "test_log_sha256": {},
        "min_generated_at_utc": (
            min_generated_at.isoformat().replace("+00:00", "Z")
            if min_generated_at is not None
            else None
        ),
        "max_generated_at_utc": (
            max_generated_at.isoformat().replace("+00:00", "Z")
            if max_generated_at is not None
            else None
        ),
    }
    if evidence is None:
        details["ok"] = False
        details["blockers"] = blockers
        return details

    for field in sorted(set(evidence) - LINEAGE_PROOF_EVIDENCE_FIELDS):
        blockers.append(
            blocker(
                "lineage_proof_evidence_unexpected_field",
                "Reserved-lineage proof evidence contains unexpected field",
                field=_display_evidence_field(field),
            )
        )

    details["schema"] = evidence.get("schema")
    details["generated_at_utc"] = None
    if evidence.get("schema") != LINEAGE_PROOF_EVIDENCE_SCHEMA:
        blockers.append(
            blocker(
                "lineage_proof_evidence_schema",
                "Reserved-lineage proof evidence schema mismatch",
            )
        )

    generated_at_text = evidence.get("generated_at_utc")
    if not isinstance(generated_at_text, str) or not generated_at_text.strip():
        blockers.append(
            blocker(
                "lineage_proof_evidence_timestamp_missing",
                "Reserved-lineage proof evidence generated_at_utc is required",
            )
        )
    else:
        generated_at_raw = generated_at_text
        details["generated_at_utc"] = generated_at_raw
        if device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw) is None:
            blockers.append(
                blocker(
                    "lineage_proof_evidence_timestamp_noncanonical",
                    "Reserved-lineage proof evidence generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    generated_at_utc=generated_at_raw,
                )
            )
        generated_at, parse_blocker = parse_utc_timestamp(
            generated_at_raw,
            "Reserved-lineage proof evidence generated_at_utc",
        )
        if parse_blocker is not None:
            parse_blocker["code"] = "lineage_proof_evidence_timestamp_invalid"
            blockers.append(parse_blocker)
        elif min_generated_at is not None and generated_at is not None and generated_at < min_generated_at:
            blockers.append(
                blocker(
                    "lineage_proof_evidence_stale",
                    "Reserved-lineage proof evidence predates the required release evidence cutoff",
                    generated_at_utc=generated_at_raw,
                    min_generated_at_utc=min_generated_at.isoformat().replace("+00:00", "Z"),
                )
            )
        elif max_generated_at is not None and generated_at is not None and generated_at > max_generated_at:
            blockers.append(
                blocker(
                    "lineage_proof_evidence_future_dated",
                    "Reserved-lineage proof evidence is ahead of the release validator clock skew",
                    generated_at_utc=generated_at_raw,
                    max_generated_at_utc=max_generated_at.isoformat().replace("+00:00", "Z"),
                )
            )

    expected_scalars = {
        "opening_len": EXPECTED_LINEAGE_PROOF_OPENING_LEN,
        "ipa_k": EXPECTED_LINEAGE_PROOF_IPA_K,
    }
    for field, expected in expected_scalars.items():
        scalar_value = evidence.get(field)
        if (
            not isinstance(scalar_value, int)
            or isinstance(scalar_value, bool)
            or scalar_value != expected
        ):
            blockers.append(
                blocker(
                    f"lineage_proof_evidence_{field}",
                    f"Reserved-lineage proof evidence {field} must be integer {expected}",
                    field=field,
                )
            )
    details["opening_len"] = evidence.get("opening_len")
    details["ipa_k"] = evidence.get("ipa_k")

    for field, expected in {
        "verifier_backend": EXPECTED_LINEAGE_PROOF_BACKEND,
        "verifier_witness_profile": EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE,
        "record_archive_proof_runtime_keygen_env": "unset",
    }.items():
        if evidence.get(field) != expected:
            blockers.append(
                blocker(
                    f"lineage_proof_evidence_{field}",
                    f"Reserved-lineage proof evidence {field} mismatch",
                    field=field,
                    expected=expected,
                )
            )
    details["record_archive_proof_runtime_keygen_env"] = evidence.get(
        "record_archive_proof_runtime_keygen_env"
    )

    circuit_ids = evidence.get("circuit_ids")
    if not isinstance(circuit_ids, dict):
        blockers.append(
            blocker(
                "lineage_proof_evidence_circuit_ids",
                "Reserved-lineage proof evidence circuit_ids must be an object",
            )
        )
    else:
        for key in sorted(set(circuit_ids) - set(EXPECTED_LINEAGE_CIRCUIT_IDS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_circuit_ids_unexpected_field",
                    "Reserved-lineage proof evidence circuit_ids contains unexpected field",
                    field=_display_evidence_field(key),
                )
            )
        details["circuit_ids"] = {
            key: circuit_ids.get(key) for key in sorted(EXPECTED_LINEAGE_CIRCUIT_IDS)
        }
        for key, expected in EXPECTED_LINEAGE_CIRCUIT_IDS.items():
            if circuit_ids.get(key) != expected:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_circuit_id",
                        f"Reserved-lineage proof evidence circuit id {key} mismatch",
                        field=f"circuit_ids.{key}",
                        expected=expected,
                    )
                )

    artifacts = evidence.get("artifacts")
    artifact_count = 0
    validated_artifact_sha256: dict[str, str] = {}
    if not isinstance(artifacts, dict):
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifacts",
                "Reserved-lineage proof evidence artifacts must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifacts) - set(LINEAGE_PROOF_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_artifacts_unexpected_field",
                    "Reserved-lineage proof evidence artifacts contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
        artifact_count = len(artifacts)
        artifact_root = path.parent
        for artifact in LINEAGE_PROOF_REQUIRED_ARTIFACTS:
            expected_digest = artifacts.get(artifact)
            _require_lineage_sha256(
                blockers,
                value=expected_digest,
                field=f"artifacts.{artifact}",
                code="lineage_proof_evidence_artifact_digest",
            )
            artifact_path = artifact_root / artifact
            artifact_file_errors = validate_lineage_local_file(
                artifact_path,
                "Reserved-lineage proof evidence artifact file",
            )
            if artifact_file_errors:
                if artifact_file_errors == [
                    "Reserved-lineage proof evidence artifact file is missing"
                ]:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_missing",
                            "Reserved-lineage proof evidence artifact file is missing",
                            artifact=artifact,
                        )
                    )
                else:
                    for error in artifact_file_errors:
                        blockers.append(
                            blocker(
                                "lineage_proof_evidence_artifact_file_shape",
                                error,
                                artifact=artifact,
                            )
                        )
                continue
            if not artifact_path.is_file():
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_artifact_missing",
                        "Reserved-lineage proof evidence artifact file is missing",
                        artifact=artifact,
                    )
                )
                continue
            if _is_lower_sha256_hex(expected_digest):
                actual_digest = _sha256_file(artifact_path)
                if actual_digest != expected_digest:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_file_digest",
                            "Reserved-lineage proof evidence artifact digest does not match local artifact bytes",
                            artifact=artifact,
                        )
                    )
                else:
                    validated_artifact_sha256[artifact] = actual_digest
    details["artifact_count"] = artifact_count
    details["artifact_sha256"] = validated_artifact_sha256

    tests = evidence.get("tests")
    validated_test_log_sha256: dict[str, str] = {}
    if not isinstance(tests, dict):
        blockers.append(
            blocker(
                "lineage_proof_evidence_tests",
                "Reserved-lineage proof evidence tests must be an object",
            )
        )
    else:
        for key in sorted(set(tests) - set(LINEAGE_PROOF_REQUIRED_TESTS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_tests_unexpected_field",
                    "Reserved-lineage proof evidence tests contains unexpected field",
                    field=_display_evidence_field(key),
                )
            )
        details["tests"] = sorted(tests)
        for key, expected_name in LINEAGE_PROOF_REQUIRED_TESTS.items():
            test = tests.get(key)
            if not isinstance(test, dict):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_missing",
                        f"Reserved-lineage proof evidence test {key} is required",
                        test=key,
                    )
                )
                continue
            for field in sorted(set(test) - LINEAGE_PROOF_TEST_FIELDS):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_unexpected_field",
                        f"Reserved-lineage proof evidence test {key} contains unexpected field",
                        test=key,
                        field=_display_evidence_field(field),
                    )
                )
            if test.get("name") != expected_name:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_name",
                        f"Reserved-lineage proof evidence test {key} name mismatch",
                        test=key,
                    )
                )
            if test.get("status") != "passed":
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_status",
                        f"Reserved-lineage proof evidence test {key} must have passed",
                        test=key,
                    )
                )
            if test.get("ignored") is not True:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_ignored",
                        f"Reserved-lineage proof evidence test {key} must record ignored=true",
                        test=key,
                    )
                )
            command = test.get("command")
            command_errors = validate_lineage_proof_command(command, expected_name)
            for error in command_errors:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_command",
                        f"Reserved-lineage proof evidence test {key} command is not the production-width ignored proof run",
                        test=key,
                        issue=error,
                    )
                )
            elapsed_seconds = test.get("elapsed_seconds")
            if (
                not isinstance(elapsed_seconds, (int, float))
                or isinstance(elapsed_seconds, bool)
                or not math.isfinite(float(elapsed_seconds))
                or elapsed_seconds <= 0
            ):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_elapsed",
                        f"Reserved-lineage proof evidence test {key} elapsed_seconds must be positive",
                        test=key,
                    )
                )
            _require_lineage_sha256(
                blockers,
                value=test.get("log_sha256"),
                field=f"tests.{key}.log_sha256",
                code="lineage_proof_evidence_test_log_digest",
            )
            expected_log_path = LINEAGE_PROOF_REQUIRED_TEST_LOGS[key]
            log_path = test.get("log_path")
            if log_path != expected_log_path:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_log_path",
                        f"Reserved-lineage proof evidence test {key} log_path mismatch",
                        test=key,
                        expected=expected_log_path,
                    )
                )
                continue
            log_artifact_path = path.parent / expected_log_path
            log_file_exists = log_artifact_path.is_file()
            actual_log_digest, log_errors = validate_lineage_proof_log(
                log_artifact_path, expected_name
            )
            if actual_log_digest is None:
                blockers.append(
                    blocker(
                        (
                            "lineage_proof_evidence_test_log_unreadable"
                            if log_file_exists
                            else "lineage_proof_evidence_test_log_missing"
                        ),
                        (
                            f"Reserved-lineage proof evidence test {key} log file could not be validated"
                            if log_file_exists
                            else f"Reserved-lineage proof evidence test {key} log file is missing"
                        ),
                        test=key,
                    )
                )
            elif _is_lower_sha256_hex(test.get("log_sha256")) and actual_log_digest != test.get(
                "log_sha256"
            ):
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_log_file_digest",
                        f"Reserved-lineage proof evidence test {key} log digest does not match local log bytes",
                        test=key,
                    )
                )
            elif actual_log_digest is not None and not log_errors:
                validated_test_log_sha256[key] = actual_log_digest
            for error in log_errors:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_test_log_content",
                        f"Reserved-lineage proof evidence test {key} log content is not a passing production proof log",
                        test=key,
                        issue=error,
                    )
                )

    details["test_log_sha256"] = validated_test_log_sha256
    details["ok"] = not blockers
    details["state"] = "production_width_proof_passed" if not blockers else "blocked"
    details["blockers"] = blockers
    return details


def check_abi7_fail_closed(repo_root: Path) -> dict[str, Any]:
    """Check that ABI-7 recursive compact remains explicitly fail-closed."""

    repo_root_blockers = validate_repo_root_path(repo_root)
    if repo_root_blockers:
        return {
            "ok": False,
            "state": "unknown",
            "circuit_id": "kagemusha-recursive-compact-v1",
            "blockers": repo_root_blockers,
        }

    blockers: list[dict[str, Any]] = []
    source_texts: dict[str, str] = {}
    for relative, label in (
        ("crates/iroha_core/src/zk.rs", "ABI-7 core marker file"),
        (
            "crates/connect_norito_bridge/src/lib.rs",
            "ABI-7 bridge marker file",
        ),
    ):
        path = repo_root / relative
        file_errors = validate_repo_source_marker_file(path, label)
        if file_errors:
            for error in file_errors:
                blockers.append(
                    blocker(
                        "abi7_source_marker_file_shape",
                        error,
                        file=relative,
                    )
                )
            continue
        try:
            source_texts[relative] = path.read_text(encoding="utf-8")
        except OSError:
            blockers.append(
                blocker(
                    "abi7_source_marker_file_unreadable",
                    "ABI-7 source marker file could not be read",
                    file=relative,
                )
            )
    core_text = source_texts.get("crates/iroha_core/src/zk.rs", "")
    bridge_text = source_texts.get("crates/connect_norito_bridge/src/lib.rs", "")
    required_core_snippets = (
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "semantic ABI-7 compact tokens are disabled for production",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "pub fn verify_kagemusha_recursive_compact_payment_token(",
        "false",
    )
    for snippet in required_core_snippets:
        if snippet not in core_text:
            blockers.append(
                blocker(
                    "abi7_fail_closed_marker_missing",
                    "ABI-7 recursive compact fail-closed marker is missing",
                    marker=snippet,
                )
            )
    if "ERR_KAGEMUSHA_RECURSIVE_COMPACT_UNAVAILABLE" not in bridge_text:
        blockers.append(
            blocker(
                "abi7_bridge_unavailable_code_missing",
                "native bridge must expose recursive compact unavailable status",
            )
        )
    return {
        "ok": not blockers,
        "state": "fail_closed" if not blockers else "unknown",
        "circuit_id": "kagemusha-recursive-compact-v1",
        "blockers": blockers,
    }


def check_lineage_key_release_tooling(repo_root: Path) -> dict[str, Any]:
    """Check release-time Reserved-lineage key packages and verifier-record tooling."""

    repo_root_blockers = validate_repo_root_path(repo_root)
    if repo_root_blockers:
        return {
            "ok": False,
            "state": "unknown",
            "checked_files": [],
            "blockers": repo_root_blockers,
        }

    blockers: list[dict[str, Any]] = []
    checked_files: list[str] = []
    for relative, snippets in LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS.items():
        path = repo_root / relative
        file_errors = validate_repo_source_marker_file(
            path,
            "Reserved-lineage release-tooling marker file",
        )
        if file_errors:
            missing_error = "Reserved-lineage release-tooling marker file is missing"
            if file_errors == [missing_error]:
                blockers.append(
                    blocker(
                        "lineage_key_release_file_missing",
                        "Reserved-lineage release-tooling file is missing",
                        file=relative,
                    )
                )
            else:
                for error in file_errors:
                    blockers.append(
                        blocker(
                            "lineage_key_release_file_shape",
                            error,
                            file=relative,
                        )
                    )
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            blockers.append(
                blocker(
                    "lineage_key_release_file_missing",
                    "Reserved-lineage release-tooling file is missing",
                    file=relative,
                )
            )
            continue
        except OSError:
            blockers.append(
                blocker(
                    "lineage_key_release_file_unreadable",
                    "Reserved-lineage release-tooling file could not be read",
                    file=relative,
                )
            )
            continue
        checked_files.append(relative)
        for snippet in snippets:
            if snippet not in text:
                blockers.append(
                    blocker(
                        "lineage_key_release_marker_missing",
                        "Reserved-lineage release-tooling marker is missing",
                        file=relative,
                        marker=snippet,
                    )
                )
    return {
        "ok": not blockers,
        "state": "record_artifacts_wired" if not blockers else "unknown",
        "checked_files": checked_files,
        "blockers": blockers,
    }


def _slot_reports(
    root: Path,
    trusted_signer_public_keys: dict[str, Path],
    slot_ids: Iterable[str] | None,
) -> list[dict[str, Any]]:
    return [
        device_lab.scan_slot(
            slot_path,
            require_kagemusha_production_evidence=True,
            trusted_signer_public_keys=trusted_signer_public_keys,
        )
        for slot_path in device_lab.discover_slots(root, slot_ids)
    ]


def _redact_secret_strings(value: Any) -> tuple[Any, bool]:
    """Return a copy with secret-looking strings redacted plus a match flag."""

    if isinstance(value, str):
        if device_lab.SECRET_RE.search(value):
            return device_lab.SECRET_PATH_REDACTION, True
        return value, False
    if isinstance(value, list):
        redacted_items = []
        matched = False
        for item in value:
            redacted_item, item_matched = _redact_secret_strings(item)
            redacted_items.append(redacted_item)
            matched = matched or item_matched
        return redacted_items, matched
    if isinstance(value, dict):
        redacted: dict[Any, Any] = {}
        matched = False
        for key, item in value.items():
            redacted_key, key_matched = _redact_secret_strings(key)
            redacted_item, item_matched = _redact_secret_strings(item)
            redacted[redacted_key] = redacted_item
            matched = matched or key_matched or item_matched
        return redacted, matched
    return value, False


def _sanitize_android_reports(
    reports: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Redact report-local secret material before rollup summary serialization."""

    sanitized: list[dict[str, Any]] = []
    blockers: list[dict[str, Any]] = []
    for report in reports:
        redacted_report, matched = _redact_secret_strings(report)
        if isinstance(redacted_report, dict):
            sanitized_report = redacted_report
        else:
            sanitized_report = {"slot": "<invalid-slot-report>", "status": "error"}
            matched = True
        sanitized.append(sanitized_report)
        slot = sanitized_report.get("slot")
        if matched:
            blockers.append(
                blocker(
                    "android_device_lab_report_secret_material",
                    "Android device-lab report contains secret-looking material",
                    slot=slot if isinstance(slot, str) else "<invalid-slot-report>",
                )
            )
    return sanitized, blockers


def _check_android_signed_evidence_freshness(
    reports: list[dict[str, Any]],
    min_signed_at: dt.datetime | None,
    max_signed_at: dt.datetime | None,
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    for report in reports:
        if report.get("status") != "ok":
            continue
        slot_name = report.get("slot")
        if not isinstance(slot_name, str):
            blockers.append(
                blocker(
                    "android_device_lab_slot_name_missing",
                    "Android device-lab report is missing a slot name",
                )
            )
            continue
        signed_at_text = report.get("kagemusha", {}).get("signed_at_utc")
        if not isinstance(signed_at_text, str) or not signed_at_text:
            blockers.append(
                blocker(
                    "android_signed_evidence_timestamp_missing",
                    "validated Android device-lab report is missing signed evidence timestamp",
                    slot=slot_name,
                )
            )
            continue
        signed_at, parse_blocker = parse_utc_timestamp(
            signed_at_text,
            "signed evidence artifact signed_at_utc",
        )
        if parse_blocker is not None:
            parse_blocker["slot"] = slot_name
            parse_blocker["code"] = "android_signed_evidence_timestamp_invalid"
            blockers.append(parse_blocker)
            continue
        if min_signed_at is not None and signed_at is not None and signed_at < min_signed_at:
            blockers.append(
                blocker(
                    "android_signed_evidence_stale",
                    "signed evidence artifact predates the required release evidence cutoff",
                    slot=slot_name,
                    signed_at_utc=signed_at_text,
                    min_signed_at_utc=min_signed_at.isoformat().replace("+00:00", "Z"),
                )
            )
        if max_signed_at is not None and signed_at is not None and signed_at > max_signed_at:
            blockers.append(
                blocker(
                    "android_signed_evidence_future_dated",
                    "signed evidence artifact is ahead of the release validator clock skew",
                    slot=slot_name,
                    signed_at_utc=signed_at_text,
                    max_signed_at_utc=max_signed_at.isoformat().replace("+00:00", "Z"),
                )
            )
    return blockers


def _check_android_matrix_unique_bindings(
    reports: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Reject matrix rows copied from the same physical device run."""

    blockers: list[dict[str, Any]] = []
    checks = (
        (
            "device_fingerprint_sha256",
            "android_device_lab_duplicate_device_fingerprint",
            "Android device-lab production slots must not reuse a device fingerprint",
        ),
        (
            "attestation_challenge_sha256",
            "android_device_lab_duplicate_attestation_challenge",
            "Android device-lab production slots must not reuse an attestation challenge",
        ),
    )
    for field, code, message in checks:
        seen: dict[str, list[str]] = {}
        for report in reports:
            if report.get("status") != "ok":
                continue
            slot = report.get("slot")
            value = report.get("kagemusha", {}).get(field)
            if not isinstance(slot, str) or not isinstance(value, str) or not value:
                continue
            seen.setdefault(value, []).append(slot)
        for value, slots in sorted(seen.items()):
            if len(slots) <= 1:
                continue
            value_sha256 = (
                value
                if field.endswith("_sha256")
                else hashlib.sha256(value.encode("utf-8")).hexdigest()
            )
            blockers.append(
                blocker(
                    code,
                    message,
                    slots=sorted(slots),
                    value_sha256=value_sha256,
                )
            )
    return blockers


def _android_signed_evidence_summary(reports: list[dict[str, Any]]) -> dict[str, dict[str, str]]:
    """Return path-safe signed-evidence details for valid Android slots."""

    signed_evidence: dict[str, dict[str, str]] = {}
    for report in reports:
        if report.get("status") != "ok":
            continue
        slot = report.get("slot")
        kagemusha = report.get("kagemusha", {})
        if not isinstance(slot, str) or not isinstance(kagemusha, dict):
            continue
        entry: dict[str, str] = {}
        for source_key, target_key in (
            ("signed_at_utc", "signed_at_utc"),
            ("signed_evidence_artifact_sha256", "artifact_sha256"),
            ("signed_evidence_signer_public_key_sha256", "signer_public_key_sha256"),
        ):
            value = kagemusha.get(source_key)
            if isinstance(value, str) and value:
                entry[target_key] = value
        if entry:
            signed_evidence[slot] = entry
    return signed_evidence


def check_android_device_lab(
    root: Path,
    trusted_signer_public_keys: dict[str, Path],
    *,
    slot_ids: Iterable[str] | None = None,
    min_signed_at: dt.datetime | None = None,
    max_signed_at: dt.datetime | None = None,
) -> dict[str, Any]:
    """Check strict Android signed evidence and standard family coverage."""

    blockers: list[dict[str, Any]] = []
    validated_slot_ids, slot_id_errors = device_lab.validate_slot_ids(slot_ids)
    slot_id_blockers = [
        blocker("android_device_lab_slot_id_invalid", error) for error in slot_id_errors
    ]
    blockers.extend(slot_id_blockers)
    root_errors = device_lab.validate_device_lab_root_path(root)
    if root_errors:
        root_blockers = [
            blocker("android_device_lab_root_invalid", error) for error in root_errors
        ]
        return {
            "ok": False,
            "root": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
            "slots": [],
            "covered_device_families": [],
            "missing_device_families": list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
            "signed_evidence": {},
            "min_signed_at_utc": (
                min_signed_at.isoformat().replace("+00:00", "Z")
                if min_signed_at is not None
                else None
            ),
            "max_signed_at_utc": (
                max_signed_at.isoformat().replace("+00:00", "Z")
                if max_signed_at is not None
                else None
            ),
            "trusted_signer_public_key_sha256": sorted(trusted_signer_public_keys),
            "blockers": [*root_blockers, *slot_id_blockers],
        }
    if not root.exists():
        return {
            "ok": False,
            "root": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
            "slots": [],
            "covered_device_families": [],
            "missing_device_families": list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
            "signed_evidence": {},
            "min_signed_at_utc": (
                min_signed_at.isoformat().replace("+00:00", "Z")
                if min_signed_at is not None
                else None
            ),
            "max_signed_at_utc": (
                max_signed_at.isoformat().replace("+00:00", "Z")
                if max_signed_at is not None
                else None
            ),
            "trusted_signer_public_key_sha256": sorted(trusted_signer_public_keys),
            "blockers": [
                blocker(
                    "android_device_lab_root_missing",
                    "Android device-lab root is missing",
                ),
                *slot_id_blockers,
            ],
        }
    if not trusted_signer_public_keys:
        blockers.append(
            blocker(
                "android_trusted_signer_missing",
                "trusted signer public key is required for Kagemusha production evidence",
            )
        )

    reports, report_secret_blockers = _sanitize_android_reports(
        _slot_reports(root, trusted_signer_public_keys, validated_slot_ids)
    )
    blockers.extend(report_secret_blockers)
    if not reports:
        blockers.append(
            blocker("android_device_lab_slots_missing", "no Android device-lab slots found")
        )

    for report in reports:
        if report.get("status") != "ok":
            blockers.append(
                blocker(
                    "android_device_lab_slot_invalid",
                    f"Android device-lab slot {report.get('slot')} is invalid",
                    slot=report.get("slot"),
                    errors=report.get("errors", []),
                )
            )

    covered = sorted(
        {
            report.get("kagemusha", {}).get("device_family")
            for report in reports
            if report.get("status") == "ok"
            and report.get("kagemusha", {}).get("device_family") is not None
        }
    )
    missing = [
        family
        for family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
        if family not in covered
    ]
    if missing:
        blockers.append(
            blocker(
                "android_device_lab_standard_matrix_missing",
                "missing Kagemusha production evidence for one or more Android device families",
                missing_device_families=missing,
            )
        )
    blockers.extend(_check_android_matrix_unique_bindings(reports))
    if min_signed_at is not None or max_signed_at is not None:
        blockers.extend(
            _check_android_signed_evidence_freshness(
                reports,
                min_signed_at,
                max_signed_at,
            )
        )

    return {
        "ok": not blockers,
        "root": ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
        "slots": reports,
        "covered_device_families": covered,
        "missing_device_families": missing,
        "signed_evidence": _android_signed_evidence_summary(reports),
        "min_signed_at_utc": (
            min_signed_at.isoformat().replace("+00:00", "Z")
            if min_signed_at is not None
            else None
        ),
        "max_signed_at_utc": (
            max_signed_at.isoformat().replace("+00:00", "Z")
            if max_signed_at is not None
            else None
        ),
        "trusted_signer_public_key_sha256": sorted(trusted_signer_public_keys),
        "blockers": blockers,
    }


def build_summary(
    *,
    repo_root: Path,
    device_lab_root: Path,
    lineage_proof_evidence_path: Path,
    trusted_signer_public_keys: dict[str, Path],
    slot_ids: Iterable[str] | None = None,
    min_signed_at: dt.datetime | None = None,
    max_signed_at: dt.datetime | None = None,
    min_lineage_proof_evidence_at: dt.datetime | None = None,
    max_lineage_proof_evidence_at: dt.datetime | None = None,
) -> dict[str, Any]:
    """Build a complete Kagemusha readiness rollup."""

    abi6 = check_abi6_reserved_lineage(repo_root)
    abi7 = check_abi7_fail_closed(repo_root)
    lineage = check_lineage_key_release_tooling(repo_root)
    lineage_proof = check_lineage_proof_evidence(
        lineage_proof_evidence_path,
        min_generated_at=min_lineage_proof_evidence_at,
        max_generated_at=max_lineage_proof_evidence_at,
    )
    android = check_android_device_lab(
        device_lab_root,
        trusted_signer_public_keys,
        slot_ids=slot_ids,
        min_signed_at=min_signed_at,
        max_signed_at=max_signed_at,
    )
    all_blockers = [
        *abi6["blockers"],
        *abi7["blockers"],
        *lineage["blockers"],
        *lineage_proof["blockers"],
        *android["blockers"],
    ]
    return {
        "schema": SUMMARY_SCHEMA,
        "generated_at": utc_now(),
        "status": "ready" if not all_blockers else "blocked",
        "ready": not all_blockers,
        "blockers": all_blockers,
        "abi6_reserved_lineage": abi6,
        "abi7_recursive_compact": abi7,
        "lineage_key_release_tooling": lineage,
        "lineage_proof_evidence": lineage_proof,
        "android_device_lab": android,
    }


def validate_summary_output_path(path: Path) -> list[dict[str, Any]]:
    """Reject readiness summary output paths that could alias external files."""

    secret_blocker = _secret_looking_path_blocker(
        str(path),
        label="--summary-out",
        code=SUMMARY_OUT_PATH_INVALID_CODE,
    )
    if secret_blocker is not None:
        return [secret_blocker]
    parent = path.parent
    if parent.is_symlink():
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent directory must not be a symlink",
            )
        ]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        "--summary-out ancestor directory",
    )
    if ancestor_errors:
        return [
            blocker(SUMMARY_OUT_PATH_INVALID_CODE, error)
            for error in ancestor_errors
        ]
    if parent.exists():
        if not parent.is_dir():
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out parent must be a directory",
                )
            ]
    else:
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out parent directory could not be created",
                )
            ]
    if path.exists():
        if path.is_symlink():
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out must not be a symlink",
                )
            ]
        if not path.is_file():
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out must be a regular file",
                )
            ]
        try:
            link_count = path.stat().st_nlink
        except OSError:
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out hardlink metadata could not be read",
                )
            ]
        if link_count > 1:
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out must not be hardlinked",
                )
            ]
    return []


def write_summary(path: Path, summary: dict[str, Any]) -> list[dict[str, Any]]:
    """Write a readiness summary JSON file."""

    errors = validate_summary_output_path(path)
    if errors:
        return errors
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Roll up strict Kagemusha production-readiness evidence."
    )
    parser.add_argument(
        "--repo-root",
        default=str(SCRIPT_DIR.parent),
        help="Repository root used for checked-in Kagemusha release guards.",
    )
    parser.add_argument(
        "--device-lab-root",
        default="artifacts/android/device_lab",
        help="Android device-lab root containing production slots.",
    )
    parser.add_argument(
        "--lineage-proof-evidence",
        default=DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH,
        help="Reserved-lineage production proof/keygen evidence JSON.",
    )
    parser.add_argument(
        "--slot",
        action="append",
        dest="slots",
        default=None,
        help="Specific Android device-lab slot id(s) to include.",
    )
    parser.add_argument(
        "--trusted-signer-public-key",
        action="append",
        dest="trusted_signer_public_keys",
        default=None,
        help="PEM public key for a trusted Android lab evidence signer.",
    )
    parser.add_argument(
        "--min-signed-at-utc",
        default=DEFAULT_MIN_SIGNED_AT_UTC,
        help=(
            "Minimum signed_at_utc timestamp accepted for Android lab evidence. "
            "Use an empty value to disable the freshness gate."
        ),
    )
    parser.add_argument(
        "--max-signed-at-future-skew-seconds",
        type=int,
        default=DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds signed_at_utc may be ahead of the "
            "readiness validator clock."
        ),
    )
    parser.add_argument(
        "--min-lineage-proof-evidence-at-utc",
        default=DEFAULT_MIN_SIGNED_AT_UTC,
        help=(
            "Minimum generated_at_utc timestamp accepted for Reserved-lineage proof evidence. "
            "Use an empty value to disable the freshness gate."
        ),
    )
    parser.add_argument(
        "--max-lineage-proof-evidence-future-skew-seconds",
        type=int,
        default=DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds Reserved-lineage proof evidence generated_at_utc "
            "may be ahead of the readiness validator clock."
        ),
    )
    parser.add_argument("--summary-out", default=None, help="Optional JSON summary path.")
    args = parser.parse_args(argv)

    path_blockers = validate_cli_path_arguments(args)
    if path_blockers:
        summary = {
            "schema": SUMMARY_SCHEMA,
            "generated_at": utc_now(),
            "status": "blocked",
            "ready": False,
            "blockers": path_blockers,
        }
    else:
        repo_root = Path(args.repo_root).resolve()
        device_lab_root = Path(args.device_lab_root)
        if not device_lab_root.is_absolute():
            device_lab_root = repo_root / device_lab_root
        lineage_proof_evidence_path = Path(args.lineage_proof_evidence)
        if not lineage_proof_evidence_path.is_absolute():
            lineage_proof_evidence_path = repo_root / lineage_proof_evidence_path
        trusted, signer_errors = device_lab.load_trusted_signer_public_keys(
            args.trusted_signer_public_keys
        )
        min_signed_at = None
        if args.min_signed_at_utc:
            min_signed_at, min_signed_at_blocker = parse_utc_timestamp(
                args.min_signed_at_utc,
                "--min-signed-at-utc",
            )
        else:
            min_signed_at_blocker = None
        min_lineage_proof_evidence_at = None
        if args.min_lineage_proof_evidence_at_utc:
            (
                min_lineage_proof_evidence_at,
                min_lineage_proof_evidence_at_blocker,
            ) = parse_utc_timestamp(
                args.min_lineage_proof_evidence_at_utc,
                "--min-lineage-proof-evidence-at-utc",
            )
        else:
            min_lineage_proof_evidence_at_blocker = None
        max_signed_at = None
        max_signed_at_blocker = None
        if args.max_signed_at_future_skew_seconds < 0:
            max_signed_at_blocker = blocker(
                "android_max_signed_at_invalid",
                "--max-signed-at-future-skew-seconds must be non-negative",
            )
        else:
            max_signed_at = (
                dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                + dt.timedelta(seconds=args.max_signed_at_future_skew_seconds)
            )
        max_lineage_proof_evidence_at = None
        max_lineage_proof_evidence_at_blocker = None
        if args.max_lineage_proof_evidence_future_skew_seconds < 0:
            max_lineage_proof_evidence_at_blocker = blocker(
                "lineage_proof_evidence_max_timestamp_invalid",
                "--max-lineage-proof-evidence-future-skew-seconds must be non-negative",
            )
        else:
            max_lineage_proof_evidence_at = (
                dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                + dt.timedelta(seconds=args.max_lineage_proof_evidence_future_skew_seconds)
            )
        if (
            signer_errors
            or min_signed_at_blocker is not None
            or min_lineage_proof_evidence_at_blocker is not None
            or max_signed_at_blocker is not None
            or max_lineage_proof_evidence_at_blocker is not None
        ):
            blockers = [
                blocker("android_trusted_signer_invalid", error) for error in signer_errors
            ]
            if min_signed_at_blocker is not None:
                min_signed_at_blocker["code"] = "android_min_signed_at_invalid"
                blockers.append(min_signed_at_blocker)
            if min_lineage_proof_evidence_at_blocker is not None:
                min_lineage_proof_evidence_at_blocker["code"] = (
                    "lineage_proof_evidence_min_timestamp_invalid"
                )
                blockers.append(min_lineage_proof_evidence_at_blocker)
            if max_signed_at_blocker is not None:
                blockers.append(max_signed_at_blocker)
            if max_lineage_proof_evidence_at_blocker is not None:
                blockers.append(max_lineage_proof_evidence_at_blocker)
            summary = {
                "schema": SUMMARY_SCHEMA,
                "generated_at": utc_now(),
                "status": "blocked",
                "ready": False,
                "blockers": blockers,
            }
        else:
            summary = build_summary(
                repo_root=repo_root,
                device_lab_root=device_lab_root,
                lineage_proof_evidence_path=lineage_proof_evidence_path,
                trusted_signer_public_keys=trusted,
                slot_ids=args.slots,
                min_signed_at=min_signed_at,
                max_signed_at=max_signed_at,
                min_lineage_proof_evidence_at=min_lineage_proof_evidence_at,
                max_lineage_proof_evidence_at=max_lineage_proof_evidence_at,
            )

    summary_out_invalid = any(
        item["code"] == SUMMARY_OUT_PATH_INVALID_CODE for item in path_blockers
    )
    if args.summary_out and not summary_out_invalid:
        write_blockers = write_summary(Path(args.summary_out), summary)
        if write_blockers:
            summary["ready"] = False
            summary["status"] = "blocked"
            summary["blockers"].extend(write_blockers)
        else:
            print("[kagemusha-readiness] wrote summary")

    if summary["ready"]:
        print("[kagemusha-readiness] ready")
        return 0
    for item in summary["blockers"]:
        print(
            f"[kagemusha-readiness] blocked: {item['code']}: {item['message']}",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
