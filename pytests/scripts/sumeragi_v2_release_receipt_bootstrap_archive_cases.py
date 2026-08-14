"""Bootstrap archive cases executed by the parent release-receipt suite."""

from __future__ import annotations

import json
import importlib.util
import hashlib
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import sysconfig
import time

import pytest

from pytests.scripts.sumeragi_v2_release_receipt_components import (
    run_fixture_cargo_cache_copy,
)
from pytests.scripts.sumeragi_v2_release_bootstrap_tool_manifest_support import (
    REQUIRED_RUNNER_TOOL_NAMES,
)
from pytests.scripts.sumeragi_v2_release_receipt_test_support import (
    canonical_json,
    sha256,
)

ROOT_DIR = Path(__file__).resolve().parents[2]


def _load_receipt_writer_module() -> object:
    """Load the production writer without relying on the parent suite namespace."""

    script = ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py"
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_receipt_archive_case_writer", script
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _fixture_validator_invocation(
    helper: object,
    *,
    invocation: Path,
    bootstrap: Path,
    source: Path,
    receipt: Path,
    acknowledgment: Path,
    source_manifest_sha256: str,
    candidate_root: Path,
    scaling_evidence_manifest: Path,
    expected_signer_fingerprint: str,
    scaling_digests: dict[str, str],
) -> dict[str, object]:
    known = _fixture_validator_values(
        helper,
        invocation=invocation,
        bootstrap=bootstrap,
        source=source,
        receipt=receipt,
        acknowledgment=acknowledgment,
        source_manifest_sha256=source_manifest_sha256,
        candidate_root=candidate_root,
        scaling_evidence_manifest=scaling_evidence_manifest,
        expected_signer_fingerprint=expected_signer_fingerprint,
        scaling_digests=scaling_digests,
    )
    bindings = []
    for name in helper.VALIDATOR_OPTION_ORDER:
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
    helper: object,
    *,
    invocation: Path,
    bootstrap: Path,
    source: Path,
    receipt: Path,
    acknowledgment: Path,
    source_manifest_sha256: str,
    candidate_root: Path,
    scaling_evidence_manifest: Path,
    expected_signer_fingerprint: str,
    scaling_digests: dict[str, str],
) -> dict[str, tuple[str, str | bool]]:
    values: dict[str, tuple[str, str | bool]] = {
        name: ("flag", True)
        if name == "--verify-existing"
        else ("path", str(invocation / "fixture" / name[2:]))
        if name in helper.VALIDATOR_PATH_OPTIONS
        else ("text", f"fixture:{name}")
        for name in helper.VALIDATOR_OPTION_ORDER
    }
    values.update({
        "--candidate-identity": ("path", str(bootstrap / "candidate-identity.json")),
        "--sealed-identity": ("path", str(invocation / "sealed-identity.json")),
        "--release-root": ("path", str(source)),
        "--bootstrap-completion": ("path", str(bootstrap / "BOOTSTRAP_COMPLETED.json")),
        "--bootstrap-evidence-dir": ("path", str(bootstrap)),
        "--bootstrap-identity": ("path", str(bootstrap / "candidate-identity.json")),
        "--bootstrap-attestation": ("path", str(bootstrap / "identity-attestation.json")),
        "--bootstrap-transcript": ("path", str(bootstrap / "identity-transcript.json")),
        "--expected-bootstrap-completion-sha256": (
            "text", hashlib.sha256((bootstrap / "BOOTSTRAP_COMPLETED.json").read_bytes()).hexdigest()
        ),
        "--bootstrap-candidate-root": ("path", str(candidate_root)),
        "--bootstrap-runner": (
            "path", str(candidate_root / "scripts" / "run_sumeragi_v2_release_gates.sh")
        ),
        "--signature-attestation": ("path", str(bootstrap / "identity-attestation.json")),
        "--signature-transcript": ("path", str(bootstrap / "identity-transcript.json")),
        "--signature-raw-commit": ("path", str(bootstrap / "identity-raw-commit")),
        "--signature-cargo-lock": ("path", str(bootstrap / "identity-Cargo.lock")),
        "--signature-allowed-signers": ("path", str(bootstrap / "identity-allowed-signers")),
        "--signature-revocation": ("path", str(bootstrap / "identity-revocation")),
        "--signature-git": ("path", str(bootstrap / "identity-git")),
        "--signature-ssh-keygen": ("path", str(bootstrap / "identity-ssh-keygen")),
        "--expected-git-sha256": ("text", "1" * 64),
        "--expected-ssh-keygen-sha256": ("text", "2" * 64),
        "--expected-allowed-signers-sha256": ("text", "3" * 64),
        "--expected-revocation-sha256": ("text", "4" * 64),
        "--expected-signer-fingerprint": ("text", expected_signer_fingerprint),
        "--scaling-evidence-manifest": ("path", str(scaling_evidence_manifest)),
        "--sdk-dependency-archive": (
            "path", str(source.parent / "sdk-dependency-bundle.tar")
        ),
        "--sdk-dependency-input-inventory": (
            "path", str(source.parent / "sdk-dependency-input.json")
        ),
        "--sdk-dependency-final-work-inventory": (
            "path", str(source.parent / "sdk-dependency-work-final.json")
        ),
        "--runtime-tool-probe-manifest": (
            "path", str(source.parent / "runtime-tool-probe-manifest.json")
        ),
        "--runtime-tool-probe-result": (
            "path", str(source.parent / "runtime-tool-probe-result.json")
        ),
        "--expected-scaling-trial-harness-sha256": (
            "text", scaling_digests["trial_harness"]
        ),
        "--expected-scaling-configuration-sha256": (
            "text", scaling_digests["configuration"]
        ),
        "--expected-scaling-irohad-sha256": ("text", scaling_digests["irohad"]),
        "--expected-scaling-iroha-cli-sha256": (
            "text", scaling_digests["iroha_cli"]
        ),
        "--repository-root": ("path", str(source)),
        "--output": ("path", str(receipt)),
        "--verify-existing": ("flag", True),
        "--validation-ack": ("path", str(acknowledgment)),
        "--source-manifest-sha256": ("text", source_manifest_sha256),
    })
    return values


def _fixture_receipt_for_validator(
    values: dict[str, tuple[str, str | bool]],
) -> dict[str, object]:
    path = lambda name: {"path": values[name][1], "sha256": "1" * 64}
    return {
        "identity": {
            "sealed_source_manifest_sha256": values["--source-manifest-sha256"][1]
        },
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
                "files": [{
                    "relative_path": "scaling_evidence.json",
                    **path("--scaling-evidence-manifest"),
                }]
            },
            "multilane_scaling_trust_anchors": {
                "trial_harness_sha256": values["--expected-scaling-trial-harness-sha256"][1],
                "configuration_sha256": values["--expected-scaling-configuration-sha256"][1],
                "irohad_sha256": values["--expected-scaling-irohad-sha256"][1],
                "iroha_cli_sha256": values["--expected-scaling-iroha-cli-sha256"][1],
            },
        },
    }


