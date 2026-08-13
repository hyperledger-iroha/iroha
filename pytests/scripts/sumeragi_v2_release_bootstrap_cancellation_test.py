"""Focused cooperative-cancellation contracts for the release bootstrap."""

from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from pytests.scripts.sumeragi_v2_release_bootstrap_test import (
    BOOTSTRAP,
    PYTHON,
    _load_bootstrap_module,
    _sha256,
    _write,
)


def test_bootstrap_supervision_has_no_forbidden_process_controls() -> None:
    source = BOOTSTRAP.read_text(encoding="utf-8")
    for forbidden in (
        "import signal",
        "os.kill(",
        "os.killpg(",
        ".kill(",
        ".terminate(",
        "start_new_session",
        "def _abort",
        "wait(timeout=",
    ):
        assert forbidden not in source


def test_cooperative_cancellation_publishes_distinct_retained_evidence(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    candidate = tmp_path / "candidate"
    control = tmp_path / "control"
    evidence = tmp_path / "evidence"
    for directory in (candidate, control, evidence):
        directory.mkdir(mode=0o700)
        directory.chmod(0o700)
    request_path = _write(
        control / "cancel.json",
        b'{"reason":"operator-request","schema_version":1}\n',
        0o600,
    )
    assert module._cancellation_control_path(
        {"IROHA_RELEASE_CANCEL_REQUEST_PATH": str(request_path)}, candidate
    ) == request_path

    identity_path = _write(evidence / "candidate-identity.json", b"{}\n", 0o400)
    bootstrap_path = _write(evidence / "BOOTSTRAP_COMPLETED.json", b"{}\n", 0o400)
    runner_path = _write(tmp_path / "runner.sh", "#!/bin/sh\nexit 125\n", 0o500)
    stdout_path = _write(evidence / "runner-stdout.log", b"finished\n", 0o400)
    stderr_path = _write(evidence / "runner-stderr.log", b"cancelled\n", 0o400)
    identity_snapshot = module._read_file(
        identity_path, "identity", maximum_bytes=module._MAX_IDENTITY_BYTES
    )
    bootstrap_snapshot = module._read_file(
        bootstrap_path, "bootstrap", maximum_bytes=module._MAX_EVIDENCE_BYTES
    )
    runner_snapshot = module._read_file(
        runner_path,
        "runner",
        maximum_bytes=module._MAX_HELPER_BYTES,
        executable=True,
    )
    logs = {
        "stdout": module._capture_large_file(stdout_path, "stdout"),
        "stderr": module._capture_large_file(stderr_path, "stderr"),
    }
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    evidence_fd = os.open(evidence, flags)
    try:
        marker = module._publish_cancellation_result(
            evidence=evidence,
            evidence_fd=evidence_fd,
            request_path=request_path,
            candidate=candidate,
            identity={"head_commit": "a" * 40, "head_tree": "b" * 40},
            identity_snapshot=identity_snapshot,
            bootstrap_marker=bootstrap_snapshot,
            runner_snapshot=runner_snapshot,
            runner_logs=logs,
        )
    finally:
        os.close(evidence_fd)

    value = json.loads(marker.data)
    assert marker.path == evidence / "BOOTSTRAP_CANCELLED.json"
    assert marker.mode == 0o400
    assert marker.nlink == 1
    assert value["result"] == "release-cancelled"
    assert value["reason"] == "operator-request"
    assert value["runner"]["exit_status"] == 125
    assert value["request"] == {
        "archive_id": "release-bootstrap.cancellation-request.v1",
        "sha256": _sha256(request_path),
        "size_bytes": request_path.stat().st_size,
        "mode": "0600",
        "owner_uid": os.getuid(),
        "nlink": 1,
    }
    assert str(request_path).encode() not in marker.data
    assert not (evidence / "BOOTSTRAP_RELEASE_COMPLETED.json").exists()


@pytest.mark.parametrize(
    ("request_bytes", "request_mode"),
    [
        (b'{"reason":"operator-request","schema_version":2}\n', 0o600),
        (b'{"reason":"operator-request","schema_version":1}\n', 0o644),
    ],
)
def test_cooperative_cancellation_rejects_noncanonical_request(
    tmp_path: Path, request_bytes: bytes, request_mode: int
) -> None:
    module = _load_bootstrap_module()
    control = tmp_path / "control"
    control.mkdir(mode=0o700)
    control.chmod(0o700)
    request = _write(control / "cancel.json", request_bytes, request_mode)

    with pytest.raises(module.BootstrapError, match="cancellation request"):
        module._read_cancellation_request(request)


def test_cooperative_cancellation_rejects_release_completion_coexistence(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    candidate = tmp_path / "candidate"
    control = tmp_path / "control"
    evidence = tmp_path / "evidence"
    for directory in (candidate, control, evidence):
        directory.mkdir(mode=0o700)
        directory.chmod(0o700)
    request = _write(
        control / "cancel.json",
        b'{"reason":"operator-request","schema_version":1}\n',
        0o600,
    )
    _write(evidence / "BOOTSTRAP_RELEASE_COMPLETED.json", b"attacker\n", 0o400)

    with pytest.raises(module.BootstrapError, match="cannot coexist"):
        module._publish_cancellation_result(
            evidence=evidence,
            evidence_fd=-1,
            request_path=request,
            candidate=candidate,
            identity={"head_commit": "a" * 40, "head_tree": "b" * 40},
            identity_snapshot=object(),
            bootstrap_marker=object(),
            runner_snapshot=object(),
            runner_logs={},
        )


def test_bounded_helper_observer_exception_waits_for_natural_completion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = _load_bootstrap_module()
    sentinel = tmp_path / "observer-exception-natural-completion"
    child = (
        "import time; from pathlib import Path; time.sleep(0.05); "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )
    real_monotonic = module.time.monotonic
    calls = 0

    def observer_clock() -> float:
        nonlocal calls
        calls += 1
        if calls == 2:
            raise RuntimeError("observer failed")
        return real_monotonic()

    monkeypatch.setattr(module.time, "monotonic", observer_clock)
    with pytest.raises(RuntimeError, match="observer failed"):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", child),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            timeout_seconds=5,
            maximum_output_bytes=1024,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"
