"""Tests for the Cargo-free Swift offline-attestation fixture checker."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
import re
import subprocess
import sys
import tarfile
import textwrap
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts/check_swift_offline_device_attestation_fixture.py"
SPEC = importlib.util.spec_from_file_location(
    "check_swift_offline_device_attestation_fixture", MODULE_PATH
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def generator_output() -> bytes:
    return (
        b"abcd\n"
        b"0011\n"
        + (b"22" * 32)
        + b"\n"
        + "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".encode()
        + b"\n"
        + (b"33" * 32)
        + b"\n"
    )


def test_render_fixture_maps_the_strict_five_line_contract() -> None:
    rendered = MODULE.render_fixture(generator_output())
    decoded = json.loads(rendered)

    assert decoded == {
        "fixture": "offline_device_attestation_abi21",
        "generated_by": "kotlin-fixture-gen offline-device-attestation",
        "registration_hex": "abcd",
        "challenge_hash_hex": "22" * 32,
        "account_id": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        "registration_id_hex": "33" * 32,
    }
    assert rendered.endswith(b"\n")


@pytest.mark.parametrize(
    "payload",
    [
        generator_output().rstrip(b"\n"),
        generator_output() + b"extra\n",
        generator_output().replace(b"abcd", b"ABCD", 1),
        generator_output().replace(b"22" * 32, b"22" * 31, 1),
        generator_output().replace(b"sorau", b" sorau", 1),
    ],
)
def test_render_fixture_rejects_noncanonical_generator_output(payload: bytes) -> None:
    with pytest.raises(MODULE.FixtureError):
        MODULE.render_fixture(payload)


def test_two_pass_check_rejects_raw_generator_drift(monkeypatch: pytest.MonkeyPatch) -> None:
    outputs = iter([generator_output(), generator_output().replace(b"abcd", b"abce", 1)])
    monkeypatch.setattr(MODULE, "run_generator", lambda _command: next(outputs))

    with pytest.raises(MODULE.FixtureError, match="between isolated passes"):
        MODULE.verify_two_pass_output(["fixture-generator"])


def test_generator_command_selects_direct_binary_or_locked_cargo() -> None:
    assert MODULE.generator_command(Path("/tmp/generator"), "cargo-unused") == [
        "/tmp/generator",
        "offline-device-attestation",
    ]
    assert MODULE.generator_command(None, "cargo-test") == [
        "cargo-test",
        "run",
        "--quiet",
        "--locked",
        "-p",
        "kotlin-fixture-gen",
        "--features",
        "dev-tools",
        "--bin",
        "kotlin-fixture-gen",
        "--",
        "offline-device-attestation",
    ]


def test_run_generator_returns_stdout_and_rejects_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        MODULE.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            ["fixture-generator"], 0, stdout=generator_output(), stderr=b""
        ),
    )
    assert MODULE.run_generator(["fixture-generator"]) == generator_output()

    monkeypatch.setattr(
        MODULE.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            ["fixture-generator"], 7, stdout=b"", stderr=b"generation failed"
        ),
    )
    with pytest.raises(MODULE.FixtureError, match="status 7: generation failed"):
        MODULE.run_generator(["fixture-generator"])


def test_atomic_write_fixture_replaces_content_and_leaves_no_temporary(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "nested" / "fixture.json"
    fixture.parent.mkdir()
    fixture.write_bytes(b"old")

    MODULE.atomic_write_fixture(fixture, b"new\n")

    assert fixture.read_bytes() == b"new\n"
    assert fixture.stat().st_mode & 0o777 == 0o644
    assert list(fixture.parent.glob(f".{fixture.name}.*")) == []


@pytest.mark.parametrize(
    ("existing", "expected_error"),
    [
        (None, "required checked-in fixture is missing"),
        (b"stale\n", "is stale; rerun this command with --write"),
    ],
)
def test_main_rejects_missing_or_stale_fixture(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    existing: bytes | None,
    expected_error: str,
) -> None:
    fixture = tmp_path / "fixture.json"
    if existing is not None:
        fixture.write_bytes(existing)
    rendered = MODULE.render_fixture(generator_output())
    monkeypatch.setattr(MODULE, "verify_two_pass_output", lambda _command: rendered)

    assert MODULE.main(["--fixture", str(fixture), "--generator", "/tmp/gen"]) == 1
    assert expected_error in capsys.readouterr().err


def test_main_atomically_writes_and_then_verifies_fixture(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    fixture = tmp_path / "fixtures" / "fixture.json"
    rendered = MODULE.render_fixture(generator_output())
    commands: list[list[str]] = []

    def verify(command: list[str]) -> bytes:
        commands.append(command)
        return rendered

    monkeypatch.setattr(MODULE, "verify_two_pass_output", verify)

    assert (
        MODULE.main(
            [
                "--fixture",
                str(fixture),
                "--generator",
                "/tmp/fixture-generator",
                "--write",
            ]
        )
        == 0
    )
    assert fixture.read_bytes() == rendered
    assert commands == [
        ["/tmp/fixture-generator", "offline-device-attestation"]
    ]
    assert "two-pass fixture check passed sha256=" in capsys.readouterr().out

    assert (
        MODULE.main(
            [
                "--fixture",
                str(fixture),
                "--generator",
                "/tmp/fixture-generator",
            ]
        )
        == 0
    )
    assert len(commands) == 2


def test_workflow_watches_and_executes_the_fixture_contract() -> None:
    workflow = (
        REPO_ROOT / ".github/workflows/pr_kagemusha_payload_bench.yml"
    ).read_text(encoding="utf-8")
    swift_gate = (
        REPO_ROOT / "ci/check_kagemusha_recursive_spend_swift_sdk.sh"
    ).read_text(encoding="utf-8")

    assert "scripts/check_swift_offline_device_attestation_fixture.py" in workflow
    assert "scripts/tests/check_swift_offline_device_attestation_fixture_test.py" in workflow
    assert "scripts/tests/norito_bridge_apple_slice_handoff_test.py" in workflow
    assert (
        "python3 -m unittest scripts.tests."
        "norito_bridge_apple_slice_handoff_test"
    ) in workflow
    assert (
        "IrohaSwift/Tests/IrohaSwiftTests/Fixtures/"
        "offline_device_attestation_abi21.json"
    ) in workflow
    assert "OfflineDeviceAttestationABI21ParityTests.swift" in swift_gate
    assert "--filter OfflineDeviceAttestationABI21ParityTests" in swift_gate


def test_workflow_splits_native_build_from_authenticated_swift_tests() -> None:
    """Five cold Apple slices authenticate into one ABI-23 Swift handoff."""

    workflow = (
        REPO_ROOT / ".github/workflows/pr_kagemusha_payload_bench.yml"
    ).read_text(encoding="utf-8")
    builder = (REPO_ROOT / "scripts/build_norito_xcframework.sh").read_text(
        encoding="utf-8"
    )

    def job_body(job_id: str) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(job_id)}:\n"
            r"(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            workflow,
        )
        assert match is not None, job_id
        return match.group("body")

    producer_targets = {
        "swift_slice_ios_device": "aarch64-apple-ios",
        "swift_slice_ios_sim_arm": "aarch64-apple-ios-sim",
        "swift_slice_ios_sim_x64": "x86_64-apple-ios",
        "swift_slice_macos_arm": "aarch64-apple-darwin",
        "swift_slice_macos_x64": "x86_64-apple-darwin",
    }
    upload_action = (
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
    )
    download_action = (
        "actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093"
    )
    final_artifact_name = (
        "kagemusha-apple-xcframework-${{ github.sha }}-"
        "${{ github.run_id }}"
    )

    for job_id, target in producer_targets.items():
        producer = job_body(job_id)
        artifact_name = (
            f"kagemusha-apple-slice-{target}-"
            "${{ github.sha }}-${{ github.run_id }}"
        )
        archive_path = (
            "${{ runner.temp }}/iroha-kagemusha-apple-slice-"
            f"{target}-artifacts/NoritoBridge.apple-slice.tar"
        )

        assert f"name: Swift Apple slice ({target})" in producer
        assert "runs-on: macos-14" in producer
        assert "timeout-minutes: 360" in producer
        assert "slice_sha256: ${{ steps.slice-handoff.outputs.sha256 }}" in producer
        assert 'python-version: "3.12"' in producer
        assert "toolchain: 1.93.1" in producer
        assert f'"$rustup_path" target add {target}' in producer
        assert producer.count('"$rustup_path" target add ') == 1
        assert producer.index("cargo fetch --locked") < producer.index(
            'echo "CARGO_NET_OFFLINE=true"'
        )
        for binding in (
            'echo "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1"',
            'echo "CARGO_BUILD_JOBS=1"',
            'echo "CARGO_INCREMENTAL=0"',
            'echo "CARGO_NET_OFFLINE=true"',
            'echo "CARGO_TARGET_DIR=$cargo_target"',
            'echo "RUSTC=$rustc_path"',
            'echo "RUSTC_BOOTSTRAP=1"',
            'echo "RUSTDOC=$rustdoc_path"',
        ):
            assert binding in producer
        assert producer.count(f"iroha-kagemusha-apple-slice-{target}-") >= 4
        assert (
            "scripts/build_norito_xcframework.sh "
            f"--ci-apple-slice {target}"
        ) in producer
        assert "id: slice-handoff" in producer
        assert (
            'archive="$NORITO_BRIDGE_OUT_DIR/NoritoBridge.apple-slice.tar"'
            in producer
        )
        assert '"$MOBILE_SDK_PYTHON_BINARY" -I -S -B -c' in producer
        assert 'echo "sha256=$slice_sha256" >> "$GITHUB_OUTPUT"' in producer
        assert producer.count(upload_action) == 1
        assert download_action not in producer
        assert f"name: {artifact_name}" in producer
        assert f"path: {archive_path}" in producer
        assert "if-no-files-found: error" in producer
        assert "retention-days: 1" in producer
        assert "compression-level: 0" in producer
        assert "overwrite: true" in producer
        assert "--ci-handoff-only" not in producer
        assert final_artifact_name not in producer
        assert "github.run_attempt" not in producer

    native = job_body("swift")
    lifecycle = job_body("swift_lifecycle")
    assert "name: Swift native artifact build" in native
    assert "timeout-minutes: 360" in native
    assert re.findall(
        r"(?m)^      - (swift_slice_[a-z0-9_]+)$", native
    ) == list(producer_targets)
    assert "Swatinem/rust-cache@" not in native
    assert '"$rustup_path" target add ' not in native
    assert native.count(download_action) == 5
    assert "scripts/build_norito_xcframework.sh \\" in native
    assert "--ci-handoff-only \\" in native
    assert (
        '--ci-assemble-apple-slices "$RUNNER_TEMP/'
        'iroha-kagemusha-apple-slices"'
        in native
    )
    assert native.count("--ci-apple-slice-sha256") == 5
    for job_id, target in producer_targets.items():
        artifact_name = (
            f"kagemusha-apple-slice-{target}-"
            "${{ github.sha }}-${{ github.run_id }}"
        )
        assert f"name: {artifact_name}" in native
        assert (
            "path: ${{ runner.temp }}/iroha-kagemusha-apple-slices/"
            f"{target}"
        ) in native
        assert (
            f'--ci-apple-slice-sha256 "{target}='
            f"${{{{ needs.{job_id}.outputs.slice_sha256 }}}}" + '"'
        ) in native
        assert workflow.count(artifact_name) == 2
    assert native.rindex(download_action) < native.index(
        "Assemble exact ABI-23 NoritoBridge XCFramework"
    )
    assert (
        'artifact_root="$MOBILE_SDK_APPLE_ARTIFACT_DIR/'
        'NoritoBridge.ci-handoff"'
        in native
    )
    assert (
        '[[ ! -e "$MOBILE_SDK_APPLE_ARTIFACT_DIR/'
        'NoritoBridge.xcframework" ]]'
        in native
    )
    assert (
        '[[ ! -L "$MOBILE_SDK_APPLE_ARTIFACT_DIR/'
        'NoritoBridge.artifacts.json" ]]'
        in native
    )
    assert "handoff_sha256: ${{ steps.apple-handoff.outputs.sha256 }}" in native
    assert "id: apple-handoff" in native
    assert "COPYFILE_DISABLE=1 /usr/bin/tar -cf" in native
    assert "NoritoBridge.xcframework NoritoBridge.artifacts.json" in native
    assert native.count(upload_action) == 1
    assert f"name: {final_artifact_name}" in native
    assert "compression-level: 0" in native
    assert "overwrite: true" in native
    assert "github.run_attempt" not in native
    assert "check_mobile_sdk_artifacts.sh --apple-only" not in native
    assert "check_kagemusha_recursive_spend_swift_sdk.sh" not in native
    assert "--ci-handoff-only cannot publish an archive or use dirty source" in builder
    assert 'CI_HANDOFF_DIR="$OUT_DIR/NoritoBridge.ci-handoff"' in builder
    assert "RENAME_EXCL = 0x00000004" in builder
    assert builder.index(
        'assert_bridge_source_seal "pre-handoff artifact verification"'
    ) < builder.index(
        'echo "[+] Atomically staged uncertified CI handoff candidate:'
    )
    assert builder.index(
        'echo "[+] Atomically staged uncertified CI handoff candidate:'
    ) < builder.index(
        'echo "[+] Atomically published XCFramework and canonical manifest:'
    )

    assert workflow.count("id: apple-handoff") == 1
    assert workflow.count("Pack authenticated Apple artifact handoff") == 1
    assert workflow.count(final_artifact_name) == 2
    assert workflow.count("--ci-handoff-only") == 1

    assert "name: Swift lifecycle surface" in lifecycle
    assert "needs: swift" in lifecycle
    assert "timeout-minutes: 360" in lifecycle
    assert download_action in lifecycle
    assert 'tarfile.open(fileobj=handle, mode="r:")' in lifecycle
    assert 'filter="data"' in lifecycle
    assert '"${{ needs.swift.outputs.handoff_sha256 }}"' in lifecycle
    assert "digest_before != expected_digest" in lifecycle
    assert "digest_after != expected_digest" in lifecycle
    assert "member.sparse is not None" in lifecycle
    assert 'member.linkname != manifest_target' in lifecycle
    assert 'os.readlink(manifest) != manifest_target' in lifecycle
    assert lifecycle.index('run 1.93.1 cargo fetch --locked') < lifecycle.index(
        'CARGO_NET_OFFLINE=true'
    )
    assert "check_mobile_sdk_artifacts.sh --apple-only" in lifecycle
    assert "check_kagemusha_recursive_spend_swift_sdk.sh" in lifecycle
    assert '      - "ci/check_authenticated_tool_controller.sh"' in workflow
    for watched_source_seal_input in (
        '".cargo/**"',
        '"codec/**"',
        '"vendor/**"',
        '"rust-toolchain"',
        '"scripts/archive_norito_xcframework.py"',
        '"scripts/check_mobile_sdk_artifact_pin_commit.py"',
        '"scripts/exec_with_file_lock.py"',
        '"scripts/norito_bridge_apple_slice_handoff.py"',
        '"scripts/package_mobile_sdk_artifacts.sh"',
        '"scripts/render_norito_bridge_podspec.py"',
        '"scripts/update_norito_bridge_swift_pins.py"',
        '"scripts/validate_norito_bridge_xcframework.py"',
    ):
        assert f"      - {watched_source_seal_input}" in workflow
    assert lifecycle.index("actions/download-artifact@") < lifecycle.index(
        "Restore authenticated Apple artifact handoff"
    )
    assert lifecycle.index(
        "Restore authenticated Apple artifact handoff"
    ) < lifecycle.index("check_mobile_sdk_artifacts.sh --apple-only")
    assert lifecycle.index(
        "check_mobile_sdk_artifacts.sh --apple-only"
    ) < lifecycle.index(
        "check_kagemusha_recursive_spend_swift_sdk.sh"
    )


def workflow_handoff_restore_program() -> str:
    """Return the Python program embedded in the handoff restore step."""

    workflow = (
        REPO_ROOT / ".github/workflows/pr_kagemusha_payload_bench.yml"
    ).read_text(encoding="utf-8")
    marker = (
        '            "$MOBILE_SDK_APPLE_ARTIFACT_DIR" '
        '"$GITHUB_WORKSPACE" <<\'PY\'\n'
    )
    _, separator, remainder = workflow.partition(marker)
    assert separator == marker
    program, separator, _ = remainder.partition("\n          PY\n")
    assert separator == "\n          PY\n"
    return textwrap.dedent(program)


def write_apple_handoff_fixture(
    archive_path: Path,
    fixture_root: Path,
    *,
    extra_name: str | None = None,
) -> None:
    """Write a minimal handoff with every required XCFramework member."""

    xcframework = fixture_root / "NoritoBridge.xcframework"
    for relative in (
        "Info.plist",
        "NoritoBridge.artifacts.json",
        "ios-arm64/libNoritoBridge.a",
        "ios-arm64_x86_64-simulator/libNoritoBridge.a",
        "macos-arm64_x86_64/libNoritoBridge.a",
    ):
        path = xcframework / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f"fixture:{relative}\n".encode())
    manifest = fixture_root / "NoritoBridge.artifacts.json"
    manifest.symlink_to("NoritoBridge.xcframework/NoritoBridge.artifacts.json")

    with tarfile.open(archive_path, mode="w", format=tarfile.PAX_FORMAT) as archive:
        archive.dereference = False
        archive.add(
            xcframework,
            arcname="NoritoBridge.xcframework",
            recursive=True,
        )
        archive.add(
            manifest,
            arcname="NoritoBridge.artifacts.json",
            recursive=False,
        )
        if extra_name is not None:
            payload = b"escape"
            extra = tarfile.TarInfo(extra_name)
            extra.size = len(payload)
            archive.addfile(extra, io.BytesIO(payload))


def run_handoff_restore(
    archive: Path,
    destination: Path,
    source_root: Path,
) -> subprocess.CompletedProcess[str]:
    """Execute the workflow's isolated restore program against a fixture."""

    return subprocess.run(
        (
            sys.executable,
            "-I",
            "-S",
            "-B",
            "-",
            str(archive),
            hashlib.sha256(archive.read_bytes()).hexdigest(),
            str(destination),
            str(source_root),
        ),
        input=workflow_handoff_restore_program(),
        check=False,
        capture_output=True,
        text=True,
    )


