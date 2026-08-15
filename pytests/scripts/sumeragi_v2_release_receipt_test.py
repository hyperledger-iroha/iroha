"""Contract tests for the aggregate Sumeragi v2 release receipt."""
from __future__ import annotations

import ast
import base64
import hashlib
import io
import importlib.util
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import shutil
import stat
import subprocess
import sys
import tarfile
from types import ModuleType
import pytest
from pytests.scripts.sumeragi_v2_release_receipt_components import (
    fixture_cargo_cache_input, fixture_corridor_legs, install_cache_helper, proof_ledger_checker_components,
    release_receipt_writer_components, terminal_output_path,
)
from pytests.scripts.sumeragi_v2_release_receipt_test_support import (
    CARGO_VERSION_OUTPUT,
    CHAOS_FIELDS,
    CHAOS_MARKER,
    FINAL_MARKER,
    PREBUILT_HOST_TRIPLE,
    RUSTC_VERSION_OUTPUT,
    SCALING_CONFIGURATION_DATA,
    SCALING_IROHAD_SHA256,
    SCALING_IROHA_CLI_SHA256,
    SCALING_TRIAL_HARNESS_DATA,
    SCENARIOS,
    SUMMARY_FIELDS,
    artifact_metadata,
    canonical_json,
    command_record,
    protected_metadata,
    sha256,
    write_tsv,
)
ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py"
BOOTSTRAP_COMPONENT_FILES = (
    ROOT_DIR / "scripts" / "bootstrap_sumeragi_v2_release_receipt_replay.py",
)
RECEIPT_VALIDATOR_COMPONENT_FILES = tuple(
    ROOT_DIR / relative
    for relative in release_receipt_writer_components(ROOT_DIR)
)
APPROVAL_CONTRACT = (
    ROOT_DIR / "scripts" / "sumeragi_v2_release_approval_contract.py"
)
APPROVAL_EVIDENCE_ROOT_ID = "fixture-release-evidence-root"
APPROVAL_DURATIONS = (900, 901, 902, 903)
APPROVAL_CLASS_IDS = (
    "offline-toolchain-sdk",
    "formal-proof-tools",
    "network-scale-soak",
    "final-bootstrap-publication",
)
IDENTITY_ARCHIVE_IDS = {
    "cargo_lock": "release-identity.cargo-lock.v1",
    "git": "release-identity.git.v1",
    "raw_commit": "release-identity.raw-commit.v1",
    "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
    "ssh_keygen": "release-identity.ssh-keygen.v1",
    "ssh_revocation": "release-identity.ssh-revocation.v1",
    "verify_transcript": "release-identity.verify-transcript.v1",
}

FIXTURE_TOOL_PROBE_OPERATION_IDS = {
    "awk": "release-tool.awk-program.v1",
    "basename": "release-tool.basename-path.v1",
    "cargo": "release-tool.cargo-version.v1",
    "cargo-verus": "release-tool.cargo-verus-help.v1",
    "cat": "release-tool.cat-file.v1",
    "chmod": "release-tool.chmod-mode.v1",
    "cmp": "release-tool.cmp-different-quiet.v1",
    "cp": "release-tool.cp-file.v1",
    "cut": "release-tool.cut-byte.v1",
    "diff": "release-tool.diff-different-brief.v1",
    "dirname": "release-tool.dirname-path.v1",
    "env": "release-tool.env-closed.v1",
    "find": "release-tool.find-file.v1",
    "git-index-pack": "release-tool.git-index-pack-empty.v1",
    "git-upload-pack": "release-tool.git-upload-pack-missing.v1",
    "grep": "release-tool.grep-exact.v1",
    "java": "release-tool.java-version.v1",
    "ln": "release-tool.ln-hardlink.v1",
    "ls": "release-tool.ls-entry.v1",
    "mkdir": "release-tool.mkdir-directory.v1",
    "mkfifo": "release-tool.mkfifo-fifo.v1",
    "mktemp": "release-tool.mktemp-file.v1",
    "mv": "release-tool.mv-file.v1",
    "node": "release-tool.node-exec-path.v1",
    "openssl": "release-tool.openssl-sha256.v1",
    "rm": "release-tool.rm-file.v1",
    "rmdir": "release-tool.rmdir-directory.v1",
    "rustc": "release-tool.rustc-version.v1",
    "sed": "release-tool.sed-first-line.v1",
    "sh": "release-tool.sh-builtin-output.v1",
    ("shasum" if sys.platform == "darwin" else "sha256sum"): (
        "release-tool.shasum-empty.v1"
        if sys.platform == "darwin"
        else "release-tool.sha256sum-empty.v1"
    ),
    "sleep": "release-tool.sleep-duration.v1",
    "swift": "release-tool.swift-version.v1",
    "tail": "release-tool.tail-last-line.v1",
    "tee": "release-tool.tee-file.v1",
    "tlapm": "release-tool.tlapm-version.v1",
    "tr": "release-tool.tr-byte.v1",
    "uname": "release-tool.uname-system.v1",
    "verus": "release-tool.verus-version.v1",
    "wc": "release-tool.wc-empty.v1",
    "xargs": "release-tool.xargs-protected-shell.v1",
}
assert len(FIXTURE_TOOL_PROBE_OPERATION_IDS) == 41


def _load_approval_component(path: Path) -> object:
    name = "sumeragi_release_receipt_approval_fixture"
    module = ModuleType(name)
    module.__file__ = str(path)
    module.__package__ = ""
    sys.modules[name] = module
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    return module


def _bootstrap_approval_fixture(
    *,
    module: object,
    identity: dict[str, object],
    tool_manifest_sha256: str,
    trust_dir: Path,
    evidence_dir: Path,
) -> tuple[dict[str, Path], dict[str, object]]:
    expectations = module.build_release_approval_expectations(
        candidate_oid=identity["head_commit"],
        candidate_tree=identity["head_tree"],
        protected_tool_manifest_sha256=tool_manifest_sha256,
        evidence_root_id=APPROVAL_EVIDENCE_ROOT_ID,
        offline_toolchain_sdk_duration_seconds=APPROVAL_DURATIONS[0],
        formal_proof_tools_duration_seconds=APPROVAL_DURATIONS[1],
        network_scale_soak_duration_seconds=APPROVAL_DURATIONS[2],
        final_bootstrap_publication_duration_seconds=APPROVAL_DURATIONS[3],
    )
    paths: dict[object, Path] = {}
    archives: dict[str, Path] = {}
    for ordinal, approval_class in enumerate(module.APPROVAL_CLASS_ORDER):
        expectation = expectations[approval_class]
        class_id = approval_class.value
        value = {
            "approval_id": f"fixture-approval-{ordinal}-{class_id}",
            "approved_at": f"2026-08-{ordinal + 1:02d}T01:02:03Z",
            "candidate_oid": expectation.candidate_oid,
            "candidate_tree": expectation.candidate_tree,
            "class_id": class_id,
            "evidence_root_id": expectation.evidence_root_id,
            "expected_duration_seconds": expectation.expected_duration_seconds,
            "format": module.APPROVAL_FORMAT,
            "operations": [item.value() for item in expectation.operations],
            "profile": expectation.profile,
            "protected_tool_manifest_sha256": (
                expectation.protected_tool_manifest_sha256
            ),
            "schema_version": module.APPROVAL_SCHEMA_VERSION,
        }
        data = canonical_json(value)
        label = "approval_" + class_id.replace("-", "_")
        source = trust_dir / label
        if source.exists():
            source.chmod(0o600)
        source.write_bytes(data)
        source.chmod(0o400)
        archive = evidence_dir / f"{class_id}.approval.v1.json"
        if archive.exists():
            archive.chmod(0o600)
        archive.write_bytes(data)
        archive.chmod(0o400)
        archives[label] = archive
        paths[approval_class] = archive
    approvals = module.load_protected_release_approval_set(
        paths, expectations=expectations, expected_owner_uid=os.getuid()
    )
    class_records: dict[str, object] = {}
    for approval in approvals:
        class_id = approval.class_id.value
        sanitized = approval.sanitized_archive()
        name = f"{class_id}.approval-attestation.v1.json"
        path = evidence_dir / name
        if path.exists():
            path.chmod(0o600)
        path.write_bytes(sanitized.canonical_bytes)
        path.chmod(0o400)
        class_records[class_id] = {
            "archive_id": module.APPROVAL_ARCHIVE_IDS[approval.class_id],
            "archive_name": name,
            "mode": "0400",
            "sha256": sanitized.sha256,
            "size_bytes": path.stat().st_size,
        }
    sanitized_set = module.sanitized_release_approval_set_archive(approvals)
    set_name = "release-approval-set-attestation.v1.json"
    set_path = evidence_dir / set_name
    if set_path.exists():
        set_path.chmod(0o600)
    set_path.write_bytes(sanitized_set.canonical_bytes)
    set_path.chmod(0o400)
    marker = {
        "format": module.APPROVAL_SET_ARCHIVE_FORMAT,
        "schema_version": 1,
        "candidate_oid": identity["head_commit"],
        "candidate_tree": identity["head_tree"],
        "protected_tool_manifest_sha256": tool_manifest_sha256,
        "evidence_root_id": APPROVAL_EVIDENCE_ROOT_ID,
        "expected_duration_seconds": dict(zip(APPROVAL_CLASS_IDS, APPROVAL_DURATIONS)),
        "operation_plan_sha256": {
            approval_class.value: digest
            for approval_class, digest in module.APPROVAL_OPERATION_PLAN_SHA256.items()
        },
        "class_attestations": class_records,
        "set_attestation": {
            "archive_id": "release-approval.set-attestation.v1",
            "archive_name": set_name,
            "mode": "0400",
            "sha256": sanitized_set.sha256,
            "size_bytes": set_path.stat().st_size,
        },
    }
    return archives, marker


def fixture_tool_probe_result(
    manifest: dict[str, object], *, archive_id_prefix: str
) -> dict[str, object]:
    """Build the synthetic path-free result emitted by the fixture helper."""

    tools = manifest["tools"]
    assert isinstance(tools, dict)
    assert set(tools) == set(FIXTURE_TOOL_PROBE_OPERATION_IDS)
    empty_sha256 = hashlib.sha256(b"").hexdigest()
    records: dict[str, object] = {}
    for name in sorted(tools):
        source = tools[name]
        assert isinstance(source, dict)
        path = Path(source["path"])
        operation_id = FIXTURE_TOOL_PROBE_OPERATION_IDS[name]
        invocation_sha256 = hashlib.sha256(
            canonical_json(
                {
                    "operation_id": operation_id,
                    "schema_version": 1,
                    "tool": name,
                }
            )
        ).hexdigest()
        postcondition_sha256 = hashlib.sha256(
            canonical_json(
                {
                    "exit_status": (
                        128
                        if name in {"git-index-pack", "git-upload-pack"}
                        else 1
                        if name in {"cmp", "diff"}
                        else 0
                    ),
                    "operation_id": operation_id,
                    "schema_version": 1,
                }
            )
        ).hexdigest()
        records[name] = {
            "archive_id": f"{archive_id_prefix}.{name}.v1",
            "exit_status": (
                128
                if name in {"git-index-pack", "git-upload-pack"}
                else 1
                if name in {"cmp", "diff"}
                else 0
            ),
            "invocation_sha256": invocation_sha256,
            "mode": "0500",
            "operation_id": operation_id,
            "postcondition_sha256": postcondition_sha256,
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "stderr_sha256": empty_sha256,
            "stderr_size_bytes": 0,
            "stdout_sha256": empty_sha256,
            "stdout_size_bytes": 0,
        }
    contract = {
        "operation_ids": FIXTURE_TOOL_PROBE_OPERATION_IDS,
        "schema_version": 1,
    }
    return {
        "format": "iroha-sumeragi-v2-release-tool-functional-probes",
        "host_family": "darwin" if sys.platform == "darwin" else "linux",
        "probe_contract_sha256": hashlib.sha256(
            canonical_json(contract)
        ).hexdigest(),
        "schema_version": 1,
        "tool_count": 41,
        "tools": records,
    }


def fixture_tool_probe_helper_source() -> bytes:
    """Return a Python-only probe fixture which never launches a tool engine."""

    operation_ids = json.dumps(
        FIXTURE_TOOL_PROBE_OPERATION_IDS,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    )
    return f'''#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

OPERATION_IDS = {operation_ids}

def canonical(value):
    return (json.dumps(value, allow_nan=False, ensure_ascii=True,
                       separators=(",", ":"), sort_keys=True) + "\\n").encode("ascii")

parser = argparse.ArgumentParser()
parser.add_argument("--tool-manifest", type=Path, required=True)
parser.add_argument("--expected-tool-manifest-sha256", required=True)
parser.add_argument("--probe-root", type=Path, required=True)
args = parser.parse_args()
data = args.tool_manifest.read_bytes()
if hashlib.sha256(data).hexdigest() != args.expected_tool_manifest_sha256:
    raise SystemExit(21)
manifest = json.loads(data.decode("ascii"))
tools = manifest.get("tools")
if set(manifest) != {{"schema_version", "tools"}} or manifest["schema_version"] != 1:
    raise SystemExit(22)
if not isinstance(tools, dict) or set(tools) != set(OPERATION_IDS) or len(tools) != 41:
    raise SystemExit(23)
empty = hashlib.sha256(b"").hexdigest()
records = {{}}
for name in sorted(tools):
    source = tools[name]
    if set(source) != {{"archive_id", "path", "sha256"}}:
        raise SystemExit(24)
    path = Path(source["path"])
    metadata = path.stat()
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if (not path.is_absolute() or not stat.S_ISREG(metadata.st_mode)
            or stat.S_IMODE(metadata.st_mode) != 0o500
            or digest != source["sha256"]):
        raise SystemExit(25)
    operation_id = OPERATION_IDS[name]
    status = 128 if name in {{"git-index-pack", "git-upload-pack"}} else 1 if name in {{"cmp", "diff"}} else 0
    records[name] = {{
        "archive_id": source["archive_id"],
        "exit_status": status,
        "invocation_sha256": hashlib.sha256(canonical({{
            "operation_id": operation_id, "schema_version": 1, "tool": name,
        }})).hexdigest(),
        "mode": "0500",
        "operation_id": operation_id,
        "postcondition_sha256": hashlib.sha256(canonical({{
            "exit_status": status, "operation_id": operation_id, "schema_version": 1,
        }})).hexdigest(),
        "sha256": digest,
        "size_bytes": metadata.st_size,
        "stderr_sha256": empty,
        "stderr_size_bytes": 0,
        "stdout_sha256": empty,
        "stdout_size_bytes": 0,
    }}
value = {{
    "format": "iroha-sumeragi-v2-release-tool-functional-probes",
    "host_family": "darwin" if sys.platform == "darwin" else "linux",
    "probe_contract_sha256": hashlib.sha256(canonical({{
        "operation_ids": OPERATION_IDS, "schema_version": 1,
    }})).hexdigest(),
    "schema_version": 1,
    "tool_count": 41,
    "tools": records,
}}
sys.stdout.buffer.write(canonical(value))
'''.encode("ascii")


def sanitized_identity_artifact(
    path: Path, mode: int, label: str
) -> dict[str, str | int]:
    return {
        "archive_id": IDENTITY_ARCHIVE_IDS[label],
        "mode": f"{mode:04o}",
        "sha256": sha256(path),
        "size_bytes": path.stat().st_size,
    }


def sanitized_operation(
    operation_id: str, status: int, stdout: bytes, stderr: bytes
) -> dict[str, str | int]:
    return {
        "operation_id": operation_id,
        "exit_status": status,
        "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
        "stdout_size_bytes": len(stdout),
        "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
        "stderr_size_bytes": len(stderr),
    }
RELEASE_RECEIPT_TEST_COMPONENT_FILES = (
    "sumeragi_v2_release_receipt_identity_replay_cases.py",
    "sumeragi_v2_release_receipt_bootstrap_archive_cases.py",
    "sumeragi_v2_release_receipt_sdk_source_closure_cases.py",
    "sumeragi_v2_release_receipt_supervision_cases.py",
    "sumeragi_v2_release_receipt_terminal_publication_cases.py",
    "sumeragi_v2_release_approval_contract_cases.py",
)
def _execute_test_component(filename: str) -> None:
    """Execute one reviewed case component in this canonical test namespace."""
    path = Path(__file__).with_name(filename)
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"release-receipt test component is unavailable: {path}")
    source = path.read_text(encoding="utf-8")
    exec(compile(source, str(path), "exec"), globals())


def _execute_test_component_function(filename: str, function_name: str) -> None:
    """Execute one exact component-owned function at its canonical order point."""

    path = Path(__file__).with_name(filename)
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"release-receipt test component is unavailable: {path}")
    source = path.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(path))
    functions = [
        node
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        and node.name == function_name
    ]
    if len(functions) != 1:
        raise RuntimeError(
            f"release-receipt component function must be unique: {function_name}"
        )
    selected = ast.Module(body=functions, type_ignores=[])
    exec(compile(selected, str(path), "exec"), globals())
def fixture_writer(tmp_path: Path) -> Path:
    project = tmp_path / "writer-project"
    scripts = project / "scripts"
    formal = scripts / "formal"
    nexus = scripts / "nexus"
    formal.mkdir(parents=True)
    nexus.mkdir()
    writer = scripts / SCRIPT.name
    shutil.copy2(SCRIPT, writer)
    for relative in release_receipt_writer_components(ROOT_DIR):
        destination = project / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    shutil.copy2(
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh",
        scripts / "run_sumeragi_v2_release_gates.sh",
    )
    shutil.copy2(
        ROOT_DIR / "scripts" / "sumeragi_v2_localnet_manifest.py",
        scripts / "sumeragi_v2_localnet_manifest.py",
    )
    shutil.copy2(
        ROOT_DIR / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py",
        nexus / "validate_multilane_scaling_evidence.py",
    )
    for relative in (
        Path("scripts/deploy_localnet.sh"),
        Path("scripts/tx_load.py"),
        Path("scripts/nexus_lane_load_test.py"),
    ):
        destination = project / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    fixture_cargo = project / ".cargo"
    fixture_cargo.mkdir()
    shutil.copy2(ROOT_DIR / ".cargo" / "config.toml", fixture_cargo / "config.toml")
    (formal / "check_sumeragi_v2_proof_ledger.py").write_text(
        """_CHECKER_COMPONENT_FILES = (); import json
import pathlib
import sys

args = sys.argv[1:]
ledger_path = pathlib.Path(args[args.index("--ledger") + 1])
ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
cross_ids = [
    item.get("id")
    for item in ledger.get("obligations", [])
    if item.get("status") == "cross_tool_proved"
]
if "--print-cross-tool-obligations" in args:
    print("\\n".join(cross_ids))
    raise SystemExit(0)
if "--release" in args:
    if "--verus-evidence" not in args:
        raise SystemExit(81)
    if "--verus-log" not in args:
        raise SystemExit(84)
    has_cross = "--cross-tool-evidence" in args
    if bool(cross_ids) != has_cross:
        raise SystemExit(82)
    if has_cross:
        cross_path = pathlib.Path(args[args.index("--cross-tool-evidence") + 1])
        if json.loads(cross_path.read_text(encoding="utf-8")) != {
            "backend_verification": True,
            "canonical": True,
        }:
            raise SystemExit(83)
    if "--production-trace-extraction-evidence" in args:
        trace_path = pathlib.Path(
            args[args.index("--production-trace-extraction-evidence") + 1]
        )
        if json.loads(trace_path.read_text(encoding="utf-8")) != {
            "backend_verification": True,
            "canonical": True,
            "multilane_source_manifest_sha256": "c" * 64,
            "theorem": "sumeragi-v2-production-trace-extraction",
            "workspace_source_manifest_sha256": "b" * 64,
        }:
            raise SystemExit(85)
raise SystemExit(0)
""",
        encoding="utf-8",
    )
    (formal / "sumeragi_v2_verus_evidence.py").write_text(
        "raise SystemExit(0)\n", encoding="utf-8"
    )
    (scripts / "check_taira_v2_soak_evidence.py").write_text(
        "raise SystemExit(0)\n", encoding="utf-8"
    )
    return writer

