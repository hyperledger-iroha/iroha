#!/usr/bin/env python3
"""Run a reproducible Cargo build profile and emit a machine-readable report."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import resource
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA_VERSION = 2
INPUT_DRIFT_EXIT_CODE = 3
PROFILE_ENV_KEYS = (
    "CARGO_BUILD_TARGET",
    "CARGO_INCREMENTAL",
    "CARGO_PROFILE_DEV_CODEGEN_UNITS",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_DEV_INCREMENTAL",
    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
    "CARGO_PROFILE_RELEASE_DEBUG",
    "CARGO_PROFILE_RELEASE_INCREMENTAL",
    "MACOSX_DEPLOYMENT_TARGET",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
    "SOURCE_DATE_EPOCH",
)


@dataclass(frozen=True)
class SourceFingerprint:
    """Digest and size of the non-ignored repository input tree."""

    sha256: str
    files: int
    bytes: int
    deleted: int


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description=(
            "Profile a locked Cargo build with a stable source/toolchain input "
            "fingerprint. The target directory must be outside the repository."
        )
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root (default: inferred from this script).",
    )
    parser.add_argument(
        "--target-dir",
        type=Path,
        required=True,
        help="External Cargo target directory used for the measured build.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        required=True,
        help="JSON report path. A .jsonl message log and .stderr.log are adjacent.",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        default=1,
        help="Cargo build jobs (default: 1 for comparable measurements).",
    )
    parser.add_argument(
        "--label",
        default="cargo-build-profile",
        help="Stable caller-supplied label stored in the report.",
    )
    parser.add_argument(
        "--reuse-target",
        action="store_true",
        help="Allow a non-empty target directory for an explicit warm build.",
    )
    parser.add_argument(
        "cargo_args",
        nargs=argparse.REMAINDER,
        help="Cargo command after `--` (default: `build --workspace`).",
    )
    args = parser.parse_args(argv)
    if args.jobs <= 0:
        parser.error("--jobs must be greater than zero")
    return args


def sha256_bytes(payload: bytes) -> str:
    """Return a lowercase SHA-256 digest."""
    return hashlib.sha256(payload).hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    """Serialize a value into a stable compact JSON representation."""
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def command_output(command: Sequence[str], cwd: Path) -> str:
    """Run a read-only identity command and return normalized stdout."""
    return subprocess.check_output(command, cwd=cwd, text=True).strip()


def resolve_inside(path: Path, parent: Path) -> bool:
    """Return whether `path` resolves inside `parent`."""
    try:
        path.resolve().relative_to(parent.resolve())
    except ValueError:
        return False
    return True


def validate_paths(root: Path, target_dir: Path, out: Path, reuse_target: bool) -> None:
    """Validate that profiling outputs cannot perturb repository inputs."""
    root = root.resolve()
    target_dir = target_dir.resolve()
    out = out.resolve()
    if not (root / "Cargo.toml").is_file():
        raise ValueError(f"repository root has no Cargo.toml: {root}")
    if resolve_inside(target_dir, root):
        raise ValueError("--target-dir must be outside the repository")
    if resolve_inside(out, root):
        raise ValueError("--out must be outside the repository")
    if target_dir.exists() and not target_dir.is_dir():
        raise ValueError("--target-dir exists and is not a directory")
    if target_dir.exists() and not reuse_target:
        try:
            next(target_dir.iterdir())
        except StopIteration:
            pass
        else:
            raise ValueError(
                "--target-dir is non-empty; pass --reuse-target for a warm profile"
            )


def tracked_and_untracked_paths(root: Path) -> list[str]:
    """List tracked and non-ignored untracked repository paths."""
    raw = subprocess.check_output(
        [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        cwd=root,
    )
    return sorted(
        entry.decode("utf-8", "surrogateescape")
        for entry in raw.split(b"\0")
        if entry
    )


def source_fingerprint(root: Path, paths: Iterable[str]) -> SourceFingerprint:
    """Hash repository-relative path, mode, and content for every input file."""
    digest = hashlib.sha256()
    file_count = 0
    byte_count = 0
    deleted_count = 0
    for relative in sorted(paths):
        candidate = root / relative
        try:
            metadata = candidate.lstat()
        except FileNotFoundError:
            record = {
                "bytes": 0,
                "executable": False,
                "kind": "deleted",
                "path": relative.replace(os.sep, "/"),
                "sha256": None,
            }
            digest.update(canonical_json_bytes(record))
            digest.update(b"\n")
            deleted_count += 1
            continue
        if stat.S_ISDIR(metadata.st_mode):
            continue
        if stat.S_ISLNK(metadata.st_mode):
            payload = os.readlink(candidate).encode("utf-8", "surrogateescape")
            kind = "symlink"
        elif stat.S_ISREG(metadata.st_mode):
            payload = candidate.read_bytes()
            kind = "file"
        else:
            raise ValueError(f"unsupported source path type: {relative}")
        record = {
            "bytes": len(payload),
            "executable": bool(metadata.st_mode & stat.S_IXUSR),
            "kind": kind,
            "path": relative.replace(os.sep, "/"),
            "sha256": sha256_bytes(payload),
        }
        digest.update(canonical_json_bytes(record))
        digest.update(b"\n")
        file_count += 1
        byte_count += len(payload)
    return SourceFingerprint(
        digest.hexdigest(),
        file_count,
        byte_count,
        deleted_count,
    )


def normalized_cargo_args(raw: Sequence[str], jobs: int) -> list[str]:
    """Build a locked, JSON-emitting Cargo command argument vector."""
    cargo_args = list(raw)
    if cargo_args and cargo_args[0] == "--":
        cargo_args.pop(0)
    if not cargo_args:
        cargo_args = ["build", "--workspace"]
    if cargo_args[0].startswith("-"):
        raise ValueError("the first Cargo argument must be a subcommand")
    separator = cargo_args.index("--") if "--" in cargo_args else len(cargo_args)
    cargo_controls = cargo_args[:separator]
    if "--locked" not in cargo_controls:
        cargo_args.insert(1, "--locked")
        separator += 1
        cargo_controls = cargo_args[:separator]
    additions: list[str] = []
    if not any(
        item == "--message-format" or item.startswith("--message-format=")
        for item in cargo_controls
    ):
        additions.extend(["--message-format", "json-render-diagnostics"])
    if not any(
        item == "--timings" or item.startswith("--timings=")
        for item in cargo_controls
    ):
        additions.append("--timings")
    if not any(
        item in ("-j", "--jobs")
        or (item.startswith("-j") and item != "-j")
        or item.startswith("--jobs=")
        for item in cargo_controls
    ):
        additions.extend(["--jobs", str(jobs)])
    cargo_args[separator:separator] = additions
    return cargo_args


def normalized_package_id(package_id: str) -> str:
    """Remove checkout-specific prefixes from path package identifiers."""
    if package_id.startswith("path+file://"):
        _, separator, fragment = package_id.rpartition("#")
        return f"workspace#{fragment}" if separator else "workspace"
    return package_id


def artifact_unit(message: dict[str, Any]) -> dict[str, Any] | None:
    """Project one Cargo compiler-artifact message into a stable unit identity."""
    if message.get("reason") != "compiler-artifact":
        return None
    target = message.get("target")
    profile = message.get("profile")
    package_id = message.get("package_id")
    if (
        not isinstance(target, dict)
        or not isinstance(profile, dict)
        or not isinstance(package_id, str)
    ):
        return None
    return {
        "crate_types": sorted(str(item) for item in target.get("crate_types", [])),
        "features": sorted(str(item) for item in message.get("features", [])),
        "kind": sorted(str(item) for item in target.get("kind", [])),
        "name": str(target.get("name", "")),
        "package_id": normalized_package_id(package_id),
        "profile": {
            "debug_assertions": bool(profile.get("debug_assertions", False)),
            "debuginfo": profile.get("debuginfo"),
            "opt_level": str(profile.get("opt_level", "")),
            "test": bool(profile.get("test", False)),
        },
    }


def parse_cargo_messages(lines: Iterable[str]) -> tuple[list[dict[str, Any]], int, int]:
    """Extract stable unit identities and fresh/compiled counts from Cargo JSON."""
    units: list[dict[str, Any]] = []
    fresh = 0
    compiled = 0
    for line in lines:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(message, dict):
            continue
        unit = artifact_unit(message)
        if unit is None:
            continue
        units.append(unit)
        if bool(message.get("fresh", False)):
            fresh += 1
        else:
            compiled += 1
    units.sort(key=canonical_json_bytes)
    return units, fresh, compiled


def timing_html(target_dir: Path) -> dict[str, Any] | None:
    """Describe Cargo's stable HTML timing artifact when present."""
    path = target_dir / "cargo-timings" / "cargo-timing.html"
    if not path.is_file():
        return None
    payload = path.read_bytes()
    return {
        "bytes": len(payload),
        "path": "cargo-timings/cargo-timing.html",
        "sha256": sha256_bytes(payload),
    }


