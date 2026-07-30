"""Tests for the Cargo-free Swift offline-attestation fixture checker."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
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
        + "sorau\u30ed1fixture".encode()
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
        "account_id": "sorau\u30ed1fixture",
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
    assert (
        "IrohaSwift/Tests/IrohaSwiftTests/Fixtures/"
        "offline_device_attestation_abi21.json"
    ) in workflow
    assert "OfflineDeviceAttestationABI21ParityTests.swift" in swift_gate
    assert "--filter OfflineDeviceAttestationABI21ParityTests" in swift_gate
