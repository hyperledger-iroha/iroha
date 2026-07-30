#!/usr/bin/env python3
"""Create and validate source-bound Verus evidence for Sumeragi v2.

The evidence is deliberately derived from the verifier transcript.  A caller
cannot choose the expected obligation counts, invocation, source inventory, or
tool digests.  Release validation recomputes each of those values from this
checked-in contract and the sealed checkout.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import secrets
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT_DIR = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT_DIR / "scripts"))

from compute_workspace_source_manifest import workspace_source_manifest  # noqa: E402


SCHEMA_VERSION = 2
EXPECTED_VERUS_VERSION = "0.2026.05.31.5dd6d83"
EXPECTED_DEPENDENCY_VERIFIED = 1690
# The local proof expansion adds 31 roots to the historical 126-root source;
# the proposal-origin closure adds one independent root.
EXPECTED_ROOT_VERIFIED = 172
EXPECTED_LOG_PATH = "target/formal/sumeragi_v2/verus.log"
EXPECTED_INVOCATION = (
    "bash",
    "scripts/formal/run_sumeragi_v2_harness.sh",
    "cargo",
    "verus",
    "verify",
    "--locked",
    "--offline",
    "-p",
    "iroha_sumeragi_core",
    "--features",
    "verus",
    "--fwd-verus-args-to",
    "roots",
    "--",
    "--rlimit",
    "60",
    "--expand-errors",
    "--no-cheating",
)

# These paths bind both the verified kernel and the ordinary production
# boundaries which remain part of the explicit refinement obligation.  Adding
# a new authoritative acquisition owner requires an intentional inventory
# change here; deleting one invalidates old evidence.
REQUIRED_SOURCE_PATHS = (
    "ci/check_sumeragi_formal.sh",
    "crates/iroha_sumeragi_core/Cargo.toml",
    "crates/iroha_sumeragi_core/src/effective_lock_verus_proofs.rs",
    "crates/iroha_sumeragi_core/src/lib.rs",
    "crates/iroha_sumeragi_core/src/verus_proofs.rs",
    "crates/iroha_core/src/sumeragi/v2.rs",
    "crates/iroha_core/src/sumeragi/status.rs",
    "crates/iroha_core/src/sumeragi/v2_apply.rs",
    "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
    "crates/iroha_core/src/sumeragi/v2_core/scheduler.rs",
    "crates/iroha_core/src/sumeragi/v2_core/tests.rs",
    "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
    "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
    "crates/iroha_core/src/sumeragi/v2_body_store.rs",
    "crates/iroha_core/src/sumeragi/v2_effects.rs",
    "crates/iroha_core/src/sumeragi/mod.rs",
    "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
    "crates/iroha_core/src/sumeragi/v2_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2_runner.rs",
    "crates/iroha_core/src/sumeragi/v2_runtime.rs",
    "crates/iroha_core/src/sumeragi/v2_transport.rs",
    "crates/iroha_core/src/sumeragi/v2_worker.rs",
    "crates/iroha_core/src/merge_sidecar.rs",
    "crates/iroha_p2p/src/network.rs",
    "crates/iroha_p2p/src/peer.rs",
    "crates/irohad/src/main.rs",
    "docs/formal/sumeragi_v2/SumeragiV2EffectiveLockAcquisition.tla",
    "docs/formal/sumeragi_v2/SumeragiV2EffectiveLockAcquisitionProofs.tla",
    "docs/formal/sumeragi_v2/proof_coverage.json",
    "scripts/formal/run_sumeragi_v2_harness.sh",
    "scripts/formal/sumeragi_v2_verus_evidence.py",
    "scripts/formal/sumeragi_v2_harness.lock",
    "scripts/run_sumeragi_v2_formal_release.sh",
    "scripts/verify_sumeragi_v2.sh",
)

EXPECTED_VERIFY_COMMAND_SOURCE = "\\\n".join(
    (
        "bash scripts/formal/run_sumeragi_v2_harness.sh --verus ",
        '  2>&1 | tee -a "$verus_log_tmp"',
    )
)
EXPECTED_HARNESS_VERUS_BRANCH = """\
  --verus)
    if (($# != 1)); then
      echo "--verus accepts no additional arguments" >&2
      exit 2
    fi
    run_cargo verus verify --locked --offline -p iroha_sumeragi_core --features verus \\
      --fwd-verus-args-to roots -- \\
      --rlimit 60 \\
      --expand-errors \\
      --no-cheating
    ;;"""

EXPECTED_TOOL_SHA256 = {
    "Darwin-arm64": {
        "platform": "macos_aarch64",
        "verus": "f11f8a863103a3c8fcaf27e6189edfdba31081516591365b5e29b0a66f570451",
        "cargo_verus": "f918c6229c8d714640c9c9ec3d60b9c1d2e0aafc09bba8ff037332b04f85d078",
    },
    "Linux-x86_64": {
        "platform": "linux_x86_64",
        "verus": "c5911ee43c7a92c49a48d2c8646c604d252a38c71c87bda88ad4d33eb9e7e0fc",
        "cargo_verus": "42a79c9afd700f8312a9ac7ab212070723e71beeb07f5ab855453010455bdc6d",
    },
}

NONCE_RE = re.compile(r"[0-9a-f]{64}")
SHA256_RE = re.compile(r"[0-9a-f]{64}")
RESULT_RE = re.compile(r"^verification results:: ([0-9]+) verified, ([0-9]+) errors$")


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def verification_contract_sha256() -> str:
    """Hash every code-owned input which defines a valid verifier transcript."""

    payload = {
        "schema_version": SCHEMA_VERSION,
        "verus_version": EXPECTED_VERUS_VERSION,
        "dependency_verified": EXPECTED_DEPENDENCY_VERIFIED,
        "root_verified": EXPECTED_ROOT_VERIFIED,
        "log_path": EXPECTED_LOG_PATH,
        "invocation": list(EXPECTED_INVOCATION),
        "required_source_paths": list(REQUIRED_SOURCE_PATHS),
        "tool_sha256": EXPECTED_TOOL_SHA256,
    }
    encoded = json.dumps(
        payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _host_key() -> str:
    return f"{platform.system()}-{platform.machine()}"


def begin_marker(nonce: str, source_manifest_sha256: str) -> str:
    """Return the exact pre-verification transcript marker."""

    return (
        "Sumeragi v2 Verus evidence begin: "
        f"nonce={nonce} source_manifest_sha256={source_manifest_sha256}"
    )


def success_marker(nonce: str, source_manifest_sha256: str) -> str:
    """Return the exact post-verification transcript marker."""

    return (
        "Sumeragi v2 Verus evidence passed: "
        f"nonce={nonce} source_manifest_sha256={source_manifest_sha256}"
    )


def parse_verus_log(
    source: str, *, nonce: str, source_manifest_sha256: str
) -> tuple[int, int]:
    """Parse one exact, successful, source-bound Verus transcript."""

    if NONCE_RE.fullmatch(nonce) is None:
        raise ValueError("Verus evidence nonce must be 64 lowercase hexadecimal digits")
    if SHA256_RE.fullmatch(source_manifest_sha256) is None:
        raise ValueError("Verus source manifest must be a lowercase SHA-256 digest")

    lines = source.splitlines()
    expected_begin = begin_marker(nonce, source_manifest_sha256)
    expected_success = success_marker(nonce, source_manifest_sha256)
    if lines.count(expected_begin) != 1:
        raise ValueError("Verus log must contain one exact source-bound begin marker")
    if lines.count(expected_success) != 1:
        raise ValueError("Verus log must contain one exact source-bound success marker")
    if not lines or lines[0] != expected_begin or lines[-1] != expected_success:
        raise ValueError("Verus evidence markers must delimit the complete transcript")

    results = [
        (int(match.group(1)), int(match.group(2)))
        for line in lines
        if (match := RESULT_RE.fullmatch(line)) is not None
    ]
    expected = [
        (EXPECTED_DEPENDENCY_VERIFIED, 0),
        (EXPECTED_ROOT_VERIFIED, 0),
    ]
    if results != expected:
        raise ValueError(
            "Verus log must contain the independently pinned dependency and root "
            f"results {expected}, found {results}"
        )
    return results[0][0], results[1][0]


def _source_entries(root: Path) -> list[dict[str, str]]:
    entries: list[dict[str, str]] = []
    for relative in REQUIRED_SOURCE_PATHS:
        path = root / relative
        if not path.is_file() or path.is_symlink():
            raise ValueError(f"required Verus source is not a regular file: {relative}")
        entries.append({"path": relative, "sha256": _sha256_file(path)})
    return entries


def _verify_invocation_contract(root: Path) -> None:
    """Pin the shell command which produced the transcript."""

    path = root / "scripts/verify_sumeragi_v2.sh"
    if not path.is_file() or path.is_symlink():
        raise ValueError("Sumeragi v2 Verus runner must be a regular file")
    source = path.read_text(encoding="utf-8")
    if source.count(EXPECTED_VERIFY_COMMAND_SOURCE) != 1:
        raise ValueError("Sumeragi v2 Verus runner command has drifted")
    if "cargo verus verify" in source:
        raise ValueError(
            "Sumeragi v2 Verus runner must delegate through the fixed harness mode"
        )
    harness = root / "scripts/formal/run_sumeragi_v2_harness.sh"
    if not harness.is_file() or harness.is_symlink():
        raise ValueError("Sumeragi v2 formal harness must be a regular file")
    harness_source = harness.read_text(encoding="utf-8")
    if harness_source.count(EXPECTED_HARNESS_VERUS_BRANCH) != 1:
        raise ValueError("Sumeragi v2 Verus harness command has drifted")
    if '"${@:2}"' in harness_source or harness_source.count('"$@"') != 1:
        raise ValueError("Sumeragi v2 formal harness permits arbitrary command dispatch")


def _verus_version(binary: Path) -> tuple[str, str]:
    result = subprocess.run(
        [str(binary), "--version"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    version = ""
    verifier_platform = ""
    for raw_line in result.stdout.splitlines():
        line = raw_line.strip()
        if line.startswith("Version:"):
            version = line.partition(":")[2].strip()
        elif line.startswith("Platform:"):
            verifier_platform = line.partition(":")[2].strip()
    if version != EXPECTED_VERUS_VERSION:
        raise ValueError(
            f"expected Verus {EXPECTED_VERUS_VERSION}, found {version or 'unknown'}"
        )
    return version, verifier_platform


def _tool_evidence(verus: Path, cargo_verus: Path) -> dict[str, str]:
    host = _host_key()
    expected = EXPECTED_TOOL_SHA256.get(host)
    if expected is None:
        raise ValueError(f"unsupported Sumeragi v2 Verus evidence host: {host}")
    for name, path in (("verus", verus), ("cargo-verus", cargo_verus)):
        if not path.is_file() or path.is_symlink() or not os.access(path, os.X_OK):
            raise ValueError(f"{name} is not a regular executable: {path}")
    version, verifier_platform = _verus_version(verus)
    actual_verus = _sha256_file(verus)
    actual_cargo_verus = _sha256_file(cargo_verus)
    if actual_verus != expected["verus"]:
        raise ValueError("Verus binary digest does not match the pinned release tool")
    if actual_cargo_verus != expected["cargo_verus"]:
        raise ValueError("cargo-verus binary digest does not match the pinned release tool")
    if verifier_platform != expected["platform"]:
        raise ValueError(
            f"expected Verus platform {expected['platform']}, found {verifier_platform}"
        )
    return {
        "version": version,
        "platform": verifier_platform,
        "verus_sha256": actual_verus,
        "cargo_verus_sha256": actual_cargo_verus,
    }


def build_evidence(
    *,
    root: Path,
    log_path: Path,
    nonce: str,
    verus: Path,
    cargo_verus: Path,
) -> dict[str, Any]:
    """Build canonical evidence from the current checkout and transcript."""

    root = root.resolve(strict=True)
    if not log_path.is_file() or log_path.is_symlink():
        raise ValueError("Verus evidence log must be a regular file")
    if not verus.is_file() or verus.is_symlink():
        raise ValueError("Verus must be a regular executable")
    if not cargo_verus.is_file() or cargo_verus.is_symlink():
        raise ValueError("cargo-verus must be a regular executable")
    log_path = log_path.resolve(strict=True)
    expected_log = (root / EXPECTED_LOG_PATH).resolve(strict=True)
    if log_path != expected_log:
        raise ValueError(f"Verus log must be {EXPECTED_LOG_PATH}")
    source_manifest_sha256 = workspace_source_manifest(root)
    _verify_invocation_contract(root)
    dependency_verified, root_verified = parse_verus_log(
        log_path.read_text(encoding="utf-8"),
        nonce=nonce,
        source_manifest_sha256=source_manifest_sha256,
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "verification_contract_sha256": verification_contract_sha256(),
        "source_manifest_sha256": source_manifest_sha256,
        "sources": _source_entries(root),
        "tool": _tool_evidence(verus.resolve(strict=True), cargo_verus.resolve(strict=True)),
        "invocation": list(EXPECTED_INVOCATION),
        "log": EXPECTED_LOG_PATH,
        "log_sha256": _sha256_file(log_path),
        "nonce": nonce,
        "results": {
            "dependency_verified": dependency_verified,
            "root_verified": root_verified,
            "errors": 0,
        },
        "backend_verification": True,
    }


def validate_evidence(
    evidence: Any,
    *,
    root: Path = ROOT_DIR,
    source_manifest_sha256: str | None = None,
    log_path: Path | None = None,
) -> tuple[str, ...]:
    """Return every structural, source, tool, and transcript error."""

    errors: list[str] = []
    if not isinstance(evidence, dict):
        return ("Verus evidence must be an object",)
    expected_keys = {
        "schema_version",
        "verification_contract_sha256",
        "source_manifest_sha256",
        "sources",
        "tool",
        "invocation",
        "log",
        "log_sha256",
        "nonce",
        "results",
        "backend_verification",
    }
    if set(evidence) != expected_keys:
        errors.append(
            "Verus evidence fields must equal "
            f"{sorted(expected_keys)}, found {sorted(map(str, evidence))}"
        )

    if evidence.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"Verus evidence schema_version must equal {SCHEMA_VERSION}")
    if evidence.get("verification_contract_sha256") != verification_contract_sha256():
        errors.append("Verus evidence verification contract digest has drifted")
    if evidence.get("backend_verification") is not True:
        errors.append("Verus evidence requires backend_verification=true")
    if evidence.get("invocation") != list(EXPECTED_INVOCATION):
        errors.append("Verus evidence invocation does not match the pinned --no-cheating command")
    if evidence.get("log") != EXPECTED_LOG_PATH:
        errors.append(f"Verus evidence log must equal {EXPECTED_LOG_PATH}")

    root = root.resolve()
    expected_manifest = source_manifest_sha256
    if expected_manifest is None:
        try:
            expected_manifest = workspace_source_manifest(root)
        except (OSError, RuntimeError, subprocess.SubprocessError) as error:
            errors.append(f"failed to compute Verus source manifest: {error}")
    if evidence.get("source_manifest_sha256") != expected_manifest:
        errors.append("Verus evidence is not bound to the current workspace source manifest")

    try:
        expected_sources = _source_entries(root)
    except (OSError, ValueError) as error:
        errors.append(str(error))
        expected_sources = None
    if evidence.get("sources") != expected_sources:
        errors.append("Verus evidence source inventory or digest has drifted")
    try:
        _verify_invocation_contract(root)
    except (OSError, UnicodeDecodeError, ValueError) as error:
        errors.append(str(error))

    host = _host_key()
    expected_tool = EXPECTED_TOOL_SHA256.get(host)
    tool = evidence.get("tool")
    if expected_tool is None:
        errors.append(f"unsupported Sumeragi v2 Verus evidence host: {host}")
    elif not isinstance(tool, dict):
        errors.append("Verus evidence tool must be an object")
    else:
        expected_tool_keys = {
            "version",
            "platform",
            "verus_sha256",
            "cargo_verus_sha256",
        }
        if set(tool) != expected_tool_keys:
            errors.append(
                "Verus evidence tool fields must equal "
                f"{sorted(expected_tool_keys)}, found {sorted(map(str, tool))}"
            )
        if tool.get("version") != EXPECTED_VERUS_VERSION:
            errors.append("Verus evidence version is not pinned")
        if tool.get("platform") != expected_tool["platform"]:
            errors.append("Verus evidence platform is not pinned")
        if tool.get("verus_sha256") != expected_tool["verus"]:
            errors.append("Verus evidence verifier digest is not pinned")
        if tool.get("cargo_verus_sha256") != expected_tool["cargo_verus"]:
            errors.append("Verus evidence cargo-verus digest is not pinned")

    results = evidence.get("results")
    expected_results = {
        "dependency_verified": EXPECTED_DEPENDENCY_VERIFIED,
        "root_verified": EXPECTED_ROOT_VERIFIED,
        "errors": 0,
    }
    if results != expected_results:
        errors.append(
            "Verus evidence results must equal independently pinned counts "
            f"{expected_results}"
        )

    nonce = evidence.get("nonce")
    if not isinstance(nonce, str) or NONCE_RE.fullmatch(nonce) is None:
        errors.append("Verus evidence nonce must be 64 lowercase hexadecimal digits")
    log_path = root / EXPECTED_LOG_PATH if log_path is None else log_path
    if not log_path.is_file() or log_path.is_symlink():
        errors.append(f"Verus evidence log is not a regular file: {log_path}")
    else:
        actual_log_sha256 = _sha256_file(log_path)
        if evidence.get("log_sha256") != actual_log_sha256:
            errors.append("Verus evidence log digest mismatch")
        if isinstance(nonce, str) and expected_manifest is not None:
            try:
                parse_verus_log(
                    log_path.read_text(encoding="utf-8"),
                    nonce=nonce,
                    source_manifest_sha256=expected_manifest,
                )
            except (UnicodeDecodeError, ValueError) as error:
                errors.append(str(error))
    return tuple(errors)


def _write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.partial")
    temporary.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    os.replace(temporary, path)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    marker = subparsers.add_parser("marker", help="print a fresh begin marker")
    marker.add_argument("--root", type=Path, default=ROOT_DIR)
    marker.add_argument("--nonce")

    write = subparsers.add_parser("write", help="write canonical Verus evidence")
    write.add_argument("--root", type=Path, default=ROOT_DIR)
    write.add_argument("--log", type=Path, required=True)
    write.add_argument("--output", type=Path, required=True)
    write.add_argument("--nonce", required=True)
    write.add_argument("--verus", type=Path, required=True)
    write.add_argument("--cargo-verus", type=Path, required=True)

    validate = subparsers.add_parser("validate", help="validate Verus evidence")
    validate.add_argument("--root", type=Path, default=ROOT_DIR)
    validate.add_argument("--evidence", type=Path, required=True)
    validate.add_argument(
        "--log",
        type=Path,
        help="validate an archived log while retaining its canonical evidence name",
    )
    return parser


def main() -> int:
    args = _parser().parse_args()
    if args.command == "marker":
        nonce = args.nonce or secrets.token_hex(32)
        manifest = workspace_source_manifest(args.root.resolve(strict=True))
        print(nonce)
        print(begin_marker(nonce, manifest))
        return 0
    if args.command == "write":
        evidence = build_evidence(
            root=args.root,
            log_path=args.log,
            nonce=args.nonce,
            verus=args.verus,
            cargo_verus=args.cargo_verus,
        )
        _write_json_atomic(args.output, evidence)
        return 0

    try:
        evidence = json.loads(args.evidence.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        print(f"failed to read Verus evidence: {error}", file=sys.stderr)
        return 1
    errors = validate_evidence(evidence, root=args.root, log_path=args.log)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("Sumeragi v2 Verus evidence is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