def write_json(path: Path, value: Any) -> None:
    """Write pretty, deterministic JSON with a trailing newline."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def selected_profile_environment() -> dict[str, str]:
    """Return build-affecting environment inputs in stable key order."""
    return {
        key: os.environ[key]
        for key in PROFILE_ENV_KEYS
        if key in os.environ and os.environ[key]
    }


def capture_input_manifest(
    root: Path,
    cargo_args: Sequence[str],
    jobs: int,
    label: str,
    reuse_target: bool,
) -> dict[str, Any]:
    """Capture every repository and toolchain input used by a profile."""
    cargo_lock = root / "Cargo.lock"
    if not cargo_lock.is_file():
        raise ValueError("Cargo.lock is missing")
    source = source_fingerprint(root, tracked_and_untracked_paths(root))
    return {
        "cargo_args": list(cargo_args),
        "cargo_lock_sha256": sha256_bytes(cargo_lock.read_bytes()),
        "git_revision": command_output(["git", "rev-parse", "HEAD"], root),
        "jobs": jobs,
        "label": label,
        "profile_mode": "warm" if reuse_target else "cold",
        "selected_env": selected_profile_environment(),
        "source": {
            "bytes": source.bytes,
            "deleted": source.deleted,
            "files": source.files,
            "sha256": source.sha256,
        },
        "toolchain": {
            "cargo": command_output(["cargo", "-Vv"], root),
            "rustc": command_output(["rustc", "-Vv"], root),
        },
    }


def changed_input_fields(
    before: dict[str, Any], after: dict[str, Any]
) -> list[str]:
    """Return sorted manifest fields whose values changed during profiling."""
    return sorted(
        key
        for key in before.keys() | after.keys()
        if before.get(key) != after.get(key)
    )


def main(argv: Sequence[str] | None = None) -> int:
    """Run the requested Cargo profile and write its report."""
    args = parse_args(argv)
    root = args.root.resolve()
    target_dir = args.target_dir.resolve()
    out = args.out.resolve()
    try:
        validate_paths(root, target_dir, out, args.reuse_target)
        cargo_args = normalized_cargo_args(args.cargo_args, args.jobs)
        input_manifest = capture_input_manifest(
            root,
            cargo_args,
            args.jobs,
            args.label,
            args.reuse_target,
        )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"profile_cargo_build: {error}", file=sys.stderr)
        return 2

    input_sha256 = sha256_bytes(canonical_json_bytes(input_manifest))

    target_dir.mkdir(parents=True, exist_ok=True)
    out.parent.mkdir(parents=True, exist_ok=True)
    message_log = out.with_suffix(out.suffix + ".jsonl")
    stderr_log = out.with_suffix(out.suffix + ".stderr.log")
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target_dir)
    environment["CARGO_BUILD_JOBS"] = str(args.jobs)
    command = ["cargo", *cargo_args]
    print(
        "profile_cargo_build: "
        f"input={input_sha256} mode={input_manifest['profile_mode']} "
        f"command={' '.join(command)}",
        file=sys.stderr,
    )

    usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started_ns = time.monotonic_ns()
    units: list[dict[str, Any]] = []
    fresh_units = 0
    compiled_units = 0
    with message_log.open("w", encoding="utf-8") as messages, stderr_log.open(
        "w", encoding="utf-8"
    ) as errors:
        process = subprocess.Popen(
            command,
            cwd=root,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=errors,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        assert process.stdout is not None
        for line in process.stdout:
            messages.write(line)
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(message, dict):
                continue
            unit = artifact_unit(message)
            if unit is None:
                continue
            units.append(unit)
            if bool(message.get("fresh", False)):
                fresh_units += 1
            else:
                compiled_units += 1
        returncode = process.wait()
    elapsed_ns = time.monotonic_ns() - started_ns
    usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)

    post_input_manifest: dict[str, Any] | None = None
    input_capture_error: str | None = None
    try:
        post_input_manifest = capture_input_manifest(
            root,
            cargo_args,
            args.jobs,
            args.label,
            args.reuse_target,
        )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        input_capture_error = f"{type(error).__name__}: {error}"

    if post_input_manifest is None:
        changed_fields = ["post_input_capture"]
        post_input_sha256 = None
    else:
        changed_fields = changed_input_fields(input_manifest, post_input_manifest)
        post_input_sha256 = sha256_bytes(canonical_json_bytes(post_input_manifest))
    input_stable = not changed_fields

    units.sort(key=canonical_json_bytes)
    unit_inventory_sha256 = sha256_bytes(canonical_json_bytes(units))
    report = {
        "schema_version": SCHEMA_VERSION,
        "valid": returncode == 0 and input_stable,
        "input": input_manifest,
        "input_sha256": input_sha256,
        "input_validation": {
            "changed_fields": changed_fields,
            "error": input_capture_error,
            "post_input": post_input_manifest,
            "post_input_sha256": post_input_sha256,
            "stable": input_stable,
        },
        "result": {
            "compiled_units": compiled_units,
            "elapsed_ns": elapsed_ns,
            "fresh_units": fresh_units,
            "max_rss_raw": usage_after.ru_maxrss,
            "max_rss_unit": "bytes" if sys.platform == "darwin" else "KiB",
            "message_log": message_log.name,
            "platform": platform.platform(),
            "returncode": returncode,
            "stderr_log": stderr_log.name,
            "system_cpu_seconds": usage_after.ru_stime - usage_before.ru_stime,
            "timings_html": timing_html(target_dir),
            "unit_inventory": units,
            "unit_inventory_sha256": unit_inventory_sha256,
            "user_cpu_seconds": usage_after.ru_utime - usage_before.ru_utime,
        },
    }
    write_json(out, report)
    profiler_returncode = (
        INPUT_DRIFT_EXIT_CODE if returncode == 0 and not input_stable else returncode
    )
    print(
        "profile_cargo_build: "
        f"returncode={returncode} elapsed_ns={elapsed_ns} "
        f"compiled={compiled_units} fresh={fresh_units} "
        f"units={unit_inventory_sha256} input_stable={input_stable} report={out}",
        file=sys.stderr,
    )
    if not input_stable:
        print(
            "profile_cargo_build: report invalidated by input drift: "
            + ", ".join(changed_fields),
            file=sys.stderr,
        )
    return profiler_returncode


if __name__ == "__main__":
    raise SystemExit(main())
