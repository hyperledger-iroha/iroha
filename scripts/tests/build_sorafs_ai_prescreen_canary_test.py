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
NOW_UNIX = GENERATED_AT
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
        "--now-unix",
        str(NOW_UNIX),
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


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )


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
    assert [probe["action"] for probe in payload["probes"]] == [
        "submit_commit",
        "submit_reveal",
    ]
    assert payload["payload_bytes_included"] is False
    assert payload["private_payloads_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "notification_transport"
    assert errors == []


def test_notification_default_probe_count_covers_shipped_actions(
    tmp_path: Path,
) -> None:
    args = args_for("notification_transport", tmp_path)
    index = args.index("--probe-count")
    del args[index : index + 2]

    assert MODULE.main(args) == 0

    payload = json.loads(
        canary_path(tmp_path, "notification_transport").read_text("utf-8")
    )
    assert payload["probe_count"] == len(MODULE.ALLOWED_NOTIFICATION_ACTIONS)
    assert [probe["action"] for probe in payload["probes"]] == list(
        MODULE.ALLOWED_NOTIFICATION_ACTIONS
    )


def test_notification_probe_count_must_cover_shipped_actions_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("notification_transport", tmp_path)
    args[args.index("--probe-count") + 1] = "1"

    assert_rejected_without_artifact(
        args,
        kind="notification_transport",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--probe-count must cover both shipped notification actions",
    )


@pytest.mark.parametrize(
    ("option", "value"),
    (
        ("--case-id", "case-\u200d1"),
        ("--round-id", "round-\u202e1"),
    ),
)
def test_notification_identity_arguments_reject_unicode_controls_before_write(
    option: str,
    value: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("notification_transport", tmp_path)
    args.extend([option, value])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "argument must be a non-empty canonical string" in captured.err
    assert value not in captured.err
    assert value.encode("unicode_escape").decode("ascii") not in captured.err
    assert not canary_path(tmp_path, "notification_transport").exists()


def test_generated_non_live_canaries_pass_their_kind_contract(tmp_path: Path) -> None:
    assert set(MODULE.CANARY_KINDS).isdisjoint(MODULE.EXTERNAL_EVIDENCE_ONLY_KINDS)
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        payload = json.loads(canary_path(tmp_path, kind).read_text("utf-8"))
        actual_kind, errors = CHECKER.validate_evidence_payload(
            payload,
            checker_options(),
        )
        assert actual_kind == kind
        assert errors == []


def test_runner_status_kind_inventory_matches_generated_statuses(tmp_path: Path) -> None:
    assert MODULE.RUNNER_STATUS_KINDS == {"runner", "committee"}
    assert set(MODULE.RUNNER_STATUS_KINDS).issubset(
        MODULE.EXTERNAL_EVIDENCE_ONLY_KINDS
    )

    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        payload = json.loads(canary_path(tmp_path, kind).read_text("utf-8"))
        assert payload["status"] == "passed"


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


@pytest.mark.parametrize("kind", MODULE.EXTERNAL_EVIDENCE_ONLY_KINDS)
def test_builder_rejects_external_evidence_only_kinds(
    kind: str, tmp_path: Path, capsys
) -> None:
    args = args_for(kind, tmp_path)

    assert MODULE.main(args) == 2

    assert "invalid choice" in capsys.readouterr().err
    assert not canary_path(tmp_path, kind).exists()


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


@pytest.mark.parametrize(
    "route_value",
    (
        f"{MODULE.REQUIRED_OPERATOR_ROUTES[0]}, {MODULE.REQUIRED_OPERATOR_ROUTES[1]}",
        f"{MODULE.REQUIRED_OPERATOR_ROUTES[0]},,{MODULE.REQUIRED_OPERATOR_ROUTES[1]}",
    ),
)
def test_operator_route_csv_rejects_padded_or_empty_components(
    route_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("operator_workflow", tmp_path)
    first_route = args.index("--operator-route") + 1
    args[first_route] = route_value

    assert_rejected_without_artifact(
        args,
        kind="operator_workflow",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--operator-route contains an unknown value",
    )


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


def test_executor_service_name_must_match_reviewed_service_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal_executor", tmp_path)
    args.extend(["--service-name", "sorafs-moderation-ballots-debug"])

    assert_rejected_without_artifact(
        args,
        kind="commit_reveal_executor",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--service-name must match the reviewed executor service",
    )


def test_executor_bundle_dir_rejects_unicode_controls_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    bundle_dir = "/tmp/sorafs-ai-prescreen-executor\u200d"
    args = args_for("commit_reveal_executor", tmp_path)
    args.extend(["--bundle-dir", bundle_dir])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "argument must be a non-empty canonical string" in captured.err
    assert bundle_dir not in captured.err
    assert bundle_dir.encode("unicode_escape").decode("ascii") not in captured.err
    assert not canary_path(tmp_path, "commit_reveal_executor").exists()


@pytest.mark.parametrize(
    ("kind", "attribute", "value", "expected_error"),
    (
        (
            "notification_transport",
            "case_id",
            "case-\u200d1",
            "--case-id must be a non-empty canonical string",
        ),
        (
            "notification_transport",
            "round_id",
            "round-\u202e1",
            "--round-id must be a non-empty canonical string",
        ),
        (
            "commit_reveal_executor",
            "bundle_dir",
            "/tmp/sorafs-ai-prescreen-executor\u200d",
            "--bundle-dir must be a non-empty canonical string",
        ),
    ),
)
def test_validate_inputs_rejects_parser_returned_unicode_controls_before_build(
    kind: str,
    attribute: str,
    value: str,
    expected_error: str,
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(args_for(kind, tmp_path))
    setattr(args, attribute, value)

    errors = MODULE.validate_inputs(args)

    assert expected_error in errors
    assert value not in "\n".join(errors)
    assert value.encode("unicode_escape").decode("ascii") not in "\n".join(errors)
    assert not canary_path(tmp_path, kind).exists()


def test_executor_action_count_must_match_action_breakdown_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("commit_reveal_executor", tmp_path)
    args.extend(["--action-count", "2"])

    assert_rejected_without_artifact(
        args,
        kind="commit_reveal_executor",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--action-count must equal commit, reveal, and tally action counts"
        ),
    )


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
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
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


@pytest.mark.parametrize(
    ("edge_value", "diagnostic"),
    (
        (
            " screening_ingest:ai-prescreen-governance-edge-screening-ingest",
            "argument must be a non-empty canonical string",
        ),
        (
            "screening_ingest :ai-prescreen-governance-edge-screening-ingest",
            "--governance-edge[0].producer must be a non-empty canonical string",
        ),
        (
            "screening_ingest: ai-prescreen-governance-edge-screening-ingest",
            "--governance-edge[0].name must be a non-empty canonical string",
        ),
        (
            "screening_ingest:ai-prescreen-governance-edge-screening-ingest ",
            "argument must be a non-empty canonical string",
        ),
    ),
)
def test_governance_edge_inventory_rejects_padded_tuple_components(
    edge_value: str,
    diagnostic: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("governance_dag", tmp_path)
    first_edge = args.index("--governance-edge") + 1
    args[first_edge] = edge_value

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert diagnostic in captured.err
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


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("operator_workflow", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = tmp_path / "runner-output"
    output_dir.mkdir()
    args = args_for("operator_workflow", tmp_path)
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


def test_base_url_arguments_reject_trailing_slash_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        ("operator_workflow", "--operator-url", "https://operator.example/"),
    )

    for index, (kind, flag, value) in enumerate(cases):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        args = args_for(kind, case_dir)
        args[args.index(flag) + 1] = value

        assert_rejected_without_artifact(
            args,
            kind=kind,
            tmp_path=case_dir,
            capsys=capsys,
            expected_error=MODULE.CANARY_BASE_URL_ARG_ERROR,
        )


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
