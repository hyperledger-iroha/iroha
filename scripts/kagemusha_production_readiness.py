"""Roll up Kagemusha production-readiness evidence into a strict summary."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shlex
import stat
import sys
import tempfile
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
COMPACT_KEY_EVIDENCE_SCHEMA = "iroha.kagemusha.recursive_compact_key_evidence.v1"
COMPACT_KEY_EVIDENCE_FILENAME = "recursive-compact-key-evidence.json"
DEFAULT_COMPACT_KEY_EVIDENCE_PATH = f"artifacts/kagemusha/{COMPACT_KEY_EVIDENCE_FILENAME}"
COMPACT_KEY_GENERATOR_LOG_FILENAME = "recursive-compact-key-artifacts.log"
DEFAULT_MIN_SIGNED_AT_UTC = "2026-06-06T00:00:00Z"
DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS = 300
ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"
LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL = "<lineage-proof-evidence>"
COMPACT_KEY_EVIDENCE_SUMMARY_LABEL = "<recursive-compact-key-evidence>"
MAX_ABI6_MANIFEST_JSON_BYTES = 1024 * 1024
MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES = 16 * 1024 * 1024
MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES = 16 * 1024 * 1024
EXPECTED_LINEAGE_PROOF_OPENING_LEN = 128
EXPECTED_LINEAGE_PROOF_IPA_K = 8
EXPECTED_LINEAGE_PROOF_BACKEND = "halo2/ipa"
EXPECTED_COMPACT_KEY_OPENING_LEN = 4
EXPECTED_COMPACT_KEY_IPA_K = 8
EXPECTED_COMPACT_KEY_BACKEND = "halo2/ipa"
EXPECTED_COMPACT_KEY_CIRCUIT_ID = "kagemusha-recursive-compact-v1"
EXPECTED_COMPACT_KEY_RECORD_NAMESPACE = "offline_kagemusha"
EXPECTED_COMPACT_KEY_RECORD_VERSION = 1
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
LINEAGE_PROOF_RESULT_RE = re.compile(
    r"^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
    r"0 filtered out; finished in [0-9]+(?:\.[0-9]+)?s$"
)
LINEAGE_ARTIFACT_ALL_ZERO_ERROR = (
    "must be generated lineage material, not all-zero placeholder bytes"
)
COMPACT_KEY_REQUIRED_ARTIFACTS = (
    "recursive-compact-len4.vk",
    "recursive-compact-len4.pk",
    "recursive-compact-key-artifacts.norito",
    "recursive-compact-verifier-keys.norito",
    "recursive-compact-len4.record.norito",
)
COMPACT_KEY_PLACEHOLDER_PREFIXES = (
    b"recursive compact key artifact ",
    b"dummy recursive compact key ",
    b"placeholder recursive compact key ",
    b"test recursive compact key ",
)
COMPACT_KEY_PLACEHOLDER_ERROR = "must be generated key material, not a placeholder fixture"
COMPACT_KEY_ALL_ZERO_ERROR = "must be generated key material, not all-zero placeholder bytes"
MAX_COMPACT_KEY_GENERATOR_LOG_BYTES = 1024 * 1024
COMPACT_KEY_GENERATOR_LOG_SIZE_FIELDS = {
    "recursive-compact-len4.vk": "vk",
    "recursive-compact-len4.pk": "pk",
    "recursive-compact-key-artifacts.norito": "key_artifacts",
    "recursive-compact-verifier-keys.norito": "verifier_keys",
    "recursive-compact-len4.record.norito": "record",
}
COMPACT_KEY_GENERATOR_LOG_DIGEST_FIELDS = {
    artifact: f"{field}_sha256"
    for artifact, field in COMPACT_KEY_GENERATOR_LOG_SIZE_FIELDS.items()
}
COMPACT_KEY_GENERATOR_LOG_RE = re.compile(
    r"^Wrote ABI-7 recursive compact key artifacts for "
    r"`kagemusha-recursive-compact-v1` opening_len=4 to "
    r"artifacts/kagemusha/recursive-compact-len4\.vk and "
    r"artifacts/kagemusha/recursive-compact-len4\.pk "
    r"\(vk=(?P<vk>[1-9][0-9]*) bytes sha256=(?P<vk_sha256>[0-9a-f]{64}), "
    r"pk=(?P<pk>[1-9][0-9]*) bytes sha256=(?P<pk_sha256>[0-9a-f]{64}), "
    r"record=(?P<record>[1-9][0-9]*) bytes sha256=(?P<record_sha256>[0-9a-f]{64}), "
    r"key_artifacts=(?P<key_artifacts>[1-9][0-9]*) bytes sha256=(?P<key_artifacts_sha256>[0-9a-f]{64}), "
    r"verifier_keys=(?P<verifier_keys>[1-9][0-9]*) bytes sha256=(?P<verifier_keys_sha256>[0-9a-f]{64})\)$"
)


def expected_lineage_proof_command(expected_name: str) -> str:
    """Return the canonical production Reserved-lineage proof command string."""

    return (
        "cargo test -p iroha_core "
        f"{expected_name} "
        "--lib -- --ignored --test-threads=1 --nocapture"
    )


def expected_compact_key_command() -> str:
    """Return the canonical ABI-7 recursive compact key-artifact command."""

    return (
        "iroha app zk kagemusha recursive-compact-key-artifacts "
        "--vk-out artifacts/kagemusha/recursive-compact-len4.vk "
        "--pk-out artifacts/kagemusha/recursive-compact-len4.pk "
        "--key-artifacts-out artifacts/kagemusha/recursive-compact-key-artifacts.norito "
        "--verifier-keys-out artifacts/kagemusha/recursive-compact-verifier-keys.norito "
        "--record-out artifacts/kagemusha/recursive-compact-len4.record.norito "
        "--record-namespace offline_kagemusha "
        "--record-version 1"
    )


def expected_compact_key_generator_log_line(
    artifact_size_bytes: dict[str, int],
    artifact_sha256: dict[str, str],
) -> str:
    """Return the canonical ABI-7 recursive compact key generator summary line."""

    return (
        "Wrote ABI-7 recursive compact key artifacts for "
        "`kagemusha-recursive-compact-v1` opening_len=4 to "
        "artifacts/kagemusha/recursive-compact-len4.vk and "
        "artifacts/kagemusha/recursive-compact-len4.pk "
        f"(vk={artifact_size_bytes['recursive-compact-len4.vk']} bytes "
        f"sha256={artifact_sha256['recursive-compact-len4.vk']}, "
        f"pk={artifact_size_bytes['recursive-compact-len4.pk']} bytes "
        f"sha256={artifact_sha256['recursive-compact-len4.pk']}, "
        f"record={artifact_size_bytes['recursive-compact-len4.record.norito']} bytes "
        f"sha256={artifact_sha256['recursive-compact-len4.record.norito']}, "
        f"key_artifacts={artifact_size_bytes['recursive-compact-key-artifacts.norito']} bytes "
        f"sha256={artifact_sha256['recursive-compact-key-artifacts.norito']}, "
        f"verifier_keys={artifact_size_bytes['recursive-compact-verifier-keys.norito']} bytes "
        f"sha256={artifact_sha256['recursive-compact-verifier-keys.norito']})"
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
        "artifact_size_bytes",
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
COMPACT_KEY_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        "generated_at_utc",
        "opening_len",
        "ipa_k",
        "verifier_backend",
        "circuit_id",
        "record_namespace",
        "record_version",
        "command",
        "generator_log_path",
        "generator_log_sha256",
        "artifacts",
        "artifact_size_bytes",
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
        "KagemushaCommand::RecursiveCompactKeyArtifacts",
        "KagemushaCommand::LineageRecord",
        "KagemushaRecursiveCompactKeyArtifactsArgs",
        "KagemushaLineageRecordArgs",
        "derive_halo2_ipa_kagemusha_recursive_compact_payment_token_proving_key_bytes",
        "kagemusha_recursive_compact_payment_token_vk_record_from_box",
        "record_out: Option<std::path::PathBuf>",
        "record_namespace: String",
        "record_version: u32",
        "kagemusha_lineage_vk_record_from_bytes",
        "std::fs::read(&self.vk)",
        "kagemusha_recursive_spend_lineage_vk_record_from_box(",
        "kagemusha_recursive_spend_lineage_append_vk_record_from_box(",
        "kagemusha_lineage_record_run_writes_norito_record_from_existing_vk_file",
        'record_summary = format!(", record={} bytes", record_bytes.len())',
        "Generating {} Reserved-lineage verifier key for `{}` opening_len={}",
        "Writing {} Reserved-lineage verifier key to {}",
        "Writing {} Reserved-lineage verifier record to {}",
        "Deriving {} Reserved-lineage proving key archive for `{}` opening_len={}",
        "Writing {} Reserved-lineage proving key archive to {}",
        "Writing {} Reserved-lineage key package to {}",
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
        "iroha app zk kagemusha recursive-compact-key-artifacts",
        "--record-out artifacts/kagemusha/recursive-compact-len4.record.norito",
        "--pk-out artifacts/kagemusha/recursive-compact-len4.pk",
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
    try:
        root_mode = root.lstat().st_mode
    except FileNotFoundError:
        root_mode = None
    except OSError:
        errors.append("--repo-root metadata could not be read")
        return [
            blocker("kagemusha_repo_root_path_invalid", error)
            for error in errors
        ]
    if root_mode is not None and stat.S_ISLNK(root_mode):
        errors.append("--repo-root must not be a symlink")
    errors.extend(
        device_lab.validate_no_symlink_ancestors(
            root,
            "--repo-root ancestor directory",
        )
    )
    if root_mode is not None and not stat.S_ISDIR(root_mode):
        errors.append("--repo-root must be a directory")
    if root_mode is None:
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
        (
            args.compact_key_evidence,
            "--compact-key-evidence",
            "compact_key_evidence_path_invalid",
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


class NonFiniteJsonConstantError(ValueError):
    """Raised when release evidence JSON uses non-standard numeric constants."""

    def __init__(self, constant: str) -> None:
        self.constant = constant
        super().__init__(constant)


def _display_json_key(key: str) -> str:
    return device_lab.SECRET_PATH_REDACTION if device_lab.SECRET_RE.search(key) else key


def _reject_duplicate_json_object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    item: dict[str, Any] = {}
    for key, value in pairs:
        if key in item:
            raise DuplicateJsonKeyError(key)
        item[key] = value
    return item


def _reject_nonfinite_json_constant(constant: str) -> None:
    raise NonFiniteJsonConstantError(constant)


def _read_json_without_duplicate_keys(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=_reject_duplicate_json_object_pairs,
        parse_constant=_reject_nonfinite_json_constant,
    )


def _read_release_json_text(
    path: Path,
    label: str,
    *,
    missing_code: str,
    shape_code: str,
    unreadable_code: str,
) -> tuple[str | None, list[dict[str, Any]]]:
    expected_stat, shape_errors = _validate_release_local_json_file_for_read(path, label)
    if shape_errors:
        missing_error = f"{label} is missing"
        if shape_errors == [missing_error]:
            return None, [blocker(missing_code, f"missing {label}")]
        return None, [blocker(shape_code, error) for error in shape_errors]
    assert expected_stat is not None
    chunks: list[bytes] = []
    size = 0
    release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            release_json_path_stat = path.lstat()
            if stat.S_ISLNK(release_json_path_stat.st_mode):
                return None, [blocker(shape_code, f"{label} must not be a symlink")]
            if not stat.S_ISREG(release_json_path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [blocker(shape_code, f"{label} must be a regular file")]
            release_json_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if release_json_open_identity != release_json_expected_identity or (
                release_json_path_stat.st_dev,
                release_json_path_stat.st_ino,
            ) != release_json_expected_identity:
                return None, [blocker(shape_code, f"{label} changed while being read")]
            if open_stat.st_nlink > 1:
                return None, [blocker(shape_code, f"{label} must not be hardlinked")]
            if open_stat.st_size > MAX_ABI6_MANIFEST_JSON_BYTES:
                return None, [
                    blocker(
                        shape_code,
                        f"{label} must be no more than {MAX_ABI6_MANIFEST_JSON_BYTES} bytes",
                    )
                ]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_ABI6_MANIFEST_JSON_BYTES:
                    return None, [
                        blocker(
                            shape_code,
                            f"{label} must be no more than {MAX_ABI6_MANIFEST_JSON_BYTES} bytes",
                        )
                    ]
                chunks.append(chunk)
            release_json_final_path_stat = path.lstat()
            if (
                release_json_final_path_stat.st_dev,
                release_json_final_path_stat.st_ino,
            ) != release_json_expected_identity:
                return None, [blocker(shape_code, f"{label} changed while being read")]
    except OSError:
        return None, [blocker(unreadable_code, f"{label} could not be read")]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [blocker(unreadable_code, f"{label} could not be read")]


def _duplicate_json_key_message(label: str, exc: DuplicateJsonKeyError) -> str:
    return f"{label} contains duplicate JSON object key {_display_json_key(exc.key)}"


def _load_json(path: Path) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    text, read_blockers = _read_release_json_text(
        path,
        "ABI-6 manifest",
        missing_code="abi6_manifest_missing",
        shape_code="abi6_manifest_file_shape",
        unreadable_code="abi6_manifest_unreadable",
    )
    if read_blockers:
        return None, read_blockers
    assert text is not None
    try:
        data = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_object_pairs,
            parse_constant=_reject_nonfinite_json_constant,
        )
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
    except NonFiniteJsonConstantError as exc:
        return None, [
            blocker(
                "abi6_manifest_invalid_json",
                f"ABI-6 manifest is not strict JSON: non-finite constant {exc.constant} is not allowed",
            )
        ]
    if not isinstance(data, dict):
        return None, [blocker("abi6_manifest_not_object", "ABI-6 manifest must be a JSON object")]
    return data, []


def validate_release_local_json_file(path: Path, label: str) -> list[str]:
    """Reject local release JSON files that could alias external bytes."""

    _file_stat, errors = _validate_release_local_json_file_for_read(path, label)
    return errors


def _validate_release_local_json_file_for_read(
    path: Path,
    label: str,
) -> tuple[os.stat_result | None, list[str]]:
    """Reject local release JSON files and return the read identity."""

    if device_lab.SECRET_RE.search(str(path)):
        return None, [f"{label} path must not contain secret-looking material"]
    release_json_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if release_json_ancestor_errors:
        return None, release_json_ancestor_errors
    try:
        file_stat = path.lstat()
    except FileNotFoundError:
        return None, [f"{label} is missing"]
    except OSError:
        return None, [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(file_stat.st_mode):
        return None, [f"{label} must not be a symlink"]
    if not stat.S_ISREG(file_stat.st_mode):
        return None, [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return None, [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return None, [f"{label} must not be hardlinked"]
    return file_stat, []


def _validate_repo_source_marker_file_for_read(
    path: Path,
    label: str,
) -> tuple[os.stat_result | None, list[str]]:
    """Reject checked-in marker files that could alias external bytes."""

    if device_lab.SECRET_RE.search(str(path)):
        return None, [f"{label} path must not contain secret-looking material"]
    errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            f"{label} ancestor directory",
        )
    ]
    try:
        file_stat = path.lstat()
    except FileNotFoundError:
        errors.append(f"{label} is missing")
        return None, errors
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return None, errors
    if stat.S_ISLNK(file_stat.st_mode):
        errors.append(f"{label} must not be a symlink")
        return None, errors
    if not stat.S_ISREG(file_stat.st_mode):
        errors.append(f"{label} must be a regular file")
        return None, errors
    try:
        link_count = path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return None, errors
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")
    if errors:
        return None, errors
    return file_stat, []


def validate_repo_source_marker_file(path: Path, label: str) -> list[str]:
    """Reject checked-in marker files that could alias external bytes."""

    _file_stat, errors = _validate_repo_source_marker_file_for_read(path, label)
    return errors


def _repo_source_marker_text(
    path: Path,
    label: str,
    unreadable_error: str,
) -> tuple[str | None, list[str]]:
    """Validate a checked-in source marker immediately before reading text."""

    expected_stat, file_errors = _validate_repo_source_marker_file_for_read(path, label)
    if file_errors:
        return None, file_errors
    assert expected_stat is not None
    expected_marker_identity = (expected_stat.st_dev, expected_stat.st_ino)
    chunks: list[bytes] = []
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            marker_path_stat = path.lstat()
            if stat.S_ISLNK(marker_path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(marker_path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [f"{label} must be a regular file"]
            open_marker_identity = (open_stat.st_dev, open_stat.st_ino)
            if open_marker_identity != expected_marker_identity or (
                marker_path_stat.st_dev,
                marker_path_stat.st_ino,
            ) != expected_marker_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                chunks.append(chunk)
            marker_final_path_stat = path.lstat()
            if (marker_final_path_stat.st_dev, marker_final_path_stat.st_ino) != (
                expected_marker_identity
            ):
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [unreadable_error]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [unreadable_error]


def _load_json_artifact(
    path: Path,
    *,
    missing_code: str,
    invalid_code: str,
    unreadable_code: str,
    shape_code: str,
    not_object_code: str,
    label: str,
    max_bytes: int | None = None,
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    size_error = (
        f"{label} must be no more than {max_bytes} bytes"
        if max_bytes is not None
        else None
    )
    digest, text, read_errors = _sha256_text_file(
        path,
        label,
        f"{label} could not be read",
        max_bytes=max_bytes,
        too_large_error=size_error,
    )
    if read_errors:
        blockers: list[dict[str, Any]] = []
        missing_error = f"{label} is missing"
        unreadable_error = f"{label} could not be read"
        for error in read_errors:
            if error == missing_error:
                blockers.append(blocker(missing_code, f"missing {label}"))
            elif error == unreadable_error:
                blockers.append(blocker(unreadable_code, error))
            else:
                blockers.append(blocker(shape_code, error))
        return None, blockers
    assert digest is not None and text is not None
    try:
        data = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_object_pairs,
            parse_constant=_reject_nonfinite_json_constant,
        )
    except json.JSONDecodeError as exc:
        return None, [blocker(invalid_code, f"{label} is not valid JSON: {exc}")]
    except DuplicateJsonKeyError as exc:
        return None, [blocker(invalid_code, _duplicate_json_key_message(label, exc))]
    except NonFiniteJsonConstantError as exc:
        return None, [
            blocker(
                invalid_code,
                f"{label} is not strict JSON: non-finite constant {exc.constant} is not allowed",
            )
        ]
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


def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:
    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)
    if file_errors:
        return None, file_errors
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    return digest.hexdigest(), []


def _sha256_file_with_size(
    path: Path,
    label: str,
    *,
    allow_empty: bool = False,
) -> tuple[str | None, int | None, list[str]]:
    digest, size, _prefix, errors = _sha256_file_with_size_and_prefix(
        path,
        label,
        allow_empty=allow_empty,
    )
    return digest, size, errors


def _sha256_file_with_size_and_prefix(
    path: Path,
    label: str,
    *,
    allow_empty: bool = False,
    prefix_len: int = 4096,
) -> tuple[str | None, int | None, bytes | None, list[str]]:
    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)
    if file_errors:
        return None, None, None, file_errors
    digest = hashlib.sha256()
    prefix_parts: list[bytes] = []
    prefix_remaining = prefix_len
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, None, None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, None, None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, None, None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if prefix_remaining > 0:
                    prefix_parts.append(chunk[:prefix_remaining])
                    prefix_remaining -= min(prefix_remaining, len(chunk))
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
    except OSError:
        return None, None, None, [f"{label} could not be read"]
    if size <= 0 and not allow_empty:
        return None, None, None, [f"{label} must be non-empty"]
    return digest.hexdigest(), size, b"".join(prefix_parts), []


def _validate_lineage_local_file_for_read(
    path: Path,
    label: str,
) -> tuple[os.stat_result | None, list[str]]:
    """Reject local lineage evidence files that could alias external bytes."""

    if device_lab.SECRET_RE.search(str(path)):
        return None, [f"{label} path must not contain secret-looking material"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return None, ancestor_errors
    try:
        file_stat = path.lstat()
    except FileNotFoundError:
        return None, [f"{label} is missing"]
    except OSError:
        return None, [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(file_stat.st_mode):
        return None, [f"{label} must not be a symlink"]
    if not stat.S_ISREG(file_stat.st_mode):
        return None, [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return None, [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return None, [f"{label} must not be hardlinked"]
    return file_stat, []


def validate_lineage_local_file(path: Path, label: str) -> list[str]:
    """Reject local lineage evidence files that could alias external bytes."""

    _file_stat, errors = _validate_lineage_local_file_for_read(path, label)
    return errors


def _lineage_local_text(
    path: Path,
    label: str,
    unreadable_error: str,
    *,
    decode_errors: str = "strict",
) -> tuple[str | None, list[str]]:
    """Validate a local lineage file immediately before reading text."""

    _digest, text, errors = _sha256_text_file(
        path,
        label,
        unreadable_error,
        decode_errors=decode_errors,
    )
    return text, errors


def _sha256_text_file(
    path: Path,
    label: str,
    unreadable_error: str,
    *,
    max_bytes: int | None = None,
    too_large_error: str | None = None,
    decode_errors: str = "strict",
) -> tuple[str | None, str | None, list[str]]:
    """Return a digest and decoded text from one opened, path-bound file."""

    expected_stat, file_errors = _validate_lineage_local_file_for_read(path, label)
    if file_errors:
        return None, None, file_errors
    digest = hashlib.sha256()
    chunks: list[bytes] = []
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, None, [f"{label} must not be hardlinked"]
            if max_bytes is not None and open_stat.st_size > max_bytes:
                return None, None, [
                    too_large_error
                    if too_large_error is not None
                    else f"{label} must be no more than {max_bytes} bytes"
                ]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if max_bytes is not None and size > max_bytes:
                    return None, None, [
                        too_large_error
                        if too_large_error is not None
                        else f"{label} must be no more than {max_bytes} bytes"
                    ]
                chunks.append(chunk)
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, None, [f"{label} changed while being read"]
    except OSError:
        return None, None, [unreadable_error]
    try:
        text = b"".join(chunks).decode("utf-8", errors=decode_errors)
    except UnicodeDecodeError:
        return None, None, [unreadable_error]
    return digest.hexdigest(), text, []


def validate_lineage_proof_log(path: Path, expected_name: str) -> tuple[str | None, list[str]]:
    """Return the SHA-256 and content errors for a captured Reserved-lineage proof log."""

    file_errors = validate_lineage_local_file(path, "production proof log")
    if file_errors:
        if file_errors == ["production proof log is missing"]:
            return None, ["missing production proof log"]
        return None, file_errors

    size_error = f"production proof log must be no more than {MAX_LINEAGE_PROOF_LOG_BYTES} bytes"
    digest, text, read_errors = _sha256_text_file(
        path,
        "production proof log",
        "production proof log could not be read",
        max_bytes=MAX_LINEAGE_PROOF_LOG_BYTES,
        too_large_error=size_error,
    )
    if read_errors:
        return None, read_errors
    assert digest is not None and text is not None
    errors: list[str] = []
    if "\r" in text:
        errors.append("--proof-log must use canonical LF line endings")
    if not text.endswith("\n"):
        errors.append("--proof-log must end with a canonical LF line terminator")
    lines = text.splitlines()
    expected_test_line = f"test {expected_name} ... ok"
    test_lines = [
        line
        for line in lines
        if line.startswith("test ") and not line.startswith("test result:")
    ]
    has_expected_test_line = expected_test_line in test_lines
    if not has_expected_test_line:
        errors.append("--proof-log must contain the passing production proof test line")
    if test_lines != [expected_test_line]:
        errors.append("--proof-log must contain only the single production proof test line")

    result_lines = [line for line in lines if line.startswith("test result:")]
    has_expected_result_line = any(
        LINEAGE_PROOF_RESULT_RE.fullmatch(line) for line in result_lines
    )
    if not has_expected_result_line:
        errors.append("--proof-log must contain a passing cargo test result")
    if (
        len(result_lines) != 1
        or LINEAGE_PROOF_RESULT_RE.fullmatch(result_lines[0]) is None
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


def _rust_function_body(source: str, signature: str) -> str | None:
    """Return the Rust function body following `signature`, ignoring braces in strings."""

    start = source.find(signature)
    if start < 0:
        return None
    brace_start = source.find("{", start)
    if brace_start < 0:
        return None

    depth = 0
    index = brace_start
    in_line_comment = False
    in_block_comment = False
    in_string = False
    raw_string_hashes: int | None = None
    in_char = False
    escaped = False
    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""

        if in_line_comment:
            if char == "\n":
                in_line_comment = False
            index += 1
            continue
        if in_block_comment:
            if char == "*" and next_char == "/":
                in_block_comment = False
                index += 2
            else:
                index += 1
            continue
        if raw_string_hashes is not None:
            if char == '"' and source.startswith("#" * raw_string_hashes, index + 1):
                index += 1 + raw_string_hashes
                raw_string_hashes = None
            else:
                index += 1
            continue
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue
        if in_char:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == "'":
                in_char = False
            index += 1
            continue

        if char == "/" and next_char == "/":
            in_line_comment = True
            index += 2
            continue
        if char == "/" and next_char == "*":
            in_block_comment = True
            index += 2
            continue
        raw_prefix_len = 0
        if char == "r":
            raw_prefix_len = 1
        elif char == "b" and next_char == "r":
            raw_prefix_len = 2
        if raw_prefix_len:
            raw_index = index + raw_prefix_len
            raw_hashes = 0
            while raw_index < len(source) and source[raw_index] == "#":
                raw_hashes += 1
                raw_index += 1
            if raw_index < len(source) and source[raw_index] == '"':
                raw_string_hashes = raw_hashes
                index = raw_index + 1
                continue
        if char == '"':
            in_string = True
            index += 1
            continue
        if char == "'" and not (next_char.isalpha() or next_char == "_"):
            in_char = True
            index += 1
            continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[brace_start : index + 1]
        index += 1
    return None


def _require_rust_function_contract(
    source: str, signature: str, snippets: Iterable[str]
) -> list[str]:
    """Return missing snippets from a Rust function contract."""

    body = _rust_function_body(source, signature)
    if body is None:
        return [signature]
    return [snippet for snippet in snippets if snippet not in body]


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


def validate_compact_key_command(command: Any) -> list[str]:
    """Return validation errors for the ABI-7 recursive compact keygen command."""

    if not isinstance(command, str) or not command.strip():
        return ["--command must be a non-empty string"]
    errors: list[str] = []
    expected_command = expected_compact_key_command()
    expected_tokens = (
        "iroha",
        "app",
        "zk",
        "kagemusha",
        "recursive-compact-key-artifacts",
        "--vk-out",
        "artifacts/kagemusha/recursive-compact-len4.vk",
        "--pk-out",
        "artifacts/kagemusha/recursive-compact-len4.pk",
        "--key-artifacts-out",
        "artifacts/kagemusha/recursive-compact-key-artifacts.norito",
        "--verifier-keys-out",
        "artifacts/kagemusha/recursive-compact-verifier-keys.norito",
        "--record-out",
        "artifacts/kagemusha/recursive-compact-len4.record.norito",
        "--record-namespace",
        EXPECTED_COMPACT_KEY_RECORD_NAMESPACE,
        "--record-version",
        str(EXPECTED_COMPACT_KEY_RECORD_VERSION),
    )
    try:
        tokens = tuple(shlex.split(command))
    except ValueError:
        tokens = ()
        errors.append("--command must be shell-tokenizable without quoting errors")
    if tokens != expected_tokens:
        errors.append(
            "--command must exactly match the production ABI-7 recursive compact keygen command"
        )
    if command != expected_command:
        errors.append(
            "--command must exactly match the canonical ABI-7 recursive compact keygen command string"
        )
    if device_lab.SECRET_RE.search(command):
        errors.append("--command must not contain secret-looking material")
    return errors


def validate_compact_key_artifact_prefix(prefix: bytes, artifact: str) -> list[str]:
    """Reject obvious development placeholders for ABI-7 compact key artifacts."""

    stripped = prefix.strip().lower()
    if prefix and all(byte == 0 for byte in prefix):
        return [
            (
                f"recursive compact key artifact {artifact} "
                f"{COMPACT_KEY_ALL_ZERO_ERROR}"
            )
        ]
    if any(stripped.startswith(marker) for marker in COMPACT_KEY_PLACEHOLDER_PREFIXES):
        return [
            (
                f"recursive compact key artifact {artifact} "
                f"{COMPACT_KEY_PLACEHOLDER_ERROR}"
            )
        ]
    return []


def validate_compact_key_artifact_content(path: Path, artifact: str) -> list[str]:
    """Reject obvious development placeholders for ABI-7 compact key artifacts."""

    _digest, _size, prefix, errors = _sha256_file_with_size_and_prefix(
        path,
        f"recursive compact key artifact {artifact}",
        allow_empty=True,
    )
    if errors:
        return errors
    assert prefix is not None
    return validate_compact_key_artifact_prefix(prefix, artifact)


def validate_lineage_artifact_prefix(prefix: bytes, artifact: str) -> list[str]:
    """Reject obvious development placeholders for Reserved-lineage artifacts."""

    if prefix and all(byte == 0 for byte in prefix):
        return [f"lineage artifact {artifact} {LINEAGE_ARTIFACT_ALL_ZERO_ERROR}"]
    return []


def validate_lineage_artifact_content(path: Path, artifact: str) -> list[str]:
    """Reject obvious development placeholders for Reserved-lineage artifacts."""

    _digest, _size, prefix, errors = _sha256_file_with_size_and_prefix(
        path,
        f"lineage artifact {artifact}",
        allow_empty=True,
    )
    if errors:
        return errors
    assert prefix is not None
    return validate_lineage_artifact_prefix(prefix, artifact)


def parse_compact_key_generator_log(
    text: str,
) -> tuple[dict[str, int], dict[str, str], list[str]]:
    """Parse the canonical ABI-7 recursive compact key generator summary log."""

    errors: list[str] = []
    if "\r" in text:
        errors.append("compact key generator log must use canonical LF line endings")
    if not text.endswith("\n"):
        errors.append("compact key generator log must end with a canonical LF line terminator")
    lines = text.splitlines()
    if len(lines) != 1:
        errors.append("compact key generator log must contain exactly one summary line")
    if errors:
        return {}, {}, errors
    line = lines[0]
    match = COMPACT_KEY_GENERATOR_LOG_RE.fullmatch(line)
    if match is None:
        return {}, {}, ["compact key generator log must match the canonical CLI summary"]
    sizes = {
        artifact: int(match.group(field))
        for artifact, field in COMPACT_KEY_GENERATOR_LOG_SIZE_FIELDS.items()
    }
    digests = {
        artifact: match.group(field)
        for artifact, field in COMPACT_KEY_GENERATOR_LOG_DIGEST_FIELDS.items()
    }
    if any(digest == "0" * 64 for digest in digests.values()):
        return (
            {},
            {},
            ["compact key generator log must contain non-zero SHA-256 artifact digests"],
        )
    return sizes, digests, []


def validate_compact_key_generator_log(
    path: Path,
    expected_sha256: Any,
    artifact_size_bytes: dict[str, int],
    artifact_sha256: dict[str, str],
) -> tuple[str | None, dict[str, int], dict[str, str], list[dict[str, Any]]]:
    """Validate the ABI-7 compact-key generator log against local artifacts."""

    blockers: list[dict[str, Any]] = []
    _require_compact_key_sha256(
        blockers,
        value=expected_sha256,
        field="generator_log_sha256",
        code="compact_key_evidence_generator_log_sha256",
    )
    file_errors = validate_lineage_local_file(
        path,
        "ABI-7 recursive compact key generator log",
    )
    if file_errors:
        for error in file_errors:
            blockers.append(
                blocker(
                    "compact_key_evidence_generator_log_file_shape",
                    error,
                )
            )
        return None, {}, {}, blockers
    size_error = (
        "ABI-7 recursive compact key generator log must be no more than "
        f"{MAX_COMPACT_KEY_GENERATOR_LOG_BYTES} bytes"
    )
    digest, text, read_errors = _sha256_text_file(
        path,
        "ABI-7 recursive compact key generator log",
        "ABI-7 recursive compact key generator log could not be read",
        max_bytes=MAX_COMPACT_KEY_GENERATOR_LOG_BYTES,
        too_large_error=size_error,
    )
    if read_errors:
        for error in read_errors:
            blockers.append(
                blocker(
                    (
                        "compact_key_evidence_generator_log_size"
                        if error == size_error
                        else "compact_key_evidence_generator_log_file_shape"
                    ),
                    error,
                )
            )
        return None, {}, {}, blockers
    assert digest is not None and text is not None
    if _is_lower_sha256_hex(expected_sha256) and digest != expected_sha256:
        blockers.append(
            blocker(
                "compact_key_evidence_generator_log_digest",
                "ABI-7 recursive compact key generator log digest does not match local bytes",
            )
        )
    parsed_sizes, parsed_digests, parse_errors = parse_compact_key_generator_log(text)
    for error in parse_errors:
        blockers.append(
            blocker(
                "compact_key_evidence_generator_log_format",
                error,
            )
        )
    for artifact, actual_size in artifact_size_bytes.items():
        logged_size = parsed_sizes.get(artifact)
        if logged_size is not None and logged_size != actual_size:
            blockers.append(
                blocker(
                    "compact_key_evidence_generator_log_artifact_size",
                    "ABI-7 recursive compact key generator log size does not match local artifact bytes",
                    artifact=artifact,
                )
            )
    for artifact, actual_digest in artifact_sha256.items():
        logged_digest = parsed_digests.get(artifact)
        if logged_digest is not None and logged_digest != actual_digest:
            blockers.append(
                blocker(
                    "compact_key_evidence_generator_log_artifact_digest",
                    "ABI-7 recursive compact key generator log digest does not match local artifact bytes",
                    artifact=artifact,
                )
            )
    return digest, parsed_sizes, parsed_digests, blockers


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


def _require_lineage_artifact_size(
    blockers: list[dict[str, Any]],
    *,
    value: Any,
    artifact: str,
    actual_size: int | None = None,
) -> bool:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifact_size",
                "Reserved-lineage proof evidence artifact size must be a positive integer",
                artifact=artifact,
            )
        )
        return False
    if actual_size is not None and value != actual_size:
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifact_size",
                "Reserved-lineage proof evidence artifact size does not match local artifact bytes",
                artifact=artifact,
            )
        )
        return False
    return True


def _require_compact_key_sha256(
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
                f"recursive compact key evidence {field} must be a non-zero lowercase sha256 hex digest",
                field=field,
            )
        )


def _require_compact_key_artifact_size(
    blockers: list[dict[str, Any]],
    *,
    value: Any,
    artifact: str,
    actual_size: int | None = None,
) -> bool:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        blockers.append(
            blocker(
                "compact_key_evidence_artifact_size",
                "ABI-7 recursive compact key evidence artifact size must be a positive integer",
                artifact=artifact,
            )
        )
        return False
    if actual_size is not None and value != actual_size:
        blockers.append(
            blocker(
                "compact_key_evidence_artifact_size",
                "ABI-7 recursive compact key evidence artifact size does not match local artifact bytes",
                artifact=artifact,
            )
        )
        return False
    return True


def _display_evidence_field(field: str) -> str:
    return device_lab.SECRET_PATH_REDACTION if device_lab.SECRET_RE.search(field) else field


def _display_evidence_value(value: Any) -> Any:
    if isinstance(value, str) and device_lab.SECRET_RE.search(value):
        return device_lab.SECRET_PATH_REDACTION
    return value


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
            "artifact_size_bytes": {},
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
        unreadable_code="lineage_proof_evidence_unreadable",
        shape_code="lineage_proof_evidence_file_shape",
        not_object_code="lineage_proof_evidence_not_object",
        label="Reserved-lineage proof evidence",
        max_bytes=MAX_LINEAGE_PROOF_EVIDENCE_JSON_BYTES,
    )
    blockers.extend(load_blockers)
    details: dict[str, Any] = {
        "path": LINEAGE_PROOF_EVIDENCE_SUMMARY_LABEL,
        "schema": None,
        "artifact_sha256": {},
        "artifact_size_bytes": {},
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

    details["schema"] = _display_evidence_value(evidence.get("schema"))
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
        details["generated_at_utc"] = _display_evidence_value(generated_at_raw)
        if device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at_raw) is None:
            blockers.append(
                blocker(
                    "lineage_proof_evidence_timestamp_noncanonical",
                    "Reserved-lineage proof evidence generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    generated_at_utc=_display_evidence_value(generated_at_raw),
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
                    generated_at_utc=_display_evidence_value(generated_at_raw),
                    min_generated_at_utc=min_generated_at.isoformat().replace("+00:00", "Z"),
                )
            )
        elif max_generated_at is not None and generated_at is not None and generated_at > max_generated_at:
            blockers.append(
                blocker(
                    "lineage_proof_evidence_future_dated",
                    "Reserved-lineage proof evidence is ahead of the release validator clock skew",
                    generated_at_utc=_display_evidence_value(generated_at_raw),
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
    details["record_archive_proof_runtime_keygen_env"] = _display_evidence_value(
        evidence.get("record_archive_proof_runtime_keygen_env")
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
            key: _display_evidence_value(circuit_ids.get(key))
            for key in sorted(EXPECTED_LINEAGE_CIRCUIT_IDS)
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
    artifact_sizes = evidence.get("artifact_size_bytes")
    artifact_count = 0
    validated_artifact_sha256: dict[str, str] = {}
    validated_artifact_sizes: dict[str, int] = {}
    artifact_sizes_valid = isinstance(artifact_sizes, dict)
    if not artifact_sizes_valid:
        blockers.append(
            blocker(
                "lineage_proof_evidence_artifact_sizes",
                "Reserved-lineage proof evidence artifact_size_bytes must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifact_sizes) - set(LINEAGE_PROOF_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "lineage_proof_evidence_artifact_sizes_unexpected_field",
                    "Reserved-lineage proof evidence artifact_size_bytes contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
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
            expected_size = artifact_sizes.get(artifact) if artifact_sizes_valid else None
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
            (
                actual_digest,
                artifact_size,
                artifact_prefix,
                digest_errors,
            ) = _sha256_file_with_size_and_prefix(
                artifact_path,
                "Reserved-lineage proof evidence artifact file",
                allow_empty=True,
            )
            if digest_errors:
                for error in digest_errors:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_file_shape",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            assert (
                actual_digest is not None
                and artifact_size is not None
                and artifact_prefix is not None
            )
            size_matches = _require_lineage_artifact_size(
                blockers,
                value=expected_size,
                artifact=artifact,
                actual_size=artifact_size,
            )
            if artifact_size <= 0:
                blockers.append(
                    blocker(
                        "lineage_proof_evidence_artifact_empty",
                        "Reserved-lineage proof evidence artifact file must be non-empty",
                        artifact=artifact,
                    )
                )
                continue
            content_errors = validate_lineage_artifact_prefix(artifact_prefix, artifact)
            if content_errors:
                for error in content_errors:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_placeholder",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            if _is_lower_sha256_hex(expected_digest):
                if actual_digest != expected_digest:
                    blockers.append(
                        blocker(
                            "lineage_proof_evidence_artifact_file_digest",
                            "Reserved-lineage proof evidence artifact digest does not match local artifact bytes",
                            artifact=artifact,
                        )
                    )
                elif size_matches:
                    validated_artifact_sha256[artifact] = actual_digest
                    validated_artifact_sizes[artifact] = artifact_size
    details["artifact_count"] = artifact_count
    details["artifact_sha256"] = validated_artifact_sha256
    details["artifact_size_bytes"] = validated_artifact_sizes

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
        details["tests"] = [_display_evidence_field(key) for key in sorted(tests)]
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
            actual_log_digest, log_errors = validate_lineage_proof_log(
                log_artifact_path, expected_name
            )
            log_file_missing = log_errors == ["missing production proof log"]
            if actual_log_digest is None:
                blockers.append(
                    blocker(
                        (
                            "lineage_proof_evidence_test_log_unreadable"
                            if not log_file_missing
                            else "lineage_proof_evidence_test_log_missing"
                        ),
                        (
                            f"Reserved-lineage proof evidence test {key} log file could not be validated"
                            if not log_file_missing
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


def check_compact_key_evidence(
    path: Path,
    *,
    min_generated_at: dt.datetime | None = None,
    max_generated_at: dt.datetime | None = None,
    require_canonical_filename: bool = True,
) -> dict[str, Any]:
    """Check ABI-7 recursive compact key-artifact release evidence."""

    blockers: list[dict[str, Any]] = []
    if require_canonical_filename and path.name != COMPACT_KEY_EVIDENCE_FILENAME:
        blockers.append(
            blocker(
                "compact_key_evidence_filename",
                (
                    "ABI-7 recursive compact key evidence file must be named "
                    f"{COMPACT_KEY_EVIDENCE_FILENAME}"
                ),
                expected=COMPACT_KEY_EVIDENCE_FILENAME,
            )
        )
    evidence_file_errors = [
        *device_lab.validate_no_symlink_ancestors(
            path,
            "ABI-7 recursive compact key evidence ancestor directory",
        )
    ]
    for error in validate_lineage_local_file(
        path,
        "ABI-7 recursive compact key evidence file",
    ):
        if error != "ABI-7 recursive compact key evidence file is missing":
            evidence_file_errors.append(error)
    if evidence_file_errors:
        for error in evidence_file_errors:
            blockers.append(blocker("compact_key_evidence_file_shape", error))
        return {
            "path": COMPACT_KEY_EVIDENCE_SUMMARY_LABEL,
            "schema": None,
            "artifact_sha256": {},
            "artifact_size_bytes": {},
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
        missing_code="compact_key_evidence_missing",
        invalid_code="compact_key_evidence_invalid_json",
        unreadable_code="compact_key_evidence_unreadable",
        shape_code="compact_key_evidence_file_shape",
        not_object_code="compact_key_evidence_not_object",
        label="ABI-7 recursive compact key evidence",
        max_bytes=MAX_COMPACT_KEY_EVIDENCE_JSON_BYTES,
    )
    blockers.extend(load_blockers)
    details: dict[str, Any] = {
        "path": COMPACT_KEY_EVIDENCE_SUMMARY_LABEL,
        "schema": None,
        "artifact_sha256": {},
        "artifact_size_bytes": {},
        "generator_log_sha256": None,
        "generator_log_artifact_size_bytes": {},
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

    for field in sorted(set(evidence) - COMPACT_KEY_EVIDENCE_FIELDS):
        blockers.append(
            blocker(
                "compact_key_evidence_unexpected_field",
                "ABI-7 recursive compact key evidence contains unexpected field",
                field=_display_evidence_field(field),
            )
        )

    details["schema"] = _display_evidence_value(evidence.get("schema"))
    details["generated_at_utc"] = None
    if evidence.get("schema") != COMPACT_KEY_EVIDENCE_SCHEMA:
        blockers.append(
            blocker(
                "compact_key_evidence_schema",
                "ABI-7 recursive compact key evidence schema mismatch",
            )
        )

    generated_at_text = evidence.get("generated_at_utc")
    if not isinstance(generated_at_text, str) or not generated_at_text.strip():
        blockers.append(
            blocker(
                "compact_key_evidence_timestamp_missing",
                "ABI-7 recursive compact key evidence generated_at_utc is required",
            )
        )
    else:
        compact_generated_at_raw = generated_at_text
        details["generated_at_utc"] = _display_evidence_value(compact_generated_at_raw)
        if device_lab.SIGNED_AT_UTC_RE.fullmatch(compact_generated_at_raw) is None:
            blockers.append(
                blocker(
                    "compact_key_evidence_timestamp_noncanonical",
                    "ABI-7 recursive compact key evidence generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    generated_at_utc=_display_evidence_value(compact_generated_at_raw),
                )
            )
        generated_at, parse_blocker = parse_utc_timestamp(
            compact_generated_at_raw,
            "ABI-7 recursive compact key evidence generated_at_utc",
        )
        if parse_blocker is not None:
            parse_blocker["code"] = "compact_key_evidence_timestamp_invalid"
            blockers.append(parse_blocker)
        elif min_generated_at is not None and generated_at is not None and generated_at < min_generated_at:
            blockers.append(
                blocker(
                    "compact_key_evidence_stale",
                    "ABI-7 recursive compact key evidence predates the required release evidence cutoff",
                    generated_at_utc=_display_evidence_value(compact_generated_at_raw),
                    min_generated_at_utc=min_generated_at.isoformat().replace("+00:00", "Z"),
                )
            )
        elif max_generated_at is not None and generated_at is not None and generated_at > max_generated_at:
            blockers.append(
                blocker(
                    "compact_key_evidence_future_dated",
                    "ABI-7 recursive compact key evidence is ahead of the release validator clock skew",
                    generated_at_utc=_display_evidence_value(compact_generated_at_raw),
                    max_generated_at_utc=max_generated_at.isoformat().replace("+00:00", "Z"),
                )
            )

    expected_scalars = {
        "opening_len": EXPECTED_COMPACT_KEY_OPENING_LEN,
        "ipa_k": EXPECTED_COMPACT_KEY_IPA_K,
        "record_version": EXPECTED_COMPACT_KEY_RECORD_VERSION,
    }
    for field, expected in expected_scalars.items():
        compact_scalar_value = evidence.get(field)
        if (
            not isinstance(compact_scalar_value, int)
            or isinstance(compact_scalar_value, bool)
            or compact_scalar_value != expected
        ):
            blockers.append(
                blocker(
                    f"compact_key_evidence_{field}",
                    f"ABI-7 recursive compact key evidence {field} must be integer {expected}",
                    field=field,
                )
            )
    details["opening_len"] = evidence.get("opening_len")
    details["ipa_k"] = evidence.get("ipa_k")
    details["record_version"] = evidence.get("record_version")

    for field, expected in {
        "verifier_backend": EXPECTED_COMPACT_KEY_BACKEND,
        "circuit_id": EXPECTED_COMPACT_KEY_CIRCUIT_ID,
        "record_namespace": EXPECTED_COMPACT_KEY_RECORD_NAMESPACE,
    }.items():
        if evidence.get(field) != expected:
            blockers.append(
                blocker(
                    f"compact_key_evidence_{field}",
                    f"ABI-7 recursive compact key evidence {field} mismatch",
                    field=field,
                    expected=expected,
                )
            )
        details[field] = _display_evidence_value(evidence.get(field))

    command_errors = validate_compact_key_command(evidence.get("command"))
    for error in command_errors:
        blockers.append(
            blocker(
                "compact_key_evidence_command",
                "ABI-7 recursive compact key evidence command is not the canonical keygen run",
                issue=error,
            )
        )
    details["command_validated"] = not command_errors

    artifacts = evidence.get("artifacts")
    artifact_sizes = evidence.get("artifact_size_bytes")
    artifact_count = 0
    validated_artifact_sha256: dict[str, str] = {}
    validated_artifact_sizes: dict[str, int] = {}
    local_artifact_sha256: dict[str, str] = {}
    local_artifact_sizes: dict[str, int] = {}
    artifact_sizes_valid = isinstance(artifact_sizes, dict)
    if not artifact_sizes_valid:
        blockers.append(
            blocker(
                "compact_key_evidence_artifact_sizes",
                "ABI-7 recursive compact key evidence artifact_size_bytes must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifact_sizes) - set(COMPACT_KEY_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "compact_key_evidence_artifact_sizes_unexpected_field",
                    "ABI-7 recursive compact key evidence artifact_size_bytes contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
    if not isinstance(artifacts, dict):
        blockers.append(
            blocker(
                "compact_key_evidence_artifacts",
                "ABI-7 recursive compact key evidence artifacts must be an object",
            )
        )
    else:
        for artifact in sorted(set(artifacts) - set(COMPACT_KEY_REQUIRED_ARTIFACTS)):
            blockers.append(
                blocker(
                    "compact_key_evidence_artifacts_unexpected_field",
                    "ABI-7 recursive compact key evidence artifacts contains unexpected field",
                    field=_display_evidence_field(artifact),
                )
            )
        artifact_count = len(artifacts)
        artifact_root = path.parent
        for artifact in COMPACT_KEY_REQUIRED_ARTIFACTS:
            expected_digest = artifacts.get(artifact)
            expected_size = artifact_sizes.get(artifact) if artifact_sizes_valid else None
            _require_compact_key_sha256(
                blockers,
                value=expected_digest,
                field=f"artifacts.{artifact}",
                code="compact_key_evidence_artifact_digest",
            )
            artifact_path = artifact_root / artifact
            artifact_file_errors = validate_lineage_local_file(
                artifact_path,
                "ABI-7 recursive compact key evidence artifact file",
            )
            if artifact_file_errors:
                if artifact_file_errors == [
                    "ABI-7 recursive compact key evidence artifact file is missing"
                ]:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_missing",
                            "ABI-7 recursive compact key evidence artifact file is missing",
                            artifact=artifact,
                        )
                    )
                else:
                    for error in artifact_file_errors:
                        blockers.append(
                            blocker(
                                "compact_key_evidence_artifact_file_shape",
                                error,
                                artifact=artifact,
                            )
                        )
                continue
            (
                actual_digest,
                artifact_size,
                artifact_prefix,
                digest_errors,
            ) = _sha256_file_with_size_and_prefix(
                artifact_path,
                "ABI-7 recursive compact key evidence artifact file",
                allow_empty=True,
            )
            if digest_errors:
                for error in digest_errors:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_file_shape",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            assert (
                actual_digest is not None
                and artifact_size is not None
                and artifact_prefix is not None
            )
            size_matches = _require_compact_key_artifact_size(
                blockers,
                value=expected_size,
                artifact=artifact,
                actual_size=artifact_size,
            )
            if artifact_size <= 0:
                blockers.append(
                    blocker(
                        "compact_key_evidence_artifact_empty",
                        "ABI-7 recursive compact key evidence artifact file must be non-empty",
                        artifact=artifact,
                    )
                )
                continue
            local_artifact_sizes[artifact] = artifact_size
            content_errors = validate_compact_key_artifact_prefix(artifact_prefix, artifact)
            if content_errors:
                for error in content_errors:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_placeholder",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
            local_artifact_sha256[artifact] = actual_digest
            if _is_lower_sha256_hex(expected_digest):
                if actual_digest != expected_digest:
                    blockers.append(
                        blocker(
                            "compact_key_evidence_artifact_file_digest",
                            "ABI-7 recursive compact key evidence artifact digest does not match local artifact bytes",
                            artifact=artifact,
                        )
                    )
                elif size_matches:
                    validated_artifact_sha256[artifact] = actual_digest
                    validated_artifact_sizes[artifact] = artifact_size
    generator_log_path = evidence.get("generator_log_path")
    generator_log_sha256 = evidence.get("generator_log_sha256")
    if generator_log_path != COMPACT_KEY_GENERATOR_LOG_FILENAME:
        _require_compact_key_sha256(
            blockers,
            value=generator_log_sha256,
            field="generator_log_sha256",
            code="compact_key_evidence_generator_log_sha256",
        )
        blockers.append(
            blocker(
                "compact_key_evidence_generator_log_path",
                (
                    "ABI-7 recursive compact key evidence generator_log_path must be "
                    f"{COMPACT_KEY_GENERATOR_LOG_FILENAME}"
                ),
                field=(
                    _display_evidence_field(generator_log_path)
                    if isinstance(generator_log_path, str)
                    else generator_log_path
                ),
            )
        )
    else:
        actual_log_digest, generator_log_sizes, generator_log_digests, generator_log_blockers = (
            validate_compact_key_generator_log(
                path.parent / COMPACT_KEY_GENERATOR_LOG_FILENAME,
                generator_log_sha256,
                local_artifact_sizes,
                local_artifact_sha256,
            )
        )
        blockers.extend(generator_log_blockers)
        if actual_log_digest is not None and not generator_log_blockers:
            details["generator_log_sha256"] = actual_log_digest
            details["generator_log_artifact_size_bytes"] = generator_log_sizes
            details["generator_log_artifact_sha256"] = generator_log_digests
    details["artifact_count"] = artifact_count
    details["artifact_sha256"] = validated_artifact_sha256
    details["artifact_size_bytes"] = validated_artifact_sizes
    details["ok"] = not blockers
    details["state"] = "compact_key_artifacts_validated" if not blockers else "blocked"
    details["blockers"] = blockers
    return details


def check_abi7_fail_closed(repo_root: Path) -> dict[str, Any]:
    """Check ABI-7 recursive compact launch-boundary source markers."""

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
        unreadable_error = "ABI-7 source marker file could not be read"
        text, file_errors = _repo_source_marker_text(
            path,
            label,
            unreadable_error,
        )
        if file_errors:
            for error in file_errors:
                code = (
                    "abi7_source_marker_file_unreadable"
                    if error == unreadable_error
                    else "abi7_source_marker_file_shape"
                )
                blockers.append(
                    blocker(
                        code,
                        error,
                        file=relative,
                    )
                )
            continue
        assert text is not None
        source_texts[relative] = text
    core_text = source_texts.get("crates/iroha_core/src/zk.rs", "")
    bridge_text = source_texts.get("crates/connect_norito_bridge/src/lib.rs", "")
    required_core_snippets = (
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE",
        "multi-hop proving requires the append verifier batch to be composed into the compact proof",
        "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_PROOF_UNAVAILABLE",
        "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_OPENING_LEN",
        "KAGEMUSHA_RECURSIVE_COMPACT_MIN_PROOF_BYTES",
        "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope",
        "height-aware detached compact Pallas archive must reject before proving",
        "height-aware extra compact Pallas opening must reject before proving",
        "height-aware missing compact Pallas opening must reject before proving",
        "duplicated multi-hop compact Pallas archive must reject before proving",
        "height-aware duplicated multi-hop compact Pallas archive must reject before proving",
        "forged multi-hop compact Pallas metadata must reject before proving",
        "height-aware forged multi-hop compact Pallas metadata must reject before proving",
        "reordered multi-hop compact Pallas archive must reject before proving",
        "height-aware reordered multi-hop compact Pallas archive must reject before proving",
    )
    for snippet in required_core_snippets:
        if snippet not in core_text:
            blockers.append(
                blocker(
                    "abi7_fail_closed_marker_missing",
                    "ABI-7 recursive compact launch-boundary marker is missing",
                    marker=snippet,
                )
            )
    core_function_contracts = (
        (
            "fn prove_kagemusha_recursive_compact_payment_token_one_hop_from_record_bundle_and_pallas_open_envelopes(",
            (
                "kagemusha_pallas_ipa_batch_verifier_preflight_bound_to_hop_proofs(",
                "validate_kagemusha_recursive_one_hop_verifier_slice_preflight_binding(",
                "kagemusha_recursive_spend_lineage_runtime_keygen_enabled()",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED",
                "missing compact one-hop proving key archive",
                "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch(",
            ),
        ),
        (
            "fn prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
            (
                "prove_kagemusha_recursive_compact_payment_token_one_hop_from_record_bundle_and_pallas_open_envelopes(",
                "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_append_envelope_dispatch(",
                "for hop_index in 1..hop_count",
                "kagemusha_recursive_spend_lineage_runtime_keygen_enabled()",
                "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACTS_REQUIRED",
                "missing compact append proving key archive",
            ),
        ),
        (
            "fn prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope_dispatch(",
            (
                "prove_halo2_ipa_kagemusha_recursive_compact_payment_token_one_hop_envelope::<$len>",
                "match usize::try_from(preflight.opening_len)",
                "4 => prove_len!(4)",
            ),
        ),
        (
            "pub fn prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive(",
            (
                "prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
                "proving_key_bytes",
                "None",
            ),
        ),
        (
            "pub fn prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_at_height(",
            (
                "prove_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelopes(",
                "proving_key_bytes",
                "Some(block_height)",
            ),
        ),
        (
            "pub fn preverify_kagemusha_recursive_compact_payment_token(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id(",
                "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
            ),
        ),
        (
            "pub fn verify_kagemusha_recursive_compact_payment_token(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_expected_circuit_id(",
                "verify_backend(",
            ),
        ),
        (
            "pub fn preverify_kagemusha_recursive_compact_payment_token_with_record(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
            ),
        ),
        (
            "pub fn preverify_kagemusha_recursive_compact_payment_token_with_record_at_height(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "Some(block_height)",
                "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
            ),
        ),
        (
            "pub fn verify_kagemusha_recursive_compact_payment_token_with_record(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "verify_backend(",
            ),
        ),
        (
            "pub fn verify_kagemusha_recursive_compact_payment_token_with_record_at_height(",
            (
                "preverify_kagemusha_recursive_compact_payment_token_with_record_at_optional_height_and_expected_circuit_id(",
                "Some(block_height)",
                "verify_backend(",
            ),
        ),
    )
    for signature, snippets in core_function_contracts:
        missing_snippets = _require_rust_function_contract(core_text, signature, snippets)
        for snippet in missing_snippets:
            blockers.append(
                blocker(
                    "abi7_fail_closed_contract_missing",
                    "ABI-7 recursive compact launch-boundary function contract is missing",
                    function=signature,
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
    bridge_function_contracts = (
        (
            "pub unsafe extern \"C\" fn connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes(",
            (
                "prove_verified_kagemusha_recursive_compact_payment_token_from_record_bundle_and_pallas_open_envelope_archive_with_key_artifacts(",
                "is_kagemusha_recursive_compact_unavailable_error(&err)",
                "BridgeError::KagemushaRecursiveCompactUnavailable",
            ),
        ),
        (
            "pub unsafe extern \"C\" fn connect_norito_kagemusha_verify_recursive_compact_payment_token(",
            (
                "preverify_kagemusha_recursive_compact_payment_token(&token, vk_box)",
                "Err(err) if is_kagemusha_recursive_compact_unavailable_error(&err) => {}",
                "verify_kagemusha_recursive_compact_payment_token(&token, vk_box)",
                "*out_valid = 0",
            ),
        ),
    )
    for signature, snippets in bridge_function_contracts:
        missing_snippets = _require_rust_function_contract(bridge_text, signature, snippets)
        for snippet in missing_snippets:
            blockers.append(
                blocker(
                    "abi7_bridge_unavailable_contract_missing",
                    "native bridge must map ABI-7 recursive compact unavailable separately",
                    function=signature,
                    marker=snippet,
                )
            )
    return {
        "ok": not blockers,
        "state": "package_aware_multi_hop_composed" if not blockers else "unknown",
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
        unreadable_error = "Reserved-lineage release-tooling file could not be read"
        text, file_errors = _repo_source_marker_text(
            path,
            "Reserved-lineage release-tooling marker file",
            unreadable_error,
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
            elif file_errors == [unreadable_error]:
                blockers.append(
                    blocker(
                        "lineage_key_release_file_unreadable",
                        unreadable_error,
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
        assert text is not None
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
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    slot_paths, discovery_errors = device_lab.discover_slots(root, slot_ids)
    if discovery_errors:
        return [], [
            blocker("android_device_lab_root_unreadable", error)
            for error in discovery_errors
        ]
    return [
        device_lab.scan_slot(
            slot_path,
            require_kagemusha_production_evidence=True,
            trusted_signer_public_keys=trusted_signer_public_keys,
        )
        for slot_path in slot_paths
    ], []


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
        if device_lab.SIGNED_AT_UTC_RE.fullmatch(signed_at_text) is None:
            blockers.append(
                blocker(
                    "android_signed_evidence_timestamp_noncanonical",
                    "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    slot=slot_name,
                    signed_at_utc=_display_evidence_value(signed_at_text),
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
            ("offline_wallet_apk_path", "offline_wallet_apk_path"),
            ("offline_wallet_apk_sha256", "offline_wallet_apk_sha256"),
            ("d2d_payment_transcript_path", "d2d_payment_transcript_path"),
            ("d2d_payment_transcript_sha256", "d2d_payment_transcript_sha256"),
            (
                "wallet_integrity_transcript_path",
                "wallet_integrity_transcript_path",
            ),
            (
                "wallet_integrity_transcript_sha256",
                "wallet_integrity_transcript_sha256",
            ),
            (
                "attestation_certificate_chain_path",
                "attestation_certificate_chain_path",
            ),
            (
                "attestation_certificate_chain_sha256",
                "attestation_certificate_chain_sha256",
            ),
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
    root_exists, root_errors = device_lab.classify_device_lab_root_path(root)
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
    if not root_exists:
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

    raw_reports, discovery_blockers = _slot_reports(
        root, trusted_signer_public_keys, validated_slot_ids
    )
    blockers.extend(discovery_blockers)
    reports, report_secret_blockers = _sanitize_android_reports(raw_reports)
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
    compact_key_evidence_path: Path | None = None,
    slot_ids: Iterable[str] | None = None,
    min_signed_at: dt.datetime | None = None,
    max_signed_at: dt.datetime | None = None,
    min_lineage_proof_evidence_at: dt.datetime | None = None,
    max_lineage_proof_evidence_at: dt.datetime | None = None,
    min_compact_key_evidence_at: dt.datetime | None = None,
    max_compact_key_evidence_at: dt.datetime | None = None,
) -> dict[str, Any]:
    """Build a complete Kagemusha readiness rollup."""

    if compact_key_evidence_path is None:
        compact_key_evidence_path = (
            lineage_proof_evidence_path.parent / COMPACT_KEY_EVIDENCE_FILENAME
        )
    abi6 = check_abi6_reserved_lineage(repo_root)
    abi7 = check_abi7_fail_closed(repo_root)
    lineage = check_lineage_key_release_tooling(repo_root)
    lineage_proof = check_lineage_proof_evidence(
        lineage_proof_evidence_path,
        min_generated_at=min_lineage_proof_evidence_at,
        max_generated_at=max_lineage_proof_evidence_at,
    )
    compact_key = check_compact_key_evidence(
        compact_key_evidence_path,
        min_generated_at=min_compact_key_evidence_at,
        max_generated_at=max_compact_key_evidence_at,
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
        *compact_key["blockers"],
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
        "compact_key_evidence": compact_key,
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
    parent_exists, parent_blockers = _validate_summary_output_parent(path)
    if parent_blockers:
        return parent_blockers
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        "--summary-out ancestor directory",
    )
    if ancestor_errors:
        return [
            blocker(SUMMARY_OUT_PATH_INVALID_CODE, error)
            for error in ancestor_errors
        ]
    if not parent_exists:
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [
                blocker(
                    SUMMARY_OUT_PATH_INVALID_CODE,
                    "--summary-out parent directory could not be created",
                )
            ]
    parent_exists, parent_blockers = _validate_summary_output_parent(
        path,
        missing_message="--summary-out parent must be a directory",
    )
    if parent_blockers:
        return parent_blockers
    if not parent_exists:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent must be a directory",
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
    try:
        summary_output_mode = path.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out file metadata could not be read",
            )
        ]
    if stat.S_ISLNK(summary_output_mode):
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out must not be a symlink",
            )
        ]
    if not stat.S_ISREG(summary_output_mode):
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


def _validate_summary_output_parent(
    path: Path,
    *,
    missing_message: str | None = None,
) -> tuple[bool, list[dict[str, Any]]]:
    """Classify the readiness summary output parent without following aliases."""

    parent = path.parent
    try:
        parent_mode = parent.lstat().st_mode
    except FileNotFoundError:
        if missing_message is None:
            return False, []
        return False, [blocker(SUMMARY_OUT_PATH_INVALID_CODE, missing_message)]
    except OSError:
        return False, [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent directory metadata could not be read",
            )
        ]
    if stat.S_ISLNK(parent_mode):
        return True, [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent directory must not be a symlink",
            )
        ]
    if not stat.S_ISDIR(parent_mode):
        return True, [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out parent must be a directory",
            )
        ]
    return True, []


def _summary_out_blocker(message: str) -> dict[str, Any]:
    return blocker(SUMMARY_OUT_PATH_INVALID_CODE, message)


def _read_summary_output_text(
    path: Path,
    expected_stat: os.stat_result,
) -> tuple[str | None, list[dict[str, Any]]]:
    """Read readiness summary output text without trusting a stale path."""

    chunks: list[bytes] = []
    summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [_summary_out_blocker("--summary-out must not be a symlink")]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [_summary_out_blocker("--summary-out must be a regular file")]
            summary_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if summary_open_identity != summary_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != summary_expected_identity:
                return None, [
                    _summary_out_blocker("--summary-out changed while being read")
                ]
            if open_stat.st_nlink > 1:
                return None, [_summary_out_blocker("--summary-out must not be hardlinked")]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != summary_expected_identity:
                return None, [
                    _summary_out_blocker("--summary-out changed while being read")
                ]
    except OSError:
        return None, [
            _summary_out_blocker("--summary-out write verification failed")
        ]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [
            _summary_out_blocker("--summary-out write verification failed")
        ]


def write_summary(path: Path, summary: dict[str, Any]) -> list[dict[str, Any]]:
    """Write a readiness summary JSON file."""

    errors = validate_summary_output_path(path)
    if errors:
        return errors
    try:
        summary_text = json.dumps(
            summary,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        ) + "\n"
    except ValueError:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out summary is not strict JSON",
            )
        ]
    tmp_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            dir=path.parent,
            encoding="utf-8",
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            tmp_path = Path(handle.name)
            handle.write(summary_text)
            handle.flush()
            os.fsync(handle.fileno())
        errors = validate_summary_output_path(path)
        if errors:
            return errors
        os.replace(tmp_path, path)
        tmp_path = None
    except OSError:
        return [
            blocker(
                SUMMARY_OUT_PATH_INVALID_CODE,
                "--summary-out could not be written",
            )
        ]
    finally:
        if tmp_path is not None:
            try:
                tmp_path.unlink(missing_ok=True)
            except OSError:
                pass
    try:
        parent_fd = os.open(path.parent, os.O_RDONLY)
    except OSError:
        parent_fd = None
    if parent_fd is not None:
        try:
            os.fsync(parent_fd)
        except OSError:
            pass
        finally:
            os.close(parent_fd)
    errors = validate_summary_output_path(path)
    if errors:
        return errors
    try:
        expected_stat = path.lstat()
    except (FileNotFoundError, OSError):
        return [_summary_out_blocker("--summary-out write verification failed")]
    if stat.S_ISLNK(expected_stat.st_mode):
        return [_summary_out_blocker("--summary-out must not be a symlink")]
    if not stat.S_ISREG(expected_stat.st_mode):
        return [_summary_out_blocker("--summary-out must be a regular file")]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [
            _summary_out_blocker("--summary-out hardlink metadata could not be read")
        ]
    if link_count > 1:
        return [_summary_out_blocker("--summary-out must not be hardlinked")]
    readback_text, readback_errors = _read_summary_output_text(path, expected_stat)
    if readback_errors:
        return readback_errors
    if readback_text != summary_text:
        return [_summary_out_blocker("--summary-out write verification failed")]
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
        "--compact-key-evidence",
        default=None,
        help=(
            "ABI-7 recursive compact key-artifact evidence JSON. Defaults to "
            f"{COMPACT_KEY_EVIDENCE_FILENAME} beside --lineage-proof-evidence."
        ),
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
    parser.add_argument(
        "--min-compact-key-evidence-at-utc",
        default=DEFAULT_MIN_SIGNED_AT_UTC,
        help=(
            "Minimum generated_at_utc timestamp accepted for ABI-7 recursive compact "
            "key evidence. Use an empty value to disable the freshness gate."
        ),
    )
    parser.add_argument(
        "--max-compact-key-evidence-future-skew-seconds",
        type=int,
        default=DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds ABI-7 recursive compact key evidence "
            "generated_at_utc may be ahead of the readiness validator clock."
        ),
    )
    parser.add_argument("--summary-out", default=None, help="Optional JSON summary path.")
    args = parser.parse_args(argv)

    path_blockers = validate_cli_path_arguments(args)
    repo_root: Path | None = None
    if not path_blockers:
        try:
            repo_root = Path(args.repo_root).resolve()
        except OSError:
            path_blockers.append(
                blocker(
                    "kagemusha_repo_root_path_invalid",
                    "--repo-root could not be resolved",
                )
            )
    if path_blockers:
        summary = {
            "schema": SUMMARY_SCHEMA,
            "generated_at": utc_now(),
            "status": "blocked",
            "ready": False,
            "blockers": path_blockers,
        }
    else:
        assert repo_root is not None
        device_lab_root = Path(args.device_lab_root)
        if not device_lab_root.is_absolute():
            device_lab_root = repo_root / device_lab_root
        lineage_proof_evidence_path = Path(args.lineage_proof_evidence)
        if not lineage_proof_evidence_path.is_absolute():
            lineage_proof_evidence_path = repo_root / lineage_proof_evidence_path
        if args.compact_key_evidence:
            compact_key_evidence_path = Path(args.compact_key_evidence)
            if not compact_key_evidence_path.is_absolute():
                compact_key_evidence_path = repo_root / compact_key_evidence_path
        else:
            compact_key_evidence_path = (
                lineage_proof_evidence_path.parent / COMPACT_KEY_EVIDENCE_FILENAME
            )
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
        min_compact_key_evidence_at = None
        if args.min_compact_key_evidence_at_utc:
            (
                min_compact_key_evidence_at,
                min_compact_key_evidence_at_blocker,
            ) = parse_utc_timestamp(
                args.min_compact_key_evidence_at_utc,
                "--min-compact-key-evidence-at-utc",
            )
        else:
            min_compact_key_evidence_at_blocker = None
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
        max_compact_key_evidence_at = None
        max_compact_key_evidence_at_blocker = None
        if args.max_compact_key_evidence_future_skew_seconds < 0:
            max_compact_key_evidence_at_blocker = blocker(
                "compact_key_evidence_max_timestamp_invalid",
                "--max-compact-key-evidence-future-skew-seconds must be non-negative",
            )
        else:
            max_compact_key_evidence_at = (
                dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
                + dt.timedelta(seconds=args.max_compact_key_evidence_future_skew_seconds)
            )
        if (
            signer_errors
            or min_signed_at_blocker is not None
            or min_lineage_proof_evidence_at_blocker is not None
            or min_compact_key_evidence_at_blocker is not None
            or max_signed_at_blocker is not None
            or max_lineage_proof_evidence_at_blocker is not None
            or max_compact_key_evidence_at_blocker is not None
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
            if min_compact_key_evidence_at_blocker is not None:
                min_compact_key_evidence_at_blocker["code"] = (
                    "compact_key_evidence_min_timestamp_invalid"
                )
                blockers.append(min_compact_key_evidence_at_blocker)
            if max_signed_at_blocker is not None:
                blockers.append(max_signed_at_blocker)
            if max_lineage_proof_evidence_at_blocker is not None:
                blockers.append(max_lineage_proof_evidence_at_blocker)
            if max_compact_key_evidence_at_blocker is not None:
                blockers.append(max_compact_key_evidence_at_blocker)
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
                compact_key_evidence_path=compact_key_evidence_path,
                slot_ids=args.slots,
                min_signed_at=min_signed_at,
                max_signed_at=max_signed_at,
                min_lineage_proof_evidence_at=min_lineage_proof_evidence_at,
                max_lineage_proof_evidence_at=max_lineage_proof_evidence_at,
                min_compact_key_evidence_at=min_compact_key_evidence_at,
                max_compact_key_evidence_at=max_compact_key_evidence_at,
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
