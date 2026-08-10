"""Hostile tests for the closed Taira Exact12 four-wave rollout contract."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pytest

from scripts import taira_privacy_rollout_contract as contract


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/taira_privacy_rollout_contract.py"
PLAN = ROOT / "configs/soranexus/taira/privacy_rollout_plan_v1.json"
BOOTSTRAP_PLAN = ROOT / "configs/soranexus/taira/privacy_bootstrap_plan.json"
PUBLISH_WORKFLOW = ROOT / ".github/workflows/publish_taira_validator.yml"
SEALED_CONTROLLER = ROOT / "scripts/seal_taira_release_controllers.py"


def _digest(label: str) -> str:
    return hashlib.sha256(label.encode("ascii")).hexdigest()


def _oci(label: str) -> str:
    return f"sha256:{_digest(label)}"


def _candidate() -> dict[str, object]:
    value: dict[str, object] = {
        "archive_sha256": _digest("archive"),
        "candidate_oci_digest": _oci("candidate"),
        "capability_schema_sha256": _digest("capability-schema"),
        "cargo_lock_sha256": contract.FROZEN_CARGO_LOCK_SHA256,
        "dpn_validator_release_commit": "b" * 40,
        "irohad_sha256": _digest("iroha3d"),
        "protocol_matrix_sha256": contract.EXACT12_MATRIX_SHA256,
        "source_commit": "a" * 40,
        "workspace_source_manifest_sha256": _digest("source-manifest"),
    }
    binding = hashlib.sha256(
        contract.CANDIDATE_ID_DOMAIN + contract.canonical_json_bytes(value)[:-1]
    ).hexdigest()
    value["candidate_binding_sha256"] = binding
    assert contract.candidate_binding(value) == binding
    return value


def _active(wave_count: int) -> list[str]:
    selected = {
        protocol
        for wave in contract.WAVES[:wave_count]
        for protocol in wave
    }
    return [protocol for protocol in contract.PROTOCOLS if protocol in selected]


def _snapshots(
    height: int,
    active: list[str],
    candidate_binding: str,
    tag: str,
) -> list[dict[str, object]]:
    block_hash = _digest(f"{tag}:block")
    manifest = _digest(f"{tag}:manifest")
    return [
        {
            "active_protocols": list(active),
            "available_protocols": list(contract.PROTOCOLS),
            "block_hash": block_hash,
            "candidate_binding_sha256": candidate_binding,
            "capability_manifest_sha256": manifest,
            "endpoint": endpoint,
            "height": height,
            "jindo_assurance": contract.JINDO_ASSURANCE,
            "jindo_missing_evidence": list(contract.JINDO_MISSING_EVIDENCE),
            "unavailable_protocols": [],
        }
        for endpoint in contract.ENDPOINTS
    ]


def _queries(height: int, tag: str) -> list[dict[str, object]]:
    state = _digest(f"{tag}:state")
    return [
        {"endpoint": endpoint, "height": height, "state_sha256": state}
        for endpoint in contract.ENDPOINTS
    ]


def _rejections(
    height: int, failure_code: str
) -> list[dict[str, object]]:
    return [
        {
            "endpoint": endpoint,
            "failure_code": failure_code,
            "observed_height": height,
        }
        for endpoint in contract.ENDPOINTS
    ]


def _canaries(
    protocols: tuple[str, ...],
    activation: int,
    candidate_binding: str,
    wave_index: int,
) -> tuple[list[dict[str, object]], int]:
    rows = []
    maximum_height = activation
    for offset, protocol in enumerate(protocols, 1):
        accepted = activation + offset
        maximum_height = max(maximum_height, accepted)
        transaction = _digest(f"wave-{wave_index}:{protocol}:positive")
        rows.append(
            {
                "candidate_binding_sha256": candidate_binding,
                "negative": {
                    "rejections": _rejections(
                        accepted, "privacy-malformed-proof"
                    ),
                    "transaction_sha256": _digest(
                        f"wave-{wave_index}:{protocol}:negative"
                    ),
                },
                "positive": {
                    "accepted_height": accepted,
                    "peer_queries": _queries(
                        accepted, f"wave-{wave_index}:{protocol}:query"
                    ),
                    "statement_sha256": _digest(
                        f"wave-{wave_index}:{protocol}:statement"
                    ),
                    "submitted_via": "public-torii",
                    "transaction_sha256": transaction,
                },
                "protocol": protocol,
                "replay": {
                    "rejections": _rejections(accepted, "privacy-replay"),
                    "transaction_sha256": transaction,
                },
            }
        )
    return rows, maximum_height


def _resources(protocols: tuple[str, ...]) -> list[dict[str, object]]:
    rows = []
    for protocol in protocols:
        evaluated = (
            contract.RESOURCE_CEILINGS["zk_ams_evaluated_key_bytes"]
            if protocol == "iroha-zk-ams-v1"
            else 0
        )
        rows.append(
            {
                "address_space_bytes": 512 * 1024 * 1024,
                "elapsed_millis": 10_000,
                "evaluated_key_publication_bytes": evaluated,
                "evaluated_key_retrieval_bytes": evaluated,
                "peak_rss_bytes": 256 * 1024 * 1024,
                "protocol": protocol,
                "transport_bytes": 1024 * 1024,
                "work_units": 1_000_000,
            }
        )
    return rows


def _restart(
    wave_index: int,
    stopped: int,
    sentinel: int,
) -> dict[str, object]:
    sentinel_hash = _digest(f"wave-{wave_index}:sentinel")
    successor = sentinel + 1
    successor_hash = _digest(f"wave-{wave_index}:successor")
    return {
        "peer_finality": [
            {
                "block_hash": successor_hash,
                "endpoint": endpoint,
                "height": successor,
            }
            for endpoint in contract.ENDPOINTS
        ],
        "recovered_hash": sentinel_hash,
        "recovered_height": sentinel,
        "sentinel_hash": sentinel_hash,
        "sentinel_height": sentinel,
        "stopped_height": stopped,
        "successor_hash": successor_hash,
        "successor_height": successor,
        "validator": contract.VALIDATORS[wave_index - 1],
    }


def _valid_result() -> dict[str, object]:
    plan = contract.expected_plan()
    plan_sha256 = contract.validate_plan(plan)
    candidate = _candidate()
    binding = candidate["candidate_binding_sha256"]
    assert isinstance(binding, str)
    baseline_height = 10
    waves = []
    previous_observation = baseline_height
    final_observation_manifest = ""
    for wave_index, protocols in enumerate(contract.WAVES, 1):
        proposed = previous_observation + 1
        activation = proposed + contract.NOTICE_INTERVAL_BLOCKS
        observation = activation + contract.OBSERVATION_INTERVAL_BLOCKS
        canaries, maximum_canary_height = _canaries(
            protocols, activation, binding, wave_index
        )
        sentinel = maximum_canary_height + 1
        successor = sentinel + 1
        active = _active(wave_index)
        observation_snapshots = _snapshots(
            observation, active, binding, f"wave-{wave_index}:observation"
        )
        final_observation_manifest = observation_snapshots[0][
            "capability_manifest_sha256"
        ]
        waves.append(
            {
                "activate_at_height": activation,
                "activation_transactions": [
                    {
                        "governance_outcome": "committed",
                        "protocol": protocol,
                        "transaction_sha256": _digest(
                            f"wave-{wave_index}:{protocol}:activation"
                        ),
                    }
                    for protocol in protocols
                ],
                "canaries": canaries,
                "candidate_binding_sha256": binding,
                "index": wave_index,
                "label": f"wave-{wave_index}",
                "observation_completed_at_height": observation,
                "observation_snapshots": observation_snapshots,
                "post_activation_snapshots": _snapshots(
                    activation,
                    active,
                    binding,
                    f"wave-{wave_index}:activation",
                ),
                "post_restart_snapshots": _snapshots(
                    successor,
                    active,
                    binding,
                    f"wave-{wave_index}:restart",
                ),
                "pre_activation_snapshots": _snapshots(
                    proposed - 1,
                    _active(wave_index - 1),
                    binding,
                    f"wave-{wave_index}:pre",
                ),
                "proposed_at_height": proposed,
                "protocols": list(protocols),
                "resources": _resources(protocols),
                "restart": _restart(
                    wave_index, maximum_canary_height, sentinel
                ),
            }
        )
        previous_observation = observation

    write_height = previous_observation + 1
    privacy_height = write_height + 1
    post_snapshots = _snapshots(
        privacy_height, _active(4), binding, "post-cutover"
    )
    privacy_transaction = _digest("post-cutover:privacy")
    result: dict[str, object] = {
        "baseline": {
            "snapshots": _snapshots(
                baseline_height, [], binding, "baseline"
            )
        },
        "candidate": candidate,
        "completed_at_unix": 2_000_000_000,
        "plan_sha256": plan_sha256,
        "post_cutover": {
            "canary": {
                "authority_scope": "dedicated-no-governance-canary",
                "bootstrap_authority": False,
                "governance_permission_present": False,
                "mode": "signed-write-and-privacy",
                "privacy": {
                    "accepted_height": privacy_height,
                    "peer_queries": _queries(
                        privacy_height, "post-cutover:privacy-query"
                    ),
                    "protocol": "verange-transparent-range-v1",
                    "statement_sha256": _digest(
                        "post-cutover:privacy-statement"
                    ),
                    "submitted_via": "public-torii",
                    "transaction_sha256": privacy_transaction,
                },
                "replay": {
                    "rejections": _rejections(
                        privacy_height, "privacy-replay"
                    ),
                    "transaction_sha256": privacy_transaction,
                },
                "skipped": False,
                "write": {
                    "accepted_height": write_height,
                    "peer_queries": _queries(
                        write_height, "post-cutover:write-query"
                    ),
                    "submitted_via": "public-torii",
                    "transaction_sha256": _digest("post-cutover:write"),
                },
            },
            "deployed_candidate_oci_digest": candidate[
                "candidate_oci_digest"
            ],
            "readmitted_candidate_oci_digest": candidate[
                "candidate_oci_digest"
            ],
            "snapshots": post_snapshots,
        },
        "rollback": {
            "armed": True,
            "invoked": False,
            "legacy_fallback_used": False,
            "previous_candidate_oci_digest": _oci("previous-candidate"),
            "previous_capability_manifest_sha256": _digest(
                "previous-capability"
            ),
            "restore_mode": "immutable-candidate-and-capability-set",
        },
        "schema": contract.RESULT_SCHEMA,
        "schema_version": contract.SCHEMA_VERSION,
        "started_at_unix": 1_999_000_000,
        "terminal": {
            "final_candidate_oci_digest": candidate[
                "candidate_oci_digest"
            ],
            "final_capability_manifest_sha256": post_snapshots[0][
                "capability_manifest_sha256"
            ],
            "halt_reason": None,
            "halted": False,
            "publication_authorized": True,
            "status": "passed",
        },
        "waves": waves,
    }
    assert final_observation_manifest
    result["rollout_id"] = hashlib.sha256(
        contract.ROLLOUT_ID_DOMAIN
        + contract.canonical_json_bytes(result)[:-1]
    ).hexdigest()
    return result


def _redigest(result: dict[str, object]) -> None:
    body = {key: value for key, value in result.items() if key != "rollout_id"}
    result["rollout_id"] = hashlib.sha256(
        contract.ROLLOUT_ID_DOMAIN
        + contract.canonical_json_bytes(body)[:-1]
    ).hexdigest()


def test_checked_in_plan_is_the_exact_canonical_contract() -> None:
    payload = PLAN.read_bytes()
    plan = json.loads(payload)
    assert payload == contract.canonical_json_bytes(plan)
    assert plan == contract.expected_plan()
    assert contract.validate_plan(plan) != "0" * 64
    assert b"--skip-write-canary" not in payload
    assert plan["post_cutover_contract"]["skip_write_canary_forbidden"] is True


def test_publish_lane_requires_owner_private_write_and_privacy_evidence() -> None:
    workflow = PUBLISH_WORKFLOW.read_text(encoding="utf-8")
    controller = SEALED_CONTROLLER.read_text(encoding="utf-8")
    bootstrap = json.loads(BOOTSTRAP_PLAN.read_bytes())

    assert "--skip-write-canary" not in workflow
    assert '--write-config "$TAIRA_POST_CUTOVER_CANARY_CLIENT_PATH"' in workflow
    assert "verify-privacy-rollout" in workflow
    assert "TAIRA_PRIVACY_ROLLOUT_OBSERVATION_PATH" in workflow
    assert '"--result", "--write-config"' in controller
    assert '"macos-deploy", "verify-privacy-rollout"' in controller
    assert bootstrap["governance_rollout"] == {
        "activation_state": "not-executed",
        "controller_observation_required": True,
        "genesis_activation_forbidden": True,
        "mode": "governance-four-wave",
        "notice_interval_blocks": contract.NOTICE_INTERVAL_BLOCKS,
        "observation_interval_blocks": contract.OBSERVATION_INTERVAL_BLOCKS,
        "rollout_plan_path": "configs/soranexus/taira/privacy_rollout_plan_v1.json",
        "rollout_plan_sha256": hashlib.sha256(PLAN.read_bytes()).hexdigest(),
    }


def test_complete_four_wave_observation_is_structurally_valid() -> None:
    result = _valid_result()
    plan_sha256, rollout_id = contract._validate_unsigned_result_structure(
        result, plan=contract.expected_plan()
    )
    assert plan_sha256 == result["plan_sha256"]
    assert rollout_id == result["rollout_id"]
    assert result["terminal"]["publication_authorized"] is True


def test_self_consistent_structure_cannot_claim_observation_authority() -> None:
    result = _valid_result()
    contract._validate_unsigned_result_structure(
        result, plan=contract.expected_plan()
    )
    with pytest.raises(
        contract.RolloutContractError,
        match=contract.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
    ):
        contract.validate_result(result, plan=contract.expected_plan())


@pytest.mark.parametrize(
    "mutate, message",
    [
        (
            lambda value: value["waves"].__setitem__(
                0, value["waves"][1]
            ),
            "identity, order",
        ),
        (
            lambda value: value["waves"][0]["protocols"].reverse(),
            "identity, order",
        ),
        (
            lambda value: value["waves"][0]["activation_transactions"][0].__setitem__(
                "governance_outcome", "submitted"
            ),
            "governance outcome",
        ),
        (
            lambda value: value["waves"][0]["post_activation_snapshots"][
                0
            ]["unavailable_protocols"].append("iroha-zk-ams-v1"),
            "unavailable",
        ),
        (
            lambda value: value["waves"][2]["observation_snapshots"][0].__setitem__(
                "jindo_missing_evidence", []
            ),
            "Jindo",
        ),
        (
            lambda value: value["post_cutover"]["canary"].__setitem__(
                "skipped", True
            ),
            "skipped",
        ),
        (
            lambda value: value["post_cutover"]["canary"].__setitem__(
                "governance_permission_present", True
            ),
            "privileged",
        ),
        (
            lambda value: value["waves"][3]["resources"][1].__setitem__(
                "work_units", 100_000_000_001
            ),
            "100-billion",
        ),
        (
            lambda value: value["waves"][3]["resources"][1].__setitem__(
                "evaluated_key_retrieval_bytes", 0
            ),
            "evaluated-key",
        ),
        (
            lambda value: value["waves"][0]["restart"].__setitem__(
                "recovered_hash", _digest("stale")
            ),
            "sentinel",
        ),
        (
            lambda value: value["waves"][0]["restart"]["peer_finality"].pop(),
            "omits",
        ),
        (
            lambda value: value["waves"][0]["canaries"][0]["replay"].__setitem__(
                "transaction_sha256", _digest("different-replay")
            ),
            "exact accepted",
        ),
        (
            lambda value: value["waves"][0]["canaries"][0]["negative"][
                "rejections"
            ][0].__setitem__(
                "observed_height",
                value["waves"][0]["observation_completed_at_height"] + 1,
            ),
            "observation window",
        ),
        (
            lambda value: value["waves"][0]["restart"].__setitem__(
                "stopped_height", 1
            ),
            "sentinel",
        ),
        (
            lambda value: value["post_cutover"]["canary"]["write"]["peer_queries"][
                0
            ].__setitem__("state_sha256", _digest("stale-write-state")),
            "write peer queries",
        ),
        (
            lambda value: value["post_cutover"]["canary"]["write"].__setitem__(
                "transaction_sha256",
                value["waves"][0]["activation_transactions"][0][
                    "transaction_sha256"
                ],
            ),
            "reused an earlier",
        ),
        (
            lambda value: value["rollback"].__setitem__("armed", False),
            "rollback",
        ),
        (
            lambda value: value["post_cutover"].__setitem__(
                "readmitted_candidate_oci_digest", _oci("stale-candidate")
            ),
            "same OCI",
        ),
        (
            lambda value: value["waves"][1].__setitem__(
                "proposed_at_height",
                value["waves"][0]["observation_completed_at_height"],
            ),
            "interval",
        ),
    ],
)
def test_hostile_rollout_substitutions_fail_closed(mutate, message: str) -> None:
    result = _valid_result()
    mutate(result)
    _redigest(result)
    with pytest.raises(contract.RolloutContractError, match=message):
        contract._validate_unsigned_result_structure(
            result, plan=contract.expected_plan()
        )


def test_raised_ceiling_and_retired_alias_plan_substitutions_reject() -> None:
    raised = contract.expected_plan()
    raised["resource_ceilings"]["max_work_units"] += 1
    with pytest.raises(contract.RolloutContractError, match="immutable"):
        contract.validate_plan(raised)

    alias = contract.expected_plan()
    alias["waves"][2]["protocols"][1] = "jindo-lattice-pcs-zk-v0"
    with pytest.raises(contract.RolloutContractError, match="immutable"):
        contract.validate_plan(alias)


def test_candidate_tuple_and_rollout_id_are_independently_rederived() -> None:
    result = _valid_result()
    result["candidate"]["archive_sha256"] = _digest("substituted-archive")
    _redigest(result)
    with pytest.raises(contract.RolloutContractError, match="candidate binding"):
        contract._validate_unsigned_result_structure(
            result, plan=contract.expected_plan()
        )

    result = _valid_result()
    result["rollout_id"] = _digest("fabricated-id")
    with pytest.raises(contract.RolloutContractError, match="rollout ID"):
        contract._validate_unsigned_result_structure(
            result, plan=contract.expected_plan()
        )


def test_cli_rejects_noncanonical_duplicate_truncated_and_suffixed_json(
    tmp_path: Path,
) -> None:
    valid = contract.canonical_json_bytes(contract.expected_plan())
    hostile = {
        "pretty": json.dumps(contract.expected_plan(), indent=2).encode() + b"\n",
        "duplicate": valid[:-2] + b',"schema_version":1}\n',
        "truncated": valid[:-4],
        "suffix": valid + b"{}\n",
    }
    for name, payload in hostile.items():
        path = tmp_path / f"{name}.json"
        path.write_bytes(payload)
        completed = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "verify-plan",
                "--plan",
                str(path),
            ],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        assert completed.returncode == 1, (name, completed.stdout, completed.stderr)


def test_cli_reports_structure_not_semantic_or_publication_authority() -> None:
    completed = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "verify-plan",
            "--plan",
            str(PLAN),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    output = json.loads(completed.stdout)
    assert output["status"] == "structurally-valid"
    assert output["qualification_authority"] is False


def test_cli_refuses_a_self_consistent_but_unauthenticated_observation(
    tmp_path: Path,
) -> None:
    result = _valid_result()
    result_path = tmp_path / "rollout-observation.json"
    result_path.write_bytes(contract.canonical_json_bytes(result))
    completed = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "verify-result",
            "--plan",
            str(PLAN),
            "--result",
            str(result_path),
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 1
    assert completed.stdout == ""
    assert (
        contract.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA
        in completed.stderr
    )


def test_result_schema_has_no_read_only_or_skip_qualification_path() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    result = _valid_result()
    assert result["post_cutover"]["canary"]["mode"] == "signed-write-and-privacy"
    assert result["post_cutover"]["canary"]["skipped"] is False
    assert "read-only checks passed" not in source
    assert "qualification_authority\": False" in source
