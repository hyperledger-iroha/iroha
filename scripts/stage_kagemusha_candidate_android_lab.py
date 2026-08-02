#!/usr/bin/env python3
"""Validate and atomically stage one real Kagemusha Android candidate lab.

This tool never creates lifecycle requests, proofs, results, consensus data, or
scenario secrets.  It consumes one immutable ``generate-candidate`` directory
and one exact directory of operator-supplied proof-independent seed inputs.
Canonical CandidateV4 decoding, inner-manifest extraction, framed KRV4 header
validation, and content-address verification are delegated to the current
source ``kagemusha_recursive_spend_v4_bundle validate-candidate`` command.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import shutil
import stat
import subprocess
import sys
import tempfile
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 CI fallback.
    import tomli as tomllib
from dataclasses import dataclass
from typing import Any, Iterator, Mapping


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PARENT = REPOSITORY_ROOT / "artifacts/kagemusha-candidate-evidence"
REPORT_SCHEMA = "iroha.kagemusha.recursive_spend.candidate_validation.v2"
REPORT_NAME = "candidate-validation-v2.json"
VALIDATED_MANIFEST_NAME = "manifest-v4.norito"
QUALIFICATION_RECEIPT_NAME = "recursive-step-two-qualification-v4.norito"
QUALIFIED_CANDIDATE_DOMAIN = b"iroha:kagemusha:recursive-spend-qualified-candidate:v4"
GENERATION_MEMORY_ENFORCEMENT_PROFILE = "self-physical-footprint-v1"
MAX_GENERATION_MEMORY_BYTES = 64 * 1024 * 1024 * 1024
STAGE_MANIFEST_NAME = "candidate-stage-manifest-v2.json"
STAGE_MANIFEST_SCHEMA = "iroha.kagemusha.android_candidate_stage_manifest.v2"
VALIDATOR_SCHEMA = "iroha.kagemusha.android_candidate_validator.v1"
SCENARIO_VALIDATION_SCHEMA = (
    "iroha.kagemusha.android_candidate_scenario_validation.v1"
)
SCENARIO_INVENTORY_DOMAIN = (
    b"iroha.kagemusha.android-candidate-scenario-inventory.v1\0"
)
CANDIDATE_RECORD_NAME = "candidate-manifest.norito"
CANDIDATE_JSON_NAME = "candidate-manifest.json"
CANDIDATE_SHA_NAME = "candidate-manifest.norito.sha256"
ROSTER_NAME = "topup-finality-roster-v4.norito"
SCENARIO_ROSTER_NAME = "init-top-up-finality-roster-artifact-v2.norito"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
PORTABLE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
PUBLIC_TAIRA_CHAIN_DISCRIMINANT = 369
AUTHORITY_BUILD_FEATURES = (
    "iroha_core/dev-tools",
    "iroha_core/kagemusha-candidate-evidence-lab",
    "connect_norito_bridge/dev-tools",
    "connect_norito_bridge/kagemusha-candidate-evidence-lab",
)

ARTIFACTS = (
    ("step_eq_params_ipa", "step-eq.params-ipa.krv4"),
    ("step_eq_proving_key", "step-eq.proving-key.krv4"),
    ("step_eq_verifying_key", "step-eq.verifying-key.krv4"),
    ("step_eq_bootstrap_witness", "step-eq.bootstrap-witness.krv4"),
    ("step_ep_params_ipa", "step-ep.params-ipa.krv4"),
    ("step_ep_proving_key", "step-ep.proving-key.krv4"),
    ("step_ep_verifying_key", "step-ep.verifying-key.krv4"),
    ("step_ep_bootstrap_witness", "step-ep.bootstrap-witness.krv4"),
)
ARTIFACT_NAMES = tuple(name for _, name in ARTIFACTS)
CANDIDATE_INVENTORY = frozenset(
    (
        *ARTIFACT_NAMES,
        CANDIDATE_RECORD_NAME,
        CANDIDATE_JSON_NAME,
        CANDIDATE_SHA_NAME,
        QUALIFICATION_RECEIPT_NAME,
        ROSTER_NAME,
    )
)

SCENARIO_FILES = (
    "init-top-up-anchor-v4.norito",
    "init-top-up-finality-proof-v2.norito",
    SCENARIO_ROSTER_NAME,
    "init-opening-v2.norito",
    "init-output-membership-v4.norito",
    "transfer-verifier-commitment-v2.bin",
    "append-hop-01-recipient-request-v2.norito",
    "append-hop-01-recipient-opening-v2.norito",
    "append-hop-01-change-opening-v2.norito",
    "append-hop-01-output-membership-v4.norito",
    "append-hop-01-operation-id.bin",
    "append-hop-01-block-height.txt",
    "append-hop-01-verified-at-ms.txt",
    "append-hop-02-recipient-request-v2.norito",
    "append-hop-02-recipient-opening-v2.norito",
    "append-hop-02-change-opening-v2.norito",
    "append-hop-02-output-membership-v4.norito",
    "append-hop-02-operation-id.bin",
    "append-hop-02-block-height.txt",
    "append-hop-02-verified-at-ms.txt",
    "redeem-recipient-account-id.txt",
    "unshield-verifier-commitment-v2.bin",
    "redeem-hop-01-operation-id.bin",
    "redeem-hop-01-block-height.txt",
    "redeem-hop-02-operation-id.bin",
    "redeem-hop-02-block-height.txt",
    "redeem-sender-change-operation-id.bin",
    "redeem-sender-change-block-height.txt",
    "duplicate-input-recipient-request-v2.norito",
    "duplicate-input-output-membership-v4.norito",
    "duplicate-input-operation-id.bin",
    "duplicate-input-block-height.txt",
    "duplicate-input-verified-at-ms.txt",
)
SCENARIO_INVENTORY = frozenset(SCENARIO_FILES)

STAGED_NON_SELF_PATHS = frozenset(
    {
        "evidence/candidate/candidate-v4.norito",
        f"evidence/candidate/{VALIDATED_MANIFEST_NAME}",
        f"evidence/candidate/{REPORT_NAME}",
        f"evidence/candidate/{QUALIFICATION_RECEIPT_NAME}",
        *(f"evidence/candidate/artifacts/{name}" for name in ARTIFACT_NAMES),
        *(f"scenario/{name}" for name in SCENARIO_FILES),
    }
)
if len(STAGED_NON_SELF_PATHS) != 45:
    raise RuntimeError("candidate stage contract must contain exactly 45 non-self files")

DIGEST_SEEDS = frozenset(name for name in SCENARIO_FILES if name.endswith(".bin"))
POSITIVE_DECIMAL_SEEDS = frozenset(
    name
    for name in SCENARIO_FILES
    if name.endswith("-block-height.txt") or name.endswith("-verified-at-ms.txt")
)
MAX_CANDIDATE_METADATA_BYTES = 1024 * 1024
MAX_ROSTER_BYTES = 2 * 1024 * 1024
MAX_ARTIFACT_BYTES = 5 * 1024 * 1024 * 1024
MAX_SCENARIO_BYTES = 16 * 1024 * 1024


class StageError(RuntimeError):
    """An input or current-source invariant failed closed."""


class StagePublicationUncertain(StageError):
    """The stage was renamed, but its durable publication cannot be confirmed."""


@dataclass(frozen=True)
class SourceIdentity:
    commit: str
    tree_sha256: str


@dataclass(frozen=True)
class AuthorityBinaries:
    candidate: Path
    scenario: Path
    candidate_sha256: str
    scenario_sha256: str
    identity: dict[str, Any]


def _identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


class OpenedRegular:
    def __init__(
        self,
        directory_fd: int,
        directory_path: Path,
        name: str,
        maximum: int,
    ) -> None:
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        try:
            self.fd = os.open(name, flags, dir_fd=directory_fd)
        except OSError as exc:
            raise StageError(f"failed to open required regular input {directory_path / name}: {exc}") from exc
        self.directory_fd = directory_fd
        self.path = directory_path / name
        self.name = name
        self.snapshot = os.fstat(self.fd)
        current = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(self.snapshot.st_mode)
            or self.snapshot.st_nlink != 1
            or self.snapshot.st_uid != os.geteuid()
            or stat.S_IMODE(self.snapshot.st_mode) & 0o077
            or self.snapshot.st_size <= 0
            or self.snapshot.st_size > maximum
            or _identity(self.snapshot) != _identity(current)
        ):
            os.close(self.fd)
            raise StageError(
                f"input must be owner-private, nonempty, singly linked, regular, and bounded: {self.path}"
            )

    def __enter__(self) -> "OpenedRegular":
        return self

    def __exit__(self, *_: object) -> None:
        os.close(self.fd)

    def verify(self) -> None:
        opened = os.fstat(self.fd)
        current = os.stat(self.name, dir_fd=self.directory_fd, follow_symlinks=False)
        if _identity(opened) != _identity(self.snapshot) or _identity(current) != _identity(self.snapshot):
            raise StageError(f"input changed or was replaced while staging: {self.path}")

    def iter_chunks(self) -> Iterator[bytes]:
        self.verify()
        os.lseek(self.fd, 0, os.SEEK_SET)
        total = 0
        while True:
            chunk = os.read(self.fd, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            yield chunk
        if total != self.snapshot.st_size:
            raise StageError(f"input length changed while staging: {self.path}")
        os.lseek(self.fd, 0, os.SEEK_SET)
        self.verify()

    def bytes(self) -> bytes:
        return b"".join(self.iter_chunks())

    def sha256(self) -> str:
        digest = hashlib.sha256()
        for chunk in self.iter_chunks():
            digest.update(chunk)
        return digest.hexdigest()

    def copy_to(self, destination: Path, expected_sha256: str | None = None) -> str:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
        descriptor = os.open(destination, flags, 0o600)
        digest = hashlib.sha256()
        try:
            for chunk in self.iter_chunks():
                digest.update(chunk)
                view = memoryview(chunk)
                while view:
                    written = os.write(descriptor, view)
                    if written <= 0:
                        raise StageError(f"short write while staging {destination}")
                    view = view[written:]
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        actual = digest.hexdigest()
        if expected_sha256 is not None and actual != expected_sha256:
            raise StageError(f"validated digest changed while copying {self.path}")
        self.verify()
        return actual


class OpenedDirectory:
    def __init__(self, path: Path, expected: frozenset[str]) -> None:
        self.path = path
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
        flags |= getattr(os, "O_NOFOLLOW", 0)
        try:
            self.fd = os.open(path, flags)
        except OSError as exc:
            raise StageError(f"input directory must be a real directory: {path}: {exc}") from exc
        opened = os.fstat(self.fd)
        current = os.stat(path, follow_symlinks=False)
        if (
            not stat.S_ISDIR(opened.st_mode)
            or opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) & 0o077
            or _identity(opened) != _identity(current)
        ):
            os.close(self.fd)
            raise StageError(f"input directory must be owner-private and must not be a symlink: {path}")
        self.snapshot = opened
        self.expected = expected
        actual = frozenset(os.listdir(self.fd))
        if actual != expected:
            os.close(self.fd)
            missing = sorted(expected - actual)
            extra = sorted(actual - expected)
            raise StageError(f"invalid inventory in {path}; missing={missing}, extra={extra}")

    def __enter__(self) -> "OpenedDirectory":
        return self

    def __exit__(self, *_: object) -> None:
        os.close(self.fd)

    def open(self, name: str, maximum: int) -> OpenedRegular:
        return OpenedRegular(self.fd, self.path, name, maximum)

    def verify(self) -> None:
        opened = os.fstat(self.fd)
        current = os.stat(self.path, follow_symlinks=False)
        if _identity(opened) != _identity(self.snapshot) or _identity(current) != _identity(
            self.snapshot
        ):
            raise StageError(f"input directory changed or was replaced: {self.path}")
        actual = frozenset(os.listdir(self.fd))
        if actual != self.expected:
            raise StageError(f"input directory inventory changed while staging: {self.path}")


def _run_git(*arguments: str) -> bytes:
    try:
        return subprocess.run(
            ["git", "-C", str(REPOSITORY_ROOT), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=_safe_system_environment(),
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise StageError(f"git {' '.join(arguments)} failed") from exc


def _safe_system_environment(temporary: Path | None = None) -> dict[str, str]:
    candidates = (Path("/usr/bin"), Path("/bin"), Path("/usr/sbin"), Path("/sbin"))
    safe_paths: list[str] = []
    for directory in candidates:
        try:
            metadata = directory.stat()
        except OSError:
            continue
        if (
            stat.S_ISDIR(metadata.st_mode)
            and metadata.st_uid == 0
            and not stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            safe_paths.append(os.fspath(directory))
    if not safe_paths:
        raise StageError("no root-owned system executable path is available")
    home = Path(pwd.getpwuid(os.geteuid()).pw_dir).resolve(strict=True)
    temporary = temporary or Path("/tmp")
    return {
        "HOME": os.fspath(home),
        "PATH": os.pathsep.join(safe_paths),
        "LANG": "C",
        "LC_ALL": "C",
        "TMPDIR": os.fspath(temporary),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_TERMINAL_PROMPT": "0",
    }


SOURCE_IDENTITY_KEYS = frozenset(
    {"schema", "source_commit", "source_repo_dirty", "source_tree_sha256"}
)


def _canonical_json(value: Mapping[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n"
    ).encode("utf-8")


def _source_identity() -> SourceIdentity:
    try:
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                str(REPOSITORY_ROOT / "scripts/kagemusha_source_tree_seal.py"),
                "identity",
                "--root",
                str(REPOSITORY_ROOT),
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=_safe_system_environment(),
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise StageError("current Iroha source-tree identity failed") from exc
    payload = completed.stdout
    if len(payload) > 1024 or not payload.endswith(b"\n"):
        raise StageError("current Iroha source-tree identity is oversized or non-canonical")
    try:
        parsed = _exact_object(
            json.loads(
                payload.decode("utf-8"),
                object_pairs_hook=_strict_json_object,
                parse_constant=_reject_json_constant,
            ),
            SOURCE_IDENTITY_KEYS,
            "source-tree identity",
        )
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise StageError("current Iroha source-tree identity is not strict JSON") from exc
    if payload != _canonical_json(parsed):
        raise StageError("current Iroha source-tree identity is not canonical JSON")
    if (
        parsed["schema"] != "iroha.kagemusha.full_source_tree_identity.v1"
        or parsed["source_repo_dirty"] is not True
        or not isinstance(parsed["source_commit"], str)
        or not COMMIT_RE.fullmatch(parsed["source_commit"])
        or not isinstance(parsed["source_tree_sha256"], str)
        or not SHA256_RE.fullmatch(parsed["source_tree_sha256"])
        or parsed["source_tree_sha256"] == "0" * 64
    ):
        raise StageError("current Iroha source-tree identity is malformed or dirty")
    return SourceIdentity(
        commit=parsed["source_commit"], tree_sha256=parsed["source_tree_sha256"]
    )


def verify_current_source() -> SourceIdentity:
    discovered = Path(os.fsdecode(_run_git("rev-parse", "--show-toplevel")).strip()).resolve()
    if discovered != REPOSITORY_ROOT:
        raise StageError("stager is not running from its exact Iroha source repository")
    first = _source_identity()
    try:
        subprocess.run(
            ["git", "-C", str(REPOSITORY_ROOT), "verify-commit", first.commit],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            env=_safe_system_environment(),
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise StageError("current Iroha HEAD must carry a locally verifiable signature") from exc
    second = _source_identity()
    if second != first:
        raise StageError("Iroha source commit/tree pair changed during signature verification")
    return first


def _file_sha256(path: Path, *, private: bool) -> str:
    before = os.stat(path, follow_symlinks=False)
    if (
        not stat.S_ISREG(before.st_mode)
        or (private and before.st_nlink != 1)
        or (private and before.st_uid != os.geteuid())
        or (private and stat.S_IMODE(before.st_mode) & 0o077)
        or before.st_size <= 0
    ):
        raise StageError(f"validator/tool path is not an acceptable regular file: {path}")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    digest = hashlib.sha256()
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            raise StageError(f"validator/tool changed before open: {path}")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        after_open = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after_path = os.stat(path, follow_symlinks=False)
    if _identity(after_open) != _identity(before) or _identity(after_path) != _identity(before):
        raise StageError(f"validator/tool changed while hashed: {path}")
    return digest.hexdigest()


def _toolchain_path(name: str) -> Path:
    account_home = Path(pwd.getpwuid(os.geteuid()).pw_dir)
    rustup_candidates = (account_home / ".cargo/bin/rustup", Path("/usr/bin/rustup"))
    rustup_path = next((path for path in rustup_candidates if path.is_file()), None)
    if rustup_path is not None:
        try:
            value = subprocess.run(
                [os.fspath(rustup_path), "which", name],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=_safe_system_environment(),
            ).stdout.decode("utf-8").strip()
        except (OSError, UnicodeError, subprocess.CalledProcessError) as exc:
            raise StageError(f"failed to resolve the active Rust {name}") from exc
        path = Path(value)
    else:
        candidates = (account_home / f".cargo/bin/{name}", Path(f"/usr/bin/{name}"))
        selected = next((path for path in candidates if path.is_file()), None)
        if selected is None:
            raise StageError(f"Rust toolchain executable is unavailable: {name}")
        path = selected
    path = path.resolve(strict=True)
    if not path.is_absolute():
        raise StageError(f"Rust toolchain path is not absolute: {path}")
    return path


def _version_verbose(path: Path) -> str:
    try:
        payload = subprocess.run(
            [str(path), "-Vv"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=_safe_system_environment(),
        ).stdout
        text = payload.decode("utf-8")
    except (OSError, UnicodeError, subprocess.CalledProcessError) as exc:
        raise StageError(f"failed to identify Rust tool: {path}") from exc
    if not text.endswith("\n") or "\x00" in text or len(text) > 4096:
        raise StageError(f"Rust tool identity is non-canonical: {path}")
    return text


def _private_umask() -> None:
    os.umask(0o077)


def _audit_repository_cargo_config() -> None:
    forbidden_build = {
        "target",
        "target-dir",
        "rustc",
        "rustc-wrapper",
        "rustc-workspace-wrapper",
        "rustdoc",
        "rustflags",
        "rustdocflags",
        "incremental",
    }
    forbidden_environment = {
        "RUSTC",
        "RUSTDOC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "RUSTDOCFLAGS",
        "RUSTC_BOOTSTRAP",
        "CC",
        "CXX",
        "CPP",
        "CFLAGS",
        "CXXFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "AR",
        "RANLIB",
    }
    for name in ("config.toml", "config"):
        path = REPOSITORY_ROOT / ".cargo" / name
        if not path.exists():
            continue
        try:
            with path.open("rb") as source:
                config = tomllib.load(source)
        except (OSError, tomllib.TOMLDecodeError) as exc:
            raise StageError(f"repository Cargo config is unreadable: {path}") from exc
        build = config.get("build", {})
        if not isinstance(build, dict) or forbidden_build.intersection(build):
            raise StageError("repository Cargo config contains an authority build override")
        targets = config.get("target", {})
        if not isinstance(targets, dict):
            raise StageError("repository Cargo target config is malformed")
        for target_config in targets.values():
            if not isinstance(target_config, dict) or {
                "runner",
                "linker",
                "rustflags",
                "rustdocflags",
            }.intersection(target_config):
                raise StageError("repository Cargo config contains a target authority override")
        configured_environment = config.get("env", {})
        if not isinstance(configured_environment, dict) or forbidden_environment.intersection(
            configured_environment
        ):
            raise StageError("repository Cargo config injects an authority build environment")
        if "alias" in config:
            raise StageError("repository Cargo aliases are forbidden for authority builds")


def _authority_build_command(cargo: Path) -> list[str]:
    return [
        os.fspath(cargo),
        "build",
        "--manifest-path",
        os.fspath(REPOSITORY_ROOT / "Cargo.toml"),
        "--locked",
        "--offline",
        "--jobs",
        "2",
        "-p",
        "iroha_core",
        "--bin",
        "kagemusha_recursive_spend_v4_bundle",
        "-p",
        "connect_norito_bridge",
        "--bin",
        "kagemusha_candidate_scenario_validator",
        "--features",
        ",".join(AUTHORITY_BUILD_FEATURES),
    ]


def build_authoritative_validators(build_root: Path) -> AuthorityBinaries:
    build_root.mkdir(mode=0o700)
    build_root.chmod(0o700)
    cargo = _toolchain_path("cargo")
    rustc = _toolchain_path("rustc")
    cargo_home = build_root / "cargo-home"
    target = build_root / "target"
    private_home = build_root / "home"
    build_temporary = build_root / "tmp"
    cargo_home.mkdir(mode=0o700)
    target.mkdir(mode=0o700)
    private_home.mkdir(mode=0o700)
    build_temporary.mkdir(mode=0o700)
    _audit_repository_cargo_config()
    ambient_cargo_home = (
        Path(pwd.getpwuid(os.geteuid()).pw_dir) / ".cargo"
    ).resolve(strict=True)
    for cache_name in ("registry", "git"):
        cache = ambient_cargo_home / cache_name
        if cache.exists():
            os.symlink(cache, cargo_home / cache_name, target_is_directory=True)

    environment = _safe_system_environment(build_temporary)
    environment.update(
        {
            "HOME": os.fspath(private_home),
            "CARGO_HOME": os.fspath(cargo_home),
            "CARGO_TARGET_DIR": os.fspath(target),
            "CARGO_BUILD_JOBS": "2",
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "RUSTC": os.fspath(rustc),
        }
    )
    for variable, choices in {
        "CC": (Path("/usr/bin/clang"), Path("/usr/bin/cc")),
        "CXX": (Path("/usr/bin/clang++"), Path("/usr/bin/c++")),
        "AR": (Path("/usr/bin/ar"),),
        "RANLIB": (Path("/usr/bin/ranlib"),),
    }.items():
        selected = next((path for path in choices if path.is_file()), None)
        if selected is not None:
            environment[variable] = os.fspath(selected)
    command = _authority_build_command(cargo)
    nice = next((path for path in (Path("/usr/bin/nice"), Path("/bin/nice")) if path.is_file()), None)
    if nice is not None:
        command = [os.fspath(nice), "-n", "10", *command]
    try:
        completed = subprocess.run(
            command,
            cwd=build_root,
            env=environment,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            preexec_fn=_private_umask,
        )
    except OSError as exc:
        raise StageError("failed to start the isolated authoritative validator build") from exc
    if completed.returncode != 0:
        detail = completed.stderr[-8192:].decode("utf-8", errors="replace")
        raise StageError(f"isolated authoritative validator build failed:\n{detail}")

    candidate = target / "debug/kagemusha_recursive_spend_v4_bundle"
    scenario = target / "debug/kagemusha_candidate_scenario_validator"
    if candidate.name != "kagemusha_recursive_spend_v4_bundle" or scenario.name != (
        "kagemusha_candidate_scenario_validator"
    ):
        raise StageError("authoritative validator binary names changed")
    candidate_sha256 = _file_sha256(candidate, private=True)
    scenario_sha256 = _file_sha256(scenario, private=True)
    if candidate_sha256 == scenario_sha256:
        raise StageError("candidate and scenario authorities must be distinct binaries")
    identity = {
        "schema": VALIDATOR_SCHEMA,
        "candidate_binary_name": candidate.name,
        "candidate_binary_sha256": candidate_sha256,
        "scenario_binary_name": scenario.name,
        "scenario_binary_sha256": scenario_sha256,
        "cargo_binary_sha256": _file_sha256(cargo, private=False),
        "cargo_version_verbose": _version_verbose(cargo),
        "rustc_binary_sha256": _file_sha256(rustc, private=False),
        "rustc_version_verbose": _version_verbose(rustc),
        "locked": True,
        "offline": True,
        "isolated_target": True,
        "build_jobs": 2,
        "candidate_package": "iroha_core",
        "scenario_package": "connect_norito_bridge",
        "features": ["kagemusha-candidate-evidence-lab"],
        "profile": "debug",
    }
    return AuthorityBinaries(
        candidate=candidate,
        scenario=scenario,
        candidate_sha256=candidate_sha256,
        scenario_sha256=scenario_sha256,
        identity=identity,
    )


def _direct_validator_command(binary: Path, *arguments: str) -> list[str]:
    command = [os.fspath(binary), *arguments]
    nice = next((path for path in (Path("/usr/bin/nice"), Path("/bin/nice")) if path.is_file()), None)
    return [os.fspath(nice), "-n", "10", *command] if nice is not None else command


def run_authoritative_validators(
    authorities: AuthorityBinaries,
    candidate_dir: Path,
    scenario_dir: Path,
    output_dir: Path,
) -> dict[str, Any]:
    if (
        authorities.candidate.name != "kagemusha_recursive_spend_v4_bundle"
        or authorities.scenario.name != "kagemusha_candidate_scenario_validator"
        or authorities.candidate_sha256 == authorities.scenario_sha256
    ):
        raise StageError("authoritative candidate/scenario binary assignment is invalid")
    if (
        _file_sha256(authorities.candidate, private=True) != authorities.candidate_sha256
        or _file_sha256(authorities.scenario, private=True) != authorities.scenario_sha256
    ):
        raise StageError("authoritative validator binary changed before invocation")
    invocation_temporary = authorities.candidate.parent.parent / "invocation-tmp"
    invocation_temporary.mkdir(mode=0o700, exist_ok=True)
    invocation_temporary.chmod(0o700)
    environment = _safe_system_environment(invocation_temporary)
    environment["KAGEMUSHA_SOURCE_SEAL_PYTHON"] = sys.executable
    try:
        subprocess.run(
            _direct_validator_command(
                authorities.candidate,
                "validate-candidate",
                "--candidate-dir",
                os.fspath(candidate_dir),
                "--out-dir",
                os.fspath(output_dir),
            ),
            cwd=REPOSITORY_ROOT,
            env=environment,
            check=True,
        )
        scenario = subprocess.run(
            _direct_validator_command(
                authorities.scenario,
                "--candidate-record",
                os.fspath(candidate_dir / CANDIDATE_RECORD_NAME),
                "--candidate-roster",
                os.fspath(candidate_dir / ROSTER_NAME),
                "--scenario-dir",
                os.fspath(scenario_dir),
                "--account-chain-discriminant",
                str(PUBLIC_TAIRA_CHAIN_DISCRIMINANT),
            ),
            cwd=REPOSITORY_ROOT,
            env=environment,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise StageError("direct current-source candidate/scenario validation failed") from exc
    if (
        _file_sha256(authorities.candidate, private=True) != authorities.candidate_sha256
        or _file_sha256(authorities.scenario, private=True) != authorities.scenario_sha256
    ):
        raise StageError("authoritative validator binary changed during invocation")
    return parse_scenario_validation_report(scenario.stdout)


def _exact_object(value: Any, keys: frozenset[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or frozenset(value) != keys:
        raise StageError(f"{label} has an incomplete or excessive schema")
    return value


def _digest(value: Any, label: str) -> str:
    if not isinstance(value, str) or not SHA256_RE.fullmatch(value) or value == "0" * 64:
        raise StageError(f"{label} must be one nonzero lowercase SHA-256")
    return value


def _positive_int(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise StageError(f"{label} must be a positive integer")
    return value


def qualified_candidate_sha256(
    candidate_record_sha256: str,
    qualification_receipt_sha256: str,
) -> str:
    """Derive the domain-separated identity of one qualified candidate."""

    candidate_digest = bytes.fromhex(_digest(candidate_record_sha256, "candidate record digest"))
    receipt_digest = bytes.fromhex(
        _digest(qualification_receipt_sha256, "qualification receipt digest")
    )
    digest = hashlib.sha256()
    digest.update(QUALIFIED_CANDIDATE_DOMAIN)
    digest.update(b"\0")
    digest.update(candidate_digest)
    digest.update(receipt_digest)
    return digest.hexdigest()


def _strict_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise StageError(f"candidate validation report repeats JSON key {key!r}")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise StageError(f"candidate validation report contains {value}")


REPORT_KEYS = frozenset(
    {
        "schema",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "qualification_receipt_file_name",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "generation",
        "generation_memory_limit_bytes",
        "generation_memory_enforcement_profile",
        "bridge_abi_version",
        "artifact_count",
        "artifacts",
        "topup_finality_roster_file_name",
        "topup_finality_roster_size_bytes",
        "topup_finality_roster_sha256",
    }
)
ARTIFACT_REPORT_KEYS = frozenset(
    {
        "role",
        "file_name",
        "framed_size_bytes",
        "framed_sha256",
        "payload_size_bytes",
        "payload_sha256",
    }
)
VALIDATOR_KEYS = frozenset(
    {
        "schema",
        "candidate_binary_name",
        "candidate_binary_sha256",
        "scenario_binary_name",
        "scenario_binary_sha256",
        "cargo_binary_sha256",
        "cargo_version_verbose",
        "rustc_binary_sha256",
        "rustc_version_verbose",
        "locked",
        "offline",
        "isolated_target",
        "build_jobs",
        "candidate_package",
        "scenario_package",
        "features",
        "profile",
    }
)
SCENARIO_VALIDATION_KEYS = frozenset(
    {
        "schema",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "scenario_inventory_sha256",
        "scenario_file_count",
        "finalized_height",
    }
)


def parse_validation_report(payload: bytes) -> dict[str, Any]:
    if len(payload) > MAX_CANDIDATE_METADATA_BYTES or not payload.endswith(b"\n"):
        raise StageError("candidate validation report is oversized or non-canonical")
    try:
        text = payload.decode("utf-8")
        report = _exact_object(
            json.loads(
                text,
                object_pairs_hook=_strict_json_object,
                parse_constant=_reject_json_constant,
            ),
            REPORT_KEYS,
            "candidate validation report",
        )
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise StageError("candidate validation report is not strict JSON") from exc
    if report["schema"] != REPORT_SCHEMA:
        raise StageError("candidate validation report schema is unsupported")
    _digest(report["candidate_record_sha256"], "candidate record digest")
    _digest(report["candidate_manifest_sha256"], "candidate manifest digest")
    if report["candidate_record_sha256"] == report["candidate_manifest_sha256"]:
        raise StageError("candidate record and embedded manifest must have distinct identities")
    if report["qualification_receipt_file_name"] != QUALIFICATION_RECEIPT_NAME:
        raise StageError("candidate report names a non-canonical qualification receipt")
    receipt_sha256 = _digest(
        report["qualification_receipt_sha256"],
        "qualification receipt digest",
    )
    qualified_sha256 = _digest(
        report["qualified_candidate_sha256"],
        "qualified candidate digest",
    )
    if qualified_sha256 != qualified_candidate_sha256(
        report["candidate_record_sha256"], receipt_sha256
    ):
        raise StageError("candidate report has an invalid qualified-candidate identity")
    if not isinstance(report["source_commit"], str) or not COMMIT_RE.fullmatch(report["source_commit"]):
        raise StageError("candidate source commit is not canonical")
    _digest(report["source_tree_sha256"], "candidate source-tree digest")
    if report["source_repo_dirty"] is not True:
        raise StageError("candidate reports a dirty source repository")
    if not isinstance(report["generation"], str) or not PORTABLE_ID_RE.fullmatch(report["generation"]):
        raise StageError("candidate generation is not portable")
    memory_limit = _positive_int(
        report["generation_memory_limit_bytes"],
        "candidate generation memory limit",
    )
    if memory_limit > MAX_GENERATION_MEMORY_BYTES:
        raise StageError("candidate generation memory limit exceeds the 64 GiB ceiling")
    if (
        report["generation_memory_enforcement_profile"]
        != GENERATION_MEMORY_ENFORCEMENT_PROFILE
    ):
        raise StageError("candidate generation memory enforcement profile is unsupported")
    if report["bridge_abi_version"] != 21 or report["artifact_count"] != len(ARTIFACTS):
        raise StageError("candidate is not the exact ABI-21 eight-artifact profile")
    artifacts = report["artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != len(ARTIFACTS):
        raise StageError("candidate validation report has the wrong artifact count")
    seen_digests: set[str] = set()
    for index, ((expected_role, expected_name), raw) in enumerate(zip(ARTIFACTS, artifacts)):
        artifact = _exact_object(raw, ARTIFACT_REPORT_KEYS, f"artifact report {index}")
        if artifact["role"] != expected_role or artifact["file_name"] != expected_name:
            raise StageError("candidate validation artifact order or role is non-canonical")
        framed_size = _positive_int(artifact["framed_size_bytes"], "framed artifact size")
        payload_size = _positive_int(artifact["payload_size_bytes"], "artifact payload size")
        if framed_size > MAX_ARTIFACT_BYTES or payload_size >= framed_size:
            raise StageError("candidate artifact sizes exceed the V4 corridor")
        framed_digest = _digest(artifact["framed_sha256"], "framed artifact digest")
        payload_digest = _digest(artifact["payload_sha256"], "artifact payload digest")
        if framed_digest in seen_digests or payload_digest in seen_digests or framed_digest == payload_digest:
            raise StageError("candidate artifact content addresses must all be distinct")
        seen_digests.update((framed_digest, payload_digest))
    if report["topup_finality_roster_file_name"] != ROSTER_NAME:
        raise StageError("candidate report names a non-canonical top-up finality roster")
    roster_size = _positive_int(report["topup_finality_roster_size_bytes"], "roster size")
    if roster_size > MAX_ROSTER_BYTES:
        raise StageError("candidate top-up finality roster exceeds its bound")
    _digest(report["topup_finality_roster_sha256"], "roster digest")
    return report


def parse_scenario_validation_report(payload: bytes) -> dict[str, Any]:
    if len(payload) > 4096 or not payload.endswith(b"\n"):
        raise StageError("scenario validation report is oversized or non-canonical")
    try:
        report = _exact_object(
            json.loads(
                payload.decode("utf-8"),
                object_pairs_hook=_strict_json_object,
                parse_constant=_reject_json_constant,
            ),
            SCENARIO_VALIDATION_KEYS,
            "scenario validation report",
        )
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise StageError("scenario validation report is not strict JSON") from exc
    if payload != _canonical_json(report):
        raise StageError("scenario validation report is not canonical JSON")
    if report["schema"] != SCENARIO_VALIDATION_SCHEMA:
        raise StageError("scenario validation report schema is unsupported")
    _digest(report["candidate_record_sha256"], "scenario candidate record digest")
    _digest(report["candidate_manifest_sha256"], "scenario candidate manifest digest")
    _digest(report["scenario_inventory_sha256"], "scenario inventory digest")
    if report["scenario_file_count"] != len(SCENARIO_FILES):
        raise StageError("scenario authority did not validate exactly 33 files")
    _positive_int(report["finalized_height"], "scenario finalized height")
    return report


def validate_validator_identity(identity: Any) -> dict[str, Any]:
    validator = _exact_object(identity, VALIDATOR_KEYS, "validator identity")
    if (
        validator["schema"] != VALIDATOR_SCHEMA
        or validator["candidate_binary_name"] != "kagemusha_recursive_spend_v4_bundle"
        or validator["scenario_binary_name"] != "kagemusha_candidate_scenario_validator"
        or validator["candidate_package"] != "iroha_core"
        or validator["scenario_package"] != "connect_norito_bridge"
        or validator["features"] != ["kagemusha-candidate-evidence-lab"]
        or validator["profile"] != "debug"
        or validator["locked"] is not True
        or validator["offline"] is not True
        or validator["isolated_target"] is not True
        or validator["build_jobs"] != 2
    ):
        raise StageError("validator identity does not describe the frozen authority build")
    for key in (
        "candidate_binary_sha256",
        "scenario_binary_sha256",
        "cargo_binary_sha256",
        "rustc_binary_sha256",
    ):
        _digest(validator[key], key)
    if validator["candidate_binary_sha256"] == validator["scenario_binary_sha256"]:
        raise StageError("candidate and scenario validator binaries are not distinct")
    for key in ("cargo_version_verbose", "rustc_version_verbose"):
        value = validator[key]
        if (
            not isinstance(value, str)
            or not value.endswith("\n")
            or not value.strip()
            or "\x00" in value
            or "\r" in value
            or len(value) > 4096
        ):
            raise StageError(f"{key} is not one bounded verbose tool identity")
    return validator


def scenario_inventory_sha256(files: Mapping[str, OpenedRegular]) -> str:
    if frozenset(files) != SCENARIO_INVENTORY:
        raise StageError("scenario inventory digest requires the exact 33 files")
    digest = hashlib.sha256()
    digest.update(SCENARIO_INVENTORY_DOMAIN)
    digest.update(len(files).to_bytes(4, "big"))
    for name in sorted(files):
        path = f"scenario/{name}".encode("utf-8")
        opened = files[name]
        digest.update(len(path).to_bytes(4, "big"))
        digest.update(path)
        digest.update(opened.snapshot.st_size.to_bytes(8, "big"))
        digest.update(bytes.fromhex(opened.sha256()))
    return digest.hexdigest()


def _write_bytes(path: Path, payload: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise StageError(f"short write while staging {path}")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _mkdir(path: Path) -> None:
    path.mkdir(mode=0o700)
    path.chmod(0o700)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _ensure_output_parent() -> int:
    OUTPUT_PARENT.mkdir(mode=0o700, parents=True, exist_ok=True)
    metadata = os.stat(OUTPUT_PARENT, follow_symlinks=False)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or OUTPUT_PARENT.is_symlink()
    ):
        raise StageError(f"candidate output parent is not trusted: {OUTPUT_PARENT}")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(OUTPUT_PARENT, flags)
    if _identity(os.fstat(descriptor))[:2] != _identity(metadata)[:2]:
        os.close(descriptor)
        raise StageError("candidate output parent changed while being pinned")
    return descriptor


def _publish_noreplace(
    parent_fd: int,
    parent_path: Path,
    source_name: str,
    destination_name: str,
) -> None:
    opened_parent = os.fstat(parent_fd)
    named_parent = os.stat(parent_path, follow_symlinks=False)
    if (
        not stat.S_ISDIR(opened_parent.st_mode)
        or opened_parent.st_uid != os.geteuid()
        or stat.S_IMODE(opened_parent.st_mode) != 0o700
        or _identity(opened_parent)[:2] != _identity(named_parent)[:2]
    ):
        raise StageError("candidate publication parent changed after it was pinned")
    source_metadata = os.stat(source_name, dir_fd=parent_fd, follow_symlinks=False)
    if (
        not stat.S_ISDIR(source_metadata.st_mode)
        or source_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(source_metadata.st_mode) != 0o700
    ):
        raise StageError("candidate staging source is not one trusted private directory")
    try:
        os.stat(destination_name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        raise StageError(
            f"candidate evidence root already exists: {parent_path / destination_name}"
        )

    libc = ctypes.CDLL(None, use_errno=True)
    source = os.fsencode(source_name)
    target = os.fsencode(destination_name)
    result: int
    if sys.platform == "darwin" and hasattr(libc, "renameatx_np"):
        renameatx_np = libc.renameatx_np
        renameatx_np.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        renameatx_np.restype = ctypes.c_int
        result = renameatx_np(
            parent_fd,
            source,
            parent_fd,
            target,
            0x00000004,
        )  # RENAME_EXCL
    elif hasattr(libc, "renameat2"):
        renameat2 = libc.renameat2
        renameat2.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        renameat2.restype = ctypes.c_int
        result = renameat2(
            parent_fd,
            source,
            parent_fd,
            target,
            0x00000001,
        )  # RENAME_NOREPLACE
    else:
        raise StageError("exclusive atomic directory publication is unsupported on this platform")
    if result != 0:
        error = ctypes.get_errno()
        if error in (errno.EEXIST, errno.ENOTEMPTY):
            raise StageError(
                f"candidate evidence root already exists: {parent_path / destination_name}"
            )
        raise StageError(f"exclusive candidate publication failed: {os.strerror(error)}")
    try:
        published = os.stat(destination_name, dir_fd=parent_fd, follow_symlinks=False)
        if _identity(published)[:2] != _identity(source_metadata)[:2]:
            raise StageError("published candidate has the wrong directory identity")
        os.fsync(parent_fd)
        named_parent = os.stat(parent_path, follow_symlinks=False)
        if _identity(os.fstat(parent_fd))[:2] != _identity(named_parent)[:2]:
            raise StageError("candidate publication parent pathname changed after rename")
    except (OSError, StageError) as exc:
        raise StagePublicationUncertain(
            "candidate stage reached its final name but publication durability or path continuity "
            f"is uncertain: {parent_path / destination_name}: {exc}"
        ) from exc


STAGE_MANIFEST_KEYS = frozenset(
    {
        "schema",
        "version",
        "stage_manifest_path",
        "stage_manifest_mode",
        "stage_manifest_size_bytes",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "candidate_validation_report_sha256",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "scenario_inventory_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "validator",
        "entry_count",
        "scenario_entry_count",
        "entries",
    }
)
STAGE_ENTRY_KEYS = frozenset({"path", "mode", "size_bytes", "sha256"})


def _stage_file_entry(root: Path, relative: str) -> dict[str, Any]:
    path = root / relative
    metadata = os.stat(path, follow_symlinks=False)
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o600
        or metadata.st_size <= 0
    ):
        raise StageError(f"staged file is not one owner-private regular file: {relative}")
    return {
        "path": relative,
        "mode": "0600",
        "size_bytes": metadata.st_size,
        "sha256": _file_sha256(path, private=True),
    }


def _stage_tree_files(root: Path) -> frozenset[str]:
    files: set[str] = set()
    for current, directories, names in os.walk(root, topdown=True, followlinks=False):
        current_path = Path(current)
        current_metadata = os.stat(current_path, follow_symlinks=False)
        if (
            not stat.S_ISDIR(current_metadata.st_mode)
            or current_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(current_metadata.st_mode) != 0o700
        ):
            raise StageError(f"staged directory is not owner-private: {current_path}")
        for name in directories:
            child = current_path / name
            metadata = os.stat(child, follow_symlinks=False)
            if not stat.S_ISDIR(metadata.st_mode) or child.is_symlink():
                raise StageError(f"staged tree contains a non-directory or symlink: {child}")
        for name in names:
            child = current_path / name
            relative = child.relative_to(root).as_posix()
            if (
                relative.startswith("/")
                or any(part in ("", ".", "..") for part in relative.split("/"))
                or relative in files
            ):
                raise StageError("staged tree contains an unsafe or duplicate path")
            files.add(relative)
    return frozenset(files)


def build_stage_manifest(
    root: Path,
    *,
    report: Mapping[str, Any],
    report_bytes: bytes,
    scenario_inventory: str,
    source: SourceIdentity,
    validator: Mapping[str, Any],
) -> bytes:
    actual = _stage_tree_files(root)
    if actual != STAGED_NON_SELF_PATHS:
        raise StageError(
            f"staged non-self inventory differs; missing={sorted(STAGED_NON_SELF_PATHS - actual)}, "
            f"extra={sorted(actual - STAGED_NON_SELF_PATHS)}"
        )
    entries = [_stage_file_entry(root, path) for path in sorted(STAGED_NON_SELF_PATHS)]
    manifest: dict[str, Any] = {
        "schema": STAGE_MANIFEST_SCHEMA,
        "version": 2,
        "stage_manifest_path": STAGE_MANIFEST_NAME,
        "stage_manifest_mode": "0600",
        "stage_manifest_size_bytes": 0,
        "candidate_record_sha256": report["candidate_record_sha256"],
        "candidate_manifest_sha256": report["candidate_manifest_sha256"],
        "candidate_validation_report_sha256": hashlib.sha256(report_bytes).hexdigest(),
        "qualification_receipt_sha256": report["qualification_receipt_sha256"],
        "qualified_candidate_sha256": report["qualified_candidate_sha256"],
        "scenario_inventory_sha256": scenario_inventory,
        "source_commit": source.commit,
        "source_tree_sha256": source.tree_sha256,
        "source_repo_dirty": True,
        "validator": validate_validator_identity(dict(validator)),
        "entry_count": len(entries),
        "scenario_entry_count": len(SCENARIO_FILES),
        "entries": entries,
    }
    for _ in range(16):
        payload = _canonical_json(manifest)
        size = len(payload)
        if manifest["stage_manifest_size_bytes"] == size:
            return payload
        manifest["stage_manifest_size_bytes"] = size
    raise StageError("stage manifest self-size did not reach a fixed point")


def parse_stage_manifest(
    payload: bytes,
    *,
    root: Path | None = None,
    expected_sha256: str | None = None,
) -> dict[str, Any]:
    if len(payload) > MAX_CANDIDATE_METADATA_BYTES or not payload.endswith(b"\n"):
        raise StageError("candidate stage manifest is oversized or non-canonical")
    try:
        manifest = _exact_object(
            json.loads(
                payload.decode("utf-8"),
                object_pairs_hook=_strict_json_object,
                parse_constant=_reject_json_constant,
            ),
            STAGE_MANIFEST_KEYS,
            "candidate stage manifest",
        )
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise StageError("candidate stage manifest is not strict JSON") from exc
    if payload != _canonical_json(manifest):
        raise StageError("candidate stage manifest is not canonical JSON")
    if (
        manifest["schema"] != STAGE_MANIFEST_SCHEMA
        or manifest["version"] != 2
        or manifest["stage_manifest_path"] != STAGE_MANIFEST_NAME
        or manifest["stage_manifest_mode"] != "0600"
        or manifest["stage_manifest_size_bytes"] != len(payload)
        or manifest["source_repo_dirty"] is not True
        or manifest["entry_count"] != len(STAGED_NON_SELF_PATHS)
        or manifest["scenario_entry_count"] != len(SCENARIO_FILES)
    ):
        raise StageError("candidate stage manifest header is invalid")
    if not isinstance(manifest["source_commit"], str) or not COMMIT_RE.fullmatch(
        manifest["source_commit"]
    ):
        raise StageError("candidate stage source commit is invalid")
    for key in (
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "candidate_validation_report_sha256",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "scenario_inventory_sha256",
        "source_tree_sha256",
    ):
        _digest(manifest[key], key)
    if manifest["qualified_candidate_sha256"] != qualified_candidate_sha256(
        manifest["candidate_record_sha256"],
        manifest["qualification_receipt_sha256"],
    ):
        raise StageError("candidate stage qualified-candidate identity is invalid")
    validate_validator_identity(manifest["validator"])
    raw_entries = manifest["entries"]
    if not isinstance(raw_entries, list) or len(raw_entries) != len(STAGED_NON_SELF_PATHS):
        raise StageError("candidate stage entry array has the wrong length")
    entries: list[dict[str, Any]] = []
    paths: list[str] = []
    for index, raw in enumerate(raw_entries):
        entry = _exact_object(raw, STAGE_ENTRY_KEYS, f"candidate stage entry {index}")
        path = entry["path"]
        if (
            not isinstance(path, str)
            or not path
            or path == STAGE_MANIFEST_NAME
            or path.startswith("/")
            or any(part in ("", ".", "..") for part in path.split("/"))
        ):
            raise StageError("candidate stage manifest contains an unsafe/self path")
        if entry["mode"] != "0600":
            raise StageError("candidate stage entry mode is not 0600")
        _positive_int(entry["size_bytes"], "candidate stage entry size")
        _digest(entry["sha256"], "candidate stage entry digest")
        paths.append(path)
        entries.append(entry)
    if paths != sorted(paths) or frozenset(paths) != STAGED_NON_SELF_PATHS:
        raise StageError("candidate stage manifest has missing, extra, duplicate, or unsorted paths")
    if expected_sha256 is not None and hashlib.sha256(payload).hexdigest() != expected_sha256:
        raise StageError("candidate stage manifest digest differs from its stage identity")
    if root is not None:
        actual = _stage_tree_files(root)
        expected_files = STAGED_NON_SELF_PATHS | frozenset({STAGE_MANIFEST_NAME})
        if actual != expected_files:
            raise StageError("published candidate stage contains missing or extra files")
        manifest_metadata = os.stat(root / STAGE_MANIFEST_NAME, follow_symlinks=False)
        if (
            not stat.S_ISREG(manifest_metadata.st_mode)
            or stat.S_IMODE(manifest_metadata.st_mode) != 0o600
            or manifest_metadata.st_size != len(payload)
            or _file_sha256(root / STAGE_MANIFEST_NAME, private=True)
            != hashlib.sha256(payload).hexdigest()
        ):
            raise StageError("candidate stage manifest self metadata is invalid")
        for expected, actual_entry in zip(entries, (_stage_file_entry(root, p) for p in paths)):
            if actual_entry != expected:
                raise StageError(f"staged entry changed: {expected['path']}")
        entry_by_path = {entry["path"]: entry for entry in entries}
        receipt_path = f"evidence/candidate/{QUALIFICATION_RECEIPT_NAME}"
        if (
            entry_by_path[receipt_path]["sha256"]
            != manifest["qualification_receipt_sha256"]
        ):
            raise StageError("candidate stage manifest does not bind its qualification receipt")
        candidate_directory = root / "evidence/candidate"
        directory_flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        candidate_directory_fd = os.open(candidate_directory, directory_flags)
        try:
            with OpenedRegular(
                candidate_directory_fd,
                candidate_directory,
                REPORT_NAME,
                MAX_CANDIDATE_METADATA_BYTES,
            ) as report_file:
                report = parse_validation_report(report_file.bytes())
        finally:
            os.close(candidate_directory_fd)
        if (
            report["candidate_record_sha256"] != manifest["candidate_record_sha256"]
            or report["candidate_manifest_sha256"]
            != manifest["candidate_manifest_sha256"]
            or report["qualification_receipt_sha256"]
            != manifest["qualification_receipt_sha256"]
            or report["qualified_candidate_sha256"]
            != manifest["qualified_candidate_sha256"]
        ):
            raise StageError("candidate validation report is not bound to the stage manifest")
    return manifest


def _ensure_candidate_parent(candidate_sha256: str, output_parent_fd: int) -> tuple[Path, int]:
    parent = OUTPUT_PARENT / candidate_sha256
    try:
        os.mkdir(candidate_sha256, mode=0o700, dir_fd=output_parent_fd)
        os.fsync(output_parent_fd)
    except FileExistsError:
        pass
    metadata = os.stat(candidate_sha256, dir_fd=output_parent_fd, follow_symlinks=False)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
    ):
        raise StageError(f"candidate identity parent is not trusted: {parent}")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(candidate_sha256, flags, dir_fd=output_parent_fd)
    if _identity(os.fstat(descriptor))[:2] != _identity(metadata)[:2]:
        os.close(descriptor)
        raise StageError("candidate identity parent changed while being pinned")
    return parent, descriptor


def stage_candidate(candidate_dir: Path, scenario_dir: Path) -> Path:
    for label, path in (("candidate", candidate_dir), ("scenario", scenario_dir)):
        if not path.is_absolute():
            raise StageError(f"{label} input directory must be an absolute canonical path")
        try:
            path.resolve(strict=True)
        except OSError as exc:
            raise StageError(f"{label} input directory is unavailable: {path}") from exc
        if Path(os.path.abspath(path)) != path:
            raise StageError(f"{label} input directory must not contain dot aliases")
    source_before = verify_current_source()
    with contextlib.ExitStack() as stack:
        candidate = stack.enter_context(OpenedDirectory(candidate_dir, CANDIDATE_INVENTORY))
        scenario = stack.enter_context(OpenedDirectory(scenario_dir, SCENARIO_INVENTORY))
        candidate_files: dict[str, OpenedRegular] = {}
        for name in sorted(CANDIDATE_INVENTORY):
            maximum = (
                MAX_ARTIFACT_BYTES
                if name in ARTIFACT_NAMES
                else MAX_ROSTER_BYTES
                if name == ROSTER_NAME
                else MAX_CANDIDATE_METADATA_BYTES
                if name == QUALIFICATION_RECEIPT_NAME
                else 65
                if name == CANDIDATE_SHA_NAME
                else MAX_CANDIDATE_METADATA_BYTES
            )
            candidate_files[name] = stack.enter_context(candidate.open(name, maximum))
        scenario_files = {
            name: stack.enter_context(scenario.open(name, MAX_SCENARIO_BYTES))
            for name in SCENARIO_FILES
        }
        scenario_inventory = scenario_inventory_sha256(scenario_files)

        with tempfile.TemporaryDirectory(
            prefix="kagemusha-candidate-authority-"
        ) as authority_temporary:
            authority_root = Path(authority_temporary)
            authority_root.chmod(0o700)
            authorities = build_authoritative_validators(authority_root / "isolated-build")
            validation_dir = authority_root / "validated"
            scenario_report = run_authoritative_validators(
                authorities,
                candidate_dir,
                scenario_dir,
                validation_dir,
            )
            validation = stack.enter_context(
                OpenedDirectory(
                    validation_dir,
                    frozenset({REPORT_NAME, VALIDATED_MANIFEST_NAME}),
                )
            )
            report_file = stack.enter_context(
                validation.open(REPORT_NAME, MAX_CANDIDATE_METADATA_BYTES)
            )
            manifest_file = stack.enter_context(
                validation.open(VALIDATED_MANIFEST_NAME, MAX_CANDIDATE_METADATA_BYTES)
            )
            report_bytes = report_file.bytes()
            manifest_bytes = manifest_file.bytes()
            report = parse_validation_report(report_bytes)

            source_after_validation = verify_current_source()
            if source_after_validation != source_before:
                raise StageError(
                    "Iroha source identity changed during authoritative candidate validation"
                )
            if (
                report["source_commit"] != source_before.commit
                or report["source_tree_sha256"] != source_before.tree_sha256
            ):
                raise StageError(
                    "candidate source identity differs from the exact current Iroha source"
                )

            record_digest = candidate_files[CANDIDATE_RECORD_NAME].sha256()
            if record_digest != report["candidate_record_sha256"]:
                raise StageError(
                    "candidate record differs from the authoritative validation report"
                )
            receipt_digest = candidate_files[QUALIFICATION_RECEIPT_NAME].sha256()
            if receipt_digest != report["qualification_receipt_sha256"]:
                raise StageError(
                    "qualification receipt differs from the authoritative validation report"
                )
            if report["qualified_candidate_sha256"] != qualified_candidate_sha256(
                record_digest, receipt_digest
            ):
                raise StageError(
                    "qualified candidate identity differs from the authoritative validation report"
                )
            digest_text = candidate_files[CANDIDATE_SHA_NAME].bytes()
            if digest_text != f"{record_digest}\n".encode("ascii"):
                raise StageError("candidate digest sidecar is not exact")
            manifest_digest = hashlib.sha256(manifest_bytes).hexdigest()
            if manifest_digest != report["candidate_manifest_sha256"]:
                raise StageError(
                    "extracted canonical manifest differs from its validated identity"
                )
            if (
                scenario_report["candidate_record_sha256"] != record_digest
                or scenario_report["candidate_manifest_sha256"] != manifest_digest
                or scenario_report["scenario_inventory_sha256"] != scenario_inventory
            ):
                raise StageError(
                    "typed scenario authority did not bind the candidate/staged seed inventory"
                )
            for artifact, raw in zip(ARTIFACTS, report["artifacts"]):
                _, name = artifact
                opened = candidate_files[name]
                if (
                    opened.snapshot.st_size != raw["framed_size_bytes"]
                    or opened.sha256() != raw["framed_sha256"]
                ):
                    raise StageError(
                        f"candidate artifact differs from authoritative validation: {name}"
                    )
            roster = candidate_files[ROSTER_NAME].bytes()
            if (
                len(roster) != report["topup_finality_roster_size_bytes"]
                or hashlib.sha256(roster).hexdigest()
                != report["topup_finality_roster_sha256"]
            ):
                raise StageError("candidate roster differs from authoritative validation")
            output_parent_fd = _ensure_output_parent()
            stack.callback(os.close, output_parent_fd)
            candidate_parent, candidate_parent_fd = _ensure_candidate_parent(
                record_digest,
                output_parent_fd,
            )
            stack.callback(os.close, candidate_parent_fd)
            staging = Path(
                tempfile.mkdtemp(prefix=".candidate-stage-staging-", dir=candidate_parent)
            )
            staging.chmod(0o700)
            published = False
            try:
                evidence = staging / "evidence"
                staged_candidate = evidence / "candidate"
                staged_artifacts = staged_candidate / "artifacts"
                staged_scenario = staging / "scenario"
                for directory in (evidence, staged_candidate, staged_artifacts, staged_scenario):
                    _mkdir(directory)
                candidate_files[CANDIDATE_RECORD_NAME].copy_to(
                    staged_candidate / "candidate-v4.norito", record_digest
                )
                _write_bytes(staged_candidate / VALIDATED_MANIFEST_NAME, manifest_bytes)
                _write_bytes(staged_candidate / REPORT_NAME, report_bytes)
                candidate_files[QUALIFICATION_RECEIPT_NAME].copy_to(
                    staged_candidate / QUALIFICATION_RECEIPT_NAME,
                    receipt_digest,
                )
                for _, name in ARTIFACTS:
                    expected = next(
                        item["framed_sha256"]
                        for item in report["artifacts"]
                        if item["file_name"] == name
                    )
                    candidate_files[name].copy_to(staged_artifacts / name, expected)
                for name in SCENARIO_FILES:
                    scenario_files[name].copy_to(staged_scenario / name)
                for directory in (
                    staged_artifacts,
                    staged_candidate,
                    evidence,
                    staged_scenario,
                    staging,
                ):
                    _fsync_directory(directory)

                stage_manifest_bytes = build_stage_manifest(
                    staging,
                    report=report,
                    report_bytes=report_bytes,
                    scenario_inventory=scenario_inventory,
                    source=source_before,
                    validator=authorities.identity,
                )
                stage_sha256 = hashlib.sha256(stage_manifest_bytes).hexdigest()
                _write_bytes(staging / STAGE_MANIFEST_NAME, stage_manifest_bytes)
                _fsync_directory(staging)
                parse_stage_manifest(
                    stage_manifest_bytes,
                    root=staging,
                    expected_sha256=stage_sha256,
                )

                for opened in (*candidate_files.values(), *scenario_files.values()):
                    opened.verify()
                candidate.verify()
                scenario.verify()
                validation.verify()
                report_file.verify()
                manifest_file.verify()
                if (
                    _file_sha256(authorities.candidate, private=True)
                    != authorities.candidate_sha256
                    or _file_sha256(authorities.scenario, private=True)
                    != authorities.scenario_sha256
                ):
                    raise StageError("authoritative validator changed before publication")
                source_before_publish = verify_current_source()
                if source_before_publish != source_before:
                    raise StageError(
                        "Iroha source identity changed while staging the candidate"
                    )
                final_root = candidate_parent / stage_sha256
                _publish_noreplace(
                    candidate_parent_fd,
                    candidate_parent,
                    staging.name,
                    stage_sha256,
                )
                published = True
                parse_stage_manifest(
                    stage_manifest_bytes,
                    root=final_root,
                    expected_sha256=stage_sha256,
                )
            finally:
                if not published:
                    try:
                        opened_parent = os.fstat(candidate_parent_fd)
                        named_parent = os.stat(candidate_parent, follow_symlinks=False)
                        staged_at = os.stat(
                            staging.name,
                            dir_fd=candidate_parent_fd,
                            follow_symlinks=False,
                        )
                        staged_by_path = os.stat(staging, follow_symlinks=False)
                    except (FileNotFoundError, OSError):
                        pass
                    else:
                        if (
                            _identity(opened_parent)[:2] == _identity(named_parent)[:2]
                            and _identity(staged_at)[:2] == _identity(staged_by_path)[:2]
                        ):
                            shutil.rmtree(staging)
            return final_root


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate and stage one source-sealed ABI-21 Android candidate lab"
    )
    parser.add_argument("--candidate-dir", type=Path, required=True)
    parser.add_argument("--scenario-seed-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    output = stage_candidate(args.candidate_dir, args.scenario_seed_dir)
    print(output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except StagePublicationUncertain as exc:
        print(f"Kagemusha Android candidate publication is uncertain: {exc}", file=sys.stderr)
        raise SystemExit(75) from exc
    except (OSError, StageError) as exc:
        print(f"Kagemusha Android candidate staging failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
