from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path

import pytest

from scripts import capture_taira_privacy_protocol_four_peer_receipt as capture

ROOT = Path(__file__).resolve().parents[2]


def _executable(path: Path, body: str) -> Path:
    path.write_text("#!/bin/sh\nset -eu\n" + body, encoding="ascii")
    path.chmod(0o700)
    return path


def test_bounded_native_driver_capture_preserves_complete_output(
    tmp_path: Path,
) -> None:
    driver = _executable(
        tmp_path / "driver",
        "printf '%s\\n' 'running 1 test' 'test privacy::case ... ok' "
        "'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured'\n",
    )
    output, status = capture._run_bounded(
        driver,
        (),
        environment={"PATH": os.environ["PATH"]},
        work_directory=tmp_path,
        timeout_seconds=5,
    )
    assert status == 0
    assert output.startswith(b"running 1 test\n")
    assert output.endswith(b"0 measured\n")


def test_root_frozen_driver_is_copied_to_digest_identical_runtime_executable(
    tmp_path: Path,
) -> None:
    source = tmp_path / "frozen-driver"
    source.write_bytes(b"#!/bin/sh\nexit 0\n")
    source.chmod(0o444)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    runtime = tmp_path / "runtime-driver"
    installed = capture._install_runtime_executable(source, runtime, digest)
    assert installed == runtime
    assert runtime.stat().st_mode & 0o777 == 0o500
    assert hashlib.sha256(runtime.read_bytes()).hexdigest() == digest


def test_native_driver_output_overrun_fails_closed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(capture.evidence, "MAX_COMMAND_OUTPUT_BYTES", 32)
    driver = _executable(
        tmp_path / "driver",
        "printf '%080d\\n' 0\n",
    )
    with pytest.raises(
        capture.PrivacyProtocolReceiptError, match="transcript bound"
    ):
        capture._run_bounded(
            driver,
            (),
            environment={"PATH": os.environ["PATH"]},
            work_directory=tmp_path,
            timeout_seconds=5,
        )


def test_native_driver_timeout_fails_closed(tmp_path: Path) -> None:
    driver = _executable(tmp_path / "driver", "sleep 5\n")
    with pytest.raises(capture.PrivacyProtocolReceiptError, match="exceeded"):
        capture._run_bounded(
            driver,
            (),
            environment={"PATH": os.environ["PATH"]},
            work_directory=tmp_path,
            timeout_seconds=1,
        )


def test_legacy_cargo_capture_surface_is_rejected() -> None:
    with pytest.raises(SystemExit):
        capture._parser().parse_args(
            [
                "--repository",
                "/tmp/repository",
                "--cargo-target-dir",
                "/tmp/target",
                "--output",
                "/tmp/privacy-protocol-four-peer-receipt-v1.json",
            ]
        )


def test_untrusted_build_compiles_but_cannot_issue_semantic_evidence() -> None:
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    build = workflow.split("  macos-native-build:\n", 1)[1].split(
        "  macos-secret-free-qualification:\n", 1
    )[0]
    qualification = workflow.split("  macos-secret-free-qualification:\n", 1)[1].split(
        "  macos-candidate-authority:\n", 1
    )[0]
    assert "cargo test --locked --release" in build
    assert "--bin privacy_exact12_action_driver" in build
    assert "--no-run --message-format=json" in build
    assert "capture_taira_privacy_protocol_four_peer_receipt.py" not in build
    assert "--privacy-network-driver" in qualification
    assert "--privacy-action-driver" in qualification
    assert "--privacy-protocol-receipt" not in qualification


def test_transitional_whole_test_capture_is_explicitly_fail_closed() -> None:
    source = (
        ROOT / "scripts/capture_taira_privacy_protocol_four_peer_receipt.py"
    ).read_text(encoding="utf-8")
    barrier = source.index("privacy protocol v2 issuance is closed")
    execution = source.index("case_rows = [")
    assert barrier < execution


def test_fail_closed_barrier_ignores_caller_claims_and_emits_no_evidence(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    paths = {}
    for name in (
        "irohad",
        "privacy-exact12-action-driver",
        "network-functional",
        "iroha-core-tests",
        "linux.tar.gz",
        "exact12.tsv",
    ):
        path = tmp_path / name
        path.write_bytes(f"{name}\n".encode())
        path.chmod(0o700)
        paths[name] = path
    source_identity = tmp_path / "taira-source-identity-v1.json"
    source_identity.write_bytes(
        capture.canonical_json_bytes(
            {
                "source": {
                    "cargo_lock_sha256": "11" * 32,
                    "commit": "22" * 20,
                    "dpn_validator_release_commit": "33" * 20,
                    "workspace_source_manifest_sha256": "44" * 32,
                },
                "source_date_epoch": 1,
            }
        )
    )
    output = tmp_path / "evidence"
    work = tmp_path / "work"
    monkeypatch.setenv("IROHA_ALLOW_DRIVER_OWNED_PRIVACY_RECEIPT", "1")
    args = argparse.Namespace(
        action_driver=paths["privacy-exact12-action-driver"],
        artifact_handoff_sha256="55" * 32,
        case_timeout_seconds=1,
        controller_owned_network=True,
        exact12_matrix=paths["exact12.tsv"],
        jindo_driver=paths["iroha-core-tests"],
        linux_archive=paths["linux.tar.gz"],
        network_driver=paths["network-functional"],
        output_directory=output,
        source_identity=source_identity,
        validator_binary=paths["irohad"],
        work_directory=work,
    )
    with pytest.raises(
        capture.PrivacyProtocolReceiptError,
        match="sealed controller does not yet own",
    ):
        capture.capture(args)
    assert not output.exists()
    assert not work.exists()
