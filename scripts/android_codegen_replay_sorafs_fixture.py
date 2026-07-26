#!/usr/bin/env python3
"""
Replay the multi-provider SoraFS manifest fixture and inject a concrete example
into the Android codegen assets.

This script:
1. Runs `sorafs_manifest_builder` against the shared orchestrator fixture to
   generate a fresh manifest report.
2. Copies the report under `target-codex/android_codegen/sorafs_manifest/`.
3. Updates the RegisterPinManifest instruction example with a `fixture_example`
   payload that mirrors the generated manifest so Kotlin/Java builders can
   consume real-world data when replaying the fixture.
"""

from __future__ import annotations

import argparse
import base64
import datetime
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import fsync_checker_output_parent
from sorafs_evidence_json import (
    json_object_without_duplicate_keys,
    reject_non_standard_json_constant,
)
from sorafs_path_identity import (
    error_diagnostic_label,
    path_diagnostic_label,
    resolve_path_identity,
)
from sorafs_runner_preflight import plan_rendered_path_is_safe


REPO_ROOT = SCRIPT_DIR.parent
DEFAULT_FIXTURE_DIR = REPO_ROOT / "fixtures/sorafs_orchestrator/multi_peer_parity_v1"
DEFAULT_CHUNKER_FIXTURE = REPO_ROOT / "fixtures/sorafs_chunker/sf1_profile_v1.json"
DEFAULT_REGISTER_PIN_EXAMPLE = (
    REPO_ROOT
    / "target-codex"
    / "android_codegen"
    / "instruction_examples"
    / "iroha_data_model::isi::sorafs::RegisterPinManifest.json"
)
DEFAULT_REPORT_DIR = (
    REPO_ROOT / "target-codex" / "android_codegen" / "sorafs_manifest"
)
DEFAULT_TRACKED_FIXTURE = (
    REPO_ROOT
    / "docs"
    / "source"
    / "sdk"
    / "android"
    / "generated"
    / "fixtures"
    / "sorafs_register_pin_manifest_multi_peer_parity_v1.json"
)
CODEGEN_PATH_DIAGNOSTIC = (
    "SoraFS Android codegen fixture paths must not contain secret-looking, "
    "control-character, parent, current, drive-prefix, or platform-specific "
    "components"
)
FIXTURE_METADATA_PATH_DIAGNOSTIC = (
    "SoraFS Android codegen fixture metadata paths must be safe relative paths"
)
FIXTURE_METADATA_NAME_RE = re.compile(r"[a-z0-9][a-z0-9_-]{0,127}\Z")
FIXTURE_METADATA_NAME_DIAGNOSTIC = (
    "SoraFS Android codegen fixture metadata fixture name must be a safe filename"
)
FIXTURE_METADATA_FIELD_DIAGNOSTIC = (
    "SoraFS Android codegen fixture metadata subprocess fields must be canonical"
)
PROFILE_HANDLE_RE = re.compile(
    r"sorafs\.[a-z0-9][a-z0-9_-]*@[0-9]+(?:\.[0-9]+){2}\Z"
)
STORAGE_CLASSES = frozenset({"hot", "warm", "cold"})
# Public, deterministic TEST-ONLY seed used solely to make the generated
# Android parity manifest cryptographically self-consistent. It is written to
# a mode-0600 file inside the private replay tempdir and is never persisted in
# reports, generated examples, or repository files.
ANDROID_CODEGEN_TEST_COUNCIL_SIGNING_SEED = bytes.fromhex("a5" * 32)


def iso_utc_from_unix_timestamp(value: int) -> str:
    """Render a reviewed Unix timestamp as canonical UTC fixture metadata."""

    return datetime.datetime.fromtimestamp(value, datetime.timezone.utc).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )


def read_open_flags() -> int:
    """Return descriptor flags for fail-closed JSON fixture reads."""

    return os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)