def exercise_release_helper_fail_atomicity(
    helper: object, tmp_path: Path, runtime_sources: list[Path],
) -> None:
    """Inject late failures into every helper publication transaction."""

    def private_directory(path: Path) -> Path:
        path.mkdir(parents=True, mode=0o700); path.chmod(0o700)
        return path

    quarantine_root = private_directory(tmp_path / "fault-quarantine")
    quarantine_file = quarantine_root / "owned"
    quarantine_file.write_bytes(b"owned bytes stay intact\n")
    quarantine_metadata = quarantine_file.lstat()
    quarantine_fd = os.open(quarantine_root, helper._DIRECTORY_FLAGS)
    try:
        assert helper._owned_remove_entry(
            quarantine_fd, quarantine_file.name,
            (quarantine_metadata.st_dev, quarantine_metadata.st_ino), "owned file",
        )
    finally:
        os.close(quarantine_fd)
    retained = tuple(quarantine_root.iterdir())
    assert not quarantine_file.exists() and len(retained) == 1
    assert retained[0].name.startswith(".owned-quarantine.")
    assert retained[0].read_bytes() == b"owned bytes stay intact\n"

    replaced_source = private_directory(tmp_path / "fault-replaced-source")
    replaced_registry = private_directory(replaced_source / "registry")
    (replaced_registry / "entry").write_bytes(b"must not reach replacement\n")
    replaced_output = private_directory(tmp_path / "fault-replaced-output")
    replaced_home = private_directory(replaced_output / "cargo-home")
    original_entry_stat = helper._entry_stat
    swapped_directory = False

    def replace_created_directory(parent_fd: int, name: str, label: str) -> os.stat_result:
        nonlocal swapped_directory
        result = original_entry_stat(parent_fd, name, label)
        if label == "new cache directory registry" and not swapped_directory:
            swapped_directory = True
            os.rename(name, "saved-owned", src_dir_fd=parent_fd, dst_dir_fd=parent_fd)
            os.mkdir(name, mode=0o700, dir_fd=parent_fd)
        return result

    helper._entry_stat = replace_created_directory
    try:
        with pytest.raises(helper.CacheCopyError, match="private cache directory is unsafe"):
            helper.copy_cache(
                replaced_source, replaced_home, replaced_output / "input.json",
            )
    finally:
        helper._entry_stat = original_entry_stat
    assert not (replaced_home / "registry").exists()
    replacement_stages = tuple(path for path in replaced_home.iterdir() if path.name.startswith(".copy-stage."))
    assert len(replacement_stages) == 1 and not tuple(replacement_stages[0].iterdir())
    assert not (replaced_output / "input.json").exists()

    mid_source = private_directory(tmp_path / "fault-mid-source")
    mid_registry = private_directory(mid_source / "registry")
    (mid_registry / "entry").write_bytes(b"mid-copy\n")
    mid_output = private_directory(tmp_path / "fault-mid-output")
    mid_home = private_directory(mid_output / "cargo-home")
    original_copy_regular = helper._copy_regular
    helper._copy_regular = lambda *args, **kwargs: (_ for _ in ()).throw(
        helper.CacheCopyError("injected mid-copy failure")
    )
    try:
        with pytest.raises(helper.CacheCopyError, match="injected mid-copy"):
            helper.copy_cache(mid_source, mid_home, mid_output / "input.json")
    finally:
        helper._copy_regular = original_copy_regular
    assert not (mid_home / "registry").exists() and not (mid_output / "input.json").exists()
    assert any(path.name.startswith(".owned-quarantine.") for path in mid_home.iterdir())

    late_source = private_directory(tmp_path / "fault-late-source")
    late_registry = private_directory(late_source / "registry")
    (late_registry / "entry").write_bytes(b"late-copy\n")
    late_output = private_directory(tmp_path / "fault-late-output")
    late_home = private_directory(late_output / "cargo-home")
    original_revalidate = helper._revalidate_directory_path
    private_home_checks = 0

    def fail_second_private_home(path: Path, expected: os.stat_result, label: str) -> None:
        nonlocal private_home_checks
        original_revalidate(path, expected, label)
        if label == "private Cargo home":
            private_home_checks += 1
            if private_home_checks == 2:
                raise helper.CacheCopyError("injected final input revalidation")

    helper._revalidate_directory_path = fail_second_private_home
    try:
        with pytest.raises(helper.CacheCopyError, match="final input revalidation"):
            helper.copy_cache(late_source, late_home, late_output / "input.json")
    finally:
        helper._revalidate_directory_path = original_revalidate
    assert not (late_home / "registry").exists() and not (late_output / "input.json").exists()
    assert any(path.name.startswith(".owned-quarantine.") for path in late_home.iterdir())

    absent_output = private_directory(tmp_path / "fault-absent-output")
    absent_home = private_directory(absent_output / "cargo-home")
    absent_source = tmp_path / "absent-caller-home"
    original_publish = helper._publish_inventory

    def publish_then_create_source(path: Path, payload: bytes) -> object:
        result = original_publish(path, payload)
        absent_source.mkdir(mode=0o700)
        return result

    helper._publish_inventory = publish_then_create_source
    try:
        with pytest.raises(helper.CacheCopyError, match="appeared during cache publication"):
            helper.copy_cache(absent_source, absent_home, absent_output / "input.json")
    finally:
        helper._publish_inventory = original_publish
    assert absent_source.is_dir() and not (absent_output / "input.json").exists()

    final_output = private_directory(tmp_path / "fault-final-output")
    final_home = private_directory(final_output / "cargo-home")
    (final_home / "generated").write_bytes(b"final-state\n")
    private_home_checks = 0
    helper._revalidate_directory_path = fail_second_private_home
    try:
        with pytest.raises(helper.CacheCopyError, match="final input revalidation"):
            helper.snapshot_cache(final_home, final_output / "final.json")
    finally:
        helper._revalidate_directory_path = original_revalidate
    assert not (final_output / "final.json").exists()

    late_runtime = tmp_path / "fault-late-runtime"
    late_runtime_inventory = tmp_path / "fault-late-runtime.json"

    def fail_runtime_parent(path: Path, expected: os.stat_result, label: str) -> None:
        original_revalidate(path, expected, label)
        if label == "private runtime parent":
            raise helper.CacheCopyError("injected final runtime revalidation")

    helper._revalidate_directory_path = fail_runtime_parent
    try:
        with pytest.raises(helper.CacheCopyError, match="final runtime revalidation"):
            helper.copy_runtime(late_runtime, runtime_sources, late_runtime_inventory)
    finally:
        helper._revalidate_directory_path = original_revalidate
    assert not late_runtime.exists() and not late_runtime_inventory.exists()
    assert any(path.name.startswith(".owned-quarantine.") for path in tmp_path.iterdir())

    bundle_source = private_directory(tmp_path / "fault-bundle-source")
    private_directory(bundle_source / "nested")
    bundle_source_file = bundle_source / "nested" / "evidence.json"
    bundle_source_file.write_bytes(b"{\"fixture\":true}\n")
    bundle_root = tmp_path / "fault-private-bundle"
    bundle_inventory = tmp_path / "fault-private-bundle.json"
    helper.copy_private_bundle(bundle_source, bundle_root, bundle_inventory)
    bundle_document = json.loads(bundle_inventory.read_bytes())
    assert bundle_document["source_disclosure"] == "withheld"
    assert str(bundle_source).encode() not in bundle_inventory.read_bytes()
    helper._verify_private_bundle(bundle_source, bundle_root, bundle_inventory)
    original_bundle_inventory = bundle_inventory.read_bytes()
    rebound_bundle = json.loads(original_bundle_inventory)
    rebound_bundle["bundle_root"] = str(tmp_path / "fault-private-bundle-rebound")
    bundle_inventory.chmod(0o600); bundle_inventory.write_bytes(canonical_json(rebound_bundle)); bundle_inventory.chmod(0o400)
    with pytest.raises(helper.CacheCopyError, match="wrong private root"):
        helper._verify_private_bundle(bundle_source, bundle_root, bundle_inventory)
    bundle_inventory.chmod(0o600); bundle_inventory.write_bytes(original_bundle_inventory); bundle_inventory.chmod(0o400)
    private_bundle_file = bundle_root / "nested" / "evidence.json"
    private_bundle_file.chmod(0o600); private_bundle_file.write_bytes(b"mutated\n")
    with pytest.raises(helper.CacheCopyError, match="private bundle changed"):
        helper._verify_private_bundle(bundle_source, bundle_root, bundle_inventory)
    private_bundle_file.write_bytes(b"{\"fixture\":true}\n"); private_bundle_file.chmod(0o400)
    bundle_source_file.write_bytes(b"source changed\n")
    with pytest.raises(helper.CacheCopyError, match="runtime source file changed"):
        helper._verify_private_bundle(bundle_source, bundle_root, bundle_inventory)

    invocation = private_directory(tmp_path / "fault-seal-invocation")
    bootstrap = private_directory(tmp_path / "fault-seal-bootstrap")
    source = private_directory(invocation / "source")
    output = private_directory(invocation / "output")
    release = private_directory(output / "release")
    for disposable in (invocation / "runtime", invocation / "target", output / "home"):
        private_directory(disposable)
    identity = invocation / "sealed-identity.json"
    receipt = release / "RELEASE_COMPLETED.json"
    validator = bootstrap / "validate-receipt.py"
    completion = bootstrap / "BOOTSTRAP_COMPLETED.json"
    completion_value = {
        "trusted_inputs": {
            "git": {"sha256": "1" * 64},
            "ssh_keygen": {"sha256": "2" * 64},
            "allowed_signers": {"sha256": "3" * 64},
            "revocation": {"sha256": "4" * 64},
        }
    }
    for path, data in (
        (identity, b"{}\n"),
        (receipt, b"{\"identity\":{}}\n"),
        (validator, b"# validator\n"),
        (completion, canonical_json(completion_value)),
    ):
        path.write_bytes(data); path.chmod(0o400)
    manifest_digest = "a" * 64
    stdout = f"Sumeragi v2 aggregate release receipt verified: {receipt}\n".encode()
    ack = invocation / "receipt-validation-ack.json"
    candidate_root = private_directory(tmp_path / "fault-seal-candidate")
    scaling_root = private_directory(tmp_path / "fault-seal-scaling")
    scaling_manifest = scaling_root / "scaling_evidence.json"
    scaling_manifest.write_bytes(b"{}\n")
    expected_signer_fingerprint = "SHA256:" + "A" * 43
    scaling_digests = {
        "trial_harness": "5" * 64,
        "configuration": "6" * 64,
        "irohad": "7" * 64,
        "iroha_cli": "8" * 64,
    }
    expected_invocation_values = _fixture_validator_values(
        helper,
        invocation=invocation,
        bootstrap=bootstrap,
        source=source,
        receipt=receipt,
        acknowledgment=ack,
        source_manifest_sha256=manifest_digest,
        candidate_root=candidate_root,
        scaling_evidence_manifest=scaling_manifest,
        expected_signer_fingerprint=expected_signer_fingerprint,
        scaling_digests=scaling_digests,
    )
    receipt.chmod(0o600)
    receipt.write_bytes(
        canonical_json(_fixture_receipt_for_validator(expected_invocation_values))
    )
    receipt.chmod(0o400)
    ack_value = {
        "format": "iroha-sumeragi-v2-receipt-validation-ack", "schema_version": 3,
        "profile": "release",
        "sealed_source": {
            "archive_id": "release-retained.source.v1",
            "manifest_sha256": manifest_digest,
        },
        "receipt": {
            "archive_id": "release-terminal.receipt.v1",
            "mode": "0400",
            "sha256": hashlib.sha256(receipt.read_bytes()).hexdigest(),
            "size_bytes": receipt.stat().st_size,
        },
        "validator": {
            "archive_id": "release-bootstrap.receipt-validator.v1",
            "sha256": hashlib.sha256(validator.read_bytes()).hexdigest(),
            "bootstrap_completion_sha256": hashlib.sha256(completion.read_bytes()).hexdigest(),
        },
        "invocation": _fixture_validator_invocation(
            helper,
            invocation=invocation,
            bootstrap=bootstrap,
            source=source,
            receipt=receipt,
            acknowledgment=ack,
            source_manifest_sha256=manifest_digest,
            candidate_root=candidate_root,
            scaling_evidence_manifest=scaling_manifest,
            expected_signer_fingerprint=expected_signer_fingerprint,
            scaling_digests=scaling_digests,
        ),
        "exit_status": 0,
        "stdout": {"sha256": hashlib.sha256(stdout).hexdigest(), "size_bytes": len(stdout)},
        "stderr": {"sha256": hashlib.sha256(b"").hexdigest(), "size_bytes": 0},
    }
    helper._validate_validator_invocation(
        ack_value["invocation"], expected_values=expected_invocation_values
    )
    changed_invocation = json.loads(json.dumps(ack_value["invocation"]))
    previously_unchecked = next(
        binding
        for binding in changed_invocation["ordered_options"]
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
        key: changed_invocation[key]
        for key in (
            "profile", "operation", "python_flags", "validator", "ordered_options"
        )
    }
    changed_invocation["invocation_sha256"] = hashlib.sha256(
        json.dumps(
            changed_core, sort_keys=True, separators=(",", ":")
        ).encode()
    ).hexdigest()
    with pytest.raises(helper.CacheCopyError, match="normalized option value"):
        helper._validate_validator_invocation(
            changed_invocation, expected_values=expected_invocation_values
        )
    ack.write_bytes(canonical_json(ack_value)); ack.chmod(0o400)
    for secret_path in (
        invocation, bootstrap, source, candidate_root, scaling_root, validator,
    ):
        assert str(secret_path).encode() not in ack.read_bytes()
    original_publish = helper._publish_inventory
    failed = False

    def fail_first_protected(path: Path, payload: bytes) -> object:
        nonlocal failed
        if path == bootstrap / "RELEASE_COMPLETED.json" and not failed:
            failed = True
            raise helper.CacheCopyError("injected protected publication failure")
        return original_publish(path, payload)

    helper._publish_inventory = fail_first_protected
    try:
        with pytest.raises(helper.CacheCopyError, match="protected publication"):
            helper.seal_release_result(
                invocation, bootstrap, manifest_digest, candidate_root,
                scaling_manifest, expected_signer_fingerprint,
                scaling_digests["trial_harness"], scaling_digests["configuration"],
                scaling_digests["irohad"], scaling_digests["iroha_cli"],
            )
    finally:
        helper._publish_inventory = original_publish
    assert not (invocation / "retained-evidence-inventory.json").exists()
    assert not any(path.name.startswith(("RELEASE_COMPLETED", "release-runner-result", "release-retained", "receipt-validation-ack", "sealed-identity")) for path in bootstrap.iterdir() if path not in {validator, completion})
    with pytest.raises(helper.CacheCopyError, match="prior cleanup quarantine"):
        helper.seal_release_result(
            invocation, bootstrap, manifest_digest, candidate_root,
            scaling_manifest, expected_signer_fingerprint,
            scaling_digests["trial_harness"], scaling_digests["configuration"],
            scaling_digests["irohad"], scaling_digests["iroha_cli"],
        )
    helper.cleanup_invocation(tmp_path, invocation, "fault-seal-")
    assert not invocation.exists()
    quiescent = private_directory(tmp_path / "fault-quiescent")
    private_directory(quiescent / "nested"); (quiescent / "nested" / "file").write_bytes(b"dispose\n")
    helper.cleanup_invocation(tmp_path, quiescent, "fault-quiescent")
    assert not quiescent.exists() and not any(path.name.startswith(".owned-quiescent.") for path in tmp_path.iterdir())

    failure_invocation = private_directory(tmp_path / "failure-invocation")
    failure_output = private_directory(failure_invocation / "output")
    failure_release = private_directory(failure_output / "release")
    failure_bootstrap = private_directory(tmp_path / "failure-bootstrap")
    failure_receipt = failure_release / "RELEASE_COMPLETED.json"
    failure_receipt.write_bytes(b'{"unverified":true}\n')
    failure_stdout = failure_invocation / "receipt-validator.stdout"
    failure_stderr = failure_invocation / "receipt-validator.stderr"
    failure_stdout.write_bytes(b"x" * (helper.VALIDATOR_DIAGNOSTIC_BYTES + 17))
    failure_stderr.write_bytes(b"validator rejected fixture\n")
    failure_identity = {
        "schema_version": 1,
        "head_commit": "1" * 40,
        "head_tree": "2" * 40,
        "index_tree": "2" * 40,
        "workspace_source_manifest_sha256": "3" * 64,
        "cargo_lock_sha256": "4" * 64,
    }
    for path, data in (
        (failure_bootstrap / "candidate-identity.json", canonical_json(failure_identity)),
        (failure_bootstrap / "BOOTSTRAP_COMPLETED.json", b"{}\n"),
        (failure_bootstrap / "validate-receipt.py", b"# protected validator\n"),
    ):
        path.write_bytes(data); path.chmod(0o400)
    failure_events: list[str] = []
    original_failure_publish = helper._publish_inventory
    original_failure_cleanup = helper.cleanup_invocation
    def record_failure_publish(path: Path, payload: bytes) -> object:
        failure_events.append(f"publish:{path.name}")
        return original_failure_publish(path, payload)
    def record_failure_cleanup(*args: object) -> None:
        failure_events.append("cleanup:start")
        original_failure_cleanup(*args)
        failure_events.append("cleanup:complete")
    helper._publish_inventory = record_failure_publish
    helper.cleanup_invocation = record_failure_cleanup
    try:
        helper.publish_validation_failure(
            failure_invocation, failure_bootstrap, tmp_path, "failure-invocation",
            "5" * 64, 72,
        )
    finally:
        helper._publish_inventory = original_failure_publish
        helper.cleanup_invocation = original_failure_cleanup
    assert failure_events == [
        "cleanup:start", "cleanup:complete",
        "publish:receipt-validator-failure.stdout",
        "publish:receipt-validator-failure.stderr",
        "publish:RECEIPT_VALIDATION_FAILED.json",
    ]
    assert not failure_invocation.exists()
    failure_marker_path = failure_bootstrap / "RECEIPT_VALIDATION_FAILED.json"
    failure_marker = json.loads(failure_marker_path.read_bytes())
    assert failure_marker_path.read_bytes() == canonical_json(failure_marker)
    assert failure_marker["validator"]["exit_status"] == 72
    assert failure_marker["invocation_cleanup"] == "complete"
    stdout_record = failure_marker["diagnostics"]["stdout"]
    stderr_record = failure_marker["diagnostics"]["stderr"]
    assert stdout_record["captured_size_bytes"] == helper.VALIDATOR_DIAGNOSTIC_BYTES
    assert stdout_record["observed_size_bytes"] == helper.VALIDATOR_DIAGNOSTIC_BYTES + 17
    assert stdout_record["truncated"] is True
    assert stderr_record["truncated"] is False
    captured_stdout = failure_bootstrap / stdout_record["name"]
    captured_stderr = failure_bootstrap / stderr_record["name"]
    assert captured_stdout.read_bytes() == b"x" * helper.VALIDATOR_DIAGNOSTIC_BYTES
    assert captured_stderr.read_bytes() == b"validator rejected fixture\n"
    assert stdout_record["sha256"] == hashlib.sha256(captured_stdout.read_bytes()).hexdigest()
    assert stat.S_IMODE(captured_stdout.stat().st_mode) == 0o400
    assert str(failure_invocation).encode() not in failure_marker_path.read_bytes()
    assert not any(
        (failure_bootstrap / name).exists()
        for name in (
            "BOOTSTRAP_RELEASE_COMPLETED.json", "RELEASE_COMPLETED.json",
            "receipt-validation-ack.json", "release-runner-result.json",
        )
    )


