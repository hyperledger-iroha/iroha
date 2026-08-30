from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import stat
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).parents[2]
SCRIPT = ROOT / "scripts/package_privacy_wallet_worker.py"
SPEC = importlib.util.spec_from_file_location("package_privacy_wallet_worker", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
package = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = package
SPEC.loader.exec_module(package)

DIGESTS = tuple(f"{index:02x}" * 32 for index in range(1, 16))
COMMIT = "a" * 40


def source_evidence() -> package.SourceEvidenceV1:
    return package.SourceEvidenceV1(
        allowed_signers_sha256=DIGESTS[0],
        cargo_lock_sha256=DIGESTS[1],
        commit=COMMIT,
        revocation_sha256=DIGESTS[2],
        source_closure_sha256=DIGESTS[3],
        source_date_epoch=1_700_000_000,
        workspace_source_manifest_sha256=DIGESTS[4],
    )


def fake_build_file(role: str, path: str) -> dict[str, object]:
    return {
        "mode": 0o500,
        "owner": 0,
        "path": path,
        "sha256": hashlib.sha256(role.encode()).hexdigest(),
        "size": len(role) + 1,
    }


def fake_build_provenance(
    source: package.SourceEvidenceV1,
    *,
    path: str = "/tools:/usr/bin:/bin",
    wrapper_digest_role: str = "cargo_iroha_fast",
    rust_component_role: str = "rustc-component",
) -> dict[str, object]:
    target = package.RELEASE_TARGET
    suffix = target.upper().replace("-", "_").replace(".", "_")
    cc_suffix = target.replace("-", "_").replace(".", "_")
    environment = package._build_environment_values(source)
    environment.update(
        {
            "AR": "/tools/ar",
            f"AR_{cc_suffix}": "/tools/ar",
            "CC": "/tools/cc",
            f"CC_{cc_suffix}": "/tools/cc",
            f"CARGO_TARGET_{suffix}_LINKER": "/tools/cc",
            "HOME": "/home/release",
            "PATH": path,
            "RUSTC": "/rust/bin/rustc",
            "RUSTC_WRAPPER": "/tools/sccache",
        }
    )
    tool_paths = {
        "archiver": "/tools/ar",
        "cargo": "/rust/bin/cargo",
        "cargo_iroha_fast": "/tools/cargo-iroha-fast",
        "dirname": "/usr/bin/dirname",
        "env": "/usr/bin/env",
        "git": "/usr/bin/git",
        "grep": "/usr/bin/grep",
        "linker": "/tools/ld",
        "linker_driver": "/tools/cc",
        "rustc": "/rust/bin/rustc",
        "rustc_wrapper": "/tools/sccache",
        "shell": "/bin/bash",
        "uname": "/usr/bin/uname",
    }
    tools = {
        role: fake_build_file(
            wrapper_digest_role if role == "cargo_iroha_fast" else role,
            tool_paths[role],
        )
        for role in package._BUILD_TOOL_ROLES
    }

    def component(role: str) -> dict[str, object]:
        digest_role = rust_component_role if role == "rustc" else role
        manifest_names = {
            "cargo": f"manifest-cargo-{target}",
            "rust_std": f"manifest-rust-std-{target}",
            "rustc": f"manifest-rustc-{target}",
        }
        return {
            "closure_sha256": hashlib.sha256(
                f"closure:{digest_role}".encode()
            ).hexdigest(),
            "file_count": 1,
            "manifest_path": f"/rust/lib/rustlib/{manifest_names[role]}",
            "manifest_sha256": hashlib.sha256(
                f"manifest:{digest_role}".encode()
            ).hexdigest(),
            "total_bytes": 1024,
        }

    toolchain = {
        "cargo_configuration": [],
        "cargo_version_sha256": hashlib.sha256(b"cargo-version").hexdigest(),
        "components": {
            role: component(role) for role in package._RUST_COMPONENT_ROLES
        },
        "host": target,
        "rustc_version_sha256": hashlib.sha256(b"rustc-version").hexdigest(),
        "schema": package._BUILD_TOOLCHAIN_SCHEMA,
        "sysroot": "/rust",
        "target": target,
        "tools": tools,
    }
    return package._build_provenance_v2(
        environment,
        toolchain,
        source=source,
        target=target,
    )


def fake_worker_source() -> bytes:
    return b"""#!/usr/bin/python3
import hashlib
import hmac
import struct
import sys

stream = sys.stdin.buffer.read()
key, request = stream[:32], stream[32:]
if len(key) != 32 or not any(key) or len(request) < 54:
    raise SystemExit(64)
declared = struct.unpack(\">I\", request[:4])[0]
body = request[4:]
authenticated, tag = body[:-32], body[-32:]
if declared != len(body) or not hmac.compare_digest(
    tag, hmac.new(key, authenticated, hashlib.sha256).digest()
):
    raise SystemExit(65)
if authenticated != b\"IPWW\" + bytes((2, 1)) + (1).to_bytes(8, \"big\") + (0).to_bytes(4, \"big\"):
    raise SystemExit(66)
response = b\"IPWW\" + bytes((2, 1)) + (1).to_bytes(8, \"big\") + (1).to_bytes(4, \"big\") + b\"\\x00\"
response += hmac.new(key, response, hashlib.sha256).digest()
sys.stdout.buffer.write(len(response).to_bytes(4, \"big\") + response)
"""


def executable(tmp_path: Path) -> tuple[Path, package.StableFileV1]:
    artifact = (tmp_path / package.ARTIFACT_FILE).resolve()
    artifact.write_bytes(fake_worker_source())
    artifact.chmod(0o700)
    identity = package._stable_file(
        artifact,
        label="fixture worker",
        maximum=package._MAX_ARTIFACT_BYTES,
        require_executable=True,
        require_owner=True,
    )
    return artifact, identity


def candidate_manifest(tmp_path: Path) -> tuple[Path, dict[str, object]]:
    artifact, identity = executable(tmp_path)
    return artifact, package.build_manifest(
        artifact=identity,
        source=source_evidence(),
        target="aarch64-apple-darwin",
    )


def release_manifest(tmp_path: Path) -> tuple[Path, dict[str, object]]:
    artifact, identity = executable(tmp_path)
    source = source_evidence()
    return artifact, package.build_manifest(
        artifact=identity,
        source=source,
        target=package.RELEASE_TARGET,
        build_method=package.AUTHENTICATED_SOURCE_BUILD_V2,
        build_command_sha256=package._build_command_sha256(package.RELEASE_TARGET),
        build_provenance=fake_build_provenance(source),
    )


def test_checked_in_source_closure_is_exact_and_deterministic() -> None:
    assert package._source_closure_paths(ROOT) == package._EXPECTED_SOURCE_CLOSURE
    first = package.source_closure_sha256(ROOT)
    assert first == package.source_closure_sha256(ROOT)
    assert first != "0" * 64


def test_operation_registry_is_closed_ordered_and_hash_pinned() -> None:
    registry = package.operation_registry_manifest_v1()
    assert len(registry) == 11
    assert sum(len(row["operation_schemas"]) for row in registry) == 12
    assert registry[3] == {
        "operation_schemas": [
            "zk_ams_batch_admission_action_v1",
            "zk_ams_provision_account_action_v1",
        ],
        "protocol_id": "iroha-zk-ams-v1",
    }
    assert registry[5]["protocol_id"] == "iroha-jindo-polynomial-commitment-v0"
    assert all("x509" not in row["protocol_id"] for row in registry)
    assert len(package.operation_registry_sha256_v1()) == 64


def test_javascript_sdk_cannot_reintroduce_owner_secret_bundle_state() -> None:
    source_root = ROOT / "javascript/iroha_js/src"
    declarations = (
        ROOT / "javascript/iroha_js/index.d.ts",
        ROOT / "javascript/iroha_js/privacy-capabilities.d.ts",
    )
    surfaces = tuple(sorted(source_root.glob("**/*.js"))) + declarations
    forbidden = (
        "credentialBytes",
        "credential_bytes",
        "executionBundle",
        "execution_bundle",
        "ownerBundle",
        "owner_bundle",
        "protocolWitness",
        "protocol_witness",
        "secretBundle",
        "secret_bundle",
        "signerSeed",
        "signer_seed",
        "witnessBytes",
        "witness_bytes",
    )
    violations = [
        f"{path.relative_to(ROOT)}:{token}"
        for path in surfaces
        for token in forbidden
        if token in path.read_text(encoding="utf-8")
    ]
    assert violations == []


def test_prebuilt_artifact_cannot_be_promoted_by_a_ready_manifest(
    tmp_path: Path,
) -> None:
    _, manifest = candidate_manifest(tmp_path)
    assert manifest["release_ready"] is False
    manifest["release_ready"] = True
    with pytest.raises(package.PrivacyWalletWorkerPackageError, match="inconsistent"):
        package.validate_manifest(manifest)


def test_authenticated_release_manifest_requires_exact_target_and_build_pins(
    tmp_path: Path,
) -> None:
    _, manifest = release_manifest(tmp_path)
    assert manifest["release_ready"] is True
    for field in (
        "artifact_build_command_sha256",
        "artifact_build_environment_sha256",
        "artifact_build_toolchain_sha256",
    ):
        changed = json.loads(json.dumps(manifest))
        changed[field] = "f" * 64
        with pytest.raises(package.PrivacyWalletWorkerPackageError, match="provenance"):
            package.validate_manifest(changed)


def test_effective_path_and_inherited_environment_drift_changes_provenance() -> None:
    source = source_evidence()
    first = fake_build_provenance(source)
    changed_path = fake_build_provenance(
        source,
        path="/different-tools:/usr/bin:/bin",
    )
    assert first["environment_sha256"] != changed_path["environment_sha256"]

    environment = dict(first["environment"])
    environment["CARGO_TARGET_DIR"] = "/different-target"
    changed_inherited = package._build_provenance_v2(
        environment,
        first["toolchain"],
        source=source,
        target=package.RELEASE_TARGET,
    )
    assert first["environment_sha256"] != changed_inherited["environment_sha256"]


def test_wrapper_and_rust_toolchain_drift_change_provenance() -> None:
    source = source_evidence()
    first = fake_build_provenance(source)
    wrapper_drift = fake_build_provenance(
        source,
        wrapper_digest_role="replaced-cargo-iroha-fast",
    )
    toolchain_drift = fake_build_provenance(
        source,
        rust_component_role="replaced-rustc-component",
    )
    assert first["toolchain_sha256"] != wrapper_drift["toolchain_sha256"]
    assert first["toolchain_sha256"] != toolchain_drift["toolchain_sha256"]


def test_path_resolution_and_wrapper_byte_drift_are_observed(tmp_path: Path) -> None:
    first_dir = tmp_path / "first"
    second_dir = tmp_path / "second"
    first_dir.mkdir()
    second_dir.mkdir()
    first_wrapper = first_dir / "cargo-iroha-fast"
    second_wrapper = second_dir / "cargo-iroha-fast"
    first_wrapper.write_bytes(b"#!/bin/sh\nexit 0\n")
    second_wrapper.write_bytes(b"#!/bin/sh\nexit 1\n")
    first_wrapper.chmod(0o700)
    second_wrapper.chmod(0o700)

    first_path = package._resolve_build_executable(
        "cargo-iroha-fast",
        {"PATH": f"{first_dir}:{second_dir}"},
        label="fixture wrapper",
    )
    second_path = package._resolve_build_executable(
        "cargo-iroha-fast",
        {"PATH": f"{second_dir}:{first_dir}"},
        label="fixture wrapper",
    )
    first_record = package._stable_build_input_record(
        first_path,
        label="fixture wrapper",
        require_executable=True,
    )
    second_record = package._stable_build_input_record(
        second_path,
        label="fixture wrapper",
        require_executable=True,
    )
    assert first_record["path"] != second_record["path"]
    assert first_record["sha256"] != second_record["sha256"]


def test_rust_component_file_drift_changes_closure(tmp_path: Path) -> None:
    sysroot = tmp_path / "rust"
    manifests = sysroot / "lib" / "rustlib"
    manifests.mkdir(parents=True)
    driver = sysroot / "lib" / "driver.so"
    driver.write_bytes(b"first rustc driver")
    manifest = manifests / "manifest-rustc-fixture"
    manifest.write_text("file:lib/driver.so\n", encoding="utf-8")
    first = package._rust_component_closure_record(
        sysroot,
        "manifest-rustc-fixture",
        label="fixture rustc component",
    )
    driver.write_bytes(b"second rustc driver")
    second = package._rust_component_closure_record(
        sysroot,
        "manifest-rustc-fixture",
        label="fixture rustc component",
    )
    assert first["closure_sha256"] != second["closure_sha256"]


@pytest.mark.parametrize(
    "mutation",
    ("path", "wrapper", "rust_component"),
)
def test_release_manifest_rejects_unrepinned_build_input_drift(
    tmp_path: Path,
    mutation: str,
) -> None:
    _, manifest = release_manifest(tmp_path)
    changed = json.loads(json.dumps(manifest))
    provenance = changed["artifact_build_provenance"]
    if mutation == "path":
        provenance["environment"]["PATH"] = "/attacker:/usr/bin:/bin"
    elif mutation == "wrapper":
        provenance["toolchain"]["tools"]["cargo_iroha_fast"]["sha256"] = "f" * 64
    else:
        provenance["toolchain"]["components"]["rustc"]["closure_sha256"] = "f" * 64
    with pytest.raises(package.PrivacyWalletWorkerPackageError, match="provenance"):
        package.validate_manifest(changed)


def test_prebuilt_candidate_cannot_carry_authenticated_build_evidence(
    tmp_path: Path,
) -> None:
    _, manifest = candidate_manifest(tmp_path)
    provenance = fake_build_provenance(source_evidence())
    manifest["artifact_build_provenance"] = provenance
    manifest["artifact_build_environment_sha256"] = provenance["environment_sha256"]
    manifest["artifact_build_toolchain_sha256"] = provenance["toolchain_sha256"]
    with pytest.raises(package.PrivacyWalletWorkerPackageError, match="cannot claim"):
        package.validate_manifest(manifest)


def test_registry_mutation_is_rejected(tmp_path: Path) -> None:
    _, manifest = candidate_manifest(tmp_path)
    manifest["operation_registry"][0]["protocol_id"] = "retired-alias"
    with pytest.raises(package.PrivacyWalletWorkerPackageError, match="registry"):
        package.validate_manifest(manifest)


def test_authenticated_ping_and_content_addressed_package_round_trip(
    tmp_path: Path,
) -> None:
    artifact, manifest = release_manifest(tmp_path)
    package.probe_worker_ping(artifact)
    output = (tmp_path / "output").resolve()
    output.mkdir(mode=0o700)
    installed = package.write_package(
        artifact_path=artifact,
        manifest=manifest,
        output_root=output,
    )
    verified = package.verify_package(
        installed,
        require_release_ready=True,
    )
    assert verified == manifest
    assert installed.name == hashlib.sha256(artifact.read_bytes()).hexdigest()
    assert stat.S_IMODE((installed / package.ARTIFACT_FILE).stat().st_mode) == 0o500
    assert stat.S_IMODE((installed / "manifest.json").stat().st_mode) == 0o400


def test_authenticated_ping_never_executes_a_swapped_worker_path(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    artifact, _identity = executable(tmp_path)
    admitted_bytes = artifact.read_bytes()
    observed_launch: dict[str, object] = {}

    def swapping_run(arguments, **kwargs):
        invocation = Path(os.fspath(arguments[0]))
        observed_launch["bytes"] = invocation.read_bytes()
        observed_launch["path"] = invocation
        backup = artifact.with_name("admitted-backup")
        artifact.rename(backup)
        artifact.write_bytes(b"#!/bin/sh\nexit 0\n")
        artifact.chmod(0o700)
        artifact.unlink()
        backup.rename(artifact)

        request = kwargs["input"]
        auth_key = request[:32]
        authenticated = b"".join(
            (
                b"IPWW",
                bytes((package.PROTOCOL_VERSION, 1)),
                (1).to_bytes(8, "big"),
                (1).to_bytes(4, "big"),
                b"\0",
            )
        )
        response = authenticated + package.hmac.new(
            auth_key,
            authenticated,
            package.hashlib.sha256,
        ).digest()
        encoded = len(response).to_bytes(4, "big") + response
        return package.subprocess.CompletedProcess(arguments, 0, encoded, b"")

    monkeypatch.setattr(package.subprocess, "run", swapping_run)
    with pytest.raises(
        package.PrivacyWalletWorkerPackageError,
        match="authenticated ping failed",
    ):
        package.probe_worker_ping(artifact)
    assert observed_launch["bytes"] == admitted_bytes
    assert observed_launch["path"] != artifact


def test_package_tamper_is_rejected_before_ping(tmp_path: Path) -> None:
    artifact, manifest = candidate_manifest(tmp_path)
    output = (tmp_path / "output").resolve()
    output.mkdir(mode=0o700)
    installed = package.write_package(
        artifact_path=artifact,
        manifest=manifest,
        output_root=output,
    )
    packaged_artifact = installed / package.ARTIFACT_FILE
    installed.chmod(0o700)
    packaged_artifact.chmod(0o700)
    packaged_artifact.write_bytes(packaged_artifact.read_bytes() + b"\n# tamper\n")
    packaged_artifact.chmod(0o500)
    installed.chmod(0o500)
    with pytest.raises(package.PrivacyWalletWorkerPackageError, match="manifest"):
        package.verify_package(installed)


def test_frozen_build_environment_drops_compiler_and_loader_injection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    allowed = {
        "CARGO_HOME": "/cargo",
        "CARGO_TARGET_DIR": "/target",
        "HOME": "/home",
        "PATH": "/bin",
        "RUSTUP_HOME": "/rustup",
        "SCCACHE_DIR": "/sccache",
        "TMPDIR": "/tmp",
    }
    for name, value in allowed.items():
        monkeypatch.setenv(name, value)
    for name in (
        "CARGO_BUILD_RUSTC",
        "CARGO_ENCODED_RUSTFLAGS",
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTFLAGS",
    ):
        monkeypatch.setenv(name, "/attacker")
    environment = package._frozen_build_environment(source_evidence())
    for name, value in allowed.items():
        assert environment[name] == value
    for name in (
        "CARGO_BUILD_RUSTC",
        "CARGO_ENCODED_RUSTFLAGS",
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTFLAGS",
    ):
        assert name not in environment
    assert environment["IROHA_PYTHON_SKIP_RUNTIME_LINK"] == "1"
    assert environment["CARGO_NET_OFFLINE"] == "true"


def test_source_identity_change_during_evidence_collection_is_rejected(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source_root = tmp_path / "source"
    source_root.mkdir()
    allowed_signers = (tmp_path / "allowed_signers").resolve()
    allowed_signers.write_bytes(b"release@example ssh-ed25519 AAAATEST\n")
    revocation = (tmp_path / "revocation").resolve()
    revocation.write_bytes(b"")
    allowed_sha = hashlib.sha256(allowed_signers.read_bytes()).hexdigest()
    revocation_sha = hashlib.sha256(b"").hexdigest()
    first = {
        "cargo_lock_sha256": DIGESTS[1],
        "head_commit": COMMIT,
        "head_tree": "b" * 40,
        "index_tree": "b" * 40,
        "schema_version": 1,
        "workspace_source_manifest_sha256": DIGESTS[4],
    }
    second = dict(first)
    second["workspace_source_manifest_sha256"] = DIGESTS[5]
    identities = iter((first, second))
    monkeypatch.setattr(package, "_raw_release_source_identity", lambda _root: next(identities))
    monkeypatch.setattr(package, "_verify_source_signature", lambda *_args: None)
    monkeypatch.setattr(package, "source_closure_sha256", lambda _root: DIGESTS[3])
    monkeypatch.setattr(
        package,
        "_git",
        lambda _root, _args: "1700000000\n",
    )
    with pytest.raises(package.PrivacyWalletWorkerPackageError, match="changed"):
        package.collect_source_evidence(
            source_root,
            allowed_signers=allowed_signers,
            expected_allowed_signers_sha256=allowed_sha,
            revocation=revocation,
            expected_revocation_sha256=revocation_sha,
        )


def test_git_environment_drops_repository_redirectors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("GIT_DIR", "/attacker")
    monkeypatch.setenv("GIT_WORK_TREE", "/attacker/worktree")
    monkeypatch.setenv("LD_PRELOAD", "/attacker/loader")
    environment = package._git_environment()
    assert "GIT_DIR" not in environment
    assert "GIT_WORK_TREE" not in environment
    assert "LD_PRELOAD" not in environment
    assert environment["GIT_CONFIG_GLOBAL"] == os.devnull
    assert environment["GIT_NO_REPLACE_OBJECTS"] == "1"