def write_open_flags() -> int:
    """Return descriptor flags for fail-closed JSON fixture writes."""

    return (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_TRUNC
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def signing_key_write_open_flags() -> int:
    """Return exclusive no-follow flags for the ephemeral test signing seed."""

    return (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def write_all(fd: int, chunk: bytes) -> None:
    """Write every byte to a generated fixture descriptor after short writes."""

    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write Android SoraFS codegen fixture")
        view = view[written:]


def validate_codegen_path(path: Path, label: str) -> None:
    """Reject symlinked codegen fixture paths and parent chains before I/O."""

    if not isinstance(path, Path):
        raise ValueError(f"{label} `{path_diagnostic_label(path)}` must be a path")
    if not plan_rendered_path_is_safe(path):
        raise ValueError(CODEGEN_PATH_DIAGNOSTIC)
    try:
        if path.is_symlink():
            raise ValueError(
                f"{label} `{path_diagnostic_label(path)}` must not be a symlink"
            )
        for parent in (path.parent, *path.parent.parents):
            if parent.is_symlink():
                raise ValueError(
                    f"{label} parent `{path_diagnostic_label(parent)}` "
                    "must not be a symlink"
                )
            if parent.exists() and not parent.is_dir():
                raise ValueError(
                    f"{label} parent `{path_diagnostic_label(parent)}` "
                    "must be a directory when it exists"
                )
    except ValueError:
        raise
    except OSError as error:
        path_label = path_diagnostic_label(path)
        error_label = error_diagnostic_label(error, path_label=path_label)
        raise ValueError(
            f"failed to inspect {label} `{path_label}`: {error_label}"
        ) from error


def ensure_codegen_directory(path: Path, label: str) -> None:
    """Create a codegen fixture directory after rejecting symlink parents."""

    validate_codegen_path(path, label)
    try:
        path.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        path_label = path_diagnostic_label(path)
        error_label = error_diagnostic_label(error, path_label=path_label)
        raise ValueError(
            f"failed to create {label} `{path_label}`: {error_label}"
        ) from error
    validate_codegen_path(path, label)
    if not path.is_dir():
        raise ValueError(f"{label} `{path_diagnostic_label(path)}` must be a directory")


def require_codegen_file(path: Path, label: str) -> Path:
    """Return an existing regular codegen fixture file after path validation."""

    validate_codegen_path(path, label)
    if not path.is_file():
        raise ValueError(
            f"{label} `{path_diagnostic_label(path)}` must exist and be a file"
        )
    return path


def require_relative_fixture_path(value: object, *, label: str) -> Path:
    """Return a safe relative path from fixture metadata."""

    if not isinstance(value, str):
        raise ValueError(FIXTURE_METADATA_PATH_DIAGNOSTIC)
    path = Path(value)
    if path.is_absolute() or not plan_rendered_path_is_safe(path):
        raise ValueError(FIXTURE_METADATA_PATH_DIAGNOSTIC)
    return path


def require_fixture_name(value: object) -> str:
    """Return a safe single-component fixture name from metadata."""

    if not isinstance(value, str) or FIXTURE_METADATA_NAME_RE.fullmatch(value) is None:
        raise ValueError(FIXTURE_METADATA_NAME_DIAGNOSTIC)
    if not plan_rendered_path_is_safe(Path(value)):
        raise ValueError(FIXTURE_METADATA_NAME_DIAGNOSTIC)
    return value


def require_profile_handle(value: object) -> str:
    """Return a canonical SoraFS chunker profile handle from metadata."""

    if not isinstance(value, str) or PROFILE_HANDLE_RE.fullmatch(value) is None:
        raise ValueError(FIXTURE_METADATA_FIELD_DIAGNOSTIC)
    return value


def require_storage_class(value: object) -> str:
    """Return a supported manifest storage class from metadata."""

    if not isinstance(value, str) or value not in STORAGE_CLASSES:
        raise ValueError(FIXTURE_METADATA_FIELD_DIAGNOSTIC)
    return value


def require_metadata_int(value: object, *, minimum: int) -> int:
    """Return a bounded integer from fixture metadata."""

    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < minimum
        or value > 2**63 - 1
    ):
        raise ValueError(FIXTURE_METADATA_FIELD_DIAGNOSTIC)
    return value


def load_json(path: Path, *, label: str = "JSON fixture") -> dict:
    require_codegen_file(path, label)
    fd = -1
    try:
        fd = os.open(path, read_open_flags())
        handle = os.fdopen(fd, "r", encoding="utf-8")
        fd = -1
        with handle:
            return json.load(
                handle,
                parse_constant=reject_non_standard_json_constant,
                object_pairs_hook=json_object_without_duplicate_keys,
            )
    except OSError as error:
        path_label = path_diagnostic_label(path)
        error_label = error_diagnostic_label(error, path_label=path_label)
        raise ValueError(f"failed to read {label} `{path_label}`: {error_label}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def write_json(path: Path, payload: dict, *, label: str = "JSON fixture") -> None:
    validate_codegen_path(path, label)
    ensure_codegen_directory(path.parent, f"{label} parent directory")
    validate_codegen_path(path, label)
    rendered = (json.dumps(payload, indent=2, allow_nan=False) + "\n").encode("utf-8")
    fd = -1
    try:
        fd = os.open(path, write_open_flags(), 0o666)
        write_all(fd, rendered)
        os.fsync(fd)
    except OSError as error:
        path_label = path_diagnostic_label(path)
        error_label = error_diagnostic_label(error, path_label=path_label)
        raise ValueError(f"failed to write {label} `{path_label}`: {error_label}") from error
    finally:
        if fd >= 0:
            os.close(fd)
    parent_sync_errors = fsync_checker_output_parent(path, label=label)
    if parent_sync_errors:
        raise ValueError(parent_sync_errors[0])


def write_test_council_signing_seed(path: Path) -> None:
    """Write the deterministic test-only seed to a private ephemeral file."""

    label = "Android codegen test-only council signing seed"
    validate_codegen_path(path, label)
    ensure_codegen_directory(path.parent, f"{label} parent directory")
    validate_codegen_path(path, label)
    fd = -1
    try:
        fd = os.open(path, signing_key_write_open_flags(), 0o600)
        if hasattr(os, "fchmod"):
            os.fchmod(fd, 0o600)
        write_all(fd, ANDROID_CODEGEN_TEST_COUNCIL_SIGNING_SEED)
        os.fsync(fd)
        metadata = os.fstat(fd)
        if not stat.S_ISREG(metadata.st_mode):
            raise OSError("test-only signing seed output is not a regular file")
        if metadata.st_size != len(ANDROID_CODEGEN_TEST_COUNCIL_SIGNING_SEED):
            raise OSError("test-only signing seed size changed during write")
        if metadata.st_mode & 0o077:
            raise OSError("test-only signing seed permissions are not private")
    except OSError as error:
        path_label = path_diagnostic_label(path)
        error_label = error_diagnostic_label(error, path_label=path_label)
        raise ValueError(f"failed to write {label} `{path_label}`: {error_label}") from error
    finally:
        if fd >= 0:
            os.close(fd)
    parent_sync_errors = fsync_checker_output_parent(path, label=label)
    if parent_sync_errors:
        raise ValueError(parent_sync_errors[0])


def run_manifest_builder(
    cargo_bin: str,
    payload_path: Path,
    plan_path: Path,
    profile_handle: str,
    min_replicas: int,
    storage_class: str,
    retention_epoch: int,
    council_signing_key_file: Path,
    json_out: Path,
    manifest_out: Path,
) -> None:
    cmd = [
        cargo_bin,
        "run",
        "--locked",
        "--quiet",
        "-p",
        "sorafs_car",
        "--features",
        "cli",
        "--bin",
        "sorafs_manifest_builder",
        str(payload_path),
        f"--plan={plan_path}",
        f"--chunker-profile={profile_handle}",
        f"--min-replicas={min_replicas}",
        f"--storage-class={storage_class}",
        f"--retention-epoch={retention_epoch}",
        f"--council-signing-key-file={council_signing_key_file}",
        f"--json-out={json_out}",
        f"--manifest-out={manifest_out}",
    ]
    subprocess.run(cmd, check=True, cwd=REPO_ROOT)


def require_generated_council_signature(manifest_report: dict) -> None:
    """Require the replayed manifest to report one canonical verified signer."""

    signatures = manifest_report.get("manifest", {}).get("council_signatures")
    if not isinstance(signatures, list) or len(signatures) != 1:
        raise ValueError("generated Android manifest must contain one council signature")
    entry = signatures[0]
    if not isinstance(entry, dict) or set(entry) != {"signer_hex", "signature_hex"}:
        raise ValueError("generated Android manifest council signature is malformed")
    signer_hex = entry.get("signer_hex")
    signature_hex = entry.get("signature_hex")
    if (
        not isinstance(signer_hex, str)
        or re.fullmatch(r"[0-9a-f]{64}", signer_hex) is None
        or not isinstance(signature_hex, str)
        or re.fullmatch(r"[0-9a-f]{128}", signature_hex) is None
    ):
        raise ValueError("generated Android manifest council signature is non-canonical")


def build_fixture_example(
    fixture_meta: dict,
    manifest_report: dict,
    manifest_report_path: Path,
    manifest_payload_base64: str,
) -> dict:
    timestamp = iso_utc_from_unix_timestamp(fixture_meta["now_unix_secs"])
    instruction = {
        "manifest_payload_base64": manifest_payload_base64,
        "submitted_epoch": fixture_meta["now_unix_secs"],
        "alias": None,
        "successor_of": None,
    }
    example = {
        "fixture": fixture_meta["fixture"],
        "generated_at": timestamp,
        "plan_file": fixture_meta["plan_file"],
        "providers_file": fixture_meta["providers_file"],
        "telemetry_file": fixture_meta["telemetry_file"],
        "manifest_report_path": str(manifest_report_path.relative_to(REPO_ROOT)),
        "chunk_digests_blake3": [
            entry["digest_blake3"] for entry in manifest_report["chunk_digests"]
        ],
        "instruction": instruction,
    }
    return example


def update_register_pin_example(example_path: Path, fixture_example: dict) -> None:
    data = load_json(example_path, label="RegisterPinManifest example")
    data["fixture_example"] = fixture_example
    write_json(example_path, data, label="RegisterPinManifest example")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Replay the SoraFS orchestrator fixture for Android codegen."
    )
    parser.add_argument(
        "--fixture-dir",
        type=Path,
        default=DEFAULT_FIXTURE_DIR,
        help="Directory that hosts multi-provider orchestrator fixture JSON files.",
    )
    parser.add_argument(
        "--chunker-fixture",
        type=Path,
        default=DEFAULT_CHUNKER_FIXTURE,
        help="Chunker profile fixture that includes the SHA3 digest metadata.",
    )
    parser.add_argument(
        "--register-pin-example",
        type=Path,
        default=DEFAULT_REGISTER_PIN_EXAMPLE,
        help="Path to the RegisterPinManifest instruction example JSON.",
    )
    parser.add_argument(
        "--report-dir",
        type=Path,
        default=DEFAULT_REPORT_DIR,
        help="Directory for storing generated manifest reports.",
    )
    parser.add_argument(
        "--tracked-fixture-out",
        type=Path,
        default=DEFAULT_TRACKED_FIXTURE,
        help="Tracked JSON file that mirrors the replayed fixture example.",
    )
    parser.add_argument(
        "--cargo-bin",
        default=os.environ.get("CARGO_BIN", "cargo"),
        help="Cargo binary to invoke (defaults to `cargo`).",
    )
    args = parser.parse_args(argv)

    try:
        fixture_meta = load_json(
            args.fixture_dir / "metadata.json",
            label="orchestrator fixture metadata",
        )
        chunker_fixture = load_json(args.chunker_fixture, label="chunker fixture")
        fixture_name = require_fixture_name(fixture_meta.get("fixture"))
        fixture_meta["fixture"] = fixture_name
        profile_handle = require_profile_handle(fixture_meta.get("profile_handle"))
        fixture_meta["profile_handle"] = profile_handle
        now_unix_secs = require_metadata_int(
            fixture_meta.get("now_unix_secs"),
            minimum=0,
        )
        fixture_meta["now_unix_secs"] = now_unix_secs
        retention_epoch = require_metadata_int(
            fixture_meta.get("retention_epoch", now_unix_secs + 86_400),
            minimum=0,
        )
        if retention_epoch <= now_unix_secs:
            raise ValueError(
                "SoraFS Android codegen fixture retention_epoch must be later than now_unix_secs"
            )
        min_replicas = require_metadata_int(
            fixture_meta.get("min_replicas", 3),
            minimum=1,
        )
        storage_class = require_storage_class(fixture_meta.get("storage_class", "hot"))
        plan_rel = require_relative_fixture_path(
            fixture_meta.get("plan_file"),
            label="plan file",
        )
        fixture_meta["plan_file"] = plan_rel.as_posix()
        for metadata_path_field in ("providers_file", "telemetry_file", "options_file"):
            metadata_path = require_relative_fixture_path(
                fixture_meta.get(metadata_path_field),
                label=metadata_path_field,
            )
            fixture_meta[metadata_path_field] = metadata_path.as_posix()

        payload_rel = require_relative_fixture_path(
            fixture_meta.get("payload_path"),
            label="payload path",
        )
        payload_path = require_codegen_file(REPO_ROOT / payload_rel, "payload path")

        plan_path = require_codegen_file(args.fixture_dir / plan_rel, "plan file")
    except ValueError as error:
        raise SystemExit(str(error)) from error

    report_path = args.report_dir / f"{fixture_name}.json"

    temporary_root_errors: list[str] = []
    temporary_root = resolve_path_identity(
        Path(tempfile.gettempdir()),
        temporary_root_errors,
        label="temporary output root",
    )
    if temporary_root is None:
        raise SystemExit(temporary_root_errors[0])
    validate_codegen_path(temporary_root, "temporary output root")
    with tempfile.TemporaryDirectory(dir=temporary_root) as tmpdir:
        tmp_report = Path(tmpdir) / "manifest_report.json"
        tmp_manifest = Path(tmpdir) / "manifest.to"
        tmp_council_signing_seed = Path(tmpdir) / "council-signing.seed"
        write_test_council_signing_seed(tmp_council_signing_seed)
        run_manifest_builder(
            args.cargo_bin,
            payload_path,
            plan_path,
            profile_handle,
            min_replicas=min_replicas,
            storage_class=storage_class,
            retention_epoch=retention_epoch,
            council_signing_key_file=tmp_council_signing_seed,
            json_out=tmp_report,
            manifest_out=tmp_manifest,
        )
        manifest_report = load_json(tmp_report, label="generated manifest report")
        require_generated_council_signature(manifest_report)
        embedded_chunk_digest = (
            manifest_report.get("manifest", {}).get("chunk_digest_sha3_256_hex")
        )
        expected_chunk_digest = chunker_fixture.get("chunk_digest_sha3_256")
        if (
            not isinstance(embedded_chunk_digest, str)
            or not isinstance(expected_chunk_digest, str)
            or embedded_chunk_digest.lower() != expected_chunk_digest.lower()
        ):
            raise SystemExit(
                "generated manifest chunk-plan commitment does not match the governed chunker fixture"
            )
        manifest_payload_base64 = base64.b64encode(tmp_manifest.read_bytes()).decode("ascii")
        write_json(report_path, manifest_report, label="manifest report")

    fixture_example = build_fixture_example(
        fixture_meta,
        manifest_report,
        report_path,
        manifest_payload_base64,
    )
    update_register_pin_example(args.register_pin_example, fixture_example)
    write_json(args.tracked_fixture_out, fixture_example, label="tracked fixture")
    print(
        f"[android-codegen] updated fixture example in "
        f"{args.register_pin_example.relative_to(REPO_ROOT)}"
    )
    print(
        f"[android-codegen] manifest report written to "
        f"{report_path.relative_to(REPO_ROOT)}"
    )
    print(
        f"[android-codegen] tracked fixture written to "
        f"{args.tracked_fixture_out.relative_to(REPO_ROOT)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