@pytest.mark.parametrize("layout", ("nested", "alias"))
def test_prebuilt_release_root_authentication_rejects_nesting_or_alias(
    tmp_path: Path, layout: str
) -> None:
    module = _load_receipt_writer_module()
    source_root = tmp_path / "source"
    source_root.mkdir()
    external_root = tmp_path / "external"
    external_root.mkdir()
    cargo_target_root = external_root / "target"

    if layout == "nested":
        artifact_root = external_root / "artifacts"
        artifact_root.mkdir(mode=0o700)
        artifact_root.chmod(0o700)
        cargo_target_root = artifact_root / "target"
        cargo_target_root.mkdir(mode=0o700)
        cargo_target_root.chmod(0o700)
        expected_error = "release artifact and Cargo target roots overlap"
    else:
        real_artifact_root = external_root / "artifacts-real"
        real_artifact_root.mkdir(mode=0o700)
        real_artifact_root.chmod(0o700)
        artifact_root = external_root / "artifacts-alias"
        try:
            artifact_root.symlink_to(real_artifact_root, target_is_directory=True)
        except OSError as error:
            pytest.skip(f"directory symlinks are unavailable: {error}")
        cargo_target_root.mkdir(mode=0o700)
        cargo_target_root.chmod(0o700)
        expected_error = "one private owner-owned directory outside the sealed source"

    fields = {
        "artifact_root_path": str(artifact_root),
        "cargo_target_root_path": str(cargo_target_root),
    }
    with pytest.raises(module.ReceiptError, match=expected_error):
        module._prebuilt_release_roots(
            repo_root=source_root,
            fields=fields,
            expected_artifact_root=artifact_root,
            expected_cargo_target_root=cargo_target_root,
        )


@pytest.mark.parametrize(
    ("field", "expected_error"),
    (
        (
            "artifact_root_path",
            "corridor artifact root is not the exact authenticated release "
            "artifact root",
        ),
        (
            "cargo_target_root_path",
            "corridor Cargo target root is not the exact authenticated release "
            "Cargo target root",
        ),
    ),
)
def test_receipt_rejects_corridor_root_substitution(
    tmp_path: Path, field: str, expected_error: str
) -> None:
    evidence = make_evidence(tmp_path)
    completion = evidence["corridor_completion"]
    assert isinstance(completion, Path)
    substituted_root = tmp_path / f"substituted-{field}"
    substituted_root.mkdir(mode=0o700)
    substituted_root.chmod(0o700)
    fields = read_tsv_fields(completion)
    fields[field] = str(substituted_root)
    write_tsv(completion, fields)
    writer = fixture_writer(tmp_path)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert expected_error in result.stderr


