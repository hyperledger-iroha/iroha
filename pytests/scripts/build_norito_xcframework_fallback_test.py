"""Regression tests for the fail-closed NoritoBridge XCFramework builder."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "build_norito_xcframework.sh"


def _source() -> str:
    return SCRIPT.read_text(encoding="utf-8")


def test_publish_manifest_heredoc_contains_no_shell_comment_lines() -> None:
    source = _source()
    marker = 'cat > "$PUBLISH_MANIFEST" <<EOF\n'
    manifest_body = source.split(marker, 1)[1].split("\nEOF", 1)[0]

    assert not any(
        line.lstrip().startswith("#") for line in manifest_body.splitlines()
    )


def test_cargo_slice_builds_use_matrix_or_sequential_isolated_locked_targets() -> None:
    source = _source()
    lines = source.splitlines()

    assert source.count("run_hermetic_apple_cargo \\\n") == 1
    assert "run_apple_slice_wave" not in source
    assert "APPLE_SLICE_BUILD_PARALLELISM" not in source
    assert 'MATRIX_MODE=produce' in source
    assert 'MATRIX_MODE=assemble' in source
    assert 'if [[ "$MATRIX_MODE" == produce ]]' in source
    assert 'if [[ "$MATRIX_MODE" == assemble ]]' in source
    assert source.count(
        'build --locked --offline --jobs 1 -p "$LIB_CRATE_NAME" --lib --release'
    ) == 1
    for invocation in (
        "build_one_apple_slice ios-arm64",
        "build_one_apple_slice ios-sim-arm64",
        "build_one_apple_slice ios-sim-x64",
        "build_one_apple_slice macos-arm64",
        "build_one_apple_slice macos-x64",
    ):
        assert source.count(invocation) == 1

    assert "CARGO_BUILD_DIR_" not in source
    assert source.count('--set "CARGO_TARGET_DIR=$slice_target_dir"') == 1
    assert 'local slice_target_dir="$CARGO_TARGET_DIR/$target_triple"' in source
    assert (
        'source_library="$cargo_build_root/$target_triple/release/'
        'lib${LIB_CRATE_NAME}.a"'
    ) in source
    assert 'slice_target_dir="$CARGO_TARGET_DIR"' in source
    assert 'rm -f -- "$source_library"' not in source
    assert not any(
        "rm -rf" in line and "CARGO_TARGET_DIR" in line
        for line in lines
    )


def test_matrix_slice_interface_and_closed_evidence_are_canonical() -> None:
    source = _source()

    for slice_id in (
        "ios-arm64",
        "ios-sim-arm64",
        "ios-sim-x64",
        "macos-arm64",
        "macos-x64",
    ):
        assert f"    {slice_id})" in source
    assert "--produce-slice" in source
    assert "--slice-output-root" in source
    assert "--assemble-slices" in source
    assert "NORITO_BRIDGE_SLICE_BUILD_ID" in source
    assert "iroha.norito-bridge.apple-slice-context.v1" in source
    assert "iroha.norito-bridge.apple-slice-evidence.v1" in source
    for binding in (
        '"status_sha256"',
        '"fingerprint_sha256"',
        '"cargo_lock_sha256"',
        '"cargo_commit_hash"',
        '"rustc_commit_hash"',
        '"rustdoc_commit_hash"',
        '"xcode_build_version"',
        '"sdk_versions"',
        '"deployment_targets"',
        '"global_defined_symbols_sha256"',
        '"required_symbol_inventory_sha256"',
        '"forbidden_symbol_inventory_sha256"',
        '"architectures"',
    ):
        assert binding in source
    assert "slice assembly root does not contain exactly five canonical bundles" in source
    assert "evidence does not match this assembly context" in source
    assert "library bytes do not match closed evidence" in source
    assert "symbols do not match closed evidence" in source


def _assembly_validator_source() -> str:
    source = _source()
    return source.split("<<'PY_ASSEMBLE_SLICES'\n", 1)[1].split(
        "\nPY_ASSEMBLE_SLICES", 1
    )[0]


def _write_fake_apple_tool(path: Path, body: str) -> None:
    path.write_text("#!/bin/sh\nset -eu\n" + body, encoding="utf-8")
    path.chmod(0o700)


def _assembly_fixture(
    tmp_path: Path,
) -> tuple[Path, Path, Path, Path, Path, Path]:
    common = tmp_path / "common"
    staged = tmp_path / "staged"
    common.mkdir(mode=0o700)
    context_path = tmp_path / "context.json"
    context = {
        "schema": "iroha.norito-bridge.apple-slice-context.v1",
        "build_id": "123.1",
        "apple": {
            "sdk_versions": {
                "iphoneos": "26.0",
                "iphonesimulator": "26.0",
                "macosx": "26.0",
            },
            "deployment_targets": {
                "iphoneos": "15.0",
                "iphonesimulator": "15.0",
                "macosx": "12.0",
            },
        },
    }
    context_path.write_text(json.dumps(context), encoding="utf-8")
    configurations = {
        "ios-arm64": ("aarch64-apple-ios", "apple-ios-device", "iphoneos", "arm64"),
        "ios-sim-arm64": (
            "aarch64-apple-ios-sim",
            "apple-ios-simulator",
            "iphonesimulator",
            "arm64",
        ),
        "ios-sim-x64": (
            "x86_64-apple-ios",
            "apple-ios-simulator",
            "iphonesimulator",
            "x86_64",
        ),
        "macos-arm64": ("aarch64-apple-darwin", "apple-macos", "macosx", "arm64"),
        "macos-x64": ("x86_64-apple-darwin", "apple-macos", "macosx", "x86_64"),
    }
    symbol_bytes = b"closed_symbol\n"
    required_bytes = b"closed_symbol\n"
    forbidden_bytes = b"forbidden_symbol\n"
    for slice_id, (triple, profile, sdk_name, architecture) in configurations.items():
        bundle = common / slice_id
        bundle.mkdir(mode=0o700)
        library = bundle / "libconnect_norito_bridge.a"
        library_bytes = f"closed-{slice_id}".encode()
        library.write_bytes(library_bytes)
        evidence = {
            "schema": "iroha.norito-bridge.apple-slice-evidence.v1",
            "context": context,
            "slice": {
                "id": slice_id,
                "target_triple": triple,
                "profile": profile,
                "sdk_name": sdk_name,
                "sdk_version": "26.0",
                "deployment_target": "12.0" if sdk_name == "macosx" else "15.0",
            },
            "library": {
                "native_bridge_abi_version": 22,
                "file_name": library.name,
                "sha256": hashlib.sha256(library_bytes).hexdigest(),
                "size": len(library_bytes),
                "architectures": [architecture],
                "global_defined_symbols_sha256": hashlib.sha256(symbol_bytes).hexdigest(),
                "global_defined_symbol_count": 1,
                "required_symbol_inventory_sha256": hashlib.sha256(
                    required_bytes
                ).hexdigest(),
                "forbidden_symbol_inventory_sha256": hashlib.sha256(
                    forbidden_bytes
                ).hexdigest(),
            },
        }
        (bundle / "slice-evidence.json").write_text(
            json.dumps(evidence), encoding="utf-8"
        )
    lipo = tmp_path / "lipo"
    nm = tmp_path / "nm"
    _write_fake_apple_tool(
        lipo,
        'case "$2" in *x86_64*) echo x86_64 ;; *) echo arm64 ;; esac\n',
    )
    _write_fake_apple_tool(nm, "echo _closed_symbol\n")
    validator = tmp_path / "validator.py"
    validator.write_text(
        "REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 22\n"
        "EXPECTED_REQUIRED_SYMBOLS = ('closed_symbol',)\n"
        "EXPECTED_FORBIDDEN_SYMBOLS = ('forbidden_symbol',)\n",
        encoding="utf-8",
    )
    return common, context_path, staged, lipo, nm, validator


def _run_assembly_validator(
    common: Path,
    context: Path,
    staged: Path,
    lipo: Path,
    nm: Path,
    validator: Path,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            "-B",
            "-",
            str(common),
            str(context),
            str(staged),
            str(lipo),
            str(nm),
            "/Developer",
            str(validator),
        ],
        input=_assembly_validator_source(),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def test_slice_assembly_rejects_changed_library_bytes(tmp_path: Path) -> None:
    common, context, staged, lipo, nm, validator = _assembly_fixture(tmp_path)
    (common / "ios-arm64/libconnect_norito_bridge.a").write_bytes(b"tampered")
    completed = _run_assembly_validator(
        common, context, staged, lipo, nm, validator
    )
    assert completed.returncode != 0
    assert "library bytes do not match closed evidence" in completed.stderr


def test_slice_assembly_rejects_stale_context(tmp_path: Path) -> None:
    common, context, staged, lipo, nm, validator = _assembly_fixture(tmp_path)
    evidence_path = common / "ios-arm64/slice-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    evidence["context"]["build_id"] = "122.9"
    evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
    completed = _run_assembly_validator(
        common, context, staged, lipo, nm, validator
    )
    assert completed.returncode != 0
    assert "does not match this assembly context" in completed.stderr


def test_slice_assembly_rejects_symlinked_library(tmp_path: Path) -> None:
    common, context, staged, lipo, nm, validator = _assembly_fixture(tmp_path)
    library = common / "ios-arm64/libconnect_norito_bridge.a"
    replacement = tmp_path / "replacement.a"
    replacement.write_bytes(library.read_bytes())
    library.unlink()
    library.symlink_to(replacement)
    completed = _run_assembly_validator(
        common, context, staged, lipo, nm, validator
    )
    assert completed.returncode != 0
    assert "not an owner-controlled regular file" in completed.stderr


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
