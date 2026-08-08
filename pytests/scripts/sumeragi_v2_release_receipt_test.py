"""Contract tests for the aggregate Sumeragi v2 release receipt."""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import stat
import subprocess
import sys
from types import ModuleType

import pytest

from pytests.scripts.sumeragi_v2_release_receipt_components import (
    proof_ledger_checker_components,
    release_receipt_writer_components,
    terminal_output_path,
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
RELEASE_RECEIPT_TEST_COMPONENT_FILES = (
    "sumeragi_v2_release_receipt_bootstrap_archive_cases.py",
    "sumeragi_v2_release_receipt_terminal_publication_cases.py",
)


def _execute_test_component(filename: str) -> None:
    """Execute one reviewed case component in this canonical test namespace."""

    path = Path(__file__).with_name(filename)
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"release-receipt test component is unavailable: {path}")
    source = path.read_text(encoding="utf-8")
    exec(compile(source, str(path), "exec"), globals())


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
            "theorem": "sumeragi-v2-production-trace-extraction",
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
    for child in ("home", "tmp", "runner-bin"):
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
        "98f0a450fd0c25c890d77e3f5c0d13faca76ff3227797962c5dd33e5a29cd2f7"
    )
    synthetic_sources: dict[str, Path] = {}
    for label, data, mode in (
        ("python", b"#!/bin/sh\nexit 0\n", 0o500),
        ("bash", b"#!/bin/sh\nexit 0\n", 0o500),
        ("manifest_helper", b"# fixture manifest helper\n", 0o400),
        ("identity_verifier", b"# fixture identity verifier\n", 0o400),
        ("receipt_validator", b"# fixture receipt validator\n", 0o400),
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
        "chmod": b"#!/bin/sh\nexit 0\n",
        "cargo": (
            b"#!/bin/sh\n"
            b"test \"$#\" = 1 && test \"$1\" = --version || exit 91\n"
            b"printf '%s\\n' 'cargo 1.93.1 (083ac5135 2025-12-15)'\n"
        ),
        "rustc": (
            b"#!/bin/sh\n"
            b"test \"$#\" = 1 && test \"$1\" = -vV || exit 92\n"
            b"cat <<'RUSTC_VERSION'\n"
            + RUSTC_VERSION_OUTPUT
            + b"RUSTC_VERSION\n"
        ),
    }
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
        alias = evidence_dir / "runner-bin" / name
        alias.symlink_to(source.resolve())
        runner_tool_aliases[name] = alias
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
        "revocation": ("bootstrap-revocation", 0o400),
        "runner_tool_manifest": ("runner-tool-manifest.json", 0o400),
        "ssh_keygen": ("ssh-keygen", 0o500),
    }
    trusted_records: dict[str, object] = {}
    trusted_archives: dict[str, Path] = {}
    for label, source in trusted_sources.items():
        archive_name, archive_mode = trusted_names[label]
        archive = evidence_dir / archive_name
        archive.write_bytes(source.read_bytes())
        archive.chmod(archive_mode)
        trusted_archives[label] = archive
        trusted_records[label] = {
            "archive_name": archive_name,
            "archive_mode": f"{archive_mode:04o}",
            "observed_sha256": sha256(source),
            "protected_sha256": sha256(source),
            "size_bytes": source.stat().st_size,
            "source_mode": f"{source.stat().st_mode & 0o7777:04o}",
            "source_path": str(source.resolve()),
        }

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
        "schema_version": 2,
        "archive_names": identity_archive_names,
        "candidate_commit_oid": head,
        "environment": identity_environment,
        "policy_overrides": historical_config,
        "policies": bootstrap_policies,
        "replay": {
            "candidate_root": "${CANDIDATE_ROOT}",
            "evidence_directory": placeholder,
            "environment": {
                key: value.replace(str(evidence_dir), placeholder)
                for key, value in identity_environment.items()
            },
            "policy_overrides": replay_config,
        },
        "tools": bootstrap_tools,
        "commands": {
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
            ),
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
            ),
        },
        "tool_probes": {
            "ssh_keygen_usage": command_record(
                [str(stages["ssh_keygen"]), "-?"],
                [f"{placeholder}/identity-ssh-keygen", "-?"],
                1,
                b"",
                b"fixture ssh-keygen usage\n",
            )
        },
    }
    identity_paths["identity_transcript"].write_bytes(
        canonical_json(bootstrap_transcript_value)
    )
    identity_paths["identity_transcript"].chmod(0o400)
    bootstrap_attestation_value = {
        "schema_version": 2,
        "release_identity": identity,
        "release_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
        "tools": bootstrap_tools,
        "policies": bootstrap_policies,
        "verification": {
            "status": "G",
            "signer_fingerprint": signer_fingerprint,
            "primary_key_fingerprint": "",
            "allowed_signers_principal": signer_principal,
        },
        "evidence": {
            label: artifact_metadata(
                identity_paths["identity_transcript"]
                if label == "verify_transcript"
                else identity_paths[label],
                0o500 if label in {"git", "ssh_keygen"} else 0o400,
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
        "schema_version": 1,
        "trust_boundary": {
            "bootstrap_authentication": "external prerequisite",
            "release_image_and_dynamic_loader": "external prerequisite",
            "same_uid_and_trusted_ancestor_owners": True,
        },
        "candidate_root": str(candidate_root),
        "candidate_identity": identity,
        "candidate_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
        "trusted_inputs": trusted_records,
        "identity_verification": identity_verification,
        "runner": {
            "argv": [str(trusted_archives["bash"]), str(runner), "--release"],
            "closed_path_resolution": {
                "bash": str(trusted_archives["bash"]),
                "git": str(trusted_archives["git"]),
                "python3": str(trusted_archives["python"]),
            },
            "environment_without_self_digest": runner_environment,
            "mode": f"{runner.stat().st_mode & 0o7777:04o}",
            "output": {
                "stderr_path": str(evidence_dir / "runner-stderr.log"),
                "stdout_path": str(evidence_dir / "runner-stdout.log"),
                "active_mode": "0600",
                "sealed_mode": "0400",
            },
            "path": str(runner),
            "self_digest_environment_variables": [
                "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
            ],
            "sha256": sha256(runner),
            "size_bytes": runner.stat().st_size,
            "tool_directory": str(evidence_dir / "runner-bin"),
            "tools": {
                name: {
                    "alias_name": name,
                    "alias_path": str(runner_tool_aliases[name]),
                    "sha256": sha256(source),
                    "size_bytes": source.stat().st_size,
                    "source_mode": "0500",
                    "source_path": str(source.resolve()),
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
                    "raise SystemExit(0)",
                ],
                "exit_status": 0,
            },
        },
    }
    completion = evidence_dir / "BOOTSTRAP_COMPLETED.json"
    completion.write_bytes(canonical_json(marker_value))
    completion.chmod(0o400)
    return {
        "bootstrap_completion": completion,
        "bootstrap_evidence_dir": evidence_dir,
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
        "bootstrap_runner_cargo": runner_tool_sources["cargo"],
        "bootstrap_runner_rustc": runner_tool_sources["rustc"],
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
    release_root: Path,
    *,
    sealed_manifest: str,
    lock: str,
) -> dict[str, Path | str | list[Path]]:
    workspace_target = (release_root / "target").resolve(strict=True)
    bundle = (
        workspace_target
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
        ("irohad", "release/irohad"),
        ("irohad_message_control", "message-control/release/irohad"),
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


def make_evidence(tmp_path: Path) -> dict[str, Path | str | list[Path]]:
    candidate_manifest = "a" * 64
    sealed_manifest = "b" * 64
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
        "schema_version": 2,
        "archive_names": archive_names,
        "candidate_commit_oid": head,
        "environment": closed_environment,
        "policy_overrides": historical_config,
        "policies": policies,
        "replay": {
            "candidate_root": "${CANDIDATE_ROOT}",
            "evidence_directory": evidence_placeholder,
            "environment": {
                key: value.replace(str(signature_dir), evidence_placeholder)
                for key, value in closed_environment.items()
            },
            "policy_overrides": replay_config,
        },
        "tools": tools,
        "commands": {
            "show_signature_metadata": command_record(
                [
                    str(stage_paths["git"]),
                    *historical_config,
                    "show",
                    "--no-patch",
                    "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
                    head,
                ],
                [
                    f"{evidence_placeholder}/git",
                    *replay_config,
                    "show",
                    "--no-patch",
                    "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
                    head,
                ],
                0,
                show_output,
                b"",
            ),
            "verify_commit": command_record(
                [
                    str(stage_paths["git"]),
                    *historical_config,
                    "verify-commit",
                    "--raw",
                    head,
                ],
                [
                    f"{evidence_placeholder}/git",
                    *replay_config,
                    "verify-commit",
                    "--raw",
                    head,
                ],
                0,
                b"",
                b"Good fixture SSH signature\n",
            ),
        },
        "tool_probes": {
            "ssh_keygen_usage": command_record(
                [str(stage_paths["ssh_keygen"]), "-?"],
                [f"{evidence_placeholder}/ssh-keygen", "-?"],
                1,
                b"",
                b"fixture ssh-keygen usage\n",
            )
        },
    }
    signature_transcript.write_bytes(canonical_json(transcript))
    signature_transcript.chmod(0o400)
    attestation_evidence = {
        "cargo_lock": artifact_metadata(signature_cargo_lock, 0o400),
        "git": artifact_metadata(signature_git, 0o500),
        "raw_commit": artifact_metadata(signature_raw_commit, 0o400),
        "ssh_allowed_signers": artifact_metadata(signature_allowed_signers, 0o400),
        "ssh_keygen": artifact_metadata(signature_ssh_keygen, 0o500),
        "ssh_revocation": artifact_metadata(signature_revocation, 0o400),
        "verify_transcript": artifact_metadata(signature_transcript, 0o400),
    }
    candidate_identity = json.loads(candidate.read_text(encoding="utf-8"))
    attestation = {
        "schema_version": 2,
        "release_identity": candidate_identity,
        "release_identity_sha256": sha256(candidate),
        "tools": tools,
        "policies": policies,
        "verification": {
            "status": "G",
            "signer_fingerprint": signer_fingerprint,
            "primary_key_fingerprint": "",
            "allowed_signers_principal": signer_principal,
        },
        "evidence": attestation_evidence,
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
    )
    bootstrap_evidence_dir = bootstrap["bootstrap_evidence_dir"]
    assert isinstance(bootstrap_evidence_dir, Path)
    release_invocation_root = bootstrap_evidence_dir / "release-runner"
    release_invocation_root.mkdir(mode=0o700)
    release_invocation_root.chmod(0o700)
    release_root = release_invocation_root / "source"
    release_root.mkdir()
    (release_root / "Cargo.lock").write_bytes(lock_bytes)
    workspace_target = release_invocation_root / "workspace-target"
    workspace_target.mkdir(mode=0o700)
    workspace_target.chmod(0o700)
    (release_root / "target").symlink_to(
        workspace_target,
        target_is_directory=True,
    )
    release_output = release_invocation_root / "output"
    release_output.mkdir(mode=0o700)
    release_output.chmod(0o700)
    release_output_directory = release_output / "release"
    release_output_directory.mkdir(mode=0o700)
    release_output_directory.chmod(0o700)
    terminal_output = release_output_directory / "RELEASE_COMPLETED.json"

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
    corridor_legs = writer_symbols["_corridor_legs"]()
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
    sdk_diagnostics_suite_source_paths = writer_symbols[
        "_SUMERAGI_SDK_DIAGNOSTICS_SUITE_SOURCE_PATHS"
    ]
    sdk_diagnostics_suite_source_manifest = writer_symbols[
        "_sumeragi_sdk_diagnostics_suite_source_manifest"
    ](ROOT_DIR)
    native_amx_grouped_fixture = writer_symbols["_NATIVE_AMX_GROUPED_FIXTURE"]
    native_amx_grouped_negative_control_count = writer_symbols[
        "_NATIVE_AMX_GROUPED_NEGATIVE_CONTROL_COUNT"
    ]
    native_amx_grouped_suite_source_paths = writer_symbols[
        "_NATIVE_AMX_GROUPED_SUITE_SOURCE_PATHS"
    ]
    expected_direct_source_groups = (
        (
            "javascript/iroha_js/src/toriiClient.js",
            "javascript/iroha_js/src/norito.js",
            "javascript/iroha_js/src/native.js",
            "javascript/iroha_js/scripts/build-dist.mjs",
            "javascript/iroha_js/scripts/native-build-provenance.mjs",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/"
            "SumeragiDiagnosticsModels.kt",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/util/"
            "HashLiteral.kt",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/crypto/"
            "IrohaHash.kt",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/"
            "SumeragiDiagnosticsModels.java",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/crypto/"
            "IrohaHash.java",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/util/"
            "HashLiteral.java",
        ),
    )
    for expected_group in expected_direct_source_groups:
        start = native_amx_grouped_suite_source_paths.index(expected_group[0])
        assert (
            tuple(
                native_amx_grouped_suite_source_paths[
                    start : start + len(expected_group)
                ]
            )
            == expected_group
        )
    assert len(native_amx_grouped_suite_source_paths) == 50
    harness_text = (
        ROOT_DIR / writer_symbols["_NATIVE_AMX_GROUPED_PARITY_HARNESS"]
    ).read_text(encoding="utf-8")
    assert (
        'readonly javascript_staged_scripts_root="${javascript_package_root}/scripts"'
        in harness_text
    )
    assert re.search(
        r'cp "\$\{javascript_sdk_root\}/scripts/native-build-provenance\.mjs"'
        r'(?:\s*\\)?\s+'
        r'"\$\{javascript_staged_scripts_root\}/native-build-provenance\.mjs"',
        harness_text,
    )
    native_amx_grouped_suite_source_manifest = writer_symbols[
        "_native_amx_grouped_suite_source_manifest"
    ](ROOT_DIR)
    native_amx_grouped_fixture_sha256 = sha256(
        ROOT_DIR / native_amx_grouped_fixture
    )
    for relative_path in (
        native_amx_grouped_fixture,
        *native_amx_grouped_suite_source_paths,
        *sdk_diagnostics_suite_source_paths,
    ):
        retained_source = release_root / relative_path
        retained_source.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative_path, retained_source)
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
            log_lines = [
                "native-amx-v2-grouped-parity "
                f"surface={surface} tests={required_count} "
                f"fixture_sha256={native_amx_grouped_fixture_sha256} "
                "suite_source_manifest_sha256="
                f"{native_amx_grouped_suite_source_manifest}"
            ]
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
        release_root,
        sealed_manifest=sealed_manifest,
        lock=lock,
    )
    prebuilt_manifest = prebuilt["prebuilt_manifest"]
    prebuilt_manifest_sha256 = prebuilt["prebuilt_manifest_sha256"]
    assert isinstance(prebuilt_manifest, Path)
    assert isinstance(prebuilt_manifest_sha256, str)
    corridor_completion = corridor_dir / "COMPLETED.tsv"
    isolated_cargo_home = tool_dir / "cargo-home"
    isolated_cargo_home.mkdir()
    write_tsv(
        corridor_completion,
        {
            "schema_version": "1",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
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
                "schema_version\t1",
                "backend\tapalache",
                "version\t0.52.2",
                "launcher_sha256\t"
                "bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7",
                "jar_sha256\t"
                "1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a",
                f"source_manifest_sha256\t{sealed_manifest}",
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
    formal_production_trace_extraction_evidence.write_text(
        '{"backend_verification":true,"canonical":true,'
        '"theorem":"sumeragi-v2-production-trace-extraction"}\n',
        encoding="utf-8",
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
    seed_source_bound_root = (
        release_root / "target" / "sumeragi-v2-release" / sealed_manifest
    )
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
                    f"CARGO_TARGET_DIR={seed_source_bound_root / 'test-suite'} "
                    f"IROHA_TEST_TARGET_DIR={seed_program_target} "
                    f"IROHA_RELEASE_SOURCE_MANIFEST_SHA256={sealed_manifest} "
                    "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256="
                    f"{prebuilt_manifest_sha256} "
                    "TEST_NETWORK_BIN_IROHAD="
                    f"{seed_program_target / 'release' / 'irohad'} "
                    "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="
                    f"{seed_program_target / 'message-control' / 'release' / 'irohad'} "
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
        "candidate": candidate,
        "sealed": sealed,
        "release_root": release_root,
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
        "head": head,
        "tree": tree,
        "lock": lock,
    }


def run_writer(
    evidence: dict[str, Path | str | list[Path]],
    output: Path,
    writer: Path,
    *,
    use_supplied_output: bool = False,
    verify_existing: bool = False,
) -> subprocess.CompletedProcess[str]:
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
        if not verify_existing or not destination.exists():
            shutil.copy2(source_root / relative, destination)
    arguments = [
            sys.executable,
            str(writer),
            "--candidate-identity",
            str(evidence["candidate"]),
            "--sealed-identity",
            str(evidence["sealed"]),
            "--release-root",
            str(evidence["release_root"]),
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
        arguments.append("--verify-existing")
    return subprocess.run(
        arguments,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
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
    manifest_source = tool.parent / "runner-tool-manifest.json"
    manifest_archive = evidence_dir / "runner-tool-manifest.json"
    manifest_value = json.loads(manifest_source.read_text(encoding="utf-8"))
    manifest_value["tools"][name]["sha256"] = sha256(tool)
    manifest_source.chmod(0o600)
    manifest_source.write_bytes(canonical_json(manifest_value))
    manifest_source.chmod(0o400)
    manifest_archive.chmod(0o600)
    manifest_archive.write_bytes(manifest_source.read_bytes())
    manifest_archive.chmod(0o400)

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
        manifest_record["observed_sha256"] = sha256(manifest_source)
        manifest_record["protected_sha256"] = sha256(manifest_source)
        manifest_record["size_bytes"] = manifest_source.stat().st_size

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
    attestation["evidence"]["verify_transcript"] = artifact_metadata(path, 0o400)
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
    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
    for value in (attestation, transcript):
        value["tools"]["git"]["observed_sha256"] = digest
        value["tools"]["git"]["protected_sha256"] = digest
        value["tools"]["git"]["size_bytes"] = git_path.stat().st_size
    attestation["evidence"]["git"] = artifact_metadata(git_path, 0o500)
    rewrite_json(transcript_path, transcript)
    attestation["evidence"]["verify_transcript"] = artifact_metadata(
        transcript_path, 0o400
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


def test_receipt_hashes_every_formal_matrix_chaos_and_soak_artifact(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    output = terminal_output_path(evidence)
    result = run_writer(evidence, output, writer)

    assert result.returncode == 0, result.stderr
    receipt = json.loads(output.read_text(encoding="utf-8"))
    assert receipt["result"] == "release-complete"
    assert receipt["identity"] == {
        "head_commit": evidence["head"],
        "head_tree": evidence["tree"],
        "index_tree": evidence["tree"],
        "cargo_lock_sha256": evidence["lock"],
        "candidate_source_manifest_sha256": evidence["candidate_manifest"],
        "sealed_source_manifest_sha256": evidence["sealed_manifest"],
    }
    assert receipt["authentication"]["schema_version"] == 2
    release_authentication = receipt["authentication"]["release_identity"]
    bootstrap_authentication = receipt["authentication"]["bootstrap"]
    assert release_authentication["signature_format"] == "ssh"
    assert release_authentication["verification_status"] == "G"
    assert release_authentication["candidate_commit_oid"] == evidence["head"]
    assert release_authentication["signer_fingerprint"] == evidence[
        "expected_signer_fingerprint"
    ]
    assert release_authentication["allowed_signers_principal"] == evidence[
        "signer_principal"
    ]
    assert release_authentication["replay"]["performed"] is True
    assert release_authentication["trust_policy"] == {
        "git_sha256": evidence["expected_git_sha256"],
        "ssh_keygen_sha256": evidence["expected_ssh_keygen_sha256"],
        "allowed_signers_sha256": evidence["expected_allowed_signers_sha256"],
        "revocation_sha256": evidence["expected_revocation_sha256"],
        "signer_fingerprint": evidence["expected_signer_fingerprint"],
    }
    assert bootstrap_authentication["completion_sha256"] == evidence[
        "expected_bootstrap_completion_sha256"
    ]
    assert bootstrap_authentication["frozen_bootstrap_sha256"] == (
        "98f0a450fd0c25c890d77e3f5c0d13faca76ff3227797962c5dd33e5a29cd2f7"
    )
    assert bootstrap_authentication["candidate_commit_oid"] == evidence["head"]
    assert receipt["evidence"]["bootstrap"]["completion"]["path"] == str(
        evidence["bootstrap_completion"]
    )
    signature_artifacts = {
        "release_signature_attestation": "signature_attestation",
        "release_signature_transcript": "signature_transcript",
        "release_signature_raw_commit": "signature_raw_commit",
        "release_signature_cargo_lock": "signature_cargo_lock",
        "release_signature_allowed_signers": "signature_allowed_signers",
        "release_signature_revocation": "signature_revocation",
        "release_signature_git": "signature_git",
        "release_signature_ssh_keygen": "signature_ssh_keygen",
    }
    for receipt_name, fixture_name in signature_artifacts.items():
        fixture_path = evidence[fixture_name]
        assert isinstance(fixture_path, Path)
        expected_mode = "0500" if fixture_name in {
            "signature_git",
            "signature_ssh_keygen",
        } else "0400"
        assert receipt["evidence"][receipt_name] == {
            "path": str(fixture_path.resolve()),
            "sha256": sha256(fixture_path),
            "size_bytes": fixture_path.stat().st_size,
            "mode": expected_mode,
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    expected_artifacts = {
        "corridor_completion": "corridor_completion",
        "corridor_summary": "corridor_summary",
        "corridor_production_inventory": "corridor_required",
        "g_unit_focused_test_inventory": "corridor_g_unit",
        "formal_completion": "formal_completion",
        "formal_gate_log": "formal_log",
        "formal_proof_coverage": "formal_ledger",
        "formal_proof_evidence": "formal_evidence",
        "formal_verus_evidence": "formal_verus_evidence",
        "formal_verus_log": "formal_verus_log",
        "formal_multilane_apalache_evidence": (
            "formal_multilane_apalache_evidence"
        ),
        "formal_cross_tool_evidence": "formal_cross_tool_evidence",
        "formal_production_trace_extraction_evidence": (
            "formal_production_trace_extraction_evidence"
        ),
        "formal_harness_lock": "formal_harness_lock",
        "formal_toolchain": "formal_toolchain",
        "formal_tlaps_resource_jsonl": "formal_tlaps_resource_jsonl",
        "formal_tlaps_resource_summary": "formal_tlaps_resource_summary",
        "seed_matrix_completion": "seed_completion",
        "seed_matrix_summary": "seed_summary",
        "seed_matrix_localnet_manifest_index": "seed_localnet_manifest_index",
        "chaos_completion": "chaos_completion",
        "chaos_log": "chaos_log",
        "taira_completion": "taira_completion",
        "taira_evidence": "taira_evidence",
        "taira_run_log": "taira_log",
    }
    for receipt_name, fixture_name in expected_artifacts.items():
        fixture_path = evidence[fixture_name]
        assert isinstance(fixture_path, Path)
        assert receipt["evidence"][receipt_name] == {
            "path": str(fixture_path.resolve()),
            "sha256": sha256(fixture_path),
        }
    seed_logs = evidence["seed_logs"]
    assert isinstance(seed_logs, list)
    assert receipt["evidence"]["seed_matrix_run_logs"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)} for path in seed_logs
    ]
    seed_localnet_manifests = evidence["seed_localnet_manifests"]
    assert isinstance(seed_localnet_manifests, list)
    assert receipt["evidence"]["seed_matrix_localnet_manifests"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)}
        for path in seed_localnet_manifests
    ]
    corridor_logs = evidence["corridor_logs"]
    assert isinstance(corridor_logs, list)
    assert receipt["evidence"]["corridor_logs"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)}
        for path in corridor_logs
    ]
    diagnostics_legs = {
        Path(artifact["path"]).stem.split("-", 1)[1]
        for artifact in receipt["evidence"]["corridor_logs"]
        if "-sumeragi-diagnostics-" in Path(artifact["path"]).name
    }
    assert diagnostics_legs == {
        "sumeragi-diagnostics-rust",
        "sumeragi-diagnostics-python",
        "sumeragi-diagnostics-javascript",
        "sumeragi-diagnostics-swift",
        "sumeragi-diagnostics-kotlin",
        "sumeragi-diagnostics-java",
    }
    proof_fidelity_logs = [
        artifact
        for artifact in receipt["evidence"]["corridor_logs"]
        if artifact["path"].endswith("-preflight-proof-fidelity.log")
    ]
    assert len(proof_fidelity_logs) == 1
    scaling_preflight_logs = [
        artifact
        for artifact in receipt["evidence"]["corridor_logs"]
        if artifact["path"].endswith("-preflight-multilane-scaling.log")
    ]
    assert len(scaling_preflight_logs) == 1

    prebuilt = receipt["evidence"]["prebuilt_binary_bundle"]
    prebuilt_manifest = evidence["prebuilt_manifest"]
    prebuilt_binaries = evidence["prebuilt_binaries"]
    prebuilt_bundle = evidence["prebuilt_bundle"]
    assert isinstance(prebuilt_manifest, Path)
    assert isinstance(prebuilt_binaries, list)
    assert isinstance(prebuilt_bundle, Path)
    assert prebuilt["schema_version"] == 2
    assert prebuilt["manifest"] == {
        "path": str(prebuilt_manifest),
        "sha256": sha256(prebuilt_manifest),
        "size_bytes": prebuilt_manifest.stat().st_size,
        "mode": "0400",
        "owner_uid": os.geteuid(),
        "nlink": 1,
    }
    assert prebuilt["source_manifest_sha256"] == evidence["sealed_manifest"]
    assert prebuilt["cargo_lock_sha256"] == evidence["lock"]
    assert prebuilt["bundle_dir"] == str(prebuilt_bundle)
    assert prebuilt["host_triple"] == PREBUILT_HOST_TRIPLE
    assert prebuilt["target_triple"] == PREBUILT_HOST_TRIPLE
    assert prebuilt["profile"] == "release"
    assert prebuilt["version_transcripts"] == {
        "cargo": {
            "argv": [str(evidence["corridor_cargo_tool"]), "--version"],
            "sha256": hashlib.sha256(CARGO_VERSION_OUTPUT).hexdigest(),
            "size_bytes": len(CARGO_VERSION_OUTPUT),
        },
        "rustc": {
            "argv": [str(evidence["corridor_rustc_tool"]), "-vV"],
            "sha256": hashlib.sha256(RUSTC_VERSION_OUTPUT).hexdigest(),
            "size_bytes": len(RUSTC_VERSION_OUTPUT),
        },
    }
    assert prebuilt["binaries"] == [
        {
            "role": role,
            "relative_path": relative,
            "path": str(path),
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": "0500",
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
        for (role, relative), path in zip(
            (
                ("irohad", "release/irohad"),
                (
                    "irohad_message_control",
                    "message-control/release/irohad",
                ),
                ("iroha", "release/iroha"),
                ("kagami", "release/kagami"),
            ),
            prebuilt_binaries,
        )
    ]

    scaling_root = evidence["scaling_root"]
    assert isinstance(scaling_root, Path)
    scaling_bundle = receipt["evidence"]["multilane_scaling_bundle"]
    scaling_paths = sorted(
        path for path in scaling_root.rglob("*") if path.is_file()
    )
    assert scaling_bundle["root"] == str(scaling_root.resolve())
    assert scaling_bundle["file_count"] == len(scaling_paths)
    assert scaling_bundle["total_size_bytes"] == sum(
        path.stat().st_size for path in scaling_paths
    )
    assert [record["relative_path"] for record in scaling_bundle["files"]] == [
        path.relative_to(scaling_root).as_posix() for path in scaling_paths
    ]
    for record, path in zip(scaling_bundle["files"], scaling_paths):
        assert record == {
            "relative_path": path.relative_to(scaling_root).as_posix(),
            "path": str(path.resolve()),
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": f"{path.stat().st_mode & 0o7777:04o}",
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    release_root = evidence["release_root"]
    assert isinstance(release_root, Path)
    expected_retained_tooling = []
    for role, source_path in (
        ("localnet", "scripts/deploy_localnet.sh"),
        ("load_generator", "scripts/tx_load.py"),
        ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
    ):
        retained_path = release_root / source_path
        expected_retained_tooling.append(
            {
                "role": role,
                "source_path": source_path,
                "path": str(retained_path),
                "sha256": sha256(retained_path),
                "size_bytes": retained_path.stat().st_size,
                "mode": f"{retained_path.stat().st_mode & 0o7777:04o}",
                "owner_uid": os.geteuid(),
                "nlink": 1,
            }
        )
    assert receipt["evidence"]["multilane_scaling_trust_anchors"] == {
        "trial_harness_sha256": evidence[
            "expected_scaling_trial_harness_sha256"
        ],
        "configuration_sha256": evidence[
            "expected_scaling_configuration_sha256"
        ],
        "irohad_sha256": evidence["expected_scaling_irohad_sha256"],
        "iroha_cli_sha256": evidence["expected_scaling_iroha_cli_sha256"],
        "repository_root": str(evidence["release_root"]),
        "retained_tooling": expected_retained_tooling,
    }

    g4p = receipt["evidence"]["g4p_multilane"]
    assert g4p["schema_version"] == 1
    g4p_logs = evidence["g4p_logs"]
    assert isinstance(g4p_logs, list)
    g4p_expected = {
        "completion": evidence["g4p_completion"],
        "run_summary": evidence["g4p_summary"],
    }
    for receipt_name, path in g4p_expected.items():
        assert isinstance(path, Path)
        assert g4p[receipt_name] == {
            "path": str(path.resolve()),
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": f"{path.stat().st_mode & 0o7777:04o}",
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    assert [record["path"] for record in g4p["run_logs"]] == [
        str(path.resolve()) for path in g4p_logs
    ]

    g12 = receipt["evidence"]["g12_cross_dataspace"]
    g12_seed_logs = evidence["g12_seed_logs"]
    assert isinstance(g12_seed_logs, list)
    g12_expected = {
        "seed_completion": evidence["g12_seed_completion"],
        "seed_summary": evidence["g12_seed_summary"],
        "fault_soak_completion": evidence["g12_soak_completion"],
        "fault_soak_log": evidence["g12_soak_log"],
    }
    for receipt_name, path in g12_expected.items():
        assert isinstance(path, Path)
        assert g12[receipt_name] == {
            "path": str(path.resolve()),
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": f"{path.stat().st_mode & 0o7777:04o}",
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    assert [record["path"] for record in g12["seed_run_logs"]] == [
        str(path.resolve()) for path in g12_seed_logs
    ]

    module = load_writer_module()
    candidate = evidence["candidate"]
    sealed = evidence["sealed"]
    assert isinstance(candidate, Path)
    assert isinstance(sealed, Path)
    candidate_contract = module._capture_path_contract(
        candidate,
        "fixture candidate identity",
        expected_sha256=sha256(candidate),
    )
    sealed_contract = module._capture_path_contract(
        sealed,
        "fixture sealed identity",
        expected_sha256=sha256(sealed),
    )
    contracts = module._snapshot_receipt_inputs(
        receipt,
        candidate_identity=candidate_contract,
        sealed_identity=sealed_contract,
    )
    directory_paths = {
        contract.path
        for contract in contracts
        if isinstance(contract, module.DirectoryContract)
    }

    family_specs = (
        (
            "corridor_completion",
            (
                "corridor_completion",
                "corridor_summary",
                "corridor_production_inventory",
                "g_unit_focused_test_inventory",
                "corridor_logs",
            ),
        ),
        (
            "formal_completion",
            (
                "formal_completion",
                "formal_gate_log",
                "formal_proof_coverage",
                "formal_proof_evidence",
                "formal_verus_evidence",
                "formal_verus_log",
                "formal_multilane_apalache_evidence",
                "formal_cross_tool_evidence",
                "formal_production_trace_extraction_evidence",
                "formal_harness_lock",
                "formal_toolchain",
                "formal_tlaps_resource_jsonl",
                "formal_tlaps_resource_summary",
            ),
        ),
        (
            "seed_matrix_completion",
            (
                "seed_matrix_completion",
                "seed_matrix_summary",
                "seed_matrix_run_logs",
                "seed_matrix_localnet_manifest_index",
                "seed_matrix_localnet_manifests",
            ),
        ),
        ("chaos_completion", ("chaos_completion", "chaos_log")),
        (
            "taira_completion",
            ("taira_completion", "taira_evidence", "taira_run_log"),
        ),
    )

    def artifact_paths(value: object) -> list[Path]:
        paths: list[Path] = []
        if isinstance(value, dict):
            if isinstance(value.get("path"), str) and isinstance(
                value.get("sha256"), str
            ):
                paths.append(Path(value["path"]))
            for child in value.values():
                paths.extend(artifact_paths(child))
        elif isinstance(value, list):
            for child in value:
                paths.extend(artifact_paths(child))
        return paths

    family_roots: set[Path] = set()
    expected_family_directories: set[Path] = set()
    for completion_key, member_keys in family_specs:
        root = Path(receipt["evidence"][completion_key]["path"]).parent
        family_roots.add(root)
        expected_family_directories.add(root)
        for member_key in member_keys:
            for path in artifact_paths(receipt["evidence"][member_key]):
                parent = path.parent
                while True:
                    expected_family_directories.add(parent)
                    if parent == root:
                        break
                    parent = parent.parent
    actual_family_directories = {
        path
        for path in directory_paths
        if any(path == root or root in path.parents for root in family_roots)
    }
    assert actual_family_directories == expected_family_directories

    corridor_root = Path(
        receipt["evidence"]["corridor_completion"]["path"]
    ).parent
    corridor_metadata = corridor_root.stat()
    real_fsync = module.os.fsync

    def fail_corridor_root_fsync(descriptor: int) -> None:
        metadata = os.fstat(descriptor)
        if (metadata.st_dev, metadata.st_ino) == (
            corridor_metadata.st_dev,
            corridor_metadata.st_ino,
        ):
            raise OSError("fixture corridor root fsync failure")
        real_fsync(descriptor)

    unpublished = output.with_name("UNPUBLISHED.json")
    monkeypatch.setattr(module.os, "fsync", fail_corridor_root_fsync)
    with pytest.raises(module.ReceiptError, match="fsync failed"):
        module._fsync_receipt_inputs(contracts)
    assert not unpublished.exists()

    replacement_identity = candidate.with_name("candidate-replacement.json")
    replacement_identity.write_bytes(candidate.read_bytes())
    os.replace(replacement_identity, candidate)
    with pytest.raises(
        module.ReceiptError, match="changed after semantic validation"
    ):
        module._snapshot_receipt_inputs(
            receipt,
            candidate_identity=candidate_contract,
            sealed_identity=sealed_contract,
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
            "Cargo version digest does not match the authenticated tool",
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
    completion = evidence["formal_completion"]
    assert isinstance(apalache, Path)
    assert isinstance(completion, Path)
    canonical = apalache.read_text(encoding="utf-8")
    apalache.write_text(
        canonical.replace("result_count\t6", "result_count\t5", 1),
        encoding="utf-8",
    )
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["multilane_apalache_evidence_sha256"] = sha256(apalache)
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "Apalache evidence header is not the exact pinned profile" in result.stderr

    apalache.write_text(
        canonical.replace(
            "result\tinflight-first-release-layout\t",
            "result\tinflight-first-release-refinement\t",
            1,
        ),
        encoding="utf-8",
    )
    fields["multilane_apalache_evidence_sha256"] = sha256(apalache)
    write_tsv(completion, fields)
    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "is not exact source-bound NoError evidence" in result.stderr


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
    completion = evidence["formal_completion"]
    assert isinstance(trace_evidence, Path)
    assert isinstance(completion, Path)
    trace_evidence.write_text(
        '{"backend_verification":false,"canonical":true,'
        '"theorem":"sumeragi-v2-production-trace-extraction"}\n',
        encoding="utf-8",
    )
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["production_trace_extraction_evidence_sha256"] = sha256(
        trace_evidence
    )
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert (
        "archived formal evidence does not authenticate production "
        "trace extraction"
    ) in result.stderr


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


@pytest.mark.parametrize(
    ("path", "replacement"),
    [
        (("schema_version",), 1),
        (("release_identity", "head_commit"), "9" * 40),
        (("release_identity_sha256",), "0" * 64),
        (("tools", "git", "archive_name"), "other-git"),
        (("tools", "git", "mode"), "0501"),
        (("tools", "git", "observed_sha256"), "1" * 64),
        (("tools", "git", "protected_sha256"), "2" * 64),
        (("policies", "signature_format"), "openpgp"),
        (("policies", "expected_signer_fingerprint"), "SHA256:" + "B" * 43),
        (("policies", "ssh_allowed_signers", "protected_sha256"), "3" * 64),
        (("policies", "ssh_allowed_signers", "size_bytes"), False),
        (("policies", "ssh_revocation", "size_bytes"), False),
        (("verification", "status"), "U"),
        (("verification", "signer_fingerprint"), "SHA256:" + "B" * 43),
        (("verification", "primary_key_fingerprint"), "SHA256:" + "C" * 43),
        (("verification", "allowed_signers_principal"), ""),
        (("evidence", "raw_commit", "archive_name"), "commit"),
        (("evidence", "raw_commit", "size_bytes"), 0),
        (("evidence", "ssh_revocation", "size_bytes"), False),
    ],
)
def test_receipt_rejects_tampered_signature_attestation_fields(
    tmp_path: Path, path: tuple[str, ...], replacement: object
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    mutate_attestation(
        evidence, lambda value: set_nested(value, path, replacement)
    )

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "release" in result.stderr.lower()
    assert not (tmp_path / "receipt.json").exists()


@pytest.mark.parametrize("artifact_name", ["signature_attestation", "signature_transcript"])
def test_receipt_rejects_noncanonical_signature_json(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    value = json.loads(artifact.read_text(encoding="utf-8"))
    artifact.chmod(0o600)
    artifact.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    artifact.chmod(0o400)
    if artifact_name == "signature_transcript":
        attestation = evidence["signature_attestation"]
        assert isinstance(attestation, Path)
        attestation_value = json.loads(attestation.read_text(encoding="utf-8"))
        attestation_value["evidence"]["verify_transcript"] = artifact_metadata(
            artifact, 0o400
        )
        rewrite_json(attestation, attestation_value)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "canonical UTF-8 JSON" in result.stderr


def test_receipt_rejects_noncanonical_candidate_identity_bytes(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    candidate = evidence["candidate"]
    assert isinstance(candidate, Path)
    value = json.loads(candidate.read_text(encoding="utf-8"))
    candidate.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "candidate identity is not canonical UTF-8 JSON" in result.stderr


@pytest.mark.parametrize(
    ("mutation", "error_fragment"),
    [
        ("non-ssh", "non-SSH signature format"),
        ("malformed-armor", "malformed SSH signature armor"),
        ("wrong-tree", "tree does not match"),
        ("duplicate-trailer", "exact terminal Sumeragi v2 release trailer block"),
    ],
)
def test_raw_commit_validator_rejects_adversarial_signed_object_shapes(
    tmp_path: Path, mutation: str, error_fragment: str
) -> None:
    evidence = make_evidence(tmp_path)
    raw_path = evidence["signature_raw_commit"]
    candidate_path = evidence["candidate"]
    assert isinstance(raw_path, Path)
    assert isinstance(candidate_path, Path)
    raw = raw_path.read_bytes()
    identity = json.loads(candidate_path.read_text(encoding="utf-8"))
    if mutation == "non-ssh":
        raw = raw.replace(b"SSH SIGNATURE", b"PGP SIGNATURE")
    elif mutation == "malformed-armor":
        raw = re.sub(rb"(?m)^ [A-Za-z0-9+/=]+$", b" ***", raw, count=1)
    elif mutation == "wrong-tree":
        raw = raw.replace(b"tree " + b"2" * 40, b"tree " + b"3" * 40)
    elif mutation == "duplicate-trailer":
        raw = raw.replace(
            b"Sumeragi v2 release fixture\n\n",
            b"Sumeragi v2 release fixture\n"
            b"Sumeragi-V2-Release-Identity-Version: 1\n\n",
        )
    else:
        raise AssertionError(mutation)
    framed = b"commit " + str(len(raw)).encode("ascii") + b"\0" + raw
    identity["head_commit"] = hashlib.sha1(
        framed, usedforsecurity=False
    ).hexdigest()
    symbols = runpy.run_path(str(SCRIPT))

    with pytest.raises(symbols["ReceiptError"], match=error_fragment):
        symbols["_validate_raw_commit"](raw, identity)


def test_allowed_signers_policy_accepts_one_unbounded_active_line() -> None:
    symbols = runpy.run_path(str(SCRIPT))

    symbols["_validate_allowed_signers_policy"](
        b"# release trust root\n\n"
        b"release@example.test ssh-ed25519 AAAAC3NzaFixtureKey\n"
    )


@pytest.mark.parametrize(
    ("policy", "error_fragment"),
    [
        (
            b"first@example.test ssh-ed25519 AAAAC3NzaFirst\n"
            b"second@example.test ssh-ed25519 AAAAC3NzaSecond\n",
            "exactly one active line",
        ),
        (
            b'release@example.test valid-after="20260101Z" '
            b"ssh-ed25519 AAAAC3NzaFixtureKey\n",
            "time-bounded",
        ),
        (
            b'release@example.test valid-before="20270101Z" '
            b"ssh-ed25519 AAAAC3NzaFixtureKey\n",
            "time-bounded",
        ),
    ],
)
def test_allowed_signers_policy_rejects_multiple_or_time_bounded_lines(
    policy: bytes, error_fragment: str
) -> None:
    symbols = runpy.run_path(str(SCRIPT))

    with pytest.raises(symbols["ReceiptError"], match=error_fragment):
        symbols["_validate_allowed_signers_policy"](policy)


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("expected_git_sha256", "0" * 64),
        ("expected_ssh_keygen_sha256", "1" * 64),
        ("expected_allowed_signers_sha256", "2" * 64),
        ("expected_revocation_sha256", "3" * 64),
        ("expected_signer_fingerprint", "SHA256:" + "B" * 43),
    ],
)
def test_receipt_rejects_wrong_out_of_band_signature_policy(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    evidence[field] = replacement

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert not (tmp_path / "receipt.json").exists()


@pytest.mark.parametrize(
    "artifact_name",
    [
        "signature_attestation",
        "signature_transcript",
        "signature_raw_commit",
        "signature_cargo_lock",
        "signature_allowed_signers",
        "signature_revocation",
        "signature_git",
        "signature_ssh_keygen",
    ],
)
def test_receipt_rejects_signature_archive_mode_drift(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    artifact.chmod(0o600)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact mode" in result.stderr


@pytest.mark.parametrize(
    "artifact_name",
    [
        "signature_raw_commit",
        "signature_cargo_lock",
        "signature_allowed_signers",
        "signature_revocation",
        "signature_git",
        "signature_ssh_keygen",
    ],
)
def test_receipt_rejects_signature_archive_content_drift(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    tool = artifact_name in {"signature_git", "signature_ssh_keygen"}
    artifact.chmod(0o700 if tool else 0o600)
    artifact.write_bytes(artifact.read_bytes() + b"tamper\n")
    artifact.chmod(0o500 if tool else 0o400)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert not (tmp_path / "receipt.json").exists()


def test_receipt_rejects_nonprivate_signature_archive_directory(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    directory = evidence["signature_dir"]
    assert isinstance(directory, Path)
    directory.chmod(0o755)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact mode 0700" in result.stderr


def test_receipt_rejects_signature_archive_wrong_name(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    attestation = evidence["signature_attestation"]
    assert isinstance(attestation, Path)
    renamed = attestation.with_name("attestation.json")
    attestation.rename(renamed)
    evidence["signature_attestation"] = renamed

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "wrong exact name" in result.stderr


def test_receipt_rejects_signature_archives_split_across_directories(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    revocation = evidence["signature_revocation"]
    assert isinstance(revocation, Path)
    other = tmp_path / "other-private"
    other.mkdir(mode=0o700)
    moved = other / revocation.name
    revocation.rename(moved)
    evidence["signature_revocation"] = moved

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "do not share one directory" in result.stderr


def test_receipt_rejects_signature_archive_symlink(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    raw_commit = evidence["signature_raw_commit"]
    assert isinstance(raw_commit, Path)
    real = raw_commit.with_name("raw-commit-real")
    raw_commit.rename(real)
    raw_commit.symlink_to(real.name)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "resolved and non-symlinked" in result.stderr


def test_receipt_rejects_hardlinked_signature_archives(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    allowed = evidence["signature_allowed_signers"]
    revocation = evidence["signature_revocation"]
    assert isinstance(allowed, Path)
    assert isinstance(revocation, Path)
    revocation.unlink()
    os.link(allowed, revocation)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "singly linked" in result.stderr


def test_receipt_rejects_signature_directory_inside_release_root(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    root = evidence["release_root"]
    assert isinstance(root, Path)
    nested = root / "release-identity"
    nested.mkdir(mode=0o700)
    nested.chmod(0o700)
    evidence["signature_dir"] = nested
    for key in (
        "signature_attestation",
        "signature_transcript",
        "signature_raw_commit",
        "signature_cargo_lock",
        "signature_allowed_signers",
        "signature_revocation",
        "signature_git",
        "signature_ssh_keygen",
    ):
        old_path = evidence[key]
        assert isinstance(old_path, Path)
        moved = nested / old_path.name
        old_path.rename(moved)
        evidence[key] = moved

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact bootstrap release-runner source" in result.stderr


@pytest.mark.parametrize(
    "mutation",
    [
        "schema-version",
        "archive-name",
        "candidate-oid",
        "environment-home",
        "environment-extra",
        "tools-disagree",
        "policies-disagree",
        "replay-root",
        "replay-environment",
        "replay-policy",
        "historical-symbolic-head",
        "replay-symbolic-head",
        "verify-failed",
        "show-size",
        "show-base64",
        "show-bad-status",
        "probe-replay",
    ],
)
def test_receipt_rejects_tampered_signature_transcript(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)

    def apply(value: dict[str, object]) -> None:
        if mutation == "schema-version":
            value["schema_version"] = 1
        elif mutation == "archive-name":
            value["archive_names"]["raw_commit"] = "commit"
        elif mutation == "candidate-oid":
            value["candidate_commit_oid"] = "9" * 40
        elif mutation == "environment-home":
            value["environment"]["HOME"] = "/tmp/untrusted-home"
        elif mutation == "environment-extra":
            value["environment"]["GIT_CONFIG_COUNT"] = "1"
        elif mutation == "tools-disagree":
            value["tools"]["git"]["observed_sha256"] = "4" * 64
        elif mutation == "policies-disagree":
            value["policies"]["signature_format"] = "openpgp"
        elif mutation == "replay-root":
            value["replay"]["candidate_root"] = "HEAD"
        elif mutation == "replay-environment":
            value["replay"]["environment"]["HOME"] = str(
                evidence["signature_dir"]
            )
        elif mutation == "replay-policy":
            value["replay"]["policy_overrides"][5] = "gpg.ssh.program=ssh-keygen"
        elif mutation == "historical-symbolic-head":
            value["commands"]["verify_commit"]["argv"][-1] = "HEAD"
        elif mutation == "replay-symbolic-head":
            value["commands"]["verify_commit"]["replay_argv"][-1] = "HEAD"
        elif mutation == "verify-failed":
            value["commands"]["verify_commit"]["exit_status"] = 1
        elif mutation == "show-size":
            value["commands"]["show_signature_metadata"]["stdout_size_bytes"] += 1
        elif mutation == "show-base64":
            value["commands"]["show_signature_metadata"]["stdout_base64"] = "***"
        elif mutation == "show-bad-status":
            command = value["commands"]["show_signature_metadata"]
            assert isinstance(command, dict)
            set_command_stream(
                command,
                "stdout",
                (
                    f"B\0{evidence['expected_signer_fingerprint']}\0\0"
                    f"{evidence['signer_principal']}\0\n"
                ).encode("utf-8"),
            )
        elif mutation == "probe-replay":
            value["tool_probes"]["ssh_keygen_usage"]["replay_argv"][0] = (
                "ssh-keygen"
            )
        else:
            raise AssertionError(mutation)

    mutate_transcript(evidence, apply)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert not (tmp_path / "receipt.json").exists()


@pytest.mark.parametrize(
    "failure",
    ["verify-failure", "metadata-change", "raw-commit-change", "top-level-change"],
)
def test_receipt_replays_archived_git_and_rejects_runtime_divergence(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failure: str,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    git_path = evidence["signature_git"]
    assert isinstance(git_path, Path)
    source = git_path.read_text(encoding="utf-8")
    if failure == "verify-failure":
        source = source.replace(
            "'Good fixture SSH signature' >&2 ;;",
            "'Good fixture SSH signature' >&2; exit 73 ;;",
        )
    elif failure == "metadata-change":
        source = source.replace("SHA256:" + "A" * 43, "SHA256:" + "B" * 43)
    elif failure == "raw-commit-change":
        source = source.replace(
            "Sumeragi v2 release fixture", "Sumeragi v2 changed release fixture"
        )
    elif failure == "top-level-change":
        other_root = tmp_path / "other-root"
        other_root.mkdir()
        source = source.replace(
            "'rev-parse --show-toplevel') pwd -P ;;",
            f"'rev-parse --show-toplevel') printf '%s\\n' '{other_root}' ;;",
        )
    else:
        raise AssertionError(failure)
    rebind_git_archive(evidence, source)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "archived Git" in result.stderr or "signature replay" in result.stderr

    if failure == "verify-failure":
        module = load_writer_module()
        environment = module._closed_replay_environment(tmp_path)
        real_popen = module.subprocess.Popen
        for swap_kind in ("file", "ancestor"):
            swap_root = tmp_path / f"execution-swap-{swap_kind}"
            trusted_directory = swap_root / "trusted"
            trusted_directory.mkdir(parents=True)
            trusted = trusted_directory / "tool"
            trusted.write_text(
                "#!/bin/sh\nprintf 'trusted\\n'\n", encoding="utf-8"
            )
            trusted.chmod(0o500)
            contract = module._bounded_path_contract(
                trusted,
                "fixture trusted executable",
                maximum_bytes=4096,
                allowed_owners={os.geteuid()},
                executable=True,
            )
            if swap_kind == "file":
                malicious = trusted_directory / "malicious"
                malicious.write_text(
                    "#!/bin/sh\nprintf 'malicious\\n'\n", encoding="utf-8"
                )
                malicious.chmod(0o500)
                saved = trusted_directory / "trusted.saved"

                def swapping_popen(
                    *args: object, **kwargs: object
                ) -> subprocess.Popen[bytes]:
                    trusted.rename(saved)
                    malicious.rename(trusted)
                    process = real_popen(*args, **kwargs)
                    process.wait(timeout=5)
                    trusted.rename(malicious)
                    saved.rename(trusted)
                    return process

            else:
                malicious_directory = swap_root / "malicious"
                malicious_directory.mkdir()
                malicious = malicious_directory / "tool"
                malicious.write_text(
                    "#!/bin/sh\nprintf 'malicious\\n'\n", encoding="utf-8"
                )
                malicious.chmod(0o500)
                saved = swap_root / "trusted.saved"

                def swapping_popen(
                    *args: object, **kwargs: object
                ) -> subprocess.Popen[bytes]:
                    trusted_directory.rename(saved)
                    malicious_directory.rename(trusted_directory)
                    process = real_popen(*args, **kwargs)
                    process.wait(timeout=5)
                    trusted_directory.rename(malicious_directory)
                    saved.rename(trusted_directory)
                    return process

            monkeypatch.setattr(module.subprocess, "Popen", swapping_popen)
            with pytest.raises(
                module.ReceiptError,
                match="changed (?:while pinned|during process execution)",
            ):
                module._run_bounded_replay(
                    trusted,
                    [],
                    cwd=tmp_path,
                    environment=environment,
                    name="fixture swapped executable",
                    executable_contract=contract,
                )
            monkeypatch.setattr(module.subprocess, "Popen", real_popen)
        interpreter = Path(sys.executable).resolve(strict=True)
        with pytest.raises(module.ReceiptError, match="output exceeds"):
            module._run_bounded_replay(
                interpreter,
                ["-I", "-S", "-c", "import os; os.write(1, b'x' * 4096)"],
                cwd=tmp_path,
                environment=environment,
                name="fixture validator",
                maximum_output_bytes=128,
            )
        monkeypatch.setattr(module, "_REPLAY_TIMEOUT_SECONDS", 0.05)
        with pytest.raises(module.ReceiptError, match="exceeded its timeout"):
            module._run_bounded_replay(
                interpreter,
                ["-I", "-S", "-c", "import time; time.sleep(2)"],
                cwd=tmp_path,
                environment=environment,
                name="fixture validator",
            )


def test_receipt_rejects_fully_rebound_cross_policy_allowed_signers(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    allowed = evidence["signature_allowed_signers"]
    transcript_path = evidence["signature_transcript"]
    attestation_path = evidence["signature_attestation"]
    assert isinstance(allowed, Path)
    assert isinstance(transcript_path, Path)
    assert isinstance(attestation_path, Path)
    allowed.chmod(0o600)
    allowed.write_text(
        "attacker@example.test ssh-ed25519 AAAAC3NzaAttacker\n", encoding="utf-8"
    )
    allowed.chmod(0o400)
    forged_digest = sha256(allowed)
    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    forged_policy = protected_metadata(allowed, 0o400, forged_digest)
    transcript["policies"]["ssh_allowed_signers"] = forged_policy
    attestation["policies"]["ssh_allowed_signers"] = forged_policy
    attestation["evidence"]["ssh_allowed_signers"] = artifact_metadata(allowed, 0o400)
    rewrite_json(transcript_path, transcript)
    attestation["evidence"]["verify_transcript"] = artifact_metadata(
        transcript_path, 0o400
    )
    rewrite_json(attestation_path, attestation)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "out-of-band digest" in result.stderr


def test_receipt_accepts_transcript_published_by_chaos_launcher(
    tmp_path: Path,
) -> None:
    release_root = tmp_path / "release"
    release_root.mkdir()
    evidence = make_evidence(release_root)
    chaos_symbols = runpy.run_path(
        str(ROOT_DIR / "pytests" / "scripts" / "sumeragi_v2_chaos_release_test.py")
    )
    launcher_root = tmp_path / "launcher"
    launcher_root.mkdir()
    launcher, env, chaos_evidence = chaos_symbols["_fixture"](
        launcher_root,
        manifest=evidence["sealed_manifest"],
        head=evidence["head"],
        tree=evidence["tree"],
        lock=evidence["lock"],
    )

    launch_result = chaos_symbols["_run"](launcher, env)

    assert launch_result.returncode == 0, launch_result.stderr
    invocations = list(chaos_evidence.glob("invocation.*"))
    assert len(invocations) == 1
    evidence["chaos_completion"] = invocations[0] / "COMPLETED.tsv"
    evidence["chaos_log"] = invocations[0] / "chaos-100k.log"
    writer = fixture_writer(tmp_path / "writer")
    output = terminal_output_path(evidence)

    receipt_result = run_writer(evidence, output, writer)

    assert receipt_result.returncode == 0, receipt_result.stderr
    assert json.loads(output.read_text(encoding="utf-8"))["result"] == "release-complete"


@pytest.mark.parametrize(
    "completion_name",
    [
        "corridor_completion",
        "formal_completion",
        "seed_completion",
        "taira_completion",
    ],
)
def test_receipt_rejects_cross_source_completion(
    tmp_path: Path, completion_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence[completion_name]
    assert isinstance(completion, Path)
    completion.write_text(
        completion.read_text(encoding="utf-8").replace("b" * 64, "c" * 64),
        encoding="utf-8",
    )
    output = tmp_path / "RELEASE_COMPLETED.json"
    output.write_text("previous valid receipt\n", encoding="utf-8")

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert (
        "not bound" in result.stderr
        or "exact release matrix" in result.stderr
        or "exact release preflight" in result.stderr
    )
    assert output.read_text(encoding="utf-8") == "previous valid receipt\n"


@pytest.mark.parametrize(
    ("artifact_name", "error_fragment"),
    [
        ("formal_log", "formal gate log digest mismatch"),
        ("formal_ledger", "formal proof ledger digest mismatch"),
        ("formal_evidence", "formal proof evidence digest mismatch"),
        ("formal_verus_evidence", "formal Verus evidence digest mismatch"),
        ("formal_verus_log", "formal Verus log digest mismatch"),
        (
            "formal_multilane_apalache_evidence",
            "formal multilane Apalache evidence digest mismatch",
        ),
        (
            "formal_production_trace_extraction_evidence",
            "formal production trace-extraction evidence digest mismatch",
        ),
        ("formal_toolchain", "formal toolchain digest mismatch"),
        ("formal_tlaps_resource_jsonl", "TLAPS resource samples digest mismatch"),
        ("formal_tlaps_resource_summary", "TLAPS resource summary digest mismatch"),
        ("formal_verus_tool", "formal verus tool digest mismatch"),
        ("corridor_summary", "corridor summary digest mismatch"),
        ("corridor_required", "corridor production inventory digest mismatch"),
        ("corridor_g_unit", "corridor G-UNIT inventory digest mismatch"),
        ("corridor_log", "corridor log 0 digest mismatch"),
        (
            "corridor_cargo_tool",
            "bootstrap runner tool cargo integrity binding is wrong",
        ),
        ("seed_summary", "summary digest mismatch"),
        ("seed_log", "seed run log 17 digest mismatch"),
        (
            "seed_localnet_manifest_index",
            "seed localnet manifest index digest mismatch",
        ),
        ("seed_localnet_manifest", "seed localnet manifest 17 digest mismatch"),
        (
            "seed_localnet_file",
            "seed localnet manifest 17 does not match retained content",
        ),
        ("chaos_log", "log digest mismatch"),
        ("taira_evidence", "evidence digest mismatch"),
    ],
)
def test_receipt_rejects_artifact_changed_after_completion(
    tmp_path: Path, artifact_name: str, error_fragment: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    original_mode = stat.S_IMODE(artifact.stat().st_mode)
    artifact.chmod(original_mode | stat.S_IWUSR)
    artifact.write_text("tampered after completion\n", encoding="utf-8")
    artifact.chmod(original_mode)
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert error_fragment in result.stderr
    assert not output.exists()


def test_receipt_rejects_candidate_and_sealed_git_identity_mismatch(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    candidate_path = evidence["candidate"]
    assert isinstance(candidate_path, Path)
    candidate = json.loads(candidate_path.read_text(encoding="utf-8"))
    candidate["head_commit"] = "9" * 40
    candidate_path.write_bytes(canonical_json(candidate))
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert "disagree on head_commit" in result.stderr
    assert not output.exists()


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("head_commit", "9" * 40),
        ("head_tree", "8" * 40),
        ("cargo_lock_sha256", "7" * 64),
    ],
)
def test_receipt_rejects_seed_exact_identity_mismatch(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact release matrix" in result.stderr


@pytest.mark.parametrize("field", ["completed_runs", "expected_runs"])
def test_receipt_rejects_stale_four_scenario_seed_count(
    tmp_path: Path, field: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = "128"
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact release matrix" in result.stderr


def test_receipt_rejects_legacy_seed_completion_without_localnet_manifests(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields = {
        name: value
        for name, value in fields.items()
        if not name.startswith("localnet_manifest")
    }
    fields["schema_version"] = "1"
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "seed completion fields do not match its completion schema" in result.stderr


def test_receipt_rejects_seed_localnet_manifest_path_escape(
    tmp_path: Path,
) -> None:
    for mutation, expected_error in (
        ("completion", "seed localnet manifest index row 17 is not canonical"),
        ("symlink-parent", "seed localnet manifest 0 escaped its archive"),
    ):
        case_root = tmp_path / mutation
        case_root.mkdir()
        evidence = make_evidence(case_root)
        writer = fixture_writer(case_root)
        completion = evidence["seed_completion"]
        assert isinstance(completion, Path)
        if mutation == "completion":
            fields = dict(
                line.split("\t", 1)
                for line in completion.read_text(encoding="utf-8").splitlines()
            )
            fields["localnet_manifest_017_path"] = "../escaped-localnet.tsv"
            write_tsv(completion, fields)
        else:
            manifest = evidence["seed_localnet_manifest"]
            assert isinstance(manifest, Path)
            manifest_directory = manifest.parent
            escaped_directory = case_root / "escaped-localnet-manifests"
            manifest_directory.rename(escaped_directory)
            manifest_directory.symlink_to(escaped_directory, target_is_directory=True)

        result = run_writer(evidence, case_root / "receipt.json", writer)

        assert result.returncode == 1
        assert expected_error in result.stderr


def test_receipt_rejects_symlink_in_retained_seed_localnet(tmp_path: Path) -> None:
    for mutation, expected_error in (
        ("entry", "contains a symlink"),
        ("parent", "root must be a resolved real directory"),
    ):
        case_root = tmp_path / mutation
        case_root.mkdir()
        evidence = make_evidence(case_root)
        writer = fixture_writer(case_root)
        localnet = evidence["seed_localnet"]
        assert isinstance(localnet, Path)
        if mutation == "entry":
            outside = case_root / "outside-localnet"
            outside.write_text("outside\n", encoding="utf-8")
            (localnet / "escape").symlink_to(outside)
            expected_index = 17
        else:
            localnets_directory = localnet.parent
            escaped_directory = case_root / "escaped-localnets"
            localnets_directory.rename(escaped_directory)
            localnets_directory.symlink_to(escaped_directory, target_is_directory=True)
            expected_index = 0

        result = run_writer(evidence, case_root / "receipt.json", writer)

        assert result.returncode == 1
        assert (
            f"seed retained localnet {expected_index} is unsafe or unstable"
            in result.stderr
        )
        assert expected_error in result.stderr


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("head_commit", "9" * 40),
        ("head_tree", "8" * 40),
        ("cargo_lock_sha256", "7" * 64),
    ],
)
def test_receipt_rejects_taira_exact_identity_mismatch(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["taira_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact release identity" in result.stderr


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("cargo_version", "cargo 9.99.9 (forged 2099-01-01)"),
        ("rustc_version", "rustc 9.99.9 (forged 2099-01-01)"),
    ],
)
def test_receipt_rejects_noncanonical_rust_tool_version(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["corridor_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "rust-toolchain.toml" in result.stderr

    tool = field.removesuffix("_version")
    fixture_key = f"corridor_{tool}_tool"
    for case_name, mutate_bytes in (
        ("same-bytes-different-path", False),
        ("exact-output-different-bytes", True),
    ):
        case_root = tmp_path / case_name
        case_root.mkdir()
        cross_bound = make_evidence(case_root)
        cross_writer = fixture_writer(case_root)
        source = cross_bound[fixture_key]
        cross_completion = cross_bound["corridor_completion"]
        assert isinstance(source, Path)
        assert isinstance(cross_completion, Path)
        alternate = case_root / f"alternate-{tool}"
        alternate.write_bytes(source.read_bytes())
        if mutate_bytes:
            alternate.write_bytes(
                alternate.read_bytes()
                + b"# distinct executable with the same accepted output\n"
            )
        alternate.chmod(0o500)
        cross_fields = read_tsv_fields(cross_completion)
        cross_fields[f"{tool}_path"] = str(alternate.resolve())
        cross_fields[f"{tool}_sha256"] = sha256(alternate)
        write_tsv(cross_completion, cross_fields)

        cross_result = run_writer(
            cross_bound,
            case_root / "receipt.json",
            cross_writer,
        )

        assert cross_result.returncode == 1
        assert (
            f"corridor {tool} is not the authenticated bootstrap runner tool"
            in cross_result.stderr
        )


def test_receipt_rejects_external_cargo_home_configuration(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    cargo_home = evidence["corridor_cargo_home"]
    assert isinstance(cargo_home, Path)
    (cargo_home / "config.toml").write_text(
        '[target."cfg(all())"]\nrunner = "fake-test-runner"\n', encoding="utf-8"
    )

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "contains external configuration" in result.stderr


def test_receipt_rejects_rehashed_missing_corridor_leg(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    lines = summary.read_text(encoding="utf-8").splitlines()
    summary.write_text("\n".join(lines[:-1]) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "must contain every exact release leg" in result.stderr


def test_receipt_rejects_rehashed_noncanonical_g_unit_inventory(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    inventory = evidence["corridor_g_unit"]
    completion = evidence["corridor_completion"]
    assert isinstance(inventory, Path)
    assert isinstance(completion, Path)
    lines = inventory.read_text(encoding="utf-8").splitlines()
    row = lines[1].split("\t")
    row[2] = "native_amx::tests::forged_g_unit_identity"
    lines[1] = "\t".join(row)
    inventory.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["g_unit_inventory_sha256"] = sha256(inventory)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "corridor G-UNIT inventory row 0 is not canonical" in result.stderr


def test_receipt_rejects_rehashed_g_unit_log_missing_named_test(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    logs = evidence["corridor_logs"]
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(logs, list)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    log = logs[0]
    first_result = next(
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if line.startswith("test ") and line.endswith(" ... ok")
    )
    log.write_text(
        log.read_text(encoding="utf-8").replace(first_result + "\n", "", 1),
        encoding="utf-8",
    )
    summary_lines = summary.read_text(encoding="utf-8").splitlines()
    row = summary_lines[1].split("\t")
    row[7] = sha256(log)
    summary_lines[1] = "\t".join(row)
    summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "G-UNIT leg g-unit-iroha-core lacks one required passing test" in result.stderr


def test_receipt_rejects_missing_or_altered_source_sealed_full_suite_leg(
    tmp_path: Path,
) -> None:
    for mutation in ("missing", "altered-command"):
        case_root = tmp_path / mutation
        case_root.mkdir()
        evidence = make_evidence(case_root)
        writer = fixture_writer(case_root)
        summary = evidence["corridor_summary"]
        completion = evidence["corridor_completion"]
        assert isinstance(summary, Path)
        assert isinstance(completion, Path)
        lines = summary.read_text(encoding="utf-8").splitlines()
        row_index = next(
            index
            for index, line in enumerate(lines[1:], 1)
            if "\tsource-sealed-workspace-tests\t" in line
        )
        if mutation == "missing":
            del lines[row_index]
        else:
            row = lines[row_index].split("\t")
            row[9] = "cargo test --workspace"
            lines[row_index] = "\t".join(row)
        summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
        fields = dict(
            line.split("\t", 1)
            for line in completion.read_text(encoding="utf-8").splitlines()
        )
        fields["summary_sha256"] = sha256(summary)
        write_tsv(completion, fields)

        result = run_writer(evidence, case_root / "receipt.json", writer)

        assert result.returncode == 1
        assert (
            "must contain every exact release leg" in result.stderr
            or "is not the exact release leg" in result.stderr
        )


def test_receipt_rejects_rehashed_malformed_corridor_log(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    log = evidence["corridor_log"]
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    log.write_text("fabricated pass without Cargo semantics\n", encoding="utf-8")
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[1].split("\t")
    row[7] = sha256(log)
    lines[1] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "ambiguous Cargo transcript" in result.stderr


def test_receipt_rejects_sumeragi_diagnostics_rust_log_missing_named_test(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    summary_lines = summary.read_text(encoding="utf-8").splitlines()
    row_index = next(
        index
        for index, line in enumerate(summary_lines[1:], 1)
        if "\tsumeragi-diagnostics-rust\t" in line
    )
    row = summary_lines[row_index].split("\t")
    log = summary.parent / row[8]
    log_lines = log.read_text(encoding="utf-8").splitlines()
    named_test_index = next(
        index
        for index, line in enumerate(log_lines)
        if line.startswith("test client::tests::get_sumeragi_")
    )
    del log_lines[named_test_index]
    log.write_text("\n".join(log_lines) + "\n", encoding="utf-8")
    row[7] = sha256(log)
    summary_lines[row_index] = "\t".join(row)
    summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    completion_fields = read_tsv_fields(completion)
    completion_fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, completion_fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert (
        "corridor exact Cargo leg sumeragi-diagnostics-rust lacks its named test"
        in result.stderr
    )


def test_receipt_rejects_sumeragi_diagnostics_suite_source_drift(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    release_root = evidence["release_root"]
    assert isinstance(release_root, Path)
    source = (
        release_root
        / "python/iroha_python/tests/client_sumeragi_v2_status_test.py"
    )
    source.write_bytes(source.read_bytes() + b"\n# forged post-harness source drift\n")

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert (
        "corridor Sumeragi v2 SDK diagnostics python leg is not bound to the "
        "exact suite sources" in result.stderr
    )


def test_hand_invoked_writer_rejects_fake_machine_completion_artifacts(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, SCRIPT)

    assert result.returncode == 1
    assert (
        "archived formal ledger has an invalid cross-tool evidence requirement"
        in result.stderr
    )
    assert not output.exists()


def test_receipt_rejects_rehashed_seed_log_without_required_semantics(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    run_log = evidence["seed_log"]
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(run_log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    run_log.write_text("forged success without libtest semantics\n", encoding="utf-8")
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    row[7] = sha256(run_log)
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing scenario" in result.stderr


def test_receipt_requires_exact_nocapture_seed_diagnostic(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    run_log = evidence["seed_log"]
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(run_log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    run_log.write_text(
        run_log.read_text(encoding="utf-8").replace(
            "deterministic network seed = ", "deterministic network seed = wrong-"
        ),
        encoding="utf-8",
    )
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    row[7] = sha256(run_log)
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing scenario" in result.stderr

    command_root = tmp_path / "hidden-start-retry"
    command_root.mkdir()
    command_evidence = make_evidence(command_root)
    command_writer = fixture_writer(command_root)
    command_summary = command_evidence["seed_summary"]
    command_completion = command_evidence["seed_completion"]
    assert isinstance(command_summary, Path)
    assert isinstance(command_completion, Path)
    command_lines = command_summary.read_text(encoding="utf-8").splitlines()
    command_row = command_lines[18].split("\t")
    command_row[10] = command_row[10].replace(
        "IROHA_TEST_NETWORK_START_ATTEMPTS=1",
        "IROHA_TEST_NETWORK_START_ATTEMPTS=2",
    )
    command_lines[18] = "\t".join(command_row)
    command_summary.write_text(
        "\n".join(command_lines) + "\n", encoding="utf-8"
    )
    command_fields = dict(
        line.split("\t", 1)
        for line in command_completion.read_text(encoding="utf-8").splitlines()
    )
    command_fields["summary_sha256"] = sha256(command_summary)
    write_tsv(command_completion, command_fields)

    command_result = run_writer(
        command_evidence, command_root / "receipt.json", command_writer
    )

    assert command_result.returncode == 1
    assert (
        "seed summary row 17 is not the exact release run" in command_result.stderr
    )


@pytest.mark.parametrize(
    ("pattern", "replacement"),
    (
        (r"IROHA_TEST_SKIP_BUILD=1", "IROHA_TEST_SKIP_BUILD=0"),
        (
            r"IROHA_TEST_ALLOW_REENTRANT_BUILD=0",
            "IROHA_TEST_ALLOW_REENTRANT_BUILD=1",
        ),
        (r"cargo test --locked --offline", "cargo test --locked"),
        (
            r"IROHA_TEST_TARGET_DIR=[^ ]+",
            "IROHA_TEST_TARGET_DIR=/tmp/escaped-program-target",
        ),
        (
            r"TEST_NETWORK_BIN_IROHAD=[^ ]+",
            "TEST_NETWORK_BIN_IROHAD=/tmp/escaped-irohad",
        ),
    ),
)
def test_receipt_rejects_nested_or_unbound_seed_replay(
    tmp_path: Path,
    pattern: str,
    replacement: str,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)

    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    mutated, replacement_count = re.subn(pattern, replacement, row[10], count=1)
    assert replacement_count == 1
    row[10] = mutated
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    completion_fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    completion_fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, completion_fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "seed summary row 17 is not the exact release run" in result.stderr


def test_receipt_rejects_seed_replay_prebuilt_manifest_drift(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    manifest = evidence["prebuilt_manifest"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    assert isinstance(manifest, Path)

    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    expected = f"IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={sha256(manifest)}"
    mutated, replacement_count = re.subn(
        re.escape(expected),
        f"IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={'0' * 64}",
        row[10],
        count=1,
    )
    assert replacement_count == 1
    row[10] = mutated
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    completion_fields = read_tsv_fields(completion)
    completion_fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, completion_fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "seed summary row 17 is not the exact release run" in result.stderr


def test_receipt_rejects_rehashed_chaos_log_without_required_semantics(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    chaos_log = evidence["chaos_log"]
    completion = evidence["chaos_completion"]
    assert isinstance(chaos_log, Path)
    assert isinstance(completion, Path)
    chaos_log.write_text("forged 100000-height success\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["log_sha256"] = sha256(chaos_log)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing release test" in result.stderr

    duplicate_root = tmp_path / "duplicate-marker"
    duplicate_root.mkdir()
    duplicate_evidence = make_evidence(duplicate_root)
    duplicate_writer = fixture_writer(duplicate_root)
    duplicate_log = duplicate_evidence["chaos_log"]
    duplicate_completion = duplicate_evidence["chaos_completion"]
    assert isinstance(duplicate_log, Path)
    assert isinstance(duplicate_completion, Path)
    duplicate_log.write_text(
        duplicate_log.read_text(encoding="utf-8") + CHAOS_MARKER + "\n",
        encoding="utf-8",
    )
    duplicate_fields = dict(
        line.split("\t", 1)
        for line in duplicate_completion.read_text(encoding="utf-8").splitlines()
    )
    duplicate_fields["log_sha256"] = sha256(duplicate_log)
    write_tsv(duplicate_completion, duplicate_fields)

    duplicate_result = run_writer(
        duplicate_evidence, duplicate_root / "receipt.json", duplicate_writer
    )

    assert duplicate_result.returncode == 1
    assert "does not prove its one exact passing release test" in (
        duplicate_result.stderr
    )

    counter_root = tmp_path / "wrong-counter"
    counter_root.mkdir()
    counter_evidence = make_evidence(counter_root)
    counter_writer = fixture_writer(counter_root)
    counter_completion = counter_evidence["chaos_completion"]
    assert isinstance(counter_completion, Path)
    counter_fields = dict(
        line.split("\t", 1)
        for line in counter_completion.read_text(encoding="utf-8").splitlines()
    )
    counter_fields["wal_append_restarts"] = "315"
    write_tsv(counter_completion, counter_fields)

    counter_result = run_writer(
        counter_evidence, counter_root / "receipt.json", counter_writer
    )

    assert counter_result.returncode == 1
    assert "does not match the exact release identity and reducer schedule" in (
        counter_result.stderr
    )


def test_receipt_rejects_seed_summary_row_with_extra_column(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    lines = summary.read_text(encoding="utf-8").splitlines()
    lines[1] += "\tforged-extra-column"
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "extra or missing columns" in result.stderr


def test_receipt_revalidates_archived_taira_semantics(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    taira_log = evidence["taira_log"]
    completion = evidence["taira_completion"]
    assert isinstance(taira_log, Path)
    assert isinstance(completion, Path)
    original_log = taira_log.read_bytes()
    taira_log.write_text(
        "running 1 test\n"
        "test forged_taira_soak ... ok\n\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
        "42 filtered out; finished in 86400.01s\n",
        encoding="utf-8",
    )
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["log_sha256"] = sha256(taira_log)
    write_tsv(completion, fields)

    malformed_result = run_writer(evidence, tmp_path / "malformed-receipt.json", writer)

    assert malformed_result.returncode == 1
    assert "Taira log does not prove its one exact passing soak" in malformed_result.stderr

    taira_log.write_bytes(original_log)
    fields["log_sha256"] = sha256(taira_log)
    write_tsv(completion, fields)
    (writer.parent / "check_taira_v2_soak_evidence.py").write_text(
        "raise SystemExit(72)\n", encoding="utf-8"
    )

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "archived Taira evidence failed release validation" in result.stderr


for _release_receipt_test_component in RELEASE_RECEIPT_TEST_COMPONENT_FILES:
    _execute_test_component(_release_receipt_test_component)
del _release_receipt_test_component
