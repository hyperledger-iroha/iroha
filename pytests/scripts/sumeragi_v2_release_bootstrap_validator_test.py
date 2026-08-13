"""Adversarial tests for the Sumeragi v2 bootstrap completion validator."""

from __future__ import annotations

from dataclasses import dataclass
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import sysconfig
from typing import Any

import pytest

from pytests.scripts.sumeragi_v2_release_bootstrap_tool_manifest_support import (
    REQUIRED_RUNNER_TOOL_NAMES,
    fixture_tool_probe_helper,
    provision_archived_python_runtime as _provision_archived_python_runtime,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = REPO_ROOT / "scripts" / "validate_sumeragi_v2_release_bootstrap.py"
BOOTSTRAP = REPO_ROOT / "scripts" / "bootstrap_sumeragi_v2_release.py"
BOOTSTRAP_COMPONENT_FILES = (
    REPO_ROOT / "scripts" / "bootstrap_sumeragi_v2_release_receipt_replay.py",
)
RECEIPT_VALIDATOR_COMPONENT_FILES = tuple(
    REPO_ROOT / "scripts" / name
    for name in (
        "write_sumeragi_v2_release_receipt_formal_artifacts.py",
        "write_sumeragi_v2_release_receipt_corridor_log.py",
        "write_sumeragi_v2_release_receipt_gate_evidence.py",
        "write_sumeragi_v2_release_receipt_publication.py",
    )
)
APPROVAL_CONTRACT = (
    REPO_ROOT / "scripts" / "sumeragi_v2_release_approval_contract.py"
)
RUNTIME_HELPER = REPO_ROOT / "scripts" / "copy_sumeragi_v2_release_cargo_cache.py"
PYTHON = Path(sys.executable).resolve(strict=True)
FRAMEWORK_PYTHON = (
    sys.platform == "darwin"
    and isinstance(sysconfig.get_config_var("PYTHONFRAMEWORK"), str)
    and bool(sysconfig.get_config_var("PYTHONFRAMEWORK"))
)
FINGERPRINT = "SHA256:" + "A" * 43
OTHER_FINGERPRINT = "SHA256:" + "B" * 43
APPROVAL_EVIDENCE_ROOT_ID = "fixture-release-evidence-root"
APPROVAL_DURATIONS = (900, 901, 902, 903)
APPROVAL_CLASS_IDS = (
    "offline-toolchain-sdk",
    "formal-proof-tools",
    "network-scale-soak",
    "final-bootstrap-publication",
)

TRUSTED_NAMES = {
    "allowed_signers": "bootstrap-allowed-signers",
    "bash": "bash",
    "bootstrap": "trusted-bootstrap.py",
    "git": "git",
    "identity_verifier": "verify-identity.py",
    "manifest_helper": "compute-manifest.py",
    "python": (
        "python-runtime/bin/python3" if FRAMEWORK_PYTHON else "python3"
    ),
    "receipt_validator": "validate-receipt.py",
    "receipt_validator_support": "sumeragi_v2_localnet_manifest.py",
    "runtime_helper": "copy-release-runtime.py",
    "tool_probe_helper": "probe-release-tools.py",
    "approval_contract": "release-approval-contract.py",
    "approval_offline_toolchain_sdk": "offline-toolchain-sdk.approval.v1.json",
    "approval_formal_proof_tools": "formal-proof-tools.approval.v1.json",
    "approval_network_scale_soak": "network-scale-soak.approval.v1.json",
    "approval_final_bootstrap_publication": (
        "final-bootstrap-publication.approval.v1.json"
    ),
    "sdk_dependency_bundle_manifest": "sdk-dependency-bundle-manifest.json",
    "revocation": "bootstrap-revocation",
    "runner_tool_manifest": "runner-tool-manifest.json",
    "ssh_keygen": "ssh-keygen",
}


def _load_approval_component(path: Path) -> object:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_validator_approval_fixture", path
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _approval_fixture(
    *,
    module: object,
    identity: dict[str, Any],
    tool_manifest_sha256: str,
    trust: Path,
    evidence: Path,
) -> tuple[dict[str, Path], dict[str, Any]]:
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
    archives: dict[str, Path] = {}
    paths: dict[object, Path] = {}
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
        data = _canonical(value)
        label = "approval_" + class_id.replace("-", "_")
        _write(trust / f"source-{label}", data, 0o400)
        path = _write(evidence / TRUSTED_NAMES[label], data, 0o400)
        archives[label] = path
        paths[approval_class] = path
    approvals = module.load_protected_release_approval_set(
        paths, expectations=expectations, expected_owner_uid=os.getuid()
    )
    class_records: dict[str, Any] = {}
    for approval in approvals:
        class_id = approval.class_id.value
        sanitized = approval.sanitized_archive()
        name = f"{class_id}.approval-attestation.v1.json"
        path = _write(evidence / name, sanitized.canonical_bytes, 0o400)
        class_records[class_id] = {
            "archive_id": module.APPROVAL_ARCHIVE_IDS[approval.class_id],
            "archive_name": name,
            "mode": "0400",
            "sha256": sanitized.sha256,
            "size_bytes": path.stat().st_size,
        }
    sanitized_set = module.sanitized_release_approval_set_archive(approvals)
    set_name = "release-approval-set-attestation.v1.json"
    set_path = _write(evidence / set_name, sanitized_set.canonical_bytes, 0o400)
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
IDENTITY_NAMES = {
    "cargo_lock": "identity-Cargo.lock",
    "git": "identity-git",
    "raw_commit": "identity-raw-commit",
    "ssh_allowed_signers": "identity-allowed-signers",
    "ssh_keygen": "identity-ssh-keygen",
    "ssh_revocation": "identity-revocation",
    "verify_transcript": "identity-transcript.json",
}
IDENTITY_IDS = {
    "cargo_lock": "release-identity.cargo-lock.v1",
    "git": "release-identity.git.v1",
    "raw_commit": "release-identity.raw-commit.v1",
    "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
    "ssh_keygen": "release-identity.ssh-keygen.v1",
    "ssh_revocation": "release-identity.ssh-revocation.v1",
    "verify_transcript": "release-identity.verify-transcript.v1",
}


def _load_validator_module() -> object:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_validator_test_module", VALIDATOR
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_bootstrap_module() -> object:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_test_module", BOOTSTRAP
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _fixture_validator_invocation(
    bootstrap: object,
    *,
    invocation: Path,
    evidence: Path,
    source: Path,
    receipt: Path,
    acknowledgment: Path,
    source_manifest_sha256: str,
) -> dict[str, Any]:
    known = _fixture_validator_values(
        bootstrap,
        invocation=invocation,
        evidence=evidence,
        source=source,
        receipt=receipt,
        acknowledgment=acknowledgment,
        source_manifest_sha256=source_manifest_sha256,
    )
    bindings = []
    for name in bootstrap._VALIDATOR_OPTION_ORDER:
        kind, value = known[name]
        bindings.append(
            {
                "name": name,
                "value_kind": kind,
                "normalized_value_sha256": hashlib.sha256(
                    json.dumps(
                        {"kind": kind, "value": value},
                        ensure_ascii=False,
                        separators=(",", ":"),
                        sort_keys=True,
                    ).encode("utf-8")
                ).hexdigest(),
            }
        )
    record = {
        "profile": "release",
        "operation": "verify-existing-and-ack",
        "python_flags": ["-I", "-S"],
        "validator": "protected:validate-receipt.py",
        "ordered_options": bindings,
    }
    return {
        **record,
        "invocation_sha256": hashlib.sha256(
            json.dumps(
                record,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            ).encode("utf-8")
        ).hexdigest(),
    }


def _fixture_validator_values(
    bootstrap: object,
    *,
    invocation: Path,
    evidence: Path,
    source: Path,
    receipt: Path,
    acknowledgment: Path,
    source_manifest_sha256: str,
) -> dict[str, tuple[str, str | bool]]:
    values: dict[str, tuple[str, str | bool]] = {
        name: (
            "flag",
            True,
        )
        if name == "--verify-existing"
        else (
            "path",
            str(invocation / "fixture" / name[2:]),
        )
        if name in bootstrap._VALIDATOR_PATH_OPTIONS
        else ("text", f"fixture:{name}")
        for name in bootstrap._VALIDATOR_OPTION_ORDER
    }
    values.update({
        "--candidate-identity": ("path", str(evidence / "candidate-identity.json")),
        "--sealed-identity": ("path", str(invocation / "sealed-identity.json")),
        "--release-root": ("path", str(source)),
        "--bootstrap-completion": ("path", str(evidence / "BOOTSTRAP_COMPLETED.json")),
        "--bootstrap-evidence-dir": ("path", str(evidence)),
        "--bootstrap-identity": ("path", str(evidence / "candidate-identity.json")),
        "--bootstrap-attestation": ("path", str(evidence / "identity-attestation.json")),
        "--bootstrap-transcript": ("path", str(evidence / "identity-transcript.json")),
        "--expected-bootstrap-completion-sha256": (
            "text", _digest((evidence / "BOOTSTRAP_COMPLETED.json").read_bytes())
        ),
        "--bootstrap-candidate-root": ("path", str(evidence.parent / "candidate")),
        "--bootstrap-runner": (
            "path", str(evidence.parent / "candidate" / "scripts" / "run_sumeragi_v2_release_gates.sh")
        ),
        "--repository-root": ("path", str(source)),
        "--sdk-dependency-archive": (
            "path", str(invocation / "sdk-dependency-bundle.tar"),
        ),
        "--sdk-dependency-input-inventory": (
            "path", str(invocation / "sdk-dependency-input.json"),
        ),
        "--sdk-dependency-final-work-inventory": (
            "path", str(invocation / "sdk-dependency-work-final.json"),
        ),
        "--runtime-tool-probe-manifest": (
            "path", str(invocation / "runtime-tool-probe-manifest.json"),
        ),
        "--runtime-tool-probe-result": (
            "path", str(invocation / "runtime-tool-probe-result.json"),
        ),
        "--output": ("path", str(receipt)),
        "--verify-existing": ("flag", True),
        "--validation-ack": ("path", str(acknowledgment)),
        "--source-manifest-sha256": ("text", source_manifest_sha256),
    })
    return values


def _fixture_receipt_for_validator(
    values: dict[str, tuple[str, str | bool]],
) -> dict[str, Any]:
    path = lambda name: {"path": values[name][1], "sha256": "1" * 64}
    return {
        "identity": {"sealed_source_manifest_sha256": values["--source-manifest-sha256"][1]},
        "authentication": {
            "bootstrap": {
                "completion_sha256": values["--expected-bootstrap-completion-sha256"][1],
                "candidate_root": values["--bootstrap-candidate-root"][1],
                "runner": {"path": values["--bootstrap-runner"][1]},
            },
            "release_identity": {
                "trust_policy": {
                    "git_sha256": values["--expected-git-sha256"][1],
                    "ssh_keygen_sha256": values["--expected-ssh-keygen-sha256"][1],
                    "allowed_signers_sha256": values["--expected-allowed-signers-sha256"][1],
                    "revocation_sha256": values["--expected-revocation-sha256"][1],
                    "signer_fingerprint": values["--expected-signer-fingerprint"][1],
                }
            },
        },
        "evidence": {
            "bootstrap": {
                "candidate_identity": path("--candidate-identity"),
                "completion": path("--bootstrap-completion"),
                "identity_verification": {
                    "identity_attestation": path("--bootstrap-attestation"),
                    "identity_transcript": path("--bootstrap-transcript"),
                },
            },
            "release_signature_attestation": path("--signature-attestation"),
            "release_signature_transcript": path("--signature-transcript"),
            "release_signature_raw_commit": path("--signature-raw-commit"),
            "release_signature_cargo_lock": path("--signature-cargo-lock"),
            "release_signature_allowed_signers": path("--signature-allowed-signers"),
            "release_signature_revocation": path("--signature-revocation"),
            "release_signature_git": path("--signature-git"),
            "release_signature_ssh_keygen": path("--signature-ssh-keygen"),
            "corridor_completion": path("--corridor-completion"),
            "formal_completion": path("--formal-completion"),
            "seed_matrix_completion": path("--seed-completion"),
            "chaos_completion": path("--chaos-completion"),
            "taira_completion": path("--taira-completion"),
            "g4p_multilane": {"completion": path("--g4p-completion")},
            "g12_cross_dataspace": {
                "seed_completion": path("--g12-seed-completion"),
                "fault_soak_completion": path("--g12-fault-soak-completion"),
            },
            "multilane_scaling_bundle": {
                "files": [
                    {
                        "relative_path": "scaling_evidence.json",
                        **path("--scaling-evidence-manifest"),
                    }
                ]
            },
            "multilane_scaling_trust_anchors": {
                "trial_harness_sha256": values["--expected-scaling-trial-harness-sha256"][1],
                "configuration_sha256": values["--expected-scaling-configuration-sha256"][1],
                "irohad_sha256": values["--expected-scaling-irohad-sha256"][1],
                "iroha_cli_sha256": values["--expected-scaling-iroha-cli-sha256"][1],
            },
        },
    }


def test_validator_supervision_has_no_forbidden_process_controls() -> None:
    source = VALIDATOR.read_text(encoding="utf-8")
    for forbidden in (
        "import signal",
        "os.kill(",
        "os.killpg(",
        ".kill(",
        ".terminate(",
        "start_new_session",
        "def _abort",
        "wait(timeout=",
    ):
        assert forbidden not in source


@pytest.mark.parametrize(
    ("timeout_seconds", "maximum_output_bytes", "program", "message"),
    [
        (
            0,
            1024,
            "import time; time.sleep(0.05)",
            "runtime limit",
        ),
        (
            5,
            32,
            "import sys; "
            "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
            "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush()",
            "output limit",
        ),
    ],
)
def test_manifest_helper_finishes_naturally_before_reporting_latched_violation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    timeout_seconds: int,
    maximum_output_bytes: int,
    program: str,
    message: str,
) -> None:
    module = _load_validator_module()
    monkeypatch.setattr(module, "_COMMAND_TIMEOUT_SECONDS", timeout_seconds)
    monkeypatch.setattr(module, "_MAX_HELPER_OUTPUT_BYTES", maximum_output_bytes)
    sentinel = tmp_path / "natural-completion"
    child = (
        f"{program}; from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(module.ValidationError, match=message):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", child),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


