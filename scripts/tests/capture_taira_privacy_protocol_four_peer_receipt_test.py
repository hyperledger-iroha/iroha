from __future__ import annotations

import hashlib
from pathlib import Path

import pytest

from scripts import capture_taira_privacy_protocol_four_peer_receipt as capture


def _executable(path: Path, output: str) -> Path:
    path.write_text(
        "#!/bin/sh\n"
        "set -eu\n"
        f"printf '%s\\n' '{output}'\n",
        encoding="ascii",
    )
    path.chmod(0o700)
    return path


def _run_with_output(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    output: str,
) -> tuple[str, Path, Path]:
    repository = tmp_path / "repository"
    repository.mkdir()
    target = tmp_path / "target"
    target.mkdir()
    validator = _executable(tmp_path / "irohad", "validator")
    command = _executable(tmp_path / "case-command", output)
    log = tmp_path / "case.log"
    monkeypatch.setattr(
        capture,
        "_case_commands",
        lambda _case: ((str(command), "-p", "integration_tests"),),
    )
    digest = capture._run_case(
        "privacy-test::case",
        repository=repository,
        target_dir=target,
        validator_binary=validator,
        log_path=log,
    )
    return digest, validator, log


def test_case_log_binds_exact_candidate_and_requires_real_test(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    digest, validator, log = _run_with_output(
        tmp_path,
        monkeypatch,
        "running 1 test\n"
        "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:privacy-test::case:passed\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured",
    )

    payload = log.read_bytes()
    assert digest == hashlib.sha256(payload).hexdigest()
    assert (
        f"TEST_NETWORK_BIN_IROHAD_SHA256={hashlib.sha256(validator.read_bytes()).hexdigest()}"
        .encode()
        in payload
    )


@pytest.mark.parametrize(
    "output, message",
    (
        (
            "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored",
            "executed zero tests",
        ),
        (
            "running 1 test\nfixture-only evidence\n"
            "TAIRA_PRIVACY_PROTOCOL_FOUR_PEER_CASE_V1:privacy-test::case:passed\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured",
            "fixture-only",
        ),
        ("running 1 test\nno libtest result", "lacks an unskipped passing"),
        (
            "running 1 test\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured",
            "post-query/restart marker",
        ),
    ),
    ids=(
        "zero-tests",
        "fixture-only-marker",
        "missing-pass-marker",
        "forged-libtest-summary-without-marker",
    ),
)
def test_case_rejects_nonexecuted_or_fixture_only_success(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    output: str,
    message: str,
) -> None:
    with pytest.raises(capture.PrivacyProtocolReceiptError, match=message):
        _run_with_output(tmp_path, monkeypatch, output)
