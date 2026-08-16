from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "configs" / "soranexus" / "taira" / "check_mcp_rollout.sh"
DPN_COMMIT = "d" * 40


def _manifest_bindings(dataspace: str, validators: list[str]) -> list[dict[str, str]]:
    return [
        {
            "validator": validator,
            "peer_id": f"{dataspace}-peer-{index}",
            "torii_url": f"https://{dataspace}-validator-{index}.test",
        }
        for index, validator in enumerate(validators, start=1)
    ]


def test_rollout_has_one_absolute_deadline_for_network_and_canary_waits() -> None:
    source = SCRIPT.read_text(encoding="utf-8")

    assert 'ROLLOUT_DEADLINE_SECONDS="${ROLLOUT_DEADLINE_SECONDS:-240}"' in source
    assert "--deadline-seconds)" in source
    assert "ROLLOUT_DEADLINE_AT_SECONDS - SECONDS" in source
    assert source.count("clamp_seconds_to_rollout_deadline") >= 3
    assert source.count("clamp_status_timeout_ms_to_rollout_deadline") >= 4
    assert '--status-timeout-ms "$status_timeout_ms"' in source
    assert '"$status_timeout_ms"\n  write_msg=' in source

    direct_sleeps = [
        line.strip()
        for line in source.splitlines()
        if re.match(r"^\s*sleep(?:\s|$)", line)
    ]
    assert direct_sleeps == ['sleep "$requested"']
    assert 'VALIDATOR_ALIGNMENT_ATTEMPTS="${VALIDATOR_ALIGNMENT_ATTEMPTS:-2}"' in source


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
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"check_status_snapshot\(\) \{.*?"
        r"python3 - \"\$label\" \"\$last_body\" .*? <<'PY'\n"
        r"(?P<body>.*?)\nPY\n  snapshot_rc=\$\?",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _effective_routing_policy_checker_source() -> str:
    return _embedded_checker_source(
        "check_effective_routing_policy",
        '  python3 - "$label" "$status_path" "$EXPECTED_TAIRA_ROUTING_POLICY_JSON" <<\'PY\'',
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


def _lane_dataspace_topology_checker_source() -> str:
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"check_taira_lane_dataspace_topology\(\) \{.*?<<'PY'\n"
        r"(?P<body>.*?)\nPY\n\}",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _canonical_config_topology_checker_source() -> str:
    source = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        r"validate_canonical_taira_topology\(\) \{.*?<<'PY'\n"
        r"(?P<body>.*?)\nPY\n\}",
        source,
        re.DOTALL,
    )
    assert match is not None
    return match.group("body")


def _physical_dataspace_roster_checker_source() -> str:
    return _embedded_checker_source(
        "check_taira_physical_dataspace_rosters",
        '  python3 - "$label" "$status_path" "$sumeragi_path" "$dataspace_summary" <<\'PY\'',
    )


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
                "lane_count": 7,
                "lanes": [
                    {"id": 0, "alias": "core", "dataspace_id": 0},
                    {"id": 1, "alias": "governance", "dataspace_id": 0},
                    {"id": 2, "alias": "zk", "dataspace_id": 0},
                    {"id": 3, "alias": "dpn", "dataspace_id": 10},
                    {
                        "id": 4,
                        "alias": "external-poc",
                        "dataspace_id": 6647857470246403404,
                    },
                    {
                        "id": 5,
                        "alias": "boi-mobile",
                        "dataspace_id": 8477022798449861195,
                    },
                    {"id": 6, "alias": "cbsi", "dataspace_id": 20},
                ],
                "dataspaces": {
                    "universal": 0,
                    "dpn": 10,
                    "is": 6647857470246403404,
                    "is2": 8477022798449861195,
                    "cbsi": 20,
                },
            },
            sort_keys=True,
            separators=(",", ":"),
        ),
        "dataspace_rosters": json.dumps(
            {
                alias: {
                    "source": "lane_manifest",
                    "members": [f"{alias}-validator-{index}" for index in range(1, 5)],
                    "bindings": _manifest_bindings(
                        alias,
                        [f"{alias}-validator-{index}" for index in range(1, 5)],
                    ),
                    "quorum": 3,
                }
                for alias in ("universal", "dpn", "is", "is2", "cbsi")
            },
            sort_keys=True,
            separators=(",", ":"),
        ),
        "dpn_validator_release_commit": DPN_COMMIT,
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


