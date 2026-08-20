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
import sysconfig
import time

import pytest
from pytests.scripts import sumeragi_v2_release_bootstrap_tool_manifest_support as _tool_support

REPO_ROOT = Path(__file__).resolve().parents[2]
BOOTSTRAP = REPO_ROOT / "scripts" / "bootstrap_sumeragi_v2_release.py"
BOOTSTRAP_COMPONENTS = tuple(
    REPO_ROOT / "scripts" / name
    for name in ("bootstrap_sumeragi_v2_release_receipt_replay.py",)
)
APPROVAL_CONTRACT = (
    REPO_ROOT / "scripts" / "sumeragi_v2_release_approval_contract.py"
)
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
    "IROHA_RELEASE_APALACHE_BIN",
    "IROHA_RELEASE_CANCEL_REQUEST_PATH",
    "IROHA_RELEASE_TLA2TOOLS_JAR",
)
DEFAULT_SCALING_DIGESTS = {
    "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": "a" * 64,
    "IROHA_RELEASE_SCALING_IROHAD_SHA256": "b" * 64,
    "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": "c" * 64,
    "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": "d" * 64,
}
APPROVAL_EVIDENCE_ROOT_ID = "fixture-release-evidence-root"
APPROVAL_DURATIONS = (900, 901, 902, 903)
RELEASE_BOOTSTRAP_TEST_COMPONENT_FILES = (
    "sumeragi_v2_release_bootstrap_terminal_cases.py",
    "sumeragi_v2_release_bootstrap_environment_cases.py",
)


def _execute_bootstrap_test_component(filename: str) -> None:
    """Execute one reviewed case component in this canonical test namespace."""
    path = Path(__file__).with_name(filename)
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"release-bootstrap test component is unavailable: {path}")
    source = path.read_text(encoding="utf-8")
    exec(compile(source, str(path), "exec"), globals())


