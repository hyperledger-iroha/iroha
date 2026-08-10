"""Adversarial tests for the externally trusted Sumeragi v2 bootstrap."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import stat
import subprocess
import sys
import time

import pytest

from pytests.scripts.sumeragi_v2_release_bootstrap_tool_manifest_support import (
    runner_tool_manifest as _runner_tool_manifest,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
BOOTSTRAP = REPO_ROOT / "scripts" / "bootstrap_sumeragi_v2_release.py"
RECEIPT_VALIDATOR_SUPPORT = REPO_ROOT / "scripts" / "sumeragi_v2_localnet_manifest.py"
PYTHON = Path(sys.executable).resolve(strict=True)
FINGERPRINT = "SHA256:" + "A" * 43
SCALING_EVIDENCE_ENV = "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
SCALING_TRUST_ENV = (
    "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
    SCALING_EVIDENCE_ENV,
    "IROHA_RELEASE_SCALING_IROHAD_SHA256",
    "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
    "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
)
RELEASE_CONTROL_ENV = (
    "IROHA_RELEASE_CANCEL_REQUEST_PATH",
    "IROHA_RELEASE_TLA2TOOLS_JAR",
)
DEFAULT_SCALING_DIGESTS = {
    "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": "a" * 64,
    "IROHA_RELEASE_SCALING_IROHAD_SHA256": "b" * 64,
    "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": "c" * 64,
    "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": "d" * 64,
}


def _load_bootstrap_module() -> object:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_test_module", BOOTSTRAP
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_release_trust_inputs_are_the_only_new_runner_environment_names() -> None:
    module = _load_bootstrap_module()

    preexisting_allowlist = {
        "CARGO_HOME",
        "CARGO_NET_GIT_FETCH_WITH_CLI",
        "CARGO_NET_OFFLINE",
        "NIX_SSL_CERT_FILE",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "SSL_CERT_FILE",
    }
    expected_release_environment = set(SCALING_TRUST_ENV) | set(RELEASE_CONTROL_ENV)
    assert (
        module._RUNNER_ENV_ALLOWLIST - preexisting_allowlist
        == expected_release_environment
    )
    assert module._RUNNER_ENV_ALLOWLIST == preexisting_allowlist | set(
        expected_release_environment
    )


def test_release_runner_waits_for_natural_completion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = _load_bootstrap_module()
    spawned: list[dict[str, object]] = []
    completed: list[bool] = []

    class FakeProcess:
        def __init__(self, _argv: object, **kwargs: object) -> None:
            spawned.append(kwargs)

        def wait(self) -> int:
            completed.append(True)
            return 23

    monkeypatch.setattr(module.subprocess, "Popen", FakeProcess)
    result = module._run_release_runner(
        tmp_path / "runner",
        (),
        cwd=tmp_path,
        environment={},
        stdout_descriptor=1,
        stderr_descriptor=2,
    )

    assert result.returncode == 23
    assert completed == [True]
    assert len(spawned) == 1
    assert "start_new_session" not in spawned[0]


@pytest.mark.parametrize(
    ("timeout_seconds", "maximum_output_bytes", "program", "message"),
    [
        (
            0,
            1024,
            "import time; time.sleep(0.05)",
            "bounded runtime",
        ),
        (
            5,
            32,
            "import sys; "
            "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
            "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush()",
            "bounded output limit",
        ),
    ],
)
def test_bounded_helper_finishes_naturally_before_reporting_latched_violation(
    tmp_path: Path,
    timeout_seconds: int,
    maximum_output_bytes: int,
    program: str,
    message: str,
) -> None:
    module = _load_bootstrap_module()
    sentinel = tmp_path / "natural-completion"
    child = (
        f"{program}; from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(module.BootstrapError, match=message):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", child),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            timeout_seconds=timeout_seconds,
            maximum_output_bytes=maximum_output_bytes,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


def test_bounded_helper_drains_inherited_pipes_until_descendant_finishes(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    sentinel = tmp_path / "descendant-natural-completion"
    descendant = (
        "import time; from pathlib import Path; time.sleep(0.05); "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )
    child = (
        "import subprocess; "
        f"subprocess.Popen([{str(PYTHON)!r}, '-I', '-S', '-c', {descendant!r}])"
    )

    with pytest.raises(module.BootstrapError, match="bounded runtime"):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", child),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            timeout_seconds=0,
            maximum_output_bytes=1024,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


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


def _receipt_validator(mutation: str = "") -> str:
    source = r'''#!/usr/bin/env python3
import argparse
import os
from pathlib import Path

parser = argparse.ArgumentParser()
for option in (
    "candidate-identity",
    "sealed-identity",
    "release-root",
    "signature-attestation",
    "signature-transcript",
    "signature-raw-commit",
    "signature-cargo-lock",
    "signature-allowed-signers",
    "signature-revocation",
    "signature-git",
    "signature-ssh-keygen",
    "expected-git-sha256",
    "expected-ssh-keygen-sha256",
    "expected-allowed-signers-sha256",
    "expected-revocation-sha256",
    "expected-signer-fingerprint",
    "bootstrap-completion",
    "bootstrap-evidence-dir",
    "bootstrap-identity",
    "bootstrap-attestation",
    "bootstrap-transcript",
    "expected-bootstrap-completion-sha256",
    "bootstrap-candidate-root",
    "bootstrap-runner",
    "corridor-completion",
    "formal-completion",
    "seed-completion",
    "chaos-completion",
    "taira-completion",
    "g4p-completion",
    "g12-seed-completion",
    "g12-fault-soak-completion",
    "scaling-evidence-manifest",
    "expected-scaling-trial-harness-sha256",
    "expected-scaling-configuration-sha256",
    "expected-scaling-irohad-sha256",
    "expected-scaling-iroha-cli-sha256",
    "repository-root",
):
    parser.add_argument(f"--{option}", required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--verify-existing", action="store_true", required=True)
args = parser.parse_args()
if not args.output.is_file() or args.output.is_symlink():
    raise SystemExit(41)
for path in (
    args.g4p_completion,
    args.g12_seed_completion,
    args.g12_fault_soak_completion,
    args.scaling_evidence_manifest,
):
    candidate = Path(path)
    if (
        not candidate.is_absolute()
        or candidate.resolve(strict=True) != candidate
        or not candidate.is_file()
        or candidate.is_symlink()
    ):
        raise SystemExit(42)
release_output = args.output.resolve(strict=True).parent.parent
for path, relative in (
    (args.g4p_completion, Path("g4p/COMPLETED.tsv")),
    (args.g12_seed_completion, Path("g12-seed/COMPLETED.tsv")),
    (args.g12_fault_soak_completion, Path("g12-soak/COMPLETED.tsv")),
):
    candidate = Path(path)
    if candidate != release_output / relative:
        raise SystemExit(44)
if args.scaling_evidence_manifest != os.environ.get(
    "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
):
    raise SystemExit(45)
for argument, environment_name in (
    (
        args.expected_scaling_trial_harness_sha256,
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
    ),
    (
        args.expected_scaling_configuration_sha256,
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
    ),
    (args.expected_scaling_irohad_sha256, "IROHA_RELEASE_SCALING_IROHAD_SHA256"),
    (
        args.expected_scaling_iroha_cli_sha256,
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
    ),
):
    if argument != os.environ.get(environment_name):
        raise SystemExit(43)
print(f"Sumeragi v2 aggregate release receipt verified: {args.output.resolve(strict=True)}")
'''
    if mutation:
        source = source.replace(
            'print(f"Sumeragi v2 aggregate release receipt verified:',
            mutation
            + '\nprint(f"Sumeragi v2 aggregate release receipt verified:',
        )
    return source


def _identity_verifier(
    *,
    mutate_path: Path | None = None,
    mutate_candidate: bool = False,
    attestation_schema: int = 2,
    transcript_schema: int = 2,
    bad_evidence_digest: bool = False,
    reject: bool = False,
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
    return f'''#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
from pathlib import Path
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
    observed_scaling_environment: Path | None = None,
    receipt_mutation_override: str | None = None,
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
        "receipt-support-archive-substitution": (
            "chmod 0600 \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/"
            "sumeragi_v2_localnet_manifest.py\"\n"
            "printf substituted > \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/"
            "sumeragi_v2_localnet_manifest.py\""
        ),
        "receipt-support-archive-omission": (
            "mv \"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/"
            "sumeragi_v2_localnet_manifest.py\" "
            "\"$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/"
            "omitted-localnet-manifest.py\""
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
    receipt_mutation = receipt_mutation_override or {
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
for directory in (release_runner, release_root, release_runner / "target", release_runner / "output", release_directory):
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
for relative in (
    "scripts/nexus/validate_multilane_scaling_evidence.py",
    "scripts/deploy_localnet.sh",
    "scripts/tx_load.py",
    "scripts/nexus_lane_load_test.py",
):
    path = release_root / relative
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.write_bytes((relative + "\\n").encode())
    path.chmod(0o500)
(release_runner / "sealed-identity.json").write_bytes(identity_bytes)
(release_runner / "sealed-identity.json").chmod(0o400)

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

def full_artifact(path):
    metadata = path.stat()
    return {{
        "path": str(path),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "size_bytes": metadata.st_size,
        "mode": f"{{metadata.st_mode & 0o7777:04o}}",
        "owner_uid": metadata.st_uid,
        "nlink": metadata.st_nlink,
    }}

def artifact(path):
    return {{
        "path": str(path),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }}

def evidence_file(directory, name, data, mode=0o400):
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    path = directory / name
    path.write_bytes(data)
    path.chmod(mode)
    return path

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

g_unit_inventory = evidence_file(
    mock_directory, "g-unit-required-tests.tsv", b"leg_id\\tcrate\\ttest\\n"
)
formal_apalache = evidence_file(
    mock_directory, "multilane_apalache_evidence.tsv", b"apalache\\n"
)
formal_resource_jsonl = evidence_file(
    mock_directory, "tlaps_resource.jsonl", b'{{"event":"sample"}}\\n'
)
formal_resource_summary = evidence_file(
    mock_directory,
    "tlaps_resource_summary.json",
    b'{{"event":"summary"}}\\n',
)

prebuilt_root = (
    release_runner
    / "output"
    / "sumeragi-v2-release"
    / identity["workspace_source_manifest_sha256"]
    / "programs"
    / "invocation.test"
)
for directory in (
    prebuilt_root,
    prebuilt_root / "release",
    prebuilt_root / "message-control",
    prebuilt_root / "message-control" / "release",
):
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
prebuilt_specs = (
    ("irohad", "release/iroha3d"),
    ("irohad_message_control", "message-control/release/iroha3d"),
    ("iroha", "release/iroha"),
    ("kagami", "release/kagami"),
)
prebuilt_binaries = []
for role, relative in prebuilt_specs:
    binary = evidence_file(
        prebuilt_root / Path(relative).parent,
        Path(relative).name,
        (role + "\\n").encode(),
        0o500,
    )
    prebuilt_binaries.append(
        {{"role": role, "relative_path": relative, **full_artifact(binary)}}
    )
cargo_version = b"cargo 1.0.0\\n"
rustc_version = b"rustc 1.0.0\\n"
cargo_tool = Path(runner["tools"]["cargo"]["source_path"])
rustc_tool = Path(runner["tools"]["rustc"]["source_path"])
prebuilt_manifest_rows = [
    ("schema_version", "2"),
    ("source_manifest_sha256", identity["workspace_source_manifest_sha256"]),
    ("cargo_lock_sha256", identity["cargo_lock_sha256"]),
    ("cargo_version_sha256", hashlib.sha256(cargo_version).hexdigest()),
    ("rustc_version_sha256", hashlib.sha256(rustc_version).hexdigest()),
    ("host_triple", "aarch64-apple-darwin"),
    ("target_triple", "aarch64-apple-darwin"),
    ("profile", "release"),
    ("bundle_dir", str(prebuilt_root)),
]
for record in prebuilt_binaries:
    role = record["role"]
    prebuilt_manifest_rows.extend(
        (
            (f"{{role}}_relative_path", record["relative_path"]),
            (f"{{role}}_sha256", record["sha256"]),
            (f"{{role}}_size_bytes", str(record["size_bytes"])),
            (f"{{role}}_mode_octal", record["mode"]),
        )
    )
prebuilt_manifest = evidence_file(
    prebuilt_root,
    ".sumeragi-v2-prebuilt-binaries.tsv",
    "".join(f"{{key}}\\t{{value}}\\n" for key, value in prebuilt_manifest_rows).encode(),
)
for directory in (
    prebuilt_root / "message-control" / "release",
    prebuilt_root / "message-control",
    prebuilt_root / "release",
    prebuilt_root,
):
    directory.chmod(0o500)
prebuilt_bundle = {{
    "schema_version": 2,
    "manifest": full_artifact(prebuilt_manifest),
    "source_manifest_sha256": identity["workspace_source_manifest_sha256"],
    "cargo_lock_sha256": identity["cargo_lock_sha256"],
    "cargo_version_sha256": hashlib.sha256(cargo_version).hexdigest(),
    "rustc_version_sha256": hashlib.sha256(rustc_version).hexdigest(),
    "host_triple": "aarch64-apple-darwin",
    "target_triple": "aarch64-apple-darwin",
    "profile": "release",
    "bundle_dir": str(prebuilt_root),
    "artifact_root": str(release_runner / "output"),
    "cargo_target_root": str(release_runner / "target"),
    "version_transcripts": {{
        "cargo": {{
            "argv": [str(cargo_tool), "--version"],
            "sha256": hashlib.sha256(cargo_version).hexdigest(),
            "size_bytes": len(cargo_version),
        }},
        "rustc": {{
            "argv": [str(rustc_tool), "-vV"],
            "sha256": hashlib.sha256(rustc_version).hexdigest(),
            "size_bytes": len(rustc_version),
        }},
    }},
    "binaries": prebuilt_binaries,
}}

scaling_root = release_runner / "output" / "scaling"
scaling_manifest = evidence_file(
    scaling_root, "scaling_evidence.json", b'{{"schema_version":1}}\\n'
)
scaling_summary = evidence_file(
    scaling_root / "runs", "summary.log", b"scaling summary\\n"
)
scaling_trial = evidence_file(
    scaling_root / "runs" / "pair-00", "trial.log", b"scaling trial\\n"
)
scaling_paths = (scaling_manifest, scaling_summary, scaling_trial)
scaling_files = [
    {{"relative_path": path.relative_to(scaling_root).as_posix(), **full_artifact(path)}}
    for path in sorted(scaling_paths)
]
retained_scaling_validator = (
    release_root / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py"
)
retained_tool_specs = (
    ("localnet", "scripts/deploy_localnet.sh"),
    ("load_generator", "scripts/tx_load.py"),
    ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
)
scaling_trust_anchors = {{
    "trial_harness_sha256": os.environ[
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256"
    ],
    "configuration_sha256": os.environ[
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256"
    ],
    "irohad_sha256": os.environ["IROHA_RELEASE_SCALING_IROHAD_SHA256"],
    "iroha_cli_sha256": os.environ["IROHA_RELEASE_SCALING_IROHA_CLI_SHA256"],
    "repository_root": str(release_root),
    "retained_tooling": [
        {{
            "role": role,
            "source_path": source_path,
            **full_artifact(release_root / source_path),
        }}
        for role, source_path in retained_tool_specs
    ],
}}

g4p_root = release_runner / "output" / "g4p"
g4p_completion = evidence_file(g4p_root, "COMPLETED.tsv", b"g4p completion\\n")
g4p_summary = evidence_file(g4p_root, "runs.tsv", b"g4p summary\\n")
g4p_names = (
    "run-00-nexus_and_streaming.log",
    "run-01-nexus_and_streaming.log",
    "run-02-nexus_and_streaming.log",
    "run-03-native_amx_routing.log",
)
g4p_logs = [
    evidence_file(g4p_root, name, (name + "\\n").encode()) for name in g4p_names
]
g4p_evidence = {{
    "schema_version": 1,
    "completion": full_artifact(g4p_completion),
    "run_summary": full_artifact(g4p_summary),
    "run_logs": [full_artifact(path) for path in g4p_logs],
}}

g12_seed_root = release_runner / "output" / "g12-seed"
g12_seed_completion = evidence_file(
    g12_seed_root, "COMPLETED.tsv", b"g12 seed completion\\n"
)
g12_seed_summary = evidence_file(g12_seed_root, "runs.tsv", b"g12 seed summary\\n")
g12_seed_logs = [
    evidence_file(
        g12_seed_root, f"seed-{{ordinal:02d}}.log", f"seed {{ordinal}}\\n".encode()
    )
    for ordinal in range(10)
]
g12_soak_root = release_runner / "output" / "g12-soak"
g12_soak_completion = evidence_file(
    g12_soak_root, "COMPLETED.tsv", b"g12 soak completion\\n"
)
g12_soak_log = evidence_file(g12_soak_root, "fault-soak.log", b"soak\\n")
g12_evidence = {{
    "seed_completion": full_artifact(g12_seed_completion),
    "seed_summary": full_artifact(g12_seed_summary),
    "seed_run_logs": [full_artifact(path) for path in g12_seed_logs],
    "fault_soak_completion": full_artifact(g12_soak_completion),
    "fault_soak_log": full_artifact(g12_soak_log),
}}

release_root.chmod(0o500)

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
        "g_unit_focused_test_inventory": artifact(g_unit_inventory),
        "corridor_logs": [],
        "prebuilt_binary_bundle": prebuilt_bundle,
        "formal_gate_log": {{}},
        "formal_proof_coverage": {{}},
        "formal_proof_evidence": {{}},
        "formal_multilane_apalache_evidence": artifact(formal_apalache),
        "formal_harness_lock": {{}},
        "formal_toolchain": {{}},
        "formal_tlaps_resource_jsonl": artifact(formal_resource_jsonl),
        "formal_tlaps_resource_summary": artifact(formal_resource_summary),
        "seed_matrix_summary": {{}},
        "seed_matrix_run_logs": [],
        "seed_matrix_localnet_manifest_index": {{}},
        "seed_matrix_localnet_manifests": [],
        "chaos_log": {{}},
        "taira_evidence": {{}},
        "taira_run_log": {{}},
        "multilane_scaling_bundle": {{
            "root": str(scaling_root),
            "file_count": len(scaling_files),
            "total_size_bytes": sum(record["size_bytes"] for record in scaling_files),
            "directories": ["runs", "runs/pair-00"],
            "files": scaling_files,
        }},
        "multilane_scaling_retained_validator": full_artifact(
            retained_scaling_validator
        ),
        "multilane_scaling_trust_anchors": scaling_trust_anchors,
        "g4p_multilane": g4p_evidence,
        "g12_cross_dataspace": g12_evidence,
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
    environment_probe = ""
    if observed_scaling_environment is not None:
        required = "\n".join(f': "${{{name}:?}}"' for name in SCALING_TRUST_ENV)
        values = " ".join(
            f"{shlex.quote(name)} \"${{{name}}}\"" for name in SCALING_TRUST_ENV
        )
        environment_probe = (
            f"{required}\n"
            f"printf '%s=%s\\n' {values}"
            f" > {shlex.quote(str(observed_scaling_environment))}"
        )
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
{environment_probe}
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
    receipt_validator_support: Path
    tool_manifest: Path
    git: Path
    ssh: Path
    bash: Path
    allowed: Path
    revocation: Path

    def arguments(self) -> list[str]:
        arguments = [
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
            "--receipt-validator-support", str(self.receipt_validator_support),
            "--expected-receipt-validator-support-sha256",
            _sha256(self.receipt_validator_support),
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
        scaling_environment = {
            **DEFAULT_SCALING_DIGESTS,
            SCALING_EVIDENCE_ENV: str(
                self.evidence
                / "release-runner"
                / "output"
                / "scaling"
                / "scaling_evidence.json"
            ),
        }
        for name in SCALING_TRUST_ENV:
            arguments.extend(
                ["--runner-environment", f"{name}={scaling_environment[name]}"]
            )
        return arguments

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
    receipt_validator_support = _write(
        trust / RECEIPT_VALIDATOR_SUPPORT.name,
        RECEIPT_VALIDATOR_SUPPORT.read_bytes(),
        0o400,
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
        receipt_validator_support,
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


def _replace_runner_environment(
    arguments: list[str], name: str, value: str
) -> list[str]:
    updated = arguments.copy()
    for index, argument in enumerate(updated[:-1]):
        if argument == "--runner-environment" and updated[index + 1].startswith(
            f"{name}="
        ):
            updated[index + 1] = f"{name}={value}"
            return updated
    raise AssertionError(f"runner environment {name} was not present")


def _relocate_receipt_evidence_root(
    evidence: dict[str, object], anchor_key: str, destination: Path
) -> None:
    anchor = evidence[anchor_key]
    assert isinstance(anchor, Path) and anchor.is_file()
    source = anchor.parent
    shutil.move(str(source), destination)

    def relocated(value: object) -> object:
        if not isinstance(value, Path):
            return value
        try:
            relative = value.relative_to(source)
        except ValueError:
            return value
        return destination / relative

    for key, value in tuple(evidence.items()):
        if isinstance(value, list):
            evidence[key] = [relocated(item) for item in value]
        else:
            evidence[key] = relocated(value)


def _rebind_bootstrap_trusted_input(
    evidence: dict[str, object],
    *,
    label: str,
    source: Path,
    archive_name: str,
    archive_mode: int,
) -> None:
    evidence_directory = evidence["bootstrap_evidence_dir"]
    marker_path = evidence["bootstrap_completion"]
    assert isinstance(evidence_directory, Path)
    assert isinstance(marker_path, Path)
    source = source.resolve(strict=True)
    source_metadata = source.stat()
    archive = _write(
        evidence_directory / archive_name,
        source.read_bytes(),
        archive_mode,
    )
    marker = json.loads(marker_path.read_text(encoding="utf-8"))
    digest = _sha256(source)
    marker["trusted_inputs"][label] = {
        "archive_name": archive_name,
        "archive_mode": f"{archive_mode:04o}",
        "observed_sha256": digest,
        "protected_sha256": digest,
        "size_bytes": source_metadata.st_size,
        "source_mode": f"{stat.S_IMODE(source_metadata.st_mode):04o}",
        "source_path": str(source),
    }
    _write(
        marker_path,
        (json.dumps(marker, sort_keys=True, separators=(",", ":")) + "\n").encode(),
        0o400,
    )
    evidence["expected_bootstrap_completion_sha256"] = _sha256(marker_path)
    assert archive == evidence_directory / archive_name


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
    support_record = value["trusted_inputs"]["receipt_validator_support"]
    assert support_record["archive_name"] == RECEIPT_VALIDATOR_SUPPORT.name
    assert support_record["protected_sha256"] == _sha256(
        release_fixture.receipt_validator_support
    )
    assert _sha256(release_fixture.evidence / RECEIPT_VALIDATOR_SUPPORT.name) == (
        support_record["protected_sha256"]
    )
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
        "g_unit_focused_test_inventory",
        "prebuilt_binary_bundle",
        "formal_verus_evidence",
        "formal_verus_log",
        "formal_multilane_apalache_evidence",
        "formal_cross_tool_evidence",
        "formal_tlaps_resource_jsonl",
        "formal_tlaps_resource_summary",
        "multilane_scaling_bundle",
        "multilane_scaling_retained_validator",
        "multilane_scaling_trust_anchors",
        "g4p_multilane",
        "g12_cross_dataspace",
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
    "field",
    [
        "g_unit_focused_test_inventory",
        "prebuilt_binary_bundle",
        "formal_multilane_apalache_evidence",
        "formal_tlaps_resource_jsonl",
        "formal_tlaps_resource_summary",
        "multilane_scaling_bundle",
        "multilane_scaling_retained_validator",
        "multilane_scaling_trust_anchors",
        "g4p_multilane",
        "g12_cross_dataspace",
    ],
)
def test_terminal_receipt_requires_every_extended_release_field(
    release_fixture: Fixture, field: str
) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            receipt_mutation_override=f'receipt["evidence"].pop({field!r})',
        ),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


@pytest.mark.parametrize(
    "mutation",
    [
        'receipt["evidence"]["g_unit_focused_test_inventory"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g_unit_focused_test_inventory"]["sha256"] = "0" * 64',
        'receipt["evidence"]["formal_multilane_apalache_evidence"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["formal_multilane_apalache_evidence"]["sha256"] = "0" * 64',
        'receipt["evidence"]["formal_tlaps_resource_jsonl"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["formal_tlaps_resource_jsonl"]["sha256"] = "0" * 64',
        'receipt["evidence"]["formal_tlaps_resource_summary"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["formal_tlaps_resource_summary"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["size_bytes"] += 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["mode"] = "0500"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["artifact_root"] = str(candidate)',
        'receipt["evidence"]["prebuilt_binary_bundle"]["cargo_target_root"] = receipt["evidence"]["prebuilt_binary_bundle"]["artifact_root"]',
        'receipt["evidence"]["prebuilt_binary_bundle"]["schema_version"] = 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["source_manifest_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["cargo_lock_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["cargo_version_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["rustc_version_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["host_triple"] = "invalid"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["target_triple"] = "x86_64-unknown-linux-gnu"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["profile"] = "debug"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["bundle_dir"] = str(candidate)',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["argv"][0] = str(candidate / "payload")',
        pytest.param(
            'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["argv"][0] = str(release_root / "scripts" / "run_sumeragi_v2_release_gates.sh")',
            id="prebuilt-cargo-transcript-authenticated-tool-substitution",
        ),
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["argv"][1] = "-V"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["size_bytes"] = 0',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["argv"][0] = str(candidate / "payload")',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["argv"][1] = "--version"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["size_bytes"] = 65537',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["role"] = "wrong"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["relative_path"] = "release/wrong"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["size_bytes"] += 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["mode"] = "0400"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["owner_uid"] += 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["nlink"] += 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"].pop()',
        'receipt["evidence"]["multilane_scaling_bundle"]["root"] = str(candidate)',
        'receipt["evidence"]["multilane_scaling_bundle"]["file_count"] += 1',
        'receipt["evidence"]["multilane_scaling_bundle"]["total_size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["relative_path"] = "../escape"',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["mode"] = "0500"',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["owner_uid"] += 1',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["nlink"] += 1',
        (
            '[record for record in receipt["evidence"]["multilane_scaling_bundle"]["files"] '
            'if record["relative_path"] == "scaling_evidence.json"][0]["relative_path"] '
            '= "missing-scaling-evidence.json"'
        ),
        (
            '[record for record in receipt["evidence"]["multilane_scaling_bundle"]["files"] '
            'if record["relative_path"] == "scaling_evidence.json"][0]["path"] '
            '= str(candidate / "payload")'
        ),
        (
            '[record for record in receipt["evidence"]["multilane_scaling_bundle"]["files"] '
            'if record["relative_path"] == "scaling_evidence.json"][0]["sha256"] '
            '= "0" * 64'
        ),
        'receipt["evidence"]["multilane_scaling_bundle"]["directories"].append("missing")',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["mode"] = "0400"',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["owner_uid"] += 1',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["nlink"] += 1',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["trial_harness_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["configuration_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["irohad_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["iroha_cli_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["repository_root"] = str(candidate)',
        (
            'receipt["evidence"]["multilane_scaling_trust_anchors"]'
            '["retained_tooling"][0]["source_path"] = "scripts/wrong.sh"'
        ),
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["role"] = "wrong"',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["mode"] = "0400"',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["owner_uid"] += 1',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["nlink"] += 1',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"].pop()',
        'receipt["evidence"]["g4p_multilane"]["schema_version"] = 2',
        'receipt["evidence"]["g4p_multilane"]["completion"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g4p_multilane"]["completion"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g4p_multilane"]["completion"]["size_bytes"] += 1',
        'receipt["evidence"]["g4p_multilane"]["completion"]["mode"] = "0500"',
        'receipt["evidence"]["g4p_multilane"]["completion"]["owner_uid"] += 1',
        'receipt["evidence"]["g4p_multilane"]["completion"]["nlink"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["size_bytes"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["mode"] = "0500"',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["owner_uid"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["nlink"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["size_bytes"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["mode"] = "0500"',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["owner_uid"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["nlink"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"].pop()',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"].pop()',
        (
            'receipt["evidence"]["g12_cross_dataspace"]'
            '["fault_soak_completion"]["path"] = str(candidate / "payload")'
        ),
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["nlink"] += 1',
    ],
)
def test_terminal_receipt_extended_artifact_mutations_fail_closed(
    release_fixture: Fixture, mutation: str
) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            receipt_mutation_override=mutation,
        ),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def _assert_terminal_receipt_mutation_rejected(
    release_fixture: Fixture, mutation: str
) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            receipt_mutation_override=mutation,
        ),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


@pytest.mark.parametrize(
    "mutation",
    [
        pytest.param(
            "evidence_file(g4p_root, 'untracked.log', b'untracked G-4P file\\n')",
            id="g4p",
        ),
        pytest.param(
            "evidence_file(g12_seed_root, 'untracked.log', b'untracked G-12 seed file\\n')",
            id="g12-seed",
        ),
        pytest.param(
            "evidence_file(g12_soak_root, 'untracked.log', b'untracked G-12 soak file\\n')",
            id="g12-soak",
        ),
        pytest.param(
            "evidence_file(scaling_root, 'untracked.log', b'untracked scaling file\\n')",
            id="scaling",
        ),
    ],
)
def test_terminal_receipt_rejects_extra_live_closed_inventory_files(
    release_fixture: Fixture, mutation: str
) -> None:
    _assert_terminal_receipt_mutation_rejected(release_fixture, mutation)


@pytest.mark.parametrize(
    "mutation",
    [
        pytest.param(
            'receipt["evidence"]["multilane_scaling_bundle"]["files"].reverse()',
            id="unsorted-files",
        ),
        pytest.param(
            'files = receipt["evidence"]["multilane_scaling_bundle"]["files"]\n'
            "files.append(dict(files[-1]))\n"
            'receipt["evidence"]["multilane_scaling_bundle"]["file_count"] += 1\n'
            'receipt["evidence"]["multilane_scaling_bundle"]["total_size_bytes"] += files[-1]["size_bytes"]',
            id="duplicate-files",
        ),
        pytest.param(
            'receipt["evidence"]["multilane_scaling_bundle"]["directories"].reverse()',
            id="unsorted-directories",
        ),
        pytest.param(
            'directories = receipt["evidence"]["multilane_scaling_bundle"]["directories"]\n'
            "directories.append(directories[-1])",
            id="duplicate-directories",
        ),
        pytest.param(
            'receipt["evidence"]["multilane_scaling_bundle"]["directories"][0] = "../escape"',
            id="directory-traversal",
        ),
    ],
)
def test_terminal_receipt_rejects_duplicate_or_unsorted_scaling_inventory(
    release_fixture: Fixture, mutation: str
) -> None:
    _assert_terminal_receipt_mutation_rejected(release_fixture, mutation)


def test_terminal_receipt_rejects_g12_seed_soak_root_alias(
    release_fixture: Fixture,
) -> None:
    _assert_terminal_receipt_mutation_rejected(
        release_fixture,
        (
            'g12 = receipt["evidence"]["g12_cross_dataspace"]\n'
            'g12["fault_soak_completion"] = dict(g12["seed_completion"])'
        ),
    )


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
        "--expected-receipt-validator-support-sha256",
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


def test_receipt_validator_support_omission_never_launches(
    release_fixture: Fixture,
) -> None:
    arguments = release_fixture.arguments()
    index = arguments.index("--receipt-validator-support")
    del arguments[index : index + 2]

    result = release_fixture.run(arguments)

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


@pytest.mark.parametrize(
    "input_flag",
    [
        "--git-bin",
        "--manifest-helper",
        "--receipt-validator-support",
        "--ssh-allowed-signers",
    ],
)
def test_candidate_contained_trust_input_never_launches(
    release_fixture: Fixture, input_flag: str
) -> None:
    source = {
        "--git-bin": release_fixture.git,
        "--manifest-helper": release_fixture.manifest,
        "--receipt-validator-support": release_fixture.receipt_validator_support,
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
        "--receipt-validator-support": (
            "--expected-receipt-validator-support-sha256"
        ),
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


@pytest.mark.parametrize(
    "action",
    [
        "source-drift",
        "evidence-tamper",
        "marker-tamper",
        "directory-mode-tamper",
        "receipt-support-archive-omission",
        "receipt-support-archive-substitution",
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


def test_post_launch_receipt_support_source_drift_fails_closed(
    release_fixture: Fixture,
) -> None:
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "trusted-drift",
            trusted_mutation=release_fixture.receipt_validator_support,
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


def test_bootstrap_protected_validation_accepts_real_terminal_receipt(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    from pytests.scripts import (
        sumeragi_v2_release_receipt_test as receipt_contract,
    )

    fixture_root = tmp_path / "nested" / "real-receipt"
    fixture_root.mkdir(parents=True)
    evidence: dict[str, object] = receipt_contract.make_evidence(fixture_root)
    writer = receipt_contract.fixture_writer(fixture_root)
    bootstrap_evidence = evidence["bootstrap_evidence_dir"]
    assert isinstance(bootstrap_evidence, Path)
    release_output = bootstrap_evidence / "release-runner" / "output"
    for anchor_key, directory_name in (
        ("corridor_completion", "corridor"),
        ("formal_completion", "formal"),
        ("seed_completion", "seed"),
        ("chaos_completion", "chaos"),
        ("taira_completion", "taira"),
        ("scaling_manifest", "scaling"),
        ("g4p_completion", "g4p"),
        ("g12_seed_completion", "g12-seed"),
        ("g12_soak_completion", "g12-soak"),
    ):
        _relocate_receipt_evidence_root(
            evidence, anchor_key, release_output / directory_name
        )

    marker_path = evidence["bootstrap_completion"]
    scaling_manifest = evidence["scaling_manifest"]
    assert isinstance(marker_path, Path)
    assert isinstance(scaling_manifest, Path)
    marker = json.loads(marker_path.read_text(encoding="utf-8"))
    marker["runner"]["environment_without_self_digest"][
        SCALING_EVIDENCE_ENV
    ] = str(scaling_manifest)
    _write(
        marker_path,
        (json.dumps(marker, sort_keys=True, separators=(",", ":")) + "\n").encode(),
        0o400,
    )
    evidence["expected_bootstrap_completion_sha256"] = _sha256(marker_path)

    _rebind_bootstrap_trusted_input(
        evidence,
        label="receipt_validator",
        source=writer,
        archive_name="validate-receipt.py",
        archive_mode=0o400,
    )
    _rebind_bootstrap_trusted_input(
        evidence,
        label="python",
        source=PYTHON,
        archive_name="python3",
        archive_mode=0o500,
    )
    sealed_source = evidence["sealed"]
    bootstrap_identity = evidence["bootstrap_identity"]
    assert isinstance(sealed_source, Path)
    assert isinstance(bootstrap_identity, Path)
    sealed_identity = _write(
        bootstrap_evidence / "release-runner" / "sealed-identity.json",
        sealed_source.read_bytes(),
        0o400,
    )
    evidence["candidate"] = bootstrap_identity
    evidence["sealed"] = sealed_identity

    writer_tmp = fixture_root / "writer-tmp"
    writer_tmp.mkdir()
    monkeypatch.setenv("TMPDIR", str(writer_tmp))
    terminal_output = evidence["terminal_output"]
    assert isinstance(terminal_output, Path)
    publication = receipt_contract.run_writer(
        evidence, terminal_output, writer
    )
    assert publication.returncode == 0, publication.stderr
    receipt_before = terminal_output.read_bytes()
    for log_name in ("runner-stdout.log", "runner-stderr.log"):
        (bootstrap_evidence / log_name).chmod(0o400)

    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_real_receipt", BOOTSTRAP
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)

    def snapshot(path: Path, label: str, maximum_bytes: int) -> object:
        return module._read_file(path, label, maximum_bytes=maximum_bytes)

    marker = json.loads(marker_path.read_text(encoding="utf-8"))
    trusted_inputs = marker["trusted_inputs"]
    archives = {
        "python": snapshot(
            bootstrap_evidence / "python3", "archived Python", module._MAX_TOOL_BYTES
        ),
        "receipt_validator": snapshot(
            bootstrap_evidence / "validate-receipt.py",
            "archived receipt validator",
            module._MAX_HELPER_BYTES,
        ),
        "receipt_validator_support": snapshot(
            bootstrap_evidence / RECEIPT_VALIDATOR_SUPPORT.name,
            "archived receipt validator support",
            module._MAX_HELPER_BYTES,
        ),
    }
    protected = {
        label: snapshot(
            Path(trusted_inputs[label]["source_path"]),
            f"protected {label}",
            (
                module._MAX_POLICY_BYTES
                if label in {"allowed_signers", "revocation"}
                else module._MAX_TOOL_BYTES
            ),
        )
        for label in ("git", "ssh_keygen", "allowed_signers", "revocation")
    }
    identity_outputs = {
        "attestation": evidence["bootstrap_attestation"],
        "transcript": evidence["bootstrap_transcript"],
        "raw_commit": evidence["bootstrap_identity_raw_commit"],
        "cargo_lock": evidence["bootstrap_identity_cargo_lock"],
        "allowed": evidence["bootstrap_identity_allowed_signers"],
        "revocation": evidence["bootstrap_identity_revocation"],
        "git": evidence["bootstrap_identity_git"],
        "ssh": evidence["bootstrap_identity_ssh_keygen"],
    }
    assert all(isinstance(path, Path) for path in identity_outputs.values())
    environment = dict(marker["runner"]["environment_without_self_digest"])
    marker_sha256 = _sha256(marker_path)
    environment.update(
        {
            "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": marker_sha256,
            "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": (
                marker_sha256
            ),
        }
    )
    receipt = json.loads(receipt_before)
    validation = module._run_protected_receipt_validator(
        evidence=bootstrap_evidence,
        candidate=Path(evidence["bootstrap_candidate_root"]),
        receipt=receipt,
        receipt_snapshot=snapshot(
            terminal_output,
            "terminal receipt",
            module._MAX_TERMINAL_RECEIPT_BYTES,
        ),
        sealed_identity_snapshot=snapshot(
            sealed_identity, "sealed identity", module._MAX_IDENTITY_BYTES
        ),
        sealed_root=Path(evidence["release_root"]),
        archives=archives,
        protected=protected,
        identity_snapshot=snapshot(
            bootstrap_identity,
            "bootstrap identity",
            module._MAX_IDENTITY_BYTES,
        ),
        identity_outputs=identity_outputs,
        bootstrap_marker=snapshot(
            marker_path, "bootstrap marker", module._MAX_EVIDENCE_BYTES
        ),
        expected_signer_fingerprint=str(evidence["expected_signer_fingerprint"]),
        environment=environment,
        timeout_seconds=30,
    )

    assert validation.returncode == 0
    assert terminal_output.read_bytes() == receipt_before
    assert not (bootstrap_evidence / "__pycache__").exists()


def test_full_bootstrap_succeeds_with_real_terminal_receipt_validator(
    release_fixture: Fixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    from pytests.scripts import (
        sumeragi_v2_release_receipt_test as receipt_contract,
    )

    fixture_root = release_fixture.root / "full-bootstrap-real-receipt"
    fixture_root.mkdir()
    evidence: dict[str, object] = receipt_contract.make_evidence(fixture_root)
    writer = receipt_contract.fixture_writer(fixture_root)
    bootstrap_evidence = evidence["bootstrap_evidence_dir"]
    assert isinstance(bootstrap_evidence, Path)
    release_output = bootstrap_evidence / "release-runner" / "output"
    for anchor_key, directory_name in (
        ("corridor_completion", "corridor"),
        ("formal_completion", "formal"),
        ("seed_completion", "seed"),
        ("chaos_completion", "chaos"),
        ("taira_completion", "taira"),
        ("g4p_completion", "g4p"),
        ("g12_seed_completion", "g12-seed"),
        ("g12_soak_completion", "g12-soak"),
    ):
        _relocate_receipt_evidence_root(
            evidence, anchor_key, release_output / directory_name
        )

    # Scaling evidence is intentionally external to the bootstrap evidence
    # tree. Its absolute root and digests are authenticated runner inputs.
    scaling_manifest = evidence["scaling_manifest"]
    assert isinstance(scaling_manifest, Path)
    assert bootstrap_evidence not in scaling_manifest.parents

    _rebind_bootstrap_trusted_input(
        evidence,
        label="receipt_validator",
        source=writer,
        archive_name="validate-receipt.py",
        archive_mode=0o400,
    )
    _rebind_bootstrap_trusted_input(
        evidence,
        label="python",
        source=PYTHON,
        archive_name="python3",
        archive_mode=0o500,
    )
    sealed_source = evidence["sealed"]
    bootstrap_identity = evidence["bootstrap_identity"]
    release_root = evidence["release_root"]
    assert isinstance(sealed_source, Path)
    assert isinstance(bootstrap_identity, Path)
    assert isinstance(release_root, Path)
    sealed_identity = _write(
        bootstrap_evidence / "release-runner" / "sealed-identity.json",
        sealed_source.read_bytes(),
        0o400,
    )
    evidence["candidate"] = bootstrap_identity
    evidence["sealed"] = sealed_identity

    writer_tmp = fixture_root / "writer-tmp"
    writer_tmp.mkdir()
    monkeypatch.setenv("TMPDIR", str(writer_tmp))
    terminal_output = evidence["terminal_output"]
    assert isinstance(terminal_output, Path)
    publication = receipt_contract.run_writer(
        evidence, terminal_output, writer
    )
    assert publication.returncode == 0, publication.stderr
    terminal_output.unlink()
    release_root.chmod(0o500)
    candidate_identity_json = bootstrap_identity.read_text(
        encoding="utf-8"
    ).strip()
    sealed_identity_json = sealed_source.read_text(encoding="utf-8").strip()

    staged_bootstrap = fixture_root / "prepared-bootstrap"
    shutil.move(str(bootstrap_evidence), staged_bootstrap)
    staged_release_runner = staged_bootstrap / "release-runner"
    assert staged_release_runner.is_dir()
    assert not bootstrap_evidence.exists()
    release_fixture.evidence = bootstrap_evidence

    release_fixture.manifest = _write(
        release_fixture.trust / "fixed-release-identity.py",
        f'''#!/usr/bin/env python3
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--root", type=Path, required=True)
parser.add_argument("--release-identity-json", action="store_true", required=True)
args = parser.parse_args()
root = args.root.resolve(strict=True)
if root == Path({str(release_fixture.candidate)!r}):
    print({candidate_identity_json!r})
elif root == Path({str(release_root)!r}):
    print({sealed_identity_json!r})
else:
    raise SystemExit(71)
''',
        0o500,
    )
    release_fixture.verifier = _write(
        release_fixture.trust / "real-identity-verifier.py",
        (
            REPO_ROOT / "scripts" / "verify_sumeragi_v2_release_identity.py"
        ).read_bytes(),
        0o500,
    )
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "real-receipt-validator.py",
        writer.read_bytes(),
        0o500,
    )
    for attribute, evidence_key, filename, mode in (
        ("git", "signature_git", "real-git", 0o500),
        ("ssh", "signature_ssh_keygen", "real-ssh-keygen", 0o500),
        ("allowed", "signature_allowed_signers", "real-allowed-signers", 0o400),
        ("revocation", "signature_revocation", "real-revocation", 0o400),
    ):
        source = evidence[evidence_key]
        assert isinstance(source, Path)
        setattr(
            release_fixture,
            attribute,
            _write(release_fixture.trust / filename, source.read_bytes(), mode),
        )
    signature_cargo_lock = evidence["signature_cargo_lock"]
    assert isinstance(signature_cargo_lock, Path)
    _write(
        release_fixture.candidate / "Cargo.lock",
        signature_cargo_lock.read_bytes(),
    )
    runner_tool_manifest = json.loads(
        release_fixture.tool_manifest.read_text(encoding="utf-8")
    )
    for name in ("cargo", "rustc"):
        source = evidence[f"bootstrap_runner_{name}"]
        assert isinstance(source, Path)
        runner_tool_manifest["tools"][name] = {
            "path": str(source.resolve(strict=True)),
            "sha256": _sha256(source),
        }
    _write(
        release_fixture.tool_manifest,
        json.dumps(
            runner_tool_manifest, sort_keys=True, separators=(",", ":")
        )
        + "\n",
        0o400,
    )

    def evidence_path(name: str) -> str:
        path = evidence[name]
        assert isinstance(path, Path)
        return shlex.quote(str(path))

    runner = f'''#!/bin/bash
set -eu
: "${{SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256:?}}"
count=0
if test -f {shlex.quote(str(release_fixture.launch_count))}; then
    count=$(<{shlex.quote(str(release_fixture.launch_count))})
fi
count=$((count + 1))
printf '%s\\n' "$count" > {shlex.quote(str(release_fixture.launch_count))}
mv {shlex.quote(str(staged_release_runner))} \
    "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/release-runner"
python3 -I -S "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/validate-receipt.py" \
    --candidate-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY" \
    --sealed-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/release-runner/sealed-identity.json" \
    --release-root {shlex.quote(str(release_root))} \
    --signature-attestation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION" \
    --signature-transcript "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT" \
    --signature-raw-commit "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-raw-commit" \
    --signature-cargo-lock "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-Cargo.lock" \
    --signature-allowed-signers "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-allowed-signers" \
    --signature-revocation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-revocation" \
    --signature-git "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-git" \
    --signature-ssh-keygen "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-ssh-keygen" \
    --expected-git-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256" \
    --expected-ssh-keygen-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256" \
    --expected-allowed-signers-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256" \
    --expected-revocation-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256" \
    --expected-signer-fingerprint "$SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT" \
    --bootstrap-completion "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION" \
    --bootstrap-evidence-dir "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" \
    --bootstrap-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY" \
    --bootstrap-attestation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION" \
    --bootstrap-transcript "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT" \
    --expected-bootstrap-completion-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256" \
    --bootstrap-candidate-root {shlex.quote(str(release_fixture.candidate))} \
    --bootstrap-runner {shlex.quote(str(release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"))} \
    --corridor-completion {evidence_path("corridor_completion")} \
    --formal-completion {evidence_path("formal_completion")} \
    --seed-completion {evidence_path("seed_completion")} \
    --chaos-completion {evidence_path("chaos_completion")} \
    --taira-completion {evidence_path("taira_completion")} \
    --g4p-completion {evidence_path("g4p_completion")} \
    --g12-seed-completion {evidence_path("g12_seed_completion")} \
    --g12-fault-soak-completion {evidence_path("g12_soak_completion")} \
    --scaling-evidence-manifest {shlex.quote(str(scaling_manifest))} \
    --expected-scaling-trial-harness-sha256 "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
    --expected-scaling-configuration-sha256 "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
    --expected-scaling-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
    --expected-scaling-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \
    --repository-root {shlex.quote(str(release_root))} \
    --output {shlex.quote(str(terminal_output))}
'''
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        runner,
        0o500,
    )

    scaling_environment = {
        SCALING_EVIDENCE_ENV: str(scaling_manifest),
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": str(
            evidence["expected_scaling_trial_harness_sha256"]
        ),
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": str(
            evidence["expected_scaling_configuration_sha256"]
        ),
        "IROHA_RELEASE_SCALING_IROHAD_SHA256": str(
            evidence["expected_scaling_irohad_sha256"]
        ),
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": str(
            evidence["expected_scaling_iroha_cli_sha256"]
        ),
    }
    arguments = release_fixture.arguments()
    for name, value in scaling_environment.items():
        arguments = _replace_runner_environment(arguments, name, value)
    arguments = _replace_flag(arguments, "--command-timeout-seconds", "20")

    result = release_fixture.run(arguments)

    assert result.returncode == 0, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert (release_fixture.evidence / "BOOTSTRAP_RELEASE_COMPLETED.json").is_file()
    marker = json.loads(
        (release_fixture.evidence / "BOOTSTRAP_COMPLETED.json").read_text(
            encoding="utf-8"
        )
    )
    assert marker["runner"]["environment_without_self_digest"][
        SCALING_EVIDENCE_ENV
    ] == str(scaling_manifest)
    receipt = json.loads(terminal_output.read_text(encoding="utf-8"))
    assert receipt["evidence"]["multilane_scaling_bundle"]["root"] == str(
        scaling_manifest.parent.resolve(strict=True)
    )
    assert release_fixture.evidence not in scaling_manifest.parents


def test_bootstrap_invokes_real_terminal_receipt_validator(
    release_fixture: Fixture,
) -> None:
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "real-receipt-validator.py",
        (REPO_ROOT / "scripts" / "write_sumeragi_v2_release_receipt.py").read_bytes(),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "protected receipt validator rejected terminal receipt" in result.stderr
    assert "Sumeragi v2 release receipt error:" in result.stderr
    assert "the following arguments are required" not in result.stderr
    assert "unrecognized arguments" not in result.stderr
    assert not release_fixture.evidence.exists()


@pytest.mark.parametrize(
    ("binding", "needle", "replacement"),
    [
        pytest.param(
            "g4p-completion",
            'receipt, ("g4p_multilane", "completion"), evidence',
            'receipt, ("g12_cross_dataspace", "seed_completion"), evidence',
            id="g4p-completion-source",
        ),
        pytest.param(
            "g12-seed-completion",
            'receipt, ("g12_cross_dataspace", "seed_completion"), evidence',
            'receipt, ("g4p_multilane", "completion"), evidence',
            id="g12-seed-completion-source",
        ),
        pytest.param(
            "g12-fault-soak-completion",
            '("g12_cross_dataspace", "fault_soak_completion"),',
            '("g12_cross_dataspace", "seed_completion"),',
            id="g12-fault-soak-completion-source",
        ),
        pytest.param(
            "scaling-evidence-manifest",
            "str(_receipt_scaling_manifest_path(receipt))",
            (
                "str(_receipt_nested_artifact_path("
                'receipt, ("g4p_multilane", "completion"), evidence))'
            ),
            id="scaling-manifest-source",
        ),
        pytest.param(
            "scaling-trial-harness-digest",
            'scaling_digests["trial_harness_sha256"]',
            'scaling_digests["configuration_sha256"]',
            id="scaling-trial-harness-value",
        ),
        pytest.param(
            "scaling-configuration-digest",
            'scaling_digests["configuration_sha256"]',
            'scaling_digests["trial_harness_sha256"]',
            id="scaling-configuration-value",
        ),
        pytest.param(
            "scaling-irohad-digest",
            'scaling_digests["irohad_sha256"]',
            'scaling_digests["iroha_cli_sha256"]',
            id="scaling-irohad-value",
        ),
        pytest.param(
            "scaling-iroha-cli-digest",
            'scaling_digests["iroha_cli_sha256"]',
            'scaling_digests["irohad_sha256"]',
            id="scaling-iroha-cli-value",
        ),
    ],
)
def test_protected_receipt_validator_extended_value_source_mutations_fail_closed(
    release_fixture: Fixture,
    binding: str,
    needle: str,
    replacement: str,
) -> None:
    source = BOOTSTRAP.read_text(encoding="utf-8")
    assert source.count(needle) == 1
    mutated = _write(
        release_fixture.trust / f"bootstrap-{binding}.py",
        source.replace(needle, replacement, 1),
        0o500,
    )
    arguments = release_fixture.arguments()
    arguments[3] = str(mutated)
    arguments = _replace_flag(
        arguments, "--expected-bootstrap-sha256", _sha256(mutated)
    )

    result = release_fixture.run(arguments)

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "protected receipt validator rejected terminal receipt" in result.stderr
    assert not release_fixture.evidence.exists()


@pytest.mark.parametrize(
    "mutation",
    [
        pytest.param(
            "target = Path(args.scaling_evidence_manifest)\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late artifact mutation\\n')",
            id="scaling-artifact",
        ),
        pytest.param(
            "target = Path(args.g4p_completion).parent / 'late.log'\n"
            "target.write_bytes(b'late directory mutation\\n')\n"
            "target.chmod(0o400)",
            id="g4p-directory-inventory",
        ),
        pytest.param(
            "target = Path(args.g12_seed_completion).parent / 'seed-00.log'\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late G-12 seed-log mutation\\n')",
            id="g12-seed-log",
        ),
        pytest.param(
            "target = Path(args.g12_fault_soak_completion).parent / 'fault-soak.log'\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late G-12 soak-log mutation\\n')",
            id="g12-fault-soak-log",
        ),
        pytest.param(
            "target = next(Path(args.release_root).glob(\n"
            "    'target/sumeragi-v2-release/*/programs/*/release/iroha'\n"
            "))\n"
            "target.chmod(0o700)\n"
            "target.write_bytes(b'late prebuilt mutation\\n')",
            id="prebuilt-binary",
        ),
        pytest.param(
            "target = Path(args.formal_completion).parent / 'tlaps_resource.jsonl'\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late formal mutation\\n')",
            id="formal-tlaps-resource",
        ),
    ],
)
def test_protected_validator_cannot_mutate_nested_terminal_evidence(
    release_fixture: Fixture, mutation: str
) -> None:
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "mutating-receipt-validator.py",
        _receipt_validator(mutation),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "changed" in result.stderr
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


def test_scaling_evidence_runner_environment_is_authenticated_and_forwarded(
    release_fixture: Fixture,
) -> None:
    scaling_manifest = (
        release_fixture.evidence
        / "release-runner"
        / "output"
        / "scaling"
        / "scaling_evidence.json"
    )
    observed_environment = release_fixture.root / "observed-scaling-environment"
    _write(
        release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            observed_scaling_environment=observed_environment,
        ),
        0o500,
    )

    scaling_environment = {
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": "a" * 64,
        SCALING_EVIDENCE_ENV: str(scaling_manifest),
        "IROHA_RELEASE_SCALING_IROHAD_SHA256": "b" * 64,
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": "c" * 64,
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": "d" * 64,
    }
    arguments = [*release_fixture.arguments()]
    for name in SCALING_TRUST_ENV:
        arguments = _replace_runner_environment(
            arguments, name, scaling_environment[name]
        )
    result = release_fixture.run(arguments)

    assert result.returncode == 0, result.stderr
    assert dict(
        line.split("=", 1)
        for line in observed_environment.read_text(encoding="utf-8").splitlines()
    ) == scaling_environment
    marker = json.loads(
        (release_fixture.evidence / "BOOTSTRAP_COMPLETED.json").read_text(
            encoding="utf-8"
        )
    )
    authenticated_environment = marker["runner"]["environment_without_self_digest"]
    assert {
        name: authenticated_environment[name] for name in SCALING_TRUST_ENV
    } == scaling_environment
    assert sorted(
        name
        for name in authenticated_environment
        if name.startswith("IROHA_RELEASE_SCALING_")
    ) == sorted(SCALING_TRUST_ENV)


@pytest.mark.parametrize(
    "name",
    [
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST_",
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST_PATH",
        "IROHA_RELEASE_SCALING_IROHAD_SHA256_PATH",
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256_",
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_DIGEST",
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA512",
        "SUMERAGI_V2_RELEASE_SCALING_EVIDENCE_MANIFEST",
    ],
)
def test_scaling_evidence_runner_environment_lookalikes_are_rejected(
    release_fixture: Fixture,
    name: str,
) -> None:
    result = release_fixture.run(
        [
            *release_fixture.arguments(),
            "--runner-environment",
            f"{name}=/tmp/scaling_evidence.json",
        ]
    )

    _assert_never_launched(release_fixture, result)
    assert "explicitly allowed NAME=VALUE" in result.stderr


def test_candidate_runner_symlink_never_launches(release_fixture: Fixture) -> None:
    runner = release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"
    target = release_fixture.root / "outside-runner"
    shutil.move(runner, target)
    runner.symlink_to(target)
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)
