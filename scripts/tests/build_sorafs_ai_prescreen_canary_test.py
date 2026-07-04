"""Tests for scripts/build_sorafs_ai_prescreen_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_ai_prescreen_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_ai_prescreen_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_ai_prescreen_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_ai_prescreen_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


DIGEST = "a" * 64
POLICY_DIGEST = "b" * 64
MANIFEST_ID = "c" * 32
QUARANTINE_ID = "d" * 32
GENERATED_AT = 1_800_400_000
SUBJECT_REFERENCE = "cid:bafyprodmoderation20260701"


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "ai-prescreen-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--body-digest-hex",
        DIGEST,
    ]
    if kind in MODULE.RUNNER_BINDING_KINDS:
        args.extend(
            [
                "--manifest-id-hex",
                MANIFEST_ID,
                "--runner-hash-hex",
                DIGEST,
                "--subject",
                SUBJECT_REFERENCE,
                "--subject-digest-hex",
                DIGEST,
            ]
        )
    if kind in MODULE.WORKFLOW_DIGEST_KINDS:
        args.extend(["--workflow-digest-hex", DIGEST])
    if kind == "runner":
        args.extend(
            [
                "--runner-url",
                "https://runner.example",
                "--evidence-digest-hex",
                DIGEST,
                "--policy-digest-hex",
                POLICY_DIGEST,
            ]
        )
    elif kind == "committee":
        args.extend(["--committee-url", "https://committee.example"])
        for result in (
            "ai-prescreen-committee-result-a",
            "ai-prescreen-committee-result-b",
            "ai-prescreen-committee-result-c",
        ):
            args.extend(["--committee-result", result])
    elif kind == "operator_workflow":
        args.extend(
            [
                "--operator-url",
                "https://operator.example",
                "--quarantine-id-hex",
                QUARANTINE_ID,
            ]
        )
        for route in MODULE.REQUIRED_OPERATOR_ROUTES:
            args.extend(["--operator-route", route])
    elif kind == "notification_transport":
        args.extend(
            [
                "--webhook-url",
                "https://notifications.example/hook",
                "--probe-count",
                "2",
            ]
        )
    elif kind == "transparency_publication":
        for source_kind in MODULE.REQUIRED_TRANSPARENCY_SOURCE_KINDS:
            args.extend(["--transparency-source-kind", source_kind])
    elif kind == "governance_dag":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
        for producer in MODULE.REQUIRED_GOVERNANCE_PRODUCERS:
            args.extend(["--governance-producer", producer])
            args.extend(
                [
                    "--governance-edge",
                    (
                        f"{producer}:ai-prescreen-governance-edge-"
                        f"{producer.replace('_', '-')}"
                    ),
                ]
            )
        args.extend(["--edge-count", str(len(MODULE.REQUIRED_GOVERNANCE_PRODUCERS))])
    elif kind == "end_to_end_workflow":
        args.extend(["--workflow-id", "sfm-4a-prod-canary-20260701"])
        for step in MODULE.REQUIRED_E2E_STEPS:
            args.extend(["--workflow-step", step])
    return args


def assert_rejected_without_artifact(
    args: list[str],
    *,
    kind: str,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_builds_payload_free_notification_transport_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("notification_transport", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "notification_transport").read_text("utf-8")
    )

    assert payload["schema"] == (
        "sorafs.moderation.juror_notifications.transport_canary.v1"
    )
    assert payload["workflow_digest_hex"] == DIGEST
    assert payload["probe_count"] == 2
    assert payload["accepted_count"] == 2
    assert [
        probe["delivery_id"] for probe in payload["probes"]
    ] == [
        "ai-prescreen-notification-delivery-01",
        "ai-prescreen-notification-delivery-02",
    ]
    assert payload["payload_bytes_included"] is False
    assert payload["private_payloads_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload)
    assert kind == "notification_transport"
    assert errors == []


def test_generated_canaries_pass_full_ai_prescreen_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = []
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["recognized_artifact_count"] == len(MODULE.CANARY_KINDS)
    assert payload["valid_runner_bindings"] == [
        {
            "manifest_id_hex": MANIFEST_ID,
            "runner_hash_hex": DIGEST,
            "subject_digest_hex": DIGEST,
        }
    ]
    assert payload["valid_workflow_digests"] == [DIGEST]
    assert payload["valid_notification_manifest_digests"] == [DIGEST]
    assert payload["valid_executor_summary_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    committee_payload = json.loads(canary_path(tmp_path, "committee").read_text("utf-8"))
    assert committee_payload["results"] == [
        {"name": "ai-prescreen-committee-result-a"},
        {"name": "ai-prescreen-committee-result-b"},
        {"name": "ai-prescreen-committee-result-c"},
    ]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_end_to_end_workflow_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "e2e.args"
    args_file.write_text(
        "\n".join(args_for("end_to_end_workflow", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(
        canary_path(tmp_path, "end_to_end_workflow").read_text("utf-8")
    )
    assert payload["step_count"] == len(MODULE.REQUIRED_E2E_STEPS)
    assert payload["passed_step_count"] == len(MODULE.REQUIRED_E2E_STEPS)


def test_workflow_id_must_be_canonical(tmp_path: Path, capsys) -> None:
    args = args_for("end_to_end_workflow", tmp_path)
    args[args.index("--workflow-id") + 1] = "sfm_4a_prod_canary_20260701"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--workflow-id must match canonical lowercase `sfm-4a-*`" in captured.err
    assert not canary_path(tmp_path, "end_to_end_workflow").exists()


def test_workflow_id_rejects_non_production_markers(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("end_to_end_workflow", tmp_path)
    args[args.index("--workflow-id") + 1] = "sfm-4a-prod-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--workflow-id must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "end_to_end_workflow").exists()


def test_workflow_id_accepts_future_production_label(tmp_path: Path) -> None:
    args = args_for("end_to_end_workflow", tmp_path)
    args[args.index("--workflow-id") + 1] = "sfm-4a-prod-canary-20260715"

    assert MODULE.main(args) == 0

    payload = json.loads(
        canary_path(tmp_path, "end_to_end_workflow").read_text("utf-8")
    )
    assert payload["workflow_id"] == "sfm-4a-prod-canary-20260715"


def test_subject_must_be_canonical(tmp_path: Path, capsys) -> None:
    args = args_for("runner", tmp_path)
    args[args.index("--subject") + 1] = "CID:prod-canary"

    assert_rejected_without_artifact(
        args,
        kind="runner",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--subject must match canonical lowercase `cid:name`",
    )


def test_subject_rejects_non_production_markers(tmp_path: Path, capsys) -> None:
    args = args_for("runner", tmp_path)
    args[args.index("--subject") + 1] = "cid:example"

    assert_rejected_without_artifact(
        args,
        kind="runner",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--subject must not contain non-production markers ['example']",
    )


def test_subject_accepts_future_production_reference(tmp_path: Path) -> None:
    subject = "cid:bafyprodmoderation20260715"
    args = args_for("runner", tmp_path)
    args[args.index("--subject") + 1] = subject

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "runner").read_text("utf-8"))
    assert payload["subject"] == subject


@pytest.mark.parametrize(
    ("option", "expected_error"),
    (
        ("--evidence-digest-hex", "--evidence-digest-hex is required for runner"),
        ("--policy-digest-hex", "--policy-digest-hex is required for runner"),
    ),
)
def test_runner_canary_requires_digest_options(
    option: str,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("runner", tmp_path)
    index = args.index(option)
    del args[index : index + 2]

    assert_rejected_without_artifact(
        args,
        kind="runner",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


@pytest.mark.parametrize("kind", ("runner", "committee"))
def test_score_bps_rejects_out_of_range_before_write(
    kind: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for(kind, tmp_path)
    args.extend(["--score-bps", "10001"])

    assert_rejected_without_artifact(
        args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--score-bps must be <= 10000",
    )


def test_missing_operator_route_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("operator_workflow", tmp_path)
    index = args.index("--operator-route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--operator-route must include every required value" in captured.err
    assert not canary_path(tmp_path, "operator_workflow").exists()


def test_operator_workflow_canary_records_passed_route_count(tmp_path: Path) -> None:
    assert MODULE.main(args_for("operator_workflow", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "operator_workflow").read_text("utf-8")
    )
    assert payload["route_count"] == len(MODULE.REQUIRED_OPERATOR_ROUTES)
    assert payload["passed_route_count"] == payload["route_count"]
    assert {
        route["name"]: route["path"] for route in payload["routes"]
    } == CHECKER.operator_route_paths(QUARANTINE_ID)
    assert {
        route["name"]: route["url"] for route in payload["routes"]
    } == {
        name: CHECKER.expected_operator_route_url(payload["operator_url"], path)
        for name, path in CHECKER.operator_route_paths(QUARANTINE_ID).items()
    }
    assert {
        route["name"]: route["content_type"] for route in payload["routes"]
    } == CHECKER.REQUIRED_OPERATOR_CONTENT_TYPES


def test_committee_result_count_must_cover_quorum(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("committee", tmp_path)
    args.extend(["--quorum", "4", "--result-count", "3"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--result-count must be >= --quorum" in captured.err
    assert not canary_path(tmp_path, "committee").exists()


def test_committee_result_inventory_must_match_result_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("committee", tmp_path)
    args.extend(["--result-count", "4"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--committee-result unique values must match --result-count" in captured.err
    assert not canary_path(tmp_path, "committee").exists()


def test_committee_result_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("committee", tmp_path)
    first_result = args.index("--committee-result") + 1
    args.extend(["--committee-result", args[first_result]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--committee-result must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "committee").exists()


def test_committee_result_inventory_requires_production_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("committee", tmp_path)
    first_result = args.index("--committee-result") + 1
    args[first_result] = "runner-result-a"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert MODULE.COMMITTEE_RESULT_LABEL_ERROR in captured.err
    assert not canary_path(tmp_path, "committee").exists()


def test_committee_result_inventory_rejects_placeholder_marker(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("committee", tmp_path)
    first_result = args.index("--committee-result") + 1
    args[first_result] = "ai-prescreen-committee-result-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--committee-result[0] must not contain non-production markers "
        "['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "committee").exists()


def test_governance_dag_canary_records_edge_inventory(tmp_path: Path) -> None:
    assert MODULE.main(args_for("governance_dag", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "governance_dag").read_text("utf-8"))
    expected_edges = [
        {
            "producer": producer,
            "name": f"ai-prescreen-governance-edge-{producer.replace('_', '-')}",
        }
        for producer in MODULE.REQUIRED_GOVERNANCE_PRODUCERS
    ]
    assert payload["edge_count"] == len(MODULE.REQUIRED_GOVERNANCE_PRODUCERS)
    assert payload["edges"] == expected_edges
    kind, errors = CHECKER.validate_evidence_payload(payload)
    assert kind == "governance_dag"
    assert errors == []


def test_governance_edge_inventory_must_match_edge_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    args.extend(["--edge-count", str(len(MODULE.REQUIRED_GOVERNANCE_PRODUCERS) + 1)])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--governance-edge unique names must match --edge-count" in captured.err
    assert not canary_path(tmp_path, "governance_dag").exists()


def test_governance_edge_count_must_match_required_producer_inventory(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    extra_edge_count = len(MODULE.REQUIRED_GOVERNANCE_PRODUCERS) + 1
    args.extend(
        [
            "--governance-edge",
            "screening_ingest:ai-prescreen-governance-edge-screening-ingest-extra",
            "--edge-count",
            str(extra_edge_count),
        ]
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--edge-count must match required governance producer inventory" in captured.err
    assert not canary_path(tmp_path, "governance_dag").exists()


def test_governance_edge_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    first_edge = args.index("--governance-edge") + 1
    args.extend(["--governance-edge", args[first_edge]])
    args.extend(["--edge-count", str(len(MODULE.REQUIRED_GOVERNANCE_PRODUCERS) + 1)])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--governance-edge names must not contain duplicates" in captured.err
    assert "--governance-edge unique names must match --edge-count" in captured.err
    assert not canary_path(tmp_path, "governance_dag").exists()


def test_governance_edge_inventory_must_cover_required_producers(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    first_edge = args.index("--governance-edge")
    del args[first_edge : first_edge + 2]
    count_index = args.index("--edge-count")
    args[count_index + 1] = str(len(MODULE.REQUIRED_GOVERNANCE_PRODUCERS) - 1)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--governance-edge must include every required producer" in captured.err
    assert not canary_path(tmp_path, "governance_dag").exists()


def test_governance_edge_inventory_rejects_unknown_producer(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    first_edge = args.index("--governance-edge") + 1
    args[first_edge] = "unknown:ai-prescreen-governance-edge-screening-ingest"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--governance-edge producer must be a required producer" in captured.err
    assert not canary_path(tmp_path, "governance_dag").exists()


def test_governance_edge_inventory_requires_production_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    first_edge = args.index("--governance-edge") + 1
    args[first_edge] = "screening_ingest:screening-ingest-edge"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert MODULE.GOVERNANCE_EDGE_LABEL_ERROR in captured.err
    assert not canary_path(tmp_path, "governance_dag").exists()


def test_governance_edge_inventory_rejects_placeholder_marker(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    first_edge = args.index("--governance-edge") + 1
    args[first_edge] = "screening_ingest:ai-prescreen-governance-edge-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--governance-edge[0].name must not contain non-production markers "
        "['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "governance_dag").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "operator_workflow",
            "--operator-route",
            MODULE.REQUIRED_OPERATOR_ROUTES[0],
            "unreviewed-operator-route",
        ),
        (
            "transparency_publication",
            "--transparency-source-kind",
            MODULE.REQUIRED_TRANSPARENCY_SOURCE_KINDS[0],
            "unreviewed-transparency-source",
        ),
        (
            "governance_dag",
            "--governance-producer",
            MODULE.REQUIRED_GOVERNANCE_PRODUCERS[0],
            "unreviewed-governance-producer",
        ),
        (
            "end_to_end_workflow",
            "--workflow-step",
            MODULE.REQUIRED_E2E_STEPS[0],
            "unreviewed-workflow-step",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    kind: str,
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for(kind, tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for(kind, unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_verdict_input_rejects_unknown_value_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("runner", tmp_path)
    args.extend(["--verdict", "allow"])

    assert_rejected_without_artifact(
        args,
        kind="runner",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--verdict must be pass, warn, quarantine, escalate, or block",
    )


def test_verdict_input_accepts_shipped_block_value(tmp_path: Path) -> None:
    args = args_for("committee", tmp_path)
    args.extend(["--verdict", "block"])

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "committee").read_text("utf-8"))
    assert payload["verdict"] == "block"
    kind, errors = CHECKER.validate_evidence_payload(payload)
    assert kind == "committee"
    assert errors == []


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("runner", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = tmp_path / "runner-output"
    output_dir.mkdir()
    args = args_for("runner", tmp_path)
    args[args.index("--out") + 1] = str(output_dir)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()


def test_url_arguments_reject_encoded_or_secret_bearing_values_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        (
            "runner",
            "--runner-url",
            "https://user:private_key@runner.example",
            ("private_key",),
        ),
        (
            "runner",
            "--runner-status-url",
            "https://runner.example/%2e%2e/status",
            ("%2e%2e",),
        ),
        (
            "runner",
            "--runner-screen-url",
            "https://runner.example/bad%2Fscreen",
            ("bad%2Fscreen",),
        ),
        (
            "committee",
            "--committee-aggregate-url",
            "https://committee.example/C%3A/aggregate",
            ("C%3A",),
        ),
        (
            "committee",
            "--committee-aggregate-url",
            "https://C%3A.committee.example/aggregate",
            ("C%3A",),
        ),
        (
            "committee",
            "--committee-aggregate-url",
            "https://http%3A.committee.example/aggregate",
            ("http%3A",),
        ),
        (
            "operator_workflow",
            "--operator-url",
            "https://operator.example/%70rivate_key",
            ("%70rivate_key", "private_key"),
        ),
        (
            "notification_transport",
            "--webhook-url",
            "https://notifications.example/hook?token=secret",
            ("token=secret",),
        ),
    )

    for index, (kind, flag, value, leaked_tokens) in enumerate(cases):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        args = args_for(kind, case_dir)
        if flag in args:
            args[args.index(flag) + 1] = value
        else:
            args.extend([flag, value])

        assert MODULE.main(args) == 2

        captured = capsys.readouterr()
        assert MODULE.CANARY_URL_ARG_ERROR in captured.err
        for leaked_token in leaked_tokens:
            assert leaked_token not in captured.err
        assert not canary_path(case_dir, kind).exists()


def test_path_arguments_reject_encoded_or_platform_values_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        (
            "notification_transport",
            "--manifest-path",
            "manifests/%2e%2e/private_key.json",
            ("%2e%2e", "private_key"),
        ),
        (
            "commit_reveal_executor",
            "--execution-summary-path",
            "summaries/C%3A/private_key.json",
            ("C%3A", "private_key"),
        ),
        (
            "commit_reveal_executor",
            "--execution-summary-path",
            "summaries/bad%2Fprivate_key.json",
            ("bad%2Fprivate_key", "private_key"),
        ),
    )

    for index, (kind, flag, value, leaked_tokens) in enumerate(cases):
        case_dir = tmp_path / f"path-case-{index}"
        case_dir.mkdir()
        args = args_for(kind, case_dir)
        args.extend([flag, value])

        assert MODULE.main(args) == 2

        captured = capsys.readouterr()
        assert MODULE.CANARY_PATH_ARG_ERROR in captured.err
        for leaked_token in leaked_tokens:
            assert leaked_token not in captured.err
        assert not canary_path(case_dir, kind).exists()