def make_bootstrap_evidence(
    tmp_path: Path,
    *,
    identity: dict[str, object],
    identity_bytes: bytes,
    raw_commit: bytes,
    lock_bytes: bytes,
    signer_fingerprint: str,
    signer_principal: str,
    signature_git: Path,
    signature_ssh_keygen: Path,
    signature_allowed_signers: Path,
    signature_revocation: Path,
    show_output: bytes,
    scaling_evidence_manifest: Path,
    scaling_trial_harness_sha256: str,
    scaling_configuration_sha256: str,
    scaling_irohad_sha256: str,
    scaling_iroha_cli_sha256: str,
    sdk_source_manifest_data: bytes,
) -> dict[str, Path | str]:
    candidate_root = tmp_path / "bootstrap-candidate"
    (candidate_root / "scripts").mkdir(parents=True)
    (candidate_root / "Cargo.lock").write_bytes(lock_bytes)
    runner = candidate_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    runner_source = ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    shutil.copy2(runner_source, runner)

    evidence_dir = tmp_path / "bootstrap-evidence"
    evidence_dir.mkdir(mode=0o700)
    evidence_dir.chmod(0o700)
    for child in ("home", "tmp", "runner-bin", "runner-tools"):
        (evidence_dir / child).mkdir(mode=0o700)
        (evidence_dir / child).chmod(0o700)
    for log_name in ("runner-stdout.log", "runner-stderr.log"):
        log = evidence_dir / log_name
        log.write_bytes(b"")
        log.chmod(0o600)
    trust_dir = tmp_path / "bootstrap-trust"
    trust_dir.mkdir(mode=0o700)
    frozen_bootstrap = ROOT_DIR / "scripts" / "bootstrap_sumeragi_v2_release.py"
    assert sha256(frozen_bootstrap) == (
        "3e87cffe611d61fb2e9a7a6d921cc263794238c57c3d22121025b74423b6468d"
    )
    python_probe_code = "import sys;sys.stdout.write(sys.executable+'\\n')"
    python_launcher = (
        "#!/bin/sh\n"
        "if test \"$#\" = 4 && test \"$1\" = -I && test \"$2\" = -S "
        "&& test \"$3\" = -c && test \"$4\" = "
        + shlex.quote(python_probe_code)
        + "; then printf '%s\\n' \"$0\"; exit 0; fi\n"
        "exec "
        + shlex.quote(sys.executable)
        + " \"$@\"\n"
    ).encode("utf-8")
    synthetic_sources: dict[str, Path] = {}
    for label, data, mode in (
        ("python", python_launcher, 0o500),
        ("bash", b"#!/bin/sh\nexit 0\n", 0o500),
        ("manifest_helper", b"# fixture manifest helper\n", 0o400),
        ("identity_verifier", b"# fixture identity verifier\n", 0o400),
        ("receipt_validator", b"# fixture receipt validator\n", 0o400),
        ("runtime_helper", b"# fixture protected runtime helper\n", 0o400),
        ("tool_probe_helper", fixture_tool_probe_helper_source(), 0o400),
        ("approval_contract", APPROVAL_CONTRACT.read_bytes(), 0o400),
        (
            "sdk_dependency_bundle_manifest",
            sdk_source_manifest_data,
            0o400,
        ),
    ):
        path = trust_dir / label
        path.write_bytes(data)
        path.chmod(mode)
        synthetic_sources[label] = path
    receipt_validator_support = trust_dir / "sumeragi_v2_localnet_manifest.py"
    shutil.copy2(
        ROOT_DIR / "scripts" / receipt_validator_support.name,
        receipt_validator_support,
    )
    receipt_validator_support.chmod(0o400)
    synthetic_sources["receipt_validator_support"] = receipt_validator_support
    runner_tool_data = {
        name: b"#!/bin/sh\nexit 97\n"
        for name in FIXTURE_TOOL_PROBE_OPERATION_IDS
    }
    runner_tool_data.update({
        "chmod": b"#!/bin/sh\nexit 0\n",
        "cargo": (
            b"#!/bin/sh\n"
            b"printf '%s\\n' 'receipt must not execute archived Cargo' >&2\n"
            b"exit 91\n"
        ),
        "rustc": (
            b"#!/bin/sh\n"
            b"test \"$#\" = 1 && test \"$1\" = -vV || exit 92\n"
            b"cat <<'RUSTC_VERSION'\n"
            + RUSTC_VERSION_OUTPUT
            + b"RUSTC_VERSION\n"
        ),
    })
    runner_tool_sources: dict[str, Path] = {}
    for name, data in runner_tool_data.items():
        source = trust_dir / f"runner-{name}"
        source.write_bytes(data)
        source.chmod(0o500)
        runner_tool_sources[name] = source
    runner_tool_manifest = trust_dir / "runner-tool-manifest.json"
    runner_tool_manifest.write_bytes(
        canonical_json(
            {
                "schema_version": 1,
                "tools": {
                    name: {
                        "path": str(source.resolve()),
                        "sha256": sha256(source),
                    }
                    for name, source in runner_tool_sources.items()
                },
            }
        )
    )
    runner_tool_manifest.chmod(0o400)
    synthetic_sources["runner_tool_manifest"] = runner_tool_manifest
    runner_tool_aliases: dict[str, Path] = {}
    for name, source in runner_tool_sources.items():
        archive = evidence_dir / "runner-tools" / name
        archive.write_bytes(source.read_bytes())
        archive.chmod(0o500)
        alias = evidence_dir / "runner-bin" / name
        alias.symlink_to(Path("..") / "runner-tools" / name)
        runner_tool_aliases[name] = alias
    runner_tool_probe_manifest_value = {
        "schema_version": 1,
        "tools": {
            name: {
                "archive_id": f"release-runner-tool.{name}.v1",
                "path": str((evidence_dir / "runner-tools" / name).resolve()),
                "sha256": sha256(evidence_dir / "runner-tools" / name),
            }
            for name in sorted(runner_tool_sources)
        },
    }
    runner_tool_probe_manifest = evidence_dir / "runner-tool-probe-manifest.json"
    runner_tool_probe_manifest.write_bytes(
        canonical_json(runner_tool_probe_manifest_value)
    )
    runner_tool_probe_manifest.chmod(0o400)
    runner_tool_probe_value = fixture_tool_probe_result(
        runner_tool_probe_manifest_value,
        archive_id_prefix="release-runner-tool",
    )
    runner_tool_probe_result = evidence_dir / "runner-tool-probes.json"
    runner_tool_probe_result.write_bytes(canonical_json(runner_tool_probe_value))
    runner_tool_probe_result.chmod(0o400)
    approval_module = _load_approval_component(
        synthetic_sources["approval_contract"]
    )
    approval_archives, release_approvals = _bootstrap_approval_fixture(
        module=approval_module,
        identity=identity,
        tool_manifest_sha256=sha256(runner_tool_manifest),
        trust_dir=trust_dir,
        evidence_dir=evidence_dir,
    )
    trusted_sources = {
        "allowed_signers": signature_allowed_signers,
        "bash": synthetic_sources["bash"],
        "bootstrap": frozen_bootstrap,
        "git": signature_git,
        "identity_verifier": synthetic_sources["identity_verifier"],
        "manifest_helper": synthetic_sources["manifest_helper"],
        "python": synthetic_sources["python"],
        "receipt_validator": synthetic_sources["receipt_validator"],
        "receipt_validator_support": synthetic_sources[
            "receipt_validator_support"
        ],
        "runtime_helper": synthetic_sources["runtime_helper"],
        "tool_probe_helper": synthetic_sources["tool_probe_helper"],
        "approval_contract": synthetic_sources["approval_contract"],
        **{
            label: trust_dir / label
            for label in approval_archives
        },
        "sdk_dependency_bundle_manifest": synthetic_sources[
            "sdk_dependency_bundle_manifest"
        ],
        "revocation": signature_revocation,
        "runner_tool_manifest": synthetic_sources["runner_tool_manifest"],
        "ssh_keygen": signature_ssh_keygen,
    }
    trusted_names = {
        "allowed_signers": ("bootstrap-allowed-signers", 0o400),
        "bash": ("bash", 0o500),
        "bootstrap": ("trusted-bootstrap.py", 0o400),
        "git": ("git", 0o500),
        "identity_verifier": ("verify-identity.py", 0o400),
        "manifest_helper": ("compute-manifest.py", 0o400),
        "python": ("python3", 0o500),
        "receipt_validator": ("validate-receipt.py", 0o400),
        "receipt_validator_support": (
            "sumeragi_v2_localnet_manifest.py",
            0o400,
        ),
        "runtime_helper": ("copy-release-runtime.py", 0o400),
        "tool_probe_helper": ("probe-release-tools.py", 0o400),
        "approval_contract": ("release-approval-contract.py", 0o400),
        "approval_offline_toolchain_sdk": (
            "offline-toolchain-sdk.approval.v1.json", 0o400
        ),
        "approval_formal_proof_tools": (
            "formal-proof-tools.approval.v1.json", 0o400
        ),
        "approval_network_scale_soak": (
            "network-scale-soak.approval.v1.json", 0o400
        ),
        "approval_final_bootstrap_publication": (
            "final-bootstrap-publication.approval.v1.json", 0o400
        ),
        "sdk_dependency_bundle_manifest": (
            "sdk-dependency-bundle-manifest.json",
            0o400,
        ),
        "revocation": ("bootstrap-revocation", 0o400),
        "runner_tool_manifest": ("runner-tool-manifest.json", 0o400),
        "ssh_keygen": ("ssh-keygen", 0o500),
    }
    trusted_records: dict[str, object] = {}
    trusted_archives: dict[str, Path] = {}
    for label, source in trusted_sources.items():
        archive_name, archive_mode = trusted_names[label]
        archive = evidence_dir / archive_name
        if archive.exists():
            archive.chmod(0o600)
        archive.write_bytes(source.read_bytes())
        archive.chmod(archive_mode)
        trusted_archives[label] = archive
        trusted_records[label] = {
            "archive_id": f"release-bootstrap.{label.replace('_', '-')}.v1",
            "archive_name": archive_name,
            "mode": f"{archive_mode:04o}",
            "sha256": sha256(source),
            "size_bytes": source.stat().st_size,
        }
    bootstrap_components: dict[str, object] = {}
    for source in BOOTSTRAP_COMPONENT_FILES:
        archive = evidence_dir / source.name
        archive.write_bytes(source.read_bytes())
        archive.chmod(0o400)
        bootstrap_components[source.name] = {
            "archive_id": (
                "release-bootstrap.bootstrap-component.v1:" + source.name
            ),
            "archive_name": source.name,
            "mode": "0400",
            "sha256": sha256(source),
            "size_bytes": source.stat().st_size,
        }
    trusted_records["bootstrap"]["components"] = bootstrap_components
    receipt_components: dict[str, object] = {}
    for source in RECEIPT_VALIDATOR_COMPONENT_FILES:
        archive = evidence_dir / source.name
        archive.write_bytes(source.read_bytes())
        archive.chmod(0o400)
        receipt_components[source.name] = {
            "archive_id": (
                "release-bootstrap.receipt-validator-component.v1:"
                + source.name
            ),
            "archive_name": source.name,
            "mode": "0400",
            "sha256": sha256(source),
            "size_bytes": source.stat().st_size,
        }
    trusted_records["receipt_validator"]["components"] = receipt_components

    bootstrap_identity = evidence_dir / "candidate-identity.json"
    bootstrap_identity.write_bytes(identity_bytes)
    bootstrap_identity.chmod(0o400)
    identity_paths = {
        "cargo_lock": evidence_dir / "identity-Cargo.lock",
        "git": evidence_dir / "identity-git",
        "identity_attestation": evidence_dir / "identity-attestation.json",
        "identity_transcript": evidence_dir / "identity-transcript.json",
        "raw_commit": evidence_dir / "identity-raw-commit",
        "ssh_allowed_signers": evidence_dir / "identity-allowed-signers",
        "ssh_keygen": evidence_dir / "identity-ssh-keygen",
        "ssh_revocation": evidence_dir / "identity-revocation",
    }
    for label, source in (
        ("cargo_lock", Path(candidate_root / "Cargo.lock")),
        ("git", trusted_archives["git"]),
        ("raw_commit", None),
        ("ssh_allowed_signers", trusted_archives["allowed_signers"]),
        ("ssh_keygen", trusted_archives["ssh_keygen"]),
        ("ssh_revocation", trusted_archives["revocation"]),
    ):
        path = identity_paths[label]
        path.write_bytes(raw_commit if source is None else source.read_bytes())
        path.chmod(0o500 if label in {"git", "ssh_keygen"} else 0o400)

    bootstrap_tools = {
        "git": {
            "archive_name": "identity-git",
            "mode": "0500",
            "observed_sha256": sha256(identity_paths["git"]),
            "protected_sha256": sha256(identity_paths["git"]),
            "size_bytes": identity_paths["git"].stat().st_size,
            "source_path": str(trusted_archives["git"]),
        },
        "ssh_keygen": {
            "archive_name": "identity-ssh-keygen",
            "mode": "0500",
            "observed_sha256": sha256(identity_paths["ssh_keygen"]),
            "protected_sha256": sha256(identity_paths["ssh_keygen"]),
            "size_bytes": identity_paths["ssh_keygen"].stat().st_size,
            "source_path": str(trusted_archives["ssh_keygen"]),
        },
    }
    bootstrap_policies = {
        "expected_signer_fingerprint": signer_fingerprint,
        "signature_format": "ssh",
        "ssh_allowed_signers": protected_metadata(
            identity_paths["ssh_allowed_signers"],
            0o400,
            sha256(trusted_archives["allowed_signers"]),
        ),
        "ssh_revocation": protected_metadata(
            identity_paths["ssh_revocation"],
            0o400,
            sha256(trusted_archives["revocation"]),
        ),
    }
    identity_archive_names = {
        "cargo_lock": "identity-Cargo.lock",
        "git": "identity-git",
        "raw_commit": "identity-raw-commit",
        "ssh_allowed_signers": "identity-allowed-signers",
        "ssh_keygen": "identity-ssh-keygen",
        "ssh_revocation": "identity-revocation",
        "verify_transcript": "identity-transcript.json",
    }
    stages = {
        label: evidence_dir / f".{name}.stage.{index:032x}"
        for index, (label, name) in enumerate(identity_archive_names.items(), 20)
    }
    historical_config = [
        "-c",
        "gpg.format=ssh",
        "-c",
        "gpg.minTrustLevel=fully",
        "-c",
        f"gpg.ssh.program={stages['ssh_keygen']}",
        "-c",
        f"gpg.ssh.allowedSignersFile={stages['ssh_allowed_signers']}",
        "-c",
        f"gpg.ssh.revocationFile={stages['ssh_revocation']}",
        "-c",
        f"gpg.program={stages['ssh_keygen']}",
        "-c",
        f"gpg.openpgp.program={stages['ssh_keygen']}",
        "-c",
        f"gpg.x509.program={stages['ssh_keygen']}",
    ]
    placeholder = "${EVIDENCE_DIRECTORY}"
    replay_config = [
        value.replace(str(stages["ssh_keygen"]), f"{placeholder}/identity-ssh-keygen")
        .replace(
            str(stages["ssh_allowed_signers"]),
            f"{placeholder}/identity-allowed-signers",
        )
        .replace(str(stages["ssh_revocation"]), f"{placeholder}/identity-revocation")
        for value in historical_config
    ]
    identity_environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": str(evidence_dir),
        "LANG": "C",
        "LANGUAGE": "C",
        "LC_ALL": "C",
        "PATH": os.defpath,
        "TZ": "UTC",
        "XDG_CONFIG_HOME": str(evidence_dir),
    }
    if sys.platform == "darwin":
        identity_environment["__CF_USER_TEXT_ENCODING"] = (
            f"0x{os.geteuid():X}:0x1:0xE"
        )
    head = identity["head_commit"]
    assert isinstance(head, str)
    bootstrap_transcript_value = {
        "format": "iroha-sumeragi-v2-release-identity-transcript",
        "schema_version": 3,
        "archive_ids": IDENTITY_ARCHIVE_IDS,
        "candidate_commit_oid": head,
        "operations": {
            "show_signature_metadata": command_record(
                [
                    str(stages["git"]),
                    *historical_config,
                    "show",
                    "--no-patch",
                    "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
                    head,
                ],
                [
                    f"{placeholder}/identity-git",
                    *replay_config,
                    "show",
                    "--no-patch",
                    "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
                    head,
                ],
                0,
                show_output,
                b"",
            ) | {"operation_id": "git.show-signature-metadata.ssh.v1"},
            "verify_commit": command_record(
                [
                    str(stages["git"]),
                    *historical_config,
                    "verify-commit",
                    "--raw",
                    head,
                ],
                [
                    f"{placeholder}/identity-git",
                    *replay_config,
                    "verify-commit",
                    "--raw",
                    head,
                ],
                0,
                b"",
                b"Good fixture SSH signature\n",
            ) | {"operation_id": "git.verify-commit.ssh.v1"},
            "ssh_keygen_usage": command_record(
                [str(stages["ssh_keygen"]), "-?"],
                [f"{placeholder}/identity-ssh-keygen", "-?"],
                1,
                b"",
                b"fixture ssh-keygen usage\n",
            ) | {"operation_id": "ssh-keygen.usage-probe.v1"},
        },
    }
    for operation in bootstrap_transcript_value["operations"].values():
        operation.pop("argv")
        operation.pop("replay_argv")
        operation.pop("stdout_base64")
        operation.pop("stderr_base64")
    identity_paths["identity_transcript"].write_bytes(
        canonical_json(bootstrap_transcript_value)
    )
    identity_paths["identity_transcript"].chmod(0o400)
    bootstrap_attestation_value = {
        "format": "iroha-sumeragi-v2-release-identity-attestation",
        "schema_version": 3,
        "candidate": {
            "commit_oid": identity["head_commit"],
            "tree_oid": identity["head_tree"],
            "source_manifest_sha256": identity[
                "workspace_source_manifest_sha256"
            ],
            "cargo_lock_sha256": identity["cargo_lock_sha256"],
            "release_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
        },
        "archives": {
            label: sanitized_identity_artifact(
                identity_paths["identity_transcript"]
                if label == "verify_transcript"
                else identity_paths[label],
                0o500 if label in {"git", "ssh_keygen"} else 0o400,
                label,
            )
            for label in identity_archive_names
        },
    }
    identity_paths["identity_attestation"].write_bytes(
        canonical_json(bootstrap_attestation_value)
    )
    identity_paths["identity_attestation"].chmod(0o400)

    identity_verification: dict[str, object] = {}
    for label, path in identity_paths.items():
        identity_verification[label] = artifact_metadata(
            path, 0o500 if label in {"git", "ssh_keygen"} else 0o400
        )
    identity_verification["verify_transcript"] = artifact_metadata(
        identity_paths["identity_transcript"], 0o400
    )
    closed_path_entries = [str(evidence_dir), str(evidence_dir / "runner-bin")]
    policy_environment = {
        "SUMERAGI_V2_RELEASE_RUNTIME_HELPER": str(
            trusted_archives["runtime_helper"]
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": sha256(
            trusted_archives["runtime_helper"]
        ),
        "SUMERAGI_V2_RELEASE_TOOL_PROBE_HELPER": str(
            trusted_archives["tool_probe_helper"]
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": sha256(
            trusted_archives["tool_probe_helper"]
        ),
        "SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN": str(trusted_archives["ssh_keygen"]),
        "SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256": sha256(trusted_archives["git"]),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256": sha256(
            trusted_archives["ssh_keygen"]
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT": signer_fingerprint,
        "SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS": str(
            trusted_archives["allowed_signers"]
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256": sha256(
            trusted_archives["allowed_signers"]
        ),
        "SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE": str(
            trusted_archives["revocation"]
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256": sha256(
            trusted_archives["revocation"]
        ),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION": str(
            evidence_dir / "BOOTSTRAP_COMPLETED.json"
        ),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION": str(
            identity_paths["identity_attestation"]
        ),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT": str(
            identity_paths["identity_transcript"]
        ),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY": str(bootstrap_identity),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR": str(evidence_dir),
    }
    alias_environment = {
        key.replace("SUMERAGI_V2_RELEASE_", "IROHA_RELEASE_", 1): value
        for key, value in policy_environment.items()
        if key.startswith("SUMERAGI_V2_RELEASE_BOOTSTRAP_")
    }
    alias_environment.update({
        "IROHA_RELEASE_RUNTIME_HELPER": str(
            trusted_archives["runtime_helper"]
        ),
        "IROHA_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": sha256(
            trusted_archives["runtime_helper"]
        ),
        "IROHA_RELEASE_TOOL_PROBE_HELPER": str(
            trusted_archives["tool_probe_helper"]
        ),
        "IROHA_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": sha256(
            trusted_archives["tool_probe_helper"]
        ),
        "IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST": str(
            trusted_archives["sdk_dependency_bundle_manifest"]
        ),
        "IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256": sha256(
            trusted_archives["sdk_dependency_bundle_manifest"]
        ),
    })
    runner_environment = {
        "HOME": str(evidence_dir / "home"),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.pathsep.join(closed_path_entries),
        "TMPDIR": str(evidence_dir / "tmp"),
        "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_COUNT": "2",
        "GIT_CONFIG_KEY_0": "core.hooksPath",
        "GIT_CONFIG_VALUE_0": os.devnull,
        "GIT_CONFIG_KEY_1": "core.fsmonitor",
        "GIT_CONFIG_VALUE_1": "false",
        "GIT_TERMINAL_PROMPT": "0",
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST": str(
            scaling_evidence_manifest
        ),
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": (
            scaling_trial_harness_sha256
        ),
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": (
            scaling_configuration_sha256
        ),
        "IROHA_RELEASE_SCALING_IROHAD_SHA256": scaling_irohad_sha256,
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": scaling_iroha_cli_sha256,
        **policy_environment,
        **alias_environment,
    }
    marker_value = {
        "schema_version": 2,
        "trust_boundary": {
            "bootstrap_authentication": "external prerequisite",
            "release_image_and_dynamic_loader": "external prerequisite",
            "same_uid_and_trusted_ancestor_owners": True,
        },
        "candidate_identity": identity,
        "candidate_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
        "trusted_inputs": trusted_records,
        "release_approvals": release_approvals,
        "identity_verification": identity_verification,
        "runner": {
            "archive_id": "release-candidate.runner.v1",
            "invocation": {
                "profile": "release",
                "operation_id": "sumeragi-v2.release.v1",
                "arguments": ["--release"],
                "bash_archive_id": "release-bootstrap.bash.v1",
            },
            "closed_path_resolution": {
                "bash": "release-bootstrap.bash.v1",
                "git": "release-bootstrap.git.v1",
                "python3": "release-bootstrap.python.v1",
            },
            "environment_sha256": hashlib.sha256(
                canonical_json(runner_environment)
            ).hexdigest(),
            "mode": f"{runner.stat().st_mode & 0o7777:04o}",
            "output": {
                "stderr_archive_id": "release-bootstrap.runner-stderr.v1",
                "stderr_name": "runner-stderr.log",
                "stdout_archive_id": "release-bootstrap.runner-stdout.v1",
                "stdout_name": "runner-stdout.log",
                "active_mode": "0600",
                "sealed_mode": "0400",
            },
            "self_digest_environment_variables": [
                "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            ],
            "sha256": sha256(runner),
            "size_bytes": runner.stat().st_size,
            "tool_directory": "runner-bin",
            "tools": {
                name: {
                    "archive_id": f"release-runner-tool.{name}.v1",
                    "alias_name": name,
                    "archive_name": f"runner-tools/{name}",
                    "mode": "0500",
                    "sha256": sha256(evidence_dir / "runner-tools" / name),
                    "size_bytes": (evidence_dir / "runner-tools" / name).stat().st_size,
                }
                for name, source in runner_tool_sources.items()
            },
        },
        "trusted_execution_probes": {
            "bash": {
                "argv": [str(trusted_archives["bash"]), "-c", ":"],
                "exit_status": 0,
            },
            "python": {
                "argv": [
                    str(trusted_archives["python"]),
                    "-I",
                    "-S",
                    "-c",
                    "import sys;sys.stdout.write(sys.executable+'\\n')",
                ],
                "expected_executable": "python3",
                "exit_status": 0,
                "stdout_sha256": hashlib.sha256(
                    f"{trusted_archives['python']}\n".encode()
                ).hexdigest(),
                "stdout_size_bytes": len(
                    f"{trusted_archives['python']}\n".encode()
                ),
            },
            "runner_tool_closure": {
                "manifest": {
                    "archive_id": (
                        "release-bootstrap.runner-tool-probe-manifest.v1"
                    ),
                    "archive_name": "runner-tool-probe-manifest.json",
                    "mode": "0400",
                    "sha256": sha256(runner_tool_probe_manifest),
                    "size_bytes": runner_tool_probe_manifest.stat().st_size,
                },
                "result": {
                    "archive_id": "release-bootstrap.runner-tool-probes.v1",
                    "archive_name": "runner-tool-probes.json",
                    "mode": "0400",
                    "sha256": sha256(runner_tool_probe_result),
                    "size_bytes": runner_tool_probe_result.stat().st_size,
                },
                "value": runner_tool_probe_value,
            },
        },
    }
    completion = evidence_dir / "BOOTSTRAP_COMPLETED.json"
    completion.write_bytes(canonical_json(marker_value))
    completion.chmod(0o400)
    return {
        "bootstrap_completion": completion,
        "bootstrap_evidence_dir": evidence_dir,
        "bootstrap_runner_environment": runner_environment,
        "bootstrap_identity": bootstrap_identity,
        "bootstrap_attestation": identity_paths["identity_attestation"],
        "bootstrap_transcript": identity_paths["identity_transcript"],
        "expected_bootstrap_completion_sha256": sha256(completion),
        "bootstrap_candidate_root": candidate_root,
        "bootstrap_runner": runner,
        "bootstrap_identity_cargo_lock": identity_paths["cargo_lock"],
        "bootstrap_identity_git": identity_paths["git"],
        "bootstrap_identity_raw_commit": identity_paths["raw_commit"],
        "bootstrap_identity_allowed_signers": identity_paths[
            "ssh_allowed_signers"
        ],
        "bootstrap_identity_ssh_keygen": identity_paths["ssh_keygen"],
        "bootstrap_identity_revocation": identity_paths["ssh_revocation"],
        "bootstrap_runner_cargo": evidence_dir / "runner-tools" / "cargo",
        "bootstrap_runner_rustc": evidence_dir / "runner-tools" / "rustc",
        "bootstrap_runner_tool_probe_manifest": runner_tool_probe_manifest,
        "bootstrap_runner_tool_probe_result": runner_tool_probe_result,
    }


def make_runtime_tool_probe_evidence(
    invocation_root: Path, bootstrap: dict[str, Path | str]
) -> dict[str, object]:
    """Copy the synthetic 41-tool closure and bind its path-free result."""

    evidence_dir = bootstrap["bootstrap_evidence_dir"]
    assert isinstance(evidence_dir, Path)
    runtime_root = invocation_root / "runtime"
    runtime_bin = runtime_root / "bin"
    runtime_bin.mkdir(parents=True, mode=0o700)
    runtime_bin.chmod(0o700)
    runtime_tools: dict[str, Path] = {}
    for name in sorted(FIXTURE_TOOL_PROBE_OPERATION_IDS):
        source = evidence_dir / "runner-tools" / name
        destination = runtime_bin / name
        shutil.copyfile(source, destination)
        destination.chmod(0o500)
        runtime_tools[name] = destination
    manifest_value = {
        "schema_version": 1,
        "tools": {
            name: {
                "archive_id": f"release-runtime-tool.{name}.v1",
                "path": str(path.resolve()),
                "sha256": sha256(path),
            }
            for name, path in sorted(runtime_tools.items())
        },
    }
    manifest = invocation_root / "runtime-tool-probe-manifest.json"
    manifest.write_bytes(canonical_json(manifest_value))
    manifest.chmod(0o400)
    result_value = fixture_tool_probe_result(
        manifest_value,
        archive_id_prefix="release-runtime-tool",
    )
    result = invocation_root / "runtime-tool-probe-result.json"
    result.write_bytes(canonical_json(result_value))
    result.chmod(0o400)
    return {
        "runtime_tool_probe_manifest": manifest,
        "runtime_tool_probe_result": result,
        "runtime_tool_probe_tools": runtime_tools,
    }


def make_scaling_evidence(
    tmp_path: Path, *, head: str, sealed_manifest: str
) -> dict[str, Path | str]:
    root = tmp_path / "scaling"
    inputs = root / "inputs"
    tooling_dir = root / "tooling"
    inputs.mkdir(parents=True)
    tooling_dir.mkdir()

    def write_json(path: Path, value: object) -> None:
        path.write_text(
            json.dumps(value, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def ref(path: Path) -> dict[str, str]:
        return {
            "path": path.relative_to(root).as_posix(),
            "sha256": sha256(path),
        }

    config = inputs / "nexus_config.toml"
    config.write_bytes(SCALING_CONFIGURATION_DATA)
    identity = {
        "schema": "iroha.sumeragi_v2.multilane_scaling.identity.v1",
        "hardware": {
            "machine_id": "receipt-contract-host",
            "cpu_model": "Receipt Contract CPU",
            "physical_core_count": 8,
            "logical_core_count": 16,
            "memory_bytes": 32_000_000_000,
            "storage_model": "Receipt Contract NVMe",
        },
        "software": {
            "os": "ContractOS",
            "kernel": "contract-kernel",
            "architecture": "x86_64",
            "python_version": "3.9.contract",
            "rustc_version": "rustc contract",
            "source_revision": head,
            "workspace_source_sha256": sealed_manifest,
            "nexus_config_sha256": sha256(config),
            "irohad_sha256": SCALING_IROHAD_SHA256,
            "iroha_cli_sha256": SCALING_IROHA_CLI_SHA256,
        },
    }
    identity_path = inputs / "identity.json"
    write_json(identity_path, identity)
    harness = inputs / "trial_harness.sh"
    harness.write_bytes(SCALING_TRIAL_HARNESS_DATA)

    validator_source = (
        ROOT_DIR / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py"
    )
    validator = tooling_dir / validator_source.name
    shutil.copy2(validator_source, validator)
    required_tooling = (
        ("localnet", "scripts/deploy_localnet.sh"),
        ("load_generator", "scripts/tx_load.py"),
        ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
    )
    tooling = []
    for role, source_path in required_tooling:
        source = ROOT_DIR / source_path
        artifact = tooling_dir / source.name
        shutil.copy2(source, artifact)
        tooling.append(
            {
                "role": role,
                "source_path": source_path,
                "artifact": ref(artifact),
            }
        )

    workload = {
        "offered_load_tps": 20.0,
        "warmup_seconds": 5.0,
        "measurement_seconds": 20.0,
        "min_interval_samples": 20,
        "min_latency_samples": 100,
        "max_offered_load_deviation_fraction": 0.01,
    }
    budgets = {
        "queue_depth_max": 100,
        "index_entries_max": 200,
        "memory_bytes_max": 10_000,
        "disk_bytes_max": 20_000,
    }
    namespace = "receipt-contract-g-scale"
    runs = []
    sequence = 0
    for pair_index in range(1, 6):
        seed = hashlib.sha256(
            f"{namespace}:{pair_index}".encode("utf-8")
        ).hexdigest()
        for variant, lane_count, committed, latency in (
            ("one_lane", 1, 100, 10.0),
            ("four_lane", 4, 160, 12.0),
        ):
            sequence += 1
            lane_ids = (
                ["lane-a"]
                if lane_count == 1
                else ["lane-a", "lane-b", "lane-c", "lane-d"]
            )
            run_dir = root / "runs" / f"pair_{pair_index:02d}" / variant
            support = run_dir / "support"
            support.mkdir(parents=True)
            lifecycle = support / "lifecycle.json"
            metrics = support / "metrics.prom"
            load_log = support / "tx_load.log"
            load_manifest = support / "load_test_manifest.json"
            write_json(lifecycle, {"active_execution_lanes": lane_ids})
            metrics.write_text(
                f"nexus_lane_configured_total {lane_count}\n", encoding="utf-8"
            )
            load_log.write_text("receipt scaling fixture\n", encoding="utf-8")
            write_json(
                load_manifest,
                {
                    "version": 1,
                    "lanes": lane_ids,
                    "workload_seed": seed,
                    "inputs": {
                        "status_file": lifecycle.name,
                        "metrics_file": metrics.name,
                    },
                },
            )
            offered_parts = [20] * 20
            accepted_parts = [20] * 20
            quotient, remainder = divmod(committed, 20)
            committed_parts = [
                quotient + (1 if index < remainder else 0)
                for index in range(20)
            ]
            samples = []
            for index, interval_committed in enumerate(committed_parts):
                samples.append(
                    {
                        "sequence": index + 1,
                        "start_offset_seconds": float(index),
                        "end_offset_seconds": float(index + 1),
                        "offered_count": offered_parts[index],
                        "accepted_count": accepted_parts[index],
                        "committed_count": interval_committed,
                        "commit_latencies_ms": [latency] * interval_committed,
                        "queue_depth": 12,
                        "index_entries": 24,
                        "memory_bytes": 1_019,
                        "disk_bytes": 2_019,
                    }
                )
            raw = run_dir / "raw_samples.json"
            write_json(
                raw,
                {
                    "schema": "iroha.sumeragi_v2.multilane_scaling.run.v1",
                    "pair_index": pair_index,
                    "variant": variant,
                    "active_execution_lanes": lane_count,
                    "execution_lane_ids": lane_ids,
                    "seed": seed,
                    "identity_before": identity,
                    "identity_after": identity,
                    "workload": workload,
                    "status": {
                        "outcome": "passed",
                        "skipped": False,
                        "failure": None,
                    },
                    "summary": {
                        "offered_count": 400,
                        "accepted_count": 400,
                        "committed_count": committed,
                        "queue_depth_max": 12,
                        "index_entries_max": 24,
                        "memory_bytes_max": 1_019,
                        "disk_bytes_max": 2_019,
                    },
                    "samples": samples,
                    "artifacts": {
                        "nexus_load_test_manifest": ref(load_manifest),
                        "lifecycle_snapshot": ref(lifecycle),
                        "metrics_snapshot": ref(metrics),
                        "load_generator_log": ref(load_log),
                    },
                },
            )
            command_log = run_dir / "trial.log"
            command_log.write_text("receipt scaling trial passed\n", encoding="utf-8")
            runs.append(
                {
                    "sequence": sequence,
                    "pair_index": pair_index,
                    "variant": variant,
                    "active_execution_lanes": lane_count,
                    "seed": seed,
                    "status": "passed",
                    "skipped": False,
                    "exit_code": 0,
                    "raw_samples": ref(raw),
                    "command_log": ref(command_log),
                }
            )

    manifest = root / "scaling_evidence.json"
    write_json(
        manifest,
        {
            "schema": "iroha.sumeragi_v2.multilane_scaling.evidence.v1",
            "generated_at_utc": "2026-07-23T12:00:00Z",
            "pair_count": 5,
            "seed_namespace": namespace,
            "seed_derivation": (
                "sha256(seed_namespace + ':' + decimal_pair_index)"
            ),
            "identity": ref(identity_path),
            "configuration": ref(config),
            "workload": workload,
            "budgets": budgets,
            "observation_scope": {
                "queue": "maximum per-peer queue depth",
                "index": "designated peer lane index entries",
                "memory": "aggregate peer RSS",
                "disk": "aggregate lane storage bytes",
            },
            "thresholds": {
                "min_four_lane_throughput_ratio": 1.5,
                "max_four_lane_p95_latency_ratio": 1.25,
            },
            "trial_harness": ref(harness),
            "validator": ref(validator),
            "tooling": tooling,
            "runs": runs,
        },
    )
    report = root / "validation_report.json"
    validation = subprocess.run(
        [
            sys.executable,
            str(validator_source),
            str(manifest),
            "--report",
            str(report),
            "--quiet",
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if validation.returncode != 0:
        raise AssertionError(validation.stderr)
    return {
        "scaling_root": root,
        "scaling_manifest": manifest,
        "scaling_report": report,
        "scaling_identity": identity_path,
        "scaling_validator": validator,
        "scaling_trial_log": root / runs[0]["command_log"]["path"],
        "expected_scaling_trial_harness_sha256": sha256(harness),
        "expected_scaling_configuration_sha256": sha256(config),
        "expected_scaling_irohad_sha256": SCALING_IROHAD_SHA256,
        "expected_scaling_iroha_cli_sha256": SCALING_IROHA_CLI_SHA256,
    }


def make_prebuilt_binary_bundle(
    artifact_root: Path,
    *,
    sealed_manifest: str,
    lock: str,
) -> dict[str, Path | str | list[Path]]:
    bundle = (
        artifact_root.resolve(strict=True)
        / "sumeragi-v2-release"
        / sealed_manifest
        / "programs"
        / "invocation.Fixture01"
    )
    release = bundle / "release"
    message_control_release = bundle / "message-control" / "release"
    release.mkdir(parents=True)
    message_control_release.mkdir(parents=True)
    binary_specs = (
        ("irohad", "release/iroha3d"),
        ("irohad_message_control", "message-control/release/iroha3d"),
        ("iroha", "release/iroha"),
        ("kagami", "release/kagami"),
    )
    binaries: list[Path] = []
    fields = {
        "schema_version": "2",
        "source_manifest_sha256": sealed_manifest,
        "cargo_lock_sha256": lock,
        "cargo_version_sha256": hashlib.sha256(CARGO_VERSION_OUTPUT).hexdigest(),
        "rustc_version_sha256": hashlib.sha256(RUSTC_VERSION_OUTPUT).hexdigest(),
        "host_triple": PREBUILT_HOST_TRIPLE,
        "target_triple": PREBUILT_HOST_TRIPLE,
        "profile": "release",
        "bundle_dir": str(bundle),
    }
    for prefix, relative in binary_specs:
        binary = bundle.joinpath(*relative.split("/"))
        binary.write_bytes(f"fixture-prebuilt-{prefix}\n".encode("ascii"))
        binary.chmod(0o500)
        binaries.append(binary)
        fields[f"{prefix}_relative_path"] = relative
        fields[f"{prefix}_sha256"] = sha256(binary)
        fields[f"{prefix}_size_bytes"] = str(binary.stat().st_size)
        fields[f"{prefix}_mode_octal"] = "0500"
    manifest = bundle / ".sumeragi-v2-prebuilt-binaries.tsv"
    write_tsv(manifest, fields)
    manifest.chmod(0o400)
    for directory in (
        release,
        message_control_release,
        bundle / "message-control",
        bundle,
    ):
        directory.chmod(0o500)
    return {
        "prebuilt_bundle": bundle,
        "prebuilt_manifest": manifest,
        "prebuilt_manifest_sha256": sha256(manifest),
        "prebuilt_binaries": binaries,
    }


def make_g4p_evidence(
    tmp_path: Path,
    *,
    head: str,
    tree: str,
    sealed_manifest: str,
    lock: str,
    prebuilt_manifest_sha256: str,
) -> dict[str, Path | list[Path]]:
    evidence_dir = tmp_path / "g4p"
    evidence_dir.mkdir()
    release_tests = (
        (
            "nexus_and_streaming",
            "nexus::autoscale_localnet::"
            "nexus_autoscale_four_peer_release_lifecycle_recreates_lane_and_"
            "rejects_stale_artifacts",
        ),
        (
            "nexus_and_streaming",
            "nexus::autoscale_localnet::"
            "nexus_autoscale_certified_merge_recovers_missing_sidecar_after_restart",
        ),
        (
            "nexus_and_streaming",
            "nexus::autoscale_localnet::"
            "nexus_autoscale_two_phase_drain_closes_certifies_then_retires_after_"
            "restart",
        ),
        (
            "native_amx_routing",
            "native_amx_rotating_validator_fault_soak_preserves_independent_"
            "participant_qcs",
        ),
    )

    logs = []
    summary_lines = ["target\ttest\tstatus\tlog_sha256\tlog"]
    for index, (target, test) in enumerate(release_tests):
        log = evidence_dir / f"run-{index:02d}-{target}.log"
        release_markers = []
        if index in (0, 3):
            release_markers.append(
                f"[multilane-release-gate] started: {test}"
            )
        if target == "native_amx_routing":
            release_markers.append(
                "[multilane-release-native-evidence] grouped_sources=2 "
                "durable_manifest=passed body_eviction_recovery=passed "
                "authenticated_remote_recovery=passed exact_once=passed"
            )
        if index in (0, 3):
            release_markers.append(
                f"[multilane-release-gate] completed: {test}"
            )
        log.write_text(
            "\n".join(
                (
                    "running 1 test",
                    *release_markers,
                    f"test {test} ... ok",
                    "",
                    "test result: ok. 1 passed; 0 failed; 0 ignored; "
                    "0 measured; 7 filtered out; finished in 0.01s",
                )
            )
            + "\n",
            encoding="utf-8",
        )
        logs.append(log)
        summary_lines.append(
            f"{target}\t{test}\tpassed\t{sha256(log)}\t{log.name}"
        )
    summary = evidence_dir / "runs.tsv"
    summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    completion = evidence_dir / "COMPLETED.tsv"
    write_tsv(
        completion,
        {
            "schema_version": "1",
            "mode": "mandatory-four-peer-multilane-release",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
            "expected_runs": "4",
            "passed_runs": "4",
            "failed_runs": "0",
            "skipped_runs": "0",
            "native_grouped_pruning_evidence": "passed",
            "runs_sha256": sha256(summary),
        },
    )
    return {
        "g4p_completion": completion,
        "g4p_summary": summary,
        "g4p_logs": logs,
        "g4p_log": logs[0],
    }


def make_g12_evidence(
    tmp_path: Path,
    *,
    head: str,
    tree: str,
    sealed_manifest: str,
    lock: str,
    prebuilt_manifest_sha256: str,
) -> dict[str, Path | list[Path]]:
    seed_dir = tmp_path / "g12-seed"
    soak_dir = tmp_path / "g12-soak"
    seed_dir.mkdir()
    soak_dir.mkdir()
    seed_test = (
        "nexus::cross_dataspace_localnet::"
        "cross_dataspace_atomic_swap_is_all_or_nothing"
    )
    soak_test = (
        "nexus::cross_dataspace_localnet::"
        "cross_dataspace_two_hour_fault_soak_preserves_multilane_application"
    )

    def passing_log(test: str, filtered: int) -> str:
        return "\n".join(
            (
                "running 1 test",
                f"test {test} ... ok",
                "",
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                f"{filtered} filtered out; finished in 0.01s",
            )
        ) + "\n"

    seed_logs = []
    summary_lines = [
        "ordinal\tseed\tstatus\tprocess_retries\tlog_sha256\tlog"
    ]
    for ordinal in range(10):
        log = seed_dir / f"seed-{ordinal:02d}.log"
        log.write_text(passing_log(seed_test, 99), encoding="utf-8")
        seed_logs.append(log)
        summary_lines.append(
            f"{ordinal}\tnexus-cross-dataspace-v1-seed-{ordinal:02d}\t"
            f"passed\t0\t{sha256(log)}\t{log.name}"
        )
    seed_summary = seed_dir / "runs.tsv"
    seed_summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    seed_completion = seed_dir / "COMPLETED.tsv"
    write_tsv(
        seed_completion,
        {
            "schema_version": "1",
            "mode": "deterministic-seed-matrix",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
            "expected_runs": "10",
            "passed_runs": "10",
            "failed_runs": "0",
            "process_retry_runs": "0",
            "runs_sha256": sha256(seed_summary),
        },
    )

    soak_log = soak_dir / "fault-soak.log"
    soak_log.write_text(passing_log(soak_test, 7), encoding="utf-8")
    soak_completion = soak_dir / "COMPLETED.tsv"
    write_tsv(
        soak_completion,
        {
            "schema_version": "1",
            "mode": "two-hour-fault-soak",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
            "seed": "nexus-cross-dataspace-v1-seed-00",
            "duration_seconds": "7200",
            "expected_runs": "1",
            "passed_runs": "1",
            "failed_runs": "0",
            "process_retry_runs": "0",
            "log_sha256": sha256(soak_log),
        },
    )
    return {
        "g12_seed_completion": seed_completion,
        "g12_seed_summary": seed_summary,
        "g12_seed_logs": seed_logs,
        "g12_seed_log": seed_logs[3],
        "g12_soak_completion": soak_completion,
        "g12_soak_log": soak_log,
    }


def _sdk_dependency_material() -> tuple[bytes, bytes, bytes, bytes, dict[str, bytes], list[dict[str, object]], list[dict[str, object]]]:
    package_lock = canonical_json(
        {
            "name": "sdk-fixture",
            "version": "1.0.0",
            "lockfileVersion": 3,
            "packages": {
                "": {"name": "sdk-fixture", "version": "1.0.0"},
                "node_modules/fixture": {"version": "1.0.0"},
            },
        }
    )
    installed_lock = canonical_json(
        {
            "name": "sdk-fixture",
            "version": "1.0.0",
            "lockfileVersion": 3,
            "packages": {"node_modules/fixture": {"version": "1.0.0"}},
        }
    )
    resolved_revision = "a" * 40
    package_resolved = canonical_json(
        {
            "pins": [
                {
                    "identity": "fixture",
                    "kind": "remoteSourceControl",
                    "location": "https://example.invalid/fixture.git",
                    "state": {"revision": resolved_revision, "version": "1.0.0"},
                }
            ],
            "version": 2,
        }
    )
    wrapper = (
        b"distributionBase=GRADLE_USER_HOME\n"
        b"distributionPath=wrapper/dists\n"
        b"distributionUrl=https\\://services.gradle.org/distributions/gradle-9.3.0-bin.zip\n"
        b"zipStoreBase=GRADLE_USER_HOME\n"
        b"zipStorePath=wrapper/dists\n"
    )
    files = {
        "gradle/gradle-9.3.0-bin.zip": b"fixture Gradle 9.3.0 distribution\n",
        (
            "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
            "79n14ral3mx1ozqr3csh2u872/gradle-9.3.0/bin/gradle"
        ): b"#!/bin/sh\n",
        (
            "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
            "79n14ral3mx1ozqr3csh2u872/gradle-9.3.0-bin.zip.ok"
        ): b"",
        "gradle/java-gradle-wrapper.properties": wrapper,
        "gradle/kotlin-gradle-wrapper.properties": wrapper,
        "node/node_modules/.package-lock.json": installed_lock,
        "node/node_modules/fixture/index.js": b"export const fixture = true;\n",
        "node/package-lock.json": package_lock,
        "openapi/node_modules/.package-lock.json": installed_lock,
        "openapi/node_modules/fixture/index.js": b"export const openapiFixture = true;\n",
        "openapi/package-lock.json": package_lock,
        "swiftpm/cache/checkouts/fixture/.git/HEAD": (
            resolved_revision + "\n"
        ).encode("ascii"),
        "swiftpm/cache/checkouts/fixture/Sources/Fixture.swift": (
            b"public let fixture = true\n"
        ),
        "swiftpm/Package.resolved": package_resolved,
    }
    directories = (
        "gradle",
        "gradle/gradle-user-home",
        "gradle/gradle-user-home/caches",
        "gradle/gradle-user-home/caches/9.3.0",
        "gradle/gradle-user-home/caches/modules-2",
        "gradle/gradle-user-home/wrapper",
        "gradle/gradle-user-home/wrapper/dists",
        "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin",
        (
            "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
            "79n14ral3mx1ozqr3csh2u872"
        ),
        (
            "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
            "79n14ral3mx1ozqr3csh2u872/gradle-9.3.0"
        ),
        (
            "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
            "79n14ral3mx1ozqr3csh2u872/gradle-9.3.0/bin"
        ),
        "node",
        "node/node_modules",
        "node/node_modules/fixture",
        "openapi",
        "openapi/node_modules",
        "openapi/node_modules/fixture",
        "swiftpm",
        "swiftpm/cache",
        "swiftpm/cache/checkouts",
        "swiftpm/cache/checkouts/fixture",
        "swiftpm/cache/checkouts/fixture/.git",
        "swiftpm/cache/checkouts/fixture/Sources",
        "swiftpm/cache/repositories",
    )
    records: list[dict[str, object]] = [
        {"path": ".", "kind": "directory", "mode": "0500"}
    ]
    records.extend(
        {"path": path, "kind": "directory", "mode": "0500"}
        for path in directories
    )
    records.extend(
        {
            "path": path,
            "kind": "file",
            "mode": (
                "0500" if path.endswith("/bin/gradle") else "0400"
            ),
            "size": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }
        for path, data in files.items()
    )
    records = [records[0], *sorted(records[1:], key=lambda item: str(item["path"]))]
    work_records = [
        {"path": ".", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/caches", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/caches/9.3.0", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/caches/modules-2", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/wrapper", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/wrapper/dists", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/wrapper/dists/gradle-9.3.0-bin", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/wrapper/dists/gradle-9.3.0-bin/79n14ral3mx1ozqr3csh2u872", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/wrapper/dists/gradle-9.3.0-bin/79n14ral3mx1ozqr3csh2u872/gradle-9.3.0", "kind": "directory", "mode": "0700"},
        {"path": "gradle-home/wrapper/dists/gradle-9.3.0-bin/79n14ral3mx1ozqr3csh2u872/gradle-9.3.0/bin", "kind": "directory", "mode": "0700"},
        {
            "path": "gradle-home/wrapper/dists/gradle-9.3.0-bin/79n14ral3mx1ozqr3csh2u872/gradle-9.3.0/bin/gradle",
            "kind": "file", "mode": "0700", "size": len(b"#!/bin/sh\n"),
            "sha256": hashlib.sha256(b"#!/bin/sh\n").hexdigest(),
        },
        {
            "path": "gradle-home/wrapper/dists/gradle-9.3.0-bin/79n14ral3mx1ozqr3csh2u872/gradle-9.3.0-bin.zip.ok",
            "kind": "file", "mode": "0600", "size": 0,
            "sha256": hashlib.sha256(b"").hexdigest(),
        },
        {"path": "swiftpm", "kind": "directory", "mode": "0700"},
        {"path": "swiftpm/checkouts", "kind": "directory", "mode": "0700"},
        {"path": "swiftpm/checkouts/fixture", "kind": "directory", "mode": "0700"},
        {"path": "swiftpm/checkouts/fixture/.git", "kind": "directory", "mode": "0700"},
        {
            "path": "swiftpm/checkouts/fixture/.git/HEAD", "kind": "file",
            "mode": "0600", "size": len((resolved_revision + "\n").encode("ascii")),
            "sha256": hashlib.sha256((resolved_revision + "\n").encode("ascii")).hexdigest(),
        },
        {"path": "swiftpm/checkouts/fixture/Sources", "kind": "directory", "mode": "0700"},
        {
            "path": "swiftpm/checkouts/fixture/Sources/Fixture.swift",
            "kind": "file", "mode": "0600",
            "size": len(b"public let fixture = true\n"),
            "sha256": hashlib.sha256(b"public let fixture = true\n").hexdigest(),
        },
        {"path": "swiftpm/repositories", "kind": "directory", "mode": "0700"},
    ]
    work_records = [
        work_records[0],
        *sorted(work_records[1:], key=lambda item: str(item["path"])),
    ]
    return (
        package_lock,
        installed_lock,
        package_resolved,
        wrapper,
        files,
        records,
        work_records,
    )


def _sdk_source_manifest_fixture(git_path: Path, git_sha256: str) -> bytes:
    package_lock, _, package_resolved, wrapper, files, records, _ = _sdk_dependency_material()

    def source_inventory(prefix: str) -> dict[str, object]:
        projected: list[dict[str, object]] = []
        for record in records:
            relative = str(record["path"])
            if relative != prefix and not relative.startswith(prefix + "/"):
                continue
            item = dict(record)
            item["path"] = (
                "." if relative == prefix else relative.removeprefix(prefix + "/")
            )
            if item["kind"] == "directory":
                item["mode"] = "0700"
            elif item["kind"] == "file":
                item["mode"] = (
                    "0700" if int(str(item["mode"]), 8) & 0o111 else "0600"
                )
            projected.append(item)
        payload = json.dumps(
            projected, ensure_ascii=True, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
        return {
            "file_bytes": sum(
                int(item["size"])
                for item in projected
                if item["kind"] == "file"
            ),
            "format": "iroha-sumeragi-v2-sdk-dependency-source-inventory",
            "record_count": len(projected),
            "records": projected,
            "records_sha256": hashlib.sha256(payload).hexdigest(),
            "schema_version": 1,
        }

    revision = "a" * 40
    tree = "b" * 40
    return canonical_json(
        {
            "format": "iroha-sumeragi-v2-sdk-dependency-sources",
            "git": {
                "executable": str(git_path.resolve()),
                "sha256": git_sha256,
            },
            "gradle": {
                "distribution_archive": "/operator/gradle-9.3.0-bin.zip",
                "distribution_sha256": hashlib.sha256(
                    files["gradle/gradle-9.3.0-bin.zip"]
                ).hexdigest(),
                "distribution_url": (
                    "https://services.gradle.org/distributions/"
                    "gradle-9.3.0-bin.zip"
                ),
                "gradle_user_home": "/operator/gradle-home",
                "gradle_user_home_inventory": source_inventory(
                    "gradle/gradle-user-home"
                ),
                "java_wrapper_properties_sha256": hashlib.sha256(wrapper).hexdigest(),
                "kotlin_wrapper_properties_sha256": hashlib.sha256(wrapper).hexdigest(),
                "version": "9.3.0",
                "wrapper_cache_key": "79n14ral3mx1ozqr3csh2u872",
            },
            "node": {
                "node_modules_inventory": source_inventory("node/node_modules"),
                "node_modules_root": "/operator/node_modules",
                "package_lock_sha256": hashlib.sha256(package_lock).hexdigest(),
            },
            "openapi_node": {
                "node_modules_inventory": source_inventory("openapi/node_modules"),
                "node_modules_root": "/operator/tools/openapi/node_modules",
                "package_lock_sha256": hashlib.sha256(package_lock).hexdigest(),
            },
            "schema_version": 3,
            "swiftpm": {
                "cache_inventory": source_inventory("swiftpm/cache"),
                "cache_root": "/operator/swiftpm-cache",
                "package_resolved_sha256": hashlib.sha256(
                    package_resolved
                ).hexdigest(),
                "resolved_revisions": [
                    {
                        "checkout": "fixture",
                        "identity": "fixture",
                        "revision": revision,
                        "tree": tree,
                    }
                ],
            },
        }
    )


def make_sdk_dependency_evidence(
    invocation_root: Path, *, source_manifest_sha256: str
) -> dict[str, Path]:
    package_lock, installed_lock, package_resolved, wrapper, files, records, work_records = _sdk_dependency_material()
    resolved_revision = "a" * 40
    resolved_tree = "b" * 40
    archive_path = invocation_root / "sdk-dependency-bundle.tar"
    with archive_path.open("xb") as raw:
        with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as archive:
            for record in records:
                relative = str(record["path"])
                member = tarfile.TarInfo(
                    "sdk-inputs" if relative == "." else f"sdk-inputs/{relative}"
                )
                member.mode = int(str(record["mode"]), 8)
                member.uid = member.gid = member.mtime = 0
                member.uname = member.gname = ""
                if record["kind"] == "directory":
                    member.type = tarfile.DIRTYPE
                    archive.addfile(member)
                else:
                    data = files[relative]
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
    archive_path.chmod(0o400)
    archive_record = {
        "archive_id": "release-sdk-dependencies.bundle.v1",
        "archive_name": archive_path.name,
        "mode": "0400",
        "size_bytes": archive_path.stat().st_size,
        "sha256": sha256(archive_path),
    }
    inventory = {
        "format": "iroha-sumeragi-v2-sdk-dependency-bundle",
        "schema_version": 1,
        "archive_id": "release-sdk-dependencies.bundle.v1",
        "source_disclosure": "withheld",
        "source_manifest_sha256": source_manifest_sha256,
        "source_state_sha256": "e" * 64,
        "bindings": {
            "node": {
                "node_modules_archive_name": "node/node_modules",
                "package_lock_archive_name": "node/package-lock.json",
                "package_lock_sha256": hashlib.sha256(package_lock).hexdigest(),
                "installed_lock_sha256": hashlib.sha256(installed_lock).hexdigest(),
            },
            "openapi_node": {
                "node_modules_archive_name": "openapi/node_modules",
                "package_lock_archive_name": "openapi/package-lock.json",
                "package_lock_sha256": hashlib.sha256(package_lock).hexdigest(),
                "installed_lock_sha256": hashlib.sha256(installed_lock).hexdigest(),
            },
            "swiftpm": {
                "cache_archive_name": "swiftpm/cache",
                "package_resolved_archive_name": "swiftpm/Package.resolved",
                "package_resolved_sha256": hashlib.sha256(package_resolved).hexdigest(),
                "resolved_revisions": [
                    {
                        "identity": "fixture",
                        "checkout": "fixture",
                        "revision": resolved_revision,
                        "tree": resolved_tree,
                    }
                ],
            },
            "gradle": {
                "distribution_archive_name": "gradle/gradle-9.3.0-bin.zip",
                "distribution_sha256": hashlib.sha256(
                    files["gradle/gradle-9.3.0-bin.zip"]
                ).hexdigest(),
                "distribution_url": (
                    "https://services.gradle.org/distributions/"
                    "gradle-9.3.0-bin.zip"
                ),
                "gradle_user_home_archive_name": "gradle/gradle-user-home",
                "launcher_archive_name": (
                    "gradle/gradle-user-home/wrapper/dists/"
                    "gradle-9.3.0-bin/79n14ral3mx1ozqr3csh2u872/"
                    "gradle-9.3.0/bin/gradle"
                ),
                "wrapper_cache_key": "79n14ral3mx1ozqr3csh2u872",
                "version": "9.3.0",
                "wrapper_properties_sha256": {
                    "java": hashlib.sha256(wrapper).hexdigest(),
                    "kotlin": hashlib.sha256(wrapper).hexdigest(),
                },
            },
        },
        "archive": archive_record,
        "record_count": len(records),
        "file_bytes": sum(len(data) for data in files.values()),
        "records": records,
        "work_initial_record_count": len(work_records),
        "work_initial_file_bytes": sum(
            int(record.get("size", 0)) for record in work_records
        ),
        "work_initial_records": work_records,
    }
    input_inventory = invocation_root / "sdk-dependency-input.json"
    input_inventory.write_bytes(canonical_json(inventory))
    input_inventory.chmod(0o400)
    final_inventory = invocation_root / "sdk-dependency-work-final.json"
    final_inventory.write_bytes(
        canonical_json(
            {
                "format": "iroha-sumeragi-v2-sdk-dependency-work-final",
                "schema_version": 1,
                "archive_id": "release-sdk-dependencies.work-final.v1",
                "sdk_dependency_inventory_sha256": sha256(input_inventory),
                "record_count": len(work_records),
                "file_bytes": sum(
                    int(record.get("size", 0)) for record in work_records
                ),
                "records": work_records,
            }
        )
    )
    final_inventory.chmod(0o400)
    return {
        "sdk_dependency_archive": archive_path,
        "sdk_dependency_input_inventory": input_inventory,
        "sdk_dependency_final_work_inventory": final_inventory,
    }


def make_evidence(tmp_path: Path) -> dict[str, Path | str | list[Path]]:
    candidate_manifest = "a" * 64
    sealed_manifest = "b" * 64
    multilane_manifest = "c" * 64
    tree = "2" * 40
    lock_bytes = b"fixture Cargo.lock\n"
    lock = hashlib.sha256(lock_bytes).hexdigest()
    signer_fingerprint = "SHA256:" + "A" * 43
    signer_principal = "release@example.test"
    signature_payload = base64.b64encode(b"SSHSIG fixture signature").decode("ascii")
    raw_commit = (
        f"tree {tree}\n"
        "author Release Fixture <release@example.test> 1700000000 +0000\n"
        "committer Release Fixture <release@example.test> 1700000000 +0000\n"
        "gpgsig -----BEGIN SSH SIGNATURE-----\n"
        f" {signature_payload}\n"
        " -----END SSH SIGNATURE-----\n"
        "\n"
        "Sumeragi v2 release fixture\n"
        "\n"
        "Sumeragi-V2-Release-Identity-Version: 1\n"
        f"Sumeragi-V2-Source-Manifest-SHA256: {candidate_manifest}\n"
        f"Sumeragi-V2-Cargo-Lock-SHA256: {lock}\n"
    ).encode("utf-8")
    framed_commit = b"commit " + str(len(raw_commit)).encode("ascii") + b"\0" + raw_commit
    head = hashlib.sha1(framed_commit, usedforsecurity=False).hexdigest()
    candidate = tmp_path / "candidate.json"
    sealed = tmp_path / "sealed.json"
    identity = {
        "schema_version": 1,
        "head_commit": head,
        "head_tree": tree,
        "index_tree": tree,
        "workspace_source_manifest_sha256": candidate_manifest,
        "cargo_lock_sha256": lock,
    }
    candidate.write_bytes(canonical_json(identity))
    identity["workspace_source_manifest_sha256"] = sealed_manifest
    sealed.write_bytes(canonical_json(identity))

    signature_dir = tmp_path / "release-identity-source"
    signature_dir.mkdir(mode=0o700)
    signature_dir.chmod(0o700)
    signature_attestation = signature_dir / "RELEASE_IDENTITY_VERIFIED.json"
    signature_transcript = signature_dir / "verify-transcript.json"
    signature_raw_commit = signature_dir / "raw-commit"
    signature_cargo_lock = signature_dir / "Cargo.lock"
    signature_allowed_signers = signature_dir / "allowed-signers"
    signature_revocation = signature_dir / "revocation-file"
    signature_git = signature_dir / "git"
    signature_ssh_keygen = signature_dir / "ssh-keygen"

    signature_raw_commit.write_bytes(raw_commit)
    signature_cargo_lock.write_bytes(lock_bytes)
    signature_allowed_signers.write_text(
        f"{signer_principal} ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFixtureKey\n",
        encoding="utf-8",
    )
    signature_revocation.write_bytes(b"")
    fake_git = (
        "#!/bin/sh\n"
        "case \"$*\" in\n"
        "  'rev-parse --show-toplevel') pwd -P ;;\n"
        f"  'rev-parse --verify HEAD^{{commit}}') printf '%s\\n' {head} ;;\n"
        f"  'rev-parse --verify {head}^{{tree}}') printf '%s\\n' {tree} ;;\n"
        f"  'cat-file commit {head}') cat <<'FIXTURE_RAW_COMMIT'\n"
        + raw_commit.decode("utf-8")
        + "FIXTURE_RAW_COMMIT\n"
        f"  ;;\n  *'verify-commit --raw {head}') printf '%s\\n' "
        "'Good fixture SSH signature' >&2 ;;\n"
        f"  *'show --no-patch --format=%G?%x00%GF%x00%GP%x00%GS%x00 {head}') "
        f"printf 'G\\000%s\\000\\000%s\\000\\n' {signer_fingerprint} {signer_principal} ;;\n"
        "  *) printf 'unexpected fake Git argv: %s\\n' \"$*\" >&2; exit 91 ;;\n"
        "esac\n"
    )
    signature_git.write_text(fake_git, encoding="utf-8")
    signature_ssh_keygen.write_text(
        "#!/bin/sh\nprintf '%s\\n' 'fixture ssh-keygen usage' >&2\nexit 1\n",
        encoding="utf-8",
    )
    for path in (
        signature_raw_commit,
        signature_cargo_lock,
        signature_allowed_signers,
        signature_revocation,
    ):
        path.chmod(0o400)
    signature_git.chmod(0o500)
    signature_ssh_keygen.chmod(0o500)

    protected_git = sha256(signature_git)
    protected_ssh = sha256(signature_ssh_keygen)
    protected_allowed = sha256(signature_allowed_signers)
    protected_revocation = sha256(signature_revocation)
    archive_names = {
        "cargo_lock": "Cargo.lock",
        "git": "git",
        "raw_commit": "raw-commit",
        "ssh_allowed_signers": "allowed-signers",
        "ssh_keygen": "ssh-keygen",
        "ssh_revocation": "revocation-file",
        "verify_transcript": "verify-transcript.json",
    }
    stage_paths = {
        name: signature_dir / f".{archive_name}.stage.{index:032x}"
        for index, (name, archive_name) in enumerate(archive_names.items(), 1)
    }
    historical_config = [
        "-c",
        "gpg.format=ssh",
        "-c",
        "gpg.minTrustLevel=fully",
        "-c",
        f"gpg.ssh.program={stage_paths['ssh_keygen']}",
        "-c",
        f"gpg.ssh.allowedSignersFile={stage_paths['ssh_allowed_signers']}",
        "-c",
        f"gpg.ssh.revocationFile={stage_paths['ssh_revocation']}",
        "-c",
        f"gpg.program={stage_paths['ssh_keygen']}",
        "-c",
        f"gpg.openpgp.program={stage_paths['ssh_keygen']}",
        "-c",
        f"gpg.x509.program={stage_paths['ssh_keygen']}",
    ]
    evidence_placeholder = "${EVIDENCE_DIRECTORY}"
    replay_config = [
        "-c",
        "gpg.format=ssh",
        "-c",
        "gpg.minTrustLevel=fully",
        "-c",
        f"gpg.ssh.program={evidence_placeholder}/ssh-keygen",
        "-c",
        f"gpg.ssh.allowedSignersFile={evidence_placeholder}/allowed-signers",
        "-c",
        f"gpg.ssh.revocationFile={evidence_placeholder}/revocation-file",
        "-c",
        f"gpg.program={evidence_placeholder}/ssh-keygen",
        "-c",
        f"gpg.openpgp.program={evidence_placeholder}/ssh-keygen",
        "-c",
        f"gpg.x509.program={evidence_placeholder}/ssh-keygen",
    ]
    tools = {
        "git": {
            **protected_metadata(signature_git, 0o500, protected_git),
            "source_path": str((tmp_path / "source-tools" / "git").resolve()),
        },
        "ssh_keygen": {
            **protected_metadata(signature_ssh_keygen, 0o500, protected_ssh),
            "source_path": str((tmp_path / "source-tools" / "ssh-keygen").resolve()),
        },
    }
    policies = {
        "expected_signer_fingerprint": signer_fingerprint,
        "signature_format": "ssh",
        "ssh_allowed_signers": protected_metadata(
            signature_allowed_signers, 0o400, protected_allowed
        ),
        "ssh_revocation": protected_metadata(
            signature_revocation, 0o400, protected_revocation
        ),
    }
    closed_environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": str(signature_dir),
        "LANG": "C",
        "LANGUAGE": "C",
        "LC_ALL": "C",
        "PATH": os.defpath,
        "TZ": "UTC",
        "XDG_CONFIG_HOME": str(signature_dir),
    }
    if sys.platform == "darwin":
        closed_environment["__CF_USER_TEXT_ENCODING"] = (
            f"0x{os.geteuid():X}:0x1:0xE"
        )
    show_output = (
        f"G\0{signer_fingerprint}\0\0{signer_principal}\0\n"
    ).encode("utf-8")
    transcript = {
        "format": "iroha-sumeragi-v2-release-identity-transcript",
        "schema_version": 3,
        "archive_ids": IDENTITY_ARCHIVE_IDS,
        "candidate_commit_oid": head,
        "operations": {
            "show_signature_metadata": sanitized_operation(
                "git.show-signature-metadata.ssh.v1", 0, show_output, b""
            ),
            "verify_commit": sanitized_operation(
                "git.verify-commit.ssh.v1",
                0,
                b"",
                b"Good fixture SSH signature\n",
            ),
            "ssh_keygen_usage": sanitized_operation(
                "ssh-keygen.usage-probe.v1",
                1,
                b"",
                b"fixture ssh-keygen usage\n",
            ),
        },
    }
    signature_transcript.write_bytes(canonical_json(transcript))
    signature_transcript.chmod(0o400)
    candidate_identity = json.loads(candidate.read_text(encoding="utf-8"))
    attestation = {
        "format": "iroha-sumeragi-v2-release-identity-attestation",
        "schema_version": 3,
        "candidate": {
            "commit_oid": candidate_identity["head_commit"],
            "tree_oid": candidate_identity["head_tree"],
            "source_manifest_sha256": candidate_identity[
                "workspace_source_manifest_sha256"
            ],
            "cargo_lock_sha256": candidate_identity["cargo_lock_sha256"],
            "release_identity_sha256": sha256(candidate),
        },
        "archives": {
            label: sanitized_identity_artifact(
                signature_transcript
                if label == "verify_transcript"
                else {
                    "cargo_lock": signature_cargo_lock,
                    "git": signature_git,
                    "raw_commit": signature_raw_commit,
                    "ssh_allowed_signers": signature_allowed_signers,
                    "ssh_keygen": signature_ssh_keygen,
                    "ssh_revocation": signature_revocation,
                }[label],
                0o500 if label in {"git", "ssh_keygen"} else 0o400,
                label,
            )
            for label in archive_names
        },
    }
    signature_attestation.write_bytes(canonical_json(attestation))
    signature_attestation.chmod(0o400)
    bootstrap = make_bootstrap_evidence(
        tmp_path,
        identity=candidate_identity,
        identity_bytes=candidate.read_bytes(),
        raw_commit=raw_commit,
        lock_bytes=lock_bytes,
        signer_fingerprint=signer_fingerprint,
        signer_principal=signer_principal,
        signature_git=signature_git,
        signature_ssh_keygen=signature_ssh_keygen,
        signature_allowed_signers=signature_allowed_signers,
        signature_revocation=signature_revocation,
        show_output=show_output,
        scaling_evidence_manifest=(
            tmp_path / "scaling" / "scaling_evidence.json"
        ).resolve(),
        scaling_trial_harness_sha256=hashlib.sha256(
            SCALING_TRIAL_HARNESS_DATA
        ).hexdigest(),
        scaling_configuration_sha256=hashlib.sha256(
            SCALING_CONFIGURATION_DATA
        ).hexdigest(),
        scaling_irohad_sha256=SCALING_IROHAD_SHA256,
        scaling_iroha_cli_sha256=SCALING_IROHA_CLI_SHA256,
        sdk_source_manifest_data=_sdk_source_manifest_fixture(
            signature_git, sha256(signature_git),
        ),
    )
    bootstrap_evidence_dir = bootstrap["bootstrap_evidence_dir"]
    assert isinstance(bootstrap_evidence_dir, Path)
    release_invocation_root = tmp_path / "release-invocation"
    release_invocation_root.mkdir(mode=0o700)
    release_invocation_root.chmod(0o700)
    runtime_tool_probes = make_runtime_tool_probe_evidence(
        release_invocation_root, bootstrap
    )
    release_root = release_invocation_root / "source"
    release_root.mkdir()
    (release_root / "Cargo.lock").write_bytes(lock_bytes)
    install_cache_helper(release_root, ROOT_DIR)
    release_target_root = release_invocation_root / "target"
    release_target_root.mkdir(mode=0o700)
    release_target_root.chmod(0o700)
    release_output = release_invocation_root / "output"
    release_output.mkdir(mode=0o700)
    release_output.chmod(0o700)
    release_artifact_root = release_output
    runtime_inventory = release_invocation_root / "runtime-input.json"
    runtime_inventory.write_bytes(canonical_json({"format": "iroha-sumeragi-v2-private-runtime", "schema_version": 1, "runtime_root": str(release_invocation_root / "runtime"), "record_count": 0, "file_bytes": 0, "records": [], "source_disclosure": "withheld", "input_record_count": 0, "input_file_bytes": 0, "input_records": []})); runtime_inventory.chmod(0o400)
    release_output_directory = release_output / "release"
    release_output_directory.mkdir(mode=0o700)
    release_output_directory.chmod(0o700)
    terminal_output = release_output_directory / "RELEASE_COMPLETED.json"
    sdk_dependencies = make_sdk_dependency_evidence(
        release_invocation_root,
        source_manifest_sha256=sha256(
            bootstrap_evidence_dir / "sdk-dependency-bundle-manifest.json"
        ),
    )

    signature_dir = bootstrap_evidence_dir
    signature_attestation = bootstrap["bootstrap_attestation"]
    signature_transcript = bootstrap["bootstrap_transcript"]
    signature_raw_commit = bootstrap["bootstrap_identity_raw_commit"]
    signature_cargo_lock = bootstrap["bootstrap_identity_cargo_lock"]
    signature_allowed_signers = bootstrap["bootstrap_identity_allowed_signers"]
    signature_revocation = bootstrap["bootstrap_identity_revocation"]
    signature_git = bootstrap["bootstrap_identity_git"]
    signature_ssh_keygen = bootstrap["bootstrap_identity_ssh_keygen"]
    assert all(
        isinstance(path, Path)
        for path in (
            signature_attestation,
            signature_transcript,
            signature_raw_commit,
            signature_cargo_lock,
            signature_allowed_signers,
            signature_revocation,
            signature_git,
            signature_ssh_keygen,
        )
    )

    writer_symbols = runpy.run_path(str(SCRIPT))
    corridor_legs = fixture_corridor_legs(writer_symbols, bootstrap)
    production_modules = writer_symbols["_PRODUCTION_MODULES"]
    canonical_production_tests = writer_symbols["_canonical_production_tests"](
        ROOT_DIR
    )
    canonical_g_unit_rows = writer_symbols["_canonical_g_unit_rows"](ROOT_DIR)
    data_status_test = writer_symbols["_DATA_STATUS_TEST"]
    data_lane_certificate_test = writer_symbols["_DATA_LANE_CERTIFICATE_TEST"]
    taira_contract_tests = writer_symbols["_TAIRA_CONTRACT_TESTS"]
    cross_sdk_tests = writer_symbols["_CROSS_SDK_TESTS"]
    rust_sdk_diagnostics_tests = writer_symbols["_RUST_SDK_DIAGNOSTICS_TESTS"]
    native_amx_grouped_fixture = writer_symbols["_NATIVE_AMX_GROUPED_FIXTURE"]
    native_amx_grouped_negative_control_count = writer_symbols[
        "_NATIVE_AMX_GROUPED_NEGATIVE_CONTROL_COUNT"
    ]
    for removed_symbol in (
        "_NATIVE_AMX_GROUPED_SUITE_SOURCE_PATHS",
        "_SUMERAGI_SDK_DIAGNOSTICS_SUITE_SOURCE_PATHS",
        "_native_amx_grouped_suite_source_manifest",
        "_sumeragi_sdk_diagnostics_suite_source_manifest",
    ):
        assert removed_symbol not in writer_symbols
    harness_text = (
        ROOT_DIR / writer_symbols["_NATIVE_AMX_GROUPED_PARITY_HARNESS"]
    ).read_text(encoding="utf-8")
    assert (
        'javascript_repository_root_first="${temporary_root}/javascript-repository-first"'
        in harness_text
    )
    assert (
        'javascript_repository_root_second="${temporary_root}/javascript-repository-second"'
        in harness_text
    )
    assert (
        'javascript_package_root_first="${javascript_repository_root_first}/javascript/iroha_js"'
        in harness_text
    )
    assert (
        'javascript_staged_scripts_root="${javascript_package_root}/scripts"'
        in harness_text
    )
    assert (
        'for javascript_package_root in \\\n'
        '      "$javascript_package_root_first" "$javascript_package_root_second"; do'
        in harness_text
    )
    assert re.search(
        r'cp "\$\{javascript_sdk_root\}/scripts/native-build-provenance\.mjs"'
        r'(?:\s*\\)?\s+'
        r'"\$\{javascript_staged_scripts_root\}/native-build-provenance\.mjs"',
        harness_text,
    )
    retained_fixture = release_root / native_amx_grouped_fixture
    retained_fixture.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ROOT_DIR / native_amx_grouped_fixture, retained_fixture)
    _install_sdk_source_closure_fixture(release_root, writer_symbols)
    sdk_source_manifest = writer_symbols["_sdk_suite_source_manifest"]
    assert callable(sdk_source_manifest)
    native_amx_grouped_suite_source_manifest = sdk_source_manifest(
        release_root,
        writer_symbols["_NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE"],
    )
    sdk_diagnostics_suite_source_manifest = sdk_source_manifest(
        release_root,
        writer_symbols[
            "_SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE"
        ],
    )
    native_amx_grouped_fixture_sha256 = sha256(
        retained_fixture
    )
    corridor_dir = tmp_path / "corridor"
    corridor_logs_dir = corridor_dir / "logs"
    corridor_logs_dir.mkdir(parents=True)
    required_by_module: dict[str, list[str]] = {}
    for _, module, count in production_modules:
        tests = [
            test
            for test in canonical_production_tests
            if test.startswith(f"{module}::")
        ]
        assert len(tests) == count
        required_by_module[module] = tests
    required_lines = ["module\ttest"]
    for test in canonical_production_tests:
        modules = [
            module
            for _, module, _ in production_modules
            if test.startswith(f"{module}::")
        ]
        assert len(modules) == 1
        required_lines.append(f"{modules[0]}\t{test}")
    corridor_required = corridor_dir / "production-required-tests.tsv"
    corridor_required.write_text("\n".join(required_lines) + "\n", encoding="utf-8")
    corridor_g_unit = corridor_dir / "g-unit-required-tests.tsv"
    corridor_g_unit.write_text(
        "\n".join(
            [
                "leg_id\tcrate\ttest",
                *(
                    f"{leg_id}\t{package}\t{test}"
                    for leg_id, package, test in canonical_g_unit_rows
                ),
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    corridor_summary_lines = [
        "leg_index\tleg_id\tkind\trequired_test_count\tobserved_test_count\t"
        "command_status\ttee_status\tlog_sha256\tlog\tcommand"
    ]
    corridor_logs = []
    module_by_leg = {
        leg_id: module for leg_id, module, _ in production_modules
    }
    g_unit_by_leg: dict[str, list[str]] = {}
    for leg_id, _, test in canonical_g_unit_rows:
        g_unit_by_leg.setdefault(leg_id, []).append(test)
    for index, (leg_id, kind, required_count, command) in enumerate(corridor_legs):
        log = corridor_logs_dir / f"{index:02d}-{leg_id}.log"
        if kind == "cargo-focus":
            test_lines = g_unit_by_leg[leg_id]
            assert len(test_lines) == required_count
            log_lines = [
                line
                for test in test_lines
                for line in (
                    "running 1 test",
                    f"test {test} ... ok",
                    "",
                    "test result: ok. 1 passed; 0 failed; 0 ignored; "
                    "0 measured; 42 filtered out; finished in 0.01s",
                )
            ]
        elif kind.startswith("cargo-"):
            test_lines = []
            if kind == "cargo-module":
                test_lines = [
                    f"test {test} ... ok"
                    for test in required_by_module[module_by_leg[leg_id]]
                ]
            elif leg_id in module_by_leg:
                test_lines = [
                    f"test {test} ... ok"
                    for test in required_by_module[module_by_leg[leg_id]]
                ]
            elif leg_id == "status-rust":
                test_lines = [f"test {data_status_test} ... ok"]
            elif leg_id == "lane-certificate-rust":
                test_lines = [f"test {data_lane_certificate_test} ... ok"]
            elif leg_id == "cross-sdk-rust":
                test_lines = [f"test {test} ... ok" for test in cross_sdk_tests]
            elif leg_id == "sumeragi-diagnostics-rust":
                test_lines = [
                    f"test {test} ... ok" for test in rust_sdk_diagnostics_tests
                ]
            elif leg_id.startswith("taira-contract-"):
                contract_index = int(leg_id.rsplit("-", 1)[1])
                test_lines = [
                    f"test {taira_contract_tests[contract_index]} ... ok"
                ]
            log_lines = [f"running {required_count} tests", *test_lines, ""]
            log_lines.append(
                f"test result: ok. {required_count} passed; 0 failed; 0 ignored; "
                "0 measured; 42 filtered out; finished in 0.01s"
            )
        elif kind == "pytest":
            log_lines = ["." * required_count, f"{required_count} passed in 0.01s"]
        elif kind == "command":
            log_lines = [f"{leg_id} completed successfully"]
        elif kind == "native-amx-sdk":
            surface = leg_id.removeprefix("native-amx-grouped-")
            log_lines = []
            if surface == "openapi":
                log_lines.append(
                    "openapi-two-mirror-replay status=success "
                    f"candidate_oid={head} candidate_tree={tree} "
                    "mirrors=2 artifacts=5 require_signed=1"
                )
            log_lines.append(
                "native-amx-v2-grouped-parity "
                f"surface={surface} tests={required_count} "
                f"fixture_sha256={native_amx_grouped_fixture_sha256} "
                "suite_source_manifest_sha256="
                f"{native_amx_grouped_suite_source_manifest}"
            )
        elif kind == "sdk-diagnostics":
            surface = leg_id.removeprefix("sumeragi-diagnostics-")
            log_lines = [
                "sumeragi-v2-sdk-diagnostics "
                f"surface={surface} tests={required_count} "
                "suite_source_manifest_sha256="
                f"{sdk_diagnostics_suite_source_manifest}"
            ]
        else:
            raise AssertionError(f"unsupported corridor leg kind {kind}")
        log.write_text("\n".join(log_lines) + "\n", encoding="utf-8")
        corridor_logs.append(log)
        corridor_summary_lines.append(
            "\t".join(
                (
                    str(index),
                    leg_id,
                    kind,
                    str(required_count),
                    str(required_count),
                    "0",
                    "0",
                    sha256(log),
                    f"logs/{log.name}",
                    command,
                )
            )
        )
    corridor_summary = corridor_dir / "summary.tsv"
    corridor_summary.write_text(
        "\n".join(corridor_summary_lines) + "\n", encoding="utf-8"
    )
    tool_dir = tmp_path / "tools"
    tool_dir.mkdir()
    tool_paths = {}
    for name in (
        "java",
        "cargo",
        "rustc",
        "python3",
        "node",
        "swift",
        "bash",
        "git",
        "tlapm",
        "tla2tools",
        "verus",
        "cargo_verus",
    ):
        if name in {"cargo", "rustc"}:
            bootstrap_key = f"bootstrap_runner_{name}"
            bootstrap_tool = bootstrap[bootstrap_key]
            assert isinstance(bootstrap_tool, Path)
            path = bootstrap_tool
        else:
            path = tool_dir / name
            path.write_text(f"fixture {name}\n", encoding="utf-8")
        tool_paths[name] = path
    prebuilt = make_prebuilt_binary_bundle(
        release_artifact_root,
        sealed_manifest=sealed_manifest,
        lock=lock,
    )
    prebuilt_manifest = prebuilt["prebuilt_manifest"]
    prebuilt_manifest_sha256 = prebuilt["prebuilt_manifest_sha256"]
    assert isinstance(prebuilt_manifest, Path)
    assert isinstance(prebuilt_manifest_sha256, str)
    corridor_completion = corridor_dir / "COMPLETED.tsv"
    isolated_cargo_home, cargo_cache_input_inventory, cargo_cache_final_inventory, caller_cargo_home, cargo_runtime_fields = fixture_cargo_cache_input(
            ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh",
            tool_dir / "caller-cargo-home",
            release_artifact_root,
        )
    write_tsv(
        corridor_completion,
        {
            "schema_version": "1",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "artifact_root_path": str(release_artifact_root),
            "cargo_target_root_path": str(release_target_root),
            "leg_count": str(len(corridor_legs)),
            "production_required_test_count": str(len(required_lines) - 1),
            "g_unit_expected_test_count": str(len(canonical_g_unit_rows)),
            "g_unit_passed_test_count": str(len(canonical_g_unit_rows)),
            "summary_sha256": sha256(corridor_summary),
            "production_required_tests_sha256": sha256(corridor_required),
            "g_unit_inventory_sha256": sha256(corridor_g_unit),
            "java_path": str(tool_paths["java"].resolve()),
            "java_sha256": sha256(tool_paths["java"]),
            "cargo_path": str(tool_paths["cargo"].resolve()),
            "cargo_sha256": sha256(tool_paths["cargo"]),
            "cargo_version": "cargo 1.93.1 (083ac5135 2025-12-15)",
            "rustc_path": str(tool_paths["rustc"].resolve()),
            "rustc_sha256": sha256(tool_paths["rustc"]),
            "rustc_version": "rustc 1.93.1 (01f6ddf75 2026-02-11)",
            "python3_path": str(tool_paths["python3"].resolve()),
            "python3_sha256": sha256(tool_paths["python3"]),
            "node_path": str(tool_paths["node"].resolve()),
            "node_sha256": sha256(tool_paths["node"]),
            "swift_path": str(tool_paths["swift"].resolve()),
            "swift_sha256": sha256(tool_paths["swift"]),
            "swift_version": "Apple Swift version 6.2.3",
            "bash_path": str(tool_paths["bash"].resolve()),
            "bash_sha256": sha256(tool_paths["bash"]),
            "git_path": str(tool_paths["git"].resolve()),
            "git_sha256": sha256(tool_paths["git"]),
            "cargo_home_path": str(isolated_cargo_home.resolve()),
            **cargo_runtime_fields,
            "cargo_cache_input_inventory_path": str(
                cargo_cache_input_inventory.resolve()
            ),
            "cargo_cache_input_inventory_sha256": sha256(
                cargo_cache_input_inventory
            ),
            "cargo_cache_final_inventory_path": str(cargo_cache_final_inventory.resolve()),
            "cargo_cache_final_inventory_sha256": sha256(cargo_cache_final_inventory),
            "runtime_inventory_path": str(runtime_inventory.resolve()),
            "runtime_inventory_sha256": sha256(runtime_inventory),
            "repo_cargo_config_sha256": sha256(ROOT_DIR / ".cargo" / "config.toml"),
            "native_amx_grouped_fixture_sha256": (
                native_amx_grouped_fixture_sha256
            ),
            "native_amx_grouped_suite_source_manifest_sha256": (
                native_amx_grouped_suite_source_manifest
            ),
            "native_amx_grouped_negative_control_count": str(
                native_amx_grouped_negative_control_count
            ),
            "tlc_profile": "ci",
            "tlaps_threads": "1",
            "prebuilt_manifest_path": str(prebuilt_manifest),
            "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        },
    )

    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    formal_log = formal_dir / "formal-gate.log"
    formal_log.write_text(f"formal work\n{FINAL_MARKER}\n", encoding="utf-8")
    formal_ledger = formal_dir / "proof_coverage.json"
    formal_ledger.write_text(
        json.dumps(
            {
                "machine_checked_completion": True,
                "obligations": [
                    {
                        "id": "effective-lock-body-acquisition-production-refinement",
                        "status": "cross_tool_proved",
                    }
                ],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    formal_evidence = formal_dir / "proof_evidence.json"
    formal_evidence.write_text('{"backend_verification":true}\n', encoding="utf-8")
    formal_verus_evidence = formal_dir / "verus_evidence.json"
    formal_verus_evidence.write_text(
        '{"backend_verification":true}\n', encoding="utf-8"
    )
    formal_verus_log = formal_dir / "verus.log"
    formal_verus_log.write_text(
        "fixture production Verus verification passed\n", encoding="utf-8"
    )
    formal_multilane_apalache_evidence = (
        formal_dir / "multilane_apalache_evidence.tsv"
    )
    formal_multilane_apalache_evidence.write_text(
        "\n".join(
            (
                "schema_version\t2",
                "backend\tapalache",
                "version\t0.52.2",
                "launcher_sha256\t"
                "bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7",
                "jar_sha256\t"
                "1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a",
                f"workspace_source_manifest_sha256\t{sealed_manifest}",
                f"multilane_source_manifest_sha256\t{multilane_manifest}",
                "result_count\t6",
                "result\tautoscale-lifecycle\tSumeragiV2AutoscaleLifecycle\t"
                "multilane_autoscale_lifecycle_fixed.cfg\t8\tNoError\t"
                f"{'1' * 64}\t{'2' * 64}\t{'3' * 64}",
                "result\tnative-application-evidence\t"
                "SumeragiV2NativeApplicationEvidence\t"
                "multilane_native_application_evidence_fixed.cfg\t8\tNoError\t"
                f"{'4' * 64}\t{'5' * 64}\t{'6' * 64}",
                "result\tautonomous-reservation-carrier\t"
                "SumeragiV2AutonomousReservationCarrier\t"
                "multilane_autonomous_reservation_carrier_fixed.cfg\t10\tNoError\t"
                f"{'7' * 64}\t{'8' * 64}\t{'9' * 64}",
                "result\tqueue-plan-admission-registry\t"
                "SumeragiV2QueuePlanAdmissionRegistry\t"
                "multilane_queue_plan_admission_registry_fixed.cfg\t8\tNoError\t"
                f"{'a' * 64}\t{'b' * 64}\t{'c' * 64}",
                "result\tkura-replica-retention\t"
                "SumeragiV2KuraReplicaRetention\t"
                "kura_replica_retention_fixed.cfg\t8\tNoError\t"
                f"{'d' * 64}\t{'e' * 64}\t{'f' * 64}",
                "result\tinflight-first-release-layout\t"
                "SumeragiV2InFlightFirstRelease\t"
                "inflight_first_release_fixed.cfg\t18\tNoError\t"
                f"{'0' * 64}\t{'1' * 64}\t{'2' * 64}",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    formal_cross_tool_evidence = formal_dir / "cross_tool_evidence.json"
    formal_cross_tool_evidence.write_text(
        '{"backend_verification":true,"canonical":true}\n', encoding="utf-8"
    )
    # Aggregate-success fixtures exercise the checker-authenticated production
    # trace-extraction theorem interface required by the release receipt.
    formal_production_trace_extraction_evidence = (
        formal_dir / "production_trace_extraction_evidence.json"
    )
    formal_production_trace_extraction_evidence.write_bytes(
        canonical_json(
            {
                "backend_verification": True,
                "canonical": True,
                "multilane_source_manifest_sha256": multilane_manifest,
                "theorem": "sumeragi-v2-production-trace-extraction",
                "workspace_source_manifest_sha256": sealed_manifest,
            }
        )
    )
    formal_harness_lock = formal_dir / "harness-Cargo.lock"
    shutil.copy2(
        ROOT_DIR / "scripts" / "formal" / "sumeragi_v2_harness.lock",
        formal_harness_lock,
    )
    formal_toolchain = formal_dir / "formal-toolchain.tsv"
    formal_toolchain_fields = {"schema_version": "1"}
    for name in ("java", "tlapm", "tla2tools", "verus", "cargo_verus"):
        path = tool_paths[name]
        formal_toolchain_fields[f"{name}_path"] = str(path.resolve())
        formal_toolchain_fields[f"{name}_sha256"] = sha256(path)
    formal_toolchain_fields["tlc_profile"] = "ci"
    formal_toolchain_fields["tlaps_threads"] = "1"
    write_tsv(formal_toolchain, formal_toolchain_fields)
    formal_tlaps_resource_summary = formal_dir / "tlaps_resource_summary.json"
    resource_summary = {
        "child_exit_code": 0,
        "ended_utc": "2026-07-22T00:00:01.000Z",
        "event": "summary",
        "exit_reason": "completed",
        "exit_status": 0,
        "evidence_peak_rss_bytes": 4096,
        "kernel_peak_rss_bytes": 4096,
        "kernel_peak_rss_method": "wait4_ru_maxrss",
        "kernel_peak_rss_scope": "direct_guarded_body",
        "memory_limit_bytes": 2 * 1024 * 1024 * 1024,
        "memory_enforcement_mode": "max_rss_physical_footprint",
        "physical_footprint_interval_seconds": 5.0,
        "peak_memory_bytes": 4096,
        "peak_physical_footprint_bytes": 4096,
        "peak_rss_bytes": 4096,
        "report_context": None,
        "sample_count": 1,
        "sample_interval_seconds": 0.25,
        "schema_version": 1,
        "started_utc": "2026-07-22T00:00:00.000Z",
        "supervisor_pid": 1234,
    }
    formal_tlaps_resource_summary.write_bytes(canonical_json(resource_summary))
    formal_tlaps_resource_jsonl = formal_dir / "tlaps_resource.jsonl"
    formal_tlaps_resource_jsonl.write_bytes(
        canonical_json(
            {
                "event": "start",
                "memory_limit_bytes": 2 * 1024 * 1024 * 1024,
                "memory_enforcement_mode": "max_rss_physical_footprint",
                "physical_footprint_interval_seconds": 5.0,
                "report_context": None,
                "sample_interval_seconds": 0.25,
                "schema_version": 1,
                "started_utc": "2026-07-22T00:00:00.000Z",
                "supervisor_pid": 1234,
            }
        )
        + canonical_json(
            {
                "event": "spawn",
                "process_group_id": 5678,
                "schema_version": 1,
                "timestamp_utc": "2026-07-22T00:00:00.100Z",
                "wrapper_pid": 5677,
            }
        )
        + canonical_json(
            {
                "accounting_method": "max_rss_physical_footprint",
                "elapsed_seconds": 0.25,
                "event": "sample",
                "memory_bytes": 4096,
                "memory_limit_bytes": 2 * 1024 * 1024 * 1024,
                "physical_footprint_bytes": 4096,
                "process_count": 1,
                "process_group_id": 5678,
                "rss_bytes": 4096,
                "schema_version": 1,
                "timestamp_utc": "2026-07-22T00:00:00.250Z",
            }
        )
        + canonical_json(resource_summary)
    )
    formal_completion = formal_dir / "COMPLETED.tsv"
    write_tsv(
        formal_completion,
        {
            "schema_version": "2",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "formal_gate_log_sha256": sha256(formal_log),
            "proof_coverage_sha256": sha256(formal_ledger),
            "proof_evidence_sha256": sha256(formal_evidence),
            "verus_evidence_sha256": sha256(formal_verus_evidence),
            "verus_log_sha256": sha256(formal_verus_log),
            "multilane_apalache_evidence_sha256": sha256(
                formal_multilane_apalache_evidence
            ),
            "cross_tool_evidence_sha256": sha256(formal_cross_tool_evidence),
            "production_trace_extraction_evidence_sha256": sha256(
                formal_production_trace_extraction_evidence
            ),
            "harness_cargo_lock_sha256": sha256(formal_harness_lock),
            "formal_toolchain_sha256": sha256(formal_toolchain),
            "tlaps_resource_jsonl_sha256": sha256(formal_tlaps_resource_jsonl),
            "tlaps_resource_summary_sha256": sha256(formal_tlaps_resource_summary),
        },
    )

    seed_dir = tmp_path / "seed"
    seed_program_target = prebuilt["prebuilt_bundle"]
    assert isinstance(seed_program_target, Path)
    runs_dir = seed_dir / "runs"
    localnets_dir = seed_dir / "localnets"
    localnet_manifests_dir = seed_dir / "localnet-manifests"
    runs_dir.mkdir(parents=True)
    localnets_dir.mkdir()
    localnet_manifests_dir.mkdir()
    seed_logs = []
    seed_localnets = []
    seed_localnet_files = []
    seed_localnet_manifests = []
    localnet_manifest_index_lines = [
        "run_index\tlocalnet\tmanifest\tmanifest_sha256"
    ]
    summary_lines = ["\t".join(SUMMARY_FIELDS)]
    seed_run_count = len(SCENARIOS) * 32
    for index in range(seed_run_count):
        scenario = SCENARIOS[index // 32]
        seed_index = index % 32
        seed = scenario if seed_index == 0 else f"{scenario}:seed:{seed_index:02d}"
        output = f"runs/run-{index:03d}.log"
        run_log = seed_dir / output
        run_log.write_text(
            "\n".join(
                (
                    "running 1 test",
                    f"test sumeragi_v2_runner::{scenario} ... "
                    f"{scenario}: deterministic network seed = {seed}",
                    "ok",
                    "",
                    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                    "42 filtered out; finished in 0.01s",
                )
            )
            + "\n",
            encoding="utf-8",
        )
        seed_logs.append(run_log)
        localnet = localnets_dir / f"run-{index:03d}"
        validator = localnet / "mock-validator"
        validator.mkdir(parents=True)
        retained_log = validator / "run-1-stdout.log"
        retained_log.write_text(
            f"sumeragi_v2_runner::{scenario}\nseed={seed}\n", encoding="utf-8"
        )
        seed_localnets.append(localnet)
        seed_localnet_files.append(retained_log)
        localnet_manifest = localnet_manifests_dir / f"run-{index:03d}.tsv"
        relative_file = "mock-validator/run-1-stdout.log"
        localnet_manifest.write_text(
            "path\tsize_bytes\tsha256\n"
            f"{relative_file}\t{retained_log.stat().st_size}\t{sha256(retained_log)}\n",
            encoding="utf-8",
        )
        seed_localnet_manifests.append(localnet_manifest)
        localnet_manifest_index_lines.append(
            "\t".join(
                (
                    str(index),
                    f"localnets/run-{index:03d}",
                    f"localnet-manifests/run-{index:03d}.tsv",
                    sha256(localnet_manifest),
                )
            )
        )
        summary_lines.append(
            "\t".join(
                (
                    "release",
                    sealed_manifest,
                    scenario,
                    seed,
                    "passed",
                    "0",
                    "0",
                    sha256(run_log),
                    output,
                    f"localnets/run-{index:03d}",
                    f"CARGO_TARGET_DIR={release_target_root} "
                    f"IROHA_TEST_TARGET_DIR={seed_program_target} "
                    f"IROHA_RELEASE_SOURCE_MANIFEST_SHA256={sealed_manifest} "
                    "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256="
                    f"{prebuilt_manifest_sha256} "
                    "TEST_NETWORK_BIN_IROHAD="
                    f"{seed_program_target / 'release' / 'iroha3d'} "
                    "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="
                    f"{seed_program_target / 'message-control' / 'release' / 'iroha3d'} "
                    f"TEST_NETWORK_BIN_IROHA={seed_program_target / 'release' / 'iroha'} "
                    f"KAGAMI_BIN={seed_program_target / 'release' / 'kagami'} "
                    "CARGO_NET_OFFLINE=true "
                    "IROHA_TEST_REQUIRE_NETWORK=1 "
                    "IROHA_TEST_NETWORK_START_ATTEMPTS=1 "
                    "IROHA_TEST_SKIP_BUILD=1 "
                    "IROHA_TEST_ALLOW_REENTRANT_BUILD=0 "
                    "IROHA_TEST_BUILD_PROFILE=release "
                    "PROFILE=release "
                    "IROHA_TEST_BUILD_TIMEOUT_MS=3600 "
                    "IROHA_TEST_PROCESS_TIMEOUT_MS=300 "
                    "IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300 "
                    f"IROHA_TEST_NETWORK_BASE_SEED={seed} "
                    "TEST_NETWORK_TMP_DIR=${SEED_MATRIX_EVIDENCE_DIRECTORY}/"
                    f"localnets/run-{index:03d} "
                    "IROHA_TEST_NETWORK_KEEP_DIRS=1 "
                    "cargo test --locked --offline -p integration_tests --test "
                    "sumeragi_v2_runner_isolated "
                    f"sumeragi_v2_runner::{scenario} -- --exact --nocapture "
                    "--test-threads=1",
                )
            )
        )
    seed_summary = seed_dir / "summary.tsv"
    seed_summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    seed_localnet_manifest_index = seed_dir / "localnet-manifests.tsv"
    seed_localnet_manifest_index.write_text(
        "\n".join(localnet_manifest_index_lines) + "\n", encoding="utf-8"
    )
    seed_completion = seed_dir / "COMPLETED.tsv"
    seed_completion_fields = {
        "schema_version": "2",
        "profile": "release",
        "head_commit": head,
        "head_tree": tree,
        "source_manifest_sha256": sealed_manifest,
        "cargo_lock_sha256": lock,
        "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        "completed_runs": str(seed_run_count),
        "expected_runs": str(seed_run_count),
        "summary_sha256": sha256(seed_summary),
        "localnet_manifest_count": str(seed_run_count),
        "localnet_manifests_path": "localnet-manifests.tsv",
        "localnet_manifests_sha256": sha256(seed_localnet_manifest_index),
    }
    for index, manifest in enumerate(seed_localnet_manifests):
        seed_completion_fields[f"localnet_manifest_{index:03d}_path"] = (
            f"localnet-manifests/run-{index:03d}.tsv"
        )
        seed_completion_fields[f"localnet_manifest_{index:03d}_sha256"] = sha256(
            manifest
        )
    write_tsv(
        seed_completion,
        seed_completion_fields,
    )

    chaos_dir = tmp_path / "chaos"
    chaos_dir.mkdir()
    chaos_log = chaos_dir / "chaos-100k.log"
    chaos_log.write_text(
        "\n".join(
            (
                "running 1 test",
                CHAOS_MARKER,
                "test accelerated_100_000_block_chaos_preserves_chain_prefix ... ok",
                "",
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                "9 filtered out; finished in 0.01s",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    chaos_completion = chaos_dir / "COMPLETED.tsv"
    write_tsv(
        chaos_completion,
        {
            **CHAOS_FIELDS,
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "log_sha256": sha256(chaos_log),
        },
    )

    taira_dir = tmp_path / "taira"
    taira_dir.mkdir()
    taira_evidence = taira_dir / "taira_v2_24h_soak.json"
    taira_evidence.write_text('{"status":"passed"}\n', encoding="utf-8")
    taira_log = taira_dir / "taira-v2-24h.log"
    taira_log.write_text(
        "\n".join(
            (
                "running 1 test",
                "test taira_public_localnet::"
                "taira_profile_24h_packet_impairment_and_restart_soak ... ok",
                "",
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                "42 filtered out; finished in 86400.01s",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    taira_completion = taira_dir / "COMPLETED.tsv"
    write_tsv(
        taira_completion,
        {
            "schema_version": "1",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
            "evidence_sha256": sha256(taira_evidence),
            "log_sha256": sha256(taira_log),
        },
    )
    scaling = make_scaling_evidence(
        tmp_path,
        head=head,
        sealed_manifest=sealed_manifest,
    )
    g12 = make_g12_evidence(
        tmp_path,
        head=head,
        tree=tree,
        sealed_manifest=sealed_manifest,
        lock=lock,
        prebuilt_manifest_sha256=prebuilt_manifest_sha256,
    )
    g4p = make_g4p_evidence(
        tmp_path,
        head=head,
        tree=tree,
        sealed_manifest=sealed_manifest,
        lock=lock,
        prebuilt_manifest_sha256=prebuilt_manifest_sha256,
    )
    return {
        **bootstrap,
        **scaling,
        **prebuilt,
        **g4p,
        **g12,
        **sdk_dependencies,
        **runtime_tool_probes,
        "candidate": candidate,
        "sealed": sealed,
        "release_root": release_root,
        "release_artifact_root": release_artifact_root,
        "release_target_root": release_target_root,
        "terminal_output": terminal_output,
        "signature_attestation": signature_attestation,
        "signature_transcript": signature_transcript,
        "signature_raw_commit": signature_raw_commit,
        "signature_cargo_lock": signature_cargo_lock,
        "signature_allowed_signers": signature_allowed_signers,
        "signature_revocation": signature_revocation,
        "signature_git": signature_git,
        "signature_ssh_keygen": signature_ssh_keygen,
        "signature_dir": signature_dir,
        "expected_git_sha256": protected_git,
        "expected_ssh_keygen_sha256": protected_ssh,
        "expected_allowed_signers_sha256": protected_allowed,
        "expected_revocation_sha256": protected_revocation,
        "expected_signer_fingerprint": signer_fingerprint,
        "signer_principal": signer_principal,
        "corridor_completion": corridor_completion,
        "corridor_summary": corridor_summary,
        "corridor_required": corridor_required,
        "corridor_g_unit": corridor_g_unit,
        "corridor_logs": corridor_logs,
        "corridor_log": corridor_logs[0],
        "formal_completion": formal_completion,
        "formal_log": formal_log,
        "formal_ledger": formal_ledger,
        "formal_evidence": formal_evidence,
        "formal_verus_evidence": formal_verus_evidence,
        "formal_verus_log": formal_verus_log,
        "formal_multilane_apalache_evidence": formal_multilane_apalache_evidence,
        "formal_cross_tool_evidence": formal_cross_tool_evidence,
        "formal_production_trace_extraction_evidence": (
            formal_production_trace_extraction_evidence
        ),
        "formal_harness_lock": formal_harness_lock,
        "formal_toolchain": formal_toolchain,
        "formal_tlaps_resource_jsonl": formal_tlaps_resource_jsonl,
        "formal_tlaps_resource_summary": formal_tlaps_resource_summary,
        "formal_verus_tool": tool_paths["verus"],
        "corridor_cargo_tool": tool_paths["cargo"],
        "corridor_rustc_tool": tool_paths["rustc"],
        "corridor_cargo_home": isolated_cargo_home,
        "cargo_cache_input_inventory": cargo_cache_input_inventory, "cargo_cache_final_inventory": cargo_cache_final_inventory,
        "caller_cargo_home": caller_cargo_home,
        "seed_completion": seed_completion,
        "seed_summary": seed_summary,
        "seed_logs": seed_logs,
        "seed_log": seed_logs[17],
        "seed_localnets": seed_localnets,
        "seed_localnet": seed_localnets[17],
        "seed_localnet_file": seed_localnet_files[17],
        "seed_localnet_manifest_index": seed_localnet_manifest_index,
        "seed_localnet_manifests": seed_localnet_manifests,
        "seed_localnet_manifest": seed_localnet_manifests[17],
        "chaos_completion": chaos_completion,
        "chaos_log": chaos_log,
        "taira_completion": taira_completion,
        "taira_evidence": taira_evidence,
        "taira_log": taira_log,
        "candidate_manifest": candidate_manifest,
        "sealed_manifest": sealed_manifest,
        "multilane_manifest": multilane_manifest,
        "head": head,
        "tree": tree,
        "lock": lock,
    }


def run_writer(
    evidence: dict[str, object],
    output: Path,
    writer: Path,
    *,
    use_supplied_output: bool = False,
    verify_existing: bool = False,
    replay_existing: bool = False,
) -> subprocess.CompletedProcess[str]:
    assert not (verify_existing and replay_existing)
    if not use_supplied_output:
        output = terminal_output_path(evidence)
    if not output.parent.exists():
        output.parent.mkdir(parents=True, mode=0o700)
        output.parent.chmod(0o700)
    repository_root = evidence["release_root"]
    assert isinstance(repository_root, Path)
    source_root = writer.parent.parent
    (repository_root / "scripts" / "formal").mkdir(parents=True, exist_ok=True)
    for relative in (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/formal/check_sumeragi_v2_proof_ledger.py"),
        *proof_ledger_checker_components(source_root),
        Path("scripts/formal/sumeragi_v2_verus_evidence.py"),
        Path("scripts/nexus/validate_multilane_scaling_evidence.py"),
        Path("scripts/deploy_localnet.sh"),
        Path("scripts/tx_load.py"),
        Path("scripts/nexus_lane_load_test.py"),
        Path("scripts/check_taira_v2_soak_evidence.py"),
        Path(".cargo/config.toml"),
    ):
        destination = repository_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        if not (verify_existing or replay_existing) or not destination.exists():
            shutil.copy2(source_root / relative, destination)
    arguments = [
            sys.executable,
            "-I",
            "-S",
            str(writer),
            "--candidate-identity",
            str(evidence["candidate"]),
            "--sealed-identity",
            str(evidence["sealed"]),
            "--release-root",
            str(evidence["release_root"]),
            "--bootstrap-completion",
            str(evidence["bootstrap_completion"]),
            "--bootstrap-evidence-dir",
            str(evidence["bootstrap_evidence_dir"]),
            "--bootstrap-identity",
            str(evidence["bootstrap_identity"]),
            "--bootstrap-attestation",
            str(evidence["bootstrap_attestation"]),
            "--bootstrap-transcript",
            str(evidence["bootstrap_transcript"]),
            "--expected-bootstrap-completion-sha256",
            str(evidence["expected_bootstrap_completion_sha256"]),
            "--bootstrap-candidate-root",
            str(evidence["bootstrap_candidate_root"]),
            "--bootstrap-runner",
            str(evidence["bootstrap_runner"]),
            "--signature-attestation",
            str(evidence["signature_attestation"]),
            "--signature-transcript",
            str(evidence["signature_transcript"]),
            "--signature-raw-commit",
            str(evidence["signature_raw_commit"]),
            "--signature-cargo-lock",
            str(evidence["signature_cargo_lock"]),
            "--signature-allowed-signers",
            str(evidence["signature_allowed_signers"]),
            "--signature-revocation",
            str(evidence["signature_revocation"]),
            "--signature-git",
            str(evidence["signature_git"]),
            "--signature-ssh-keygen",
            str(evidence["signature_ssh_keygen"]),
            "--expected-git-sha256",
            str(evidence["expected_git_sha256"]),
            "--expected-ssh-keygen-sha256",
            str(evidence["expected_ssh_keygen_sha256"]),
            "--expected-allowed-signers-sha256",
            str(evidence["expected_allowed_signers_sha256"]),
            "--expected-revocation-sha256",
            str(evidence["expected_revocation_sha256"]),
            "--expected-signer-fingerprint",
            str(evidence["expected_signer_fingerprint"]),
            "--corridor-completion",
            str(evidence["corridor_completion"]),
            "--formal-completion",
            str(evidence["formal_completion"]),
            "--seed-completion",
            str(evidence["seed_completion"]),
            "--chaos-completion",
            str(evidence["chaos_completion"]),
            "--taira-completion",
            str(evidence["taira_completion"]),
            "--g4p-completion",
            str(evidence["g4p_completion"]),
            "--g12-seed-completion",
            str(evidence["g12_seed_completion"]),
            "--g12-fault-soak-completion",
            str(evidence["g12_soak_completion"]),
            "--scaling-evidence-manifest",
            str(evidence["scaling_manifest"]),
            "--sdk-dependency-archive",
            str(evidence["sdk_dependency_archive"]),
            "--sdk-dependency-input-inventory",
            str(evidence["sdk_dependency_input_inventory"]),
            "--sdk-dependency-final-work-inventory",
            str(evidence["sdk_dependency_final_work_inventory"]),
            "--runtime-tool-probe-manifest",
            str(evidence["runtime_tool_probe_manifest"]),
            "--runtime-tool-probe-result",
            str(evidence["runtime_tool_probe_result"]),
            "--expected-scaling-trial-harness-sha256",
            str(evidence["expected_scaling_trial_harness_sha256"]),
            "--expected-scaling-configuration-sha256",
            str(evidence["expected_scaling_configuration_sha256"]),
            "--expected-scaling-irohad-sha256",
            str(evidence["expected_scaling_irohad_sha256"]),
            "--expected-scaling-iroha-cli-sha256",
            str(evidence["expected_scaling_iroha_cli_sha256"]),
            "--repository-root",
            str(repository_root),
            "--output",
            str(output),
        ]
    if verify_existing:
        arguments.extend(("--verify-existing", "--validation-ack",
            str(repository_root.parent / "receipt-validation-ack.json"),
            "--source-manifest-sha256", str(evidence["sealed_manifest"])))
    elif replay_existing:
        arguments.append("--replay-existing")
    runner_environment = evidence["bootstrap_runner_environment"]
    assert isinstance(runner_environment, dict)
    execution_environment = dict(runner_environment)
    marker_digest = str(evidence["expected_bootstrap_completion_sha256"])
    execution_environment.update(
        {
            "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": marker_digest,
            "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": marker_digest,
        }
    )
    return subprocess.run(
        arguments,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=execution_environment,
    )


def rewrite_json(path: Path, value: dict[str, object]) -> None:
    path.chmod(0o600)
    path.write_bytes(canonical_json(value))
    path.chmod(0o400)


def rewrite_tlaps_resource_evidence(
    evidence: dict[str, object],
    records: list[dict[str, object]],
    summary: dict[str, object],
) -> None:
    jsonl = evidence["formal_tlaps_resource_jsonl"]
    summary_path = evidence["formal_tlaps_resource_summary"]
    completion = evidence["formal_completion"]
    assert isinstance(jsonl, Path)
    assert isinstance(summary_path, Path)
    assert isinstance(completion, Path)
    jsonl.chmod(0o600)
    jsonl.write_bytes(b"".join(canonical_json(record) for record in records))
    jsonl.chmod(0o400)
    rewrite_json(summary_path, summary)
    fields = read_tsv_fields(completion)
    fields["tlaps_resource_jsonl_sha256"] = sha256(jsonl)
    fields["tlaps_resource_summary_sha256"] = sha256(summary_path)
    write_tsv(completion, fields)


def read_tsv_fields(path: Path) -> dict[str, str]:
    return dict(
        line.split("\t", 1)
        for line in path.read_text(encoding="utf-8").splitlines()
    )


def rewrite_prebuilt_manifest(
    evidence: dict[str, Path | str | list[Path]],
    data: bytes,
    *,
    update_corridor_digest: bool = True,
) -> None:
    manifest = evidence["prebuilt_manifest"]
    bundle = evidence["prebuilt_bundle"]
    corridor = evidence["corridor_completion"]
    assert isinstance(manifest, Path)
    assert isinstance(bundle, Path)
    assert isinstance(corridor, Path)
    bundle.chmod(0o700)
    manifest.chmod(0o600)
    manifest.write_bytes(data)
    manifest.chmod(0o400)
    bundle.chmod(0o500)
    if update_corridor_digest:
        fields = read_tsv_fields(corridor)
        fields["prebuilt_manifest_sha256"] = sha256(manifest)
        write_tsv(corridor, fields)


def load_writer_module() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_receipt_writer", SCRIPT
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def mutate_bootstrap_marker(
    evidence: dict[str, Path | str | list[Path]], mutator: object
) -> None:
    marker = evidence["bootstrap_completion"]
    assert isinstance(marker, Path)
    value = json.loads(marker.read_text(encoding="utf-8"))
    assert callable(mutator)
    mutator(value)
    rewrite_json(marker, value)
    evidence["expected_bootstrap_completion_sha256"] = sha256(marker)


def rebind_bootstrap_runner_tool(
    evidence: dict[str, Path | str | list[Path]], name: str
) -> None:
    tool = evidence[f"bootstrap_runner_{name}"]
    evidence_dir = evidence["bootstrap_evidence_dir"]
    assert isinstance(tool, Path)
    assert isinstance(evidence_dir, Path)
    manifest_archive = evidence_dir / "runner-tool-manifest.json"
    manifest_source = manifest_archive
    manifest_value = json.loads(manifest_source.read_text(encoding="utf-8"))
    manifest_value["tools"][name]["sha256"] = sha256(tool)
    manifest_source.chmod(0o600)
    manifest_source.write_bytes(canonical_json(manifest_value))
    manifest_source.chmod(0o400)

    probe_manifest = evidence["bootstrap_runner_tool_probe_manifest"]
    probe_result = evidence["bootstrap_runner_tool_probe_result"]
    runtime_manifest = evidence["runtime_tool_probe_manifest"]
    runtime_result = evidence["runtime_tool_probe_result"]
    runtime_tools = evidence["runtime_tool_probe_tools"]
    assert isinstance(probe_manifest, Path)
    assert isinstance(probe_result, Path)
    assert isinstance(runtime_manifest, Path)
    assert isinstance(runtime_result, Path)
    assert isinstance(runtime_tools, dict)
    runtime_tool = runtime_tools[name]
    assert isinstance(runtime_tool, Path)
    runtime_tool.chmod(0o700)
    runtime_tool.write_bytes(tool.read_bytes())
    runtime_tool.chmod(0o500)

    probe_manifest_value = json.loads(probe_manifest.read_text(encoding="ascii"))
    probe_manifest_value["tools"][name]["sha256"] = sha256(tool)
    rewrite_json(probe_manifest, probe_manifest_value)
    probe_value = fixture_tool_probe_result(
        probe_manifest_value,
        archive_id_prefix="release-runner-tool",
    )
    rewrite_json(probe_result, probe_value)

    runtime_manifest_value = json.loads(
        runtime_manifest.read_text(encoding="ascii")
    )
    runtime_manifest_value["tools"][name]["sha256"] = sha256(runtime_tool)
    rewrite_json(runtime_manifest, runtime_manifest_value)
    runtime_value = fixture_tool_probe_result(
        runtime_manifest_value,
        archive_id_prefix="release-runtime-tool",
    )
    rewrite_json(runtime_result, runtime_value)

    current_marker = json.loads(
        Path(evidence["bootstrap_completion"]).read_text(encoding="utf-8")
    )
    approval_module = _load_approval_component(
        evidence_dir / "release-approval-contract.py"
    )
    approval_archives, release_approvals = _bootstrap_approval_fixture(
        module=approval_module,
        identity=current_marker["candidate_identity"],
        tool_manifest_sha256=sha256(manifest_source),
        trust_dir=evidence_dir.parent / "bootstrap-trust",
        evidence_dir=evidence_dir,
    )

    def mutate(value: dict[str, object]) -> None:
        runner = value["runner"]
        trusted_inputs = value["trusted_inputs"]
        assert isinstance(runner, dict)
        assert isinstance(trusted_inputs, dict)
        tools = runner["tools"]
        manifest_record = trusted_inputs["runner_tool_manifest"]
        assert isinstance(tools, dict)
        assert isinstance(manifest_record, dict)
        tool_record = tools[name]
        assert isinstance(tool_record, dict)
        tool_record["sha256"] = sha256(tool)
        tool_record["size_bytes"] = tool.stat().st_size
        manifest_record["sha256"] = sha256(manifest_source)
        manifest_record["size_bytes"] = manifest_source.stat().st_size
        value["release_approvals"] = release_approvals
        for label, archive in approval_archives.items():
            approval_record = trusted_inputs[label]
            assert isinstance(approval_record, dict)
            approval_record["sha256"] = sha256(archive)
            approval_record["size_bytes"] = archive.stat().st_size
        probes = value["trusted_execution_probes"]
        assert isinstance(probes, dict)
        closure = probes["runner_tool_closure"]
        assert isinstance(closure, dict)
        probe_manifest_record = closure["manifest"]
        probe_result_record = closure["result"]
        assert isinstance(probe_manifest_record, dict)
        assert isinstance(probe_result_record, dict)
        probe_manifest_record["sha256"] = sha256(probe_manifest)
        probe_manifest_record["size_bytes"] = probe_manifest.stat().st_size
        probe_result_record["sha256"] = sha256(probe_result)
        probe_result_record["size_bytes"] = probe_result.stat().st_size
        closure["value"] = probe_value

    mutate_bootstrap_marker(evidence, mutate)


def mutate_attestation(
    evidence: dict[str, Path | str | list[Path]], mutator: object
) -> None:
    path = evidence["signature_attestation"]
    assert isinstance(path, Path)
    value = json.loads(path.read_text(encoding="utf-8"))
    assert callable(mutator)
    mutator(value)
    rewrite_json(path, value)


def mutate_transcript(
    evidence: dict[str, Path | str | list[Path]], mutator: object
) -> None:
    path = evidence["signature_transcript"]
    assert isinstance(path, Path)
    value = json.loads(path.read_text(encoding="utf-8"))
    assert callable(mutator)
    mutator(value)
    rewrite_json(path, value)

    attestation_path = evidence["signature_attestation"]
    assert isinstance(attestation_path, Path)
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    attestation["archives"]["verify_transcript"] = sanitized_identity_artifact(
        path, 0o400, "verify_transcript"
    )
    rewrite_json(attestation_path, attestation)


def rebind_git_archive(
    evidence: dict[str, Path | str | list[Path]], new_source: str
) -> None:
    git_path = evidence["signature_git"]
    assert isinstance(git_path, Path)
    git_path.chmod(0o700)
    git_path.write_text(new_source, encoding="utf-8")
    git_path.chmod(0o500)
    digest = sha256(git_path)
    evidence["expected_git_sha256"] = digest

    attestation_path = evidence["signature_attestation"]
    transcript_path = evidence["signature_transcript"]
    assert isinstance(attestation_path, Path)
    assert isinstance(transcript_path, Path)
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    attestation["archives"]["git"] = sanitized_identity_artifact(
        git_path, 0o500, "git"
    )
    rewrite_json(attestation_path, attestation)


def set_nested(value: dict[str, object], path: tuple[str, ...], replacement: object) -> None:
    current: object = value
    for key in path[:-1]:
        assert isinstance(current, dict)
        current = current[key]
    assert isinstance(current, dict)
    current[path[-1]] = replacement


def set_command_stream(
    record: dict[str, object], stream: str, data: bytes
) -> None:
    record[f"{stream}_base64"] = base64.b64encode(data).decode("ascii")
    record[f"{stream}_sha256"] = hashlib.sha256(data).hexdigest()
    record[f"{stream}_size_bytes"] = len(data)


def write_pretty_json(path: Path, value: object) -> None:
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def regenerate_scaling_report(
    evidence: dict[str, Path | str | list[Path]],
) -> None:
    manifest = evidence["scaling_manifest"]
    report = evidence["scaling_report"]
    assert isinstance(manifest, Path)
    assert isinstance(report, Path)
    report.unlink(missing_ok=True)
    result = subprocess.run(
        [
            sys.executable,
            str(
                ROOT_DIR
                / "scripts"
                / "nexus"
                / "validate_multilane_scaling_evidence.py"
            ),
            str(manifest),
            "--report",
            str(report),
            "--quiet",
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert result.returncode == 0, result.stderr


def rebind_scaling_identity(
    evidence: dict[str, Path | str | list[Path]],
    field: str,
    value: str,
    *,
    regenerate_report: bool = True,
) -> None:
    root = evidence["scaling_root"]
    manifest_path = evidence["scaling_manifest"]
    identity_path = evidence["scaling_identity"]
    assert isinstance(root, Path)
    assert isinstance(manifest_path, Path)
    assert isinstance(identity_path, Path)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    identity = json.loads(identity_path.read_text(encoding="utf-8"))
    identity["software"][field] = value
    write_pretty_json(identity_path, identity)
    manifest["identity"]["sha256"] = sha256(identity_path)
    for entry in manifest["runs"]:
        raw_path = root / entry["raw_samples"]["path"]
        raw = json.loads(raw_path.read_text(encoding="utf-8"))
        raw["identity_before"] = identity
        raw["identity_after"] = identity
        write_pretty_json(raw_path, raw)
        entry["raw_samples"]["sha256"] = sha256(raw_path)
    write_pretty_json(manifest_path, manifest)
    if regenerate_report:
        regenerate_scaling_report(evidence)


_execute_test_component_function(
    "sumeragi_v2_release_receipt_terminal_publication_cases.py",
    "test_receipt_hashes_every_formal_matrix_chaos_and_soak_artifact",
)




@pytest.mark.parametrize("mutation", ("unknown", "duplicate", "reordered"))
def test_receipt_rejects_noncanonical_prebuilt_manifest_fields(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    manifest = evidence["prebuilt_manifest"]
    assert isinstance(manifest, Path)
    rows = manifest.read_bytes().splitlines(keepends=True)
    if mutation == "unknown":
        rows.append(b"unexpected_field\tunexpected\n")
    elif mutation == "duplicate":
        rows.append(rows[0])
    else:
        rows[0], rows[1] = rows[1], rows[0]
    rewrite_prebuilt_manifest(evidence, b"".join(rows))
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "exact ordered 25 fields" in result.stderr


def test_receipt_rejects_forged_external_prebuilt_manifest_digest(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    completion = evidence["corridor_completion"]
    assert isinstance(completion, Path)
    fields = read_tsv_fields(completion)
    fields["prebuilt_manifest_sha256"] = "0" * 64
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "externally carried digest" in result.stderr


@pytest.mark.parametrize("mutation", ("completion_path", "bundle_field"))
def test_receipt_rejects_prebuilt_manifest_path_substitution(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    manifest = evidence["prebuilt_manifest"]
    completion = evidence["corridor_completion"]
    assert isinstance(manifest, Path)
    assert isinstance(completion, Path)
    if mutation == "completion_path":
        outside = tmp_path / "outside" / manifest.name
        outside.parent.mkdir()
        shutil.copy2(manifest, outside)
        fields = read_tsv_fields(completion)
        fields["prebuilt_manifest_path"] = str(outside)
        fields["prebuilt_manifest_sha256"] = sha256(outside)
        write_tsv(completion, fields)
        expected = "outside its exact source-bound invocation bundle"
    else:
        fields = read_tsv_fields(manifest)
        fields["bundle_dir"] = str(tmp_path / "substituted-bundle")
        rewrite_prebuilt_manifest(
            evidence,
            "".join(f"{name}\t{value}\n" for name, value in fields.items()).encode(),
        )
        expected = "not bound to the exact release identity"
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize("mutation", ("manifest_hash", "binary_bytes"))
def test_receipt_rejects_prebuilt_binary_hash_substitution(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    manifest = evidence["prebuilt_manifest"]
    binaries = evidence["prebuilt_binaries"]
    assert isinstance(manifest, Path)
    assert isinstance(binaries, list)
    module = load_writer_module()
    streamed = module._read_evidence_snapshot(
        binaries[0],
        "streamed prebuilt binary fixture",
        maximum_bytes=2 * 1024 * 1024 * 1024,
        expected_mode=0o500,
        allowed_owners={os.geteuid()},
        executable=True,
        retain_bytes=False,
    )
    assert isinstance(streamed, module.PathContract)
    assert not hasattr(streamed, "data")
    assert streamed.sha256 == sha256(binaries[0])
    if mutation == "manifest_hash":
        fields = read_tsv_fields(manifest)
        fields["iroha_sha256"] = "0" * 64
        rewrite_prebuilt_manifest(
            evidence,
            "".join(f"{name}\t{value}\n" for name, value in fields.items()).encode(),
        )
    else:
        binary = binaries[2]
        binary.chmod(0o700)
        binary.write_bytes(b"tampered-prebuilt-iroha\n")
        binary.chmod(0o500)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "binary identity does not match manifest" in result.stderr


@pytest.mark.parametrize(
    "fixture_name",
    ("prebuilt_manifest", "prebuilt_binary", "prebuilt_bundle"),
)
def test_receipt_rejects_prebuilt_artifact_mode_drift(
    tmp_path: Path, fixture_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    if fixture_name == "prebuilt_manifest":
        artifact = evidence["prebuilt_manifest"]
        expected = "exact mode 0400"
    elif fixture_name == "prebuilt_binary":
        binaries = evidence["prebuilt_binaries"]
        assert isinstance(binaries, list)
        artifact = binaries[0]
        expected = "exact mode 0500"
    else:
        artifact = evidence["prebuilt_bundle"]
        expected = "exact mode 0500"
    assert isinstance(artifact, Path)
    artifact.chmod(0o700)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize(
    ("field", "value", "expected"),
    (
        (
            "irohad_size_bytes",
            str(2 * 1024 * 1024 * 1024 + 1),
            "metadata is not exact and bounded",
        ),
        (
            "source_manifest_sha256",
            "0" * 64,
            "not bound to the exact release identity",
        ),
        (
            "cargo_lock_sha256",
            "0" * 64,
            "not bound to the exact release identity",
        ),
        (
            "cargo_version_sha256",
            "0" * 64,
            "Cargo version digest does not match the policy-captured corridor transcript",
        ),
        (
            "rustc_version_sha256",
            "0" * 64,
            "rustc version digest does not match the authenticated tool",
        ),
        (
            "host_triple",
            "aarch64-unknown-linux-gnu",
            "authenticated rustc version probe is not exact rustc -vV output",
        ),
    ),
)
def test_receipt_rejects_prebuilt_bound_and_toolchain_forgery(
    tmp_path: Path, field: str, value: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    manifest = evidence["prebuilt_manifest"]
    assert isinstance(manifest, Path)
    fields = read_tsv_fields(manifest)
    fields[field] = value
    if field == "host_triple":
        fields["target_triple"] = value
    rewrite_prebuilt_manifest(
        evidence,
        "".join(f"{name}\t{item}\n" for name, item in fields.items()).encode(),
    )
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize(
    "rustc_output",
    (
        RUSTC_VERSION_OUTPUT.replace(
            b"commit-hash: 01f6ddf7501f6ddf7501f6ddf7501f6ddf7501f6\n",
            b"commit-hash: ffffffffffffffffffffffffffffffffffffffff\n",
        ),
        RUSTC_VERSION_OUTPUT.replace(
            b"commit-date: 2026-02-11\n",
            b"commit-date: 2026-02-12\n",
        ),
        RUSTC_VERSION_OUTPUT.replace(
            b"LLVM version: 21.1.0\n",
            b"LLVM version: forged\n",
        ),
    ),
)
def test_receipt_rejects_self_consistent_forged_rustc_verbose_semantics(
    tmp_path: Path, rustc_output: bytes
) -> None:
    evidence = make_evidence(tmp_path)
    rustc = evidence["corridor_rustc_tool"]
    corridor = evidence["corridor_completion"]
    manifest = evidence["prebuilt_manifest"]
    assert isinstance(rustc, Path)
    assert isinstance(corridor, Path)
    assert isinstance(manifest, Path)
    rustc.chmod(0o700)
    rustc.write_bytes(
        b"#!/bin/sh\n"
        b"test \"$#\" = 1 && test \"$1\" = -vV || exit 92\n"
        b"cat <<'RUSTC_VERSION'\n"
        + rustc_output
        + b"RUSTC_VERSION\n"
    )
    rustc.chmod(0o500)
    rebind_bootstrap_runner_tool(evidence, "rustc")
    corridor_fields = read_tsv_fields(corridor)
    corridor_fields["rustc_sha256"] = sha256(rustc)
    write_tsv(corridor, corridor_fields)
    manifest_fields = read_tsv_fields(manifest)
    manifest_fields["rustc_version_sha256"] = hashlib.sha256(
        rustc_output
    ).hexdigest()
    rewrite_prebuilt_manifest(
        evidence,
        "".join(
            f"{name}\t{value}\n" for name, value in manifest_fields.items()
        ).encode(),
    )
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert (
        "authenticated rustc version probe is not exact rustc -vV output"
        in result.stderr
    )


def test_receipt_rejects_unmanifested_prebuilt_bundle_file(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    bundle = evidence["prebuilt_bundle"]
    assert isinstance(bundle, Path)
    bundle.chmod(0o700)
    (bundle / ".late.tmp").write_bytes(b"unmanifested\n")
    bundle.chmod(0o500)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "exact closed inventory" in result.stderr


@pytest.mark.parametrize("fixture_name", ("prebuilt_manifest", "prebuilt_binary"))
def test_receipt_rejects_prebuilt_artifact_symlink(
    tmp_path: Path, fixture_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    bundle = evidence["prebuilt_bundle"]
    assert isinstance(bundle, Path)
    if fixture_name == "prebuilt_manifest":
        artifact = evidence["prebuilt_manifest"]
    else:
        binaries = evidence["prebuilt_binaries"]
        assert isinstance(binaries, list)
        artifact = binaries[0]
    assert isinstance(artifact, Path)
    original_bytes = artifact.read_bytes()
    original_mode = stat.S_IMODE(artifact.stat().st_mode)
    replacement = tmp_path / f"{fixture_name}-replacement"
    replacement.write_bytes(original_bytes)
    replacement.chmod(original_mode)
    parent = artifact.parent
    parent.chmod(0o700)
    artifact.unlink()
    try:
        artifact.symlink_to(replacement)
    except (NotImplementedError, OSError) as error:
        pytest.skip(f"symlinks unavailable: {error}")
    parent.chmod(0o500)

    try:
        writer = fixture_writer(tmp_path)
        result = run_writer(evidence, terminal_output_path(evidence), writer)

        assert result.returncode == 1
        assert "non-symlink" in result.stderr
    finally:
        # Restore the sealed fixture before pytest's retained tmp-path cleanup.
        # Otherwise the external replacement can be removed before this
        # read-only parent's symlink, leaving a dangling link that pytest cannot
        # chmod and unlink on macOS.
        parent.chmod(0o700)
        if artifact.is_symlink() or artifact.exists():
            artifact.unlink()
        artifact.write_bytes(original_bytes)
        artifact.chmod(original_mode)
        parent.chmod(0o500)


def test_receipt_rejects_prebuilt_binary_hardlink_alias(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    binaries = evidence["prebuilt_binaries"]
    assert isinstance(binaries, list)
    try:
        os.link(binaries[0], tmp_path / "prebuilt-binary-alias")
    except OSError as error:
        pytest.skip(f"hard links unavailable: {error}")
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "exactly one hard link" in result.stderr


@pytest.mark.parametrize(
    ("completion_name", "expected"),
    (
        (
            "seed_completion",
            "seed completion does not describe the exact release matrix",
        ),
        (
            "taira_completion",
            "Taira completion is not bound to the exact release identity",
        ),
        (
            "g4p_completion",
            "G-4P completion is not exact passing release-bound accounting",
        ),
        (
            "g12_seed_completion",
            "G-12P seed completion is not exact passing release-bound accounting",
        ),
        (
            "g12_soak_completion",
            "G-12P fault-soak completion is not exact passing release-bound accounting",
        ),
    ),
)
def test_receipt_rejects_cross_completion_prebuilt_manifest_mismatch(
    tmp_path: Path, completion_name: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    completion = evidence[completion_name]
    assert isinstance(completion, Path)
    fields = read_tsv_fields(completion)
    fields["prebuilt_manifest_sha256"] = "0" * 64
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize("report_state", ["missing", "failed", "tampered"])
def test_receipt_requires_exact_existing_scaling_pass_report(
    tmp_path: Path, report_state: str
) -> None:
    evidence = make_evidence(tmp_path)
    report = evidence["scaling_report"]
    assert isinstance(report, Path)
    if report_state == "missing":
        report.unlink()
        expected = "lacks canonical validation_report.json"
    else:
        value = json.loads(report.read_text(encoding="utf-8"))
        if report_state == "failed":
            value["result"] = "fail"
            value["errors"] = ["fabricated failure"]
            value["metrics"] = None
            expected = "not an exact pass"
        else:
            value["metrics"]["four_to_one_median_throughput_ratio"] = 9.0
            expected = "does not match retained revalidation"
        write_pretty_json(report, value)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize(
    ("field", "value", "expected"),
    (
        ("source_revision", "f" * 40, "source_revision is not the sealed"),
        (
            "workspace_source_sha256",
            "e" * 64,
            "workspace_source_sha256 is not the sealed",
        ),
    ),
)
def test_receipt_rejects_valid_scaling_bundle_with_wrong_release_binding(
    tmp_path: Path, field: str, value: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    rebind_scaling_identity(evidence, field, value)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize(
    ("anchor", "expected"),
    (
        ("trial_harness", "trial harness is not the authenticated digest"),
        ("configuration", "configuration is not the authenticated digest"),
        ("irohad", "irohad_sha256 is not the authenticated digest"),
        ("iroha_cli", "iroha_cli_sha256 is not the authenticated digest"),
    ),
)
def test_receipt_rejects_self_consistent_scaling_bundle_outside_trust_anchors(
    tmp_path: Path, anchor: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    root = evidence["scaling_root"]
    manifest_path = evidence["scaling_manifest"]
    assert isinstance(root, Path)
    assert isinstance(manifest_path, Path)
    if anchor in {"irohad", "iroha_cli"}:
        rebind_scaling_identity(
            evidence,
            f"{anchor}_sha256",
            "e" * 64,
        )
    else:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        artifact_path = root / manifest[anchor]["path"]
        artifact_path.write_bytes(f"substituted {anchor}\n".encode("utf-8"))
        if anchor == "configuration":
            rebind_scaling_identity(
                evidence,
                "nexus_config_sha256",
                sha256(artifact_path),
                regenerate_report=False,
            )
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest[anchor]["sha256"] = sha256(artifact_path)
        write_pretty_json(manifest_path, manifest)
        regenerate_scaling_report(evidence)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize(
    ("mutation", "expected"),
    (
        ("symlink", "contains a symlink"),
        ("hardlink", "hard-link alias"),
        ("unexpected", "failed retained-validator revalidation"),
        ("oversize", "file exceeds its size limit"),
        ("count", "file-count limit"),
    ),
)
def test_receipt_rejects_unsafe_or_unbounded_scaling_bundle_inventory(
    tmp_path: Path, mutation: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    root = evidence["scaling_root"]
    trial_log = evidence["scaling_trial_log"]
    assert isinstance(root, Path)
    assert isinstance(trial_log, Path)
    if mutation == "symlink":
        try:
            (root / "late-link").symlink_to(trial_log)
        except (NotImplementedError, OSError) as error:
            pytest.skip(f"symlinks unavailable: {error}")
    elif mutation == "hardlink":
        try:
            os.link(trial_log, root / "late-hardlink")
        except OSError as error:
            pytest.skip(f"hard links unavailable: {error}")
    elif mutation == "unexpected":
        (root / "unexpected.txt").write_text("unexpected\n", encoding="utf-8")
    elif mutation == "oversize":
        with trial_log.open("r+b") as destination:
            destination.truncate(256 * 1024 * 1024 + 1)
    else:
        for index in range(257):
            (root / f"extra-{index:03d}").write_bytes(b"")
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


def test_receipt_rejects_scaling_artifact_mutated_by_revalidator(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    manifest = make_scaling_evidence(
        tmp_path,
        head="1" * 40,
        sealed_manifest="2" * 64,
    )
    manifest_path = manifest["scaling_manifest"]
    trial_log = manifest["scaling_trial_log"]
    assert isinstance(manifest_path, Path)
    assert isinstance(trial_log, Path)
    repo_root = tmp_path / "retained-root"
    retained = (
        repo_root / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py"
    )
    retained.parent.mkdir(parents=True)
    shutil.copy2(
        ROOT_DIR / "scripts" / "nexus" / retained.name,
        retained,
    )
    for relative in (
        Path("scripts/deploy_localnet.sh"),
        Path("scripts/tx_load.py"),
        Path("scripts/nexus_lane_load_test.py"),
    ):
        destination = repo_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    real_replay = module._run_bounded_replay

    def mutate_after_validation(*args: object, **kwargs: object) -> object:
        result = real_replay(*args, **kwargs)
        trial_log.write_text("mutated after validation\n", encoding="utf-8")
        return result

    monkeypatch.setattr(module, "_run_bounded_replay", mutate_after_validation)
    with pytest.raises(
        module.ReceiptError, match="changed during retained revalidation"
    ):
        module._validate_scaling_evidence(
            manifest_path=manifest_path,
            sealed={
                "head_commit": "1" * 40,
                "workspace_source_manifest_sha256": "2" * 64,
            },
            repo_root=repo_root,
            checker_environment=module._closed_replay_environment(repo_root),
            expected_trial_harness_sha256=manifest[
                "expected_scaling_trial_harness_sha256"
            ],
            expected_configuration_sha256=manifest[
                "expected_scaling_configuration_sha256"
            ],
            expected_irohad_sha256=manifest[
                "expected_scaling_irohad_sha256"
            ],
            expected_iroha_cli_sha256=manifest[
                "expected_scaling_iroha_cli_sha256"
            ],
        )


@pytest.mark.parametrize(
    ("fixture_name", "expected"),
    (
        ("g4p_completion", "G-4P completion is unavailable"),
        ("g4p_summary", "G-4P run summary is unavailable"),
        ("g4p_log", "G-4P run log 0 is unavailable"),
    ),
)
def test_receipt_requires_complete_g4p_evidence_inventory(
    tmp_path: Path, fixture_name: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    path = evidence[fixture_name]
    assert isinstance(path, Path)
    path.unlink()
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize(
    ("fixture_name", "expected"),
    (
        ("g4p_summary", "G-4P run summary digest mismatch"),
        ("g4p_log", "G-4P run log 0 digest mismatch"),
    ),
)
def test_receipt_rejects_tampered_g4p_summary_or_log(
    tmp_path: Path, fixture_name: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    path = evidence[fixture_name]
    assert isinstance(path, Path)
    path.write_text(
        path.read_text(encoding="utf-8") + "tampered\n",
        encoding="utf-8",
    )
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


def test_receipt_rejects_rehashed_g4p_run_identity_mismatch(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    summary = evidence["g4p_summary"]
    completion = evidence["g4p_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    rows = summary.read_text(encoding="utf-8").splitlines()
    fields = rows[1].split("\t")
    fields[0] = "native_amx_routing"
    rows[1] = "\t".join(fields)
    summary.write_text("\n".join(rows) + "\n", encoding="utf-8")
    completion_fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    completion_fields["runs_sha256"] = sha256(summary)
    write_tsv(completion, completion_fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "G-4P run summary row 0 is not canonical" in result.stderr


@pytest.mark.parametrize(
    ("field", "value"),
    (
        ("schema_version", "2"),
        ("source_manifest_sha256", "0" * 64),
    ),
)
def test_receipt_rejects_g4p_completion_schema_or_release_mismatch(
    tmp_path: Path, field: str, value: str
) -> None:
    evidence = make_evidence(tmp_path)
    completion = evidence["g4p_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = value
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert (
        "G-4P completion is not exact passing release-bound accounting"
        in result.stderr
    )


@pytest.mark.parametrize(
    ("fixture_name", "expected"),
    (
        ("g12_seed_completion", "G-12P seed completion is unavailable"),
        (
            "g12_soak_completion",
            "G-12P fault-soak completion is unavailable",
        ),
    ),
)
def test_receipt_requires_both_g12_completions(
    tmp_path: Path, fixture_name: str, expected: str
) -> None:
    evidence = make_evidence(tmp_path)
    path = evidence[fixture_name]
    assert isinstance(path, Path)
    path.unlink()
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize(
    ("fixture_name", "mutation", "expected"),
    (
        (
            "g12_seed_completion",
            "completion",
            "seed completion is not exact passing",
        ),
        ("g12_seed_summary", "append", "seed summary digest mismatch"),
        ("g12_seed_log", "append", "seed log 3 digest mismatch"),
        (
            "g12_soak_completion",
            "completion",
            "fault-soak completion is not exact passing",
        ),
        ("g12_soak_log", "append", "fault-soak log digest mismatch"),
    ),
)
def test_receipt_rejects_tampered_g12_accounting_summary_or_log(
    tmp_path: Path,
    fixture_name: str,
    mutation: str,
    expected: str,
) -> None:
    evidence = make_evidence(tmp_path)
    path = evidence[fixture_name]
    assert isinstance(path, Path)
    if mutation == "append":
        path.write_text(
            path.read_text(encoding="utf-8") + "tampered\n",
            encoding="utf-8",
        )
    else:
        fields = dict(
            line.split("\t", 1)
            for line in path.read_text(encoding="utf-8").splitlines()
        )
        if fixture_name == "g12_seed_completion":
            fields["passed_runs"] = "9"
        else:
            fields["duration_seconds"] = "7199"
        write_tsv(path, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected in result.stderr


def test_receipt_rejects_g12_completion_bound_to_wrong_release_source(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    completion = evidence["g12_seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["source_manifest_sha256"] = "0" * 64
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "seed completion is not exact passing release-bound" in result.stderr


def test_receipt_rejects_rehashed_noncanonical_apalache_evidence(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    apalache = evidence["formal_multilane_apalache_evidence"]
    workspace_manifest = evidence["sealed_manifest"]
    multilane_manifest = evidence["multilane_manifest"]
    assert isinstance(apalache, Path)
    assert isinstance(workspace_manifest, str) and isinstance(multilane_manifest, str)
    canonical = apalache.read_text(encoding="utf-8")
    mutations = (
        (f"workspace_source_manifest_sha256\t{workspace_manifest}\n", "", "wrong result inventory"),
        (f"multilane_source_manifest_sha256\t{multilane_manifest}\n", "", "wrong result inventory"),
        (workspace_manifest, multilane_manifest, "header is not the exact pinned profile"),
        (multilane_manifest, workspace_manifest, "header is not the exact pinned profile"),
        ("result_count\t6", "result_count\t5", "header is not the exact pinned profile"),
        ("result\tinflight-first-release-layout\t", "result\tinflight-first-release-refinement\t", "is not exact source-bound NoError evidence"),
    )
    module = load_writer_module()
    for old, new, expected in mutations:
        apalache.write_text(canonical.replace(old, new, 1), encoding="utf-8")
        snapshot = module._bounded_evidence_snapshot(
            apalache, "formal multilane Apalache evidence", maximum_bytes=module._MAX_SCALING_JSON_BYTES
        )
        with pytest.raises(module.ReceiptError, match=expected):
            module._validate_multilane_apalache_evidence(
                snapshot, workspace_manifest, multilane_manifest
            )


def test_receipt_rejects_legacy_formal_completion_without_trace_extraction(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    completion = evidence["formal_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["schema_version"] = "1"
    fields.pop("production_trace_extraction_evidence_sha256")
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert (
        "formal completion is release-ineligible without authenticated "
        "production trace-extraction evidence"
    ) in result.stderr


def test_receipt_rejects_trace_extraction_not_authenticated_by_checker(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    trace_evidence = evidence["formal_production_trace_extraction_evidence"]
    apalache = evidence["formal_multilane_apalache_evidence"]
    completion = evidence["formal_completion"]
    sealed_manifest = evidence["sealed_manifest"]
    multilane_manifest = evidence["multilane_manifest"]
    assert isinstance(trace_evidence, Path) and isinstance(apalache, Path)
    assert isinstance(completion, Path) and isinstance(sealed_manifest, str)
    assert isinstance(multilane_manifest, str)
    substituted_manifest = "d" * 64
    trace_evidence.write_bytes(canonical_json({
        "backend_verification": True, "canonical": True,
        "multilane_source_manifest_sha256": substituted_manifest,
        "theorem": "sumeragi-v2-production-trace-extraction",
        "workspace_source_manifest_sha256": sealed_manifest,
    }))
    apalache.write_text(
        apalache.read_text(encoding="utf-8").replace(multilane_manifest, substituted_manifest, 1),
        encoding="utf-8",
    )
    fields = dict(line.split("\t", 1) for line in completion.read_text(encoding="utf-8").splitlines())
    fields["production_trace_extraction_evidence_sha256"] = sha256(trace_evidence)
    fields["multilane_apalache_evidence_sha256"] = sha256(apalache)
    write_tsv(completion, fields)
    result = run_writer(evidence, terminal_output_path(evidence), fixture_writer(tmp_path))
    assert result.returncode == 1
    assert "archived formal evidence does not authenticate production trace extraction" in result.stderr


def test_receipt_links_required_cross_tool_evidence(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    cross_tool = evidence["formal_cross_tool_evidence"]
    assert isinstance(cross_tool, Path)
    writer = fixture_writer(tmp_path)
    output = terminal_output_path(evidence)

    result = run_writer(evidence, output, writer)

    assert result.returncode == 0, result.stderr
    receipt = json.loads(output.read_text(encoding="utf-8"))
    assert receipt["evidence"]["formal_cross_tool_evidence"] == {
        "path": str(cross_tool.resolve()),
        "sha256": sha256(cross_tool),
    }


def test_receipt_rejects_missing_required_cross_tool_evidence(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    cross_tool = evidence["formal_cross_tool_evidence"]
    assert isinstance(cross_tool, Path)
    cross_tool.unlink()
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "formal cross-tool evidence is not a regular file" in result.stderr


def test_receipt_rejects_stale_cross_tool_evidence(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    cross_tool = evidence["formal_cross_tool_evidence"]
    assert isinstance(cross_tool, Path)
    cross_tool.write_text('{"stale":true}\n', encoding="utf-8")
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "formal cross-tool evidence digest mismatch" in result.stderr


def test_receipt_rejects_resource_summary_above_the_formal_memory_ceiling(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    jsonl = evidence["formal_tlaps_resource_jsonl"]
    summary_path = evidence["formal_tlaps_resource_summary"]
    assert isinstance(jsonl, Path)
    assert isinstance(summary_path, Path)
    records = [
        json.loads(line)
        for line in jsonl.read_text(encoding="utf-8").splitlines()
    ]
    document = json.loads(summary_path.read_text(encoding="utf-8"))
    document["peak_memory_bytes"] = document["memory_limit_bytes"] + 1
    records[-1] = document
    rewrite_tlaps_resource_evidence(evidence, records, document)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "not a successful bounded release run" in result.stderr


@pytest.mark.parametrize(
    ("field", "value"),
    [
        pytest.param("schema_version", True, id="boolean-schema-version"),
        pytest.param("exit_status", False, id="boolean-exit-status"),
        pytest.param("peak_memory_bytes", False, id="boolean-peak-memory"),
    ],
)
def test_receipt_rejects_boolean_tlaps_resource_summary_integers(
    tmp_path: Path, field: str, value: bool
) -> None:
    evidence = make_evidence(tmp_path)
    jsonl = evidence["formal_tlaps_resource_jsonl"]
    summary_path = evidence["formal_tlaps_resource_summary"]
    assert isinstance(jsonl, Path)
    assert isinstance(summary_path, Path)
    records = [
        json.loads(line)
        for line in jsonl.read_text(encoding="utf-8").splitlines()
    ]
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary[field] = value
    records[-1] = summary
    rewrite_tlaps_resource_evidence(evidence, records, summary)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "not one bounded integer" in result.stderr


def test_receipt_rejects_extra_tlaps_resource_summary_field(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    jsonl = evidence["formal_tlaps_resource_jsonl"]
    summary_path = evidence["formal_tlaps_resource_summary"]
    assert isinstance(jsonl, Path)
    assert isinstance(summary_path, Path)
    records = [
        json.loads(line)
        for line in jsonl.read_text(encoding="utf-8").splitlines()
    ]
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["unreviewed"] = 0
    records[-1] = summary
    rewrite_tlaps_resource_evidence(evidence, records, summary)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "fields do not match its canonical schema" in result.stderr


def test_receipt_rejects_tlaps_resource_stream_with_forged_aggregate(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    jsonl = evidence["formal_tlaps_resource_jsonl"]
    summary_path = evidence["formal_tlaps_resource_summary"]
    assert isinstance(jsonl, Path)
    assert isinstance(summary_path, Path)
    records = [
        json.loads(line)
        for line in jsonl.read_text(encoding="utf-8").splitlines()
    ]
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    sample = records[2]
    sample["memory_bytes"] = 2048
    sample["physical_footprint_bytes"] = 2048
    sample["rss_bytes"] = 2048
    rewrite_tlaps_resource_evidence(evidence, records, summary)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "summary peaks do not match" in result.stderr


@pytest.mark.parametrize(
    "mutation",
    [
        "missing-spawn",
        "extra-start-field",
        "spawn-supervisor-collision",
        "spawn-wrapper-group-collision",
        "wrong-sample-process-group",
        "terminal-summary-mismatch",
    ],
)
def test_receipt_rejects_noncanonical_tlaps_resource_stream(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    jsonl = evidence["formal_tlaps_resource_jsonl"]
    summary_path = evidence["formal_tlaps_resource_summary"]
    assert isinstance(jsonl, Path)
    assert isinstance(summary_path, Path)
    records = [
        json.loads(line)
        for line in jsonl.read_text(encoding="utf-8").splitlines()
    ]
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    if mutation == "missing-spawn":
        records.pop(1)
    elif mutation == "extra-start-field":
        records[0]["unreviewed"] = 0
    elif mutation == "spawn-supervisor-collision":
        records[1]["wrapper_pid"] = summary["supervisor_pid"]
    elif mutation == "spawn-wrapper-group-collision":
        records[1]["wrapper_pid"] = records[1]["process_group_id"]
    elif mutation == "wrong-sample-process-group":
        records[2]["process_group_id"] += 1
    else:
        records[-1] = {**summary, "ended_utc": "2026-07-22T00:00:02.000Z"}
    rewrite_tlaps_resource_evidence(evidence, records, summary)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "TLAPS resource" in result.stderr


def test_receipt_rejects_substituted_cross_tool_evidence(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    cross_tool = evidence["formal_cross_tool_evidence"]
    assert isinstance(cross_tool, Path)
    cross_tool.write_text(
        '{"backend_verification":true,"canonical":false}\n', encoding="utf-8"
    )
    completion = evidence["formal_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["cross_tool_evidence_sha256"] = sha256(cross_tool)
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "archived formal ledger/evidence failed release validation" in result.stderr


def test_receipt_rejects_formal_release_ledger_without_cross_tool_obligations(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    formal_ledger = evidence["formal_ledger"]
    formal_completion = evidence["formal_completion"]
    assert isinstance(formal_ledger, Path)
    assert isinstance(formal_completion, Path)
    formal_ledger.write_text(
        '{"machine_checked_completion":true}\n', encoding="utf-8"
    )
    fields = dict(
        line.split("\t", 1)
        for line in formal_completion.read_text(encoding="utf-8").splitlines()
    )
    fields["proof_coverage_sha256"] = sha256(formal_ledger)
    write_tsv(formal_completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "formal release ledger does not require cross-tool evidence" in result.stderr


def test_verify_existing_rebuilds_and_durably_accepts_the_exact_receipt(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    output = terminal_output_path(evidence)

    published = run_writer(evidence, output, writer)
    assert published.returncode == 0, published.stderr
    evidence_directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(evidence_directory, Path)
    for name in ("runner-stdout.log", "runner-stderr.log"):
        (evidence_directory / name).chmod(0o400)

    verified = run_writer(
        evidence,
        output,
        writer,
        verify_existing=True,
    )

    assert verified.returncode == 0, verified.stderr
    assert "aggregate release receipt verified" in verified.stdout
    assert stat.S_IMODE(output.stat().st_mode) == 0o400
    acknowledgment = json.loads(
        (output.parents[2] / "receipt-validation-ack.json").read_bytes()
    )
    assert acknowledgment["schema_version"] == 3
    invocation = acknowledgment["invocation"]
    invocation_core = {
        key: invocation[key]
        for key in (
            "profile", "operation", "python_flags", "validator", "ordered_options"
        )
    }
    assert invocation["invocation_sha256"] == hashlib.sha256(
        json.dumps(
            invocation_core,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    ).hexdigest()
    names = [binding["name"] for binding in invocation["ordered_options"]]
    assert names[-3:] == [
        "--verify-existing", "--validation-ack", "--source-manifest-sha256"
    ]
    source_binding = invocation["ordered_options"][-1]
    assert source_binding["normalized_value_sha256"] == hashlib.sha256(
        json.dumps(
            {"kind": "text", "value": evidence["sealed_manifest"]},
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    ).hexdigest()
    assert str(evidence["bootstrap_candidate_root"]) not in json.dumps(invocation)


def test_verify_existing_rejects_terminal_receipt_semantic_drift(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    output = terminal_output_path(evidence)

    published = run_writer(evidence, output, writer)
    assert published.returncode == 0, published.stderr
    evidence_directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(evidence_directory, Path)
    for name in ("runner-stdout.log", "runner-stderr.log"):
        (evidence_directory / name).chmod(0o400)
    receipt = json.loads(output.read_text(encoding="utf-8"))
    receipt["identity"]["head_commit"] = "9" * 40
    rewrite_json(output, receipt)

    verified = run_writer(
        evidence,
        output,
        writer,
        verify_existing=True,
    )

    assert verified.returncode == 1
    assert "existing terminal receipt" in verified.stderr




for _release_receipt_test_component in RELEASE_RECEIPT_TEST_COMPONENT_FILES:
    _execute_test_component(_release_receipt_test_component)
del _release_receipt_test_component