@pytest.mark.parametrize(
    "fault_method", ("register", "select", "read", "wait")
)
def test_manifest_helper_drains_after_generic_supervisor_exception(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    fault_method: str,
) -> None:
    module = _load_validator_module()
    real_selector = module.selectors.DefaultSelector

    class FaultingSelector:
        def __init__(self) -> None:
            self._selector = real_selector()
            self._failed = False

        def __getattr__(self, name: str) -> object:
            return getattr(self._selector, name)

        def register(self, *args: object, **kwargs: object) -> object:
            if fault_method == "register" and not self._failed:
                self._failed = True
                raise RuntimeError("injected supervisor failure")
            return self._selector.register(*args, **kwargs)

        def select(self, timeout: float | None = None) -> object:
            if fault_method == "select" and not self._failed:
                self._failed = True
                raise RuntimeError("injected supervisor failure")
            return self._selector.select(timeout)

    monkeypatch.setattr(module.selectors, "DefaultSelector", FaultingSelector)
    if fault_method == "read":
        real_read = module.os.read
        real_popen = module.subprocess.Popen
        read_armed = False
        read_failed = False

        def faulting_read(descriptor: int, size: int) -> bytes:
            nonlocal read_failed
            if read_armed and not read_failed:
                read_failed = True
                raise RuntimeError("injected supervisor failure")
            return real_read(descriptor, size)

        def arming_popen(*args: object, **kwargs: object) -> object:
            nonlocal read_armed
            process = real_popen(*args, **kwargs)
            read_armed = True
            return process

        monkeypatch.setattr(module.os, "read", faulting_read)
        monkeypatch.setattr(module.subprocess, "Popen", arming_popen)
    if fault_method == "wait":
        real_popen = module.subprocess.Popen

        class FaultingProcess:
            def __init__(self, *args: object, **kwargs: object) -> None:
                self._process = real_popen(*args, **kwargs)
                self._failed = False

            def __getattr__(self, name: str) -> object:
                return getattr(self._process, name)

            def wait(self) -> int:
                if not self._failed:
                    self._failed = True
                    raise RuntimeError("injected supervisor failure")
                return self._process.wait()

        monkeypatch.setattr(module.subprocess, "Popen", FaultingProcess)
    sentinel = tmp_path / "supervisor-exception-natural-completion"
    child = (
        "import sys; "
        "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
        "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush(); "
        "from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(RuntimeError, match="injected supervisor failure"):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", child),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _write(path: Path, data: bytes | str, mode: int) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and not path.is_symlink():
        path.chmod(0o600)
    if isinstance(data, str):
        data = data.encode()
    path.write_bytes(data)
    path.chmod(mode)
    return path.resolve(strict=True)


def _copy(source: Path, destination: Path, mode: int) -> Path:
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    destination.chmod(mode)
    return destination.resolve(strict=True)


def _source_payload(candidate: Path) -> bytes:
    data = b""
    for relative in (
        "Cargo.lock",
        "payload",
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/validate_sumeragi_v2_release_bootstrap.py",
    ):
        data += relative.encode() + b"\0" + (candidate / relative).read_bytes()
    return data