def exercise_private_pr_outer_lifecycle(runner: Path, tmp_path: Path) -> None:
    """Run the outer PR boundary without entering Cargo or the real gate suite."""

    fixture = tmp_path / "private-pr-outer"
    candidate = fixture / "candidate"
    scripts = candidate / "scripts"
    tools = fixture / "tools"
    git_exec = fixture / "git-exec"
    rust_toolchain = fixture / "rust-toolchain"
    for directory in (scripts, tools, git_exec, rust_toolchain / "bin", rust_toolchain / "lib"):
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        directory.chmod(0o700)

    def executable(path: Path, body: str) -> Path:
        path.write_text(body, encoding="utf-8"); path.chmod(0o700)
        return path

    host_python = Path(sys.executable).resolve(strict=True)
    helper = runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")
    runner_support = runner.with_name("run_sumeragi_v2_release_gates_support.sh")
    shutil.copy2(runner, scripts / runner.name)
    shutil.copy2(helper, scripts / helper.name)
    shutil.copy2(runner_support, scripts / runner_support.name)
    (candidate / "Cargo.lock").write_bytes(b"private PR fixture lock\n")
    (candidate / "Cargo.lock").chmod(0o600)
    candidate_lock = (candidate / "Cargo.lock").read_bytes()
    candidate_helper = (scripts / helper.name).read_bytes()
    candidate_runner_support = (scripts / runner_support.name).read_bytes()
    (fixture / "caller-ancestor-marker").write_text("must remain unreachable\n", encoding="utf-8")

    executable(
        scripts / "compute_workspace_source_manifest.py",
        f"#!{host_python}\n"
        "from pathlib import Path\nimport hashlib,json,sys\n"
        "root=Path(sys.argv[sys.argv.index('--root')+1])\n"
        "value={'head_commit':'a'*40,'head_tree':'b'*40,'cargo_lock_sha256':hashlib.sha256((root/'Cargo.lock').read_bytes()).hexdigest(),'workspace_source_manifest_sha256':'c'*64}\n"
        "print(json.dumps(value,sort_keys=True,separators=(',',':')))\n",
    )
    executable(scripts / "seal_workspace_source.py", f"#!{host_python}\nraise SystemExit(0)\n")
    child_runner = executable(
        fixture / "child-runner.sh",
        "#!/bin/bash\nset -euo pipefail\n"
        '"$IROHA_RELEASE_PR_PYTHON_BIN" -I -S - "$@" <<\'PY\'\n'
        "from pathlib import Path\nimport json,os,sys\n"
        "cwd=Path.cwd()\n"
        "value={'argv':sys.argv[1:],'cwd':str(cwd),'environment':dict(os.environ),'ancestor_marker':any((root/'caller-ancestor-marker').exists() for root in (cwd,*cwd.parents)),'helper_fd_open':Path('/dev/fd/9').exists()}\n"
        "print('FAKE_PR_RESULT='+json.dumps(value,sort_keys=True))\n"
        "PY\n"
        'exit "$(cat fake-exit-status)"\n',
    )
    executable(tools / "python3", f'#!/bin/sh\nexec {json.dumps(str(host_python))} "$@"\n')
    executable(tools / "bash", '#!/bin/sh\nexec /bin/bash "$@"\n')
    executable(tools / "cat", '#!/bin/sh\nexec /bin/cat "$@"\n')
    executable(tools / "env", '#!/bin/sh\nexec /usr/bin/env "$@"\n')
    executable(tools / "cp", '#!/bin/sh\nif [ "${FAKE_PR_STAGE_COPY_FAIL:-0}" = 1 ]; then printf "%s\\n" "${3%/*}" >"$FAKE_PR_STAGE_FAILURE_LOG"; exit 89; fi\nexec /bin/cp "$@"\n')
    cargo = executable(rust_toolchain / "bin" / "cargo", "#!/bin/sh\nprintf '%s\\n' 'cargo fixture'\n")
    rustc = executable(rust_toolchain / "bin" / "rustc", "#!/bin/sh\nprintf '%s\\n' 'rustc fixture'\n")
    (rust_toolchain / "lib" / "support").write_bytes(b"toolchain support\n")
    executable(
        tools / "rustup",
        f"#!/bin/sh\ncase \"$4\" in cargo) printf '%s\\n' {json.dumps(str(cargo))};; rustc) printf '%s\\n' {json.dumps(str(rustc))};; *) exit 1;; esac\n",
    )
    for name in ("git-upload-pack", "git-index-pack"):
        executable(git_exec / name, "#!/bin/sh\nexit 0\n")
    fake_git = executable(
        tools / "git",
        f"#!{host_python}\n"
        "from pathlib import Path\nimport shutil,sys\n"
        f"exec_path=Path({str(git_exec)!r}); child=Path({str(child_runner)!r})\n"
        "args=sys.argv[1:]\n"
        "if args==['--exec-path']: print(exec_path)\n"
        "elif 'clone' in args:\n"
        " source,destination=Path(args[-2]),Path(args[-1]); shutil.copytree(source,destination); shutil.copy2(child,destination/'scripts'/'run_sumeragi_v2_release_gates.sh'); (destination/'scripts'/'run_sumeragi_v2_release_gates.sh').chmod(0o700)\n"
        "elif 'rev-parse' in args: print('b'*40 if args[-1].endswith('^{tree}') else 'a'*40)\n"
        "elif 'status' in args or 'checkout' in args or 'remote' in args: pass\n"
        "else: raise SystemExit('unexpected fake Git argv: '+repr(args))\n",
    )
    assert fake_git.is_file()
    caller_cache = fixture / "caller-cargo-home"
    home = fixture / "home"
    home.mkdir(mode=0o700)
    for expected_status in (0, 23):
        (candidate / "fake-exit-status").write_text(str(expected_status), encoding="utf-8")
        environment = {
            "PATH": f"{tools}:/usr/bin:/bin",
            "HOME": str(home),
            "CARGO_HOME": str(caller_cache),
            "CALLER_PATH_CANARY": str(candidate),
            "LANG": "C", "LC_ALL": "C", "PYTHONDONTWRITEBYTECODE": "1",
        }
        result = subprocess.run(
            ["/bin/bash", str(scripts / runner.name), "--pr"], cwd=candidate,
            env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, check=False,
        )
        assert result.returncode == expected_status, result.stderr
        child = json.loads(next(line.removeprefix("FAKE_PR_RESULT=") for line in result.stdout.splitlines() if line.startswith("FAKE_PR_RESULT=")))
        child_text = json.dumps(child, sort_keys=True)
        assert str(candidate) not in child_text and str(caller_cache) not in child_text
        assert child["argv"] == ["--pr"] and not child["ancestor_marker"]
        assert not child["helper_fd_open"]
        invocation = Path(child["environment"]["IROHA_RELEASE_INVOCATION_ROOT"])
        assert child["cwd"] == str(invocation / "source") and not invocation.exists()
        assert child["environment"]["CARGO_HOME"] == str(invocation / "host" / "cargo-home")
        assert child["environment"]["RUSTUP_HOME"] == str(invocation / "host" / "rustup-home")
        assert child["environment"]["PATH"].startswith(str(invocation / "runtime" / "bin"))
        assert (candidate / "Cargo.lock").read_bytes() == candidate_lock
        assert (scripts / helper.name).read_bytes() == candidate_helper
        assert (scripts / runner_support.name).read_bytes() == candidate_runner_support
    stage_failure_log = fixture / "stage-failure-root"
    stage_failure_environment = dict(environment, FAKE_PR_STAGE_COPY_FAIL="1", FAKE_PR_STAGE_FAILURE_LOG=str(stage_failure_log))
    stage_failure = subprocess.run(
        ["/bin/bash", str(scripts / runner.name), "--pr"], cwd=candidate,
        env=stage_failure_environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    )
    assert stage_failure.returncode == 89 and not Path(stage_failure_log.read_text(encoding="utf-8").strip()).exists()


