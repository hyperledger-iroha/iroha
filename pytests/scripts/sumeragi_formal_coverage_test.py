"""Tests for Sumeragi formal coverage wiring helpers."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "formal" / "check_sumeragi_formal_coverage.py"


def load_coverage_module():
    spec = importlib.util.spec_from_file_location("sumeragi_formal_coverage", SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_documented_fast_table_modes_parses_markdown_rows(tmp_path: Path) -> None:
    module = load_coverage_module()
    readme = tmp_path / "README.md"
    readme.write_text(
        "\n".join(
            [
                "| Other | Table | Notes |",
                "| --- | --- | --- |",
                "| `decoy-fast` | 1 | unrelated table |",
                "",
                "| Mode | Length | Intended use |",
                "| --- | ---: | --- |",
                "| `alpha-fast` | 1 | covered |",
                "| `beta-deep` | 2 | not a TLC fast row |",
                "| `gamma-fast` | 3 | covered |",
            ]
        ),
        encoding="utf-8",
    )

    assert module.documented_fast_table_modes(readme) == [
        "alpha-fast",
        "gamma-fast",
    ]


def test_documented_apalache_length_rows_parses_markdown_rows(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    readme = tmp_path / "README.md"
    readme.write_text(
        "\n".join(
            [
                "| Mode | Length | Intended use |",
                "| --- | ---: | --- |",
                "| `fast` | 10 | commit path |",
                "| `fork-fast` | 9 | fork safety |",
            ]
        ),
        encoding="utf-8",
    )

    assert module.documented_apalache_length_rows(readme) == [
        ("fast", 10),
        ("fork-fast", 9),
    ]


def test_apalache_length_table_shape_errors_rejects_malformed_rows(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    readme = tmp_path / "README.md"
    readme.write_text(
        "\n".join(
            [
                "| Mode | Length | Intended use |",
                "| --- | ---: | --- |",
                "| `frontier-fast` | wide | not numeric |",
                "| frontier-fast | 2 | missing code ticks |",
                "| `quorum-fast` | 2 |",
                "| `rbc-fast` | 1 | valid use | extra cell |",
                "| `commit-fast` | 1 |   |",
            ]
        ),
        encoding="utf-8",
    )

    assert module.apalache_length_table_shape_errors(readme) == [
        f"{readme}:3: README Apalache length for frontier-fast "
        "is not a non-negative integer: wide",
        f"{readme}:4: malformed README Apalache length table row: "
        "| frontier-fast | 2 | missing code ticks |",
        f"{readme}:5: malformed README Apalache length table row: "
        "| `quorum-fast` | 2 |",
        f"{readme}:6: malformed README Apalache length table row: "
        "| `rbc-fast` | 1 | valid use | extra cell |",
        f"{readme}:7: README Apalache length row for commit-fast "
        "has an empty intended-use cell",
    ]


def test_documented_apalache_length_row_duplicates_are_visible(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    readme = tmp_path / "README.md"
    readme.write_text(
        "\n".join(
            [
                "| Mode | Length | Intended use |",
                "| --- | ---: | --- |",
                "| `frontier-fast` | 2 | first declaration |",
                "| `quorum-fast` | 3 | separate mode |",
                "| `frontier-fast` | 4 | conflicting declaration |",
            ]
        ),
        encoding="utf-8",
    )

    rows = module.documented_apalache_length_rows(readme)

    assert module.duplicate_values([mode for mode, _ in rows]) == [
        "frontier-fast"
    ]


def test_command_modes_uses_requested_runner_regex(tmp_path: Path) -> None:
    module = load_coverage_module()
    script = tmp_path / "commands.sh"
    script.write_text(
        "\n".join(
            [
                "# bash scripts/formal/sumeragi_tlc.sh ignored-fast",
                "bash scripts/formal/sumeragi_apalache.sh quorum-fast",
                "bash scripts/formal/sumeragi_tlc.sh frontier-fast",
            ]
        ),
        encoding="utf-8",
    )

    assert module.command_modes(script) == ["quorum-fast"]
    assert module.command_modes(script, module.TLC_COMMAND_RE) == ["frontier-fast"]


def test_command_shape_errors_rejects_malformed_mode_tokens(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "commands.sh"
    script.write_text(
        "\n".join(
            [
                "# bash scripts/formal/sumeragi_apalache.sh ignored?mode",
                "bash scripts/formal/sumeragi_apalache.sh quorum-fast",
                "bash scripts/formal/sumeragi_apalache.sh bad?mode",
                "bash scripts/formal/sumeragi_apalache.sh quoted-fast extra",
                "bash scripts/formal/sumeragi_apalache.sh",
            ]
        ),
        encoding="utf-8",
    )

    assert module.command_shape_errors(
        script,
        module.APALACHE_COMMAND_PREFIX,
        "Apalache command",
    ) == [
        f"Apalache command {script}:3 has invalid mode token 'bad?mode'",
        f"Apalache command {script}:4 has malformed command: "
        "bash scripts/formal/sumeragi_apalache.sh quoted-fast extra",
        f"Apalache command {script}:5 has malformed command: "
        "bash scripts/formal/sumeragi_apalache.sh",
    ]


def test_conflict_marker_errors_rejects_unresolved_merge_markers(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "commands.sh"
    docs = tmp_path / "README.md"
    script.write_text(
        "\n".join(
            [
                "<<<<<<< HEAD",
                "bash scripts/formal/sumeragi_apalache.sh fast",
                "=======",
                "bash scripts/formal/sumeragi_apalache.sh deep",
                ">>>>>>> origin",
            ]
        ),
        encoding="utf-8",
    )
    docs.write_text("TLA terminator text: ====\n", encoding="utf-8")

    assert module.conflict_marker_errors((script, docs)) == [
        f"{script}:1 contains merge conflict marker: <<<<<<< HEAD",
        f"{script}:3 contains merge conflict marker: =======",
        f"{script}:5 contains merge conflict marker: >>>>>>> origin",
    ]


def test_formal_artifact_conflict_marker_scan_uses_tla_cfg_inventory(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Sumeragi.cfg"
    tla = tmp_path / "Sumeragi.tla"
    nested_tla = tmp_path / "nested" / "Hidden.tla"
    readme = tmp_path / "README.md"
    cfg.write_text("INIT Init\n", encoding="utf-8")
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "<<<<<<< HEAD",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    nested_tla.parent.mkdir()
    nested_tla.write_text(
        "\n".join(
            [
                "---- MODULE Hidden ----",
                "=======",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    readme.write_text("<<<<<<< HEAD\n", encoding="utf-8")

    paths = module.formal_artifact_paths(tmp_path)

    assert set(paths) == {cfg, nested_tla, tla}
    assert set(module.conflict_marker_errors(paths)) == {
        f"{nested_tla}:2 contains merge conflict marker: =======",
        f"{tla}:2 contains merge conflict marker: <<<<<<< HEAD",
    }


def test_command_mode_duplicates_are_visible_to_guard(tmp_path: Path) -> None:
    module = load_coverage_module()
    script = tmp_path / "commands.sh"
    script.write_text(
        "\n".join(
            [
                "bash scripts/formal/sumeragi_apalache.sh quorum-fast",
                "bash scripts/formal/sumeragi_apalache.sh frontier-fast",
                "bash scripts/formal/sumeragi_apalache.sh quorum-fast",
            ]
        ),
        encoding="utf-8",
    )

    assert module.duplicate_values(module.command_modes(script)) == ["quorum-fast"]


def test_required_command_errors_reports_missing_entrypoint(tmp_path: Path) -> None:
    module = load_coverage_module()
    workflow = tmp_path / "pr.yml"
    workflow.write_text("run: bash ci/other.sh\n", encoding="utf-8")

    assert module.required_command_errors(
        workflow,
        ("bash ci/check_sumeragi_formal.sh",),
        "PR workflow",
    ) == [
        f"PR workflow {workflow} is missing command: "
        "bash ci/check_sumeragi_formal.sh"
    ]


def test_required_text_errors_reports_missing_runner_semantics(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    runner = tmp_path / "runner.sh"
    runner.write_text('if [[ "$expect_failure" -eq 1 ]]; then\n', encoding="utf-8")

    assert module.required_text_errors(
        runner,
        (
            'if [[ "$expect_failure" -eq 1 ]]; then',
            'if [[ "$status" != "12" ]]; then',
        ),
        "Apalache expected-failure path",
    ) == [
        f"Apalache expected-failure path {runner} is missing required text: "
        'if [[ "$status" != "12" ]]; then'
    ]


def test_command_order_errors_require_guard_before_apalache(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "formal.sh"
    script.write_text(
        "\n".join(
            [
                "bash scripts/formal/sumeragi_apalache.sh fast",
                "python3 scripts/formal/check_sumeragi_formal_coverage.py",
            ]
        ),
        encoding="utf-8",
    )

    assert module.command_order_errors(
        script,
        "python3 scripts/formal/check_sumeragi_formal_coverage.py",
        "bash scripts/formal/sumeragi_apalache.sh",
        "formal baseline script",
    ) == [
        f"formal baseline script {script} must run "
        "'python3 scripts/formal/check_sumeragi_formal_coverage.py' before "
        "'bash scripts/formal/sumeragi_apalache.sh'"
    ]


def test_workflow_entrypoint_errors_require_install_before_baseline(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    pr_workflow = tmp_path / "pr.yml"
    nightly_workflow = tmp_path / "nightly.yml"
    fast_ci = tmp_path / "check_sumeragi_formal.sh"
    pr_workflow.write_text(
        "\n".join(
            [
                "run: bash ci/check_sumeragi_formal.sh",
                "run: bash scripts/formal/install_apalache.sh 0.52.2",
            ]
        ),
        encoding="utf-8",
    )
    nightly_workflow.write_text(
        "\n".join(
            [
                "run: bash scripts/formal/install_apalache.sh 0.52.2",
                "run: bash ci/check_sumeragi_formal.sh",
                "run: bash scripts/formal/sumeragi_apalache.sh frontier-nightly",
            ]
        ),
        encoding="utf-8",
    )
    fast_ci.write_text(
        "\n".join(
            [
                "python3 scripts/formal/check_sumeragi_formal_coverage.py",
                "bash scripts/formal/sumeragi_apalache.sh fast",
                "bash ci/check_sumeragi_formal_expected_failures.sh",
            ]
        ),
        encoding="utf-8",
    )
    module.PR_WORKFLOW = pr_workflow
    module.NIGHTLY_WORKFLOW = nightly_workflow
    module.FAST_CI = fast_ci

    assert module.workflow_entrypoint_errors() == [
        f"PR workflow {pr_workflow} must run "
        "'bash scripts/formal/install_apalache.sh' before "
        "'bash ci/check_sumeragi_formal.sh'"
    ]


def test_single_regex_value_reports_missing_or_duplicate_version(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "runner.sh"
    script.write_text(
        "\n".join(
            [
                'apalache_version="${APALACHE_VERSION:-0.52.2}"',
                'apalache_version="${APALACHE_VERSION:-0.53.0}"',
            ]
        ),
        encoding="utf-8",
    )

    assert module.single_regex_value(
        script,
        module.RUNNER_APALACHE_VERSION_RE,
        "Apalache runner",
    ) == (
        None,
        [f"Apalache runner {script} declares Apalache version 2 times"],
    )


def test_version_values_mismatch_errors_rejects_missing_and_mismatched_pin(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    missing = tmp_path / "missing.yml"
    mismatched = tmp_path / "mismatched.yml"
    missing.write_text("run: echo no install\n", encoding="utf-8")
    mismatched.write_text(
        "run: bash scripts/formal/install_apalache.sh 0.53.0\n",
        encoding="utf-8",
    )

    assert module.version_values_mismatch_errors(
        missing,
        module.INSTALL_APALACHE_COMMAND_VERSION_RE,
        "0.52.2",
        "workflow install command",
    ) == [f"workflow install command {missing} does not declare Apalache 0.52.2"]
    assert module.version_values_mismatch_errors(
        mismatched,
        module.INSTALL_APALACHE_COMMAND_VERSION_RE,
        "0.52.2",
        "workflow install command",
    ) == [
        f"workflow install command {mismatched} uses Apalache 0.53.0, "
        "expected 0.52.2"
    ]


def test_expected_failure_semantics_errors_require_specific_rejections(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    apalache = tmp_path / "sumeragi_apalache.sh"
    tlc = tmp_path / "sumeragi_tlc.sh"
    apalache.write_text(
        "\n".join(
            [
                'if [[ "$expect_failure" == "1" ]]; then',
                'if [[ "$status" == "0" ]]; then',
                'if [[ "$status" != "12" ]]; then',
                "expected Apalache rejection observed",
            ]
        ),
        encoding="utf-8",
    )
    tlc.write_text(
        "\n".join(
            [
                'if [[ "$expect_failure" -eq 1 ]]; then',
                'if [[ "$tlc_status" -eq 0 ]]; then',
                "failed without the expected invariant violation",
                "produced the expected failure",
            ]
        ),
        encoding="utf-8",
    )

    assert module.expected_failure_semantics_errors(apalache, tlc) == [
        f"Apalache expected-failure path {apalache} is missing required text: "
        "expected Apalache invariant rejection",
        f"TLC expected-failure path {tlc} is missing required text: "
        "Invariant .* is violated|Error: Invariant",
    ]


def test_runner_invocation_errors_require_selected_proof_inputs(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    apalache = tmp_path / "sumeragi_apalache.sh"
    tlc = tmp_path / "sumeragi_tlc.sh"
    apalache.write_text(
        'run_with_expected_status "$apalache_bin" check '
        '--config="$cfg_file" "$spec_file"\n',
        encoding="utf-8",
    )
    tlc.write_text(
        "\n".join(
            [
                'java ${TLC_JAVA_OPTS:-} -cp "$tlc_jar" tlc2.TLC',
                '  -workers "$workers"',
                '  -config "$cfg_file"',
                '  "$module"',
            ]
        ),
        encoding="utf-8",
    )

    assert module.runner_invocation_errors(apalache, tlc) == [
        f"Apalache runner invocation {apalache} is missing required text: "
        'check --length="$apalache_length" --config="$cfg_file" '
        '--run-dir="$run_dir" "$spec_file"',
        f"Apalache runner invocation {apalache} is missing required text: "
        'check --length="$apalache_length" --config="$cfg_rel" '
        '--run-dir="$run_rel" "$spec_rel"',
        f"TLC runner invocation {tlc} is missing required text: "
        '-metadir "$run_dir"',
    ]


def test_exact_fast_runner_modes_ignores_wildcards_and_nonfast() -> None:
    module = load_coverage_module()
    cases = {
        "alpha-fast": module.RunnerCase("alpha-fast", "", 1),
        "beta-bug-*": module.RunnerCase("beta-bug-*", "", 2),
        "frontier-small": module.RunnerCase("frontier-small", "", 3),
    }

    assert module.exact_fast_runner_modes(cases) == {"alpha-fast"}


def test_unused_runner_case_labels_reports_unmatched_branches() -> None:
    module = load_coverage_module()
    cases = {
        "frontier-fast": module.RunnerCase("frontier-fast", "", 1),
        "frontier-bug-*": module.RunnerCase("frontier-bug-*", "", 2),
        "stale-bug-*": module.RunnerCase("stale-bug-*", "", 3),
    }

    assert module.unused_runner_case_labels(
        {"frontier-fast", "frontier-bug-stale-owner"}, cases
    ) == ["stale-bug-*"]


def test_runner_case_shadow_errors_detects_prior_wildcards() -> None:
    module = load_coverage_module()
    cases = {
        "rbc-bug-*": module.RunnerCase("rbc-bug-*", "", 10),
        "rbc-bug-duplicate-ready": module.RunnerCase(
            "rbc-bug-duplicate-ready", "", 20
        ),
        "frontier-bug-*": module.RunnerCase("frontier-bug-*", "", 30),
        "frontier-fast": module.RunnerCase("frontier-fast", "", 40),
    }

    assert module.runner_case_shadow_errors(cases, "TLC") == [
        "TLC runner case 'rbc-bug-duplicate-ready' at line 20 "
        "is shadowed by earlier wildcard case 'rbc-bug-*' at line 10"
    ]


def test_runner_case_shadow_errors_allows_exact_before_wildcard() -> None:
    module = load_coverage_module()
    cases = {
        "rbc-bug-duplicate-ready": module.RunnerCase(
            "rbc-bug-duplicate-ready", "", 10
        ),
        "rbc-bug-*": module.RunnerCase("rbc-bug-*", "", 20),
    }

    assert module.runner_case_shadow_errors(cases, "TLC") == []


def test_runner_case_shape_errors_rejects_malformed_case_lines(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    runner = tmp_path / "runner.sh"
    runner.write_text(
        "\n".join(
            [
                'case "$mode" in',
                "  frontier-fast)",
                "    ;;",
                "  frontier-bug-*)",
                "    ;;",
                "  bad+mode)",
                "    ;;",
                "  trailing-fast) echo hidden",
                "    ;; # comment",
                "  missing-term)",
                "    echo no terminator",
                "  *)",
                "    exit 2",
                "    ;;",
                "esac",
            ]
        ),
        encoding="utf-8",
    )

    assert module.runner_case_shape_errors(runner, "TLC") == [
        f"TLC runner {runner}:6 has malformed case label: bad+mode)",
        f"TLC runner {runner}:8 has malformed case label: "
        "trailing-fast) echo hidden",
        f"TLC runner {runner}:9 has malformed case terminator: ;; # comment",
        f"TLC runner {runner}:10 case label has no exact terminator: "
        "missing-term)",
    ]


def test_runner_case_shape_errors_rejects_unindented_case_content(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    runner = tmp_path / "runner.sh"
    runner.write_text(
        "\n".join(
            [
                'case "$mode" in',
                "  frontier-fast)",
                "<<<<<<< HEAD",
                "    ;;",
                "  *)",
                "    exit 2",
                "    ;;",
                "esac",
            ]
        ),
        encoding="utf-8",
    )

    assert module.runner_case_shape_errors(runner, "Apalache") == [
        f"Apalache runner {runner}:3 has malformed case content: <<<<<<< HEAD"
    ]


def test_bug_modes_filters_expected_failure_mutations() -> None:
    module = load_coverage_module()

    assert module.bug_modes(
        [
            "frontier-fast",
            "frontier-bug-stale-owner",
            "quorum-bug-count-under-threshold",
            "deep",
        ]
    ) == {
        "frontier-bug-stale-owner",
        "quorum-bug-count-under-threshold",
    }


def test_matching_case_uses_wildcard_for_tlc_mutations() -> None:
    module = load_coverage_module()
    cases = {
        "frontier-bug-*": module.RunnerCase("frontier-bug-*", "", 1),
        "frontier-fast": module.RunnerCase("frontier-fast", "", 2),
    }

    case = module.matching_case("frontier-bug-stale-owner", cases)

    assert case is not None
    assert case.label == "frontier-bug-*"


def test_matching_case_prefers_exact_tlc_mutation_override() -> None:
    module = load_coverage_module()
    cases = {
        "rbc-bug-duplicate-ready": module.RunnerCase(
            "rbc-bug-duplicate-ready", "", 1
        ),
        "rbc-bug-*": module.RunnerCase("rbc-bug-*", "", 2),
    }

    case = module.matching_case("rbc-bug-duplicate-ready", cases)

    assert case is not None
    assert case.label == "rbc-bug-duplicate-ready"


def test_expected_failure_marker_check_uses_matching_case_body() -> None:
    module = load_coverage_module()
    cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*", "\n    expect_failure=1\n", 10
        ),
        "rbc-bug-*": module.RunnerCase(
            "rbc-bug-*", '\n    cfg_file="$mode.cfg"\n', 20
        ),
    }

    assert module.modes_without_expected_failure_marker(
        {"frontier-bug-stale-owner", "rbc-bug-duplicate-ready"},
        cases,
        "TLC",
    ) == ["rbc-bug-duplicate-ready: TLC runner case 'rbc-bug-*' at line 20"]


def test_unexpected_failure_marker_check_rejects_baseline_modes() -> None:
    module = load_coverage_module()
    cases = {
        "frontier-fast": module.RunnerCase(
            "frontier-fast", "\n    expect_failure=1\n", 10
        ),
        "quorum-fast": module.RunnerCase("quorum-fast", "\n", 20),
    }

    assert module.modes_with_unexpected_failure_marker(
        {"frontier-fast", "quorum-fast"},
        cases,
        "Apalache",
    ) == ["frontier-fast: Apalache runner case 'frontier-fast' at line 10"]


def test_mutation_cfg_equivalence_allows_same_and_commit_roots_tlc_cfg() -> None:
    module = load_coverage_module()
    apalache_cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiFrontier_bug_${cfg_bug_name}.cfg"\n',
            10,
        ),
        "commit-roots-bug-*": module.RunnerCase(
            "commit-roots-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiCommitRootConsistency_bug_${cfg_bug_name}.cfg"\n',
            20,
        ),
    }
    tlc_cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiFrontier_bug_${cfg_bug_name}.cfg"\n',
            30,
        ),
        "commit-roots-bug-*": module.RunnerCase(
            "commit-roots-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiCommitRootConsistency_tlc_bug_${cfg_bug_name}.cfg"\n',
            40,
        ),
    }

    assert (
        module.mutation_cfg_equivalence_errors(
            {"frontier-bug-stale-owner", "commit-roots-bug-under-quorum-accept"},
            apalache_cases,
            tlc_cases,
        )
        == []
    )


def test_mutation_cfg_equivalence_rejects_unexpected_tlc_cfg() -> None:
    module = load_coverage_module()
    apalache_cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiFrontier_bug_${cfg_bug_name}.cfg"\n',
            10,
        )
    }
    tlc_cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiFrontier_tlc_bug_${cfg_bug_name}.cfg"\n',
            20,
        )
    }
    apalache_cfg = module.display_path(
        module.SPEC_DIR / "SumeragiFrontier_bug_stale_owner.cfg"
    )
    tlc_cfg = module.display_path(
        module.SPEC_DIR / "SumeragiFrontier_tlc_bug_stale_owner.cfg"
    )

    assert module.mutation_cfg_equivalence_errors(
        {"frontier-bug-stale-owner"},
        apalache_cases,
        tlc_cases,
    ) == [
        f"frontier-bug-stale-owner: Apalache cfg {apalache_cfg} "
        f"differs from TLC cfg {tlc_cfg}"
    ]


def test_mutation_cfg_name_errors_accepts_matching_cfg_fragments() -> None:
    module = load_coverage_module()
    cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiFrontier_bug_${cfg_bug_name}.cfg"\n',
            10,
        ),
        "rbc-bug-duplicate-ready": module.RunnerCase(
            "rbc-bug-duplicate-ready",
            '\n    cfg_file="$spec_dir/SumeragiRbcDeliverQuorum_bug_duplicate_ready_count.cfg"\n',
            20,
        ),
        "commit-roots-bug-*": module.RunnerCase(
            "commit-roots-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiCommitRootConsistency_tlc_bug_${cfg_bug_name}.cfg"\n',
            30,
        ),
    }

    assert (
        module.mutation_cfg_name_errors(
            {
                "frontier-bug-stale-owner",
                "rbc-bug-duplicate-ready",
                "commit-roots-bug-under-quorum-accept",
            },
            cases,
            "TLC",
        )
        == []
    )


def test_mutation_cfg_name_errors_rejects_mismatched_cfg_fragment() -> None:
    module = load_coverage_module()
    cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            '\n    cfg_file="$spec_dir/SumeragiFrontier_bug_other_case.cfg"\n',
            10,
        )
    }
    cfg = module.display_path(module.SPEC_DIR / "SumeragiFrontier_bug_other_case.cfg")

    assert module.mutation_cfg_name_errors(
        {"frontier-bug-stale-owner"}, cases, "Apalache"
    ) == [
        f"frontier-bug-stale-owner: Apalache cfg {cfg} does not contain "
        "expected mutation fragment _bug_stale_owner"
    ]


def test_module_identity_allows_matching_specs_and_tlc_only_modes() -> None:
    module = load_coverage_module()
    apalache_cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            '\n    spec_file="$spec_dir/SumeragiFrontier.tla"\n',
            10,
        )
    }
    tlc_cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*", '\n    module="SumeragiFrontier"\n', 20
        ),
        "frontier-small": module.RunnerCase(
            "frontier-small", '\n    module="SumeragiFrontierRecovery"\n', 30
        ),
    }

    assert module.module_identity_errors(
        {"frontier-bug-stale-owner", "frontier-small"},
        apalache_cases,
        tlc_cases,
    ) == []


def test_module_identity_rejects_cross_runner_module_drift() -> None:
    module = load_coverage_module()
    apalache_cases = {
        "frontier-fast": module.RunnerCase(
            "frontier-fast",
            '\n    spec_file="$spec_dir/SumeragiFrontier.tla"\n',
            10,
        )
    }
    tlc_cases = {
        "frontier-fast": module.RunnerCase(
            "frontier-fast", '\n    module="SumeragiQuorumPolicy"\n', 20
        )
    }
    apalache_spec = module.display_path(module.SPEC_DIR / "SumeragiFrontier.tla")
    tlc_module = module.display_path(module.SPEC_DIR / "SumeragiQuorumPolicy.tla")

    assert module.module_identity_errors(
        {"frontier-fast"},
        apalache_cases,
        tlc_cases,
    ) == [
        f"frontier-fast: Apalache spec {apalache_spec} "
        f"differs from TLC module {tlc_module}"
    ]


def test_referenced_files_rejects_missing_duplicate_and_dynamic_inputs() -> None:
    module = load_coverage_module()
    files, errors = module.referenced_files(
        "frontier-fast",
        module.RunnerCase(
            "frontier-fast",
            "\n".join(
                [
                    '    spec_file="$spec_dir/SumeragiFrontier.tla"',
                    '    spec_file="$spec_dir/SumeragiQuorumPolicy.tla"',
                    '    cfg_file="$spec_dir/SumeragiFrontier_fast.cfg"',
                ]
            ),
            7,
        ),
    )

    assert files == [module.SPEC_DIR / "SumeragiFrontier_fast.cfg"]
    assert errors == [
        "frontier-fast: runner case 'frontier-fast' at line 7 "
        "assigns spec_file 2 times"
    ]

    assert module.referenced_files(
        "frontier-fast",
        module.RunnerCase(
            "frontier-fast",
            '    spec_file="$spec_dir/${dynamic}.tla"',
            7,
        ),
    ) == (
        [],
        [
            "frontier-fast: spec_file in runner case 'frontier-fast' "
            "did not resolve statically: ${dynamic}.tla",
            "frontier-fast: runner case 'frontier-fast' at line 7 "
            "assigns cfg_file 0 times",
        ],
    )


def test_referenced_files_rejects_malformed_proof_input_assignments() -> None:
    module = load_coverage_module()

    assert module.referenced_files(
        "frontier-fast",
        module.RunnerCase(
            "frontier-fast",
            "\n".join(
                [
                    '    spec_file="$spec_dir/SumeragiFrontier.tla"',
                    '    cfg_file="$spec_dir/SumeragiFrontier_fast.cfg"',
                    "    cfg_file=$spec_dir/SumeragiOther_fast.cfg",
                    '    spec_file="$other_dir/SumeragiOther.tla"',
                    '    cfg_file+="$spec_dir/SumeragiMutated_fast.cfg"',
                    '    readonly spec_file="$spec_dir/SumeragiReadonly.tla"',
                    '    cfg_file[0]="$spec_dir/SumeragiArray_fast.cfg"',
                    '    printf -v spec_file "%s" "$spec_dir/SumeragiPrintf.tla"',
                    "    read -r cfg_file",
                    "    unset spec_file",
                    "    eval 'cfg_file=$spec_dir/SumeragiEval_fast.cfg'",
                ]
            ),
            20,
        ),
    ) == (
        [
            module.SPEC_DIR / "SumeragiFrontier.tla",
            module.SPEC_DIR / "SumeragiFrontier_fast.cfg",
        ],
        [
            "frontier-fast: runner case 'frontier-fast' line 22 "
            "has malformed proof-input assignment: "
            "cfg_file=$spec_dir/SumeragiOther_fast.cfg",
            "frontier-fast: runner case 'frontier-fast' line 23 "
            "has malformed proof-input assignment: "
            'spec_file="$other_dir/SumeragiOther.tla"',
            "frontier-fast: runner case 'frontier-fast' line 24 "
            "has malformed proof-input assignment: "
            'cfg_file+="$spec_dir/SumeragiMutated_fast.cfg"',
            "frontier-fast: runner case 'frontier-fast' line 25 "
            "has malformed proof-input assignment: "
            'readonly spec_file="$spec_dir/SumeragiReadonly.tla"',
            "frontier-fast: runner case 'frontier-fast' line 26 "
            "has malformed proof-input assignment: "
            'cfg_file[0]="$spec_dir/SumeragiArray_fast.cfg"',
            "frontier-fast: runner case 'frontier-fast' line 27 "
            "has malformed proof-input assignment: "
            'printf -v spec_file "%s" "$spec_dir/SumeragiPrintf.tla"',
            "frontier-fast: runner case 'frontier-fast' line 28 "
            "has malformed proof-input assignment: read -r cfg_file",
            "frontier-fast: runner case 'frontier-fast' line 29 "
            "has malformed proof-input assignment: unset spec_file",
            "frontier-fast: runner case 'frontier-fast' line 30 "
            "has malformed proof-input assignment: "
            "eval 'cfg_file=$spec_dir/SumeragiEval_fast.cfg'",
        ],
    )


def test_referenced_files_rejects_path_escape_inputs() -> None:
    module = load_coverage_module()

    assert module.referenced_files(
        "frontier-fast",
        module.RunnerCase(
            "frontier-fast",
            "\n".join(
                [
                    '    spec_file="$spec_dir/../SumeragiFrontier.tla"',
                    '    cfg_file="$spec_dir/nested/SumeragiFrontier_fast.cfg"',
                ]
            ),
            7,
        ),
    ) == (
        [],
        [
            "frontier-fast: spec_file in runner case 'frontier-fast' "
            "must reference a flat Sumeragi formal file: ../SumeragiFrontier.tla",
            "frontier-fast: cfg_file in runner case 'frontier-fast' "
            "must reference a flat Sumeragi formal file: nested/SumeragiFrontier_fast.cfg",
        ],
    )


def test_referenced_files_rejects_wrong_suffix_inputs() -> None:
    module = load_coverage_module()

    assert module.referenced_files(
        "frontier-fast",
        module.RunnerCase(
            "frontier-fast",
            "\n".join(
                [
                    '    spec_file="$spec_dir/SumeragiFrontier.cfg"',
                    '    cfg_file="$spec_dir/SumeragiFrontier.tla"',
                ]
            ),
            7,
        ),
    ) == (
        [],
        [
            "frontier-fast: spec_file in runner case 'frontier-fast' "
            "must reference a .tla file: SumeragiFrontier.cfg",
            "frontier-fast: cfg_file in runner case 'frontier-fast' "
            "must reference a .cfg file: SumeragiFrontier.tla",
        ],
    )


def test_tlc_module_files_resolve_static_module_names() -> None:
    module = load_coverage_module()
    files, errors = module.tlc_module_files(
        "frontier-fast",
        module.RunnerCase("frontier-fast", '\n    module="SumeragiFrontier"\n', 7),
    )

    assert files == [module.SPEC_DIR / "SumeragiFrontier.tla"]
    assert errors == []


def test_tlc_module_files_rejects_missing_or_dynamic_module() -> None:
    module = load_coverage_module()
    assert module.tlc_module_files(
        "frontier-fast", module.RunnerCase("frontier-fast", "", 7)
    ) == (
        [],
        ["frontier-fast: TLC runner case 'frontier-fast' at line 7 assigns module 0 times"],
    )

    assert module.tlc_module_files(
        "frontier-fast",
        module.RunnerCase("frontier-fast", '\n    module="${dynamic}"\n', 7),
    ) == (
        [],
        [
            "frontier-fast: module in TLC runner case 'frontier-fast' "
            "did not resolve statically: ${dynamic}"
        ],
    )

    assert module.tlc_module_files(
        "frontier-fast",
        module.RunnerCase("frontier-fast", '\n    module="Bad-Module"\n', 7),
    ) == (
        [],
        [
            "frontier-fast: module in TLC runner case 'frontier-fast' "
            "must be a TLA identifier: Bad-Module"
        ],
    )


def test_tlc_module_files_rejects_malformed_module_assignments() -> None:
    module = load_coverage_module()

    assert module.tlc_module_files(
        "frontier-fast",
        module.RunnerCase(
            "frontier-fast",
            "\n".join(
                [
                    '    module="SumeragiFrontier"',
                    '    module = "SumeragiOther"',
                    '    module+="SumeragiMutated"',
                    '    export module="SumeragiExported"',
                    '    module[0]="SumeragiArray"',
                    '    printf -v module "%s" SumeragiPrintf',
                    "    unset module",
                    "    eval 'module=SumeragiEval'",
                ]
            ),
            20,
        ),
    ) == (
        [module.SPEC_DIR / "SumeragiFrontier.tla"],
        [
            "frontier-fast: TLC runner case 'frontier-fast' line 21 "
            'has malformed module assignment: module = "SumeragiOther"',
            "frontier-fast: TLC runner case 'frontier-fast' line 22 "
            'has malformed module assignment: module+="SumeragiMutated"',
            "frontier-fast: TLC runner case 'frontier-fast' line 23 "
            'has malformed module assignment: export module="SumeragiExported"',
            "frontier-fast: TLC runner case 'frontier-fast' line 24 "
            'has malformed module assignment: module[0]="SumeragiArray"',
            "frontier-fast: TLC runner case 'frontier-fast' line 25 "
            'has malformed module assignment: printf -v module "%s" SumeragiPrintf',
            "frontier-fast: TLC runner case 'frontier-fast' line 26 "
            "has malformed module assignment: unset module",
            "frontier-fast: TLC runner case 'frontier-fast' line 27 "
            "has malformed module assignment: eval 'module=SumeragiEval'",
        ],
    )


def test_tlc_runner_constraint_errors_require_defined_operator(tmp_path: Path) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "DefinedConstraint ==",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="DefinedConstraint"\n',
            7,
        ),
        tla,
    ) == []
    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="MissingConstraint"\n',
            7,
        ),
        tla,
    ) == [
        f"certified-fetch-fast: TLC runner case 'certified-fetch-fast' "
        f"appends CONSTRAINT MissingConstraint, but {tla} does not define it"
    ]


def test_tlc_runner_constraint_errors_rejects_dynamic_constraint() -> None:
    module = load_coverage_module()

    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="${dynamic}"\n',
            7,
        ),
        module.SPEC_DIR / "Missing.tla",
    ) == [
        "certified-fetch-fast: tlc_constraint in TLC runner case "
        "'certified-fetch-fast' does not name a static TLA operator: ${dynamic}"
    ]


def test_tlc_runner_constraint_errors_rejects_duplicate_assignments() -> None:
    module = load_coverage_module()

    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            "\n".join(
                [
                    "",
                    '    tlc_constraint="FirstConstraint"',
                    '    tlc_constraint="SecondConstraint"',
                ]
            ),
            7,
        ),
        module.SPEC_DIR / "Missing.tla",
    ) == [
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' "
        "at line 7 assigns tlc_constraint 2 times"
    ]


def test_tlc_runner_constraint_errors_rejects_malformed_assignments(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "DefinedConstraint ==",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            "\n".join(
                [
                    '    tlc_constraint="DefinedConstraint"',
                    "    tlc_constraint = OtherConstraint",
                    '    tlc_constraint+="AppendedConstraint"',
                    '    readonly tlc_constraint="ReadonlyConstraint"',
                    '    tlc_constraint[0]="ArrayConstraint"',
                    '    printf -v tlc_constraint "%s" OtherConstraint',
                    "    unset tlc_constraint",
                    "    eval 'tlc_constraint=EvalConstraint'",
                ]
            ),
            30,
        ),
        tla,
    ) == [
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' line 31 "
        "has malformed tlc_constraint assignment: "
        "tlc_constraint = OtherConstraint",
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' line 32 "
        "has malformed tlc_constraint assignment: "
        'tlc_constraint+="AppendedConstraint"',
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' line 33 "
        "has malformed tlc_constraint assignment: "
        'readonly tlc_constraint="ReadonlyConstraint"',
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' line 34 "
        "has malformed tlc_constraint assignment: "
        'tlc_constraint[0]="ArrayConstraint"',
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' line 35 "
        "has malformed tlc_constraint assignment: "
        'printf -v tlc_constraint "%s" OtherConstraint',
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' line 36 "
        "has malformed tlc_constraint assignment: unset tlc_constraint",
        "certified-fetch-fast: TLC runner case 'certified-fetch-fast' line 37 "
        "has malformed tlc_constraint assignment: "
        "eval 'tlc_constraint=EvalConstraint'",
    ]


def test_tlc_runner_constraint_errors_rejects_trivial_constraints(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "TypeInvariant == TRUE",
                "BoundConstraint == counter \\in 0..1",
                "DirectFalse == FALSE",
                "DirectType == TypeInvariant",
                "AliasFalse == DirectFalse",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="BoundConstraint"\n',
            7,
        ),
        tla,
    ) == []
    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="DirectFalse"\n',
            7,
        ),
        tla,
    ) == [
        f"certified-fetch-fast: TLC runner case 'certified-fetch-fast' "
        f"appends CONSTRAINT DirectFalse, but {tla}:4 defines it as literal FALSE"
    ]
    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="DirectType"\n',
            7,
        ),
        tla,
    ) == [
        f"certified-fetch-fast: TLC runner case 'certified-fetch-fast' "
        f"appends CONSTRAINT DirectType, but {tla}:5 aliases "
        "TypeInvariant directly"
    ]
    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="AliasFalse"\n',
            7,
        ),
        tla,
    ) == [
        f"certified-fetch-fast: TLC runner case 'certified-fetch-fast' "
        f"appends CONSTRAINT AliasFalse, but AliasFalse@{tla}:6 -> "
        f"DirectFalse@{tla}:4 resolves to literal FALSE"
    ]


def test_tlc_runner_constraint_errors_rejects_parameterized_constraint(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "BoundConstraint == counter \\in 0..1",
                "ParameterizedConstraint(value) == value \\in 0..1",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="BoundConstraint"\n',
            7,
        ),
        tla,
    ) == []
    assert module.tlc_runner_constraint_errors(
        "certified-fetch-fast",
        module.RunnerCase(
            "certified-fetch-fast",
            '\n    tlc_constraint="ParameterizedConstraint"\n',
            7,
        ),
        tla,
    ) == [
        f"certified-fetch-fast: TLC runner case 'certified-fetch-fast' "
        f"appends CONSTRAINT ParameterizedConstraint, but {tla}:3 defines it "
        "with arity 1; TLC runner constraints must target zero-arity operators"
    ]


def test_apalache_length_errors_require_non_negative_integer() -> None:
    module = load_coverage_module()

    assert (
        module.apalache_length_errors(
            "frontier-fast",
            module.RunnerCase("frontier-fast", "\n    apalache_length=0\n", 7),
        )
        == []
    )
    assert module.apalache_length_errors(
        "frontier-fast", module.RunnerCase("frontier-fast", "", 7)
    ) == [
        "frontier-fast: runner case 'frontier-fast' at line 7 "
        "assigns apalache_length 0 times"
    ]
    assert module.apalache_length_errors(
        "frontier-fast",
        module.RunnerCase("frontier-fast", "\n    apalache_length=wide\n", 7),
    ) == [
        "frontier-fast: apalache_length in runner case 'frontier-fast' "
        "is not a non-negative integer: wide"
    ]


def test_apalache_length_errors_rejects_malformed_assignments() -> None:
    module = load_coverage_module()

    assert module.apalache_length_errors(
        "frontier-fast",
        module.RunnerCase(
            "frontier-fast",
            "\n".join(
                [
                    "    apalache_length=2",
                    "    apalache_length = 3",
                    "    apalache_length+=4",
                    "    declare apalache_length=5",
                    "    apalache_length[0]=6",
                    '    printf -v apalache_length "%s" 7',
                    "    unset apalache_length",
                    "    eval 'apalache_length=8'",
                ]
            ),
            40,
        ),
    ) == [
        "frontier-fast: runner case 'frontier-fast' line 41 "
        "has malformed apalache_length assignment: apalache_length = 3",
        "frontier-fast: runner case 'frontier-fast' line 42 "
        "has malformed apalache_length assignment: apalache_length+=4",
        "frontier-fast: runner case 'frontier-fast' line 43 "
        "has malformed apalache_length assignment: declare apalache_length=5",
        "frontier-fast: runner case 'frontier-fast' line 44 "
        "has malformed apalache_length assignment: apalache_length[0]=6",
        "frontier-fast: runner case 'frontier-fast' line 45 "
        'has malformed apalache_length assignment: printf -v apalache_length "%s" 7',
        "frontier-fast: runner case 'frontier-fast' line 46 "
        "has malformed apalache_length assignment: unset apalache_length",
        "frontier-fast: runner case 'frontier-fast' line 47 "
        "has malformed apalache_length assignment: eval 'apalache_length=8'",
    ]


def test_apalache_length_table_errors_rejects_documented_runner_drift() -> None:
    module = load_coverage_module()
    cases = {
        "frontier-fast": module.RunnerCase(
            "frontier-fast", "\n    apalache_length=1\n", 7
        ),
        "quorum-fast": module.RunnerCase(
            "quorum-fast", "\n    apalache_length=2\n", 12
        ),
    }

    assert module.apalache_length_table_errors(
        {"frontier-fast": 2, "quorum-fast": 2},
        cases,
    ) == [
        "frontier-fast: README length 2 differs from "
        "Apalache runner length 1"
    ]


def test_tla_module_header_errors_validate_declared_module(tmp_path: Path) -> None:
    module = load_coverage_module()
    matching = tmp_path / "SumeragiFrontier.tla"
    mismatched = tmp_path / "SumeragiQuorum.tla"
    missing = tmp_path / "SumeragiEmpty.tla"
    duplicate = tmp_path / "SumeragiDuplicate.tla"
    prefixed = tmp_path / "SumeragiPrefixed.tla"
    no_end = tmp_path / "SumeragiNoEnd.tla"
    duplicate_end = tmp_path / "SumeragiDuplicateEnd.tla"
    trailing_end = tmp_path / "SumeragiTrailingEnd.tla"
    invalid_identifier = tmp_path / "SumeragiInvalidIdentifier.tla"
    reserved_header = tmp_path / "TRUE.tla"
    matching.write_text("---- MODULE SumeragiFrontier ----\n====\n", encoding="utf-8")
    mismatched.write_text("---- MODULE Different ----\n====\n", encoding="utf-8")
    missing.write_text("EXTENDS Naturals\n", encoding="utf-8")
    duplicate.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDuplicate ----",
                "---- MODULE SumeragiDuplicate ----",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    prefixed.write_text(
        "\\* leading content\n---- MODULE SumeragiPrefixed ----\n====\n",
        encoding="utf-8",
    )
    no_end.write_text("---- MODULE SumeragiNoEnd ----\n", encoding="utf-8")
    duplicate_end.write_text(
        "---- MODULE SumeragiDuplicateEnd ----\n====\n====\n",
        encoding="utf-8",
    )
    trailing_end.write_text(
        "---- MODULE SumeragiTrailingEnd ----\n====\nTrailing == TRUE\n",
        encoding="utf-8",
    )
    invalid_identifier.write_text("---- MODULE 123Bad ----\n====\n", encoding="utf-8")
    reserved_header.write_text("---- MODULE TRUE ----\n====\n", encoding="utf-8")

    assert module.tla_module_header_errors("frontier-fast", [matching]) == []
    assert module.tla_module_header_errors(
        "frontier-fast",
        [
            mismatched,
            missing,
            duplicate,
            prefixed,
            no_end,
            duplicate_end,
            trailing_end,
            invalid_identifier,
            reserved_header,
        ],
    ) == [
        f"frontier-fast: {mismatched} declares MODULE Different, expected SumeragiQuorum",
        f"frontier-fast: {missing} has no TLA MODULE declaration",
        f"frontier-fast: {duplicate} declares TLA MODULE 2 times",
        f"frontier-fast: {prefixed}:2 declares MODULE after content at line 1",
        f"frontier-fast: {no_end} declares TLA terminator 0 times",
        f"frontier-fast: {duplicate_end} declares TLA terminator 2 times",
        f"frontier-fast: {trailing_end}:2 has content after TLA terminator",
        f"frontier-fast: {invalid_identifier} has no TLA MODULE declaration",
        f"frontier-fast: {reserved_header}:1 declares reserved TLA MODULE name TRUE",
    ]


def test_cfg_shape_errors_accept_behavior_and_checks(tmp_path: Path) -> None:
    module = load_coverage_module()
    init_next = tmp_path / "InitNext.cfg"
    specification = tmp_path / "Specification.cfg"
    init_next.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CHECK_DEADLOCK FALSE",
                "CONSTANTS",
                "  Bug = 0",
                "INVARIANT TypeInvariant",
            ]
        ),
        encoding="utf-8",
    )
    specification.write_text(
        "\n".join(["SPECIFICATION Spec", "PROPERTIES EventuallyCommits"]),
        encoding="utf-8",
    )

    assert module.cfg_shape_errors("frontier-fast", [init_next, specification]) == []


def test_cfg_directive_errors_rejects_unknown_directives(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTANTS",
                "  Bug = 0",
                "CHECK_DEADLOCK maybe",
                "INVARANT TypeInvariant",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_directive_errors(cfg) == [
        f"{cfg}:5 CHECK_DEADLOCK must be TRUE or FALSE",
        f"{cfg}:6 unknown CFG directive INVARANT",
    ]


def test_cfg_directive_errors_rejects_duplicate_check_deadlock(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CHECK_DEADLOCK FALSE",
                "CHECK_DEADLOCK TRUE",
                "INVARIANT TypeInvariant",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_directive_errors(cfg) == [
        f"{cfg}:4 repeats CHECK_DEADLOCK directive first declared at line 3"
    ]


def test_cfg_shape_errors_rejects_incomplete_configs(tmp_path: Path) -> None:
    module = load_coverage_module()
    empty = tmp_path / "Empty.cfg"
    missing_next = tmp_path / "MissingNext.cfg"
    missing_check = tmp_path / "MissingCheck.cfg"
    mixed = tmp_path / "Mixed.cfg"
    empty.write_text("", encoding="utf-8")
    missing_next.write_text("INIT Init\nINVARIANT TypeInvariant\n", encoding="utf-8")
    missing_check.write_text("INIT Init\nNEXT Next\n", encoding="utf-8")
    mixed.write_text(
        "SPECIFICATION Spec\nINIT Init\nNEXT Next\nINVARIANT TypeInvariant\n",
        encoding="utf-8",
    )

    assert module.cfg_shape_errors(
        "frontier-fast", [empty, missing_next, missing_check, mixed]
    ) == [
        f"frontier-fast: {empty} is empty",
        f"frontier-fast: {missing_next} must define SPECIFICATION or both INIT and NEXT",
        f"frontier-fast: {missing_check} has no invariant or property checks",
        f"frontier-fast: {mixed} mixes SPECIFICATION with INIT/NEXT behavior",
    ]


def test_cfg_shape_errors_rejects_indented_directive_spoofing(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    missing_next = tmp_path / "MissingNext.cfg"
    missing_check = tmp_path / "MissingCheck.cfg"
    missing_next.write_text(
        "\n".join(
            [
                "INIT Init",
                "INVARIANTS",
                "  NEXT",
            ]
        ),
        encoding="utf-8",
    )
    missing_check.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTANTS",
                "  INVARIANT = TypeInvariant",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_shape_errors(
        "frontier-fast", [missing_next, missing_check]
    ) == [
        f"frontier-fast: {missing_next}:3 indented CFG directive NEXT must be top-level",
        f"frontier-fast: {missing_next} must define SPECIFICATION or both INIT and NEXT",
        f"frontier-fast: {missing_check}:4 indented CFG directive INVARIANT must be top-level",
        f"frontier-fast: {missing_check} has no invariant or property checks",
    ]


def test_cfg_operator_references_parse_behavior_and_checks(tmp_path: Path) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "SPECIFICATION Spec",
                "CONSTRAINT TlcStateBound",
                "INVARIANT TypeInvariant",
                "INVARIANTS SafetyFast BugCheck",
                "PROPERTIES",
                "  EventuallyCommits",
                "  EventuallyRecovers",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_references(cfg) == (
        [
            (1, "SPECIFICATION", "Spec"),
            (2, "CONSTRAINT", "TlcStateBound"),
            (3, "INVARIANT", "TypeInvariant"),
            (4, "INVARIANTS", "SafetyFast"),
            (4, "INVARIANTS", "BugCheck"),
            (6, "PROPERTIES", "EventuallyCommits"),
            (7, "PROPERTIES", "EventuallyRecovers"),
        ],
        [],
    )


def test_cfg_operator_references_reject_malformed_operator_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "MalformedOperators.cfg"
    cfg.write_text(
        "\n".join(
            [
                "SPECIFICATION Spec!",
                "NEXT THEOREM",
                "PROPERTY WF_vars",
                "INVARIANTS Safety Good",
                "INVARIANTS Safety Bad-Name TRUE",
                "PROPERTIES",
                "  Eventually Extra",
                "  FALSE",
                "  EventuallyRecovers",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_references(cfg) == (
        [
            (4, "INVARIANTS", "Safety"),
            (4, "INVARIANTS", "Good"),
            (5, "INVARIANTS", "Safety"),
            (9, "PROPERTIES", "EventuallyRecovers"),
        ],
        [
            f"{cfg}:1 directive SPECIFICATION must reference a static TLA operator: Spec!",
            f"{cfg}:2 directive NEXT must reference a static TLA operator: THEOREM",
            f"{cfg}:3 directive PROPERTY must reference a static TLA operator: WF_vars",
            f"{cfg}:5 directive INVARIANTS must reference static TLA operators: Bad-Name",
            f"{cfg}:5 directive INVARIANTS must reference static TLA operators: TRUE",
            f"{cfg}:7 PROPERTIES block line must reference exactly one static TLA operator",
            f"{cfg}:8 PROPERTIES block line must reference exactly one static TLA operator",
        ],
    )


def test_cfg_operator_references_reject_vars_tuple_as_operator_target(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "VarsTarget.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT vars",
                "INVARIANTS Safety vars",
                "PROPERTIES",
                "  EventuallyRecovers",
                "  vars",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_references(cfg) == (
        [
            (1, "INIT", "Init"),
            (3, "INVARIANTS", "Safety"),
            (5, "PROPERTIES", "EventuallyRecovers"),
        ],
        [
            f"{cfg}:2 directive NEXT must reference a TLA operator other than "
            "vars tuple",
            f"{cfg}:3 directive INVARIANTS must reference TLA operators other "
            "than vars tuple",
            f"{cfg}:6 PROPERTIES block line must reference a TLA operator "
            "other than vars tuple",
        ],
    )


def test_cfg_operator_references_reject_indented_directive_lines(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "IndentedDirective.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INVARIANTS",
                "  NEXT",
                "PROPERTIES",
                "  CHECK_DEADLOCK",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_references(cfg) == (
        [],
        [
            f"{cfg}:2 indented CFG directive NEXT must be top-level",
            f"{cfg}:4 indented CFG directive CHECK_DEADLOCK must be top-level",
        ],
    )


def test_cfg_operator_references_reject_empty_multiline_check_blocks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "EmptyBlocks.cfg"
    cfg.write_text(
        "\n".join(
            [
                "SPECIFICATION Spec",
                "INVARIANTS",
                "PROPERTIES",
                "INVARIANT TypeInvariant",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_references(cfg) == (
        [
            (1, "SPECIFICATION", "Spec"),
            (4, "INVARIANT", "TypeInvariant"),
        ],
        [
            f"{cfg}:2 INVARIANTS block must reference at least one static TLA operator",
            f"{cfg}:3 PROPERTIES block must reference at least one static TLA operator",
        ],
    )


def test_tla_operator_definitions_parse_plain_local_and_parameterized(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "LOCAL Helper ==",
                "Parameterized(value) ==",
                "  ScopedLetHelper ==",
                "TypeInvariant ==",
                "RECURSIVE RecursiveOne(_), RecursiveTwo(_, _)",
                "RecursiveOne(value) == TRUE",
                "RecursiveTwo(left, right) == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_operator_definitions(tla) == {
        "Helper",
        "Parameterized",
        "RecursiveOne",
        "RecursiveTwo",
        "TypeInvariant",
    }


def test_tla_duplicate_operator_definition_errors_rejects_malformed_signatures(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Good(value) == TRUE",
                "Bad(, value) == TRUE",
                "Empty() == TRUE",
                "Broken(value == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_operator_definition_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:3 TLA operator definition must use static "
        "parameters: Bad(, value) == TRUE",
        f"frontier-fast: {tla}:4 TLA operator definition must use static "
        "parameters: Empty() == TRUE",
        f"frontier-fast: {tla}:5 TLA operator definition must use a static "
        "signature: Broken(value == TRUE",
    ]
    assert module.tla_operator_definitions(tla) == {"Good"}


def test_tla_duplicate_operator_definition_errors_rejects_reserved_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "THEOREM == TRUE",
                "SF_Action == TRUE",
                "ReservedParam(THEOREM) == TRUE",
                "RECURSIVE INSTANCE, RecursiveBad(TRUE), RecursiveOk(_)",
                "RecursiveOk(value) == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_operator_definition_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:5 RECURSIVE declaration must list static "
        "operator declarations: INSTANCE, RecursiveBad(TRUE)",
        f"frontier-fast: {tla}:3 TLA operator definition must use a "
        "non-reserved static name: SF_Action == TRUE",
        f"frontier-fast: {tla}:4 TLA operator definition must use static "
        "parameters: ReservedParam(THEOREM) == TRUE",
    ]
    assert module.tla_operator_definitions(tla) == {"RecursiveOk"}


def test_tla_operator_definitions_ignore_named_instance_aliases(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "RealOperator == TRUE",
                "Alias == INSTANCE Named",
                "Configured == INSTANCE WithConfig WITH Foo <- Bar",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_operator_definitions(tla) == {"RealOperator"}


def test_tla_literal_operator_definitions_parse_top_level_literals(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "LiteralTrue == TRUE",
                "LiteralFalse(_) == FALSE \\* comment",
                "LOCAL LocalLiteral == TRUE",
                "Multiline ==",
                "  TRUE",
                "UsesLet ==",
                "  LET scoped == FALSE",
                "  IN scoped",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_literal_operator_definitions(tla) == {
        "LiteralTrue": (2, "TRUE"),
        "LiteralFalse": (3, "FALSE"),
        "LocalLiteral": (4, "TRUE"),
        "Multiline": (6, "TRUE"),
    }


def test_tla_type_invariant_alias_definitions_parse_top_level_aliases(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "InlineAlias == TypeInvariant",
                "MultilineAlias ==",
                "  TypeInvariant",
                "NotAlias == TypeInvariant /\\ TRUE",
                "UsesLet ==",
                "  LET scoped == TypeInvariant",
                "  IN scoped",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_type_invariant_alias_definitions(tla) == {
        "InlineAlias": (2, "TypeInvariant"),
        "MultilineAlias": (4, "TypeInvariant"),
    }


def test_tla_static_boolean_literal_evaluates_obvious_wrappers() -> None:
    module = load_coverage_module()

    assert module.tla_static_boolean_literal("TRUE /\\ TRUE") == "TRUE"
    assert module.tla_static_boolean_literal("(FALSE \\/ FALSE)") == "FALSE"
    assert module.tla_static_boolean_literal("~FALSE") == "TRUE"
    assert module.tla_static_boolean_literal("/\\ TRUE") == "TRUE"
    assert module.tla_static_boolean_literal("counter = 0") is None
    assert module.tla_static_boolean_literal("TRUE /\\ Safety") is None


def test_tla_trivial_operator_chains_parse_transitive_aliases(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "LiteralHelper == TRUE",
                "AliasToLiteral == LiteralHelper",
                "TypeHelper == TypeInvariant",
                "AliasToType ==",
                "  TypeHelper",
                "RealCheck == AliasToLiteral /\\ TypeInvariant",
                "CycleA == CycleB",
                "CycleB == CycleA",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    chains = module.tla_trivial_operator_chains(tla)

    assert chains["AliasToLiteral"] == [
        ("AliasToLiteral", 3, "LiteralHelper"),
        ("LiteralHelper", 2, "TRUE"),
    ]
    assert chains["AliasToType"] == [
        ("AliasToType", 6, "TypeHelper"),
        ("TypeHelper", 4, "TypeInvariant"),
    ]
    assert "RealCheck" not in chains
    assert "CycleA" not in chains


def test_tla_trivial_operator_chains_parse_boolean_literal_expressions(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "WrappedLiteral == (TRUE)",
                "ConjLiteral == TRUE /\\ TRUE",
                "FalseList ==",
                "  \\/ FALSE",
                "  \\/ FALSE",
                "AliasToWrapped == (WrappedLiteral)",
                "AliasToFalseList == FalseList",
                "RealCheck == TRUE /\\ Safety",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    chains = module.tla_trivial_operator_chains(tla)

    assert chains["WrappedLiteral"] == [("WrappedLiteral", 2, "TRUE")]
    assert chains["ConjLiteral"] == [("ConjLiteral", 3, "TRUE")]
    assert chains["AliasToWrapped"] == [
        ("AliasToWrapped", 7, "WrappedLiteral"),
        ("WrappedLiteral", 2, "TRUE"),
    ]
    assert chains["FalseList"] == [("FalseList", 5, "FALSE")]
    assert chains["AliasToFalseList"] == [
        ("AliasToFalseList", 8, "FalseList"),
        ("FalseList", 5, "FALSE"),
    ]
    assert "RealCheck" not in chains


def test_tla_duplicate_operator_definition_errors_rejects_repeated_top_level_entries(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Helper ==",
                "  TRUE",
                "Helper ==",
                "  FALSE",
                "RECURSIVE Recur(_), Recur(_)",
                "Recur(value) == TRUE",
                "UsesLet ==",
                "  LET scoped == TRUE",
                "      scoped == FALSE",
                "  IN scoped",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_operator_definition_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:4 repeats TLA operator definition Helper "
        "first declared at line 2",
        f"frontier-fast: {tla}:6 repeats TLA RECURSIVE declaration Recur "
        "first declared at line 6",
    ]


def test_tla_duplicate_operator_definition_errors_require_recursive_definitions(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "RECURSIVE Missing(_), Mismatch(_, _), Good(_)",
                "Mismatch(value) == TRUE",
                "Good(value) == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_operator_definition_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:2 declares TLA RECURSIVE operator Missing, "
        "but no top-level definition exists",
        f"frontier-fast: {tla}:2 declares TLA RECURSIVE operator Mismatch "
        "with arity 2, but definition at line 3 has arity 1",
    ]


def test_tla_duplicate_operator_definition_errors_rejects_malformed_recursive(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "RECURSIVE Good(_), Bad(, _)",
                "RECURSIVE Good(_),, Hidden(_)",
                "RECURSIVE Broken(_, Hidden",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_operator_definition_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:2 RECURSIVE declaration must list static "
        "operator declarations: Bad(, _)",
        f"frontier-fast: {tla}:3 RECURSIVE declaration must list static "
        "operator declarations: empty recursive declaration entry",
        f"frontier-fast: {tla}:4 RECURSIVE declaration must list static "
        "operator declarations: unbalanced parentheses",
    ]
    assert module.tla_recursive_declaration_entries(tla) == []


def test_tla_module_dependency_references_parse_extends_and_instances(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "EXTENDS Naturals, LocalHelpers \\* comment",
                "LOCAL INSTANCE Imported",
                "Alias == INSTANCE Named",
                "Configured == INSTANCE WithConfig WITH Foo <- Bar",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [
            (2, "EXTENDS", "Naturals"),
            (2, "EXTENDS", "LocalHelpers"),
            (3, "INSTANCE", "Imported"),
            (4, "INSTANCE", "Named"),
            (5, "INSTANCE", "WithConfig"),
        ],
        [],
    )


def test_tla_module_dependency_references_rejects_malformed_extends(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "EXTENDS Naturals,,FiniteSets",
                "EXTENDS Naturals + Hidden",
                "EXTENDS Naturals,",
                "EXTENDS TRUE",
                "INSTANCE Imported",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(6, "INSTANCE", "Imported")],
        [
            f"{tla}:2 EXTENDS must list static module identifiers: Naturals,,FiniteSets",
            f"{tla}:3 EXTENDS must list static module identifiers: Naturals + Hidden",
            f"{tla}:4 EXTENDS must list static module identifiers: Naturals,",
            f"{tla}:5 EXTENDS must list non-reserved static module identifiers: TRUE",
        ],
    )


def test_tla_module_dependency_references_rejects_malformed_instances(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "INSTANCE 123Bad",
                "Alias == INSTANCE Named + Hidden",
                "LOCAL INSTANCE",
                "INSTANCE TRUE",
                "Alias == INSTANCE WF_Module",
                "TRUE == INSTANCE Imported",
                "WF_Alias == INSTANCE Imported",
                "INSTANCE Imported",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(9, "INSTANCE", "Imported")],
        [
            f"{tla}:2 INSTANCE must reference a static module identifier: INSTANCE 123Bad",
            f"{tla}:3 INSTANCE must reference a static module identifier: "
            "Alias == INSTANCE Named + Hidden",
            f"{tla}:4 INSTANCE must reference a static module identifier: LOCAL INSTANCE",
            f"{tla}:5 INSTANCE must reference a non-reserved static module "
            "identifier: INSTANCE TRUE",
            f"{tla}:6 INSTANCE must reference a non-reserved static module "
            "identifier: Alias == INSTANCE WF_Module",
            f"{tla}:7 INSTANCE alias must be a non-reserved static identifier: "
            "TRUE == INSTANCE Imported",
            f"{tla}:8 INSTANCE alias must be a non-reserved static identifier: "
            "WF_Alias == INSTANCE Imported",
        ],
    )


def test_tla_module_dependency_errors_rejects_missing_local_module(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    helper = tmp_path / "LocalHelpers.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "EXTENDS Naturals, LocalHelpers, MissingHelpers",
                "INSTANCE MissingInstance",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    helper.write_text("---- MODULE LocalHelpers ----\n====\n", encoding="utf-8")

    assert module.tla_module_dependency_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:2 references EXTENDS module MissingHelpers, "
        f"but neither TLA standard module nor {tmp_path / 'MissingHelpers.tla'} exists",
        f"frontier-fast: {tla}:3 references INSTANCE module MissingInstance, "
        f"but neither TLA standard module nor {tmp_path / 'MissingInstance.tla'} exists",
    ]


def test_tla_forbidden_directive_errors_rejects_top_level_assumptions_and_proofs(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "ASSUME FALSE",
                "ASSUMPTION TRUE",
                "AXIOM TypeInvariant",
                "THEOREM TypeInvariant",
                "PROOF OMITTED",
                "QED",
                "  ASSUME LocalHelper",
                "  THEOREM LocalHelper",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_forbidden_directive_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:2 uses top-level ASSUME directive; "
        "Sumeragi formal modules must be assumption-free",
        f"frontier-fast: {tla}:3 uses top-level ASSUMPTION directive; "
        "Sumeragi formal modules must be assumption-free",
        f"frontier-fast: {tla}:4 uses top-level AXIOM directive; "
        "Sumeragi formal modules must be assumption-free",
        f"frontier-fast: {tla}:5 uses top-level THEOREM directive; "
        "Sumeragi formal modules must be proof-free",
        f"frontier-fast: {tla}:6 uses top-level PROOF directive; "
        "Sumeragi formal modules must be proof-free",
        f"frontier-fast: {tla}:7 uses top-level QED directive; "
        "Sumeragi formal modules must be proof-free",
    ]


def test_cfg_constant_bindings_parse_inline_and_block_assignments(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                'CONSTANT Bug = "none"',
                "CONSTANTS",
                "  MaxView = 2",
                "  Toggle <- Bool",
                "INIT Init",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [(1, "Bug"), (3, "MaxView"), (4, "Toggle")],
        [],
    )


def test_cfg_constant_bindings_rejects_malformed_block_line(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "CONSTANTS",
                "  MaxView = 2",
                "  MissingBinding",
                "  Toggle <- Bool",
                "INIT Init",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [(2, "MaxView"), (4, "Toggle")],
        [f"{cfg}:3 CONSTANTS block line must bind exactly one constant"],
    )


def test_cfg_constant_bindings_rejects_ambiguous_inline_binding(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "CONSTANT Bug = \"none\" Hidden = TRUE",
                "CONSTANT MissingBinding",
                "INIT Init",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [],
        [
            f"{cfg}:1 directive CONSTANT contains nested binding-looking token Hidden",
            f"{cfg}:2 directive CONSTANT must bind exactly one constant",
        ],
    )


def test_cfg_constant_bindings_rejects_reserved_binding_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "CONSTANT TRUE = Bool",
                "CONSTANTS",
                "  WF_guard = 1",
                "  MaxView = 2",
                "INIT Init",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [(4, "MaxView")],
        [
            f"{cfg}:1 directive CONSTANT must bind a non-reserved static constant: TRUE",
            f"{cfg}:3 CONSTANTS block line must bind a non-reserved static constant: WF_guard",
        ],
    )


def test_cfg_constant_bindings_rejects_empty_constant_blocks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "CONSTANTS",
                "INIT Init",
                "CONSTANT",
                "",
                "NEXT Next",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [],
        [
            f"{cfg}:1 CONSTANTS block must bind at least one constant",
            f"{cfg}:3 CONSTANTS block must bind at least one constant",
        ],
    )


def test_cfg_constant_bindings_rejects_indented_directive_lines(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Model.cfg"
    cfg.write_text(
        "\n".join(
            [
                "CONSTANTS",
                "  MaxView = 2",
                "  INVARIANT = TypeInvariant",
                "  Toggle <- Bool",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [(2, "MaxView")],
        [f"{cfg}:3 indented CFG directive INVARIANT must be top-level"],
    )


def test_tla_constant_declarations_parse_plain_and_annotated_blocks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS",
                "  \\* @type: Int;",
                "  MaxView,",
                "  \\* @type: Bool;",
                "  Toggle",
                "CONSTANT",
                "  Bug",
                "VARIABLE checked",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_constant_declarations(tla) == {
        "Bug",
        "MaxView",
        "Toggle",
    }


def test_tla_duplicate_constant_declaration_errors_rejects_repeated_constants(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS",
                "  MaxView,",
                "  Toggle",
                "CONSTANT MaxView",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_constant_declaration_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:5 repeats TLA constant declaration MaxView "
        "first declared at line 3"
    ]


def test_tla_duplicate_constant_declaration_errors_rejects_empty_blocks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS",
                "VARIABLE checked",
                "CONSTANT",
                "",
                "vars == <<checked>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_constant_declaration_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:2 CONSTANTS block must declare at least one identifier",
        f"frontier-fast: {tla}:4 CONSTANTS block must declare at least one identifier",
    ]


def test_tla_duplicate_constant_declaration_errors_rejects_malformed_lines(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS Good, , Bad",
                "CONSTANTS",
                "  MaxView,",
                "VARIABLE checked",
                "vars == <<checked>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_constant_declaration_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:2 CONSTANTS declaration line must list static "
        "identifiers: Good, , Bad",
        f"frontier-fast: {tla}:4 CONSTANTS declaration block ends with "
        "trailing comma",
    ]


def test_tla_duplicate_constant_declaration_errors_rejects_reserved_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS TRUE",
                "CONSTANTS",
                "  WF_guard",
                "VARIABLE checked",
                "vars == <<checked>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_constant_declaration_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:2 CONSTANTS declaration line must list "
        "non-reserved static identifiers: TRUE",
        f"frontier-fast: {tla}:4 CONSTANTS declaration line must list "
        "non-reserved static identifiers: WF_guard",
    ]


def test_tla_variable_surface_errors_accepts_matching_multiline_vars_tuple(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLES",
                "  checked,",
                "  accepted",
                "vars ==",
                "  <<checked,",
                "    accepted",
                ">>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == []


def test_tla_variable_surface_errors_rejects_empty_variable_blocks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLES",
                "CONSTANT MaxView",
                "VARIABLE",
                "",
                "vars == <<>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:2 VARIABLES block must declare at least one identifier",
        f"frontier-fast: {tla}:4 VARIABLES block must declare at least one identifier",
        f"frontier-fast: {tla}:6 vars must list static variables",
    ]


def test_tla_variable_surface_errors_rejects_malformed_declaration_lines(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLES",
                "  checked",
                "  bad-token",
                "vars == <<checked>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:4 VARIABLES declaration line must list static "
        "identifiers: bad-token",
    ]


def test_tla_variable_surface_errors_rejects_reserved_declarations_and_vars(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLE TRUE",
                "VARIABLES",
                "  WF_state",
                "  checked",
                "vars == <<checked, FALSE, SF_step>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:2 VARIABLES declaration line must list "
        "non-reserved static identifiers: TRUE",
        f"frontier-fast: {tla}:4 VARIABLES declaration line must list "
        "non-reserved static identifiers: WF_state",
        f"frontier-fast: {tla}:6 vars must list non-reserved static variables: FALSE",
        f"frontier-fast: {tla}:6 vars must list non-reserved static variables: SF_step",
    ]


def test_tla_variable_surface_errors_rejects_duplicates_and_tuple_drift(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLES",
                "  checked,",
                "  checked,",
                "  missing",
                "vars == <<checked, extra, extra>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:4 repeats TLA variable declaration checked "
        "first declared at line 3",
        f"frontier-fast: {tla}:6 repeats vars tuple variable extra "
        "first declared at line 6",
        f"frontier-fast: {tla} declares variable missing "
        "but vars does not include it",
        f"frontier-fast: {tla} vars includes undeclared variable extra",
    ]


def test_tla_variable_surface_errors_rejects_dynamic_vars_tuple(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLE checked",
                "vars == DynamicVars",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:3 vars must be a static tuple",
        f"frontier-fast: {tla} declares variable checked but vars does not include it",
    ]


def test_tla_variable_surface_errors_rejects_wrapped_vars_tuple_expression(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLE checked",
                "vars == IF TRUE THEN <<checked>> ELSE <<checked>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:3 vars must be a static tuple",
        f"frontier-fast: {tla} declares variable checked but vars does not include it",
    ]


def test_cfg_constant_binding_errors_rejects_undeclared_constant(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Model.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANT Bug",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text(
        "\n".join(
            [
                'CONSTANT Bug = "known"',
                "CONSTANTS",
                '  MissingConstant = "unknown"',
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_binding_errors("frontier-fast", tla, cfg) == [
        f"frontier-fast: {cfg}:3 binds constant MissingConstant, "
        f"but {tla} does not declare it"
    ]


def test_cfg_constant_binding_errors_rejects_unbound_declared_constant(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Model.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS",
                "  Bug,",
                "  MissingBinding",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text('CONSTANT Bug = "known"\n', encoding="utf-8")

    assert module.cfg_constant_binding_errors("frontier-fast", tla, cfg) == [
        f"frontier-fast: {cfg} does not bind constant MissingBinding "
        f"declared by {tla}"
    ]


def test_cfg_module_ownership_errors_accepts_module_prefixed_cfg_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    exact = tmp_path / "Model.cfg"
    suffixed = tmp_path / "Model_fast.cfg"

    assert module.cfg_module_ownership_errors("frontier-fast", tla, exact) == []
    assert module.cfg_module_ownership_errors("frontier-fast", tla, suffixed) == []


def test_cfg_module_ownership_errors_rejects_cross_module_cfg_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Other_fast.cfg"

    assert module.cfg_module_ownership_errors("frontier-fast", tla, cfg) == [
        f"frontier-fast: CFG {cfg} does not belong to TLA module {tla}; "
        "expected filename stem Model or Model_*"
    ]


def test_cfg_duplicate_constant_binding_errors_accepts_unique_bindings(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "UniqueConstants.cfg"
    cfg.write_text(
        "\n".join(
            [
                'CONSTANT Bug = "none"',
                "CONSTANTS",
                "  MaxView = 2",
                "  Toggle = FALSE",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_duplicate_constant_binding_errors("frontier-fast", cfg) == []


def test_cfg_duplicate_constant_binding_errors_rejects_repeated_bindings(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "DuplicateConstants.cfg"
    cfg.write_text(
        "\n".join(
            [
                'CONSTANT Bug = "first"',
                "CONSTANTS",
                "  MaxView = 2",
                '  Bug = "second"',
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_duplicate_constant_binding_errors(
        "frontier-fast", cfg
    ) == [
        f"frontier-fast: {cfg}:4 repeats constant binding Bug "
        "first declared at line 1"
    ]


def test_cfg_operator_reference_errors_rejects_missing_module_operator(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Model.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Init ==",
                "Next ==",
                "TypeInvariant ==",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANTS TypeInvariant MissingInvariant",
                "PROPERTY MissingLiveness",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_reference_errors("frontier-fast", tla, cfg) == [
        f"frontier-fast: {cfg}:3 references INVARIANTS operator MissingInvariant, "
        f"but {tla} does not define it",
        f"frontier-fast: {cfg}:4 references PROPERTY operator MissingLiveness, "
        f"but {tla} does not define it",
    ]


def test_cfg_operator_reference_errors_rejects_instance_alias_targets(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Model.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Imported == INSTANCE ImportedModel",
                "Safety == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANTS TypeInvariant Imported Safety",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_reference_errors("frontier-fast", tla, cfg) == [
        f"frontier-fast: {cfg}:3 references INVARIANTS operator Imported, "
        f"but {tla} does not define it",
    ]


def test_cfg_operator_reference_errors_rejects_parameterized_targets(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Model.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Init(value) == TRUE",
                "Next(value) == TRUE",
                "BoundConstraint(value) == TRUE",
                "Safety(value) == TRUE",
                "Eventually(value) == TRUE",
                "RECURSIVE RecursiveSafety(_)",
                "RecursiveSafety(value) == TRUE",
                "Good == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTRAINT BoundConstraint",
                "INVARIANTS Safety Good RecursiveSafety",
                "PROPERTY Eventually",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_reference_errors("frontier-fast", tla, cfg) == [
        f"frontier-fast: {cfg}:1 references INIT operator Init, but {tla}:2 "
        "defines it with arity 1; CFG references must target zero-arity operators",
        f"frontier-fast: {cfg}:2 references NEXT operator Next, but {tla}:3 "
        "defines it with arity 1; CFG references must target zero-arity operators",
        f"frontier-fast: {cfg}:3 references CONSTRAINT operator "
        f"BoundConstraint, but {tla}:4 defines it with arity 1; CFG references "
        "must target zero-arity operators",
        f"frontier-fast: {cfg}:4 references INVARIANTS operator Safety, but "
        f"{tla}:5 defines it with arity 1; CFG references must target "
        "zero-arity operators",
        f"frontier-fast: {cfg}:4 references INVARIANTS operator "
        f"RecursiveSafety, but {tla}:8 defines it with arity 1; CFG references "
        "must target zero-arity operators",
        f"frontier-fast: {cfg}:5 references PROPERTY operator Eventually, but "
        f"{tla}:6 defines it with arity 1; CFG references must target "
        "zero-arity operators",
    ]


def test_cfg_trivial_check_operator_errors_rejects_trivial_semantic_checks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Model.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SafetyInvariant == TRUE",
                "EventuallyRecovers == FALSE",
                "LiteralHelper == TRUE",
                "AliasToLiteral == LiteralHelper",
                "TypeHelper == TypeInvariant",
                "AliasCheck == TypeInvariant",
                "AliasToType ==",
                "  TypeHelper",
                "RealCheck ==",
                "  TypeInvariant /\\ AliasCheck",
                "BooleanSafety == TRUE /\\ TRUE",
                "WrappedFalse == (FALSE)",
                "MultilineBoolean ==",
                "  /\\ TRUE",
                "  /\\ TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANTS TypeInvariant SafetyInvariant AliasCheck AliasToLiteral AliasToType RealCheck BooleanSafety WrappedFalse MultilineBoolean",
                "PROPERTY EventuallyRecovers",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_trivial_check_operator_errors(
        "frontier-fast", tla, cfg, "Apalache"
    ) == [
        f"frontier-fast: Apalache cfg {cfg}:3 references INVARIANTS check "
        f"SafetyInvariant, but {tla}:5 defines it as literal TRUE",
        f"frontier-fast: Apalache cfg {cfg}:3 references INVARIANTS check "
        f"AliasCheck, but {tla}:10 aliases TypeInvariant directly",
        f"frontier-fast: Apalache cfg {cfg}:3 references INVARIANTS check "
        f"AliasToLiteral, but AliasToLiteral@{tla}:8 -> LiteralHelper@{tla}:7 "
        "resolves to literal TRUE",
        f"frontier-fast: Apalache cfg {cfg}:3 references INVARIANTS check "
        f"AliasToType, but AliasToType@{tla}:12 -> TypeHelper@{tla}:9 "
        "resolves to TypeInvariant",
        f"frontier-fast: Apalache cfg {cfg}:3 references INVARIANTS check "
        f"BooleanSafety, but {tla}:15 defines it as literal TRUE",
        f"frontier-fast: Apalache cfg {cfg}:3 references INVARIANTS check "
        f"WrappedFalse, but {tla}:16 defines it as literal FALSE",
        f"frontier-fast: Apalache cfg {cfg}:3 references INVARIANTS check "
        f"MultilineBoolean, but {tla}:18 defines it as literal TRUE",
        f"frontier-fast: Apalache cfg {cfg}:4 references PROPERTY check "
        f"EventuallyRecovers, but {tla}:6 defines it as literal FALSE",
    ]


def test_cfg_trivial_check_operator_errors_rejects_trivial_constraints(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    cfg = tmp_path / "Model.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "BoundConstraint == counter \\in 0..1",
                "FalseConstraint == FALSE",
                "AliasConstraint == FalseConstraint",
                "WrappedFalseConstraint == (FALSE \\/ FALSE)",
                "MultilineFalseConstraint ==",
                "  /\\ FALSE",
                "  /\\ FALSE",
                "Safety == counter >= 0",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTRAINT BoundConstraint",
                "CONSTRAINT FalseConstraint",
                "CONSTRAINT AliasConstraint",
                "CONSTRAINT WrappedFalseConstraint",
                "CONSTRAINT MultilineFalseConstraint",
                "INVARIANT TypeInvariant",
                "INVARIANT Safety",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_trivial_check_operator_errors(
        "frontier-fast", tla, cfg, "TLC"
    ) == [
        f"frontier-fast: TLC cfg {cfg}:4 references CONSTRAINT operator "
        f"FalseConstraint, but {tla}:6 defines it as literal FALSE",
        f"frontier-fast: TLC cfg {cfg}:5 references CONSTRAINT operator "
        f"AliasConstraint, but AliasConstraint@{tla}:7 -> "
        f"FalseConstraint@{tla}:6 resolves to literal FALSE",
        f"frontier-fast: TLC cfg {cfg}:6 references CONSTRAINT operator "
        f"WrappedFalseConstraint, but {tla}:8 defines it as literal FALSE",
        f"frontier-fast: TLC cfg {cfg}:7 references CONSTRAINT operator "
        f"MultilineFalseConstraint, but {tla}:10 defines it as literal FALSE",
    ]


def test_cfg_duplicate_operator_reference_errors_accepts_unique_checks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Unique.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTRAINT BoundState",
                "INVARIANT TypeInvariant",
                "INVARIANTS Safety LivenessBridge",
                "PROPERTY EventuallyCommit",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_duplicate_operator_reference_errors("frontier-fast", cfg) == []


def test_cfg_duplicate_operator_reference_errors_rejects_repeated_constraint(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "DuplicateConstraint.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTRAINT BoundState",
                "CONSTRAINT NarrowState",
                "INVARIANT Safety",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_duplicate_operator_reference_errors(
        "frontier-fast", cfg
    ) == [
        f"frontier-fast: {cfg}:4 repeats CONSTRAINT directive first declared at line 3",
    ]


def test_cfg_duplicate_operator_reference_errors_rejects_repeated_entries(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Duplicate.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "NEXT NextAgain",
                "CONSTRAINT BoundState",
                "CONSTRAINT NarrowState",
                "INVARIANT TypeInvariant",
                "INVARIANTS TypeInvariant Safety",
                "PROPERTIES EventuallyCommit EventuallyCommit",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_duplicate_operator_reference_errors(
        "frontier-fast", cfg
    ) == [
        f"frontier-fast: {cfg}:3 repeats NEXT behavior directive first declared at line 2",
        f"frontier-fast: {cfg}:5 repeats CONSTRAINT directive first declared at line 4",
        f"frontier-fast: {cfg}:7 repeats INVARIANT check TypeInvariant first declared at line 6",
        f"frontier-fast: {cfg}:8 repeats PROPERTY check EventuallyCommit first declared at line 8",
    ]


def test_cfg_semantic_check_errors_accepts_non_type_check(tmp_path: Path) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiFrontier_fast.cfg"
    cfg.write_text(
        "\n".join(["INIT Init", "NEXT Next", "INVARIANTS TypeInvariant Safety"]),
        encoding="utf-8",
    )

    assert (
        module.cfg_semantic_check_errors("frontier-fast", cfg, "Apalache")
        == []
    )


def test_cfg_semantic_check_errors_rejects_type_only_check(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiFrontier_fast.cfg"
    cfg.write_text(
        "\n".join(["INIT Init", "NEXT Next", "INVARIANT TypeInvariant"]),
        encoding="utf-8",
    )

    assert module.cfg_semantic_check_errors(
        "frontier-fast", cfg, "TLC"
    ) == [
        f"frontier-fast: TLC cfg {cfg} "
        "has no non-TypeInvariant invariant/property check"
    ]


def test_cfg_fast_generic_check_errors_rejects_fast_generic_checks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiFrontier_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "INVARIANT SafetyFast",
                "INVARIANTS DirectSafety Safety",
                "INVARIANT NoBugInvariant",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_fast_generic_check_errors(
        "frontier-fast", cfg, "Apalache"
    ) == [
        f"frontier-fast: Apalache cfg {cfg}:4 references generic check "
        "SafetyFast; fast configs must use a model-specific direct invariant",
        f"frontier-fast: Apalache cfg {cfg}:5 references generic check "
        "Safety; fast configs must use a model-specific direct invariant",
        f"frontier-fast: Apalache cfg {cfg}:6 references generic check "
        "NoBugInvariant; fast configs must use a model-specific direct invariant",
    ]


def test_cfg_fast_generic_check_errors_accepts_direct_fast_checks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiFrontier_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "INVARIANT FrontierDirectSafety",
                "INVARIANTS FrontierAnchors FrontierExactness",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_fast_generic_check_errors("frontier-fast", cfg, "TLC")
        == []
    )


def test_cfg_fast_generic_check_errors_ignores_mutation_configs(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiFrontier_bug_stale.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "INVARIANT NoBugInvariant",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_fast_generic_check_errors(
            "frontier-bug-stale", cfg, "Apalache"
        )
        == []
    )


def test_unreferenced_formal_file_errors_require_inventory_reachability(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    used_tla = tmp_path / "Used.tla"
    used_cfg = tmp_path / "Used.cfg"
    orphan_tla = tmp_path / "Orphan.tla"
    orphan_cfg = tmp_path / "Orphan.cfg"
    nested_orphan = tmp_path / "nested" / "Hidden.tla"
    nested_orphan.parent.mkdir()
    for path in (used_tla, used_cfg, orphan_tla, orphan_cfg, nested_orphan):
        path.write_text("", encoding="utf-8")
    module.SPEC_DIR = tmp_path

    assert module.unreferenced_formal_file_errors({used_tla, used_cfg}) == [
        f"{orphan_cfg} is not referenced by any checked or documented "
        "Sumeragi formal mode",
        f"{orphan_tla} is not referenced by any checked or documented "
        "Sumeragi formal mode",
        f"{nested_orphan} is not referenced by any checked or documented "
        "Sumeragi formal mode",
    ]
    assert module.unreferenced_formal_file_errors(
        {used_tla, used_cfg, orphan_tla, orphan_cfg, nested_orphan}
    ) == []


def test_duplicate_values_reports_each_duplicate_once() -> None:
    module = load_coverage_module()

    assert module.duplicate_values(
        ["frontier-fast", "quorum-fast", "frontier-fast", "quorum-fast", "quorum-fast"]
    ) == ["frontier-fast", "quorum-fast"]


def test_runner_case_labels_parse_duplicates_for_guarding(tmp_path: Path) -> None:
    module = load_coverage_module()
    runner = tmp_path / "runner.sh"
    runner.write_text(
        "\n".join(
            [
                "case \"$mode\" in",
                "  frontier-fast)",
                "    ;;",
                "  quorum-fast)",
                "    ;;",
                "  frontier-fast)",
                "    ;;",
                "esac",
            ]
        ),
        encoding="utf-8",
    )

    assert module.runner_case_labels(runner) == [
        "frontier-fast",
        "quorum-fast",
        "frontier-fast",
    ]
    assert module.duplicate_values(module.runner_case_labels(runner)) == [
        "frontier-fast"
    ]
