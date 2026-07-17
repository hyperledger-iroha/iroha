"""Contract tests for the aggregate Sumeragi v2 release receipt."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import sys

import pytest

ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py"
FINAL_MARKER = (
    "Sumeragi v2 formal gate passed: source-bound TLAPS, adversarial scheduler "
    "mutations, bounded TLC, trace replay, and production Verus"
)
SCENARIOS = (
    "authoritative_v2_genesis_commits_on_every_validator",
    "authoritative_v2_finalizes_through_validator_restart",
    "taira_npos_leader_timeout_commits_within_rotation_bound",
    "real_network_divergent_prepare_qcs_converge_after_ordered_release",
)
SUMMARY_FIELDS = (
    "profile",
    "source_manifest_sha256",
    "scenario",
    "seed",
    "result",
    "cargo_status",
    "tee_status",
    "run_log_sha256",
    "output",
    "localnet",
    "command",
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_tsv(path: Path, fields: dict[str, str]) -> None:
    path.write_text(
        "".join(f"{name}\t{value}\n" for name, value in fields.items()),
        encoding="utf-8",
    )


def fixture_writer(tmp_path: Path) -> Path:
    project = tmp_path / "writer-project"
    scripts = project / "scripts"
    formal = scripts / "formal"
    formal.mkdir(parents=True)
    writer = scripts / SCRIPT.name
    shutil.copy2(SCRIPT, writer)
    shutil.copy2(
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh",
        scripts / "run_sumeragi_v2_release_gates.sh",
    )
    fixture_cargo = project / ".cargo"
    fixture_cargo.mkdir()
    shutil.copy2(ROOT_DIR / ".cargo" / "config.toml", fixture_cargo / "config.toml")
    (formal / "check_sumeragi_v2_proof_ledger.py").write_text(
        "raise SystemExit(0)\n", encoding="utf-8"
    )
    (scripts / "check_taira_v2_soak_evidence.py").write_text(
        "raise SystemExit(0)\n", encoding="utf-8"
    )
    return writer


def make_evidence(tmp_path: Path) -> dict[str, Path | str | list[Path]]:
    candidate_manifest = "a" * 64
    sealed_manifest = "b" * 64
    head = "1" * 40
    tree = "2" * 40
    lock = "3" * 64
    candidate = tmp_path / "candidate.json"
    sealed = tmp_path / "sealed.json"
    identity = {
        "schema_version": 1,
        "head_commit": head,
        "head_tree": tree,
        "index_tree": tree,
        "workspace_source_manifest_sha256": candidate_manifest,
        "cargo_lock_sha256": lock,
    }
    candidate.write_text(json.dumps(identity, sort_keys=True), encoding="utf-8")
    identity["workspace_source_manifest_sha256"] = sealed_manifest
    sealed.write_text(json.dumps(identity, sort_keys=True), encoding="utf-8")

    writer_symbols = runpy.run_path(str(SCRIPT))
    corridor_legs = writer_symbols["_corridor_legs"]()
    production_modules = writer_symbols["_PRODUCTION_MODULES"]
    canonical_production_tests = writer_symbols["_canonical_production_tests"](
        ROOT_DIR
    )
    data_status_test = writer_symbols["_DATA_STATUS_TEST"]
    taira_contract_tests = writer_symbols["_TAIRA_CONTRACT_TESTS"]
    cross_sdk_tests = writer_symbols["_CROSS_SDK_TESTS"]
    js_status_tests = writer_symbols["_JS_STATUS_TESTS"]
    corridor_dir = tmp_path / "corridor"
    corridor_logs_dir = corridor_dir / "logs"
    corridor_logs_dir.mkdir(parents=True)
    required_by_module: dict[str, list[str]] = {}
    for _, module, count in production_modules:
        tests = [
            test
            for test in canonical_production_tests
            if test.startswith(f"{module}::")
        ]
        assert len(tests) == count
        required_by_module[module] = tests
    required_lines = ["module\ttest"]
    for test in canonical_production_tests:
        modules = [
            module
            for _, module, _ in production_modules
            if test.startswith(f"{module}::")
        ]
        assert len(modules) == 1
        required_lines.append(f"{modules[0]}\t{test}")
    corridor_required = corridor_dir / "production-required-tests.tsv"
    corridor_required.write_text("\n".join(required_lines) + "\n", encoding="utf-8")
    corridor_summary_lines = [
        "leg_index\tleg_id\tkind\trequired_test_count\tobserved_test_count\t"
        "command_status\ttee_status\tlog_sha256\tlog\tcommand"
    ]
    corridor_logs = []
    module_by_leg = {
        leg_id: module for leg_id, module, _ in production_modules
    }
    for index, (leg_id, kind, required_count, command) in enumerate(corridor_legs):
        log = corridor_logs_dir / f"{index:02d}-{leg_id}.log"
        if kind.startswith("cargo-"):
            test_lines = []
            if kind == "cargo-module":
                test_lines = [
                    f"test {test} ... ok"
                    for test in required_by_module[module_by_leg[leg_id]]
                ]
            elif leg_id == "status-rust":
                test_lines = [f"test {data_status_test} ... ok"]
            elif leg_id == "cross-sdk-rust":
                test_lines = [f"test {test} ... ok" for test in cross_sdk_tests]
            elif leg_id.startswith("taira-contract-"):
                contract_index = int(leg_id.rsplit("-", 1)[1])
                test_lines = [
                    f"test {taira_contract_tests[contract_index]} ... ok"
                ]
            log_lines = [f"running {required_count} tests", *test_lines, ""]
            log_lines.append(
                f"test result: ok. {required_count} passed; 0 failed; 0 ignored; "
                "0 measured; 42 filtered out; finished in 0.01s"
            )
        elif kind == "pytest":
            log_lines = ["." * required_count, f"{required_count} passed in 0.01s"]
        else:
            log_lines = [
                *(
                    line
                    for test_index, test in enumerate(js_status_tests, 1)
                    for line in (f"# Subtest: {test}", f"ok {test_index} - {test}")
                ),
                f"# pass {required_count}",
                "# fail 0",
            ]
        log.write_text("\n".join(log_lines) + "\n", encoding="utf-8")
        corridor_logs.append(log)
        corridor_summary_lines.append(
            "\t".join(
                (
                    str(index),
                    leg_id,
                    kind,
                    str(required_count),
                    str(required_count),
                    "0",
                    "0",
                    sha256(log),
                    f"logs/{log.name}",
                    command,
                )
            )
        )
    corridor_summary = corridor_dir / "summary.tsv"
    corridor_summary.write_text(
        "\n".join(corridor_summary_lines) + "\n", encoding="utf-8"
    )
    tool_dir = tmp_path / "tools"
    tool_dir.mkdir()
    tool_paths = {}
    for name in (
        "java",
        "cargo",
        "rustc",
        "python3",
        "node",
        "bash",
        "git",
        "tlapm",
        "tla2tools",
        "verus",
        "cargo_verus",
    ):
        path = tool_dir / name
        path.write_text(f"fixture {name}\n", encoding="utf-8")
        tool_paths[name] = path
    corridor_completion = corridor_dir / "COMPLETED.tsv"
    isolated_cargo_home = tool_dir / "cargo-home"
    isolated_cargo_home.mkdir()
    write_tsv(
        corridor_completion,
        {
            "schema_version": "1",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "leg_count": str(len(corridor_legs)),
            "production_required_test_count": str(len(required_lines) - 1),
            "summary_sha256": sha256(corridor_summary),
            "production_required_tests_sha256": sha256(corridor_required),
            "java_path": str(tool_paths["java"].resolve()),
            "java_sha256": sha256(tool_paths["java"]),
            "cargo_path": str(tool_paths["cargo"].resolve()),
            "cargo_sha256": sha256(tool_paths["cargo"]),
            "cargo_version": "cargo 1.93.1 (083ac5135 2025-12-15)",
            "rustc_path": str(tool_paths["rustc"].resolve()),
            "rustc_sha256": sha256(tool_paths["rustc"]),
            "rustc_version": "rustc 1.93.1 (01f6ddf75 2026-02-11)",
            "python3_path": str(tool_paths["python3"].resolve()),
            "python3_sha256": sha256(tool_paths["python3"]),
            "node_path": str(tool_paths["node"].resolve()),
            "node_sha256": sha256(tool_paths["node"]),
            "bash_path": str(tool_paths["bash"].resolve()),
            "bash_sha256": sha256(tool_paths["bash"]),
            "git_path": str(tool_paths["git"].resolve()),
            "git_sha256": sha256(tool_paths["git"]),
            "cargo_home_path": str(isolated_cargo_home.resolve()),
            "repo_cargo_config_sha256": sha256(ROOT_DIR / ".cargo" / "config.toml"),
            "tlc_profile": "ci",
            "tlaps_threads": "4",
        },
    )

    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    formal_log = formal_dir / "formal-gate.log"
    formal_log.write_text(f"formal work\n{FINAL_MARKER}\n", encoding="utf-8")
    formal_ledger = formal_dir / "proof_coverage.json"
    formal_ledger.write_text('{"machine_checked_completion":true}\n', encoding="utf-8")
    formal_evidence = formal_dir / "proof_evidence.json"
    formal_evidence.write_text('{"backend_verification":true}\n', encoding="utf-8")
    formal_harness_lock = formal_dir / "harness-Cargo.lock"
    shutil.copy2(
        ROOT_DIR / "scripts" / "formal" / "sumeragi_v2_harness.lock",
        formal_harness_lock,
    )
    formal_toolchain = formal_dir / "formal-toolchain.tsv"
    formal_toolchain_fields = {"schema_version": "1"}
    for name in ("java", "tlapm", "tla2tools", "verus", "cargo_verus"):
        path = tool_paths[name]
        formal_toolchain_fields[f"{name}_path"] = str(path.resolve())
        formal_toolchain_fields[f"{name}_sha256"] = sha256(path)
    formal_toolchain_fields["tlc_profile"] = "ci"
    formal_toolchain_fields["tlaps_threads"] = "4"
    write_tsv(formal_toolchain, formal_toolchain_fields)
    formal_completion = formal_dir / "COMPLETED.tsv"
    write_tsv(
        formal_completion,
        {
            "schema_version": "1",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "formal_gate_log_sha256": sha256(formal_log),
            "proof_coverage_sha256": sha256(formal_ledger),
            "proof_evidence_sha256": sha256(formal_evidence),
            "harness_cargo_lock_sha256": sha256(formal_harness_lock),
            "formal_toolchain_sha256": sha256(formal_toolchain),
        },
    )

    seed_dir = tmp_path / "seed"
    runs_dir = seed_dir / "runs"
    runs_dir.mkdir(parents=True)
    seed_logs = []
    summary_lines = ["\t".join(SUMMARY_FIELDS)]
    for index in range(128):
        scenario = SCENARIOS[index // 32]
        seed_index = index % 32
        seed = scenario if seed_index == 0 else f"{scenario}:seed:{seed_index:02d}"
        output = f"runs/run-{index:03d}.log"
        run_log = seed_dir / output
        run_log.write_text(
            "\n".join(
                (
                    "running 1 test",
                    f"test sumeragi_v2_runner::{scenario} ... "
                    f"{scenario}: deterministic network seed = {seed}",
                    "ok",
                    "",
                    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                    "42 filtered out; finished in 0.01s",
                )
            )
            + "\n",
            encoding="utf-8",
        )
        seed_logs.append(run_log)
        summary_lines.append(
            "\t".join(
                (
                    "release",
                    sealed_manifest,
                    scenario,
                    seed,
                    "passed",
                    "0",
                    "0",
                    sha256(run_log),
                    output,
                    f"localnets/run-{index:03d}",
                    f"cargo exact release run {index}",
                )
            )
        )
    seed_summary = seed_dir / "summary.tsv"
    seed_summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    seed_completion = seed_dir / "COMPLETED.tsv"
    write_tsv(
        seed_completion,
        {
            "schema_version": "1",
            "profile": "release",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "completed_runs": "128",
            "expected_runs": "128",
            "summary_sha256": sha256(seed_summary),
        },
    )

    chaos_dir = tmp_path / "chaos"
    chaos_dir.mkdir()
    chaos_log = chaos_dir / "chaos-100k.log"
    chaos_log.write_text(
        "\n".join(
            (
                "running 1 test",
                "test accelerated_100_000_block_chaos_preserves_chain_prefix ... "
                "SUMERAGI_V2_CHAOS_COMPLETED permissioned_heights=50000 "
                "npos_heights=50000 total_heights=100000",
                "ok",
                "",
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                "9 filtered out; finished in 0.01s",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    chaos_completion = chaos_dir / "COMPLETED.tsv"
    write_tsv(
        chaos_completion,
        {
            "schema_version": "1",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "permissioned_heights": "50000",
            "npos_heights": "50000",
            "completed_heights": "100000",
            "log_sha256": sha256(chaos_log),
        },
    )

    taira_dir = tmp_path / "taira"
    taira_dir.mkdir()
    taira_evidence = taira_dir / "taira_v2_24h_soak.json"
    taira_evidence.write_text('{"status":"passed"}\n', encoding="utf-8")
    taira_log = taira_dir / "taira-v2-24h.log"
    taira_log.write_text(
        "\n".join(
            (
                "running 1 test",
                "test taira_public_localnet::"
                "taira_profile_24h_packet_impairment_and_restart_soak ... ok",
                "",
                "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                "42 filtered out; finished in 86400.01s",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    taira_completion = taira_dir / "COMPLETED.tsv"
    write_tsv(
        taira_completion,
        {
            "schema_version": "1",
            "head_commit": head,
            "head_tree": tree,
            "source_manifest_sha256": sealed_manifest,
            "cargo_lock_sha256": lock,
            "evidence_sha256": sha256(taira_evidence),
            "log_sha256": sha256(taira_log),
        },
    )
    return {
        "candidate": candidate,
        "sealed": sealed,
        "corridor_completion": corridor_completion,
        "corridor_summary": corridor_summary,
        "corridor_required": corridor_required,
        "corridor_logs": corridor_logs,
        "corridor_log": corridor_logs[0],
        "formal_completion": formal_completion,
        "formal_log": formal_log,
        "formal_ledger": formal_ledger,
        "formal_evidence": formal_evidence,
        "formal_harness_lock": formal_harness_lock,
        "formal_toolchain": formal_toolchain,
        "formal_verus_tool": tool_paths["verus"],
        "corridor_cargo_tool": tool_paths["cargo"],
        "corridor_cargo_home": isolated_cargo_home,
        "seed_completion": seed_completion,
        "seed_summary": seed_summary,
        "seed_logs": seed_logs,
        "seed_log": seed_logs[17],
        "chaos_completion": chaos_completion,
        "chaos_log": chaos_log,
        "taira_completion": taira_completion,
        "taira_evidence": taira_evidence,
        "taira_log": taira_log,
        "candidate_manifest": candidate_manifest,
        "sealed_manifest": sealed_manifest,
        "head": head,
        "tree": tree,
        "lock": lock,
    }


def run_writer(
    evidence: dict[str, Path | str | list[Path]], output: Path, writer: Path
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(writer),
            "--candidate-identity",
            str(evidence["candidate"]),
            "--sealed-identity",
            str(evidence["sealed"]),
            "--corridor-completion",
            str(evidence["corridor_completion"]),
            "--formal-completion",
            str(evidence["formal_completion"]),
            "--seed-completion",
            str(evidence["seed_completion"]),
            "--chaos-completion",
            str(evidence["chaos_completion"]),
            "--taira-completion",
            str(evidence["taira_completion"]),
            "--output",
            str(output),
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def test_receipt_hashes_every_formal_matrix_chaos_and_soak_artifact(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    output = tmp_path / "receipt" / "RELEASE_COMPLETED.json"
    result = run_writer(evidence, output, writer)

    assert result.returncode == 0, result.stderr
    receipt = json.loads(output.read_text(encoding="utf-8"))
    assert receipt["result"] == "release-complete"
    assert receipt["identity"] == {
        "head_commit": evidence["head"],
        "head_tree": evidence["tree"],
        "index_tree": evidence["tree"],
        "cargo_lock_sha256": evidence["lock"],
        "candidate_source_manifest_sha256": evidence["candidate_manifest"],
        "sealed_source_manifest_sha256": evidence["sealed_manifest"],
    }
    expected_artifacts = {
        "corridor_completion": "corridor_completion",
        "corridor_summary": "corridor_summary",
        "corridor_production_inventory": "corridor_required",
        "formal_completion": "formal_completion",
        "formal_gate_log": "formal_log",
        "formal_proof_coverage": "formal_ledger",
        "formal_proof_evidence": "formal_evidence",
        "formal_harness_lock": "formal_harness_lock",
        "formal_toolchain": "formal_toolchain",
        "seed_matrix_completion": "seed_completion",
        "seed_matrix_summary": "seed_summary",
        "chaos_completion": "chaos_completion",
        "chaos_log": "chaos_log",
        "taira_completion": "taira_completion",
        "taira_evidence": "taira_evidence",
        "taira_run_log": "taira_log",
    }
    for receipt_name, fixture_name in expected_artifacts.items():
        fixture_path = evidence[fixture_name]
        assert isinstance(fixture_path, Path)
        assert receipt["evidence"][receipt_name] == {
            "path": str(fixture_path.resolve()),
            "sha256": sha256(fixture_path),
        }
    seed_logs = evidence["seed_logs"]
    assert isinstance(seed_logs, list)
    assert receipt["evidence"]["seed_matrix_run_logs"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)} for path in seed_logs
    ]
    corridor_logs = evidence["corridor_logs"]
    assert isinstance(corridor_logs, list)
    assert receipt["evidence"]["corridor_logs"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)}
        for path in corridor_logs
    ]


@pytest.mark.parametrize(
    "completion_name",
    [
        "corridor_completion",
        "formal_completion",
        "seed_completion",
        "taira_completion",
    ],
)
def test_receipt_rejects_cross_source_completion(
    tmp_path: Path, completion_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence[completion_name]
    assert isinstance(completion, Path)
    completion.write_text(
        completion.read_text(encoding="utf-8").replace("b" * 64, "c" * 64),
        encoding="utf-8",
    )
    output = tmp_path / "RELEASE_COMPLETED.json"
    output.write_text("previous valid receipt\n", encoding="utf-8")

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert (
        "not bound" in result.stderr
        or "exact release matrix" in result.stderr
        or "exact release preflight" in result.stderr
    )
    assert output.read_text(encoding="utf-8") == "previous valid receipt\n"


@pytest.mark.parametrize(
    ("artifact_name", "error_fragment"),
    [
        ("formal_log", "formal gate log digest mismatch"),
        ("formal_ledger", "formal proof ledger digest mismatch"),
        ("formal_evidence", "formal proof evidence digest mismatch"),
        ("formal_toolchain", "formal toolchain digest mismatch"),
        ("formal_verus_tool", "formal verus tool digest mismatch"),
        ("corridor_summary", "corridor summary digest mismatch"),
        ("corridor_required", "corridor production inventory digest mismatch"),
        ("corridor_log", "corridor log 0 digest mismatch"),
        ("corridor_cargo_tool", "corridor cargo tool digest mismatch"),
        ("seed_summary", "summary digest mismatch"),
        ("seed_log", "seed run log 17 digest mismatch"),
        ("chaos_log", "log digest mismatch"),
        ("taira_evidence", "evidence digest mismatch"),
    ],
)
def test_receipt_rejects_artifact_changed_after_completion(
    tmp_path: Path, artifact_name: str, error_fragment: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    artifact.write_text("tampered after completion\n", encoding="utf-8")
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert error_fragment in result.stderr
    assert not output.exists()


def test_receipt_rejects_candidate_and_sealed_git_identity_mismatch(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    candidate_path = evidence["candidate"]
    assert isinstance(candidate_path, Path)
    candidate = json.loads(candidate_path.read_text(encoding="utf-8"))
    candidate["head_commit"] = "9" * 40
    candidate_path.write_text(json.dumps(candidate), encoding="utf-8")
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert "disagree on head_commit" in result.stderr
    assert not output.exists()


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("head_commit", "9" * 40),
        ("head_tree", "8" * 40),
        ("cargo_lock_sha256", "7" * 64),
    ],
)
def test_receipt_rejects_seed_exact_identity_mismatch(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact release matrix" in result.stderr


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("head_commit", "9" * 40),
        ("head_tree", "8" * 40),
        ("cargo_lock_sha256", "7" * 64),
    ],
)
def test_receipt_rejects_taira_exact_identity_mismatch(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["taira_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact release identity" in result.stderr


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("cargo_version", "cargo 9.99.9 (forged 2099-01-01)"),
        ("rustc_version", "rustc 9.99.9 (forged 2099-01-01)"),
    ],
)
def test_receipt_rejects_noncanonical_rust_tool_version(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["corridor_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "rust-toolchain.toml" in result.stderr


def test_receipt_rejects_external_cargo_home_configuration(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    cargo_home = evidence["corridor_cargo_home"]
    assert isinstance(cargo_home, Path)
    (cargo_home / "config.toml").write_text(
        '[target."cfg(all())"]\nrunner = "fake-test-runner"\n', encoding="utf-8"
    )

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "contains external configuration" in result.stderr


def test_receipt_rejects_rehashed_missing_corridor_leg(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    lines = summary.read_text(encoding="utf-8").splitlines()
    summary.write_text("\n".join(lines[:-1]) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "must contain every exact release leg" in result.stderr


def test_receipt_rejects_rehashed_malformed_corridor_log(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    log = evidence["corridor_log"]
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    log.write_text("fabricated pass without Cargo semantics\n", encoding="utf-8")
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[1].split("\t")
    row[7] = sha256(log)
    lines[1] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "ambiguous Cargo transcript" in result.stderr


def test_hand_invoked_writer_rejects_fake_machine_completion_json(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, SCRIPT)

    assert result.returncode == 1
    assert "archived formal ledger/evidence failed release validation" in result.stderr
    assert not output.exists()


def test_receipt_rejects_rehashed_seed_log_without_required_semantics(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    run_log = evidence["seed_log"]
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(run_log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    run_log.write_text("forged success without libtest semantics\n", encoding="utf-8")
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    row[7] = sha256(run_log)
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing scenario" in result.stderr


def test_receipt_requires_exact_nocapture_seed_diagnostic(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    run_log = evidence["seed_log"]
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(run_log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    run_log.write_text(
        run_log.read_text(encoding="utf-8").replace(
            "deterministic network seed = ", "deterministic network seed = wrong-"
        ),
        encoding="utf-8",
    )
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    row[7] = sha256(run_log)
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing scenario" in result.stderr


def test_receipt_rejects_rehashed_chaos_log_without_required_semantics(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    chaos_log = evidence["chaos_log"]
    completion = evidence["chaos_completion"]
    assert isinstance(chaos_log, Path)
    assert isinstance(completion, Path)
    chaos_log.write_text("forged 100000-height success\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["log_sha256"] = sha256(chaos_log)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing release test" in result.stderr


def test_receipt_rejects_seed_summary_row_with_extra_column(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    lines = summary.read_text(encoding="utf-8").splitlines()
    lines[1] += "\tforged-extra-column"
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "extra or missing columns" in result.stderr


def test_receipt_revalidates_archived_taira_semantics(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    taira_log = evidence["taira_log"]
    completion = evidence["taira_completion"]
    assert isinstance(taira_log, Path)
    assert isinstance(completion, Path)
    original_log = taira_log.read_bytes()
    taira_log.write_text(
        "running 1 test\n"
        "test forged_taira_soak ... ok\n\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
        "42 filtered out; finished in 86400.01s\n",
        encoding="utf-8",
    )
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["log_sha256"] = sha256(taira_log)
    write_tsv(completion, fields)

    malformed_result = run_writer(evidence, tmp_path / "malformed-receipt.json", writer)

    assert malformed_result.returncode == 1
    assert "Taira log does not prove its one exact passing soak" in malformed_result.stderr

    taira_log.write_bytes(original_log)
    fields["log_sha256"] = sha256(taira_log)
    write_tsv(completion, fields)
    (writer.parent / "check_taira_v2_soak_evidence.py").write_text(
        "raise SystemExit(72)\n", encoding="utf-8"
    )

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "archived Taira evidence failed release validation" in result.stderr
