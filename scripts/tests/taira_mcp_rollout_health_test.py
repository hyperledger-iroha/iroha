from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "configs" / "soranexus" / "taira" / "check_mcp_rollout.sh"


def _embedded_checker_source(function: str, invocation: str) -> str:
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        rf"{function}\(\) \{{.*?{re.escape(invocation)}\n(?P<body>.*?)\nPY\n\}}",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _sumeragi_snapshot_checker_source() -> str:
    return _embedded_checker_source(
        "check_sumeragi_snapshot",
        'python3 - "$label" "$last_body" "$MIN_VALIDATOR_SET_LEN" "$allow_pending_commit_qc" <<\'PY\'',
    )


def _status_snapshot_checker_source() -> str:
    return _embedded_checker_source(
        "check_status_snapshot",
        'python3 - "$label" "$last_body" "$MIN_VALIDATOR_SET_LEN" "$allow_pending_commit_qc" "$EXPECTED_TAIRA_GIT_SHA" "$REQUIRE_EXACT_GIT_SHA" <<\'PY\'',
    )


def _validator_fleet_checker_source() -> str:
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"capture_validator_fleet_sample\(\) \{.*?"
        r"python3 - \"\$records_file\" <<'PY'\n(?P<body>.*?)\nPY\n"
        r"  local rc=\$\?",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _validator_progress_checker_source() -> str:
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"python3 - \"\$previous_summary\" \"\$summary\" <<'PY'\n"
        r"(?P<body>.*?)\nPY",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _fleet_record(label: str, node: str) -> dict[str, object]:
    return {
        "label": label,
        "node": node,
        "build": "build",
        "config": "config",
        "context": "context",
        "height": 708,
        "view": 0,
        "epoch": 1,
        "mode": "permissioned",
        "validator_count": 4,
        "quorum": "3/4",
        "status_blocks": 707,
        "committed_height": 707,
        "committed_block_hash": "ab" * 32,
        "committed_subject": "block-707",
        "commit_qc": "qc-707",
        "dataspace_catalog": json.dumps(
            {
                "catalog_hash": "hash:" + "cd" * 32,
                "dataspaces": {
                    "boi-mobile": 8477022798449861195,
                    "external-poc": 6647857470246403404,
                },
            },
            sort_keys=True,
            separators=(",", ":"),
        ),
    }