def _identity(candidate: Path) -> dict[str, Any]:
    payload = _source_payload(candidate)
    tree = hashlib.sha256(b"tree" + payload).hexdigest()[:40]
    return {
        "schema_version": 1,
        "head_commit": (candidate / "HEAD_ID").read_text(encoding="ascii").strip(),
        "head_tree": tree,
        "index_tree": tree,
        "workspace_source_manifest_sha256": _digest(payload),
        "cargo_lock_sha256": _digest((candidate / "Cargo.lock").read_bytes()),
    }


def _raw_commit(tree: str, manifest: str, lock: str) -> bytes:
    signature = base64.b64encode(b"synthetic authenticated signature")
    return (
        f"tree {tree}\n"
        "author Release Test <release@example.test> 0 +0000\n"
        "committer Release Test <release@example.test> 0 +0000\n"
        "gpgsig -----BEGIN SSH SIGNATURE-----\n"
        f" {signature.decode()}\n"
        " -----END SSH SIGNATURE-----\n"
        "\n"
        "Synthetic signed release\n"
        "\n"
        "Sumeragi-V2-Release-Identity-Version: 1\n"
        f"Sumeragi-V2-Source-Manifest-SHA256: {manifest}\n"
        f"Sumeragi-V2-Cargo-Lock-SHA256: {lock}\n"
    ).encode()


def _oid(raw: bytes) -> str:
    framed = b"commit " + str(len(raw)).encode() + b"\0" + raw
    return hashlib.sha1(framed, usedforsecurity=False).hexdigest()


def _manifest_helper() -> str:
    return r'''#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--root", type=Path, required=True)
parser.add_argument("--release-identity-json", action="store_true", required=True)
args = parser.parse_args()
root = args.root
payload = b""
for relative in (
    "Cargo.lock",
    "payload",
    "scripts/run_sumeragi_v2_release_gates.sh",
    "scripts/validate_sumeragi_v2_release_bootstrap.py",
):
    payload += relative.encode() + b"\0" + (root / relative).read_bytes()
tree = hashlib.sha256(b"tree" + payload).hexdigest()[:40]
value = {
    "schema_version": 1,
    "head_commit": (root / "HEAD_ID").read_text(encoding="ascii").strip(),
    "head_tree": tree,
    "index_tree": tree,
    "workspace_source_manifest_sha256": hashlib.sha256(payload).hexdigest(),
    "cargo_lock_sha256": hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest(),
}
print(json.dumps(value, sort_keys=True, separators=(",", ":")))
'''


def _artifact(data: bytes, name: str, mode: int) -> dict[str, Any]:
    return {
        "archive_name": name,
        "mode": f"{mode:04o}",
        "sha256": _digest(data),
        "size_bytes": len(data),
    }


def _framework_python_runtime_record(inventory_path: Path) -> dict[str, Any]:
    """Project one private helper inventory into the public marker schema."""

    data = inventory_path.read_bytes()
    inventory = json.loads(data)
    assert data == _canonical(inventory)
    assert set(inventory) == {
        "format",
        "schema_version",
        "runtime_root",
        "record_count",
        "file_bytes",
        "records",
        "source_disclosure",
        "input_record_count",
        "input_file_bytes",
        "input_records",
    }
    assert inventory["format"] == "iroha-sumeragi-v2-private-framework-python-runtime"
    assert inventory["schema_version"] == 1
    assert inventory["source_disclosure"] == "withheld"
    projected: list[dict[str, Any]] = []
    for record in inventory["records"]:
        kind = record["kind"]
        keys = (
            ("path", "kind", "mode")
            if kind == "directory"
            else ("path", "kind", "mode", "size", "sha256")
            if kind == "file"
            else ("path", "kind", "mode", "target")
        )
        projected.append({key: record[key] for key in keys})
    projected.sort(key=lambda record: record["path"])
    assert inventory["record_count"] == len(projected)
    assert inventory["file_bytes"] == sum(
        record["size"] for record in projected if record["kind"] == "file"
    )
    metadata = inventory_path.stat()
    return {
        "format": "iroha-sumeragi-v2-framework-python-runtime",
        "schema_version": 1,
        "archive_root": "python-runtime",
        "root_mode": "0500",
        "executable": "bin/python3",
        "inventory": {
            "archive_name": "python-runtime-input.json",
            "mode": f"{stat.S_IMODE(metadata.st_mode):04o}",
            "sha256": _digest(data),
            "size_bytes": len(data),
        },
        "record_count": len(projected),
        "file_bytes": inventory["file_bytes"],
        "records": projected,
    }


def _protected(data: bytes, name: str, mode: int) -> dict[str, Any]:
    return {
        "archive_name": name,
        "mode": f"{mode:04o}",
        "observed_sha256": _digest(data),
        "protected_sha256": _digest(data),
        "size_bytes": len(data),
    }


def _command(argv: list[str], status: int, stdout: bytes = b"", stderr: bytes = b"") -> dict[str, Any]:
    return {
        "argv": argv,
        "replay_argv": argv,
        "exit_status": status,
        "stdout_base64": base64.b64encode(stdout).decode(),
        "stdout_sha256": _digest(stdout),
        "stdout_size_bytes": len(stdout),
        "stderr_base64": base64.b64encode(stderr).decode(),
        "stderr_sha256": _digest(stderr),
        "stderr_size_bytes": len(stderr),
    }


