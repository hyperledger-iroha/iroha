"""Regression checks for CI entrypoints that must run on macOS Bash 3.2."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
BASH_3_ENTRYPOINTS = (
    REPO_ROOT / "scripts/ci/run_xcframework_smoke.sh",
    REPO_ROOT / "scripts/swift_fixture_regen.sh",
    REPO_ROOT / "scripts/run_full_tests.sh",
    REPO_ROOT / "check_pending_incentive_snapshots.sh",
    REPO_ROOT / "scripts/android_sbom_provenance.sh",
)
BASH_4_ONLY_PATTERNS = (
    re.compile(r"\bdeclare\s+-A\b"),
    re.compile(r"\blocal\s+-n\b"),
    re.compile(r"\b(?:mapfile|readarray)\b"),
    re.compile(r"\$\{[^}\n]+(?:,,|\^\^)[^}\n]*\}"),
)


def _write_executable(path: Path, source: str) -> None:
    path.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
    path.chmod(0o755)


@pytest.mark.parametrize("script", BASH_3_ENTRYPOINTS, ids=lambda path: path.name)
def test_entrypoint_parses_with_stock_macos_bash(script: Path) -> None:
    result = subprocess.run(
        ["/bin/bash", "-n", str(script)],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("script", BASH_3_ENTRYPOINTS, ids=lambda path: path.name)
def test_entrypoint_avoids_bash_4_only_builtins_and_expansions(script: Path) -> None:
    source = script.read_text(encoding="utf-8")

    for pattern in BASH_4_ONLY_PATTERNS:
        assert pattern.search(source) is None, f"{script} contains {pattern.pattern}"

@pytest.mark.parametrize(
    "relay_args",
    [("--relay", "ABCD"), ("--relay=ABCD",)],
    ids=["separate-value", "equals-value"],
)
def test_pending_snapshot_filter_lowercases_relay_on_bash_3(
    tmp_path: Path,
    relay_args: tuple[str, ...],
) -> None:
    (tmp_path / "relay-abcd-epoch-7-100.to").write_bytes(b"uptime")
    (tmp_path / "relay-abcd-epoch-7-101.to").write_bytes(b"measurement")

    result = subprocess.run(
        [
            "/bin/bash",
            str(REPO_ROOT / "check_pending_incentive_snapshots.sh"),
            "--dir",
            str(tmp_path),
            *relay_args,
        ],
        check=False,
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
    )

    assert result.returncode == 0, result.stderr
    report = json.loads(result.stdout)
    assert report["relays"] == {"abcd": {"7": ["measurement", "uptime"]}}
    assert report["missing"] == []


def test_portable_replacements_keep_all_fixed_mappings() -> None:
    xcframework = (REPO_ROOT / "scripts/ci/run_xcframework_smoke.sh").read_text(
        encoding="utf-8"
    )
    for lane, index in (
        ("iphone-sim", 0),
        ("ipad-sim", 1),
        ("strongbox", 2),
        ("macos-fallback", 3),
    ):
        assert f'{lane}) printf \'%s\\n\' {index} ;;' in xcframework

    sbom = (REPO_ROOT / "scripts/android_sbom_provenance.sh").read_text(
        encoding="utf-8"
    )
    for module, filename in (
        ("java/iroha_android/jvm", "iroha-android-jvm.cyclonedx.json"),
        ("java/iroha_android/android", "iroha-android.cyclonedx.json"),
        ("examples/android/operator-console", "operator-console.cyclonedx.json"),
        ("examples/android/retail-wallet", "retail-wallet.cyclonedx.json"),
    ):
        assert module in sbom
        assert filename in sbom

    for script_name in (
        "python_fixture_regen.sh",
        "swift_fixture_regen.sh",
        "android_fixture_regen.sh",
    ):
        fixture_regen = (REPO_ROOT / "scripts" / script_name).read_text(
            encoding="utf-8"
        )
        assert 'norito-rpc-fixtures "$@"' in fixture_regen
        for retired in ("rsync", "export_norito_fixtures", "SWIFT_FIXTURE_ARCHIVE"):
            assert retired not in fixture_regen


@pytest.mark.parametrize(
    "script_name",
    [
        "python_fixture_regen.sh",
        "swift_fixture_regen.sh",
        "android_fixture_regen.sh",
    ],
)
def test_fixture_regen_delegates_exactly_to_the_canonical_owner(
    tmp_path: Path, script_name: str
) -> None:
    fake_cargo = tmp_path / "cargo"
    capture = tmp_path / "args.txt"
    _write_executable(
        fake_cargo,
        """
        #!/bin/bash
        printf '%s\n' "$@" > "${CAPTURE_ARGS}"
        """,
    )
    env = os.environ.copy()
    env.update({"CARGO_BIN": str(fake_cargo), "CAPTURE_ARGS": str(capture)})

    result = subprocess.run(
        [
            "/bin/bash",
            str(REPO_ROOT / "scripts" / script_name),
            "--output-root",
            "artifacts/norito-stage",
        ],
        check=False,
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
        env=env,
    )

    assert result.returncode == 0, result.stderr
    assert capture.read_text(encoding="utf-8").splitlines() == [
        "run",
        "--locked",
        "-p",
        "xtask",
        "--features",
        "dev-tools",
        "--bin",
        "xtask",
        "--",
        "norito-rpc-fixtures",
        "--output-root",
        "artifacts/norito-stage",
    ]


def test_android_sbom_collection_preserves_each_module_report(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    destination = tmp_path / "collected"
    destination.mkdir()
    reports = {
        "java/iroha_android/jvm": "jvm",
        "java/iroha_android/android": "android",
        "examples/android/operator-console": "operator-console",
        "examples/android/retail-wallet": "retail-wallet",
    }
    for module, contents in reports.items():
        report = repo_root / module / "build" / "reports" / "bom" / "bom.json"
        report.parent.mkdir(parents=True)
        report.write_text(contents, encoding="utf-8")

    script = REPO_ROOT / "scripts/android_sbom_provenance.sh"
    result = subprocess.run(
        [
            "/bin/bash",
            "-c",
            'source "$1"; collect_sbom_reports "$2" "$3"',
            "bash",
            str(script),
            str(repo_root),
            str(destination),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert {
        path.name: path.read_text(encoding="utf-8")
        for path in destination.iterdir()
    } == {
        "iroha-android-jvm.cyclonedx.json": "jvm",
        "iroha-android.cyclonedx.json": "android",
        "operator-console.cyclonedx.json": "operator-console",
        "retail-wallet.cyclonedx.json": "retail-wallet",
    }


def test_xcframework_smoke_emits_all_lanes_through_separate_data_fd(
    tmp_path: Path,
) -> None:
    harness_root = tmp_path / "repo"
    harness = harness_root / "scripts" / "ci" / "run_xcframework_smoke.sh"
    harness.parent.mkdir(parents=True)
    harness.write_bytes(
        (REPO_ROOT / "scripts" / "ci" / "run_xcframework_smoke.sh").read_bytes()
    )
    harness.chmod(0o755)
    anomaly_checker = harness_root / "scripts" / "swift_smoke_anomalies.py"
    anomaly_checker.write_bytes(
        (REPO_ROOT / "scripts" / "swift_smoke_anomalies.py").read_bytes()
    )
    (harness_root / "examples" / "ios" / "NoritoDemoXcode").mkdir(parents=True)

    bridge_sentinel = tmp_path / "bridge-built"
    _write_executable(
        harness_root / "scripts" / "build_norito_xcframework.sh",
        f"""
        #!/bin/bash
        set -euo pipefail
        [[ "${{CARGO_TARGET_DIR:-}}" == "{tmp_path / 'cargo-target'}" ]]
        [[ "${{CARGO_BUILD_JOBS:-}}" == "1" ]]
        [[ "${{CARGO_INCREMENTAL:-}}" == "0" ]]
        [[ "${{CARGO_NET_OFFLINE:-}}" == "true" ]]
        [[ "${{RUSTC_BOOTSTRAP:-}}" == "1" ]]
        [[ -n "${{RUSTC:-}}" ]]
        [[ -n "${{RUSTDOC:-}}" ]]
        printf '%s\n' built > "{bridge_sentinel}"
        """,
    )

    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_executable(
        fake_bin / "swift",
        """
        #!/bin/bash
        exit 0
        """,
    )
    _write_executable(
        fake_bin / "xcodebuild",
        """
        #!/bin/bash
        printf '%s\n' "fake xcodebuild $*"
        if [[ "${IOS6_TEST_XCODEBUILD_FAIL:-0}" == "1" ]]; then
          exit 37
        fi
        exit 0
        """,
    )
    _write_executable(
        fake_bin / "xcrun",
        """
        #!/bin/bash
        if [[ "$1" == "simctl" && "$2" == "list" ]]; then
          printf '%s\n' '{"devices":{"iOS-17-0":[{"name":"iPhone 15","udid":"IPHONE-UDID","isAvailable":true},{"name":"iPad (10th generation)","udid":"IPAD-UDID","isAvailable":true}]}}'
          exit 0
        fi
        if [[ "$1" == "simctl" && ( "$2" == "bootstatus" || "$2" == "boot" ) ]]; then
          exit 0
        fi
        exit 1
        """,
    )
    _write_executable(
        fake_bin / "df",
        """
        #!/bin/bash
        printf '%s\n' 'Filesystem 1024-blocks Used Available Capacity Mounted on'
        printf '%s\n' 'fake 100000000 1 99999999 1% /'
        """,
    )

    result_path = tmp_path / "result.json"
    anomaly_path = tmp_path / "anomalies.json"
    env = os.environ.copy()
    env.update(
        {
            "PATH": os.pathsep.join(
                [str(fake_bin), str(Path(sys.executable).parent), "/usr/bin", "/bin"]
            ),
            "CARGO_TARGET_DIR": str(tmp_path / "cargo-target"),
            "CARGO_BUILD_JOBS": "1",
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "RUSTC_BOOTSTRAP": "1",
            "RUSTC": "/toolchains/1.93.1/bin/rustc",
            "RUSTDOC": "/toolchains/1.93.1/bin/rustdoc",
            "IOS6_SMOKE_DERIVED_DATA": str(tmp_path / "derived"),
            "IOS6_SMOKE_RESULTS_PATH": str(result_path),
            "IOS6_SMOKE_ANOMALY_PATH": str(anomaly_path),
        }
    )

    result = subprocess.run(
        ["/bin/bash", str(harness)],
        check=False,
        capture_output=True,
        text=True,
        cwd=harness_root,
        env=env,
        timeout=30,
    )

    assert result.returncode == 0, result.stderr
    assert bridge_sentinel.read_text(encoding="utf-8") == "built\n"
    telemetry = json.loads(result_path.read_text(encoding="utf-8"))
    lanes = telemetry["buildkite"]["lanes"]
    assert [lane["name"] for lane in lanes] == [
        "ci/xcframework-smoke:iphone-sim",
        "ci/xcframework-smoke:ipad-sim",
        "ci/xcframework-smoke:strongbox",
    ]
    assert [lane["status"] for lane in lanes] == ["pass", "pass", "pass"]
    assert [lane["device_tag"] for lane in lanes] == [
        "iphone-sim",
        "ipad-sim",
        "strongbox",
    ]
    assert telemetry["devices"] == {
        "emulators": {"passes": 2, "failures": 0},
        "strongbox_capable": {"passes": 1, "failures": 0},
    }
    assert telemetry["alert_state"] == {
        "consecutive_failures": 0,
        "open_incidents": [],
    }
    assert anomaly_path.exists()

    failed_result_path = tmp_path / "failed-result.json"
    failed_anomaly_path = tmp_path / "failed-anomalies.json"
    env.update(
        {
            "IOS6_TEST_XCODEBUILD_FAIL": "1",
            "IOS6_SMOKE_RESULTS_PATH": str(failed_result_path),
            "IOS6_SMOKE_ANOMALY_PATH": str(failed_anomaly_path),
        }
    )
    failed = subprocess.run(
        ["/bin/bash", str(harness)],
        check=False,
        capture_output=True,
        text=True,
        cwd=harness_root,
        env=env,
        timeout=30,
    )
    assert failed.returncode != 0
    failed_telemetry = json.loads(failed_result_path.read_text(encoding="utf-8"))
    assert {lane["status"] for lane in failed_telemetry["buildkite"]["lanes"]} == {
        "fail"
    }
    assert failed_anomaly_path.exists()
