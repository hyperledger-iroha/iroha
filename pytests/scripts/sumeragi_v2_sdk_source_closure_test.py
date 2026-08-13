"""Fail-closed tests for the shared Sumeragi SDK source-closure resolver."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
from typing import Any
import zipfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
RESOLVER = ROOT / "ci" / "resolve_sumeragi_v2_sdk_source_closure.py"
MANIFEST = ROOT / "ci" / "sumeragi_v2_sdk_source_closure.json"
NATIVE_HARNESS = ROOT / "ci" / "run_native_amx_v2_grouped_sdk_parity.sh"
DIAGNOSTICS_HARNESS = ROOT / "ci" / "run_sumeragi_v2_sdk_diagnostics.sh"
NATIVE_FIXTURE = ROOT / "fixtures" / "sumeragi_v2" / "native_amx_v2_grouped.json"
WIRE_FIXTURE = ROOT / "fixtures" / "sumeragi_v2" / "wire_v2.tsv"
RELEASE_HELPER = ROOT / "scripts" / "copy_sumeragi_v2_release_cargo_cache.py"
RELEASE_RUNNER = ROOT / "scripts" / "run_sumeragi_v2_release_gates.sh"


def _load_resolver():
    spec = importlib.util.spec_from_file_location("sdk_source_closure", RESOLVER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _load_release_helper():
    spec = importlib.util.spec_from_file_location(
        "sdk_dependency_release_helper", RELEASE_HELPER
    )
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
    environment = os.environ.copy()
    if root.resolve() != ROOT:
        environment.pop("GIT_INDEX_FILE", None)
    return subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
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
    if root.resolve() != ROOT:
        environment.pop("GIT_INDEX_FILE", None)
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


def _check_regeneration(
    root: Path,
    suite: str,
    kind: str,
    first: Path | None,
    second: Path | None,
) -> subprocess.CompletedProcess[str]:
    arguments = [
        "--suite",
        suite,
        "--check-regeneration",
        kind,
    ]
    if first is not None:
        arguments.extend(("--first-output-root", str(first)))
    if second is not None:
        arguments.extend(("--second-output-root", str(second)))
    return _run_resolver(root, *arguments)


def _private_directory(path: Path) -> Path:
    path.mkdir(mode=0o700)
    path.chmod(0o700)
    return path


def _copy_regular_tree(source: Path, destination: Path) -> None:
    for candidate in sorted(source.rglob("*")):
        relative = candidate.relative_to(source)
        target = destination / relative
        if candidate.is_dir():
            target.mkdir(mode=0o700, parents=True, exist_ok=True)
        elif candidate.is_file():
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            shutil.copyfile(candidate, target)


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
    grouped_count = len(grouped_records.stdout.splitlines())
    assert grouped_count > 0

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


@pytest.mark.parametrize(
    ("kind", "suite", "source"),
    (
        (
            "rust-fixtures",
            "native-amx-v2-grouped",
            ROOT / "fixtures" / "sumeragi_v2",
        ),
        (
            "javascript",
            "native-amx-v2-grouped",
            ROOT / "javascript" / "iroha_js" / "src",
        ),
    ),
)
def test_two_regeneration_outputs_match_exact_checked_in_inventory(
    tmp_path: Path,
    kind: str,
    suite: str,
    source: Path,
) -> None:
    first = _private_directory(tmp_path / "first")
    second = _private_directory(tmp_path / "second")
    _copy_regular_tree(source, first)
    _copy_regular_tree(source, second)

    result = _check_regeneration(ROOT, suite, kind, first, second)

    assert result.returncode == 0, result.stderr
    summary = json.loads(result.stdout)
    assert summary["schema_version"] == 1
    assert summary["kind"] == kind
    assert summary["status"] == "byte-identical"
    assert summary["artifact_count"] == len(
        [path for path in source.rglob("*") if path.is_file()]
    )
    assert re.fullmatch(r"[0-9a-f]{64}", summary["artifact_manifest_sha256"])


def test_openapi_regeneration_requires_exact_five_artifacts_and_protected_input(
    tmp_path: Path,
) -> None:
    source = ROOT / "artifacts" / "openapi"
    first = _private_directory(tmp_path / "first")
    second = _private_directory(tmp_path / "second")
    _copy_regular_tree(source, first)
    _copy_regular_tree(source, second)

    result = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "openapi", first, second
    )

    assert result.returncode == 0, result.stderr
    summary = json.loads(result.stdout)
    assert summary["artifact_count"] == 5
    assert summary["kind"] == "openapi"
    assert summary["status"] == "byte-identical"


@pytest.mark.parametrize(
    ("kind", "relative"),
    (
        ("rust-fixtures", Path("wire_v2.tsv")),
        ("javascript", Path("toriiClient.js")),
        ("openapi", Path("versions.json")),
    ),
)
def test_regeneration_rejects_two_run_mismatch_and_stale_checked_in_bytes(
    tmp_path: Path,
    kind: str,
    relative: Path,
) -> None:
    sources = {
        "rust-fixtures": ROOT / "fixtures" / "sumeragi_v2",
        "javascript": ROOT / "javascript" / "iroha_js" / "src",
        "openapi": ROOT / "artifacts" / "openapi",
    }
    first = _private_directory(tmp_path / "first")
    second = _private_directory(tmp_path / "second")
    _copy_regular_tree(sources[kind], first)
    _copy_regular_tree(sources[kind], second)
    (second / relative).write_bytes((second / relative).read_bytes() + b"\n")

    mismatch = _check_regeneration(
        ROOT, "native-amx-v2-grouped", kind, first, second
    )
    assert mismatch.returncode == 1
    assert "regenerations disagree" in mismatch.stderr

    shutil.copyfile(first / relative, second / relative)
    for root in (first, second):
        (root / relative).write_bytes((root / relative).read_bytes() + b"\n")
    stale = _check_regeneration(
        ROOT, "native-amx-v2-grouped", kind, first, second
    )
    assert stale.returncode == 1
    assert "artifact is stale" in stale.stderr


def test_regeneration_rejects_missing_extra_symlink_and_nonregular_artifacts(
    tmp_path: Path,
) -> None:
    source = ROOT / "fixtures" / "sumeragi_v2"

    def outputs(label: str) -> tuple[Path, Path]:
        parent = _private_directory(tmp_path / label)
        first = _private_directory(parent / "first")
        second = _private_directory(parent / "second")
        _copy_regular_tree(source, first)
        _copy_regular_tree(source, second)
        return first, second

    first, second = outputs("missing")
    (first / "wire_v2.tsv").unlink()
    missing = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "rust-fixtures", first, second
    )
    assert missing.returncode == 1
    assert "missing=['wire_v2.tsv']" in missing.stderr

    first, second = outputs("extra")
    (first / "unexpected.txt").write_text("unexpected\n", encoding="utf-8")
    extra = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "rust-fixtures", first, second
    )
    assert extra.returncode == 1
    assert "unexpected=['unexpected.txt']" in extra.stderr

    first, second = outputs("symlink")
    (first / "wire_v2.tsv").unlink()
    try:
        (first / "wire_v2.tsv").symlink_to(NATIVE_FIXTURE)
    except OSError as error:
        pytest.skip(f"symlinks are unavailable: {error}")
    symlink = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "rust-fixtures", first, second
    )
    assert symlink.returncode == 1
    assert "non-regular or symlink artifact" in symlink.stderr

    first, second = outputs("nonregular")
    (first / "wire_v2.tsv").unlink()
    (first / "wire_v2.tsv").mkdir()
    nonregular = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "rust-fixtures", first, second
    )
    assert nonregular.returncode == 1
    assert "inventory is not exact" in nonregular.stderr


def test_regeneration_rejects_missing_overlapping_and_unsafe_output_roots(
    tmp_path: Path,
) -> None:
    first = _private_directory(tmp_path / "first")
    second = _private_directory(first / "second")

    missing = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "rust-fixtures", first, None
    )
    assert missing.returncode == 1
    assert "requires both --first-output-root and --second-output-root" in missing.stderr

    overlap = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "rust-fixtures", first, second
    )
    assert overlap.returncode == 1
    assert "must not overlap" in overlap.stderr

    relative = _check_regeneration(
        ROOT,
        "native-amx-v2-grouped",
        "rust-fixtures",
        Path("relative-output"),
        tmp_path,
    )
    assert relative.returncode == 1
    assert "must be an absolute path" in relative.stderr

    unsafe = tmp_path / "unsafe"
    unsafe.mkdir(mode=0o755)
    safe = _private_directory(tmp_path / "safe")
    unsafe_result = _check_regeneration(
        ROOT, "native-amx-v2-grouped", "rust-fixtures", unsafe, safe
    )
    assert unsafe_result.returncode == 1
    assert "owner-private" in unsafe_result.stderr


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
        "python/iroha_torii_client/orderbook_submission.py",
        "javascript/iroha_js/src/browser.js",
        "javascript/iroha_js/src/networkId.d.ts",
        "javascript/iroha_js/src/sorafsOrderbookSubmission.d.ts",
        "javascript/iroha_js/src/sorafsOrderbookSubmission.js",
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
        assert source.count(f'--suite "{suite}"') == 2
        assert source.count("--manifest-sha256") == 1
        assert source.count("--check-regeneration javascript") == 1
        assert source.count("--first-output-root") == 1
        assert source.count("--second-output-root") == 1


def test_release_sdk_harnesses_require_only_authenticated_private_dependencies() -> None:
    for harness in (NATIVE_HARNESS, DIAGNOSTICS_HARNESS):
        source = harness.read_text(encoding="utf-8")
        assert source.count('if [[ "${IROHA_RELEASE_SEALED_WORKTREE:-0}" == 1 ]]') == 1
        assert '${IROHA_RELEASE_INVOCATION_ROOT:-}/sdk-inputs' in source
        assert '${IROHA_RELEASE_INVOCATION_ROOT:-}/sdk-dependency-input.json' in source
        assert 'IROHA_RELEASE_SDK_WORK_PARENT' in source
        assert 'IROHA_RELEASE_SDK_WORK_HELPER' in source
        assert '--create-sdk-command-work' in source
        assert '--cleanup-sdk-command-work' in source
        assert 'sdk-command-work.' in source
        assert 'ln -s "$sdk_node_modules_root"' in source
        assert '--scratch-path "$sdk_swiftpm_work_root"' in source
        assert "--disable-automatic-resolution" in source
        assert "--only-use-versions-from-resolved-file" in source
        assert "--skip-update" in source
        assert "GIT_ALLOW_PROTOCOL=" in source
        assert "HTTPS_PROXY=http://127.0.0.1:9" in source
        assert source.count('GRADLE_USER_HOME="$sdk_gradle_user_home"') == 2
        assert 'ln -s "${javascript_sdk_root}/node_modules"' not in source
        assert '79n14ral3mx1ozqr3csh2u872' in source
        assert 'binding.get("launcher_archive_name")' in source
        assert 'gradle_command=("$sdk_gradle_launcher")' in source
        assert '"${gradle_command[@]}"' in source
        assert 'node "${javascript_staged_scripts_root}/build-dist.mjs"' in source
        assert 'node "${javascript_sdk_root}/scripts/build-dist.mjs"' not in source
        assert (
            '"${javascript_package_root_first}/test/' in source
            or 'javascript_test="${javascript_package_root_first}/test/' in source
        )
        assert 'node --test --test-reporter=tap "$javascript_test"' in source \
            or (
                'node --test --test-reporter=tap \\\n'
                '      "${javascript_package_root_first}/test/' in source
            )


def _sdk_dependency_fixture(
    tmp_path: Path,
) -> tuple[object, tuple[Path | str, ...], Path, Path]:
    helper = _load_release_helper()
    repository = _private_directory(tmp_path / "candidate")
    external = _private_directory(tmp_path / "operator-inputs")
    output = _private_directory(tmp_path / "private-release")
    tree = "b" * 40
    protected_git = external / "git"
    protected_git.write_text(
        f"#!{sys.executable}\n"
        "from pathlib import Path\n"
        "import sys\n"
        f"tree = {tree!r}\n"
        "args = sys.argv[1:]\n"
        "checkout = Path(args[args.index('-C') + 1])\n"
        "if 'rev-parse' in args:\n"
        "    print((checkout / '.git/HEAD').read_text(encoding='ascii').strip())\n"
        "    print(tree)\n"
        "elif 'status' not in args:\n"
        "    raise SystemExit(91)\n",
        encoding="utf-8",
    )
    protected_git.chmod(0o700)

    package_lock = {
        "lockfileVersion": 3,
        "name": "fixture",
        "packages": {
            "": {"name": "fixture", "version": "1.0.0"},
            "node_modules/example": {"version": "1.0.0"},
        },
        "requires": True,
        "version": "1.0.0",
    }
    installed_lock = {
        "lockfileVersion": 3,
        "name": "fixture",
        "packages": {"node_modules/example": {"version": "1.0.0"}},
        "requires": True,
        "version": "1.0.0",
    }
    package_lock_path = repository / "javascript/iroha_js/package-lock.json"
    package_lock_path.parent.mkdir(parents=True)
    package_lock_path.write_text(_canonical_json(package_lock), encoding="utf-8")
    node_modules = _private_directory(external / "node-modules")
    (node_modules / "node_modules/example").mkdir(parents=True)
    (node_modules / ".package-lock.json").write_text(
        _canonical_json(installed_lock), encoding="utf-8"
    )
    (node_modules / "node_modules/example/index.js").write_text(
        "export const value = 1;\n", encoding="utf-8"
    )

    revision = "1" * 40
    package_resolved = {
        "pins": [{"identity": "example", "state": {"revision": revision}}],
        "version": 2,
    }
    package_resolved_path = repository / "IrohaSwift/Package.resolved"
    package_resolved_path.parent.mkdir(parents=True)
    package_resolved_path.write_text(
        _canonical_json(package_resolved), encoding="utf-8"
    )
    swift_cache = _private_directory(external / "swiftpm-cache")
    (swift_cache / "checkouts/example/.git").mkdir(parents=True)
    (swift_cache / "repositories").mkdir()
    (swift_cache / "checkouts/example/.git/HEAD").write_text(
        f"{revision}\n", encoding="ascii"
    )
    (swift_cache / "checkouts/example/Sources").mkdir()
    (swift_cache / "checkouts/example/Sources/Example.swift").write_text(
        "public let fixtureValue = 1\n", encoding="utf-8"
    )

    wrapper_bytes = (
        "distributionBase=GRADLE_USER_HOME\n"
        "distributionPath=wrapper/dists\n"
        "distributionUrl=https\\://services.gradle.org/distributions/"
        "gradle-9.3.0-bin.zip\n"
        "networkTimeout=10000\n"
        "validateDistributionUrl=true\n"
        "zipStoreBase=GRADLE_USER_HOME\n"
        "zipStorePath=wrapper/dists\n"
    ).encode()
    for wrapper in (
        repository / "kotlin/gradle/wrapper/gradle-wrapper.properties",
        repository / "java/iroha_android/gradle/wrapper/gradle-wrapper.properties",
    ):
        wrapper.parent.mkdir(parents=True)
        wrapper.write_bytes(wrapper_bytes)
    distribution = external / "gradle-9.3.0-bin.zip"
    with zipfile.ZipFile(distribution, "w") as archive:
        archive.writestr("gradle-9.3.0/bin/gradle", b"#!/bin/sh\n")
        archive.writestr("gradle-9.3.0/lib/gradle-core.jar", b"fixture jar\n")
    gradle_home = _private_directory(external / "gradle-home")
    (gradle_home / "caches/9.3.0").mkdir(parents=True)
    (gradle_home / "caches/modules-2").mkdir()
    extracted = (
        gradle_home
        / "wrapper/dists/gradle-9.3.0-bin"
        / helper.SDK_GRADLE_WRAPPER_CACHE_KEY
        / "gradle-9.3.0"
    )
    (extracted / "bin").mkdir(parents=True)
    (extracted / "lib").mkdir()
    (extracted / "bin/gradle").write_bytes(b"#!/bin/sh\n")
    (extracted / "bin/gradle").chmod(0o700)
    (extracted / "lib/gradle-core.jar").write_bytes(b"fixture jar\n")
    (extracted.parent / "gradle-9.3.0-bin.zip.ok").write_bytes(b"")

    def source_inventory(path: Path) -> dict[str, Any]:
        records, file_bytes = helper._sdk_sanitized_snapshot(
            path, f"fixture source inventory {path.name}",
        )
        return {
            "file_bytes": file_bytes,
            "format": helper.SDK_SOURCE_INVENTORY_FORMAT,
            "record_count": len(records),
            "records": records,
            "records_sha256": helper._sdk_records_sha256(records),
            "schema_version": 1,
        }

    source_manifest = {
        "format": helper.SDK_SOURCE_FORMAT,
        "git": {
            "executable": str(protected_git),
            "sha256": hashlib.sha256(protected_git.read_bytes()).hexdigest(),
        },
        "gradle": {
            "distribution_archive": str(distribution),
            "distribution_sha256": hashlib.sha256(
                distribution.read_bytes()
            ).hexdigest(),
            "distribution_url": helper.SDK_GRADLE_DISTRIBUTION_URL,
            "gradle_user_home": str(gradle_home),
            "gradle_user_home_inventory": source_inventory(gradle_home),
            "java_wrapper_properties_sha256": hashlib.sha256(
                wrapper_bytes
            ).hexdigest(),
            "kotlin_wrapper_properties_sha256": hashlib.sha256(
                wrapper_bytes
            ).hexdigest(),
            "version": "9.3.0",
            "wrapper_cache_key": helper.SDK_GRADLE_WRAPPER_CACHE_KEY,
        },
        "node": {
            "node_modules_root": str(node_modules),
            "node_modules_inventory": source_inventory(node_modules),
            "package_lock_sha256": hashlib.sha256(
                package_lock_path.read_bytes()
            ).hexdigest(),
        },
        "schema_version": 2,
        "swiftpm": {
            "cache_root": str(swift_cache),
            "cache_inventory": source_inventory(swift_cache),
            "package_resolved_sha256": hashlib.sha256(
                package_resolved_path.read_bytes()
            ).hexdigest(),
            "resolved_revisions": [
                {
                    "checkout": "example",
                    "identity": "example",
                    "revision": revision,
                    "tree": tree,
                }
            ],
        },
    }
    manifest_path = tmp_path / "protected-sdk-source-manifest.json"
    manifest_path.write_bytes(helper._canonical_payload(source_manifest))
    manifest_path.chmod(0o400)
    manifest_digest = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    arguments: tuple[Path | str, ...] = (
        manifest_path,
        manifest_digest,
        repository,
        output / "sdk-inputs",
        output / "sdk-work",
        output / "sdk-dependency-bundle.tar",
        output / "sdk-dependency-input.json",
    )
    return helper, arguments, external, output


def test_sdk_dependency_bundle_withholds_paths_and_uses_new_inodes(
    tmp_path: Path,
) -> None:
    helper, arguments, external, output = _sdk_dependency_fixture(tmp_path)
    helper.copy_sdk_dependencies(*arguments)

    inventory_path = output / "sdk-dependency-input.json"
    inventory_bytes = inventory_path.read_bytes()
    inventory = json.loads(inventory_bytes)
    assert inventory["format"] == helper.SDK_BUNDLE_FORMAT
    assert inventory["source_disclosure"] == "withheld"
    assert inventory["archive"] == {
        "archive_id": helper.SDK_BUNDLE_ARCHIVE_ID,
        "archive_name": "sdk-dependency-bundle.tar",
        "mode": "0400",
        "sha256": hashlib.sha256(
            (output / "sdk-dependency-bundle.tar").read_bytes()
        ).hexdigest(),
        "size_bytes": (output / "sdk-dependency-bundle.tar").stat().st_size,
    }
    assert os.fsencode(str(external)) not in inventory_bytes
    assert os.fsencode(str(tmp_path / "candidate")) not in inventory_bytes
    archive_bytes = (output / "sdk-dependency-bundle.tar").read_bytes()
    assert os.fsencode(str(external)) not in archive_bytes
    assert os.fsencode(str(tmp_path / "candidate")) not in archive_bytes
    assert stat.S_IMODE((output / "sdk-inputs").stat().st_mode) == 0o500
    assert stat.S_IMODE((output / "sdk-work").stat().st_mode) == 0o700
    assert stat.S_IMODE(inventory_path.stat().st_mode) == 0o400
    assert stat.S_IMODE(
        (output / "sdk-dependency-bundle.tar").stat().st_mode
    ) == 0o400
    source_node = external / "node-modules/node_modules/example/index.js"
    archived_node = output / "sdk-inputs/node/node_modules/node_modules/example/index.js"
    assert source_node.stat().st_ino != archived_node.stat().st_ino
    assert stat.S_IMODE(archived_node.stat().st_mode) == 0o400

    archived_swift = output / "sdk-inputs/swiftpm/cache/checkouts/example/.git/HEAD"
    working_swift = output / "sdk-work/swiftpm/checkouts/example/.git/HEAD"
    archived_gradle = output / "sdk-inputs/gradle/gradle-user-home/caches"
    working_gradle = output / "sdk-work/gradle-home/caches"
    assert archived_swift.stat().st_ino != working_swift.stat().st_ino
    assert archived_gradle.stat().st_ino != working_gradle.stat().st_ino
    assert stat.S_IMODE(working_swift.stat().st_mode) == 0o600
    (output / "sdk-work/swiftpm/generated").write_text("mutable work\n")
    final_inventory = output / "sdk-dependency-work-final.json"
    with pytest.raises(
        helper.CacheCopyError,
        match="work template changed after child execution",
    ):
        helper.verify_sdk_dependencies(
            *arguments, final_work_inventory=final_inventory
        )
    (output / "sdk-work/swiftpm/generated").unlink()
    helper.verify_sdk_dependencies(*arguments, final_work_inventory=final_inventory)
    assert json.loads(final_inventory.read_text(encoding="utf-8"))["format"] \
        == helper.SDK_WORK_FORMAT
    command_work = output / f"sdk-command-work.{('a' * 32)}"
    helper.create_sdk_command_work(output / "sdk-inputs", command_work)
    command_launcher = (
        command_work
        / "gradle-home/wrapper/dists/gradle-9.3.0-bin"
        / helper.SDK_GRADLE_WRAPPER_CACHE_KEY
        / "gradle-9.3.0/bin/gradle"
    )
    immutable_launcher = (
        output
        / "sdk-inputs/gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin"
        / helper.SDK_GRADLE_WRAPPER_CACHE_KEY
        / "gradle-9.3.0/bin/gradle"
    )
    assert command_launcher.stat().st_ino != immutable_launcher.stat().st_ino
    (command_work / "swiftpm/generated").write_text(
        "disposable\n", encoding="utf-8"
    )
    with pytest.raises(
        helper.CacheCopyError,
        match="survived natural completion",
    ):
        helper.verify_sdk_dependencies(
            *arguments,
            final_work_inventory=output / "sdk-dependency-work-survivor.json",
        )
    helper.cleanup_sdk_command_work(output / "sdk-inputs", command_work)
    assert not command_work.exists()


def test_sdk_dependency_bundle_rejects_hardlinked_inputs(tmp_path: Path) -> None:
    helper, arguments, external, _ = _sdk_dependency_fixture(
        _private_directory(tmp_path / "hardlink")
    )
    node_file = external / "node-modules/node_modules/example/index.js"
    os.link(node_file, external / "node-modules/node_modules/example/alias.js")
    with pytest.raises(helper.CacheCopyError, match="metadata is unsafe"):
        helper.copy_sdk_dependencies(*arguments)

    helper, arguments, external, _ = _sdk_dependency_fixture(
        _private_directory(tmp_path / "unlisted-node")
    )
    (external / "node-modules/node_modules/example/rogue.js").write_text(
        "export const rogue = true;\n", encoding="utf-8"
    )
    with pytest.raises(
        helper.CacheCopyError,
        match="Node node_modules source inventory differs",
    ):
        helper.copy_sdk_dependencies(*arguments)

    helper, arguments, external, _ = _sdk_dependency_fixture(
        _private_directory(tmp_path / "dirty-swift")
    )
    swift_source = (
        external
        / "swiftpm-cache/checkouts/example/Sources/Example.swift"
    )
    swift_source.write_text(
        "public let fixtureValue = 2\n", encoding="utf-8"
    )
    with pytest.raises(
        helper.CacheCopyError,
        match="SwiftPM cache source inventory differs",
    ):
        helper.copy_sdk_dependencies(*arguments)

    helper, arguments, _, _ = _sdk_dependency_fixture(
        _private_directory(tmp_path / "wrong-gradle-key")
    )
    manifest_path = arguments[0]
    assert isinstance(manifest_path, Path)
    document = json.loads(manifest_path.read_text(encoding="utf-8"))
    document["gradle"]["wrapper_cache_key"] = "invented"
    manifest_path.chmod(0o600)
    manifest_path.write_bytes(helper._canonical_payload(document))
    manifest_path.chmod(0o400)
    rebound = (
        manifest_path,
        hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        *arguments[2:],
    )
    with pytest.raises(
        helper.CacheCopyError,
        match="must be version 9.3.0",
    ):
        helper.copy_sdk_dependencies(*rebound)

    helper, arguments, _, _ = _sdk_dependency_fixture(
        _private_directory(tmp_path / "wrong-swift-tree")
    )
    manifest_path = arguments[0]
    assert isinstance(manifest_path, Path)
    document = json.loads(manifest_path.read_text(encoding="utf-8"))
    document["swiftpm"]["resolved_revisions"][0]["tree"] = "f" * 40
    manifest_path.chmod(0o600)
    manifest_path.write_bytes(helper._canonical_payload(document))
    manifest_path.chmod(0o400)
    rebound = (
        manifest_path,
        hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        *arguments[2:],
    )
    with pytest.raises(
        helper.CacheCopyError,
        match="exact clean protected Git tree",
    ):
        helper.copy_sdk_dependencies(*rebound)

    helper, arguments, external, _ = _sdk_dependency_fixture(
        _private_directory(tmp_path / "zip-alias")
    )
    distribution = external / "gradle-9.3.0-bin.zip"
    with zipfile.ZipFile(distribution, "w") as archive:
        archive.writestr("gradle-9.3.0/bin/gradle", b"#!/bin/sh\n")
        archive.writestr("gradle-9.3.0/lib/gradle-core.jar", b"fixture jar\n")
        archive.writestr("gradle-9.3.0/lib/../lib/alias.jar", b"alias\n")
    manifest_path = arguments[0]
    assert isinstance(manifest_path, Path)
    document = json.loads(manifest_path.read_text(encoding="utf-8"))
    document["gradle"]["distribution_sha256"] = hashlib.sha256(
        distribution.read_bytes()
    ).hexdigest()
    manifest_path.chmod(0o600)
    manifest_path.write_bytes(helper._canonical_payload(document))
    manifest_path.chmod(0o400)
    rebound = (
        manifest_path,
        hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        *arguments[2:],
    )
    with pytest.raises(helper.CacheCopyError, match="ZIP member is unsafe"):
        helper.copy_sdk_dependencies(*rebound)


def test_release_runner_keeps_sdk_sources_private_and_budgets_before_build() -> None:
    source = RELEASE_RUNNER.read_text(encoding="utf-8")
    child = source[
        source.index('"$release_child_bin/env" -i'):
        source.index('"$release_child_bin/bash" "$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release')
    ]
    assert "IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST" not in child
    assert "IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256" not in child
    assert 'IROHA_RELEASE_SDK_INPUT_ROOT="$release_sdk_input_root"' in child
    assert 'IROHA_RELEASE_SDK_WORK_PARENT="$release_invocation_root"' in child
    assert 'IROHA_RELEASE_SDK_WORK_HELPER="$release_child_runtime/copy-release-runtime.py"' in child
    assert 'IROHA_RELEASE_SDK_WORK_HELPER_SHA256=' in child
    assert source.index('release_gate_boundary "source-file-budget:before"') \
        < source.index('release_gate_boundary "release-prebuilt-publication:before"')
    assert (
        '"$IROHA_RELEASE_PYTHON_BIN" -I -S scripts/check_source_file_budget.py'
        in source
    )
    assert (
        'readonly source_budget_log="${release_source_bound_root}/source-file-budget.log"'
        in source
    )
    assert 'verify_release_identity "before source-file budget guard"' in source
    assert 'chmod 0400 "$source_budget_log"' in source
    assert 'verify_release_identity "after source-file budget guard"' in source
    assert '(invocation_root, "sdk-inputs")' in RELEASE_HELPER.read_text(
        encoding="utf-8"
    )
    assert '(invocation_root, "sdk-work")' in RELEASE_HELPER.read_text(
        encoding="utf-8"
    )
