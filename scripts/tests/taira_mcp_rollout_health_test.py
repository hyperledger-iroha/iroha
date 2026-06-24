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
        r"python3 - \"\$label\" \"\$last_body\" \"\$MIN_VALIDATOR_SET_LEN\" "
        r"\"\$allow_pending_commit_qc\" <<'PY'\n"
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
        ["python3", "-", "public", str(payload_path), "4", "0"],
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
        "commit_qc": {"height": 42, "validator_set_len": 4},
        "highest_qc": {"height": 42},
        "locked_qc": {"height": 42},
        "canonical": {"height": 42},
        "membership": {"height": 42},
        "tx_queue": {"depth": 0, "capacity": 20_000, "saturated": False},
        "view_change_causes": {"last_cause": None},
    }


def test_sumeragi_checker_allows_low_depth_legacy_saturated_queue(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["tx_queue"] = {
        "depth": 147,
        "capacity": 20_000,
        "saturated": True,
    }

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


def test_status_checker_rejects_nested_highest_qc_behind_commit(
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

    assert result.returncode == 1
    assert "highest QC height 6544 is behind commit QC height 9532" in result.stderr


def test_sumeragi_checker_rejects_explicit_count_saturation(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["tx_queue"] = {
        "depth": 20_000,
        "capacity": 20_000,
        "saturated": True,
        "saturated_by_count": True,
        "saturated_by_age": False,
    }

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "capacity saturation" in result.stderr


def test_sumeragi_checker_reports_finality_fault_before_queue_blame(tmp_path: Path) -> None:
    payload = {
        "commit_qc": {"height": 9532, "validator_set_len": 4},
        "highest_qc": {"height": 9532},
        "locked_qc": {"height": 9532},
        "canonical": {"height": 9533},
        "membership": {"height": 9533},
        "tx_queue": {
            "depth": 148,
            "capacity": 20_000,
            "saturated": True,
        },
        "view_change_causes": {"last_cause": "quorum_timeout"},
    }

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "finality fault" in result.stderr
    assert "quorum_timeout" in result.stderr
    assert "capacity saturation" not in result.stderr


def test_sumeragi_checker_rejects_membership_ahead_without_last_cause(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["commit_qc"] = {"height": 51, "validator_set_len": 4}
    payload["highest_qc"] = {"height": 51}
    payload["locked_qc"] = {"height": 51}
    payload["canonical"] = {"height": 52}
    payload["membership"] = {"height": 52}
    payload["view_change_causes"] = {"last_cause": None}

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "finality fault" in result.stderr
    assert "unknown" in result.stderr
    assert "52 > 51" in result.stderr


def test_sumeragi_checker_reports_finality_before_count_saturation(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["commit_qc"] = {"height": 51, "validator_set_len": 4}
    payload["highest_qc"] = {"height": 51}
    payload["locked_qc"] = {"height": 51}
    payload["canonical"] = {"height": 52}
    payload["membership"] = {"height": 52}
    payload["tx_queue"] = {
        "depth": 20_000,
        "capacity": 20_000,
        "saturated": True,
        "saturated_by_count": True,
        "saturated_by_age": False,
    }
    payload["view_change_causes"] = {"last_cause": None}

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "finality fault" in result.stderr
    assert "52 > 51" in result.stderr
    assert "capacity saturation" not in result.stderr


def test_sumeragi_checker_rejects_locked_qc_behind_commit(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["commit_qc"] = {"height": 9532, "validator_set_len": 4}
    payload["highest_qc"] = {"height": 9532}
    payload["locked_qc"] = {"height": 6544}
    payload["tx_queue"] = {
        "depth": 20_000,
        "capacity": 20_000,
        "saturated": True,
        "saturated_by_count": True,
        "saturated_by_age": False,
    }

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "locked QC height 6544 is behind commit QC height 9532" in result.stderr
    assert "capacity saturation" not in result.stderr


def test_sumeragi_checker_allows_age_only_queue_pressure(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["tx_queue"] = {
        "depth": 4,
        "capacity": 20_000,
        "saturated": False,
        "saturated_by_count": False,
        "saturated_by_age": True,
        "oldest_queued_age_ms": 600_000,
    }

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 0, result.stderr
