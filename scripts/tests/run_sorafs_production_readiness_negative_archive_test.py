"""Tests for the fixed SoraFS negative-promotion archive runner."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shlex
import stat
import subprocess
import sys
from pathlib import Path

import pytest


SCRIPT_DIR = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_DIR / "run_sorafs_production_readiness_negative_archive.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_production_readiness_negative_archive",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

FIXTURE_PATH = Path(__file__).with_name(
    "check_sorafs_production_readiness_test.py"
)
FIXTURE_SPEC = importlib.util.spec_from_file_location(
    "negative_archive_readiness_fixtures",
    FIXTURE_PATH,
)
FIXTURES = importlib.util.module_from_spec(FIXTURE_SPEC)
assert FIXTURE_SPEC and FIXTURE_SPEC.loader  # pragma: no cover - defensive
sys.modules[FIXTURE_SPEC.name] = FIXTURES
FIXTURE_SPEC.loader.exec_module(FIXTURES)


EXPECTED_MUTATION_IDS = (
    "tampered-lane-summary-bytes",
    "stale-explicit-clock",
    "missing-lane-summary",
    "duplicate-lane-summary",
    "predecessor-expectation-mismatch",
    "foundational-signature-forgery",
)
EXPECTED_DIAGNOSTIC_CLASSES = (
    "lane_summary_binding_mismatch",
    "summary_artifact_stale",
    "required_lane_missing",
    "required_lane_duplicate",
    "foundational_predecessor_mismatch",
    "foundational_signature_invalid",
)
EXPECTED_AGGREGATE_CONTRACT_ERRORS = (
    (
        "ai_prescreen aggregate foundational lane digest must match "
        "required row sha256",
    ),
    (),
    (),
    (),
    (),
    (),
)


def write_promotion_args(root: Path) -> Path:
    """Write one complete reviewed promotion response file."""

    FIXTURES.write_all_gates(root)
    foundation = FIXTURES.write_foundational_summary(root)
    qualification_args = FIXTURES.topology_cli_args(root)
    assert len(qualification_args) % 2 == 0
    qualification_lines = [
        shlex.join(
            [
                flag,
                (
                    str(
                        2
                        * MODULE.promotion_runner.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS
                    )
                    if flag == "--max-topology-qualification-review-age-secs"
                    else value
                ),
            ]
        )
        for flag, value in zip(
            qualification_args[::2], qualification_args[1::2]
        )
        if flag != "--l1-lane-summary"
    ]
    lines = [
        shlex.join(["--out-dir", str(root / "unused-positive-output")]),
        shlex.join(
            [
                "--summary-out",
                str(root / "unused-positive-output" / "aggregate.json"),
            ]
        ),
        *qualification_lines,
        shlex.join(["--foundational-prerequisite-summary", str(foundation)]),
        shlex.join(
            [
                "--foundational-prerequisite-signer-public-key-hex",
                FIXTURES.FOUNDATIONAL_SIGNER_PUBLIC_KEY.hex(),
            ]
        ),
        shlex.join(
            [
                "--foundational-prerequisite-release-sequence",
                str(FIXTURES.FOUNDATIONAL_RELEASE_SEQUENCE),
            ]
        ),
        shlex.join(
            [
                "--foundational-prerequisite-previous-envelope-sha256",
                FIXTURES.FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256,
            ]
        ),
        shlex.join(["--deployment-id", FIXTURES.DEPLOYMENT_ID]),
        shlex.join(["--environment", FIXTURES.ENVIRONMENT]),
        shlex.join(["--now-unix", str(FIXTURES.NOW_UNIX)]),
        shlex.join(
            [
                "--max-summary-artifact-age-secs",
                str(MODULE.promotion_runner.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS),
            ]
        ),
        shlex.join(
            [
                "--require-gate",
                ",".join(MODULE.DEFAULT_REQUIRED_GATES),
            ]
        ),
    ]
    for gate in MODULE.DEFAULT_REQUIRED_GATES:
        lines.append(
            shlex.join(
                [
                    MODULE.promotion_runner.SUMMARY_FLAGS_BY_GATE[gate],
                    str(root / f"{gate}.json"),
                ]
            )
        )
    path = root / "promotion-collection.args"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def baseline_input_digests(root: Path) -> dict[str, str]:
    """Hash every source summary before the isolated archive run."""

    paths = [
        root / "l1-topology-qualification.summary",
        root / "l1-topology-qualification.summary.ed25519",
        root / "l1-resilience-qualification.summary",
        root / "l1-lane-evidence.inventory",
        root / "foundational_prerequisites.json",
        *(root / f"{gate}.json" for gate in MODULE.DEFAULT_REQUIRED_GATES),
    ]
    return {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in paths
    }


def test_closed_mutation_matrix_is_exact() -> None:
    assert tuple(case.mutation_id for case in MODULE.MUTATION_CASES) == (
        EXPECTED_MUTATION_IDS
    )
    assert tuple(case.diagnostic_class for case in MODULE.MUTATION_CASES) == (
        EXPECTED_DIAGNOSTIC_CLASSES
    )
    assert tuple(
        case.expected_aggregate_contract_errors
        for case in MODULE.MUTATION_CASES
    ) == EXPECTED_AGGREGATE_CONTRACT_ERRORS
    assert len(MODULE.MUTATION_BY_ID) == 6


def test_aggregate_contract_errors_are_exactly_case_bound() -> None:
    tampered = MODULE.MUTATION_CASES[0]
    expected = tampered.expected_aggregate_contract_errors
    summary_errors = [tampered.diagnostic_fragment, *expected]

    assert MODULE._aggregate_contract_matches_case(
        tampered,
        summary_errors,
        expected,
    )
    assert not MODULE._aggregate_contract_matches_case(
        tampered,
        summary_errors,
        (),
    )
    assert not MODULE._aggregate_contract_matches_case(
        tampered,
        summary_errors,
        (*expected, "unexpected aggregate contract drift"),
    )
    assert not MODULE._aggregate_contract_matches_case(
        tampered,
        [tampered.diagnostic_fragment],
        expected,
    )
    assert not MODULE._aggregate_contract_matches_case(
        tampered,
        [*summary_errors, *expected],
        expected,
    )

    clean = MODULE.MUTATION_CASES[1]
    assert MODULE._aggregate_contract_matches_case(clean, [], ())
    assert not MODULE._aggregate_contract_matches_case(
        clean,
        [],
        ("unexpected aggregate contract drift",),
    )


def test_tamper_mutation_changes_only_json_encoding() -> None:
    raw = b'{"b": 2, "a": 1}'

    mutated = MODULE._semantically_equivalent_json_mutation(raw)

    assert mutated != raw
    assert len(mutated) <= MODULE.MAX_SUMMARY_BYTES
    assert json.loads(mutated) == json.loads(raw)


def test_toolchain_snapshot_binds_runner_checker_and_inventory() -> None:
    snapshot = MODULE._snapshot_toolchain()

    names = tuple(row[0] for row in snapshot.rows)
    digests = {row[0]: row[3] for row in snapshot.rows}
    assert names == tuple(sorted(names))
    assert len(names) == len(set(names))
    assert snapshot.runner_sha256 == digests[MODULE.BUNDLED_RUNNER.name]
    assert snapshot.checker_sha256 == digests[MODULE.BUNDLED_CHECKER.name]
    assert len(snapshot.aggregate_sha256) == 64


def test_bounded_process_rejects_output_before_buffering_past_limit(
    tmp_path: Path,
    monkeypatch,
) -> None:
    monkeypatch.setattr(MODULE, "MAX_PROCESS_OUTPUT_BYTES", 1024)

    with pytest.raises(
        MODULE.NegativeArchiveError,
        match="output exceeded its bound",
    ):
        MODULE._run_bounded(
            [
                sys.executable,
                "-c",
                "import sys; sys.stdout.buffer.write(b'x' * 4096)",
            ],
            tmp_path,
        )


def test_bounded_process_cleans_up_on_base_exception(
    tmp_path: Path,
    monkeypatch,
) -> None:
    selector_probe = MODULE.selectors.DefaultSelector()
    selector_type = type(selector_probe)
    selector_probe.close()
    stopped_processes = []
    original_stop = MODULE._stop_child_process

    def interrupt_select(_selector, timeout=None):
        del timeout
        raise KeyboardInterrupt

    def record_stop(process):
        stopped_processes.append(process)
        original_stop(process)

    monkeypatch.setattr(selector_type, "select", interrupt_select)
    monkeypatch.setattr(MODULE, "_stop_child_process", record_stop)

    with pytest.raises(KeyboardInterrupt):
        MODULE._run_bounded(
            [
                sys.executable,
                "-c",
                "import time; time.sleep(30)",
            ],
            tmp_path,
        )

    assert len(stopped_processes) == 1
    assert stopped_processes[0].poll() is not None
    if os.name == "posix":
        assert not MODULE._posix_process_group_exists(
            stopped_processes[0].pid
        )


@pytest.mark.skipif(
    os.name != "posix",
    reason="private process-group cleanup is POSIX-only",
)
def test_stop_child_escalates_when_descendants_outlive_group_leader(
    monkeypatch,
) -> None:
    class FakeProcess:
        pid = 424_242

        def __init__(self) -> None:
            self.wait_calls = 0

        def poll(self):
            return 0

        def wait(self, timeout):
            self.wait_calls += 1
            return 0

        def kill(self):
            raise AssertionError("direct kill fallback was not expected")

    process = FakeProcess()
    signals = []
    wait_results = iter((False, True))

    monkeypatch.setattr(
        MODULE.os,
        "killpg",
        lambda process_group_id, process_signal: signals.append(
            (process_group_id, process_signal)
        ),
    )
    monkeypatch.setattr(
        MODULE,
        "_wait_for_process_group_exit",
        lambda _process_group_id, _process, _timeout: next(wait_results),
    )

    MODULE._stop_child_process(process)

    assert signals == [
        (process.pid, MODULE.signal.SIGTERM),
        (process.pid, MODULE.signal.SIGKILL),
    ]
    assert process.wait_calls == 1


def test_example_response_file_parses_without_runtime_material() -> None:
    example = SCRIPT_DIR / "examples" / (
        "sorafs_production_readiness_negative_archive.args.example"
    )

    args = MODULE.parse_args([f"@{example}"])

    assert args.promotion_args_file == Path(
        "/runtime/evidence/sorafs-production-readiness-collection.args"
    )
    assert args.archive_out_dir == Path(
        "/runtime/evidence/sorafs-negative-promotion-archive"
    )
    assert "private" not in example.read_text(encoding="utf-8").lower()


def test_runner_archives_six_payload_free_negative_receipts(
    tmp_path: Path,
    capsys,
) -> None:
    promotion_args = write_promotion_args(tmp_path)
    before = baseline_input_digests(tmp_path)
    archive = tmp_path / "negative-promotion-archive"

    assert (
        MODULE.main(
            [
                "--promotion-args-file",
                str(promotion_args),
                "--archive-out-dir",
                str(archive),
            ]
        )
        == 0
    )

    assert baseline_input_digests(tmp_path) == before
    assert not (tmp_path / "unused-positive-output").exists()
    assert stat.S_IMODE(archive.stat().st_mode) == 0o700
    files = sorted(path.name for path in archive.iterdir())
    expected_receipt_files = [
        f"{index:02d}-{mutation_id}.json"
        for index, mutation_id in enumerate(EXPECTED_MUTATION_IDS, start=1)
    ]
    assert files == [
        *expected_receipt_files,
        MODULE.ARCHIVE_MANIFEST_FILENAME,
    ]

    manifest_path = archive / MODULE.ARCHIVE_MANIFEST_FILENAME
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert stat.S_IMODE(manifest_path.stat().st_mode) == 0o600
    assert manifest["schema"] == MODULE.ARCHIVE_SCHEMA
    assert manifest["status"] == MODULE.ARCHIVE_STATUS
    assert manifest["attestation_scope"] == MODULE.ARCHIVE_ATTESTATION_SCOPE
    assert manifest["externally_authenticated"] is False
    assert manifest["promotion_eligible"] is False
    assert manifest["baseline_input_count"] == MODULE.BASELINE_INPUT_COUNT
    assert manifest["mutation_count"] == 6
    assert manifest["mutation_ids"] == list(EXPECTED_MUTATION_IDS)
    assert set(manifest["baseline_output_sha256"]) == (
        MODULE.BASELINE_OUTPUT_HASH_FIELDS
    )
    assert (
        MODULE.validate_archive_manifest(
            manifest,
            baseline_input_set_sha256=manifest["baseline_input_set_sha256"],
            runner_sha256=manifest["aggregate_runner_sha256"],
            checker_sha256=manifest["aggregate_checker_sha256"],
            toolchain_sha256=manifest["aggregate_toolchain_sha256"],
            python_runtime=MODULE._python_runtime(),
        )
        == []
    )

    archived_text = manifest_path.read_text(encoding="utf-8")
    for index, (case, row) in enumerate(
        zip(MODULE.MUTATION_CASES, manifest["receipts"]),
        start=1,
    ):
        receipt_path = archive / row["receipt_file"]
        receipt_raw = receipt_path.read_bytes()
        receipt = json.loads(receipt_raw)
        assert stat.S_IMODE(receipt_path.stat().st_mode) == 0o600
        assert row == {
            "mutation_id": case.mutation_id,
            "receipt_file": f"{index:02d}-{case.mutation_id}.json",
            "sha256": hashlib.sha256(receipt_raw).hexdigest(),
        }
        assert receipt["expected_rejection"] == {
            "checker_exit_code": 1,
            "aggregate_status": "blocked",
            "diagnostic_class": case.diagnostic_class,
        }
        assert receipt["observed_diagnostic_class"] == case.diagnostic_class
        assert (
            MODULE.validate_receipt(
                receipt,
                case=case,
                baseline_input_set_sha256=manifest[
                    "baseline_input_set_sha256"
                ],
                checker_sha256=manifest["aggregate_checker_sha256"],
                toolchain_sha256=manifest["aggregate_toolchain_sha256"],
            )
            == []
        )
        archived_text += receipt_raw.decode("utf-8")

    assert "signature_hex" not in archived_text
    assert "lane_summaries" not in archived_text
    assert "artifacts/" not in archived_text
    assert FIXTURES.DEPLOYMENT_ID not in archived_text
    captured = capsys.readouterr()
    assert "locally qualified all six fixed rejection cases" in captured.err
    assert "external provenance is still required" in captured.err
    assert captured.out == ""


def test_runner_rejects_non_ready_baseline_without_archive(
    tmp_path: Path,
    capsys,
) -> None:
    promotion_args = write_promotion_args(tmp_path)
    lane = tmp_path / "ai_prescreen.json"
    lane.write_bytes(lane.read_bytes() + b"\n")
    archive = tmp_path / "negative-promotion-archive"

    assert (
        MODULE.main(
            [
                "--promotion-args-file",
                str(promotion_args),
                "--archive-out-dir",
                str(archive),
            ]
        )
        == 1
    )

    assert not archive.exists()
    captured = capsys.readouterr()
    assert "pinned promotion runner rejected the reviewed baseline" in captured.err
    assert "lane_summaries" not in captured.err
    assert captured.out == ""


def test_runner_rejects_existing_archive_before_reading_inputs(
    tmp_path: Path,
    capsys,
) -> None:
    archive = tmp_path / "negative-promotion-archive"
    archive.mkdir()
    missing_args = tmp_path / "missing-promotion.args"

    assert (
        MODULE.main(
            [
                "--promotion-args-file",
                str(missing_args),
                "--archive-out-dir",
                str(archive),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--archive-out-dir must not already exist" in captured.err
    assert "@ARGFILE" not in captured.err
    assert captured.out == ""


def test_runner_rejects_world_writable_parent_before_reading_inputs(
    tmp_path: Path,
    capsys,
) -> None:
    if not hasattr(os, "geteuid"):
        return
    shared_parent = tmp_path / "shared"
    shared_parent.mkdir(mode=0o700)
    shared_parent.chmod(0o777)
    archive = shared_parent / "negative-promotion-archive"
    missing_args = tmp_path / "missing-promotion.args"

    try:
        assert (
            MODULE.main(
                [
                    "--promotion-args-file",
                    str(missing_args),
                    "--archive-out-dir",
                    str(archive),
                ]
            )
            == 2
        )
    finally:
        shared_parent.chmod(0o700)

    captured = capsys.readouterr()
    assert "parent must not be group- or world-writable" in captured.err
    assert "@ARGFILE" not in captured.err
    assert captured.out == ""


def test_receipt_and_manifest_reject_unknown_fields() -> None:
    case = MODULE.MUTATION_CASES[0]
    digest = "12" * 32
    receipt = {
        "schema": MODULE.RECEIPT_SCHEMA,
        "mutation_id": case.mutation_id,
        "baseline_input_set_sha256": digest,
        "aggregate_checker_sha256": digest,
        "aggregate_toolchain_sha256": digest,
        "expected_rejection": {
            "checker_exit_code": 1,
            "aggregate_status": "blocked",
            "diagnostic_class": case.diagnostic_class,
        },
        "observed_diagnostic_class": case.diagnostic_class,
        "output_sha256": {
            field: digest for field in MODULE.OUTPUT_HASH_FIELDS
        },
        "errors": [],
        "raw_payload": "must-not-be-accepted",
    }
    assert any(
        "schema-closed" in error
        for error in MODULE.validate_receipt(
            receipt,
            case=case,
            baseline_input_set_sha256=digest,
            checker_sha256=digest,
            toolchain_sha256=digest,
        )
    )

    runtime = MODULE.PythonRuntime(
        executable=Path(sys.executable),
        implementation="cpython",
        version="3.12.0",
        executable_sha256=digest,
    )
    manifest = {
        "schema": MODULE.ARCHIVE_SCHEMA,
        "status": MODULE.ARCHIVE_STATUS,
        "attestation_scope": MODULE.ARCHIVE_ATTESTATION_SCOPE,
        "externally_authenticated": False,
        "promotion_eligible": False,
        "baseline_input_count": MODULE.BASELINE_INPUT_COUNT,
        "baseline_input_set_sha256": digest,
        "aggregate_runner_sha256": digest,
        "aggregate_checker_sha256": digest,
        "aggregate_toolchain_sha256": digest,
        "python_runtime": runtime.receipt_value(),
        "baseline_output_sha256": {
            field: digest for field in MODULE.BASELINE_OUTPUT_HASH_FIELDS
        },
        "mutation_count": 6,
        "mutation_ids": list(EXPECTED_MUTATION_IDS),
        "receipts": [
            {
                "mutation_id": case.mutation_id,
                "receipt_file": f"{index:02d}-{case.mutation_id}.json",
                "sha256": digest,
            }
            for index, case in enumerate(MODULE.MUTATION_CASES, start=1)
        ],
        "errors": [],
        "evidence": {},
    }
    assert any(
        "schema-closed" in error
        for error in MODULE.validate_archive_manifest(
            manifest,
            baseline_input_set_sha256=digest,
            runner_sha256=digest,
            checker_sha256=digest,
            toolchain_sha256=digest,
            python_runtime=runtime,
        )
    )