def _load_bootstrap_module() -> object:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_test_module", BOOTSTRAP
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_release_trust_inputs_are_the_only_new_runner_environment_names(
    tmp_path: Path,
) -> None:
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

    names = tuple(path.name for path in BOOTSTRAP_COMPONENTS)
    assert module._BOOTSTRAP_COMPONENT_FILES == names
    assert set(module._BOOTSTRAP_COMPONENT_SHA256) == set(names)
    assert all(path.is_file() and not path.is_symlink() for path in BOOTSTRAP_COMPONENTS)
    assert {
        path.name: _sha256(path) for path in BOOTSTRAP_COMPONENTS
    } == module._BOOTSTRAP_COMPONENT_SHA256

    copied = tmp_path / BOOTSTRAP.name
    copied.write_text(
        BOOTSTRAP.read_text(encoding="utf-8").replace(
            next(iter(module._BOOTSTRAP_COMPONENT_SHA256.values())),
            "0" * 64,
            1,
        ),
        encoding="utf-8",
    )
    copied.chmod(0o500)
    for component in BOOTSTRAP_COMPONENTS:
        shutil.copy2(component, tmp_path / component.name)
    result = subprocess.run(
        [str(PYTHON), "-I", "-S", str(copied), "--help"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert result.returncode != 0
    assert b"bootstrap component binding is invalid" in result.stderr


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


def test_authenticated_sdk_source_manifest_pruning_is_exact(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    evidence = tmp_path / "evidence"
    evidence.mkdir(mode=0o700)
    manifest = _write(
        evidence / "sdk-dependency-bundle-manifest.json",
        b'{"schema_version":1}\n',
        0o400,
    )
    snapshot = module._read_file(
        manifest,
        "SDK source manifest fixture",
        maximum_bytes=module._MAX_SDK_MANIFEST_BYTES,
    )
    evidence_fd = os.open(evidence, os.O_RDONLY | os.O_DIRECTORY)
    try:
        module._prune_authenticated_sdk_source_manifest(evidence_fd, snapshot)
        module._require_sdk_source_manifest_pruned(evidence_fd, snapshot)
        assert not os.path.lexists(manifest)

        _write(manifest, snapshot.data, snapshot.mode)
        with pytest.raises(
            module.BootstrapError,
            match="survived acknowledgment pruning",
        ):
            module._require_sdk_source_manifest_pruned(evidence_fd, snapshot)
    finally:
        os.close(evidence_fd)


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


def _load_approval_component(path: Path) -> object:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_approval_fixture", path
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _approval_fixture_files(
    *,
    contract: Path,
    trust: Path,
    identity: dict[str, object],
    tool_manifest_sha256: str,
) -> dict[str, Path]:
    module = _load_approval_component(contract)
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
    paths: dict[str, Path] = {}
    for ordinal, approval_class in enumerate(module.APPROVAL_CLASS_ORDER):
        expectation = expectations[approval_class]
        value = {
            "approval_id": f"fixture-approval-{ordinal}-{approval_class.value}",
            "approved_at": f"2026-08-{ordinal + 1:02d}T01:02:03Z",
            "candidate_oid": expectation.candidate_oid,
            "candidate_tree": expectation.candidate_tree,
            "class_id": approval_class.value,
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
        paths[approval_class.value] = _write(
            trust / module.APPROVAL_FILENAMES[approval_class],
            (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(),
            0o400,
        )
    return paths


def _candidate_release_identity(candidate: Path, manifest: Path) -> dict[str, object]:
    result = subprocess.run(
        [
            str(PYTHON),
            "-I",
            "-S",
            str(manifest),
            "--root",
            str(candidate),
            "--release-identity-json",
        ],
        cwd=candidate,
        env={"PATH": os.environ.get("PATH", "")},
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert result.returncode == 0, result.stderr.decode()
    identity = json.loads(result.stdout)
    assert isinstance(identity, dict)
    return identity


def _fixture_canonical(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def _sdk_dependency_fixture_material() -> tuple[
    dict[str, bytes], list[dict[str, object]], dict[str, object]
]:
    package_lock = _fixture_canonical({
        "name": "sdk-fixture",
        "version": "1.0.0",
        "lockfileVersion": 3,
        "packages": {
            "": {"name": "sdk-fixture", "version": "1.0.0"},
            "node_modules/fixture": {"version": "1.0.0"},
        },
    })
    installed_lock = _fixture_canonical({
        "name": "sdk-fixture",
        "version": "1.0.0",
        "lockfileVersion": 3,
        "packages": {"node_modules/fixture": {"version": "1.0.0"}},
    })
    revision = "a" * 40
    tree = "b" * 40
    package_resolved = _fixture_canonical({
        "pins": [{
            "identity": "fixture",
            "kind": "remoteSourceControl",
            "location": "https://example.invalid/fixture.git",
            "state": {"revision": revision, "version": "1.0.0"},
        }],
        "version": 2,
    })
    wrapper = (
        b"distributionBase=GRADLE_USER_HOME\n"
        b"distributionPath=wrapper/dists\n"
        b"distributionUrl=https\\://services.gradle.org/distributions/gradle-9.3.0-bin.zip\n"
        b"zipStoreBase=GRADLE_USER_HOME\n"
        b"zipStorePath=wrapper/dists\n"
    )
    gradle_key = "79n14ral3mx1ozqr3csh2u872"
    launcher = (
        "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
        f"{gradle_key}/gradle-9.3.0/bin/gradle"
    )
    files = {
        "gradle/gradle-9.3.0-bin.zip": b"fixture Gradle 9.3.0 distribution\n",
        launcher: b"#!/bin/sh\n",
        (
            "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
            f"{gradle_key}/gradle-9.3.0-bin.zip.ok"
        ): b"",
        "gradle/java-gradle-wrapper.properties": wrapper,
        "gradle/kotlin-gradle-wrapper.properties": wrapper,
        "node/node_modules/.package-lock.json": installed_lock,
        "node/package-lock.json": package_lock,
        "openapi/node_modules/.package-lock.json": installed_lock,
        "openapi/package-lock.json": package_lock,
        "swiftpm/cache/checkouts/fixture/.git/HEAD": (revision + "\n").encode(),
        "swiftpm/Package.resolved": package_resolved,
    }
    directories = (
        "gradle", "gradle/gradle-user-home", "gradle/gradle-user-home/caches",
        "gradle/gradle-user-home/caches/9.3.0",
        "gradle/gradle-user-home/caches/modules-2",
        "gradle/gradle-user-home/wrapper",
        "gradle/gradle-user-home/wrapper/dists",
        "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin",
        f"gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/{gradle_key}",
        f"gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/{gradle_key}/gradle-9.3.0",
        f"gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/{gradle_key}/gradle-9.3.0/bin",
        "node", "node/node_modules", "openapi", "openapi/node_modules",
        "swiftpm", "swiftpm/cache",
        "swiftpm/cache/checkouts", "swiftpm/cache/checkouts/fixture",
        "swiftpm/cache/checkouts/fixture/.git", "swiftpm/cache/repositories",
    )
    records: list[dict[str, object]] = [
        {"path": ".", "kind": "directory", "mode": "0500"},
        *(
            {"path": path, "kind": "directory", "mode": "0500"}
            for path in directories
        ),
        *(
            {
                "path": path,
                "kind": "file",
                "mode": "0500" if path == launcher else "0400",
                "size": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            }
            for path, data in files.items()
        ),
    ]
    records = [records[0], *sorted(records[1:], key=lambda item: str(item["path"]))]
    bindings = {
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
            "resolved_revisions": [{
                "identity": "fixture", "checkout": "fixture",
                "revision": revision, "tree": tree,
            }],
        },
        "gradle": {
            "distribution_archive_name": "gradle/gradle-9.3.0-bin.zip",
            "distribution_sha256": hashlib.sha256(
                files["gradle/gradle-9.3.0-bin.zip"]
            ).hexdigest(),
            "distribution_url": (
                "https://services.gradle.org/distributions/gradle-9.3.0-bin.zip"
            ),
            "gradle_user_home_archive_name": "gradle/gradle-user-home",
            "launcher_archive_name": launcher,
            "wrapper_cache_key": gradle_key,
            "version": "9.3.0",
            "wrapper_properties_sha256": {
                "java": hashlib.sha256(wrapper).hexdigest(),
                "kotlin": hashlib.sha256(wrapper).hexdigest(),
            },
        },
    }
    return files, records, bindings


def _sdk_source_manifest_fixture(git: Path) -> bytes:
    _, records, bindings = _sdk_dependency_fixture_material()

    def inventory(prefix: str) -> dict[str, object]:
        projected: list[dict[str, object]] = []
        for record in records:
            path = str(record["path"])
            if path != prefix and not path.startswith(prefix + "/"):
                continue
            item = dict(record)
            item["path"] = "." if path == prefix else path.removeprefix(prefix + "/")
            if item["kind"] == "directory":
                item["mode"] = "0700"
            elif item["kind"] == "file":
                item["mode"] = "0700" if int(str(item["mode"]), 8) & 0o111 else "0600"
            projected.append(item)
        payload = json.dumps(
            projected, ensure_ascii=True, sort_keys=True, separators=(",", ":")
        ).encode()
        return {
            "format": "iroha-sumeragi-v2-sdk-dependency-source-inventory",
            "schema_version": 1,
            "record_count": len(projected),
            "file_bytes": sum(
                int(item.get("size", 0))
                for item in projected if item["kind"] == "file"
            ),
            "records_sha256": hashlib.sha256(payload).hexdigest(),
            "records": projected,
        }

    return _fixture_canonical({
        "format": "iroha-sumeragi-v2-sdk-dependency-sources",
        "schema_version": 3,
        "git": {"executable": str(git), "sha256": _sha256(git)},
        "node": {
            "node_modules_root": "/operator/node_modules",
            "node_modules_inventory": inventory("node/node_modules"),
            "package_lock_sha256": bindings["node"]["package_lock_sha256"],
        },
        "openapi_node": {
            "node_modules_root": "/operator/tools/openapi/node_modules",
            "node_modules_inventory": inventory("openapi/node_modules"),
            "package_lock_sha256": bindings["openapi_node"][
                "package_lock_sha256"
            ],
        },
        "swiftpm": {
            "cache_root": "/operator/swiftpm-cache",
            "cache_inventory": inventory("swiftpm/cache"),
            "package_resolved_sha256": bindings["swiftpm"]["package_resolved_sha256"],
            "resolved_revisions": bindings["swiftpm"]["resolved_revisions"],
        },
        "gradle": {
            "distribution_archive": "/operator/gradle-9.3.0-bin.zip",
            "distribution_sha256": bindings["gradle"]["distribution_sha256"],
            "distribution_url": bindings["gradle"]["distribution_url"],
            "gradle_user_home": "/operator/gradle-home",
            "gradle_user_home_inventory": inventory("gradle/gradle-user-home"),
            "java_wrapper_properties_sha256": bindings["gradle"][
                "wrapper_properties_sha256"
            ]["java"],
            "kotlin_wrapper_properties_sha256": bindings["gradle"][
                "wrapper_properties_sha256"
            ]["kotlin"],
            "version": "9.3.0",
            "wrapper_cache_key": bindings["gradle"]["wrapper_cache_key"],
        },
    })


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
tree = hashlib.sha256(b"tree" + payload).hexdigest()[:40]
raw = (
    f"tree {tree}\n"
    "author Release Fixture <release@example.test> 1700000000 +0000\n"
    "committer Release Fixture <release@example.test> 1700000000 +0000\n"
    "gpgsig -----BEGIN SSH SIGNATURE-----\n"
    " Zml4dHVyZQ==\n"
    " -----END SSH SIGNATURE-----\n\n"
    "Fixture release\n\n"
    "Sumeragi-V2-Release-Identity-Version: 1\n"
    f"Sumeragi-V2-Source-Manifest-SHA256: {digest}\n"
    f"Sumeragi-V2-Cargo-Lock-SHA256: {lock}\n"
).encode()
framed = b"commit " + str(len(raw)).encode() + b"\0" + raw
value = {
    "schema_version": 1,
    "head_commit": hashlib.sha1(framed, usedforsecurity=False).hexdigest(),
    "head_tree": tree,
    "index_tree": tree,
    "workspace_source_manifest_sha256": digest,
    "cargo_lock_sha256": lock,
}
print(json.dumps(value, sort_keys=True, separators=(",", ":")))
'''


def _receipt_validator(mutation: str = "") -> str:
    source = r'''#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

OPTION_ORDER = (
    "--candidate-identity", "--sealed-identity", "--release-root",
    "--bootstrap-completion", "--bootstrap-evidence-dir",
    "--bootstrap-identity", "--bootstrap-attestation",
    "--bootstrap-transcript", "--expected-bootstrap-completion-sha256",
    "--bootstrap-candidate-root", "--bootstrap-runner",
    "--signature-attestation", "--signature-transcript",
    "--signature-raw-commit", "--signature-cargo-lock",
    "--signature-allowed-signers", "--signature-revocation",
    "--signature-git", "--signature-ssh-keygen", "--expected-git-sha256",
    "--expected-ssh-keygen-sha256", "--expected-allowed-signers-sha256",
    "--expected-revocation-sha256", "--expected-signer-fingerprint",
    "--corridor-completion", "--formal-completion", "--seed-completion",
    "--chaos-completion", "--g4p-completion",
    "--g12-seed-completion", "--g12-fault-soak-completion",
    "--scaling-evidence-manifest",
    "--sdk-dependency-archive", "--sdk-dependency-input-inventory",
    "--sdk-dependency-final-work-inventory",
    "--runtime-tool-probe-manifest", "--runtime-tool-probe-result",
    "--expected-scaling-trial-harness-sha256",
    "--expected-scaling-configuration-sha256",
    "--expected-scaling-irohad-sha256",
    "--expected-scaling-iroha-cli-sha256", "--repository-root",
    "--output", "--verify-existing", "--validation-ack",
    "--source-manifest-sha256",
)
PATH_OPTIONS = frozenset({
    name for name in OPTION_ORDER
    if name not in {
        "--verify-existing", "--source-manifest-sha256",
        "--expected-bootstrap-completion-sha256", "--expected-git-sha256",
        "--expected-ssh-keygen-sha256", "--expected-allowed-signers-sha256",
        "--expected-revocation-sha256", "--expected-signer-fingerprint",
        "--expected-scaling-trial-harness-sha256",
        "--expected-scaling-configuration-sha256",
        "--expected-scaling-irohad-sha256",
        "--expected-scaling-iroha-cli-sha256",
    }
})

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
    "g4p-completion",
    "g12-seed-completion",
    "g12-fault-soak-completion",
    "scaling-evidence-manifest",
    "sdk-dependency-archive",
    "sdk-dependency-input-inventory",
    "sdk-dependency-final-work-inventory",
    "runtime-tool-probe-manifest",
    "runtime-tool-probe-result",
    "expected-scaling-trial-harness-sha256",
    "expected-scaling-configuration-sha256",
    "expected-scaling-irohad-sha256",
    "expected-scaling-iroha-cli-sha256",
    "repository-root",
):
    parser.add_argument(f"--{option}", required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--verify-existing", action="store_true")
parser.add_argument("--replay-existing", action="store_true")
parser.add_argument("--validation-ack", type=Path)
parser.add_argument("--source-manifest-sha256")
args = parser.parse_args()
if args.verify_existing == args.replay_existing:
    raise SystemExit(48)
if not args.output.is_file() or args.output.is_symlink():
    raise SystemExit(41)
receipt_metadata = args.output.stat()
if stat.S_IMODE(receipt_metadata.st_mode) != 0o400 or receipt_metadata.st_nlink != 1:
    raise SystemExit(47)
for path in (
    args.g4p_completion,
    args.g12_seed_completion,
    args.g12_fault_soak_completion,
    args.scaling_evidence_manifest,
    args.sdk_dependency_archive,
    args.sdk_dependency_input_inventory,
    args.sdk_dependency_final_work_inventory,
    args.runtime_tool_probe_result,
    *(() if args.replay_existing else (args.runtime_tool_probe_manifest,)),
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
release_invocation = release_output.parent
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
for path, name in (
    (args.sdk_dependency_archive, "sdk-dependency-bundle.tar"),
    (args.sdk_dependency_input_inventory, "sdk-dependency-input.json"),
    (args.sdk_dependency_final_work_inventory, "sdk-dependency-work-final.json"),
    (args.runtime_tool_probe_result, "runtime-tool-probe-result.json"),
    *((
        (args.runtime_tool_probe_manifest, "runtime-tool-probe-manifest.json"),
    ) if not args.replay_existing else ()),
):
    if Path(path) != release_invocation / name:
        raise SystemExit(49)
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
receipt = args.output.resolve(strict=True)
action = "verified" if args.verify_existing else "replayed"
stdout = f"Sumeragi v2 aggregate release receipt {action}: {receipt}\n".encode()
if (args.validation_ack is None) != (args.source_manifest_sha256 is None):
    raise SystemExit(46)
if args.validation_ack is not None:
    values = {}
    for name in OPTION_ORDER:
        attribute = name[2:].replace("-", "_")
        value = getattr(args, attribute)
        kind = "flag" if name == "--verify-existing" else "path" if name in PATH_OPTIONS else "text"
        if kind == "path":
            value = os.path.abspath(os.path.normpath(str(value)))
        values[name] = (kind, value)
    bindings = [
        {
            "name": name,
            "value_kind": values[name][0],
            "normalized_value_sha256": hashlib.sha256(
                json.dumps(
                    {"kind": values[name][0], "value": values[name][1]},
                    ensure_ascii=False, sort_keys=True, separators=(",", ":"),
                ).encode()
            ).hexdigest(),
        }
        for name in OPTION_ORDER
    ]
    invocation_core = {
        "profile": "release",
        "operation": "verify-existing-and-ack",
        "python_flags": ["-I", "-S"],
        "validator": "protected:validate-receipt.py",
        "ordered_options": bindings,
    }
    invocation = {
        **invocation_core,
        "invocation_sha256": hashlib.sha256(
            json.dumps(
                invocation_core, ensure_ascii=False, sort_keys=True,
                separators=(",", ":"),
            ).encode()
        ).hexdigest(),
    }
    receipt_metadata = receipt.stat()
    validator = Path(__file__).resolve(strict=True)
    completion = Path(args.bootstrap_completion).resolve(strict=True)
    acknowledgment = {
        "format": "iroha-sumeragi-v2-receipt-validation-ack",
        "schema_version": 3,
        "profile": "release",
        "sealed_source": {
            "archive_id": "release-retained.source.v1",
            "manifest_sha256": args.source_manifest_sha256,
        },
        "receipt": {
            "archive_id": "release-terminal.receipt.v1",
            "mode": f"{stat.S_IMODE(receipt_metadata.st_mode):04o}",
            "sha256": hashlib.sha256(receipt.read_bytes()).hexdigest(),
            "size_bytes": receipt_metadata.st_size,
        },
        "validator": {
            "archive_id": "release-bootstrap.receipt-validator.v1",
            "sha256": hashlib.sha256(validator.read_bytes()).hexdigest(),
            "bootstrap_completion_sha256": hashlib.sha256(
                completion.read_bytes()
            ).hexdigest(),
        },
        "invocation": invocation,
        "exit_status": 0,
        "stdout": {
            "sha256": hashlib.sha256(stdout).hexdigest(),
            "size_bytes": len(stdout),
        },
        "stderr": {
            "sha256": hashlib.sha256(b"").hexdigest(),
            "size_bytes": 0,
        },
    }
    payload = (
        json.dumps(acknowledgment, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()
    with args.validation_ack.open("xb") as stream:
        stream.write(payload)
        stream.flush()
        os.fchmod(stream.fileno(), 0o400)
        os.fsync(stream.fileno())
print(stdout.decode(), end="")
'''
    if mutation:
        source = source.replace(
            'print(stdout.decode(), end="")',
            mutation + '\nprint(stdout.decode(), end="")',
        )
    return source


def _identity_verifier(
    *,
    mutate_path: Path | None = None,
    mutate_candidate: bool = False,
    attestation_schema: int = 3,
    transcript_schema: int = 3,
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
parser.add_argument("--original-git-path", type=Path, required=True)
parser.add_argument("--expected-git-sha256", required=True)
parser.add_argument("--ssh-keygen-bin", type=Path, required=True)
parser.add_argument("--original-ssh-keygen-path", type=Path, required=True)
parser.add_argument("--expected-ssh-keygen-sha256", required=True)
parser.add_argument("--expected-signer-fingerprint", required=True)
parser.add_argument("--ssh-allowed-signers", type=Path, required=True)
parser.add_argument("--original-ssh-allowed-signers-path", type=Path, required=True)
parser.add_argument("--expected-ssh-allowed-signers-sha256", required=True)
parser.add_argument("--ssh-revocation-file", type=Path, required=True)
parser.add_argument("--original-ssh-revocation-path", type=Path, required=True)
parser.add_argument("--expected-ssh-revocation-sha256", required=True)
parser.add_argument("--attestation-output", type=Path, required=True)
parser.add_argument("--bootstrap-private-provenance-output", type=Path, required=True)
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
raw = (
    f"tree {{identity['head_tree']}}\\n"
    "author Release Fixture <release@example.test> 1700000000 +0000\\n"
    "committer Release Fixture <release@example.test> 1700000000 +0000\\n"
    "gpgsig -----BEGIN SSH SIGNATURE-----\\n"
    " Zml4dHVyZQ==\\n"
    " -----END SSH SIGNATURE-----\\n\\n"
    "Fixture release\\n\\n"
    "Sumeragi-V2-Release-Identity-Version: 1\\n"
    f"Sumeragi-V2-Source-Manifest-SHA256: {{identity['workspace_source_manifest_sha256']}}\\n"
    f"Sumeragi-V2-Cargo-Lock-SHA256: {{identity['cargo_lock_sha256']}}\\n"
).encode()
lock = (args.root / "Cargo.lock").read_bytes()

archive_ids = {{
    "cargo_lock": "release-identity.cargo-lock.v1",
    "git": "release-identity.git.v1",
    "raw_commit": "release-identity.raw-commit.v1",
    "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
    "ssh_keygen": "release-identity.ssh-keygen.v1",
    "ssh_revocation": "release-identity.ssh-revocation.v1",
    "verify_transcript": "release-identity.verify-transcript.v1",
}}
archive_names = {{
    "cargo_lock": args.cargo_lock_output.name,
    "git": args.git_archive_output.name,
    "raw_commit": args.raw_commit_output.name,
    "ssh_allowed_signers": args.ssh_allowed_signers_output.name,
    "ssh_keygen": args.ssh_keygen_archive_output.name,
    "ssh_revocation": args.ssh_revocation_output.name,
    "verify_transcript": args.verify_transcript_output.name,
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
def operation(operation_id, exit_status):
    empty_digest = digest(b"")
    return {{
        "operation_id": operation_id,
        "exit_status": exit_status,
        "stdout_sha256": empty_digest,
        "stdout_size_bytes": 0,
        "stderr_sha256": empty_digest,
        "stderr_size_bytes": 0,
    }}
transcript = {{
    "format": "iroha-sumeragi-v2-release-identity-transcript",
    "schema_version": {transcript_schema},
    "archive_ids": archive_ids,
    "candidate_commit_oid": identity["head_commit"],
    "operations": {{
        "show_signature_metadata": operation(
            "git.show-signature-metadata.ssh.v1", 0
        ),
        "verify_commit": operation("git.verify-commit.ssh.v1", 0),
        "ssh_keygen_usage": operation("ssh-keygen.usage-probe.v1", 1),
    }},
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
        "archive_id": archive_ids[label],
        "mode": f"{{mode:04o}}",
        "sha256": ("0" * 64 if {bad_evidence_digest!r} and label == "raw_commit" else digest(data)),
        "size_bytes": len(data),
    }}
attestation = {{
    "format": "iroha-sumeragi-v2-release-identity-attestation",
    "schema_version": {attestation_schema},
    "candidate": {{
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": digest(identity_bytes),
    }},
    "archives": evidence,
}}
private_directory = args.bootstrap_private_provenance_output.parent
def private_archive_path(label):
    return str(private_directory / ("." + archive_names[label] + ".stage." + "1" * 32))
def private_tool(label, data, expected, source):
    return {{
        "archive_id": archive_ids[label],
        "archive_path": private_archive_path(label),
        "mode": "0500",
        "observed_sha256": digest(data),
        "protected_sha256": expected,
        "size_bytes": len(data),
        "source_path": str(source),
    }}
def private_policy(label, data, expected, source):
    return {{
        "archive_id": archive_ids[label],
        "archive_path": private_archive_path(label),
        "mode": "0400",
        "observed_sha256": digest(data),
        "protected_sha256": expected,
        "size_bytes": len(data),
        "source_path": str(source),
    }}
private_provenance = {{
    "format": "iroha-sumeragi-v2-release-identity-bootstrap-private-provenance",
    "schema_version": 1,
    "candidate": {{
        "root_path": str(args.root),
        "identity_source_path": str(args.identity),
        "cargo_lock_source_path": str(args.root / "Cargo.lock"),
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": digest(identity_bytes),
    }},
    "outputs": {{
        "attestation": str(args.attestation_output),
        "bootstrap-private provenance": str(args.bootstrap_private_provenance_output),
        "verify transcript": str(args.verify_transcript_output),
        "raw commit": str(args.raw_commit_output),
        "Cargo.lock archive": str(args.cargo_lock_output),
        "SSH allowed-signers archive": str(args.ssh_allowed_signers_output),
        "SSH revocation-policy archive": str(args.ssh_revocation_output),
        "Git archive": str(args.git_archive_output),
        "ssh-keygen archive": str(args.ssh_keygen_archive_output),
    }},
    "archive_names": archive_names,
    "tools": {{
        "git": private_tool("git", git, args.expected_git_sha256, args.original_git_path),
        "ssh_keygen": private_tool("ssh_keygen", ssh, args.expected_ssh_keygen_sha256, args.original_ssh_keygen_path),
    }},
    "policies": {{
        "expected_signer_fingerprint": args.expected_signer_fingerprint,
        "signature_format": "ssh",
        "ssh_allowed_signers": private_policy("ssh_allowed_signers", allowed, args.expected_ssh_allowed_signers_sha256, args.original_ssh_allowed_signers_path),
        "ssh_revocation": private_policy("ssh_revocation", revocation, args.expected_ssh_revocation_sha256, args.original_ssh_revocation_path),
    }},
    "verification": {{
        "signature_format": "ssh",
        "status": "G",
        "signer_fingerprint": args.expected_signer_fingerprint,
        "primary_key_fingerprint": "",
        "allowed_signers_principal": "release",
    }},
    "execution": {{
        "environment": {{"HOME": str(private_directory)}},
        "policy_overrides": [],
        "replay": {{}},
        "commands": {{
            "show_signature_metadata": command_record(["git", "show"], 0),
            "verify_commit": command_record(["git", "verify-commit"], 0),
        }},
        "tool_probes": {{
            "ssh_keygen_usage": command_record(["ssh-keygen", "-?"], 1),
        }},
    }},
    "sanitized_transcript": evidence["verify_transcript"],
}}
publish(args.bootstrap_private_provenance_output, canonical(private_provenance), 0o400)
publish(args.attestation_output, canonical(attestation), 0o400)
{mutation}'''


def _isolated_python_action(source: str, *arguments: Path) -> str:
    """Render one runner-fixture action for the protected archived Python."""

    rendered_arguments = "".join(
        f" {shlex.quote(str(argument))}" for argument in arguments
    )
    return (
        f"python3 -I -S -{rendered_arguments} <<'PY'\n"
        f"{source.rstrip()}\n"
        "PY"
    )


def _runner(
    launch_count: Path,
    candidate: Path,
    action: str,
    *,
    trusted_mutation: Path | None = None,
    observed_scaling_environment: Path | None = None,
    receipt_mutation_override: str | None = None,
) -> str:
    retained_root = launch_count.parent / "release-runner"
    sdk_fixture_files, sdk_fixture_records, sdk_fixture_bindings = (
        _sdk_dependency_fixture_material()
    )
    sdk_fixture_work_records = [
        {"path": ".", "kind": "directory", "mode": "0700"}
    ]
    actions = {
        "success": ":",
        "slow-success": _isolated_python_action(
            "import time\n"
            "time.sleep(21)"
        ),
        "unlisted-command": "iroha-unlisted-release-command",
        "fail": "exit 37",
        "source-drift": f"printf drift > {candidate / 'payload'}",
        "evidence-tamper": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "target = Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION\"])\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b\"tamper\")"
        ),
        "marker-tamper": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "target = Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION\"])\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b\"tamper\")"
        ),
        "directory-mode-tamper": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR\"]).chmod(0o755)"
        ),
        "receipt-support-archive-substitution": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "target = Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR\"]) / \"sumeragi_v2_localnet_manifest.py\"\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b\"substituted\")"
        ),
        "receipt-support-archive-omission": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "root = Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR\"])\n"
            "(root / \"sumeragi_v2_localnet_manifest.py\").replace(root / \"omitted-localnet-manifest.py\")"
        ),
        "fail-and-tamper": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "target = Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION\"])\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b\"tamper\")\n"
            "raise SystemExit(37)"
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
        action_script = _isolated_python_action(
            "from pathlib import Path\n"
            "import sys\n"
            "target = Path(sys.argv[1])\n"
            "target.chmod(0o700)\n"
            "target.write_bytes(b\"mutated\")",
            trusted_mutation,
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
import io
import json
import os
from pathlib import Path
import tarfile

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
signer_fingerprint = os.environ[
    "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT"
]
allowed_signers_principal = "release"
completion_sha256 = hashlib.sha256(marker_bytes).hexdigest()
assert completion_sha256 == os.environ["SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"]
release_runner = Path({str(retained_root)!r})
release_root = release_runner / "source"
release_directory = release_runner / "output" / "release"
for directory in (release_runner, release_root, release_runner / "target", release_runner / "output", release_directory):
    directory.mkdir(mode=0o700, exist_ok=True)
    directory.chmod(0o700)
runner = marker["runner"]
bootstrap_runner = {{
    "archive_id": runner["archive_id"],
    "sha256": runner["sha256"],
    "mode": runner["mode"],
    "invocation": runner["invocation"],
    "closed_path_resolution": runner["closed_path_resolution"],
    "output": runner["output"],
    "tool_directory": runner["tool_directory"],
    "tools": runner["tools"],
    "environment_sha256": runner["environment_sha256"],
    "self_digest_environment_variables": runner["self_digest_environment_variables"],
}}
candidate = Path({str(candidate)!r})
for relative, mode in (
    ("Cargo.lock", 0o400),
    ("payload", 0o400),
    ("scripts/run_sumeragi_v2_release_gates.sh", 0o500),
    ("scripts/run_sumeragi_v2_release_gates_support.sh", 0o400),
    (
        "scripts/copy_sumeragi_v2_release_cargo_cache_validation_ack.py",
        0o400,
    ),
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

def archive(path, archive_id, **extra):
    metadata = path.stat()
    return {{
        "archive_id": archive_id,
        "mode": f"{{metadata.st_mode & 0o7777:04o}}",
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "size_bytes": metadata.st_size,
        **extra,
    }}

def evidence_file(directory, name, data, mode=0o400):
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    path = directory / name
    path.write_bytes(data)
    path.chmod(mode)
    return path

for name in ("home", "tmp", "cache", "cargo-home"):
    directory = release_runner / "output" / name
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
cargo_cache_input = evidence_file(
    release_runner / "output", "cargo-cache-input.json", b'{{"input":true}}\\n'
)
cargo_cache_final = evidence_file(
    release_runner / "output", "cargo-cache-final.json", b'{{"final":true}}\\n'
)
runtime_inventory = evidence_file(
    release_runner, "runtime-input.json", b'{{"runtime":true}}\\n'
)
runtime_probe_manifest = evidence_file(
    release_runner,
    "runtime-tool-probe-manifest.json",
    canonical({{"schema_version": 1, "tools": {{}}}}),
)
runtime_probe_value = json.loads(
    canonical(
        marker["trusted_execution_probes"]["runner_tool_closure"]["value"]
    )
)
for name, record in runtime_probe_value["tools"].items():
    record["archive_id"] = f"release-runtime-tool.{{name}}.v1"
runtime_probe_result = evidence_file(
    release_runner,
    "runtime-tool-probe-result.json",
    canonical(runtime_probe_value),
)
sdk_files = {sdk_fixture_files!r}
sdk_records = {sdk_fixture_records!r}
sdk_bindings = {sdk_fixture_bindings!r}
sdk_work_records = {sdk_fixture_work_records!r}
sdk_archive = release_runner / "sdk-dependency-bundle.tar"
with tarfile.open(sdk_archive, mode="w", format=tarfile.PAX_FORMAT) as sdk_tar:
    for record in sdk_records:
        relative = record["path"]
        member = tarfile.TarInfo(
            "sdk-inputs" if relative == "." else f"sdk-inputs/{{relative}}"
        )
        member.mode = int(record["mode"], 8)
        member.uid = member.gid = member.mtime = 0
        member.uname = member.gname = ""
        if record["kind"] == "directory":
            member.type = tarfile.DIRTYPE
            sdk_tar.addfile(member)
        else:
            data = sdk_files[relative]
            member.size = len(data)
            sdk_tar.addfile(member, io.BytesIO(data))
sdk_archive.chmod(0o400)
sdk_archive_record = archive(
    sdk_archive, "release-sdk-dependencies.bundle.v1",
    archive_name=sdk_archive.name,
)
sdk_input_inventory = evidence_file(
    release_runner,
    "sdk-dependency-input.json",
    canonical({{
        "format": "iroha-sumeragi-v2-sdk-dependency-bundle",
        "schema_version": 1,
        "archive_id": "release-sdk-dependencies.bundle.v1",
        "source_disclosure": "withheld",
        "source_manifest_sha256": marker["trusted_inputs"]
            ["sdk_dependency_bundle_manifest"]["sha256"],
        "source_state_sha256": "e" * 64,
        "bindings": sdk_bindings,
        "archive": sdk_archive_record,
        "record_count": len(sdk_records),
        "file_bytes": sum(
            record.get("size", 0) for record in sdk_records
        ),
        "records": sdk_records,
        "work_initial_record_count": len(sdk_work_records),
        "work_initial_file_bytes": sum(
            record.get("size", 0) for record in sdk_work_records
        ),
        "work_initial_records": sdk_work_records,
    }}),
)
sdk_final_work_inventory = evidence_file(
    release_runner,
    "sdk-dependency-work-final.json",
    canonical({{
        "format": "iroha-sumeragi-v2-sdk-dependency-work-final",
        "schema_version": 1,
        "archive_id": "release-sdk-dependencies.work-final.v1",
        "sdk_dependency_inventory_sha256": hashlib.sha256(
            sdk_input_inventory.read_bytes()
        ).hexdigest(),
        "record_count": len(sdk_work_records),
        "file_bytes": sum(
            record.get("size", 0) for record in sdk_work_records
        ),
        "records": sdk_work_records,
    }}),
)
sdk_evidence = {{
    "schema_version": 1,
    "source_disclosure": "withheld",
    "source_manifest_sha256": marker["trusted_inputs"]
        ["sdk_dependency_bundle_manifest"]["sha256"],
    "source_state_sha256": "e" * 64,
    "archive": sdk_archive_record,
    "input_inventory": archive(
        sdk_input_inventory,
        "release-sdk-dependencies.input-inventory.v1",
        archive_name=sdk_input_inventory.name,
    ),
    "final_work_inventory": archive(
        sdk_final_work_inventory,
        "release-sdk-dependencies.work-final.v1",
        archive_name=sdk_final_work_inventory.name,
    ),
}}
runtime_environment = {{
    "runtime_home_path": str(release_runner / "output" / "home"),
    "runtime_tmpdir_path": str(release_runner / "output" / "tmp"),
    "runtime_tmp_path": str(release_runner / "output" / "tmp"),
    "runtime_temp_path": str(release_runner / "output" / "tmp"),
    "runtime_cache_path": str(release_runner / "output" / "cache"),
}}
cargo_cache = {{
    "schema_version": 2,
    "inventory": archive(
        cargo_cache_input, "release-cargo-cache.input-inventory.v1"
    ),
    "final_inventory": archive(
        cargo_cache_final, "release-cargo-cache.final-inventory.v1"
    ),
    "runtime_inventory": archive(
        runtime_inventory, "release-runtime.inventory.v1"
    ),
    "runtime_environment_sha256": hashlib.sha256(
        canonical(runtime_environment)
    ).hexdigest(),
    "runtime_directories": {{
        name: {{
            "archive_id": f"release-runtime.directory.{{name}}.v1",
            "mode": "0700",
        }}
        for name in ("cache", "home", "tmp")
    }},
    "cargo_home": {{
        "archive_id": "release-cargo-cache.home.v1",
        "mode": "0700",
    }},
    "source_cargo_home_disclosure": "withheld",
    "input_root_count": 0,
    "input_record_count": 0,
    "input_file_count": 0,
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
        archive(
            binary,
            f"release-prebuilt.binary.{{role}}.v1",
            role=role,
            relative_path=relative,
        )
    )
cargo_version = b"cargo 1.0.0\\n"
rustc_version = b"rustc 1.0.0\\n"
cargo_tool = evidence / runner["tools"]["cargo"]["archive_name"]
rustc_tool = evidence / runner["tools"]["rustc"]["archive_name"]
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
    "schema_version": 3,
    "archive_id": "release-prebuilt.bundle.v1:invocation.test",
    "manifest": archive(prebuilt_manifest, "release-prebuilt.manifest.v2"),
    "source_manifest_sha256": identity["workspace_source_manifest_sha256"],
    "cargo_lock_sha256": identity["cargo_lock_sha256"],
    "cargo_version_sha256": hashlib.sha256(cargo_version).hexdigest(),
    "rustc_version_sha256": hashlib.sha256(rustc_version).hexdigest(),
    "host_triple": "aarch64-apple-darwin",
    "target_triple": "aarch64-apple-darwin",
    "profile": "release",
    "version_transcripts": {{
        "cargo": {{
            "operation_id": "cargo.version.v1",
            "tool_archive_id": "release-runner-tool.cargo.v1",
            "sha256": hashlib.sha256(cargo_version).hexdigest(),
            "size_bytes": len(cargo_version),
        }},
        "rustc": {{
            "operation_id": "rustc.version.v1",
            "tool_archive_id": "release-runner-tool.rustc.v1",
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
    archive(
        path,
        "release-scaling.file.v1:" + path.relative_to(scaling_root).as_posix(),
        relative_path=path.relative_to(scaling_root).as_posix(),
    )
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
    "retained_tooling": [
        archive(
            release_root / source_path,
            f"release-scaling.retained-tool.{{role}}.v1",
            role=role,
        )
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
    "git_sha256": marker["trusted_inputs"]["git"]["sha256"],
    "ssh_keygen_sha256": marker["trusted_inputs"]["ssh_keygen"]["sha256"],
    "allowed_signers_sha256": marker["trusted_inputs"]["allowed_signers"]["sha256"],
    "revocation_sha256": marker["trusted_inputs"]["revocation"]["sha256"],
    "signer_fingerprint": signer_fingerprint,
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
            "schema_version": 2,
            "completion_sha256": completion_sha256,
            "frozen_bootstrap_sha256": marker["trusted_inputs"]["bootstrap"]["sha256"],
            "candidate_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
            "candidate_commit_oid": identity["head_commit"],
            "candidate_tree_oid": identity["head_tree"],
            "runner": bootstrap_runner,
            "signer_fingerprint": signer_fingerprint,
            "allowed_signers_principal": allowed_signers_principal,
            "trusted_input_digests": {{
                label: record["sha256"]
                for label, record in marker["trusted_inputs"].items()
            }},
            "trusted_input_archives": {{
                label: {{
                    key: record[key]
                    for key in ("archive_id", "mode", "sha256", "size_bytes")
                }}
                for label, record in marker["trusted_inputs"].items()
            }},
            "release_approvals": {{
                "archive_id": "release-approval.set-attestation.v1",
                "sha256": marker["release_approvals"]["set_attestation"]["sha256"],
                "operation_plan_sha256": marker["release_approvals"][
                    "operation_plan_sha256"
                ],
            }},
        }},
        "release_identity": {{
            "schema_version": 1,
            "signature_format": "ssh",
            "verification_status": "G",
            "candidate_commit_oid": identity["head_commit"],
            "candidate_tree_oid": identity["head_tree"],
            "signer_fingerprint": signer_fingerprint,
            "primary_key_fingerprint": "",
            "allowed_signers_principal": allowed_signers_principal,
            "trust_policy": trust_policy,
            "replay": {{
                "performed": True,
                "archive_ids": {{
                    "cargo_lock": "release-identity.cargo-lock.v1",
                    "git": "release-identity.git.v1",
                    "raw_commit": "release-identity.raw-commit.v1",
                    "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
                    "ssh_keygen": "release-identity.ssh-keygen.v1",
                    "ssh_revocation": "release-identity.ssh-revocation.v1",
                    "verify_transcript": "release-identity.verify-transcript.v1",
                }},
            }},
        }},
    }},
    "evidence": {{
        "bootstrap": {{
            "completion": {{}},
            "candidate_identity": {{}},
            "runner": {{}},
            "candidate_cargo_lock": {{}},
            "trusted_inputs": {{
                label: (
                    {{
                        key: record[key]
                        for key in (
                            "archive_id", "mode", "sha256", "size_bytes"
                        )
                    }}
                    if label == "sdk_dependency_bundle_manifest"
                    else {{}}
                )
                for label, record in marker["trusted_inputs"].items()
            }},
            "identity_verification": {{
                "identity_attestation": {{}},
                "identity_transcript": {{}},
            }},
            "runner_tools": {{label: {{}} for label in runner["tools"]}},
            "release_approvals": marker["release_approvals"],
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
        "cargo_cache_input": cargo_cache,
        "cargo_cache_input_inventory": cargo_cache["inventory"],
        "cargo_cache_final_inventory": cargo_cache["final_inventory"],
        "sdk_dependencies": sdk_evidence,
        "runtime_tool_probes": {{
            "format": runtime_probe_value["format"],
            "schema_version": 1,
            "host_family": runtime_probe_value["host_family"],
            "probe_contract_sha256": runtime_probe_value[
                "probe_contract_sha256"
            ],
            "tool_count": 41,
            "result": archive(
                runtime_probe_result,
                "release-runtime.tool-probes.v1",
                archive_name="runtime-tool-probe-result.json",
            ),
        }},
        "prebuilt_binary_bundle": prebuilt_bundle,
        "formal_gate_log": {{}},
        "formal_proof_coverage": {{}},
        "formal_proof_evidence": {{}},
        "formal_multilane_apalache_evidence": artifact(formal_apalache),
        "formal_production_trace_extraction_evidence": {{}},
        "formal_harness_lock": {{}},
        "formal_toolchain": {{}},
        "formal_tlaps_resource_jsonl": artifact(formal_resource_jsonl),
        "formal_tlaps_resource_summary": artifact(formal_resource_summary),
        "seed_matrix_summary": {{}},
        "seed_matrix_run_logs": [],
        "seed_matrix_localnet_manifest_index": {{}},
        "seed_matrix_localnet_manifests": [],
        "chaos_log": {{}},
        "multilane_scaling_bundle": {{
            "archive_id": "release-scaling.bundle.v1",
            "file_count": len(scaling_files),
            "total_size_bytes": sum(record["size_bytes"] for record in scaling_files),
            "directories": ["runs", "runs/pair-00"],
            "files": scaling_files,
        }},
        "multilane_scaling_retained_validator": archive(
            retained_scaling_validator,
            "release-scaling.retained-validator.v1",
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
    validation_script = f'''
release_runner={shlex.quote(str(retained_root))}
release_output="$release_runner/output"
release_receipt="$release_output/release/RELEASE_COMPLETED.json"
release_ack="$release_runner/receipt-validation-ack.json"
source_manifest_sha256="$(python3 -I -S -c 'import json,sys;print(json.load(open(sys.argv[1], encoding="utf-8"))["workspace_source_manifest_sha256"])' "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY")"
set +e
python3 -I -S "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/validate-receipt.py" \
  --candidate-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY" \
  --sealed-identity "$release_runner/sealed-identity.json" \
  --release-root "$release_runner/source" \
  --bootstrap-completion "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION" \
  --bootstrap-evidence-dir "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" \
  --bootstrap-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY" \
  --bootstrap-attestation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION" \
  --bootstrap-transcript "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT" \
  --expected-bootstrap-completion-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256" \
  --bootstrap-candidate-root {shlex.quote(str(candidate))} \
  --bootstrap-runner {shlex.quote(str(candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"))} \
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
  --corridor-completion "$release_output/mock-completions/corridor_completion.tsv" \
  --formal-completion "$release_output/mock-completions/formal_completion.tsv" \
  --seed-completion "$release_output/mock-completions/seed_matrix_completion.tsv" \
  --chaos-completion "$release_output/mock-completions/chaos_completion.tsv" \
  --g4p-completion "$release_output/g4p/COMPLETED.tsv" \
  --g12-seed-completion "$release_output/g12-seed/COMPLETED.tsv" \
  --g12-fault-soak-completion "$release_output/g12-soak/COMPLETED.tsv" \
  --scaling-evidence-manifest "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
  --sdk-dependency-archive "$release_runner/sdk-dependency-bundle.tar" \
  --sdk-dependency-input-inventory "$release_runner/sdk-dependency-input.json" \
  --sdk-dependency-final-work-inventory "$release_runner/sdk-dependency-work-final.json" \
  --runtime-tool-probe-manifest "$release_runner/runtime-tool-probe-manifest.json" \
  --runtime-tool-probe-result "$release_runner/runtime-tool-probe-result.json" \
  --expected-scaling-trial-harness-sha256 "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
  --expected-scaling-configuration-sha256 "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
  --expected-scaling-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
  --expected-scaling-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \
  --repository-root "$release_runner/source" \
  --output "$release_receipt" \
  --verify-existing \
  --validation-ack "$release_ack" \
  --source-manifest-sha256 "$source_manifest_sha256" \
  >"$release_runner/receipt-validator.stdout" \
  2>"$release_runner/receipt-validator.stderr"
validator_status=$?
set -e
if ((validator_status != 0)); then
  python3 -I -S "$IROHA_RELEASE_RUNTIME_HELPER" \
    --publish-validation-failure \
    --invocation-root "$release_runner" \
    --bootstrap-evidence "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" \
    --cleanup-base {shlex.quote(str(retained_root.parent))} \
    --cleanup-prefix release-runner \
    --source-manifest-sha256 "$source_manifest_sha256" \
    --validator-exit-status "$validator_status"
  exit 74
fi
python3 -I -S "$IROHA_RELEASE_RUNTIME_HELPER" \
  --seal-release-result \
  --invocation-root "$release_runner" \
  --bootstrap-evidence "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" \
  --source-manifest-sha256 "$source_manifest_sha256" \
  --candidate-root {shlex.quote(str(candidate))} \
  --scaling-evidence-manifest "$IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST" \
  --expected-signer-fingerprint "$SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT" \
  --expected-scaling-trial-harness-sha256 "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
  --expected-scaling-configuration-sha256 "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
  --expected-scaling-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
  --expected-scaling-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256"
'''
    post_receipt_action = {
        "receipt-tamper": _isolated_python_action(
            "from pathlib import Path\n"
            "import sys\n"
            "target = Path(sys.argv[1])\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b\"tamper\")",
            retained_root / "output/release/RELEASE_COMPLETED.json",
        ),
        "receipt-wrong-mode": _isolated_python_action(
            "from pathlib import Path\n"
            "import sys\n"
            "Path(sys.argv[1]).chmod(0o600)",
            retained_root / "output/release/RELEASE_COMPLETED.json",
        ),
        "receipt-hardlink": _isolated_python_action(
            "import os\n"
            "import sys\n"
            "os.link(sys.argv[1], sys.argv[2])",
            retained_root / "output/release/RELEASE_COMPLETED.json",
            retained_root / "output/release/receipt-alias",
        ),
        "receipt-symlink": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "import sys\n"
            "receipt = Path(sys.argv[1])\n"
            "receipt.replace(sys.argv[2])\n"
            "os.symlink(\"receipt-target\", receipt)",
            retained_root / "output/release/RELEASE_COMPLETED.json",
            retained_root / "output/release/receipt-target",
        ),
        "receipt-wrong-path": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "import sys\n"
            "destination = Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR\"]) / \"RELEASE_COMPLETED.json\"\n"
            "Path(sys.argv[1]).replace(destination)",
            retained_root / "output/release/RELEASE_COMPLETED.json",
        ),
        "preexisting-postmarker": _isolated_python_action(
            "import os\n"
            "from pathlib import Path\n"
            "target = Path(os.environ[\"SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR\"]) / \"BOOTSTRAP_RELEASE_COMPLETED.json\"\n"
            "target.write_bytes(b\"attacker\")\n"
            "target.chmod(0o400)"
        ),
    }.get(action, ":")
    if action in {"missing-receipt", "unlisted-command"}:
        receipt_script = ":"
        validation_script = ":"
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
: "${{IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST:?}}"
: "${{IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256:?}}"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT"
test -f "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY"
test "$IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST" = \
  "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/sdk-dependency-bundle-manifest.json"
count=0
if test -f {launch_count}; then count=$(<{launch_count}); fi
count=$((count + 1))
printf '%s\n' "$count" > {launch_count}
{environment_probe}
{receipt_script}
{post_receipt_action}
{action_script}
{validation_script}
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
    runtime_helper: Path
    runtime_helper_cli: Path
    tool_probe_helper: Path
    approval_contract: Path
    approvals: dict[str, Path]
    sdk_manifest: Path
    tool_manifest: Path
    git: Path
    ssh: Path
    bash: Path
    allowed: Path
    revocation: Path

    @property
    def retained_root(self) -> Path:
        return self.root / "release-runner"

    def install_planned_runner(self, source: str | bytes) -> None:
        """Install a fixture runner and rebind only its identity-bound approvals."""
        _write(
            self.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh",
            source,
            0o500,
        )
        identity = _candidate_release_identity(self.candidate, self.manifest)
        approvals = _approval_fixture_files(
            contract=self.approval_contract,
            trust=self.trust,
            identity=identity,
            tool_manifest_sha256=_sha256(self.tool_manifest),
        )
        assert set(approvals) == set(self.approvals)
        self.approvals = approvals

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
            "--runtime-helper", str(self.runtime_helper),
            "--expected-runtime-helper-sha256", _sha256(self.runtime_helper),
            "--runtime-helper-cli", str(self.runtime_helper_cli),
            "--expected-runtime-helper-cli-sha256", _sha256(self.runtime_helper_cli),
            "--tool-probe-helper", str(self.tool_probe_helper),
            "--expected-tool-probe-helper-sha256",
            _sha256(self.tool_probe_helper),
            "--approval-contract", str(self.approval_contract),
            "--expected-approval-contract-sha256",
            _sha256(self.approval_contract),
            "--offline-toolchain-sdk-approval",
            str(self.approvals["offline-toolchain-sdk"]),
            "--formal-proof-tools-approval",
            str(self.approvals["formal-proof-tools"]),
            "--network-scale-soak-approval",
            str(self.approvals["network-scale-soak"]),
            "--final-bootstrap-publication-approval",
            str(self.approvals["final-bootstrap-publication"]),
            "--approval-evidence-root-id", APPROVAL_EVIDENCE_ROOT_ID,
            "--offline-toolchain-sdk-duration-seconds",
            str(APPROVAL_DURATIONS[0]),
            "--formal-proof-tools-duration-seconds",
            str(APPROVAL_DURATIONS[1]),
            "--network-scale-soak-duration-seconds",
            str(APPROVAL_DURATIONS[2]),
            "--final-bootstrap-publication-duration-seconds",
            str(APPROVAL_DURATIONS[3]),
            "--sdk-dependency-bundle-manifest", str(self.sdk_manifest),
            "--expected-sdk-dependency-bundle-manifest-sha256",
            _sha256(self.sdk_manifest),
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
                self.retained_root
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

    def run(
        self,
        arguments: list[str] | None = None,
        *,
        timeout_seconds: float = 30,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            arguments or self.arguments(),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout_seconds,
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
    _tool_support.provision_future_archived_python_runtime(PYTHON, root)
    _write(candidate / "Cargo.lock", b"locked\n")
    _write(candidate / "payload", b"candidate\n")
    _write(
        candidate
        / "scripts"
        / "copy_sumeragi_v2_release_cargo_cache_validation_ack.py",
        (
            REPO_ROOT
            / "scripts"
            / "copy_sumeragi_v2_release_cargo_cache_validation_ack.py"
        ).read_bytes(),
        0o400,
    )
    _write(
        candidate / "scripts" / "run_sumeragi_v2_release_gates_support.sh",
        (
            REPO_ROOT / "scripts" / "run_sumeragi_v2_release_gates_support.sh"
        ).read_bytes(),
        0o400,
    )
    manifest = _write(trust / "manifest.py", _manifest_helper(), 0o500)
    verifier = _write(trust / "verifier.py", _identity_verifier(), 0o500)
    receipt_validator = _write(
        trust / "receipt-validator.py", _receipt_validator(), 0o500
    )
    for component in (
        "write_sumeragi_v2_release_receipt_formal_artifacts.py",
        "write_sumeragi_v2_release_receipt_corridor_log.py",
        "write_sumeragi_v2_release_receipt_gate_evidence.py",
        "write_sumeragi_v2_release_receipt_publication.py",
    ):
        _write(
            trust / component,
            (REPO_ROOT / "scripts" / component).read_bytes(),
            0o400,
        )
    receipt_validator_support = _write(
        trust / RECEIPT_VALIDATOR_SUPPORT.name,
        RECEIPT_VALIDATOR_SUPPORT.read_bytes(),
        0o400,
    )
    runtime_helper = _write(
        trust / "runtime-helper.py",
        (REPO_ROOT / "scripts" / "copy_sumeragi_v2_release_cargo_cache.py").read_bytes(),
        0o400,
    )
    runtime_helper_cli = _write(
        trust / "copy_sumeragi_v2_release_cargo_cache_cli.py",
        (REPO_ROOT / "scripts" / "copy_sumeragi_v2_release_cargo_cache_cli.py").read_bytes(),
        0o400,
    )
    tool_probe_helper = _write(
        trust / "tool-probe-helper.py",
        _tool_support.fixture_tool_probe_helper(),
        0o400,
    )
    approval_contract = _write(
        trust / "release-approval-contract.py",
        APPROVAL_CONTRACT.read_bytes(),
        0o400,
    )
    tool_manifest = _write(
        trust / "runner-tool-manifest.json", _tool_support.runner_tool_manifest(trust), 0o400
    )
    git = _write(trust / "git", "#!/bin/sh\nexit 0\n", 0o500)
    sdk_manifest = _write(
        trust / "sdk-dependency-bundle-manifest.json",
        _sdk_source_manifest_fixture(git),
        0o400,
    )
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
    identity = _candidate_release_identity(candidate, manifest)
    approvals = _approval_fixture_files(
        contract=approval_contract,
        trust=trust,
        identity=identity,
        tool_manifest_sha256=_sha256(tool_manifest),
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
        runtime_helper,
        runtime_helper_cli,
        tool_probe_helper,
        approval_contract,
        approvals,
        sdk_manifest,
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
    marker = json.loads(marker_path.read_text(encoding="utf-8"))
    framework_python = (
        label == "python"
        and source == PYTHON
        and sys.platform == "darwin"
        and isinstance(sysconfig.get_config_var("PYTHONFRAMEWORK"), str)
        and bool(sysconfig.get_config_var("PYTHONFRAMEWORK"))
    )
    runtime_record: dict[str, object] | None = None
    if framework_python:
        legacy_archive = evidence_directory / archive_name
        if legacy_archive.exists():
            legacy_archive.unlink()
        runtime_root = evidence_directory / "python-runtime"
        inventory_path = evidence_directory / "python-runtime-input.json"
        copied = subprocess.run(
            [
                str(source),
                "-I",
                "-S",
                str(
                    REPO_ROOT
                    / "scripts"
                    / "copy_sumeragi_v2_release_cargo_cache.py"
                ),
                "--copy-framework-python",
                "--runtime-root",
                str(runtime_root),
                "--runtime-inventory",
                str(inventory_path),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        assert copied.returncode == 0, copied.stderr
        archive_name = "python-runtime/bin/python3"
        archive = runtime_root / "bin" / "python3"
        private_inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
        records = []
        for record in private_inventory["records"]:
            records.append(
                {
                    key: value
                    for key, value in record.items()
                    if key not in {"device", "inode"}
                }
            )
        runtime_record = {
            "format": "iroha-sumeragi-v2-framework-python-runtime",
            "schema_version": 2,
            "archive_root": "python-runtime",
            "root_mode": "0500",
            "executable": "bin/python3",
            "inventory": {
                "archive_name": "python-runtime-input.json",
                "mode": "0400",
                "sha256": _sha256(inventory_path),
                "size_bytes": inventory_path.stat().st_size,
            },
            "record_count": len(records),
            "file_bytes": sum(
                int(record["size"])
                for record in records
                if record["kind"] == "file"
            ),
            "records": records,
            "relocation": private_inventory["relocation"],
        }
        probe_code = "import sys;sys.stdout.write(sys.executable+'\\n')"
        probe = subprocess.run(
            [str(archive), "-I", "-S", "-c", probe_code],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        assert probe.returncode == 0, probe.stderr.decode("utf-8", "replace")
        assert probe.stdout == f"{archive}\n".encode()
        assert not probe.stderr
        environment = evidence.get("bootstrap_runner_environment")
        assert isinstance(environment, dict)
        path_entries = environment["PATH"].split(os.pathsep)
        path_entries.insert(1, str(archive.parent))
        environment["PATH"] = os.pathsep.join(dict.fromkeys(path_entries))
        marker["runner"]["environment_sha256"] = hashlib.sha256(
            (json.dumps(environment, sort_keys=True, separators=(",", ":")) + "\n").encode()
        ).hexdigest()
    else:
        archive = _write(
            evidence_directory / archive_name,
            source.read_bytes(),
            archive_mode,
        )
    if label == "receipt_validator":
        components: dict[str, object] = {}
        for component_name in (
            "write_sumeragi_v2_release_receipt_corridor_log.py",
            "write_sumeragi_v2_release_receipt_formal_artifacts.py",
            "write_sumeragi_v2_release_receipt_gate_evidence.py",
            "write_sumeragi_v2_release_receipt_publication.py",
        ):
            component_source = source.with_name(component_name)
            assert component_source.is_file()
            component_archive = _write(
                evidence_directory / component_name,
                component_source.read_bytes(),
                0o400,
            )
            components[component_name] = {
                "archive_id": (
                    "release-bootstrap.receipt-validator-component.v1:"
                    + component_name
                ),
                "archive_name": component_name,
                "mode": "0400",
                "sha256": _sha256(component_archive),
                "size_bytes": component_archive.stat().st_size,
            }
    digest = _sha256(archive if framework_python else source)
    source_metadata = (archive if framework_python else source).stat()
    runtime_name = {"python": "python3", "bash": "bash", "git": "git"}.get(label)
    if runtime_name is not None:
        runtime_tools = evidence.get("runtime_tool_probe_tools")
        corridor_completion = evidence.get("corridor_completion")
        assert isinstance(runtime_tools, dict)
        assert isinstance(corridor_completion, Path)
        runtime_tool = runtime_tools[runtime_name]
        assert isinstance(runtime_tool, Path)
        runtime_tool.chmod(0o700)
        runtime_tool.write_bytes(source.read_bytes())
        runtime_tool.chmod(0o500)
        completion_lines = corridor_completion.read_text(
            encoding="utf-8"
        ).splitlines()
        digest_field = "python3_sha256" if label == "python" else f"{label}_sha256"
        completion_lines = [
            f"{digest_field}\t{digest}" if line.startswith(f"{digest_field}\t")
            else line
            for line in completion_lines
        ]
        corridor_completion.chmod(0o600)
        corridor_completion.write_text(
            "\n".join(completion_lines) + "\n", encoding="utf-8"
        )
        corridor_completion.chmod(0o400)
    marker["trusted_inputs"][label] = {
        "archive_id": f"release-bootstrap.{label.replace('_', '-')}.v1",
        "archive_name": archive_name,
        "mode": f"{archive_mode:04o}",
        "sha256": digest,
        "size_bytes": source_metadata.st_size,
    }
    if runtime_record is not None:
        marker["trusted_inputs"][label]["runtime"] = runtime_record
    if label == "receipt_validator":
        marker["trusted_inputs"][label]["components"] = components
    if label == "python":
        probe_code = "import sys;sys.stdout.write(sys.executable+'\\n')"
        probe_stdout = f"{archive}\n".encode()
        marker["trusted_execution_probes"]["python"] = {
            "argv": [str(archive), "-I", "-S", "-c", probe_code],
            "expected_executable": archive_name,
            "exit_status": 0,
            "stdout_sha256": hashlib.sha256(probe_stdout).hexdigest(),
            "stdout_size_bytes": len(probe_stdout),
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
    assert value["schema_version"] == 2
    assert value["trust_boundary"] == {
        "bootstrap_authentication": "external prerequisite",
        "release_image_and_dynamic_loader": "external prerequisite",
        "same_uid_and_trusted_ancestor_owners": True,
    }
    assert value["runner"]["invocation"] == {
        "profile": "release",
        "operation_id": "sumeragi-v2.release.v1",
        "arguments": ["--release"],
        "bash_archive_id": "release-bootstrap.bash.v1",
    }
    assert value["runner"]["closed_path_resolution"] == {
        "bash": "release-bootstrap.bash.v1",
        "git": "release-bootstrap.git.v1",
        "python3": "release-bootstrap.python.v1",
    }
    assert re.fullmatch(r"[0-9a-f]{64}", value["runner"]["environment_sha256"])
    assert value["runner"]["self_digest_environment_variables"] == [
        "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
        "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
    ]
    support_record = value["trusted_inputs"]["receipt_validator_support"]
    assert support_record["archive_name"] == RECEIPT_VALIDATOR_SUPPORT.name
    assert support_record["sha256"] == _sha256(
        release_fixture.receipt_validator_support
    )
    bootstrap_components = value["trusted_inputs"]["bootstrap"]["components"]
    assert set(bootstrap_components) == {
        path.name for path in BOOTSTRAP_COMPONENTS
    }
    assert all(
        bootstrap_components[path.name]["sha256"] == _sha256(path)
        for path in BOOTSTRAP_COMPONENTS
    )
    receipt_components = value["trusted_inputs"]["receipt_validator"][
        "components"
    ]
    assert set(receipt_components) == {
        "write_sumeragi_v2_release_receipt_formal_artifacts.py",
        "write_sumeragi_v2_release_receipt_corridor_log.py",
        "write_sumeragi_v2_release_receipt_gate_evidence.py",
        "write_sumeragi_v2_release_receipt_publication.py",
    }
    assert all(
        str(path).encode("utf-8") not in data
        for path in (
            BOOTSTRAP,
            *BOOTSTRAP_COMPONENTS,
            release_fixture.receipt_validator,
        )
    )
    approvals = value["release_approvals"]
    assert set(approvals["class_attestations"]) == set(
        release_fixture.approvals
    )
    assert approvals["expected_duration_seconds"] == dict(
        zip(release_fixture.approvals, APPROVAL_DURATIONS)
    )
    assert b'"arguments"' not in _fixture_canonical(approvals)
    assert _sha256(release_fixture.evidence / RECEIPT_VALIDATOR_SUPPORT.name) == (
        support_record["sha256"]
    )
    assert stat.S_IMODE(release_fixture.evidence.stat().st_mode) == 0o700
    assert not any(
        path.name.startswith(".component-private.")
        for path in release_fixture.evidence.iterdir()
    )
    assert stat.S_IMODE(marker.stat().st_mode) == 0o400
    receipt = (
        release_fixture.retained_root
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
        "schema_version": 2,
        "result": "release-complete",
        "bootstrap_completion_sha256": hashlib.sha256(data).hexdigest(),
        "candidate_identity_sha256": _sha256(
            release_fixture.evidence / "candidate-identity.json"
        ),
        "candidate_commit_oid": value["candidate_identity"]["head_commit"],
        "candidate_tree_oid": value["candidate_identity"]["head_tree"],
        "release_approvals": {
            "archive_id": "release-approval.set-attestation.v1",
            "sha256": value["release_approvals"]["set_attestation"]["sha256"],
            "operation_plan_sha256": value["release_approvals"][
                "operation_plan_sha256"
            ],
        },
        "runner": {
            "archive_id": "release-candidate.runner.v1",
            "sha256": value["runner"]["sha256"],
            "mode": value["runner"]["mode"],
            "logs": {
                label: {
                    "archive_id": f"release-bootstrap.runner-{label}.v1",
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
            "archive_id": "release-retained.source.v1",
            "identity_archive_id": "release-retained.identity.v1",
            "identity_sha256": _sha256(
                release_fixture.retained_root / "sealed-identity.json"
            ),
            "source_manifest_sha256": value["candidate_identity"][
                "workspace_source_manifest_sha256"
            ],
            "mode": "0500",
        },
        "receipt_validator": {
            "archive_id": "release-bootstrap.receipt-validator.v1",
            "sha256": _sha256(release_fixture.receipt_validator),
            "exit_status": 0,
            "ack_archive_id": "release-retained.receipt-validation-ack.v3",
            "ack_sha256": _sha256(
                release_fixture.evidence / "receipt-validation-ack.json"
            ),
        },
        "terminal_receipt": {
            "archive_id": "release-terminal.receipt.v1",
            "sha256": _sha256(receipt),
            "size_bytes": receipt.stat().st_size,
            "mode": "0400",
        },
    }
    terminal_documents = (
        external_data,
        (release_fixture.evidence / "receipt-validation-ack.json").read_bytes(),
        (release_fixture.evidence / "release-runner-result.json").read_bytes(),
        (release_fixture.evidence / "release-retained-inventory.json").read_bytes(),
        receipt.read_bytes(),
    )
    for canary in (
        release_fixture.candidate,
        release_fixture.trust,
        release_fixture.root / "caller-cargo-home-canary",
        release_fixture.root / "caller-rustup-home-canary",
        release_fixture.root / "original-scaling-source-canary",
        release_fixture.trust / "original-tool-path-canary",
    ):
        assert all(str(canary).encode() not in payload for payload in terminal_documents)
    result_document = json.loads(terminal_documents[2])
    inventory_document = json.loads(terminal_documents[3])
    assert result_document["schema_version"] == 2
    assert inventory_document["schema_version"] == 2
    assert "invocation_root" not in result_document
    assert "source_root" not in result_document
    assert "invocation_root" not in inventory_document
    assert "source_root" not in inventory_document
    assert not (
        release_fixture.evidence / "release-runner-private-provenance.json"
    ).exists()
    assert stat.S_IMODE(external_marker.stat().st_mode) == 0o400
    assert external_marker.stat().st_nlink == 1
    assert not any(
        child.name.startswith(".BOOTSTRAP_") and ".stage." in child.name
        for child in release_fixture.evidence.iterdir()
    )


_execute_bootstrap_test_component(RELEASE_BOOTSTRAP_TEST_COMPONENT_FILES[0])


def test_undeclared_runner_tool_has_no_ambient_path_fallback(
    release_fixture: Fixture,
) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "unlisted-command",
        ),
    )

    result = release_fixture.run()

    assert result.returncode == 127
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


_execute_bootstrap_test_component(RELEASE_BOOTSTRAP_TEST_COMPONENT_FILES[1])
