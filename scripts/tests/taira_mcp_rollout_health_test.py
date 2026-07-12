from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "configs" / "soranexus" / "taira" / "check_mcp_rollout.sh"


def _sumeragi_snapshot_checker_source() -> str:
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"check_sumeragi_snapshot\(\) \{.*?"
        r"python3 - \"\$label\" \"\$last_body\" <<'PY'\n"
        r"(?P<body>.*?)\nPY\n\}",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _status_snapshot_checker_source() -> str:
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"check_status_snapshot\(\) \{.*?"
        r"python3 - \"\$label\" \"\$last_body\" \"\$MIN_VALIDATOR_SET_LEN\" "
        r"\"\$allow_pending_commit_qc\" \"\$EXPECTED_TAIRA_GIT_SHA\" <<'PY'\n"
        r"(?P<body>.*?)\nPY\n\}",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _run_checker(tmp_path: Path, payload: dict[str, object]) -> subprocess.CompletedProcess[str]:
    payload_path = tmp_path / "sumeragi-status.json"
    payload_path.write_text(json.dumps(payload), encoding="utf-8")
    return subprocess.run(
        ["python3", "-", "public", str(payload_path)],
        input=_sumeragi_snapshot_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _run_status_checker(
    tmp_path: Path,
    payload: dict[str, object],
    *,
    expected_git_sha: str = "",
) -> subprocess.CompletedProcess[str]:
    payload_path = tmp_path / "status.json"
    payload_path.write_text(json.dumps(payload), encoding="utf-8")
    return subprocess.run(
        ["python3", "-", "public", str(payload_path), "4", "0", expected_git_sha],
        input=_status_snapshot_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _healthy_base_payload() -> dict[str, object]:
    return {
        "protocol_version": 2,
        "node_fingerprint": "node-fingerprint",
        "build_fingerprint": "build-fingerprint",
        "config_fingerprint": "config-fingerprint",
        "height_context_id": ["height-context-id"],
        "height": 43,
        "view": 0,
        "phase": {"phase": "AwaitingProposal", "details": None},
        "leader": 0,
        "locked_prepare_qc": None,
        "highest_prepare_qc": None,
        "last_timeout_certificate": None,
        "body_state": {"state": "Applied", "details": None},
        "pending_persistence_id": None,
        "last_committed_height": 42,
        "last_committed_subject": {
            "parent_block_hash": "parent",
            "block_hash": "abc",
            "payload_hash": "payload",
        },
    }


def test_sumeragi_checker_accepts_authoritative_v2_status(tmp_path: Path) -> None:
    result = _run_checker(tmp_path, _healthy_base_payload())

    assert result.returncode == 0, result.stderr


def test_sumeragi_checker_rejects_legacy_rbc_status(tmp_path: Path) -> None:
    result = _run_checker(
        tmp_path,
        {"commit_qc": {"height": 42}, "pending_rbc": {"sessions": 0}},
    )

    assert result.returncode == 1
    assert "expected the Sumeragi v2 reducer status" in result.stderr


def test_sumeragi_checker_rejects_wrong_protocol_version(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["protocol_version"] = 1

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "expected the Sumeragi v2 reducer status" in result.stderr


def test_sumeragi_checker_rejects_missing_consensus_fingerprint(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    del payload["config_fingerprint"]

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "v2 status omitted required field(s): config_fingerprint" in result.stderr


def test_sumeragi_checker_rejects_invalid_numeric_state(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["view"] = -1

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "v2 status reported invalid view: -1" in result.stderr


def test_sumeragi_checker_rejects_committed_height_ahead_of_reducer(
    tmp_path: Path,
) -> None:
    payload = _healthy_base_payload()
    payload["height"] = 41

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "committed height 42 is ahead of reducer height 41" in result.stderr


def test_sumeragi_checker_requires_subject_after_first_commit(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["last_committed_subject"] = None

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "omitted last_committed_subject for a non-genesis commit" in result.stderr


def test_sumeragi_checker_rejects_zero_pending_persistence_id(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["pending_persistence_id"] = 0

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "invalid pending persistence id: 0" in result.stderr


def test_sumeragi_checker_accepts_positive_pending_persistence_id(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["pending_persistence_id"] = 9

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 0, result.stderr


def test_status_checker_accepts_expected_git_sha_prefix(tmp_path: Path) -> None:
    payload = {
        "build": {"git_commit_sha": "490dacc287f00d"},
        "blocks": 42,
        "queue_size": 0,
        "sumeragi": {
            "commit_qc_height": 42,
            "highest_qc_height": 42,
            "locked_qc_height": 42,
        },
    }

    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")

    assert result.returncode == 0, result.stderr


def test_status_checker_accepts_full_expected_git_sha_with_short_published_sha(
    tmp_path: Path,
) -> None:
    payload = {
        "build": {"git_commit_sha": "490dacc"},
        "blocks": 42,
        "queue_size": 0,
        "sumeragi": {
            "commit_qc_height": 42,
            "highest_qc_height": 42,
            "locked_qc_height": 42,
        },
    }

    result = _run_status_checker(
        tmp_path,
        payload,
        expected_git_sha="490dacc287f00d",
    )

    assert result.returncode == 0, result.stderr


def test_status_checker_rejects_missing_expected_git_sha(tmp_path: Path) -> None:
    payload = {
        "blocks": 42,
        "queue_size": 0,
        "sumeragi": {
            "commit_qc_height": 42,
            "highest_qc_height": 42,
            "locked_qc_height": 42,
        },
    }

    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")

    assert result.returncode == 1
    assert "did not publish build.git_commit_sha" in result.stderr


def test_status_checker_rejects_mismatched_expected_git_sha(tmp_path: Path) -> None:
    payload = {
        "build": {"git_commit_sha": "94dcbf7c28a46d"},
        "blocks": 42,
        "queue_size": 0,
        "sumeragi": {
            "commit_qc_height": 42,
            "highest_qc_height": 42,
            "locked_qc_height": 42,
        },
    }

    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")

    assert result.returncode == 1
    assert "build git SHA 94dcbf7c28a46d does not match expected 490dacc" in result.stderr


def test_status_checker_rejects_too_short_published_expected_git_sha(
    tmp_path: Path,
) -> None:
    payload = {
        "build": {"git_commit_sha": "490dac"},
        "blocks": 42,
        "queue_size": 0,
        "sumeragi": {
            "commit_qc_height": 42,
            "highest_qc_height": 42,
            "locked_qc_height": 42,
        },
    }

    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")

    assert result.returncode == 1
    assert "is not a 7 to 40 character hexadecimal SHA prefix" in result.stderr


def test_status_checker_rejects_non_hex_published_expected_git_sha(
    tmp_path: Path,
) -> None:
    payload = {
        "build": {"git_commit_sha": "490dacz"},
        "blocks": 42,
        "queue_size": 0,
        "sumeragi": {
            "commit_qc_height": 42,
            "highest_qc_height": 42,
            "locked_qc_height": 42,
        },
    }

    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")

    assert result.returncode == 1
    assert "is not a 7 to 40 character hexadecimal SHA prefix" in result.stderr


def test_status_checker_leaves_finality_to_detailed_sumeragi_status(
    tmp_path: Path,
) -> None:
    payload = {
        "build": {"git_commit_sha": "490dacc287f00d"},
        "blocks": 9532,
        "queue_size": 149,
        "sumeragi": {
            "commit_qc": {"height": 9532},
            "highest_qc": {"height": 6544},
            "locked_qc": {"height": 9532},
        },
    }

    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")

    assert result.returncode == 0, result.stderr