def test_release_runner_recovers_authenticated_tool_sources(tmp_path: Path) -> None:
    root = tmp_path.resolve()
    source_root = root / "sources"
    archive_root = root / "evidence" / "runner-tools"
    runner_bin = root / "evidence" / "runner-bin"
    source_root.mkdir(parents=True, mode=0o700)
    archive_root.mkdir(parents=True, mode=0o700)
    runner_bin.mkdir(mode=0o700)
    sources: dict[str, Path] = {}
    archived_tools: dict[str, dict[str, object]] = {}
    source_tools: dict[str, dict[str, str]] = {}
    for name in REQUIRED_RUNNER_TOOL_NAMES:
        payload = f"#!/bin/sh\n# authenticated {name}\n".encode()
        source = source_root / name
        source.write_bytes(payload)
        source.chmod(0o500)
        archive = archive_root / name
        archive.write_bytes(payload)
        archive.chmod(0o500)
        (runner_bin / name).symlink_to(f"../runner-tools/{name}")
        digest = hashlib.sha256(payload).hexdigest()
        sources[name] = source
        source_tools[name] = {"path": str(source), "sha256": digest}
        archived_tools[name] = {
            "archive_id": f"release-runner-tool.{name}.v1",
            "alias_name": name,
            "archive_name": f"runner-tools/{name}",
            "mode": "0500",
            "sha256": digest,
            "size_bytes": len(payload),
        }
    manifest = root / "evidence" / "runner-tool-manifest.json"
    manifest.write_bytes(canonical_json({"schema_version": 1, "tools": source_tools}))
    manifest.chmod(0o400)
    marker = root / "evidence" / "BOOTSTRAP_COMPLETED.json"
    marker.write_bytes(canonical_json({
        "runner": {"tools": archived_tools},
        "trusted_inputs": {"runner_tool_manifest": {
            "archive_name": manifest.name,
            "mode": "0400",
            "sha256": sha256(manifest),
            "size_bytes": manifest.stat().st_size,
        }},
    }))
    invocation = [
        "/bin/bash", "-c",
        'source "$1"; authenticated_runner_tool_sources "$2" "$3" "$4" "$5" "$6"',
        "runner-support-fixture",
        str(ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates_support.sh"),
        sys.executable, str(marker), sha256(marker), str(runner_bin), str(manifest),
    ]
    recovered = subprocess.run(
        invocation, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    )
    assert recovered.returncode == 0, recovered.stderr
    assert dict(line.split("\t", 1) for line in recovered.stdout.splitlines()) == {
        name: str(source) for name, source in sources.items()
    }
    changed = sources["cargo"]
    changed.chmod(0o700)
    changed.write_bytes(b"#!/bin/sh\n# changed cargo\n")
    changed.chmod(0o500)
    rejected = subprocess.run(
        invocation, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    )
    assert rejected.returncode != 0
    assert "authenticated runner tool source cargo changed" in rejected.stderr


def test_receipt_rejects_external_cargo_home_configuration(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    runner = ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    runner_source = runner.read_text(encoding="utf-8")
    for token, count in (
        ('if [[ "$profile" == "--pr" && "${IROHA_RELEASE_PRIVATE_PR:-0}" != 1 ]]', 1),
        ("--no-local --no-hardlinks --no-checkout", 2),
        ('pr_clone_helper="$pr_source_root/scripts/copy_sumeragi_v2_release_cargo_cache.py"', 1),
        ('"$release_runtime_helper"', 13),
        ('"$release_child_bin/python3" -I -S "$release_tool_probe_helper"', 2),
        ('--probe-root "$release_invocation_root/.runtime-tool-probe"', 1),
        ('--probe-root "$release_invocation_root/.runtime-tool-probe-post"', 1),
        ("--runtime-tool-probe-manifest", 1),
        ("--runtime-tool-probe-result", 1),
        ('"$repo_root/scripts/copy_sumeragi_v2_release_cargo_cache.py"', 3),
        ('IROHA_RELEASE_PRIVATE_PR=1 \\', 1),
        ('HOME="$pr_host_root/home"', 1),
        ('TMPDIR="$pr_host_root/tmp"', 1),
        ('XDG_CACHE_HOME="$pr_host_root/cache"', 1),
        ('unset OLDPWD', 1),
        ('/usr/bin/env -i \\', 0),
        ('"$pr_bin/env" -i \\', 1),
        ('"$release_child_bin/env" -i \\', 1),
        ('GIT_EXEC_PATH="$pr_bin"', 1),
        ('GIT_EXEC_PATH="$release_git_exec_path"', 1),
        ('IROHA_RELEASE_CHILD_RESULT_PATH="$release_child_result_path"', 1),
        ('release_runner_tool_source_tsv="$(authenticated_runner_tool_sources \\', 1),
        ('"$(release_runner_tool_source cargo)"', 1),
        ('return (runtime / "swift-toolchain" / "bin" / name).resolve(strict=True)', 1),
        ('IROHA_RELEASE_TLAPM_BIN="$release_child_runtime/tlapm-distribution/bin/tlapm"', 1),
        ('IROHA_RELEASE_APALACHE_BIN="$release_child_runtime/apalache-distribution/bin/apalache-mc"', 1),
        ('IROHA_RELEASE_APALACHE_BIN="$release_child_bin/apalache-mc"', 0),
        ('"$sealed_repo_root/scripts/write_sumeragi_v2_release_receipt.py"', 1),
        ("--profile --release --checkpoint sealed", 0),
    ):
        assert runner_source.count(token) == count
    assert "--checkpoint entry" not in runner_source
    assert "bootstrap-source-binding" not in runner_source
    assert 'IROHA_RELEASE_CANDIDATE_IDENTITY_PATH="$candidate_identity_path"' not in runner_source
    assert 'IROHA_RELEASE_AGGREGATE_RECEIPT_PATH="$aggregate_receipt_path"' not in runner_source
    assert "GIT_TRACE_*" in runner_source
    assert 'HOME="$release_host_root/home" \\' in runner_source
    assert 'TMP="$release_host_root/tmp" \\' in runner_source
    assert 'TEMP="$release_host_root/tmp" \\' in runner_source
    assert runner_source.index('exit "$sealed_status"\nfi') < runner_source.index(
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"'
    )
    assert 'readonly release_runtime_inventory="$release_invocation_root/runtime-input.json"' in runner_source
    assert '--cleanup-base "$pr_temp_base" --invocation-root "$pr_invocation_root"' in runner_source
    assert 'RUSTUP_HOME="$pr_host_root/rustup-home"' in runner_source
    assert 'export PATH="${IROHA_RELEASE_INVOCATION_ROOT}/runtime/bin"' in runner_source
    assert '--verify-cache-sources' in runner_source
    assert 'os.symlink(str(Path(source).resolve(strict=True))' not in runner_source
    assert runner_source.count('/dev/fd/9') == 1 and 'run_pr_helper() {' in runner_source
    assert 'exec 9<' in runner_source and 'exec 9<&-' in runner_source and 'exec {' not in runner_source
    assert runner_source.index('trap - EXIT\n    unset OLDPWD') < runner_source.index('exec 9<&-')
    assert runner_source.index('exec 9<&-') < runner_source.index('"$pr_bin/env" -i')
    assert runner_source.index('readonly release_child_result_path=') < runner_source.index('"$release_child_bin/env" -i') < runner_source.index(
        '"$release_child_bin/bash" "$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release'
    )
    assert '"$pr_python_bin" -I -S "$pr_cleanup_helper"' not in runner_source
    assert runner_source.index('pr_runtime_inventory="$pr_invocation_root/runtime-input.json"') < runner_source.index(
        '"$pr_bin/bash" "$pr_source_root/scripts/run_sumeragi_v2_release_gates.sh" --pr'
    )
    assert runner_source.index('readonly release_runtime_inventory="$release_invocation_root/runtime-input.json"') < runner_source.index(
        '"$release_child_bin/bash" "$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release'
    )
    assert runner_source.index('readonly release_scaling_inventory_sha256=') < runner_source.index(
        '"$release_child_bin/bash" "$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release'
    ) < runner_source.index('"$(sha256_file "$release_scaling_inventory")" == "$release_scaling_inventory_sha256"')
    private_fixture = tmp_path / "private-runtime-fixture"
    private_fixture.mkdir(mode=0o700)
    private_fixture.chmod(0o700)
    runtime_sources = private_fixture / "sources"
    runtime_sources.mkdir(mode=0o700)
    loose_sources = private_fixture / "loose-sources"
    loose_sources.mkdir(mode=0o700)

    def executable(path: Path, output: str) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        path.write_text(f"#!/bin/sh\nprintf '%s\\n' '{output}'\n", encoding="utf-8")
        path.chmod(0o700)
        return path

    toolchain = runtime_sources / "rust-toolchain"
    cargo = executable(toolchain / "bin" / "cargo", "cargo fixture 1")
    rustc = executable(toolchain / "bin" / "rustc", "rustc fixture 1")
    (toolchain / "lib").mkdir(mode=0o700)
    rust_support = toolchain / "lib" / "support.bin"
    rust_support.write_bytes(b"rust support\n")
    rust_support.chmod(0o600)
    swift_bin = runtime_sources / "swift-toolchain" / "bin"
    swift = executable(swift_bin / "swift-frontend", "swift fixture 1")
    (swift_bin / "swift").symlink_to(swift.name)
    (swift_bin / "swiftc").symlink_to(swift.name)
    (swift.parent.parent / "lib").mkdir(mode=0o700)
    (swift.parent.parent / "lib" / "support.bin").write_bytes(b"swift support\n")
    java = executable(runtime_sources / "java-runtime" / "bin" / "java", "java fixture 1")
    (java.parent.parent / "lib").mkdir(mode=0o700)
    (java.parent.parent / "lib" / "support.bin").write_bytes(b"java support\n")
    verus_root = runtime_sources / "verus-distribution"
    verus = executable(verus_root / "verus", "verus fixture 1")
    cargo_verus = executable(verus_root / "cargo-verus", "cargo-verus fixture 1")
    (verus_root / "support.bin").write_bytes(b"verus support\n")
    release_shell_utilities = (
        "awk", "basename", "cat", "chmod", "cmp", "cp", "cut", "diff",
        "dirname", "env", "find", "grep", "ln", "ls", "mkdir", "mkfifo",
        "mktemp", "mv", "openssl", "rm", "rmdir", "sed", "sh", "sleep",
        "tail", "tee", "tr", "uname", "wc", "xargs",
        "shasum" if sys.platform == "darwin" else "sha256sum",
    )
    ordinary = {
        name: executable(loose_sources / name / name, f"{name} fixture 1")
        for name in (
            "python3", "git", "ssh-keygen", "bash", "node",
            "git-upload-pack", "git-index-pack", *release_shell_utilities,
        )
    }
    tlapm_root = runtime_sources / "tlapm-distribution"
    tlapm = executable(tlapm_root / "bin" / "tlapm", "tlapm fixture 1")
    tlapm_stdlib = tlapm_root / "lib" / "tlapm" / "stdlib"
    tlapm_stdlib.mkdir(parents=True, mode=0o700)
    (tlapm_stdlib / "Fixture.tla").write_bytes(b"---- MODULE Fixture ----\n")
    tlapm_backend = executable(
        tlapm_root / "lib" / "tlapm" / "backends" / "bin" / "z3",
        "z3 fixture 1",
    )
    apalache_root = runtime_sources / "apalache-distribution"
    apalache = executable(
        apalache_root / "bin" / "apalache-mc", "apalache fixture 0.52.2"
    )
    apalache_jar = apalache_root / "lib" / "apalache.jar"
    apalache_jar.parent.mkdir(parents=True, mode=0o700)
    apalache_jar.write_bytes(b"apalache jar fixture\n")
    framework_name = sysconfig.get_config_var("PYTHONFRAMEWORK")
    framework_python = (
        sys.platform == "darwin"
        and isinstance(framework_name, str)
        and bool(framework_name)
    )
    fixture_python = ordinary["python3"]
    if framework_python:
        ordinary["python3"] = Path(sys.executable).resolve(strict=True)
    runtime_helper = loose_sources / "runtime-helper" / "copy-release-runtime.py"
    runtime_helper.parent.mkdir(mode=0o700)
    runtime_helper.write_bytes(b"# protected runtime helper fixture\n")
    runtime_helper.chmod(0o600)
    tla2tools = runtime_sources / "tla2tools.jar"
    tla2tools.write_bytes(b"jar fixture\n")
    tla2tools.chmod(0o600)
    sources = [
        ordinary["python3"], ordinary["git"], ordinary["ssh-keygen"], ordinary["bash"],
        runtime_helper,
        cargo, rustc, ordinary["node"], swift, tlapm, apalache, java,
        verus, cargo_verus, tla2tools,
        ordinary["git-upload-pack"], ordinary["git-index-pack"],
        *(ordinary[name] for name in release_shell_utilities),
    ]
    runtime = tmp_path / "runtime"
    runtime_inventory = tmp_path / "runtime-input.json"
    runtime_arguments = sum((["--runtime-source", str(path)] for path in sources), [])
    copy_result = subprocess.run(
        [sys.executable, "-I", "-S", str(runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")),
         "--copy-runtime", "--runtime-root", str(runtime), "--runtime-inventory", str(runtime_inventory),
         *runtime_arguments],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False,
    )
    assert copy_result.returncode == 0, copy_result.stderr
    assert (runtime / "tlapm-distribution/lib/tlapm/stdlib/Fixture.tla").is_file()
    assert (runtime / "tlapm-distribution/lib/tlapm/backends/bin/z3").read_bytes() \
        == tlapm_backend.read_bytes()
    assert (runtime / "apalache-distribution/lib/apalache.jar").read_bytes() \
        == apalache_jar.read_bytes()
    assert (runtime / "swift-toolchain/bin/swift").is_symlink()
    assert (runtime / "swift-toolchain/bin/swift").resolve() \
        == runtime / "swift-toolchain/bin/swift-frontend"
    assert (runtime / "swift-toolchain/bin/swiftc").resolve() \
        == runtime / "swift-toolchain/bin/swift-frontend"
    assert (runtime / "bin/tlapm").resolve() == runtime / "tlapm-distribution/bin/tlapm"
    assert (runtime / "bin/apalache-mc").resolve() \
        == runtime / "apalache-distribution/bin/apalache-mc"
    runtime_document = json.loads(runtime_inventory.read_bytes())
    assert runtime_document["source_disclosure"] == "withheld"
    assert runtime_document["input_record_count"] == len(runtime_document["input_records"])
    if framework_python:
        stdlib_name = f"python{sys.version_info.major}.{sys.version_info.minor}"
        runtime_records = {
            record["path"]: record for record in runtime_document["records"]
        }
        assert len(runtime_records) == len(runtime_document["records"])
        for path, kind in {
            "bin/python3": "file",
            framework_name: "file",
            "Resources": "directory",
            "Resources/Python.app/Contents/MacOS/Python": "file",
            "lib": "directory",
            f"lib/{stdlib_name}": "directory",
            f"lib/{stdlib_name}/lib-dynload": "directory",
        }.items():
            assert runtime_records[path]["kind"] == kind

        observed_paths: set[str] = set()

        def observe_runtime_members(directory: Path, relative: str = "") -> None:
            with os.scandir(directory) as entries:
                for entry in entries:
                    path = entry.name if not relative else f"{relative}/{entry.name}"
                    observed_paths.add(path)
                    if entry.is_dir(follow_symlinks=False):
                        observe_runtime_members(Path(entry.path), path)

        observe_runtime_members(runtime)
        assert observed_paths == set(runtime_records)
        source_root = ordinary["python3"].parent.parent
        source_records: dict[str, tuple[object, ...]] = {
            str(framework_name): (
                "file",
                (source_root / str(framework_name)).stat().st_size,
                hashlib.sha256(
                    (source_root / str(framework_name)).read_bytes()
                ).hexdigest(),
            )
        }

        def observe_source_members(directory: Path, relative: str) -> None:
            source_records[relative] = ("directory",)
            with os.scandir(directory) as entries:
                for entry in entries:
                    path = f"{relative}/{entry.name}"
                    metadata = entry.stat(follow_symlinks=False)
                    if entry.is_symlink():
                        source_records[path] = ("symlink", os.readlink(entry.path))
                    elif entry.is_dir(follow_symlinks=False):
                        observe_source_members(Path(entry.path), path)
                    elif entry.is_file(follow_symlinks=False):
                        source_records[path] = (
                            "file",
                            metadata.st_size,
                            hashlib.sha256(Path(entry.path).read_bytes()).hexdigest(),
                        )
                    else:
                        raise AssertionError(
                            f"framework source has special member: {entry.path}"
                        )

        observe_source_members(source_root / "Resources", "Resources")
        observe_source_members(source_root / "lib", "lib")
        for path, expected in source_records.items():
            actual = runtime_records[path]
            assert actual["kind"] == expected[0]
            if expected[0] == "file":
                assert (actual["size"], actual["sha256"]) == expected[1:]
            elif expected[0] == "symlink":
                assert actual["target"] == expected[1]
        archived_python = runtime / "bin" / "python3"
        python_probe = subprocess.run(
            [
                str(archived_python),
                "-I",
                "-S",
                "-c",
                "import sys;sys.stdout.write(sys.executable+'\\n')",
            ],
            cwd=runtime,
            env={
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": str(runtime / "bin"),
            },
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        assert python_probe.returncode == 0
        assert python_probe.stdout == f"{archived_python}\n"
        assert python_probe.stderr == ""
    for name in ("cargo", "rustc"):
        probe = subprocess.run(
            [str(runtime / "bin" / name), "--version"],
            env={"PATH": str(runtime / "bin")}, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, check=False,
        )
        assert probe.returncode == 0 and probe.stdout == f"{name} fixture 1\n"
    verify_command = [
        sys.executable, "-I", "-S", str(runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")),
        "--verify-runtime-sources", "--runtime-root", str(runtime),
        "--runtime-inventory", str(runtime_inventory),
        *runtime_arguments,
    ]
    verified = subprocess.run(
        verify_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    )
    assert verified.returncode == 0, verified.stderr
    pr_runtime = tmp_path / "pr-runtime"
    pr_inventory = tmp_path / "pr-runtime-input.json"
    pr_sources = [
        ordinary["python3"], ordinary["git"], ordinary["bash"], cargo, rustc,
        ordinary["git-upload-pack"], ordinary["git-index-pack"],
        *(ordinary[name] for name in release_shell_utilities),
    ]
    pr_arguments = sum((["--runtime-source", str(path)] for path in pr_sources), [])
    copied_pr_runtime = subprocess.run(
        [sys.executable, "-I", "-S", str(runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")),
         "--copy-runtime", "--runtime-root", str(pr_runtime),
         "--runtime-inventory", str(pr_inventory), *pr_arguments],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False,
    )
    assert copied_pr_runtime.returncode == 0, copied_pr_runtime.stderr
    pr_document = json.loads(pr_inventory.read_bytes())
    assert pr_document["source_disclosure"] == "withheld"
    assert all(str(source) not in pr_inventory.read_text(encoding="utf-8") for source in pr_sources)
    verified_pr_runtime = subprocess.run(
        [sys.executable, "-I", "-S", str(runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")),
         "--verify-runtime-sources", "--runtime-root", str(pr_runtime),
         "--runtime-inventory", str(pr_inventory), *pr_arguments],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False,
    )
    assert verified_pr_runtime.returncode == 0, verified_pr_runtime.stderr
    original_runtime_inventory = runtime_inventory.read_bytes()
    for mutation in ("extra", "count-bool", "bytes-string", "ghost", "destination"):
        changed = json.loads(original_runtime_inventory)
        if mutation == "extra":
            changed["unexpected"] = "field"
        elif mutation == "count-bool":
            changed["input_record_count"] = True
        elif mutation == "bytes-string":
            changed["input_file_bytes"] = str(changed["input_file_bytes"])
        elif mutation == "ghost":
            ghost = dict(changed["input_records"][0])
            ghost["path"] = "ghost-runtime-source"
            changed["input_records"].append(ghost)
            changed["input_record_count"] += 1
        else:
            changed["input_records"][0]["destination_inode"] = 0
        runtime_inventory.chmod(0o600)
        runtime_inventory.write_bytes(canonical_json(changed))
        runtime_inventory.chmod(0o400)
        rejected_inventory = subprocess.run(
            verify_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, check=False,
        )
        assert rejected_inventory.returncode == 1, mutation
    runtime_inventory.chmod(0o600)
    runtime_inventory.write_bytes(original_runtime_inventory)
    runtime_inventory.chmod(0o400)
    private_support = runtime / "rust-toolchain" / "lib" / "support.bin"
    private_support.chmod(0o600)
    private_support.write_bytes(b"mutated private runtime support\n")
    rejected = subprocess.run(
        verify_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    )
    assert rejected.returncode == 1
    assert "private runtime changed after publication" in rejected.stderr
    helper_path = runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")
    helper_spec = importlib.util.spec_from_file_location("release_cache_helper_faults", helper_path)
    assert helper_spec is not None and helper_spec.loader is not None
    helper_module = importlib.util.module_from_spec(helper_spec)
    helper_spec.loader.exec_module(helper_module)
    if framework_python:
        source_framework_root = ordinary["python3"].parent.parent
        with pytest.raises(
            helper_module.CacheCopyError,
            match="must be disjoint",
        ):
            helper_module._reject_framework_python_destination_overlap(
                source_framework_root,
                source_framework_root / "private-runtime-overlap",
                tmp_path / "disjoint-inventory.json",
            )
        with pytest.raises(
            helper_module.CacheCopyError,
            match="must be disjoint",
        ):
            helper_module._reject_framework_python_destination_overlap(
                source_framework_root,
                tmp_path / "disjoint-runtime",
                source_framework_root / "private-runtime-inventory.json",
            )

        unsafe_framework = tmp_path / "unsafe-framework"
        (unsafe_framework / "Resources").mkdir(parents=True, mode=0o700)
        (unsafe_framework / "lib").mkdir(mode=0o700)
        unsafe_library = unsafe_framework / str(framework_name)
        unsafe_library.write_bytes(b"framework\n")
        unsafe_library.chmod(0o622)
        with pytest.raises(
            helper_module.CacheCopyError,
            match="framework member is unsafe",
        ):
            helper_module._validate_framework_python_sources(
                unsafe_framework, str(framework_name),
            )
        unsafe_library.chmod(0o600)
        outside_framework = tmp_path / "outside-framework-member"
        outside_framework.write_bytes(b"outside\n")
        (unsafe_framework / "lib" / "escape").symlink_to(
            "../../outside-framework-member"
        )
        with pytest.raises(
            helper_module.CacheCopyError,
            match="symlink escapes",
        ):
            helper_module._validate_framework_python_sources(
                unsafe_framework, str(framework_name),
            )
        (unsafe_framework / "lib" / "escape").unlink()
        original_euid = helper_module.os.geteuid
        helper_module.os.geteuid = lambda: original_euid() + 1
        try:
            with pytest.raises(
                helper_module.CacheCopyError,
                match="framework member is unsafe",
            ):
                helper_module._validate_framework_python_sources(
                    unsafe_framework, str(framework_name),
                )
        finally:
            helper_module.os.geteuid = original_euid
    cleanup_base = tmp_path / "cleanup-base"
    cleanup_base.mkdir(mode=0o700)
    helper_module.cleanup_invocation(cleanup_base, cleanup_base / "absent", "absent")
    helper_module.cleanup_invocation(cleanup_base, cleanup_base / "absent", "absent")
    fail_atomicity_sources = [fixture_python, *sources[1:]]
    exercise_release_helper_fail_atomicity(
        helper_module, tmp_path, fail_atomicity_sources,
    )
    fake_git_root = tmp_path / "fake-git-closure"
    ambient_git = fake_git_root / "ambient"
    closed_git = fake_git_root / "closed"
    ambient_git.mkdir(parents=True)
    closed_git.mkdir()
    closure_log = fake_git_root / "closed.log"
    ambient_log = fake_git_root / "ambient.log"
    for directory, log in ((ambient_git, ambient_log), (closed_git, closure_log)):
        for name in ("git-upload-pack", "git-index-pack"):
            tool = directory / name
            tool.write_text(f'#!/bin/sh\nprintf "%s\\n" "{name}" >> "{log}"\n', encoding="utf-8")
            tool.chmod(0o700)
    frontend = fake_git_root / "git"
    frontend.write_text(
        '#!/bin/sh\n"$GIT_EXEC_PATH/git-upload-pack"\n"$GIT_EXEC_PATH/git-index-pack"\n',
        encoding="utf-8",
    )
    frontend.chmod(0o700)
    closure = subprocess.run(
        [str(frontend)], env={"PATH": str(ambient_git), "GIT_EXEC_PATH": str(closed_git)},
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False,
    )
    assert closure.returncode == 0, closure.stderr
    assert closure_log.read_text(encoding="utf-8").splitlines() == ["git-upload-pack", "git-index-pack"]
    assert not ambient_log.exists()
    source_home = tmp_path / "caller-cargo-home"
    registry_cache = source_home / "registry" / "cache"
    registry_cache.mkdir(parents=True, mode=0o700)
    source_home.chmod(0o700)
    registry_cache.parent.chmod(0o700)
    registry_cache.chmod(0o700)
    crate = registry_cache / "fixture.crate"
    crate.write_bytes(b"caller registry bytes\n")
    crate.chmod(0o600)
    (registry_cache / "fixture-copy").symlink_to(crate.name)
    git_db = source_home / "git" / "db"
    git_db.mkdir(parents=True, mode=0o700)
    git_db.parent.chmod(0o700)
    git_db.chmod(0o700)
    git_head = git_db / "HEAD"
    git_head.write_bytes(b"ref: refs/heads/main\n")
    git_head.chmod(0o600)

    def caller_snapshot() -> dict[str, tuple[object, ...]]:
        snapshot = {}
        for path in (source_home, *sorted(source_home.rglob("*"))):
            metadata = path.lstat()
            if stat.S_ISREG(metadata.st_mode):
                payload: object = path.read_bytes()
            elif stat.S_ISLNK(metadata.st_mode):
                payload = os.readlink(path)
            else:
                payload = None
            # Access time is intentionally excluded: a read can update it under
            # host filesystem policy and there is no portable safe O_NOATIME.
            snapshot[str(path.relative_to(source_home))] = (
                metadata.st_dev,
                metadata.st_ino,
                metadata.st_mode,
                metadata.st_uid,
                metadata.st_gid,
                metadata.st_nlink,
                metadata.st_size,
                metadata.st_mtime_ns,
                metadata.st_ctime_ns,
                payload,
            )
        return snapshot

    before = caller_snapshot()
    artifact_root = tmp_path / "output"
    artifact_root.mkdir(mode=0o700)
    artifact_root.chmod(0o700)
    for runtime_name in ("home", "tmp", "cache"):
        (artifact_root / runtime_name).mkdir(mode=0o700)
        (artifact_root / runtime_name).chmod(0o700)
    cargo_home = artifact_root / "cargo-home"
    cargo_home.mkdir(mode=0o700)
    cargo_home.chmod(0o700)
    inventory = artifact_root / "cargo-cache-input.json"
    copied = run_fixture_cargo_cache_copy(
        runner, source_home, cargo_home, inventory
    )
    assert copied.returncode == 0, copied.stderr
    assert caller_snapshot() == before
    assert (cargo_home / "registry/cache/fixture.crate").read_bytes() == crate.read_bytes()
    assert os.readlink(cargo_home / "registry/cache/fixture-copy") == crate.name
    for relative in ("registry/cache/fixture.crate", "git/db/HEAD"):
        source_stat = (source_home / relative).stat()
        copied_stat = (cargo_home / relative).stat()
        assert (source_stat.st_dev, source_stat.st_ino) != (
            copied_stat.st_dev,
            copied_stat.st_ino,
        )
        assert copied_stat.st_nlink == 1
    inventory_value = json.loads(inventory.read_text(encoding="utf-8"))
    assert inventory.read_bytes() == canonical_json(inventory_value)
    assert inventory_value["source_read_semantics"] == (
        "read-only; host filesystem may update access time"
    )
    verify_cache_command = [
        sys.executable, "-I", "-S", str(runner.with_name("copy_sumeragi_v2_release_cargo_cache.py")),
        "--verify-cache-sources", "--source-cargo-home", str(source_home),
        "--cargo-home", str(cargo_home),
        "--inventory", str(inventory),
    ]
    source_verified = subprocess.run(
        verify_cache_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    )
    assert source_verified.returncode == 0, source_verified.stderr
    crate.write_bytes(b"mutated caller cache\n")
    source_rejected = subprocess.run(
        verify_cache_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    )
    assert source_rejected.returncode == 1
    crate.write_bytes(b"caller registry bytes\n")
    original_cache_inventory = inventory.read_bytes()
    for field, value in (("cargo_home_path", False), ("roots", [[]])):
        changed_inventory = json.loads(original_cache_inventory)
        changed_inventory[field] = value
        inventory.chmod(0o600)
        inventory.write_bytes(canonical_json(changed_inventory))
        inventory.chmod(0o400)
        malformed_source = subprocess.run(
            verify_cache_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, check=False,
        )
        assert malformed_source.returncode == 1 and "Traceback" not in malformed_source.stderr
    changed_inventory = json.loads(original_cache_inventory)
    next(record for record in changed_inventory["records"] if record["kind"] == "file")["destination_inode"] = 0
    inventory.chmod(0o600)
    inventory.write_bytes(canonical_json(changed_inventory))
    inventory.chmod(0o400)
    assert subprocess.run(
        verify_cache_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, check=False,
    ).returncode == 1
    inventory.chmod(0o600)
    inventory.write_bytes(original_cache_inventory)
    inventory.chmod(0o400)

    generated = cargo_home / ".package-cache"
    generated.write_bytes(b"Cargo-created state\n")
    generated.chmod(0o600)
    final_inventory = artifact_root / "cargo-cache-final.json"
    finalized = run_fixture_cargo_cache_copy(
        runner, None, cargo_home, final_inventory
    )
    assert finalized.returncode == 0, finalized.stderr

    module = _load_receipt_writer_module()
    fields = {
        "cargo_home_path": str(cargo_home.resolve()),
        "runtime_home_path": str((artifact_root / "home").resolve()),
        "runtime_tmpdir_path": str((artifact_root / "tmp").resolve()),
        "runtime_tmp_path": str((artifact_root / "tmp").resolve()),
        "runtime_temp_path": str((artifact_root / "tmp").resolve()),
        "runtime_cache_path": str((artifact_root / "cache").resolve()),
        "cargo_cache_input_inventory_path": str(inventory.resolve()),
        "cargo_cache_input_inventory_sha256": sha256(inventory),
        "cargo_cache_final_inventory_path": str(final_inventory.resolve()),
        "cargo_cache_final_inventory_sha256": sha256(final_inventory),
        "runtime_inventory_path": str(runtime_inventory.resolve()),
        "runtime_inventory_sha256": sha256(runtime_inventory),
    }
    accepted = module._validate_cargo_cache_input(
        fields, artifact_root=artifact_root.resolve()
    )
    assert accepted["input_record_count"] == len(inventory_value["records"])

    final_value = json.loads(final_inventory.read_text(encoding="utf-8"))
    typed_record = next(
        record for record in final_value["records"] if record["kind"] == "file"
    )
    original_size = typed_record["size"]
    for numeric_alias in (True, float(original_size)):
        typed_record["size"] = numeric_alias
        final_inventory.chmod(0o600)
        final_inventory.write_bytes(canonical_json(final_value))
        final_inventory.chmod(0o400)
        fields["cargo_cache_final_inventory_sha256"] = sha256(final_inventory)
        with pytest.raises(module.ReceiptError, match="nonnegative integer"):
            module._validate_cargo_cache_input(
                fields, artifact_root=artifact_root.resolve()
            )
    typed_record["size"] = original_size
    final_inventory.chmod(0o600)
    final_inventory.write_bytes(canonical_json(final_value))
    final_inventory.chmod(0o400)
    fields["cargo_cache_final_inventory_sha256"] = sha256(final_inventory)

    original_cache_open = module._cargo_cache_open_regular
    swapped_registry = cargo_home / "registry-original"
    replacement_registry = cargo_home / "registry"
    did_swap = False

    def swap_cache_ancestor(*args: object, **kwargs: object) -> object:
        nonlocal did_swap
        if not did_swap:
            did_swap = True
            replacement_registry.rename(swapped_registry)
            replacement_registry.mkdir(mode=0o700)
        return original_cache_open(*args, **kwargs)

    module._cargo_cache_open_regular = swap_cache_ancestor
    try:
        with pytest.raises(module.ReceiptError, match="changed during traversal"):
            module._validate_cargo_cache_input(
                fields, artifact_root=artifact_root.resolve()
            )
    finally:
        module._cargo_cache_open_regular = original_cache_open
        replacement_registry.rmdir()
        swapped_registry.rename(replacement_registry)

    unbound = cargo_home / "unbound-post-Cargo-entry"
    unbound.write_bytes(b"not in the final inventory\n")
    unbound.chmod(0o600)
    with pytest.raises(module.ReceiptError, match="exact final tree"):
        module._validate_cargo_cache_input(fields, artifact_root=artifact_root.resolve())
    unbound.unlink()

    config = cargo_home / "config.toml"
    config.write_text(
        '[target."cfg(all())"]\nrunner = "fake-test-runner"\n', encoding="utf-8"
    )
    config.chmod(0o600)
    with pytest.raises(module.ReceiptError, match="contains external configuration"):
        module._validate_cargo_cache_input(fields, artifact_root=artifact_root.resolve())
    config.unlink()

    external = tmp_path / "external-cache-file"
    external.write_bytes(b"external\n")
    external.chmod(0o600)
    hardlink = cargo_home / "generated-hardlink"
    os.link(external, hardlink)
    with pytest.raises(module.ReceiptError, match="hard-link escape"):
        module._validate_cargo_cache_input(fields, artifact_root=artifact_root.resolve())
    hardlink.unlink()
    external_link = cargo_home / "generated-external-link"
    external_link.symlink_to(Path("../..") / external.name)
    with pytest.raises(module.ReceiptError, match="external symlink escape"):
        module._validate_cargo_cache_input(fields, artifact_root=artifact_root.resolve())
    external_link.unlink()

    inventory.chmod(0o600)
    file_record = next(
        record for record in inventory_value["records"] if record["kind"] == "file"
    )
    file_record["source_device"] = file_record["destination_device"]
    file_record["source_inode"] = file_record["destination_inode"]
    inventory.write_bytes(canonical_json(inventory_value))
    inventory.chmod(0o400)
    fields["cargo_cache_input_inventory_sha256"] = sha256(inventory)
    with pytest.raises(module.ReceiptError, match="shares a regular-file inode"):
        module._validate_cargo_cache_input(fields, artifact_root=artifact_root.resolve())

    overlap_root = tmp_path / "overlapping-cache"
    overlap_root.mkdir(mode=0o700)
    overlap_home = overlap_root / "cargo-home"
    overlap_home.mkdir(mode=0o700)
    overlap = run_fixture_cargo_cache_copy(
        runner, overlap_root, overlap_home, overlap_root / "cargo-cache-input.json"
    )
    assert overlap.returncode != 0
    assert "must be disjoint" in overlap.stderr
    assert not tuple(overlap_home.iterdir())

    occupied_output = tmp_path / "occupied-output"
    occupied_output.mkdir(mode=0o700)
    occupied_home = occupied_output / "cargo-home"
    occupied_home.mkdir(mode=0o700)
    occupied_inventory = occupied_output / "cargo-cache-input.json"
    occupied_inventory.write_bytes(b"preexisting inventory\n")
    occupied_inventory.chmod(0o400)
    occupied = run_fixture_cargo_cache_copy(
        runner, source_home, occupied_home, occupied_inventory
    )
    assert occupied.returncode != 0
    assert occupied_inventory.read_bytes() == b"preexisting inventory\n"
    assert not tuple(occupied_home.iterdir())

    for case, expected_error in (
        ("absolute-link", "absolute target"),
        ("escaping-link", "escapes its cache root"),
        ("special-file", "forbidden special file"),
        ("oversized-file", "exceeds its size limit"),
    ):
        case_root = tmp_path / case
        case_source = case_root / "caller"
        case_registry = case_source / "registry"
        case_registry.mkdir(parents=True, mode=0o700)
        case_source.chmod(0o700)
        case_registry.chmod(0o700)
        if case == "absolute-link":
            (case_registry / "entry").symlink_to(external)
        elif case == "escaping-link":
            outside = case_source / "outside"
            outside.write_bytes(b"outside\n")
            outside.chmod(0o600)
            (case_registry / "entry").symlink_to("../outside")
        elif case == "special-file":
            os.mkfifo(case_registry / "entry", 0o600)
        else:
            with (case_registry / "entry").open("wb") as sparse:
                sparse.truncate(4 * 1024 * 1024 * 1024 + 1)
        case_output = case_root / "output"
        case_output.mkdir(mode=0o700)
        case_output.chmod(0o700)
        case_cargo_home = case_output / "cargo-home"
        case_cargo_home.mkdir(mode=0o700)
        case_cargo_home.chmod(0o700)
        rejected = run_fixture_cargo_cache_copy(
            runner,
            case_source,
            case_cargo_home,
            case_output / "cargo-cache-input.json",
        )
        assert rejected.returncode != 0
        assert expected_error in rejected.stderr
    exercise_private_pr_outer_lifecycle(runner, tmp_path)

@pytest.mark.parametrize(
    ("field_path", "replacement"),
    [
        (("schema_version",), True),
        (("schema_version",), 1.0),
        (("trust_boundary", "same_uid_and_trusted_ancestor_owners"), 1),
        (("trusted_inputs", "revocation", "size_bytes"), False),
        (("runner", "size_bytes"), True),
        (("runner", "size_bytes"), 1.0),
        (("runner", "mode"), 0o755),
        (("runner", "path_entries"), ["relative-path"]),
        (
            ("runner", "self_digest_environment_variables"),
            ["SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"],
        ),
        (("trusted_execution_probes", "bash", "exit_status"), False),
    ],
)
def test_receipt_rejects_bootstrap_marker_schema_and_type_confusion(
    tmp_path: Path, field_path: tuple[str, ...], replacement: object
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    mutate_bootstrap_marker(
        evidence,
        lambda value: set_nested(value, field_path, replacement),
    )

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()
    assert not terminal_output_path(evidence).exists()


def test_receipt_rejects_bootstrap_marker_without_exact_external_digest(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    evidence["expected_bootstrap_completion_sha256"] = "0" * 64

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "out-of-band digest" in result.stderr
    assert not terminal_output_path(evidence).exists()


@pytest.mark.parametrize(
    "artifact_name",
    [
        "trusted-bootstrap.py",
        "compute-manifest.py",
        "verify-identity.py",
        "python3",
        "bash",
        "bootstrap-allowed-signers",
        "bootstrap-revocation",
    ],
)
def test_receipt_rejects_tampered_bootstrap_trusted_archives(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(directory, Path)
    artifact = directory / artifact_name
    original_mode = artifact.stat().st_mode & 0o7777
    artifact.chmod(0o700 if original_mode == 0o500 else 0o600)
    artifact.write_bytes(artifact.read_bytes() + b"tampered\n")
    artifact.chmod(original_mode)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()
    assert not terminal_output_path(evidence).exists()


@pytest.mark.parametrize("mutation", ["mode", "symlink", "hardlink"])
def test_receipt_rejects_bootstrap_archive_path_and_inode_aliases(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(directory, Path)
    helper = directory / "compute-manifest.py"
    if mutation == "mode":
        helper.chmod(0o600)
    elif mutation == "symlink":
        real = directory / "compute-manifest.real"
        helper.rename(real)
        helper.symlink_to(real.name)
    elif mutation == "hardlink":
        verifier = directory / "verify-identity.py"
        verifier.unlink()
        os.link(helper, verifier)
    else:
        raise AssertionError(mutation)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()
    assert not terminal_output_path(evidence).exists()


@pytest.mark.parametrize(
    ("key", "basename"),
    [
        ("bootstrap_completion", "BOOTSTRAP_COMPLETED.copy.json"),
        ("bootstrap_identity", "candidate-identity.copy.json"),
        ("bootstrap_attestation", "identity-attestation.copy.json"),
        ("bootstrap_transcript", "identity-transcript.copy.json"),
    ],
)
def test_receipt_rejects_bootstrap_cli_path_aliases(
    tmp_path: Path, key: str, basename: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    source = evidence[key]
    directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(source, Path)
    assert isinstance(directory, Path)
    alias = directory / basename
    alias.write_bytes(source.read_bytes())
    alias.chmod(source.stat().st_mode & 0o7777)
    evidence[key] = alias

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "exact evidence path" in result.stderr


@pytest.mark.parametrize(
    ("field_path", "replacement"),
    [
        (("runner", "invocation", "profile"), "pr"),
        (("runner", "invocation", "operation_id"), "forged.release.v1"),
        (("runner", "invocation", "arguments", "0"), "--pr"),
        (
            ("runner", "invocation", "bash_archive_id"),
            "release-bootstrap.forged-bash.v1",
        ),
        (("runner", "closed_path_resolution", "git"), "forged-bootstrap.git.v1"),
        (("runner", "environment_sha256"), "0" * 64),
        (("runner", "tool_directory"), "forged-runner-bin"),
    ],
)
def test_receipt_rejects_bootstrap_runner_aliases(
    tmp_path: Path, field_path: tuple[str, ...], replacement: object
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)

    def mutate(value: dict[str, object]) -> None:
        if field_path[-1].isdigit():
            current: object = value
            for field in field_path[:-2]:
                assert isinstance(current, dict)
                current = current[field]
            assert isinstance(current, dict)
            sequence = current[field_path[-2]]
            assert isinstance(sequence, list)
            sequence[int(field_path[-1])] = replacement
        else:
            set_nested(value, field_path, replacement)

    mutate_bootstrap_marker(evidence, mutate)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()


def test_receipt_requires_distinct_original_and_sealed_candidate_roots(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    evidence["bootstrap_candidate_root"] = evidence["release_root"]

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "must be distinct" in result.stderr


def test_receipt_requires_sealed_source_external_to_bootstrap_archive(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    bootstrap_evidence = evidence["bootstrap_evidence_dir"]
    assert isinstance(bootstrap_evidence, Path)
    wrong_root = bootstrap_evidence / "nested-sealed-source"
    wrong_root.mkdir()
    source_lock = evidence["signature_cargo_lock"]
    assert isinstance(source_lock, Path)
    (wrong_root / "Cargo.lock").write_bytes(source_lock.read_bytes())
    evidence["release_root"] = wrong_root

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "sealed release root must be external to the bootstrap archive" in result.stderr


def test_receipt_requires_exact_terminal_output_path(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    wrong_parent = tmp_path / "wrong-output"
    wrong_parent.mkdir(mode=0o700)
    wrong_parent.chmod(0o700)
    wrong_output = wrong_parent / "RELEASE_COMPLETED.json"

    result = run_writer(
        evidence,
        wrong_output,
        writer,
        use_supplied_output=True,
    )

    assert result.returncode == 1
    assert "exact bootstrap release output path" in result.stderr
    assert not wrong_output.exists()
    assert not terminal_output_path(evidence).exists()