@dataclass
class Fixture:
    root: Path
    candidate: Path
    trust: Path
    evidence: Path
    runner: Path
    validator: Path
    environment: dict[str, str]

    @property
    def marker_path(self) -> Path:
        return self.evidence / "BOOTSTRAP_COMPLETED.json"

    def marker(self) -> dict[str, Any]:
        return json.loads(self.marker_path.read_bytes())

    def seal_marker(self, value: dict[str, Any], *, canonical: bool = True) -> None:
        data = _canonical(value) if canonical else json.dumps(value, indent=2).encode() + b"\n"
        _write(self.marker_path, data, 0o400)
        digest = _digest(data)
        self.environment["IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"] = digest
        self.environment["SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"] = digest

    def reseal_attestation(self, value: dict[str, Any]) -> None:
        path = self.evidence / "identity-attestation.json"
        data = _canonical(value)
        _write(path, data, 0o400)
        marker = self.marker()
        marker["identity_verification"]["identity_attestation"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        self.seal_marker(marker)

    def reseal_transcript(self, value: dict[str, Any]) -> None:
        path = self.evidence / "identity-transcript.json"
        data = _canonical(value)
        _write(path, data, 0o400)
        attestation = json.loads((self.evidence / "identity-attestation.json").read_bytes())
        attestation["archives"]["verify_transcript"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        _write(self.evidence / "identity-attestation.json", _canonical(attestation), 0o400)
        marker = self.marker()
        for label in ("identity_transcript", "verify_transcript"):
            marker["identity_verification"][label].update(
                {"sha256": _digest(data), "size_bytes": len(data)}
            )
        attestation_data = _canonical(attestation)
        marker["identity_verification"]["identity_attestation"].update(
            {"sha256": _digest(attestation_data), "size_bytes": len(attestation_data)}
        )
        self.seal_marker(marker)

    def reseal_raw(self, data: bytes) -> None:
        _write(self.evidence / "identity-raw-commit", data, 0o400)
        attestation = json.loads((self.evidence / "identity-attestation.json").read_bytes())
        attestation["archives"]["raw_commit"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        attestation_data = _canonical(attestation)
        _write(self.evidence / "identity-attestation.json", attestation_data, 0o400)
        marker = self.marker()
        marker["identity_verification"]["raw_commit"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        marker["identity_verification"]["identity_attestation"].update(
            {"sha256": _digest(attestation_data), "size_bytes": len(attestation_data)}
        )
        self.seal_marker(marker)

    def reseal_trusted(self, label: str, data: bytes, mode: int) -> None:
        source = self.trust / f"source-{label}"
        archive = self.evidence / TRUSTED_NAMES[label]
        _write(source, data, mode)
        _write(archive, data, 0o500 if label in {"bash", "git", "python", "ssh_keygen"} else 0o400)
        marker = self.marker()
        record = marker["trusted_inputs"][label]
        record.update(
            {
                "sha256": _digest(data),
                "size_bytes": len(data),
            }
        )
        self.seal_marker(marker)

    def reseal_allowed_policy(self, data: bytes) -> None:
        _write(self.evidence / "bootstrap-allowed-signers", data, 0o400)
        _write(self.trust / "source-allowed_signers", data, 0o400)
        _write(self.evidence / "identity-allowed-signers", data, 0o400)
        attestation = json.loads((self.evidence / "identity-attestation.json").read_bytes())
        attestation["archives"]["ssh_allowed_signers"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        attestation_data = _canonical(attestation)
        _write(self.evidence / "identity-attestation.json", attestation_data, 0o400)
        marker = self.marker()
        marker["trusted_inputs"]["allowed_signers"].update(
            {
                "sha256": _digest(data),
                "size_bytes": len(data),
            }
        )
        marker["identity_verification"]["ssh_allowed_signers"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        marker["identity_verification"]["identity_attestation"].update(
            {"sha256": _digest(attestation_data), "size_bytes": len(attestation_data)}
        )
        self.environment[
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256"
        ] = _digest(data)
        marker_environment = {
            key: value
            for key, value in self.environment.items()
            if key
            not in {
                "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "PWD",
                "SHLVL",
                "_",
                "__CF_USER_TEXT_ENCODING",
            }
        }
        marker["runner"]["environment_sha256"] = _digest(
            _canonical(marker_environment)
        )
        self.seal_marker(marker)

    def run(
        self,
        *,
        checkpoint: str = "entry",
        environment: dict[str, str] | None = None,
        arguments: list[str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        result_path = self.evidence / "release-runner-result.json"
        if checkpoint == "sealed" and result_path.exists():
            invocation_root = (environment or self.environment).get(
                "IROHA_RELEASE_INVOCATION_ROOT"
            )
            assert invocation_root is not None
            sealed_validator = (
                Path(invocation_root)
                / "source"
                / "scripts"
                / "validate_sumeragi_v2_release_bootstrap.py"
            )
        else:
            sealed_validator = (
                self.evidence
                / "release-runner"
                / "source"
                / "scripts"
                / "validate_sumeragi_v2_release_bootstrap.py"
            )
        program = (
            sealed_validator
            if checkpoint == "sealed" and sealed_validator.exists()
            else self.validator
        )
        argv = arguments or [
            str(self.evidence / TRUSTED_NAMES["python"]),
            "-I",
            "-S",
            str(program),
            "--candidate-root",
            str(self.candidate),
            "--runner",
            str(self.runner),
            "--profile",
            "--release",
            "--checkpoint",
            checkpoint,
        ]
        return subprocess.run(
            argv,
            cwd=self.candidate,
            env=environment or self.environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=20,
            check=False,
        )

    def prepare_sealed(self, *, mode: int = 0o700) -> Path:
        release_runner = self.evidence / "release-runner"
        release_runner.mkdir(mode=mode)
        source_scripts = release_runner / "source" / "scripts"
        source_scripts.mkdir(parents=True, mode=0o700)
        _copy(
            self.runner,
            source_scripts / "run_sumeragi_v2_release_gates.sh",
            stat.S_IMODE(self.runner.stat().st_mode),
        )
        _copy(
            self.validator,
            source_scripts / "validate_sumeragi_v2_release_bootstrap.py",
            stat.S_IMODE(self.validator.stat().st_mode),
        )
        return release_runner

    def prepare_retained_sealed(self) -> Path:
        invocation = self.root / "release-invocation"
        invocation.mkdir(mode=0o700)
        self.environment["IROHA_RELEASE_INVOCATION_ROOT"] = str(invocation)
        self.environment["IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"] = str(
            invocation / "fixture" / "scaling-evidence-manifest"
        )
        marker = self.marker()
        marker_environment = {
            key: value
            for key, value in self.environment.items()
            if key
            not in {
                "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "PWD",
                "SHLVL",
                "_",
                "__CF_USER_TEXT_ENCODING",
            }
        }
        marker["runner"]["environment_sha256"] = _digest(
            _canonical(marker_environment)
        )
        self.seal_marker(marker)
        source_scripts = invocation / "source" / "scripts"
        source_scripts.mkdir(parents=True, mode=0o700)
        _copy(
            self.runner,
            source_scripts / "run_sumeragi_v2_release_gates.sh",
            stat.S_IMODE(self.runner.stat().st_mode),
        )
        _copy(
            self.validator,
            source_scripts / "validate_sumeragi_v2_release_bootstrap.py",
            stat.S_IMODE(self.validator.stat().st_mode),
        )
        local = {
            "receipt": invocation / "output" / "release" / "RELEASE_COMPLETED.json",
            "sealed_identity": invocation / "sealed-identity.json",
            "receipt_validation": invocation / "receipt-validation-ack.json",
        }
        protected_names = {
            "receipt": "RELEASE_COMPLETED.json",
            "sealed_identity": "sealed-identity.json",
            "inventory": "release-retained-inventory.json",
            "receipt_validation": "receipt-validation-ack.json",
        }
        validator = self.evidence / "validate-receipt.py"
        completion = self.evidence / "BOOTSTRAP_COMPLETED.json"
        stdout = (
            f"Sumeragi v2 aggregate release receipt verified: {local['receipt']}\n"
        ).encode()
        bootstrap = _load_bootstrap_module()
        invocation_values = _fixture_validator_values(
            bootstrap,
            invocation=invocation,
            evidence=self.evidence,
            source=invocation / "source",
            receipt=local["receipt"],
            acknowledgment=local["receipt_validation"],
            source_manifest_sha256="a" * 64,
        )
        receipt_data = _canonical(
            _fixture_receipt_for_validator(invocation_values)
        )
        payloads = {
            "receipt": receipt_data,
            "sealed_identity": _canonical({"field": "sealed_identity"}),
            "receipt_validation": _canonical({
                "format": "iroha-sumeragi-v2-receipt-validation-ack",
                "schema_version": 3,
                "profile": "release",
                "sealed_source": {
                    "archive_id": "release-retained.source.v1",
                    "manifest_sha256": "a" * 64,
                },
                "receipt": {
                    "archive_id": "release-terminal.receipt.v1",
                    "mode": "0400",
                    "sha256": _digest(receipt_data),
                    "size_bytes": len(receipt_data),
                },
                "validator": {
                    "archive_id": "release-bootstrap.receipt-validator.v1",
                    "sha256": _digest(validator.read_bytes()),
                    "bootstrap_completion_sha256": _digest(completion.read_bytes()),
                },
                "invocation": _fixture_validator_invocation(
                    bootstrap,
                    invocation=invocation,
                    evidence=self.evidence,
                    source=invocation / "source",
                    receipt=local["receipt"],
                    acknowledgment=local["receipt_validation"],
                    source_manifest_sha256="a" * 64,
                ),
                "exit_status": 0,
                "stdout": {
                    "sha256": _digest(stdout),
                    "size_bytes": len(stdout),
                },
                "stderr": {
                    "sha256": _digest(b""),
                    "size_bytes": 0,
                },
            }),
        }
        private_artifacts = {}
        bindings = {}
        archive_ids = {
            "receipt": "release-terminal.receipt.v1",
            "sealed_identity": "release-retained.identity.v1",
            "receipt_validation": "release-retained.receipt-validation-ack.v3",
        }
        for field, path in local.items():
            data = payloads[field]
            _write(path, data, 0o400)
            protected = _write(
                self.evidence / protected_names[field], data, 0o400
            )
            private_artifacts[field] = {
                "path": str(path),
                "protected_path": str(protected),
            }
            bindings[field] = {
                "archive_id": archive_ids[field],
                "mode": "0400",
                "sha256": _digest(data),
                "size_bytes": len(data),
            }
        output = invocation / "output"
        release = output / "release"
        retained_records = [
            {"path": "output", "kind": "directory", "mode": f"{stat.S_IMODE(output.stat().st_mode):04o}"},
            {"path": "output/release", "kind": "directory", "mode": f"{stat.S_IMODE(release.stat().st_mode):04o}"},
            {"path": "output/release/RELEASE_COMPLETED.json", "kind": "file", "mode": "0400", "size": local["receipt"].stat().st_size, "sha256": _digest(local["receipt"].read_bytes())},
            {"path": "receipt-validation-ack.json", "kind": "file", "mode": "0400", "size": local["receipt_validation"].stat().st_size, "sha256": _digest(local["receipt_validation"].read_bytes())},
            {"path": "sealed-identity.json", "kind": "file", "mode": "0400", "size": local["sealed_identity"].stat().st_size, "sha256": _digest(local["sealed_identity"].read_bytes())},
        ]
        inventory_data = _canonical({
            "format": "iroha-sumeragi-v2-retained-release-evidence",
            "schema_version": 2,
            "invocation_archive_id": "release-retained.invocation.v1",
            "source_archive_id": "release-retained.source.v1",
            "source_manifest_sha256": "a" * 64,
            "record_count": len(retained_records),
            "file_bytes": sum(record.get("size", 0) for record in retained_records),
            "records": retained_records,
        })
        inventory_path = invocation / "retained-evidence-inventory.json"
        _write(inventory_path, inventory_data, 0o400)
        protected_inventory = _write(
            self.evidence / protected_names["inventory"], inventory_data, 0o400
        )
        private_artifacts["inventory"] = {
            "path": str(inventory_path),
            "protected_path": str(protected_inventory),
        }
        bindings["inventory"] = {
            "archive_id": "release-retained.inventory.v2",
            "mode": "0400",
            "sha256": _digest(inventory_data),
            "size_bytes": len(inventory_data),
        }
        result = {
            "format": "iroha-sumeragi-v2-retained-release-evidence",
            "schema_version": 2,
            "invocation_archive_id": "release-retained.invocation.v1",
            "source_archive_id": "release-retained.source.v1",
            "source_manifest_sha256": "a" * 64,
            **bindings,
        }
        _write(
            self.evidence / "release-runner-result.json",
            _canonical(result),
            0o400,
        )
        return invocation

    def publish_private_retained_provenance(self, invocation: Path) -> None:
        """Publish the bootstrap-private locator used only for terminal replay."""

        local = {
            "receipt": invocation / "output" / "release" / "RELEASE_COMPLETED.json",
            "sealed_identity": invocation / "sealed-identity.json",
            "inventory": invocation / "retained-evidence-inventory.json",
            "receipt_validation": invocation / "receipt-validation-ack.json",
        }
        protected_names = {
            "receipt": "RELEASE_COMPLETED.json",
            "sealed_identity": "sealed-identity.json",
            "inventory": "release-retained-inventory.json",
            "receipt_validation": "receipt-validation-ack.json",
        }
        _write(
            self.evidence / "release-runner-private-provenance.json",
            _canonical({
                "format": "iroha-sumeragi-v2-bootstrap-private-retained-provenance",
                "schema_version": 1,
                "invocation_root": str(invocation),
                "source_root": str(invocation / "source"),
                "artifacts": {
                    field: {
                        "path": str(path),
                        "protected_path": str(self.evidence / protected_names[field]),
                    }
                    for field, path in local.items()
                },
            }),
            0o400,
        )


@pytest.fixture
def release_fixture(tmp_path: Path) -> Fixture:
    root = tmp_path.resolve(strict=True)
    root.chmod(0o700)
    candidate = root / "candidate"
    trust = root / "trust"
    evidence = root / "evidence"
    candidate.mkdir(mode=0o700)
    trust.mkdir(mode=0o700)
    evidence.mkdir(mode=0o700)
    (evidence / "home").mkdir(mode=0o700)
    (evidence / "tmp").mkdir(mode=0o700)
    (evidence / "runner-bin").mkdir(mode=0o700)
    (evidence / "runner-tools").mkdir(mode=0o700)
    _write(evidence / "runner-stdout.log", b"", 0o600)
    _write(evidence / "runner-stderr.log", b"", 0o600)

    runner = _write(
        candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        "#!/bin/bash\nexit 0\n",
        0o755,
    )
    validator = _copy(
        VALIDATOR,
        candidate / "scripts" / "validate_sumeragi_v2_release_bootstrap.py",
        0o755,
    )
    _write(candidate / "Cargo.lock", b"locked\n", 0o600)
    _write(candidate / "payload", b"candidate\n", 0o600)
    _write(candidate / "HEAD_ID", "0" * 40 + "\n", 0o600)

    runner_tool_sources = {
        name: _write(
            trust / f"runner-tool-{name}",
            f"synthetic fixture tool {name}\n".encode(),
            0o500,
        )
        for name in REQUIRED_RUNNER_TOOL_NAMES
    }
    runner_tool_manifest = _canonical(
        {
            "schema_version": 1,
            "tools": {
                name: {
                    "path": str(source),
                    "sha256": _digest(source.read_bytes()),
                }
                for name, source in sorted(runner_tool_sources.items())
            },
        }
    )
    runner_tool_archives = {
        name: _copy(
            source, evidence / "runner-tools" / name, 0o500
        )
        for name, source in sorted(runner_tool_sources.items())
    }
    for name in REQUIRED_RUNNER_TOOL_NAMES:
        (evidence / "runner-bin" / name).symlink_to(f"../runner-tools/{name}")
    source_data = {
        "allowed_signers": b'release namespaces="git" ssh-ed25519 AAAATEST\n',
        "bash": b"synthetic relocatable bash",
        "bootstrap": b"synthetic protected bootstrap",
        "git": b"synthetic relocatable git",
        "identity_verifier": b"synthetic protected verifier",
        "manifest_helper": _manifest_helper().encode(),
        "receipt_validator": b"synthetic protected receipt validator",
        "receipt_validator_support": b"synthetic receipt validator support",
        "runtime_helper": b"synthetic protected runtime helper",
        "tool_probe_helper": fixture_tool_probe_helper(),
        "approval_contract": APPROVAL_CONTRACT.read_bytes(),
        "sdk_dependency_bundle_manifest": _canonical({
            "format": "iroha-sumeragi-v2-sdk-dependency-sources",
            "schema_version": 1,
            "fixture": "bootstrap-private",
        }),
        "revocation": b"",
        "runner_tool_manifest": runner_tool_manifest,
        "ssh_keygen": b"synthetic relocatable ssh-keygen",
    }
    sources: dict[str, Path] = {}
    archives: dict[str, Path] = {}
    for label, data in source_data.items():
        mode = 0o500 if label in {"bash", "git", "ssh_keygen"} else 0o400
        sources[label] = _write(trust / f"source-{label}", data, mode)
        archives[label] = _write(evidence / TRUSTED_NAMES[label], data, mode)
    sources["python"] = _copy(PYTHON, trust / "source-python", 0o500)
    framework_runtime_record: dict[str, Any] | None = None
    if FRAMEWORK_PYTHON:
        framework_inventory = evidence / "python-runtime-input.json"
        result = subprocess.run(
            [
                str(PYTHON),
                "-I",
                "-S",
                str(RUNTIME_HELPER),
                "--copy-framework-python",
                "--runtime-root",
                str(evidence / "python-runtime"),
                "--runtime-inventory",
                str(framework_inventory),
            ],
            cwd=evidence,
            env={"LANG": "C", "LC_ALL": "C", "PATH": os.defpath},
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert result.returncode == 0, result.stderr.decode("utf-8", "replace")
        assert result.stdout == b""
        assert result.stderr == b""
        archives["python"] = evidence / TRUSTED_NAMES["python"]
        framework_runtime_record = _framework_python_runtime_record(
            framework_inventory
        )
    else:
        archives["python"] = _copy(
            sources["python"], evidence / TRUSTED_NAMES["python"], 0o500
        )
        _provision_archived_python_runtime(PYTHON, archives["python"])

    tool_probe_manifest_value = {
        "schema_version": 1,
        "tools": {
            name: {
                "archive_id": f"release-runner-tool.{name}.v1",
                "path": str(runner_tool_archives[name]),
                "sha256": _digest(runner_tool_archives[name].read_bytes()),
            }
            for name in REQUIRED_RUNNER_TOOL_NAMES
        },
    }
    tool_probe_manifest = _write(
        evidence / "runner-tool-probe-manifest.json",
        _canonical(tool_probe_manifest_value),
        0o400,
    )
    tool_probe_run = subprocess.run(
        [
            str(archives["python"]),
            "-I",
            "-S",
            str(archives["tool_probe_helper"]),
            "--tool-manifest",
            str(tool_probe_manifest),
            "--expected-tool-manifest-sha256",
            _digest(tool_probe_manifest.read_bytes()),
            "--probe-root",
            str(evidence / ".fixture-tool-probe"),
        ],
        cwd=evidence,
        env={"LANG": "C", "LC_ALL": "C", "PATH": str(evidence)},
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert tool_probe_run.returncode == 0, tool_probe_run.stderr.decode(
        "utf-8", "replace"
    )
    assert tool_probe_run.stderr == b""
    tool_probe_result = _write(
        evidence / "runner-tool-probes.json", tool_probe_run.stdout, 0o400
    )
    tool_probe_value = json.loads(tool_probe_run.stdout)

    first_identity = _identity(candidate)
    raw = _raw_commit(
        first_identity["head_tree"],
        first_identity["workspace_source_manifest_sha256"],
        first_identity["cargo_lock_sha256"],
    )
    _write(candidate / "HEAD_ID", _oid(raw) + "\n", 0o600)
    identity = _identity(candidate)
    identity_bytes = _canonical(identity)
    _write(evidence / "candidate-identity.json", identity_bytes, 0o400)
    approval_module = _load_approval_component(archives["approval_contract"])
    approval_archives, release_approvals = _approval_fixture(
        module=approval_module,
        identity=identity,
        tool_manifest_sha256=_digest(archives["runner_tool_manifest"].read_bytes()),
        trust=trust,
        evidence=evidence,
    )
    archives.update(approval_archives)

    identity_data = {
        "cargo_lock": (candidate / "Cargo.lock").read_bytes(),
        "git": archives["git"].read_bytes(),
        "raw_commit": raw,
        "ssh_allowed_signers": archives["allowed_signers"].read_bytes(),
        "ssh_keygen": archives["ssh_keygen"].read_bytes(),
        "ssh_revocation": archives["revocation"].read_bytes(),
    }
    for label, data in identity_data.items():
        mode = 0o500 if label in {"git", "ssh_keygen"} else 0o400
        _write(evidence / IDENTITY_NAMES[label], data, mode)

    tools = {
        "git": {
            **_protected(identity_data["git"], IDENTITY_NAMES["git"], 0o500),
            "source_path": str(archives["git"]),
        },
        "ssh_keygen": {
            **_protected(identity_data["ssh_keygen"], IDENTITY_NAMES["ssh_keygen"], 0o500),
            "source_path": str(archives["ssh_keygen"]),
        },
    }
    policies = {
        "expected_signer_fingerprint": FINGERPRINT,
        "signature_format": "ssh",
        "ssh_allowed_signers": _protected(
            identity_data["ssh_allowed_signers"], IDENTITY_NAMES["ssh_allowed_signers"], 0o400
        ),
        "ssh_revocation": _protected(
            identity_data["ssh_revocation"], IDENTITY_NAMES["ssh_revocation"], 0o400
        ),
    }
    transcript = {
        "format": "iroha-sumeragi-v2-release-identity-transcript",
        "schema_version": 3,
        "archive_ids": IDENTITY_IDS,
        "candidate_commit_oid": identity["head_commit"],
        "operations": {
            "show_signature_metadata": {
                "operation_id": "git.show-signature-metadata.ssh.v1",
                "exit_status": 0,
                "stdout_sha256": _digest(b"metadata"),
                "stdout_size_bytes": len(b"metadata"),
                "stderr_sha256": _digest(b""),
                "stderr_size_bytes": 0,
            },
            "verify_commit": {
                "operation_id": "git.verify-commit.ssh.v1",
                "exit_status": 0,
                "stdout_sha256": _digest(b""),
                "stdout_size_bytes": 0,
                "stderr_sha256": _digest(b""),
                "stderr_size_bytes": 0,
            },
            "ssh_keygen_usage": {
                "operation_id": "ssh-keygen.usage-probe.v1",
                "exit_status": 1,
                "stdout_sha256": _digest(b""),
                "stdout_size_bytes": 0,
                "stderr_sha256": _digest(b"usage"),
                "stderr_size_bytes": len(b"usage"),
            },
        },
    }
    transcript_bytes = _canonical(transcript)
    _write(evidence / "identity-transcript.json", transcript_bytes, 0o400)
    evidence_records = {
        label: {
            "archive_id": IDENTITY_IDS[label],
            "mode": f"{0o500 if label in {'git', 'ssh_keygen'} else 0o400:04o}",
            "sha256": _digest(
                transcript_bytes
                if label == "verify_transcript"
                else identity_data[label]
            ),
            "size_bytes": len(
                transcript_bytes
                if label == "verify_transcript"
                else identity_data[label]
            ),
        }
        for label in IDENTITY_NAMES
    }
    attestation = {
        "format": "iroha-sumeragi-v2-release-identity-attestation",
        "schema_version": 3,
        "candidate": {
            "commit_oid": identity["head_commit"],
            "tree_oid": identity["head_tree"],
            "source_manifest_sha256": identity[
                "workspace_source_manifest_sha256"
            ],
            "cargo_lock_sha256": identity["cargo_lock_sha256"],
            "release_identity_sha256": _digest(identity_bytes),
        },
        "archives": evidence_records,
    }
    attestation_bytes = _canonical(attestation)
    _write(evidence / "identity-attestation.json", attestation_bytes, 0o400)

    trusted_records: dict[str, Any] = {}
    for label in sorted(TRUSTED_NAMES):
        archive = archives[label]
        trusted_records[label] = {
            "archive_id": f"release-bootstrap.{label.replace('_', '-')}.v1",
            "archive_name": TRUSTED_NAMES[label],
            "mode": f"{stat.S_IMODE(archive.stat().st_mode):04o}",
            "sha256": _digest(archive.read_bytes()),
            "size_bytes": archive.stat().st_size,
        }
    if framework_runtime_record is not None:
        trusted_records["python"]["runtime"] = framework_runtime_record
    for label, sources, archive_id in (
        (
            "bootstrap",
            BOOTSTRAP_COMPONENT_FILES,
            "release-bootstrap.bootstrap-component.v1:",
        ),
        (
            "receipt_validator",
            RECEIPT_VALIDATOR_COMPONENT_FILES,
            "release-bootstrap.receipt-validator-component.v1:",
        ),
    ):
        components: dict[str, Any] = {}
        for source in sources:
            archive = _write(evidence / source.name, source.read_bytes(), 0o400)
            components[source.name] = {
                "archive_id": archive_id + source.name,
                "archive_name": source.name,
                "mode": "0400",
                "sha256": _digest(archive.read_bytes()),
                "size_bytes": archive.stat().st_size,
            }
        trusted_records[label]["components"] = components
    identity_records = {
        "identity_attestation": _artifact(attestation_bytes, "identity-attestation.json", 0o400),
        "identity_transcript": _artifact(transcript_bytes, "identity-transcript.json", 0o400),
        **{
            label: _artifact(
                transcript_bytes if label == "verify_transcript" else identity_data[label],
                name,
                0o500 if label in {"git", "ssh_keygen"} else 0o400,
            )
            for label, name in IDENTITY_NAMES.items()
        },
    }
    policy_environment = {
        "SUMERAGI_V2_RELEASE_RUNTIME_HELPER": str(archives["runtime_helper"]),
        "SUMERAGI_V2_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": _digest(archives["runtime_helper"].read_bytes()),
        "SUMERAGI_V2_RELEASE_TOOL_PROBE_HELPER": str(
            archives["tool_probe_helper"]
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": _digest(
            archives["tool_probe_helper"].read_bytes()
        ),
        "SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN": str(archives["ssh_keygen"]),
        "SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256": _digest(archives["git"].read_bytes()),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256": _digest(archives["ssh_keygen"].read_bytes()),
        "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT": FINGERPRINT,
        "SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS": str(archives["allowed_signers"]),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256": _digest(archives["allowed_signers"].read_bytes()),
        "SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE": str(archives["revocation"]),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256": _digest(archives["revocation"].read_bytes()),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION": str(evidence / "BOOTSTRAP_COMPLETED.json"),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION": str(evidence / "identity-attestation.json"),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT": str(evidence / "identity-transcript.json"),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY": str(evidence / "candidate-identity.json"),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR": str(evidence),
    }
    aliases = {
        key.replace("SUMERAGI_V2_RELEASE_", "IROHA_RELEASE_", 1): value
        for key, value in policy_environment.items()
        if key.startswith("SUMERAGI_V2_RELEASE_BOOTSTRAP_")
    }
    aliases.update({
        "IROHA_RELEASE_RUNTIME_HELPER": str(archives["runtime_helper"]),
        "IROHA_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": _digest(archives["runtime_helper"].read_bytes()),
        "IROHA_RELEASE_TOOL_PROBE_HELPER": str(
            archives["tool_probe_helper"]
        ),
        "IROHA_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": _digest(
            archives["tool_probe_helper"].read_bytes()
        ),
        "IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST": str(
            archives["sdk_dependency_bundle_manifest"]
        ),
        "IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256": _digest(
            archives["sdk_dependency_bundle_manifest"].read_bytes()
        ),
    })
    closed_environment = {
        "HOME": str(evidence / "home"),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.pathsep.join([
            str(evidence),
            *(
                [str(archives["python"].parent)]
                if FRAMEWORK_PYTHON
                else []
            ),
            str(evidence / "runner-bin"),
        ]),
        "TMPDIR": str(evidence / "tmp"),
        "TZ": "UTC",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_COUNT": "2",
        "GIT_CONFIG_KEY_0": "core.hooksPath",
        "GIT_CONFIG_VALUE_0": os.devnull,
        "GIT_CONFIG_KEY_1": "core.fsmonitor",
        "GIT_CONFIG_VALUE_1": "false",
        "GIT_TERMINAL_PROMPT": "0",
        **policy_environment,
        **aliases,
    }
    marker = {
        "schema_version": 2,
        "trust_boundary": {
            "bootstrap_authentication": "external prerequisite",
            "release_image_and_dynamic_loader": "external prerequisite",
            "same_uid_and_trusted_ancestor_owners": True,
        },
        "candidate_identity": identity,
        "candidate_identity_sha256": _digest(identity_bytes),
        "trusted_inputs": trusted_records,
        "release_approvals": release_approvals,
        "identity_verification": identity_records,
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
            "environment_sha256": _digest(_canonical(closed_environment)),
            "mode": f"{stat.S_IMODE(runner.stat().st_mode):04o}",
            "output": {
                "stderr_archive_id": "release-bootstrap.runner-stderr.v1",
                "stderr_name": "runner-stderr.log",
                "stdout_archive_id": "release-bootstrap.runner-stdout.v1",
                "stdout_name": "runner-stdout.log",
                "active_mode": "0600",
                "sealed_mode": "0400",
            },
            "tool_directory": "runner-bin",
            "tools": {
                name: {
                    "archive_id": f"release-runner-tool.{name}.v1",
                    "alias_name": name,
                    "archive_name": f"runner-tools/{name}",
                    "mode": "0500",
                    "sha256": _digest(runner_tool_archives[name].read_bytes()),
                    "size_bytes": runner_tool_archives[name].stat().st_size,
                }
                for name in REQUIRED_RUNNER_TOOL_NAMES
            },
            "self_digest_environment_variables": [
                "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            ],
            "sha256": _digest(runner.read_bytes()),
            "size_bytes": runner.stat().st_size,
        },
        "trusted_execution_probes": {
            "bash": {"argv": [str(archives["bash"]), "-c", ":"], "exit_status": 0},
            "python": {
                "argv": [
                    str(archives["python"]),
                    "-I",
                    "-S",
                    "-c",
                    "import sys;sys.stdout.write(sys.executable+'\\n')",
                ],
                "expected_executable": TRUSTED_NAMES["python"],
                "exit_status": 0,
                "stdout_sha256": _digest(
                    f"{archives['python']}\n".encode()
                ),
                "stdout_size_bytes": len(f"{archives['python']}\n".encode()),
            },
            "runner_tool_closure": {
                "manifest": {
                    "archive_id": (
                        "release-bootstrap.runner-tool-probe-manifest.v1"
                    ),
                    **_artifact(
                        tool_probe_manifest.read_bytes(),
                        tool_probe_manifest.name,
                        0o400,
                    ),
                },
                "result": {
                    "archive_id": "release-bootstrap.runner-tool-probes.v1",
                    **_artifact(
                        tool_probe_result.read_bytes(),
                        tool_probe_result.name,
                        0o400,
                    ),
                },
                "value": tool_probe_value,
            },
        },
    }
    marker_bytes = _canonical(marker)
    _write(evidence / "BOOTSTRAP_COMPLETED.json", marker_bytes, 0o400)
    marker_digest = _digest(marker_bytes)
    environment = {
        **closed_environment,
        "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": marker_digest,
        "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": marker_digest,
        "PWD": str(candidate),
        "SHLVL": "1",
        "_": str(archives["python"]),
    }
    if sys.platform == "darwin":
        environment["__CF_USER_TEXT_ENCODING"] = f"0x{os.geteuid():X}:0x1:0xE"
    return Fixture(root, candidate, trust, evidence, runner, validator, environment)


def _assert_rejected(result: subprocess.CompletedProcess[str]) -> None:
    assert result.returncode == 2, result
    assert "bootstrap validation failed" in result.stderr


def test_entry_accepts_exact_authenticated_contract(release_fixture: Fixture) -> None:
    result = release_fixture.run()
    assert result.returncode == 0, result.stderr
    assert result.stdout == ""
    assert result.stderr == ""
    marker = release_fixture.marker()
    approvals = marker["release_approvals"]
    assert set(approvals["class_attestations"]) == set(APPROVAL_CLASS_IDS)
    public_bytes = _canonical(approvals)
    assert str(release_fixture.trust).encode() not in public_bytes
    assert b'"arguments"' not in public_bytes
    raw = release_fixture.evidence / TRUSTED_NAMES[
        "approval_offline_toolchain_sdk"
    ]
    raw.chmod(0o600)
    _assert_rejected(release_fixture.run())


def test_entry_rejects_legacy_arbitrary_path_entries(release_fixture: Fixture) -> None:
    marker = release_fixture.marker()
    marker["runner"]["path_entries"] = [
        str(release_fixture.trust),
        str(release_fixture.trust),
    ]
    release_fixture.seal_marker(marker)
    _assert_rejected(release_fixture.run())


def test_sealed_checkpoint_accepts_private_runner_subtree_and_ambient_changes(
    release_fixture: Fixture,
) -> None:
    invocation = release_fixture.prepare_retained_sealed()
    environment = {**release_fixture.environment, "BASH_ENV": "/untrusted", "RUSTFLAGS": "poison"}
    result = release_fixture.run(checkpoint="sealed", environment=environment)
    assert result.returncode == 0, result.stderr
    release_fixture.publish_private_retained_provenance(invocation)
    bootstrap_spec = importlib.util.spec_from_file_location(
        "release_bootstrap_retained_contract",
        REPO_ROOT / "scripts" / "bootstrap_sumeragi_v2_release.py",
    )
    assert bootstrap_spec is not None and bootstrap_spec.loader is not None
    bootstrap = importlib.util.module_from_spec(bootstrap_spec)
    sys.modules[bootstrap_spec.name] = bootstrap
    bootstrap_spec.loader.exec_module(bootstrap)
    evidence_fd = os.open(
        release_fixture.evidence,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0),
    )
    try:
        retained = bootstrap._retained_release_layout(
            release_fixture.evidence,
            evidence_fd,
            candidate=release_fixture.candidate,
            authenticated_environment=release_fixture.environment,
        )
    finally:
        os.close(evidence_fd)
    assert retained[:3] == (
        invocation,
        release_fixture.evidence / "RELEASE_COMPLETED.json",
        release_fixture.evidence / "sealed-identity.json",
    )
    local_acknowledgment = invocation / "receipt-validation-ack.json"
    invocation_record = json.loads(local_acknowledgment.read_bytes())["invocation"]
    expected_invocation_values = _fixture_validator_values(
        bootstrap,
        invocation=invocation,
        evidence=release_fixture.evidence,
        source=invocation / "source",
        receipt=invocation / "output" / "release" / "RELEASE_COMPLETED.json",
        acknowledgment=local_acknowledgment,
        source_manifest_sha256="a" * 64,
    )
    bootstrap._validate_validator_invocation(
        invocation_record, expected_values=expected_invocation_values
    )
    changed_value = json.loads(json.dumps(invocation_record))
    previously_unchecked = next(
        binding
        for binding in changed_value["ordered_options"]
        if binding["name"] == "--signature-transcript"
    )
    previously_unchecked["normalized_value_sha256"] = hashlib.sha256(
        json.dumps(
            {"kind": "path", "value": str(invocation / "attacker-transcript")},
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    ).hexdigest()
    changed_core = {
        key: changed_value[key]
        for key in (
            "profile", "operation", "python_flags", "validator", "ordered_options"
        )
    }
    changed_value["invocation_sha256"] = _digest(
        json.dumps(
            changed_core, sort_keys=True, separators=(",", ":")
        ).encode()
    )
    with pytest.raises(
        bootstrap.BootstrapError, match="normalized option value"
    ):
        bootstrap._validate_validator_invocation(
            changed_value, expected_values=expected_invocation_values
        )
    incomplete_values = dict(expected_invocation_values)
    incomplete_values.pop("--signature-transcript")
    with pytest.raises(
        bootstrap.BootstrapError, match="reconstruction is incomplete"
    ):
        bootstrap._validate_validator_invocation(
            invocation_record, expected_values=incomplete_values
        )
    changed_digest = json.loads(json.dumps(invocation_record))
    changed_digest["invocation_sha256"] = "c" * 64
    with pytest.raises(bootstrap.BootstrapError, match="invocation digest"):
        bootstrap._validate_validator_invocation(
            changed_digest, expected_values=expected_invocation_values
        )
    local_inventory = invocation / "retained-evidence-inventory.json"
    protected_inventory = release_fixture.evidence / "release-retained-inventory.json"
    result_path = release_fixture.evidence / "release-runner-result.json"
    original_inventory = local_inventory.read_bytes()
    original_result = result_path.read_bytes()
    numeric_alias = json.loads(original_inventory)
    numeric_alias["record_count"] = True
    changed_inventory = _canonical(numeric_alias)
    _write(local_inventory, changed_inventory, 0o400)
    _write(protected_inventory, changed_inventory, 0o400)
    changed_result = json.loads(original_result)
    changed_result["inventory"].update({
        "sha256": _digest(changed_inventory),
        "size_bytes": len(changed_inventory),
    })
    _write(result_path, _canonical(changed_result), 0o400)
    release_fixture.publish_private_retained_provenance(invocation)
    evidence_fd = os.open(
        release_fixture.evidence,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0),
    )
    try:
        with pytest.raises(bootstrap.BootstrapError, match="inventory contract"):
            bootstrap._retained_release_layout(
                release_fixture.evidence,
                evidence_fd,
                candidate=release_fixture.candidate,
                authenticated_environment=release_fixture.environment,
            )
    finally:
        os.close(evidence_fd)
    _write(local_inventory, original_inventory, 0o400)
    _write(protected_inventory, original_inventory, 0o400)
    _write(result_path, original_result, 0o400)
    acknowledgment = release_fixture.evidence / "receipt-validation-ack.json"
    _write(acknowledgment, b"drifted acknowledgment\n", 0o400)
    _assert_rejected(
        release_fixture.run(checkpoint="sealed", environment=environment)
    )


def test_entry_rejects_runner_subtree_before_phase_transition(release_fixture: Fixture) -> None:
    release_fixture.prepare_sealed()
    _assert_rejected(release_fixture.run())


def test_sealed_requires_exact_private_runner_subtree(release_fixture: Fixture) -> None:
    _assert_rejected(release_fixture.run(checkpoint="sealed"))
    (release_fixture.evidence / "release-runner").mkdir(mode=0o755)
    _assert_rejected(release_fixture.run(checkpoint="sealed"))


def test_sealed_rejects_other_top_level_evidence(release_fixture: Fixture) -> None:
    release_fixture.prepare_sealed()
    _write(release_fixture.evidence / "intruder", b"unexpected", 0o400)
    _assert_rejected(release_fixture.run(checkpoint="sealed"))


def test_sealed_rejects_drifted_runner_owned_validator_copy(
    release_fixture: Fixture,
) -> None:
    release_fixture.prepare_sealed()
    sealed_validator = (
        release_fixture.evidence
        / "release-runner"
        / "source"
        / "scripts"
        / "validate_sumeragi_v2_release_bootstrap.py"
    )
    _write(sealed_validator, sealed_validator.read_bytes() + b"\n# drift\n", 0o755)
    _assert_rejected(release_fixture.run(checkpoint="sealed"))


@pytest.mark.parametrize(
    "field,value",
    [
        (("schema_version",), True),
        (
            (
                "release_approvals",
                "expected_duration_seconds",
                "formal-proof-tools",
            ),
            True,
        ),
        (("trusted_inputs", "git", "size_bytes"), 1.0),
    ],
)
def test_marker_rejects_bool_and_float_as_integers(
    release_fixture: Fixture, field: tuple[str, ...], value: Any
) -> None:
    marker = release_fixture.marker()
    target: dict[str, Any] = marker
    for name in field[:-1]:
        target = target[name]
    target[field[-1]] = value
    release_fixture.seal_marker(marker)
    _assert_rejected(release_fixture.run())


def test_noncanonical_authenticated_marker_is_rejected(release_fixture: Fixture) -> None:
    release_fixture.seal_marker(release_fixture.marker(), canonical=False)
    _assert_rejected(release_fixture.run())


def test_out_of_band_marker_digest_mismatch_is_rejected(release_fixture: Fixture) -> None:
    release_fixture.environment["IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"] = "0" * 64
    release_fixture.environment["SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"] = "0" * 64
    _assert_rejected(release_fixture.run())


def test_unequal_path_aliases_are_rejected(release_fixture: Fixture) -> None:
    release_fixture.environment["IROHA_RELEASE_BOOTSTRAP_IDENTITY"] += ".other"
    _assert_rejected(release_fixture.run())


def test_entry_rejects_ambient_environment_poisoning(release_fixture: Fixture) -> None:
    environment = {**release_fixture.environment, "BASH_ENV": "/untrusted"}
    _assert_rejected(release_fixture.run(environment=environment))


@pytest.mark.parametrize("name", ["git", "bootstrap-allowed-signers", "candidate-identity.json"])
def test_archived_content_substitution_is_rejected(release_fixture: Fixture, name: str) -> None:
    _write(release_fixture.evidence / name, b"substituted", 0o500 if name == "git" else 0o400)
    _assert_rejected(release_fixture.run())


def test_archived_mode_substitution_is_rejected(release_fixture: Fixture) -> None:
    (release_fixture.evidence / "git").chmod(0o700)
    _assert_rejected(release_fixture.run())


def test_archived_hardlink_is_rejected(release_fixture: Fixture) -> None:
    os.link(release_fixture.evidence / "git", release_fixture.root / "second-git-link")
    _assert_rejected(release_fixture.run())


def test_archived_symlink_is_rejected(release_fixture: Fixture) -> None:
    target = release_fixture.evidence / "git"
    saved = release_fixture.root / "saved-git"
    target.rename(saved)
    target.symlink_to(saved)
    _assert_rejected(release_fixture.run())


def test_marker_hardlink_is_rejected(release_fixture: Fixture) -> None:
    os.link(release_fixture.marker_path, release_fixture.root / "second-marker-link")
    _assert_rejected(release_fixture.run())


def test_private_evidence_mode_is_enforced(release_fixture: Fixture) -> None:
    release_fixture.evidence.chmod(0o755)
    _assert_rejected(release_fixture.run())


def test_private_source_drift_is_not_disclosed_to_child_validator(
    release_fixture: Fixture,
) -> None:
    source = release_fixture.trust / "source-git"
    _write(source, b"drift", 0o500)
    result = release_fixture.run()
    assert result.returncode == 0, result.stderr


def test_external_source_may_have_multiple_links(release_fixture: Fixture) -> None:
    os.link(release_fixture.trust / "source-git", release_fixture.root / "source-git-link")
    result = release_fixture.run()
    assert result.returncode == 0, result.stderr


def test_current_candidate_identity_drift_is_rejected(release_fixture: Fixture) -> None:
    _write(release_fixture.candidate / "payload", b"drift", 0o600)
    _assert_rejected(release_fixture.run())


def test_current_runner_drift_is_rejected(release_fixture: Fixture) -> None:
    _write(release_fixture.runner, "#!/bin/bash\nexit 9\n", 0o755)
    _assert_rejected(release_fixture.run())


def test_authenticated_wrong_runner_argv_is_rejected(release_fixture: Fixture) -> None:
    marker = release_fixture.marker()
    marker["runner"]["invocation"]["arguments"][-1] = "--pr"
    release_fixture.seal_marker(marker)
    _assert_rejected(release_fixture.run())


@pytest.mark.parametrize(
    "mutation",
    [
        lambda raw: raw.replace(b"Synthetic signed release", b"Synthetic altered release"),
        lambda raw: raw.replace(b"tree ", b"tree 0", 1),
        lambda raw: raw.replace(b"BEGIN SSH SIGNATURE", b"BEGIN PGP SIGNATURE", 1),
    ],
    ids=("oid", "tree", "signature"),
)
def test_authenticated_raw_commit_semantic_corruption_is_rejected(
    release_fixture: Fixture, mutation: Any
) -> None:
    raw = (release_fixture.evidence / "identity-raw-commit").read_bytes()
    release_fixture.reseal_raw(mutation(raw))
    _assert_rejected(release_fixture.run())


def test_authenticated_signer_binding_corruption_is_rejected(release_fixture: Fixture) -> None:
    attestation = json.loads((release_fixture.evidence / "identity-attestation.json").read_bytes())
    attestation["signer_fingerprint"] = OTHER_FINGERPRINT
    release_fixture.reseal_attestation(attestation)
    _assert_rejected(release_fixture.run())


def test_transcript_rejects_bool_as_command_status(release_fixture: Fixture) -> None:
    transcript = json.loads((release_fixture.evidence / "identity-transcript.json").read_bytes())
    transcript["operations"]["verify_commit"]["exit_status"] = True
    release_fixture.reseal_transcript(transcript)
    _assert_rejected(release_fixture.run())


@pytest.mark.parametrize("option", ["valid-after=\"20260717Z\"", "valid-before=\"20270717Z\""])
def test_time_bounded_allowed_signer_policy_is_rejected(
    release_fixture: Fixture, option: str
) -> None:
    data = f'release namespaces="git",{option} ssh-ed25519 AAAATEST\n'.encode()
    release_fixture.reseal_allowed_policy(data)
    _assert_rejected(release_fixture.run())


def test_multiple_active_allowed_signers_are_rejected(release_fixture: Fixture) -> None:
    data = (
        b'release namespaces="git" ssh-ed25519 AAAATEST\n'
        b'backup namespaces="git" ssh-ed25519 AAAABACKUP\n'
    )
    release_fixture.reseal_allowed_policy(data)
    _assert_rejected(release_fixture.run())


def test_same_byte_runner_rewrite_during_validation_is_rejected(
    release_fixture: Fixture,
) -> None:
    helper = _manifest_helper().replace(
        "root = args.root\n",
        "root = args.root\n"
        "runner = root / 'scripts' / 'run_sumeragi_v2_release_gates.sh'\n"
        "runner.write_bytes(runner.read_bytes())\n",
    ).encode()
    release_fixture.reseal_trusted("manifest_helper", helper, 0o400)
    _assert_rejected(release_fixture.run())


def test_missing_literal_profile_contract_is_rejected(release_fixture: Fixture) -> None:
    arguments = [
        str(release_fixture.evidence / TRUSTED_NAMES["python"]),
        "-I",
        "-S",
        str(release_fixture.validator),
        "--candidate-root",
        str(release_fixture.candidate),
        "--runner",
        str(release_fixture.runner),
        "--release",
    ]
    result = release_fixture.run(arguments=arguments)
    assert result.returncode != 0
