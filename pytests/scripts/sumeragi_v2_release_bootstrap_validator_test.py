"""Adversarial tests for the Sumeragi v2 bootstrap completion validator."""

from __future__ import annotations

from dataclasses import dataclass
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
from typing import Any

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = REPO_ROOT / "scripts" / "validate_sumeragi_v2_release_bootstrap.py"
PYTHON = Path(sys.executable).resolve(strict=True)
FINGERPRINT = "SHA256:" + "A" * 43
OTHER_FINGERPRINT = "SHA256:" + "B" * 43

TRUSTED_NAMES = {
    "allowed_signers": "bootstrap-allowed-signers",
    "bash": "bash",
    "bootstrap": "trusted-bootstrap.py",
    "git": "git",
    "identity_verifier": "verify-identity.py",
    "manifest_helper": "compute-manifest.py",
    "python": "python3",
    "receipt_validator": "validate-receipt.py",
    "revocation": "bootstrap-revocation",
    "runner_tool_manifest": "runner-tool-manifest.json",
    "ssh_keygen": "ssh-keygen",
}
IDENTITY_NAMES = {
    "cargo_lock": "identity-Cargo.lock",
    "git": "identity-git",
    "raw_commit": "identity-raw-commit",
    "ssh_allowed_signers": "identity-allowed-signers",
    "ssh_keygen": "identity-ssh-keygen",
    "ssh_revocation": "identity-revocation",
    "verify_transcript": "identity-transcript.json",
}


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
        attestation["evidence"]["verify_transcript"].update(
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
        attestation["evidence"]["raw_commit"].update(
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
                "observed_sha256": _digest(data),
                "protected_sha256": _digest(data),
                "size_bytes": len(data),
                "source_mode": f"{mode:04o}",
            }
        )
        self.seal_marker(marker)

    def reseal_allowed_policy(self, data: bytes) -> None:
        _write(self.evidence / "bootstrap-allowed-signers", data, 0o400)
        _write(self.trust / "source-allowed_signers", data, 0o400)
        _write(self.evidence / "identity-allowed-signers", data, 0o400)
        attestation = json.loads((self.evidence / "identity-attestation.json").read_bytes())
        attestation["policies"]["ssh_allowed_signers"].update(
            {
                "observed_sha256": _digest(data),
                "protected_sha256": _digest(data),
                "size_bytes": len(data),
            }
        )
        attestation["evidence"]["ssh_allowed_signers"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        transcript = json.loads((self.evidence / "identity-transcript.json").read_bytes())
        transcript["policies"] = attestation["policies"]
        transcript_data = _canonical(transcript)
        _write(self.evidence / "identity-transcript.json", transcript_data, 0o400)
        attestation["evidence"]["verify_transcript"].update(
            {"sha256": _digest(transcript_data), "size_bytes": len(transcript_data)}
        )
        attestation_data = _canonical(attestation)
        _write(self.evidence / "identity-attestation.json", attestation_data, 0o400)
        marker = self.marker()
        marker["trusted_inputs"]["allowed_signers"].update(
            {
                "observed_sha256": _digest(data),
                "protected_sha256": _digest(data),
                "size_bytes": len(data),
            }
        )
        marker["identity_verification"]["ssh_allowed_signers"].update(
            {"sha256": _digest(data), "size_bytes": len(data)}
        )
        for label in ("identity_transcript", "verify_transcript"):
            marker["identity_verification"][label].update(
                {"sha256": _digest(transcript_data), "size_bytes": len(transcript_data)}
            )
        marker["identity_verification"]["identity_attestation"].update(
            {"sha256": _digest(attestation_data), "size_bytes": len(attestation_data)}
        )
        marker["runner"]["environment_without_self_digest"][
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256"
        ] = _digest(data)
        self.environment[
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256"
        ] = _digest(data)
        self.seal_marker(marker)

    def run(
        self,
        *,
        checkpoint: str = "entry",
        environment: dict[str, str] | None = None,
        arguments: list[str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
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
            str(self.evidence / "python3"),
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

    runner_tool_source = _write(
        trust / "runner-tool-chmod", b"synthetic chmod", 0o500
    )
    runner_tool_manifest = _canonical(
        {
            "schema_version": 1,
            "tools": {
                "chmod": {
                    "path": str(runner_tool_source),
                    "sha256": _digest(runner_tool_source.read_bytes()),
                }
            },
        }
    )
    (evidence / "runner-bin" / "chmod").symlink_to(runner_tool_source)
    source_data = {
        "allowed_signers": b'release namespaces="git" ssh-ed25519 AAAATEST\n',
        "bash": b"synthetic relocatable bash",
        "bootstrap": b"synthetic protected bootstrap",
        "git": b"synthetic relocatable git",
        "identity_verifier": b"synthetic protected verifier",
        "manifest_helper": _manifest_helper().encode(),
        "receipt_validator": b"synthetic protected receipt validator",
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
    archives["python"] = _copy(sources["python"], evidence / "python3", 0o500)

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
    metadata = f"G\0{FINGERPRINT}\0\0release\0\n".encode()
    transcript = {
        "schema_version": 2,
        "archive_names": IDENTITY_NAMES,
        "candidate_commit_oid": identity["head_commit"],
        "environment": {"HOME": str(evidence), "PATH": os.defpath},
        "policy_overrides": ["-c", "gpg.format=ssh"],
        "policies": policies,
        "replay": {
            "candidate_root": "${CANDIDATE_ROOT}",
            "evidence_directory": "${EVIDENCE_DIRECTORY}",
            "environment": {"HOME": "${EVIDENCE_DIRECTORY}", "PATH": os.defpath},
            "policy_overrides": ["-c", "gpg.format=ssh"],
        },
        "tools": tools,
        "commands": {
            "show_signature_metadata": _command(["git", "show"], 0, metadata),
            "verify_commit": _command(["git", "verify-commit"], 0),
        },
        "tool_probes": {"ssh_keygen_usage": _command(["ssh-keygen", "-?"], 1)},
    }
    transcript_bytes = _canonical(transcript)
    _write(evidence / "identity-transcript.json", transcript_bytes, 0o400)
    evidence_records = {
        label: _artifact(
            transcript_bytes if label == "verify_transcript" else identity_data[label],
            name,
            0o500 if label in {"git", "ssh_keygen"} else 0o400,
        )
        for label, name in IDENTITY_NAMES.items()
    }
    attestation = {
        "schema_version": 2,
        "release_identity": identity,
        "release_identity_sha256": _digest(identity_bytes),
        "tools": tools,
        "policies": policies,
        "verification": {
            "status": "G",
            "signer_fingerprint": FINGERPRINT,
            "primary_key_fingerprint": "",
            "allowed_signers_principal": "release",
        },
        "evidence": evidence_records,
    }
    attestation_bytes = _canonical(attestation)
    _write(evidence / "identity-attestation.json", attestation_bytes, 0o400)

    trusted_records: dict[str, Any] = {}
    for label in sorted(TRUSTED_NAMES):
        source = sources[label]
        archive = archives[label]
        trusted_records[label] = {
            "archive_name": TRUSTED_NAMES[label],
            "archive_mode": f"{stat.S_IMODE(archive.stat().st_mode):04o}",
            "observed_sha256": _digest(source.read_bytes()),
            "protected_sha256": _digest(source.read_bytes()),
            "size_bytes": source.stat().st_size,
            "source_mode": f"{stat.S_IMODE(source.stat().st_mode):04o}",
            "source_path": str(source),
        }
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
    closed_environment = {
        "HOME": str(evidence / "home"),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.pathsep.join([str(evidence), str(evidence / "runner-bin")]),
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
        "schema_version": 1,
        "trust_boundary": {
            "bootstrap_authentication": "external prerequisite",
            "release_image_and_dynamic_loader": "external prerequisite",
            "same_uid_and_trusted_ancestor_owners": True,
        },
        "candidate_root": str(candidate),
        "candidate_identity": identity,
        "candidate_identity_sha256": _digest(identity_bytes),
        "trusted_inputs": trusted_records,
        "identity_verification": identity_records,
        "runner": {
            "argv": [str(archives["bash"]), str(runner), "--release"],
            "closed_path_resolution": {
                "bash": str(archives["bash"]),
                "git": str(archives["git"]),
                "python3": str(archives["python"]),
            },
            "environment_without_self_digest": closed_environment,
            "mode": f"{stat.S_IMODE(runner.stat().st_mode):04o}",
            "output": {
                "stderr_path": str(evidence / "runner-stderr.log"),
                "stdout_path": str(evidence / "runner-stdout.log"),
                "active_mode": "0600",
                "sealed_mode": "0400",
            },
            "path": str(runner),
            "tool_directory": str(evidence / "runner-bin"),
            "tools": {
                "chmod": {
                    "alias_name": "chmod",
                    "alias_path": str(evidence / "runner-bin" / "chmod"),
                    "sha256": _digest(runner_tool_source.read_bytes()),
                    "size_bytes": runner_tool_source.stat().st_size,
                    "source_mode": "0500",
                    "source_path": str(runner_tool_source),
                }
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
                "argv": [str(archives["python"]), "-I", "-S", "-c", "raise SystemExit(0)"],
                "exit_status": 0,
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
    release_fixture.prepare_sealed()
    environment = {**release_fixture.environment, "BASH_ENV": "/untrusted", "RUSTFLAGS": "poison"}
    result = release_fixture.run(checkpoint="sealed", environment=environment)
    assert result.returncode == 0, result.stderr


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
        (("runner", "size_bytes"), True),
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


def test_source_archive_drift_is_rejected(release_fixture: Fixture) -> None:
    source = release_fixture.trust / "source-git"
    _write(source, b"drift", 0o500)
    _assert_rejected(release_fixture.run())


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
    marker["runner"]["argv"][-1] = "--pr"
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
    attestation["verification"]["signer_fingerprint"] = OTHER_FINGERPRINT
    release_fixture.reseal_attestation(attestation)
    _assert_rejected(release_fixture.run())


def test_transcript_rejects_bool_as_command_status(release_fixture: Fixture) -> None:
    transcript = json.loads((release_fixture.evidence / "identity-transcript.json").read_bytes())
    transcript["commands"]["verify_commit"]["exit_status"] = True
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
        str(release_fixture.evidence / "python3"),
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
