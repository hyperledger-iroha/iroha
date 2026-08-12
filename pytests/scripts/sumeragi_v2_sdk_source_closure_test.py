"""Fail-closed tests for the shared Sumeragi SDK source-closure resolver."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any

import pytest


ROOT = Path(__file__).resolve().parents[2]
RESOLVER = ROOT / "ci" / "resolve_sumeragi_v2_sdk_source_closure.py"
MANIFEST = ROOT / "ci" / "sumeragi_v2_sdk_source_closure.json"
NATIVE_HARNESS = ROOT / "ci" / "run_native_amx_v2_grouped_sdk_parity.sh"
DIAGNOSTICS_HARNESS = ROOT / "ci" / "run_sumeragi_v2_sdk_diagnostics.sh"
NATIVE_FIXTURE = ROOT / "fixtures" / "sumeragi_v2" / "native_amx_v2_grouped.json"
WIRE_FIXTURE = ROOT / "fixtures" / "sumeragi_v2" / "wire_v2.tsv"


def _load_resolver():
    spec = importlib.util.spec_from_file_location("sdk_source_closure", RESOLVER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _canonical_json(document: dict[str, Any]) -> str:
    return json.dumps(document, ensure_ascii=True, indent=2, sort_keys=True) + "\n"


def _fixture_manifest() -> dict[str, Any]:
    return {
        "closure_roots": [
            {
                "extensions": [".js"],
                "group": "production",
                "path": "sdk/javascript",
                "recursive": True,
            },
            {
                "extensions": [".py"],
                "group": "production",
                "path": "sdk/python",
                "recursive": True,
            },
        ],
        "format": "iroha-sumeragi-v2-sdk-production-source-closure",
        "groups": {
            "closure-resolver": [
                "ci/resolve_sumeragi_v2_sdk_source_closure.py",
                "ci/sumeragi_v2_sdk_source_closure.json",
            ],
            "diagnostics-suite": [
                "ci/run_sumeragi_v2_sdk_diagnostics.sh",
                "fixtures/sumeragi_v2/native_amx_v2_grouped.json",
                "fixtures/sumeragi_v2/wire_v2.tsv",
            ],
            "native-suite": [
                "ci/run_native_amx_v2_grouped_sdk_parity.sh",
            ],
            "production": [
                "sdk/javascript/client.js",
                "sdk/python/client.py",
            ],
        },
        "suites": {
            "native-amx-v2-grouped": [
                "closure-resolver",
                "native-suite",
                "production",
            ],
            "sumeragi-v2-sdk-diagnostics": [
                "closure-resolver",
                "diagnostics-suite",
                "production",
            ],
        },
        "version": 1,
    }


def _git(root: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def _source_fixture(tmp_path: Path) -> tuple[Path, dict[str, Any]]:
    (tmp_path / "ci").mkdir()
    (tmp_path / "fixtures" / "sumeragi_v2").mkdir(parents=True)
    (tmp_path / "sdk" / "javascript").mkdir(parents=True)
    (tmp_path / "sdk" / "python").mkdir(parents=True)
    shutil.copyfile(
        RESOLVER,
        tmp_path / "ci" / "resolve_sumeragi_v2_sdk_source_closure.py",
    )
    (tmp_path / "ci" / "run_native_amx_v2_grouped_sdk_parity.sh").write_text(
        "#!/usr/bin/env bash\n",
        encoding="utf-8",
    )
    (tmp_path / "ci" / "run_sumeragi_v2_sdk_diagnostics.sh").write_text(
        "#!/usr/bin/env bash\n",
        encoding="utf-8",
    )
    shutil.copyfile(
        NATIVE_FIXTURE,
        tmp_path / "fixtures" / "sumeragi_v2" / "native_amx_v2_grouped.json",
    )
    shutil.copyfile(
        WIRE_FIXTURE,
        tmp_path / "fixtures" / "sumeragi_v2" / "wire_v2.tsv",
    )
    (tmp_path / "sdk" / "javascript" / "client.js").write_text(
        "export const height = 1n;\n",
        encoding="utf-8",
    )
    (tmp_path / "sdk" / "python" / "client.py").write_text(
        "HEIGHT = 1\n",
        encoding="utf-8",
    )
    document = _fixture_manifest()
    manifest = tmp_path / "ci" / "sumeragi_v2_sdk_source_closure.json"
    manifest.write_text(_canonical_json(document), encoding="utf-8")
    _git(tmp_path, "init", "--quiet")
    _git(tmp_path, "add", "--all")
    return manifest, document


def _run_resolver(
    root: Path,
    *arguments: str,
) -> subprocess.CompletedProcess[str]:
    environment = {
        **os.environ,
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONHASHSEED": "0",
    }
    return subprocess.run(
        [
            sys.executable,
            str(root / "ci" / "resolve_sumeragi_v2_sdk_source_closure.py"),
            "--root",
            str(root),
            *arguments,
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
    )


def _native_digest(root: Path) -> subprocess.CompletedProcess[str]:
    return _run_resolver(
        root,
        "--suite",
        "native-amx-v2-grouped",
        "--manifest-sha256",
    )


def _diagnostics_digest(root: Path) -> subprocess.CompletedProcess[str]:
    return _run_resolver(
        root,
        "--suite",
        "sumeragi-v2-sdk-diagnostics",
        "--manifest-sha256",
    )


def test_resolver_emits_sorted_paths_records_and_stable_content_digest(
    tmp_path: Path,
) -> None:
    _source_fixture(tmp_path)
    paths = _run_resolver(
        tmp_path,
        "--suite",
        "native-amx-v2-grouped",
        "--print-paths",
    )
    assert paths.returncode == 0, paths.stderr
    observed_paths = paths.stdout.splitlines()
    assert observed_paths == sorted(observed_paths)
    assert len(observed_paths) == len(set(observed_paths)) == 5

    records = _run_resolver(
        tmp_path,
        "--suite",
        "native-amx-v2-grouped",
        "--print-records",
    )
    assert records.returncode == 0, records.stderr
    expected_manifest = hashlib.sha256(records.stdout.encode("utf-8")).hexdigest()
    first = _native_digest(tmp_path)
    second = _native_digest(tmp_path)
    assert first.returncode == second.returncode == 0
    assert first.stdout == second.stdout == f"{expected_manifest}\n"

    (tmp_path / "sdk" / "python" / "client.py").write_text(
        "HEIGHT = 2\n",
        encoding="utf-8",
    )
    dirty_first = _native_digest(tmp_path)
    dirty_second = _native_digest(tmp_path)
    assert dirty_first.returncode == dirty_second.returncode == 0
    assert dirty_first.stdout == dirty_second.stdout
    assert dirty_first.stdout != first.stdout


def test_wire_fixture_drift_rotates_only_diagnostics_suite_digest(
    tmp_path: Path,
) -> None:
    grouped_records = _run_resolver(
        ROOT,
        "--suite",
        "native-amx-v2-grouped",
        "--print-records",
    )
    assert grouped_records.returncode == 0, grouped_records.stderr
    assert len(grouped_records.stdout.splitlines()) == 1_380

    _source_fixture(tmp_path)
    grouped_before = _native_digest(tmp_path)
    diagnostics_before = _diagnostics_digest(tmp_path)
    assert grouped_before.returncode == diagnostics_before.returncode == 0

    wire_fixture = tmp_path / "fixtures" / "sumeragi_v2" / "wire_v2.tsv"
    wire_fixture.write_bytes(wire_fixture.read_bytes() + b"# tracked TSV drift\n")

    grouped_after = _native_digest(tmp_path)
    diagnostics_after = _diagnostics_digest(tmp_path)
    assert grouped_after.returncode == diagnostics_after.returncode == 0
    assert grouped_after.stdout == grouped_before.stdout
    assert diagnostics_after.stdout != diagnostics_before.stdout


def test_resolver_rejects_noncanonical_or_unsorted_manifest(tmp_path: Path) -> None:
    manifest, document = _source_fixture(tmp_path)
    document["groups"]["production"].reverse()
    manifest.write_text(_canonical_json(document), encoding="utf-8")
    result = _native_digest(tmp_path)
    assert result.returncode == 1
    assert "strictly sorted and duplicate-free" in result.stderr
    assert result.stdout == ""

    document["groups"]["production"].sort()
    manifest.write_text(json.dumps(document), encoding="utf-8")
    result = _native_digest(tmp_path)
    assert result.returncode == 1
    assert "canonical sorted two-space JSON" in result.stderr


def test_resolver_rejects_missing_or_untracked_declared_input(tmp_path: Path) -> None:
    manifest, document = _source_fixture(tmp_path)
    (tmp_path / "sdk" / "python" / "client.py").unlink()
    missing = _native_digest(tmp_path)
    assert missing.returncode == 1
    assert "source-closure path is missing" in missing.stderr

    (tmp_path / "sdk" / "python" / "client.py").write_text(
        "HEIGHT = 1\n",
        encoding="utf-8",
    )
    untracked_path = "sdk/python/status_models.py"
    (tmp_path / untracked_path).write_text("STATUS = 1\n", encoding="utf-8")
    document["groups"]["production"].append(untracked_path)
    document["groups"]["production"].sort()
    manifest.write_text(_canonical_json(document), encoding="utf-8")
    untracked = _native_digest(tmp_path)
    assert untracked.returncode == 1
    assert f"source-closure input is untracked: {untracked_path}" in untracked.stderr


@pytest.mark.parametrize("tracked", (False, True))
def test_resolver_rejects_unexpected_production_input(
    tmp_path: Path,
    tracked: bool,
) -> None:
    _source_fixture(tmp_path)
    unexpected_path = tmp_path / "sdk" / "javascript" / "status.js"
    unexpected_path.write_text("export const status = {};\n", encoding="utf-8")
    if tracked:
        _git(tmp_path, "add", "sdk/javascript/status.js")
    result = _native_digest(tmp_path)
    assert result.returncode == 1
    expected_kind = "tracked" if tracked else "untracked"
    assert (
        f"unexpected {expected_kind} input: sdk/javascript/status.js"
        in result.stderr
    )


def test_resolver_rejects_symlinked_source_path(tmp_path: Path) -> None:
    _source_fixture(tmp_path)
    javascript_path = tmp_path / "sdk" / "javascript" / "client.js"
    javascript_path.unlink()
    try:
        javascript_path.symlink_to(Path("../python/client.py"))
    except OSError as error:
        pytest.skip(f"symlinks are unavailable: {error}")
    result = _native_digest(tmp_path)
    assert result.returncode == 1
    assert "traverses a symlink" in result.stderr


def test_production_manifest_exactly_covers_declared_source_roots() -> None:
    module = _load_resolver()
    manifest = module._manifest_from_bytes(MANIFEST.read_bytes())
    discovered_by_group = {}
    for closure_root in manifest.closure_roots:
        discovered = module._discover_root_paths(ROOT, closure_root)
        discovered_by_group.setdefault(closure_root.group, set()).update(discovered)
    for group, discovered in discovered_by_group.items():
        assert discovered == set(manifest.groups[group])
    all_paths = {
        path.as_posix()
        for paths in manifest.groups.values()
        for path in paths
    }
    required_omissions_closed = {
        "python/iroha_torii_client/client_status_models.py",
        "python/iroha_torii_client/kaigi_relay_client.py",
        "python/iroha_torii_client/connect_session.py",
        "javascript/iroha_js/src/browser.js",
        "javascript/iroha_js/src/networkId.d.ts",
        "javascript/iroha_js/src/strictLosslessJson.js",
        "javascript/iroha_js/src/sumeragiTyped.js",
        "javascript/iroha_js/src/toriiBrowserClient.js",
        "javascript/iroha_js/src/toriiClientPrimitives.js",
        "IrohaSwift/Sources/IrohaSwift/SumeragiV2Wire.swift",
        "IrohaSwift/Sources/IrohaSwift/ToriiStatusModels.swift",
        "IrohaSwift/Sources/IrohaSwift/ToriiSumeragiModels.swift",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/NativeAmxV2.kt",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/"
        "SumeragiDiagnosticsModels.kt",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/consensus/SumeragiV2Wire.kt",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/"
        "transport/BoundedResponseBodyReader.java",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/"
        "NativeAmxV2Models.java",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/"
        "SumeragiDiagnosticsModels.java",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/consensus/"
        "SumeragiV2Wire.java",
    }
    assert required_omissions_closed <= all_paths
    assert {
        module.PurePosixPath(
            "crates/connect_norito_bridge/src/platform_jni.rs"
        ),
        module.PurePosixPath(
            "crates/connect_norito_bridge/src/platform_jni/part_1.rs"
        ),
        module.PurePosixPath(
            "crates/connect_norito_bridge/src/platform_jni/part_2.rs"
        ),
        module.PurePosixPath(
            "crates/connect_norito_bridge/src/platform_jni/part_3.rs"
        ),
    } <= set(manifest.groups["native-amx-v2-grouped-suite"])
    assert {
        module.PurePosixPath(
            "IrohaSwift/Tests/IrohaSwiftTests/SumeragiV2WireFixtureTests.swift"
        ),
        module.PurePosixPath(
            "python/iroha_torii_client/tests/sumeragi_exact_json_test_support.py"
        ),
        module.PurePosixPath(
            "fixtures/sumeragi_v2/native_amx_v2_grouped.json"
        ),
        module.PurePosixPath("fixtures/sumeragi_v2/wire_v2.tsv"),
        module.PurePosixPath(
            "javascript/iroha_js/test/sumeragiBrowserFixtures.js"
        ),
    } <= set(manifest.groups["diagnostics-suite"])
    swift_fixture_support = "native-amx-v2-swift-fixture-support"
    assert set(manifest.groups[swift_fixture_support]) == {
        module.PurePosixPath(
            "IrohaSwift/Tests/IrohaSwiftTests/NativeAmxV2GroupedFixtureTests.swift"
        )
    }
    assert swift_fixture_support in manifest.suites["native-amx-v2-grouped"]
    assert swift_fixture_support in manifest.suites["sumeragi-v2-sdk-diagnostics"]
    assert set(manifest.groups["openapi-current-artifacts"]) == {
        module.PurePosixPath("artifacts/openapi/manifest.json"),
        module.PurePosixPath("artifacts/openapi/torii.json"),
        module.PurePosixPath("artifacts/openapi/versions.json"),
        module.PurePosixPath("artifacts/openapi/versions/current/manifest.json"),
        module.PurePosixPath("artifacts/openapi/versions/current/torii.json"),
    }


def test_both_harnesses_consume_only_the_shared_closure_resolver() -> None:
    expected = (
        (
            NATIVE_HARNESS,
            "native-amx-v2-grouped",
        ),
        (
            DIAGNOSTICS_HARNESS,
            "sumeragi-v2-sdk-diagnostics",
        ),
    )
    for harness, suite in expected:
        source = harness.read_text(encoding="utf-8")
        assert "source_paths=(" not in source
        assert source.count("resolve_sumeragi_v2_sdk_source_closure.py") == 1
        assert source.count("sumeragi_v2_sdk_source_closure.json") == 1
        assert source.count(f'--suite "{suite}"') == 1
        assert source.count("--manifest-sha256") == 1