def _run_fleet_checker(
    tmp_path: Path,
    records: list[dict[str, object]],
) -> subprocess.CompletedProcess[str]:
    records_path = tmp_path / "fleet-records.jsonl"
    records_path.write_text(
        "".join(json.dumps(record) + "\n" for record in records),
        encoding="utf-8",
    )
    return subprocess.run(
        ["python3", "-", str(records_path)],
        input=_validator_fleet_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _run_progress_checker(
    previous: dict[str, object],
    current: dict[str, object],
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", "-", json.dumps(previous), json.dumps(current)],
        input=_validator_progress_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _run_checker(
    tmp_path: Path,
    payload: dict[str, object],
    *,
    allow_pending: bool = False,
) -> subprocess.CompletedProcess[str]:
    payload_path = tmp_path / "sumeragi-status.json"
    payload_path.write_text(json.dumps(payload), encoding="utf-8")
    return subprocess.run(
        [
            "python3",
            "-",
            "public",
            str(payload_path),
            "4",
            "1" if allow_pending else "0",
        ],
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
    require_exact: bool = False,
) -> subprocess.CompletedProcess[str]:
    payload_path = tmp_path / "status.json"
    payload_path.write_text(json.dumps(payload), encoding="utf-8")
    return subprocess.run(
        [
            "python3",
            "-",
            "public",
            str(payload_path),
            "4",
            "0",
            expected_git_sha,
            "1" if require_exact else "0",
        ],
        input=_status_snapshot_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _healthy_base_payload() -> dict[str, object]:
    subject = {
        "block_hash": "hash:" + "E" * 64,
        "payload_hash": "hash:" + "F" * 64,
    }
    return {
        "protocol_version": 4,
        "restart_required": False,
        "node_fingerprint": "hash:" + "A" * 64,
        "build_fingerprint": "hash:" + "B" * 64,
        "config_fingerprint": "hash:" + "C" * 64,
        "height_context_id": ["hash:" + "D" * 64],
        "height": 43,
        "view": 2,
        "phase": {"phase": "prepare", "details": None},
        "leader": 1,
        "body_state": {"state": "missing", "details": None},
        "last_committed_height": 42,
        "last_committed_subject": subject,
        "height_context": {
            "epoch": 3,
            "epoch_end_height": 100,
            "mode": {"mode": "permissioned", "details": None},
            "epoch_seed": "11" * 32,
            "validator_count": 4,
            "quorum": {"min_signers": 3, "total_power": 4},
        },
        "last_commit_qc": {
            "certificate": {
                "round": {"height": 42, "view": 1},
                "phase": {"phase": "commit", "details": None},
                "subject": subject,
            },
            "validator_count": 4,
            "signer_count": 3,
            "min_signers": 3,
            "signed_power": 3,
            "total_power": 4,
        },
        "lane_settlement_commitments": [],
        "lane_relay_envelopes": [],
        "lane_payload_ownerships": [],
        "committed_lane_blocks": [],
        "lane_block_sessions": [],
        "local_peer_removed": False,
        "operator": {
            "view_change_install_total": 2,
            "busy_deferral_total": 1,
            "adapter_queues": {
                "ingress_keys": 0,
                "ingress_capacity": 64,
                "deferred_completion": 0,
                "deferred_progress": 0,
                "deferred_progress_capacity": 64,
                "deferred_normal": 0,
                "deferred_normal_capacity": 64,
            },
            "tx_queue": {
                "tracked_transactions": 2,
                "queued_transactions": 1,
                "capacity": 100,
                "retained_bytes": 128,
                "max_retained_bytes": 8192,
                "oldest_queued_age_ms": 5,
                "saturated_by_count": False,
                "saturated_by_bytes": False,
                "saturated_by_age": False,
            },
        },
    }


def test_sumeragi_checker_accepts_authoritative_v2(tmp_path: Path) -> None:
    result = _run_checker(tmp_path, _healthy_base_payload())
    assert result.returncode == 0, result.stderr
    assert '"commit_qc_signers": 3' in result.stdout


def test_validator_fleet_gate_retains_exact_dataspace_and_commit_identity(
    tmp_path: Path,
) -> None:
    records = [_fleet_record("v1", "node-1"), _fleet_record("v2", "node-2")]
    result = _run_fleet_checker(tmp_path, records)

    assert result.returncode == 0, result.stderr
    summary = json.loads(result.stdout)
    assert summary["status_blocks"] == summary["committed_height"] == 707
    assert summary["committed_block_hash"] == "ab" * 32
    catalog = json.loads(summary["dataspace_catalog"])
    assert catalog["dataspaces"] == {
        "boi-mobile": 8477022798449861195,
        "external-poc": 6647857470246403404,
    }


def test_validator_fleet_gate_rejects_commit_and_dataspace_mismatches(
    tmp_path: Path,
) -> None:
    baseline = _fleet_record("v1", "node-1")

    wrong_status_blocks = _fleet_record("v2", "node-2")
    wrong_status_blocks["status_blocks"] = 706
    result = _run_fleet_checker(tmp_path, [baseline, wrong_status_blocks])
    assert result.returncode == 1
    assert "status_blocks" in result.stderr

    wrong_block = _fleet_record("v2", "node-2")
    wrong_block["committed_block_hash"] = "ef" * 32
    result = _run_fleet_checker(tmp_path, [baseline, wrong_block])
    assert result.returncode == 1
    assert "committed_block_hash" in result.stderr

    wrong_catalog = _fleet_record("v2", "node-2")
    catalog = json.loads(str(wrong_catalog["dataspace_catalog"]))
    catalog["dataspaces"]["boi-mobile"] = 9
    wrong_catalog["dataspace_catalog"] = json.dumps(
        catalog,
        sort_keys=True,
        separators=(",", ":"),
    )
    result = _run_fleet_checker(tmp_path, [baseline, wrong_catalog])
    assert result.returncode == 1
    assert "dataspace_catalog" in result.stderr


def test_validator_progress_gate_requires_stable_catalog_and_advancing_commit() -> None:
    previous = {
        "build": "build",
        "config": "config",
        "nodes": ["node-1", "node-2"],
        "dataspace_catalog": "is-and-is2",
        "status_blocks": 707,
        "committed_height": 707,
        "committed_block_hash": "ab" * 32,
        "committed_subject": "block-707",
    }
    current = {
        **previous,
        "status_blocks": 708,
        "committed_height": 708,
        "committed_block_hash": "cd" * 32,
        "committed_subject": "block-708",
    }
    accepted = _run_progress_checker(previous, current)
    assert accepted.returncode == 0, accepted.stderr

    changed_catalog = {**current, "dataspace_catalog": "is-only"}
    rejected = _run_progress_checker(previous, changed_catalog)
    assert rejected.returncode == 1
    assert "changed dataspace_catalog between progress samples" in rejected.stderr

    stale_status = {**current, "status_blocks": 707}
    rejected = _run_progress_checker(previous, stale_status)
    assert rejected.returncode == 1
    assert "/status.blocks did not advance" in rejected.stderr

    stale_hash = {**current, "committed_block_hash": previous["committed_block_hash"]}
    rejected = _run_progress_checker(previous, stale_hash)
    assert rejected.returncode == 1
    assert "without changing the common block hash" in rejected.stderr


def test_sumeragi_checker_rejects_legacy_shape(tmp_path: Path) -> None:
    result = _run_checker(
        tmp_path,
        {"commit_qc": {"height": 42}, "canonical": {"height": 43}},
    )
    assert result.returncode == 1
    assert "legacy RBC/recovery status is not accepted" in result.stderr


def test_sumeragi_checker_rejects_noncanonical_tag_and_seed(tmp_path: Path) -> None:
    pascal_case = _healthy_base_payload()
    pascal_case["phase"] = {"phase": "Prepare", "details": None}
    result = _run_checker(tmp_path, pascal_case)
    assert result.returncode == 1
    assert "invalid phase tag" in result.stderr

    extra_field = _healthy_base_payload()
    extra_field["phase"] = {
        "phase": "prepare",
        "details": None,
        "unexpected": True,
    }
    result = _run_checker(tmp_path, extra_field)
    assert result.returncode == 1
    assert "phase is not a canonical tagged unit" in result.stderr

    array_seed = _healthy_base_payload()
    array_seed["height_context"]["epoch_seed"] = [17] * 32  # type: ignore[index]
    result = _run_checker(tmp_path, array_seed)
    assert result.returncode == 1
    assert "epoch-seed hex string" in result.stderr


def test_sumeragi_checker_rejects_commit_identity_mismatch(tmp_path: Path) -> None:
    wrong_height = _healthy_base_payload()
    wrong_height["last_commit_qc"]["certificate"]["round"]["height"] = 41  # type: ignore[index]
    result = _run_checker(tmp_path, wrong_height)
    assert result.returncode == 1
    assert "CommitQC height does not match" in result.stderr

    wrong_subject = _healthy_base_payload()
    wrong_subject["last_commit_qc"]["certificate"]["subject"] = {  # type: ignore[index]
        "block_hash": "hash:" + "0" * 64,
        "payload_hash": "hash:" + "1" * 64,
    }
    result = _run_checker(tmp_path, wrong_subject)
    assert result.returncode == 1
    assert "CommitQC subject does not match" in result.stderr


def test_sumeragi_checker_rejects_underpowered_commit_qc(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["last_commit_qc"]["signed_power"] = 2  # type: ignore[index]
    result = _run_checker(tmp_path, payload)
    assert result.returncode == 1
    assert "does not satisfy its frozen dual quorum" in result.stderr


def test_sumeragi_checker_rejects_context_and_operator_bounds(tmp_path: Path) -> None:
    bad_leader = _healthy_base_payload()
    bad_leader["leader"] = 4
    result = _run_checker(tmp_path, bad_leader)
    assert result.returncode == 1
    assert "outside frozen validator roster" in result.stderr

    bad_adapter = _healthy_base_payload()
    bad_adapter["operator"]["adapter_queues"]["ingress_keys"] = 65  # type: ignore[index]
    result = _run_checker(tmp_path, bad_adapter)
    assert result.returncode == 1
    assert "adapter queue ingress_keys exceeds" in result.stderr

    bad_tx_queue = _healthy_base_payload()
    bad_tx_queue["operator"]["tx_queue"]["queued_transactions"] = 3  # type: ignore[index]
    result = _run_checker(tmp_path, bad_tx_queue)
    assert result.returncode == 1
    assert "transaction queue occupancy exceeds" in result.stderr


def test_sumeragi_checker_requires_all_lane_evidence_arrays(tmp_path: Path) -> None:
    for field in (
        "lane_settlement_commitments",
        "lane_relay_envelopes",
        "lane_payload_ownerships",
        "committed_lane_blocks",
        "lane_block_sessions",
    ):
        payload = _healthy_base_payload()
        del payload[field]
        result = _run_checker(tmp_path, payload)
        assert result.returncode == 1
        assert f"omitted required {field} array" in result.stderr


def test_sumeragi_checker_only_allows_genesis_without_qc_during_bootstrap(
    tmp_path: Path,
) -> None:
    payload = _healthy_base_payload()
    payload["last_committed_height"] = 0
    payload.pop("last_committed_subject")
    payload.pop("last_commit_qc")

    strict = _run_checker(tmp_path, payload)
    assert strict.returncode == 1
    assert "has not published a durable CommitQC" in strict.stderr

    bootstrap = _run_checker(tmp_path, payload, allow_pending=True)
    assert bootstrap.returncode == 0, bootstrap.stderr


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


def test_sumeragi_checker_requires_boolean_restart_required(tmp_path: Path) -> None:
    missing = _healthy_base_payload()
    missing.pop("restart_required")

    result = _run_checker(tmp_path, missing)

    assert result.returncode == 1
    assert "restart_required must be a boolean" in result.stderr

    invalid = _healthy_base_payload()
    invalid["restart_required"] = 0

    result = _run_checker(tmp_path, invalid)

    assert result.returncode == 1
    assert "restart_required must be a boolean" in result.stderr


def test_sumeragi_checker_accepts_restart_required_state(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["restart_required"] = True

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 0, result.stderr


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
    assert "omitted required last_committed_subject object" in result.stderr


def test_sumeragi_checker_rejects_zero_pending_persistence_id(tmp_path: Path) -> None:
    payload = _healthy_base_payload()
    payload["pending_persistence_id"] = 0

    result = _run_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "invalid pending_persistence_id: 0" in result.stderr


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
    }
    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")
    assert result.returncode == 0, result.stderr


def test_release_status_checker_requires_full_exact_git_sha(tmp_path: Path) -> None:
    expected = "490dacc287f00d490dacc287f00d490dacc287f0"
    exact = _run_status_checker(
        tmp_path,
        {"build": {"git_commit_sha": expected}, "blocks": 42, "queue_size": 0},
        expected_git_sha=expected,
        require_exact=True,
    )
    assert exact.returncode == 0, exact.stderr

    shortened = _run_status_checker(
        tmp_path,
        {
            "build": {"git_commit_sha": expected[:12]},
            "blocks": 42,
            "queue_size": 0,
        },
        expected_git_sha=expected,
        require_exact=True,
    )
    assert shortened.returncode == 1
    assert "does not exactly match release commit" in shortened.stderr


def test_status_checker_rejects_missing_or_mismatched_git_sha(tmp_path: Path) -> None:
    missing = _run_status_checker(
        tmp_path,
        {"blocks": 42, "queue_size": 0},
        expected_git_sha="490dacc",
    )
    assert missing.returncode == 1
    assert "did not publish build.git_commit_sha" in missing.stderr

    mismatch = _run_status_checker(
        tmp_path,
        {
            "build": {"git_commit_sha": "94dcbf7c28a46d"},
            "blocks": 42,
            "queue_size": 0,
        },
        expected_git_sha="490dacc",
    )
    assert mismatch.returncode == 1
    assert "does not match expected" in mismatch.stderr


def test_status_checker_leaves_consensus_semantics_to_v2_route(tmp_path: Path) -> None:
    payload = {
        "build": {"git_commit_sha": "490dacc287f00d"},
        "blocks": 9532,
        "queue_size": 149,
        "sumeragi": {"commit_qc_height": "malformed legacy field is ignored"},
    }
    result = _run_status_checker(tmp_path, payload, expected_git_sha="490dacc")
    assert result.returncode == 0, result.stderr