def _run_lane_dataspace_topology_checker(
    tmp_path: Path,
    payload: dict[str, object],
) -> subprocess.CompletedProcess[str]:
    payload_path = tmp_path / "nexus-lifecycle.json"
    payload_path.write_text(json.dumps(payload), encoding="utf-8")
    return subprocess.run(
        [
            "python3",
            "-",
            "public",
            str(payload_path),
            "7",
            "0",
            "10",
            "external-poc",
            "6647857470246403404",
            "boi-mobile",
            "8477022798449861195",
            "20",
            "core",
            "governance",
            "zk",
            "dpn",
            "cbsi",
        ],
        input=_lane_dataspace_topology_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _run_canonical_config_topology_checker(
    config_path: Path,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "python3",
            "-",
            str(config_path),
            "7",
            "0",
            "10",
            "6647857470246403404",
            "8477022798449861195",
            "20",
            "core",
            "governance",
            "zk",
            "dpn",
            "external-poc",
            "boi-mobile",
            "cbsi",
        ],
        input=_canonical_config_topology_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _run_physical_dataspace_roster_checker(
    tmp_path: Path,
    status_payload: dict[str, object],
    sumeragi_payload: dict[str, object],
) -> subprocess.CompletedProcess[str]:
    status_path = tmp_path / "status.json"
    sumeragi_path = tmp_path / "sumeragi-status.json"
    status_path.write_text(json.dumps(status_payload), encoding="utf-8")
    sumeragi_path.write_text(json.dumps(sumeragi_payload), encoding="utf-8")
    topology_payload = _healthy_lane_dataspace_topology()
    topology_payload["dataspaces"] = {
        "universal": 0,
        "dpn": 10,
        "is": 6647857470246403404,
        "is2": 8477022798449861195,
        "cbsi": 20,
    }
    topology = json.dumps(topology_payload, separators=(",", ":"))
    return subprocess.run(
        ["python3", "-", "validator-1", str(status_path), str(sumeragi_path), topology],
        input=_physical_dataspace_roster_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _run_effective_routing_policy_checker(
    tmp_path: Path,
    status_payload: dict[str, object],
) -> subprocess.CompletedProcess[str]:
    status_path = tmp_path / "routing-status.json"
    status_path.write_text(json.dumps(status_payload), encoding="utf-8")
    expected_routing = json.dumps(
        _expected_routing_policy_summary(),
        separators=(",", ":"),
    )
    return subprocess.run(
        ["python3", "-", "status", str(status_path), expected_routing],
        input=_effective_routing_policy_checker_source(),
        text=True,
        capture_output=True,
        check=False,
    )


def _healthy_lane_dataspace_topology() -> dict[str, object]:
    return {
        "version": 1,
        "nexus_enabled": True,
        "lane_count": 7,
        "lanes": [
            {"id": 0, "alias": "core", "dataspace_id": 0},
            {"id": 1, "alias": "governance", "dataspace_id": 0},
            {"id": 2, "alias": "zk", "dataspace_id": 0},
            {"id": 3, "alias": "dpn", "dataspace_id": 10},
            {
                "id": 4,
                "alias": "external-poc",
                "dataspace_id": 6647857470246403404,
            },
            {
                "id": 5,
                "alias": "boi-mobile",
                "dataspace_id": 8477022798449861195,
            },
            {"id": 6, "alias": "cbsi", "dataspace_id": 20},
        ],
        "catalog_hash": "hash:" + "c" * 64,
    }


def _canonical_routing_rule_tuples() -> list[list[object]]:
    return [
        [3, 10, "account", "*@dpn"],
        [4, 6647857470246403404, "account", "*@wonderland.is"],
        [5, 8477022798449861195, "account", "*@boi.is2"],
        [5, 8477022798449861195, "account", "*@leumi.is2"],
        [5, 8477022798449861195, "account", "*@hapoalim.is2"],
        [5, 8477022798449861195, "account", "*@discount.is2"],
        [5, 8477022798449861195, "account", "*@mizrahi.is2"],
        [5, 8477022798449861195, "account", "*@fibi.is2"],
        [5, 8477022798449861195, "account", "*@onezero.is2"],
        [5, 8477022798449861195, "account", "*@jerusalem.is2"],
        [6, 20, "account", "*@cbsi"],
        [6, 20, "account", "*@pob.cbsi"],
        [6, 20, "account", "*@bred.cbsi"],
        [6, 20, "account", "*@anz.cbsi"],
        [6, 20, "account", "*@bsp.cbsi"],
        [6, 20, "account", "*@m-selen.cbsi"],
        [6, 20, "account", "*@ezipei.cbsi"],
        [1, 0, "instruction", "governance"],
        [2, 0, "instruction", "smartcontract::deploy"],
    ]


def _expected_routing_policy_summary() -> dict[str, object]:
    return {
        "default_lane": 0,
        "default_dataspace": 0,
        "rules": _canonical_routing_rule_tuples(),
    }


def _healthy_routing_policy() -> dict[str, object]:
    rules = []
    for position, (lane_id, dataspace_id, matcher_kind, matcher_value) in enumerate(
        _canonical_routing_rule_tuples()
    ):
        rules.append(
            {
                "lane": lane_id,
                "dataspace_id": dataspace_id,
                "matcher": {
                    matcher_kind: matcher_value,
                    "description": f"canonical Taira routing rule {position}",
                },
            }
        )
    return {
        "default_lane": 0,
        "default_dataspace": 0,
        "rules": rules,
    }


def _healthy_physical_dataspace_status() -> dict[str, object]:
    lane_specs = [
        (0, "core", 0, "universal"),
        (1, "governance", 0, "universal"),
        (2, "zk", 0, "universal"),
        (3, "dpn", 10, "dpn"),
        (4, "external-poc", 6647857470246403404, "is"),
        (5, "boi-mobile", 8477022798449861195, "is2"),
        (6, "cbsi", 20, "cbsi"),
    ]
    rosters = {
        alias: [f"{alias}-validator-{index}" for index in range(1, 5)]
        for alias in ("universal", "dpn", "is", "is2", "cbsi")
    }
    teu_lane_commit = []
    dataspace_catalog = []
    for lane_id, lane_alias, dataspace_id, dataspace_alias in lane_specs:
        manifest_required = lane_alias == "governance"
        has_manifest = lane_alias == "governance" or dataspace_alias != "universal"
        manifest_path = (
            f"/manifests/{lane_alias}.manifest.json" if has_manifest else None
        )
        teu_lane_commit.append(
            {
                "lane_id": lane_id,
                "alias": lane_alias,
                "dataspace_id": dataspace_id,
                "dataspace_alias": dataspace_alias,
                "manifest_required": manifest_required,
                "manifest_ready": has_manifest,
                "manifest_path": manifest_path,
                "manifest_validators": (
                    rosters[dataspace_alias].copy() if has_manifest else []
                ),
                "manifest_validator_bindings": (
                    _manifest_bindings(dataspace_alias, rosters[dataspace_alias])
                    if has_manifest
                    else []
                ),
                "manifest_quorum": 3 if has_manifest else None,
            }
        )
        dataspace_catalog.append(
            {
                "lane_id": lane_id,
                "lane_alias": lane_alias,
                "dataspace_id": dataspace_id,
                "alias": dataspace_alias,
                "manifest_required": manifest_required,
                "manifest_ready": has_manifest,
                "manifest_path": manifest_path,
            }
        )
    return {
        "nexus": {"routing_policy": _healthy_routing_policy()},
        "teu_lane_commit": teu_lane_commit,
        "dataspace_catalog": dataspace_catalog,
    }


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
    expected_dpn_commit: str = "",
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
            expected_dpn_commit,
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


def test_lane_dataspace_topology_accepts_seven_lanes_on_five_dataspaces(
    tmp_path: Path,
) -> None:
    result = _run_lane_dataspace_topology_checker(
        tmp_path, _healthy_lane_dataspace_topology()
    )

    assert result.returncode == 0, result.stderr
    summary = json.loads(result.stdout)
    assert summary["lane_count"] == 7
    assert summary["dataspaces"] == {
        "universal": 0,
        "dpn": 10,
        "is": 6647857470246403404,
        "is2": 8477022798449861195,
        "cbsi": 20,
    }


def test_canonical_config_topology_accepts_exact_routing_matcher_inventory() -> None:
    config = ROOT / "configs" / "soranexus" / "taira" / "config.toml"

    result = _run_canonical_config_topology_checker(config)

    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout) == _expected_routing_policy_summary()


def test_canonical_config_topology_rejects_missing_extra_or_wrong_matcher(
    tmp_path: Path,
) -> None:
    canonical = (
        ROOT / "configs" / "soranexus" / "taira" / "config.toml"
    ).read_text(encoding="utf-8")
    external_poc_rule = """
[[nexus.routing_policy.rules]]
lane = 4
dataspace = "is"
[nexus.routing_policy.rules.matcher]
account = "*@wonderland.is"
description = "Route the PoC authority to the private is dataspace"
"""
    assert external_poc_rule in canonical
    cases = {
        "missing": canonical.replace(external_poc_rule, "", 1),
        "extra": canonical.replace(
            "\n[nexus.fusion]\n",
            external_poc_rule + "\n[nexus.fusion]\n",
            1,
        ),
        # `wonderland` is a namespace bound to `is`; replacing the complete
        # namespace-qualified matcher with the dataspace root conflates two
        # independent topology layers and must fail closed.
        "wrong-namespace-scope": canonical.replace(
            'account = "*@wonderland.is"',
            'account = "*@is"',
            1,
        ),
    }

    for name, mutated in cases.items():
        config = tmp_path / f"{name}.toml"
        config.write_text(mutated, encoding="utf-8")

        result = _run_canonical_config_topology_checker(config)

        assert result.returncode == 1, name
        assert "expected exact routing matcher tuples" in result.stderr, (
            name,
            result.stderr,
        )


def test_canonical_topology_environment_overrides_are_ignored() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    assignments, separator, _rest = source.partition("\nusage() {")
    assert separator
    variable_names = [
        "EXPECTED_TAIRA_LANE_COUNT",
        "EXPECTED_UNIVERSAL_DATASPACE_ID",
        "EXPECTED_DPN_DATASPACE_ID",
        "EXPECTED_IS_DATASPACE_ID",
        "EXPECTED_IS2_DATASPACE_ID",
        "EXPECTED_CBSI_DATASPACE_ID",
        "EXPECTED_CORE_ROUTE_ALIAS",
        "EXPECTED_GOVERNANCE_ROUTE_ALIAS",
        "EXPECTED_ZK_ROUTE_ALIAS",
        "EXPECTED_DPN_ROUTE_ALIAS",
        "EXPECTED_IS_ROUTE_ALIAS",
        "EXPECTED_IS2_ROUTE_ALIAS",
        "EXPECTED_CBSI_ROUTE_ALIAS",
    ]
    expected = [
        "7",
        "0",
        "10",
        "6647857470246403404",
        "8477022798449861195",
        "20",
        "core",
        "governance",
        "zk",
        "dpn",
        "external-poc",
        "boi-mobile",
        "cbsi",
    ]
    probe = assignments + "\nprintf '%s\\n' " + " ".join(
        f'"${{{name}}}"' for name in variable_names
    )
    environment = os.environ.copy()
    environment.update({name: "hostile-override" for name in variable_names})

    result = subprocess.run(
        ["bash", "-c", probe],
        cwd=ROOT,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 0, result.stderr
    assert result.stdout.splitlines() == expected


def test_lane_dataspace_topology_rejects_lane_count_regression(
    tmp_path: Path,
) -> None:
    payload = _healthy_lane_dataspace_topology()
    payload["lane_count"] = 5

    result = _run_lane_dataspace_topology_checker(tmp_path, payload)

    assert result.returncode == 1
    assert "lane_count must be exactly 7" in result.stderr


def test_lane_dataspace_topology_rejects_governance_or_zk_dataspaces(
    tmp_path: Path,
) -> None:
    for lane_position, forbidden_dataspace_id in ((1, 1), (2, 2)):
        payload = _healthy_lane_dataspace_topology()
        lanes = payload["lanes"]
        assert isinstance(lanes, list)
        lane = lanes[lane_position]
        assert isinstance(lane, dict)
        lane["dataspace_id"] = forbidden_dataspace_id

        result = _run_lane_dataspace_topology_checker(tmp_path, payload)

        assert result.returncode == 1
        assert "Taira lane/dataspace topology mismatch" in result.stderr


def test_physical_dataspace_rosters_accept_distinct_manifest_cohorts(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    result = _run_physical_dataspace_roster_checker(
        tmp_path,
        status,
        _healthy_base_payload(),
    )

    assert result.returncode == 0, result.stderr
    summary = json.loads(result.stdout)
    assert set(summary) == {"universal", "dpn", "is", "is2", "cbsi"}
    assert summary["universal"]["source"] == "lane_manifest"
    assert summary["universal"]["inherited_lanes"] == ["core", "zk"]
    assert len(summary["dpn"]["members"]) == 4
    assert len(summary["dpn"]["bindings"]) == 4
    assert all(
        set(binding) == {"validator", "peer_id", "torii_url"}
        for binding in summary["dpn"]["bindings"]
    )
    assert summary["dpn"]["quorum"] == 3
    lanes = status["teu_lane_commit"]
    assert isinstance(lanes, list)
    for lane in lanes[3:]:
        assert isinstance(lane, dict)
        assert lane["manifest_required"] is False


def test_physical_dataspace_rosters_accept_identical_same_dataspace_bindings(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    lanes = status["teu_lane_commit"]
    catalog = status["dataspace_catalog"]
    assert isinstance(lanes, list) and isinstance(catalog, list)
    governance = lanes[1]
    zk = lanes[2]
    zk_catalog = catalog[2]
    assert isinstance(governance, dict)
    assert isinstance(zk, dict) and isinstance(zk_catalog, dict)
    zk.update(
        {
            "manifest_ready": True,
            "manifest_path": "/manifests/zk.manifest.json",
            "manifest_validators": list(governance["manifest_validators"]),
            "manifest_validator_bindings": list(
                governance["manifest_validator_bindings"]
            ),
            "manifest_quorum": governance["manifest_quorum"],
        }
    )
    zk_catalog.update(
        {
            "manifest_ready": True,
            "manifest_path": "/manifests/zk.manifest.json",
        }
    )

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 0, result.stderr
    summary = json.loads(result.stdout)
    assert summary["universal"]["inherited_lanes"] == ["core"]


def test_physical_dataspace_rosters_require_schema_closed_binding_rows(
    tmp_path: Path,
) -> None:
    for mutation, expected in (
        ("missing_field", "fields must be exactly"),
        ("unknown_field", "fields must be exactly"),
        ("missing_projection", "manifest_validator_bindings is not an array"),
    ):
        status = _healthy_physical_dataspace_status()
        lanes = status["teu_lane_commit"]
        assert isinstance(lanes, list)
        dpn = lanes[3]
        assert isinstance(dpn, dict)
        if mutation == "missing_projection":
            dpn.pop("manifest_validator_bindings")
        else:
            bindings = dpn["manifest_validator_bindings"]
            assert isinstance(bindings, list) and isinstance(bindings[0], dict)
            if mutation == "missing_field":
                bindings[0].pop("torii_url")
            else:
                bindings[0]["unexpected"] = True

        result = _run_physical_dataspace_roster_checker(
            tmp_path, status, _healthy_base_payload()
        )

        assert result.returncode == 1
        assert expected in result.stderr


def test_physical_dataspace_rosters_require_exact_binding_account_set(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    lanes = status["teu_lane_commit"]
    assert isinstance(lanes, list)
    dpn = lanes[3]
    assert isinstance(dpn, dict)
    bindings = dpn["manifest_validator_bindings"]
    assert isinstance(bindings, list) and isinstance(bindings[0], dict)
    bindings[0]["validator"] = "unlisted-validator"

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 1
    assert "roster does not exactly match its validator-binding account set" in result.stderr


def test_physical_dataspace_rosters_reject_duplicate_binding_identity(
    tmp_path: Path,
) -> None:
    for field, expected in (
        ("peer_id", "contains duplicate PeerIds"),
        ("torii_url", "contains duplicate Torii origins"),
    ):
        status = _healthy_physical_dataspace_status()
        lanes = status["teu_lane_commit"]
        assert isinstance(lanes, list)
        dpn = lanes[3]
        assert isinstance(dpn, dict)
        bindings = dpn["manifest_validator_bindings"]
        assert isinstance(bindings, list)
        first, second = bindings[:2]
        assert isinstance(first, dict) and isinstance(second, dict)
        second[field] = first[field]

        result = _run_physical_dataspace_roster_checker(
            tmp_path, status, _healthy_base_payload()
        )

        assert result.returncode == 1
        assert expected in result.stderr


def test_physical_dataspace_rosters_reject_noncanonical_torii_origin(
    tmp_path: Path,
) -> None:
    for torii_url in (
        "https://DPN-validator-1.test",
        "https://dpn-validator-1.test/",
        "https://dpn-validator-1.test:443",
        "https://dpn-validator-1.test/path",
        "http://dpn-validator-1.test",
    ):
        status = _healthy_physical_dataspace_status()
        lanes = status["teu_lane_commit"]
        assert isinstance(lanes, list)
        dpn = lanes[3]
        assert isinstance(dpn, dict)
        bindings = dpn["manifest_validator_bindings"]
        assert isinstance(bindings, list) and isinstance(bindings[0], dict)
        bindings[0]["torii_url"] = torii_url

        result = _run_physical_dataspace_roster_checker(
            tmp_path, status, _healthy_base_payload()
        )

        assert result.returncode == 1
        assert "torii_url" in result.stderr


def test_effective_routing_policy_accepts_canonical_live_status(tmp_path: Path) -> None:
    result = _run_effective_routing_policy_checker(
        tmp_path,
        _healthy_physical_dataspace_status(),
    )

    assert result.returncode == 0, result.stderr


def test_effective_routing_policy_rejects_missing_or_drifted_live_status(
    tmp_path: Path,
) -> None:
    def routing_policy(status: dict[str, object]) -> dict[str, object]:
        nexus = status["nexus"]
        assert isinstance(nexus, dict)
        policy = nexus["routing_policy"]
        assert isinstance(policy, dict)
        return policy

    def routing_rules(status: dict[str, object]) -> list[object]:
        rules = routing_policy(status)["rules"]
        assert isinstance(rules, list)
        return rules

    missing_nexus = _healthy_physical_dataspace_status()
    missing_nexus.pop("nexus")

    missing_policy = _healthy_physical_dataspace_status()
    nexus = missing_policy["nexus"]
    assert isinstance(nexus, dict)
    nexus.pop("routing_policy")

    wrong_default = _healthy_physical_dataspace_status()
    routing_policy(wrong_default)["default_dataspace"] = 10

    wrong_dataspace = _healthy_physical_dataspace_status()
    first_rule = routing_rules(wrong_dataspace)[0]
    assert isinstance(first_rule, dict)
    first_rule["dataspace_id"] = 11

    wrong_matcher = _healthy_physical_dataspace_status()
    external_poc_rule = routing_rules(wrong_matcher)[1]
    assert isinstance(external_poc_rule, dict)
    matcher = external_poc_rule["matcher"]
    assert isinstance(matcher, dict)
    matcher["account"] = "*@is"

    reordered = _healthy_physical_dataspace_status()
    rules = routing_rules(reordered)
    rules[0], rules[1] = rules[1], rules[0]

    ambiguous_matcher = _healthy_physical_dataspace_status()
    first_rule = routing_rules(ambiguous_matcher)[0]
    assert isinstance(first_rule, dict)
    matcher = first_rule["matcher"]
    assert isinstance(matcher, dict)
    matcher["instruction"] = "governance"

    null_description = _healthy_physical_dataspace_status()
    first_rule = routing_rules(null_description)[0]
    assert isinstance(first_rule, dict)
    matcher = first_rule["matcher"]
    assert isinstance(matcher, dict)
    matcher["description"] = None

    cases = (
        (missing_nexus, "/status.nexus is not an object"),
        (missing_policy, "/status.nexus.routing_policy is not an object"),
        (wrong_default, "default_dataspace must be 0, observed 10"),
        (wrong_dataspace, "expected exact ordered rule tuples"),
        (wrong_matcher, "expected exact ordered rule tuples"),
        (reordered, "expected exact ordered rule tuples"),
        (ambiguous_matcher, "must contain exactly one account or instruction selector"),
        (null_description, "matcher.description is empty"),
    )
    for status, expected_error in cases:
        result = _run_effective_routing_policy_checker(
            tmp_path,
            status,
        )

        assert result.returncode == 1
        assert "Taira effective routing policy mismatch" in result.stderr
        assert expected_error in result.stderr


def test_physical_dataspace_rosters_require_ready_private_manifest_evidence(
    tmp_path: Path,
) -> None:
    cases = (
        ("manifest_ready", False, "publishes a roster without a ready manifest"),
        ("manifest_path", None, "publishes a roster without a manifest_path"),
    )
    for field, value, expected in cases:
        status = _healthy_physical_dataspace_status()
        lanes = status["teu_lane_commit"]
        catalog = status["dataspace_catalog"]
        assert isinstance(lanes, list) and isinstance(catalog, list)
        dpn = lanes[3]
        dpn_catalog = catalog[3]
        assert isinstance(dpn, dict) and isinstance(dpn_catalog, dict)
        assert dpn["manifest_required"] is False
        dpn[field] = value
        dpn_catalog[field] = value

        result = _run_physical_dataspace_roster_checker(
            tmp_path, status, _healthy_base_payload()
        )

        assert result.returncode == 1
        assert expected in result.stderr


def test_physical_dataspace_rosters_reject_required_manifest_without_roster(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    lanes = status["teu_lane_commit"]
    catalog = status["dataspace_catalog"]
    assert isinstance(lanes, list) and isinstance(catalog, list)
    governance = lanes[1]
    governance_catalog = catalog[1]
    assert isinstance(governance, dict) and isinstance(governance_catalog, dict)
    governance.update(
        {
            "manifest_ready": False,
            "manifest_path": None,
            "manifest_validators": [],
            "manifest_validator_bindings": [],
            "manifest_quorum": None,
        }
    )
    governance_catalog.update({"manifest_ready": False, "manifest_path": None})

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 1
    assert (
        "lane 'governance' requires a ready manifest with a non-empty validator roster"
        in result.stderr
    )


def test_physical_dataspace_rosters_reject_universal_without_manifest_membership(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    lanes = status["teu_lane_commit"]
    catalog = status["dataspace_catalog"]
    assert isinstance(lanes, list) and isinstance(catalog, list)
    governance = lanes[1]
    governance_catalog = catalog[1]
    assert isinstance(governance, dict) and isinstance(governance_catalog, dict)
    governance.update(
        {
            "manifest_required": False,
            "manifest_ready": False,
            "manifest_path": None,
            "manifest_validators": [],
            "manifest_validator_bindings": [],
            "manifest_quorum": None,
        }
    )
    governance_catalog.update(
        {
            "manifest_required": False,
            "manifest_ready": False,
            "manifest_path": None,
        }
    )

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 1
    assert (
        "physical dataspace 'universal' lacks a non-empty ready manifest validator roster"
        in result.stderr
    )


def test_physical_dataspace_rosters_fail_closed_without_private_manifest(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    lanes = status["teu_lane_commit"]
    assert isinstance(lanes, list)
    cbsi = lanes[6]
    assert isinstance(cbsi, dict)
    cbsi["manifest_validators"] = []
    cbsi["manifest_validator_bindings"] = []
    cbsi["manifest_quorum"] = None

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 1
    assert "physical dataspace 'cbsi' lacks a non-empty manifest validator roster" in result.stderr


def test_physical_dataspace_rosters_reject_invalid_quorum(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    lanes = status["teu_lane_commit"]
    assert isinstance(lanes, list)
    dpn = lanes[3]
    assert isinstance(dpn, dict)
    dpn["manifest_quorum"] = 2

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 1
    assert "manifest quorum 2 is invalid for 4 validators" in result.stderr


def test_physical_dataspace_rosters_reject_reused_cross_dataspace_cohort(
    tmp_path: Path,
) -> None:
    for target_lane in (3, 5):
        status = _healthy_physical_dataspace_status()
        lanes = status["teu_lane_commit"]
        assert isinstance(lanes, list)
        universal = lanes[1]
        target = lanes[target_lane]
        assert isinstance(universal, dict) and isinstance(target, dict)
        if target_lane == 3:
            target["manifest_validators"] = list(universal["manifest_validators"])
            target["manifest_validator_bindings"] = list(
                universal["manifest_validator_bindings"]
            )
            expected = "physical dataspaces 'universal' and 'dpn' reuse the same validator roster"
        else:
            source = lanes[4]
            assert isinstance(source, dict)
            target["manifest_validators"] = list(source["manifest_validators"])
            target["manifest_validator_bindings"] = list(
                source["manifest_validator_bindings"]
            )
            expected = "physical dataspaces 'is' and 'is2' reuse the same validator roster"

        result = _run_physical_dataspace_roster_checker(
            tmp_path, status, _healthy_base_payload()
        )

        assert result.returncode == 1
        assert expected in result.stderr


def test_physical_dataspace_rosters_reject_partial_cross_dataspace_binding_reuse(
    tmp_path: Path,
) -> None:
    for field, expected in (
        ("validator", "same manifest validator account"),
        ("peer_id", "same manifest PeerId"),
        ("torii_url", "same manifest Torii origin"),
    ):
        status = _healthy_physical_dataspace_status()
        lanes = status["teu_lane_commit"]
        assert isinstance(lanes, list)
        universal = lanes[1]
        dpn = lanes[3]
        assert isinstance(universal, dict) and isinstance(dpn, dict)
        universal_bindings = universal["manifest_validator_bindings"]
        dpn_bindings = dpn["manifest_validator_bindings"]
        assert isinstance(universal_bindings, list) and isinstance(dpn_bindings, list)
        source = universal_bindings[0]
        target = dpn_bindings[0]
        assert isinstance(source, dict) and isinstance(target, dict)
        target[field] = source[field]
        if field == "validator":
            members = dpn["manifest_validators"]
            assert isinstance(members, list)
            members[0] = source[field]

        result = _run_physical_dataspace_roster_checker(
            tmp_path, status, _healthy_base_payload()
        )

        assert result.returncode == 1
        assert expected in result.stderr


def test_physical_dataspace_rosters_reject_same_dataspace_projection_drift(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    lanes = status["teu_lane_commit"]
    catalog = status["dataspace_catalog"]
    assert isinstance(lanes, list) and isinstance(catalog, list)
    zk = lanes[2]
    zk_catalog = catalog[2]
    assert isinstance(zk, dict) and isinstance(zk_catalog, dict)
    zk.update(
        {
            "manifest_required": True,
            "manifest_ready": True,
            "manifest_path": "/manifests/zk.manifest.json",
            "manifest_validators": [f"zk-validator-{index}" for index in range(1, 5)],
            "manifest_validator_bindings": _manifest_bindings(
                "zk", [f"zk-validator-{index}" for index in range(1, 5)]
            ),
            "manifest_quorum": 3,
        }
    )
    zk_catalog.update(
        {
            "manifest_required": True,
            "manifest_ready": True,
            "manifest_path": "/manifests/zk.manifest.json",
        }
    )

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 1
    assert "lanes in physical dataspace 'universal' project different validator rosters" in result.stderr


def test_physical_dataspace_rosters_cross_check_status_catalog_projection(
    tmp_path: Path,
) -> None:
    status = _healthy_physical_dataspace_status()
    catalog = status["dataspace_catalog"]
    assert isinstance(catalog, list)
    dpn = catalog[3]
    assert isinstance(dpn, dict)
    dpn["alias"] = "governance"

    result = _run_physical_dataspace_roster_checker(
        tmp_path, status, _healthy_base_payload()
    )

    assert result.returncode == 1
    assert "/status.dataspace_catalog lane 3 field alias" in result.stderr


def test_physical_dataspace_rosters_cross_check_universal_frozen_quorum(
    tmp_path: Path,
) -> None:
    for validator_count, min_signers in ((5, 4), (4, 4)):
        sumeragi = _healthy_base_payload()
        context = sumeragi["height_context"]
        assert isinstance(context, dict)
        context["validator_count"] = validator_count
        context["quorum"] = {
            "min_signers": min_signers,
            "total_power": validator_count,
        }

        result = _run_physical_dataspace_roster_checker(
            tmp_path, _healthy_physical_dataspace_status(), sumeragi
        )

        assert result.returncode == 1
        assert "universal manifest roster does not match the frozen global" in result.stderr


def test_sumeragi_checker_accepts_authoritative_v2(tmp_path: Path) -> None:
    result = _run_checker(tmp_path, _healthy_base_payload())
    assert result.returncode == 0, result.stderr
    assert '"commit_qc_signers": 3' in result.stdout


def test_public_sumeragi_checker_rejects_non_four_validator_roster(
    tmp_path: Path,
) -> None:
    context = _healthy_base_payload()
    context["height_context"]["validator_count"] = 7  # type: ignore[index]
    context["height_context"]["quorum"] = {  # type: ignore[index]
        "min_signers": 5,
        "total_power": 7,
    }
    result = _run_checker(tmp_path, context)
    assert result.returncode == 1
    assert "Taira requires exactly 4" in result.stderr

    commit = _healthy_base_payload()
    commit["last_commit_qc"].update(  # type: ignore[union-attr]
        {
            "validator_count": 7,
            "signer_count": 5,
            "min_signers": 5,
            "signed_power": 5,
            "total_power": 7,
        }
    )
    result = _run_checker(tmp_path, commit)
    assert result.returncode == 1
    assert "durable CommitQC does not satisfy its frozen dual quorum" in result.stderr


def test_sorafs_public_smoke_pins_exact_four_validator_status() -> None:
    source = (
        ROOT / "configs/soranexus/taira/check_sorafs_rollout.sh"
    ).read_text(encoding="utf-8")

    assert "if validator_count != 4:" in source
    assert "Taira requires exactly 4" in source
    assert "commit_validators != 4" in source


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
        "universal": 0,
        "dpn": 10,
        "is": 6647857470246403404,
        "is2": 8477022798449861195,
        "cbsi": 20,
    }
    assert catalog["lane_count"] == 7
    assert [lane["alias"] for lane in catalog["lanes"]] == [
        "core",
        "governance",
        "zk",
        "dpn",
        "external-poc",
        "boi-mobile",
        "cbsi",
    ]
    assert [lane["dataspace_id"] for lane in catalog["lanes"][:3]] == [0, 0, 0]
    rosters = json.loads(summary["dataspace_rosters"])
    assert len({tuple(roster["members"]) for roster in rosters.values()}) == 5


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
    catalog["dataspaces"]["is2"] = 9
    wrong_catalog["dataspace_catalog"] = json.dumps(
        catalog,
        sort_keys=True,
        separators=(",", ":"),
    )
    result = _run_fleet_checker(tmp_path, [baseline, wrong_catalog])
    assert result.returncode == 1
    assert "dataspace_catalog" in result.stderr

    wrong_roster = _fleet_record("v2", "node-2")
    rosters = json.loads(str(wrong_roster["dataspace_rosters"]))
    rosters["cbsi"]["members"][0] = "other-cbsi-validator"
    wrong_roster["dataspace_rosters"] = json.dumps(
        rosters,
        sort_keys=True,
        separators=(",", ":"),
    )
    result = _run_fleet_checker(tmp_path, [baseline, wrong_roster])
    assert result.returncode == 1
    assert "dataspace_rosters" in result.stderr


def test_validator_progress_gate_requires_stable_catalog_and_advancing_commit() -> None:
    previous = {
        "build": "build",
        "config": "config",
        "nodes": ["node-1", "node-2"],
        "dataspace_catalog": "seven-lanes-five-dataspaces",
        "dataspace_rosters": "five-distinct-validator-cohorts",
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

    changed_catalog = {**current, "dataspace_catalog": "seven-lanes-six-dataspaces"}
    rejected = _run_progress_checker(previous, changed_catalog)
    assert rejected.returncode == 1
    assert "changed dataspace_catalog between progress samples" in rejected.stderr

    changed_rosters = {**current, "dataspace_rosters": "reused-validator-cohort"}
    rejected = _run_progress_checker(previous, changed_rosters)
    assert rejected.returncode == 1
    assert "changed dataspace_rosters between progress samples" in rejected.stderr

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
    payload["protocol_version"] = 3

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
        {
            "build": {
                "dpn_validator_release_commit": DPN_COMMIT,
                "git_commit_sha": expected,
            },
            "blocks": 42,
            "queue_size": 0,
        },
        expected_git_sha=expected,
        expected_dpn_commit=DPN_COMMIT,
        require_exact=True,
    )
    assert exact.returncode == 0, exact.stderr

    shortened = _run_status_checker(
        tmp_path,
        {
            "build": {
                "dpn_validator_release_commit": DPN_COMMIT,
                "git_commit_sha": expected[:12],
            },
            "blocks": 42,
            "queue_size": 0,
        },
        expected_git_sha=expected,
        expected_dpn_commit=DPN_COMMIT,
        require_exact=True,
    )
    assert shortened.returncode == 1
    assert "does not exactly match release commit" in shortened.stderr


def test_release_status_checker_rejects_dpn_only_mismatch(tmp_path: Path) -> None:
    expected_git = "490dacc287f00d490dacc287f00d490dacc287f0"
    mismatch = _run_status_checker(
        tmp_path,
        {
            "build": {
                "dpn_validator_release_commit": "e" * 40,
                "git_commit_sha": expected_git,
            },
            "blocks": 42,
            "queue_size": 0,
        },
        expected_git_sha=expected_git,
        expected_dpn_commit=DPN_COMMIT,
        require_exact=True,
    )

    assert mismatch.returncode == 1
    assert "DPN validator release commit" in mismatch.stderr
    assert "does not exactly match" in mismatch.stderr


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