def test_workflow_handoff_restore_preserves_only_the_manifest_symlink(
    tmp_path: Path,
) -> None:
    """The downloaded tar restores one exact, checker-ready artifact root."""

    source_root = tmp_path / "source"
    source_root.mkdir()
    fixture_root = tmp_path / "fixture"
    fixture_root.mkdir()
    archive = tmp_path / "NoritoBridge.xcframework.tar"
    write_apple_handoff_fixture(archive, fixture_root)
    destination = tmp_path / "restored"
    destination.mkdir()

    completed = run_handoff_restore(archive, destination, source_root)

    assert completed.returncode == 0, completed.stderr
    manifest = destination / "NoritoBridge.artifacts.json"
    assert manifest.is_symlink()
    assert manifest.readlink() == Path(
        "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
    )
    assert not any(
        path.is_symlink()
        for path in (destination / "NoritoBridge.xcframework").rglob("*")
    )


def test_workflow_handoff_restore_accepts_the_system_tar_output(
    tmp_path: Path,
) -> None:
    """The exact producer command emits a handoff accepted by the consumer."""

    source_root = tmp_path / "source"
    source_root.mkdir()
    fixture_root = tmp_path / "fixture"
    fixture_root.mkdir()
    seed_archive = tmp_path / "seed.tar"
    write_apple_handoff_fixture(seed_archive, fixture_root)
    seed_archive.unlink()
    archive = tmp_path / "NoritoBridge.xcframework.tar"
    environment = os.environ.copy()
    environment["COPYFILE_DISABLE"] = "1"
    subprocess.run(
        (
            "/usr/bin/tar",
            "-cf",
            str(archive),
            "NoritoBridge.xcframework",
            "NoritoBridge.artifacts.json",
        ),
        cwd=fixture_root,
        env=environment,
        check=True,
        capture_output=True,
    )
    destination = tmp_path / "restored"
    destination.mkdir()

    completed = run_handoff_restore(archive, destination, source_root)

    assert completed.returncode == 0, completed.stderr
    assert (destination / "NoritoBridge.artifacts.json").is_symlink()


def test_workflow_handoff_restore_rejects_traversal_before_extraction(
    tmp_path: Path,
) -> None:
    """A transferred tar cannot write outside its fresh artifact directory."""

    source_root = tmp_path / "source"
    source_root.mkdir()
    fixture_root = tmp_path / "fixture"
    fixture_root.mkdir()
    archive = tmp_path / "NoritoBridge.xcframework.tar"
    write_apple_handoff_fixture(archive, fixture_root, extra_name="../escape")
    destination = tmp_path / "restored"
    destination.mkdir()

    completed = run_handoff_restore(archive, destination, source_root)

    assert completed.returncode != 0
    assert "non-canonical path" in completed.stderr
    assert list(destination.iterdir()) == []
    assert not (tmp_path / "escape").exists()
