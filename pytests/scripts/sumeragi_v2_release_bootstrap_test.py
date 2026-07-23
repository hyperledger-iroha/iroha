"""Adversarial tests for the externally trusted Sumeragi v2 bootstrap."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import time

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
BOOTSTRAP = REPO_ROOT / "scripts" / "bootstrap_sumeragi_v2_release.py"
PYTHON = Path(sys.executable).resolve(strict=True)
FINGERPRINT = "SHA256:" + "A" * 43


def test_outer_abort_grace_exceeds_nested_tlaps_cleanup_window() -> None:
    bootstrap_source = BOOTSTRAP.read_text(encoding="utf-8")
    guard_source = (
        REPO_ROOT / "scripts" / "formal" / "run_sumeragi_v2_tlapm_guard.py"
    ).read_text(encoding="utf-8")
    outer = re.search(
        r"^_RUNNER_ABORT_TERM_GRACE_SECONDS\s*=\s*(\d+)$",
        bootstrap_source,
        re.MULTILINE,
    )
    inner = re.search(
        r"^TERM_GRACE_SECONDS\s*=\s*([0-9.]+)$", guard_source, re.MULTILINE
    )
    assert outer is not None and inner is not None
    # The nested guard has a TERM wait, two child wait/reap windows, and process
    # snapshot overhead. The outer group must not SIGKILL that guard mid-cleanup.
    assert int(outer.group(1)) >= 3 * float(inner.group(1)) + 10


def test_release_runner_defers_launch_signal_until_process_is_owned(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    spec = importlib.util.spec_from_file_location("sumeragi_release_bootstrap", BOOTSTRAP)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    spawned: list[object] = []
    aborted: list[object] = []

    class FakeProcess:
        pid = 424242

        def __init__(self, *_args: object, **_kwargs: object) -> None:
            spawned.append(self)
            os.kill(os.getpid(), signal.SIGTERM)

        def wait(self) -> int:
            raise AssertionError("interrupted launch must abort before waiting")

    monkeypatch.setattr(module.subprocess, "Popen", FakeProcess)
    monkeypatch.setattr(module, "_abort", lambda process: aborted.append(process))

    with pytest.raises(module.BootstrapError, match="interrupted by signal SIGTERM"):
        module._run_release_runner(
            tmp_path / "runner",
            (),
            cwd=tmp_path,
            environment={},
            stdout_descriptor=1,
            stderr_descriptor=2,
        )

    assert len(spawned) == 1
    assert aborted == spawned


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write(path: Path, data: str | bytes, mode: int = 0o600) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and not path.is_symlink():
        path.chmod(0o600)
    if isinstance(data, str):
        path.write_text(data, encoding="utf-8")
    else:
        path.write_bytes(data)
    path.chmod(mode)
    return path.resolve(strict=True)


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
root = args.root.resolve(strict=True)
payload = b""
for relative in ("Cargo.lock", "payload", "scripts/run_sumeragi_v2_release_gates.sh"):
    path = root / relative
    payload += relative.encode() + b"\0" + path.read_bytes()
digest = hashlib.sha256(payload).hexdigest()
lock = hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest()
value = {
    "schema_version": 1,
    "head_commit": digest[:40],
    "head_tree": hashlib.sha256(b"tree" + payload).hexdigest()[:40],
    "index_tree": hashlib.sha256(b"tree" + payload).hexdigest()[:40],
    "workspace_source_manifest_sha256": digest,
    "cargo_lock_sha256": lock,
}
print(json.dumps(value, sort_keys=True, separators=(",", ":")))
'''


def _receipt_validator() -> str:
    return r'''#!/usr/bin/env python3
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--verify-existing", action="store_true", required=True)
args, _ = parser.parse_known_args()
if not args.output.is_file() or args.output.is_symlink():
    raise SystemExit(41)
print(f"Sumeragi v2 aggregate release receipt verified: {args.output.resolve(strict=True)}")
'''


def _runner_tool_manifest() -> bytes:
    tools = {}
    for name in ("chmod", "ln", "mv", "sleep"):
        discovered = shutil.which(name, path=os.defpath)
        assert discovered is not None
        path = Path(discovered).resolve(strict=True)
        tools[name] = {"path": str(path), "sha256": _sha256(path)}
    return (
        json.dumps(
            {"schema_version": 1, "tools": tools},
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode()


def _identity_verifier(
    *,
    mutate_path: Path | None = None,
    mutate_candidate: bool = False,
    attestation_schema: int = 2,
    transcript_schema: int = 2,
    bad_evidence_digest: bool = False,
    reject: bool = False,
    hold_pipe_open: bool = False,
) -> str:
    mutation = ""
    if mutate_path is not None:
        mutation += (
            f"os.chmod(Path({str(mutate_path)!r}), 0o700)\n"
            f"Path({str(mutate_path)!r}).write_bytes(b'mutated-tool')\n"
        )
    if mutate_candidate:
        mutation += "(args.root / 'payload').write_bytes(b'pre-run-source-drift')\n"
    if reject:
        mutation += "raise SystemExit(23)\n"
    if hold_pipe_open:
        mutation += (
            "subprocess.Popen([sys.executable, '-I', '-S', '-c', "
            "'import time; time.sleep(30)'])\n"
        )
    return f'''#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument("--root", type=Path, required=True)
parser.add_argument("--identity", type=Path, required=True)
parser.add_argument("--git-bin", type=Path, required=True)
parser.add_argument("--expected-git-sha256", required=True)
parser.add_argument("--ssh-keygen-bin", type=Path, required=True)
parser.add_argument("--expected-ssh-keygen-sha256", required=True)
parser.add_argument("--expected-signer-fingerprint", required=True)
parser.add_argument("--ssh-allowed-signers", type=Path, required=True)
parser.add_argument("--expected-ssh-allowed-signers-sha256", required=True)
parser.add_argument("--ssh-revocation-file", type=Path, required=True)
parser.add_argument("--expected-ssh-revocation-sha256", required=True)
parser.add_argument("--attestation-output", type=Path, required=True)
parser.add_argument("--verify-transcript-output", type=Path, required=True)
parser.add_argument("--raw-commit-output", type=Path, required=True)
parser.add_argument("--cargo-lock-output", type=Path, required=True)
parser.add_argument("--ssh-allowed-signers-output", type=Path, required=True)
parser.add_argument("--ssh-revocation-output", type=Path, required=True)
parser.add_argument("--git-archive-output", type=Path, required=True)
parser.add_argument("--ssh-keygen-archive-output", type=Path, required=True)
args = parser.parse_args()

def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\\n").encode()

def digest(data):
    return hashlib.sha256(data).hexdigest()

def publish(path, data, mode):
    path.write_bytes(data)
    path.chmod(mode)

identity_bytes = args.identity.read_bytes()
identity = json.loads(identity_bytes)
git = args.git_bin.read_bytes()
ssh = args.ssh_keygen_bin.read_bytes()
allowed = args.ssh_allowed_signers.read_bytes()
revocation = args.ssh_revocation_file.read_bytes()
raw = b"tree " + identity["head_tree"].encode() + b"\\n"
lock = (args.root / "Cargo.lock").read_bytes()

tools = {{
    "git": {{
        "archive_name": args.git_archive_output.name,
        "mode": "0500",
        "observed_sha256": digest(git),
        "protected_sha256": args.expected_git_sha256,
        "size_bytes": len(git),
        "source_path": str(args.git_bin),
    }},
    "ssh_keygen": {{
        "archive_name": args.ssh_keygen_archive_output.name,
        "mode": "0500",
        "observed_sha256": digest(ssh),
        "protected_sha256": args.expected_ssh_keygen_sha256,
        "size_bytes": len(ssh),
        "source_path": str(args.ssh_keygen_bin),
    }},
}}
def protected(data, name, expected):
    return {{
        "archive_name": name,
        "mode": "0400",
        "observed_sha256": digest(data),
        "protected_sha256": expected,
        "size_bytes": len(data),
    }}
policies = {{
    "expected_signer_fingerprint": args.expected_signer_fingerprint,
    "signature_format": "ssh",
    "ssh_allowed_signers": protected(
        allowed, args.ssh_allowed_signers_output.name,
        args.expected_ssh_allowed_signers_sha256,
    ),
    "ssh_revocation": protected(
        revocation, args.ssh_revocation_output.name,
        args.expected_ssh_revocation_sha256,
    ),
}}
transcript = {{
    "schema_version": {transcript_schema},
    "archive_names": {{
        "cargo_lock": args.cargo_lock_output.name,
        "git": args.git_archive_output.name,
        "raw_commit": args.raw_commit_output.name,
        "ssh_allowed_signers": args.ssh_allowed_signers_output.name,
        "ssh_keygen": args.ssh_keygen_archive_output.name,
        "ssh_revocation": args.ssh_revocation_output.name,
        "verify_transcript": args.verify_transcript_output.name,
    }},
    "candidate_commit_oid": identity["head_commit"],
    "environment": {{}},
    "policy_overrides": [],
    "policies": policies,
    "replay": {{}},
    "tools": tools,
    "commands": {{}},
    "tool_probes": {{}},
}}
def command_record(argv, exit_status):
    empty_digest = digest(b"")
    return {{
        "argv": argv,
        "replay_argv": argv,
        "exit_status": exit_status,
        "stdout_base64": "",
        "stdout_sha256": empty_digest,
        "stdout_size_bytes": 0,
        "stderr_base64": "",
        "stderr_sha256": empty_digest,
        "stderr_size_bytes": 0,
    }}
transcript["commands"] = {{
    "show_signature_metadata": command_record(["git", "show"], 0),
    "verify_commit": command_record(["git", "verify-commit"], 0),
}}
transcript["tool_probes"] = {{
    "ssh_keygen_usage": command_record(["ssh-keygen", "-?"], 1),
}}
transcript_bytes = canonical(transcript)
outputs = {{
    "cargo_lock": (args.cargo_lock_output, lock, 0o400),
    "git": (args.git_archive_output, git, 0o500),
    "raw_commit": (args.raw_commit_output, raw, 0o400),
    "ssh_allowed_signers": (args.ssh_allowed_signers_output, allowed, 0o400),
    "ssh_keygen": (args.ssh_keygen_archive_output, ssh, 0o500),
    "ssh_revocation": (args.ssh_revocation_output, revocation, 0o400),
    "verify_transcript": (args.verify_transcript_output, transcript_bytes, 0o400),
}}
evidence = {{}}
for label, (path, data, mode) in outputs.items():
    publish(path, data, mode)
    evidence[label] = {{
        "archive_name": path.name,
        "mode": f"{{mode:04o}}",
        "sha256": ("0" * 64 if {bad_evidence_digest!r} and label == "raw_commit" else digest(data)),
        "size_bytes": len(data),
    }}
attestation = {{
    "schema_version": {attestation_schema},
    "release_identity": identity,
    "release_identity_sha256": digest(identity_bytes),
    "tools": tools,
    "policies": policies,
    "verification": {{
        "status": "G",
        "signer_fingerprint": args.expected_signer_fingerprint,
        "primary_key_fingerprint": "",
        "allowed_signers_principal": "release",
    }},
    "evidence": evidence,
}}
publish(args.attestation_output, canonical(attestation), 0o400)
{mutation}'''


def _runner(
    launch_count: Path,
    candidate: Path,
    action: str,
    *,
    trusted_mutation: Path | None = None,
) -> str:
    actions = {
        "success": ":",
        "slow-success": "sleep 2",
        "fail": "exit 37",
        "source-drift": f"printf drift > {candidate / 'payload'}",
        "evidence-tamper": (
            "chmod 0600 \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION\"\n"
            "printf tamper > \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION\""
        ),
        "marker-tamper": (
            "chmod 0600 \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION\"\n"
            "printf tamper > \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION\""
        ),
        "directory-mode-tamper": (
            "chmod 0755 \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR\""
        ),
        "fail-and-tamper": (
            "chmod 0600 \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION\"\n"
            "printf tamper > \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION\"\n"
            "exit 37"
        ),
        "missing-receipt": "exit 0",
        "receipt-tamper": ":",
        "receipt-wrong-mode": ":",
        "receipt-hardlink": ":",
        "receipt-symlink": ":",
        "receipt-wrong-path": ":",
        "receipt-wrong-schema": ":",
        "receipt-wrong-bootstrap": ":",
        "receipt-wrong-runner": ":",
        "receipt-wrong-identity": ":",
        "receipt-wrong-trust-policy": ":",
        "receipt-mutual-wrong-signer": ":",
        "receipt-missing-cross-tool-evidence": ":",
        "preexisting-postmarker": ":",
    }
    if action == "trusted-drift":
        assert trusted_mutation is not None
        action_script = (
            f"chmod 0700 {trusted_mutation}\n"
            f"printf mutated > {trusted_mutation}"
        )
    else:
        action_script = actions[action]
    receipt_mutation = {
        "receipt-wrong-schema": 'receipt["schema_version"] = True',
        "receipt-wrong-bootstrap": (
            'receipt["authentication"]["bootstrap"]["completion_sha256"] = "0" * 64'
        ),
        "receipt-wrong-runner": (
            'receipt["authentication"]["bootstrap"]["runner"]["sha256"] = "0" * 64'
        ),
        "receipt-wrong-identity": 'receipt["identity"]["head_commit"] = "0" * 40',
        "receipt-wrong-trust-policy": (
            'receipt["authentication"]["release_identity"]["trust_policy"]'
            '["git_sha256"] = "0" * 64'
        ),
        "receipt-mutual-wrong-signer": (
            'wrong = "SHA256:" + "B" * 43\n'
            'receipt["authentication"]["bootstrap"]["signer_fingerprint"] = wrong\n'
            'receipt["authentication"]["release_identity"]'
            '["signer_fingerprint"] = wrong\n'
            'receipt["authentication"]["release_identity"]["trust_policy"]'
            '["signer_fingerprint"] = wrong'
        ),
        "receipt-missing-cross-tool-evidence": (
            'receipt["evidence"].pop("formal_cross_tool_evidence")'
        ),
    }.get(action, "pass")
    receipt_script = f'''python3 -I -S - <<'PY'
import hashlib
import json
import os
from pathlib import Path

def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\\n").encode()

evidence = Path(os.environ["SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR"])
marker_path = Path(os.environ["SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION"])
marker_bytes = marker_path.read_bytes()
marker = json.loads(marker_bytes)
identity_bytes = Path(os.environ["SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY"]).read_bytes()
identity = json.loads(identity_bytes)
attestation = json.loads(
    Path(os.environ["SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION"]).read_bytes()
)
verification = attestation["verification"]
completion_sha256 = hashlib.sha256(marker_bytes).hexdigest()
assert completion_sha256 == os.environ["SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"]
release_runner = evidence / "release-runner"
release_root = release_runner / "source"
release_directory = release_runner / "output" / "release"
for directory in (release_runner, release_root, release_runner / "output", release_directory):
    directory.mkdir(mode=0o700, exist_ok=True)
    directory.chmod(0o700)
runner = marker["runner"]
bootstrap_runner = {{
    "path": runner["path"],
    "sha256": runner["sha256"],
    "mode": runner["mode"],
    "argv": runner["argv"],
    "closed_path_resolution": runner["closed_path_resolution"],
    "output": runner["output"],
    "tool_directory": runner["tool_directory"],
    "tools": runner["tools"],
    "self_digest_environment_variables": runner["self_digest_environment_variables"],
}}
candidate = Path(marker["candidate_root"])
for relative, mode in (
    ("Cargo.lock", 0o400),
    ("payload", 0o400),
    ("scripts/run_sumeragi_v2_release_gates.sh", 0o500),
):
    destination = release_root / relative
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    destination.write_bytes((candidate / relative).read_bytes())
    destination.chmod(mode)
(release_root / "scripts").chmod(0o500)
(release_runner / "sealed-identity.json").write_bytes(identity_bytes)
(release_runner / "sealed-identity.json").chmod(0o400)
release_root.chmod(0o500)

def source_artifact(record):
    path = Path(record["source_path"])
    metadata = path.stat()
    return {{
        "path": str(path),
        "sha256": record["protected_sha256"],
        "size_bytes": metadata.st_size,
        "mode": f"{{metadata.st_mode & 0o7777:04o}}",
        "owner_uid": metadata.st_uid,
        "nlink": metadata.st_nlink,
    }}

mock_directory = release_runner / "output" / "mock-completions"
mock_directory.mkdir(mode=0o700)
completion_records = {{}}
for label in (
    "corridor_completion",
    "formal_completion",
    "formal_verus_evidence",
    "formal_verus_log",
    "formal_cross_tool_evidence",
    "seed_matrix_completion",
    "chaos_completion",
    "taira_completion",
):
    path = mock_directory / (label + ".tsv")
    data = (label + "\\n").encode()
    path.write_bytes(data)
    path.chmod(0o400)
    completion_records[label] = {{
        "path": str(path),
        "sha256": hashlib.sha256(data).hexdigest(),
    }}

trust_policy = {{
    "git_sha256": marker["trusted_inputs"]["git"]["protected_sha256"],
    "ssh_keygen_sha256": marker["trusted_inputs"]["ssh_keygen"]["protected_sha256"],
    "allowed_signers_sha256": marker["trusted_inputs"]["allowed_signers"]["protected_sha256"],
    "revocation_sha256": marker["trusted_inputs"]["revocation"]["protected_sha256"],
    "signer_fingerprint": verification["signer_fingerprint"],
}}
receipt = {{
    "schema_version": 1,
    "protocol": "sumeragi-v2",
    "result": "release-complete",
    "identity": {{
        "head_commit": identity["head_commit"],
        "head_tree": identity["head_tree"],
        "index_tree": identity["index_tree"],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "candidate_source_manifest_sha256": identity["workspace_source_manifest_sha256"],
        "sealed_source_manifest_sha256": identity["workspace_source_manifest_sha256"],
    }},
    "authentication": {{
        "schema_version": 2,
        "bootstrap": {{
            "schema_version": 1,
            "completion_sha256": completion_sha256,
            "frozen_bootstrap_sha256": marker["trusted_inputs"]["bootstrap"]["protected_sha256"],
            "candidate_root": marker["candidate_root"],
            "candidate_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
            "candidate_commit_oid": identity["head_commit"],
            "candidate_tree_oid": identity["head_tree"],
            "runner": bootstrap_runner,
            "signer_fingerprint": verification["signer_fingerprint"],
            "allowed_signers_principal": verification["allowed_signers_principal"],
            "trusted_input_digests": {{
                label: record["protected_sha256"]
                for label, record in marker["trusted_inputs"].items()
            }},
            "trusted_input_sources": {{
                label: source_artifact(record)
                for label, record in marker["trusted_inputs"].items()
            }},
        }},
        "release_identity": {{
            "schema_version": 1,
            "signature_format": "ssh",
            "verification_status": "G",
            "candidate_commit_oid": identity["head_commit"],
            "candidate_tree_oid": identity["head_tree"],
            "signer_fingerprint": verification["signer_fingerprint"],
            "primary_key_fingerprint": "",
            "allowed_signers_principal": verification["allowed_signers_principal"],
            "release_root": str(release_root),
            "archive_directory": str(evidence),
            "trust_policy": trust_policy,
            "attested_tools": attestation["tools"],
            "attested_policies": attestation["policies"],
            "replay": {{"performed": True}},
        }},
    }},
    "evidence": {{
        "bootstrap": {{
            "completion": {{}},
            "candidate_identity": {{}},
            "runner": {{}},
            "candidate_cargo_lock": {{}},
            "trusted_inputs": {{label: {{}} for label in marker["trusted_inputs"]}},
            "identity_verification": {{}},
            "runner_tools": {{label: {{}} for label in runner["tools"]}},
        }},
        "release_signature_attestation": {{}},
        "release_signature_transcript": {{}},
        "release_signature_raw_commit": {{}},
        "release_signature_cargo_lock": {{}},
        "release_signature_allowed_signers": {{}},
        "release_signature_revocation": {{}},
        "release_signature_git": {{}},
        "release_signature_ssh_keygen": {{}},
        **completion_records,
        "corridor_summary": {{}},
        "corridor_production_inventory": {{}},
        "corridor_logs": [],
        "formal_gate_log": {{}},
        "formal_proof_coverage": {{}},
        "formal_proof_evidence": {{}},
        "formal_harness_lock": {{}},
        "formal_toolchain": {{}},
        "seed_matrix_summary": {{}},
        "seed_matrix_run_logs": [],
        "seed_matrix_localnet_manifest_index": {{}},
        "seed_matrix_localnet_manifests": [],
        "chaos_log": {{}},
        "taira_evidence": {{}},
        "taira_run_log": {{}},
    }},
}}
{receipt_mutation}
output = release_directory / "RELEASE_COMPLETED.json"
with output.open("xb") as stream:
    stream.write(canonical(receipt))
    stream.flush()
    os.fchmod(stream.fileno(), 0o400)
    os.fsync(stream.fileno())
directory_fd = os.open(release_directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
try:
    os.fsync(directory_fd)
finally:
    os.close(directory_fd)
PY'''
    post_receipt_action = {
        "receipt-tamper": (
            'chmod 0600 "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/RELEASE_COMPLETED.json"\n'
            'printf tamper > "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/RELEASE_COMPLETED.json"'
        ),
        "receipt-wrong-mode": (
            'chmod 0600 "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/RELEASE_COMPLETED.json"'
        ),
        "receipt-hardlink": (
            'ln "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/RELEASE_COMPLETED.json" '
            '"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/receipt-alias"'
        ),
        "receipt-symlink": (
            'mv "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/RELEASE_COMPLETED.json" '
            '"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/receipt-target"\n'
            'ln -s receipt-target "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/RELEASE_COMPLETED.json"'
        ),
        "receipt-wrong-path": (
            'mv "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'release-runner/output/release/RELEASE_COMPLETED.json" '
            '"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/RELEASE_COMPLETED.json"'
        ),
        "preexisting-postmarker": (
            'printf attacker > "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'BOOTSTRAP_RELEASE_COMPLETED.json"\n'
            'chmod 0400 "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/'
            'BOOTSTRAP_RELEASE_COMPLETED.json"'
        ),
    }.get(action, ":")
    if action == "missing-receipt":
        receipt_script = ":"
    return f'''#!/bin/bash
set -eu
: "${{SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256:?}}"
: "${{SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION:?}}"
: "${{SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT:?}}"
: "${{SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY:?}}"
: "${{SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR:?}}"
: "${{IROHA_RELEASE_BOOTSTRAP_COMPLETION:?}}"
: "${{IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256:?}}"
: "${{IROHA_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256:?}}"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY"
count=0
if test -f {launch_count}; then count=$(<{launch_count}); fi
count=$((count + 1))
printf '%s\n' "$count" > {launch_count}
{receipt_script}
{action_script}
{post_receipt_action}
'''


def _continuous_writer_runner(
    launch_count: Path,
    completed: Path,
    *,
    chunks: int,
    hold_seconds: float,
) -> str:
    return f'''#!/bin/bash
set -eu
count=0
if test -f {launch_count}; then count=$(<{launch_count}); fi
count=$((count + 1))
printf '%s\n' "$count" > {launch_count}
python3 -I -S - {completed} {chunks} {hold_seconds} <<'PY'
import os
from pathlib import Path
import sys
import time

completed = Path(sys.argv[1])
chunks = int(sys.argv[2])
hold_seconds = float(sys.argv[3])
stdout_chunk = b"O" * 65536
stderr_chunk = b"E" * 65536
for _ in range(chunks):
    os.write(1, stdout_chunk)
    os.write(2, stderr_chunk)
completed.write_text(str(chunks * len(stdout_chunk)) + "\\n", encoding="utf-8")
time.sleep(hold_seconds)
PY
exit 37
'''


def _wait_for(path: Path, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.02)
    raise AssertionError(f"timed out waiting for {path}")


@dataclass
class Fixture:
    root: Path
    candidate: Path
    trust: Path
    evidence: Path
    launch_count: Path
    manifest: Path
    verifier: Path
    receipt_validator: Path
    tool_manifest: Path
    git: Path
    ssh: Path
    bash: Path
    allowed: Path
    revocation: Path

    def arguments(self) -> list[str]:
        return [
            str(PYTHON),
            "-I",
            "-S",
            str(BOOTSTRAP),
            "--candidate-root", str(self.candidate),
            "--evidence-dir", str(self.evidence),
            "--expected-bootstrap-sha256", _sha256(BOOTSTRAP),
            "--python-bin", str(PYTHON),
            "--expected-python-sha256", _sha256(PYTHON),
            "--git-bin", str(self.git),
            "--expected-git-sha256", _sha256(self.git),
            "--ssh-keygen-bin", str(self.ssh),
            "--expected-ssh-keygen-sha256", _sha256(self.ssh),
            "--manifest-helper", str(self.manifest),
            "--expected-manifest-helper-sha256", _sha256(self.manifest),
            "--identity-verifier", str(self.verifier),
            "--expected-identity-verifier-sha256", _sha256(self.verifier),
            "--receipt-validator", str(self.receipt_validator),
            "--expected-receipt-validator-sha256", _sha256(self.receipt_validator),
            "--runner-tool-manifest", str(self.tool_manifest),
            "--expected-runner-tool-manifest-sha256", _sha256(self.tool_manifest),
            "--bash-bin", str(self.bash),
            "--expected-bash-sha256", _sha256(self.bash),
            "--expected-signer-fingerprint", FINGERPRINT,
            "--ssh-allowed-signers", str(self.allowed),
            "--expected-ssh-allowed-signers-sha256", _sha256(self.allowed),
            "--ssh-revocation-file", str(self.revocation),
            "--expected-ssh-revocation-sha256", _sha256(self.revocation),
            "--command-timeout-seconds", "10",
        ]

    def run(self, arguments: list[str] | None = None) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            arguments or self.arguments(),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=30,
            check=False,
            env={"PATH": os.environ.get("PATH", "")},
        )


@pytest.fixture
def release_fixture(tmp_path: Path) -> Fixture:
    root = tmp_path.resolve(strict=True)
    candidate = root / "candidate"
    trust = root / "trust"
    candidate.mkdir()
    trust.mkdir()
    launch_count = root / "launch-count"
    evidence = root / "evidence"
    _write(candidate / "Cargo.lock", b"locked\n")
    _write(candidate / "payload", b"candidate\n")
    manifest = _write(trust / "manifest.py", _manifest_helper(), 0o500)
    verifier = _write(trust / "verifier.py", _identity_verifier(), 0o500)
    receipt_validator = _write(
        trust / "receipt-validator.py", _receipt_validator(), 0o500
    )
    tool_manifest = _write(
        trust / "runner-tool-manifest.json", _runner_tool_manifest(), 0o400
    )
    git = _write(trust / "git", "#!/bin/sh\nexit 0\n", 0o500)
    ssh = _write(trust / "ssh-keygen", "#!/bin/sh\nexit 0\n", 0o500)
    bash = _write(
        trust / "relocatable-bash",
        "#!/bin/bash\nexec /bin/bash \"$@\"\n",
        0o500,
    )
    allowed = _write(
        trust / "allowed-signers",
        "release namespaces=\"git\" ssh-ed25519 AAAATEST\n",
        0o400,
    )
    revocation = _write(trust / "revocation", b"", 0o400)
    _write(
        candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(launch_count, candidate, "success"),
        0o500,
    )
    return Fixture(
        root,
        candidate.resolve(strict=True),
        trust.resolve(strict=True),
        evidence,
        launch_count,
        manifest,
        verifier,
        receipt_validator,
        tool_manifest,
        git,
        ssh,
        bash,
        allowed,
        revocation,
    )


def _replace_flag(arguments: list[str], flag: str, value: str) -> list[str]:
    updated = arguments.copy()
    updated[updated.index(flag) + 1] = value
    return updated


def _assert_never_launched(fixture: Fixture, result: subprocess.CompletedProcess[str]) -> None:
    assert result.returncode != 0, result
    assert not fixture.launch_count.exists()
    assert not fixture.evidence.exists()


def test_success_authenticates_then_launches_exactly_once(release_fixture: Fixture) -> None:
    result = release_fixture.run()
    assert result.returncode == 0, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    marker = release_fixture.evidence / "BOOTSTRAP_COMPLETED.json"
    data = marker.read_bytes()
    value = json.loads(data)
    assert data == (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
    assert value["schema_version"] == 1
    assert value["trust_boundary"] == {
        "bootstrap_authentication": "external prerequisite",
        "release_image_and_dynamic_loader": "external prerequisite",
        "same_uid_and_trusted_ancestor_owners": True,
    }
    assert value["runner"]["argv"] == [
        str(release_fixture.evidence / "bash"),
        str(release_fixture.candidate / "scripts/run_sumeragi_v2_release_gates.sh"),
        "--release",
    ]
    assert value["runner"]["closed_path_resolution"] == {
        "bash": str(release_fixture.evidence / "bash"),
        "git": str(release_fixture.evidence / "git"),
        "python3": str(release_fixture.evidence / "python3"),
    }
    closed_environment = value["runner"]["environment_without_self_digest"]
    assert closed_environment["PATH"].split(os.pathsep)[0] == str(
        release_fixture.evidence
    )
    assert "BASH_ENV" not in closed_environment
    assert "PYTHONPATH" not in closed_environment
    assert "LD_PRELOAD" not in closed_environment
    assert "DYLD_INSERT_LIBRARIES" not in closed_environment
    assert value["runner"]["self_digest_environment_variables"] == [
        "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
        "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
    ]
    assert stat.S_IMODE(release_fixture.evidence.stat().st_mode) == 0o700
    assert stat.S_IMODE(marker.stat().st_mode) == 0o400
    receipt = (
        release_fixture.evidence
        / "release-runner"
        / "output"
        / "release"
        / "RELEASE_COMPLETED.json"
    )
    terminal_receipt = json.loads(receipt.read_text(encoding="utf-8"))
    assert {
        "formal_verus_evidence",
        "formal_verus_log",
        "formal_cross_tool_evidence",
    } <= set(terminal_receipt["evidence"])
    external_marker = release_fixture.evidence / "BOOTSTRAP_RELEASE_COMPLETED.json"
    external_data = external_marker.read_bytes()
    external = json.loads(external_data)
    assert external_data == (
        json.dumps(external, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()
    assert external == {
        "schema_version": 1,
        "result": "release-complete",
        "bootstrap_completion_sha256": hashlib.sha256(data).hexdigest(),
        "candidate_root": str(release_fixture.candidate),
        "candidate_identity_sha256": _sha256(
            release_fixture.evidence / "candidate-identity.json"
        ),
        "candidate_commit_oid": value["candidate_identity"]["head_commit"],
        "candidate_tree_oid": value["candidate_identity"]["head_tree"],
        "runner": {
            "path": str(
                release_fixture.candidate
                / "scripts"
                / "run_sumeragi_v2_release_gates.sh"
            ),
            "sha256": value["runner"]["sha256"],
            "mode": value["runner"]["mode"],
            "logs": {
                label: {
                    "path": str(release_fixture.evidence / f"runner-{label}.log"),
                    "sha256": _sha256(
                        release_fixture.evidence / f"runner-{label}.log"
                    ),
                    "size_bytes": (
                        release_fixture.evidence / f"runner-{label}.log"
                    ).stat().st_size,
                    "mode": "0400",
                }
                for label in ("stderr", "stdout")
            },
        },
        "retained_source": {
            "path": str(release_fixture.evidence / "release-runner" / "source"),
            "identity_path": str(
                release_fixture.evidence
                / "release-runner"
                / "sealed-identity.json"
            ),
            "identity_sha256": _sha256(
                release_fixture.evidence
                / "release-runner"
                / "sealed-identity.json"
            ),
            "source_manifest_sha256": value["candidate_identity"][
                "workspace_source_manifest_sha256"
            ],
            "mode": "0500",
        },
        "receipt_validator": {
            "archive_path": str(release_fixture.evidence / "validate-receipt.py"),
            "sha256": _sha256(release_fixture.receipt_validator),
            "exit_status": 0,
        },
        "terminal_receipt": {
            "path": str(receipt),
            "sha256": _sha256(receipt),
            "size_bytes": receipt.stat().st_size,
            "mode": "0400",
        },
    }
    assert stat.S_IMODE(external_marker.stat().st_mode) == 0o400
    assert external_marker.stat().st_nlink == 1
    assert not any(
        child.name.startswith(".BOOTSTRAP_") and ".stage." in child.name
        for child in release_fixture.evidence.iterdir()
    )


def test_release_runner_has_no_outer_timeout_or_output_capture(
    release_fixture: Fixture,
) -> None:
    source = BOOTSTRAP.read_text(encoding="utf-8")
    assert "--runner-timeout-seconds" not in source
    assert "_MAX_RUNNER_OUTPUT_BYTES" not in source
    assert "runner = _run_release_runner(" in source
    runner_source = source[
        source.index("def _run_release_runner(") : source.index(
            "def _open_runner_log("
        )
    ]
    assert "subprocess.PIPE" not in runner_source
    assert "selector" not in runner_source
    assert "stdout=stdout_descriptor" in runner_source
    assert "stderr=stderr_descriptor" in runner_source

    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(release_fixture.launch_count, release_fixture.candidate, "slow-success"),
        0o500,
    )
    arguments = _replace_flag(
        release_fixture.arguments(), "--command-timeout-seconds", "1"
    )
    started = time.monotonic()
    result = release_fixture.run(arguments)

    assert result.returncode == 0, result.stderr
    assert time.monotonic() - started >= 1.5
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert (release_fixture.evidence / "BOOTSTRAP_RELEASE_COMPLETED.json").is_file()


def test_blocked_bootstrap_diagnostics_cannot_backpressure_runner_output(
    release_fixture: Fixture,
) -> None:
    completed = release_fixture.root / "continuous-writer-completed"
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _continuous_writer_runner(
            release_fixture.launch_count,
            completed,
            chunks=32,
            hold_seconds=0.2,
        ),
        0o500,
    )
    process = subprocess.Popen(
        release_fixture.arguments(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        env={"PATH": os.environ.get("PATH", "")},
    )
    _wait_for(completed)
    expected_size = int(completed.read_text(encoding="utf-8"))
    assert (release_fixture.evidence / "runner-stdout.log").stat().st_size >= expected_size
    assert (release_fixture.evidence / "runner-stderr.log").stat().st_size >= expected_size
    stdout, stderr = process.communicate(timeout=10)
    assert process.returncode == 37, stderr.decode("utf-8", "replace")
    assert stdout == b""


def test_bootstrap_interruption_terminates_owned_runner_and_removes_evidence(
    release_fixture: Fixture,
) -> None:
    runner_pid_path = release_fixture.root / "interrupted-runner-pid"
    child_pid_path = release_fixture.root / "interrupted-child-pid"
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        f"""#!/bin/bash
set -eu
printf '%s\n' "$$" > {runner_pid_path}
sleep 60 &
child=$!
printf '%s\n' "$child" > {child_pid_path}
wait "$child"
""",
        0o500,
    )
    process = subprocess.Popen(
        release_fixture.arguments(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        env={"PATH": os.environ.get("PATH", "")},
    )
    _wait_for(runner_pid_path)
    _wait_for(child_pid_path)
    runner_pid = int(runner_pid_path.read_text(encoding="utf-8"))
    child_pid = int(child_pid_path.read_text(encoding="utf-8"))
    assert runner_pid > 1
    assert child_pid > 1

    # Signal only the bootstrap PID. Its handler owns cleanup of the private
    # release-runner session and must not leave either runner or descendant.
    process.terminate()
    process.wait(timeout=10)
    assert process.returncode != 0
    assert not release_fixture.evidence.exists()
    for pid in (runner_pid, child_pid):
        with pytest.raises(ProcessLookupError):
            os.kill(pid, 0)


@pytest.mark.parametrize(
    "action",
    [
        "missing-receipt",
        "receipt-tamper",
        "receipt-wrong-mode",
        "receipt-hardlink",
        "receipt-symlink",
        "receipt-wrong-path",
        "receipt-wrong-schema",
        "receipt-wrong-bootstrap",
        "receipt-wrong-runner",
        "receipt-wrong-identity",
        "receipt-wrong-trust-policy",
        "receipt-mutual-wrong-signer",
        "receipt-missing-cross-tool-evidence",
        "preexisting-postmarker",
    ],
)
def test_success_status_without_exact_authenticated_receipt_fails_closed(
    release_fixture: Fixture, action: str
) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(release_fixture.launch_count, release_fixture.candidate, action),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


@pytest.mark.parametrize(
    "flag",
    [
        "--expected-bootstrap-sha256",
        "--expected-python-sha256",
        "--expected-git-sha256",
        "--expected-ssh-keygen-sha256",
        "--expected-manifest-helper-sha256",
        "--expected-identity-verifier-sha256",
        "--expected-receipt-validator-sha256",
        "--expected-runner-tool-manifest-sha256",
        "--expected-bash-sha256",
        "--expected-ssh-allowed-signers-sha256",
        "--expected-ssh-revocation-sha256",
    ],
)
def test_protected_hash_mismatch_never_launches(
    release_fixture: Fixture, flag: str
) -> None:
    result = release_fixture.run(
        _replace_flag(release_fixture.arguments(), flag, "0" * 64)
    )
    _assert_never_launched(release_fixture, result)


def test_relative_trusted_path_never_launches(release_fixture: Fixture) -> None:
    result = release_fixture.run(
        _replace_flag(release_fixture.arguments(), "--git-bin", "trust/git")
    )
    _assert_never_launched(release_fixture, result)


def test_nonisolated_python_startup_never_launches(release_fixture: Fixture) -> None:
    arguments = release_fixture.arguments()
    del arguments[1:3]
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize("input_flag", ["--git-bin", "--manifest-helper", "--ssh-allowed-signers"])
def test_candidate_contained_trust_input_never_launches(
    release_fixture: Fixture, input_flag: str
) -> None:
    source = {
        "--git-bin": release_fixture.git,
        "--manifest-helper": release_fixture.manifest,
        "--ssh-allowed-signers": release_fixture.allowed,
    }[input_flag]
    destination = _write(
        release_fixture.candidate / f"untrusted-{source.name}",
        source.read_bytes(),
        stat.S_IMODE(source.stat().st_mode),
    )
    arguments = _replace_flag(release_fixture.arguments(), input_flag, str(destination))
    digest_flag = {
        "--git-bin": "--expected-git-sha256",
        "--manifest-helper": "--expected-manifest-helper-sha256",
        "--ssh-allowed-signers": "--expected-ssh-allowed-signers-sha256",
    }[input_flag]
    arguments = _replace_flag(arguments, digest_flag, _sha256(destination))
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_bootstrap_copy_inside_candidate_is_rejected(release_fixture: Fixture) -> None:
    copied = _write(
        release_fixture.candidate / "bootstrap.py", BOOTSTRAP.read_bytes(), 0o500
    )
    arguments = release_fixture.arguments()
    arguments[3] = str(copied)
    arguments = _replace_flag(arguments, "--expected-bootstrap-sha256", _sha256(copied))
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_symlinked_tool_never_launches(release_fixture: Fixture) -> None:
    link = release_fixture.trust / "git-link"
    link.symlink_to(release_fixture.git)
    result = release_fixture.run(
        _replace_flag(release_fixture.arguments(), "--git-bin", str(link))
    )
    _assert_never_launched(release_fixture, result)


def test_wrong_python_even_with_matching_hash_never_launches(release_fixture: Fixture) -> None:
    fake = _write(release_fixture.trust / "python", "#!/bin/sh\nexit 0\n", 0o500)
    arguments = _replace_flag(release_fixture.arguments(), "--python-bin", str(fake))
    arguments = _replace_flag(arguments, "--expected-python-sha256", _sha256(fake))
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_identity_rejection_never_launches(release_fixture: Fixture) -> None:
    release_fixture.verifier = _write(
        release_fixture.trust / "reject.py", _identity_verifier(reject=True), 0o500
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize(
    "policy",
    [
        "release valid-after=\"20260717Z\" ssh-ed25519 AAAATEST\n",
        "release valid-before=\"20270717Z\" ssh-ed25519 AAAATEST\n",
        "release ssh-ed25519 AAAATEST\nbackup ssh-ed25519 AAAATEST\n",
    ],
)
def test_bootstrap_independently_rejects_nondeterministic_signer_policy(
    release_fixture: Fixture, policy: str
) -> None:
    release_fixture.allowed = _write(
        release_fixture.trust / "nondeterministic-allowed-signers",
        policy,
        0o400,
    )

    result = release_fixture.run()

    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize(
    "verifier_source",
    [
        _identity_verifier(attestation_schema=1),
        _identity_verifier(attestation_schema=True),
        _identity_verifier(attestation_schema=2.0),
        _identity_verifier(transcript_schema=1),
        _identity_verifier(transcript_schema=True),
        _identity_verifier(transcript_schema=2.0),
        _identity_verifier(bad_evidence_digest=True),
    ],
)
def test_malformed_schema_v2_evidence_never_launches(
    release_fixture: Fixture, verifier_source: str
) -> None:
    release_fixture.verifier = _write(
        release_fixture.trust / "malformed.py", verifier_source, 0o500
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize("target_name", ["git", "manifest", "allowed"])
def test_trusted_input_toctou_after_verification_never_launches(
    release_fixture: Fixture, target_name: str
) -> None:
    target = getattr(release_fixture, target_name)
    release_fixture.verifier = _write(
        release_fixture.trust / "mutator.py",
        _identity_verifier(mutate_path=target),
        0o500,
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


def test_source_drift_after_verification_never_launches(release_fixture: Fixture) -> None:
    release_fixture.verifier = _write(
        release_fixture.trust / "source-mutator.py",
        _identity_verifier(mutate_candidate=True),
        0o500,
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


def test_descendant_holding_helper_pipes_cannot_defeat_timeout(
    release_fixture: Fixture,
) -> None:
    release_fixture.verifier = _write(
        release_fixture.trust / "pipe-holder.py",
        _identity_verifier(hold_pipe_open=True),
        0o500,
    )
    arguments = _replace_flag(
        release_fixture.arguments(), "--command-timeout-seconds", "1"
    )
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)
    assert "bounded runtime" in result.stderr


@pytest.mark.parametrize(
    "action",
    [
        "source-drift",
        "evidence-tamper",
        "marker-tamper",
        "directory-mode-tamper",
    ],
)
def test_post_launch_tampering_fails_closed(
    release_fixture: Fixture, action: str
) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(release_fixture.launch_count, release_fixture.candidate, action),
        0o500,
    )
    result = release_fixture.run()
    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_post_launch_trusted_tool_drift_fails_closed(release_fixture: Fixture) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "trusted-drift",
            trusted_mutation=release_fixture.git,
        ),
        0o500,
    )
    result = release_fixture.run()
    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_runner_failure_status_is_preserved_exactly(release_fixture: Fixture) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(release_fixture.launch_count, release_fixture.candidate, "fail"),
        0o500,
    )
    result = release_fixture.run()
    assert result.returncode == 37, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_protected_receipt_validator_failure_blocks_external_completion(
    release_fixture: Fixture,
) -> None:
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "reject-receipt.py",
        "raise SystemExit(72)\n",
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "protected receipt validator" in result.stderr.lower()
    assert not release_fixture.evidence.exists()


def test_runner_failure_wins_over_post_validation_failure(
    release_fixture: Fixture,
) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "fail-and-tamper",
        ),
        0o500,
    )
    result = release_fixture.run()
    assert result.returncode == 37, result.stderr
    assert "post-run bootstrap validation also failed" in result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_existing_evidence_is_never_overwritten(release_fixture: Fixture) -> None:
    release_fixture.evidence.mkdir(mode=0o700)
    sentinel = _write(release_fixture.evidence / "sentinel", b"keep", 0o400)
    result = release_fixture.run()
    assert result.returncode != 0
    assert sentinel.read_bytes() == b"keep"
    assert not release_fixture.launch_count.exists()


@pytest.mark.parametrize("mode", [0o755, 0o777])
def test_unsafe_evidence_parent_mode_never_launches(
    release_fixture: Fixture, mode: int
) -> None:
    release_fixture.root.chmod(mode)
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


def test_legacy_runner_path_entry_never_launches(
    release_fixture: Fixture,
) -> None:
    unsafe = release_fixture.root / "unsafe:path"
    unsafe.mkdir(mode=0o700)
    result = release_fixture.run(
        [*release_fixture.arguments(), "--runner-path-entry", str(unsafe)]
    )
    _assert_never_launched(release_fixture, result)


def test_runner_tool_manifest_digest_mismatch_never_launches(
    release_fixture: Fixture,
) -> None:
    manifest = json.loads(release_fixture.tool_manifest.read_text(encoding="utf-8"))
    manifest["tools"]["chmod"]["sha256"] = "0" * 64
    _write(
        release_fixture.tool_manifest,
        json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
        0o400,
    )

    result = release_fixture.run()

    _assert_never_launched(release_fixture, result)
    assert "protected sha-256" in result.stderr.lower()


def test_runner_tool_manifest_rejects_writable_source_ancestor(
    release_fixture: Fixture,
) -> None:
    writable_directory = release_fixture.root / "writable-tools"
    writable_directory.mkdir(mode=0o700)
    tool = _write(
        writable_directory / "chmod",
        "#!/bin/sh\nexit 0\n",
        0o500,
    )
    writable_directory.chmod(0o770)
    manifest = json.loads(release_fixture.tool_manifest.read_text(encoding="utf-8"))
    manifest["tools"]["chmod"] = {
        "path": str(tool),
        "sha256": _sha256(tool),
    }
    _write(
        release_fixture.tool_manifest,
        json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
        0o400,
    )

    result = release_fixture.run()

    _assert_never_launched(release_fixture, result)
    assert "writable, symlinked, or untrusted ancestor" in result.stderr.lower()


def test_undeclared_runner_tool_has_no_ambient_path_fallback(
    release_fixture: Fixture,
) -> None:
    manifest = json.loads(release_fixture.tool_manifest.read_text(encoding="utf-8"))
    del manifest["tools"]["sleep"]
    _write(
        release_fixture.tool_manifest,
        json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
        0o400,
    )
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(release_fixture.launch_count, release_fixture.candidate, "slow-success"),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 127
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_nonrelocatable_protected_bash_never_launches(
    release_fixture: Fixture,
) -> None:
    protected = _write(
        release_fixture.trust / "protected-shell",
        "#!/bin/sh\nexit 19\n",
        0o500,
    )
    arguments = _replace_flag(
        release_fixture.arguments(), "--bash-bin", str(protected)
    )
    arguments = _replace_flag(
        arguments, "--expected-bash-sha256", _sha256(protected)
    )
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_unapproved_runner_environment_is_rejected(release_fixture: Fixture) -> None:
    result = release_fixture.run(
        [*release_fixture.arguments(), "--runner-environment", "BASH_ENV=/tmp/attack"]
    )
    _assert_never_launched(release_fixture, result)


def test_candidate_runner_symlink_never_launches(release_fixture: Fixture) -> None:
    runner = release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"
    target = release_fixture.root / "outside-runner"
    shutil.move(runner, target)
    runner.symlink_to(target)
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)
