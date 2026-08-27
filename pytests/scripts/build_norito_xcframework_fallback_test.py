"""Regression tests for the fail-closed NoritoBridge XCFramework builder."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "build_norito_xcframework.sh"
PINS_SCRIPT = ROOT / "scripts" / "update_norito_bridge_swift_pins.py"


def _source() -> str:
    return SCRIPT.read_text(encoding="utf-8")


def test_cargo_slice_builds_use_one_locked_offline_single_job_target() -> None:
    source = _source()
    lines = source.splitlines()
    root_assignment = source.index('ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"')
    root_anchor = source.index('builtin cd "$ROOT_DIR"')
    cargo_runner = source.index("run_hermetic_apple_cargo()")
    assert root_assignment < root_anchor < cargo_runner

    call_starts = [
        index
        for index, line in enumerate(lines)
        if line == "run_hermetic_apple_cargo \\"
    ]
    assert len(call_starts) == 5

    calls: list[list[str]] = []
    for start in call_starts:
        end = start
        while lines[end].endswith("\\"):
            end += 1
        calls.append(lines[start : end + 1])

    expected_slices = (
        ("apple-ios-device", "$IPHONEOS_SDKROOT", "$DEVICE_TRIPLE"),
        (
            "apple-ios-simulator",
            "$IPHONESIMULATOR_SDKROOT",
            "$SIM_ARM_TRIPLE",
        ),
        (
            "apple-ios-simulator",
            "$IPHONESIMULATOR_SDKROOT",
            "$SIM_X64_TRIPLE",
        ),
        ("apple-macos", "$MACOSX_SDKROOT", "$MACOS_ARM_TRIPLE"),
        ("apple-macos", "$MACOSX_SDKROOT", "$MACOS_X64_TRIPLE"),
    )
    for call, (profile, sdkroot, triple) in zip(calls, expected_slices, strict=True):
        assert len(call) == 5
        assert call[1].strip() == f'{profile} "{sdkroot}" \\'
        assert (
            call[2].strip()
            == 'build --locked --offline --jobs 1 -p "$LIB_CRATE_NAME" --lib --release \\'
        )
        assert call[3].strip() == f'--target "{triple}" \\'
        assert call[4].strip().startswith('"${CARGO_FEATURE_ARGS[@]+')

    assert "CARGO_BUILD_DIR_" not in source
    assert "local cargo_target_dir" not in source
    assert source.count('--set "CARGO_TARGET_DIR=$CARGO_TARGET_DIR"') == 1
    assert (
        'source_library="$CARGO_TARGET_DIR/$target_triple/release/'
        'lib${LIB_CRATE_NAME}.a"'
    ) in source
    assert not any(
        "rm -rf" in line and "CARGO_TARGET_DIR" in line
        for line in lines
    )


def test_retired_build_modes_are_rejected_before_cargo(tmp_path: Path) -> None:
    assert sys.version_info[:2] == (3, 12)
    python = Path(sys.executable).resolve(strict=True)
    assert not python.is_symlink()
    for retired in (
        "NORITO_BRIDGE_SKIP_CARGO_BUILDS",
        "NORITO_BRIDGE_PRESERVE_CARGO_TARGETS",
    ):
        environment = os.environ.copy()
        environment["MOBILE_SDK_PYTHON_BINARY"] = str(python)
        environment["NORITO_BRIDGE_BUILD_DIR"] = str(tmp_path / f"{retired}-build")
        environment["NORITO_BRIDGE_OUT_DIR"] = str(tmp_path / f"{retired}-out")
        environment[retired] = ""
        completed = subprocess.run(
            ["bash", str(SCRIPT)],
            cwd=ROOT,
            env=environment,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert completed.returncode == 1
        assert (
            f"{retired} is not part of the first-release build contract"
            in completed.stderr
        )
        assert "Building Rust static libraries" not in completed.stderr


def test_xcodebuild_failure_has_no_manual_packaging_fallback() -> None:
    source = _source()
    packaging_start = source.index('echo "[+] Creating XCFramework"')
    packaging_end = source.index(
        'assert_bridge_source_seal "XCFramework packaging"', packaging_start
    )
    packaging = source[packaging_start:packaging_end]

    assert "write_static_xcframework_info_plist" not in source
    assert "copy_static_xcframework_slice" not in source
    assert "rebuilding the fallback" not in source
    assert "after xcodebuild failure" not in source
    assert "xcodebuild_status=$?" in packaging
    assert 'exit "$xcodebuild_status"' in packaging
    assert 'rm -rf -- "$PUBLISH_XCFRAMEWORK"' not in packaging
    assert "$FINAL_XCFRAMEWORK" not in packaging
    assert "$FINAL_MANIFEST" not in packaging

    cleanup = source.index("cleanup_build_state()")
    candidate = source.index(
        'PUBLISH_ROOT="$(mktemp -d "$OUT_DIR/.NoritoBridge.publish.XXXXXX")"'
    )
    publication = source.index(
        'assert_bridge_source_seal "pre-publication artifact verification"'
    )
    assert cleanup < candidate < packaging_start < publication


def test_test_only_prebuilt_slice_mode_is_fully_retired() -> None:
    source = _source()

    for retired in (
        "TEST_ONLY_PREBUILT_SLICES",
        "NORITO_BRIDGE_TEST_PREBUILT_SLICES",
        "NORITO_BRIDGE_CHECKER_MARKER",
        "NORITO_BRIDGE_CHECKER_EXIT",
        "TEST_ONLY_MANIFEST_FIELD",
    ):
        assert retired not in source

    assert source.count('"$LIPO_BINARY" -create -output "$SIM_UNI"') == 1
    assert source.count('"$LIPO_BINARY" -create -output "$MAC_UNI"') == 1
    assert source.count("release staged NoritoBridge contains test-only prebuilt slices") == 1
    assert source.count("scripts/check_mobile_sdk_artifacts.sh") == 2


def test_ci_handoff_never_enters_the_release_publication_corridor() -> None:
    """The cold CI producer emits only a structurally validated candidate."""

    source = _source()
    validation = source.index(
        '"$ROOT_DIR/scripts/validate_norito_bridge_xcframework.py"'
    )
    handoff_branch = source.index('if [[ "$CI_HANDOFF_ONLY" == "1" ]]', validation)
    checker = source.index(
        'bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh"',
        handoff_branch,
    )
    publish_lock_cleanup = source.index(
        'rm -f "$PUBLISH_ROOT/.NoritoBridge.publish.lockfile"',
        handoff_branch,
    )
    exact_root_check = source.index(
        "{entry.name for entry in staged.iterdir()} != expected_entries",
        publish_lock_cleanup,
    )
    candidate = source.index(
        'echo "[+] Atomically staged uncertified CI handoff candidate:',
        checker,
    )
    canonical_block = source.index(
        'run_isolated_python - \\\n'
        '  "$PUBLISH_XCFRAMEWORK" "$FINAL_XCFRAMEWORK"',
        candidate,
    )
    canonical_publication = source.index(
        'echo "[+] Atomically published XCFramework and canonical manifest:',
        canonical_block,
    )
    handoff = source[handoff_branch:canonical_block]

    assert "--ci-handoff-only cannot publish an archive or use dirty source" in source
    assert "CI_HANDOFF_ONLY=0" in source
    assert 'CI_HANDOFF_ONLY="${' not in source
    assert "authenticated Kagemusha Swift producer" in source
    assert "requires canonical release outputs to remain absent" in source
    assert '${GITHUB_WORKFLOW:-}' in source
    assert '${GITHUB_JOB:-}' in source
    assert '${GITHUB_WORKSPACE:-}' in source
    assert 'CI_HANDOFF_DIR="$OUT_DIR/NoritoBridge.ci-handoff"' in source
    assert (
        validation
        < handoff_branch
        < checker
        < publish_lock_cleanup
        < exact_root_check
        < candidate
        < canonical_block
        < canonical_publication
    )
    assert 'assert_bridge_source_seal "pre-handoff artifact verification"' in handoff
    assert 'assert_bridge_source_seal "pre-publication artifact verification"' in handoff
    assert 'rm -f "$PUBLISH_ROOT/.NoritoBridge.publish.lockfile"' in handoff
    assert (
        'PUBLISH_LOCK_NAME = ".NoritoBridge.publish.lockfile"'
        in PINS_SCRIPT.read_text(encoding="utf-8")
    )
    assert '"NoritoBridge.xcframework"' in handoff
    assert '"NoritoBridge.artifacts.json"' in handoff
    assert "RENAME_EXCL = 0x00000004" in handoff
    assert 'PUBLISH_ROOT=""' in handoff
    assert "exit 0" in handoff
    assert "$FINAL_XCFRAMEWORK" not in handoff
    assert "$FINAL_MANIFEST" not in handoff
    assert source.count("scripts/check_mobile_sdk_artifacts.sh") == 2

    workflow_users = [
        workflow
        for workflow in (ROOT / ".github" / "workflows").glob("*.yml")
        if "--ci-handoff-only" in workflow.read_text(encoding="utf-8")
    ]
    assert workflow_users == [
        ROOT / ".github" / "workflows" / "pr_kagemusha_payload_bench.yml"
    ]


def test_build_and_output_roots_are_canonical_disjoint_directories() -> None:
    source = _source()
    assert "canonical_writable_directory()" in source
    assert 'OUT_DIR="$(canonical_writable_directory' in source
    assert 'BUILD_DIR="$(canonical_writable_directory' in source
    assert (
        "Cargo target, build, and output directories must be pairwise disjoint"
        in source
    )
    assert " ps " not in source
